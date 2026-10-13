//! Steady incompressible laminar flow: SIMPLE and SIMPLEC pressure-velocity
//! coupling on collocated meshes with Rhie-Chow (momentum) interpolation.
//!
//! Algorithm per iteration (kinematic pressure, constant viscosity):
//! 1. assemble and implicitly under-relax the momentum matrix with the
//!    current face flux; solve `A u = b - V grad p`;
//! 2. `HbyA = H / a_P`, face flux `phiHbyA = HbyA_f . S` (linear
//!    interpolation; the pressure-gradient part is dropped so that the
//!    face pressure gradient enters compactly -> Rhie-Chow);
//! 3. solve `div(rAU grad p) = div(phiHbyA)` (reference cell pinned when
//!    the problem is pressure-singular), correct the flux
//!    `phi = phiHbyA - rAU_f snGrad(p) |S|` and the cell velocity
//!    `u = HbyA - rAU grad p`.
//!
//! SIMPLEC replaces `rAU` in the pressure equation by `V / (a_P - H1)`
//! with `H1` the sum of neighbour coefficients.

use crate::convection::{convection, face_flux, ConvScheme};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{green_gauss, least_squares};
use crate::pressure::{
    boundary_hby_a, boundary_values_pbc, corrected_flux, face_interpolate, pressure_bc, solve_pressure,
    PressureControls,
};
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::SolveControl;
use std::sync::Arc;

/// Pressure-velocity coupling algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coupling {
    /// Classic SIMPLE (needs pressure under-relaxation).
    Simple,
    /// SIMPLE-Consistent (pressure relaxation near 1).
    Simplec,
}

/// Cell and face values of a body acceleration (per unit mass). The face values
/// `a_f . S_f` enter the face flux exactly like the pressure gradient does, which
/// keeps buoyancy and pressure balanced (no spurious currents at rest).
#[derive(Debug, Clone)]
pub struct BodyForce {
    /// Acceleration at every cell centre.
    pub cell: Vec<Vec3>,
    /// `a_f . S_f` at every face (area vector, boundary faces use the owner value).
    pub face: Vec<f64>,
}

impl BodyForce {
    /// Uniform acceleration.
    pub fn uniform(geom: &FvGeometry, a: Vec3) -> Self {
        let sf = geom.mesh.face_area_vectors();
        BodyForce {
            cell: vec![a; geom.n_cells()],
            face: sf.iter().map(|s| vec3::dot(a, *s)).collect(),
        }
    }

    /// Acceleration `a = value(cell scalar) * dir` with the scalar interpolated linearly to the faces
    /// (boundary faces: `boundary` values, indexed `f - n_internal`).
    pub fn from_scalar(geom: &FvGeometry, scalar: &[f64], boundary: &[f64], dir: Vec3) -> Self {
        let mesh = &geom.mesh;
        let sf = mesh.face_area_vectors();
        let cell = scalar.iter().map(|&x| vec3::scale(dir, x)).collect();
        let face = (0..mesh.n_faces())
            .map(|f| {
                let v = if f < geom.n_internal {
                    let w = geom.weights[f];
                    w * scalar[mesh.owner()[f]] + (1.0 - w) * scalar[mesh.neighbour()[f]]
                } else {
                    boundary[f - geom.n_internal]
                };
                v * vec3::dot(dir, sf[f])
            })
            .collect();
        BodyForce { cell, face }
    }

    /// Acceleration given at every cell centre, interpolated linearly to the faces
    /// (boundary faces use the owner value).
    pub fn from_cells(geom: &FvGeometry, cell: Vec<Vec3>) -> Self {
        let mesh = &geom.mesh;
        let sf = mesh.face_area_vectors();
        let face = (0..mesh.n_faces())
            .map(|f| {
                let a = if f < geom.n_internal {
                    let w = geom.weights[f];
                    vec3::add(
                        vec3::scale(cell[mesh.owner()[f]], w),
                        vec3::scale(cell[mesh.neighbour()[f]], 1.0 - w),
                    )
                } else {
                    cell[mesh.owner()[f]]
                };
                vec3::dot(a, sf[f])
            })
            .collect();
        BodyForce { cell, face }
    }

    /// Sum of two body forces.
    pub fn add(&mut self, o: &BodyForce) {
        for (a, b) in self.cell.iter_mut().zip(&o.cell) {
            *a = vec3::add(*a, *b);
        }
        for (a, b) in self.face.iter_mut().zip(&o.face) {
            *a += b;
        }
    }
}

/// How the pseudo-time step of a pseudo-transient steady solve is chosen (F-106).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PseudoMode {
    /// One global pseudo time step.
    Physical {
        /// Pseudo time step.
        dt: f64,
    },
    /// Per-cell step from a local Courant/diffusion number `cfl`:
    /// `dt_c = cfl / ((0.5 sum|phi| + sum nu_f lap_f) / V_c)`.
    Local {
        /// Local pseudo-Courant number.
        cfl: f64,
    },
    /// One global step `factor * L / U` with `L = V_domain^(1/dim)` and `U` the
    /// current maximum speed (after the solver's own boundary-velocity scale).
    Auto,
}

/// Pseudo-transient settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PseudoTransient {
    /// Time-step selection.
    pub mode: PseudoMode,
    /// Multiplier applied to the selected step (the "timescale factor").
    pub factor: f64,
}

/// Adjusts a uniform body acceleration along `dir` each iteration so that the
/// volume-mean velocity along `dir` approaches `target` (the usual way to drive
/// a periodic channel at a prescribed bulk velocity; cf. Patankar, Liu and
/// Sparrow, J. Heat Transfer 99 (1977) 180).
#[derive(Debug, Clone, Copy)]
pub struct MeanVelocityDriver {
    /// Flow direction (unit).
    pub dir: Vec3,
    /// Target bulk velocity along `dir`.
    pub target: f64,
    /// Current acceleration magnitude.
    pub accel: f64,
}

/// Solver settings.
#[derive(Debug, Clone)]
pub struct SimpleSettings {
    /// Kinematic viscosity.
    pub nu: f64,
    /// Coupling algorithm.
    pub coupling: Coupling,
    /// Velocity under-relaxation factor.
    pub alpha_u: f64,
    /// Pressure under-relaxation factor.
    pub alpha_p: f64,
    /// Convection scheme for momentum.
    pub convection: ConvScheme,
    /// Extra pressure solves with non-orthogonal correction per iteration.
    pub non_orth_correctors: usize,
    /// Reference cell and value used when the pressure level is undetermined.
    pub p_ref: (usize, f64),
    /// Evaluate the H operator from the solved momentum predictor (as in classic SIMPLE and
    /// OpenFOAM `simpleFoam`) instead of from the previous iterate. Default on since wave 6: the
    /// previous-iterate H makes the iteration marginally unstable for second-order convection on
    /// the backward-facing step (F-106 diagnosis). Ignored (previous iterate used) while a
    /// [`MeanVelocityDriver`] is active, whose `1/rAU` gain assumes the lagged response.
    pub h_from_solution: bool,
    /// Linear solver controls for momentum.
    pub momentum_solver: SolveControl,
    /// Linear solver controls for pressure.
    pub pressure_solver: SolveControl,
}

impl SimpleSettings {
    /// Reasonable defaults for the chosen coupling.
    pub fn new(nu: f64, coupling: Coupling) -> Self {
        let (alpha_u, alpha_p) = match coupling {
            Coupling::Simple => (0.7, 0.3),
            Coupling::Simplec => (0.9, 1.0),
        };
        SimpleSettings {
            nu,
            coupling,
            alpha_u,
            alpha_p,
            convection: ConvScheme::Upwind,
            non_orth_correctors: 0,
            p_ref: (0, 0.0),
            h_from_solution: true,
            momentum_solver: SolveControl {
                tol_abs: 1e-12,
                tol_rel: 1e-3,
                max_iter: 200,
            },
            pressure_solver: SolveControl {
                tol_abs: 1e-14,
                tol_rel: 1e-3,
                max_iter: 2000,
            },
        }
    }
}

/// Per-iteration diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct IterationInfo {
    /// Iteration number (1-based).
    pub iteration: usize,
    /// Largest change of any velocity component in the iteration.
    pub max_du: f64,
    /// Largest cell imbalance of the corrected flux, `|sum phi|` divided by `U_scale * face area scale`.
    pub continuity: f64,
    /// Linear-solver iterations spent on pressure.
    pub p_iters: usize,
    /// Normalised initial momentum residual (largest over components).
    pub res_u: f64,
    /// Normalised initial pressure-equation residual.
    pub res_p: f64,
}

/// Normalised residual `sum|b - A x| / normFactor` with the usual
/// `normFactor = sum(|A x - A xbar| + |b - A xbar|)` (xbar = mean of x).
pub fn normalised_residual(m: &FvMatrix, x: &[f64]) -> f64 {
    let n = x.len();
    let xbar = x.iter().sum::<f64>() / n as f64;
    let mut ax = vec![0.0; n];
    m.a.matvec(x, &mut ax);
    let mut axb = vec![0.0; n];
    m.a.matvec(&vec![xbar; n], &mut axb);
    let num: f64 = (0..n).map(|i| (m.b[i] - ax[i]).abs()).sum();
    let norm: f64 = (0..n).map(|i| (ax[i] - axb[i]).abs() + (m.b[i] - axb[i]).abs()).sum();
    num / (norm + 1e-30)
}

/// [`normalised_residual`] for a CSR system.
pub fn normalised_residual_csr(a: &rcfd_linalg::CsrMatrix, b: &[f64], x: &[f64]) -> f64 {
    let n = x.len();
    let xbar = x.iter().sum::<f64>() / n as f64;
    let mut ax = vec![0.0; n];
    a.matvec(x, &mut ax);
    let mut axb = vec![0.0; n];
    a.matvec(&vec![xbar; n], &mut axb);
    let num: f64 = (0..n).map(|i| (b[i] - ax[i]).abs()).sum();
    let norm: f64 = (0..n).map(|i| (ax[i] - axb[i]).abs() + (b[i] - axb[i]).abs()).sum();
    num / (norm + 1e-30)
}

/// Residual-based stopping criteria (all given tolerances must be met).
#[derive(Debug, Clone, Copy)]
pub struct ConvergenceCriteria {
    /// Iteration cap.
    pub max_iter: usize,
    /// Tolerance on the normalised momentum residual.
    pub res_u: f64,
    /// Tolerance on the normalised pressure residual.
    pub res_p: f64,
}

/// Outcome of [`SimpleSolver::run`].
#[derive(Debug, Clone)]
pub struct RunSummary {
    /// Iterations performed.
    pub iterations: usize,
    /// Convergence criterion reached.
    pub converged: bool,
    /// Last iteration diagnostics.
    pub last: IterationInfo,
}

/// Steady SIMPLE/SIMPLEC solver state.
pub struct SimpleSolver {
    /// Geometry.
    pub geom: Arc<FvGeometry>,
    /// Settings.
    pub settings: SimpleSettings,
    /// Velocity field with its boundary conditions.
    pub u: VectorField,
    /// Kinematic pressure with its boundary conditions.
    pub p: ScalarField,
    /// Face flux (positive out of the owner), consistent with continuity.
    pub phi: Vec<f64>,
    /// Diagnostics of every iteration performed so far.
    pub history: Vec<IterationInfo>,
    /// Effective kinematic viscosity at every face (laminar plus turbulent); `None` uses `settings.nu`.
    pub nu_face: Option<Vec<f64>>,
    /// Body acceleration (buoyancy, ...), applied with balanced-force treatment.
    pub body_force: Option<BodyForce>,
    /// Implicit linear momentum sink per cell, `-sink * u` (porous media), in 1/s.
    pub sink: Option<Vec<f64>>,
    /// Pseudo-transient (local time stepping) option.
    pub pseudo: Option<PseudoTransient>,
    /// Bulk-velocity driver for periodic flows.
    pub driver: Option<MeanVelocityDriver>,
    iteration: usize,
    /// `rAU / rAtU` of the previous iteration (wall pressure extrapolation with body forces).
    body_ratio: Vec<f64>,
}

/// Restartable solver state (everything that influences later iterations).
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleState {
    /// Iterations completed.
    pub iteration: usize,
    /// Cell velocities.
    pub u: Vec<Vec3>,
    /// Cell pressures.
    pub p: Vec<f64>,
    /// Face fluxes.
    pub phi: Vec<f64>,
}

impl SimpleSolver {
    /// Create a solver; initial flux is computed from the velocity field.
    pub fn new(geom: Arc<FvGeometry>, u: VectorField, p: ScalarField, settings: SimpleSettings) -> Self {
        let bu = u.boundary_values(&geom);
        let phi = face_flux(&geom, &u.values, &bu);
        SimpleSolver {
            geom,
            settings,
            u,
            p,
            phi,
            history: Vec::new(),
            nu_face: None,
            body_force: None,
            sink: None,
            pseudo: None,
            driver: None,
            iteration: 0,
            body_ratio: Vec::new(),
        }
    }

    /// Pseudo time step of every cell for the current state (None without pseudo-transient).
    pub fn pseudo_time_steps(&self) -> Option<Vec<f64>> {
        let ps = self.pseudo?;
        let g = &self.geom;
        let mesh = &g.mesh;
        let n = g.n_cells();
        let vol = mesh.cell_volumes();
        let dt: Vec<f64> = match ps.mode {
            PseudoMode::Physical { dt } => vec![dt; n],
            PseudoMode::Local { cfl } => (0..n)
                .map(|c| {
                    let mut rate = 0.0;
                    for &f in mesh.cell_faces(c) {
                        if g.empty[f] {
                            continue;
                        }
                        let nu = self.nu_face.as_ref().map_or(self.settings.nu, |v| v[f]);
                        rate += 0.5 * self.phi[f].abs() + nu * g.lap_coeff[f];
                    }
                    cfl * vol[c] / rate.max(1e-300)
                })
                .collect(),
            PseudoMode::Auto => {
                let dim = g.active.iter().filter(|&&a| a).count().max(1) as f64;
                let len = mesh.total_volume().powf(1.0 / dim);
                let ub = self.u.boundary_values(g);
                let umax = self
                    .u
                    .values
                    .iter()
                    .chain(ub.iter())
                    .map(|v| vec3::norm(*v))
                    .fold(0.0, f64::max)
                    .max(1e-12);
                vec![len / umax; n]
            }
        };
        Some(dt.into_iter().map(|d| d * ps.factor).collect())
    }

    /// Run one SIMPLE/SIMPLEC iteration.
    pub fn iterate(&mut self) -> IterationInfo {
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let vol = mesh.cell_volumes();
        let sf = mesh.face_area_vectors();
        let n_int = g.n_internal;
        let s = self.settings.clone();
        self.iteration += 1;
        let u_old = self.u.values.clone();

        // ---- momentum predictor ------------------------------------------
        let gamma_nu = match &self.nu_face {
            Some(v) => v.clone(),
            None => uniform_gamma(&g, s.nu),
        };
        let pseudo_dt = self.pseudo_time_steps();
        let body: Option<BodyForce> = match (&self.body_force, &self.driver) {
            (None, None) => None,
            (b, d) => {
                let mut tot = b.clone().unwrap_or_else(|| BodyForce::uniform(&g, [0.0; 3]));
                if let Some(d) = d {
                    tot.add(&BodyForce::uniform(&g, vec3::scale(d.dir, d.accel)));
                }
                Some(tot)
            }
        };
        let mut pbc = pressure_bc(&g, &self.u, &self.p);
        if let Some(bf) = &body {
            // Where the normal velocity is prescribed the flux is zero, so the pressure
            // gradient balances the body force there: extrapolate the wall pressure with
            // that gradient (the fixed-flux pressure condition) so that the cell-centred
            // gradient matches the cell-centred body force next to walls.
            for i in 0..pbc.coeffs.len() {
                if pbc.fixed_u[i] && pbc.coupled[i].is_none() {
                    let f = n_int + i;
                    let ratio = self.body_ratio.get(mesh.owner()[f]).copied().unwrap_or(1.0);
                    let gradient = ratio * bf.face[f] / vec3::norm(sf[f]).max(1e-300);
                    pbc.coeffs[i] = crate::bc::FaceCoeffs {
                        a: 1.0,
                        b: g.dn[i] * gradient,
                        c: 0.0,
                        d: gradient,
                    };
                }
            }
        }
        let p_bvals_for_grad = boundary_values_pbc(&g, &pbc, &self.p.values);
        let grad_p = green_gauss(&g, &self.p.values, &p_bvals_for_grad, 0);
        let mut a_p = vec![0.0; n];
        let mut h = vec![[0.0; 3]; n];
        let mut h1 = vec![0.0; n];
        let mut n_active = 0.0;
        let mut res_u: f64 = 0.0;
        let ub = self.u.boundary_values(&g);
        for k in 0..3 {
            if !g.active[k] {
                continue;
            }
            n_active += 1.0;
            let cells: Vec<f64> = u_old.iter().map(|v| v[k]).collect();
            let bc = self.u.boundary_coeffs(&g, k);
            let bvals: Vec<f64> = ub.iter().map(|v| v[k]).collect();
            let grad = if s.convection.needs_gradient() || s.non_orth_correctors > 0 {
                Some(least_squares(&g, &cells, &bvals))
            } else {
                None
            };
            let mut m = convection(&g, &self.phi, &cells, &bc, s.convection, grad.as_deref());
            m.add(&laplacian(&g, &gamma_nu, &bc, grad.as_deref()));
            if let Some(sink) = &self.sink {
                for c in 0..n {
                    m.a.diag[c] += sink[c] * vol[c];
                }
            }
            if let Some(dtc) = &pseudo_dt {
                for c in 0..n {
                    let a = vol[c] / dtc[c];
                    m.a.diag[c] += a;
                    m.b[c] += a * cells[c];
                }
            }
            m.relax(s.alpha_u, &cells);
            // H operator (without pressure): b - sum offdiag u
            let mut hk = m.a.h_operator(&cells, &m.b);
            let off = m.a.off_diag_abs_sum();
            for c in 0..n {
                a_p[c] += m.a.diag[c];
                h1[c] += off[c];
            }
            // solve with the pressure gradient as source
            let mut sys = m.clone();
            for c in 0..n {
                sys.b[c] -= vol[c] * grad_p[c][k];
                if let Some(bf) = &body {
                    sys.b[c] += vol[c] * bf.cell[c][k];
                }
            }
            res_u = res_u.max(normalised_residual(&sys, &cells));
            let mut x = cells;
            sys.solve(&mut x, &s.momentum_solver);
            if s.h_from_solution && self.driver.is_none() {
                // classic SIMPLE: neighbour contributions from the solved predictor instead of the old iterate
                hk = m.a.h_operator(&x, &m.b);
            }
            for c in 0..n {
                h[c][k] = hk[c];
                self.u.values[c][k] = x[c];
            }
        }
        for c in 0..n {
            a_p[c] /= n_active;
            h1[c] /= n_active;
        }

        // ---- pressure equation --------------------------------------------
        let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
        let r_atu: Vec<f64> = match s.coupling {
            Coupling::Simple => r_au.clone(),
            Coupling::Simplec => (0..n).map(|c| vol[c] / (a_p[c] - h1[c]).max(1e-3 * a_p[c])).collect(),
        };
        self.body_ratio = (0..n).map(|c| r_au[c] / r_atu[c]).collect();
        let hby_a0: Vec<Vec3> = (0..n)
            .map(|c| {
                let mut v = [0.0; 3];
                for k in 0..3 {
                    if g.active[k] {
                        v[k] = h[c][k] / a_p[c];
                    }
                }
                v
            })
            .collect();
        // SIMPLEC: cell velocity uses HbyA - (rAU - rAtU) grad p; the face flux uses the
        // compact form of the same correction (added below), built from the original HbyA.
        let mut hby_a = hby_a0.clone();
        if let Some(bf) = &body {
            for c in 0..n {
                for k in 0..3 {
                    if g.active[k] {
                        hby_a[c][k] += r_au[c] * bf.cell[c][k];
                    }
                }
            }
        }
        if s.coupling == Coupling::Simplec {
            for c in 0..n {
                for k in 0..3 {
                    hby_a[c][k] -= (r_au[c] - r_atu[c]) * grad_p[c][k];
                }
            }
        }
        let gamma_p = face_interpolate(&g, &r_atu);
        // HbyA at boundary faces: prescribed velocity or owner value
        let hb = boundary_hby_a(&g, &pbc, &ub, &hby_a0);
        let mut phi_hby_a = face_flux(&g, &hby_a0, &hb);
        if let Some(bf) = &body {
            let r_au_f = face_interpolate(&g, &r_au);
            for f in 0..mesh.n_faces() {
                if !g.empty[f] {
                    phi_hby_a[f] += r_au_f[f] * bf.face[f];
                }
            }
        }
        if s.coupling == Coupling::Simplec {
            let corr = self.simplec_flux_correction(&r_au, &r_atu, &self.p.values);
            for f in 0..n_int {
                phi_hby_a[f] += corr[f];
            }
        }
        let mut p_new = self.p.values.clone();
        let p_old = self.p.values.clone();
        let controls = PressureControls {
            p_ref: s.p_ref,
            non_orth_correctors: s.non_orth_correctors,
            solver: s.pressure_solver,
            amg: false,
        };
        let (res_p, p_iters) = solve_pressure(&g, &gamma_p, &pbc, &phi_hby_a, &mut p_new, &controls);
        // flux correction: phi = phiHbyA - rAtU_f snGrad(p) |S|
        let bv_new = boundary_values_pbc(&g, &pbc, &p_new);
        self.phi = corrected_flux(&g, &gamma_p, &pbc, &phi_hby_a, &p_new, &ub, s.non_orth_correctors > 0);

        // under-relax pressure, correct velocity
        for c in 0..n {
            self.p.values[c] = p_old[c] + s.alpha_p * (p_new[c] - p_old[c]);
        }
        // velocity correction uses the new (unrelaxed-consistent) pressure gradient
        let grad_corr = green_gauss(&g, &p_new, &bv_new, 0);
        let mut max_du: f64 = 0.0;
        for c in 0..n {
            for k in 0..3 {
                if g.active[k] {
                    let un = hby_a[c][k] - r_atu[c] * grad_corr[c][k];
                    self.u.values[c][k] = un;
                    max_du = max_du.max((un - u_old[c][k]).abs());
                }
            }
        }
        // bulk-velocity driver: adapt the acceleration so the mean velocity approaches the target
        if let Some(d) = self.driver.as_mut() {
            let (mut ub, mut ra, mut vt) = (0.0, 0.0, 0.0);
            for c in 0..n {
                ub += vol[c] * vec3::dot(self.u.values[c], d.dir);
                ra += vol[c] * r_au[c];
                vt += vol[c];
            }
            let (ub, ra) = (ub / vt, ra / vt);
            d.accel += (d.target - ub) / ra.max(1e-300);
        }
        // continuity measure
        let mut cont: f64 = 0.0;
        for c in 0..n {
            let mut sum = 0.0;
            for &f in mesh.cell_faces(c) {
                if g.empty[f] {
                    continue;
                }
                sum += if f >= n_int || mesh.owner()[f] == c {
                    self.phi[f]
                } else {
                    -self.phi[f]
                };
            }
            cont = cont.max(sum.abs());
        }
        let area_scale = sf.iter().map(|s| vec3::norm(*s)).fold(0.0, f64::max);
        let info = IterationInfo {
            iteration: self.iteration,
            max_du,
            continuity: cont / area_scale.max(1e-300),
            p_iters,
            res_u,
            res_p,
        };
        self.history.push(info);
        info
    }

    /// Capture the restartable state.
    pub fn snapshot(&self) -> SimpleState {
        SimpleState {
            iteration: self.iteration,
            u: self.u.values.clone(),
            p: self.p.values.clone(),
            phi: self.phi.clone(),
        }
    }

    /// Restore a state captured with [`snapshot`](Self::snapshot) (sizes must match the mesh).
    pub fn restore(&mut self, st: &SimpleState) -> rcfd_core::Result<()> {
        let (nc, nf) = (self.geom.n_cells(), self.geom.mesh.n_faces());
        if st.u.len() != nc || st.p.len() != nc || st.phi.len() != nf {
            return Err(rcfd_core::Error::InvalidValue {
                path: "checkpoint".into(),
                reason: "state size does not match the mesh".into(),
            });
        }
        self.u.values = st.u.clone();
        self.p.values = st.p.clone();
        self.phi = st.phi.clone();
        self.iteration = st.iteration;
        self.history.clear();
        Ok(())
    }

    /// Replace the cell velocity and pressure (for example by values interpolated from another
    /// mesh) and rebuild the face flux from the velocity and the boundary values. The iteration
    /// counter restarts at zero.
    pub fn set_fields(&mut self, u: Vec<Vec3>, p: Vec<f64>) -> rcfd_core::Result<()> {
        let nc = self.geom.n_cells();
        if u.len() != nc || p.len() != nc {
            return Err(rcfd_core::Error::InvalidValue {
                path: "initial fields".into(),
                reason: "field size does not match the mesh".into(),
            });
        }
        self.u.values = u;
        self.p.values = p;
        let bu = self.u.boundary_values(&self.geom);
        self.phi = face_flux(&self.geom, &self.u.values, &bu);
        self.iteration = 0;
        self.history.clear();
        Ok(())
    }

    /// Residual history as CSV (`iteration,res_u,res_p,continuity,max_du,p_iters`).
    pub fn history_csv(&self) -> String {
        let mut o = String::from("iteration,res_u,res_p,continuity,max_du,p_iters\n");
        for h in &self.history {
            o.push_str(&format!(
                "{},{:e},{:e},{:e},{:e},{}\n",
                h.iteration, h.res_u, h.res_p, h.continuity, h.max_du, h.p_iters
            ));
        }
        o
    }

    /// Explicit SIMPLEC flux correction `(rAtU - rAU)_f snGrad(p) |S|` from the old pressure
    /// (outward from the owner, internal faces; boundary faces carry none).
    fn simplec_flux_correction(&self, r_au: &[f64], r_atu: &[f64], p: &[f64]) -> Vec<f64> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let mut out = vec![0.0; mesh.n_faces()];
        for f in 0..g.n_internal {
            let w = g.weights[f];
            let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
            let dr = w * (r_atu[o] - r_au[o]) + (1.0 - w) * (r_atu[nb] - r_au[nb]);
            // phiHbyA += (rAtU - rAU) snGrad(p) |S|  ->  sign: HbyA' = HbyA - (rAU - rAtU) grad p
            out[f] = dr * g.lap_coeff[f] * (p[nb] - p[o]);
        }
        out
    }

    /// Iterate until the normalised momentum and pressure residuals are below the given criteria.
    pub fn run_until(&mut self, c: &ConvergenceCriteria) -> RunSummary {
        let mut last = IterationInfo {
            iteration: 0,
            max_du: f64::INFINITY,
            continuity: f64::INFINITY,
            p_iters: 0,
            res_u: f64::INFINITY,
            res_p: f64::INFINITY,
        };
        for _ in 0..c.max_iter {
            last = self.iterate();
            if last.res_u < c.res_u && last.res_p < c.res_p {
                return RunSummary {
                    iterations: last.iteration,
                    converged: true,
                    last,
                };
            }
        }
        RunSummary {
            iterations: last.iteration,
            converged: false,
            last,
        }
    }

    /// Iterate until the largest velocity change and the continuity error
    /// both fall below `tol`, or `max_iter` is reached.
    pub fn run(&mut self, max_iter: usize, tol: f64) -> RunSummary {
        let mut last = IterationInfo {
            iteration: 0,
            max_du: f64::INFINITY,
            continuity: f64::INFINITY,
            p_iters: 0,
            res_u: f64::INFINITY,
            res_p: f64::INFINITY,
        };
        for _ in 0..max_iter {
            last = self.iterate();
            if last.max_du < tol && last.continuity < 1e-6 {
                return RunSummary {
                    iterations: last.iteration,
                    converged: true,
                    last,
                };
            }
        }
        RunSummary {
            iterations: last.iteration,
            converged: false,
            last,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bc::FixedValue;
    use rcfd_mesh::gen::cavity_2d;

    fn small_cavity(coupling: Coupling) -> SimpleSolver {
        let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(10, 1.0).unwrap())).unwrap());
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
            .unwrap();
        u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
            .unwrap();
        let p = ScalarField::uniform("p", &geom, 0.0);
        SimpleSolver::new(geom, u, p, SimpleSettings::new(0.01, coupling))
    }

    #[test]
    fn residual_criteria_stop_the_run_and_flux_is_conservative() {
        for coupling in [Coupling::Simple, Coupling::Simplec] {
            let mut s = small_cavity(coupling);
            let r = s.run_until(&ConvergenceCriteria {
                max_iter: 3000,
                res_u: 1e-7,
                res_p: 1e-7,
            });
            assert!(r.converged, "{coupling:?}: {:?}", r.last);
            assert!(r.iterations < 3000);
            assert!(r.last.continuity < 1e-8, "{:?}", r.last);
            // residuals decrease by orders of magnitude over the run
            assert!(s.history.first().unwrap().res_u > 1e3 * s.history.last().unwrap().res_u);
            // reference cell pins the pressure level
            assert!(s.p.values[0].abs() < 0.5);
        }
    }

    #[test]
    fn restore_rejects_wrong_sizes() {
        let mut s = small_cavity(Coupling::Simple);
        let mut st = s.snapshot();
        st.p.pop();
        assert!(s.restore(&st).is_err());
    }
}
