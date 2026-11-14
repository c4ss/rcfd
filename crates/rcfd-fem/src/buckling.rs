//! Linear buckling factors from the geometric stiffness. Declared by ADR 0291; filled by ADR 0390 (the registration of
//! ADR 0298, F-395).
//!
//! The lowest positive factors `lambda` of `(K + lambda K_sigma) x = 0` (ADR 0390 D6), `K_sigma` the geometric stiffness of
//! the pre-stress of a reference load ([`crate::geometric::Prestress`]): the load `lambda` times the reference one buckles
//! the model. Both matrices are reduced by the elimination of the model's zero prescribed displacements, which must hold
//! every rigid motion and every mechanism (`K` positive definite; a reduced `K_FF` with a zero-shift pivot ratio below
//! `1e-8` is refused, ADR 0402), and the factors come from
//! [`rcfd_linalg::eigen::lowest_buckling_factors`], verified by its Sturm count of `K + s K_sigma`. A pre-stress with no
//! direction of compression gives no factor, and the result says below which value there is none
//! ([`BucklingResult::exhausted`]).
//!
//! ADR 0459 (F-542): the orthotropic and the tabulated material of ADR 0452 on solid elements, both paths, through the
//! per-point stiffness (a table at the model's temperature field); not with a cyclic constraint.

use crate::assemble::point_stiffness;
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reduce};
use crate::error::{FemError, Result};
use crate::geometric::{assemble_geometric, Prestress};
use crate::model::{DofMap, SolidModel};
use rcfd_linalg::eigen::{lowest_buckling_factors, BucklingEigenOptions, Factorisation, ResidualTest, SturmCheck};

/// Options of [`factors`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BucklingOptions {
    /// Number of factors wanted (the lowest positive ones).
    pub factors: usize,
    /// Residual tolerance of each pair.
    pub tolerance: f64,
    /// Residual measure (default [`ResidualTest::Backward`], whose floor does not grow with the slenderness; ADR 0354).
    pub residual_test: ResidualTest,
    /// Iteration cap of the subspace iteration.
    pub max_iterations: usize,
    /// Subspace size (`None`: `max(2 m, m + 8)`).
    pub subspace_size: Option<usize>,
    /// Worker threads of the assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
    /// The factorisation of the eigensolver (default [`Factorisation::Envelope`]; ADR 0447).
    pub factorisation: Factorisation,
}

impl BucklingOptions {
    /// The lowest `factors` factors with the backward-error test at 1e-10 and a cap of 200 iterations (ADR 0390 D6).
    pub fn new(factors: usize) -> BucklingOptions {
        BucklingOptions {
            factors,
            tolerance: 1e-10,
            residual_test: ResidualTest::Backward,
            max_iterations: 200,
            subspace_size: None,
            threads: None,
            factorisation: Factorisation::Envelope,
        }
    }
    /// The same options with a residual tolerance.
    pub fn tolerance(mut self, tolerance: f64) -> BucklingOptions {
        self.tolerance = tolerance;
        self
    }
    /// The same options with a residual measure.
    pub fn residual_test(mut self, test: ResidualTest) -> BucklingOptions {
        self.residual_test = test;
        self
    }
    /// The same options with a subspace size.
    pub fn subspace_size(mut self, q: usize) -> BucklingOptions {
        self.subspace_size = Some(q);
        self
    }
    /// The same options with a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> BucklingOptions {
        self.threads = Some(threads);
        self
    }
    /// The same options with a factorisation of the eigensolver (ADR 0447).
    pub fn factorisation(mut self, factorisation: Factorisation) -> BucklingOptions {
        self.factorisation = factorisation;
        self
    }
}

/// The result of a linear buckling analysis.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BucklingResult {
    /// Load factors, positive, ascending: the reference load times `factors[j]` buckles in mode `j`.
    pub factors: Vec<f64>,
    /// Buckling modes, `K`-normalised, one value per equation of the model's `DofMap` (0 at prescribed equations).
    pub modes: Vec<Vec<f64>>,
    /// Residuals of the pairs in the measure of the options.
    pub residuals: Vec<f64>,
    /// Iterations of the subspace iteration.
    pub iterations: usize,
    /// The Sturm check (`counted`: the factors in `(0, sturm.shift)`).
    pub sturm: SturmCheck,
    /// Number of free unknowns.
    pub unknowns: usize,
    /// `Some(S)` when fewer factors than asked exist below `S` (all of them are returned; none if `factors` is empty).
    pub exhausted: Option<f64>,
    dofs: DofMap,
}

impl BucklingResult {
    /// The unknown numbering of the modes.
    pub fn dofs(&self) -> &DofMap {
        &self.dofs
    }
}

/// The lowest positive buckling factors of `model` for the pre-stress of its reference load (ADR 0390 D6).
///
/// A model with linear constraints, rigid sets or elastic supports is solved on the transformation `x = T q` (ADR 0398):
/// `(T^T K_ext T + lambda T^T K_sigma T) q = 0`, `K_ext` with the supports, zero constraint values only, each mode expanded,
/// `x = T q`; a motion the constraints and supports leave free, rigid or not, is refused by the zero-shift pivot-ratio rule
/// of ADR 0384 on `T^T K_ext T` (a pivot `d_i < 1e-8 a_ii`; ADR 0401), `FemError::Solver` naming the unknown of the failing
/// pivot. A spring so soft that its pivot ratio is below the rule is refused too. On the prescribed-displacement path the
/// same rule is applied to `K_FF` after the rigid-motion test (ADR 0402): a mechanism that is not rigid, such as two parts
/// joined by a hinge line, is refused with `FemError::Solver` naming the unknown of the failing pivot.
pub fn factors(model: &SolidModel, prestress: &Prestress, options: &BucklingOptions) -> Result<BucklingResult> {
    factors_with(model, prestress, &[], options)
}

/// [`factors`] with `extra` triplets added to `K_sigma` after its assembly (ADR 0480 D3: the negative symmetric load
/// stiffness of a follower pressure); with none it executes the operations of [`factors`]. A non-empty `extra` is refused on
/// the transformation path (key `"load stiffness"`).
pub(crate) fn factors_with(
    model: &SolidModel,
    prestress: &Prestress,
    extra: &[(usize, usize, f64)],
    options: &BucklingOptions,
) -> Result<BucklingResult> {
    model.validate()?;
    // ADR 0459 D1, D3: solid elements, also of the orthotropic and the tabulated material (the per-point stiffness); not
    // with a cyclic constraint. ADR 0473 D6: also beams and shells.
    model.check_supported_with_materials()?;
    crate::mat_ortho::refuse_with_cyclic(model)?;
    if crate::mpc::uses_transformation(model) {
        if !extra.is_empty() {
            return Err(FemError::InvalidValue {
                key: "load stiffness",
                reason: "a load stiffness is not offered on the transformation path (linear constraints, rigid sets, \
                         cyclic constraints, elastic supports; ADR 0480 D3)"
                    .to_string(),
            });
        }
        return factors_on_transformation(model, prestress, options);
    }
    let dofs = model.dof_map();
    let fixed = prescribed_equations(model, &[])?;
    if let Some(&(eq, v)) = fixed.iter().find(|&&(_, v)| v != 0.0) {
        let (node, component) = dofs.node_component(eq).expect("existing equation");
        return Err(FemError::InvalidValue {
            key: "constraint",
            reason: format!(
                "prescribed value {v:e} at node {node} component {component}: a buckling analysis takes zero values only"
            ),
        });
    }
    check_rigid_body_motions(model, &fixed)?;
    let mut kg = assemble_geometric(model, prestress, options.threads)?;
    if !extra.is_empty() {
        crate::nonlinear::add_triplets(&mut kg, extra);
    }
    let k = point_stiffness(model, options.threads)?;
    let zero = vec![0.0; k.n];
    let kr = reduce(&k, &zero, &fixed)?;
    let gr = reduce(&kg, &zero, &fixed)?;
    let free = kr.free().to_vec();
    let nf = free.len();
    if options.factors == 0 || options.factors >= nf {
        return Err(FemError::InvalidValue {
            key: "factors",
            reason: format!(
                "{} factors requested of a model with {nf} free unknowns (at least 1 and fewer than the unknowns)",
                options.factors
            ),
        });
    }
    if let Ok(Some((row, ratio))) = crate::mpc::zero_shift_failing_pivot(&kr.matrix) {
        let (node, component) = dofs.node_component(free[row]).expect("free equation");
        return Err(FemError::Solver {
            solver: "lowest_buckling_factors",
            reason: format!(
                "node {node} component {component}: pivot ratio {ratio:e} of K_FF at zero shift, below 1e-8 (the rule of \
                 Shift::Auto, ADR 0384): the prescribed displacements leave a motion free that is not a rigid motion of a \
                 part (a mechanism); a buckling analysis needs every motion held (ADR 0402)"
            ),
        });
    }
    let opts = BucklingEigenOptions {
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..BucklingEigenOptions::default()
    };
    let eig =
        lowest_buckling_factors(&kr.matrix, &gr.matrix, options.factors, &opts).map_err(|e| FemError::Solver {
            solver: "lowest_buckling_factors",
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
    Ok(BucklingResult {
        factors: eig.values,
        modes,
        residuals: eig.residuals,
        iterations: eig.iterations,
        sturm: eig.sturm,
        unknowns: nf,
        exhausted: eig.exhausted,
        dofs,
    })
}

fn eigen_options(options: &BucklingOptions) -> BucklingEigenOptions {
    BucklingEigenOptions {
        tolerance: options.tolerance,
        residual_test: options.residual_test,
        max_iterations: options.max_iterations,
        subspace_size: options.subspace_size,
        factorisation: options.factorisation,
        ..BucklingEigenOptions::default()
    }
}

/// [`factors`] on the transformation (ADR 0398 D2).
fn factors_on_transformation(
    model: &SolidModel,
    prestress: &Prestress,
    options: &BucklingOptions,
) -> Result<BucklingResult> {
    crate::mpc::refuse_nonzero_values(model, "a buckling analysis")?;
    let dofs = model.dof_map();
    let tr = crate::mpc::transformation(model, &[])?;
    let n = tr.n_mesh();
    let mut k = point_stiffness(model, options.threads)?;
    crate::mpc::add_supports(&mut k, model)?;
    let kg = assemble_geometric(model, prestress, options.threads)?;
    let zero = vec![0.0; tr.n_unknowns()];
    let (kr, _) = tr.reduce(&k, &zero)?;
    let (gr, _) = tr.reduce(&kg, &zero)?;
    let nq = kr.n;
    if options.factors == 0 || options.factors >= nq {
        return Err(FemError::InvalidValue {
            key: "factors",
            reason: format!(
                "{} factors requested of a model with {nq} free unknowns (at least 1 and fewer than the unknowns)",
                options.factors
            ),
        });
    }
    if let Some((row, ratio)) = crate::mpc::zero_shift_failing_pivot(&kr)? {
        let name = crate::mpc::unknown_name(&tr, model, tr.independent()[row]);
        return Err(FemError::Solver {
            solver: "lowest_buckling_factors",
            reason: format!(
                "{name}: pivot ratio {ratio:e} of T^T K_ext T at zero shift, below 1e-8 (the rule of Shift::Auto, ADR \
                 0384): the constraints and supports leave a motion free (a mechanism); a buckling analysis needs every \
                 motion held (ADR 0401)"
            ),
        });
    }
    let eig =
        lowest_buckling_factors(&kr, &gr, options.factors, &eigen_options(options)).map_err(|e| FemError::Solver {
            solver: "lowest_buckling_factors",
            reason: e.to_string(),
        })?;
    let modes = eig.vectors.iter().map(|q| tr.expand(q)[..n].to_vec()).collect();
    Ok(BucklingResult {
        factors: eig.values,
        modes,
        residuals: eig.residuals,
        iterations: eig.iterations,
        sturm: eig.sturm,
        unknowns: nq,
        exhausted: eig.exhausted,
        dofs,
    })
}
