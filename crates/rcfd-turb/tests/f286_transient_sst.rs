//! ADR 0199 (F-286), library checks of the transient k-omega SST sweep: the homogeneous set-up H against the fully implicit,
//! the variable-step and the lagged recursions of its decay (D1, D1v, D1b, the negative control D1N), the closed form of the
//! decay (D2, D2v, report-only D2b), and the global k budget of a convected, diffused, decaying field (C1). The limits are those
//! registered in ADR 0199 before this code ran.
//!
//! H (amendment 2): `box_2d(4, 4, 1, 1)` closed, all four sides the model's walls with k and omega at zero gradient, `u = 0`,
//! `nu = 0.1`, `k0 = 1`, `omega0 = 10`. Then `F1 = 1` in every cell, `P = 0`, and every cell obeys `dk/dt = -beta* omega k`,
//! `d omega/dt = -beta1 omega^2`, whose solution is `omega = omega0 / (1 + beta1 omega0 t)`,
//! `k = k0 (1 + beta1 omega0 t)^(-beta*/beta1)`.
//!
//! The first set-ups (periodic in x) were defective (amendment 2: the periodic seam lags the partner value within a sweep, and
//! its two faces carry one-sided diffusivities); their tests stay as ignored records (`*_periodic_record`).

use rcfd_fv::convection::{face_flux, ConvScheme};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::{FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use rcfd_turb::KOmegaSst;
use std::f64::consts::PI;
use std::sync::Arc;

const BETA_STAR: f64 = 0.09;
const BETA1: f64 = 0.075;
const NU: f64 = 0.1;
const K0: f64 = 1.0;
const W0: f64 = 10.0;
/// Sweeps per step that stand for the converged outer iterations.
const M: usize = 30;

/// The registered set-up (periodic in x, the record) and the corrected one of amendment 2 (closed box).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Setup {
    Periodic,
    Closed,
}

/// `T = 1 / (beta1 omega0)`: omega halves.
fn t_end() -> f64 {
    1.0 / (BETA1 * W0)
}

/// The closed form at `t`: (k, omega).
fn closed(t: f64) -> (f64, f64) {
    let s = 1.0 + BETA1 * W0 * t;
    (K0 * s.powf(-BETA_STAR / BETA1), W0 / s)
}

fn tight() -> SolveControl {
    SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-14,
        max_iter: 1000,
    }
}

fn geometry(setup: Setup, nx: usize, ny: usize) -> Arc<FvGeometry> {
    let mesh = box_2d(nx, ny, 1.0, 1.0, setup == Setup::Periodic, false).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

fn walls(setup: Setup) -> &'static [&'static str] {
    match setup {
        Setup::Periodic => &["ymin", "ymax"],
        Setup::Closed => &["xmin", "xmax", "ymin", "ymax"],
    }
}

/// The model of H and C1: the walls of the set-up, k and omega at zero gradient on them, the tight solver controls.
fn model(geom: &Arc<FvGeometry>, setup: Setup) -> KOmegaSst {
    let mut m = KOmegaSst::new(geom.clone(), NU, walls(setup), K0, W0).unwrap();
    for w in walls(setup) {
        m.omega.set_bc(geom, w, Box::new(ZeroGradient)).unwrap();
        m.k.set_bc(geom, w, Box::new(ZeroGradient)).unwrap();
    }
    m.solver = tight();
    m
}

fn spread(v: &[f64]) -> f64 {
    let (lo, hi) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    (hi - lo) / hi.abs()
}

/// H0: `F1 = 1` exactly in every cell of the last sweep, k and omega uniform to 1e-13.
fn h0(m: &KOmegaSst, what: &str) {
    assert!(
        m.f1().iter().all(|&f| f == 1.0),
        "H0 {what}: F1 is not 1 everywhere: {:?}",
        m.f1()
    );
    let (sk, sw) = (spread(&m.k.values), spread(&m.omega.values));
    assert!(
        sk <= 1e-13 && sw <= 1e-13,
        "H0 {what}: spread k {sk:.3e}, omega {sw:.3e}"
    );
}

/// BDF2 weights `(c0, c1, c2)` of a step of size `dt` after one of size `dt_old` (Euler without an old step).
fn weights(scheme: TimeScheme, dt: f64, dt_old: Option<f64>) -> (f64, f64, f64) {
    match (scheme, dt_old) {
        (TimeScheme::Bdf2, Some(d)) => {
            let r = dt / d;
            ((1.0 + 2.0 * r) / (1.0 + r), 1.0 + r, r * r / (1.0 + r))
        }
        _ => (1.0, 1.0, 0.0),
    }
}

/// Reference recursions of the decay: the fully implicit step and the lagged one-sweep Euler step.
#[derive(Clone, Copy)]
struct Ref {
    k: f64,
    w: f64,
    k_old: f64,
    w_old: f64,
    dt_old: Option<f64>,
}

impl Ref {
    fn new() -> Self {
        Ref {
            k: K0,
            w: W0,
            k_old: K0,
            w_old: W0,
            dt_old: None,
        }
    }

    fn implicit(&mut self, scheme: TimeScheme, dt: f64) {
        let (c0, c1, c2) = weights(scheme, dt, self.dt_old);
        let a = dt * BETA1;
        let rw = c1 * self.w - c2 * self.w_old;
        let rk = c1 * self.k - c2 * self.k_old;
        let w = (-c0 + (c0 * c0 + 4.0 * a * rw).sqrt()) / (2.0 * a);
        let k = rk / (c0 + dt * BETA_STAR * w);
        *self = Ref {
            k,
            w,
            k_old: self.k,
            w_old: self.w,
            dt_old: Some(dt),
        };
    }

    fn lagged(&mut self, dt: f64) {
        let w = self.w / (1.0 + dt * BETA1 * self.w);
        let k = self.k / (1.0 + dt * BETA_STAR * self.w);
        *self = Ref {
            k,
            w,
            k_old: self.k,
            w_old: self.w,
            dt_old: Some(dt),
        };
    }
}

/// Runs H over the steps `dts` with `sweeps` sweeps per step; after each step calls `each(step, k, omega)` with the cell-0
/// values (H0 is checked after every sweep). Returns the final (k, omega).
fn run_h(
    setup: Setup,
    scheme: TimeScheme,
    dts: &[f64],
    sweeps: usize,
    mut each: impl FnMut(usize, f64, f64),
) -> (f64, f64) {
    let geom = geometry(setup, 4, 4);
    let mut m = model(&geom, setup);
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let phi = vec![0.0; geom.mesh.n_faces()];
    for (i, &dt) in dts.iter().enumerate() {
        m.begin_step(dt, scheme);
        for s in 0..sweeps {
            m.update(&u, &phi);
            h0(&m, &format!("{setup:?} step {} sweep {}", i + 1, s + 1));
        }
        each(i + 1, m.k.values[0], m.omega.values[0]);
    }
    (m.k.values[0], m.omega.values[0])
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The D1 step: `dt beta1 omega0 = 0.1`.
fn d1_dt() -> f64 {
    0.1 / (BETA1 * W0)
}

/// Steps alternating `1.2 h` and `0.8 h`, `h = T / n`, `n` steps.
fn alternating(n: usize) -> Vec<f64> {
    let h = t_end() / n as f64;
    (0..n).map(|i| if i % 2 == 0 { 1.2 * h } else { 0.8 * h }).collect()
}

/// Largest relative error of the run against the implicit recursion over the steps.
fn against_implicit(setup: Setup, scheme: TimeScheme, dts: &[f64], sweeps: usize) -> f64 {
    let mut r = Ref::new();
    let mut worst = 0.0_f64;
    run_h(setup, scheme, dts, sweeps, |i, k, w| {
        r.implicit(scheme, dts[i - 1]);
        worst = worst.max(rel(k, r.k)).max(rel(w, r.w));
    });
    worst
}

fn d1(setup: Setup) {
    let dts = vec![d1_dt(); 40];
    for scheme in [TimeScheme::ImplicitEuler, TimeScheme::Bdf2] {
        let worst = against_implicit(setup, scheme, &dts, M);
        println!(
            "D1 {setup:?} {scheme:?}: largest relative error against the fully implicit recursion {worst:.3e} (limit 1e-12)"
        );
        assert!(worst <= 1e-12, "D1 {scheme:?}: {worst:.3e}");
    }
}

/// D1: with converged sweeps every step is the fully implicit step of the scheme.
#[test]
fn d1_converged_sweeps_give_the_fully_implicit_recursion() {
    d1(Setup::Closed)
}

fn d1v(setup: Setup) {
    let worst = against_implicit(setup, TimeScheme::Bdf2, &alternating(40), M);
    println!(
        "D1v {setup:?}: largest relative error against the variable-step BDF2 recursion {worst:.3e} (limit 1e-12)"
    );
    assert!(worst <= 1e-12, "D1v: {worst:.3e}");
}

/// D1v: the variable-step BDF2 recursion (steps alternating 1.2 h and 0.8 h).
#[test]
fn d1v_variable_step_bdf2_recursion() {
    d1v(Setup::Closed)
}

/// The one-sweep Euler run: (largest error against the lagged recursion, against the implicit recursion).
fn one_sweep(setup: Setup) -> (f64, f64) {
    let dts = vec![d1_dt(); 40];
    let (mut lag, mut imp) = (Ref::new(), Ref::new());
    let (mut e_lag, mut e_imp) = (0.0_f64, 0.0_f64);
    run_h(setup, TimeScheme::ImplicitEuler, &dts, 1, |i, k, w| {
        lag.lagged(dts[i - 1]);
        imp.implicit(TimeScheme::ImplicitEuler, dts[i - 1]);
        e_lag = e_lag.max(rel(k, lag.k)).max(rel(w, lag.w));
        e_imp = e_imp.max(rel(k, imp.k)).max(rel(w, imp.w));
    });
    (e_lag, e_imp)
}

fn d1b(setup: Setup) {
    let (e_lag, _) = one_sweep(setup);
    println!("D1b {setup:?}: largest relative error against the lagged recursion {e_lag:.3e} (limit 1e-12)");
    assert!(e_lag <= 1e-12, "D1b: {e_lag:.3e}");
}

/// D1b: one sweep per step is the lagged (once-per-step) update.
#[test]
fn d1b_one_sweep_gives_the_lagged_recursion() {
    d1b(Setup::Closed)
}

fn d1n(setup: Setup) {
    let (_, e_imp) = one_sweep(setup);
    println!(
        "D1N {setup:?}: largest relative difference from the fully implicit recursion {e_imp:.3e} (must be >= 1e-3)"
    );
    assert!(e_imp >= 1e-3, "D1N did not fire: {e_imp:.3e}");
}

/// D1N: the one-sweep run misses the fully implicit recursion (D1 can fail).
#[test]
fn d1n_one_sweep_misses_the_implicit_recursion() {
    d1n(Setup::Closed)
}

/// Relative errors (k, omega) at `T` against the closed form.
fn error_at_t(setup: Setup, scheme: TimeScheme, dts: &[f64]) -> (f64, f64) {
    let (k, w) = run_h(setup, scheme, dts, M, |_, _, _| {});
    let (ke, we) = closed(dts.iter().sum());
    (rel(k, ke), rel(w, we))
}

/// (e(n1), e(n2), p) for k and omega.
fn order(setup: Setup, scheme: TimeScheme, a: &[f64], b: &[f64]) -> [(f64, f64, f64); 2] {
    let (ea, eb) = (error_at_t(setup, scheme, a), error_at_t(setup, scheme, b));
    [(ea.0, eb.0, (ea.0 / eb.0).log2()), (ea.1, eb.1, (ea.1 / eb.1).log2())]
}

fn equal(n: usize) -> Vec<f64> {
    vec![t_end() / n as f64; n]
}

fn d2(setup: Setup) {
    for (scheme, band, cap) in [
        (TimeScheme::ImplicitEuler, (0.9, 1.1), 1e-2),
        (TimeScheme::Bdf2, (1.9, 2.1), 1e-4),
    ] {
        let o = order(setup, scheme, &equal(40), &equal(80));
        for (name, (e40, e80, p)) in ["k", "omega"].iter().zip(o) {
            println!(
                "D2 {setup:?} {scheme:?} {name}: e(40) {e40:.4e}, e(80) {e80:.4e}, p {p:.4} (band [{}, {}], e(80) <= {cap:e})",
                band.0, band.1
            );
            assert!(p >= band.0 && p <= band.1, "D2 {scheme:?} {name}: p {p}");
            assert!(e80 <= cap, "D2 {scheme:?} {name}: e(80) {e80:.4e}");
        }
    }
}

/// D2: the order and the error at T of Euler and BDF2 against the closed form.
#[test]
fn d2_order_against_the_closed_form() {
    d2(Setup::Closed)
}

fn d2v(setup: Setup) {
    let o = order(setup, TimeScheme::Bdf2, &alternating(80), &alternating(160));
    for (name, (e80, e160, p)) in ["k", "omega"].iter().zip(o) {
        println!(
            "D2v {setup:?} {name}: e(80) {e80:.4e}, e(160) {e160:.4e}, p {p:.4} (band [1.9, 2.1], e(160) <= 5e-5)"
        );
        assert!((1.9..=2.1).contains(&p), "D2v {name}: p {p}");
        assert!(e160 <= 5e-5, "D2v {name}: e(160) {e160:.4e}");
    }
}

/// D2v: variable-step BDF2 against the closed form.
#[test]
fn d2v_variable_step_bdf2_order() {
    d2v(Setup::Closed)
}

fn d2b(setup: Setup) {
    let o = order(setup, TimeScheme::Bdf2Bounded, &equal(40), &equal(80));
    for (name, (e40, e80, p)) in ["k", "omega"].iter().zip(o) {
        println!("D2b (report only) {setup:?} Bdf2Bounded {name}: e(40) {e40:.4e}, e(80) {e80:.4e}, p {p:.4}");
        assert!(e80.is_finite());
    }
}

/// D2b: bounded BDF2, report only.
#[test]
fn d2b_bounded_bdf2_report() {
    d2b(Setup::Closed)
}

/// The C1 flux of the closed box (amendment 2): `phi_f = +-(psi(B) - psi(A)) depth` of `psi = sin(pi x) sin(pi y) / pi` on
/// every side face, zero on the front and back. Checks C1-0 (divergence-free cells, no flux through the boundary).
fn stream_flux(geom: &FvGeometry) -> Vec<f64> {
    let mesh = &geom.mesh;
    let pts = mesh.points();
    let sf = mesh.face_area_vectors();
    let psi = |p: [f64; 3]| (PI * p[0]).sin() * (PI * p[1]).sin() / PI;
    // the boundary faces keep 0: the sides, where psi = 0 (sin(pi) is not 0 in floating point), and the front and back
    let mut phi = vec![0.0; mesh.n_faces()];
    for (f, s) in sf.iter().enumerate().take(geom.n_internal) {
        assert!(
            s[2].abs() <= 1e-12 * (s[0].abs() + s[1].abs()),
            "internal face {f} is not a side face"
        );
        let ids = mesh.face_points(f);
        let z0 = ids.iter().map(|&i| pts[i][2]).fold(f64::INFINITY, f64::min);
        let low: Vec<[f64; 3]> = ids
            .iter()
            .map(|&i| pts[i])
            .filter(|p| (p[2] - z0).abs() < 1e-12)
            .collect();
        assert_eq!(low.len(), 2, "face {f}: {low:?}");
        let (a, b) = (low[0], low[1]);
        let t = [b[0] - a[0], b[1] - a[1]];
        let len = t[0].hypot(t[1]);
        let depth = (s[0] * s[0] + s[1] * s[1]).sqrt() / len;
        let sign = if t[1] * s[0] - t[0] * s[1] >= 0.0 { 1.0 } else { -1.0 };
        phi[f] = sign * (psi(b) - psi(a)) * depth;
    }
    let big = phi.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let own = mesh.owner();
    let mut div = vec![0.0; mesh.n_cells()];
    for f in 0..mesh.n_faces() {
        div[own[f]] += phi[f];
        if f < geom.n_internal {
            div[mesh.neighbour()[f]] -= phi[f];
        }
    }
    let worst = div.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    let boundary = phi[geom.n_internal..].iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    println!(
        "C1-0: largest cell divergence {:.3e} of the largest flux {big:.3e}; largest boundary flux {boundary:e}",
        worst / big
    );
    assert!(worst <= 1e-14 * big, "C1-0: divergence {worst:.3e}");
    assert!(boundary == 0.0, "C1-0: boundary flux {boundary:e}");
    phi
}

/// C1 for one set-up, scheme and convection: the largest relative budget residual over the steps and the smallest per-step
/// change.
fn c1_run(setup: Setup, scheme: TimeScheme, conv: ConvScheme) -> (f64, f64) {
    let geom = match setup {
        Setup::Periodic => geometry(setup, 16, 4),
        Setup::Closed => geometry(setup, 16, 16),
    };
    let mut m = model(&geom, setup);
    m.convection = conv;
    m.k = ScalarField::from_fn("k", &geom, |x| 1.0 + 0.5 * (2.0 * PI * x[0]).sin());
    for w in walls(setup) {
        m.k.set_bc(&geom, w, Box::new(ZeroGradient)).unwrap();
    }
    let (u, phi) = match setup {
        Setup::Periodic => {
            let u = VectorField::uniform("U", &geom, [1.0, 0.0, 0.0]);
            let phi = face_flux(&geom, &u.values, &u.boundary_values(&geom));
            (u, phi)
        }
        Setup::Closed => (VectorField::uniform("U", &geom, [0.0; 3]), stream_flux(&geom)),
    };
    let vol = geom.mesh.cell_volumes().to_vec();
    let dt = 0.05;
    let (mut k_old, mut k_n): (Option<Vec<f64>>, Vec<f64>) = (None, m.k.values.clone());
    let mut dt_old = None;
    let (mut worst, mut least_change) = (0.0_f64, f64::INFINITY);
    for step in 1..=10 {
        m.begin_step(dt, scheme);
        let mut w_star = m.omega.values.clone();
        for s in 0..M {
            if s + 1 == M {
                w_star = m.omega.values.clone();
            }
            m.update(&u, &phi);
            assert!(m.f1().iter().all(|&f| f == 1.0), "C1 step {step}: F1 is not 1");
        }
        let (c0, c1, c2) = weights(scheme, dt, dt_old);
        let k = &m.k.values;
        let (mut b, mut norm) = (0.0, 0.0);
        for c in 0..k.len() {
            let older = k_old.as_ref().map_or(0.0, |o| o[c]);
            b += vol[c] * ((c0 * k[c] - c1 * k_n[c] + c2 * older) / dt + BETA_STAR * w_star[c] * k[c]);
            norm += vol[c] * BETA_STAR * w_star[c] * k[c];
        }
        let change = k.iter().zip(&k_n).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        worst = worst.max(b.abs() / norm);
        least_change = least_change.min(change);
        k_old = Some(std::mem::replace(&mut k_n, k.clone()));
        dt_old = Some(dt);
    }
    (worst, least_change)
}

fn c1(setup: Setup) {
    for scheme in [TimeScheme::ImplicitEuler, TimeScheme::Bdf2] {
        for conv in [ConvScheme::Upwind, ConvScheme::LinearUpwind] {
            let (worst, least) = c1_run(setup, scheme, conv);
            println!(
                "C1 {setup:?} {scheme:?} {conv:?}: largest |B| / sum V beta* omega* k {worst:.3e} (limit 1e-10); smallest per-step max |k' - k^n| {least:.3e} (must be >= 1e-3)"
            );
            assert!(worst <= 1e-10, "C1 {scheme:?} {conv:?}: budget {worst:.3e}");
            assert!(least >= 1e-3, "C1 {scheme:?} {conv:?}: vacuous, change {least:.3e}");
        }
    }
}

/// C1: the global k budget closes: the time derivative balances the destruction, the transport sums to zero.
#[test]
fn c1_global_k_budget_closes() {
    c1(Setup::Closed)
}

// ---------------------------------------------------------------- records of the defective periodic set-ups (amendment 2)

#[test]
#[ignore = "record of the defective periodic set-up (ADR 0199 amendment 2): H0 spread k 1.889e-2 at step 1 sweep 1"]
fn d1_periodic_record() {
    d1(Setup::Periodic)
}

#[test]
#[ignore = "record of the defective periodic set-up (ADR 0199 amendment 2): H0 at step 1 sweep 1"]
fn d1v_periodic_record() {
    d1v(Setup::Periodic)
}

#[test]
#[ignore = "record of the defective periodic set-up (ADR 0199 amendment 2): H0 at step 1 sweep 1"]
fn d1b_periodic_record() {
    d1b(Setup::Periodic)
}

#[test]
#[ignore = "record of the defective periodic set-up (ADR 0199 amendment 2): H0 at step 1 sweep 1"]
fn d1n_periodic_record() {
    d1n(Setup::Periodic)
}

#[test]
#[ignore = "record of the defective periodic set-up (ADR 0199 amendment 2): H0 at step 1 sweep 1"]
fn d2_periodic_record() {
    d2(Setup::Periodic)
}

#[test]
#[ignore = "record of the defective periodic set-up (ADR 0199 amendment 2): H0 at step 1 sweep 1"]
fn d2v_periodic_record() {
    d2v(Setup::Periodic)
}

#[test]
#[ignore = "record of the defective periodic set-up (ADR 0199 amendment 2): H0 at step 1 sweep 1"]
fn d2b_periodic_record() {
    d2b(Setup::Periodic)
}

#[test]
#[ignore = "record of the defective periodic set-up (ADR 0199 amendment 2): budget 2.876e-2 (Euler, upwind)"]
fn c1_periodic_record() {
    c1(Setup::Periodic)
}
