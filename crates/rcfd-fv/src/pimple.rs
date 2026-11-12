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
    boundary_hby_a, boundary_values_pbc, corrected_flux, coupled_gamma, divergence, face_interpolate, pressure_bc,
    solve_pressure, PressureControls,
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
    /// Make the pressure-lag check (`pressure_lag_check`) and the adaptive corrector count (`corrector_control`) also on a
    /// moving-mesh step (`step_ale`, `step_ale_with_walls`, `step_sliding`; ADR 0223 E0), on that step's own frozen matrices (the
    /// moved geometry, the relative flux, the step's wall conditions made homogeneous), exactly as on a fixed mesh. Default `false`:
    /// a moving-mesh step makes neither (as before).
    pub moving_mesh_lag: bool,
    /// The face coefficient of the pressure term that the transient Rhie-Chow carry-over is consistent with (ADR 0245 C): the
    /// harmonic face value of `rAU` and the commutator of the lagged pressure gradient on internal faces, so that the converged
    /// face flux depends on the step size only through the time scheme. Acts in `step` on a mesh at rest and in `OversetSolver`;
    /// no effect (the default step) with `consistent_correctors`, on the library's moving-mesh steps and in the fractional step;
    /// coupled faces, face jumps and the immersed-solid flux are left as they are. Default `false` (bit-identical).
    pub rhie_chow_step_consistent: bool,
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
    /// ADR 0050. Fixed meshes without pressure gradient options and without periodic faces.
    pub non_orthogonal: bool,
    /// Accept the component diagonal (`PimpleSettings::component_diagonal`; ADR 0258 X2); `rAtU` keeps the component averages.
    /// Fixed meshes without periodic faces.
    pub component_diagonal: bool,
    /// Accept face jumps (`PimpleSolver::face_jump` and the hook's; ADR 0258 X3): a jump face compensates the old pressure
    /// difference less the old total rise and takes `interp(rAtU)` in its resistance factor. Fixed meshes without periodic faces.
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
                "no steady identity is registered for this extension with periodic faces"
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
        } else if self.quantity == "consistent correctors" {
            // ADR 0258 V2: the located part (flag, other setting, reason) after the sentence of ADR 0214
            let located =
                ConsistentRefusal::from_stage(self.stage).map_or(String::new(), |r| format!(" ({})", r.message()));
            return format!(
                "PIMPLE consistent correctors (ADR 0214) are not defined with the {}: the run is stopped at step \
                 {}{outer}{located}",
                self.stage, self.step
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
            moving_mesh_lag: false,
            rhie_chow_step_consistent: false,
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
            fs_q: Vec::new(),
            diverged: None,
            lag_estimates: Vec::new(),
            lag_warnings: Vec::new(),
            correctors: CorrectorRecord::default(),
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
            fs_q: self.fs_q.clone(),
            diverged: None,
            lag_estimates: Vec::new(),
            lag_warnings: Vec::new(),
            correctors: CorrectorRecord::default(),
        }
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
        if let Some(nu) = nu_face {
            s.nu_face = Some(nu.to_vec());
        }
        s.settings.corrector_control = None;
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
        flags.validate(&self.consistent_context(&flags)).err()
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
                if let Some(nu) = up.nu_face {
                    // ADR 0199 D2: the face viscosity of this outer iteration
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
                let found = cf.conflicts(&self.consistent_context(&cf));
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
            for k in 0..3 {
                if !g.active[k] {
                    continue;
                }
                let cells: Vec<f64> = u_iter.iter().map(|v| v[k]).collect();
                let bc = self.u.boundary_coeffs(&g, k);
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
                // ADR 0183 P-A (opt-in): the limited gradient in the convection term only
                let limited: Option<Vec<Vec3>> = match (s.linear_upwind_limiter, &grad) {
                    (Some(kind), Some(gr)) if s.convection == ConvScheme::LinearUpwind => {
                        let psi = crate::gradient::limiter(&g, &conv_cells, &bv, gr, kind);
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
                    limited.as_deref().or(grad.as_deref()),
                );
                // ADR 0204 K3: the gradient options of the momentum Laplacian, with the values the gradient was computed from
                m.add(&laplacian_opt(
                    &g,
                    &gamma_nu,
                    &bc,
                    grad.as_deref(),
                    &conv_cells,
                    &bv,
                    s.gradient_momentum,
                ));
                // periodic partner values implicit (ADR 0046): move the lagged partner term back into the matrix
                let lk = PeriodicLinks::build(&g, &self.u, &phi_conv, &gamma_nu, &bc);
                let q_lag: Vec<f64> = u_iter.iter().map(|v| v[k]).collect();
                lk.fix_rhs(&mut m, &q_lag);
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
                    a.fix_ddt(&mut m, &vol, &self.hist_u[k].old, dt);
                }
                if let Some(ts) = &transposed {
                    for c in 0..n {
                        m.b[c] += vol[c] * ts[c][k];
                    }
                }
                if alpha_u < 1.0 {
                    lk.relax(&mut m, alpha_u, &cells);
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
                if cf.enabled {
                    add_consistent_h1(&mut h1, &m, &lk, cf.periodic);
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
                });
            }
            if let (Some(c), Some(reason), true) = (&cc, cc_reason, last) {
                // ADR 0156 D3: choose the count on the frozen matrices of this (final) outer iteration, before its correctors
                let ctx = LagCtx {
                    mats: mats.clone(),
                    links: links.clone(),
                    pbc: pbc.clone(),
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
            let r_au: Vec<f64> = (0..n).map(|c| vol[c] / a_p[c]).collect();
            // ADR 0245 C (opt-in): the face coefficient the carry-over is consistent with
            let rc = s.rhie_chow_step_consistent && !s.consistent_correctors && self.ale.is_none();
            let gamma_p = if rc {
                face_consistent::harmonic_face(&g, &r_au)
            } else {
                face_interpolate(&g, &r_au)
            };
            let gamma_bf = (s.coupled_body_pair_mean && body.is_some()).then(|| coupled_gamma(&g, &pbc, &gamma_p));
            // ADR 0214 F0: rAtU of the consistent correctors and its face values (`None` = the default correctors)
            let r_t: Option<Vec<f64>> = cf.enabled.then(|| consistent_rau(&vol, &a_p, &h1));
            let gamma_t: Option<Vec<f64>> = r_t.as_ref().map(|r| face_interpolate(&g, r));

            // ---- pressure correctors ---------------------------------------
            for corr in 0..n_corr {
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
                    // ADR 0214 F0: the SIMPLEC split of this corrector with the pressure that drove it
                    consistent_split(&g, &pbc, &r_au, rt, &self.p.values, &mut phi_hby_a, &mut hby_a);
                    if s.non_orth_correctors > 0 {
                        // ADR 0258 X1 (reached only with `non_orthogonal`): the non-orthogonal part of the compensation
                        eq_only = consistent_non_orth(&g, &pbc, &r_au, rt, &self.p.values, &mut phi_hby_a);
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
                let (r0, it) = match s.gradient_pressure {
                    None => match &ib_src {
                        None => solve_pressure(&g, &gamma_j, &pbc, phi_solve, &mut p_new, &controls),
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
                    None => corrected_flux(&g, &gamma_j, &pbc, &phi_hby_a, &p_new, &ub, s.non_orth_correctors > 0),
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
            let (rho, cell, _) = self.lag_amplification_n(ctx, n_corr_final, chk.iterations.max(2), None);
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

    /// One application of the linearised pressure-lag map of a step (ADR 0073 B4): the pressure error `e` drives the momentum predictors
    /// and the `n_corr` correctors of every outer iteration with the frozen matrices of `ctx`; returns the pressure error at the end of
    /// the step.
    fn lag_map(&self, ctx: &LagCtx, n_corr: usize, e: &[f64], pbc_h: &crate::pressure::PressureBc) -> Vec<f64> {
        let g = &self.geom;
        let s = &self.settings;
        let cf = s.consistent_flags();
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
                if let Some(rt) = &r_t {
                    consistent_split(g, pbc_h, &r_au, rt, &p, &mut phi_h, &mut hby_a);
                    if s.non_orth_correctors > 0 {
                        // ADR 0258 X1: the non-orthogonal part of the compensation, as in the step
                        for (f, v) in consistent_non_orth(g, pbc_h, &r_au, rt, &p, &mut phi_h) {
                            phi_h[f] += v;
                        }
                    }
                }
                let p_old = p.clone();
                let mut p_new = p_old.clone();
                solve_pressure(g, &gamma, pbc_h, &phi_h, &mut p_new, &controls);
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
            let mut next = self.lag_map(ctx, n_corr, &e, &pbc_h);
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
/// velocity of the corrector are those of the default.
fn consistent_split(
    g: &FvGeometry,
    pbc: &crate::pressure::PressureBc,
    r_au: &[f64],
    r_t: &[f64],
    p_prev: &[f64],
    phi_hby_a: &mut [f64],
    hby_a: &mut [Vec3],
) {
    let mesh = &g.mesh;
    let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
    let n_int = g.n_internal;
    let dr: Vec<f64> = r_t.iter().zip(r_au).map(|(t, a)| t - a).collect();
    let dr_f = face_interpolate(g, &dr);
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
/// equation of a pass has the default's solution. Without pressure gradient options and periodic faces.
fn consistent_non_orth(
    g: &FvGeometry,
    pbc: &crate::pressure::PressureBc,
    r_au: &[f64],
    r_t: &[f64],
    p_prev: &[f64],
    phi_hby_a: &mut [f64],
) -> Vec<(usize, f64)> {
    let mesh = &g.mesh;
    let (own, nei, sf) = (mesh.owner(), mesh.neighbour(), mesh.face_area_vectors());
    let n_int = g.n_internal;
    let dr: Vec<f64> = r_t.iter().zip(r_au).map(|(t, a)| t - a).collect();
    let dr_f = face_interpolate(g, &dr);
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
