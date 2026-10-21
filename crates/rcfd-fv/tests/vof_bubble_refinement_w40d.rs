//! ADR 0100 part B: refinement check of the Hysing et al. 2009 case-1 rising bubble (set-up of `vof_bubble.rs`, free-slip sides)
//! on three grids. Reference values read in Hysing et al. 2009 Table V (ADR 0096): V_c,max 0.2417 at t 0.9213, y_c(3) 1.0813.

use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const YC_REF: f64 = 1.0813;
const V_REF: f64 = 0.2417;
const T_REF: f64 = 0.9213;

/// The `vof_bubble.rs` set-up on `n` x `2n` cells.
fn bubble(n: usize, full: bool, cd: bool) -> VofSolver {
    let mesh = Arc::new(box_2d(n, 2 * n, 1.0, 2.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    for p in ["xmin", "xmax"] {
        u.set_bc(&geom, p, Box::new(Symmetry)).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 1.0);
    alpha.values = indicator_fraction(&geom, 10, |x| (x[0] - 0.5).hypot(x[1] - 0.5) > 0.25);
    let mut s = VofSettings::new([1000.0, 100.0], [10.0, 1.0]);
    s.gravity = [0.0, -0.98, 0.0];
    s.sigma = 24.5;
    s.n_correctors = 3;
    s.full_viscous_stress = full; // ADR 0103 (b1f only)
    s.component_diagonal = cd; // ADR 0103 (b1c only)
    s.amg_pressure = true; // CG would take about 2.1 h on 160 x 320 (ADR 0100 smoke); the tree uses AMG above 2000 cells
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

/// Runs `n` x `2n` to `t_end` with `dt = 0.1 / n` (2.5e-3 on 40 x 80, as `vof_bubble.rs`):
/// (y_c(t_end), V_c,max, time of V_c,max, phase-volume drift, bound violation).
fn run(n: usize, t_end: f64, full: bool, cd: bool) -> (f64, f64, f64, f64, f64) {
    let mut s = bubble(n, full, cd);
    let dt = 0.1 / n as f64;
    assert!(dt <= s.capillary_dt(), "capillary limit {}", s.capillary_dt());
    let vol = s.geom.mesh.cell_volumes().to_vec();
    let cc = s.geom.mesh.cell_centres().to_vec();
    let v0 = s.phase_volume();
    let (mut vmax, mut tmax, mut dv, mut wb) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let t0 = std::time::Instant::now();
    while s.time < t_end - 1e-9 {
        let i = s.step(dt).unwrap();
        let a = &s.transport.alpha.values;
        let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
        let vc: f64 = (0..a.len())
            .map(|c| (1.0 - a[c]) * vol[c] * s.u.values[c][1])
            .sum::<f64>()
            / m;
        if vc > vmax {
            vmax = vc;
            tmax = s.time;
        }
        dv = dv.max((s.phase_volume() - v0).abs() / v0);
        wb = wb.max((-i.alpha_min).max(i.alpha_max - 1.0));
    }
    let a = &s.transport.alpha.values;
    let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
    let yc: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c] * cc[c][1]).sum::<f64>() / m;
    let umax = s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max);
    let finite = s.u.values.iter().all(|v| v.iter().all(|x| x.is_finite())) && a.iter().all(|x| x.is_finite());
    println!(
        "B {n} x {}, dt/dt_cap {:.3}: y_c({t_end}) = {yc:.4} (err {:.4}), V_c,max = {vmax:.4} at t = {tmax:.4} (err {:.4}, time err {:.4}), dV {dv:.2e}, bound {wb:.2e}, max|U| {umax:.4}, finite {finite}, {:.1} s",
        2 * n,
        0.1 / n as f64 / s.capillary_dt(),
        (yc - YC_REF).abs(),
        (vmax - V_REF).abs(),
        (tmax - T_REF).abs(),
        t0.elapsed().as_secs_f64()
    );
    (yc, vmax, tmax, dv, wb)
}

/// Rule (d) smoke: 160 x 320 to t = 0.4 (finiteness, bounds, max|U|; no reference comparison).
#[test]
#[ignore = "smoke of ADR 0100 part B"]
fn b0_smoke() {
    run(160, 0.4, false, false);
}

/// B1-B4 of ADR 0100: errors to the Hysing values on 40 x 80, 80 x 160, 160 x 320; all grids are printed before any assert.
#[test]
#[ignore = "KNOWN MISS (ADR 0100 addendum): errors to Hysing grow under refinement (y_c 0.053, 0.064, 0.069); heavy, about 30 min"]
fn b1_hysing_bubble_error_decreases_under_refinement_known_miss() {
    b1(false, false);
}

/// ADR 0103: B1-B4 of ADR 0100 (limits unchanged) with the transposed viscous stress on (`full_viscous_stress`). Not run on
/// 160 x 320: its 40 and 80 points are the ADR 0103 D2 runs, whose errors already grow (y_c 0.0594 -> 0.0668, V_c,max 0.0306 ->
/// 0.0322), so B1 fails (ADR 0103 amendment A).
#[test]
#[ignore = "KNOWN MISS (ADR 0103 amendment A): B1 fails on the 40 and 80 points (D2); heavy, about 30 min"]
fn b1f_hysing_bubble_error_decreases_under_refinement_full_viscous_stress_known_miss() {
    b1(true, false);
}

/// ADR 0103 amendment C: B1-B4 of ADR 0100 (limits unchanged) with the component-diagonal correction (`component_diagonal`). Not
/// run on 160 x 320: its 40 and 80 points are the ADR 0103 C2 runs, whose errors still grow (y_c 0.0046 -> 0.0101, V_c,max 0.0043
/// -> 0.0052), so B1 fails.
#[test]
#[ignore = "KNOWN MISS (ADR 0103 amendment C): B1 fails on the 40 and 80 points (C2); heavy, about 30 min"]
fn b1c_hysing_bubble_error_decreases_under_refinement_component_diagonal_known_miss() {
    b1(false, true);
}

fn b1(full: bool, cd: bool) {
    let r: Vec<_> = [40, 80, 160].into_iter().map(|n| run(n, 3.0, full, cd)).collect();
    let ey: Vec<f64> = r.iter().map(|x| (x.0 - YC_REF).abs()).collect();
    let ev: Vec<f64> = r.iter().map(|x| (x.1 - V_REF).abs()).collect();
    let et: Vec<f64> = r.iter().map(|x| (x.2 - T_REF).abs()).collect();
    println!("B errors (40, 80, 160): y_c {ey:?}, V_c,max {ev:?}, peak time {et:?}");
    assert!(ey[1] < ey[0] && ey[2] < ey[1], "B1 y_c(3) not monotone: {ey:?}");
    assert!(ev[1] < ev[0] && ev[2] < ev[1], "B1 V_c,max not monotone: {ev:?}");
    assert!(ey[2] <= 0.5 * ey[0], "B2 y_c(3): {ey:?}");
    assert!(ev[2] <= 0.5 * ev[0], "B2 V_c,max: {ev:?}");
    assert!(et[2] < et[0], "B3 peak time: {et:?}");
    for (n, x) in [40, 80, 160].iter().zip(&r) {
        assert!(
            x.3 <= 1e-12 && x.4 <= 1e-9,
            "B4 {n}: drift {:.2e}, bound {:.2e}",
            x.3,
            x.4
        );
    }
}
