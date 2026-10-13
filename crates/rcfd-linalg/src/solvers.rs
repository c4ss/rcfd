//! Krylov solvers: preconditioned CG, BiCGStab and restarted GMRES.

use crate::matrix::{CsrMatrix, LduMatrix};
use crate::precond::Preconditioner;
use crate::vecops::{axpy, dot, norm2};

/// Square linear operator.
pub trait LinearOperator: Sync {
    /// Dimension.
    fn size(&self) -> usize;
    /// `y = A x`
    fn apply(&self, x: &[f64], y: &mut [f64]);
}

impl LinearOperator for CsrMatrix {
    fn size(&self) -> usize {
        self.n
    }
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        self.matvec(x, y)
    }
}

impl LinearOperator for LduMatrix {
    fn size(&self) -> usize {
        self.addr.n
    }
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        self.matvec(x, y)
    }
}

/// Convergence controls. The solve stops when the L2 residual is below
/// `tol_abs` or below `tol_rel` times the initial residual.
#[derive(Debug, Clone, Copy)]
pub struct SolveControl {
    /// Absolute residual tolerance.
    pub tol_abs: f64,
    /// Relative residual tolerance.
    pub tol_rel: f64,
    /// Iteration cap.
    pub max_iter: usize,
}

impl Default for SolveControl {
    fn default() -> Self {
        SolveControl {
            tol_abs: 1e-14,
            tol_rel: 1e-8,
            max_iter: 1000,
        }
    }
}

/// Outcome of a linear solve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolveStats {
    /// Iterations performed.
    pub iterations: usize,
    /// Initial L2 residual.
    pub initial_residual: f64,
    /// Final L2 residual (recursive estimate).
    pub final_residual: f64,
    /// Whether a tolerance was met.
    pub converged: bool,
}

fn residual<A: LinearOperator>(a: &A, b: &[f64], x: &[f64], r: &mut [f64]) {
    a.apply(x, r);
    for (ri, bi) in r.iter_mut().zip(b) {
        *ri = bi - *ri;
    }
}

fn done(res: f64, res0: f64, c: &SolveControl) -> bool {
    res <= c.tol_abs || res <= c.tol_rel * res0
}

/// Preconditioned conjugate gradients for symmetric positive definite systems.
pub fn cg<A: LinearOperator>(a: &A, b: &[f64], x: &mut [f64], m: &dyn Preconditioner, c: &SolveControl) -> SolveStats {
    let n = a.size();
    let (mut r, mut z, mut p, mut q) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    residual(a, b, x, &mut r);
    let res0 = norm2(&r);
    let mut res = res0;
    if done(res, res0, c) {
        return SolveStats {
            iterations: 0,
            initial_residual: res0,
            final_residual: res,
            converged: true,
        };
    }
    m.apply(&r, &mut z);
    p.copy_from_slice(&z);
    let mut rz = dot(&r, &z);
    let mut it = 0;
    while it < c.max_iter {
        a.apply(&p, &mut q);
        let pq = dot(&p, &q);
        if pq == 0.0 {
            break;
        }
        let alpha = rz / pq;
        axpy(alpha, &p, x);
        axpy(-alpha, &q, &mut r);
        it += 1;
        res = norm2(&r);
        if done(res, res0, c) {
            return SolveStats {
                iterations: it,
                initial_residual: res0,
                final_residual: res,
                converged: true,
            };
        }
        m.apply(&r, &mut z);
        let rz_new = dot(&r, &z);
        let beta = rz_new / rz;
        rz = rz_new;
        crate::vecops::xpby(&z, beta, &mut p);
    }
    SolveStats {
        iterations: it,
        initial_residual: res0,
        final_residual: res,
        converged: false,
    }
}

/// Preconditioned BiCGStab for general nonsingular systems.
pub fn bicgstab<A: LinearOperator>(
    a: &A,
    b: &[f64],
    x: &mut [f64],
    m: &dyn Preconditioner,
    c: &SolveControl,
) -> SolveStats {
    let n = a.size();
    let mut r = vec![0.0; n];
    residual(a, b, x, &mut r);
    let res0 = norm2(&r);
    let mut res = res0;
    if done(res, res0, c) {
        return SolveStats {
            iterations: 0,
            initial_residual: res0,
            final_residual: res,
            converged: true,
        };
    }
    let r0 = r.clone();
    let (mut p, mut v) = (vec![0.0; n], vec![0.0; n]);
    let (mut ph, mut s, mut sh, mut t) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    let (mut rho, mut alpha, mut omega) = (1.0, 1.0, 1.0);
    let mut it = 0;
    while it < c.max_iter {
        let rho_new = dot(&r0, &r);
        if rho_new == 0.0 || omega == 0.0 {
            break;
        }
        let beta = (rho_new / rho) * (alpha / omega);
        rho = rho_new;
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
        }
        m.apply(&p, &mut ph);
        a.apply(&ph, &mut v);
        let r0v = dot(&r0, &v);
        if r0v == 0.0 {
            break;
        }
        alpha = rho / r0v;
        for i in 0..n {
            s[i] = r[i] - alpha * v[i];
        }
        it += 1;
        let sn = norm2(&s);
        if done(sn, res0, c) {
            axpy(alpha, &ph, x);
            return SolveStats {
                iterations: it,
                initial_residual: res0,
                final_residual: sn,
                converged: true,
            };
        }
        m.apply(&s, &mut sh);
        a.apply(&sh, &mut t);
        let tt = dot(&t, &t);
        if tt == 0.0 {
            break;
        }
        omega = dot(&t, &s) / tt;
        axpy(alpha, &ph, x);
        axpy(omega, &sh, x);
        for i in 0..n {
            r[i] = s[i] - omega * t[i];
        }
        res = norm2(&r);
        if done(res, res0, c) {
            return SolveStats {
                iterations: it,
                initial_residual: res0,
                final_residual: res,
                converged: true,
            };
        }
    }
    SolveStats {
        iterations: it,
        initial_residual: res0,
        final_residual: res,
        converged: false,
    }
}

/// Right-preconditioned restarted GMRES(`restart`).
pub fn gmres<A: LinearOperator>(
    a: &A,
    b: &[f64],
    x: &mut [f64],
    m: &dyn Preconditioner,
    restart: usize,
    c: &SolveControl,
) -> SolveStats {
    let n = a.size();
    let restart = restart.max(1);
    let mut r = vec![0.0; n];
    residual(a, b, x, &mut r);
    let res0 = norm2(&r);
    let mut res = res0;
    if done(res, res0, c) {
        return SolveStats {
            iterations: 0,
            initial_residual: res0,
            final_residual: res,
            converged: true,
        };
    }
    let mut it = 0;
    let (mut w, mut zt) = (vec![0.0; n], vec![0.0; n]);
    while it < c.max_iter {
        residual(a, b, x, &mut r);
        let beta = norm2(&r);
        res = beta;
        if done(res, res0, c) {
            return SolveStats {
                iterations: it,
                initial_residual: res0,
                final_residual: res,
                converged: true,
            };
        }
        let mut v: Vec<Vec<f64>> = vec![r.iter().map(|ri| ri / beta).collect()];
        let mut h = vec![vec![0.0; restart]; restart + 1];
        let (mut cs, mut sn) = (vec![0.0; restart], vec![0.0; restart]);
        let mut g = vec![0.0; restart + 1];
        g[0] = beta;
        let mut k = 0;
        while k < restart && it < c.max_iter {
            m.apply(&v[k], &mut zt);
            a.apply(&zt, &mut w);
            for j in 0..=k {
                h[j][k] = dot(&w, &v[j]);
                axpy(-h[j][k], &v[j], &mut w);
            }
            h[k + 1][k] = norm2(&w);
            let nv = if h[k + 1][k] > 0.0 {
                w.iter().map(|wi| wi / h[k + 1][k]).collect()
            } else {
                vec![0.0; n]
            };
            v.push(nv);
            for j in 0..k {
                let t = cs[j] * h[j][k] + sn[j] * h[j + 1][k];
                h[j + 1][k] = -sn[j] * h[j][k] + cs[j] * h[j + 1][k];
                h[j][k] = t;
            }
            let d = h[k][k].hypot(h[k + 1][k]);
            if d == 0.0 {
                cs[k] = 1.0;
                sn[k] = 0.0;
            } else {
                cs[k] = h[k][k] / d;
                sn[k] = h[k + 1][k] / d;
            }
            h[k][k] = d;
            h[k + 1][k] = 0.0;
            g[k + 1] = -sn[k] * g[k];
            g[k] *= cs[k];
            res = g[k + 1].abs();
            k += 1;
            it += 1;
            if done(res, res0, c) {
                break;
            }
        }
        // solve upper-triangular H y = g and update x += M^-1 V y
        let mut y = vec![0.0; k];
        for i in (0..k).rev() {
            let mut s = g[i];
            for j in i + 1..k {
                s -= h[i][j] * y[j];
            }
            y[i] = s / h[i][i];
        }
        let mut upd = vec![0.0; n];
        for (j, yj) in y.iter().enumerate() {
            axpy(*yj, &v[j], &mut upd);
        }
        m.apply(&upd, &mut zt);
        axpy(1.0, &zt, x);
        if done(res, res0, c) {
            return SolveStats {
                iterations: it,
                initial_residual: res0,
                final_residual: res,
                converged: true,
            };
        }
    }
    SolveStats {
        iterations: it,
        initial_residual: res0,
        final_residual: res,
        converged: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::poisson_2d;
    use crate::precond::{Ic0, Identity, Ilu0, Jacobi};

    fn true_res<A: LinearOperator>(a: &A, b: &[f64], x: &[f64]) -> f64 {
        let mut r = vec![0.0; b.len()];
        residual(a, b, x, &mut r);
        norm2(&r)
    }

    fn setup(nx: usize) -> (LduMatrix, Vec<f64>, Vec<f64>) {
        let m = poisson_2d(nx, nx);
        let n = nx * nx;
        let xe: Vec<f64> = (0..n).map(|i| ((i * 37 + 11) % 17) as f64 / 8.0 - 1.0).collect();
        let mut b = vec![0.0; n];
        m.matvec(&xe, &mut b);
        (m, xe, b)
    }

    fn err(x: &[f64], xe: &[f64]) -> f64 {
        x.iter().zip(xe).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max)
    }

    #[test]
    fn cg_variants_converge_and_preconditioning_helps() {
        let (m, xe, b) = setup(40);
        let c = SolveControl {
            tol_abs: 1e-13,
            tol_rel: 1e-10,
            max_iter: 2000,
        };
        let mut iters = Vec::new();
        for name in ["identity", "jacobi", "dic"] {
            let mut x = vec![0.0; b.len()];
            let st = match name {
                "identity" => cg(&m, &b, &mut x, &Identity, &c),
                "jacobi" => cg(&m, &b, &mut x, &Jacobi::new(&m.diag), &c),
                _ => cg(&m, &b, &mut x, &Ic0::new(&m), &c),
            };
            assert!(st.converged, "{name}");
            assert!(err(&x, &xe) < 1e-8, "{name} err {}", err(&x, &xe));
            assert!(true_res(&m, &b, &x) < 1e-8 * norm2(&b));
            println!("CG {name}: {} iterations", st.iterations);
            iters.push(st.iterations);
        }
        assert!(iters[2] < iters[0], "DIC should beat unpreconditioned CG: {iters:?}");
    }

    #[test]
    fn bicgstab_and_gmres_on_nonsymmetric() {
        let (mut m, _, _) = setup(25);
        // add convection-like asymmetry, keep diagonally dominant
        for f in 0..m.upper.len() {
            m.upper[f] -= 0.4;
            m.lower[f] += 0.4;
        }
        for d in m.diag.iter_mut() {
            *d += 1.0;
        }
        let csr = m.to_csr();
        let n = m.addr.n;
        let xe: Vec<f64> = (0..n).map(|i| (i as f64 * 0.01).cos()).collect();
        let mut b = vec![0.0; n];
        csr.matvec(&xe, &mut b);
        let c = SolveControl {
            tol_abs: 1e-13,
            tol_rel: 1e-10,
            max_iter: 2000,
        };
        let ilu = Ilu0::new(&csr);

        let mut x = vec![0.0; n];
        let s1 = bicgstab(&csr, &b, &mut x, &Identity, &c);
        assert!(s1.converged && err(&x, &xe) < 1e-7);
        let mut x = vec![0.0; n];
        let s2 = bicgstab(&csr, &b, &mut x, &ilu, &c);
        assert!(s2.converged && err(&x, &xe) < 1e-7);
        let mut x = vec![0.0; n];
        let s3 = gmres(&csr, &b, &mut x, &ilu, 30, &c);
        assert!(s3.converged && err(&x, &xe) < 1e-7, "gmres {s3:?}");
        let mut x = vec![0.0; n];
        let s4 = gmres(&csr, &b, &mut x, &Identity, 50, &c);
        assert!(s4.converged && err(&x, &xe) < 1e-6, "gmres noprec {s4:?}");
        println!(
            "bicgstab id {} / ilu {} ; gmres ilu {} / id {}",
            s1.iterations, s2.iterations, s3.iterations, s4.iterations
        );
        assert!(s2.iterations < s1.iterations);
    }

    #[test]
    fn zero_rhs_and_exact_guess() {
        let (m, xe, b) = setup(5);
        let c = SolveControl::default();
        let mut x = xe.clone();
        let st = cg(&m, &b, &mut x, &Identity, &c);
        assert!(st.converged && st.iterations == 0);
        let mut x = vec![0.0; b.len()];
        let st = cg(&m, &vec![0.0; b.len()], &mut x, &Identity, &c);
        assert!(st.converged && st.iterations == 0);
    }

    #[test]
    fn solve_is_deterministic_across_threads() {
        let (m, _, b) = setup(70); // 4900 unknowns > one CHUNK
        let csr = m.to_csr();
        let c = SolveControl {
            tol_abs: 0.0,
            tol_rel: 1e-12,
            max_iter: 500,
        };
        let run = |threads: usize| {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
            pool.install(|| {
                let mut x = vec![0.0; b.len()];
                let st = cg(&csr, &b, &mut x, &Jacobi::new(&m.diag), &c);
                (x, st.iterations)
            })
        };
        let (x1, i1) = run(1);
        let (x4, i4) = run(4);
        assert_eq!(i1, i4);
        assert!(x1.iter().zip(&x4).all(|(a, b)| a.to_bits() == b.to_bits()));
    }
}
