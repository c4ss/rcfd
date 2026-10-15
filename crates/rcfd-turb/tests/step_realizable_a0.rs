//! F-042: effect of the Realizable constant A0 (4.04 default, 4.0 original report) on the BFS-C reattachment (ADR 0017).
//!
//! Registered before any run (ADR 0017):
//! * P1: A0 = 4.0, medium mesh, converged, x_r/h in [6.16, 6.36] (unchanged window).
//! * P2: A0 = 4.04 set explicitly is bit-equal to the default run, which equals 6.413 (ADR 0015) within 5e-4.
//! * P3: A0 = 4.0 grid check, |x_r(fine) - x_r(medium)| <= 0.10 and all converged.
//! * P4: delta = x_r(4.0) - x_r(4.04) on the medium mesh, hypothesis |delta| < 0.053 (reported).
//!
//! Heavy: run in release with `--ignored --nocapture`.

mod common;
use common::step::*;
use rcfd_turb::{KEpsilonVariant, REALIZABLE_A0_DEFAULT, REALIZABLE_A0_ORIGINAL};

fn cfg(a0: Option<f64>, n: [usize; 4]) -> StepCfg {
    let mut c = StepCfg::base(Model::Ke(KEpsilonVariant::Realizable), Inlet::Measured, n);
    c.nit = env("NIT", 6000);
    c.verbose = env("VERBOSE", 0) == 1;
    c.a0 = a0;
    c
}

fn report(tag: &str, o: &StepOut) {
    println!(
        "{tag}: cells {} iterations {} converged {:?} du {:.2e} dnut {:.2e} x_r/h {:?}",
        o.n_cells, o.iterations, o.converged_at, o.du, o.dnut, o.xr
    );
}

#[test]
fn a0_constants_are_the_documented_values() {
    assert_eq!(REALIZABLE_A0_DEFAULT, 4.04);
    assert_eq!(REALIZABLE_A0_ORIGINAL, 4.0);
}

/// P2 and P4 (reference run and effect size on the medium mesh).
#[test]
#[ignore = "heavy: three BFS-C medium runs, release"]
fn a0_default_reproduces_adr0015_and_effect_size() {
    let d = run_step(&cfg(None, MESH_MEDIUM));
    report("A0 default", &d);
    let e = run_step(&cfg(Some(REALIZABLE_A0_DEFAULT), MESH_MEDIUM));
    report("A0 explicit 4.04", &e);
    let o = run_step(&cfg(Some(REALIZABLE_A0_ORIGINAL), MESH_MEDIUM));
    report("A0 4.0", &o);
    let (xd, xe, xo) = (d.xr.unwrap(), e.xr.unwrap(), o.xr.unwrap());
    println!("P4: x_r(4.0) - x_r(4.04) = {:+.4}", xo - xd);
    assert_eq!(xd, xe, "explicit 4.04 differs from the default");
    assert!((xd - 6.413).abs() <= 5e-4, "default {xd} does not reproduce 6.413");
}

/// P1: the question.
#[test]
#[ignore = "heavy: BFS-C medium mesh, release"]
fn step_c_realizable_a0_4_window() {
    let o = run_step(&cfg(Some(REALIZABLE_A0_ORIGINAL), MESH_MEDIUM));
    report("BFS-C Realizable A0=4.0 medium", &o);
    assert!(o.converged_at.is_some(), "not converged");
    let xr = o.xr.expect("no reattachment point found");
    assert!(
        (xr - XR_REF).abs() <= XR_TOL,
        "x_r/h = {xr:.3} outside {XR_REF} +- {XR_TOL}"
    );
}

/// P3: grid check with A0 = 4.0.
#[test]
#[ignore = "heavy: three runs, release"]
fn step_c_realizable_a0_4_grid_study_known_miss() {
    let mut xr = Vec::new();
    for (label, n) in [("coarse", MESH_COARSE), ("medium", MESH_MEDIUM), ("fine", MESH_FINE)] {
        let o = run_step(&cfg(Some(REALIZABLE_A0_ORIGINAL), n));
        report(&format!("BFS-C Realizable A0=4.0 {label}"), &o);
        assert!(o.converged_at.is_some(), "{label}: not converged");
        xr.push(o.xr.expect("no reattachment"));
    }
    let diff = (xr[2] - xr[1]).abs();
    println!(
        "GRID A0=4.0: coarse {:.3} medium {:.3} fine {:.3}; |fine - medium| = {diff:.3} -> grid-converged (<= 0.10): {}",
        xr[0],
        xr[1],
        xr[2],
        diff <= 0.10
    );
    assert!(diff <= 0.10, "not grid-converged");
}
