//! F-869 (ADR 0763): the mesh-converged mean drag coefficient of the laminar flow past a circular cylinder at Re = 100
//! against the band of the published values of ADR 0761, [1.314, 1.370], unchanged.
//!
//! Set-up of `f869_cylinder_re100_mean_drag.rs` (ADR 0761) with three things changed: the far field at 30 D, the time
//! step 0.025 on every mesh, and the meshes 90 x 70, 128 x 100, 181 x 141. O-grid, PISO with two outer iterations,
//! `time_scheme = Bdf2`, linear upwind, to t = 160, statistics over the last 8 whole shedding periods.
//!
//! * The gate runs `g1_...` (the Richardson procedure for unequal ratios recovers a manufactured limit and order and
//!   reproduces the record of ADR 0761), `n1_...` (the negative controls of the decision), `g3_...` (the window statistics
//!   on a signal with a closed form) and `g4_...` (90 x 70 at 30 D, 40 steps through the path of the heavy record).
//! * `h1_..._heavy_record` (ignored) carries W1 (the fine value in the band), S1 and K4w; `w2_..._heavy_record` (ignored)
//!   carries W3a, W3c and W2; `w3b_..._known_miss` (ignored) is the missed W3b, limit intact (amendment 1): the observed
//!   order is 1.387 against [1.5, 2.5], so the extrapolation is not valid and the mesh-converged drag is not shown. They
//!   share the three runs. Command:
//!   `cargo test --release -p rcfd-api --test f869_cylinder_mesh_converged_drag -- --ignored --nocapture`.
//!
//! Every limit is registered in ADR 0763 and is not changed after a run. The tests of ADR 0761 are untouched.

mod validation_common;

use rcfd_core::vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::{boundary_values, pressure_bc};
use rcfd_fv::{FixedValue, Freestream, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{cylinder_ogrid, Grading};
use rcfd_post::integrals::wall_force_and_moment;
use std::sync::{Arc, OnceLock};
use validation_common::*;

const DEPTH: f64 = 0.1;
const NU: f64 = 0.01;
/// The far-field radius of ADR 0763: 30 D.
const R_OUT: f64 = 30.0;
/// The time step of ADR 0763, on every mesh.
const DT: f64 = 0.025;
/// The three meshes of ADR 0763 (n_theta, n_r), coarse to fine.
const MESHES: [(usize, usize); 3] = [(90, 70), (128, 100), (181, 141)];

/// The band of the published mean drag coefficients at Re 100 (ADR 0761, "The band, registered"; unchanged).
const BAND: (f64, f64) = (1.314, 1.370);
/// The range of the observed order in which the extrapolation is valid (ADR 0763: within 0.5 of the formal order 2).
const ORDER_RANGE: (f64, f64) = (1.5, 2.5);
/// The limit of K3 of ADR 0060 on the grid-convergence index of the drag.
const GCI_LIMIT: f64 = 0.03;
/// The window of K1 of ADR 0060 for the Strouhal number (reported, not a limit of this file).
const K1_WINDOW: (f64, f64) = (0.165 * 0.97, 0.165 * 1.03);

fn within(x: f64, window: (f64, f64)) -> bool {
    x.is_finite() && x >= window.0 && x <= window.1
}

/// The refinement ratios (r21, r32) of three meshes from their cell counts, with h = N^(-1/2).
fn ratios(meshes: [(usize, usize); 3]) -> (f64, f64) {
    let n = |m: (usize, usize)| (m.0 * m.1) as f64;
    (
        (n(meshes[1]) / n(meshes[0])).sqrt(),
        (n(meshes[2]) / n(meshes[1])).sqrt(),
    )
}

/// What the Richardson procedure of ADR 0763 gives for three values, coarse to fine.
#[derive(Clone, Copy, Debug)]
struct Rich {
    /// `(f1 - f2)(f2 - f3) > 0`.
    monotone: bool,
    /// The observed order; NaN where the sequence is not monotone or the bisection has no root.
    p: f64,
    /// The Richardson value with the observed order; NaN where there is no order.
    value: f64,
}

/// `g(p) = r32^p (r21^p - 1) / (r32^p - 1)`, the ratio `(f1 - f2)/(f2 - f3)` of the model `f = f_0 + C h^p`.
fn difference_ratio(p: f64, r21: f64, r32: f64) -> f64 {
    r32.powf(p) * (r21.powf(p) - 1.0) / (r32.powf(p) - 1.0)
}

/// The Richardson step from the fine pair at the order `p`.
fn richardson_value(f2: f64, f3: f64, p: f64, r32: f64) -> f64 {
    f3 + (f3 - f2) / (r32.powf(p) - 1.0)
}

/// Observed order (bisection on [0.01, 20]) and Richardson value for unequal refinement ratios.
fn richardson(f: [f64; 3], r21: f64, r32: f64) -> Rich {
    let (d21, d32) = (f[0] - f[1], f[1] - f[2]);
    let monotone = d21 * d32 > 0.0;
    let mut p = f64::NAN;
    if monotone {
        let target = d21 / d32;
        let (mut lo, mut hi) = (0.01, 20.0);
        if difference_ratio(lo, r21, r32) < target && target < difference_ratio(hi, r21, r32) {
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if difference_ratio(mid, r21, r32) < target {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            p = 0.5 * (lo + hi);
        }
    }
    let value = if p.is_finite() {
        richardson_value(f[1], f[2], p, r32)
    } else {
        f64::NAN
    };
    Rich { monotone, p, value }
}

/// Why the outcome is partial (ADR 0763, the decision rule).
#[derive(Debug, PartialEq)]
enum Why {
    /// W3 (a): the sequence is not monotone or has no observed order.
    NotMonotone,
    /// W3 (b): the observed order is outside [1.5, 2.5].
    Order,
    /// W2: the Richardson value is outside the band.
    OutsideBand,
    /// W3 (c): on a half window the fine value or the Richardson value is not in the band.
    Window,
    /// S1: the grid-convergence index is above 3 %.
    Gci,
}

#[derive(Debug, PartialEq)]
enum Verdict {
    Done,
    Partial(Why),
    /// W1 missed: the fine value itself is outside the band.
    Stopped,
}

/// The decision of ADR 0763 on the solution checks: `fine` is the fine value on the whole window, `full` the procedure
/// on the whole window, `halves` the fine value and the procedure on each half window, `gci` the index of the fine pair.
fn verdict(fine: f64, full: &Rich, halves: &[(f64, Rich); 2], gci: f64) -> Verdict {
    if !within(fine, BAND) {
        return Verdict::Stopped;
    }
    if !full.monotone || !full.p.is_finite() {
        return Verdict::Partial(Why::NotMonotone);
    }
    if !within(full.p, ORDER_RANGE) {
        return Verdict::Partial(Why::Order);
    }
    if !within(full.value, BAND) {
        return Verdict::Partial(Why::OutsideBand);
    }
    if !halves_hold(halves) {
        return Verdict::Partial(Why::Window);
    }
    if !(gci.is_finite() && gci <= GCI_LIMIT) {
        return Verdict::Partial(Why::Gci);
    }
    Verdict::Done
}

/// W3 (c): on each half window the fine value is in the band and the Richardson value exists and is in the band.
fn halves_hold(halves: &[(f64, Rich); 2]) -> bool {
    halves
        .iter()
        .all(|(fine, r)| within(*fine, BAND) && r.monotone && r.p.is_finite() && within(r.value, BAND))
}

/// The recorded signals of one run: the probe velocity for t > `t_probe` and the force for t > `t_force`.
struct Series {
    label: String,
    t: Vec<f64>,
    v: Vec<f64>,
    tc: Vec<f64>,
    fx: Vec<f64>,
    fy: Vec<f64>,
}

/// What one run gives over its window of 8 whole periods.
struct Shed {
    label: String,
    crossings: usize,
    st: f64,
    /// Mean drag coefficient over the window.
    cd: f64,
    /// Mean drag coefficient over the first and the last 4 whole periods of the window.
    cd_half: [f64; 2],
    /// Half of the largest less the smallest lift coefficient.
    cl_amplitude: f64,
    /// Root mean square of the lift coefficient about its mean.
    cl_rms: f64,
    window: (f64, f64),
}

fn setup(n_theta: usize, n_r: usize, r_out: f64) -> PimpleSolver {
    let mesh = Arc::new(cylinder_ogrid(n_theta, n_r, 0.5, r_out, Grading::Expansion(150.0), DEPTH).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let uinf = [1.0, 0.0, 0.0];
    let mut u = VectorField::from_fn("U", &geom, |c| {
        let blob = 0.1 * (-((c[0] - 2.0).powi(2) + (c[1] - 0.3).powi(2))).exp();
        [1.0, blob, 0.0]
    });
    u.set_bc(&geom, "cylinder", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: uinf,
            fixed_on_inflow: true,
        }),
    )
    .unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(
        &geom,
        "farfield",
        Box::new(Freestream {
            u_ref: uinf,
            value: [0.0; 3],
            fixed_on_inflow: false,
        }),
    )
    .unwrap();
    let mut st = PimpleSettings::piso(NU);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.n_outer = 2;
    st.amg_pressure = true;
    st.alpha_u = 0.9;
    st.alpha_p = 0.9;
    PimpleSolver::new(geom, u, p, st)
}

fn nearest(s: &PimpleSolver, x: [f64; 3]) -> usize {
    let c = s.geom.mesh.cell_centres();
    (0..c.len())
        .min_by(|&a, &b| {
            let d = |i: usize| (c[i][0] - x[0]).powi(2) + (c[i][1] - x[1]).powi(2);
            d(a).total_cmp(&d(b))
        })
        .unwrap()
}

/// Force of the fluid on the cylinder: (F_x, F_y).
fn force(s: &PimpleSolver) -> (f64, f64) {
    let pbc = pressure_bc(&s.geom, &s.u, &s.p);
    let pf = boundary_values(&s.geom, &pbc.coeffs, &s.p.values);
    let (f, _) = wall_force_and_moment(&s.geom, "cylinder", &s.u, &pf, NU, 1.0, vec3::ZERO).unwrap();
    (f.total()[0], f.total()[1])
}

/// One run at the registered far field and step, to `t_end`, with the blow-up guard of ADR 0062 as ADR 0761 uses it.
fn advance(n_theta: usize, n_r: usize, t_end: f64, t_probe: f64, t_force: f64) -> Series {
    let label = format!("{n_theta}x{n_r} dt {DT} r_out {R_OUT}");
    let mut s = setup(n_theta, n_r, R_OUT);
    let probe = nearest(&s, [3.0, 0.0, DEPTH / 2.0]);
    let (mut t, mut v) = (Vec::new(), Vec::new());
    let (mut tc, mut fx, mut fy) = (Vec::new(), Vec::new(), Vec::new());
    let t0 = std::time::Instant::now();
    let mut next_report = 20.0;
    while s.time < t_end {
        let info = s.step(DT);
        // a diverged run fails here and is not repeated
        assert!(
            info.courant.is_finite() && info.courant < 1e4,
            "blow-up of {label} at t = {:.2}: Courant number {}",
            s.time,
            info.courant
        );
        if s.time >= next_report {
            println!("PROGRESS {label}: t {:.1}, {:.0} s", s.time, t0.elapsed().as_secs_f64());
            next_report += 20.0;
        }
        if s.time > t_probe {
            t.push(s.time);
            v.push(s.u.values[probe][1]);
        }
        if s.time > t_force {
            let (x, y) = force(&s);
            tc.push(s.time);
            fx.push(x);
            fy.push(y);
        }
    }
    println!("RAN {label} to t {:.3} in {:.0} s", s.time, t0.elapsed().as_secs_f64());
    Series {
        label,
        t,
        v,
        tc,
        fx,
        fy,
    }
}

/// Upward zero crossings (linear interpolation) of `v - mean`.
fn crossings(t: &[f64], v: &[f64]) -> Vec<f64> {
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    let mut cross = Vec::new();
    for i in 1..v.len() {
        if v[i - 1] - mean < 0.0 && v[i] - mean >= 0.0 {
            let w = (mean - v[i - 1]) / (v[i] - v[i - 1]);
            cross.push(t[i - 1] + w * (t[i] - t[i - 1]));
        }
    }
    cross
}

/// Mean over `[ta, tb]` (trapezoid rule) of the force coefficient `f / (0.5 U^2 D depth)` with U = 1, D = 1.
fn mean_coefficient(t: &[f64], f: &[f64], ta: f64, tb: f64) -> f64 {
    let (mut integral, mut span) = (0.0, 0.0);
    for i in 1..t.len() {
        let (a, b) = (t[i - 1].max(ta), t[i].min(tb));
        if b > a {
            integral += 0.5 * (f[i - 1] + f[i]) * (b - a);
            span += b - a;
        }
    }
    integral / span / (0.5 * DEPTH)
}

/// The statistics of ADR 0763 over the last 8 whole periods between upward zero crossings of the probe signal.
fn statistics(s: &Series) -> Shed {
    let cross = crossings(&s.t, &s.v);
    let n_use = 8;
    let k = cross.len();
    assert!(k > n_use, "{}: only {k} zero crossings, no sustained shedding", s.label);
    let (ta, tm, tb) = (cross[k - 1 - n_use], cross[k - 1 - n_use / 2], cross[k - 1]);
    let st = n_use as f64 / (tb - ta);
    let cd = mean_coefficient(&s.tc, &s.fx, ta, tb);
    let cd_half = [
        mean_coefficient(&s.tc, &s.fx, ta, tm),
        mean_coefficient(&s.tc, &s.fx, tm, tb),
    ];
    let cl_mean = mean_coefficient(&s.tc, &s.fy, ta, tb);
    let cl: Vec<f64> =
        s.tc.iter()
            .zip(&s.fy)
            .filter(|(&time, _)| time >= ta && time <= tb)
            .map(|(_, &y)| y / (0.5 * DEPTH))
            .collect();
    let cl_max = cl.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let cl_min = cl.iter().cloned().fold(f64::INFINITY, f64::min);
    let cl_rms = (cl.iter().map(|c| (c - cl_mean).powi(2)).sum::<f64>() / cl.len() as f64).sqrt();
    Shed {
        label: s.label.clone(),
        crossings: k,
        st,
        cd,
        cd_half,
        cl_amplitude: 0.5 * (cl_max - cl_min),
        cl_rms,
        window: (ta, tb),
    }
}

/// One registered run: to t = 160, the probe for t > 60, the force for t > 85 (as ADR 0761).
fn run(n_theta: usize, n_r: usize) -> Shed {
    let r = statistics(&advance(n_theta, n_r, 160.0, 60.0, 85.0));
    println!(
        "RESULT {}: {} crossings, St {:.5} ({:+.2} % of 0.165, in K1's window: {}), C_D {:.5} (first half {:.5}, second \
         half {:.5}), C_L amplitude {:.4}, C_L rms {:.4}, window {:.1}..{:.1}",
        r.label,
        r.crossings,
        r.st,
        pct(r.st, 0.165),
        within(r.st, K1_WINDOW),
        r.cd,
        r.cd_half[0],
        r.cd_half[1],
        r.cl_amplitude,
        r.cl_rms,
        r.window.0,
        r.window.1
    );
    r
}

/// The manufactured triple `f_0 + C h^p` on the registered cell counts, with h of the fine mesh as 1.
fn manufactured(f0: f64, c: f64, p: f64) -> [f64; 3] {
    let (r21, r32) = ratios(MESHES);
    [f0 + c * (r32 * r21).powf(p), f0 + c * r32.powf(p), f0 + c]
}

/// G1 (ADR 0763): the procedure returns a manufactured limit and order, and with equal ratios the record of ADR 0761.
#[test]
fn g1_the_richardson_procedure_recovers_a_manufactured_limit_and_the_record_of_adr_0761() {
    let (r21, r32) = ratios(MESHES);
    println!("ratios r21 {r21:.5}, r32 {r32:.5}");
    assert!((r21 - 1.42539).abs() < 1e-5 && (r32 - 1.41203).abs() < 1e-5);
    let mut c = Checks::default();
    for p in [1.5, 2.0, 2.5] {
        let f = manufactured(1.345, -0.008, p);
        let r = richardson(f, r21, r32);
        c.is_true(&format!("G1i monotone, p {p}"), r.monotone, &format!("{f:?}"));
        c.at_most(&format!("G1i order, p {p}"), (r.p / p - 1.0).abs(), 1e-9);
        c.at_most(&format!("G1i limit, p {p}"), (r.value / 1.345 - 1.0).abs(), 1e-9);
    }
    // the record of ADR 0761 (V2b): 64 x 50, 90 x 70, 128 x 100 at dt 0.05, both ratios sqrt 2
    let root2 = 2f64.sqrt();
    let old = richardson([1.31674, 1.33879, 1.35247], root2, root2);
    println!("ADR 0761 triple: order {:.4}, Richardson value {:.5}", old.p, old.value);
    c.at_most(
        "G1ii the Richardson value of ADR 0761, 1.37479",
        (old.value - 1.37479).abs(),
        1e-4,
    );
    c.at_most("G1ii the observed order of ADR 0761, 1.38", (old.p - 1.38).abs(), 0.01);
    // with equal ratios the root is the closed form of `validation_common::observed_order`
    let closed = observed_order(1.31674, 1.33879, 1.35247, root2);
    c.at_most(
        "G1ii the closed form for equal ratios",
        (old.p / closed - 1.0).abs(),
        1e-9,
    );
    c.finish();
}

/// N1 (ADR 0763): the negative controls of the decision, through the function the heavy record uses.
#[test]
fn n1_the_decision_rejects_a_value_outside_the_band_and_an_extrapolation_that_is_not_valid() {
    let (r21, r32) = ratios(MESHES);
    let judge = |f: [f64; 3], half: Option<[f64; 3]>| {
        let full = richardson(f, r21, r32);
        let h = half.unwrap_or(f);
        let halves = [(f[2], full), (h[2], richardson(h, r21, r32))];
        verdict(f[2], &full, &halves, gci_fine(f[1], f[2], full.p, r32))
    };
    let good = manufactured(1.345, -0.008, 2.0);
    let outside = manufactured(1.375, -0.012, 2.0);
    assert!(within(outside[2], BAND), "the fine value of control (a) is in the band");
    // (a) the Richardson value 1.375 is outside the band although the fine value is inside
    assert_eq!(judge(outside, None), Verdict::Partial(Why::OutsideBand));
    // (b) a sequence that is not monotone
    assert_eq!(judge([1.330, 1.340, 1.337], None), Verdict::Partial(Why::NotMonotone));
    // (c) the observed order 1.2 is outside [1.5, 2.5] although the limit is in the band
    assert_eq!(
        judge(manufactured(1.345, -0.008, 1.2), None),
        Verdict::Partial(Why::Order)
    );
    // (d) the recorded triple of ADR 0761 with its ratios is not "done"
    let root2 = 2f64.sqrt();
    let old = richardson([1.31674, 1.33879, 1.35247], root2, root2);
    let old_verdict = verdict(
        1.35247,
        &old,
        &[(1.35247, old), (1.35247, old)],
        gci_fine(1.33879, 1.35247, old.p, root2),
    );
    assert!(matches!(old_verdict, Verdict::Partial(_)), "{old_verdict:?}");
    // (e) a fine value outside the band stops
    assert_eq!(judge(manufactured(1.390, -0.010, 2.0), None), Verdict::Stopped);
    // (f) a limit in the band at the formal order is done
    assert_eq!(judge(good, None), Verdict::Done);
    // (g) one half window whose Richardson value is outside the band
    assert_eq!(judge(good, Some(outside)), Verdict::Partial(Why::Window));
    // the limit of S1 is part of the decision
    let full = richardson(good, r21, r32);
    assert_eq!(
        verdict(good[2], &full, &[(good[2], full), (good[2], full)], 0.031),
        Verdict::Partial(Why::Gci)
    );
}

/// G3 (ADR 0763): the window statistics on signals with a closed form, sampled as a run records them.
#[test]
fn g3_the_window_statistics_agree_with_a_closed_form() {
    let (freq, two_pi) = (0.165, 2.0 * std::f64::consts::PI);
    let mut s = Series {
        label: "closed form".into(),
        t: Vec::new(),
        v: Vec::new(),
        tc: Vec::new(),
        fx: Vec::new(),
        fy: Vec::new(),
    };
    for i in 1..=6400 {
        let time = i as f64 * DT;
        if time > 60.0 {
            s.t.push(time);
            s.v.push((two_pi * freq * time).sin());
        }
        if time > 85.0 {
            s.tc.push(time);
            s.fx.push(0.5 * DEPTH * (1.34 + 0.01 * (2.0 * two_pi * freq * time).sin()));
            s.fy.push(0.5 * DEPTH * 0.33 * (two_pi * freq * time + 0.7).sin());
        }
    }
    let r = statistics(&s);
    let mut c = Checks::default();
    let rel = |x: f64, exact: f64| (x / exact - 1.0).abs();
    c.at_most("G3 St", rel(r.st, freq), 1e-3);
    c.at_most("G3 mean C_D on W", rel(r.cd, 1.34), 1e-3);
    c.at_most("G3 mean C_D on W1", rel(r.cd_half[0], 1.34), 1e-3);
    c.at_most("G3 mean C_D on W2", rel(r.cd_half[1], 1.34), 1e-3);
    c.at_most("G3 C_L amplitude", rel(r.cl_amplitude, 0.33), 1e-3);
    c.at_most("G3 C_L rms", rel(r.cl_rms, 0.33 / 2f64.sqrt()), 1e-3);
    let period = 1.0 / freq;
    c.at_most(
        "G3 8 whole periods in W",
        rel(r.window.1 - r.window.0, 8.0 * period),
        1e-3,
    );
    c.finish();
}

/// G4 (ADR 0763): the coarsest registered mesh through the path of the heavy record, 40 steps.
#[test]
fn g4_the_coarsest_registered_mesh_advances_through_the_path_of_the_heavy_record() {
    let (n_theta, n_r) = MESHES[0];
    let s = advance(n_theta, n_r, 0.9999, 0.0, 0.0);
    assert_eq!((s.t.len(), s.tc.len()), (40, 40), "40 steps of dt 0.025");
    assert!((s.t[39] - 1.0).abs() < 1e-9, "t = {}", s.t[39]);
    assert!(s.v.iter().chain(&s.fx).chain(&s.fy).all(|x| x.is_finite()));
    println!(
        "after 40 steps: v at the probe {:.6}, F_x / (0.5 depth) {:.5}, F_y / (0.5 depth) {:.5}",
        s.v[39],
        s.fx[39] / (0.5 * DEPTH),
        s.fy[39] / (0.5 * DEPTH)
    );
}

/// The three runs of the heavy record, one after the other, made once for the ignored tests of this file.
fn record() -> &'static [Shed; 3] {
    static RECORD: OnceLock<[Shed; 3]> = OnceLock::new();
    RECORD.get_or_init(|| MESHES.map(|(n_theta, n_r)| run(n_theta, n_r)))
}

/// Of the three runs: the procedure on the whole window and on each half window (with the fine value of the half).
fn extrapolation(m: &[Shed; 3]) -> (Rich, [(f64, Rich); 2]) {
    let (r21, r32) = ratios(MESHES);
    let full = richardson([m[0].cd, m[1].cd, m[2].cd], r21, r32);
    let half = |i: usize| {
        (
            m[2].cd_half[i],
            richardson([m[0].cd_half[i], m[1].cd_half[i], m[2].cd_half[i]], r21, r32),
        )
    };
    (full, [half(0), half(1)])
}

/// The heavy record of ADR 0763: W1 (the fine value in the band), S1 and K4w; R1 and R2 printed.
/// Recorded on 2026-11-22 (3260 s): C_D 1.32259, 1.33285, 1.33900 on the three meshes; St 0.16169, 0.16395, 0.16486;
/// GCI_fine 0.935 %; observed order 1.387.
#[test]
#[ignore = "heavy record of ADR 0763 (three solves): cargo test --release -p rcfd-api --test f869_cylinder_mesh_converged_drag -- --ignored --nocapture"]
fn h1_fine_mesh_mean_drag_at_dt_0025_and_30d_in_the_band_heavy_record() {
    let m = record();
    let (r21, r32) = ratios(MESHES);
    let (full, halves) = extrapolation(m);
    let (f1, f2, f3) = (m[0].cd, m[1].cd, m[2].cd);
    let gci = gci_fine(f2, f3, full.p, r32);
    let mut c = Checks::default();
    for r in m {
        c.is_true(
            &format!("K4w {}", r.label),
            r.crossings > 9,
            &format!("{} crossings", r.crossings),
        );
    }
    c.is_true(
        "W1 mean C_D on 181x141, 30 D, dt 0.025 in [1.314, 1.370]",
        within(f3, BAND),
        &format!("C_D {f3:.5} ({:+.2} % of 1.342)", pct(f3, 1.342)),
    );
    c.at_most("S1 GCI_fine(C_D), three meshes at dt 0.025", gci, GCI_LIMIT);
    let root2 = 2f64.sqrt();
    let p_equal = observed_order(f1, f2, f3, root2);
    println!(
        "R2 mesh sequence at dt {DT}, far field {R_OUT}: C_D {f1:.5}, {f2:.5}, {f3:.5}; ratios {r21:.5}, {r32:.5}; \
         monotone {}; observed order {:.3}; Richardson value {:.5}; GCI_fine {:.3} %",
        full.monotone,
        full.p,
        full.value,
        100.0 * gci
    );
    println!(
        "R2 by the equal-ratio form of ADR 0761 (r = sqrt 2): order {p_equal:.3}, value {:.5}; at the formal order 2: \
         {:.5}",
        richardson_value(f2, f3, p_equal, root2),
        richardson_value(f2, f3, 2.0, r32)
    );
    for (i, (fine, r)) in halves.iter().enumerate() {
        println!(
            "R2 half window {}: C_D {:.5}, {:.5}, {fine:.5}; monotone {}; order {:.3}; Richardson value {:.5}",
            i + 1,
            m[0].cd_half[i],
            m[1].cd_half[i],
            r.monotone,
            r.p,
            r.value
        );
    }
    println!("VERDICT of the solution checks: {:?}", verdict(f3, &full, &halves, gci));
    c.finish();
}

/// The heavy record of ADR 0763: W3a, W3c and W2. W3b is the known miss below (amendment 1), so the value that W2 compares
/// here, 1.34901 as recorded on 2026-11-22, is not a valid extrapolation by the registration and is not a result.
#[test]
#[ignore = "heavy record of ADR 0763, shares the runs of h1: -- --ignored --nocapture"]
fn w2_richardson_value_of_the_mesh_sequence_valid_and_in_the_band_heavy_record() {
    let (full, halves) = extrapolation(record());
    let mut c = Checks::default();
    c.is_true(
        "W3a the sequence is monotone and has an observed order",
        full.monotone && full.p.is_finite(),
        &format!("monotone {}, order {:.3}", full.monotone, full.p),
    );
    c.is_true(
        "W3c on each half window the fine value and the Richardson value in [1.314, 1.370]",
        halves_hold(&halves),
        &format!(
            "fine {:.5} and {:.5}, Richardson value {:.5} and {:.5}",
            halves[0].0, halves[1].0, halves[0].1.value, halves[1].1.value
        ),
    );
    c.is_true(
        "W2 the Richardson value of the three meshes in [1.314, 1.370]",
        within(full.value, BAND),
        &format!("Richardson value {:.5}", full.value),
    );
    c.finish();
}

/// KNOWN MISS (W3b of ADR 0763; the range unchanged): the observed order of the mean drag on 90 x 70, 128 x 100, 181 x 141
/// at dt 0.025 with the far field at 30 D is 1.387, under the range [1.5, 2.5] in which the extrapolation is valid. The
/// fine value, 1.33900, is in the band (W1). What follows and what is missing: ADR 0763, addendum 1.
#[test]
#[ignore = "known miss W3b (observed order 1.387 against [1.5, 2.5]); heavy, shares the runs of h1: -- --ignored w3b"]
fn w3b_observed_order_of_the_mesh_sequence_in_the_registered_range_known_miss() {
    let (full, _) = extrapolation(record());
    let mut c = Checks::default();
    c.is_true(
        "W3b the observed order in [1.5, 2.5]",
        within(full.p, ORDER_RANGE),
        &format!("order {:.3}", full.p),
    );
    c.finish();
}
