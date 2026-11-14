//! F-529 (ADR 0503): nested dissection and the supernodal multifrontal `L D L^T`. Checks T1 to T10 and the negative
//! controls N1 to N3 of the registration.

use rcfd_linalg::skyline::{PivotRule, SkylineError};
use rcfd_linalg::sparse_chol::{
    factor_count, minimum_degree_order, nested_dissection_order, nested_dissection_order_with_points, SparseLdlt,
    SparseOptions, SparseOrdering, SupernodalLdlt,
};
use rcfd_linalg::CsrMatrix;
use std::time::Instant;

const SEED: u64 = 0xF529_0503;

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

/// The 5-point Dirichlet Laplacian on `m x m` with the diagonal `diag(row)`, unknowns offset by `base`, into `t`.
fn lap5_into(t: &mut Vec<(usize, usize, f64)>, m: usize, base: usize, diag: impl Fn(usize) -> f64) {
    for j in 0..m {
        for i in 0..m {
            let r = j * m + i;
            t.push((base + r, base + r, diag(r)));
            if i > 0 {
                t.push((base + r, base + r - 1, -1.0));
            }
            if i + 1 < m {
                t.push((base + r, base + r + 1, -1.0));
            }
            if j > 0 {
                t.push((base + r, base + r - m, -1.0));
            }
            if j + 1 < m {
                t.push((base + r, base + r + m, -1.0));
            }
        }
    }
}

fn lap5_with(m: usize, diag: impl Fn(usize) -> f64) -> CsrMatrix {
    let mut t = Vec::new();
    lap5_into(&mut t, m, 0, diag);
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

/// The grid points of `lap7(m)` and `lap5(m)`.
fn points3(m: usize) -> Vec<[f64; 3]> {
    (0..m * m * m)
        .map(|p| [(p % m) as f64, ((p / m) % m) as f64, (p / (m * m)) as f64])
        .collect()
}

fn points2(m: usize) -> Vec<[f64; 3]> {
    (0..m * m).map(|p| [(p % m) as f64, (p / m) as f64, 0.0]).collect()
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

/// `max |a_i - b_i| / max |b_i|`, asserted finite.
fn max_rel_diff(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    assert!(a.iter().chain(b).all(|v| v.is_finite()), "finite values");
    let d = a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    let s = b.iter().fold(0.0f64, |m, y| m.max(y.abs()));
    let r = d / s;
    assert!(r.is_finite(), "finite difference");
    r
}

fn is_permutation(o: &[usize], n: usize) -> bool {
    let mut seen = vec![false; n];
    o.len() == n && o.iter().all(|&i| i < n && !std::mem::replace(&mut seen[i], true))
}

fn given(order: Vec<usize>) -> SparseOptions {
    SparseOptions {
        ordering: SparseOrdering::Given(order),
        ..SparseOptions::default()
    }
}

fn position_of(order: &[usize], row: usize) -> usize {
    order.iter().position(|&o| o == row).expect("in the order")
}

#[test]
fn t1_nested_dissection_fill_against_minimum_degree_and_n1() {
    let a = lap7(30);
    let t = Instant::now();
    let nd = nested_dissection_order(&a);
    let tn = t.elapsed().as_secs_f64();
    assert!(is_permutation(&nd, a.n));
    let c_nd = factor_count(&a, &nd);
    let c_md = factor_count(&a, &minimum_degree_order(&a));
    let ratio = c_nd as f64 / c_md as f64;
    println!("T1: 7-point 30^3: nnz(L) nested dissection {c_nd}, minimum degree {c_md}, ratio {ratio:.4} (limit 0.6); ordering {tn:.3} s");
    let c_nat = factor_count(&a, &(0..a.n).collect::<Vec<_>>());
    let r_nat = c_nat as f64 / c_md as f64;
    println!("N1: natural order nnz(L) {c_nat}, ratio {r_nat:.4} (must exceed 0.6)");
    assert!(r_nat > 0.6, "N1");
    assert!(ratio.is_finite() && ratio <= 0.6, "T1 ratio {ratio}");
}

#[test]
fn t1r_records() {
    let a = lap7(30);
    let geo = nested_dissection_order_with_points(&a, &points3(30));
    assert!(is_permutation(&geo, a.n));
    println!(
        "T1r: 7-point 30^3 geometric nested dissection nnz(L) {}",
        factor_count(&a, &geo)
    );
    let a = lap5(100);
    let g = nested_dissection_order(&a);
    let p = nested_dissection_order_with_points(&a, &points2(100));
    assert!(is_permutation(&g, a.n) && is_permutation(&p, a.n));
    println!(
        "T1r: 5-point 100 x 100 nnz(L): level-structure ND {}, geometric ND {}, minimum degree {}",
        factor_count(&a, &g),
        factor_count(&a, &p),
        factor_count(&a, &minimum_degree_order(&a))
    );
}

#[test]
fn t2_t3_supernodal_30_cubed_residual_counts_against_up_looking_and_n2() {
    let a = lap7(30);
    let b = rhs(a.n);
    let t = Instant::now();
    let f = SupernodalLdlt::factor(&a).expect("T2 factor");
    let tf = t.elapsed().as_secs_f64();
    let x = f.solve(&b);
    let res = relative_residual(&a, &b, &x);
    println!(
        "T2: 7-point 30^3 supernodal: relative residual {res:.3e} (limit 1e-12); factor {tf:.3} s, {} supernodes, max front {}, \
         split fronts {}, peak front bytes {}, storage bytes {}",
        f.supernodes(),
        f.max_front(),
        f.split_fronts(),
        f.peak_front_bytes(),
        f.storage_bytes()
    );
    assert!(res.is_finite() && res < 1e-12, "T2 residual {res:e}");
    // T3
    let order = f.order().to_vec();
    let count = factor_count(&a, &order);
    let t = Instant::now();
    let u = SparseLdlt::factor_with(&a, &given(order)).expect("T3 up-looking");
    let tu = t.elapsed().as_secs_f64();
    let xu = u.solve(&b);
    let diff = max_rel_diff(&x, &xu);
    println!(
        "T3: factor_entries supernodal {}, factor_count {count}, up-looking {}; max |x_s - x_u| / max |x_u| = {diff:.3e} \
         (limit 1e-11); up-looking factor {tu:.3} s",
        f.factor_entries(),
        u.factor_entries()
    );
    assert_eq!(f.factor_entries(), count, "T3 count");
    assert_eq!(u.factor_entries(), count, "T3 up-looking count");
    assert!(diff <= 1e-11, "T3 difference {diff:e}");
    // N2
    let mut differ = 0usize;
    for j in 0..a.n {
        let (rs, vs) = f.column(j);
        let (ru, vu) = u.column(j);
        assert_eq!(rs, ru, "column {j} rows");
        differ += vs.iter().zip(vu).filter(|(p, q)| p.to_bits() != q.to_bits()).count();
    }
    println!("N2: {differ} values of L differ in their bits from the up-looking factor's");
    assert!(differ > 0, "N2");
}

#[test]
fn t4_supernodal_5_point_100() {
    let a = lap5(100);
    let b = rhs(a.n);
    let f = SupernodalLdlt::factor(&a).expect("T4");
    let res = relative_residual(&a, &b, &f.solve(&b));
    println!("T4: 5-point 100 x 100 supernodal: relative residual {res:.3e} (limit 1e-12)");
    assert!(res.is_finite() && res < 1e-12, "T4 {res:e}");
}

#[test]
fn t5_refusal_names_the_moved_equation_and_n3() {
    let a = lap5_with(20, |r| if r == 137 { -1.0 } else { 4.0 });
    let nd = nested_dissection_order(&a);
    let p = position_of(&nd, 137);
    // (a)
    match SupernodalLdlt::factor(&a) {
        Err(
            e @ SkylineError::Pivot {
                row, position, pivot, ..
            },
        ) => {
            let msg = e.to_string();
            println!("T5a: {msg}");
            assert_eq!(row, 137, "T5a row");
            assert_eq!(position, p, "T5a position");
            assert_ne!(position, 137, "T5a: the ordering moves the equation");
            assert!(pivot < 0.0, "T5a pivot");
            assert!(msg.contains("row 137"), "T5a message row");
            assert!(
                msg.contains(&format!("elimination position {p}")),
                "T5a message position"
            );
        }
        other => panic!("T5a: expected a pivot refusal, got {other:?}"),
    }
    // (b)
    let rev: Vec<usize> = (0..a.n).rev().collect();
    match SupernodalLdlt::factor_with(&a, &given(rev)) {
        Err(SkylineError::Pivot { row, position, .. }) => {
            println!("T5b: row {row}, position {position}");
            assert_eq!((row, position), (137, 262), "T5b");
        }
        other => panic!("T5b: expected a pivot refusal, got {other:?}"),
    }
    // (c)
    match SparseLdlt::factor_with(&a, &given(nd)) {
        Err(SkylineError::Pivot { row, position, .. }) => {
            println!("T5c: up-looking: row {row}, position {position}");
            assert_eq!((row, position), (137, p), "T5c");
        }
        other => panic!("T5c: expected a pivot refusal, got {other:?}"),
    }
    // N3
    let natural = SparseOptions {
        ordering: SparseOrdering::Natural,
        ..SparseOptions::default()
    };
    match SupernodalLdlt::factor_with(&a, &natural) {
        Err(SkylineError::Pivot { row, position, .. }) => {
            println!("N3: natural: row {row}, position {position}");
            assert_eq!((row, position), (137, 137), "N3");
        }
        other => panic!("N3: expected a pivot refusal, got {other:?}"),
    }
}

/// `lambda_j = 4 sin^2(j pi / (2 (n + 1)))`.
fn tridiag_eigenvalue(n: usize, j: usize) -> f64 {
    let s = (j as f64 * std::f64::consts::PI / (2.0 * (n as f64 + 1.0))).sin();
    4.0 * s * s
}

#[test]
fn t6_inertia_of_the_shifted_tridiagonal() {
    let n = 200;
    for (j, want) in [(10, 10), (95, 95), (190, 190)] {
        let s = 0.5 * (tridiag_eigenvalue(n, j) + tridiag_eigenvalue(n, j + 1));
        let expected = (1..=n).filter(|&i| tridiag_eigenvalue(n, i) < s).count();
        assert_eq!(expected, want, "closed form");
        let f = SupernodalLdlt::factor_with(
            &tridiag_shifted(n, s),
            &SparseOptions {
                ordering: SparseOrdering::NestedDissection,
                pivots: PivotRule::NonZero,
                ..SparseOptions::default()
            },
        )
        .expect("T6 factor");
        let (neg, pos) = f.inertia();
        println!("T6: s = {s:.6}: negative pivots {neg}, eigenvalues below {expected}");
        assert_eq!(neg + pos, n);
        assert_eq!(neg, expected, "T6 at s = {s}");
    }
}

#[test]
fn t7_bit_identical_for_1_and_8_threads() {
    let a = lap7(20);
    let b = rhs(a.n);
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let f = SupernodalLdlt::factor(&a).expect("T7");
            let x = f.solve(&b);
            (f, x)
        })
    };
    let (f1, x1) = run(1);
    let (f8, x8) = run(8);
    assert_eq!(f1.order(), f8.order(), "T7 order");
    for j in 0..a.n {
        let (r1, v1) = f1.column(j);
        let (r8, v8) = f8.column(j);
        assert_eq!(r1, r8, "T7 rows of column {j}");
        assert_eq!(bits(v1), bits(v8), "T7 values of column {j}");
    }
    assert_eq!(bits(f1.pivots()), bits(f8.pivots()), "T7 D");
    assert_eq!(bits(&x1), bits(&x8), "T7 x");
    println!(
        "T7: 7-point 20^3: order, L ({} values), D and x bit-identical for 1 and 8 threads; split fronts {}",
        f1.factor_entries(),
        f1.split_fronts()
    );
    assert!(f1.split_fronts() > 0, "T7: the split path ran");
}

#[test]
fn t8_refusals() {
    // the matrices of ADR 0447 A9
    let ns = CsrMatrix {
        n: 2,
        ncols: 3,
        row_ptr: vec![0, 1, 2],
        col: vec![0, 1],
        val: vec![1.0, 1.0],
    };
    let e = SupernodalLdlt::factor(&ns).unwrap_err();
    println!("T8: {e}");
    assert!(matches!(e, SkylineError::NotSquare { rows: 2, cols: 3 }), "{e:?}");
    let mut t: Vec<(usize, usize, f64)> = (0..6).map(|i| (i, i, 4.0)).collect();
    t.push((3, 4, f64::NAN));
    t.push((4, 3, -1.0));
    let e = SupernodalLdlt::factor(&CsrMatrix::from_triplets(6, t)).unwrap_err();
    println!("T8: {e}");
    assert!(matches!(e, SkylineError::NonFinite { row: 3, col: 4, .. }), "{e:?}");
    let mut t: Vec<(usize, usize, f64)> = (0..6).map(|i| (i, i, 4.0)).collect();
    t.push((1, 2, -1.0));
    t.push((2, 1, -2.0));
    let e = SupernodalLdlt::factor(&CsrMatrix::from_triplets(6, t)).unwrap_err();
    println!("T8: {e}");
    assert!(matches!(e, SkylineError::Unsymmetric { row: 1, col: 2, .. }), "{e:?}");
    let e = SupernodalLdlt::factor_with(&lap5(3), &given(vec![0, 1, 2, 3, 4, 5, 6, 7, 7])).unwrap_err();
    println!("T8: {e}");
    assert!(matches!(e, SkylineError::BadOrdering { .. }), "{e:?}");
}

#[test]
fn t9_orderings_and_solves_on_odd_graphs() {
    let mut cases: Vec<(&str, CsrMatrix)> = Vec::new();
    let mut t = Vec::new();
    lap5_into(&mut t, 10, 0, |_| 4.0);
    lap5_into(&mut t, 7, 100, |_| 4.0);
    cases.push(("two disconnected grids", CsrMatrix::from_triplets(149, t)));
    cases.push((
        "diagonal 50",
        CsrMatrix::from_triplets(50, (0..50).map(|i| (i, i, 1.0 + i as f64)).collect()),
    ));
    cases.push(("order 1", CsrMatrix::from_triplets(1, vec![(0, 0, 3.0)])));
    let mut dense = Vec::new();
    for i in 0..30 {
        for j in 0..30 {
            dense.push((i, j, if i == j { 31.0 } else { 1.0 }));
        }
    }
    cases.push(("dense 30", CsrMatrix::from_triplets(30, dense)));
    cases.push(("tridiagonal 500", tridiag_shifted(500, 0.0)));
    for (name, a) in &cases {
        let b = rhs(a.n);
        let pts: Vec<[f64; 3]> = (0..a.n).map(|i| [(i % 7) as f64, (i / 7) as f64, 0.5]).collect();
        let same: Vec<[f64; 3]> = vec![[1.0, 2.0, 3.0]; a.n];
        for (kind, order) in [
            ("levels", nested_dissection_order(a)),
            ("points", nested_dissection_order_with_points(a, &pts)),
            ("equal points", nested_dissection_order_with_points(a, &same)),
        ] {
            assert!(is_permutation(&order, a.n), "T9 {name} {kind}: a permutation");
            let f = SupernodalLdlt::factor_with(a, &given(order)).expect("T9 factor");
            let res = relative_residual(a, &b, &f.solve(&b));
            println!("T9: {name}, {kind}: relative residual {res:.3e}");
            assert!(res.is_finite() && res < 1e-12, "T9 {name} {kind}: {res:e}");
        }
    }
}

#[test]
fn t10_solve_many_equals_solves() {
    let a = lap5(30);
    let f = SupernodalLdlt::factor(&a).unwrap();
    let mut s = 7u64;
    let bs: Vec<Vec<f64>> = (0..3)
        .map(|_| {
            (0..a.n)
                .map(|_| (lcg(&mut s) >> 11) as f64 / (1u64 << 53) as f64)
                .collect()
        })
        .collect();
    let many = f.solve_many(&bs);
    for (b, x) in bs.iter().zip(&many) {
        assert_eq!(bits(&f.solve(b)), bits(x), "T10");
    }
    println!("T10: solve_many equals three solves bit for bit");
}
