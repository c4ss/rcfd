//! Laminar backward-facing step (F-106, F-042 prerequisites): second-order convergence study and
//! reattachment length against Armaly et al. (1983) / Gartling (1990) as quoted in the roadmap
//! notes. Expansion ratio 2 (inlet channel height = step height), parabolic inlet with mean
//! velocity 1, Re_h = U_mean h / nu.

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::simple::{Coupling, PseudoMode, PseudoTransient, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{backward_step_graded, StepGrading, StepSpec};
use std::sync::Arc;

struct Run {
    solver: SimpleSolver,
    iterations: usize,
    converged: bool,
    res: f64,
    x_reattach: Option<f64>,
}

/// Reattachment length (x - l_in, in step heights) from the sign change of the streamwise
/// velocity in the first cell row above the floor (last negative-to-positive crossing downstream
/// of the step, linear interpolation).
fn reattachment(s: &SimpleSolver, l_in: f64, step: f64) -> Option<f64> {
    let cc = s.geom.mesh.cell_centres();
    let mut row: Vec<(f64, f64)> = Vec::new();
    let ymin = cc.iter().map(|c| c[1]).fold(f64::INFINITY, f64::min);
    for (c, p) in cc.iter().enumerate() {
        if (p[1] - ymin).abs() < 1e-9 && p[0] > l_in {
            row.push((p[0], s.u.values[c][0]));
        }
    }
    row.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut out = None;
    for w in row.windows(2) {
        if w[0].1 < 0.0 && w[1].1 >= 0.0 {
            let t = -w[0].1 / (w[1].1 - w[0].1);
            out = Some(w[0].0 + t * (w[1].0 - w[0].0) - l_in);
        }
    }
    out.map(|x| x / step)
}

fn laminar_step(
    re_h: f64,
    spec: StepSpec,
    grading: StepGrading,
    scheme: ConvScheme,
    coupling: Coupling,
    alphas: Option<(f64, f64)>,
    h_from_solution: bool,
    pseudo: Option<PseudoTransient>,
    max_iter: usize,
    tol: f64,
) -> Run {
    let geom = Arc::new(FvGeometry::new(Arc::new(backward_step_graded(spec, grading).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let patch = geom.mesh.patch("inlet").unwrap().clone();
    let cf = geom.mesh.face_centres();
    let prof: Vec<[f64; 3]> = patch
        .faces()
        .map(|f| {
            let s = (cf[f][1] - spec.step) / spec.h_in;
            [6.0 * s * (1.0 - s), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(spec.step / re_h, coupling);
    if let Some((au, ap)) = alphas {
        st.alpha_u = au;
        st.alpha_p = ap;
    }
    st.convection = scheme;
    st.h_from_solution = h_from_solution;
    let mut solver = SimpleSolver::new(geom, u, p, st);
    solver.pseudo = pseudo;
    let mut res = f64::INFINITY;
    let mut iterations = max_iter;
    let mut converged = false;
    for i in 1..=max_iter {
        let info = solver.iterate();
        res = info.res_u.max(info.res_p);
        let finite = solver.u.values.iter().all(|v| v.iter().all(|x| x.is_finite()));
        if !finite || !res.is_finite() || res > 1e8 {
            res = f64::INFINITY;
            iterations = i;
            break;
        }
        if res < tol {
            iterations = i;
            converged = true;
            break;
        }
    }
    let x_reattach = reattachment(&solver, spec.l_in, spec.step);
    Run {
        solver,
        iterations,
        converged,
        res,
        x_reattach,
    }
}

fn spec_er2(l_out: f64, n: [usize; 4]) -> StepSpec {
    StepSpec {
        l_in: 2.0,
        l_out,
        h_in: 1.0,
        step: 1.0,
        depth: 0.1,
        n,
    }
}

fn env<T: std::str::FromStr>(k: &str, d: T) -> T {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

/// Exploration entry: parameters from the environment, residual histories printed.
#[test]
#[ignore = "exploration"]
fn explore_laminar_step() {
    let re: f64 = env("RE", 200.0);
    let sch = match env::<String>("SCH", "linup".into()).as_str() {
        "up" => ConvScheme::Upwind,
        "central" => ConvScheme::Central,
        "linup" => ConvScheme::LinearUpwind,
        _ => ConvScheme::Tvd(TvdLimiter::VanLeer),
    };
    let cpl = if env::<String>("CPL", "simple".into()) == "simplec" {
        Coupling::Simplec
    } else {
        Coupling::Simple
    };
    let n = [env("NXU", 8), env("NXD", 120), env("NYI", 16), env("NYS", 16)];
    let spec = spec_er2(env("LOUT", 30.0), n);
    let gr = if env::<f64>("RX", 1.0) > 1.0 {
        StepGrading::clustered(env("RX", 1.0), env("RY", 1.0))
    } else {
        StepGrading::uniform()
    };
    let alphas = match (env::<f64>("AU", 0.0), env::<f64>("AP", 0.0)) {
        (a, b) if a > 0.0 && b > 0.0 => Some((a, b)),
        _ => None,
    };
    let hs = env::<String>("HSOL", "1".into()) == "1";
    let nit: usize = env("NIT", 4000);
    let cfl: f64 = env("CFL", 0.0);
    let pseudo = (cfl > 0.0).then_some(PseudoTransient {
        mode: PseudoMode::Local { cfl },
        factor: 1.0,
    });
    let r = laminar_step(re, spec, gr, sch, cpl, alphas, hs, pseudo, nit, env("TOL", 1e-7));
    for h in r.solver.history.iter().step_by(env("EVERY", 250)) {
        println!(
            "it {:5} res_u {:.3e} res_p {:.3e} du {:.3e}",
            h.iteration, h.res_u, h.res_p, h.max_du
        );
    }
    println!(
        "RESULT Re_h {re} its {} converged {} res {:.2e} xr/h {:?}",
        r.iterations, r.converged, r.res, r.x_reattach
    );
}

fn vl() -> ConvScheme {
    ConvScheme::Tvd(TvdLimiter::VanLeer)
}

/// Regression for the wave-6 diagnosis (F-106): on this step mesh at Re_h = 200 the legacy
/// previous-iterate H operator never converges with second-order convection (a wave of order
/// one grows in the downstream channel; with the uniform inlet the residual stalls near 0.3, with the
/// parabolic inlet used here it diverges to non-finite fields), while H from the solved
/// momentum predictor (the default) converges, for SIMPLE and SIMPLEC.
#[test]
fn second_order_step_re200_needs_h_from_solution() {
    let spec = spec_er2(20.0, [8, 80, 8, 8]);
    let g = StepGrading::uniform();
    let legacy = laminar_step(
        200.0,
        spec,
        g,
        ConvScheme::LinearUpwind,
        Coupling::Simple,
        None,
        false,
        None,
        1500,
        1e-6,
    );
    println!("legacy lagged H: converged {} res {:.2e}", legacy.converged, legacy.res);
    assert!(!legacy.converged);
    for (name, sch, cpl) in [
        ("linearUpwind SIMPLE", ConvScheme::LinearUpwind, Coupling::Simple),
        ("vanLeer SIMPLEC", vl(), Coupling::Simplec),
    ] {
        let r = laminar_step(200.0, spec, g, sch, cpl, None, true, None, 3000, 1e-6);
        println!("{name}: {} its xr/h {:?}", r.iterations, r.x_reattach);
        assert!(r.converged, "{name}: res {:.2e}", r.res);
        assert!(r.x_reattach.unwrap() > 6.0);
    }
}

/// Reattachment length grows with Reynolds number and agrees between uniform and graded
/// (clustered toward the walls and the step) meshes of the same size to a few percent.
#[test]
fn step_reattachment_grows_with_re_and_is_grading_insensitive() {
    let spec = spec_er2(20.0, [8, 80, 12, 12]);
    let mut xr = vec![];
    for re in [50.0, 100.0, 150.0] {
        let r = laminar_step(
            re,
            spec,
            StepGrading::uniform(),
            vl(),
            Coupling::Simplec,
            None,
            true,
            None,
            3000,
            1e-6,
        );
        assert!(r.converged, "Re {re}: res {:.2e}", r.res);
        xr.push(r.x_reattach.unwrap());
    }
    println!("xr/h at Re_h 50/100/150: {xr:?}");
    assert!(xr[0] < xr[1] && xr[1] < xr[2]);
    let gr = laminar_step(
        100.0,
        spec,
        StepGrading::clustered(2.0, 1.5),
        vl(),
        Coupling::Simplec,
        None,
        true,
        None,
        3000,
        1e-6,
    );
    assert!(gr.converged);
    let rel = (gr.x_reattach.unwrap() / xr[1] - 1.0).abs();
    println!("graded xr/h {:?} vs uniform {}", gr.x_reattach, xr[1]);
    assert!(rel < 0.06, "{rel}");
}

/// Print-only sweep point kept from the wave-6 exploration (Re_h = 389; note Armaly's own "389" would be Re_h about
/// 195). It has no reference value and asserts nothing.
#[test]
#[ignore = "heavy validation sweep, print only"]
fn laminar_step_validation_sweep() {
    let r = laminar_step(
        389.0,
        spec_er2(40.0, [16, 320, 32, 32]),
        StepGrading::uniform(),
        vl(),
        Coupling::Simplec,
        None,
        true,
        None,
        8000,
        1e-7,
    );
    println!(
        "SWEEP Re_h 389 (Re_Armaly 778): its {} converged {} xr/h {:?}",
        r.iterations, r.converged, r.x_reattach
    );
}

// ---------------------------------------------------------------------------------------------------------------
// Registered reattachment criteria (audit follow-up: the sweep had no assertion). Fixed before the assertions below
// were first run; a miss is moved, unchanged, into an #[ignore]d `*_known_miss` test.
//
// Set-up of all runs: expansion ratio 2 (inlet channel height = step height h), parabolic inlet of mean velocity 1,
// Re_h = U_mean h / nu, which is half of the Armaly/Gartling/Erturk Reynolds number (Re_D = U_mean 2h / nu, the
// hydraulic diameter of the inlet channel); mesh [16, 240, 32, 32] cells (l_out = 30 h; 40 h and [16, 320, 32, 32]
// for Re_h = 400), van Leer, SIMPLEC, h from the solved predictor, converged to 1e-7 (the sweep set-up of F-106,
// unchanged). Reattachment: last negative-to-positive crossing of u in the first cell row above the floor.
//
// (S1) Re_h = 400 (Re = 800): x_r/h within 5 % of Erturk (2008) 11.834, i.e. [11.242, 12.426].
// (S2) Re_h = 400: x_r/h within 5 % of Gartling (1990) 12.20, i.e. [11.590, 12.810].
//      S1/S2 values are as quoted in Table 2 of arXiv:2507.16509 (a secondary source, retrieved 2026-10-14; the
//      original papers were not accessed); Armaly et al. (1983) give no table accessible here (marked unverified in
//      docs/reference-data.md). The 5 % window is the literature spread (Erturk and Gartling differ by 3.0 %) plus
//      the discretisation allowance; it was not tuned to the measured values, but the F-106 note (11.24 / 11.67 /
//      11.75 on three meshes at Re_h = 400) was known when it was chosen, so S1/S2 are not blind.
// (S3) Re_h = 100 (Re = 200): x_r/h within 5 % of 4.95; (S4) Re_h = 200 (Re = 400): within 5 % of 8.10. These two
//      references are the finite-volume results of the same secondary source's Table 1 (ER 2, 2-D), NOT a published
//      benchmark; they check consistency with an independent code only. Marked "secondary, unverified".
// (S5) every run converges (residual below 1e-7 within 8000 iterations).
// ---------------------------------------------------------------------------------------------------------------

fn sweep_point(re_h: f64) -> Run {
    let (l_out, n) = if re_h >= 300.0 {
        (40.0, [16, 320, 32, 32])
    } else {
        (30.0, [16, 240, 32, 32])
    };
    let r = laminar_step(
        re_h,
        spec_er2(l_out, n),
        StepGrading::uniform(),
        vl(),
        Coupling::Simplec,
        None,
        true,
        None,
        8000,
        1e-7,
    );
    println!(
        "REGISTERED Re_h {re_h}: its {} converged {} res {:.2e} xr/h {:?}",
        r.iterations, r.converged, r.res, r.x_reattach
    );
    assert!(r.converged, "S5: Re_h {re_h} did not converge (res {:.2e})", r.res);
    r
}

fn within(x: f64, reference: f64, rel: f64) -> bool {
    (x / reference - 1.0).abs() <= rel
}

#[test]
#[ignore = "heavy validation (minutes, release)"]
fn step_reattachment_re_h_100_vs_secondary_fv() {
    let x = sweep_point(100.0).x_reattach.expect("no reattachment");
    assert!(within(x, 4.95, 0.05), "S3: x_r/h = {x} vs 4.95 (secondary FV), limit 5 %");
}

#[test]
#[ignore = "heavy validation (minutes, release)"]
fn step_reattachment_re_h_200_vs_secondary_fv() {
    let x = sweep_point(200.0).x_reattach.expect("no reattachment");
    assert!(within(x, 8.10, 0.05), "S4: x_r/h = {x} vs 8.10 (secondary FV), limit 5 %");
}

#[test]
#[ignore = "heavy validation (minutes, release)"]
fn step_reattachment_re_h_400_vs_erturk_and_gartling() {
    let x = sweep_point(400.0).x_reattach.expect("no reattachment");
    assert!(within(x, 11.834, 0.05), "S1: x_r/h = {x} vs Erturk 2008 11.834, limit 5 %");
    assert!(within(x, 12.20, 0.05), "S2: x_r/h = {x} vs Gartling 1990 12.20, limit 5 %");
}
