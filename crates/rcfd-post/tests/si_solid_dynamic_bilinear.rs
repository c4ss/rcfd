//! F-504 (ADR 0414): the opt-in bilinear projection of F-362 (`MapSettings::bilinear_projection`) on the 3:2 walls of F-502
//! for solid end faces of 2 x 2, 4 x 4 and 8 x 8 elements: its weights under the eight symmetries of the square (S-B-n),
//! against the test-side bilinear evaluation `Q` of ADR 0412 (Q-B-n), and the coupled frequency change against the
//! static-compliance prediction of ADR 0412 D3 (C-B-n); companions PD, K0, FM, I, MB, E1 and the negative control N1 (the
//! default projection). The bar, the piston mock, the 1D prediction, the runs and the balance are copied from
//! `si_solid_dynamic_nonmatching.rs` (ADR 0412; test helpers are not shared between test crates; that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::transient::TransientOptions;
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_fv::cosim::{
    coupled_step, interface_rows, CoupledStep, CouplingSettings, Fluid, InterfaceMap, MapSettings, Participant,
};
use rcfd_post::solid_coupling::{solve_face_forces, SolidInterface};
use rcfd_post::solid_dynamic::DynamicSolid;
use std::f64::consts::PI;

const L: f64 = 4.0;
const W: f64 = 1.0;
const NX: usize = 20;
const E: f64 = 2e11;
const RHO: f64 = 7850.0;
const V0: f64 = 1.0;
const STEPS: usize = 320;
const RATIO: f64 = 4.0;
const BETA: f64 = 0.25;
const GAMMA: f64 = 0.5;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

// ---- set-ups (ADR 0410 D4, ADR 0412 D4) -----------------------------------------------------------------------------

/// The bar: Hex8 20 x n x n, nu = 0, clamped on xmin.
fn bar(n: usize) -> SolidModel {
    let mut model = SolidModel::new(hex_box([0.0; 3], [L, W, W], [NX, n, n]).unwrap());
    model
        .add_material("all", Material::isotropic_elastic(E, 0.0, RHO, 0.0).unwrap())
        .unwrap();
    for c in 0..3 {
        model.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    model
}

/// A fluid wall on `x = L` of `m x m` square faces. With `permuted` the points are listed z first, y descending.
fn wall(m: usize, permuted: bool) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let index = |a: usize, b: usize| {
        if permuted {
            (m - a) * (m + 1) + b
        } else {
            b * (m + 1) + a
        }
    };
    let mut points = vec![[0.0; 3]; (m + 1) * (m + 1)];
    for a in 0..=m {
        for b in 0..=m {
            points[index(a, b)] = [L, W * a as f64 / m as f64, W * b as f64 / m as f64];
        }
    }
    let mut faces = Vec::new();
    for b in 0..m {
        for a in 0..m {
            faces.push(vec![index(a, b), index(a + 1, b), index(a + 1, b + 1), index(a, b + 1)]);
        }
    }
    (points, faces)
}

fn face_area(points: &[Vec3], face: &[usize]) -> f64 {
    let d1 = vec3::sub(points[face[2]], points[face[0]]);
    let d2 = vec3::sub(points[face[3]], points[face[1]]);
    0.5 * vec3::norm(vec3::cross(d1, d2))
}

fn mean(values: &[Vec3], face: &[usize]) -> Vec3 {
    let s = face.iter().fold(vec3::ZERO, |a, &q| vec3::add(a, values[q]));
    vec3::scale(s, 1.0 / face.len() as f64)
}

/// The piston mock of ADR 0410 amendment 1: one wall acceleration (the area mean of the faces' x components, the points'
/// kinematics by the Newmark relation), the force `-m_a a_bar` distributed uniformly per area, axial.
#[derive(Debug, Clone)]
struct PistonWall {
    points: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    areas: Vec<f64>,
    dt: f64,
    m_a: f64,
    state: WallState,
    saved: Option<WallState>,
}

#[derive(Debug, Clone, PartialEq)]
struct WallState {
    step: u64,
    d: Vec<Vec3>,
    v: Vec<Vec3>,
    a: Vec<Vec3>,
}

impl PistonWall {
    fn new(m: usize, permuted: bool, dt: f64, m_a: f64) -> Self {
        let (points, faces) = wall(m, permuted);
        let areas = faces.iter().map(|f| face_area(&points, f)).collect();
        let np = points.len();
        PistonWall {
            points,
            faces,
            areas,
            dt,
            m_a,
            state: WallState {
                step: 0,
                d: vec![vec3::ZERO; np],
                v: vec![vec3::ZERO; np],
                a: vec![vec3::ZERO; np],
            },
            saved: None,
        }
    }
    fn area(&self) -> f64 {
        self.areas.iter().sum()
    }
    /// Unit total axial force distributed per area.
    fn unit_forces(&self) -> Vec<Vec3> {
        let a = self.area();
        self.areas.iter().map(|&af| [af / a, 0.0, 0.0]).collect()
    }
}

impl Participant for PistonWall {
    fn name(&self) -> &str {
        "fluid"
    }
    fn time(&self) -> f64 {
        self.state.step as f64 * self.dt
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.saved = Some(self.state.clone());
        Ok(())
    }
    fn restore(&mut self) -> Result<(), String> {
        let s = self.saved.as_ref().ok_or("mock wall: no checkpoint")?;
        self.state.clone_from(s);
        Ok(())
    }
}

impl Fluid for PistonWall {
    fn point_coordinates(&self) -> Vec<Vec3> {
        self.points.clone()
    }
    fn face_points(&self) -> Vec<Vec<usize>> {
        self.faces.clone()
    }
    fn forces(&self) -> Result<Vec<Vec3>, String> {
        let a_total = self.area();
        let mut a_bar = 0.0;
        for (f, &af) in self.faces.iter().zip(&self.areas) {
            a_bar += af / a_total * mean(&self.state.a, f)[0];
        }
        let total = -self.m_a * a_bar;
        Ok(self.areas.iter().map(|&af| [af / a_total * total, 0.0, 0.0]).collect())
    }
    fn advance(&mut self, dt: f64, displacements: &[Vec3]) -> Result<Vec<Vec3>, String> {
        if displacements.len() != self.points.len() || dt.to_bits() != self.dt.to_bits() {
            return Err("mock wall: wrong count or step".into());
        }
        let st = &mut self.state;
        for i in 0..displacements.len() {
            let (d0, v0, a0, dn) = (st.d[i], st.v[i], st.a[i], displacements[i]);
            let mut a1 = [0.0; 3];
            let mut v1 = [0.0; 3];
            for c in 0..3 {
                a1[c] = (dn[c] - d0[c] - dt * v0[c] - dt * dt * (0.5 - BETA) * a0[c]) / (BETA * dt * dt);
                v1[c] = v0[c] + dt * ((1.0 - GAMMA) * a0[c] + GAMMA * a1[c]);
            }
            st.d[i] = dn;
            st.v[i] = v1;
            st.a[i] = a1;
        }
        st.step += 1;
        self.forces()
    }
}

// ---- the 1D predictions (ADR 0410 D6/D7, ADR 0412 D3) -----------------------------------------------------------------

/// The first root of `x tan x = rhs` on `(0, pi/2)`.
fn root(rhs: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, PI / 2.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid * mid.tan() < rhs {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// A symmetric tridiagonal matrix (diagonal, off-diagonal).
struct Tri(Vec<f64>, Vec<f64>);

impl Tri {
    fn mul(&self, x: &[f64]) -> Vec<f64> {
        let n = x.len();
        (0..n)
            .map(|i| {
                let mut y = self.0[i] * x[i];
                if i > 0 {
                    y += self.1[i - 1] * x[i - 1];
                }
                if i + 1 < n {
                    y += self.1[i] * x[i + 1];
                }
                y
            })
            .collect()
    }
    /// Thomas algorithm.
    fn solve(&self, b: &[f64]) -> Vec<f64> {
        let n = b.len();
        let (mut c, mut d) = (vec![0.0; n], vec![0.0; n]);
        for i in 0..n {
            let sub = if i > 0 { self.1[i - 1] } else { 0.0 };
            let den = self.0[i] - if i > 0 { sub * c[i - 1] } else { 0.0 };
            c[i] = if i + 1 < n { self.1[i] / den } else { 0.0 };
            d[i] = (b[i] - if i > 0 { sub * d[i - 1] } else { 0.0 }) / den;
        }
        let mut x = vec![0.0; n];
        for i in (0..n).rev() {
            x[i] = d[i] - if i + 1 < n { c[i] * x[i + 1] } else { 0.0 };
        }
        x
    }
}

/// The 1D chain of ADR 0410 D5 (nodes 1..=NX, node 0 clamped, area W^2) with the added mass `m_a`: on the tip when
/// `dc = None`, else on one more unknown joined to the tip by a spring of compliance `dc` (ADR 0412 D3).
fn chain(m_a: f64, dc: Option<f64>) -> (Tri, Tri) {
    let h = L / NX as f64;
    let a = W * W;
    let (ke, me) = (E * a / h, RHO * a * h / 6.0);
    let n = NX + usize::from(dc.is_some());
    let mut kd = vec![2.0 * ke; n];
    let mut md = vec![4.0 * me; n];
    let mut ko = vec![-ke; n - 1];
    let mut mo = vec![me; n - 1];
    kd[NX - 1] = ke;
    md[NX - 1] = 2.0 * me;
    match dc {
        None => md[NX - 1] += m_a,
        Some(dc) => {
            let kc = 1.0 / dc;
            kd[NX - 1] += kc;
            kd[NX] = kc;
            ko[NX - 1] = -kc;
            md[NX] = m_a;
            mo[NX - 1] = 0.0;
        }
    }
    (Tri(kd, ko), Tri(md, mo))
}

/// First eigenfrequency of `K phi = omega^2 M phi` by inverse iteration (200 steps, Rayleigh quotient).
fn first_omega(k: &Tri, m: &Tri) -> f64 {
    let n = k.0.len();
    let mut y = vec![1.0; n];
    let mut lambda = 0.0;
    for _ in 0..200 {
        let x = k.solve(&m.mul(&y));
        let norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        y = x.iter().map(|v| v / norm).collect();
        let ky: f64 = y.iter().zip(k.mul(&y)).map(|(a, b)| a * b).sum();
        let my: f64 = y.iter().zip(m.mul(&y)).map(|(a, b)| a * b).sum();
        lambda = ky / my;
    }
    lambda.sqrt()
}

struct Setup {
    kl: f64,
    dt: f64,
    m_a: f64,
    /// Discrete frequency of the matching prediction.
    omega_pred0: f64,
}

impl Setup {
    fn new() -> Self {
        let kl = root(1.0 / RATIO);
        let omega = kl * (E / RHO).sqrt() / L;
        let dt = 2.0 * PI / omega / 100.0;
        let m_a = RATIO * RHO * W * W * L;
        let (k, m) = chain(m_a, None);
        let omega_pred0 = discrete(first_omega(&k, &m), dt);
        Setup {
            kl,
            dt,
            m_a,
            omega_pred0,
        }
    }
    /// The predicted change of the discrete frequency for the extra static compliance `dc` (ADR 0412 D3).
    fn predicted_change(&self, dc: f64) -> f64 {
        let (k, m) = chain(self.m_a, Some(dc));
        discrete(first_omega(&k, &m), self.dt) / self.omega_pred0 - 1.0
    }
}

/// `Omega = (2/dt) atan(omega dt/2)` (average acceleration, undamped).
fn discrete(omega: f64, dt: f64) -> f64 {
    2.0 / dt * (omega * dt / 2.0).atan()
}

// ---- maps -------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Map {
    /// The library's opt-in bilinear projection (ADR 0414).
    Bilinear,
    /// The test-side bilinear evaluation `Q` of ADR 0412 D6.
    TestQ,
    /// The default projection (negative control N1).
    Projection,
}

impl Map {
    fn id(self) -> &'static str {
        match self {
            Map::Bilinear => "B",
            Map::TestQ => "Q",
            Map::Projection => "P",
        }
    }
}

/// Local coordinates `(s, t)` of `x` on an axis-aligned rectangular element along its edges `0 -> 1` and `0 -> 3`, if
/// `x` lies on it (ADR 0412, copied).
fn local(c: &[Vec3], x: Vec3) -> Option<(f64, f64)> {
    let e1 = vec3::sub(c[1], c[0]);
    let e3 = vec3::sub(c[3], c[0]);
    let r = vec3::sub(x, c[0]);
    let s = vec3::dot(r, e1) / vec3::dot(e1, e1);
    let t = vec3::dot(r, e3) / vec3::dot(e3, e3);
    let tol = 1e-12;
    (s >= -tol && s <= 1.0 + tol && t >= -tol && t <= 1.0 + tol).then(|| (s.clamp(0.0, 1.0), t.clamp(0.0, 1.0)))
}

/// The rows of `Q`: on the first interface element containing the point, the bilinear shape functions (ADR 0412 D6).
fn q_rows(interface: &SolidInterface, points: &[Vec3]) -> Vec<Vec<(usize, f64)>> {
    let nodes = interface.coordinates();
    points
        .iter()
        .map(|&x| {
            let (el, (s, t)) = interface
                .elements()
                .iter()
                .find_map(|el| {
                    let c: Vec<Vec3> = el.iter().map(|&i| nodes[i]).collect();
                    local(&c, x).map(|st| (el, st))
                })
                .expect("every wall point lies on the face");
            let w = [(1.0 - s) * (1.0 - t), s * (1.0 - t), s * t, (1.0 - s) * t];
            el.iter().zip(w).filter(|r| r.1 != 0.0).map(|(&i, w)| (i, w)).collect()
        })
        .collect()
}

/// The map of `kind` and its rows; the precision defect of the bilinear projection is checked (PD).
fn build(
    checks: &mut Checks,
    id: &str,
    interface: &SolidInterface,
    wall: &PistonWall,
    kind: Map,
) -> (InterfaceMap, Vec<Vec<(usize, f64)>>) {
    let nodes = interface.coordinates();
    match kind {
        Map::Bilinear | Map::Projection => {
            let settings = if kind == Map::Bilinear {
                MapSettings::bilinear_projection(1e-9)
            } else {
                MapSettings::projection(1e-9)
            };
            let (rows, report) = interface_rows(nodes, interface.elements(), &wall.points, &settings).unwrap();
            let (map, _) = interface
                .build_map(&wall.points, wall.faces.clone(), &settings)
                .unwrap();
            if kind == Map::Bilinear {
                checks.at_most(&format!("PD {id} precision defect"), report.precision_defect, 1e-14);
            }
            (map, rows)
        }
        Map::TestQ => {
            let rows = q_rows(interface, &wall.points);
            let map = InterfaceMap::new(rows.clone(), nodes.len(), wall.faces.clone()).unwrap();
            (map, rows)
        }
    }
}

/// Axial interface weights `w = B^T (A_f/A)` of a map.
fn weights(map: &InterfaceMap, wall: &PistonWall) -> Vec<f64> {
    map.forces_to_nodes(&wall.unit_forces())
        .unwrap()
        .iter()
        .map(|f| f[0])
        .collect()
}

/// `max |H_pi - G_pi|` over the two rows' entries.
fn row_difference(a: &[Vec<(usize, f64)>], b: &[Vec<(usize, f64)>]) -> f64 {
    let mut worst = 0.0f64;
    for (ra, rb) in a.iter().zip(b) {
        let mut d: std::collections::BTreeMap<usize, f64> = ra.iter().copied().collect();
        for &(i, w) in rb {
            *d.entry(i).or_insert(0.0) -= w;
        }
        worst = d.values().fold(worst, |m, v| m.max(v.abs()));
    }
    worst
}

/// Static compliance `w^T K^{-1} w` of a map (ADR 0412 D5).
fn compliance(model: &SolidModel, interface: &SolidInterface, map: &InterfaceMap, wall: &PistonWall) -> f64 {
    let faces = wall.unit_forces();
    let f = map.forces_to_nodes(&faces).unwrap();
    let result = solve_face_forces(model, interface, map, &faces, &StaticOptions::default()).unwrap();
    let u = interface.displacements(&result);
    f.iter().zip(&u).map(|(a, b)| vec3::dot(*a, *b)).sum()
}

/// The seven non-trivial symmetries of the square `[0, W]^2` in `(y, z)`, the first three those that keep the set of
/// diagonals of the projection's split (ADR 0412 D6).
const SYMMETRIES: [&str; 7] = [
    "mirror y <-> z",
    "mirror (y, z) -> (W - z, W - y)",
    "half turn",
    "mirror y -> W - y",
    "mirror z -> W - z",
    "quarter turn (y, z) -> (W - z, y)",
    "quarter turn (y, z) -> (z, W - y)",
];

fn apply(k: usize, y: f64, z: f64) -> (f64, f64) {
    match k {
        0 => (z, y),
        1 => (W - z, W - y),
        2 => (W - y, W - z),
        3 => (W - y, z),
        4 => (y, W - z),
        5 => (W - z, y),
        _ => (z, W - y),
    }
}

/// `max |w(g x) - w(x)|/max |w|` for symmetry `k`.
fn symmetry_defect(nodes: &[Vec3], w: &[f64], k: usize) -> f64 {
    let scale = w.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    let mut defect = 0.0f64;
    for (i, x) in nodes.iter().enumerate() {
        let (y, z) = apply(k, x[1], x[2]);
        let j = nodes
            .iter()
            .position(|q| (q[1] - y).abs() < 1e-12 && (q[2] - z).abs() < 1e-12)
            .expect("the face is symmetric");
        defect = defect.max((w[j] - w[i]).abs() / scale);
    }
    defect
}

// ---- the runs ---------------------------------------------------------------------------------------------------------

struct Run {
    omega_m: f64,
    max_iterations: usize,
    converged_steps: usize,
    failure: Option<String>,
    last: Option<CoupledStep>,
}

/// 320 coupled steps from the initial state of ADR 0410 D4 on `wall` with `map`.
fn run(p: &Setup, model: &SolidModel, mut wall: PistonWall, map: &InterfaceMap, cap: usize) -> (Run, PistonWall) {
    let mut solid = DynamicSolid::new("solid", model, "xmax", &[], &TransientOptions::new(p.dt), 1).unwrap();
    let dofs = model.dof_map();
    let nn = solid.integrator().n_unknowns();
    let mut v0 = vec![0.0; nn];
    for (node, x) in model.mesh().nodes().iter().enumerate() {
        if let Some(e) = dofs.equation(node, 0) {
            v0[e] = V0 * (p.kl * x[0] / L).sin();
        }
    }
    solid.start(0.0, &vec![0.0; nn], &v0).unwrap();
    let vn: Vec<Vec3> = solid
        .interface()
        .coordinates()
        .iter()
        .map(|x| [V0 * (p.kl * x[0] / L).sin(), 0.0, 0.0])
        .collect();
    wall.state.v = map.displacement_to_points(&vn).unwrap();
    let settings = CouplingSettings {
        max_iterations: cap,
        ..CouplingSettings::aitken(0.5)
    };
    let mut samples = Vec::with_capacity(STEPS);
    let (mut max_it, mut conv) = (0, 0);
    let mut failure = None;
    let mut last = None;
    for k in 0..STEPS {
        match coupled_step(&mut wall, &mut solid, map, p.dt, &settings) {
            Ok(step) => {
                max_it = max_it.max(step.iterations);
                if step.converged && step.residuals.last().is_some_and(|&r| r <= 1e-8) {
                    conv += 1;
                }
                let ux =
                    step.node_displacements.iter().map(|q| q[0]).sum::<f64>() / step.node_displacements.len() as f64;
                samples.push(((k + 1) as f64 * p.dt, ux));
                last = Some(step);
            }
            Err(e) => {
                failure = Some(format!("step {}: {e}", k + 1));
                break;
            }
        }
    }
    let mut times = Vec::new();
    for w in samples.windows(2) {
        let ((ta, ua), (_, ub)) = (w[0], w[1]);
        if ua * ub < 0.0 || (ub == 0.0 && ua != 0.0) {
            times.push(ta + p.dt * ua / (ua - ub));
        }
    }
    let omega_m = if times.len() >= 2 {
        2.0 * PI / (2.0 * (times[times.len() - 1] - times[0]) / (times.len() - 1) as f64)
    } else {
        f64::NAN
    };
    println!(
        "    run: Omega_m {omega_m:.12e}, {} crossings, iterations max {max_it}, converged {conv}/{STEPS}, failure {failure:?}",
        times.len()
    );
    (
        Run {
            omega_m,
            max_iterations: max_it,
            converged_steps: conv,
            failure,
            last,
        },
        wall,
    )
}

fn check_iterations(checks: &mut Checks, id: &str, run: &Run, cap: usize) {
    checks.holds(
        &format!("I {id} all converged"),
        run.failure.is_none() && run.converged_steps == STEPS,
        &format!("every one of the {STEPS} steps converged to 1e-8"),
    );
    checks.at_most(&format!("I {id} max iterations"), run.max_iterations as f64, cap as f64);
}

/// Resultant, moment (about the origin) and work on the two sides of a map, each relative to its scale (ADR 0410 M1).
fn balance(map: &InterfaceMap, nodes: &[Vec3], wall: &PistonWall, face_forces: &[Vec3], q: &[Vec3]) -> [f64; 3] {
    let d = map.face_displacements(&map.displacement_to_points(q).unwrap()).unwrap();
    let (mut rf, mut mf, mut wf) = (vec3::ZERO, vec3::ZERO, 0.0);
    let (mut rs_scale, mut ms_scale, mut ws_scale) = (0.0, 0.0, 0.0);
    for ((face, &force), df) in wall.faces.iter().zip(face_forces).zip(&d) {
        let c = mean(&wall.points, face);
        rf = vec3::add(rf, force);
        mf = vec3::add(mf, vec3::cross(c, force));
        wf += vec3::dot(force, *df);
        rs_scale += vec3::norm(force);
        ms_scale += vec3::norm(c) * vec3::norm(force);
        ws_scale += vec3::norm(force) * vec3::norm(*df);
    }
    let f = map.forces_to_nodes(face_forces).unwrap();
    let (mut rs, mut ms, mut ws) = (vec3::ZERO, vec3::ZERO, 0.0);
    for ((x, fn_), qn) in nodes.iter().zip(&f).zip(q) {
        rs = vec3::add(rs, *fn_);
        ms = vec3::add(ms, vec3::cross(*x, *fn_));
        ws += vec3::dot(*fn_, *qn);
    }
    [
        vec3::norm(vec3::sub(rs, rf)) / rs_scale,
        vec3::norm(vec3::sub(ms, mf)) / ms_scale,
        (ws - wf).abs() / ws_scale,
    ]
}

/// The matching frequency of the 2 x 2 face (the reference of FU).
/// Every check of ADR 0414 on the 3:2 wall of the face of `n x n` elements.
fn face_refinement(n: usize) {
    let mut checks = Checks::new();
    let p = Setup::new();
    let model = bar(n);
    let interface = SolidInterface::new(&model, "xmax").unwrap();
    let nodes = interface.coordinates().to_vec();
    let m = 3 * n / 2;
    println!(
        "n = {n} (3:2 wall {m} x {m}): kL {:.12}, dt {:.6e}, Omega_pred,0 {:.12e}",
        p.kl, p.dt, p.omega_pred0
    );

    // C0: the static compliances and the predictions, before any coupled run (the matching wall: the projection, as
    // ADR 0412).
    let matching_wall = PistonWall::new(n, true, p.dt, p.m_a);
    let (matching_map, _) = build(
        &mut checks,
        &format!("matching n{n}"),
        &interface,
        &matching_wall,
        Map::Projection,
    );
    let c0 = compliance(&model, &interface, &matching_map, &matching_wall);
    checks.at_most(
        &format!("K0 n{n} |c_w0 E A/L - 1|"),
        (c0 * E * W * W / L - 1.0).abs(),
        1e-9,
    );
    let mut built = Vec::new();
    for kind in [Map::Bilinear, Map::TestQ, Map::Projection] {
        let wall = PistonWall::new(m, false, p.dt, p.m_a);
        let id = format!("{} n{n}", kind.id());
        let (map, rows) = build(&mut checks, &id, &interface, &wall, kind);
        let dc = compliance(&model, &interface, &map, &wall) - c0;
        let pred = p.predicted_change(dc);
        println!(
            "  C0 {id}: Dc E A/L {:.9e}, predicted change {pred:.9e} (before the coupled runs)",
            dc * E * W * W / L
        );
        built.push((kind, wall, map, rows, pred));
    }
    checks.holds(
        &format!("C0 n{n}"),
        true,
        "every prediction computed and printed before the first coupled run",
    );

    // S-B-n, Q-B-n, N1: the weights on the 3:2 wall.
    let w_b = weights(&built[0].2, &built[0].1);
    let w_q = weights(&built[1].2, &built[1].1);
    let w_p = weights(&built[2].2, &built[2].1);
    let scale = |w: &[f64]| w.iter().fold(0.0f64, |a, x| a.max(x.abs()));
    for (k, name) in SYMMETRIES.iter().enumerate() {
        checks.at_most(&format!("S-B-{n} {name}"), symmetry_defect(&nodes, &w_b, k), 1e-13);
    }
    let dev = w_b.iter().zip(&w_q).fold(0.0f64, |a, (x, y)| a.max((x - y).abs())) / scale(&w_b);
    checks.at_most(&format!("Q-B-{n} |w - w_Q|/max w"), dev, 1e-13);
    checks.at_most(
        &format!("Q-B-{n} rows |H - H_Q|"),
        row_difference(&built[0].3, &built[1].3),
        1e-13,
    );
    let dev_p = w_p.iter().zip(&w_q).fold(0.0f64, |a, (x, y)| a.max((x - y).abs())) / scale(&w_p);
    checks.at_least(&format!("N1 n{n} projection |w_P - w_Q|/max w_P"), dev_p, 0.05);
    for k in [3, 4] {
        checks.at_least(
            &format!("N1 n{n} projection {}", SYMMETRIES[k]),
            symmetry_defect(&nodes, &w_p, k),
            0.05,
        );
    }

    // The coupled runs: matching, the bilinear projection, Q.
    println!("  matching n{n}:");
    let (matching, _) = run(&p, &model, matching_wall, &matching_map, 4);
    check_iterations(&mut checks, &format!("matching n{n}"), &matching, 4);
    checks.at_most(
        &format!("FM n{n} |Omega_m/Omega_pred,0 - 1|"),
        (matching.omega_m / p.omega_pred0 - 1.0).abs(),
        2e-5,
    );
    let mut changes = Vec::new();
    for (kind, wall, map, _, pred) in built.into_iter().take(2) {
        let id = format!("{} n{n}", kind.id());
        println!("  3:2 {id}:");
        let (r, wall) = run(&p, &model, wall, &map, 20);
        check_iterations(&mut checks, &id, &r, 20);
        let change = r.omega_m / matching.omega_m - 1.0;
        println!("  {id}: measured change {change:.9e}, predicted {pred:.9e}");
        if kind == Map::Bilinear {
            let ratio = (change / pred - 1.0).abs();
            checks.at_most(
                &format!("C-B-{n} |Delta_m/Delta_pred - 1|"),
                if ratio.is_nan() { f64::INFINITY } else { ratio },
                0.1,
            );
            match &r.last {
                Some(step) => {
                    let b = balance(&map, &nodes, &wall, &step.face_forces, &step.node_displacements);
                    for (name, v) in ["resultant", "moment", "work"].iter().zip(b) {
                        checks.at_most(&format!("MB {id} {name}"), v, 1e-12);
                    }
                }
                None => checks.holds(&format!("MB {id}"), false, "a last step exists"),
            }
        }
        changes.push(change);
    }
    let e1 = (changes[0] / changes[1] - 1.0).abs();
    checks.at_most(
        &format!("E1 n{n} |Delta_m(B)/Delta_m(Q) - 1|"),
        if e1.is_nan() { f64::INFINITY } else { e1 },
        1e-6,
    );
    checks.finish();
}

#[test]
fn face_of_2_x_2_elements() {
    face_refinement(2);
}

#[test]
fn face_of_4_x_4_elements() {
    face_refinement(4);
}

#[test]
fn face_of_8_x_8_elements() {
    face_refinement(8);
}
