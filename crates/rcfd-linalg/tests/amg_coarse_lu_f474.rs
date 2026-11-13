//! F-474 / F-471 (ADR 0369): the order of the row interchanges in the coarsest dense LU solve of the scalar AMG.
//!
//! `DenseLu::new` swaps whole rows (the stored factors are `P A = L U`), while the default `DenseLu::solve` applies each
//! interchange and then eliminates its column, interleaved; that is exact only when no row is interchanged. The opt-in
//! `AmgSettings::coarse_lu_interchanges_first` applies every interchange first. A matrix of at most `max_coarse` unknowns is a
//! single AMG level, so `Preconditioner::apply` is then exactly the dense coarsest solve.

use rcfd_linalg::{Amg, AmgSettings, CsrMatrix, Preconditioner};

fn opt_in() -> AmgSettings {
    AmgSettings {
        coarse_lu_interchanges_first: true,
        ..AmgSettings::default()
    }
}

fn a3() -> CsrMatrix {
    CsrMatrix::from_triplets(
        3,
        vec![
            (0, 0, 1.0),
            (0, 1, 1.0),
            (1, 0, 1.0),
            (1, 1, 2.0),
            (1, 2, 2.0),
            (2, 1, 2.0),
            (2, 2, 5.0),
        ],
    )
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn rel_residual(a: &CsrMatrix, b: &[f64], x: &[f64]) -> f64 {
    let mut r = vec![0.0; a.n];
    a.residual(b, x, &mut r);
    norm(&r) / norm(b)
}

/// ADR 0369 L1: the default solve on `A3` gives the interleaved result of the registration's arithmetic, bit for bit.
#[test]
fn l1_default_interleaved_solve_is_wrong_on_a3() {
    let a = a3();
    let amg = Amg::new(&a, AmgSettings::default());
    assert_eq!(amg.n_levels(), 1);
    assert_eq!(amg.coarse_interchanges(), vec![1], "L1: interchanges");
    let mut z = vec![0.0; 3];
    amg.apply(&[2.0, 5.0, 7.0], &mut z);
    println!("L1: default z = {z:?}");
    assert_eq!(bits(&z), bits(&[-13.0, 15.0, -5.0]), "L1: default z = {z:?}");
}

/// ADR 0369 L2: the opt-in solve on `A3` is exact.
#[test]
fn l2_opt_in_solve_is_exact_on_a3() {
    let amg = Amg::new(&a3(), opt_in());
    let mut z = vec![0.0; 3];
    amg.apply(&[2.0, 5.0, 7.0], &mut z);
    println!("L2: opt-in z = {z:?}");
    assert_eq!(bits(&z), bits(&[1.0, 1.0, 1.0]), "L2: opt-in z = {z:?}");
}

/// The 20 x 20 SPD tridiagonal of ADR 0369 (diagonal alternating 1e-4 and 1, off-diagonals -0.4 sqrt(d_i d_j)).
fn tri20() -> CsrMatrix {
    let n = 20;
    let d = |i: usize| -> f64 {
        if i % 2 == 0 {
            1e-4
        } else {
            1.0
        }
    };
    let mut t = Vec::new();
    for i in 0..n {
        t.push((i, i, d(i)));
        if i + 1 < n {
            let o = -0.4 * (d(i) * d(i + 1)).sqrt();
            t.push((i, i + 1, o));
            t.push((i + 1, i, o));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

/// ADR 0369 L3: a matrix that needs interchanges at steps k >= 1 is solved by the opt-in to a relative residual of 1e-12.
#[test]
fn l3_opt_in_solves_a_matrix_that_needs_interchanges() {
    let a = tri20();
    let x: Vec<f64> = (0..a.n).map(|i| 1.0 + i as f64 / 20.0).collect();
    let mut b = vec![0.0; a.n];
    a.matvec(&x, &mut b);
    let def = Amg::new(&a, AmgSettings::default());
    let opt = Amg::new(&a, opt_in());
    assert_eq!(def.n_levels(), 1);
    let ks = opt.coarse_interchanges();
    println!("L3: interchanges at steps {ks:?}");
    assert!(
        ks.iter().any(|&k| k >= 1),
        "L3: no interchange at a step k >= 1: {ks:?}"
    );
    let (mut zd, mut zo) = (vec![0.0; a.n], vec![0.0; a.n]);
    def.apply(&b, &mut zd);
    opt.apply(&b, &mut zo);
    let (rd, ro) = (rel_residual(&a, &b, &zd), rel_residual(&a, &b, &zo));
    println!("L3: relative residual default {rd:.3e}, opt-in {ro:.3e}");
    assert!(ro <= 1e-12, "L3: opt-in relative residual {ro:e}");
}

/// The 5-point Dirichlet Laplacian on an n x n grid.
fn laplace2d(n: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let id = j * n + i;
            t.push((id, id, 4.0));
            if i > 0 {
                t.push((id, id - 1, -1.0));
            }
            if i + 1 < n {
                t.push((id, id + 1, -1.0));
            }
            if j > 0 {
                t.push((id, id - n, -1.0));
            }
            if j + 1 < n {
                t.push((id, id + n, -1.0));
            }
        }
    }
    CsrMatrix::from_triplets(n * n, t)
}

fn rhs(n: usize) -> Vec<f64> {
    (0..n).map(|i| 1.0 + ((i * 31) % 17) as f64 / 17.0).collect()
}

/// ADR 0369 L4: without interchanges the opt-in is the default bit for bit. KNOWN MISS (amendment 1): the coarsest
/// factorisation of this hierarchy interchanges at step 0 (measured `coarse_interchanges() == [0]`); the limit is kept.
#[test]
#[ignore = "known miss (ADR 0369 amendment 1): the 40 x 40 Laplacian's coarsest LU interchanges at step 0, measured [0]"]
fn l4_opt_in_is_bit_identical_without_interchanges_known_miss() {
    let a = laplace2d(40);
    let b = rhs(a.n);
    let def = Amg::new(&a, AmgSettings::default());
    let opt = Amg::new(&a, opt_in());
    assert!(def.n_levels() >= 3);
    assert!(
        def.coarse_interchanges().is_empty(),
        "L4: {:?}",
        def.coarse_interchanges()
    );
    let (mut zd, mut zo) = (vec![0.0; a.n], vec![0.0; a.n]);
    def.apply(&b, &mut zd);
    opt.apply(&b, &mut zo);
    let diff = zd.iter().zip(&zo).filter(|(p, q)| p.to_bits() != q.to_bits()).count();
    assert_eq!(diff, 0, "L4: differing values");
}

/// ADR 0369 amendment 1 L4b: with interchanges only at step 0 the opt-in is the default bit for bit.
#[test]
fn l4b_opt_in_is_bit_identical_with_interchanges_only_at_step_0() {
    let a = laplace2d(40);
    let b = rhs(a.n);
    let def = Amg::new(&a, AmgSettings::default());
    let opt = Amg::new(&a, opt_in());
    let ks = def.coarse_interchanges();
    println!("L4b: interchanges at steps {ks:?}");
    assert!(ks.iter().all(|&k| k == 0), "L4b: {ks:?}");
    let (mut zd, mut zo) = (vec![0.0; a.n], vec![0.0; a.n]);
    def.apply(&b, &mut zd);
    opt.apply(&b, &mut zo);
    let diff = zd.iter().zip(&zo).filter(|(p, q)| p.to_bits() != q.to_bits()).count();
    assert_eq!(diff, 0, "L4b: differing values");
}

/// ADR 0369 L5: the read-only accessors.
#[test]
fn l5_accessors() {
    let a = laplace2d(40);
    let b = rhs(a.n);
    let amg = Amg::new(&a, AmgSettings::default());
    let (mut z1, mut z2) = (vec![0.0; a.n], vec![0.0; a.n]);
    amg.apply(&b, &mut z1);
    amg.cycle_at(0, &b, &mut z2);
    assert_eq!(bits(&z1), bits(&z2), "L5: cycle_at(0) differs from apply");
    let sizes = amg.level_sizes();
    let nl = amg.n_levels();
    for (l, &s) in sizes.iter().enumerate() {
        assert_eq!(amg.level_operator(l).n, s, "L5: level {l} size");
        assert_eq!(
            amg.level_prolongator(l).is_some(),
            l + 1 < nl,
            "L5: level {l} prolongator"
        );
    }
    let last = nl - 1;
    let bc = rhs(sizes[last]);
    let (mut c1, mut c2) = (vec![0.0; bc.len()], vec![0.0; bc.len()]);
    amg.coarse_solve(&bc, &mut c1);
    amg.cycle_at(last, &bc, &mut c2);
    assert_eq!(bits(&c1), bits(&c2), "L5: coarse_solve differs from the coarsest cycle");
}

/// Double-double dot product (TwoProd by a fused multiply-add, TwoSum), ADR 0369 D3.
fn dd_dot(a: &[f64], b: &[f64]) -> f64 {
    let (mut hi, mut lo) = (0.0f64, 0.0f64);
    for (x, y) in a.iter().zip(b) {
        let p = x * y;
        let pe = x.mul_add(*y, -p);
        let s = hi + p;
        let z = s - hi;
        let e = (hi - (s - z)) + (p - z);
        hi = s;
        lo += e + pe;
    }
    hi + lo
}

/// ADR 0369 L6: the double-double dot is exact on a cancellation that plain f64 loses.
#[test]
fn l6_double_double_dot() {
    let (a, b) = ([1e16, 1.0, -1e16], [1.0, 1.0, 1.0]);
    let plain: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
    assert_eq!(plain, 0.0);
    assert_eq!(dd_dot(&a, &b), 1.0, "L6");
}
