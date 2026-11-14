//! F-557 (ADR 0486): the bit record of the default paths that F-557 touches (check B0 of the registration).
//!
//! This file uses only the API of the base fb7c5d94: it was run once in the worktree at that commit with the registration
//! committed and no library line changed, and the digests it printed are the constants of `BASE`. After the change it runs
//! unchanged and compares. The models have no mixed elements: the pre-stress of a static result of a beam, a Shell4 plate
//! and a Shell3 plate (structural only), of a Hex20 cantilever, a Hex8 block with a temperature field and a Tet10 block
//! (solid only), the integration-point field of the thermal block, the buckling factors of the Shell4 plate, the rotating
//! part of a Shell4 strip; and, for reference, a static solve of a mixed model with a beam joint.

use rcfd_core::vec3::Vec3;
use rcfd_fem::buckling::{factors, BucklingOptions};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::geometric::Prestress;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::rotating::{RotatingOptions, RotatingPart, SpeedModes};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::tie::{self, JointCoupling};
use rcfd_fem::{beam, mpc};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
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

fn steel(nu: f64) -> Material {
    Material::isotropic_elastic(2.0e11, nu, 7850.0, 1.2e-5).unwrap()
}

fn prestress_digest(tag: &str, p: &Prestress, out: &mut Vec<(String, u64)>) {
    out.push((
        format!("{tag} prestress offsets"),
        digest(p.offsets().iter().map(|&o| o as f64)),
    ));
    out.push((
        format!("{tag} prestress stress"),
        digest(p.stress().iter().flatten().copied()),
    ));
}

fn static_prestress(m: &SolidModel) -> Prestress {
    let r = solve_static(m, &StaticOptions::default()).unwrap();
    Prestress::from_static(m, &r).unwrap()
}

/// A beam along a skew line, 8 elements, clamped at node 0, an end force and gravity.
fn beam_model() -> SolidModel {
    let n = 8;
    let d = [0.8, 0.5, 0.3];
    let nodes: Vec<Vec3> = (0..=n)
        .map(|i| {
            let s = i as f64 / n as f64;
            [0.1 + d[0] * s, -0.2 + d[1] * s, 0.05 + d[2] * s]
        })
        .collect();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..n).flat_map(|i| [i, i + 1]).collect();
    mesh.add_block(ElementKind::Beam2, conn).unwrap();
    let s = beam::rectangle_section(0.02, 0.03, 1.0e-8, 5.0 / 6.0).unwrap();
    mesh.set_block_property(0, BlockProperty::beam(s, [0.0, 0.0, 1.0]).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("x0", vec![0]).unwrap();
    mesh.add_node_set("xl", vec![n]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xl", [3.0e4, -1.0e3, 2.0e3])).unwrap();
    m.add_load(Load::gravity([0.0, 0.0, -9.81])).unwrap();
    m
}

/// A 4 x 3 plate of Shell4 (or Shell3) in a tilted plane, root clamped, in-plane end forces and gravity.
fn shell_plate(tri: bool) -> SolidModel {
    let (nx, ny) = (4usize, 3usize);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            let (x, y) = (0.25 * i as f64, 0.2 * j as f64);
            nodes.push([x, 0.9 * y, 0.1 * x + 0.3 * y]);
        }
    }
    let mut conn = Vec::new();
    let mut ne = 0;
    for j in 0..ny {
        for i in 0..nx {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if tri {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
                ne += 2;
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
                ne += 1;
            }
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    let kind = if tri { ElementKind::Shell3 } else { ElementKind::Shell4 };
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("root", (0..=ny).map(|j| idx(0, j)).collect())
        .unwrap();
    mesh.add_node_set("tip", (0..=ny).map(|j| idx(nx, j)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("tip", [2.0e3, 5.0e2, -1.0e2])).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, 0.0])).unwrap();
    m
}

fn hex20_cantilever() -> SolidModel {
    let base = hex_box([0.0; 3], [1.0, 0.1, 0.1], [8, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [1.0e3, -2.0e2, 3.0e2])).unwrap();
    m
}

fn hex8_thermal() -> SolidModel {
    let mesh = hex_box([0.0; 3], [0.4, 0.2, 0.1], [4, 2, 2]).unwrap();
    let t: Vec<f64> = mesh
        .nodes()
        .iter()
        .map(|p| 20.0 + 100.0 * p[0] + 50.0 * p[1] * p[2])
        .collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::temperature(t, 20.0)).unwrap();
    m.add_load(Load::nodal_force("xmax", [0.0, 1.0e3, 0.0])).unwrap();
    m
}

fn tet10_block() -> SolidModel {
    let base = tet_box([0.0; 3], [0.3, 0.2, 0.2], [3, 2, 2]).unwrap();
    let mesh = upgrade_to_tet10(&base).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.25)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [-5.0e2, 0.0, 2.0e2])).unwrap();
    m
}

/// A Shell4 plate 6 x 6 on `[0, 1]^2`, `w = 0` on the edges, compressed along `x` by edge forces.
fn shell_plate_buckling() -> (SolidModel, SolidModel) {
    let n = 6usize;
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let h = 1.0 / n as f64;
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push([h * i as f64, h * j as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..n {
        for i in 0..n {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..n * n).collect()).unwrap();
    let mut edges: Vec<usize> = (0..=n)
        .flat_map(|k| [idx(0, k), idx(n, k), idx(k, 0), idx(k, n)])
        .collect();
    edges.sort_unstable();
    edges.dedup();
    mesh.add_node_set("edges", edges).unwrap();
    mesh.add_node_set("x0", (0..=n).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("c00", vec![idx(0, 0)]).unwrap();
    mesh.add_node_set("xl_mid", (1..n).map(|j| idx(n, j)).collect())
        .unwrap();
    mesh.add_node_set("xl_end", vec![idx(n, 0), idx(n, n)]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    m.add_constraint(Constraint::displacement("edges", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("x0", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("c00", 1, 0.0)).unwrap();
    let mut loaded = m.clone();
    loaded
        .add_load(Load::nodal_force("xl_mid", [-1000.0 * h, 0.0, 0.0]))
        .unwrap();
    loaded
        .add_load(Load::nodal_force("xl_end", [-500.0 * h, 0.0, 0.0]))
        .unwrap();
    (m, loaded)
}

/// A clamped radial Shell4 strip 8 x 2 along `x` from `0.05` to `0.2` in the `x-y` plane.
fn shell_strip() -> SolidModel {
    let (nx, ny) = (8usize, 2usize);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            nodes.push([
                0.05 + 0.15 * i as f64 / nx as f64,
                -0.025 + 0.05 * j as f64 / ny as f64,
                0.0,
            ]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            conn.extend_from_slice(&[idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.001).unwrap())
        .unwrap();
    mesh.add_element_set("all", (0..nx * ny).collect()).unwrap();
    mesh.add_node_set("root", (0..=ny).map(|j| idx(0, j)).collect())
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("root", c, 0.0)).unwrap();
    }
    m
}

/// J1 of ADR 0460 shortened: a Hex20 bar with a Beam2 continuing it, the consistent joint, a tip force.
fn mixed_joint() -> SolidModel {
    let a = upgrade_to_hex20(&hex_box([0.0; 3], [0.5, 0.1, 0.1], [5, 1, 1]).unwrap())
        .unwrap()
        .0;
    let nb = 6;
    let nodes: Vec<Vec3> = (0..=nb)
        .map(|i| [0.5 + 0.5 * i as f64 / nb as f64, 0.05, 0.05])
        .collect();
    let mut b = SolidMesh::new(nodes).unwrap();
    let conn: Vec<usize> = (0..nb).flat_map(|i| [i, i + 1]).collect();
    let k = b.add_block(ElementKind::Beam2, conn).unwrap();
    let s = beam::rectangle_section(0.1, 0.1, 1.406e-5, 5.0 / 6.0).unwrap();
    b.set_block_property(k, BlockProperty::beam(s, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    b.add_element_set("all", (0..nb).collect()).unwrap();
    b.add_node_set("tip", vec![nb]).unwrap();
    let mesh = tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap();
    let joint = a.nodes().len();
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", steel(0.0)).unwrap();
    m.add_material("B.all", steel(0.0)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("A.xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("B.tip", [1.0e3, 0.0, -1.0e3])).unwrap();
    for r in tie::beam_to_solid_with(&m, joint, "A.xmax", JointCoupling::Consistent).unwrap() {
        m.add_constraint(r).unwrap();
    }
    m
}

fn words() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    prestress_digest("beam", &static_prestress(&beam_model()), &mut out);
    prestress_digest("shell4", &static_prestress(&shell_plate(false)), &mut out);
    prestress_digest("shell3", &static_prestress(&shell_plate(true)), &mut out);
    prestress_digest("hex20", &static_prestress(&hex20_cantilever()), &mut out);
    let thermal = hex8_thermal();
    prestress_digest("hex8 thermal", &static_prestress(&thermal), &mut out);
    prestress_digest("tet10", &static_prestress(&tet10_block()), &mut out);
    let r = solve_static(&thermal, &StaticOptions::default()).unwrap();
    let f = IntegrationPointField::from_result(&thermal, &r).unwrap();
    out.push((
        "field positions".into(),
        digest(f.positions().iter().flatten().copied()),
    ));
    out.push(("field weights".into(), digest(f.weights().iter().copied())));
    out.push(("field strain".into(), digest(f.strain().iter().flatten().copied())));
    out.push(("field stress".into(), digest(f.stress().iter().flatten().copied())));
    out.push((
        "field thermal".into(),
        digest(f.thermal_strain().unwrap().iter().flatten().copied()),
    ));
    let (plate, loaded) = shell_plate_buckling();
    let pre = static_prestress(&loaded);
    let b = factors(&plate, &pre, &BucklingOptions::new(2)).unwrap();
    out.push(("shell4 buckling factors".into(), digest(b.factors.iter().copied())));
    let strip = shell_strip();
    let options = RotatingOptions::new(3).modal(
        ModalOptions::new(3)
            .residual_test(ResidualTest::Backward)
            .tolerance(1e-10),
    );
    let part = RotatingPart::new(&strip, [0.0, 0.0, 1.0], [0.0; 3], &options).unwrap();
    prestress_digest("strip unit", part.unit_prestress(), &mut out);
    match part.modes(300.0).unwrap() {
        SpeedModes::Modes(r) => out.push(("strip modes 300".into(), digest(r.eigenvalues.iter().copied()))),
        SpeedModes::LossOfStiffness(l) => panic!("strip at 300 rad/s: {l:?}"),
    }
    let j = mixed_joint();
    let s = mpc::solve_static_mpc(&j, &[], &[], &[], &StaticOptions::default()).unwrap();
    out.push((
        "mixed joint static".into(),
        digest(s.result.displacement_values().iter().copied()),
    ));
    out
}

const BASE: &[(&str, u64)] = &[
    ("beam prestress offsets", 0xb7f1764bfac16e29),
    ("beam prestress stress", 0xce4689322c37565a),
    ("shell4 prestress offsets", 0xe086e1a4001fa075),
    ("shell4 prestress stress", 0xb815ae1321b3c00b),
    ("shell3 prestress offsets", 0x7daaf8d9fa7e29f1),
    ("shell3 prestress stress", 0x2dbf3b07eee4a7c0),
    ("hex20 prestress offsets", 0x21a8ae1a231f9a33),
    ("hex20 prestress stress", 0x221a3c7985e6dc1e),
    ("hex8 thermal prestress offsets", 0xc83a2ef2454ebabc),
    ("hex8 thermal prestress stress", 0xa3e9694256c9b3c6),
    ("tet10 prestress offsets", 0x1d3e2e4f2965b916),
    ("tet10 prestress stress", 0x0c650fbe5f782a32),
    ("field positions", 0x5da6b2534040846a),
    ("field weights", 0xb426b0a7ca482735),
    ("field strain", 0x9c41a3af48fb06b4),
    ("field stress", 0xa3e9694256c9b3c6),
    ("field thermal", 0x65ed69d8153d963d),
    ("shell4 buckling factors", 0x0791250ad5ffecad),
    ("strip unit prestress offsets", 0x12158e408811e921),
    ("strip unit prestress stress", 0xbd962603e149d2ec),
    ("strip modes 300", 0xfe2127c9d378db67),
    ("mixed joint static", 0xf8e035472856641e),
];

#[test]
fn b0_default_bits_of_models_without_mixed_elements() {
    let got = words();
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    assert_eq!(got.len(), BASE.len(), "the record has {} words", BASE.len());
    for ((name, d), (bn, bd)) in got.iter().zip(BASE) {
        assert_eq!(name, bn);
        assert_eq!(*d, *bd, "{name}: 0x{d:016x} against the base 0x{bd:016x}");
    }
}
