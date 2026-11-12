//! Envelope (skyline) `L D L^T` factorisation of a symmetric matrix after reverse Cuthill-McKee (F-373, ADR 0274).
//!
//! The factor is computed once and reused for any number of right-hand sides. The unit lower factor `L` is stored row
//! by row on the envelope of the (reordered) matrix: row `i` holds columns `first[i] .. i`, which is exactly where `L`
//! can be non-zero (no fill outside the envelope, ADR 0274 D1). Every operation is sequential with a fixed order, so the
//! bits of the factor and of a solution do not depend on the number of threads.

use crate::matrix::CsrMatrix;
use crate::reorder::{inverse_permutation, rcm_order};
use std::fmt;

/// Order in which the unknowns are eliminated.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SkylineOrdering {
    /// Reverse Cuthill-McKee ordering of the matrix graph ([`rcm_order`]).
    #[default]
    Rcm,
    /// The input order.
    Natural,
    /// A given permutation: `order[i]` = original index eliminated at position `i`.
    Given(Vec<usize>),
    /// Reverse Cuthill-McKee or the input order, whichever has the smaller envelope (ties: reverse Cuthill-McKee).
    SmallerEnvelope,
}

/// Which pivots are accepted (ADR 0274 D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PivotRule {
    /// Symmetric positive definite: refuse `d_i <= theta_i`.
    #[default]
    Positive,
    /// Symmetric indefinite, no pivoting: refuse `|d_i| <= theta_i`.
    NonZero,
}

/// Options of [`SkylineLdlt::factor_with`].
#[derive(Debug, Clone, PartialEq)]
pub struct SkylineOptions {
    /// Elimination order.
    pub ordering: SkylineOrdering,
    /// Pivot acceptance.
    pub pivots: PivotRule,
    /// Relative symmetry tolerance `delta`: `|a_rc - a_cr| <= delta sqrt(|a_rr a_cc|)`.
    pub symmetry_tolerance: f64,
}

impl Default for SkylineOptions {
    fn default() -> Self {
        SkylineOptions {
            ordering: SkylineOrdering::Rcm,
            pivots: PivotRule::Positive,
            symmetry_tolerance: 1e-12,
        }
    }
}

/// Refusal of a factorisation. Rows and columns are indices of the input matrix.
#[derive(Debug, Clone, PartialEq)]
pub enum SkylineError {
    /// The matrix is not square.
    NotSquare {
        /// Rows.
        rows: usize,
        /// Columns.
        cols: usize,
    },
    /// A stored value is NaN or infinite.
    NonFinite {
        /// Row.
        row: usize,
        /// Column.
        col: usize,
        /// The value.
        value: f64,
    },
    /// `a_rc` and `a_cr` differ by more than the tolerance.
    Unsymmetric {
        /// Row.
        row: usize,
        /// Column.
        col: usize,
        /// `a_rc`.
        a_rc: f64,
        /// `a_cr` (0 if not stored).
        a_cr: f64,
    },
    /// A pivot failed the test of the pivot rule.
    Pivot {
        /// Original index of the row.
        row: usize,
        /// Its position in the elimination order.
        position: usize,
        /// The computed pivot `d`.
        pivot: f64,
        /// The threshold `theta` it was compared with.
        threshold: f64,
    },
    /// A given ordering is not a permutation of `0..n`.
    BadOrdering {
        /// What is wrong.
        reason: String,
    },
}

impl fmt::Display for SkylineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkylineError::NotSquare { rows, cols } => write!(f, "matrix is not square ({rows} rows, {cols} columns)"),
            SkylineError::NonFinite { row, col, value } => {
                write!(f, "non-finite entry {value} at row {row}, column {col}")
            }
            SkylineError::Unsymmetric { row, col, a_rc, a_cr } => write!(
                f,
                "matrix is not symmetric at row {row}, column {col}: a[{row}][{col}] = {a_rc}, a[{col}][{row}] = {a_cr}"
            ),
            SkylineError::Pivot {
                row,
                position,
                pivot,
                threshold,
            } => write!(
                f,
                "pivot {pivot:e} of row {row} (elimination position {position}) fails the test against {threshold:e}: \
                 the matrix is not positive definite (or numerically singular) there"
            ),
            SkylineError::BadOrdering { reason } => write!(f, "bad ordering: {reason}"),
        }
    }
}

impl std::error::Error for SkylineError {}

/// Envelope `L D L^T` factor of a symmetric matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct SkylineLdlt {
    n: usize,
    order: Vec<usize>,
    first: Vec<usize>,
    ptr: Vec<usize>,
    env: Vec<f64>,
    d: Vec<f64>,
}

/// Dot product with four fixed accumulators (ADR 0274 D3): blocks of four in index order, the tail into the first,
/// combined as `(s0 + s1) + (s2 + s3)`.
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

/// First envelope column of every row in the order `order` (`inv` its inverse), from the stored pattern of `a`.
fn first_columns(a: &CsrMatrix, inv: &[usize]) -> Vec<usize> {
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

/// Strict lower envelope `sum_i (i - f_i)` of the square matrix `a` in the order `order` (`None`: the input order).
/// Every stored entry counts, of either triangle (ADR 0274 D1).
pub fn envelope_size(a: &CsrMatrix, order: Option<&[usize]>) -> usize {
    let inv = match order {
        Some(o) => inverse_permutation(o),
        None => (0..a.n).collect(),
    };
    first_columns(a, &inv).iter().enumerate().map(|(i, &f)| i - f).sum()
}

fn rcm_of(a: &CsrMatrix) -> Vec<usize> {
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

/// Stored value `a[r][c]`, 0 if not stored (columns sorted within rows).
fn entry(a: &CsrMatrix, r: usize, c: usize) -> Option<f64> {
    let (s, e) = (a.row_ptr[r], a.row_ptr[r + 1]);
    a.col[s..e].binary_search(&c).ok().map(|k| a.val[s + k])
}

fn validate(a: &CsrMatrix, delta: f64) -> Result<(), SkylineError> {
    if a.ncols != a.n {
        return Err(SkylineError::NotSquare {
            rows: a.n,
            cols: a.ncols,
        });
    }
    for r in 0..a.n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            if !a.val[k].is_finite() {
                return Err(SkylineError::NonFinite {
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
            let a_cr = entry(a, c, r).unwrap_or(0.0);
            if (a_rc - a_cr).abs() > delta * (diag[r].abs() * diag[c].abs()).sqrt() {
                return Err(SkylineError::Unsymmetric {
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

fn check_permutation(order: &[usize], n: usize) -> Result<(), SkylineError> {
    if order.len() != n {
        return Err(SkylineError::BadOrdering {
            reason: format!("length {} for {n} unknowns", order.len()),
        });
    }
    let mut seen = vec![false; n];
    for (i, &o) in order.iter().enumerate() {
        if o >= n {
            return Err(SkylineError::BadOrdering {
                reason: format!("entry {i} is {o}, out of range 0..{n}"),
            });
        }
        if seen[o] {
            return Err(SkylineError::BadOrdering {
                reason: format!("index {o} repeated (at position {i})"),
            });
        }
        seen[o] = true;
    }
    Ok(())
}

impl SkylineLdlt {
    /// Factorise a symmetric positive definite matrix with the default options (reverse Cuthill-McKee order,
    /// positive pivots, symmetry tolerance 1e-12).
    pub fn factor(a: &CsrMatrix) -> Result<SkylineLdlt, SkylineError> {
        Self::factor_with(a, &SkylineOptions::default())
    }

    /// Factorise `a` with the given options. The matrix is validated first (square, finite, symmetric within the
    /// tolerance); the lower triangle is used.
    pub fn factor_with(a: &CsrMatrix, opts: &SkylineOptions) -> Result<SkylineLdlt, SkylineError> {
        validate(a, opts.symmetry_tolerance)?;
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
                if envelope_size(a, Some(&rcm)) <= envelope_size(a, None) {
                    rcm
                } else {
                    (0..n).collect()
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
        let mut env = vec![0.0; ptr[n]];
        let mut d = vec![0.0; n];
        // Scatter the lower triangle (in the elimination order) into the envelope: the entry (order[i], order[j]),
        // j < i, if stored, else its transpose.
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
                if j > i && entry(a, a.col[k], r).is_none() {
                    env[ptr[j] + i - first[j]] = a.val[k];
                }
            }
        }
        for i in 0..n {
            let fi = first[i];
            let (done, rest) = env.split_at_mut(ptr[i]);
            let row = &mut rest[..i - fi];
            // step 1: u_ij = a_ij - sum_{k=lo}^{j-1} u_ik l_jk
            for j in fi..i {
                let fj = first[j];
                let lo = fi.max(fj);
                if lo < j {
                    let lj = &done[ptr[j] + lo - fj..ptr[j] + j - fj];
                    let s = dot4(&row[lo - fi..j - fi], lj);
                    row[j - fi] -= s;
                }
            }
            // step 2: l_ij = u_ij / d_j, t = sum_j u_ij l_ij, d_i = a_ii - t
            // (the same summation order as `dot4`: blocks of four, the tail into the first accumulator)
            let mut t = [0.0f64; 4];
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
            // step 3: the pivot test (D4)
            let w = (i - fi) as f64;
            let theta = (w + 1.0) * f64::EPSILON * (a_ii.abs() + sabs);
            let refused = match opts.pivots {
                PivotRule::Positive => di <= theta,
                PivotRule::NonZero => di.abs() <= theta,
            };
            if refused || !di.is_finite() {
                return Err(SkylineError::Pivot {
                    row: order[i],
                    position: i,
                    pivot: di,
                    threshold: theta,
                });
            }
            d[i] = di;
        }
        Ok(SkylineLdlt {
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
    pub fn pivots(&self) -> &[f64] {
        &self.d
    }

    /// Unit lower factor on the envelope: row `i` (elimination order) holds columns `first_column(i) .. i`.
    pub fn envelope_values(&self) -> &[f64] {
        &self.env
    }

    /// First envelope column of row `i` (elimination order).
    pub fn first_column(&self, i: usize) -> usize {
        self.first[i]
    }

    /// Counts of negative and positive pivots (by Sylvester's law of inertia, of eigenvalues when the factorisation is
    /// exact).
    pub fn inertia(&self) -> (usize, usize) {
        let neg = self.d.iter().filter(|&&v| v < 0.0).count();
        (neg, self.n - neg)
    }

    /// Strict lower envelope size `|Env|`.
    pub fn envelope_size(&self) -> usize {
        self.env.len()
    }

    /// Bytes of the factor's arrays: `8 (|Env| + n)` values and `8 (3 n + 1)` indices.
    pub fn storage_bytes(&self) -> usize {
        8 * (self.env.len() + self.n) + 8 * (3 * self.n + 1)
    }

    /// Solve `A x = b`.
    pub fn solve(&self, b: &[f64]) -> Vec<f64> {
        let mut x = vec![0.0; self.n];
        self.solve_into(b, &mut x);
        x
    }

    /// Solve `A x = b` into `x`.
    pub fn solve_into(&self, b: &[f64], x: &mut [f64]) {
        assert_eq!(b.len(), self.n, "right-hand side length");
        assert_eq!(x.len(), self.n, "solution length");
        let mut y: Vec<f64> = self.order.iter().map(|&o| b[o]).collect();
        for i in 0..self.n {
            let fi = self.first[i];
            if fi < i {
                let s = dot4(&self.env[self.ptr[i]..self.ptr[i + 1]], &y[fi..i]);
                y[i] -= s;
            }
        }
        for (yi, di) in y.iter_mut().zip(&self.d) {
            *yi /= di;
        }
        for i in (0..self.n).rev() {
            let fi = self.first[i];
            let yi = y[i];
            let row = &self.env[self.ptr[i]..self.ptr[i + 1]];
            for (yj, l) in y[fi..i].iter_mut().zip(row) {
                *yj -= l * yi;
            }
        }
        for (i, &o) in self.order.iter().enumerate() {
            x[o] = y[i];
        }
    }
}
