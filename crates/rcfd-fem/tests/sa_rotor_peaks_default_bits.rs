//! F-564 (ADR 0501) B1: the bits of the existing rotor paths of ADR 0476, recorded at 059d2cb6 before any library line of
//! F-564 was written. Uses only the API of that commit. Each digest is the FNV-1a hash of the `Debug` text of an output.

use rcfd_fem::beam;
use rcfd_fem::rotor::{Bearing, Disc, RotorModel, Unbalance, C64};
use rcfd_fem::Material;

fn fnv(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Model G of ADR 0501 (isotropic bearings, gyroscopic discs).
fn model_g() -> RotorModel {
    let z: Vec<f64> = (0..5).map(|i| 0.25 * i as f64).collect();
    let mut m = RotorModel::new(z).unwrap();
    let steel = Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap();
    let s = beam::circle_section(0.025, 0.9).unwrap();
    for i in 0..4 {
        m.add_shaft(i, i + 1, s, &steel).unwrap();
    }
    m.add_disc(
        2,
        Disc {
            mass: 20.0,
            transverse: 0.05,
            polar: 0.1,
        },
    )
    .unwrap();
    m.add_disc(
        3,
        Disc {
            mass: 5.0,
            transverse: 0.01,
            polar: 0.02,
        },
    )
    .unwrap();
    let b = Bearing::isotropic(2.0e6, 0.0);
    m.add_bearing(0, b).unwrap();
    m.add_bearing(4, b).unwrap();
    m
}

fn digests() -> Vec<u64> {
    let g = model_g();
    let mut out = vec![fnv(&format!("{:?}", g.matrices()))];
    for w in [0.0, 700.0] {
        out.push(fnv(&format!("{:?}", g.whirl(w).unwrap())));
    }
    let speeds = [0.0, 200.0, 400.0, 600.0, 800.0, 1000.0];
    let c = g.campbell(&speeds).unwrap();
    out.push(fnv(&format!("{:?}", c)));
    out.push(fnv(&c.to_csv()));
    out.push(fnv(&format!("{:?}", c.campbell.crossings(1))));
    let loads = [(2usize, [C64::new(1.0, 0.5), C64::new(-0.3, 0.2), C64::ZERO, C64::ZERO])];
    out.push(fnv(&format!(
        "{:?}",
        g.harmonic_response(300.0, 123.4, &loads).unwrap()
    )));
    let ub = [Unbalance {
        node: 2,
        amount: 0.01,
        angle: 0.3,
    }];
    out.push(fnv(&format!("{:?}", g.unbalance_response(400.0, &ub).unwrap())));
    out.push(fnv(&format!("{:?}", g.critical_speeds().unwrap())));
    out
}

/// Prints the digests (run at the base to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B1 (ADR 0501); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

#[test]
fn b1_default_rotor_paths_unchanged() {
    let got = digests();
    assert_eq!(got.len(), BASE.len());
    let bad: Vec<usize> = (0..BASE.len()).filter(|&i| got[i] != BASE[i]).collect();
    for (i, (g, b)) in got.iter().zip(BASE).enumerate() {
        println!(
            "B1 digest {i}: 0x{g:016x} (base 0x{b:016x}) {}",
            if *g == b { "equal" } else { "DIFFERS" }
        );
    }
    assert!(bad.is_empty(), "B1 differs at digests {bad:?}");
}

/// The digests printed at 059d2cb6 (ADR 0501 B1).
const BASE: [u64; 9] = [
    0x2913_446e_7645_cc8f,
    0xd97a_66b5_8aa9_eed4,
    0x228b_f7ad_e55f_f705,
    0x80d5_311e_7c85_7f4d,
    0x2fa2_78ee_e615_05d1,
    0x40b6_98a3_75bb_95e1,
    0xa3d8_af06_3173_aa4b,
    0xd1f9_ec17_9463_6430,
    0x22a4_da61_02b5_4078,
];
