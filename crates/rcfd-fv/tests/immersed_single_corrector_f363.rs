//! F-363 (ADR 0374): the second-order immersed face flux with a single PISO corrector.
//!
//! ADR 0254 D11 and SB: DFG 2D-2 on M2 with one PISO corrector stops with a flux face of weight above 1 (step 126 with weights
//! up to 2, step 46 and step 34 with the bounded form) and runs with weights of at most 1 or with two correctors. This file
//! holds the fingerprints of the defaults measured before any library code of F-363 (N0), the measurement of the hypothesis
//! HL of ADR 0374 (R0, L1, L1n, L2, L3: heavy records, ignored, run once in release), the refusal (RF1, RF2) and the fast
//! checks of the new code (U1, U2). The DFG and circular Couette helpers are copies of those of `immersed_piso_stop_f355.rs`
//! (that file is not edited).

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
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
// DFG channel (ADR 0218), copied from immersed_piso_stop_f355.rs
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

fn fnv(words: impl Iterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

fn state_hash(u: &[Vec3], p: &[f64]) -> u64 {
    fnv(u
        .iter()
        .flat_map(|v| v.iter().map(|x| x.to_bits()))
        .chain(p.iter().map(|x| x.to_bits())))
}

// ---------------------------------------------------------------------------------------------------------------------------
// N0: the fingerprints of the defaults (measured on the unchanged tree 18c92d08 before any library code of F-363)
// ---------------------------------------------------------------------------------------------------------------------------

/// 20 steps of R with the forcing `f`, `n` correctors and `flux_weight_check = check`; the hash of `u` then `p`.
fn fp_run(f: Forcing, n: usize, check: bool) -> u64 {
    let (_, mut s) = r_solver(f, n, check);
    for k in 1..=20 {
        s.step(DT);
        assert!(s.diverged.is_none(), "N0: {f:?} with {n} correctors stopped at step {k}: {:?}", s.diverged);
    }
    state_hash(&s.u.values, &s.p.values)
}

/// `lag_trial(1e-3, 20, n)` of R with the forcing `f` at step 0: the bits of `rho` and the cell.
fn fp_lag(f: Forcing, n: usize) -> (u64, usize) {
    let (_, s) = r_solver(f, n, true);
    let w = s.lag_trial(DT, 20, n).expect("N0: no lag estimate");
    println!("N0: lag_trial {f:?} n {n}: rho {:.6} cell {}", w.rho, w.cell);
    (w.rho.to_bits(), w.cell)
}

/// ADR 0374 N0: FP1 (F, one corrector), FP2 (B, two correctors), FP3 (the lag estimates), FP4 (Sa, one corrector, check on).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn n0_fingerprints_of_the_defaults() {
    let fp1 = fp_run(Forcing::F, 1, true);
    let fp2 = fp_run(Forcing::B, 2, true);
    let fp3a = fp_lag(Forcing::F, 1);
    let fp3b = fp_lag(Forcing::B, 2);
    let fp4 = fp_run(Forcing::Sa, 1, true);
    println!("N0: FP1 {fp1:016x} FP2 {fp2:016x} FP3 {:016x}/{} {:016x}/{} FP4 {fp4:016x}", fp3a.0, fp3a.1, fp3b.0, fp3b.1);
    assert_eq!(fp1, 0xdd07702a17b9a36f, "FP1");
    assert_eq!(fp2, 0xe4d227de316e6de4, "FP2");
    assert_eq!(fp3a, (0x3ff4a94abc88ca99, 11855), "FP3 F");
    assert_eq!(fp3b, (0x3fb06aeaae8972f1, 11855), "FP3 B");
    assert_eq!(fp4, 0x279c4c4ed4cf89be, "FP4");
}
