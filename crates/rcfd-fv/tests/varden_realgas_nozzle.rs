//! ADR 0007 checks (R3 solver level), (R5) and (R6): real-gas (Peng-Robinson CO2) converging and converging-diverging
//! ducts in the pressure-based variable-density kernel against an independent real-gas isentropic reference.
//!
//! The reference integrates the isentropic path from the stagnation state with the same EOS in the test itself:
//! `dT/dp|_s = T (dv/dT)_p / cp`, `d(u^2/2) = -dp/rho` (RK4 in `p`), not through the entropy departure; the energy
//! balance `h + u^2/2 = h0` of the result is checked as a self-consistency test. Geometry, gas state, columns and
//! tolerances are those written in ADR 0007 before the first run.

use rcfd_core::eos_data::critical_data;
use rcfd_core::fluids::{CubicEos, CubicKind};
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::realgas::RealGas;
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{DensityMode, IdealGas, Property, StagnationInlet, VarDensitySolver, VdSettings};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, warp};
use std::sync::Arc;

const H_T: f64 = 0.05;
const L: f64 = 40.0 * H_T;
const CP0: f64 = 940.0;

fn co2() -> RealGas {
    let d = critical_data("CO2").unwrap();
    RealGas {
        eos: CubicEos::new(CubicKind::PengRobinson, d.tc, d.pc, d.omega, d.molar_mass).unwrap(),
        cp0: CP0,
    }
}

/// Isentropic path tabulated on a uniform pressure grid from `p0` downwards.
struct Path {
    p: Vec<f64>,
    t: Vec<f64>,
    ke: Vec<f64>,
    rho: Vec<f64>,
    c: Vec<f64>,
    g: Vec<f64>,
    p0: f64,
    t0: f64,
    i_star: usize,
}

fn integrate_path(rg: &RealGas, p0: f64, t0: f64, n: usize, p_min_frac: f64) -> Path {
    let dp = -(1.0 - p_min_frac) * p0 / n as f64;
    let f = |p: f64, t: f64| -> (f64, f64) {
        let rho = rg.rho(p, t);
        let dt = 1e-4 * t;
        let dvdt = (1.0 / rg.rho(p, t + dt) - 1.0 / rg.rho(p, t - dt)) / (2.0 * dt);
        (t * dvdt / rg.cp(p, t), -1.0 / rho)
    };
    let (mut p, mut t, mut ke) = (p0, t0, 0.0);
    let mut path = Path {
        p: vec![],
        t: vec![],
        ke: vec![],
        rho: vec![],
        c: vec![],
        g: vec![],
        p0,
        t0,
        i_star: 0,
    };
    for _ in 0..=n {
        let rho = rg.rho(p, t);
        path.p.push(p);
        path.t.push(t);
        path.ke.push(ke);
        path.rho.push(rho);
        path.c.push(rg.sound_speed(p, t));
        path.g.push(rho * (2.0 * ke).sqrt());
        let (k1t, k1k) = f(p, t);
        let (k2t, k2k) = f(p + 0.5 * dp, t + 0.5 * dp * k1t);
        let (k3t, k3k) = f(p + 0.5 * dp, t + 0.5 * dp * k2t);
        let (k4t, k4k) = f(p + dp, t + dp * k3t);
        t += dp / 6.0 * (k1t + 2.0 * k2t + 2.0 * k3t + k4t);
        ke += dp / 6.0 * (k1k + 2.0 * k2k + 2.0 * k3k + k4k);
        p += dp;
        let _ = (k1k, k2k, k3k, k4k);
    }
    let mut best = 0;
    for i in 0..path.g.len() {
        if path.g[i] > path.g[best] {
            best = i;
        }
    }
    path.i_star = best;
    path
}

impl Path {
    fn g_max(&self) -> f64 {
        self.g[self.i_star]
    }
    fn mach(&self, i: usize) -> f64 {
        (2.0 * self.ke[i]).sqrt() / self.c[i]
    }
    /// State index pair on the subsonic (`sup = false`, indices up to `i_star`) or supersonic branch where the mass
    /// flux equals `g`, linearly interpolated: returns (p, T, Mach).
    fn at_flux(&self, g: f64, sup: bool) -> (f64, f64, f64) {
        let range: Vec<usize> = if sup {
            (self.i_star..self.g.len() - 1).collect()
        } else {
            (0..self.i_star).collect()
        };
        for &i in &range {
            let (a, b) = (self.g[i] - g, self.g[i + 1] - g);
            if a * b <= 0.0 && a != b {
                let w = a / (a - b);
                let lerp = |x: &Vec<f64>| x[i] + w * (x[i + 1] - x[i]);
                let (p, t) = (lerp(&self.p), lerp(&self.t));
                let m = self.mach(i) + w * (self.mach(i + 1) - self.mach(i));
                return (p, t, m);
            }
        }
        panic!(
            "mass flux {g} not on the {} branch (G_max {})",
            if sup { "supersonic" } else { "subsonic" },
            self.g_max()
        );
    }
    /// Index where the Mach number first reaches `m` (interpolated state): (p, T, G).
    fn at_mach(&self, m: f64) -> (f64, f64, f64) {
        for i in 0..self.g.len() - 1 {
            let (a, b) = (self.mach(i) - m, self.mach(i + 1) - m);
            if a * b <= 0.0 && a != b {
                let w = a / (a - b);
                let lerp = |x: &Vec<f64>| x[i] + w * (x[i + 1] - x[i]);
                return (lerp(&self.p), lerp(&self.t), lerp(&self.g));
            }
        }
        panic!("Mach {m} not reached");
    }
}

fn profile(h_in: f64, h_e: f64, x: f64) -> f64 {
    let s = x / L;
    let h_end = if s <= 0.5 { h_in } else { h_e };
    H_T + (h_end - H_T) * (1.0 + (2.0 * std::f64::consts::PI * s).cos()) / 2.0
}

struct Case {
    sol: VarDensitySolver,
    geom: Arc<FvGeometry>,
    nx: usize,
    ny: usize,
    h_in: f64,
    h_e: f64,
}

#[allow(clippy::too_many_arguments)]
fn build(
    real: bool,
    nx: usize,
    ny: usize,
    h_in: f64,
    h_e: f64,
    p0: f64,
    t0: f64,
    p_out: f64,
    relax: Option<(f64, f64, f64)>,
) -> Case {
    let base = box_2d(nx, ny, L, 1.0, false, false).unwrap();
    let mesh = warp(&base, |p| [p[0], p[1] * profile(h_in, h_e, p[0]), p[2]]).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [30.0, 0.0, 0.0]);
    u.set_bc(&geom, "xmin", Box::new(ZeroGradient)).unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    u.set_bc(&geom, "ymin", Box::new(Symmetry)).unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.5 * (p0 + p_out));
    p.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(p0))).unwrap();
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(p_out))).unwrap();
    p.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    p.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut t = ScalarField::uniform("T", &geom, t0);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(t0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let rg = co2();
    let gas = IdealGas { r: rg.r(), cp: CP0 };
    let mu = 1.8e-5;
    let mut s = VdSettings::new(
        gas,
        Property::Constant(mu),
        Property::Constant(mu * CP0 / 0.71),
        Coupling::Simplec,
    );
    s.mode = DensityMode::Compressible;
    if real {
        s.real = Some(rg);
    }
    if let Some((au, ap, ah)) = relax {
        s.alpha_u = au;
        s.alpha_p = ap;
        s.alpha_h = ah;
    }
    s.inlets = vec![StagnationInlet {
        patch: "xmin".into(),
        p0,
        t0,
    }];
    let sol = VarDensitySolver::new(geom.clone(), u, p, t, s).unwrap();
    Case {
        sol,
        geom,
        nx,
        ny,
        h_in,
        h_e,
    }
}

fn column_mean(c: &Case, i: usize, f: &[f64]) -> f64 {
    let vol = c.geom.mesh.cell_volumes();
    let (mut num, mut den) = (0.0, 0.0);
    for j in 0..c.ny {
        let cell = j * c.nx + i;
        num += vol[cell] * f[cell];
        den += vol[cell];
    }
    num / den
}

fn run(c: &mut Case, iters: usize) {
    for k in 0..iters {
        let info = c.sol.iterate();
        if k % 250 == 0 || k + 1 == iters {
            println!(
                "iter {k}: res u {:.2e} p {:.2e} h {:.2e} cont {:.2e} du {:.2e} mdot {:.4e}",
                info.res_u,
                info.res_p,
                info.res_h,
                info.continuity,
                info.max_du,
                c.sol.mass_flow("xmax").unwrap()
            );
        }
    }
}

struct Errors {
    mach: f64,
    p: f64,
    t: f64,
    mdot_in: f64,
    mdot_out: f64,
    in_out: f64,
}

/// Compare column means with the reference path. `g_throat` is the mass flux at the throat of the reference,
/// `a_t_over_a` the throat-to-star area is implicit: local flux = `g_throat * H_T / h(x)`.
fn report(c: &Case, path: &Path, columns: &[usize], g_throat: f64, throat_x: Option<f64>, mdot_ref: f64) -> Errors {
    let mach = c.sol.mach();
    let p_abs = c.sol.p.values.clone();
    let tt = c.sol.t.values.clone();
    let (mut em, mut ep, mut et) = (0.0f64, 0.0f64, 0.0f64);
    for &i in columns {
        let x = (i as f64 + 0.5) * L / c.nx as f64;
        let g_loc = g_throat * H_T / profile(c.h_in, c.h_e, x);
        let sup = throat_x.map(|xt| x > xt).unwrap_or(false);
        let (p_r, t_r, m_r) = path.at_flux(g_loc, sup);
        let m = column_mean(c, i, &mach);
        let p = column_mean(c, i, &p_abs);
        let t = column_mean(c, i, &tt);
        println!(
            "col {i:3} x {x:.3}: M {m:.4} ({m_r:.4})  p/p0 {:.4} ({:.4})  T/T0 {:.4} ({:.4})",
            p / path.p0,
            p_r / path.p0,
            t / path.t0,
            t_r / path.t0
        );
        em = em.max((m / m_r - 1.0).abs());
        ep = ep.max((p / p_r - 1.0).abs());
        et = et.max((t / t_r - 1.0).abs());
    }
    let m_in = -c.sol.mass_flow("xmin").unwrap();
    let m_out = c.sol.mass_flow("xmax").unwrap();
    Errors {
        mach: em,
        p: ep,
        t: et,
        mdot_in: m_in / mdot_ref - 1.0,
        mdot_out: m_out / mdot_ref - 1.0,
        in_out: (m_in - m_out).abs() / m_in.abs(),
    }
}

#[test]
fn reference_path_is_energy_consistent() {
    let rg = co2();
    let path = integrate_path(&rg, 5.0e6, 400.0, 4000, 0.3);
    let mut worst = 0.0f64;
    for i in (0..path.p.len()).step_by(100) {
        let lhs = rg.h(path.p[i], path.t[i]) + path.ke[i];
        let rhs = rg.h(path.p0, path.t0);
        worst = worst.max((lhs - rhs).abs() / (rg.h(path.p0, path.t0) - rg.h(path.p[i], path.t[i])).abs().max(1.0));
        let ds = (rg.s(path.p[i], path.t[i]) - rg.s(path.p0, path.t0)).abs();
        assert!(ds < 1e-3, "entropy drift {ds} at i={i}");
    }
    println!("energy-balance deviation relative to the enthalpy drop: {worst:.2e}");
    assert!(worst < 1e-4, "{worst}");
    assert!(
        path.i_star > 0 && path.i_star < path.g.len() - 1,
        "no sonic maximum on the path"
    );
    let m_star = path.mach(path.i_star);
    println!(
        "critical state: p*/p0 {:.4}, M* {m_star:.4}",
        path.p[path.i_star] / path.p0
    );
    assert!((m_star - 1.0).abs() < 5e-3, "Mach at the mass-flux maximum {m_star}");
    let (r, gamma) = (rg.r(), CP0 / (CP0 - rg.r()));
    let g_ideal =
        path.p0 * (gamma / (r * path.t0)).sqrt() * (2.0 / (gamma + 1.0)).powf((gamma + 1.0) / (2.0 * (gamma - 1.0)));
    println!(
        "G* real {:.3} ideal-gas formula {g_ideal:.3} ({:+.2} %)",
        path.g_max(),
        100.0 * (path.g_max() / g_ideal - 1.0)
    );
}

#[test]
fn r5_real_gas_subsonic_duct_matches_real_gas_isentropic_flow() {
    let (p0, t0) = (5.0e6, 400.0);
    let rg = co2();
    let path = integrate_path(&rg, p0, t0, 8000, 0.3);
    let (p_t, t_t, g_t) = path.at_mach(0.3);
    let (p_e, t_e, m_e) = path.at_flux(g_t * 0.5, false);
    println!(
        "throat p/p0 {:.4} T {t_t:.2} G {g_t:.3}; A_t/A* {:.4}; exit p {p_e:.0} T {t_e:.2} M {m_e:.4}",
        p_t / p0,
        path.g_max() / g_t
    );
    let (nx, ny) = (200, 10);
    let mut c = build(true, nx, ny, 2.0 * H_T, 2.0 * H_T, p0, t0, p_e, None);
    run(&mut c, 2000);
    let depth = L / nx as f64;
    let mdot_ref = g_t * H_T * depth;
    let cols: Vec<usize> = (0..10).map(|k| 10 + 20 * k).collect();
    let e = report(&c, &path, &cols, g_t, None, mdot_ref);
    println!(
        "r5: Mach err {:.3e}, p err {:.3e}, T err {:.3e}, mdot in {:.3e} out {:.3e}, in/out {:.3e}",
        e.mach, e.p, e.t, e.mdot_in, e.mdot_out, e.in_out
    );
    assert!(
        e.mdot_in.abs() < 0.01 && e.mdot_out.abs() < 0.01,
        "mass flow {} {}",
        e.mdot_in,
        e.mdot_out
    );
    assert!(e.in_out < 1e-6, "in/out {}", e.in_out);
    assert!(e.mach < 0.015, "Mach {}", e.mach);
    assert!(e.p < 0.005, "p {}", e.p);
    assert!(e.t < 0.003, "T {}", e.t);
}

#[test]
#[ignore = "heavy: minutes in release, cargo test --release -p rcfd-fv --test varden_realgas_nozzle -- --ignored"]
fn r6_real_gas_choked_nozzle_mass_flow_and_supersonic_branch() {
    let (p0, t0) = (5.0e6, 400.0);
    let rg = co2();
    let path = integrate_path(&rg, p0, t0, 8000, 0.05);
    let g_star = path.g_max();
    let (p_e, t_e, m_e) = path.at_flux(g_star / 1.5, true);
    // ideal-gas comparison (not a criterion): gamma = cp0/(cp0 - R)
    let r = rg.r();
    let gamma = CP0 / (CP0 - r);
    let g_ideal = p0 * (gamma / (r * t0)).sqrt() * (2.0 / (gamma + 1.0)).powf((gamma + 1.0) / (2.0 * (gamma - 1.0)));
    println!(
        "G* real {g_star:.3} ideal-gas formula {g_ideal:.3} ({:+.2} %); exit p {p_e:.0} ({:.4} p0) T {t_e:.2} M {m_e:.4}",
        100.0 * (g_star / g_ideal - 1.0),
        p_e / p0
    );
    let (nx, ny) = (240, 12);
    // start-up (disclosed, not part of the checks): the ideal-gas strategy of ADR 0005 a2
    let mut c = build(
        true,
        nx,
        ny,
        2.0 * H_T,
        1.5 * H_T,
        p0,
        t0,
        0.98 * p0,
        Some((0.7, 0.5, 0.7)),
    );
    let stages = 40;
    for st in 1..=stages {
        let pb = 0.98 * p0 + (p_e - 0.98 * p0) * st as f64 / stages as f64;
        c.sol
            .p
            .set_bc(&c.geom, "xmax", Box::new(FixedValue::scalar(pb)))
            .unwrap();
        for _ in 0..100 {
            c.sol.iterate();
        }
        println!(
            "stage {st}: p_out {pb:.0}, mdot {:.4e}",
            c.sol.mass_flow("xmax").unwrap()
        );
    }
    run(&mut c, 6000);
    let depth = L / nx as f64;
    let mdot_ref = g_star * H_T * depth;
    let throat_x = 0.5 * L;
    let cols: Vec<usize> = (0..10)
        .map(|k| 12 + 24 * k)
        .filter(|&i| ((i as f64 + 0.5) * L / nx as f64 - throat_x).abs() > 0.5 * L / nx as f64)
        .collect();
    let e = report(&c, &path, &cols, g_star, Some(throat_x), mdot_ref);
    println!(
        "r6: Mach err {:.3e}, p err {:.3e}, mdot in {:.3e} out {:.3e}, in/out {:.3e}",
        e.mach, e.p, e.mdot_in, e.mdot_out, e.in_out
    );
    assert!(
        e.mdot_in.abs() < 0.01 && e.mdot_out.abs() < 0.01,
        "mass flow {} {}",
        e.mdot_in,
        e.mdot_out
    );
    assert!(e.mach < 0.03, "Mach {}", e.mach);
    assert!(e.p < 0.02, "p {}", e.p);
}

#[test]
fn r3_solver_ideal_gas_limit_real_gas_at_2_kpa_matches_ideal_gas_solver() {
    let (p0, t0) = (2000.0, 400.0);
    let rg = co2();
    let path = integrate_path(&rg, p0, t0, 4000, 0.3);
    let (_, _, g_t) = path.at_mach(0.3);
    let (p_e, _, _) = path.at_flux(g_t * 0.5, false);
    let (nx, ny) = (200, 10);
    let mut real = build(true, nx, ny, 2.0 * H_T, 2.0 * H_T, p0, t0, p_e, None);
    let mut ideal = build(false, nx, ny, 2.0 * H_T, 2.0 * H_T, p0, t0, p_e, None);
    run(&mut real, 1500);
    run(&mut ideal, 1500);
    let (mr, mi) = (
        real.sol.mass_flow("xmax").unwrap(),
        ideal.sol.mass_flow("xmax").unwrap(),
    );
    let dm = (mr / mi - 1.0).abs();
    let (mach_r, mach_i) = (real.sol.mach(), ideal.sol.mach());
    let (mut dmach, mut dp, mut dt) = (0.0f64, 0.0f64, 0.0f64);
    for k in 0..10 {
        let i = 10 + 20 * k;
        dmach = dmach.max((column_mean(&real, i, &mach_r) - column_mean(&ideal, i, &mach_i)).abs());
        let (pr, pi) = (
            column_mean(&real, i, &real.sol.p.values),
            column_mean(&ideal, i, &ideal.sol.p.values),
        );
        let (tr, ti) = (
            column_mean(&real, i, &real.sol.t.values),
            column_mean(&ideal, i, &ideal.sol.t.values),
        );
        dp = dp.max((pr / pi - 1.0).abs());
        dt = dt.max((tr / ti - 1.0).abs());
    }
    println!("r3 solver: mdot diff {dm:.3e}, Mach diff {dmach:.3e}, p diff {dp:.3e}, T diff {dt:.3e}");
    assert!(dm <= 1e-3, "mass flow {dm}");
    assert!(dmach <= 2e-3, "Mach {dmach}");
    assert!(dp <= 1e-3, "p {dp}");
    assert!(dt <= 1e-3, "T {dt}");
}
