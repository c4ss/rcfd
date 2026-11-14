//! F-415 (ADR 0472) B2: the bits of the default rotating paths next to the split Campbell table (the F-396 Campbell table, the
//! F-499 whirl Campbell table, the F-505 stability over speeds, the F-396 modes of the radial beam at speed) on the set-ups of
//! ADR 0472, recorded on main 4abff5fe before any library line of F-415 was written. Uses only the API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, SpeedModes, Stability};
use rcfd_fem::{Constraint, ElasticSupport, Material, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// The FNV-1a hash of a sequence of 64-bit words.
fn fnv(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn opt_bits(v: &[Option<f64>]) -> impl Iterator<Item = u64> + '_ {
    v.iter().map(|x| x.map_or(u64::MAX, f64::to_bits))
}

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const RHO: f64 = 7850.0;
const W0: f64 = 1000.0;
const H: f64 = 0.1;

/// CB (`ky = 1`) and CBA (`ky = 2.25`) of ADR 0472.
fn cube(ky: f64) -> SolidModel {
    let mesh = hex_box([-H / 2.0; 3], [H; 3], [1, 1, 1]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    let kn = RHO * H * H * H * W0 * W0 / 8.0;
    m.add_support(ElasticSupport::nodal_spring("all", 0, kn)).unwrap();
    m.add_support(ElasticSupport::nodal_spring("all", 1, ky * kn)).unwrap();
    m
}

/// F of ADR 0472 (the beam of ADR 0408).
fn beam() -> SolidModel {
    let base = hex_box([0.0, -0.025, -0.025], [1.0, 0.05, 0.05], [20, 2, 2]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn digests() -> [u64; 8] {
    let cb = cube(1.0);
    let speeds: Vec<f64> = (0..=12).map(|j| 0.07 * j as f64 * W0).collect();
    let part = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(3)).unwrap();
    let table = part.campbell(&speeds).unwrap();
    let mut ev = Vec::new();
    let mut fr = Vec::new();
    let mut mac = Vec::new();
    for t in &table.tracks {
        ev.extend(opt_bits(&t.eigenvalues));
        fr.extend(opt_bits(&t.frequencies));
        mac.extend(opt_bits(&t.mac));
    }
    let on = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(3).coriolis(true)).unwrap();
    let wc = on.whirl_campbell(&speeds).unwrap();
    let mut wev = Vec::new();
    let mut wk = Vec::new();
    for (t, k) in wc.table.tracks.iter().zip(&wc.whirl) {
        wev.extend(opt_bits(&t.eigenvalues));
        wk.extend(opt_bits(k));
    }
    let cba = cube(2.25);
    let pa = RotatingPart::new(&cba, AXIS, ORIGIN, &RotatingOptions::new(3).coriolis(true)).unwrap();
    let st = pa
        .whirl_stability(&[0.0, 0.5 * W0, 1.2 * W0, 1.8 * W0])
        .unwrap();
    let growth: Vec<u64> = st
        .results
        .iter()
        .map(|r| match r.stability {
            Stability::Stable => 0,
            Stability::Unstable { growth } => growth.to_bits(),
        })
        .collect();
    let f = beam();
    let fo = RotatingOptions::new(14).modal(
        ModalOptions::new(14)
            .residual_test(ResidualTest::Backward)
            .tolerance(1e-12),
    );
    let fp = RotatingPart::new(&f, AXIS, ORIGIN, &fo).unwrap();
    let rest = match fp.modes(0.0).unwrap() {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
    };
    let omega = 0.2 * rest.eigenvalues[0].sqrt();
    let r = match fp.modes(omega).unwrap() {
        SpeedModes::Modes(r) => r,
        SpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
    };
    [
        fnv(ev),
        fnv(fr),
        fnv(mac),
        fnv(wev),
        fnv(wk),
        fnv(growth),
        fnv(r.eigenvalues.iter().map(|x| x.to_bits())),
        fnv(r.modes[0].iter().map(|x| x.to_bits())),
    ]
}

/// Prints the digests (run at main 4abff5fe to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B2 (ADR 0472); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 4abff5fe (ADR 0472 B2).
const BASE: [u64; 8] = [
    0x0c67_04e7_07f1_6bc4,
    0x4199_4df0_8b96_64f9,
    0x9a73_a278_b290_f286,
    0xf221_6a84_5948_85bf,
    0xac73_ee7d_8eb9_9bc7,
    0xc895_ec42_dc69_e489,
    0xc6cb_5ca8_4a66_edbe,
    0x3d13_0109_0675_d0a7,
];

#[test]
fn b2_default_rotating_paths_unchanged() {
    let got = digests();
    let names = [
        "CB campbell eigenvalues",
        "CB campbell frequencies",
        "CB campbell MAC",
        "CB whirl_campbell eigenvalues",
        "CB whirl_campbell whirl numbers",
        "CBA whirl_stability growth",
        "F modes(Omega_F) eigenvalues",
        "F modes(Omega_F) first mode",
    ];
    let mut bad = Vec::new();
    for ((n, g), b) in names.iter().zip(got).zip(BASE) {
        println!("B2 {n}: 0x{g:016x} (base 0x{b:016x}) {}", if g == b { "equal" } else { "DIFFERS" });
        if g != b {
            bad.push(*n);
        }
    }
    assert!(bad.is_empty(), "B2 differs: {bad:?}");
}
