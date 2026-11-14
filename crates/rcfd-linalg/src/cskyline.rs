//! Complex symmetric `L D L^T` on the envelope (F-412, ADR 0448; declared by ADR 0440, design stub ADR 0315).
//!
//! The algorithm of [`crate::skyline`] (ADR 0274 D1 to D4) in complex arithmetic without conjugation: `A = L D L^T` with
//! `A = A^T` (not Hermitian), `L` unit lower on the envelope of the matrix after reverse Cuthill-McKee, no pivoting. Every sum
//! has four fixed accumulators in index order, every operation is sequential, so the bits of the factor and of a solution do
//! not depend on the number of threads. For a real matrix every imaginary part is zero and the real parts are those of
//! [`crate::skyline::SkylineLdlt`] bit for bit (ADR 0448 D1).
//!
//! Without pivoting the factor exists when every leading principal submatrix is non-singular: for a strictly diagonally
//! dominant matrix (dominance is kept by every Schur complement) and for `A = K' + i B` with `B` symmetric positive definite
//! (`Im(x^H A_k x) = x^H B_k x > 0`). Anything else is guarded by the pivot test (ADR 0448 D1): `d_i` is refused when
//! `|d_i| <= max(theta_i, tau s_i)` or is not finite, `s_i = |a_ii| + sum_j |u_ij l_ij|`, `theta_i = (w_i + 1) eps s_i` with
//! `w_i` the envelope width of row `i`, and `tau` the option [`ComplexSkylineOptions::min_relative_pivot`] (default 0).

use crate::complex::C64;
use crate::matrix::CsrMatrix;
use crate::reorder::{inverse_permutation, rcm_order};
use crate::skyline::SkylineOrdering;
use std::fmt;

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// A square complex sparse matrix in compressed rows, columns sorted (strictly ascending) within rows.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexCsr {
    /// Rows (and columns).
    pub n: usize,
    /// Row pointers (length `n + 1`).
    pub row_ptr: Vec<usize>,
    /// Column indices.
    pub col: Vec<usize>,
    /// Values.
    pub val: Vec<C64>,
}

impl ComplexCsr {
    /// A matrix from its arrays; the structure is checked (pointer lengths and monotonicity, columns in range and strictly
    /// ascending within each row).
    pub fn new(
        n: usize,
        row_ptr: Vec<usize>,
        col: Vec<usize>,
        val: Vec<C64>,
    ) -> Result<ComplexCsr, ComplexSkylineError> {
        let bad = |reason: String| Err(ComplexSkylineError::Malformed { reason });
        if row_ptr.len() != n + 1 || row_ptr[0] != 0 {
            return bad(format!(
                "row pointers of length {} for {n} rows (must be n + 1, starting at 0)",
                row_ptr.len()
            ));
        }
        if col.len() != val.len() || row_ptr[n] != col.len() {
            return bad(format!(
                "{} columns, {} values, last row pointer {}",
                col.len(),
                val.len(),
                row_ptr[n]
            ));
        }
        for r in 0..n {
            if row_ptr[r + 1] < row_ptr[r] {
                return bad(format!("row pointer of row {r} decreases"));
            }
            for k in row_ptr[r]..row_ptr[r + 1] {
                if col[k] >= n {
                    return bad(format!("column {} of row {r} out of range 0..{n}", col[k]));
                }
                if k > row_ptr[r] && col[k] <= col[k - 1] {
                    return bad(format!(
                        "columns of row {r} not strictly ascending ({} after {})",
                        col[k],
                        col[k - 1]
                    ));
                }
            }
        }
        Ok(ComplexCsr { n, row_ptr, col, val })
    }

    /// A real square matrix with zero imaginary parts (its pattern unchanged).
    pub fn from_real(a: &CsrMatrix) -> ComplexCsr {
        ComplexCsr {
            n: a.n,
            row_ptr: a.row_ptr.clone(),
            col: a.col.clone(),
            val: a.val.iter().map(|&v| C64::from_real(v)).collect(),
        }
    }

    /// `re + i im` on the union of the two patterns (square matrices of the same size, columns sorted within rows).
    pub fn from_parts(re: &CsrMatrix, im: &CsrMatrix) -> Result<ComplexCsr, ComplexSkylineError> {
        if re.n != re.ncols || im.n != im.ncols || re.n != im.n {
            return Err(ComplexSkylineError::Malformed {
                reason: format!(
                    "parts of {} x {} and {} x {} (square and equal sizes needed)",
                    re.n, re.ncols, im.n, im.ncols
                ),
            });
        }
        let n = re.n;
        let mut row_ptr = Vec::with_capacity(n + 1);
        row_ptr.push(0usize);
        let (mut col, mut val) = (Vec::new(), Vec::new());
        for r in 0..n {
            let (mut p, pe) = (re.row_ptr[r], re.row_ptr[r + 1]);
            let (mut q, qe) = (im.row_ptr[r], im.row_ptr[r + 1]);
            while p < pe || q < qe {
                let cp = if p < pe { re.col[p] } else { usize::MAX };
                let cq = if q < qe { im.col[q] } else { usize::MAX };
                if cp == cq {
                    col.push(cp);
                    val.push(C64::new(re.val[p], im.val[q]));
                    p += 1;
                    q += 1;
                } else if cp < cq {
                    col.push(cp);
                    val.push(C64::new(re.val[p], 0.0));
                    p += 1;
                } else {
                    col.push(cq);
                    val.push(C64::new(0.0, im.val[q]));
                    q += 1;
                }
            }
            row_ptr.push(col.len());
        }
        ComplexCsr::new(n, row_ptr, col, val)
    }

    /// `y = A x`, each row summed in column order.
    pub fn matvec(&self, x: &[C64]) -> Vec<C64> {
        assert_eq!(x.len(), self.n, "matvec: vector length");
        (0..self.n)
            .map(|r| {
                let mut s = C64::ZERO;
                for k in self.row_ptr[r]..self.row_ptr[r + 1] {
                    s += self.val[k] * x[self.col[k]];
                }
                s
            })
            .collect()
    }

    /// The stored value `a[r][c]`, if stored.
    pub fn get(&self, r: usize, c: usize) -> Option<C64> {
        let (s, e) = (self.row_ptr[r], self.row_ptr[r + 1]);
        self.col[s..e].binary_search(&c).ok().map(|k| self.val[s + k])
    }

    /// The diagonal (0 where not stored).
    pub fn diag(&self) -> Vec<C64> {
        (0..self.n).map(|r| self.get(r, r).unwrap_or(C64::ZERO)).collect()
    }
}

/// Options of [`ComplexSkylineLdlt::factor_with`].
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexSkylineOptions {
    /// Elimination order (default reverse Cuthill-McKee, the order of [`crate::skyline::SkylineLdlt`]).
    pub ordering: SkylineOrdering,
    /// Relative symmetry tolerance `delta`: `|a_rc - a_cr| <= delta sqrt(|a_rr| |a_cc|)` (no conjugation). Default 1e-12.
    pub symmetry_tolerance: f64,
    /// `tau`: a pivot with `|d_i| <= tau s_i` is refused besides the rounding test (default 0; ADR 0448 D1).
    pub min_relative_pivot: f64,
}

impl Default for ComplexSkylineOptions {
    fn default() -> Self {
        ComplexSkylineOptions {
            ordering: SkylineOrdering::Rcm,
            symmetry_tolerance: 1e-12,
            min_relative_pivot: 0.0,
        }
    }
}

/// Refusal of a complex factorisation. Rows and columns are indices of the input matrix.
#[derive(Debug, Clone, PartialEq)]
pub enum ComplexSkylineError {
    /// The arrays do not describe a square matrix with sorted columns.
    Malformed {
        /// What is wrong.
        reason: String,
    },
    /// A stored value is NaN or infinite.
    NonFinite {
        /// Row.
        row: usize,
        /// Column.
        col: usize,
        /// The value.
        value: C64,
    },
    /// `a_rc` and `a_cr` differ by more than the tolerance (a Hermitian matrix is not complex symmetric).
    Unsymmetric {
        /// Row.
        row: usize,
        /// Column.
        col: usize,
        /// `a_rc`.
        a_rc: C64,
        /// `a_cr` (0 if not stored).
        a_cr: C64,
    },
    /// A pivot failed the test.
    Pivot {
        /// Original index of the row (the equation).
        row: usize,
        /// Its position in the elimination order.
        position: usize,
        /// The computed pivot `d`.
        pivot: C64,
        /// The threshold `max(theta, tau s)` its modulus was compared with.
        threshold: f64,
        /// `s = |a_ii| + sum |u l|`, the scale of the test.
        scale: f64,
    },
    /// A given ordering is not a permutation of `0..n`.
    BadOrdering {
        /// What is wrong.
        reason: String,
    },
}

impl fmt::Display for ComplexSkylineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComplexSkylineError::Malformed { reason } => write!(f, "malformed complex matrix: {reason}"),
            ComplexSkylineError::NonFinite { row, col, value } => {
                write!(f, "non-finite entry {value} at row {row}, column {col}")
            }
            ComplexSkylineError::Unsymmetric { row, col, a_rc, a_cr } => write!(
                f,
                "matrix is not complex symmetric at row {row}, column {col}: a[{row}][{col}] = {a_rc}, a[{col}][{row}] = \
                 {a_cr}"
            ),
            ComplexSkylineError::Pivot {
                row,
                position,
                pivot,
                threshold,
                scale,
            } => write!(
                f,
                "pivot {pivot} of row {row} (elimination position {position}), modulus {:e}, is at most the threshold \
                 {threshold:e} (scale {scale:e}): the matrix is numerically singular there without pivoting",
                pivot.abs()
            ),
            ComplexSkylineError::BadOrdering { reason } => write!(f, "bad ordering: {reason}"),
        }
    }
}

impl std::error::Error for ComplexSkylineError {}

/// Envelope `L D L^T` factor of a complex symmetric matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexSkylineLdlt {
    n: usize,
    order: Vec<usize>,
    first: Vec<usize>,
    ptr: Vec<usize>,
    env: Vec<C64>,
    d: Vec<C64>,
}

/// Complex dot product without conjugation with four fixed accumulators: blocks of four in index order, the tail into the
/// first, combined as `(s0 + s1) + (s2 + s3)` (the order of the real `dot4` of ADR 0274 D3).
#[inline]
fn cdot4(a: &[C64], b: &[C64]) -> C64 {
    debug_assert_eq!(a.len(), b.len());
    let mut s = [C64::ZERO; 4];
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
        s[0] += *x * *y;
    }
    (s[0] + s[1]) + (s[2] + s[3])
}

fn first_columns(a: &ComplexCsr, inv: &[usize]) -> Vec<usize> {
    let mut first: Vec<usize> = (0..a.n).collect();
    for r in 0..a.n {
        let ir = inv[r];
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            let ic = inv[a.col[k]];
            let (hi, lo) = if ir >= ic { (ir, ic) } else { (ic, ir) };
            if lo < first[hi] {
                first[hi] = lo;
            }
        }
    }
    first
}

fn envelope_of(a: &ComplexCsr, order: &[usize]) -> usize {
    let inv = inverse_permutation(order);
    first_columns(a, &inv).iter().enumerate().map(|(i, &f)| i - f).sum()
}

/// Reverse Cuthill-McKee of the pattern, built as in [`crate::skyline`] (the same order for the same pattern).
fn rcm_of(a: &ComplexCsr) -> Vec<usize> {
    let (mut lower, mut upper) = (Vec::new(), Vec::new());
    for r in 0..a.n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = a.col[k];
            if c < r {
                lower.push(c);
                upper.push(r);
            } else if c > r {
                lower.push(r);
                upper.push(c);
            }
        }
    }
    rcm_order(a.n, &lower, &upper)
}

fn validate(a: &ComplexCsr, delta: f64) -> Result<(), ComplexSkylineError> {
    for r in 0..a.n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            if !a.val[k].is_finite() {
                return Err(ComplexSkylineError::NonFinite {
                    row: r,
                    col: a.col[k],
                    value: a.val[k],
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
            let a_rc = a.val[k];
            let a_cr = a.get(c, r).unwrap_or(C64::ZERO);
            if (a_rc - a_cr).abs() > delta * (diag[r].abs() * diag[c].abs()).sqrt() {
                return Err(ComplexSkylineError::Unsymmetric {
                    row: r,
                    col: c,
                    a_rc,
                    a_cr,
                });
            }
        }
    }
    Ok(())
}

fn check_permutation(order: &[usize], n: usize) -> Result<(), ComplexSkylineError> {
    let bad = |reason: String| Err(ComplexSkylineError::BadOrdering { reason });
    if order.len() != n {
        return bad(format!("length {} for {n} unknowns", order.len()));
    }
    let mut seen = vec![false; n];
    for (i, &o) in order.iter().enumerate() {
        if o >= n {
            return bad(format!("entry {i} is {o}, out of range 0..{n}"));
        }
        if seen[o] {
            return bad(format!("index {o} repeated (at position {i})"));
        }
        seen[o] = true;
    }
    Ok(())
}

impl ComplexSkylineLdlt {
    /// Factorise with the default options (reverse Cuthill-McKee, symmetry tolerance 1e-12, rounding pivot test only).
    pub fn factor(a: &ComplexCsr) -> Result<ComplexSkylineLdlt, ComplexSkylineError> {
        Self::factor_with(a, &ComplexSkylineOptions::default())
    }

    /// Factorise `a` with the given options. The matrix is validated first (finite, complex symmetric within the
    /// tolerance); the lower triangle is used.
    pub fn factor_with(
        a: &ComplexCsr,
        opts: &ComplexSkylineOptions,
    ) -> Result<ComplexSkylineLdlt, ComplexSkylineError> {
        validate(a, opts.symmetry_tolerance)?;
        let tau = opts.min_relative_pivot;
        if !(tau.is_finite() && tau >= 0.0) {
            return Err(ComplexSkylineError::Malformed {
                reason: format!("minimum relative pivot {tau:e} (must be finite and >= 0)"),
            });
        }
        let n = a.n;
        let order = match &opts.ordering {
            SkylineOrdering::Rcm => rcm_of(a),
            SkylineOrdering::Natural => (0..n).collect(),
            SkylineOrdering::Given(o) => {
                check_permutation(o, n)?;
                o.clone()
            }
            SkylineOrdering::SmallerEnvelope => {
                let rcm = rcm_of(a);
                let natural: Vec<usize> = (0..n).collect();
                if envelope_of(a, &rcm) <= envelope_of(a, &natural) {
                    rcm
                } else {
                    natural
                }
            }
        };
        let inv = inverse_permutation(&order);
        let first = first_columns(a, &inv);
        let mut ptr = Vec::with_capacity(n + 1);
        ptr.push(0usize);
        for (i, &f) in first.iter().enumerate() {
            ptr.push(ptr[i] + (i - f));
        }
        let mut env = vec![C64::ZERO; ptr[n]];
        let mut d = vec![C64::ZERO; n];
        for r in 0..n {
            let i = inv[r];
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                let j = inv[a.col[k]];
                if j < i {
                    env[ptr[i] + j - first[i]] = a.val[k];
                } else if j == i {
                    d[i] = a.val[k];
                }
            }
        }
        for r in 0..n {
            let i = inv[r];
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                let j = inv[a.col[k]];
                if j > i && a.get(a.col[k], r).is_none() {
                    env[ptr[j] + i - first[j]] = a.val[k];
                }
            }
        }
        for i in 0..n {
            let fi = first[i];
            let (done, rest) = env.split_at_mut(ptr[i]);
            let row = &mut rest[..i - fi];
            // u_ij = a_ij - sum_{k=lo}^{j-1} u_ik l_jk
            for j in fi..i {
                let fj = first[j];
                let lo = fi.max(fj);
                if lo < j {
                    let lj = &done[ptr[j] + lo - fj..ptr[j] + j - fj];
                    let s = cdot4(&row[lo - fi..j - fi], lj);
                    row[j - fi] -= s;
                }
            }
            // l_ij = u_ij / d_j, t = sum_j u_ij l_ij, d_i = a_ii - t
            let mut t = [C64::ZERO; 4];
            let mut sabs = 0.0;
            let blocks = (i - fi) / 4 * 4;
            for (q, j) in (fi..i).enumerate() {
                let u = row[q];
                let l = u / d[j];
                row[q] = l;
                let p = u * l;
                t[if q < blocks { q % 4 } else { 0 }] += p;
                sabs += p.abs();
            }
            let a_ii = d[i];
            let di = a_ii - ((t[0] + t[1]) + (t[2] + t[3]));
            let w = (i - fi) as f64;
            let scale = a_ii.abs() + sabs;
            let theta = ((w + 1.0) * f64::EPSILON * scale).max(tau * scale);
            if di.abs() <= theta || !di.is_finite() {
                return Err(ComplexSkylineError::Pivot {
                    row: order[i],
                    position: i,
                    pivot: di,
                    threshold: theta,
                    scale,
                });
            }
            d[i] = di;
        }
        Ok(ComplexSkylineLdlt {
            n,
            order,
            first,
            ptr,
            env,
            d,
        })
    }

    /// Dimension.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Elimination order: `order()[i]` = original index at position `i`.
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// Pivots `d_i` in elimination order.
    pub fn pivots(&self) -> &[C64] {
        &self.d
    }

    /// Unit lower factor on the envelope, row by row in elimination order.
    pub fn envelope_values(&self) -> &[C64] {
        &self.env
    }

    /// Strict lower envelope size.
    pub fn envelope_size(&self) -> usize {
        self.env.len()
    }

    /// Solve `A x = b`.
    pub fn solve(&self, b: &[C64]) -> Vec<C64> {
        let mut x = vec![C64::ZERO; self.n];
        self.solve_into(b, &mut x);
        x
    }

    /// Solve `A x = b` into `x`.
    pub fn solve_into(&self, b: &[C64], x: &mut [C64]) {
        assert_eq!(b.len(), self.n, "right-hand side length");
        assert_eq!(x.len(), self.n, "solution length");
        let mut y: Vec<C64> = self.order.iter().map(|&o| b[o]).collect();
        for i in 0..self.n {
            let fi = self.first[i];
            if fi < i {
                let s = cdot4(&self.env[self.ptr[i]..self.ptr[i + 1]], &y[fi..i]);
                y[i] -= s;
            }
        }
        for (yi, di) in y.iter_mut().zip(&self.d) {
            *yi /= *di;
        }
        for i in (0..self.n).rev() {
            let fi = self.first[i];
            let yi = y[i];
            let row = &self.env[self.ptr[i]..self.ptr[i + 1]];
            for (yj, l) in y[fi..i].iter_mut().zip(row) {
                *yj -= *l * yi;
            }
        }
        for (i, &o) in self.order.iter().enumerate() {
            x[o] = y[i];
        }
    }
}
