//! F-563 (ADR 0498) B1: the bits of the constant-bearing paths of the flexible rotor (ADR 0476), recorded at 9386de34 before any
//! library line of F-563 was written. Uses only the API of that commit. Each digest is the FNV-1a hash of the `Debug` text of
//! an output (`f64` Debug prints the shortest text that round-trips, so equal text is equal bits).

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

/// Model X (cross-coupled bearings) or Y (symmetric) of ADR 0498 D7.
fn model(cross: bool) -> RotorModel {
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
    let k = if cross {
        [[2.0e6, 3.0e5], [-1.0e5, 3.0e6]]
    } else {
        [[2.0e6, 1.0e5], [1.0e5, 3.0e6]]
    };
    let b = Bearing::new(k, [[200.0, 20.0], [20.0, 300.0]]);
    m.add_bearing(0, b).unwrap();
    m.add_bearing(4, b).unwrap();
    m
}

fn digests() -> Vec<u64> {
    let x = model(true);
    let y = model(false);
    let mut out = Vec::new();
    let mx = x.matrices();
    out.push(fnv(&format!("{:?}", mx)));
    for w in [0.0, 300.0, 900.0] {
        out.push(fnv(&format!("{:?}", x.whirl(w).unwrap())));
    }
    let speeds = [0.0, 200.0, 400.0, 600.0, 800.0];
    let c = x.campbell(&speeds).unwrap();
    out.push(fnv(&format!("{:?}", c)));
    out.push(fnv(&c.to_csv()));
    let loads = [(2usize, [C64::new(1.0, 0.5), C64::new(-0.3, 0.2), C64::ZERO, C64::ZERO])];
    out.push(fnv(&format!(
        "{:?}",
        x.harmonic_response(300.0, 123.4, &loads).unwrap()
    )));
    let ub = [
        Unbalance {
            node: 2,
            amount: 0.01,
            angle: 0.3,
        },
        Unbalance {
            node: 3,
            amount: 0.004,
            angle: -1.0,
        },
    ];
    out.push(fnv(&format!("{:?}", x.unbalance_response(300.0, &ub).unwrap())));
    out.push(fnv(&format!("{:?}", x.unbalance_response(900.0, &ub).unwrap())));
    out.push(fnv(&format!("{:?}", y.matrices())));
    out.push(fnv(&format!("{:?}", y.critical_speeds().unwrap())));
    out.push(fnv(&format!("{:?}", y.whirl(500.0).unwrap())));
    out
}

/// Prints the digests (run at the base to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B1 (ADR 0498); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at 9386de34 (ADR 0498 B1).
const BASE: [u64; 12] = [
    0x3bb6_8023_88b3_ade3,
    0x7bc5_30e4_9bea_a3a6,
    0xcfb7_6bea_0569_1e7a,
    0x5af2_bfa4_beea_9a5c,
    0x735c_5f23_8392_8928,
    0xbbe7_6a49_2174_377e,
    0xfcaa_c9bc_7552_d3ea,
    0x5309_4eec_5476_49ec,
    0x0d04_3226_f1b0_15a7,
    0x73d0_7a85_8f32_5983,
    0xa397_651e_b0f5_eb6b,
    0xecea_05fd_7876_83e0,
];

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
