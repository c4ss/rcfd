//! The sparse whirl solver (ADR 0427, F-506): the lowest whirl modes of `(K - omega^2 M + i omega G) phi = 0` by a
//! shift-invert Lanczos process on the state form of ADR 0411 without the dense cap.
//!
//! With `x = [u_t; u]`, `M* = blockdiag(M, K)` and `G* = [[G, K], [-K, 0]]` (ADR 0411 D2), the operator `W = G*^-1 M*` has the
//! eigenvalues `i / omega` and acts as `W [a; b] = [-b; K^-1 (M a + G b)]`: one solve with the sparse factor of `K` per
//! application (D1). `W` is skew in the inner product `<x, y>_* = x^T M* y`, and the Lanczos process on it in that product is
//! the process on `A^-1` of ADR 0411 in the coordinates `X = L^-T y` (D2). The process builds a skew tridiagonal matrix
//! (`W q_j = beta_j q_{j+1} - beta_{j-1} q_{j-1}`), full reorthogonalisation, made real symmetric by `diag(i^j)` and solved by
//! the QL routine of ADR 0411 (D3). Multiple frequencies are found by deflated restarts with locking (D5).

use super::{dot, tql};
use crate::error::{FemError, Result};
use rcfd_linalg::skyline::{PivotRule, SkylineLdlt, SkylineOptions, SkylineOrdering};
use rcfd_linalg::CsrMatrix;

/// Which eigensolver the whirl analyses use at speeds above zero (ADR 0427 D6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WhirlSolver {
    /// The dense solver of ADR 0411 (refuses more than [`super::MAX_WHIRL_UNKNOWNS`] reduced unknowns). The default.
    #[default]
    Dense,
    /// The sparse shift-invert Lanczos solver of ADR 0427 at every speed above zero.
    Sparse,
    /// The dense solver up to [`super::MAX_WHIRL_UNKNOWNS`] reduced unknowns, the sparse one above.
    Auto,
}

impl WhirlSolver {
    /// Whether a reduced problem of `n` unknowns goes to the sparse solver.
    pub(crate) fn sparse_for(self, n: usize) -> bool {
        match self {
            WhirlSolver::Dense => false,
            WhirlSolver::Sparse => true,
            WhirlSolver::Auto => n > super::MAX_WHIRL_UNKNOWNS,
        }
    }
}

/// Options of the sparse whirl solver (ADR 0427 D3, D5).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct LanczosOptions {
    /// A Ritz pair is converged when `beta_m |w_m| <= tolerance |mu|` (default 1e-12).
    pub tolerance: f64,
    /// The largest number of deflated runs (default 8).
    pub max_runs: usize,
}

impl Default for LanczosOptions {
    fn default() -> Self {
        LanczosOptions {
            tolerance: LANCZOS_TOLERANCE,
            max_runs: MAX_RUNS,
        }
    }
}

impl LanczosOptions {
    /// The same options with another convergence tolerance.
    pub fn tolerance(mut self, tolerance: f64) -> LanczosOptions {
        self.tolerance = tolerance;
        self
    }
    /// The same options with another largest number of runs.
    pub fn max_runs(mut self, runs: usize) -> LanczosOptions {
        self.max_runs = runs;
        self
    }
}

const LANCZOS_TOLERANCE: f64 = 1e-12;
const MAX_RUNS: usize = 8;
const BREAKDOWN: f64 = 1e-13;
const SEED: u64 = 0x5eed_0506;

/// The lowest whirl pairs of a sparse quadratic problem (ADR 0427).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SparseWhirl {
    /// The lowest positive frequencies, ascending, rad/s.
    pub omegas: Vec<f64>,
    /// The real parts of the modes, `phi^H M phi = 1`, the entry of largest modulus real and positive.
    pub re: Vec<Vec<f64>>,
    /// The imaginary parts.
    pub im: Vec<Vec<f64>>,
    /// The relative residual `||(K - omega^2 M + i omega G) phi|| / (||K phi|| + omega^2 ||M phi|| + omega ||G phi||)`.
    pub residuals: Vec<f64>,
    /// The number of Lanczos runs made.
    pub runs: usize,
    /// The number of Lanczos steps (applications of the operator) over all runs.
    pub steps: usize,
    /// Whether the stopping rule of ADR 0427 D5 was met (a run found nothing below the frequencies already locked).
    pub confirmed: bool,
}

fn matrix_refused(reason: String) -> FemError {
    FemError::InvalidValue { key: "matrix", reason }
}

fn check_sparse(a: &CsrMatrix, n: usize, what: &str) -> Result<()> {
    if a.n != n || a.ncols != n {
        return Err(matrix_refused(format!("{what} is {} x {}, not {n} x {n}", a.n, a.ncols)));
    }
    if a.val.iter().any(|v| !v.is_finite()) {
        return Err(matrix_refused(format!("{what} has a non-finite entry")));
    }
    Ok(())
}

fn factor(a: &CsrMatrix) -> Option<SkylineLdlt> {
    let opts = SkylineOptions {
        ordering: SkylineOrdering::Rcm,
        pivots: PivotRule::Positive,
        symmetry_tolerance: 1e-12,
    };
    SkylineLdlt::factor_with(a, &opts).ok()
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

/// `scale (a - a^T)`, applied without forming it.
struct Skew<'a> {
    a: &'a CsrMatrix,
    at: CsrMatrix,
    scale: f64,
}

impl Skew<'_> {
    fn apply(&self, x: &[f64]) -> Vec<f64> {
        let p = matvec(self.a, x);
        let q = matvec(&self.at, x);
        p.iter().zip(&q).map(|(u, v)| self.scale * (u - v)).collect()
    }
}

/// The state operator `W` and the inner product `M*` (ADR 0427 D1, D2).
struct Op<'a> {
    k: &'a CsrMatrix,
    m: &'a CsrMatrix,
    g: Skew<'a>,
    fk: SkylineLdlt,
    n: usize,
}

impl Op<'_> {
    /// `W [a; b] = [-b; K^-1 (M a + G b)]`.
    fn apply(&self, x: &[f64]) -> Vec<f64> {
        let n = self.n;
        let (a, b) = x.split_at(n);
        let ma = matvec(self.m, a);
        let gb = self.g.apply(b);
        let rhs: Vec<f64> = ma.iter().zip(&gb).map(|(p, q)| p + q).collect();
        let sol = self.fk.solve(&rhs);
        let mut out = Vec::with_capacity(2 * n);
        out.extend(b.iter().map(|v| -v));
        out.extend(sol);
        out
    }
    /// `M* x = [M a; K b]`.
    fn star(&self, x: &[f64]) -> Vec<f64> {
        let n = self.n;
        let (a, b) = x.split_at(n);
        let mut out = matvec(self.m, a);
        out.extend(matvec(self.k, b));
        out
    }
    fn norm(&self, x: &[f64]) -> f64 {
        dot(x, &self.star(x)).max(0.0).sqrt()
    }
}

/// Classical Gram-Schmidt of `v` against the `*`-orthonormal `sets`, applied twice.
fn orthogonalise(op: &Op, v: &mut [f64], sets: &[&[Vec<f64>]]) {
    for _ in 0..2 {
        let sv = op.star(v);
        let coeffs: Vec<Vec<f64>> = sets.iter().map(|s| s.iter().map(|q| dot(q, &sv)).collect()).collect();
        for (s, c) in sets.iter().zip(&coeffs) {
            for (q, &ci) in s.iter().zip(c) {
                if ci != 0.0 {
                    for (x, y) in v.iter_mut().zip(q) {
                        *x -= ci * y;
                    }
                }
            }
        }
    }
}

fn start_vector(len: usize, run: usize) -> Vec<f64> {
    let mut s = SEED.wrapping_add(run as u64);
    (0..len)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
        })
        .collect()
}

/// A converged Ritz pair: the frequency and the real and imaginary parts of the state vector.
struct Ritz {
    omega: f64,
    xr: Vec<f64>,
    xi: Vec<f64>,
}

/// One Lanczos run on the operator deflated by `z` (ADR 0427 D3): the converged pairs among the `count` lowest, and the
/// number of steps.
fn run(op: &Op, z: &[Vec<f64>], count: usize, tol: f64, index: usize) -> (Vec<Ritz>, usize) {
    let len = 2 * op.n;
    let avail = len - z.len();
    let max_steps = avail.min(20 * count + 100);
    let mut q0 = start_vector(len, index);
    orthogonalise(op, &mut q0, &[z]);
    let nq = op.norm(&q0);
    if !(nq > 0.0 && nq.is_finite()) {
        return (Vec::new(), 0);
    }
    q0.iter_mut().for_each(|x| *x /= nq);
    let mut q: Vec<Vec<f64>> = vec![q0];
    let mut betas: Vec<f64> = Vec::new();
    let mut maxb: f64 = 0.0;
    let mut steps = 0;
    let first_check = 2 * count + 2;
    loop {
        let j = q.len() - 1;
        let mut v = op.apply(&q[j]);
        steps += 1;
        orthogonalise(op, &mut v, &[z, &q]);
        let beta = op.norm(&v);
        maxb = maxb.max(beta);
        let m = q.len();
        let breakdown = !(beta > BREAKDOWN * maxb) || m >= avail;
        let last = m >= max_steps;
        let due = m >= first_check && (m - first_check) % 8 == 0;
        if breakdown || last || due {
            let res_beta = if breakdown { 0.0 } else { beta };
            let mut d = vec![0.0; m];
            let mut e: Vec<f64> = betas[..m - 1].to_vec();
            e.push(0.0);
            let Ok(w) = tql(&mut d, &mut e) else {
                return (Vec::new(), steps);
            };
            // The negative mu (a zero of an odd order, to rounding, is not a frequency).
            let dmax = d.iter().fold(0.0f64, |a, x| a.max(x.abs()));
            let mut idx: Vec<usize> = (0..m).filter(|&i| d[i] < -1e-10 * dmax).collect();
            idx.sort_by(|&i, &j| d[i].total_cmp(&d[j]).then(i.cmp(&j)));
            idx.truncate(count);
            let conv: Vec<bool> = idx
                .iter()
                .map(|&i| res_beta * w[i * m + m - 1].abs() <= tol * d[i].abs())
                .collect();
            let all = idx.len() == count && conv.iter().all(|&c| c);
            if breakdown || last || all {
                let mut out = Vec::new();
                for (&i, &c) in idx.iter().zip(&conv) {
                    if !c {
                        continue;
                    }
                    let wi = &w[i * m..i * m + m];
                    let mut xr = vec![0.0; len];
                    let mut xi = vec![0.0; len];
                    for (jj, (&wj, qj)) in wi.iter().zip(&q).enumerate() {
                        let (target, sign) = match jj % 4 {
                            0 => (&mut xr, 1.0),
                            1 => (&mut xi, 1.0),
                            2 => (&mut xr, -1.0),
                            _ => (&mut xi, -1.0),
                        };
                        let f = sign * wj;
                        for (x, y) in target.iter_mut().zip(qj) {
                            *x += f * y;
                        }
                    }
                    out.push(Ritz {
                        omega: -1.0 / d[i],
                        xr,
                        xi,
                    });
                }
                return (out, steps);
            }
        }
        debug_assert!(!breakdown, "a breakdown always ends the run");
        v.iter_mut().for_each(|x| *x /= beta);
        betas.push(beta);
        q.push(v);
    }
}

/// The core of the solver: the lowest `count` frequencies with their displacement blocks (unnormalised), runs, steps and
/// the confirmation flag.
#[allow(clippy::type_complexity)]
fn lanczos(op: &Op, count: usize, opts: &LanczosOptions) -> (Vec<(f64, Vec<f64>, Vec<f64>)>, usize, usize, bool) {
    let n = op.n;
    let mut z: Vec<Vec<f64>> = Vec::new();
    let mut locked: Vec<(f64, Vec<f64>, Vec<f64>)> = Vec::new();
    let mut steps = 0;
    let mut runs = 0;
    let mut confirmed = false;
    for r in 0..opts.max_runs {
        if z.len() >= 2 * n {
            confirmed = true;
            break;
        }
        let threshold = if locked.len() >= count {
            let mut om: Vec<f64> = locked.iter().map(|p| p.0).collect();
            om.sort_by(f64::total_cmp);
            om[count - 1]
        } else {
            f64::INFINITY
        };
        let (found, s) = run(op, &z, count, opts.tolerance, r);
        runs += 1;
        steps += s;
        let below = found.iter().any(|p| p.omega < threshold);
        for p in found {
            for x in [&p.xr, &p.xi] {
                let mut v = x.clone();
                orthogonalise(op, &mut v, &[&z]);
                let nv = op.norm(&v);
                if nv > 0.0 && nv.is_finite() && z.len() < 2 * n {
                    v.iter_mut().for_each(|t| *t /= nv);
                    z.push(v);
                }
            }
            locked.push((p.omega, p.xr[n..].to_vec(), p.xi[n..].to_vec()));
        }
        if r >= 1 && !below && locked.len() >= count {
            confirmed = true;
            break;
        }
    }
    if !confirmed && z.len() >= 2 * n {
        confirmed = true;
    }
    locked.sort_by(|a, b| a.0.total_cmp(&b.0));
    locked.truncate(count);
    (locked, runs, steps, confirmed)
}

/// Normalises `phi = re + i im` to `phi^H M phi = 1` with the entry of largest modulus (the first on a tie) real and
/// positive, as the dense solver does (ADR 0411 D2 step 5).
fn normalise(m: &CsrMatrix, re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let nm = dot(re, &matvec(m, re)) + dot(im, &matvec(m, im));
    let sc = 1.0 / nm.sqrt();
    let mut imax = 0;
    let mut best = -1.0;
    for i in 0..n {
        let v = re[i] * re[i] + im[i] * im[i];
        if v > best {
            best = v;
            imax = i;
        }
    }
    let theta = im[imax].atan2(re[imax]);
    let (cs, sn) = (theta.cos(), theta.sin());
    for i in 0..n {
        let (r, s) = (re[i], im[i]);
        re[i] = sc * (r * cs + s * sn);
        im[i] = sc * (s * cs - r * sn);
    }
    im[imax] = 0.0;
}

fn residual(op: &Op, omega: f64, re: &[f64], im: &[f64]) -> f64 {
    let (kr, ki) = (matvec(op.k, re), matvec(op.k, im));
    let (mr, mi) = (matvec(op.m, re), matvec(op.m, im));
    let (gr, gi) = (op.g.apply(re), op.g.apply(im));
    let w2 = omega * omega;
    let mut num = 0.0;
    for i in 0..re.len() {
        let a = kr[i] - w2 * mr[i] - omega * gi[i];
        let b = ki[i] - w2 * mi[i] + omega * gr[i];
        num += a * a + b * b;
    }
    let n2 = |x: &[f64], y: &[f64]| (dot(x, x) + dot(y, y)).sqrt();
    num.sqrt() / (n2(&kr, &ki) + w2 * n2(&mr, &mi) + omega * n2(&gr, &gi))
}

/// The solve on validated matrices; `g` is applied as `scale (g - g^T)`.
fn solve(k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, scale: f64, count: usize, opts: &LanczosOptions) -> Result<SparseWhirl> {
    let n = k.n;
    let fk = factor(k).ok_or_else(|| FemError::InvalidValue {
        key: "stiffness",
        reason: "the stiffness of the whirl problem is not positive definite (a loss of stiffness)".to_string(),
    })?;
    // The factor of M: the test that M* is an inner product (ADR 0427 D2, D4); it is not applied.
    factor(m).ok_or_else(|| FemError::InvalidValue {
        key: "mass",
        reason: "the mass of the whirl problem is not positive definite".to_string(),
    })?;
    let op = Op {
        k,
        m,
        g: Skew {
            a: g,
            at: g.transpose(),
            scale,
        },
        fk,
        n,
    };
    let (pairs, runs, steps, confirmed) = lanczos(&op, count, opts);
    if pairs.len() < count {
        return Err(FemError::Solver {
            solver: "sparse_whirl",
            reason: format!("{} of {count} whirl modes converged in {runs} runs ({steps} steps)", pairs.len()),
        });
    }
    if pairs.iter().any(|p| !p.0.is_finite()) {
        return Err(FemError::Solver {
            solver: "sparse_whirl",
            reason: "a non-finite frequency".to_string(),
        });
    }
    let mut r = SparseWhirl {
        omegas: Vec::with_capacity(count),
        re: Vec::with_capacity(count),
        im: Vec::with_capacity(count),
        residuals: Vec::with_capacity(count),
        runs,
        steps,
        confirmed,
    };
    for (omega, mut re, mut im) in pairs {
        normalise(m, &mut re, &mut im);
        r.residuals.push(residual(&op, omega, &re, &im));
        r.omegas.push(omega);
        r.re.push(re);
        r.im.push(im);
    }
    Ok(r)
}

/// The lowest `count` whirl pairs of `(K - omega^2 M + i omega G) phi = 0` for sparse `n x n` matrices by the shift-invert
/// Lanczos solver of ADR 0427 with the default [`LanczosOptions`]: `k` and `m` symmetric positive definite, `g` skew (its
/// symmetric part is discarded). Refuses matrices of unequal or non-square size or with non-finite entries (`"matrix"`),
/// `count` of 0 or above `n` (`"modes"`), a `k` (`"stiffness"`) or `m` (`"mass"`) that is not positive definite, and a
/// solve whose stopping rule was not met (`FemError::Solver`).
pub fn gyroscopic_eigen_sparse(k: &CsrMatrix, m: &CsrMatrix, g: &CsrMatrix, count: usize) -> Result<SparseWhirl> {
    let r = gyroscopic_eigen_sparse_with(k, m, g, count, &LanczosOptions::default())?;
    if !r.confirmed {
        return Err(unconfirmed(&r));
    }
    Ok(r)
}

fn unconfirmed(r: &SparseWhirl) -> FemError {
    FemError::Solver {
        solver: "sparse_whirl",
        reason: format!(
            "the lowest frequencies were not confirmed by a deflated run in {} runs ({} steps)",
            r.runs, r.steps
        ),
    }
}

/// As [`gyroscopic_eigen_sparse`] with given options; an unconfirmed result is returned with `confirmed == false`.
pub fn gyroscopic_eigen_sparse_with(
    k: &CsrMatrix,
    m: &CsrMatrix,
    g: &CsrMatrix,
    count: usize,
    opts: &LanczosOptions,
) -> Result<SparseWhirl> {
    let n = k.n;
    check_sparse(k, n, "K")?;
    check_sparse(m, n, "M")?;
    check_sparse(g, n, "G")?;
    if count == 0 || count > n {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!("{count} modes requested of a whirl problem of {n} unknowns (at least 1, at most {n})"),
        });
    }
    if !(opts.tolerance > 0.0 && opts.tolerance.is_finite()) || opts.max_runs == 0 {
        return Err(FemError::InvalidValue {
            key: "options",
            reason: format!(
                "tolerance {} and {} runs (a positive tolerance and at least one run)",
                opts.tolerance, opts.max_runs
            ),
        });
    }
    solve(k, m, g, 0.5, count, opts)
}

/// The sparse branch of the whirl analyses (ADR 0427 D6): the reduced `K_eff`, `M` and the Coriolis matrix per unit speed
/// `C` at the speed `omega` (`G = 2 omega C`, applied as `omega (C - C^T)`; `omega = 0` for `G = 0`), the lowest `count`
/// pairs with their whirl numbers `kappa = 2 r^T C s / (r^T M r + s^T M s)` as `(omega, re, im, residual, kappa)`.
#[allow(clippy::type_complexity)]
pub(crate) fn solve_reduced(
    kr: &CsrMatrix,
    mr: &CsrMatrix,
    cr: &CsrMatrix,
    omega: f64,
    count: usize,
) -> Result<Vec<(f64, Vec<f64>, Vec<f64>, f64, f64)>> {
    let r = solve(kr, mr, cr, omega, count, &LanczosOptions::default())?;
    if !r.confirmed {
        return Err(unconfirmed(&r));
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..r.omegas.len() {
        let (re, im) = (&r.re[i], &r.im[i]);
        let num = 2.0 * dot(re, &matvec(cr, im));
        let den = dot(re, &matvec(mr, re)) + dot(im, &matvec(mr, im));
        out.push((r.omegas[i], re.clone(), im.clone(), r.residuals[i], num / den));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(s: &mut u64) -> f64 {
        *s = s.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        (*s >> 11) as f64 / (1u64 << 53) as f64 - 0.5
    }

    fn csr(a: &[f64], n: usize) -> CsrMatrix {
        let t = (0..n * n).filter(|&p| a[p] != 0.0).map(|p| (p / n, p % n, a[p])).collect();
        CsrMatrix::from_triplets(n, t)
    }

    /// The solver of the development against the dense solver of ADR 0411 on random banded problems (not a registered
    /// check of ADR 0427).
    #[test]
    fn development_against_dense() {
        for (n, count, seed) in [(3, 2, 5u64), (7, 4, 11), (16, 5, 23), (40, 6, 31)] {
            let mut s = seed;
            let mut k = vec![0.0; n * n];
            let mut m = vec![0.0; n * n];
            let mut g = vec![0.0; n * n];
            for i in 0..n {
                k[i * n + i] = 2.0e4 + 1.0e3 * lcg(&mut s);
                m[i * n + i] = 1.0 + 0.2 * lcg(&mut s);
                if i + 1 < n {
                    let b = -9.0e3 + 1.0e3 * lcg(&mut s);
                    k[i * n + i + 1] = b;
                    k[(i + 1) * n + i] = b;
                    let c = 0.1 * lcg(&mut s);
                    m[i * n + i + 1] = c;
                    m[(i + 1) * n + i] = c;
                    let v = 20.0 * lcg(&mut s);
                    g[i * n + i + 1] = v;
                    g[(i + 1) * n + i] = -v;
                }
            }
            let d = super::super::gyroscopic_eigen_dense(&k, &m, &g, n).unwrap();
            let r = gyroscopic_eigen_sparse(&csr(&k, n), &csr(&m, n), &csr(&g, n), count).unwrap();
            for i in 0..count {
                let e = (r.omegas[i] / d.omegas[i] - 1.0).abs();
                assert!(e < 1e-10, "n {n} mode {i}: {} against {} ({e:e})", r.omegas[i], d.omegas[i]);
                assert!(r.residuals[i] < 1e-10, "n {n} mode {i}: residual {:e}", r.residuals[i]);
            }
        }
    }
}
