//! F-535 (ADR 0471): the opt-in refusal of the first PISO step at the pressure-lag onset on the fixed-mesh channel FC of ADR 0451
//! (checks R1 to R12): refused at `nu dt/dy^2 = 25, 50, 100` (and 3.0, 5.03), run and bit-identical at 2.5, the remedies pass.
//! The limits were registered in ADR 0471 before this file existed. The channel, `settings`, `step` and `Checks` are those of
//! `piso_diffusion_f530.rs` (copied, that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{LagAction, LagCheck, PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::sync::Arc;

const H: f64 = 0.02;
const OMEGA: f64 = 2.0 * PI;
const WO: f64 = 1.0;
const INFLOW: &str = "xmin";

fn nu() -> f64 {
    OMEGA * (H / 2.0).powi(2) / (WO * WO)
}

fn q0() -> f64 {
    1e-3 * H
}

fn v0() -> f64 {
    q0() * OMEGA
}

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
    fn below(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value < limit;
        println!(
            "{id}: {value:e} (limit < {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} >= {limit:e}"));
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

fn mesh() -> Mesh {
    box_mesh(
        [120, 40, 1],
        [100.0 * H, H, H],
        [Grading::Expansion(21.0), Grading::Uniform, Grading::Uniform],
    )
    .unwrap()
}

/// `nu dt/dy^2` with `dt = T/n`, 40 cells across the gap.
fn dn(n: usize) -> f64 {
    nu() * (1.0 / n as f64) / (H / 40.0).powi(2)
}

fn settings(consistent: bool) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu());
    s.convection = ConvScheme::Central;
    for c in [&mut s.momentum_solver, &mut s.pressure_solver] {
        c.tol_abs = 1e-16;
        c.tol_rel = 1e-13;
        c.max_iter = 5000;
    }
    s.consistent_correctors = consistent;
    s
}

fn outer_settings() -> PimpleSettings {
    let mut s = settings(true);
    s.n_outer = 8;
    s
}

fn with_option(mut s: PimpleSettings) -> PimpleSettings {
    s.lag_onset_refusal = Some(20);
    s
}

fn solver(s: PimpleSettings) -> PimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh())).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let mut pr = ScalarField::uniform("p", &geom, 0.0);
    pr.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, pr, s)
}

fn inflow(n: usize, dt: f64) -> f64 {
    let q = |m: usize| q0() * (OMEGA * m as f64 * dt).sin();
    (q(n) - q(n - 1)) / dt
}

fn step(fl: &mut PimpleSolver, n: usize, dt: f64) {
    let g = fl.geom.clone();
    fl.u.set_bc(&g, INFLOW, Box::new(FixedValue::vector([inflow(n, dt), 0.0, 0.0])))
        .unwrap();
    fl.step(dt);
}

fn max_speed(fl: &PimpleSolver) -> f64 {
    fl.u.values
        .iter()
        .map(|v| vec3::norm(*v))
        .fold(0.0, |a: f64, b| if b.is_nan() { f64::NAN } else { a.max(b) })
}

fn same(a: &[Vec3], b: &[Vec3]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (0..3).all(|k| x[k].to_bits() == y[k].to_bits()))
}

fn same_s(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// The B4 estimate of step 1 taken without the option (`lag_estimates[0].rho`).
fn b4(n: usize) -> f64 {
    let mut s = settings(false);
    s.pressure_lag_check = Some(LagCheck::new(0, LagAction::Warn));
    let mut fl = solver(s);
    step(&mut fl, 1, 1.0 / n as f64);
    fl.lag_estimates[0].rho
}

/// R1, R3, R4, R5, R6 at one level.
fn refused_level(c: &mut Checks, n: usize) {
    let dt = 1.0 / n as f64;
    let t0 = std::time::Instant::now();
    let mut fl = solver(with_option(settings(false)));
    let (u0, p0, phi0) = (fl.u.values.clone(), fl.p.values.clone(), fl.phi.clone());
    step(&mut fl, 1, dt);
    let secs = t0.elapsed().as_secs_f64();
    let Some(d) = fl.diverged.clone() else {
        c.holds(&format!("R1 N={n} refused"), false, "diverged is None");
        return;
    };
    c.holds(
        &format!("R1 N={n} refused at step 1 as lag onset"),
        d.quantity == "lag onset" && d.stage == "pressure-lag onset" && d.step == 1,
        &format!("{} / {} / step {}", d.quantity, d.stage, d.step),
    );
    println!(
        "N={n}: D {:.3}, estimate {:.4}, refusal call {secs:.2} s",
        dn(n),
        d.value
    );
    c.at_least(&format!("R1 N={n} estimate"), d.value, 1.0);
    let r = b4(n);
    c.at_most(
        &format!("R3 N={n} against the Warn estimate"),
        (d.value / r - 1.0).abs(),
        1e-12,
    );
    c.at_most(
        &format!("R5 N={n} diffusion number"),
        (d.diffusion / dn(n) - 1.0).abs(),
        1e-9,
    );
    let msg = d.message(None);
    for needle in [
        format!("{:.3}", dn(n)),
        format!("{:.3}", d.value),
        "nu dt/dx^2".to_string(),
        "consistent_correctors".to_string(),
        "n_outer".to_string(),
        "solver.dt".to_string(),
        "step 1".to_string(),
        "state is as it was".to_string(),
    ] {
        c.holds(&format!("R6 N={n} message has {needle:?}"), msg.contains(&needle), &msg);
    }
    let unchanged = |fl: &PimpleSolver| {
        same(&fl.u.values, &u0)
            && same_s(&fl.p.values, &p0)
            && same_s(&fl.phi, &phi0)
            && fl.time == 0.0
            && fl.history.is_empty()
            && fl.lag_estimates.is_empty()
            && fl.lag_warnings.is_empty()
            && fl.outer_record.is_empty()
    };
    c.holds(
        &format!("R4 N={n} state as it was"),
        unchanged(&fl),
        "fields, time, records",
    );
    let first = d.info;
    for k in 2..5 {
        step(&mut fl, k, dt);
        let again = fl.step(dt);
        c.holds(
            &format!("R4 N={n} later call {k}"),
            again.step == first.step && unchanged(&fl),
            "same info, state unchanged",
        );
    }
}

#[test]
fn r1_r3_r6_fc_refused_at_25_50_100() {
    let mut c = Checks::new();
    for n in [100, 50, 25] {
        refused_level(&mut c, n);
    }
    c.finish();
}

#[test]
fn r2_fc_refused_above_the_onset() {
    let mut c = Checks::new();
    for n in [838, 500] {
        let mut fl = solver(with_option(settings(false)));
        step(&mut fl, 1, 1.0 / n as f64);
        match &fl.diverged {
            Some(d) => {
                c.holds(&format!("R2 N={n} refused"), d.quantity == "lag onset", d.quantity);
                c.at_least(&format!("R2 N={n} estimate (D {:.2})", dn(n)), d.value, 1.0);
            }
            None => c.holds(&format!("R2 N={n} refused"), false, "diverged is None"),
        }
    }
    c.finish();
}

#[test]
fn r7_r10_fc_runs_at_2_5_and_is_bit_identical() {
    let mut c = Checks::new();
    let n = 1000;
    let dt = 1.0 / n as f64;
    // R7: the estimate of the step's settings
    let mut probe = solver(with_option(settings(false)));
    let g = probe.geom.clone();
    probe
        .u
        .set_bc(&g, INFLOW, Box::new(FixedValue::vector([inflow(1, dt), 0.0, 0.0])))
        .unwrap();
    let rho = probe.lag_preflight(dt, 20).map_or(f64::NAN, |w| w.rho);
    c.below("R7 estimate at D = 2.51", rho, 1.0);
    // R8, R9: 20 steps with and without the option
    let (mut a, mut b) = (solver(with_option(settings(false))), solver(settings(false)));
    let mut all_equal = true;
    let mut worst = 0.0f64;
    for k in 1..=20 {
        step(&mut a, k, dt);
        step(&mut b, k, dt);
        worst = worst.max(max_speed(&a) / v0());
        all_equal &= same(&a.u.values, &b.u.values)
            && same_s(&a.p.values, &b.p.values)
            && same_s(&a.phi, &b.phi)
            && a.time.to_bits() == b.time.to_bits();
        if a.diverged.is_some() {
            break;
        }
    }
    c.holds("R8 no trip in 20 steps", a.diverged.is_none(), "diverged is None");
    c.at_most("R8 max |u| / V0", worst, 10.0);
    c.holds(
        "R9 bit for bit equal to the run without the option, every step",
        all_equal,
        "u, p, phi, time",
    );
    // R10: the first step only
    step(&mut a, 21, 1.0 / 25.0);
    c.holds(
        "R10 a later step with D = 100 is not refused by the option",
        a.diverged.as_ref().is_none_or(|d| d.quantity != "lag onset"),
        "not a lag onset trip",
    );
    c.finish();
}

#[test]
fn r11_r12_remedies_pass_and_the_default_is_unchanged() {
    let mut c = Checks::new();
    for n in [100, 50, 25] {
        let dt = 1.0 / n as f64;
        let mut fl = solver(with_option(outer_settings()));
        for k in 1..=4 {
            step(&mut fl, k, dt);
        }
        c.holds(
            &format!("R11 N={n} consistent correctors, 8 outer: not refused over 4 steps"),
            fl.diverged.is_none(),
            &fl.diverged.as_ref().map_or(String::new(), |d| d.message(None)),
        );
    }
    // R12: the option None takes the first step
    let mut fl = solver(settings(false));
    step(&mut fl, 1, 0.01);
    c.holds(
        "R12 no option: no trip at N = 100",
        fl.diverged.is_none(),
        "diverged is None",
    );
    c.holds(
        "R12 no option: the velocity moved",
        fl.u.values.iter().any(|v| v[0] != 0.0),
        "u differs from rest",
    );
    c.finish();
}
