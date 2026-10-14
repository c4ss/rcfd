//! Transient variable-density kernel (ADR 0009). Every geometry, parameter and limit is the one registered in the
//! ADR before any transient code existed. A miss is kept as an `#[ignore]`d `*_known_miss` test.
//!
//! T1 closed box with uniform heating (ideal gas, both modes; real gas), T2 temporal order, T3 standing acoustic wave,
//! T5 steady limit, T6 Couette flow with viscous heating. T4 (cavity) is in `varden_transient_cavity.rs`.

use rcfd_fv::bc::{FixedValue, Symmetry};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{DensityMode, IdealGas, Property, TransientSettings, VarDensitySolver, VdSettings};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const R: f64 = 287.0;
const GAMMA: f64 = 1.4;
const P0: f64 = 101_325.0;
const T0: f64 = 300.0;

fn gas() -> IdealGas {
    IdealGas {
        r: R,
        cp: GAMMA * R / (GAMMA - 1.0),
    }
}

/// Tight linear solves so that the inner-iteration tolerance, not the linear solver, limits the accuracy.
fn tighten(s: &mut VdSettings) {
    s.momentum_solver.tol_rel = 1e-10;
    s.momentum_solver.max_iter = 2000;
    s.pressure_solver.tol_rel = 1e-10;
    s.pressure_solver.max_iter = 20000;
    s.energy_solver.tol_rel = 1e-12;
    s.energy_solver.max_iter = 2000;
}

fn settings(mode: DensityMode, scheme: TimeScheme, max_inner: usize, tol: f64) -> VdSettings {
    let g = gas();
    let mut s = VdSettings::new(
        g,
        Property::Constant(1.8e-5),
        Property::Constant(1.8e-5 * g.cp / 0.71),
        Coupling::Simplec,
    );
    s.mode = mode;
    s.reference_pressure = P0;
    s.alpha_h = 1.0; // registered: alpha_u 0.9, alpha_p 1.0, alpha_rho 1, alpha_h 1.0
    s.transient = Some(TransientSettings { scheme, max_inner, tol });
    tighten(&mut s);
    s
}

/// Closed adiabatic no-slip box, `n x n` cells, side `l`, uniform state.
fn closed_box(n: usize, l: f64, mode: DensityMode, s: VdSettings) -> VarDensitySolver {
    let mesh = Arc::new(box_2d(n, n, l, l, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_init = if mode == DensityMode::Compressible { P0 } else { 0.0 };
    let p = ScalarField::uniform("p", &geom, p_init);
    let t = ScalarField::uniform("T", &geom, T0);
    VarDensitySolver::new(geom, u, p, t, s).unwrap()
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().map(|x| x.abs()).fold(0.0, f64::max)
}

struct Box1 {
    p_err: f64,
    t_err: f64,
    m_err: f64,
    u_max: f64,
    uniform: f64,
}

/// T1 for one mode and scheme: worst relative errors over the 20 steps.
fn t1(mode: DensityMode, scheme: TimeScheme) -> Box1 {
    let cv = gas().cp - R;
    let rho0 = P0 / (R * T0);
    let mut s = settings(mode, scheme, 60, 1e-12);
    s.heat_source = rho0 * cv * 100.0;
    let mut sol = closed_box(6, 0.1, mode, s);
    let m0 = sol.total_mass();
    let mut w = Box1 {
        p_err: 0.0,
        t_err: 0.0,
        m_err: 0.0,
        u_max: 0.0,
        uniform: 0.0,
    };
    let dt = 0.05;
    for k in 1..=20 {
        let info = sol.advance(dt).unwrap();
        // the compressible inner change stalls near 1e-11 (round-off of the absolute pressure), below the 1e-12 target
        assert!(
            info.change < 1e-9,
            "step {k}: inner change {:.2e} after {}",
            info.change,
            info.inner
        );
        let t_an = T0 + 100.0 * dt * k as f64;
        let p_an = rho0 * R * t_an;
        let pa = sol.absolute_pressure();
        let p_meas = if mode == DensityMode::Compressible {
            pa[0]
        } else {
            sol.p_th
        };
        w.p_err = w.p_err.max((p_meas / p_an - 1.0).abs());
        w.t_err = w.t_err.max((sol.t.values[0] / t_an - 1.0).abs());
        w.m_err = w.m_err.max((sol.total_mass() / m0 - 1.0).abs());
        let speed: Vec<f64> = sol.u.values.iter().map(|v| v[0].abs().max(v[1].abs())).collect();
        w.u_max = w.u_max.max(max_abs(&speed));
        let (tmin, tmax) = sol
            .t
            .values
            .iter()
            .fold((f64::MAX, 0.0f64), |a, &x| (a.0.min(x), a.1.max(x)));
        let (pmin, pmax) = pa.iter().fold((f64::MAX, 0.0f64), |a, &x| (a.0.min(x), a.1.max(x)));
        w.uniform = w.uniform.max((tmax - tmin) / tmax).max((pmax - pmin) / pmax);
    }
    w
}

fn check_t1(w: &Box1, label: &str) {
    println!(
        "T1 {label}: p {:.2e} T {:.2e} mass {:.2e} |u| {:.2e} nonuniformity {:.2e}",
        w.p_err, w.t_err, w.m_err, w.u_max, w.uniform
    );
    assert!(w.p_err <= 1e-6, "{label}: pressure error {}", w.p_err);
    assert!(w.t_err <= 1e-6, "{label}: temperature error {}", w.t_err);
    assert!(w.m_err <= 1e-11, "{label}: mass error {}", w.m_err);
    assert!(w.u_max <= 1e-8, "{label}: spurious velocity {}", w.u_max);
    assert!(w.uniform <= 1e-9, "{label}: non-uniformity {}", w.uniform);
}

#[test]
fn t1_closed_box_heating_compressible_euler() {
    check_t1(
        &t1(DensityMode::Compressible, TimeScheme::ImplicitEuler),
        "compressible euler",
    );
}

#[test]
fn t1_closed_box_heating_compressible_bdf2() {
    check_t1(&t1(DensityMode::Compressible, TimeScheme::Bdf2), "compressible bdf2");
}

#[test]
fn t1_closed_box_heating_low_mach_euler() {
    check_t1(&t1(DensityMode::LowMach, TimeScheme::ImplicitEuler), "low-mach euler");
}

#[test]
fn t1_closed_box_heating_low_mach_bdf2() {
    check_t1(&t1(DensityMode::LowMach, TimeScheme::Bdf2), "low-mach bdf2");
}

/// T1r: CO2 (Peng-Robinson) closed box, first-law and equation-of-state consistency.
#[test]
fn t1r_closed_box_heating_real_gas() {
    use rcfd_core::eos_data::critical_data;
    use rcfd_core::fluids::{CubicEos, CubicKind, R_UNIVERSAL};
    use rcfd_fv::realgas::RealGas;
    let d = critical_data("CO2").unwrap();
    let rg = RealGas {
        eos: CubicEos::new(CubicKind::PengRobinson, d.tc, d.pc, d.omega, d.molar_mass).unwrap(),
        cp0: 940.0,
    };
    let (p0, t0, q) = (8.0e6, 330.0, 7.0e6);
    let rho0 = rg.rho(p0, t0);
    let u0 = rg.h(p0, t0) - p0 / rho0;
    let molar = R_UNIVERSAL / rg.r();
    let g = IdealGas { r: rg.r(), cp: 940.0 };
    for scheme in [TimeScheme::ImplicitEuler, TimeScheme::Bdf2] {
        let mut s = VdSettings::new(
            g,
            Property::Constant(1.5e-5),
            Property::Constant(0.03),
            Coupling::Simplec,
        );
        s.real = Some(rg);
        s.alpha_h = 1.0;
        s.heat_source = q;
        s.transient = Some(TransientSettings {
            scheme,
            max_inner: 60,
            tol: 1e-12,
        });
        tighten(&mut s);
        let mesh = Arc::new(box_2d(4, 4, 0.1, 0.1, false, false).unwrap());
        let geom = Arc::new(FvGeometry::new(mesh).unwrap());
        let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
        for p in ["xmin", "xmax", "ymin", "ymax"] {
            u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
        }
        let pf = ScalarField::uniform("p", &geom, p0);
        let tf = ScalarField::uniform("T", &geom, t0);
        let mut sol = VarDensitySolver::new(geom, u, pf, tf, s).unwrap();
        let m0 = sol.total_mass();
        let (mut e_energy, mut e_p, mut e_m) = (0.0f64, 0.0f64, 0.0f64);
        let mut t_end = 0.0;
        for k in 1..=20 {
            let info = sol.advance(0.05).unwrap();
            assert!(info.change < 1e-9, "step {k}: inner change {:.2e}", info.change);
            let (p, t) = (sol.p.values[0], sol.t.values[0]);
            let time = 0.05 * k as f64;
            let du = rho0 * ((rg.h(p, t) - p / rho0) - u0);
            e_energy = e_energy.max((du / (q * time) - 1.0).abs());
            let p_eos = rg.eos.pressure(t, molar / rho0);
            e_p = e_p.max((p / p_eos - 1.0).abs());
            e_m = e_m.max((sol.total_mass() / m0 - 1.0).abs());
            t_end = t;
        }
        println!("T1r {scheme:?}: energy balance {e_energy:.2e} p vs EOS {e_p:.2e} mass {e_m:.2e}, T_end {t_end:.2} K");
        assert!(e_energy <= 1e-6, "energy balance {e_energy}");
        assert!(e_p <= 1e-6, "p vs EOS {e_p}");
        assert!(e_m <= 1e-11, "mass {e_m}");
    }
}

/// T2: observed temporal order with a time-varying heat source, `T(t)` known in closed form.
fn t2_error(scheme: TimeScheme, n: usize) -> f64 {
    let cv = gas().cp - R;
    let rho0 = P0 / (R * T0);
    let q0 = rho0 * cv * 100.0;
    let t_end = 1.0;
    let mut s = settings(DensityMode::Compressible, scheme, 60, 1e-12);
    s.heat_source = 0.0;
    let mut sol = closed_box(4, 0.1, DensityMode::Compressible, s);
    let dt = t_end / n as f64;
    let mut worst = 0.0f64;
    for k in 1..=n {
        let t = dt * k as f64;
        let w = 2.0 * std::f64::consts::PI / t_end;
        sol.settings.heat_source = q0 * (w * t).sin();
        sol.advance(dt).unwrap();
        let t_an = T0 + q0 / (rho0 * cv * w) * (1.0 - (w * t).cos());
        worst = worst.max((sol.t.values[0] - t_an).abs());
    }
    worst
}

fn t2_order(scheme: TimeScheme) -> f64 {
    let e: Vec<f64> = [10usize, 20, 40, 80].iter().map(|&n| t2_error(scheme, n)).collect();
    let orders: Vec<f64> = e.windows(2).map(|w| (w[0] / w[1]).log2()).collect();
    println!("T2 {scheme:?}: errors {e:?} orders {orders:?}");
    orders[2]
}

#[test]
fn t2_euler_is_first_order() {
    let o = t2_order(TimeScheme::ImplicitEuler);
    assert!((0.90..=1.10).contains(&o), "Euler order {o}");
}

#[test]
fn t2_bdf2_is_second_order() {
    let o = t2_order(TimeScheme::Bdf2);
    assert!((1.85..=2.15).contains(&o), "BDF2 order {o}");
}

// ---- T3: standing acoustic wave in a closed tube ----------------------------------------------------------

const TUBE_L: f64 = 1.0;
const EPS: f64 = 1e-4;
const NX: usize = 200;
/// Registered strip height (one cell high).
const LY: f64 = 0.01;

struct Wave {
    sol: VarDensitySolver,
    x: Vec<f64>,
}

fn wave(scheme: TimeScheme, nx: usize, ly: f64, corrected: bool) -> Wave {
    let mesh = Arc::new(box_2d(nx, 1, TUBE_L, ly, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "xmin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "xmax", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymin", Box::new(Symmetry)).unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    let x: Vec<f64> = geom.mesh.cell_centres().iter().map(|c| c[0]).collect();
    let mut p = ScalarField::uniform("p", &geom, P0);
    let mut t = ScalarField::uniform("T", &geom, T0);
    for (c, &xc) in x.iter().enumerate() {
        let pc = P0 * (1.0 + EPS * (std::f64::consts::PI * xc / TUBE_L).cos());
        p.values[c] = pc;
        t.values[c] = T0 * (pc / P0).powf((GAMMA - 1.0) / GAMMA);
    }
    let mut s = settings(DensityMode::Compressible, scheme, 40, 1e-10);
    s.alpha_u = 1.0;
    s.alpha_p = 1.0;
    s.alpha_h = 1.0;
    s.convection = ConvScheme::Central;
    s.energy_convection = ConvScheme::Central;
    s.component_diag_correction = corrected;
    Wave {
        sol: VarDensitySolver::new(geom, u, p, t, s).unwrap(),
        x,
    }
}

fn omega() -> f64 {
    std::f64::consts::PI * (GAMMA * R * T0).sqrt() / TUBE_L
}

/// Probe series `(t, p' at the closed-end cell)` over `periods` periods with `per_period` steps each.
fn run_wave(
    scheme: TimeScheme,
    per_period: usize,
    periods: usize,
    nx: usize,
    ly: f64,
    corrected: bool,
) -> (Wave, Vec<(f64, f64)>) {
    let mut w = wave(scheme, nx, ly, corrected);
    let tp = 2.0 * std::f64::consts::PI / omega();
    let dt = tp / per_period as f64;
    let mut series = vec![(0.0, w.sol.p.values[0] - P0)];
    for k in 1..=per_period * periods {
        let info = w.sol.advance(dt).unwrap();
        assert!(
            info.change < 1e-7,
            "step {k}: inner change {:.2e} after {}",
            info.change,
            info.inner
        );
        series.push((w.sol.time, w.sol.p.values[0] - P0));
    }
    (w, series)
}

fn crossings(series: &[(f64, f64)], n: usize) -> Vec<f64> {
    let mut out = Vec::new();
    for w in series.windows(2) {
        if w[0].1 * w[1].1 < 0.0 {
            out.push(w[0].0 + (w[1].0 - w[0].0) * w[0].1 / (w[0].1 - w[1].1));
            if out.len() == n {
                break;
            }
        }
    }
    out
}

fn t3ab(scheme: TimeScheme) -> (f64, f64) {
    let (_, series) = run_wave(scheme, 100, 4, NX, LY, false);
    let c = crossings(&series, 8);
    assert_eq!(c.len(), 8, "crossings found: {}", c.len());
    let omega_num = std::f64::consts::PI * 7.0 / (c[7] - c[0]);
    let amp = series.last().unwrap().1 / series[0].1;
    let freq_err = omega_num / omega() - 1.0;
    println!("T3 {scheme:?}: omega_num/omega - 1 = {freq_err:.3e}, amplitude ratio after 4 periods {amp:.5}");
    (freq_err, amp)
}

#[test]
fn t3a_frequency_bdf2() {
    let (f, _) = t3ab(TimeScheme::Bdf2);
    assert!(f.abs() <= 0.01, "frequency error {f}");
}

#[test]
fn t3a_frequency_euler() {
    let (f, _) = t3ab(TimeScheme::ImplicitEuler);
    assert!(f.abs() <= 0.01, "frequency error {f}");
}

#[test]
fn t3b_amplitude_bdf2() {
    let (_, a) = t3ab(TimeScheme::Bdf2);
    assert!((0.98..=1.001).contains(&a), "amplitude ratio {a}");
}

fn t3c_order(scheme: TimeScheme, nx: usize, ly: f64, corrected: bool) -> f64 {
    let tp = 2.0 * std::f64::consts::PI / omega();
    let e: Vec<f64> = [50usize, 100, 200]
        .iter()
        .map(|&n| {
            let (w, _) = run_wave(scheme, n, 1, nx, ly, corrected);
            let sq: f64 =
                w.x.iter()
                    .enumerate()
                    .map(|(c, &x)| {
                        let exact = EPS * P0 * (std::f64::consts::PI * x / TUBE_L).cos() * (omega() * tp).cos();
                        (w.sol.p.values[c] - P0 - exact).powi(2)
                    })
                    .sum();
            (sq / w.x.len() as f64).sqrt() / (EPS * P0)
        })
        .collect();
    println!(
        "T3c {scheme:?}: errors {e:?}, orders {:?}",
        [(e[0] / e[1]).log2(), (e[1] / e[2]).log2()]
    );
    (e[1] / e[2]).log2()
}

#[test]
fn t3c_order_euler() {
    let o = t3c_order(TimeScheme::ImplicitEuler, NX, LY, false);
    assert!((0.85..=1.15).contains(&o), "Euler order {o}");
}

#[test]
fn t3c_order_euler_corrected_diagonal() {
    let o = t3c_order(TimeScheme::ImplicitEuler, NX, LY, true);
    assert!((0.85..=1.15).contains(&o), "Euler order {o}");
}

/// Registered limit `[1.80, 2.20]`, run unchanged with the component-consistent diagonal (ADR 0010,
/// `solver.component_diagonal = "corrected"`): measured 2.08 (errors 1.05e-2, 2.37e-3, 5.6e-4). With the historical
/// averaged diagonal it MISSES, see `t3c_order_bdf2_averaged_diagonal_known_miss` (1.34). The earlier explanation
/// (a spatial floor) was wrong: the floor came from the averaged diagonal on the one-cell-high strip.
#[test]
fn t3c_order_bdf2() {
    let o = t3c_order(TimeScheme::Bdf2, NX, LY, true);
    assert!((1.80..=2.20).contains(&o), "BDF2 order {o}");
}

/// The same registered limit with the historical averaged diagonal (the default): MISSED, measured 1.34 (errors
/// 1.1e-2, 3.0e-3, 1.2e-3). Cause: the pressure-equation diagonal `a_P` is the mean of the component diagonals, and
/// on the one-cell-high strip the Dirichlet normal component at the symmetry planes adds a viscous term `~ mu/ly^2`
/// to the y diagonal, which reduces the acceleration of `u_x` by about 1e-5 per step.
#[test]
#[ignore = "known miss of the registered T3c BDF2 order limit with the averaged diagonal (measured 1.34 vs [1.80, 2.20]); passes with component_diagonal = corrected"]
fn t3c_order_bdf2_averaged_diagonal_known_miss() {
    let o = t3c_order(TimeScheme::Bdf2, NX, LY, false);
    assert!((1.80..=2.20).contains(&o), "BDF2 order {o}");
}

/// Post-hoc diagnostic, NOT a registered gate. Averaged diagonal on a strip ten times higher (`ly = 0.1`): order 2.07
/// (errors 1.05e-2, 2.38e-3, 5.7e-4); this is what located the cause above.
#[test]
#[ignore = "post-hoc diagnostic (not a registered criterion)"]
fn t3c_order_bdf2_posthoc_tall_strip() {
    let o = t3c_order(TimeScheme::Bdf2, NX, 0.1, false);
    println!("post-hoc BDF2 order, ly = 0.1: {o}");
}

// ---------------------------------------------------------------------------------------------------------------
// T6: plane Couette flow with viscous heating (ADR 0009), analytic temperature profile.

const H: f64 = 0.01;
const U_WALL: f64 = 100.0;
const MU: f64 = 1.8e-5;
const K_C: f64 = 0.026;
const TW: f64 = 300.0;

struct Couette {
    max_dt: f64,
    q_bottom: f64,
    q_top: f64,
}

/// `adiabatic_top`: case B (bottom isothermal, top adiabatic), otherwise case A (both isothermal).
fn couette(mode: DensityMode, adiabatic_top: bool, viscous_heating: bool) -> Couette {
    let mut s = settings(mode, TimeScheme::Bdf2, 40, 1e-10);
    s.mu = Property::Constant(MU);
    s.k = Property::Constant(K_C);
    s.viscous_heating = viscous_heating;
    let mesh = Arc::new(box_2d(4, 40, 0.004, H, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let cc = geom.mesh.cell_centres().to_vec();
    let fc = geom.mesh.face_centres().to_vec();
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for (c, v) in cc.iter().zip(u.values.iter_mut()) {
        *v = [U_WALL * c[1] / H, 0.0, 0.0];
    }
    u.set_bc(&geom, "ymin", Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    u.set_bc(&geom, "ymax", Box::new(FixedValue::vector([U_WALL, 0.0, 0.0])))
        .unwrap();
    for patch in ["xmin", "xmax"] {
        let prof: Vec<[f64; 3]> = geom
            .mesh
            .patch(patch)
            .unwrap()
            .faces()
            .map(|f| [U_WALL * fc[f][1] / H, 0.0, 0.0])
            .collect();
        u.set_bc(&geom, patch, Box::new(FixedValue(rcfd_fv::bc::Profile::PerFace(prof))))
            .unwrap();
    }
    let p_init = if mode == DensityMode::Compressible { P0 } else { 0.0 };
    let p = ScalarField::uniform("p", &geom, p_init);
    let mut t = ScalarField::uniform("T", &geom, TW);
    t.set_bc(&geom, "ymin", Box::new(FixedValue::scalar(TW))).unwrap();
    if !adiabatic_top {
        t.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(TW))).unwrap();
    }
    let mut sol = VarDensitySolver::new(geom.clone(), u, p, t, s).unwrap();
    let mut worst_change = 0.0f64;
    for _ in 0..40 {
        let info = sol.advance(0.5).unwrap();
        worst_change = worst_change.max(info.change);
    }
    // the inner iteration of the first steps (flow Courant number ~5e4 through the imposed-velocity ends) stops at the
    // 40 inner iterations below 1e-4; the final state is converged
    assert!(worst_change < 1e-4, "Couette: worst inner change {worst_change:.2e}");
    let scale = MU * U_WALL * U_WALL / (2.0 * K_C);
    let mut max_dt = 0.0f64;
    for (c, &tc) in cc.iter().zip(&sol.t.values) {
        let y = c[1] / H;
        let t_an = if adiabatic_top {
            TW + scale * y * (2.0 - y)
        } else {
            TW + scale * y * (1.0 - y)
        };
        max_dt = max_dt.max(if viscous_heating {
            (tc - t_an).abs() / scale
        } else {
            (tc - TW).abs()
        });
    }
    let area = |patch: &str| -> f64 {
        let sf = geom.mesh.face_area_vectors();
        geom.mesh
            .patch(patch)
            .unwrap()
            .faces()
            .map(|f| rcfd_core::vec3::norm(sf[f]))
            .sum()
    };
    let q_bottom = sol.heat_flow_out("ymin").unwrap() / area("ymin");
    let q_top = sol.heat_flow_out("ymax").unwrap() / area("ymax");
    Couette {
        max_dt,
        q_bottom,
        q_top,
    }
}

fn check_couette(mode: DensityMode, label: &str) {
    let w = MU * U_WALL * U_WALL / H;
    let a = couette(mode, false, true);
    println!(
        "T6 {label} A: max |dT|/scale {:.3e}, q_bottom/(w/2) {:.5}, q_top/(w/2) {:.5}",
        a.max_dt,
        a.q_bottom / (0.5 * w),
        a.q_top / (0.5 * w)
    );
    assert!(a.max_dt <= 0.01, "{label} A profile {}", a.max_dt);
    assert!((a.q_bottom / (0.5 * w) - 1.0).abs() <= 0.01, "{label} A bottom flux");
    assert!((a.q_top / (0.5 * w) - 1.0).abs() <= 0.01, "{label} A top flux");
    let b = couette(mode, true, true);
    println!(
        "T6 {label} B: max |dT|/scale {:.3e}, q_bottom/w {:.5}, |q_top|/q_bottom {:.3e}",
        b.max_dt,
        b.q_bottom / w,
        b.q_top.abs() / b.q_bottom
    );
    assert!(b.max_dt <= 0.01, "{label} B profile {}", b.max_dt);
    assert!((b.q_bottom / w - 1.0).abs() <= 0.01, "{label} B bottom flux");
    assert!(b.q_top.abs() <= 1e-3 * b.q_bottom, "{label} B top flux");
    let off = couette(mode, false, false);
    println!("T6 {label} switch off: max |T - T_w| {:.3e} K", off.max_dt);
    assert!(off.max_dt <= 1e-6, "{label} switch-off {}", off.max_dt);
}

#[test]
fn t6_couette_viscous_heating_compressible() {
    check_couette(DensityMode::Compressible, "compressible");
}

#[test]
fn t6_couette_viscous_heating_low_mach() {
    check_couette(DensityMode::LowMach, "low-mach");
}
