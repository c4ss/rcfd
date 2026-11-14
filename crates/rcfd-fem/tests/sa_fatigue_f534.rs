//! F-534 (ADR 0453): rainflow residue guard and the Gerber compressive-mean message. Checks P1, P2, G1 to G3.

use rcfd_core::rng::Rng;
use rcfd_fem::fatigue::{rainflow_periodic, MeanStress, MeanStressCorrection};
use rcfd_fem::FemError;
use std::f64::consts::PI;

const SU: f64 = 600e6;
const SY: f64 = 400e6;

fn parts(e: &FemError) -> (&'static str, String) {
    match e {
        FemError::InvalidValue { key, reason } => (key, reason.clone()),
        _ => ("", e.to_string()),
    }
}

#[test]
fn p1_p2_periodic_counts_ok_with_empty_residue() {
    let s = 120e6;
    for n in [1usize, 7, 1000] {
        let h: Vec<f64> = (0..n).flat_map(|_| [s, -s]).collect();
        let r = rainflow_periodic(&h).expect("P1: constant amplitude must not be refused");
        assert_eq!(r.cycles.len(), n, "P1 n {n}");
        assert!(
            r.cycles
                .iter()
                .all(|c| c.range == 2.0 * s && c.mean == 0.0 && c.count == 1.0),
            "P1 n {n}"
        );
        assert!(r.residue.is_empty(), "P1 n {n}");
        println!("P1 n {n}: met");
    }
    let sine: Vec<f64> = (0..16 * 50).map(|k| s * (2.0 * PI * k as f64 / 16.0).sin()).collect();
    let r = rainflow_periodic(&sine).expect("P2 sine");
    assert!(r.residue.is_empty());
    let mut rng = Rng::new(0xF534_0453);
    for i in 0..200 {
        let len = 12 + (rng.range(0.0, 1.0) * 48.0) as usize;
        let h: Vec<f64> = (0..len).map(|_| rng.range(-1.0, 1.0) * 100e6).collect();
        let r = rainflow_periodic(&h).unwrap_or_else(|e| panic!("P2 history {i}: {e}"));
        assert!(r.residue.is_empty(), "P2 history {i}");
    }
    println!("P2: met");
}

#[test]
fn g1_g2_g3_gerber_message() {
    let gerb = MeanStressCorrection::new(MeanStress::Gerber { ultimate: SU });
    let good = MeanStressCorrection::new(MeanStress::Goodman { ultimate: SU });
    let sode = MeanStressCorrection::new(MeanStress::Soderberg { yield_strength: SY });
    let cred = gerb.with_compressive_credit(true);
    for sm in [-SU, -1.1 * SU] {
        let e = cred.equivalent_amplitude(100e6, sm).expect_err("G1 must be refused");
        let (key, msg) = parts(&e);
        assert_eq!(key, "mean stress");
        assert!(
            msg.contains("in modulus") && msg.contains("at or above the strength"),
            "G1 {msg}"
        );
        println!("G1 {sm:e}: met ({msg})");
    }
    for (name, k, sm, strength) in [
        ("Goodman", good, SU, SU),
        ("Gerber", gerb, SU, SU),
        ("Soderberg", sode, SY, SY),
    ] {
        let e = k.equivalent_amplitude(100e6, sm).expect_err("G2 must be refused");
        let (key, msg) = parts(&e);
        let want =
            format!("mean stress {sm:e} at or above the strength {strength:e} ({name}): no equivalent amplitude");
        assert_eq!(key, "mean stress");
        assert!(!msg.contains("in modulus"), "G2 {msg}");
        assert!(msg.contains(&want), "G2 {msg} vs {want}");
        println!("G2 {name}: met");
    }
    assert!(
        good.with_compressive_credit(true)
            .equivalent_amplitude(100e6, -SU)
            .is_ok(),
        "G3 Goodman"
    );
    assert!(
        sode.with_compressive_credit(true)
            .equivalent_amplitude(100e6, -SY)
            .is_ok(),
        "G3 Soderberg"
    );
    assert!(cred.equivalent_amplitude(100e6, -0.999 * SU).is_ok(), "G3 Gerber");
    println!("G3: met");
}
