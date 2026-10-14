//! Variable-density kernel (ADR 0005), check (c): differentially heated square cavity with a large temperature
//! difference in `low_mach` mode, against Le Quere, Weisman, Paillere, Vierendeels, Dick, Becker, Braack and Locke,
//! ESAIM: M2AN 39 (2005) 609-616, DOI 10.1051/m2an:2005027, Table 1 (values verified from the author's postprint,
//! see docs/reference-data.md): T0 = 600 K, epsilon = 0.6, P0 = 101325 Pa, Pr = 0.71, Ra = 1e6.
//!
//! Checks written in the ADR before any run: Nu on both walls within 1.0 % of the reference, P/P0 within 0.3 %, hot
//! and cold wall heat flow equal to 1e-6 relative. T1 constant properties (mu = mu(T0) of the Sutherland law,
//! k = mu cp/Pr), T2 Sutherland mu(T) with k = mu cp/Pr; mesh 96 x 96 with symmetric grading 8; `linear_upwind`
//! momentum and energy; SIMPLEC.

use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{DensityMode, IdealGas, Property, VarDensitySolver, VdSettings};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use std::sync::Arc;

const R: f64 = 287.0;
const GAMMA: f64 = 1.4;
const P0: f64 = 101_325.0;
const T0: f64 = 600.0;
const EPS: f64 = 0.6;
const PR: f64 = 0.71;
const G: f64 = 9.81;
const RA: f64 = 1.0e6;

fn sutherland() -> Property {
    Property::Sutherland {
        reference: 1.68e-5,
        t_ref: 273.0,
        s: 110.5,
    }
}

struct Outcome {
    nu_hot: f64,
    nu_cold: f64,
    p_ratio: f64,
    q_hot: f64,
    q_cold: f64,
    iterations: usize,
    residual: f64,
}

fn solve(variable_properties: bool, n: usize, grading: f64, max_iter: usize, tol: f64) -> Outcome {
    let cp = GAMMA * R / (GAMMA - 1.0);
    let th = T0 * (1.0 + EPS);
    let tc = T0 * (1.0 - EPS);
    let dt = th - tc;
    let mu0 = sutherland().at(T0);
    let rho0 = P0 / (R * T0);
    // Ra = Pr g rho0^2 dT L^3 / (T0 mu0^2)  (Le Quere et al. 2005, eq. 1)
    let l = (RA * T0 * mu0 * mu0 / (PR * G * rho0 * rho0 * dt)).cbrt();
    let k0 = mu0 * cp / PR;
    let g1 = Grading::Symmetric(grading);
    let mesh = Arc::new(box_2d_graded(n, n, l, l, [g1, g1], false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut t = ScalarField::uniform("T", &geom, T0);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(th))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(tc))).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let gas = IdealGas { r: R, cp };
    let (mu, k) = if variable_properties {
        (
            sutherland(),
            Property::Sutherland {
                reference: 1.68e-5 * cp / PR,
                t_ref: 273.0,
                s: 110.5,
            },
        )
    } else {
        (Property::Constant(mu0), Property::Constant(k0))
    };
    let mut s = VdSettings::new(gas, mu, k, Coupling::Simplec);
    s.mode = DensityMode::LowMach;
    s.gravity = [0.0, -G, 0.0];
    s.reference_pressure = P0;
    s.p_ref = (0, 0.0);
    s.alpha_u = 0.8;
    s.alpha_h = 0.9;
    let mut sol = VarDensitySolver::new(geom.clone(), u, p, t, s).unwrap();
    let mut residual = f64::INFINITY;
    let mut it = 0;
    while it < max_iter {
        let info = sol.iterate();
        it += 1;
        residual = info.max_du.max(1e3 * info.res_h);
        if it % 500 == 0 {
            println!(
                "iter {it}: res u {:.2e} p {:.2e} T {:.2e} P_th/P0 {:.6}",
                info.res_u,
                info.res_p,
                info.res_h,
                sol.p_th / P0
            );
        }
        if residual < tol {
            // velocity change below tol (m/s)
            break;
        }
    }
    let depth = l / n as f64;
    let q_hot = -sol.heat_flow_out("xmin").unwrap();
    let q_cold = sol.heat_flow_out("xmax").unwrap();
    // Nu = L / (k0 dT) * mean(k dT/dx) = Q / (k0 dT depth)
    let norm = k0 * dt * depth;
    Outcome {
        nu_hot: q_hot / norm,
        nu_cold: q_cold / norm,
        p_ratio: sol.p_th / P0,
        q_hot,
        q_cold,
        iterations: it,
        residual,
    }
}

fn check(o: &Outcome, nu_ref: f64, p_ref: f64) {
    println!(
        "iterations {}, residual {:.2e}: Nu hot {:.5} cold {:.5} (ref {nu_ref}), P/P0 {:.6} (ref {p_ref}), heat flow imbalance {:.2e}",
        o.iterations,
        o.residual,
        o.nu_hot,
        o.nu_cold,
        o.p_ratio,
        (o.q_hot - o.q_cold).abs() / o.q_hot.abs()
    );
    assert!((o.nu_hot / nu_ref - 1.0).abs() < 0.01, "hot-wall Nu {}", o.nu_hot);
    assert!((o.nu_cold / nu_ref - 1.0).abs() < 0.01, "cold-wall Nu {}", o.nu_cold);
    assert!((o.p_ratio / p_ref - 1.0).abs() < 0.003, "P/P0 {}", o.p_ratio);
    assert!((o.q_hot - o.q_cold).abs() / o.q_hot.abs() < 1e-6, "heat flow imbalance");
}

#[test]
#[ignore = "heavy: 96 x 96 cavity, cargo test --release -- --ignored varden"]
fn c_t1_constant_properties_ra_1e6_eps_0_6() {
    let o = solve(false, 96, 8.0, 4000, 0.0);
    check(&o, 8.85978, 0.856338);
}

#[test]
#[ignore = "heavy: 96 x 96 cavity, cargo test --release -- --ignored varden"]
fn c_t2_sutherland_ra_1e6_eps_0_6() {
    let o = solve(true, 96, 8.0, 4000, 0.0);
    check(&o, 8.6866, 0.924487);
}
