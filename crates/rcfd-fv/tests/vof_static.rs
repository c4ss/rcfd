//! ADR 0049 checks B1-B2 (hydrostatic equilibrium of a two-layer pool in the p_rgh formulation). The limits are those
//! registered in the ADR before any run; none was changed afterwards.

use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

const G: f64 = 9.81;

fn pool(water_top: f64, sigma: f64) -> VofSolver {
    let mesh = Arc::new(box_2d(10, 20, 0.5, 1.0, false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 0.0);
    alpha.values = indicator_fraction(&geom, 10, |x| x[1] <= water_top);
    let mut s = VofSettings::new([1000.0, 1.0], [1e-3, 1.8e-5]);
    s.gravity = [0.0, -G, 0.0];
    s.sigma = sigma;
    s.n_correctors = 3;
    VofSolver::new(geom, u, p_rgh, alpha, s).unwrap()
}

fn max_u(s: &VofSolver) -> f64 {
    s.u.values
        .iter()
        .map(|v| v[0].abs().max(v[1].abs()))
        .fold(0.0, f64::max)
}

#[test]
fn b1_interface_on_a_face() {
    let mut s = pool(0.5, 0.0);
    let a0 = s.transport.alpha.values.clone();
    let mut worst_u: f64 = 0.0;
    for _ in 0..500 {
        s.step(1e-3).unwrap();
        worst_u = worst_u.max(max_u(&s));
    }
    // layered hydrostatic pressure, differences to cell 0
    let cc = s.geom.mesh.cell_centres();
    let pe = |y: f64| {
        if y > 0.5 {
            1.0 * G * (1.0 - y)
        } else {
            1.0 * G * 0.5 + 1000.0 * G * (0.5 - y)
        }
    };
    let scale = 1000.0 * G * 0.5;
    let perr = (0..cc.len())
        .map(|c| ((s.p[c] - s.p[0]) - (pe(cc[c][1]) - pe(cc[0][1]))).abs() / scale)
        .fold(0.0, f64::max);
    let da = (0..a0.len())
        .map(|c| (s.transport.alpha.values[c] - a0[c]).abs())
        .fold(0.0, f64::max);
    println!("B1 max|U| {worst_u:.3e} (limit 1e-10) pressure error {perr:.3e} (limit 1e-10) alpha change {da:.3e} (limit 1e-12)");
    assert!(worst_u <= 1e-10, "velocity {worst_u}");
    assert!(perr <= 1e-10, "pressure {perr}");
    assert!(da <= 1e-12, "alpha {da}");
}

#[test]
fn b2_interface_inside_a_cell() {
    for sigma in [0.0, 0.07] {
        let mut s = pool(0.5 + 0.4 * 0.05, sigma);
        let mut worst_u: f64 = 0.0;
        for _ in 0..500 {
            s.step(1e-3).unwrap();
            worst_u = worst_u.max(max_u(&s));
        }
        println!("B2 sigma {sigma}: max|U| {worst_u:.3e} (limit 1e-10)");
        assert!(worst_u <= 1e-10, "velocity {worst_u}");
    }
}
