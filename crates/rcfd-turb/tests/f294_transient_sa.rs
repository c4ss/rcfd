//! ADR 0210 (F-294) part A, library checks of the transient Spalart-Allmaras sweep (DES off): the homogeneous set-up HA against
//! the fully implicit, the variable-step and the lagged recursions of its decay (A1, A1v, A1b, the negative control A1N), the
//! closed form of the decay (A2, A2v, report-only A2b), and the global nuTilda budget of a convected, diffused, decaying field
//! (AC1). The limits are those registered in ADR 0210 before this code ran.
//!
//! HA: `box_2d(4, 1, 1, 1)` closed, one row of cells between the walls `ymin` and `ymax` (nuTilda at zero gradient on them), so
//! every cell centre is at `d = 0.5`; `u = 0`, `nu = 0.01`, `nuT0 = 16 nu`. With `Omega = 0` and `chi` in (1.0028, 18.397),
//! `fv2 < 0`, the Allmaras limiter gives `S~ = 0`, `r = 10`, and every cell obeys `dnuT/dt = -a nuT^2`, `a = cw1 fw(10)/d^2`,
//! whose solution is `nuT = nuT0 / (1 + a nuT0 t)`.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::gradient::green_gauss;
use rcfd_fv::{FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::box_2d;
use rcfd_turb::sa::fw;
use rcfd_turb::{sa_s_fw, SLimiter, SaCoefficients, SpalartAllmaras};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 0.01;
const X0: f64 = 16.0 * NU;
const D: f64 = 0.5;
/// Sweeps per step that stand for the converged outer iterations.
const M: usize = 30;

/// `a = cw1 fw(10) / d^2`.
fn a_coef() -> f64 {
    let c = SaCoefficients::default();
    c.cw1() * fw(10.0, c.cw2, c.cw3) / (D * D)
}

/// `T = 1 / (a nuT0)`: nuT halves.
fn t_end() -> f64 {
    1.0 / (a_coef() * X0)
}

/// The closed form at `t`.
fn closed(t: f64) -> f64 {
    X0 / (1.0 + a_coef() * X0 * t)
}

fn tight() -> SolveControl {
    SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-14,
        max_iter: 1000,
    }
}

fn geometry(nx: usize, ny: usize) -> Arc<FvGeometry> {
    let mesh = box_2d(nx, ny, 1.0, 1.0, false, false).unwrap();
    Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap())
}

/// The model of HA (walls `ymin`, `ymax`) or AC1 (all four sides): nuTilda at zero gradient on its walls, the tight controls.
fn model(geom: &Arc<FvGeometry>, walls: &[&str], x0: f64) -> SpalartAllmaras {
    let mut m = SpalartAllmaras::new(geom.clone(), NU, walls, x0).unwrap();
    for w in walls {
        m.nutilde.set_bc(geom, w, Box::new(ZeroGradient)).unwrap();
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

/// HA0 for the iterate at the start of a sweep: nuT uniform, `d = 0.5`, and `(S~, fw) = (0, fw(10))` in every cell.
fn ha0(m: &SpalartAllmaras, what: &str) {
    let c = SaCoefficients::default();
    let x = &m.nutilde.values;
    let s = spread(x);
    assert!(s <= 1e-13, "HA0 {what}: spread of nuT {s:.3e}");
    let d = m.wall_distance();
    let dw = d.iter().map(|&v| (v - D).abs() / D).fold(0.0, f64::max);
    assert!(dw <= 1e-13, "HA0 {what}: wall distance off 0.5 by {dw:.3e}");
    let fw10 = fw(10.0, c.cw2, c.cw3);
    for (i, &xi) in x.iter().enumerate() {
        let (st, f) = sa_s_fw(xi, 0.0, d[i], NU, &c, SLimiter::Allmaras);
        assert!(
            st == 0.0 && f == fw10,
            "HA0 {what}: cell {i} chi {:.4}: S~ {st:e}, fw {f} (fw(10) {fw10})",
            xi / NU
        );
    }
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
    x: f64,
    x_old: f64,
    dt_old: Option<f64>,
}

impl Ref {
    fn new() -> Self {
        Ref {
            x: X0,
            x_old: X0,
            dt_old: None,
        }
    }

    fn implicit(&mut self, scheme: TimeScheme, dt: f64) {
        let (c0, c1, c2) = weights(scheme, dt, self.dt_old);
        let r = c1 * self.x - c2 * self.x_old;
        let x = 2.0 * r / (c0 + (c0 * c0 + 4.0 * a_coef() * dt * r).sqrt());
        *self = Ref {
            x,
            x_old: self.x,
            dt_old: Some(dt),
        };
    }

    fn lagged(&mut self, dt: f64) {
        let x = self.x / (1.0 + dt * a_coef() * self.x);
        *self = Ref {
            x,
            x_old: self.x,
            dt_old: Some(dt),
        };
    }
}

/// Runs HA over the steps `dts` with `sweeps` sweeps per step; after each step calls `each(step, nuT)` with the value of cell 0
/// (HA0 is checked before every sweep and at the end). Returns the final nuT.
fn run_h(scheme: TimeScheme, dts: &[f64], sweeps: usize, mut each: impl FnMut(usize, f64)) -> f64 {
    let geom = geometry(4, 1);
    let mut m = model(&geom, &["ymin", "ymax"], X0);
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let phi = vec![0.0; geom.mesh.n_faces()];
    for (i, &dt) in dts.iter().enumerate() {
        m.begin_time_step(dt, scheme);
        assert!(m.is_transient());
        for s in 0..sweeps {
            ha0(&m, &format!("step {} sweep {}", i + 1, s + 1));
            m.update(&u, &phi);
        }
        each(i + 1, m.nutilde.values[0]);
    }
    ha0(&m, "end");
    m.nutilde.values[0]
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// Steps alternating `1.2 h` and `0.8 h`, `h = T / n`, `n` steps.
fn alternating(n: usize) -> Vec<f64> {
    let h = t_end() / n as f64;
    (0..n).map(|i| if i % 2 == 0 { 1.2 * h } else { 0.8 * h }).collect()
}

/// Largest relative error of the run against the implicit recursion over the steps.
fn against_implicit(scheme: TimeScheme, dts: &[f64], sweeps: usize) -> f64 {
    let mut r = Ref::new();
    let mut worst = 0.0_f64;
    run_h(scheme, dts, sweeps, |i, x| {
        r.implicit(scheme, dts[i - 1]);
        worst = worst.max(rel(x, r.x));
    });
    worst
}

/// A1: with converged sweeps every step is the fully implicit step of the scheme.
#[test]
fn a1_converged_sweeps_give_the_fully_implicit_recursion() {
    let dts = vec![0.1 * t_end(); 40];
    println!("A1: a = {:.8}, T = {:.10}", a_coef(), t_end());
    for scheme in [TimeScheme::ImplicitEuler, TimeScheme::Bdf2] {
        let worst = against_implicit(scheme, &dts, M);
        println!(
            "A1 {scheme:?}: largest relative error against the fully implicit recursion {worst:.3e} (limit 1e-12)"
        );
        assert!(worst <= 1e-12, "A1 {scheme:?}: {worst:.3e}");
    }
}

/// A1v: the variable-step BDF2 recursion (steps alternating 1.2 h and 0.8 h, h = T / 40).
#[test]
fn a1v_variable_step_bdf2_recursion() {
    let worst = against_implicit(TimeScheme::Bdf2, &alternating(40), M);
    println!("A1v: largest relative error against the variable-step BDF2 recursion {worst:.3e} (limit 1e-12)");
    assert!(worst <= 1e-12, "A1v: {worst:.3e}");
}

/// The one-sweep Euler run: (largest error against the lagged recursion, against the implicit recursion).
fn one_sweep() -> (f64, f64) {
    let dts = vec![0.1 * t_end(); 40];
    let (mut lag, mut imp) = (Ref::new(), Ref::new());
    let (mut e_lag, mut e_imp) = (0.0_f64, 0.0_f64);
    run_h(TimeScheme::ImplicitEuler, &dts, 1, |i, x| {
        lag.lagged(dts[i - 1]);
        imp.implicit(TimeScheme::ImplicitEuler, dts[i - 1]);
        e_lag = e_lag.max(rel(x, lag.x));
        e_imp = e_imp.max(rel(x, imp.x));
    });
    (e_lag, e_imp)
}

/// A1b: one sweep per step is the lagged (once-per-step) update.
#[test]
fn a1b_one_sweep_gives_the_lagged_recursion() {
    let (e_lag, _) = one_sweep();
    println!("A1b: largest relative error against the lagged recursion {e_lag:.3e} (limit 1e-12)");
    assert!(e_lag <= 1e-12, "A1b: {e_lag:.3e}");
}

/// A1N: the one-sweep run misses the fully implicit recursion (A1 can fail).
#[test]
fn a1n_one_sweep_misses_the_implicit_recursion() {
    let (_, e_imp) = one_sweep();
    println!("A1N: largest relative difference from the fully implicit recursion {e_imp:.3e} (must be >= 1e-3)");
    assert!(e_imp >= 1e-3, "A1N did not fire: {e_imp:.3e}");
}

/// Relative error at `T` against the closed form.
fn error_at_t(scheme: TimeScheme, dts: &[f64]) -> f64 {
    let x = run_h(scheme, dts, M, |_, _| {});
    rel(x, closed(dts.iter().sum()))
}

/// (e(n1), e(n2), p).
fn order(scheme: TimeScheme, a: &[f64], b: &[f64]) -> (f64, f64, f64) {
    let (ea, eb) = (error_at_t(scheme, a), error_at_t(scheme, b));
    (ea, eb, (ea / eb).log2())
}

fn equal(n: usize) -> Vec<f64> {
    vec![t_end() / n as f64; n]
}

/// A2: the order and the error at T of Euler and BDF2 against the closed form.
#[test]
fn a2_order_against_the_closed_form() {
    for (scheme, band, cap) in [
        (TimeScheme::ImplicitEuler, (0.9, 1.1), 1e-2),
        (TimeScheme::Bdf2, (1.9, 2.1), 1e-4),
    ] {
        let (e40, e80, p) = order(scheme, &equal(40), &equal(80));
        println!(
            "A2 {scheme:?}: e(40) {e40:.4e}, e(80) {e80:.4e}, p {p:.4} (band [{}, {}], e(80) <= {cap:e})",
            band.0, band.1
        );
        assert!(p >= band.0 && p <= band.1, "A2 {scheme:?}: p {p}");
        assert!(e80 <= cap, "A2 {scheme:?}: e(80) {e80:.4e}");
    }
}

/// A2v: variable-step BDF2 against the closed form.
#[test]
fn a2v_variable_step_bdf2_order() {
    let (e80, e160, p) = order(TimeScheme::Bdf2, &alternating(80), &alternating(160));
    println!("A2v: e(80) {e80:.4e}, e(160) {e160:.4e}, p {p:.4} (band [1.9, 2.1], e(160) <= 5e-5)");
    assert!((1.9..=2.1).contains(&p), "A2v: p {p}");
    assert!(e160 <= 5e-5, "A2v: e(160) {e160:.4e}");
}

/// A2b: bounded BDF2, report only.
#[test]
fn a2b_bounded_bdf2_report() {
    let (e40, e80, p) = order(TimeScheme::Bdf2Bounded, &equal(40), &equal(80));
    println!("A2b (report only) Bdf2Bounded: e(40) {e40:.4e}, e(80) {e80:.4e}, p {p:.4}");
    assert!(e80.is_finite());
}

/// The flux of `psi = sin(pi x) sin(pi y) / pi` on the side faces (ADR 0199 amendment 2); every boundary face carries 0.
/// Checks C1-0 (divergence-free cells, no flux through the boundary).
fn stream_flux(geom: &FvGeometry) -> Vec<f64> {
    let mesh = &geom.mesh;
    let pts = mesh.points();
    let sf = mesh.face_area_vectors();
    let psi = |p: [f64; 3]| (PI * p[0]).sin() * (PI * p[1]).sin() / PI;
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

/// AC1 for one scheme and convection: the largest relative budget residual over the steps and the smallest per-step change
/// (relative to the largest nuT).
fn ac1_run(scheme: TimeScheme, conv: ConvScheme) -> (f64, f64) {
    let geom = geometry(16, 16);
    let walls = ["xmin", "xmax", "ymin", "ymax"];
    let mut m = model(&geom, &walls, NU);
    m.convection = conv;
    m.nutilde = ScalarField::from_fn("nuTilda", &geom, |x| NU * (10.0 + 5.0 * (2.0 * PI * x[0]).sin()));
    for w in walls {
        m.nutilde.set_bc(&geom, w, Box::new(ZeroGradient)).unwrap();
    }
    let c = SaCoefficients::default();
    let cw1 = c.cw1();
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let phi = stream_flux(&geom);
    let vol = geom.mesh.cell_volumes().to_vec();
    let d = m.wall_distance().to_vec();
    let dt = 0.05;
    let (mut x_old, mut x_n): (Option<Vec<f64>>, Vec<f64>) = (None, m.nutilde.values.clone());
    let mut dt_old = None;
    let (mut worst, mut least_change) = (0.0_f64, f64::INFINITY);
    for step in 1..=10 {
        m.begin_time_step(dt, scheme);
        let mut star = m.nutilde.values.clone();
        let mut star_b = m.nutilde.boundary_values(&geom);
        for s in 0..M {
            if s + 1 == M {
                star = m.nutilde.values.clone();
                star_b = m.nutilde.boundary_values(&geom);
            }
            m.update(&u, &phi);
        }
        let grad = green_gauss(&geom, &star, &star_b, 0);
        let (c0, c1, c2) = weights(scheme, dt, dt_old);
        let x = &m.nutilde.values;
        let (mut b, mut norm) = (0.0, 0.0);
        for i in 0..x.len() {
            let older = x_old.as_ref().map_or(0.0, |o| o[i]);
            let xs = star[i];
            let di = d[i].max(1e-30);
            let (p, dd) = if xs >= 0.0 {
                let (st, f) = sa_s_fw(xs, 0.0, di, NU, &c, SLimiter::Allmaras);
                (c.cb1 * st.max(0.0) * xs, cw1 * f * xs / (di * di))
            } else {
                (cw1 * xs * xs / (di * di), 0.0)
            };
            let g2: f64 = grad[i].iter().map(|v| v * v).sum();
            let q = c.cb2 / c.sigma * g2;
            b += vol[i] * ((c0 * x[i] - c1 * x_n[i] + c2 * older) / dt - p - q + dd * x[i]);
            norm += vol[i] * (p.abs() + q + (dd * x[i]).abs());
        }
        let big = x_n.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
        let change = x.iter().zip(&x_n).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max) / big;
        let neg = x.iter().filter(|&&v| v < 0.0).count();
        println!(
            "  AC1 {scheme:?} {conv:?} step {step}: |B_A| / sources {:.3e}, max |x' - x^n| / max |x^n| {change:.3e}, negative cells {neg}",
            b.abs() / norm
        );
        worst = worst.max(b.abs() / norm);
        least_change = least_change.min(change);
        x_old = Some(std::mem::replace(&mut x_n, x.clone()));
        dt_old = Some(dt);
    }
    (worst, least_change)
}

/// AC1: the global nuTilda budget closes: the time derivative balances the local sources, the transport sums to zero.
#[test]
fn ac1_global_nutilde_budget_closes() {
    for scheme in [TimeScheme::ImplicitEuler, TimeScheme::Bdf2] {
        for conv in [ConvScheme::Upwind, ConvScheme::LinearUpwind] {
            let (worst, least) = ac1_run(scheme, conv);
            println!(
                "AC1 {scheme:?} {conv:?}: largest |B_A| / sum V (|P*| + Q* + |D* x'|) {worst:.3e} (limit 1e-10); smallest per-step max |x' - x^n| / max |x^n| {least:.3e} (must be >= 1e-3)"
            );
            assert!(worst <= 1e-10, "AC1 {scheme:?} {conv:?}: budget {worst:.3e}");
            assert!(least >= 1e-3, "AC1 {scheme:?} {conv:?}: vacuous, change {least:.3e}");
        }
    }
}
