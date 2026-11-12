//! The linear static solve (solver choice: ADR 0282). Declared by ADR 0271; filled by ADR 0275.
//!
//! Steps (ADR 0275 D6): the load vector ([`crate::loads::nodal_load_vector`]), the prescribed equations and the check of
//! free rigid-body motions ([`crate::constraint`]), the stiffness ([`crate::assemble::stiffness_with`]), the symmetric
//! elimination ([`crate::constraint::reduce`]), the solve of `K_FF u_F = f_F - K_FP g` by the envelope `L D L^T` of ADR 0274
//! (default) or by CG with Jacobi, and the reactions `K u - f`. A pivot `d_i <= 1e-10 a_ii` of the direct solve is refused
//! as a singular stiffness (a mechanism the constraint check cannot see; ADR 0275 arithmetic 4).

use crate::assemble::stiffness_with;
use crate::constraint::{check_rigid_body_motions, prescribed_equations, reactions, reduce, Prescribed};
use crate::error::{FemError, Result};
use crate::loads::{nodal_load_vector, NodalForceAt};
use crate::model::{SolidModel, SolidResult, SolverStats};
use rcfd_linalg::skyline::{SkylineError, SkylineLdlt};
use rcfd_linalg::{cg, CsrMatrix, Jacobi, SolveControl};

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
}

/// Options of the static solve.
#[derive(Debug, Clone, PartialEq, Default)]
#[non_exhaustive]
pub struct StaticOptions {
    /// The linear solver.
    pub solver: StaticSolver,
    /// Worker threads of the assembly (`None`: the available parallelism); the result does not depend on it.
    pub threads: Option<usize>,
}

impl StaticOptions {
    /// The given solver, the available parallelism.
    pub fn with_solver(solver: StaticSolver) -> StaticOptions {
        StaticOptions { solver, threads: None }
    }
    /// The same options with a thread count for the assembly.
    pub fn threads(mut self, threads: usize) -> StaticOptions {
        self.threads = Some(threads);
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
    model.validate()?;
    let dofs = model.dof_map();
    let f = nodal_load_vector(model, forces)?;
    let fixed = prescribed_equations(model, prescribed)?;
    check_rigid_body_motions(model, &fixed)?;
    let k = stiffness_with(model, options.threads)?;
    let sys = reduce(&k, &f, &fixed)?;
    let nf = sys.free().len();
    let (u_free, name, iterations) = if nf == 0 {
        (Vec::new(), solver_name(&options.solver), 0)
    } else {
        match &options.solver {
            StaticSolver::Direct => (direct(&sys.matrix, &sys.rhs, sys.free(), &dofs)?, "skyline-ldlt", 0),
            StaticSolver::ConjugateGradient {
                tolerance,
                max_iterations,
            } => {
                let (u, it) = conjugate_gradient(&sys.matrix, &sys.rhs, *tolerance, *max_iterations)?;
                (u, "cg-jacobi", it)
            }
        }
    };
    let relative_residual = residual_norm(&sys.matrix, &sys.rhs, &u_free);
    let u = sys.expand(&u_free);
    let r = reactions(&k, &u, &f, &fixed);
    Ok(SolidResult::new(dofs, u)?
        .with_reactions(r)?
        .with_stats(SolverStats::new(name, nf, iterations, relative_residual)))
}

fn solver_name(s: &StaticSolver) -> &'static str {
    match s {
        StaticSolver::Direct => "skyline-ldlt",
        StaticSolver::ConjugateGradient { .. } => "cg-jacobi",
    }
}

fn direct(a: &CsrMatrix, b: &[f64], free: &[usize], dofs: &crate::model::DofMap) -> Result<Vec<f64>> {
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
