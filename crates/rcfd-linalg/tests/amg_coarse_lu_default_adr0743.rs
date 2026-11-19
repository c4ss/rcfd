//! F-471 (ADR 0743): the corrected order of the row interchanges in the coarsest dense LU solve is the default of the scalar
//! AMG (`AmgSettings::default().coarse_lu_interchanges_first` is `true`); `coarse_lu_interchanges_first: false` asks for the
//! interleaved order of before and reproduces its bits.
//!
//! The matrices and the numbers are those of ADR 0369 (`amg_coarse_lu_f474.rs`, not edited): the 3 x 3 matrix `A3`, the
//! 20 x 20 SPD tridiagonal and the 40 x 40 Laplacian. A matrix of at most `max_coarse` unknowns is a single AMG level, so
//! `Preconditioner::apply` is then exactly the dense coarsest solve.

use rcfd_linalg::{Amg, AmgSettings, CsrMatrix, Preconditioner};

/// The settings of before ADR 0743: the interleaved order.
fn old_order() -> AmgSettings {
    AmgSettings {
        coarse_lu_interchanges_first: false,
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

/// ADR 0743 N1 (A2): the default is the corrected order, and the default solve on `A3` is exact.
#[test]
fn n1_default_solve_is_exact_on_a3() {
    assert!(AmgSettings::default().coarse_lu_interchanges_first, "N1: the default");
    let amg = Amg::new(&a3(), AmgSettings::default());
    assert_eq!(amg.n_levels(), 1);
    assert_eq!(amg.coarse_interchanges(), vec![1], "N1: interchanges");
    let mut z = vec![0.0; 3];
    amg.apply(&[2.0, 5.0, 7.0], &mut z);
    println!("N1: default z = {z:?}");
    assert_eq!(bits(&z), bits(&[1.0, 1.0, 1.0]), "N1: default z = {z:?}");
}

/// ADR 0743 N2: the old order on `A3` gives the bits the default gave before (ADR 0369 L1).
#[test]
fn n2_old_order_reproduces_the_old_bits_on_a3() {
    let amg = Amg::new(&a3(), old_order());
    assert_eq!(amg.n_levels(), 1);
    assert_eq!(amg.coarse_interchanges(), vec![1], "N2: interchanges");
    let mut z = vec![0.0; 3];
    amg.apply(&[2.0, 5.0, 7.0], &mut z);
    println!("N2: old-order z = {z:?}");
    assert_eq!(bits(&z), bits(&[-13.0, 15.0, -5.0]), "N2: old-order z = {z:?}");
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

/// ADR 0743 N3 (A2) and N3a: the default solves a matrix that needs interchanges at steps k >= 1 to a relative residual
/// of 1e-12; the old order does not.
#[test]
fn n3_default_solves_a_matrix_that_needs_interchanges() {
    let a = tri20();
    let x: Vec<f64> = (0..a.n).map(|i| 1.0 + i as f64 / 20.0).collect();
    let mut b = vec![0.0; a.n];
    a.matvec(&x, &mut b);
    let def = Amg::new(&a, AmgSettings::default());
    let old = Amg::new(&a, old_order());
    assert_eq!(def.n_levels(), 1);
    let ks = def.coarse_interchanges();
    println!("N3: interchanges at steps {ks:?}");
    assert!(
        ks.iter().any(|&k| k >= 1),
        "N3: no interchange at a step k >= 1: {ks:?}"
    );
    assert_eq!(ks, old.coarse_interchanges(), "N3: the factorisation");
    let (mut zd, mut zo) = (vec![0.0; a.n], vec![0.0; a.n]);
    def.apply(&b, &mut zd);
    old.apply(&b, &mut zo);
    let (rd, ro) = (rel_residual(&a, &b, &zd), rel_residual(&a, &b, &zo));
    println!("N3: relative residual default {rd:.3e}, old order {ro:.3e}");
    assert!(rd <= 1e-12, "N3: default relative residual {rd:e}");
    assert!(ro > 1e-12, "N3a: old-order relative residual {ro:e}");
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

/// ADR 0743 N3b: with interchanges only at step 0 the old order and the default are the same bits (ADR 0369 L4b, whose
/// two hierarchies are the same settings since ADR 0743).
#[test]
fn n3b_old_order_is_the_default_with_interchanges_only_at_step_0() {
    let a = laplace2d(40);
    let b = rhs(a.n);
    let def = Amg::new(&a, AmgSettings::default());
    let old = Amg::new(&a, old_order());
    assert!(def.n_levels() >= 3);
    let ks = def.coarse_interchanges();
    println!("N3b: interchanges at steps {ks:?}");
    assert!(ks.iter().all(|&k| k == 0), "N3b: {ks:?}");
    let (mut zd, mut zo) = (vec![0.0; a.n], vec![0.0; a.n]);
    def.apply(&b, &mut zd);
    old.apply(&b, &mut zo);
    let diff = zd.iter().zip(&zo).filter(|(p, q)| p.to_bits() != q.to_bits()).count();
    assert_eq!(diff, 0, "N3b: differing values");
}
