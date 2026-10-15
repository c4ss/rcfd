//! F-042: corrected turbulent backward-facing-step set-up "BFS-C", grid study, sensitivities (ADR 0015).
//!
//! The old test `step_kepsilon.rs` stays unchanged as a record. BFS-C differs from it by the TMR Reynolds number
//! (36000), the inlet plane at x/H = -4 and the measured inlet profile of Driver and Seegmiller at x/H = -4
//! (u, k from the measured stresses; epsilon from a mixing length), see `tests/common/step.rs` and ADR 0015.
//!
//! Registered checks (ADR 0015, limits written before the first run):
//! * A1: x_r/h in [6.16, 6.36] (window 6.26 +- 0.10, unchanged) on the medium mesh for Standard, RNG, Realizable.
//! * A2: grid-converged iff |x_r(fine) - x_r(medium)| <= 0.10 and all runs converge (reported).
//! * A3: sensitivities, reported only.
//! * A4: component_diagonal = corrected is a bitwise no-op on this mesh (coarse mesh, 200 iterations).
//!
//! Heavy: run in release with `--ignored --nocapture`.

mod common;
use common::step::*;
use rcfd_fv::convection::ConvScheme;
use rcfd_turb::KEpsilonVariant;

const VARIANTS: [(&str, KEpsilonVariant); 3] = [
    ("Standard", KEpsilonVariant::Standard),
    ("RNG", KEpsilonVariant::Rng),
    ("Realizable", KEpsilonVariant::Realizable),
];

fn cfg(v: KEpsilonVariant, n: [usize; 4]) -> StepCfg {
    let mut c = StepCfg::base(Model::Ke(v), Inlet::Measured, n);
    c.nit = env("NIT", 6000);
    c.verbose = env("VERBOSE", 0) == 1;
    c
}

fn report(tag: &str, o: &StepOut) {
    println!(
        "{tag}: cells {} iterations {} converged {:?} du {:.2e} dnut {:.2e} x_r/h {:?}",
        o.n_cells, o.iterations, o.converged_at, o.du, o.dnut, o.xr
    );
}

fn window(name: &str, v: KEpsilonVariant) {
    let o = run_step(&cfg(v, MESH_MEDIUM));
    report(&format!("BFS-C {name} medium"), &o);
    assert!(o.converged_at.is_some(), "{name}: not converged");
    let xr = o.xr.expect("no reattachment point found");
    assert!(
        (xr - XR_REF).abs() <= XR_TOL,
        "{name}: x_r/h = {xr:.3} outside {XR_REF} +- {XR_TOL}"
    );
}

#[test]
#[ignore = "heavy: BFS-C medium mesh, release"]
fn step_c_reattachment_standard_known_miss() {
    window("Standard", KEpsilonVariant::Standard);
}

#[test]
#[ignore = "heavy: BFS-C medium mesh, release"]
fn step_c_reattachment_rng() {
    window("RNG", KEpsilonVariant::Rng);
}

#[test]
#[ignore = "heavy: BFS-C medium mesh, release"]
fn step_c_reattachment_realizable_known_miss() {
    window("Realizable", KEpsilonVariant::Realizable);
}

/// A2: three meshes, BFS-C and (for the record) the old set-up.
#[test]
#[ignore = "heavy: nine runs, release"]
fn step_c_grid_study() {
    let which = env::<String>("VARIANT", "all".into());
    for (name, v) in VARIANTS {
        if which != "all" && which != name {
            continue;
        }
        let mut xr = Vec::new();
        for (label, n) in [("coarse", MESH_COARSE), ("medium", MESH_MEDIUM), ("fine", MESH_FINE)] {
            let o = run_step(&cfg(v, n));
            report(&format!("BFS-C {name} {label}"), &o);
            assert!(o.converged_at.is_some(), "{name} {label}: not converged");
            xr.push(o.xr.expect("no reattachment"));
        }
        let conv = (xr[2] - xr[1]).abs() <= 0.10;
        println!(
            "GRID {name}: coarse {:.3} medium {:.3} fine {:.3}; |fine - medium| = {:.3} -> grid-converged (<= 0.10): {conv}",
            xr[0],
            xr[1],
            xr[2],
            (xr[2] - xr[1]).abs()
        );
    }
}

/// A3: single-factor sensitivities on the medium mesh (reported only).
#[test]
#[ignore = "heavy: sensitivity runs, release"]
fn step_c_sensitivities() {
    for (name, v) in VARIANTS {
        // reference runs
        let old = StepCfg::base(Model::Ke(v), Inlet::Old, MESH_MEDIUM);
        let r_old = run_step(&old);
        report(&format!("{name} OLD set-up"), &r_old);
        let r_c = run_step(&cfg(v, MESH_MEDIUM));
        report(&format!("{name} BFS-C"), &r_c);
        // (a) old set-up, second-order k/epsilon convection
        let mut a = old.clone();
        a.turb_conv = ConvScheme::LinearUpwind;
        let ra = run_step(&a);
        report(&format!("{name} (a) OLD + linearUpwind k/eps"), &ra);
        // (b) BFS-C with the old free-stream k
        let mut b = cfg(v, MESH_MEDIUM);
        b.k_free = Some(1.5e-4);
        let rb = run_step(&b);
        report(&format!("{name} (b) BFS-C + free-stream k 1.5e-4"), &rb);
        // (c) BFS-C with Re_h 37500
        let mut c = cfg(v, MESH_MEDIUM);
        c.re_h = 37500.0;
        let rc = run_step(&c);
        report(&format!("{name} (c) BFS-C + Re_h 37500"), &rc);
        // (d) old set-up with the inlet plane at x/H = -4
        let mut d = old.clone();
        d.l_in = 4.0;
        let rd = run_step(&d);
        report(&format!("{name} (d) OLD + l_in 4"), &rd);
        // (e) BFS-C with second-order k/epsilon convection
        let mut e = cfg(v, MESH_MEDIUM);
        e.turb_conv = ConvScheme::LinearUpwind;
        let re = run_step(&e);
        report(&format!("{name} (e) BFS-C + linearUpwind k/eps"), &re);
        let base_old = r_old.xr.unwrap_or(f64::NAN);
        let base_c = r_c.xr.unwrap_or(f64::NAN);
        println!(
            "SENS {name}: OLD {base_old:.3}; (a) {:+.3}; (d) {:+.3}; BFS-C {base_c:.3}; (b) {:+.3}; (c) {:+.3}; (e) {:+.3}",
            ra.xr.unwrap_or(f64::NAN) - base_old,
            rd.xr.unwrap_or(f64::NAN) - base_old,
            rb.xr.unwrap_or(f64::NAN) - base_c,
            rc.xr.unwrap_or(f64::NAN) - base_c,
            re.xr.unwrap_or(f64::NAN) - base_c
        );
    }
}

/// A4: the component-consistent diagonal is a bitwise no-op on the step mesh (no symmetry plane).
#[test]
#[ignore = "heavy-ish: two 200-iteration runs, release"]
fn step_component_diagonal_is_noop() {
    let mut a = cfg(KEpsilonVariant::Standard, MESH_COARSE);
    a.nit = 200;
    let mut b = a.clone();
    b.comp_diag = true;
    let (ra, rb) = (run_step(&a), run_step(&b));
    let max_diff =
        ra.u.iter()
            .zip(&rb.u)
            .map(|(p, q)| (0..3).map(|k| (p[k] - q[k]).abs()).fold(0.0, f64::max))
            .fold(0.0, f64::max);
    println!("component_diagonal corrected vs averaged: max |du| = {max_diff:e}");
    assert_eq!(max_diff, 0.0);
}
