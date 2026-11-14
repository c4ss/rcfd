//! Symmetric eigensolvers (F-382, ADR 0283).
//!
//! * [`symmetric_eigen`]: all eigenpairs of a dense symmetric matrix by cyclic Jacobi rotations (ADR 0283 D1), and
//!   [`generalized_eigen_dense`] for a dense pair `(K, M)` with `M` positive definite, by Cholesky reduction (D2).
//! * [`lowest_eigenpairs`]: the lowest eigenpairs of the sparse pencil `K x = lambda M x` (`K` symmetric, `M` symmetric
//!   positive definite) by subspace iteration on the envelope factor of `K + sigma M` ([`crate::skyline`]) with a
//!   Rayleigh-Ritz step (D3 to D7), verified by a Sturm count: [`count_below`] counts the eigenvalues below a value from
//!   the signs of `D` in `K - s M = L D L^T` (Sylvester's law of inertia, D8). The convergence test is the relative
//!   residual of D6 by default, or on request ([`ResidualTest::Backward`]) the normwise backward error of ADR 0354, whose
//!   rounding floor does not grow with `lambda_max / lambda_1`.
//!
//! Everything is sequential with a fixed operation order except the row-parallel [`CsrMatrix::matvec`], whose rows are
//! summed sequentially, so the results are bit-identical for any number of threads.

use crate::matrix::CsrMatrix;
use crate::skyline::{PivotRule, SkylineError, SkylineLdlt, SkylineOptions, SkylineOrdering};
// ADR 0447 (F-431): the sparse L D L^T as the alternative factorisation.
use crate::sparse_chol::{SparseLdlt, SparseOptions, SparseOrdering};
use std::fmt;

/// Cap on the number of Jacobi sweeps (ADR 0283 D1).
const MAX_SWEEPS: usize = 50;
/// Relative gap below which two Ritz values belong to one cluster (D6).
const CLUSTER_GAP: f64 = 1e-6;
/// Smallest accepted pivot ratio `d_i / a_ii` of an automatic shift (D3).
const AUTO_PIVOT_RATIO: f64 = 1e-8;
/// Seed of the pseudo-random start vectors (D4).
const START_SEED: u64 = 0xD1B54A32D192ED03;
/// Relative symmetry tolerance of the sparse inputs (as ADR 0274 D6).
const SYMMETRY_TOLERANCE: f64 = 1e-12;

/// Refusal or failure of an eigensolver. Rows and columns are indices of the input matrices.
#[derive(Debug, Clone, PartialEq)]
pub enum EigenError {
    /// Sizes do not fit.
    DimensionMismatch {
        /// What does not fit.
        what: String,
    },
    /// A stored value is NaN or infinite.
    NonFinite {
        /// "A", "K" or "M".
        matrix: &'static str,
        /// Row.
        row: usize,
        /// Column.
        col: usize,
    },
    /// `a_rc` and `a_cr` differ by more than the tolerance.
    Unsymmetric {
        /// "A", "K" or "M".
        matrix: &'static str,
        /// Row.
        row: usize,
        /// Column.
        col: usize,
    },
    /// The (dense) mass matrix failed its Cholesky pivot test.
    MassNotPositiveDefinite {
        /// Row of the failing pivot.
        row: usize,
        /// The radicand `m_jj - sum_k l_jk^2`.
        pivot: f64,
    },
    /// Jacobi did not converge within the sweep cap.
    DenseNotConverged {
        /// Sweeps made.
        sweeps: usize,
        /// Off-diagonal Frobenius norm left.
        off: f64,
    },
    /// The number of wanted pairs or the subspace size is out of range.
    BadCount {
        /// Wanted pairs.
        requested: usize,
        /// Dimension.
        n: usize,
        /// Subspace size.
        subspace: usize,
    },
    /// Given start vectors are unusable.
    BadStart {
        /// What is wrong.
        reason: String,
    },
    /// `K + sigma M` is not accepted as positive definite.
    ShiftNotPositiveDefinite {
        /// The (last) shift tried.
        shift: f64,
        /// The refusal of the factorisation.
        source: SkylineError,
    },
    /// The iterated subspace lost rank (reduced mass matrix not positive definite).
    RankDeficient {
        /// Iteration at which it happened.
        iteration: usize,
    },
    /// Not converged within the iteration cap.
    NotConverged {
        /// Iterations made.
        iterations: usize,
        /// Tolerance of the residual test.
        tolerance: f64,
        /// Residuals of the wanted pairs at the last iteration, in the measure of [`SubspaceOptions::residual_test`]
        /// (default `||A x - mu M x|| / ||A x||`).
        residuals: Vec<f64>,
    },
    /// The cluster containing the m-th Ritz value fills the subspace.
    ClusterExceedsSubspace {
        /// Wanted pairs.
        m: usize,
        /// Subspace size.
        q: usize,
    },
    /// The Sturm count does not equal the number of converged eigenvalues below the shift.
    SturmMismatch {
        /// Value `s` of the count.
        shift: f64,
        /// Eigenvalues below `s` by the signs of `D`.
        counted: usize,
        /// Converged Ritz values below `s`.
        expected: usize,
    },
    /// The factorisation of `K - s M` for a count was refused.
    CountFailed {
        /// The (last) value `s` tried.
        shift: f64,
        /// The refusal.
        source: SkylineError,
    },
}

impl fmt::Display for EigenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EigenError::DimensionMismatch { what } => write!(f, "dimension mismatch: {what}"),
            EigenError::NonFinite { matrix, row, col } => {
                write!(f, "non-finite entry of {matrix} at row {row}, column {col}")
            }
            EigenError::Unsymmetric { matrix, row, col } => {
                write!(f, "{matrix} is not symmetric at row {row}, column {col}")
            }
            EigenError::MassNotPositiveDefinite { row, pivot } => write!(
                f,
                "mass matrix is not positive definite: Cholesky pivot {pivot:e} at row {row}"
            ),
            EigenError::DenseNotConverged { sweeps, off } => write!(
                f,
                "Jacobi did not converge in {sweeps} sweeps (off-diagonal norm {off:e})"
            ),
            EigenError::BadCount { requested, n, subspace } => write!(
                f,
                "bad count: {requested} eigenpairs with a subspace of {subspace} for dimension {n} \
                 (need 1 <= count < subspace <= n)"
            ),
            EigenError::BadStart { reason } => write!(f, "bad start vectors: {reason}"),
            EigenError::ShiftNotPositiveDefinite { shift, source } => write!(
                f,
                "K + sigma M is not positive definite for sigma = {shift:e}: {source}"
            ),
            EigenError::RankDeficient { iteration } => write!(
                f,
                "the iterated subspace lost rank at iteration {iteration} (reduced mass matrix not positive definite)"
            ),
            EigenError::NotConverged {
                iterations,
                tolerance,
                residuals,
            } => {
                write!(
                    f,
                    "subspace iteration did not converge in {iterations} iterations (tolerance {tolerance:e}); \
                     residuals of the wanted pairs:"
                )?;
                for r in residuals {
                    write!(f, " {r:e}")?;
                }
                Ok(())
            }
            EigenError::ClusterExceedsSubspace { m, q } => write!(
                f,
                "the cluster containing eigenvalue {m} fills the subspace of {q} vectors; increase the subspace size"
            ),
            EigenError::SturmMismatch {
                shift,
                counted,
                expected,
            } => write!(
                f,
                "Sturm count: {counted} eigenvalues below {shift:e}, but {expected} were found: an eigenvalue was missed"
            ),
            EigenError::CountFailed { shift, source } => {
                write!(f, "eigenvalue count at {shift:e} failed: {source}")
            }
        }
    }
}

impl std::error::Error for EigenError {}

/// All eigenpairs of a dense symmetric matrix (or of a dense pencil).
#[derive(Debug, Clone, PartialEq)]
pub struct DenseEigen {
    /// Eigenvalues, ascending.
    pub values: Vec<f64>,
    /// Eigenvectors, `vectors[j]` belongs to `values[j]`; orthonormal (M-orthonormal for a pencil).
    pub vectors: Vec<Vec<f64>>,
    /// Jacobi sweeps made (the last one without a rotation).
    pub sweeps: usize,
}

/// Dot product with four fixed accumulators (as ADR 0274 D3).
#[inline]
fn dot4(a: &[f64], b: &[f64]) -> f64 {
    debug_assert_eq!(a.len(), b.len());
    let mut s = [0.0f64; 4];
    let ca = a.chunks_exact(4);
    let cb = b.chunks_exact(4);
    let (ra, rb) = (ca.remainder(), cb.remainder());
    for (x, y) in ca.zip(cb) {
        s[0] += x[0] * y[0];
        s[1] += x[1] * y[1];
        s[2] += x[2] * y[2];
        s[3] += x[3] * y[3];
    }
    for (x, y) in ra.iter().zip(rb) {
        s[0] += x * y;
    }
    (s[0] + s[1]) + (s[2] + s[3])
}

fn norm(a: &[f64]) -> f64 {
    dot4(a, a).sqrt()
}

/// Make the entry of largest modulus (first such index) positive.
fn normalise_sign(v: &mut [f64]) {
    let mut imax = 0;
    for (i, x) in v.iter().enumerate() {
        if x.abs() > v[imax].abs() {
            imax = i;
        }
    }
    if v.get(imax).is_some_and(|&x| x < 0.0) {
        for x in v.iter_mut() {
            *x = -*x;
        }
    }
}

fn validate_dense(a: &[f64], n: usize, matrix: &'static str) -> Result<(), EigenError> {
    if a.len() != n * n {
        return Err(EigenError::DimensionMismatch {
            what: format!("{matrix} has {} entries, expected {n} x {n}", a.len()),
        });
    }
    if let Some(k) = a.iter().position(|v| !v.is_finite()) {
        return Err(EigenError::NonFinite {
            matrix,
            row: k / n,
            col: k % n,
        });
    }
    let amax = a.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    for r in 0..n {
        for c in 0..n {
            if (a[r * n + c] - a[c * n + r]).abs() > 1e-12 * amax {
                return Err(EigenError::Unsymmetric { matrix, row: r, col: c });
            }
        }
    }
    Ok(())
}

fn symmetrised(a: &[f64], n: usize) -> Vec<f64> {
    let mut s = vec![0.0; n * n];
    for r in 0..n {
        for c in 0..n {
            s[r * n + c] = 0.5 * (a[r * n + c] + a[c * n + r]);
        }
    }
    s
}

/// Cyclic Jacobi on the symmetric row-major `a` (consumed). Returns values ascending and orthonormal vectors.
fn jacobi(mut a: Vec<f64>, n: usize) -> Result<DenseEigen, EigenError> {
    let eps = f64::EPSILON;
    // w[j * n + k] = V[k][j]: row j of w is eigenvector j.
    let mut w = vec![0.0; n * n];
    for j in 0..n {
        w[j * n + j] = 1.0;
    }
    let mut sweeps = 0;
    loop {
        let mut rotations = 0usize;
        for p in 0..n {
            for q in p + 1..n {
                let apq = a[p * n + q];
                let app = a[p * n + p];
                let aqq = a[q * n + q];
                if apq.abs() <= eps * (app.abs() * aqq.abs()).sqrt() {
                    continue;
                }
                rotations += 1;
                let tau = (aqq - app) / (2.0 * apq);
                let t = if tau == 0.0 {
                    1.0
                } else if tau.abs() > 1e150 {
                    1.0 / (2.0 * tau)
                } else {
                    tau.signum() / (tau.abs() + (1.0 + tau * tau).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;
                for k in 0..n {
                    if k == p || k == q {
                        continue;
                    }
                    let akp = a[p * n + k];
                    let akq = a[q * n + k];
                    let bkp = c * akp - s * akq;
                    let bkq = s * akp + c * akq;
                    a[p * n + k] = bkp;
                    a[k * n + p] = bkp;
                    a[q * n + k] = bkq;
                    a[k * n + q] = bkq;
                }
                a[p * n + p] = app - t * apq;
                a[q * n + q] = aqq + t * apq;
                a[p * n + q] = 0.0;
                a[q * n + p] = 0.0;
                let (lo, hi) = w.split_at_mut(q * n);
                let wp = &mut lo[p * n..p * n + n];
                let wq = &mut hi[..n];
                for (vp, vq) in wp.iter_mut().zip(wq.iter_mut()) {
                    let (x, y) = (*vp, *vq);
                    *vp = c * x - s * y;
                    *vq = s * x + c * y;
                }
            }
        }
        sweeps += 1;
        if rotations == 0 {
            break;
        }
        if sweeps >= MAX_SWEEPS {
            let mut off = 0.0;
            for r in 0..n {
                for c in 0..n {
                    if r != c {
                        off += a[r * n + c] * a[r * n + c];
                    }
                }
            }
            return Err(EigenError::DenseNotConverged {
                sweeps,
                off: off.sqrt(),
            });
        }
    }
    let d: Vec<f64> = (0..n).map(|i| a[i * n + i]).collect();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&i, &j| d[i].total_cmp(&d[j]).then(i.cmp(&j)));
    let values = idx.iter().map(|&i| d[i]).collect();
    let vectors = idx
        .iter()
        .map(|&i| {
            let mut v = w[i * n..i * n + n].to_vec();
            normalise_sign(&mut v);
            v
        })
        .collect();
    Ok(DenseEigen {
        values,
        vectors,
        sweeps,
    })
}

/// All eigenpairs of the dense symmetric `n x n` matrix `a` (row-major) by cyclic Jacobi rotations (ADR 0283 D1).
/// Values ascending; vectors orthonormal, each with its entry of largest modulus positive.
pub fn symmetric_eigen(a: &[f64], n: usize) -> Result<DenseEigen, EigenError> {
    validate_dense(a, n, "A")?;
    jacobi(symmetrised(a, n), n)
}

/// `K x = lambda M x` for symmetric `k` and `m`, without validation (D2).
fn generalized_unchecked(k: &[f64], m: &[f64], n: usize) -> Result<DenseEigen, EigenError> {
    let eps = f64::EPSILON;
    // Cholesky M = L L^T, L lower, row-major.
    let mut l = vec![0.0; n * n];
    for j in 0..n {
        let s = m[j * n + j] - dot4(&l[j * n..j * n + j], &l[j * n..j * n + j]);
        if s.is_nan() || s <= (j + 1) as f64 * eps * m[j * n + j].abs() || s.is_infinite() {
            return Err(EigenError::MassNotPositiveDefinite { row: j, pivot: s });
        }
        let ljj = s.sqrt();
        l[j * n + j] = ljj;
        for i in j + 1..n {
            let v = (m[i * n + j] - dot4(&l[i * n..i * n + j], &l[j * n..j * n + j])) / ljj;
            l[i * n + j] = v;
        }
    }
    // W = L^{-1} K (forward substitution on rows), then C = L^{-1} W^T.
    let forward = |b: &[f64]| -> Vec<f64> {
        let mut x = b.to_vec();
        for i in 0..n {
            for kk in 0..i {
                let lik = l[i * n + kk];
                if lik != 0.0 {
                    for c in 0..n {
                        x[i * n + c] -= lik * x[kk * n + c];
                    }
                }
            }
            let lii = l[i * n + i];
            for c in 0..n {
                x[i * n + c] /= lii;
            }
        }
        x
    };
    let wmat = forward(k);
    let mut wt = vec![0.0; n * n];
    for r in 0..n {
        for c in 0..n {
            wt[c * n + r] = wmat[r * n + c];
        }
    }
    let cmat = forward(&wt);
    let mut e = jacobi(symmetrised(&cmat, n), n)?;
    // x = L^{-T} y
    for v in e.vectors.iter_mut() {
        for i in (0..n).rev() {
            let mut s = v[i];
            for kk in i + 1..n {
                s -= l[kk * n + i] * v[kk];
            }
            v[i] = s / l[i * n + i];
        }
        normalise_sign(v);
    }
    Ok(e)
}

/// All eigenpairs of the dense pencil `K x = lambda M x` (`k` symmetric, `m` symmetric positive definite, both
/// row-major `n x n`) by Cholesky reduction and Jacobi (ADR 0283 D2). Vectors are M-orthonormal.
pub fn generalized_eigen_dense(k: &[f64], m: &[f64], n: usize) -> Result<DenseEigen, EigenError> {
    validate_dense(k, n, "K")?;
    validate_dense(m, n, "M")?;
    generalized_unchecked(&symmetrised(k, n), &symmetrised(m, n), n)
}

/// The direct factorisation of the eigensolvers (ADR 0447 D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Factorisation {
    /// The envelope `L D L^T` of ADR 0274 in the options' `ordering` (the default).
    #[default]
    Envelope,
    /// The sparse `L D L^T` after approximate minimum degree of ADR 0447 (the options' `ordering` is not used).
    Sparse,
}

/// A factor of either kind (ADR 0447 D5).
enum Factor {
    Envelope(SkylineLdlt),
    Sparse(SparseLdlt),
}

impl Factor {
    fn solve(&self, b: &[f64]) -> Vec<f64> {
        match self {
            Factor::Envelope(f) => f.solve(b),
            Factor::Sparse(f) => f.solve(b),
        }
    }
    fn pivots(&self) -> &[f64] {
        match self {
            Factor::Envelope(f) => f.pivots(),
            Factor::Sparse(f) => f.pivots(),
        }
    }
    fn order(&self) -> &[usize] {
        match self {
            Factor::Envelope(f) => f.order(),
            Factor::Sparse(f) => f.order(),
        }
    }
    fn negatives(&self) -> usize {
        match self {
            Factor::Envelope(f) => f.inertia().0,
            Factor::Sparse(f) => f.inertia().0,
        }
    }
}

/// The factor of `a` by the chosen factorisation with the pivot rule and symmetry tolerance of `opts`.
fn factor_by(a: &CsrMatrix, opts: &SkylineOptions, factorisation: Factorisation) -> Result<Factor, SkylineError> {
    match factorisation {
        Factorisation::Envelope => SkylineLdlt::factor_with(a, opts).map(Factor::Envelope),
        Factorisation::Sparse => SparseLdlt::factor_with(
            a,
            &SparseOptions {
                ordering: SparseOrdering::MinimumDegree,
                pivots: opts.pivots,
                symmetry_tolerance: opts.symmetry_tolerance,
            },
        )
        .map(Factor::Sparse),
    }
}

/// Shift `sigma` of the factorised matrix `K + sigma M` (ADR 0283 D3).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Shift {
    /// The first of `0, 1e-4 rho, 1e-3 rho, ..., 10 rho` (`rho = sum |k_ii| / sum m_ii`) whose factor is accepted with a
    /// smallest pivot ratio `d_i / a_ii >= 1e-8`.
    #[default]
    Auto,
    /// The given value.
    Value(f64),
}

/// Residual measure of the convergence test of [`lowest_eigenpairs`] (ADR 0283 D6, ADR 0354 D1 to D3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResidualTest {
    /// `||A x - mu M x|| / ||A x||` (ADR 0283 D6). Its rounding floor is about `c eps ||A|| / mu`, which a slender model
    /// with a large `lambda_max / lambda_1` may not reach below the tolerance.
    #[default]
    Relative,
    /// The normwise backward error `||A x - mu M x|| / ((||A||_inf + |mu| ||M||_inf) ||x||)` (ADR 0354 D1, D2): the pair
    /// is exact for a pencil within that relative distance of `(A, M)`. Its floor is about `c eps` whatever the spectrum.
    Backward,
}

/// Options of [`lowest_eigenpairs`].
#[derive(Debug, Clone, PartialEq)]
pub struct SubspaceOptions {
    /// Shift of the factorisation.
    pub shift: Shift,
    /// Residual tolerance `tau` of the convergence test `residual <= tau`, the residual measured by `residual_test`
    /// (default `||A x - mu M x|| / ||A x||`).
    pub tolerance: f64,
    /// Residual measure of the convergence test (default [`ResidualTest::Relative`]).
    pub residual_test: ResidualTest,
    /// Iteration cap.
    pub max_iterations: usize,
    /// Subspace size `q` (`None`: `min(n, max(2 m, m + 8))`).
    pub subspace_size: Option<usize>,
    /// Elimination order of the factorisations.
    pub ordering: SkylineOrdering,
    /// Start vectors (q columns of length n) instead of the default ones.
    pub start: Option<Vec<Vec<f64>>>,
    /// The factorisation of `K + sigma M` and of the Sturm counts (default [`Factorisation::Envelope`]; ADR 0447).
    pub factorisation: Factorisation,
}

impl Default for SubspaceOptions {
    fn default() -> Self {
        SubspaceOptions {
            shift: Shift::Auto,
            tolerance: 1e-10,
            residual_test: ResidualTest::Relative,
            max_iterations: 200,
            subspace_size: None,
            ordering: SkylineOrdering::Rcm,
            start: None,
            factorisation: Factorisation::Envelope,
        }
    }
}

/// Record of the Sturm check of a result (ADR 0283 D8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SturmCheck {
    /// Value `s` of the count.
    pub shift: f64,
    /// Eigenvalues below `s` by the signs of `D`.
    pub counted: usize,
    /// Converged Ritz values below `s` (the wanted pairs and the rest of the last cluster).
    pub expected: usize,
}

/// Lowest eigenpairs of a sparse pencil.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneralizedEigen {
    /// Eigenvalues `lambda`, ascending.
    pub values: Vec<f64>,
    /// M-orthonormal eigenvectors, `vectors[j]` belongs to `values[j]`.
    pub vectors: Vec<Vec<f64>>,
    /// Residuals of the returned pairs in the measure of [`SubspaceOptions::residual_test`] (default `||A x - mu M x|| /
    /// ||A x||`).
    pub residuals: Vec<f64>,
    /// Iterations made.
    pub iterations: usize,
    /// Shift `sigma` of the factorisation.
    pub shift: f64,
    /// Subspace size `q`.
    pub subspace_size: usize,
    /// The Sturm check.
    pub sturm: SturmCheck,
}

fn entry(a: &CsrMatrix, r: usize, c: usize) -> Option<f64> {
    let (s, e) = (a.row_ptr[r], a.row_ptr[r + 1]);
    a.col[s..e].binary_search(&c).ok().map(|k| a.val[s + k])
}

fn validate_sparse(a: &CsrMatrix, matrix: &'static str) -> Result<(), EigenError> {
    if a.ncols != a.n {
        return Err(EigenError::DimensionMismatch {
            what: format!("{matrix} has {} rows and {} columns", a.n, a.ncols),
        });
    }
    for r in 0..a.n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            if !a.val[k].is_finite() {
                return Err(EigenError::NonFinite {
                    matrix,
                    row: r,
                    col: a.col[k],
                });
            }
        }
    }
    let diag = a.diag();
    for r in 0..a.n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = a.col[k];
            if c == r {
                continue;
            }
            let a_cr = entry(a, c, r).unwrap_or(0.0);
            if (a.val[k] - a_cr).abs() > SYMMETRY_TOLERANCE * (diag[r].abs() * diag[c].abs()).sqrt() {
                return Err(EigenError::Unsymmetric { matrix, row: r, col: c });
            }
        }
    }
    Ok(())
}

fn validate_pair(k: &CsrMatrix, m: &CsrMatrix) -> Result<(), EigenError> {
    validate_sparse(k, "K")?;
    validate_sparse(m, "M")?;
    if k.n != m.n {
        return Err(EigenError::DimensionMismatch {
            what: format!("K is {} x {}, M is {} x {}", k.n, k.n, m.n, m.n),
        });
    }
    Ok(())
}

/// `K + s M` (the pattern union; `s = 0` gives `K` itself).
fn combine(k: &CsrMatrix, m: &CsrMatrix, s: f64) -> CsrMatrix {
    if s == 0.0 {
        return k.clone();
    }
    let mut t = Vec::with_capacity(k.nnz() + m.nnz());
    for r in 0..k.n {
        for p in k.row_ptr[r]..k.row_ptr[r + 1] {
            t.push((r, k.col[p], k.val[p]));
        }
    }
    for r in 0..m.n {
        for p in m.row_ptr[r]..m.row_ptr[r + 1] {
            t.push((r, m.col[p], s * m.val[p]));
        }
    }
    CsrMatrix::from_triplets(k.n, t)
}

fn count_unchecked(
    k: &CsrMatrix,
    m: &CsrMatrix,
    s: f64,
    ordering: &SkylineOrdering,
    factorisation: Factorisation,
) -> Result<usize, EigenError> {
    let a = combine(k, m, -s);
    let opts = SkylineOptions {
        ordering: ordering.clone(),
        pivots: PivotRule::NonZero,
        symmetry_tolerance: SYMMETRY_TOLERANCE,
    };
    match factor_by(&a, &opts, factorisation) {
        Ok(f) => Ok(f.negatives()),
        Err(source) => Err(EigenError::CountFailed { shift: s, source }),
    }
}

/// Number of eigenvalues of `K x = lambda M x` below `s`: the number of negative pivots of `K - s M = L D L^T`
/// (Sylvester's law of inertia, ADR 0283 D8). A refused factorisation (`s` numerically at an eigenvalue) is an error.
pub fn count_below(k: &CsrMatrix, m: &CsrMatrix, s: f64, ordering: &SkylineOrdering) -> Result<usize, EigenError> {
    validate_pair(k, m)?;
    count_unchecked(k, m, s, ordering, Factorisation::Envelope)
}

fn factor_shifted(
    k: &CsrMatrix,
    m: &CsrMatrix,
    shift: Shift,
    ordering: &SkylineOrdering,
    factorisation: Factorisation,
) -> Result<(Factor, CsrMatrix, f64), EigenError> {
    let opts = SkylineOptions {
        ordering: ordering.clone(),
        pivots: PivotRule::Positive,
        symmetry_tolerance: SYMMETRY_TOLERANCE,
    };
    match shift {
        Shift::Value(s) => {
            let a = combine(k, m, s);
            match factor_by(&a, &opts, factorisation) {
                Ok(f) => Ok((f, a, s)),
                Err(source) => Err(EigenError::ShiftNotPositiveDefinite { shift: s, source }),
            }
        }
        Shift::Auto => {
            let tk: f64 = k.diag().iter().map(|v| v.abs()).sum();
            let tm: f64 = m.diag().iter().sum();
            let rho = tk / tm;
            let candidates = [0.0, 1e-4 * rho, 1e-3 * rho, 1e-2 * rho, 1e-1 * rho, rho, 10.0 * rho];
            let mut last = None;
            for &s in &candidates {
                let a = combine(k, m, s);
                match factor_by(&a, &opts, factorisation) {
                    Ok(f) => {
                        let ad = a.diag();
                        let mut worst: Option<(usize, f64)> = None;
                        for (i, (&d, &o)) in f.pivots().iter().zip(f.order()).enumerate() {
                            let ratio = d / ad[o];
                            if worst.map_or(true, |(_, w)| ratio < w) {
                                worst = Some((i, ratio));
                            }
                        }
                        match worst {
                            Some((i, ratio)) if ratio < AUTO_PIVOT_RATIO => {
                                let o = f.order()[i];
                                last = Some((
                                    s,
                                    SkylineError::Pivot {
                                        row: o,
                                        position: i,
                                        pivot: f.pivots()[i],
                                        threshold: AUTO_PIVOT_RATIO * ad[o],
                                    },
                                ));
                            }
                            _ => return Ok((f, a, s)),
                        }
                    }
                    Err(e) => last = Some((s, e)),
                }
            }
            let (shift, source) = last.expect("at least one candidate");
            Err(EigenError::ShiftNotPositiveDefinite { shift, source })
        }
    }
}

fn lcg(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s
}

/// `||a||_inf = max_i sum_j |a_ij|`, rows in order (ADR 0354 D2; for a symmetric matrix an upper bound of `||a||_2`).
fn inf_norm(a: &CsrMatrix) -> f64 {
    (0..a.n).fold(0.0f64, |w, r| {
        w.max(
            a.val[a.row_ptr[r]..a.row_ptr[r + 1]]
                .iter()
                .map(|v| v.abs())
                .sum::<f64>(),
        )
    })
}

fn matvec(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

/// The lowest `count` eigenpairs of `K x = lambda M x` (`K` symmetric, `M` symmetric positive definite) by subspace
/// iteration on the factor of `K + sigma M` with a Rayleigh-Ritz step, each pair converged to the residual tolerance,
/// and verified by a Sturm count above the last converged cluster (ADR 0283 D3 to D8).
pub fn lowest_eigenpairs(
    k: &CsrMatrix,
    m: &CsrMatrix,
    count: usize,
    opts: &SubspaceOptions,
) -> Result<GeneralizedEigen, EigenError> {
    validate_pair(k, m)?;
    let n = k.n;
    let q = opts.subspace_size.unwrap_or_else(|| n.min((2 * count).max(count + 8)));
    if count == 0 || count >= n || q <= count || q > n {
        return Err(EigenError::BadCount {
            requested: count,
            n,
            subspace: q,
        });
    }
    let mut x: Vec<Vec<f64>> = match &opts.start {
        Some(cols) => {
            if cols.len() != q {
                return Err(EigenError::BadStart {
                    reason: format!("{} columns for a subspace of {q}", cols.len()),
                });
            }
            if let Some(j) = cols
                .iter()
                .position(|c| c.len() != n || c.iter().any(|v| !v.is_finite()))
            {
                return Err(EigenError::BadStart {
                    reason: format!("column {j} has a wrong length or a non-finite entry"),
                });
            }
            cols.clone()
        }
        None => {
            let mut cols = Vec::with_capacity(q);
            cols.push(matvec(m, &vec![1.0; n]));
            let mut s = START_SEED;
            for _ in 1..q {
                cols.push(
                    (0..n)
                        .map(|_| ((lcg(&mut s) >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0)
                        .collect(),
                );
            }
            cols
        }
    };
    let (fac, a, sigma) = factor_shifted(k, m, opts.shift, &opts.ordering, opts.factorisation)?;
    // ADR 0354 D2: the norms of the backward error, computed only when it is asked for.
    let scale = match opts.residual_test {
        ResidualTest::Relative => None,
        ResidualTest::Backward => Some((inf_norm(&a), inf_norm(m))),
    };
    let mut mx: Vec<Vec<f64>> = x.iter().map(|c| matvec(m, c)).collect();
    let mut last_res = Vec::new();
    for it in 1..=opts.max_iterations {
        // steps 1 to 3: Xbar = A^{-1} M X, columns scaled to unit M-norm
        let mut y = mx;
        let mut xb: Vec<Vec<f64>> = y.iter().map(|c| fac.solve(c)).collect();
        let mut yb: Vec<Vec<f64>> = xb.iter().map(|c| matvec(m, c)).collect();
        for j in 0..q {
            let nm = dot4(&xb[j], &yb[j]);
            if nm.is_nan() || nm <= 0.0 || nm.is_infinite() {
                return Err(EigenError::RankDeficient { iteration: it });
            }
            let inv = 1.0 / nm.sqrt();
            for v in xb[j].iter_mut() {
                *v *= inv;
            }
            for v in yb[j].iter_mut() {
                *v *= inv;
            }
            for v in y[j].iter_mut() {
                *v *= inv;
            }
        }
        // step 4: Rayleigh-Ritz on the reduced pair
        let mut ar = vec![0.0; q * q];
        let mut mr = vec![0.0; q * q];
        for i in 0..q {
            for j in 0..q {
                ar[i * q + j] = dot4(&xb[i], &y[j]);
                mr[i * q + j] = dot4(&xb[i], &yb[j]);
            }
        }
        let red = match generalized_unchecked(&symmetrised(&ar, q), &symmetrised(&mr, q), q) {
            Ok(r) => r,
            Err(EigenError::MassNotPositiveDefinite { .. }) => {
                return Err(EigenError::RankDeficient { iteration: it });
            }
            Err(e) => return Err(e),
        };
        // step 5: X = Xbar Z
        x = red
            .vectors
            .iter()
            .map(|z| {
                let mut col = vec![0.0; n];
                for (xi, &zi) in xb.iter().zip(z) {
                    for (c, v) in col.iter_mut().zip(xi) {
                        *c += zi * v;
                    }
                }
                col
            })
            .collect();
        let mu = red.values;
        // step 6: residuals of the shifted operator
        mx = x.iter().map(|c| matvec(m, c)).collect();
        let res: Vec<f64> = (0..q)
            .map(|j| {
                let ax = matvec(&a, &x[j]);
                let r: Vec<f64> = ax.iter().zip(&mx[j]).map(|(p, v)| p - mu[j] * v).collect();
                match scale {
                    None => norm(&r) / norm(&ax),
                    Some((na, nm)) => norm(&r) / ((na + mu[j].abs() * nm) * norm(&x[j])),
                }
            })
            .collect();
        let mut mp = count;
        while mp < q && mu[mp] - mu[mp - 1] <= CLUSTER_GAP * mu[mp] {
            mp += 1;
        }
        last_res = res[..mp].to_vec();
        if res[..mp].iter().all(|&r| r <= opts.tolerance) {
            if mp == q {
                return Err(EigenError::ClusterExceedsSubspace { m: count, q });
            }
            let mut counted = None;
            let mut fail = None;
            for f in [0.5, 0.25, 0.75] {
                let s = mu[mp - 1] + f * (mu[mp] - mu[mp - 1]) - sigma;
                match count_unchecked(k, m, s, &opts.ordering, opts.factorisation) {
                    Ok(c) => {
                        counted = Some((s, c));
                        break;
                    }
                    Err(e) => fail = Some(e),
                }
            }
            let (s, c) = match counted {
                Some(v) => v,
                None => return Err(fail.expect("a refusal")),
            };
            if c != mp {
                return Err(EigenError::SturmMismatch {
                    shift: s,
                    counted: c,
                    expected: mp,
                });
            }
            let mut vectors: Vec<Vec<f64>> = x.into_iter().take(count).collect();
            for v in vectors.iter_mut() {
                normalise_sign(v);
            }
            return Ok(GeneralizedEigen {
                values: mu[..count].iter().map(|v| v - sigma).collect(),
                vectors,
                residuals: res[..count].to_vec(),
                iterations: it,
                shift: sigma,
                subspace_size: q,
                sturm: SturmCheck {
                    shift: s,
                    counted: c,
                    expected: mp,
                },
            });
        }
    }
    Err(EigenError::NotConverged {
        iterations: opts.max_iterations,
        tolerance: opts.tolerance,
        residuals: last_res,
    })
}

// ------------------------------------------------------------------------------------------------------------ buckling

/// Relative cut below which a Ritz value `theta = 1/lambda` of [`lowest_buckling_factors`] is not a factor (ADR 0390 D5).
const POSITIVE_CUT: f64 = 1e-8;

/// Options of [`lowest_buckling_factors`] (ADR 0390 D5).
#[derive(Debug, Clone, PartialEq)]
pub struct BucklingEigenOptions {
    /// Residual tolerance of each wanted pair.
    pub tolerance: f64,
    /// Residual measure: [`ResidualTest::Relative`] is `||A x - theta K x|| / (|theta| ||K x||)`, [`ResidualTest::Backward`]
    /// is `||A x - theta K x|| / ((||A||_inf + |theta| ||K||_inf) ||x||)`, with `A = -K_sigma`, `theta = 1/lambda`.
    pub residual_test: ResidualTest,
    /// Iteration cap.
    pub max_iterations: usize,
    /// Subspace size `q` (`None`: `min(n, max(2 m, m + 8))`).
    pub subspace_size: Option<usize>,
    /// Elimination order of the factorisations.
    pub ordering: SkylineOrdering,
    /// Start vectors (q columns of length n) instead of the default pseudo-random ones.
    pub start: Option<Vec<Vec<f64>>>,
    /// The factorisation of `K` and of the Sturm counts (default [`Factorisation::Envelope`]; ADR 0447).
    pub factorisation: Factorisation,
}

impl Default for BucklingEigenOptions {
    fn default() -> Self {
        BucklingEigenOptions {
            tolerance: 1e-10,
            residual_test: ResidualTest::Relative,
            max_iterations: 200,
            subspace_size: None,
            ordering: SkylineOrdering::Rcm,
            start: None,
            factorisation: Factorisation::Envelope,
        }
    }
}

/// The lowest positive buckling factors of a sparse pair (ADR 0390 D5).
#[derive(Debug, Clone, PartialEq)]
pub struct BucklingEigen {
    /// Factors `lambda` of `(K + lambda K_sigma) x = 0`, positive, ascending.
    pub values: Vec<f64>,
    /// Vectors, `K`-normalised (`x^T K x = 1`), `vectors[j]` belongs to `values[j]`.
    pub vectors: Vec<Vec<f64>>,
    /// Residuals of the returned pairs in the measure of [`BucklingEigenOptions::residual_test`].
    pub residuals: Vec<f64>,
    /// Iterations made.
    pub iterations: usize,
    /// Subspace size `q`.
    pub subspace_size: usize,
    /// The Sturm check: `counted` is the number of negative pivots of `K + s K_sigma`, i.e. of factors in `(0, s)`.
    pub sturm: SturmCheck,
    /// `Some(S)` when fewer factors than asked were found: every factor below `S` is returned (none at all if `values` is
    /// empty), confirmed by the Sturm count at `S`. `None` when the asked number was found.
    pub exhausted: Option<f64>,
}

fn buckling_count(
    k: &CsrMatrix,
    kg: &CsrMatrix,
    s: f64,
    ordering: &SkylineOrdering,
    factorisation: Factorisation,
) -> Result<usize, EigenError> {
    let a = combine(k, kg, s);
    let opts = SkylineOptions {
        ordering: ordering.clone(),
        pivots: PivotRule::NonZero,
        symmetry_tolerance: SYMMETRY_TOLERANCE,
    };
    match factor_by(&a, &opts, factorisation) {
        Ok(f) => Ok(f.negatives()),
        Err(source) => Err(EigenError::CountFailed { shift: s, source }),
    }
}

/// `sum_i z_i cols_i`.
fn combine_columns(cols: &[Vec<f64>], z: &[f64], n: usize) -> Vec<f64> {
    let mut col = vec![0.0; n];
    for (xi, &zi) in cols.iter().zip(z) {
        for (c, v) in col.iter_mut().zip(xi) {
            *c += zi * v;
        }
    }
    col
}

/// The lowest `count` positive factors `lambda` of `(K + lambda K_sigma) x = 0` (`K` symmetric positive definite,
/// `K_sigma` symmetric, indefinite in general) by subspace iteration on `K^{-1} (-K_sigma)` for `theta = 1/lambda` with a
/// Rayleigh-Ritz step, verified by the Sturm count of `K + s K_sigma` (ADR 0390 D5). Fewer factors than asked (none if
/// `K_sigma` has no negative direction) are returned with [`BucklingEigen::exhausted`] set, not refused.
pub fn lowest_buckling_factors(
    k: &CsrMatrix,
    kg: &CsrMatrix,
    count: usize,
    opts: &BucklingEigenOptions,
) -> Result<BucklingEigen, EigenError> {
    validate_sparse(k, "K")?;
    validate_sparse(kg, "K_sigma")?;
    if k.n != kg.n {
        return Err(EigenError::DimensionMismatch {
            what: format!("K is {} x {}, K_sigma is {} x {}", k.n, k.n, kg.n, kg.n),
        });
    }
    let n = k.n;
    let q = opts.subspace_size.unwrap_or_else(|| n.min((2 * count).max(count + 8)));
    if count == 0 || count >= n || q <= count || q > n {
        return Err(EigenError::BadCount {
            requested: count,
            n,
            subspace: q,
        });
    }
    let x0: Vec<Vec<f64>> = match &opts.start {
        Some(cols) => {
            if cols.len() != q {
                return Err(EigenError::BadStart {
                    reason: format!("{} columns for a subspace of {q}", cols.len()),
                });
            }
            if let Some(j) = cols
                .iter()
                .position(|c| c.len() != n || c.iter().any(|v| !v.is_finite()))
            {
                return Err(EigenError::BadStart {
                    reason: format!("column {j} has a wrong length or a non-finite entry"),
                });
            }
            cols.clone()
        }
        None => {
            let mut s = START_SEED;
            (0..q)
                .map(|_| {
                    (0..n)
                        .map(|_| ((lcg(&mut s) >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0)
                        .collect()
                })
                .collect()
        }
    };
    let fopts = SkylineOptions {
        ordering: opts.ordering.clone(),
        pivots: PivotRule::Positive,
        symmetry_tolerance: SYMMETRY_TOLERANCE,
    };
    let fac = factor_by(k, &fopts, opts.factorisation)
        .map_err(|source| EigenError::ShiftNotPositiveDefinite { shift: 0.0, source })?;
    let scale = match opts.residual_test {
        ResidualTest::Relative => None,
        ResidualTest::Backward => Some((inf_norm(kg), inf_norm(k))),
    };
    // A v with A = -K_sigma.
    let apply_a = |v: &[f64]| -> Vec<f64> { matvec(kg, v).into_iter().map(|t| -t).collect() };
    let mut ax: Vec<Vec<f64>> = x0.iter().map(|c| apply_a(c)).collect();
    let mut last_res = Vec::new();
    for it in 1..=opts.max_iterations {
        // step 1: Xbar = K^{-1} A X, columns scaled to unit K-norm (K Xbar = Y = A X)
        let mut y = ax;
        let mut xb: Vec<Vec<f64>> = y.iter().map(|c| fac.solve(c)).collect();
        for j in 0..q {
            let nk = dot4(&xb[j], &y[j]);
            if nk.is_nan() || nk <= 0.0 || nk.is_infinite() {
                return Err(EigenError::RankDeficient { iteration: it });
            }
            let inv = 1.0 / nk.sqrt();
            for v in xb[j].iter_mut() {
                *v *= inv;
            }
            for v in y[j].iter_mut() {
                *v *= inv;
            }
        }
        let axb: Vec<Vec<f64>> = xb.iter().map(|c| apply_a(c)).collect();
        // step 2: Rayleigh-Ritz on (A_r, K_r)
        let mut ar = vec![0.0; q * q];
        let mut kr = vec![0.0; q * q];
        for i in 0..q {
            for j in 0..q {
                ar[i * q + j] = dot4(&xb[i], &axb[j]);
                kr[i * q + j] = dot4(&xb[i], &y[j]);
            }
        }
        let red = match generalized_unchecked(&symmetrised(&ar, q), &symmetrised(&kr, q), q) {
            Ok(r) => r,
            Err(EigenError::MassNotPositiveDefinite { .. }) => {
                return Err(EigenError::RankDeficient { iteration: it });
            }
            Err(e) => return Err(e),
        };
        // Ritz pairs in descending order of theta (ascending lambda for the positive ones).
        let order: Vec<usize> = (0..q).rev().collect();
        let x: Vec<Vec<f64>> = order
            .iter()
            .map(|&j| combine_columns(&xb, &red.vectors[j], n))
            .collect();
        ax = order
            .iter()
            .map(|&j| combine_columns(&axb, &red.vectors[j], n))
            .collect();
        let theta: Vec<f64> = order.iter().map(|&j| red.values[j]).collect();
        // step 3: residuals
        let kx: Vec<Vec<f64>> = x.iter().map(|c| matvec(k, c)).collect();
        let res: Vec<f64> = (0..q)
            .map(|j| {
                let r: Vec<f64> = ax[j].iter().zip(&kx[j]).map(|(p, v)| p - theta[j] * v).collect();
                match scale {
                    None => norm(&r) / (theta[j].abs() * norm(&kx[j])),
                    Some((na, nk)) => norm(&r) / ((na + theta[j].abs() * nk) * norm(&x[j])),
                }
            })
            .collect();
        // step 4: the positive cut, the wanted set W (with its cluster) and the settled set D
        let tmax = theta.iter().fold(0.0f64, |m, t| m.max(t.abs()));
        if tmax.is_nan() || tmax <= 0.0 || tmax.is_infinite() {
            return Err(EigenError::RankDeficient { iteration: it });
        }
        let cut = POSITIVE_CUT * tmax;
        let positive = theta.iter().take_while(|&&t| t > cut).count();
        let mut mp = count.min(positive);
        while mp > 0 && mp < positive && theta[mp - 1] - theta[mp] <= CLUSTER_GAP * theta[mp - 1] {
            mp += 1;
        }
        let mut by_size: Vec<usize> = (0..q).collect();
        by_size.sort_by(|&i, &j| theta[j].abs().total_cmp(&theta[i].abs()).then(i.cmp(&j)));
        last_res = res[..mp].to_vec();
        let converged = res[..mp].iter().all(|&r| r <= opts.tolerance)
            && by_size[..count].iter().all(|&j| res[j] <= opts.tolerance);
        if !converged {
            continue;
        }
        if mp == q {
            return Err(EigenError::ClusterExceedsSubspace { m: count, q });
        }
        // step 5: the Sturm count of K + s K_sigma
        let bound = 1.0 / cut;
        let exhausted = mp < count;
        let candidates: Vec<f64> = if !exhausted && mp < positive {
            let (l_last, l_next) = (1.0 / theta[mp - 1], 1.0 / theta[mp]);
            [0.5, 0.25, 0.75]
                .iter()
                .map(|f| l_last + f * (l_next - l_last))
                .collect()
        } else {
            vec![bound]
        };
        let mut counted = None;
        let mut fail = None;
        for &s in &candidates {
            match buckling_count(k, kg, s, &opts.ordering, opts.factorisation) {
                Ok(c) => {
                    counted = Some((s, c));
                    break;
                }
                Err(e) => fail = Some(e),
            }
        }
        let (s, c) = match counted {
            Some(v) => v,
            None => return Err(fail.expect("a refusal")),
        };
        if c != mp {
            return Err(EigenError::SturmMismatch {
                shift: s,
                counted: c,
                expected: mp,
            });
        }
        let found = count.min(mp);
        let mut vectors: Vec<Vec<f64>> = x.into_iter().take(found).collect();
        for v in vectors.iter_mut() {
            normalise_sign(v);
        }
        return Ok(BucklingEigen {
            values: theta[..found].iter().map(|t| 1.0 / t).collect(),
            vectors,
            residuals: res[..found].to_vec(),
            iterations: it,
            subspace_size: q,
            sturm: SturmCheck {
                shift: s,
                counted: c,
                expected: mp,
            },
            exhausted: if exhausted { Some(bound) } else { None },
        });
    }
    Err(EigenError::NotConverged {
        iterations: opts.max_iterations,
        tolerance: opts.tolerance,
        residuals: last_res,
    })
}
