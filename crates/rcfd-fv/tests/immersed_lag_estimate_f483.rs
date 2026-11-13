//! F-483 (ADR 0388): the pressure-lag estimate at an immersed body.
//!
//! ADR 0374 found the pressure-lag estimate of ADR 0073 at 1.13 to 1.30 for DFG 2D-2 on M2 with the first-order immersed forcing
//! and one PISO corrector at dt 1e-3, a run that runs, with the mode in a solid cell at the surface. This file holds the
//! registered measurement of that number (the record H: the solid cells A1, the start A2, the tolerance A3), the estimate that
//! leaves out what is not a fluid mode (`PimpleSolver::lag_fluid = Some(LagFluid::FLUID)`: B1 to B3, C2), and the fast checks of
//! the new code (C1, U1 to U4). The DFG and circular Couette helpers are copies of those of `immersed_single_corrector_f363.rs`
//! (that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{LagAction, LagCheck, LagFluid, PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{axis_coords, box_2d, box_2d_coords, expansion_for_end_cell, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------------------
// The forcings of ADR 0374 (the table "The runs")
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `build`, `theta_min = 0.01`.
    F,
    /// `build_second_order`, `theta_min = 0.5` (weights at most 1).
    Sa,
    /// `build_second_order`, `theta_min = 0.25` (weights up to 2, no image).
    Sb,
    /// `build_second_order_bounded`, `theta_min = 0.1`.
    B1,
    /// `build_second_order_bounded`, `theta_min = 0.01`.
    B,
}

/// The forcing on `geom` with the penalty `k` and the viscosity `nu`; `check` is `flux_weight_check`.
fn forcing(geom: &FvGeometry, bodies: &[ImmersedBody], k: f64, nu: f64, f: Forcing, check: bool) -> ImmersedForcing {
    let gamma = uniform_gamma(geom, nu);
    let st = |theta_min: f64| {
        let mut s = ImmersedSettings::new(k);
        s.theta_min = theta_min;
        s
    };
    let mut ib = match f {
        Forcing::F => ImmersedForcing::build(geom, bodies, st(1e-2), &gamma),
        Forcing::Sa => ImmersedForcing::build_second_order(geom, bodies, st(0.5), &gamma),
        Forcing::Sb => ImmersedForcing::build_second_order(geom, bodies, st(0.25), &gamma),
        Forcing::B1 => ImmersedForcing::build_second_order_bounded(geom, bodies, st(0.1), &gamma),
        Forcing::B => ImmersedForcing::build_second_order_bounded(geom, bodies, st(1e-2), &gamma),
    }
    .unwrap();
    ib.flux_weight_check = check;
    ib
}

// ---------------------------------------------------------------------------------------------------------------------------
// DFG channel (ADR 0218), copied from immersed_single_corrector_f363.rs
// ---------------------------------------------------------------------------------------------------------------------------

const H: f64 = 0.41;
const NU: f64 = 1e-3;
const DIAM: f64 = 0.1;
const CENTRE: Vec3 = [0.2, 0.2, 0.0];
const DZ: f64 = 0.01;
const K_DFG: f64 = 1e7;
const DT: f64 = 1e-3;

fn geo_end(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(r)).unwrap()
}

fn geo_start(n: usize, len: f64, h: f64) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    axis_coords(n, len, Grading::Expansion(1.0 / r)).unwrap()
}

fn uniform(a: f64, b: f64, n: usize, centre: f64) -> Vec<f64> {
    let h = (b - a) / n as f64;
    let i0 = ((centre - a) / h).round() as i64;
    (0..=n).map(|i| centre + (i as i64 - i0) as f64 * h).collect()
}

fn join(parts: &[Vec<f64>], offsets: &[f64]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for (p, &o) in parts.iter().zip(offsets) {
        let start = usize::from(!out.is_empty());
        out.extend(p[start..].iter().map(|x| x + o));
    }
    out
}

fn bisect(xs: &[f64]) -> Vec<f64> {
    let mut out = vec![xs[0]];
    for w in xs.windows(2) {
        out.push(0.5 * (w[0] + w[1]));
        out.push(w[1]);
    }
    out
}

/// ADR 0218 set-up: M1 coordinates, bisected `level - 1` times.
fn dfg_family(level: usize) -> (Vec<f64>, Vec<f64>) {
    let h = 0.005;
    let mut xs = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.32, 40, 0.2),
            geo_start(38, 1.88, h),
        ],
        &[0.0, 0.0, 0.32],
    );
    let mut ys = join(
        &[
            geo_end(10, 0.12, h),
            uniform(0.12, 0.28, 32, 0.2),
            geo_start(10, 0.13, h),
        ],
        &[0.0, 0.0, 0.28],
    );
    for _ in 1..level {
        xs = bisect(&xs);
        ys = bisect(&ys);
    }
    (xs, ys)
}

fn inflow_mean(um: f64, y1: f64, y2: f64) -> f64 {
    let prim = |y: f64| H * y * y / 2.0 - y * y * y / 3.0;
    4.0 * um / (H * H) * (prim(y2) - prim(y1)) / (y2 - y1)
}

fn inflow_point(um: f64, y: f64) -> f64 {
    4.0 * um * y * (H - y) / (H * H)
}

fn dfg_body() -> ImmersedBody {
    ImmersedBody::fixed(
        "cylinder",
        SolidShape::Cylinder {
            centre: CENTRE,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5 * DIAM,
        },
    )
}

/// The DFG channel on the given coordinates (ADR 0218), without the forcing.
fn dfg_fields(xs: &[f64], ys: &[f64], um: f64) -> (Arc<FvGeometry>, VectorField, ScalarField) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d_coords(xs, ys, DZ).unwrap())).unwrap());
    let mut u = VectorField::from_fn("U", &geom, |c| [inflow_point(um, c[1]), 0.0, 0.0]);
    let cf = geom.mesh.face_centres();
    let prof: Vec<Vec3> = geom
        .mesh
        .patch("xmin")
        .unwrap()
        .faces()
        .map(|f| {
            let y = cf[f][1];
            let j = ys.partition_point(|&v| v < y) - 1;
            [inflow_mean(um, ys[j], ys[j + 1]), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "xmin", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    (geom, u, p)
}

/// The start field of the 2D-2 runs of ADR 0218 and 0249: the inflow profile everywhere plus a transverse blob.
fn two_d_two_start(geom: &FvGeometry, u: &mut VectorField, um: f64) {
    let cc = geom.mesh.cell_centres();
    for (c, x) in cc.iter().enumerate() {
        let blob = 0.2 * (-((x[0] - 0.35).powi(2) + (x[1] - 0.2).powi(2)) / (0.05 * 0.05)).exp();
        u.values[c] = [inflow_point(um, x[1]), blob, 0.0];
    }
}

/// The set-up R of ADR 0254 on M2 with `n_correctors` correctors and the forcing `f` (`check` = `flux_weight_check`).
fn r_solver(f: Forcing, n_correctors: usize, check: bool) -> (Arc<FvGeometry>, PimpleSolver) {
    let mut st = PimpleSettings::piso(NU);
    st.n_correctors = n_correctors;
    r_solver_with(f, st, check)
}

fn r_solver_with(f: Forcing, mut st: PimpleSettings, check: bool) -> (Arc<FvGeometry>, PimpleSolver) {
    let (xs, ys) = dfg_family(2);
    let um = 1.5;
    let (geom, mut u, p) = dfg_fields(&xs, &ys, um);
    two_d_two_start(&geom, &mut u, um);
    st.time_scheme = TimeScheme::Bdf2;
    st.convection = ConvScheme::LinearUpwind;
    st.amg_pressure = true;
    st.momentum_solver.tol_rel = 1e-6;
    st.pressure_solver.tol_rel = 1e-6;
    let mut s = PimpleSolver::new(geom.clone(), u, p, st);
    s.immersed = Some(forcing(&geom, &[dfg_body()], K_DFG, NU, f, check));
    (geom, s)
}

// ---------------------------------------------------------------------------------------------------------------------------
// Circular Couette flow (ADR 0218, 0234), copied from immersed_single_corrector_f363.rs
// ---------------------------------------------------------------------------------------------------------------------------

const CC_C: Vec3 = [1.2, 1.2, 0.0];

fn cc_bodies() -> Vec<ImmersedBody> {
    let mut inner = ImmersedBody::fixed(
        "inner",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: 0.5,
        },
    );
    inner.omega = [0.0, 0.0, 1.0];
    inner.origin = CC_C;
    let mut outer = ImmersedBody::fixed(
        "outer",
        SolidShape::Cylinder {
            centre: CC_C,
            axis: [0.0, 0.0, 1.0],
            radius: 1.0,
        },
    );
    outer.inverted = true;
    vec![inner, outer]
}

fn cc_geom(n: usize) -> (Arc<FvGeometry>, VectorField, ScalarField) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(n, n, 2.4, 2.4, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    (geom, u, p)
}

/// The forcing of circular Couette flow (`K = 1e8`, `nu = 1`), `theta_min` given, `check` = `flux_weight_check`.
fn cc_forcing(geom: &FvGeometry, kind: &str, theta_min: f64, check: bool) -> ImmersedForcing {
    let gamma = uniform_gamma(geom, 1.0);
    let mut st = ImmersedSettings::new(1e8);
    st.theta_min = theta_min;
    let mut ib = match kind {
        "build" => ImmersedForcing::build(geom, &cc_bodies(), st, &gamma),
        "second" => ImmersedForcing::build_second_order(geom, &cc_bodies(), st, &gamma),
        _ => ImmersedForcing::build_second_order_bounded(geom, &cc_bodies(), st, &gamma),
    }
    .unwrap();
    ib.flux_weight_check = check;
    ib
}

// ---------------------------------------------------------------------------------------------------------------------------
// The variants of the estimate (ADR 0388 "The runs")
// ---------------------------------------------------------------------------------------------------------------------------

/// Z: the zero start only.
const Z: LagFluid = LagFluid {
    zero_start: true,
    fluid_norm: false,
    tol_rel: None,
};
/// N: the fluid norm only.
const NF: LagFluid = LagFluid {
    zero_start: false,
    fluid_norm: true,
    tol_rel: None,
};
/// T: the tolerance 1e-12 only (start from the error).
const T: LagFluid = LagFluid {
    zero_start: false,
    fluid_norm: false,
    tol_rel: Some(1e-12),
};
/// Every part off (U1).
const OFF: LagFluid = LagFluid {
    zero_start: false,
    fluid_norm: false,
    tol_rel: None,
};

/// `lag_trial(dt, iterations, n)` of `s` with `lag_fluid = lf`; the solver is left as it was.
fn est(s: &mut PimpleSolver, lf: Option<LagFluid>, dt: f64, iterations: usize, n: usize) -> (f64, usize) {
    let keep = s.lag_fluid;
    s.lag_fluid = lf;
    let w = s.lag_trial(dt, iterations, n);
    s.lag_fluid = keep;
    w.map_or((f64::NAN, usize::MAX), |w| (w.rho, w.cell))
}

// ---------------------------------------------------------------------------------------------------------------------------
// U1 to U4: the fast checks of the new code on circular Couette flow at N 24 (the set-up of ADR 0374 U2)
// ---------------------------------------------------------------------------------------------------------------------------

/// The PISO solver of ADR 0234 V3 at N 24 (Euler, central, one corrector) with the forcing `build`, `theta_min = 1e-2`.
fn cc_solver() -> PimpleSolver {
    let (geom, u, p) = cc_geom(24);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    st.n_correctors = 1;
    let ib = cc_forcing(&geom, "build", 1e-2, true);
    let mut s = PimpleSolver::new(geom, u, p, st);
    s.immersed = Some(ib);
    s
}

const CC_DT: f64 = 2.5e-3;

fn cc_est(lf: Option<LagFluid>) -> (u64, usize) {
    let s = cc_solver();
    let mut s = s;
    let (rho, cell) = est(&mut s, lf, CC_DT, 20, 1);
    println!("U: lag_fluid {lf:?}: rho {rho:.6} cell {cell}");
    (rho.to_bits(), cell)
}

/// ADR 0388 U1: the option with every part off is the default estimate.
#[test]
fn u1_every_part_off_is_the_default() {
    assert_eq!(cc_est(Some(OFF)), cc_est(None), "U1");
}

/// ADR 0388 U2: each part changes the estimate.
#[test]
fn u2_each_part_changes_the_estimate() {
    let p = cc_est(None);
    for (name, lf) in [("Z", Z), ("N", NF), ("T", T)] {
        assert_ne!(cc_est(Some(lf)).0, p.0, "U2: {name} gives the bits of P");
    }
}

/// ADR 0388 U3: `lag_trial_mode` is `lag_trial` with the mode.
#[test]
fn u3_lag_trial_mode() {
    let s = cc_solver();
    let geom = s.geom.clone();
    let vol = geom.mesh.cell_volumes();
    let solid: Vec<bool> = s
        .immersed
        .as_ref()
        .unwrap()
        .body_of_cell
        .iter()
        .map(|b| b.is_some())
        .collect();
    assert!(solid.iter().any(|&x| x) && solid.iter().any(|&x| !x));
    for lf in [None, Some(LagFluid::FLUID)] {
        let mut s = cc_solver();
        s.lag_fluid = lf;
        let w = s.lag_trial(CC_DT, 20, 1).expect("U3: no estimate");
        let (wm, mode) = s.lag_trial_mode(CC_DT, 20, 1).expect("U3: no mode");
        assert_eq!(
            (wm.step, wm.rho.to_bits(), wm.cell),
            (w.step, w.rho.to_bits(), w.cell),
            "U3: {lf:?} the warning"
        );
        assert_eq!(mode.len(), vol.len());
        let fluid_only = lf.is_some();
        let take = |c: usize| !fluid_only || !solid[c];
        let arg = (0..mode.len())
            .filter(|&c| take(c))
            .max_by(|&a, &b| mode[a].abs().total_cmp(&mode[b].abs()))
            .unwrap();
        assert_eq!(arg, w.cell, "U3: {lf:?} the cell is not the argmax of the mode");
        if fluid_only {
            assert!(!solid[w.cell], "U3: the cell of FL is solid");
        }
        let nrm: f64 = (0..mode.len())
            .filter(|&c| take(c))
            .map(|c| mode[c] * mode[c] * vol[c])
            .sum::<f64>()
            .sqrt();
        println!("U3: {lf:?}: rho {:.6} cell {} norm {nrm:.15}", w.rho, w.cell);
        assert!((nrm - 1.0).abs() <= 1e-12, "U3: {lf:?} the norm of the mode {nrm}");
    }
}

/// ADR 0388 U4: the run-time check takes the option as `lag_trial` does.
#[test]
fn u4_run_time_check_with_the_option() {
    let mut a = cc_solver();
    a.lag_fluid = Some(LagFluid::FLUID);
    a.settings.pressure_lag_check = Some(LagCheck::new(0, LagAction::Warn));
    a.step(CC_DT);
    assert!(a.diverged.is_none());
    let wa = a.lag_estimates.first().copied().expect("U4: no estimate of the check");
    let mut b = cc_solver();
    b.lag_fluid = Some(LagFluid::FLUID);
    let wb = b.lag_trial(CC_DT, 20, 1).expect("U4: no estimate of lag_trial");
    println!(
        "U4: check rho {:.6} cell {}; lag_trial rho {:.6} cell {}",
        wa.rho, wa.cell, wb.rho, wb.cell
    );
    assert_eq!(
        (wa.step, wa.rho.to_bits(), wa.cell),
        (wb.step, wb.rho.to_bits(), wb.cell),
        "U4"
    );
    let p = cc_est(None);
    assert_ne!(wa.rho.to_bits(), p.0, "U4: the check with FL gives the bits of P");
}

// ---------------------------------------------------------------------------------------------------------------------------
// C1: the periodic Stokes box of ADR 0073 B2 (no body, converged solves)
// ---------------------------------------------------------------------------------------------------------------------------

/// The box of `w34b_lag.rs` `box_run` (N 32, BDF2, `pimple(nu, 2)`, tolerances 1e-10) with the diffusion number `d`, after 5
/// steps.
fn box_solver(d: f64) -> PimpleSolver {
    let n = 32;
    let h = 1.0 / n as f64;
    let dt = 0.01;
    let nu = d * 1.5 / (4.0 / (h * h) * dt);
    let mesh = Arc::new(box_2d(n, n, 1.0, 1.0, true, true).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let p = ScalarField::from_fn("p", &geom, |c| 1e-6 * (2.0 * PI * c[0]).cos() * (2.0 * PI * c[1]).cos());
    let mut st = PimpleSettings::pimple(nu, 2);
    st.convection = ConvScheme::LinearUpwind;
    st.time_scheme = TimeScheme::Bdf2;
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
    let mut s = PimpleSolver::new(geom, u, p, st);
    for _ in 0..5 {
        s.step(dt);
        assert!(s.diverged.is_none());
    }
    s
}

/// ADR 0388 C1: without a body and with converged solves the option does not move the estimate.
#[test]
fn c1_box_without_a_body() {
    for d in [1.0, 4.0] {
        let mut s = box_solver(d);
        let p = est(&mut s, None, 0.01, 20, 1);
        let f = est(&mut s, Some(LagFluid::FLUID), 0.01, 20, 1);
        let r = f.0 / p.0 - 1.0;
        println!(
            "C1 D {d}: rho P {:.6} (cell {}), FL {:.6} (cell {}), ratio - 1 {r:.3e}",
            p.0, p.1, f.0, f.1
        );
        assert!(r.abs() <= 1e-3, "C1 D {d}: rho P {} FL {}", p.0, f.0);
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The record H (R1, R2, A1 to A3, B1 to B3, C2)
// ---------------------------------------------------------------------------------------------------------------------------

const IT: usize = 40;

/// `a < b`, false for a NaN.
fn lt(a: f64, b: f64) -> bool {
    a < b
}

/// `a >= b`, false for a NaN.
fn ge(a: f64, b: f64) -> bool {
    a >= b
}

fn max_abs_u(u: &[Vec3]) -> f64 {
    u.iter().fold(0.0f64, |m, v| {
        let x = vec3::norm(*v);
        if x.is_nan() {
            f64::INFINITY
        } else {
            m.max(x)
        }
    })
}

fn surface_distance(x: Vec3) -> f64 {
    (vec3::norm(vec3::sub(x, CENTRE)) - 0.5 * DIAM).abs()
}

/// The estimates of F at one instant.
#[derive(Clone, Debug, Default)]
struct FEst {
    p: (f64, usize),
    share: f64,
    share_layer: f64,
    z: (f64, usize),
    t: (f64, usize),
    nf: (f64, usize),
    fl: (f64, usize),
    fl2: (f64, usize),
}

/// The estimates of the other forcings at one instant: P, FL, FL with two correctors.
#[derive(Clone, Copy, Debug)]
struct OEst {
    p: (f64, usize),
    fl: (f64, usize),
    fl2: (f64, usize),
}

/// The solid cells and the first solid layer (solid cells with a fluid face neighbour).
fn solid_sets(s: &PimpleSolver) -> (Vec<bool>, Vec<bool>) {
    let ib = s.immersed.as_ref().unwrap();
    let solid: Vec<bool> = ib.body_of_cell.iter().map(|b| b.is_some()).collect();
    let mut layer = vec![false; solid.len()];
    let (own, nei) = (s.geom.mesh.owner(), s.geom.mesh.neighbour());
    for f in 0..s.geom.n_internal {
        let (a, b) = (own[f], nei[f]);
        if solid[a] && !solid[b] {
            layer[a] = true;
        }
        if solid[b] && !solid[a] {
            layer[b] = true;
        }
    }
    (solid, layer)
}

fn f_estimates(s: &mut PimpleSolver) -> FEst {
    let (solid, layer) = solid_sets(s);
    let vol = s.geom.mesh.cell_volumes().to_vec();
    s.lag_fluid = None;
    let (w, mode) = s.lag_trial_mode(DT, IT, 1).expect("H: no mode");
    let e2 = |pick: &dyn Fn(usize) -> bool| -> f64 {
        (0..mode.len())
            .filter(|&c| pick(c))
            .map(|c| mode[c] * mode[c] * vol[c])
            .sum()
    };
    let all = e2(&|_| true);
    FEst {
        p: (w.rho, w.cell),
        share: e2(&|c| solid[c]) / all,
        share_layer: e2(&|c| layer[c]) / all,
        z: est(s, Some(Z), DT, IT, 1),
        t: est(s, Some(T), DT, IT, 1),
        nf: est(s, Some(NF), DT, IT, 1),
        fl: est(s, Some(LagFluid::FLUID), DT, IT, 1),
        fl2: est(s, Some(LagFluid::FLUID), DT, IT, 2),
    }
}

fn o_estimates(s: &mut PimpleSolver) -> OEst {
    OEst {
        p: est(s, None, DT, IT, 1),
        fl: est(s, Some(LagFluid::FLUID), DT, IT, 1),
        fl2: est(s, Some(LagFluid::FLUID), DT, IT, 2),
    }
}

enum Out {
    F([FEst; 2]),
    O(Forcing, [OEst; 2], Option<usize>, String),
    Channel((f64, usize), (f64, usize)),
}

/// F: the estimates at step 2 and at step 11 (10 steps run between).
fn run_f() -> Out {
    let (_, mut s) = r_solver(Forcing::F, 1, false);
    s.immersed_lag = true;
    let a = f_estimates(&mut s);
    for k in 1..=10 {
        s.step(DT);
        assert!(s.diverged.is_none(), "H: F stopped at step {k}");
    }
    let b = f_estimates(&mut s);
    Out::F([a, b])
}

/// Sa, Sb, B1, B: the estimates with the reconstruction at step 2 and 11; B runs on to its stop (at most 200 steps).
fn run_o(f: Forcing) -> Out {
    let (_, mut s) = r_solver(f, 1, false);
    s.immersed_lag = true;
    let a = o_estimates(&mut s);
    let mut stop = None;
    let mut reason = String::new();
    let mut b = None;
    let last = if f == Forcing::B { 200 } else { 10 };
    for n in 1..=last {
        s.step(DT);
        if let Some(d) = &s.diverged {
            stop = Some(n);
            reason = format!("{} {} {:.4e}", d.stage, d.quantity, d.value);
            break;
        }
        if n == 10 {
            b = Some(o_estimates(&mut s));
        }
    }
    if stop.is_none() && f == Forcing::B {
        reason = format!("max |u| {:.4e}", max_abs_u(&s.u.values));
    }
    let nan = OEst {
        p: (f64::NAN, usize::MAX),
        fl: (f64::NAN, usize::MAX),
        fl2: (f64::NAN, usize::MAX),
    };
    Out::O(f, [a, b.unwrap_or(nan)], stop, reason)
}

/// C2: the channel of R without the cylinder, step 2: P and FL.
fn run_channel() -> Out {
    let (_, mut s) = r_solver(Forcing::F, 1, false);
    s.immersed = None;
    let p = est(&mut s, None, DT, IT, 1);
    let f = est(&mut s, Some(LagFluid::FLUID), DT, IT, 1);
    Out::Channel(p, f)
}

/// ADR 0388 record H: R1, R2, A1, A2, A3, B1, B2, B3 and C2. Run once; every check is collected and the test fails with the
/// list of the missed ones.
#[test]
#[ignore = "heavy record (ADR 0388 H: R1, R2, A1 to A3, B1 to B3, C2): run once in release"]
fn h_lag_estimate_at_the_body_record() {
    let jobs: Vec<Box<dyn FnOnce() -> Out + Send>> = vec![
        Box::new(run_f),
        Box::new(|| run_o(Forcing::B)),
        Box::new(|| run_o(Forcing::Sa)),
        Box::new(|| run_o(Forcing::Sb)),
        Box::new(|| run_o(Forcing::B1)),
        Box::new(run_channel),
    ];
    let outs: Vec<Out> = std::thread::scope(|sc| {
        let hs: Vec<_> = jobs.into_iter().map(|j| sc.spawn(j)).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let (geom, s0) = r_solver(Forcing::B, 1, false);
    let cc = geom.mesh.cell_centres();
    let ib = s0.immersed.as_ref().unwrap();
    let (solid, layer) = solid_sets(&s0);
    println!(
        "H: {} cells, {} solid, {} in the first solid layer",
        solid.len(),
        solid.iter().filter(|&&x| x).count(),
        layer.iter().filter(|&&x| x).count()
    );
    let place = |e: (f64, usize)| {
        let c = e.1;
        if c < cc.len() {
            format!(
                "rho {:.5} cell {c} ({:.5}, {:.5}) solid {} surface distance {:.5}",
                e.0,
                cc[c][0],
                cc[c][1],
                ib.body_of_cell[c].is_some(),
                surface_distance(cc[c])
            )
        } else {
            format!("rho {:.5} no cell", e.0)
        }
    };
    let names = ["step 2", "step 11"];
    let mut missed = Vec::new();
    let mut fe: Option<[FEst; 2]> = None;
    let mut be: Option<[OEst; 2]> = None;
    for o in &outs {
        match o {
            Out::F(es) => {
                for (e, name) in es.iter().zip(names) {
                    println!(
                        "H F {name}: P {}; solid share {:.5}, first-layer share {:.5}",
                        place(e.p),
                        e.share,
                        e.share_layer
                    );
                    println!("H F {name}: Z {}", place(e.z));
                    println!("H F {name}: T {}", place(e.t));
                    println!("H F {name}: N {}", place(e.nf));
                    println!("H F {name}: FL {}", place(e.fl));
                    println!("H F {name}: FL two correctors {}", place(e.fl2));
                }
                fe = Some(es.clone());
            }
            Out::O(f, es, stop, reason) => {
                println!("H {f:?}: stop {stop:?} ({reason})");
                for (e, name) in es.iter().zip(names) {
                    println!("H {f:?} {name}: P {}", place(e.p));
                    println!("H {f:?} {name}: FL {}", place(e.fl));
                    println!("H {f:?} {name}: FL two correctors {}", place(e.fl2));
                }
                if *f == Forcing::B {
                    be = Some(*es);
                    // R2
                    if *stop != Some(34) || !reason.starts_with("end of step courant") {
                        missed.push(format!("R2: B stop {stop:?} ({reason}) against step 34"));
                    }
                }
            }
            Out::Channel(p, f) => {
                println!(
                    "H channel without the cylinder, step 2: P {}; FL {}",
                    place(*p),
                    place(*f)
                );
                // C2
                if !(ge(0.02, (f.0 - p.0).abs()) && lt(p.0, 1.0)) {
                    missed.push(format!("C2: P {} FL {}", p.0, f.0));
                }
            }
        }
    }
    let fe = fe.unwrap();
    let be = be.unwrap();
    // R1
    for (e, (name, want)) in fe.iter().zip([("step 2", 1.1986), ("step 11", 0.9983)]) {
        if !ge(1e-4, (e.p.0 - want).abs()) {
            missed.push(format!("R1: F {name} P {} against {want}", e.p.0));
        }
    }
    // A1, A2, A3, B1
    for (e, name) in fe.iter().zip(names) {
        if !ge(e.share, 0.5) {
            missed.push(format!("A1: F {name} solid share {}", e.share));
        }
        if !lt(e.z.0, 1.0) {
            missed.push(format!("A2: F {name} Z {}", e.z.0));
        }
        if !lt(e.t.0, 1.0) {
            missed.push(format!("A3: F {name} T {}", e.t.0));
        }
        if !lt(e.fl.0, 1.0) {
            missed.push(format!("B1: F {name} FL {}", e.fl.0));
        }
    }
    // B2
    let m = be[0].fl.0.max(be[1].fl.0);
    if !lt(1.0, m) {
        missed.push(format!(
            "B2: B max FL {m} (step 2 {}, step 11 {})",
            be[0].fl.0, be[1].fl.0
        ));
    }
    // B3
    for i in 0..2 {
        if !(lt(fe[i].fl2.0, 1.0) && lt(be[i].fl2.0, 1.0)) {
            missed.push(format!(
                "B3: {} FL two correctors F {} B {}",
                names[i], fe[i].fl2.0, be[i].fl2.0
            ));
        }
    }
    println!("missed: {missed:#?}");
    assert!(missed.is_empty(), "{missed:#?}");
}
