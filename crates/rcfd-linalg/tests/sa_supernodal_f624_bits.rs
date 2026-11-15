//! F-624 (ADR 0566) check B, records G1 and G2: the bits of `SupernodalLdlt::factor` and `ComplexSupernodalLdlt::factor`
//! (pivots and one solve) on fixed matrices, recorded on b29c1383 before any library code of F-624 and asserted unchanged
//! after the split of the supernodal factor into its symbolic and numeric phases (ADR 0566 D1).

use rcfd_linalg::complex::C64;
use rcfd_linalg::cskyline::ComplexCsr;
use rcfd_linalg::sparse_chol::{ComplexSupernodalLdlt, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// The 7-point Dirichlet Laplacian on `m^3` (diagonal 6).
fn lap7(m: usize) -> CsrMatrix {
    let id = |i: usize, j: usize, k: usize| i + m * j + m * m * k;
    let mut t = Vec::new();
    for k in 0..m {
        for j in 0..m {
            for i in 0..m {
                let p = id(i, j, k);
                t.push((p, p, 6.0));
                let mut nb = |q: usize| t.push((p, q, -1.0));
                if i > 0 {
                    nb(id(i - 1, j, k));
                }
                if i + 1 < m {
                    nb(id(i + 1, j, k));
                }
                if j > 0 {
                    nb(id(i, j - 1, k));
                }
                if j + 1 < m {
                    nb(id(i, j + 1, k));
                }
                if k > 0 {
                    nb(id(i, j, k - 1));
                }
                if k + 1 < m {
                    nb(id(i, j, k + 1));
                }
            }
        }
    }
    CsrMatrix::from_triplets(m * m * m, t)
}

/// The 5-point Dirichlet Laplacian on `m x m` (diagonal 4).
fn lap5(m: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for j in 0..m {
        for i in 0..m {
            let r = j * m + i;
            t.push((r, r, 4.0));
            if i > 0 {
                t.push((r, r - 1, -1.0));
            }
            if i + 1 < m {
                t.push((r, r + 1, -1.0));
            }
            if j > 0 {
                t.push((r, r - m, -1.0));
            }
            if j + 1 < m {
                t.push((r, r + m, -1.0));
            }
        }
    }
    CsrMatrix::from_triplets(m * m, t)
}

/// ADR 0566 A3's second matrix: `a` with every stored entry `k` scaled by `1 + 0.1 sin(k)`, then each pair `(r, c)`, `(c, r)`
/// replaced by its mean (the same pattern, other values, symmetric).
fn rescaled(a: &CsrMatrix) -> CsrMatrix {
    let mut b = a.clone();
    for (k, v) in b.val.iter_mut().enumerate() {
        *v *= 1.0 + 0.1 * (k as f64).sin();
    }
    let mut out = b.clone();
    for r in 0..b.n {
        for k in b.row_ptr[r]..b.row_ptr[r + 1] {
            let c = b.col[k];
            let row = &b.col[b.row_ptr[c]..b.row_ptr[c + 1]];
            let m = b.row_ptr[c] + row.binary_search(&r).expect("symmetric pattern");
            out.val[k] = 0.5 * (b.val[k] + b.val[m]);
        }
    }
    out
}

/// The fixed right-hand side `b_i = sin(i + 1)`.
fn rhs(n: usize) -> Vec<f64> {
    (0..n).map(|i| ((i + 1) as f64).sin()).collect()
}

fn real_record(a: &CsrMatrix) -> u64 {
    let f = SupernodalLdlt::factor(a).expect("positive definite");
    let mut v: Vec<f64> = f.pivots().to_vec();
    v.extend(f.solve(&rhs(a.n)));
    v.extend(f.order().iter().map(|&o| o as f64));
    digest(&v)
}

fn complex_record(a: &ComplexCsr) -> u64 {
    let f = ComplexSupernodalLdlt::factor(a).expect("factorable");
    let n = f.n();
    let b: Vec<C64> = rhs(n).iter().map(|&x| C64 { re: x, im: 0.5 * x }).collect();
    let mut v: Vec<f64> = f.pivots().iter().flat_map(|z| [z.re, z.im]).collect();
    v.extend(f.solve(&b).iter().flat_map(|z| [z.re, z.im]));
    digest(&v)
}

/// G1 and G2 of ADR 0566 check B.
#[test]
fn f624_b_g1_g2_supernodal_bits() {
    let k7 = lap7(10);
    let g1 = [real_record(&k7), real_record(&rescaled(&k7)), real_record(&lap5(40))];
    let k8 = lap7(8);
    let mut im = k8.clone();
    for v in im.val.iter_mut() {
        *v *= 0.1;
    }
    let g2 = complex_record(&ComplexCsr::from_parts(&k8, &im).expect("same pattern"));
    println!("G1 {:#018x} {:#018x} {:#018x}; G2 {g2:#018x}", g1[0], g1[1], g1[2]);
    assert_eq!(g1, G1, "G1: the real supernodal factor's bits");
    assert_eq!(g2, G2, "G2: the complex supernodal factor's bits");
}

/// Recorded on b29c1383 (`f624-scratch/bits-base-linalg.log`).
const G1: [u64; 3] = [0xa9e4_99ca_732e_8861, 0xfb76_0756_993e_305d, 0x380c_d6df_de2f_3fe6];
const G2: u64 = 0x6874_dfe0_129f_6fbf;
