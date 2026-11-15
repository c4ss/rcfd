//! F-572 (ADR 0560): check N1 (record of D7), the symmetric path and the default arc-length path give the bits they gave
//! before F-572: the ring under the follower pressure by `solve_with`, the free-edged plate under the follower pressure by
//! `solve_with`, the two-bar truss by `trace`, the ring with a dead pressure and a follower contribution by `trace`, the
//! zero-load refusal of `trace`, and the assembled tangent with the follower's triplets. Uses only API that existed at
//! 3378a508; the expected digests were printed by this file run on the unchanged base 3378a508 (`f572-scratch/n1-base.log`)
//! and written into `EXPECTED` before any library code of F-572.

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace, ArcLengthOptions, ArcLengthPath, PathPoint};
use rcfd_fem::builders::hex_box;
use rcfd_fem::follower::FollowerPressure;
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::nonlinear::{
    initial_states, solve_with, tangent, Contribution, DriverExtensions, NewtonOptions, NonlinearResult,
};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

/// Digests of 3378a508 in the order of the run, from the base run (`f572-scratch/n1-base.log`).
const EXPECTED: Option<[(&str, u64); 7]> = Some([
    ("ring follower solve_with", 0xfe2d_a70a_a3cc_8a67),
    ("plate follower solve_with", 0x217c_a534_6940_57e6),
    ("truss trace", 0xea15_cefa_bd23_44c3),
    ("ring mixed trace", 0x3d84_53e1_c6de_65f3),
    ("zero-load refusal", 0x015d_1a15_38c4_321c),
    ("tangent with follower triplets", 0xedce_9bb3_16af_1cbc),
    ("count", 0x0000_0000_0000_0006),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    digest_bytes(v.iter().flat_map(|x| x.to_bits().to_le_bytes()))
}

fn digest_bytes<I: IntoIterator<Item = u8>>(bytes: I) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

const E: f64 = 2.0e11;

fn material() -> Material {
    Material::isotropic_elastic(E, 0.0, 7850.0, 0.0).unwrap()
}

/// Every value of a nonlinear result, in a fixed order.
fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap());
    for s in &r.steps {
        v.push(s.load_factor);
        v.push(s.step as f64);
        v.push(s.halvings as f64);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    v
}

/// Every value of an arc-length path, in a fixed order.
fn flat_path(p: &ArcLengthPath) -> Vec<f64> {
    let mut v = Vec::new();
    for q in &p.points {
        v.push(q.load_factor);
        v.extend_from_slice(&q.displacement);
        v.push(q.length);
        v.push(q.halvings as f64);
        v.push(q.negative_pivots as f64);
        v.extend_from_slice(&q.residuals);
        v.extend_from_slice(&q.energies);
    }
    v.push(p.load_scale);
    v.push(if p.stopped { 1.0 } else { 0.0 });
    v
}

const R: f64 = 1.0;
const T: f64 = 0.02;
const BW: f64 = 0.02;

/// The quarter ring of ADR 0480 D5.
fn ring_model(n: usize) -> SolidModel {
    let b = hex_box([R - T / 2.0, 0.0, 0.0], [T, PI / 2.0, BW], [1, n, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m
}

fn p_ref_ring() -> f64 {
    E * T * (1.51f64 * 1.51 - 1.0) / (2.0 * R)
}

/// The plate of ADR 0560 D5.
fn plate() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.05], [10, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&mesh).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

fn p_plate() -> f64 {
    E * 0.05f64.powi(3) / 2.0
}

const L0: f64 = 1.0;
const H: f64 = 0.2;
const S: f64 = 0.1;
const APEX: usize = 2;

fn volume(p: &[Vec3; 4]) -> f64 {
    let d = |a: Vec3, b: Vec3| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let (u, v, w) = (d(p[0], p[1]), d(p[0], p[2]), d(p[0], p[3]));
    u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0]) + u[2] * (v[0] * w[1] - v[1] * w[0])
}

/// The two-bar truss of ADR 0480 D6.
fn truss() -> SolidModel {
    let a = (L0 * L0 - H * H).sqrt();
    let apex = [0.0, H, 0.0];
    let mut nodes: Vec<Vec3> = vec![[-a, 0.0, 0.0], [a, 0.0, 0.0], apex];
    let mut conn = Vec::new();
    for b in 0..2usize {
        let pb = nodes[b];
        let a0 = [(apex[0] - pb[0]) / L0, (apex[1] - pb[1]) / L0];
        let n0 = [-a0[1], a0[0]];
        let m = [(pb[0] + apex[0]) / 2.0, (pb[1] + apex[1]) / 2.0];
        let first = nodes.len();
        for (cn, cz) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            nodes.push([m[0] + cn * S / 2.0 * n0[0], m[1] + cn * S / 2.0 * n0[1], cz * S / 2.0]);
        }
        for i in 0..4 {
            let mut t = [b, APEX, first + i, first + (i + 1) % 4];
            if volume(&[nodes[t[0]], nodes[t[1]], nodes[t[2]], nodes[t[3]]]) < 0.0 {
                t.swap(2, 3);
            }
            conn.extend_from_slice(&t);
        }
    }
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Tet4, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("pins", vec![0, 1]).unwrap();
    mesh.add_node_set("every", (0..nn).collect()).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("pins", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pins", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m
}

fn record() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    // (1) the ring under the follower pressure, 4 steps.
    {
        let model = ring_model(8);
        let fp = FollowerPressure::new(&model, "xmin", p_ref_ring()).unwrap();
        let ext = DriverExtensions {
            contributions: vec![&fp],
            step_control: None,
        };
        let r = solve_with(&model, &[], &[], &NewtonOptions::new(4), ext).unwrap();
        out.push(("ring follower solve_with", digest(&flat(&r))));
    }
    // (2) the plate under the follower pressure, half the pressure, 2 steps.
    {
        let model = plate();
        let fp = FollowerPressure::new(&model, "zmax", 0.5 * p_plate()).unwrap();
        let ext = DriverExtensions {
            contributions: vec![&fp],
            step_control: None,
        };
        let r = solve_with(&model, &[], &[], &NewtonOptions::new(2), ext).unwrap();
        out.push(("plate follower solve_with", digest(&flat(&r))));
    }
    // (3) the two-bar truss by trace, 12 points.
    {
        let model = truss();
        let pm = 2.0 * E * (S * S / 3.0) * H.powi(3) / (3.0 * 3f64.sqrt() * L0.powi(3));
        let forces = [NodalForceAt::new(APEX, [0.0, -pm, 0.0])];
        let options = ArcLengthOptions::new(0.05 * H).max_points(12);
        let path = trace(&model, &[], &forces, &options, &[], &mut |_: &PathPoint| false).unwrap();
        out.push(("truss trace", digest(&flat_path(&path))));
    }
    // (4) the ring with a dead pressure and a follower contribution by trace, 6 points.
    {
        let mut model = ring_model(8);
        model.add_load(Load::pressure("xmin", p_ref_ring())).unwrap();
        let fp = FollowerPressure::new(&model, "xmin", 0.1 * p_ref_ring()).unwrap();
        let options = ArcLengthOptions::new(0.1).max_points(6);
        let contributions: Vec<&dyn Contribution> = vec![&fp];
        let path = trace(&model, &[], &[], &options, &contributions, &mut |_: &PathPoint| false).unwrap();
        out.push(("ring mixed trace", digest(&flat_path(&path))));
    }
    // (5) the zero-load refusal of trace with the follower pressure alone.
    {
        let model = ring_model(8);
        let fp = FollowerPressure::new(&model, "xmin", p_ref_ring()).unwrap();
        let contributions: Vec<&dyn Contribution> = vec![&fp];
        let e = trace(
            &model,
            &[],
            &[],
            &ArcLengthOptions::new(0.1),
            &contributions,
            &mut |_: &PathPoint| false,
        )
        .unwrap_err()
        .to_string();
        println!("refusal: {e}");
        out.push(("zero-load refusal", digest_bytes(e.into_bytes())));
    }
    // (6) the tangent with the follower's triplets on the ring at a deformed state.
    {
        let model = ring_model(8);
        let fp = FollowerPressure::new(&model, "xmin", p_ref_ring()).unwrap();
        let n = model.dof_map().n_equations();
        let u: Vec<f64> = (0..n).map(|i| 1e-3 * ((i as f64) * 0.37).sin()).collect();
        let states = initial_states(&model).unwrap();
        let mut k = tangent(&model, &u, &states, Some(1)).unwrap();
        let mut force = vec![0.0; n];
        let mut triplets = Vec::new();
        fp.add(&u, 0.7, &mut force, &mut triplets).unwrap();
        rcfd_fem::nonlinear::add_triplets(&mut k, &triplets);
        let mut v = k.val.clone();
        v.extend(k.col.iter().map(|&c| c as f64));
        v.extend(k.row_ptr.iter().map(|&c| c as f64));
        v.extend_from_slice(&force);
        out.push(("tangent with follower triplets", digest(&v)));
    }
    out.push(("count", out.len() as u64));
    out
}

#[test]
fn n1_symmetric_and_default_arc_length_paths_keep_their_bits() {
    let got = record();
    for (name, d) in &got {
        println!(
            "(\"{name}\", 0x{:04x}_{:04x}_{:04x}_{:04x}),",
            d >> 48,
            (d >> 32) & 0xffff,
            (d >> 16) & 0xffff,
            d & 0xffff
        );
    }
    match EXPECTED {
        None => panic!("N1: no expected digests yet (the base run prints them)"),
        Some(expected) => {
            assert_eq!(got.len(), expected.len(), "N1: number of records");
            for ((n, g), (en, e)) in got.iter().zip(expected.iter()) {
                assert_eq!(n, en, "N1: record order");
                assert_eq!(g, e, "N1: digest of '{n}' changed");
            }
        }
    }
}
