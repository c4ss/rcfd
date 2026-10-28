//! ADR 0100 part B: refinement check of the Hysing et al. 2009 case-1 rising bubble (set-up of `vof_bubble.rs`, free-slip sides)
//! on three grids. Reference values read in Hysing et al. 2009 Table V (ADR 0096): V_c,max 0.2417 at t 0.9213, y_c(3) 1.0813.

use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const YC_REF: f64 = 1.0813;
const V_REF: f64 = 0.2417;
const T_REF: f64 = 0.9213;

/// Area of the disc of radius `r` centred at the origin intersected with `[x0, x1] x [y0, y1]` (exact up to round-off). Copy of
/// `disc_rect` in `vof_height_function_w42b.rs` (verified there by ADR 0106 H1).
fn disc_rect(x0: f64, x1: f64, y0: f64, y1: f64, r: f64) -> f64 {
    let f = |x: f64| {
        let x = x.clamp(-r, r);
        0.5 * (x * (r * r - x * x).max(0.0).sqrt() + r * r * (x / r).asin())
    };
    let (a, b) = (x0.max(-r), x1.min(r));
    if a >= b || y0 >= y1 {
        return 0.0;
    }
    let mut pts = vec![a, b];
    for y in [y0, y1] {
        if y.abs() < r {
            let xb = (r * r - y * y).sqrt();
            pts.extend([-xb, xb].into_iter().filter(|&x| x > a && x < b));
        }
    }
    pts.sort_by(f64::total_cmp);
    let mut area = 0.0;
    for w in pts.windows(2) {
        let (p, q) = (w[0], w[1]);
        let m = 0.5 * (p + q);
        let sm = (r * r - m * m).max(0.0).sqrt();
        if y1.min(sm) <= y0.max(-sm) {
            continue;
        }
        let up = if y1 < sm { y1 * (q - p) } else { f(q) - f(p) };
        let lo = if y0 > -sm { y0 * (q - p) } else { -(f(q) - f(p)) };
        area += up - lo;
    }
    area
}

/// ADR 0108: exact initial fractions of the bubble (R = 0.25 at (0.5, 0.5), alpha = 1 outside) on the `n` x `2n` grid (h = 1/n);
/// asserts the bubble area pi R^2 to relative 1e-10.
fn exact_fraction(geom: &FvGeometry, n: usize) -> Vec<f64> {
    let h = 1.0 / n as f64;
    let inside: Vec<f64> = geom
        .mesh
        .cell_centres()
        .iter()
        .map(|p| {
            let (x, y) = (p[0] - 0.5, p[1] - 0.5);
            disc_rect(x - 0.5 * h, x + 0.5 * h, y - 0.5 * h, y + 0.5 * h, 0.25)
        })
        .collect();
    let exact = std::f64::consts::PI * 0.0625;
    let total: f64 = inside.iter().sum();
    assert!((total / exact - 1.0).abs() <= 1e-10, "bubble area {total} vs {exact}");
    inside.iter().map(|v| (1.0 - v / (h * h)).clamp(0.0, 1.0)).collect()
}

/// The `vof_bubble.rs` set-up on `n` x `2n` cells with the three option flags assigned.
fn bubble(n: usize, full: bool, cd: bool, hf: bool, exact: bool) -> VofSolver {
    bubble_with(n, exact, &|s| {
        s.full_viscous_stress = full; // ADR 0103 (b1f only)
        s.component_diagonal = cd; // ADR 0103 (b1c only)
        s.height_function = hf; // ADR 0106 (b1h only)
    })
}

/// The `vof_bubble.rs` set-up on `n` x `2n` cells; `edit` changes the settings (nothing: the library defaults, ADR 0114).
fn bubble_with(n: usize, exact: bool, edit: &dyn Fn(&mut VofSettings)) -> VofSolver {
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
    alpha.values = if exact {
        exact_fraction(&geom, n) // ADR 0108 (b1x only)
    } else {
        indicator_fraction(&geom, 10, |x| (x[0] - 0.5).hypot(x[1] - 0.5) > 0.25)
    };
    let mut s = VofSettings::new([1000.0, 100.0], [10.0, 1.0]);
    s.gravity = [0.0, -0.98, 0.0];
    s.sigma = 24.5;
    s.n_correctors = 3;
    edit(&mut s);
    s.amg_pressure = true; // CG would take about 2.1 h on 160 x 320 (ADR 0100 smoke); the tree uses AMG above 2000 cells
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

/// Runs `n` x `2n` to `t_end` with `dt = 0.1 / n` (2.5e-3 on 40 x 80, as `vof_bubble.rs`):
/// (y_c(t_end), V_c,max, time of V_c,max, phase-volume drift, bound violation).
fn run(n: usize, t_end: f64, full: bool, cd: bool, hf: bool, exact: bool) -> (f64, f64, f64, f64, f64) {
    run_solver(n, t_end, bubble(n, full, cd, hf, exact))
}

/// [`run`] of a constructed solver.
fn run_solver(n: usize, t_end: f64, mut s: VofSolver) -> (f64, f64, f64, f64, f64) {
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
    run(160, 0.4, false, false, false, false);
}

/// B1-B4 of ADR 0100: errors to the Hysing values on 40 x 80, 80 x 160, 160 x 320; all grids are printed before any assert.
#[test]
#[ignore = "KNOWN MISS (ADR 0100 addendum): errors to Hysing grow under refinement (y_c 0.053, 0.064, 0.069); heavy, about 30 min"]
fn b1_hysing_bubble_error_decreases_under_refinement_known_miss() {
    b1(false, false, false, false);
}

/// ADR 0103: B1-B4 of ADR 0100 (limits unchanged) with the transposed viscous stress on (`full_viscous_stress`). Not run on
/// 160 x 320: its 40 and 80 points are the ADR 0103 D2 runs, whose errors already grow (y_c 0.0594 -> 0.0668, V_c,max 0.0306 ->
/// 0.0322), so B1 fails (ADR 0103 amendment A).
#[test]
#[ignore = "KNOWN MISS (ADR 0103 amendment A): B1 fails on the 40 and 80 points (D2); heavy, about 30 min"]
fn b1f_hysing_bubble_error_decreases_under_refinement_full_viscous_stress_known_miss() {
    b1(true, false, false, false);
}

/// ADR 0103 amendment C: B1-B4 of ADR 0100 (limits unchanged) with the component-diagonal correction (`component_diagonal`). Not
/// run on 160 x 320: its 40 and 80 points are the ADR 0103 C2 runs, whose errors still grow (y_c 0.0046 -> 0.0101, V_c,max 0.0043
/// -> 0.0052), so B1 fails.
#[test]
#[ignore = "KNOWN MISS (ADR 0103 amendment C): B1 fails on the 40 and 80 points (C2); heavy, about 30 min"]
fn b1c_hysing_bubble_error_decreases_under_refinement_component_diagonal_known_miss() {
    b1(false, true, false, false);
}

/// ADR 0106 H4 (the verdict): B1-B4 of ADR 0100 (limits unchanged) with the component-diagonal correction and the
/// height-function curvature (`component_diagonal`, `height_function`). Run once: y_c(3) errors 0.0002, 0.0035, 0.0076 (growing),
/// V_c,max errors 0.0069, 0.0075, 0.0069 (bubble about 3 percent fast on every grid).
#[test]
#[ignore = "KNOWN MISS (ADR 0106 H4): y_c(3) error grows 0.0002 -> 0.0035 -> 0.0076; heavy, about 25 min"]
fn b1h_hysing_bubble_error_decreases_under_refinement_height_function_known_miss() {
    b1(false, true, true, false);
}

/// ADR 0106 H5 (second registered variant, information): component diagonal + height function + transposed viscous stress on
/// 40 x 80 and 80 x 160 only (the single 160 x 320 run goes to H4).
#[test]
#[ignore = "ADR 0106 H5 diagnostic, run once"]
fn h5_height_function_full_viscous_stress_40_80() {
    let r: Vec<_> = [40, 80]
        .into_iter()
        .map(|n| run(n, 3.0, true, true, true, false))
        .collect();
    let ey: Vec<f64> = r.iter().map(|x| (x.0 - YC_REF).abs()).collect();
    let ev: Vec<f64> = r.iter().map(|x| (x.1 - V_REF).abs()).collect();
    println!(
        "H5 errors (40, 80): y_c {ey:?}, V_c,max {ev:?}; e(80) < e(40): y_c {}, V_c,max {}",
        ey[1] < ey[0],
        ev[1] < ev[0]
    );
}

/// ADR 0106 S1 (bubble part) and S2: short runs of H4 / H5 on 40 x 80 (t = 0.05) and H4 on 160 x 320 (t = 0.1); height-function
/// curvature of the indicator initial field on the three grids (construction only).
#[test]
#[ignore = "ADR 0106 smoke"]
fn s_height_function_smoke() {
    for n in [40, 80, 160] {
        let s = bubble(n, false, true, true, false);
        let k = s.curvature();
        let a = &s.transport.alpha.values;
        let e: Vec<f64> = (0..a.len())
            .filter(|&c| a[c] > 0.05 && a[c] < 0.95)
            .map(|c| -k[c] / 4.0 - 1.0)
            .collect();
        let rms = (e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt();
        let max = e.iter().map(|x| x.abs()).fold(0.0, f64::max);
        println!(
            "S2 {n} x {}: indicator field, height function rms {rms:.3e}, max {max:.3e}, fallbacks {:?}",
            2 * n,
            s.curvature_fallbacks()
        );
    }
    run(40, 0.05, false, true, true, false);
    run(40, 0.05, true, true, true, false);
    run(160, 0.1, false, true, true, false);
}

/// ADR 0108 X (the verdict): B1-B4 of ADR 0100 (limits unchanged) with all three options (`component_diagonal`,
/// `height_function`, `full_viscous_stress`) and exact initial fractions. Run once: y_c(3) errors 0.0021, 0.0013, 0.0018 (not
/// monotone), V_c,max errors 0.0015, 0.0008, 0.0001 (monotone); B1 misses on y_c(3).
#[test]
#[ignore = "KNOWN MISS (ADR 0108 X): y_c(3) error 0.0021 -> 0.0013 -> 0.0018 not monotone; heavy, about 25 min"]
fn b1x_hysing_bubble_error_decreases_under_refinement_all_options_exact_fraction_known_miss() {
    b1(true, true, true, true);
}

/// ADR 0108 XC (control, cannot change the verdict): the X series with the 10-point indicator initial field. Run once: y_c(3)
/// errors 0.0019, 0.0013, 0.0019, V_c,max errors 0.0015, 0.0008, 0.0001 (within 2e-4 of X on every grid).
#[test]
#[ignore = "KNOWN MISS (ADR 0108 XC control, ignored in every outcome): y_c(3) error 0.0019 -> 0.0013 -> 0.0019; heavy, about 25 min"]
fn b1xc_hysing_bubble_all_options_indicator_fraction_control_known_miss() {
    b1(true, true, true, false);
}

/// ADR 0108 S1 / S2 smoke: exact-fraction construction on the three grids (curvature of the initial field), X on 40 x 80 to
/// t = 0.05, X and XC on 160 x 320 to t = 0.1.
#[test]
#[ignore = "ADR 0108 smoke"]
fn s_all_options_exact_fraction_smoke() {
    for n in [40, 80, 160] {
        let s = bubble(n, true, true, true, true);
        let k = s.curvature();
        let a = &s.transport.alpha.values;
        let e: Vec<f64> = (0..a.len())
            .filter(|&c| a[c] > 0.05 && a[c] < 0.95)
            .map(|c| -k[c] / 4.0 - 1.0)
            .collect();
        let rms = (e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64).sqrt();
        let max = e.iter().map(|x| x.abs()).fold(0.0, f64::max);
        println!(
            "S1 {n} x {}: exact field, height function rms {rms:.3e}, max {max:.3e}, fallbacks {:?}",
            2 * n,
            s.curvature_fallbacks()
        );
    }
    run(40, 0.05, true, true, true, true);
    run(160, 0.1, true, true, true, true);
    run(160, 0.1, true, true, true, false);
}

fn b1(full: bool, cd: bool, hf: bool, exact: bool) {
    let r: Vec<_> = [40, 80, 160]
        .into_iter()
        .map(|n| run(n, 3.0, full, cd, hf, exact))
        .collect();
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

/// ADR 0114 (3): the full-precision y_c(3) and V_c,max errors of the ADR 0108 X run (40, 80, 160), from its log.
const X43A_EY: [f64; 3] = [0.002053058745789871, 0.0012840785484355965, 0.0018457966779672486];
const X43A_EV: [f64; 3] = [0.0015273890756644926, 0.0007790117810689556, 8.49155348201236e-5];

/// The ADR 0108 X errors (e_y, e_v) on grid `i` for this platform (ADR 0137). The registered values above are from the Windows
/// log. The bubble's initial field and mesh call the platform's libm, so the last digits differ per platform: on x86_64 Linux the
/// 40 x 80 values are those of the ADR 0108 X run made at its code (132c0da) in a Linux container, bit-identical to the HEAD default
/// run there; 80 and 160 were not run on Linux. `None`: no recorded value for this platform and grid (print and skip).
fn x43a(i: usize) -> Option<(f64, f64)> {
    if cfg!(windows) {
        Some((X43A_EY[i], X43A_EV[i]))
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) && i == 0 {
        Some((0.002053061866984063, 0.0015273879501069287))
    } else {
        None
    }
}

/// ADR 0114 (3): the X set-up with no option flag assigned (the library defaults) on grid `i` of 40, 80, 160; asserts the
/// ADR 0108 X errors to 1e-9.
fn default_matches_x43a(i: usize) -> (f64, f64, f64, f64, f64) {
    let n = [40, 80, 160][i];
    let r = run_solver(n, 3.0, bubble_with(n, true, &|_| {}));
    let (ey, ev) = ((r.0 - YC_REF).abs(), (r.1 - V_REF).abs());
    let Some((x_ey, x_ev)) = x43a(i) else {
        println!("X0 {n}: e_y {ey:?}, e_v {ev:?}; no recorded ADR 0108 X value for this platform and grid");
        return r;
    };
    println!(
        "X0 {n}: e_y {ey:?} (43A {x_ey:?}, diff {:.2e}), e_v {ev:?} (43A {x_ev:?}, diff {:.2e}) (limit 1e-9)",
        (ey - x_ey).abs(),
        (ev - x_ev).abs()
    );
    assert!((ey - x_ey).abs() <= 1e-9, "X0 {n} y_c(3) error {ey} vs {x_ey}");
    assert!((ev - x_ev).abs() <= 1e-9, "X0 {n} V_c,max error {ev} vs {x_ev}");
    r
}

/// ADR 0114 (3), not ignored: the 40 x 80 point of the default-settings series equals ADR 0108 X to 1e-9.
#[test]
fn x0_default_settings_reproduce_adr0108_x_40() {
    default_matches_x43a(0);
}

/// ADR 0114 (3): the default-settings series on 40, 80, 160 equals ADR 0108 X to 1e-9 (B1-B4 printed, not asserted: the
/// ADR 0108 known miss stands).
#[test]
#[ignore = "ADR 0114 (3), heavy (about 25 min), run once"]
fn x0_default_settings_reproduce_adr0108_x_series() {
    let r: Vec<_> = (0..3).map(default_matches_x43a).collect();
    for (n, x) in [40, 80, 160].iter().zip(&r) {
        println!("X0 {n}: drift {:.2e} (B4 1e-12), bound {:.2e} (B4 1e-9)", x.3, x.4);
    }
}

/// ADR 0114 (4): the F-055 bubble clause (user text 2026-10-24, Hysing et al. 2009 Table XII code-to-code range) with the
/// default settings and exact initial fractions on 160 x 320. Expected to miss (ADR 0108 X: y_c(3) 1.0795, V_c,max 0.2416).
#[test]
#[ignore = "KNOWN MISS (ADR 0114 (4)): y_c(3) and V_c,max below the published range; heavy, about 20 min"]
fn f055_hysing_bubble_finest_grid_in_published_range_known_miss() {
    let (yc, vmax, tmax, dv, wb) = run_solver(160, 3.0, bubble_with(160, true, &|_| {}));
    println!(
        "F055 160 x 320: y_c(3) {yc:.6} in [1.0799, 1.0817]: {}; V_c,max {vmax:.6} in [0.2417, 0.2421]: {}; peak t {tmax:.4} in [0.9213, 0.9313] (reported): {}; drift {dv:.2e} (1e-12), bound {wb:.2e} (1e-9)",
        (1.0799..=1.0817).contains(&yc),
        (0.2417..=0.2421).contains(&vmax),
        (0.9213..=0.9313).contains(&tmax)
    );
    assert!(dv <= 1e-12 && wb <= 1e-9, "phase volume {dv:.2e}, bound {wb:.2e}");
    assert!((1.0799..=1.0817).contains(&yc), "y_c(3) {yc}");
    assert!((0.2417..=0.2421).contains(&vmax), "V_c,max {vmax}");
}
