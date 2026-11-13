//! F-484 (ADR 0383): Hex20 face loads through the model path. Checks A1 to A7, K1, K2a, K2b and the negative controls N1 to
//! N3 of the registration (A8 is `rcfd-api`'s `baseline_hashes`).

use rcfd_core::vec3::{cross, norm, sub, Vec3};
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10, BOX_FACES};
use rcfd_fem::hex20::{self, pressure_loads, traction_loads, upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::{face_rule, face_shape_functions, nodal_load_vector};
use rcfd_fem::mesh::FaceRef;
use rcfd_fem::{Load, Material, SolidMesh, SolidModel};
use std::f64::consts::PI;

const P: f64 = 1.3e5;
const T: Vec3 = [2.0e4, -3.5e4, 1.1e4];

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.4e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} > {limit:e}"));
        }
    }
    fn equals(&mut self, name: &str, value: usize, expected: usize) {
        let ok = value == expected;
        println!(
            "{name}: {value} (expected {expected}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value} != {expected}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut m = SolidModel::new(mesh);
    m.add_material("all", Material::isotropic_elastic(2.0e11, 0.3, 7850.0, 0.0).unwrap())
        .unwrap();
    m
}

fn polar_warp(x: Vec3) -> Vec3 {
    let (r, t, z) = (x[0], x[1], x[2]);
    [r * t.cos(), r * t.sin(), z + 0.1 * r * t.sin()]
}

/// Set-up B (flat faces).
fn mesh_b() -> SolidMesh {
    upgrade_to_hex20(&hex_box([0.0; 3], [0.3, 0.5, 0.7], [2, 2, 2]).unwrap())
        .unwrap()
        .0
}

/// Set-up C (curved faces).
fn mesh_c() -> SolidMesh {
    let base = hex_box([1.0, 0.0, 0.0], [1.0, PI / 2.0, 0.2], [2, 3, 1]).unwrap();
    upgrade_to_hex20_mapped(&base, polar_warp).unwrap().0
}

/// Set-up F (every face a parallelogram).
fn mesh_f() -> SolidMesh {
    let base = hex_box([0.0; 3], [1.0; 3], [2, 1, 1]).unwrap();
    upgrade_to_hex20_mapped(&base, |x| {
        [
            x[0] + 0.3 * x[1] + 0.1 * x[2],
            0.9 * x[1] + 0.2 * x[2],
            1.1 * x[2] + 0.05 * x[0],
        ]
    })
    .unwrap()
    .0
}

/// The same mesh with every node moved by `map`.
fn rebuild(mesh: &SolidMesh, map: impl Fn(Vec3) -> Vec3) -> SolidMesh {
    let mut out = SolidMesh::new(mesh.nodes().iter().map(|&x| map(x)).collect()).unwrap();
    for b in mesh.blocks() {
        out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.node_set_names().map(str::to_string).collect();
    for n in &names {
        out.add_node_set(n, mesh.node_set(n).unwrap().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.element_set_names().map(str::to_string).collect();
    for n in &names {
        out.add_element_set(n, mesh.element_set(n).unwrap().to_vec()).unwrap();
    }
    let names: Vec<String> = mesh.surface_set_names().map(str::to_string).collect();
    for n in &names {
        out.add_surface_set(n, mesh.surface_set(n).unwrap().to_vec()).unwrap();
    }
    out
}

/// Set-ups Q: Quad4 (Hex8), Tri3 (Tet4) and Tri6 (Tet10) faces, the nodes moved by the map of C.
fn meshes_q() -> Vec<(&'static str, SolidMesh)> {
    let warp = |x: Vec3| polar_warp([1.0 + x[0], x[1], x[2]]);
    let (o, l, d) = ([0.0; 3], [1.0, 0.6, 0.4], [2, 2, 1]);
    let tet = tet_box(o, l, d).unwrap();
    vec![
        ("Quad4", rebuild(&hex_box(o, l, d).unwrap(), warp)),
        ("Tri3", rebuild(&tet, warp)),
        ("Tri6", rebuild(&upgrade_to_tet10(&tet).unwrap().0, warp)),
    ]
}

fn differing_bits(a: &[f64], b: &[f64]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(x, y)| x.to_bits() != y.to_bits()).count()
}

fn all_faces(mesh: &SolidMesh) -> Vec<FaceRef> {
    let mut out = Vec::new();
    for e in 0..mesh.n_elements() {
        for f in 0..mesh.element_kind(e).faces().len() {
            out.push(FaceRef { element: e, face: f });
        }
    }
    out
}

fn coords(mesh: &SolidMesh, f: FaceRef) -> Vec<Vec3> {
    mesh.face_nodes(f).iter().map(|&n| mesh.nodes()[n]).collect()
}

fn rel(a: Vec3, b: Vec3) -> f64 {
    norm(sub(a, b)) / norm(b)
}

/// Model path against the per-node forces of `hex20` (A1 or A2) for every box-face set; returns the vectors of the model.
fn model_against_hex20(ch: &mut Checks, tag: &str, mesh: &SolidMesh, pressure: bool) -> Vec<(String, Vec<f64>)> {
    let bare = model_of(mesh.clone());
    let mut out = Vec::new();
    for set in BOX_FACES {
        let mut m = model_of(mesh.clone());
        let (load, extra) = if pressure {
            (Load::pressure(set, P), pressure_loads(mesh, set, P).unwrap())
        } else {
            (Load::traction(set, T), traction_loads(mesh, set, T).unwrap())
        };
        m.add_load(load).unwrap();
        let fm = nodal_load_vector(&m, &[]).unwrap();
        let fh = nodal_load_vector(&bare, &extra).unwrap();
        let name = if pressure { "A1 pressure" } else { "A2 traction" };
        ch.equals(
            &format!("{name} {tag} {set}: differing entries"),
            differing_bits(&fm, &fh),
            0,
        );
        out.push((set.to_string(), fm));
    }
    out
}

#[test]
fn a1_a2_k2_n1_n3_model_loads_equal_hex20_per_node_forces() {
    let mut ch = Checks::new();
    for (tag, mesh) in [("B", mesh_b()), ("C", mesh_c())] {
        let press = model_against_hex20(&mut ch, tag, &mesh, true);
        model_against_hex20(&mut ch, tag, &mesh, false);
        let dofs = model_of(mesh.clone()).dof_map();
        for (set, f) in &press {
            // K2a: total force of the set against -p sum A_f.
            let mut a = [0.0; 3];
            let mut scale = 0.0;
            for &face in mesh.surface_set(set).unwrap() {
                let af = mesh.face_area_vector(face);
                scale += norm(af);
                for i in 0..3 {
                    a[i] += af[i];
                }
            }
            let mut total = [0.0; 3];
            for n in 0..mesh.nodes().len() {
                for (k, tk) in total.iter_mut().enumerate() {
                    *tk += f[dofs.equation(n, k).unwrap()];
                }
            }
            let err = norm([total[0] + P * a[0], total[1] + P * a[1], total[2] + P * a[2]]) / (P * scale);
            ch.at_most(&format!("K2a {tag} {set}: total force"), err, 1e-13);
            // N3: every node of the set has a non-zero force component.
            let nodes = mesh.surface_set_nodes(set).unwrap();
            let zero = nodes
                .iter()
                .filter(|&&n| (0..3).all(|k| f[dofs.equation(n, k).unwrap()] == 0.0))
                .count();
            ch.equals(&format!("N3 {tag} {set}: nodes without force"), zero, 0);
        }
    }
    // K2b: the whole boundary of B.
    let mesh = mesh_b();
    let mut m = model_of(mesh.clone());
    let mut scale = 0.0;
    for set in BOX_FACES {
        m.add_load(Load::pressure(set, P)).unwrap();
        for &face in mesh.surface_set(set).unwrap() {
            scale += norm(mesh.face_area_vector(face));
        }
    }
    let f = nodal_load_vector(&m, &[]).unwrap();
    let dofs = m.dof_map();
    let mut total = [0.0; 3];
    for n in 0..mesh.nodes().len() {
        for (k, tk) in total.iter_mut().enumerate() {
            *tk += f[dofs.equation(n, k).unwrap()];
        }
    }
    ch.at_most("K2b B closed surface: total force", norm(total) / (P * scale), 1e-13);
    // N1: the comparator sees one ulp.
    let mut g = f.clone();
    let i = g.iter().position(|&v| v != 0.0).unwrap();
    g[i] = f64::from_bits(g[i].to_bits() + 1);
    ch.equals("N1 one ulp seen", differing_bits(&f, &g), 1);
    ch.finish();
}

#[test]
fn a3_to_a5_k1_face_area_of_8_node_faces() {
    let mut ch = Checks::new();
    for (tag, mesh) in [("B", mesh_b()), ("C", mesh_c())] {
        let (mut e3, mut e4) = (0.0f64, 0.0f64);
        let faces = all_faces(&mesh);
        for &f in &faces {
            let x = coords(&mesh, f);
            assert_eq!(x.len(), 8);
            let a_h = hex20::face_area(&x).unwrap();
            e3 = e3.max((mesh.face_area(f) - a_h).abs() / a_h);
            let mut a_ref = [0.0; 3];
            for (uv, w) in hex20::face_rule() {
                let (_, d) = hex20::face_shape_functions(uv);
                let (mut xu, mut xv) = ([0.0; 3], [0.0; 3]);
                for (xa, da) in x.iter().zip(&d) {
                    for i in 0..3 {
                        xu[i] += xa[i] * da[0];
                        xv[i] += xa[i] * da[1];
                    }
                }
                let c = cross(xu, xv);
                for i in 0..3 {
                    a_ref[i] += w * c[i];
                }
            }
            e4 = e4.max(rel(mesh.face_area_vector(f), a_ref));
        }
        ch.at_most(&format!("A3 {tag} face_area vs hex20::face_area"), e3, 1e-15);
        ch.at_most(&format!("A4 {tag} face_area_vector vs 3 x 3 Gauss"), e4, 1e-15);
        ch.holds(
            &format!("A5 {tag} no panic"),
            true,
            &format!("{} faces of 8 nodes", faces.len()),
        );
    }
    let mesh = mesh_f();
    let (mut ev, mut ea) = (0.0f64, 0.0f64);
    for f in all_faces(&mesh) {
        let x = coords(&mesh, f);
        let c = cross(sub(x[2], x[0]), sub(x[3], x[1]));
        let exact = [c[0] / 2.0, c[1] / 2.0, c[2] / 2.0];
        ev = ev.max(rel(mesh.face_area_vector(f), exact));
        ea = ea.max((mesh.face_area(f) - norm(exact)).abs() / norm(exact));
    }
    ch.at_most("K1 F area vector vs diagonals", ev, 1e-14);
    ch.at_most("K1 F area vs diagonals", ea, 1e-14);
    ch.finish();
}

#[test]
fn n2_other_face_counts_refused() {
    let mut ch = Checks::new();
    let m1 = face_shape_functions(5, [0.0, 0.0]).unwrap_err().to_string();
    ch.holds(
        "N2 face_shape_functions(5) refused",
        m1.contains("a face of 5 nodes"),
        &m1,
    );
    let m2 = face_rule(5).unwrap_err().to_string();
    ch.holds("N2 face_rule(5) refused", m2.contains("a face of 5 nodes"), &m2);
    ch.finish();
}

// ------------------------------------------------------------------------------------------------------------------------
// A6, A7: transcription of the code before ADR 0383 (loads.rs and mesh.rs at e05871f6, the arms of 3, 4 and 6 nodes).

mod before {
    use rcfd_core::vec3::{cross, norm, sub, Vec3};

    const G: f64 = 0.577_350_269_189_625_8;
    const GAUSS3_X: [f64; 3] = [-0.774_596_669_241_483_4, 0.0, 0.774_596_669_241_483_4];
    const GAUSS3_W: [f64; 3] = [5.0 / 9.0, 8.0 / 9.0, 5.0 / 9.0];

    pub fn face_shape_functions(n_nodes: usize, uv: [f64; 2]) -> (Vec<f64>, Vec<[f64; 2]>) {
        let [u, v] = uv;
        match n_nodes {
            3 => (vec![1.0 - u - v, u, v], vec![[-1.0, -1.0], [1.0, 0.0], [0.0, 1.0]]),
            4 => {
                let r = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
                let n = r.iter().map(|a| (1.0 + u * a[0]) * (1.0 + v * a[1]) / 4.0).collect();
                let d = r
                    .iter()
                    .map(|a| [a[0] * (1.0 + v * a[1]) / 4.0, (1.0 + u * a[0]) * a[1] / 4.0])
                    .collect();
                (n, d)
            }
            6 => {
                let l = [1.0 - u - v, u, v];
                let dl = [[-1.0, -1.0], [1.0, 0.0], [0.0, 1.0]];
                let mut n = Vec::with_capacity(6);
                let mut d = Vec::with_capacity(6);
                for i in 0..3 {
                    n.push(l[i] * (2.0 * l[i] - 1.0));
                    let s = 4.0 * l[i] - 1.0;
                    d.push([s * dl[i][0], s * dl[i][1]]);
                }
                for [i, j] in [[0usize, 1usize], [1, 2], [2, 0]] {
                    n.push(4.0 * l[i] * l[j]);
                    d.push([
                        4.0 * (l[i] * dl[j][0] + l[j] * dl[i][0]),
                        4.0 * (l[i] * dl[j][1] + l[j] * dl[i][1]),
                    ]);
                }
                (n, d)
            }
            n => panic!("a face of {n} nodes"),
        }
    }

    pub fn face_rule(n_nodes: usize) -> Vec<([f64; 2], f64)> {
        let mut out = Vec::with_capacity(9);
        match n_nodes {
            4 => {
                for j in 0..3 {
                    for i in 0..3 {
                        out.push(([GAUSS3_X[i], GAUSS3_X[j]], GAUSS3_W[i] * GAUSS3_W[j]));
                    }
                }
            }
            3 | 6 => {
                for j in 0..3 {
                    let (r, wr) = ((1.0 + GAUSS3_X[j]) / 2.0, GAUSS3_W[j] / 2.0);
                    for i in 0..3 {
                        let (s, ws) = ((1.0 + GAUSS3_X[i]) / 2.0, GAUSS3_W[i] / 2.0);
                        out.push(([s * (1.0 - r), s * r], ws * wr * s));
                    }
                }
            }
            n => panic!("a face of {n} nodes"),
        }
        out
    }

    /// `add_face_load`: the per-node forces of one face, `pressure` or the uniform traction `t`.
    pub fn face_load(x: &[Vec3], pressure: Option<f64>, t: Vec3) -> Vec<Vec3> {
        let mut fe = vec![[0.0; 3]; x.len()];
        for (uv, w) in face_rule(x.len()) {
            let (n, d) = face_shape_functions(x.len(), uv);
            let mut xu = [0.0; 3];
            let mut xv = [0.0; 3];
            for (xa, da) in x.iter().zip(&d) {
                for i in 0..3 {
                    xu[i] += xa[i] * da[0];
                    xv[i] += xa[i] * da[1];
                }
            }
            let c = cross(xu, xv);
            let tq = match pressure {
                None => {
                    let s = norm(c);
                    [t[0] * s, t[1] * s, t[2] * s]
                }
                Some(p) => [-p * c[0], -p * c[1], -p * c[2]],
            };
            for (fa, na) in fe.iter_mut().zip(&n) {
                for i in 0..3 {
                    fa[i] += w * na * tq[i];
                }
            }
        }
        fe
    }

    /// `SolidMesh::face_tangents`.
    pub fn face_tangents(x: &[Vec3]) -> Vec<(f64, Vec3, Vec3)> {
        match x.len() {
            3 => vec![(0.5, sub(x[1], x[0]), sub(x[2], x[0]))],
            4 => {
                let r = [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
                let mut out = Vec::with_capacity(4);
                for &v in &[-G, G] {
                    for &u in &[-G, G] {
                        let mut xu = [0.0; 3];
                        let mut xv = [0.0; 3];
                        for a in 0..4 {
                            let du = r[a][0] * (1.0 + v * r[a][1]) / 4.0;
                            let dv = (1.0 + u * r[a][0]) * r[a][1] / 4.0;
                            for i in 0..3 {
                                xu[i] += x[a][i] * du;
                                xv[i] += x[a][i] * dv;
                            }
                        }
                        out.push((1.0, xu, xv));
                    }
                }
                out
            }
            6 => {
                let pts = [[1.0 / 6.0, 1.0 / 6.0], [2.0 / 3.0, 1.0 / 6.0], [1.0 / 6.0, 2.0 / 3.0]];
                let mut out = Vec::with_capacity(3);
                for [u, v] in pts {
                    let l = [1.0 - u - v, u, v];
                    let du = [-1.0, 1.0, 0.0];
                    let dv = [-1.0, 0.0, 1.0];
                    let mut gu = [0.0; 6];
                    let mut gv = [0.0; 6];
                    for i in 0..3 {
                        gu[i] = (4.0 * l[i] - 1.0) * du[i];
                        gv[i] = (4.0 * l[i] - 1.0) * dv[i];
                    }
                    for (k, [i, j]) in [[0usize, 1usize], [1, 2], [2, 0]].into_iter().enumerate() {
                        gu[3 + k] = 4.0 * (l[i] * du[j] + l[j] * du[i]);
                        gv[3 + k] = 4.0 * (l[i] * dv[j] + l[j] * dv[i]);
                    }
                    let mut xu = [0.0; 3];
                    let mut xv = [0.0; 3];
                    for a in 0..6 {
                        for i in 0..3 {
                            xu[i] += x[a][i] * gu[a];
                            xv[i] += x[a][i] * gv[a];
                        }
                    }
                    out.push((1.0 / 6.0, xu, xv));
                }
                out
            }
            n => panic!("a face of {n} nodes"),
        }
    }

    pub fn face_area(x: &[Vec3]) -> f64 {
        face_tangents(x)
            .into_iter()
            .map(|(w, xu, xv)| w * norm(cross(xu, xv)))
            .sum()
    }

    pub fn face_area_vector(x: &[Vec3]) -> Vec3 {
        let mut a = [0.0; 3];
        for (w, xu, xv) in face_tangents(x) {
            let c = cross(xu, xv);
            for i in 0..3 {
                a[i] += w * c[i];
            }
        }
        a
    }
}

#[test]
fn a6_a7_tri3_quad4_tri6_unchanged() {
    let mut ch = Checks::new();
    for (tag, mesh) in meshes_q() {
        let bare = model_of(mesh.clone());
        let dofs = bare.dof_map();
        for set in BOX_FACES {
            for pressure in [Some(P), None] {
                let mut m = model_of(mesh.clone());
                m.add_load(match pressure {
                    Some(p) => Load::pressure(set, p),
                    None => Load::traction(set, T),
                })
                .unwrap();
                let fm = nodal_load_vector(&m, &[]).unwrap();
                let mut fr = vec![0.0; fm.len()];
                for &face in mesh.surface_set(set).unwrap() {
                    let fe = before::face_load(&coords(&mesh, face), pressure, T);
                    for (&node, fa) in mesh.face_nodes(face).iter().zip(&fe) {
                        for (k, &fk) in fa.iter().enumerate() {
                            fr[dofs.equation(node, k).unwrap()] += fk;
                        }
                    }
                }
                let what = if pressure.is_some() { "pressure" } else { "traction" };
                ch.equals(
                    &format!("A6 {tag} {set} {what}: differing entries"),
                    differing_bits(&fm, &fr),
                    0,
                );
            }
        }
        let mut diff = 0;
        let faces = all_faces(&mesh);
        for &f in &faces {
            let x = coords(&mesh, f);
            if mesh.face_area(f).to_bits() != before::face_area(&x).to_bits() {
                diff += 1;
            }
            let (a, b) = (mesh.face_area_vector(f), before::face_area_vector(&x));
            diff += (0..3).filter(|&i| a[i].to_bits() != b[i].to_bits()).count();
        }
        ch.equals(
            &format!(
                "A7 {tag} areas and area vectors of {} faces: differing values",
                faces.len()
            ),
            diff,
            0,
        );
    }
    ch.finish();
}
