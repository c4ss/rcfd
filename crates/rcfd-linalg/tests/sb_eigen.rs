//! F-382 registered checks (ADR 0283): dense Jacobi, subspace iteration on the factor of K + sigma M, Sturm count.

use rcfd_linalg::eigen::{
    count_below, generalized_eigen_dense, lowest_eigenpairs, symmetric_eigen, EigenError, GeneralizedEigen, Shift,
    SubspaceOptions,
};
use rcfd_linalg::skyline::SkylineOrdering;
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;
use std::time::Instant;

fn lcg(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *s
}

fn uniform(s: &mut u64) -> f64 {
    ((lcg(s) >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
}

/// Symmetric tridiagonal matrix with diagonal `d[i]` and off-diagonal `e`.
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

fn diag(v: &[f64]) -> CsrMatrix {
    CsrMatrix::from_triplets(v.len(), v.iter().enumerate().map(|(i, &x)| (i, i, x)).collect())
}

fn identity(n: usize) -> CsrMatrix {
    diag(&vec![1.0; n])
}

/// 5-point Laplacian of an m x m grid, unknown i + m j, diagonal 4, Dirichlet by omission.
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

fn lap2d_closed(m: usize) -> Vec<f64> {
    let s: Vec<f64> = (1..=m)
        .map(|j| 4.0 * (j as f64 * PI / (2.0 * (m as f64 + 1.0))).sin().powi(2))
        .collect();
    let mut v: Vec<f64> = s.iter().flat_map(|a| s.iter().map(move |b| a + b)).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

fn tri_closed(n: usize) -> Vec<f64> {
    (1..=n)
        .map(|j| 4.0 * (j as f64 * PI / (2.0 * (n as f64 + 1.0))).sin().powi(2))
        .collect()
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

/// Free chain: path Laplacian, diagonal 1, 2, ..., 2, 1.
fn free_chain(n: usize) -> CsrMatrix {
    let mut d = vec![2.0; n];
    d[0] = 1.0;
    d[n - 1] = 1.0;
    tridiag(&d, -1.0)
}

/// Consistent-mass bar: K = (1/h)(2, -1), M = (h/6)(4, 1), h = 1/(n+1).
fn bar(n: usize) -> (CsrMatrix, CsrMatrix, Vec<f64>) {
    let h = 1.0 / (n as f64 + 1.0);
    let k = tridiag(&vec![2.0 / h; n], -1.0 / h);
    let m = tridiag(&vec![4.0 * h / 6.0; n], h / 6.0);
    let lam = (1..=n)
        .map(|j| {
            let t = j as f64 * PI / (n as f64 + 1.0);
            (6.0 / (h * h)) * (1.0 - t.cos()) / (2.0 + t.cos())
        })
        .collect();
    (k, m, lam)
}

/// Decoupled pair: (2, -1) of 50 and 1.5 x (2, -1) of 50.
fn decoupled() -> CsrMatrix {
    let mut t = Vec::new();
    for b in 0..2 {
        let f = if b == 0 { 1.0 } else { 1.5 };
        for i in 0..50 {
            let p = 50 * b + i;
            t.push((p, p, 2.0 * f));
            if i + 1 < 50 {
                t.push((p, p + 1, -f));
                t.push((p + 1, p, -f));
            }
        }
    }
    CsrMatrix::from_triplets(100, t)
}

fn dense(a: &CsrMatrix) -> Vec<f64> {
    let n = a.n;
    let mut d = vec![0.0; n * n];
    for r in 0..n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            d[r * n + a.col[p]] = a.val[p];
        }
    }
    d
}

fn mv(a: &CsrMatrix, x: &[f64]) -> Vec<f64> {
    let mut y = vec![0.0; a.n];
    a.matvec(x, &mut y);
    y
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn nrm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// max_{ij} |x_i^T M x_j - delta_ij|
fn m_orth(m: &CsrMatrix, v: &[Vec<f64>]) -> f64 {
    let mv_: Vec<Vec<f64>> = v.iter().map(|x| mv(m, x)).collect();
    let mut w = 0.0f64;
    for i in 0..v.len() {
        for j in 0..v.len() {
            let e = dot(&v[i], &mv_[j]) - if i == j { 1.0 } else { 0.0 };
            w = w.max(e.abs());
        }
    }
    w
}

/// (||K x - lambda M x||, ||K x||, ||M x||)
fn residual_parts(k: &CsrMatrix, m: &CsrMatrix, lam: f64, x: &[f64]) -> (f64, f64, f64) {
    let kx = mv(k, x);
    let mx = mv(m, x);
    let r: Vec<f64> = kx.iter().zip(&mx).map(|(a, b)| a - lam * b).collect();
    (nrm(&r), nrm(&kx), nrm(&mx))
}

/// max over pairs of ||K x - lambda M x|| / ||K x||
fn max_k_residual(k: &CsrMatrix, m: &CsrMatrix, r: &GeneralizedEigen) -> f64 {
    r.values
        .iter()
        .zip(&r.vectors)
        .map(|(&l, x)| {
            let (a, b, _) = residual_parts(k, m, l, x);
            a / b
        })
        .fold(0.0, f64::max)
}

fn max_rel(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| ((x - y) / y).abs()).fold(0.0, f64::max)
}

fn dense_checks(name: &str, a: &[f64], n: usize, exact: &[f64], norm_a: f64) -> (f64, f64, f64, usize) {
    let e = symmetric_eigen(a, n).expect("dense");
    let err = e
        .values
        .iter()
        .zip(exact)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max);
    let mut orth = 0.0f64;
    for i in 0..n {
        for j in 0..n {
            let v = dot(&e.vectors[i], &e.vectors[j]) - if i == j { 1.0 } else { 0.0 };
            orth = orth.max(v.abs());
        }
    }
    let mut res = 0.0f64;
    for (l, v) in e.values.iter().zip(&e.vectors) {
        let av: Vec<f64> = (0..n).map(|r| dot(&a[r * n..r * n + n], v)).collect();
        let r: Vec<f64> = av.iter().zip(v).map(|(x, y)| x - l * y).collect();
        res = res.max(nrm(&r));
    }
    println!(
        "{name}: max eigenvalue error {err:.3e}, orthogonality {orth:.3e}, residual {res:.3e} (||A|| = {norm_a}), sweeps {}",
        e.sweeps
    );
    (err, orth, res, e.sweeps)
}

#[test]
fn e1_dense_jacobi_tridiagonal() {
    let n = 200;
    let a = dense(&second_difference(n));
    let (err, orth, res, _) = dense_checks("E1", &a, n, &tri_closed(n), 4.0);
    assert!(err <= 1e-12, "E1 eigenvalues {err:e}");
    assert!(orth <= 1e-12, "E1 orthogonality {orth:e}");
    assert!(res <= 4e-12, "E1 residual {res:e}");
}

#[test]
fn e2_dense_jacobi_laplacian_12() {
    let a = dense(&lap2d(12));
    let (err, orth, res, _) = dense_checks("E2", &a, 144, &lap2d_closed(12), 8.0);
    assert!(err <= 1e-12, "E2 eigenvalues {err:e}");
    assert!(orth <= 1e-12, "E2 orthogonality {orth:e}");
    assert!(res <= 8e-12, "E2 residual {res:e}");
}

#[test]
fn e3_dense_generalized_consistent_mass_bar() {
    let n = 200;
    let (k, m, lam) = bar(n);
    let e = generalized_eigen_dense(&dense(&k), &dense(&m), n).expect("dense pencil");
    let lmax = lam[n - 1];
    let err = e
        .values
        .iter()
        .zip(&lam)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
        / lmax;
    let orth = m_orth(&m, &e.vectors);
    println!(
        "E3: max |error| / lambda_max {err:.3e} (lambda_max {lmax:.6e}), M-orthonormality {orth:.3e}, sweeps {}",
        e.sweeps
    );
    assert!(err <= 1e-12, "E3 eigenvalues {err:e}");
    assert!(orth <= 1e-12, "E3 M-orthonormality {orth:e}");
}

#[test]
fn e4_dense_refusals() {
    let n = 6;
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        a[i * n + i] = 2.0;
    }
    let mut long = a.clone();
    long.push(0.0);
    assert!(matches!(
        symmetric_eigen(&long, n),
        Err(EigenError::DimensionMismatch { .. })
    ));
    let mut bad = a.clone();
    bad[3 * n + 5] = f64::NAN;
    assert_eq!(
        symmetric_eigen(&bad, n),
        Err(EigenError::NonFinite {
            matrix: "A",
            row: 3,
            col: 5
        })
    );
    let mut uns = a.clone();
    uns[2 * n + 5] = 1.0;
    uns[5 * n + 2] = 1.0 + 1e-9;
    assert_eq!(
        symmetric_eigen(&uns, n),
        Err(EigenError::Unsymmetric {
            matrix: "A",
            row: 2,
            col: 5
        })
    );
    let k = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let m = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, -1.0];
    match generalized_eigen_dense(&k, &m, 3) {
        Err(EigenError::MassNotPositiveDefinite { row, pivot }) => {
            assert_eq!(row, 2);
            println!("E4: M = diag(1, 1, -1) refused at row {row}, pivot {pivot}");
        }
        other => panic!("E4 expected MassNotPositiveDefinite, got {other:?}"),
    }
    let e = symmetric_eigen(&[0.0, 1.0, 1.0, 0.0], 2).expect("2 x 2");
    println!("E4 control: [[0, 1], [1, 0]] -> {:?}", e.values);
    assert!((e.values[0] + 1.0).abs() <= 1e-15 && (e.values[1] - 1.0).abs() <= 1e-15);
}

fn run_t(opts: &SubspaceOptions) -> Result<GeneralizedEigen, EigenError> {
    lowest_eigenpairs(&second_difference(200), &identity(200), 10, opts)
}

#[test]
fn e5_e6_tridiagonal() {
    let n = 200;
    let k = second_difference(n);
    let m = identity(n);
    let exact = tri_closed(n);
    let r = run_t(&SubspaceOptions::default()).expect("T");
    let ev = max_rel(&r.values, &exact[..10]);
    let res = max_k_residual(&k, &m, &r);
    let orth = m_orth(&m, &r.vectors);
    println!(
        "E5: T m = 10: eigenvalue error {ev:.3e}, residual {res:.3e}, M-orthonormality {orth:.3e}, sturm {:?}, shift {}, \
         iterations {}, q {}",
        r.sturm, r.shift, r.iterations, r.subspace_size
    );
    assert!(ev <= 1e-10, "E5 eigenvalues {ev:e}");
    assert!(res <= 1e-8, "E5 residual {res:e}");
    assert!(orth <= 1e-10, "E5 M-orthonormality {orth:e}");
    assert_eq!((r.sturm.counted, r.sturm.expected), (10, 10), "E5 Sturm");
    assert_eq!(r.shift, 0.0, "E5 shift");
    assert!(r.iterations <= 34, "E5 iterations {}", r.iterations);

    let ord = SkylineOrdering::Rcm;
    let s1 = 0.5 * (r.values[8] + r.values[9]);
    let s2 = r.values[9] * (1.0 - 1e-6);
    let c1 = count_below(&k, &m, s1, &ord).expect("count 1");
    let c2 = count_below(&k, &m, s2, &ord).expect("count 2");
    println!("E6: count below {s1:.9e} = {c1}, below {s2:.9e} = {c2}");
    assert_eq!(c1, 9, "E6 midpoint");
    assert_eq!(c2, 9, "E6 just below");
    for j in 1..=10 {
        let s = 0.5 * (exact[j - 1] + exact[j]);
        let c = count_below(&k, &m, s, &ord).expect("count j");
        assert_eq!(c, j, "E6 closed-form midpoint {j}");
    }
    println!("E6: closed-form midpoints j = 1..10 give j");
}

#[test]
fn e7_laplacian_multiplicity() {
    let k = lap2d(30);
    let m = identity(900);
    let exact = lap2d_closed(30);
    let r = lowest_eigenpairs(&k, &m, 11, &SubspaceOptions::default()).expect("L");
    let ev = max_rel(&r.values, &exact[..11]);
    let mut split = 0.0f64;
    for (a, b) in [(1, 2), (4, 5), (6, 7), (8, 9)] {
        split = split.max(((r.values[a] - r.values[b]) / r.values[a]).abs());
    }
    let res = max_k_residual(&k, &m, &r);
    let orth = m_orth(&m, &r.vectors);
    let s = 0.5 * (r.values[9] + r.values[10]);
    let c = count_below(&k, &m, s, &SkylineOrdering::Rcm).expect("count");
    println!(
        "E7: L m = 11: eigenvalue error {ev:.3e}, pair split {split:.3e}, residual {res:.3e}, M-orthonormality {orth:.3e}, \
         sturm {:?}, count below {s:.9e} = {c}, iterations {}, q {}",
        r.sturm, r.iterations, r.subspace_size
    );
    assert!(ev <= 1e-10, "E7 eigenvalues {ev:e}");
    assert!(split <= 1e-10, "E7 split {split:e}");
    assert!(res <= 1e-8, "E7 residual {res:e}");
    assert!(orth <= 1e-10, "E7 M-orthonormality {orth:e}");
    assert_eq!((r.sturm.counted, r.sturm.expected), (11, 11), "E7 Sturm");
    assert_eq!(c, 10, "E7 count");
    assert!(r.iterations <= 68, "E7 iterations {}", r.iterations);
}

#[test]
fn e8_cluster_crossing_m() {
    let k = lap2d(30);
    let m = identity(900);
    let exact = lap2d_closed(30);
    let r = lowest_eigenpairs(&k, &m, 9, &SubspaceOptions::default()).expect("L m = 9");
    let ev = max_rel(&r.values, &exact[..9]);
    let res = max_k_residual(&k, &m, &r);
    let orth = m_orth(&m, &r.vectors);
    println!(
        "E8: L m = 9: eigenvalue error {ev:.3e}, residual {res:.3e}, M-orthonormality {orth:.3e}, sturm {:?}, iterations {}",
        r.sturm, r.iterations
    );
    assert_eq!(r.values.len(), 9);
    assert!(ev <= 1e-10, "E8 eigenvalues {ev:e}");
    assert_eq!((r.sturm.counted, r.sturm.expected), (10, 10), "E8 Sturm");
    assert!(res <= 1e-8, "E8 residual {res:e}");
    assert!(orth <= 1e-10, "E8 M-orthonormality {orth:e}");
}

fn run_n() -> (CsrMatrix, CsrMatrix, GeneralizedEigen) {
    let k = lap2d(30);
    let m = diag(&nonuniform_mass());
    let r = lowest_eigenpairs(&k, &m, 11, &SubspaceOptions::default()).expect("N");
    (k, m, r)
}

#[test]
fn e9_nonuniform_mass_against_dense() {
    let (k, m, r) = run_n();
    let t0 = Instant::now();
    let d = generalized_eigen_dense(&dense(&k), &dense(&m), 900).expect("dense N");
    let dt = t0.elapsed().as_secs_f64();
    let dev = max_rel(&r.values, &d.values[..11]);
    let res = max_k_residual(&k, &m, &r);
    let orth = m_orth(&m, &r.vectors);
    let mut dres = 0.0f64;
    for i in 0..11 {
        let (a, b, _) = residual_parts(&k, &m, d.values[i], &d.vectors[i]);
        dres = dres.max(a / b);
    }
    println!(
        "E9: N m = 11: against dense {dev:.3e}, residual {res:.3e}, M-orthonormality {orth:.3e}, sturm {:?}, iterations {}; \
         dense residual control {dres:.3e}, dense time {dt:.2} s, sweeps {}; values {:?}",
        r.sturm, r.iterations, d.sweeps, r.values
    );
    assert!(dev <= 1e-9, "E9 against dense {dev:e}");
    assert!(res <= 1e-8, "E9 residual {res:e}");
    assert!(orth <= 1e-10, "E9 M-orthonormality {orth:e}");
    assert!(dres <= 1e-10, "E9 dense control {dres:e}");
}

fn check_free_chain(name: &str, r: &GeneralizedEigen, k: &CsrMatrix, m: &CsrMatrix) {
    let n = 100;
    let exact: Vec<f64> = (0..5)
        .map(|j| 4.0 * (j as f64 * PI / (2.0 * n as f64)).sin().powi(2))
        .collect();
    let zero = (r.values[0] / r.values[1]).abs();
    let ev = max_rel(&r.values[1..5], &exact[1..5]);
    let mut res = 0.0f64;
    for i in 1..5 {
        let (a, b, _) = residual_parts(k, m, r.values[i], &r.vectors[i]);
        res = res.max(a / b);
    }
    let (a0, _, m0) = residual_parts(k, m, r.values[0], &r.vectors[0]);
    let kx0 = mv(k, &r.vectors[0]);
    let ax0: Vec<f64> = kx0
        .iter()
        .zip(mv(m, &r.vectors[0]))
        .map(|(a, b)| a + r.shift * b)
        .collect();
    let res0 = a0 / nrm(&ax0);
    let orth = m_orth(m, &r.vectors);
    let s = 0.5 * (r.values[0] + r.values[1]);
    let c = count_below(k, m, s, &SkylineOrdering::Rcm).expect("count");
    println!(
        "{name}: F shift {:e}: |lambda_0| / lambda_1 {zero:.3e} (lambda_0 {:e}), lambda_1..4 error {ev:.3e}, residual {res:.3e}, \
         zero-mode residual {res0:.3e} (||M x|| {m0:.3}), M-orthonormality {orth:.3e}, count below {s:.6e} = {c}, sturm {:?}, \
         iterations {}",
        r.shift, r.values[0], r.sturm, r.iterations
    );
    assert!(zero <= 1e-9, "{name} zero eigenvalue {zero:e}");
    assert!(ev <= 1e-10, "{name} eigenvalues {ev:e}");
    assert!(res <= 1e-8, "{name} residual {res:e}");
    assert!(res0 <= 1e-8, "{name} zero-mode residual {res0:e}");
    assert!(orth <= 1e-10, "{name} M-orthonormality {orth:e}");
    assert_eq!(c, 1, "{name} count");
}

#[test]
fn e10_free_chain_with_shift() {
    let k = free_chain(100);
    let m = identity(100);
    let opts = SubspaceOptions {
        shift: Shift::Value(0.01),
        ..SubspaceOptions::default()
    };
    let r = lowest_eigenpairs(&k, &m, 5, &opts).expect("F");
    check_free_chain("E10", &r, &k, &m);
    let zero = SubspaceOptions {
        shift: Shift::Value(0.0),
        ..SubspaceOptions::default()
    };
    match lowest_eigenpairs(&k, &m, 5, &zero) {
        Err(EigenError::ShiftNotPositiveDefinite { shift, source }) => {
            println!("E10 control: sigma = {shift} refused: {source}")
        }
        other => panic!("E10 control expected ShiftNotPositiveDefinite, got {other:?}"),
    }
}

#[test]
fn e11_free_chain_auto_shift() {
    let k = free_chain(100);
    let m = identity(100);
    let r = lowest_eigenpairs(&k, &m, 5, &SubspaceOptions::default()).expect("F auto");
    assert_eq!(r.shift, 1e-4 * (198.0 / 100.0), "E11 shift {}", r.shift);
    check_free_chain("E11", &r, &k, &m);
}

#[test]
fn e12_cap_is_an_error_with_residuals() {
    let opts = SubspaceOptions {
        max_iterations: 3,
        ..SubspaceOptions::default()
    };
    match run_t(&opts) {
        Err(e @ EigenError::NotConverged { .. }) => {
            let text = e.to_string();
            println!("E12: {text}");
            if let EigenError::NotConverged {
                iterations,
                tolerance,
                residuals,
            } = &e
            {
                assert_eq!(*iterations, 3);
                assert_eq!(*tolerance, 1e-10);
                assert_eq!(residuals.len(), 10);
                assert!(residuals.iter().fold(0.0f64, |a, &b| a.max(b)) > 1e-10);
                assert!(text.contains("residuals"));
                for r in residuals {
                    assert!(text.contains(&format!("{r:e}")), "E12 residual {r:e} not stated");
                }
            }
        }
        other => panic!("E12 expected NotConverged, got {other:?}"),
    }
}

#[test]
fn e13_sturm_detects_a_missed_mode() {
    let k = decoupled();
    let m = identity(100);
    let mut s = 0x2545F4914F6CDD1D_u64;
    let start: Vec<Vec<f64>> = (0..13)
        .map(|_| (0..100).map(|i| if i < 50 { uniform(&mut s) } else { 0.0 }).collect())
        .collect();
    let opts = SubspaceOptions {
        start: Some(start),
        ..SubspaceOptions::default()
    };
    match lowest_eigenpairs(&k, &m, 5, &opts) {
        Err(EigenError::SturmMismatch {
            shift,
            counted,
            expected,
        }) => {
            println!("E13: deficient start: SturmMismatch at {shift:.6e}: counted {counted}, expected {expected}");
            assert_eq!((counted, expected), (9, 5), "E13 counts");
        }
        other => panic!("E13 expected SturmMismatch, got {other:?}"),
    }
    let r = lowest_eigenpairs(&k, &m, 5, &SubspaceOptions::default()).expect("S default");
    let mut union: Vec<f64> = tri_closed(50).iter().flat_map(|&v| [v, 1.5 * v]).collect();
    union.sort_by(|a, b| a.total_cmp(b));
    let ev = max_rel(&r.values, &union[..5]);
    println!("E13: default start: eigenvalue error {ev:.3e}, sturm {:?}", r.sturm);
    assert!(ev <= 1e-10, "E13 eigenvalues {ev:e}");
    assert_eq!((r.sturm.counted, r.sturm.expected), (5, 5), "E13 Sturm");
}

#[test]
fn e14_consistent_mass_bar() {
    let (k, m, lam) = bar(200);
    let r = lowest_eigenpairs(&k, &m, 10, &SubspaceOptions::default()).expect("B");
    let ev = max_rel(&r.values, &lam[..10]);
    let res = max_k_residual(&k, &m, &r);
    let orth = m_orth(&m, &r.vectors);
    println!(
        "E14: B m = 10: eigenvalue error {ev:.3e}, residual {res:.3e}, M-orthonormality {orth:.3e}, sturm {:?}, iterations {}",
        r.sturm, r.iterations
    );
    assert!(ev <= 1e-10, "E14 eigenvalues {ev:e}");
    assert!(res <= 1e-8, "E14 residual {res:e}");
    assert!(orth <= 1e-10, "E14 M-orthonormality {orth:e}");
}

#[test]
fn e15_bit_identical_for_1_and_8_threads() {
    let run = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| run_n().2)
    };
    let a = run(1);
    let b = run(8);
    let bits = |r: &GeneralizedEigen| -> Vec<u64> {
        let mut v: Vec<u64> = r.values.iter().map(|x| x.to_bits()).collect();
        v.extend(r.vectors.iter().flatten().map(|x| x.to_bits()));
        v.extend(r.residuals.iter().map(|x| x.to_bits()));
        v.push(r.iterations as u64);
        v
    };
    println!(
        "E15: {} words compared, iterations {} and {}",
        bits(&a).len(),
        a.iterations,
        b.iterations
    );
    assert_eq!(bits(&a), bits(&b), "E15");
}

#[test]
fn e17_sparse_refusals() {
    let k = second_difference(10);
    let m = identity(10);
    let o = SubspaceOptions::default();
    assert!(matches!(
        lowest_eigenpairs(&k, &m, 0, &o),
        Err(EigenError::BadCount { .. })
    ));
    assert!(matches!(
        lowest_eigenpairs(&k, &m, 10, &o),
        Err(EigenError::BadCount { .. })
    ));
    assert!(matches!(
        lowest_eigenpairs(&k, &identity(11), 3, &o),
        Err(EigenError::DimensionMismatch { .. })
    ));
    let mut d = vec![1.0; 10];
    d[4] = f64::NAN;
    assert_eq!(
        lowest_eigenpairs(&k, &diag(&d), 3, &o),
        Err(EigenError::NonFinite {
            matrix: "M",
            row: 4,
            col: 4
        })
    );
    let mut ku = k.clone();
    // k_21 = -1 + 1e-9 (row 2, column 1)
    let p = (ku.row_ptr[2]..ku.row_ptr[3]).find(|&p| ku.col[p] == 1).unwrap();
    ku.val[p] = -1.0 + 1e-9;
    assert_eq!(
        lowest_eigenpairs(&ku, &m, 3, &o),
        Err(EigenError::Unsymmetric {
            matrix: "K",
            row: 1,
            col: 2
        })
    );
    println!("E17: BadCount (0, n), DimensionMismatch, NonFinite M (4, 4), Unsymmetric K (1, 2): as registered");
}
