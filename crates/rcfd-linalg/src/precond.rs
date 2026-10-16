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
