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
