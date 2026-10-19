//! Transient incompressible laminar flow: PISO and PIMPLE.
//!
//! Per time step (kinematic pressure, constant viscosity):
//!
//! * **Momentum predictor** – assemble `ddt(U) + div(phi U) - div(nu grad U)`
//!   with the flux of the previous (outer) iterate and solve
//!   `A U = b - V grad p`.
//! * **PISO correctors** (Issa, 1986) – with the frozen momentum matrix, form
//!   `HbyA = H(U) / a_P`, the face flux `phiHbyA = HbyA_f . S` plus the
//!   transient Rhie-Chow (Choi, 1999) term that removes the time-step
//!   dependence of the collocated interpolation, solve
//!   `div(rAU grad p) = div(phiHbyA)`, then `phi = phiHbyA - rAU_f snGrad(p) |S|`
//!   and `U = HbyA - rAU grad p`.
//! * **PIMPLE** – wraps the above in `n_outer` outer iterations per step with
//!   implicit under-relaxation (`alpha_u`, `alpha_p`) on all but the last
//!   outer iteration, which allows Courant numbers above one. With
//!   `n_outer = 1` the algorithm is plain PISO.

use crate::bc::FixedValue;
use crate::convection::{convection, face_flux, ConvScheme};
use crate::ddt::{add_ddt, max_courant, CflController, History, TimeScheme};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{green_gauss, least_squares};
use crate::lumped::FaceJump;
use crate::pressure::{
    boundary_hby_a, boundary_values_pbc, corrected_flux, divergence, face_interpolate, pressure_bc, solve_pressure,
    PressureControls,
};
use crate::simple::{normalised_residual, BodyForce};
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::{LinearOptions, SolveControl};
use std::sync::Arc;

pub mod fractional;
pub mod moving;

/// Settings of the transient solver.
#[derive(Debug, Clone)]
pub struct PimpleSettings {
    /// Kinematic viscosity.
    pub nu: f64,
    /// Temporal scheme (`ImplicitEuler` or `Bdf2`).
    pub time_scheme: TimeScheme,
    /// Convection scheme for momentum.
    pub convection: ConvScheme,
    /// Outer iterations per step (1 = PISO).
    pub n_outer: usize,
    /// Pressure correctors per outer iteration (PISO uses 2).
    pub n_correctors: usize,
    /// Extra pressure solves with non-orthogonal correction per corrector.
    pub non_orth_correctors: usize,
    /// Velocity under-relaxation on non-final outer iterations.
    pub alpha_u: f64,
    /// Pressure under-relaxation on non-final outer iterations.
    pub alpha_p: f64,
    /// Apply the transient Rhie-Chow correction.
    pub ddt_correction: bool,
    /// Consistent momentum interpolation (ADR 0073 F-G): subtract `C(G p - b)` from the face flux of every corrector, with
    /// `C(x) = interp(rAU x).S - interp(rAU) interp(x).S` (the lagged pressure of the corrector, `b` the body-force cell value), so that the face
    /// flux is the textbook Rhie-Chow form `interp(u) - gamma (snGrad p - interp(grad p))` also where `rAU` varies in space; and carry the
    /// transient face defect over translationally periodic faces too. Removes the O(dt) dependence of the converged spatial operator that
    /// the variable `rAU = dt/(c0 + a dt)` causes (the BDF2 order floor of ADR 0069). Translationally and rotationally periodic seams
    /// (ADR 0076). Default `false`.
    pub rhie_chow_consistent: bool,
    /// Periodic seams on the first outer iteration of a BDF2 step (ADR 0076 F-S): the coupled-face coefficients of the momentum predictor
    /// come from the extrapolated velocity, like the convected cells, instead of `u^n` (whose lag is first order in time). Default `false`.
    pub seam_extrapolation: bool,
    /// On the first outer iteration of a BDF2 step the implicit under-relaxation of the momentum predictor anchors at the extrapolated
    /// velocity instead of `u^n` (ADR 0076 F-R; the `u^n` anchor makes an under-relaxed PIMPLE with few outer iterations first order in
    /// time). The pressure relaxation is unchanged. Default `false`.
    pub relax_extrapolated: bool,
    /// Consistent split of the momentum matrix when its diagonal differs between the velocity components,
    /// `HbyA_k = (H_k + (a_P - a_k) u_k) / a_P` (ADR 0069 F3). Default `false`: `HbyA_k = H_k / a_P`.
    pub component_diagonal: bool,
    /// Reference cell and value for a pressure level that is otherwise undetermined.
    pub p_ref: (usize, f64),
    /// Use AMG-preconditioned CG for the pressure equation.
    pub amg_pressure: bool,
    /// Linear solver controls for momentum.
    pub momentum_solver: SolveControl,
    /// Linear solver controls for pressure.
    pub pressure_solver: SolveControl,
    /// Linear-solver device of the pressure equation (ADR 0038); default CPU.
    pub linear: LinearOptions,
    /// Add the transposed part `div(nu_eff (grad u)^T)` of the viscous stress divergence as an explicit
    /// (deferred-correction) source (ADR 0032). Default `false`: Laplacian form only.
    pub full_viscous_stress: bool,
    /// Non-iterative fractional-step (projection) advance instead of the PISO/PIMPLE loop (ADR 0037); `None` = PISO.
    pub fractional: Option<fractional::FractionalSettings>,
    /// Divergence guard (ADR 0067): a step whose maximum Courant number exceeds this limit, or whose velocity, pressure or flux
    /// is not finite, stops the run with a located [`Divergence`]. Default [`DEFAULT_DIVERGENCE_COURANT_LIMIT`]; a non-finite
    /// state is always fatal.
    pub divergence_courant_limit: f64,
    /// Run-time check of the pressure-lag amplification of the step (ADR 0073 B4); `None` (default) = no check.
    pub pressure_lag_check: Option<LagCheck>,
}

/// What the pressure-lag check does when the estimated amplification reaches one (ADR 0073 B4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LagAction {
    /// Append a [`LagWarning`] to `PimpleSolver::lag_warnings` and continue.
    Warn,
    /// Stop the run through the divergence guard with a located message.
    Raise,
}

/// Run-time check of the pressure-lag amplification (ADR 0073 B4).
///
/// With one pressure corrector the pressure of an outer iteration inherits the lag error of the pressure that drove its momentum
/// predictor, multiplied by about `-D` (`D` = off-diagonal part of the momentum diagonal times `dt/c0`, the diffusion number of the
/// diagonal for a smooth mode); under-relaxed outer iterations damp it only partly, so the step amplifies a smooth pressure error once
/// `D` exceeds a threshold (`D* = 1, 3.33, 4.65, 5.65` for `n_outer = 1..4` with `alpha_u = 0.8`, `alpha_p = 0.5`). The check estimates
/// the spectral radius `rho` of the linearised pressure-lag map of one step by power iteration with the matrices of the final outer
/// iteration frozen (velocity error zero at the step start, homogeneous pressure conditions, no carry-over and no body force) and acts
/// when `rho >= 1`. Settings rule: use `n_correctors >= 2` (a second corrector starts from a projected velocity and removes the lag) or
/// a smaller step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LagCheck {
    /// Check at step 1 and then every `every` steps (0 = step 1 only).
    pub every: usize,
    /// Power iterations per estimate.
    pub iterations: usize,
    /// Action when `rho >= 1`.
    pub action: LagAction,
}

impl LagCheck {
    /// Check with 20 power iterations.
    pub fn new(every: usize, action: LagAction) -> Self {
        LagCheck {
            every,
            iterations: 20,
            action,
        }
    }
}

/// One estimate of the pressure-lag check (ADR 0073 B4).
#[derive(Debug, Clone, Copy)]
pub struct LagWarning {
    /// Step (1-based) at which the estimate was made.
    pub step: usize,
    /// Estimated amplification per step of the lagged pressure error.
    pub rho: f64,
    /// Cell holding the largest entry of the estimated mode.
    pub cell: usize,
}

/// A periodic face: (face, partner face, weight of the face's owner, rotation of the partner vectors).
type SeamFace = (usize, usize, f64, Option<[[f64; 3]; 3]>);

/// Matrices of the final outer iteration kept for the pressure-lag check.
struct LagCtx {
    mats: Vec<Option<FvMatrix>>,
    links: Vec<PeriodicLinks>,
    pbc: crate::pressure::PressureBc,
}

/// Default Courant number above which a step is declared diverged (ADR 0067).
pub const DEFAULT_DIVERGENCE_COURANT_LIMIT: f64 = 1e6;

/// Where and why the divergence guard stopped a run (ADR 0067).
#[derive(Debug, Clone)]
pub struct Divergence {
    /// Step number (1-based) that tripped.
    pub step: usize,
    /// Outer iteration (0-based) in progress, `None` for a check after the step.
    pub outer: Option<usize>,
    /// Stage of the algorithm: `momentum predictor`, `pressure right-hand side`, `pressure solve`,
    /// `velocity correction`, `end of step`.
    pub stage: &'static str,
    /// Quantity that was bad: `velocity`, `phiHbyA`, `pressure`, `flux`, `courant`.
    pub quantity: &'static str,
    /// Cell (or owner cell of a face) holding the first bad value, if known.
    pub cell: Option<usize>,
    /// The offending value.
    pub value: f64,
    /// The Courant limit that applied.
    pub limit: f64,
    /// The record returned for the tripping step and for every later `step` call.
    pub info: StepInfo,
}

impl Divergence {
    /// One-line located message; `centres` (cell centres of the mesh) adds the position of the cell.
    pub fn message(&self, centres: Option<&[Vec3]>) -> String {
        let at = match (self.cell, centres) {
            (Some(c), Some(x)) if c < x.len() => {
                format!(", cell {c} at ({:.4e}, {:.4e}, {:.4e})", x[c][0], x[c][1], x[c][2])
            }
            (Some(c), _) => format!(", cell {c}"),
            _ => String::new(),
        };
        let outer = self
            .outer
            .map_or(String::new(), |o| format!(", outer iteration {}", o + 1));
        let what = if self.quantity == "courant" {
            format!(
                "maximum Courant number is {:.4e}, above the limit {:.1e}",
                self.value, self.limit
            )
        } else if self.quantity == "lag amplification" {
            return format!(
                "PIMPLE pressure-lag check: the lagged pressure error is amplified by {:.3} per step (>= 1) at step {}, {}{at}; \
                 with one pressure corrector the run will diverge: use solver.n_correctors >= 2 or a smaller solver.dt",
                self.value, self.step, self.stage
            );
        } else {
            format!("{} is {} (not finite)", self.quantity, self.value)
        };
        format!(
            "PIMPLE divergence guard: {what} at step {}{outer}, {}{at}; lower solver.dt or the relaxation factors",
            self.step, self.stage
        )
    }
}

fn first_nonfinite(v: &[f64]) -> Option<(usize, f64)> {
    v.iter().copied().enumerate().find(|(_, x)| !x.is_finite())
}

impl PimpleSettings {
    /// PISO defaults: Euler time scheme, two correctors, one outer iteration.
    pub fn piso(nu: f64) -> Self {
        PimpleSettings {
            nu,
            time_scheme: TimeScheme::ImplicitEuler,
            convection: ConvScheme::Upwind,
            n_outer: 1,
            n_correctors: 2,
            non_orth_correctors: 0,
            alpha_u: 1.0,
            alpha_p: 1.0,
            ddt_correction: true,
            rhie_chow_consistent: false,
            seam_extrapolation: false,
            relax_extrapolated: false,
            component_diagonal: false,
            p_ref: (0, 0.0),
            amg_pressure: false,
            momentum_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-8,
                max_iter: 500,
            },
            pressure_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-7,
                max_iter: 3000,
            },
            linear: LinearOptions::default(),
            full_viscous_stress: false,
            fractional: None,
            divergence_courant_limit: DEFAULT_DIVERGENCE_COURANT_LIMIT,
            pressure_lag_check: None,
        }
    }

    /// PIMPLE defaults: several outer iterations with under-relaxation.
    pub fn pimple(nu: f64, n_outer: usize) -> Self {
        PimpleSettings {
            n_outer,
            n_correctors: 1,
            alpha_u: 0.8,
            alpha_p: 0.5,
            ..Self::piso(nu)
        }
    }
}

/// Explicit transposed part of the viscous stress divergence, `S_i = d_j (nu d_i u_j)` per unit volume at every cell
/// (ADR 0032). The face value of `grad u` is the linear interpolation of the cell Green-Gauss gradients, `nu_face` the
/// face viscosity of every face (internal faces only enter; boundary faces contribute nothing).
pub fn transposed_stress_source(geom: &FvGeometry, u: &VectorField, nu_face: &[f64]) -> Vec<Vec3> {
    let mesh = &geom.mesh;
    let n = geom.n_cells();
    let (own, nei) = (mesh.owner(), mesh.neighbour());
    let sf = mesh.face_area_vectors();
    let vol = mesh.cell_volumes();
    let ub = u.boundary_values(geom);
    // grads[j][c][i] = d_i u_j
    let grads: Vec<Vec<Vec3>> = (0..3)
        .map(|j| {
            if geom.active[j] {
                let cells: Vec<f64> = u.values.iter().map(|v| v[j]).collect();
                let bv: Vec<f64> = ub.iter().map(|v| v[j]).collect();
                green_gauss(geom, &cells, &bv, 0)
            } else {
                vec![[0.0; 3]; n]
            }
        })
        .collect();
    let mut out = vec![[0.0; 3]; n];
    for f in 0..geom.n_internal {
        let (o, nb) = (own[f], nei[f]);
        let w = geom.weights[f];
        let mut flux = [0.0; 3];
        for (i, fl) in flux.iter_mut().enumerate() {
            for j in 0..3 {
                let gf = w * grads[j][o][i] + (1.0 - w) * grads[j][nb][i];
                *fl += nu_face[f] * gf * sf[f][j];
            }
        }
        for i in 0..3 {
            out[o][i] += flux[i] / vol[o];
            out[nb][i] -= flux[i] / vol[nb];
        }
    }
    // translationally periodic (coupled) faces: both sides of the pair visit their own face, the partner cell supplies the
    // other half of the face gradient (rotationally periodic patches are skipped, a named limitation)
    for (pi, patch) in mesh.patches().iter().enumerate() {
        let Some(map) = &geom.periodic_partner[pi] else {
            continue;
        };
        if geom.periodic_rot[pi].is_some() || !u.bc(pi).is_coupled() {
            continue;
        }
        for (local, f) in patch.faces().enumerate() {
            let g = map[local];
            let (dn_o, dn_p) = (geom.dn[f - geom.n_internal], geom.dn[g - geom.n_internal]);
            let w = dn_p / (dn_o + dn_p);
            let (o, q) = (own[f], own[g]);
            for i in 0..3 {
                let mut fl = 0.0;
                for j in 0..3 {
                    let gf = w * grads[j][o][i] + (1.0 - w) * grads[j][q][i];
                    fl += nu_face[f] * gf * sf[f][j];
                }
                out[o][i] += fl / vol[o];
            }
        }
    }
    out
}

/// Diagnostics of one time step.
#[derive(Debug, Clone, Copy)]
pub struct StepInfo {
    /// Step number (1-based).
    pub step: usize,
    /// Time after the step.
    pub time: f64,
    /// Step size used.
    pub dt: f64,
    /// Maximum Courant number of the new flux.
    pub courant: f64,
    /// Largest cell imbalance of the corrected flux, normalised by the face-area scale.
    pub continuity: f64,
    /// Normalised initial momentum residual of the first outer iteration.
    pub res_u: f64,
    /// Normalised initial pressure residual of the first corrector of the last outer iteration.
    pub res_p: f64,
    /// Linear-solver iterations spent on pressure.
    pub p_iters: usize,
}

/// Momentum-side sources of one outer iteration, returned by the hook of [`PimpleSolver::step_with`] (ADR 0045). The same
/// quantities the steady kernel receives: an explicit body acceleration (balanced-force face values), an implicit
/// isotropic sink, pressure jumps at internal faces (fans, porous jumps, loss closures) and fixed pressures on named
/// patches (network outlets, fan boundaries).
#[derive(Debug, Clone, Default)]
pub struct OuterUpdate {
    /// Explicit source acceleration (replaces `PimpleSolver::source_force`).
    pub source_force: Option<BodyForce>,
    /// Implicit sink per cell, `-sink * u`, in 1/s (replaces `PimpleSolver::sink`).
    pub sink: Option<Vec<f64>>,
    /// Face pressure jumps (replaces `PimpleSolver::face_jump`).
    pub face_jump: Option<FaceJump>,
    /// New fixed kinematic pressure of named pressure patches.
    pub patch_pressures: Vec<(String, f64)>,
}

/// Hook of [`PimpleSolver::step_with`]: called at the start of every outer iteration with the current velocity and flux.
pub type OuterHook<'a> = dyn FnMut(&VectorField, &[f64]) -> OuterUpdate + 'a;

/// Transient PISO/PIMPLE solver state.
pub struct PimpleSolver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Settings.
    pub settings: PimpleSettings,
    /// Velocity.
    pub u: VectorField,
    /// Kinematic pressure.
    pub p: ScalarField,
    /// Face flux (positive out of the owner).
    pub phi: Vec<f64>,
    /// Current time.
    pub time: f64,
    /// Diagnostics of every step.
    pub history: Vec<StepInfo>,
    hist_u: [History; 3],
    phi_hist: History,
    step_no: usize,
    /// Kinematic viscosity at every face; `None` uses `settings.nu`.
    pub nu_face: Option<Vec<f64>>,
    /// Body acceleration (balanced-force face values, as in the steady solver), e.g. Boussinesq buoyancy.
    pub body_force: Option<BodyForce>,
    /// Explicit source acceleration of the outer iteration in progress, set by the `step_with` hook (ADR 0045); summed with
    /// `body_force` and applied through the same four balanced-force places. `None` leaves the solver unchanged.
    pub source_force: Option<BodyForce>,
    /// Implicit linear momentum sink per cell, `-sink * u`, in 1/s (porous media, Lorentz damping), set by the hook.
    pub sink: Option<Vec<f64>>,
    /// Pressure jumps at internal faces (lumped fans and porous jumps), set by the hook; same algebra as the SIMPLE kernel.
    pub face_jump: Option<FaceJump>,
    /// Moving-mesh data of the current step (set only by `step_ale`, ADR 0033).
    pub(crate) ale: Option<moving::AleStep>,
    /// Pressure increment of the previous fractional step (lagged boundary treatment, ADR 0037).
    pub(crate) fs_q: Vec<f64>,
    /// Set when the divergence guard stopped the run (ADR 0067); every later `step` returns at once. Cleared by `restore`
    /// and `reinitialise`.
    pub diverged: Option<Divergence>,
    /// Every estimate of the pressure-lag check (ADR 0073 B4).
    pub lag_estimates: Vec<LagWarning>,
    /// Estimates of the pressure-lag check with `rho >= 1` made in `Warn` mode.
    pub lag_warnings: Vec<LagWarning>,
}

impl PimpleSolver {
    /// Create a solver at `t = 0`; the initial flux is interpolated from `u`.
    pub fn new(geom: Arc<FvGeometry>, u: VectorField, p: ScalarField, settings: PimpleSettings) -> Self {
        let bu = u.boundary_values(&geom);
        let phi = face_flux(&geom, &u.values, &bu);
        let hist_u = [0, 1, 2].map(|k| History::new(u.values.iter().map(|v| v[k]).collect()));
        let phi_hist = History::new(phi.clone());
        PimpleSolver {
            geom,
            settings,
            u,
            p,
            phi,
            time: 0.0,
            history: Vec::new(),
            hist_u,
            phi_hist,
            step_no: 0,
            nu_face: None,
            body_force: None,
            source_force: None,
            sink: None,
            face_jump: None,
            ale: None,
            fs_q: Vec::new(),
            diverged: None,
            lag_estimates: Vec::new(),
            lag_warnings: Vec::new(),
        }
    }

    /// Record a trip of the divergence guard and build the record returned for the step.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn trip(
        &mut self,
        dt: f64,
        outer: Option<usize>,
        stage: &'static str,
        quantity: &'static str,
        cell: Option<usize>,
        value: f64,
        (res_u, res_p, p_iters): (f64, f64, usize),
    ) -> StepInfo {
        let limit = self.settings.divergence_courant_limit;
        let info = StepInfo {
            step: self.step_no,
            time: self.time,
            dt,
            courant: if quantity == "courant" { value } else { f64::NAN },
            continuity: f64::NAN,
            res_u,
            res_p,
            p_iters,
        };
        self.diverged = Some(Divergence {
            step: self.step_no,
            outer,
            stage,
            quantity,
            cell,
            value,
            limit,
            info,
        });
        info
    }

    /// Check after the pressure/velocity update of a step and before the history advances (ADR 0067): finite flux and
    /// velocity and a Courant number within the limit. Returns the tripping record, if any.
    pub(crate) fn guard_end(&mut self, dt: f64, courant: f64, res: (f64, f64, usize)) -> Option<StepInfo> {
        if let Some((f, v)) = first_nonfinite(&self.phi) {
            let cell = self.geom.mesh.owner().get(f).copied();
            return Some(self.trip(dt, None, "end of step", "flux", cell, v, res));
        }
        if let Some((c, v)) = self.bad_velocity() {
            return Some(self.trip(dt, None, "end of step", "velocity", Some(c), v, res));
        }
        let limit = self.settings.divergence_courant_limit;
        if !courant.is_finite() || courant > limit {
            return Some(self.trip(dt, None, "end of step", "courant", None, courant, res));
        }
        None
    }

    /// First cell with a non-finite velocity component.
    fn bad_velocity(&self) -> Option<(usize, f64)> {
        let c = self.u.values.iter().position(|v| v.iter().any(|x| !x.is_finite()))?;
        let v = self.u.values[c].iter().copied().find(|x| !x.is_finite())?;
        Some((c, v))
    }

    /// Advance one time step of size `dt`.
    pub fn step(&mut self, dt: f64) -> StepInfo {
        self.step_with(dt, None)
    }

    /// Advance one time step of size `dt`; `hook` (if any) is called at the start of every outer iteration with the current
    /// velocity and flux and supplies the solution-dependent momentum sources of that iteration (ADR 0045). The hook is not
    /// used by the non-iterative fractional-step advance.
    pub fn step_with(&mut self, dt: f64, mut hook: Option<&mut OuterHook>) -> StepInfo {
        if let Some(d) = &self.diverged {
            // the guard has stopped this run (ADR 0067): do no work
            return d.info;
        }
        if self.settings.fractional.is_some() && self.ale.is_none() {
            return self.step_fractional(dt);
        }
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let vol = mesh.cell_volumes().to_vec();
        let sf = mesh.face_area_vectors();
        let s = self.settings.clone();
        self.step_no += 1;

        let gamma_nu = match &self.nu_face {
            Some(v) => v.clone(),
            None => uniform_gamma(&g, s.nu),
        };
        let n_active = g.active.iter().filter(|&&a| a).count() as f64;
        let mut res_u: f64 = 0.0;
        let mut res_p = 0.0;
        let mut p_iters = 0;

        // transient Rhie-Chow data: (c1, c2) weights of the old levels, sized for the scheme
        let (c1, c2) = ddt_history_weights(&s.time_scheme, &self.hist_u[0], dt);
        // old-level flux defects `phi_old - (U_old)_f . S`
        let ddt_defect: Vec<f64> = if let Some(a) = &self.ale {
            a.defect.clone()
        } else if s.ddt_correction {
            let zero_b = vec![[0.0; 3]; mesh.n_boundary_faces()];
            let uold: Vec<Vec3> = (0..n).map(|c| [0, 1, 2].map(|k| self.hist_u[k].old[c])).collect();
            let fo = face_flux(&g, &uold, &zero_b);
            let mut d: Vec<f64> = (0..n_int).map(|f| c1 * (self.phi_hist.old[f] - fo[f])).collect();
            if c2 != 0.0 {
                let older: Vec<Vec3> = (0..n)
                    .map(|c| [0, 1, 2].map(|k| self.hist_u[k].older.as_ref().map_or(self.hist_u[k].old[c], |o| o[c])))
                    .collect();
                let fol = face_flux(&g, &older, &zero_b);
                let phi_older = self.phi_hist.older.as_ref().unwrap_or(&self.phi_hist.old);
                for f in 0..n_int {
                    d[f] -= c2 * (phi_older[f] - fol[f]);
                }
            }
            d
        } else {
            vec![0.0; n_int]
        };

        let lag_due = self.ale.is_none()
            && s.pressure_lag_check
                .is_some_and(|c| self.step_no == 1 || (c.every > 0 && self.step_no % c.every == 0));
        let mut lag_ctx: Option<LagCtx> = None;
        let rc_consistent = s.rhie_chow_consistent && self.ale.is_none();
        let seam_defect: Vec<(usize, usize, usize, f64, f64)> = if rc_consistent && s.ddt_correction {
            self.seam_defects(c1, c2)
        } else {
            Vec::new()
        };
        for outer in 0..s.n_outer {
            let last = outer + 1 == s.n_outer;
            let (alpha_u, alpha_p) = if last { (1.0, 1.0) } else { (s.alpha_u, s.alpha_p) };
            if let Some(h) = hook.as_mut() {
                let up = h(&self.u, &self.phi);
                self.source_force = up.source_force;
                self.sink = up.sink;
                self.face_jump = up.face_jump;
                for (patch, pk) in up.patch_pressures {
                    let _ = self.p.set_bc(&g, &patch, Box::new(FixedValue::scalar(pk)));
                }
            }
            // effective body acceleration of this outer iteration: the static part plus the hook's source
            let body_cow: Option<std::borrow::Cow<BodyForce>> = match (&self.body_force, &self.source_force) {
                (None, None) => None,
                (Some(b), None) => Some(std::borrow::Cow::Borrowed(b)),
                (None, Some(x)) => Some(std::borrow::Cow::Borrowed(x)),
                (Some(b), Some(x)) => {
                    let mut t = b.clone();
                    t.add(x);
                    Some(std::borrow::Cow::Owned(t))
                }
            };
            let body: Option<&BodyForce> = body_cow.as_deref();
            let mut pbc = pressure_bc(&g, &self.u, &self.p);
            if let Some(bf) = body {
                // fixed-flux pressure condition: where the normal velocity is prescribed the pressure gradient balances
                // the body force (as in the steady solver)
                for i in 0..pbc.coeffs.len() {
                    if pbc.fixed_u[i] && pbc.coupled[i].is_none() {
                        let f = n_int + i;
                        let gradient = bf.face[f] / vec3::norm(sf[f]).max(1e-300);
                        pbc.coeffs[i] = crate::bc::FaceCoeffs {
                            a: 1.0,
                            b: g.dn[i] * gradient,
                            c: 0.0,
                            d: gradient,
                        };
                    }
                }
            }
            if let Some(a) = &self.ale {
                a.apply_wall_gradients(&mut pbc, &g.dn);
            }
            let ub = self.u.boundary_values(&g);
            let bvals_p = boundary_values_pbc(&g, &pbc, &self.p.values);
            let mut grad_p = green_gauss(&g, &self.p.values, &bvals_p, 0);
            if let Some(j) = &self.face_jump {
                // the jump is removed from the face interpolation of the cell gradient (as in the SIMPLE kernel)
                let fix = j.gradient_fix(&g, &j.totals(&self.phi));
                for (a, b) in grad_p.iter_mut().zip(&fix) {
                    *a = vec3::add(*a, *b);
                }
            }

            // ---- momentum predictor -----------------------------------------
            let transposed = s
                .full_viscous_stress
                .then(|| transposed_stress_source(&g, &self.u, &gamma_nu));
            let mut mats: Vec<Option<FvMatrix>> = vec![None, None, None];
            let mut links: Vec<PeriodicLinks> = vec![
                PeriodicLinks::default(),
                PeriodicLinks::default(),
                PeriodicLinks::default(),
            ];
            let mut a_p = vec![0.0; n];
            let mut diag_k: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
            let u_iter: Vec<Vec3> = self.u.values.clone();
            // second-order linearisation: on the first outer iteration of a BDF2 step the
            // convecting flux and the deferred-correction state are extrapolated in time
            let extrap = if outer == 0 && c2 != 0.0 { Some(c1 - 1.0) } else { None };
            let phi_conv: Vec<f64> = match (extrap, &self.phi_hist.older) {
                (Some(r), Some(older)) => (0..self.phi.len())
                    .map(|f| (1.0 + r) * self.phi_hist.old[f] - r * older[f])
                    .collect(),
                _ => self.phi.clone(),
            };
            let phi_conv = match &self.ale {
                Some(a) => a.relative_flux(&phi_conv),
                None => phi_conv,
            };
            // ADR 0076 F-S: the velocity whose boundary coefficients the coupled faces take on this outer iteration
            let u_ext: Option<VectorField> = match extrap {
                Some(r) if s.seam_extrapolation && self.ale.is_none() && g.coupled_face.iter().any(|&c| c) => {
                    let mut ue = self.u.clone();
                    for (c, v) in ue.values.iter_mut().enumerate() {
                        for (j, x) in v.iter_mut().enumerate() {
                            let h = &self.hist_u[j];
                            *x = (1.0 + r) * h.old[c] - r * h.older.as_ref().map_or(h.old[c], |o| o[c]);
                        }
                    }
                    Some(ue)
                }
                _ => None,
            };
            for k in 0..3 {
                if !g.active[k] {
                    continue;
                }
                let cells: Vec<f64> = u_iter.iter().map(|v| v[k]).collect();
                let mut bc = self.u.boundary_coeffs(&g, k);
                if let Some(ue) = &u_ext {
                    let be = ue.boundary_coeffs(&g, k);
                    for (i, c) in bc.iter_mut().enumerate() {
                        if g.coupled_face[i] {
                            c.b = be[i].b;
                            c.d = be[i].d;
                        }
                    }
                }
                let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
                let conv_cells: Vec<f64> = match (extrap, &self.hist_u[k].older) {
                    (Some(r), Some(older)) => (0..n)
                        .map(|c| (1.0 + r) * self.hist_u[k].old[c] - r * older[c])
                        .collect(),
                    _ => cells.clone(),
                };
                let grad = if s.convection.needs_gradient() || s.non_orth_correctors > 0 {
                    Some(least_squares(&g, &conv_cells, &bv))
                } else {
                    None
                };
                let mut m = convection(&g, &phi_conv, &conv_cells, &bc, s.convection, grad.as_deref());
                m.add(&laplacian(&g, &gamma_nu, &bc, grad.as_deref()));
                // periodic partner values implicit (ADR 0046): move the lagged partner term back into the matrix
                let lk = PeriodicLinks::build(&g, &self.u, &phi_conv, &gamma_nu, &bc);
                // the partner values the coefficients were built from (F-S: the extrapolated cells)
                let q_lag: Vec<f64> = match &u_ext {
                    Some(ue) => ue.values.iter().map(|v| v[k]).collect(),
                    None => u_iter.iter().map(|v| v[k]).collect(),
                };
                lk.fix_rhs(&mut m, &q_lag);
                if let Some(sink) = &self.sink {
                    for c in 0..n {
                        m.a.diag[c] += sink[c] * vol[c];
                    }
                }
                let mut m = add_ddt(s.time_scheme, m, &vol, dt, &self.hist_u[k]);
                if let Some(a) = &self.ale {
                    a.fix_ddt(&mut m, &vol, &self.hist_u[k].old, dt);
                }
                if let Some(ts) = &transposed {
                    for c in 0..n {
                        m.b[c] += vol[c] * ts[c][k];
                    }
                }
                if alpha_u < 1.0 {
                    // ADR 0076 F-R: on the first outer iteration of a BDF2 step anchor the relaxation at the extrapolated velocity
                    let anchor = if s.relax_extrapolated && extrap.is_some() && self.ale.is_none() {
                        &conv_cells
                    } else {
                        &cells
                    };
                    lk.relax(&mut m, alpha_u, anchor);
                }
                let mut sys = m.clone();
                for c in 0..n {
                    sys.b[c] -= vol[c] * grad_p[c][k];
                    if let Some(bf) = body {
                        sys.b[c] += vol[c] * bf.cell[c][k];
                    }
                }
                if outer == 0 {
                    res_u = res_u.max(lk.residual(&sys, &cells));
                }
                let mut x = cells;
                lk.solve(&sys, &mut x, &s.momentum_solver);
                for c in 0..n {
                    self.u.values[c][k] = x[c];
                    a_p[c] += m.a.diag[c];
                }
                diag_k[k] = m.a.diag.clone();
                mats[k] = Some(m);
                links[k] = lk;
            }
            if let Some((c, v)) = self.bad_velocity() {
                return self.trip(
                    dt,
                    Some(outer),
                    "momentum predictor",
                    "velocity",
                    Some(c),
                    v,
                    (res_u, res_p, p_iters),
                );
            }
            for a in a_p.iter_mut() {
                *a /= n_active;
            }
            if lag_due && last {
                lag_ctx = Some(LagCtx {
                    mats: mats.clone(),
                    links: links.clone(),
                    pbc: pbc.clone(),
                });
            }
            let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            let gamma_p = face_interpolate(&g, &r_au);

            // ---- pressure correctors ---------------------------------------
            for corr in 0..s.n_correctors {
                let mut hby_a = vec![[0.0; 3]; n];
                for k in 0..3 {
                    if let Some(m) = &mats[k] {
                        let cells: Vec<f64> = self.u.values.iter().map(|v| v[k]).collect();
                        let h = links[k].h_operator(m, &cells);
                        for c in 0..n {
                            hby_a[c][k] = if s.component_diagonal {
                                (h[c] + (a_p[c] - diag_k[k][c]) * self.u.values[c][k]) / a_p[c]
                            } else {
                                h[c] / a_p[c]
                            };
                        }
                    }
                }
                let hb = boundary_hby_a(&g, &pbc, &ub, &hby_a);
                let mut phi_hby_a = face_flux(&g, &hby_a, &hb);
                if s.ddt_correction {
                    for f in 0..n_int {
                        phi_hby_a[f] += gamma_p[f] / dt * ddt_defect[f];
                    }
                }
                if let Some(bf) = body {
                    // balanced-force face value of the body acceleration, like the pressure gradient
                    for f in 0..mesh.n_faces() {
                        if !g.empty[f] {
                            phi_hby_a[f] += gamma_p[f] * bf.face[f];
                        }
                    }
                }
                if let Some(a) = &self.ale {
                    a.add_wall_flux(&g, &pbc, &gamma_p, &mut phi_hby_a);
                }
                if rc_consistent {
                    // consistent momentum interpolation (ADR 0073 F-G): remove C(G p_lag - b) and carry the seam defect over
                    let bv_lag = boundary_values_pbc(&g, &pbc, &self.p.values);
                    let mut x = green_gauss(&g, &self.p.values, &bv_lag, 0);
                    if let Some(j) = &self.face_jump {
                        let fix = j.gradient_fix(&g, &j.totals(&self.phi));
                        for (a, b) in x.iter_mut().zip(&fix) {
                            *a = vec3::add(*a, *b);
                        }
                    }
                    if let Some(bf) = body {
                        for (a, b) in x.iter_mut().zip(&bf.cell) {
                            *a = vec3::sub(*a, *b);
                        }
                    }
                    for (f, c) in self.rc_commutator(&r_au, &x) {
                        phi_hby_a[f] -= c;
                    }
                    for &(f, o, q, w, d) in &seam_defect {
                        phi_hby_a[f] += (w * r_au[o] + (1.0 - w) * r_au[q]) / dt * d;
                    }
                }
                // pressure jumps: phi (1 + G R) = phiHbyA + G rise - G snGrad p with G = gamma lap, as in the SIMPLE kernel
                let mut gamma_j = gamma_p.clone();
                if let Some(j) = &self.face_jump {
                    for f in 0..n_int {
                        if g.empty[f] || (j.rise[f] == 0.0 && j.resist[f] == 0.0) {
                            continue;
                        }
                        let gu = gamma_p[f] * g.lap_coeff[f];
                        let den = 1.0 + gu * j.resist[f];
                        phi_hby_a[f] = (phi_hby_a[f] + gu * j.rise[f]) / den;
                        gamma_j[f] = gamma_p[f] / den;
                    }
                }
                if let Some((f, v)) = first_nonfinite(&phi_hby_a) {
                    let cell = mesh.owner().get(f).copied();
                    return self.trip(
                        dt,
                        Some(outer),
                        "pressure right-hand side",
                        "phiHbyA",
                        cell,
                        v,
                        (res_u, res_p, p_iters),
                    );
                }
                let controls = PressureControls {
                    p_ref: s.p_ref,
                    non_orth_correctors: s.non_orth_correctors,
                    solver: s.pressure_solver,
                    amg: s.amg_pressure,
                    linear: s.linear,
                };
                let p_old = self.p.values.clone();
                let mut p_new = p_old.clone();
                let (r0, it) = solve_pressure(&g, &gamma_j, &pbc, &phi_hby_a, &mut p_new, &controls);
                p_iters += it;
                if corr == 0 {
                    res_p = r0;
                }
                if let Some((c, v)) = first_nonfinite(&p_new) {
                    return self.trip(
                        dt,
                        Some(outer),
                        "pressure solve",
                        "pressure",
                        Some(c),
                        v,
                        (res_u, res_p, p_iters),
                    );
                }
                self.phi = corrected_flux(&g, &gamma_j, &pbc, &phi_hby_a, &p_new, &ub, s.non_orth_correctors > 0);
                let bv_new = boundary_values_pbc(&g, &pbc, &p_new);
                let mut grad_new = green_gauss(&g, &p_new, &bv_new, 0);
                if let Some(j) = &self.face_jump {
                    let fix = j.gradient_fix(&g, &j.totals(&self.phi));
                    for (a, b) in grad_new.iter_mut().zip(&fix) {
                        *a = vec3::add(*a, *b);
                    }
                }
                for c in 0..n {
                    for k in 0..3 {
                        if g.active[k] {
                            let bforce = body.map_or(0.0, |bf| r_au[c] * bf.cell[c][k]);
                            self.u.values[c][k] = hby_a[c][k] + bforce - r_au[c] * grad_new[c][k];
                        }
                    }
                }
                if let Some((c, v)) = self.bad_velocity() {
                    return self.trip(
                        dt,
                        Some(outer),
                        "velocity correction",
                        "velocity",
                        Some(c),
                        v,
                        (res_u, res_p, p_iters),
                    );
                }
                let last_corr = corr + 1 == s.n_correctors;
                let a = if last_corr { alpha_p } else { 1.0 };
                for c in 0..n {
                    self.p.values[c] = p_old[c] + a * (p_new[c] - p_old[c]);
                }
            }
        }

        // ---- finish the step ------------------------------------------------
        if let (Some(ctx), Some(chk)) = (&lag_ctx, s.pressure_lag_check) {
            let (rho, cell) = self.lag_amplification(ctx, chk.iterations.max(2));
            let w = LagWarning {
                step: self.step_no,
                rho,
                cell,
            };
            self.lag_estimates.push(w);
            if rho >= 1.0 {
                match chk.action {
                    LagAction::Warn => self.lag_warnings.push(w),
                    LagAction::Raise => {
                        return self.trip(
                            dt,
                            None,
                            "pressure-lag check",
                            "lag amplification",
                            Some(cell),
                            rho,
                            (res_u, res_p, p_iters),
                        );
                    }
                }
            }
        }
        let courant = max_courant(&g, &self.phi, dt);
        if let Some(info) = self.guard_end(dt, courant, (res_u, res_p, p_iters)) {
            return info;
        }
        for k in 0..3 {
            let new: Vec<f64> = self.u.values.iter().map(|v| v[k]).collect();
            self.hist_u[k].push(new, dt);
        }
        self.phi_hist.push(self.phi.clone(), dt);
        self.time += dt;
        let cont = divergence(&g, &self.phi).iter().fold(0.0f64, |a, &b| a.max(b.abs()));
        let area_scale = sf.iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
        let info = StepInfo {
            step: self.step_no,
            time: self.time,
            dt,
            courant,
            continuity: cont / area_scale.max(1e-300),
            res_u,
            res_p,
            p_iters,
        };
        self.history.push(info);
        info
    }

    /// Periodic coupled faces `(face, partner face, weight of the face's owner, rotation of the partner vectors)` (ADR 0073 F-G; rotationally
    /// periodic seams ADR 0076).
    fn seam_faces(&self) -> Vec<SeamFace> {
        let g = &self.geom;
        let mut out = Vec::new();
        for (pi, patch) in g.mesh.patches().iter().enumerate() {
            let Some(map) = &g.periodic_partner[pi] else {
                continue;
            };
            if !self.p.bc(pi).is_coupled() {
                continue;
            }
            for (local, f) in patch.faces().enumerate() {
                if g.empty[f] {
                    continue;
                }
                let gf = map[local];
                let (dp, dq) = (g.dn[f - g.n_internal], g.dn[gf - g.n_internal]);
                out.push((f, gf, dq / (dp + dq), g.periodic_rot[pi]));
            }
        }
        out
    }

    /// `C(x) = interp(rAU x).S - interp(rAU) interp(x).S` on internal and periodic faces (ADR 0073 F-G), as (face, value).
    fn rc_commutator(&self, r_au: &[f64], x: &[Vec3]) -> Vec<(usize, f64)> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
        let term = |f: usize, o: usize, q: usize, w: f64, xq: Vec3| -> f64 {
            let gam = w * r_au[o] + (1.0 - w) * r_au[q];
            let v = [0, 1, 2]
                .map(|k| w * r_au[o] * x[o][k] + (1.0 - w) * r_au[q] * xq[k] - gam * (w * x[o][k] + (1.0 - w) * xq[k]));
            vec3::dot(v, sf[f])
        };
        let mut out: Vec<(usize, f64)> = (0..g.n_internal)
            .filter(|&f| !g.empty[f])
            .map(|f| (f, term(f, own[f], nei[f], g.weights[f], x[nei[f]])))
            .collect();
        for (f, gf, w, rot) in self.seam_faces() {
            let q = own[gf];
            let xq = rot.as_ref().map_or(x[q], |r| crate::field::rotate(r, x[q]));
            out.push((f, term(f, own[f], q, w, xq)));
        }
        out
    }

    /// Old-level face defects of the periodic faces, `c1 (phi^n - u^n_f.S) - c2 (phi^{n-1} - u^{n-1}_f.S)` with the seam
    /// interpolation (partner velocity rotated on rotational seams), as (face, owner cell, partner cell, weight, defect) (ADR 0073 F-G).
    fn seam_defects(&self, c1: f64, c2: f64) -> Vec<(usize, usize, usize, f64, f64)> {
        let g = &self.geom;
        let (own, sf) = (g.mesh.owner(), g.mesh.face_area_vectors());
        let at = |older: bool, c: usize| -> Vec3 {
            [0, 1, 2].map(|k| {
                let h = &self.hist_u[k];
                if older {
                    h.older.as_ref().map_or(h.old[c], |o| o[c])
                } else {
                    h.old[c]
                }
            })
        };
        self.seam_faces()
            .into_iter()
            .map(|(f, gf, w, rot)| {
                let (o, q) = (own[f], own[gf]);
                let defect = |older: bool| -> f64 {
                    let phi = if older {
                        self.phi_hist.older.as_ref().unwrap_or(&self.phi_hist.old)[f]
                    } else {
                        self.phi_hist.old[f]
                    };
                    let a = at(older, o);
                    let b = rot
                        .as_ref()
                        .map_or(at(older, q), |r| crate::field::rotate(r, at(older, q)));
                    phi - vec3::dot([0, 1, 2].map(|k| w * a[k] + (1.0 - w) * b[k]), sf[f])
                };
                let mut d = c1 * defect(false);
                if c2 != 0.0 {
                    d -= c2 * defect(true);
                }
                (f, o, q, w, d)
            })
            .collect()
    }

    /// One application of the linearised pressure-lag map of a step (ADR 0073 B4): the pressure error `e` drives the momentum predictors
    /// and correctors of every outer iteration with the frozen matrices of `ctx`; returns the pressure error at the end of the step.
    fn lag_map(&self, ctx: &LagCtx, e: &[f64], pbc_h: &crate::pressure::PressureBc) -> Vec<f64> {
        let g = &self.geom;
        let s = &self.settings;
        let n = g.n_cells();
        let vol = g.mesh.cell_volumes();
        let n_active = g.active.iter().filter(|&&a| a).count() as f64;
        let zero_ub = vec![[0.0; 3]; g.mesh.n_boundary_faces()];
        let controls = PressureControls {
            p_ref: (s.p_ref.0, 0.0),
            non_orth_correctors: s.non_orth_correctors,
            solver: s.pressure_solver,
            amg: s.amg_pressure,
            linear: s.linear,
        };
        let mut p = e.to_vec();
        let mut u: Vec<Vec3> = vec![[0.0; 3]; n];
        for outer in 0..s.n_outer {
            let last = outer + 1 == s.n_outer;
            let (alpha_u, alpha_p) = if last { (1.0, 1.0) } else { (s.alpha_u, s.alpha_p) };
            let bv = boundary_values_pbc(g, pbc_h, &p);
            let grad = green_gauss(g, &p, &bv, 0);
            let mut used: Vec<Option<FvMatrix>> = vec![None, None, None];
            let mut a_p = vec![0.0; n];
            let mut diag_k: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
            for k in 0..3 {
                let Some(m0) = &ctx.mats[k] else {
                    continue;
                };
                let mut m = m0.clone();
                m.b.iter_mut().for_each(|b| *b = 0.0);
                let uk: Vec<f64> = u.iter().map(|v| v[k]).collect();
                if alpha_u < 1.0 {
                    ctx.links[k].relax(&mut m, alpha_u, &uk);
                }
                let mut sys = m.clone();
                for c in 0..n {
                    sys.b[c] -= vol[c] * grad[c][k];
                }
                let mut x = vec![0.0; n];
                ctx.links[k].solve(&sys, &mut x, &s.momentum_solver);
                for c in 0..n {
                    u[c][k] = x[c];
                    a_p[c] += m.a.diag[c];
                }
                diag_k[k] = m.a.diag.clone();
                used[k] = Some(m);
            }
            for a in a_p.iter_mut() {
                *a /= n_active;
            }
            let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            let gamma = face_interpolate(g, &r_au);
            for corr in 0..s.n_correctors {
                let mut hby_a = vec![[0.0; 3]; n];
                for k in 0..3 {
                    if let Some(m) = &used[k] {
                        let cells: Vec<f64> = u.iter().map(|v| v[k]).collect();
                        let h = ctx.links[k].h_operator(m, &cells);
                        for c in 0..n {
                            hby_a[c][k] = if s.component_diagonal {
                                (h[c] + (a_p[c] - diag_k[k][c]) * u[c][k]) / a_p[c]
                            } else {
                                h[c] / a_p[c]
                            };
                        }
                    }
                }
                let hb = boundary_hby_a(g, pbc_h, &zero_ub, &hby_a);
                let phi_h = face_flux(g, &hby_a, &hb);
                let p_old = p.clone();
                let mut p_new = p_old.clone();
                solve_pressure(g, &gamma, pbc_h, &phi_h, &mut p_new, &controls);
                let bvn = boundary_values_pbc(g, pbc_h, &p_new);
                let gn = green_gauss(g, &p_new, &bvn, 0);
                for c in 0..n {
                    for k in 0..3 {
                        if g.active[k] {
                            u[c][k] = hby_a[c][k] - r_au[c] * gn[c][k];
                        }
                    }
                }
                let a = if corr + 1 == s.n_correctors { alpha_p } else { 1.0 };
                for c in 0..n {
                    p[c] = p_old[c] + a * (p_new[c] - p_old[c]);
                }
            }
        }
        p
    }

    /// Power-iteration estimate of the spectral radius of the pressure-lag map (ADR 0073 B4) and the cell of the largest mode entry.
    fn lag_amplification(&self, ctx: &LagCtx, iterations: usize) -> (f64, usize) {
        let g = &self.geom;
        let n = g.n_cells();
        let vol = g.mesh.cell_volumes();
        let tv: f64 = vol.iter().sum();
        let mut pbc_h = ctx.pbc.clone();
        for (c, cp) in pbc_h.coeffs.iter_mut().zip(&ctx.pbc.coupled) {
            if cp.is_none() {
                c.b = 0.0;
                c.d = 0.0;
            }
        }
        let fixed_level = pbc_h.has_dirichlet();
        let norm = |e: &mut Vec<f64>| -> f64 {
            if !fixed_level {
                let m = e.iter().zip(vol).map(|(a, v)| a * v).sum::<f64>() / tv;
                e.iter_mut().for_each(|x| *x -= m);
            }
            e.iter().zip(vol).map(|(a, v)| a * a * v).sum::<f64>().sqrt()
        };
        // smooth start plus a small deterministic pseudo-random part
        let cc = g.mesh.cell_centres();
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for x in cc {
            for k in 0..3 {
                lo[k] = lo[k].min(x[k]);
                hi[k] = hi[k].max(x[k]);
            }
        }
        let mut seed: u64 = 0x2545F4914F6CDD1D;
        let mut e: Vec<f64> = cc
            .iter()
            .map(|x| {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                let r = (seed >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
                let mut v = 1.0;
                for k in 0..3 {
                    if g.active[k] && hi[k] > lo[k] {
                        v *= (std::f64::consts::PI * (x[k] - lo[k]) / (hi[k] - lo[k])).cos();
                    }
                }
                v + 0.1 * r
            })
            .collect();
        let n0 = norm(&mut e);
        if n0 <= 0.0 || !n0.is_finite() {
            return (0.0, 0);
        }
        e.iter_mut().for_each(|x| *x /= n0);
        let mut logs = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let mut next = self.lag_map(ctx, &e, &pbc_h);
            let nn = norm(&mut next);
            if !(nn.is_finite() && nn > 0.0) {
                logs.push(if nn == 0.0 { f64::NEG_INFINITY } else { f64::INFINITY });
                break;
            }
            logs.push(nn.ln());
            e = next.iter().map(|x| x / nn).collect();
        }
        let half = logs.len() / 2;
        let tail = &logs[half..];
        let rho = (tail.iter().sum::<f64>() / tail.len().max(1) as f64).exp();
        let cell = (0..n).max_by(|&a, &b| e[a].abs().total_cmp(&e[b].abs())).unwrap_or(0);
        (rho, cell)
    }

    /// Advance to `t_end` with a fixed step `dt` (the last step is shortened to land on `t_end`).
    pub fn run_fixed(&mut self, t_end: f64, dt: f64) -> Vec<StepInfo> {
        let mut out = Vec::new();
        while self.time < t_end - 1e-12 * dt.max(1e-300) {
            let d = dt.min(t_end - self.time);
            out.push(self.step(d));
        }
        out
    }

    /// Advance to `t_end` with a Courant-controlled step starting from `dt0`.
    pub fn run_adaptive(&mut self, t_end: f64, dt0: f64, ctl: &CflController) -> Vec<StepInfo> {
        let mut out = Vec::new();
        let mut dt = dt0;
        while self.time < t_end - 1e-12 {
            let d = dt.min(t_end - self.time);
            let info = self.step(d);
            dt = ctl.next_dt(&self.geom, &self.phi, d);
            out.push(info);
        }
        out
    }

    /// Volume-integrated kinetic energy per unit mass.
    pub fn kinetic_energy(&self) -> f64 {
        let vol = self.geom.mesh.cell_volumes();
        self.u
            .values
            .iter()
            .zip(vol)
            .map(|(u, v)| 0.5 * vec3::dot(*u, *u) * v)
            .sum()
    }
}

/// Implicit partner links of the translationally periodic faces in the momentum matrix (ADR 0046, D1). `convection` and
/// `laplacian` put the partner part of a coupled face on the right-hand side with the value of the previous iterate; here its
/// coefficient `c_f` (`-gamma |S| / span` for the Laplacian, plus `phi_f` for upwind convection when `phi_f < 0`) is moved back
/// to the matrix as an off-diagonal entry (row = owner cell, column = owner cell of the partner face).
#[derive(Debug, Clone, Default)]
struct PeriodicLinks {
    /// (row, column, coefficient)
    t: Vec<(usize, usize, f64)>,
}

impl PeriodicLinks {
    /// Links of the velocity component whose boundary coefficients are `bc`. Rotationally periodic patches and
    /// self-partnered cells keep the lagged treatment.
    fn build(geom: &FvGeometry, u: &VectorField, flux: &[f64], gamma: &[f64], bc: &[crate::bc::FaceCoeffs]) -> Self {
        let mesh = &geom.mesh;
        let n_int = geom.n_internal;
        let (own, sf) = (mesh.owner(), mesh.face_area_vectors());
        let mut t = Vec::new();
        for (pi, p) in mesh.patches().iter().enumerate() {
            let Some(map) = &geom.periodic_partner[pi] else {
                continue;
            };
            if geom.periodic_rot[pi].is_some() || !u.bc(pi).is_coupled() {
                continue;
            }
            for (local, f) in p.faces().enumerate() {
                if geom.empty[f] {
                    continue;
                }
                let (row, col) = (own[f], own[map[local]]);
                let co = &bc[f - n_int];
                if row == col || !(co.a > 0.0 && co.a < 1.0) {
                    continue;
                }
                let mut coef = gamma[f] * vec3::norm(sf[f]) * co.c;
                if flux[f] < 0.0 {
                    coef += flux[f];
                }
                if coef != 0.0 {
                    t.push((row, col, coef));
                }
            }
        }
        PeriodicLinks { t }
    }

    fn is_empty(&self) -> bool {
        self.t.is_empty()
    }

    /// Remove the lagged partner term from the right-hand side (`q` = the cell values the assembly used).
    fn fix_rhs(&self, m: &mut FvMatrix, q: &[f64]) {
        for &(r, c, a) in &self.t {
            m.b[r] += a * q[c];
        }
    }

    /// Implicit under-relaxation including the link magnitudes in the diagonal-dominance sum.
    fn relax(&self, m: &mut FvMatrix, alpha: f64, x_old: &[f64]) {
        if self.is_empty() {
            return m.relax(alpha, x_old);
        }
        assert!(alpha > 0.0 && alpha <= 1.0, "relaxation factor must be in (0,1]");
        let mut sums = m.a.off_diag_abs_sum();
        for &(r, _, a) in &self.t {
            sums[r] += a.abs();
        }
        for i in 0..m.a.diag.len() {
            let d = m.a.diag[i].max(sums[i]);
            let dr = d / alpha;
            m.b[i] += (dr - m.a.diag[i]) * x_old[i];
            m.a.diag[i] = dr;
        }
    }

    /// The matrix with the links as a CSR system.
    fn csr(&self, sys: &FvMatrix) -> rcfd_linalg::CsrMatrix {
        let csr = sys.a.to_csr();
        let mut tr = Vec::with_capacity(csr.val.len() + self.t.len());
        for r in 0..csr.n {
            for k in csr.row_ptr[r]..csr.row_ptr[r + 1] {
                tr.push((r, csr.col[k], csr.val[k]));
            }
        }
        tr.extend(self.t.iter().copied());
        rcfd_linalg::CsrMatrix::from_triplets(csr.n, tr)
    }

    fn residual(&self, sys: &FvMatrix, x: &[f64]) -> f64 {
        if self.is_empty() {
            return normalised_residual(sys, x);
        }
        crate::simple::normalised_residual_csr(&self.csr(sys), &sys.b, x)
    }

    fn solve(&self, sys: &FvMatrix, x: &mut [f64], ctl: &SolveControl) {
        if self.is_empty() {
            sys.solve(x, ctl);
            return;
        }
        let a = self.csr(sys);
        rcfd_linalg::bicgstab(&a, &sys.b, x, &rcfd_linalg::Ilu0::new(&a), ctl);
    }

    /// `H` numerator `b - sum_offdiag A_ij x_j` including the links.
    fn h_operator(&self, m: &FvMatrix, x: &[f64]) -> Vec<f64> {
        let mut h = m.a.h_operator(x, &m.b);
        for &(r, c, a) in &self.t {
            h[r] -= a * x[c];
        }
        h
    }
}
/// Weights `(c1, c2)` of the old levels in the time derivative
/// `(c0 u - c1 u^n + c2 u^{n-1}) / dt` for the active scheme and history.
fn ddt_history_weights(scheme: &TimeScheme, hist: &History, dt: f64) -> (f64, f64) {
    match (scheme, &hist.older, hist.dt_old) {
        (TimeScheme::Bdf2, Some(_), Some(dt_old)) => {
            let r = dt / dt_old;
            (1.0 + r, r * r / (1.0 + r))
        }
        _ => (1.0, 0.0),
    }
}

/// Restartable state of the transient solver: fields, time, step counter and the time-level
/// history that the multi-step schemes read (wave 19C, ADR 0031 D3).
#[derive(Debug, Clone)]
pub struct PimpleState {
    /// Cell velocity.
    pub u: Vec<Vec3>,
    /// Kinematic pressure.
    pub p: Vec<f64>,
    /// Face flux.
    pub phi: Vec<f64>,
    /// Time.
    pub time: f64,
    /// Number of steps taken.
    pub step_no: usize,
    /// History of the three velocity components.
    pub hist_u: [History; 3],
    /// History of the face flux.
    pub phi_hist: History,
    /// Record of the last step (restored as the only history entry).
    pub last: Option<StepInfo>,
}

impl PimpleSolver {
    /// Capture the restartable state.
    pub fn snapshot(&self) -> PimpleState {
        PimpleState {
            u: self.u.values.clone(),
            p: self.p.values.clone(),
            phi: self.phi.clone(),
            time: self.time,
            step_no: self.step_no,
            hist_u: self.hist_u.clone(),
            phi_hist: self.phi_hist.clone(),
            last: self.history.last().copied(),
        }
    }

    /// Restore a state captured with [`snapshot`](Self::snapshot) (sizes must match the mesh).
    pub fn restore(&mut self, st: &PimpleState) -> rcfd_core::Result<()> {
        let (nc, nf) = (self.geom.n_cells(), self.geom.mesh.n_faces());
        if st.u.len() != nc
            || st.p.len() != nc
            || st.phi.len() != nf
            || !st.hist_u.iter().all(|h| h.old.len() == nc)
            || st.phi_hist.old.len() != nf
        {
            return Err(rcfd_core::Error::InvalidValue {
                path: "checkpoint".into(),
                reason: "state size does not match the mesh".into(),
            });
        }
        self.u.values = st.u.clone();
        self.p.values = st.p.clone();
        self.phi = st.phi.clone();
        self.time = st.time;
        self.step_no = st.step_no;
        self.hist_u = st.hist_u.clone();
        self.phi_hist = st.phi_hist.clone();
        self.history.clear();
        self.history.extend(st.last);
        self.diverged = None;
        Ok(())
    }

    /// Start afresh from the given fields at time `time` with an empty history, as
    /// [`new`](Self::new) does; the face flux is rebuilt from the velocity when `phi` is `None`.
    pub fn reinitialise(
        &mut self,
        u: Vec<Vec3>,
        p: Vec<f64>,
        phi: Option<Vec<f64>>,
        time: f64,
    ) -> rcfd_core::Result<()> {
        let (nc, nf) = (self.geom.n_cells(), self.geom.mesh.n_faces());
        if u.len() != nc || p.len() != nc || phi.as_ref().is_some_and(|f| f.len() != nf) {
            return Err(rcfd_core::Error::InvalidValue {
                path: "initial fields".into(),
                reason: "field size does not match the mesh".into(),
            });
        }
        self.u.values = u;
        self.p.values = p;
        self.phi = match phi {
            Some(f) => f,
            None => {
                let bu = self.u.boundary_values(&self.geom);
                face_flux(&self.geom, &self.u.values, &bu)
            }
        };
        self.hist_u = [0, 1, 2].map(|k| History::new(self.u.values.iter().map(|v| v[k]).collect()));
        self.phi_hist = History::new(self.phi.clone());
        self.time = time;
        self.step_no = 0;
        self.history.clear();
        self.diverged = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcfd_mesh::gen::box_2d;
    use std::f64::consts::PI;

    /// Taylor-Green vortex on a doubly periodic square.
    pub(crate) fn taylor_green(
        n: usize,
        nu: f64,
        settings: impl FnOnce(PimpleSettings) -> PimpleSettings,
    ) -> PimpleSolver {
        let l = 2.0 * PI;
        let mesh = Arc::new(box_2d(n, n, l, l, true, true).unwrap());
        let geom = Arc::new(FvGeometry::new(mesh).unwrap());
        let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
        // initial pressure p = +(cos 2x + cos 2y)/4 for u = (sin x cos y, -cos x sin y)
        // (Taylor-Green; the earlier minus sign belongs to the u = (-cos x sin y, sin x cos y) form)
        let p = ScalarField::from_fn("p", &geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
        PimpleSolver::new(geom, u, p, settings(PimpleSettings::piso(nu)))
    }

    /// Relative amplitude error of the velocity field against the analytic decay.
    fn amplitude_error(s: &PimpleSolver, nu: f64) -> f64 {
        let decay = (-2.0 * nu * s.time).exp();
        let c = s.geom.mesh.cell_centres();
        let (mut num, mut den) = (0.0, 0.0);
        for (i, u) in s.u.values.iter().enumerate() {
            let ex = [
                c[i][0].sin() * c[i][1].cos() * decay,
                -c[i][0].cos() * c[i][1].sin() * decay,
            ];
            num += (u[0] - ex[0]).powi(2) + (u[1] - ex[1]).powi(2);
            den += ex[0] * ex[0] + ex[1] * ex[1];
        }
        (num / den).sqrt()
    }

    #[test]
    fn taylor_green_decay_below_one_percent() {
        let nu = 0.05;
        let mut s = taylor_green(64, nu, |mut st| {
            st.convection = ConvScheme::Central;
            st
        });
        let ke0 = 0.25 * s.geom.mesh.total_volume(); // mean of 0.5|u|^2 is 1/4
        s.run_fixed(2.0, 0.02);
        let ke_exact = ke0 * (-4.0 * nu * 2.0f64).exp();
        let ke_err = (s.kinetic_energy() - ke_exact).abs() / ke_exact;
        assert!(ke_err < 0.01, "kinetic energy error {ke_err}");
        let amp = amplitude_error(&s, nu);
        assert!(amp < 0.01, "field error {amp}");
        assert!(s.history.last().unwrap().continuity < 1e-6);
    }

    #[test]
    fn taylor_green_error_is_second_order_in_space() {
        let nu = 0.1;
        let err = |n| {
            let mut s = taylor_green(n, nu, |mut st| {
                st.convection = ConvScheme::Central;
                st
            });
            s.run_fixed(0.5, 0.01);
            amplitude_error(&s, nu)
        };
        let (e1, e2) = (err(16), err(32));
        assert!(e1 / e2 > 3.0, "ratio {}", e1 / e2);
    }

    #[test]
    fn pimple_outer_iterations_and_bdf2_reduce_the_temporal_error() {
        let nu = 0.1;
        let run = |scheme, dt, outer| {
            let mut s = taylor_green(32, nu, |mut st| {
                st.convection = ConvScheme::Central;
                st.time_scheme = scheme;
                st.n_outer = outer;
                st.alpha_u = 1.0;
                st.alpha_p = 1.0;
                st
            });
            s.run_fixed(1.0, dt);
            s
        };
        let reference = run(TimeScheme::Bdf2, 0.0125, 3);
        let diff = |a: &PimpleSolver| {
            a.u.values
                .iter()
                .zip(&reference.u.values)
                .map(|(x, y)| (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2))
                .sum::<f64>()
                .sqrt()
        };
        let piso = diff(&run(TimeScheme::ImplicitEuler, 0.1, 1));
        let pimple = diff(&run(TimeScheme::ImplicitEuler, 0.1, 3));
        let bdf2 = diff(&run(TimeScheme::Bdf2, 0.1, 3));
        assert!(pimple < 0.5 * piso, "piso {piso} pimple {pimple}");
        assert!(bdf2 < pimple, "euler {pimple} bdf2 {bdf2}");
    }

    #[test]
    fn pimple_allows_courant_above_one() {
        let nu = 0.1;
        let mut s = taylor_green(16, nu, |mut st| {
            st.convection = ConvScheme::Central;
            st.n_outer = 4;
            st.n_correctors = 2;
            st.alpha_u = 0.9;
            st.alpha_p = 0.9;
            st
        });
        // dx = 0.39, |u| <= 1 -> Co ~ dt / dx; dt = 0.5 gives Co ~ 1.3
        let infos = s.run_fixed(1.0, 0.5);
        assert!(infos.iter().any(|i| i.courant > 1.0), "{infos:?}");
        let err = amplitude_error(&s, nu);
        assert!(err.is_finite() && err < 0.15, "err {err}");
    }

    #[test]
    fn piso_mean_pressure_and_momentum_are_conserved() {
        let mut s = taylor_green(16, 0.05, |st| st);
        s.run_fixed(0.2, 0.02);
        let vol = s.geom.mesh.cell_volumes();
        let tot = vol.iter().sum::<f64>();
        let mom: f64 = s.u.values.iter().zip(vol).map(|(u, v)| u[0] * v).sum::<f64>() / tot;
        assert!(mom.abs() < 1e-8, "{mom}");
    }
}
