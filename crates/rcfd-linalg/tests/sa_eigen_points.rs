//! F-595 (ADR 0531): the eigensolvers with point coordinates for the supernodal factor's nested dissection. Checks P1 to
//! P3 and the negative control N3 (as moved by amendment 1) of the registration.

use rcfd_linalg::eigen::{
    lowest_buckling_factors, lowest_eigenpairs, BucklingEigenOptions, EigenError, Factorisation, SubspaceOptions,
};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

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

fn line_points(n: usize) -> Vec<[f64; 3]> {
    (0..n).map(|i| [i as f64, 0.0, 0.0]).collect()
}

/// The 7-point Laplacian on `m^3` (diagonal 6) and the points `(i, j, k)` of its unknowns.
fn lap7(m: usize) -> (CsrMatrix, Vec<[f64; 3]>) {
    let id = |i: usize, j: usize, k: usize| i + m * j + m * m * k;
    let mut t = Vec::new();
    let mut p = vec![[0.0; 3]; m * m * m];
    for k in 0..m {
        for j in 0..m {
            for i in 0..m {
                let r = id(i, j, k);
                p[r] = [i as f64, j as f64, k as f64];
                t.push((r, r, 6.0));
                let mut nb = |q: usize| t.push((r, q, -1.0));
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
    (CsrMatrix::from_triplets(m * m * m, t), p)
}

fn diagonal(n: usize, v: f64) -> CsrMatrix {
    CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, v)).collect())
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

fn flat(v: &[Vec<f64>]) -> Vec<u64> {
    v.iter().flat_map(|c| bits(c)).collect()
}

fn with_points(points: Option<Vec<[f64; 3]>>) -> SubspaceOptions {
    SubspaceOptions {
        factorisation: Factorisation::Supernodal,
        points,
        ..SubspaceOptions::default()
    }
}

#[test]
fn p1_p2_the_eigensolvers_with_points_against_the_closed_form() {
    let n = 200;
    let k = tridiagonal(n);
    let exact = |j: usize| 2.0 - 2.0 * (j as f64 * PI / (n as f64 + 1.0)).cos();
    let rel = |values: &[f64]| {
        values
            .iter()
            .enumerate()
            .fold(0.0f64, |m, (j, &l)| m.max(((l - exact(j + 1)) / exact(j + 1)).abs()))
    };
    let e = lowest_eigenpairs(&k, &diagonal(n, 1.0), 5, &with_points(Some(line_points(n)))).unwrap();
    let err = rel(&e.values);
    println!(
        "P1: {:?}, max relative error {err:.3e} (limit 1e-9), Sturm {:?}, {} iterations",
        e.values, e.sturm, e.iterations
    );
    assert!(err.is_finite() && err <= 1e-9, "P1 {err:e}");
    assert_eq!(e.sturm.counted, e.sturm.expected, "P1 Sturm");
    let bopts = BucklingEigenOptions {
        factorisation: Factorisation::Supernodal,
        points: Some(line_points(n)),
        ..BucklingEigenOptions::default()
    };
    let b = lowest_buckling_factors(&k, &diagonal(n, -1.0), 3, &bopts).unwrap();
    assert_eq!(b.values.len(), 3, "P2 count");
    let err = rel(&b.values);
    println!(
        "P2: {:?}, max relative error {err:.3e} (limit 1e-9), Sturm {:?}",
        b.values, b.sturm
    );
    assert!(err.is_finite() && err <= 1e-9, "P2 {err:e}");
    assert_eq!(b.sturm.counted, b.sturm.expected, "P2 Sturm");
}

#[test]
fn p3_refusals_of_wrong_points() {
    let n = 200;
    let k = tridiagonal(n);
    let m = diagonal(n, 1.0);
    // (a) 199 points
    match lowest_eigenpairs(&k, &m, 5, &with_points(Some(line_points(n - 1)))) {
        Err(e @ EigenError::DimensionMismatch { .. }) => println!("P3a: {e}"),
        other => panic!("P3a: expected DimensionMismatch, got {other:?}"),
    }
    // (b) a NaN at point 7, component 1
    let mut p = line_points(n);
    p[7][1] = f64::NAN;
    match lowest_eigenpairs(&k, &m, 5, &with_points(Some(p))) {
        Err(EigenError::NonFinite { matrix, row, col }) => {
            println!("P3b: NonFinite {matrix} ({row}, {col})");
            assert_eq!((matrix, row, col), ("points", 7, 1), "P3b");
        }
        other => panic!("P3b: expected NonFinite, got {other:?}"),
    }
    // (c) buckling with 201 points
    let bopts = BucklingEigenOptions {
        factorisation: Factorisation::Supernodal,
        points: Some(line_points(n + 1)),
        ..BucklingEigenOptions::default()
    };
    match lowest_buckling_factors(&k, &diagonal(n, -1.0), 3, &bopts) {
        Err(e @ EigenError::DimensionMismatch { .. }) => println!("P3c: {e}"),
        other => panic!("P3c: expected DimensionMismatch, got {other:?}"),
    }
    // (d) the envelope does not read the points
    let plain = lowest_eigenpairs(&k, &m, 5, &SubspaceOptions::default()).unwrap();
    let given = lowest_eigenpairs(
        &k,
        &m,
        5,
        &SubspaceOptions {
            points: Some(line_points(3)),
            ..SubspaceOptions::default()
        },
    )
    .unwrap();
    assert_eq!(bits(&plain.values), bits(&given.values), "P3d values");
    assert_eq!(flat(&plain.vectors), flat(&given.vectors), "P3d vectors");
    println!("P3d: the envelope with 3 points equals the envelope without points, bit for bit");
}

#[test]
fn n3_the_points_change_the_factor_on_the_grid() {
    let (k, p) = lap7(10);
    let m = diagonal(k.n, 1.0);
    let coords = lowest_eigenpairs(&k, &m, 5, &with_points(Some(p))).unwrap();
    let levels = lowest_eigenpairs(&k, &m, 5, &with_points(None)).unwrap();
    let same = bits(&coords.values) == bits(&levels.values) && flat(&coords.vectors) == flat(&levels.vectors);
    let diff = coords
        .values
        .iter()
        .zip(&levels.values)
        .fold(0.0f64, |w, (a, b)| w.max(((a - b) / b).abs()));
    println!("N3: bit-equal {same}; largest relative eigenvalue difference {diff:.3e}");
    assert!(diff.is_finite(), "N3 finite");
    assert!(!same, "N3: the coordinate order must change the factor");
}
