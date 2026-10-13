//! Face-addressed LDU and CSR sparse matrices.

use crate::vecops::CHUNK;
use rayon::prelude::*;
use std::sync::Arc;

/// Sparsity pattern of a cell-centred matrix: one off-diagonal pair per
/// internal face. `lower[f]` is the owner row, `upper[f]` the neighbour row
/// (`lower[f] < upper[f]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LduAddressing {
    /// Matrix dimension.
    pub n: usize,
    /// Owner (smaller) cell of each face.
    pub lower: Vec<usize>,
    /// Neighbour (larger) cell of each face.
    pub upper: Vec<usize>,
}

/// LDU matrix: `A[owner][neighbour] = upper_coeff[f]`,
/// `A[neighbour][owner] = lower_coeff[f]`, plus a diagonal.
#[derive(Debug, Clone)]
pub struct LduMatrix {
    /// Shared addressing.
    pub addr: Arc<LduAddressing>,
    /// Diagonal.
    pub diag: Vec<f64>,
    /// Coefficient of row `upper[f]`, column `lower[f]`.
    pub lower: Vec<f64>,
    /// Coefficient of row `lower[f]`, column `upper[f]`.
    pub upper: Vec<f64>,
}

impl LduMatrix {
    /// Zero matrix on the given addressing.
    pub fn zeros(addr: Arc<LduAddressing>) -> Self {
        let (n, nf) = (addr.n, addr.lower.len());
        LduMatrix {
            addr,
            diag: vec![0.0; n],
            lower: vec![0.0; nf],
            upper: vec![0.0; nf],
        }
    }

    /// Whether `lower == upper` (symmetric matrix).
    pub fn is_symmetric(&self) -> bool {
        self.lower == self.upper
    }

    /// `y = A x` (serial scatter; kept for assembly-time use and tests).
    pub fn matvec(&self, x: &[f64], y: &mut [f64]) {
        for (yi, (d, xi)) in y.iter_mut().zip(self.diag.iter().zip(x)) {
            *yi = d * xi;
        }
        for f in 0..self.upper.len() {
            let (l, u) = (self.addr.lower[f], self.addr.upper[f]);
            y[l] += self.upper[f] * x[u];
            y[u] += self.lower[f] * x[l];
        }
    }

    /// Sum of absolute off-diagonal entries per row.
    pub fn off_diag_abs_sum(&self) -> Vec<f64> {
        let mut s = vec![0.0; self.addr.n];
        for f in 0..self.upper.len() {
            s[self.addr.lower[f]] += self.upper[f].abs();
            s[self.addr.upper[f]] += self.lower[f].abs();
        }
        s
    }

    /// `H` operator numerator: `b - sum_offdiag A_ij x_j`, per row.
    pub fn h_operator(&self, x: &[f64], b: &[f64]) -> Vec<f64> {
        let mut h = b.to_vec();
        for f in 0..self.upper.len() {
            let (l, u) = (self.addr.lower[f], self.addr.upper[f]);
            h[l] -= self.upper[f] * x[u];
            h[u] -= self.lower[f] * x[l];
        }
        h
    }

    /// Implicit under-relaxation (Patankar): enforce diagonal dominance, then
    /// `D/alpha`, `b += (1-alpha)/alpha * D * x_old`.
    pub fn relax(&mut self, alpha: f64, x_old: &[f64], b: &mut [f64]) {
        assert!(alpha > 0.0 && alpha <= 1.0, "relaxation factor must be in (0,1]");
        let sums = self.off_diag_abs_sum();
        for i in 0..self.diag.len() {
            let d = self.diag[i].max(sums[i]);
            let d_relaxed = d / alpha;
            b[i] += (d_relaxed - self.diag[i]) * x_old[i];
            self.diag[i] = d_relaxed;
        }
    }

    /// Convert to CSR with sorted columns.
    pub fn to_csr(&self) -> CsrMatrix {
        let n = self.addr.n;
        let mut counts = vec![1usize; n];
        for f in 0..self.upper.len() {
            counts[self.addr.lower[f]] += 1;
            counts[self.addr.upper[f]] += 1;
        }
        let mut row_ptr = vec![0usize; n + 1];
        for i in 0..n {
            row_ptr[i + 1] = row_ptr[i] + counts[i];
        }
        let mut fill = row_ptr.clone();
        let mut col = vec![0usize; row_ptr[n]];
        let mut val = vec![0.0; row_ptr[n]];
        for i in 0..n {
            col[fill[i]] = i;
            val[fill[i]] = self.diag[i];
            fill[i] += 1;
        }
        for f in 0..self.upper.len() {
            let (l, u) = (self.addr.lower[f], self.addr.upper[f]);
            col[fill[l]] = u;
            val[fill[l]] = self.upper[f];
            fill[l] += 1;
            col[fill[u]] = l;
            val[fill[u]] = self.lower[f];
            fill[u] += 1;
        }
        // sort each row by column
        for i in 0..n {
            let (s, e) = (row_ptr[i], row_ptr[i + 1]);
            let mut idx: Vec<usize> = (s..e).collect();
            idx.sort_by_key(|&k| col[k]);
            let cs: Vec<usize> = idx.iter().map(|&k| col[k]).collect();
            let vs: Vec<f64> = idx.iter().map(|&k| val[k]).collect();
            col[s..e].copy_from_slice(&cs);
            val[s..e].copy_from_slice(&vs);
        }
        CsrMatrix {
            n,
            ncols: n,
            row_ptr,
            col,
            val,
        }
    }
}

/// Compressed sparse row matrix (square).
#[derive(Debug, Clone, PartialEq)]
pub struct CsrMatrix {
    /// Number of rows.
    pub n: usize,
    /// Number of columns (equal to `n` for square matrices).
    pub ncols: usize,
    /// Row pointers (length n+1).
    pub row_ptr: Vec<usize>,
    /// Column indices, sorted within rows.
    pub col: Vec<usize>,
    /// Values.
    pub val: Vec<f64>,
}

impl CsrMatrix {
    /// Build from (row, col, value) triplets; duplicates are summed.
    pub fn from_triplets(n: usize, mut t: Vec<(usize, usize, f64)>) -> Self {
        t.sort_by_key(|&(r, c, _)| (r, c));
        let mut row_ptr = vec![0usize; n + 1];
        let mut col = Vec::new();
        let mut val: Vec<f64> = Vec::new();
        let mut last: Option<(usize, usize)> = None;
        for (r, c, v) in t {
            if last == Some((r, c)) {
                *val.last_mut().expect("non-empty") += v;
            } else {
                col.push(c);
                val.push(v);
                row_ptr[r + 1] += 1;
                last = Some((r, c));
            }
        }
        for i in 0..n {
            row_ptr[i + 1] += row_ptr[i];
        }
        CsrMatrix {
            n,
            ncols: n,
            row_ptr,
            col,
            val,
        }
    }

    /// Rectangular matrix from raw CSR arrays (columns sorted within rows).
    pub fn rect(n: usize, ncols: usize, row_ptr: Vec<usize>, col: Vec<usize>, val: Vec<f64>) -> Self {
        CsrMatrix {
            n,
            ncols,
            row_ptr,
            col,
            val,
        }
    }

    /// Transpose (columns of the result are sorted).
    pub fn transpose(&self) -> CsrMatrix {
        let mut counts = vec![0usize; self.ncols + 1];
        for &c in &self.col {
            counts[c + 1] += 1;
        }
        for i in 0..self.ncols {
            counts[i + 1] += counts[i];
        }
        let mut fill = counts.clone();
        let mut col = vec![0usize; self.nnz()];
        let mut val = vec![0.0; self.nnz()];
        for i in 0..self.n {
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                let c = self.col[k];
                col[fill[c]] = i;
                val[fill[c]] = self.val[k];
                fill[c] += 1;
            }
        }
        CsrMatrix {
            n: self.ncols,
            ncols: self.n,
            row_ptr: counts,
            col,
            val,
        }
    }

    /// Sparse product `self * other`, parallel over row blocks; each row is
    /// accumulated in a fixed order, so the result is deterministic.
    pub fn matmul(&self, other: &CsrMatrix) -> CsrMatrix {
        assert_eq!(self.ncols, other.n, "dimension mismatch in matmul");
        const BLOCK: usize = 512;
        let blocks: Vec<(Vec<usize>, Vec<usize>, Vec<f64>)> = (0..self.n.div_ceil(BLOCK))
            .into_par_iter()
            .map(|b| {
                let (lo, hi) = (b * BLOCK, ((b + 1) * BLOCK).min(self.n));
                let mut marker = vec![usize::MAX; other.ncols];
                let mut acc = vec![0.0; other.ncols];
                let mut counts = Vec::with_capacity(hi - lo);
                let mut cols = Vec::new();
                let mut vals = Vec::new();
                let mut row_cols: Vec<usize> = Vec::new();
                for i in lo..hi {
                    row_cols.clear();
                    for ka in self.row_ptr[i]..self.row_ptr[i + 1] {
                        let (j, av) = (self.col[ka], self.val[ka]);
                        for kb in other.row_ptr[j]..other.row_ptr[j + 1] {
                            let c = other.col[kb];
                            if marker[c] != i {
                                marker[c] = i;
                                acc[c] = 0.0;
                                row_cols.push(c);
                            }
                            acc[c] += av * other.val[kb];
                        }
                    }
                    row_cols.sort_unstable();
                    counts.push(row_cols.len());
                    for &c in &row_cols {
                        cols.push(c);
                        vals.push(acc[c]);
                    }
                }
                (counts, cols, vals)
            })
            .collect();
        let mut row_ptr = Vec::with_capacity(self.n + 1);
        row_ptr.push(0);
        let (mut col, mut val) = (Vec::new(), Vec::new());
        for (counts, c, v) in blocks {
            for k in counts {
                row_ptr.push(row_ptr.last().expect("non-empty") + k);
            }
            col.extend(c);
            val.extend(v);
        }
        CsrMatrix {
            n: self.n,
            ncols: other.ncols,
            row_ptr,
            col,
            val,
        }
    }

    /// Number of stored entries.
    pub fn nnz(&self) -> usize {
        self.val.len()
    }

    /// `y = A x`, parallel over fixed row chunks (each row is summed sequentially).
    pub fn matvec(&self, x: &[f64], y: &mut [f64]) {
        y.par_chunks_mut(CHUNK).enumerate().for_each(|(ci, yc)| {
            for (k, yi) in yc.iter_mut().enumerate() {
                let i = ci * CHUNK + k;
                let mut s = 0.0;
                for j in self.row_ptr[i]..self.row_ptr[i + 1] {
                    s += self.val[j] * x[self.col[j]];
                }
                *yi = s;
            }
        });
    }

    /// Diagonal entries (0 where absent).
    pub fn diag(&self) -> Vec<f64> {
        (0..self.n)
            .map(|i| {
                (self.row_ptr[i]..self.row_ptr[i + 1])
                    .find(|&j| self.col[j] == i)
                    .map_or(0.0, |j| self.val[j])
            })
            .collect()
    }

    /// `r = b - A x`
    pub fn residual(&self, b: &[f64], x: &[f64], r: &mut [f64]) {
        self.matvec(x, r);
        r.par_chunks_mut(CHUNK).zip(b.par_chunks(CHUNK)).for_each(|(rc, bc)| {
            for (ri, bi) in rc.iter_mut().zip(bc) {
                *ri = bi - *ri;
            }
        });
    }
}

/// 2D five-point Poisson-like SPD test matrix.
#[cfg(test)]
pub(crate) fn poisson_2d(nx: usize, ny: usize) -> LduMatrix {
    // cell id = i + nx*j, owner<neighbour ordered by owner
    let mut lower = Vec::new();
    let mut upper = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let c = i + nx * j;
            if i + 1 < nx {
                lower.push(c);
                upper.push(c + 1);
            }
            if j + 1 < ny {
                lower.push(c);
                upper.push(c + nx);
            }
        }
    }
    let addr = Arc::new(LduAddressing {
        n: nx * ny,
        lower,
        upper,
    });
    let mut m = LduMatrix::zeros(addr.clone());
    for f in 0..addr.lower.len() {
        m.lower[f] = -1.0;
        m.upper[f] = -1.0;
        m.diag[addr.lower[f]] += 1.0;
        m.diag[addr.upper[f]] += 1.0;
    }
    // Dirichlet-like: add 1 on the diagonal of boundary-adjacent cells so the matrix is SPD
    for j in 0..ny {
        for i in 0..nx {
            if i == 0 || i == nx - 1 {
                m.diag[i + nx * j] += 2.0;
            }
            if j == 0 || j == ny - 1 {
                m.diag[i + nx * j] += 2.0;
            }
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1D Poisson on n cells: diag 2, off-diagonals -1.
    pub fn laplace_1d(n: usize) -> LduMatrix {
        let addr = Arc::new(LduAddressing {
            n,
            lower: (0..n - 1).collect(),
            upper: (1..n).collect(),
        });
        let mut m = LduMatrix::zeros(addr);
        m.diag.iter_mut().for_each(|d| *d = 2.0);
        m.lower.iter_mut().for_each(|c| *c = -1.0);
        m.upper.iter_mut().for_each(|c| *c = -1.0);
        m
    }

    #[test]
    fn ldu_and_csr_agree() {
        let mut m = laplace_1d(6);
        // make it asymmetric
        m.upper[2] = -0.5;
        m.diag[3] = 7.0;
        assert!(!m.is_symmetric());
        let c = m.to_csr();
        assert_eq!(c.nnz(), 6 + 2 * 5);
        let x: Vec<f64> = (0..6).map(|i| (i as f64 + 1.0).sin()).collect();
        let (mut y1, mut y2) = (vec![0.0; 6], vec![0.0; 6]);
        m.matvec(&x, &mut y1);
        c.matvec(&x, &mut y2);
        for (a, b) in y1.iter().zip(&y2) {
            assert!((a - b).abs() < 1e-14);
        }
        assert_eq!(c.diag()[3], 7.0);
        // columns sorted
        for i in 0..6 {
            assert!(c.col[c.row_ptr[i]..c.row_ptr[i + 1]].windows(2).all(|w| w[0] < w[1]));
        }
        let mut r = vec![0.0; 6];
        c.residual(&y1, &x, &mut r);
        assert!(r.iter().all(|v| v.abs() < 1e-14));
    }

    #[test]
    fn triplets_sum_duplicates() {
        let c = CsrMatrix::from_triplets(2, vec![(0, 0, 1.0), (0, 0, 2.0), (1, 0, 5.0), (0, 1, -1.0)]);
        assert_eq!(c.val, vec![3.0, -1.0, 5.0]);
        assert_eq!(c.row_ptr, vec![0, 2, 3]);
    }

    #[test]
    fn h_operator_and_relax() {
        let m = laplace_1d(3);
        let x = vec![1.0, 2.0, 3.0];
        let b = vec![0.0, 0.0, 0.0];
        // H_i = b_i - sum_j!=i a_ij x_j
        assert_eq!(m.h_operator(&x, &b), vec![2.0, 4.0, 2.0]);
        let mut m2 = m.clone();
        let mut b2 = b.clone();
        m2.relax(0.5, &x, &mut b2);
        // interior row: diag 2 (offdiag sum 2) -> 2/0.5 = 4, b += (4-2)*x
        assert_eq!(m2.diag, vec![4.0, 4.0, 4.0]);
        assert_eq!(b2, vec![2.0, 4.0, 6.0]);
        // the relaxed system keeps x as its solution if x solved the original with source b
        let mut ax = vec![0.0; 3];
        m.matvec(&x, &mut ax);
        let mut ax2 = vec![0.0; 3];
        m2.matvec(&x, &mut ax2);
        for i in 0..3 {
            assert!(((ax2[i] - b2[i]) - (ax[i] - b[i])).abs() < 1e-14);
        }
    }

    #[test]
    fn poisson_matrix_is_spd_like() {
        let m = poisson_2d(4, 4);
        assert!(m.is_symmetric());
        let x: Vec<f64> = (0..16).map(|i| ((i * 5 + 1) % 7) as f64 - 3.0).collect();
        let mut y = vec![0.0; 16];
        m.matvec(&x, &mut y);
        let xtax: f64 = x.iter().zip(&y).map(|(a, b)| a * b).sum();
        assert!(xtax > 0.0);
    }
}
