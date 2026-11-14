//! F-560 (ADR 0515): check B0, the bits of the J2 sphere and beam solves of ADR 0475 (S1, S2, S3 and the beams at kappa = 1.25
//! and 5 kappa_y) with the default options. Uses only API that existed at the base d41012db; the expected digests were
//! printed by this file run on the unchanged base (`f560-scratch/b0-base.log`) and written into `EXPECTED` before any library
//! code of F-560. B1 of `sa_j2_order.rs` compares the solves with the active-set record switched on against the same digests.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::Prescribed;
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::mesh::FaceRef;
use rcfd_fem::nonlinear::{solve, NewtonOptions, NonlinearResult};
use rcfd_fem::{Constraint, ElementKind, Load, Material, SolidMesh, SolidModel};
use std::collections::HashMap;

const E_MOD: f64 = 2.0e11;
const NU: f64 = 0.3;
const SY: f64 = 2.0e8;
const RHO: f64 = 7850.0;

fn j2(h_iso: f64, h_kin: f64) -> Material {
    Material::j2(
        Material::isotropic_elastic(E_MOD, NU, RHO, 0.0).unwrap(),
        SY,
        h_iso,
        h_kin,
    )
    .unwrap()
}

const SPHERE_N: usize = 2;
const SPHERE_M: usize = 8;
const ELEMENTS_PER_LAYER: usize = 3 * SPHERE_N * SPHERE_N;

fn p_of_c(c: f64) -> f64 {
    2.0 * c.ln() + 2.0 / 3.0 * (1.0 - c * c * c / 8.0)
}

/// The octant of the sphere `a = 1`, `b = 2` of D5 (cubed sphere, 2 x 2 cells per face, 8 radial layers, Hex20 on the
/// spheres), element `layer * 12 + cell`; with the bore pressure `p` (Pa) if given.
fn sphere(mat: Material, p: Option<f64>) -> SolidModel {
    let (n, m) = (SPHERE_N, SPHERE_M);
    let mut index: HashMap<([usize; 3], usize), usize> = HashMap::new();
    let mut params: Vec<Vec3> = Vec::new();
    let mut node = |q: [usize; 3], k: usize, params: &mut Vec<Vec3>| -> usize {
        *index.entry((q, k)).or_insert_with(|| {
            let r = 1.0 + k as f64 / m as f64;
            params.push([
                r * q[0] as f64 / n as f64,
                r * q[1] as f64 / n as f64,
                r * q[2] as f64 / n as f64,
            ]);
            params.len() - 1
        })
    };
    let mut conn = Vec::new();
    for k in 0..m {
        for a in 0..3 {
            for j in 0..n {
                for i in 0..n {
                    let q = |i: usize, j: usize| {
                        let mut v = [0; 3];
                        v[a] = n;
                        v[(a + 1) % 3] = i;
                        v[(a + 2) % 3] = j;
                        v
                    };
                    for kk in [k, k + 1] {
                        for (ii, jj) in [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)] {
                            let id = node(q(ii, jj), kk, &mut params);
                            conn.push(id);
                        }
                    }
                }
            }
        }
    }
    let nn = params.len();
    let mut mesh = SolidMesh::new(params).unwrap();
    mesh.add_block(ElementKind::Hex8, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        let set: Vec<usize> = (0..nn).filter(|&v| mesh.nodes()[v][c] == 0.0).collect();
        mesh.add_node_set(name, set).unwrap();
    }
    mesh.add_surface_set(
        "bore",
        (0..ELEMENTS_PER_LAYER)
            .map(|e| FaceRef { element: e, face: 0 })
            .collect(),
    )
    .unwrap();
    let mesh = upgrade_to_hex20_mapped(&mesh, |p| {
        let inf = p[0].abs().max(p[1].abs()).max(p[2].abs());
        let two = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        [p[0] * inf / two, p[1] * inf / two, p[2] * inf / two]
    })
    .unwrap()
    .0;
    let mut model = SolidModel::new(mesh);
    model.add_material("all", mat).unwrap();
    for (c, name) in ["x0", "y0", "z0"].iter().enumerate() {
        model.add_constraint(Constraint::displacement(name, c, 0.0)).unwrap();
    }
    if let Some(p) = p {
        model.add_load(Load::pressure("bore", p)).unwrap();
    }
    model
}

const BEAM_L: f64 = 0.02;
const BEAM_H: f64 = 0.1;
const BEAM_W: f64 = 0.01;

fn kappa_y() -> f64 {
    2.0 * SY / (E_MOD * BEAM_H)
}

/// The narrow beam of D6 and the prescribed displacements of the curvature `kappa`.
fn beam(kappa: f64) -> (SolidModel, Vec<Prescribed>) {
    let mesh = upgrade_to_hex20(
        &hex_box(
            [0.0, -BEAM_H / 2.0, -BEAM_W / 2.0],
            [BEAM_L, BEAM_H, BEAM_W],
            [1, 16, 1],
        )
        .unwrap(),
    )
    .unwrap()
    .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", j2(0.0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("xmin", 0, 0.0)).unwrap();
    let nodes = m.mesh().nodes().to_vec();
    let at = |p: Vec3| {
        nodes
            .iter()
            .position(|q| (q[0] - p[0]).abs() < 1e-12 && (q[1] - p[1]).abs() < 1e-12 && (q[2] - p[2]).abs() < 1e-12)
            .expect("node exists")
    };
    let mut pres: Vec<Prescribed> = m
        .mesh()
        .node_set("xmax")
        .unwrap()
        .iter()
        .map(|&n| Prescribed::new(n, 0, -kappa * BEAM_L * nodes[n][1]))
        .collect();
    let o = at([0.0, 0.0, 0.0]);
    let top = at([0.0, BEAM_H / 2.0, 0.0]);
    pres.push(Prescribed::new(o, 1, 0.0));
    pres.push(Prescribed::new(o, 2, 0.0));
    pres.push(Prescribed::new(top, 2, 0.0));
    (m, pres)
}

/// FNV-1a 64 over the little-endian bytes of the bits of each value.
fn digest(v: &[f64]) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325;
    for b in v.iter().flat_map(|x| x.to_bits().to_le_bytes()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Every value of a nonlinear result in a fixed order: displacement, reactions, per increment the step, load factor,
/// halvings, residuals and energies, then the state values.
fn flat(r: &NonlinearResult) -> Vec<f64> {
    let mut v = r.result.displacement_values().to_vec();
    v.extend_from_slice(r.result.reactions().unwrap_or(&[]));
    for s in &r.steps {
        v.push(s.load_factor);
        v.push(s.step as f64);
        v.push(s.halvings as f64);
        v.extend_from_slice(&s.residuals);
        v.extend_from_slice(&s.energies);
    }
    for s in &r.states {
        v.push(s.values().len() as f64);
        v.extend_from_slice(s.values());
    }
    v
}

fn solves() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();
    for (id, cc, steps) in [("S1", 1.25, 8usize), ("S2", 1.5, 10)] {
        let model = sphere(j2(0.0, 0.0), Some(p_of_c(cc) * SY));
        let r = solve(&model, &[], &[], &NewtonOptions::new(steps)).unwrap();
        out.push((id, digest(&flat(&r))));
    }
    let model = sphere(j2(0.0, 0.0), Some(0.98 * 2.0 * 2.0f64.ln() * SY));
    out.push((
        "S3",
        digest(&flat(&solve(&model, &[], &[], &NewtonOptions::new(20)).unwrap())),
    ));
    for (id, k) in [("P1 1.25", 1.25), ("P1 5", 5.0)] {
        let (m, pres) = beam(k * kappa_y());
        let r = solve(&m, &pres, &[], &NewtonOptions::new((4.0 * k).ceil() as usize)).unwrap();
        out.push((id, digest(&flat(&r))));
    }
    out
}

const EXPECTED: Option<[(&str, u64); 5]> = Some([
    ("S1", 0xfa47_eed3_8c0f_def8),
    ("S2", 0x3425_56fa_7f12_f899),
    ("S3", 0x54fd_cd5c_d089_c65b),
    ("P1 1.25", 0x1c36_1cb4_7319_fc47),
    ("P1 5", 0x8855_6e1f_cea7_bf0f),
]);

#[test]
fn b0_j2_sphere_and_beam_default_bits() {
    let got = solves();
    for (id, d) in &got {
        println!("{id}: {d:#018x}");
    }
    let expected = EXPECTED.expect("the digests of the base are not written yet");
    for ((id, d), (eid, ed)) in got.iter().zip(expected) {
        assert_eq!(*id, eid);
        assert_eq!(*d, ed, "{id}: the default bits changed");
    }
}
