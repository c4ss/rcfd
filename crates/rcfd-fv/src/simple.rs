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

use crate::convection::{convection, face_flux, lag_boundary_inflow, ConvScheme};
use crate::diffusion::{laplacian, uniform_gamma};
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{green_gauss, least_squares};
use crate::pressure::{
    boundary_hby_a, boundary_values_pbc, corrected_flux, face_interpolate, pressure_bc, solve_pressure_parts,
    PressureBc, PressureControls,
};
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::{LinearOptions, SolveControl};
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
    /// SIMPLEC with `non_orth_correctors > 0`: let the flux compensation `(rAtU - rAU)_f snGrad(p_old) |S|` carry the
    /// non-orthogonal secondary-gradient part as well (ADR 0050 D1). Without it the deferred part of the corrected flux is
    /// weighted with `rAtU` instead of `rAU` and an axial mode of the outer iteration grows for `alpha_u` near 1. Default
    /// `false` (every earlier result unchanged); no effect for SIMPLE or without correctors.
    pub simplec_non_orth_compensation: bool,
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
    /// Linear-solver device of the pressure equation (ADR 0038); default CPU.
    pub linear: LinearOptions,
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
            simplec_non_orth_compensation: false,
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
            linear: LinearOptions::default(),
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

/// Normalisation of the momentum residual (ADR 0023). `Reference` is the long-standing definition
/// ([`normalised_residual`]); `Backward` divides by the magnitude of the summed terms instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResidualNorm {
    /// `sum|b - A x| / sum(|A x - A xbar| + |b - A xbar|)` (default; unchanged).
    #[default]
    Reference,
    /// Normwise backward error `sum|b - A x| / sum_i(sum_j |a_ij||x_j| + |b_i|)`: does not vanish with the
    /// variation of the iterate, so a (nearly) uniform exact solution can reach round-off level.
    Backward,
    /// Iterate-independent common scale (ADR 0023 addendum A2): momentum `max_k num_k / max_j S_j` over the
    /// active components; pressure `num / max(S, T)` with `T` half the total absolute face flux of `phiHbyA`.
    Scaled,
}

/// Parts of a normalised residual (diagnostic, ADR 0023): `num = sum|b - A x|`, `norm_ref` the denominator of
/// the reference residual and `scale = sum_i(sum_j |a_ij||x_j| + |b_i|)` (round-off size of `num` is `eps * scale`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResidualParts {
    /// Numerator `sum|b - A x|`.
    pub num: f64,
    /// Denominator of the reference normalisation.
    pub norm_ref: f64,
    /// Magnitude of the summed terms.
    pub scale: f64,
}

impl ResidualParts {
    /// Reference-normalised residual.
    pub fn res_ref(&self) -> f64 {
        self.num / (self.norm_ref + 1e-30)
    }
    /// Backward-error residual.
    pub fn res_bwd(&self) -> f64 {
        self.num / (self.scale + 1e-30)
    }
}

/// Parts of the residual of `m` at `x` (see [`ResidualParts`]).
pub fn residual_parts(m: &FvMatrix, x: &[f64]) -> ResidualParts {
    residual_parts_csr(&m.a.to_csr(), &m.b, x)
}

/// [`residual_parts`] for a CSR system.
pub fn residual_parts_csr(csr: &rcfd_linalg::CsrMatrix, b: &[f64], x: &[f64]) -> ResidualParts {
    let n = x.len();
    let xbar = x.iter().sum::<f64>() / n as f64;
    let mut ax = vec![0.0; n];
    csr.matvec(x, &mut ax);
    let mut axb = vec![0.0; n];
    csr.matvec(&vec![xbar; n], &mut axb);
    let num: f64 = (0..n).map(|i| (b[i] - ax[i]).abs()).sum();
    let norm_ref: f64 = (0..n).map(|i| (ax[i] - axb[i]).abs() + (b[i] - axb[i]).abs()).sum();
    let mut scale = 0.0;
    for i in 0..n {
        let mut row = b[i].abs();
        for k in csr.row_ptr[i]..csr.row_ptr[i + 1] {
            row += csr.val[k].abs() * x[csr.col[k]].abs();
        }
        scale += row;
    }
    ResidualParts { num, norm_ref, scale }
}

/// Update law of the [`PseudoController`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PseudoLaw {
    /// ADR 0023 D1 (settings value `ser`): grows on every falling residual, backs off only on a rise by more than
    /// `rise`. A one-way ratchet for oscillating residuals (registered misses, addendum A3); kept for the record.
    Ratchet,
    /// ADR 0023 addendum A3 (settings value `ser2`): switched evolution relaxation, the scale follows the inverse
    /// residual ratio in both directions, clamped to `[backoff, growth]` per iteration.
    Symmetric,
}

/// Automatic pseudo-time-step controller (ADR 0023 D1, A3): switched evolution relaxation.
/// The pseudo time step of every cell is multiplied by `scale`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PseudoController {
    /// Update law.
    pub law: PseudoLaw,
    /// Largest growth factor of the scale per iteration.
    pub growth: f64,
    /// Factor applied to the scale when the residual rises by more than `rise` or is not finite.
    pub backoff: f64,
    /// Residual rise ratio that triggers the back-off.
    pub rise: f64,
    /// Smallest scale.
    pub min_scale: f64,
    /// Largest scale.
    pub max_scale: f64,
    /// Current scale.
    pub scale: f64,
    prev: f64,
}

impl PseudoController {
    /// Controller with the registered constants (growth 2, back-off 0.5, rise 2, minimum 1/64) and the given cap.
    pub fn new(max_scale: f64) -> Self {
        PseudoController {
            law: PseudoLaw::Ratchet,
            growth: 2.0,
            backoff: 0.5,
            rise: 2.0,
            min_scale: 1.0 / 64.0,
            max_scale,
            scale: 1.0,
            prev: f64::INFINITY,
        }
    }

    /// Controller with the symmetric law of addendum A3 and the same constants.
    pub fn new_symmetric(max_scale: f64) -> Self {
        PseudoController {
            law: PseudoLaw::Symmetric,
            ..Self::new(max_scale)
        }
    }

    /// Update the scale from the residual `r` of the iteration just finished.
    pub fn update(&mut self, r: f64) {
        if !r.is_finite() {
            self.scale = (self.backoff * self.scale).max(self.min_scale);
            return;
        }
        if self.law == PseudoLaw::Symmetric {
            if self.prev.is_finite() {
                let ratio = if r > 0.0 { self.prev / r } else { self.growth };
                self.scale =
                    (self.scale * ratio.clamp(self.backoff, self.growth)).clamp(self.min_scale, self.max_scale);
            }
            self.prev = r;
            return;
        }
        if self.prev.is_finite() {
            if r > self.rise * self.prev {
                self.scale = (self.backoff * self.scale).max(self.min_scale);
            } else {
                let ratio = if r > 0.0 {
                    (self.prev / r).clamp(1.0, self.growth)
                } else {
                    self.growth
                };
                self.scale = (self.scale * ratio).min(self.max_scale);
            }
        }
        self.prev = r;
    }
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
    /// Automatic pseudo-time-step controller (ADR 0023); needs `pseudo`. `None` keeps the step as configured.
    pub pseudo_control: Option<PseudoController>,
    /// Scale applied to the pseudo time step in every iteration performed (filled when the controller is on).
    pub pseudo_scale_history: Vec<f64>,
    /// Normalisation of the momentum residual (default [`ResidualNorm::Reference`], the historical definition).
    pub residual_norm: ResidualNorm,
    /// Record the residual parts of every momentum component in every iteration (diagnostic, off by default).
    pub record_parts: bool,
    /// Residual parts per iteration and active component (filled when `record_parts` is on).
    pub parts_history: Vec<Vec<ResidualParts>>,
    /// Parts of the initial pressure-equation residual per iteration (filled when `record_parts` is on).
    pub pressure_parts_history: Vec<Option<ResidualParts>>,
    /// Bulk-velocity driver for periodic flows.
    pub driver: Option<MeanVelocityDriver>,
    /// Wall pressure used by the cell gradients next to fixed-velocity walls with a body force: chosen so that the
    /// wall-normal gradient of the wall cell equals the cell-centre body force (off by default; set for
    /// rotating-frame sources, F-031). With SIMPLEC it also applies the lagged (rAU - rAtU) correction to the wall
    /// pressure gradient of the flux balance.
    pub wall_pressure_trapezoid: bool,
    /// Use the distance-weighted mean of `rAU` of the two cells of a periodic pair for the body-force term of the face flux on
    /// periodic boundary faces (ADR 0071 C5). The default (owner value) makes the two fluxes of a pair differ by
    /// `(rAU_a - rAU_b) f_f . S` whenever the solution is not the same on both sides of the seam, so a body force that acts on
    /// the seam faces (Coriolis, centrifugal) breaks the exact antisymmetry of the rotational pair. Off by default so that every
    /// earlier case stays bit-identical; the API switches it on together with `wall_pressure_trapezoid` (rotating-frame sources).
    pub coupled_body_pair_mean: bool,
    /// Per-face offset subtracted from the flux that convects momentum (`phi_conv = phi - offset`, F-031 MRF zones:
    /// the frame flux `(Omega x r) . S`); the pressure equation and `phi` itself are unaffected.
    pub conv_flux_offset: Option<Vec<f64>>,
    /// Make the H operator consistent with the component-averaged diagonal `a_p`: in cells where the component
    /// matrices have different diagonals (a symmetry plane puts a Dirichlet term on the normal component only) the
    /// H of component `k` gets `(a_p - a_k) u_k`, as OpenFOAM's `fvMatrix::H` does. Without it the converged
    /// tangential velocity next to such a plane is multiplied by `a_k / a_p < 1` (a spurious end-wall drag, found by the
    /// MRF Taylor-Couette checks of ADR 0008). Off by default so that every earlier case stays bit-identical (it
    /// changes the packed-bed baseline hash, ADR 0010); the API switches it on for MRF zones and for
    /// `solver.component_diagonal = "corrected"`.
    pub component_diag_correction: bool,
    /// Pressure jumps at internal faces (lumped fans and porous jumps, ADR 0012); `None` leaves the solver unchanged.
    pub face_jump: Option<crate::lumped::FaceJump>,
    /// Per boundary face (index `f - n_internal`): `true` makes the mass flux of a fixed-velocity face zero while the momentum
    /// condition keeps the prescribed wall velocity (ADR 0072, the impermeable wall of revolution whose facets are not exactly
    /// perpendicular to its velocity). `None` (default) leaves the solver unchanged.
    pub impermeable: Option<Vec<bool>>,
    /// Relaxed jump per face consistent with the relaxed pressure (face-jump hook state).
    jump_state: Vec<f64>,
    iteration: usize,
    /// Wall pressure gradient used in the previous iteration at every boundary face (SIMPLEC wall treatment of
    /// `wall_pressure_trapezoid`).
    wall_grad: Vec<f64>,
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
    /// Boundary-face pressures for the cell Green-Gauss gradients. By default the values of `pbc`. With
    /// `wall_pressure_trapezoid` the value on a fixed-velocity wall face is chosen so that the wall-normal
    /// component of the cell gradient equals the cell-centre body force exactly (the gradient of the
    /// linearly interpolated opposite faces is O(h) off for a curved pressure otherwise).
    fn gradient_wall_values(&self, pbc: &PressureBc, body: Option<&BodyForce>, p: &[f64]) -> Vec<f64> {
        let g = &self.geom;
        let mut out = boundary_values_pbc(g, pbc, p);
        let (Some(bf), true) = (body, self.wall_pressure_trapezoid) else {
            return out;
        };
        let mesh = &g.mesh;
        let (sf, own, nei, vol) = (
            mesh.face_area_vectors(),
            mesh.owner(),
            mesh.neighbour(),
            mesh.cell_volumes(),
        );
        let base = out.clone();
        for i in 0..out.len() {
            if !(pbc.fixed_u[i] && pbc.coupled[i].is_none()) {
                continue;
            }
            let f = g.n_internal + i;
            let c = own[f];
            let area = vec3::norm(sf[f]).max(1e-300);
            let nhat = vec3::scale(sf[f], 1.0 / area);
            let mut others = 0.0;
            for &fo in mesh.cell_faces(c) {
                if fo == f || g.empty[fo] {
                    continue;
                }
                let (pf, sign) = if fo < g.n_internal {
                    let w = g.weights[fo];
                    (
                        w * p[own[fo]] + (1.0 - w) * p[nei[fo]],
                        if own[fo] == c { 1.0 } else { -1.0 },
                    )
                } else {
                    (base[fo - g.n_internal], 1.0)
                };
                others += pf * sign * vec3::dot(sf[fo], nhat);
            }
            out[i] = (vol[c] * vec3::dot(bf.cell[c], nhat) - others) / area;
        }
        out
    }

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
            face_jump: None,
            impermeable: None,
            jump_state: Vec::new(),
            pseudo: None,
            pseudo_control: None,
            pseudo_scale_history: Vec::new(),
            residual_norm: ResidualNorm::Reference,
            record_parts: false,
            parts_history: Vec::new(),
            pressure_parts_history: Vec::new(),
            driver: None,
            wall_pressure_trapezoid: false,
            coupled_body_pair_mean: false,
            conv_flux_offset: None,
            component_diag_correction: false,
            iteration: 0,
            wall_grad: Vec::new(),
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
        let scale = self.pseudo_control.map_or(1.0, |c| c.scale);
        Some(dt.into_iter().map(|d| d * ps.factor * scale).collect())
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
        let mut wall_prev = self.wall_grad.clone();
        if let Some(bf) = &body {
            // Where the normal velocity is prescribed the flux is zero, so the pressure
            // gradient balances the body force there: extrapolate the wall pressure with
            // that gradient (the fixed-flux pressure condition) so that the cell-centred
            // gradient matches the cell-centred body force next to walls.
            for i in 0..pbc.coeffs.len() {
                if pbc.fixed_u[i] && pbc.coupled[i].is_none() {
                    let f = n_int + i;
                    let ratio = self.body_ratio.get(mesh.owner()[f]).copied().unwrap_or(1.0);
                    let mut gradient = ratio * bf.face[f] / vec3::norm(sf[f]).max(1e-300);
                    if self.wall_pressure_trapezoid && s.coupling == Coupling::Simplec {
                        // SIMPLEC: the interior faces converge to dp/dn = f_f (the (rAU - rAtU) grad p_old term of
                        // the flux); apply the same lagged correction at walls so that the wall gradient also
                        // converges to f_f while the wall flux stays zero
                        if wall_prev.len() != pbc.coeffs.len() {
                            wall_prev = (0..pbc.coeffs.len())
                                .map(|j| bf.face[n_int + j] / vec3::norm(sf[n_int + j]).max(1e-300))
                                .collect();
                        }
                        let f_n = bf.face[f] / vec3::norm(sf[f]).max(1e-300);
                        gradient = ratio * f_n - (ratio - 1.0) * wall_prev[i];
                    }
                    pbc.coeffs[i] = crate::bc::FaceCoeffs {
                        a: 1.0,
                        b: g.dn[i] * gradient,
                        c: 0.0,
                        d: gradient,
                    };
                }
            }
        }
        let wall_used: Vec<f64> = pbc.coeffs.iter().map(|c| c.d).collect();
        // wall pressure used by the cell gradients only: with the trapezoid option the wall gradient is the mean
        // of the cell-centre and face force (the matrix and the flux keep the face value, so mass stays balanced)
        let p_bvals_for_grad = self.gradient_wall_values(&pbc, body.as_ref(), &self.p.values);
        let mut grad_p = green_gauss(&g, &self.p.values, &p_bvals_for_grad, 0);
        if let Some(j) = &self.face_jump {
            if self.jump_state.len() != j.rise.len() {
                self.jump_state = vec![0.0; j.rise.len()];
            }
            let fix = j.gradient_fix(&g, &self.jump_state);
            for (a, b) in grad_p.iter_mut().zip(&fix) {
                *a = vec3::add(*a, *b);
            }
        }
        let mut a_p = vec![0.0; n];
        let mut h = vec![[0.0; 3]; n];
        let mut h1 = vec![0.0; n];
        let mut n_active = 0.0;
        let mut res_u: f64 = 0.0;
        let mut parts_now: Vec<ResidualParts> = Vec::new();
        let mut scaled_parts: Vec<ResidualParts> = Vec::new();
        let mut comp_diag: Vec<Vec<f64>> = vec![Vec::new(); 3];
        let mut comp_x: Vec<Vec<f64>> = vec![Vec::new(); 3];
        let ub = self.u.boundary_values(&g);
        let phi_conv: Vec<f64> = match &self.conv_flux_offset {
            Some(o) => self.phi.iter().zip(o).map(|(p, o)| p - o).collect(),
            None => self.phi.clone(),
        };
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
            let mut m = convection(&g, &phi_conv, &cells, &bc, s.convection, grad.as_deref());
            lag_boundary_inflow(&g, &phi_conv, &cells, &bc, &mut m);
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
            if self.component_diag_correction {
                comp_diag[k] = m.a.diag.clone();
            }
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
            let r_k = if self.record_parts || self.residual_norm != ResidualNorm::Reference {
                let parts = residual_parts(&sys, &cells);
                if self.record_parts {
                    parts_now.push(parts);
                }
                scaled_parts.push(parts);
                match self.residual_norm {
                    ResidualNorm::Reference => normalised_residual(&sys, &cells),
                    ResidualNorm::Backward => parts.res_bwd(),
                    ResidualNorm::Scaled => 0.0,
                }
            } else {
                normalised_residual(&sys, &cells)
            };
            res_u = res_u.max(r_k);
            let mut x = cells;
            sys.solve(&mut x, &s.momentum_solver);
            if s.h_from_solution && self.driver.is_none() {
                // classic SIMPLE: neighbour contributions from the solved predictor instead of the old iterate
                hk = m.a.h_operator(&x, &m.b);
            }
            if self.component_diag_correction {
                comp_x[k] = if s.h_from_solution && self.driver.is_none() {
                    x.clone()
                } else {
                    u_old.iter().map(|v| v[k]).collect()
                };
            }
            for c in 0..n {
                h[c][k] = hk[c];
                self.u.values[c][k] = x[c];
            }
        }
        if self.residual_norm == ResidualNorm::Scaled {
            let s_max = scaled_parts.iter().map(|p| p.scale).fold(0.0, f64::max);
            let n_max = scaled_parts.iter().map(|p| p.num).fold(0.0, f64::max);
            res_u = n_max / (s_max + 1e-30);
        }
        for c in 0..n {
            a_p[c] /= n_active;
            h1[c] /= n_active;
        }
        if self.component_diag_correction {
            for c in 0..n {
                let ks: Vec<usize> = (0..3).filter(|&k| g.active[k]).collect();
                if ks
                    .iter()
                    .any(|&k| comp_diag[k][c].to_bits() != comp_diag[ks[0]][c].to_bits())
                {
                    for &k in &ks {
                        h[c][k] += (a_p[c] - comp_diag[k][c]) * comp_x[k][c];
                    }
                }
            }
        }

        // ---- pressure equation --------------------------------------------
        let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
        let r_atu: Vec<f64> = match s.coupling {
            Coupling::Simple => r_au.clone(),
            Coupling::Simplec => (0..n).map(|c| vol[c] / (a_p[c] - h1[c]).max(1e-3 * a_p[c])).collect(),
        };
        self.body_ratio = (0..n).map(|c| r_au[c] / r_atu[c]).collect();
        if self.wall_pressure_trapezoid && s.coupling == Coupling::Simplec && body.is_some() {
            self.wall_grad = wall_used.clone();
        }
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
        let mut gamma_p = face_interpolate(&g, &r_atu);
        // velocity that carries the boundary mass flux: the prescribed one, or its tangential part on impermeable faces
        let ub_mass_store: Vec<Vec3>;
        let ub_mass: &[Vec3] = match &self.impermeable {
            None => &ub,
            Some(mask) => {
                let sfb: Vec<Vec3> = (0..ub.len()).map(|i| sf[n_int + i]).collect();
                let (proj, _) = crate::bc::tangential_projection(&ub, &sfb);
                ub_mass_store = ub
                    .iter()
                    .zip(proj)
                    .zip(mask)
                    .map(|((u, p), &m)| if m { p } else { *u })
                    .collect();
                &ub_mass_store
            }
        };
        // HbyA at boundary faces: prescribed velocity or owner value
        let hb = boundary_hby_a(&g, &pbc, ub_mass, &hby_a0);
        let mut phi_hby_a = face_flux(&g, &hby_a0, &hb);
        if let Some(bf) = &body {
            let mut r_au_f = face_interpolate(&g, &r_au);
            if self.coupled_body_pair_mean {
                for (i, cp) in pbc.coupled.iter().enumerate() {
                    if let Some(gf) = *cp {
                        let (dp, dq) = (g.dn[i], g.dn[gf - n_int]);
                        let w = dq / (dp + dq);
                        r_au_f[n_int + i] = w * r_au[mesh.owner()[n_int + i]] + (1.0 - w) * r_au[mesh.owner()[gf]];
                    }
                }
            }
            for f in 0..mesh.n_faces() {
                if !g.empty[f] {
                    phi_hby_a[f] += r_au_f[f] * bf.face[f];
                }
            }
        }
        if body.is_some() && self.wall_pressure_trapezoid && s.coupling == Coupling::Simplec && !wall_prev.is_empty() {
            for i in 0..pbc.coeffs.len() {
                if pbc.fixed_u[i] && pbc.coupled[i].is_none() {
                    let f = n_int + i;
                    let c = mesh.owner()[f];
                    phi_hby_a[f] -= (r_au[c] - r_atu[c]) * wall_prev[i] * vec3::norm(sf[f]);
                }
            }
        }
        if s.coupling == Coupling::Simplec {
            let corr = self.simplec_flux_correction(&r_au, &r_atu, &self.p.values);
            for f in 0..n_int {
                phi_hby_a[f] += corr[f];
            }
            if s.simplec_non_orth_compensation && s.non_orth_correctors > 0 {
                let corr = self.simplec_non_orth_flux_correction(&r_au, &r_atu, &pbc);
                for f in 0..n_int {
                    phi_hby_a[f] += corr[f];
                }
            }
        }
        if let Some(j) = &self.face_jump {
            // jump faces: phi (1 + G K) = phiHbyA + G R_explicit - G snGrad(p) with G = gamma_p lap, the same face
            // coefficient the flux correction uses (the jump is then exactly the removed part of the pressure)
            for f in 0..n_int {
                if g.empty[f] || (j.rise[f] == 0.0 && j.resist[f] == 0.0) {
                    continue;
                }
                let gu = gamma_p[f] * g.lap_coeff[f];
                let den = 1.0 + gu * j.resist[f];
                phi_hby_a[f] = (phi_hby_a[f] + gu * j.rise[f]) / den;
                gamma_p[f] /= den;
            }
        }
        let mut p_new = self.p.values.clone();
        let p_old = self.p.values.clone();
        let controls = PressureControls {
            p_ref: s.p_ref,
            non_orth_correctors: s.non_orth_correctors,
            solver: s.pressure_solver,
            amg: false,
            linear: s.linear,
        };
        let (res_p, p_iters, p_parts) = solve_pressure_parts(
            &g,
            &gamma_p,
            &pbc,
            &phi_hby_a,
            &mut p_new,
            &controls,
            self.record_parts || self.residual_norm == ResidualNorm::Scaled,
        );
        let res_p = match (self.residual_norm, p_parts) {
            (ResidualNorm::Scaled, Some(pp)) => {
                let t: f64 = (0..mesh.n_faces())
                    .filter(|&f| !g.empty[f])
                    .map(|f| phi_hby_a[f].abs())
                    .sum::<f64>()
                    * 0.5;
                pp.num / (pp.scale.max(t) + 1e-30)
            }
            _ => res_p,
        };
        if self.record_parts {
            self.pressure_parts_history.push(p_parts);
        }
        // flux correction: phi = phiHbyA - rAtU_f snGrad(p) |S|
        let bv_new = self.gradient_wall_values(&pbc, body.as_ref(), &p_new);
        self.phi = corrected_flux(
            &g,
            &gamma_p,
            &pbc,
            &phi_hby_a,
            &p_new,
            ub_mass,
            s.non_orth_correctors > 0,
        );

        // under-relax pressure, correct velocity
        for c in 0..n {
            self.p.values[c] = p_old[c] + s.alpha_p * (p_new[c] - p_old[c]);
        }
        // velocity correction uses the new (unrelaxed-consistent) pressure gradient
        let mut grad_corr = green_gauss(&g, &p_new, &bv_new, 0);
        if let Some(j) = &self.face_jump {
            let r_new = j.totals(&self.phi);
            let fix = j.gradient_fix(&g, &r_new);
            for (st, r) in self.jump_state.iter_mut().zip(&r_new) {
                *st += s.alpha_p * (r - *st);
            }
            for (a, b) in grad_corr.iter_mut().zip(&fix) {
                *a = vec3::add(*a, *b);
            }
        }
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
        if self.record_parts {
            self.parts_history.push(parts_now);
        }
        if let Some(c) = self.pseudo_control.as_mut() {
            self.pseudo_scale_history.push(c.scale);
            c.update(res_u.max(res_p));
        }
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
            // a pressure jump plane (face-jump hook) is not part of the smooth difference
            let jump = if self.face_jump.is_some() {
                self.jump_state.get(f).copied().unwrap_or(0.0)
            } else {
                0.0
            };
            out[f] = dr * g.lap_coeff[f] * (p[nb] - p[o] - jump);
        }
        out
    }

    /// Non-orthogonal part of the SIMPLEC flux compensation, `(rAtU - rAU)_f k . grad(p_old)_f` with the least-squares cell
    /// gradient and boundary values of the pressure corrector (ADR 0050 D1); internal faces only.
    fn simplec_non_orth_flux_correction(&self, r_au: &[f64], r_atu: &[f64], pbc: &PressureBc) -> Vec<f64> {
        let g = &self.geom;
        let mesh = &g.mesh;
        let (own, nei) = (mesh.owner(), mesh.neighbour());
        let sf = mesh.face_area_vectors();
        let bv = boundary_values_pbc(g, pbc, &self.p.values);
        let gr = least_squares(g, &self.p.values, &bv);
        let mut out = vec![0.0; mesh.n_faces()];
        for f in 0..g.n_internal {
            let d = g.delta[f];
            let delta = vec3::scale(d, vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], d));
            let k = vec3::sub(sf[f], delta);
            let w = g.weights[f];
            let gf = [0, 1, 2].map(|i| w * gr[own[f]][i] + (1.0 - w) * gr[nei[f]][i]);
            let dr = w * (r_atu[own[f]] - r_au[own[f]]) + (1.0 - w) * (r_atu[nei[f]] - r_au[nei[f]]);
            out[f] = dr * vec3::dot(k, gf);
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
