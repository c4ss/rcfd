//! Envelope `L D U` factorisation of an unsymmetric matrix without pivoting (F-572, ADR 0560 D4).
//!
//! Crout's form of Gaussian elimination on the envelope of the symmetrised pattern after the ordering of
//! [`crate::skyline`] (reverse Cuthill-McKee by default): `L` unit lower, stored by rows on columns `first[i] .. i`, `U` unit
//! upper, stored by columns on rows `first[i] .. i`, `D` the pivots (ADR 0560 arithmetic 2). For `j < i`, `k` over
//! `max(f_i, f_j) .. j - 1`: `l~_ij = a_ij - sum_k l~_ik u_kj`, `u~_ji = a_ji - sum_k l_jk u~_ki`, `l_ij = l~_ij / d_j`, `u_ji =
//! u~_ji / d_j`, `d_i = a_ii - sum_j l~_ij u_ji`. The sums and the pivot test are those of [`crate::skyline::SkylineLdlt`],
//! so that on a symmetric matrix the two factors perform the same operations. No row interchanges: a matrix whose symmetric
//! part is positive definite has positive pivots (ADR 0560 arithmetic 2); any other matrix is factored while its pivots pass
//! the test. Every operation is sequential with a fixed order.

use crate::matrix::CsrMatrix;
use crate::reorder::inverse_permutation;
use crate::skyline::{
    check_permutation, dot4, envelope_size, first_columns, rcm_of, PivotRule, SkylineError, SkylineOrdering,
};

/// Envelope `L D U` factor of a square matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeLu {
    n: usize,
    order: Vec<usize>,
    first: Vec<usize>,
    ptr: Vec<usize>,
    lower: Vec<f64>,
    upper: Vec<f64>,
    d: Vec<f64>,
}

fn validate(a: &CsrMatrix) -> Result<(), SkylineError> {
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
    Ok(())
}

impl EnvelopeLu {
    /// Factorise `a` in the reverse Cuthill-McKee order with the pivot rule [`PivotRule::NonZero`].
    pub fn factor(a: &CsrMatrix) -> Result<EnvelopeLu, SkylineError> {
        Self::factor_with(a, &SkylineOrdering::Rcm, PivotRule::NonZero)
    }

    /// Factorise `a` in the order `ordering` with the pivot rule `pivots` (the threshold of [`crate::skyline`]). The matrix
    /// must be square and finite; it need not be symmetric, in values or in pattern.
    pub fn factor_with(
        a: &CsrMatrix,
        ordering: &SkylineOrdering,
        pivots: PivotRule,
    ) -> Result<EnvelopeLu, SkylineError> {
        validate(a)?;
        let n = a.n;
        let order = match ordering {
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
        let mut lower = vec![0.0; ptr[n]];
        let mut upper = vec![0.0; ptr[n]];
        let mut d = vec![0.0; n];
        // Scatter: (i, j) with j < i into row i of the lower envelope; with j > i into column j of the upper envelope.
        for r in 0..n {
            let i = inv[r];
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                let j = inv[a.col[k]];
                if j < i {
                    lower[ptr[i] + j - first[i]] = a.val[k];
                } else if j > i {
                    upper[ptr[j] + i - first[j]] = a.val[k];
                } else {
                    d[i] = a.val[k];
                }
            }
        }
        for i in 0..n {
            let fi = first[i];
            let (ldone, lrest) = lower.split_at_mut(ptr[i]);
            let (udone, urest) = upper.split_at_mut(ptr[i]);
            let lrow = &mut lrest[..i - fi];
            let ucol = &mut urest[..i - fi];
            // step 1: l~_ij = a_ij - sum l~_ik u_kj and u~_ji = a_ji - sum l_jk u~_ki
            for j in fi..i {
                let fj = first[j];
                let lo = fi.max(fj);
                if lo < j {
                    let uj = &udone[ptr[j] + lo - fj..ptr[j] + j - fj];
                    let lj = &ldone[ptr[j] + lo - fj..ptr[j] + j - fj];
                    let sl = dot4(&lrow[lo - fi..j - fi], uj);
                    lrow[j - fi] -= sl;
                    let su = dot4(&ucol[lo - fi..j - fi], lj);
                    ucol[j - fi] -= su;
                }
            }
            // step 2: l_ij = l~_ij / d_j, u_ji = u~_ji / d_j, d_i = a_ii - sum_j l~_ij u_ji (the summation of `SkylineLdlt`)
            let mut t = [0.0f64; 4];
            let mut sabs = 0.0;
            let blocks = (i - fi) / 4 * 4;
            for (q, j) in (fi..i).enumerate() {
                let lt = lrow[q];
                let u = ucol[q] / d[j];
                lrow[q] = lt / d[j];
                ucol[q] = u;
                let p = lt * u;
                t[if q < blocks { q % 4 } else { 0 }] += p;
                sabs += p.abs();
            }
            let a_ii = d[i];
            let di = a_ii - ((t[0] + t[1]) + (t[2] + t[3]));
            let w = (i - fi) as f64;
            let theta = (w + 1.0) * f64::EPSILON * (a_ii.abs() + sabs);
            let refused = match pivots {
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
        Ok(EnvelopeLu {
            n,
            order,
            first,
            ptr,
            lower,
            upper,
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

    /// The number of negative pivots (for an unsymmetric matrix not an inertia).
    pub fn negative_pivots(&self) -> usize {
        self.d.iter().filter(|&&v| v < 0.0).count()
    }

    /// Strict lower envelope size (the upper one is the same).
    pub fn envelope_size(&self) -> usize {
        self.lower.len()
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
                let s = dot4(&self.lower[self.ptr[i]..self.ptr[i + 1]], &y[fi..i]);
                y[i] -= s;
            }
        }
        for (yi, di) in y.iter_mut().zip(&self.d) {
            *yi /= di;
        }
        for i in (0..self.n).rev() {
            let fi = self.first[i];
            let yi = y[i];
            let col = &self.upper[self.ptr[i]..self.ptr[i + 1]];
            for (yj, u) in y[fi..i].iter_mut().zip(col) {
                *yj -= u * yi;
            }
        }
        for (i, &o) in self.order.iter().enumerate() {
            x[o] = y[i];
        }
    }
}
