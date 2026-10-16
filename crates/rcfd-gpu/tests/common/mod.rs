//! Shared helpers of the rcfd-gpu integration tests: registered matrices and compensated references.
#![allow(dead_code, missing_docs)]

use rcfd_linalg::CsrMatrix;

pub struct Lcg(pub u64);

impl Lcg {
    pub fn next_f64(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    pub fn range(&mut self, n: usize) -> usize {
        ((self.next_f64() * n as f64) as usize).min(n - 1)
    }
}

/// 7-point (or 5-point for nz = 1) Poisson-like matrix; `conv` > 0 adds upwind convection in x (nonsymmetric).
pub fn grid_matrix(nx: usize, ny: usize, nz: usize, conv: f64) -> CsrMatrix {
    let n = nx * ny * nz;
    let idx = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    let mut t = Vec::with_capacity(7 * n);
    let dim = if nz > 1 { 3.0 } else { 2.0 };
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let r = idx(i, j, k);
                t.push((r, r, 2.0 * dim + conv));
                if i > 0 {
                    t.push((r, idx(i - 1, j, k), -1.0 - conv));
                }
                if i + 1 < nx {
                    t.push((r, idx(i + 1, j, k), -1.0));
                }
                if j > 0 {
                    t.push((r, idx(i, j - 1, k), -1.0));
                }
                if j + 1 < ny {
                    t.push((r, idx(i, j + 1, k), -1.0));
                }
                if k > 0 {
                    t.push((r, idx(i, j, k - 1), -1.0));
                }
                if k + 1 < nz {
                    t.push((r, idx(i, j, k + 1), -1.0));
                }
            }
        }
    }
    CsrMatrix::from_triplets(n, t)
}

/// Symmetric diagonally dominant pseudo-random sparse matrix (SPD).
pub fn random_spd(n: usize, seed: u64) -> CsrMatrix {
    let mut g = Lcg(seed);
    let mut t = Vec::new();
    let mut offsum = vec![0.0; n];
    for i in 0..n {
        for _ in 0..3 {
            let j = g.range(n);
            if j == i {
                continue;
            }
            let v = -(0.1 + g.next_f64());
            t.push((i, j, v));
            t.push((j, i, v));
            offsum[i] += v.abs();
            offsum[j] += v.abs();
        }
    }
    for i in 0..n {
        t.push((i, i, offsum[i] + 1.0));
    }
    CsrMatrix::from_triplets(n, t)
}

/// Irregular general sparse matrix: 0..39 off-diagonal entries per row (some rows diagonal only).
pub fn random_irregular(n: usize, seed: u64) -> CsrMatrix {
    let mut g = Lcg(seed);
    let mut t = Vec::new();
    for i in 0..n {
        t.push((i, i, 1.0 + g.next_f64()));
        let k = if i % 7 == 0 { 0 } else { g.range(40) };
        for _ in 0..k {
            t.push((i, g.range(n), g.next_f64() * 2.0 - 1.0));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

/// Neumaier compensated sum.
pub fn neumaier(it: impl Iterator<Item = f64>) -> f64 {
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for x in it {
        let t = s + x;
        if s.abs() >= x.abs() {
            c += (s - t) + x;
        } else {
            c += (x - t) + s;
        }
        s = t;
    }
    s + c
}

pub fn rel2(a: &[f64], b: &[f64]) -> f64 {
    let num: f64 = a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt();
    let den: f64 = b.iter().map(|y| y * y).sum::<f64>().sqrt();
    if den == 0.0 {
        num
    } else {
        num / den
    }
}

pub fn matvec_ref(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

#[cfg(feature = "cuda")]
pub mod bench;
