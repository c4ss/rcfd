//! F-572 (ADR 0560): arc-length with a follower reference load (L1 to L3: the ring of ADR 0480 under the follower pressure
//! alone, against the load-controlled path of ADR 0480 at the common load factors), the unsymmetric factor of the tangent on
//! a free-edged follower load (U1 to U5: the cantilever plate, its quadratic rate, the symmetric control, the arc-length
//! trace with both options), and the refusals (C2). Each registered run is made once; a miss stays as an ignored
//! `*_known_miss` test with its limit.

use rcfd_fem::arclength::{trace, ArcLengthOptions, ArcLengthPath, PathPoint};
use rcfd_fem::builders::hex_box;
use rcfd_fem::constraint::prescribed_equations;
use rcfd_fem::contact::{solve as contact_solve, ContactOptions, Kinematics};
use rcfd_fem::error::FemError;
use rcfd_fem::follower::FollowerPressure;
use rcfd_fem::hex20::{upgrade_to_hex20, upgrade_to_hex20_mapped};
use rcfd_fem::nonlinear::{solve, solve_with, Contribution, DriverExtensions, LoadStep, NewtonOptions, StepControl};
use rcfd_fem::solve::{solve_static, DirectFactor, StaticOptions};
use rcfd_fem::{Constraint, Load, Material, SolidModel};
use std::cell::RefCell;
use std::f64::consts::PI;

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
    fn at_least(&mut self, name: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{name}: {value:.6e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.failed.push(format!("{name}: {value:e} < {limit:e}"));
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

fn material() -> Material {
    Material::isotropic_elastic(E, 0.0, 7850.0, 0.0).unwrap()
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, x| m.max(x.abs()))
}

fn rel_diff(a: &[f64], b: &[f64]) -> f64 {
    let d = a.iter().zip(b).fold(0.0f64, |m, (x, y)| m.max((x - y).abs()));
    d / max_abs(b)
}

/// A contribution that records `(load factor, u)` of every call after its inner contribution (every method forwarded).
struct Recorder<'a> {
    inner: &'a dyn Contribution,
    calls: RefCell<Vec<(f64, Vec<f64>)>>,
}

impl Recorder<'_> {
    fn new(inner: &dyn Contribution) -> Recorder<'_> {
        Recorder {
            inner,
            calls: RefCell::new(Vec::new()),
        }
    }
    /// The displacement of the last call at the load factor's bits.
    fn at(&self, load_factor: f64) -> Option<Vec<f64>> {
        self.calls
            .borrow()
            .iter()
            .rev()
            .find(|(l, _)| l.to_bits() == load_factor.to_bits())
            .map(|(_, u)| u.clone())
    }
}

impl Contribution for Recorder<'_> {
    fn name(&self) -> &str {
        "recorder"
    }
    fn add(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        self.inner.add(u, load_factor, force, tangent)?;
        self.calls.borrow_mut().push((load_factor, u.to_vec()));
        Ok(())
    }
    fn add_full(
        &self,
        u: &[f64],
        load_factor: f64,
        force: &mut [f64],
        tangent: &mut Vec<(usize, usize, f64)>,
    ) -> rcfd_fem::Result<()> {
        self.inner.add_full(u, load_factor, force, tangent)?;
        self.calls.borrow_mut().push((load_factor, u.to_vec()));
        Ok(())
    }
    fn add_load_factor_derivative(&self, u: &[f64], load_factor: f64, out: &mut [f64]) -> rcfd_fem::Result<()> {
        self.inner.add_load_factor_derivative(u, load_factor, out)
    }
}

/// The hook of reading (a2): inside the user steps, also stop at every given load factor.
struct Hits {
    targets: Vec<f64>,
}

impl StepControl for Hits {
    fn name(&self) -> &str {
        "common load factors"
    }
    fn load_factor(&mut self, committed: f64, proposed: f64, _step: usize, _halvings: usize) -> rcfd_fem::Result<f64> {
        Ok(match self.targets.iter().find(|&&t| t > committed) {
            Some(&t) if t < proposed => t,
            _ => proposed,
        })
    }
    fn converged(&mut self, _increment: &LoadStep) {}
}

/// The free equations of `model` (no extra prescribed values).
fn free_equations(model: &SolidModel) -> Vec<usize> {
    let n = model.dof_map().n_equations();
    let fixed = prescribed_equations(model, &[]).unwrap();
    let mut is_fixed = vec![false; n];
    for &(eq, _) in &fixed {
        is_fixed[eq] = true;
    }
    (0..n).filter(|&i| !is_fixed[i]).collect()
}

/// The constraint of every point from the stored points: `| |Du|^2 + c Dlambda^2 - Ds^2 | / Ds^2`, the largest.
fn constraint_error(model: &SolidModel, path: &ArcLengthPath) -> f64 {
    let free = free_equations(model);
    let n = model.dof_map().n_equations();
    let mut u_prev = vec![0.0; n];
    let mut l_prev = 0.0;
    let mut worst: f64 = 0.0;
    for p in &path.points {
        let du2: f64 = free.iter().map(|&q| (p.displacement[q] - u_prev[q]).powi(2)).sum();
        let dl = p.load_factor - l_prev;
        let e = (du2 + path.load_scale * dl * dl - p.length * p.length).abs() / (p.length * p.length);
        worst = worst.max(e);
        u_prev = p.displacement.clone();
        l_prev = p.load_factor;
    }
    worst
}

/// `Q = max r_(k+1) / r_k^2` over `k >= 1` with `r_k <= 1e-2` (reading (a3)).
fn quadratic_constant<'a>(histories: impl IntoIterator<Item = &'a Vec<f64>>) -> f64 {
    let mut q: f64 = 0.0;
    for r in histories {
        for k in 1..r.len().saturating_sub(1) {
            if r[k] <= 1e-2 {
                q = q.max(r[k + 1] / (r[k] * r[k]));
            }
        }
    }
    q
}

fn fmt_residuals(r: &[f64]) -> String {
    r.iter().map(|x| format!("{x:.2e}")).collect::<Vec<_>>().join(" ")
}

/// The linear static solution of the model with a dead pressure, squared norm: the load scale of D5.
fn load_scale(model: &SolidModel, set: &str, pressure: f64) -> f64 {
    let mut m = model.clone();
    m.add_load(Load::pressure(set, pressure)).unwrap();
    let lin = solve_static(&m, &StaticOptions::default()).unwrap();
    lin.displacement_values().iter().map(|x| x * x).sum()
}

// ------------------------------------------------------------------------------------------------------- ring

const R: f64 = 1.0;
const T: f64 = 0.02;
const BW: f64 = 0.02;

/// The quarter ring of ADR 0480 D5 with `n` Hex20 elements over the quarter (as `sa_follower.rs`).
fn ring_model(n: usize) -> SolidModel {
    let b = hex_box([R - T / 2.0, 0.0, 0.0], [T, PI / 2.0, BW], [1, n, 1]).unwrap();
    let mesh = upgrade_to_hex20_mapped(&b, |p| [p[0] * p[1].cos(), p[0] * p[1].sin(), p[2]])
        .unwrap()
        .0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    m.add_constraint(Constraint::displacement("ymin", 1, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("ymax", 0, 0.0)).unwrap();
    m.add_constraint(Constraint::displacement("all", 2, 0.0)).unwrap();
    m
}

/// `p_ref` of R1 of ADR 0480: the thin law at the stretch 1.51.
fn p_ref_ring() -> f64 {
    E * T * (1.51f64 * 1.51 - 1.0) / (2.0 * R)
}

#[test]
fn l1_l2_l3_ring_follower_only_by_arc_length() {
    let mut ck = Checks::new();
    let model = ring_model(8);
    let p_ref = p_ref_ring();
    let fp = FollowerPressure::new(&model, "xmin", p_ref).unwrap();
    let contributions: Vec<&dyn Contribution> = vec![&fp];
    // L1c: the default route refuses the follower-only reference load.
    let e = trace(
        &model,
        &[],
        &[],
        &ArcLengthOptions::new(0.1),
        &contributions,
        &mut |_: &PathPoint| false,
    );
    ck.holds(
        "L1c default route refuses (key arc length)",
        matches!(&e, Err(FemError::InvalidValue { key: "arc length", .. })),
        &format!("{:?}", e.as_ref().err()),
    );
    // L1: with the load direction of the contributions.
    let c = load_scale(&model, "xmin", p_ref);
    let options = ArcLengthOptions::new(0.05 * (2.0 * c).sqrt())
        .load_scale(c)
        .max_points(200)
        .contribution_load(true);
    let path = trace(&model, &[], &[], &options, &contributions, &mut |p: &PathPoint| {
        p.load_factor >= 1.0
    });
    let path = match path {
        Ok(p) => p,
        Err(e) => {
            ck.holds("L1 trace with contribution_load", false, &format!("error {e}"));
            ck.finish();
            return;
        }
    };
    println!("L1: {} points, load scale {:e}", path.points.len(), path.load_scale);
    for (i, p) in path.points.iter().enumerate() {
        println!(
            "  point {:3}: lambda {:.6} halvings {} residuals {}",
            i + 1,
            p.load_factor,
            p.halvings,
            fmt_residuals(&p.residuals)
        );
    }
    ck.holds(
        "L1 stopped at lambda >= 1 within 200 points",
        path.stopped && path.points.len() <= 200,
        &format!("stopped {}, {} points", path.stopped, path.points.len()),
    );
    // L3.
    ck.at_most("L3 constraint", constraint_error(&model, &path), 1e-10);
    // L2: the R1 run of ADR 0480 with the hook.
    let targets: Vec<f64> = path.points.iter().map(|p| p.load_factor).filter(|&l| l < 1.0).collect();
    ck.at_least("L2n common load factors", targets.len() as f64, 10.0);
    let rec = Recorder::new(&fp);
    let mut hook = Hits {
        targets: targets.clone(),
    };
    let ext = DriverExtensions {
        contributions: vec![&rec],
        step_control: Some(&mut hook),
    };
    let lc = solve_with(&model, &[], &[], &NewtonOptions::new(20), ext);
    match lc {
        Err(e) => ck.holds("L2 load-controlled run", false, &format!("error {e}")),
        Ok(r) => {
            println!("L2: load-controlled run, {} increments", r.steps.len());
            let mut worst: f64 = 0.0;
            for p in path.points.iter().filter(|p| p.load_factor < 1.0) {
                match rec.at(p.load_factor) {
                    Some(u) => {
                        let d = rel_diff(&p.displacement, &u);
                        println!("  lambda {:.6}: {d:.3e}", p.load_factor);
                        worst = worst.max(d);
                    }
                    None => {
                        worst = f64::INFINITY;
                        println!("  lambda {:.6}: not reached by the load-controlled run", p.load_factor);
                    }
                }
            }
            ck.at_most(
                "L2 arc-length against load control at the common load factors",
                worst,
                1e-6,
            );
        }
    }
    ck.finish();
}

// ------------------------------------------------------------------------------------------------------- plate

/// The cantilever plate of D5.
fn plate() -> SolidModel {
    let mesh = hex_box([0.0; 3], [1.0, 0.1, 0.05], [10, 1, 1]).unwrap();
    let mesh = upgrade_to_hex20(&mesh).unwrap().0;
    let mut m = SolidModel::new(mesh);
    m.add_material("all", material()).unwrap();
    for c in 0..3 {
        m.add_constraint(Constraint::displacement("xmin", c, 0.0)).unwrap();
    }
    m
}

/// `p = E t^3/(2 L^3)` of D5.
fn p_plate() -> f64 {
    E * 0.05f64.powi(3) / 2.0
}

#[test]
fn u1_u2_u3_plate_unsymmetric_factor_rate() {
    let mut ck = Checks::new();
    let model = plate();
    let fp = FollowerPressure::new(&model, "zmax", p_plate()).unwrap();
    let run = |o: NewtonOptions| {
        let ext = DriverExtensions {
            contributions: vec![&fp],
            step_control: None,
        };
        solve_with(&model, &[], &[], &o, ext)
    };
    let u1 = run(NewtonOptions::new(5).unsymmetric(true));
    let u2 = run(NewtonOptions::new(5));
    match (&u1, &u2) {
        (Ok(a), Ok(b)) => {
            for (name, r) in [("U1 unsymmetric", a), ("U2 symmetric", b)] {
                for s in &r.steps {
                    println!(
                        "{name} lambda {:.3} halvings {}: {}",
                        s.load_factor,
                        s.halvings,
                        fmt_residuals(&s.residuals)
                    );
                }
            }
            ck.at_most(
                "U1 Q unsymmetric",
                quadratic_constant(a.steps.iter().map(|s| &s.residuals)),
                1e4,
            );
            let its = a.steps.iter().map(|s| s.residuals.len() - 1).max().unwrap();
            ck.at_most("U1i iterations per step (largest)", its as f64, 7.0);
            ck.at_least(
                "U2 Q symmetric (control)",
                quadratic_constant(b.steps.iter().map(|s| &s.residuals)),
                1e5,
            );
            ck.at_most(
                "U3 final states of U1 and U2",
                rel_diff(a.result.displacement_values(), b.result.displacement_values()),
                1e-6,
            );
        }
        _ => ck.holds(
            "U1, U2 runs",
            false,
            &format!("{:?} {:?}", u1.as_ref().err(), u2.as_ref().err()),
        ),
    }
    ck.finish();
}

#[test]
fn u4_u5_plate_by_arc_length_with_both_options() {
    let mut ck = Checks::new();
    let model = plate();
    let p = p_plate();
    let fp = FollowerPressure::new(&model, "zmax", p).unwrap();
    let contributions: Vec<&dyn Contribution> = vec![&fp];
    let c = load_scale(&model, "zmax", p);
    let options = ArcLengthOptions::new(0.05 * (2.0 * c).sqrt())
        .load_scale(c)
        .max_points(200)
        .contribution_load(true)
        .newton(NewtonOptions::new(1).unsymmetric(true));
    let path = match trace(&model, &[], &[], &options, &contributions, &mut |q: &PathPoint| {
        q.load_factor >= 1.0
    }) {
        Ok(p) => p,
        Err(e) => {
            ck.holds("U4 trace", false, &format!("error {e}"));
            ck.finish();
            return;
        }
    };
    println!("U4: {} points, load scale {:e}", path.points.len(), path.load_scale);
    for (i, q) in path.points.iter().enumerate() {
        println!(
            "  point {:3}: lambda {:.6} halvings {} residuals {}",
            i + 1,
            q.load_factor,
            q.halvings,
            fmt_residuals(&q.residuals)
        );
    }
    ck.holds(
        "U4 stopped at lambda >= 1 within 200 points",
        path.stopped && path.points.len() <= 200,
        &format!("stopped {}, {} points", path.stopped, path.points.len()),
    );
    println!(
        "U4r Q of the points (record): {:e}",
        quadratic_constant(path.points.iter().map(|q| &q.residuals))
    );
    ck.at_most("U5 constraint", constraint_error(&model, &path), 1e-10);
    let targets: Vec<f64> = path.points.iter().map(|q| q.load_factor).filter(|&l| l < 1.0).collect();
    let rec = Recorder::new(&fp);
    let mut hook = Hits { targets };
    let ext = DriverExtensions {
        contributions: vec![&rec],
        step_control: Some(&mut hook),
    };
    match solve_with(&model, &[], &[], &NewtonOptions::new(5).unsymmetric(true), ext) {
        Err(e) => ck.holds("U4 load-controlled run", false, &format!("error {e}")),
        Ok(r) => {
            println!("U4: load-controlled run, {} increments", r.steps.len());
            let mut worst: f64 = 0.0;
            for q in path.points.iter().filter(|q| q.load_factor < 1.0) {
                match rec.at(q.load_factor) {
                    Some(u) => {
                        let d = rel_diff(&q.displacement, &u);
                        println!("  lambda {:.6}: {d:.3e}", q.load_factor);
                        worst = worst.max(d);
                    }
                    None => worst = f64::INFINITY,
                }
            }
            ck.at_most(
                "U4 arc-length against load control at the common load factors",
                worst,
                1e-6,
            );
        }
    }
    ck.finish();
}

#[test]
fn c2_refusals_of_the_unsymmetric_option() {
    let mut ck = Checks::new();
    let mut model = plate();
    model.add_load(Load::pressure("zmax", 1e3)).unwrap();
    let e = solve(
        &model,
        &[],
        &[],
        &NewtonOptions::new(1)
            .unsymmetric(true)
            .factor(DirectFactor::SupernodalNestedDissection),
    );
    ck.holds(
        "C2a unsymmetric with the supernodal factor refused (key solver)",
        matches!(&e, Err(FemError::InvalidValue { key: "solver", reason }) if reason.contains("ADR 0560")),
        &format!("{:?}", e.as_ref().err()),
    );
    let o = ContactOptions::new(1, 1e-6)
        .kinematics(Kinematics::Linear)
        .newton(NewtonOptions::new(1).unsymmetric(true));
    let e = contact_solve(&model, &[], &[], &[], &o);
    ck.holds(
        "C2b the linear contact body with newton.unsymmetric refused (key solver)",
        matches!(&e, Err(FemError::InvalidValue { key: "solver", reason }) if reason.contains("ADR 0560")),
        &format!("{:?}", e.as_ref().err()),
    );
    ck.finish();
}
