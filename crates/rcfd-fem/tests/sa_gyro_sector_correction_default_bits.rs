//! F-569 (ADR 0482) B1: the bits of the sector split modes and the sector split Campbell table of F-555 (ADR 0481) without the
//! correction option, on the set-ups of ADR 0482, recorded at 44b6c812 before any library line of F-569 was written. Uses only
//! the API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::gyro::{Basis, HarmonicSplitModes};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{HarmonicWhirlSpeedModes, RotatingOptions, RotatingSector, Stability};
use rcfd_fem::{Constraint, Material, SolidModel};
use std::f64::consts::PI;

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

fn stab_bits(s: Option<Stability>) -> [u64; 2] {
    match s {
        None => [0, 0],
        Some(Stability::Stable) => [1, 0],
        Some(Stability::Unstable { growth }) => [2, growth.to_bits()],
    }
}

fn basis_bits(b: Option<Basis>) -> u64 {
    match b {
        None => 0,
        Some(Basis::Speed) => 1,
        Some(Basis::Rest) => 2,
    }
}

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const N: usize = 6;

/// DS of ADR 0411 (`stiffening` false: DSS).
fn ds() -> SolidModel {
    let a = 2.0 * PI / N as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, a, 0.2], [2, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, N))
        .unwrap();
    m
}

fn options(coriolis: bool, stiffening: bool) -> RotatingOptions {
    let mut o = ModalOptions::new(4).tolerance(1e-9);
    o.max_iterations = 400;
    RotatingOptions::new(4)
        .modal(o)
        .coriolis(coriolis)
        .stress_stiffening(stiffening)
}

/// The words of one sector split result.
fn words_of(s: &HarmonicSplitModes) -> Vec<u64> {
    let mut w = vec![s.harmonic as u64, s.multiplicity as u64];
    let m = &s.modes;
    w.push(m.speed.to_bits());
    for sm in &m.modes {
        w.extend([
            sm.omega.to_bits(),
            sm.eigenvalue.to_bits(),
            sm.frequency.to_bits(),
            sm.growth.to_bits(),
            sm.whirl.to_bits(),
            sm.residual.to_bits(),
        ]);
    }
    if let Some(first) = m.modes.first() {
        w.extend(first.re.iter().chain(&first.im).map(|x| x.to_bits()));
    }
    w.extend(opt_bits(&s.waves));
    w.extend(m.roots.iter().flat_map(|r| [r.re.to_bits(), r.im.to_bits()]));
    w.extend(m.real_roots.iter().map(|x| x.to_bits()));
    w.extend(stab_bits(m.stability));
    w.push(basis_bits(m.basis));
    match m.loss {
        None => w.push(0),
        Some(l) => {
            w.push(l.speed.to_bits());
            w.push(l.negative.map_or(u64::MAX, |n| n as u64));
        }
    }
    w.extend([m.asymmetry.to_bits(), m.skew_defect.to_bits(), m.basis_size as u64]);
    w
}

fn digests() -> [u64; 6] {
    let model = ds();
    let on = RotatingSector::new(&model, &options(true, true)).unwrap();
    let w_ref = (0..=N / 2)
        .map(|k| match on.whirl_modes(k, 0.0).unwrap() {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => modes.modes[0].omega,
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
        })
        .fold(f64::INFINITY, f64::min);
    let mut split_on = Vec::new();
    for f in [0.4, 0.8] {
        for k in 0..=N / 2 {
            split_on.extend(words_of(&on.split_modes(k, f * w_ref).unwrap()));
        }
    }
    let off = RotatingSector::new(&model, &options(false, true)).unwrap();
    let mut split_off = Vec::new();
    for f in [0.4, 0.8] {
        for k in 0..=N / 2 {
            split_off.extend(words_of(&off.split_modes(k, f * w_ref).unwrap()));
        }
    }
    let speeds: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let sc = on.split_campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let mut ev = Vec::new();
    let mut rest = Vec::new();
    for fam in &sc.families {
        for t in &fam.campbell.table.tracks {
            ev.extend(opt_bits(&t.eigenvalues));
            ev.extend(opt_bits(&t.mac));
        }
        for (wk, wv) in fam.campbell.whirl.iter().zip(&fam.campbell.waves) {
            rest.extend(opt_bits(wk));
            rest.extend(opt_bits(wv));
        }
        for g in &fam.growth {
            rest.extend(opt_bits(g));
        }
        for s in &fam.speeds {
            rest.extend(stab_bits(s.stability));
            rest.push(basis_bits(s.basis));
        }
    }
    let sc_off = off.split_campbell(&[0, 1, 2, 3], &speeds).unwrap();
    let mut ev_off = Vec::new();
    for fam in &sc_off.families {
        for t in &fam.campbell.table.tracks {
            ev_off.extend(opt_bits(&t.eigenvalues));
            ev_off.extend(opt_bits(&t.mac));
        }
    }
    let dss = RotatingSector::new(&model, &options(true, false)).unwrap();
    let mut loss = Vec::new();
    for k in 0..=N / 2 {
        loss.extend(words_of(&dss.split_modes(k, 10.0 * w_ref).unwrap()));
    }
    [
        fnv(split_on),
        fnv(split_off),
        fnv(ev),
        fnv(rest),
        fnv(ev_off),
        fnv(loss),
    ]
}

/// Prints the digests (run at 44b6c812 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B1 (ADR 0482); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at 44b6c812 (ADR 0482 B1).
const BASE: [u64; 6] = [
    0xb310_30b8_b5dc_2278,
    0xe920_86f0_8003_43cc,
    0x6430_4ff3_eb30_056b,
    0x6c02_adde_f6c9_0de6,
    0xba59_4927_8e42_cbb5,
    0xc7e0_0b0b_8f4c_cd4f,
];

#[test]
fn b1_sector_split_paths_without_the_option_unchanged() {
    let got = digests();
    let names = [
        "DS B4 split_modes, coriolis on, k = 0 .. 3, 0.4 and 0.8 omega_ref",
        "DS B4 split_modes, coriolis off",
        "DS B4 split_campbell on: eigenvalues and MAC",
        "DS B4 split_campbell on: whirl, waves, growth, stability, basis",
        "DS B4 split_campbell off: eigenvalues and MAC",
        "DSS split_modes at 10 omega_ref (rest basis)",
    ];
    let mut bad = Vec::new();
    for ((n, g), b) in names.iter().zip(got).zip(BASE) {
        println!(
            "B1 {n}: 0x{g:016x} (base 0x{b:016x}) {}",
            if g == b { "equal" } else { "DIFFERS" }
        );
        if g != b {
            bad.push(*n);
        }
    }
    assert!(bad.is_empty(), "B1 differs: {bad:?}");
}
