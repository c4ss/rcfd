//! F-409 (ADR 0452): the bit record of the default paths that F-409 touches (check B1 of the registration).
//!
//! This file uses only the API of main 0ca10e83: it was run once in the worktree at that commit (no library line changed)
//! before any library code of F-409, and the digests it printed are the constants of `BASE`. After the change it runs
//! unchanged and compares. The models cover the static solve with a temperature field (thermal load and the recovery with
//! the thermal strain) on Hex8, Tet10 and Hex20, the Shell4 membrane thermal load of ADR 0445, the assembled stiffness
//! and the element thermal load.

use rcfd_core::vec3::Vec3;
use rcfd_fem::assemble::stiffness;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::hex20::{pressure_loads, upgrade_to_hex20_mapped};
use rcfd_fem::mesh::ElementKind;
use rcfd_fem::recover::{nodal_average, IntegrationPointField};
use rcfd_fem::solve::{solve_static, solve_static_with, StaticOptions};
use rcfd_fem::thermal::element_thermal_load;
use rcfd_fem::{BlockProperty, Constraint, Load, Material, SolidMesh, SolidModel};
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

fn flat(v: &[[f64; 6]]) -> Vec<f64> {
    v.iter().flatten().copied().collect()
}

fn steel(expansion: f64) -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, expansion).unwrap()
}

fn temperatures(mesh: &SolidMesh) -> Vec<f64> {
    mesh.nodes()
        .iter()
        .map(|x| 293.0 + 120.0 * x[0] + 35.0 * x[1] - 20.0 * x[2])
        .collect()
}

/// A solid model with a temperature field, a force and the xmin clamp: displacement, stress and thermal strain digests.
fn solid_digests(mesh: SolidMesh, tag: &str, out: &mut Vec<(String, u64)>) {
    let t = temperatures(&mesh);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(1.2e-5)).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m.add_load(Load::nodal_force("xmax", [0.0, 300.0, -100.0])).unwrap();
    m.add_load(Load::temperature(t, 293.0)).unwrap();
    let res = solve_static(&m, &StaticOptions::default()).unwrap();
    let field = IntegrationPointField::compute(&m, res.displacement_values()).unwrap();
    out.push((format!("{tag} displacement"), digest(res.displacement_values().iter().copied())));
    out.push((format!("{tag} stress"), digest(flat(field.stress()))));
    out.push((format!("{tag} thermal strain"), digest(flat(field.thermal_strain().unwrap()))));
    out.push((format!("{tag} energy"), digest([field.strain_energy()])));
}

fn all_digests() -> Vec<(String, u64)> {
    let mut out = Vec::new();
    // Hex8 cantilever 20 x 2 x 2 with a temperature field.
    solid_digests(hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap(), "hex8", &mut out);
    // Tet10 box with a temperature field.
    let tet = upgrade_to_tet10(&tet_box([0.0; 3], [1.0, 0.4, 0.3], [4, 2, 2]).unwrap()).unwrap().0;
    solid_digests(tet, "tet10", &mut out);
    // Hex20 quarter cylinder 4 x 8 under bore pressure (ADR 0292 T1).
    let param = hex_box([1.0, 0.0, 0.0], [1.0, PI / 2.0, 0.2], [4, 8, 1]).unwrap();
    let (mesh, _) = upgrade_to_hex20_mapped(&param, |q| [q[0] * q[1].cos(), q[0] * q[1].sin(), q[2]]).unwrap();
    let loads = pressure_loads(&mesh, "xmin", 1e8).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.0)).unwrap();
    for (set, c) in [("zmin", 2), ("zmax", 2), ("ymin", 1), ("ymax", 0)] {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
    let res = solve_static_with(&m, &[], &loads, &StaticOptions::default()).unwrap();
    let field = IntegrationPointField::compute(&m, res.displacement_values()).unwrap();
    let nodal = nodal_average(&m, &field, field.stress(), None).unwrap();
    out.push(("hex20 displacement".into(), digest(res.displacement_values().iter().copied())));
    out.push(("hex20 stress".into(), digest(flat(field.stress()))));
    out.push((
        "hex20 nodal stress".into(),
        digest(nodal.iter().flat_map(|s| s.unwrap_or([0.0; 6]))),
    ));
    // Hex8 cantilever stiffness.
    let mut c = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]).unwrap());
    c.add_material("all", steel(0.0)).unwrap();
    out.push(("hex8 stiffness".into(), digest(stiffness(&c).unwrap().val.iter().copied())));
    // The element thermal load of one distorted Hex8.
    let coords: Vec<Vec3> = vec![
        [0.0, 0.0, 0.0],
        [1.1, 0.05, 0.0],
        [1.0, 0.9, 0.1],
        [-0.05, 1.0, 0.0],
        [0.0, 0.1, 1.0],
        [1.0, 0.0, 1.1],
        [1.05, 1.0, 1.0],
        [0.1, 1.0, 0.95],
    ];
    let dte = [10.0, 20.0, 15.0, 5.0, 30.0, 25.0, 40.0, 35.0];
    let fe = element_thermal_load(ElementKind::Hex8, &coords, &steel(1.1e-5), &dte).unwrap();
    out.push(("hex8 element thermal load".into(), digest(fe)));
    // Shell4 plate 8 x 8 with a shell pressure and a temperature field (membrane thermal load, ADR 0445).
    let n = 8;
    let idx = |i: usize, j: usize| j * (n + 1) + i;
    let mut nodes = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            nodes.push([i as f64 / n as f64, j as f64 / n as f64, 0.0]);
        }
    }
    let mut conn = Vec::new();
    for j in 0..n {
        for i in 0..n {
            conn.extend([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    let mut pres = Vec::new();
    for (k, x) in nodes.iter().enumerate() {
        if x[0] == 0.0 {
            for comp in 0..6 {
                pres.push(Prescribed::new(k, comp, 0.0));
            }
        }
    }
    let t: Vec<f64> = nodes.iter().map(|x| 293.0 + 50.0 * x[0] + 10.0 * x[1]).collect();
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Shell4, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..n * n).collect()).unwrap();
    mesh.add_node_set("all", (0..nn).collect()).unwrap();
    let mut s = SolidModel::new(mesh);
    s.add_material("all", steel(1.2e-5)).unwrap();
    s.add_load(Load::shell_pressure("all", 1000.0)).unwrap();
    s.add_load(Load::temperature(t, 293.0)).unwrap();
    let res = solve_static_with(&s, &pres, &[], &StaticOptions::default()).unwrap();
    out.push(("shell4 displacement".into(), digest(res.displacement_values().iter().copied())));
    out
}

/// The digests printed by the run at main 0ca10e83 (`None` before that run).
const BASE: Option<&[(&str, u64)]> = Some(&[
    ("hex8 displacement", 0x184810507077be4c),
    ("hex8 stress", 0x923a9718a65be23c),
    ("hex8 thermal strain", 0xc1851055d0eebc19),
    ("hex8 energy", 0x9f0f8fc8cb1b3d1e),
    ("tet10 displacement", 0x2725e6371347d496),
    ("tet10 stress", 0x4e65257e53906449),
    ("tet10 thermal strain", 0x20b4852ef5bb73e6),
    ("tet10 energy", 0x34ee1bda7b333dda),
    ("hex20 displacement", 0xd1330e466a15a76c),
    ("hex20 stress", 0x3c3422033631b391),
    ("hex20 nodal stress", 0x2079d147bdaa4567),
    ("hex8 stiffness", 0x08aeab234e3188f7),
    ("hex8 element thermal load", 0xbc23a388c796c09e),
    ("shell4 displacement", 0xb1b7611f5aecdcfd),
]);

#[test]
fn b1_default_paths_bits() {
    let got = all_digests();
    for (name, d) in &got {
        println!("(\"{name}\", 0x{d:016x}),");
    }
    let base = BASE.expect("the record of main 0ca10e83 is filled in");
    let mut failed = Vec::new();
    assert_eq!(got.len(), base.len(), "number of digests");
    for ((name, d), (bname, bd)) in got.iter().zip(base) {
        let ok = name == bname && d == bd;
        println!("B1 {name}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            failed.push(name.clone());
        }
    }
    assert!(failed.is_empty(), "missed: {failed:?}");
}
