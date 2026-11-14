//! Whirl modes at any speed, above the loss of stiffness too: the state matrix of the gyroscopic problem, its eigenvalues by
//! Hessenberg reduction and the Francis double-shift QR algorithm, a stability flag with the growth rate (ADR 0416, F-505).
//!
//! `M u_tt + G u_t + K_eff u = 0` with `K_eff` of any inertia. With `M = L L^T`, `u = L^-T y`: `y_tt + G^ y_t + K^ y = 0`; the
//! state `z = [y; y_t / c]` gives `z_t = A z`, `A = [[0, c I], [-K^ / c, -G^]]`, `c = sqrt(max|K^| + max|G^|^2)` (D1). The
//! eigenvalues `s` of `A` (D2) are those of `Q(s) = s^2 M + s G + K_eff`; the spectrum is symmetric about both axes (D3), so
//! the speed is stable iff every `s` is imaginary: flagged [`Stability::Stable`] when `max|Re s| <= STABILITY_TOLERANCE
//! max|s|`, otherwise [`Stability::Unstable`] with the growth rate `max Re s`. The mode vectors come from inverse iteration on
//! `Q(s)` (D4). The F-499 functions are not changed: [`RotatingPart::whirl_modes`] still reports such a speed as a loss.

use super::{
    check_size, cholesky, dense_matvec, dense_of, inertia, lower_solve, mass_refused, speed_refused, transpose,
};
use super::{whirl_number, Whirl};
use crate::constraint::reduce;
use crate::error::{FemError, Result};
use crate::rotating::{check_speeds, LossOfStiffness, Path, RotatingPart};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::fmt::Write as _;

mod sector;
mod springs;
pub use sector::{CyclicWhirlStability, HarmonicStateWhirl};

/// `max|Re s| / max|s|` at or below which a speed is stable (ADR 0416 D3).
pub const STABILITY_TOLERANCE: f64 = 1e-8;

/// Iterations of the Francis QR algorithm on one active window before the solve is refused.
const QR_ITERATIONS: usize = 30;

/// Solves of the inverse iteration for a mode vector.
const INVERSE_ITERATIONS: usize = 3;

/// The stability of a speed (ADR 0416 D3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stability {
    /// Every eigenvalue imaginary within [`STABILITY_TOLERANCE`]: every frequency real.
    Stable,
    /// An eigenvalue with a positive real part: the motion grows as `e^{growth t}`.
    Unstable {
        /// The growth rate `max Re s`, 1/s.
        growth: f64,
    },
}

impl Stability {
    /// Whether the speed is stable.
    pub fn is_stable(&self) -> bool {
        matches!(self, Stability::Stable)
    }
}

/// All eigenvalues of the state matrix of a dense gyroscopic problem and its reported modes (ADR 0416 D3, D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct DenseStateWhirl {
    /// The `2 n` eigenvalues `[Re s, Im s]`, ordered by `(Im s, Re s)`.
    pub eigenvalues: Vec<[f64; 2]>,
    /// The flag of D3.
    pub stability: Stability,
    /// `max|Re s| / max|s|` (0 if every eigenvalue is 0).
    pub real_defect: f64,
    /// `Re s` of each reported mode (`Im s > 0`, or real with `Re s >= 0`, ordered by `(Im s, Re s)`).
    pub growth: Vec<f64>,
    /// `Im s` of each reported mode, rad/s.
    pub omegas: Vec<f64>,
    /// The real parts of the modes, `phi^H M phi = 1`, the entry of largest modulus real and positive.
    pub re: Vec<Vec<f64>>,
    /// The imaginary parts.
    pub im: Vec<Vec<f64>>,
    /// `||Q(s) phi|| / (||K phi|| + |s|^2 ||M phi|| + |s| ||G phi||)` of each.
    pub residuals: Vec<f64>,
}

/// One mode `u = Re((re + i im) e^{s t})`, `s = growth + i omega`, of a rotating part (ADR 0416 D4).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct StateWhirlMode {
    /// `Re s`, 1/s (0 to rounding for a stable speed).
    pub growth: f64,
    /// `Im s >= 0`, rad/s, the frequency in the rotating frame.
    pub omega: f64,
    /// `omega / (2 pi)`, Hz.
    pub frequency: f64,
    /// The real part of the mode on the model's equations, `phi^H M phi = 1`.
    pub re: Vec<f64>,
    /// The imaginary part.
    pub im: Vec<f64>,
    /// The whirl number `kappa = Im(phi^H C phi) / phi^H M phi` (ADR 0411 D3).
    pub whirl: f64,
    /// The direction of `kappa`.
    pub direction: Whirl,
    /// The residual of the quadratic problem on the reduced unknowns.
    pub residual: f64,
}

/// The whirl analysis of a part at one speed by the state matrix (ADR 0416).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct StateWhirl {
    /// The speed, rad/s.
    pub speed: f64,
    /// `None` if the reduced `K_eff` is positive definite, otherwise the loss of stiffness of ADR 0403 D3.
    pub loss: Option<LossOfStiffness>,
    /// The `2 n` eigenvalues `[Re s, Im s]` of the state matrix, ordered by `(Im s, Re s)`.
    pub eigenvalues: Vec<[f64; 2]>,
    /// The flag of D3.
    pub stability: Stability,
    /// `max|Re s| / max|s|`.
    pub real_defect: f64,
    /// The lowest `options.modal.modes` reported modes (D3), ordered by `(omega, growth)`.
    pub modes: Vec<StateWhirlMode>,
    /// The number of reduced unknowns `n`.
    pub unknowns: usize,
}

/// The stability of a part over a list of speeds (ADR 0416 D6).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct WhirlStability {
    /// The speeds, rad/s, increasing.
    pub speeds: Vec<f64>,
    /// The analysis at each speed.
    pub results: Vec<StateWhirl>,
}

impl WhirlStability {
    /// The table as CSV: `speed_rad_s,negative_stiffness,stable,growth_rate_1_s` and `f_<j>_hz` per reported mode
    /// (`negative_stiffness` 0 when `K_eff` is positive definite, empty when its factorisation met a zero pivot;
    /// `growth_rate_1_s` 0 for a stable speed; a missing mode is an empty field).
    pub fn to_csv(&self) -> String {
        let width = self.results.iter().map(|r| r.modes.len()).max().unwrap_or(0);
        let mut s = String::from("speed_rad_s,negative_stiffness,stable,growth_rate_1_s");
        for j in 0..width {
            let _ = write!(s, ",f_{j}_hz");
        }
        s.push('\n');
        for (w, r) in self.speeds.iter().zip(&self.results) {
            let negative = match r.loss {
                None => "0".to_string(),
                Some(l) => l.negative.map_or_else(String::new, |n| n.to_string()),
            };
            let growth = match r.stability {
                Stability::Stable => 0.0,
                Stability::Unstable { growth } => growth,
            };
            let _ = write!(s, "{w},{negative},{},{growth}", r.stability.is_stable());
            for j in 0..width {
                s.push(',');
                if let Some(m) = r.modes.get(j) {
                    let _ = write!(s, "{}", m.frequency);
                }
            }
            s.push('\n');
        }
        s
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// D2: Hessenberg reduction and the Francis double-shift QR algorithm (eigenvalues only).

/// Householder reduction of the row-major `m x m` matrix `a` to upper Hessenberg form, in place (a similarity).
fn hessenberg(a: &mut [f64], m: usize) {
    let mut v = vec![0.0; m];
    for k in 0..m.saturating_sub(2) {
        let alpha = (k + 1..m).map(|i| a[i * m + k] * a[i * m + k]).sum::<f64>().sqrt();
        if alpha == 0.0 {
            continue;
        }
        let sg = if a[(k + 1) * m + k] >= 0.0 { 1.0 } else { -1.0 };
        v.iter_mut().for_each(|x| *x = 0.0);
        for i in k + 1..m {
            v[i] = a[i * m + k];
        }
        v[k + 1] += sg * alpha;
        let vn2: f64 = v[k + 1..].iter().map(|x| x * x).sum();
        if vn2 == 0.0 {
            continue;
        }
        let beta = 2.0 / vn2;
        // A <- (I - beta v v^T) A on rows k+1.. (the columns before k are zero there).
        for j in k..m {
            let d: f64 = (k + 1..m).map(|i| v[i] * a[i * m + j]).sum();
            let f = beta * d;
            if f != 0.0 {
                for i in k + 1..m {
                    a[i * m + j] -= f * v[i];
                }
            }
        }
        // A <- A (I - beta v v^T) on columns k+1..
        for i in 0..m {
            let d: f64 = (k + 1..m).map(|j| a[i * m + j] * v[j]).sum();
            let f = beta * d;
            if f != 0.0 {
                for j in k + 1..m {
                    a[i * m + j] -= f * v[j];
                }
            }
        }
        for i in k + 2..m {
            a[i * m + k] = 0.0;
        }
    }
}

/// The Householder vector and `beta` of `I - beta v v^T` that maps `x` onto a multiple of `e_1`; `None` for `x = 0`.
fn house<const N: usize>(x: [f64; N]) -> Option<([f64; N], f64)> {
    let alpha = x.iter().map(|v| v * v).sum::<f64>().sqrt();
    if alpha == 0.0 {
        return None;
    }
    let mut v = x;
    v[0] += if x[0] >= 0.0 { alpha } else { -alpha };
    let vn2: f64 = v.iter().map(|t| t * t).sum();
    if vn2 == 0.0 {
        return None;
    }
    Some((v, 2.0 / vn2))
}

/// Applies `I - beta v v^T` on rows `r..r+N` from the left (columns `c0..=c1`) and on columns `r..r+N` from the right (rows
/// `l..=r1`).
#[allow(clippy::too_many_arguments)]
fn reflect<const N: usize>(
    h: &mut [f64],
    m: usize,
    r: usize,
    v: &[f64; N],
    beta: f64,
    c0: usize,
    c1: usize,
    l: usize,
    r1: usize,
) {
    for j in c0..=c1 {
        let d: f64 = (0..N).map(|q| v[q] * h[(r + q) * m + j]).sum();
        let f = beta * d;
        for q in 0..N {
            h[(r + q) * m + j] -= f * v[q];
        }
    }
    for i in l..=r1 {
        let d: f64 = (0..N).map(|q| h[i * m + r + q] * v[q]).sum();
        let f = beta * d;
        for q in 0..N {
            h[i * m + r + q] -= f * v[q];
        }
    }
}

/// One Francis double-shift step on the window `[l, hi]` (size >= 3) of the Hessenberg `h`, shifts of sum `s` and product
/// `t` (D2 step 3).
fn francis(h: &mut [f64], m: usize, l: usize, hi: usize, s: f64, t: f64) {
    let at = |i: usize, j: usize| i * m + j;
    let mut x = h[at(l, l)] * h[at(l, l)] + h[at(l, l + 1)] * h[at(l + 1, l)] - s * h[at(l, l)] + t;
    let mut y = h[at(l + 1, l)] * (h[at(l, l)] + h[at(l + 1, l + 1)] - s);
    let mut z = h[at(l + 1, l)] * h[at(l + 2, l + 1)];
    for k in l..hi - 1 {
        let c0 = if k > l { k - 1 } else { l };
        if let Some((v, beta)) = house([x, y, z]) {
            reflect(h, m, k, &v, beta, c0, hi, l, (k + 3).min(hi));
            if k > l {
                h[at(k + 1, k - 1)] = 0.0;
                h[at(k + 2, k - 1)] = 0.0;
            }
        }
        x = h[at(k + 1, k)];
        y = h[at(k + 2, k)];
        if k + 3 <= hi {
            z = h[at(k + 3, k)];
        }
    }
    if let Some((v, beta)) = house([x, y]) {
        reflect(h, m, hi - 1, &v, beta, hi - 2, hi, l, hi);
        h[at(hi, hi - 2)] = 0.0;
    }
}

/// The two eigenvalues of `[[a, b], [c, d]]` (D2 step 2).
fn eig2(a: f64, b: f64, c: f64, d: f64) -> [[f64; 2]; 2] {
    let t = 0.5 * (a + d);
    let p = 0.5 * (a - d);
    let q = p * p + b * c;
    if q >= 0.0 {
        let sq = q.sqrt();
        let r1 = if t >= 0.0 { t + sq } else { t - sq };
        let r2 = if r1 != 0.0 {
            (a * d - b * c) / r1
        } else if t >= 0.0 {
            t - sq
        } else {
            t + sq
        };
        [[r1, 0.0], [r2, 0.0]]
    } else {
        let w = (-q).sqrt();
        [[t, w], [t, -w]]
    }
}

/// The eigenvalues of the upper Hessenberg row-major `h` (consumed) by the real Schur iteration (D2 steps 2, 3).
fn hessenberg_eigenvalues(mut h: Vec<f64>, m: usize) -> Result<Vec<[f64; 2]>> {
    let mut out = Vec::with_capacity(m);
    if m == 0 {
        return Ok(out);
    }
    let scale = h.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
    let mut hi = m - 1;
    let mut iter = 0;
    loop {
        let mut l = hi;
        while l > 0 {
            let mut s = h[(l - 1) * m + l - 1].abs() + h[l * m + l].abs();
            if s == 0.0 {
                s = scale;
            }
            if h[l * m + l - 1].abs() <= f64::EPSILON * s {
                h[l * m + l - 1] = 0.0;
                break;
            }
            l -= 1;
        }
        if l == hi {
            out.push([h[hi * m + hi], 0.0]);
            iter = 0;
            if l == 0 {
                break;
            }
            hi = l - 1;
            continue;
        }
        if l + 1 == hi {
            let e = eig2(h[l * m + l], h[l * m + hi], h[hi * m + l], h[hi * m + hi]);
            out.extend_from_slice(&e);
            iter = 0;
            if l == 0 {
                break;
            }
            hi = l - 1;
            continue;
        }
        iter += 1;
        if iter > QR_ITERATIONS {
            return Err(FemError::Solver {
                solver: "gyroscopic_state_eigen",
                reason: format!("the QR iteration did not converge on rows {l}..={hi} in {QR_ITERATIONS} steps"),
            });
        }
        let (s, t) = if iter == 10 || iter == 20 {
            let w = h[hi * m + hi - 1].abs() + h[(hi - 1) * m + hi - 2].abs();
            (1.5 * w, w * w)
        } else {
            let (a, b, c, d) = (
                h[(hi - 1) * m + hi - 1],
                h[(hi - 1) * m + hi],
                h[hi * m + hi - 1],
                h[hi * m + hi],
            );
            (a + d, a * d - b * c)
        };
        francis(&mut h, m, l, hi, s, t);
    }
    Ok(out)
}

/// The eigenvalues of a general real row-major `m x m` matrix (D2).
fn real_eigenvalues(mut a: Vec<f64>, m: usize) -> Result<Vec<[f64; 2]>> {
    hessenberg(&mut a, m);
    hessenberg_eigenvalues(a, m)
}

// ------------------------------------------------------------------------------------------------------------------------
// D4: the mode vectors by inverse iteration on Q(s), in real pairs.

fn cmul(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] * b[0] - a[1] * b[1], a[0] * b[1] + a[1] * b[0]]
}

fn cdiv(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let d = b[0] * b[0] + b[1] * b[1];
    [(a[0] * b[0] + a[1] * b[1]) / d, (a[1] * b[0] - a[0] * b[1]) / d]
}

/// `Q(s) = K + s G + s^2 M` as a complex row-major matrix.
fn pencil(k: &[f64], m: &[f64], g: &[f64], s: [f64; 2]) -> Vec<[f64; 2]> {
    let s2 = cmul(s, s);
    (0..k.len())
        .map(|p| [k[p] + s[0] * g[p] + s2[0] * m[p], s[1] * g[p] + s2[1] * m[p]])
        .collect()
}

/// A null vector of `Q(s)` by inverse iteration (D4), as (real part, imaginary part), not yet normalised.
fn null_vector(k: &[f64], m: &[f64], g: &[f64], n: usize, s: [f64; 2]) -> (Vec<f64>, Vec<f64>) {
    let mut q = pencil(k, m, g, s);
    let qmax = q.iter().map(|z| z[0].hypot(z[1])).fold(0.0f64, f64::max);
    let tiny = f64::EPSILON * if qmax > 0.0 { qmax } else { 1.0 };
    let mut perm: Vec<usize> = (0..n).collect();
    for j in 0..n {
        let mut p = j;
        let mut best = -1.0;
        for i in j..n {
            let v = q[i * n + j][0].hypot(q[i * n + j][1]);
            if v > best {
                best = v;
                p = i;
            }
        }
        if p != j {
            for c in 0..n {
                q.swap(j * n + c, p * n + c);
            }
            perm.swap(j, p);
        }
        if q[j * n + j][0].hypot(q[j * n + j][1]) < tiny {
            q[j * n + j] = [tiny, 0.0];
        }
        let piv = q[j * n + j];
        for i in j + 1..n {
            let f = cdiv(q[i * n + j], piv);
            q[i * n + j] = f;
            if f != [0.0, 0.0] {
                for c in j + 1..n {
                    let u = cmul(f, q[j * n + c]);
                    q[i * n + c][0] -= u[0];
                    q[i * n + c][1] -= u[1];
                }
            }
        }
    }
    let mut x: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            [
                1.0 + ((7 * i + 3) % 11) as f64 / 11.0,
                ((5 * i + 1) % 13) as f64 / 13.0 - 0.5,
            ]
        })
        .collect();
    for _ in 0..INVERSE_ITERATIONS {
        let mut y: Vec<[f64; 2]> = perm.iter().map(|&p| x[p]).collect();
        for i in 0..n {
            for c in 0..i {
                let u = cmul(q[i * n + c], y[c]);
                y[i][0] -= u[0];
                y[i][1] -= u[1];
            }
        }
        for i in (0..n).rev() {
            for c in i + 1..n {
                let u = cmul(q[i * n + c], y[c]);
                y[i][0] -= u[0];
                y[i][1] -= u[1];
            }
            y[i] = cdiv(y[i], q[i * n + i]);
        }
        let nrm = y.iter().map(|z| z[0] * z[0] + z[1] * z[1]).sum::<f64>().sqrt();
        x = y.iter().map(|z| [z[0] / nrm, z[1] / nrm]).collect();
    }
    (x.iter().map(|z| z[0]).collect(), x.iter().map(|z| z[1]).collect())
}

/// `phi^H M phi = 1` and the entry of largest modulus real and positive (ADR 0411 D2 step 5).
fn normalise(m: &[f64], n: usize, re: &mut [f64], im: &mut [f64]) {
    let nm = super::dot(re, &dense_matvec(m, n, re)) + super::dot(im, &dense_matvec(m, n, im));
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

/// `||Q(s) phi|| / (||K phi|| + |s|^2 ||M phi|| + |s| ||G phi||)`.
#[allow(clippy::too_many_arguments)]
fn state_residual(k: &[f64], m: &[f64], g: &[f64], n: usize, s: [f64; 2], re: &[f64], im: &[f64]) -> f64 {
    let (kr, ki) = (dense_matvec(k, n, re), dense_matvec(k, n, im));
    let (mr, mi) = (dense_matvec(m, n, re), dense_matvec(m, n, im));
    let (gr, gi) = (dense_matvec(g, n, re), dense_matvec(g, n, im));
    let s2 = cmul(s, s);
    let mut num = 0.0;
    for i in 0..n {
        let sg = cmul(s, [gr[i], gi[i]]);
        let sm = cmul(s2, [mr[i], mi[i]]);
        let a = kr[i] + sg[0] + sm[0];
        let b = ki[i] + sg[1] + sm[1];
        num += a * a + b * b;
    }
    let n2 = |x: &[f64], y: &[f64]| (super::norm2(x) + super::norm2(y)).sqrt();
    let abs = s[0].hypot(s[1]);
    let den = n2(&kr, &ki) + abs * abs * n2(&mr, &mi) + abs * n2(&gr, &gi);
    if den == 0.0 {
        num.sqrt()
    } else {
        num.sqrt() / den
    }
}

// ------------------------------------------------------------------------------------------------------------------------
// D1, D3: the state solve.

/// `(s, re, im, residual)` of one reported mode.
type StateMode = ([f64; 2], Vec<f64>, Vec<f64>, f64);

struct StateOut {
    eigenvalues: Vec<[f64; 2]>,
    stability: Stability,
    real_defect: f64,
    /// `(s, re, im, residual)` of the lowest `wanted` reported modes.
    modes: Vec<StateMode>,
}

fn state_solve(k: &[f64], m: &[f64], g: &[f64], n: usize, wanted: usize) -> Result<StateOut> {
    let lm = cholesky(m, n).ok_or_else(|| FemError::InvalidValue {
        key: "mass",
        reason: "the mass of the whirl problem is not positive definite".to_string(),
    })?;
    // K^ = L^-1 K L^-T and G^ = L^-1 G L^-T: Y = L^-1 (L^-1 X)^T = X^^T, made exactly symmetric resp. skew.
    let scaled = |x: &[f64]| -> Vec<f64> {
        let mut a = x.to_vec();
        lower_solve(&lm, n, &mut a, n);
        let mut y = transpose(&a, n, n);
        lower_solve(&lm, n, &mut y, n);
        y
    };
    let yk = scaled(k);
    let yg = scaled(g);
    let mut kh = vec![0.0; n * n];
    let mut gh = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            kh[i * n + j] = 0.5 * (yk[j * n + i] + yk[i * n + j]);
            gh[i * n + j] = 0.5 * (yg[j * n + i] - yg[i * n + j]);
        }
    }
    let kmax = kh.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
    let gmax = gh.iter().map(|v| v.abs()).fold(0.0f64, f64::max);
    let mut c = (kmax + gmax * gmax).sqrt();
    if !(c.is_finite() && c > 0.0) {
        c = 1.0;
    }
    let mm = 2 * n;
    let mut a = vec![0.0; mm * mm];
    for i in 0..n {
        a[i * mm + n + i] = c;
        for j in 0..n {
            a[(n + i) * mm + j] = -kh[i * n + j] / c;
            a[(n + i) * mm + n + j] = -gh[i * n + j];
        }
    }
    let mut ev = real_eigenvalues(a, mm)?;
    if ev.iter().any(|s| !(s[0].is_finite() && s[1].is_finite())) {
        return Err(FemError::Solver {
            solver: "gyroscopic_state_eigen",
            reason: "a non-finite eigenvalue".to_string(),
        });
    }
    ev.sort_by(|p, q| p[1].total_cmp(&q[1]).then(p[0].total_cmp(&q[0])));
    let smax = ev.iter().map(|s| s[0].hypot(s[1])).fold(0.0f64, f64::max);
    let remax = ev.iter().map(|s| s[0].abs()).fold(0.0f64, f64::max);
    let real_defect = if smax > 0.0 { remax / smax } else { 0.0 };
    let stability = if real_defect <= STABILITY_TOLERANCE {
        Stability::Stable
    } else {
        Stability::Unstable {
            growth: ev.iter().map(|s| s[0]).fold(f64::NEG_INFINITY, f64::max),
        }
    };
    let reported: Vec<[f64; 2]> = ev
        .iter()
        .copied()
        .filter(|s| s[1] > 0.0 || (s[1] == 0.0 && s[0] >= 0.0))
        .collect();
    let mut modes = Vec::new();
    for &s in reported.iter().take(wanted) {
        let (mut re, mut im) = null_vector(k, m, g, n, s);
        normalise(m, n, &mut re, &mut im);
        let residual = state_residual(k, m, g, n, s, &re, &im);
        modes.push((s, re, im, residual));
    }
    Ok(StateOut {
        eigenvalues: ev,
        stability,
        real_defect,
        modes,
    })
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

/// Every eigenvalue of the state matrix of `(s^2 M + s G + K) phi = 0` for dense row-major `n x n` matrices (ADR 0416 D1 to
/// D4): `k` symmetric of any inertia (its symmetric part is used), `m` symmetric positive definite, `g` skew (its skew part
/// is used). The `2 n` eigenvalues, the stability flag and the reported modes (`Im s > 0`, or real with `Re s >= 0`). Refuses
/// more than [`super::MAX_WHIRL_UNKNOWNS`] unknowns (`"unknowns"`), matrices of the wrong length or with non-finite entries
/// (`"matrix"`) and an `m` that is not positive definite (`"mass"`).
pub fn gyroscopic_state_eigen_dense(k: &[f64], m: &[f64], g: &[f64], n: usize) -> Result<DenseStateWhirl> {
    check_size(n)?;
    check_dense(k, n, "K")?;
    check_dense(m, n, "M")?;
    check_dense(g, n, "G")?;
    let out = state_solve(k, m, g, n, n)?;
    let mut r = DenseStateWhirl {
        eigenvalues: out.eigenvalues,
        stability: out.stability,
        real_defect: out.real_defect,
        growth: Vec::new(),
        omegas: Vec::new(),
        re: Vec::new(),
        im: Vec::new(),
        residuals: Vec::new(),
    };
    for (s, re, im, res) in out.modes {
        r.growth.push(s[0]);
        r.omegas.push(s[1]);
        r.re.push(re);
        r.im.push(im);
        r.residuals.push(res);
    }
    Ok(r)
}

// ------------------------------------------------------------------------------------------------------------------------
// The whole part.

impl<'a> RotatingPart<'a> {
    /// The whirl analysis at the speed `omega`, rad/s, by the state matrix (ADR 0416): valid at any speed, also where
    /// `K_eff` is not positive definite (then `loss` is set and the speed may still be stable, gyroscopically stabilised).
    /// `G` is the Coriolis matrix if `options.coriolis` and 0 otherwise (as [`RotatingPart::whirl_modes`]). Refuses a negative
    /// or non-finite speed (`"speed"`), a mode count of 0 or not below the unknowns (`"modes"`) and more than
    /// [`super::MAX_WHIRL_UNKNOWNS`] reduced unknowns (`"unknowns"`).
    pub fn whirl_state(&self, omega: f64) -> Result<StateWhirl> {
        if !(omega.is_finite() && omega >= 0.0) {
            return Err(speed_refused(omega));
        }
        let c = self.coriolis()?;
        self.whirl_state_at(omega, &c)
    }

    /// [`RotatingPart::whirl_state`] over a list of speeds, rad/s (as [`RotatingPart::campbell`]; key `"speeds"`).
    pub fn whirl_stability(&self, speeds: &[f64]) -> Result<WhirlStability> {
        check_speeds(speeds)?;
        let c = self.coriolis()?;
        let mut results = Vec::with_capacity(speeds.len());
        for &w in speeds {
            results.push(self.whirl_state_at(w, &c)?);
        }
        Ok(WhirlStability {
            speeds: speeds.to_vec(),
            results,
        })
    }

    fn whirl_state_at(&self, omega: f64, c: &CsrMatrix) -> Result<StateWhirl> {
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
        let loss = inertia(&kr, &mr)?.map(|negative| LossOfStiffness { speed: omega, negative });
        check_size(nf)?;
        let kd = dense_of(&kr);
        let md = dense_of(&mr);
        let cd = dense_of(&cr);
        let gd: Vec<f64> = if self.options.coriolis {
            cd.iter().map(|v| 2.0 * omega * v).collect()
        } else {
            vec![0.0; nf * nf]
        };
        let out = state_solve(&kd, &md, &gd, nf, opts.modes)?;
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
        let modes = out
            .modes
            .iter()
            .map(|(s, re, im, res)| {
                let kappa = whirl_number(&cd, &md, nf, re, im);
                StateWhirlMode {
                    growth: s[0],
                    omega: s[1],
                    frequency: s[1] / (2.0 * PI),
                    re: expand(re),
                    im: expand(im),
                    whirl: kappa,
                    direction: Whirl::of(kappa),
                    residual: *res,
                }
            })
            .collect();
        Ok(StateWhirl {
            speed: omega,
            loss,
            eigenvalues: out.eigenvalues,
            stability: out.stability,
            real_defect: out.real_defect,
            modes,
            unknowns: nf,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poly_mul(a: &[f64], b: &[f64]) -> Vec<f64> {
        let mut c = vec![0.0; a.len() + b.len() - 1];
        for (i, x) in a.iter().enumerate() {
            for (j, y) in b.iter().enumerate() {
                c[i + j] += x * y;
            }
        }
        c
    }

    /// A2 of ADR 0416: the companion matrix of `(s - 1)(s - 2)(s - 3)(s^2 + 1)(s^2 + 2 s + 5)`.
    #[test]
    fn a2_companion_matrix_roots() {
        // Coefficients highest power first.
        let p = poly_mul(
            &poly_mul(
                &poly_mul(&poly_mul(&[1.0, -1.0], &[1.0, -2.0]), &[1.0, -3.0]),
                &[1.0, 0.0, 1.0],
            ),
            &[1.0, 2.0, 5.0],
        );
        let m = p.len() - 1;
        let mut a = vec![0.0; m * m];
        for j in 0..m {
            a[j] = -p[j + 1] / p[0];
        }
        for i in 1..m {
            a[i * m + i - 1] = 1.0;
        }
        let ev = real_eigenvalues(a, m).unwrap();
        let exact = [
            [1.0, 0.0],
            [2.0, 0.0],
            [3.0, 0.0],
            [0.0, 1.0],
            [0.0, -1.0],
            [-1.0, 2.0],
            [-1.0, -2.0],
        ];
        assert_eq!(ev.len(), 7);
        let mut worst: f64 = 0.0;
        for e in exact {
            let d = ev
                .iter()
                .map(|s| (s[0] - e[0]).hypot(s[1] - e[1]))
                .fold(f64::INFINITY, f64::min);
            worst = worst.max(d / 3.0);
        }
        // The roots are distinct, so each exact root has its own computed one.
        println!("A2: max |s - s_exact| / 3 = {worst:.3e} (limit 1e-9)");
        assert!(worst <= 1e-9, "A2 missed: {worst:e}");
    }
}
