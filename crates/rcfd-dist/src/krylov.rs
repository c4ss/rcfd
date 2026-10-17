//! Distributed CG and BiCGStab (ADR 0058, D6): the `rcfd-linalg` routines operation for operation, with the inner products
//! replaced by the deterministic global reduction of D3 (fixed-chunk local dot over owned entries, then rank-ordered sum).

use crate::comm::{CommError, Communicator};
use crate::matrix::DistOperator;
use rcfd_linalg::precond::Preconditioner;
use rcfd_linalg::solvers::{SolveControl, SolveStats};
use rcfd_linalg::vecops::{axpy, dot, xpby};

/// Global inner product of owned-length vectors.
pub fn gdot(comm: &dyn Communicator, a: &[f64], b: &[f64]) -> Result<f64, CommError> {
    comm.allreduce_sum(dot(a, b))
}

/// Global Euclidean norm of an owned-length vector.
pub fn gnorm2(comm: &dyn Communicator, a: &[f64]) -> Result<f64, CommError> {
    Ok(gdot(comm, a, a)?.sqrt())
}

fn residual(op: &DistOperator, b: &[f64], x: &[f64], r: &mut [f64]) -> Result<(), CommError> {
    op.try_apply(x, r)?;
    for (ri, bi) in r.iter_mut().zip(b) {
        *ri = bi - *ri;
    }
    Ok(())
}

fn done(res: f64, res0: f64, c: &SolveControl) -> bool {
    res <= c.tol_abs || res <= c.tol_rel * res0
}

/// Preconditioned conjugate gradients on a distributed SPD system. `b` and `x` are owned-length.
pub fn cg(
    op: &DistOperator,
    b: &[f64],
    x: &mut [f64],
    m: &dyn Preconditioner,
    c: &SolveControl,
) -> Result<SolveStats, CommError> {
    let comm = op.comm;
    let n = op.mat.n_owned;
    let (mut r, mut z, mut p, mut q) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    residual(op, b, x, &mut r)?;
    let res0 = gnorm2(comm, &r)?;
    let mut res = res0;
    if done(res, res0, c) {
        return Ok(SolveStats {
            iterations: 0,
            initial_residual: res0,
            final_residual: res,
            converged: true,
        });
    }
    m.apply(&r, &mut z);
    p.copy_from_slice(&z);
    let mut rz = gdot(comm, &r, &z)?;
    let mut it = 0;
    while it < c.max_iter {
        op.try_apply(&p, &mut q)?;
        let pq = gdot(comm, &p, &q)?;
        if pq == 0.0 {
            break;
        }
        let alpha = rz / pq;
        axpy(alpha, &p, x);
        axpy(-alpha, &q, &mut r);
        it += 1;
        res = gnorm2(comm, &r)?;
        if done(res, res0, c) {
            return Ok(SolveStats {
                iterations: it,
                initial_residual: res0,
                final_residual: res,
                converged: true,
            });
        }
        m.apply(&r, &mut z);
        let rz_new = gdot(comm, &r, &z)?;
        let beta = rz_new / rz;
        rz = rz_new;
        xpby(&z, beta, &mut p);
    }
    Ok(SolveStats {
        iterations: it,
        initial_residual: res0,
        final_residual: res,
        converged: false,
    })
}

/// Preconditioned BiCGStab on a distributed general system. `b` and `x` are owned-length.
pub fn bicgstab(
    op: &DistOperator,
    b: &[f64],
    x: &mut [f64],
    m: &dyn Preconditioner,
    c: &SolveControl,
) -> Result<SolveStats, CommError> {
    let comm = op.comm;
    let n = op.mat.n_owned;
    let mut r = vec![0.0; n];
    residual(op, b, x, &mut r)?;
    let res0 = gnorm2(comm, &r)?;
    let mut res = res0;
    if done(res, res0, c) {
        return Ok(SolveStats {
            iterations: 0,
            initial_residual: res0,
            final_residual: res,
            converged: true,
        });
    }
    let r0 = r.clone();
    let (mut p, mut v) = (vec![0.0; n], vec![0.0; n]);
    let (mut ph, mut s, mut sh, mut t) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    let (mut rho, mut alpha, mut omega) = (1.0, 1.0, 1.0);
    let mut it = 0;
    while it < c.max_iter {
        let rho_new = gdot(comm, &r0, &r)?;
        if rho_new == 0.0 || omega == 0.0 {
            break;
        }
        let beta = (rho_new / rho) * (alpha / omega);
        rho = rho_new;
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
        }
        m.apply(&p, &mut ph);
        op.try_apply(&ph, &mut v)?;
        let r0v = gdot(comm, &r0, &v)?;
        if r0v == 0.0 {
            break;
        }
        alpha = rho / r0v;
        for i in 0..n {
            s[i] = r[i] - alpha * v[i];
        }
        it += 1;
        let sn = gnorm2(comm, &s)?;
        if done(sn, res0, c) {
            axpy(alpha, &ph, x);
            return Ok(SolveStats {
                iterations: it,
                initial_residual: res0,
                final_residual: sn,
                converged: true,
            });
        }
        m.apply(&s, &mut sh);
        op.try_apply(&sh, &mut t)?;
        let tt = gdot(comm, &t, &t)?;
        if tt == 0.0 {
            break;
        }
        omega = gdot(comm, &t, &s)? / tt;
        axpy(alpha, &ph, x);
        axpy(omega, &sh, x);
        for i in 0..n {
            r[i] = s[i] - omega * t[i];
        }
        res = gnorm2(comm, &r)?;
        if done(res, res0, c) {
            return Ok(SolveStats {
                iterations: it,
                initial_residual: res0,
                final_residual: res,
                converged: true,
            });
        }
    }
    Ok(SolveStats {
        iterations: it,
        initial_residual: res0,
        final_residual: res,
        converged: false,
    })
}
