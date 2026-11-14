//! The linear static solve (solver choice: ADR 0282). Declared by ADR 0271; filled by ADR 0275.
//!
//! Steps (ADR 0275 D6): the load vector ([`crate::loads::nodal_load_vector`]), the prescribed equations and the check of
//! free rigid-body motions ([`crate::constraint`]), the stiffness ([`crate::assemble::stiffness_with`]), the symmetric
//! elimination ([`crate::constraint::reduce`]), the solve of `K_FF u_F = f_F - K_FP g` by the envelope `L D L^T` of ADR 0274
//! (default) or by CG with Jacobi, and the reactions `K u - f`. A pivot `d_i <= 1e-10 a_ii` of the direct solve is refused
//! as a singular stiffness (a mechanism the constraint check cannot see; ADR 0275 arithmetic 4).

use crate::constraint::{check_rigid_body_motions, prescribed_equations, reactions, reduce, Prescribed};
use crate::error::{FemError, Result};
use crate::loads::{nodal_load_vector, NodalForceAt};
use crate::model::DofMap;
use crate::model::{SolidModel, SolidResult, SolverStats};
use rcfd_linalg::precond::{BlockJacobi, IcBreakdown, IcOptions, IncompleteCholesky, PrecondError};
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
// ADR 0447 (F-431): the sparse L D L^T after approximate minimum degree.
use rcfd_linalg::sparse_chol::SparseLdlt;
use rcfd_linalg::vecops::norm2;
use rcfd_linalg::{cg, CsrMatrix, Jacobi, Preconditioner, SolveControl};
// ADR 0357 (F-460): fill levels and the node-wise ordering of the incomplete Cholesky.
use rcfd_linalg::precond::Permuted;
use rcfd_linalg::reorder::block_rcm_order;
// ADR 0305 (F-402): smoothed-aggregation multigrid with the rigid-body modes.
use rcfd_linalg::amg_elastic::Aggregation;
use rcfd_linalg::amg_elastic::{rigid_body_modes, ElasticAmg, ElasticAmgSettings};

/// Relative pivot threshold of the direct solve: a pivot `d_i <= PIVOT_TOLERANCE a_ii` is refused (ADR 0275 arithmetic 4).
pub const PIVOT_TOLERANCE: f64 = 1e-10;

/// The linear solver of the static solve.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub enum StaticSolver {
    /// Envelope `L D L^T` after reverse Cuthill-McKee (ADR 0274), the default.
    #[default]
    Direct,
    /// Conjugate gradients with the Jacobi preconditioner, to a relative residual.
    ConjugateGradient {
        /// Relative residual tolerance.
        tolerance: f64,
        /// Iteration cap.
        max_iterations: usize,
    },
    /// Conjugate gradients with block Jacobi or incomplete Cholesky (ADR 0282), to a true relative residual
    /// `|b - K u| <= tolerance |b|`.
    PreconditionedCg {
        /// The preconditioner.
        preconditioner: CgPreconditioner,
        /// Relative residual tolerance (checked on the true residual).
        tolerance: f64,
        /// Iteration cap (summed over restarts).
        max_iterations: usize,
    },
    /// Sparse `L D L^T` after approximate minimum degree (ADR 0447), with the pivot checks of `Direct`. Not on the
    /// transformation path, the cyclic solve or with refinement.
    SparseDirect,
}

/// The preconditioner of [`StaticSolver::PreconditionedCg`] (ADR 0282).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum CgPreconditioner {
    /// The inverse of each node's block of free components (3 x 3 where no component is prescribed).
    BlockJacobi,
    /// Incomplete Cholesky with zero fill, the diagonal shifted on breakdown.
    IncompleteCholesky(IcOptions),
    /// Incomplete Cholesky with fill up to `level` and an ordering, the shift strategy of `IncompleteCholesky` (ADR 0357).
    IncompleteCholeskyFill {
        /// The shift strategy.
        options: IcOptions,
        /// The level of fill (0: the pattern of the matrix).
        level: usize,
        /// The ordering of the factor.
        ordering: IcOrdering,
    },
    /// One V-cycle of smoothed-aggregation multigrid with the six rigid-body modes of the free nodes as near-null space
    /// (ADR 0305).
    SmoothedAggregation(ElasticAmgSettings),
}

/// The ordering of the incomplete Cholesky factor of [`CgPreconditioner::IncompleteCholeskyFill`] (ADR 0357).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IcOrdering {
    /// The order of the reduced equations.
    Natural,
    /// Reverse Cuthill-McKee of the node graph, each node's equations kept together (the CG itself runs in the original
    /// order; only the preconditioner is permuted).
    NodeReverseCuthillMcKee,
}

/// The static solver name of a fill level and an ordering (ADR 0357 D5).
fn ic_fill_name(level: usize, ordering: IcOrdering) -> &'static str {
    let rcm = ordering == IcOrdering::NodeReverseCuthillMcKee;
    match (level, rcm) {
        (0, false) => "cg-ic0",
        (1, false) => "cg-ic1",
        (0, true) => "cg-ic0-rcm",
        (1, true) => "cg-ic1-rcm",
        (_, false) => "cg-icp",
        (_, true) => "cg-icp-rcm",
    }
}

/// What a static solve did, beside its result (ADR 0282 D4).
#[derive(Debug, Clone, PartialEq)]
pub struct SolveReport {
    /// The solver name, as in [`SolverStats`].
    pub solver: String,
    /// Iterations (0 for the direct solve), summed over restarts.
    pub iterations: usize,
    /// Restarts of CG from the true residual.
    pub restarts: usize,
    /// The relative residual `|b_F - K_FF u_F| / |b_F|`.
    pub relative_residual: f64,
    /// The shift of the incomplete Cholesky factor, if one was used.
    pub shift: Option<f64>,
    /// The breakdowns of the incomplete Cholesky factorisation, with the node and component of the failing row.
    pub breakdowns: Vec<(usize, usize, IcBreakdown)>,
}

/// Restarts of the preconditioned CG from the true residual (ADR 0282 D5).
pub const MAX_RESTARTS: usize = 3;

/// Options of the static solve.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct StaticOptions {
    /// The linear solver.
    pub solver: StaticSolver,
    /// Worker threads of the assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
    /// Steps of iterative refinement of the direct solve with the residual in compensated arithmetic (ADR 0397; 0, the
    /// default, solves as before). Refused with another solver and on the transformation path.
    pub refinement: usize,
}

impl StaticOptions {
    /// The given solver, the available parallelism.
    pub fn with_solver(solver: StaticSolver) -> StaticOptions {
        StaticOptions {
            solver,
            threads: None,
            refinement: 0,
        }
    }
    /// The same options with a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> StaticOptions {
        self.threads = Some(threads);
        self
    }
    /// The same options with `steps` steps of iterative refinement of the direct solve (ADR 0397).
    pub fn refinement(mut self, steps: usize) -> StaticOptions {
        self.refinement = steps;
        self
    }
}

/// The static solve of a model with its own constraints and loads.
pub fn solve_static(model: &SolidModel, options: &StaticOptions) -> Result<SolidResult> {
    solve_static_with(model, &[], &[], options)
}

/// The static solve of a model with, in addition to its own constraints and loads, per-node prescribed values and forces.
pub fn solve_static_with(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    options: &StaticOptions,
) -> Result<SolidResult> {
    solve_static_report(model, prescribed, forces, options).map(|(r, _)| r)
}

/// As [`solve_static_with`], also returning what the solver did (iterations, restarts, shift and breakdowns).
pub fn solve_static_report(
    model: &SolidModel,
    prescribed: &[Prescribed],
    forces: &[NodalForceAt],
    options: &StaticOptions,
) -> Result<(SolidResult, SolveReport)> {
    model.validate()?;
    // ADR 0417 D4, ADR 0432 D3, ADR 0435 D1: the elimination path and the transformation path take beams and shells (the
    // transformation has its own check). ADR 0452 D6: the elimination path also takes the per-point materials.
    model.check_supported_with_materials()?;
    if options.refinement > 0 {
        check_refinement(model, options)?;
    }
    // ADR 0296 D6: a linear constraint, a rigid set or an elastic support takes the transformation u = T q + g.
    if crate::mpc::uses_transformation(model) {
        return crate::mpc::solve_static_mpc(model, prescribed, forces, &[], options).map(|s| (s.result, s.report));
    }
    let dofs = model.dof_map();
    if let StaticSolver::PreconditionedCg {
        preconditioner: CgPreconditioner::SmoothedAggregation(_),
        ..
    } = &options.solver
    {
        // ADR 0417 D4, ADR 0432 D3: the rigid-body modes of the multigrid are those of three-unknown nodes.
        use crate::mesh::ElementKind::{Beam2, Shell4};
        if let Some(b) = model
            .mesh()
            .blocks()
            .iter()
            .position(|b| matches!(b.kind(), Beam2 | Shell4))
        {
            let kind = model.mesh().blocks()[b].kind();
            return Err(FemError::NotYetSupported {
                item: "element kind",
                variant: kind.name(),
                location: format!("block {b}, solver cg-amg-rbm"),
                package: if kind == Beam2 {
                    crate::beam::FOLLOW_UP
                } else {
                    crate::shell::FOLLOW_UP
                },
            });
        }
    }
    let f = nodal_load_vector(model, forces)?;
    let fixed = prescribed_equations(model, prescribed)?;
    check_rigid_body_motions(model, &fixed)?;
    let k = crate::assemble::stiffness_with_materials(model, options.threads)?;
    let sys = reduce(&k, &f, &fixed)?;
    let nf = sys.free().len();
    let mut report = SolveReport {
        solver: solver_name(&options.solver).to_string(),
        iterations: 0,
        restarts: 0,
        relative_residual: 0.0,
        shift: None,
        breakdowns: Vec::new(),
    };
    let (u_free, iterations) = if nf == 0 {
        (Vec::new(), 0)
    } else {
        match &options.solver {
            StaticSolver::Direct if options.refinement > 0 => {
                report.solver = REFINED_NAME.to_string();
                (
                    direct_refined(&sys.matrix, &sys.rhs, sys.free(), &dofs, options.refinement)?,
                    0,
                )
            }
            StaticSolver::Direct => (direct(&sys.matrix, &sys.rhs, sys.free(), &dofs)?, 0),
            StaticSolver::SparseDirect => (sparse_direct(&sys.matrix, &sys.rhs, sys.free(), &dofs)?, 0),
            StaticSolver::ConjugateGradient {
                tolerance,
                max_iterations,
            } => conjugate_gradient(&sys.matrix, &sys.rhs, *tolerance, *max_iterations)?,
            StaticSolver::PreconditionedCg {
                preconditioner,
                tolerance,
                max_iterations,
            } => preconditioned_cg(
                &sys.matrix,
                &sys.rhs,
                sys.free(),
                &dofs,
                model.mesh().nodes(),
                preconditioner,
                *tolerance,
                *max_iterations,
                &mut report,
            )?,
        }
    };
    let relative_residual = residual_norm(&sys.matrix, &sys.rhs, &u_free);
    report.iterations = iterations;
    report.relative_residual = relative_residual;
    let u = sys.expand(&u_free);
    let r = reactions(&k, &u, &f, &fixed);
    let result = SolidResult::new(dofs, u)?
        .with_reactions(r)?
        .with_stats(SolverStats::new(&report.solver, nf, iterations, relative_residual));
    Ok((result, report))
}

fn solver_name(s: &StaticSolver) -> &'static str {
    match s {
        StaticSolver::Direct => "skyline-ldlt",
        StaticSolver::SparseDirect => SPARSE_NAME,
        StaticSolver::ConjugateGradient { .. } => "cg-jacobi",
        StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::BlockJacobi,
            ..
        } => "cg-block-jacobi",
        StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::IncompleteCholesky(_),
            ..
        } => "cg-ic0",
        StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::IncompleteCholeskyFill { level, ordering, .. },
            ..
        } => ic_fill_name(*level, *ordering),
        StaticSolver::PreconditionedCg {
            preconditioner: CgPreconditioner::SmoothedAggregation(_),
            ..
        } => "cg-amg-rbm",
    }
}

/// The block partition of the reduced rows by node: `block_ptr` with one block per run of consecutive free equations of
/// the same node (ADR 0282 D2).
pub fn node_blocks(free: &[usize], dofs: &DofMap) -> Vec<usize> {
    let mut ptr = vec![0];
    for r in 1..free.len() {
        let a = dofs.node_component(free[r - 1]).map(|(n, _)| n);
        let b = dofs.node_component(free[r]).map(|(n, _)| n);
        if a != b {
            ptr.push(r);
        }
    }
    if !free.is_empty() {
        ptr.push(free.len());
    }
    ptr
}

/// The multigrid of [`CgPreconditioner::SmoothedAggregation`] for the reduced stiffness `a` (ADR 0305 D7): the node
/// blocks of the free equations and the six rigid-body modes of their nodes, from the coordinates `nodes`.
pub fn smoothed_aggregation(
    a: &CsrMatrix,
    free: &[usize],
    dofs: &DofMap,
    nodes: &[rcfd_core::vec3::Vec3],
    settings: &ElasticAmgSettings,
) -> std::result::Result<ElasticAmg, rcfd_linalg::amg_elastic::ElasticAmgError> {
    let (points, components): (Vec<[f64; 3]>, Vec<usize>) = free
        .iter()
        .map(|&q| {
            let (n, c) = dofs.node_component(q).expect("free equation exists");
            (nodes[n], c)
        })
        .unzip();
    let modes = rigid_body_modes(&points, &components);
    // ADR 0370 D2: the geometric aggregation takes the point of each node block.
    if settings.aggregation != Aggregation::Greedy {
        let bp = node_blocks(free, dofs);
        let block_points: Vec<[f64; 3]> = bp[..bp.len().saturating_sub(1)].iter().map(|&s| points[s]).collect();
        return ElasticAmg::with_points(a, &bp, &modes, &block_points, settings);
    }
    ElasticAmg::new(a, &node_blocks(free, dofs), &modes, settings)
}

fn precond_reason(e: &PrecondError, free: &[usize], dofs: &DofMap) -> String {
    let at = |row: usize| match free.get(row).and_then(|&q| dofs.node_component(q)) {
        Some((n, c)) => format!(" (node {n} component {c})"),
        None => String::new(),
    };
    let row = match e {
        PrecondError::MissingDiagonal { row }
        | PrecondError::NonPositiveDiagonal { row, .. }
        | PrecondError::NonFinite { row, .. }
        | PrecondError::BlockNotPositive { row, .. } => Some(*row),
        PrecondError::NoShift { breakdowns, .. } => breakdowns.last().map(|b| b.row),
        _ => None,
    };
    format!("{e}{}", row.map_or(String::new(), at))
}

#[allow(clippy::too_many_arguments)]
fn preconditioned_cg(
    a: &CsrMatrix,
    b: &[f64],
    free: &[usize],
    dofs: &DofMap,
    nodes: &[rcfd_core::vec3::Vec3],
    preconditioner: &CgPreconditioner,
    tolerance: f64,
    max_iterations: usize,
    report: &mut SolveReport,
) -> Result<(Vec<f64>, usize)> {
    let solver = solver_name(&StaticSolver::PreconditionedCg {
        preconditioner: preconditioner.clone(),
        tolerance,
        max_iterations,
    });
    let m: Box<dyn Preconditioner> = match preconditioner {
        CgPreconditioner::BlockJacobi => {
            Box::new(
                BlockJacobi::new(a, &node_blocks(free, dofs)).map_err(|e| FemError::Solver {
                    solver,
                    reason: precond_reason(&e, free, dofs),
                })?,
            )
        }
        CgPreconditioner::IncompleteCholesky(o) => {
            let ic = IncompleteCholesky::new(a, o).map_err(|e| FemError::Solver {
                solver,
                reason: precond_reason(&e, free, dofs),
            })?;
            report.shift = Some(ic.shift());
            report.breakdowns = ic
                .breakdowns()
                .iter()
                .map(|bd| {
                    let (n, c) = dofs.node_component(free[bd.row]).expect("free equation exists");
                    (n, c, *bd)
                })
                .collect();
            if !ic.breakdowns().is_empty() {
                report.solver = format!(
                    "cg-ic0, shift {:e} after {} breakdown(s)",
                    ic.shift(),
                    ic.breakdowns().len()
                );
            }
            Box::new(ic)
        }
        CgPreconditioner::IncompleteCholeskyFill {
            options,
            level,
            ordering,
        } => {
            // ADR 0357 D5: rows of the factor are mapped to free equations through the ordering.
            let order = match ordering {
                IcOrdering::NodeReverseCuthillMcKee => Some(block_rcm_order(a, &node_blocks(free, dofs))),
                _ => None,
            };
            let free_p: Vec<usize> = match &order {
                Some(o) => o.iter().map(|&r| free[r]).collect(),
                None => free.to_vec(),
            };
            let factored = match &order {
                Some(o) => IncompleteCholesky::with_fill(&a.permute_symmetric(o), *level, options),
                None => IncompleteCholesky::with_fill(a, *level, options),
            };
            let ic = factored.map_err(|e| FemError::Solver {
                solver,
                reason: precond_reason(&e, &free_p, dofs),
            })?;
            report.shift = Some(ic.shift());
            report.breakdowns = ic
                .breakdowns()
                .iter()
                .map(|bd| {
                    let (n, c) = dofs.node_component(free_p[bd.row]).expect("free equation exists");
                    (n, c, *bd)
                })
                .collect();
            let suffix = if order.is_some() { "-rcm" } else { "" };
            report.solver = format!("cg-ic{level}{suffix}");
            if !ic.breakdowns().is_empty() {
                report.solver = format!(
                    "{}, shift {:e} after {} breakdown(s)",
                    report.solver,
                    ic.shift(),
                    ic.breakdowns().len()
                );
            }
            match order {
                Some(o) => Box::new(Permuted::new(o, ic)),
                None => Box::new(ic),
            }
        }
        CgPreconditioner::SmoothedAggregation(settings) => {
            let amg = smoothed_aggregation(a, free, dofs, nodes, settings).map_err(|e| FemError::Solver {
                solver,
                reason: e.to_string(),
            })?;
            Box::new(amg)
        }
    };
    let target = tolerance * norm2(b);
    let mut x = vec![0.0; b.len()];
    let mut used = 0;
    let mut r = vec![0.0; b.len()];
    loop {
        let control = SolveControl {
            tol_abs: target,
            tol_rel: 0.0,
            max_iter: max_iterations - used,
        };
        let s = if report.restarts == 0 {
            cg(a, b, &mut x, m.as_ref(), &control)
        } else {
            // ADR 0282 amendment 2 (D5'): a restart is a step of iterative refinement; the correction of the true
            // residual `r` is iterated from zero, so its roundings are those of the correction, and `x` is rounded once.
            let mut delta = vec![0.0; b.len()];
            let s = cg(a, &r, &mut delta, m.as_ref(), &control);
            for (xi, di) in x.iter_mut().zip(&delta) {
                *xi += di;
            }
            s
        };
        used += s.iterations;
        if !s.final_residual.is_finite() || x.iter().any(|v| !v.is_finite()) {
            return Err(FemError::Solver {
                solver,
                reason: format!("non-finite iterate after {used} iterations"),
            });
        }
        a.residual(b, &x, &mut r);
        let true_res = norm2(&r);
        if true_res <= target {
            return Ok((x, used));
        }
        if !s.converged || report.restarts == MAX_RESTARTS || used >= max_iterations {
            return Err(FemError::Solver {
                solver,
                reason: format!(
                    "not converged in {used} iterations and {} restart(s) (true residual {:e}, target {:e} = \
                     {tolerance:e} relative)",
                    report.restarts, true_res, target
                ),
            });
        }
        report.restarts += 1;
    }
}

/// The solver name of the direct solve with iterative refinement (ADR 0397).
pub const REFINED_NAME: &str = "skyline-ldlt-refined";

/// The refusals of ADR 0397 D3: refinement only with the direct solver and not on the transformation path.
fn check_refinement(model: &SolidModel, options: &StaticOptions) -> Result<()> {
    if options.solver != StaticSolver::Direct {
        return Err(FemError::Solver {
            solver: solver_name(&options.solver),
            reason: "iterative refinement (ADR 0397) applies to the direct solve only".to_string(),
        });
    }
    if crate::mpc::uses_transformation(model) {
        return Err(FemError::Solver {
            solver: REFINED_NAME,
            reason: "iterative refinement (ADR 0397) is not available on the transformation path".to_string(),
        });
    }
    Ok(())
}

/// The direct solve followed by `steps` steps of [`crate::refine::refine_step`] with the same factor (ADR 0397 D3).
fn direct_refined(a: &CsrMatrix, b: &[f64], free: &[usize], dofs: &DofMap, steps: usize) -> Result<Vec<f64>> {
    let ldlt = direct_factor(a, free, dofs)?;
    let mut x = ldlt.solve(b);
    for _ in 0..steps {
        crate::refine::refine_step(a, b, &ldlt, &mut x);
    }
    Ok(x)
}

fn direct(a: &CsrMatrix, b: &[f64], free: &[usize], dofs: &crate::model::DofMap) -> Result<Vec<f64>> {
    Ok(direct_factor(a, free, dofs)?.solve(b))
}

/// The `L D L^T` factor of the direct solve with its pivot checks (ADR 0275 arithmetic 4).
fn direct_factor(a: &CsrMatrix, free: &[usize], dofs: &crate::model::DofMap) -> Result<SkylineLdlt> {
    let diag = a.diag();
    let at = |reduced: usize| dofs.node_component(free[reduced]).expect("free equation exists");
    let ldlt = SkylineLdlt::factor(a).map_err(|e| match e {
        SkylineError::Pivot { row, pivot, .. } => {
            let (node, component) = at(row);
            FemError::SingularStiffness {
                node,
                component,
                pivot,
                diagonal: diag[row],
            }
        }
        other => FemError::Solver {
            solver: "skyline-ldlt",
            reason: other.to_string(),
        },
    })?;
    for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
        if d <= PIVOT_TOLERANCE * diag[row] {
            let (node, component) = at(row);
            return Err(FemError::SingularStiffness {
                node,
                component,
                pivot: d,
                diagonal: diag[row],
            });
        }
    }
    Ok(ldlt)
}

/// The solver name of [`StaticSolver::SparseDirect`] (ADR 0447).
pub const SPARSE_NAME: &str = "sparse-ldlt";

/// The sparse `L D L^T` solve with the pivot checks of [`direct_factor`] (ADR 0447 D5).
fn sparse_direct(a: &CsrMatrix, b: &[f64], free: &[usize], dofs: &DofMap) -> Result<Vec<f64>> {
    let diag = a.diag();
    let at = |reduced: usize| dofs.node_component(free[reduced]).expect("free equation exists");
    let ldlt = SparseLdlt::factor(a).map_err(|e| match e {
        SkylineError::Pivot { row, pivot, .. } => {
            let (node, component) = at(row);
            FemError::SingularStiffness {
                node,
                component,
                pivot,
                diagonal: diag[row],
            }
        }
        other => FemError::Solver {
            solver: SPARSE_NAME,
            reason: other.to_string(),
        },
    })?;
    for (&row, &d) in ldlt.order().iter().zip(ldlt.pivots()) {
        if d <= PIVOT_TOLERANCE * diag[row] {
            let (node, component) = at(row);
            return Err(FemError::SingularStiffness {
                node,
                component,
                pivot: d,
                diagonal: diag[row],
            });
        }
    }
    Ok(ldlt.solve(b))
}

fn conjugate_gradient(a: &CsrMatrix, b: &[f64], tolerance: f64, max_iterations: usize) -> Result<(Vec<f64>, usize)> {
    let mut x = vec![0.0; b.len()];
    let m = Jacobi::new(&a.diag());
    let control = SolveControl {
        tol_abs: 0.0,
        tol_rel: tolerance,
        max_iter: max_iterations,
    };
    let s = cg(a, b, &mut x, &m, &control);
    if !s.converged {
        return Err(FemError::Solver {
            solver: "cg-jacobi",
            reason: format!(
                "not converged in {} iterations (residual {:e} from {:e}, tolerance {tolerance:e} relative)",
                s.iterations, s.final_residual, s.initial_residual
            ),
        });
    }
    Ok((x, s.iterations))
}

fn residual_norm(a: &CsrMatrix, b: &[f64], x: &[f64]) -> f64 {
    let bn = b.iter().map(|v| v * v).sum::<f64>().sqrt();
    if bn == 0.0 || x.is_empty() {
        return 0.0;
    }
    let mut r = vec![0.0; b.len()];
    a.residual(b, x, &mut r);
    r.iter().map(|v| v * v).sum::<f64>().sqrt() / bn
}
