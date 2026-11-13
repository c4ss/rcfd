//! F-363 (ADR 0374): the second-order immersed face flux with a single PISO corrector.
//!
//! ADR 0254 D11 and SB: DFG 2D-2 on M2 with one PISO corrector stops with a flux face of weight above 1 (step 126 with weights
//! up to 2, step 46 and step 34 with the bounded form) and runs with weights of at most 1 or with two correctors. This file
//! holds the fingerprints of the defaults measured before any library code of F-363 (N0), the measurement of the hypothesis
//! HL of ADR 0374 (R0, L1, L1n, L2, L3: heavy records, ignored, run once in release), the refusal (RF1, RF2) and the fast
//! checks of the new code (U1, U2). The DFG and circular Couette helpers are copies of those of `immersed_piso_stop_f355.rs`
//! (that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{axis_coords, box_2d, box_2d_coords, expansion_for_end_cell, Grading};
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
        assert!(
            s.diverged.is_none(),
            "N0: {f:?} with {n} correctors stopped at step {k}: {:?}",
            s.diverged
        );
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
    println!(
        "N0: FP1 {fp1:016x} FP2 {fp2:016x} FP3 {:016x}/{} {:016x}/{} FP4 {fp4:016x}",
        fp3a.0, fp3a.1, fp3b.0, fp3b.1
    );
    assert_eq!(fp1, 0xdd07702a17b9a36f, "FP1");
    assert_eq!(fp2, 0xe4d227de316e6de4, "FP2");
    assert_eq!(fp3a, (0x3ff4a94abc88ca99, 11855), "FP3 F");
    assert_eq!(fp3b, (0x3fb06aeaae8972f1, 11855), "FP3 B");
    assert_eq!(fp4, 0x279c4c4ed4cf89be, "FP4");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Circular Couette flow (ADR 0218, 0234), copied from immersed_piso_stop_f355.rs: U1 and U2
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

/// ADR 0374 U1: `single_corrector_face` on circular Couette flow at N 24.
#[test]
fn u1_single_corrector_face() {
    let (geom, _, _) = cc_geom(24);
    assert_eq!(
        cc_forcing(&geom, "build", 1e-2, true).single_corrector_face(),
        None,
        "U1: build"
    );
    let sa = cc_forcing(&geom, "second", 0.5, true);
    assert!(!sa.flux_faces.is_empty());
    assert_eq!(
        sa.single_corrector_face(),
        None,
        "U1: build_second_order with theta_min 0.5"
    );
    let b = cc_forcing(&geom, "bounded", 1e-2, true);
    let wmax = b.flux_faces.iter().fold(0.0f64, |m, ff| m.max(ff.weight));
    let (c, w) = b.single_corrector_face().expect("U1: the bounded form is not reported");
    assert!(wmax > 1.0);
    assert_eq!(w, wmax, "U1: the reported weight");
    assert!(
        b.flux_faces.iter().any(|ff| ff.fluid == c && ff.weight == w),
        "U1: cell {c} is not the fluid cell of the face of the largest weight"
    );
    let mut off = b.clone();
    off.flux_weight_check = false;
    assert_eq!(off.single_corrector_face(), None, "U1: the check off");
}

/// `lag_trial(2.5e-3, 20, 1)` of the PISO run of ADR 0234 V3 at N 24 (Euler, central) with the forcing `ib`.
fn cc_lag(ib: &ImmersedForcing, immersed_lag: bool) -> (u64, usize) {
    let (geom, u, p) = cc_geom(24);
    let mut st = PimpleSettings::piso(1.0);
    st.convection = ConvScheme::Central;
    let mut s = PimpleSolver::new(geom, u, p, st);
    s.immersed = Some(ib.clone());
    s.immersed_lag = immersed_lag;
    let w = s.lag_trial(2.5e-3, 20, 1).expect("U2: no estimate");
    println!(
        "U2: flux faces {} immersed_lag {immersed_lag}: rho {:.6} cell {}",
        ib.flux_faces.len(),
        w.rho,
        w.cell
    );
    (w.rho.to_bits(), w.cell)
}

/// ADR 0374 U2: the lag estimate with `immersed_lag` is the same without flux faces and differs with them.
#[test]
fn u2_lag_map_with_the_reconstruction() {
    let (geom, _, _) = cc_geom(24);
    let first = cc_forcing(&geom, "build", 1e-2, true);
    assert_eq!(cc_lag(&first, true), cc_lag(&first, false), "U2: build");
    let second = cc_forcing(&geom, "second", 1e-2, false);
    let (a, b) = (cc_lag(&second, true), cc_lag(&second, false));
    assert_ne!(a.0, b.0, "U2: the reconstruction does not change the estimate");
}

// ---------------------------------------------------------------------------------------------------------------------------
// RF1, RF2: the refusal
// ---------------------------------------------------------------------------------------------------------------------------

/// ADR 0374 RF1 (the bounded form and the unbounded form with weights up to 2, one corrector, refused at step 1 with a located
/// message) and RF2 (two correctors, weights of at most 1, the first-order forcing and PIMPLE are not refused).
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn rf_refusal_with_one_corrector() {
    for f in [Forcing::B, Forcing::Sb] {
        let (geom, mut s) = r_solver(f, 1, true);
        s.step(DT);
        let d = s
            .diverged
            .clone()
            .unwrap_or_else(|| panic!("RF1: {f:?} with one corrector is not refused"));
        let msg = d.message(Some(geom.mesh.cell_centres()));
        println!("RF1 {f:?}: {msg}");
        assert_eq!(d.step, 1, "RF1 {f:?}");
        assert_eq!(d.stage, "pressure correctors", "RF1 {f:?}");
        assert_eq!(d.quantity, "immersed flux corrector", "RF1 {f:?}");
        assert!(d.value > 1.0, "RF1 {f:?}: value {}", d.value);
        let c = d.cell.expect("RF1: no cell");
        let ib = s.immersed.as_ref().unwrap();
        assert!(
            ib.flux_faces
                .iter()
                .any(|ff| ff.fluid == c && (ff.image.is_some() || ff.weight > 1.0)),
            "RF1 {f:?}: cell {c} is not the fluid cell of a reversed face"
        );
        assert!(
            msg.contains("ADR 0374") && msg.contains("n_correctors") && msg.contains(&format!("cell {c}")),
            "RF1 {f:?}: {msg}"
        );
    }
    let not_refused = |name: &str, f: Forcing, st: PimpleSettings, steps: usize| {
        let (_, mut s) = r_solver_with(f, st, true);
        for k in 1..=steps {
            s.step(DT);
            assert!(
                s.diverged.is_none(),
                "RF2: {name} stopped at step {k}: {:?}",
                s.diverged
            );
        }
    };
    let piso = |n: usize| {
        let mut st = PimpleSettings::piso(NU);
        st.n_correctors = n;
        st
    };
    not_refused("B with two correctors", Forcing::B, piso(2), 5);
    not_refused("Sa with one corrector", Forcing::Sa, piso(1), 5);
    not_refused("F with one corrector", Forcing::F, piso(1), 5);
    not_refused(
        "B with PIMPLE, two outer iterations",
        Forcing::B,
        PimpleSettings::pimple(NU, 2),
        1,
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// R0, L1, L1n, L1', L2, L3: the heavy records
// ---------------------------------------------------------------------------------------------------------------------------

/// The lag estimates of one instant: (with the reconstruction, n 1), (with, n 2), (without, n 1), each (rho, cell).
type Est = [(f64, usize); 3];

fn estimates(s: &mut PimpleSolver, dt: f64) -> Est {
    let mut one = |imm: bool, n: usize| {
        s.immersed_lag = imm;
        let w = s.lag_trial(dt, 40, n);
        s.immersed_lag = false;
        w.map_or((f64::NAN, usize::MAX), |w| (w.rho, w.cell))
    };
    [one(true, 1), one(true, 2), one(false, 1)]
}

struct Record {
    f: Forcing,
    stop: Option<usize>,
    reason: String,
    max_all: f64,
    growth: f64,
    at2: Est,
    at11: Option<Est>,
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

/// One run of the records with one corrector and the check off: the estimates at step 0 (estimate at step 2) and after step
/// 10 (estimate at step 11), at most 200 steps.
fn record(f: Forcing, dt: f64) -> Record {
    let (_, mut s) = r_solver(f, 1, false);
    let at2 = estimates(&mut s, dt);
    let (mut stop, mut reason, mut at11, mut hist) = (None, String::new(), None, Vec::new());
    for n in 1..=200 {
        s.step(dt);
        if let Some(d) = &s.diverged {
            stop = Some(n);
            reason = format!("{} {} {:.4e}", d.stage, d.quantity, d.value);
            break;
        }
        hist.push(max_abs_u(&s.u.values));
        if n == 10 {
            at11 = Some(estimates(&mut s, dt));
        }
    }
    let k = hist.len();
    let growth = if k >= 4 {
        (hist[k - 1] / hist[k - 4]).powf(1.0 / 3.0)
    } else {
        f64::NAN
    };
    Record {
        f,
        stop,
        reason,
        max_all: max_abs_u(&s.u.values),
        growth,
        at2,
        at11,
    }
}

fn surface_distance(x: Vec3) -> f64 {
    (vec3::norm(vec3::sub(x, CENTRE)) - 0.5 * DIAM).abs()
}

fn runs(r: &Record) -> bool {
    r.stop.is_none() && r.max_all <= 10.0
}

/// `a <= b`, false for a NaN.
fn le(a: f64, b: f64) -> bool {
    a <= b
}

/// `a > b`, false for a NaN.
fn gt(a: f64, b: f64) -> bool {
    a > b
}

fn rhos(es: &[Est], i: usize) -> Vec<f64> {
    es.iter().map(|e| e[i].0).collect()
}

/// ADR 0374 R0, L1, L1n (extended by amendment 1), L1' and L2: the five runs with one corrector and their lag estimates. Run
/// once; every check is collected and the test fails with the list of the missed ones.
#[test]
#[ignore = "heavy record (ADR 0374 R0, L1, L1n, L1', L2): run once in release"]
fn r0_l1_l2_single_corrector_record() {
    let all = [Forcing::F, Forcing::Sa, Forcing::Sb, Forcing::B1, Forcing::B];
    let mut out: Vec<Option<Record>> = (0..all.len()).map(|_| None).collect();
    std::thread::scope(|sc| {
        for (f, slot) in all.iter().zip(out.iter_mut()) {
            let f = *f;
            sc.spawn(move || *slot = Some(record(f, DT)));
        }
    });
    let recs: Vec<Record> = out.into_iter().map(|r| r.unwrap()).collect();
    let (geom, s0) = r_solver(Forcing::B, 1, false);
    let cc = geom.mesh.cell_centres();
    let ib = s0.immersed.as_ref().unwrap();
    let place = |c: usize| {
        if c < cc.len() {
            format!(
                "cell {c} at ({:.5}, {:.5}) solid {} surface distance {:.5}",
                cc[c][0],
                cc[c][1],
                ib.body_of_cell[c].is_some(),
                surface_distance(cc[c])
            )
        } else {
            "no estimate".into()
        }
    };
    for r in &recs {
        println!(
            "R0 {:?}: stop {:?} ({}), max |u| {:.4e}, growth over the last three steps {:.3}",
            r.f, r.stop, r.reason, r.max_all, r.growth
        );
        for (name, e) in [("step 2", Some(r.at2)), ("step 11", r.at11)] {
            match e {
                Some(e) => println!(
                    "L1 {:?} {name}: rho_1 with {:.5} ({}); rho_2 with {:.5} ({}); rho_1 without {:.5} ({}); q {:.5}",
                    r.f,
                    e[0].0,
                    place(e[0].1),
                    e[1].0,
                    place(e[1].1),
                    e[2].0,
                    place(e[2].1),
                    e[0].0 / e[2].0
                ),
                None => println!("L1 {:?} {name}: not reached", r.f),
            }
        }
    }
    let get = |f: Forcing| recs.iter().find(|r| r.f == f).unwrap();
    let both = |f: Forcing| -> Vec<Est> {
        let r = get(f);
        std::iter::once(r.at2).chain(r.at11).collect()
    };
    let mut missed = Vec::new();
    // R0
    for f in [Forcing::F, Forcing::Sa] {
        if !runs(get(f)) {
            missed.push(format!("R0: {f:?} does not run"));
        }
    }
    for (f, step) in [(Forcing::Sb, 126), (Forcing::B1, 46), (Forcing::B, 34)] {
        let r = get(f);
        if r.stop != Some(step) || !r.reason.starts_with("end of step courant") {
            missed.push(format!(
                "R0: {f:?} stop {:?} ({}) against step {step}",
                r.stop, r.reason
            ));
        }
    }
    // L1 (a)
    for f in [Forcing::Sb, Forcing::B1, Forcing::B] {
        let m = rhos(&both(f), 0).into_iter().fold(f64::NEG_INFINITY, f64::max);
        if !gt(m, 1.0) {
            missed.push(format!("L1 (a): {f:?} max rho_1 {m:.5}"));
        }
    }
    // L1 (b)
    for f in [Forcing::F, Forcing::Sa] {
        let es = both(f);
        if es.len() != 2 || !es.iter().all(|e| e[0].0 < 1.0) {
            missed.push(format!("L1 (b): {f:?} rho_1 {:?}", rhos(&es, 0)));
        }
    }
    // L1 (c)
    for f in all {
        let es = both(f);
        if es.len() != 2 || !es.iter().all(|e| e[1].0 < 1.0) {
            missed.push(format!("L1 (c): {f:?} rho_2 {:?}", rhos(&es, 1)));
        }
    }
    // L1n
    let es = both(Forcing::B);
    if es.len() != 2 || !es.iter().all(|e| e[2].0 < 1.0) {
        missed.push(format!("L1n: B rho_1 without {:?}", rhos(&es, 2)));
    }
    // L1' (amendment 1)
    let q = |e: &Est| e[0].0 / e[2].0;
    for f in [Forcing::Sb, Forcing::B1, Forcing::B] {
        let m = both(f).iter().map(q).fold(f64::NEG_INFINITY, f64::max);
        if !le(1.05, m) {
            missed.push(format!("L1' (a): {f:?} max q {m:.5}"));
        }
    }
    let es = both(Forcing::Sa);
    if es.len() != 2 || !es.iter().all(|e| q(e) <= 1.01) {
        missed.push(format!("L1' (b): Sa q {:?}", es.iter().map(q).collect::<Vec<_>>()));
    }
    let es = both(Forcing::F);
    if es.len() != 2 || !es.iter().all(|e| e[0].0.to_bits() == e[2].0.to_bits()) {
        missed.push(format!("L1' (c): F q {:?}", es.iter().map(q).collect::<Vec<_>>()));
    }
    // L2
    let c = get(Forcing::B).at2[0].1;
    println!("L2: B mode cell: {}", place(c));
    if c >= cc.len() || !le(surface_distance(cc[c]), 0.0075) {
        missed.push(format!("L2: the mode cell {c}"));
    } else {
        let faces: Vec<(f64, bool)> = ib
            .flux_faces
            .iter()
            .filter(|ff| ff.fluid == c || ff.solid == c)
            .map(|ff| (ff.weight, ff.image.is_some()))
            .collect();
        println!("L2: flux faces of that cell (weight, image): {faces:?}");
    }
    println!("missed: {missed:#?}");
    assert!(missed.is_empty(), "{missed:#?}");
}

/// ADR 0374 L3: the bounded form with one corrector at a step four times smaller runs 200 steps, its control runs, and the
/// lag estimate with the reconstruction at step 2 is below one.
#[test]
#[ignore = "heavy record (ADR 0374 L3): run once in release"]
fn l3_smaller_time_step_record() {
    let dt = 2.5e-4;
    let (mut b, mut f) = (None, None);
    std::thread::scope(|sc| {
        sc.spawn(|| b = Some(record(Forcing::B, dt)));
        sc.spawn(|| f = Some(record(Forcing::F, dt)));
    });
    let (b, f) = (b.unwrap(), f.unwrap());
    for r in [&b, &f] {
        println!(
            "L3 {:?} dt {dt:.1e}: stop {:?} ({}), max |u| {:.4e}; step 2: rho_1 with {:.5}, rho_2 with {:.5}, rho_1 without \
             {:.5}; step 11: {:?}",
            r.f, r.stop, r.reason, r.max_all, r.at2[0].0, r.at2[1].0, r.at2[2].0, r.at11
        );
    }
    let mut missed = Vec::new();
    if !runs(&b) {
        missed.push(format!("L3: B does not run (stop {:?} {})", b.stop, b.reason));
    }
    if !runs(&f) {
        missed.push("L3: the control F does not run".to_string());
    }
    if !gt(1.0, b.at2[0].0) {
        missed.push(format!("L3: B rho_1 {:.5}", b.at2[0].0));
    }
    assert!(missed.is_empty(), "{missed:#?}");
}
