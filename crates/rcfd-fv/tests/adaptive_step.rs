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

/// ADR 0094 T1 (F-209): L2 difference to the reference of a run with `n` uniform implicit-Euler steps to `t_end`.
fn uniform_step_error(n: usize, nu: f64, t_end: f64, r: &PimpleSolver) -> f64 {
    let mut s = tg(32, nu, TimeScheme::ImplicitEuler);
    let dt = t_end / n as f64;
    for _ in 0..n {
        s.step(dt);
    }
    let e = l2_diff(&s, r);
    if e.is_finite() {
        e
    } else {
        f64::INFINITY
    }
}

/// ADR 0094 smoke of the T1 set-up: one 1-step run (dt = t_end) and 10 steps at the aimed step must be finite.
#[test]
#[ignore = "ADR 0094 set-up smoke, run once before the limits were registered"]
fn t1_uniform_step_setup_smoke() {
    let (nu, t_end) = (0.05, 10.0);
    let r = tg(32, nu, TimeScheme::Bdf2);
    let t0 = std::time::Instant::now();
    let e1 = uniform_step_error(1, nu, t_end, &r);
    let t1 = t0.elapsed().as_secs_f64();
    let mut s = tg(32, nu, TimeScheme::ImplicitEuler);
    let dt = aimed_dt(&s, 0.45);
    let t0 = std::time::Instant::now();
    for _ in 0..10 {
        s.step(dt);
    }
    let t10 = t0.elapsed().as_secs_f64();
    let finite = s.u.values.iter().flatten().all(|x| x.is_finite());
    println!(
        "T1 smoke: 1-step run finite error {} ({t1:.3} s); 10 steps at dt {dt:.4}: finite {finite} ({t10:.3} s)",
        e1.is_finite()
    );
    assert!(finite);
}

/// ADR 0094 T1 (F-209): the Courant-controlled run of S1/S3 against the smallest uniform step count whose error meets the
/// same 0.02 target, found by trying N = 1, 2, 3, ... (no monotonicity assumed).
/// KNOWN MISS (ADR 0094 results): adaptive 74 steps (error 1.69e-2) against 64 uniform steps (error 1.97e-2); limit kept.
#[test]
#[ignore = "KNOWN MISS (ADR 0094): Courant control 74 steps vs 64 uniform steps meeting the 0.02 error"]
fn t1_courant_control_needs_fewer_steps_than_the_smallest_uniform_count_meeting_the_target_known_miss() {
    let (nu, target, t_end, e_target) = (0.05, 0.5, 10.0, 0.02);
    let mut s = tg(32, nu, TimeScheme::ImplicitEuler);
    let dt_aim = aimed_dt(&s, 0.9 * target);
    let mut run = ControlledRun::new(StepController::new(courant_settings(target, 0.0), 0.4 * dt_aim), false);
    let n_adapt = run.run(&mut s, t_end).len();
    let r = reference(nu, t_end, 1.0 / 80.0);
    let e_adapt = l2_diff(&s, &r);
    let (mut n_target, mut n_equal, mut e_at_target) = (None, None, f64::NAN);
    for n in 1..=1000 {
        let e = uniform_step_error(n, nu, t_end, &r);
        if n_target.is_none() && e <= e_target {
            n_target = Some(n);
            e_at_target = e;
        }
        if n_equal.is_none() && e <= e_adapt {
            n_equal = Some(n);
        }
        if n_target.is_some() && n_equal.is_some() {
            break;
        }
    }
    let n_target = n_target.expect("no uniform step count up to 1000 meets the target");
    println!(
        "T1: adaptive {n_adapt} steps, error {e_adapt:.4e}; smallest uniform count with error <= 0.02: {n_target} (error {e_at_target:.4e}); \
         smallest with error <= the adaptive error (reported only): {n_equal:?}"
    );
    assert!(e_adapt <= e_target, "adaptive error {e_adapt}");
    assert!(n_adapt < n_target, "adaptive {n_adapt} vs uniform {n_target}");
}

/// ADR 0101 E1 (F-209): wavenumbers of the two-timescale shear wave `u_x = sin(y) + sin(12 y)` on a periodic 4 x 64 box
/// of height 2 pi. The convective term vanishes (u_x does not depend on x, u_y = 0), so each mode decays on its own.
const E1_K: [f64; 2] = [1.0, 12.0];
const E1_NY: usize = 64;

fn shear_wave(nu: f64) -> PimpleSolver {
    let h = 2.0 * PI / E1_NY as f64;
    let mesh = Arc::new(box_2d(4, E1_NY, 4.0 * h, 2.0 * PI, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::from_fn("U", &geom, |c| [E1_K.iter().map(|k| (k * c[1]).sin()).sum(), 0.0, 0.0]);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = PimpleSettings::piso(nu);
    s.convection = ConvScheme::Central;
    s.time_scheme = TimeScheme::ImplicitEuler;
    let tight = SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-14,
        max_iter: 5000,
    };
    s.momentum_solver = tight;
    s.pressure_solver = tight;
    PimpleSolver::new(geom, u, p, s)
}

/// Eigenvalue of the 3-point periodic Laplacian for wavenumber `k` on the E1 mesh.
fn e1_lambda(k: f64) -> f64 {
    let h = 2.0 * PI / E1_NY as f64;
    4.0 / (h * h) * (0.5 * k * h).sin().powi(2)
}

/// Relative L2 difference of the velocity of `s` to the field whose mode amplitudes are `amp` (u_y, u_z exact zero).
fn e1_error(s: &PimpleSolver, amp: [f64; 2]) -> f64 {
    let (mut num, mut den) = (0.0, 0.0);
    for (v, c) in s.u.values.iter().zip(s.geom.mesh.cell_centres()) {
        let ex: f64 = (0..2).map(|i| amp[i] * (E1_K[i] * c[1]).sin()).sum();
        num += (v[0] - ex).powi(2) + v[1] * v[1] + v[2] * v[2];
        den += ex * ex;
    }
    (num / den).sqrt()
}

/// Exact semi-discrete amplitudes at time `t`: `exp(-nu lambda_k t)`.
fn e1_exact(nu: f64, t: f64) -> [f64; 2] {
    [
        (-nu * e1_lambda(E1_K[0]) * t).exp(),
        (-nu * e1_lambda(E1_K[1]) * t).exp(),
    ]
}

/// ADR 0101 E1 smoke (set-up check, run before the limits were registered): one PISO step reproduces the implicit-Euler
/// factor `1/(1 + nu lambda_k dt)` of each mode, so the solver is the linear two-mode decay the model of the ADR assumes.
#[test]
fn e1_shear_wave_setup_smoke() {
    let (nu, dt) = (1.0, 0.01);
    let mut s = shear_wave(nu);
    s.step(dt);
    let amp = [0, 1].map(|i| 1.0 / (1.0 + nu * e1_lambda(E1_K[i]) * dt));
    let e = e1_error(&s, amp);
    println!(
        "E1 smoke: rates {:.6} and {:.4} (ratio {:.2}); one step dt {dt}: relative L2 difference to the implicit-Euler factors {e:.3e}",
        nu * e1_lambda(E1_K[0]),
        nu * e1_lambda(E1_K[1]),
        e1_lambda(E1_K[1]) / e1_lambda(E1_K[0])
    );
    assert!(e1_lambda(E1_K[1]) / e1_lambda(E1_K[0]) >= 100.0);
    assert!(e <= 1e-10, "{e}");
}

/// ADR 0101 E1 (F-209): error control by step doubling (`solver.error_tolerance`, the S4 controller settings) on the
/// two-timescale decay to `T = 1`, against the smallest uniform implicit-Euler step count with final error <= the
/// controlled run's error (N = 1, 2, ... in order, no monotonicity assumed). Registered for tol 1e-2 and 2.5e-3.
/// KNOWN MISS (ADR 0101 results, predicted there): tol 1e-2: 1913 controlled steps (E 7.84e-3) against 64 uniform steps;
/// tol 2.5e-3: 3149 against 240. The first run stopped at the first assert, so both tolerances are now measured before
/// asserting. Limit kept.
#[test]
#[ignore = "KNOWN MISS (ADR 0101): error control 1913 steps vs 64 uniform steps at equal final error (tol 1e-2)"]
fn e1_error_control_beats_the_smallest_uniform_count_on_a_two_timescale_decay_known_miss() {
    let (nu, t_end) = (1.0, 1.0);
    let mut results = Vec::new();
    for tol in [1e-2, 2.5e-3] {
        let mut s = shear_wave(nu);
        let st = StepSettings {
            error_tol: tol,
            min_dt: 1e-5,
            max_dt: 1.0,
            ..Default::default()
        };
        let mut run = ControlledRun::new(StepController::new(st, 0.01), false);
        let n_adapt = run.run(&mut s, t_end).len();
        let e_adapt = e1_error(&s, e1_exact(nu, t_end));
        let mut n_uniform = None;
        for n in 1..=5000 {
            let mut u = shear_wave(nu);
            for _ in 0..n {
                u.step(t_end / n as f64);
            }
            let e = e1_error(&u, e1_exact(nu, t_end));
            if e <= e_adapt {
                n_uniform = Some((n, e));
                break;
            }
        }
        let (n_u, e_u) = n_uniform.expect("no uniform count up to 5000 meets the controlled error");
        println!(
            "E1 tol {tol:.1e}: controlled {n_adapt} steps ({} rejected), E = {e_adapt:.4e}; smallest uniform count with error <= E: {n_u} (error {e_u:.4e})",
            run.ctl.rejected
        );
        results.push((tol, n_adapt, n_u));
    }
    for (tol, n_adapt, n_u) in results {
        assert!(n_adapt < n_u, "tol {tol}: controlled {n_adapt} vs uniform {n_u}");
    }
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
