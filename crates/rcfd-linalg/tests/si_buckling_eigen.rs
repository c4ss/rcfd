//! F-395 (ADR 0390): the buckling eigensolver `lowest_buckling_factors`. Checks A4, A5a, L1 to L6 and the negative
//! control N3 of the registration (the model checks are in `rcfd-fem/tests/si_geometric.rs`).

use rcfd_linalg::eigen::{generalized_eigen_dense, lowest_buckling_factors, BucklingEigenOptions, EigenError};
use rcfd_linalg::CsrMatrix;
use std::f64::consts::PI;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (expected > {limit:e}) {}",
            if ok { "as expected" } else { "NOT AS EXPECTED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

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

fn reference(j: usize, n: usize) -> f64 {
    4.0 * (j as f64 * PI / (2.0 * (n as f64 + 1.0))).sin().powi(2)
}

fn max_rel_to_reference(values: &[f64], n: usize, scale: f64) -> f64 {
    values
        .iter()
        .enumerate()
        .map(|(i, &l)| (l / (scale * reference(i + 1, n)) - 1.0).abs())
        .fold(0.0, f64::max)
}

#[test]
fn a4_l1_l6_tridiagonal_against_closed_form() {
    let mut c = Checks::new();
    let n = 200;
    let k = second_difference(n);
    let kg = diag(&vec![-1.0; n]);
    let opts = BucklingEigenOptions::default();
    let r = lowest_buckling_factors(&k, &kg, 10, &opts).expect("A4 solves");
    println!(
        "A4: iterations {}, subspace {}, factors {:?}",
        r.iterations, r.subspace_size, r.values
    );
    c.holds(
        "A4 count",
        r.values.len() == 10 && r.exhausted.is_none(),
        &format!("{} factors, exhausted {:?}", r.values.len(), r.exhausted),
    );
    c.at_most(
        "A4 max relative difference",
        max_rel_to_reference(&r.values, n, 1.0),
        1e-10,
    );
    c.holds(
        "L1 Sturm",
        r.sturm.counted == 10 && r.sturm.expected == 10,
        &format!(
            "counted {} expected {} at s = {:e}",
            r.sturm.counted, r.sturm.expected, r.sturm.shift
        ),
    );
    let again = lowest_buckling_factors(&k, &kg, 10, &opts).expect("A4 again");
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let same =
        bits(&r.values) == bits(&again.values) && r.vectors.iter().zip(&again.vectors).all(|(a, b)| bits(a) == bits(b));
    c.holds("L6 bit-equal on a second call", same, "values and vectors");
    // N3: K_sigma = -2 I halves the factors.
    let r2 = lowest_buckling_factors(&k, &diag(&vec![-2.0; n]), 10, &opts).expect("N3 solves");
    c.above(
        "N3 A4's measure with K_sigma = -2 I",
        max_rel_to_reference(&r2.values, n, 1.0),
        1e-10,
    );
    c.finish();
}

#[test]
fn a5a_no_negative_direction_returns_no_factor() {
    let mut c = Checks::new();
    let n = 200;
    let k = second_difference(n);
    let pos = diag(&vec![1.0; n]);
    let semi: Vec<f64> = (0..n).map(|i| (i % 2) as f64).collect();
    for (name, kg) in [("K_sigma = +I", pos), ("K_sigma = diag(0, 1, 0, 1, ...)", diag(&semi))] {
        let r = lowest_buckling_factors(&k, &kg, 10, &BucklingEigenOptions::default());
        match r {
            Ok(r) => c.holds(
                &format!("A5a {name}"),
                r.values.is_empty() && r.vectors.is_empty() && r.exhausted.is_some() && r.sturm.counted == 0,
                &format!(
                    "{} factors, exhausted {:?}, Sturm counted {} at s = {:e}, iterations {}",
                    r.values.len(),
                    r.exhausted,
                    r.sturm.counted,
                    r.sturm.shift,
                    r.iterations
                ),
            ),
            Err(e) => c.holds(&format!("A5a {name}"), false, &format!("refused: {e}")),
        }
    }
    c.finish();
}

#[test]
fn l2_mixed_signs_against_dense_reference() {
    let mut c = Checks::new();
    let n = 200;
    let k = second_difference(n);
    let d: Vec<f64> = (0..n).map(|i| if i < 100 { -1.0 } else { 1.0 }).collect();
    let kg = diag(&d);
    let opts = BucklingEigenOptions {
        subspace_size: Some(30),
        ..BucklingEigenOptions::default()
    };
    let r = lowest_buckling_factors(&k, &kg, 5, &opts).expect("L2 solves");
    let a: Vec<f64> = dense(&kg).iter().map(|v| -v).collect();
    let full = generalized_eigen_dense(&a, &dense(&k), n).expect("dense reference");
    let mut refs: Vec<f64> = full
        .values
        .iter()
        .rev()
        .filter(|&&t| t > 0.0)
        .map(|t| 1.0 / t)
        .collect();
    refs.truncate(5);
    println!(
        "L2: factors {:?}, reference {:?}, iterations {}",
        r.values, refs, r.iterations
    );
    let worst = r
        .values
        .iter()
        .zip(&refs)
        .map(|(l, x)| (l / x - 1.0).abs())
        .fold(0.0, f64::max);
    c.holds("L2 count", r.values.len() == 5, &format!("{} factors", r.values.len()));
    c.at_most("L2 max relative difference", worst, 1e-9);
    c.finish();
}

#[test]
fn l3_l4_hidden_factor_is_refused_then_found() {
    let mut c = Checks::new();
    let n = 10;
    let k = diag(&vec![1.0; n]);
    let d: Vec<f64> = (0..n).map(|i| if i == 0 { -1.0 } else { 100.0 * i as f64 }).collect();
    let kg = diag(&d);
    let small = BucklingEigenOptions {
        subspace_size: Some(3),
        ..BucklingEigenOptions::default()
    };
    match lowest_buckling_factors(&k, &kg, 1, &small) {
        Err(EigenError::SturmMismatch {
            shift,
            counted,
            expected,
        }) => c.holds(
            "L3 refused",
            counted == 1 && expected == 0,
            &format!("SturmMismatch at s = {shift:e}: counted {counted}, expected {expected}"),
        ),
        other => c.holds("L3 refused", false, &format!("got {other:?}")),
    }
    let whole = BucklingEigenOptions {
        subspace_size: Some(10),
        ..BucklingEigenOptions::default()
    };
    match lowest_buckling_factors(&k, &kg, 1, &whole) {
        Ok(r) if r.values.len() == 1 => c.at_most("L4 relative difference", (r.values[0] - 1.0).abs(), 1e-12),
        other => c.holds("L4 found", false, &format!("got {other:?}")),
    }
    c.finish();
}

#[test]
fn l5_refusals() {
    let mut c = Checks::new();
    let n = 200;
    let k = second_difference(n);
    let kg = diag(&vec![-1.0; n]);
    let opts = BucklingEigenOptions::default();
    let neg = tridiag(&vec![-2.0; n], 1.0);
    let r = lowest_buckling_factors(&neg, &kg, 3, &opts);
    c.holds(
        "L5 K not positive definite",
        matches!(r, Err(EigenError::ShiftNotPositiveDefinite { .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    let r = lowest_buckling_factors(&k, &kg, 0, &opts);
    c.holds(
        "L5 count 0",
        matches!(r, Err(EigenError::BadCount { .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    let r = lowest_buckling_factors(&k, &diag(&vec![-1.0; n - 1]), 3, &opts);
    c.holds(
        "L5 sizes 200 and 199",
        matches!(r, Err(EigenError::DimensionMismatch { .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    let mut t: Vec<(usize, usize, f64)> = (0..n).map(|i| (i, i, -1.0)).collect();
    t.push((0, 1, 0.5));
    let unsym = CsrMatrix::from_triplets(n, t);
    let r = lowest_buckling_factors(&k, &unsym, 3, &opts);
    c.holds(
        "L5 unsymmetric K_sigma",
        matches!(r, Err(EigenError::Unsymmetric { matrix: "K_sigma", .. })),
        &format!("{:?}", r.as_ref().err()),
    );
    c.finish();
}
