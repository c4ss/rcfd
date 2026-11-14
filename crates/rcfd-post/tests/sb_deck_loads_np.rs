//! F-523 (ADR 0441): deck pressure loads on non-parallelogram Hex8 faces. The nodal loads of `deck_pressure_loads` against those
//! of `Load::pressure` node by node on a trapezoidal face, a warped face and a strip of two trapezoids (B1 to B3), the same
//! comparison against the lumped loads `-p A/4` as the negative control (D1 to D3), and the resultant of the planar trapezoid
//! against `deck_force_moment` (R1).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::{Load, SolidMesh, SolidModel};
use rcfd_io::nastran::Deck;
use rcfd_post::solid_loads::*;
use rcfd_post::structural::*;

const P0: f64 = 1.0e5;
const TOL: f64 = 1e-9;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
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
    fn above(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value > limit;
        println!(
            "{name}: {value:.4e} (limit > {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{name} = {value:e} <= {limit:e}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

fn dist(a: Vec3, b: Vec3) -> f64 {
    vec3::norm(vec3::sub(a, b))
}

/// A `hex_box` of unit footprint with the nodes at the given positions moved (the `zmin` set copied).
fn moved(divisions: [usize; 3], moves: &[(Vec3, Vec3)]) -> SolidMesh {
    let base = hex_box([0.0, 0.0, 1.0], [1.0, 1.0, 0.2], divisions).unwrap();
    let mut nodes = base.nodes().to_vec();
    for (from, to) in moves {
        let k = nodes.iter().position(|x| dist(*x, *from) < 1e-12).unwrap();
        nodes[k] = *to;
    }
    let mut m = SolidMesh::new(nodes).unwrap();
    m.add_block(base.blocks()[0].kind(), base.blocks()[0].connectivity().to_vec())
        .unwrap();
    m.add_surface_set("zmin", base.surface_set("zmin").unwrap().to_vec())
        .unwrap();
    m
}

fn trapezoid() -> SolidMesh {
    moved(
        [1, 1, 1],
        &[
            ([0.0, 1.0, 1.0], [0.25, 1.0, 1.0]),
            ([1.0, 1.0, 1.0], [0.75, 1.0, 1.0]),
        ],
    )
}

fn warped() -> SolidMesh {
    moved([1, 1, 1], &[([1.0, 1.0, 1.0], [1.3, 1.0, 1.01])])
}

fn strip() -> SolidMesh {
    moved(
        [2, 1, 1],
        &[
            ([0.0, 1.0, 1.0], [0.25, 1.0, 1.0]),
            ([0.5, 1.0, 1.0], [0.5, 1.0, 1.0]),
            ([1.0, 1.0, 1.0], [0.75, 1.0, 1.0]),
        ],
    )
}

/// The parsed deck of the uniform pressure `P0` on the `zmin` set, its shell, and the deck's nodal loads.
fn deck_side(mesh: &SolidMesh) -> (SolidShell, Deck, Vec<NodalForceAt>) {
    let shell = shell_from_surface_set(mesh, "zmin", SurfaceParam::Plane { axes: [0, 1] }).unwrap();
    let deck = shell_deck(&shell.surface, &vec![P0; shell.surface.quads.len()], 1).unwrap();
    let deck = Deck::parse(&deck.to_string_checked().unwrap()).unwrap();
    let forces = deck_pressure_loads(mesh, "zmin", &deck, 1, TOL).unwrap();
    (shell, deck, forces)
}

/// Per-node 3-vectors of a global load vector.
fn per_node(model: &SolidModel, f: &[f64]) -> Vec<Vec3> {
    let dofs = model.dof_map();
    let mut out = vec![[0.0; 3]; model.mesh().nodes().len()];
    for (eq, &v) in f.iter().enumerate() {
        let (n, c) = dofs.node_component(eq).unwrap();
        out[n][c] += v;
    }
    out
}

/// `Load::pressure("zmin", P0)` of the model, per node.
fn reference_loads(mesh: &SolidMesh) -> Vec<Vec3> {
    let mut m = SolidModel::new(mesh.clone());
    m.add_load(Load::pressure("zmin", P0)).unwrap();
    per_node(&m, &nodal_load_vector(&m, &[]).unwrap())
}

/// The deck's forces, per node, through the same assembly (as K3 of ADR 0436).
fn deck_loads(mesh: &SolidMesh, forces: &[NodalForceAt]) -> Vec<Vec3> {
    let m = SolidModel::new(mesh.clone());
    per_node(&m, &nodal_load_vector(&m, forces).unwrap())
}

/// The lumped loads `-p A / 4` at the corners of every quad of the shell (the mutated route), per solid node.
fn lumped_loads(mesh: &SolidMesh, shell: &SolidShell) -> Vec<Vec3> {
    let mut out = vec![[0.0; 3]; mesh.nodes().len()];
    for q in &shell.surface.quads {
        let x: Vec<Vec3> = q.iter().map(|&i| shell.surface.nodes[i]).collect();
        let a = vec3::scale(vec3::cross(vec3::sub(x[2], x[0]), vec3::sub(x[3], x[1])), 0.5);
        for &i in q {
            let n = shell.solid_nodes[i];
            out[n] = vec3::add(out[n], vec3::scale(a, -P0 / 4.0));
        }
    }
    out
}

/// `max_n |a_n - b_n| / max_n |b_n|`.
fn node_by_node(a: &[Vec3], b: &[Vec3]) -> f64 {
    let big = b.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max);
    let diff = a.iter().zip(b).map(|(x, y)| dist(*x, *y)).fold(0.0, f64::max);
    diff / big
}

fn run(name: &str, mesh: &SolidMesh, b_id: &str, d_id: &str, c: &mut Checks) -> (SolidShell, Deck, Vec<NodalForceAt>) {
    let (shell, deck, forces) = deck_side(mesh);
    let f_ref = reference_loads(mesh);
    let f_deck = deck_loads(mesh, &forces);
    println!("{name}: {} nodal forces, {} quads", forces.len(), shell.surface.quads.len());
    c.at_most(
        &format!("{b_id} {name} deck vs Load::pressure node by node"),
        node_by_node(&f_deck, &f_ref),
        1e-14,
    );
    c.above(
        &format!("{d_id} {name} lumped vs Load::pressure node by node (negative control)"),
        node_by_node(&lumped_loads(mesh, &shell), &f_ref),
        1e-2,
    );
    (shell, deck, forces)
}

#[test]
fn deck_loads_on_non_parallelogram_faces() {
    let mut c = Checks(Vec::new());
    let t = trapezoid();
    let (_, deck, forces) = run("trapezoid", &t, "B1", "D1", &mut c);
    run("warped", &warped(), "B2", "D2", &mut c);
    run("strip", &strip(), "B3", "D3", &mut c);

    // R1: the planar trapezoid's resultant against deck_force_moment
    let about = [0.5, 0.5, 1.0];
    let (f_n, m_n) = nodal_force_moment(&t, &forces, about);
    let (f_d, m_d) = deck_force_moment(&deck, 1, about).unwrap();
    let pos: std::collections::HashMap<i64, Vec3> = deck.grids.iter().cloned().collect();
    let mut s = 0.0;
    for l in deck.pload4.iter().filter(|l| l.sid == 1) {
        let e = deck.elements.iter().find(|e| e.id == l.eid).unwrap();
        let x: Vec<Vec3> = e.nodes.iter().map(|g| pos[g]).collect();
        let a = vec3::scale(vec3::cross(vec3::sub(x[2], x[0]), vec3::sub(x[3], x[1])), 0.5);
        s += l.p[0].abs() * vec3::norm(a);
    }
    let l_d = deck.grids.iter().map(|(_, x)| dist(*x, about)).fold(0.0, f64::max);
    c.at_most("R1 trapezoid |F_n - F_d|/S_d", dist(f_n, f_d) / s, 1e-12);
    c.at_most("R1 trapezoid |M_n - M_d|/(S_d L_d)", dist(m_n, m_d) / (s * l_d), 1e-12);
    c.finish();
}
