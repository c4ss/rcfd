//! Overset (chimera) PISO step, F-134 (ADR 0219): a component mesh moving rigidly over a static background mesh.
//!
//! The solver works on the composite mesh of `rcfd_mesh::overset` (background and component cells, no face between them).
//! Every step it moves the component (ALE, ADR 0033), finds the kinds and stencils of the cells
//! (`rcfd_mesh::overset::Overset::connectivity`) and takes one implicit-Euler PISO step whose calculated rows are those of
//! `PimpleSolver::step` (ADR 0219 M5 to M8): the rows of the fringe cells interpolate the other part, the rows of the hole
//! cells are fixed, and in the conservative mass mode one offset `delta` of the background fringe pressures makes the net
//! flux into the hole region zero (M7), so the mass of the domain is conserved. With every cell calculated the step is the
//! library's ALE step (check Q0).
//!
//! Outer iterations (F-335, ADR 0238) are an opt-in of the solver, [`OversetSolver::set_outer_iterations`]: the PISO pass
//! is then repeated within the step on the kinds, donors and weights of the new pose, each pass with the convecting flux
//! and the pressure of the pass before it, as `PimpleSolver::step` does for `n_outer > 1`.
//!
//! BDF2 (F-338, ADR 0241) is an opt-in of the solver too, [`OversetSolver::set_time_scheme`]: the three-level backward
//! difference of `V u` on the moving cells with the mesh flux `(c0 W^{n+1} - c2 W^n) / dt` of the swept volumes of the
//! last two steps (the geometric conservation law of the three-level scheme), the first step an implicit Euler step, and
//! one implicit Euler row for a cell whose old-old value is a hole value.
//!
//! A donor interpolation that reproduces quadratic fields (F-336, ADR 0251) is an opt-in of the coupling's
//! `rcfd_mesh::overset::Overset`, `set_donor_interpolation(DonorInterpolation::Quadratic)`, made before the solver is
//! built: the solver takes its stencils from there. [`OversetInfo::n_fallback`] counts the fringe cells that kept the
//! linear stencil, and a step whose interpolated `rAU` is not positive in a fringe cell is refused (the weights can be
//! negative and nothing is clipped).

use crate::ale::{move_mesh, set_follow_conditions, MeshStep};
use crate::convection::{convection, face_flux, ConvScheme};
use crate::ddt::{add_ddt, max_courant, History, TimeScheme};
use crate::diffusion::{laplacian, laplacian_opt, uniform_gamma};
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::green_gauss;
use crate::pimple::moving::AleStep;
use crate::pimple::PimpleSettings;
use crate::pressure::{
    boundary_hby_a, boundary_values_pbc, corrected_flux, divergence, face_interpolate, pressure_bc, solve_pressure,
    PressureBc, PressureControls,
};
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::{bicgstab, gmres, Amg, AmgSettings, CsrMatrix, Ilu0, LinearDevice, SolveControl, SolveStats};
use rcfd_mesh::motion::Pose;
use rcfd_mesh::overset::{pose_inverse_apply, CellKind, Connectivity, DonorInterpolation, Overset};
use std::sync::Arc;

/// How the pressure of the background fringe cells is coupled (ADR 0219 M7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MassMode {
    /// One offset `delta` of the background fringe pressures from their interpolants, chosen so that the net flux into the
    /// hole region is zero (open domain) or the reference cell keeps its value exactly (closed domain).
    Conservative,
    /// Plain interpolation (`delta = 0`); the overlap is not conservative.
    Interpolated,
}

/// The composite mesh's overset data and the current pose of the component.
#[derive(Debug, Clone)]
pub struct OversetCoupling {
    /// Composite mesh, specification and locators.
    pub overset: Overset,
    /// Pose of the component at the solver's time.
    pub pose: Pose,
}

/// One prepared step (from [`OversetSolver::prepare`]); every field is public so that a check can alter it.
#[derive(Debug, Clone)]
pub struct OversetStep {
    /// Geometry of the moved composite mesh.
    pub geom: Arc<FvGeometry>,
    /// Mesh flux `W_f / dt` of every face.
    pub mesh_flux: Vec<f64>,
    /// Fixed-flux pressure gradient of the moving walls per boundary face (`NaN` elsewhere; empty = none).
    pub wall_dpdn: Vec<f64>,
    /// Kinds and stencils at the new pose.
    pub conn: Connectivity,
    /// The new pose (`None` for a plain moving mesh).
    pub pose: Option<Pose>,
    /// Rigid velocity of the component over the step at every cell centre (used for the hole cells; empty without
    /// coupling).
    pub hole_velocity: Vec<Vec3>,
}

/// Diagnostics of one overset step.
#[derive(Debug, Clone, Copy)]
pub struct OversetInfo {
    /// Step number (1-based).
    pub step: usize,
    /// Time after the step.
    pub time: f64,
    /// Step size.
    pub dt: f64,
    /// Largest Courant number.
    pub courant: f64,
    /// Largest net outflow of a calculated cell over the largest face area (the library's continuity measure).
    pub continuity: f64,
    /// Number of hole cells.
    pub n_hole: usize,
    /// Number of background fringe cells.
    pub n_fringe_background: usize,
    /// Number of component fringe cells.
    pub n_fringe_component: usize,
    /// Number of cells whose kind changed in this step.
    pub n_changed: usize,
    /// Number of fringe cells that kept the linear stencil although the quadratic donor interpolation was asked for
    /// (ADR 0251 D4); zero without that opt-in.
    pub n_fallback: usize,
    /// Offset of the background fringe pressures (zero in the interpolated mode).
    pub delta: f64,
    /// Net flux out of the calculated background cells into the background fringe cells after the step.
    pub hole_flux: f64,
    /// Linear-solver iterations spent on pressure.
    pub p_iters: usize,
    /// Linear solves of the step that did not converge (solves of the library path are not counted).
    pub unconverged: usize,
}

/// Overset PISO solver state (ADR 0219 S2).
pub struct OversetSolver {
    /// Geometry of the composite mesh at the solver's time.
    pub geom: Arc<FvGeometry>,
    /// Settings (the refused ones are listed at [`OversetSolver::new`]).
    pub settings: PimpleSettings,
    /// Pressure coupling of the background fringe.
    pub mass: MassMode,
    /// Velocity.
    pub u: VectorField,
    /// Kinematic pressure.
    pub p: ScalarField,
    /// Face flux (positive out of the owner).
    pub phi: Vec<f64>,
    /// Current time.
    pub time: f64,
    /// Diagnostics of every step.
    pub history: Vec<OversetInfo>,
    /// Overset coupling (`None`: a plain mesh, every cell calculated).
    pub coupling: Option<OversetCoupling>,
    /// Kinds and stencils at the solver's time.
    pub conn: Connectivity,
    /// The last offset of the background fringe pressures.
    pub delta: f64,
    step_no: usize,
    /// Set by [`OversetSolver::set_outer_iterations`]: `settings.n_outer` above 1 is accepted (ADR 0238 D1).
    outer_opt_in: bool,
    /// Set by [`OversetSolver::set_time_scheme`]: `settings.time_scheme = Bdf2` is accepted (ADR 0241 D1).
    bdf2_opt_in: bool,
    /// The old-old level, kept while BDF2 is on (`None` before the first step with it).
    bdf2: Option<Bdf2History>,
    /// Calculated cells of the last step that took the implicit Euler row for their hole history (ADR 0241 D3).
    euler_fallback: usize,
}

/// What a BDF2 step needs of the time before the old one (ADR 0241 D3). The values follow the cells and faces.
#[derive(Debug, Clone)]
struct Bdf2History {
    /// Cell velocity at the old-old time (a solved value, a fringe interpolant or, in a hole cell, the rigid velocity).
    u: Vec<Vec3>,
    /// Cell volumes at the old-old time.
    vol: Vec<f64>,
    /// Whether the cell was a hole at the old-old time.
    hole: Vec<bool>,
    /// Face flux at the old-old time.
    phi: Vec<f64>,
    /// Whether that flux was a flux of the fluid: an internal face that kept its flux, a boundary face of a calculated cell.
    flux_ok: Vec<bool>,
    /// Rhie-Chow defect `phi - (u)_f . S` of the internal faces at the old-old time, zero where the flux was not kept.
    defect: Vec<f64>,
    /// Mesh flux `W_f / dt` of the last step.
    mesh_flux: Vec<f64>,
    /// Size of the last step.
    dt: f64,
}

/// The old level as the step after this one needs it (the part of [`Bdf2History`] that is known before the step).
struct OldLevel {
    hole: Vec<bool>,
    phi: Vec<f64>,
    flux_ok: Vec<bool>,
    defect: Vec<f64>,
}

/// The three-level data of one BDF2 step (ADR 0241 D2 to D4).
struct Bdf2Step {
    /// Weights of `(c0 V u - c1 V^n u^n + c2 V^{n-1} u^{n-1}) / dt` (those of `ddt::add_ddt`).
    c0: f64,
    c1: f64,
    c2: f64,
    /// Whether the cell takes the BDF2 row (it was not a hole at either old time); otherwise the implicit Euler row.
    full: Vec<bool>,
    /// Velocity and volume at the old-old time.
    u_older: Vec<Vec3>,
    vol_older: Vec<f64>,
    /// Convecting flux (before the mesh flux is taken off) and cell values of the first outer iteration: extrapolated in
    /// time where both old values are fluid values, the current ones elsewhere.
    phi_first: Vec<f64>,
    u_first: Vec<Vec3>,
}

impl Bdf2Step {
    /// The transient term of component `k`: for a BDF2 cell the expressions of `ddt::add_ddt` with the new volume and the
    /// correction that puts the old and the old-old volume back (zero on a mesh at rest); for the other cells the
    /// implicit Euler row with the old volume.
    fn add_ddt(&self, mut m: FvMatrix, vol: &[f64], vol_old: &[f64], dt: f64, u_old: &[f64], k: usize) -> FvMatrix {
        for i in 0..vol.len() {
            if self.full[i] {
                let older = self.u_older[i][k];
                m.a.diag[i] += vol[i] * self.c0 / dt;
                m.b[i] += vol[i] / dt * (self.c1 * u_old[i] - self.c2 * older);
                let old_part = (vol_old[i] - vol[i]) * (self.c1 * u_old[i]);
                let older_part = (self.vol_older[i] - vol[i]) * (self.c2 * older);
                m.b[i] += (old_part - older_part) / dt;
            } else {
                m.a.diag[i] += vol[i] / dt;
                m.b[i] += vol[i] / dt * u_old[i];
                m.b[i] += (vol_old[i] - vol[i]) / dt * u_old[i];
            }
        }
        m
    }
}

/// Whether `a` is an under-relaxation factor the outer iterations accept.
fn factor_ok(a: f64) -> bool {
    a > 0.0 && a <= 1.0
}

fn refusal(s: &PimpleSettings, geom: &FvGeometry, outer_opt_in: bool, bdf2_opt_in: bool) -> Option<&'static str> {
    match s.time_scheme {
        TimeScheme::ImplicitEuler => {}
        TimeScheme::Bdf2 if bdf2_opt_in => {}
        _ if bdf2_opt_in => return Some("overset: the implicit Euler or the BDF2 time scheme only"),
        _ => return Some("overset: the implicit Euler time scheme only"),
    }
    if !outer_opt_in {
        if s.n_outer != 1 {
            return Some("overset: one outer iteration (PISO) only");
        }
    } else if s.n_outer == 0 {
        return Some("overset: at least one outer iteration");
    } else if !factor_ok(s.alpha_u) || !factor_ok(s.alpha_p) {
        return Some("overset: the under-relaxation factors of the outer iterations must lie in (0, 1]");
    }
    if s.n_correctors == 0 {
        return Some("overset: at least one pressure corrector");
    }
    if s.non_orth_correctors != 0 {
        return Some("overset: no non-orthogonal correctors");
    }
    if !matches!(
        s.convection,
        ConvScheme::Upwind | ConvScheme::Central | ConvScheme::Blended(_)
    ) {
        return Some("overset: upwind, central or blended convection only");
    }
    if s.component_diagonal
        || s.amg_pressure
        || s.linear.device != LinearDevice::Cpu
        || s.fractional.is_some()
        || s.full_viscous_stress
        || s.pressure_lag_check.is_some()
        || s.corrector_control.is_some()
        || s.linear_upwind_limiter.is_some()
        || s.gradient_momentum.is_some()
        || s.gradient_pressure.is_some()
        || s.consistent_correctors
        || s.coupled_body_pair_mean
    {
        return Some("overset: a setting outside the overset path (ADR 0219 S2)");
    }
    if geom.periodic_partner.iter().any(|p| p.is_some()) {
        return Some("overset: periodic patches are not supported");
    }
    None
}

fn first_nonfinite(v: &[f64]) -> Option<(usize, f64)> {
    v.iter().copied().enumerate().find(|(_, x)| !x.is_finite())
}

/// The system `sys` with the rows of the fringe cells replaced by `s_i (x_i - sum_k w_ik x_k) = 0` (minus `s_i delta` in
/// the column `extra` when given), the rows of the hole cells by `s_i x_i = s_i hole_value_i`, as compressed rows and a
/// right-hand side (ADR 0219 M5, M7).
fn overset_system(sys: &FvMatrix, conn: &Connectivity, scale: &[f64], hole_value: &[f64]) -> (CsrMatrix, Vec<f64>) {
    let n = sys.b.len();
    let a = &sys.a;
    let calc = |i: usize| conn.kind[i] == CellKind::Calculated;
    let mut t: Vec<(usize, usize, f64)> = Vec::with_capacity(n + 2 * a.upper.len());
    for i in 0..n {
        match conn.kind[i] {
            CellKind::Calculated => t.push((i, i, a.diag[i])),
            CellKind::Hole => t.push((i, i, scale[i])),
            CellKind::Fringe => {}
        }
    }
    for f in 0..a.upper.len() {
        let (l, u) = (a.addr.lower[f], a.addr.upper[f]);
        if calc(l) {
            t.push((l, u, a.upper[f]));
        }
        if calc(u) {
            t.push((u, l, a.lower[f]));
        }
    }
    for (c, st) in &conn.stencils {
        t.push((*c, *c, scale[*c]));
        for (&k, &w) in st.cells.iter().zip(&st.weights) {
            t.push((*c, k, -scale[*c] * w));
        }
    }
    (
        CsrMatrix::from_triplets(n, t),
        overset_rhs(sys, conn, scale, hole_value),
    )
}

/// The right-hand side of [`overset_system`].
fn overset_rhs(sys: &FvMatrix, conn: &Connectivity, scale: &[f64], hole_value: &[f64]) -> Vec<f64> {
    (0..sys.b.len())
        .map(|i| match conn.kind[i] {
            CellKind::Calculated => sys.b[i],
            CellKind::Hole => scale[i] * hole_value[i],
            CellKind::Fringe => 0.0,
        })
        .collect()
}

/// The pressure matrix of a step with its overset rows, its AMG hierarchy and the response `p1` to a unit offset of the
/// background fringe pressures (ADR 0219 M7); the matrix does not change between the correctors of a step.
struct PressureSystem {
    a: CsrMatrix,
    amg: Amg,
    p1: Option<Vec<f64>>,
}

/// A pressure system with overset rows: restarted GMRES(30) right-preconditioned with one AMG cycle (ADR 0219
/// amendment 1).
fn solve_pressure_rows(sys: &PressureSystem, b: &[f64], x: &mut [f64], ctl: &SolveControl) -> SolveStats {
    gmres(&sys.a, b, x, &sys.amg, 30, ctl)
}

fn solve_csr(a: &CsrMatrix, b: &[f64], x: &mut [f64], ctl: &SolveControl) -> SolveStats {
    bicgstab(a, b, x, &Ilu0::new(a), ctl)
}

impl OversetSolver {
    /// Create a solver at `t = 0` on `geom` (with a coupling: the composite mesh at `coupling.pose`). With a coupling the
    /// fringe cells get their interpolated velocity and pressure; the initial flux is then interpolated from `u`, as
    /// `PimpleSolver::new`. Refused (`Err`): a time scheme other than implicit Euler (BDF2 is switched on afterwards, by
    /// [`set_time_scheme`](Self::set_time_scheme)), `n_outer != 1` (outer iterations are
    /// switched on afterwards, by [`set_outer_iterations`](Self::set_outer_iterations)), no corrector,
    /// non-orthogonal correctors, a convection scheme other than upwind, central or blended, the component diagonal, AMG
    /// pressure, a linear device other than the CPU, the fractional step, the full viscous stress, the lag check, the
    /// adaptive correctors, the limiter, the gradient options, the consistent correctors, the pair mean, periodic patches,
    /// and a geometry that is not the coupling's composite.
    pub fn new(
        geom: Arc<FvGeometry>,
        mut u: VectorField,
        mut p: ScalarField,
        settings: PimpleSettings,
        mass: MassMode,
        coupling: Option<OversetCoupling>,
    ) -> Result<Self, String> {
        if let Some(r) = refusal(&settings, &geom, false, false) {
            return Err(r.into());
        }
        let n = geom.n_cells();
        let conn = match &coupling {
            Some(cp) => {
                if cp.overset.composite.mesh.n_cells() != n {
                    return Err("overset: the geometry is not the coupling's composite mesh".into());
                }
                let conn = cp
                    .overset
                    .connectivity(&geom.mesh, &cp.pose)
                    .map_err(|e| e.to_string())?;
                for (c, st) in &conn.stencils {
                    let mut v = [0.0; 3];
                    let mut q = 0.0;
                    for (&k, &w) in st.cells.iter().zip(&st.weights) {
                        for (d, vd) in v.iter_mut().enumerate() {
                            *vd += w * u.values[k][d];
                        }
                        q += w * p.values[k];
                    }
                    for d in 0..3 {
                        if geom.active[d] {
                            u.values[*c][d] = v[d];
                        }
                    }
                    p.values[*c] = q;
                }
                conn
            }
            None => Connectivity::all_calculated(n),
        };
        let bu = u.boundary_values(&geom);
        let phi = face_flux(&geom, &u.values, &bu);
        Ok(OversetSolver {
            geom,
            settings,
            mass,
            u,
            p,
            phi,
            time: 0.0,
            history: Vec::new(),
            coupling,
            conn,
            delta: 0.0,
            step_no: 0,
            outer_opt_in: false,
            bdf2_opt_in: false,
            bdf2: None,
            euler_fallback: 0,
        })
    }

    /// Choose the time scheme (F-338, ADR 0241 D1). `TimeScheme::Bdf2` opts in to the three-level scheme: the next step
    /// is still an implicit Euler step (there is no old-old level yet), every later one a BDF2 step with the weights of
    /// the two step sizes; the mesh flux is `(c0 W^{n+1} - c2 W^n) / dt` of the swept volumes of the last two steps, and
    /// a cell that was a hole two steps back takes the implicit Euler row once. `TimeScheme::ImplicitEuler` returns to the
    /// solver that did not opt in (the history is dropped). Sets `settings.time_scheme`. Refused, leaving the solver
    /// unchanged: every other scheme.
    pub fn set_time_scheme(&mut self, scheme: TimeScheme) -> Result<(), String> {
        match scheme {
            TimeScheme::Bdf2 => self.bdf2_opt_in = true,
            TimeScheme::ImplicitEuler => {
                self.bdf2_opt_in = false;
                self.bdf2 = None;
                self.euler_fallback = 0;
            }
            _ => return Err("overset: the implicit Euler or the BDF2 time scheme only".into()),
        }
        self.settings.time_scheme = scheme;
        Ok(())
    }

    /// The number of calculated cells that took the implicit Euler row in the last step because they were a hole two
    /// steps back (ADR 0241 D3); zero without BDF2 and on its first step.
    pub fn euler_fallback_cells(&self) -> usize {
        self.euler_fallback
    }

    /// Opt in to outer iterations (F-335, ADR 0238 D1): from the next step on every step takes `n_outer` passes of the
    /// PISO step on the new pose, each with the convecting flux and the pressure of the pass before it, as
    /// `PimpleSolver::step` does. `alpha_u` and `alpha_p` are the under-relaxation factors of the momentum matrices and of
    /// the pressure on every pass but the last (1 = none). Sets `settings.n_outer`, `settings.alpha_u` and
    /// `settings.alpha_p`. With `n_outer = 1` the step is the PISO step of a solver that did not opt in, bit for bit.
    /// Refused, leaving the solver unchanged: `n_outer = 0` and a factor outside `(0, 1]`.
    pub fn set_outer_iterations(&mut self, n_outer: usize, alpha_u: f64, alpha_p: f64) -> Result<(), String> {
        let mut s = self.settings.clone();
        s.n_outer = n_outer;
        s.alpha_u = alpha_u;
        s.alpha_p = alpha_p;
        if let Some(r) = refusal(&s, &self.geom, true, self.bdf2_opt_in) {
            return Err(r.into());
        }
        self.settings = s;
        self.outer_opt_in = true;
        Ok(())
    }

    /// Move the component to `pose` over a step of size `dt` (ADR 0219 M10): the moved composite mesh and its mesh flux
    /// (`ale::move_mesh`), the follow conditions of the moving walls `walls` (set on `u`, as a moving-mesh loop does), and
    /// the kinds and stencils at the new pose.
    pub fn prepare(&mut self, dt: f64, pose: Pose, walls: &[String]) -> Result<OversetStep, String> {
        let Some(cp) = &self.coupling else {
            return Err("overset: prepare needs an overset coupling".into());
        };
        let points = cp.overset.composite.points_at(&pose);
        let ms = move_mesh(&self.geom, points, dt)?;
        let conn = cp
            .overset
            .connectivity(&ms.geom.mesh, &pose)
            .map_err(|e| e.to_string())?;
        let old_pose = cp.pose;
        let hole_velocity: Vec<Vec3> = ms
            .geom
            .mesh
            .cell_centres()
            .iter()
            .map(|&x| vec3::scale(vec3::sub(x, old_pose.apply(pose_inverse_apply(&pose, x))), 1.0 / dt))
            .collect();
        let old = self.geom.mesh.clone();
        let wall_dpdn = if walls.is_empty() {
            Vec::new()
        } else {
            set_follow_conditions(&mut self.u, &ms, &old, walls, dt)?
        };
        Ok(OversetStep {
            geom: ms.geom,
            mesh_flux: ms.mesh_flux,
            wall_dpdn,
            conn,
            pose: Some(pose),
            hole_velocity,
        })
    }

    /// [`prepare`](Self::prepare) and [`advance`](Self::advance).
    pub fn step(&mut self, dt: f64, pose: Pose, walls: &[String]) -> Result<OversetInfo, String> {
        let st = self.prepare(dt, pose, walls)?;
        self.advance(dt, st)
    }

    /// One step on a plain moving mesh (no coupling, every cell calculated): the library's
    /// `PimpleSolver::step_ale_with_walls` (ADR 0219 Q0).
    pub fn advance_mesh(&mut self, dt: f64, ms: MeshStep, wall_dpdn: Vec<f64>) -> Result<OversetInfo, String> {
        let n = ms.geom.n_cells();
        self.advance(
            dt,
            OversetStep {
                geom: ms.geom,
                mesh_flux: ms.mesh_flux,
                wall_dpdn,
                conn: Connectivity::all_calculated(n),
                pose: None,
                hole_velocity: Vec::new(),
            },
        )
    }

    /// Advance one PISO step of size `dt` (implicit Euler; BDF2 after [`set_time_scheme`](Self::set_time_scheme)) onto
    /// the prepared step `st` (ADR 0219 M5 to M9). Fails, leaving
    /// `u`, `p`, `phi` and the time unchanged, for a step that does not fit the solver (cell or face counts, sizes), a
    /// refused setting (`settings.n_outer` above 1 without the opt-in among them), a cell that was a hole and is now calculated ("uncovered"), and a closed domain whose reference cell
    /// is not calculated; fails during the step for a non-finite state or a Courant number above the divergence limit.
    pub fn advance(&mut self, dt: f64, st: OversetStep) -> Result<OversetInfo, String> {
        if let Some(r) = refusal(&self.settings, &st.geom, self.outer_opt_in, self.bdf2_opt_in) {
            return Err(r.into());
        }
        let old = self.geom.clone();
        let n = old.n_cells();
        if st.geom.n_cells() != n
            || st.geom.mesh.n_faces() != old.mesh.n_faces()
            || st.mesh_flux.len() != old.mesh.n_faces()
            || st.conn.kind.len() != n
            || (!st.wall_dpdn.is_empty() && st.wall_dpdn.len() != old.mesh.n_boundary_faces())
        {
            return Err("overset: the step does not fit the solver's mesh".into());
        }
        let trivial = st.conn.is_trivial();
        if !trivial && (self.coupling.is_none() || st.hole_velocity.len() != n) {
            return Err("overset: fringe or hole cells without an overset coupling".into());
        }
        if let Some(c) =
            (0..n).find(|&c| self.conn.kind[c] == CellKind::Hole && st.conn.kind[c] == CellKind::Calculated)
        {
            return Err(format!(
                "overset: cell {c} was a hole and is uncovered as a calculated cell in one step (take a smaller step)"
            ));
        }
        let s = self.settings.clone();
        let n_int = old.n_internal;
        let nb = st.conn.n_background_cells;
        let OversetStep {
            geom: new_geom,
            mut mesh_flux,
            wall_dpdn,
            conn,
            pose,
            hole_velocity,
        } = st;
        // the pressure level: a closed domain uses the reference cell, which must be calculated (M7)
        {
            let u_tmp = &self.u;
            let pbc0 = pressure_bc(&new_geom, u_tmp, &self.p);
            let own = new_geom.mesh.owner();
            let fixes = (0..pbc0.coeffs.len())
                .any(|i| pbc0.coeffs[i].c != 0.0 && conn.kind[own[n_int + i]] == CellKind::Calculated);
            if !trivial && !fixes && conn.kind[s.p_ref.0] != CellKind::Calculated {
                return Err(format!(
                    "overset: the reference cell {} of the closed domain is not a calculated cell",
                    s.p_ref.0
                ));
            }
        }
        // ---- the moving-mesh data, as step_ale_with_walls (M9 for the faces that keep their flux) ----------------
        for (f, m) in mesh_flux.iter_mut().enumerate() {
            if old.empty[f] {
                *m = 0.0;
            }
        }
        let vol_old = old.mesh.cell_volumes().to_vec();
        let conn_old = std::mem::replace(&mut self.conn, conn);
        let conn = self.conn.clone();
        let kept: Vec<bool> = {
            let (own, nei) = (old.mesh.owner(), old.mesh.neighbour());
            (0..n_int)
                .map(|f| {
                    let (a, b) = (conn_old.kind[own[f]], conn_old.kind[nei[f]]);
                    (a == CellKind::Calculated || b == CellKind::Calculated)
                        && a != CellKind::Hole
                        && b != CellKind::Hole
                })
                .collect()
        };
        let defect: Vec<f64> = if s.ddt_correction {
            let vol_new = new_geom.mesh.cell_volumes();
            let zero_b = vec![[0.0; 3]; old.mesh.n_boundary_faces()];
            let uold: Vec<Vec3> = self.u.values.clone();
            let fo = face_flux(&old, &uold, &zero_b);
            let (own, nei) = (new_geom.mesh.owner(), new_geom.mesh.neighbour());
            (0..n_int)
                .map(|f| {
                    if !kept[f] {
                        return 0.0;
                    }
                    let (o, nn) = (own[f], nei[f]);
                    let w = new_geom.weights[f];
                    let r = w * vol_old[o] / vol_new[o] + (1.0 - w) * vol_old[nn] / vol_new[nn];
                    r * (self.phi[f] - fo[f])
                })
                .collect()
        } else {
            vec![0.0; n_int]
        };
        let u_old: Vec<Vec3> = self.u.values.clone();
        let n_changed = (0..n).filter(|&c| conn_old.kind[c] != conn.kind[c]).count();
        // ADR 0241 D3: with BDF2 on, the old level as the next step needs it (taken before the refresh M9)
        let bdf_on = self.bdf2_opt_in && s.time_scheme == TimeScheme::Bdf2;
        let level = bdf_on.then(|| {
            let own = old.mesh.owner();
            let defect = if s.ddt_correction {
                let zero_b = vec![[0.0; 3]; old.mesh.n_boundary_faces()];
                let fo = face_flux(&old, &u_old, &zero_b);
                (0..n_int)
                    .map(|f| if kept[f] { self.phi[f] - fo[f] } else { 0.0 })
                    .collect()
            } else {
                vec![0.0; n_int]
            };
            OldLevel {
                hole: conn_old.kind.iter().map(|k| *k == CellKind::Hole).collect(),
                phi: self.phi.clone(),
                flux_ok: (0..old.mesh.n_faces())
                    .map(|f| {
                        if f < n_int {
                            kept[f]
                        } else {
                            conn_old.kind[own[f]] == CellKind::Calculated
                        }
                    })
                    .collect(),
                defect,
            }
        });
        self.geom = new_geom;
        if let (Some(cp), Some(pz)) = (self.coupling.as_mut(), pose) {
            cp.pose = pz;
        }
        let g = self.geom.clone();
        let active = g.active;
        if !trivial {
            // M9: the fringe cells from their new donors (calculated now, so defined), the hole cells the rigid velocity,
            // and the faces that do not keep their flux the face interpolation of that velocity on the new geometry
            let shift = if self.mass == MassMode::Conservative {
                self.delta
            } else {
                0.0
            };
            for (c, stn) in &conn.stencils {
                let mut v = [0.0; 3];
                let mut q = 0.0;
                for (&k, &w) in stn.cells.iter().zip(&stn.weights) {
                    for d in 0..3 {
                        v[d] += w * u_old[k][d];
                    }
                    q += w * self.p.values[k];
                }
                for d in 0..3 {
                    if active[d] {
                        self.u.values[*c][d] = v[d];
                    }
                }
                self.p.values[*c] = q + if *c < nb { shift } else { 0.0 };
            }
            for c in 0..n {
                if conn.kind[c] == CellKind::Hole {
                    self.u.values[c] = hole_velocity[c];
                }
            }
            let zero_b = vec![[0.0; 3]; g.mesh.n_boundary_faces()];
            let fi = face_flux(&g, &self.u.values, &zero_b);
            for f in 0..n_int {
                if !kept[f] {
                    self.phi[f] = fi[f];
                }
            }
        }
        // ADR 0241 D2 to D4: the three-level data of a BDF2 step (none on its first step, which is an implicit Euler step)
        let mut step_flux: Option<Vec<f64>> = None;
        let mut defect = defect;
        let bdf: Option<Bdf2Step> = match (&level, self.bdf2.take()) {
            (Some(lv), Some(h)) => {
                let r = dt / h.dt;
                let (c0, c1, c2) = ((1.0 + 2.0 * r) / (1.0 + r), 1.0 + r, r * r / (1.0 + r));
                let full: Vec<bool> = (0..n)
                    .map(|c| !h.hole[c] && conn_old.kind[c] != CellKind::Hole)
                    .collect();
                // the mesh flux whose sum over a cell is (c0 V^{n+1} - c1 V^n + c2 V^{n-1}) / dt
                step_flux = Some(mesh_flux.clone());
                for (m1, m0) in mesh_flux.iter_mut().zip(&h.mesh_flux) {
                    *m1 = c0 * *m1 - c2 / r * m0;
                }
                if s.ddt_correction {
                    let vol_new = g.mesh.cell_volumes();
                    let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
                    for f in 0..n_int {
                        let (o, nn) = (own[f], nei[f]);
                        if full[o] && full[nn] {
                            let w = g.weights[f];
                            let r00 = w * h.vol[o] / vol_new[o] + (1.0 - w) * h.vol[nn] / vol_new[nn];
                            let mut d = c1 * defect[f];
                            d -= c2 * (r00 * h.defect[f]);
                            defect[f] = d;
                        }
                    }
                }
                let e = c1 - 1.0;
                let phi_first: Vec<f64> = (0..self.phi.len())
                    .map(|f| {
                        if lv.flux_ok[f] && h.flux_ok[f] {
                            (1.0 + e) * lv.phi[f] - e * h.phi[f]
                        } else {
                            self.phi[f]
                        }
                    })
                    .collect();
                let u_first: Vec<Vec3> = (0..n)
                    .map(|c| {
                        if full[c] {
                            [0, 1, 2].map(|k| (1.0 + e) * u_old[c][k] - e * h.u[c][k])
                        } else {
                            self.u.values[c]
                        }
                    })
                    .collect();
                Some(Bdf2Step {
                    c0,
                    c1,
                    c2,
                    full,
                    u_older: h.u,
                    vol_older: h.vol,
                    phi_first,
                    u_first,
                })
            }
            _ => None,
        };
        self.euler_fallback = bdf.as_ref().map_or(0, |b| {
            (0..n)
                .filter(|&c| conn.kind[c] == CellKind::Calculated && !b.full[c])
                .count()
        });
        let ale = AleStep {
            mesh_flux,
            vol_old,
            defect,
            wall_dpdn,
        };
        let (p_iters, unconverged, delta, hole_flux) =
            self.piso(dt, &s, &ale, bdf.as_ref(), &conn, &u_old, &hole_velocity, trivial)?;
        if let Some(lv) = level {
            let AleStep { mesh_flux, vol_old, .. } = ale;
            self.bdf2 = Some(Bdf2History {
                u: u_old,
                vol: vol_old,
                hole: lv.hole,
                phi: lv.phi,
                flux_ok: lv.flux_ok,
                defect: lv.defect,
                mesh_flux: step_flux.unwrap_or(mesh_flux),
                dt,
            });
        }
        self.delta = delta;
        self.step_no += 1;
        let courant = max_courant(&g, &self.phi, dt);
        if let Some((f, v)) = first_nonfinite(&self.phi) {
            return Err(format!(
                "overset: step {}: non-finite flux {v} at face {f}",
                self.step_no
            ));
        }
        if !courant.is_finite() || courant > s.divergence_courant_limit {
            return Err(format!("overset: step {}: Courant number {courant}", self.step_no));
        }
        self.time += dt;
        let div = divergence(&g, &self.phi);
        let cont = (0..n)
            .filter(|&c| conn.kind[c] == CellKind::Calculated)
            .fold(0.0f64, |a, c| a.max(div[c].abs()));
        let area_scale = g
            .mesh
            .face_area_vectors()
            .iter()
            .map(|s| vec3::norm(*s))
            .fold(0.0, f64::max);
        let info = OversetInfo {
            step: self.step_no,
            time: self.time,
            dt,
            courant,
            continuity: cont / area_scale.max(1e-300),
            n_hole: conn.n_hole(),
            n_fringe_background: conn.n_fringe_background(),
            n_fringe_component: conn.n_fringe_component(),
            n_changed,
            n_fallback: conn.n_fallback(),
            delta,
            hole_flux,
            p_iters,
            unconverged,
        };
        self.history.push(info);
        Ok(info)
    }

    /// The faces between a calculated and a fringe background cell with the sign that makes the flux point out of the
    /// calculated cell (`Gamma_B` of ADR 0219 M7).
    fn hole_boundary(&self, conn: &Connectivity) -> Vec<(usize, f64)> {
        let g = &self.geom;
        let (own, nei) = (g.mesh.owner(), g.mesh.neighbour());
        let nb = conn.n_background_cells;
        (0..g.n_internal)
            .filter_map(|f| {
                let (o, n) = (own[f], nei[f]);
                if o >= nb || n >= nb {
                    return None;
                }
                match (conn.kind[o], conn.kind[n]) {
                    (CellKind::Calculated, CellKind::Fringe) => Some((f, 1.0)),
                    (CellKind::Fringe, CellKind::Calculated) => Some((f, -1.0)),
                    _ => None,
                }
            })
            .collect()
    }

    /// The PISO step on the installed geometry (`PimpleSolver::step` for the supported settings, with the overset rows).
    /// Returns the pressure iterations, the unconverged solves, `delta` and the final flux into the hole region.
    #[allow(clippy::too_many_arguments)]
    fn piso(
        &mut self,
        dt: f64,
        s: &PimpleSettings,
        ale: &AleStep,
        bdf: Option<&Bdf2Step>,
        conn: &Connectivity,
        u_old: &[Vec3],
        hole_velocity: &[Vec3],
        trivial: bool,
    ) -> Result<(usize, usize, f64, f64), String> {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let vol = mesh.cell_volumes().to_vec();
        let gamma_nu = uniform_gamma(&g, s.nu);
        let n_active = g.active.iter().filter(|&&a| a).count() as f64;
        let mut p_iters = 0;
        let mut unconverged = 0;
        let hole_rows = |k: usize| -> Vec<f64> {
            if trivial {
                Vec::new()
            } else {
                hole_velocity.iter().map(|v| v[k]).collect()
            }
        };
        let scale_u: Vec<f64> = vol.iter().map(|v| v / dt).collect();
        let gamma_b = (!trivial).then(|| self.hole_boundary(conn));
        let mut delta = 0.0;
        let mut hole_flux = 0.0;
        // ADR 0238 D2: the outer iterations. Every pass re-evaluates the pressure conditions and gradient, the convecting
        // flux (the flux of the pass before it), the momentum matrices with their overset rows, rAU and the pressure system;
        // the kinds, donors and weights are those of the new pose throughout. One pass is the PISO step of ADR 0219.
        for outer in 0..s.n_outer {
            let last = outer + 1 == s.n_outer;
            let (alpha_u, alpha_p) = if last { (1.0, 1.0) } else { (s.alpha_u, s.alpha_p) };
            let mut pbc = pressure_bc(&g, &self.u, &self.p);
            ale.apply_wall_gradients(&mut pbc, &g.dn);
            let ub = self.u.boundary_values(&g);
            let bvals_p = boundary_values_pbc(&g, &pbc, &self.p.values);
            let grad_p = green_gauss(&g, &self.p.values, &bvals_p, 0);

            // ---- momentum predictor (M5) -----------------------------------------------------------------------
            let mut mats: Vec<Option<FvMatrix>> = vec![None, None, None];
            let mut a_p = vec![0.0; n];
            let u_iter: Vec<Vec3> = self.u.values.clone();
            // ADR 0241 D4: the first outer iteration of a BDF2 step takes the flux and the cell values extrapolated in time
            let first = if outer == 0 { bdf } else { None };
            let phi_conv = match first {
                Some(b) => ale.relative_flux(&b.phi_first),
                None => ale.relative_flux(&self.phi),
            };
            for k in 0..3 {
                if !g.active[k] {
                    continue;
                }
                let cells: Vec<f64> = u_iter.iter().map(|v| v[k]).collect();
                let bc = self.u.boundary_coeffs(&g, k);
                let bv: Vec<f64> = ub.iter().map(|v| v[k]).collect();
                let first_cells: Option<Vec<f64>> = first.map(|b| b.u_first.iter().map(|v| v[k]).collect());
                let conv_cells: &[f64] = first_cells.as_deref().unwrap_or(&cells);
                let mut m = convection(&g, &phi_conv, conv_cells, &bc, s.convection, None);
                m.add(&laplacian_opt(&g, &gamma_nu, &bc, None, conv_cells, &bv, None));
                let u_old_k: Vec<f64> = u_old.iter().map(|v| v[k]).collect();
                let mut m = match bdf {
                    None => {
                        let mut m = add_ddt(TimeScheme::ImplicitEuler, m, &vol, dt, &History::new(u_old_k.clone()));
                        ale.fix_ddt(&mut m, &vol, &u_old_k, dt);
                        m
                    }
                    Some(b) => b.add_ddt(m, &vol, &ale.vol_old, dt, &u_old_k, k),
                };
                if alpha_u < 1.0 {
                    m.relax(alpha_u, &cells);
                }
                let mut sys = m.clone();
                for c in 0..n {
                    sys.b[c] -= vol[c] * grad_p[c][k];
                }
                let mut x = cells;
                if trivial {
                    unconverged += usize::from(!sys.solve(&mut x, &s.momentum_solver).converged);
                } else {
                    let (a, b) = overset_system(&sys, conn, &scale_u, &hole_rows(k));
                    unconverged += usize::from(!solve_csr(&a, &b, &mut x, &s.momentum_solver).converged);
                }
                for c in 0..n {
                    self.u.values[c][k] = x[c];
                    a_p[c] += m.a.diag[c];
                }
                mats[k] = Some(m);
            }
            if let Some(c) = self.u.values.iter().position(|v| v.iter().any(|x| !x.is_finite())) {
                return Err(format!("overset: momentum predictor: non-finite velocity in cell {c}"));
            }
            for a in a_p.iter_mut() {
                *a /= n_active;
            }
            let mut r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            if !trivial {
                // M6: interpolated rAU of the fringe cells, dt for the hole cells (no calculated row uses them)
                let calc = r_au.clone();
                for (c, stn) in &conn.stencils {
                    r_au[*c] = stn.cells.iter().zip(&stn.weights).map(|(&k, &w)| w * calc[k]).sum();
                }
                for c in 0..n {
                    if conn.kind[c] == CellKind::Hole {
                        r_au[c] = dt;
                    }
                }
                // ADR 0251 D5: the quadratic weights can be negative and nothing is clipped
                if conn.interpolation == DonorInterpolation::Quadratic {
                    if let Some((c, _)) = conn.stencils.iter().find(|(c, _)| r_au[*c].is_nan() || r_au[*c] <= 0.0) {
                        return Err(format!(
                            "overset: the interpolated rAU of the fringe cell {c} is not positive ({:e})",
                            r_au[*c]
                        ));
                    }
                }
            }
            let gamma_p = face_interpolate(&g, &r_au);
            let mut psys: Option<PressureSystem> = None;

            // ---- pressure correctors (M6 to M8) ------------------------------------------------------------------
            for corr in 0..s.n_correctors {
                let mut hby_a = vec![[0.0; 3]; n];
                for k in 0..3 {
                    if let Some(m) = &mats[k] {
                        let cells: Vec<f64> = self.u.values.iter().map(|v| v[k]).collect();
                        let h = m.a.h_operator(&cells, &m.b);
                        for c in 0..n {
                            hby_a[c][k] = h[c] / a_p[c];
                        }
                    }
                }
                if !trivial {
                    let calc = hby_a.clone();
                    for (c, stn) in &conn.stencils {
                        let mut v = [0.0; 3];
                        for (&k, &w) in stn.cells.iter().zip(&stn.weights) {
                            for d in 0..3 {
                                v[d] += w * calc[k][d];
                            }
                        }
                        for d in 0..3 {
                            if g.active[d] {
                                hby_a[*c][d] = v[d];
                            }
                        }
                    }
                    for c in 0..n {
                        if conn.kind[c] == CellKind::Hole {
                            for d in 0..3 {
                                if g.active[d] {
                                    hby_a[c][d] = hole_velocity[c][d];
                                }
                            }
                        }
                    }
                }
                let hb = boundary_hby_a(&g, &pbc, &ub, &hby_a);
                let mut phi_hby_a = face_flux(&g, &hby_a, &hb);
                if s.ddt_correction {
                    for f in 0..n_int {
                        phi_hby_a[f] += gamma_p[f] / dt * ale.defect[f];
                    }
                }
                ale.add_wall_flux(&g, &pbc, &gamma_p, &mut phi_hby_a);
                if let Some((f, v)) = first_nonfinite(&phi_hby_a) {
                    return Err(format!(
                        "overset: pressure right-hand side: non-finite phiHbyA {v} at face {f}"
                    ));
                }
                let gamma_j = gamma_p.clone();
                let p_old = self.p.values.clone();
                let mut p_new = p_old.clone();
                if trivial {
                    let controls = PressureControls {
                        p_ref: s.p_ref,
                        non_orth_correctors: s.non_orth_correctors,
                        solver: s.pressure_solver,
                        amg: s.amg_pressure,
                        linear: s.linear,
                    };
                    let (_r0, it) = solve_pressure(&g, &gamma_j, &pbc, &phi_hby_a, &mut p_new, &controls);
                    p_iters += it;
                } else {
                    let gb = gamma_b.as_deref().unwrap_or(&[]);
                    let (it, unc, d) =
                        self.overset_pressure(&g, &gamma_j, &pbc, &phi_hby_a, &mut p_new, conn, gb, &mut psys, s)?;
                    p_iters += it;
                    unconverged += unc;
                    delta = d;
                }
                if let Some((c, v)) = first_nonfinite(&p_new) {
                    return Err(format!("overset: pressure solve: non-finite pressure {v} in cell {c}"));
                }
                self.phi = corrected_flux(&g, &gamma_j, &pbc, &phi_hby_a, &p_new, &ub, false);
                let bv_new = boundary_values_pbc(&g, &pbc, &p_new);
                let grad_new = green_gauss(&g, &p_new, &bv_new, 0);
                for c in 0..n {
                    let r_c = r_au[c];
                    for k in 0..3 {
                        if g.active[k] {
                            let bforce = 0.0;
                            self.u.values[c][k] = hby_a[c][k] + bforce - r_c * grad_new[c][k];
                        }
                    }
                }
                if !trivial {
                    // M8: the fringe and hole velocities, and the flux of the faces without a calculated cell
                    for (c, stn) in &conn.stencils {
                        let mut v = [0.0; 3];
                        for (&k, &w) in stn.cells.iter().zip(&stn.weights) {
                            for d in 0..3 {
                                v[d] += w * self.u.values[k][d];
                            }
                        }
                        for d in 0..3 {
                            if g.active[d] {
                                self.u.values[*c][d] = v[d];
                            }
                        }
                    }
                    for c in 0..n {
                        if conn.kind[c] == CellKind::Hole {
                            self.u.values[c] = hole_velocity[c];
                        }
                    }
                    let ubn = self.u.boundary_values(&g);
                    let fi = face_flux(&g, &self.u.values, &ubn);
                    let (own, nei) = (mesh.owner(), mesh.neighbour());
                    for f in 0..mesh.n_faces() {
                        let has_calc = conn.kind[own[f]] == CellKind::Calculated
                            || (f < n_int && conn.kind[nei[f]] == CellKind::Calculated);
                        if !has_calc {
                            self.phi[f] = fi[f];
                        }
                    }
                    hole_flux = gb_flux(&self.phi, gamma_b.as_deref().unwrap_or(&[]));
                }
                if let Some(c) = self.u.values.iter().position(|v| v.iter().any(|x| !x.is_finite())) {
                    return Err(format!("overset: velocity correction: non-finite velocity in cell {c}"));
                }
                let a = if corr + 1 == s.n_correctors { alpha_p } else { 1.0 };
                for c in 0..n {
                    self.p.values[c] = p_old[c] + a * (p_new[c] - p_old[c]);
                }
            }
        }
        Ok((p_iters, unconverged, delta, hole_flux))
    }

    /// The pressure solve with the overset rows and, in the conservative mode, the offset `delta` (ADR 0219 M7). `psys`
    /// holds the matrix, its AMG hierarchy and the response to a unit offset, built in the first corrector of the step.
    /// Returns the iterations, the unconverged solves and `delta`.
    #[allow(clippy::too_many_arguments)]
    fn overset_pressure(
        &self,
        g: &FvGeometry,
        gamma_p: &[f64],
        pbc: &PressureBc,
        phi_hby_a: &[f64],
        p: &mut [f64],
        conn: &Connectivity,
        gamma_b: &[(usize, f64)],
        psys: &mut Option<PressureSystem>,
        s: &PimpleSettings,
    ) -> Result<(usize, usize, f64), String> {
        let n = g.n_cells();
        let nb = conn.n_background_cells;
        let own = g.mesh.owner();
        let div = divergence(g, phi_hby_a);
        let mut pm = laplacian(g, gamma_p, &pbc.coeffs, None);
        for (b, d) in pm.b.iter_mut().zip(&div) {
            *b -= d;
        }
        let fixes = (0..pbc.coeffs.len())
            .any(|i| pbc.coeffs[i].c != 0.0 && conn.kind[own[g.n_internal + i]] == CellKind::Calculated);
        let (cell_ref, value_ref) = s.p_ref;
        if !fixes {
            let d = pm.a.diag[cell_ref];
            pm.a.diag[cell_ref] += d;
            pm.b[cell_ref] += d * value_ref;
        }
        let scale: Vec<f64> = pm.a.diag.clone();
        let current: Vec<f64> = p.to_vec();
        let b = match psys {
            Some(_) => overset_rhs(&pm, conn, &scale, &current),
            None => {
                let (a, b) = overset_system(&pm, conn, &scale, &current);
                let amg = Amg::new(&a, AmgSettings::default());
                *psys = Some(PressureSystem { a, amg, p1: None });
                b
            }
        };
        let sys = psys.as_mut().expect("set above");
        let mut iters = 0;
        let mut unc = 0;
        let st0 = solve_pressure_rows(sys, &b, p, &s.pressure_solver);
        iters += st0.iterations;
        unc += usize::from(!st0.converged);
        let n_bg_fringe = conn.stencils.iter().filter(|(c, _)| *c < nb).count();
        if self.mass == MassMode::Interpolated || n_bg_fringe == 0 {
            return Ok((iters, unc, 0.0));
        }
        if sys.p1.is_none() {
            let mut e = vec![0.0; n];
            for (c, _) in &conn.stencils {
                if *c < nb {
                    e[*c] = scale[*c];
                }
            }
            let mut x = vec![0.0; n];
            let st1 = solve_pressure_rows(sys, &e, &mut x, &s.pressure_solver);
            iters += st1.iterations;
            unc += usize::from(!st1.converged);
            sys.p1 = Some(x);
        }
        let q1 = sys.p1.as_deref().expect("set above");
        let lap = &g.lap_coeff;
        let (o, nn) = (g.mesh.owner(), g.mesh.neighbour());
        let jac = |q: &[f64]| -> f64 {
            gamma_b
                .iter()
                .map(|&(f, sg)| -sg * gamma_p[f] * lap[f] * (q[nn[f]] - q[o[f]]))
                .sum()
        };
        let delta = if fixes {
            let q_h: f64 = gamma_b.iter().map(|&(f, sg)| sg * phi_hby_a[f]).sum();
            -(q_h + jac(p)) / jac(q1)
        } else {
            (value_ref - p[cell_ref]) / q1[cell_ref]
        };
        if !delta.is_finite() {
            return Err(format!("overset: the mass offset is not finite ({delta})"));
        }
        for (pi, qi) in p.iter_mut().zip(q1) {
            *pi += delta * qi;
        }
        Ok((iters, unc, delta))
    }
}

/// Net flux out of the calculated background cells through `Gamma_B`.
fn gb_flux(phi: &[f64], gamma_b: &[(usize, f64)]) -> f64 {
    gamma_b.iter().map(|&(f, sg)| sg * phi[f]).sum()
}
