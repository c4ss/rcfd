//! Dense general real eigenproblem; quadratic eigenproblem on a subspace. Declared by ADR 0440 (F-408, the scaffold of ADR
//! 0311); filled by ADR 0450 (F-414, design stub ADR 0317).
//!
//! [`general_eigen`]: every eigenvalue and eigenvector of a dense real `n x n` matrix (ADR 0450 D1), from published
//! algorithms (sources named from memory, every step checked by residual tests): balancing by powers of 2 (Parlett and
//! Reinsch 1969, the scaling part), Householder reduction to Hessenberg form (Golub and Van Loan, Matrix Computations,
//! section 7.4), the real Schur form by the Francis implicit double-shift QR iteration with the exceptional shifts of
//! Wilkinson and Reinsch's hqr (Golub and Van Loan section 7.5), real 2 x 2 blocks split by a rotation, and eigenvectors by
//! back-substitution on the quasi-triangular factor (Golub and Van Loan section 7.6, the small-divisor device of hqr2).
//! Eigenvalues whose vectors are numerically parallel (a Jordan block) are returned with a [`Defect`] report, not an error;
//! no convergence within `ITERATIONS_PER_EIGENVALUE n` iterations is [`GeneralEigenError::NotConverged`].
//!
//! [`quadratic_eigen`]: `(lambda^2 M + lambda C + K + i H) x = 0` with `M` symmetric positive definite (ADR 0450 D2), by the
//! Cholesky factor of `M` and the first-order state matrix; a complex stiffness `K + i H` by the real embedding of the
//! complex state matrix.

use crate::complex::C64;
use std::fmt;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// Francis iterations allowed per eigenvalue: the cap is `ITERATIONS_PER_EIGENVALUE n` in total (ADR 0450 D1.3).
pub const ITERATIONS_PER_EIGENVALUE: usize = 30;

/// Two eigenvalues are linked in a [`Defect`] when `1 - |v_j^H v_k| <= DEFECT_COSINE` (ADR 0450 D1.6)...
pub const DEFECT_COSINE: f64 = 1e-6;

/// ...and `|lambda_j - lambda_k| <= DEFECT_GAP ||A||_F`.
pub const DEFECT_GAP: f64 = 1e-3;

/// Refusal or failure of the general eigensolvers.
#[derive(Debug, Clone, PartialEq)]
pub enum GeneralEigenError {
    /// Sizes do not fit.
    DimensionMismatch {
        /// What does not fit.
        what: String,
    },
    /// A stored value is NaN or infinite.
    NonFinite {
        /// "A", "M", "C", "K" or "H".
        matrix: &'static str,
        /// Row.
        row: usize,
        /// Column.
        col: usize,
    },
    /// The Francis iteration did not converge within the cap.
    NotConverged {
        /// Iterations made.
        iterations: usize,
        /// The active window `(first, last)` row of the Hessenberg matrix not yet reduced.
        window: (usize, usize),
        /// The last subdiagonal entry of the window.
        subdiagonal: f64,
    },
    /// The mass matrix failed its Cholesky pivot test.
    MassNotPositiveDefinite {
        /// Row of the failing pivot.
        row: usize,
        /// The radicand.
        pivot: f64,
    },
}

impl fmt::Display for GeneralEigenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeneralEigenError::DimensionMismatch { what } => write!(f, "dimension mismatch: {what}"),
            GeneralEigenError::NonFinite { matrix, row, col } => {
                write!(f, "{matrix}[{row}, {col}] is not finite")
            }
            GeneralEigenError::NotConverged {
                iterations,
                window,
                subdiagonal,
            } => write!(
                f,
                "the QR iteration did not converge in {iterations} iterations: rows {} to {} not reduced, last \
                 subdiagonal {subdiagonal:e}",
                window.0, window.1
            ),
            GeneralEigenError::MassNotPositiveDefinite { row, pivot } => {
                write!(f, "mass matrix not positive definite: pivot {pivot:e} at row {row}")
            }
        }
    }
}

impl std::error::Error for GeneralEigenError {}

/// Options of [`general_eigen_with`] and [`quadratic_eigen_with`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct GeneralEigenOptions {
    /// Balance the matrix first (default `true`).
    pub balance: bool,
    /// Total Francis iterations allowed (`None`: `ITERATIONS_PER_EIGENVALUE n`).
    pub max_iterations: Option<usize>,
}

impl Default for GeneralEigenOptions {
    fn default() -> Self {
        GeneralEigenOptions {
            balance: true,
            max_iterations: None,
        }
    }
}

impl GeneralEigenOptions {
    /// The defaults.
    pub fn new() -> GeneralEigenOptions {
        GeneralEigenOptions::default()
    }
    /// With or without balancing.
    pub fn balance(mut self, on: bool) -> GeneralEigenOptions {
        self.balance = on;
        self
    }
    /// With an iteration cap.
    pub fn max_iterations(mut self, cap: usize) -> GeneralEigenOptions {
        self.max_iterations = Some(cap);
        self
    }
}

/// A group of eigenvalues with numerically parallel eigenvectors (a Jordan block or a near one; ADR 0450 D1.6).
#[derive(Debug, Clone, PartialEq)]
pub struct Defect {
    /// Indices into the values, ascending.
    pub indices: Vec<usize>,
    /// The mean of the group's eigenvalues (the best estimate of a defective eigenvalue).
    pub mean: C64,
    /// The largest distance of a member from the mean.
    pub spread: f64,
}

/// All eigenpairs of a dense real matrix.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GeneralEigen {
    /// Eigenvalues in the order of the real Schur form; a complex pair as `(re + i im, re - i im)`, `im > 0`, adjacent.
    pub values: Vec<C64>,
    /// Unit 2-norm eigenvectors, the entry of largest modulus real and positive; a pair's vectors are conjugate.
    pub vectors: Vec<Vec<C64>>,
    /// `||A v - lambda v|| / ||A||_F` of each pair (0 for the zero matrix).
    pub residuals: Vec<f64>,
    /// The defective groups (empty for a matrix with independent eigenvectors).
    pub defects: Vec<Defect>,
    /// Francis iterations made.
    pub iterations: usize,
    /// `||A||_F`.
    pub norm: f64,
}

/// All eigenvalues and eigenvectors of the dense real `n x n` matrix `a` (row-major) with the default options.
pub fn general_eigen(a: &[f64], n: usize) -> Result<GeneralEigen, GeneralEigenError> {
    general_eigen_with(a, n, &GeneralEigenOptions::default())
}

/// [`general_eigen`] with options.
pub fn general_eigen_with(
    a: &[f64],
    n: usize,
    options: &GeneralEigenOptions,
) -> Result<GeneralEigen, GeneralEigenError> {
    check(a, n, "A")?;
    let norm = frob(a);
    let (values, vectors, iterations) = real_eigen(a.to_vec(), n, options)?;
    let residuals = vectors
        .iter()
        .zip(&values)
        .map(|(v, &l)| {
            let mut r = 0.0;
            for i in 0..n {
                let mut s = C64::ZERO;
                for j in 0..n {
                    s += v[j].scale(a[i * n + j]);
                }
                s -= l * v[i];
                r += s.norm_sqr();
            }
            if norm > 0.0 {
                r.sqrt() / norm
            } else {
                r.sqrt()
            }
        })
        .collect();
    let defects = defects_of(&values, &vectors, Some(norm));
    Ok(GeneralEigen {
        values,
        vectors,
        residuals,
        defects,
        iterations,
        norm,
    })
}

fn check(a: &[f64], n: usize, matrix: &'static str) -> Result<(), GeneralEigenError> {
    if a.len() != n * n {
        return Err(GeneralEigenError::DimensionMismatch {
            what: format!("{matrix} has {} entries, expected {n} x {n}", a.len()),
        });
    }
    if let Some(p) = a.iter().position(|x| !x.is_finite()) {
        return Err(GeneralEigenError::NonFinite {
            matrix,
            row: p / n,
            col: p % n,
        });
    }
    Ok(())
}

fn frob(a: &[f64]) -> f64 {
    a.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// Balancing by powers of 2 (D1.1): `a <- D^-1 a D`; returns `D`.
fn balance(a: &mut [f64], n: usize) -> Vec<f64> {
    let mut d = vec![1.0; n];
    loop {
        let mut changed = false;
        for i in 0..n {
            let (mut c, mut r) = (0.0, 0.0);
            for j in 0..n {
                if j != i {
                    c += a[j * n + i].abs();
                    r += a[i * n + j].abs();
                }
            }
            if c == 0.0 || r == 0.0 {
                continue;
            }
            let s = c + r;
            let mut f = 1.0;
            let mut cc = c;
            while cc < r / 2.0 {
                f *= 2.0;
                cc *= 4.0;
            }
            while cc >= r * 2.0 {
                f /= 2.0;
                cc /= 4.0;
            }
            if (cc + r) / f < 0.95 * s {
                changed = true;
                d[i] *= f;
                for j in 0..n {
                    a[i * n + j] /= f;
                    a[j * n + i] *= f;
                }
            }
        }
        if !changed {
            return d;
        }
    }
}

/// A Householder vector of `x`: `(v, beta)` with `v[0] = 1`, `(I - beta v v^T) x = alpha e1`; `None` if `x = 0`.
fn house<const N: usize>(x: [f64; N]) -> Option<([f64; N], f64)> {
    let nrm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
    if nrm == 0.0 {
        return None;
    }
    let alpha = if x[0] > 0.0 { -nrm } else { nrm };
    let v0 = x[0] - alpha;
    if v0 == 0.0 {
        return None;
    }
    let mut v = [0.0; N];
    v[0] = 1.0;
    for i in 1..N {
        v[i] = x[i] / v0;
    }
    let vv: f64 = v.iter().map(|t| t * t).sum();
    Some((v, 2.0 / vv))
}

/// `H <- P H` on rows `r0..r0+N`, columns `c0..c1`.
fn left<const N: usize>(h: &mut [f64], n: usize, v: &[f64; N], beta: f64, r0: usize, c0: usize, c1: usize) {
    for j in c0..c1 {
        let mut s = 0.0;
        for i in 0..N {
            s += v[i] * h[(r0 + i) * n + j];
        }
        s *= beta;
        for i in 0..N {
            h[(r0 + i) * n + j] -= s * v[i];
        }
    }
}

/// `H <- H P` on columns `c0..c0+N`, rows `r0..r1`.
fn right<const N: usize>(h: &mut [f64], n: usize, v: &[f64; N], beta: f64, c0: usize, r0: usize, r1: usize) {
    for i in r0..r1 {
        let mut s = 0.0;
        for j in 0..N {
            s += h[i * n + c0 + j] * v[j];
        }
        s *= beta;
        for j in 0..N {
            h[i * n + c0 + j] -= s * v[j];
        }
    }
}

/// Householder reduction to Hessenberg form with `Q` accumulated (D1.2): `a <- Q^T a Q`; returns `Q` (row-major).
fn hessenberg(a: &mut [f64], n: usize) -> Vec<f64> {
    let mut q = vec![0.0; n * n];
    for i in 0..n {
        q[i * n + i] = 1.0;
    }
    for k in 0..n.saturating_sub(2) {
        let m = n - k - 1;
        let x: Vec<f64> = (k + 1..n).map(|i| a[i * n + k]).collect();
        let nrm = x.iter().map(|t| t * t).sum::<f64>().sqrt();
        if nrm == 0.0 {
            continue;
        }
        let alpha = if x[0] > 0.0 { -nrm } else { nrm };
        let mut v = x;
        v[0] -= alpha;
        let vv: f64 = v.iter().map(|t| t * t).sum();
        if vv == 0.0 {
            continue;
        }
        let beta = 2.0 / vv;
        // left: rows k+1.., all columns from k
        for j in k..n {
            let mut s = 0.0;
            for i in 0..m {
                s += v[i] * a[(k + 1 + i) * n + j];
            }
            s *= beta;
            for i in 0..m {
                a[(k + 1 + i) * n + j] -= s * v[i];
            }
        }
        // right: all rows, columns k+1..
        for i in 0..n {
            let mut s = 0.0;
            for j in 0..m {
                s += a[i * n + k + 1 + j] * v[j];
            }
            s *= beta;
            for j in 0..m {
                a[i * n + k + 1 + j] -= s * v[j];
            }
            let mut s = 0.0;
            for j in 0..m {
                s += q[i * n + k + 1 + j] * v[j];
            }
            s *= beta;
            for j in 0..m {
                q[i * n + k + 1 + j] -= s * v[j];
            }
        }
        a[(k + 1) * n + k] = alpha;
        for i in k + 2..n {
            a[i * n + k] = 0.0;
        }
    }
    q
}

/// One Francis double-shift step on the window `l..=hi` (`hi - l >= 2`) of the full `h`, accumulated in `z` (D1.3).
#[allow(clippy::too_many_arguments)]
fn francis(h: &mut [f64], z: &mut [f64], n: usize, l: usize, hi: usize, s: f64, t: f64) {
    let at = |h: &[f64], i: usize, j: usize| h[i * n + j];
    let mut x = at(h, l, l) * at(h, l, l) + at(h, l, l + 1) * at(h, l + 1, l) - s * at(h, l, l) + t;
    let mut y = at(h, l + 1, l) * (at(h, l, l) + at(h, l + 1, l + 1) - s);
    let mut w = at(h, l + 1, l) * at(h, l + 2, l + 1);
    for k in l..hi - 1 {
        if let Some((v, beta)) = house([x, y, w]) {
            let q = if k > l { k - 1 } else { l };
            left(h, n, &v, beta, k, q, n);
            let r = (k + 4).min(hi + 1);
            right(h, n, &v, beta, k, 0, r);
            right(z, n, &v, beta, k, 0, n);
            if k > l {
                h[(k + 1) * n + k - 1] = 0.0;
                h[(k + 2) * n + k - 1] = 0.0;
            }
        }
        x = at(h, k + 1, k);
        y = at(h, k + 2, k);
        if k + 3 <= hi {
            w = at(h, k + 3, k);
        }
    }
    if let Some((v, beta)) = house([x, y]) {
        let k = hi - 1;
        left(h, n, &v, beta, k, k - 1, n);
        right(h, n, &v, beta, k, 0, hi + 1);
        right(z, n, &v, beta, k, 0, n);
        h[hi * n + k - 1] = 0.0;
    }
}

/// A rotation of rows and columns `p, p + 1` of `h` (and columns of `z`) by `(cs, sn)`: `h <- G^T h G`, `z <- z G`.
fn rotate(h: &mut [f64], z: &mut [f64], n: usize, p: usize, cs: f64, sn: f64) {
    for j in 0..n {
        let (a, b) = (h[p * n + j], h[(p + 1) * n + j]);
        h[p * n + j] = cs * a + sn * b;
        h[(p + 1) * n + j] = -sn * a + cs * b;
    }
    for i in 0..n {
        let (a, b) = (h[i * n + p], h[i * n + p + 1]);
        h[i * n + p] = cs * a + sn * b;
        h[i * n + p + 1] = -sn * a + cs * b;
        let (a, b) = (z[i * n + p], z[i * n + p + 1]);
        z[i * n + p] = cs * a + sn * b;
        z[i * n + p + 1] = -sn * a + cs * b;
    }
}

/// Splits the deflated 2 x 2 block at `p` when its eigenvalues are real (D1.4).
fn split_block(h: &mut [f64], z: &mut [f64], n: usize, p: usize) {
    let (a, b, c, d) = (
        h[p * n + p],
        h[p * n + p + 1],
        h[(p + 1) * n + p],
        h[(p + 1) * n + p + 1],
    );
    let pp = 0.5 * (a - d);
    let q = pp * pp + b * c;
    if q < 0.0 {
        return;
    }
    let zz = pp + q.sqrt().copysign(if pp == 0.0 { 1.0 } else { pp });
    let lam = d + zz;
    // eigenvector of lam: (lam - d, c) or (b, lam - a)
    let (v1, v2) = ((lam - d, c), (b, lam - a));
    let n1 = v1.0.hypot(v1.1);
    let n2 = v2.0.hypot(v2.1);
    let (vx, vy, nv) = if n1 >= n2 { (v1.0, v1.1, n1) } else { (v2.0, v2.1, n2) };
    if nv == 0.0 {
        h[(p + 1) * n + p] = 0.0;
        return;
    }
    rotate(h, z, n, p, vx / nv, vy / nv);
    h[(p + 1) * n + p] = 0.0;
}

type Eigs = (Vec<C64>, Vec<Vec<C64>>, usize);

/// The eigenpairs of `a` (consumed): balancing, Hessenberg, Schur, vectors.
fn real_eigen(mut a: Vec<f64>, n: usize, options: &GeneralEigenOptions) -> Result<Eigs, GeneralEigenError> {
    if n == 0 {
        return Ok((Vec::new(), Vec::new(), 0));
    }
    let d = if options.balance {
        balance(&mut a, n)
    } else {
        vec![1.0; n]
    };
    let mut z = hessenberg(&mut a, n);
    let h = &mut a;
    let eps = f64::EPSILON;
    let hnorm = frob(h);
    let cap = options.max_iterations.unwrap_or(ITERATIONS_PER_EIGENVALUE * n);
    let mut total = 0usize;
    let mut its = 0usize;
    let mut hi = n - 1;
    loop {
        // find l
        let mut l = hi;
        while l > 0 {
            let mut s = h[(l - 1) * n + l - 1].abs() + h[l * n + l].abs();
            if s == 0.0 {
                s = hnorm;
            }
            if h[l * n + l - 1].abs() <= eps * s {
                h[l * n + l - 1] = 0.0;
                break;
            }
            l -= 1;
        }
        if l == hi {
            its = 0;
            if hi == 0 {
                break;
            }
            hi -= 1;
            continue;
        }
        if l + 1 == hi {
            split_block(h, &mut z, n, l);
            its = 0;
            if l == 0 {
                break;
            }
            hi = l - 1;
            continue;
        }
        if total >= cap {
            return Err(GeneralEigenError::NotConverged {
                iterations: total,
                window: (l, hi),
                subdiagonal: h[hi * n + hi - 1],
            });
        }
        total += 1;
        its += 1;
        let (s, t) = if its == 10 || its == 20 {
            let e = h[hi * n + hi - 1].abs() + h[(hi - 1) * n + hi - 2].abs();
            (1.5 * e, e * e)
        } else {
            let (p, q, r, w) = (
                h[(hi - 1) * n + hi - 1],
                h[(hi - 1) * n + hi],
                h[hi * n + hi - 1],
                h[hi * n + hi],
            );
            (p + w, p * w - q * r)
        };
        francis(h, &mut z, n, l, hi, s, t);
    }
    // eigenvalues from the quasi-triangular T
    let t = &*h;
    let tnorm = frob(t);
    let tiny = if tnorm > 0.0 { eps * tnorm } else { eps };
    let mut values = Vec::with_capacity(n);
    let mut vectors = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        if i + 1 < n && t[(i + 1) * n + i] != 0.0 {
            let (a, b, c, dd) = (
                t[i * n + i],
                t[i * n + i + 1],
                t[(i + 1) * n + i],
                t[(i + 1) * n + i + 1],
            );
            let pp = 0.5 * (a - dd);
            let q = pp * pp + b * c;
            let lam = C64::new(0.5 * (a + dd), (-q).max(0.0).sqrt());
            let v = vector(t, &z, &d, n, i, true, lam, tiny);
            values.push(lam);
            values.push(lam.conj());
            vectors.push(v.clone());
            vectors.push(v.iter().map(|x| x.conj()).collect());
            i += 2;
        } else {
            let lam = C64::from_real(t[i * n + i]);
            values.push(lam);
            vectors.push(vector(t, &z, &d, n, i, false, lam, tiny));
            i += 1;
        }
    }
    Ok((values, vectors, total))
}

/// The eigenvector of `lam` at block `p` (2 x 2 if `pair`) by back-substitution on `t`, then `D Z y`, normalised (D1.5).
#[allow(clippy::too_many_arguments)]
fn vector(t: &[f64], z: &[f64], d: &[f64], n: usize, p: usize, pair: bool, lam: C64, tiny: f64) -> Vec<C64> {
    let mut y = vec![C64::ZERO; n];
    let top = if pair {
        let (a, b, c, dd) = (
            t[p * n + p],
            t[p * n + p + 1],
            t[(p + 1) * n + p],
            t[(p + 1) * n + p + 1],
        );
        let v1 = (C64::from_real(b), lam - C64::from_real(a));
        let v2 = (lam - C64::from_real(dd), C64::from_real(c));
        let (u, w) = if v1.0.norm_sqr() + v1.1.norm_sqr() >= v2.0.norm_sqr() + v2.1.norm_sqr() {
            v1
        } else {
            v2
        };
        y[p] = u;
        y[p + 1] = w;
        p + 1
    } else {
        y[p] = C64::ONE;
        p
    };
    let guard = |x: C64| if x.abs() < tiny { C64::from_real(tiny) } else { x };
    let mut i = p;
    while i > 0 {
        let r = i - 1;
        let rhs = |row: usize, y: &[C64]| {
            let mut s = C64::ZERO;
            for j in row + 1..=top {
                s -= y[j].scale(t[row * n + j]);
            }
            s
        };
        if r > 0 && t[r * n + r - 1] != 0.0 {
            // 2 x 2 block rows r-1, r: [[a - lam, b], [c, d - lam]] [y0; y1] = [f0; f1]
            let q = r - 1;
            // y[r] is still 0, so the block's own term j = r drops out of row q's sum
            let f0 = rhs(q, &y);
            let f1 = rhs(r, &y);
            let m00 = C64::from_real(t[q * n + q]) - lam;
            let m01 = C64::from_real(t[q * n + r]);
            let m10 = C64::from_real(t[r * n + q]);
            let m11 = C64::from_real(t[r * n + r]) - lam;
            // Gaussian elimination with partial pivoting
            let (y0, y1) = if m00.abs() >= m10.abs() {
                let p0 = guard(m00);
                let l = m10 / p0;
                let p1 = guard(m11 - l * m01);
                let y1 = (f1 - l * f0) / p1;
                ((f0 - m01 * y1) / p0, y1)
            } else {
                let p0 = guard(m10);
                let l = m00 / p0;
                let p1 = guard(m01 - l * m11);
                let y1 = (f0 - l * f1) / p1;
                ((f1 - m11 * y1) / p0, y1)
            };
            y[q] = y0;
            y[r] = y1;
            i = q;
        } else {
            let f = rhs(r, &y);
            y[r] = f / guard(C64::from_real(t[r * n + r]) - lam);
            i = r;
        }
        let big = y.iter().map(|x| x.abs()).fold(0.0f64, f64::max);
        if big > 1e100 {
            for x in y.iter_mut() {
                *x = x.scale(1.0 / big);
            }
        }
    }
    let mut v = vec![C64::ZERO; n];
    for (r, vr) in v.iter_mut().enumerate() {
        let mut s = C64::ZERO;
        for j in 0..=top {
            s += y[j].scale(z[r * n + j]);
        }
        *vr = s.scale(d[r]);
    }
    normalise(&mut v);
    v
}

/// Unit 2-norm, the entry of largest modulus real and positive.
fn normalise(v: &mut [C64]) {
    let nrm = v.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
    if nrm == 0.0 {
        return;
    }
    let mut k = 0;
    for (i, x) in v.iter().enumerate() {
        if x.abs() > v[k].abs() {
            k = i;
        }
    }
    let ph = v[k].conj().scale(1.0 / (v[k].abs() * nrm));
    for x in v.iter_mut() {
        *x *= ph;
    }
}

/// The defect report of D1.6 on unit vectors: the gap relative to `norm` (`||A||_F`), or with `None` relative to
/// `max(|lambda_j|, |lambda_k|)` (the quadratic problem, ADR 0450 amendment 1).
fn defects_of(values: &[C64], vectors: &[Vec<C64>], norm: Option<f64>) -> Vec<Defect> {
    let n = values.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    let unit: Vec<Vec<C64>> = vectors
        .iter()
        .map(|v| {
            let nrm = v.iter().map(|x| x.norm_sqr()).sum::<f64>().sqrt();
            if nrm > 0.0 {
                v.iter().map(|x| x.scale(1.0 / nrm)).collect()
            } else {
                v.clone()
            }
        })
        .collect();
    for j in 0..n {
        for k in j + 1..n {
            let scale = norm.unwrap_or_else(|| values[j].abs().max(values[k].abs()));
            if (values[j] - values[k]).abs() > DEFECT_GAP * scale {
                continue;
            }
            let mut s = C64::ZERO;
            for (a, b) in unit[j].iter().zip(&unit[k]) {
                s += a.conj() * *b;
            }
            if 1.0 - s.abs() <= DEFECT_COSINE {
                let (a, b) = (find(&mut parent, j), find(&mut parent, k));
                if a != b {
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = vec![Vec::new(); n];
    for i in 0..n {
        let r = find(&mut parent, i);
        groups[r].push(i);
    }
    groups
        .into_iter()
        .filter(|g| g.len() >= 2)
        .map(|indices| {
            let mut mean = C64::ZERO;
            for &i in &indices {
                mean += values[i];
            }
            mean = mean.scale(1.0 / indices.len() as f64);
            let spread = indices.iter().map(|&i| (values[i] - mean).abs()).fold(0.0, f64::max);
            Defect { indices, mean, spread }
        })
        .collect()
}

/// The eigenpairs of a quadratic eigenproblem (ADR 0450 D2).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct QuadraticEigen {
    /// The `2 n` eigenvalues, in the order of the real Schur form of the state matrix (of its embedding with `H`).
    pub values: Vec<C64>,
    /// The vectors `x` (`n` entries), `x^H M x = 1`, the entry of largest modulus real and positive.
    pub vectors: Vec<Vec<C64>>,
    /// `||Q(lambda) x|| / ((|lambda|^2 ||M|| + |lambda| ||C|| + ||K + i H||) ||x||)`, Frobenius norms.
    pub residuals: Vec<f64>,
    /// The defect report of D1.6 on the state vectors, the gap relative to the roots (ADR 0450 amendment 1).
    pub defects: Vec<Defect>,
    /// Francis iterations made.
    pub iterations: usize,
}

/// `(lambda^2 M + lambda C + K) x = 0` for dense `n x n` row-major matrices, `M` symmetric positive definite.
pub fn quadratic_eigen(m: &[f64], c: &[f64], k: &[f64], n: usize) -> Result<QuadraticEigen, GeneralEigenError> {
    quadratic_eigen_with(m, c, k, None, n, &GeneralEigenOptions::default())
}

/// Cholesky `M = L L^T` (L row-major lower).
fn cholesky(m: &[f64], n: usize) -> Result<Vec<f64>, GeneralEigenError> {
    let eps = f64::EPSILON;
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let mut s = m[j * n + j];
        for p in 0..j {
            s -= l[j * n + p] * l[j * n + p];
        }
        if s.is_nan() || s <= (j + 1) as f64 * eps * m[j * n + j].abs() {
            return Err(GeneralEigenError::MassNotPositiveDefinite { row: j, pivot: s });
        }
        let ljj = s.sqrt();
        l[j * n + j] = ljj;
        for i in j + 1..n {
            let mut v = 0.5 * (m[i * n + j] + m[j * n + i]);
            for p in 0..j {
                v -= l[i * n + p] * l[j * n + p];
            }
            l[i * n + j] = v / ljj;
        }
    }
    Ok(l)
}

/// `L^-1 X L^-T` for a general `X`.
fn congruence(l: &[f64], x: &[f64], n: usize) -> Vec<f64> {
    // W = L^-1 X (forward substitution on each column)
    let fwd = |b: &[f64]| -> Vec<f64> {
        let mut w = b.to_vec();
        for col in 0..n {
            for i in 0..n {
                let mut s = w[i * n + col];
                for p in 0..i {
                    s -= l[i * n + p] * w[p * n + col];
                }
                w[i * n + col] = s / l[i * n + i];
            }
        }
        w
    };
    let w = fwd(x);
    let mut wt = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            wt[j * n + i] = w[i * n + j];
        }
    }
    let u = fwd(&wt);
    let mut out = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            out[j * n + i] = u[i * n + j];
        }
    }
    out
}

/// [`quadratic_eigen`] with a loss stiffness `H` (`K + i H`) and options.
pub fn quadratic_eigen_with(
    m: &[f64],
    c: &[f64],
    k: &[f64],
    h: Option<&[f64]>,
    n: usize,
    options: &GeneralEigenOptions,
) -> Result<QuadraticEigen, GeneralEigenError> {
    check(m, n, "M")?;
    check(c, n, "C")?;
    check(k, n, "K")?;
    if let Some(h) = h {
        check(h, n, "H")?;
    }
    let l = cholesky(m, n)?;
    let kh = congruence(&l, k, n);
    let ch = congruence(&l, c, n);
    let hh = h.map(|h| congruence(&l, h, n));
    let n2 = 2 * n;
    let mut ar = vec![0.0; n2 * n2];
    for i in 0..n {
        ar[i * n2 + n + i] = 1.0;
        for j in 0..n {
            ar[(n + i) * n2 + j] = -kh[i * n + j];
            ar[(n + i) * n2 + n + j] = -ch[i * n + j];
        }
    }
    let (values, states, iterations) = match &hh {
        None => real_eigen(ar, n2, options)?,
        Some(hh) => {
            // B = [[Ar, -Ai], [Ai, Ar]], Ai = [[0, 0], [-H^, 0]]
            let n4 = 2 * n2;
            let mut b = vec![0.0; n4 * n4];
            for i in 0..n2 {
                for j in 0..n2 {
                    b[i * n4 + j] = ar[i * n2 + j];
                    b[(n2 + i) * n4 + n2 + j] = ar[i * n2 + j];
                }
            }
            for i in 0..n {
                for j in 0..n {
                    let ai = -hh[i * n + j];
                    b[(n2 + n + i) * n4 + j] = ai; // Ai block, bottom-left
                    b[(n + i) * n4 + n2 + j] = -ai; // -Ai block, top-right
                }
            }
            let (v, w, it) = real_eigen(b, n4, options)?;
            let mut ranked: Vec<(f64, usize, Vec<C64>)> = w
                .iter()
                .enumerate()
                .map(|(idx, wv)| {
                    let zp: Vec<C64> = (0..n2).map(|i| wv[i] + C64::I * wv[n2 + i]).collect();
                    let zm2: f64 = (0..n2).map(|i| (wv[i] - C64::I * wv[n2 + i]).norm_sqr()).sum();
                    let zp2: f64 = zp.iter().map(|x| x.norm_sqr()).sum();
                    (zp2.sqrt() / (zm2.sqrt() + f64::MIN_POSITIVE), idx, zp)
                })
                .collect();
            ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            ranked.truncate(n2);
            ranked.sort_by_key(|r| r.1);
            let vals = ranked.iter().map(|r| v[r.1]).collect();
            let zs = ranked.into_iter().map(|r| r.2).collect();
            (vals, zs, it)
        }
    };
    let defects = defects_of(&values, &states, None);
    let (nm, nc) = (frob(m), frob(c));
    let nk = (frob(k).powi(2) + h.map_or(0.0, |h| frob(h).powi(2))).sqrt();
    let mut vectors = Vec::with_capacity(n2);
    let mut residuals = Vec::with_capacity(n2);
    for (zs, &lam) in states.iter().zip(&values) {
        // x = L^-T y
        let mut x: Vec<C64> = zs[..n].to_vec();
        for i in (0..n).rev() {
            let mut s = x[i];
            for p in i + 1..n {
                s -= x[p].scale(l[p * n + i]);
            }
            x[i] = s.scale(1.0 / l[i * n + i]);
        }
        // x^H M x = 1, largest entry real positive
        let mut xmx = 0.0;
        for i in 0..n {
            let mut s = C64::ZERO;
            for j in 0..n {
                s += x[j].scale(m[i * n + j]);
            }
            xmx += (x[i].conj() * s).re;
        }
        let mut kmax = 0;
        for i in 0..n {
            if x[i].abs() > x[kmax].abs() {
                kmax = i;
            }
        }
        if xmx > 0.0 && x[kmax].abs() > 0.0 {
            let ph = x[kmax].conj().scale(1.0 / (x[kmax].abs() * xmx.sqrt()));
            for v in x.iter_mut() {
                *v *= ph;
            }
        }
        // residual
        let l2 = lam * lam;
        let mut r2 = 0.0;
        for i in 0..n {
            let mut s = C64::ZERO;
            for j in 0..n {
                let kij = C64::new(k[i * n + j], h.map_or(0.0, |h| h[i * n + j]));
                let q = l2.scale(m[i * n + j]) + lam.scale(c[i * n + j]) + kij;
                s += q * x[j];
            }
            r2 += s.norm_sqr();
        }
        let xn = x.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();
        let den = (lam.norm_sqr() * nm + lam.abs() * nc + nk) * xn;
        residuals.push(if den > 0.0 { r2.sqrt() / den } else { r2.sqrt() });
        vectors.push(x);
    }
    Ok(QuadraticEigen {
        values,
        vectors,
        residuals,
        defects,
        iterations,
    })
}
