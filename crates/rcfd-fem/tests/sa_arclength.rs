//! F-424 (ADR 0480): the two-bar truss of solid bars followed by arc-length through both limit points (A1 to A5, A7) and
//! load control failing at the first limit point (A6). Each registered run is made once; a miss stays as an ignored
//! `*_known_miss` test with its limit.

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace, ArcLengthOptions, PathPoint};
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::nonlinear::{solve_with, DriverExtensions, LoadStep, NewtonOptions, StepControl};
use rcfd_fem::{Constraint, ElementKind, FemError, Material, SolidMesh, SolidModel};

struct Checks {
    failed: Vec<String>,
}

impl Checks {
    fn new() -> Checks {
        Checks { failed: Vec::new() }
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{name}: {value:.6e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} > {limit:e}"));
        }
    }
    fn holds(&mut self, name: &str, ok: bool, detail: &str) {
        println!("{name}: {detail} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.failed.push(format!("{name}: {detail}"));
        }
    }
    fn finish(self) {
        assert!(self.failed.is_empty(), "missed: {:?}", self.failed);
    }
}

const E: f64 = 2.0e11;
const L0: f64 = 1.0;
const H: f64 = 0.2;
const S: f64 = 0.1;
/// The effective area of the bipyramid, `V/L0 = s^2/3` (ADR 0480 arithmetic 5).
const A: f64 = S * S / 3.0;
const APEX: usize = 2;

fn p_max() -> f64 {
    2.0 * E * A * H.powi(3) / (3.0 * 3f64.sqrt() * L0.powi(3))
}

fn p_closed(w: f64) -> f64 {
    E * A * (H - w) * (2.0 * H * w - w * w) / L0.powi(3)
}

fn volume(p: &[Vec3; 4]) -> f64 {
    let d = |a: Vec3, b: Vec3| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let (u, v, w) = (d(p[0], p[1]), d(p[0], p[2]), d(p[0], p[3]));
    u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0]) + u[2] * (v[0] * w[1] - v[1] * w[0])
}

/// The truss of D6: supports 0 and 1, apex 2, each bar a bipyramid of four Tet4 around its axis.
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
    m.add_material("all", Material::isotropic_elastic(E, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("pins", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pins", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    m
}

fn apex_w(model: &SolidModel, u: &[f64]) -> f64 {
    -u[model.dof_map().equation(APEX, 1).unwrap()]
}

#[test]
fn a1_to_a5_a7_two_bar_truss_by_arc_length() {
    let mut ck = Checks::new();
    let model = truss();
    let pm = p_max();
    let forces = [NodalForceAt::new(APEX, [0.0, -pm, 0.0])];
    let options = ArcLengthOptions::new(0.05 * H).max_points(400);
    let m2 = model.clone();
    let mut stop = |p: &PathPoint| apex_w(&m2, &p.displacement) >= 2.2 * H;
    let path = trace(&model, &[], &forces, &options, &[], &mut stop).unwrap();
    let pts = &path.points;
    println!("{} points, load scale {:e}", pts.len(), path.load_scale);
    // A1.
    let mut worst: f64 = 0.0;
    for p in pts {
        let w = apex_w(&model, &p.displacement);
        worst = worst.max((p.load_factor * pm - p_closed(w)).abs() / pm);
    }
    ck.at_most("A1 max over the points |lambda P_ref - P(w)| / P_max", worst, 1e-6);
    // A2.
    let w_last = apex_w(&model, &pts.last().unwrap().displacement);
    ck.holds(
        "A2 stopped at w >= 2.2 h within 400 points",
        path.stopped && w_last >= 2.2 * H,
        &format!(
            "stopped {}, {} points, w_last/h = {:.6}",
            path.stopped,
            pts.len(),
            w_last / H
        ),
    );
    // A3, A7.
    let mut top = f64::NEG_INFINITY;
    let mut bottom = f64::INFINITY;
    for p in pts {
        let w = apex_w(&model, &p.displacement);
        if w < H {
            top = top.max(p.load_factor);
        }
        if w > H && w < 2.0 * H {
            bottom = bottom.min(p.load_factor);
        }
    }
    ck.at_most("A3 |max_{w<h} lambda P_ref / P_max - 1|", (top - 1.0).abs(), 5e-3);
    ck.at_most("A7 |min_{h<w<2h} lambda P_ref / P_max + 1|", (bottom + 1.0).abs(), 5e-3);
    // A4: the constraint at every point, from the stored points.
    let c = path.load_scale;
    let mut prev_u = vec![0.0; model.dof_map().n_equations()];
    let mut prev_l = 0.0;
    let mut worst4: f64 = 0.0;
    for p in pts {
        let du2: f64 = p.displacement.iter().zip(&prev_u).map(|(a, b)| (a - b) * (a - b)).sum();
        let dl = p.load_factor - prev_l;
        worst4 = worst4.max((du2 + c * dl * dl - p.length * p.length).abs() / (p.length * p.length));
        prev_u = p.displacement.clone();
        prev_l = p.load_factor;
    }
    ck.at_most("A4 max |Du^2 + c Dlambda^2 - Ds^2| / Ds^2", worst4, 1e-10);
    // A5: the inertia along the path.
    let (w1, w2) = (H * (1.0 - 1.0 / 3f64.sqrt()), H * (1.0 + 1.0 / 3f64.sqrt()));
    let mut bad = Vec::new();
    for (k, p) in pts.iter().enumerate() {
        let w = apex_w(&model, &p.displacement);
        let want = if w < w1 - 0.02 * H || w > w2 + 0.02 * H {
            Some(0)
        } else if w > w1 + 0.02 * H && w < w2 - 0.02 * H {
            Some(1)
        } else {
            None
        };
        if let Some(n) = want {
            if p.negative_pivots != n {
                bad.push(format!(
                    "point {} w/h {:.4}: {} negative",
                    k + 1,
                    w / H,
                    p.negative_pivots
                ));
            }
        }
    }
    ck.holds(
        "A5 negative pivots 0, 1, 0 along the path",
        bad.is_empty(),
        &format!("{bad:?}"),
    );
    let iters: usize = pts.iter().map(|p| p.residuals.len() - 1).sum();
    let halvings: usize = pts.iter().map(|p| p.halvings).sum();
    println!("iterations {iters}, halvings {halvings}");
    ck.finish();
}

/// A hook that keeps the driver's proposal and records every converged load factor.
struct Record {
    converged: Vec<f64>,
}

impl StepControl for Record {
    fn name(&self) -> &str {
        "record"
    }
    fn load_factor(&mut self, _committed: f64, proposed: f64, _step: usize, _halvings: usize) -> rcfd_fem::Result<f64> {
        Ok(proposed)
    }
    fn converged(&mut self, increment: &LoadStep) {
        self.converged.push(increment.load_factor);
    }
}

#[test]
fn a6_load_control_fails_at_the_first_limit_point() {
    let mut ck = Checks::new();
    let model = truss();
    let pm = p_max();
    let target = 1.2 * pm;
    let forces = [NodalForceAt::new(APEX, [0.0, -target, 0.0])];
    let mut hook = Record { converged: Vec::new() };
    let ext = DriverExtensions {
        contributions: Vec::new(),
        step_control: Some(&mut hook),
    };
    let r = solve_with(&model, &[], &forces, &NewtonOptions::new(10), ext);
    let largest = hook.converged.iter().fold(0.0f64, |m, &l| m.max(l)) * target / pm;
    match r {
        Err(FemError::Solver { solver, reason }) => {
            println!("A6 load control: {solver}: {reason}");
            ck.holds(
                "A6 Err, largest converged load in [0.97, 1.0] P_max",
                (0.97..=1.0).contains(&largest),
                &format!("largest {largest:.8} P_max"),
            );
        }
        Err(other) => ck.holds("A6", false, &format!("unexpected error {other}")),
        Ok(r) => {
            let w = apex_w(&model, r.result.displacement_values());
            ck.holds(
                "A6 Ok with a snap through (w > h (1 + 1/sqrt 3))",
                w > H * (1.0 + 1.0 / 3f64.sqrt()),
                &format!("w/h = {:.6}, largest converged before {largest:.6} P_max", w / H),
            );
        }
    }
    ck.finish();
}
