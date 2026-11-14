//! F-612 (ADR 0535) J7: the bits of the sector split modes of the `Perturbation` and `Dense` runs of F-584 (ADR 0527, with
//! `schur_eta` and `corrected_frequency`), recorded at 679081a1 before any library line of F-612 was written. Uses only the
//! API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::gyro::{Basis, HarmonicSplitModes};
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{HarmonicWhirlSpeedModes, RotatingOptions, RotatingSector, SplitSchur, Stability};
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

fn options(mb: usize, coriolis: bool, stiffening: bool, schur: SplitSchur) -> RotatingOptions {
    let mut o = ModalOptions::new(mb).tolerance(1e-9);
    o.max_iterations = 400;
    RotatingOptions::new(mb)
        .modal(o)
        .coriolis(coriolis)
        .stress_stiffening(stiffening)
        .split_correction(true)
        .split_schur(schur)
        .corrected_frequency(true)
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
    w.extend(opt_bits(&s.schur_eta));
    w.extend(opt_bits(&s.corrected_frequency));
    w
}

fn digests() -> [u64; 4] {
    let model = ds();
    let w_ref = {
        let plain = RotatingSector::new(
            &model,
            &options(4, true, true, SplitSchur::Dense).split_correction(false),
        )
        .unwrap();
        (0..=N / 2)
            .map(|k| match plain.whirl_modes(k, 0.0).unwrap() {
                HarmonicWhirlSpeedModes::Modes { modes, .. } => modes.modes[0].omega,
                HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
            })
            .fold(f64::INFINITY, f64::min)
    };
    let run = |mb: usize, stiffening: bool, schur: SplitSchur, speeds: &[f64]| -> u64 {
        let rs = RotatingSector::new(&model, &options(mb, true, stiffening, schur)).unwrap();
        let mut w = Vec::new();
        for &f in speeds {
            for k in 0..=N / 2 {
                w.extend(words_of(&rs.split_modes(k, f * w_ref).unwrap()));
            }
        }
        fnv(w)
    };
    [
        run(4, true, SplitSchur::Perturbation, &[0.4, 0.8]),
        run(4, true, SplitSchur::Dense, &[0.4, 0.8]),
        run(41, true, SplitSchur::Perturbation, &[0.4, 0.8]),
        run(4, false, SplitSchur::Perturbation, &[10.0]),
    ]
}

/// Prints the digests (run at 679081a1 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of J7 (ADR 0535); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at 679081a1 (ADR 0535 J7).
const BASE: [u64; 4] = [
    0xa744_5684_600d_2134,
    0x05ca_a84e_4015_37ac,
    0xdf0f_2fc5_16e3_6a2a,
    0x9806_c703_feee_f1da,
];

#[test]
fn j7_perturbation_and_dense_paths_unchanged() {
    let got = digests();
    let names = [
        "DS B4 Perturbation split_modes with schur_eta and corrected_frequency, k = 0 .. 3, 0.4 and 0.8 omega_ref",
        "DS B4 Dense split_modes with schur_eta and corrected_frequency, the same",
        "DS B41 Perturbation split_modes, 0.4 and 0.8 omega_ref",
        "DSS Perturbation split_modes at 10 omega_ref",
    ];
    let mut bad = Vec::new();
    for ((n, g), b) in names.iter().zip(got).zip(BASE) {
        println!(
            "J7 {n}: 0x{g:016x} (base 0x{b:016x}) {}",
            if g == b { "equal" } else { "DIFFERS" }
        );
        if g != b {
            bad.push(*n);
        }
    }
    assert!(bad.is_empty(), "J7 differs: {bad:?}");
}
