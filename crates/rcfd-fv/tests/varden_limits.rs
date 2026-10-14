//! Variable-density kernel (ADR 0005), check (d): the constant-density limit against the old kernel.
//!
//! d2: 20 x 20 lid-driven cavity, Re = 100, `linear_upwind`, SIMPLEC, air at 293 K isothermal with nu = mu/rho = 0.01
//! and a lid speed of 1 m/s. Max |u - u_old| / U_lid <= 1e-4 (both components) in `low_mach` mode and <= 1e-3 in
//! `compressible` mode (Mach about 3e-3).
//! d3: Boussinesq limit, 24 x 24 cavity graded 4, Ra = 1e4, Pr = 0.71, air at 300 K, dT = 6 K (epsilon = 0.01,
//! beta = 1/T0), `low_mach`: Nu within 1 % of the old Boussinesq kernel on the same mesh and settings.
//! (d1, bit-identical old paths, is the hash comparison of docs/adr/0005 and is not a test here.)

use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::energy::{BuoyantSimple, EnergySolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::varden::{DensityMode, IdealGas, Property, VarDensitySolver, VdSettings};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, cavity_2d, Grading};
use std::sync::Arc;

const R: f64 = 287.0;
const GAMMA: f64 = 1.4;
const P0: f64 = 101_325.0;

fn lid_cavity_new(mode: DensityMode) -> (Vec<[f64; 3]>, usize) {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(20, 1.0).unwrap())).unwrap());
    let t_air = 293.0;
    let rho = P0 / (R * t_air);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p0 = if mode == DensityMode::Compressible { P0 } else { 0.0 };
    let p = ScalarField::uniform("p", &geom, p0);
    let mut t = ScalarField::uniform("T", &geom, t_air);
    t.set_bc(&geom, "lid", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "walls", Box::new(ZeroGradient)).unwrap();
    let gas = IdealGas {
        r: R,
        cp: GAMMA * R / (GAMMA - 1.0),
    };
    let mut s = VdSettings::new(
        gas,
        Property::Constant(0.01 * rho),
        Property::Constant(0.026),
        Coupling::Simplec,
    );
    s.mode = mode;
    s.convection = ConvScheme::LinearUpwind;
    s.energy_convection = ConvScheme::LinearUpwind;
    s.reference_pressure = P0;
    s.p_ref = (0, p0);
    s.pressure_solver.tol_rel = 1e-8;
    s.momentum_solver.tol_rel = 1e-8;
    s.energy_solver.tol_rel = 1e-10;
    let mut sol = VarDensitySolver::new(geom, u, p, t, s).unwrap();
    let mut it = 0;
    for _ in 0..3000 {
        let info = sol.iterate();
        it += 1;
        if info.max_du < 1e-12 {
            break;
        }
    }
    (sol.u.values.clone(), it)
}

fn lid_cavity_old() -> Vec<[f64; 3]> {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(20, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(0.01, Coupling::Simplec);
    s.convection = ConvScheme::LinearUpwind;
    s.pressure_solver.tol_rel = 1e-8;
    s.momentum_solver.tol_rel = 1e-8;
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let sum = solver.run(3000, 1e-12);
    println!(
        "old kernel: {} iterations, max_du {:.2e}",
        sum.iterations, sum.last.max_du
    );
    solver.u.values.clone()
}

fn max_diff(a: &[[f64; 3]], b: &[[f64; 3]]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (0..3).map(|k| (x[k] - y[k]).abs()).fold(0.0, f64::max))
        .fold(0.0, f64::max)
}

#[test]
fn d2_lid_cavity_low_mach_equals_the_old_kernel() {
    let old = lid_cavity_old();
    let (new, it) = lid_cavity_new(DensityMode::LowMach);
    let d = max_diff(&new, &old);
    println!("d2 low_mach: {it} iterations, max |u - u_old| / U_lid = {d:.3e}");
    assert!(d <= 1e-4, "{d}");
}

#[test]
fn d2_lid_cavity_compressible_equals_the_old_kernel() {
    let old = lid_cavity_old();
    let (new, it) = lid_cavity_new(DensityMode::Compressible);
    let d = max_diff(&new, &old);
    println!("d2 compressible: {it} iterations, max |u - u_old| / U_lid = {d:.3e}");
    assert!(d <= 1e-3, "{d}");
}

const PR: f64 = 0.71;
const RA: f64 = 1.0e4;
const T_MEAN: f64 = 300.0;
const DT: f64 = 6.0;
const G: f64 = 9.81;

fn d3_mesh() -> (Arc<FvGeometry>, f64) {
    let _ = (RA, PR);
    let rho0 = P0 / (R * T_MEAN);
    let mu0 = 1.85e-5;
    let l = (RA * T_MEAN * mu0 * mu0 / (PR * G * rho0 * rho0 * DT)).cbrt();
    let g1 = Grading::Symmetric(4.0);
    let mesh = Arc::new(box_2d_graded(24, 24, l, l, [g1, g1], false, false).unwrap());
    (Arc::new(FvGeometry::new(mesh).unwrap()), l)
}

fn boussinesq_old() -> f64 {
    let g1 = Grading::Symmetric(4.0);
    let mesh = Arc::new(box_2d_graded(24, 24, 1.0, 1.0, [g1, g1], false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut t = ScalarField::uniform("T", &geom, 0.5);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut s = SimpleSettings::new(PR, Coupling::Simplec);
    s.convection = ConvScheme::LinearUpwind;
    s.alpha_u = 0.8;
    let flow = SimpleSolver::new(geom.clone(), u, p, s);
    let mut energy = EnergySolver::new(geom.clone(), t, 1.0);
    energy.convection = ConvScheme::LinearUpwind;
    energy.relax = 0.9;
    let mut sim = BuoyantSimple {
        flow,
        energy,
        beta: 1.0,
        gravity: [0.0, -RA * PR, 0.0],
        t_ref: 0.5,
    };
    for _ in 0..3000 {
        let info = sim.iterate();
        if info.flow.res_u.max(info.res_t) < 1e-10 {
            break;
        }
    }
    let gr = sim.energy.wall_gradient("xmin");
    let pt = geom.mesh.patch("xmin").unwrap();
    let area: f64 = pt.faces().map(|f| geom.mesh.face_area(f)).sum();
    let tot: f64 = pt.faces().zip(&gr).map(|(f, g)| g * geom.mesh.face_area(f)).sum();
    tot / area
}

#[test]
fn d3_boussinesq_limit_nusselt_matches_the_old_kernel() {
    let nu_old = boussinesq_old();
    let (geom, l) = d3_mesh();
    let rho0 = P0 / (R * T_MEAN);
    let mu0 = 1.85e-5;
    let cp = GAMMA * R / (GAMMA - 1.0);
    let k0 = mu0 * cp / PR;
    let (th, tc) = (T_MEAN + 0.5 * DT, T_MEAN - 0.5 * DT);
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut t = ScalarField::uniform("T", &geom, T_MEAN);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(th))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(tc))).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let gas = IdealGas { r: R, cp };
    let mut s = VdSettings::new(gas, Property::Constant(mu0), Property::Constant(k0), Coupling::Simplec);
    s.mode = DensityMode::LowMach;
    s.gravity = [0.0, -G, 0.0];
    s.reference_pressure = P0;
    s.p_ref = (0, 0.0);
    s.convection = ConvScheme::LinearUpwind;
    s.energy_convection = ConvScheme::LinearUpwind;
    s.alpha_u = 0.8;
    s.alpha_h = 0.9;
    let mut sol = VarDensitySolver::new(geom.clone(), u, p, t, s).unwrap();
    for _ in 0..3000 {
        let info = sol.iterate();
        if info.max_du < 1e-12 && info.res_h < 1e-10 {
            break;
        }
    }
    let depth = l / 24.0;
    let nu_new = -sol.heat_flow_out("xmin").unwrap() / (k0 * DT * depth);
    let _ = rho0;
    println!(
        "d3: Nu new {nu_new:.5}, old Boussinesq kernel {nu_old:.5}, relative difference {:.3e}",
        nu_new / nu_old - 1.0
    );
    assert!((nu_new / nu_old - 1.0).abs() < 0.01, "{nu_new} vs {nu_old}");
}
