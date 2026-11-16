//! F-687 (ADR 0626): check B1b, the results of layup and Shell3 models without a through-thickness gradient are the bits
//! they were before F-687 (an unsymmetric cross-ply under a temperature field and gravity: load vector, solved displacement,
//! stresses, ply response; an isotropic bimetal: load and stresses; a Shell3 plate with the ANDES membrane: load and
//! stresses). Uses only API that existed at 9cf95fc3; the expected digests were printed by this file run on a copy of
//! 9cf95fc3 (`f687-scratch/b1b-base.log`) and written into `EXPECTED` before any library code of F-687.

use rcfd_core::vec3::{dot, norm, Vec3};
use rcfd_fem::laminate::layup_response;
use rcfd_fem::loads::nodal_load_vector;
use rcfd_fem::material::MaterialAxes;
use rcfd_fem::shell;
use rcfd_fem::shell3::Shell3Membrane;
use rcfd_fem::solve::{solve_static, StaticOptions};
use rcfd_fem::{BlockProperty, Constraint, ElementKind, Load, Material, Ply, SolidMesh, SolidModel};

/// Digests of 9cf95fc3 in the order of the run, from the base run (`f687-scratch/b1b-base.log`).
const EXPECTED: Option<[(&str, u64); 8]> = Some([
    ("cross-ply load", 0x1a72_7c3a_0700_5512),
    ("cross-ply displacement", 0x9361_801d_b26a_9b12),
    ("cross-ply stresses", 0x6ea4_4c59_35b7_caa0),
    ("cross-ply response", 0xc501_5c91_7645_51a1),
    ("bimetal load", 0x64a8_f58c_c48f_5354),
    ("bimetal stresses", 0x0c71_1798_1fd3_87a7),
    ("shell3 andes load", 0x3bbf_b371_ca2c_5617),
    ("shell3 andes stresses", 0x3aae_55ea_4840_8748),
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

fn steel() -> Material {
    Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 1.2e-5).unwrap()
}

fn aluminium() -> Material {
    Material::isotropic_elastic(7.0e10, 0.33, 2700.0, 2.3e-5).unwrap()
}

fn cf() -> Material {
    let axes = MaterialAxes::fixed([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    Material::orthotropic(
        [1.4e11, 1.0e10, 1.0e10],
        [0.3, 0.3, 0.45],
        [5.0e9, 5.0e9, 3.5e9],
        1600.0,
        [-0.5e-6, 3.0e-5, 3.0e-5],
        axes,
    )
    .unwrap()
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

/// The 4 x 3 grid on `[0, 2] x [0, 1]`; `distort` moves the interior nodes as the ADR 0445 TH plate.
fn plane(distort: bool) -> (Vec<[f64; 2]>, Vec<[usize; 4]>) {
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
    if distort {
        for j in 1..3 {
            for i in 1..4 {
                let k = j * 5 + i;
                p[k][0] += 0.06 * (1.3 * k as f64 + 0.2).sin();
                p[k][1] += 0.05 * (0.9 * k as f64 + 0.4).cos();
            }
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
    m.add_material("all", steel()).unwrap();
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

fn response_values(m: &SolidModel, u: &[f64]) -> Vec<f64> {
    let mut v = Vec::new();
    for r in layup_response(m, u).unwrap() {
        v.push(r.element as f64);
        v.extend(r.strain);
        v.extend(r.curvature);
        v.push(r.delta_t);
        for p in &r.plies {
            v.push(p.ply as f64);
            v.extend(p.z);
            v.extend(p.strain.iter().flatten());
            v.extend(p.stress.iter().flatten());
        }
    }
    v
}

fn fixed_displacement(m: &SolidModel) -> Vec<f64> {
    (0..m.dof_map().n_equations())
        .map(|i| 1e-6 * (0.37 * i as f64).sin())
        .collect()
}

#[test]
fn b1b_layup_and_shell3_models_without_a_gradient_keep_their_bits() {
    let mut got: Vec<(&str, u64)> = Vec::new();
    // (1) RG placed Q, the unsymmetric cross-ply [0/90], temperature and gravity, node 0 clamped.
    let (p, el) = plane(false);
    let conn: Vec<usize> = el.iter().flatten().copied().collect();
    let plies = vec![
        Ply::new(cf(), 0.05, 0.0).unwrap(),
        Ply::new(cf(), 0.05, std::f64::consts::FRAC_PI_2).unwrap(),
    ];
    let mut m = model(
        placed(&p),
        ElementKind::Shell4,
        conn,
        BlockProperty::layup(plies).unwrap(),
    );
    for comp in 0..6 {
        m.add_constraint(Constraint::displacement("origin", comp, 0.0)).unwrap();
    }
    m.add_load(temperature(&p)).unwrap();
    m.add_load(Load::gravity([0.0, -9.81, 2.0])).unwrap();
    got.push(("cross-ply load", digest(&nodal_load_vector(&m, &[]).unwrap())));
    let r = solve_static(&m, &StaticOptions::default()).unwrap();
    let u = r.displacement_values();
    got.push(("cross-ply displacement", digest(u)));
    got.push(("cross-ply stresses", digest(&stress_values(&m, u))));
    got.push(("cross-ply response", digest(&response_values(&m, u))));
    // (2) TH placed Q, the bimetal steel / aluminium, temperature only.
    let (p, el) = plane(true);
    let conn: Vec<usize> = el.iter().flatten().copied().collect();
    let plies = vec![
        Ply::new(steel(), 0.05, 0.0).unwrap(),
        Ply::new(aluminium(), 0.05, 0.0).unwrap(),
    ];
    let mut m = model(
        placed(&p),
        ElementKind::Shell4,
        conn,
        BlockProperty::layup(plies).unwrap(),
    );
    m.add_load(temperature(&p)).unwrap();
    got.push(("bimetal load", digest(&nodal_load_vector(&m, &[]).unwrap())));
    let u = fixed_displacement(&m);
    got.push(("bimetal stresses", digest(&stress_values(&m, &u))));
    // (3) TH3 placed Q, the ANDES membrane, temperature.
    let tris: Vec<usize> = el.iter().flat_map(|e| [e[0], e[1], e[2], e[0], e[2], e[3]]).collect();
    let mut m = model(
        placed(&p),
        ElementKind::Shell3,
        tris,
        BlockProperty::shell_with_membrane(0.05, Shell3Membrane::AndesOpt).unwrap(),
    );
    m.add_load(temperature(&p)).unwrap();
    got.push(("shell3 andes load", digest(&nodal_load_vector(&m, &[]).unwrap())));
    let u = fixed_displacement(&m);
    got.push(("shell3 andes stresses", digest(&stress_values(&m, &u))));
    for (name, d) in &got {
        println!("B1b {name}: 0x{d:016x}");
    }
    match EXPECTED {
        Some(expected) => {
            assert_eq!(got.len(), expected.len());
            for ((name, d), (en, ed)) in got.iter().zip(expected.iter()) {
                assert_eq!(name, en);
                assert_eq!(*d, *ed, "B1b {name}: 0x{d:016x} against the base 0x{ed:016x}");
            }
        }
        None => println!("B1b: no expected digests yet (the base run)"),
    }
}
