//! Iterative refinement of the direct static solve with a compensated residual, and residual statistics (ADR 0397).
//!
//! The residual `b - A x` of a refinement step is evaluated as if in twice the working precision by error-free
//! transformations: Knuth's TwoSum, the product with its exact error by a fused multiply-add, and the dot product Dot2 of
//! Ogita, Rump and Oishi (SIAM J. Sci. Comput. 26 (2005) 1955), `|dot2 - x.y| <= u |x.y| + gamma_n^2 sum |x_i y_i|`
//! (ADR 0397 D2). The correction is solved with the existing `L D L^T` factor and added in double precision (classical
//! iterative refinement, ADR 0397 D3).
//!
//! In double precision a relative residual `|b - A x| / |b|` cannot be made smaller than about the representability floor
//! `phi_rep` of [`ResidualStatistics`], and, evaluated in working precision, not smaller than the evaluation floor `phi_w`
//! (ADR 0397 arithmetic 1 and 2). One refinement step reaches the first; the backward-error statistic `omega` and the energy
//! defect [`energy_defect`] show what the step achieves.

use rcfd_linalg::skyline::SkylineLdlt;
use rcfd_linalg::CsrMatrix;

/// `(s, e)` with `s = fl(a + b)` and `a + b = s + e` exactly (Knuth's TwoSum).
#[inline]
pub fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    let e = (a - (s - bb)) + (b - bb);
    (s, e)
}

/// `(p, e)` with `p = fl(a b)` and `a b = p + e` exactly (the error by one fused multiply-add).
#[inline]
pub fn two_product(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    let e = a.mul_add(b, -p);
    (p, e)
}

/// The dot product `x . y` as if computed in twice the working precision and rounded once (Dot2 of Ogita, Rump and Oishi).
/// Sequential, in index order. Empty vectors give 0.
pub fn dot2(x: &[f64], y: &[f64]) -> f64 {
    assert_eq!(x.len(), y.len(), "dot2: lengths");
    if x.is_empty() {
        return 0.0;
    }
    let (mut p, mut s) = two_product(x[0], y[0]);
    for (&a, &b) in x.iter().zip(y).skip(1) {
        let (h, r) = two_product(a, b);
        let (pn, q) = two_sum(p, h);
        p = pn;
        s += q + r;
    }
    p + s
}

/// `r = b - A x`, each row as the compensated dot product of `(b_i, -a_i1, ..., -a_in)` with `(1, x_1, ..., x_n)`
/// (ADR 0397 D2); rows in order, no threads.
pub fn residual_compensated(a: &CsrMatrix, b: &[f64], x: &[f64]) -> Vec<f64> {
    assert_eq!(b.len(), a.n, "residual_compensated: right-hand side length");
    assert_eq!(x.len(), a.ncols, "residual_compensated: solution length");
    let mut r = vec![0.0; a.n];
    for (i, ri) in r.iter_mut().enumerate() {
        let mut p = b[i];
        let mut s = 0.0;
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            let (h, e) = two_product(a.val[k], x[a.col[k]]);
            let (pn, q) = two_sum(p, -h);
            p = pn;
            s += q - e;
        }
        *ri = p + s;
    }
    r
}

/// One step of iterative refinement in place: `x += solve(residual_compensated(A, b, x))` with the given factor of `A`.
/// Returns the 2-norm of the correction.
pub fn refine_step(a: &CsrMatrix, b: &[f64], factor: &SkylineLdlt, x: &mut [f64]) -> f64 {
    let r = residual_compensated(a, b, x);
    let d = factor.solve(&r);
    for (xi, di) in x.iter_mut().zip(&d) {
        *xi += di;
    }
    norm2(&d)
}

fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|t| t * t).sum::<f64>().sqrt()
}

/// Residual statistics of a solution `x` of `A x = b` (ADR 0397 D1). Norms are 2-norms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResidualStatistics {
    /// `|b - A x| / |b|` with the residual in working precision (`CsrMatrix::residual`), as the static solve reports it.
    pub rho_w: f64,
    /// `|r_x| / |b|`, `r_x` the compensated residual ([`residual_compensated`]): the true relative residual.
    pub rho_x: f64,
    /// `eps || |A| |x| || / |b|`: the evaluation floor of `rho_w`.
    pub phi_w: f64,
    /// `(u / sqrt 3) || (sum_j a_ij^2 x_j^2)^(1/2) || / |b|`, `u = eps / 2`: the representability floor of `rho_x`.
    pub phi_rep: f64,
    /// `|r_x| / (|| |A| |x| || + |b|)`: a normwise backward-error statistic.
    pub omega: f64,
}

/// The [`ResidualStatistics`] of `x`. A zero right-hand side gives zeros (and `omega` from the residual alone).
pub fn residual_statistics(a: &CsrMatrix, b: &[f64], x: &[f64]) -> ResidualStatistics {
    let bn = norm2(b);
    let mut rw = vec![0.0; a.n];
    a.residual(b, x, &mut rw);
    let rx = residual_compensated(a, b, x);
    let mut abs_sum = vec![0.0; a.n];
    let mut rss = vec![0.0; a.n];
    for i in 0..a.n {
        let (mut t, mut q) = (0.0, 0.0);
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            let v = a.val[k] * x[a.col[k]];
            t += v.abs();
            q += v * v;
        }
        abs_sum[i] = t;
        rss[i] = q.sqrt();
    }
    let (rwn, rxn, an, sn) = (norm2(&rw), norm2(&rx), norm2(&abs_sum), norm2(&rss));
    let rel = |v: f64| if bn > 0.0 { v / bn } else { 0.0 };
    let u = f64::EPSILON / 2.0;
    let den = an + bn;
    ResidualStatistics {
        rho_w: rel(rwn),
        rho_x: rel(rxn),
        phi_w: rel(f64::EPSILON * an),
        phi_rep: rel(u / 3f64.sqrt() * sn),
        omega: if den > 0.0 { rxn / den } else { 0.0 },
    }
}

/// `|x^T A x - b^T x| / (b^T x) = |x . r_x| / (b . x)` by [`dot2`] and [`residual_compensated`]: the defect of "the strain
/// energy `x^T A x / 2` equals half the work `b^T x`" of the reduced system (ADR 0397 D1). `b . x = 0` gives infinity
/// unless the numerator is zero.
pub fn energy_defect(a: &CsrMatrix, b: &[f64], x: &[f64]) -> f64 {
    let r = residual_compensated(a, b, x);
    let num = dot2(x, &r).abs();
    let work = dot2(b, x);
    if num == 0.0 {
        0.0
    } else {
        num / work.abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_cases() {
        assert_eq!(dot2(&[1e16, 1.0, -1e16], &[1.0, 1.0, 1.0]), 1.0);
        let t = 1.0 + 2f64.powi(-30);
        assert_eq!(dot2(&[t, -1.0], &[t, 1.0]), 2f64.powi(-29) + 2f64.powi(-60));
        assert_eq!(two_sum(1.0, 1e-20), (1.0, 1e-20));
    }
}
