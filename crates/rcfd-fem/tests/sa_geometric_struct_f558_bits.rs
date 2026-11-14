//! F-558 (ADR 0508): the bit record of the default paths that F-558 touches (check B0 of the registration).
//!
//! This file uses only the API of the base 031b09c6 (plus the ADR commit): it was run once with the registration committed
//! and no library line changed, and the digests it printed are the constants of `BASE`. After the change it runs unchanged and
//! compares. The models are a Beam2 model under thrust and moment (the lateral-torsional set-up of F-558), a Shell4 and a
//! Shell3 plate under uniaxial compression; the element matrices of the three kinds, the assembled geometric stiffness, the
//! buckling factors and the pre-stressed modes.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::geometric::{geometric_stiffness, prestressed_modes, Prestress};
use rcfd_fem::geometric_struct::element_geometric_stiffness;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{beam, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use rcfd_linalg::eigen::ResidualTest;

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

/// A beam along `x`, 8 elements, `L = 3`, section `0.01 x 0.2`, fork supports, torsion held at both ends.
fn beam_model() -> SolidModel {
    let n = 8;
    let nodes: Vec<Vec3> = (0..=n).map(|i| [3.0 * i as f64 / n as f64, 0.0, 0.0]).collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let s = beam::rectangle_section(0.01, 0.2, 6.4567e-8, 5.0 / 6.0).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
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
    m
}

fn plate(tri: bool) -> SolidModel {
    let (nx, ny, t) = (6usize, 6usize, 0.05);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([i as f64 / nx as f64, j as f64 / ny as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    let ne = if tri { 2 * nx * ny } else { nx * ny };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(if tri { ElementKind::Shell3 } else { ElementKind::Shell4 }, conn)
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(t).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let col = |i: usize| (0..=ny).map(|j| idx(i, j)).collect::<Vec<_>>();
    let row = |j: usize| (0..=nx).map(|i| idx(i, j)).collect::<Vec<_>>();
    let mut edges: Vec<usize> = col(0).into_iter().chain(col(nx)).chain(row(0)).chain(row(ny)).collect();
    edges.sort_unstable();
    edges.dedup();
    mesh.add_node_set("edges", edges).unwrap();
    mesh.add_node_set("x0", col(0)).unwrap();
    mesh.add_node_set("xl_mid", (1..ny).map(|j| idx(nx, j)).collect())
        .unwrap();
    mesh.add_node_set("xl_end", vec![idx(nx, 0), idx(nx, ny)]).unwrap();
    mesh.add_node_set("c00", vec![idx(0, 0)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel()).unwrap();
    m.add_constraint(Constraint::displacement("edges", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c00", 1, 0.0)).unwrap();
    m
}

fn loaded_plate(m: &SolidModel) -> SolidModel {
    let h = 1.0 / 6.0;
    let mut l = m.clone();
    l.add_load(Load::nodal_force("xl_mid", [-1000.0 * h, 0.0, 0.0]))
        .unwrap();
    l.add_load(Load::nodal_force("xl_end", [-500.0 * h, 0.0, 0.0])).unwrap();
    l
}

fn mopts(n: usize) -> ModalOptions {
    ModalOptions::new(n)
        .residual_test(ResidualTest::Backward)
        .tolerance(1e-12)
}

fn element_mesh(kind: ElementKind, conn: Vec<usize>, nodes: Vec<Vec3>, property: BlockProperty) -> SolidMesh {
    let mut m = SolidMesh::new(nodes).unwrap();
    m.add_block(kind, conn).unwrap();
    m.set_block_property(0, property).unwrap();
    m
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    // Element matrices (global) of the three kinds, three orientations of the beam.
    let mat = steel();
    let s = beam::rectangle_section(0.01, 0.2, 6.4567e-8, 5.0 / 6.0).unwrap();
    let sigma = [3.0e6, 0.2e6, -0.1e6, 0.4e6, 0.3e6, -0.2e6];
    for (k, (a, b)) in [
        ([0.0, 0.0, 0.0], [2.0, 0.0, 0.0]),
        ([0.5, -0.3, 0.2], [1.1, 0.9, -0.4]),
        ([0.0, 0.0, 0.0], [0.0, 1.5, 1.5]),
    ]
    .into_iter()
    .enumerate()
    {
        let coords = [a, b];
        let mesh = element_mesh(
            ElementKind::Beam2,
            vec![0, 1],
            coords.to_vec(),
            BlockProperty::beam(s, [0.3, 0.2, 1.0]).unwrap(),
        );
        let ke = element_geometric_stiffness(&mesh, 0, &coords, &mat, &[sigma]).unwrap();
        out.push((format!("beam element {k}"), digest(ke.values().iter().copied())));
    }
    let quad = [[0.0, 0.0, 0.0], [1.2, 0.1, 0.0], [1.3, 1.1, 0.2], [0.0, 0.9, 0.1]];
    let m4 = element_mesh(
        ElementKind::Shell4,
        vec![0, 1, 2, 3],
        quad.to_vec(),
        BlockProperty::shell(0.05).unwrap(),
    );
    let k4 = element_geometric_stiffness(&m4, 0, &quad, &mat, &[sigma]).unwrap();
    out.push(("shell4 element".to_string(), digest(k4.values().iter().copied())));
    let tri = [[0.0, 0.0, 0.0], [1.2, 0.1, 0.0], [0.3, 1.0, 0.2]];
    let m3 = element_mesh(
        ElementKind::Shell3,
        vec![0, 1, 2],
        tri.to_vec(),
        BlockProperty::shell(0.05).unwrap(),
    );
    let k3 = element_geometric_stiffness(&m3, 0, &tri, &mat, &[sigma]).unwrap();
    out.push(("shell3 element".to_string(), digest(k3.values().iter().copied())));

    // Beam under thrust 1e3 and end moments 1e3 about the strong axis (the F-558 set-up): static pre-stress, geometric
    // stiffness, buckling factors, pre-stressed modes.
    let m = beam_model();
    let mut loaded = m.clone();
    loaded.add_load(Load::nodal_force("xl", [-1.0e3, 0.0, 0.0])).unwrap();
    loaded.add_load(Load::nodal_moment("x0", [0.0, -1.0e3, 0.0])).unwrap();
    loaded.add_load(Load::nodal_moment("xl", [0.0, 1.0e3, 0.0])).unwrap();
    let r = solve_static(&loaded, &StaticOptions::default()).unwrap();
    out.push((
        "beam static displacement".to_string(),
        digest(r.displacement_values().iter().copied()),
    ));
    let pre = Prestress::from_static(&loaded, &r).unwrap();
    out.push((
        "beam prestress".to_string(),
        digest(pre.stress().iter().flatten().copied()),
    ));
    let kg = geometric_stiffness(&m, &pre, Some(1)).unwrap();
    out.push(("beam geometric stiffness".to_string(), digest(kg.val.iter().copied())));
    let f = factors(&m, &pre, &BucklingOptions::new(3)).unwrap();
    out.push(("beam buckling factors".to_string(), digest(f.factors.iter().copied())));
    let small = pre.scaled(1e-3);
    let pm = prestressed_modes(&m, &small, &mopts(3)).unwrap();
    out.push((
        "beam prestressed eigenvalues".to_string(),
        digest(pm.eigenvalues.iter().copied()),
    ));

    for tri in [false, true] {
        let tag = if tri { "shell3" } else { "shell4" };
        let m = plate(tri);
        let loaded = loaded_plate(&m);
        let r = solve_static(&loaded, &StaticOptions::default()).unwrap();
        let pre = Prestress::from_static(&loaded, &r).unwrap();
        out.push((
            format!("{tag} prestress"),
            digest(pre.stress().iter().flatten().copied()),
        ));
        let kg = geometric_stiffness(&m, &pre, Some(1)).unwrap();
        out.push((format!("{tag} geometric stiffness"), digest(kg.val.iter().copied())));
        let f = factors(&m, &pre, &BucklingOptions::new(3)).unwrap();
        out.push((format!("{tag} buckling factors"), digest(f.factors.iter().copied())));
    }
    out
}

const BASE: &[(&str, u64)] = &[
    ("beam element 0", 0xacfacf4c75af6b46),
    ("beam element 1", 0x855370c290866c61),
    ("beam element 2", 0x92d09af4c4e90fd4),
    ("shell4 element", 0xf3216984429acd8d),
    ("shell3 element", 0xb7b1ede330f5b8ad),
    ("beam static displacement", 0x5f9914938b3d928f),
    ("beam prestress", 0x996efb82f2f8a53f),
    ("beam geometric stiffness", 0x254abf97865b9087),
    ("beam buckling factors", 0x3b6f67a820f3a49c),
    ("beam prestressed eigenvalues", 0x01030767bc956a6a),
    ("shell4 prestress", 0xebdf61184d01aae3),
    ("shell4 geometric stiffness", 0xa015459772aa832a),
    ("shell4 buckling factors", 0x572309a0a462fc61),
    ("shell3 prestress", 0xc624ca78f865fe7f),
    ("shell3 geometric stiffness", 0xd56eead4ed7db86b),
    ("shell3 buckling factors", 0x2f78a66eb5bbc548),
];

#[test]
fn b0_default_paths_keep_their_bits() {
    let got = all_digests();
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
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
