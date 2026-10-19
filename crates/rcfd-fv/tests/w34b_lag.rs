//! ADR 0073 (wave 34B) Part B: the one-corrector pressure-lag instability of PIMPLE. B2 (model against the solver on a periodic Stokes box),
//! B3 (new cavity predictions of the model), B4 (the run-time check `PimpleSettings::pressure_lag_check`). The cavity harness is the registered one of
//! ADR 0069 (`w33b_stability.rs`), copied with the first-violation step and the warnings recorded.
//! Heavy tests are ignored: `cargo test --release -p rcfd-fv --test w34b_lag -- --ignored --nocapture`.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{LagAction, LagCheck, PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_2d_graded, cavity_2d, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const EULER: TimeScheme = TimeScheme::ImplicitEuler;
const BDF2: TimeScheme = TimeScheme::Bdf2;
const BOTH: [TimeScheme; 2] = [EULER, BDF2];

fn c0_of(scheme: TimeScheme) -> f64 {
    if scheme == BDF2 {
        1.5
    } else {
        1.0
    }
}

/// Model of ADR 0073 R5 for one Fourier mode: full symbol `lam`, diagonal part `lam_d`, both per volume.
fn model_g(lam: f64, lam_d: f64, dt: f64, c0: f64, n_outer: usize, alpha_u: f64, alpha_p: f64) -> f64 {
    let kappa = (lam - lam_d) / (c0 / dt + lam);
    let kappa_r = (lam - lam_d) / ((c0 / dt + lam_d) / alpha_u + lam - lam_d);
    kappa * ((1.0 - alpha_p) + alpha_p * kappa_r).powi(n_outer as i32 - 1)
}

// ---- B2: periodic Stokes box -----------------------------------------------------------------------------------------------------
struct BoxRun {
    model: f64,
    g: f64,
    rho: f64,
}

/// `est_step`: the step whose check estimate is returned (the check runs at step 1 and every `est_step` steps).
fn box_run(d: f64, scheme: TimeScheme, n_outer: usize, est_step: usize) -> BoxRun {
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
    st.pressure_lag_check = Some(LagCheck::new(if est_step > 1 { est_step } else { 0 }, LagAction::Warn));
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
        assert!(s.diverged.is_none());
        norms.push(norm(&s.p.values));
    }
    let g = (norms[11] / norms[5]).powf(1.0 / 6.0);
    let lam_d = 4.0 * nu / (h * h);
    let lam = nu * 4.0 / (h * h) * 2.0 * (PI * h).sin().powi(2);
    let model = model_g(lam, lam_d, dt, c0, n_outer, 0.8, 0.5);
    BoxRun {
        model,
        g,
        rho: s.lag_estimates.iter().find(|w| w.step == est_step).unwrap().rho,
    }
}

/// Registered B2/B4a with the step-1 estimate (KNOWN MISS: set-up defect for BDF2, whose step 1 is an Euler step; ADR 0073 amendment 2).
#[test]
#[ignore = "known miss of ADR 0073 B4a (set-up defect for the BDF2 rows, amendment 2); limits unchanged"]
fn b2_b4a_periodic_stokes_box_known_miss() {
    b2_b4a(1);
}

/// Corrected B2/B4a (ADR 0073 amendment 2): the check estimate made at step 6, same limits.
#[test]
fn b2_b4a_step6_corrected() {
    b2_b4a(6);
}

fn b2_b4a(est_step: usize) {
    let mut bad = Vec::new();
    for scheme in BOTH {
        for n_outer in [2usize, 4] {
            for d in [1.0, 2.0, 3.0, 4.0, 6.0] {
                let r = box_run(d, scheme, n_outer, est_step);
                let gm = r.model.abs();
                println!(
                    "B2 {scheme:?} n_outer={n_outer} D={d}: model G {:.4} (|G| {gm:.4}), measured g {:.4}, check rho (step {est_step}) {:.4}",
                    r.model, r.g, r.rho
                );
                if gm >= 1.15 {
                    if !(r.g > 1.0 && (r.g - gm).abs() <= 0.10 * gm) {
                        bad.push(format!("B2a {scheme:?} {n_outer} D={d}: g {} vs |G| {gm}", r.g));
                    }
                    if (r.rho - r.g).abs() > 0.10 * r.g {
                        bad.push(format!("B4a {scheme:?} {n_outer} D={d}: rho {} vs g {}", r.rho, r.g));
                    }
                } else if gm <= 0.85 {
                    if r.g >= 1.0 {
                        bad.push(format!("B2b {scheme:?} {n_outer} D={d}: g {} >= 1", r.g));
                    }
                    if r.rho >= 1.0 {
                        bad.push(format!("B4a {scheme:?} {n_outer} D={d}: rho {} >= 1", r.rho));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// ---- cavity harness (ADR 0069, copied) --------------------------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mesh {
    Uniform,
    Graded,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Alg {
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
        Alg::Pimple2 => PimpleSettings::pimple(nu, 2),
        Alg::Pimple4 => PimpleSettings::pimple(nu, 4),
    };
    st.convection = ConvScheme::LinearUpwind;
    st.time_scheme = scheme;
    tweak(&mut st);
    (PimpleSolver::new(geom, u, p, st), h_min)
}

#[derive(Debug)]
struct Outcome {
    stable: bool,
    why: String,
    first_bad_step: Option<usize>,
    first_warn_step: Option<usize>,
    rho_first: f64,
    rho_max: f64,
    steps: usize,
    raised: Option<String>,
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
        first_bad_step: None,
        first_warn_step: None,
        rho_first: f64::NAN,
        rho_max: f64::NAN,
        steps: 0,
        raised: None,
    };
    fn fail(o: &mut Outcome, why: String) {
        if o.stable {
            o.stable = false;
            o.why = why;
            o.first_bad_step = Some(o.steps);
        }
    }
    while s.time < T_END - 1e-12 {
        let d = dt.min(T_END - s.time);
        let info = s.step(d);
        o.steps += 1;
        if let Some(dv) = &s.diverged {
            if dv.quantity == "lag amplification" {
                o.raised = Some(dv.message(Some(s.geom.mesh.cell_centres())));
                let ok =
                    s.u.values
                        .iter()
                        .all(|v| v.iter().all(|x| x.is_finite()) && (v[0] * v[0] + v[1] * v[1]).sqrt() <= 1.5)
                        && s.p.values.iter().all(|x| x.is_finite());
                if !ok {
                    fail(&mut o, "state at the raise not finite or max|u| > 1.5".into());
                }
                break;
            }
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
    if o.stable && o.raised.is_none() && s.time < T_END - 1e-9 {
        fail(&mut o, "t_end not reached".into());
    }
    o.first_warn_step = s.lag_warnings.first().map(|w| w.step);
    o.rho_first = s.lag_estimates.first().map_or(f64::NAN, |w| w.rho);
    o.rho_max = s.lag_estimates.iter().map(|w| w.rho).fold(f64::NAN, f64::max);
    o
}

fn warn_every(n: usize) -> impl Fn(&mut PimpleSettings) {
    move |st: &mut PimpleSettings| st.pressure_lag_check = Some(LagCheck::new(n, LagAction::Warn))
}

// ---- B3: new cavity predictions ---------------------------------------------------------------------------------------------------
#[test]
#[ignore = "ADR 0073 B3 (heavy, limits)"]
fn b3_new_cavity_predictions() {
    let cases = [
        (Alg::Pimple2, EULER, 2.3, true),
        (Alg::Pimple2, EULER, 3.0, false),
        (Alg::Pimple2, BDF2, 3.4, true),
        (Alg::Pimple2, BDF2, 4.6, false),
        (Alg::Pimple4, EULER, 5.0, false),
        (Alg::Pimple4, BDF2, 6.0, true),
        (Alg::Pimple4, BDF2, 7.5, false),
    ];
    let mut bad = Vec::new();
    for (alg, scheme, co, predicted_stable) in cases {
        let o = run_case(Mesh::Uniform, 100.0, alg, scheme, co, &|_| {});
        println!(
            "B3 Uniform Re=100 {alg:?} {scheme:?} Co_t={co}: {} (predicted {}){}",
            if o.stable { "STABLE" } else { "UNSTABLE" },
            if predicted_stable { "stable" } else { "unstable" },
            if o.why.is_empty() {
                String::new()
            } else {
                format!(" [{}]", o.why)
            }
        );
        if o.stable != predicted_stable {
            bad.push(format!("{alg:?} {scheme:?} Co_t={co}"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// ---- B4 ---------------------------------------------------------------------------------------------------------------------------
fn b4b_set(mesh: Mesh) -> (Vec<String>, usize) {
    let mut misses = Vec::new();
    let mut false_pos = 0;
    for alg in [Alg::Pimple2, Alg::Pimple4] {
        for re in [100.0, 1000.0] {
            for scheme in BOTH {
                for co in [0.5, 1.0, 2.0, 4.0, 8.0, 16.0] {
                    let o = run_case(mesh, re, alg, scheme, co, &warn_every(20));
                    println!(
                        "B4b {mesh:?} Re={re} {alg:?} {scheme:?} Co_t={co}: {} first violation step {:?}, first warning step {:?}, rho(step 1) {:.4}, rho max {:.4}{}",
                        if o.stable { "STABLE" } else { "UNSTABLE" },
                        o.first_bad_step,
                        o.first_warn_step,
                        o.rho_first,
                        o.rho_max,
                        if o.why.is_empty() { String::new() } else { format!(" [{}]", o.why) }
                    );
                    match (o.stable, o.first_bad_step, o.first_warn_step) {
                        (false, Some(b), Some(w)) if w <= b => {}
                        (false, _, _) => misses.push(format!(
                            "{mesh:?} Re={re} {alg:?} {scheme:?} Co_t={co}: not warned in time"
                        )),
                        (true, _, Some(_)) => false_pos += 1,
                        _ => {}
                    }
                }
            }
        }
    }
    (misses, false_pos)
}

/// KNOWN MISS (ADR 0073 addendum): the two uniform Re = 1000 PIMPLE-2 runs at Co_t = 16 diverge without a warning (their instability is not the
/// pressure lag, rho <= 0.33), and graded Re = 100 PIMPLE-4 BDF2 at Co_t = 8 is warned although stable; limits unchanged.
#[test]
#[ignore = "known miss of ADR 0073 B4b (2 unwarned Re = 1000 runs; heavy); limits unchanged"]
fn b4b_cavity_warnings_known_miss() {
    let (mut misses, mut fp) = b4b_set(Mesh::Uniform);
    let (m2, f2) = b4b_set(Mesh::Graded);
    misses.extend(m2);
    fp += f2;
    println!(
        "B4b: {} unstable runs not warned in time, {fp} stable runs warned (limit 0 and <= 4)",
        misses.len()
    );
    assert!(misses.is_empty() && fp <= 4, "{misses:#?}, false positives {fp}");
}

#[test]
fn b4c_two_correctors_recognised() {
    let mut bad = Vec::new();
    for mesh in [Mesh::Uniform, Mesh::Graded] {
        for scheme in BOTH {
            let (mut s, h_min) = setup(mesh, 0.01, Alg::Pimple2, scheme, &|st| {
                st.n_correctors = 2;
                st.pressure_lag_check = Some(LagCheck::new(0, LagAction::Warn));
            });
            s.step(4.0 * h_min);
            let rho = s.lag_estimates[0].rho;
            let (mut s1, _) = setup(mesh, 0.01, Alg::Pimple2, scheme, &warn_every(0));
            s1.step(4.0 * h_min);
            println!(
                "B4c {mesh:?} {scheme:?} Co_t=4 Re=100: rho with 2 correctors {rho:.4} (limit < 1); with 1 corrector {:.4}",
                s1.lag_estimates[0].rho
            );
            if rho >= 1.0 {
                bad.push(format!("{mesh:?} {scheme:?}: {rho}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn b4d_raise_stops_before_divergence() {
    let o = run_case(Mesh::Uniform, 100.0, Alg::Pimple2, EULER, 4.0, &|st| {
        st.pressure_lag_check = Some(LagCheck::new(20, LagAction::Raise))
    });
    let msg = o.raised.clone().unwrap_or_default();
    println!("B4d: stopped at step {} with: {msg}", o.steps);
    assert_eq!(o.steps, 1, "{o:?}");
    assert!(
        msg.contains("pressure-lag check") && msg.contains("n_correctors >= 2") && msg.contains("cell"),
        "{msg}"
    );
    assert!(o.stable, "{o:?}");
}
