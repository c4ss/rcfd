//! The pencils of F-664 (ADR 0587 D3): K1 and K2 of ADR 0580 (the same constructions and seeds) and K5c (complex
//! eigenvalues nearest the shift). Each returns `(A, B, shift, count)` as CSR, with the dense `A` and `B` (row-major).

#![allow(dead_code)]

use rcfd_linalg::CsrMatrix;

pub struct Pencil {
    pub name: &'static str,
    pub n: usize,
    pub a: CsrMatrix,
    pub b: CsrMatrix,
    pub a_dense: Vec<f64>,
    pub b_dense: Vec<f64>,
    pub shift: f64,
    pub count: usize,
}

fn splitmix(state: &mut u64) -> f64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

fn dense_to_csr(a: &[f64], n: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        for j in 0..n {
            if a[i * n + j] != 0.0 {
                t.push((i, j, a[i * n + j]));
            }
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn matmul(a: &[f64], b: &[f64], n: usize) -> Vec<f64> {
    let mut c = vec![0.0; n * n];
    for i in 0..n {
        for k in 0..n {
            let x = a[i * n + k];
            if x != 0.0 {
                for j in 0..n {
                    c[i * n + j] += x * b[k * n + j];
                }
            }
        }
    }
    c
}

/// `A = diag(bdiag) Q D Q^-1`, `B = diag(bdiag)`, `Q = I + L`; `blocks` 2 x 2 blocks `[[a, c], [-c, a]]` then real values.
fn similar(
    n: usize,
    s: &mut u64,
    blocks: usize,
    block: impl Fn(&mut u64) -> (f64, f64),
    real: impl Fn(&mut u64) -> f64,
) -> (Vec<f64>, Vec<f64>) {
    let bdiag: Vec<f64> = (0..n).map(|_| 1.0 + splitmix(s)).collect();
    let mut q = vec![0.0; n * n];
    for i in 0..n {
        q[i * n + i] = 1.0;
        for j in 0..i {
            q[i * n + j] = 0.2 * splitmix(s) - 0.1;
        }
    }
    let mut qi = vec![0.0; n * n];
    for c in 0..n {
        for i in 0..n {
            let mut v = if i == c { 1.0 } else { 0.0 };
            for j in 0..i {
                v -= q[i * n + j] * qi[j * n + c];
            }
            qi[i * n + c] = v;
        }
    }
    let mut d = vec![0.0; n * n];
    for blk in 0..blocks {
        let (a, c) = block(s);
        let i = 2 * blk;
        d[i * n + i] = a;
        d[(i + 1) * n + i + 1] = a;
        d[i * n + i + 1] = c;
        d[(i + 1) * n + i] = -c;
    }
    for i in 2 * blocks..n {
        d[i * n + i] = real(s);
    }
    let mut a = matmul(&matmul(&q, &d, n), &qi, n);
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] *= bdiag[i];
        }
    }
    let mut b = vec![0.0; n * n];
    for i in 0..n {
        b[i * n + i] = bdiag[i];
    }
    (a, b)
}

fn make(name: &'static str, n: usize, a: Vec<f64>, b: Vec<f64>, shift: f64, count: usize) -> Pencil {
    Pencil {
        name,
        n,
        a: dense_to_csr(&a, n),
        b: dense_to_csr(&b, n),
        a_dense: a,
        b_dense: b,
        shift,
        count,
    }
}

/// K1 of ADR 0580: order 120, seed `0x0580_0001`, 8 nearest 0.3.
pub fn k1() -> Pencil {
    let mut s = 0x0580_0001u64;
    let (a, b) = similar(
        120,
        &mut s,
        20,
        |s| {
            let a = 4.0 * splitmix(s) - 2.0;
            let c = 0.1 + 0.9 * splitmix(s);
            (a, c)
        },
        |s| 4.0 * splitmix(s) - 2.0,
    );
    make("K1", 120, a, b, 0.3, 8)
}

/// K2 of ADR 0580: the 1-D Laplacian of order 200, `B = I`, 5 nearest 0.
pub fn k2() -> Pencil {
    let n = 200;
    let mut a = vec![0.0; n * n];
    let mut b = vec![0.0; n * n];
    for i in 0..n {
        a[i * n + i] = 2.0;
        b[i * n + i] = 1.0;
        if i > 0 {
            a[i * n + i - 1] = -1.0;
            a[(i - 1) * n + i] = -1.0;
        }
    }
    make("K2", n, a, b, 0.0, 5)
}

/// K5c of ADR 0587 D3: order 60, seed `0x0587_0001`, 10 complex blocks `a in (0.1, 1)`, `c in (0.05, 0.5)`, 40 real values
/// in `(2, 5)`, 6 nearest 0.
pub fn k5c() -> Pencil {
    let mut s = 0x0587_0001u64;
    let (a, b) = similar(
        60,
        &mut s,
        10,
        |s| {
            let a = 0.1 + 0.9 * splitmix(s);
            let c = 0.05 + 0.45 * splitmix(s);
            (a, c)
        },
        |s| 2.0 + 3.0 * splitmix(s),
    );
    make("K5c", 60, a, b, 0.0, 6)
}

pub fn all() -> Vec<Pencil> {
    vec![k1(), k2(), k5c()]
}
