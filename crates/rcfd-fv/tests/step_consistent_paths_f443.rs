//! F-443 (ADR 0352): the step-consistent face coefficient of ADR 0245 (`PimpleSettings::rhie_chow_step_consistent`, "C")
//! on the remaining paths: with the consistent correctors (K1), on `step_ale_with_walls` (K2), with a body force (the
//! hydrostatic column H1, the buoyant cavity B1 and B2), and the paths it is not taken on: the fractional step (F1, F0) and
//! `step_sliding` (S1). The limits were registered in ADR 0352 before this file existed. Helpers are copied from
//! `bdf2_remainder_f344.rs`, `ale_gcl.rs`, `transient_buoyancy.rs` and `sliding_interface_f250.rs` (not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::energy::{BuoyantPimple, EnergySolver};
use rcfd_fv::pimple::fractional::FractionalSettings;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::sliding::sliding_step;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_2d_graded, cavity_2d, cylinder_ogrid, Grading};
use rcfd_mesh::sliding::SlidingInterface;
use rcfd_mesh::Mesh;
use std::sync::Arc;

/// Solver controls of every set-up (ADR 0219 PS).
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

/// ADR 0219 PS (as `bdf2_remainder_f344.rs`).
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// The options of a run: C (`rhie_chow_step_consistent`) and CC (the consistent correctors, with their body-force flag).
#[derive(Debug, Clone, Copy)]
struct Opt {
    c: bool,
    cc: bool,
}

const C: Opt = Opt { c: true, cc: false };
const CC_C: Opt = Opt { c: true, cc: true };
const CC: Opt = Opt { c: false, cc: true };
const DEFAULT: Opt = Opt { c: false, cc: false };

fn apply(s: &mut PimpleSettings, o: Opt) {
    s.rhie_chow_step_consistent = o.c;
    if o.cc {
        s.consistent_correctors = true;
        s.consistent_body_force = true;
    }
}

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    /// `value <= limit`.
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    /// `value >= limit`.
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.4e} (limit >= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: String) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn field_distance(a: &[Vec3], b: &[Vec3]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(p, q)| vec3::norm(vec3::sub(*p, *q)))
        .fold(0.0, f64::max)
}

fn max_speed(a: &[Vec3]) -> f64 {
    a.iter().map(|v| vec3::norm(*v)).fold(0.0, f64::max)
}

/// The result of a steady run: the cell velocities, the largest change of the velocity over the last step, the largest
/// continuity over the steps.
struct Steady {
    u: Vec<Vec3>,
    change: f64,
    continuity: f64,
}

// ---- K1: the consistent correctors ------------------------------------------------------------------------------------

fn cavity_solver(scheme: TimeScheme, o: Opt) -> PimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(20, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut set = settings(0.1);
    set.time_scheme = scheme;
    set.n_outer = 3;
    apply(&mut set, o);
    PimpleSolver::new(geom, u, p, set)
}

/// CAV of ADR 0352 (ADR 0245 C3) from rest to `t = 20` in steps of `dt` on the fixed mesh.
fn cavity_run(dt: f64, scheme: TimeScheme, o: Opt) -> Steady {
    let mut s = cavity_solver(scheme, o);
    let steps = (20.0 / dt).round() as usize;
    let (mut change, mut continuity) = (0.0f64, 0.0f64);
    for k in 0..steps {
        let before = s.u.values.clone();
        let info = s.step(dt);
        continuity = continuity.max(info.continuity);
        if k + 1 == steps {
            change = field_distance(&s.u.values, &before);
        }
    }
    assert!(s.diverged.is_none(), "the cavity run stopped ({o:?}, dt {dt})");
    Steady {
        u: s.u.values.clone(),
        change,
        continuity,
    }
}

/// ADR 0352 K1, K1n, K1c, K1d: C3 of ADR 0245 with the consistent correctors.
#[test]
fn k1_steady_cavity_with_the_consistent_correctors() {
    let mut c = Checks::new();
    let a = cavity_run(0.04, TimeScheme::Bdf2, CC_C);
    let b = cavity_run(0.01, TimeScheme::Bdf2, CC_C);
    println!(
        "K1 change over the last step: {:.3e} (dt 0.04), {:.3e} (dt 0.01)",
        a.change, b.change
    );
    c.at_most(
        "K1 steady cavity with C and CC, steps 0.04 against 0.01",
        field_distance(&a.u, &b.u),
        1e-9,
    );
    c.at_most("K1d continuity of the K1 runs", a.continuity.max(b.continuity), 1e-9);
    let r = cavity_run(0.01, TimeScheme::Bdf2, C);
    c.at_most(
        "K1c C with CC against C with the default correctors, dt 0.01",
        field_distance(&b.u, &r.u),
        1e-9,
    );
    let na = cavity_run(0.04, TimeScheme::Bdf2, CC);
    let nb = cavity_run(0.01, TimeScheme::Bdf2, CC);
    println!(
        "K1n change over the last step: {:.3e} (dt 0.04), {:.3e} (dt 0.01)",
        na.change, nb.change
    );
    c.at_least(
        "K1n (negative control) steady cavity with CC without C, steps 0.04 against 0.01",
        field_distance(&na.u, &nb.u),
        1e-6,
    );
    c.finish();
}

// ---- K2: step_ale_with_walls ------------------------------------------------------------------------------------------

/// The frame velocity of CAVT.
const V: Vec3 = [0.6, 0.2, 0.0];

/// CAVT of ADR 0352: CAV in a frame translating with `V` through `step_ale_with_walls` (implicit Euler); returns `u - V`.
fn translating_run(dt: f64, o: Opt) -> Steady {
    let mesh: Mesh = cavity_2d(20, 1.0).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, V);
    u.set_bc(&geom, "walls", Box::new(FixedValue(Profile::Uniform(V))))
        .unwrap();
    u.set_bc(
        &geom,
        "lid",
        Box::new(FixedValue(Profile::Uniform(vec3::add(V, [1.0, 0.0, 0.0])))),
    )
    .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut set = settings(0.1);
    set.time_scheme = TimeScheme::ImplicitEuler;
    set.n_outer = 3;
    apply(&mut set, o);
    let mut s = PimpleSolver::new(geom, u, p, set);
    let names = vec!["walls".to_string()];
    let steps = (20.0 / dt).round() as usize;
    let (mut change, mut continuity) = (0.0f64, 0.0f64);
    for n in 1..=steps {
        let t = n as f64 * dt;
        let pts: Vec<Vec3> = mesh.points().iter().map(|&q| vec3::add(q, vec3::scale(V, t))).collect();
        let st = move_mesh(&s.geom, pts, dt).unwrap();
        let old = s.geom.mesh.clone();
        let walls = set_follow_conditions(&mut s.u, &st, &old, &names, dt).unwrap();
        let before = s.u.values.clone();
        let info = s.step_ale_with_walls(dt, st.geom, st.mesh_flux, walls).unwrap();
        continuity = continuity.max(info.continuity);
        if n == steps {
            change = field_distance(&s.u.values, &before);
        }
    }
    assert!(s.diverged.is_none(), "the translating cavity stopped ({o:?}, dt {dt})");
    Steady {
        u: s.u.values.iter().map(|x| vec3::sub(*x, V)).collect(),
        change,
        continuity,
    }
}

/// ADR 0352 K2, K2n, K2c, K2d: C3 of ADR 0245 on `step_ale_with_walls`.
#[test]
fn k2_steady_cavity_on_a_translating_mesh() {
    let mut c = Checks::new();
    let a = translating_run(0.04, C);
    let b = translating_run(0.01, C);
    println!(
        "K2 change over the last step: {:.3e} (dt 0.04), {:.3e} (dt 0.01)",
        a.change, b.change
    );
    c.at_most(
        "K2 steady translating cavity with C, steps 0.04 against 0.01 (u - V)",
        field_distance(&a.u, &b.u),
        1e-9,
    );
    c.at_most("K2d continuity of the K2 runs", a.continuity.max(b.continuity), 1e-9);
    let e = cavity_run(0.01, TimeScheme::ImplicitEuler, C);
    c.at_most(
        "K2c translating (u - V) against the fixed mesh, both C, Euler, dt 0.01",
        field_distance(&b.u, &e.u),
        1e-9,
    );
    let na = translating_run(0.04, DEFAULT);
    let nb = translating_run(0.01, DEFAULT);
    println!(
        "K2n change over the last step: {:.3e} (dt 0.04), {:.3e} (dt 0.01)",
        na.change, nb.change
    );
    c.at_least(
        "K2n (negative control) steady translating cavity without C, steps 0.04 against 0.01",
        field_distance(&na.u, &nb.u),
        1e-6,
    );
    // printed, no limit: C with the consistent correctors on the translating mesh
    let ca = translating_run(0.04, CC_C);
    let cb = translating_run(0.01, CC_C);
    println!(
        "K2 printed: C and CC on the translating mesh, steps 0.04 against 0.01: {:.3e}; against C alone at 0.01: {:.3e}",
        field_distance(&ca.u, &cb.u),
        field_distance(&cb.u, &b.u)
    );
    c.finish();
}

// ---- H1: the hydrostatic column ---------------------------------------------------------------------------------------

const GRAVITY: Vec3 = [0.0, -9.81, 0.0];

/// COL of ADR 0352: 100 steps of 0.01; returns the largest cell speed after the last step and over every step.
fn column_run(hydrostatic_start: bool, o: Opt) -> (f64, f64) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(10, 20, 1.0, 2.0, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = if hydrostatic_start {
        ScalarField::from_fn("p", &geom, |x| vec3::dot(GRAVITY, x))
    } else {
        ScalarField::uniform("p", &geom, 0.0)
    };
    let mut set = settings(0.1);
    set.time_scheme = TimeScheme::Bdf2;
    set.n_outer = 3;
    apply(&mut set, o);
    let mut s = PimpleSolver::new(geom.clone(), u, p, set);
    s.body_force = Some(BodyForce::uniform(&geom, GRAVITY));
    let mut worst = 0.0f64;
    for _ in 0..100 {
        s.step(0.01);
        worst = worst.max(max_speed(&s.u.values));
    }
    assert!(s.diverged.is_none(), "the column stopped ({o:?})");
    (max_speed(&s.u.values), worst)
}

/// ADR 0352 H1a, H1b, H1c: a hydrostatic column stays at rest with C (and with C and the consistent correctors).
#[test]
fn h1_hydrostatic_column_stays_at_rest() {
    let mut c = Checks::new();
    let (end0, _) = column_run(false, C);
    let (_, all_h) = column_run(true, C);
    c.at_most("H1a column from p = 0 with C: speed after step 100", end0, 1e-10);
    c.at_most(
        "H1b column from the hydrostatic p with C: speed over every step",
        all_h,
        1e-10,
    );
    let (end0c, _) = column_run(false, CC_C);
    let (_, all_hc) = column_run(true, CC_C);
    c.at_most(
        "H1c column from p = 0 with C and CC: speed after step 100",
        end0c,
        1e-10,
    );
    c.at_most(
        "H1c column from the hydrostatic p with C and CC: speed over every step",
        all_hc,
        1e-10,
    );
    let (d0, _) = column_run(false, DEFAULT);
    let (_, dh) = column_run(true, DEFAULT);
    println!("H1 printed, the default: speed after step 100 from p = 0 {d0:.3e}, over every step from the hydrostatic p {dh:.3e}");
    c.finish();
}

// ---- B1, B2: the buoyant cavity ---------------------------------------------------------------------------------------

const PR: f64 = 0.71;
const RA: f64 = 1e3;

/// BUO of ADR 0352 to `t = 8`: the steady result, the temperature and the largest speed.
fn buoyant_run(dt: f64, o: Opt) -> (Steady, Vec<f64>, f64) {
    let mesh = Arc::new(box_2d_graded(20, 20, 1.0, 1.0, [Grading::Uniform; 2], false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut t = ScalarField::uniform("T", &geom, 0.5);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut set = settings(PR);
    set.time_scheme = TimeScheme::Bdf2;
    set.n_outer = 3;
    apply(&mut set, o);
    let flow = PimpleSolver::new(geom.clone(), u, p, set);
    let mut energy = EnergySolver::new(geom.clone(), t, 1.0);
    energy.convection = ConvScheme::Central;
    energy.solver.tol_rel = 1e-12;
    energy.solver.tol_abs = 1e-300;
    energy.solver.max_iter = 5000;
    let mut sim = BuoyantPimple {
        flow,
        energy,
        beta: 1.0,
        gravity: [0.0, -RA * PR, 0.0],
        t_ref: 0.5,
    };
    let steps = (8.0 / dt).round() as usize;
    let (mut change, mut continuity) = (0.0f64, 0.0f64);
    for k in 0..steps {
        let before = sim.flow.u.values.clone();
        let (info, _) = sim.step(dt);
        continuity = continuity.max(info.continuity);
        if k + 1 == steps {
            change = field_distance(&sim.flow.u.values, &before);
        }
    }
    assert!(
        sim.flow.diverged.is_none(),
        "the buoyant cavity stopped ({o:?}, dt {dt})"
    );
    let speed = max_speed(&sim.flow.u.values);
    (
        Steady {
            u: sim.flow.u.values.clone(),
            change,
            continuity,
        },
        sim.energy.t.values.clone(),
        speed,
    )
}

fn buoyant_pair(c: &mut Checks, id: &str, o: Opt, limit_at_most: bool) -> f64 {
    let (a, ta, _) = buoyant_run(0.004, o);
    let (b, tb, speed) = buoyant_run(0.001, o);
    let abs = field_distance(&a.u, &b.u);
    let dtemp = ta.iter().zip(&tb).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max);
    println!(
        "{id} ({o:?}): largest speed {speed:.5}; absolute difference {abs:.3e}; temperature difference {dtemp:.3e}; change over \
         the last step {:.3e} (dt 0.004), {:.3e} (dt 0.001)",
        a.change, b.change
    );
    let rel = abs / speed;
    if limit_at_most {
        c.at_most(
            &format!("{id} steady buoyant cavity, steps 0.004 against 0.001, relative"),
            rel,
            1e-9,
        );
        c.at_most(
            &format!("B3 continuity of the {id} runs"),
            a.continuity.max(b.continuity),
            1e-9,
        );
    } else {
        c.at_least(
            &format!("{id} (negative control) steady buoyant cavity without C, relative"),
            rel,
            1e-6,
        );
    }
    rel
}

/// ADR 0352 B1, B1n, B3: the steady buoyant cavity does not depend on the step with C.
#[test]
#[ignore = "heavy (four buoyant-cavity runs of 2000 and 8000 steps): run once for the addendum; H1 is the fast test of the body-force path of C"]
fn b1_steady_buoyant_cavity_with_c() {
    let mut c = Checks::new();
    buoyant_pair(&mut c, "B1", C, true);
    buoyant_pair(&mut c, "B1n", DEFAULT, false);
    c.finish();
}

/// ADR 0352 B2, B3: the same with the consistent correctors.
#[test]
#[ignore = "heavy (two buoyant-cavity runs of 2000 and 8000 steps): run once for the addendum; H1c is the fast test of the path"]
fn b2_steady_buoyant_cavity_with_c_and_the_consistent_correctors() {
    let mut c = Checks::new();
    buoyant_pair(&mut c, "B2", CC_C, true);
    c.finish();
}

// ---- F1, F0: the fractional step ----------------------------------------------------------------------------------

fn fractional_solver(flag: bool) -> PimpleSolver {
    let mut s = cavity_solver(TimeScheme::ImplicitEuler, DEFAULT);
    s.settings.fractional = Some(FractionalSettings::default());
    s.settings.rhie_chow_step_consistent = flag;
    s
}

fn state_bits(s: &PimpleSolver) -> Vec<u64> {
    let mut v: Vec<u64> = s.u.values.iter().flatten().map(|x| x.to_bits()).collect();
    v.extend(s.p.values.iter().chain(&s.phi).map(|x| x.to_bits()));
    v
}

/// ADR 0352 F1 and F0: the fractional step does not read the flag; its own step dependence, printed.
#[test]
fn f1_the_fractional_step_keeps_the_default_step() {
    let mut c = Checks::new();
    let (mut a, mut b) = (fractional_solver(true), fractional_solver(false));
    let mut equal = true;
    for _ in 0..100 {
        a.step(0.04);
        b.step(0.04);
        equal &= state_bits(&a) == state_bits(&b);
    }
    c.holds(
        "F1 fractional step with the flag against without, 100 steps of 0.04",
        equal,
        format!("bit for bit {equal}"),
    );
    let run = |dt: f64| {
        let mut s = fractional_solver(true);
        let steps = (20.0 / dt).round() as usize;
        for _ in 0..steps {
            s.step(dt);
        }
        (s.u.values.clone(), s.diverged.is_some())
    };
    let ((u4, d4), (u1, d1)) = (run(0.04), run(0.01));
    println!(
        "F0 printed: the fractional step's steady cavity, steps 0.04 against 0.01: {:.4e} (stopped: {d4}, {d1})",
        field_distance(&u4, &u1)
    );
    c.finish();
}

// ---- S1: step_sliding ---------------------------------------------------------------------------------------------

const L: f64 = 0.1;
const RI: f64 = 1.0;
const RM: f64 = 1.5;
const RO: f64 = 2.0;
const Z: Vec3 = [0.0, 0.0, 1.0];
const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-13,
    max_iter: 5000,
};

fn pair(n_stator: usize) -> SlidingInterface {
    let a = cylinder_ogrid(144, 8, RI, RM, Grading::Uniform, L).unwrap();
    let b = cylinder_ogrid(n_stator, 8, RM, RO, Grading::Uniform, L).unwrap();
    SlidingInterface::cylindrical(a, "farfield", b, "cylinder", vec3::ZERO, Z, 1e-8).unwrap()
}

fn tc_solver(g: Arc<FvGeometry>, nu: f64, nonorth: usize, flag: bool) -> PimpleSolver {
    let mut u = VectorField::uniform("U", &g, vec3::ZERO);
    let cf = g.mesh.face_centres();
    let wall: Vec<Vec3> = g
        .mesh
        .patch("cylinder")
        .unwrap()
        .faces()
        .map(|k| vec3::cross(Z, cf[k]))
        .collect();
    u.set_bc(&g, "cylinder", Box::new(FixedValue(Profile::PerFace(wall))))
        .unwrap();
    u.set_bc(&g, "farfield", Box::new(FixedValue::vector(vec3::ZERO)))
        .unwrap();
    let p = ScalarField::uniform("p", &g, 0.0);
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.non_orth_correctors = nonorth;
    s.momentum_solver = TIGHT;
    s.pressure_solver = TIGHT;
    s.rhie_chow_step_consistent = flag;
    PimpleSolver::new(g, u, p, s)
}

/// ADR 0352 S1: `step_sliding` keeps the default step with the flag set, and leaves the flag set.
#[test]
fn s1_step_sliding_keeps_the_default_step() {
    let mut c = Checks::new();
    let sc = pair(120);
    let dt = 0.005;
    let m0 = sc.mesh_at(0.0).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(m0.mesh.clone())).unwrap());
    let mut on = tc_solver(geom.clone(), 1.0, 2, true);
    let mut off = tc_solver(geom, 1.0, 2, false);
    let mut cur = m0;
    let mut equal = true;
    for k in 1..=3 {
        let new = sc.mesh_at(k as f64 * dt).unwrap();
        let st_on = sliding_step(&on.geom, &cur, &new, &on.phi, &on.u.values, dt).unwrap();
        let st_off = sliding_step(&off.geom, &cur, &new, &off.phi, &off.u.values, dt).unwrap();
        on.step_sliding(dt, st_on, Vec::new()).unwrap();
        off.step_sliding(dt, st_off, Vec::new()).unwrap();
        let e = state_bits(&on) == state_bits(&off);
        println!("S1 step {k}: bit for bit {e}");
        equal &= e;
        cur = new;
    }
    c.holds(
        "S1 three sliding steps with the flag against without",
        equal,
        format!("bit for bit {equal}"),
    );
    let set = on.settings.rhie_chow_step_consistent;
    c.holds("S1 the flag after the steps", set, format!("set {set}"));
    c.finish();
}
