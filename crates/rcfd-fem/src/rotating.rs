//! Modes of a rotating part: spin softening, modes over speed, mode tracking, Campbell table. Declared by ADR 0291; filled
//! by ADR 0403 (the registration of ADR 0299, F-396).
//!
//! A part rotating at the constant speed `Omega` about the unit axis `a` is analysed in the frame that rotates with it
//! (ADR 0403 D1). The kinetic energy `1/2 int rho |u_t + Omega a x (X - p + u)|^2` gives, beside the mass and the centrifugal
//! body force, the term `1/2 Omega^2 int rho |a x u|^2`, a negative stiffness, the spin softening `-Omega^2 M_perp` with
//! `M_perp[(a, i), (b, j)] = m_ab (delta_ij - a_i a_j)` ([`spin_softening_mass`]). The centrifugal pre-stress is linear in
//! `Omega^2`: one static solve at 1 rad/s gives `K_sigma,1` ([`crate::geometric`]), and
//!
//! `K_eff(Omega) phi = omega^2 M phi`, `K_eff = K + Omega^2 (K_sigma,1 - M_perp)`.
//!
//! The Coriolis (gyroscopic) term is neglected (ADR 0403 D6). At `Omega = 0` the result is that of
//! [`crate::modal::modal_analysis`] itself. At `Omega > 0` the reduced `K_eff` is tested by its inertia first: a speed at
//! which it is not positive definite is reported as [`SpeedModes::LossOfStiffness`] with the speed (D3), never as a
//! frequency. [`RotatingPart::campbell`] solves a list of speeds, tracks the modes by the mass-weighted modal assurance (D4)
//! and finds the crossings with the engine orders by interpolation linear in `Omega^2` ([`engine_order_crossings`]); the
//! table is written as CSV and JSON.
//!
//! ADR 0411 (F-499): the Coriolis (gyroscopic) matrix `G = 2 Omega int rho N_a N_b [a x]` ([`coriolis_matrix`]) and the
//! whirl modes of `(K_eff - omega^2 M + i omega G) phi = 0` by a symmetric linearisation ([`RotatingPart::whirl_modes`],
//! [`RotatingSector::whirl_modes`]), forward and backward whirl, the Campbell table with directions. Opt-in by
//! [`RotatingOptions::coriolis`]; nothing above reads it.
//!
//! ADR 0416 (F-505): at any speed, also above the loss of stiffness, the whirl problem by the eigenvalues of its state
//! matrix (Hessenberg reduction and the Francis QR algorithm), each speed flagged stable or unstable with its growth rate
//! ([`RotatingPart::whirl_state`], [`RotatingPart::whirl_stability`]); the functions above are not changed.
//!
//! ADR 0420 (F-511): the same on a cyclic sector, per harmonic index on its real cos/sin problem, with the flag and the growth
//! rate per index and the rotor's flag from them ([`RotatingSector::whirl_state`], [`RotatingSector::whirl_stability`]).
//!
//! ADR 0427 (F-506): the lowest whirl modes of a large part by a shift-invert Lanczos process on the state form of ADR 0411
//! with a sparse factor of `K_eff` ([`gyroscopic_eigen_sparse`]), without the dense cap; opt-in by
//! [`RotatingOptions::whirl_solver`] (default [`WhirlSolver::Dense`], the solver of ADR 0411).
//!
//! ADR 0428 (F-519): the whirl state above the loss of stiffness of a large part or sector index by an isotropic shift-invert
//! Arnoldi process on `(C^2 + sigma)^-1` of the state matrix, one indefinite skyline factor of the real form of `Q(i
//! sqrt(sigma))` (diagonal blocks `K_eff - sigma M`), the eigenvalues nearest the shift with ADR 0416's flag on them
//! ([`RotatingPart::whirl_state_sparse`], [`RotatingSector::whirl_state_sparse`], [`gyroscopic_state_eigen_sparse`]);
//! opt-in by calling them.
//!
//! ADR 0459 (F-542): a part of the orthotropic or the tabulated material of ADR 0452 (solid elements) takes the per-point
//! stiffness, the stiffness of a table at the model's temperature field, also in the static solve of the pre-stress at 1
//! rad/s, which has no thermal strain; a sector or a cyclic constraint with either material is refused.
//!
//! ADR 0462 (F-544): a sector or a cyclic constraint with either material is taken when every material with its own axes has
//! cylindrical axes about the cyclic axis and origin ([`crate::mat_ortho::check_cyclic_axes`]); other axes stay refused.

use crate::assemble::{assemble_with, point_stiffness};
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce};
use crate::element::{element_for, ElementMatrix};
use crate::error::{FemError, Result};
use crate::geometric::{assemble_geometric, Prestress};
use crate::mass::{consistent_with, element_mass};
use crate::modal::{modal_analysis, ModalOptions, ModalResult};
use crate::model::{Load, SolidModel};
use crate::mpc::Transformation;
use crate::solve::{solve_static, StaticOptions};
use rcfd_core::vec3::{norm, Vec3};
use rcfd_linalg::eigen::{count_below, lowest_eigenpairs, EigenError, Shift, SubspaceOptions};
use rcfd_linalg::skyline::SkylineOrdering;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::fmt::Write as _;

mod whirl;
pub use whirl::{
    coriolis_matrix, gyroscopic_eigen_dense, stationary_frequency, whirl_order_crossings, CyclicWhirlCampbell,
    DenseWhirl, HarmonicWhirlSpeedModes, Whirl, WhirlCampbell, WhirlMode, WhirlModes, WhirlSpeedModes,
    MAX_WHIRL_UNKNOWNS, WHIRL_TOLERANCE,
};
pub use whirl::{gyroscopic_eigen_sparse, gyroscopic_eigen_sparse_with, LanczosOptions, SparseWhirl, WhirlSolver};
pub use whirl::{
    gyroscopic_state_eigen_dense, DenseStateWhirl, Stability, StateWhirl, StateWhirlMode, WhirlStability,
    STABILITY_TOLERANCE,
};
pub use whirl::{gyroscopic_state_eigen_sparse, SparseStateEigen, SparseStateOptions, SparseStateWhirl};
pub use whirl::{CyclicWhirlStability, HarmonicStateWhirl};

fn unit_axis(axis: Vec3) -> Result<Vec3> {
    let len = norm(axis);
    if !(len.is_finite() && len > 0.0) || axis.iter().any(|c| !c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "axis",
            reason: format!("{axis:?} is not a finite non-zero direction"),
        });
    }
    Ok([axis[0] / len, axis[1] / len, axis[2] / len])
}

/// The spin-softening mass `M_perp[(a, i), (b, j)] = m_ab (delta_ij - a_i a_j)` of a rotation about `axis` (ADR 0403 D1),
/// `m_ab = int rho N_a N_b` with each element's mass rule (the scalar of [`crate::mass::consistent`]), on the stiffness
/// pattern, with `threads` workers (`None`: the available parallelism); bit-identical for any value. A zero or non-finite
/// axis is refused (key `"axis"`).
pub fn spin_softening_mass(model: &SolidModel, axis: Vec3, threads: Option<usize>) -> Result<CsrMatrix> {
    let a = unit_axis(axis)?;
    let mut p = [[0.0; 3]; 3];
    for (i, row) in p.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = if i == j { 1.0 } else { 0.0 } - a[i] * a[j];
        }
    }
    assemble_with(model, threads, |ctx| {
        // ADR 0473 D3: a Beam2, Shell4 or Shell3 element takes its own kinematics.
        if crate::geometric_struct::is_structural(ctx.kind) {
            return crate::geometric_struct::element_spin_softening_mass(
                model.mesh(),
                ctx.element,
                ctx.coords,
                ctx.material,
                a,
            );
        }
        let m = element_mass(element_for(ctx.kind), ctx.coords, ctx.material.density())?;
        let size = m.size();
        let n = size / 3;
        let mut out = ElementMatrix::zeros(size);
        let src = m.values();
        let v = out.values_mut();
        for ia in 0..n {
            for ib in 0..n {
                let mab = src[(3 * ia) * size + 3 * ib];
                for i in 0..3 {
                    for j in 0..3 {
                        v[(3 * ia + i) * size + 3 * ib + j] = mab * p[i][j];
                    }
                }
            }
        }
        Ok(out)
    })
}

/// Options of a rotating modal analysis.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct RotatingOptions {
    /// The options of each modal analysis (modes, tolerance, residual test, shift, threads). The default shift at a speed
    /// above zero is `Shift::Value(0.0)`, after the inertia test has found `K_eff` positive definite.
    pub modal: ModalOptions,
    /// Whether the geometric stiffness of the centrifugal pre-stress is included (default true).
    pub stress_stiffening: bool,
    /// Whether the spin softening `-Omega^2 M_perp` is included (default true).
    pub spin_softening: bool,
    /// The options of the static solve of the pre-stress at 1 rad/s.
    pub static_options: StaticOptions,
    /// Whether the whirl analyses ([`RotatingPart::whirl_modes`] and the like) include the Coriolis matrix (default false;
    /// ADR 0411). [`RotatingPart::modes`] and [`RotatingPart::campbell`] never include it.
    pub coriolis: bool,
    /// The eigensolver of the whirl analyses at speeds above zero (default [`WhirlSolver::Dense`], ADR 0411; ADR 0427).
    pub whirl_solver: WhirlSolver,
}

impl RotatingOptions {
    /// The lowest `modes` modes, both effects, the default modal and static options.
    pub fn new(modes: usize) -> RotatingOptions {
        RotatingOptions {
            modal: ModalOptions::new(modes),
            stress_stiffening: true,
            spin_softening: true,
            static_options: StaticOptions::default(),
            coriolis: false,
            whirl_solver: WhirlSolver::Dense,
        }
    }
    /// The same options with other modal options.
    pub fn modal(mut self, modal: ModalOptions) -> RotatingOptions {
        self.modal = modal;
        self
    }
    /// The same options with the stress stiffening switched on or off.
    pub fn stress_stiffening(mut self, on: bool) -> RotatingOptions {
        self.stress_stiffening = on;
        self
    }
    /// The same options with the spin softening switched on or off.
    pub fn spin_softening(mut self, on: bool) -> RotatingOptions {
        self.spin_softening = on;
        self
    }
    /// The same options with the Coriolis matrix of the whirl analyses switched on or off (ADR 0411).
    pub fn coriolis(mut self, on: bool) -> RotatingOptions {
        self.coriolis = on;
        self
    }
    /// The same options with another eigensolver of the whirl analyses (ADR 0427).
    pub fn whirl_solver(mut self, solver: WhirlSolver) -> RotatingOptions {
        self.whirl_solver = solver;
        self
    }
}

/// A speed at which the reduced `K_eff` is not positive definite (ADR 0403 D3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LossOfStiffness {
    /// The speed, rad/s.
    pub speed: f64,
    /// The number of negative eigenvalues of `K_eff` on the admissible motions (the negative pivots of its factor);
    /// `None` when the factorisation met a zero pivot (a speed at the loss itself).
    pub negative: Option<usize>,
}

/// The result of the modal analysis at one speed.
#[derive(Debug, Clone, PartialEq)]
pub enum SpeedModes {
    /// The modes of `K_eff phi = omega^2 M phi`.
    Modes(ModalResult),
    /// `K_eff` is not positive definite at this speed: no frequency is computed.
    LossOfStiffness(LossOfStiffness),
}

impl SpeedModes {
    /// The modes, if the speed has them.
    pub fn modes(&self) -> Option<&ModalResult> {
        match self {
            SpeedModes::Modes(r) => Some(r),
            SpeedModes::LossOfStiffness(_) => None,
        }
    }
    /// The loss of stiffness, if the speed has one.
    pub fn loss(&self) -> Option<&LossOfStiffness> {
        match self {
            SpeedModes::Modes(_) => None,
            SpeedModes::LossOfStiffness(l) => Some(l),
        }
    }
}

enum Path {
    Elimination { fixed: Vec<(usize, f64)> },
    Transformation(Box<Transformation>),
}

/// A model prepared for modal analyses at any speed about one axis: the pre-stress at 1 rad/s and the matrices `K` (with
/// the supports on the transformation path), `K_sigma,1`, `M` and `M_perp`, assembled once (ADR 0403 D1, D5).
pub struct RotatingPart<'a> {
    model: &'a SolidModel,
    options: RotatingOptions,
    prestress: Prestress,
    k: CsrMatrix,
    kg1: CsrMatrix,
    m: CsrMatrix,
    m_perp: CsrMatrix,
    path: Path,
    /// The unit axis (ADR 0411).
    axis: Vec3,
}

/// A copy of `model` whose only load is the centrifugal load at 1 rad/s. With an orthotropic or tabulated material (ADR 0459
/// D2) the copy also has the model's temperature field, for the stiffness of a table, and every material without its thermal
/// expansion: the pre-stress has no thermal strain.
fn unit_centrifugal_model(model: &SolidModel, axis: Vec3, origin: Vec3) -> Result<SolidModel> {
    let mut copy = SolidModel::new(model.mesh().clone());
    let points = crate::mat_ortho::uses_point_materials(model);
    for (set, material) in model.materials() {
        if points {
            copy.add_material(set, crate::mat_ortho::without_expansion(material))?;
        } else {
            copy.add_material(set, material.clone())?;
        }
    }
    for c in model.constraints() {
        copy.add_constraint(c.clone())?;
    }
    for s in model.supports() {
        copy.add_support(s.clone())?;
    }
    copy.add_load(Load::centrifugal(axis, origin, 1.0))?;
    if points {
        if let Some((nodal, reference)) = crate::thermal::temperature_field(model)? {
            copy.add_load(Load::temperature(nodal.to_vec(), reference))?;
        }
    }
    Ok(copy)
}

impl<'a> RotatingPart<'a> {
    /// Prepares `model` rotating about `axis` through `origin`: one static solve of the centrifugal load at 1 rad/s with the
    /// model's constraints and supports (its own loads are not part of the pre-stress), then the four matrices. A zero or
    /// non-finite axis (`"axis"`), a non-finite origin (`"origin"`) and a non-zero constraint value are refused.
    pub fn new(model: &'a SolidModel, axis: Vec3, origin: Vec3, options: &RotatingOptions) -> Result<RotatingPart<'a>> {
        check_frame(axis, origin)?;
        check_model(model)?;
        let prestress = if options.stress_stiffening {
            let copy = unit_centrifugal_model(model, axis, origin)?;
            let r = solve_static(&copy, &options.static_options)?;
            Prestress::from_static(&copy, &r)?
        } else {
            Prestress::uniform(model, [0.0; 6])?
        };
        RotatingPart::with_unit_prestress(model, axis, origin, prestress, options)
    }

    /// As [`RotatingPart::new`] with a pre-stress the caller computed at 1 rad/s (for example of a sector), which must fit
    /// the model (`"prestress"`).
    pub fn with_unit_prestress(
        model: &'a SolidModel,
        axis: Vec3,
        origin: Vec3,
        prestress: Prestress,
        options: &RotatingOptions,
    ) -> Result<RotatingPart<'a>> {
        check_frame(axis, origin)?;
        check_model(model)?;
        prestress.check_fits(model)?;
        let threads = options.modal.threads;
        // ADR 0459 D1: the per-point stiffness (the orthotropic and the tabulated material at each point).
        let mut k = point_stiffness(model, threads)?;
        let path = if crate::mpc::uses_transformation(model) {
            crate::mpc::add_supports(&mut k, model)?;
            Path::Transformation(Box::new(crate::mpc::transformation(model, &[])?))
        } else {
            Path::Elimination {
                fixed: prescribed_equations(model, &[])?,
            }
        };
        let kg1 = assemble_geometric(model, &prestress, threads)?;
        let m = consistent_with(model, threads)?;
        let m_perp = spin_softening_mass(model, axis, threads)?;
        Ok(RotatingPart {
            model,
            options: options.clone(),
            prestress,
            k,
            kg1,
            m,
            m_perp,
            path,
            axis: unit_axis(axis)?,
        })
    }

    /// The pre-stress at 1 rad/s.
    pub fn unit_prestress(&self) -> &Prestress {
        &self.prestress
    }
    /// The consistent mass `M` (the model's equations).
    pub fn mass(&self) -> &CsrMatrix {
        &self.m
    }
    /// The spin-softening mass `M_perp`.
    pub fn spin_softening(&self) -> &CsrMatrix {
        &self.m_perp
    }
    /// The geometric stiffness of the pre-stress at 1 rad/s, `K_sigma,1`.
    pub fn unit_geometric_stiffness(&self) -> &CsrMatrix {
        &self.kg1
    }

    /// `K + Omega^2 (s K_sigma,1 - c M_perp)` on the stiffness pattern, `s` and `c` 1 or 0 by the options; on the
    /// transformation path `K` includes the supports.
    pub fn operator(&self, omega: f64) -> CsrMatrix {
        let w2 = omega * omega;
        let s = if self.options.stress_stiffening { 1.0 } else { 0.0 };
        let c = if self.options.spin_softening { 1.0 } else { 0.0 };
        let mut out = self.k.clone();
        for ((v, g), mp) in out.val.iter_mut().zip(&self.kg1.val).zip(&self.m_perp.val) {
            *v += w2 * (s * g - c * mp);
        }
        out
    }

    /// `phi^T K_sigma,1 phi / phi^T M phi`: the Rayleigh quotient of the centrifugal tension at 1 rad/s.
    pub fn stiffening_quotient(&self, phi: &[f64]) -> f64 {
        quad(&self.kg1, phi) / quad(&self.m, phi)
    }

    /// `phi^T (s K_sigma,1 - c M_perp) phi / phi^T M phi`: at a mode at rest, the first-order slope `d omega^2 / d Omega^2`.
    pub fn slope(&self, phi: &[f64]) -> f64 {
        let s = if self.options.stress_stiffening { 1.0 } else { 0.0 };
        let c = if self.options.spin_softening { 1.0 } else { 0.0 };
        (s * quad(&self.kg1, phi) - c * quad(&self.m_perp, phi)) / quad(&self.m, phi)
    }

    /// The modes at the speed `omega`, rad/s (ADR 0403 D1 to D3). At `omega = 0` the result of
    /// [`crate::modal::modal_analysis`] with the modal options. A negative or non-finite speed is refused (`"speed"`).
    pub fn modes(&self, omega: f64) -> Result<SpeedModes> {
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(FemError::InvalidValue {
                key: "speed",
                reason: format!("{omega} rad/s is not a finite speed >= 0"),
            });
        }
        if omega == 0.0 {
            return modal_analysis(self.model, &self.options.modal).map(SpeedModes::Modes);
        }
        let opts = &self.options.modal;
        let dofs = self.model.dof_map();
        let keff = self.operator(omega);
        let n = self.m.n;
        let (kr, mr, free): (CsrMatrix, CsrMatrix, Option<Vec<usize>>) = match &self.path {
            Path::Elimination { fixed } => {
                let zero = vec![0.0; n];
                let kr = reduce(&keff, &zero, fixed)?;
                let mr = reduce(&self.m, &zero, fixed)?;
                let free = kr.free().to_vec();
                (kr.matrix, mr.matrix, Some(free))
            }
            Path::Transformation(tr) => {
                let zero = vec![0.0; tr.n_unknowns()];
                let (kr, _) = tr.reduce(&keff, &zero)?;
                let (mr, _) = tr.reduce(&self.m, &zero)?;
                (kr, mr, None)
            }
        };
        let nf = kr.n;
        if opts.modes == 0 || opts.modes >= nf {
            return Err(FemError::InvalidValue {
                key: "modes",
                reason: format!(
                    "{} modes requested of a model with {nf} free unknowns (at least 1 and fewer than the unknowns)",
                    opts.modes
                ),
            });
        }
        for (i, d) in mr.diag().iter().enumerate() {
            if d.is_nan() || *d <= 0.0 {
                let what = match (&self.path, &free) {
                    (Path::Elimination { .. }, Some(free)) => {
                        let (node, component) = dofs.node_component(free[i]).expect("free equation exists");
                        format!("node {node} component {component}")
                    }
                    (Path::Transformation(tr), _) => crate::mpc::unknown_name(tr, self.model, tr.independent()[i]),
                    _ => unreachable!("the elimination path has its free list"),
                };
                return Err(FemError::InvalidValue {
                    key: "density",
                    reason: format!("{what} has no mass (diagonal {d:e})"),
                });
            }
        }
        // D3: the inertia of the reduced K_eff.
        match count_below(&kr, &mr, 0.0, &SkylineOrdering::Rcm) {
            Ok(0) => {}
            Ok(negative) => {
                return Ok(SpeedModes::LossOfStiffness(LossOfStiffness {
                    speed: omega,
                    negative: Some(negative),
                }))
            }
            Err(EigenError::CountFailed { .. }) => {
                return Ok(SpeedModes::LossOfStiffness(LossOfStiffness {
                    speed: omega,
                    negative: None,
                }))
            }
            Err(e) => {
                return Err(FemError::Solver {
                    solver: "count_below",
                    reason: e.to_string(),
                })
            }
        }
        let sub = SubspaceOptions {
            shift: opts.shift.unwrap_or(Shift::Value(0.0)),
            tolerance: opts.tolerance,
            residual_test: opts.residual_test,
            max_iterations: opts.max_iterations,
            subspace_size: opts.subspace_size,
            factorisation: opts.factorisation,
            ..SubspaceOptions::default()
        };
        let eig = lowest_eigenpairs(&kr, &mr, opts.modes, &sub).map_err(|e| FemError::Solver {
            solver: "lowest_eigenpairs",
            reason: e.to_string(),
        })?;
        let modes = match (&self.path, &free) {
            (Path::Elimination { .. }, Some(free)) => eig
                .vectors
                .iter()
                .map(|v| {
                    let mut full = vec![0.0; n];
                    for (&eq, &x) in free.iter().zip(v) {
                        full[eq] = x;
                    }
                    full
                })
                .collect(),
            (Path::Transformation(tr), _) => eig.vectors.iter().map(|q| tr.expand(q)[..n].to_vec()).collect(),
            _ => unreachable!("the elimination path has its free list"),
        };
        Ok(SpeedModes::Modes(ModalResult::from_parts(eig, modes, nf, false, dofs)))
    }

    /// The modes over a list of speeds, rad/s (finite, `>= 0`, strictly increasing; refused otherwise, key `"speeds"`),
    /// tracked by the mass-weighted modal assurance (ADR 0403 D4).
    pub fn campbell(&self, speeds: &[f64]) -> Result<CampbellTable> {
        check_speeds(speeds)?;
        let mut results = Vec::with_capacity(speeds.len());
        for &w in speeds {
            results.push(self.modes(w)?);
        }
        Ok(CampbellTable::track(speeds, &results, &self.m))
    }
}

fn check_frame(axis: Vec3, origin: Vec3) -> Result<()> {
    unit_axis(axis)?;
    if origin.iter().any(|c| !c.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "origin",
            reason: format!("{origin:?} is not finite"),
        });
    }
    Ok(())
}

fn check_model(model: &SolidModel) -> Result<()> {
    model.validate()?;
    // ADR 0459 D1: solid elements, also of the orthotropic and the tabulated material; ADR 0462 D2: with a cyclic constraint
    // only with cylindrical axes about its axis and origin. ADR 0473 D6: also beams and shells.
    model.check_supported_with_materials()?;
    crate::mat_ortho::check_cyclic_axes(model)?;
    if crate::mpc::uses_transformation(model) {
        return crate::mpc::refuse_nonzero_values(model, "a rotating modal analysis");
    }
    let dofs = model.dof_map();
    let fixed = prescribed_equations(model, &[])?;
    if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
        let (node, component) = dofs.node_component(eq).expect("existing equation");
        return Err(FemError::InvalidValue {
            key: "constraint",
            reason: format!(
                "prescribed value {v:e} at node {node} component {component}: a modal analysis takes zero values only"
            ),
        });
    }
    // A free body is allowed: at a speed above zero it is reported as a loss of stiffness.
    match check_rigid_body_motions(model, &fixed) {
        Ok(()) | Err(FemError::RigidBodyMotionFree { .. }) => Ok(()),
        Err(e) => Err(e),
    }
}

fn check_speeds(speeds: &[f64]) -> Result<()> {
    if speeds.is_empty() {
        return Err(FemError::InvalidValue {
            key: "speeds",
            reason: "the list of speeds is empty".to_string(),
        });
    }
    for (i, &w) in speeds.iter().enumerate() {
        if !(w.is_finite() && w >= 0.0) {
            return Err(FemError::InvalidValue {
                key: "speeds",
                reason: format!("speed {i} ({w} rad/s) is not a finite speed >= 0"),
            });
        }
        if i > 0 && w <= speeds[i - 1] {
            return Err(FemError::InvalidValue {
                key: "speeds",
                reason: format!(
                    "speed {i} ({w} rad/s) does not exceed speed {} ({} rad/s): the list must increase",
                    i - 1,
                    speeds[i - 1]
                ),
            });
        }
    }
    Ok(())
}

fn quad(a: &CsrMatrix, x: &[f64]) -> f64 {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    x.iter().zip(&y).map(|(p, q)| p * q).sum()
}

fn bilinear(a: &CsrMatrix, x: &[f64], z: &[f64]) -> f64 {
    let mut y = vec![0.0; a.n];
    a.matvec(z, &mut y);
    x.iter().zip(&y).map(|(p, q)| p * q).sum()
}

/// The mass-weighted modal assurance `(phi^T M psi)^2 / ((phi^T M phi)(psi^T M psi))` (ADR 0403 D4).
pub fn mac(m: &CsrMatrix, phi: &[f64], psi: &[f64]) -> f64 {
    let c = bilinear(m, phi, psi);
    c * c / (quad(m, phi) * quad(m, psi))
}

/// The speeds, rad/s, at which a branch `lambda(Omega)` given at increasing speeds (`None` where it has no value) crosses the
/// engine-order line `omega = order Omega`: on each interval with values at both ends, `lambda` linear in `Omega^2`, the root
/// of `lambda - order^2 Omega^2` (ADR 0403 D4). Exact for `lambda = omega_0^2 + S Omega^2`.
pub fn engine_order_crossings(speeds: &[f64], eigenvalues: &[Option<f64>], order: u32) -> Vec<f64> {
    let k2 = f64::from(order) * f64::from(order);
    let g = |j: usize| eigenvalues[j].map(|l| (speeds[j] * speeds[j], l - k2 * speeds[j] * speeds[j]));
    let mut out = Vec::new();
    let n = speeds.len().min(eigenvalues.len());
    for j in 0..n {
        if let Some((_, gj)) = g(j) {
            if gj == 0.0 {
                out.push(speeds[j]);
            }
        }
        if j + 1 < n {
            if let (Some((sa, ga)), Some((sb, gb))) = (g(j), g(j + 1)) {
                if (ga < 0.0 && gb > 0.0) || (ga > 0.0 && gb < 0.0) {
                    let s = sa - ga * (sb - sa) / (gb - ga);
                    out.push(s.sqrt());
                }
            }
        }
    }
    out
}

/// One tracked mode over the speeds of a [`CampbellTable`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ModeTrack {
    /// `omega^2` at each speed (`None` at a loss of stiffness or before the track starts).
    pub eigenvalues: Vec<Option<f64>>,
    /// Frequency, Hz, at each speed.
    pub frequencies: Vec<Option<f64>>,
    /// The modal assurance with the track's previous mode (`None` at its first speed and where it has no mode).
    pub mac: Vec<Option<f64>>,
    /// The mode shape at each speed (the model's equations).
    pub modes: Vec<Option<Vec<f64>>>,
}

/// A crossing of a tracked mode with an engine order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    /// The track.
    pub track: usize,
    /// The engine order `k` of the line `f = k Omega/(2 pi)`.
    pub order: u32,
    /// The speed, rad/s.
    pub speed: f64,
    /// The frequency, Hz, `order speed/(2 pi)`.
    pub frequency: f64,
}

/// The table of a Campbell diagram: the tracked modes over increasing speeds (ADR 0403 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CampbellTable {
    /// The speeds, rad/s.
    pub speeds: Vec<f64>,
    /// The tracked modes.
    pub tracks: Vec<ModeTrack>,
    /// The speeds at which `K_eff` is not positive definite.
    pub losses: Vec<LossOfStiffness>,
}

impl CampbellTable {
    fn track(speeds: &[f64], results: &[SpeedModes], m: &CsrMatrix) -> CampbellTable {
        let ns = speeds.len();
        let mut tracks: Vec<ModeTrack> = Vec::new();
        let mut losses = Vec::new();
        for (j, r) in results.iter().enumerate() {
            match r {
                SpeedModes::LossOfStiffness(l) => losses.push(*l),
                SpeedModes::Modes(res) => {
                    if tracks.is_empty() {
                        for (i, phi) in res.modes.iter().enumerate() {
                            let mut t = ModeTrack {
                                eigenvalues: vec![None; ns],
                                frequencies: vec![None; ns],
                                mac: vec![None; ns],
                                modes: vec![None; ns],
                            };
                            t.eigenvalues[j] = Some(res.eigenvalues[i]);
                            t.frequencies[j] = Some(res.frequencies[i]);
                            t.modes[j] = Some(phi.clone());
                            tracks.push(t);
                        }
                        continue;
                    }
                    let last: Vec<&Vec<f64>> = tracks
                        .iter()
                        .map(|t| {
                            t.modes[..j]
                                .iter()
                                .rev()
                                .find_map(|m| m.as_ref())
                                .expect("a track has a mode at its start")
                        })
                        .collect();
                    let mut pairs = Vec::with_capacity(last.len() * res.modes.len());
                    for (t, phi) in last.iter().enumerate() {
                        for (i, psi) in res.modes.iter().enumerate() {
                            pairs.push((mac(m, phi, psi), t, i));
                        }
                    }
                    // Decreasing MAC; ties by the lower track, then the lower mode.
                    pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
                    let mut track_used = vec![false; tracks.len()];
                    let mut mode_used = vec![false; res.modes.len()];
                    for (value, t, i) in pairs {
                        if track_used[t] || mode_used[i] {
                            continue;
                        }
                        track_used[t] = true;
                        mode_used[i] = true;
                        let tr = &mut tracks[t];
                        tr.eigenvalues[j] = Some(res.eigenvalues[i]);
                        tr.frequencies[j] = Some(res.frequencies[i]);
                        tr.mac[j] = Some(value);
                        tr.modes[j] = Some(res.modes[i].clone());
                    }
                }
            }
        }
        CampbellTable {
            speeds: speeds.to_vec(),
            tracks,
            losses,
        }
    }

    /// The crossings of every track with the engine order `order` (by [`engine_order_crossings`]), track by track in
    /// increasing speed.
    pub fn crossings(&self, order: u32) -> Vec<Crossing> {
        let mut out = Vec::new();
        for (t, tr) in self.tracks.iter().enumerate() {
            for speed in engine_order_crossings(&self.speeds, &tr.eigenvalues, order) {
                out.push(Crossing {
                    track: t,
                    order,
                    speed,
                    frequency: f64::from(order) * speed / (2.0 * PI),
                });
            }
        }
        out
    }

    /// The table as CSV: `speed_rad_s,speed_rpm,track_0_hz,...,loss_negative`, one row per speed; a frequency is empty
    /// where the track has none, `loss_negative` is the negative count at a loss of stiffness (`singular` for a zero
    /// pivot) and empty otherwise. Numbers in Rust's shortest round-trip form.
    pub fn to_csv(&self) -> String {
        let mut s = String::from("speed_rad_s,speed_rpm");
        for t in 0..self.tracks.len() {
            let _ = write!(s, ",track_{t}_hz");
        }
        s.push_str(",loss_negative\n");
        for (j, &w) in self.speeds.iter().enumerate() {
            let _ = write!(s, "{w},{}", w * 60.0 / (2.0 * PI));
            for t in &self.tracks {
                match t.frequencies[j] {
                    Some(f) => {
                        let _ = write!(s, ",{f}");
                    }
                    None => s.push(','),
                }
            }
            match self.losses.iter().find(|l| l.speed.to_bits() == w.to_bits()) {
                Some(LossOfStiffness { negative: Some(n), .. }) => {
                    let _ = write!(s, ",{n}");
                }
                Some(LossOfStiffness { negative: None, .. }) => s.push_str(",singular"),
                None => s.push(','),
            }
            s.push('\n');
        }
        s
    }

    /// The table as JSON: the speeds, each track's frequencies (Hz) and modal assurance (`null` where it has none), the
    /// losses of stiffness and the crossings with the given engine orders.
    pub fn to_json(&self, orders: &[u32]) -> String {
        fn opt(v: Option<f64>) -> String {
            v.map_or_else(|| "null".to_string(), |x| format!("{x}"))
        }
        fn list(v: impl Iterator<Item = String>) -> String {
            format!("[{}]", v.collect::<Vec<_>>().join(", "))
        }
        let mut s = String::from("{\n");
        let _ = writeln!(
            s,
            "  \"speeds_rad_s\": {},",
            list(self.speeds.iter().map(|w| format!("{w}")))
        );
        s.push_str("  \"tracks\": [\n");
        for (t, tr) in self.tracks.iter().enumerate() {
            let _ = write!(
                s,
                "    {{\"track\": {t}, \"frequencies_hz\": {}, \"mac\": {}}}",
                list(tr.frequencies.iter().map(|&f| opt(f))),
                list(tr.mac.iter().map(|&m| opt(m)))
            );
            s.push_str(if t + 1 < self.tracks.len() { ",\n" } else { "\n" });
        }
        s.push_str("  ],\n");
        let _ = writeln!(
            s,
            "  \"losses_of_stiffness\": {},",
            list(self.losses.iter().map(|l| format!(
                "{{\"speed_rad_s\": {}, \"negative_eigenvalues\": {}}}",
                l.speed,
                l.negative.map_or_else(|| "null".to_string(), |n| n.to_string())
            )))
        );
        let crossings: Vec<Crossing> = orders.iter().flat_map(|&k| self.crossings(k)).collect();
        let _ = writeln!(
            s,
            "  \"crossings\": {}",
            list(crossings.iter().map(|c| format!(
                "{{\"track\": {}, \"order\": {}, \"speed_rad_s\": {}, \"frequency_hz\": {}}}",
                c.track, c.order, c.speed, c.frequency
            )))
        );
        s.push('}');
        s.push('\n');
        s
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// ADR 0407 (F-500): rotating modes of a cyclic sector at a harmonic index.

/// A loss of stiffness of one harmonic index of a sector (ADR 0407 D2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicLoss {
    /// The harmonic index `k`.
    pub harmonic: usize,
    /// 1 for `k = 0` and `2 k = N`, 2 otherwise.
    pub multiplicity: usize,
    /// The speed and the negative count of the real problem of the index (for a pair every eigenvalue twice, so even).
    pub loss: LossOfStiffness,
}

/// The result of one harmonic index of a sector at one speed (ADR 0407 D3).
#[derive(Debug, Clone, PartialEq)]
pub enum HarmonicSpeedModes {
    /// The modes of the index, as [`crate::cyclic::modal_analysis_harmonic`] packs them, with `reduced_stiffness` the reduced
    /// `K_eff`.
    Modes(Box<crate::cyclic::HarmonicModes>),
    /// The reduced `K_eff` of the index is not positive definite at this speed: no frequency is computed.
    LossOfStiffness(HarmonicLoss),
}

impl HarmonicSpeedModes {
    /// The modes, if the index has them at this speed.
    pub fn modes(&self) -> Option<&crate::cyclic::HarmonicModes> {
        match self {
            HarmonicSpeedModes::Modes(r) => Some(&**r),
            HarmonicSpeedModes::LossOfStiffness(_) => None,
        }
    }
    /// The loss of stiffness, if the index has one at this speed.
    pub fn loss(&self) -> Option<&HarmonicLoss> {
        match self {
            HarmonicSpeedModes::Modes(_) => None,
            HarmonicSpeedModes::LossOfStiffness(l) => Some(l),
        }
    }
}

/// A sector with one cyclic constraint rotating about the constraint's axis, prepared for the modes of any harmonic index at
/// any speed (ADR 0407): the [`RotatingPart`] of the sector (the pre-stress of the index-0 static solve at 1 rad/s and the
/// matrices `K`, `K_sigma,1`, `M`, `M_perp`), applied at index `k` on the real cos/sin pair of ADR 0386.
pub struct RotatingSector<'a> {
    part: RotatingPart<'a>,
    sectors: usize,
    axis: Vec3,
    origin: Vec3,
}

/// One family of a [`CyclicCampbell`]: the Campbell table of one harmonic index.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HarmonicCampbell {
    /// The harmonic index `k` (the nodal diameters).
    pub harmonic: usize,
    /// 1 for `k = 0` and `2 k = N`, 2 otherwise (each track is a pair of full modes).
    pub multiplicity: usize,
    /// The tracks of the family; a track's mode is the first vector `(a, b)` of its group, concatenated, its eigenvalue the
    /// pair's mean, its MAC the subspace assurance (ADR 0407 D3).
    pub table: CampbellTable,
}

/// A crossing of a family's track with an engine order (ADR 0407 D3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FamilyCrossing {
    /// The harmonic index of the family.
    pub harmonic: usize,
    /// The crossing (track of the family, order, speed, frequency).
    pub crossing: Crossing,
    /// Whether the engine order excites the index ([`engine_order_excites`]).
    pub excited: bool,
}

/// The Campbell diagram of a cyclic sector: one family of tracks per harmonic index (ADR 0407 D3).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CyclicCampbell {
    /// The number of sectors `N`.
    pub sectors: usize,
    /// The families, in increasing index.
    pub families: Vec<HarmonicCampbell>,
}

/// Whether engine order `order` excites harmonic index `harmonic` of a rotor of `sectors` sectors: `order mod N` is `k` or
/// `N - k` (ADR 0407 D3; the projection of the sector phases `e^{i E j alpha}` on index `k`).
pub fn engine_order_excites(order: u32, harmonic: usize, sectors: usize) -> bool {
    if sectors == 0 {
        return false;
    }
    let e = order as usize % sectors;
    let k = harmonic % sectors;
    e == k || (e + k) % sectors == 0
}

/// `x^T M_2 y` with `M_2 = blockdiag(m, m, ...)` over vectors of a multiple of `m.n` entries.
fn block_dot(m: &CsrMatrix, x: &[f64], y: &[f64]) -> f64 {
    let n = m.n;
    let mut out = 0.0;
    let mut t = vec![0.0; n];
    for (xb, yb) in x.chunks(n).zip(y.chunks(n)) {
        m.matvec(yb, &mut t);
        out += xb.iter().zip(&t).map(|(p, q)| p * q).sum::<f64>();
    }
    out
}

/// The inverse of a small symmetric positive definite matrix by Gauss-Jordan elimination with partial pivoting.
fn small_inverse(g: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let d = g.len();
    if d == 1 {
        return vec![vec![1.0 / g[0][0]]];
    }
    let mut a: Vec<Vec<f64>> = g.to_vec();
    let mut inv: Vec<Vec<f64>> = (0..d)
        .map(|i| (0..d).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    for c in 0..d {
        let p = (c..d)
            .max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs()))
            .unwrap_or(c);
        a.swap(c, p);
        inv.swap(c, p);
        let piv = a[c][c];
        for j in 0..d {
            a[c][j] /= piv;
            inv[c][j] /= piv;
        }
        for r in 0..d {
            if r != c {
                let f = a[r][c];
                for j in 0..d {
                    a[r][j] -= f * a[c][j];
                    inv[r][j] -= f * inv[c][j];
                }
            }
        }
    }
    inv
}

/// The subspace assurance `tr(G_V^-1 C G_W^-1 C^T) / min(d_V, d_W)` of two groups of vectors, `C = V^T M_2 W`, `G_V = V^T M_2
/// V`, `G_W = W^T M_2 W`, `M_2` the mass `m` block-diagonal over the vectors' length (ADR 0407 D3 and amendment 1): the share
/// of the smaller space inside the larger (1 when one contains the other), independent of the basis in either group; the
/// mass-weighted MAC for two single vectors. 0 for an empty group.
pub fn subspace_mac(m: &CsrMatrix, v: &[Vec<f64>], w: &[Vec<f64>]) -> f64 {
    let (dv, dw) = (v.len(), w.len());
    if dv == 0 || dw == 0 {
        return 0.0;
    }
    let gram = |a: &[Vec<f64>], b: &[Vec<f64>]| -> Vec<Vec<f64>> {
        a.iter()
            .map(|x| b.iter().map(|y| block_dot(m, x, y)).collect())
            .collect()
    };
    let c = gram(v, w);
    let gv = small_inverse(&gram(v, v));
    let gw = small_inverse(&gram(w, w));
    // tr((G_V^-1 C) (G_W^-1 C^T))
    let mut s = 0.0;
    for i in 0..dv {
        for p in 0..dw {
            let a: f64 = (0..dv).map(|j| gv[i][j] * c[j][p]).sum();
            let b: f64 = (0..dw).map(|q| gw[p][q] * c[i][q]).sum();
            s += a * b;
        }
    }
    s / dv.min(dw) as f64
}

fn check_harmonics(harmonics: &[usize], sectors: usize) -> Result<()> {
    if harmonics.is_empty() {
        return Err(FemError::InvalidValue {
            key: "harmonics",
            reason: "the list of harmonic indices is empty".to_string(),
        });
    }
    for (i, &k) in harmonics.iter().enumerate() {
        if 2 * k > sectors {
            return Err(FemError::InvalidValue {
                key: "harmonics",
                reason: format!("index {i} (harmonic {k}) is above N/2 for a rotor of {sectors} sectors"),
            });
        }
        if i > 0 && k <= harmonics[i - 1] {
            return Err(FemError::InvalidValue {
                key: "harmonics",
                reason: format!(
                    "index {i} (harmonic {k}) does not exceed index {} (harmonic {}): the list must increase",
                    i - 1,
                    harmonics[i - 1]
                ),
            });
        }
    }
    Ok(())
}

impl<'a> RotatingSector<'a> {
    /// Prepares a sector with one cyclic constraint, rotating about that constraint's axis through its origin: the
    /// [`RotatingPart::new`] of the sector (one static solve of the centrifugal load at 1 rad/s under the index-0 rows).
    /// Refuses a model without exactly one cyclic constraint and whatever [`RotatingPart::new`] refuses.
    pub fn new(model: &'a SolidModel, options: &RotatingOptions) -> Result<RotatingSector<'a>> {
        // ADR 0459 D3, ADR 0462 D2: a sector of either material with other axes is refused before its frame.
        crate::mat_ortho::check_cyclic_axes(model)?;
        let (sectors, axis, origin) = Self::frame(model)?;
        let part = RotatingPart::new(model, axis, origin, options)?;
        Ok(RotatingSector {
            part,
            sectors,
            axis,
            origin,
        })
    }

    /// As [`RotatingSector::new`] with a pre-stress at 1 rad/s the caller computed (of index 0; ADR 0407 D1).
    pub fn with_unit_prestress(
        model: &'a SolidModel,
        prestress: Prestress,
        options: &RotatingOptions,
    ) -> Result<RotatingSector<'a>> {
        crate::mat_ortho::check_cyclic_axes(model)?;
        let (sectors, axis, origin) = Self::frame(model)?;
        let part = RotatingPart::with_unit_prestress(model, axis, origin, prestress, options)?;
        Ok(RotatingSector {
            part,
            sectors,
            axis,
            origin,
        })
    }

    fn frame(model: &SolidModel) -> Result<(usize, Vec3, Vec3)> {
        model.validate()?;
        let ci = crate::cyclic::the_cyclic_constraint(model)?;
        let bd = crate::cyclic::boundary(model, ci)?;
        Ok((bd.sectors(), bd.axis(), bd.origin()))
    }

    /// The rotating part of the sector (index-0 operator, matrices, pre-stress).
    pub fn part(&self) -> &RotatingPart<'a> {
        &self.part
    }
    /// The number of sectors `N`.
    pub fn sectors(&self) -> usize {
        self.sectors
    }
    /// The unit axis of rotation (the cyclic axis).
    pub fn axis(&self) -> Vec3 {
        self.axis
    }
    /// The origin on the axis.
    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    /// The modes of harmonic index `harmonic` at the speed `omega`, rad/s (ADR 0407 D1 to D3). At `omega = 0` the result of
    /// [`crate::cyclic::modal_analysis_harmonic`] with the modal options. Refuses a harmonic above `N / 2` (`"harmonic"`) and a
    /// negative or non-finite speed (`"speed"`).
    pub fn modes(&self, harmonic: usize, omega: f64) -> Result<HarmonicSpeedModes> {
        use crate::cyclic::{doubled_matrix, unknown_label, HarmonicModes, SectorMode};
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(FemError::InvalidValue {
                key: "speed",
                reason: format!("{omega} rad/s is not a finite speed >= 0"),
            });
        }
        let sectors = self.sectors;
        if 2 * harmonic > sectors {
            return Err(FemError::InvalidValue {
                key: "harmonic",
                reason: format!(
                    "harmonic index {harmonic} is above N/2 for a rotor of {sectors} sectors (the largest is {})",
                    sectors / 2
                ),
            });
        }
        let model = self.part.model;
        let options = &self.part.options.modal;
        if omega == 0.0 {
            return crate::cyclic::modal_analysis_harmonic(model, &[], harmonic, options)
                .map(|h| HarmonicSpeedModes::Modes(Box::new(h)));
        }
        let n = model.dof_map().n_equations();
        let tr = if harmonic == 0 {
            crate::mpc::transformation(model, &[])?
        } else {
            crate::mpc::harmonic_transformation(model, &[], harmonic)?
        };
        let multiplicity = if harmonic == 0 || 2 * harmonic == sectors { 1 } else { 2 };
        let keff = self.part.operator(omega);
        let (k2, m2) = if multiplicity == 2 {
            (doubled_matrix(&keff), doubled_matrix(&self.part.m))
        } else {
            (keff, self.part.m.clone())
        };
        let zero = vec![0.0; tr.n_unknowns()];
        let (kr, _) = tr.reduce(&k2, &zero)?;
        let (mr, _) = tr.reduce(&m2, &zero)?;
        let nq = kr.n;
        let count = options.modes * multiplicity;
        if options.modes == 0 || count >= nq {
            return Err(FemError::InvalidValue {
                key: "modes",
                reason: format!(
                    "{} modes ({count} eigenvalues of the real problem) requested of harmonic {harmonic} with {nq} free \
                     unknowns (at least 1 and fewer than the unknowns)",
                    options.modes
                ),
            });
        }
        for (a, d) in mr.diag().iter().enumerate() {
            if d.is_nan() || *d <= 0.0 {
                let i = tr.independent()[a];
                return Err(FemError::InvalidValue {
                    key: "density",
                    reason: format!("{} has no mass (diagonal {d:e})", unknown_label(&tr, model, i)),
                });
            }
        }
        // D2: the inertia of the reduced K_eff of the index.
        let lost = |negative: Option<usize>| {
            HarmonicSpeedModes::LossOfStiffness(HarmonicLoss {
                harmonic,
                multiplicity,
                loss: LossOfStiffness { speed: omega, negative },
            })
        };
        match count_below(&kr, &mr, 0.0, &SkylineOrdering::Rcm) {
            Ok(0) => {}
            Ok(negative) => return Ok(lost(Some(negative))),
            Err(EigenError::CountFailed { .. }) => return Ok(lost(None)),
            Err(e) => {
                return Err(FemError::Solver {
                    solver: "count_below",
                    reason: e.to_string(),
                })
            }
        }
        let sub = SubspaceOptions {
            shift: options.shift.unwrap_or(Shift::Value(0.0)),
            tolerance: options.tolerance,
            residual_test: options.residual_test,
            max_iterations: options.max_iterations,
            subspace_size: options.subspace_size,
            factorisation: options.factorisation,
            ..SubspaceOptions::default()
        };
        let eig = lowest_eigenpairs(&kr, &mr, count, &sub).map_err(|e| FemError::Solver {
            solver: "lowest_eigenpairs",
            reason: e.to_string(),
        })?;
        let rigid_free = match options.shift {
            None => false,
            Some(_) => crate::mpc::zero_shift_refused(&kr),
        };
        let modes: Vec<SectorMode> = eig
            .vectors
            .iter()
            .map(|q| {
                let x = tr.expand(q);
                SectorMode {
                    cos: x[..n].to_vec(),
                    sin: if multiplicity == 2 {
                        x[n..2 * n].to_vec()
                    } else {
                        vec![0.0; n]
                    },
                }
            })
            .collect();
        let (values, pair_splits): (Vec<f64>, Vec<f64>) = if multiplicity == 2 {
            eig.values
                .chunks(2)
                .map(|p| {
                    let mean = 0.5 * (p[0] + p[1]);
                    let split = if mean == 0.0 {
                        (p[1] - p[0]).abs()
                    } else {
                        (p[1] - p[0]).abs() / mean.abs()
                    };
                    (mean, split)
                })
                .unzip()
        } else {
            (eig.values.clone(), vec![0.0; eig.values.len()])
        };
        Ok(HarmonicSpeedModes::Modes(Box::new(HarmonicModes {
            harmonic,
            sectors,
            multiplicity,
            frequencies: values.iter().map(|&l| crate::modal::frequency(l)).collect(),
            values,
            pair_splits,
            eigenvalues: eig.values,
            modes,
            residuals: eig.residuals,
            iterations: eig.iterations,
            shift: eig.shift,
            sturm: eig.sturm,
            unknowns: nq,
            rigid_free,
            transformation: tr,
            reduced_stiffness: kr,
            reduced_mass: mr,
        })))
    }

    /// The Campbell diagram over the harmonic indices `harmonics` (non-empty, strictly increasing, each at most `N / 2`; key
    /// `"harmonics"`) and the speeds `speeds` (as [`RotatingPart::campbell`]; key `"speeds"`): one family of tracks per index,
    /// tracked by the subspace assurance of each sector frequency's group (ADR 0407 D3).
    pub fn campbell(&self, harmonics: &[usize], speeds: &[f64]) -> Result<CyclicCampbell> {
        check_harmonics(harmonics, self.sectors)?;
        check_speeds(speeds)?;
        let mut families = Vec::with_capacity(harmonics.len());
        for &k in harmonics {
            let mut results = Vec::with_capacity(speeds.len());
            for &w in speeds {
                results.push(self.modes(k, w)?);
            }
            let multiplicity = if k == 0 || 2 * k == self.sectors { 1 } else { 2 };
            families.push(HarmonicCampbell {
                harmonic: k,
                multiplicity,
                table: track_families(speeds, &results, &self.part.m, multiplicity),
            });
        }
        Ok(CyclicCampbell {
            sectors: self.sectors,
            families,
        })
    }
}

/// The groups of a harmonic result: per sector frequency its vectors `(a, b)` concatenated (only `a` for a single harmonic).
fn groups_of(res: &crate::cyclic::HarmonicModes, multiplicity: usize) -> Vec<Vec<Vec<f64>>> {
    let vector = |m: &crate::cyclic::SectorMode| -> Vec<f64> {
        if multiplicity == 2 {
            m.cos.iter().chain(&m.sin).copied().collect()
        } else {
            m.cos.clone()
        }
    };
    res.modes
        .chunks(multiplicity)
        .map(|g| g.iter().map(vector).collect())
        .collect()
}

/// Relative distance of two sector frequencies' eigenvalues within which they are one cluster (ADR 0407 amendment 1).
pub const CLUSTER_TOLERANCE: f64 = 1e-8;

/// The subspace of each sector frequency: the vectors of every frequency of its cluster (consecutive values within
/// [`CLUSTER_TOLERANCE`] relative), its own first (ADR 0407 amendment 1).
fn cluster_spaces(values: &[f64], groups: &[Vec<Vec<f64>>]) -> Vec<Vec<Vec<f64>>> {
    let n = values.len().min(groups.len());
    let mut start = vec![0usize; n];
    for i in 1..n {
        let close = (values[i] - values[i - 1]).abs() <= CLUSTER_TOLERANCE * values[i - 1].abs().max(values[i].abs());
        start[i] = if close { start[i - 1] } else { i };
    }
    (0..n)
        .map(|i| {
            let mut end = i + 1;
            while end < n && start[end] == start[i] {
                end += 1;
            }
            let mut space = groups[i].clone();
            for (j, g) in groups.iter().enumerate().take(end).skip(start[i]) {
                if j != i {
                    space.extend(g.iter().cloned());
                }
            }
            space
        })
        .collect()
}

/// ADR 0403 D4's greedy tracking with groups in place of vectors and the subspace assurance in place of the MAC.
fn track_families(speeds: &[f64], results: &[HarmonicSpeedModes], m: &CsrMatrix, multiplicity: usize) -> CampbellTable {
    let ns = speeds.len();
    let mut tracks: Vec<ModeTrack> = Vec::new();
    let mut last: Vec<Vec<Vec<f64>>> = Vec::new();
    let mut losses = Vec::new();
    for (j, r) in results.iter().enumerate() {
        match r {
            HarmonicSpeedModes::LossOfStiffness(l) => losses.push(l.loss),
            HarmonicSpeedModes::Modes(res) => {
                let groups = cluster_spaces(&res.values, &groups_of(res, multiplicity));
                if tracks.is_empty() {
                    for (i, g) in groups.iter().enumerate() {
                        let mut t = ModeTrack {
                            eigenvalues: vec![None; ns],
                            frequencies: vec![None; ns],
                            mac: vec![None; ns],
                            modes: vec![None; ns],
                        };
                        t.eigenvalues[j] = Some(res.values[i]);
                        t.frequencies[j] = Some(res.frequencies[i]);
                        t.modes[j] = Some(g[0].clone());
                        tracks.push(t);
                        last.push(g.clone());
                    }
                    continue;
                }
                let mut pairs = Vec::with_capacity(last.len() * groups.len());
                for (t, v) in last.iter().enumerate() {
                    for (i, w) in groups.iter().enumerate() {
                        pairs.push((subspace_mac(m, v, w), t, i));
                    }
                }
                // Decreasing assurance; ties by the lower track, then the lower group.
                pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
                let mut track_used = vec![false; tracks.len()];
                let mut group_used = vec![false; groups.len()];
                for (value, t, i) in pairs {
                    if track_used[t] || group_used[i] {
                        continue;
                    }
                    track_used[t] = true;
                    group_used[i] = true;
                    let tr = &mut tracks[t];
                    tr.eigenvalues[j] = Some(res.values[i]);
                    tr.frequencies[j] = Some(res.frequencies[i]);
                    tr.mac[j] = Some(value);
                    tr.modes[j] = Some(groups[i][0].clone());
                    last[t] = groups[i].clone();
                }
            }
        }
    }
    CampbellTable {
        speeds: speeds.to_vec(),
        tracks,
        losses,
    }
}

impl CyclicCampbell {
    /// The crossings of every family's tracks with the engine order `order`, family by family, each flagged by whether the
    /// order excites the family.
    pub fn crossings(&self, order: u32) -> Vec<FamilyCrossing> {
        let mut out = Vec::new();
        for f in &self.families {
            let excited = engine_order_excites(order, f.harmonic, self.sectors);
            for c in f.table.crossings(order) {
                out.push(FamilyCrossing {
                    harmonic: f.harmonic,
                    crossing: c,
                    excited,
                });
            }
        }
        out
    }

    /// The table as CSV: `harmonic,speed_rad_s,speed_rpm,track_0_hz,...,loss_negative`, one row per family and speed (the
    /// rows of [`CampbellTable::to_csv`] with the index in front, padded to the largest number of tracks).
    pub fn to_csv(&self) -> String {
        let width = self.families.iter().map(|f| f.table.tracks.len()).max().unwrap_or(0);
        let mut s = String::from("harmonic,speed_rad_s,speed_rpm");
        for t in 0..width {
            let _ = write!(s, ",track_{t}_hz");
        }
        s.push_str(",loss_negative\n");
        for f in &self.families {
            let pad = width - f.table.tracks.len();
            for line in f.table.to_csv().lines().skip(1) {
                // The last field is the loss; the padding goes before it.
                let (head, loss) = line.rsplit_once(',').unwrap_or((line, ""));
                let _ = write!(s, "{},{head}", f.harmonic);
                for _ in 0..pad {
                    s.push(',');
                }
                let _ = writeln!(s, ",{loss}");
            }
        }
        s
    }

    /// The table as JSON: the number of sectors, each family with its index, multiplicity and table
    /// ([`CampbellTable::to_json`] with the same orders), and the crossings with the orders, each flagged by
    /// [`engine_order_excites`].
    pub fn to_json(&self, orders: &[u32]) -> String {
        let mut s = String::from("{\n");
        let _ = writeln!(s, "  \"sectors\": {},", self.sectors);
        s.push_str("  \"families\": [\n");
        for (i, f) in self.families.iter().enumerate() {
            let _ = write!(
                s,
                "    {{\"harmonic\": {}, \"multiplicity\": {}, \"table\": {}}}",
                f.harmonic,
                f.multiplicity,
                f.table.to_json(orders).trim_end()
            );
            s.push_str(if i + 1 < self.families.len() { ",\n" } else { "\n" });
        }
        s.push_str("  ],\n");
        let crossings: Vec<FamilyCrossing> = orders.iter().flat_map(|&k| self.crossings(k)).collect();
        let items: Vec<String> = crossings
            .iter()
            .map(|c| {
                format!(
                    "{{\"harmonic\": {}, \"track\": {}, \"order\": {}, \"speed_rad_s\": {}, \"frequency_hz\": {}, \
                     \"excited\": {}}}",
                    c.harmonic, c.crossing.track, c.crossing.order, c.crossing.speed, c.crossing.frequency, c.excited
                )
            })
            .collect();
        let _ = writeln!(s, "  \"crossings\": [{}]", items.join(", "));
        s.push('}');
        s.push('\n');
        s
    }
}
