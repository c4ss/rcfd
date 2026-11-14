//! F-544 (ADR 0462): the bit record of the default paths that F-544 touches (check B1 of the registration).
//!
//! This file uses only the API of main ea76abd8: it was run once in the worktree at that commit (no library line changed)
//! before any library code of F-544, and the digests it printed are the constants of `BASE`. After the change it runs
//! unchanged and compares. The models cover the cyclic harmonic modal analysis (indices 0, 1 and N/2), the static harmonic
//! solve (the same indices), the modal analysis and the static solve of a sector on the transformation (a centrifugal load
//! and a temperature field with an expansion), the rotating sector (the unit pre-stress, the modes at speed of indices 0, 1
//! and N/2, a caller's pre-stress) and the dense whirl modes of a sector.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::cyclic::{modal_analysis_harmonic, solve_static_harmonic, HarmonicLoad, HarmonicModes};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::modal::{modal_analysis_with, ModalOptions};
use rcfd_fem::rotating::{HarmonicSpeedModes, HarmonicWhirlSpeedModes, RotatingOptions, RotatingSector};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

/// FNV-1a 64 over the count and the `to_bits()` of each value, little-endian bytes (the digest of ADR 0291).
fn digest<I: IntoIterator<Item = f64>>(values: I) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    let mut n = 0u64;
    let mut buf = Vec::new();
    for v in values {
        buf.push(v.to_bits());
        n += 1;
    }
    eat(n);
    for b in buf {
        eat(b);
    }
    h
}

const AXIS: Vec3 = [0.0, 0.0, 1.0];

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn opts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

/// A Hex20 sector of the annulus `r in [0.25, 1]`, `z in [-0.1, 0.1]` of `n` sectors with `div` divisions in `(r, phi, z)`,
/// `xmin` clamped, one cyclic constraint about z through the origin.
fn sector(n: usize, div: [usize; 3]) -> SolidModel {
    let alpha = 2.0 * PI / n as f64;
    let b = hex_box([0.25, 0.0, -0.1], [0.75, alpha, 0.2], div).unwrap();
    let mesh: SolidMesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], n))
        .unwrap();
    m
}

fn harmonic_digests(tag: &str, h: &HarmonicModes, out: &mut Vec<(String, u64)>) {
    out.push((format!("{tag} eigenvalues"), digest(h.eigenvalues.iter().copied())));
    out.push((
        format!("{tag} modes"),
        digest(h.modes.iter().flat_map(|m| m.cos.iter().chain(&m.sin).copied())),
    ));
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let s = sector(12, [3, 2, 1]);
    // The cyclic harmonic modal analysis.
    for k in [0, 1, 6] {
        let h = modal_analysis_harmonic(&s, &[], k, &opts(3)).unwrap();
        harmonic_digests(&format!("sector harmonic {k}"), &h, &mut out);
    }
    // The static harmonic solve: a pressure on the outer face (cosine part) and a traction on zmax (sine part).
    let mut mc = s.clone();
    mc.add_load(Load::pressure("xmax", 1.0e6)).unwrap();
    let fa = nodal_load_vector(&mc, &[]).unwrap();
    let mut ms = s.clone();
    ms.add_load(Load::traction("zmax", [2.0e5, -1.0e5, 5.0e5])).unwrap();
    let fb = nodal_load_vector(&ms, &[]).unwrap();
    for k in [0, 1, 6] {
        let sin = if k == 1 { fb.clone() } else { vec![0.0; fb.len()] };
        let r = solve_static_harmonic(
            &s,
            &[],
            &HarmonicLoad::new(k, fa.clone(), sin),
            &StaticOptions::default(),
        )
        .unwrap();
        out.push((
            format!("sector static harmonic {k}"),
            digest(r.cos.iter().chain(&r.sin).copied()),
        ));
    }
    // The modal analysis and the static solve of the sector on the transformation.
    let mr = modal_analysis_with(&s, &[], &opts(3)).unwrap();
    out.push((
        "sector modal_analysis_with eigenvalues".into(),
        digest(mr.eigenvalues.iter().copied()),
    ));
    let mut mt = s.clone();
    mt.add_load(Load::centrifugal(AXIS, [0.0; 3], 300.0)).unwrap();
    let t: Vec<f64> = mt
        .mesh()
        .nodes()
        .iter()
        .map(|x| 293.0 + 200.0 * (x[0] * x[0] + x[1] * x[1]).sqrt() + 50.0 * x[2])
        .collect();
    mt.add_load(Load::temperature(t, 293.0)).unwrap();
    let st = solve_static(&mt, &StaticOptions::default()).unwrap();
    out.push((
        "sector solve_static centrifugal and thermal".into(),
        digest(st.displacement_values().iter().copied()),
    ));
    // The rotating sector.
    let rs = RotatingSector::new(&s, &RotatingOptions::new(3).modal(opts(3))).unwrap();
    out.push((
        "rotating sector unit prestress".into(),
        digest(rs.part().unit_prestress().stress().iter().flatten().copied()),
    ));
    for k in [0, 1, 6] {
        match rs.modes(k, 300.0).unwrap() {
            HarmonicSpeedModes::Modes(h) => harmonic_digests(&format!("rotating sector {k} 300"), &h, &mut out),
            HarmonicSpeedModes::LossOfStiffness(l) => panic!("sector {k}: unexpected loss of stiffness {l:?}"),
        }
    }
    let pre = Prestress::uniform(&s, [1.0e3, 2.0e3, 0.0, 0.0, 0.0, 0.0]).unwrap();
    let rp = RotatingSector::with_unit_prestress(&s, pre, &RotatingOptions::new(3).modal(opts(3))).unwrap();
    match rp.modes(1, 300.0).unwrap() {
        HarmonicSpeedModes::Modes(h) => harmonic_digests("rotating sector caller prestress 1 300", &h, &mut out),
        HarmonicSpeedModes::LossOfStiffness(l) => panic!("caller prestress: unexpected loss of stiffness {l:?}"),
    }
    // The dense whirl modes of a sector (6 sectors, Hex20 2 x 1 x 1).
    let w = sector(6, [2, 1, 1]);
    let ws = RotatingSector::new(&w, &RotatingOptions::new(3).modal(opts(3)).coriolis(true)).unwrap();
    for k in [0, 1, 3] {
        match ws.whirl_modes(k, 300.0).unwrap() {
            HarmonicWhirlSpeedModes::Modes { modes, .. } => out.push((
                format!("sector whirl {k} 300"),
                digest(
                    modes
                        .modes
                        .iter()
                        .flat_map(|x| [x.omega, x.whirl])
                        .chain(modes.frequencies_all.iter().copied()),
                ),
            )),
            HarmonicWhirlSpeedModes::LossOfStiffness(l) => panic!("whirl {k}: unexpected loss of stiffness {l:?}"),
        }
    }
    out
}

/// The digests printed by the run at main ea76abd8 (`None` before that run).
const BASE: Option<&[(&str, u64)]> = Some(&[
    ("sector harmonic 0 eigenvalues", 0x166fd4ba2c9e5f40),
    ("sector harmonic 0 modes", 0x298756d82d899383),
    ("sector harmonic 1 eigenvalues", 0xb24c50505a72f23f),
    ("sector harmonic 1 modes", 0xeb97143be9aa12e5),
    ("sector harmonic 6 eigenvalues", 0xd64685051ec19919),
    ("sector harmonic 6 modes", 0x66fe2d5344396177),
    ("sector static harmonic 0", 0x3a4a8b4d2c38ae0d),
    ("sector static harmonic 1", 0xe2e5d179a2730e8d),
    ("sector static harmonic 6", 0xe3df1f24d50a7658),
    ("sector modal_analysis_with eigenvalues", 0x166fd4ba2c9e5f40),
    ("sector solve_static centrifugal and thermal", 0xb3896741105d04e5),
    ("rotating sector unit prestress", 0x39ff8a3300202409),
    ("rotating sector 0 300 eigenvalues", 0x3d976a7773cc1e0b),
    ("rotating sector 0 300 modes", 0x0e2d56e26a7765ac),
    ("rotating sector 1 300 eigenvalues", 0xa8ce6968d9749451),
    ("rotating sector 1 300 modes", 0xbcf2a7de04d91d23),
    ("rotating sector 6 300 eigenvalues", 0xefbfc174481a1692),
    ("rotating sector 6 300 modes", 0xb2b4c92c7373bc3d),
    ("rotating sector caller prestress 1 300 eigenvalues", 0x1ba66e9864d0480f),
    ("rotating sector caller prestress 1 300 modes", 0xde1142b80e83d2af),
    ("sector whirl 0 300", 0x8fcdb420e136208b),
    ("sector whirl 1 300", 0xac449ac1b5421297),
    ("sector whirl 3 300", 0xdabea8e352966790),
]);

#[test]
fn b1_default_paths_bits() {
    let got = all_digests();
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    let base = BASE.expect("the record of main ea76abd8 is filled in");
    let mut failed = Vec::new();
    assert_eq!(got.len(), base.len(), "number of digests");
    for ((name, d), (bname, bd)) in got.iter().zip(base) {
        let ok = name == bname && d == bd;
        println!("B1 {name}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            failed.push(name.clone());
        }
    }
    assert!(failed.is_empty(), "missed: {failed:?}");
}
