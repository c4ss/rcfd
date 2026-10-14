//! Variable-density kernel (ADR 0005), check (b): hydrostatic ideal-gas column in a closed box.
//!
//! Checks fixed before the first run (see the ADR): 1 x 40 cells, H = 8000 m, g = 9.81, R = 287, uniform initial
//! p = 101325 Pa and T = 300 K (the mass of the column), compressible mode, total-enthalpy energy equation.
//! b1: T = 300 K at top and bottom (isothermal), rho = rho_b exp(-g y/(R T)); b2: T = 300 K at the bottom and 240 K
//! at the top (steady conduction, constant k), p = p_b (T/T_b)^(g/(R a)) with a = 60/H.
//! Limits: max relative density error at the cell centres 1e-3 (b1) and 2e-3 (b2), max speed 1e-6 m/s,
//! mass conserved to 1e-10.
//!
//! Setup choices not fixed in the ADR (disclosed): mu = 10 Pa s and k = 100 W/m/K. At rest neither enters the
//! analytic solution, but with air properties (mu = 1.8e-5, k = 0.026) the momentum inverse diagonal is ~1e4 and the
//! round-off of the O(1e5) balanced face terms (~1e-8 kg/m2/s) exceeds the conduction mass-flux scale
//! k/(cp H) = 3e-9 kg/m2/s, so the steady-conduction column is not resolvable in double precision.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{DensityMode, IdealGas, Property, VarDensitySolver, VdSettings};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const R: f64 = 287.0;
const G: f64 = 9.81;
const H: f64 = 8000.0;
const N: usize = 40;

fn column(t_top: f64) -> (VarDensitySolver, Arc<FvGeometry>) {
    let geom = Arc::new(FvGeometry::new(Arc::new(box_2d(1, N, 1.0, H, false, false).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for patch in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, patch, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 101_325.0);
    let mut t = ScalarField::uniform("T", &geom, 300.0);
    t.set_bc(&geom, "ymin", Box::new(FixedValue::scalar(300.0))).unwrap();
    t.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(t_top))).unwrap();
    let gas = IdealGas { r: R, cp: 3.5 * R };
    let mut s = VdSettings::new(
        gas,
        Property::Constant(10.0),
        Property::Constant(100.0),
        Coupling::Simplec,
    );
    s.mode = DensityMode::Compressible;
    s.gravity = [0.0, -G, 0.0];
    s.p_ref = (0, 101_325.0);
    s.alpha_h = 1.0;
    s.pressure_solver.tol_rel = 1e-10;
    s.energy_solver.tol_rel = 1e-10;
    let sol = VarDensitySolver::new(geom.clone(), u, p, t, s).unwrap();
    (sol, geom)
}

fn run(t_top: f64, analytic: impl Fn(f64, f64) -> f64, limit: f64) {
    let (mut sol, geom) = column(t_top);
    let m0 = sol.total_mass();
    sol.initialise_hydrostatic(30);
    for _ in 0..600 {
        sol.iterate();
    }
    let yc: Vec<f64> = geom.mesh.cell_centres().iter().map(|c| c[1]).collect();
    let mut worst: f64 = 0.0;
    for c in 0..N {
        let t_exact = 300.0 + (t_top - 300.0) * yc[c] / H;
        let rho_exact = analytic(yc[c], t_exact);
        worst = worst.max((sol.rho[c] / rho_exact - 1.0).abs());
    }
    let speed = sol
        .u
        .values
        .iter()
        .map(|v| (v[0] * v[0] + v[1] * v[1]).sqrt())
        .fold(0.0, f64::max);
    let dm = (sol.total_mass() / m0 - 1.0).abs();
    println!("t_top {t_top}: max rel density error {worst:.3e}, max speed {speed:.3e}, mass drift {dm:.3e}");
    assert!(worst < limit, "density error {worst}");
    assert!(speed < 1e-6, "speed {speed}");
    assert!(dm < 1e-10, "mass drift {dm}");
}

#[test]
fn b1_isothermal_column_follows_the_exponential_atmosphere() {
    let hs = R * 300.0 / G;
    let p_b = 101_325.0 * (H / hs) / (1.0 - (-H / hs).exp());
    run(300.0, |y, _| p_b / (R * 300.0) * (-y / hs).exp(), 1e-3);
}

#[test]
fn b2_linear_temperature_column_follows_the_polytropic_atmosphere() {
    let t_top = 240.0;
    let a = (300.0 - t_top) / H;
    let expo = G / (R * a);
    let shape = move |y: f64| (1.0 - a * y / 300.0).powf(expo) / (R * (300.0 - a * y));
    // mass of the initial state per unit area: p0 H/(R 300); p_b from the mass by quadrature
    let m0 = 101_325.0 * H / (R * 300.0);
    let nq = 200_000;
    let integral: f64 = (0..nq).map(|i| shape((i as f64 + 0.5) * H / nq as f64)).sum::<f64>() * H / nq as f64;
    let p_b = m0 / integral;
    run(t_top, move |y, _| p_b * shape(y), 2e-3);
}
