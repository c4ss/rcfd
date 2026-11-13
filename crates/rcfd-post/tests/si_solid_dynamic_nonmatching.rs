//! F-502 (ADR 0412): the first coupled frequency of the dynamic solid on a matching and a 3:2 interface, for solid end
//! faces of 2 x 2, 4 x 4 and 8 x 8 elements, with the projection and the radial-basis map of F-362, against the change
//! predicted from the static compliance `w^T K^{-1} w` of the mapped weights (ADR 0412 D1 to D5); the symmetry of the
//! projection's weights and its cause (D6). Checks C-P-n, C-R-n, C0, B1, B2, S1 to S5, the companions C-Q-n, FM, FU, K0,
//! I, MB, PD and the negative control N1. The set-up and the piston mock are those of ADR 0410 (copied: test helpers are
//! not shared between test crates). No flow solver is coupled.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::solve::StaticOptions;
use rcfd_fem::transient::TransientOptions;
use rcfd_fem::{Constraint, Material, SolidModel};
use rcfd_fv::cosim::{coupled_step, CoupledStep, CouplingSettings, Fluid, InterfaceMap, MapSettings, Participant};
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
    Projection,
    RadialBasis,
    /// The bilinear evaluation of ADR 0412 D6 (test-only).
    Bilinear,
}

impl Map {
    fn name(self) -> &'static str {
        match self {
            Map::Projection => "projection",
            Map::RadialBasis => "radial basis",
            Map::Bilinear => "bilinear Q",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Map::Projection => "P",
            Map::RadialBasis => "R",
            Map::Bilinear => "Q",
        }
    }
}

/// Local coordinates `(s, t)` of `x` on an axis-aligned rectangular element along its edges `0 -> 1` and `0 -> 3`, if
/// `x` lies on it.
fn local(c: &[Vec3], x: Vec3) -> Option<(f64, f64)> {
    let e1 = vec3::sub(c[1], c[0]);
    let e3 = vec3::sub(c[3], c[0]);
    let r = vec3::sub(x, c[0]);
    let s = vec3::dot(r, e1) / vec3::dot(e1, e1);
    let t = vec3::dot(r, e3) / vec3::dot(e3, e3);
    let tol = 1e-12;
    (s >= -tol && s <= 1.0 + tol && t >= -tol && t <= 1.0 + tol).then(|| (s.clamp(0.0, 1.0), t.clamp(0.0, 1.0)))
}

/// Rows of a test-side map: on the first interface element containing the point, the bilinear shape functions
/// (`bilinear`) or the barycentric coordinates of the triangles `(0, 1, 2)`, `(0, 2, 3)` of the corner order (ADR 0412 D6).
fn test_rows(interface: &SolidInterface, points: &[Vec3], bilinear: bool) -> Vec<Vec<(usize, f64)>> {
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
            let w = if bilinear {
                [(1.0 - s) * (1.0 - t), s * (1.0 - t), s * t, (1.0 - s) * t]
            } else if s >= t {
                [1.0 - s, s - t, t, 0.0]
            } else {
                [1.0 - t, 0.0, s, t - s]
            };
            el.iter().zip(w).filter(|r| r.1 != 0.0).map(|(&i, w)| (i, w)).collect()
        })
        .collect()
}

/// The map of `kind` from the solid's interface to `wall`; its precision defect is checked (PD).
fn build(checks: &mut Checks, id: &str, interface: &SolidInterface, wall: &PistonWall, kind: Map, n: usize) -> InterfaceMap {
    let h = W / n as f64;
    match kind {
        Map::Projection | Map::RadialBasis => {
            let settings = if kind == Map::Projection {
                MapSettings::projection(1e-9)
            } else {
                MapSettings::radial_basis(3.0 * h, 1e-9)
            };
            let (map, report) = interface.build_map(&wall.points, wall.faces.clone(), &settings).unwrap();
            let limit = if kind == Map::Projection { 1e-14 } else { 1e-12 };
            checks.at_most(&format!("PD {id} precision defect"), report.precision_defect, limit);
            map
        }
        Map::Bilinear => {
            let rows = test_rows(interface, &wall.points, true);
            let nodes = interface.coordinates();
            let defect = rows
                .iter()
                .zip(&wall.points)
                .map(|(row, &x)| {
                    let image = row.iter().fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(nodes[i], w)));
                    vec3::norm(vec3::sub(image, x))
                })
                .fold(0.0, f64::max);
            checks.at_most(&format!("PD {id} precision defect"), defect, 1e-14);
            InterfaceMap::new(rows, nodes.len(), wall.faces.clone()).unwrap()
        }
    }
}

/// Axial interface weights `w = B^T (A_f/A)` of a map.
fn weights(map: &InterfaceMap, wall: &PistonWall) -> Vec<f64> {
    map.forces_to_nodes(&wall.unit_forces()).unwrap().iter().map(|f| f[0]).collect()
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
fn matching_omega_2(p: &Setup) -> f64 {
    let model = bar(2);
    let interface = SolidInterface::new(&model, "xmax").unwrap();
    let wall = PistonWall::new(2, true, p.dt, p.m_a);
    let mut scratch = Checks::new();
    let map = build(&mut scratch, "FU reference", &interface, &wall, Map::Projection, 2);
    run(p, &model, wall, &map, 4).0.omega_m
}

/// Every check of ADR 0412 for the face of `n x n` elements.
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

    // C0: the static compliances and the predictions, before any coupled run.
    let matching_wall = PistonWall::new(n, true, p.dt, p.m_a);
    let matching_map = build(&mut checks, &format!("matching n{n}"), &interface, &matching_wall, Map::Projection, n);
    let c0 = compliance(&model, &interface, &matching_map, &matching_wall);
    checks.at_most(&format!("K0 n{n} |c_w0 E A/L - 1|"), (c0 * E * W * W / L - 1.0).abs(), 1e-9);
    let kinds = [Map::Projection, Map::RadialBasis, Map::Bilinear];
    let mut predicted = Vec::new();
    for kind in kinds {
        let wall = PistonWall::new(m, false, p.dt, p.m_a);
        let id = format!("{} n{n}", kind.id());
        let map = build(&mut checks, &id, &interface, &wall, kind, n);
        let dc = compliance(&model, &interface, &map, &wall) - c0;
        let pred = p.predicted_change(dc);
        let crude = -dc * E * W * W / (2.0 * L);
        println!(
            "  C0 {id} ({}): Dc E A/L {:.9e}, predicted change {pred:.9e}, crude {crude:.9e} (before the coupled runs)",
            kind.name(),
            dc * E * W * W / L
        );
        predicted.push((kind, wall, map, pred, crude));
    }
    checks.holds(&format!("C0 n{n}"), true, "every prediction computed and printed before the first coupled run");

    // S1 to S5: the weights on the 3:2 wall.
    for (kind, wall, map, _, _) in &predicted {
        let w = weights(map, wall);
        let id = format!("{} n{n}", kind.id());
        if *kind == Map::Projection {
            let tri = InterfaceMap::new(test_rows(&interface, &wall.points, false), nodes.len(), wall.faces.clone()).unwrap();
            let wt = weights(&tri, wall);
            let scale = w.iter().fold(0.0f64, |a, x| a.max(x.abs()));
            let dev = w.iter().zip(&wt).fold(0.0f64, |a, (x, y)| a.max((x - y).abs())) / scale;
            checks.at_most(&format!("S1 {id} |w - w_tri|/max w"), dev, 1e-13);
        }
        for (k, name) in SYMMETRIES.iter().enumerate() {
            let defect = symmetry_defect(&nodes, &w, k);
            match (kind, k < 3) {
                (Map::Projection, true) => checks.at_most(&format!("S2 {id} {name}"), defect, 1e-13),
                (Map::Projection, false) => checks.at_least(&format!("S3 {id} {name}"), defect, 0.05),
                (Map::Bilinear, _) => checks.at_most(&format!("S4 {id} {name}"), defect, 1e-13),
                (Map::RadialBasis, _) => checks.at_most(&format!("S5 {id} {name}"), defect, 1e-12),
            }
        }
    }

    // The coupled runs: matching, then the three 3:2 maps.
    println!("  matching n{n}:");
    let (matching, _) = run(&p, &model, matching_wall, &matching_map, 4);
    check_iterations(&mut checks, &format!("matching n{n}"), &matching, 4);
    checks.at_most(
        &format!("FM n{n} |Omega_m/Omega_pred,0 - 1|"),
        (matching.omega_m / p.omega_pred0 - 1.0).abs(),
        2e-5,
    );
    if n != 2 {
        let omega_2 = matching_omega_2(&p);
        checks.at_most(
            &format!("FU n{n} |Omega_m(n)/Omega_m(2) - 1|"),
            (matching.omega_m / omega_2 - 1.0).abs(),
            1e-9,
        );
    }
    let mut below = (Vec::new(), Vec::new());
    for (kind, wall, map, pred, crude) in predicted {
        let id = format!("{} n{n}", kind.id());
        println!("  3:2 {id} ({}):", kind.name());
        let (r, wall) = run(&p, &model, wall, &map, 20);
        check_iterations(&mut checks, &id, &r, 20);
        let change = r.omega_m / matching.omega_m - 1.0;
        println!("  {id}: measured change {change:.9e}, predicted {pred:.9e}");
        let ratio = (change / pred - 1.0).abs();
        let cid = match kind {
            Map::Projection => "C-P",
            Map::RadialBasis => "C-R",
            Map::Bilinear => "C-Q",
        };
        checks.at_most(
            &format!("{cid}-{n} |Delta_m/Delta_pred - 1|"),
            if ratio.is_nan() { f64::INFINITY } else { ratio },
            0.1,
        );
        let crude_ratio = (change / crude - 1.0).abs();
        checks.at_least(
            &format!("N1 {id} |Delta_m/Delta_crude - 1|"),
            if crude_ratio.is_nan() { 0.0 } else { crude_ratio },
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
        if change.abs() < 1e-3 {
            below.0.push(id.clone());
        }
        if pred.abs() < 1e-3 {
            below.1.push(id);
        }
    }
    println!("B1 n{n}: measured |Delta_m| < 1e-3: {:?}", below.0);
    println!("    predicted |Delta_pred| < 1e-3: {:?}", below.1);
    checks.holds(
        &format!("B2 n{n}"),
        below.0 == below.1,
        "the measured list below 1e-3 equals the predicted list",
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
