//! F-578 (ADR 0490): check N1 (record of D7), the solid nonlinear paths and the linear shell paths give the bits they gave
//! before F-578: `nonlinear::solve` of a Hex8 cantilever under a tip traction, `internal_force` and `tangent` of a Tet10 and
//! a Hex20 block at a deformed state, `solve_with` of a Hex8 column with the solid `FollowerPressure`, a J2 bar by
//! `nonlinear::solve`, `arclength::trace` of the Hex8 column, `solve_static` of a Shell4 and a Shell3 plate under a shell
//! pressure and a nodal moment. Uses only API that existed at e90c2350; the expected digests were printed by this file run on
//! the unchanged base e90c2350 (`f578-scratch/n1-base.log`) and written into `EXPECTED` before any library code of F-578.

use rcfd_fem::arclength::{trace, ArcLengthOptions, PathPoint};
use rcfd_fem::builders::{hex_box, upgrade_to_tet10};
use rcfd_fem::follower::FollowerPressure;
use rcfd_fem::hex20::upgrade_to_hex20;
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve, solve_with, tangent, DriverExtensions, NewtonOptions,
};
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};

/// Digests of e90c2350 in the order of the run, from the base run (`f578-scratch/n1-base.log`).
const EXPECTED: Option<[(&str, u64); 9]> = Some([
    ("Hex8 cantilever solve", 0x28bf_09e0_afd4_510b),
    ("Tet10 force and tangent", 0x3179_822d_4c9a_e4ea),
    ("Hex20 force and tangent", 0x047e_a775_eb1e_4a40),
    ("Hex8 column follower solve_with", 0x55ec_cb90_7957_b863),
    ("J2 bar solve", 0xe58a_d8f6_a71a_ef08),
    ("Hex8 column arc length", 0xeb5e_5948_2059_0e0f),
    ("Shell4 plate static", 0xc8ae_5211_6d43_07ad),
    ("Shell3 plate static", 0x49f8_55ee_e8a4_6c6c),
    ("count", 0x0000_0000_0000_0008),
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

fn nonlinear_values(r: &rcfd_fem::nonlinear::NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap());
    for s in &r.steps {
        v.push(s.load_factor);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    for s in &r.states {
        v.extend_from_slice(s.values());
    }
    v
}

fn column() -> SolidModel {
    let mut mesh = hex_box([0.0; 3], [0.1, 0.1, 1.0], [1, 1, 4]).unwrap();
    mesh.add_surface_set("top", mesh.surface_set("zmax").unwrap().to_vec())
        .unwrap();
    let mut col = SolidModel::new(mesh);
    col.add_material("all", steel(0.0)).unwrap();
    for c in 0..3 {
        col.add_constraint(Constraint::displacement("zmin", c, 0.0)).unwrap();
    }
    col
}

fn plate(kind: ElementKind) -> SolidModel {
    let mut nodes = Vec::new();
    for j in 0..=3 {
        for i in 0..=3 {
            nodes.push([i as f64 / 3.0, j as f64 / 3.0, 0.02 * (i * j) as f64]);
        }
    }
    let idx = |i: usize, j: usize| j * 4 + i;
    let mut conn = Vec::new();
    for j in 0..3 {
        for i in 0..3 {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            if kind == ElementKind::Shell3 {
                conn.extend_from_slice(&[a, b, c, a, c, d]);
            } else {
                conn.extend_from_slice(&[a, b, c, d]);
            }
        }
    }
    let ne = if kind == ElementKind::Shell3 { 18 } else { 9 };
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, BlockProperty::shell(0.01).unwrap()).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("x0", (0..=3).map(|j| idx(0, j)).collect()).unwrap();
    mesh.add_node_set("x1", (0..=3).map(|j| idx(3, j)).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", steel(0.3)).unwrap();
    for c in 0..6 {
        m.add_constraint(Constraint::displacement("x0", c, 0.0)).unwrap();
    }
    m.add_load(Load::shell_pressure("all", 1.0e3)).unwrap();
    m.add_load(Load::nodal_moment("x1", [0.3, -2.0, 0.1])).unwrap();
    m
}

#[test]
fn n1_default_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    // A Hex8 cantilever under a tip traction, three load steps.
    let mut cant = SolidModel::new(hex_box([0.0; 3], [1.0, 0.05, 0.05], [10, 1, 1]).unwrap());
    cant.add_material("all", steel(0.0)).unwrap();
    for c in 0..3 {
        cant.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    cant.add_load(Load::traction("xmax", [0.0, 4.0e6, 0.0])).unwrap();
    let r = solve(&cant, &[], &[], &NewtonOptions::new(3)).unwrap();
    got.push(("Hex8 cantilever solve", digest(&nonlinear_values(&r))));
    // The assembled force and tangent of a Tet10 and a Hex20 block at a deformed state.
    let tet = upgrade_to_tet10(&rcfd_fem::builders::tet_box([0.0; 3], [1.0, 0.5, 0.4], [2, 1, 1]).unwrap())
        .unwrap()
        .0;
    let hex = upgrade_to_hex20(&hex_box([0.0; 3], [1.0, 0.5, 0.4], [2, 1, 1]).unwrap())
        .unwrap()
        .0;
    for (name, mesh) in [("Tet10 force and tangent", tet), ("Hex20 force and tangent", hex)] {
        let mut m = SolidModel::new(mesh);
        m.add_material("all", steel(0.3)).unwrap();
        let n = m.dof_map().n_equations();
        let u: Vec<f64> = (0..n).map(|i| 0.02 * ((i as f64) * 0.37).sin()).collect();
        let states = initial_states(&m).unwrap();
        let mut v = internal_force(&m, &u, &states).unwrap();
        v.extend_from_slice(&tangent(&m, &u, &states, Some(2)).unwrap().val);
        got.push((name, digest(&v)));
    }
    // solve_with of the Hex8 column under the solid follower pressure.
    let col = column();
    let fp = FollowerPressure::new(&col, "top", 2.0e8).unwrap();
    let ext = DriverExtensions {
        contributions: vec![&fp],
        step_control: None,
    };
    let r = solve_with(&col, &[], &[], &NewtonOptions::new(2), ext).unwrap();
    got.push(("Hex8 column follower solve_with", digest(&nonlinear_values(&r))));
    // A J2 bar stretched beyond yield by a prescribed end displacement.
    let mut bar = SolidModel::new(hex_box([0.0; 3], [1.0, 0.1, 0.1], [4, 1, 1]).unwrap());
    let j2 = Material::j2(steel(0.3), 2.5e8, 1.0e9, 5.0e8).unwrap();
    bar.add_material("all", j2).unwrap();
    bar.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("zmin", 2, 0.0)).unwrap();
    bar.add_constraint(Constraint::displacement("xmax", 0, 4.0e-3)).unwrap();
    let r = solve(&bar, &[], &[], &NewtonOptions::new(4)).unwrap();
    got.push(("J2 bar solve", digest(&nonlinear_values(&r))));
    // The arc-length trace of the Hex8 column under an end force, three points.
    let mut col = column();
    col.add_load(Load::traction("top", [1.0e6, 0.0, -1.0e7])).unwrap();
    let mut count = 0;
    let mut stop = |_: &PathPoint| {
        count += 1;
        count >= 3
    };
    let path = trace(&col, &[], &[], &ArcLengthOptions::new(1e-3), &[], &mut stop).unwrap();
    let mut v = Vec::new();
    for p in &path.points {
        v.push(p.load_factor);
        v.extend_from_slice(&p.displacement);
        v.extend_from_slice(&p.residuals);
    }
    got.push(("Hex8 column arc length", digest(&v)));
    // Linear statics of shell plates under a shell pressure and a nodal moment.
    for (name, kind) in [
        ("Shell4 plate static", ElementKind::Shell4),
        ("Shell3 plate static", ElementKind::Shell3),
    ] {
        let m = plate(kind);
        let s = solve_static(&m, &StaticOptions::default()).unwrap();
        let mut v = s.displacement_values().to_vec();
        v.extend_from_slice(s.reactions().unwrap());
        got.push((name, digest(&v)));
    }
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
