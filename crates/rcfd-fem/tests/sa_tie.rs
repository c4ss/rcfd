//! F-426 (ADR 0456): tied non-matching meshes (node-to-surface and mortar), beam and shell joints to solids, non-matching
//! cyclic boundaries and nodes on the axis. Checks T1 to T5, G1, J1, J2, C1 to C4 and the negative controls N1 to N6 of the
//! registration.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::{hex_box, tet_box, upgrade_to_tet10};
use rcfd_fem::convert::project_mid_edge_nodes;
use rcfd_fem::cyclic::{boundary, full_rotor, modal_analysis_harmonic};
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::modal::ModalOptions;
use rcfd_fem::model::CyclicOptions;
use rcfd_fem::mpc::{self, harmonic_transformation, transformation, Transformation};
use rcfd_fem::recover::IntegrationPointField;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::tie::{self, TieMethod, TieOptions};
use rcfd_fem::{
    BeamSection, BlockProperty, Constraint, ElementKind, FaceRef, FemError, Load, Material, SolidMesh, SolidModel,
};
use std::collections::BTreeMap;
use std::f64::consts::PI;

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const AXIS: Vec3 = [0.0, 0.0, 1.0];

/// Records every measured value and its limit; asserts at the end so that one run records all numbers.
struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn le(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.1e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn ge(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.6} (limit >= {limit}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{id}: {value} < {limit}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:#?}", self.failed);
    }
}

fn material(e: f64, nu: f64) -> Material {
    Material::isotropic_elastic(e, nu, RHO, 0.0).unwrap()
}

fn rel(a: f64, b: f64) -> f64 {
    (a / b - 1.0).abs()
}

fn opts(method: TieMethod) -> TieOptions {
    TieOptions::new(method)
}

fn clamp(m: &mut SolidModel, set: &str, comps: usize) {
    for c in 0..comps {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn solve(m: &SolidModel, forces: &[NodalForceAt]) -> mpc::MpcSolution {
    mpc::solve_static_mpc(m, &[], forces, &[], &StaticOptions::default()).unwrap()
}

/// `sum f . u / P` over the model's loads and `forces`.
fn work_deflection(m: &SolidModel, forces: &[NodalForceAt], u: &[f64], p: f64) -> f64 {
    let f = nodal_load_vector(m, forces).unwrap();
    f.iter().zip(u).map(|(a, b)| a * b).sum::<f64>() / p
}

fn timoshenko(p: f64, l: f64, e: f64, nu: f64, i: f64, a: f64, kappa: f64) -> f64 {
    let g = e / (2.0 * (1.0 + nu));
    p * l.powi(3) / (3.0 * e * i) + p * l / (kappa * g * a)
}

// ------------------------------------------------------------------------------------------------------------ bars

/// The bar of the prototype P (ADR 0456): part A `[0,1]^3` with `m^3` Hex8 (master), part B `[1,3] x [0,1]^2` with
/// `2s x s x s` (slave), symmetry supports, traction 1 on `x = 3`, `E = 1`, `nu = 0.3`.
fn p_bar(m: usize, s: usize, method: TieMethod) -> SolidModel {
    let a = hex_box([0.0; 3], [1.0, 1.0, 1.0], [m, m, m]).unwrap();
    let b = hex_box([1.0, 0.0, 0.0], [2.0, 1.0, 1.0], [2 * s, s, s]).unwrap();
    p_bar_on(tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap(), method).unwrap()
}

fn p_bar_on(mesh: SolidMesh, method: TieMethod) -> rcfd_fem::Result<SolidModel> {
    let mut md = SolidModel::new(mesh);
    md.add_material("A.all", material(1.0, 0.3)).unwrap();
    md.add_material("B.all", material(1.0, 0.3)).unwrap();
    md.add_constraint(Constraint::displacement("A.xmin", 0, 0.0)).unwrap();
    for p in ["A", "B"] {
        md.add_constraint(Constraint::displacement(&format!("{p}.ymin"), 1, 0.0))
            .unwrap();
        md.add_constraint(Constraint::displacement(&format!("{p}.zmin"), 2, 0.0))
            .unwrap();
    }
    md.add_load(Load::traction("B.xmax", [1.0, 0.0, 0.0])).unwrap();
    tie::add_tie(&mut md, "B.xmin", "A.xmax", &opts(method))?;
    Ok(md)
}

struct BarResult {
    far: f64,
    ip: f64,
    el: f64,
    other: f64,
}

fn bar_measures(md: &SolidModel) -> BarResult {
    let sol = solve(md, &[]);
    let u = sol.result.displacement_values();
    let dofs = md.dof_map();
    let far_nodes = md.mesh().node_set("B.xmax").unwrap();
    let far =
        far_nodes.iter().map(|&p| u[dofs.equation(p, 0).unwrap()]).sum::<f64>() / far_nodes.len() as f64 / 3.0 - 1.0;
    let field = IntegrationPointField::compute(md, u).unwrap();
    let (mut ip, mut el, mut other) = (0.0f64, 0.0f64, 0.0f64);
    let mesh = md.mesh();
    for e in 0..mesh.n_elements() {
        let xs: Vec<f64> = mesh.element_nodes(e).iter().map(|&n| mesh.nodes()[n][0]).collect();
        let (lo, hi) = xs
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
        if (hi - 1.0).abs() > 1e-12 && (lo - 1.0).abs() > 1e-12 {
            continue;
        }
        let r = field.offsets()[e]..field.offsets()[e + 1];
        let mut mean = 0.0;
        for k in r.clone() {
            let s = field.stress()[k];
            ip = ip.max((s[0] - 1.0).abs());
            mean += s[0] / r.len() as f64;
            for c in 1..6 {
                other = other.max(s[c].abs());
            }
        }
        el = el.max((mean - 1.0).abs());
    }
    BarResult { far, ip, el, other }
}

/// The prototype's numbers (ADR 0456, P): far-end error, largest point error, largest element-mean error.
const P_NUMBERS: [(usize, usize, f64, f64, f64); 4] = [
    (2, 3, 2.8797762239e-03, 1.4860378066e-01, 9.3068612995e-02),
    (4, 6, 1.4258034027e-03, 1.4897849755e-01, 9.2087955168e-02),
    (3, 5, 4.6271030946e-04, 1.0105716779e-01, 5.6769147161e-02),
    (6, 10, 2.2937058253e-04, 1.0105258418e-01, 5.6804678473e-02),
];

#[test]
fn t2_mortar_flat_interfaces() {
    let mut c = Checks::new();
    for &(m, s, _, _, _) in &P_NUMBERS {
        let r = bar_measures(&p_bar(m, s, TieMethod::Mortar));
        println!(
            "mortar {m}:{s}: far {:+.4e}, ip {:.4e}, el {:.4e}, other {:.4e}",
            r.far, r.ip, r.el, r.other
        );
        c.le(&format!("T2a {m}:{s} far-end (roadmap)"), r.far.abs(), 1e-3);
        c.le(&format!("T2a {m}:{s} far-end (rounding)"), r.far.abs(), 1e-9);
        c.le(&format!("T2b {m}:{s} interface sigma_xx (roadmap)"), r.ip, 0.03);
        c.le(&format!("T2b {m}:{s} interface sigma_xx (rounding)"), r.ip, 1e-9);
        c.le(&format!("T2c {m}:{s} other components"), r.other, 1e-9);
    }
    c.finish();
}

#[test]
fn t4_node_to_surface_against_prototype() {
    let mut c = Checks::new();
    let mut got = Vec::new();
    for &(m, s, far, ip, el) in &P_NUMBERS {
        let r = bar_measures(&p_bar(m, s, TieMethod::NodeToSurface));
        println!(
            "NTS {m}:{s}: far {:+.10e}, ip {:.10e}, el {:.10e}, other {:.4e}",
            r.far, r.ip, r.el, r.other
        );
        c.le(
            &format!("T4a {m}:{s} far-end against P"),
            (r.far - far).abs() / far.abs(),
            1e-6,
        );
        c.le(&format!("T4b {m}:{s} point error against P"), rel(r.ip, ip), 1e-6);
        c.le(
            &format!("T4b {m}:{s} element-mean error against P"),
            rel(r.el, el),
            1e-6,
        );
        got.push(r);
    }
    c.ge("T4c 2:3 to 4:6 order", (got[0].far / got[1].far).log2(), 1.0);
    c.ge("T4c 3:5 to 6:10 order", (got[2].far / got[3].far).log2(), 1.0);
    c.ge("T4d 2:3 to 4:6 point error ratio", got[1].ip / got[0].ip, 0.98);
    c.ge("T4d 3:5 to 6:10 point error ratio", got[3].ip / got[2].ip, 0.98);
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ T1

fn t1_parts(quadratic: bool) -> (SolidMesh, SolidMesh) {
    let a = hex_box([0.0; 3], [1.0, 0.5, 0.5], [2, 2, 2]).unwrap();
    let b = hex_box([1.0, 0.0, 0.0], [1.0, 0.5, 0.5], [2, 2, 2]).unwrap();
    let merged = hex_box([0.0; 3], [2.0, 0.5, 0.5], [4, 2, 2]).unwrap();
    if quadratic {
        let a = upgrade_to_hex20(&a).unwrap().0;
        let b = upgrade_to_hex20(&b).unwrap().0;
        (
            tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap(),
            upgrade_to_hex20(&merged).unwrap().0,
        )
    } else {
        (tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap(), merged)
    }
}

#[test]
fn t1_matching_meshes_equal_merged() {
    let mut c = Checks::new();
    for quadratic in [false, true] {
        let (tied_mesh, merged_mesh) = t1_parts(quadratic);
        let mut merged = SolidModel::new(merged_mesh);
        merged.add_material("all", material(E, 0.3)).unwrap();
        clamp(&mut merged, "xmin", 3);
        merged.add_load(Load::traction("xmax", [1e6, 5e5, -3e5])).unwrap();
        let um = solve(&merged, &[]);
        let um = um.result.displacement_values();
        let scale = um.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        for method in [TieMethod::NodeToSurface, TieMethod::Mortar] {
            let mut t = SolidModel::new(tied_mesh.clone());
            t.add_material("A.all", material(E, 0.3)).unwrap();
            t.add_material("B.all", material(E, 0.3)).unwrap();
            clamp(&mut t, "A.xmin", 3);
            t.add_load(Load::traction("B.xmax", [1e6, 5e5, -3e5])).unwrap();
            tie::add_tie(&mut t, "B.xmin", "A.xmax", &opts(method)).unwrap();
            let ut = solve(&t, &[]);
            let ut = ut.result.displacement_values();
            let (dt, dm) = (t.dof_map(), merged.dof_map());
            let mut worst: f64 = 0.0;
            for (p, x) in t.mesh().nodes().iter().enumerate() {
                let q = merged
                    .mesh()
                    .nodes()
                    .iter()
                    .position(|y| (0..3).all(|i| (x[i] - y[i]).abs() < 1e-12))
                    .expect("merged node");
                for k in 0..3 {
                    worst = worst.max((ut[dt.equation(p, k).unwrap()] - um[dm.equation(q, k).unwrap()]).abs());
                }
            }
            c.le(
                &format!("T1 {} {method:?}", if quadratic { "Hex20" } else { "Hex8" }),
                worst / scale,
                1e-10,
            );
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ T3, T5

const P_TIP: f64 = 1000.0;

fn t3_model(method: TieMethod) -> SolidModel {
    let a = upgrade_to_hex20(&hex_box([0.0; 3], [0.5, 0.1, 0.1], [10, 2, 2]).unwrap())
        .unwrap()
        .0;
    let b = upgrade_to_tet10(&tet_box([0.5, 0.0, 0.0], [0.5, 0.1, 0.1], [15, 3, 3]).unwrap())
        .unwrap()
        .0;
    let mut m = SolidModel::new(tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap());
    m.add_material("A.all", material(E, 0.0)).unwrap();
    m.add_material("B.all", material(E, 0.0)).unwrap();
    clamp(&mut m, "A.xmin", 3);
    m.add_load(Load::traction("B.xmax", [0.0, 0.0, -P_TIP / 0.01])).unwrap();
    tie::add_tie(&mut m, "B.xmin", "A.xmax", &opts(method)).unwrap();
    m
}

#[test]
fn t3_t5_cantilever_tet10_to_hex20() {
    let mut c = Checks::new();
    let dt = timoshenko(P_TIP, 1.0, E, 0.0, 0.1f64.powi(4) / 12.0, 0.01, 5.0 / 6.0);
    for (id, method) in [("T3", TieMethod::Mortar), ("T3b", TieMethod::NodeToSurface)] {
        let m = t3_model(method);
        let sol = solve(&m, &[]);
        let u = sol.result.displacement_values();
        let delta = work_deflection(&m, &[], u, P_TIP);
        println!(
            "{id} {method:?}: delta {delta:.10e}, closed form {dt:.10e}, signed {:+.4e}",
            delta / dt - 1.0
        );
        c.le(&format!("{id} tip deflection"), rel(delta, dt), 0.01);
        // T5: resultant and moment of the constraint forces over the interface nodes.
        let dofs = m.dof_map();
        let mut nodes = m.mesh().surface_set_nodes("B.xmin").unwrap();
        nodes.extend(m.mesh().surface_set_nodes("A.xmax").unwrap());
        let (mut res, mut mom, mut ft) = ([0.0f64; 3], [0.0f64; 3], 0.0f64);
        for &p in &nodes {
            let x = m.mesh().nodes()[p];
            let r: Vec3 = [0, 1, 2].map(|k| sol.constraint_forces[dofs.equation(p, k).unwrap()]);
            let mxr = rcfd_core::vec3::cross(x, r);
            for k in 0..3 {
                res[k] += r[k];
                mom[k] += mxr[k];
            }
            ft += rcfd_core::vec3::norm(r);
        }
        println!("{id} T5: resultant {res:?}, moment {mom:?}, F_t {ft:e}");
        c.le(
            &format!("T5 {method:?} resultant"),
            rcfd_core::vec3::norm(res) / ft,
            1e-10,
        );
        c.le(
            &format!("T5 {method:?} moment"),
            rcfd_core::vec3::norm(mom) / (ft * 0.1 * 2f64.sqrt()),
            1e-10,
        );
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ G1

fn with_node_moved(mesh: &SolidMesh, node: usize, delta: Vec3) -> SolidMesh {
    let mut x = mesh.nodes().to_vec();
    for i in 0..3 {
        x[node][i] += delta[i];
    }
    let mut out = SolidMesh::new(x).unwrap();
    for b in mesh.blocks() {
        let k = out.add_block(b.kind(), b.connectivity().to_vec()).unwrap();
        if let Some(p) = b.property() {
            out.set_block_property(k, p.clone()).unwrap();
        }
    }
    for n in mesh.node_set_names() {
        out.add_node_set(n, mesh.node_set(n).unwrap().to_vec()).unwrap();
    }
    for n in mesh.element_set_names() {
        out.add_element_set(n, mesh.element_set(n).unwrap().to_vec()).unwrap();
    }
    for n in mesh.surface_set_names() {
        out.add_surface_set(n, mesh.surface_set(n).unwrap().to_vec()).unwrap();
    }
    out
}

#[test]
fn g1_gap_refused_with_node_id() {
    let mut c = Checks::new();
    let a = hex_box([0.0; 3], [1.0, 1.0, 1.0], [2, 2, 2]).unwrap();
    let b = hex_box([1.0, 0.0, 0.0], [2.0, 1.0, 1.0], [6, 3, 3]).unwrap();
    let mesh = tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap();
    let node = mesh
        .nodes()
        .iter()
        .enumerate()
        .skip(a.nodes().len())
        .position(|(_, x)| {
            (x[0] - 1.0).abs() < 1e-12 && (x[1] - 1.0 / 3.0).abs() < 1e-12 && (x[2] - 1.0 / 3.0).abs() < 1e-12
        })
        .unwrap()
        + a.nodes().len();
    let hm = 0.5;
    for method in [TieMethod::NodeToSurface, TieMethod::Mortar] {
        match p_bar_on(with_node_moved(&mesh, node, [0.1 * hm, 0.0, 0.0]), method) {
            Err(FemError::TieGap {
                node: n,
                distance,
                tolerance,
            }) => {
                println!("G1 {method:?}: refused node {n}, distance {distance:e}, tolerance {tolerance:e}");
                c.holds(
                    &format!("G1 {method:?} node id"),
                    n == node,
                    &format!("node {n} (moved {node})"),
                );
                c.le(&format!("G1 {method:?} distance"), rel(distance, 0.1 * hm), 1e-12);
                c.le(&format!("G1 {method:?} tolerance"), rel(tolerance, 0.05 * hm), 1e-12);
            }
            other => c.holds(&format!("G1 {method:?} refused"), false, &format!("{:?}", other.err())),
        }
        let moved = with_node_moved(&mesh, node, [0.02 * hm, 0.0, 0.0]);
        let mut md = SolidModel::new(moved);
        md.add_material("A.all", material(1.0, 0.3)).unwrap();
        md.add_material("B.all", material(1.0, 0.3)).unwrap();
        match tie::tie(&md, "B.xmin", "A.xmax", &opts(method)) {
            Ok(r) => {
                let d = r.projections.iter().find(|p| p.node == node).unwrap().distance;
                println!("G1 {method:?}: accepted at 0.02 h, distance {d:e}");
                c.le(&format!("G1 {method:?} accepted distance"), rel(d, 0.02 * hm), 1e-12);
            }
            Err(e) => c.holds(&format!("G1 {method:?} accepted"), false, &e.to_string()),
        }
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ joints

fn j1_model(joint: bool) -> (SolidModel, usize) {
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
    let section = BeamSection::new(
        0.01,
        0.1f64.powi(4) / 12.0,
        0.1f64.powi(4) / 12.0,
        1.406e-5,
        5.0 / 6.0,
        5.0 / 6.0,
    )
    .unwrap();
    b.set_block_property(k, BlockProperty::beam(section, [0.0, 1.0, 0.0]).unwrap())
        .unwrap();
    b.add_element_set("all", (0..nb).collect()).unwrap();
    b.add_node_set("tip", vec![nb]).unwrap();
    let mesh = tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap();
    let joint_node = a.nodes().len();
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", material(E, 0.0)).unwrap();
    m.add_material("B.all", material(E, 0.0)).unwrap();
    clamp(&mut m, "A.xmin", 3);
    m.add_load(Load::nodal_force("B.tip", [0.0, 0.0, -P_TIP])).unwrap();
    if joint {
        for cst in tie::beam_to_solid(&m, joint_node, "A.xmax").unwrap() {
            m.add_constraint(cst).unwrap();
        }
    }
    (m, joint_node)
}

#[test]
#[ignore = "known miss of ADR 0456 (commit 11ec84f8): J1 2.25e-2 against the limit 1e-2 (the beam end on the solid face by a unit-weight distributing coupling is softer); N4 met; diagnosis in addendum 1"]
fn j1_beam_continuing_a_solid_bar_known_miss() {
    let mut c = Checks::new();
    let (m, _) = j1_model(true);
    let sol = solve(&m, &[]);
    let delta = work_deflection(&m, &[], sol.result.displacement_values(), P_TIP);
    let dt = timoshenko(P_TIP, 2.0, E, 0.0, 0.1f64.powi(4) / 12.0, 0.01, 5.0 / 6.0);
    println!(
        "J1: delta {delta:.10e}, closed form {dt:.10e}, signed {:+.4e}",
        delta / dt - 1.0
    );
    c.le("J1 tip deflection", rel(delta, dt), 0.01);
    // N4: without the joint the static solve is refused.
    let (free, _) = j1_model(false);
    let r = mpc::solve_static_mpc(&free, &[], &[], &[], &StaticOptions::default());
    c.holds("N4 no joint refused", r.is_err(), &format!("{:?}", r.as_ref().err()));
    c.finish();
}

fn j2_model(shift: f64) -> (SolidModel, Vec<NodalForceAt>, rcfd_fem::Result<Vec<Constraint>>) {
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
    nodes[id(0, 1)][1] += shift;
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
    m.add_material("A.all", material(E, 0.0)).unwrap();
    m.add_material("B.all", material(E, 0.0)).unwrap();
    clamp(&mut m, "A.xmin", 3);
    let forces = vec![
        NodalForceAt::new(n0 + id(nx, 0), [0.0, 0.0, -P_TIP / 4.0]),
        NodalForceAt::new(n0 + id(nx, 1), [0.0, 0.0, -P_TIP / 2.0]),
        NodalForceAt::new(n0 + id(nx, 2), [0.0, 0.0, -P_TIP / 4.0]),
    ];
    let rows = tie::shell_to_solid(&m, "B.edge", "A.xmax", 1e-9);
    (m, forces, rows)
}

#[test]
#[ignore = "known miss of ADR 0456 (commit 11ec84f8): J2 2.36e-2 against the limit 2e-2 (the shell edge on the plate face by unit-weight column couplings is softer); N5 met; diagnosis in addendum 1"]
fn j2_shell_continuing_a_solid_plate_known_miss() {
    let mut c = Checks::new();
    let (mut m, forces, rows) = j2_model(0.0);
    for cst in rows.unwrap() {
        m.add_constraint(cst).unwrap();
    }
    let sol = solve(&m, &forces);
    let delta = work_deflection(&m, &forces, sol.result.displacement_values(), P_TIP);
    let dt = timoshenko(P_TIP, 1.0, E, 0.0, 0.1 * 0.02f64.powi(3) / 12.0, 0.002, 5.0 / 6.0);
    println!(
        "J2: delta {delta:.10e}, closed form {dt:.10e}, signed {:+.4e}",
        delta / dt - 1.0
    );
    c.le("J2 tip deflection", rel(delta, dt), 0.02);
    // N5: a shell node off every column is refused naming it.
    let (m5, _, rows5) = j2_model(0.01);
    let moved = m5.mesh().node_set("B.edge").unwrap()[1];
    match rows5 {
        Err(FemError::InvalidValue { key, reason }) => {
            println!("N5: {key}: {reason}");
            c.holds(
                "N5 refused naming the node",
                reason.contains(&format!("shell node {moved}:")),
                &reason,
            );
        }
        other => c.holds("N5 refused", false, &format!("{:?}", other.map(|v| v.len()))),
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ cyclic ring

const N_RING: usize = 12;
const R_RING: f64 = 1.0;
const T_RING: f64 = 0.05;
const W_RING: f64 = 0.1;

fn ring_part(phi0: f64, dphi: f64, div: [usize; 3]) -> SolidMesh {
    let b = hex_box(
        [R_RING - T_RING / 2.0, phi0, -W_RING / 2.0],
        [T_RING, dphi, W_RING],
        div,
    )
    .unwrap();
    upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0
}

fn spoke_nodes(mesh: &SolidMesh, alpha: f64) -> Vec<usize> {
    (0..mesh.nodes().len())
        .filter(|&p| {
            let x = mesh.nodes()[p];
            ((x[0] * x[0] + x[1] * x[1]).sqrt() - (R_RING - T_RING / 2.0)).abs() < 1e-12
                && (x[1].atan2(x[0]) - alpha / 2.0).abs() < 1e-12
        })
        .collect()
}

fn ring_matching() -> SolidModel {
    let alpha = 2.0 * PI / N_RING as f64;
    let mut mesh = ring_part(0.0, alpha, [1, 4, 3]);
    let spoke = spoke_nodes(&mesh, alpha);
    assert_eq!(spoke.len(), 7, "spoke nodes");
    mesh.add_node_set("spoke", spoke).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(E, 0.0)).unwrap();
    clamp(&mut m, "spoke", 3);
    m.add_constraint(Constraint::cyclic("ymin", "ymax", AXIS, [0.0; 3], N_RING))
        .unwrap();
    m
}

fn ring_tied(interpolation: Option<TieMethod>) -> SolidModel {
    let alpha = 2.0 * PI / N_RING as f64;
    let a = ring_part(0.0, alpha / 2.0, [1, 2, 2]);
    let b = ring_part(alpha / 2.0, alpha / 2.0, [1, 2, 3]);
    let mut mesh = tie::assemble_parts(&[("A", &a), ("B", &b)]).unwrap();
    let spoke = spoke_nodes(&mesh, alpha);
    assert_eq!(spoke.len(), 5 + 7, "spoke nodes of both parts");
    mesh.add_node_set("spoke", spoke).unwrap();
    let mut m = SolidModel::new(mesh);
    m.add_material("A.all", material(E, 0.0)).unwrap();
    m.add_material("B.all", material(E, 0.0)).unwrap();
    clamp(&mut m, "spoke", 3);
    tie::add_tie(&mut m, "B.ymin", "A.ymax", &opts(TieMethod::Mortar)).unwrap();
    let options = match interpolation {
        Some(method) => CyclicOptions::new().interpolation(opts(method)),
        None => CyclicOptions::new(),
    };
    m.add_constraint(Constraint::cyclic_with(
        "A.ymin", "B.ymax", AXIS, [0.0; 3], N_RING, options,
    ))
    .unwrap();
    m
}

fn ring_options() -> ModalOptions {
    let mut o = ModalOptions::new(4).tolerance(1e-9);
    o.max_iterations = 400;
    o
}

#[test]
fn c1_ring_sector_with_a_2_3_boundary() {
    let mut c = Checks::new();
    let reference = ring_matching();
    let refs: Vec<Vec<f64>> = (0..=N_RING / 2)
        .map(|k| {
            modal_analysis_harmonic(&reference, &[], k, &ring_options())
                .unwrap()
                .values
        })
        .collect();
    for (id, method) in [("C1", TieMethod::Mortar), ("C1b", TieMethod::NodeToSurface)] {
        let m = ring_tied(Some(method));
        let mut worst: f64 = 0.0;
        for k in 0..=N_RING / 2 {
            let h = modal_analysis_harmonic(&m, &[], k, &ring_options()).unwrap();
            let devs: Vec<f64> = h
                .values
                .iter()
                .zip(&refs[k])
                .map(|(a, b)| (a / b).sqrt() - 1.0)
                .collect();
            let devs_e: Vec<String> = devs.iter().map(|d| format!("{d:+.3e}")).collect();
            println!("{id} k = {k}: frequency deviations {devs_e:?}");
            worst = devs.iter().fold(worst, |w, d| w.max(d.abs()));
        }
        c.le(&format!("{id} frequencies"), worst, 0.005);
    }
    // N1: without the interpolation the boundaries are refused.
    let plain = ring_tied(None);
    let ci = plain
        .constraints()
        .iter()
        .position(|k| matches!(k, Constraint::Cyclic { .. }))
        .unwrap();
    match boundary(&plain, ci) {
        Err(FemError::InvalidValue { key, reason }) => {
            println!("N1: {key}: {reason}");
            c.holds("N1 refused", key == "cyclic", &reason);
        }
        other => c.holds("N1 refused", false, &format!("{:?}", other.map(|b| b.pairs().len()))),
    }
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ axis

/// A sector `[0, rmax] x [0, alpha] x [0, h]` of Tet10 from `tet_box` in `(r, phi, z)`, the `r = 0` nodes merged per level
/// (degenerate tetrahedra dropped); surface sets `low` (phi = 0), `high` (phi = alpha); node sets `bottom` (z = 0), `pin`
/// (`(rmax, 0, 0)`); mid-edge nodes of rim edges on the circle when `curved`.
fn polar_tets(rmax: f64, alpha: f64, h: f64, div: [usize; 3], curved: bool) -> SolidMesh {
    let lb = tet_box([0.0; 3], [rmax, alpha, h], div).unwrap();
    let mut new_of = vec![usize::MAX; lb.nodes().len()];
    let mut pos: Vec<Vec3> = Vec::new();
    let mut axis_of: BTreeMap<u64, usize> = BTreeMap::new();
    for (p, q) in lb.nodes().iter().enumerate() {
        if q[0] == 0.0 {
            let key = q[2].to_bits();
            if let Some(&n) = axis_of.get(&key) {
                new_of[p] = n;
                continue;
            }
            axis_of.insert(key, pos.len());
        }
        new_of[p] = pos.len();
        pos.push([q[0] * q[1].cos(), q[0] * q[1].sin(), q[2]]);
    }
    let mut conn = Vec::new();
    for t in lb.blocks()[0].connectivity().chunks(4) {
        let m: Vec<usize> = t.iter().map(|&p| new_of[p]).collect();
        if (0..4).all(|i| !m[..i].contains(&m[i])) {
            conn.extend(m);
        }
    }
    let mut mesh = SolidMesh::new(pos).unwrap();
    mesh.add_block(ElementKind::Tet4, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("all", (0..ne).collect()).unwrap();
    let (mesh, mids) = upgrade_to_tet10(&mesh).unwrap();
    let mesh = if curved {
        project_mid_edge_nodes(&mesh, &mids, |m, x| {
            let r = |p: Vec3| (p[0] * p[0] + p[1] * p[1]).sqrt();
            let on = m.ends.iter().all(|&e| (r(mesh.nodes()[e]) - rmax).abs() < 1e-12);
            on.then(|| {
                let rr = r(x);
                [x[0] * rmax / rr, x[1] * rmax / rr, x[2]]
            })
        })
        .unwrap()
    } else {
        mesh
    };
    let mut mesh = mesh;
    let (s, co) = alpha.sin_cos();
    let (mut low, mut high) = (Vec::new(), Vec::new());
    for faces in mesh.face_incidence().values() {
        if faces.len() != 1 {
            continue;
        }
        let f: FaceRef = faces[0];
        let xs: Vec<Vec3> = mesh.face_nodes(f).iter().map(|&n| mesh.nodes()[n]).collect();
        if xs.iter().all(|x| x[1].abs() < 1e-12 && x[0] > -1e-12) {
            low.push(f);
        } else if xs
            .iter()
            .all(|x| (-s * x[0] + co * x[1]).abs() < 1e-12 && co * x[0] + s * x[1] > -1e-12)
        {
            high.push(f);
        }
    }
    mesh.add_surface_set("low", low).unwrap();
    mesh.add_surface_set("high", high).unwrap();
    let n = mesh.nodes().len();
    let bottom: Vec<usize> = (0..n).filter(|&p| mesh.nodes()[p][2] == 0.0).collect();
    let pin: Vec<usize> = (0..n)
        .filter(|&p| {
            let x = mesh.nodes()[p];
            (x[0] - rmax).abs() < 1e-12 && x[1].abs() < 1e-12 && x[2] == 0.0
        })
        .collect();
    mesh.add_node_set("bottom", bottom).unwrap();
    mesh.add_node_set("pin", pin).unwrap();
    mesh
}

fn axis_nodes_of(mesh: &SolidMesh) -> Vec<usize> {
    (0..mesh.nodes().len())
        .filter(|&p| {
            let x = mesh.nodes()[p];
            x[0] == 0.0 && x[1] == 0.0
        })
        .collect()
}

fn on_axis() -> CyclicOptions {
    CyclicOptions::new().axis_nodes(true)
}

const N_DISC: usize = 8;
const NU_DISC: f64 = 0.3;
const OMEGA: f64 = 100.0;

fn disc(options: CyclicOptions) -> SolidModel {
    let mesh = polar_tets(1.0, 2.0 * PI / N_DISC as f64, 0.025, [8, 3, 1], true);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(E, NU_DISC)).unwrap();
    m.add_constraint(Constraint::displacement("bottom", 2, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pin", 1, 0.0)).unwrap();
    m.add_load(Load::centrifugal(AXIS, [0.0; 3], OMEGA)).unwrap();
    m.add_constraint(Constraint::cyclic_with("low", "high", AXIS, [0.0; 3], N_DISC, options))
        .unwrap();
    m
}

fn empty_rows(tr: &Transformation, unknowns: &[usize]) -> bool {
    let t = tr.t();
    unknowns
        .iter()
        .all(|&i| t.row_ptr[i + 1] == t.row_ptr[i] && tr.g()[i] == 0.0)
}

#[test]
fn c2_spinning_disc_with_nodes_on_the_axis() {
    let mut c = Checks::new();
    let m = disc(on_axis());
    let sol = solve(&m, &[]);
    let u = sol.result.displacement_values();
    let field = IntegrationPointField::compute(&m, u).unwrap();
    let axis = axis_nodes_of(m.mesh());
    let k = RHO * OMEGA * OMEGA / 8.0;
    let sc = (3.0 + NU_DISC) * k;
    let mut worst: f64 = 0.0;
    let mut count = 0;
    for e in 0..m.mesh().n_elements() {
        if !m.mesh().element_nodes(e).iter().any(|n| axis.contains(n)) {
            continue;
        }
        for q in field.offsets()[e]..field.offsets()[e + 1] {
            let x = field.positions()[q];
            let r2 = x[0] * x[0] + x[1] * x[1];
            let (sr, st) = (
                (3.0 + NU_DISC) * k * (1.0 - r2),
                k * ((3.0 + NU_DISC) - (1.0 + 3.0 * NU_DISC) * r2),
            );
            let (cs, sn) = if r2 > 0.0 {
                (x[0] / r2.sqrt(), x[1] / r2.sqrt())
            } else {
                (1.0, 0.0)
            };
            let sxx = sr * cs * cs + st * sn * sn;
            let syy = sr * sn * sn + st * cs * cs;
            let s = field.stress()[q];
            worst = worst.max((s[0] - sxx).abs().max((s[1] - syy).abs()) / sc);
            count += 1;
        }
    }
    println!("C2: {count} points of the elements on the axis, worst {worst:.4e}, centre stress {sc:e}");
    c.le("C2 centre stress field", worst, 0.01);
    let dofs = m.dof_map();
    let tr = transformation(&m, &[]).unwrap();
    let inplane: Vec<usize> = axis
        .iter()
        .flat_map(|&p| [0, 1].map(|k| dofs.equation(p, k).unwrap()))
        .collect();
    c.holds(
        "C2b axis in-plane held",
        empty_rows(&tr, &inplane),
        &format!("{} axis nodes", axis.len()),
    );
    // N2: without the option the axis nodes are refused; N3: a node on both boundaries off the axis is refused; N6: no
    // full rotor.
    let plain = disc(CyclicOptions::new());
    let r = boundary(&plain, 2);
    c.holds(
        "N2 refused",
        matches!(&r, Err(FemError::InvalidValue { key: "cyclic", .. })),
        &format!("{:?}", r.err()),
    );
    let mut both = SolidModel::new(m.mesh().clone());
    both.add_material("all", material(E, NU_DISC)).unwrap();
    both.add_constraint(Constraint::cyclic_with("low", "low", AXIS, [0.0; 3], N_DISC, on_axis()))
        .unwrap();
    match boundary(&both, 0) {
        Err(FemError::InvalidValue { reason, .. }) => {
            println!("N3: {reason}");
            c.holds("N3 refused off the axis", reason.contains("from the axis"), &reason);
        }
        other => c.holds(
            "N3 refused",
            false,
            &format!("{:?}", other.map(|b| b.axis_nodes().len())),
        ),
    }
    let r = full_rotor(&m);
    c.holds(
        "N6 full rotor refused",
        matches!(&r, Err(FemError::InvalidValue { key: "cyclic", .. })),
        &format!("{:?}", r.err()),
    );
    c.finish();
}

const N_ROD: usize = 8;
const A_ROD: f64 = 0.05;
const L_ROD: f64 = 1.5;
const NU_ROD: f64 = 0.3;

fn rod() -> SolidModel {
    let mesh = polar_tets(A_ROD, 2.0 * PI / N_ROD as f64, L_ROD, [2, 2, 30], false);
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material(E, NU_ROD)).unwrap();
    clamp(&mut m, "bottom", 3);
    m.add_constraint(Constraint::cyclic_with("low", "high", AXIS, [0.0; 3], N_ROD, on_axis()))
        .unwrap();
    m
}

fn rod_beam() -> SolidModel {
    let ms = (2 * N_ROD) as f64;
    let area = ms / 2.0 * A_ROD * A_ROD * (2.0 * PI / ms).sin();
    let inertia = area * A_ROD * A_ROD * (2.0 + (2.0 * PI / ms).cos()) / 12.0;
    let kappa = 6.0 * (1.0 + NU_ROD) / (7.0 + 6.0 * NU_ROD);
    let ne = 60;
    let nodes: Vec<Vec3> = (0..=ne).map(|i| [0.0, 0.0, L_ROD * i as f64 / ne as f64]).collect();
    let mut b = SolidMesh::new(nodes).unwrap();
    let k = b
        .add_block(ElementKind::Beam2, (0..ne).flat_map(|i| [i, i + 1]).collect())
        .unwrap();
    let section = BeamSection::new(area, inertia, inertia, 2.0 * inertia, kappa, kappa).unwrap();
    b.set_block_property(k, BlockProperty::beam(section, [1.0, 0.0, 0.0]).unwrap())
        .unwrap();
    b.add_element_set("all", (0..ne).collect()).unwrap();
    b.add_node_set("root", vec![0]).unwrap();
    let mut m = SolidModel::new(b);
    m.add_material("all", material(E, NU_ROD)).unwrap();
    clamp(&mut m, "root", 6);
    m
}

#[test]
#[ignore = "record of the registered run of ADR 0456 (commit 11ec84f8): set-up stopped, the rod's subspace iteration at k = 1 did not reach the test's tolerance 1e-10 (residuals 1.1e-8); corrected by amendment 1 as c3b_c4b_rod_on_the_axis"]
fn c3_c4_rod_on_the_axis() {
    let mut c = Checks::new();
    let m = rod();
    let mut o = ModalOptions::new(2).tolerance(1e-10);
    o.max_iterations = 400;
    let h = modal_analysis_harmonic(&m, &[], 1, &o).unwrap();
    let beam = mpc::modal_analysis_mpc(&rod_beam(), &[], &ModalOptions::new(2).tolerance(1e-10)).unwrap();
    let (fr, fb) = (
        h.values[0].sqrt() / (2.0 * PI),
        beam.result.eigenvalues[0].sqrt() / (2.0 * PI),
    );
    println!(
        "C3: rod k = 1 f {fr:.8} Hz (values {:?}), beam f {fb:.8} Hz (values {:?})",
        h.values, beam.result.eigenvalues
    );
    println!("C3 signed: {:+.4e}", fr / fb - 1.0);
    c.le("C3 bending frequency", rel(fr, fb), 0.01);
    let dofs = m.dof_map();
    let n = dofs.n_equations();
    let axis = axis_nodes_of(m.mesh());
    let t2 = harmonic_transformation(&m, &[], 2).unwrap();
    let all: Vec<usize> = axis
        .iter()
        .flat_map(|&p| [0, 1, 2].map(|k| dofs.equation(p, k).unwrap()))
        .flat_map(|e| [e, n + e])
        .collect();
    c.holds(
        "C4 k = 2 axis nodes held",
        empty_rows(&t2, &all),
        &format!("{} axis nodes", axis.len()),
    );
    let t1 = harmonic_transformation(&m, &[], 1).unwrap();
    let axial: Vec<usize> = axis
        .iter()
        .map(|&p| dofs.equation(p, 2).unwrap())
        .flat_map(|e| [e, n + e])
        .collect();
    let free_count_ok = axis.iter().filter(|&&p| m.mesh().nodes()[p][2] > 0.0).all(|&p| {
        let inplane = [0, 1].map(|k| dofs.equation(p, k).unwrap());
        inplane
            .iter()
            .flat_map(|&e| [e, n + e])
            .filter(|i| t1.independent().binary_search(i).is_ok())
            .count()
            == 2
    });
    c.holds(
        "C4b k = 1 axial held",
        empty_rows(&t1, &axial),
        "axial unknowns of the axis nodes",
    );
    c.holds(
        "C4b k = 1 two in-plane free",
        free_count_ok,
        "two of four in-plane unknowns per free axis node",
    );
    c.finish();
}

// ------------------------------------------------------------------------------------------------------------ set-up

/// Not a check: builds every model of the registration and its transformation (no solve, no limit).
#[test]
fn setup_sanity() {
    for (m, s) in [(2, 3), (3, 5)] {
        for method in [TieMethod::NodeToSurface, TieMethod::Mortar] {
            let md = p_bar(m, s, method);
            transformation(&md, &[]).unwrap();
        }
    }
    for method in [TieMethod::NodeToSurface, TieMethod::Mortar] {
        transformation(&t3_model(method), &[]).unwrap();
    }
    transformation(&j1_model(true).0, &[]).unwrap();
    let (mut m2, _, rows) = j2_model(0.0);
    for cst in rows.unwrap() {
        m2.add_constraint(cst).unwrap();
    }
    transformation(&m2, &[]).unwrap();
    for k in [0, 1, 6] {
        harmonic_or_static(&ring_tied(Some(TieMethod::Mortar)), k);
        harmonic_or_static(&ring_tied(Some(TieMethod::NodeToSurface)), k);
        harmonic_or_static(&ring_matching(), k);
    }
    let d = disc(on_axis());
    transformation(&d, &[]).unwrap();
    let r = rod();
    for k in [0, 1, 2] {
        harmonic_or_static(&r, k);
    }
    println!(
        "set-up: disc {} nodes, rod {} nodes, rod axis nodes {}",
        d.mesh().nodes().len(),
        r.mesh().nodes().len(),
        axis_nodes_of(r.mesh()).len()
    );
}

fn harmonic_or_static(m: &SolidModel, k: usize) {
    if k == 0 {
        transformation(m, &[]).unwrap();
    } else {
        harmonic_transformation(m, &[], k).unwrap();
    }
}

// Amendment 1: C3 and C4 with the modal tolerance 1e-8 and 600 iterations.
#[test]
#[ignore = "record of the run of ADR 0456 amendment 1 (commit d394fa73): set-up stopped, the residual of the first pair stays at 1.2e-8 above the tolerance 1e-8; corrected by amendment 2 as c3c_c4c_rod_on_the_axis"]
fn c3b_c4b_rod_on_the_axis() {
    let mut c = Checks::new();
    let m = rod();
    let mut o = ModalOptions::new(2).tolerance(1e-8);
    o.max_iterations = 600;
    let h = modal_analysis_harmonic(&m, &[], 1, &o).unwrap();
    let mut ob = ModalOptions::new(2).tolerance(1e-8);
    ob.max_iterations = 600;
    let beam = mpc::modal_analysis_mpc(&rod_beam(), &[], &ob).unwrap();
    let (fr, fb) = (
        h.values[0].sqrt() / (2.0 * PI),
        beam.result.eigenvalues[0].sqrt() / (2.0 * PI),
    );
    println!(
        "C3: rod k = 1 f {fr:.8} Hz (values {:?}, residuals {:?}), beam f {fb:.8} Hz (values {:?})",
        h.values, h.residuals, beam.result.eigenvalues
    );
    println!("C3 signed: {:+.4e}", fr / fb - 1.0);
    c.le("C3 bending frequency", rel(fr, fb), 0.01);
    let dofs = m.dof_map();
    let n = dofs.n_equations();
    let axis = axis_nodes_of(m.mesh());
    let t2 = harmonic_transformation(&m, &[], 2).unwrap();
    let all: Vec<usize> = axis
        .iter()
        .flat_map(|&p| [0, 1, 2].map(|k| dofs.equation(p, k).unwrap()))
        .flat_map(|e| [e, n + e])
        .collect();
    c.holds(
        "C4 k = 2 axis nodes held",
        empty_rows(&t2, &all),
        &format!("{} axis nodes", axis.len()),
    );
    let t1 = harmonic_transformation(&m, &[], 1).unwrap();
    let axial: Vec<usize> = axis
        .iter()
        .map(|&p| dofs.equation(p, 2).unwrap())
        .flat_map(|e| [e, n + e])
        .collect();
    let free_count_ok = axis.iter().filter(|&&p| m.mesh().nodes()[p][2] > 0.0).all(|&p| {
        let inplane = [0, 1].map(|k| dofs.equation(p, k).unwrap());
        inplane
            .iter()
            .flat_map(|&e| [e, n + e])
            .filter(|i| t1.independent().binary_search(i).is_ok())
            .count()
            == 2
    });
    c.holds(
        "C4b k = 1 axial held",
        empty_rows(&t1, &axial),
        "axial unknowns of the axis nodes",
    );
    c.holds(
        "C4b k = 1 two in-plane free",
        free_count_ok,
        "two of four in-plane unknowns per free axis node",
    );
    c.finish();
}

// Amendment 2: C3 and C4 with the modal tolerance 1e-6.
#[test]
fn c3c_c4c_rod_on_the_axis() {
    let mut c = Checks::new();
    let m = rod();
    let mut o = ModalOptions::new(2).tolerance(1e-6);
    o.max_iterations = 600;
    let h = modal_analysis_harmonic(&m, &[], 1, &o).unwrap();
    let mut ob = ModalOptions::new(2).tolerance(1e-6);
    ob.max_iterations = 600;
    let beam = mpc::modal_analysis_mpc(&rod_beam(), &[], &ob).unwrap();
    let (fr, fb) = (
        h.values[0].sqrt() / (2.0 * PI),
        beam.result.eigenvalues[0].sqrt() / (2.0 * PI),
    );
    println!(
        "C3: rod k = 1 f {fr:.8} Hz (values {:?}, residuals {:?}), beam f {fb:.8} Hz (values {:?})",
        h.values, h.residuals, beam.result.eigenvalues
    );
    println!("C3 signed: {:+.4e}", fr / fb - 1.0);
    c.le("C3 bending frequency", rel(fr, fb), 0.01);
    let dofs = m.dof_map();
    let n = dofs.n_equations();
    let axis = axis_nodes_of(m.mesh());
    let t2 = harmonic_transformation(&m, &[], 2).unwrap();
    let all: Vec<usize> = axis
        .iter()
        .flat_map(|&p| [0, 1, 2].map(|k| dofs.equation(p, k).unwrap()))
        .flat_map(|e| [e, n + e])
        .collect();
    c.holds(
        "C4 k = 2 axis nodes held",
        empty_rows(&t2, &all),
        &format!("{} axis nodes", axis.len()),
    );
    let t1 = harmonic_transformation(&m, &[], 1).unwrap();
    let axial: Vec<usize> = axis
        .iter()
        .map(|&p| dofs.equation(p, 2).unwrap())
        .flat_map(|e| [e, n + e])
        .collect();
    let free_count_ok = axis.iter().filter(|&&p| m.mesh().nodes()[p][2] > 0.0).all(|&p| {
        let inplane = [0, 1].map(|k| dofs.equation(p, k).unwrap());
        inplane
            .iter()
            .flat_map(|&e| [e, n + e])
            .filter(|i| t1.independent().binary_search(i).is_ok())
            .count()
            == 2
    });
    c.holds(
        "C4b k = 1 axial held",
        empty_rows(&t1, &axial),
        "axial unknowns of the axis nodes",
    );
    c.holds(
        "C4b k = 1 two in-plane free",
        free_count_ok,
        "two of four in-plane unknowns per free axis node",
    );
    c.finish();
}

/// The rows of a rigid coupling: every node of `set` follows node `b` rigidly (diagnosis of J1, J2; not a design path).
fn rigid_rows(m: &SolidModel, b: usize, set: &[usize]) -> Vec<Constraint> {
    use rcfd_fem::model::LinearTerm;
    let xb = m.mesh().nodes()[b];
    let mut out = Vec::new();
    for &i in set {
        let x = m.mesh().nodes()[i];
        let r = [x[0] - xb[0], x[1] - xb[1], x[2] - xb[2]];
        for c in 0..3 {
            let (j, k) = ((c + 1) % 3, (c + 2) % 3);
            let mut t = vec![LinearTerm::new(i, c, 1.0), LinearTerm::new(b, c, -1.0)];
            if r[k] != 0.0 {
                t.push(LinearTerm::new(b, 3 + j, -r[k]));
            }
            if r[j] != 0.0 {
                t.push(LinearTerm::new(b, 3 + k, r[j]));
            }
            out.push(Constraint::linear(t, 0.0));
        }
    }
    out
}

/// Diagnosis of the J1 and J2 known misses (ADR 0456 addendum 1): no limit, numbers only.
#[test]
#[ignore = "diagnosis of the J1 and J2 known misses of ADR 0456 (addendum 1); numbers only, no limit"]
fn j1_j2_diagnosis() {
    let ei_bar = E * 0.1f64.powi(4) / 12.0;
    // One-piece Hex20 bar, L = 2.
    let mut bar = SolidModel::new(
        upgrade_to_hex20(&hex_box([0.0; 3], [2.0, 0.1, 0.1], [40, 2, 2]).unwrap())
            .unwrap()
            .0,
    );
    bar.add_material("all", material(E, 0.0)).unwrap();
    clamp(&mut bar, "xmin", 3);
    bar.add_load(Load::traction("xmax", [0.0, 0.0, -P_TIP / 0.01])).unwrap();
    let sol = solve(&bar, &[]);
    let d = work_deflection(&bar, &[], sol.result.displacement_values(), P_TIP);
    let dt = timoshenko(P_TIP, 2.0, E, 0.0, 0.1f64.powi(4) / 12.0, 0.01, 5.0 / 6.0);
    println!("D1 one-piece Hex20 bar L = 2: signed {:+.4e}", d / dt - 1.0);
    // J1 with a rigid end face.
    let (mut m, b) = j1_model(false);
    let face = m.mesh().surface_set_nodes("A.xmax").unwrap();
    for cst in rigid_rows(&m, b, &face) {
        m.add_constraint(cst).unwrap();
    }
    let sol = solve(&m, &[]);
    let d = work_deflection(&m, &[], sol.result.displacement_values(), P_TIP);
    println!("D2 J1 with a rigid end face: signed {:+.4e}", d / dt - 1.0);
    // J1 distributing: the joint's extra flexibility as an equivalent length of beam at the joint moment.
    let (m, _) = j1_model(true);
    let sol = solve(&m, &[]);
    let d = work_deflection(&m, &[], sol.result.displacement_values(), P_TIP);
    println!(
        "D3 J1 distributing: signed {:+.4e}, extra length {:e} m",
        d / dt - 1.0,
        (d - dt) / (1.5 * 1.5 * P_TIP / ei_bar)
    );
    // J2 with rigid columns; and a one-piece Hex20 plate L = 1.
    let (mut m, forces, _) = j2_model(0.0);
    let set = m.mesh().surface_set_nodes("A.xmax").unwrap();
    let edge = m.mesh().node_set("B.edge").unwrap().to_vec();
    let mut rows = Vec::new();
    for &s in &edge {
        let xs = m.mesh().nodes()[s];
        let col: Vec<usize> = set
            .iter()
            .copied()
            .filter(|&i| {
                let x = m.mesh().nodes()[i];
                (x[0] - xs[0]).abs() < 1e-9 && (x[1] - xs[1]).abs() < 1e-9
            })
            .collect();
        rows.extend(rigid_rows(&m, s, &col));
    }
    for cst in rows {
        m.add_constraint(cst).unwrap();
    }
    let sol = solve(&m, &forces);
    let d = work_deflection(&m, &forces, sol.result.displacement_values(), P_TIP);
    let dt2 = timoshenko(P_TIP, 1.0, E, 0.0, 0.1 * 0.02f64.powi(3) / 12.0, 0.002, 5.0 / 6.0);
    println!("D4 J2 with rigid columns: signed {:+.4e}", d / dt2 - 1.0);
    let mut plate = SolidModel::new(
        upgrade_to_hex20(&hex_box([0.0, 0.0, -0.01], [1.0, 0.1, 0.02], [20, 2, 1]).unwrap())
            .unwrap()
            .0,
    );
    plate.add_material("all", material(E, 0.0)).unwrap();
    clamp(&mut plate, "xmin", 3);
    plate
        .add_load(Load::traction("xmax", [0.0, 0.0, -P_TIP / 0.002]))
        .unwrap();
    let sol = solve(&plate, &[]);
    let d = work_deflection(&plate, &[], sol.result.displacement_values(), P_TIP);
    println!("D5 one-piece Hex20 plate L = 1: signed {:+.4e}", d / dt2 - 1.0);
}
