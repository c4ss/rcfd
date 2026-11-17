//! F-797 (ADR 0726): the arc-length driver from a given state under prescribed-displacement loading. The fast test: a
//! closed-form snap-back under a prescribed displacement (ADR 0480's two-bar truss with a soft series bar whose far end is
//! prescribed, D3) traced by `arclength::trace_from` through both limit points; a restart from a converged point with its
//! increment against the continuing trace; the default `trace` against `trace_from` with zero prescribed values; the
//! refusals. The heavy record: ADR 0711's bordered trace of rule L's 16^3 punch branch rebuilt (the helpers are copies of
//! those of `si_eas_hg17.rs`; existing tests are not edited) and the arc-length trace from its state at 0.9645 against the
//! bordered Newton at the same amplitudes (D4). Each registered run is made once; a miss stays as an ignored
//! `*_known_miss` test with its limit. Every aggregated value is required finite before it is compared.

use rcfd_core::vec3::Vec3;
use rcfd_fem::arclength::{trace, trace_from, ArcLengthOptions, ArcLengthPath, PathPoint, PathStart};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::{prescribed_equations, reduce, Prescribed};
use rcfd_fem::element::ElementState;
use rcfd_fem::loads::NodalForceAt;
use rcfd_fem::material::HyperelasticEnergy;
use rcfd_fem::nl_incompatible::IncompatibleStabilisation;
use rcfd_fem::nonlinear::{
    initial_states, internal_force, solve_from, solve_increment, solve_with, tangent, Contribution, DriverExtensions,
    LoadStep, NewtonOptions, StepControl,
};
use rcfd_fem::solve::DirectFactor;
use rcfd_fem::{BlockProperty, Constraint, ElementKind, FemError, Material, SolidFormulation, SolidMesh, SolidModel};
use rcfd_linalg::eigen::{lowest_eigenpairs, Factorisation, ResidualTest, Shift, SubspaceOptions};
use rcfd_linalg::skyline::PivotRule;
use rcfd_linalg::sparse_chol::{SparseOptions, SparseOrdering, SupernodalLdlt};
use rcfd_linalg::CsrMatrix;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed. A value that is
/// not finite misses.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Checks {
        Checks(Vec::new())
    }
    fn at_most(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value.is_finite() && value <= limit;
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
    fn holds(&mut self, name: &str, ok: bool) {
        println!("{name}: {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(name.to_string());
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

fn norm(v: impl Iterator<Item = f64>) -> f64 {
    v.map(|x| x * x).sum::<f64>().sqrt()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn max_abs(v: impl Iterator<Item = f64>) -> f64 {
    v.fold(0.0f64, |w, x| if x.is_finite() { w.max(x.abs()) } else { f64::NAN })
}

fn key_of(e: &FemError) -> Option<&'static str> {
    match e {
        FemError::InvalidValue { key, .. } | FemError::FieldLength { key, .. } => Some(key),
        _ => None,
    }
}

/// The constraint `|Du_F|^2 + c Dlambda^2 = Ds^2` of every point from the previous one (`(u, lambda)` of the start
/// first), relative; the largest over the points.
fn constraint_error(free: &[usize], start: (&[f64], f64), points: &[PathPoint], c: f64) -> f64 {
    let (mut pu, mut pl) = (start.0.to_vec(), start.1);
    let mut worst = 0.0f64;
    for p in points {
        let du2: f64 = free
            .iter()
            .map(|&q| (p.displacement[q] - pu[q]) * (p.displacement[q] - pu[q]))
            .sum();
        let dl = p.load_factor - pl;
        let e = (du2 + c * dl * dl - p.length * p.length).abs() / (p.length * p.length);
        worst = if e.is_finite() { worst.max(e) } else { f64::NAN };
        pu = p.displacement.clone();
        pl = p.load_factor;
    }
    worst
}

/// Whether every point carries `lambda v` on the prescribed equations (`f64` equality).
fn prescribed_follow(fixed: &[(usize, f64)], points: &[PathPoint]) -> bool {
    points
        .iter()
        .all(|p| fixed.iter().all(|&(eq, v)| p.displacement[eq] == p.load_factor * v))
}

fn free_of(m: &SolidModel, fixed: &[(usize, f64)]) -> Vec<usize> {
    let n = m.dof_map().n_equations();
    let mut is_fixed = vec![false; n];
    for &(eq, _) in fixed {
        is_fixed[eq] = true;
    }
    (0..n).filter(|&q| !is_fixed[q]).collect()
}

// ============================================================================ D3: the truss of ADR 0480 and the series bar

const E: f64 = 2.0e11;
const L0: f64 = 1.0;
const H: f64 = 0.2;
const S: f64 = 0.1;
/// The effective area of the bipyramid, `V/L0 = s^2/3` (ADR 0480 arithmetic 5).
const A: f64 = S * S / 3.0;
const APEX: usize = 2;
/// The series bar (D3): its material, length, the prescribed displacement of its far end at load 1.
const E_S: f64 = E / 50.0;
const L_S: f64 = 1.0;
const D_TOP: f64 = 1.0;
const TOP: usize = 3;

fn p_max() -> f64 {
    2.0 * E * A * H.powi(3) / (3.0 * 3f64.sqrt() * L0.powi(3))
}

fn p_closed(w: f64) -> f64 {
    E * A * (H - w) * (2.0 * H * w - w * w) / L0.powi(3)
}

fn dp_closed(w: f64) -> f64 {
    E * A * (3.0 * (w - H) * (w - H) - H * H) / L0.powi(3)
}

/// The stretch `r` of the series bar under the force `P(w)`: the root of `E_s A r (1 - r^2) / 2 = P(w)` through `r = 1`
/// (Newton from 1, 100 iterations).
fn r_closed(w: f64) -> f64 {
    let p = p_closed(w);
    let mut r = 1.0f64;
    for _ in 0..100 {
        let g = E_S * A * r * (1.0 - r * r) / 2.0 - p;
        let dg = E_S * A * (1.0 - 3.0 * r * r) / 2.0;
        r -= g / dg;
    }
    r
}

/// The closed form `lambda(w) = (w + L_s (1 - r(w))) / d`.
fn lambda_closed(w: f64) -> f64 {
    (w + L_S * (1.0 - r_closed(w))) / D_TOP
}

/// `dlambda / dw = (1 + P'(w) / k_t) / d`, `k_t = -dN/dr / L_s`.
fn dlambda_closed(w: f64) -> f64 {
    let r = r_closed(w);
    let kt = -E_S * A * (1.0 - 3.0 * r * r) / 2.0 / L_S;
    (1.0 + dp_closed(w) / kt) / D_TOP
}

fn bisect(f: impl Fn(f64) -> f64, mut a: f64, mut b: f64) -> f64 {
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if f(a) * f(m) <= 0.0 {
            b = m;
        } else {
            a = m;
        }
    }
    0.5 * (a + b)
}

fn volume(p: &[Vec3; 4]) -> f64 {
    let d = |a: Vec3, b: Vec3| [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let (u, v, w) = (d(p[0], p[1]), d(p[0], p[2]), d(p[0], p[3]));
    u[0] * (v[1] * w[2] - v[2] * w[1]) - u[1] * (v[0] * w[2] - v[2] * w[0]) + u[2] * (v[0] * w[1] - v[1] * w[0])
}

/// A bar from node `p` to node `q` of length `len`: a bipyramid of four Tet4 around its axis (ADR 0480 D6).
fn bar(nodes: &mut Vec<Vec3>, conn: &mut Vec<usize>, p: usize, q: usize, len: f64) {
    let (pp, pq) = (nodes[p], nodes[q]);
    let a0 = [(pq[0] - pp[0]) / len, (pq[1] - pp[1]) / len];
    let n0 = [-a0[1], a0[0]];
    let m = [(pp[0] + pq[0]) / 2.0, (pp[1] + pq[1]) / 2.0];
    let first = nodes.len();
    for (cn, cz) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
        nodes.push([m[0] + cn * S / 2.0 * n0[0], m[1] + cn * S / 2.0 * n0[1], cz * S / 2.0]);
    }
    for i in 0..4 {
        let mut t = [p, q, first + i, first + (i + 1) % 4];
        if volume(&[nodes[t[0]], nodes[t[1]], nodes[t[2]], nodes[t[3]]]) < 0.0 {
            t.swap(2, 3);
        }
        conn.extend_from_slice(&t);
    }
}

/// ADR 0480's truss (`series = false`, nodes 0, 1 the supports, 2 the apex) or the model of D3 with the series bar from
/// the apex to the top node 3, prescribed to `x = 0`, `y = -d` at load 1.
fn truss(series: bool) -> SolidModel {
    let a = (L0 * L0 - H * H).sqrt();
    let mut nodes: Vec<Vec3> = vec![[-a, 0.0, 0.0], [a, 0.0, 0.0], [0.0, H, 0.0]];
    if series {
        nodes.push([0.0, H + L_S, 0.0]);
    }
    let mut conn = Vec::new();
    for b in 0..2usize {
        bar(&mut nodes, &mut conn, b, APEX, L0);
    }
    let n_bars = conn.len() / 4;
    if series {
        bar(&mut nodes, &mut conn, APEX, TOP, L_S);
    }
    let nn = nodes.len();
    let mut mesh = SolidMesh::new(nodes).unwrap();
    mesh.add_block(ElementKind::Tet4, conn).unwrap();
    let ne = mesh.n_elements();
    mesh.add_element_set("bars", (0..n_bars).collect()).unwrap();
    mesh.add_node_set("pins", vec![0, 1]).unwrap();
    mesh.add_node_set("every", (0..nn).collect()).unwrap();
    if series {
        mesh.add_element_set("spring", (n_bars..ne).collect()).unwrap();
        mesh.add_node_set("top", vec![TOP]).unwrap();
    }
    let mut m = SolidModel::new(mesh);
    m.add_material("bars", Material::isotropic_elastic(E, 0.0, 7850.0, 0.0).unwrap())
        .unwrap();
    m.add_constraint(Constraint::displacement("pins", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("pins", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("every", 2, 0.0)).unwrap();
    if series {
        m.add_material("spring", Material::isotropic_elastic(E_S, 0.0, 7850.0, 0.0).unwrap())
            .unwrap();
        m.add_constraint(Constraint::displacement("top", 0, 0.0)).unwrap();
        m.add_constraint(Constraint::displacement("top", 1, -D_TOP)).unwrap();
    }
    m
}

fn apex_w(model: &SolidModel, u: &[f64]) -> f64 {
    -u[model.dof_map().equation(APEX, 1).unwrap()]
}

fn same_points(a: &[PathPoint], b: &[PathPoint], residuals: bool) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(p, q)| {
            p.load_factor == q.load_factor
                && p.displacement.len() == q.displacement.len()
                && p.displacement.iter().zip(&q.displacement).all(|(x, y)| x == y)
                && p.length == q.length
                && p.halvings == q.halvings
                && p.negative_pivots == q.negative_pivots
                && (!residuals || p.residuals == q.residuals)
        })
}

#[test]
fn f797_fast() {
    let mut ck = Checks::new();
    let model = truss(true);
    let dofs = model.dof_map();
    let fixed = prescribed_equations(&model, &[]).unwrap();
    let free = free_of(&model, &fixed);
    let top_y = dofs.equation(TOP, 1).unwrap();
    let w1 = bisect(dlambda_closed, 0.0, H);
    let w2 = bisect(dlambda_closed, H, 2.0 * H);
    let (l_max, l_min) = (lambda_closed(w1), lambda_closed(w2));
    println!(
        "closed form: w_1/h {:.9}, w_2/h {:.9}, lambda_max {l_max:.12}, lambda_min {l_min:.12}",
        w1 / H,
        w2 / H
    );

    // FC1 to FC6: the snap-back from the unloaded start.
    let ds = 0.01;
    let options = ArcLengthOptions::new(ds).max_points(600);
    let m2 = model.clone();
    let mut stop = |p: &PathPoint| apex_w(&m2, &p.displacement) >= 1.8 * H;
    let t0 = Instant::now();
    let start0 = PathStart::unloaded(&model).unwrap();
    let path = match trace_from(&model, &[], &[], &options, &[], &start0, &mut stop) {
        Ok(p) => p,
        Err(e) => {
            ck.holds(&format!("FC: trace_from on the snap-back ({e})"), false);
            ck.finish();
            return;
        }
    };
    let pts = &path.points;
    let iters: usize = pts.iter().map(|p| p.residuals.len() - 1).sum();
    let halvings: usize = pts.iter().map(|p| p.halvings).sum();
    println!(
        "FC: {} points, load scale {:e}, {iters} iterations, {halvings} halvings, {:.2} s",
        pts.len(),
        path.load_scale,
        t0.elapsed().as_secs_f64()
    );
    let mut worst = 0.0f64;
    for p in pts {
        let w = apex_w(&model, &p.displacement);
        let e = (p.load_factor - lambda_closed(w)).abs();
        worst = if e.is_finite() { worst.max(e) } else { f64::NAN };
    }
    ck.at_most("FC1: max |lambda_j - lambda(w_j)|", worst, 1e-9);
    let w_last = apex_w(&model, &pts.last().unwrap().displacement);
    ck.holds(
        &format!(
            "FC2: stopped at w >= 1.8 h within 600 points (stopped {}, {} points, w_last/h {:.6})",
            path.stopped,
            pts.len(),
            w_last / H
        ),
        path.stopped && w_last >= 1.8 * H,
    );
    let (mut top, mut bottom) = (f64::NEG_INFINITY, f64::INFINITY);
    for p in pts {
        let w = apex_w(&model, &p.displacement);
        if w < H {
            top = top.max(p.load_factor);
        }
        if w > H && w < 1.8 * H {
            bottom = bottom.min(p.load_factor);
        }
    }
    ck.at_most(
        "FC3: |max_{w<h} lambda / lambda_max - 1|",
        (top / l_max - 1.0).abs(),
        5e-3,
    );
    ck.at_most(
        "FC3: |min_{h<w<1.8h} lambda / lambda_min - 1|",
        (bottom / l_min - 1.0).abs(),
        5e-3,
    );
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
        &format!("FC4: negative pivots 0, 1, 0 along the path {bad:?}"),
        bad.is_empty(),
    );
    ck.at_most(
        "FC5: max |Du_F^2 + c Dlambda^2 - Ds^2| / Ds^2",
        constraint_error(&free, (&start0.displacement, 0.0), pts, path.load_scale),
        1e-10,
    );
    ck.holds(
        "FC6: the top node at -lambda d at every point",
        pts.iter().all(|p| p.displacement[top_y] == p.load_factor * -D_TOP) && prescribed_follow(&fixed, pts),
    );

    // GS: the restart from the first point with w > h.
    match pts.iter().position(|p| apex_w(&model, &p.displacement) > H) {
        Some(k) if k >= 1 && k + 20 < pts.len() => {
            ck.holds(
                &format!("GS: point {} (w > h) has the full length", k + 1),
                pts[k].length == ds,
            );
            let mut never = |_: &PathPoint| false;
            let first = trace_from(
                &model,
                &[],
                &[],
                &options.clone().max_points(k + 1),
                &[],
                &start0,
                &mut never,
            );
            match first {
                Ok(fp) if same_points(&fp.points, &pts[..=k], true) => {
                    let du: Vec<f64> = pts[k]
                        .displacement
                        .iter()
                        .zip(&pts[k - 1].displacement)
                        .map(|(a, b)| a - b)
                        .collect();
                    let start = PathStart::new(pts[k].load_factor, pts[k].displacement.clone(), fp.states.clone())
                        .increment(du, pts[k].load_factor - pts[k - 1].load_factor);
                    let ro = options.clone().max_points(20).load_scale(path.load_scale);
                    match trace_from(&model, &[], &[], &ro, &[], &start, &mut never) {
                        Ok(rp) => {
                            println!(
                                "GS: restart at point {} (lambda {:.9}, decreasing: {}), {} points",
                                k + 1,
                                pts[k].load_factor,
                                pts[k].load_factor < pts[k - 1].load_factor,
                                rp.points.len()
                            );
                            ck.holds(
                                "GS: the 20 points of the restart equal points k+1 .. k+20 (f64 equality)",
                                same_points(&rp.points, &pts[k + 1..k + 21], false),
                            );
                        }
                        Err(e) => ck.holds(&format!("GS: the restart ({e})"), false),
                    }
                }
                Ok(_) => ck.holds("GS: the trace to point k reproduces the first k points", false),
                Err(e) => ck.holds(&format!("GS: the trace to point k ({e})"), false),
            }
        }
        _ => ck.holds("GS: a point with w > h and 20 after it", false),
    }

    // G0: the default trace against trace_from with zero prescribed values.
    let t480 = truss(false);
    let pm = p_max();
    let forces = [NodalForceAt::new(APEX, [0.0, -pm, 0.0])];
    let o480 = ArcLengthOptions::new(0.05 * H).max_points(50);
    let mut never = |_: &PathPoint| false;
    let a = trace(&t480, &[], &forces, &o480, &[], &mut never);
    let b = trace_from(
        &t480,
        &[],
        &forces,
        &o480,
        &[],
        &PathStart::unloaded(&t480).unwrap(),
        &mut never,
    );
    match (a, b) {
        (Ok(a), Ok(b)) => {
            println!("G0: {} and {} points", a.points.len(), b.points.len());
            ck.holds(
                "G0: trace and trace_from equal (lambda, u, length, halvings, pivots, residuals)",
                a.points.len() == 50 && same_points(&a.points, &b.points, true) && a.load_scale == b.load_scale,
            );
        }
        (a, b) => ck.holds(&format!("G0: both traces ({:?}, {:?})", a.err(), b.err()), false),
    }

    // N1 to N5: the refusals.
    let refused = |r: rcfd_fem::Result<ArcLengthPath>, key: &str, length: bool| -> bool {
        match r {
            Err(e) => {
                println!("refused: {e}");
                key_of(&e) == Some(key) && (!length || matches!(e, FemError::FieldLength { .. }))
            }
            Ok(_) => false,
        }
    };
    let mut never = |_: &PathPoint| false;
    ck.holds(
        "N1: trace refuses the non-zero prescribed value (key 'arc length')",
        refused(trace(&model, &[], &[], &options, &[], &mut never), "arc length", false),
    );
    let mut bad_start = vec![0.0; dofs.n_equations()];
    bad_start[top_y] = 0.1 * -D_TOP + 1e-3;
    let st = PathStart::new(0.1, bad_start, initial_states(&model).unwrap());
    ck.holds(
        "N2: trace_from refuses a start off lambda_0 v (key 'arc-length start')",
        refused(
            trace_from(&model, &[], &[], &options, &[], &st, &mut never),
            "arc-length start",
            false,
        ),
    );
    let retry = options.clone().newton(NewtonOptions::new(1).indefinite_retry(true));
    let indef = options.clone().newton(NewtonOptions::new(1).indefinite_tangent(true));
    ck.holds(
        "N3: trace_from refuses indefinite_retry (key 'indefinite_retry')",
        refused(
            trace_from(&model, &[], &[], &retry, &[], &start0, &mut never),
            "indefinite_retry",
            false,
        ),
    );
    ck.holds(
        "N3: trace_from refuses indefinite_tangent (key 'indefinite_tangent')",
        refused(
            trace_from(&model, &[], &[], &indef, &[], &start0, &mut never),
            "indefinite_tangent",
            false,
        ),
    );
    ck.holds(
        "N4: trace_from refuses no load and no prescribed value (key 'arc length')",
        refused(
            trace_from(
                &t480,
                &[],
                &[],
                &o480,
                &[],
                &PathStart::unloaded(&t480).unwrap(),
                &mut never,
            ),
            "arc length",
            false,
        ),
    );
    let short = PathStart::new(0.0, vec![0.0; dofs.n_equations() - 1], initial_states(&model).unwrap());
    ck.holds(
        "N5: trace_from refuses a short displacement (FieldLength, key 'arc-length start')",
        refused(
            trace_from(&model, &[], &[], &options, &[], &short, &mut never),
            "arc-length start",
            true,
        ),
    );
    ck.finish();
}

// ============================================================================ the heavy record: the materials and the block

const MU: f64 = 1.0e6;
const NU: f64 = 0.49;

fn kappa_of(nu: f64) -> f64 {
    2.0 * MU * (1.0 + nu) / (3.0 * (1.0 - 2.0 * nu))
}

/// The neo-Hookean material of ADR 0636 (density 1000).
fn neo_hookean() -> Material {
    Material::hyperelastic(HyperelasticEnergy::neo_hookean(MU, kappa_of(NU)).unwrap(), 1000.0).unwrap()
}

const SUPERNODAL: DirectFactor = DirectFactor::SupernodalNestedDissection;

/// Rule L: the incompatible-modes Hex8 with `IncompatibleStabilisation::LinearStress` (ADR 0711).
fn model_of(mesh: SolidMesh) -> SolidModel {
    let mut mesh = mesh;
    mesh.set_block_property(0, BlockProperty::solid(SolidFormulation::Incompatible))
        .unwrap();
    let mut m = SolidModel::new(mesh);
    m.set_incompatible_stabilisation(IncompatibleStabilisation::LinearStress);
    m.add_material("all", neo_hookean()).unwrap();
    m
}

const DELTA: f64 = 0.1;
const TOL: f64 = 1e-9;

fn nodes_where(mesh: &SolidMesh, set: &str, keep: impl Fn(&Vec3) -> bool) -> Vec<usize> {
    mesh.node_set(set)
        .unwrap()
        .iter()
        .copied()
        .filter(|&n| keep(&mesh.nodes()[n]))
        .collect()
}

fn block_mesh(n: usize) -> SolidMesh {
    let mut mesh = hex_box([0.0; 3], [1.0, 1.0, 1.0], [n, n, n]).unwrap();
    let punch = nodes_where(&mesh, "zmax", |p| p[0] <= 0.5 + TOL && p[1] <= 0.5 + TOL);
    let corner = nodes_where(&mesh, "zmax", |p| (p[0] - 1.0).abs() < TOL && (p[1] - 1.0).abs() < TOL);
    mesh.add_node_set("punch", punch).unwrap();
    mesh.add_node_set("corner", corner).unwrap();
    mesh
}

/// The model of the block with the punch at `-0.1` (the die constraints of ADR 0678 reading c1).
fn block_model(mesh: &SolidMesh) -> SolidModel {
    let mut m = model_of(mesh.clone());
    for (set, comp, value) in [
        ("xmin", 0, 0.0),
        ("xmax", 0, 0.0),
        ("ymin", 1, 0.0),
        ("ymax", 1, 0.0),
        ("zmin", 2, 0.0),
        ("punch", 2, -DELTA),
    ] {
        m.add_constraint(Constraint::displacement(set, comp, value)).unwrap();
    }
    m
}

/// The block model without its punch constraint (the punch given as a prescribed list in the fixed-load solves).
fn block_model_free_punch(mesh: &SolidMesh) -> SolidModel {
    let mut m = model_of(mesh.clone());
    for (set, comp) in [("xmin", 0), ("xmax", 0), ("ymin", 1), ("ymax", 1), ("zmin", 2)] {
        m.add_constraint(Constraint::displacement(set, comp, 0.0)).unwrap();
    }
    m
}

fn options(steps: usize) -> NewtonOptions {
    NewtonOptions::new(steps)
        .threads(4)
        .record_active_set(true)
        .factor(SUPERNODAL)
}

/// The error text of the EC1 solve of ADR 0678 (`f747-scratch/run3-heavy.log`, line 23).
const ADR0678_TEXT: &str = "solver newton: load step 10 of 10 (load factor 9.625e-1 to 9.656250000000001e-1) after 5 \
                            halvings: singular tangent: pivot -4.651826504388123e6 at node 4768 component 0 (diagonal \
                            2.8253460477227545e6)";

// ============================================================================ the observer (ADR 0688 D1)

struct AttemptLog {
    committed: f64,
    attempted: f64,
    converged: Option<bool>,
    iterates: Vec<Vec<f64>>,
}

#[derive(Default)]
struct Log {
    attempts: Vec<AttemptLog>,
}

struct Watch<'a> {
    log: &'a RefCell<Log>,
}

impl StepControl for Watch<'_> {
    fn name(&self) -> &str {
        "watch"
    }
    fn load_factor(&mut self, committed: f64, proposed: f64, _step: usize, _halvings: usize) -> rcfd_fem::Result<f64> {
        self.log.borrow_mut().attempts.push(AttemptLog {
            committed,
            attempted: proposed,
            converged: None,
            iterates: Vec::new(),
        });
        Ok(proposed)
    }
    fn converged(&mut self, _increment: &LoadStep) {
        if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
            a.converged = Some(true);
        }
    }
    fn failed(&mut self, _committed: f64, _attempted: f64) {
        if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
            a.converged = Some(false);
        }
    }
}

struct Observer<'a> {
    log: &'a RefCell<Log>,
}

impl Contribution for Observer<'_> {
    fn name(&self) -> &str {
        "observer"
    }
    fn add(
        &self,
        u: &[f64],
        _load_factor: f64,
        _force: &mut [f64],
        _tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        if let Some(a) = self.log.borrow_mut().attempts.last_mut() {
            if a.converged.is_none() {
                a.iterates.push(u.to_vec());
            }
        }
        Ok(())
    }
    fn add_trial_force(&self, _u: &[f64], _load_factor: f64, _force: &mut [f64]) -> rcfd_fem::Result<()> {
        Ok(())
    }
}

/// The solve of the block with the observer: the error text (empty if it passes) and the log.
fn solve_observed(mesh: &SolidMesh, o: &NewtonOptions) -> (String, Log) {
    let m = block_model(mesh);
    let log = RefCell::new(Log::default());
    let obs = Observer { log: &log };
    let mut watch = Watch { log: &log };
    let t0 = Instant::now();
    let out = solve_with(
        &m,
        &[],
        &[],
        o,
        DriverExtensions {
            contributions: vec![&obs],
            step_control: Some(&mut watch),
        },
    );
    let secs = t0.elapsed().as_secs_f64();
    let text = match out {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    };
    println!("OB observed: {text}, {secs:.1} s");
    let log = log.into_inner();
    let att: Vec<String> = log
        .attempts
        .iter()
        .map(|a| {
            format!(
                "{:.6}->{:.6}{}",
                a.committed,
                a.attempted,
                if a.converged == Some(true) { "" } else { " x" }
            )
        })
        .collect();
    println!("OB observed attempts: [{}]", att.join(", "));
    (text, log)
}

// ============================================================================ the tangent and its lowest pair

/// The symmetric part `(K + K^T) / 2` (ADR 0694 `symmetrise`).
fn symmetrise(a: &CsrMatrix) -> CsrMatrix {
    let mut t = Vec::with_capacity(2 * a.val.len());
    for r in 0..a.n {
        for p in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = a.col[p];
            t.push((r, c, 0.5 * a.val[p]));
            t.push((c, r, 0.5 * a.val[p]));
        }
    }
    CsrMatrix::from_triplets(a.n, t)
}

/// The supernodal `L D L^T` with the `NonZero` rule (ADR 0694 `inertia`).
fn factor_nonzero(a: &CsrMatrix) -> Option<SupernodalLdlt> {
    SupernodalLdlt::factor_with(
        a,
        &SparseOptions {
            ordering: SparseOrdering::NestedDissection,
            pivots: PivotRule::NonZero,
            symmetry_tolerance: 1e-12,
        },
    )
    .ok()
}

fn identity(n: usize) -> CsrMatrix {
    CsrMatrix::from_triplets(n, (0..n).map(|i| (i, i, 1.0)).collect())
}

fn points_of(m: &SolidModel, free: &[usize]) -> Vec<[f64; 3]> {
    let dofs = m.dof_map();
    free.iter()
        .map(|&e| {
            let (n, _) = dofs.node_component(e).unwrap();
            let p = m.mesh().nodes()[n];
            [p[0], p[1], p[2]]
        })
        .collect()
}

/// The lowest pair of `(K, I)` at a state with `nu = 0` (`Shift::Auto`, the supernodal factor, tolerance `1e-8`, 500
/// iterations, the backward residual test: ADR 0694, 0697 amendment 1), its vector of unit 2-norm with the sign fixed by its
/// largest component (positive).
fn lowest_pair(m: &SolidModel, sys: &Sys) -> Option<(f64, Vec<f64>)> {
    let opts = SubspaceOptions {
        shift: Shift::Auto,
        tolerance: 1e-8,
        residual_test: ResidualTest::Backward,
        max_iterations: 500,
        factorisation: Factorisation::Supernodal,
        points: Some(points_of(m, &sys.free)),
        ..SubspaceOptions::default()
    };
    let e = lowest_eigenpairs(&sys.k, &identity(sys.free.len()), 4, &opts).ok()?;
    println!("lowest pairs: values {:?}, residuals {:?}", e.values, e.residuals);
    let mut phi = e.vectors[0].clone();
    let n = norm(phi.iter().copied());
    let big = phi.iter().fold(0.0f64, |w, &x| if x.abs() > w.abs() { x } else { w });
    let s = if big < 0.0 { -1.0 / n } else { 1.0 / n };
    phi.iter_mut().for_each(|x| *x *= s);
    Some((e.values[0], phi))
}

// ============================================================================ the branch case of the block (ADR 0711 D6)

/// A converged state of the path at the load factor `p`.
#[derive(Clone)]
struct St {
    p: f64,
    u: Vec<f64>,
    states: Vec<ElementState>,
}

/// The symmetrised reduced tangent `K`, the load vector `f = -dr_F/dp` and the free equations at a state.
struct Sys {
    k: CsrMatrix,
    f: Vec<f64>,
    free: Vec<usize>,
}

/// The inertia and the path tangent `v_t = K^{-1} f` at a state (one factor).
struct Info {
    nu: Option<usize>,
    vt: Option<Vec<f64>>,
}

fn expand(free: &[usize], n: usize, x: &[f64]) -> Vec<f64> {
    let mut v = vec![0.0; n];
    for (&e, &xi) in free.iter().zip(x) {
        v[e] = xi;
    }
    v
}

fn amp(phi: &[f64], free: &[usize], u: &[f64]) -> f64 {
    phi.iter().zip(free).map(|(p, &q)| p * u[q]).sum()
}

/// `solve_increment` and the tangent on the block model, `solve_from` on the model without the punch constraint with the
/// punch prescribed at `p v` (ADR 0711, `FemBranch`).
struct FemBranch<'a> {
    m: &'a SolidModel,
    m2: &'a SolidModel,
    opts: NewtonOptions,
    fixed_eqs: Vec<(usize, f64)>,
    punch: Vec<(usize, usize, f64)>,
    free: Vec<usize>,
}

impl<'a> FemBranch<'a> {
    fn new(m: &'a SolidModel, m2: &'a SolidModel) -> FemBranch<'a> {
        let fixed_eqs = prescribed_equations(m, &[]).unwrap();
        let values: HashMap<usize, f64> = fixed_eqs.iter().copied().collect();
        let dofs = m.dof_map();
        let punch = m
            .mesh()
            .node_set("punch")
            .unwrap()
            .iter()
            .map(|&nd| {
                let eq = dofs.equation(nd, 2).unwrap();
                (nd, 2, values[&eq])
            })
            .collect();
        let free = free_of(m, &fixed_eqs);
        FemBranch {
            m,
            m2,
            opts: options(10).indefinite_tangent(true),
            fixed_eqs,
            punch,
            free,
        }
    }
    /// `solve_increment` with the indefinite tangent from a converged state to the load `to`.
    fn solve(&self, from: &St, to: f64) -> Result<St, String> {
        let zero = vec![0.0; self.m.dof_map().n_equations()];
        solve_increment(self.m, &[], &zero, &from.u, &from.states, from.p, to, &self.opts, &[])
            .map(|eq| St {
                p: to,
                u: eq.u,
                states: eq.states,
            })
            .map_err(|e| e.to_string())
    }
    fn system(&self, s: &St) -> Sys {
        let kt = tangent(self.m, &s.u, &s.states, Some(4)).unwrap();
        let sys = reduce(&kt, &vec![0.0; kt.n], &self.fixed_eqs).unwrap();
        let free = sys.free().to_vec();
        Sys {
            k: symmetrise(&sys.matrix),
            f: sys.rhs,
            free,
        }
    }
    fn analyse(&self, s: &St) -> (Info, Sys) {
        let sys = self.system(s);
        let (nu, vt) = match factor_nonzero(&sys.k) {
            Some(fac) => (Some(fac.inertia().0), Some(fac.solve(&sys.f))),
            None => (None, None),
        };
        (Info { nu, vt }, sys)
    }
    /// Newton at the fixed load `p` from `start` (its prescribed values replaced by those at `p`) with the external force
    /// `eta g` on the free equations.
    fn fixed(&self, start: &St, p: f64, eta: f64, g: &[f64]) -> Result<St, String> {
        let mut u0 = start.u.clone();
        for &(eq, v) in &self.fixed_eqs {
            u0[eq] = p * v;
        }
        let pres: Vec<Prescribed> = self
            .punch
            .iter()
            .map(|&(nd, c, v)| Prescribed::new(nd, c, p * v))
            .collect();
        let mut load = vec![0.0; u0.len()];
        for (k, &q) in self.free.iter().enumerate() {
            load[q] = eta * g[k];
        }
        solve_from(self.m2, &pres, &load, &u0, &start.states, &self.opts, &[])
            .map(|eq| St {
                p,
                u: eq.u,
                states: eq.states,
            })
            .map_err(|e| e.to_string())
    }
    /// The free residual and its measure relative to `|f_int|_2` (ADR 0711 D3).
    fn residual(&self, s: &St) -> Result<(Vec<f64>, f64), String> {
        let fint = internal_force(self.m, &s.u, &s.states).map_err(|e| e.to_string())?;
        let r: Vec<f64> = self.free.iter().map(|&q| fint[q]).collect();
        let reference = norm(fint.iter().copied());
        let rn = norm(r.iter().copied());
        Ok((r, if reference > 0.0 { rn / reference } else { rn }))
    }
    /// The state `s` at the load `p` (the prescribed values `p v`).
    fn at_load(&self, s: &St, p: f64) -> St {
        let mut u = s.u.clone();
        for &(eq, v) in &self.fixed_eqs {
            u[eq] = p * v;
        }
        St {
            p,
            u,
            states: s.states.clone(),
        }
    }
}

/// ADR 0711 D3: the bordered Newton from `start` under the amplitude constraint `phi . u_F = c`: `K y_1 = -R`, `K y_2 = f`,
/// `dl = (c - phi . u_F - phi . y_1) / (phi . y_2)`, `du = y_1 + dl y_2`; residual and energy `1e-10`, the constraint `1e-9`
/// relative, 20 iterations; the converged state and the number of iterations.
fn bordered(case: &FemBranch, start: &St, phi: &[f64], c: f64) -> Result<(St, usize), String> {
    let free = &case.free;
    let mut s = start.clone();
    let (mut e_first, mut e_last) = (0.0f64, 0.0f64);
    for k in 0..=20 {
        let (r, m) = case.residual(&s)?;
        if !m.is_finite() {
            return Err(format!("non-finite residual at iteration {k}"));
        }
        let gu: f64 = phi.iter().zip(free).map(|(g, &q)| g * s.u[q]).sum();
        let cres = gu - c;
        let c_ok = cres.abs() <= 1e-9 * c.abs().max(1e-300);
        if k >= 1 && m <= 1e-10 && e_last <= 1e-10 * e_first && c_ok {
            return Ok((s, k));
        }
        if k == 20 {
            break;
        }
        let sys = case.system(&s);
        let fac = factor_nonzero(&sys.k).ok_or_else(|| format!("the factor refused at iteration {k}"))?;
        let mr: Vec<f64> = r.iter().map(|x| -x).collect();
        let y1 = fac.solve(&mr);
        let y2 = fac.solve(&sys.f);
        let den = dot(phi, &y2);
        let dl = (-cres - dot(phi, &y1)) / den;
        if !dl.is_finite() {
            return Err(format!("non-finite load correction at iteration {k}"));
        }
        let du: Vec<f64> = y1.iter().zip(&y2).map(|(a, b)| a + dl * b).collect();
        let e = du
            .iter()
            .zip(sys.f.iter().zip(&r))
            .map(|(d, (f, ri))| d * (dl * f - ri))
            .sum::<f64>()
            .abs();
        if k == 0 {
            e_first = e;
        }
        e_last = e;
        for (&q, d) in free.iter().zip(&du) {
            s.u[q] += d;
        }
        s = case.at_load(&s, s.p + dl);
    }
    Err("not converged in 20 iterations".to_string())
}

/// A state of the bordered reference with its amplitude, tangent and slope `dlambda/da`.
struct BPoint {
    s: St,
    a: f64,
    nu: Option<usize>,
    vt: Option<Vec<f64>>,
    sigma: f64,
}

fn bpoint(case: &FemBranch, s: St, phi: &[f64]) -> BPoint {
    let (info, _) = case.analyse(&s);
    let a = amp(phi, &case.free, &s.u);
    let sigma = info.vt.as_ref().map(|v| 1.0 / dot(phi, v)).unwrap_or(f64::NAN);
    BPoint {
        s,
        a,
        nu: info.nu,
        vt: info.vt,
        sigma,
    }
}

/// The tangent predictor of ADR 0711 D3 from `cur` to the amplitude `tgt` and the bordered Newton from it; with the
/// continuity test `|u - u_pred|_inf <= |u_pred - u_cur|_inf` when `continuity`.
fn amp_step(case: &FemBranch, cur: &BPoint, tgt: f64, phi: &[f64], continuity: bool) -> Result<(St, usize), String> {
    let vt = cur.vt.as_ref().ok_or("the factor refused at the previous state")?;
    let dp = (tgt - cur.a) / dot(phi, vt);
    let mut up = cur.s.clone();
    for (&q, v) in case.free.iter().zip(vt) {
        up.u[q] += dp * v;
    }
    let pred = case.at_load(&up, cur.s.p + dp);
    let (s, its) = bordered(case, &pred, phi, tgt)?;
    if continuity {
        let corr = max_abs(case.free.iter().map(|&q| s.u[q] - pred.u[q]));
        let step = max_abs(case.free.iter().map(|&q| pred.u[q] - cur.s.u[q]));
        if !(corr.is_finite() && step.is_finite() && corr <= step) {
            return Err(format!("discontinuous ({corr:.4e} > {step:.4e}), {its} iterations"));
        }
    }
    Ok((s, its))
}

const LF: f64 = 0.9647;
const LB: f64 = 0.9645;
/// ADR 0700's `a*_pred` (the `Debug` form of its log line 81).
const EPS_SWITCH: f64 = 2.7468713663317314e-5;
/// ADR 0711's rungs 1 to 5 (`f789-scratch/run2-heavy.log`, lines 89 to 93): the loads and the slopes.
const BT_LAMBDA: [f64; 5] = [
    0.964113319598,
    0.963379741112,
    0.962070119673,
    0.960069738280,
    0.958472223921,
];
const BT_SIGMA: [f64; 5] = [-2.808553e1, -3.695525e1, -4.454366e1, -4.163801e1, -5.382683e-1];
const A_4: f64 = 1.530877696e-4;
const A_5: f64 = 2.164988001e-4;

#[test]
#[ignore = "ADR 0726: the heavy record on 16^3 (D4), run once with --ignored (656.7 s), a known miss with its limits intact: OB, R0, R1, BT (ADR 0711 rungs 1 to 5 to 3.1e-13 in lambda), AL0, AL1 (29 points, 15 compared), AL2, AL5 met; AL3 met where the bordered reference converged (|Dlambda| 5.3e-10, |Du|/|u| 6.1e-10 at 16 points) but missed at 13 points where the bordered reference failed in element 3720 (plain local modes); AL4 missed, 2.6e-6 > 1e-10 (the measure rebuilt from stored displacements at arc lengths down to 6e-11, below its rounding); the trace lost at a 2.0584e-4, lambda 0.958535 (element 3720), before rung 5"]
fn f797_nh16_punch_branch_heavy_known_miss() {
    let mut ch = Checks::new();
    let t_all = Instant::now();
    let mesh16 = block_mesh(16);
    let m = block_model(&mesh16);
    let m2 = block_model_free_punch(&mesh16);
    let states = initial_states(&m).unwrap();
    let ndof = m.dof_map().n_equations();
    let fb = FemBranch::new(&m, &m2);
    let free = fb.free.clone();

    // OB.
    let (text, log) = solve_observed(&mesh16, &options(10));
    ch.holds("OB: the error text of ADR 0678", text == ADR0678_TEXT);
    let Some(fa) = log
        .attempts
        .iter()
        .rposition(|a| a.committed == 0.9625 && a.converged == Some(false))
    else {
        ch.holds("OB: the failing attempt from 0.9625 observed", false);
        ch.finish();
        return;
    };
    let uc = St {
        p: log.attempts[fa].committed,
        u: log.attempts[fa].iterates[0].clone(),
        states: states.clone(),
    };

    // R0: the symmetric chain (ADR 0700's prefix).
    let mut sym: Vec<St> = vec![uc];
    let mut r0 = true;
    for p in [0.963, 0.9635, 0.964, LB, LF] {
        match fb.solve(sym.last().unwrap(), p) {
            Ok(s) => sym.push(s),
            Err(e) => {
                println!("R0 symmetric at {p}: fails: {e}");
                r0 = false;
                break;
            }
        }
    }
    ch.holds("R0: the symmetric chain converges", r0);
    let find = |p: f64| sym.iter().position(|s| s.p == p);
    let (Some(kb), Some(kf)) = (find(LB), find(LF)) else {
        ch.finish();
        return;
    };
    let (_, sys_f) = fb.analyse(&sym[kf]);
    let Some((mu1, phi)) = lowest_pair(&m, &sys_f) else {
        ch.holds("R0: the lowest pair at 0.9647", false);
        ch.finish();
        return;
    };
    ch.at_most(
        "R0: mu_1(0.9647) against 873.6706402833074",
        (mu1 / 873.6706402833074 - 1.0).abs(),
        1e-9,
    );
    ch.at_most("R0: |phi_f . u_s(0.9645)|", amp(&phi, &free, &sym[kb].u).abs(), 1e-9);

    // R1: the switch at 0.9647 and the step to 0.9645 (ADR 0700 D3, D4).
    let zero = vec![0.0; free.len()];
    let mut start = sym[kf].clone();
    for (k, &q) in free.iter().enumerate() {
        start.u[q] += EPS_SWITCH * phi[k];
    }
    let Ok(ssw) = fb.fixed(&start, LF, 0.0, &phi) else {
        ch.holds("R1: the switch at 0.9647 converges", false);
        ch.finish();
        return;
    };
    let a_sw = amp(&phi, &free, &ssw.u) - amp(&phi, &free, &sym[kf].u);
    ch.at_most(
        "R1: a at 0.9647 against 2.694982374354194e-5",
        (a_sw / 2.694982374354194e-5 - 1.0).abs(),
        1e-9,
    );
    let Ok(sb) = fb.fixed(&ssw, LB, 0.0, &zero) else {
        ch.holds("R1: the step to 0.9645 converges", false);
        ch.finish();
        return;
    };
    let b0 = bpoint(&fb, sb.clone(), &phi);
    let a_rel = b0.a - amp(&phi, &free, &sym[kb].u);
    println!(
        "R1 the branch at 0.9645: a {:+.12e} (relative {a_rel:+.12e}), nu {:?}, dlambda/da {:+.6e}",
        b0.a, b0.nu, b0.sigma
    );
    ch.at_most(
        "R1: a - a_s at 0.9645 against 3.827194475619e-5",
        (a_rel / 3.827194475619e-5 - 1.0).abs(),
        1e-11,
    );
    ch.at_most(
        "R1: a_b against 3.827194241e-5",
        (b0.a / 3.827194241e-5 - 1.0).abs(),
        1e-9,
    );
    ch.equals("R1: nu at 0.9645", b0.nu.unwrap_or(usize::MAX), 1);
    let a_b = b0.a;

    // BT: ADR 0711's rungs 1 to 5.
    let mut bt: Vec<BPoint> = vec![b0];
    for k in 1..=5usize {
        let tgt = a_b * 2f64.powf(k as f64 / 2.0);
        let t0 = Instant::now();
        match amp_step(&fb, bt.last().unwrap(), tgt, &phi, true) {
            Ok((s, its)) => {
                let pt = bpoint(&fb, s, &phi);
                println!(
                    "BT rung {k}: a {:+.9e}, lambda {:.12}, nu {:?}, dlambda/da {:+.6e}, {its} iterations, {:.1} s",
                    pt.a,
                    pt.s.p,
                    pt.nu,
                    pt.sigma,
                    t0.elapsed().as_secs_f64()
                );
                ch.at_most(
                    &format!("BT rung {k}: |lambda - {}|", BT_LAMBDA[k - 1]),
                    (pt.s.p - BT_LAMBDA[k - 1]).abs(),
                    1e-11,
                );
                ch.at_most(
                    &format!("BT rung {k}: |sigma / {:e} - 1|", BT_SIGMA[k - 1]),
                    (pt.sigma / BT_SIGMA[k - 1] - 1.0).abs(),
                    1e-6,
                );
                bt.push(pt);
            }
            Err(e) => {
                ch.holds(&format!("BT rung {k}: accepted ({e})"), false);
                break;
            }
        }
    }

    // AL: the arc-length trace from the B state (D4).
    let Some(vt) = bt[0].vt.clone() else {
        ch.holds("AL: the tangent at the B state", false);
        ch.finish();
        return;
    };
    let c = dot(&vt, &vt);
    let ds1 = 2.5e-4 * 2f64.sqrt() * c.sqrt();
    println!("AL: |v_t| {:.6e}, c {c:.6e}, Ds_1 {ds1:.6e}", c.sqrt());
    let newton = NewtonOptions::new(1).threads(4);
    let mut du0 = expand(&free, ndof, &vt.iter().map(|x| -x).collect::<Vec<f64>>());
    for &(eq, v) in &fb.fixed_eqs {
        du0[eq] = -v;
    }
    let start1 = PathStart::new(LB, sb.u.clone(), sb.states.clone()).increment(du0, -1.0);
    let o1 = ArcLengthOptions::new(ds1)
        .load_scale(c)
        .max_points(2)
        .newton(newton.clone());
    let clock = RefCell::new(Instant::now());
    let report = |tag: &str, p: &PathPoint| {
        let a = amp(&phi, &free, &p.displacement);
        println!(
            "AL {tag} point: lambda {:.12}, a {a:+.9e}, negative pivots {}, length {:.6e}, halvings {}, {} iterations, {:.1} s",
            p.load_factor,
            p.negative_pivots,
            p.length,
            p.halvings,
            p.residuals.len() - 1,
            clock.borrow().elapsed().as_secs_f64()
        );
        *clock.borrow_mut() = Instant::now();
        a
    };
    let mut never = |p: &PathPoint| {
        report("stage 1", p);
        false
    };
    let r1 = trace_from(&m, &[], &[], &o1, &[], &start1, &mut never);
    let path1 = match r1 {
        Ok(p) if p.points.len() == 2 => p,
        Ok(p) => {
            ch.holds(&format!("AL0: stage 1 gives two points ({})", p.points.len()), false);
            ch.finish();
            return;
        }
        Err(e) => {
            ch.holds(&format!("AL0: stage 1 ({e})"), false);
            ch.finish();
            return;
        }
    };
    let p1 = &path1.points[0];
    ch.holds(
        "AL0: stage 1, the first point below 0.9645 with a > a_b",
        p1.load_factor < LB && amp(&phi, &free, &p1.displacement) > a_b,
    );
    let (q1, q2) = (&path1.points[0], &path1.points[1]);
    let du1: Vec<f64> = q2
        .displacement
        .iter()
        .zip(&q1.displacement)
        .map(|(x, y)| x - y)
        .collect();
    let start2 = PathStart::new(q2.load_factor, q2.displacement.clone(), path1.states.clone())
        .increment(du1, q2.load_factor - q1.load_factor);
    let o2 = ArcLengthOptions::new(2.0 * ds1)
        .load_scale(c)
        .max_points(40)
        .newton(newton);
    let seen: RefCell<Vec<(f64, f64, usize)>> = RefCell::new(vec![
        (amp(&phi, &free, &q1.displacement), q1.load_factor, q1.negative_pivots),
        (amp(&phi, &free, &q2.displacement), q2.load_factor, q2.negative_pivots),
    ]);
    let fold_at: RefCell<Option<usize>> = RefCell::new(None);
    let stop_why: RefCell<String> = RefCell::new("the cap of 40 points".to_string());
    let stage2: RefCell<Vec<PathPoint>> = RefCell::new(Vec::new());
    let mut stop2 = |p: &PathPoint| {
        let a = report("stage 2", p);
        stage2.borrow_mut().push(p.clone());
        let mut v = seen.borrow_mut();
        let (pa, pl, pn) = *v.last().unwrap();
        let decreasing_before = v.len() >= 2 && pl < v[v.len() - 2].1;
        v.push((a, p.load_factor, p.negative_pivots));
        let j = v.len() - 1;
        if fold_at.borrow().is_none() && decreasing_before && p.load_factor > pl && pn == 1 && p.negative_pivots == 0 {
            *fold_at.borrow_mut() = Some(j);
            println!(
                "AL READING: fold between a {pa:+.9e} (lambda {pl:.12}) and a {a:+.9e} (lambda {:.12})",
                p.load_factor
            );
        }
        let why = if fold_at.borrow().is_some_and(|f| j >= f + 2) {
            Some("two points after the fold")
        } else if a > 1.5 * A_5 {
            Some("a > 1.5 a_5")
        } else if !(0.95..=0.97).contains(&p.load_factor) {
            Some("lambda outside [0.95, 0.97]")
        } else if a < pa {
            Some("a decreasing (a fall-back)")
        } else {
            None
        };
        if let Some(w) = why {
            *stop_why.borrow_mut() = w.to_string();
            return true;
        }
        false
    };
    let r2 = trace_from(&m, &[], &[], &o2, &[], &start2, &mut stop2);
    let end = match &r2 {
        Ok(p) if p.stopped => format!("stopped: {}", stop_why.borrow()),
        Ok(_) => "the cap of 40 points".to_string(),
        Err(e) => format!("lost: {e}"),
    };
    println!("AL READING: the end of stage 2: {end}");
    let mut all: Vec<PathPoint> = path1.points.clone();
    all.extend(stage2.borrow().iter().cloned());

    // AL4, AL5 over both stages.
    let e1 = constraint_error(&free, (&sb.u, LB), &path1.points, c);
    let e2 = constraint_error(&free, (&q2.displacement, q2.load_factor), &stage2.borrow(), c);
    ch.at_most("AL4: the constraint at every point, relative", e1.max(e2), 1e-10);
    ch.holds(
        "AL5: the punch equations at lambda v at every point",
        prescribed_follow(&fb.fixed_eqs, &all),
    );

    // AL1 to AL3: the compared points against the bordered Newton at their amplitudes.
    let mut compared = 0;
    let mut in_last = false;
    let mut prev_a = a_b;
    let mut increasing = true;
    let (mut worst_l, mut worst_u) = (0.0f64, 0.0f64);
    for (j, p) in all.iter().enumerate() {
        let a = amp(&phi, &free, &p.displacement);
        if a > A_5 {
            break;
        }
        if a <= prev_a {
            increasing = false;
        }
        prev_a = a;
        if a <= a_b {
            continue;
        }
        compared += 1;
        in_last |= a > A_4;
        ch.equals(
            &format!("AL2: negative pivots at point {}", j + 1),
            p.negative_pivots,
            1,
        );
        let Some(base) = bt.iter().rev().find(|b| b.a <= a) else {
            ch.holds(&format!("AL3: a reference state below point {}", j + 1), false);
            continue;
        };
        let t0 = Instant::now();
        match amp_step(&fb, base, a, &phi, false) {
            Ok((s, its)) => {
                let dl = (p.load_factor - s.p).abs();
                let du = max_abs(p.displacement.iter().zip(&s.u).map(|(x, y)| x - y)) / max_abs(s.u.iter().copied());
                println!(
                    "AL3 point {}: a {a:+.9e}, lambda {:.12} against the bordered {:.12} ({its} iterations, {:.1} s): \
                     |Dlambda| {dl:.3e}, |Du| / |u| {du:.3e}",
                    j + 1,
                    p.load_factor,
                    s.p,
                    t0.elapsed().as_secs_f64()
                );
                worst_l = if dl.is_finite() { worst_l.max(dl) } else { f64::NAN };
                worst_u = if du.is_finite() { worst_u.max(du) } else { f64::NAN };
            }
            Err(e) => ch.holds(&format!("AL3 point {}: the bordered reference ({e})", j + 1), false),
        }
    }
    ch.holds(
        &format!("AL1: at least 5 compared points ({compared}) and one in (a_4, a_5] ({in_last})"),
        compared >= 5 && in_last,
    );
    ch.holds("AL2: a strictly increasing up to the last compared point", increasing);
    ch.at_most("AL3: max |lambda_j - lambda^B_j|", worst_l, 1e-8);
    ch.at_most("AL3: max |u_j - u^B_j|_inf / |u^B_j|_inf", worst_u, 1e-7);
    println!(
        "AR: {} points in all, fold {:?}, end: {end}",
        all.len(),
        fold_at.borrow().map(|j| (seen.borrow()[j - 1].0, seen.borrow()[j].0))
    );
    println!("f797_nh16_punch_branch_heavy: {:.1} s", t_all.elapsed().as_secs_f64());
    ch.finish();
}
