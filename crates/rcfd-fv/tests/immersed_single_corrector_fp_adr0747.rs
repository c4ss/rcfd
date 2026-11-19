//! F-471 (ADR 0747): the fingerprints of ADR 0374 N0 (F-363) restated under the corrected order of the coarsest dense LU.
//!
//! ADR 0743 made the corrected order of the coarsest dense LU solve the default of the scalar AMG. The fingerprint FP4 of ADR
//! 0374 N0 (`immersed_single_corrector_f363::n0_fingerprints_of_the_defaults`, now an ignored record) moved with it, from
//! `0x279c4c4ed4cf89be` to `0xda894fae1d52a3ad`; the maintainer approved that move on 2026-11-19. This file restates N0
//! under the default (F1) and under the old order, asked for with `LinearOptions::amg_coarse_lu_interleaved` (F2), names
//! the one pressure solve whose coarsest factorisation interchanges rows at a step `k >= 1` (F3), and shows that the two
//! orders give the same bits up to that solve and different bits after it (F4, F4a). The helpers are copies of those of
//! `immersed_single_corrector_f363.rs` (that file is not edited beyond the attribute that retires N0), without the two
//! forcings N0 does not use.

use rcfd_core::vec3::Vec3;
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::diffusion::uniform_gamma;
use rcfd_fv::immersed::{ImmersedBody, ImmersedForcing, ImmersedSettings, SolidShape};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::pressure::pressure_system_src;
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_linalg::{Amg, AmgSettings};
use rcfd_mesh::gen::{axis_coords, box_2d_coords, expansion_for_end_cell, Grading};
use std::sync::Arc;

// ---------------------------------------------------------------------------------------------------------------------------
// The forcings of ADR 0374 that N0 uses (the table "The runs")
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Forcing {
    /// `build`, `theta_min = 0.01`.
    F,
    /// `build_second_order`, `theta_min = 0.5` (weights at most 1).
    Sa,
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
// The two orders of the coarsest dense LU solve (ADR 0743 D2) and the runs of N0
// ---------------------------------------------------------------------------------------------------------------------------

/// The registered fingerprints of ADR 0374 N0 (measured on the tree 18c92d08, before ADR 0743).
const FP1: u64 = 0xdd07702a17b9a36f;
const FP2: u64 = 0xe4d227de316e6de4;
const FP3_F: (u64, usize) = (0x3ff4a94abc88ca99, 11855);
const FP3_B: (u64, usize) = (0x3fb06aeaae8972f1, 11855);
/// FP4 as registered by ADR 0374 N0: the result of the interleaved order (the default before ADR 0743).
const FP4_OLD_ORDER: u64 = 0x279c4c4ed4cf89be;
/// FP4 under the default of ADR 0743 (ADR 0743 addendum 1; the move approved by the maintainer on 2026-11-19).
const FP4_DEFAULT: u64 = 0xda894fae1d52a3ad;

/// The solver of R; with `old` its pressure AMG uses the interleaved order of before ADR 0743.
fn r_solver_order(f: Forcing, n_correctors: usize, old: bool) -> (Arc<FvGeometry>, PimpleSolver) {
    let (geom, mut s) = r_solver(f, n_correctors, true);
    s.settings.linear.amg_coarse_lu_interleaved = old;
    (geom, s)
}

/// 20 steps of R with the forcing `f`, `n` correctors and `flux_weight_check` on; the hash of `u` then `p`.
fn fp_run(f: Forcing, n: usize, old: bool) -> u64 {
    let (_, mut s) = r_solver_order(f, n, old);
    for k in 1..=20 {
        s.step(DT);
        assert!(
            s.diverged.is_none(),
            "N0: {f:?} with {n} correctors (old order {old}) stopped at step {k}: {:?}",
            s.diverged
        );
    }
    state_hash(&s.u.values, &s.p.values)
}

/// `lag_trial(1e-3, 20, n)` of R with the forcing `f` at step 0: the bits of `rho` and the cell.
fn fp_lag(f: Forcing, n: usize, old: bool) -> (u64, usize) {
    let (_, s) = r_solver_order(f, n, old);
    let w = s.lag_trial(DT, 20, n).expect("N0: no lag estimate");
    println!(
        "N0: lag_trial {f:?} n {n} (old order {old}): rho {:.6} cell {}",
        w.rho, w.cell
    );
    (w.rho.to_bits(), w.cell)
}

/// The five values of ADR 0374 N0: FP1, FP2, FP3 (F), FP3 (B), FP4.
type Fingerprints = (u64, u64, (u64, usize), (u64, usize), u64);

/// The body of ADR 0374 N0 under one order of the coarsest solve.
fn n0(old: bool) -> Fingerprints {
    let fp1 = fp_run(Forcing::F, 1, old);
    let fp2 = fp_run(Forcing::B, 2, old);
    let fp3a = fp_lag(Forcing::F, 1, old);
    let fp3b = fp_lag(Forcing::B, 2, old);
    let fp4 = fp_run(Forcing::Sa, 1, old);
    println!(
        "N0 (old order {old}): FP1 {fp1:016x} FP2 {fp2:016x} FP3 {:016x}/{} {:016x}/{} FP4 {fp4:016x}",
        fp3a.0, fp3a.1, fp3b.0, fp3b.1
    );
    (fp1, fp2, fp3a, fp3b, fp4)
}

/// ADR 0747 F1: ADR 0374 N0 restated under the default: FP1 to FP3 as registered, FP4 the value of the corrected order.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f1_fingerprints_of_the_defaults_restated() {
    let (fp1, fp2, fp3a, fp3b, fp4) = n0(false);
    assert_eq!(fp1, FP1, "FP1");
    assert_eq!(fp2, FP2, "FP2");
    assert_eq!(fp3a, FP3_F, "FP3 F");
    assert_eq!(fp3b, FP3_B, "FP3 B");
    assert_eq!(fp4, FP4_DEFAULT, "FP4");
}

/// ADR 0747 F2: under the old order every value of ADR 0374 N0 is the registered one, FP4 included.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f2_old_order_gives_the_registered_fingerprints() {
    let (fp1, fp2, fp3a, fp3b, fp4) = n0(true);
    assert_eq!(fp1, FP1, "FP1 under the old order");
    assert_eq!(fp2, FP2, "FP2 under the old order");
    assert_eq!(fp3a, FP3_F, "FP3 F under the old order");
    assert_eq!(fp3b, FP3_B, "FP3 B under the old order");
    assert_eq!(fp4, FP4_OLD_ORDER, "FP4 under the old order");
}

/// One captured pressure system whose coarsest factorisation interchanges at a step `k >= 1`: (step, corrector, coarsest
/// size, the interchange steps `>= 1`).
type Interchange = (usize, usize, usize, Vec<usize>);

/// 20 steps of R under the default with every pressure system captured: the number of systems, those with an interchange
/// at a step `k >= 1`, and the hash of `u` then `p`.
fn interchanges_of_run(f: Forcing, n: usize) -> (usize, Vec<Interchange>, u64) {
    let (geom, mut s) = r_solver_order(f, n, false);
    s.pressure_capture = Some(Vec::new());
    for k in 1..=20 {
        s.step(DT);
        assert!(s.diverged.is_none(), "F3: {f:?} stopped at step {k}: {:?}", s.diverged);
    }
    let caps = s.pressure_capture.take().unwrap();
    let mut found = Vec::new();
    for c in &caps {
        let m = pressure_system_src(&geom, &c.gamma, &c.bc, &c.phi, &c.controls, c.src.as_deref())
            .expect("F3: the plain pressure path");
        let amg = Amg::new(&m.a.to_csr(), AmgSettings::default());
        let all = amg.coarse_interchanges();
        let ks: Vec<usize> = all.iter().copied().filter(|&k| k >= 1).collect();
        let coarsest = *amg.level_sizes().last().unwrap();
        println!(
            "F3 {f:?} step {} corrector {}: levels {:?}, coarsest interchanges at {all:?}",
            c.step,
            c.corrector,
            amg.level_sizes()
        );
        if !ks.is_empty() {
            found.push((c.step, c.corrector, coarsest, ks));
        }
    }
    (caps.len(), found, state_hash(&s.u.values, &s.p.values))
}

/// ADR 0747 F3: of the pressure systems of the three runs of N0 exactly one has a coarsest factorisation that interchanges
/// rows at a step `k >= 1`: step 3 of the `Sa` run (coarsest 90 unknowns, step 35). The F and B runs have none.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f3_one_factorisation_takes_the_other_interchange() {
    let (n_f, found_f, _) = interchanges_of_run(Forcing::F, 1);
    let (n_b, found_b, _) = interchanges_of_run(Forcing::B, 2);
    let (n_sa, found_sa, hash_sa) = interchanges_of_run(Forcing::Sa, 1);
    println!("F3: F {n_f} systems {found_f:?}; B {n_b} systems {found_b:?}; Sa {n_sa} systems {found_sa:?}");
    assert_eq!(n_f, 20, "F3: systems of F");
    assert!(found_f.is_empty(), "F3: F {found_f:?}");
    assert_eq!(n_b, 40, "F3: systems of B");
    assert!(found_b.is_empty(), "F3: B {found_b:?}");
    assert_eq!(n_sa, 20, "F3: systems of Sa");
    assert_eq!(found_sa, vec![(3, 0, 90, vec![35])], "F3: Sa");
    assert_eq!(hash_sa, FP4_DEFAULT, "F3: the captured Sa run is the run of FP4");
}

/// The number of values of `u` and `p` whose bits differ between two solvers.
fn state_diff(a: &PimpleSolver, b: &PimpleSolver) -> usize {
    let du =
        a.u.values
            .iter()
            .zip(&b.u.values)
            .flat_map(|(x, y)| x.iter().zip(y.iter()))
            .filter(|(x, y)| x.to_bits() != y.to_bits())
            .count();
    let dp =
        a.p.values
            .iter()
            .zip(&b.p.values)
            .filter(|(x, y)| x.to_bits() != y.to_bits())
            .count();
    du + dp
}

/// ADR 0747 F4 and F4a: the old order and the default give the same bits of the `Sa` run after steps 1 and 2 (before the
/// solve of F3) and different bits after step 3 and after step 20.
#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f4_the_two_orders_part_at_step_3() {
    let (_, mut new) = r_solver_order(Forcing::Sa, 1, false);
    let (_, mut old) = r_solver_order(Forcing::Sa, 1, true);
    let mut diffs = Vec::new();
    for k in 1..=20 {
        new.step(DT);
        old.step(DT);
        assert!(new.diverged.is_none() && old.diverged.is_none(), "F4: stop at step {k}");
        let d = state_diff(&new, &old);
        println!("F4 step {k}: {d} values differ");
        diffs.push(d);
    }
    assert_eq!(diffs[0], 0, "F4: step 1");
    assert_eq!(diffs[1], 0, "F4: step 2");
    assert!(diffs[2] >= 1, "F4a: step 3: the two orders do not differ");
    assert!(diffs[19] >= 1, "F4a: step 20: the two orders do not differ");
}
