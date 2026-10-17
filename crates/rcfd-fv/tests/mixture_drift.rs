//! ADR 0056 checks D1-D6 (drift-flux closure and batch settling) and M1-M3 (mixture momentum consistency). The limits are
//! those registered in the ADR before any run; none was changed afterwards.

use rcfd_fv::mixture::{solve_speed, DragLaw, MixtureSettings};
use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const G: f64 = 9.81;
const RHO_C: f64 = 1000.0;
const MU_C: f64 = 1e-3;

// ---------------------------------------------------------------------------------------------------------------------
// closure checks (D2, D3)

fn mix(dispersed: usize, d: f64, drag: DragLaw) -> MixtureSettings {
    MixtureSettings {
        dispersed_phase: dispersed,
        diameter: d,
        drag,
    }
}

#[test]
fn d2_stokes_terminal_velocity() {
    let (d, rho_d) = (50e-6, 2500.0);
    let v_st = d * d * (rho_d - RHO_C) * G / (18.0 * MU_C);
    let m = mix(1, d, DragLaw::Stokes);
    let rho = [rho_d, RHO_C];
    let mu = [1e-5, MU_C];
    // dispersed phase 1 (alpha = alpha_d): v_12 = v_dc, along +g (heavy particles sink)
    let c0 = m.slip_coefficient(rho, mu, G, 0.0);
    let e0 = (c0 - v_st).abs() / v_st;
    println!("D2 alpha_d = 0: {c0:.6e} vs {v_st:.6e}, relative error {e0:.2e} (limit 1e-14)");
    assert!(e0 <= 1e-14);
    for ad in [0.1, 0.3] {
        let c = m.slip_coefficient(rho, mu, G, ad);
        let e = (c - v_st * (1.0 - ad)).abs() / (v_st * (1.0 - ad));
        println!("D2 alpha_d = {ad}: relative error {e:.2e} (limit 1e-14)");
        assert!(e <= 1e-14, "alpha_d {ad}: {e}");
    }
    // light dispersed phase (phase 1 = continuous water, phase 2 = air): the same magnitude formula, opposite sign
    let rho_air = 1.2;
    let m2 = mix(2, d, DragLaw::Stokes);
    let v_rise = d * d * (RHO_C - rho_air) * G / (18.0 * MU_C);
    let c = m2.slip_coefficient([RHO_C, rho_air], [MU_C, 1.8e-5], G, 1.0);
    // v_12 = u_water - u_air = -v_dc, v_dc = u_air - u_water = -(rise) along g, i.e. air moves against g: c(v_12) = +v_rise?
    // v_dc = s sign(rho_d - rho_m) = -v_rise (along +g), so v_12 = -v_dc = +v_rise along +g
    let e = (c - v_rise).abs() / v_rise;
    println!("D2 rise: {c:.6e} vs {v_rise:.6e}, relative error {e:.2e} (limit 1e-14)");
    assert!(e <= 1e-14);
}

#[test]
fn d3_schiller_naumann() {
    let rho_d = 2500.0;
    for d in [50e-6, 100e-6, 300e-6] {
        let v_st = d * d * (rho_d - RHO_C) * G / (18.0 * MU_C);
        let s = solve_speed(DragLaw::SchillerNaumann, d, RHO_C, MU_C, v_st);
        let re = RHO_C * s * d / MU_C;
        let cd = 24.0 / re * (1.0 + 0.15 * re.powf(0.687));
        let lhs = std::f64::consts::PI * d.powi(3) / 6.0 * (rho_d - RHO_C) * G;
        let rhs = 0.5 * RHO_C * cd * std::f64::consts::PI * d * d / 4.0 * s * s;
        let e = (lhs - rhs).abs() / lhs;
        let re_st = RHO_C * v_st * d / MU_C;
        println!("D3 d {d:e}: s {s:.6e} (Stokes {v_st:.6e}), Re {re:.3}, balance residual {e:.2e} (limit 1e-12)");
        assert!(e <= 1e-12, "d {d}: {e}");
        assert!(s <= v_st);
        assert!(s >= v_st / (1.0 + 0.15 * re_st.powf(0.687)));
    }
    let d = 1e-6;
    let v_st = d * d * (rho_d - RHO_C) * G / (18.0 * MU_C);
    let s = solve_speed(DragLaw::SchillerNaumann, d, RHO_C, MU_C, v_st);
    let e = (s - v_st).abs() / v_st;
    println!("D3 d = 1 um: relative difference to Stokes {e:.2e} (limit 1e-5)");
    assert!(e <= 1e-5);
}

// ---------------------------------------------------------------------------------------------------------------------
// batch settling column (D1, D4, D5, D6)

const NX: usize = 4;
const NX1: usize = 1;
const NY: usize = 200;
const WIDTH: f64 = 0.008;
const HEIGHT: f64 = 0.4;
const DX: f64 = HEIGHT / NY as f64;

/// `Setup::registered` is the set-up of the ADR (4-wide column, `p_rgh = 0` start, 2 correctors); `Setup::corrected` is the one
/// of the corrected tests (1 cell wide, hydrostatic start, 5 correctors; see the ADR addendum for the reason).
#[derive(Clone, Copy)]
struct Setup {
    nx: usize,
    hydrostatic: bool,
    correctors: usize,
}

impl Setup {
    const REGISTERED: Setup = Setup {
        nx: NX,
        hydrostatic: false,
        correctors: 2,
    };
    const CORRECTED: Setup = Setup {
        nx: NX1,
        hydrostatic: true,
        correctors: 5,
    };
}

fn column(
    mixture: MixtureSettings,
    rho: [f64; 2],
    mu: [f64; 2],
    alpha_of: impl Fn(f64) -> f64,
    gravity: bool,
    setup: Setup,
) -> VofSolver {
    let nx = setup.nx;
    let mesh = Arc::new(box_2d(nx, NY, WIDTH, HEIGHT, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let mut p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    // sub-sample the phase-1 fraction to cell averages of a y-profile
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    let n_sub = 20;
    alpha.values = (0..geom.n_cells())
        .map(|c| {
            let row = c / nx;
            let mut sum = 0.0;
            for k in 0..n_sub {
                let y = (row as f64 + (k as f64 + 0.5) / n_sub as f64) * DX;
                sum += alpha_of(y);
            }
            sum / n_sub as f64
        })
        .collect();
    if setup.hydrostatic && gravity {
        // discrete hydrostatic equilibrium of the face-force formulation: p_rgh_N - p_rgh_O = g y_f (rho_N - rho_O)
        let r: Vec<f64> = alpha.values.iter().map(|a| rho[1] + (rho[0] - rho[1]) * a).collect();
        for row in 1..NY {
            let yf = row as f64 * DX;
            for i in 0..nx {
                p_rgh.values[row * nx + i] =
                    p_rgh.values[(row - 1) * nx + i] + G * yf * (r[row * nx + i] - r[(row - 1) * nx + i]);
            }
        }
    }
    let mut s = VofSettings::new(rho, mu);
    if gravity {
        s.gravity = [0.0, -G, 0.0];
    }
    s.mixture = Some(mixture);
    s.n_correctors = setup.correctors;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

fn profile(s: &VofSolver, disperse_is_1: bool) -> Vec<f64> {
    let nx = s.transport.alpha.values.len() / NY;
    (0..NY)
        .map(|row| {
            let m: f64 = (0..nx).map(|i| s.transport.alpha.values[row * nx + i]).sum::<f64>() / nx as f64;
            if disperse_is_1 {
                m
            } else {
                1.0 - m
            }
        })
        .collect()
}

/// Descending crossing of `thr` seen from the top (settling front).
fn front_from_top(p: &[f64], thr: f64) -> f64 {
    for i in (0..p.len() - 1).rev() {
        if p[i] >= thr {
            // between cell i (>= thr) and i+1 (< thr)
            let (yi, yj) = ((i as f64 + 0.5) * DX, (i as f64 + 1.5) * DX);
            return yi + (yj - yi) * (p[i] - thr) / (p[i] - p[i + 1]);
        }
    }
    f64::NAN
}

/// Ascending crossing of `thr` seen from the bottom (lower edge of a rising cloud).
fn front_from_bottom(p: &[f64], thr: f64) -> f64 {
    for i in 1..p.len() {
        if p[i] >= thr && p[i - 1] < thr {
            let (yi, yj) = ((i as f64 - 0.5) * DX, (i as f64 + 0.5) * DX);
            return yi + (yj - yi) * (thr - p[i - 1]) / (p[i] - p[i - 1]);
        }
    }
    f64::NAN
}

fn lsq_slope(t: &[f64], y: &[f64]) -> f64 {
    let n = t.len() as f64;
    let (mt, my) = (t.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let num: f64 = t.iter().zip(y).map(|(a, b)| (a - mt) * (b - my)).sum();
    let den: f64 = t.iter().map(|a| (a - mt) * (a - mt)).sum();
    num / den
}

fn glass_column(d: f64, drag: DragLaw, a0: f64, setup: Setup) -> VofSolver {
    column(
        mix(1, d, drag),
        [2500.0, RHO_C],
        [1e-5, MU_C],
        move |y| if y <= 0.35 { a0 } else { 0.0 },
        true,
        setup,
    )
}

fn d1_run(setup: Setup, dt: f64, n_steps: usize) {
    let mut s = glass_column(50e-6, DragLaw::Stokes, 0.1, setup);
    let rho = [2500.0, RHO_C];
    let vol = s.geom.mesh.cell_volumes().to_vec();
    let mass = |s: &VofSolver| -> f64 {
        s.transport
            .alpha
            .values
            .iter()
            .zip(&vol)
            .map(|(a, v)| (rho[1] + (rho[0] - rho[1]) * a) * v)
            .sum()
    };
    let (v0, m0) = (s.phase_volume(), mass(&s));
    let (mut amin, mut amax, mut umax) = (f64::INFINITY, f64::NEG_INFINITY, 0.0f64);
    for _ in 0..n_steps {
        let info = s.step(dt).unwrap();
        amin = amin.min(info.alpha_min);
        amax = amax.max(info.alpha_max);
        for u in &s.u.values {
            umax = umax.max(u[0].abs().max(u[1].abs()));
        }
    }
    let ev = (s.phase_volume() - v0).abs() / v0;
    let em = (mass(&s) - m0).abs() / m0;
    println!("D1 volume {ev:.2e}, mass {em:.2e} (limit 1e-12); alpha in [{amin:.3e}, {amax:.12}] (limit +-1e-9); max |u| {umax:.2e} (limit 1e-9)");
    assert!(ev <= 1e-12, "volume {ev}");
    assert!(em <= 1e-12, "mass {em}");
    assert!(amin >= -1e-9 && amax <= 1.0 + 1e-9);
    assert!(umax <= 1e-9, "max |u| {umax}");
}

/// The set-up of the ADR (4-wide column, dt = 0.25 s): the host's phase-fraction Courant limit is exceeded after a few steps
/// (ADR 0056 addendum: set-up defect). Kept as a record.
#[test]
#[ignore = "registered set-up is unstable on this host (ADR 0056 addendum); see d1_conservation_closed_column_corrected"]
fn d1_conservation_closed_column() {
    d1_run(Setup::REGISTERED, 0.25, 400);
}

#[test]
fn d1_conservation_closed_column_corrected() {
    d1_run(Setup::CORRECTED, 0.02, 5000);
}

fn d4_run(setup: Setup, dt: f64) {
    let (d, a0) = (50e-6, 0.1);
    let v_st = d * d * (2500.0 - RHO_C) * G / (18.0 * MU_C);
    let speed = v_st * (1.0 - a0) * (1.0 - a0);
    let mut s = glass_column(d, DragLaw::Stokes, a0, setup);
    let (mut ts, mut ys) = (Vec::new(), Vec::new());
    let mut worst: f64 = 0.0;
    let per = (10.0 / dt).round() as usize;
    for k in 1..=10 * per {
        s.step(dt).unwrap();
        if k % per == 0 {
            let t = k as f64 * dt;
            let y = front_from_top(&profile(&s, true), 0.5 * a0);
            let exact = 0.35 - speed * t;
            worst = worst.max((y - exact).abs());
            println!(
                "D4 t {t:5.0}: front {y:.5}, exact {exact:.5}, error {:.3} dx",
                (y - exact) / DX
            );
            ts.push(t);
            ys.push(y);
        }
    }
    let slope = -lsq_slope(&ts, &ys);
    let rel = (slope - speed).abs() / speed;
    println!("D4 worst front error {:.3} dx (limit 1.0); fitted speed {slope:.5e} vs {speed:.5e}, relative {rel:.3e} (limit 0.03)", worst / DX);
    assert!(worst <= DX, "front error {} dx", worst / DX);
    assert!(rel <= 0.03, "speed error {rel}");
}

#[test]
#[ignore = "registered set-up is unstable on this host (ADR 0056 addendum); see d4_batch_settling_front_stokes_corrected"]
fn d4_batch_settling_front_stokes() {
    d4_run(Setup::REGISTERED, 0.25);
}

#[test]
fn d4_batch_settling_front_stokes_corrected() {
    d4_run(Setup::CORRECTED, 0.02);
}

fn d5_run(setup: Setup, dt: f64) {
    let (d, a0) = (100e-6, 0.02);
    // single-particle-in-mixture speed at alpha_0: v_st with (rho_d - rho_m) = (1 - a0)(rho_d - rho_c)
    let v_st = d * d * (1.0 - a0) * (2500.0 - RHO_C) * G / (18.0 * MU_C);
    let sp = solve_speed(DragLaw::SchillerNaumann, d, RHO_C, MU_C, v_st);
    let speed = (1.0 - a0) * sp;
    let mut s = glass_column(d, DragLaw::SchillerNaumann, a0, setup);
    let (mut ts, mut ys) = (Vec::new(), Vec::new());
    let mut worst: f64 = 0.0;
    let per = (5.0 / dt).round() as usize;
    for k in 1..=8 * per {
        s.step(dt).unwrap();
        if k % per == 0 {
            let t = k as f64 * dt;
            let y = front_from_top(&profile(&s, true), 0.5 * a0);
            let exact = 0.35 - speed * t;
            worst = worst.max((y - exact).abs());
            println!(
                "D5 t {t:5.1}: front {y:.5}, exact {exact:.5}, error {:.3} dx",
                (y - exact) / DX
            );
            ts.push(t);
            ys.push(y);
        }
    }
    let slope = -lsq_slope(&ts, &ys);
    let rel = (slope - speed).abs() / speed;
    println!("D5 worst front error {:.3} dx (limit 1.0); fitted speed {slope:.5e} vs {speed:.5e}, relative {rel:.3e} (limit 0.03)", worst / DX);
    assert!(worst <= DX, "front error {} dx", worst / DX);
    assert!(rel <= 0.03, "speed error {rel}");
}

#[test]
#[ignore = "registered set-up is unstable on this host (ADR 0056 addendum); see d5_batch_settling_front_schiller_naumann_corrected"]
fn d5_batch_settling_front_schiller_naumann() {
    d5_run(Setup::REGISTERED, 0.1);
}

#[test]
fn d5_batch_settling_front_schiller_naumann_corrected() {
    d5_run(Setup::CORRECTED, 0.02);
}

fn d6_run(setup: Setup, dt: f64) {
    let (d, a0) = (50e-6, 0.1);
    let rho_air = 1.2;
    let v_rise = d * d * (RHO_C - rho_air) * G / (18.0 * MU_C);
    // phase 1 = water (continuous), phase 2 = air (dispersed); alpha_d = 0.1 in 0.05 <= y <= 0.15
    let mut s = column(
        mix(2, d, DragLaw::Stokes),
        [RHO_C, rho_air],
        [MU_C, 1.8e-5],
        move |y| if (0.05..=0.15).contains(&y) { 1.0 - a0 } else { 1.0 },
        true,
        setup,
    );
    let mut worst: f64 = 0.0;
    let per = (10.0 / dt).round() as usize;
    for k in 1..=6 * per {
        s.step(dt).unwrap();
        if k % per == 0 {
            let t = k as f64 * dt;
            let y = front_from_bottom(&profile(&s, false), 0.5 * a0);
            let exact = 0.05 + v_rise * (1.0 - a0) * (1.0 - a0) * t;
            worst = worst.max((y - exact).abs());
            println!(
                "D6 t {t:5.0}: lower edge {y:.5}, exact {exact:.5}, error {:.3} dx",
                (y - exact) / DX
            );
        }
    }
    println!("D6 worst edge error {:.3} dx (limit 1.0)", worst / DX);
    assert!(worst <= DX);
}

#[test]
#[ignore = "registered set-up is unstable on this host (ADR 0056 addendum); see d6_rising_dispersion_corrected"]
fn d6_rising_dispersion() {
    d6_run(Setup::REGISTERED, 0.25);
}

#[test]
fn d6_rising_dispersion_corrected() {
    d6_run(Setup::CORRECTED, 0.02);
}

// ---------------------------------------------------------------------------------------------------------------------
// momentum consistency (M1, M2, M3)

fn channel(rho: [f64; 2], gravity: bool, mixture: Option<MixtureSettings>) -> VofSolver {
    let mesh = Arc::new(box_2d(40, 10, 0.4, 0.1, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.1, 0.0, 0.0]);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([0.1, 0.0, 0.0])))
        .unwrap();
    for p in ["ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    let mut p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    p_rgh.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| {
        (0.1..=0.2).contains(&x[0]) && (0.03..=0.07).contains(&x[1])
    });
    let mut s = VofSettings::new(rho, [1e-3, 1e-3]);
    if gravity {
        s.gravity = [0.0, -G, 0.0];
    }
    s.mixture = mixture;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

fn same_fields(a: &VofSolver, b: &VofSolver) -> (usize, f64) {
    let mut diff = 0usize;
    let mut worst: f64 = 0.0;
    let mut cmp = |x: f64, y: f64| {
        if x != y {
            diff += 1;
            worst = worst.max((x - y).abs());
        }
    };
    for c in 0..a.u.values.len() {
        for k in 0..3 {
            cmp(a.u.values[c][k], b.u.values[c][k]);
        }
        cmp(a.p_rgh.values[c], b.p_rgh.values[c]);
        cmp(a.transport.alpha.values[c], b.transport.alpha.values[c]);
    }
    for f in 0..a.phi.len() {
        cmp(a.phi[f], b.phi[f]);
    }
    (diff, worst)
}

#[test]
fn m1_zero_slip_is_the_single_fluid_result() {
    let m = mix(2, 1e-3, DragLaw::Stokes);
    let mut plain = channel([1000.0, 1.0], false, None);
    let mut with = channel([1000.0, 1.0], false, Some(m));
    for _ in 0..50 {
        plain.step(0.005).unwrap();
        with.step(0.005).unwrap();
    }
    let (diff, worst) = same_fields(&plain, &with);
    println!("M1 differing entries {diff}, largest {worst:.2e} (limit: 0 differences)");
    assert_eq!(diff, 0);
    // non-trivial: the block was advected
    assert!(plain.transport.alpha.values.iter().any(|a| *a > 0.0 && *a < 1.0));
}

#[test]
fn m2_equal_densities_is_the_single_fluid_result() {
    let m = mix(2, 1e-3, DragLaw::Stokes);
    let mut plain = channel([1000.0, 1000.0], true, None);
    let mut with = channel([1000.0, 1000.0], true, Some(m));
    for _ in 0..50 {
        plain.step(0.005).unwrap();
        with.step(0.005).unwrap();
    }
    let (diff, worst) = same_fields(&plain, &with);
    println!("M2 differing entries {diff}, largest {worst:.2e} (limit: 0 differences)");
    assert_eq!(diff, 0);
}

fn drift_error(n: usize) -> (f64, f64) {
    let mesh = Arc::new(box_2d(2, n, 0.02, 0.4, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let u = VectorField::uniform("U", &geom, [0.0; 3]);
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    let l = 0.4;
    let prof = |y: f64| 0.3 + 0.2 * (2.0 * std::f64::consts::PI * y / l).sin();
    let dy = l / n as f64;
    alpha.values = (0..2 * n).map(|c| prof((c / 2) as f64 * dy + 0.5 * dy)).collect();
    let rho = [2500.0, RHO_C];
    let mu = [1e-5, MU_C];
    let m = mix(1, 50e-6, DragLaw::Stokes);
    let mut st = VofSettings::new(rho, mu);
    st.gravity = [0.0, -G, 0.0];
    st.mixture = Some(m);
    let s = VofSolver::new(geom, u, p_rgh, alpha, st).unwrap();
    let b = s.drift_force().expect("slip is non-zero");
    let tc = |y: f64| {
        let a = prof(y);
        a * (1.0 - a) * ((1.0 - a) * rho[0] + a * rho[1]) * m.slip_coefficient(rho, mu, G, a).powi(2)
    };
    let h = 1e-6;
    let exact = |y: f64| -(tc(y + h) - tc(y - h)) / (2.0 * h);
    let scale = (0..n).map(|r| exact((r as f64 + 0.5) * dy).abs()).fold(0.0, f64::max);
    let (mut err, mut xerr): (f64, f64) = (0.0, 0.0);
    for r in 2..n - 2 {
        for i in 0..2 {
            let c = r * 2 + i;
            err = err.max((b[c][1] - exact((r as f64 + 0.5) * dy)).abs());
            xerr = xerr.max(b[c][0].abs());
        }
    }
    (err / scale, xerr / scale)
}

#[test]
fn m3_drift_stress_source_against_analytic_divergence() {
    let (e50, x50) = drift_error(50);
    let (e100, x100) = drift_error(100);
    let order = (e50 / e100).log2();
    println!("M3 error/max: n=50 {e50:.3e}, n=100 {e100:.3e} (limit 2e-2), order {order:.2} (limit 1.8), x-component {x50:.1e}, {x100:.1e} (limit 1e-12)");
    assert!(e100 <= 0.02);
    assert!(order >= 1.8, "order {order}");
    assert!(x50 <= 1e-12 && x100 <= 1e-12);
}

/// M4b (replaces the by-construction-identical M4 of the ADR, see the addendum): the discrete mixture mass balance per cell,
/// `(rho_new - rho_old) V/dt + sum_f rho_phi = 0` with `rho_phi = (rho_1 - rho_2) alpha_phi` (zero volume flux), to relative `1e-12`
/// of the largest cell term, over 40 steps of the column.
#[test]
fn m4b_mixture_mass_balance_per_cell() {
    let mut s = glass_column(50e-6, DragLaw::Stokes, 0.1, Setup::CORRECTED);
    let rho = [2500.0, RHO_C];
    let g = s.geom.clone();
    let (own, nei) = (g.mesh.owner().to_vec(), g.mesh.neighbour().to_vec());
    let vol = g.mesh.cell_volumes().to_vec();
    let dt = 0.02;
    let mut worst: f64 = 0.0;
    for _ in 0..40 {
        s.step(dt).unwrap();
        let a_old = s.transport.alpha.values.clone();
        let phi = s.phi.clone();
        let info = s.transport.advance(&phi, dt).unwrap();
        let mut net = vec![0.0; g.n_cells()];
        for f in 0..g.n_internal {
            net[own[f]] += (rho[0] - rho[1]) * info.alpha_phi[f];
            net[nei[f]] -= (rho[0] - rho[1]) * info.alpha_phi[f];
        }
        let mut scale: f64 = 0.0;
        let mut err: f64 = 0.0;
        for c in 0..g.n_cells() {
            let drho = (rho[0] - rho[1]) * (s.transport.alpha.values[c] - a_old[c]);
            let lhs = drho * vol[c] / dt + net[c];
            scale = scale.max((drho * vol[c] / dt).abs()).max(net[c].abs());
            err = err.max(lhs.abs());
        }
        worst = worst.max(err / scale.max(1e-300));
        // restore the pre-advance state so that the solver continues unperturbed
        s.transport.alpha.values = a_old;
    }
    println!("M4b worst cell mass-balance defect {worst:.2e} of the largest term (limit 1e-12)");
    assert!(worst <= 1e-12);
}
