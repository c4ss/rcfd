//! F-385 (ADR 0286): fluid face forces in process on a solid surface set, and a static solid as a co-simulation
//! participant. Checks B1 to B4 (balance), U1, U2a to U2d (closed form, matching and 3:2 walls), S1, S2 (restore, advance),
//! C1 to C3 (coupled fixed point with a test-only mock fluid), F1 to F3 (failing solve), extensions X1, X2. The fluid-side
//! sums and the closed forms are written here, independent of the library. No flow solver is coupled.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::{hex_box, tet_box};
use rcfd_fem::loads::{nodal_load_vector, NodalForceAt};
use rcfd_fem::solve::{solve_static_with, StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Load, Material, SolidMesh, SolidModel};
use rcfd_fv::cosim::{coupled_step, CouplingSettings, Fluid, InterfaceMap, MapSettings, Participant, Structure};
use rcfd_fv::fsi::Relaxation;
use rcfd_post::solid_coupling::{face_forces_to_nodes, solve_face_forces, SolidInterface, StaticSolid};

const L: f64 = 4.0;
const W: f64 = 1.0;
const E: f64 = 2e11;
const NU: f64 = 0.3;
const P: f64 = 1e6;
const SEED: u64 = 0xF385_0286;

struct SplitMix64(u64);

impl SplitMix64 {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }
    fn force(&mut self, scale: f64) -> Vec3 {
        [
            scale * (2.0 * self.uniform() - 1.0),
            scale * (2.0 * self.uniform() - 1.0),
            scale * (2.0 * self.uniform() - 1.0),
        ]
    }
}

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

/// The bar of arithmetic 1 with the rollers.
fn bar(mesh: SolidMesh) -> SolidModel {
    let mut model = SolidModel::new(mesh);
    model
        .add_material("all", Material::isotropic_elastic(E, NU, 0.0, 0.0).unwrap())
        .unwrap();
    for (set, c) in [("xmin", 0), ("ymin", 1), ("zmin", 2)] {
        model.add_constraint(Constraint::displacement(set, c, 0.0)).unwrap();
    }
    model
}

fn hex_bar() -> SolidModel {
    bar(hex_box([0.0; 3], [L, W, W], [8, 2, 2]).unwrap())
}

fn tet_bar() -> SolidModel {
    bar(tet_box([0.0; 3], [L, W, W], [8, 2, 2]).unwrap())
}

/// A fluid wall on `x = L` of `n x n` square faces. With `permuted` the points are listed z first, y descending.
fn wall(n: usize, permuted: bool) -> (Vec<Vec3>, Vec<Vec<usize>>) {
    let index = |a: usize, b: usize| {
        if permuted {
            (n - a) * (n + 1) + b
        } else {
            b * (n + 1) + a
        }
    };
    let mut points = vec![[0.0; 3]; (n + 1) * (n + 1)];
    for a in 0..=n {
        for b in 0..=n {
            points[index(a, b)] = [L, W * a as f64 / n as f64, W * b as f64 / n as f64];
        }
    }
    let mut faces = Vec::new();
    for b in 0..n {
        for a in 0..n {
            faces.push(vec![index(a, b), index(a + 1, b), index(a + 1, b + 1), index(a, b + 1)]);
        }
    }
    (points, faces)
}

/// Area of a planar quadrilateral face: half the cross product of its diagonals.
fn face_area(points: &[Vec3], face: &[usize]) -> f64 {
    let d1 = vec3::sub(points[face[2]], points[face[0]]);
    let d2 = vec3::sub(points[face[3]], points[face[1]]);
    0.5 * vec3::norm(vec3::cross(d1, d2))
}

fn mean(points: &[Vec3], face: &[usize]) -> Vec3 {
    let s = face.iter().fold(vec3::ZERO, |a, &q| vec3::add(a, points[q]));
    vec3::scale(s, 1.0 / face.len() as f64)
}

fn map_for(interface: &SolidInterface, points: &[Vec3], faces: &[Vec<usize>]) -> InterfaceMap {
    let (map, report) = interface
        .build_map(points, faces.to_vec(), &MapSettings::projection(1e-9))
        .unwrap();
    assert!(report.precision_defect < 1e-14, "defect {}", report.precision_defect);
    map
}

fn uniform_pressure(points: &[Vec3], faces: &[Vec<usize>]) -> Vec<Vec3> {
    faces.iter().map(|f| [-P * face_area(points, f), 0.0, 0.0]).collect()
}

/// Resultant, moment (about the origin) and work on the fluid side and the solid side, each difference relative to its
/// scale (arithmetic 4). The solid side uses the load vector the solve receives and the full displacement.
fn balance(
    model: &SolidModel,
    interface: &SolidInterface,
    map: &InterfaceMap,
    points: &[Vec3],
    faces: &[Vec<usize>],
    face_forces: &[Vec3],
) -> [f64; 3] {
    let result = solve_face_forces(model, interface, map, face_forces, &StaticOptions::default()).unwrap();
    // fluid side
    let q = interface.displacements(&result);
    let d = map
        .face_displacements(&map.displacement_to_points(&q).unwrap())
        .unwrap();
    let (mut rf, mut mf, mut wf) = (vec3::ZERO, vec3::ZERO, 0.0);
    let (mut rs_scale, mut ms_scale, mut ws_scale) = (0.0, 0.0, 0.0);
    for ((face, &force), df) in faces.iter().zip(face_forces).zip(&d) {
        let c = mean(points, face);
        rf = vec3::add(rf, force);
        mf = vec3::add(mf, vec3::cross(c, force));
        wf += vec3::dot(force, *df);
        rs_scale += vec3::norm(force);
        ms_scale += vec3::norm(c) * vec3::norm(force);
        ws_scale += vec3::norm(force) * vec3::norm(*df);
    }
    // solid side: the load vector of the solve
    let nodal = face_forces_to_nodes(interface, map, face_forces).unwrap();
    let loads = interface.nodal_forces(&nodal).unwrap();
    let f = nodal_load_vector(model, &loads).unwrap();
    let u = result.displacement_values();
    let (mut rs, mut ms) = (vec3::ZERO, vec3::ZERO);
    for (n, x) in model.mesh().nodes().iter().enumerate() {
        let fn_ = [f[3 * n], f[3 * n + 1], f[3 * n + 2]];
        rs = vec3::add(rs, fn_);
        ms = vec3::add(ms, vec3::cross(*x, fn_));
    }
    let ws: f64 = f.iter().zip(u).map(|(a, b)| a * b).sum();
    [
        vec3::norm(vec3::sub(rs, rf)) / rs_scale,
        vec3::norm(vec3::sub(ms, mf)) / ms_scale,
        (ws - wf).abs() / ws_scale,
    ]
}

fn exact(x: Vec3) -> Vec3 {
    [-P * x[0] / E, NU * P * x[1] / E, NU * P * x[2] / E]
}

fn tip() -> f64 {
    P * L / E
}

fn report_balance(checks: &mut Checks, id: &str, b: [f64; 3]) {
    checks.at_most(&format!("{id} resultant"), b[0], 1e-12);
    checks.at_most(&format!("{id} moment"), b[1], 1e-12);
    checks.at_most(&format!("{id} work"), b[2], 1e-12);
}

#[test]
fn b1_to_b4_balance() {
    let mut checks = Checks::new();
    let mut rng = SplitMix64(SEED);
    // B1: matching, uniform pressure
    let model = hex_bar();
    let interface = SolidInterface::new(&model, "xmax").unwrap();
    let (points, faces) = wall(2, true);
    let map = map_for(&interface, &points, &faces);
    let b = balance(
        &model,
        &interface,
        &map,
        &points,
        &faces,
        &uniform_pressure(&points, &faces),
    );
    report_balance(&mut checks, "B1", b);
    // B2: 3:2, uniform pressure
    let (points, faces) = wall(3, false);
    let map = map_for(&interface, &points, &faces);
    let b = balance(
        &model,
        &interface,
        &map,
        &points,
        &faces,
        &uniform_pressure(&points, &faces),
    );
    report_balance(&mut checks, "B2", b);
    // B3: 3:2, random face forces, Hex8
    let forces: Vec<Vec3> = faces.iter().map(|_| rng.force(1e3)).collect();
    let b = balance(&model, &interface, &map, &points, &faces, &forces);
    report_balance(&mut checks, "B3", b);
    // B4: 3:2, random face forces, Tet4
    let model = tet_bar();
    let interface = SolidInterface::new(&model, "xmax").unwrap();
    let map = map_for(&interface, &points, &faces);
    let forces: Vec<Vec3> = faces.iter().map(|_| rng.force(1e3)).collect();
    let b = balance(&model, &interface, &map, &points, &faces, &forces);
    report_balance(&mut checks, "B4", b);
    checks.finish();
}

#[test]
fn u1_x1_matching_uniform_pressure() {
    let mut checks = Checks::new();
    let model = hex_bar();
    let interface = SolidInterface::new(&model, "xmax").unwrap();
    let (points, faces) = wall(2, true);
    let map = map_for(&interface, &points, &faces);
    let forces = uniform_pressure(&points, &faces);
    let result = solve_face_forces(&model, &interface, &map, &forces, &StaticOptions::default()).unwrap();
    let mut err: f64 = 0.0;
    for (n, x) in model.mesh().nodes().iter().enumerate() {
        let u = result.displacement(n);
        let ue = exact(*x);
        for c in 0..3 {
            err = err.max((u[c] - ue[c]).abs());
        }
    }
    for (q, x) in interface.displacements(&result).iter().zip(interface.coordinates()) {
        let ue = exact(*x);
        for c in 0..3 {
            err = err.max((q[c] - ue[c]).abs());
        }
    }
    checks.at_most("U1", err / tip(), 1e-10);
    // X1: the mapped nodal forces against the consistent load of Load::pressure
    let nodal = face_forces_to_nodes(&interface, &map, &forces).unwrap();
    let mapped = nodal_load_vector(&model, &interface.nodal_forces(&nodal).unwrap()).unwrap();
    let mut pressed = model.clone();
    pressed.add_load(Load::pressure("xmax", P)).unwrap();
    let consistent = nodal_load_vector(&pressed, &[]).unwrap();
    let largest = consistent.iter().fold(0.0f64, |a, b| a.max(b.abs()));
    let diff = mapped
        .iter()
        .zip(&consistent)
        .fold(0.0f64, |a, (x, y)| a.max((x - y).abs()));
    checks.at_most("X1", diff / largest, 1e-14);
    checks.finish();
}

#[test]
fn u2_three_to_two_uniform_pressure() {
    let mut checks = Checks::new();
    let model = hex_bar();
    let interface = SolidInterface::new(&model, "xmax").unwrap();
    let (points, faces) = wall(3, false);
    let map = map_for(&interface, &points, &faces);
    let forces = uniform_pressure(&points, &faces);
    let result = solve_face_forces(&model, &interface, &map, &forces, &StaticOptions::default()).unwrap();
    let mut pressed = model.clone();
    pressed.add_load(Load::pressure("xmax", P)).unwrap();
    let consistent = nodal_load_vector(&pressed, &[]).unwrap();
    let nodes = model.mesh().nodes();
    // U2a: the mean of u_x on xmax weighted by the consistent load
    let xmax = model.mesh().node_set("xmax").unwrap();
    let (mut num, mut den) = (0.0, 0.0);
    for &n in xmax {
        num += consistent[3 * n] * result.displacement(n)[0];
        den += consistent[3 * n];
    }
    checks.at_most("U2a", (num / den - (-tip())).abs() / tip(), 1e-10);
    // U2b, U2d: the largest error on xmax; U2c: on x <= L/2
    let err_at = |n: usize| {
        let u = result.displacement(n);
        let ue = exact(nodes[n]);
        (0..3).fold(0.0f64, |a, c| a.max((u[c] - ue[c]).abs())) / tip()
    };
    let face = xmax.iter().fold(0.0f64, |a, &n| a.max(err_at(n)));
    let far = (0..nodes.len())
        .filter(|&n| nodes[n][0] <= 0.5 * L + 1e-12)
        .fold(0.0f64, |a, n| a.max(err_at(n)));
    checks.at_most("U2b", face, 0.25);
    checks.at_most("U2c", far, 0.01);
    checks.at_least("U2d", face, 0.005);
    for &n in xmax {
        println!("  node {n} at {:?}: error {:e}", nodes[n], err_at(n));
    }
    checks.finish();
}

// ---- the static solid as a participant --------------------------------------------------------------------------------

fn solid() -> StaticSolid {
    StaticSolid::new("solid", hex_bar(), "xmax", StaticOptions::default()).unwrap()
}

fn failing() -> StaticOptions {
    StaticOptions::with_solver(StaticSolver::ConjugateGradient {
        tolerance: 1e-14,
        max_iterations: 1,
    })
}

type Bits = (u64, Vec<u64>, Vec<u64>);

fn solid_bits(s: &StaticSolid) -> Bits {
    (
        s.time().to_bits(),
        s.displacement().iter().map(|x| x.to_bits()).collect(),
        s.forces().iter().flatten().map(|x| x.to_bits()).collect(),
    )
}

#[test]
fn s1_restore_bit_for_bit() {
    let mut checks = Checks::new();
    let mut rng = SplitMix64(SEED ^ 1);
    let mut s = solid();
    let n = s.interface().n_nodes();
    let fa: Vec<Vec3> = (0..n).map(|_| rng.force(1e4)).collect();
    let fb: Vec<Vec3> = (0..n).map(|_| rng.force(1e4)).collect();
    s.advance(1e-3, &fa).unwrap();
    s.checkpoint().unwrap();
    let at_checkpoint = solid_bits(&s);
    let q1 = s.advance(1e-3, &fb).unwrap();
    let after = solid_bits(&s);
    s.restore().unwrap();
    checks.holds(
        "S1 restore",
        solid_bits(&s) == at_checkpoint,
        "state bit-equal to the checkpoint",
    );
    checks.holds("S1 changed", after != at_checkpoint, "the advance changed the state");
    let q2 = s.advance(1e-3, &fb).unwrap();
    let bits = |q: &[Vec3]| q.iter().flatten().map(|x| x.to_bits()).collect::<Vec<u64>>();
    checks.holds(
        "S1 repeat",
        bits(&q1) == bits(&q2) && solid_bits(&s) == after,
        "the advance from the checkpoint repeats bit for bit",
    );
    s.restore().unwrap();
    checks.holds(
        "S1 again",
        solid_bits(&s) == at_checkpoint,
        "a second restore gives the checkpoint",
    );
    checks.finish();
}

#[test]
fn s2_advance_is_a_static_solve() {
    let mut checks = Checks::new();
    let mut rng = SplitMix64(SEED ^ 2);
    let mut s = solid();
    let model = hex_bar();
    let nodes = s.interface().nodes().to_vec();
    for k in 0..2 {
        let f: Vec<Vec3> = nodes.iter().map(|_| rng.force(1e4)).collect();
        let t0 = s.time();
        let q = s.advance(1e-3, &f).unwrap();
        let loads: Vec<NodalForceAt> = nodes.iter().zip(&f).map(|(&n, &x)| NodalForceAt::new(n, x)).collect();
        let r = solve_static_with(&model, &[], &loads, &StaticOptions::default()).unwrap();
        let expected: Vec<u64> = nodes
            .iter()
            .flat_map(|&n| r.displacement(n).iter().map(|x| x.to_bits()).collect::<Vec<_>>())
            .collect();
        let got: Vec<u64> = q.iter().flatten().map(|x| x.to_bits()).collect();
        let full = r.displacement_values().iter().map(|x| x.to_bits()).collect::<Vec<_>>()
            == s.displacement().iter().map(|x| x.to_bits()).collect::<Vec<_>>();
        checks.holds(
            &format!("S2 advance {k}"),
            got == expected && full,
            "bit-equal to solve_static_with",
        );
        checks.holds(
            &format!("S2 time {k}"),
            s.time().to_bits() == (t0 + 1e-3).to_bits(),
            "time t0 + dt",
        );
    }
    checks.finish();
}

/// The test-only mock fluid of arithmetic 8: the force on its wall falls linearly with the area-weighted mean x
/// displacement of its faces.
#[derive(Debug, Clone)]
struct LinearWall {
    points: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    areas: Vec<f64>,
    f0: f64,
    kf: f64,
    t: f64,
    d: Vec<Vec3>,
    saved: Option<(f64, Vec<Vec3>)>,
}

impl LinearWall {
    fn new(f0: f64, kf: f64) -> Self {
        let (points, faces) = wall(2, true);
        let areas = faces.iter().map(|f| face_area(&points, f)).collect();
        let n = points.len();
        LinearWall {
            points,
            faces,
            areas,
            f0,
            kf,
            t: 0.0,
            d: vec![vec3::ZERO; n],
            saved: None,
        }
    }
    fn bits(&self) -> (u64, Vec<u64>) {
        (self.t.to_bits(), self.d.iter().flatten().map(|x| x.to_bits()).collect())
    }
    fn area(&self) -> f64 {
        self.areas.iter().sum()
    }
}

impl Participant for LinearWall {
    fn name(&self) -> &str {
        "fluid"
    }
    fn time(&self) -> f64 {
        self.t
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.saved = Some((self.t, self.d.clone()));
        Ok(())
    }
    fn restore(&mut self) -> Result<(), String> {
        let (t, d) = self.saved.as_ref().ok_or("linear wall: no checkpoint")?;
        self.t = *t;
        self.d.clone_from(d);
        Ok(())
    }
}

impl Fluid for LinearWall {
    fn point_coordinates(&self) -> Vec<Vec3> {
        self.points.clone()
    }
    fn face_points(&self) -> Vec<Vec<usize>> {
        self.faces.clone()
    }
    fn forces(&self) -> Result<Vec<Vec3>, String> {
        let a = self.area();
        let ubar: f64 = self
            .faces
            .iter()
            .zip(&self.areas)
            .map(|(f, af)| af * mean(&self.d, f)[0])
            .sum::<f64>()
            / a;
        let total = self.f0 - self.kf * ubar;
        Ok(self.areas.iter().map(|af| [af / a * total, 0.0, 0.0]).collect())
    }
    fn advance(&mut self, dt: f64, displacements: &[Vec3]) -> Result<Vec<Vec3>, String> {
        if displacements.len() != self.points.len() {
            return Err("linear wall: wrong count".into());
        }
        self.d = displacements.to_vec();
        self.t += dt;
        self.forces()
    }
}

const F0: f64 = 1e6;
const DT: f64 = 1e-3;

fn ks() -> f64 {
    E * W * W / L
}

fn u_star() -> f64 {
    F0 / (ks() + 1.5 * ks())
}

fn setup() -> (LinearWall, StaticSolid, InterfaceMap) {
    let fluid = LinearWall::new(F0, 1.5 * ks());
    let s = solid();
    let map = map_for(s.interface(), &fluid.points, &fluid.faces);
    (fluid, s, map)
}

fn with_tolerance(settings: CouplingSettings) -> CouplingSettings {
    CouplingSettings {
        tolerance: 1e-10,
        ..settings
    }
}

fn coupled(id: &str, settings: CouplingSettings, checks: &mut Checks) {
    let (mut fluid, mut s, map) = setup();
    let step = coupled_step(&mut fluid, &mut s, &map, DT, &with_tolerance(settings)).unwrap();
    checks.holds(&format!("{id} converged"), step.converged, "converged");
    let us = u_star();
    let mut err = step
        .node_displacements
        .iter()
        .fold(0.0f64, |a, q| a.max((q[0] - us).abs() / us));
    let faces = map
        .face_displacements(&map.displacement_to_points(&step.node_displacements).unwrap())
        .unwrap();
    err = faces.iter().fold(err, |a, d| a.max((d[0] - us).abs() / us));
    checks.at_most(&format!("{id} |u - u*|/u*"), err, 1e-8);
    println!("  {id}: residuals {:?}", step.residuals);
    checks.at_most(&format!("X2 {id} iterations"), step.iterations as f64, 6.0);
}

#[test]
fn c1_aitken_converges_to_two_springs() {
    let mut checks = Checks::new();
    coupled("C1", CouplingSettings::aitken(0.5), &mut checks);
    checks.finish();
}

#[test]
fn c2_least_squares_converges_to_two_springs() {
    let mut checks = Checks::new();
    coupled("C2", CouplingSettings::least_squares(0.5), &mut checks);
    checks.finish();
}

#[test]
fn c3_plain_fixed_point_fails() {
    let mut checks = Checks::new();
    let (mut fluid, mut s, map) = setup();
    let settings = CouplingSettings {
        relaxation: Relaxation::Fixed(1.0),
        ..with_tolerance(CouplingSettings::aitken(0.5))
    };
    let r = coupled_step(&mut fluid, &mut s, &map, DT, &settings);
    match r {
        Err(e) => {
            println!("  C3: {e}");
            checks.holds("C3", e.participant == "coupler", "the step fails at the cap (coupler)");
        }
        Ok(_) => checks.holds("C3", false, "the step fails at the cap (coupler)"),
    }
    checks.finish();
}

#[test]
fn f1_failing_solve_in_the_first_iteration() {
    let mut checks = Checks::new();
    let (mut fluid, mut s, map) = setup();
    let settings = with_tolerance(CouplingSettings::aitken(0.5));
    coupled_step(&mut fluid, &mut s, &map, DT, &settings).unwrap();
    let (fb, sb) = (fluid.bits(), solid_bits(&s));
    s.options = failing();
    match coupled_step(&mut fluid, &mut s, &map, DT, &settings) {
        Err(e) => {
            println!("  F1: {e}");
            checks.holds(
                "F1 located",
                e.participant == "solid" && e.iteration == 1 && e.message.contains("cg-jacobi"),
                "participant solid, iteration 1, the solver's text",
            );
        }
        Ok(_) => checks.holds("F1 located", false, "the step fails"),
    }
    checks.holds(
        "F1 restored",
        fluid.bits() == fb && solid_bits(&s) == sb,
        "both states bit-equal to before the step",
    );
    checks.finish();
}

/// Switches the static solid's options to a failing solve before its second advance.
struct FailSecond {
    inner: StaticSolid,
    calls: usize,
}

impl Participant for FailSecond {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn time(&self) -> f64 {
        self.inner.time()
    }
    fn checkpoint(&mut self) -> Result<(), String> {
        self.inner.checkpoint()
    }
    fn restore(&mut self) -> Result<(), String> {
        self.inner.restore()
    }
}

impl Structure for FailSecond {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.inner.node_coordinates()
    }
    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        self.calls += 1;
        if self.calls == 2 {
            self.inner.options = failing();
        }
        self.inner.advance(dt, forces)
    }
}

#[test]
#[ignore = "record of ADR 0286 amendment 1: the set-up never reaches iteration 2 (the step starts at the fixed point); corrected by f2b"]
fn f2_failing_solve_in_the_second_iteration() {
    let mut checks = Checks::new();
    let (mut fluid, mut s, map) = setup();
    let settings = with_tolerance(CouplingSettings::aitken(0.5));
    coupled_step(&mut fluid, &mut s, &map, DT, &settings).unwrap();
    let (fb, sb) = (fluid.bits(), solid_bits(&s));
    let mut wrapped = FailSecond { inner: s, calls: 0 };
    match coupled_step(&mut fluid, &mut wrapped, &map, DT, &settings) {
        Err(e) => {
            println!("  F2: {e}");
            checks.holds(
                "F2 located",
                e.participant == "solid" && e.iteration == 2 && e.message.contains("cg-jacobi"),
                "participant solid, iteration 2, the solver's text",
            );
        }
        Ok(_) => checks.holds("F2 located", false, "the step fails"),
    }
    checks.holds(
        "F2 restored",
        fluid.bits() == fb && solid_bits(&wrapped.inner) == sb,
        "both states bit-equal to before the step",
    );
    checks.finish();
}

/// ADR 0286 amendment 1: as F2, with the mock's F0 doubled before the failing step so that it does not start at its fixed
/// point and reaches iteration 2.
#[test]
fn f2b_failing_solve_in_the_second_iteration_after_a_load_step() {
    let mut checks = Checks::new();
    let (mut fluid, mut s, map) = setup();
    let settings = with_tolerance(CouplingSettings::aitken(0.5));
    coupled_step(&mut fluid, &mut s, &map, DT, &settings).unwrap();
    fluid.f0 = 2.0 * F0;
    let (fb, sb) = (fluid.bits(), solid_bits(&s));
    let mut wrapped = FailSecond { inner: s, calls: 0 };
    match coupled_step(&mut fluid, &mut wrapped, &map, DT, &settings) {
        Err(e) => {
            println!("  F2b: {e}");
            checks.holds(
                "F2b located",
                e.participant == "solid" && e.iteration == 2 && e.message.contains("cg-jacobi"),
                "participant solid, iteration 2, the solver's text",
            );
        }
        Ok(_) => checks.holds("F2b located", false, "the step fails"),
    }
    checks.holds(
        "F2b restored",
        fluid.bits() == fb && solid_bits(&wrapped.inner) == sb,
        "both states bit-equal to before the step",
    );
    checks.holds(
        "F2b reached",
        wrapped.calls >= 2,
        "the solid advanced twice in the step",
    );
    checks.finish();
}

#[test]
fn f3_failed_advance_leaves_the_state() {
    let mut checks = Checks::new();
    let mut rng = SplitMix64(SEED ^ 3);
    let mut s = solid();
    checks.holds(
        "F3 name",
        s.name() == "solid",
        "the participant's name is the given one",
    );
    let n = s.interface().n_nodes();
    let f: Vec<Vec3> = (0..n).map(|_| rng.force(1e4)).collect();
    s.advance(DT, &f).unwrap();
    let before = solid_bits(&s);
    s.options = failing();
    let cases: [(&str, f64, Vec<Vec3>); 3] = [
        ("failing solve", DT, f.clone()),
        ("wrong count", DT, f[..n - 1].to_vec()),
        ("zero step", 0.0, f.clone()),
    ];
    for (what, dt, forces) in cases {
        let r = s.advance(dt, &forces);
        let named = matches!(&r, Err(m) if m.contains("static solid 'solid'"));
        println!("  F3 {what}: {r:?}");
        checks.holds(
            &format!("F3 {what}"),
            named && solid_bits(&s) == before,
            "error naming solid, state unchanged",
        );
    }
    checks.finish();
}
