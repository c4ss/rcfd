//! F-626 (ADR 0548): the bit record of the default paths and of the ADR 0546 warping buckling path (check B0, D8).
//!
//! This file uses only the API of the base e906faa8 (plus the ADR commit): it was run once with the registration committed and
//! no library line changed, and the digests it printed are the constants of `BASE`. After the change it runs unchanged and
//! compares.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{self, buckling_factors, matrices, Warping, WarpingSection};
use rcfd_fem::buckling::BucklingOptions;
use rcfd_fem::geometric::{prestressed_modes, Prestress};
use rcfd_fem::modal::{modal_analysis, ModalOptions};
use rcfd_fem::solve::{solve_static, StaticOptions};
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

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap()
}

fn i_parts() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5.0 / 6.0).unwrap()
}

/// A beam along `x`, `n` elements, length 3, with the node sets `x0`, `xl`.
fn beam_mesh(section: &BeamSection, n: usize) -> SolidMesh {
    let nodes: Vec<Vec3> = (0..=n).map(|i| [3.0 * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(*section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    mesh
}

fn fork(section: &BeamSection, n: usize) -> SolidModel {
    let mut m = SolidModel::new(beam_mesh(section, n));
    m.add_material("all", steel()).unwrap();
    for c in [0, 1, 2, 3] {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    for c in [1, 2, 3] {
        m.add_constraint(Constraint::displacement("xl", c, 0.0)).unwrap();
    }
    m
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let (s, ws) = i_parts();
    let mat = steel();
    // Plain static solve of the cantilever under end torque and a transverse force.
    let mut cant = SolidModel::new(beam_mesh(&s, 16));
    cant.add_material("all", steel()).unwrap();
    for c in 0..6 {
        cant.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    cant.add_load(Load::nodal_moment("xl", [1000.0, 0.0, 0.0])).unwrap();
    cant.add_load(Load::nodal_force("xl", [0.0, 200.0, -300.0])).unwrap();
    let r = solve_static(&cant, &StaticOptions::default()).unwrap();
    out.push((
        "cantilever static displacement".to_string(),
        digest(r.displacement_values().to_vec()),
    ));
    // Plain modes of the fork beam.
    let m = fork(&s, 16);
    let md = modal_analysis(&m, &ModalOptions::new(4)).unwrap();
    out.push(("fork eigenvalues".to_string(), digest(md.eigenvalues.clone())));
    out.push(("fork modes".to_string(), digest(md.modes.iter().flatten().copied())));
    // Plain pre-stressed modes under axial tension.
    let mut tens = m.clone();
    tens.add_load(Load::nodal_force("xl", [6.4e5, 0.0, 0.0])).unwrap();
    let rt = solve_static(&tens, &StaticOptions::default()).unwrap();
    let pt = Prestress::from_static(&tens, &rt).unwrap();
    let pm = prestressed_modes(&m, &pt, &ModalOptions::new(4)).unwrap();
    out.push((
        "fork prestressed eigenvalues".to_string(),
        digest(pm.eigenvalues.clone()),
    ));
    out.push((
        "fork prestressed modes".to_string(),
        digest(pm.modes.iter().flatten().copied()),
    ));
    // ADR 0546 element matrices.
    out.push((
        "warping local stiffness".to_string(),
        digest(beam_warping::local_stiffness(0.7, &s, &mat, ws.c_w()).values().to_vec()),
    ));
    let f = [
        -50.0, -2.0, 3.0, 0.0, 1000.0, -400.0, 50.0, 2.0, -3.0, 0.0, -997.9, 401.4,
    ];
    out.push((
        "warping local geometric stiffness".to_string(),
        digest(
            beam_warping::local_geometric_stiffness(0.7, &s, &mat, -5e3, Some(&f), [0.7, -0.4])
                .values()
                .to_vec(),
        ),
    ));
    // ADR 0546 assembly and buckling of the uniform-moment I beam.
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_moment("x0", [0.0, -1000.0, 0.0])).unwrap();
    loaded.add_load(Load::nodal_moment("xl", [0.0, 1000.0, 0.0])).unwrap();
    let rl = solve_static(&loaded, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&loaded, &rl)
        .unwrap()
        .with_beam_resultants(&loaded, rl.displacement_values(), [0.0; 2])
        .unwrap();
    let w = Warping::new().block(0, ws);
    let mats = matrices(&m, &pre, &w, Some(1)).unwrap();
    out.push(("warping K_aug".to_string(), digest(mats.stiffness.val.clone())));
    out.push(("warping K_sigma_aug".to_string(), digest(mats.geometric.val.clone())));
    let b = buckling_factors(&m, &pre, &w, &BucklingOptions::new(3)).unwrap();
    out.push(("warping buckling factors".to_string(), digest(b.factors.clone())));
    out.push((
        "warping buckling modes".to_string(),
        digest(b.modes.iter().flatten().copied()),
    ));
    println!("warping factors {:?}", b.factors);
    out
}

/// Printed by the run on the base e906faa8 plus the ADR commit (no library line changed).
const BASE: &[(&str, u64)] = &[
    ("cantilever static displacement", 0x1d368a11891abfca),
    ("fork eigenvalues", 0x21227728fc698023),
    ("fork modes", 0x46b1510bc8b63caf),
    ("fork prestressed eigenvalues", 0xd15c1422f1916a95),
    ("fork prestressed modes", 0x0cfd91c193e608d8),
    ("warping local stiffness", 0xfc42cec625257c19),
    ("warping local geometric stiffness", 0xdcc7965c676b2c95),
    ("warping K_aug", 0xf9c39e344e1d3383),
    ("warping K_sigma_aug", 0x553f1759348a458f),
    ("warping buckling factors", 0x8fbf692f68397531),
    ("warping buckling modes", 0x9c5a17416ebe42e0),
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
