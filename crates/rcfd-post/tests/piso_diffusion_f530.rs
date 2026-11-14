//! F-530 (ADR 0451): the divergence of the non-iterated PISO step above a diffusion number `nu dt/dy^2` of about 3.5 found by
//! ADR 0446 (a plane channel driven by an oscillating inflow on a fixed mesh): located as the corrector instability of ADR 0214
//! (H-RC: the first corrector over-corrects by about the cross-gap diffusion number, the collocated cell update leaves the
//! Rhie-Chow part of short pressure waves for the second to amplify), and removed by the consistent correctors of ADR 0214 F0
//! (`PimpleSettings::consistent_correctors`, opt-in): the same channel at `D = 25` and `D = 100` bounded over 3 periods and
//! matching the oscillating-channel impedance as the stable runs do.
//!
//! The limits, set-ups and mutations were registered in ADR 0451 before this file existed. The channel, the impedance formula
//! and the piston run are those of `sa_aerodamp.rs` (ADR 0446; copied, that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::FixedValue;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{FlowWall, Fluid, InterfaceMap};
use rcfd_fv::pimple::{LagAction, LagCheck, PimpleSettings, PimpleSolver};
use rcfd_fv::sixdof::patch_face_forces;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::Mesh;
use rcfd_post::solid_aerodamp::{drive_mode, periodic_response, ModeMotion};
use rcfd_post::solid_fsi::ChannelMotion;
use std::f64::consts::PI;
use std::sync::Arc;

const TOL_PERIODIC: f64 = 1e-6;
const H: f64 = 0.02;
const RHO: f64 = 1000.0;
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

// ---- complex arithmetic, the formula and the discrete model (ADR 0446 D3, ADR 0451 D2) -------------------------------------

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
    fn abs(self) -> f64 {
        self.0.hypot(self.1)
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

// ---- the fixed-mesh channel (ADR 0451 FC, FQ) -----------------------------------------------------------------------------

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

/// ADR 0446's `settings(nu)`, with the consistent correctors when `consistent`.
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

fn diff(a: &[Vec3], b: &[Vec3]) -> Vec<f64> {
    a.iter()
        .zip(b)
        .flat_map(|(x, y)| [x[0] - y[0], x[1] - y[1], x[2] - y[2]])
        .collect()
}

/// What a run of the fixed-mesh channel returns.
struct Run {
    /// Bounded in the sense of ADR 0451 (no trip, every velocity finite, `max |u| <= 10 V0` after every step).
    bounded: bool,
    /// The first step that broke it, with the reason.
    broke: Option<(usize, String)>,
    /// `Q_n` after every step.
    q: Vec<f64>,
    /// `max |u|` after every step.
    umax: Vec<f64>,
    /// `e_n = |u_n - u_{n-1}|_2` (index `n - 1`).
    e: Vec<f64>,
    /// `<u_n - u_{n-1}, u_{n-1} - u_{n-2}>` (index `n - 1`; 0 for `n = 1`).
    dot: Vec<f64>,
    seconds: f64,
}

fn channel_run(ch: &Channel, n: usize, cycles: usize, consistent: bool) -> Run {
    let t0 = std::time::Instant::now();
    let dt = 1.0 / n as f64;
    let mut fl = solver(ch.mesh(), settings(consistent));
    let mut prev = fl.u.values.clone();
    let mut prev_d: Option<Vec<f64>> = None;
    let mut run = Run {
        bounded: true,
        broke: None,
        q: Vec::new(),
        umax: Vec::new(),
        e: Vec::new(),
        dot: Vec::new(),
        seconds: 0.0,
    };
    for k in 1..=cycles * n {
        step(&mut fl, k, dt);
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
        if let (Some(r), true) = (reason, run.bounded) {
            run.bounded = false;
            run.broke = Some((k, r));
        }
        let d = diff(&fl.u.values, &prev);
        run.e.push(d.iter().map(|x| x * x).sum::<f64>().sqrt());
        run.dot.push(
            prev_d
                .as_ref()
                .map_or(0.0, |p| p.iter().zip(&d).map(|(a, b)| a * b).sum()),
        );
        prev_d = Some(d);
        prev = fl.u.values.clone();
        run.umax.push(um);
        run.q.push(if fl.diverged.is_some() {
            f64::NAN
        } else {
            inflow_force(&fl)
        });
        if fl.diverged.is_some() {
            break;
        }
    }
    run.seconds = t0.elapsed().as_secs_f64();
    run
}

/// The B4 estimate of the pressure-lag amplification at step 1 from rest (ADR 0073), `abs(rho)`.
fn b4(ch: &Channel, n: usize, mut s: PimpleSettings) -> f64 {
    s.pressure_lag_check = Some(LagCheck::new(0, LagAction::Warn));
    let mut fl = solver(ch.mesh(), s);
    step(&mut fl, 1, 1.0 / n as f64);
    fl.lag_estimates.first().map_or(f64::NAN, |w| w.rho.abs())
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

fn order(a: f64, b: f64, cc: f64) -> f64 {
    ((a - b).abs() / (b - cc).abs()).log2()
}

// ---- QF: the fast test ----------------------------------------------------------------------------------------------------

#[test]
fn qf_fast_channel_default_diverges_consistent_bounded() {
    let mut c = Checks::new();
    let n = 16;
    println!("FQ: D {:.4}", FQ.diffusion_number(n));
    let d = channel_run(&FQ, n, 2, false);
    println!("QF1 default: broke {:?}", d.broke);
    c.holds("QF1 default not bounded within 2 periods", !d.bounded, "not bounded");
    c.at_least("QF1 B4 default abs(rho)", b4(&FQ, n, settings(false)), 1.0);
    c.below("QF2 B4 consistent abs(rho)", b4(&FQ, n, settings(true)), 1.0);
    let r = channel_run(&FQ, n, 2, true);
    println!(
        "QF2 consistent: broke {:?}, max |u|/V0 {:.4}",
        r.broke,
        r.umax.iter().fold(0.0f64, |a, &b| a.max(b)) / v0()
    );
    c.holds("QF2 consistent bounded", r.bounded, "bounded");
    if let Some(z) = impedance(&mut c, "QF2", &r, n) {
        let m = model_zdt(n, FQ.ny, FQ.lf);
        let (re, im) = rel_parts(z, m);
        println!("QF2: Z {z:?}, model {m:?}");
        c.at_most("QF2 real part against Z_dt", re.abs(), 5e-2);
        c.at_most("QF2 imaginary part against Z_dt", im.abs(), 5e-2);
    }
    c.finish();
}

// ---- DG: the diagnosis (records, run once) ---------------------------------------------------------------------------------

#[test]
#[ignore]
fn dg_diagnosis_record() {
    let mut c = Checks::new();
    for n in [100usize, 25] {
        let r = channel_run(&FC, n, 3, false);
        println!(
            "DG0 N {n} (D {:.3}): broke {:?} ({:.1} s)",
            FC.diffusion_number(n),
            r.broke,
            r.seconds
        );
        c.holds(&format!("DG0 N {n} not bounded"), !r.bounded, "not bounded");
        if n == 100 {
            let rho100 = b4(&FC, 100, settings(false));
            c.at_least("DG1 B4 default N 100 abs(rho)", rho100, 3.5);
            c.at_most("DG1 B4 default N 100 abs(rho) (upper)", rho100, 6.5);
            // DG3: the growth of the run against the estimate
            match r.umax.iter().position(|&u| u >= 100.0 * v0()) {
                Some(m) if m + 5 < r.e.len() => {
                    let ratios: Vec<f64> = (m + 1..=m + 5).map(|i| r.e[i] / r.e[i - 1]).collect();
                    let gm = (ratios.iter().map(|x| x.ln()).sum::<f64>() / 5.0).exp();
                    let dots: Vec<f64> = (m + 1..=m + 5).map(|i| r.dot[i]).collect();
                    println!(
                        "DG3: first step >= 100 V0 is {}, ratios {ratios:?}, dots {dots:?}",
                        m + 1
                    );
                    c.at_most("DG3 growth against B4 (relative)", (gm / rho100 - 1.0).abs(), 0.25);
                    c.holds(
                        "DG3 alternating",
                        dots.iter().all(|&x| x < 0.0),
                        "dot < 0 at each of the 5 steps",
                    );
                }
                other => c.holds(
                    "DG3",
                    false,
                    &format!("no 5 steps after 100 V0 ({other:?}, {} steps)", r.e.len()),
                ),
            }
        }
    }
    c.below(
        "DG2 B4 default N 1000 (D 2.51) abs(rho)",
        b4(&FC, 1000, settings(false)),
        1.0,
    );
    c.at_least(
        "DG2 B4 default N 500 (D 5.03) abs(rho)",
        b4(&FC, 500, settings(false)),
        1.0,
    );
    for n in [100usize, 25] {
        c.below(
            &format!("DG4 B4 consistent N {n} abs(rho)"),
            b4(&FC, n, settings(true)),
            1.0,
        );
    }
    // printed records, no limit
    for nc in [3usize, 4] {
        let mut s = settings(false);
        s.n_correctors = nc;
        println!("record: B4 default N 100, {nc} correctors: {:.5}", b4(&FC, 100, s));
    }
    let mut s = settings(false);
    s.ddt_correction = false;
    println!(
        "record: B4 default N 100, no Rhie-Chow time term: {:.5}",
        b4(&FC, 100, s)
    );
    for n in [838usize, 718, 628] {
        println!(
            "record: B4 default N {n} (D {:.3}): {:.5}",
            FC.diffusion_number(n),
            b4(&FC, n, settings(false))
        );
    }
    c.finish();
}

// ---- A: the removal and the stable runs (record, run once; heavy) ------------------------------------------------------------

#[test]
#[ignore]
fn a_removal_and_stable_reference_record() {
    let mut c = Checks::new();
    let zf = impedance_formula(FC.lf);
    println!("FC: formula Z {zf:?}");
    let mut z = std::collections::BTreeMap::new();
    for n in [25usize, 50, 100] {
        let r = channel_run(&FC, n, 3, true);
        println!(
            "A1 N {n} (D {:.3}): broke {:?}, max |u|/V0 {:.4} ({:.1} s)",
            FC.diffusion_number(n),
            r.broke,
            r.umax.iter().fold(0.0f64, |a, &b| a.max(b)) / v0(),
            r.seconds
        );
        c.holds(&format!("A1 N {n} bounded"), r.bounded, "bounded");
        if let Some(zz) = impedance(&mut c, &format!("A2 N {n}"), &r, n) {
            let m = model_zdt(n, FC.ny, FC.lf);
            let (re, im) = rel_parts(zz, m);
            println!("A3 N {n}: Z {zz:?}, model {m:?}");
            c.at_most(&format!("A3 N {n} real part against Z_dt"), re.abs(), 3e-2);
            c.at_most(&format!("A3 N {n} imaginary part against Z_dt"), im.abs(), 3e-2);
            if n == 25 {
                let (fr, fi) = rel_parts(zz, zf);
                println!("NC1 N 25 against the formula: real {fr:+.3e}, imaginary {fi:+.3e}");
                c.holds("NC1 (must miss 3e-2)", fr.abs() > 3e-2 || fi.abs() > 3e-2, "misses");
            }
            z.insert(n, zz);
        }
        if n == 100 {
            // NC2: no alternation over 5 successive steps in the last period
            let last = &r.dot[2 * n..];
            let mut longest = 0;
            let mut run = 0;
            for &d in last {
                run = if d < 0.0 { run + 1 } else { 0 };
                longest = longest.max(run);
            }
            println!("NC2: longest run of negative dots in the last period {longest}");
            c.holds("NC2 (no 5 alternating steps)", longest < 5, "no alternation");
        }
    }
    let rc = match (z.get(&25), z.get(&50), z.get(&100)) {
        (Some(&z1), Some(&z2), Some(&z4)) => {
            let rc = z4.scale(2.0).sub(z2);
            let (re, im) = rel_parts(rc, zf);
            println!("A4: R_c {rc:?}: real {re:+.4e}, imaginary {im:+.4e}");
            c.at_most("A4 R_c real part against the formula", re.abs(), 3e-2);
            c.at_most("A4 R_c imaginary part against the formula", im.abs(), 3e-2);
            let (p_re, p_im) = (order(z1.0, z2.0, z4.0), order(z1.1, z2.1, z4.1));
            for (id, p) in [("O real", p_re), ("O imaginary", p_im)] {
                c.at_least(id, p, 0.8);
                c.at_most(&format!("{id} (upper)"), p, 1.25);
            }
            Some(rc)
        }
        _ => {
            c.holds("A4", false, "a level refused");
            None
        }
    };
    let mut zd = Vec::new();
    for n in [1000usize, 2000] {
        let r = channel_run(&FC, n, 3, false);
        println!(
            "A5 default N {n} (D {:.3}): broke {:?} ({:.1} s)",
            FC.diffusion_number(n),
            r.broke,
            r.seconds
        );
        c.holds(&format!("A5 default N {n} bounded"), r.bounded, "bounded");
        let zz = impedance(&mut c, &format!("A5 N {n}"), &r, n);
        println!("A5 N {n}: Z {zz:?}");
        zd.push(zz);
    }
    if let [Some(z1), Some(z2)] = zd[..] {
        let rd = z2.scale(2.0).sub(z1);
        let (re, im) = rel_parts(rd, zf);
        println!("A5: R_d {rd:?}: real {re:+.4e}, imaginary {im:+.4e}");
        c.at_most("A5 R_d real part against the formula", re.abs(), 3e-2);
        c.at_most("A5 R_d imaginary part against the formula", im.abs(), 3e-2);
        if let Some(rc) = rc {
            c.at_most("A6 |R_c - R_d|/|R_d|", rc.sub(rd).abs() / rd.abs(), 1e-2);
        }
    } else {
        c.holds("A5/A6", false, "a stable level refused");
    }
    c.finish();
}

// ---- AP: the piston of ADR 0446 with the consistent correctors (record) -------------------------------------------------------

fn piston_run(n: usize) -> Result<Cx, String> {
    let mesh0 = FC.mesh();
    let motion = ChannelMotion::new(&mesh0, INFLOW, 0, FC.lf, 1e-9)?;
    let mut fluid = FlowWall::new(
        solver(mesh0.clone(), settings(true)),
        INFLOW,
        &mesh0,
        RHO,
        nu(),
        motion.into_motion(),
    )?;
    let np = fluid.point_coordinates().len();
    let rows = (0..np).map(|i| vec![(i, 1.0)]).collect();
    let map = InterfaceMap::new(rows, np, fluid.face_points())?;
    let motion = ModeMotion {
        shape: vec![[1.0, 0.0, 0.0]; np],
        amplitude: q0(),
        omega: OMEGA,
        steps_per_cycle: n,
        cycles: 3,
    };
    let h = drive_mode(&mut fluid, &map, &motion)?;
    let r = h.response(TOL_PERIODIC)?;
    println!("AP N {n}: cycle change {:e}", r.cycle_change);
    let (re, im) = r.impedance();
    Ok(Cx(re, im))
}

#[test]
#[ignore]
fn ap_piston_consistent_record() {
    let mut c = Checks::new();
    let zf = impedance_formula(FC.lf);
    let mut z = Vec::new();
    for n in [50usize, 100] {
        let t0 = std::time::Instant::now();
        let r = piston_run(n);
        println!(
            "AP N {n} (D {:.3}): {r:?} ({:.1} s)",
            FC.diffusion_number(n),
            t0.elapsed().as_secs_f64()
        );
        c.holds(&format!("AP N {n} bounded and periodic"), r.is_ok(), &format!("{r:?}"));
        z.push(r.ok());
    }
    if let [Some(z2), Some(z4)] = z[..] {
        let ext = z4.scale(2.0).sub(z2);
        let (re, im) = rel_parts(ext, zf);
        println!("AP: 2 Z_100 - Z_50 = {ext:?}: real {re:+.4e}, imaginary {im:+.4e}");
        c.at_most("AP real part against the formula", re.abs(), 3e-2);
        c.at_most("AP imaginary part against the formula", im.abs(), 3e-2);
    }
    c.finish();
}

/// The discrete model reproduces the registration's arithmetic (ADR 0451 D2), a check of the test's own model.
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
