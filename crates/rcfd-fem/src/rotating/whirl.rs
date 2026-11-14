//! Coriolis coupling of a rotating part: the gyroscopic matrix, whirl modes, forward and backward whirl, the Campbell table
//! with directions, on the whole part and on a cyclic sector (ADR 0411, F-499).
//!
//! The kinetic energy of ADR 0403 D1 has the term `Omega u_t^T C u`, `C[(a, i), (b, j)] = int rho N_a N_b [a x]_ij`, skew; the
//! free vibration about the pre-stressed state is `M u_tt + G u_t + K_eff u = 0`, `G = 2 Omega C` ([`coriolis_matrix`]),
//! and with `u = Re(phi e^{i omega t})`: `(K_eff - omega^2 M + i omega G) phi = 0` (D1). For `K_eff` positive definite the
//! state form `M* x_t + G* x = 0`, `x = [u_t; u]`, `M* = blockdiag(M, K_eff)` symmetric positive definite, `G* = [[G, K_eff],
//! [-K_eff, 0]]` skew (Meirovitch's symmetric linearisation), is reduced by the Cholesky factors to a real skew matrix `A`
//! whose eigenvalues are `+/- i omega`; `A` is brought to skew tridiagonal form by Householder reflections, made real
//! symmetric tridiagonal by the diagonal similarity `diag(i^j)` and solved by the implicit QL algorithm (D2,
//! [`gyroscopic_eigen_dense`]). The whirl number `kappa = Im(phi^H C phi) / phi^H M phi` gives the direction: forward
//! (`kappa > 0`, the orbit turns in the sense of the rotation, the frequency falls as `omega_0 - kappa Omega`) or backward
//! (D3). The Campbell table of F-396 carries the whirl branches with their directions (D4); on a sector the gyroscopic
//! matrix of index `k` is `T_k^T blockdiag(G_s, G_s) T_k` (D5).

use super::{
    check_harmonics, check_speeds, cluster_spaces, subspace_mac, unit_axis, CampbellTable, Crossing, HarmonicLoss,
    LossOfStiffness, ModeTrack, Path, RotatingPart, RotatingSector, SpeedModes,
};
use crate::assemble::assemble_with;
use crate::constraint::reduce;
use crate::element::{element_for, ElementMatrix};
use crate::error::{FemError, Result};
use crate::mass::element_mass;
use crate::model::SolidModel;
use crate::mpc::Transformation;
use rcfd_core::vec3::Vec3;
use rcfd_linalg::eigen::{count_below, EigenError};
use rcfd_linalg::skyline::SkylineOrdering;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::fmt::Write as _;

mod state;
pub use state::{
    gyroscopic_state_eigen_dense, DenseStateWhirl, Stability, StateWhirl, StateWhirlMode, WhirlStability,
    STABILITY_TOLERANCE,
};
pub use state::{gyroscopic_state_eigen_sparse, SparseStateEigen, SparseStateOptions, SparseStateWhirl};
pub use state::{CyclicWhirlStability, HarmonicStateWhirl};

mod sparse;
pub use sparse::{gyroscopic_eigen_sparse, gyroscopic_eigen_sparse_with, LanczosOptions, SparseWhirl, WhirlSolver};

/// `|kappa|` at or below which a mode has no whirl direction (ADR 0411 D3).
pub const WHIRL_TOLERANCE: f64 = 1e-6;

/// The largest reduced problem the dense whirl solver takes (ADR 0411 D2); larger ones are refused (key `"unknowns"`).
pub const MAX_WHIRL_UNKNOWNS: usize = 1500;

/// Iterations of the implicit QL algorithm per eigenvalue before the solve is refused.
const QL_ITERATIONS: usize = 30;

/// The Coriolis matrix per unit speed `C[(a, i), (b, j)] = 1/2 (m_ab + m_ba) [a x]_ij` of a rotation about `axis` (ADR 0411
/// D1), `m_ab` the scalar consistent mass of each element's mass rule, on the stiffness pattern; `G = 2 Omega C`. Skew bit
/// for bit; bit-identical for any thread count. A zero or non-finite axis is refused (key `"axis"`).
pub fn coriolis_matrix(model: &SolidModel, axis: Vec3, threads: Option<usize>) -> Result<CsrMatrix> {
    let a = unit_axis(axis)?;
    let x = [[0.0, -a[2], a[1]], [a[2], 0.0, -a[0]], [-a[1], a[0], 0.0]];
    assemble_with(model, threads, |ctx| {
        let m = element_mass(element_for(ctx.kind), ctx.coords, ctx.material.density())?;
        let size = m.size();
        let n = size / 3;
        let mut out = ElementMatrix::zeros(size);
        let src = m.values();
        let v = out.values_mut();
        for ia in 0..n {
            for ib in 0..n {
                let mab = 0.5 * (src[(3 * ia) * size + 3 * ib] + src[(3 * ib) * size + 3 * ia]);
                for (i, row) in x.iter().enumerate() {
                    for (j, &xij) in row.iter().enumerate() {
                        v[(3 * ia + i) * size + 3 * ib + j] = mab * xij;
                    }
                }
            }
        }
        Ok(out)
    })
}

/// The direction of a whirl mode (ADR 0411 D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whirl {
    /// `kappa > WHIRL_TOLERANCE`: the orbit turns in the sense of the rotation about the axis.
    Forward,
    /// `kappa < -WHIRL_TOLERANCE`: against it.
    Backward,
    /// `|kappa| <= WHIRL_TOLERANCE` (no orbit in the plane of rotation, or a mode at rest).
    Neutral,
}

impl Whirl {
    /// The direction of the whirl number `kappa`.
    pub fn of(kappa: f64) -> Whirl {
        if kappa > WHIRL_TOLERANCE {
            Whirl::Forward
        } else if kappa < -WHIRL_TOLERANCE {
            Whirl::Backward
        } else {
            Whirl::Neutral
        }
    }
    /// `"forward"`, `"backward"` or `"neutral"`.
    pub fn name(self) -> &'static str {
        match self {
            Whirl::Forward => "forward",
            Whirl::Backward => "backward",
            Whirl::Neutral => "neutral",
        }
    }
}

/// One whirl mode `u = Re((re + i im) e^{i omega t})` (ADR 0411 D2, D3).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WhirlMode {
    /// The frequency in the rotating frame, rad/s, positive.
    pub omega: f64,
    /// `omega^2`.
    pub eigenvalue: f64,
    /// `omega / (2 pi)`, Hz.
    pub frequency: f64,
    /// The real part of the mode (the model's equations; on a sector pair the cosine then the sine part), `phi^H M phi = 1`,
    /// the entry of largest modulus real and positive.
    pub re: Vec<f64>,
    /// The imaginary part.
    pub im: Vec<f64>,
    /// The whirl number `kappa = Im(phi^H C phi) / phi^H M phi`, in `[-1, 1]` (0 at rest).
    pub whirl: f64,
    /// The direction of `kappa`.
    pub direction: Whirl,
    /// On a sector pair the wave sense `tau = 2 Im(a^H M b) / (a^H M a + b^H M b)` (`+1` a wave travelling in the sense of the
    /// rotation, `-1` against, 0 standing); `None` on the whole part and on single indices.
    pub wave: Option<f64>,
    /// `||(K_eff - omega^2 M + i omega G) phi|| / (||K_eff phi|| + omega^2 ||M phi|| + omega ||G phi||)` on the reduced
    /// problem (the modal residual at rest).
    pub residual: f64,
}

/// The whirl modes at one speed.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WhirlModes {
    /// The lowest modes, ascending.
    pub modes: Vec<WhirlMode>,
    /// Every positive frequency of the reduced problem, ascending, rad/s (at rest, and with the sparse solver of ADR 0427,
    /// only those of the returned modes).
    pub frequencies_all: Vec<f64>,
    /// The number of reduced unknowns.
    pub unknowns: usize,
}

/// The whirl analysis at one speed of a whole part.
#[derive(Debug, Clone, PartialEq)]
pub enum WhirlSpeedModes {
    /// The whirl modes.
    Modes(WhirlModes),
    /// `K_eff` is not positive definite at this speed (ADR 0403 D3): no frequency is computed.
    LossOfStiffness(LossOfStiffness),
}

impl WhirlSpeedModes {
    /// The modes, if the speed has them.
    pub fn modes(&self) -> Option<&WhirlModes> {
        match self {
            WhirlSpeedModes::Modes(r) => Some(r),
            WhirlSpeedModes::LossOfStiffness(_) => None,
        }
    }
    /// The loss of stiffness, if the speed has one.
    pub fn loss(&self) -> Option<&LossOfStiffness> {
        match self {
            WhirlSpeedModes::Modes(_) => None,
            WhirlSpeedModes::LossOfStiffness(l) => Some(l),
        }
    }
}

/// The whirl analysis of one harmonic index of a sector at one speed (ADR 0411 D5).
#[derive(Debug, Clone, PartialEq)]
pub enum HarmonicWhirlSpeedModes {
    /// The whirl modes of the index (`modes x multiplicity` of them).
    Modes {
        /// The harmonic index.
        harmonic: usize,
        /// 1 for `k = 0` and `2 k = N`, 2 otherwise.
        multiplicity: usize,
        /// The modes.
        modes: WhirlModes,
    },
    /// The reduced `K_eff` of the index is not positive definite at this speed.
    LossOfStiffness(HarmonicLoss),
}

impl HarmonicWhirlSpeedModes {
    /// The modes, if the index has them at this speed.
    pub fn modes(&self) -> Option<&WhirlModes> {
        match self {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => Some(modes),
            HarmonicWhirlSpeedModes::LossOfStiffness(_) => None,
        }
    }
    /// The loss of stiffness, if the index has one at this speed.
    pub fn loss(&self) -> Option<&HarmonicLoss> {
        match self {
            HarmonicWhirlSpeedModes::Modes { .. } => None,
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => Some(l),
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// D2: the dense solver.

/// All whirl pairs of a dense quadratic problem (ADR 0411 D2).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DenseWhirl {
    /// The positive frequencies, ascending, rad/s.
    pub omegas: Vec<f64>,
    /// The real parts of the modes, `phi^H M phi = 1`.
    pub re: Vec<Vec<f64>>,
    /// The imaginary parts.
    pub im: Vec<Vec<f64>>,
    /// The relative residual of each pair.
    pub residuals: Vec<f64>,
}

/// The lower Cholesky factor of a dense symmetric row-major matrix, `None` if it is not positive definite.
fn cholesky(a: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let mut s = a[j * n + j];
        for p in 0..j {
            s -= l[j * n + p] * l[j * n + p];
        }
        if !(s.is_finite() && s > 0.0) {
            return None;
        }
        let d = s.sqrt();
        l[j * n + j] = d;
        for i in j + 1..n {
            let mut t = a[i * n + j];
            for p in 0..j {
                t -= l[i * n + p] * l[j * n + p];
            }
            l[i * n + j] = t / d;
        }
    }
    Some(l)
}

/// `L X = B` in place, `B` row-major `n x c`.
fn lower_solve(l: &[f64], n: usize, b: &mut [f64], c: usize) {
    for i in 0..n {
        let (head, tail) = b.split_at_mut(i * c);
        let ri = &mut tail[..c];
        for p in 0..i {
            let f = l[i * n + p];
            if f != 0.0 {
                for (x, y) in ri.iter_mut().zip(&head[p * c..p * c + c]) {
                    *x -= f * y;
                }
            }
        }
        let d = l[i * n + i];
        for x in ri.iter_mut() {
            *x /= d;
        }
    }
}

/// `L^T x = b` in place for one vector.
fn upper_t_solve(l: &[f64], n: usize, b: &mut [f64]) {
    for i in (0..n).rev() {
        let mut s = b[i];
        for p in i + 1..n {
            s -= l[p * n + i] * b[p];
        }
        b[i] = s / l[i * n + i];
    }
}

fn transpose(a: &[f64], r: usize, c: usize) -> Vec<f64> {
    let mut t = vec![0.0; r * c];
    for i in 0..r {
        for j in 0..c {
            t[j * r + i] = a[i * c + j];
        }
    }
    t
}

fn dense_matvec(a: &[f64], n: usize, x: &[f64]) -> Vec<f64> {
    (0..n)
        .map(|i| a[i * n..i * n + n].iter().zip(x).map(|(p, q)| p * q).sum())
        .collect()
}

fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(p, q)| p * q).sum()
}

fn norm2(x: &[f64]) -> f64 {
    dot(x, x)
}

/// Householder reduction of the real skew row-major `a` (`m x m`, consumed) to skew tridiagonal form `a = Q T Q^T`: the
/// sub-diagonal `e_j = T_{j+1, j}` and `Q` row-major (D2 step 1).
fn skew_tridiagonal(mut a: Vec<f64>, m: usize) -> (Vec<f64>, Vec<f64>) {
    let mut q = vec![0.0; m * m];
    for i in 0..m {
        q[i * m + i] = 1.0;
    }
    let mut v = vec![0.0; m];
    let mut p = vec![0.0; m];
    let mut qv = vec![0.0; m];
    for k in 0..m.saturating_sub(2) {
        let alpha = (k + 1..m).map(|i| a[i * m + k] * a[i * m + k]).sum::<f64>().sqrt();
        if alpha == 0.0 {
            continue;
        }
        let s = if a[(k + 1) * m + k] >= 0.0 { 1.0 } else { -1.0 };
        v.iter_mut().for_each(|x| *x = 0.0);
        for i in k + 1..m {
            v[i] = a[i * m + k];
        }
        v[k + 1] += s * alpha;
        let vn2: f64 = v[k + 1..].iter().map(|x| x * x).sum();
        if vn2 == 0.0 {
            continue;
        }
        let beta = 2.0 / vn2;
        // p = A v (rows below k are zero beyond the band, so rows k.. suffice).
        for i in 0..m {
            p[i] = if i < k {
                0.0
            } else {
                (k + 1..m).map(|j| a[i * m + j] * v[j]).sum()
            };
        }
        // P A P = A + beta (v p^T - p v^T), on rows and columns k..
        for i in k..m {
            let (vi, pi) = (v[i], p[i]);
            if vi == 0.0 && pi == 0.0 {
                continue;
            }
            let row = &mut a[i * m..i * m + m];
            for j in k..m {
                row[j] += beta * (vi * p[j] - pi * v[j]);
            }
        }
        // Q <- Q P.
        for i in 0..m {
            qv[i] = (k + 1..m).map(|j| q[i * m + j] * v[j]).sum();
        }
        for i in 0..m {
            let f = beta * qv[i];
            if f == 0.0 {
                continue;
            }
            for j in k + 1..m {
                q[i * m + j] -= f * v[j];
            }
        }
    }
    let e = (0..m.saturating_sub(1)).map(|j| a[(j + 1) * m + j]).collect();
    (e, q)
}

/// The implicit QL algorithm with shifts on the symmetric tridiagonal matrix of diagonal `d` and off-diagonal `e` (`e[i]`
/// couples `i` and `i + 1`; length `m`, the last entry ignored): eigenvalues in `d`, eigenvector `i` in row `i` of the
/// returned row-major matrix (D2 step 3).
fn tql(d: &mut [f64], e: &mut [f64]) -> Result<Vec<f64>> {
    let m = d.len();
    let mut z = vec![0.0; m * m];
    for i in 0..m {
        z[i * m + i] = 1.0;
    }
    if m == 0 {
        return Ok(z);
    }
    e[m - 1] = 0.0;
    for l in 0..m {
        let mut iter = 0;
        loop {
            let mut mi = l;
            while mi + 1 < m {
                let dd = d[mi].abs() + d[mi + 1].abs();
                if e[mi].abs() <= f64::EPSILON * dd {
                    break;
                }
                mi += 1;
            }
            if mi == l {
                break;
            }
            iter += 1;
            if iter > QL_ITERATIONS {
                return Err(FemError::Solver {
                    solver: "gyroscopic_eigen",
                    reason: format!("the QL iteration did not converge for eigenvalue {l} in {QL_ITERATIONS} steps"),
                });
            }
            let mut g = (d[l + 1] - d[l]) / (2.0 * e[l]);
            let mut r = g.hypot(1.0);
            g = d[mi] - d[l] + e[l] / (g + r.copysign(g));
            let (mut s, mut c, mut p) = (1.0, 1.0, 0.0);
            let mut i = mi;
            let mut early = false;
            while i > l {
                i -= 1;
                let f = s * e[i];
                let b = c * e[i];
                r = f.hypot(g);
                e[i + 1] = r;
                if r == 0.0 {
                    d[i + 1] -= p;
                    e[mi] = 0.0;
                    early = true;
                    break;
                }
                s = f / r;
                c = g / r;
                g = d[i + 1] - p;
                r = (d[i] - g) * s + 2.0 * c * b;
                p = s * r;
                d[i + 1] = g + p;
                g = c * r - b;
                let (lo, hi) = z.split_at_mut((i + 1) * m);
                let zi = &mut lo[i * m..i * m + m];
                let zj = &mut hi[..m];
                for (x, y) in zi.iter_mut().zip(zj.iter_mut()) {
                    let f2 = *y;
                    *y = s * *x + c * f2;
                    *x = c * *x - s * f2;
                }
            }
            if early {
                continue;
            }
            d[l] -= p;
            e[l] = g;
            e[mi] = 0.0;
        }
    }
    Ok(z)
}

/// The output of the dense solve: every positive frequency, and the lowest `wanted` pairs with their residuals.
struct DenseOut {
    all: Vec<f64>,
    pairs: Vec<(f64, Vec<f64>, Vec<f64>, f64)>,
}

/// D2 on dense row-major `k` (positive definite), `m` (positive definite) and `g` (skew), all `n x n`.
fn solve_dense(k: &[f64], m: &[f64], g: &[f64], n: usize, wanted: usize) -> Result<DenseOut> {
    let lm = cholesky(m, n).ok_or_else(|| FemError::InvalidValue {
        key: "mass",
        reason: "the mass of the whirl problem is not positive definite".to_string(),
    })?;
    let lk = cholesky(k, n).ok_or_else(|| FemError::InvalidValue {
        key: "stiffness",
        reason: "the stiffness of the whirl problem is not positive definite (a loss of stiffness)".to_string(),
    })?;
    // G^ = L_m^-1 G L_m^-T = Y^T with Y = L_m^-1 (L_m^-1 G)^T, made exactly skew.
    let mut x = g.to_vec();
    lower_solve(&lm, n, &mut x, n);
    let mut y = transpose(&x, n, n);
    lower_solve(&lm, n, &mut y, n);
    // B = L_m^-1 L_k.
    let mut b = lk.clone();
    lower_solve(&lm, n, &mut b, n);
    let mm = 2 * n;
    let mut a = vec![0.0; mm * mm];
    for i in 0..n {
        for j in 0..n {
            a[i * mm + j] = 0.5 * (y[j * n + i] - y[i * n + j]);
            a[i * mm + n + j] = b[i * n + j];
            a[(n + j) * mm + i] = -b[i * n + j];
        }
    }
    let (e, q) = skew_tridiagonal(a, mm);
    let mut d = vec![0.0; mm];
    let mut ee = e;
    ee.push(0.0);
    let z = tql(&mut d, &mut ee)?;
    let mut idx: Vec<usize> = (0..mm).filter(|&i| d[i] > 0.0).collect();
    idx.sort_by(|&i, &j| d[i].total_cmp(&d[j]).then(i.cmp(&j)));
    if idx.iter().any(|&i| !d[i].is_finite()) {
        return Err(FemError::Solver {
            solver: "gyroscopic_eigen",
            reason: "a non-finite frequency".to_string(),
        });
    }
    if idx.len() < wanted {
        return Err(FemError::InvalidValue {
            key: "modes",
            reason: format!(
                "{wanted} modes requested, the whirl problem has {} positive frequencies",
                idx.len()
            ),
        });
    }
    let all: Vec<f64> = idx.iter().map(|&i| d[i]).collect();
    let mut pairs = Vec::with_capacity(wanted);
    for &j in idx.iter().take(wanted) {
        let w = &z[j * mm..j * mm + mm];
        // y = Q D w, D = diag(i^k): the real coefficients from k = 0, 2 (mod 4), the imaginary from k = 1, 3.
        let mut rc = vec![0.0; mm];
        let mut ic = vec![0.0; mm];
        for (kk, &wk) in w.iter().enumerate() {
            match kk % 4 {
                0 => rc[kk] = wk,
                1 => ic[kk] = wk,
                2 => rc[kk] = -wk,
                _ => ic[kk] = -wk,
            }
        }
        let mut re: Vec<f64> = (n..mm).map(|r| dot(&q[r * mm..r * mm + mm], &rc)).collect();
        let mut im: Vec<f64> = (n..mm).map(|r| dot(&q[r * mm..r * mm + mm], &ic)).collect();
        upper_t_solve(&lk, n, &mut re);
        upper_t_solve(&lk, n, &mut im);
        // phi^H M phi = 1, the entry of largest modulus real and positive.
        let nm = dot(&re, &dense_matvec(m, n, &re)) + dot(&im, &dense_matvec(m, n, &im));
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
        let omega = d[j];
        let residual = quadratic_residual(k, m, g, n, omega, &re, &im);
        pairs.push((omega, re, im, residual));
    }
    Ok(DenseOut { all, pairs })
}

/// `||(K - omega^2 M + i omega G) phi|| / (||K phi|| + omega^2 ||M phi|| + omega ||G phi||)`.
fn quadratic_residual(k: &[f64], m: &[f64], g: &[f64], n: usize, omega: f64, re: &[f64], im: &[f64]) -> f64 {
    let (kr, ki) = (dense_matvec(k, n, re), dense_matvec(k, n, im));
    let (mr, mi) = (dense_matvec(m, n, re), dense_matvec(m, n, im));
    let (gr, gi) = (dense_matvec(g, n, re), dense_matvec(g, n, im));
    let w2 = omega * omega;
    let mut num = 0.0;
    for i in 0..n {
        let a = kr[i] - w2 * mr[i] - omega * gi[i];
        let b = ki[i] - w2 * mi[i] + omega * gr[i];
        num += a * a + b * b;
    }
    let den = (norm2(&kr) + norm2(&ki)).sqrt()
        + w2 * (norm2(&mr) + norm2(&mi)).sqrt()
        + omega * (norm2(&gr) + norm2(&gi)).sqrt();
    num.sqrt() / den
}

fn check_dense(a: &[f64], n: usize, what: &'static str) -> Result<()> {
    if a.len() != n * n {
        return Err(FemError::InvalidValue {
            key: "matrix",
            reason: format!("{what} has {} entries, not {n} x {n}", a.len()),
        });
    }
    if a.iter().any(|v| !v.is_finite()) {
        return Err(FemError::InvalidValue {
            key: "matrix",
            reason: format!("{what} has a non-finite entry"),
        });
    }
    Ok(())
}

/// Every whirl pair of `(K - omega^2 M + i omega G) phi = 0` for dense row-major `n x n` matrices (ADR 0411 D2): `k` and `m`
/// symmetric positive definite, `g` skew (its symmetric part is discarded). The `n` positive frequencies ascending with
/// their modes. Refuses matrices of the wrong length or with non-finite entries (`"matrix"`), a `k` (`"stiffness"`) or `m`
/// (`"mass"`) that is not positive definite, and more than [`MAX_WHIRL_UNKNOWNS`] unknowns (`"unknowns"`).
pub fn gyroscopic_eigen_dense(k: &[f64], m: &[f64], g: &[f64], n: usize) -> Result<DenseWhirl> {
    check_dense(k, n, "K")?;
    check_dense(m, n, "M")?;
    check_dense(g, n, "G")?;
    check_size(n)?;
    let out = solve_dense(k, m, g, n, n)?;
    let mut r = DenseWhirl {
        omegas: Vec::with_capacity(n),
        re: Vec::with_capacity(n),
        im: Vec::with_capacity(n),
        residuals: Vec::with_capacity(n),
    };
    for (w, re, im, res) in out.pairs {
        r.omegas.push(w);
        r.re.push(re);
        r.im.push(im);
        r.residuals.push(res);
    }
    Ok(r)
}

fn check_size(n: usize) -> Result<()> {
    if n > MAX_WHIRL_UNKNOWNS {
        return Err(FemError::InvalidValue {
            key: "unknowns",
            reason: format!(
                "{n} reduced unknowns: the dense whirl solver takes at most {MAX_WHIRL_UNKNOWNS} (ADR 0411 D2)"
            ),
        });
    }
    Ok(())
}

fn dense_of(a: &CsrMatrix) -> Vec<f64> {
    let n = a.n;
    let mut d = vec![0.0; n * a.ncols];
    for r in 0..n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            d[r * a.ncols + a.col[p]] += a.val[p];
        }
    }
    d
}

/// `kappa = 2 r^T C s / (r^T M r + s^T M s)` (`Im(phi^H C phi) = 2 r^T C s` for skew `C`).
fn whirl_number(c: &[f64], m: &[f64], n: usize, re: &[f64], im: &[f64]) -> f64 {
    let num = 2.0 * dot(re, &dense_matvec(c, n, im));
    let den = dot(re, &dense_matvec(m, n, re)) + dot(im, &dense_matvec(m, n, im));
    num / den
}

fn speed_refused(omega: f64) -> FemError {
    FemError::InvalidValue {
        key: "speed",
        reason: format!("{omega} rad/s is not a finite speed >= 0"),
    }
}

fn mass_refused(what: String, d: f64) -> FemError {
    FemError::InvalidValue {
        key: "density",
        reason: format!("{what} has no mass (diagonal {d:e})"),
    }
}

/// The inertia test of ADR 0403 D3: `Ok(None)` if `kr` is positive definite, `Ok(Some(negative))` otherwise.
fn inertia(kr: &CsrMatrix, mr: &CsrMatrix) -> Result<Option<Option<usize>>> {
    match count_below(kr, mr, 0.0, &SkylineOrdering::Rcm) {
        Ok(0) => Ok(None),
        Ok(negative) => Ok(Some(Some(negative))),
        Err(EigenError::CountFailed { .. }) => Ok(Some(None)),
        Err(e) => Err(FemError::Solver {
            solver: "count_below",
            reason: e.to_string(),
        }),
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// The whole part.

impl<'a> RotatingPart<'a> {
    /// The Coriolis matrix per unit speed `C` of the part ([`coriolis_matrix`] about its axis; `G = 2 Omega C`).
    pub fn coriolis(&self) -> Result<CsrMatrix> {
        coriolis_matrix(self.model, self.axis, self.options.modal.threads)
    }

    /// The whirl modes at the speed `omega`, rad/s (ADR 0411 D2, D3): the lowest `options.modal.modes` of `(K_eff - omega^2
    /// M + i omega G) phi = 0`, `G` the Coriolis matrix if `options.coriolis` and 0 otherwise. At `omega = 0` the modes of
    /// [`RotatingPart::modes`] (the bits of the modal analysis) as real modes. A speed at which `K_eff` is not positive
    /// definite is a loss of stiffness. Refuses a negative or non-finite speed (`"speed"`) and more than
    /// [`MAX_WHIRL_UNKNOWNS`] reduced unknowns (`"unknowns"`).
    pub fn whirl_modes(&self, omega: f64) -> Result<WhirlSpeedModes> {
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(speed_refused(omega));
        }
        let c = self.coriolis()?;
        self.whirl_at(omega, &c)
    }

    fn whirl_at(&self, omega: f64, c: &CsrMatrix) -> Result<WhirlSpeedModes> {
        if omega == 0.0 {
            return match self.modes(0.0)? {
                SpeedModes::Modes(r) => {
                    let modes: Vec<WhirlMode> = (0..r.eigenvalues.len())
                        .map(|i| {
                            let l = r.eigenvalues[i];
                            WhirlMode {
                                omega: l.signum() * l.abs().sqrt(),
                                eigenvalue: l,
                                frequency: r.frequencies[i],
                                re: r.modes[i].clone(),
                                im: vec![0.0; r.modes[i].len()],
                                whirl: 0.0,
                                direction: Whirl::Neutral,
                                wave: None,
                                residual: r.residuals[i],
                            }
                        })
                        .collect();
                    Ok(WhirlSpeedModes::Modes(WhirlModes {
                        frequencies_all: modes.iter().map(|m| m.omega).collect(),
                        modes,
                        unknowns: r.unknowns,
                    }))
                }
                SpeedModes::LossOfStiffness(l) => Ok(WhirlSpeedModes::LossOfStiffness(l)),
            };
        }
        let opts = &self.options.modal;
        let dofs = self.model.dof_map();
        let keff = self.operator(omega);
        let n = self.m.n;
        let (kr, mr, cr, free) = match &self.path {
            Path::Elimination { fixed } => {
                let zero = vec![0.0; n];
                let kr = reduce(&keff, &zero, fixed)?;
                let mr = reduce(&self.m, &zero, fixed)?;
                let cr = reduce(c, &zero, fixed)?;
                let free = kr.free().to_vec();
                (kr.matrix, mr.matrix, cr.matrix, Some(free))
            }
            Path::Transformation(tr) => {
                let zero = vec![0.0; tr.n_unknowns()];
                let (kr, _) = tr.reduce(&keff, &zero)?;
                let (mr, _) = tr.reduce(&self.m, &zero)?;
                let (cr, _) = tr.reduce(c, &zero)?;
                (kr, mr, cr, None)
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
                return Err(mass_refused(what, *d));
            }
        }
        if let Some(negative) = inertia(&kr, &mr)? {
            return Ok(WhirlSpeedModes::LossOfStiffness(LossOfStiffness {
                speed: omega,
                negative,
            }));
        }
        let (pairs, all) = if self.options.whirl_solver.sparse_for(nf) {
            let g_speed = if self.options.coriolis { omega } else { 0.0 };
            let pairs = sparse::solve_reduced(&kr, &mr, &cr, g_speed, opts.modes)?;
            let all = pairs.iter().map(|p| p.0).collect();
            (pairs, all)
        } else {
            check_size(nf)?;
            let kd = dense_of(&kr);
            let md = dense_of(&mr);
            let cd = dense_of(&cr);
            let gd: Vec<f64> = if self.options.coriolis {
                cd.iter().map(|v| 2.0 * omega * v).collect()
            } else {
                vec![0.0; nf * nf]
            };
            let out = solve_dense(&kd, &md, &gd, nf, opts.modes)?;
            let pairs: Vec<_> = out
                .pairs
                .into_iter()
                .map(|(w, re, im, res)| {
                    let kappa = whirl_number(&cd, &md, nf, &re, &im);
                    (w, re, im, res, kappa)
                })
                .collect();
            (pairs, out.all)
        };
        let expand = |q: &[f64]| -> Vec<f64> {
            match (&self.path, &free) {
                (Path::Elimination { .. }, Some(free)) => {
                    let mut full = vec![0.0; n];
                    for (&eq, &x) in free.iter().zip(q) {
                        full[eq] = x;
                    }
                    full
                }
                (Path::Transformation(tr), _) => tr.expand(q)[..n].to_vec(),
                _ => unreachable!("the elimination path has its free list"),
            }
        };
        let modes = pairs
            .iter()
            .map(|(w, re, im, res, kappa)| WhirlMode {
                omega: *w,
                eigenvalue: w * w,
                frequency: w / (2.0 * PI),
                re: expand(re),
                im: expand(im),
                whirl: *kappa,
                direction: Whirl::of(*kappa),
                wave: None,
                residual: *res,
            })
            .collect();
        Ok(WhirlSpeedModes::Modes(WhirlModes {
            modes,
            frequencies_all: all,
            unknowns: nf,
        }))
    }

    /// The whirl modes over a list of speeds, rad/s (as [`RotatingPart::campbell`]; key `"speeds"`), tracked by the
    /// subspace assurance of the complex lines of eigenvalue clusters (ADR 0411 D4), with each track's whirl number and
    /// direction.
    pub fn whirl_campbell(&self, speeds: &[f64]) -> Result<WhirlCampbell> {
        check_speeds(speeds)?;
        let c = self.coriolis()?;
        let mut entries = Vec::with_capacity(speeds.len());
        for &w in speeds {
            entries.push(match self.whirl_at(w, &c)? {
                WhirlSpeedModes::Modes(r) => Entry::Modes(r),
                WhirlSpeedModes::LossOfStiffness(l) => Entry::Loss(l),
            });
        }
        Ok(track_whirl(speeds, &entries, &self.m))
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// The sector.

/// `blockdiag(a, a)`.
fn doubled(a: &CsrMatrix) -> CsrMatrix {
    crate::cyclic::doubled_matrix(a)
}

impl<'a> RotatingSector<'a> {
    /// The whirl modes of harmonic index `harmonic` at the speed `omega`, rad/s (ADR 0411 D5): `K_eff`, `M` and `G` of the
    /// sector, doubled for a pair, reduced by the rows of the index, solved by D2; `options.modal.modes x multiplicity`
    /// modes, each with its whirl number and, for a pair, its wave sense. At `omega = 0` the modes of
    /// [`crate::cyclic::modal_analysis_harmonic`] as real modes. Refuses a harmonic above `N / 2` (`"harmonic"`), a negative
    /// or non-finite speed (`"speed"`) and more than [`MAX_WHIRL_UNKNOWNS`] reduced unknowns (`"unknowns"`).
    pub fn whirl_modes(&self, harmonic: usize, omega: f64) -> Result<HarmonicWhirlSpeedModes> {
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(speed_refused(omega));
        }
        let c = self.part.coriolis()?;
        self.whirl_at(harmonic, omega, &c)
    }

    fn whirl_at(&self, harmonic: usize, omega: f64, c: &CsrMatrix) -> Result<HarmonicWhirlSpeedModes> {
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
        let multiplicity = if harmonic == 0 || 2 * harmonic == sectors { 1 } else { 2 };
        let n = model.dof_map().n_equations();
        if omega == 0.0 {
            let h = crate::cyclic::modal_analysis_harmonic(model, &[], harmonic, options)?;
            let modes: Vec<WhirlMode> = (0..h.eigenvalues.len())
                .map(|i| {
                    let l = h.eigenvalues[i];
                    let m = &h.modes[i];
                    let re: Vec<f64> = if multiplicity == 2 {
                        m.cos.iter().chain(&m.sin).copied().collect()
                    } else {
                        m.cos.clone()
                    };
                    WhirlMode {
                        omega: l.signum() * l.abs().sqrt(),
                        eigenvalue: l,
                        frequency: crate::modal::frequency(l),
                        im: vec![0.0; re.len()],
                        re,
                        whirl: 0.0,
                        direction: Whirl::Neutral,
                        wave: if multiplicity == 2 { Some(0.0) } else { None },
                        residual: h.residuals[i],
                    }
                })
                .collect();
            return Ok(HarmonicWhirlSpeedModes::Modes {
                harmonic,
                multiplicity,
                modes: WhirlModes {
                    frequencies_all: modes.iter().map(|m| m.omega).collect(),
                    modes,
                    unknowns: h.unknowns,
                },
            });
        }
        let tr: Transformation = if harmonic == 0 {
            crate::mpc::transformation(model, &[])?
        } else {
            crate::mpc::harmonic_transformation(model, &[], harmonic)?
        };
        let keff = self.part.operator(omega);
        let (k2, m2, c2) = if multiplicity == 2 {
            (doubled(&keff), doubled(&self.part.m), doubled(c))
        } else {
            (keff, self.part.m.clone(), c.clone())
        };
        let zero = vec![0.0; tr.n_unknowns()];
        let (kr, _) = tr.reduce(&k2, &zero)?;
        let (mr, _) = tr.reduce(&m2, &zero)?;
        let (cr, _) = tr.reduce(&c2, &zero)?;
        let nq = kr.n;
        let count = options.modes * multiplicity;
        if options.modes == 0 || count >= nq {
            return Err(FemError::InvalidValue {
                key: "modes",
                reason: format!(
                    "{} modes ({count} whirl modes) requested of harmonic {harmonic} with {nq} free unknowns (at least 1 \
                     and fewer than the unknowns)",
                    options.modes
                ),
            });
        }
        for (a, d) in mr.diag().iter().enumerate() {
            if d.is_nan() || *d <= 0.0 {
                let i = tr.independent()[a];
                return Err(mass_refused(crate::cyclic::unknown_label(&tr, model, i), *d));
            }
        }
        if let Some(negative) = inertia(&kr, &mr)? {
            return Ok(HarmonicWhirlSpeedModes::LossOfStiffness(HarmonicLoss {
                harmonic,
                multiplicity,
                loss: LossOfStiffness { speed: omega, negative },
            }));
        }
        let (pairs, all) = if self.part.options.whirl_solver.sparse_for(nq) {
            let g_speed = if self.part.options.coriolis { omega } else { 0.0 };
            let pairs = sparse::solve_reduced(&kr, &mr, &cr, g_speed, count)?;
            let all = pairs.iter().map(|p| p.0).collect();
            (pairs, all)
        } else {
            check_size(nq)?;
            let kd = dense_of(&kr);
            let md = dense_of(&mr);
            let cd = dense_of(&cr);
            let gd: Vec<f64> = if self.part.options.coriolis {
                cd.iter().map(|v| 2.0 * omega * v).collect()
            } else {
                vec![0.0; nq * nq]
            };
            let out = solve_dense(&kd, &md, &gd, nq, count)?;
            let pairs: Vec<_> = out
                .pairs
                .into_iter()
                .map(|(w, re, im, res)| {
                    let kappa = whirl_number(&cd, &md, nq, &re, &im);
                    (w, re, im, res, kappa)
                })
                .collect();
            (pairs, out.all)
        };
        let width = multiplicity * n;
        let m = &self.part.m;
        let modes = pairs
            .iter()
            .map(|(w, re, im, res, kappa)| {
                let kappa = *kappa;
                let xr = tr.expand(re)[..width].to_vec();
                let xi = tr.expand(im)[..width].to_vec();
                let wave = if multiplicity == 2 {
                    let bil = |x: &[f64], y: &[f64]| -> f64 {
                        let mut t = vec![0.0; n];
                        m.matvec(y, &mut t);
                        dot(x, &t)
                    };
                    let (ar, br) = (&xr[..n], &xr[n..]);
                    let (ai, bi) = (&xi[..n], &xi[n..]);
                    let num = 2.0 * (bil(ar, bi) - bil(ai, br));
                    let den = bil(ar, ar) + bil(ai, ai) + bil(br, br) + bil(bi, bi);
                    Some(num / den)
                } else {
                    None
                };
                WhirlMode {
                    omega: *w,
                    eigenvalue: w * w,
                    frequency: w / (2.0 * PI),
                    re: xr,
                    im: xi,
                    whirl: kappa,
                    direction: Whirl::of(kappa),
                    wave,
                    residual: *res,
                }
            })
            .collect();
        Ok(HarmonicWhirlSpeedModes::Modes {
            harmonic,
            multiplicity,
            modes: WhirlModes {
                modes,
                frequencies_all: all,
                unknowns: nq,
            },
        })
    }

    /// The whirl Campbell diagram over the harmonic indices `harmonics` (as [`RotatingSector::campbell`]; key
    /// `"harmonics"`) and the speeds `speeds` (key `"speeds"`): one [`WhirlCampbell`] per index (ADR 0411 D5).
    pub fn whirl_campbell(&self, harmonics: &[usize], speeds: &[f64]) -> Result<CyclicWhirlCampbell> {
        check_harmonics(harmonics, self.sectors)?;
        check_speeds(speeds)?;
        let c = self.part.coriolis()?;
        let mut families = Vec::with_capacity(harmonics.len());
        let mut multiplicities = Vec::with_capacity(harmonics.len());
        for &k in harmonics {
            let mut entries = Vec::with_capacity(speeds.len());
            for &w in speeds {
                entries.push(match self.whirl_at(k, w, &c)? {
                    HarmonicWhirlSpeedModes::Modes { modes, .. } => Entry::Modes(modes),
                    HarmonicWhirlSpeedModes::LossOfStiffness(l) => Entry::Loss(l.loss),
                });
            }
            multiplicities.push(if k == 0 || 2 * k == self.sectors { 1 } else { 2 });
            families.push(track_whirl(speeds, &entries, &self.part.m));
        }
        let stationary = harmonics
            .iter()
            .zip(&families)
            .map(|(&k, fam)| stationary_column(k, fam))
            .collect();
        Ok(CyclicWhirlCampbell {
            sectors: self.sectors,
            harmonics: harmonics.to_vec(),
            multiplicities,
            families,
            stationary,
        })
    }
}

/// The frequency, rad/s, seen in the stationary frame of a whirl mode of harmonic index `harmonic` with the rotating-frame
/// frequency `omega`, rad/s, and the wave sense `wave` at the speed `speed`, rad/s (ADR 0419 D2): `omega + tau harmonic
/// speed`, `tau = wave` (`0` for `None`: a single index, `k = 0` or `2 k = N`, is a standing wave whose two lines `omega -/+ k
/// Omega` have the mean `omega`). Signed: a negative value is a backward wave seen at `|omega_stat|` travelling forward in
/// the stationary frame. For `|tau| < 1` (a degenerate pair) it is the energy-weighted mean of the lines `omega -/+ k Omega`.
pub fn stationary_frequency(omega: f64, harmonic: usize, wave: Option<f64>, speed: f64) -> f64 {
    omega + wave.unwrap_or(0.0) * harmonic as f64 * speed
}

/// The stationary-frame frequencies, Hz, of one family (ADR 0419 D4): `f + tau k Omega / (2 pi)` per track and speed from the
/// stored frequency, wave sense and speed; `None` where the track has no mode.
fn stationary_column(harmonic: usize, fam: &WhirlCampbell) -> Vec<Vec<Option<f64>>> {
    let speeds = &fam.table.speeds;
    fam.table
        .tracks
        .iter()
        .zip(&fam.waves)
        .map(|(tr, waves)| {
            tr.frequencies
                .iter()
                .zip(waves)
                .zip(speeds)
                .map(|((f, w), &omega)| f.map(|f| f + w.unwrap_or(0.0) * harmonic as f64 * omega / (2.0 * PI)))
                .collect()
        })
        .collect()
}

// ------------------------------------------------------------------------------------------------------------------------
// D4: tracking and the table.

pub(crate) enum Entry {
    Modes(WhirlModes),
    Loss(LossOfStiffness),
}

/// The complex line of `r + i s` in real coordinates: `{(r, s), (-s, r)}`.
fn complex_line(re: &[f64], im: &[f64]) -> Vec<Vec<f64>> {
    let a: Vec<f64> = re.iter().chain(im).copied().collect();
    let b: Vec<f64> = im.iter().map(|x| -x).chain(re.iter().copied()).collect();
    vec![a, b]
}

/// The tracked whirl branches over the speeds of a [`WhirlCampbell`]: the table of F-396 (ADR 0403 D4) with the whirl
/// number, the direction and the wave sense of each track at each speed (ADR 0411 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WhirlCampbell {
    /// The table: eigenvalues `omega^2`, frequencies Hz, the complex MAC with the previous assigned mode, the modes as the
    /// real vectors `(re, im)` concatenated.
    pub table: CampbellTable,
    /// Per track and speed, the whirl number `kappa`.
    pub whirl: Vec<Vec<Option<f64>>>,
    /// Per track and speed, the direction (`None` at rest and where the track has no mode).
    pub directions: Vec<Vec<Option<Whirl>>>,
    /// Per track and speed, the wave sense of a sector pair (`None` otherwise).
    pub waves: Vec<Vec<Option<f64>>>,
}

/// The tracks under construction.
struct Tracks {
    ns: usize,
    tracks: Vec<ModeTrack>,
    whirl: Vec<Vec<Option<f64>>>,
    directions: Vec<Vec<Option<Whirl>>>,
    waves: Vec<Vec<Option<f64>>>,
}

impl Tracks {
    fn open(&mut self) {
        let ns = self.ns;
        self.tracks.push(ModeTrack {
            eigenvalues: vec![None; ns],
            frequencies: vec![None; ns],
            mac: vec![None; ns],
            modes: vec![None; ns],
        });
        self.whirl.push(vec![None; ns]);
        self.directions.push(vec![None; ns]);
        self.waves.push(vec![None; ns]);
    }
    fn put(&mut self, t: usize, j: usize, w: &WhirlMode, line: &[f64], mac: Option<f64>, at_rest: bool) {
        let tr = &mut self.tracks[t];
        tr.eigenvalues[j] = Some(w.eigenvalue);
        tr.frequencies[j] = Some(w.frequency);
        tr.mac[j] = mac;
        tr.modes[j] = Some(line.to_vec());
        self.whirl[t][j] = Some(w.whirl);
        self.directions[t][j] = if at_rest { None } else { Some(w.direction) };
        self.waves[t][j] = w.wave;
    }
}

pub(crate) fn track_whirl(speeds: &[f64], entries: &[Entry], m: &CsrMatrix) -> WhirlCampbell {
    let mut tk = Tracks {
        ns: speeds.len(),
        tracks: Vec::new(),
        whirl: Vec::new(),
        directions: Vec::new(),
        waves: Vec::new(),
    };
    let mut last: Vec<Vec<Vec<f64>>> = Vec::new();
    let mut losses = Vec::new();
    for (j, entry) in entries.iter().enumerate() {
        let res = match entry {
            Entry::Loss(l) => {
                losses.push(*l);
                continue;
            }
            Entry::Modes(r) => r,
        };
        let values: Vec<f64> = res.modes.iter().map(|w| w.eigenvalue).collect();
        let lines: Vec<Vec<Vec<f64>>> = res.modes.iter().map(|w| complex_line(&w.re, &w.im)).collect();
        let groups = cluster_spaces(&values, &lines);
        if tk.tracks.is_empty() {
            for (i, g) in groups.iter().enumerate() {
                tk.open();
                tk.put(i, j, &res.modes[i], &lines[i][0], None, speeds[j] == 0.0);
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
        // Decreasing assurance; ties by the lower track, then the lower mode.
        pairs.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut track_used = vec![false; tk.tracks.len()];
        let mut mode_used = vec![false; groups.len()];
        for (value, t, i) in pairs {
            if track_used[t] || mode_used[i] {
                continue;
            }
            track_used[t] = true;
            mode_used[i] = true;
            tk.put(t, j, &res.modes[i], &lines[i][0], Some(value), speeds[j] == 0.0);
            last[t] = groups[i].clone();
        }
    }
    WhirlCampbell {
        table: CampbellTable {
            speeds: speeds.to_vec(),
            tracks: tk.tracks,
            losses,
        },
        whirl: tk.whirl,
        directions: tk.directions,
        waves: tk.waves,
    }
}

/// The speeds, rad/s, at which a whirl branch `omega(Omega)` (rad/s, `None` where it has no value) given at increasing speeds
/// crosses the engine-order line `omega = order Omega`: on each interval with values at both ends, `omega` linear in
/// `Omega`, the root of `omega - order Omega` (ADR 0411 D4). Exact for `omega_0 -/+ Omega`.
pub fn whirl_order_crossings(speeds: &[f64], omegas: &[Option<f64>], order: u32) -> Vec<f64> {
    let k = f64::from(order);
    let n = speeds.len().min(omegas.len());
    let g = |j: usize| omegas[j].map(|w| w - k * speeds[j]);
    let mut out = Vec::new();
    for j in 0..n {
        if g(j) == Some(0.0) {
            out.push(speeds[j]);
        }
        if j + 1 < n {
            if let (Some(ga), Some(gb)) = (g(j), g(j + 1)) {
                if (ga < 0.0 && gb > 0.0) || (ga > 0.0 && gb < 0.0) {
                    let (sa, sb) = (speeds[j], speeds[j + 1]);
                    out.push(sa - ga * (sb - sa) / (gb - ga));
                }
            }
        }
    }
    out
}

impl WhirlCampbell {
    /// The rotating-frame frequency, rad/s, of track `t` at each speed (`sqrt` of the stored `omega^2`, which gives `omega`
    /// back exactly).
    pub fn omegas(&self, t: usize) -> Vec<Option<f64>> {
        self.table.tracks[t]
            .eigenvalues
            .iter()
            .map(|l| l.map(f64::sqrt))
            .collect()
    }

    /// The crossings of every track with the engine order `order` by [`whirl_order_crossings`], track by track.
    pub fn crossings(&self, order: u32) -> Vec<Crossing> {
        let mut out = Vec::new();
        for t in 0..self.table.tracks.len() {
            for speed in whirl_order_crossings(&self.table.speeds, &self.omegas(t), order) {
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

    /// The table as CSV: the columns of [`CampbellTable::to_csv`] followed by `track_t_dir` per track (`forward`,
    /// `backward`, `neutral`, empty where the track has no mode or at rest).
    pub fn to_csv(&self) -> String {
        let base = self.table.to_csv();
        let mut s = String::new();
        for (j, line) in base.lines().enumerate() {
            s.push_str(line);
            if j == 0 {
                for t in 0..self.table.tracks.len() {
                    let _ = write!(s, ",track_{t}_dir");
                }
            } else {
                let r = j - 1;
                let at_rest = self.table.speeds[r] == 0.0;
                for d in &self.directions {
                    s.push(',');
                    if let (Some(d), false) = (d[r], at_rest) {
                        s.push_str(d.name());
                    }
                }
            }
            s.push('\n');
        }
        s
    }

    /// The table as JSON: the speeds, each track's frequencies (Hz), MAC, whirl numbers and directions (`null` where it has
    /// none), the losses of stiffness and the crossings ([`WhirlCampbell::crossings`]) with the given orders.
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
            list(self.table.speeds.iter().map(|w| format!("{w}")))
        );
        s.push_str("  \"tracks\": [\n");
        let nt = self.table.tracks.len();
        for (t, tr) in self.table.tracks.iter().enumerate() {
            let _ = write!(
                s,
                "    {{\"track\": {t}, \"frequencies_hz\": {}, \"mac\": {}, \"whirl\": {}, \"direction\": {}}}",
                list(tr.frequencies.iter().map(|&f| opt(f))),
                list(tr.mac.iter().map(|&m| opt(m))),
                list(self.whirl[t].iter().map(|&k| opt(k))),
                list(
                    self.directions[t]
                        .iter()
                        .map(|d| d.map_or_else(|| "null".to_string(), |d| format!("\"{}\"", d.name())))
                )
            );
            s.push_str(if t + 1 < nt { ",\n" } else { "\n" });
        }
        s.push_str("  ],\n");
        let _ = writeln!(
            s,
            "  \"losses_of_stiffness\": {},",
            list(self.table.losses.iter().map(|l| format!(
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
        s.push_str("}\n");
        s
    }
}

/// The whirl Campbell diagram of a cyclic sector: one [`WhirlCampbell`] per harmonic index (ADR 0411 D5).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct CyclicWhirlCampbell {
    /// The number of sectors `N`.
    pub sectors: usize,
    /// The harmonic indices, increasing.
    pub harmonics: Vec<usize>,
    /// Their multiplicities (1 for `k = 0` and `2 k = N`, 2 otherwise).
    pub multiplicities: Vec<usize>,
    /// The families, one per index; a pair mode's vectors are `(cos, sin)` of the real and of the imaginary part.
    pub families: Vec<WhirlCampbell>,
    /// Per family, track and speed, the stationary-frame frequency, Hz (ADR 0419 D2, D4): `f + tau k Omega / (2 pi)`, `f` the
    /// family's frequency, `tau` its wave sense (`0` where it has none), `k` the index, `Omega` the speed; `None` where the
    /// track has no mode. At rest and on single indices the bits of `f`.
    pub stationary: Vec<Vec<Vec<Option<f64>>>>,
}

impl CyclicWhirlCampbell {
    /// The stationary-frame frequencies as CSV (ADR 0419 D4): `harmonic,speed_rad_s,speed_rpm` then `track_t_stationary_hz`
    /// up to the widest family; one row per family and speed; empty where a track has no mode and in the padding of a
    /// narrower family.
    pub fn stationary_csv(&self) -> String {
        let width = self.stationary.iter().map(Vec::len).max().unwrap_or(0);
        let mut s = String::from("harmonic,speed_rad_s,speed_rpm");
        for t in 0..width {
            let _ = write!(s, ",track_{t}_stationary_hz");
        }
        s.push('\n');
        for ((&k, fam), col) in self.harmonics.iter().zip(&self.families).zip(&self.stationary) {
            for (j, &w) in fam.table.speeds.iter().enumerate() {
                let _ = write!(s, "{k},{w},{}", w * 60.0 / (2.0 * PI));
                for t in 0..width {
                    s.push(',');
                    if let Some(Some(f)) = col.get(t).map(|c| c[j]) {
                        let _ = write!(s, "{f}");
                    }
                }
                s.push('\n');
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic pseudo-random number in [-1, 1).
    fn lcg(state: &mut u64) -> f64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((*state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }

    fn spd(n: usize, state: &mut u64) -> Vec<f64> {
        let b: Vec<f64> = (0..n * n).map(|_| lcg(state)).collect();
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                a[i * n + j] = (0..n).map(|k| b[i * n + k] * b[j * n + k]).sum::<f64>();
            }
            a[i * n + i] += n as f64;
        }
        a
    }

    #[test]
    fn dense_solver_residuals_and_count() {
        let mut st = 7u64;
        for n in [1usize, 2, 5, 12] {
            let k = spd(n, &mut st);
            let m = spd(n, &mut st);
            let mut g = vec![0.0; n * n];
            for i in 0..n {
                for j in i + 1..n {
                    let v = 3.0 * lcg(&mut st);
                    g[i * n + j] = v;
                    g[j * n + i] = -v;
                }
            }
            let r = gyroscopic_eigen_dense(&k, &m, &g, n).unwrap();
            assert_eq!(r.omegas.len(), n);
            for (w, res) in r.omegas.iter().zip(&r.residuals) {
                assert!(w.is_finite() && *w > 0.0);
                assert!(*res < 1e-12, "n = {n}: residual {res:e}");
            }
        }
    }
}
