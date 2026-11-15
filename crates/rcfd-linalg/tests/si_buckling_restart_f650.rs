//! F-650 (ADR 0564): `lowest_buckling_factors` when the subspace holds fewer positive Ritz values than asked, the restart
//! of D1. Checks P1, P2 and the negative control N1 of the registration (A1 to A3 and B2 are in
//! `rcfd-fem/tests/sa_buckling_restart_f650.rs`). Each registered run is made once; every value is printed.

use rcfd_linalg::eigen::{lowest_buckling_factors, BucklingEigenOptions, EigenError};
use rcfd_linalg::CsrMatrix;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
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

fn diag(v: &[f64]) -> CsrMatrix {
    CsrMatrix::from_triplets(v.len(), v.iter().enumerate().map(|(i, &x)| (i, i, x)).collect())
}

fn bits(v: &[f64]) -> Vec<u64> {
    v.iter().map(|x| x.to_bits()).collect()
}

/// The P1 pencil: positive factors `1, 2, 10`, nine negative factors `-1/10 .. -1/18` and eight at `-100`. The default
/// subspace of 11 holds the nine negative ones and only two positive ones.
fn p1_pencil() -> (CsrMatrix, CsrMatrix) {
    let mut d = vec![-1.0, -0.5, -0.1];
    d.extend((10..=18).map(|i| i as f64));
    d.extend([0.01; 8]);
    assert_eq!(d.len(), 20);
    (diag(&[1.0; 20]), diag(&d))
}

#[test]
fn p1_p2_restart_returns_the_lowest_factors() {
    let mut c = Checks::new();
    let (k, kg) = p1_pencil();
    let r = match lowest_buckling_factors(&k, &kg, 3, &BucklingEigenOptions::default()) {
        Ok(r) => r,
        Err(e) => {
            c.holds("P1 returned", false, &format!("refused: {e:?}"));
            c.finish();
            return;
        }
    };
    println!(
        "P1: factors {:?}, subspace {}, iterations {}, Sturm {:?}, exhausted {:?}",
        r.values, r.subspace_size, r.iterations, r.sturm, r.exhausted
    );
    c.holds("P1 three factors", r.values.len() == 3, &format!("{}", r.values.len()));
    for (j, want) in [1.0, 2.0, 10.0].iter().enumerate() {
        let got = r.values.get(j).copied().unwrap_or(f64::NAN);
        c.at_most(&format!("P1 factor {}", j + 1), (got / want - 1.0).abs(), 1e-12);
    }
    c.holds("P1 subspace 12", r.subspace_size == 12, &format!("{}", r.subspace_size));
    c.holds(
        "P1 Sturm counted 3, expected 3",
        r.sturm.counted == 3 && r.sturm.expected == 3,
        &format!("{:?}", r.sturm),
    );
    c.at_most(
        "P1 Sturm shift against 1/(1e-8 * 18)",
        (r.sturm.shift / (1.0 / (1e-8 * 18.0)) - 1.0).abs(),
        1e-12,
    );
    c.holds(
        "P1 exhausted None",
        r.exhausted.is_none(),
        &format!("{:?}", r.exhausted),
    );
    // P2: the restart is the solve with a subspace of 12 from the default start.
    let o12 = BucklingEigenOptions {
        subspace_size: Some(12),
        ..BucklingEigenOptions::default()
    };
    match lowest_buckling_factors(&k, &kg, 3, &o12) {
        Ok(e) => {
            println!("P2: q 12 iterations {}, Sturm {:?}", e.iterations, e.sturm);
            let same = bits(&r.values) == bits(&e.values)
                && r.vectors.len() == e.vectors.len()
                && r.vectors.iter().zip(&e.vectors).all(|(a, b)| bits(a) == bits(b))
                && bits(&r.residuals) == bits(&e.residuals)
                && r.sturm.shift.to_bits() == e.sturm.shift.to_bits()
                && r.sturm.counted == e.sturm.counted
                && r.sturm.expected == e.sturm.expected
                && r.exhausted.map(f64::to_bits) == e.exhausted.map(f64::to_bits);
            c.holds("P2 bit-equal to the q 12 solve", same, "");
            c.holds(
                "P2 iterations summed",
                r.iterations > e.iterations,
                &format!("{} against {}", r.iterations, e.iterations),
            );
        }
        Err(e) => c.holds("P2 q 12 solve", false, &format!("refused: {e:?}")),
    }
    c.finish();
}

/// N1: the L3 pencil (no positive Ritz value in a subspace of 3): the restart is not made, the refusal is unchanged.
#[test]
fn n1_no_positive_ritz_value_keeps_the_refusal() {
    let mut c = Checks::new();
    let n = 10;
    let k = diag(&vec![1.0; n]);
    let d: Vec<f64> = (0..n).map(|i| if i == 0 { -1.0 } else { 100.0 * i as f64 }).collect();
    let o = BucklingEigenOptions {
        subspace_size: Some(3),
        ..BucklingEigenOptions::default()
    };
    match lowest_buckling_factors(&k, &diag(&d), 1, &o) {
        Err(EigenError::SturmMismatch {
            shift,
            counted,
            expected,
        }) => {
            c.holds(
                "N1 counted 1, expected 0",
                counted == 1 && expected == 0,
                &format!("shift {shift:e}, counted {counted}, expected {expected}"),
            );
            c.at_most(
                "N1 shift against 1/(1e-8 * 900)",
                (shift / (1.0 / (1e-8 * 900.0)) - 1.0).abs(),
                1e-12,
            );
        }
        other => c.holds("N1 refused", false, &format!("got {other:?}")),
    }
    c.finish();
}
