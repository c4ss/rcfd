//! F-667 (ADR 0591): the bit record of the default warping transient path (check B0, D6).
//!
//! This file uses only the API of the base 5e6c3587: it was run once with the registration committed and no library line
//! changed, and the digests it printed are the constants of `BASE`. After the change it runs unchanged and compares.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{load_vector, Warping, WarpingSection, WarpingTransient};
use rcfd_fem::transient::{LoadHistory, LoadTable, Method, Rayleigh, TransientOptions};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
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

fn i_parts() -> (BeamSection, WarpingSection) {
    WarpingSection::doubly_symmetric_i(0.1, 0.01, 0.18, 0.006, 5.0 / 6.0).unwrap()
}

/// The fork beam of 16 elements, length 3, under the midspan torque 1000.
fn loaded_fork(section: &BeamSection) -> SolidModel {
    let n = 16;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [3.0 * i as f64 / n as f64, 0.0, 0.0]).collect();
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
    m.add_load(Load::nodal_moment("mid", [1000.0, 0.0, 0.0])).unwrap();
    m
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    let (s, ws) = i_parts();
    let g = 2.0e11 / 2.6;
    let k = PI / 3.0;
    let w1 = ((g * s.torsion() * k * k + 2.0e11 * ws.c_w() * k.powi(4)) / (7850.0 * (s.i_y() + s.i_z()))).sqrt();
    let dt = 2.0 * PI / w1 / 400.0;
    let m = loaded_fork(&s);
    let mut push = |name: &str, w: &Warping, options: &TransientOptions| {
        let f = load_vector(&m, w).unwrap();
        let lh = LoadHistory::new(f, LoadTable::constant(1.0, 0.0, 10.0).unwrap());
        let mut tr = WarpingTransient::new(&m, w, &[lh], options).unwrap();
        tr.start_at_rest(0.0).unwrap();
        tr.run(100).unwrap();
        let u = tr.displacement().unwrap();
        let v = tr.velocity().unwrap();
        out.push((name.to_string(), digest(u.iter().chain(&v).copied())));
    };
    let w = Warping::new().block(0, ws);
    push("Newmark undamped", &w, &TransientOptions::new(dt));
    push(
        "Newmark Rayleigh",
        &w,
        &TransientOptions::new(dt).damping(Rayleigh {
            mass: 0.02 * w1,
            stiffness: 0.02 / w1,
        }),
    );
    let wh = Warping::new().block(0, ws).restrain("x0").restrain("xl");
    push("Newmark undamped, warping held", &wh, &TransientOptions::new(dt));
    push(
        "generalised-alpha undamped",
        &w,
        &TransientOptions::new(dt).method(Method::GeneralisedAlpha { rho_inf: 0.8 }),
    );
    out
}

/// Printed by the run on the base 5e6c3587 plus the ADR commit (no library line changed).
const BASE: &[(&str, u64)] = &[
    ("Newmark undamped", 0xfa1501d55015a1b9),
    ("Newmark Rayleigh", 0x65709ebc32311f62),
    ("Newmark undamped, warping held", 0x0a078dfcc20a00e9),
    ("generalised-alpha undamped", 0xea1c721ea7534ff1),
];

#[test]
fn b0_default_warping_transient_keeps_its_bits() {
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
