//! F-623 (ADR 0551): copies of the ADR 0540 set-ups of `tests/sa_contact_faces_f601.rs` used by the F-623 test files (the
//! patch blocks of PT10 and PT20, the T1 models with Hex20 and Tet10 faces, the Q20 punch).
#![allow(dead_code)]

use crate::f601_common::*;
use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::tie::assemble_parts;
use rcfd_fem::{Constraint, ElementKind, FaceRef, Load, SolidMesh, SolidModel};

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Tet4,
    Tet10,
    Hex20,
}

pub fn patch_mesh(kind: Kind, origin: Vec3) -> SolidMesh {
    match kind {
        Kind::Tet4 => tet_box(origin, [1.0, 1.0, 1.0], [2, 2, 2]).unwrap(),
        Kind::Tet10 => {
            upgrade_to_tet10(&tet_box(origin, [1.0, 1.0, 1.0], [2, 2, 2]).unwrap())
                .unwrap()
                .0
        }
        Kind::Hex20 => {
            upgrade_to_hex20(&hex_box(origin, [1.0, 1.0, 1.0], [2, 2, 2]).unwrap())
                .unwrap()
                .0
        }
    }
}

/// ADR 0540 D5 PT: ADR 0507 B1's blocks of `kind`, pressed by `p` on the upper top.
pub fn patch_model(kind: Kind, p: f64) -> SolidModel {
    let lower = patch_mesh(kind, [0.0; 3]);
    let upper = patch_mesh(kind, [0.0, 0.0, 1.0]);
    let mesh = assemble_parts(&[("lower", &lower), ("upper", &upper)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("lower.all", elastic(E_BL, 0.3)).unwrap();
    m.add_material("upper.all", elastic(E_BL, 0.3)).unwrap();
    m.add_constraint(Constraint::displacement("lower.zmin", 2, 0.0))
        .unwrap();
    for s in ["lower", "upper"] {
        m.add_constraint(Constraint::displacement(&format!("{s}.xmin"), 0, 0.0))
            .unwrap();
        m.add_constraint(Constraint::displacement(&format!("{s}.ymin"), 1, 0.0))
            .unwrap();
    }
    m.add_load(Load::pressure("upper.zmax", p)).unwrap();
    m
}

/// ADR 0540 Q20: ADR 0507 T2's punch on its plate upgraded to Hex20.
pub fn q20_model() -> SolidModel {
    let plate = upgrade_to_hex20(&hex_box([0.0; 3], [2.0, 0.5, 0.25], [8, 2, 2]).unwrap())
        .unwrap()
        .0;
    let pn = hex_box([0.8, 0.0, 0.25], [0.4, 0.2, 0.2], [3, 1, 1]).unwrap();
    let mesh = assemble_parts(&[("plate", &plate), ("punch", &pn)]).unwrap();
    punch_constraints(mesh, 0.01)
}

/// Hex20 nodes of a hexahedron with the given corners: the corners, then the mid-edge nodes in the order of `ElementKind::Hex20`.
pub fn hex20_nodes(corners: &[Vec3; 8]) -> Vec<Vec3> {
    const EDGES: [[usize; 2]; 12] = [
        [0, 1],
        [1, 2],
        [2, 3],
        [3, 0],
        [4, 5],
        [5, 6],
        [6, 7],
        [7, 4],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];
    let mut v = corners.to_vec();
    for [a, b] in EDGES {
        v.push([
            0.5 * (corners[a][0] + corners[b][0]),
            0.5 * (corners[a][1] + corners[b][1]),
            0.5 * (corners[a][2] + corners[b][2]),
        ]);
    }
    v
}

/// Tet10 nodes: the corners, then the mid-edge nodes of (0,1), (1,2), (0,2), (0,3), (1,3), (2,3).
pub fn tet10_nodes(corners: &[Vec3; 4]) -> Vec<Vec3> {
    let mut v = corners.to_vec();
    for [a, b] in [[0, 1], [1, 2], [0, 2], [0, 3], [1, 3], [2, 3]] {
        v.push([
            0.5 * (corners[a][0] + corners[b][0]),
            0.5 * (corners[a][1] + corners[b][1]),
            0.5 * (corners[a][2] + corners[b][2]),
        ]);
    }
    v
}

pub const T1_LOWER: [Vec3; 8] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, 0.0, 1.05],
    [1.0, 0.0, 0.96],
    [1.0, 1.0, 1.06],
    [0.0, 1.0, 0.97],
];

pub fn t1_upper() -> [Vec3; 8] {
    let c = T1_CONTACTOR;
    let mut v = [[0.0; 3]; 8];
    for a in 0..4 {
        v[a] = c[a];
        v[a + 4] = [c[a][0], c[a][1], c[a][2] + 1.0];
    }
    v
}

/// A mesh of a target block (first) and a contactor block (second); surface sets "target" and "contactor".
pub fn two_blocks(target: (ElementKind, Vec<Vec3>, usize), contactor: (ElementKind, Vec<Vec3>, usize)) -> SolidModel {
    let nt = target.1.len();
    let mut nodes = target.1.clone();
    nodes.extend_from_slice(&contactor.1);
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(target.0, (0..nt).collect()).unwrap();
    mesh.add_block(contactor.0, (nt..nt + contactor.1.len()).collect())
        .unwrap();
    mesh.add_element_set("all", vec![0, 1]).unwrap();
    mesh.add_surface_set(
        "target",
        vec![FaceRef {
            element: 0,
            face: target.2,
        }],
    )
    .unwrap();
    mesh.add_surface_set(
        "contactor",
        vec![FaceRef {
            element: 1,
            face: contactor.2,
        }],
    )
    .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", elastic(1.0, 0.3)).unwrap();
    m
}

/// ADR 0540 T1q's curved Hex20 target block's nodes.
pub fn hex20_target_nodes() -> Vec<Vec3> {
    let mut t = hex20_nodes(&T1_LOWER);
    for (k, dz) in [(12, 0.03), (13, -0.02), (14, 0.025), (15, -0.015)] {
        t[k][2] += dz;
    }
    t
}

/// ADR 0540 T1g20's Hex20 contactor block's nodes.
pub fn hex20_contactor_nodes() -> Vec<Vec3> {
    let mut cn = hex20_nodes(&t1_upper());
    for (k, dz) in [(8, 0.01), (9, -0.01), (10, 0.005), (11, -0.005)] {
        cn[k][2] -= dz;
    }
    cn
}

/// ADR 0540 T1g10's Tet10 contactor block's nodes.
pub fn tet10_contactor_nodes() -> Vec<Vec3> {
    let mut cn = tet10_nodes(&[[0.2, 0.2, 0.99], [0.8, 0.25, 0.98], [0.3, 0.8, 1.0], [0.4, 0.4, 1.8]]);
    for (k, dz) in [(4, 0.01), (5, -0.01), (6, 0.005)] {
        cn[k][2] += dz;
    }
    cn
}

/// ADR 0540 T1g20's model: the Hex20 contactor on T1's warped Hex8 target.
pub fn hex20_contactor_model() -> SolidModel {
    two_blocks(
        (ElementKind::Hex8, T1_LOWER.to_vec(), 1),
        (ElementKind::Hex20, hex20_contactor_nodes(), 0),
    )
}

/// ADR 0540 T1g10's model: the Tet10 contactor on T1's warped Hex8 target.
pub fn tet10_contactor_model() -> SolidModel {
    two_blocks(
        (ElementKind::Hex8, T1_LOWER.to_vec(), 1),
        (ElementKind::Tet10, tet10_contactor_nodes(), 0),
    )
}

/// ADR 0551 M20q: the Hex20 contactor of T1g20 on T1q's curved Hex20 target.
pub fn hex20_on_hex20_model() -> SolidModel {
    two_blocks(
        (ElementKind::Hex20, hex20_target_nodes(), 1),
        (ElementKind::Hex20, hex20_contactor_nodes(), 0),
    )
}
