//! Variable-density kernel (ADR 0005), checks (a1) and (a2): quasi-1-D converging-diverging duct, compressible
//! mode, compared with the isentropic area-Mach relations. All geometry, tolerances and column sets are those
//! written in the ADR before the first run. The duct is the upper half (symmetry at the centre line, slip adiabatic
//! wall on top), unit mesh depth `L/nx`.

use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{DensityMode, IdealGas, Property, StagnationInlet, VarDensitySolver, VdSettings};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, warp};
use std::sync::Arc;

const R: f64 = 287.0;
const GAMMA: f64 = 1.4;
const P0: f64 = 101_325.0;
const T0: f64 = 300.0;
const H_T: f64 = 0.05;
const L: f64 = 40.0 * H_T;

fn area_ratio(m: f64) -> f64 {
    let t = 1.0 + 0.5 * (GAMMA - 1.0) * m * m;
    (1.0 / m) * (2.0 / (GAMMA + 1.0) * t).powf((GAMMA + 1.0) / (2.0 * (GAMMA - 1.0)))
}

/// Mach number for an area ratio on the subsonic or supersonic branch (bisection).
fn mach_of_area(ar: f64, supersonic: bool) -> f64 {
    let (mut lo, mut hi) = if supersonic { (1.0, 10.0) } else { (1e-6, 1.0) };
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let too_big = area_ratio(mid) > ar;
        // subsonic: the area ratio decreases with M; supersonic: it increases
        if too_big == supersonic {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

fn p_ratio(m: f64) -> f64 {
    (1.0 + 0.5 * (GAMMA - 1.0) * m * m).powf(-GAMMA / (GAMMA - 1.0))
}

fn t_ratio(m: f64) -> f64 {
    1.0 / (1.0 + 0.5 * (GAMMA - 1.0) * m * m)
}

fn mdot_star(a_star: f64) -> f64 {
    a_star * P0 * (GAMMA / (R * T0)).sqrt() * (2.0 / (GAMMA + 1.0)).powf((GAMMA + 1.0) / (2.0 * (GAMMA - 1.0)))
}

struct Case {
    sol: VarDensitySolver,
    geom: Arc<FvGeometry>,
    nx: usize,
    ny: usize,
    h_in: f64,
    h_e: f64,
}

fn profile(h_in: f64, h_e: f64, x: f64) -> f64 {
    let s = x / L;
    let h_end = if s <= 0.5 { h_in } else { h_e };
    H_T + (h_end - H_T) * (1.0 + (2.0 * std::f64::consts::PI * s).cos()) / 2.0
}

fn build(nx: usize, ny: usize, h_in: f64, h_e: f64, p_out: f64, relax: Option<(f64, f64, f64)>) -> Case {
    let base = box_2d(nx, ny, L, 1.0, false, false).unwrap();
    let mesh = warp(&base, |p| [p[0], p[1] * profile(h_in, h_e, p[0]), p[2]]).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [30.0, 0.0, 0.0]);
    u.set_bc(&geom, "xmin", Box::new(ZeroGradient)).unwrap();
    u.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    u.set_bc(&geom, "ymin", Box::new(Symmetry)).unwrap();
    u.set_bc(&geom, "ymax", Box::new(Symmetry)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.5 * (P0 + p_out));
    p.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(P0))).unwrap();
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(p_out))).unwrap();
    p.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    p.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut t = ScalarField::uniform("T", &geom, T0);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(T0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let gas = IdealGas {
        r: R,
        cp: GAMMA * R / (GAMMA - 1.0),
    };
    let mu = 1.8e-5;
    let mut s = VdSettings::new(
        gas,
        Property::Constant(mu),
        Property::Constant(mu * gas.cp / 0.71),
        Coupling::Simplec,
    );
    s.mode = DensityMode::Compressible;
    if let Some((au, ap, ah)) = relax {
        s.alpha_u = au;
        s.alpha_p = ap;
        s.alpha_h = ah;
    }
    s.inlets = vec![StagnationInlet {
        patch: "xmin".into(),
        p0: P0,
        t0: T0,
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

/// Volume-weighted column mean of a cell field.
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
                "iter {k}: res u {:.2e} p {:.2e} h {:.2e} cont {:.2e} piters {} du {:.2e} mdot {:.4e}",
                info.res_u,
                info.res_p,
                info.res_h,
                info.continuity,
                info.p_iters,
                info.max_du,
                c.sol.mass_flow("xmax").unwrap()
            );
        }
    }
}

/// A_t/A* of the family: 1 for the choked case, 2.0351 (throat Mach 0.3) for the subsonic one.
fn area_ratio_at_throat(c: &Case) -> f64 {
    if c.h_e < c.h_in {
        1.0
    } else {
        area_ratio(0.3)
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

fn report(c: &Case, columns: &[usize], supersonic_after: Option<f64>, a_star_half: f64) -> Errors {
    let mach = c.sol.mach();
    let p_abs = c.sol.p.values.clone();
    let tt = c.sol.t.values.clone();
    let (mut em, mut ep, mut et) = (0.0f64, 0.0f64, 0.0f64);
    let ratio_throat = area_ratio_at_throat(c);
    for &i in columns {
        let x = (i as f64 + 0.5) * L / c.nx as f64;
        let ar = profile(c.h_in, c.h_e, x) / H_T * ratio_throat;
        let sup = supersonic_after.map(|xt| x > xt).unwrap_or(false);
        let m_ex = mach_of_area(ar, sup);
        let m = column_mean(c, i, &mach);
        let p = column_mean(c, i, &p_abs) / P0;
        let t = column_mean(c, i, &tt) / T0;
        println!(
            "col {i:3} x {x:.3}: M {m:.4} (exact {m_ex:.4})  p/p0 {p:.4} ({:.4})  T/T0 {t:.4} ({:.4})",
            p_ratio(m_ex),
            t_ratio(m_ex)
        );
        em = em.max((m / m_ex - 1.0).abs());
        ep = ep.max((p / p_ratio(m_ex) - 1.0).abs());
        et = et.max((t / t_ratio(m_ex) - 1.0).abs());
    }
    let m_in = -c.sol.mass_flow("xmin").unwrap();
    let m_out = c.sol.mass_flow("xmax").unwrap();
    let m_star = mdot_star(a_star_half);
    Errors {
        mach: em,
        p: ep,
        t: et,
        mdot_in: m_in / m_star - 1.0,
        mdot_out: m_out / m_star - 1.0,
        in_out: (m_in - m_out).abs() / m_in.abs(),
    }
}

#[test]
fn a1_subsonic_nozzle_matches_area_mach_relation() {
    let ar_t = area_ratio(0.3);
    let m_e = mach_of_area(2.0 * ar_t, false);
    let p_e = P0 * p_ratio(m_e);
    println!("A_t/A* = {ar_t:.5}, exit Mach {m_e:.4}, p_e = {p_e:.1}");
    let (nx, ny) = (200, 10);
    let mut c = build(nx, ny, 2.0 * H_T, 2.0 * H_T, p_e, None);
    run(&mut c, 2000);
    let depth = L / nx as f64;
    let a_star_half = H_T * depth / ar_t;
    let cols: Vec<usize> = (0..10).map(|k| 10 + 20 * k).collect();
    let e = report(&c, &cols, None, a_star_half);
    println!(
        "a1: Mach err {:.3e}, p err {:.3e}, T err {:.3e}, mdot in {:.3e} out {:.3e}, in/out {:.3e}",
        e.mach, e.p, e.t, e.mdot_in, e.mdot_out, e.in_out
    );
    assert!(e.mach < 0.015, "Mach {}", e.mach);
    assert!(e.p < 0.005, "p {}", e.p);
    assert!(e.t < 0.003, "T {}", e.t);
    assert!(
        e.mdot_in.abs() < 0.01 && e.mdot_out.abs() < 0.01,
        "mass flow {} {}",
        e.mdot_in,
        e.mdot_out
    );
    assert!(e.in_out < 1e-6, "in/out {}", e.in_out);
}

#[test]
#[ignore = "heavy: about 80 s in release, cargo test --release -- --ignored varden"]
fn a2_choked_nozzle_mass_flow_and_supersonic_branch() {
    let m_e = mach_of_area(1.5, true);
    let p_e = P0 * p_ratio(m_e);
    println!("exit Mach {m_e:.4}, p_e = {p_e:.1}");
    let (nx, ny) = (240, 12);
    // continuation in the back pressure (a start-up strategy, not part of the checks): from 0.98 p0 to the exit
    // value of the registered case in equal steps, then iterate at the registered exit pressure
    let mut c = build(nx, ny, 2.0 * H_T, 1.5 * H_T, 0.98 * P0, Some((0.7, 0.5, 0.7)));
    let stages = 40;
    for st in 1..=stages {
        let pb = 0.98 * P0 + (p_e - 0.98 * P0) * st as f64 / stages as f64;
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
    let a_star_half = H_T * depth;
    let throat_x = 0.5 * L;
    let cols: Vec<usize> = (0..10)
        .map(|k| 12 + 24 * k)
        .filter(|&i| ((i as f64 + 0.5) * L / nx as f64 - throat_x).abs() > 0.5 * L / nx as f64)
        .collect();
    let e = report(&c, &cols, Some(throat_x), a_star_half);
    println!(
        "a2: Mach err {:.3e}, p err {:.3e}, mdot in {:.3e} out {:.3e}, in/out {:.3e}",
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
