//! F-043 (ADR 0015, check B5): k-omega SST and BSL on the corrected backward-facing step BFS-C with wall-resolved
//! meshes (first-cell y+ about 2, 1, 0.5 on the floor downstream of the step).
//!
//! Registered limits (ADR 0015, written before the first run):
//! * each run converges (max update < 1e-7, nu_t change < 1e-6 within 8000 iterations);
//! * x_r/h of the y+ ~ 1 mesh lies in [6.16, 6.36];
//! * spread (max - min) of x_r/h over the three meshes <= 0.15.
//!
//! The first-cell height is chosen for u_tau = 0.04 (about the reattachment value of these runs); the realised
//! y+ range on the floor is printed. Mesh: 24 + 160 cells in x, 60 (inlet channel, layers on both walls) and 40
//! (step segment, layers on the floor) in y, 20 layers of growth 1.25.
//!
//! Heavy: release, `--ignored --nocapture`.

mod common;
use common::step::*;
use rcfd_turb::sst::SstVariant;

const MESH_WR: [usize; 4] = [24, 160, 60, 40];
const NU: f64 = 1.0 / 36000.0;

fn run(variant: SstVariant, y_plus: f64, b: bool) -> StepOut {
    let mut c = StepCfg::base(Model::Sst, Inlet::Measured, MESH_WR);
    c.sst_variant = variant;
    if b {
        // B5b (ADR 0015, addendum A): centre-based y+ at the floor maximum (u_tau 0.03), chosen alpha_u
        c.wall_first = Some(2.0 * y_plus * NU / 0.03);
        c.alpha_u = env("AU", 0.8);
    } else {
        c.wall_first = Some(y_plus * NU / 0.04);
    }
    c.nit = env("NIT", 8000);
    c.verbose = env("VERBOSE", 0) == 1;
    run_step(&c)
}

fn study(variant: SstVariant, b: bool) {
    let mut xrs = Vec::new();
    let only = env::<f64>("YP", 0.0);
    for yp in [2.0, 1.0, 0.5] {
        if only > 0.0 && (only - yp).abs() > 1e-9 {
            continue;
        }
        let o = run(variant, yp, b);
        println!(
            "B5{} {variant:?} target y+ {yp}: cells {} iterations {} converged {:?} du {:.2e} dnut {:.2e} x_r/h {:?} floor y+ [{:.2}, {:.2}]",
            if b { "b" } else { "" },
            o.n_cells, o.iterations, o.converged_at, o.du, o.dnut, o.xr, o.y_plus_floor.0, o.y_plus_floor.1
        );
        assert!(o.converged_at.is_some(), "{variant:?} y+ {yp}: not converged");
        xrs.push(o.xr.expect("no reattachment"));
    }
    let (mx, mn) = (
        xrs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        xrs.iter().cloned().fold(f64::INFINITY, f64::min),
    );
    println!(
        "B5 {variant:?}: x_r/h = {:.3} / {:.3} / {:.3} (y+ 2 / 1 / 0.5), spread {:.3} (limit 0.15), window [6.16, 6.36]",
        xrs[0],
        xrs[1],
        xrs[2],
        mx - mn
    );
    assert!(
        (xrs[1] - XR_REF).abs() <= XR_TOL,
        "{variant:?} y+ 1: x_r/h = {:.3} outside {XR_REF} +- {XR_TOL}",
        xrs[1]
    );
    assert!(mx - mn <= 0.15, "{variant:?}: spread {:.3} > 0.15", mx - mn);
}

#[test]
#[ignore = "heavy: three wall-resolved BFS-C runs, release"]
fn step_c_sst_wall_resolved_known_miss() {
    study(SstVariant::Sst, false);
}

#[test]
#[ignore = "heavy: three wall-resolved BFS-C runs, release"]
fn step_c_sst_wall_resolved_b_known_miss() {
    study(SstVariant::Sst, true);
}

#[test]
#[ignore = "heavy: three wall-resolved BFS-C runs, release"]
fn step_c_bsl_wall_resolved_known_miss() {
    study(SstVariant::Bsl, false);
}

#[test]
#[ignore = "heavy: three wall-resolved BFS-C runs, release"]
fn step_c_bsl_wall_resolved_b_known_miss() {
    study(SstVariant::Bsl, true);
}
