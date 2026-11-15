//! F-663 (ADR 0588): the bit record B1 of the warping buckling path without a distributed load height (D5).
//!
//! This file uses only the API of the base 83b9d18c: it was run once with the registration committed and no library line
//! changed, and the digests it printed are the constants of `BASE`. After the change it runs unchanged and compares.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{
    buckling_factors, buckling_factors_with, matrices, solve_static, Warping, WarpingPrestress, WarpingSection,
};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::geometric::Prestress;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

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

const L: f64 = 3.0;

fn dsym() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5.0 / 6.0).unwrap()
}

fn mono() -> (BeamSection, WarpingSection) {
    WarpingSection::monosymmetric_i(0.15, 0.012, 0.08, 0.008, 0.2, 0.006, 5.0 / 6.0).unwrap()
}

/// The fork beam of `n` elements along `x` of ADR 0582 (sets `all`, `x0`, `xl`, `mid`).
fn fork(n: usize, section: &BeamSection) -> SolidModel {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [L * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

fn six_prestress(loaded: &SolidModel) -> Prestress {
    let r = rcfd_fem::solve::solve_static(loaded, &StaticOptions::default()).unwrap();
    Prestress::from_static(loaded, &r)
        .unwrap()
        .with_beam_resultants(loaded, r.displacement_values(), [0.0; 2])
        .unwrap()
}

fn with_load(m: &SolidModel, load: Load) -> SolidModel {
    let mut l = m.clone();
    l.add_load(load).unwrap();
    l
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let (ds, dws) = dsym();
    let (ms, mws) = mono();
    let point = [0.0, 0.0, -1000.0];
    let down = [0.0, 0.0, -1000.0];
    let up = [0.0, 0.0, 1000.0];
    let opts = BucklingOptions::new(3);
    // (1), (2): the ADR 0582 DS-top point load at 16 and 8 elements.
    for n in [16, 8] {
        let m = fork(n, &ds);
        let pre = six_prestress(&with_load(&m, Load::nodal_force("mid", point)));
        let w = Warping::new().block(0, dws).load_point("mid", point, [0.0, 0.095]);
        let r = buckling_factors(&m, &pre, &w, &opts).unwrap();
        out.push((format!("point DS-top {n} factors"), digest(r.factors.iter().copied())));
    }
    // (3) to (5): the UDL without an entry.
    for (name, s, ws, q) in [
        ("UDL DS down", &ds, dws, down),
        ("UDL MS down", &ms, mws, down),
        ("UDL MS up", &ms, mws, up),
    ] {
        let m = fork(16, s);
        let pre = six_prestress(&with_load(&m, Load::line_load("all", q)));
        let w = Warping::new().block(0, ws);
        let r = buckling_factors(&m, &pre, &w, &opts).unwrap();
        out.push((format!("{name} factors"), digest(r.factors.iter().copied())));
    }
    // (6): K and K_sigma of the DS-top point-load model.
    let m = fork(16, &ds);
    let pre = six_prestress(&with_load(&m, Load::nodal_force("mid", point)));
    let w = Warping::new().block(0, dws).load_point("mid", point, [0.0, 0.095]);
    let mats = matrices(&m, &pre, &w, None).unwrap();
    out.push(("point DS-top K".to_string(), digest(mats.stiffness.val.iter().copied())));
    out.push((
        "point DS-top K_sigma".to_string(),
        digest(mats.geometric.val.iter().copied()),
    ));
    // (7): K_sigma of the UDL model.
    let pre = six_prestress(&with_load(&m, Load::line_load("all", down)));
    let w = Warping::new().block(0, dws);
    let mats = matrices(&m, &pre, &w, None).unwrap();
    out.push(("UDL DS K_sigma".to_string(), digest(mats.geometric.val.iter().copied())));
    // (8): the pre-stress of the warping static solve under the UDL.
    let loaded = with_load(&m, Load::line_load("all", down));
    let r = solve_static(&loaded, &w, None).unwrap();
    let wp = WarpingPrestress::from_static(&loaded, &w, &r, [0.0; 2]).unwrap();
    let f = buckling_factors_with(&m, &wp, &w, &opts).unwrap();
    out.push((
        "UDL DS from_static factors".to_string(),
        digest(f.factors.iter().copied()),
    ));
    out
}

/// Printed by the run on the base 83b9d18c plus the ADR commit (no library line changed).
const BASE: &[(&str, u64)] = &[
    ("point DS-top 16 factors", 0xff76d3f8fd8094ad),
    ("point DS-top 8 factors", 0x841ee94d41d03931),
    ("UDL DS down factors", 0x31c49d999fdadd8b),
    ("UDL MS down factors", 0x3efa98a55e27d2f1),
    ("UDL MS up factors", 0x9850738f9ca689ea),
    ("point DS-top K", 0xf9c39e344e1d3383),
    ("point DS-top K_sigma", 0xe06e26ce887bbb84),
    ("UDL DS K_sigma", 0x354770ecd40f1b47),
    ("UDL DS from_static factors", 0x0c71d15ce3dc7df0),
];

#[test]
fn b1_paths_without_a_distributed_height_keep_their_bits() {
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
