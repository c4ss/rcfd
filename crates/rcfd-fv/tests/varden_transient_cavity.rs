//! ADR 0009 check T4: time-transient approach of the differentially heated cavity to the steady result validated in
//! ADR 0005 (c) T1 (Le Quere et al. 2005, Table 1: Nu 8.85978, P/P0 0.856338; see `varden_cavity.rs`). Started from
//! rest at uniform T0, BDF2, dt = 0.02 s, run to t = 20 s, inner tolerance 1e-6, at most 20 inner iterations.

use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::simple::Coupling;
use rcfd_fv::varden::{DensityMode, IdealGas, Property, TransientSettings, VarDensitySolver, VdSettings};
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

struct Nu {
    hot: f64,
    cold: f64,
    p_ratio: f64,
    imbalance: f64,
}

#[test]
#[ignore = "heavy: 96 x 96 cavity, 1000 BDF2 steps, cargo test --release -- --ignored varden_transient_cavity"]
fn t4_cavity_transient_approaches_steady_le_quere() {
    let cp = GAMMA * R / (GAMMA - 1.0);
    let th = T0 * (1.0 + EPS);
    let tc = T0 * (1.0 - EPS);
    let dt_wall = th - tc;
    let mu0 = sutherland().at(T0);
    let rho0 = P0 / (R * T0);
    let l = (RA * T0 * mu0 * mu0 / (PR * G * rho0 * rho0 * dt_wall)).cbrt();
    let k0 = mu0 * cp / PR;
    let n = 96;
    let g1 = Grading::Symmetric(8.0);
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
    let mut s = VdSettings::new(
        IdealGas { r: R, cp },
        Property::Constant(mu0),
        Property::Constant(k0),
        Coupling::Simplec,
    );
    s.mode = DensityMode::LowMach;
    s.gravity = [0.0, -G, 0.0];
    s.reference_pressure = P0;
    s.p_ref = (0, 0.0);
    s.alpha_u = 0.8;
    s.alpha_h = 0.9;
    s.transient = Some(TransientSettings {
        scheme: TimeScheme::Bdf2,
        max_inner: 20,
        tol: 1e-6,
    });
    let mut sol = VarDensitySolver::new(geom.clone(), u, p, t, s).unwrap();
    let depth = l / n as f64;
    let norm = k0 * dt_wall * depth;
    let measure = |sol: &VarDensitySolver| {
        let hot = -sol.heat_flow_out("xmin").unwrap();
        let cold = sol.heat_flow_out("xmax").unwrap();
        Nu {
            hot: hot / norm,
            cold: cold / norm,
            p_ratio: sol.p_th / P0,
            imbalance: (hot - cold).abs() / hot.abs(),
        }
    };
    let (mut at15, mut unconverged) = (0.0, 0usize);
    for k in 1..=1000 {
        let info = sol.advance(0.02).unwrap();
        if !info.converged {
            unconverged += 1;
        }
        if k % 50 == 0 {
            let m = measure(&sol);
            println!(
                "t = {:.2}: inner {} change {:.2e} Nu hot {:.5} cold {:.5} P/P0 {:.6} imbalance {:.2e}",
                sol.time, info.inner, info.change, m.hot, m.cold, m.p_ratio, m.imbalance
            );
        }
        if k == 750 {
            at15 = measure(&sol).hot;
        }
    }
    let m = measure(&sol);
    println!(
        "T4 at 20 s: Nu hot {:.5} cold {:.5} (ref 8.85978), P/P0 {:.6} (ref 0.856338), imbalance {:.2e}, |dNu(15->20)|/Nu {:.2e}, steps not converged {unconverged}",
        m.hot,
        m.cold,
        m.p_ratio,
        m.imbalance,
        (m.hot - at15).abs() / m.hot
    );
    assert!((m.hot / 8.85978 - 1.0).abs() <= 0.01, "hot-wall Nu {}", m.hot);
    assert!((m.cold / 8.85978 - 1.0).abs() <= 0.01, "cold-wall Nu {}", m.cold);
    assert!((m.p_ratio / 0.856338 - 1.0).abs() <= 0.003, "P/P0 {}", m.p_ratio);
    assert!(m.imbalance <= 2e-3, "heat flow imbalance {}", m.imbalance);
    assert!((m.hot - at15).abs() / m.hot <= 1e-3, "not stationary: Nu(15 s) {at15}");
}
