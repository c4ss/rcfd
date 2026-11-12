//! F-457 registered checks (ADR 0354): the scale-aware (normwise backward error) convergence test of `lowest_eigenpairs`.
//! L1 (the reported residual against its definition), L2 (T: backward against the default), L3 (Q = T^2, a pencil whose
//! default floor is above 1e-10), X1 (defaults and the cap). The checks of ADR 0283 are `sb_eigen.rs`, unedited.

use rcfd_linalg::eigen::{lowest_eigenpairs, EigenError, GeneralizedEigen, ResidualTest, Shift, SubspaceOptions};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// The scale-aware tolerance of the acceptance.
const TAU: f64 = 1e-12;

fn tridiag(d: &[f64], e: f64) -> CsrMatrix {
    let n = d.len();
    let mut t = Vec::new();
    for i in 0..n {
        t.push((i, i, d[i]));
        if i + 1 < n {
            t.push((i, i + 1, e));
            t.push((i + 1, i, e));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn second_difference(n: usize) -> CsrMatrix {
    tridiag(&vec![2.0; n], -1.0)
}

/// `T^2` of the (2, -1) matrix: rows 0 and n-1 (5, -4, 1), rows 1 and n-2 (-4, 6, -4, 1), interior (1, -4, 6, -4, 1).
fn fourth_difference(n: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for i in 0..n {
        let d = if i == 0 || i == n - 1 { 5.0 } else { 6.0 };
        t.push((i, i, d));
        if i + 1 < n {
            t.push((i, i + 1, -4.0));
            t.push((i + 1, i, -4.0));
        }
        if i + 2 < n {
            t.push((i, i + 2, 1.0));
            t.push((i + 2, i, 1.0));
        }
    }
    CsrMatrix::from_triplets(n, t)
}

fn diag(v: &[f64]) -> CsrMatrix {
    CsrMatrix::from_triplets(v.len(), v.iter().enumerate().map(|(i, &x)| (i, i, x)).collect())
}

fn identity(n: usize) -> CsrMatrix {
    diag(&vec![1.0; n])
}

fn lap2d(m: usize) -> CsrMatrix {
    let mut t = Vec::new();
    for j in 0..m {
        for i in 0..m {
            let p = i + m * j;
            t.push((p, p, 4.0));
            if i > 0 {
                t.push((p, p - 1, -1.0));
            }
            if i + 1 < m {
                t.push((p, p + 1, -1.0));
            }
            if j > 0 {
                t.push((p, p - m, -1.0));
            }
            if j + 1 < m {
                t.push((p, p + m, -1.0));
            }
        }
    }
    CsrMatrix::from_triplets(m * m, t)
}

fn nonuniform_mass() -> Vec<f64> {
    let mut v = vec![0.0; 900];
    for j in 0..30 {
        for i in 0..30 {
            v[i + 30 * j] = 1.0 + 0.5 * i as f64 / 29.0 + 0.25 * (j as f64 / 29.0).powi(2);
        }
    }
    v
}

fn tri_closed(n: usize) -> Vec<f64> {
    (1..=n)
        .map(|j| 4.0 * (j as f64 * PI / (2.0 * (n as f64 + 1.0))).sin().powi(2))
        .collect()
}

fn mv(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

/// Plain sequential 2-norm (a different summation from the library's).
fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

/// `max_i sum_j |a_ij|`, computed here from the entries.
fn inf_norm(a: &CsrMatrix) -> f64 {
    let mut w: f64 = 0.0;
    for r in 0..a.n {
        let mut s = 0.0;
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            s += a.val[p].abs();
        }
        w = w.max(s);
    }
    w
}

/// `K + s M` by triplets (`s = 0`: `K`).
fn shifted(k: &CsrMatrix, m: &CsrMatrix, s: f64) -> CsrMatrix {
    if s == 0.0 {
        return k.clone();
    }
    let mut t = Vec::new();
    for r in 0..k.n {
        for p in k.row_ptr[r]..k.row_ptr[r + 1] {
            t.push((r, k.col[p], k.val[p]));
        }
        for p in m.row_ptr[r]..m.row_ptr[r + 1] {
            t.push((r, m.col[p], s * m.val[p]));
        }
    }
    CsrMatrix::from_triplets(k.n, t)
}

/// The backward errors of the returned pairs recomputed from their definition (ADR 0354 D1, D2).
fn backward_errors(k: &CsrMatrix, m: &CsrMatrix, r: &GeneralizedEigen) -> Vec<f64> {
    let a = shifted(k, m, r.shift);
    let (na, nm) = (inf_norm(&a), inf_norm(m));
    r.values
        .iter()
        .zip(&r.vectors)
        .map(|(&lambda, x)| {
            let mu = lambda + r.shift;
            let kx = mv(k, x);
            let mx = mv(m, x);
            let res: Vec<f64> = kx.iter().zip(&mx).map(|(p, v)| p - lambda * v).collect();
            norm2(&res) / ((na + mu.abs() * nm) * norm2(x))
        })
        .collect()
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| ((x - y) / y).abs()).fold(0.0, f64::max)
}

fn m_orth(m: &CsrMatrix, x: &[Vec<f64>]) -> f64 {
    let mut w: f64 = 0.0;
    for (i, xi) in x.iter().enumerate() {
        let mxi = mv(m, xi);
        for (j, xj) in x.iter().enumerate() {
            let d: f64 = xj.iter().zip(&mxi).map(|(p, q)| p * q).sum();
            w = w.max((d - if i == j { 1.0 } else { 0.0 }).abs());
        }
    }
    w
}

fn backward(tolerance: f64) -> SubspaceOptions {
    SubspaceOptions {
        tolerance,
        residual_test: ResidualTest::Backward,
        ..SubspaceOptions::default()
    }
}

/// L1: the reported residuals are the backward errors of the definition, on N (M != I) and T.
#[test]
fn l1_reported_residual_is_the_backward_error() {
    let mut worst: f64 = 0.0;
    for (name, k, m, count) in [
        ("N", lap2d(30), diag(&nonuniform_mass()), 11),
        ("T", second_difference(200), identity(200), 10),
    ] {
        let r = lowest_eigenpairs(&k, &m, count, &backward(TAU)).expect("backward run");
        let mine = backward_errors(&k, &m, &r);
        let rel = r
            .residuals
            .iter()
            .zip(&mine)
            .map(|(a, b)| ((a - b) / b).abs())
            .fold(0.0, f64::max);
        println!(
            "L1 {name}: shift {:e}, iterations {}, reported {:?}, recomputed {:?}, max relative difference {rel:.3e}",
            r.shift, r.iterations, r.residuals, mine
        );
        worst = worst.max(rel);
    }
    println!("L1: max relative difference {worst:.3e} (limit <= 1e-12)");
    assert!(worst <= 1e-12, "L1 reported against recomputed {worst:e}");
}

/// L2: T, the backward test at 1e-12 against the default run.
#[test]
fn l2_tridiagonal_backward_against_default() {
    let n = 200;
    let k = second_difference(n);
    let m = identity(n);
    let exact = tri_closed(n);
    let d = lowest_eigenpairs(&k, &m, 10, &SubspaceOptions::default()).expect("T default");
    let b = lowest_eigenpairs(&k, &m, 10, &backward(TAU)).expect("T backward");
    let ev_d = max_rel(&d.values, &exact[..10]);
    let ev_b = max_rel(&b.values, &exact[..10]);
    let orth_d = m_orth(&m, &d.vectors);
    let orth_b = m_orth(&m, &b.vectors);
    println!(
        "L2 default: eigenvalue error {ev_d:.3e}, M-orthonormality {orth_d:.3e}, iterations {}, shift {}, Sturm {:?}",
        d.iterations, d.shift, d.sturm
    );
    println!(
        "L2 backward: eigenvalue error {ev_b:.3e}, M-orthonormality {orth_b:.3e}, iterations {}, shift {}, Sturm {:?}, \
         residuals {:?}",
        b.iterations, b.shift, b.sturm, b.residuals
    );
    let mut missed = Vec::new();
    if ev_b > 1e-10 {
        missed.push(format!("backward eigenvalues {ev_b:e}"));
    }
    if orth_b > 1e-10 {
        missed.push(format!("backward M-orthonormality {orth_b:e}"));
    }
    if b.iterations > d.iterations {
        missed.push(format!("iterations {} > {}", b.iterations, d.iterations));
    }
    if ev_d > 1e-10
        || orth_d > 1e-10
        || d.iterations > 34
        || d.shift != 0.0
        || (d.sturm.counted, d.sturm.expected) != (10, 10)
    {
        missed.push("default run outside the limits of ADR 0283 E5".to_string());
    }
    assert!(missed.is_empty(), "L2 missed: {missed:?}");
}

/// L3: Q = T^2 (kappa 2.7e8), the backward test at 1e-12; the default run recorded.
#[test]
fn l3_fourth_difference_backward() {
    let n = 200;
    let k = fourth_difference(n);
    let m = identity(n);
    let exact: Vec<f64> = tri_closed(n).iter().map(|v| v * v).collect();
    let opts = SubspaceOptions {
        shift: Shift::Value(0.0),
        ..backward(TAU)
    };
    let default = SubspaceOptions {
        shift: Shift::Value(0.0),
        ..SubspaceOptions::default()
    };
    match lowest_eigenpairs(&k, &m, 10, &default) {
        Ok(r) => println!(
            "L3 record, default test: converged in {} iterations, eigenvalue error {:.3e}, residuals {:?}",
            r.iterations,
            max_rel(&r.values, &exact[..10]),
            r.residuals
        ),
        Err(e) => println!("L3 record, default test: {e}"),
    }
    let r = lowest_eigenpairs(&k, &m, 10, &opts).expect("L3 backward run");
    let ev = max_rel(&r.values, &exact[..10]);
    let orth = m_orth(&m, &r.vectors);
    println!(
        "L3 backward: iterations {}, eigenvalue error {ev:.3e}, M-orthonormality {orth:.3e}, Sturm {:?}, residuals {:?}",
        r.iterations, r.sturm, r.residuals
    );
    let mut missed = Vec::new();
    if r.iterations > 40 {
        missed.push(format!("iterations {}", r.iterations));
    }
    if ev > 1e-6 {
        missed.push(format!("eigenvalues {ev:e}"));
    }
    if orth > 1e-10 {
        missed.push(format!("M-orthonormality {orth:e}"));
    }
    assert!(missed.is_empty(), "L3 missed: {missed:?}");
}

/// X1: the defaults stay the relative test; the cap reports the backward residuals.
#[test]
fn x1_defaults_and_cap() {
    assert_eq!(SubspaceOptions::default().residual_test, ResidualTest::Relative);
    assert_eq!(ResidualTest::default(), ResidualTest::Relative);
    let opts = SubspaceOptions {
        max_iterations: 3,
        ..backward(TAU)
    };
    match lowest_eigenpairs(&second_difference(200), &identity(200), 10, &opts) {
        Err(e @ EigenError::NotConverged { .. }) => {
            println!("X1: {e}");
            if let EigenError::NotConverged {
                iterations,
                tolerance,
                residuals,
            } = e
            {
                assert_eq!(iterations, 3);
                assert_eq!(tolerance, TAU);
                assert_eq!(residuals.len(), 10);
                assert!(residuals.iter().any(|&r| r > TAU));
            }
        }
        other => panic!("X1 expected NotConverged, got {other:?}"),
    }
}
