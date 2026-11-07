//! ADR 0210 (F-294) part K, library checks of the transient k-epsilon sweep: the homogeneous set-up HK against the fully
//! implicit, the realizable, the variable-step and the lagged recursions of its decay (K1, K1r, K1v, K1b, the negative control
//! K1N), the closed form of the decay (K2, K2v, report-only K2b), and the global k budget of a convected, diffused, decaying
//! field (KC1). The limits are those registered in ADR 0210 before this code ran.
//!
//! HK: a two-block mesh. Block A, `box_2d(4, 4, 1, 1)` closed, whose sides are not walls (k and epsilon at their default zero
//! gradient); block B, one cell at `x` in [3, 4] whose four sides are the patch `wall`, the model's only wall (the wall
//! distance needs one). No cell of A is a wall cell and A and B share no face. `u = 0`, `nu = 0.01`, `k0 = eps0 = 1`. In A every
//! cell obeys `dk/dt = -eps`, `d eps/dt = -C2 eps^2 / k`, whose solution is `k = k0 (1 + t/t0)^(-n)`,
//! `eps = eps0 (1 + t/t0)^(-n-1)`, `n = 1/(C2 - 1)`, `t0 = n k0/eps0`.
//!
//! Records of defective set-ups (ignored): the first HK runs (`*_tol_record`, amendment 2: the wall row of block B, fixed by a
//! diagonal 1e8 times its own, sets the solver's stopping rule, so the epsilon of A stops near 1e-6 of its scale), and the runs
//! of amendment 2 with `tol_rel = 0` (`*_amendment2_record`, amendment 3: one K2 run drifted to 2.7e-8 in CG's 1000 iterations
//! on that stiff row). The corrected HK (amendment 3) scales block B by `(1, 1, 1e-12)`, so its rows are about 1e-4 of A's, and
//! solves with the registered controls. KC1 keeps the registered block B and controls and its run of record.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::edit::{merge_meshes, merge_patches, transform_mesh, Affine};
use rcfd_mesh::gen::box_2d;
use rcfd_turb::{KEpsilon, KEpsilonVariant};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 0.01;
const K0: f64 = 1.0;
const E0: f64 = 1.0;
/// Sweeps per step that stand for the converged outer iterations.
const M: usize = 30;

fn c2_of(v: KEpsilonVariant) -> f64 {
    match v {
        KEpsilonVariant::Standard => 1.92,
        KEpsilonVariant::Rng => 1.68,
        KEpsilonVariant::Realizable => 1.9,
    }
}

/// `t0 = n k0 / eps0`, `n = 1/(C2 - 1)`.
fn t0(c2: f64) -> f64 {
    K0 / ((c2 - 1.0) * E0)
}

/// The closed form at `t`: (k, epsilon).
fn closed(c2: f64, t: f64) -> (f64, f64) {
    let n = 1.0 / (c2 - 1.0);
    let s = 1.0 + t / t0(c2);
    (K0 * s.powf(-n), E0 * s.powf(-n - 1.0))
}

/// The set-ups of HK: as registered (block B of unit volume, `tol_rel = 1e-14`; record of amendment 2), as corrected by
/// amendment 2 (`tol_rel = 0`; record of amendment 3) and as corrected by amendment 3 (block B of volume 1e-12,
/// `tol_rel = 1e-14`). KC1 keeps the registered one.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Setup {
    Registered,
    Amendment2,
    Corrected,
}

fn controls(setup: Setup) -> SolveControl {
    SolveControl {
        tol_abs: 1e-300,
        tol_rel: match setup {
            Setup::Amendment2 => 0.0,
            Setup::Registered | Setup::Corrected => 1e-14,
        },
        max_iter: 1000,
    }
}

/// The two-block mesh of HK (A: `n x n`; B thin for the set-up of amendment 3) and the cell count of A. Checks the mesh part of
/// HK0.
fn geometry(n: usize, setup: Setup) -> (Arc<FvGeometry>, usize) {
    let a = box_2d(n, n, 1.0, 1.0, false, false).unwrap();
    let b = box_2d(1, 1, 1.0, 1.0, false, false).unwrap();
    let b = if setup == Setup::Corrected {
        transform_mesh(&b, &Affine::scaling([1.0, 1.0, 1e-12])).unwrap()
    } else {
        b
    };
    let b = transform_mesh(&b, &Affine::translation([3.0, 0.0, 0.0])).unwrap();
    let b = merge_patches(&b, &["xmin", "xmax", "ymin", "ymax"], "wall").unwrap();
    let mesh = merge_meshes(&a, &b, 1e-9).unwrap();
    let na = a.n_cells();
    assert_eq!(mesh.n_cells(), na + 1, "HK0: cell count");
    let cc = mesh.cell_centres();
    assert!(
        cc[..na].iter().all(|c| c[0] < 1.0 && c[1] < 1.0) && cc[na][0] > 3.0,
        "HK0: the cells of A do not come first"
    );
    let own = mesh.owner();
    let wall = mesh.patch("wall").expect("HK0: no patch wall");
    assert!(
        wall.faces().all(|f| own[f] >= na),
        "HK0: a wall face has its owner in A"
    );
    let nei = mesh.neighbour();
    for f in 0..mesh.n_internal_faces() {
        assert_eq!(own[f] < na, nei[f] < na, "HK0: internal face {f} joins A and B");
    }
    if setup == Setup::Corrected {
        let v = mesh.cell_volumes()[na];
        assert!((v / 1e-12 - 1.0).abs() < 1e-9, "HK0: volume of B {v:e}");
    }
    (Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap()), na)
}

/// The model of HK and KC1: the wall `wall` (block B), the solver controls of `setup`.
fn model(geom: &Arc<FvGeometry>, variant: KEpsilonVariant, setup: Setup) -> KEpsilon {
    let mut m = KEpsilon::new(variant, geom.clone(), NU, &["wall"], K0, E0).unwrap();
    m.solver = controls(setup);
    m
}

fn spread(v: &[f64]) -> f64 {
    let (lo, hi) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    (hi - lo) / hi.abs()
}

/// HK0 (field part): k and epsilon uniform over A to 1e-13.
fn hk0(m: &KEpsilon, na: usize, what: &str) {
    let (sk, se) = (spread(&m.k.values[..na]), spread(&m.eps.values[..na]));
    assert!(
        sk <= 1e-13 && se <= 1e-13,
        "HK0 {what}: spread k {sk:.3e}, epsilon {se:.3e}"
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

/// Reference recursions of the decay: the fully implicit step (standard / RNG closed root, realizable by bisection) and the
/// lagged one-sweep Euler step.
#[derive(Clone, Copy)]
struct Ref {
    c2: f64,
    realizable: bool,
    k: f64,
    e: f64,
    k_old: f64,
    e_old: f64,
    dt_old: Option<f64>,
}

impl Ref {
    fn new(variant: KEpsilonVariant) -> Self {
        Ref {
            c2: c2_of(variant),
            realizable: variant == KEpsilonVariant::Realizable,
            k: K0,
            e: E0,
            k_old: K0,
            e_old: E0,
            dt_old: None,
        }
    }

    fn advance(&mut self, k: f64, e: f64, dt: f64) {
        self.k_old = self.k;
        self.e_old = self.e;
        self.k = k;
        self.e = e;
        self.dt_old = Some(dt);
    }

    fn implicit(&mut self, scheme: TimeScheme, dt: f64) {
        let (c0, c1, c2w) = weights(scheme, dt, self.dt_old);
        let rk = c1 * self.k - c2w * self.k_old;
        let re = c1 * self.e - c2w * self.e_old;
        let c2 = self.c2;
        let e = if self.realizable {
            // the root in (0, R_e / c0) of c0 e - R_e + dt C2 e^2 / ((R_k - dt e)/c0 + sqrt(nu e)) = 0
            let f = |x: f64| c0 * x - re + dt * c2 * x * x / ((rk - dt * x) / c0 + (NU * x).sqrt());
            let (mut lo, mut hi) = (0.0, re / c0);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if f(mid) > 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            0.5 * (lo + hi)
        } else {
            let a = dt * c0 * (c2 - 1.0);
            let b = c0 * rk + dt * re;
            2.0 * re * rk / (b + (b * b + 4.0 * a * re * rk).sqrt())
        };
        let k = (rk - dt * e) / c0;
        self.advance(k, e, dt);
    }

    fn lagged(&mut self, dt: f64) {
        let k = self.k / (1.0 + dt * self.e / self.k);
        let e = self.e / (1.0 + dt * self.c2 * self.e / k);
        self.advance(k, e, dt);
    }
}

/// Runs HK over the steps `dts` with `sweeps` sweeps per step; after each step calls `each(step, k, epsilon)` with the values
/// of cell 0 (HK0 is checked after every sweep). Returns the final (k, epsilon).
fn run_h(
    setup: Setup,
    variant: KEpsilonVariant,
    scheme: TimeScheme,
    dts: &[f64],
    sweeps: usize,
    mut each: impl FnMut(usize, f64, f64),
) -> (f64, f64) {
    let (geom, na) = geometry(4, setup);
    let mut m = model(&geom, variant, setup);
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let phi = vec![0.0; geom.mesh.n_faces()];
    hk0(&m, na, "initial");
    for (i, &dt) in dts.iter().enumerate() {
        m.begin_step(dt, scheme);
        assert!(m.is_transient());
        for s in 0..sweeps {
            m.update(&u, &phi);
            hk0(&m, na, &format!("{setup:?} {variant:?} step {} sweep {}", i + 1, s + 1));
        }
        each(i + 1, m.k.values[0], m.eps.values[0]);
    }
    (m.k.values[0], m.eps.values[0])
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// The K1 step: `dt C2 eps0 / k0 = 0.1`.
fn k1_dt(c2: f64) -> f64 {
    0.1 * K0 / (c2 * E0)
}

/// Steps alternating `1.2 h` and `0.8 h`, `h = T / n`, `n` steps.
fn alternating(t: f64, n: usize) -> Vec<f64> {
    let h = t / n as f64;
    (0..n).map(|i| if i % 2 == 0 { 1.2 * h } else { 0.8 * h }).collect()
}

/// Largest relative error of the run against the implicit recursion over the steps.
fn against_implicit(setup: Setup, variant: KEpsilonVariant, scheme: TimeScheme, dts: &[f64], sweeps: usize) -> f64 {
    let mut r = Ref::new(variant);
    let mut worst = 0.0_f64;
    run_h(setup, variant, scheme, dts, sweeps, |i, k, e| {
        r.implicit(scheme, dts[i - 1]);
        worst = worst.max(rel(k, r.k)).max(rel(e, r.e));
    });
    worst
}

fn k1(setup: Setup) {
    for variant in [KEpsilonVariant::Standard, KEpsilonVariant::Rng] {
        let dts = vec![k1_dt(c2_of(variant)); 40];
        for scheme in [TimeScheme::ImplicitEuler, TimeScheme::Bdf2] {
            let worst = against_implicit(setup, variant, scheme, &dts, M);
            println!(
                "K1 {setup:?} {variant:?} {scheme:?}: largest relative error against the fully implicit recursion {worst:.3e} (limit 1e-12)"
            );
            assert!(worst <= 1e-12, "K1 {variant:?} {scheme:?}: {worst:.3e}");
        }
    }
}

/// K1: with converged sweeps every step is the fully implicit step of the scheme (standard and RNG).
#[test]
fn k1_converged_sweeps_give_the_fully_implicit_recursion() {
    k1(Setup::Corrected)
}

fn k1r(setup: Setup) {
    let variant = KEpsilonVariant::Realizable;
    let dts = vec![k1_dt(c2_of(variant)); 40];
    for scheme in [TimeScheme::ImplicitEuler, TimeScheme::Bdf2] {
        let worst = against_implicit(setup, variant, scheme, &dts, M);
        println!(
            "K1r {setup:?} {scheme:?}: largest relative error against the realizable implicit recursion {worst:.3e} (limit 1e-12)"
        );
        assert!(worst <= 1e-12, "K1r {scheme:?}: {worst:.3e}");
    }
}

/// K1r: the realizable variant against its fully implicit recursion (no closed form).
#[test]
fn k1r_realizable_recursion() {
    k1r(Setup::Corrected)
}

fn k1v(setup: Setup) {
    let v = KEpsilonVariant::Standard;
    let worst = against_implicit(setup, v, TimeScheme::Bdf2, &alternating(t0(c2_of(v)), 40), M);
    println!(
        "K1v {setup:?}: largest relative error against the variable-step BDF2 recursion {worst:.3e} (limit 1e-12)"
    );
    assert!(worst <= 1e-12, "K1v: {worst:.3e}");
}

/// K1v: the variable-step BDF2 recursion (steps alternating 1.2 h and 0.8 h, h = t0 / 40).
#[test]
fn k1v_variable_step_bdf2_recursion() {
    k1v(Setup::Corrected)
}

/// The one-sweep Euler run: (largest error against the lagged recursion, against the implicit recursion).
fn one_sweep(setup: Setup) -> (f64, f64) {
    let v = KEpsilonVariant::Standard;
    let dts = vec![k1_dt(c2_of(v)); 40];
    let (mut lag, mut imp) = (Ref::new(v), Ref::new(v));
    let (mut e_lag, mut e_imp) = (0.0_f64, 0.0_f64);
    run_h(setup, v, TimeScheme::ImplicitEuler, &dts, 1, |i, k, e| {
        lag.lagged(dts[i - 1]);
        imp.implicit(TimeScheme::ImplicitEuler, dts[i - 1]);
        e_lag = e_lag.max(rel(k, lag.k)).max(rel(e, lag.e));
        e_imp = e_imp.max(rel(k, imp.k)).max(rel(e, imp.e));
    });
    (e_lag, e_imp)
}

fn k1b(setup: Setup) {
    let (e_lag, _) = one_sweep(setup);
    println!("K1b {setup:?}: largest relative error against the lagged recursion {e_lag:.3e} (limit 1e-12)");
    assert!(e_lag <= 1e-12, "K1b: {e_lag:.3e}");
}

/// K1b: one sweep per step is the lagged (once-per-step) update.
#[test]
fn k1b_one_sweep_gives_the_lagged_recursion() {
    k1b(Setup::Corrected)
}

fn k1n(setup: Setup) {
    let (_, e_imp) = one_sweep(setup);
    println!(
        "K1N {setup:?}: largest relative difference from the fully implicit recursion {e_imp:.3e} (must be >= 1e-3)"
    );
    assert!(e_imp >= 1e-3, "K1N did not fire: {e_imp:.3e}");
}

/// K1N: the one-sweep run misses the fully implicit recursion (K1 can fail).
#[test]
fn k1n_one_sweep_misses_the_implicit_recursion() {
    k1n(Setup::Corrected)
}

/// Largest relative errors (k, epsilon) over the steps against the closed form.
fn max_error(setup: Setup, variant: KEpsilonVariant, scheme: TimeScheme, dts: &[f64]) -> (f64, f64) {
    let c2 = c2_of(variant);
    let (mut t, mut ek, mut ee) = (0.0, 0.0_f64, 0.0_f64);
    run_h(setup, variant, scheme, dts, M, |i, k, e| {
        t += dts[i - 1];
        let (kc, ec) = closed(c2, t);
        ek = ek.max(rel(k, kc));
        ee = ee.max(rel(e, ec));
    });
    (ek, ee)
}

/// (e(n1), e(n2), p) for k and epsilon.
fn order(setup: Setup, variant: KEpsilonVariant, scheme: TimeScheme, a: &[f64], b: &[f64]) -> [(f64, f64, f64); 2] {
    let (ea, eb) = (
        max_error(setup, variant, scheme, a),
        max_error(setup, variant, scheme, b),
    );
    [(ea.0, eb.0, (ea.0 / eb.0).log2()), (ea.1, eb.1, (ea.1 / eb.1).log2())]
}

fn equal(t: f64, n: usize) -> Vec<f64> {
    vec![t / n as f64; n]
}

fn k2(setup: Setup) {
    for variant in [KEpsilonVariant::Standard, KEpsilonVariant::Rng] {
        let t = t0(c2_of(variant));
        for (scheme, band, cap) in [
            (TimeScheme::ImplicitEuler, (0.9, 1.1), 5e-3),
            (TimeScheme::Bdf2, (1.9, 2.1), 1e-4),
        ] {
            let o = order(setup, variant, scheme, &equal(t, 160), &equal(t, 320));
            for (name, (e160, e320, p)) in ["k", "epsilon"].iter().zip(o) {
                println!(
                    "K2 {setup:?} {variant:?} {scheme:?} {name}: e(160) {e160:.4e}, e(320) {e320:.4e}, p {p:.4} (band [{}, {}], e(320) <= {cap:e})",
                    band.0, band.1
                );
                assert!(p >= band.0 && p <= band.1, "K2 {variant:?} {scheme:?} {name}: p {p}");
                assert!(e320 <= cap, "K2 {variant:?} {scheme:?} {name}: e(320) {e320:.4e}");
            }
        }
    }
}

/// K2: the order and the largest error of Euler and BDF2 against the closed form, N = 160 and 320 to T = t0.
#[test]
fn k2_order_against_the_closed_form() {
    k2(Setup::Corrected)
}

fn k2v(setup: Setup) {
    let v = KEpsilonVariant::Standard;
    let t = t0(c2_of(v));
    let o = order(setup, v, TimeScheme::Bdf2, &alternating(t, 160), &alternating(t, 320));
    for (name, (e160, e320, p)) in ["k", "epsilon"].iter().zip(o) {
        println!(
            "K2v {setup:?} {name}: e(160) {e160:.4e}, e(320) {e320:.4e}, p {p:.4} (band [1.9, 2.1], e(320) <= 1e-4)"
        );
        assert!((1.9..=2.1).contains(&p), "K2v {name}: p {p}");
        assert!(e320 <= 1e-4, "K2v {name}: e(320) {e320:.4e}");
    }
}

/// K2v: variable-step BDF2 against the closed form.
#[test]
fn k2v_variable_step_bdf2_order() {
    k2v(Setup::Corrected)
}

fn k2b(setup: Setup) {
    let v = KEpsilonVariant::Standard;
    let t = t0(c2_of(v));
    let o = order(setup, v, TimeScheme::Bdf2Bounded, &equal(t, 160), &equal(t, 320));
    for (name, (e160, e320, p)) in ["k", "epsilon"].iter().zip(o) {
        println!("K2b (report only) {setup:?} Bdf2Bounded {name}: e(160) {e160:.4e}, e(320) {e320:.4e}, p {p:.4}");
        assert!(e320.is_finite());
    }
}

/// K2b: bounded BDF2, report only.
#[test]
fn k2b_bounded_bdf2_report() {
    k2b(Setup::Corrected)
}

/// The flux of `psi = sin(pi x) sin(pi y) / pi` on the side faces of block A (ADR 0199 amendment 2); every boundary face and
/// the front and back carry 0. Checks C1-0 (divergence-free cells, no flux through the boundary).
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

/// KC1 for one scheme and convection: the largest relative budget residual over the steps and the smallest per-step change.
fn kc1_run(scheme: TimeScheme, conv: ConvScheme) -> (f64, f64) {
    let (geom, na) = geometry(16, Setup::Registered);
    let mut m = model(&geom, KEpsilonVariant::Standard, Setup::Registered);
    m.convection = conv;
    m.k = ScalarField::from_fn("k", &geom, |x| 1.0 + 0.5 * (2.0 * PI * x[0]).sin());
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let phi = stream_flux(&geom);
    let vol = geom.mesh.cell_volumes().to_vec();
    let dt = 0.05;
    let (mut k_old, mut k_n): (Option<Vec<f64>>, Vec<f64>) = (None, m.k.values.clone());
    let mut dt_old = None;
    let (mut worst, mut least_change) = (0.0_f64, f64::INFINITY);
    for step in 1..=10 {
        m.begin_step(dt, scheme);
        let (mut k_star, mut e_star) = (m.k.values.clone(), m.eps.values.clone());
        for s in 0..M {
            if s + 1 == M {
                k_star = m.k.values.clone();
                e_star = m.eps.values.clone();
            }
            m.update(&u, &phi);
        }
        let (c0, c1, c2) = weights(scheme, dt, dt_old);
        let k = &m.k.values;
        let (mut b, mut norm) = (0.0, 0.0);
        for c in 0..na {
            let older = k_old.as_ref().map_or(0.0, |o| o[c]);
            let sink = e_star[c] / k_star[c].max(1e-30) * k[c];
            b += vol[c] * ((c0 * k[c] - c1 * k_n[c] + c2 * older) / dt + sink);
            norm += vol[c] * sink;
        }
        let change = k[..na]
            .iter()
            .zip(&k_n[..na])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        println!(
            "  KC1 {scheme:?} {conv:?} step {step}: |B_K| / sink {:.3e}, max |k' - k^n| {change:.3e}",
            b.abs() / norm
        );
        worst = worst.max(b.abs() / norm);
        least_change = least_change.min(change);
        k_old = Some(std::mem::replace(&mut k_n, k.clone()));
        dt_old = Some(dt);
    }
    (worst, least_change)
}

/// KC1: the global k budget over A closes: the time derivative balances the dissipation, the transport sums to zero.
#[test]
fn kc1_global_k_budget_closes() {
    for scheme in [TimeScheme::ImplicitEuler, TimeScheme::Bdf2] {
        for conv in [ConvScheme::Upwind, ConvScheme::LinearUpwind] {
            let (worst, least) = kc1_run(scheme, conv);
            println!(
                "KC1 {scheme:?} {conv:?}: largest |B_K| / sum V (eps*/k*) k' {worst:.3e} (limit 1e-10); smallest per-step max |k' - k^n| {least:.3e} (must be >= 1e-3)"
            );
            assert!(worst <= 1e-10, "KC1 {scheme:?} {conv:?}: budget {worst:.3e}");
            assert!(least >= 1e-3, "KC1 {scheme:?} {conv:?}: vacuous, change {least:.3e}");
        }
    }
}

// ---------------------------------------------------------------- records of the registered solver controls (amendment 2)

#[test]
#[ignore = "record of the registered solver controls (ADR 0210 amendment 2): HK0 epsilon spread 1.017e-6 at step 1 sweep 1"]
fn k1_tol_record() {
    k1(Setup::Registered)
}

#[test]
#[ignore = "record of the registered solver controls (ADR 0210 amendment 2): HK0 epsilon spread 1.255e-6 at step 1 sweep 1"]
fn k1r_tol_record() {
    k1r(Setup::Registered)
}

#[test]
#[ignore = "record of the registered solver controls (ADR 0210 amendment 2): HK0 epsilon spread 1.355e-7 at step 1 sweep 1"]
fn k1v_tol_record() {
    k1v(Setup::Registered)
}

#[test]
#[ignore = "record of the registered solver controls (ADR 0210 amendment 2): HK0 epsilon spread 1.017e-6 at step 1 sweep 1"]
fn k1b_tol_record() {
    k1b(Setup::Registered)
}

#[test]
#[ignore = "record of the registered solver controls (ADR 0210 amendment 2): HK0 epsilon spread 1.017e-6 at step 1 sweep 1"]
fn k1n_tol_record() {
    k1n(Setup::Registered)
}

#[test]
#[ignore = "record of the registered solver controls (ADR 0210 amendment 2): HK0 epsilon spread 1.820e-6 at step 1 sweep 1"]
fn k2_tol_record() {
    k2(Setup::Registered)
}

#[test]
#[ignore = "record of the registered solver controls (ADR 0210 amendment 2): HK0 epsilon spread 3.107e-6 at step 1 sweep 1"]
fn k2v_tol_record() {
    k2v(Setup::Registered)
}

#[test]
#[ignore = "record of the registered solver controls (ADR 0210 amendment 2): HK0 epsilon spread 1.820e-6 at step 1 sweep 1"]
fn k2b_tol_record() {
    k2b(Setup::Registered)
}

// ---------------------------------------------------------------- records of the set-up of amendment 2 (amendment 3)

#[test]
#[ignore = "record of the set-up of amendment 2 (ADR 0210 amendment 3); met in its run"]
fn k1_amendment2_record() {
    k1(Setup::Amendment2)
}

#[test]
#[ignore = "record of the set-up of amendment 2 (ADR 0210 amendment 3); met in its run"]
fn k1r_amendment2_record() {
    k1r(Setup::Amendment2)
}

#[test]
#[ignore = "record of the set-up of amendment 2 (ADR 0210 amendment 3); met in its run"]
fn k1v_amendment2_record() {
    k1v(Setup::Amendment2)
}

#[test]
#[ignore = "record of the set-up of amendment 2 (ADR 0210 amendment 3); met in its run"]
fn k1b_amendment2_record() {
    k1b(Setup::Amendment2)
}

#[test]
#[ignore = "record of the set-up of amendment 2 (ADR 0210 amendment 3); met in its run"]
fn k1n_amendment2_record() {
    k1n(Setup::Amendment2)
}

#[test]
#[ignore = "record of the set-up of amendment 2 (ADR 0210 amendment 3): HK0 epsilon spread 2.676e-8 at RNG Euler N 160 step 16 sweep 2"]
fn k2_amendment2_record() {
    k2(Setup::Amendment2)
}

#[test]
#[ignore = "record of the set-up of amendment 2 (ADR 0210 amendment 3); met in its run"]
fn k2v_amendment2_record() {
    k2v(Setup::Amendment2)
}

#[test]
#[ignore = "record of the set-up of amendment 2 (ADR 0210 amendment 3); reported in its run"]
fn k2b_amendment2_record() {
    k2b(Setup::Amendment2)
}
