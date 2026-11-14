//! Modal analysis. Declared by ADR 0271; filled by ADR 0284.
//!
//! The lowest natural frequencies and mass-normalised mode shapes of `K phi = lambda M phi`, `lambda = omega^2`, of a
//! model at rest (ADR 0284 D2 to D4): the stiffness ([`crate::assemble::stiffness_with`]) and the consistent mass
//! ([`crate::mass::consistent_with`]) are reduced by the symmetric elimination of the model's zero prescribed
//! displacements ([`crate::constraint::reduce`]), and the lowest pairs come from the subspace iteration of
//! [`rcfd_linalg::eigen::lowest_eigenpairs`] (ADR 0283), verified by its Sturm count. The shift is `sigma = 0` when the
//! constraints hold every rigid motion, and `Shift::Auto` otherwise (a free or partly free body). Loads are ignored.
//!
//! A model with a linear constraint, a rigid set or an elastic support is reduced instead by the transformation `x = T q` of
//! [`crate::mpc`] (`T^T K T`, `T^T M T`, the modes expanded by `T`; zero values only): [`crate::mpc::modal_analysis_mpc`]
//! (ADR 0384).

use crate::assemble::stiffness_with;
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce, Prescribed};
use crate::error::{FemError, Result};
use crate::mass::consistent_with;
use crate::model::{DofMap, SolidModel};
use rcfd_linalg::eigen::{lowest_eigenpairs, ResidualTest, Shift, SturmCheck, SubspaceOptions};

/// Options of a modal analysis.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ModalOptions {
    /// Number of modes wanted (the lowest).
    pub modes: usize,
    /// Shift of the eigensolver; `None`: `Shift::Value(0.0)` if the constraints hold every rigid motion, `Shift::Auto`
    /// otherwise (ADR 0284 D4).
    pub shift: Option<Shift>,
    /// Residual tolerance of each pair (ADR 0283 D6).
    pub tolerance: f64,
    /// Residual measure of the tolerance (default [`ResidualTest::Relative`]; ADR 0354).
    pub residual_test: ResidualTest,
    /// Iteration cap of the subspace iteration.
    pub max_iterations: usize,
    /// Subspace size (`None`: the eigensolver's default `max(2 m, m + 8)`).
    pub subspace_size: Option<usize>,
    /// Worker threads of the assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
}

impl ModalOptions {
    /// The lowest `modes` modes with the default tolerance (1e-10), cap (200) and shift rule.
    pub fn new(modes: usize) -> ModalOptions {
        let d = SubspaceOptions::default();
        ModalOptions {
            modes,
            shift: None,
            tolerance: d.tolerance,
            residual_test: d.residual_test,
            max_iterations: d.max_iterations,
            subspace_size: None,
            threads: None,
        }
    }
    /// The same options with a given shift.
    pub fn shift(mut self, shift: Shift) -> ModalOptions {
        self.shift = Some(shift);
        self
    }
    /// The same options with a residual tolerance. A slender model needs more than the default 1e-10: the residual of the
    /// lowest pair cannot fall below about `c eps lambda_max / lambda_1` (ADR 0283 D6, ADR 0284 amendment 1).
    pub fn tolerance(mut self, tolerance: f64) -> ModalOptions {
        self.tolerance = tolerance;
        self
    }
    /// The same options with a residual measure: [`ResidualTest::Backward`] measures each pair against `(||A|| + |mu|
    /// ||M||) ||x||`, so that a slender model converges at a tolerance near 1e-12 (ADR 0354).
    pub fn residual_test(mut self, test: ResidualTest) -> ModalOptions {
        self.residual_test = test;
        self
    }
    /// The same options with a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> ModalOptions {
        self.threads = Some(threads);
        self
    }
}

/// The result of a modal analysis.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ModalResult {
    /// Eigenvalues `lambda = omega^2`, ascending, rad^2/s^2.
    pub eigenvalues: Vec<f64>,
    /// Frequencies `frequency(lambda)`, Hz.
    pub frequencies: Vec<f64>,
    /// Mode shapes, M-orthonormal, one value per equation of the model's `DofMap` (0 at prescribed equations).
    pub modes: Vec<Vec<f64>>,
    /// Relative residuals of the pairs (ADR 0283 D6).
    pub residuals: Vec<f64>,
    /// Iterations of the subspace iteration.
    pub iterations: usize,
    /// The shift used.
    pub shift: f64,
    /// The Sturm check of the eigensolver.
    pub sturm: SturmCheck,
    /// Number of free unknowns.
    pub unknowns: usize,
    /// Whether the constraints leave a rigid motion free (and the automatic shift was used). On the transformation path
    /// ([`crate::mpc::modal_analysis_mpc`]): whether the factor of `T^T K T` at zero shift was not accepted by the rule of
    /// `Shift::Auto` (a free rigid motion or another zero-energy motion; ADR 0384).
    pub rigid_free: bool,
    dofs: DofMap,
}

impl ModalResult {
    /// A result from the parts of an eigensolution (the modes already expanded to the model's equations).
    pub(crate) fn from_parts(
        eig: rcfd_linalg::eigen::GeneralizedEigen,
        modes: Vec<Vec<f64>>,
        unknowns: usize,
        rigid_free: bool,
        dofs: DofMap,
    ) -> ModalResult {
        ModalResult {
            frequencies: eig.values.iter().map(|&l| frequency(l)).collect(),
            eigenvalues: eig.values,
            modes,
            residuals: eig.residuals,
            iterations: eig.iterations,
            shift: eig.shift,
            sturm: eig.sturm,
            unknowns,
            rigid_free,
            dofs,
        }
    }

    /// The unknown numbering of the modes.
    pub fn dofs(&self) -> &DofMap {
        &self.dofs
    }
}

/// The frequency of an eigenvalue, `sqrt(lambda) / (2 pi)`; a negative (rounding) eigenvalue gives `-sqrt(-lambda) / (2
/// pi)`.
pub fn frequency(lambda: f64) -> f64 {
    lambda.signum() * lambda.abs().sqrt() / (2.0 * std::f64::consts::PI)
}

/// The modal analysis of a model with its own (zero) displacement constraints.
pub fn modal_analysis(model: &SolidModel, options: &ModalOptions) -> Result<ModalResult> {
    modal_analysis_with(model, &[], options)
}

/// The modal analysis with, in addition to the model's constraints, per-node prescribed values (all zero).
pub fn modal_analysis_with(model: &SolidModel, extra: &[Prescribed], options: &ModalOptions) -> Result<ModalResult> {
    model.validate()?;
    // ADR 0417 D4, ADR 0432 D3, ADR 0435 D1: the elimination path and the transformation path take beams and shells (the
    // transformation has its own check).
    model.check_supported_with_structures()?;
    // Linear constraints, rigid sets and supports: the reduction by the transformation of ADR 0296 (ADR 0384).
    if crate::mpc::uses_transformation(model) {
        return crate::mpc::modal_analysis_mpc(model, extra, options).map(|s| s.result);
    }
    let dofs = model.dof_map();
    let fixed = prescribed_equations(model, extra)?;
    if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
        let (node, component) = dofs.node_component(eq).expect("existing equation");
        return Err(FemError::InvalidValue {
            key: "constraint",
            reason: format!(
                "prescribed value {v:e} at node {node} component {component}: a modal analysis takes zero values only"
            ),
        });
    }
    let rigid_free = match check_rigid_body_motions(model, &fixed) {
        Ok(()) => false,
        Err(FemError::RigidBodyMotionFree { .. }) => true,
        Err(e) => return Err(e),
    };
    let k = stiffness_with(model, options.threads)?;
    let m = consistent_with(model, options.threads)?;
    let zero = vec![0.0; k.n];
    let kr = reduce(&k, &zero, &fixed)?;
    let mr = reduce(&m, &zero, &fixed)?;
    let free = kr.free().to_vec();
    let nf = free.len();
    if options.modes == 0 || options.modes >= nf {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{} modes requested of a model with {nf} free unknowns (at least 1 and fewer than the unknowns)",
                options.modes
            ),
        });
    }
    for (i, d) in mr.matrix.diag().iter().enumerate() {
        if d.is_nan() || *d <= 0.0 {
            let (node, component) = dofs.node_component(free[i]).expect("free equation exists");
            return Err(FemError::InvalidValue {
                key: "density",
                reason: format!("node {node} component {component} has no mass (diagonal {d:e})"),
            });
        }
    }
    let shift = options
        .shift
        .unwrap_or(if rigid_free { Shift::Auto } else { Shift::Value(0.0) });
    let opts = SubspaceOptions {
        shift,
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        ..SubspaceOptions::default()
    };
    let eig = lowest_eigenpairs(&kr.matrix, &mr.matrix, options.modes, &opts).map_err(|e| FemError::Solver {
        solver: "lowest_eigenpairs",
        reason: e.to_string(),
    })?;
    let modes = eig
        .vectors
        .iter()
        .map(|v| {
            let mut full = vec![0.0; k.n];
            for (&eq, &x) in free.iter().zip(v) {
                full[eq] = x;
            }
            full
        })
        .collect();
    Ok(ModalResult {
        frequencies: eig.values.iter().map(|&l| frequency(l)).collect(),
        eigenvalues: eig.values,
        modes,
        residuals: eig.residuals,
        iterations: eig.iterations,
        shift: eig.shift,
        sturm: eig.sturm,
        unknowns: nf,
        rigid_free,
        dofs,
    })
}
