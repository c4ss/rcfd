//! F-562 (ADR 0504) B1: the bits of the existing `whirl` paths of ADR 0476 and of its refusal of a massless damped unknown,
//! recorded at b7bffcac (d85922b8 plus the registration) before any library line of F-562 was written. Uses only the API of
//! that commit. Each digest is the FNV-1a hash of the `Debug` text of an output.

use rcfd_fem::beam;
use rcfd_fem::rotor::{Bearing, Disc, RotorModel};
use rcfd_fem::Material;

fn fnv(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Two-bearing flexible rotor with two gyroscopic discs and cross-coupled damped bearings.
fn model_two_disc() -> RotorModel {
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
    let b = Bearing::new([[2.0e6, 3.0e5], [-1.0e5, 3.0e6]], [[200.0, 20.0], [20.0, 300.0]]);
    m.add_bearing(0, b).unwrap();
    m.add_bearing(4, b).unwrap();
    m
}

/// Massless shaft, a disc at the middle, the pedestals at the ends with damping `cp` (0: massless undamped, condensed).
fn model_pedestal(cp: f64) -> RotorModel {
    let z: Vec<f64> = (0..5).map(|i| 0.25 * i as f64).collect();
    let mut m = RotorModel::new(z).unwrap();
    let mat = Material::isotropic_elastic(2.0e11, 0.3, 0.0, 0.0).unwrap();
    let s = beam::circle_section(0.025, 0.9).unwrap();
    for i in 0..4 {
        m.add_shaft(i, i + 1, s, &mat).unwrap();
    }
    m.add_disc(
        2,
        Disc {
            mass: 20.0,
            transverse: 0.0,
            polar: 0.0,
        },
    )
    .unwrap();
    m.add_bearing(0, Bearing::isotropic(2.0e6, cp)).unwrap();
    m.add_bearing(4, Bearing::isotropic(2.0e6, cp)).unwrap();
    m
}

/// Jeffcott disc with a damper at the disc and massless undamped end stations.
fn model_jeffcott() -> RotorModel {
    let mut m = model_pedestal(0.0);
    m.add_bearing(2, Bearing::isotropic(0.0, 150.0)).unwrap();
    m
}

fn digests() -> Vec<u64> {
    let a = model_two_disc();
    let b = model_pedestal(0.0);
    let c = model_jeffcott();
    let d = model_pedestal(2000.0);
    let mut out = Vec::new();
    for w in [0.0, 300.0, 900.0] {
        out.push(fnv(&format!("{:?}", a.whirl(w).unwrap())));
    }
    for w in [0.0, 500.0] {
        out.push(fnv(&format!("{:?}", b.whirl(w).unwrap())));
    }
    for w in [0.0, 100.0] {
        out.push(fnv(&format!("{:?}", c.whirl(w).unwrap())));
    }
    out.push(fnv(&format!("{:?}", d.whirl(0.0))));
    out.push(fnv(&format!("{:?}", d.whirl(500.0))));
    out
}

/// Prints the digests (run at the base to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B1 (ADR 0504); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

#[test]
fn b1_default_whirl_paths_unchanged() {
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

/// The digests printed at b7bffcac (ADR 0504 B1).
const BASE: [u64; 9] = [
    0x7bc5_30e4_9bea_a3a6,
    0xcfb7_6bea_0569_1e7a,
    0x5af2_bfa4_beea_9a5c,
    0xfb80_5236_63dd_c29a,
    0x518e_9507_6f7b_373d,
    0xf20c_e81c_130d_d342,
    0x4434_6658_fd6d_f3f1,
    0x2964_8578_ceb9_3e84,
    0x2964_8578_ceb9_3e84,
];
