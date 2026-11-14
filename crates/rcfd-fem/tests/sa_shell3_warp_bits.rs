//! F-574 (ADR 0537): check B1, the default Shell3 paths and the messages of the shell model checks keep their bits. The
//! constant-strain Shell3 strip of ADR 0473 at twist 0, 15 and 30 degrees (stiffness), the rest frequencies of the 30 degree
//! strip, and the text of the messages of `check_shell_model` for three refused models and its result for the warped strip.
//! Uses only the API of c1122807; the expected digests were printed by this file run on that base with ADR 0537 committed and no
//! other line changed (`f574-scratch/b1-base.log`) and written into `EXPECTED` before any library code of F-574.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::shell::check_shell_model;
use rcfd_fem::shell3::Shell3Membrane;
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

/// Digests of c1122807 in the order of the run, from the base run (`f574-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 8]> = Some([
    ("Shell3 strip stiffness twist 0", 0xb501_2bcd_4300_573c),
    ("Shell3 strip stiffness twist 15", 0xf9e3_f898_2021_dffb),
    ("Shell3 strip stiffness twist 30", 0xd80a_f026_ecce_c7b8),
    ("Shell3 strip frequencies twist 30", 0x1734_d0db_ccaa_9a20),
    ("check_shell_model warped strip", 0x08b0_5d07_b556_6bef),
    ("message no property", 0x5884_fcfc_0290_15e2),
    ("message degenerate facet", 0xf6f5_b9a7_48ba_a530),
    ("message mixed membranes", 0x6072_d431_bbbc_790b),
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

/// The same FNV-1a 64 over the bytes of a text.
fn digest_text(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn strip_shell3(twist_deg: f64, property: BlockProperty) -> SolidModel {
    let idx = |i: usize, j: usize| j * (NCH + 1) + i;
    let mut nodes: Vec<Vec3> = Vec::new();
    for j in 0..=NSP {
        for i in 0..=NCH {
            let r = SPAN * j as f64 / NSP as f64;
            let s = -0.5 * CHORD + CHORD * i as f64 / NCH as f64;
            let th = twist_deg.to_radians() * r / SPAN;
            nodes.push([r, s * th.cos(), s * th.sin()]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..NSP {
        for i in 0..NCH {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            conn.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell3, conn).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..2 * NCH * NSP).collect()).unwrap();
    mesh.add_node_set("root", (0..=NCH).map(|i| idx(i, 0)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    m
}

fn message(r: rcfd_fem::Result<()>) -> String {
    match r {
        Ok(()) => "ok".to_string(),
        Err(e) => format!("{e}"),
    }
}

#[test]
fn b1_shell3_defaults_and_messages_keep_their_bits() {
    let prop = || BlockProperty::shell(THICK).unwrap();
    let mut got: Vec<(&str, u64)> = Vec::new();
    for (name, tw) in [
        ("Shell3 strip stiffness twist 0", 0.0),
        ("Shell3 strip stiffness twist 15", 15.0),
        ("Shell3 strip stiffness twist 30", 30.0),
    ] {
        got.push((name, digest(&stiffness(&strip_shell3(tw, prop())).unwrap().val)));
    }
    let o = ModalOptions::new(6)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-10);
    let f = modal_analysis(&strip_shell3(30.0, prop()), &o).unwrap().frequencies;
    got.push(("Shell3 strip frequencies twist 30", digest(&f)));
    got.push((
        "check_shell_model warped strip",
        digest_text(&message(check_shell_model(&strip_shell3(30.0, prop())))),
    ));
    // A Shell3 block without a Shell property.
    let tri = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let mut m1 = SolidMesh::new(tri.clone()).unwrap();
    m1.add_block(ElementKind::Shell3, vec![0, 1, 2]).unwrap();
    got.push((
        "message no property",
        digest_text(&message(check_shell_model(&SolidModel::new(m1)))),
    ));
    // A degenerate facet.
    let mut m2 = SolidMesh::new(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]]).unwrap();
    m2.add_block(ElementKind::Shell3, vec![0, 1, 2]).unwrap();
    m2.set_block_property(0, prop()).unwrap();
    got.push((
        "message degenerate facet",
        digest_text(&message(check_shell_model(&SolidModel::new(m2)))),
    ));
    // A constant-strain and an ANDES element sharing an edge.
    let mut m3 = SolidMesh::new(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]).unwrap();
    m3.add_block(ElementKind::Shell3, vec![0, 1, 2]).unwrap();
    m3.add_block(ElementKind::Shell3, vec![0, 2, 3]).unwrap();
    m3.set_block_property(0, prop()).unwrap();
    m3.set_block_property(
        1,
        BlockProperty::shell_with_membrane(THICK, Shell3Membrane::AndesOpt).unwrap(),
    )
    .unwrap();
    got.push((
        "message mixed membranes",
        digest_text(&message(check_shell_model(&SolidModel::new(m3)))),
    ));
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
