//! F-536 (ADR 0458): the number of outer iterations of the transient step with the consistent correctors set by the registered
//! criterion `PimpleSettings::outer_pressure_tolerance` (ADR 0458 C1: the next outer iteration is the final one once the
//! pressure change of an iteration is at most `tol` of that of the step's first one; `n_outer` the cap) instead of a fixed
//! count, on the fixed-mesh channel of ADR 0451 at `nu dt/dy^2 = 25, 50, 100`, with the outer iterations of every step recorded.
//!
//! The limits, set-ups and mutations were registered in ADR 0458 before this file existed. The channel, the model, the formula
//! and the cycle rule are those of `piso_diffusion_f530.rs` (ADR 0451; copied, that file is not edited).

use rcfd_core::vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{LagAction, LagCheck, OuterRecord, PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::patch_face_forces;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::solid_aerodamp::periodic_response;
use std::f64::consts::PI;
use std::sync::Arc;

const TOL_PERIODIC: f64 = 1e-6;
const H: f64 = 0.02;
const RHO: f64 = 1000.0;
const OMEGA: f64 = 2.0 * PI;
const WO: f64 = 1.0;
const INFLOW: &str = "xmin";
/// ADR 0458: the cap and the tolerance of the settings K.
const CAP: usize = 50;
const TOL_OUTER: f64 = 1e-3;

fn nu() -> f64 {
    OMEGA * (H / 2.0).powi(2) / (WO * WO)
}

fn q0() -> f64 {
    1e-3 * H
}

fn v0() -> f64 {
    q0() * OMEGA
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

// ---- complex arithmetic, the formula and the discrete model (ADR 0446 D3, ADR 0451 D2; copied) ------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Cx(f64, f64);

impl Cx {
    fn add(self, o: Cx) -> Cx {
        Cx(self.0 + o.0, self.1 + o.1)
    }
    fn sub(self, o: Cx) -> Cx {
        Cx(self.0 - o.0, self.1 - o.1)
    }
    fn scale(self, a: f64) -> Cx {
        Cx(a * self.0, a * self.1)
    }
    fn mul(self, o: Cx) -> Cx {
        Cx(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn div(self, o: Cx) -> Cx {
        let d = o.0 * o.0 + o.1 * o.1;
        Cx((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn exp(self) -> Cx {
        let r = self.0.exp();
        Cx(r * self.1.cos(), r * self.1.sin())
    }
}

/// `A i w rho L_f/(1 - tanh(kappa)/kappa)`, `kappa = (h/2) sqrt(i w/nu)`, `A = h^2` (ADR 0446 D3).
fn impedance_formula(lf: f64) -> Cx {
    let r = (H / 2.0) * (OMEGA / nu()).sqrt();
    let kappa = Cx(r * (PI / 4.0).cos(), r * (PI / 4.0).sin());
    let e = Cx(-2.0 * kappa.0, -2.0 * kappa.1).exp();
    let tanh = Cx(1.0 - e.0, -e.1).div(Cx(1.0 + e.0, e.1));
    let t = tanh.div(kappa);
    Cx(0.0, H * H * OMEGA * RHO * lf).div(Cx(1.0 - t.0, -t.1))
}

/// The discrete impedance `Z_dt = A rho L_f G(s) s/(i w)` of implicit Euler on the cell-centred profile of `ny` cells across
/// the gap (half-cell wall coefficient), `s = (1 - e^{-i w dt})/dt`, `dt = T/n`, `G` such that the mean of `u` is one
/// (ADR 0451 D2). The tridiagonal system `(s I - nu L_y) x = 1` by the Thomas algorithm.
fn model_zdt(n: usize, ny: usize, lf: f64) -> Cx {
    let dt = 2.0 * PI / (OMEGA * n as f64);
    let s = Cx(1.0, 0.0).sub(Cx(0.0, -OMEGA * dt).exp()).scale(1.0 / dt);
    let k = nu() / (H / ny as f64).powi(2);
    // links: 1 to each neighbour, 2 to a wall (half cell)
    let diag = |i: usize| {
        let left = if i == 0 { 2.0 } else { 1.0 };
        let right = if i == ny - 1 { 2.0 } else { 1.0 };
        s.add(Cx(k * (left + right), 0.0))
    };
    let off = Cx(-k, 0.0);
    let mut c = vec![Cx(0.0, 0.0); ny];
    let mut d = vec![Cx(0.0, 0.0); ny];
    for i in 0..ny {
        let (b, r) = if i == 0 {
            (diag(0), Cx(1.0, 0.0))
        } else {
            (diag(i).sub(off.mul(c[i - 1])), Cx(1.0, 0.0).sub(off.mul(d[i - 1])))
        };
        c[i] = off.div(b);
        d[i] = r.div(b);
    }
    let mut x = vec![Cx(0.0, 0.0); ny];
    x[ny - 1] = d[ny - 1];
    for i in (0..ny - 1).rev() {
        x[i] = d[i].sub(c[i].mul(x[i + 1]));
    }
    let mean = x.iter().fold(Cx(0.0, 0.0), |a, &v| a.add(v)).scale(1.0 / ny as f64);
    let g = Cx(1.0, 0.0).div(mean);
    g.mul(s).div(Cx(0.0, OMEGA)).scale(H * H * RHO * lf)
}

fn rel_parts(z: Cx, reference: Cx) -> (f64, f64) {
    (z.0 / reference.0 - 1.0, z.1 / reference.1 - 1.0)
}

// ---- the fixed-mesh channel (ADR 0451 FC, FQ; copied) -------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Channel {
    nx: usize,
    ny: usize,
    lf: f64,
    expansion: f64,
}

const FC: Channel = Channel {
    nx: 120,
    ny: 40,
    lf: 100.0 * H,
    expansion: 21.0,
};

const FQ: Channel = Channel {
    nx: 30,
    ny: 16,
    lf: 20.0 * H,
    expansion: 5.5,
};

impl Channel {
    fn mesh(&self) -> Mesh {
        box_mesh(
            [self.nx, self.ny, 1],
            [self.lf, H, H],
            [Grading::Expansion(self.expansion), Grading::Uniform, Grading::Uniform],
        )
        .unwrap()
    }
    fn diffusion_number(&self, n: usize) -> f64 {
        nu() * (1.0 / n as f64) / (H / self.ny as f64).powi(2)
    }
}

/// ADR 0451's `settings(true)`: `piso`, central convection, tight solvers, the consistent correctors (2 correctors, no
/// relaxation).
fn settings() -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu());
    s.convection = ConvScheme::Central;
    for c in [&mut s.momentum_solver, &mut s.pressure_solver] {
        c.tol_abs = 1e-16;
        c.tol_rel = 1e-13;
        c.max_iter = 5000;
    }
    s.consistent_correctors = true;
    s
}

/// ADR 0451's `outer_settings()` (the fixed 8), used for the cycle rule only (ADR 0458 "Periods").
fn outer_settings() -> PimpleSettings {
    let mut s = settings();
    s.n_outer = 8;
    s
}

/// The settings K of ADR 0458: the cap 50 and the criterion `tol`.
fn k_settings(tol: f64) -> PimpleSettings {
    let mut s = settings();
    s.n_outer = CAP;
    s.outer_pressure_tolerance = Some(tol);
    s
}

/// Fluid at rest on the fixed mesh: no-slip `ymin`, `ymax`; `p = 0` at `xmax`; `z` planes and the rest at the defaults.
fn solver(mesh: Mesh, s: PimpleSettings) -> PimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let mut pr = ScalarField::uniform("p", &geom, 0.0);
    pr.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, pr, s)
}

/// The inflow of step `n` (1-based): `(q_n - q_{n-1})/dt`, `q_n = q0 sin(w n dt)`.
fn inflow(n: usize, dt: f64) -> f64 {
    let q = |m: usize| q0() * (OMEGA * m as f64 * dt).sin();
    (q(n) - q(n - 1)) / dt
}

/// One step of the fixed-mesh channel with the inflow of step `n`.
fn step(fl: &mut PimpleSolver, n: usize, dt: f64) {
    let g = fl.geom.clone();
    fl.u.set_bc(&g, INFLOW, Box::new(FixedValue::vector([inflow(n, dt), 0.0, 0.0])))
        .unwrap();
    fl.step(dt);
}

/// The `x` load of the flow on the inflow plane.
fn inflow_force(fl: &PimpleSolver) -> f64 {
    patch_face_forces(&fl.geom, &fl.u, &fl.p, RHO, nu(), &[INFLOW.to_string()], None)
        .unwrap()
        .iter()
        .map(|f| f[0])
        .sum()
}

fn max_speed(fl: &PimpleSolver) -> f64 {
    fl.u.values
        .iter()
        .map(|v| vec3::norm(*v))
        .fold(0.0, |a: f64, b| if b.is_nan() { f64::NAN } else { a.max(b) })
}

/// The B4 estimate of the pressure-lag amplification at step 1 from rest (ADR 0073), `abs(rho)`.
fn b4(ch: &Channel, n: usize, mut s: PimpleSettings) -> f64 {
    s.pressure_lag_check = Some(LagCheck::new(0, LagAction::Warn));
    let mut fl = solver(ch.mesh(), s);
    step(&mut fl, 1, 1.0 / n as f64);
    fl.lag_estimates.first().map_or(f64::NAN, |w| w.rho.abs())
}

/// The cycle rule of ADR 0451 amendment 1: `C = max(3, 2 + ceil(ln(1e-7)/(N ln rho_c)))`, `rho_c` the B4 estimate of the settings
/// at `N`; refused if `rho_c >= 1` or `C > 80`.
fn cycles_for(ch: &Channel, n: usize, s: &PimpleSettings) -> Result<(usize, f64), String> {
    let rho = b4(ch, n, s.clone());
    if rho.is_nan() || rho >= 1.0 {
        return Err(format!("B4 estimate {rho} >= 1"));
    }
    let c = (2.0 + (1e-7f64.ln() / (n as f64 * rho.ln())).ceil()).max(3.0) as usize;
    if c > 80 {
        return Err(format!("{c} periods (> 80) for B4 {rho}"));
    }
    Ok((c, rho))
}

/// What a run of the channel returns.
struct Run {
    /// The first step that broke boundedness (ADR 0451's sense), with the reason.
    broke: Option<(usize, String)>,
    /// `Q_n` after every completed step.
    q: Vec<f64>,
    /// `max |u|` over the run.
    umax: f64,
    /// `PimpleSolver::outer_record` at the end.
    record: Vec<OuterRecord>,
    steps: usize,
    seconds: f64,
}

fn channel_run(ch: &Channel, n: usize, cycles: usize, s: PimpleSettings) -> Run {
    let t0 = std::time::Instant::now();
    let dt = 1.0 / n as f64;
    let mut fl = solver(ch.mesh(), s);
    let mut run = Run {
        broke: None,
        q: Vec::new(),
        umax: 0.0,
        record: Vec::new(),
        steps: 0,
        seconds: 0.0,
    };
    for k in 1..=cycles * n {
        step(&mut fl, k, dt);
        run.steps = k;
        let um = max_speed(&fl);
        let reason = if let Some(d) = &fl.diverged {
            Some(format!("divergence guard: {}", d.message(None)))
        } else if !fl.u.values.iter().flatten().all(|x| x.is_finite()) {
            Some("non-finite velocity".to_string())
        } else if um > 10.0 * v0() {
            Some(format!("max |u| = {:.3e} V0", um / v0()))
        } else {
            None
        };
        run.umax = run.umax.max(um);
        if let (Some(r), None) = (reason, &run.broke) {
            run.broke = Some((k, r));
        }
        if fl.diverged.is_some() {
            break;
        }
        run.q.push(inflow_force(&fl));
    }
    run.record = fl.outer_record.clone();
    run.seconds = t0.elapsed().as_secs_f64();
    run
}

fn impedance(c: &mut Checks, id: &str, run: &Run, n: usize) -> Option<Cx> {
    match periodic_response(&run.q, n, q0(), OMEGA, TOL_PERIODIC) {
        Ok(r) => {
            c.at_most(&format!("{id} periodic (cycle change)"), r.cycle_change, TOL_PERIODIC);
            let (re, im) = r.impedance();
            Some(Cx(re, im))
        }
        Err(m) => {
            c.holds(&format!("{id} periodic"), false, &m);
            None
        }
    }
}

/// K1, K2 (and QFk's parts): one record per step, numbered, every step met before the cap; returns the mean count.
fn record_checks(c: &mut Checks, id: &str, run: &Run) -> f64 {
    let r = &run.record;
    c.holds(
        &format!("{id} one record per step"),
        r.len() == run.steps && r.iter().enumerate().all(|(i, e)| e.step == i + 1),
        &format!("{} records for {} steps", r.len(), run.steps),
    );
    let bad = r.iter().filter(|e| !(e.met && e.used < CAP)).count();
    c.holds(
        &format!("{id} every step met before the cap"),
        bad == 0,
        &format!("{bad} steps not met or at the cap"),
    );
    let total: usize = r.iter().map(|e| e.used).sum();
    let mean = total as f64 / r.len().max(1) as f64;
    let (lo, hi) = r.iter().fold((usize::MAX, 0), |(a, b), e| (a.min(e.used), b.max(e.used)));
    println!(
        "{id}: outer iterations per step mean {mean:.3}, min {lo}, max {hi}, total {total} (the fixed 8: {}), {:.1} s",
        8 * run.steps,
        run.seconds
    );
    mean
}

fn order(a: f64, b: f64, cc: f64) -> f64 {
    ((a - b).abs() / (b - cc).abs()).log2()
}

// ---- QFk and NC1: the fast tests ----------------------------------------------------------------------------------------------

#[test]
fn qfk_fast_channel_outer_criterion() {
    let mut c = Checks::new();
    let n = 16;
    match cycles_for(&FQ, n, &outer_settings()) {
        Ok((cy, rho)) => {
            println!("QFk: B4 (fixed 8) {rho:.5}, {cy} periods");
            let r = channel_run(&FQ, n, cy, k_settings(TOL_OUTER));
            c.holds("QFk bounded", r.broke.is_none(), &format!("bounded ({:?})", r.broke));
            record_checks(&mut c, "QFk", &r);
            if let Some(z) = impedance(&mut c, "QFk", &r, n) {
                let m = model_zdt(n, FQ.ny, FQ.lf);
                let (re, im) = rel_parts(z, m);
                println!("QFk: Z {z:?}, model {m:?}: real {re:+.4e}, imaginary {im:+.4e}");
                c.at_most("QFk real part against Z_dt", re.abs(), 5e-2);
                c.at_most("QFk imaginary part against Z_dt", im.abs(), 5e-2);
            }
        }
        Err(m) => c.holds("QFk cycle rule", false, &m),
    }
    c.finish();
}

#[test]
fn nc1_fast_channel_loose_tolerance_misses() {
    let mut c = Checks::new();
    let n = 16;
    match cycles_for(&FQ, n, &outer_settings()) {
        Ok((cy, _)) => {
            let r = channel_run(&FQ, n, cy, k_settings(0.5));
            record_checks(&mut Checks::new(), "NC1 (printed)", &r);
            match periodic_response(&r.q, n, q0(), OMEGA, TOL_PERIODIC) {
                Ok(p) => {
                    let (re, im) = p.impedance();
                    let (fr, fi) = rel_parts(Cx(re, im), model_zdt(n, FQ.ny, FQ.lf));
                    println!("NC1: real {fr:+.4e}, imaginary {fi:+.4e}");
                    c.holds("NC1 (must miss 5e-2)", fr.abs() > 5e-2 || fi.abs() > 5e-2, "misses");
                }
                Err(m) => println!("NC1: refused ({m}), counts as a miss"),
            }
        }
        Err(m) => c.holds("NC1 cycle rule", false, &m),
    }
    c.finish();
}

// ---- FCk: the channel at D = 100, 50, 25 (heavy record, run once) ---------------------------------------------------------------

#[test]
#[ignore]
fn fck_channel_outer_criterion_record() {
    let mut c = Checks::new();
    let zf = impedance_formula(FC.lf);
    let mut z = std::collections::BTreeMap::new();
    for n in [25usize, 50, 100] {
        match cycles_for(&FC, n, &outer_settings()) {
            Ok((cy, rho)) => {
                let r = channel_run(&FC, n, cy, k_settings(TOL_OUTER));
                println!(
                    "FCk N {n} (D {:.3}): B4 (fixed 8) {rho:.5}, {cy} periods, broke {:?}, max |u|/V0 {:.4}",
                    FC.diffusion_number(n),
                    r.broke,
                    r.umax / v0()
                );
                c.holds(&format!("A1k N {n} bounded"), r.broke.is_none(), "bounded");
                let mean = record_checks(&mut c, &format!("K1/K2 N {n}"), &r);
                if n != 25 {
                    c.at_least(&format!("K3 N {n} mean count"), mean, 5.0);
                    c.at_most(&format!("K3 N {n} mean count (upper)"), mean, 20.0);
                }
                if let Some(zz) = impedance(&mut c, &format!("A2k N {n}"), &r, n) {
                    let m = model_zdt(n, FC.ny, FC.lf);
                    let (re, im) = rel_parts(zz, m);
                    println!("A3k N {n}: Z {zz:?}, model {m:?}: real {re:+.4e}, imaginary {im:+.4e}");
                    c.at_most(&format!("A3k N {n} real part against Z_dt"), re.abs(), 1e-2);
                    c.at_most(&format!("A3k N {n} imaginary part against Z_dt"), im.abs(), 1e-2);
                    if n == 25 {
                        let (fr, fi) = rel_parts(zz, zf);
                        println!("NC2 N 25 against the formula: real {fr:+.3e}, imaginary {fi:+.3e}");
                        c.holds("NC2 (must miss 1e-2)", fr.abs() > 1e-2 || fi.abs() > 1e-2, "misses");
                    }
                    z.insert(n, zz);
                }
            }
            Err(m) => c.holds(&format!("FCk N {n} cycle rule"), false, &m),
        }
    }
    match (z.get(&25), z.get(&50), z.get(&100)) {
        (Some(&z1), Some(&z2), Some(&z4)) => {
            let rc = z4.scale(2.0).sub(z2);
            let (re, im) = rel_parts(rc, zf);
            println!("A4k: R_c {rc:?}: real {re:+.4e}, imaginary {im:+.4e}");
            c.at_most("A4k R_c real part against the formula", re.abs(), 3e-2);
            c.at_most("A4k R_c imaginary part against the formula", im.abs(), 3e-2);
            println!(
                "record: order real {:.3}, imaginary {:.3} (no limit)",
                order(z1.0, z2.0, z4.0),
                order(z1.1, z2.1, z4.1)
            );
        }
        _ => c.holds("A4k", false, "a level refused"),
    }
    c.finish();
}

/// The discrete model reproduces the registration's arithmetic of ADR 0451 D2 (copied), a check of the test's own model.
#[test]
fn model_matches_the_registered_arithmetic() {
    let zf = impedance_formula(FC.lf);
    let (re, im) = rel_parts(model_zdt(25, 40, FC.lf), zf);
    assert!(
        (re - 8.680e-2).abs() < 1e-4 && (im + 3.513e-1).abs() < 1e-4,
        "{re} {im}"
    );
    let rc = model_zdt(100, 40, FC.lf).scale(2.0).sub(model_zdt(50, 40, FC.lf));
    let (re, im) = rel_parts(rc, zf);
    assert!(
        (re - 2.588e-4).abs() < 1e-6 && (im - 3.481e-3).abs() < 1e-6,
        "{re} {im}"
    );
}
