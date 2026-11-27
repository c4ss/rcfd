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
use crate::diffusion::{laplacian_opt, uniform_gamma, GradientOptions};
use crate::field::{ScalarField, VectorField};
use crate::fvmatrix::FvMatrix;
use crate::geometry::FvGeometry;
use crate::gradient::{green_gauss, least_squares};
use crate::lumped::FaceJump;
use crate::pressure::{
    apply_body_force_wall_gradient, boundary_hby_a, boundary_values_pbc, corrected_flux, coupled_gamma, divergence,
    face_interpolate, pressure_bc, solve_pressure, CoupledNonOrth, PressureBc, PressureControls,
};
use crate::simple::{normalised_residual, BodyForce};
use rcfd_core::vec3::{self, Vec3};
use rcfd_linalg::{LinearOptions, SolveControl};
use std::sync::Arc;

pub(crate) mod face_consistent;
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
    /// Use the distance-weighted pair mean of `rAU` (the coefficient of the pressure term) for the body-force term of the face flux on
    /// periodic faces, as the steady `SimpleSolver::coupled_body_pair_mean` (ADR 0080). The default (owner value) leaves
    /// `(rAU_o - rAU_q) f.S` per seam pair, a seam mass source wherever `rAU` differs across the pair. Default `false` (bit-identical).
    pub coupled_body_pair_mean: bool,
    /// Adaptive pressure-corrector count (ADR 0156): `None` (default) = exactly `n_correctors` correctors per outer iteration;
    /// `Some` = at least `n_correctors`, raised until the estimated pressure-lag amplification of the step is at most the
    /// control's target. Not used by the fractional step or moving meshes.
    pub corrector_control: Option<CorrectorControl>,
    /// Slope limiter of the `linear_upwind` face extrapolation of the momentum convection (ADR 0183 P-A, as the steady
    /// `SimpleSolver::linear_upwind_limiter` of ADR 0081 part L): the least-squares gradient of each velocity component is
    /// multiplied by its limiter factor in the convection term only, also on the moving-mesh step. `None` (default) =
    /// unlimited. Only with `ConvScheme::LinearUpwind`; not used by the fractional step.
    pub linear_upwind_limiter: Option<crate::gradient::Limiter>,
    /// Gradient options of the explicit non-orthogonal flux of the momentum Laplacian (ADR 0204, case value
    /// `solver.gradient_equations.momentum`), also on the moving-mesh step; `None` (default) assembles the plain Laplacian. Not used by
    /// the fractional step.
    pub gradient_momentum: Option<GradientOptions>,
    /// Gradient options of the explicit non-orthogonal flux of every corrector's pressure equation and of the internal faces of its
    /// flux correction (ADR 0204, `solver.gradient_equations.pressure`); `None` (default) leaves them unchanged. They act only with
    /// `non_orth_correctors > 0`; the pressure-lag estimate keeps the plain pressure operator (ADR 0204 Q4). Not used by the
    /// fractional step.
    pub gradient_pressure: Option<GradientOptions>,
    /// Consistent (SIMPLEC-type) pressure correctors (ADR 0214 F0): every corrector, with `p_prev` the pressure that drove it,
    /// uses `rAtU = V/(a_P - H1)` (`H1` the component-averaged off-diagonal magnitudes) in the pressure equation, the flux
    /// correction and the cell velocity, the cell term `-(rAU - rAtU) grad(p_prev)` and the face compensation
    /// `(rAtU - rAU)_f snGrad(p_prev) |S|`, so that a converged state is that of the default. It removes the over-correction of a
    /// pressure error by about the cell diffusion number that makes two correctors unstable across thin cells (ADR 0214 M1, M2).
    /// Also on the moving-mesh step and in the pressure-lag estimate. Refused (the step stops through the divergence guard) with
    /// the fractional step, non-orthogonal correctors, the component diagonal, a face jump, periodic faces unless
    /// `consistent_periodic`, and a body force unless `consistent_body_force` (ADR 0223). Default `false` (bit-identical).
    pub consistent_correctors: bool,
    /// With `consistent_correctors`: accept periodic (coupled) pressure faces (ADR 0223 E2). Each coupled face gets the compensation
    /// `coupled_gamma(interp(rAtU - rAU))_f |S| (p_prev,q - p_prev,o) / span` of the steady SIMPLEC (ADR 0102), and the implicit
    /// periodic link coefficients of the momentum matrix count in `H1` of their row (ADR 0113 F1), so that a seam cell has the
    /// `rAtU` of the same cell of the unsplit mesh. Default `false`: periodic faces are refused as in ADR 0214.
    pub consistent_periodic: bool,
    /// With `consistent_correctors`: accept body forces (`body_force` and the hook's `source_force`; ADR 0223 E3). The body-force
    /// term of the face flux keeps `interp(rAU)` except on fixed-velocity, non-periodic boundary faces, where it takes `interp(rAtU)`,
    /// the coefficient of the pressure equation's Neumann term there; the cell velocity keeps `rAU b`. Default `false`: body forces
    /// are refused as in ADR 0214. Face jumps stay refused.
    pub consistent_body_force: bool,
    /// The consistent-corrector flags as one value (ADR 0258 V1): `enabled`, `periodic` and `body_force` are read together with
    /// the three fields above (either sets the flag, see [`PimpleSettings::consistent_flags`]); the flags of the further
    /// combinations (`non_orthogonal`, `component_diagonal`, `face_jump`) exist only here. Default: every flag `false`
    /// (bit-identical). Set it with [`PimpleSettings::set_consistent`], which also writes the three fields above.
    pub consistent: ConsistentCorrectors,
    /// With the consistent correctors: accept the combinations of `ConsistentRefusal::PERIODIC_EXTENSIONS` (non-orthogonal
    /// correctors, the component diagonal, face jumps, each with periodic pressure faces), whose periodic steady identity is met
    /// (ADR 0360). They need no term beyond the extension's own and E2's. Default `false`: refused as in ADR 0258.
    pub consistent_periodic_extensions: bool,
    /// Make the pressure-lag check (`pressure_lag_check`) and the adaptive corrector count (`corrector_control`) also on a
    /// moving-mesh step (`step_ale`, `step_ale_with_walls`, `step_sliding`; ADR 0223 E0), on that step's own frozen matrices (the
    /// moved geometry, the relative flux, the step's wall conditions made homogeneous), exactly as on a fixed mesh. Default `false`:
    /// a moving-mesh step makes neither (as before).
    pub moving_mesh_lag: bool,
    /// The face coefficient of the pressure term that the transient Rhie-Chow carry-over is consistent with (ADR 0245 C): the
    /// harmonic face value of `rAU` and the commutator of the lagged pressure gradient on internal faces, so that the converged
    /// face flux depends on the step size only through the time scheme. Acts in `step` on a mesh at rest, with the consistent
    /// correctors (their compensation takes `interp(rAtU) - Gamma` face by face, ADR 0352 D1), on `step_ale` and
    /// `step_ale_with_walls` (ADR 0352 D2) and in `OversetSolver`; `step_sliding` and the fractional step keep the default step
    /// (in the fractional step the face coefficient is uniform and the option would change nothing, ADR 0352 D5); coupled
    /// faces, face jumps and the immersed-solid flux are left as they are. Default `false` (bit-identical). For the fractional
    /// step see [`PimpleSettings::fractional_step_consistent`].
    pub rhie_chow_step_consistent: bool,
    /// The fractional step's intermediate flux with a carried face defect (ADR 0356): on internal faces
    /// `interp(u*) . S + Gamma R(p^n) + (Gamma / dt)(c1 d^n - c2 d^{n-1})` in place of `interp(u*) . S + dte R(p^n)`, with
    /// `Gamma` the harmonic face value of `V / a_P` of the predictor and `d = phi - interp(u) . S`, so that a steady state has
    /// the face defect `R(p) / interp(a)` with no step size in it. Acts in the incremental form only (the Chorin form carries
    /// the step in its cell equations, ADR 0356 D3); the Poisson solve, the projection and boundary faces are unchanged. Not
    /// read by the PISO/PIMPLE step. Default `false` (bit-identical).
    pub fractional_step_consistent: bool,
    /// F-466 (ADR 0363 amendment 1): a pressure solve of the plain pressure path (`gradient_pressure = None`) that returns a
    /// non-finite pressure is repeated from the same start pressure with incomplete-Cholesky CG (`amg_pressure` off for that
    /// solve); each repeat is recorded in `PimpleSolver::pressure_fallbacks`. Acts only on a non-finite pressure, so a run
    /// whose solves are finite is unchanged. Default `false` (bit-identical).
    pub pressure_ic_fallback: bool,
    /// F-536 (ADR 0458 C1): end the outer loop of a step by a criterion, with `n_outer` as the cap. After outer iteration `k` the
    /// ratio `c_k = max |p_k - p_(k-1)| / max |p_1 - p^n|` (the pressure change of the iteration over that of the step's first
    /// one, `p^n` the pressure at the start of the step) is formed; once `c_k <= tol`, iteration `k + 1` is run as the final one
    /// (unrelaxed, the checks of the final iteration on its matrices) and the loop ends after it. For a linear contraction `r`
    /// per outer iteration this leaves `r tol` of the error of the non-iterated step. Every step is recorded in
    /// `PimpleSolver::outer_record`. A negative or NaN tolerance is never met. The pressure-lag estimate of the step runs the
    /// outer iterations the step used (F-550, ADR 0617 S3). Default `None`: exactly `n_outer` outer iterations (bit-identical).
    pub outer_pressure_tolerance: Option<f64>,
    /// F-550 (ADR 0617 S2): with `outer_pressure_tolerance`, form the ratio against the largest first-iteration pressure change
    /// `max |p_1 - p^n|` of the step and the `w - 1` completed steps before it (`w = max(w, 1)`) instead of the step's own, so the
    /// scale does not vanish at the extremes of a periodic pressure history (floor `sin(pi (w - 1)/N)` of the period's largest
    /// for `N` steps per period). `Some(1)` is ADR 0458's scale. Read only with the criterion. Default `None`: ADR 0458's scale.
    pub outer_scale_window: Option<usize>,
    /// F-885 (ADR 0767): keep, for every outer iteration `k` of a step, the largest velocity change
    /// `d_k = max_c |u_k(c) - u_(k-1)(c)|` (Euclidean norm of the cell velocity difference; `u_0` the velocity at the start of
    /// the step's first outer iteration, `u_k` the iterate after the correctors of iteration `k`) in
    /// `PimpleSolver::outer_velocity`. Read-only: the copies and the norms enter no equation. Not read by the fractional step
    /// or the moving-mesh steps. Default `false`: no copy is made and nothing is kept (bit-identical).
    pub outer_velocity_record: bool,
    /// F-885 (ADR 0811): a face flux whose converged value does not depend on `alpha_u`. In every outer iteration with
    /// `alpha_u < 1` (never the final one) each corrector adds `(1 - alpha_u) (phi* - F(u*))` to `phiHbyA` on every face
    /// that is neither empty nor a fixed-velocity boundary face, with `u*` and `phi*` the iterate and the flux at the start of
    /// the outer iteration and `F` the face flux of a cell field as the correctors form it (N. Kuhl, arXiv:2607.21736, Eq. 25).
    /// At a fixed point the relaxed iterations then have the face flux of the unrelaxed one. Refused (panics) with the
    /// consistent correctors, a face jump, an immersed solid or a moving mesh; not read by the fractional step or the
    /// moving-mesh steps. Default `false`: no term is formed (bit-identical); with `alpha_u = 1` it is not formed either.
    pub alpha_free_face_flux: bool,
    /// F-885 (ADR 0815, a control of the outer loop): in the outer iterations after the first of a step, the hook's
    /// `source_force` is not taken; the one of the step's first outer iteration stays. Read only with a hook. Default `false`
    /// (bit-identical).
    pub outer_freeze_source: bool,
    /// F-885 (ADR 0815, a control of the outer loop): in the outer iterations after the first of a step, the hook's `nu_face`
    /// is not taken; the face viscosity of the step's first outer iteration stays (the hook still runs). Read only with a
    /// hook. Default `false` (bit-identical).
    pub outer_freeze_viscosity: bool,
    /// F-885 (ADR 0816, a record of the outer loop): for each cell named here, keep in `PimpleSolver::outer_probe`, per outer
    /// iteration, the momentum diagonal of each component before the relaxation, the sum of the off-diagonal magnitudes the
    /// relaxation takes (`d = max(a_c, sum |a_nb|)`, with the periodic and rotational link magnitudes as the relaxation counts
    /// them), the Barth-Jespersen factor of each component at the cell and its face neighbours, the convecting flux on each
    /// face of the cell (outward) and the cell's velocity after the correctors. Read-only: the copies enter no equation. Not
    /// read by the fractional step or the moving-mesh steps. Default empty: nothing is computed or kept (bit-identical).
    pub outer_cell_probe: Vec<usize>,
    /// F-885 (ADR 0819, opt-in): with `linear_upwind_limiter` and linear upwind convection, keep each component's limiter
    /// factor of the step's first outer iteration and use it in the step's later outer iterations (the final one included)
    /// in place of a factor formed anew; the least-squares gradient is still formed from each iteration's values. The kept
    /// factors are reset at every step. Without the limiter nothing is kept. Not read by the fractional step or the
    /// moving-mesh steps. Default `false` (bit-identical).
    pub outer_frozen_limiter: bool,
    /// F-885 (ADR 0820, opt-in): keep the momentum predictor's convecting flux `phi_conv` of the step's first outer
    /// iteration (after the BDF2 extrapolation and the moving-mesh map) and use it in the step's later outer iterations (the
    /// final one included): in the convection, the periodic and rotational links and the probe. Not the correctors' flux,
    /// not ADR 0811's term. Reset at every step. Not read by the fractional step. Default `false` (bit-identical).
    pub outer_frozen_convecting_flux: bool,
    /// F-885 (ADR 0820, opt-in): keep each component's least-squares gradient that enters the convection (its deferred
    /// correction, and the limiter factor where one is formed) in the step's first outer iteration and use it in the later
    /// ones. Read only where a gradient is formed. Reset at every step. Default `false` (bit-identical).
    pub outer_frozen_convection_gradient: bool,
    /// F-885 (ADR 0824, opt-in): with `outer_frozen_convection_gradient`, on the step's first outer iteration where the cells
    /// of the convection are extrapolated in time (BDF2), form the convection gradient's boundary values from the
    /// extrapolated field (a copy of the velocity with the extrapolated cells and the same conditions), so that the kept
    /// gradient is of one time level. The convection's coefficients, the Laplacian's gradient and the limiter keep theirs.
    /// Default `false` (bit-identical).
    pub outer_frozen_convection_gradient_extrapolated_boundary: bool,
    /// F-885 (ADR 0824, opt-in): with `outer_frozen_convection_gradient`, keep the convection gradient of the step's second
    /// outer iteration (formed from its iterate) for the later ones; the first iteration uses the gradient it forms. With one
    /// outer iteration nothing is kept. Default `false` (bit-identical).
    pub outer_frozen_convection_gradient_second: bool,
    /// F-885 (ADR 0820, opt-in): keep each component's gradient that enters the momentum Laplacian's non-orthogonal
    /// correction in the step's first outer iteration and use it in the later ones. Read only where a gradient is formed and
    /// `coupled_non_orth` is off. Reset at every step. Default `false` (bit-identical).
    pub outer_frozen_laplacian_gradient: bool,
    /// F-885 (ADR 0820, a record of the outer loop): for each outer iteration named here (1-based), push to
    /// `PimpleSolver::outer_fields` the velocity after the iteration's correctors, the predictor's convecting flux and the
    /// gradients that entered the convection and the Laplacian. Read-only: the copies enter no equation. Default empty
    /// (bit-identical).
    pub outer_field_record: Vec<usize>,
    /// F-535 (ADR 0471 D1): refuse the first step the solver takes, through the divergence guard and before any field is
    /// touched, when the pressure-lag estimate of ADR 0073 for the step's settings (`lag_preflight` with this many power
    /// iterations, at least 2) is at least one. The message names the largest cell diffusion number `nu dt/dx^2`
    /// (`PimpleSolver::max_cell_diffusion`), the estimate and the remedies. Not applied on a moving mesh or to the fractional
    /// step. Default `None` (bit-identical).
    pub lag_onset_refusal: Option<usize>,
    /// F-743 (ADR 0664): make the rotated partner of the rotationally periodic pairs implicit in the momentum predictor, as
    /// ADR 0046 D1 does for translational pairs. The partner term `coef (R u_partner)_k` of every rotational pair goes into
    /// one block system of the active velocity components (entries `coef R_kj`), its lagged value is removed from the
    /// right-hand side, and the three components are assembled from the same iterate and solved together (BiCGStab with
    /// ILU(0), `momentum_solver`); the correctors' `H` operator, the relaxation and the consistent correctors' `H1` count the
    /// links. Acts only where a rotational link exists (no-op otherwise); the pressure-lag estimate and the adaptive corrector
    /// count keep the lagged coupling. Default `false` (bit-identical).
    pub rotational_periodic_implicit: bool,
    /// F-744 (ADR 0667): the explicit non-orthogonal correction on the coupled (periodic) faces, in the pressure equation, its
    /// flux and the momentum diffusion. Each pair is treated as the internal face between the owner and the partner cell's
    /// image in the owner's frame (`pressure::CoupledNonOrth`): the orthogonal part is unchanged and the remainder `k . grad_f`
    /// is added where the internal faces add theirs (with `non_orth_correctors > 0` in the pressure, with a momentum gradient in
    /// the momentum diffusion); the cell gradients of the correction (also those of the internal faces' correction) are the
    /// least-squares gradients with the partner cell as the point of every coupled face (ADR 0077 F-H0). Refused (panics) with
    /// `gradient_pressure`, `gradient_momentum`, an immersed volume source or the consistent correctors with non-orthogonal
    /// correctors. Not used by the fractional step, the pressure-lag estimate or the adaptive corrector count. Default `false`
    /// (bit-identical).
    pub coupled_non_orth: bool,
    /// F-752 (ADR 0672): with `coupled_non_orth`, leave out its pressure part: the pressure equation and its flux take the
    /// default path (the orthogonal coupling on the coupled faces, the face-value least-squares gradient), the momentum part
    /// stays. A control for the effect of the pressure part. Read only with `coupled_non_orth`. Default `false`
    /// (bit-identical).
    pub coupled_non_orth_without_pressure: bool,
    /// F-754 (ADR 0675): with `ddt_correction`, give the coupled (periodic) faces the transient Rhie-Chow term the internal
    /// faces carry, the partner cell in the place of the neighbour (its old velocity rotated into the owner's frame on a
    /// rotational pair) and the pair mean of `rAU`. Not read on a moving mesh. In the fractional step (F-757, ADR 0687) it is
    /// read with `fractional_step_consistent` in the incremental form (the fractional step's ddt correction; `ddt_correction`
    /// is not read there): the coupled faces then take `Gamma R(p^n) + (Gamma / dt) D` with the pair-harmonic `Gamma` in place
    /// of `dte R(p^n)`. Default `false` (bit-identical).
    pub coupled_ddt_correction: bool,
    /// F-757 (ADR 0687): in the fractional step's momentum predictor, make the partner of the translationally periodic faces
    /// implicit as the PISO step does (ADR 0046 D1, `PeriodicLinks`); rotational pairs keep the lagged partner. No-op without
    /// translational pairs and outside the fractional step. Default `false` (bit-identical).
    pub fractional_periodic_implicit: bool,
    /// F-757 (ADR 0687 amendment 1): in the PISO/PIMPLE step, on the first outer iteration of a BDF2 step (where the convected
    /// flux and the deferred-correction state are extrapolated in time), give the translationally periodic faces the partner
    /// value of the extrapolated field in the velocity's boundary coefficients, as the fractional step does (ADR 0043 D1), so
    /// that the coupled face's deferred central part is the internal face's; the implicit links remove that value. Rotational
    /// pairs keep the lagged partner. Not read without the extrapolation (implicit Euler, later outer iterations, the first
    /// step of BDF2) or by the fractional step. Default `false` (bit-identical).
    pub periodic_partner_extrapolated: bool,
    /// F-771 (ADR 0691): the fractional step reads `PimpleSolver::body_force` (not the hook's `source_force`; the fractional
    /// step runs no hook) in the three places of the PISO step: the walls' pressure gradient that balances it
    /// (`pressure::apply_body_force_wall_gradient`), the predictor's cell term `V b`, and the balanced-force face term of the
    /// intermediate flux, `c_f (b_f . S_f - B_f . S_f)` with `c_f` the coefficient of the face's pressure term (`dte`, or
    /// `Gamma` with `fractional_step_consistent` and on the coupled faces with `coupled_ddt_correction` as well) and `B_f` the
    /// cell values interpolated as the face interpolates the pressure gradient (no term on fixed-velocity faces). Incremental
    /// form only: with the Chorin form the step panics. Not read by the PISO/PIMPLE step (which always reads the body force)
    /// or without a body force. Default `false` (bit-identical).
    pub fractional_body_force: bool,
}

/// The outer iterations of one step with `PimpleSettings::outer_pressure_tolerance` (F-536, ADR 0458 C2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OuterRecord {
    /// Step number (1-based).
    pub step: usize,
    /// Outer iterations run.
    pub used: usize,
    /// Whether the criterion ended the loop before the cap `n_outer`.
    pub met: bool,
    /// The last ratio `c_k` formed (NaN when none was formed).
    pub ratio: f64,
    /// The step's first-iteration pressure change `max |p_1 - p^n|` (F-550, ADR 0617; NaN when no ratio was formed).
    pub first: f64,
    /// The denominator of the ratio: `first`, or with `outer_scale_window` the windowed maximum (F-550, ADR 0617 S2; NaN when
    /// no ratio was formed).
    pub scale: f64,
}

/// The velocity change of each outer iteration of one step with `PimpleSettings::outer_velocity_record` (F-885, ADR 0767).
#[derive(Debug, Clone, PartialEq)]
pub struct OuterVelocity {
    /// Step number (1-based).
    pub step: usize,
    /// `d_k = max_c |u_k(c) - u_(k-1)(c)|` for the outer iterations `k = 1 ..` the step ran, in that order.
    pub change: Vec<f64>,
    /// The cell of `d_k` for each outer iteration, in the same order (ADR 0810): the first cell of the largest change, cell 0
    /// when every change is zero.
    pub cell: Vec<usize>,
}

/// One outer iteration at a probe cell with `PimpleSettings::outer_cell_probe` (F-885, ADR 0816). NaN where a value is not
/// formed (an inactive component; the factor without a limiter).
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeIteration {
    /// The momentum diagonal `a_c` of each component before the relaxation (after the time term).
    pub diag: Vec3,
    /// The sum of the off-diagonal magnitudes the relaxation takes, per component.
    pub off_sum: Vec3,
    /// The Barth-Jespersen factor of each component: the cell first, then its face neighbours in the order of
    /// `OuterProbe::neighbours`.
    pub psi: Vec<Vec3>,
    /// The convecting flux of the predictor on each face of the cell, signed outward, in the order of `OuterProbe::faces`.
    pub flux: Vec<f64>,
    /// The cell's velocity after the iteration's correctors.
    pub u: Vec3,
}

/// The outer iterations of one step at one probe cell (F-885, ADR 0816).
#[derive(Debug, Clone, PartialEq)]
pub struct OuterProbe {
    /// Step number (1-based).
    pub step: usize,
    /// The probe cell.
    pub cell: usize,
    /// The faces of the cell, in face order.
    pub faces: Vec<usize>,
    /// The other cell of each internal face of the cell, in the order of `faces`.
    pub neighbours: Vec<usize>,
    /// The outer iterations of the step, in order.
    pub iterations: Vec<ProbeIteration>,
}

/// One recorded outer iteration with `PimpleSettings::outer_field_record` (F-885, ADR 0820).
#[derive(Debug, Clone, PartialEq)]
pub struct OuterField {
    /// Step number (1-based).
    pub step: usize,
    /// Outer iteration (1-based).
    pub iteration: usize,
    /// The velocity after the iteration's correctors.
    pub u: Vec<Vec3>,
    /// The convecting flux of the iteration's momentum predictor.
    pub phi_conv: Vec<f64>,
    /// Per component, the gradient that entered the convection (before a limiter factor); `None` where none was formed.
    pub grad_convection: [Option<Vec<Vec3>>; 3],
    /// Per component, the gradient that entered the Laplacian's non-orthogonal correction; `None` where none was formed.
    pub grad_laplacian: [Option<Vec<Vec3>>; 3],
}

/// The largest Euclidean norm over the cells of `b - a` and its first cell (F-885, ADR 0767; the cell ADR 0810).
fn max_velocity_change(a: &[Vec3], b: &[Vec3]) -> (f64, usize) {
    a.iter()
        .zip(b)
        .enumerate()
        .fold((0.0f64, 0usize), |(m, at), (c, (x, y))| {
            let (dx, dy, dz) = (y[0] - x[0], y[1] - x[1], y[2] - x[2]);
            let v = (dx * dx + dy * dy + dz * dz).sqrt();
            if v > m {
                (v, c)
            } else {
                (m, at)
            }
        })
}

/// Every flag of the consistent (SIMPLEC-type) pressure correctors in one value (ADR 0258 V1). `enabled` asks for the correctors
/// (ADR 0214 F0); every other flag accepts one combination the correctors are otherwise refused with. A flag without `enabled`
/// does nothing. [`ConsistentCorrectors::conflicts`] names the combinations that are not supported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConsistentCorrectors {
    /// Use the consistent correctors (as `PimpleSettings::consistent_correctors`, ADR 0214 F0).
    pub enabled: bool,
    /// Accept periodic (coupled) pressure faces (as `PimpleSettings::consistent_periodic`, ADR 0223 E2).
    pub periodic: bool,
    /// Accept body forces (as `PimpleSettings::consistent_body_force`, ADR 0223 E3).
    pub body_force: bool,
    /// Accept non-orthogonal correctors (`PimpleSettings::non_orth_correctors > 0`; ADR 0258 X1): the compensation of the old
    /// pressure also carries the explicit non-orthogonal part, `(rAtU - rAU)_f k_f . grad(p_prev)_f`, as the steady SIMPLEC of
    /// ADR 0050. Fixed meshes without pressure gradient options; with periodic faces only with
    /// `PimpleSettings::consistent_periodic_extensions` (ADR 0360).
    pub non_orthogonal: bool,
    /// Accept the component diagonal (`PimpleSettings::component_diagonal`; ADR 0258 X2); `rAtU` keeps the component averages.
    /// Fixed meshes; with periodic faces only with `PimpleSettings::consistent_periodic_extensions` (ADR 0360).
    pub component_diagonal: bool,
    /// Accept face jumps (`PimpleSolver::face_jump` and the hook's; ADR 0258 X3): a jump face compensates the old pressure
    /// difference less the old total rise and takes `interp(rAtU)` in its resistance factor. Fixed meshes; with periodic faces
    /// only with `PimpleSettings::consistent_periodic_extensions` (ADR 0360).
    pub face_jump: bool,
}

impl ConsistentCorrectors {
    /// The consistent correctors with every extension this library supports (ADR 0258).
    pub fn all() -> Self {
        ConsistentCorrectors {
            enabled: true,
            periodic: true,
            body_force: true,
            non_orthogonal: true,
            component_diagonal: true,
            face_jump: true,
        }
    }

    /// Every combination of these flags with the settings of `ctx` that the consistent correctors are not defined with, in the
    /// order of ADR 0258 V2 (empty without `enabled`).
    pub fn conflicts(&self, ctx: &ConsistentContext) -> Vec<ConsistentRefusal> {
        use ConsistentRefusal as R;
        let mut out = Vec::new();
        if !self.enabled {
            return out;
        }
        if ctx.fractional {
            out.push(R::FractionalStep);
        }
        if ctx.non_orth_correctors > 0 {
            if !self.non_orthogonal {
                out.push(R::NonOrthogonal);
            } else {
                if ctx.gradient_pressure {
                    out.push(R::NonOrthogonalGradientOptions);
                }
                if ctx.periodic_faces {
                    out.push(R::NonOrthogonalPeriodic);
                }
                if ctx.moving_mesh {
                    out.push(R::NonOrthogonalMovingMesh);
                }
            }
        }
        if ctx.component_diagonal {
            if !self.component_diagonal {
                out.push(R::ComponentDiagonal);
            } else {
                if ctx.periodic_faces {
                    out.push(R::ComponentDiagonalPeriodic);
                }
                if ctx.moving_mesh {
                    out.push(R::ComponentDiagonalMovingMesh);
                }
            }
        }
        if ctx.body_force && !self.body_force {
            out.push(R::BodyForce);
        }
        if ctx.face_jump {
            if !self.face_jump {
                out.push(R::FaceJump);
            } else {
                if ctx.periodic_faces {
                    out.push(R::FaceJumpPeriodic);
                }
                if ctx.moving_mesh {
                    out.push(R::FaceJumpMovingMesh);
                }
            }
        }
        if ctx.periodic_faces && !self.periodic {
            out.push(R::PeriodicFaces);
        }
        out
    }

    /// `Ok` when the flags are supported with the settings of `ctx`, else the first conflict of
    /// [`conflicts`](Self::conflicts).
    pub fn validate(&self, ctx: &ConsistentContext) -> Result<(), ConsistentRefusal> {
        match self.conflicts(ctx).first() {
            Some(r) => Err(*r),
            None => Ok(()),
        }
    }

    /// [`conflicts`](Self::conflicts) with, when `periodic_extensions` (`PimpleSettings::consistent_periodic_extensions`), the
    /// refusals of [`ConsistentRefusal::PERIODIC_EXTENSIONS`] removed, the order of the others kept (ADR 0360 D2). The solver
    /// refuses through it.
    pub fn conflicts_extended(&self, ctx: &ConsistentContext, periodic_extensions: bool) -> Vec<ConsistentRefusal> {
        let mut out = self.conflicts(ctx);
        if periodic_extensions {
            out.retain(|r| !ConsistentRefusal::PERIODIC_EXTENSIONS.contains(r));
        }
        out
    }

    /// `Ok` when the flags are supported with the settings of `ctx`, else the first conflict of
    /// [`conflicts_extended`](Self::conflicts_extended).
    pub fn validate_extended(
        &self,
        ctx: &ConsistentContext,
        periodic_extensions: bool,
    ) -> Result<(), ConsistentRefusal> {
        match self.conflicts_extended(ctx, periodic_extensions).first() {
            Some(r) => Err(*r),
            None => Ok(()),
        }
    }
}

/// The settings a set of consistent-corrector flags is validated against (ADR 0258 V2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConsistentContext {
    /// The fractional step is selected (`PimpleSettings::fractional`).
    pub fractional: bool,
    /// `PimpleSettings::non_orth_correctors`.
    pub non_orth_correctors: usize,
    /// Pressure gradient options are set (`PimpleSettings::gradient_pressure`).
    pub gradient_pressure: bool,
    /// `PimpleSettings::component_diagonal`.
    pub component_diagonal: bool,
    /// A body force acts (`PimpleSolver::body_force` or the hook's `source_force`).
    pub body_force: bool,
    /// A face jump acts (`PimpleSolver::face_jump` or the hook's).
    pub face_jump: bool,
    /// The pressure has periodic (coupled) faces.
    pub periodic_faces: bool,
    /// The step is taken on a moving mesh (ALE or sliding).
    pub moving_mesh: bool,
}

/// One combination the consistent correctors are not defined with (ADR 0258 V2): which flag, with which other setting, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsistentRefusal {
    /// The fractional step.
    FractionalStep,
    /// Non-orthogonal correctors without the flag `non_orthogonal`.
    NonOrthogonal,
    /// Non-orthogonal correctors with pressure gradient options.
    NonOrthogonalGradientOptions,
    /// Non-orthogonal correctors with periodic faces.
    NonOrthogonalPeriodic,
    /// Non-orthogonal correctors on a moving mesh.
    NonOrthogonalMovingMesh,
    /// The component diagonal without the flag `component_diagonal`.
    ComponentDiagonal,
    /// The component diagonal with periodic faces.
    ComponentDiagonalPeriodic,
    /// The component diagonal on a moving mesh.
    ComponentDiagonalMovingMesh,
    /// A body force without the flag `body_force`.
    BodyForce,
    /// A face jump without the flag `face_jump`.
    FaceJump,
    /// A face jump with periodic faces.
    FaceJumpPeriodic,
    /// A face jump on a moving mesh.
    FaceJumpMovingMesh,
    /// Periodic faces without the flag `periodic`.
    PeriodicFaces,
}

impl ConsistentRefusal {
    /// Every refusal, in the order of [`ConsistentCorrectors::conflicts`].
    pub const ALL: [ConsistentRefusal; 13] = [
        ConsistentRefusal::FractionalStep,
        ConsistentRefusal::NonOrthogonal,
        ConsistentRefusal::NonOrthogonalGradientOptions,
        ConsistentRefusal::NonOrthogonalPeriodic,
        ConsistentRefusal::NonOrthogonalMovingMesh,
        ConsistentRefusal::ComponentDiagonal,
        ConsistentRefusal::ComponentDiagonalPeriodic,
        ConsistentRefusal::ComponentDiagonalMovingMesh,
        ConsistentRefusal::BodyForce,
        ConsistentRefusal::FaceJump,
        ConsistentRefusal::FaceJumpPeriodic,
        ConsistentRefusal::FaceJumpMovingMesh,
        ConsistentRefusal::PeriodicFaces,
    ];

    /// The combinations with periodic faces whose periodic steady identity is met (ADR 0360), accepted with
    /// `PimpleSettings::consistent_periodic_extensions`.
    pub const PERIODIC_EXTENSIONS: &'static [ConsistentRefusal] = &[
        ConsistentRefusal::NonOrthogonalPeriodic,
        ConsistentRefusal::ComponentDiagonalPeriodic,
        ConsistentRefusal::FaceJumpPeriodic,
    ];

    /// The conflicting setting as the divergence record names it (`Divergence::stage`).
    pub fn stage(&self) -> &'static str {
        use ConsistentRefusal as R;
        match self {
            R::FractionalStep => "fractional step",
            R::NonOrthogonal => "non-orthogonal correctors",
            R::NonOrthogonalGradientOptions => "non-orthogonal correctors with pressure gradient options",
            R::NonOrthogonalPeriodic => "non-orthogonal correctors with periodic faces",
            R::NonOrthogonalMovingMesh => "non-orthogonal correctors on a moving mesh",
            R::ComponentDiagonal => "component diagonal",
            R::ComponentDiagonalPeriodic => "component diagonal with periodic faces",
            R::ComponentDiagonalMovingMesh => "component diagonal on a moving mesh",
            R::BodyForce => "body force",
            R::FaceJump => "face jump",
            R::FaceJumpPeriodic => "face jump with periodic faces",
            R::FaceJumpMovingMesh => "face jump on a moving mesh",
            R::PeriodicFaces => "periodic faces",
        }
    }

    /// The flag of [`ConsistentCorrectors`] the refusal belongs to: the one that is missing, or the one whose combination is
    /// not supported (`enabled` for the fractional step, which no flag allows).
    pub fn flag(&self) -> &'static str {
        use ConsistentRefusal as R;
        match self {
            R::FractionalStep => "enabled",
            R::NonOrthogonal
            | R::NonOrthogonalGradientOptions
            | R::NonOrthogonalPeriodic
            | R::NonOrthogonalMovingMesh => "non_orthogonal",
            R::ComponentDiagonal | R::ComponentDiagonalPeriodic | R::ComponentDiagonalMovingMesh => {
                "component_diagonal"
            }
            R::BodyForce => "body_force",
            R::FaceJump | R::FaceJumpPeriodic | R::FaceJumpMovingMesh => "face_jump",
            R::PeriodicFaces => "periodic",
        }
    }

    /// The other setting of the combination, as the library names it.
    pub fn setting(&self) -> &'static str {
        use ConsistentRefusal as R;
        match self {
            R::FractionalStep => "PimpleSettings::fractional",
            R::NonOrthogonal => "PimpleSettings::non_orth_correctors > 0",
            R::NonOrthogonalGradientOptions => "PimpleSettings::gradient_pressure",
            R::ComponentDiagonal => "PimpleSettings::component_diagonal",
            R::BodyForce => "PimpleSolver::body_force or the hook's source_force",
            R::FaceJump => "PimpleSolver::face_jump or the hook's face_jump",
            R::NonOrthogonalPeriodic | R::ComponentDiagonalPeriodic | R::FaceJumpPeriodic | R::PeriodicFaces => {
                "periodic pressure faces"
            }
            R::NonOrthogonalMovingMesh | R::ComponentDiagonalMovingMesh | R::FaceJumpMovingMesh => {
                "the moving-mesh step"
            }
        }
    }

    /// Why the combination is refused.
    pub fn reason(&self) -> &'static str {
        use ConsistentRefusal as R;
        match self {
            R::FractionalStep => "the fractional step has no pressure correctors",
            R::NonOrthogonal => "the flag is not set; it adds the non-orthogonal part of the compensation",
            R::NonOrthogonalGradientOptions => {
                "the non-orthogonal compensation is derived for the plain non-orthogonal flux only"
            }
            R::ComponentDiagonal => {
                "the flag is not set; it accepts the component diagonal with the component-averaged rAtU"
            }
            R::BodyForce => "the flag is not set; it accepts body forces",
            R::FaceJump => "the flag is not set; it adds the compensation of the jump faces",
            R::PeriodicFaces => "the flag is not set; it adds the compensation of the periodic faces",
            R::NonOrthogonalPeriodic | R::ComponentDiagonalPeriodic | R::FaceJumpPeriodic => {
                if Self::PERIODIC_EXTENSIONS.contains(self) {
                    "accepted only with PimpleSettings::consistent_periodic_extensions, its periodic steady identity is met \
                     (ADR 0360)"
                } else {
                    "its periodic steady identity was missed (ADR 0360)"
                }
            }
            R::NonOrthogonalMovingMesh | R::ComponentDiagonalMovingMesh | R::FaceJumpMovingMesh => {
                "the extension is not defined on a moving mesh (roadmap item F-321)"
            }
        }
    }

    /// The located part of the refusal: flag, other setting and reason.
    pub fn message(&self) -> String {
        format!(
            "ConsistentCorrectors::{} with {}: {}; ADR 0258",
            self.flag(),
            self.setting(),
            self.reason()
        )
    }

    /// The refusal a divergence record's stage names, if any.
    pub fn from_stage(stage: &str) -> Option<ConsistentRefusal> {
        Self::ALL.iter().copied().find(|r| r.stage() == stage)
    }
}

impl std::fmt::Display for ConsistentRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the consistent correctors (ADR 0214) are not defined with the {} ({})",
            self.stage(),
            self.message()
        )
    }
}

/// Adaptive corrector count of the PISO/PIMPLE loop (ADR 0156 D2-D4).
///
/// At an estimate step the ADR 0073 pressure-lag map of the step (the final outer iteration's momentum matrices frozen, the
/// estimate made on copies, so the step's solution is that of a fixed count) is estimated with `N = n_correctors,
/// n_correctors + 1, ...` correctors until its amplification is at most `target` or `N = max_correctors`; that count is then used
/// by every outer iteration until the next estimate. Estimates are made at the first step at or after `start`, every `every`
/// steps, and after the growth monitor fired: the largest pressure change of the last corrector grew by more than `growth` per
/// step on `growth_steps` consecutive steps (a lag mode that grows by its amplification once it dominates that change); and
/// (ADR 0162) when the step's dt differs from the dt of the last estimate by `dt_factor` or more, up or down (a step marked as
/// a landing step, [`CorrectorRecord::mark_landing_step`], is not compared).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorrectorControl {
    /// Largest number of correctors per outer iteration (the cap).
    pub max_correctors: usize,
    /// Largest accepted estimate of the pressure-lag amplification per step.
    pub target: f64,
    /// Power iterations per estimate.
    pub iterations: usize,
    /// Steps between scheduled estimates after the first (0 = the first estimate only).
    pub every: usize,
    /// First step estimated (2 for BDF2, whose first step is implicit Euler).
    pub start: usize,
    /// Growth factor per step of the last corrector's largest pressure change counted by the monitor.
    pub growth: f64,
    /// Consecutive growing steps that fire the monitor (0 = no monitor).
    pub growth_steps: usize,
    /// Fewest steps between two estimates fired by the monitor.
    pub trigger_gap: usize,
    /// Change of the step's dt against the dt of the last estimate (either way) that makes an estimate (ADR 0162); 0 = none.
    pub dt_factor: f64,
}

impl CorrectorControl {
    /// The registered control of ADR 0156: cap 10, target 0.5 (safety factor 2), 20 power iterations, an estimate every 1000
    /// steps, the monitor at growth 1.5 on 3 consecutive steps with 25 steps between fired estimates; first estimate at step 1;
    /// and ADR 0162's dt trigger at a factor 1.5.
    pub fn registered() -> Self {
        CorrectorControl {
            max_correctors: 10,
            target: 0.5,
            iterations: 20,
            every: 1000,
            start: 1,
            growth: 1.5,
            growth_steps: 3,
            trigger_gap: 25,
            dt_factor: 1.5,
        }
    }
}

/// Why an estimate of the adaptive corrector count is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EstimateReason {
    /// The first step, or the schedule of every `every` steps.
    Scheduled,
    /// The growth monitor fired.
    Monitor,
    /// The step's dt grew by `dt_factor` or more since the last estimate (ADR 0162).
    DtUp,
    /// The step's dt fell by `dt_factor` or more since the last estimate (ADR 0162).
    DtDown,
}

/// One estimate of the adaptive corrector count (ADR 0156).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CorrectorEstimate {
    /// Step (1-based) at which the estimate was made.
    pub step: usize,
    /// Count chosen.
    pub correctors: usize,
    /// Estimated pressure-lag amplification per step with that count.
    pub rho: f64,
    /// Cell holding the largest entry of the estimated mode.
    pub cell: usize,
    /// The search ended at the cap with the estimate above the target.
    pub cap_bound: bool,
    /// Made because the growth monitor fired.
    pub triggered: bool,
    /// Lag-map estimates (counts tried) of this search.
    pub evaluations: usize,
    /// The step's dt.
    pub dt: f64,
    /// Made because the step's dt changed by the control's `dt_factor` since the last estimate (ADR 0162).
    pub dt_triggered: bool,
    /// Lag-map corrector applications of this search: the sum over its evaluations of count x power iterations.
    pub work: usize,
}

/// State and record of the adaptive corrector count (ADR 0156); stays empty while `corrector_control` is `None`.
#[derive(Debug, Clone, Default)]
pub struct CorrectorRecord {
    /// Correctors per outer iteration in use (0 before the first step with the control).
    pub current: usize,
    /// Steps taken with the control.
    pub steps: u64,
    /// Sum over those steps of the correctors of the final outer iteration.
    pub sum: u64,
    /// Largest count used by a step.
    pub max: usize,
    /// Count used by the last step.
    pub last: usize,
    /// Every estimate, in order.
    pub estimates: Vec<CorrectorEstimate>,
    /// Estimates whose search ended at the cap with the estimate above the target.
    pub cap_bound: usize,
    /// Estimates made because the growth monitor fired.
    pub triggered: usize,
    /// Estimates made because the step's dt changed by the control's `dt_factor` (ADR 0162).
    pub dt_triggered: usize,
    mode: Option<Vec<f64>>,
    last_estimate: Option<usize>,
    last_trigger: Option<usize>,
    growth_run: usize,
    last_change: Option<f64>,
    fired: bool,
    /// dt of the step of the last estimate.
    last_dt: Option<f64>,
    /// The next step is a landing step (see [`mark_landing_step`](Self::mark_landing_step)).
    landing: bool,
}

impl CorrectorRecord {
    /// Whether step `step` (of size `dt`; `landing`: a landing step) makes an estimate, and why.
    fn due(&self, step: usize, dt: f64, landing: bool, c: &CorrectorControl) -> Option<EstimateReason> {
        if step < c.start.max(1) {
            return None;
        }
        let Some(last) = self.last_estimate else {
            return Some(EstimateReason::Scheduled);
        };
        if c.every > 0 && step.saturating_sub(last) >= c.every {
            return Some(EstimateReason::Scheduled);
        }
        let gap_ok = match self.last_trigger {
            Some(t) => step.saturating_sub(t) >= c.trigger_gap,
            None => true,
        };
        if self.fired && gap_ok {
            return Some(EstimateReason::Monitor);
        }
        // ADR 0162 D1, D2: the dt trigger, not on a landing step
        if let (true, false, Some(de)) = (c.dt_factor > 0.0, landing, self.last_dt) {
            if dt >= c.dt_factor * de {
                return Some(EstimateReason::DtUp);
            }
            if de >= c.dt_factor * dt {
                return Some(EstimateReason::DtDown);
            }
        }
        None
    }

    /// The next step is a landing step: its dt was shortened or stretched from the step size the time control proposed (to land
    /// on a write time or the end time), so the dt trigger of ADR 0162 neither compares it nor takes it as the dt of an
    /// estimate. Cleared by that step.
    pub fn mark_landing_step(&mut self) {
        self.landing = true;
    }

    /// The growth monitor (ADR 0156 D3.5): `change` = largest cell change of the pressure in the step's last corrector,
    /// `range` = pressure range after the step.
    pub fn observe(&mut self, change: f64, range: f64, c: &CorrectorControl) {
        if c.growth_steps == 0 {
            return;
        }
        let grew = self.last_change.is_some_and(|prev| change > c.growth * prev) && change > 1e-10 * range;
        self.growth_run = if grew { self.growth_run + 1 } else { 0 };
        self.last_change = Some(change);
        if self.growth_run >= c.growth_steps {
            self.fired = true;
        }
    }

    /// Whether the growth monitor has fired and its estimate is still to be made.
    pub fn monitor_fired(&self) -> bool {
        self.fired
    }

    /// Take the count of a search made at `step` of size `dt` (`landing`: a landing step).
    fn apply(&mut self, pick: CorrectorPick, step: usize, reason: EstimateReason, dt: f64, landing: bool) {
        let triggered = reason == EstimateReason::Monitor;
        let dt_triggered = matches!(reason, EstimateReason::DtUp | EstimateReason::DtDown);
        self.current = pick.n;
        self.estimates.push(CorrectorEstimate {
            step,
            correctors: pick.n,
            rho: pick.rho,
            cell: pick.cell,
            cap_bound: pick.cap_bound,
            triggered,
            evaluations: pick.evaluations,
            dt,
            dt_triggered,
            work: pick.work,
        });
        if pick.cap_bound {
            self.cap_bound += 1;
        }
        if triggered {
            self.triggered += 1;
            self.last_trigger = Some(step);
        }
        if dt_triggered {
            self.dt_triggered += 1;
        }
        // ADR 0162 D2: a landing step does not become the reference dt (the first estimate's is kept if it is one)
        if !landing || self.last_dt.is_none() {
            self.last_dt = Some(dt);
        }
        self.last_estimate = Some(step);
        self.mode = Some(pick.mode);
        self.growth_run = 0;
        self.last_change = None;
        self.fired = false;
    }

    /// Forget the schedule (a restored or reinitialised state): the next step at or after `start` estimates afresh.
    fn reset_schedule(&mut self) {
        self.last_estimate = None;
        self.last_dt = None;
        self.last_trigger = None;
        self.growth_run = 0;
        self.last_change = None;
        self.fired = false;
    }

    /// Mean correctors per step taken with the control (0 before any).
    pub fn mean(&self) -> f64 {
        if self.steps == 0 {
            0.0
        } else {
            self.sum as f64 / self.steps as f64
        }
    }
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
/// when `rho >= 1`, with the corrector count the step uses. Settings rule: use `n_correctors >= 2` or a smaller step. A second corrector
/// removes the lag of smooth pressure errors (the ADR 0073 model, where it starts from a projected velocity), but on a collocated mesh
/// not that of short pressure waves across thin cells: the cell update leaves the Rhie-Chow part of such a wave and the next corrector
/// amplifies it again by about the cell diffusion number (ADR 0214 M1, M2), so a step with two or more correctors can still be unstable
/// (thin gaps, fine wall layers). The consistent correctors (`PimpleSettings::consistent_correctors`) remove that instability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LagCheck {
    /// Check at step `start` and then every `every` steps (0 = step `start` only).
    pub every: usize,
    /// First step checked (1 = the first step; ADR 0077 uses 2 for BDF2, whose first step is an implicit Euler step).
    pub start: usize,
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
            start: 1,
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

/// Options of the pressure-lag estimate that leave out what is not a fluid mode (F-483, ADR 0388 D1), opt-in through
/// [`PimpleSolver::lag_fluid`]. [`LagFluid::FLUID`] is the estimate offered: at an immersed body the pressure solve of the map,
/// started from the error and stopped at a relative tolerance, keeps the part of the error in the weakly coupled solid cells,
/// which the estimate of ADR 0073 then counts as a mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LagFluid {
    /// Start every pressure solve of the map from zero instead of from the pressure error.
    pub zero_start: bool,
    /// Take the norm, the level (when no boundary fixes it) and the mode cell of the power iteration over the fluid cells only
    /// (the cells whose centre is in no body of the immersed forcing; every cell without a forcing). The vector is iterated in
    /// full.
    pub fluid_norm: bool,
    /// Stop the pressure solves of the map at this relative tolerance, with the absolute tolerance zero (diagnostic, ADR 0388
    /// A3); `None` keeps the solver's pressure tolerances.
    pub tol_rel: Option<f64>,
}

impl LagFluid {
    /// The zero start and the fluid norm (ADR 0388 D1).
    pub const FLUID: LagFluid = LagFluid {
        zero_start: true,
        fluid_norm: true,
        tol_rel: None,
    };
}

/// Matrices of the final outer iteration kept for the pressure-lag check.
struct LagCtx {
    mats: Vec<Option<FvMatrix>>,
    links: Vec<PeriodicLinks>,
    pbc: crate::pressure::PressureBc,
    /// F-550 (ADR 0617 S3): the outer iterations of the step the context was captured in (`n_outer` without the criterion).
    outer: usize,
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
    /// The largest cell diffusion number `nu dt/dx^2` of the refused step (F-535, ADR 0471 D2); `NaN` for every other trip.
    pub diffusion: f64,
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
        } else if self.quantity == "immersed solid" {
            return format!(
                "PIMPLE immersed-solid forcing (ADR 0218) is not defined with the {}: the run is stopped at step {}{outer}",
                self.stage, self.step
            );
        } else if self.quantity == "immersed flux weight" {
            return format!(
                "PIMPLE second-order immersed face flux (ADR 0254): a fluid-solid face extends the velocity of its fluid cell \
                 through the surface with a weight above 5 and can return {:.3} times what the time term carries (> 1){at}; \
                 the run is stopped at step {}{outer}: build the forcing with ImmersedForcing::build_second_order_bounded",
                self.value, self.step
            );
        } else if self.quantity == "immersed flux corrector" {
            return format!(
                "PIMPLE second-order immersed face flux with a single pressure corrector (ADR 0374): a fluid-solid face returns \
                 the velocity of its fluid cell reversed (weight {:.3} > 1, or an image face){at}, and with one corrector it \
                 returns the pressure gradient of the momentum predictor reversed; the run is stopped at step {}{outer}: use \
                 solver.n_correctors >= 2",
                self.value, self.step
            );
        } else if self.quantity == "consistent correctors" {
            // ADR 0258 V2: the located part (flag, other setting, reason) after the sentence of ADR 0214
            let located =
                ConsistentRefusal::from_stage(self.stage).map_or(String::new(), |r| format!(" ({})", r.message()));
            return format!(
                "PIMPLE consistent correctors (ADR 0214) are not defined with the {}: the run is stopped at step \
                 {}{outer}{located}",
                self.stage, self.step
            );
        } else if self.quantity == "lag onset" {
            // F-535 (ADR 0471 D4)
            return format!(
                "PIMPLE pressure-lag onset refusal (opt-in, ADR 0471): the largest cell diffusion number nu dt/dx^2 of the step                  is {:.3}{at}, and the estimated amplification of the lagged pressure error (ADR 0073) is {:.3} per step (>= 1),                  so the step would diverge; the run is stopped at step {} before any work and the state is as it was: use the                  consistent correctors with outer iterations (solver.consistent_correctors.enabled with solver.n_outer >= 8,                  ADR 0214, ADR 0451) or a smaller solver.dt",
                self.diffusion, self.value, self.step
            );
        } else if self.quantity == "lag amplification" {
            return format!(
                "PIMPLE pressure-lag check: the lagged pressure error is amplified by {:.3} per step (>= 1) at step {}, {}{at}; \
                 the run will diverge: use more pressure correctors (solver.n_correctors >= 2 removes the lag of smooth pressure \
                 errors; across thin cells short pressure waves need more, ADR 0214), the consistent correctors \
                 (solver.consistent_correctors.enabled, ADR 0214) or a smaller solver.dt",
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
            coupled_body_pair_mean: false,
            corrector_control: None,
            linear_upwind_limiter: None,
            gradient_momentum: None,
            gradient_pressure: None,
            consistent_correctors: false,
            consistent_periodic: false,
            consistent_body_force: false,
            consistent: ConsistentCorrectors::default(),
            consistent_periodic_extensions: false,
            moving_mesh_lag: false,
            rhie_chow_step_consistent: false,
            fractional_step_consistent: false,
            pressure_ic_fallback: false,
            outer_pressure_tolerance: None,
            outer_scale_window: None,
            outer_velocity_record: false,
            alpha_free_face_flux: false,
            outer_freeze_source: false,
            outer_freeze_viscosity: false,
            outer_cell_probe: Vec::new(),
            outer_frozen_limiter: false,
            outer_frozen_convecting_flux: false,
            outer_frozen_convection_gradient: false,
            outer_frozen_convection_gradient_extrapolated_boundary: false,
            outer_frozen_convection_gradient_second: false,
            outer_frozen_laplacian_gradient: false,
            outer_field_record: Vec::new(),
            lag_onset_refusal: None,
            rotational_periodic_implicit: false,
            coupled_non_orth: false,
            coupled_non_orth_without_pressure: false,
            coupled_ddt_correction: false,
            fractional_periodic_implicit: false,
            periodic_partner_extrapolated: false,
            fractional_body_force: false,
        }
    }

    /// The consistent-corrector flags in effect (ADR 0258 V1): the value `consistent` with `enabled`, `periodic` and `body_force`
    /// each also set by its older field (`consistent_correctors`, `consistent_periodic`, `consistent_body_force`).
    pub fn consistent_flags(&self) -> ConsistentCorrectors {
        ConsistentCorrectors {
            enabled: self.consistent.enabled || self.consistent_correctors,
            periodic: self.consistent.periodic || self.consistent_periodic,
            body_force: self.consistent.body_force || self.consistent_body_force,
            ..self.consistent
        }
    }

    /// Set every consistent-corrector flag from one value (ADR 0258 V1): stores it and writes the three older fields from it, so
    /// that [`consistent_flags`](Self::consistent_flags) returns exactly `flags`.
    pub fn set_consistent(&mut self, flags: ConsistentCorrectors) {
        self.consistent = flags;
        self.consistent_correctors = flags.enabled;
        self.consistent_periodic = flags.periodic;
        self.consistent_body_force = flags.body_force;
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
    /// Face viscosity of this outer iteration's momentum predictor and of the solver from then on (`PimpleSolver::nu_face`),
    /// e.g. from a turbulence sweep made at the start of the outer iteration (F-286, ADR 0199 D2); `None` keeps the present one.
    pub nu_face: Option<Vec<f64>>,
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
    /// Immersed-solid forcing `-(coeff u - source)` (F-219, ADR 0218 D3), added next to the sink; the hook does not change it. The
    /// fractional step and moving meshes refuse it (ADR 0218 D4). `None` (default) leaves the solver unchanged.
    pub immersed: Option<crate::immersed::ImmersedForcing>,
    /// Pressure jumps at internal faces (lumped fans and porous jumps), set by the hook; same algebra as the SIMPLE kernel.
    pub face_jump: Option<FaceJump>,
    /// Moving-mesh data of the current step (set only by `step_ale`, ADR 0033).
    pub(crate) ale: Option<moving::AleStep>,
    /// Set by `set_moving_mesh_time_scheme(Bdf2)`: the moving-mesh step accepts BDF2 (F-345, ADR 0379 D1).
    pub(crate) ale_bdf2_opt_in: bool,
    /// What the next BDF2 moving step needs of the last one (ADR 0379 D5); `None` without the opt-in.
    pub(crate) ale_bdf2: Option<moving::AleHistory>,
    /// Pressure increment of the previous fractional step (lagged boundary treatment, ADR 0037).
    pub(crate) fs_q: Vec<f64>,
    /// Set when the divergence guard stopped the run (ADR 0067); every later `step` returns at once. Cleared by `restore`
    /// and `reinitialise`.
    pub diverged: Option<Divergence>,
    /// Every estimate of the pressure-lag check (ADR 0073 B4).
    pub lag_estimates: Vec<LagWarning>,
    /// Estimates of the pressure-lag check with `rho >= 1` made in `Warn` mode.
    pub lag_warnings: Vec<LagWarning>,
    /// State and record of the adaptive corrector count (ADR 0156); empty while `settings.corrector_control` is `None`.
    pub correctors: CorrectorRecord,
    /// Diagnostic capture of the pressure systems (F-466, ADR 0363 D1): when `Some`, every pressure solve of the plain pressure
    /// path (`gradient_pressure = None`) appends its inputs and its result, before the divergence guard looks at them. `None`
    /// (default) leaves the solver unchanged.
    pub pressure_capture: Option<Vec<PressureCapture>>,
    /// The pressure solves repeated with incomplete-Cholesky CG by `PimpleSettings::pressure_ic_fallback` (F-466, ADR 0363
    /// amendment 1): (step, outer iteration, corrector). Empty while the setting is off.
    pub pressure_fallbacks: Vec<(usize, usize, usize)>,
    /// The pressure-lag map with the second-order immersed face flux (F-363, ADR 0374 D1, diagnostic): when `true` and the
    /// forcing has flux faces, the map of the pressure-lag estimate (`pressure_lag_check`, `corrector_control`, `lag_preflight`,
    /// `lag_trial`) reconstructs the flux of those faces and solves the pressure with the source of the solid cells, as the step
    /// does. `false` (default) leaves the estimate unchanged.
    pub immersed_lag: bool,
    /// The pressure-lag estimate without what is not a fluid mode (F-483, ADR 0388 D1): when `Some`, every estimate
    /// (`pressure_lag_check`, `corrector_control`, `lag_preflight`, `lag_trial`) uses these options. `None` (default) leaves the
    /// estimate unchanged.
    pub lag_fluid: Option<LagFluid>,
    /// When `Some`, the check of the step keeps the mode of its last estimate here (ADR 0388 D2, for `lag_trial_mode`).
    lag_mode: Option<Vec<f64>>,
    /// Set by [`PimpleSolver::project_immersed_start`] (F-515, ADR 0425 D3): the start was projected onto the fluid region of
    /// the immersed bodies, and the refusal of ADR 0374 D2 is lifted. Cleared by `reinitialise`. `false` from `new`.
    start_projected: bool,
    /// The outer iterations of every completed step with `PimpleSettings::outer_pressure_tolerance` (F-536, ADR 0458 C2); empty
    /// while it is `None`.
    pub outer_record: Vec<OuterRecord>,
    /// The velocity change of each outer iteration of every completed step with `PimpleSettings::outer_velocity_record`
    /// (F-885, ADR 0767); empty while it is `false`.
    pub outer_velocity: Vec<OuterVelocity>,
    /// The outer iterations at the probe cells of every completed step with `PimpleSettings::outer_cell_probe` (F-885, ADR
    /// 0816); empty while it is empty.
    pub outer_probe: Vec<OuterProbe>,
    /// The recorded outer iterations of every completed step with `PimpleSettings::outer_field_record` (F-885, ADR 0820);
    /// empty while it is empty.
    pub outer_fields: Vec<OuterField>,
    /// F-550 (ADR 0617 S4): the first-iteration pressure changes of the last completed steps (only with the criterion and
    /// `outer_scale_window`).
    outer_firsts: std::collections::VecDeque<f64>,
}

/// One pressure solve of a step, as captured by [`PimpleSolver::pressure_capture`] (F-466, ADR 0363 D1). With
/// [`crate::pressure::pressure_system_src`] the system can be assembled again and replayed from `p0`.
#[derive(Debug, Clone)]
pub struct PressureCapture {
    /// Step number (1-based).
    pub step: usize,
    /// Outer iteration (0-based).
    pub outer: usize,
    /// Pressure corrector (0-based).
    pub corrector: usize,
    /// Face coefficient of the pressure Laplacian (with the face jumps applied).
    pub gamma: Vec<f64>,
    /// Pressure boundary data.
    pub bc: PressureBc,
    /// Flux whose divergence is the right-hand side.
    pub phi: Vec<f64>,
    /// Volume source of the solid cells of the second-order immersed face flux (ADR 0234 D3); `None` without flux faces.
    pub src: Option<Vec<f64>>,
    /// Controls of the solve.
    pub controls: PressureControls,
    /// Start pressure of the solve.
    pub p0: Vec<f64>,
    /// Pressure returned by the solve.
    pub p: Vec<f64>,
    /// Normalised initial residual returned by the solve.
    pub res0: f64,
    /// Linear iterations of the solve.
    pub iters: usize,
}

/// The count chosen by one search of the adaptive corrector control.
struct CorrectorPick {
    n: usize,
    rho: f64,
    cell: usize,
    cap_bound: bool,
    evaluations: usize,
    work: usize,
    mode: Vec<f64>,
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
            immersed: None,
            face_jump: None,
            ale: None,
            ale_bdf2_opt_in: false,
            ale_bdf2: None,
            fs_q: Vec::new(),
            diverged: None,
            lag_estimates: Vec::new(),
            lag_warnings: Vec::new(),
            correctors: CorrectorRecord::default(),
            pressure_capture: None,
            pressure_fallbacks: Vec::new(),
            immersed_lag: false,
            lag_fluid: None,
            lag_mode: None,
            start_projected: false,
            outer_record: Vec::new(),
            outer_velocity: Vec::new(),
            outer_probe: Vec::new(),
            outer_fields: Vec::new(),
            outer_firsts: std::collections::VecDeque::new(),
        }
    }

    /// A copy of the solver's state for a trial step (ADR 0156 D7): fields, histories, sources and settings; no records, no
    /// moving-mesh step, not stopped.
    fn scratch(&self) -> PimpleSolver {
        PimpleSolver {
            geom: self.geom.clone(),
            settings: self.settings.clone(),
            u: self.u.clone(),
            p: self.p.clone(),
            phi: self.phi.clone(),
            time: self.time,
            history: Vec::new(),
            hist_u: self.hist_u.clone(),
            phi_hist: self.phi_hist.clone(),
            step_no: self.step_no,
            nu_face: self.nu_face.clone(),
            body_force: self.body_force.clone(),
            source_force: self.source_force.clone(),
            sink: self.sink.clone(),
            immersed: self.immersed.clone(),
            face_jump: self.face_jump.clone(),
            ale: None,
            ale_bdf2_opt_in: self.ale_bdf2_opt_in,
            ale_bdf2: self.ale_bdf2.clone(),
            fs_q: self.fs_q.clone(),
            diverged: None,
            lag_estimates: Vec::new(),
            lag_warnings: Vec::new(),
            correctors: CorrectorRecord::default(),
            pressure_capture: None,
            pressure_fallbacks: Vec::new(),
            immersed_lag: self.immersed_lag,
            lag_fluid: self.lag_fluid,
            lag_mode: None,
            start_projected: self.start_projected,
            outer_record: Vec::new(),
            outer_velocity: Vec::new(),
            outer_probe: Vec::new(),
            outer_fields: Vec::new(),
            outer_firsts: self.outer_firsts.clone(),
        }
    }

    /// The largest cell diffusion number `nu dt/dx^2` and its cell (F-535, ADR 0471 D3): per cell the length `h = V / max |S_f|`
    /// over its faces (leaving out the faces normal to an empty direction, so the depth of a 2D mesh is not a length) and the
    /// viscosity of the solver, or the largest `nu_face` over the cell's faces when that is set.
    pub fn max_cell_diffusion(&self, dt: f64) -> (f64, usize) {
        let g = &self.geom;
        let mesh = &g.mesh;
        let n = g.n_cells();
        let (vol, sf, own) = (mesh.cell_volumes(), mesh.face_area_vectors(), mesh.owner());
        let nei = mesh.neighbour();
        let mut amax = vec![0.0f64; n];
        let mut nu_c = vec![self.settings.nu; n];
        if self.nu_face.is_some() {
            nu_c.iter_mut().for_each(|v| *v = 0.0);
        }
        for f in 0..sf.len() {
            let a = vec3::norm(sf[f]);
            let empty_normal = (0..3).any(|k| !g.active[k] && sf[f][k].abs() > 0.99 * a);
            let cells = if f < g.n_internal {
                vec![own[f], nei[f]]
            } else {
                vec![own[f]]
            };
            for c in cells {
                if !empty_normal {
                    amax[c] = amax[c].max(a);
                }
                if let Some(nf) = &self.nu_face {
                    nu_c[c] = nu_c[c].max(nf[f]);
                }
            }
        }
        let mut best = (0.0f64, 0usize);
        for c in 0..n {
            if amax[c] > 0.0 {
                let h = vol[c] / amax[c];
                let d = nu_c[c] * dt / (h * h);
                if d > best.0 {
                    best = (d, c);
                }
            }
        }
        best
    }

    /// Pre-run estimate of the pressure-lag amplification (ADR 0156 D7, the setup rule SV301): a copy of the solver takes the
    /// next step (the second one from the start under BDF2, whose first step is implicit Euler) with the ADR 0073 check at that
    /// step, `iterations` power iterations and the configured `n_correctors`, without the adaptive control. The solver itself is
    /// not changed. `None` for the fractional step, a moving mesh, a stopped run, or when the trial is stopped by the guard.
    pub fn lag_preflight(&self, dt: f64, iterations: usize) -> Option<LagWarning> {
        self.lag_preflight_with(dt, iterations, None)
    }

    /// [`lag_preflight`](Self::lag_preflight) with the face viscosity `nu_face` for the trial step(s) (ADR 0160 D6: the one
    /// the caller would set before the next step); `None` keeps the solver's own, exactly as `lag_preflight`.
    pub fn lag_preflight_with(&self, dt: f64, iterations: usize, nu_face: Option<&[f64]>) -> Option<LagWarning> {
        if self.ale.is_some() || self.settings.fractional.is_some() || self.diverged.is_some() {
            return None;
        }
        let mut s = self.scratch();
        Self::preflight_run(&mut s, dt, iterations, nu_face)
    }

    /// The trial of [`lag_preflight_with`](Self::lag_preflight_with) on the copy `s` (ADR 0388 D2: split from it, the same
    /// operations).
    fn preflight_run(s: &mut PimpleSolver, dt: f64, iterations: usize, nu_face: Option<&[f64]>) -> Option<LagWarning> {
        if let Some(nu) = nu_face {
            s.nu_face = Some(nu.to_vec());
        }
        s.settings.corrector_control = None;
        // F-535 (ADR 0471 D2): the trial copy does not refuse itself
        s.settings.lag_onset_refusal = None;
        let euler_first =
            matches!(s.settings.time_scheme, TimeScheme::Bdf2 | TimeScheme::Bdf2Bounded) && s.hist_u[0].older.is_none();
        let at = s.step_no + if euler_first { 2 } else { 1 };
        s.settings.pressure_lag_check = Some(LagCheck {
            every: 0,
            start: at,
            iterations,
            action: LagAction::Warn,
        });
        while s.step_no < at && s.diverged.is_none() {
            s.step(dt);
        }
        s.lag_estimates.iter().find(|w| w.step == at).copied()
    }

    /// [`lag_preflight`](Self::lag_preflight) with `n_correctors` correctors instead of the configured count (ADR 0162 D5: the
    /// effective amplification of the count in use between estimates). The solver itself is not changed.
    pub fn lag_trial(&self, dt: f64, iterations: usize, n_correctors: usize) -> Option<LagWarning> {
        let mut s = self.scratch();
        s.settings.n_correctors = n_correctors.max(1);
        s.lag_preflight(dt, iterations)
    }

    /// [`lag_trial`](Self::lag_trial) with the normalised mode of the power iteration (F-483, ADR 0388 D2; one value per cell).
    /// The solver itself is not changed.
    pub fn lag_trial_mode(&self, dt: f64, iterations: usize, n_correctors: usize) -> Option<(LagWarning, Vec<f64>)> {
        let mut t = self.scratch();
        t.settings.n_correctors = n_correctors.max(1);
        if t.ale.is_some() || t.settings.fractional.is_some() || t.diverged.is_some() {
            return None;
        }
        let mut s = t.scratch();
        s.lag_mode = Some(Vec::new());
        let w = Self::preflight_run(&mut s, dt, iterations, None)?;
        let mode = s.lag_mode.take()?;
        (mode.len() == s.geom.n_cells()).then_some((w, mode))
    }

    /// Pre-run estimate of the pressure-lag amplification of the next step taken on a moving mesh (ADR 0223 E1): a copy of the solver
    /// with `moving_mesh_lag` on, the adaptive count off and the ADR 0073 check (`Warn`, `iterations` power iterations) at the next
    /// step takes [`step_ale_with_walls`](Self::step_ale_with_walls) with `new_geom`, `mesh_flux` and `wall_dpdn` and returns that
    /// step's estimate. The solver itself is not changed; the caller sets the moving walls' velocity conditions on it first, as
    /// before a real step. `None` for the fractional step, a stopped run, or a step that fails or is stopped.
    pub fn lag_preflight_moving(
        &self,
        dt: f64,
        iterations: usize,
        new_geom: Arc<FvGeometry>,
        mesh_flux: Vec<f64>,
        wall_dpdn: Vec<f64>,
    ) -> Option<LagWarning> {
        if self.settings.fractional.is_some() || self.diverged.is_some() {
            return None;
        }
        let mut s = self.scratch();
        s.settings.corrector_control = None;
        s.settings.moving_mesh_lag = true;
        let at = s.step_no + 1;
        s.settings.pressure_lag_check = Some(LagCheck {
            every: 0,
            start: at,
            iterations,
            action: LagAction::Warn,
        });
        s.step_ale_with_walls(dt, new_geom, mesh_flux, wall_dpdn).ok()?;
        s.lag_estimates.iter().find(|w| w.step == at).copied()
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
            diffusion: f64::NAN,
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

    /// What the consistent-corrector flags are validated against at this step (ADR 0258 V2). The periodic faces are searched only
    /// when a flag in effect can be refused with them.
    fn consistent_context(&self, flags: &ConsistentCorrectors) -> ConsistentContext {
        let s = &self.settings;
        let (non_orth, jump) = (s.non_orth_correctors > 0, self.face_jump.is_some());
        let periodic_matters = !flags.periodic
            || (non_orth && flags.non_orthogonal)
            || (s.component_diagonal && flags.component_diagonal)
            || (jump && flags.face_jump);
        ConsistentContext {
            fractional: s.fractional.is_some(),
            non_orth_correctors: s.non_orth_correctors,
            gradient_pressure: s.gradient_pressure.is_some(),
            component_diagonal: s.component_diagonal,
            body_force: self.body_force.is_some() || self.source_force.is_some(),
            face_jump: jump,
            periodic_faces: periodic_matters
                && pressure_bc(&self.geom, &self.u, &self.p)
                    .coupled
                    .iter()
                    .any(|c| c.is_some()),
            moving_mesh: self.ale.is_some(),
        }
    }

    /// The first combination the consistent correctors are not defined with (ADR 0214 F0, ADR 0258 V2); the sources a hook
    /// supplies are checked again in every outer iteration.
    fn consistent_conflict(&self) -> Option<ConsistentRefusal> {
        let flags = self.settings.consistent_flags();
        // ADR 0360 D2: with the opt-in, the combinations with periodic faces whose identity is met are accepted
        flags
            .validate_extended(
                &self.consistent_context(&flags),
                self.settings.consistent_periodic_extensions,
            )
            .err()
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
        // ADR 0162 D2: the landing mark holds for this step only
        let landing = std::mem::take(&mut self.correctors.landing);
        if let Some(d) = &self.diverged {
            // the guard has stopped this run (ADR 0067): do no work
            return d.info;
        }
        if self.settings.consistent_flags().enabled {
            // ADR 0214 F0: refuse the combinations the consistent correctors are not defined with, before any work
            if let Some(conflict) = self.consistent_conflict() {
                self.step_no += 1;
                return self.trip(
                    dt,
                    None,
                    conflict.stage(),
                    "consistent correctors",
                    None,
                    f64::NAN,
                    (0.0, 0.0, 0),
                );
            }
        }
        if self.immersed.is_some() && self.settings.fractional.is_some() {
            // ADR 0218 D4: the fractional step does not apply the immersed-solid forcing
            self.step_no += 1;
            return self.trip(
                dt,
                None,
                "fractional step",
                "immersed solid",
                None,
                f64::NAN,
                (0.0, 0.0, 0),
            );
        }
        if self.settings.fractional.is_some() && self.ale.is_none() {
            return self.step_fractional(dt);
        }
        if let Some(its) = self.settings.lag_onset_refusal {
            if self.step_no == 0 {
                // F-535 (ADR 0471 D2): the estimate for the step's settings on a copy, before any field is touched
                if let Some(w) = self.lag_preflight(dt, its.max(2)) {
                    if w.rho >= 1.0 {
                        let (d, cell) = self.max_cell_diffusion(dt);
                        self.step_no += 1;
                        let info = self.trip(
                            dt,
                            None,
                            "pressure-lag onset",
                            "lag onset",
                            Some(cell),
                            w.rho,
                            (0.0, 0.0, 0),
                        );
                        if let Some(div) = self.diverged.as_mut() {
                            div.diffusion = d;
                        }
                        return info;
                    }
                }
            }
        }
        let g = self.geom.clone();
        let mesh = &g.mesh;
        let n = g.n_cells();
        let n_int = g.n_internal;
        let vol = mesh.cell_volumes().to_vec();
        let sf = mesh.face_area_vectors();
        let s = self.settings.clone();
        // ADR 0258 V1: the consistent-corrector flags in effect (the value and the older fields)
        let cf = s.consistent_flags();
        self.step_no += 1;

        let mut gamma_nu = match &self.nu_face {
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

        // ADR 0223 E0: on a moving mesh the estimate and the adaptive count only with `moving_mesh_lag`
        let lag_here = self.ale.is_none() || s.moving_mesh_lag;
        let lag_due = lag_here
            && s.pressure_lag_check
                .is_some_and(|c| self.step_no == c.start.max(1) || (c.every > 0 && self.step_no % c.every == 0));
        let mut lag_ctx: Option<LagCtx> = None;
        // ADR 0156: the adaptive corrector count (on a moving mesh only with `moving_mesh_lag`, ADR 0223 E0)
        let cc = if lag_here { s.corrector_control } else { None };
        let n_lo = s.n_correctors.max(1);
        let cc_reason = match &cc {
            Some(c) => {
                if self.correctors.current == 0 {
                    self.correctors.current = n_lo;
                }
                self.correctors.due(self.step_no, dt, landing, c)
            }
            None => None,
        };
        // largest pressure change of the last corrector of the final outer iteration (the growth monitor; adaptive only)
        let mut cc_change = 0.0f64;
        let mut n_corr_final = s.n_correctors;
        // F-536 (ADR 0458 C1): the outer-iteration criterion (only with `outer_pressure_tolerance`)
        let mut final_next = false;
        let mut outer_used = 0usize;
        let mut outer_ratio = f64::NAN;
        let mut outer_first = 0.0f64;
        // F-885 (ADR 0815, opt-in): the hook's source of the step's first outer iteration (only with `outer_freeze_source`)
        let mut frozen_source: Option<Option<BodyForce>> = None;
        let mut outer_scale = 0.0f64;
        // F-885 (ADR 0767): the velocity change of each outer iteration (only with `outer_velocity_record`)
        let mut outer_dv: Vec<f64> = Vec::new();
        let mut outer_cell: Vec<usize> = Vec::new();
        // F-885 (ADR 0819, opt-in): each component's limiter factor of the step's first outer iteration
        let mut frozen_psi: [Option<Vec<f64>>; 3] = [None, None, None];
        // F-885 (ADR 0820, opt-in): the convecting flux and each component's gradients of the step's first outer iteration,
        // and the recorded iterations of the step
        let mut frozen_phi_conv: Option<Vec<f64>> = None;
        let mut frozen_grad_conv: [Option<Vec<Vec3>>; 3] = [None, None, None];
        let mut frozen_grad_lap: [Option<Vec<Vec3>>; 3] = [None, None, None];
        let mut fields_now: Vec<OuterField> = Vec::new();
        // F-885 (ADR 0816, opt-in): the probe cells' faces and face neighbours, and their iterations
        let probe_topo: Vec<(usize, Vec<usize>, Vec<usize>)> = s
            .outer_cell_probe
            .iter()
            .map(|&c| {
                let (own, nei) = (mesh.owner(), mesh.neighbour());
                let faces: Vec<usize> = (0..mesh.n_faces())
                    .filter(|&f| own[f] == c || (f < n_int && nei[f] == c))
                    .collect();
                let nbrs = faces
                    .iter()
                    .filter(|&&f| f < n_int)
                    .map(|&f| if own[f] == c { nei[f] } else { own[f] })
                    .collect();
                (c, faces, nbrs)
            })
            .collect();
        let mut probe_iters: Vec<Vec<ProbeIteration>> = vec![Vec::new(); probe_topo.len()];
        for outer in 0..s.n_outer {
            let last = outer + 1 == s.n_outer || final_next;
            let (alpha_u, alpha_p) = if last { (1.0, 1.0) } else { (s.alpha_u, s.alpha_p) };
            let p_outer_start = s.outer_pressure_tolerance.map(|_| self.p.values.clone());
            let u_outer_start = s.outer_velocity_record.then(|| self.u.values.clone());
            if let Some(h) = hook.as_mut() {
                let up = h(&self.u, &self.phi);
                self.source_force = up.source_force;
                if s.outer_freeze_source {
                    // F-885 (ADR 0815, opt-in): after the step's first outer iteration its source is kept
                    if outer == 0 {
                        frozen_source = Some(self.source_force.clone());
                    } else if let Some(f) = &frozen_source {
                        self.source_force = f.clone();
                    }
                }
                self.sink = up.sink;
                self.face_jump = up.face_jump;
                for (patch, pk) in up.patch_pressures {
                    let _ = self.p.set_bc(&g, &patch, Box::new(FixedValue::scalar(pk)));
                }
                if let Some(nu) = up.nu_face.filter(|_| !(s.outer_freeze_viscosity && outer > 0)) {
                    // ADR 0199 D2: the face viscosity of this outer iteration (ADR 0815: kept after the first if frozen)
                    gamma_nu = nu.clone();
                    self.nu_face = Some(nu);
                }
            }
            // ADR 0214 F0, ADR 0258 V2: a source the hook supplied for this outer iteration that the flags do not accept (a
            // face-jump refusal first, as before)
            let source_conflict = if cf.enabled
                && (self.body_force.is_some() || self.source_force.is_some() || self.face_jump.is_some())
            {
                use ConsistentRefusal as R;
                let found = cf.conflicts_extended(&self.consistent_context(&cf), s.consistent_periodic_extensions);
                let is_jump = |r: &&R| matches!(r, R::FaceJump | R::FaceJumpPeriodic | R::FaceJumpMovingMesh);
                found
                    .iter()
                    .find(is_jump)
                    .or_else(|| found.iter().find(|r| matches!(r, R::BodyForce)))
                    .map(|r| r.stage())
            } else {
                None
            };
            if let Some(what) = source_conflict {
                return self.trip(
                    dt,
                    Some(outer),
                    what,
                    "consistent correctors",
                    None,
                    f64::NAN,
                    (res_u, res_p, p_iters),
                );
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
                apply_body_force_wall_gradient(&g, bf, &mut pbc);
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
            // ADR 0214 F0: the off-diagonal magnitudes of the consistent correctors (empty when the option is off)
            let mut h1 = if cf.enabled { vec![0.0; n] } else { Vec::new() };
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
            // F-885 (ADR 0820, opt-in): the convecting flux of the step's first outer iteration kept for the later ones
            let phi_conv = match (s.outer_frozen_convecting_flux, &frozen_phi_conv) {
                (true, Some(kept)) => kept.clone(),
                (true, None) => {
                    frozen_phi_conv = Some(phi_conv.clone());
                    phi_conv
                }
                _ => phi_conv,
            };
            // F-885 (ADR 0820, opt-in): this iteration in the field record
            let mut field_now: Option<OuterField> = s.outer_field_record.contains(&(outer + 1)).then(|| OuterField {
                step: self.step_no,
                iteration: outer + 1,
                u: Vec::new(),
                phi_conv: phi_conv.clone(),
                grad_convection: [None, None, None],
                grad_laplacian: [None, None, None],
            });
            // F-885 (ADR 0816, opt-in): this iteration at the probe cells (the convecting flux, outward)
            let mut probe_now: Vec<ProbeIteration> = probe_topo
                .iter()
                .map(|(c, faces, nbrs)| ProbeIteration {
                    diag: [f64::NAN; 3],
                    off_sum: [f64::NAN; 3],
                    psi: vec![[f64::NAN; 3]; nbrs.len() + 1],
                    flux: faces
                        .iter()
                        .map(|&f| {
                            if mesh.owner()[f] == *c {
                                phi_conv[f]
                            } else {
                                -phi_conv[f]
                            }
                        })
                        .collect(),
                    u: [f64::NAN; 3],
                })
                .collect();
            // F-743 (ADR 0664, opt-in): the rotational partner links; when any exists the component solves are deferred to
            // one block solve, so that every component is assembled from the iterate `u_iter`
            let rot = match (s.rotational_periodic_implicit, (0..3).find(|&k| g.active[k])) {
                (true, Some(k0)) => {
                    RotationalLinks::build(&g, &self.u, &phi_conv, &gamma_nu, &self.u.boundary_coeffs(&g, k0))
                }
                _ => RotationalLinks::default(),
            };
            // F-744 (ADR 0667, opt-in): the coupled faces' non-orthogonal correction and the partner-point gradients of the
            // momentum, formed from the values the default gradient is formed from
            let cn_u = s.coupled_non_orth.then(|| {
                assert!(
                    s.gradient_momentum.is_none() && s.gradient_pressure.is_none(),
                    "coupled_non_orth: not supported with gradient_momentum or gradient_pressure"
                );
                assert!(
                    !(cf.enabled && s.non_orth_correctors > 0),
                    "coupled_non_orth: not supported with the consistent correctors and non-orthogonal correctors"
                );
                CoupledNonOrth::build(&g, |pi| self.u.bc(pi).is_coupled())
            });
            let cn_grads: Option<[Option<Vec<Vec3>>; 3]> = match &cn_u {
                Some(cn) if s.convection.needs_gradient() || s.non_orth_correctors > 0 => {
                    let cells: [Vec<f64>; 3] = [0, 1, 2].map(|k| match (extrap, &self.hist_u[k].older) {
                        (Some(r), Some(older)) => (0..n)
                            .map(|c| (1.0 + r) * self.hist_u[k].old[c] - r * older[c])
                            .collect(),
                        _ => u_iter.iter().map(|v| v[k]).collect(),
                    });
                    let values: Vec<Vec3> = (0..n).map(|c| [cells[0][c], cells[1][c], cells[2][c]]).collect();
                    let bvals: [Vec<f64>; 3] = [0, 1, 2].map(|k| ub.iter().map(|v| v[k]).collect());
                    Some(cn.vector_gradients(&g, &values, &cells, &bvals))
                }
                _ => None,
            };
            let mut deferred: Vec<Option<FvMatrix>> = vec![None, None, None];
            // F-757 (ADR 0687 amendment 1, opt-in): the translational coupled faces' partner values of the extrapolated field
            let bc_extrapolated: Vec<Vec<crate::bc::FaceCoeffs>> = match extrap {
                Some(r) if s.periodic_partner_extrapolated && g.coupled_face.iter().any(|&c| c) => {
                    let mut ue = self.u.clone();
                    for c in 0..n {
                        for j in 0..3 {
                            let older = self.hist_u[j].older.as_ref().map_or(self.hist_u[j].old[c], |v| v[c]);
                            ue.values[c][j] = (1.0 + r) * self.hist_u[j].old[c] - r * older;
                        }
                    }
                    (0..3).map(|k| ue.boundary_coeffs(&g, k)).collect()
                }
                _ => Vec::new(),
            };
            // F-885 (ADR 0824, opt-in): the boundary values of the extrapolated field for the kept convection gradient
            let ub_extrapolated: Option<Vec<Vec3>> = match extrap {
                Some(r)
                    if s.outer_frozen_convection_gradient
                        && s.outer_frozen_convection_gradient_extrapolated_boundary =>
                {
                    let mut ue = self.u.clone();
                    for c in 0..n {
                        for j in 0..3 {
                            let older = self.hist_u[j].older.as_ref().map_or(self.hist_u[j].old[c], |v| v[c]);
                            ue.values[c][j] = (1.0 + r) * self.hist_u[j].old[c] - r * older;
                        }
                    }
                    Some(ue.boundary_values(&g))
                }
                _ => None,
            };
            for k in 0..3 {
                if !g.active[k] {
                    continue;
                }
                let cells: Vec<f64> = u_iter.iter().map(|v| v[k]).collect();
                let mut bc = self.u.boundary_coeffs(&g, k);
                if !bc_extrapolated.is_empty() {
                    for (pi, p) in mesh.patches().iter().enumerate() {
                        if g.periodic_partner[pi].is_none() || g.periodic_rot[pi].is_some() {
                            continue;
                        }
                        for f in p.faces() {
                            let i = f - n_int;
                            bc[i].b = bc_extrapolated[k][i].b;
                            bc[i].d = bc_extrapolated[k][i].d;
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
                // F-885 (ADR 0824, opt-in): the convection gradient with the extrapolated field's boundary values
                let grad_e: Option<Vec<Vec3>> = match (&grad, &ub_extrapolated) {
                    (Some(_), Some(ue)) => {
                        let bve: Vec<f64> = ue.iter().map(|v| v[k]).collect();
                        Some(least_squares(&g, &conv_cells, &bve))
                    }
                    _ => None,
                };
                // F-885 (ADR 0820, opt-in): the convection's gradient of the step's first outer iteration kept for the later
                // ones (ADR 0824: of the second with `outer_frozen_convection_gradient_second`)
                let keep = !(s.outer_frozen_convection_gradient_second && outer == 0);
                let grad_c: Option<&[Vec3]> = match (&grad, s.outer_frozen_convection_gradient) {
                    (Some(gr), true) if keep => Some(
                        frozen_grad_conv[k]
                            .get_or_insert_with(|| grad_e.unwrap_or_else(|| gr.clone()))
                            .as_slice(),
                    ),
                    _ => grad.as_deref(),
                };
                // ADR 0183 P-A (opt-in): the limited gradient in the convection term only
                let limited: Option<Vec<Vec3>> = match (s.linear_upwind_limiter, grad_c) {
                    (Some(kind), Some(gr)) if s.convection == ConvScheme::LinearUpwind => {
                        let psi = match &frozen_psi[k] {
                            // F-885 (ADR 0819, opt-in): the factor kept from the step's first outer iteration
                            Some(kept) if s.outer_frozen_limiter => kept.clone(),
                            _ => {
                                let formed = crate::gradient::limiter(&g, &conv_cells, &bv, gr, kind);
                                if s.outer_frozen_limiter {
                                    frozen_psi[k] = Some(formed.clone());
                                }
                                formed
                            }
                        };
                        for ((c, _, nbrs), pr) in probe_topo.iter().zip(probe_now.iter_mut()) {
                            // F-885 (ADR 0816, opt-in): the factor at the probe cell and its face neighbours
                            for (i, &cell) in std::iter::once(c).chain(nbrs).enumerate() {
                                pr.psi[i][k] = psi[cell];
                            }
                        }
                        Some(gr.iter().zip(&psi).map(|(v, k)| vec3::scale(*v, *k)).collect())
                    }
                    _ => None,
                };
                let mut m = convection(
                    &g,
                    &phi_conv,
                    &conv_cells,
                    &bc,
                    s.convection,
                    limited.as_deref().or(grad_c),
                );
                // ADR 0204 K3: the gradient options of the momentum Laplacian, with the values the gradient was computed from
                // (F-744, ADR 0667: with `coupled_non_orth` the partner-point gradient)
                let lap_grad = match &cn_grads {
                    Some(gs) => gs[k].as_deref(),
                    // F-885 (ADR 0820, opt-in): the Laplacian's gradient of the step's first outer iteration kept for the
                    // later ones
                    None => match (&grad, s.outer_frozen_laplacian_gradient) {
                        (Some(gr), true) => Some(frozen_grad_lap[k].get_or_insert_with(|| gr.clone()).as_slice()),
                        _ => grad.as_deref(),
                    },
                };
                if let Some(fr) = field_now.as_mut() {
                    // F-885 (ADR 0820, the record): the gradients this iteration's convection and Laplacian take
                    fr.grad_convection[k] = grad_c.map(|x| x.to_vec());
                    fr.grad_laplacian[k] = lap_grad.map(|x| x.to_vec());
                }
                m.add(&laplacian_opt(
                    &g,
                    &gamma_nu,
                    &bc,
                    lap_grad,
                    &conv_cells,
                    &bv,
                    s.gradient_momentum,
                ));
                if let (Some(cn), Some(gs)) = (&cn_u, &cn_grads) {
                    cn.add_vector_rhs(&mut m.b, k, &gamma_nu, gs);
                }
                // periodic partner values implicit (ADR 0046): move the lagged partner term back into the matrix
                let lk = PeriodicLinks::build(&g, &self.u, &phi_conv, &gamma_nu, &bc);
                let q_lag: Vec<f64> = if bc_extrapolated.is_empty() {
                    u_iter.iter().map(|v| v[k]).collect()
                } else {
                    conv_cells.clone()
                };
                lk.fix_rhs(&mut m, &q_lag);
                if !rot.is_empty() {
                    rot.fix_rhs(&mut m, k, &u_iter);
                }
                if let Some(sink) = &self.sink {
                    for c in 0..n {
                        m.a.diag[c] += sink[c] * vol[c];
                    }
                }
                if let Some(ib) = &self.immersed {
                    // ADR 0218 D3: the immersed-solid forcing, implicit coefficient and source
                    for c in 0..n {
                        m.a.diag[c] += ib.coeff[c] * vol[c];
                        m.b[c] += ib.source[c][k] * vol[c];
                    }
                }
                let mut m = add_ddt(s.time_scheme, m, &vol, dt, &self.hist_u[k]);
                if let Some(a) = &self.ale {
                    match (&a.vol_older, &self.hist_u[k].older) {
                        // ADR 0379 D2: the three-level step (only with the BDF2 opt-in of the moving step)
                        (Some(_), Some(older)) if c2 != 0.0 => {
                            a.fix_ddt_bdf2(&mut m, &vol, &self.hist_u[k].old, older, c1, c2, dt)
                        }
                        _ => a.fix_ddt(&mut m, &vol, &self.hist_u[k].old, dt),
                    }
                }
                if let Some(ts) = &transposed {
                    for c in 0..n {
                        m.b[c] += vol[c] * ts[c][k];
                    }
                }
                if !probe_topo.is_empty() {
                    // F-885 (ADR 0816, opt-in): the diagonal and the sum the relaxation takes, as the three `relax` form it
                    let mut sums = m.a.off_diag_abs_sum();
                    for &(r, _, a) in &lk.t {
                        sums[r] += a.abs();
                    }
                    if !rot.is_empty() {
                        rot.add_magnitudes(&mut sums, k);
                    }
                    for ((c, _, _), pr) in probe_topo.iter().zip(probe_now.iter_mut()) {
                        pr.diag[k] = m.a.diag[*c];
                        pr.off_sum[k] = sums[*c];
                    }
                }
                if alpha_u < 1.0 {
                    if rot.is_empty() {
                        lk.relax(&mut m, alpha_u, &cells);
                    } else {
                        rot.relax(&lk, &mut m, k, alpha_u, &cells);
                    }
                }
                let mut sys = m.clone();
                for c in 0..n {
                    sys.b[c] -= vol[c] * grad_p[c][k];
                    if let Some(bf) = body {
                        sys.b[c] += vol[c] * bf.cell[c][k];
                    }
                }
                if rot.is_empty() {
                    if outer == 0 {
                        res_u = res_u.max(lk.residual(&sys, &cells));
                    }
                    let mut x = cells;
                    lk.solve(&sys, &mut x, &s.momentum_solver);
                    for c in 0..n {
                        self.u.values[c][k] = x[c];
                    }
                } else {
                    deferred[k] = Some(sys);
                }
                for c in 0..n {
                    a_p[c] += m.a.diag[c];
                }
                if cf.enabled {
                    add_consistent_h1(&mut h1, &m, &lk, cf.periodic);
                    if cf.periodic && !rot.is_empty() {
                        rot.add_magnitudes(&mut h1, k);
                    }
                }
                diag_k[k] = m.a.diag.clone();
                mats[k] = Some(m);
                links[k] = lk;
            }
            if !rot.is_empty() {
                // F-743 (ADR 0664): the one block solve of the deferred components
                let (a, b, mut x) = rot.block_system(&deferred, &links, &u_iter);
                if outer == 0 {
                    res_u = res_u.max(crate::simple::normalised_residual_csr(&a, &b, &x));
                }
                rcfd_linalg::bicgstab(&a, &b, &mut x, &rcfd_linalg::Ilu0::new(&a), &s.momentum_solver);
                for k in 0..3 {
                    if let Some(bk) = rot.block[k] {
                        for c in 0..n {
                            self.u.values[c][k] = x[bk * n + c];
                        }
                    }
                }
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
            for h in h1.iter_mut() {
                *h /= n_active;
            }
            // ADR 0254 E2: the unbounded extension of ADR 0234 D1 is refused where it can amplify
            if let Some((c, v)) = self
                .immersed
                .as_ref()
                .and_then(|ib| ib.flux_growth((c1 - c2) / dt, &a_p))
            {
                let res = (res_u, res_p, p_iters);
                return self.trip(
                    dt,
                    Some(outer),
                    "momentum predictor",
                    "immersed flux weight",
                    Some(c),
                    v,
                    res,
                );
            }
            if lag_due && last {
                lag_ctx = Some(LagCtx {
                    mats: mats.clone(),
                    links: links.clone(),
                    pbc: pbc.clone(),
                    outer: outer + 1,
                });
            }
            if let (Some(c), Some(reason), true) = (&cc, cc_reason, last) {
                // ADR 0156 D3: choose the count on the frozen matrices of this (final) outer iteration, before its correctors
                let ctx = LagCtx {
                    mats: mats.clone(),
                    links: links.clone(),
                    pbc: pbc.clone(),
                    outer: outer + 1,
                };
                let n_hi = c.max_correctors.max(n_lo);
                // ADR 0162 D3: no step-down check when the step has grown
                let pick = self.pick_correctors(&ctx, c, n_lo, n_hi, reason != EstimateReason::DtUp);
                self.correctors.apply(pick, self.step_no, reason, dt, landing);
            }
            let n_corr = if cc.is_some() {
                self.correctors.current
            } else {
                s.n_correctors
            };
            if last {
                n_corr_final = n_corr;
            }
            // ADR 0374 D2: a flux face that returns the fluid velocity reversed is refused with a single PISO corrector
            // (F-515, ADR 0425 D3: not for a start projected onto the fluid region)
            if n_corr == 1 && s.n_outer == 1 && !self.start_projected {
                if let Some((c, w)) = self.immersed.as_ref().and_then(|ib| ib.single_corrector_face()) {
                    let res = (res_u, res_p, p_iters);
                    return self.trip(
                        dt,
                        Some(outer),
                        "pressure correctors",
                        "immersed flux corrector",
                        Some(c),
                        w,
                        res,
                    );
                }
            }
            let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            // ADR 0245 C (opt-in): the face coefficient the carry-over is consistent with (ADR 0352: also with the consistent
            // correctors and on a moving mesh)
            let rc = s.rhie_chow_step_consistent;
            let gamma_p = if rc {
                face_consistent::harmonic_face(&g, &r_au)
            } else {
                face_interpolate(&g, &r_au)
            };
            let gamma_bf = (s.coupled_body_pair_mean && body.is_some()).then(|| coupled_gamma(&g, &pbc, &gamma_p));
            // F-754 (ADR 0675): the coupled faces' transient Rhie-Chow term (`None` unless asked for)
            let ddt_cp = (s.ddt_correction && s.coupled_ddt_correction && self.ale.is_none())
                .then(|| coupled_ddt_term(&g, &pbc, &self.hist_u, &self.phi_hist, &gamma_p, c1, c2));
            // ADR 0214 F0: rAtU of the consistent correctors and its face values (`None` = the default correctors)
            let r_t: Option<Vec<f64>> = cf.enabled.then(|| consistent_rau(&vol, &a_p, &h1));
            let gamma_t: Option<Vec<f64>> = r_t.as_ref().map(|r| face_interpolate(&g, r));
            // F-885 (ADR 0811, opt-in): `(1 - alpha_u) (phi* - F(u*))`, the same in every corrector of this outer iteration
            let af_term: Option<Vec<f64>> = (s.alpha_free_face_flux && alpha_u < 1.0).then(|| {
                assert!(
                    !cf.enabled && self.ale.is_none() && self.immersed.is_none() && self.face_jump.is_none(),
                    "alpha_free_face_flux: not supported with the consistent correctors, a face jump, an immersed solid or a moving mesh"
                );
                let fu = face_flux(&g, &u_iter, &boundary_hby_a(&g, &pbc, &ub, &u_iter));
                (0..mesh.n_faces())
                    .map(|f| {
                        if g.empty[f] || (f >= n_int && pbc.fixed_u[f - n_int]) {
                            0.0
                        } else {
                            (1.0 - alpha_u) * (self.phi[f] - fu[f])
                        }
                    })
                    .collect()
            });

            // ---- pressure correctors ---------------------------------------
            for corr in 0..n_corr {
                let mut hby_a = vec![[0.0; 3]; n];
                for k in 0..3 {
                    if let Some(m) = &mats[k] {
                        let cells: Vec<f64> = self.u.values.iter().map(|v| v[k]).collect();
                        let mut h = links[k].h_operator(m, &cells);
                        if !rot.is_empty() {
                            rot.fix_h(&mut h, k, &self.u.values);
                        }
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
                for &(f, t) in ddt_cp.iter().flatten() {
                    phi_hby_a[f] += t / dt;
                }
                if let Some(t) = &af_term {
                    for (a, b) in phi_hby_a.iter_mut().zip(t) {
                        *a += b;
                    }
                }
                if rc {
                    let b = body.map(|bf| bf.cell.as_slice());
                    face_consistent::add_lagged_gradient(&g, &pbc, &gamma_p, &r_au, &self.p.values, b, &mut phi_hby_a);
                }
                // ADR 0234 D1 to D3: the flux of the fluid-solid faces at the surface crossing, the source of the solid cells
                let ib_src = self
                    .immersed
                    .as_ref()
                    .and_then(|ib| ib.reconstruct_face_flux(&hby_a, &mut phi_hby_a, s.gradient_pressure.is_none()));
                if let Some(bf) = body {
                    // balanced-force face value of the body acceleration, like the pressure gradient
                    let gbf = gamma_bf.as_deref().unwrap_or(&gamma_p);
                    // ADR 0223 E3 (reached only with `consistent_body_force`): with the consistent correctors the fixed-velocity
                    // faces take the coefficient of the pressure equation's Neumann term there, interp(rAtU)
                    let wall_t = gamma_t.as_deref();
                    for f in 0..mesh.n_faces() {
                        if !g.empty[f] {
                            let c = match wall_t {
                                Some(gt)
                                    if f >= n_int && pbc.fixed_u[f - n_int] && pbc.coupled[f - n_int].is_none() =>
                                {
                                    gt[f]
                                }
                                _ => gbf[f],
                            };
                            phi_hby_a[f] += c * bf.face[f];
                        }
                    }
                }
                if let Some(a) = &self.ale {
                    // the face coefficient of the pressure equation (ADR 0214 F0: interp(rAtU) with the consistent correctors)
                    a.add_wall_flux(&g, &pbc, gamma_t.as_deref().unwrap_or(&gamma_p), &mut phi_hby_a);
                }
                // ADR 0258 X1: the fixed-pressure boundary part of the non-orthogonal compensation, for the pressure equation only
                let mut eq_only: Vec<(usize, f64)> = Vec::new();
                if let Some(rt) = &r_t {
                    // ADR 0352 D1 (with C): the compensation takes interp(rAtU) - Gamma face by face
                    let dr_c: Option<Vec<f64>> = match (&gamma_t, rc) {
                        (Some(gt), true) => Some(face_consistent::consistent_face_difference(gt, &gamma_p)),
                        _ => None,
                    };
                    // ADR 0214 F0: the SIMPLEC split of this corrector with the pressure that drove it
                    let p_prev = &self.p.values;
                    consistent_split(&g, &pbc, &r_au, rt, p_prev, dr_c.as_deref(), &mut phi_hby_a, &mut hby_a);
                    if s.non_orth_correctors > 0 {
                        // ADR 0258 X1 (reached only with `non_orthogonal`): the non-orthogonal part of the compensation
                        eq_only = consistent_non_orth(&g, &pbc, &r_au, rt, p_prev, dr_c.as_deref(), &mut phi_hby_a);
                    }
                    if let Some(j) = &self.face_jump {
                        // ADR 0258 X3 (reached only with `face_jump`): the cell term with the gradient the predictor used, the
                        // jump removed from the face interpolation
                        let fix = j.gradient_fix(&g, &j.totals(&self.phi));
                        for (c, h) in hby_a.iter_mut().enumerate() {
                            for k in 0..3 {
                                if g.active[k] {
                                    h[k] -= (r_au[c] - rt[c]) * fix[c][k];
                                }
                            }
                        }
                    }
                }
                // pressure jumps: phi (1 + G R) = phiHbyA + G rise - G snGrad p with G = gamma lap, as in the SIMPLE kernel
                let mut gamma_j = match &gamma_t {
                    Some(t) => t.clone(),
                    None => gamma_p.clone(),
                };
                if let Some(j) = &self.face_jump {
                    for f in 0..n_int {
                        if g.empty[f] || (j.rise[f] == 0.0 && j.resist[f] == 0.0) {
                            continue;
                        }
                        if let Some(gt) = &gamma_t {
                            // ADR 0258 X3: the old driving difference less the old total rise is compensated (the pressure
                            // part by `consistent_split`), and the face takes interp(rAtU)
                            let gu = gt[f] * g.lap_coeff[f];
                            phi_hby_a[f] +=
                                (gt[f] - gamma_p[f]) * g.lap_coeff[f] * (j.resist[f] * self.phi[f] - j.rise[f]);
                            let den = 1.0 + gu * j.resist[f];
                            phi_hby_a[f] = (phi_hby_a[f] + gu * j.rise[f]) / den;
                            gamma_j[f] = gt[f] / den;
                            continue;
                        }
                        let gu = gamma_p[f] * g.lap_coeff[f];
                        let den = 1.0 + gu * j.resist[f];
                        phi_hby_a[f] = (phi_hby_a[f] + gu * j.rise[f]) / den;
                        gamma_j[f] = gamma_p[f] / den;
                    }
                }
                // ADR 0258 X1: the flux divergence of the pressure equation (`None`: that of `phi_hby_a`, as before)
                let phi_eq: Option<Vec<f64>> = (!eq_only.is_empty()).then(|| {
                    let mut q = phi_hby_a.clone();
                    for &(f, v) in &eq_only {
                        q[f] += v;
                    }
                    q
                });
                let phi_solve: &[f64] = phi_eq.as_deref().unwrap_or(&phi_hby_a);
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
                // ADR 0204 K3: the gradient options of the pressure equation (`None`: as before)
                // F-744 (ADR 0667, opt-in): the coupled faces' non-orthogonal correction in the pressure equation and its flux
                // (F-752, ADR 0672: not with `coupled_non_orth_without_pressure`)
                let cn_p = (s.coupled_non_orth && !s.coupled_non_orth_without_pressure).then(|| {
                    assert!(
                        ib_src.is_none(),
                        "coupled_non_orth: not supported with an immersed volume source"
                    );
                    CoupledNonOrth::build(&g, |pi| self.p.bc(pi).is_coupled())
                });
                let (r0, it) = match s.gradient_pressure {
                    None => match &ib_src {
                        None => match &cn_p {
                            None => solve_pressure(&g, &gamma_j, &pbc, phi_solve, &mut p_new, &controls),
                            Some(cn) => crate::pressure::solve_pressure_coupled(
                                &g, &gamma_j, &pbc, phi_solve, &mut p_new, &controls, cn,
                            ),
                        },
                        Some(q) => {
                            crate::pressure::solve_pressure_src(&g, &gamma_j, &pbc, phi_solve, &mut p_new, &controls, q)
                        }
                    },
                    Some(o) => {
                        let (r, i, _) = crate::pressure::solve_pressure_parts_opt(
                            &g,
                            &gamma_j,
                            &pbc,
                            &phi_hby_a,
                            &mut p_new,
                            &controls,
                            false,
                            None,
                            Some(o),
                        );
                        (r, i)
                    }
                };
                p_iters += it;
                if corr == 0 {
                    res_p = r0;
                }
                if let (Some(cap), None) = (self.pressure_capture.as_mut(), s.gradient_pressure) {
                    // F-466, ADR 0363 D1: the diagnostic capture (opt-in), before the guard
                    cap.push(PressureCapture {
                        step: self.step_no,
                        outer,
                        corrector: corr,
                        gamma: gamma_j.clone(),
                        bc: pbc.clone(),
                        phi: phi_solve.to_vec(),
                        src: ib_src.clone(),
                        controls,
                        p0: p_old.clone(),
                        p: p_new.clone(),
                        res0: r0,
                        iters: it,
                    });
                }
                if s.pressure_ic_fallback && s.gradient_pressure.is_none() && first_nonfinite(&p_new).is_some() {
                    // F-466, ADR 0363 amendment 1 (opt-in): repeat a non-finite solve with incomplete-Cholesky CG
                    let ic = PressureControls { amg: false, ..controls };
                    p_new = p_old.clone();
                    let (_, it_ic) = match &ib_src {
                        None => solve_pressure(&g, &gamma_j, &pbc, phi_solve, &mut p_new, &ic),
                        Some(q) => {
                            crate::pressure::solve_pressure_src(&g, &gamma_j, &pbc, phi_solve, &mut p_new, &ic, q)
                        }
                    };
                    p_iters += it_ic;
                    self.pressure_fallbacks.push((self.step_no, outer, corr));
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
                self.phi = match s.gradient_pressure {
                    None => match &cn_p {
                        None => corrected_flux(&g, &gamma_j, &pbc, &phi_hby_a, &p_new, &ub, s.non_orth_correctors > 0),
                        Some(cn) => crate::pressure::corrected_flux_coupled(
                            &g,
                            &gamma_j,
                            &pbc,
                            &phi_hby_a,
                            &p_new,
                            &ub,
                            s.non_orth_correctors > 0,
                            cn,
                        ),
                    },
                    Some(o) => {
                        let grad = (s.non_orth_correctors > 0)
                            .then(|| least_squares(&g, &p_new, &boundary_values_pbc(&g, &pbc, &p_new)));
                        crate::pressure::corrected_flux_opt(
                            &g,
                            &gamma_j,
                            &pbc,
                            &phi_hby_a,
                            &p_new,
                            &ub,
                            grad.as_deref(),
                            Some(o),
                        )
                    }
                };
                let bv_new = boundary_values_pbc(&g, &pbc, &p_new);
                let mut grad_new = green_gauss(&g, &p_new, &bv_new, 0);
                if let Some(j) = &self.face_jump {
                    let fix = j.gradient_fix(&g, &j.totals(&self.phi));
                    for (a, b) in grad_new.iter_mut().zip(&fix) {
                        *a = vec3::add(*a, *b);
                    }
                }
                for c in 0..n {
                    // the coefficient of the new pressure gradient (ADR 0214 F0: rAtU with the consistent correctors)
                    let r_c = r_t.as_ref().map_or(r_au[c], |r| r[c]);
                    for k in 0..3 {
                        if g.active[k] {
                            let bforce = body.map_or(0.0, |bf| r_au[c] * bf.cell[c][k]);
                            self.u.values[c][k] = hby_a[c][k] + bforce - r_c * grad_new[c][k];
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
                let last_corr = corr + 1 == n_corr;
                let a = if last_corr { alpha_p } else { 1.0 };
                if cc.is_some() && last && last_corr {
                    cc_change = (0..n).fold(0.0f64, |m, c| m.max((p_new[c] - p_old[c]).abs()));
                }
                for c in 0..n {
                    self.p.values[c] = p_old[c] + a * (p_new[c] - p_old[c]);
                }
            }
            outer_used = outer + 1;
            if let Some(u0) = &u_outer_start {
                // ADR 0767: d_k = max_c |u_k - u_(k-1)|; ADR 0810: its cell
                let (d, cell) = max_velocity_change(u0, &self.u.values);
                outer_dv.push(d);
                outer_cell.push(cell);
            }
            for (((c, _, _), mut pr), iters) in probe_topo.iter().zip(probe_now).zip(probe_iters.iter_mut()) {
                // F-885 (ADR 0816, opt-in): the probe cell's velocity after the correctors
                pr.u = self.u.values[*c];
                iters.push(pr);
            }
            if let Some(mut fr) = field_now {
                // F-885 (ADR 0820, the record): the velocity after the correctors
                fr.u = self.u.values.clone();
                fields_now.push(fr);
            }
            if last {
                break;
            }
            if let (Some(tol), Some(p0)) = (s.outer_pressure_tolerance, &p_outer_start) {
                // ADR 0458 C1: c_k = max |p_k - p_(k-1)| / max |p_1 - p^n|; met: the next iteration is the final one
                let change = (0..n).fold(0.0f64, |m, c| m.max((self.p.values[c] - p0[c]).abs()));
                if outer == 0 {
                    outer_first = change;
                    // F-550 (ADR 0617 S2): the windowed scale (only with `outer_scale_window`)
                    outer_scale = match s.outer_scale_window {
                        Some(_) => self.outer_firsts.iter().fold(change, |m, &d| m.max(d)),
                        None => change,
                    };
                }
                outer_ratio = if outer_scale > 0.0 {
                    change / outer_scale
                } else if change == 0.0 {
                    0.0
                } else {
                    f64::INFINITY
                };
                final_next = outer_ratio <= tol;
            }
        }

        // ---- finish the step ------------------------------------------------
        if let Some(c) = &cc {
            // ADR 0156: the step's record and the growth monitor
            let (lo, hi) = self
                .p
                .values
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
            let r = &mut self.correctors;
            r.steps += 1;
            r.sum += n_corr_final as u64;
            r.max = r.max.max(n_corr_final);
            r.last = n_corr_final;
            r.observe(cc_change, hi - lo, c);
        }
        if let (Some(ctx), Some(chk)) = (&lag_ctx, s.pressure_lag_check) {
            // the count the step used (the configured one unless the adaptive control is on)
            let (rho, cell, mode) = self.lag_amplification_n(ctx, n_corr_final, chk.iterations.max(2), None);
            if self.lag_mode.is_some() {
                // ADR 0388 D2: the mode for `lag_trial_mode`
                self.lag_mode = Some(mode);
            }
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
        if s.outer_pressure_tolerance.is_some() {
            // ADR 0458 C2: the outer iterations of the step
            self.outer_record.push(OuterRecord {
                step: self.step_no,
                used: outer_used,
                met: final_next,
                ratio: outer_ratio,
                first: if outer_ratio.is_nan() { f64::NAN } else { outer_first },
                scale: if outer_ratio.is_nan() { f64::NAN } else { outer_scale },
            });
            if let Some(w) = s.outer_scale_window {
                // ADR 0617 S4: the history of the window (the `w - 1` steps before the next one)
                if !outer_ratio.is_nan() {
                    self.outer_firsts.push_back(outer_first);
                }
                while self.outer_firsts.len() > w.max(1) - 1 {
                    self.outer_firsts.pop_front();
                }
            }
        }
        if s.outer_velocity_record {
            // ADR 0767: the velocity change of each outer iteration of the step
            self.outer_velocity.push(OuterVelocity {
                step: self.step_no,
                change: outer_dv,
                cell: outer_cell,
            });
        }
        // ADR 0820: the recorded iterations of the step
        self.outer_fields.append(&mut fields_now);
        for ((c, faces, nbrs), iterations) in probe_topo.into_iter().zip(probe_iters) {
            // ADR 0816: the probe cells' iterations of the step
            self.outer_probe.push(OuterProbe {
                step: self.step_no,
                cell: c,
                faces,
                neighbours: nbrs,
                iterations,
            });
        }
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

    /// One application of the linearised pressure-lag map of a step (ADR 0073 B4): the pressure error `e` drives the momentum predictors
    /// and the `n_corr` correctors of every outer iteration with the frozen matrices of `ctx`; returns the pressure error at the end of
    /// the step.
    fn lag_map(
        &self,
        ctx: &LagCtx,
        n_corr: usize,
        e: &[f64],
        pbc_h: &crate::pressure::PressureBc,
        hom: Option<&crate::immersed::ImmersedForcing>,
        lf: Option<&LagFluid>,
    ) -> Vec<f64> {
        let g = &self.geom;
        let s = &self.settings;
        let cf = s.consistent_flags();
        let n = g.n_cells();
        let vol = g.mesh.cell_volumes();
        let n_active = g.active.iter().filter(|&&a| a).count() as f64;
        let zero_ub = vec![[0.0; 3]; g.mesh.n_boundary_faces()];
        let mut controls = PressureControls {
            p_ref: (s.p_ref.0, 0.0),
            non_orth_correctors: s.non_orth_correctors,
            solver: s.pressure_solver,
            amg: s.amg_pressure,
            linear: s.linear,
        };
        // ADR 0388 D1 (`lag_fluid`): the tolerance of the pressure solves of the map
        if let Some(t) = lf.and_then(|o| o.tol_rel) {
            controls.solver.tol_rel = t;
            controls.solver.tol_abs = 0.0;
        }
        let zero_start = lf.is_some_and(|o| o.zero_start);
        let mut p = e.to_vec();
        let mut u: Vec<Vec3> = vec![[0.0; 3]; n];
        // F-550 (ADR 0617 S3): the outer passes of the step the context was captured in
        for outer in 0..ctx.outer {
            let last = outer + 1 == ctx.outer;
            let (alpha_u, alpha_p) = if last { (1.0, 1.0) } else { (s.alpha_u, s.alpha_p) };
            let bv = boundary_values_pbc(g, pbc_h, &p);
            let grad = green_gauss(g, &p, &bv, 0);
            let mut used: Vec<Option<FvMatrix>> = vec![None, None, None];
            let mut a_p = vec![0.0; n];
            // ADR 0214 F0: the off-diagonal magnitudes of the consistent correctors (empty when the option is off)
            let mut h1 = if cf.enabled { vec![0.0; n] } else { Vec::new() };
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
                if cf.enabled {
                    add_consistent_h1(&mut h1, &m, &ctx.links[k], cf.periodic);
                }
                diag_k[k] = m.a.diag.clone();
                used[k] = Some(m);
            }
            for a in a_p.iter_mut() {
                *a /= n_active;
            }
            for h in h1.iter_mut() {
                *h /= n_active;
            }
            let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            // ADR 0214 F0: the consistent correctors' rAtU (`None` = the default correctors)
            let r_t: Option<Vec<f64>> = cf.enabled.then(|| consistent_rau(vol, &a_p, &h1));
            let r_c: &[f64] = r_t.as_deref().unwrap_or(&r_au);
            let gamma = face_interpolate(g, r_c);
            for corr in 0..n_corr {
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
                let mut phi_h = face_flux(g, &hby_a, &hb);
                // ADR 0374 D1 (`immersed_lag`): the homogeneous face flux of the flux faces and the source of the solid cells
                let ib_src = hom.and_then(|ib| ib.reconstruct_face_flux(&hby_a, &mut phi_h, true));
                if let Some(rt) = &r_t {
                    consistent_split(g, pbc_h, &r_au, rt, &p, None, &mut phi_h, &mut hby_a);
                    if s.non_orth_correctors > 0 {
                        // ADR 0258 X1: the non-orthogonal part of the compensation, as in the step
                        for (f, v) in consistent_non_orth(g, pbc_h, &r_au, rt, &p, None, &mut phi_h) {
                            phi_h[f] += v;
                        }
                    }
                }
                let p_old = p.clone();
                // ADR 0388 D1 (`lag_fluid`): the solve started from zero instead of from the error
                let mut p_new = if zero_start { vec![0.0; n] } else { p_old.clone() };
                match &ib_src {
                    None => {
                        solve_pressure(g, &gamma, pbc_h, &phi_h, &mut p_new, &controls);
                    }
                    Some(q) => {
                        crate::pressure::solve_pressure_src(g, &gamma, pbc_h, &phi_h, &mut p_new, &controls, q);
                    }
                }
                let bvn = boundary_values_pbc(g, pbc_h, &p_new);
                let gn = green_gauss(g, &p_new, &bvn, 0);
                for c in 0..n {
                    for k in 0..3 {
                        if g.active[k] {
                            u[c][k] = hby_a[c][k] - r_c[c] * gn[c][k];
                        }
                    }
                }
                let a = if corr + 1 == n_corr { alpha_p } else { 1.0 };
                for c in 0..n {
                    p[c] = p_old[c] + a * (p_new[c] - p_old[c]);
                }
            }
        }
        p
    }

    /// Power-iteration estimate of the spectral radius of the pressure-lag map with `n_corr` correctors (ADR 0073 B4), the cell of
    /// the largest mode entry and the normalised mode. `warm` (same length as the cells) replaces the ADR 0073 start vector (ADR 0156
    /// D3: the search starts each count from the previous count's mode).
    fn lag_amplification_n(
        &self,
        ctx: &LagCtx,
        n_corr: usize,
        iterations: usize,
        warm: Option<&[f64]>,
    ) -> (f64, usize, Vec<f64>) {
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
        // ADR 0374 D1 (`immersed_lag`): the forcing with its flux faces made homogeneous (wall and rigid flux zero)
        let hom: Option<crate::immersed::ImmersedForcing> = match &self.immersed {
            Some(ib) if self.immersed_lag && !ib.flux_faces.is_empty() => {
                let mut h = ib.clone();
                for ff in h.flux_faces.iter_mut() {
                    ff.wall = [0.0; 3];
                    ff.rigid = 0.0;
                }
                Some(h)
            }
            _ => None,
        };
        let lf = self.lag_fluid.as_ref();
        // ADR 0388 D1 (`lag_fluid.fluid_norm`): the weights of the norm, zero in the solid cells
        let fluid_w: Option<Vec<f64>> = match (lf, &self.immersed) {
            (Some(o), Some(ib)) if o.fluid_norm => Some(
                (0..n)
                    .map(|c| if ib.body_of_cell[c].is_none() { vol[c] } else { 0.0 })
                    .collect(),
            ),
            _ => None,
        };
        let (wv, tw): (&[f64], f64) = match &fluid_w {
            Some(w) => (w, w.iter().sum()),
            None => (vol, tv),
        };
        let norm = |e: &mut Vec<f64>| -> f64 {
            if !fixed_level {
                let m = e.iter().zip(wv).map(|(a, v)| a * v).sum::<f64>() / tw;
                e.iter_mut().for_each(|x| *x -= m);
            }
            e.iter().zip(wv).map(|(a, v)| a * a * v).sum::<f64>().sqrt()
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
        let mut e: Vec<f64> = match warm {
            Some(w) if w.len() == n => w.to_vec(),
            _ => cc
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
                .collect(),
        };
        let n0 = norm(&mut e);
        if n0 <= 0.0 || !n0.is_finite() {
            return (0.0, 0, e);
        }
        e.iter_mut().for_each(|x| *x /= n0);
        let mut logs = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let mut next = self.lag_map(ctx, n_corr, &e, &pbc_h, hom.as_ref(), lf);
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
        let cell = match &fluid_w {
            // ADR 0388 D1: the mode cell among the fluid cells
            Some(w) => (0..n)
                .filter(|&c| w[c] > 0.0)
                .max_by(|&a, &b| e[a].abs().total_cmp(&e[b].abs()))
                .unwrap_or(0),
            None => (0..n).max_by(|&a, &b| e[a].abs().total_cmp(&e[b].abs())).unwrap_or(0),
        };
        (rho, cell, e)
    }

    /// The corrector count of an estimate of the adaptive control (ADR 0156 D3.3) on the frozen matrices `ctx`: the first search
    /// counts up from `n_lo` until the estimate is at most the target or `n_hi` is reached; a later one checks the count in use,
    /// counts up from it when above the target, and else (when `allow_down`; ADR 0162 D3) takes one count fewer when that one
    /// is at or below the target.
    fn pick_correctors(
        &self,
        ctx: &LagCtx,
        c: &CorrectorControl,
        n_lo: usize,
        n_hi: usize,
        allow_down: bool,
    ) -> CorrectorPick {
        let first = self.correctors.last_estimate.is_none();
        let mut warm: Option<Vec<f64>> = self.correctors.mode.clone();
        let mut evaluations = 0;
        let mut work = 0;
        let iterations = c.iterations.max(2);
        let mut estimate = |k: usize, warm: &mut Option<Vec<f64>>| -> (f64, usize) {
            let (rho, cell, mode) = self.lag_amplification_n(ctx, k, iterations, warm.as_deref());
            *warm = Some(mode);
            evaluations += 1;
            work += k * iterations;
            (rho, cell)
        };
        let mut n = if first {
            n_lo
        } else {
            self.correctors.current.clamp(n_lo, n_hi)
        };
        let above = |r: f64| r.is_nan() || r > c.target;
        let (mut rho, mut cell) = estimate(n, &mut warm);
        if above(rho) {
            while above(rho) && n < n_hi {
                n += 1;
                (rho, cell) = estimate(n, &mut warm);
            }
        } else if !first && allow_down && n > n_lo {
            let (r1, c1) = estimate(n - 1, &mut warm);
            if r1 <= c.target {
                n -= 1;
                rho = r1;
                cell = c1;
            }
        }
        CorrectorPick {
            n,
            rho,
            cell,
            cap_bound: above(rho),
            evaluations,
            work,
            mode: warm.unwrap_or_default(),
        }
    }

    /// Advance to `t_end` with a fixed step `dt` (the last step is shortened to land on `t_end`; it is marked as a landing step
    /// for the adaptive corrector count, ADR 0162 D2).
    pub fn run_fixed(&mut self, t_end: f64, dt: f64) -> Vec<StepInfo> {
        let mut out = Vec::new();
        while self.time < t_end - 1e-12 * dt.max(1e-300) {
            let d = dt.min(t_end - self.time);
            if d != dt {
                self.correctors.mark_landing_step();
            }
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
/// Implicit partner links of the rotationally periodic faces in the momentum matrix (F-743, ADR 0664): the partner term
/// `coef (R u_c)_k` of a coupled face (row = owner cell `r`, partner owner cell `c`, `R` the patch rotation, `coef` as in
/// [`PeriodicLinks`]) couples component `k` of `r` to every active component `j` of `c` through `coef R_kj`, so the active
/// components form one block system. Built only with `PimpleSettings::rotational_periodic_implicit`.
#[derive(Debug, Clone, Default)]
struct RotationalLinks {
    /// (row, column, coefficient, rotation)
    t: Vec<(usize, usize, f64, [[f64; 3]; 3])>,
    /// Block index of every active component (`None` = inactive).
    block: [Option<usize>; 3],
}

impl RotationalLinks {
    /// The links of the rotational patches from the boundary coefficients `bc` of one active component (the face filter and
    /// the coefficient of a periodic face do not depend on the component).
    fn build(geom: &FvGeometry, u: &VectorField, flux: &[f64], gamma: &[f64], bc: &[crate::bc::FaceCoeffs]) -> Self {
        let mesh = &geom.mesh;
        let n_int = geom.n_internal;
        let (own, sf) = (mesh.owner(), mesh.face_area_vectors());
        let mut t = Vec::new();
        for (pi, p) in mesh.patches().iter().enumerate() {
            let (Some(map), Some(rot)) = (&geom.periodic_partner[pi], &geom.periodic_rot[pi]) else {
                continue;
            };
            if !u.bc(pi).is_coupled() {
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
                    t.push((row, col, coef, *rot));
                }
            }
        }
        let mut block = [None; 3];
        let mut nb = 0;
        for (k, b) in block.iter_mut().enumerate() {
            if geom.active[k] {
                *b = Some(nb);
                nb += 1;
            }
        }
        RotationalLinks { t, block }
    }

    fn is_empty(&self) -> bool {
        self.t.is_empty()
    }

    /// `sum_(j active) R_kj x_j` of the partner velocity `x`.
    fn turned(&self, rot: &[[f64; 3]; 3], k: usize, x: Vec3) -> f64 {
        (0..3)
            .filter(|&j| self.block[j].is_some())
            .map(|j| rot[k][j] * x[j])
            .sum()
    }

    /// Remove the lagged partner term of the active components from the right-hand side of component `k` (`u` = the
    /// iterate the assembly used).
    fn fix_rhs(&self, m: &mut FvMatrix, k: usize, u: &[Vec3]) {
        for (r, c, a, rot) in &self.t {
            m.b[*r] += a * self.turned(rot, k, u[*c]);
        }
    }

    /// The row magnitudes `|coef| sum_(j active) |R_kj|` of component `k`, added to `sums`.
    fn add_magnitudes(&self, sums: &mut [f64], k: usize) {
        for (r, _, a, rot) in &self.t {
            let s: f64 = (0..3)
                .filter(|&j| self.block[j].is_some())
                .map(|j| rot[k][j].abs())
                .sum();
            sums[*r] += a.abs() * s;
        }
    }

    /// Implicit under-relaxation of component `k` with the translational links `lk` and these links in the dominance sum.
    fn relax(&self, lk: &PeriodicLinks, m: &mut FvMatrix, k: usize, alpha: f64, x_old: &[f64]) {
        assert!(alpha > 0.0 && alpha <= 1.0, "relaxation factor must be in (0,1]");
        let mut sums = m.a.off_diag_abs_sum();
        for &(r, _, a) in &lk.t {
            sums[r] += a.abs();
        }
        self.add_magnitudes(&mut sums, k);
        for i in 0..m.a.diag.len() {
            let d = m.a.diag[i].max(sums[i]);
            let dr = d / alpha;
            m.b[i] += (dr - m.a.diag[i]) * x_old[i];
            m.a.diag[i] = dr;
        }
    }

    /// Subtract the links of component `k` with the velocity `u` from the `H` numerator `h`.
    fn fix_h(&self, h: &mut [f64], k: usize, u: &[Vec3]) {
        for (r, c, a, rot) in &self.t {
            h[*r] -= a * self.turned(rot, k, u[*c]);
        }
    }

    /// The block system of the active components: the systems `sys[k]` with their translational links `lk[k]` on the
    /// diagonal blocks and these links between the blocks; the right-hand side and the start vector from `u`.
    fn block_system(
        &self,
        sys: &[Option<FvMatrix>],
        lk: &[PeriodicLinks],
        u: &[Vec3],
    ) -> (rcfd_linalg::CsrMatrix, Vec<f64>, Vec<f64>) {
        let n = u.len();
        let nb = self.block.iter().filter(|b| b.is_some()).count();
        let mut tr = Vec::new();
        let (mut b, mut x) = (vec![0.0; nb * n], vec![0.0; nb * n]);
        for k in 0..3 {
            let (Some(bk), Some(m)) = (self.block[k], &sys[k]) else {
                continue;
            };
            let csr = lk[k].csr(m);
            for r in 0..n {
                for e in csr.row_ptr[r]..csr.row_ptr[r + 1] {
                    tr.push((bk * n + r, bk * n + csr.col[e], csr.val[e]));
                }
                b[bk * n + r] = m.b[r];
                x[bk * n + r] = u[r][k];
            }
            for (r, c, a, rot) in &self.t {
                for j in 0..3 {
                    if let Some(bj) = self.block[j] {
                        if rot[k][j] != 0.0 {
                            tr.push((bk * n + r, bj * n + c, a * rot[k][j]));
                        }
                    }
                }
            }
        }
        (rcfd_linalg::CsrMatrix::from_triplets(nb * n, tr), b, x)
    }
}

/// Add to `h1` the off-diagonal magnitudes of the momentum matrix `m` of one component (ADR 0214 F0) and, with `periodic`, the
/// magnitudes of its implicit periodic link coefficients in their rows (ADR 0223 E2, ADR 0113 F1's rule). The step and the
/// pressure-lag map both use it, so the estimate has the step's algebra.
fn add_consistent_h1(h1: &mut [f64], m: &FvMatrix, links: &PeriodicLinks, periodic: bool) {
    for (h, o) in h1.iter_mut().zip(m.a.off_diag_abs_sum()) {
        *h += o;
    }
    if periodic {
        for &(r, _, a) in &links.t {
            h1[r] += a.abs();
        }
    }
}

/// `rAtU = V/(a_P - H1)` of the consistent correctors (ADR 0214 F0), with the floor of the steady SIMPLEC
/// (`a_P - H1 >= 1e-3 a_P`); `a_p` and `h1` are the component averages.
fn consistent_rau(vol: &[f64], a_p: &[f64], h1: &[f64]) -> Vec<f64> {
    (0..a_p.len())
        .map(|c| vol[c] / (a_p[c] - h1[c]).max(1e-3 * a_p[c]))
        .collect()
}

/// The SIMPLEC split of one consistent corrector (ADR 0214 F0) with the pressure `p_prev` that drove it: adds to `phi_hby_a` the
/// compensation `(rAtU - rAU)_f snGrad(p_prev) |S|` on internal faces and `(rAtU - rAU)_owner gradient_b(p_prev) |S|` on boundary
/// faces whose flux the pressure sets (not fixed-velocity, not periodic, not empty), and to `hby_a` the cell term
/// `-(rAU - rAtU) grad(p_prev)` (Green-Gauss with the boundary values of `pbc`). With `p_new = p_prev` the face flux and the cell
/// velocity of the corrector are those of the default. `dr_face` (ADR 0352 D1, with `rhie_chow_step_consistent`): the face
/// values of the compensation's coefficient in place of `face_interpolate(rAtU - rAU)`; `None` = as before.
#[allow(clippy::too_many_arguments)]
fn consistent_split(
    g: &FvGeometry,
    pbc: &crate::pressure::PressureBc,
    r_au: &[f64],
    r_t: &[f64],
    p_prev: &[f64],
    dr_face: Option<&[f64]>,
    phi_hby_a: &mut [f64],
    hby_a: &mut [Vec3],
) {
    let mesh = &g.mesh;
    let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
    let n_int = g.n_internal;
    let dr_f = match dr_face {
        Some(d) => d.to_vec(),
        None => {
            let dr: Vec<f64> = r_t.iter().zip(r_au).map(|(t, a)| t - a).collect();
            face_interpolate(g, &dr)
        }
    };
    for f in 0..n_int {
        phi_hby_a[f] += dr_f[f] * g.lap_coeff[f] * (p_prev[nei[f]] - p_prev[own[f]]);
    }
    for f in n_int..mesh.n_faces() {
        let i = f - n_int;
        if g.empty[f] || pbc.fixed_u[i] || pbc.coupled[i].is_some() {
            continue;
        }
        phi_hby_a[f] += dr_f[f] * vec3::norm(sf[f]) * pbc.coeffs[i].gradient(p_prev[own[f]]);
    }
    if pbc.coupled.iter().any(|c| c.is_some()) {
        // ADR 0223 E2: periodic faces (reached only with `consistent_periodic`), the steady SIMPLEC's form (ADR 0102): the pair
        // mean of the coefficient and the partner-cell difference over the summed distances, as the pressure equation has them
        let dr_c = crate::pressure::coupled_gamma(g, pbc, &dr_f);
        for f in n_int..mesh.n_faces() {
            let i = f - n_int;
            let Some(q) = pbc.coupled[i] else {
                continue;
            };
            if g.empty[f] || pbc.fixed_u[i] {
                continue;
            }
            let span = g.dn[i] + g.dn[q - n_int];
            phi_hby_a[f] += dr_c[f] * vec3::norm(sf[f]) * (p_prev[own[q]] - p_prev[own[f]]) / span;
        }
    }
    let bv = boundary_values_pbc(g, pbc, p_prev);
    let grad = green_gauss(g, p_prev, &bv, 0);
    for (c, h) in hby_a.iter_mut().enumerate() {
        for k in 0..3 {
            if g.active[k] {
                h[k] -= (r_au[c] - r_t[c]) * grad[c][k];
            }
        }
    }
}

/// The non-orthogonal part of the compensation of one consistent corrector (ADR 0258 X1, as the steady SIMPLEC of ADR 0050) with
/// the pressure `p_prev` that drove it: adds to `phi_hby_a` on every internal face `(rAtU - rAU)_f k_f . grad(p_prev)_f`, the
/// explicit non-orthogonal flux of the flux correction (`k_f = S_f - d_f |S_f|^2/(S_f . d_f)`, the least-squares gradient
/// interpolated with the face weights), and returns, per fixed-pressure boundary face, `(rAtU - rAU)_f k_f . grad(p_prev)_owner`:
/// the non-orthogonal part the pressure equation has there and the flux correction has not, to be added to the flux divergence
/// of the pressure equation only. With `p_new = p_prev` the face flux of the corrector is that of the default and the pressure
/// equation of a pass has the default's solution. Without pressure gradient options and periodic faces. `dr_face` as in
/// [`consistent_split`].
fn consistent_non_orth(
    g: &FvGeometry,
    pbc: &crate::pressure::PressureBc,
    r_au: &[f64],
    r_t: &[f64],
    p_prev: &[f64],
    dr_face: Option<&[f64]>,
    phi_hby_a: &mut [f64],
) -> Vec<(usize, f64)> {
    let mesh = &g.mesh;
    let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
    let n_int = g.n_internal;
    let dr_f = match dr_face {
        Some(d) => d.to_vec(),
        None => {
            let dr: Vec<f64> = r_t.iter().zip(r_au).map(|(t, a)| t - a).collect();
            face_interpolate(g, &dr)
        }
    };
    let gr = least_squares(g, p_prev, &boundary_values_pbc(g, pbc, p_prev));
    let k_of = |f: usize| {
        let d = g.delta[f];
        vec3::sub(sf[f], vec3::scale(d, vec3::dot(sf[f], sf[f]) / vec3::dot(sf[f], d)))
    };
    for f in 0..n_int {
        if g.empty[f] {
            continue;
        }
        let w = g.weights[f];
        let gf = [0, 1, 2].map(|i| w * gr[own[f]][i] + (1.0 - w) * gr[nei[f]][i]);
        phi_hby_a[f] += dr_f[f] * vec3::dot(k_of(f), gf);
    }
    let mut eq_only = Vec::new();
    for f in n_int..mesh.n_faces() {
        let i = f - n_int;
        if g.empty[f] || pbc.coupled[i].is_some() || pbc.coeffs[i].a != 0.0 {
            continue;
        }
        eq_only.push((f, dr_f[f] * vec3::dot(k_of(f), gr[own[f]])));
    }
    eq_only
}

/// F-754 (ADR 0675): `(f, gamma_f D_f)` of every coupled face `f`, the internal faces' transient Rhie-Chow term with the
/// partner cell `c` in the place of the neighbour: `gamma_f = w rAU_r + (1 - w) rAU_c` (the pair mean of the pressure
/// equation) and `D_f` of [`coupled_ddt_defect`].
fn coupled_ddt_term(
    g: &FvGeometry,
    pbc: &PressureBc,
    hist_u: &[History; 3],
    phi_hist: &History,
    gamma_p: &[f64],
    c1: f64,
    c2: f64,
) -> Vec<(usize, f64)> {
    let gamma = coupled_gamma(g, pbc, gamma_p);
    coupled_ddt_defect(g, pbc, hist_u, phi_hist, c1, c2)
        .into_iter()
        .map(|(f, d)| (f, gamma[f] * d))
        .collect()
}

/// F-754 (ADR 0675), F-757 (ADR 0687): `(f, D_f)` of every coupled face `f`, the old-level face defect
/// `D_f = c1 (phi_old - U_old_f . S) - c2 (phi_older - U_older_f . S)` with `U_f = w U_r + (1 - w) R U_c`, the face value
/// `boundary_hby_a` gives the coupled faces' `HbyA`. Shared by the PISO step and the fractional step.
fn coupled_ddt_defect(
    g: &FvGeometry,
    pbc: &PressureBc,
    hist_u: &[History; 3],
    phi_hist: &History,
    c1: f64,
    c2: f64,
) -> Vec<(usize, f64)> {
    let n = g.mesh.n_cells();
    let n_int = g.n_internal;
    let sf = g.mesh.face_area_vectors();
    let zero_b = vec![[0.0; 3]; g.mesh.n_boundary_faces()];
    let level = |older: bool| -> Vec<Vec3> {
        let u: Vec<Vec3> = (0..n)
            .map(|c| {
                [0, 1, 2].map(|k| match (older, &hist_u[k].older) {
                    (true, Some(o)) => o[c],
                    _ => hist_u[k].old[c],
                })
            })
            .collect();
        boundary_hby_a(g, pbc, &zero_b, &u)
    };
    let bo = level(false);
    let bol = (c2 != 0.0).then(|| level(true));
    let phi_older = phi_hist.older.as_ref().unwrap_or(&phi_hist.old);
    let mut out = Vec::new();
    for (i, cp) in pbc.coupled.iter().enumerate() {
        let f = n_int + i;
        if cp.is_none() || g.empty[f] {
            continue;
        }
        let mut d = c1 * (phi_hist.old[f] - vec3::dot(bo[i], sf[f]));
        if let Some(b) = &bol {
            d -= c2 * (phi_older[f] - vec3::dot(b[i], sf[f]));
        }
        out.push((f, d));
    }
    out
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
        self.correctors.reset_schedule();
        self.ale_bdf2 = None;
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
        self.correctors.reset_schedule();
        self.ale_bdf2 = None;
        self.start_projected = false;
        self.outer_firsts.clear();
        Ok(())
    }

    /// Opt-in (F-515, ADR 0425 D2): project the start velocity onto the fluid region of the immersed forcing
    /// ([`crate::immersed_start::project_start`]: the solid cells at rest, the fluid-solid faces closed, the face flux
    /// divergence-free) and start afresh from it with [`reinitialise`](Self::reinitialise), passing the projected face flux;
    /// the pressure and the time are kept. With a projected start the refusal of a reversed flux face with a single PISO
    /// corrector (ADR 0374 D2) is lifted (ADR 0421: the stop it guards against comes from a start with flow through the body).
    /// Fails without a forcing, after the first step, or when the projection fails.
    pub fn project_immersed_start(&mut self) -> rcfd_core::Result<crate::immersed_start::StartProjection> {
        let ib = self.immersed.as_ref().ok_or_else(|| rcfd_core::Error::InvalidValue {
            path: "immersed start projection".into(),
            reason: "the solver has no immersed forcing".into(),
        })?;
        if self.step_no != 0 {
            return Err(rcfd_core::Error::InvalidValue {
                path: "immersed start projection".into(),
                reason: format!(
                    "the solver has taken {} steps: project the start before the first step",
                    self.step_no
                ),
            });
        }
        let st = crate::immersed_start::project_start(&self.geom, &self.u, &self.p, ib)?;
        let p = self.p.values.clone();
        self.reinitialise(st.u, p, Some(st.phi), self.time)?;
        self.start_projected = true;
        Ok(st.report)
    }

    /// True when the start was projected by [`project_immersed_start`](Self::project_immersed_start) and the solver has not
    /// been reinitialised since (ADR 0425 D3).
    pub fn start_projected(&self) -> bool {
        self.start_projected
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

    /// D1 of ADR 0664 (F-743): on the doubly periodic Taylor-Green box (translational pairs only) the opt-in
    /// `rotational_periodic_implicit` is a no-op: five PISO steps give the same velocity, pressure and flux bit for bit.
    #[test]
    fn rotational_periodic_implicit_is_a_no_op_without_rotational_pairs() {
        let run = |on: bool| {
            let mut s = taylor_green(16, 0.05, |mut st| {
                st.convection = ConvScheme::Central;
                st.rotational_periodic_implicit = on;
                st
            });
            for _ in 0..5 {
                s.step(0.02);
            }
            s
        };
        let (a, b) = (run(false), run(true));
        let bits = |s: &PimpleSolver| -> Vec<u64> {
            let mut v: Vec<u64> = s.u.values.iter().flat_map(|u| u.iter().map(|x| x.to_bits())).collect();
            v.extend(s.p.values.iter().map(|x| x.to_bits()));
            v.extend(s.phi.iter().map(|x| x.to_bits()));
            v
        };
        assert_eq!(
            bits(&a),
            bits(&b),
            "D1: the setting changed a run without rotational pairs"
        );
    }

    /// U1 of ADR 0667 (F-744): on the doubly periodic Taylor-Green box (translational pairs, an orthogonal mesh, so `k = 0`
    /// to rounding on every coupled face) `coupled_non_orth` with two non-orthogonal correctors changes the velocity and the
    /// pressure of five PISO steps by at most `1e-12`.
    #[test]
    fn coupled_non_orth_on_an_orthogonal_periodic_box() {
        let run = |on: bool| {
            let mut s = taylor_green(16, 0.05, |mut st| {
                st.convection = ConvScheme::Central;
                st.non_orth_correctors = 2;
                st.coupled_non_orth = on;
                st
            });
            for _ in 0..5 {
                s.step(0.02);
            }
            s
        };
        let (a, b) = (run(false), run(true));
        let du =
            a.u.values
                .iter()
                .zip(&b.u.values)
                .flat_map(|(x, y)| (0..3).map(move |k| (x[k] - y[k]).abs()))
                .fold(0.0f64, f64::max);
        let dp =
            a.p.values
                .iter()
                .zip(&b.p.values)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0f64, f64::max);
        let cn = CoupledNonOrth::build(&b.geom, |pi| b.u.bc(pi).is_coupled());
        let kmax = cn.geometry().iter().map(|e| vec3::norm(e.2)).fold(0.0f64, f64::max);
        println!(
            "U1: {} coupled faces, largest |k| {kmax:.2e}, velocity {du:.2e}, pressure {dp:.2e}",
            cn.geometry().len()
        );
        assert!(!cn.is_empty(), "U1: the box has coupled faces");
        assert!(
            du <= 1e-12 && dp <= 1e-12,
            "U1: velocity {du:.3e}, pressure {dp:.3e} (1e-12)"
        );
    }
}
