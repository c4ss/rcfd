//! F-555 (ADR 0481) B2: the bits of the non-cyclic split Campbell table of F-415 and of the default sector paths (the F-396
//! sector Campbell table, the F-499 sector whirl modes, the F-396 sector modes at speed) on the set-ups of ADR 0481, recorded on
//! main 4c422025 before any library line of F-555 was written. Uses only the API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::rotating::{HarmonicSpeedModes, HarmonicWhirlSpeedModes, RotatingOptions, RotatingPart, RotatingSector};
use rcfd_fem::{Constraint, ElasticSupport, Material, SolidModel};
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

const AXIS: Vec3 = [0.0, 0.0, 1.0];
const ORIGIN: Vec3 = [0.0; 3];
const RHO: f64 = 7850.0;
const W0: f64 = 1000.0;
const H: f64 = 0.1;
const N: usize = 6;

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

/// DS of ADR 0411.
fn ds_sector() -> SolidModel {
    let a = 2.0 * PI / N as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, a, 0.2], [2, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, ORIGIN, N))
        .unwrap();
    m
}

fn ds_options(coriolis: bool) -> RotatingOptions {
    let mut o = ModalOptions::new(4).tolerance(1e-9);
    o.max_iterations = 400;
    RotatingOptions::new(4).modal(o).coriolis(coriolis)
}

fn digests() -> [u64; 13] {
    let cb = cube(1.0);
    let speeds: Vec<f64> = (0..=12).map(|j| 0.07 * j as f64 * W0).collect();
    let on = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(3).coriolis(true)).unwrap();
    let sc = on.split_campbell(&speeds).unwrap();
    let mut ev = Vec::new();
    let mut wk = Vec::new();
    let mut gr = Vec::new();
    for ((t, k), g) in sc.campbell.table.tracks.iter().zip(&sc.campbell.whirl).zip(&sc.growth) {
        ev.extend(opt_bits(&t.eigenvalues));
        wk.extend(opt_bits(k));
        gr.extend(opt_bits(g));
    }
    let off = RotatingPart::new(&cb, AXIS, ORIGIN, &RotatingOptions::new(3)).unwrap();
    let so = off.split_campbell(&speeds).unwrap();
    let mut ev_off = Vec::new();
    for t in &so.campbell.table.tracks {
        ev_off.extend(opt_bits(&t.eigenvalues));
    }
    let cba = cube(2.25);
    let pa = RotatingPart::new(&cba, AXIS, ORIGIN, &RotatingOptions::new(3).coriolis(true)).unwrap();
    let un = pa
        .split_campbell(&[0.0, 0.5 * W0, 1.2 * W0, 1.8 * W0])
        .unwrap()
        .unstable();
    let sm = on.split_modes(0.5 * W0).unwrap();

    let ds = ds_sector();
    let rs = RotatingSector::new(&ds, &ds_options(true)).unwrap();
    let w_ref = (0..=N / 2)
        .map(|k| match rs.whirl_modes(k, 0.0).unwrap() {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => modes.modes[0].omega,
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
        })
        .fold(f64::INFINITY, f64::min);
    let cs: Vec<f64> = [0.0, 0.2, 0.4, 0.6, 0.8].iter().map(|f| f * w_ref).collect();
    let cc = rs.campbell(&[0, 1, 2, 3], &cs).unwrap();
    let mut cev = Vec::new();
    let mut cmac = Vec::new();
    for fam in &cc.families {
        for t in &fam.table.tracks {
            cev.extend(opt_bits(&t.eigenvalues));
            cmac.extend(opt_bits(&t.mac));
        }
    }
    let wm = match rs.whirl_modes(1, 0.8 * w_ref).unwrap() {
        HarmonicWhirlSpeedModes::Modes { modes, .. } => modes,
        HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
    };
    let hm = match rs.modes(2, 0.4 * w_ref).unwrap() {
        HarmonicSpeedModes::Modes(h) => h,
        HarmonicSpeedModes::LossOfStiffness(l) => panic!("{l:?}"),
    };
    [
        fnv(ev),
        fnv(wk),
        fnv(gr),
        fnv(ev_off),
        fnv(un.iter().flat_map(|(s, g)| [s.to_bits(), g.to_bits()])),
        fnv(sm.modes.iter().map(|m| m.frequency.to_bits())),
        fnv(sm.modes[0].re.iter().chain(&sm.modes[0].im).map(|x| x.to_bits())),
        fnv(cev),
        fnv(cmac),
        fnv(wm.frequencies_all.iter().map(|x| x.to_bits())),
        fnv(wm.modes[0].re.iter().map(|x| x.to_bits())),
        fnv(hm.eigenvalues.iter().map(|x| x.to_bits())),
        w_ref.to_bits(),
    ]
}

/// Prints the digests (run at main 4c422025 to record [`BASE`]).
#[test]
#[ignore = "prints the digests of B2 (ADR 0481); run at the base to record BASE"]
fn print_base_digests() {
    for d in digests() {
        println!("0x{d:016x},");
    }
}

/// The digests printed at main 4c422025 (ADR 0481 B2).
const BASE: [u64; 13] = [
    0xfafa_58ef_2bd0_e2b6,
    0x17c8_c6a7_3cf9_8d3b,
    0x3857_9eb6_f550_0283,
    0x0c67_04e7_07f1_6bc4,
    0xa587_b2c6_57a0_700b,
    0x420e_604e_be43_853b,
    0x5c83_fe7c_c7df_4764,
    0xfe2f_b1d8_8871_1e70,
    0xa92f_f619_e52a_dff0,
    0xed86_9f16_d30c_fbf0,
    0x8020_1aed_0a6a_612d,
    0xdeac_298f_4583_02dc,
    0x4099_dd4a_58b0_2ec5,
];

#[test]
fn b2_default_paths_unchanged() {
    let got = digests();
    let names = [
        "CB split_campbell on: eigenvalues",
        "CB split_campbell on: whirl numbers",
        "CB split_campbell on: growth",
        "CB split_campbell off: eigenvalues",
        "CBA split_campbell unstable()",
        "CB split_modes(0.5 w0) frequencies",
        "CB split_modes(0.5 w0) first shape",
        "DS campbell eigenvalues",
        "DS campbell MAC",
        "DS whirl_modes(1, 0.8 omega_ref) frequencies_all",
        "DS whirl_modes(1, 0.8 omega_ref) first mode re",
        "DS modes(2, 0.4 omega_ref) eigenvalues",
        "DS omega_ref",
    ];
    let mut bad = Vec::new();
    for ((n, g), b) in names.iter().zip(got).zip(BASE) {
        println!(
            "B2 {n}: 0x{g:016x} (base 0x{b:016x}) {}",
            if g == b { "equal" } else { "DIFFERS" }
        );
        if g != b {
            bad.push(*n);
        }
    }
    assert!(bad.is_empty(), "B2 differs: {bad:?}");
}
