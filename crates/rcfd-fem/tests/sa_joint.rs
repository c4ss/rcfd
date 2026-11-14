//! F-543 (ADR 0460): beam and shell joints to solids with consistent face weights and a rigid-face option. Checks W1 to W4,
//! R1, J1, J1R, J2, J2R and the negative controls N1 to N3 of the registration. The set-ups are copies of the J1 and J2
//! set-ups of ADR 0456 (`sa_tie.rs`, not edited) with the new joint calls.

use rcfd_core::vec3::Vec3;
use rcfd_fem::builders::hex_box;
use rcfd_fem::hex20::{traction_field_loads, upgrade_to_hex20};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::mpc;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::tie::{self, JointCoupling};
use rcfd_fem::{BeamSection, BlockProperty, Constraint, ElementKind, FemError, Load, Material, SolidMesh, SolidModel};
use std::collections::{BTreeMap, BTreeSet};

const E: f64 = 2.0e11;
const RHO: f64 = 7850.0;
const P_TIP: f64 = 1000.0;

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

fn clamp(m: &mut SolidModel, set: &str, comps: usize) {
    for c in 0..comps {
        m.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
}

fn solve(m: &SolidModel, forces: &[NodalForceAt]) -> mpc::MpcSolution {
    mpc::solve_static_mpc(m, &[], forces, &[], &StaticOptions::default()).unwrap()
}

fn work_deflection(m: &SolidModel, forces: &[NodalForceAt], u: &[f64], p: f64) -> f64 {
    let f = nodal_load_vector(m, forces).unwrap();
    f.iter().zip(u).map(|(a, b)| a * b).sum::<f64>() / p
}

fn timoshenko(p: f64, l: f64, e: f64, nu: f64, i: f64, a: f64, kappa: f64) -> f64 {
    let g = e / (2.0 * (1.0 + nu));
    p * l.powi(3) / (3.0 * e * i) + p * l / (kappa * g * a)
}

// ------------------------------------------------------------------------------------------------------------ set-ups

/// J1 of ADR 0456 without joint rows; the beam's first node at `x = 0.5 + offset` (N3), its tip at `x = 2`.
fn j1_model(offset: f64) -> (SolidModel, usize) {
    let a = upgrade_to_hex20(&hex_box([0.0; 3], [0.5, 0.1, 0.1], [10, 2, 2]).unwrap())
        .unwrap()
        .0;
    let nb = 15;
    let x0 = 0.5 + offset;
    let nodes: Vec<Vec3> = (0..=nb)
        .map(|i| [x0 + (2.0 - x0) * i as f64 / nb as f64, 0.05, 0.05])
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
    (m, joint_node)
}

/// J2 of ADR 0456 without joint rows, its tip forces, and the shell edge nodes; the node set `B.one` holds one edge node (N1).
fn j2_model() -> (SolidModel, Vec<NodalForceAt>, Vec<usize>) {
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
    b.add_node_set("one", vec![id(0, 1)]).unwrap();
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
    let edge = (0..=ny).map(|j| n0 + id(0, j)).collect();
    (m, forces, edge)
}

fn add(m: &mut SolidModel, rows: Vec<Constraint>) {
    for c in rows {
        m.add_constraint(c).unwrap();
    }
}

// ------------------------------------------------------------------------------------------------------------ measures

/// The forces on the solid nodes dual to the generalised forces `q` on the dependent reference components: each row is
/// `y_d - sum c u = 0` with `y_d` its first term, so `f_ic = sum_d q_d c_d,ic` over the terms of the nodes not in `refs`.
fn dual_forces(
    rows: &[Constraint],
    q: &BTreeMap<(usize, usize), f64>,
    refs: &[usize],
) -> BTreeMap<(usize, usize), f64> {
    let mut f = BTreeMap::new();
    for r in rows {
        let Constraint::Linear { terms, .. } = r else {
            panic!("a linear row")
        };
        let d = (terms[0].node, terms[0].component);
        assert_eq!(terms[0].coefficient, 1.0);
        let Some(&qd) = q.get(&d) else { continue };
        for t in &terms[1..] {
            if !refs.contains(&t.node) {
                *f.entry((t.node, t.component)).or_insert(0.0) += -qd * t.coefficient;
            }
        }
    }
    f
}

/// Max abs difference over max abs of the reference, over every component of the reference loads' nodes and of `f`.
fn force_error(f: &BTreeMap<(usize, usize), f64>, reference: &[NodalForceAt]) -> f64 {
    let mut r: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for l in reference {
        for c in 0..3 {
            *r.entry((l.node, c)).or_insert(0.0) += l.force[c];
        }
    }
    let keys: BTreeSet<(usize, usize)> = r.keys().chain(f.keys()).copied().collect();
    let scale = r.values().fold(0.0f64, |m, v| m.max(v.abs()));
    keys.iter()
        .map(|k| (f.get(k).copied().unwrap_or(0.0) - r.get(k).copied().unwrap_or(0.0)).abs())
        .fold(0.0, f64::max)
        / scale
}

/// The largest residual of the rows for the rigid motion `u = t + theta x (x - x0)` (rotations `theta` on six-unknown
/// nodes), over the largest nodal value involved.
fn rigid_residual(m: &SolidModel, rows: &[Constraint], t: Vec3, theta: Vec3) -> f64 {
    let x0 = [0.3, -0.2, 0.1];
    let val = |node: usize, c: usize| -> f64 {
        if c >= 3 {
            return theta[c - 3];
        }
        let x = m.mesh().nodes()[node];
        let r = [x[0] - x0[0], x[1] - x0[1], x[2] - x0[2]];
        let w = [
            theta[1] * r[2] - theta[2] * r[1],
            theta[2] * r[0] - theta[0] * r[2],
            theta[0] * r[1] - theta[1] * r[0],
        ];
        t[c] + w[c]
    };
    let (mut res, mut scale) = (0.0f64, 0.0f64);
    for r in rows {
        let Constraint::Linear { terms, value, .. } = r else {
            panic!("a linear row")
        };
        let mut s = -value;
        for term in terms {
            let v = val(term.node, term.component);
            scale = scale.max(v.abs());
            s += term.coefficient * v;
        }
        res = res.max(s.abs());
    }
    res / scale
}

fn six_motions() -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for k in 0..3 {
        let mut t = [0.0; 3];
        t[k] = 1.0;
        out.push((t, [0.0; 3]));
    }
    for k in 0..3 {
        let mut th = [0.0; 3];
        th[k] = 0.1;
        out.push(([0.0; 3], th));
    }
    out
}

// ------------------------------------------------------------------------------------------------------------ checks

#[test]
fn w1_to_w4_consistent_nodal_forces() {
    let mut c = Checks::new();
    // W1, W2: the beam node of J1 at the centroid of A.xmax.
    let (m, b) = j1_model(0.0);
    let rows = tie::beam_to_solid_with(&m, b, "A.xmax", JointCoupling::Consistent).unwrap();
    let s = 0.01;
    let q = BTreeMap::from([((b, 2), -P_TIP)]);
    let f = dual_forces(&rows, &q, &[b]);
    let reference = traction_field_loads(m.mesh(), "A.xmax", |_| [0.0, 0.0, -P_TIP / s]).unwrap();
    c.le("W1 uniform force, beam", force_error(&f, &reference), 1e-12);
    let mom = 1000.0;
    let inertia = 0.1f64.powi(4) / 12.0;
    let q = BTreeMap::from([((b, 4), mom)]);
    let f = dual_forces(&rows, &q, &[b]);
    let reference = traction_field_loads(m.mesh(), "A.xmax", |x| [mom * (x[2] - 0.05) / inertia, 0.0, 0.0]).unwrap();
    c.le("W2 moment about y, beam", force_error(&f, &reference), 1e-12);
    // W3, W4: the shell edge of J2.
    let (m, _, edge) = j2_model();
    let rows = tie::shell_to_solid_with(&m, "B.edge", "A.xmax", JointCoupling::Consistent).unwrap();
    let lens = [0.025, 0.05, 0.025];
    let q: BTreeMap<(usize, usize), f64> = edge
        .iter()
        .zip(lens)
        .map(|(&n, l)| ((n, 2), -P_TIP * l / 0.1))
        .collect();
    let f = dual_forces(&rows, &q, &edge);
    let reference = traction_field_loads(m.mesh(), "A.xmax", |_| [0.0, 0.0, -P_TIP / (0.1 * 0.02)]).unwrap();
    c.le("W3 uniform line force, shell", force_error(&f, &reference), 1e-12);
    let face = m.mesh().surface_set_nodes("A.xmax").unwrap();
    let fmax = f.values().fold(0.0f64, |a, v| a.max(v.abs()));
    let loaded = face
        .iter()
        .filter(|&&n| (0..3).any(|k| f.get(&(n, k)).is_some_and(|v| v.abs() > 1e-12 * fmax)))
        .count();
    c.holds(
        "W3 every face node loaded",
        loaded == face.len() && face.len() == 13,
        &format!("{loaded} of {} face nodes", face.len()),
    );
    let lm = 1000.0;
    let i1 = 0.02f64.powi(3) / 12.0;
    let q: BTreeMap<(usize, usize), f64> = edge.iter().zip(lens).map(|(&n, l)| ((n, 4), lm * l)).collect();
    let f = dual_forces(&rows, &q, &edge);
    let reference = traction_field_loads(m.mesh(), "A.xmax", |x| [lm * x[2] / i1, 0.0, 0.0]).unwrap();
    c.le("W4 line moment about y, shell", force_error(&f, &reference), 1e-12);
    c.finish();
}

#[test]
fn r1_n3_six_rigid_motions() {
    let mut c = Checks::new();
    let (m1, b) = j1_model(0.0);
    let (m3, b3) = j1_model(0.05);
    let (m2, _, _) = j2_model();
    for coupling in [JointCoupling::Consistent, JointCoupling::Rigid] {
        let cases = [
            ("J1", &m1, tie::beam_to_solid_with(&m1, b, "A.xmax", coupling).unwrap()),
            (
                "J2",
                &m2,
                tie::shell_to_solid_with(&m2, "B.edge", "A.xmax", coupling).unwrap(),
            ),
            (
                "N3 lever",
                &m3,
                tie::beam_to_solid_with(&m3, b3, "A.xmax", coupling).unwrap(),
            ),
        ];
        for (name, m, rows) in &cases {
            let worst = six_motions()
                .into_iter()
                .map(|(t, th)| rigid_residual(m, rows, t, th))
                .fold(0.0, f64::max);
            let id = if name.starts_with("N3") { "N3" } else { "R1" };
            println!("{name} {coupling:?}: {} rows", rows.len());
            c.le(&format!("{id} {name} {coupling:?} six rigid motions"), worst, 1e-12);
        }
    }
    c.finish();
}

#[test]
fn j1_j2_tip_deflections() {
    let mut c = Checks::new();
    let dt1 = timoshenko(P_TIP, 2.0, E, 0.0, 0.1f64.powi(4) / 12.0, 0.01, 5.0 / 6.0);
    for (id, coupling, limit) in [
        ("J1", JointCoupling::Consistent, 1e-2),
        ("J1R", JointCoupling::Rigid, 1e-3),
    ] {
        let (mut m, b) = j1_model(0.0);
        let rows = tie::beam_to_solid_with(&m, b, "A.xmax", coupling).unwrap();
        add(&mut m, rows);
        let sol = solve(&m, &[]);
        let d = work_deflection(&m, &[], sol.result.displacement_values(), P_TIP);
        println!(
            "{id}: delta {d:.10e}, closed form {dt1:.10e}, signed {:+.4e}",
            d / dt1 - 1.0
        );
        c.le(&format!("{id} tip deflection"), rel(d, dt1), limit);
    }
    let dt2 = timoshenko(P_TIP, 1.0, E, 0.0, 0.1 * 0.02f64.powi(3) / 12.0, 0.002, 5.0 / 6.0);
    for (id, coupling, limit) in [
        ("J2", JointCoupling::Consistent, 2e-2),
        ("J2R", JointCoupling::Rigid, 2e-3),
    ] {
        let (mut m, forces, _) = j2_model();
        let rows = tie::shell_to_solid_with(&m, "B.edge", "A.xmax", coupling).unwrap();
        add(&mut m, rows);
        let sol = solve(&m, &forces);
        let d = work_deflection(&m, &forces, sol.result.displacement_values(), P_TIP);
        println!(
            "{id}: delta {d:.10e}, closed form {dt2:.10e}, signed {:+.4e}",
            d / dt2 - 1.0
        );
        c.le(&format!("{id} tip deflection"), rel(d, dt2), limit);
    }
    c.finish();
}

#[test]
fn n1_n2_refusals() {
    let mut c = Checks::new();
    let (m, _, _) = j2_model();
    match tie::shell_to_solid_with(&m, "B.one", "A.xmax", JointCoupling::Consistent) {
        Err(FemError::InvalidValue { reason, .. }) => {
            println!("N1: {reason}");
            c.holds("N1 refused naming the set", reason.contains("'B.one'"), &reason);
        }
        other => c.holds("N1 refused", false, &format!("{:?}", other.map(|v| v.len()))),
    }
    let (m, _) = j1_model(0.0);
    match tie::beam_to_solid_with(&m, 0, "A.xmax", JointCoupling::Consistent) {
        Err(FemError::InvalidValue { reason, .. }) => {
            println!("N2: {reason}");
            c.holds(
                "N2 refused naming the node",
                reason.contains("coupling node 0 "),
                &reason,
            );
        }
        other => c.holds("N2 refused", false, &format!("{:?}", other.map(|v| v.len()))),
    }
    c.finish();
}
