//! F-527 (ADR 0612): check B1, the results of models without a through-thickness gradient are the bits they were before
//! F-527 (a Shell4 plate under a temperature field and gravity: load vector, solved displacement, stresses; the same plate as
//! a one-ply Layup; a Hex8 box; a Shell3 plate). Uses only API that existed at 2bc0141c; the expected digests were printed by
//! this file run on a copy of 2bc0141c (`f527-scratch/b1-base.log`) and written into `EXPECTED` before any library code of
//! F-527.

use rcfd_core::vec3::{dot, norm, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::shell;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, Ply, SolidMesh, SolidModel};

/// Digests of 2bc0141c in the order of the run, from the base run (`f527-scratch/b1-base.log`).
const EXPECTED: Option<[(&str, u64); 7]> = Some([
    ("shell4 load", 0x8e4f_fdf3_79bf_9113),
    ("shell4 displacement", 0x4c04_e131_2621_a65e),
    ("shell4 stresses", 0xfb6d_7eaa_7cfc_529c),
    ("layup load", 0xef55_03ab_5852_5f49),
    ("hex8 load", 0xab6f_6039_81c2_6e2e),
    ("shell3 load", 0xa756_8f92_906d_f2b9),
    ("shell3 stresses", 0x946a_3cc6_6754_2faf),
]);

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for x in v {
        for b in x.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h
}

fn material() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn rotation(axis: Vec3, angle: f64) -> [[f64; 3]; 3] {
    let n = norm(axis);
    let a = [axis[0] / n, axis[1] / n, axis[2] / n];
    let (s, c) = angle.sin_cos();
    let k = [[0.0, -a[2], a[1]], [a[2], 0.0, -a[0]], [-a[1], a[0], 0.0]];
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let kk: f64 = (0..3).map(|m| k[i][m] * k[m][j]).sum();
            r[i][j] = f64::from(u8::from(i == j)) + s * k[i][j] + (1.0 - c) * kk;
        }
    }
    r
}

/// The ADR 0445 TH plate in plane coordinates (4 x 3 on `[0, 2] x [0, 1]`, interior nodes moved).
fn plane() -> (Vec<[f64; 2]>, Vec<[usize; 4]>) {
    let (nx, ny) = (4, 3);
    let idx = |i: usize, j: usize| j * (nx + 1) + i;
    let mut p = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            p.push([2.0 * i as f64 / nx as f64, j as f64 / ny as f64]);
        }
    }
    let mut el = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            el.push([idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1)]);
        }
    }
    for j in 1..3 {
        for i in 1..4 {
            let k = j * 5 + i;
            p[k][0] += 0.06 * (1.3 * k as f64 + 0.2).sin();
            p[k][1] += 0.05 * (0.9 * k as f64 + 0.4).cos();
        }
    }
    (p, el)
}

fn placed(p: &[[f64; 2]]) -> Vec<Vec3> {
    let q = rotation([1.0, 2.0, 3.0], 0.7);
    let d = [0.3, -0.2, 0.5];
    p.iter()
        .map(|x| {
            let v = [x[0], x[1], 0.0];
            [dot(q[0], v) + d[0], dot(q[1], v) + d[1], dot(q[2], v) + d[2]]
        })
        .collect()
}

fn model(nodes: Vec<Vec3>, kind: ElementKind, conn: Vec<usize>, property: BlockProperty) -> SolidModel {
    let n = nodes.len();
    let per = if kind == ElementKind::Shell3 { 3 } else { 4 };
    let ne = conn.len() / per;
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(kind, conn).unwrap();
    mesh.set_block_property(0, property).unwrap();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    mesh.add_node_set("all", (0..n).collect()).unwrap();
    mesh.add_node_set("origin", vec![0]).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m
}

fn temperature(p: &[[f64; 2]]) -> Load {
    Load::temperature(p.iter().map(|q| 300.0 + 40.0 * q[0] - 25.0 * q[1]).collect(), 290.0)
}

fn stress_values(m: &SolidModel, u: &[f64]) -> Vec<f64> {
    let mut v = Vec::new();
    for (_, s) in shell::stresses(m, u).unwrap() {
        v.extend(s.axes.iter().flatten());
        v.extend(s.membrane);
        v.extend(s.bending);
        v.extend(s.shear);
        v.extend(s.top);
        v.extend(s.bottom);
    }
    v
}

#[test]
fn b1_models_without_a_gradient_keep_their_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    let (p, el) = plane();
    let conn: Vec<usize> = el.iter().flatten().copied().collect();
    // (1) Shell4 plate Q, t = 0.05, temperature and gravity, node 0 clamped.
    let mut m = model(
        placed(&p),
        ElementKind::Shell4,
        conn.clone(),
        BlockProperty::shell(0.05).unwrap(),
    );
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("origin", comp, 0.0)).unwrap();
    }
    m.add_load(temperature(&p)).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, 2.0])).unwrap();
    got.push(("shell4 load", digest(&nodal_load_vector(&m, &[]).unwrap())));
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    let u = r.displacement_values();
    got.push(("shell4 displacement", digest(u)));
    got.push(("shell4 stresses", digest(&stress_values(&m, u))));
    // (2) the same mesh as a one-ply isotropic Layup, temperature only.
    let ply = Ply::new(material(), 0.05, 0.0).unwrap();
    let mut m = model(
        placed(&p),
        ElementKind::Shell4,
        conn,
        BlockProperty::layup(vec![ply]).unwrap(),
    );
    m.add_load(temperature(&p)).unwrap();
    got.push(("layup load", digest(&nodal_load_vector(&m, &[]).unwrap())));
    // (3) a Hex8 box with a temperature field.
    let mesh = hex_box([0.0; 3], [2.0, 1.0, 1.0], [2, 1, 1]).unwrap();
    let t: Vec<f64> = mesh.nodes().iter().map(|x| 300.0 + 10.0 * x[0] + 5.0 * x[1] * x[2]).collect();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_load(Load::temperature(t, 290.0)).unwrap();
    got.push(("hex8 load", digest(&nodal_load_vector(&m, &[]).unwrap())));
    // (4) a Shell3 plate: each quadrilateral cut on its 0-2 diagonal, the plate-Q temperature.
    let tris: Vec<usize> = el.iter().flat_map(|e| [e[0], e[1], e[2], e[0], e[2], e[3]]).collect();
    let mut m = model(placed(&p), ElementKind::Shell3, tris, BlockProperty::shell(0.05).unwrap());
    m.add_load(temperature(&p)).unwrap();
    got.push(("shell3 load", digest(&nodal_load_vector(&m, &[]).unwrap())));
    let u: Vec<f64> = (0..m.dof_map().n_equations()).map(|i| 1e-6 * (0.37 * i as f64).sin()).collect();
    got.push(("shell3 stresses", digest(&stress_values(&m, &u))));
    for (name, d) in &got {
        println!("B1 {name}: 0x{d:016x}");
    }
    match EXPECTED {
        Some(expected) => {
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                assert_eq!(name, en);
                assert_eq!(*d, *ed, "B1 {name}: 0x{d:016x} against the base 0x{ed:016x}");
            }
        }
        None => println!("B1: no expected digests yet (the base run)"),
    }
}
