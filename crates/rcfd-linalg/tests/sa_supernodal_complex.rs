//! F-594 (ADR 0509): the complex symmetric supernodal `L D L^T` and the supernodal factor in the eigensolvers. Checks L1 to
//! L5 and the negative control N4 of the registration.

use rcfd_linalg::complex::C64;
use rcfd_linalg::cskyline::{ComplexCsr, ComplexSkylineError, ComplexSkylineLdlt};
use rcfd_linalg::eigen::{
    lowest_buckling_factors, lowest_eigenpairs, BucklingEigenOptions, Factorisation, SubspaceOptions,
};
use rcfd_linalg::sparse_chol::{ComplexSupernodalLdlt, ComplexSupernodalOptions, SparseOrdering, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

const SEED: u64 = 0xF594_0509;

fn lcg(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s
}

fn unit(s: &mut u64) -> f64 {
    2.0 * ((lcg(s) >> 11) as f64 / (1u64 << 53) as f64) - 1.0
}

/// The registered complex right-hand side `(2 u - 1) + i (2 u' - 1)`.
fn crhs(n: usize) -> Vec<C64> {
    let mut s = SEED;
    (0..n)
        .map(|_| {
            let re = unit(&mut s);
            let im = unit(&mut s);
            C64::new(re, im)
        })
        .collect()
}

/// The real right-hand side `2 u - 1`.
fn rhs(n: usize) -> Vec<f64> {
    let mut s = SEED;
    (0..n).map(|_| unit(&mut s)).collect()
}

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

/// `a + shift I` with `shift` added to every diagonal entry (the diagonal is stored).
fn shifted(a: &CsrMatrix, shift: C64) -> ComplexCsr {
    let mut c = ComplexCsr::from_real(a);
    for r in 0..c.n {
        for k in c.row_ptr[r]..c.row_ptr[r + 1] {
            if c.col[k] == r {
                c.val[k] += shift;
            }
        }
    }
    c
}

fn cdiff(x: &[C64], y: &[C64]) -> f64 {
    let scale = y.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    x.iter().zip(y).fold(0.0f64, |m, (a, b)| m.max((*a - *b).abs())) / scale
}

fn cresidual(a: &ComplexCsr, x: &[C64], b: &[C64]) -> f64 {
    let ax = a.matvec(x);
    let r: f64 = ax.iter().zip(b).map(|(p, q)| (*q - *p).norm_sqr()).sum::<f64>().sqrt();
    let bn: f64 = b.iter().map(|v| v.norm_sqr()).sum::<f64>().sqrt();
    r / bn
}

fn cbits(v: &[C64]) -> Vec<(u64, u64)> {
    v.iter().map(|z| (z.re.to_bits(), z.im.to_bits())).collect()
}

fn l1_matrices() -> [(&'static str, ComplexCsr); 2] {
    let a = lap7(20);
    [
        ("(a) L7 + 0.5 i I", shifted(&a, C64::new(0.0, 0.5))),
        ("(b) L7 - 3 I + 0.5 i I", shifted(&a, C64::new(-3.0, 0.5))),
    ]
}

#[test]
fn l1_complex_residual_and_agreement_with_the_envelope_and_n4() {
    for (name, a) in l1_matrices() {
        let b = crhs(a.n);
        let f = ComplexSupernodalLdlt::factor(&a).unwrap();
        let x = f.solve(&b);
        let res = cresidual(&a, &x, &b);
        let e = ComplexSkylineLdlt::factor(&a).unwrap();
        let xe = e.solve(&b);
        let diff = cdiff(&x, &xe);
        println!(
            "L1 {name}: n {}, relative residual {res:.3e} (limit 1e-12), against the envelope {diff:.3e} (limit 1e-11); \
             {} supernodes, largest front {}, {} fronts split, nnz(L) {}",
            a.n,
            f.supernodes(),
            f.max_front(),
            f.split_fronts(),
            f.factor_entries()
        );
        assert!(res.is_finite() && diff.is_finite(), "L1 finite");
        assert!(res < 1e-12, "L1 {name} residual {res:e}");
        assert!(diff <= 1e-11, "L1 {name} difference {diff:e}");
        if name.starts_with("(b)") {
            let same = cbits(&x) == cbits(&xe);
            println!("N4: supernodal solution bit-equal to the envelope's: {same} (must be false)");
            assert!(!same, "N4");
        }
    }
}

#[test]
fn l2_a_real_matrix_gives_the_real_factor() {
    let a = lap7(20);
    let r = SupernodalLdlt::factor(&a).unwrap();
    let c = ComplexSupernodalLdlt::factor(&ComplexCsr::from_real(&a)).unwrap();
    assert_eq!(r.order(), c.order(), "L2 order");
    let (mut values, mut zero_im) = (0usize, true);
    for j in 0..a.n {
        let (rr, rv) = r.column(j);
        let (cr, cv) = c.column(j);
        assert_eq!(rr, cr, "L2 rows of column {j}");
        for (x, z) in rv.iter().zip(cv) {
            assert!(*x == z.re, "L2 L value in column {j}: {x:e} against {:e}", z.re);
            zero_im &= z.im == 0.0;
            values += 1;
        }
    }
    for (x, z) in r.pivots().iter().zip(c.pivots()) {
        assert!(*x == z.re, "L2 D");
        zero_im &= z.im == 0.0;
    }
    let b = rhs(a.n);
    let xr = r.solve(&b);
    let bc: Vec<C64> = b.iter().map(|&v| C64::from_real(v)).collect();
    let xc = c.solve(&bc);
    for (x, z) in xr.iter().zip(&xc) {
        assert!(*x == z.re, "L2 solution");
        zero_im &= z.im == 0.0;
    }
    println!(
        "L2: {values} values of L, D and the solution equal to the real factor's; imaginary parts zero: {zero_im}"
    );
    assert!(zero_im, "L2 imaginary parts");
}

#[test]
fn l3_bit_identical_for_1_and_8_threads() {
    let [_, (_, a)] = l1_matrices();
    let b = crhs(a.n);
    type Record = (
        Vec<usize>,
        Vec<Vec<usize>>,
        Vec<Vec<(u64, u64)>>,
        Vec<(u64, u64)>,
        Vec<(u64, u64)>,
        usize,
    );
    let run = |threads: usize| -> Record {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let f = ComplexSupernodalLdlt::factor(&a).unwrap();
            let x = f.solve(&b);
            let (mut rows, mut vals) = (Vec::new(), Vec::new());
            for j in 0..f.n() {
                let (r, v) = f.column(j);
                rows.push(r.to_vec());
                vals.push(cbits(v));
            }
            (
                f.order().to_vec(),
                rows,
                vals,
                cbits(f.pivots()),
                cbits(&x),
                f.split_fronts(),
            )
        })
    };
    let r1 = run(1);
    let r8 = run(8);
    assert_eq!(r1.0, r8.0, "L3 order");
    assert_eq!(r1.1, r8.1, "L3 rows");
    assert_eq!(r1.2, r8.2, "L3 values");
    assert_eq!(r1.3, r8.3, "L3 D");
    assert_eq!(r1.4, r8.4, "L3 x");
    println!(
        "L3: order, L, D and x bit-identical for 1 and 8 threads; {} fronts split",
        r8.5
    );
    assert!(r8.5 > 0, "L3 split fronts");
}

fn options(ordering: SparseOrdering, tau: f64) -> ComplexSupernodalOptions {
    ComplexSupernodalOptions {
        ordering,
        min_relative_pivot: tau,
        ..ComplexSupernodalOptions::default()
    }
}

#[test]
fn l4_refusals() {
    // (a) the singular 2 x 2 in the reversed order: equation 0 at position 1
    let a = ComplexCsr::new(
        2,
        vec![0, 2, 4],
        vec![0, 1, 0, 1],
        vec![C64::ONE, C64::ONE, C64::ONE, C64::ONE],
    )
    .unwrap();
    match ComplexSupernodalLdlt::factor_with(&a, &options(SparseOrdering::Given(vec![1, 0]), 0.0)) {
        Err(
            e @ ComplexSkylineError::Pivot {
                row: 0, position: 1, ..
            },
        ) => println!("L4a: {e}"),
        other => panic!("L4a: {other:?}"),
    }
    // (b) tau = 1 refuses the first pivot
    let g = ComplexCsr::from_real(&lap5(20));
    let order0 = ComplexSupernodalLdlt::factor(&g).unwrap().order()[0];
    match ComplexSupernodalLdlt::factor_with(&g, &options(SparseOrdering::NestedDissection, 1.0)) {
        Err(
            e @ ComplexSkylineError::Pivot {
                row,
                position: 0,
                threshold,
                scale,
                ..
            },
        ) if row == order0 && threshold == scale => println!("L4b: {e}"),
        other => panic!("L4b: {other:?}"),
    }
    // (c) NaN at (3, 4)
    let mut c = g.clone();
    let k = (c.row_ptr[3]..c.row_ptr[4]).find(|&k| c.col[k] == 4).unwrap();
    c.val[k] = C64::new(f64::NAN, 0.0);
    match ComplexSupernodalLdlt::factor(&c) {
        Err(ComplexSkylineError::NonFinite { row: 3, col: 4, .. }) => println!("L4c: NonFinite (3, 4)"),
        other => panic!("L4c: {other:?}"),
    }
    // (d) a_12 = -1, a_21 = -2
    let mut d = g.clone();
    let k = (d.row_ptr[2]..d.row_ptr[3]).find(|&k| d.col[k] == 1).unwrap();
    d.val[k] = C64::new(-2.0, 0.0);
    match ComplexSupernodalLdlt::factor(&d) {
        Err(ComplexSkylineError::Unsymmetric { row: 1, col: 2, .. }) => println!("L4d: Unsymmetric (1, 2)"),
        other => panic!("L4d: {other:?}"),
    }
    // (e) a repeated index
    let mut bad: Vec<usize> = (0..g.n).collect();
    bad[5] = 4;
    match ComplexSupernodalLdlt::factor_with(&g, &options(SparseOrdering::Given(bad), 0.0)) {
        Err(e @ ComplexSkylineError::BadOrdering { .. }) => println!("L4e: {e}"),
        other => panic!("L4e: {other:?}"),
    }
    // (f) tau = -1
    match ComplexSupernodalLdlt::factor_with(&g, &options(SparseOrdering::NestedDissection, -1.0)) {
        Err(e @ ComplexSkylineError::Malformed { .. }) => println!("L4f: {e}"),
        other => panic!("L4f: {other:?}"),
    }
}

fn tridiagonal(n: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        t.push((i, i, 2.0));
        if i > 0 {
            t.push((i, i - 1, -1.0));
        }
        if i + 1 < n {
            t.push((i, i + 1, -1.0));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn diagonal(n: usize, v: f64) -> CsrMatrix {
    CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, v)).collect())
}

#[test]
fn l5_the_eigensolvers_with_the_supernodal_factor() {
    let n = 200;
    let k = tridiagonal(n);
    let exact = |j: usize| 2.0 - 2.0 * (j as f64 * PI / (n as f64 + 1.0)).cos();
    let opts = SubspaceOptions {
        factorisation: Factorisation::Supernodal,
        ..SubspaceOptions::default()
    };
    let e = lowest_eigenpairs(&k, &diagonal(n, 1.0), 5, &opts).unwrap();
    let err = e
        .values
        .iter()
        .enumerate()
        .fold(0.0f64, |m, (j, &l)| m.max(((l - exact(j + 1)) / exact(j + 1)).abs()));
    println!(
        "L5a: {:?}, max relative error {err:.3e} (limit 1e-9), Sturm {:?}, {} iterations",
        e.values, e.sturm, e.iterations
    );
    assert!(err.is_finite() && err <= 1e-9, "L5a {err:e}");
    assert_eq!(e.sturm.counted, e.sturm.expected, "L5a Sturm");
    let bopts = BucklingEigenOptions {
        factorisation: Factorisation::Supernodal,
        ..BucklingEigenOptions::default()
    };
    let b = lowest_buckling_factors(&k, &diagonal(n, -1.0), 3, &bopts).unwrap();
    assert_eq!(b.values.len(), 3, "L5b count");
    let err = b
        .values
        .iter()
        .enumerate()
        .fold(0.0f64, |m, (j, &l)| m.max(((l - exact(j + 1)) / exact(j + 1)).abs()));
    println!(
        "L5b: {:?}, max relative error {err:.3e} (limit 1e-9), Sturm {:?}",
        b.values, b.sturm
    );
    assert!(err.is_finite() && err <= 1e-9, "L5b {err:e}");
}
