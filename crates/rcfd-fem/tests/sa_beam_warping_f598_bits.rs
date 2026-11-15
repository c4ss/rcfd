//! F-598 (ADR 0546): the bit record of the default paths next to the warping option (check B0 of the registration, D9).
//!
//! This file uses only the API of the base f69eb128 (plus the ADR commit): it was run once with the registration committed and
//! no library line changed, and the digests it printed are the constants of `BASE`. After the change it runs unchanged and
//! compares. The models are the I-section and the rectangle Beam2 beams of ADR 0508 under uniform moment (fork supports, 16
//! elements); the element matrices of an I-section element along a skew axis, the static displacement, the assembled geometric
//! stiffness with the resultant terms, the buckling factors and modes of the F-558 path.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::geometric::{geometric_stiffness, Prestress};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{beam, BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

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

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

/// The I section of ADR 0508 I1.
fn i_section() -> BeamSection {
    let (bf, tf, hw, tw): (f64, f64, f64, f64) = (0.1, 0.01, 0.18, 0.006);
    let a = 2.0 * bf * tf + hw * tw;
    let izz = 2.0 * tf * bf.powi(3) / 12.0 + hw * tw.powi(3) / 12.0;
    let hc = hw + tf;
    let iyy = 2.0 * (bf * tf.powi(3) / 12.0 + bf * tf * (hc / 2.0).powi(2)) + tw * hw.powi(3) / 12.0;
    let j = (2.0 * bf * tf.powi(3) + hw * tw.powi(3)) / 3.0;
    BeamSection::new(a, iyy, izz, j, 5.0 / 6.0, 5.0 / 6.0).unwrap()
}

fn rect_section() -> BeamSection {
    let (b, h): (f64, f64) = (0.01, 0.2);
    let j = (h * b * b * b / 3.0) * (1.0 - 0.63 * (b / h) * (1.0 - b.powi(4) / (12.0 * h.powi(4))));
    beam::rectangle_section(b, h, j, 5.0 / 6.0).unwrap()
}

/// A beam along `x`, `n` elements, length 3, fork supports, end moments `m_ref` about `y`.
fn ltb_model(section: &BeamSection, n: usize, m_ref: f64) -> (SolidModel, SolidModel) {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [3.0 * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_moment("x0", [0.0, -m_ref, 0.0])).unwrap();
    loaded.add_load(Load::nodal_moment("xl", [0.0, m_ref, 0.0])).unwrap();
    (m, loaded)
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let s = i_section();
    let mat = steel();
    // Element matrices of an I-section element along a skew axis.
    let coords = [
        [0.1, -0.2, 0.3],
        [0.1 + 0.7 / 2f64.sqrt(), -0.2 + 0.7 / 2f64.sqrt(), 0.3],
    ];
    let orient = [-1.0, 1.0, 0.4];
    out.push((
        "local stiffness".to_string(),
        digest(beam::local_stiffness(0.7, &s, &mat).values().to_vec()),
    ));
    out.push((
        "element stiffness".to_string(),
        digest(
            beam::element_stiffness(&coords, &s, orient, &mat)
                .unwrap()
                .values()
                .to_vec(),
        ),
    ));
    let stress = [-3.0e6, 1.0e5, -2.0e5, 4.0e5, -5.0e4, 7.0e4];
    out.push((
        "element axial geometric stiffness".to_string(),
        digest(
            beam::element_geometric_stiffness(&coords, &s, orient, &mat, &stress)
                .unwrap()
                .values()
                .to_vec(),
        ),
    ));
    let f = [
        -50.0, -2.0, 3.0, -1.5, 1000.0, -400.0, 50.0, 2.0, -3.0, 1.5, -997.9, 401.4,
    ];
    out.push((
        "element resultant geometric stiffness".to_string(),
        digest(
            beam::element_resultant_geometric_stiffness(&coords, &s, orient, &mat, &f, [0.7, -0.4])
                .unwrap()
                .values()
                .to_vec(),
        ),
    ));
    for (name, sec) in [("I", s), ("rect", rect_section())] {
        let (m, loaded) = ltb_model(&sec, 16, 1000.0);
        let r = solve_static(&loaded, &StaticOptions::default()).unwrap();
        out.push((
            format!("{name} static displacement"),
            digest(r.displacement_values().to_vec()),
        ));
        let pre = Prestress::from_static(&loaded, &r)
            .unwrap()
            .with_beam_resultants(&loaded, r.displacement_values(), [0.0; 2])
            .unwrap();
        let kg = geometric_stiffness(&m, &pre, Some(1)).unwrap();
        out.push((format!("{name} geometric stiffness"), digest(kg.val.clone())));
        let b = factors(&m, &pre, &BucklingOptions::new(3)).unwrap();
        out.push((format!("{name} buckling factors"), digest(b.factors.clone())));
        out.push((
            format!("{name} buckling modes"),
            digest(b.modes.iter().flatten().copied()),
        ));
        println!("{name} factors {:?}", b.factors);
    }
    out
}

/// Printed by the run on the base f69eb128 plus the ADR commit (no library line changed).
const BASE: &[(&str, u64)] = &[
    ("local stiffness", 0xcd23c4f2b35e8231),
    ("element stiffness", 0x988a5e2d2aadbda1),
    ("element axial geometric stiffness", 0x20abdd8277dd7e2f),
    ("element resultant geometric stiffness", 0xedb0abcd46200663),
    ("I static displacement", 0x79be4b7683dc962e),
    ("I geometric stiffness", 0x88da12b3d85aa3e3),
    ("I buckling factors", 0xadcb46fa33386773),
    ("I buckling modes", 0x7fd63bc4161df4fe),
    ("rect static displacement", 0x06a73c097adb4e9c),
    ("rect geometric stiffness", 0x887d1d9f0ab7e293),
    ("rect buckling factors", 0x8f5333b75077860e),
    ("rect buckling modes", 0x7624c634feb5d52a),
];

#[test]
fn b0_default_paths_keep_their_bits() {
    let got = all_digests();
    for (name, d) in &got {
        println!("    (\"{name}\", 0x{d:016x}),");
    }
    assert_eq!(got.len(), BASE.len(), "digest count");
    let mut missed = Vec::new();
    for ((name, d), (bname, bd)) in got.iter().zip(BASE) {
        assert_eq!(name, bname, "digest order");
        if d != bd {
            missed.push(name.clone());
        }
    }
    assert!(missed.is_empty(), "changed bits: {missed:?}");
}
