//! F-642 (ADR 0559): `lowest_buckling_factors` on a sign-asymmetric spectrum, the near Sturm count of D1. Checks P1 and the
//! negative control N1 of the registration (A1 to A4 and B2 are in `rcfd-fem/tests/sa_buckling_sign_asymmetric_f642.rs`).
//! Each registered run is made once; every value is printed.

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

/// P1: three positive factors `1, 1/0.9, 1/0.8` among eight negative ones of smaller magnitude and nine larger positive ones;
/// the default subspace of 11 holds exactly the three wanted positive Ritz values.
#[test]
fn p1_near_count_returns_the_lowest_factors() {
    let mut c = Checks::new();
    let mut d = vec![-1.0, -0.9, -0.8];
    d.extend((10..=17).map(|i| i as f64));
    d.extend((0..9).map(|i| -(0.5 - 0.05 * i as f64)));
    assert_eq!(d.len(), 20);
    let k = diag(&[1.0; 20]);
    let kg = diag(&d);
    match lowest_buckling_factors(&k, &kg, 3, &BucklingEigenOptions::default()) {
        Ok(r) => {
            println!(
                "P1: factors {:?}, subspace {}, iterations {}, Sturm {:?}, exhausted {:?}",
                r.values, r.subspace_size, r.iterations, r.sturm, r.exhausted
            );
            c.holds("P1 three factors", r.values.len() == 3, &format!("{}", r.values.len()));
            for (j, want) in [1.0, 1.0 / 0.9, 1.0 / 0.8].iter().enumerate() {
                let got = r.values.get(j).copied().unwrap_or(f64::NAN);
                c.at_most(&format!("P1 factor {}", j + 1), (got / want - 1.0).abs(), 1e-12);
            }
            c.holds(
                "P1 Sturm record",
                r.sturm.counted == 3 && r.sturm.expected == 3 && r.sturm.shift > 1.25 && r.sturm.shift <= 1.2625,
                &format!("{:?}", r.sturm),
            );
        }
        Err(e) => c.holds("P1 returned", false, &format!("refused: {e:?}")),
    }
    c.finish();
}

/// N1: the factor 0.5 is invisible to the start vectors (zero first entry); the subspace finds 1.0 only. The near count
/// sees both and the solver keeps the refusal of the far bound.
#[test]
fn n1_hidden_lower_factor_is_still_refused() {
    let mut c = Checks::new();
    let k = diag(&[1.0; 6]);
    let kg = diag(&[-2.0, -1.0, 300.0, 400.0, 1e-7, 1e-7]);
    let opts = BucklingEigenOptions {
        subspace_size: Some(3),
        start: Some(vec![
            vec![0.0, 1.0, 0.3, -0.7, 0.2, 0.5],
            vec![0.0, -0.4, 1.0, 0.6, -0.3, 0.1],
            vec![0.0, 0.2, -0.5, 1.0, 0.8, -0.6],
        ]),
        ..BucklingEigenOptions::default()
    };
    let bound = 1.0 / (1e-8 * 400.0);
    match lowest_buckling_factors(&k, &kg, 1, &opts) {
        Err(EigenError::SturmMismatch {
            shift,
            counted,
            expected,
        }) => {
            println!("N1: SturmMismatch at {shift:.12e}: counted {counted}, expected {expected}");
            c.holds(
                "N1 counted 2, expected 1",
                counted == 2 && expected == 1,
                &format!("{counted} {expected}"),
            );
            c.at_most("N1 shift against S", (shift / bound - 1.0).abs(), 1e-12);
        }
        other => c.holds("N1 refused", false, &format!("got {other:?}")),
    }
    c.finish();
}
