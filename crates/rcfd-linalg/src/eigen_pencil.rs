//! Eigenpairs of a real unsymmetric pencil nearest a shift (F-653, ADR 0580 D1).
//!
//! [`nearest_pencil_eigenpairs`]: the `count` eigenpairs of `A x = lambda B x` (`A`, `B` real, square, sparse; neither need be
//! symmetric, `B` may be singular) nearest a real shift `sigma`, by Arnoldi's process on the shift-invert operator `T = (A -
//! sigma B)^-1 B`, whose eigenvalues are `theta = 1/(lambda - sigma)` with the same vectors (ADR 0580 arithmetic 4). The factor
//! of `A - sigma B` is [`crate::skyline_lu::EnvelopeLu`] (reverse Cuthill-McKee, no pivoting); the Arnoldi vectors are
//! orthogonalised by classical Gram-Schmidt applied twice; the Ritz pairs come from [`crate::eigen_general::general_eigen`]
//! on the Hessenberg matrix, and a pair is converged when `|h_(m+1,m)| |e_m^T y| <= tolerance |theta|` (the residual of `T`
//! of its Ritz vector). Every returned pair carries its true residual `||A x - lambda B x|| / (||A x|| + |lambda| ||B x||)`.
//! No restart: the Krylov dimension grows to `max_steps`. Every operation is sequential with a fixed order.

use crate::complex::C64;
use crate::eigen_general::{general_eigen, GeneralEigenError};
use crate::matrix::CsrMatrix;
use crate::skyline::SkylineError;
use crate::skyline_lu::EnvelopeLu;
use std::fmt;

/// An Arnoldi vector whose norm after the orthogonalisation is below this fraction of its norm before is taken as the end of
/// an invariant subspace (ADR 0580 D1).
pub const BREAKDOWN: f64 = 1e-13;

/// Options of [`nearest_pencil_eigenpairs`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct PencilOptions {
    /// The real shift `sigma`: the eigenvalues nearest it are found (default 0).
    pub shift: f64,
    /// A Ritz pair is converged when its estimate `|h_(m+1,m)| |e_m^T y|` is at most `tolerance |theta|` (default 1e-10).
    pub tolerance: f64,
    /// The largest Krylov dimension (default 200; at most the order of the pencil).
    pub max_steps: usize,
    /// The seed of the start vector (SplitMix64, uniform in `(-1, 1)`).
    pub seed: u64,
}

impl Default for PencilOptions {
    fn default() -> Self {
        PencilOptions {
            shift: 0.0,
            tolerance: 1e-10,
            max_steps: 200,
            seed: 0x0580_F653,
        }
    }
}

impl PencilOptions {
    /// The defaults.
    pub fn new() -> PencilOptions {
        PencilOptions::default()
    }
    /// With a shift.
    pub fn shift(mut self, shift: f64) -> PencilOptions {
        self.shift = shift;
        self
    }
    /// With a convergence tolerance.
    pub fn tolerance(mut self, tolerance: f64) -> PencilOptions {
        self.tolerance = tolerance;
        self
    }
    /// With a largest Krylov dimension.
    pub fn max_steps(mut self, steps: usize) -> PencilOptions {
        self.max_steps = steps;
        self
    }
    /// With a seed of the start vector.
    pub fn seed(mut self, seed: u64) -> PencilOptions {
        self.seed = seed;
        self
    }
}

/// Refusal or failure of [`nearest_pencil_eigenpairs`].
#[derive(Debug, Clone, PartialEq)]
pub enum PencilError {
    /// Sizes do not fit.
    DimensionMismatch {
        /// What does not fit.
        what: String,
    },
    /// A stored value is NaN or infinite.
    NonFinite {
        /// "A" or "B".
        matrix: &'static str,
        /// Row.
        row: usize,
        /// Column.
        col: usize,
    },
    /// The number of wanted pairs is zero or not below the order.
    InvalidCount {
        /// The count asked for.
        count: usize,
        /// The order of the pencil.
        n: usize,
    },
    /// The factor of `A - sigma B` failed (a pivot test, for instance a shift at an eigenvalue).
    Factor(SkylineError),
    /// A wanted Ritz value of the shift-invert operator is zero (an infinite eigenvalue).
    InfiniteEigenvalue,
    /// The dense eigensolver of the Hessenberg matrix failed.
    Dense(GeneralEigenError),
    /// Not every wanted pair converged within the steps.
    NotConverged {
        /// The Krylov dimension reached.
        steps: usize,
        /// The largest estimate relative to `|theta|` among the wanted pairs.
        worst: f64,
    },
}

impl fmt::Display for PencilError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PencilError::DimensionMismatch { what } => write!(f, "dimension mismatch: {what}"),
            PencilError::NonFinite { matrix, row, col } => write!(f, "{matrix}[{row}, {col}] is not finite"),
            PencilError::InvalidCount { count, n } => write!(
                f,
                "{count} eigenpairs requested of a pencil of order {n} (at least 1 and fewer than the order)"
            ),
            PencilError::Factor(e) => write!(f, "the factor of A - sigma B failed: {e}"),
            PencilError::InfiniteEigenvalue => {
                write!(f, "a wanted Ritz value of (A - sigma B)^-1 B is zero (an infinite eigenvalue)")
            }
            PencilError::Dense(e) => write!(f, "the eigensolver of the Hessenberg matrix failed: {e}"),
            PencilError::NotConverged { steps, worst } => write!(
                f,
                "the Arnoldi process did not converge in {steps} steps: worst relative estimate {worst:e}"
            ),
        }
    }
}

impl std::error::Error for PencilError {}

/// The eigenpairs of a pencil nearest a shift.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PencilEigen {
    /// Eigenvalues `lambda`, ordered by `|lambda - sigma|` ascending (ties: `Im` descending). A complex pair is returned
    /// whole, so there may be one more than asked.
    pub values: Vec<C64>,
    /// The eigenvectors, unit 2-norm, the entry of largest modulus real and positive.
    pub vectors: Vec<Vec<C64>>,
    /// `||A x - lambda B x|| / (||A x|| + |lambda| ||B x||)` of each pair.
    pub residuals: Vec<f64>,
    /// The convergence estimate `|h_(m+1,m)| |e_m^T y| / |theta|` of each pair.
    pub estimates: Vec<f64>,
    /// The Krylov dimension (solves with the factor).
    pub steps: usize,
}

fn check(m: &CsrMatrix, n: usize, name: &'static str) -> Result<(), PencilError> {
    if m.n != n || m.ncols != n {
        return Err(PencilError::DimensionMismatch {
            what: format!("{name} is {} x {}, the pencil is of order {n}", m.n, m.ncols),
        });
    }
    for r in 0..m.n {
        for k in m.row_ptr[r]..m.row_ptr[r + 1] {
            if !m.val[k].is_finite() {
                return Err(PencilError::NonFinite {
                    matrix: name,
                    row: r,
                    col: m.col[k],
                });
            }
        }
    }
    Ok(())
}

fn splitmix(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    2.0 * ((z >> 11) as f64 / (1u64 << 53) as f64) - 1.0
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// Classical Gram-Schmidt twice of `w` against `basis`; the coefficients are added to `h`.
fn orthogonalise(w: &mut [f64], basis: &[Vec<f64>], h: &mut [f64]) {
    for _ in 0..2 {
        let c: Vec<f64> = basis.iter().map(|v| dot(v, w)).collect();
        for (v, ci) in basis.iter().zip(&c) {
            for (wi, vi) in w.iter_mut().zip(v) {
                *wi -= ci * vi;
            }
        }
        for (hi, ci) in h.iter_mut().zip(&c) {
            *hi += ci;
        }
    }
}

/// A new unit vector orthogonal to `basis` from the seeded generator.
fn fresh(n: usize, state: &mut u64, basis: &[Vec<f64>]) -> Vec<f64> {
    loop {
        let mut v: Vec<f64> = (0..n).map(|_| splitmix(state)).collect();
        let before = norm(&v);
        let mut scratch = vec![0.0; basis.len()];
        orthogonalise(&mut v, basis, &mut scratch);
        let after = norm(&v);
        if after > 1e-3 * before {
            for x in v.iter_mut() {
                *x /= after;
            }
            return v;
        }
    }
}

/// The wanted Ritz pairs of the `m x m` Hessenberg matrix: indices into the dense result, by `|theta|` descending.
fn wanted(values: &[C64], count: usize) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..values.len()).collect();
    idx.sort_by(|&i, &j| {
        values[j]
            .abs()
            .partial_cmp(&values[i].abs())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(values[j].im.partial_cmp(&values[i].im).unwrap_or(std::cmp::Ordering::Equal))
    });
    let mut take: Vec<usize> = idx.iter().take(count.min(idx.len())).copied().collect();
    if let Some(&last) = take.last() {
        let v = values[last];
        if v.im != 0.0 {
            let has = take.iter().any(|&k| values[k].re == v.re && values[k].im == -v.im);
            if !has {
                if let Some(&k) = idx[take.len()..]
                    .iter()
                    .find(|&&k| values[k].re == v.re && values[k].im == -v.im)
                {
                    take.push(k);
                }
            }
        }
    }
    take
}

fn matvec_complex(a: &CsrMatrix, x: &[C64]) -> Vec<C64> {
    (0..a.n)
        .map(|r| {
            let mut s = C64::ZERO;
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                s += x[a.col[k]].scale(a.val[k]);
            }
            s
        })
        .collect()
}

fn cnorm(x: &[C64]) -> f64 {
    x.iter().map(|z| z.norm_sqr()).sum::<f64>().sqrt()
}

/// The `count` eigenpairs of `A x = lambda B x` nearest `options.shift` (ADR 0580 D1).
pub fn nearest_pencil_eigenpairs(
    a: &CsrMatrix,
    b: &CsrMatrix,
    count: usize,
    options: &PencilOptions,
) -> Result<PencilEigen, PencilError> {
    let n = a.n;
    check(a, n, "A")?;
    check(b, n, "B")?;
    if count == 0 || count >= n {
        return Err(PencilError::InvalidCount { count, n });
    }
    let sigma = options.shift;
    if !sigma.is_finite() {
        return Err(PencilError::NonFinite {
            matrix: "shift",
            row: 0,
            col: 0,
        });
    }
    let mut t = Vec::with_capacity(a.nnz() + b.nnz());
    for r in 0..n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            t.push((r, a.col[k], a.val[k]));
        }
        if sigma != 0.0 {
            for k in b.row_ptr[r]..b.row_ptr[r + 1] {
                t.push((r, b.col[k], -sigma * b.val[k]));
            }
        }
    }
    let shifted = CsrMatrix::from_triplets(n, t);
    let lu = EnvelopeLu::factor(&shifted).map_err(PencilError::Factor)?;
    let max_steps = options.max_steps.max(count + 2).min(n);
    let mut state = options.seed;
    let mut basis: Vec<Vec<f64>> = vec![fresh(n, &mut state, &[])];
    // Hessenberg columns: hcols[j] has j + 2 entries (the last is h_(j+1,j)).
    let mut hcols: Vec<Vec<f64>> = Vec::new();
    let mut bv = vec![0.0; n];
    let mut worst = f64::INFINITY;
    loop {
        let j = hcols.len();
        b.matvec(&basis[j], &mut bv);
        let mut w = lu.solve(&bv);
        let before = norm(&w);
        let mut h = vec![0.0; j + 2];
        orthogonalise(&mut w, &basis, &mut h[..j + 1]);
        let after = norm(&w);
        let m = j + 1;
        let next = if after <= BREAKDOWN * before {
            h[j + 1] = 0.0;
            if m < n {
                Some(fresh(n, &mut state, &basis))
            } else {
                None
            }
        } else {
            h[j + 1] = after;
            Some(w.iter().map(|x| x / after).collect())
        };
        hcols.push(h);
        if m >= count + 2 || m == n {
            let mut dense = vec![0.0; m * m];
            for (c, col) in hcols.iter().enumerate() {
                for (r, &v) in col.iter().enumerate().take(m) {
                    dense[r * m + c] = v;
                }
            }
            let eig = general_eigen(&dense, m).map_err(PencilError::Dense)?;
            let set = wanted(&eig.values, count);
            let hlast = if m == n { 0.0 } else { hcols[m - 1][m] };
            let mut est = Vec::with_capacity(set.len());
            for &k in &set {
                let th = eig.values[k].abs();
                if th == 0.0 {
                    return Err(PencilError::InfiniteEigenvalue);
                }
                est.push(hlast.abs() * eig.vectors[k][m - 1].abs() / th);
            }
            worst = est.iter().fold(0.0f64, |x, &y| x.max(y));
            if worst <= options.tolerance || m == n {
                return Ok(finish(a, b, sigma, &basis, &eig.values, &eig.vectors, &set, &est, m));
            }
        }
        if m >= max_steps {
            return Err(PencilError::NotConverged { steps: m, worst });
        }
        match next {
            Some(v) => basis.push(v),
            None => return Err(PencilError::NotConverged { steps: m, worst }),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn finish(
    a: &CsrMatrix,
    b: &CsrMatrix,
    sigma: f64,
    basis: &[Vec<f64>],
    values: &[C64],
    vectors: &[Vec<C64>],
    set: &[usize],
    est: &[f64],
    m: usize,
) -> PencilEigen {
    let n = a.n;
    let mut out: Vec<(C64, Vec<C64>, f64, f64)> = Vec::with_capacity(set.len());
    for (&k, &e) in set.iter().zip(est) {
        let theta = values[k];
        let lambda = C64::from_real(sigma) + C64::ONE / theta;
        let y = &vectors[k];
        let mut x = vec![C64::ZERO; n];
        for (c, v) in basis.iter().take(m).enumerate() {
            let yc = y[c];
            for (xi, vi) in x.iter_mut().zip(v) {
                *xi += yc.scale(*vi);
            }
        }
        let nx = cnorm(&x);
        let mut big = 0;
        for (i, z) in x.iter().enumerate() {
            if z.abs() > x[big].abs() {
                big = i;
            }
        }
        let phase = if x[big].abs() > 0.0 {
            x[big].conj() / C64::from_real(x[big].abs())
        } else {
            C64::ONE
        };
        for z in x.iter_mut() {
            *z = (*z * phase).scale(1.0 / nx);
        }
        x[big].im = 0.0;
        let ax = matvec_complex(a, &x);
        let bx = matvec_complex(b, &x);
        let r: Vec<C64> = ax.iter().zip(&bx).map(|(&p, &q)| p - lambda * q).collect();
        let den = cnorm(&ax) + lambda.abs() * cnorm(&bx);
        let res = if den > 0.0 { cnorm(&r) / den } else { cnorm(&r) };
        out.push((lambda, x, res, e));
    }
    out.sort_by(|p, q| {
        let dp = (p.0 - C64::from_real(sigma)).abs();
        let dq = (q.0 - C64::from_real(sigma)).abs();
        dp.partial_cmp(&dq)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(q.0.im.partial_cmp(&p.0.im).unwrap_or(std::cmp::Ordering::Equal))
    });
    PencilEigen {
        values: out.iter().map(|o| o.0).collect(),
        vectors: out.iter().map(|o| o.1.clone()).collect(),
        residuals: out.iter().map(|o| o.2).collect(),
        estimates: out.iter().map(|o| o.3).collect(),
        steps: m,
    }
}
