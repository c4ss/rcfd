//! F-635 (ADR 0574): the bit record of the default paths (the refusals of the plain harmonic and transient paths on a beam,
//! the plain modes) and of the ADR 0548 warping path (check B0 with B0r, D7 as corrected by amendment 1).
//!
//! This file uses only the API of the base 61033836: it was run once with the registration committed and no library line
//! changed, and the digests it printed are the constants of `BASE`. After the change it runs unchanged and compares.

use rcfd_core::vec3::Vec3;
use rcfd_fem::beam_warping::{modal_analysis, solve_static, Warping, WarpingSection};
use rcfd_fem::harmonic_direct::{DirectHarmonic, DirectHarmonicOptions, LossFactor};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::transient::{Integrator, LoadHistory, LoadTable, TransientOptions};
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

/// A beam along `x`, `n` elements, length 3, with the node sets `x0`, `xl`, `mid`.
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
    mesh.add_node_set("mid", vec![n / 2]).unwrap();
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
    // The plain fork beam under the midspan torque.
    let mut m = fork(&s, 16);
    m.add_load(Load::nodal_moment("mid", [1000.0, 0.0, 0.0])).unwrap();
    // The plain harmonic and transient paths refuse the beam (amendment 1, B0r): the refusals are recorded by their text.
    let refusal = |r: Result<(), rcfd_fem::FemError>| match r {
        Ok(()) => "accepted".to_string(),
        Err(e) => format!("{e:?}"),
    };
    let r1 = refusal(DirectHarmonic::new(&m, &DirectHarmonicOptions::new()).map(|_| ()));
    let r2 = refusal(
        DirectHarmonic::new(
            &m,
            &DirectHarmonicOptions::new().loss_factor("all", LossFactor::Constant(0.02)),
        )
        .map(|_| ()),
    );
    let lh = LoadHistory::model_loads(&m, LoadTable::constant(1.0, 0.0, 1.0).unwrap()).unwrap();
    let r3 = refusal(Integrator::new(&m, &[lh], &TransientOptions::new(0.026062982 / 400.0)).map(|_| ()));
    for (name, r) in [
        ("plain harmonic refused", &r1),
        ("plain harmonic damped refused", &r2),
        ("plain transient refused", &r3),
    ] {
        println!("{name}: {r}");
        out.push((name.to_string(), digest(r.bytes().map(f64::from))));
    }
    // The plain modes of the fork beam.
    let pm = rcfd_fem::modal::modal_analysis(&fork(&s, 16), &ModalOptions::new(4)).unwrap();
    out.push(("plain modal eigenvalues".to_string(), digest(pm.eigenvalues.clone())));
    out.push((
        "plain modal modes".to_string(),
        digest(pm.modes.iter().flatten().copied()),
    ));
    // The ADR 0548 warping path: the cantilever with the root warping held, with and without a bimoment.
    let mut cant = SolidModel::new(beam_mesh(&s, 16));
    cant.add_material("all", steel()).unwrap();
    for c in 0..6 {
        cant.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    cant.add_load(Load::nodal_moment("xl", [1000.0, 0.0, 0.0])).unwrap();
    let w = Warping::new().block(0, ws).restrain("x0");
    let st = solve_static(&cant, &w, Some(1)).unwrap();
    out.push((
        "warping static displacement".to_string(),
        digest(st.displacement.clone()),
    ));
    out.push(("warping static reactions".to_string(), digest(st.reactions.clone())));
    let wb = Warping::new().block(0, ws).restrain("x0").bimoment("xl", 250.0);
    let sb = solve_static(&cant, &wb, Some(1)).unwrap();
    out.push((
        "warping static bimoment displacement".to_string(),
        digest(sb.displacement.clone()),
    ));
    // The ADR 0548 modes of the fork beam.
    let md = modal_analysis(&fork(&s, 16), &Warping::new().block(0, ws), &ModalOptions::new(4)).unwrap();
    out.push(("warping modal eigenvalues".to_string(), digest(md.eigenvalues.clone())));
    out.push((
        "warping modal modes".to_string(),
        digest(md.modes.iter().flatten().copied()),
    ));
    println!("warping eigenvalues {:?}", md.eigenvalues);
    out
}

/// Printed by the run on the base 61033836 plus the ADR commit (no library line changed).
const BASE: &[(&str, u64)] = &[
    ("plain harmonic refused", 0x4809f51a0f40fbfd),
    ("plain harmonic damped refused", 0x4809f51a0f40fbfd),
    ("plain transient refused", 0x4809f51a0f40fbfd),
    ("plain modal eigenvalues", 0x21227728fc698023),
    ("plain modal modes", 0x46b1510bc8b63caf),
    ("warping static displacement", 0xa8975cf9dd949f9c),
    ("warping static reactions", 0x1bb129f8eb1db22f),
    ("warping static bimoment displacement", 0xec09a840661be5b8),
    ("warping modal eigenvalues", 0x059c234a22148daa),
    ("warping modal modes", 0xd3346403a744dea3),
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
