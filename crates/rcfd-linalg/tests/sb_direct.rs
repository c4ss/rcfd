//! F-373 registered checks (ADR 0274): envelope L D L^T after reverse Cuthill-McKee.

use rcfd_linalg::skyline::{envelope_size, PivotRule, SkylineError, SkylineLdlt, SkylineOptions, SkylineOrdering};
use rcfd_linalg::{cg, CsrMatrix, Identity, SolveControl};
use std::time::Instant;

const SEED_PERM: u64 = 0x9E3779B97F4A7C15;
const SEED_RHS: u64 = 0x2545F4914F6CDD1D;
const B3: [[f64; 3]; 3] = [[4.0, 1.0, 0.5], [1.0, 3.0, 0.25], [0.5, 0.25, 2.0]];

fn lcg(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s
}

/// Fisher-Yates shuffle of `0..n` with the LCG of `tests/reorder.rs`.
fn shuffled(n: usize, seed: u64) -> Vec<usize> {
    let mut p: Vec<usize> = (0..n).collect();
    let mut s = seed;
    for i in (1..n).rev() {
        let j = ((lcg(&mut s) >> 33) as usize) % (i + 1);
        p.swap(i, j);
    }
    p
}

/// Right-hand side `b_q`, entries in [-1, 1).
fn rhs(n: usize, q: u64) -> Vec<f64> {
    let mut s = SEED_RHS.wrapping_add(q);
    (0..n)
        .map(|_| ((lcg(&mut s) >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0)
        .collect()
}

/// 7-point grid graph of m^3 nodes: diagonal `diag(degree)`, -1 per neighbour.
fn grid7(m: usize, diag: impl Fn(usize) -> f64) -> CsrMatrix {
    let id = |i: usize, j: usize, k: usize| i + m * j + m * m * k;
    let mut t = Vec::new();
    for k in 0..m {
        for j in 0..m {
            for i in 0..m {
                let p = id(i, j, k);
                let mut deg = 0;
                let mut nb = |q: usize| {
                    t.push((p, q, -1.0));
                    deg += 1;
                };
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
                t.push((p, p, diag(deg)));
            }
        }
    }
    CsrMatrix::from_triplets(m * m * m, t)
}

/// Dirichlet 7-point Laplacian (diagonal 6).
fn lap7(m: usize) -> CsrMatrix {
    grid7(m, |_| 6.0)
}

/// 27-point node graph Laplacian of m^3 nodes plus the identity.
fn lap27_plus_identity(m: usize) -> CsrMatrix {
    let id = |i: usize, j: usize, k: usize| i + m * j + m * m * k;
    let mut t = Vec::new();
    for k in 0..m {
        for j in 0..m {
            for i in 0..m {
                let p = id(i, j, k);
                let mut deg = 0.0;
                for dk in -1i64..=1 {
                    for dj in -1i64..=1 {
                        for di in -1i64..=1 {
                            if (di, dj, dk) == (0, 0, 0) {
                                continue;
                            }
                            let (ii, jj, kk) = (i as i64 + di, j as i64 + dj, k as i64 + dk);
                            let r = 0..m as i64;
                            if r.contains(&ii) && r.contains(&jj) && r.contains(&kk) {
                                t.push((p, id(ii as usize, jj as usize, kk as usize), -1.0));
                                deg += 1.0;
                            }
                        }
                    }
                }
                t.push((p, p, deg + 1.0));
            }
        }
    }
    CsrMatrix::from_triplets(m * m * m, t)
}

/// `a (x) B3`, unknown `3 p + c`.
fn kron3(a: &CsrMatrix) -> CsrMatrix {
    let mut t = Vec::with_capacity(9 * a.nnz());
    for r in 0..a.n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            let (c, v) = (a.col[k], a.val[k]);
            for (bi, brow) in B3.iter().enumerate() {
                for (bj, &bv) in brow.iter().enumerate() {
                    t.push((3 * r + bi, 3 * c + bj, v * bv));
                }
            }
        }
    }
    CsrMatrix::from_triplets(3 * a.n, t)
}

fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn rel_res(a: &CsrMatrix, x: &[f64], b: &[f64]) -> f64 {
    let mut r = vec![0.0; a.n];
    a.residual(b, x, &mut r);
    norm2(&r) / norm2(b)
}

fn rel_diff(x: &[f64], y: &[f64]) -> f64 {
    let d: Vec<f64> = x.iter().zip(y).map(|(a, b)| a - b).collect();
    norm2(&d) / norm2(y)
}

fn cg_ref(a: &CsrMatrix, b: &[f64]) -> Vec<f64> {
    let mut x = vec![0.0; a.n];
    let st = cg(
        a,
        b,
        &mut x,
        &Identity,
        &SolveControl {
            tol_abs: 0.0,
            tol_rel: 1e-13,
            max_iter: 5000,
        },
    );
    println!("  cg: {} iterations, converged {}", st.iterations, st.converged);
    assert!(st.converged, "control: cg converged ({} iterations)", st.iterations);
    x
}

fn set_entry(a: &mut CsrMatrix, r: usize, c: usize, v: f64) {
    let k = a.row_ptr[r]
        + a.col[a.row_ptr[r]..a.row_ptr[r + 1]]
            .binary_search(&c)
            .expect("stored entry");
    a.val[k] = v;
}

/// C1 to C3 for one matrix: residual <= 1e-12, agreement with cg <= 1e-9. Returns the solution.
fn solve_and_check(name: &str, a: &CsrMatrix, b: &[f64]) -> Vec<f64> {
    let f = SkylineLdlt::factor(a).expect("factorises");
    let x = f.solve(b);
    let res = rel_res(a, &x, b);
    let xc = cg_ref(a, b);
    let agree = rel_diff(&x, &xc);
    println!(
        "{name}: n {} envelope {} residual {res:.3e} agreement with cg {agree:.3e}",
        a.n,
        f.envelope_size()
    );
    assert!(res <= 1e-12, "{name}: relative residual {res:e}");
    assert!(agree <= 1e-9, "{name}: agreement with cg {agree:e}");
    x
}

#[test]
fn c1_laplacian_12() {
    let a = lap7(12);
    solve_and_check("C1 A", &a, &rhs(a.n, 0));
}

#[test]
fn c2_permuted_copy() {
    let a = lap7(12);
    let perm = shuffled(a.n, SEED_PERM);
    let p = a.permute_symmetric(&perm);
    let ba = rhs(a.n, 0);
    let bp: Vec<f64> = perm.iter().map(|&o| ba[o]).collect();
    solve_and_check("C2 P", &p, &bp);
}

#[test]
fn c3_kronecker() {
    let k = kron3(&lap7(12));
    solve_and_check("C3 K", &k, &rhs(k.n, 0));
}

#[test]
fn c4_envelope_of_the_permuted_matrix() {
    let a = lap7(12);
    let e_nat = envelope_size(&a, None);
    println!("C4: E_nat {e_nat}");
    assert_eq!(e_nat, 229_691);
    let perm = shuffled(a.n, SEED_PERM);
    let p = a.permute_symmetric(&perm);
    println!("C4: permuted copy, input order envelope {}", envelope_size(&p, None));
    let fa = SkylineLdlt::factor(&a).expect("A");
    let fp = SkylineLdlt::factor(&p).expect("P");
    let (ea, ep) = (fa.envelope_size(), fp.envelope_size());
    println!(
        "C4/R2: |Env| A (RCM) {ea}, P (RCM) {ep}, P / E_nat {:.4}, P / A {:.4}",
        ep as f64 / e_nat as f64,
        ep as f64 / ea as f64
    );
    assert!(ep as f64 <= 1.2 * e_nat as f64, "P envelope {ep} > 1.2 x {e_nat}");
    assert!(ep <= 271_986, "P envelope {ep} > U");
    assert!(ea <= 271_986, "A envelope {ea} > U");
    assert_eq!(envelope_size(&p, Some(fp.order())), ep);
}

#[test]
fn c4k_envelope_of_the_kronecker_matrix() {
    let k = kron3(&lap7(12));
    let e_nat = envelope_size(&k, None);
    let f = SkylineLdlt::factor(&k).expect("K");
    println!("C4k: E_nat {e_nat}, RCM {}", f.envelope_size());
    assert_eq!(e_nat, 2_072_403);
    assert!(f.envelope_size() <= 2_458_294);
}

#[test]
fn c5_many_right_hand_sides() {
    let k = kron3(&lap7(12));
    let f = SkylineLdlt::factor(&k).expect("K");
    let before = f.clone();
    let x0 = f.solve(&rhs(k.n, 0));
    for q in 0..4 {
        let b = rhs(k.n, q);
        let x = f.solve(&b);
        let res = rel_res(&k, &x, &b);
        println!("C5: b_{q} residual {res:.3e}");
        assert!(res <= 1e-12, "b_{q}: {res:e}");
    }
    let mut x0b = vec![0.0; k.n];
    f.solve_into(&rhs(k.n, 0), &mut x0b);
    assert!(x0.iter().zip(&x0b).all(|(a, b)| a.to_bits() == b.to_bits()));
    assert_eq!(f, before);
}

fn path_laplacian(n: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        let deg = if i == 0 || i == n - 1 { 1.0 } else { 2.0 };
        t.push((i, i, deg));
        if i + 1 < n {
            t.push((i, i + 1, -1.0));
            t.push((i + 1, i, -1.0));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn natural() -> SkylineOptions {
    SkylineOptions {
        ordering: SkylineOrdering::Natural,
        ..Default::default()
    }
}

#[test]
fn c6_zero_pivot() {
    let a = path_laplacian(50);
    let e = SkylineLdlt::factor_with(&a, &natural()).unwrap_err();
    println!("C6 natural: {e}");
    match e {
        SkylineError::Pivot {
            row, position, pivot, ..
        } => {
            assert_eq!((row, position), (49, 49));
            assert_eq!(pivot, 0.0);
        }
        other => panic!("expected a pivot error, got {other:?}"),
    }
    let e = SkylineLdlt::factor(&a).unwrap_err();
    println!("C6 RCM: {e}");
    let mut edges = (Vec::new(), Vec::new());
    for i in 0..49 {
        edges.0.push(i);
        edges.1.push(i + 1);
    }
    let order = rcfd_linalg::rcm_order(50, &edges.0, &edges.1);
    match e {
        SkylineError::Pivot {
            row, position, pivot, ..
        } => {
            assert_eq!(position, 49);
            assert_eq!(row, order[49]);
            assert_eq!(pivot, 0.0);
        }
        other => panic!("expected a pivot error, got {other:?}"),
    }
}

#[test]
fn c7_negative_pivot() {
    let mut a = lap7(12);
    set_entry(&mut a, 777, 777, -1.0);
    let e = SkylineLdlt::factor(&a).unwrap_err();
    println!("C7 a_777,777 = -1: {e}");
    match e {
        SkylineError::Pivot { row, pivot, .. } => {
            assert_eq!(row, 777);
            assert!(pivot < 0.0);
        }
        other => panic!("expected a pivot error, got {other:?}"),
    }
    let d = CsrMatrix::from_triplets(4, vec![(0, 0, 1.0), (1, 1, 2.0), (2, 2, -3.0), (3, 3, 4.0)]);
    let e = SkylineLdlt::factor_with(&d, &natural()).unwrap_err();
    println!("C7 diag: {e}");
    match e {
        SkylineError::Pivot {
            row, position, pivot, ..
        } => {
            assert_eq!((row, position), (2, 2));
            assert_eq!(pivot, -3.0);
        }
        other => panic!("expected a pivot error, got {other:?}"),
    }
    let s = grid7(12, |_| 3.0);
    let e = SkylineLdlt::factor(&s).unwrap_err();
    println!("C7 A - 3I: {e}");
    // the order depends on the pattern only; take it from A
    let order = SkylineLdlt::factor(&lap7(12)).expect("A").order().to_vec();
    match e {
        SkylineError::Pivot {
            row,
            position,
            pivot,
            threshold,
        } => {
            assert_eq!(order[position], row);
            assert!(pivot <= threshold);
        }
        other => panic!("expected a pivot error, got {other:?}"),
    }
}

#[test]
fn c8_unsymmetric() {
    let mut a = lap7(12);
    set_entry(&mut a, 5, 6, -1.0 + 1e-9);
    let e = SkylineLdlt::factor(&a).unwrap_err();
    println!("C8: {e}");
    assert!(matches!(e, SkylineError::Unsymmetric { row: 5, col: 6, .. }), "{e:?}");

    let base = lap7(12);
    let mut t = Vec::new();
    for r in 0..base.n {
        for k in base.row_ptr[r]..base.row_ptr[r + 1] {
            t.push((r, base.col[k], base.val[k]));
        }
    }
    t.push((0, 100, 0.5));
    let b = CsrMatrix::from_triplets(base.n, t);
    let e = SkylineLdlt::factor(&b).unwrap_err();
    println!("C8 structural: {e}");
    assert!(matches!(e, SkylineError::Unsymmetric { row: 0, col: 100, .. }), "{e:?}");

    let mut c = lap7(12);
    set_entry(&mut c, 5, 6, -1.0 + 1e-15);
    assert!(SkylineLdlt::factor(&c).is_ok(), "negative control: 1e-15 is tolerated");
}

#[test]
fn c9_non_finite_and_shape() {
    let mut a = lap7(12);
    set_entry(&mut a, 3, 3, f64::NAN);
    let e = SkylineLdlt::factor(&a).unwrap_err();
    println!("C9: {e}");
    assert!(matches!(e, SkylineError::NonFinite { row: 3, col: 3, .. }), "{e:?}");
    let mut a = lap7(12);
    set_entry(&mut a, 10, 11, f64::INFINITY);
    let e = SkylineLdlt::factor(&a).unwrap_err();
    println!("C9: {e}");
    assert!(matches!(e, SkylineError::NonFinite { row: 10, col: 11, .. }), "{e:?}");
    let r = CsrMatrix::rect(2, 3, vec![0, 1, 2], vec![0, 1], vec![1.0, 1.0]);
    let e = SkylineLdlt::factor(&r).unwrap_err();
    println!("C9: {e}");
    assert!(matches!(e, SkylineError::NotSquare { rows: 2, cols: 3 }), "{e:?}");
    let a = lap7(3);
    let mut bad: Vec<usize> = (0..27).collect();
    bad[5] = 4;
    let e = SkylineLdlt::factor_with(
        &a,
        &SkylineOptions {
            ordering: SkylineOrdering::Given(bad),
            ..Default::default()
        },
    )
    .unwrap_err();
    println!("C9: {e}");
    assert!(matches!(e, SkylineError::BadOrdering { .. }), "{e:?}");
}

#[test]
fn c10_bit_identical_for_1_and_8_threads() {
    let k = kron3(&lap7(12));
    let b = rhs(k.n, 0);
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let f = SkylineLdlt::factor(&k).expect("K");
            let x = f.solve(&b);
            (f, x)
        })
    };
    let (f1, x1) = run(1);
    let (f8, x8) = run(8);
    assert_eq!(f1.order(), f8.order());
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<u64>>();
    assert_eq!(bits(f1.envelope_values()), bits(f8.envelope_values()));
    assert_eq!(bits(f1.pivots()), bits(f8.pivots()));
    assert_eq!(bits(&x1), bits(&x8));
    println!(
        "C10: factor and solution bit-identical for 1 and 8 threads ({} values)",
        f1.envelope_size()
    );
}

#[test]
fn c11_inertia() {
    let nz = SkylineOptions {
        pivots: PivotRule::NonZero,
        ..Default::default()
    };
    let d = CsrMatrix::from_triplets(4, vec![(0, 0, 1.0), (1, 1, 2.0), (2, 2, -3.0), (3, 3, 4.0)]);
    let f = SkylineLdlt::factor_with(&d, &nz).expect("diag");
    assert_eq!(f.inertia(), (1, 3));
    // eigenvalues of the 12^3 Dirichlet Laplacian, closed form
    let m = 12;
    let one: Vec<f64> = (1..=m)
        .map(|k| 2.0 - 2.0 * (k as f64 * std::f64::consts::PI / (m + 1) as f64).cos())
        .collect();
    let mut lam = Vec::new();
    for a in &one {
        for b in &one {
            for c in &one {
                lam.push(a + b + c);
            }
        }
    }
    lam.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut distinct: Vec<f64> = Vec::new();
    for &l in &lam {
        let new = match distinct.last() {
            None => true,
            Some(&p) => l - p > 1e-9,
        };
        if new {
            distinct.push(l);
        }
    }
    let sigma1 = (distinct[0] + distinct[1]) / 2.0;
    let sigma2 = (distinct[9] + distinct[10]) / 2.0;
    println!("C11: sigma1 {sigma1:.6}, sigma2 {sigma2:.6}");
    assert!((sigma1 - 0.259835).abs() < 1e-6 && (sigma2 - 1.193060).abs() < 1e-6);
    for (sigma, want) in [(sigma1, (1, 1727)), (sigma2, (32, 1696))] {
        let s = grid7(m, |_| 6.0 - sigma);
        let below = lam.iter().filter(|&&l| l < sigma).count();
        assert_eq!(below, want.0, "closed-form count");
        let f = SkylineLdlt::factor_with(&s, &nz).expect("shifted factorises");
        println!("C11: sigma {sigma:.6}: inertia {:?}", f.inertia());
        assert_eq!(f.inertia(), want);
    }
}

#[test]
fn c13_permuted_solution() {
    let a = lap7(12);
    let perm = shuffled(a.n, SEED_PERM);
    let p = a.permute_symmetric(&perm);
    let ba = rhs(a.n, 0);
    let bp: Vec<f64> = perm.iter().map(|&o| ba[o]).collect();
    let xa = SkylineLdlt::factor(&a).expect("A").solve(&ba);
    let xp = SkylineLdlt::factor(&p).expect("P").solve(&bp);
    let xa_p: Vec<f64> = perm.iter().map(|&o| xa[o]).collect();
    let d = rel_diff(&xp, &xa_p);
    println!("C13: {d:.3e}");
    assert!(d <= 1e-10);
}

#[test]
fn r1_singular_neumann_record() {
    let a = grid7(12, |deg| deg as f64);
    match SkylineLdlt::factor(&a) {
        Ok(f) => println!("R1: factorised; last pivot {:e}", f.pivots()[a.n - 1]),
        Err(e) => println!("R1: refused: {e}"),
    }
}

fn heavy(name: &str, a: &CsrMatrix, opts: &SkylineOptions, env_limit: Option<usize>, env_exact: Option<usize>) {
    let b = rhs(a.n, 0);
    let t0 = Instant::now();
    let f = SkylineLdlt::factor_with(a, opts).expect("factorises");
    let t_factor = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let x = f.solve(&b);
    let t_solve = t1.elapsed().as_secs_f64();
    let res = rel_res(a, &x, &b);
    let w_max = (0..a.n).map(|i| i - f.first_column(i)).max().unwrap_or(0);
    println!(
        "{name}: n {} nnz {} |Env| {} max width {w_max} storage {} bytes ({:.1} MB); order + factor {t_factor:.2} s; \
         solve {t_solve:.3} s; residual {res:.3e}",
        a.n,
        a.nnz(),
        f.envelope_size(),
        f.storage_bytes(),
        f.storage_bytes() as f64 / 1e6
    );
    assert!(res <= 1e-12, "{name}: residual {res:e}");
    if let Some(l) = env_limit {
        assert!(f.envelope_size() <= l, "{name}: envelope {} > {l}", f.envelope_size());
    }
    if let Some(e) = env_exact {
        assert_eq!(f.envelope_size(), e, "{name}: envelope");
    }
}

#[test]
#[ignore = "heavy record H1 (ADR 0274): 30^3 Laplacian"]
fn h1_laplacian_30_heavy() {
    let a = lap7(30);
    let t0 = Instant::now();
    heavy("H1", &a, &SkylineOptions::default(), Some(26_702_997), None);
    let total = t0.elapsed().as_secs_f64();
    println!("H1: total {total:.2} s");
    assert!(total <= 300.0);
}

#[test]
#[ignore = "heavy record H2 (ADR 0274): Hex8-box sparsity of 20^3 elements, 27 783 unknowns"]
fn h2_hex_box_sparsity_heavy() {
    let k = kron3(&lap27_plus_identity(21));
    for (name, ordering, limit, exact) in [
        ("H2 natural", SkylineOrdering::Natural, None, Some(36_780_723)),
        ("H2 rcm", SkylineOrdering::Rcm, Some(124_510_132), None),
    ] {
        let t0 = Instant::now();
        heavy(
            name,
            &k,
            &SkylineOptions {
                ordering,
                ..Default::default()
            },
            limit,
            exact,
        );
        let total = t0.elapsed().as_secs_f64();
        println!("{name}: total {total:.2} s");
        assert!(total <= 600.0);
    }
}
