//! In-process driver: scatter a global system over a decomposition, solve it on P ranks (threads) or on the serial fallback,
//! gather the global solution (ADR 0058, D8).

use crate::comm::{run_ranks, Communicator, SerialComm};
use crate::krylov;
use crate::matrix::{BlockJacobi, DistCsr, DistOperator};
use rcfd_core::{Error, Result};
use rcfd_linalg::precond::{Identity, Jacobi, Preconditioner};
use rcfd_linalg::solvers::{SolveControl, SolveStats};
use rcfd_linalg::CsrMatrix;
use rcfd_mesh::partition::Decomposition;
use std::time::Instant;

/// Krylov method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Conjugate gradients (SPD systems).
    Cg,
    /// BiCGStab (general systems).
    BiCgStab,
}

/// Preconditioner choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precond {
    /// None.
    None,
    /// Point Jacobi (partition independent).
    Jacobi,
    /// Non-overlapping block-Jacobi with ILU(0) blocks.
    BlockJacobi,
}

/// Result of a gathered solve.
#[derive(Debug, Clone)]
pub struct DistSolveResult {
    /// Solution in the global numbering.
    pub x: Vec<f64>,
    /// Per-rank statistics (identical on every rank by construction).
    pub stats: Vec<SolveStats>,
    /// Wall time of the solve loop in seconds (maximum over ranks, measured after the matrices are built).
    pub solve_secs: f64,
}

/// Solve one rank's part of the system on `comm`; `b` and `x` are owned-length.
pub fn solve_rank(
    mat: &DistCsr,
    comm: &dyn Communicator,
    b: &[f64],
    x: &mut [f64],
    method: Method,
    precond: Precond,
    ctl: &SolveControl,
) -> Result<SolveStats> {
    let op = DistOperator::new(mat, comm);
    let m: Box<dyn Preconditioner> = match precond {
        Precond::None => Box::new(Identity),
        Precond::Jacobi => Box::new(Jacobi::new(&mat.diag())),
        Precond::BlockJacobi => Box::new(BlockJacobi::new(mat)),
    };
    let stats = match method {
        Method::Cg => krylov::cg(&op, b, x, m.as_ref(), ctl),
        Method::BiCgStab => krylov::bicgstab(&op, b, x, m.as_ref(), ctl),
    };
    stats.map_err(Error::from)
}

/// Solve `a x = b` (global numbering, start from `x0`) over `dec`. One part uses [`SerialComm`] on the calling thread unless
/// `force_threads` is set; more parts use one thread per rank with the in-process transport.
pub fn solve_distributed(
    a: &CsrMatrix,
    b: &[f64],
    x0: &[f64],
    dec: &Decomposition,
    method: Method,
    precond: Precond,
    ctl: &SolveControl,
    force_threads: bool,
) -> Result<DistSolveResult> {
    if b.len() != a.n || x0.len() != a.n {
        return Err(Error::Mesh(format!(
            "solve_distributed: matrix {} vs rhs {} vs x0 {}",
            a.n,
            b.len(),
            x0.len()
        )));
    }
    let p = dec.n_parts;
    let mats: Vec<DistCsr> = (0..p).map(|r| DistCsr::from_global(a, dec, r)).collect::<Result<_>>()?;
    let run = |comm: &dyn Communicator| -> Result<(Vec<f64>, SolveStats, f64)> {
        let mat = &mats[comm.rank()];
        let bl: Vec<f64> = mat.owned.iter().map(|&c| b[c]).collect();
        let mut xl: Vec<f64> = mat.owned.iter().map(|&c| x0[c]).collect();
        comm.barrier()?;
        let t0 = Instant::now();
        let st = solve_rank(mat, comm, &bl, &mut xl, method, precond, ctl)?;
        Ok((xl, st, t0.elapsed().as_secs_f64()))
    };
    let per_rank: Vec<Result<(Vec<f64>, SolveStats, f64)>> = if p == 1 && !force_threads {
        vec![run(&SerialComm)]
    } else {
        run_ranks(p, |c| run(c))
    };
    let mut x = vec![0.0; a.n];
    let mut stats = Vec::new();
    let mut secs: f64 = 0.0;
    for (r, res) in per_rank.into_iter().enumerate() {
        let (xl, st, t) = res?;
        for (&c, v) in mats[r].owned.iter().zip(xl) {
            x[c] = v;
        }
        stats.push(st);
        secs = secs.max(t);
    }
    Ok(DistSolveResult {
        x,
        stats,
        solve_secs: secs,
    })
}
