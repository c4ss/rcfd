//! F-512 (ADR 0423): the bytes of the solid output for models without beams. Procedure B0 records the FNV-1a 64 hash and the
//! length of `solid_vtu_string` for the six no-beam fixtures of ADR 0278 at the base commit; check B1 compares against them.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::mesh::{ElementKind, SolidMesh};
use rcfd_fem::solve::{solve_static_with, StaticOptions};
use rcfd_fem::{Material, SolidModel, SolidResult};
use rcfd_post::solid_output::solid_vtu_string;
use std::collections::BTreeSet;

// The fixtures of sb_solid_output.rs (ADR 0278), copied.
const KINDS: [ElementKind; 3] = [ElementKind::Hex8, ElementKind::Tet4, ElementKind::Tet10];
const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const ORIGIN: Vec3 = [0.1, -0.2, 0.3];
const LENGTHS: Vec3 = [1.2, 1.0, 0.8];
const DIV: [usize; 3] = [2, 2, 2];
const EPS: [[f64; 3]; 3] = [
    [1.0e-3, 0.3e-3, 0.4e-3],
    [0.3e-3, -0.5e-3, -0.2e-3],
    [0.4e-3, -0.2e-3, 0.8e-3],
];

fn mesh_of(kind: ElementKind) -> SolidMesh {
    match kind {
        ElementKind::Hex8 => hex_box(ORIGIN, LENGTHS, DIV).unwrap(),
        ElementKind::Tet4 => tet_box(ORIGIN, LENGTHS, DIV).unwrap(),
        _ => upgrade_to_tet10(&tet_box(ORIGIN, LENGTHS, DIV).unwrap()).unwrap().0,
    }
}

fn case(kind: ElementKind) -> (SolidModel, SolidResult) {
    let mesh = mesh_of(kind);
    let mut b = BTreeSet::new();
    for f in BOX_FACES {
        b.extend(mesh.node_set(f).unwrap().iter().copied());
    }
    let mut pres = Vec::new();
    for &n in &b {
        let x = mesh.nodes()[n];
        for (i, row) in EPS.iter().enumerate() {
            let u = row[0] * x[0] + row[1] * x[1] + row[2] * x[2];
            pres.push(Prescribed::new(n, i, u));
        }
    }
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E_MOD, NU, 7850.0, 0.0).unwrap())
        .unwrap();
    let res = solve_static_with(&model, &pres, &[], &StaticOptions::default()).unwrap();
    (model, res)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// (fixture name, hash, length) of the six fixtures: each kind with reactions, then without.
fn fixtures() -> Vec<(String, u64, usize)> {
    let mut out = Vec::new();
    for kind in KINDS {
        let (model, res) = case(kind);
        let plain = SolidResult::new(res.dofs().clone(), res.displacement_values().to_vec()).unwrap();
        for (tag, r) in [("reactions", &res), ("no reactions", &plain)] {
            let s = solid_vtu_string(&model, r).unwrap();
            out.push((format!("{} {tag}", kind.name()), fnv1a64(s.as_bytes()), s.len()));
        }
    }
    out
}

#[test]
fn b0_no_beam_hashes_at_the_base() {
    let f = fixtures();
    assert_eq!(f.len(), 6);
    for (name, h, len) in &f {
        assert!(*len > 0);
        println!("B0: {name}: hash 0x{h:016x}, length {len}");
    }
}

/// B1: the values recorded by B0 at the base commit 3cf27f63 (ADR 0423 addendum 0).
#[test]
fn b1_no_beam_bytes_unchanged() {
    let recorded: [(&str, u64, usize); 6] = [
        ("Hex8 reactions", 0x63fe_d7da_a56f_1a63, 15571),
        ("Hex8 no reactions", 0x989f_7368_6c43_913e, 13987),
        ("Tet4 reactions", 0xe741_f54c_e312_ad4d, 20588),
        ("Tet4 no reactions", 0xf0a0_6ff6_536f_969c, 19006),
        ("Tet10 reactions", 0x65e3_8c3b_684c_f06c, 68654),
        ("Tet10 no reactions", 0x526b_f4d7_4d05_98c9, 62488),
    ];
    let now = fixtures();
    let mut missed = Vec::new();
    for ((name, h, len), (rn, rh, rl)) in now.iter().zip(recorded) {
        assert_eq!(name, rn, "fixture order");
        let ok = *h == rh && *len == rl;
        println!(
            "B1: {name}: hash 0x{h:016x} length {len} against 0x{rh:016x} {rl}: {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            missed.push(name.clone());
        }
    }
    assert!(missed.is_empty(), "B1 missed: {missed:?}");
}
