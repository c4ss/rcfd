//! Preconditioners: identity, Jacobi, diagonal incomplete Cholesky (DIC) on
//! LDU matrices, and ILU(0) on CSR matrices.

use crate::matrix::{CsrMatrix, LduMatrix};

/// Approximate inverse application `z = M^-1 r`.
pub trait Preconditioner: Sync {
    /// Apply the preconditioner.
    fn apply(&self, r: &[f64], z: &mut [f64]);
}

/// No preconditioning.
pub struct Identity;

impl Preconditioner for Identity {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        z.copy_from_slice(r);
    }
}

/// Diagonal scaling.
pub struct Jacobi {
    inv: Vec<f64>,
}

impl Jacobi {
    /// Build from a diagonal (zero entries are treated as 1).
    pub fn new(diag: &[f64]) -> Self {
        Jacobi {
            inv: diag.iter().map(|&d| if d != 0.0 { 1.0 / d } else { 1.0 }).collect(),
        }
    }
}

impl Preconditioner for Jacobi {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for ((zi, ri), di) in z.iter_mut().zip(r).zip(&self.inv) {
            *zi = ri * di;
        }
    }
}

/// Diagonal-based incomplete Cholesky (DIC) for symmetric LDU matrices whose
/// faces are ordered by owner then neighbour (as produced by `rcfd-mesh`).
pub struct Ic0 {
    r_d: Vec<f64>,
    lower: Vec<usize>,
    upper_idx: Vec<usize>,
    coeff: Vec<f64>,
}

impl Ic0 {
    /// Factorise a symmetric positive definite matrix.
    pub fn new(m: &LduMatrix) -> Self {
        let mut r_d = m.diag.clone();
        for f in 0..m.upper.len() {
            let (l, u) = (m.addr.lower[f], m.addr.upper[f]);
            r_d[u] -= m.upper[f] * m.upper[f] / r_d[l];
        }
        for d in r_d.iter_mut() {
            *d = 1.0 / *d;
        }
        Ic0 {
            r_d,
            lower: m.addr.lower.clone(),
            upper_idx: m.addr.upper.clone(),
            coeff: m.upper.clone(),
        }
    }
}

impl Preconditioner for Ic0 {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for i in 0..z.len() {
            z[i] = self.r_d[i] * r[i];
        }
        for f in 0..self.coeff.len() {
            let (l, u) = (self.lower[f], self.upper_idx[f]);
            z[u] -= self.r_d[u] * self.coeff[f] * z[l];
        }
        for f in (0..self.coeff.len()).rev() {
            let (l, u) = (self.lower[f], self.upper_idx[f]);
            z[l] -= self.r_d[l] * self.coeff[f] * z[u];
        }
    }
}

/// Incomplete LU with zero fill on a CSR matrix with sorted columns.
pub struct Ilu0 {
    n: usize,
    row_ptr: Vec<usize>,
    col: Vec<usize>,
    lu: Vec<f64>,
    diag_pos: Vec<usize>,
}

impl Ilu0 {
    /// Factorise `a` (every row must contain its diagonal).
    pub fn new(a: &CsrMatrix) -> Self {
        let n = a.n;
        let mut lu = a.val.clone();
        let diag_pos: Vec<usize> = (0..n)
            .map(|i| {
                (a.row_ptr[i]..a.row_ptr[i + 1])
                    .find(|&j| a.col[j] == i)
                    .expect("ILU(0) requires a stored diagonal in every row")
            })
            .collect();
        let mut pos = vec![usize::MAX; n];
        for i in 0..n {
            for j in a.row_ptr[i]..a.row_ptr[i + 1] {
                pos[a.col[j]] = j;
            }
            for jk in a.row_ptr[i]..diag_pos[i] {
                let k = a.col[jk];
                lu[jk] /= lu[diag_pos[k]];
                let lik = lu[jk];
                for jj in diag_pos[k] + 1..a.row_ptr[k + 1] {
                    let p = pos[a.col[jj]];
                    if p != usize::MAX {
                        lu[p] -= lik * lu[jj];
                    }
                }
            }
            for j in a.row_ptr[i]..a.row_ptr[i + 1] {
                pos[a.col[j]] = usize::MAX;
            }
        }
        Ilu0 {
            n,
            row_ptr: a.row_ptr.clone(),
            col: a.col.clone(),
            lu,
            diag_pos,
        }
    }
}

impl Preconditioner for Ilu0 {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for i in 0..self.n {
            let mut s = r[i];
            for j in self.row_ptr[i]..self.diag_pos[i] {
                s -= self.lu[j] * z[self.col[j]];
            }
            z[i] = s;
        }
        for i in (0..self.n).rev() {
            let mut s = z[i];
            for j in self.diag_pos[i] + 1..self.row_ptr[i + 1] {
                s -= self.lu[j] * z[self.col[j]];
            }
            z[i] = s / self.lu[self.diag_pos[i]];
        }
    }
}

/// Coefficients of the Chebyshev polynomial preconditioner of `D^-1 A` (Saad, "Iterative Methods for Sparse Linear
/// Systems", Algorithm 12.1, started from a zero iterate). Shared by the host preconditioner and the device
/// implementation so that both use the very same scalars.
#[derive(Debug, Clone)]
pub struct ChebyshevCoeffs {
    /// Inverse diagonal (a zero diagonal counts as 1).
    pub dinv: Vec<f64>,
    /// Centre of the eigenvalue interval of `D^-1 A`.
    pub theta: f64,
    /// Number of terms (`degree - 1` matrix products per application).
    pub degree: usize,
    /// For step `i + 1`: `(c1, c2)` with `d_{i+1} = c1 d_i + c2 r_{i+1}`.
    pub steps: Vec<(f64, f64)>,
}

impl ChebyshevCoeffs {
    /// Build for a CSR matrix. The interval is `[lmax/30, lmax]` with `lmax` the Gershgorin bound
    /// `max_i sum_j |a_ij| / |a_ii|`. Returns `None` for an empty matrix or a non-finite bound.
    pub fn new(a: &CsrMatrix, degree: usize) -> Option<Self> {
        let degree = degree.max(1);
        let diag = a.diag();
        let dinv: Vec<f64> = diag.iter().map(|&d| if d != 0.0 { 1.0 / d } else { 1.0 }).collect();
        let mut lmax = 0.0f64;
        for i in 0..a.n {
            let s: f64 = (a.row_ptr[i]..a.row_ptr[i + 1]).map(|k| a.val[k].abs()).sum();
            lmax = lmax.max(s * dinv[i].abs());
        }
        if !(lmax.is_finite() && lmax > 0.0) {
            return None;
        }
        let lmin = lmax / 30.0;
        let theta = 0.5 * (lmax + lmin);
        let delta = 0.5 * (lmax - lmin);
        let sigma1 = theta / delta;
        let mut rho = 1.0 / sigma1;
        let mut steps = Vec::with_capacity(degree.saturating_sub(1));
        for _ in 1..degree {
            let rho_new = 1.0 / (2.0 * sigma1 - rho);
            steps.push((rho_new * rho, 2.0 * rho_new / delta));
            rho = rho_new;
        }
        Some(ChebyshevCoeffs {
            dinv,
            theta,
            degree,
            steps,
        })
    }
}

/// Chebyshev polynomial preconditioner of the Jacobi-scaled operator (fixed polynomial, so it is a legal
/// preconditioner for CG on symmetric positive definite matrices). Host twin of the device preconditioner.
pub struct Chebyshev {
    a: CsrMatrix,
    c: ChebyshevCoeffs,
}

impl Chebyshev {
    /// Build with `degree` terms (`degree - 1` matrix products per application); `None` if the matrix has no
    /// usable spectral bound (see [`ChebyshevCoeffs::new`]).
    pub fn new(a: &CsrMatrix, degree: usize) -> Option<Self> {
        Some(Chebyshev {
            c: ChebyshevCoeffs::new(a, degree)?,
            a: a.clone(),
        })
    }
}

impl Preconditioner for Chebyshev {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        let n = self.a.n;
        let c = &self.c;
        let mut rc: Vec<f64> = (0..n).map(|i| c.dinv[i] * r[i]).collect();
        let mut d: Vec<f64> = rc.iter().map(|v| v / c.theta).collect();
        let mut t = vec![0.0; n];
        z.iter_mut().for_each(|v| *v = 0.0);
        for i in 0..c.degree {
            for k in 0..n {
                z[k] += d[k];
            }
            if i + 1 < c.degree {
                self.a.matvec(&d, &mut t);
                let (c1, c2) = c.steps[i];
                for k in 0..n {
                    rc[k] -= c.dinv[k] * t[k];
                    d[k] = c1 * d[k] + c2 * rc[k];
                }
            }
        }
    }
}

/// Refusal of a preconditioner of ADR 0282 (block Jacobi, incomplete Cholesky).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum PrecondError {
    /// The matrix is not square.
    NotSquare {
        /// Rows.
        rows: usize,
        /// Columns.
        cols: usize,
    },
    /// A row has no stored diagonal entry.
    MissingDiagonal {
        /// The row.
        row: usize,
    },
    /// A diagonal entry is not positive.
    NonPositiveDiagonal {
        /// The row.
        row: usize,
        /// Its value.
        value: f64,
    },
    /// A stored value is not finite.
    NonFinite {
        /// Its row.
        row: usize,
        /// Its column.
        col: usize,
    },
    /// The block partition is not `0 = p_0 < p_1 < ... = n`.
    InvalidBlocks {
        /// What is wrong.
        reason: String,
    },
    /// A diagonal block is not positive definite (Cholesky pivot not positive and finite).
    BlockNotPositive {
        /// The block.
        block: usize,
        /// The global row of the failing pivot.
        row: usize,
        /// The pivot.
        pivot: f64,
    },
    /// No shift within the allowed attempts gave positive pivots.
    NoShift {
        /// Attempts made.
        attempts: usize,
        /// The last shift tried.
        last_shift: f64,
        /// Every breakdown, in order.
        breakdowns: Vec<IcBreakdown>,
    },
}

impl std::fmt::Display for PrecondError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrecondError::NotSquare { rows, cols } => write!(f, "the matrix is not square ({rows} x {cols})"),
            PrecondError::MissingDiagonal { row } => write!(f, "row {row} has no stored diagonal entry"),
            PrecondError::NonPositiveDiagonal { row, value } => {
                write!(f, "row {row}: diagonal {value:e} is not positive")
            }
            PrecondError::NonFinite { row, col } => write!(f, "entry ({row}, {col}) is not finite"),
            PrecondError::InvalidBlocks { reason } => write!(f, "invalid block partition: {reason}"),
            PrecondError::BlockNotPositive { block, row, pivot } => write!(
                f,
                "diagonal block {block} is not positive definite (pivot {pivot:e} at row {row})"
            ),
            PrecondError::NoShift {
                attempts,
                last_shift,
                breakdowns,
            } => {
                write!(
                    f,
                    "incomplete Cholesky broke down in all {attempts} attempts (last shift {last_shift:e}"
                )?;
                if let Some(b) = breakdowns.last() {
                    write!(f, ", pivot {:e} at row {}", b.pivot, b.row)?;
                }
                write!(f, ")")
            }
        }
    }
}

impl std::error::Error for PrecondError {}

/// Block Jacobi: the inverse of the diagonal blocks of a CSR matrix, each factored by Cholesky (ADR 0282 D2).
#[derive(Debug, Clone)]
pub struct BlockJacobi {
    block_ptr: Vec<usize>,
    /// Per block, the lower Cholesky factor, dense row-major, at `offset[b]`.
    factor: Vec<f64>,
    offset: Vec<usize>,
}

impl BlockJacobi {
    /// Blocks of `size` consecutive rows (the last one shorter).
    pub fn uniform(a: &CsrMatrix, size: usize) -> Result<Self, PrecondError> {
        if size == 0 {
            return Err(PrecondError::InvalidBlocks {
                reason: "block size 0".to_string(),
            });
        }
        let mut ptr: Vec<usize> = (0..a.n).step_by(size).collect();
        ptr.push(a.n);
        BlockJacobi::new(a, &ptr)
    }

    /// Blocks given by `block_ptr` (`block_ptr[0] = 0`, strictly increasing, last entry `n`).
    pub fn new(a: &CsrMatrix, block_ptr: &[usize]) -> Result<Self, PrecondError> {
        if a.n != a.ncols {
            return Err(PrecondError::NotSquare {
                rows: a.n,
                cols: a.ncols,
            });
        }
        let n = a.n;
        if block_ptr.first() != Some(&0) || block_ptr.last() != Some(&n) || (n > 0 && block_ptr.len() < 2) {
            return Err(PrecondError::InvalidBlocks {
                reason: format!("must start at 0 and end at {n}"),
            });
        }
        if let Some(w) = block_ptr.windows(2).find(|w| w[1] <= w[0]) {
            return Err(PrecondError::InvalidBlocks {
                reason: format!("not strictly increasing at {} -> {}", w[0], w[1]),
            });
        }
        let mut factor = Vec::new();
        let mut offset = Vec::with_capacity(block_ptr.len());
        for (b, w) in block_ptr.windows(2).enumerate() {
            let (lo, hi) = (w[0], w[1]);
            let m = hi - lo;
            let mut c = vec![0.0; m * m];
            for i in lo..hi {
                for k in a.row_ptr[i]..a.row_ptr[i + 1] {
                    let j = a.col[k];
                    if !a.val[k].is_finite() {
                        return Err(PrecondError::NonFinite { row: i, col: j });
                    }
                    if (lo..hi).contains(&j) {
                        c[(i - lo) * m + (j - lo)] = a.val[k];
                    }
                }
            }
            // Cholesky in place on the lower triangle: C C^T = B.
            for i in 0..m {
                for j in 0..=i {
                    let mut s = c[i * m + j];
                    for k in 0..j {
                        s -= c[i * m + k] * c[j * m + k];
                    }
                    if i == j {
                        if !(s > 0.0 && s.is_finite()) {
                            return Err(PrecondError::BlockNotPositive {
                                block: b,
                                row: lo + i,
                                pivot: s,
                            });
                        }
                        c[i * m + i] = s.sqrt();
                    } else {
                        c[i * m + j] = s / c[j * m + j];
                    }
                }
            }
            offset.push(factor.len());
            factor.extend_from_slice(&c);
        }
        Ok(BlockJacobi {
            block_ptr: block_ptr.to_vec(),
            factor,
            offset,
        })
    }

    /// Number of blocks.
    pub fn n_blocks(&self) -> usize {
        self.block_ptr.len().saturating_sub(1)
    }
}

impl Preconditioner for BlockJacobi {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for (b, w) in self.block_ptr.windows(2).enumerate() {
            let (lo, m) = (w[0], w[1] - w[0]);
            let c = &self.factor[self.offset[b]..self.offset[b] + m * m];
            for i in 0..m {
                let mut s = r[lo + i];
                for k in 0..i {
                    s -= c[i * m + k] * z[lo + k];
                }
                z[lo + i] = s / c[i * m + i];
            }
            for i in (0..m).rev() {
                let mut s = z[lo + i];
                for k in i + 1..m {
                    s -= c[k * m + i] * z[lo + k];
                }
                z[lo + i] = s / c[i * m + i];
            }
        }
    }
}

/// Options of the shifted incomplete Cholesky factorisation (ADR 0282 D3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IcOptions {
    /// Shift of the first attempt (the diagonal is `(1 + shift) a_ii`).
    pub initial_shift: f64,
    /// The first non-zero shift after a breakdown; each later breakdown doubles the shift.
    pub shift_step: f64,
    /// A pivot `d_i` is a breakdown unless `d_i > pivot_tolerance (1 + shift) a_ii`.
    pub pivot_tolerance: f64,
    /// Attempts before the factorisation is refused.
    pub max_attempts: usize,
}

impl Default for IcOptions {
    fn default() -> Self {
        IcOptions {
            initial_shift: 0.0,
            shift_step: 1e-3,
            pivot_tolerance: 1e-10,
            max_attempts: 64,
        }
    }
}

/// One failed attempt of the incomplete Cholesky factorisation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IcBreakdown {
    /// The shift of the attempt.
    pub shift: f64,
    /// The row of the failing pivot.
    pub row: usize,
    /// The pivot.
    pub pivot: f64,
    /// The shifted diagonal `(1 + shift) a_ii` of that row.
    pub diagonal: f64,
}

/// Incomplete Cholesky `L D L^T` with zero fill on the lower triangle of a symmetric CSR matrix, with the diagonal shift
/// `A + alpha diag(A)` raised on breakdown (ADR 0282 D3; Meijerink and van der Vorst 1977, Manteuffel 1980).
#[derive(Debug, Clone)]
pub struct IncompleteCholesky {
    n: usize,
    /// Strictly lower part of `L` (unit diagonal not stored), rows with sorted columns.
    row_ptr: Vec<usize>,
    col: Vec<usize>,
    l: Vec<f64>,
    inv_d: Vec<f64>,
    d: Vec<f64>,
    shift: f64,
    attempts: usize,
    breakdowns: Vec<IcBreakdown>,
}

impl IncompleteCholesky {
    /// Factorise; see [`IcOptions`].
    pub fn new(a: &CsrMatrix, options: &IcOptions) -> Result<Self, PrecondError> {
        if a.n != a.ncols {
            return Err(PrecondError::NotSquare {
                rows: a.n,
                cols: a.ncols,
            });
        }
        let n = a.n;
        let mut row_ptr = Vec::with_capacity(n + 1);
        row_ptr.push(0);
        let mut col = Vec::new();
        let mut low = Vec::new();
        let mut diag = vec![0.0; n];
        for i in 0..n {
            let mut has_diag = false;
            for k in a.row_ptr[i]..a.row_ptr[i + 1] {
                let j = a.col[k];
                if !a.val[k].is_finite() {
                    return Err(PrecondError::NonFinite { row: i, col: j });
                }
                if j < i {
                    col.push(j);
                    low.push(a.val[k]);
                } else if j == i {
                    has_diag = true;
                    diag[i] = a.val[k];
                }
            }
            if !has_diag {
                return Err(PrecondError::MissingDiagonal { row: i });
            }
            if diag[i] <= 0.0 {
                return Err(PrecondError::NonPositiveDiagonal { row: i, value: diag[i] });
            }
            row_ptr.push(col.len());
        }
        let mut breakdowns = Vec::new();
        let mut shift = options.initial_shift;
        let mut marker = vec![usize::MAX; n];
        let mut l = vec![0.0; low.len()];
        let mut d = vec![0.0; n];
        for attempt in 0..options.max_attempts {
            match ic_attempt(
                n,
                &row_ptr,
                &col,
                &low,
                &diag,
                shift,
                options.pivot_tolerance,
                &mut marker,
                &mut l,
                &mut d,
            ) {
                Ok(()) => {
                    return Ok(IncompleteCholesky {
                        n,
                        row_ptr,
                        col,
                        l,
                        inv_d: d.iter().map(|v| 1.0 / v).collect(),
                        d,
                        shift,
                        attempts: attempt + 1,
                        breakdowns,
                    })
                }
                Err((row, pivot)) => {
                    breakdowns.push(IcBreakdown {
                        shift,
                        row,
                        pivot,
                        diagonal: (1.0 + shift) * diag[row],
                    });
                    if attempt + 1 < options.max_attempts {
                        shift = (2.0 * shift).max(options.shift_step);
                    }
                }
            }
        }
        Err(PrecondError::NoShift {
            attempts: options.max_attempts,
            last_shift: shift,
            breakdowns,
        })
    }

    /// The shift `alpha` of the factor (`L D L^T ~ A + alpha diag(A)`).
    pub fn shift(&self) -> f64 {
        self.shift
    }

    /// Attempts made (1 when the first one succeeded).
    pub fn attempts(&self) -> usize {
        self.attempts
    }

    /// The failed attempts, in order.
    pub fn breakdowns(&self) -> &[IcBreakdown] {
        &self.breakdowns
    }

    /// The pivots `d_i`.
    pub fn pivots(&self) -> &[f64] {
        &self.d
    }
}

/// One attempt with the given shift; on breakdown the row and the pivot.
#[allow(clippy::too_many_arguments)]
fn ic_attempt(
    n: usize,
    row_ptr: &[usize],
    col: &[usize],
    low: &[f64],
    diag: &[f64],
    shift: f64,
    tol: f64,
    marker: &mut [usize],
    l: &mut [f64],
    d: &mut [f64],
) -> Result<(), (usize, f64)> {
    for i in 0..n {
        let (lo, hi) = (row_ptr[i], row_ptr[i + 1]);
        for p in lo..hi {
            marker[col[p]] = p;
        }
        let mut di = (1.0 + shift) * diag[i];
        let dshift = di;
        for p in lo..hi {
            let k = col[p];
            let mut s = low[p];
            for q in row_ptr[k]..row_ptr[k + 1] {
                let j = col[q];
                let mp = marker[j];
                if mp != usize::MAX && mp < p {
                    s -= l[mp] * d[j] * l[q];
                }
            }
            let lik = s / d[k];
            l[p] = lik;
            di -= lik * lik * d[k];
        }
        for p in lo..hi {
            marker[col[p]] = usize::MAX;
        }
        if di.is_nan() || di <= tol * dshift {
            return Err((i, di));
        }
        d[i] = di;
    }
    Ok(())
}

impl Preconditioner for IncompleteCholesky {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        for i in 0..self.n {
            let mut s = r[i];
            for p in self.row_ptr[i]..self.row_ptr[i + 1] {
                s -= self.l[p] * z[self.col[p]];
            }
            z[i] = s;
        }
        for i in 0..self.n {
            z[i] *= self.inv_d[i];
        }
        for i in (0..self.n).rev() {
            let xi = z[i];
            for p in self.row_ptr[i]..self.row_ptr[i + 1] {
                z[self.col[p]] -= self.l[p] * xi;
            }
        }
    }
}

/// The strictly lower pattern (row pointers, sorted columns) of the incomplete Cholesky factor of `a` with fill up to
/// `level` (ADR 0357 D1): the stored entries of the lower triangle have level 0; eliminating `k` gives `(i, j)`, `i, j >
/// k`, the level `lev(i, k) + lev(k, j) + 1`, the minimum over `k` is kept, and positions of level `<= level` are kept
/// (ILU(p) of Watts 1981, Saad section 10.3.3, on the symmetric pattern). `level = 0` is the stored lower pattern.
pub fn ic_fill_pattern(a: &CsrMatrix, level: usize) -> (Vec<usize>, Vec<usize>) {
    let n = a.n;
    let mut row_ptr = Vec::with_capacity(n + 1);
    row_ptr.push(0);
    let mut col = Vec::new();
    // Per column k: the finished rows j > k with (j, k) in the pattern, ascending j, with their level.
    let mut below: Vec<Vec<(usize, usize)>> = vec![Vec::new(); n];
    let mut lev = vec![usize::MAX; n];
    let mut set = std::collections::BTreeSet::new();
    for i in 0..n {
        for p in a.row_ptr[i]..a.row_ptr[i + 1] {
            let k = a.col[p];
            if k < i {
                lev[k] = 0;
                set.insert(k);
            }
        }
        let mut next = set.first().copied();
        while let Some(k) = next {
            let lk = lev[k];
            for &(j, ljk) in &below[k] {
                let cand = lk + ljk + 1;
                if cand <= level {
                    if lev[j] == usize::MAX {
                        set.insert(j);
                    }
                    lev[j] = lev[j].min(cand);
                }
            }
            next = set.range(k + 1..).next().copied();
        }
        for &k in &set {
            col.push(k);
            below[k].push((i, lev[k]));
            lev[k] = usize::MAX;
        }
        set.clear();
        row_ptr.push(col.len());
    }
    (row_ptr, col)
}

impl IncompleteCholesky {
    /// Incomplete Cholesky with fill up to `level` ([`ic_fill_pattern`]) and the shift strategy of [`IncompleteCholesky::new`]
    /// (ADR 0357 D2). The refusals are those of `new`; `level = 0` gives the factor of `new` bit for bit.
    pub fn with_fill(a: &CsrMatrix, level: usize, options: &IcOptions) -> Result<Self, PrecondError> {
        if a.n != a.ncols {
            return Err(PrecondError::NotSquare {
                rows: a.n,
                cols: a.ncols,
            });
        }
        let n = a.n;
        let mut diag = vec![0.0; n];
        for i in 0..n {
            let mut has_diag = false;
            for k in a.row_ptr[i]..a.row_ptr[i + 1] {
                let j = a.col[k];
                if !a.val[k].is_finite() {
                    return Err(PrecondError::NonFinite { row: i, col: j });
                }
                if j == i {
                    has_diag = true;
                    diag[i] = a.val[k];
                }
            }
            if !has_diag {
                return Err(PrecondError::MissingDiagonal { row: i });
            }
            if diag[i] <= 0.0 {
                return Err(PrecondError::NonPositiveDiagonal { row: i, value: diag[i] });
            }
        }
        let (row_ptr, col) = ic_fill_pattern(a, level);
        // The stored values at the positions of `a`, zero at the fill positions.
        let mut low = vec![0.0; col.len()];
        for i in 0..n {
            let mut p = row_ptr[i];
            for k in a.row_ptr[i]..a.row_ptr[i + 1] {
                let j = a.col[k];
                if j >= i {
                    break;
                }
                while col[p] < j {
                    p += 1;
                }
                low[p] = a.val[k];
            }
        }
        let mut breakdowns = Vec::new();
        let mut shift = options.initial_shift;
        let mut marker = vec![usize::MAX; n];
        let mut l = vec![0.0; low.len()];
        let mut d = vec![0.0; n];
        for attempt in 0..options.max_attempts {
            match ic_attempt(
                n,
                &row_ptr,
                &col,
                &low,
                &diag,
                shift,
                options.pivot_tolerance,
                &mut marker,
                &mut l,
                &mut d,
            ) {
                Ok(()) => {
                    return Ok(IncompleteCholesky {
                        n,
                        row_ptr,
                        col,
                        l,
                        inv_d: d.iter().map(|v| 1.0 / v).collect(),
                        d,
                        shift,
                        attempts: attempt + 1,
                        breakdowns,
                    })
                }
                Err((row, pivot)) => {
                    breakdowns.push(IcBreakdown {
                        shift,
                        row,
                        pivot,
                        diagonal: (1.0 + shift) * diag[row],
                    });
                    if attempt + 1 < options.max_attempts {
                        shift = (2.0 * shift).max(options.shift_step);
                    }
                }
            }
        }
        Err(PrecondError::NoShift {
            attempts: options.max_attempts,
            last_shift: shift,
            breakdowns,
        })
    }

    /// Stored entries of the strictly lower factor.
    pub fn fill_entries(&self) -> usize {
        self.col.len()
    }
}

/// A preconditioner of a symmetrically permuted matrix used for the original one (ADR 0357 D3): with `B = P A P^T`,
/// `B[i][j] = A[order[i]][order[j]]` (as [`CsrMatrix::permute_symmetric`]) and `inner ~ B`, applies `P^T inner^-1 P`.
#[derive(Debug, Clone)]
pub struct Permuted<P> {
    order: Vec<usize>,
    inner: P,
}

impl<P: Preconditioner> Permuted<P> {
    /// Wrap `inner`, a preconditioner of the matrix permuted by `order`.
    pub fn new(order: Vec<usize>, inner: P) -> Self {
        Permuted { order, inner }
    }

    /// The ordering (`order[i]` is the original row at position `i`).
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// The preconditioner of the permuted matrix.
    pub fn inner(&self) -> &P {
        &self.inner
    }
}

impl<P: Preconditioner> Preconditioner for Permuted<P> {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        let rp: Vec<f64> = self.order.iter().map(|&o| r[o]).collect();
        let mut zp = vec![0.0; rp.len()];
        self.inner.apply(&rp, &mut zp);
        for (i, &o) in self.order.iter().enumerate() {
            z[o] = zp[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::poisson_2d;

    #[test]
    fn ilu0_is_exact_for_tridiagonal() {
        // no fill-in in a tridiagonal matrix: ILU(0) == LU
        let m = crate::matrix::LduMatrix {
            addr: std::sync::Arc::new(crate::matrix::LduAddressing {
                n: 5,
                lower: vec![0, 1, 2, 3],
                upper: vec![1, 2, 3, 4],
            }),
            diag: vec![2.0; 5],
            lower: vec![-1.0; 4],
            upper: vec![-1.0; 4],
        };
        let a = m.to_csr();
        let p = Ilu0::new(&a);
        let x = vec![1.0, -2.0, 3.0, 0.5, 4.0];
        let mut b = vec![0.0; 5];
        a.matvec(&x, &mut b);
        let mut z = vec![0.0; 5];
        p.apply(&b, &mut z);
        for (u, v) in x.iter().zip(&z) {
            assert!((u - v).abs() < 1e-12);
        }
        let d = Ic0::new(&m);
        d.apply(&b, &mut z);
        for (u, v) in x.iter().zip(&z) {
            assert!((u - v).abs() < 1e-12);
        }
    }

    #[test]
    fn jacobi_and_identity() {
        let m = poisson_2d(3, 3);
        let j = Jacobi::new(&m.diag);
        let r = vec![1.0; 9];
        let mut z = vec![0.0; 9];
        j.apply(&r, &mut z);
        assert!((z[4] - 1.0 / m.diag[4]).abs() < 1e-15);
        Identity.apply(&r, &mut z);
        assert_eq!(z, r);
    }
}
