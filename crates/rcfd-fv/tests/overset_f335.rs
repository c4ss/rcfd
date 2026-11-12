//! F-335 (ADR 0238): outer iterations on the overset path, an opt-in of `OversetSolver` (`set_outer_iterations`).
//!
//! Q0o, Q0r, Q0n: with every cell calculated the step with three outer iterations is the library's ALE step with
//! `n_outer = 3`, with and without under-relaxation. U1o, U3o, U1r, U3r, N2o: a uniform flow through a moving, turning
//! component stays uniform. L1, L1r: the Taylor-Green set-up at level 1. D (slow): the acceptance, the time error
//! `max |d_200|` of TG_2 at `dt = 0.0025` and T2 at level 2 of ADR 0219 with three outer iterations. E4 to E7: refusals.
//! The set-ups BF, U, TG_L, TGB_L and the settings PS are those of ADR 0219 (`overset_f134.rs`).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::{move_mesh, set_follow_conditions};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::overset::{MassMode, OversetCoupling, OversetSolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{cylinder_ogrid, rectilinear_2d, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{compose, CellKind, Overset, OversetSpec};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const DEPTH: f64 = 0.1;
/// Solver controls of every set-up (ADR 0219 PS).
const SOLVE: SolveControl = SolveControl {
    tol_abs: 1e-15,
    tol_rel: 1e-12,
    max_iter: 3000,
};

/// ADR 0219 PS.
fn settings(nu: f64) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.momentum_solver = SOLVE;
    s.pressure_solver = SOLVE;
    s
}

/// The outer iterations of a run: the number and the two under-relaxation factors of the non-final iterations.
#[derive(Clone, Copy, Debug)]
struct Outer {
    n: usize,
    alpha_u: f64,
    alpha_p: f64,
}

/// Three outer iterations without under-relaxation ("three outer iterations" of ADR 0238).
const THREE: Outer = Outer {
    n: 3,
    alpha_u: 1.0,
    alpha_p: 1.0,
};
/// Three outer iterations with the factors of `PimpleSettings::pimple` ("relaxed" of ADR 0238).
const RELAXED: Outer = Outer {
    n: 3,
    alpha_u: 0.8,
    alpha_p: 0.5,
};

fn opt_in(s: &mut OversetSolver, o: Option<Outer>) {
    if let Some(o) = o {
        s.set_outer_iterations(o.n, o.alpha_u, o.alpha_p).unwrap();
    }
}

/// PS with the outer iterations `o` (for the library's solver).
fn settings_with(nu: f64, o: Outer) -> PimpleSettings {
    let mut s = settings(nu);
    s.n_outer = o.n;
    s.alpha_u = o.alpha_u;
    s.alpha_p = o.alpha_p;
    s
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
    /// `value < limit`.
    fn below(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{id}: {value:.4e} (limit < {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} >= {limit:e}"));
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
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn geometry_at(ov: &Overset, pose: &Pose) -> Arc<FvGeometry> {
    let mesh = moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

fn box_overset(bg: Mesh, half: f64, n: usize, hole: f64) -> Overset {
    let cs = uniform(-half, 2.0 * half, n);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    let composite = compose(&bg, &comp, "c_").unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    };
    Overset::new(composite, spec).unwrap()
}

// ---- Q: BF of ADR 0219, every cell calculated --------------------------------------------------------------------------

const NU_BF: f64 = 0.025;
const DT_BF: f64 = 0.02;

fn displacement(t: f64) -> f64 {
    if t <= 1.0 {
        -(0.5 * t - (PI * t).sin() / (2.0 * PI))
    } else {
        -(0.5 + (t - 1.0))
    }
}

fn bf_mesh() -> Mesh {
    let q: f64 = 1.08;
    let r_out = 0.5 + 0.01 * (q.powi(66) - 1.0) / (q - 1.0);
    cylinder_ogrid(96, 66, 0.5, r_out, Grading::Expansion(q.powi(65)), DEPTH).unwrap()
}

/// BF for 20 steps: the library's ALE step with the outer iterations `lib` and the overset step with every cell calculated
/// and the outer iterations `ov` (`None`: no opt-in).
fn bf_pair(lib: Outer, ov: Option<Outer>) -> (PimpleSolver, OversetSolver) {
    let mesh0 = bf_mesh();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh0.clone())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for name in ["cylinder", "farfield"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut a = PimpleSolver::new(geom, u, p, settings_with(NU_BF, lib));
    let mut b = OversetSolver::new(
        a.geom.clone(),
        a.u.clone(),
        a.p.clone(),
        settings(NU_BF),
        MassMode::Conservative,
        None,
    )
    .unwrap();
    opt_in(&mut b, ov);
    let walls = vec!["cylinder".to_string()];
    for n in 1..=20 {
        let d = displacement(n as f64 * DT_BF);
        let pts: Vec<Vec3> = mesh0.points().iter().map(|q| [q[0] + d, q[1], q[2]]).collect();
        let st = move_mesh(&a.geom, pts.clone(), DT_BF).unwrap();
        let old = a.geom.mesh.clone();
        let dp = set_follow_conditions(&mut a.u, &st, &old, &walls, DT_BF).unwrap();
        a.step_ale_with_walls(DT_BF, st.geom, st.mesh_flux, dp).unwrap();
        let st = move_mesh(&b.geom, pts, DT_BF).unwrap();
        let old = b.geom.mesh.clone();
        let dp = set_follow_conditions(&mut b.u, &st, &old, &walls, DT_BF).unwrap();
        b.advance_mesh(DT_BF, st, dp).unwrap();
    }
    (a, b)
}

/// The largest differences of `u`, `p` and `phi` (over the largest flux) and whether the two states are equal bit for bit.
fn bf_difference(a: &PimpleSolver, b: &OversetSolver) -> (f64, f64, f64, bool) {
    let du =
        a.u.values
            .iter()
            .zip(&b.u.values)
            .map(|(x, y)| vec3::norm(vec3::sub(*x, *y)))
            .fold(0.0, f64::max);
    let dp =
        a.p.values
            .iter()
            .zip(&b.p.values)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
    let phi_max = a.phi.iter().map(|x| x.abs()).fold(0.0, f64::max);
    let dphi = a.phi.iter().zip(&b.phi).map(|(x, y)| (x - y).abs()).fold(0.0, f64::max) / phi_max;
    let bits = a.u.values == b.u.values && a.p.values == b.p.values && a.phi == b.phi && a.time == b.time;
    (du, dp, dphi, bits)
}

/// ADR 0238 Q0o, Q0r and the negative control Q0n.
#[test]
fn q0_outer_iterations_with_every_cell_calculated_equal_the_library_ale_step() {
    let mut c = Checks::new();
    for (id, o) in [("Q0o", THREE), ("Q0r", RELAXED)] {
        let (a, b) = bf_pair(o, Some(o));
        let (du, dp, dphi, bits) = bf_difference(&a, &b);
        println!("{id}: bit for bit {bits}");
        c.at_most(&format!("{id} u"), du, 1e-12);
        c.at_most(&format!("{id} p"), dp, 1e-12);
        c.at_most(&format!("{id} phi"), dphi, 1e-12);
    }
    let (a, b) = bf_pair(THREE, None);
    let (du, ..) = bf_difference(&a, &b);
    c.at_least(
        "Q0n (negative control) library with three against overset with one, u",
        du,
        1e-8,
    );
    c.finish();
}

// ---- U of ADR 0219 -----------------------------------------------------------------------------------------------------

const U_FLOW: Vec3 = [1.0, 0.5, 0.0];
const DT_U: f64 = 0.01;

fn u_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [t, 0.5 * t, 0.0],
    }
}

fn u_new() -> OversetSolver {
    let bs = uniform(-2.5, 5.0, 50);
    let ov = box_overset(rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 16, 0.25);
    let pose = u_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::uniform("U", &geom, U_FLOW);
    for name in ["xmin", "ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(U_FLOW))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    OversetSolver::new(
        geom,
        u,
        p,
        settings(0.01),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap()
}

/// The U run with the outer iterations `o`; `perturb` multiplies every stencil weight by 1.001 after `prepare` (N2o).
/// Returns the largest velocity and pressure deviations, continuity and outflow error over the steps.
fn u_run(o: Outer, perturb: bool) -> (f64, f64, f64, f64) {
    let mut s = u_new();
    opt_in(&mut s, Some(o));
    let n = s.geom.n_cells();
    let un = vec3::norm(U_FLOW);
    let (mut eu, mut ep, mut cont, mut q) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for k in 1..=80 {
        let mut st = s.prepare(DT_U, u_pose(k as f64 * DT_U), &[]).unwrap();
        if perturb {
            for (_, stn) in st.conn.stencils.iter_mut() {
                stn.weights.iter_mut().for_each(|w| *w *= 1.001);
            }
        }
        let info = s.advance(DT_U, st).unwrap();
        cont = cont.max(info.continuity);
        for c in 0..n {
            if s.conn.kind[c] != CellKind::Hole {
                eu = eu.max(vec3::norm(vec3::sub(s.u.values[c], U_FLOW)) / un);
                ep = ep.max(s.p.values[c].abs() / (un * un));
            }
        }
        let qx: f64 = s.geom.mesh.patch("xmax").unwrap().faces().map(|f| s.phi[f]).sum();
        q = q.max((qx - 0.5).abs() / 0.5);
    }
    (eu, ep, cont, q)
}

/// ADR 0238 U1o, U3o, U1r, U3r.
#[test]
fn u_uniform_flow_stays_uniform_with_outer_iterations() {
    let mut c = Checks::new();
    for (tag, o) in [("o", THREE), ("r", RELAXED)] {
        let (eu, ep, cont, q) = u_run(o, false);
        c.at_most(&format!("U1{tag} velocity"), eu, 1e-10);
        c.at_most(&format!("U1{tag} pressure"), ep, 1e-10);
        c.at_most(&format!("U3{tag} continuity"), cont, 1e-9);
        c.at_most(&format!("U3{tag} outflow"), q, 1e-10);
    }
    c.finish();
}

/// ADR 0238 N2o (negative control): every stencil weight multiplied by 1.001, three outer iterations.
#[test]
fn n2o_uniform_flow_negative_control_perturbed_weights() {
    let (eu, ..) = u_run(THREE, true);
    let mut c = Checks::new();
    c.at_least("N2o velocity deviation", eu, 1e-6);
    c.finish();
}

// ---- E -----------------------------------------------------------------------------------------------------------------

/// ADR 0238 E4 to E7.
#[test]
fn e_refusals_of_the_outer_iterations() {
    let mut c = Checks::new();
    let mut s = u_new();
    let before = (s.settings.n_outer, s.settings.alpha_u, s.settings.alpha_p);
    for (what, n, au, ap) in [
        ("n_outer 0", 0, 1.0, 1.0),
        ("alpha_u 0", 3, 0.0, 1.0),
        ("alpha_u 1.5", 3, 1.5, 1.0),
        ("alpha_u NaN", 3, f64::NAN, 1.0),
        ("alpha_p 0", 3, 1.0, 0.0),
        ("alpha_p 1.5", 3, 1.0, 1.5),
    ] {
        let r = s.set_outer_iterations(n, au, ap);
        let msg = r.as_ref().err().cloned().unwrap_or_default();
        let same = (s.settings.n_outer, s.settings.alpha_u, s.settings.alpha_p) == before;
        c.holds(
            "E4",
            r.is_err() && same,
            &format!("{what} refused, settings unchanged: {msg}"),
        );
    }
    // E5: no opt-in, the setting written directly
    let mut s = u_new();
    s.settings.n_outer = 3;
    let state = (s.u.values.clone(), s.p.values.clone(), s.phi.clone(), s.time);
    let st = s.prepare(DT_U, u_pose(DT_U), &[]).unwrap();
    let r = s.advance(DT_U, st);
    let msg = r.as_ref().err().cloned().unwrap_or_default();
    c.holds(
        "E5",
        r.is_err() && (s.u.values.clone(), s.p.values.clone(), s.phi.clone(), s.time) == state,
        &format!("n_outer 3 without the opt-in refused, state unchanged: {msg}"),
    );
    // E6: opted in, a factor written directly
    let mut s = u_new();
    s.set_outer_iterations(3, 1.0, 1.0).unwrap();
    s.settings.alpha_u = 0.0;
    let state = (s.u.values.clone(), s.p.values.clone(), s.phi.clone(), s.time);
    let st = s.prepare(DT_U, u_pose(DT_U), &[]).unwrap();
    let r = s.advance(DT_U, st);
    let msg = r.as_ref().err().cloned().unwrap_or_default();
    c.holds(
        "E6",
        r.is_err() && (s.u.values.clone(), s.p.values.clone(), s.phi.clone(), s.time) == state,
        &format!("alpha_u 0 on an opted-in solver refused, state unchanged: {msg}"),
    );
    // E7
    let mut s = u_new();
    let ok = s.set_outer_iterations(2, 1.0, 1.0).is_ok() && s.step(DT_U, u_pose(DT_U), &[]).is_ok();
    c.holds("E7", ok, "two outer iterations accepted and one step taken");
    c.finish();
}

// ---- TG_L and TGB_L of ADR 0219 ------------------------------------------------------------------------------------------

const NU_TG: f64 = 0.1;
const T_END: f64 = 0.5;

fn tg_u(x: Vec3, t: f64) -> Vec3 {
    let e = (-2.0 * NU_TG * t).exp();
    [-x[0].cos() * x[1].sin() * e, x[0].sin() * x[1].cos() * e, 0.0]
}

fn tg_p(x: Vec3, t: f64) -> f64 {
    -((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * NU_TG * t).exp() / 4.0
}

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

fn tg_background(level: usize) -> Mesh {
    let bs = uniform(0.0, PI, 40 * level);
    rectilinear_2d(&bs, &bs, DEPTH).unwrap()
}

fn tg_conditions(geom: &FvGeometry, u: &mut VectorField, t: f64) {
    for name in ["xmin", "xmax", "ymin", "ymax"] {
        let v: Vec<Vec3> = geom
            .mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| tg_u(geom.mesh.face_centres()[f], t))
            .collect();
        u.set_bc(geom, name, Box::new(FixedValue(Profile::PerFace(v)))).unwrap();
    }
}

/// TGB_L: the background alone with the library step and `n_outer` outer iterations (no under-relaxation), `steps` steps
/// to `t = 0.5`; the error `e`.
fn tg_reference(level: usize, steps: usize, n_outer: usize) -> f64 {
    let dt = T_END / steps as f64;
    let geom = Arc::new(FvGeometry::new(Arc::new(tg_background(level))).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut set = settings(NU_TG);
    set.n_outer = n_outer;
    let mut s = PimpleSolver::new(geom.clone(), u, p, set);
    for k in 1..=steps {
        tg_conditions(&geom, &mut s.u, k as f64 * dt);
        s.step(dt);
    }
    let decay = (-2.0 * NU_TG * T_END).exp();
    (0..geom.n_cells())
        .map(|c| vec3::norm(vec3::sub(s.u.values[c], tg_u(geom.mesh.cell_centres()[c], T_END))) / decay)
        .fold(0.0, f64::max)
}

/// One run of TG_L: the solver at `t = 0.5`, the error `e` of ADR 0219 (largest over the non-hole cells), the largest
/// continuity, the range of `delta`, the pressure iterations and the unconverged linear solves over the steps, the wall time.
struct Run {
    s: OversetSolver,
    e: f64,
    cont: f64,
    delta: (f64, f64),
    p_iters: usize,
    unconverged: usize,
    secs: f64,
}

/// TG_L in `steps` steps to `t = 0.5` with the outer iterations `o` (`None`: no opt-in, the solver of ADR 0219).
fn tg_run(level: usize, steps: usize, o: Option<Outer>) -> Run {
    let t0 = Instant::now();
    let dt = T_END / steps as f64;
    let ov = box_overset(tg_background(level), 0.6, 20 * level, 0.25);
    let pose = tg_pose(0.0);
    let geom = geometry_at(&ov, &pose);
    let mut u = VectorField::from_fn("U", &geom, |x| tg_u(x, 0.0));
    tg_conditions(&geom, &mut u, 0.0);
    let p = ScalarField::from_fn("p", &geom, |x| tg_p(x, 0.0));
    let mut s = OversetSolver::new(
        geom,
        u,
        p,
        settings(NU_TG),
        MassMode::Conservative,
        Some(OversetCoupling { overset: ov, pose }),
    )
    .unwrap();
    opt_in(&mut s, o);
    let (mut cont, mut dlo, mut dhi, mut p_iters, mut unconverged) =
        (0.0f64, f64::INFINITY, f64::NEG_INFINITY, 0usize, 0usize);
    for k in 1..=steps {
        let t = k as f64 * dt;
        let g = s.geom.clone();
        tg_conditions(&g, &mut s.u, t);
        let info = s.step(dt, tg_pose(t), &[]).unwrap();
        cont = cont.max(info.continuity);
        dlo = dlo.min(info.delta);
        dhi = dhi.max(info.delta);
        p_iters += info.p_iters;
        unconverged += info.unconverged;
    }
    let decay = (-2.0 * NU_TG * T_END).exp();
    let e = (0..s.geom.n_cells())
        .filter(|&c| s.conn.kind[c] != CellKind::Hole)
        .map(|c| vec3::norm(vec3::sub(s.u.values[c], tg_u(s.geom.mesh.cell_centres()[c], T_END))) / decay)
        .fold(0.0, f64::max);
    let r = Run {
        s,
        e,
        cont,
        delta: (dlo, dhi),
        p_iters,
        unconverged,
        secs: t0.elapsed().as_secs_f64(),
    };
    println!(
        "TG_{level} {steps} steps, outer {:?} ({:.1} s): e {:.4e}, continuity {:.2e}, delta [{:.3e}, {:.3e}], pressure iterations per step {:.1}, unconverged solves {}",
        o.map(|o| (o.n, o.alpha_u, o.alpha_p)),
        r.secs,
        r.e,
        r.cont,
        r.delta.0,
        r.delta.1,
        r.p_iters as f64 / steps as f64,
        r.unconverged
    );
    r
}

/// Cell classes: component fringe, component calculated, background fringe, background calculated.
const CLASSES: [&str; 4] = ["CF", "CC", "BF", "BC"];

fn class_of(s: &OversetSolver, c: usize) -> Option<usize> {
    match (s.conn.kind[c], c < s.conn.n_background_cells) {
        (CellKind::Hole, _) => None,
        (CellKind::Fringe, false) => Some(0),
        (CellKind::Calculated, false) => Some(1),
        (CellKind::Fringe, true) => Some(2),
        (CellKind::Calculated, true) => Some(3),
    }
}

/// The difference field of two runs of one level at `t = 0.5` (`d_200` of ADR 0232 for runs of 200 and 800 steps): the
/// largest `|u_a - u_b|` over the decay per class, and the largest over the non-hole cells with its cell.
fn difference(a: &Run, b: &Run) -> ([f64; 4], f64, usize) {
    assert!(a.s.conn.kind == b.s.conn.kind, "the two runs end at different kinds");
    let decay = (-2.0 * NU_TG * T_END).exp();
    let mut per = [0.0f64; 4];
    let mut best = (0.0, 0);
    for c in 0..a.s.geom.n_cells() {
        if let Some(k) = class_of(&a.s, c) {
            let d = vec3::norm(vec3::sub(a.s.u.values[c], b.s.u.values[c])) / decay;
            per[k] = per[k].max(d);
            if d > best.0 {
                best = (d, c);
            }
        }
    }
    (per, best.0, best.1)
}

fn describe(s: &OversetSolver, c: usize) -> String {
    let x = s.geom.mesh.cell_centres()[c];
    format!(
        "cell {c} ({}) at ({:.3}, {:.3})",
        class_of(s, c).map_or("hole", |k| CLASSES[k]),
        x[0],
        x[1]
    )
}

/// `max |d_200(n)|` of TG_2 with the outer iterations `o` in both runs, printed per class. Returns it with the two runs.
fn d200(tag: &str, o: Option<Outer>) -> (f64, Run, Run) {
    let (a, b) = (tg_run(2, 200, o), tg_run(2, 800, o));
    let (per, d, cell) = difference(&a, &b);
    println!(
        "{tag}: max |d_200| {d:.4e} in {}; CF {:.4e}, CC {:.4e}, BF {:.4e}, BC {:.4e}",
        describe(&b.s, cell),
        per[0],
        per[1],
        per[2],
        per[3]
    );
    (d, a, b)
}

/// ADR 0238 L1 and L1r: TG_1 in 200 steps with three outer iterations, plain and relaxed.
#[test]
fn l1_taylor_green_level_1_with_outer_iterations() {
    let eb1 = tg_reference(1, 200, 1);
    println!("L1: e(TGB_1) {eb1:.4e}");
    let mut c = Checks::new();
    for (id, o) in [("L1", THREE), ("L1r", RELAXED)] {
        let r = tg_run(1, 200, Some(o));
        c.at_most(&format!("{id} TG_1 / TGB_1"), r.e / eb1, 2.0);
        c.at_most(&format!("{id} continuity"), r.cont, 1e-9);
    }
    c.finish();
}

/// The measurements of D: S1, D1, T2o-a, T2o-b, T3o, with everything ADR 0238 prints beside them.
struct Acceptance {
    d1: f64,
    d3: f64,
    ratio_a: f64,
    ratio_b: f64,
    cont: f64,
}

fn acceptance() -> Acceptance {
    let (eb_1, eb_3) = (tg_reference(2, 200, 1), tg_reference(2, 200, 3));
    println!("D: e(TGB_2) in 200 steps: one outer iteration {eb_1:.4e}, three {eb_3:.4e}");
    let (d1, a1, b1) = d200("D one outer iteration", None);
    let (d3, a3, b3) = d200("D three outer iterations", Some(THREE));
    let e1_3 = tg_run(1, 200, Some(THREE)).e;
    println!(
        "D: e(TG_2) in 200 steps: one outer iteration {:.4e} ({:.4} e(TGB_2)), three {:.4e}; in 800 steps: one {:.4e}, three {:.4e}",
        a1.e,
        a1.e / eb_1,
        a3.e,
        b1.e,
        b3.e
    );
    println!(
        "D (printed, T1 of ADR 0219 with three outer iterations, limit >= 1.7, not checked): {:.4} (e(TG_1) {e1_3:.4e})",
        (e1_3 / a3.e).log2()
    );
    Acceptance {
        d1,
        d3,
        ratio_a: a3.e / eb_1,
        ratio_b: a3.e / eb_3,
        cont: a1.cont.max(b1.cont).max(a3.cont).max(b3.cont),
    }
}

/// ADR 0238 S1, D1, T2o-a, T2o-b and T3o: the acceptance of F-335.
#[test]
#[ignore = "slow (four level-2 runs of ADR 0238, about 5 minutes): run once for the addendum"]
fn d_time_error_and_t2_at_level_2_with_three_outer_iterations() {
    let m = acceptance();
    let mut c = Checks::new();
    c.at_least("S1 (set-up) max |d_200| with one outer iteration", m.d1, 4.47e-4);
    c.at_most("S1 (set-up) max |d_200| with one outer iteration", m.d1, 4.52e-4);
    c.below("D1 max |d_200| with three outer iterations", m.d3, 1.2e-4);
    c.at_most("T2o-a e(TG_2) / e(TGB_2 as recorded)", m.ratio_a, 2.0);
    c.at_most("T2o-b e(TG_2) / e(TGB_2 with three outer iterations)", m.ratio_b, 2.0);
    c.at_most("T3o continuity, every step of the four runs", m.cont, 1e-9);
    c.finish();
}

/// ADR 0238, printed only: the trend of `max |d_200|` over the number of outer iterations (two and five).
#[test]
#[ignore = "slow (four level-2 runs with two and five outer iterations, about 8 minutes): printed only, run once"]
fn d_trend_with_two_and_five_outer_iterations() {
    for n in [2, 5] {
        let o = Outer {
            n,
            alpha_u: 1.0,
            alpha_p: 1.0,
        };
        let (d, a, _) = d200(&format!("trend, {n} outer iterations"), Some(o));
        println!(
            "trend: n_outer {n}: max |d_200| {d:.4e}; e(TG_2) in 200 steps {:.4e}",
            a.e
        );
    }
}
