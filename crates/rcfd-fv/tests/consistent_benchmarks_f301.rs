//! F-301 (ADR 0223) (c): the benchmark set of the consistent correctors, part 1 (rcfd-fv).
//!
//! * BO, BF: ADR 0069's temporal-order harness (`w33b_transient_order.rs`, copied, with the option as a configuration field) and
//!   ADR 0073's TG-S set-up, arms D (default) and C (`consistent_correctors`, `consistent_periodic`, `consistent_body_force`).
//! * BS: ADR 0069's cavity stability harness (`w33b_stability.rs`, copied): the required set in both arms, the scan in arm C.
//! * BB: ADR 0073 Part B (`w34b_lag.rs`, copied): the B2 periodic Stokes box and the B3 cavity runs in arm C.
//!
//! The deliverable checks (BOc, BSc with BSd, BBc) and the default-decision criteria (DC1 to DC5) are separate tests over one cached
//! computation per benchmark. Everything is heavy and ignored: `cargo test --release -p rcfd-fv --test consistent_benchmarks_f301 --
//! --ignored --nocapture`. The limits were registered in ADR 0223 before this file existed.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{LagAction, LagCheck, OuterHook, OuterUpdate, PimpleSettings, PimpleSolver};
use rcfd_fv::simple::BodyForce;
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_2d_graded, cavity_2d, Grading};
use std::f64::consts::PI;
use std::sync::{Arc, Mutex, OnceLock};

/// The option C of ADR 0223 on a settings value.
fn option_c(s: &mut PimpleSettings) {
    s.consistent_correctors = true;
    s.consistent_periodic = true;
    s.consistent_body_force = true;
}

/// Run `jobs` on three worker threads, results in the order of `jobs`.
fn parallel<T: Send, R: Send>(jobs: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let n = jobs.len();
    let queue = Mutex::new(jobs.into_iter().enumerate().collect::<Vec<_>>());
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|sc| {
        for _ in 0..3 {
            sc.spawn(|| loop {
                let job = queue.lock().unwrap().pop();
                let Some((i, j)) = job else { break };
                let r = f(j);
                out.lock().unwrap()[i] = Some(r);
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|r| r.unwrap()).collect()
}

// =================================================================================================================
// BO: ADR 0069's temporal-order harness (copied from w33b_transient_order.rs; TG-S of ADR 0073 added)
// =================================================================================================================

#[derive(Clone, Copy, PartialEq, Debug)]
enum Prob {
    Tg,
    TgS,
    Mode,
    Mms,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Path {
    Piso,
    PimpleC,
    Pimple4,
}

#[derive(Clone, Copy, Debug)]
struct Cfg {
    prob: Prob,
    path: Path,
    scheme: TimeScheme,
    consistent: bool,
}

const SHIFT: f64 = 0.6;

fn nu_of(p: Prob) -> f64 {
    match p {
        Prob::Tg | Prob::TgS | Prob::Mode => 0.1,
        Prob::Mms => 0.05,
    }
}

fn g_of(t: f64) -> f64 {
    80.0 * (1.0 + 0.5 * (2.0 * t + 0.3).sin())
}
fn gdot_of(t: f64) -> f64 {
    80.0 * (2.0 * t + 0.3).cos()
}
fn xs(x: f64) -> [f64; 4] {
    [
        x * x * (1.0 - x) * (1.0 - x),
        2.0 * x - 6.0 * x * x + 4.0 * x * x * x,
        2.0 - 12.0 * x + 12.0 * x * x,
        -12.0 + 24.0 * x,
    ]
}
fn p_amp(t: f64) -> f64 {
    0.5 * (g_of(t) / 80.0).powi(2)
}
fn mms_pressure(x: Vec3, t: f64) -> f64 {
    p_amp(t) * (PI * x[0]).cos() * (PI * x[1]).cos()
}
fn mms_grad_p(x: Vec3, t: f64) -> Vec3 {
    let a = p_amp(t);
    [
        -a * PI * (PI * x[0]).sin() * (PI * x[1]).cos(),
        -a * PI * (PI * x[0]).cos() * (PI * x[1]).sin(),
        0.0,
    ]
}
fn mms_velocity(x: Vec3, t: f64) -> Vec3 {
    let (a, b) = (xs(x[0]), xs(x[1]));
    let g = g_of(t);
    [g * a[0] * b[1], -g * a[1] * b[0], 0.0]
}
fn mms_source(x: Vec3, t: f64) -> Vec3 {
    let nu = nu_of(Prob::Mms);
    let (a, b) = (xs(x[0]), xs(x[1]));
    let (g, gd) = (g_of(t), gdot_of(t));
    let dp = mms_grad_p(x, t);
    let fx = gd * a[0] * b[1] + g * g * (a[0] * a[1] * b[1] * b[1] - a[0] * a[1] * b[0] * b[2])
        - nu * g * (a[2] * b[1] + a[0] * b[3])
        + dp[0];
    let fy = -gd * a[1] * b[0]
        + g * g * (-a[0] * a[2] * b[0] * b[1] + a[1] * a[1] * b[0] * b[1])
        + nu * g * (a[3] * b[0] + a[1] * b[2])
        + dp[1];
    [fx, fy, 0.0]
}

fn exact_u(p: Prob, x: Vec3, t: f64) -> Vec3 {
    match p {
        Prob::Tg => {
            let d = (-2.0 * nu_of(p) * t).exp();
            [x[0].sin() * x[1].cos() * d, -x[0].cos() * x[1].sin() * d, 0.0]
        }
        Prob::TgS => {
            let d = (-2.0 * nu_of(p) * t).exp();
            let (a, b) = (x[0] + SHIFT, x[1] + SHIFT);
            [a.sin() * b.cos() * d, -a.cos() * b.sin() * d, 0.0]
        }
        Prob::Mode => [0.0, (PI * x[0]).sin() * (-nu_of(p) * PI * PI * t).exp(), 0.0],
        Prob::Mms => mms_velocity(x, t),
    }
}

fn exact_p(p: Prob, x: Vec3, t: f64) -> f64 {
    match p {
        Prob::Tg => 0.25 * ((2.0 * x[0]).cos() + (2.0 * x[1]).cos()) * (-4.0 * nu_of(p) * t).exp(),
        Prob::TgS => 0.25 * ((2.0 * (x[0] + SHIFT)).cos() + (2.0 * (x[1] + SHIFT)).cos()) * (-4.0 * nu_of(p) * t).exp(),
        Prob::Mode => 0.0,
        Prob::Mms => mms_pressure(x, t),
    }
}

fn mms_force(geom: &FvGeometry, t: f64) -> BodyForce {
    let mesh = &geom.mesh;
    let (cc, fc, sf) = (mesh.cell_centres(), mesh.face_centres(), mesh.face_area_vectors());
    let cell: Vec<Vec3> = cc.iter().map(|&c| mms_source(c, t)).collect();
    let face: Vec<f64> = (0..mesh.n_faces())
        .map(|f| {
            if geom.empty[f] {
                0.0
            } else if f < geom.n_internal {
                vec3::dot(mms_source(fc[f], t), sf[f])
            } else {
                vec3::dot(mms_grad_p(fc[f], t), sf[f])
            }
        })
        .collect();
    BodyForce { cell, face }
}

fn settings(cfg: &Cfg) -> PimpleSettings {
    let mut s = PimpleSettings::piso(nu_of(cfg.prob));
    s.convection = ConvScheme::Central;
    s.time_scheme = cfg.scheme;
    match cfg.path {
        Path::Piso => {}
        Path::PimpleC => {
            s.n_outer = 2;
            s.alpha_u = 0.9;
            s.alpha_p = 0.9;
        }
        Path::Pimple4 => s.n_outer = 4,
    }
    s.momentum_solver = SolveControl {
        tol_abs: 1e-14,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    s.pressure_solver = SolveControl {
        tol_abs: 1e-14,
        tol_rel: 1e-12,
        max_iter: 5000,
    };
    if cfg.consistent {
        option_c(&mut s);
    }
    s
}

fn make(cfg: &Cfg, n: usize) -> PimpleSolver {
    let st = settings(cfg);
    let prob = cfg.prob;
    match prob {
        Prob::Tg | Prob::TgS => {
            let l = 2.0 * PI;
            let mesh = Arc::new(box_2d(n, n, l, l, true, true).unwrap());
            let geom = Arc::new(FvGeometry::new(mesh).unwrap());
            let u = VectorField::from_fn("U", &geom, |c| exact_u(prob, c, 0.0));
            let p = ScalarField::from_fn("p", &geom, |c| exact_p(prob, c, 0.0));
            PimpleSolver::new(geom, u, p, st)
        }
        Prob::Mode => {
            let mesh = Arc::new(box_2d(n, n, 1.0, 1.0, false, false).unwrap());
            let geom = Arc::new(FvGeometry::new(mesh).unwrap());
            let mut u = VectorField::from_fn("U", &geom, |c| exact_u(prob, c, 0.0));
            u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
            u.set_bc(&geom, "xmax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
            u.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
            u.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
            let mut p = ScalarField::uniform("p", &geom, 0.0);
            p.set_bc(&geom, "ymin", Box::new(FixedValue::scalar(0.0))).unwrap();
            p.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
            PimpleSolver::new(geom, u, p, st)
        }
        Prob::Mms => {
            let mesh = Arc::new(box_2d(n, n, 1.0, 1.0, false, false).unwrap());
            let geom = Arc::new(FvGeometry::new(mesh).unwrap());
            let mut u = VectorField::from_fn("U", &geom, |c| exact_u(prob, c, 0.0));
            for patch in ["xmin", "xmax", "ymin", "ymax"] {
                u.set_bc(&geom, patch, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
            }
            let p = ScalarField::from_fn("p", &geom, |c| exact_p(prob, c, 0.0));
            PimpleSolver::new(geom, u, p, st)
        }
    }
}

/// Run with the given step sequence; `Err` names a stop (a refusal or a guard trip).
fn run_seq(cfg: &Cfg, n: usize, dts: &[f64]) -> Result<PimpleSolver, String> {
    let mut s = make(cfg, n);
    for &dt in dts {
        if cfg.prob == Prob::Mms {
            let bf = mms_force(&s.geom, s.time + dt);
            let mut hook = move |_u: &VectorField, _phi: &[f64]| OuterUpdate {
                source_force: Some(bf.clone()),
                ..Default::default()
            };
            s.step_with(dt, Some(&mut hook as &mut OuterHook));
        } else {
            s.step(dt);
        }
        if let Some(d) = &s.diverged {
            return Err(format!("{cfg:?} N={n}: {}", d.message(None)));
        }
    }
    Ok(s)
}

fn const_seq(dt: f64) -> Vec<f64> {
    vec![dt; (1.0 / dt).round() as usize]
}

fn var_seq(dt: f64) -> Vec<f64> {
    let pairs = (1.0 / (2.0 * dt)).round() as usize;
    (0..2 * pairs)
        .map(|i| if i % 2 == 0 { 1.25 * dt } else { 0.75 * dt })
        .collect()
}

const DT_REF: f64 = 1.0 / 1280.0;

fn rel_l2(a: &[Vec3], b: &[Vec3]) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        num += (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2);
        den += y[0] * y[0] + y[1] * y[1];
    }
    (num / den).sqrt()
}

fn exact_error(s: &PimpleSolver, prob: Prob) -> f64 {
    let ex: Vec<Vec3> = s
        .geom
        .mesh
        .cell_centres()
        .iter()
        .map(|&c| exact_u(prob, c, s.time))
        .collect();
    rel_l2(&s.u.values, &ex)
}

/// ADR 0073 N1's pressure error: relative L2 of the cell pressure against the exact one, both with their volume means removed.
fn pressure_error(s: &PimpleSolver, prob: Prob) -> f64 {
    let vol = s.geom.mesh.cell_volumes();
    let tv: f64 = vol.iter().sum();
    let ex: Vec<f64> = s
        .geom
        .mesh
        .cell_centres()
        .iter()
        .map(|&c| exact_p(prob, c, s.time))
        .collect();
    let mean = |v: &[f64]| v.iter().zip(vol).map(|(a, w)| a * w).sum::<f64>() / tv;
    let (mp, me) = (mean(&s.p.values), mean(&ex));
    let (mut num, mut den) = (0.0, 0.0);
    for ((p, e), w) in s.p.values.iter().zip(&ex).zip(vol) {
        num += w * ((p - mp) - (e - me)).powi(2);
        den += w * (e - me).powi(2);
    }
    (num / den).sqrt()
}

/// One order row of BO: the arm's errors of the pair against its reference, the order, and the stop if any.
#[derive(Debug, Clone)]
struct Arm {
    e: [f64; 2],
    ex: [f64; 2],
    order: f64,
    stop: Option<String>,
}

fn arm(cfg: &Cfg, n: usize, dt: f64, variable: bool) -> Arm {
    let seq = |d: f64| if variable { var_seq(d) } else { const_seq(d) };
    let run = || -> Result<Arm, String> {
        let r = run_seq(cfg, n, &const_seq(DT_REF))?;
        let a = run_seq(cfg, n, &seq(dt))?;
        let b = run_seq(cfg, n, &seq(0.5 * dt))?;
        let e = [rel_l2(&a.u.values, &r.u.values), rel_l2(&b.u.values, &r.u.values)];
        Ok(Arm {
            e,
            ex: [exact_error(&a, cfg.prob), exact_error(&b, cfg.prob)],
            order: (e[0] / e[1]).log2(),
            stop: None,
        })
    };
    run().unwrap_or_else(|stop| Arm {
        e: [f64::NAN; 2],
        ex: [f64::NAN; 2],
        order: f64::NAN,
        stop: Some(stop),
    })
}

#[derive(Debug, Clone)]
struct Row {
    id: String,
    cfg: Cfg,
    dt: f64,
    variable: bool,
    lo: f64,
    hi: f64,
    d: Arm,
    c: Arm,
}

impl Row {
    fn in_band(&self, a: &Arm) -> bool {
        a.order >= self.lo && a.order <= self.hi
    }
}

const FINE: f64 = 0.025;
const COARSE: f64 = 0.1;
const BDF2: TimeScheme = TimeScheme::Bdf2;
const EULER: TimeScheme = TimeScheme::ImplicitEuler;

/// The BO rows of ADR 0223: (id, problem, path, scheme, pairs, variable, band).
#[allow(clippy::type_complexity)]
fn bo_specs() -> Vec<(&'static str, Prob, Path, TimeScheme, Vec<f64>, bool, (f64, f64))> {
    let b = (1.9, 2.3);
    let e = (0.9, 1.15);
    vec![
        ("O1", Prob::Tg, Path::Piso, BDF2, vec![FINE, COARSE], false, b),
        ("O2", Prob::Tg, Path::PimpleC, BDF2, vec![FINE, COARSE], false, b),
        ("O3", Prob::Tg, Path::Pimple4, BDF2, vec![FINE], false, b),
        ("O5", Prob::Mode, Path::Piso, BDF2, vec![FINE, COARSE], false, b),
        ("O6", Prob::Mms, Path::Piso, BDF2, vec![FINE, COARSE], false, b),
        ("O6", Prob::Mms, Path::PimpleC, BDF2, vec![FINE, COARSE], false, b),
        ("O7", Prob::Tg, Path::Piso, BDF2, vec![FINE], true, b),
        ("O7", Prob::Mms, Path::Piso, BDF2, vec![FINE], true, b),
        ("O8", Prob::Tg, Path::Piso, EULER, vec![FINE], false, e),
        ("O8", Prob::Mode, Path::Piso, EULER, vec![FINE], false, e),
        ("O8", Prob::Mms, Path::Piso, EULER, vec![FINE], false, e),
        ("TG-S", Prob::TgS, Path::Piso, BDF2, vec![FINE], false, b),
        ("TG-S", Prob::TgS, Path::Pimple4, BDF2, vec![FINE], false, b),
    ]
}

/// One DC4 comparison: (problem, dt, D and C exact velocity errors, D and C pressure errors, stops).
type NoHarm = (Prob, f64, [f64; 2], [f64; 2], Option<String>);

/// O9 of one arm: errors against the exact solution at (16, 0.1), (32, 0.05), (64, 0.025) and the two orders, or the stop.
type JointRef = Result<([f64; 3], [f64; 2]), String>;

struct Bo {
    rows: Vec<Row>,
    o9: [JointRef; 2],
    no_harm: Vec<NoHarm>,
}

fn joint(consistent: bool) -> JointRef {
    let cfg = Cfg {
        prob: Prob::Tg,
        path: Path::Piso,
        scheme: BDF2,
        consistent,
    };
    let mut e = [0.0; 3];
    for (i, (n, dt)) in [(16usize, 0.1), (32, 0.05), (64, 0.025)].into_iter().enumerate() {
        e[i] = exact_error(&run_seq(&cfg, n, &const_seq(dt))?, Prob::Tg);
    }
    Ok((e, [(e[0] / e[1]).log2(), (e[1] / e[2]).log2()]))
}

fn bo() -> &'static Bo {
    static BO: OnceLock<Bo> = OnceLock::new();
    BO.get_or_init(|| {
        let mut jobs = Vec::new();
        for (id, prob, path, scheme, pairs, variable, band) in bo_specs() {
            for dt in pairs {
                jobs.push((id, prob, path, scheme, dt, variable, band));
            }
        }
        let rows = parallel(jobs, |(id, prob, path, scheme, dt, variable, (lo, hi))| {
            let cfg = Cfg {
                prob,
                path,
                scheme,
                consistent: false,
            };
            let d = arm(&cfg, 32, dt, variable);
            let c = arm(
                &Cfg {
                    consistent: true,
                    ..cfg
                },
                32,
                dt,
                variable,
            );
            let row = Row {
                id: id.into(),
                cfg,
                dt,
                variable,
                lo,
                hi,
                d,
                c,
            };
            println!(
                "BO {} {:?}/{:?}/{:?} dt={}{}: D e {:.4e} {:.4e} (exact {:.4e} {:.4e}) order {:.3}; C e {:.4e} {:.4e} (exact {:.4e} \
                 {:.4e}) order {:.3}; band [{lo}, {hi}] D {} C {}{}",
                row.id,
                prob,
                path,
                scheme,
                dt,
                if variable { " variable" } else { "" },
                row.d.e[0],
                row.d.e[1],
                row.d.ex[0],
                row.d.ex[1],
                row.d.order,
                row.c.e[0],
                row.c.e[1],
                row.c.ex[0],
                row.c.ex[1],
                row.c.order,
                if row.in_band(&row.d) { "MET" } else { "MISS" },
                if row.in_band(&row.c) { "MET" } else { "MISS" },
                row.c.stop.as_ref().map_or(String::new(), |s| format!(" C STOPPED: {s}"))
            );
            row
        });
        let o9 = [joint(false), joint(true)];
        for (arm, r) in ["D", "C"].iter().zip(&o9) {
            println!("BO O9 {arm}: {r:?} (limit both orders >= 1.9)");
        }
        let mut nh_jobs = Vec::new();
        for prob in [Prob::Tg, Prob::TgS, Prob::Mms, Prob::Mode] {
            for dt in [0.05, 0.025, 0.0125] {
                nh_jobs.push((prob, dt));
            }
        }
        let no_harm = parallel(nh_jobs, |(prob, dt)| {
            let mut ev = [f64::NAN; 2];
            let mut ep = [f64::NAN; 2];
            let mut stop = None;
            for (i, consistent) in [false, true].into_iter().enumerate() {
                let cfg = Cfg {
                    prob,
                    path: Path::Piso,
                    scheme: BDF2,
                    consistent,
                };
                match run_seq(&cfg, 32, &const_seq(dt)) {
                    Ok(s) => {
                        ev[i] = exact_error(&s, prob);
                        ep[i] = pressure_error(&s, prob);
                    }
                    Err(e) => stop = Some(e),
                }
            }
            println!(
                "BO DC4 {prob:?} PISO BDF2 N=32 dt={dt}: velocity D {:.4e} C {:.4e} (ratio {:.4}); pressure D {:.4e} C {:.4e} (ratio \
                 {:.4}){}",
                ev[0],
                ev[1],
                ev[1] / ev[0],
                ep[0],
                ep[1],
                ep[1] / ep[0],
                stop.as_ref().map_or(String::new(), |s| format!(" STOPPED: {s}"))
            );
            (prob, dt, ev, ep, stop)
        });
        Bo { rows, o9, no_harm }
    })
}

/// ADR 0223 BOc: every run of the option in BO reaches `t = 1` without a refusal or a guard trip.
#[test]
#[ignore = "heavy (ADR 0223 BO, several minutes in release)"]
fn boc_every_run_of_the_option_completes() {
    let b = bo();
    let mut stops: Vec<String> = b.rows.iter().filter_map(|r| r.c.stop.clone()).collect();
    if let Err(e) = &b.o9[1] {
        stops.push(e.clone());
    }
    stops.extend(b.no_harm.iter().filter_map(|n| n.4.clone()));
    println!("BOc: {} rows, {} stopped runs of C", b.rows.len(), stops.len());
    assert!(stops.is_empty(), "BOc {stops:#?}");
}

/// ADR 0223 DC4 (default-decision criterion): the option's exact velocity and pressure errors at most 1.05 times the default's.
/// KNOWN MISS of the option (limit intact): 20 of 21 comparisons met; the MMS pressure error at dt 0.025 is 1.119 times the
/// default's (ADR 0223 addendum).
#[test]
#[ignore = "known miss of the option (ADR 0223 DC4): the MMS PISO pressure error at dt 0.025 is 1.119 x the default's (limit 1.05); heavy"]
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn dc4_no_harm_on_the_exact_errors_known_miss() {
    let mut bad = Vec::new();
    let mut count = 0;
    for (prob, dt, ev, ep, _) in &bo().no_harm {
        count += 1;
        if !(ev[1] <= 1.05 * ev[0]) {
            bad.push(format!("{prob:?} dt={dt} velocity {:.4}", ev[1] / ev[0]));
        }
        if *prob != Prob::Mode {
            count += 1;
            if !(ep[1] <= 1.05 * ep[0]) {
                bad.push(format!("{prob:?} dt={dt} pressure {:.4}", ep[1] / ep[0]));
            }
        }
    }
    println!("DC4: {count} comparisons, {} above 1.05: {bad:?}", bad.len());
    assert_eq!(count, 21, "DC4 comparisons");
    assert!(bad.is_empty(), "DC4 {bad:#?}");
}

/// ADR 0223 DC5 (default-decision criterion): every row whose default order is in its band has the option's order in the band.
/// KNOWN MISS of the option (limit intact): TG PISO's coarse pair (O1) has the order 2.305 with the option (2.290 by default)
/// against the band's top 2.3; the option brings MMS PISO and PIMPLE-C coarse into the band (ADR 0223 addendum).
#[test]
#[ignore = "known miss of the option (ADR 0223 DC5): O1 coarse order 2.305 with the option, above the band [1.9, 2.3] (default 2.290); heavy"]
fn dc5_no_new_order_miss_known_miss() {
    let b = bo();
    let lost: Vec<String> = b
        .rows
        .iter()
        .filter(|r| r.in_band(&r.d) && !r.in_band(&r.c))
        .map(|r| {
            format!(
                "{} {:?}/{:?}/{:?} dt={}{}: D {:.3} C {:.3}",
                r.id,
                r.cfg.prob,
                r.cfg.path,
                r.cfg.scheme,
                r.dt,
                if r.variable { " variable" } else { "" },
                r.d.order,
                r.c.order
            )
        })
        .collect();
    let gained: Vec<String> = b
        .rows
        .iter()
        .filter(|r| !r.in_band(&r.d) && r.in_band(&r.c))
        .map(|r| format!("{} {:?}/{:?} dt={}", r.id, r.cfg.prob, r.cfg.path, r.dt))
        .collect();
    let o9 = |r: &JointRef| r.as_ref().is_ok_and(|(_, o)| o[0] >= 1.9 && o[1] >= 1.9);
    println!(
        "DC5: rows met by D and missed by C: {lost:?}; rows missed by D and met by C: {gained:?}; O9 D {} C {}",
        o9(&b.o9[0]),
        o9(&b.o9[1])
    );
    assert!(lost.is_empty() && (!o9(&b.o9[0]) || o9(&b.o9[1])), "DC5 {lost:#?}");
}

/// ADR 0223 BF (prints): the TG BDF2 fine-pair orders at N = 16, 32, 64 for PISO and PIMPLE-4, arms D and C.
#[test]
#[ignore = "heavy (ADR 0223 BF, prints)"]
fn bf_fine_pair_orders_against_the_mesh() {
    let mut jobs = Vec::new();
    for path in [Path::Piso, Path::Pimple4] {
        for n in [16usize, 32, 64] {
            for consistent in [false, true] {
                jobs.push((path, n, consistent));
            }
        }
    }
    let out = parallel(jobs, |(path, n, consistent)| {
        let cfg = Cfg {
            prob: Prob::Tg,
            path,
            scheme: BDF2,
            consistent,
        };
        (path, n, consistent, arm(&cfg, n, FINE, false))
    });
    for (path, n, consistent, a) in out {
        println!(
            "BF TG {path:?} BDF2 N={n} {}: e {:.4e} {:.4e} order {:.3}{}",
            if consistent { "C" } else { "D" },
            a.e[0],
            a.e[1],
            a.order,
            a.stop.map_or(String::new(), |s| format!(" STOPPED: {s}"))
        );
    }
}

// =================================================================================================================
// BS: ADR 0069's cavity stability harness (copied from w33b_stability.rs)
// =================================================================================================================

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mesh {
    Uniform,
    Graded,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Alg {
    Piso,
    Pimple2,
    Pimple4,
}

const T_END: f64 = 10.0;

fn setup(
    mesh: Mesh,
    nu: f64,
    alg: Alg,
    scheme: TimeScheme,
    tweak: &dyn Fn(&mut PimpleSettings),
) -> (PimpleSolver, f64) {
    let n = 32;
    let (m, lid) = match mesh {
        Mesh::Uniform => (cavity_2d(n, 1.0).unwrap(), "lid"),
        Mesh::Graded => (
            box_2d_graded(n, n, 1.0, 1.0, [Grading::Symmetric(20.0); 2], false, false).unwrap(),
            "ymax",
        ),
    };
    let geom = Arc::new(FvGeometry::new(Arc::new(m)).unwrap());
    let h_min = 2.0 * geom.dn.iter().copied().fold(f64::INFINITY, f64::min);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    match mesh {
        Mesh::Uniform => {
            u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
                .unwrap();
            u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
                .unwrap();
        }
        Mesh::Graded => {
            u.set_bc(&geom, lid, Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
                .unwrap();
            for w in ["xmin", "xmax", "ymin"] {
                u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
            }
        }
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = match alg {
        Alg::Piso => PimpleSettings::piso(nu),
        Alg::Pimple2 => PimpleSettings::pimple(nu, 2),
        Alg::Pimple4 => PimpleSettings::pimple(nu, 4),
    };
    st.convection = ConvScheme::LinearUpwind;
    st.time_scheme = scheme;
    tweak(&mut st);
    (PimpleSolver::new(geom, u, p, st), h_min)
}

#[derive(Debug, Clone)]
struct Outcome {
    stable: bool,
    why: String,
    steps: usize,
    max_u: f64,
    max_p: f64,
    max_co: f64,
    max_cont: f64,
    t_reached: f64,
    refused: bool,
}

fn run_case(
    mesh: Mesh,
    re: f64,
    alg: Alg,
    scheme: TimeScheme,
    co_t: f64,
    tweak: &dyn Fn(&mut PimpleSettings),
) -> Outcome {
    let (mut s, h_min) = setup(mesh, 1.0 / re, alg, scheme, tweak);
    let dt = co_t * h_min;
    let mut o = Outcome {
        stable: true,
        why: String::new(),
        steps: 0,
        max_u: 0.0,
        max_p: 0.0,
        max_co: 0.0,
        max_cont: 0.0,
        t_reached: 0.0,
        refused: false,
    };
    fn fail(o: &mut Outcome, why: String) {
        if o.stable {
            o.stable = false;
            o.why = why;
        }
    }
    while s.time < T_END - 1e-12 {
        let d = dt.min(T_END - s.time);
        let info = s.step(d);
        o.steps += 1;
        o.t_reached = s.time;
        if let Some(dv) = &s.diverged {
            o.refused = dv.quantity == "consistent correctors";
            let k = o.steps;
            fail(&mut o, format!("guard trip at step {k}"));
            break;
        }
        let mu =
            s.u.values
                .iter()
                .map(|v| (v[0] * v[0] + v[1] * v[1]).sqrt())
                .fold(0.0f64, |a, b| if b.is_nan() { f64::INFINITY } else { a.max(b) });
        let mp =
            s.p.values
                .iter()
                .map(|x| x.abs())
                .fold(0.0f64, |a, b| if b.is_nan() { f64::INFINITY } else { a.max(b) });
        o.max_u = o.max_u.max(mu);
        o.max_p = o.max_p.max(mp);
        o.max_co = o.max_co.max(info.courant);
        o.max_cont = o.max_cont.max(info.continuity);
        if !(mu.is_finite() && mp.is_finite()) {
            let k = o.steps;
            fail(&mut o, format!("non-finite at step {k}"));
            break;
        }
        if mu > 1.5 {
            fail(&mut o, format!("max|u| {mu:.3} > 1.5 at t = {:.3}", s.time));
            if mu > 1e3 {
                break;
            }
        }
        if mp > 50.0 {
            fail(&mut o, format!("max|p| {mp:.3} > 50 at t = {:.3}", s.time));
            if mp > 1e6 {
                break;
            }
        }
        if info.continuity > 1e-3 {
            fail(
                &mut o,
                format!("continuity {:.2e} > 1e-3 at t = {:.3}", info.continuity, s.time),
            );
        }
    }
    if o.stable && s.time < T_END - 1e-9 {
        fail(&mut o, "t_end not reached".into());
    }
    o
}

fn required_co(alg: Alg) -> &'static [f64] {
    match alg {
        Alg::Piso => &[0.25, 0.5, 1.0],
        _ => &[0.5, 1.0, 2.0, 4.0],
    }
}

fn scan_co(alg: Alg) -> &'static [f64] {
    match alg {
        Alg::Piso => &[2.0, 4.0, 8.0],
        _ => &[8.0, 16.0],
    }
}

const BOTH: [TimeScheme; 2] = [EULER, BDF2];

/// One BS run: (mesh, Re, algorithm, scheme, Co_t, arm C, required set) and its outcome.
type BsRun = (Mesh, f64, Alg, TimeScheme, f64, bool, bool, Outcome);

fn bs() -> &'static Vec<BsRun> {
    static BS: OnceLock<Vec<BsRun>> = OnceLock::new();
    BS.get_or_init(|| {
        let mut jobs = Vec::new();
        for mesh in [Mesh::Uniform, Mesh::Graded] {
            for alg in [Alg::Piso, Alg::Pimple2, Alg::Pimple4] {
                for re in [100.0, 1000.0] {
                    for scheme in BOTH {
                        for &co in required_co(alg) {
                            jobs.push((mesh, re, alg, scheme, co, false, true));
                            jobs.push((mesh, re, alg, scheme, co, true, true));
                        }
                        for &co in scan_co(alg) {
                            jobs.push((mesh, re, alg, scheme, co, true, false));
                        }
                    }
                }
            }
        }
        // the slow (graded, small Co) runs first
        jobs.sort_by(|a, b| (b.0 == Mesh::Graded).cmp(&(a.0 == Mesh::Graded)).then(a.4.total_cmp(&b.4)));
        parallel(jobs, |(mesh, re, alg, scheme, co, c, required)| {
            let o = run_case(mesh, re, alg, scheme, co, &|st: &mut PimpleSettings| {
                if c {
                    option_c(st)
                }
            });
            println!(
                "BS {} {mesh:?} Re={re} {alg:?} {scheme:?} Co_t={co} {}: {} (achieved Co {:.3}, steps {}, t {:.2}, max|u| {:.3}, max|p| \
                 {:.3}, cont {:.1e}){}",
                if required { "required" } else { "scan" },
                if c { "C" } else { "D" },
                if o.stable { "STABLE" } else { "UNSTABLE" },
                o.max_co,
                o.steps,
                o.t_reached,
                o.max_u,
                o.max_p,
                o.max_cont,
                if o.why.is_empty() { String::new() } else { format!(" [{}]", o.why) }
            );
            (mesh, re, alg, scheme, co, c, required, o)
        })
    })
}

/// ADR 0223 BSc and BSd: every BS run made (none refused); the default's required-set verdicts are ADR 0069's.
#[test]
#[ignore = "heavy (ADR 0223 BS, about an hour in release)"]
fn bsc_bsd_the_stability_set_ran_and_the_default_reproduces_adr_0069() {
    let runs = bs();
    let refused: Vec<String> = runs
        .iter()
        .filter(|r| r.7.refused)
        .map(|r| format!("{:?} Re={} {:?} {:?} Co_t={} C={}", r.0, r.1, r.2, r.3, r.4, r.5))
        .collect();
    let required_d: Vec<&BsRun> = runs.iter().filter(|r| r.6 && !r.5).collect();
    let unstable_d: Vec<String> = required_d
        .iter()
        .filter(|r| !r.7.stable)
        .map(|r| format!("{:?} Re={} {:?} {:?} Co_t={}", r.0, r.1, r.2, r.3, r.4))
        .collect();
    let expected = |r: &&BsRun| r.2 == Alg::Pimple2 && r.1 == 100.0 && r.4 == 4.0;
    let bsd = required_d.len() == 88
        && required_d.iter().filter(|r| !r.7.stable).all(&expected)
        && required_d.iter().filter(|r| expected(r)).all(|r| !r.7.stable);
    println!(
        "BSc: {} runs, refused {refused:?}; BSd: D required set {} runs, unstable {unstable_d:?} (ADR 0069: exactly the four PIMPLE-2 \
         Co_t 4 Re 100 runs) -> {bsd}",
        runs.len(),
        required_d.len()
    );
    assert_eq!(runs.len(), 232, "BSc run count");
    assert!(refused.is_empty(), "BSc refused {refused:#?}");
    assert!(bsd, "BSd: the default does not reproduce ADR 0069: {unstable_d:#?}");
}

/// ADR 0223 DC1 (default-decision criterion): the required set is stable with the option (88 of 88).
#[test]
#[ignore = "heavy (ADR 0223 BS, about an hour in release)"]
fn dc1_the_required_stability_set_with_the_option() {
    let runs = bs();
    let c: Vec<&BsRun> = runs.iter().filter(|r| r.6 && r.5).collect();
    let unstable: Vec<String> = c
        .iter()
        .filter(|r| !r.7.stable)
        .map(|r| format!("{:?} Re={} {:?} {:?} Co_t={}: {}", r.0, r.1, r.2, r.3, r.4, r.7.why))
        .collect();
    let scan_unstable: Vec<String> = runs
        .iter()
        .filter(|r| !r.6 && !r.7.stable)
        .map(|r| format!("{:?} Re={} {:?} {:?} Co_t={}: {}", r.0, r.1, r.2, r.3, r.4, r.7.why))
        .collect();
    println!(
        "DC1: C required set {} runs, {} stable; unstable {unstable:?}; scan (C) unstable {scan_unstable:?}",
        c.len(),
        c.len() - unstable.len()
    );
    assert_eq!(c.len(), 88, "DC1 run count");
    assert!(unstable.is_empty(), "DC1 {unstable:#?}");
}

// =================================================================================================================
// BB: ADR 0073 Part B (copied from w34b_lag.rs)
// =================================================================================================================

fn c0_of(scheme: TimeScheme) -> f64 {
    if scheme == BDF2 {
        1.5
    } else {
        1.0
    }
}

/// The B2 periodic Stokes box with the option (the check estimate at step 6): (measured g, rho at step 6) or the stop.
fn box_run(d: f64, scheme: TimeScheme, n_outer: usize) -> Result<(f64, f64), String> {
    let n = 32;
    let h = 1.0 / n as f64;
    let dt = 0.01;
    let c0 = c0_of(scheme);
    let nu = d * c0 / (4.0 / (h * h) * dt);
    let mesh = Arc::new(box_2d(n, n, 1.0, 1.0, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let p = ScalarField::from_fn("p", &geom, |c| 1e-6 * (2.0 * PI * c[0]).cos() * (2.0 * PI * c[1]).cos());
    let mut st = PimpleSettings::pimple(nu, n_outer);
    st.convection = ConvScheme::LinearUpwind;
    st.time_scheme = scheme;
    st.momentum_solver = SolveControl {
        tol_abs: 1e-30,
        tol_rel: 1e-10,
        max_iter: 5000,
    };
    st.pressure_solver = SolveControl {
        tol_abs: 1e-30,
        tol_rel: 1e-10,
        max_iter: 5000,
    };
    st.pressure_lag_check = Some(LagCheck::new(6, LagAction::Warn));
    option_c(&mut st);
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    let vol = geom.mesh.cell_volumes().to_vec();
    let tv: f64 = vol.iter().sum();
    let norm = |p: &[f64]| -> f64 {
        let m = p.iter().zip(&vol).map(|(a, v)| a * v).sum::<f64>() / tv;
        p.iter().zip(&vol).map(|(a, v)| (a - m).powi(2) * v).sum::<f64>().sqrt()
    };
    let mut norms = Vec::new();
    for _ in 0..12 {
        s.step(dt);
        if let Some(dv) = &s.diverged {
            return Err(dv.message(None));
        }
        norms.push(norm(&s.p.values));
    }
    let g = (norms[11] / norms[5]).powf(1.0 / 6.0);
    let rho = s.lag_estimates.iter().find(|w| w.step == 6).map_or(f64::NAN, |w| w.rho);
    Ok((g, rho))
}

/// One BB B2 row: (scheme, n_outer, D, result).
type B2Row = (TimeScheme, usize, f64, Result<(f64, f64), String>);

struct Bb {
    b2: Vec<B2Row>,
    b3: Vec<(Alg, TimeScheme, f64, Outcome)>,
}

fn bb() -> &'static Bb {
    static BB: OnceLock<Bb> = OnceLock::new();
    BB.get_or_init(|| {
        let mut jobs = Vec::new();
        for scheme in BOTH {
            for n_outer in [2usize, 4] {
                for d in [1.0, 2.0, 3.0, 4.0, 6.0] {
                    jobs.push((scheme, n_outer, d));
                }
            }
        }
        let b2 = parallel(jobs, |(scheme, n_outer, d)| {
            let r = box_run(d, scheme, n_outer);
            println!("BB B2 C {scheme:?} n_outer={n_outer} D={d}: {r:?} (measured g, check rho at step 6)");
            (scheme, n_outer, d, r)
        });
        let cases = vec![
            (Alg::Pimple2, EULER, 2.3),
            (Alg::Pimple2, EULER, 3.0),
            (Alg::Pimple2, BDF2, 3.4),
            (Alg::Pimple2, BDF2, 4.6),
            (Alg::Pimple4, EULER, 5.0),
            (Alg::Pimple4, BDF2, 6.0),
            (Alg::Pimple4, BDF2, 7.5),
        ];
        let b3 = parallel(cases, |(alg, scheme, co)| {
            let o = run_case(Mesh::Uniform, 100.0, alg, scheme, co, &|st: &mut PimpleSettings| {
                option_c(st)
            });
            println!(
                "BB B3 C Uniform Re=100 {alg:?} {scheme:?} Co_t={co}: {}{}",
                if o.stable { "STABLE" } else { "UNSTABLE" },
                if o.why.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", o.why)
                }
            );
            (alg, scheme, co, o)
        });
        Bb { b2, b3 }
    })
}

/// ADR 0223 BBc: every BB run made with the option (the B2 box needs the periodic extension).
#[test]
#[ignore = "heavy (ADR 0223 BB, minutes in release)"]
fn bbc_part_b_ran_with_the_option() {
    let b = bb();
    let stopped: Vec<String> =
        b.b2.iter()
            .filter_map(|r| r.3.as_ref().err().map(|e| format!("{:?} {} D={}: {e}", r.0, r.1, r.2)))
            .chain(
                b.b3.iter()
                    .filter(|r| r.3.refused)
                    .map(|r| format!("B3 {:?} {:?} {}: refused", r.0, r.1, r.2)),
            )
            .collect();
    println!(
        "BBc: {} B2 rows, {} B3 runs; refused or stopped B2 rows: {stopped:?}",
        b.b2.len(),
        b.b3.len()
    );
    assert!(
        b.b2.len() == 20 && b.b3.len() == 7 && stopped.is_empty(),
        "BBc {stopped:#?}"
    );
}

/// ADR 0223 DC2 (default-decision criterion): every B2 row decays with the option and its check estimate is below one.
#[test]
#[ignore = "heavy (ADR 0223 BB, minutes in release)"]
fn dc2_the_periodic_stokes_box_decays_with_the_option() {
    let bad: Vec<String> = bb()
        .b2
        .iter()
        .filter(|r| !matches!(r.3, Ok((g, rho)) if g < 1.0 && rho < 1.0))
        .map(|r| format!("{:?} {} D={}: {:?}", r.0, r.1, r.2, r.3))
        .collect();
    println!("DC2: rows that do not decay or have rho >= 1: {bad:?}");
    assert!(bad.is_empty(), "DC2 {bad:#?}");
}

/// ADR 0223 DC3 (default-decision criterion): the seven B3 cavity runs are stable with the option.
#[test]
#[ignore = "heavy (ADR 0223 BB, minutes in release)"]
fn dc3_the_b3_cavity_runs_are_stable_with_the_option() {
    let bad: Vec<String> = bb()
        .b3
        .iter()
        .filter(|r| !r.3.stable)
        .map(|r| format!("{:?} {:?} Co_t={}: {}", r.0, r.1, r.2, r.3.why))
        .collect();
    println!("DC3: unstable B3 runs with the option: {bad:?}");
    assert!(bad.is_empty(), "DC3 {bad:#?}");
}
