//! F-575 (ADR 0492) F5: the bits of the sector split modes and the sector split Campbell table of F-569 (ADR 0482) with the
//! correction option, on the set-ups of ADR 0492, recorded at 12fce5c9 before any library line of F-575 was written. Uses only
//! the API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::gyro::{Basis, HarmonicSplitModes, SectorSplitCampbell};
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

fn options(mb: usize, coriolis: bool, stiffening: bool) -> RotatingOptions {
    let mut o = ModalOptions::new(mb).tolerance(1e-9);
    o.max_iterations = 400;
    RotatingOptions::new(mb)
        .modal(o)
        .coriolis(coriolis)
        .stress_stiffening(stiffening)
        .split_correction(true)
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
    w.extend([s.corrections as u64, s.dropped_corrections as u64]);
    w.extend(opt_bits(&s.eta));
    w
}

fn campbell_words(sc: &SectorSplitCampbell) -> Vec<u64> {
    let mut w = Vec::new();
    for fam in &sc.families {
        for t in &fam.campbell.table.tracks {
            w.extend(opt_bits(&t.eigenvalues));
            w.extend(opt_bits(&t.mac));
        }
        for (wk, wv) in fam.campbell.whirl.iter().zip(&fam.campbell.waves) {
            w.extend(opt_bits(wk));
            w.extend(opt_bits(wv));
        }
        for g in &fam.growth {
            w.extend(opt_bits(g));
        }
        for s in &fam.speeds {
            w.extend(stab_bits(s.stability));
            w.push(basis_bits(s.basis));
        }
    }
    w
}

fn digests() -> [u64; 6] {
    let model = ds();
    let w_ref = {
        let plain = RotatingSector::new(&model, &options(4, true, true).split_correction(false)).unwrap();
        (0..=N / 2)
            .map(|k| match plain.whirl_modes(k, 0.0).unwrap() {
                HarmonicWhirlSpeedModes::Modes { modes, .. } => modes.modes[0].omega,
                HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
            })
            .fold(f64::INFINITY, f64::min)
    };
    let on = RotatingSector::new(&model, &options(4, true, true)).unwrap();
    let mut split_b4 = Vec::new();
    for f in [0.4, 0.8] {
        for k in 0..=N / 2 {
            split_b4.extend(words_of(&on.split_modes(k, f * w_ref).unwrap()));
        }
    }
    let speeds: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let campbell_b4 = campbell_words(&on.split_campbell(&[0, 1, 2, 3], &speeds).unwrap());
    let b41 = RotatingSector::new(&model, &options(41, true, true)).unwrap();
    let mut split_b41 = Vec::new();
    for f in [0.4, 0.8] {
        for k in 0..=N / 2 {
            split_b41.extend(words_of(&b41.split_modes(k, f * w_ref).unwrap()));
        }
    }
    let dss = RotatingSector::new(&model, &options(4, true, false)).unwrap();
    let mut loss = Vec::new();
    for k in 0..=N / 2 {
        loss.extend(words_of(&dss.split_modes(k, 10.0 * w_ref).unwrap()));
    }
    let off = RotatingSector::new(&model, &options(4, false, true)).unwrap();
    let mut split_off = Vec::new();
    for k in 0..=N / 2 {
        split_off.extend(words_of(&off.split_modes(k, 0.4 * w_ref).unwrap()));
    }
    let mut campbell_off = Vec::new();
    for fam in &off.split_campbell(&[0, 1, 2, 3], &speeds).unwrap().families {
        for t in &fam.campbell.table.tracks {
            campbell_off.extend(opt_bits(&t.eigenvalues));
            campbell_off.extend(opt_bits(&t.mac));
        }
    }
    [
        fnv(split_b4),
        fnv(campbell_b4),
        fnv(split_b41),
        fnv(loss),
        fnv(split_off),
        fnv(campbell_off),
    ]
}

/// Prints the digests (run at 12fce5c9 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of F5 (ADR 0492); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at 12fce5c9 (ADR 0492 F5).
const BASE: [u64; 6] = [
    0x5f15_f99f_d204_49a2,
    0x7e64_abfe_4c83_c832,
    0x20a1_0c78_d37c_8568,
    0xc563_2ec9_9b6c_17fa,
    0xa5ec_bf78_33fd_3acc,
    0xba59_4927_8e42_cbb5,
];

#[test]
fn f5_corrected_sector_split_paths_unchanged() {
    let got = digests();
    let names = [
        "DS B4 split_modes, coriolis and split_correction on, k = 0 .. 3, 0.4 and 0.8 omega_ref",
        "DS B4 split_campbell, both options",
        "DS B41 split_modes, both options, 0.4 and 0.8 omega_ref",
        "DSS split_modes at 10 omega_ref, both options",
        "DS B4 split_modes at 0.4 omega_ref, split_correction without coriolis",
        "DS B4 split_campbell, split_correction without coriolis",
    ];
    let mut bad = Vec::new();
    for ((n, g), b) in names.iter().zip(got).zip(BASE) {
        println!(
            "F5 {n}: 0x{g:016x} (base 0x{b:016x}) {}",
            if g == b { "equal" } else { "DIFFERS" }
        );
        if g != b {
            bad.push(*n);
        }
    }
    assert!(bad.is_empty(), "F5 differs: {bad:?}");
}
