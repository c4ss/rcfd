//! ADR 0103: diagnosis of the Hysing case-1 rising-bubble refinement miss of ADR 0100 (F-055). Every experiment is an ignored
//! diagnostic run once for the record; it prints its numbers and the ADR applies the decision rules registered before the run.
//! Set-up of `vof_bubble_refinement_w40d.rs` (Hysing et al. 2009 Table I, free-slip sides, 3 PISO correctors, AMG pressure,
//! dt = 0.1/n) unless a test says otherwise.

use rcfd_fv::vof::{indicator_fraction, AlphaTransport, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, Symmetry, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const YC_REF: f64 = 1.0813;
const V_REF: f64 = 0.2417;

/// The w40d bubble on `n` x `2n` cells with `edit` applied to the settings.
fn bubble(n: usize, edit: &dyn Fn(&mut VofSettings)) -> VofSolver {
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
    s.amg_pressure = true;
    edit(&mut s);
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

/// Runs the bubble to `t_end` with step `dt`: (y_c(t_end), V_c,max, time of V_c,max); prints one line tagged `tag`.
fn rise(tag: &str, n: usize, t_end: f64, dt: f64, edit: &dyn Fn(&mut VofSettings)) -> (f64, f64, f64) {
    let mut s = bubble(n, edit);
    let vol = s.geom.mesh.cell_volumes().to_vec();
    let cc = s.geom.mesh.cell_centres().to_vec();
    let (mut vmax, mut tmax) = (0.0f64, 0.0f64);
    let t0 = std::time::Instant::now();
    while s.time < t_end - 1e-9 {
        s.step(dt).unwrap();
        let a = &s.transport.alpha.values;
        let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
        let vc = (0..a.len())
            .map(|c| (1.0 - a[c]) * vol[c] * s.u.values[c][1])
            .sum::<f64>()
            / m;
        if vc > vmax {
            vmax = vc;
            tmax = s.time;
        }
    }
    let a = &s.transport.alpha.values;
    let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
    let yc = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c] * cc[c][1]).sum::<f64>() / m;
    let finite = s.u.values.iter().all(|v| v.iter().all(|x| x.is_finite()));
    println!(
        "{tag} {n} x {}: dt/dt_cap {:.3}, y_c({t_end}) = {yc:.4} (err {:.4}), V_c,max = {vmax:.4} at t = {tmax:.4} (err {:.4}), finite {finite}, {:.1} s",
        2 * n,
        dt / s.capillary_dt(),
        (yc - YC_REF).abs(),
        (vmax - V_REF).abs(),
        t0.elapsed().as_secs_f64()
    );
    (yc, vmax, tmax)
}

fn std_rise(tag: &str, n: usize, edit: &dyn Fn(&mut VofSettings)) -> (f64, f64, f64) {
    rise(tag, n, 3.0, 0.1 / n as f64, edit)
}

/// D0: control, defaults on 40 x 80 (must equal ADR 0100: 1.0282, 0.2204 at 0.850).
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d0_baseline_40() {
    std_rise("D0", 40, &|_| {});
}

/// D2: transposed viscous stress on (40 x 80, 80 x 160).
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d2_full_viscous_stress() {
    for n in [40, 80] {
        std_rise("D2 full", n, &|s| s.full_viscous_stress = true);
    }
}

/// D2c: viscosity ratio 1 (mu_2 = mu_1 = 10) on 40 x 80, transposed stress off and on.
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d2c_viscosity_ratio_one_control() {
    std_rise("D2c laplacian", 40, &|s| s.mu = [10.0, 10.0]);
    std_rise("D2c full", 40, &|s| {
        s.mu = [10.0, 10.0];
        s.full_viscous_stress = true;
    });
}

/// D3: surface-tension sensitivity on 40 x 80 (sigma x 0.9 and x 1.1).
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d3_sigma_sensitivity() {
    std_rise("D3 0.9 sigma", 40, &|s| s.sigma = 0.9 * 24.5);
    std_rise("D3 1.1 sigma", 40, &|s| s.sigma = 1.1 * 24.5);
}

/// D4: time stepping and pressure-velocity coupling on 40 x 80: dt / 2, PIMPLE with 3 outer iterations, no transient Rhie-Chow.
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d4_time_and_coupling() {
    rise("D4 dt/2", 40, 3.0, 0.05 / 40.0, &|_| {});
    std_rise("D4 pimple3", 40, &|s| s.n_outer = 3);
    std_rise("D4 no ddt corr", 40, &|s| s.ddt_correction = false);
}

/// D5: interface compression and curvature smoothing on 40 x 80.
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d5_compression_and_smoothing() {
    std_rise("D5 c_alpha 0.5", 40, &|s| s.c_alpha = 0.5);
    std_rise("D5 n_smooth 0", 40, &|s| s.n_smooth = 0);
    std_rise("D5 n_smooth 4", 40, &|s| s.n_smooth = 4);
}

/// D7: second-order momentum convection (linear upwind) on 40 x 80 and 80 x 160.
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d7_linear_upwind() {
    for n in [40, 80] {
        std_rise("D7 linear_upwind", n, &|s| {
            s.convection = rcfd_fv::convection::ConvScheme::LinearUpwind
        });
    }
}

/// Static Hysing bubble (g = 0) in the unit square on n x n cells to t = 1 with dt = 0.1/n: (Laplace error, max|U| at t = 1,
/// max over time of max|U|).
fn static_bubble(n: usize, rho: [f64; 2], t_end: f64) -> (f64, f64, f64) {
    let mesh = Arc::new(box_2d(n, n, 1.0, 1.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 1.0);
    alpha.values = indicator_fraction(&geom, 10, |x| (x[0] - 0.5).hypot(x[1] - 0.5) > 0.25);
    let mut st = VofSettings::new(rho, [10.0, 1.0]);
    st.sigma = 24.5;
    st.n_correctors = 3;
    st.amg_pressure = true;
    let mut s = VofSolver::new(geom, u, p_rgh, alpha, st).unwrap();
    let dt = 0.1 / n as f64;
    let dx = 1.0 / n as f64;
    let umax = |s: &VofSolver| s.u.values.iter().map(|v| v[0].hypot(v[1])).fold(0.0, f64::max);
    let mut peak = 0.0f64;
    let t0 = std::time::Instant::now();
    while s.time < t_end - 1e-9 {
        s.step(dt).unwrap();
        peak = peak.max(umax(&s));
    }
    let cc = s.geom.mesh.cell_centres();
    let a = &s.transport.alpha.values;
    let (mut pin, mut nin, mut pout, mut nout) = (0.0, 0, 0.0, 0);
    for c in 0..a.len() {
        let r = (cc[c][0] - 0.5).hypot(cc[c][1] - 0.5);
        if a[c] < 0.01 && r < 0.25 - 3.0 * dx {
            pin += s.p[c];
            nin += 1;
        }
        if a[c] > 0.99 && r > 0.25 + 4.0 * dx {
            pout += s.p[c];
            nout += 1;
        }
    }
    let dp = pin / nin as f64 - pout / nout as f64;
    let err = (dp - 24.5 / 0.25) / (24.5 / 0.25);
    let end = umax(&s);
    println!(
        "D1 static {n} x {n}, rho {rho:?}: dp {dp:.3} vs 98 (signed rel. error {err:.4}), max|U| at t_end {end:.3e}, peak {peak:.3e}, Ca(t_end) {:.2e}, dt/dt_cap {:.3}, {:.1} s",
        10.0 * end / 24.5,
        dt / s.capillary_dt(),
        t0.elapsed().as_secs_f64()
    );
    (err, end, peak)
}

/// D1: static bubble, density ratio 10 on 40, 80, 160 and density ratio 1 on 40, 80.
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d1_static_bubble() {
    for n in [40, 80, 160] {
        static_bubble(n, [1000.0, 100.0], 1.0);
    }
    for n in [40, 80] {
        static_bubble(n, [1000.0, 1000.0], 1.0);
    }
}

/// D1b: curvature of the initial exact circle (R = 0.25, exact 1/R = 4 in magnitude) on the bubble grids, interface cells
/// 0.05 < alpha < 0.95: (mean, rms relative error, max relative error) of -kappa (construction only).
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d1b_initial_curvature() {
    initial_curvature("D1b", &[20, 40, 80, 160, 320], &|_| 2);
}

fn initial_curvature(tag: &str, grids: &[usize], n_smooth: &dyn Fn(usize) -> usize) {
    for &n in grids {
        let s = bubble(n, &|s| s.n_smooth = n_smooth(n));
        let k = s.curvature();
        let a = &s.transport.alpha.values;
        let sel: Vec<f64> = (0..a.len())
            .filter(|&c| a[c] > 0.05 && a[c] < 0.95)
            .map(|c| -k[c])
            .collect();
        let mean = sel.iter().sum::<f64>() / sel.len() as f64;
        let rms = (sel.iter().map(|x| (x / 4.0 - 1.0).powi(2)).sum::<f64>() / sel.len() as f64).sqrt();
        let max = sel.iter().map(|x| (x / 4.0 - 1.0).abs()).fold(0.0, f64::max);
        println!(
            "{tag} {n} x {}, n_smooth {}: {} interface cells, mean -kappa {mean:.4} (exact 4), rms rel. error {rms:.4}, max {max:.4}",
            2 * n,
            s.settings.n_smooth,
            sel.len()
        );
    }
}

/// D6: translation of the bubble with a prescribed uniform flux v = 0.2 for t = 2.5 (displacement 0.5) on the bubble grid:
/// centroid displacement error and L1 shape error against the exact translated indicator.
#[test]
#[ignore = "ADR 0103 diagnostic, run once"]
fn d6_translation() {
    for n in [40, 80] {
        let mesh = Arc::new(box_2d(n, 2 * n, 1.0, 2.0, false, false).unwrap());
        let geom = Arc::new(FvGeometry::new(mesh).unwrap());
        let mut alpha = ScalarField::uniform("alpha", &geom, 1.0);
        alpha.values = indicator_fraction(&geom, 10, |x| (x[0] - 0.5).hypot(x[1] - 0.5) > 0.25);
        let sf = geom.mesh.face_area_vectors();
        let phi: Vec<f64> = (0..geom.mesh.n_faces())
            .map(|f| if geom.empty[f] { 0.0 } else { 0.2 * sf[f][1] })
            .collect();
        let mut t = AlphaTransport::new(geom.clone(), alpha, 1.0, 0.25);
        let dt = 0.1 / n as f64;
        let steps = (2.5 / dt).round() as usize;
        for _ in 0..steps {
            t.advance(&phi, dt).unwrap();
        }
        let vol = geom.mesh.cell_volumes();
        let cc = geom.mesh.cell_centres();
        let a = &t.alpha.values;
        let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
        let yc = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c] * cc[c][1]).sum::<f64>() / m;
        let exact = indicator_fraction(&geom, 10, |x| (x[0] - 0.5).hypot(x[1] - 1.0) > 0.25);
        let l1 = (0..a.len()).map(|c| (a[c] - exact[c]).abs() * vol[c]).sum::<f64>() / m;
        println!(
            "D6 {n} x {}: y_c {yc:.6} (exact 1.0, error {:.2e}), L1 shape error / bubble area {l1:.4}",
            2 * n,
            yc - 1.0
        );
    }
}

/// Smoke of the transposed-stress option on 160 x 320 to t = 0.3 (stability at the finest grid, no reference comparison).
#[test]
#[ignore = "ADR 0103 smoke"]
fn s160_full_viscous_stress_smoke() {
    rise("S160 full", 160, 0.3, 0.1 / 160.0, &|s| s.full_viscous_stress = true);
}

/// Smoke of every set-up: a few steps each.
#[test]
#[ignore = "ADR 0103 smoke"]
fn s_smoke() {
    rise("S full", 40, 0.05, 0.0025, &|s| s.full_viscous_stress = true);
    rise("S mu1", 40, 0.05, 0.0025, &|s| s.mu = [10.0, 10.0]);
    rise("S pimple3", 40, 0.05, 0.0025, &|s| s.n_outer = 3);
    rise("S lu", 40, 0.05, 0.0025, &|s| {
        s.convection = rcfd_fv::convection::ConvScheme::LinearUpwind
    });
    rise("S ns0", 40, 0.05, 0.0025, &|s| s.n_smooth = 0);
    rise("S harmonic", 40, 0.05, 0.0025, &|s| s.harmonic_viscosity = true);
    static_bubble(40, [1000.0, 100.0], 0.05);
    static_bubble(40, [1000.0, 1000.0], 0.05);
}

/// A1 (amendment A): initial-circle curvature with n_smooth = 2 (n/40)^2 (constant physical smoothing width).
#[test]
#[ignore = "ADR 0103 amendment A, run once"]
fn a1_initial_curvature_scaled_smoothing() {
    initial_curvature("A1", &[40, 80, 160], &|n| 2 * (n / 40) * (n / 40));
}

/// A2 (amendment A): 80 x 160 with n_smooth 8.
#[test]
#[ignore = "ADR 0103 amendment A, run once"]
fn a2_scaled_smoothing_80() {
    std_rise("A2 n_smooth 8", 80, &|s| s.n_smooth = 8);
}

/// A3 (amendment A): 80 x 160 with dt = 0.05/80.
#[test]
#[ignore = "ADR 0103 amendment A, run once"]
fn a3_half_step_80() {
    rise("A3 dt/2", 80, 3.0, 0.05 / 80.0, &|_| {});
}

/// A4 (amendment A): harmonic mixture viscosity on 40 x 80 and 80 x 160.
#[test]
#[ignore = "ADR 0103 amendment A, run once"]
fn a4_harmonic_viscosity() {
    for n in [40, 80] {
        std_rise("A4 harmonic mu", n, &|s| s.harmonic_viscosity = true);
    }
}

/// The single 160 x 320 run of amendment A (only if A2 confirms): n_smooth 32.
#[test]
#[ignore = "ADR 0103 amendment A: run only if A2 confirms; about 25 min"]
fn a5_scaled_smoothing_160() {
    std_rise("A5 n_smooth 32", 160, &|s| s.n_smooth = 32);
}

/// B1 (amendment B): impulsive start of the Hysing bubble (sigma 0) in an 8 x 8 free-slip box, one step of 1e-4: acceleration
/// V_c(dt) / dt against the potential-flow added-mass value (rho_1 - rho_2) g / (rho_1 + rho_2) = 0.8018.
fn added_mass(n_per_unit: usize) -> f64 {
    let n = 8 * n_per_unit;
    let mesh = Arc::new(box_2d(n, n, 8.0, 8.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(Symmetry)).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 1.0);
    alpha.values = indicator_fraction(&geom, 10, |x| (x[0] - 4.0).hypot(x[1] - 4.0) > 0.25);
    let mut st = VofSettings::new([1000.0, 100.0], [10.0, 1.0]);
    st.gravity = [0.0, -0.98, 0.0];
    st.n_correctors = 3;
    st.amg_pressure = true;
    let mut s = VofSolver::new(geom, u, p_rgh, alpha, st).unwrap();
    let dt = 1e-4;
    s.step(dt).unwrap();
    let vol = s.geom.mesh.cell_volumes();
    let a = &s.transport.alpha.values;
    let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
    let vc = (0..a.len())
        .map(|c| (1.0 - a[c]) * vol[c] * s.u.values[c][1])
        .sum::<f64>()
        / m;
    let acc = vc / dt;
    println!(
        "B1 h = 1/{n_per_unit} ({n} x {n}): acceleration {acc:.4} (potential flow 0.8018, ratio {:.4})",
        acc / 0.8018
    );
    acc
}

#[test]
#[ignore = "ADR 0103 amendment B, run once"]
fn b1_added_mass_impulsive_start() {
    for k in [20, 40, 80] {
        added_mass(k);
    }
}

/// C1 (amendment C): uniform flow v = 0.2 through a 1 x 2 channel with symmetry side walls (single fluid, exact solution uniform);
/// returns the largest |u_y - 0.2| in the wall-adjacent cells at t = 2.
fn uniform_channel(component_diagonal: bool) -> f64 {
    let n = 40;
    let mesh = Arc::new(box_2d(n, 2 * n, 1.0, 2.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0, 0.2, 0.0]);
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0, 0.2, 0.0])))
        .unwrap();
    for p in ["xmin", "xmax"] {
        u.set_bc(&geom, p, Box::new(Symmetry)).unwrap();
    }
    let mut p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    p_rgh.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let alpha = ScalarField::uniform("alpha", &geom, 1.0);
    let mut st = VofSettings::new([1000.0, 1000.0], [10.0, 10.0]);
    st.n_correctors = 3;
    st.component_diagonal = component_diagonal;
    let mut s = VofSolver::new(geom, u, p_rgh, alpha, st).unwrap();
    s.run_fixed(2.0, 2.5e-3).unwrap();
    let cc = s.geom.mesh.cell_centres();
    let h = 1.0 / n as f64;
    let wall = (0..cc.len())
        .filter(|&c| cc[c][0] < h || cc[c][0] > 1.0 - h)
        .map(|c| (s.u.values[c][1] - 0.2).abs())
        .fold(0.0, f64::max);
    let all =
        s.u.values
            .iter()
            .map(|v| (v[1] - 0.2).abs().max(v[0].abs()))
            .fold(0.0, f64::max);
    println!(
        "C1 component_diagonal {component_diagonal}: wall-column max |u_y - 0.2| {wall:.3e}, whole field max deviation {all:.3e}"
    );
    wall
}

/// C1 rule of ADR 0103 amendment C (not ignored: about 20 s in release): the averaged diagonal slows the wall column by more than
/// 1 % of the exact uniform flow, the component-diagonal correction keeps it within 0.1 %.
#[test]
fn c1_uniform_channel_symmetry_walls() {
    let off = uniform_channel(false);
    let on = uniform_channel(true);
    assert!(off > 0.002, "C1 averaged diagonal: {off}");
    assert!(on < 0.0002, "C1 component diagonal: {on}");
}

/// C2 / C2b (amendment C): the bubble with the component-diagonal correction (and with the transposed stress as well).
#[test]
#[ignore = "ADR 0103 amendment C, run once"]
fn c2_component_diagonal() {
    for n in [40, 80] {
        std_rise("C2 component_diagonal", n, &|s| s.component_diagonal = true);
    }
    for n in [40, 80] {
        std_rise("C2b component_diagonal + full stress", n, &|s| {
            s.component_diagonal = true;
            s.full_viscous_stress = true;
        });
    }
}

/// C3 (amendment C): the bubble on 40 x 80 with no-slip side walls.
#[test]
#[ignore = "ADR 0103 amendment C, run once"]
fn c3_no_slip_side_walls() {
    let mut s = bubble(40, &|_| {});
    for p in ["xmin", "xmax"] {
        s.u.set_bc(&s.geom.clone(), p, Box::new(FixedValue::vector([0.0; 3])))
            .unwrap();
    }
    let vol = s.geom.mesh.cell_volumes().to_vec();
    let cc = s.geom.mesh.cell_centres().to_vec();
    let mut vmax = 0.0f64;
    while s.time < 3.0 - 1e-9 {
        s.step(0.0025).unwrap();
        let a = &s.transport.alpha.values;
        let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
        vmax = vmax.max(
            (0..a.len())
                .map(|c| (1.0 - a[c]) * vol[c] * s.u.values[c][1])
                .sum::<f64>()
                / m,
        );
    }
    let a = &s.transport.alpha.values;
    let m: f64 = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c]).sum();
    let yc = (0..a.len()).map(|c| (1.0 - a[c]) * vol[c] * cc[c][1]).sum::<f64>() / m;
    println!("C3 no-slip side walls 40 x 80: y_c(3) = {yc:.4}, V_c,max = {vmax:.4}");
}
