//! F-556 (ADR 0483): check B1, the bit record of the default Shell3, the ANDES Shell3, the Shell4 and the Hex20 twisted strip
//! of ADR 0473 (stiffness, mass, rest frequencies) and of one twisted Shell3 element for each membrane. Uses only the API of
//! 44b6c812; the expected digests were printed by this file run on that base with ADR 0483 committed and no other line changed
//! (`f556-scratch/b1-base.log`) and written into `EXPECTED` before any other code of F-556.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20_mapped;
use rcfd_fem::mass::consistent;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::shell3::{self, Shell3Membrane, Shell3Options};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;
use std::f64::consts::PI;

/// Digests of 44b6c812 in the order of the run, from the base run (`f556-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 12]> = Some([
    ("Shell3 strip stiffness", 0xd80a_f026_ecce_c7b8),
    ("Shell3 strip mass", 0x554f_f02f_be6a_ae3f),
    ("Shell3 strip frequencies", 0x1734_d0db_ccaa_9a20),
    ("ANDES Shell3 strip stiffness", 0x4eb9_dc9f_8f08_3dd8),
    ("ANDES Shell3 strip frequencies", 0x6910_3c8a_558b_a30a),
    ("Shell4 strip stiffness", 0x495c_2a04_cb6f_7d1f),
    ("Shell4 strip frequencies", 0xd33e_a9eb_34fd_96f3),
    ("Hex20 strip stiffness", 0x0cbe_762f_ad47_fee5),
    ("element stiffness", 0x4384_3dd0_409f_6dec),
    ("element stiffness CST", 0x4384_3dd0_409f_6dec),
    ("element stiffness Allman", 0xc634_37a9_8375_b44e),
    ("element stiffness ANDES", 0x692f_7b8f_4d3d_9851),
]);

const CHORD: f64 = 0.05;
const THICK: f64 = 0.001;
const SPAN: f64 = 0.15;
const NCH: usize = 12;
const NSP: usize = 36;

/// FNV-1a 64 over the count and the `to_bits()` of each value, little-endian bytes (the digest of ADR 0291).
fn digest(values: &[f64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |x: u64| {
        for b in x.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(values.len() as u64);
    for v in values {
        eat(v.to_bits());
    }
    h
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn surface(r: f64, s: f64) -> (Vec3, Vec3) {
    let th = (PI / 6.0) * r / SPAN;
    let dth = (PI / 6.0) / SPAN;
    let x = [r, s * th.cos(), s * th.sin()];
    let n = [-s * dth, -th.sin(), th.cos()];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    (x, [n[0] / l, n[1] / l, n[2] / l])
}

fn strip_shell(kind: ElementKind, property: BlockProperty) -> SolidModel {
    let idx = |i: usize, j: usize| j * (NCH + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=NSP {
        for i in 0..=NCH {
            let s = -0.5 * CHORD + CHORD * i as f64 / NCH as f64;
            nodes.push(surface(SPAN * j as f64 / NSP as f64, s).0);
        }
    }
    let tri = kind == ElementKind::Shell3;
    let mut conn = Vec::new();
    for j in 0..NSP {
        for i in 0..NCH {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    let ne = if tri { 2 * NCH * NSP } else { NCH * NSP };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("root", (0..=NCH).map(|i| idx(i, 0)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    m
}

fn strip_solid() -> SolidModel {
    let base = hex_box([-0.5 * CHORD, 0.0, -0.5 * THICK], [CHORD, SPAN, THICK], [NCH, NSP, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&base, |p| {
        let (x, n) = surface(p[1], p[0]);
        let z = -p[2];
        [x[0] + z * n[0], x[1] + z * n[1], x[2] + z * n[2]]
    })
    .unwrap()
    .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("ymin", c, 0.0)).unwrap();
    }
    m
}

fn freqs(m: &SolidModel) -> Vec<f64> {
    let o = ModalOptions::new(6)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-10);
    modal_analysis(m, &o).unwrap().frequencies
}

#[test]
fn b1_twisted_strip_defaults_keep_their_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let s3 = strip_shell(ElementKind::Shell3, BlockProperty::shell(THICK).unwrap());
    got.push(("Shell3 strip stiffness", digest(&stiffness(&s3).unwrap().val)));
    got.push(("Shell3 strip mass", digest(&consistent(&s3).unwrap().val)));
    got.push(("Shell3 strip frequencies", digest(&freqs(&s3))));
    let a3 = strip_shell(
        ElementKind::Shell3,
        BlockProperty::shell_with_membrane(THICK, Shell3Membrane::AndesOpt).unwrap(),
    );
    got.push(("ANDES Shell3 strip stiffness", digest(&stiffness(&a3).unwrap().val)));
    got.push(("ANDES Shell3 strip frequencies", digest(&freqs(&a3))));
    let s4 = strip_shell(ElementKind::Shell4, BlockProperty::shell(THICK).unwrap());
    got.push(("Shell4 strip stiffness", digest(&stiffness(&s4).unwrap().val)));
    got.push(("Shell4 strip frequencies", digest(&freqs(&s4))));
    got.push(("Hex20 strip stiffness", digest(&stiffness(&strip_solid()).unwrap().val)));
    let x: [Vec3; 3] = [surface(0.1, -0.01).0, surface(0.1, 0.004).0, surface(0.11, 0.004).0];
    let mat = steel();
    got.push((
        "element stiffness",
        digest(shell3::element_stiffness(&x, THICK, &mat).unwrap().values()),
    ));
    for (name, mem) in [
        ("element stiffness CST", Shell3Membrane::ConstantStrain),
        ("element stiffness Allman", Shell3Membrane::Allman),
        ("element stiffness ANDES", Shell3Membrane::AndesOpt),
    ] {
        let o = Shell3Options::default().membrane(mem);
        got.push((
            name,
            digest(shell3::element_stiffness_with(&x, THICK, &mat, &o).unwrap().values()),
        ));
    }
    for (name, d) in &got {
        println!("B1 {name}: {d:#018x}");
    }
    let expected = EXPECTED.expect("EXPECTED is written from the base run");
    assert_eq!(got.len(), expected.len());
    for ((name, d), (en, e)) in got.iter().zip(expected.iter()) {
        assert_eq!(name, en);
        assert_eq!(d, e, "B1 {name}: {d:#018x} against {e:#018x}");
    }
}
