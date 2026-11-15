//! F-661 (ADR 0585): the bit record of the default warping harmonic path (check B0, D5).
//!
//! This file uses only the API of the base 535de064: it was run once with the registration committed and no library line
//! changed, and the digests it printed are the constants of `BASE`. After the change it runs unchanged and compares.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{load_vector, Warping, WarpingHarmonic, WarpingHarmonicOptions, WarpingSection};
use rcfd_fem::harmonic::HarmonicLoad;
use rcfd_fem::transient::Rayleigh;
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
    let m = loaded_fork(&s);
    let mut push = |name: &str, w: &Warping, options: &WarpingHarmonicOptions, omega: f64| {
        let load = HarmonicLoad::real(load_vector(&m, w).unwrap()).unwrap();
        let h = WarpingHarmonic::new(&m, w, options).unwrap();
        let r = h.response(&load, omega).unwrap();
        out.push((
            name.to_string(),
            digest(r.re.iter().chain(&r.im).copied().chain([r.omega])),
        ));
    };
    let w = Warping::new().block(0, ws);
    push("undamped at 0.5 omega_1", &w, &WarpingHarmonicOptions::new(), 0.5 * w1);
    push(
        "loss factor at omega_1",
        &w,
        &WarpingHarmonicOptions::new().loss_factor(0.02),
        w1,
    );
    push(
        "Rayleigh at omega_1",
        &w,
        &WarpingHarmonicOptions::new().rayleigh(Rayleigh {
            mass: 0.02 * w1,
            stiffness: 0.02 / w1,
        }),
        w1,
    );
    push("undamped at 0", &w, &WarpingHarmonicOptions::new(), 0.0);
    let wh = Warping::new().block(0, ws).restrain("x0").restrain("xl");
    push(
        "held, undamped at 0.5 omega_1",
        &wh,
        &WarpingHarmonicOptions::new(),
        0.5 * w1,
    );
    out
}

/// Printed by the run on the base 535de064 plus the ADR commit (no library line changed).
const BASE: &[(&str, u64)] = &[
    ("undamped at 0.5 omega_1", 0x65308581589d76bd),
    ("loss factor at omega_1", 0x6feb87c639f58739),
    ("Rayleigh at omega_1", 0x5d87768e6c5b095b),
    ("undamped at 0", 0x358baf70ed4fd83b),
    ("held, undamped at 0.5 omega_1", 0xc77dc17cfb544444),
];

#[test]
fn b0_default_warping_harmonic_keeps_its_bits() {
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
