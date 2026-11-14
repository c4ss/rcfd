//! F-543 (ADR 0460) B1: the bits of the default joint paths of ADR 0456 (`tie::coupling`, `tie::beam_to_solid`,
//! `tie::shell_to_solid`, and the static solves of the J1 and J2 set-ups with them), recorded on main 2504fd62 before any
//! library line of F-543 was written. Uses only the API of that commit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::mpc;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::tie;
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

const E: f64 = 2.0e11;
const P_TIP: f64 = 1000.0;

/// The FNV-1a hash of a sequence of 64-bit words.
fn fnv(words: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

fn rows_words(rows: &[Constraint]) -> Vec<u64> {
    let mut out = Vec::new();
    for r in rows {
        match r {
            Constraint::Linear { terms, value, .. } => {
                out.push(terms.len() as u64);
                for t in terms {
                    out.push(t.node as u64);
                    out.push(t.component as u64);
                    out.push(t.coefficient.to_bits());
                }
                out.push(value.to_bits());
            }
            _ => out.push(u64::MAX),
        }
    }
    out
}

fn material() -> Material {
    Material::isotropic_elastic(E, 0.0, 7850.0, 0.0).unwrap()
}

fn clamp(m: &mut SolidModel, set: &str) {
    for c in 0..3 {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

/// The J1 set-up of ADR 0456 without the joint rows.
fn j1() -> (SolidModel, usize) {
    let a = upgrade_to_hex20(&hex_box([0.0; 3], [0.5, 0.1, 0.1], [10, 2, 2]).unwrap())
        .unwrap()
        .0;
    let nb = 15;
    let nodes: Vec<Vec3> = (0..=nb)
        .map(|i| [0.5 + 1.5 * i as f64 / nb as f64, 0.05, 0.05])
        .collect();
    let mut b = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..nb).flat_map(|i| [i, i + 1]).collect();
    let k = b.add_block(ElementKind::Beam2, conn).unwrap();
    let s = BeamSection::new(0.01, 1e-4 / 12.0, 1e-4 / 12.0, 1.406e-5, 5.0 / 6.0, 5.0 / 6.0).unwrap();
    b.set_block_property(k, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    b.add_element_set("all", (0..nb).collect()).unwrap();
    b.add_node_set("tip", vec![nb]).unwrap();
    let mesh = tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap();
    let jn = a.nodes().len();
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", material()).unwrap();
    m.add_material("B.all", material()).unwrap();
    clamp(&mut m, "A.xmin");
    m.add_load(Load::nodal_force("B.tip", [0.0, 0.0, -P_TIP])).unwrap();
    (m, jn)
}

/// The J2 set-up of ADR 0456 without the joint rows, and its tip forces.
fn j2() -> (SolidModel, Vec<NodalForceAt>) {
    let a = upgrade_to_hex20(&hex_box([0.0, 0.0, -0.01], [0.2, 0.1, 0.02], [4, 2, 1]).unwrap())
        .unwrap()
        .0;
    let (nx, ny) = (16usize, 2usize);
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([0.2 + 0.8 * i as f64 / nx as f64, 0.1 * j as f64 / ny as f64, 0.0]);
        }
    }
    let id = |i: usize, j: usize| i + (nx + 1) * j;
    let mut b = SolidMesh::new(nodes).unwrap();
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            conn.extend_from_slice(&[id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)]);
        }
    }
    let k = b.add_block(ElementKind::Shell4, conn).unwrap();
    b.set_block_property(k, BlockProperty::shell(0.02).unwrap()).unwrap();
    b.add_element_set("all", (0..nx * ny).collect()).unwrap();
    b.add_node_set("edge", (0..=ny).map(|j| id(0, j)).collect()).unwrap();
    let n0 = a.nodes().len();
    let mesh = tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", material()).unwrap();
    m.add_material("B.all", material()).unwrap();
    clamp(&mut m, "A.xmin");
    let forces = vec![
        NodalForceAt::new(n0 + id(nx, 0), [0.0, 0.0, -P_TIP / 4.0]),
        NodalForceAt::new(n0 + id(nx, 1), [0.0, 0.0, -P_TIP / 2.0]),
        NodalForceAt::new(n0 + id(nx, 2), [0.0, 0.0, -P_TIP / 4.0]),
    ];
    (m, forces)
}

fn words() -> [u64; 6] {
    let (mut m1, jn) = j1();
    let face = m1.mesh().surface_set_nodes("A.xmax").unwrap();
    let w0 = fnv(rows_words(&tie::coupling(&m1, jn, &face[..5]).unwrap()));
    let r1 = tie::beam_to_solid(&m1, jn, "A.xmax").unwrap();
    let w1 = fnv(rows_words(&r1));
    for c in r1 {
        m1.add_constraint(c).unwrap();
    }
    let s1 = mpc::solve_static_mpc(&m1, &[], &[], &[], &StaticOptions::default()).unwrap();
    let w2 = fnv(s1.result.displacement_values().iter().map(|x| x.to_bits()));
    let (mut m2, forces) = j2();
    let r2 = tie::shell_to_solid(&m2, "B.edge", "A.xmax", 1e-9).unwrap();
    let w3 = fnv(rows_words(&r2));
    for c in r2 {
        m2.add_constraint(c).unwrap();
    }
    let s2 = mpc::solve_static_mpc(&m2, &[], &forces, &[], &StaticOptions::default()).unwrap();
    let w4 = fnv(s2.result.displacement_values().iter().map(|x| x.to_bits()));
    let w5 = fnv(s2.constraint_forces.iter().map(|x| x.to_bits()));
    [w0, w1, w2, w3, w4, w5]
}

/// The words recorded on main 2504fd62 (ADR 0460 B1).
const BASE: [u64; 6] = [
    15140349673565816372,
    9651008286018655624,
    4512398192402801758,
    18165430941804242041,
    10055471568661746040,
    4936766323991312539,
];

#[test]
fn b1_default_joint_bits() {
    let w = words();
    for (i, x) in w.iter().enumerate() {
        println!("B1 word {i}: {x:#018x}");
    }
    assert_eq!(w, BASE);
}
