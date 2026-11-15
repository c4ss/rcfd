//! F-653 (ADR 0580): the pencil eigensolver on its own. K1: a pencil of known complex spectrum; K2: the 1-D Laplacian
//! against its closed form; K3: the refusals. Each registered run is made once.

use rcfd_linalg::complex::C64;
use rcfd_linalg::eigen_pencil::{nearest_pencil_eigenpairs, PencilError, PencilOptions};
use rcfd_linalg::CsrMatrix;

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

#[test]
fn k1_known_complex_spectrum() {
    let n = 120;
    let mut s = 0x0580_0001u64;
    let bdiag: Vec<f64> = (0..n).map(|_| 1.0 + splitmix(&mut s)).collect();
    // Q = I + L, L strictly lower; Q^-1 by forward substitution of the identity.
    let mut q = vec![0.0; n * n];
    for i in 0..n {
        q[i * n + i] = 1.0;
        for j in 0..i {
            q[i * n + j] = 0.2 * splitmix(&mut s) - 0.1;
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
    let mut exact = Vec::new();
    for blk in 0..20 {
        let a = 4.0 * splitmix(&mut s) - 2.0;
        let c = 0.1 + 0.9 * splitmix(&mut s);
        let i = 2 * blk;
        d[i * n + i] = a;
        d[(i + 1) * n + i + 1] = a;
        d[i * n + i + 1] = c;
        d[(i + 1) * n + i] = -c;
        exact.push(C64::new(a, c));
        exact.push(C64::new(a, -c));
    }
    for i in 40..n {
        let v = 4.0 * splitmix(&mut s) - 2.0;
        d[i * n + i] = v;
        exact.push(C64::from_real(v));
    }
    let mut a = matmul(&matmul(&q, &d, n), &qi, n);
    for i in 0..n {
        for j in 0..n {
            a[i * n + j] *= bdiag[i];
        }
    }
    let am = dense_to_csr(&a, n);
    let bm = CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, bdiag[i])).collect());
    let sigma = 0.3;
    let r = nearest_pencil_eigenpairs(&am, &bm, 8, &PencilOptions::new().shift(sigma)).unwrap();
    let mut near = exact.clone();
    near.sort_by(|x, y| {
        (*x - C64::from_real(sigma))
            .abs()
            .partial_cmp(&(*y - C64::from_real(sigma)).abs())
            .unwrap()
    });
    let mut err: f64 = 0.0;
    for e in near.iter().take(8) {
        let m = r.values.iter().map(|v| (*v - *e).abs()).fold(f64::INFINITY, f64::min);
        err = err.max(m / e.abs());
    }
    for v in &r.values {
        let m = exact.iter().map(|e| (*v - *e).abs() / e.abs()).fold(f64::INFINITY, f64::min);
        err = err.max(m);
    }
    let res = r.residuals.iter().fold(0.0f64, |x, &y| x.max(y));
    println!(
        "K1: {} values, {} steps, max relative eigenvalue error {err:.3e} (limit 1e-10), max residual {res:.3e} (limit 1e-10)",
        r.values.len(),
        r.steps
    );
    println!("K1 values {:?}", r.values);
    assert!(r.values.len() >= 8);
    assert!(err <= 1e-10, "K1 eigenvalue error {err:e}");
    assert!(res <= 1e-10, "K1 residual {res:e}");
}

#[test]
fn k2_laplacian_closed_form() {
    let n = 200;
    let mut t = Vec::new();
    for i in 0..n {
        t.push((i, i, 2.0));
        if i > 0 {
            t.push((i, i - 1, -1.0));
            t.push((i - 1, i, -1.0));
        }
    }
    let a = CsrMatrix::from_triplets(n, t);
    let b = CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect());
    let r = nearest_pencil_eigenpairs(&a, &b, 5, &PencilOptions::new()).unwrap();
    let mut err: f64 = 0.0;
    let mut im: f64 = 0.0;
    for (k, v) in r.values.iter().take(5).enumerate() {
        let e = 2.0 - 2.0 * ((k + 1) as f64 * std::f64::consts::PI / (n + 1) as f64).cos();
        err = err.max((v.re - e).abs() / e);
        im = im.max(v.im.abs() / v.abs());
    }
    println!(
        "K2: {} steps, relative error {err:.3e} (limit 1e-10), |Im|/|lambda| {im:.3e} (limit 1e-12)",
        r.steps
    );
    assert_eq!(r.values.len(), 5);
    assert!(err <= 1e-10, "K2 error {err:e}");
    assert!(im <= 1e-12, "K2 imaginary part {im:e}");
}

#[test]
fn k3_refusals() {
    let eye = |n: usize| CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect());
    let a = CsrMatrix::from_triplets(3, vec![(0, 0, 2.0), (1, 1, 3.0), (2, 2, 4.0)]);
    let o = PencilOptions::new();
    assert!(matches!(
        nearest_pencil_eigenpairs(&a, &eye(3), 0, &o),
        Err(PencilError::InvalidCount { .. })
    ));
    assert!(matches!(
        nearest_pencil_eigenpairs(&a, &eye(3), 3, &o),
        Err(PencilError::InvalidCount { .. })
    ));
    assert!(matches!(
        nearest_pencil_eigenpairs(&a, &eye(2), 1, &o),
        Err(PencilError::DimensionMismatch { .. })
    ));
    let nan = CsrMatrix::from_triplets(3, vec![(0, 0, f64::NAN), (1, 1, 3.0), (2, 2, 4.0)]);
    assert!(matches!(
        nearest_pencil_eigenpairs(&nan, &eye(3), 1, &o),
        Err(PencilError::NonFinite { matrix: "A", .. })
    ));
    let sing = CsrMatrix::from_triplets(
        3,
        vec![(0, 0, 1.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 1.0), (2, 2, 1.0)],
    );
    let e = nearest_pencil_eigenpairs(&sing, &eye(3), 1, &o);
    println!("K3 singular: {e:?}");
    assert!(matches!(e, Err(PencilError::Factor(_))));
    println!("K3: the refusals hold");
}
