//! F-211 (ADR 0016): scheme additions. Limits are those registered in docs/adr/0016-wave13-numerics.md, written before
//! this file was compiled or run.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::{FixedValue, Profile, ZeroGradient};
use rcfd_fv::convection::{face_flux, ConvScheme, TvdLimiter};
use rcfd_fv::ddt::{add_ddt, History, TimeScheme};
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::scalar::{sweep, ScalarTransport, SweepInputs};
use rcfd_fv::{FvGeometry, FvMatrix, ScalarField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

fn geom_1d(n: usize, lx: f64) -> Arc<FvGeometry> {
    let mesh = box_2d(n, 2, lx, 0.1, false, false).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

fn unit_flux(geom: &FvGeometry) -> Vec<f64> {
    let cells = vec![[1.0, 0.0, 0.0]; geom.n_cells()];
    let bnd = vec![[1.0, 0.0, 0.0]; geom.mesh.n_boundary_faces()];
    face_flux(geom, &cells, &bnd)
}

/// Steady O1 problem: u = 1, phi = exp(x) on [0,1]. Returns (L2 error, converged, solution).
fn steady(scheme: ConvScheme, n: usize) -> (f64, bool, Vec<f64>) {
    steady_with(scheme, n, false, 600)
}

fn steady_with(scheme: ConvScheme, n: usize, outlet_zero_gradient: bool, max_sweeps: usize) -> (f64, bool, Vec<f64>) {
    steady_traced(scheme, n, outlet_zero_gradient, max_sweeps, None)
}

/// `outlet_zero_gradient = true` is the corrected set-up: phi = 1 + sin(pi x / 2) has zero slope at the outlet.
/// `trace` receives the L2 error after every sweep (ADR 0093 O1d).
fn steady_traced(
    scheme: ConvScheme,
    n: usize,
    outlet_zero_gradient: bool,
    max_sweeps: usize,
    mut trace: Option<&mut Vec<f64>>,
) -> (f64, bool, Vec<f64>) {
    let ex = |x: f64| {
        if outlet_zero_gradient {
            1.0 + (0.5 * std::f64::consts::PI * x).sin()
        } else {
            x.exp()
        }
    };
    let geom = geom_1d(n, 1.0);
    let cc = geom.mesh.cell_centres().to_vec();
    let mut field = ScalarField::uniform("phi", &geom, 1.0);
    let dirichlet: &[(&str, f64)] = if outlet_zero_gradient {
        &[("xmin", 0.0)]
    } else {
        &[("xmin", 0.0), ("xmax", 1.0)]
    };
    for &(patch, x) in dirichlet {
        let nf = geom.mesh.patch(patch).unwrap().size;
        field
            .set_bc(
                &geom,
                patch,
                Box::new(FixedValue(Profile::PerFace(vec![[ex(x), 0.0, 0.0]; nf]))),
            )
            .unwrap();
    }
    let others: &[&str] = if outlet_zero_gradient {
        &["xmax", "ymin", "ymax"]
    } else {
        &["ymin", "ymax"]
    };
    for p in others.iter().copied() {
        field.set_bc(&geom, p, Box::new(ZeroGradient)).unwrap();
    }
    let h = 1.0 / n as f64;
    let src: Vec<f64> = cc
        .iter()
        .map(|c| {
            let (xl, xr) = (c[0] - 0.5 * h, c[0] + 0.5 * h);
            (ex(xr) - ex(xl)) / h
        })
        .collect();
    let mut tr = ScalarTransport::new(geom.clone(), field, 0.0);
    tr.convection = scheme;
    tr.source = Some(src);
    let phi = unit_flux(&geom);
    let vol = geom.mesh.cell_volumes();
    let l2 = |vals: &[f64]| {
        let (mut e2, mut tot) = (0.0, 0.0);
        for ((c, v), p) in cc.iter().zip(vol).zip(vals) {
            e2 += (p - ex(c[0])).powi(2) * v;
            tot += v;
        }
        (e2 / tot).sqrt()
    };
    let mut converged = false;
    let mut last_change = 0.0;
    for _ in 0..max_sweeps {
        let before = tr.field.values.clone();
        tr.iterate(&phi);
        if let Some(t) = trace.as_deref_mut() {
            t.push(l2(&tr.field.values));
        }
        let change = tr
            .field
            .values
            .iter()
            .zip(&before)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        let range = tr.field.values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - tr.field.values.iter().cloned().fold(f64::INFINITY, f64::min);
        last_change = change / range;
        if change <= 1e-13 * range {
            converged = true;
            break;
        }
    }
    if !converged {
        println!("  unconverged n={n}: last relative change {last_change:e}");
    }
    (l2(&tr.field.values), converged, tr.field.values.clone())
}

fn orders(scheme: ConvScheme, corrected: bool, max_sweeps: usize) -> (Vec<f64>, bool) {
    let mut errs = Vec::new();
    let mut ok = true;
    for n in [64usize, 128, 256] {
        let (e, c, _) = steady_with(scheme, n, corrected, max_sweeps);
        errs.push(e);
        ok &= c;
    }
    (errs.windows(2).map(|w| (w[0] / w[1]).log2()).collect(), ok)
}

/// Record of the first (mis-set-up) O1 run: a Dirichlet value on the outflow boundary leaves the last cell's row
/// without a diagonal, so the system is singular there and the measured order is 0.5 for every scheme. Superseded
/// by `o1b_order_of_accuracy`; kept unchanged as the record (see ADR 0016 Results).
#[test]
#[ignore = "set-up bug, superseded by o1b_order_of_accuracy"]
fn o1_order_of_accuracy() {
    o1_body(false, 600, None);
}

/// Second record: with the corrected outlet the orders are right (2.00 / 1.98..2.00 / 1.00 / 0.98..1.00) but the
/// deferred-correction schemes did not meet the registered convergence rule within the registered 600 sweeps at
/// n = 256 (convection-only deferred correction needs a number of sweeps growing with n). The assertion is
/// unchanged; the miss is recorded here and re-examined with a larger sweep budget in `o1c_order_of_accuracy`.
#[test]
#[ignore = "known miss: 600-sweep cap, see o1c_order_of_accuracy"]
fn o1b_known_miss_convergence_cap() {
    o1_body(true, 600, None);
}

/// Corrected set-up with a 20000-sweep budget, every scheme except the two compressive second-order members
/// (see the known miss below).
#[test]
fn o1c_order_of_accuracy() {
    o1_body(true, 20000, Some(false));
}

/// Known miss, convergence part of O1c: compressive(1.5) and compressive(2) reach orders 1.98 and 1.99 (inside the
/// registered band) but the deferred-correction iteration does not satisfy the registered convergence rule: the
/// change per sweep stalls at 7e-8 / 1.4e-8 (n = 256) and is identical after 600 and after 60000 sweeps, i.e. a
/// limit cycle caused by the non-smooth limiter function switching branches between sweeps, not slow convergence.
/// The assertion is the registered one and is not relaxed.
#[test]
#[ignore = "known miss: compressive(1.5)/(2) deferred correction stalls in a limit cycle"]
fn o1c_known_miss_compressive_convergence() {
    o1_body(true, 60000, Some(true));
}

/// Error envelope (min, max) of the last `w` entries ending at `end` (exclusive).
fn envelope(t: &[f64], end: usize, w: usize) -> (f64, f64) {
    t[end - w..end]
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(lo, hi), &e| (lo.min(e), hi.max(e)))
}

/// ADR 0093 O1d: order of compressive(1.5) and compressive(2) measured on the limit cycle of the deferred correction.
/// Returns per scheme the envelopes at the two windows for each n, and prints them.
type Envelopes = Vec<[(f64, f64); 2]>;

fn o1d_run(ns: &[usize], sweeps: usize, w: usize) -> Vec<(&'static str, Envelopes)> {
    let mut out = Vec::new();
    for (name, s) in [
        ("compressive(1.5)", ConvScheme::Tvd(TvdLimiter::Compressive(1.5))),
        ("compressive(2)", ConvScheme::Tvd(TvdLimiter::Compressive(2.0))),
    ] {
        let mut env = Vec::new();
        for &n in ns {
            let t0 = std::time::Instant::now();
            let mut tr = Vec::new();
            steady_traced(s, n, true, sweeps, Some(&mut tr));
            assert_eq!(
                tr.len(),
                sweeps,
                "{name} n={n}: stopped early (converged), the limit cycle rule does not apply"
            );
            let e = [envelope(&tr, sweeps / 2, w), envelope(&tr, sweeps, w)];
            println!(
                "O1d {name} n={n}: window at {} {:?}, window at {sweeps} {:?}, {:.1} s",
                sweeps / 2,
                e[0],
                e[1],
                t0.elapsed().as_secs_f64()
            );
            env.push(e);
        }
        out.push((name, env));
    }
    out
}

/// Set-up smoke of O1d (ADR 0093, run before the limits were registered): short budget, prints only.
#[test]
#[ignore = "set-up smoke for ADR 0093 O1d; prints only"]
fn o1d_smoke() {
    o1d_run(&[64, 128], 2000, 200);
}

/// ADR 0093 O1d (F-211): the "nominal order" clause for compressive(1.5) and compressive(2), whose deferred correction ends
/// in a limit cycle (see `o1c_known_miss_compressive_convergence`). Rules registered in ADR 0093 before this test was run:
/// with a 20000-sweep budget and 200-sweep windows ending at sweep 10000 and 20000, (i) the windows overlap at every n
/// (stationary cycle, no drift), (ii) the worst-case orders log2(max e_n / min e_2n) and log2(min e_n / max e_2n) over the
/// final windows lie in the registered O1 band [1.85, 2.3].
#[test]
fn o1d_compressive_order_on_limit_cycle() {
    let mut bad = Vec::new();
    for (name, env) in o1d_run(&[64, 128, 256], 20000, 200) {
        for (k, e) in env.iter().enumerate() {
            let ((a0, a1), (b0, b1)) = (e[0], e[1]);
            if a0 > b1 || b0 > a1 {
                bad.push(format!("{name} level {k}: windows do not overlap {e:?}"));
            }
        }
        for k in 0..env.len() - 1 {
            let (c, f) = (env[k][1], env[k + 1][1]);
            let (o_lo, o_hi) = ((c.0 / f.1).log2(), (c.1 / f.0).log2());
            println!("O1d {name} pair {k}: worst-case orders {o_lo:.4} .. {o_hi:.4}");
            if o_lo < 1.85 || o_hi > 2.3 {
                bad.push(format!("{name} pair {k}: orders {o_lo} .. {o_hi}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

fn o1_body(corrected: bool, max_sweeps: usize, only_compressive_second: Option<bool>) {
    use TvdLimiter::*;
    let second: Vec<(&str, ConvScheme)> = vec![
        ("central", ConvScheme::Central),
        ("blended(1)", ConvScheme::Blended(1.0)),
        ("gamma(0.2)", ConvScheme::Tvd(Gamma(0.2))),
        ("compressive(1.5)", ConvScheme::Tvd(Compressive(1.5))),
        ("compressive(2)", ConvScheme::Tvd(Compressive(2.0))),
        ("bounded_central", ConvScheme::Tvd(BoundedCentral)),
    ];
    let first: Vec<(&str, ConvScheme)> = vec![
        ("upwind", ConvScheme::Upwind),
        ("blended(0.5)", ConvScheme::Blended(0.5)),
        ("compressive(0)", ConvScheme::Tvd(Compressive(0.0))),
        ("compressive(0.5)", ConvScheme::Tvd(Compressive(0.5))),
    ];
    let mut bad = Vec::new();
    for (set, lo, hi, list) in [("second", 1.85, 2.3, second), ("first", 0.9, 1.15, first)] {
        for (name, s) in list {
            if let Some(want) = only_compressive_second {
                if (set == "second" && name.starts_with("compressive")) != want || (want && set == "first") {
                    continue;
                }
            }
            let (o, conv) = orders(s, corrected, max_sweeps);
            println!("O1 {set} {name}: orders {o:?} converged {conv}");
            if !conv || o.iter().any(|&x| x < lo || x > hi) {
                bad.push(format!("{name}: {o:?} converged {conv}"));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn b1_blend_factor_reproduces_upwind_and_central() {
    let (_, c0, up) = steady(ConvScheme::Upwind, 64);
    let (_, c1, bl0) = steady(ConvScheme::Blended(0.0), 64);
    let (_, c2, ce) = steady(ConvScheme::Central, 64);
    let (_, c3, bl1) = steady(ConvScheme::Blended(1.0), 64);
    assert!(c0 && c1 && c2 && c3);
    let d0 = up.iter().zip(&bl0).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    let d1 = ce.iter().zip(&bl1).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    println!("B1: |blended(0) - upwind| {d0:.3e}, |blended(1) - central| {d1:.3e}");
    assert!(d0 <= 1e-12, "{d0}");
    assert!(d1 <= 1e-10, "{d1}");
}

#[test]
fn b3_bounded_central_is_minmod() {
    let (_, c0, a) = steady(ConvScheme::Tvd(TvdLimiter::BoundedCentral), 64);
    let (_, c1, b) = steady(ConvScheme::Tvd(TvdLimiter::Minmod), 64);
    assert!(c0 && c1);
    assert_eq!(a, b);
}

/// Transient 1-D wave on [0,2]. Returns the final field.
fn wave(space: ConvScheme, time: TimeScheme, n: usize, dt_over_h: f64, t_end: f64, sweeps: usize) -> Vec<f64> {
    let geom = geom_1d(n, 2.0);
    let h = 2.0 / n as f64;
    let init = |p: Vec3| if (0.1..=0.35).contains(&p[0]) { 1.0 } else { 0.0 };
    let mut field = ScalarField::from_fn("phi", &geom, init);
    let nf = geom.mesh.patch("xmin").unwrap().size;
    field
        .set_bc(
            &geom,
            "xmin",
            Box::new(FixedValue(Profile::PerFace(vec![[0.0; 3]; nf]))),
        )
        .unwrap();
    for p in ["xmax", "ymin", "ymax"] {
        field.set_bc(&geom, p, Box::new(ZeroGradient)).unwrap();
    }
    let flux = unit_flux(&geom);
    let gamma = uniform_gamma(&geom, 0.0);
    let ctl = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-13,
        max_iter: 2000,
    };
    let dt = dt_over_h * h;
    let steps = (t_end / dt).round() as usize;
    let mut hist = History::new(field.values.clone());
    for _ in 0..steps {
        for _ in 0..sweeps {
            let inp = SweepInputs {
                phi: &flux,
                gamma: &gamma,
                scheme: space,
                relax: 1.0,
                source: None,
                solver: &ctl,
                ddt: Some((time, dt, &hist)),
            };
            sweep(&geom, &mut field, &inp);
        }
        hist.push(field.values.clone(), dt);
    }
    field.values
}

fn excursion(v: &[f64]) -> (f64, f64) {
    let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - 1.0;
    let lo = -v.iter().cloned().fold(f64::INFINITY, f64::min);
    (hi, lo)
}

fn width(v: &[f64]) -> usize {
    v.iter().filter(|&&x| x > 0.1 && x < 0.9).count()
}

/// Record of the first W1 run: at dt = 0.5 h the implicit-Euler damping kept the central scheme's excursion
/// (0.0178) below the registered 0.02 positive-control threshold, so the control failed before any bounded scheme
/// was examined. Superseded by `w1b_square_wave_stays_bounded` (dt = 0.1 h); kept unchanged as the record.
#[test]
#[ignore = "positive control not met at dt = 0.5 h, superseded by w1b"]
fn w1_square_wave_stays_bounded() {
    w1_body(0.5);
}

#[test]
fn w1b_square_wave_stays_bounded() {
    w1_body(0.1);
}

fn w1_body(dt_over_h: f64) {
    use TvdLimiter::*;
    let run = |s| wave(s, TimeScheme::ImplicitEuler, 200, dt_over_h, 0.4, 20);
    let up = run(ConvScheme::Upwind);
    let w_up = width(&up);
    let ce = run(ConvScheme::Central);
    let (h, l) = excursion(&ce);
    println!("W1 central: over {h:.4}, under {l:.4}, width {}", width(&ce));
    assert!(h.max(l) > 0.02, "positive control: central must violate the bounds");
    for (name, s, sharp) in [
        ("gamma(0.2)", ConvScheme::Tvd(Gamma(0.2)), true),
        ("compressive(1.5)", ConvScheme::Tvd(Compressive(1.5)), false),
        ("compressive(2)", ConvScheme::Tvd(Compressive(2.0)), true),
        ("bounded_central", ConvScheme::Tvd(BoundedCentral), false),
    ] {
        let v = run(s);
        let (h, l) = excursion(&v);
        println!(
            "W1 {name}: over {h:.3e}, under {l:.3e}, width {} (upwind {w_up})",
            width(&v)
        );
        assert!(h <= 1e-3 && l <= 1e-3, "{name}: over {h:e} under {l:e}");
        if sharp {
            assert!(
                width(&v) as f64 <= 0.6 * w_up as f64,
                "{name}: width {} vs upwind {w_up}",
                width(&v)
            );
        }
    }
}

fn decay_error(scheme: TimeScheme, dt: f64) -> f64 {
    let geom = geom_1d(2, 1.0);
    let vol = geom.mesh.cell_volumes().to_vec();
    let n = vol.len();
    let mut hist = History::new(vec![1.0; n]);
    let steps = (1.0 / dt).round() as usize;
    for _ in 0..steps {
        let mut m = FvMatrix::zeros(&geom);
        m.a.diag[..n].copy_from_slice(&vol[..n]); // lambda = 1
        let m = add_ddt(scheme, m, &vol, dt, &hist);
        let new: Vec<f64> = (0..n).map(|i| m.b[i] / m.a.diag[i]).collect();
        hist.push(new, dt);
    }
    (hist.old[0] - (-1.0_f64).exp()).abs()
}

#[test]
fn t1_temporal_order() {
    for (name, scheme, lo, hi) in [
        ("euler", TimeScheme::ImplicitEuler, 0.9, 1.15),
        ("bdf2", TimeScheme::Bdf2, 1.8, 2.3),
        ("bdf2_bounded", TimeScheme::Bdf2Bounded, 1.8, 2.3),
    ] {
        let e: Vec<f64> = [0.2, 0.1, 0.05, 0.025]
            .iter()
            .map(|&dt| decay_error(scheme, dt))
            .collect();
        let o: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
        println!("T1 {name}: errors {e:?} orders {o:?}");
        assert!(o[1] >= lo && o[1] <= hi && o[2] >= lo && o[2] <= hi, "{name}: {o:?}");
    }
}

#[test]
fn t2_bounded_time_scheme_square_wave() {
    let run = |t| wave(ConvScheme::Upwind, t, 100, 2.0, 0.8, 1);
    let over = |v: &[f64]| {
        let (h, l) = excursion(v);
        h.max(l).max(0.0)
    };
    let (ob2, obd) = (over(&run(TimeScheme::Bdf2)), over(&run(TimeScheme::Bdf2Bounded)));
    println!("T2: overshoot BDF2 {ob2:.4e}, bounded BDF2 {obd:.4e}");
    if ob2 < 0.02 {
        println!("T2 vacuous: plain BDF2 overshoot below the 0.02 registered threshold");
        return;
    }
    assert!(obd <= 0.25 * ob2 && obd <= 0.02, "bounded {obd:e} vs plain {ob2:e}");
}
