//! ADR 0050 Part 2 checks S1-S7, S9, S10 (F-209 adaptive time stepping). The limits are those registered in the ADR before the
//! module `rcfd_fv::stepctl` existed; none was changed afterwards.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::{add_ddt, History, TimeScheme};
use rcfd_fv::fvmatrix::FvMatrix;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::stepctl::{
    courant_stats, extrapolate_initial, Aitken, ControlledRun, CourantStats, StepController, StepSettings,
};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_mesh, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

fn tg(n: usize, nu: f64, scheme: TimeScheme) -> PimpleSolver {
    let l = 2.0 * PI;
    let mesh = Arc::new(box_2d(n, n, l, l, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [c[0].sin() * c[1].cos(), -c[0].cos() * c[1].sin(), 0.0]);
    let p = ScalarField::from_fn("p", &geom, |c| 0.25 * ((2.0 * c[0]).cos() + (2.0 * c[1]).cos()));
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.time_scheme = scheme;
    PimpleSolver::new(geom, u, p, s)
}

fn l2_diff(a: &PimpleSolver, b: &PimpleSolver) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for (x, y) in a.u.values.iter().zip(&b.u.values) {
        num += (x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2);
        den += y[0] * y[0] + y[1] * y[1];
    }
    (num / den).sqrt()
}

fn reference(nu: f64, t_end: f64, dt: f64) -> PimpleSolver {
    let mut r = tg(32, nu, TimeScheme::Bdf2);
    r.run_fixed(t_end, dt);
    r
}

/// The step the controller would aim at for the initial field: `aim / Co(dt = 1)`.
fn aimed_dt(s: &PimpleSolver, aim: f64) -> f64 {
    aim / courant_stats(&s.geom, &s.phi, 1.0).max
}

fn courant_settings(co_max: f64, co_rms: f64) -> StepSettings {
    StepSettings {
        co_max,
        co_rms,
        min_dt: 1e-4,
        max_dt: 5.0,
        ..Default::default()
    }
}

#[test]
fn s1_s3_courant_max_band_overshoot_economy_and_agreement() {
    let nu = 0.05;
    let (target, t_end) = (0.5, 10.0);
    let mut s = tg(32, nu, TimeScheme::ImplicitEuler);
    let dt_aim = aimed_dt(&s, 0.9 * target);
    let mut run = ControlledRun::new(StepController::new(courant_settings(target, 0.0), 0.4 * dt_aim), false);
    let infos = run.run(&mut s, t_end);
    let n = infos.len();
    if std::env::var("DBG").is_ok() {
        for (k, i) in infos.iter().enumerate() {
            println!(
                "  k {k} t {:.5} dt {:.5} Co/target {:.4}",
                i.time,
                i.dt,
                i.courant / target
            );
        }
    }
    let start = infos
        .iter()
        .position(|i| i.courant >= 0.8 * target)
        .expect("transient start");
    let co_min = infos[start..n - 1].iter().map(|i| i.courant).fold(f64::MAX, f64::min);
    let co_top = infos[start..n - 1].iter().map(|i| i.courant).fold(0.0, f64::max);
    let over = infos[1..].iter().map(|i| i.courant).fold(0.0, f64::max) / target;
    let growth = infos.windows(2).map(|w| w[1].dt / w[0].dt).fold(0.0, f64::max);
    println!(
        "S1: {n} steps, start at step {start}, Co in [{:.4}, {:.4}] x target, max overshoot {over:.4}, max dt growth {growth:.4}",
        co_min / target,
        co_top / target
    );
    assert!(co_min >= 0.8 * target && co_top <= target, "band");
    assert!(over <= 1.05, "overshoot {over}");
    assert!(growth <= 1.2 * (1.0 + 1e-12));
    assert!(infos.iter().all(|i| i.dt >= 1e-4 && i.dt <= 5.0));
    assert!((s.time - t_end).abs() <= 1e-12 * t_end);
    // S3: economy against the fixed run at the aimed initial step, agreement with the small-step reference
    let n_fixed = (t_end / dt_aim).ceil() as usize;
    let r = reference(nu, t_end, 1.0 / 80.0);
    let e = l2_diff(&s, &r);
    println!(
        "S3: adaptive {n} steps vs fixed {n_fixed} (limit 0.75), L2 difference to the reference {e:.3e} (limit 0.02)"
    );
    assert!(n as f64 <= 0.75 * n_fixed as f64, "{n} vs {n_fixed}");
    assert!(e <= 0.02, "{e}");
}

#[test]
fn s2_courant_rms_band_and_combination() {
    let (target, t_end) = (0.2, 10.0);
    let mut s = tg(32, 0.05, TimeScheme::ImplicitEuler);
    let st0 = courant_stats(&s.geom, &s.phi, 1.0);
    let dt_aim = 0.9 * target / st0.rms;
    let mut run = ControlledRun::new(StepController::new(courant_settings(0.0, target), 0.4 * dt_aim), false);
    let mut start = None;
    let (mut lo, mut hi) = (f64::MAX, 0.0_f64);
    let mut steps = 0;
    while s.time < t_end - 1e-12 {
        let dt_before = run.ctl.dt_next;
        let i = run.advance(&mut s, t_end);
        let st = courant_stats(&s.geom, &s.phi, i.dt);
        assert!(st.rms <= st.max * (1.0 + 1e-12), "rms <= max");
        steps += 1;
        let _ = dt_before;
        if start.is_none() && st.rms >= 0.8 * target {
            start = Some(steps);
        }
        if start.is_some() && s.time < t_end - 1e-9 {
            lo = lo.min(st.rms);
            hi = hi.max(st.rms);
        }
    }
    println!(
        "S2: RMS Courant in [{:.4}, {:.4}] x target after step {:?}",
        lo / target,
        hi / target,
        start
    );
    assert!(lo >= 0.8 * target * (1.0 - 1e-12) && hi <= target * (1.0 + 1e-12));
    // both criteria: the minimum of the single proposals
    let stats = CourantStats { max: 0.9, rms: 0.3 };
    let (d, prop) = (0.01, 0.01);
    let both = StepController::new(courant_settings(0.5, 0.2), 0.01).propose_courant(stats, d, prop);
    let a = StepController::new(courant_settings(0.5, 0.0), 0.01).propose_courant(stats, d, prop);
    let b = StepController::new(courant_settings(0.0, 0.2), 0.01).propose_courant(stats, d, prop);
    assert!((both - a.min(b)).abs() <= 1e-14 * both, "{both} {a} {b}");
}

fn error_run(tol: f64, nu: f64, t_end: f64, dt0: f64) -> (PimpleSolver, ControlledRun) {
    let mut s = tg(32, nu, TimeScheme::ImplicitEuler);
    let st = StepSettings {
        error_tol: tol,
        min_dt: 1e-5,
        max_dt: 1.0,
        ..Default::default()
    };
    let mut run = ControlledRun::new(StepController::new(st, dt0), false);
    run.run(&mut s, t_end);
    (s, run)
}

#[test]
fn s4_error_control_global_error_scales_with_tolerance() {
    let (nu, t_end) = (0.5, 1.0);
    let r = reference(nu, t_end, 1.0 / 400.0);
    let mut e = Vec::new();
    for tol in [1e-2, 5e-3, 2.5e-3] {
        let (s, run) = error_run(tol, nu, t_end, 0.01);
        let err = l2_diff(&s, &r);
        println!(
            "S4: tol {tol:.1e}: {} steps ({} rejected), E = {err:.3e}, E/(tol T) = {:.3}",
            run.ctl.steps,
            run.ctl.rejected,
            err / (tol * t_end)
        );
        assert!(
            (0.3..=3.0).contains(&(err / (tol * t_end))),
            "ratio {}",
            err / (tol * t_end)
        );
        e.push(err);
    }
    for w in e.windows(2) {
        let q = w[0] / w[1];
        println!("S4: E(tol)/E(tol/2) = {q:.3} (band [1.5, 2.6])");
        assert!((1.5..=2.6).contains(&q), "{q}");
    }
}

#[test]
fn s5_landing_on_write_times_and_period_steps() {
    let (nu, t_end) = (0.05, 10.0);
    let mut s = tg(32, nu, TimeScheme::ImplicitEuler);
    let dt_aim = aimed_dt(&s, 0.45);
    let times = vec![0.37, 1.0, 2.5, 7.123456789, 10.0];
    let mut set = courant_settings(0.5, 0.0);
    set.write_times = times.clone();
    let mut run = ControlledRun::new(StepController::new(set, 0.4 * dt_aim), false);
    let mut worst: f64 = 0.0;
    while s.time < t_end - 1e-12 {
        let t_before = s.time;
        let n0 = run.landed.len();
        let i = run.advance(&mut s, t_end);
        if run.landed.len() > n0 {
            let tw = *run.landed.last().unwrap();
            worst = worst
                .max((s.time - tw).abs() / tw)
                .max((t_before + i.dt - tw).abs() / tw);
        }
    }
    println!(
        "S5: landed {:?}, worst relative landing error {worst:.2e} (limit 1e-12)",
        run.landed
    );
    assert_eq!(run.landed.len(), times.len());
    for (a, b) in run.landed.iter().zip(&times) {
        assert!((a - b).abs() <= 1e-12 * b);
    }
    assert!(worst <= 1e-12);
    // period-based steps
    let mut s = tg(32, nu, TimeScheme::ImplicitEuler);
    let set = StepSettings {
        period: 2.0,
        steps_per_period: 20,
        ..Default::default()
    };
    let mut run = ControlledRun::new(StepController::new(set, 1.0), false);
    let infos = run.run(&mut s, 6.0);
    assert_eq!(infos.len(), 60);
    for (k, i) in infos.iter().enumerate() {
        let tk = (k + 1) as f64 * 2.0 / 20.0;
        assert!((i.dt - 0.1).abs() <= 1e-14, "dt {}", i.dt);
        assert!((i.time - tk).abs() <= 1e-12 * tk, "t {} vs {tk}", i.time);
    }
}

fn bits(s: &PimpleSolver) -> Vec<u64> {
    s.u.values.iter().flatten().map(|x| x.to_bits()).collect()
}

fn restart_case(settings: StepSettings, dt0: f64) {
    let make = || tg(32, 0.5, TimeScheme::ImplicitEuler);
    let t_far = 1.0e3;
    // uninterrupted
    let mut a = make();
    let mut ra = ControlledRun::new(StepController::new(settings.clone(), dt0), false);
    let dts_a: Vec<u64> = (0..40).map(|_| ra.advance(&mut a, t_far).dt.to_bits()).collect();
    // determinism: an identical second run
    let mut a2 = make();
    let mut ra2 = ControlledRun::new(StepController::new(settings.clone(), dt0), false);
    let dts_a2: Vec<u64> = (0..40).map(|_| ra2.advance(&mut a2, t_far).dt.to_bits()).collect();
    assert_eq!(dts_a, dts_a2, "deterministic dt sequence");
    assert_eq!(bits(&a), bits(&a2));
    // interrupted after 15 steps, restored into a fresh solver and a fresh controller
    let mut b = make();
    let mut rb = ControlledRun::new(StepController::new(settings.clone(), dt0), false);
    let mut dts_b: Vec<u64> = (0..15).map(|_| rb.advance(&mut b, t_far).dt.to_bits()).collect();
    let snap = b.snapshot();
    let state = rb.ctl.to_f64s();
    let mut c = make();
    c.restore(&snap).unwrap();
    let mut rc = ControlledRun::new(StepController::new(settings, 12345.0), false);
    rc.ctl.restore_f64s(&state).unwrap();
    dts_b.extend((0..25).map(|_| rc.advance(&mut c, t_far).dt.to_bits()));
    assert_eq!(dts_a, dts_b, "restart gives a bit-identical dt sequence");
    assert_eq!(bits(&a), bits(&c), "restart gives bit-identical velocity");
    assert_eq!(ra.ctl.rejected, rc.ctl.rejected);
}

#[test]
fn s6_deterministic_and_restart_safe_courant_and_error_modes() {
    let mut c = courant_settings(0.5, 0.0);
    c.max_dt = 0.05;
    restart_case(c, 0.01);
    let e = StepSettings {
        error_tol: 1e-3,
        min_dt: 1e-5,
        max_dt: 0.05,
        ..Default::default()
    };
    // dt0 = 0.2 forces rejected trial steps
    restart_case(e.clone(), 0.2);
    let mut s = tg(32, 0.5, TimeScheme::ImplicitEuler);
    let mut r = ControlledRun::new(StepController::new(e, 0.2), false);
    r.advance(&mut s, 1e3);
    assert!(r.ctl.rejected > 0, "the first trial step must be rejected");
}

fn one_cell() -> (FvGeometry, Vec<f64>) {
    let g = FvGeometry::new(Arc::new(box_mesh([1, 1, 1], [1.0; 3], [Grading::Uniform; 3]).unwrap())).unwrap();
    let vol = g.mesh.cell_volumes().to_vec();
    (g, vol)
}

#[test]
fn s7_variable_step_bdf2_coefficients_are_exact_for_quadratics() {
    // y' = f with f = 2 t: the history holds exact values of y = t^2; the BDF2 step must reproduce t^2 for any step ratio
    let (g, vol) = one_cell();
    let ctl = SolveControl {
        tol_abs: 1e-15,
        tol_rel: 1e-14,
        max_iter: 10,
    };
    for (t, dt_old, dt) in [
        (1.0, 0.1, 0.1),
        (1.0, 0.1, 0.25),
        (2.0, 0.3, 0.05),
        (0.7, 0.02, 0.06),
        (3.0, 0.5, 1.5),
    ] {
        let y = |x: f64| x * x;
        let mut hist = History::new(vec![y(t)]);
        hist.older = Some(vec![y(t - dt_old)]);
        hist.dt_old = Some(dt_old);
        let mut sys = FvMatrix::zeros(&g);
        sys.b[0] = vol[0] * 2.0 * (t + dt);
        let sys = add_ddt(TimeScheme::Bdf2, sys, &vol, dt, &hist);
        let mut x = vec![0.0];
        sys.solve(&mut x, &ctl);
        assert!(
            (x[0] - y(t + dt)).abs() <= 1e-12 * y(t + dt),
            "r = {}: {} vs {}",
            dt / dt_old,
            x[0],
            y(t + dt)
        );
    }
}

#[test]
fn s7_bdf2_order_two_with_controller_generated_steps() {
    // y' = -2 y + sin t, y(0) = 1; exact y = 6/5 e^{-2t} + (2 sin t - cos t)/5
    let (g, vol) = one_cell();
    let ctl = SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-14,
        max_iter: 10,
    };
    let exact = |t: f64| 1.2 * (-2.0 * t).exp() + (2.0 * t.sin() - t.cos()) / 5.0;
    let t_end = 2.0;
    let run = |h: f64| {
        let st = StepSettings {
            co_max: h,
            safety: 1.0,
            growth: 1.2,
            shrink: 0.5,
            min_dt: 1e-9,
            max_dt: 1.0,
            ..Default::default()
        };
        let mut c = StepController::new(st, 0.5 * h);
        let (mut t, mut phi) = (0.0_f64, vec![1.0]);
        let mut hist = History::new(phi.clone());
        let (mut rmin, mut rmax) = (f64::MAX, 0.0_f64);
        let mut prev: Option<f64> = None;
        while t < t_end - 1e-13 {
            let (d, target) = c.land(t, c.dt_next, t_end);
            let tn = target.unwrap_or(t + d);
            let mut sys = FvMatrix::zeros(&g);
            sys.a.diag[0] = 2.0 * vol[0];
            sys.b[0] = vol[0] * tn.sin();
            let sys = add_ddt(TimeScheme::Bdf2, sys, &vol, d, &hist);
            sys.solve(&mut phi, &ctl);
            hist.push(phi.clone(), d);
            if let Some(p) = prev {
                rmin = rmin.min(d / p);
                rmax = rmax.max(d / p);
            }
            prev = Some(d);
            // synthetic Courant statistics proportional to the step, varying in time like a transient flow
            let co = d * (1.0 + 0.8 * (3.0 * tn).sin());
            let dt_prop = c.dt_next;
            c.dt_next = c.propose_courant(CourantStats { max: co, rms: 0.0 }, d, dt_prop);
            t = tn;
        }
        ((phi[0] - exact(t_end)).abs(), rmin, rmax)
    };
    let (e1, r1, r2) = run(0.04);
    let (e2, _, _) = run(0.02);
    let (e3, _, _) = run(0.01);
    let (p1, p2) = ((e1 / e2).log2(), (e2 / e3).log2());
    println!("S7: errors {e1:.3e} {e2:.3e} {e3:.3e}, orders {p1:.3} {p2:.3} (band [1.85, 2.2]), step ratios in [{r1:.3}, {r2:.3}]");
    assert!(r2 / r1 > 1.2, "the step sequence must vary");
    assert!((1.85..=2.2).contains(&p2), "{p2}");
}

fn residual_ratio(extrapolate: bool, steps: usize, dt: f64) -> (Vec<f64>, PimpleSolver) {
    let mut s = tg(32, 0.05, TimeScheme::ImplicitEuler);
    let mut res = Vec::new();
    for _ in 0..steps {
        if extrapolate {
            extrapolate_initial(&mut s, dt);
        }
        res.push(s.step(dt).res_u);
    }
    (res, s)
}

#[test]
fn s9_extrapolated_initial_guess_reduces_the_initial_residual() {
    let dt = 0.1;
    let (plain, sp) = residual_ratio(false, 20, dt);
    let (extra, se) = residual_ratio(true, 20, dt);
    let mut ratios: Vec<f64> = (2..20).map(|k| extra[k] / plain[k]).collect();
    ratios.sort_by(|a, b| a.total_cmp(b));
    let median = ratios[ratios.len() / 2];
    let d = l2_diff(&se, &sp);
    println!("S9: median res_u ratio {median:.3e} (limit 0.5), L2 difference at t = 2: {d:.3e} (limit 5e-3)");
    assert!(median <= 0.5, "{median}");
    assert!(d <= 5e-3, "{d}");
}

#[test]
fn s10_aitken_relaxation_accelerates_a_linear_fixed_point() {
    let a = [0.2, 0.5, 1.0, 1.5];
    let rhs = [1.0, -2.0, 0.5, 3.0];
    let res = |x: &[f64]| -> Vec<f64> { (0..4).map(|i| rhs[i] - a[i] * x[i]).collect() };
    let norm = |r: &[f64]| r.iter().map(|v| v * v).sum::<f64>().sqrt();
    let solve = |aitken: bool| -> usize {
        let mut x = vec![0.0; 4];
        let mut at = Aitken::new(0.5, 0.05, 1.5);
        for it in 1..5000 {
            let r = res(&x);
            if norm(&r) < 1e-10 {
                return it;
            }
            let w = if aitken { at.factor(&r) } else { 0.5 };
            for i in 0..4 {
                x[i] += w * r[i];
            }
        }
        5000
    };
    let (fixed, acc) = (solve(false), solve(true));
    println!("S10: fixed omega 0.5: {fixed} iterations, Aitken: {acc} (limit 0.5 x fixed)");
    assert!(acc as f64 <= 0.5 * fixed as f64);
}
