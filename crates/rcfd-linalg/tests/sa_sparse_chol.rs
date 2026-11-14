//! F-431 (ADR 0447): sparse `L D L^T` after approximate minimum degree. Checks A1 to A9 and the negative controls N1 and N3
//! of the registration.

use rcfd_linalg::skyline::{envelope_size, PivotRule, SkylineError, SkylineLdlt};
use rcfd_linalg::sparse_chol::{factor_count, minimum_degree_order, SparseLdlt, SparseOptions, SparseOrdering};
use rcfd_linalg::CsrMatrix;
use std::time::Instant;

const SEED: u64 = 0xF431_0447;

fn lcg(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s
}

/// The registered right-hand side: `b_i = 2 u_i - 1`.
fn rhs(n: usize) -> Vec<f64> {
    let mut s = SEED;
    (0..n)
        .map(|_| 2.0 * ((lcg(&mut s) >> 11) as f64 / (1u64 << 53) as f64) - 1.0)
        .collect()
}

/// The 5-point Dirichlet Laplacian on `m x m` with the diagonal `diag(row)`.
fn lap5_with(m: usize, diag: impl Fn(usize) -> f64) -> CsrMatrix {
    let mut t = Vec::new();
    for j in 0..m {
        for i in 0..m {
            let r = j * m + i;
            t.push((r, r, diag(r)));
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

fn lap5(m: usize) -> CsrMatrix {
    lap5_with(m, |_| 4.0)
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

/// The tridiagonal (2, -1) of order `n` minus `s I`.
fn tridiag_shifted(n: usize, s: f64) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        t.push((i, i, 2.0 - s));
        if i > 0 {
            t.push((i, i - 1, -1.0));
        }
        if i + 1 < n {
            t.push((i, i + 1, -1.0));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn relative_residual(a: &CsrMatrix, b: &[f64], x: &[f64]) -> f64 {
    let mut r = vec![0.0; b.len()];
    a.residual(b, x, &mut r);
    norm(&r) / norm(b)
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// `lambda_j = 4 sin^2(j pi / (2 (n + 1)))`, `j = 1..=n`.
fn tridiag_eigenvalue(n: usize, j: usize) -> f64 {
    let s = (j as f64 * std::f64::consts::PI / (2.0 * (n as f64 + 1.0))).sin();
    4.0 * s * s
}

/// The three registered shifts: midpoints of `(lambda_10, lambda_11)`, `(lambda_100, lambda_101)`, `(lambda_190, lambda_191)`.
fn shifts(n: usize) -> [f64; 3] {
    [10, 100, 190].map(|j| 0.5 * (tridiag_eigenvalue(n, j) + tridiag_eigenvalue(n, j + 1)))
}

fn nonzero() -> SparseOptions {
    SparseOptions {
        pivots: PivotRule::NonZero,
        ..SparseOptions::default()
    }
}

#[test]
fn a1_five_point_100_residual_and_a3r_counts_and_a8_against_the_envelope() {
    let a = lap5(100);
    let b = rhs(a.n);
    let t = Instant::now();
    let f = SparseLdlt::factor(&a).expect("A1 factor");
    let x = f.solve(&b);
    let secs = t.elapsed().as_secs_f64();
    let r = relative_residual(&a, &b, &x);
    let sky = SkylineLdlt::factor(&a).expect("envelope");
    println!(
        "A1: 5-point 100 x 100: relative residual {r:.3e} (limit < 1e-12), factor and solve {secs:.3} s; A3r: nnz(L) {}, \
         RCM envelope {}, ratio {:.4}",
        f.factor_entries(),
        sky.envelope_size(),
        f.factor_entries() as f64 / sky.envelope_size() as f64
    );
    assert!(r < 1e-12, "A1 residual {r:e}");
    // A8: agreement with the envelope solve; A4 (second part): the symbolic count equals the numeric one.
    let xe = sky.solve(&b);
    let xmax = xe.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let diff = x.iter().zip(&xe).fold(0.0f64, |m, (p, q)| m.max((p - q).abs())) / xmax;
    println!("A8: max |x_sparse - x_envelope| / max |x_envelope| = {diff:.3e} (limit 1e-10)");
    assert!(diff <= 1e-10, "A8 {diff:e}");
    let count = factor_count(&a, f.order());
    println!("A4b: factor_count {count} = factor_entries {}", f.factor_entries());
    assert_eq!(count, f.factor_entries(), "A4b");
}

#[test]
fn a2_a3_seven_point_30_residual_and_fill() {
    let a = lap7(30);
    let b = rhs(a.n);
    let t = Instant::now();
    let f = SparseLdlt::factor(&a).expect("A2 factor");
    let tf = t.elapsed().as_secs_f64();
    let x = f.solve(&b);
    let r = relative_residual(&a, &b, &x);
    println!("A2: 7-point 30^3: relative residual {r:.3e} (limit < 1e-12), factor {tf:.3} s");
    let sky = SkylineLdlt::factor(&a).expect("envelope");
    let (nnz, env) = (f.factor_entries(), sky.envelope_size());
    let ratio = nnz as f64 / env as f64;
    println!(
        "A3: 7-point 30^3: nnz(L) = {nnz} (minimum degree), envelope after RCM = {env}, ratio {ratio:.4} (limit <= 0.5); \
         factor bytes {} against {}",
        f.storage_bytes(),
        sky.storage_bytes()
    );
    assert!(r < 1e-12, "A2 residual {r:e}");
    assert!(2 * nnz <= env, "A3 ratio {ratio}");
    // N1: the natural order fills more than half the envelope (the check can fail).
    let natural: Vec<usize> = (0..a.n).collect();
    let nat = factor_count(&a, &natural);
    println!(
        "N1: natural order nnz(L) = {nat}, ratio {:.4} (must be > 0.5)",
        nat as f64 / env as f64
    );
    assert!(2 * nat > env, "N1");
}

#[test]
#[ignore = "record of the registered run of ADR 0447 A4: the closed form 7 790 was wrong (the natural envelope is 7 619); superseded by a4_natural_order_fills_the_band_amended (amendment 1)"]
fn a4_natural_order_fills_the_band() {
    let a = lap5(20);
    let f = SparseLdlt::factor_with(
        &a,
        &SparseOptions {
            ordering: SparseOrdering::Natural,
            ..SparseOptions::default()
        },
    )
    .expect("A4");
    let env = envelope_size(&a, None);
    let closed: usize = (0..400).map(|i: usize| i.min(20)).sum();
    println!(
        "A4: natural order 5-point 20 x 20: nnz(L) {}, natural envelope {env}, closed form {closed}",
        f.factor_entries()
    );
    assert_eq!(closed, 7790);
    assert_eq!(env, closed, "A4 envelope");
    assert_eq!(f.factor_entries(), closed, "A4 nnz(L)");
}

#[test]
#[ignore = "record of the registered run of ADR 0447 A5: the middle shift is exactly 2, where every diagonal of K - s I is 0 (singular for an unpivoted factorisation); superseded by a5_inertia_of_the_shifted_tridiagonal_and_n3_amended (amendment 1)"]
fn a5_inertia_of_the_shifted_tridiagonal_and_n3() {
    let n = 200;
    for (q, s) in shifts(n).into_iter().enumerate() {
        let expected = (1..=n).filter(|&j| tridiag_eigenvalue(n, j) < s).count();
        let f = SparseLdlt::factor_with(&tridiag_shifted(n, s), &nonzero()).expect("A5 factor");
        let (neg, pos) = f.inertia();
        println!("A5: shift {s:.6}: negative pivots {neg}, positive {pos}, eigenvalues below {expected}");
        assert_eq!(expected, [10, 100, 190][q], "the shifts are where registered");
        assert_eq!(neg, expected, "A5 at {s}");
    }
    // N3: the order reversed gives the same count.
    let s = shifts(n)[0];
    let rev: Vec<usize> = (0..n).rev().collect();
    let f = SparseLdlt::factor_with(
        &tridiag_shifted(n, s),
        &SparseOptions {
            ordering: SparseOrdering::Given(rev),
            ..nonzero()
        },
    )
    .expect("N3 factor");
    println!("N3: reversed order: negative pivots {}", f.inertia().0);
    assert_eq!(f.inertia().0, 10, "N3");
}

#[test]
fn a6_not_positive_definite_is_reported_with_the_equation() {
    // A6a
    let a = lap5_with(20, |r| if r == 137 { -1.0 } else { 4.0 });
    let e = SparseLdlt::factor(&a).unwrap_err();
    println!("A6a: {e}");
    match &e {
        SkylineError::Pivot { row, pivot, .. } => {
            assert_eq!(*row, 137, "A6a row");
            assert!(*pivot < 0.0, "A6a pivot {pivot}");
        }
        other => panic!("A6a: expected Pivot, got {other:?}"),
    }
    let msg = e.to_string();
    assert!(
        msg.contains("row 137") && msg.contains("not positive definite"),
        "A6a message"
    );
    // A6b
    let k = tridiag_shifted(200, shifts(200)[0]);
    let e = SparseLdlt::factor(&k).unwrap_err();
    println!("A6b: {e}");
    match e {
        SkylineError::Pivot {
            row, pivot, threshold, ..
        } => {
            assert!(row < 200, "A6b row");
            assert!(pivot <= threshold, "A6b pivot");
        }
        other => panic!("A6b: expected Pivot, got {other:?}"),
    }
    assert!(SparseLdlt::factor_with(&k, &nonzero()).is_ok(), "A6b NonZero accepted");
}

#[test]
fn a7_bit_identical_for_1_and_8_threads() {
    let a = lap7(20);
    let b = rhs(a.n);
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let f = SparseLdlt::factor(&a).expect("A7");
            let x = f.solve(&b);
            (f, x)
        })
    };
    let (f1, x1) = run(1);
    let (f8, x8) = run(8);
    assert_eq!(f1.order(), f8.order(), "A7 order");
    assert_eq!(f1.parent(), f8.parent(), "A7 parent");
    for j in 0..a.n {
        let (r1, v1) = f1.column(j);
        let (r8, v8) = f8.column(j);
        assert_eq!(r1, r8, "A7 rows of column {j}");
        assert_eq!(bits(v1), bits(v8), "A7 values of column {j}");
    }
    assert_eq!(bits(f1.pivots()), bits(f8.pivots()), "A7 D");
    assert_eq!(bits(&x1), bits(&x8), "A7 x");
    println!(
        "A7: order, tree, L ({} values), D and x bit-identical for 1 and 8 threads",
        f1.factor_entries()
    );
}

#[test]
fn a8_solve_many_equals_single_solves() {
    let a = lap5(30);
    let f = SparseLdlt::factor(&a).unwrap();
    let mut bs = Vec::new();
    for q in 0..3 {
        let mut b = rhs(a.n);
        b.rotate_left(q * 7);
        bs.push(b);
    }
    let many = f.solve_many(&bs);
    for (b, x) in bs.iter().zip(&many) {
        assert_eq!(bits(&f.solve(b)), bits(x), "A8 solve_many");
    }
    println!("A8: solve_many of three right-hand sides bit-equal to three solves");
}

#[test]
fn a9_refusals() {
    let ns = CsrMatrix {
        n: 2,
        ncols: 3,
        row_ptr: vec![0, 1, 2],
        col: vec![0, 1],
        val: vec![1.0, 1.0],
    };
    let e = SparseLdlt::factor(&ns).unwrap_err();
    println!("A9: {e}");
    assert!(matches!(e, SkylineError::NotSquare { rows: 2, cols: 3 }), "{e:?}");
    let mut t: Vec<(usize, usize, f64)> = (0..6).map(|i| (i, i, 4.0)).collect();
    t.push((3, 4, f64::NAN));
    t.push((4, 3, -1.0));
    let e = SparseLdlt::factor(&CsrMatrix::from_triplets(6, t)).unwrap_err();
    println!("A9: {e}");
    assert!(matches!(e, SkylineError::NonFinite { row: 3, col: 4, .. }), "{e:?}");
    let mut t: Vec<(usize, usize, f64)> = (0..6).map(|i| (i, i, 4.0)).collect();
    t.push((1, 2, -1.0));
    t.push((2, 1, -2.0));
    let e = SparseLdlt::factor(&CsrMatrix::from_triplets(6, t)).unwrap_err();
    println!("A9: {e}");
    assert!(matches!(e, SkylineError::Unsymmetric { row: 1, col: 2, .. }), "{e:?}");
    let e = SparseLdlt::factor_with(
        &lap5(3),
        &SparseOptions {
            ordering: SparseOrdering::Given(vec![0, 1, 2, 3, 4, 5, 6, 7, 7]),
            ..SparseOptions::default()
        },
    )
    .unwrap_err();
    println!("A9: {e}");
    assert!(matches!(e, SkylineError::BadOrdering { .. }), "{e:?}");
}

#[test]
fn minimum_degree_order_is_a_permutation() {
    // Not a registered check: the ordering's output is a permutation on the grids of A1 and A7 (a helper for diagnosis).
    for a in [lap5(37), lap7(11)] {
        let o = minimum_degree_order(&a);
        let mut seen = vec![false; a.n];
        for &i in &o {
            assert!(!seen[i]);
            seen[i] = true;
        }
        assert_eq!(o.len(), a.n);
    }
}

// ------------------------------------------------------------------------------------------------- amendment 1

/// A4' (ADR 0447 amendment 1): the natural-order factor fills the band: `nnz(L)` = the natural envelope = 0 + 19 + 380 x 20.
#[test]
fn a4_natural_order_fills_the_band_amended() {
    let a = lap5(20);
    let f = SparseLdlt::factor_with(
        &a,
        &SparseOptions {
            ordering: SparseOrdering::Natural,
            ..SparseOptions::default()
        },
    )
    .expect("A4'");
    let env = envelope_size(&a, None);
    let closed = 19 + 380 * 20;
    println!(
        "A4': natural order 5-point 20 x 20: nnz(L) {}, natural envelope {env}, closed form {closed}",
        f.factor_entries()
    );
    assert_eq!(env, closed, "A4' envelope");
    assert_eq!(f.factor_entries(), closed, "A4' nnz(L)");
}

/// A5' (ADR 0447 amendment 1): shifts at the midpoints of `(lambda_10, lambda_11)`, `(lambda_95, lambda_96)`,
/// `(lambda_190, lambda_191)`; then N3 unchanged.
#[test]
fn a5_inertia_of_the_shifted_tridiagonal_and_n3_amended() {
    let n = 200;
    let mid = |j: usize| 0.5 * (tridiag_eigenvalue(n, j) + tridiag_eigenvalue(n, j + 1));
    for (j, want) in [(10, 10), (95, 95), (190, 190)] {
        let s = mid(j);
        let expected = (1..=n).filter(|&q| tridiag_eigenvalue(n, q) < s).count();
        let f = SparseLdlt::factor_with(&tridiag_shifted(n, s), &nonzero()).expect("A5' factor");
        let (neg, pos) = f.inertia();
        println!("A5': shift {s:.6}: negative pivots {neg}, positive {pos}, eigenvalues below {expected}");
        assert_eq!(expected, want, "the shifts are where registered");
        assert_eq!(neg, expected, "A5' at {s}");
    }
    let s = shifts(n)[0];
    let rev: Vec<usize> = (0..n).rev().collect();
    let f = SparseLdlt::factor_with(
        &tridiag_shifted(n, s),
        &SparseOptions {
            ordering: SparseOrdering::Given(rev),
            ..nonzero()
        },
    )
    .expect("N3 factor");
    println!("N3: reversed order: negative pivots {}", f.inertia().0);
    assert_eq!(f.inertia().0, 10, "N3");
}
