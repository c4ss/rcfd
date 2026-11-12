//! F-463 (ADR 0361): the failing static solve in iteration 2 and in iteration 3 of a coupled step. The StaticSolid and the
//! mock fluid of ADR 0286 (copied from `sb_solid_coupling.rs`, which is not edited): a converged step, the mock's F0 doubled,
//! then a step that starts away from its fixed point and whose solid solve fails in a later iteration. Checks P1 to P4
//! (controls: the healthy step reaches iteration 3), A2, A3 (Aitken), L2, L3 (least squares), N1. No flow solver is coupled.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fem::builders::hex_box;
use rcfd_fem::solve::{StaticOptions, StaticSolver};
use rcfd_fem::{Constraint, Material, SolidMesh, SolidModel};
use rcfd_fv::cosim::{
    coupled_step, CoupledStep, CouplingSettings, Fluid, InterfaceMap, MapSettings, Participant, Structure,
};
use rcfd_post::solid_coupling::{SolidInterface, StaticSolid};

const L: f64 = 4.0;
const W: f64 = 1.0;
const E: f64 = 2e11;
const NU: f64 = 0.3;

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

// ---- ADR 0361 ---------------------------------------------------------------------------------------------------------

/// Switches the static solid's options to a failing solve at its `fail_at`-th advance (`usize::MAX`: never).
struct FailAt {
    inner: StaticSolid,
    fail_at: usize,
    calls: usize,
}

impl FailAt {
    fn new(inner: StaticSolid, fail_at: usize) -> Self {
        FailAt {
            inner,
            fail_at,
            calls: 0,
        }
    }
}

impl Participant for FailAt {
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

impl Structure for FailAt {
    fn node_coordinates(&self) -> Vec<Vec3> {
        self.inner.node_coordinates()
    }
    fn advance(&mut self, dt: f64, forces: &[Vec3]) -> Result<Vec<Vec3>, String> {
        self.calls += 1;
        if self.calls == self.fail_at {
            self.inner.options = failing();
        }
        self.inner.advance(dt, forces)
    }
}

/// The start of the step of ADR 0361: a converged step, then the mock's F0 doubled.
fn start(settings: &CouplingSettings) -> (LinearWall, StaticSolid, InterfaceMap) {
    let (mut fluid, mut s, map) = setup();
    coupled_step(&mut fluid, &mut s, &map, DT, settings).unwrap();
    fluid.f0 = 2.0 * F0;
    (fluid, s, map)
}

fn bits_of(q: &[Vec3]) -> Vec<u64> {
    q.iter().flatten().map(|x| x.to_bits()).collect()
}

fn residual_bits(step: &CoupledStep) -> Vec<u64> {
    step.residuals.iter().map(|x| x.to_bits()).collect()
}

/// The healthy step from the start state with a transparent wrapper (P1, P2, P3, P4): returns the step, after checking the
/// registered numbers.
fn control(id: &str, settings: &CouplingSettings, checks: &mut Checks) -> CoupledStep {
    let (mut fluid, s, map) = start(settings);
    let (fb, sb) = (fluid.bits(), solid_bits(&s));
    let mut wrapped = FailAt::new(s, usize::MAX);
    let step = coupled_step(&mut fluid, &mut wrapped, &map, DT, settings).unwrap();
    println!("  {id} residuals {:?}", step.residuals);
    checks.holds(
        &format!("{id}a"),
        step.converged && step.iterations == 3,
        "the healthy step converges in 3 iterations",
    );
    let rel = |x: f64, y: f64| (x - y).abs() / y;
    checks.at_most(&format!("{id}b"), rel(step.residuals[0], 15.0), 1e-6);
    checks.at_most(&format!("{id}c"), rel(step.residuals[1], 0.375 / 1.025), 1e-6);
    checks.at_most(&format!("{id}d"), step.residuals[2], 1e-10);
    checks.holds(
        &format!("{id}e"),
        wrapped.calls == 3,
        "the solid advanced 3 times (iteration 3 exists)",
    );
    let us2 = 2.0 * u_star();
    let err = step
        .node_displacements
        .iter()
        .fold(0.0f64, |a, q| a.max((q[0] - us2).abs() / us2));
    checks.at_most(&format!("{id} P2 |u - 2u*|/2u*"), err, 1e-8);
    checks.holds(
        &format!("{id} P3"),
        fluid.bits() != fb && solid_bits(&wrapped.inner) != sb,
        "the step changes both states",
    );
    step
}

/// A step whose solid solve fails at its `n`-th advance, from the start state (A2, A3, L2, L3).
fn failing_step(id: &str, settings: &CouplingSettings, n: usize, control: &CoupledStep, checks: &mut Checks) {
    let (mut fluid, s, map) = start(settings);
    let (fb, sb, t0) = (fluid.bits(), solid_bits(&s), fluid.time());
    let mut wrapped = FailAt::new(s, n);
    match coupled_step(&mut fluid, &mut wrapped, &map, DT, settings) {
        Err(e) => {
            println!("  {id}: {e}");
            checks.holds(
                &format!("{id}a"),
                e.participant == "solid" && e.iteration == n,
                &format!("participant solid, iteration {n}"),
            );
            checks.holds(
                &format!("{id}b"),
                e.message.contains("static solid 'solid'")
                    && e.message.contains("solver cg-jacobi: not converged in 1 iterations")
                    && !e.message.contains("restore failed"),
                "the solver's message, naming the solid, no failed restore",
            );
            checks.holds(
                &format!("{id}c"),
                e.time.to_bits() == t0.to_bits(),
                "the error's time is the start",
            );
        }
        Ok(_) => {
            checks.holds(&format!("{id}a"), false, "the step fails");
            checks.holds(&format!("{id}b"), false, "the step fails");
            checks.holds(&format!("{id}c"), false, "the step fails");
        }
    }
    checks.holds(
        &format!("{id}d"),
        wrapped.calls == n,
        "the solid advanced exactly n times",
    );
    checks.holds(
        &format!("{id}e"),
        fluid.bits() == fb && solid_bits(&wrapped.inner) == sb,
        "both states bit-equal to the start of the step",
    );
    // f: the same step again with the healthy solver reproduces the control bit for bit
    wrapped.inner.options = StaticOptions::default();
    wrapped.fail_at = usize::MAX;
    wrapped.calls = 0;
    match coupled_step(&mut fluid, &mut wrapped, &map, DT, settings) {
        Ok(step) => checks.holds(
            &format!("{id}f"),
            step.converged
                && step.iterations == 3
                && bits_of(&step.node_displacements) == bits_of(&control.node_displacements)
                && residual_bits(&step) == residual_bits(control),
            "the retry reproduces the control bit for bit",
        ),
        Err(e) => checks.holds(&format!("{id}f"), false, &format!("the retry runs: {e}")),
    }
}

#[test]
fn p1_a2_a3_aitken() {
    let mut checks = Checks::new();
    let settings = with_tolerance(CouplingSettings::aitken(0.5));
    let c = control("P1", &settings, &mut checks);
    failing_step("A2", &settings, 2, &c, &mut checks);
    failing_step("A3", &settings, 3, &c, &mut checks);
    checks.finish();
}

#[test]
fn p4_l2_l3_least_squares() {
    let mut checks = Checks::new();
    let settings = with_tolerance(CouplingSettings::least_squares(0.5));
    let c = control("P4", &settings, &mut checks);
    failing_step("L2", &settings, 2, &c, &mut checks);
    failing_step("L3", &settings, 3, &c, &mut checks);
    checks.finish();
}

#[test]
fn n1_failure_at_the_fourth_advance_is_never_reached() {
    let mut checks = Checks::new();
    let settings = with_tolerance(CouplingSettings::aitken(0.5));
    let c = control("N1 control", &settings, &mut checks);
    let (mut fluid, s, map) = start(&settings);
    let mut wrapped = FailAt::new(s, 4);
    match coupled_step(&mut fluid, &mut wrapped, &map, DT, &settings) {
        Ok(step) => checks.holds(
            "N1",
            step.converged
                && step.iterations == 3
                && wrapped.calls == 3
                && bits_of(&step.node_displacements) == bits_of(&c.node_displacements),
            "ok, 3 iterations, 3 advances, bit-equal to the control",
        ),
        Err(e) => checks.holds("N1", false, &format!("the step runs: {e}")),
    }
    checks.finish();
}
