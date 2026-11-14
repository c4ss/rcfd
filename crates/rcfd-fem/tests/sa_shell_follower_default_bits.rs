//! F-573 (ADR 0487): check N1 (record of D5), the dead shell pressure and the paths F-573 could reach give the bits they gave
//! before F-573: `nodal_load_vector` of `Load::ShellPressure` on a flat and a warped Shell4 and on a Shell3 element and of
//! `Load::ShellPressureAngleWeighted` on the Shell3, `solve_static` of a Shell4 plate under a shell pressure,
//! `buckling::factors` of the quarter-ring strip of Shell4 under the dead external pressure, the solid `FollowerPressure`
//! (force, load stiffness, contribution) on a Hex8 and a Tet10 element at a deformed state, and `follower::buckling_factors`
//! of a Hex8 column. Uses only API that existed at fb7c5d94; the expected digests were printed by this file run on the
//! unchanged base fb7c5d94 (`f573-scratch/n1-base.log`) and written into `EXPECTED` before any library code of F-573.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::{hex_box, upgrade_to_tet10};
use rcfd_fem::follower::{buckling_factors, FollowerPressure};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::nonlinear::Contribution;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FaceRef, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

/// Digests of fb7c5d94 in the order of the run, from the base run (`f573-scratch/n1-base.log`).
const EXPECTED: Option<[(&str, u64); 10]> = Some([
    ("Shell4 flat", 0xac39_6fdb_fcef_e931),
    ("Shell4 warped", 0xeb1a_6d45_ba81_6186),
    ("Shell3", 0x0069_541f_839c_ade2),
    ("Shell3 angle weighted", 0x51a1_cd08_9ced_88da),
    ("Shell4 plate static", 0xb38e_5da4_17ab_443d),
    ("Shell4 strip buckling", 0x7d84_ce15_277e_16a0),
    ("Hex8 follower", 0xd11e_0f58_100c_951d),
    ("Tet10 follower", 0x1c6f_a733_89d9_8f24),
    ("Hex8 column follower buckling", 0x62ef_9f0b_5188_9f1b),
    ("count", 0x0000_0000_0000_0009),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(2.0e11, nu, 7850.0, 0.0).unwrap()
}

fn shell_model(kind: ElementKind, nodes: Vec<Vec3>) -> SolidModel {
    let n = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, (0..n).collect()).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    m
}

fn dead(model: &SolidModel, load: Load) -> Vec<f64> {
    let mut m = model.clone();
    m.add_load(load).unwrap();
    nodal_load_vector(&m, &[]).unwrap()
}

/// The quarter-ring strip of ADR 0487 D3: `R = 1`, width 0.05, 24 elements over the quarter, 1 across.
fn strip(tri: bool) -> SolidModel {
    let (r, b, n) = (1.0, 0.05, 24usize);
    let mut nodes = Vec::new();
    for i in 0..=n {
        let th = 0.5 * PI * i as f64 / n as f64;
        for z in [0.0, b] {
            nodes.push([r * th.cos(), r * th.sin(), z]);
        }
    }
    let idx = |i: usize, k: usize| 2 * i + k;
    let mut conn = Vec::new();
    for i in 0..n {
        let (a, bb, c, d) = (idx(i, 0), idx(i + 1, 0), idx(i + 1, 1), idx(i, 1));
        if tri {
            conn.extend_from_slice(&[a, bb, c, a, c, d]);
        } else {
            conn.extend_from_slice(&[a, bb, c, d]);
        }
    }
    let ne = if tri { 2 * n } else { n };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(if tri { ElementKind::Shell3 } else { ElementKind::Shell4 }, conn)
        .unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("nodes", (0..2 * (n + 1)).collect()).unwrap();
    mesh.add_node_set("th0", vec![idx(0, 0), idx(0, 1)]).unwrap();
    mesh.add_node_set("th90", vec![idx(n, 0), idx(n, 1)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.0)).unwrap();
    for c in [2, 3, 4] {
        m.add_constraint(Constraint::displacement("nodes", c, 0.0)).unwrap();
    }
    m.add_constraint(Constraint::displacement("th0", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th0", 5, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th90", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("th90", 5, 0.0)).unwrap();
    m
}

fn solid_element(kind: ElementKind) -> SolidModel {
    let mesh = match kind {
        ElementKind::Hex8 => {
            let mut m = hex_box([0.0; 3], [1.0, 1.0, 1.0], [1, 1, 1]).unwrap();
            m.add_surface_set("p", m.surface_set("zmax").unwrap().to_vec()).unwrap();
            m
        }
        _ => {
            let mut m =
                SolidMesh::new(vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
            m.add_block(ElementKind::Tet4, vec![0, 1, 2, 3]).unwrap();
            m.add_element_set("all", vec![0]).unwrap();
            m.add_surface_set("p", vec![FaceRef { element: 0, face: 0 }]).unwrap();
            upgrade_to_tet10(&m).unwrap().0
        }
    };
    let mut model = SolidModel::new(mesh);
    model.add_material("all", steel(0.0)).unwrap();
    model
}

#[test]
fn n1_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    // Dead shell pressure on single elements.
    let flat4 = shell_model(
        ElementKind::Shell4,
        vec![[0.0, 0.0, 0.0], [1.2, 0.1, 0.0], [1.0, 0.9, 0.0], [-0.1, 1.1, 0.0]],
    );
    got.push(("Shell4 flat", digest(&dead(&flat4, Load::shell_pressure("all", 3.0e7)))));
    let warped4 = shell_model(
        ElementKind::Shell4,
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.06], [1.1, 0.9, 0.0], [-0.1, 1.0, 0.08]],
    );
    got.push((
        "Shell4 warped",
        digest(&dead(&warped4, Load::shell_pressure("all", 3.0e7))),
    ));
    let tri = shell_model(
        ElementKind::Shell3,
        vec![[0.0, 0.0, 0.0], [1.0, 0.1, 0.05], [0.2, 0.9, -0.03]],
    );
    got.push(("Shell3", digest(&dead(&tri, Load::shell_pressure("all", 3.0e7)))));
    got.push((
        "Shell3 angle weighted",
        digest(&dead(&tri, Load::shell_pressure_angle_weighted("all", 3.0e7))),
    ));
    // A Shell4 plate under a shell pressure, clamped on one edge.
    let mut nodes = Vec::new();
    for j in 0..=3 {
        for i in 0..=3 {
            nodes.push([i as f64 / 3.0, j as f64 / 3.0, 0.0]);
        }
    }
    let idx = |i: usize, j: usize| j * 4 + i;
    let mut conn = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..9).collect()).unwrap();
    mesh.add_node_set("x0", (0..=3).map(|j| idx(0, j)).collect()).unwrap();
    let mut plate = SolidModel::new(mesh);
    plate.add_material("all", steel(0.3)).unwrap();
    for c in 0..6 {
        plate.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    plate.add_load(Load::shell_pressure("all", 1.0e3)).unwrap();
    let s = solve_static(&plate, &StaticOptions::default()).unwrap();
    let mut v = s.displacement_values().to_vec();
    v.extend_from_slice(s.reactions().unwrap());
    got.push(("Shell4 plate static", digest(&v)));
    // Buckling of the Shell4 strip under the dead external pressure.
    let mut m = strip(false);
    m.add_load(Load::shell_pressure("all", -1.0e3)).unwrap();
    let s = solve_static(&m, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&m, &s).unwrap();
    let b = factors(&m, &pre, &BucklingOptions::new(2)).unwrap();
    let mut v = b.factors.clone();
    for mode in &b.modes {
        v.extend_from_slice(mode);
    }
    got.push(("Shell4 strip buckling", digest(&v)));
    // The solid follower pressure at a deformed state.
    for (name, kind) in [
        ("Hex8 follower", ElementKind::Hex8),
        ("Tet10 follower", ElementKind::Tet10),
    ] {
        let model = solid_element(kind);
        let n = model.dof_map().n_equations();
        let fp = FollowerPressure::new(&model, "p", 3.0e7).unwrap();
        let u: Vec<f64> = (0..n).map(|i| 0.03 * ((i as f64) * 0.7).sin()).collect();
        let mut v = fp.force(&u, 0.8).unwrap();
        for (r, c, x) in fp.load_stiffness(&u, 0.8).unwrap() {
            v.extend_from_slice(&[r as f64, c as f64, x]);
        }
        let mut force = vec![0.5; n];
        let mut tangent = Vec::new();
        fp.add(&u, 0.8, &mut force, &mut tangent).unwrap();
        v.extend_from_slice(&force);
        for (r, c, x) in tangent {
            v.extend_from_slice(&[r as f64, c as f64, x]);
        }
        got.push((name, digest(&v)));
    }
    // Buckling with the load stiffness of a solid follower pressure: a Hex8 column.
    let mut mesh = hex_box([0.0; 3], [0.1, 0.1, 1.0], [1, 1, 4]).unwrap();
    mesh.add_surface_set("top", mesh.surface_set("zmax").unwrap().to_vec())
        .unwrap();
    let mut col = SolidModel::new(mesh);
    col.add_material("all", steel(0.0)).unwrap();
    for c in 0..3 {
        col.add_constraint(Constraint::displacement("zmin", c, 0.0)).unwrap();
    }
    col.add_load(Load::pressure("top", 1.0e6)).unwrap();
    let s = solve_static(&col, &StaticOptions::default()).unwrap();
    let pre = Prestress::from_static(&col, &s).unwrap();
    let fp = FollowerPressure::new(&col, "top", 1.0e6).unwrap();
    let b = buckling_factors(&col, &pre, &[&fp], &BucklingOptions::new(2)).unwrap();
    let mut v = b.factors.clone();
    for mode in &b.modes {
        v.extend_from_slice(mode);
    }
    got.push(("Hex8 column follower buckling", digest(&v)));
    got.push(("count", got.len() as u64));
    for (name, d) in &got {
        println!(
            "    (\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    match EXPECTED {
        None => panic!("no expected digests yet: print them on the base"),
        Some(expected) => {
            let mut bad = Vec::new();
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                if name != en || d != ed {
                    bad.push(format!("{name}: 0x{d:016x} != 0x{ed:016x}"));
                }
            }
            assert!(got.len() == expected.len() && bad.is_empty(), "N1 changed: {bad:?}");
        }
    }
}
