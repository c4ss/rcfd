//! ADR 0108 part 2: `VofSettings::height_function_csf_fallback` keeps the CSF curvature on meshes the height function refuses
//! and skips the lattice without surface tension. F1-F4 registered in ADR 0108 (exact equality, no tolerance).

use rcfd_fv::vof::{indicator_fraction, VofSettings, VofSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d, warp};
use std::sync::Arc;

/// Settings: Hysing fluids, gravity, `sigma`, and the two height-function flags.
fn settings(sigma: f64, hf: bool, fallback: bool) -> VofSettings {
    let mut s = VofSettings::new([1000.0, 100.0], [10.0, 1.0]);
    s.gravity = [0.0, -0.98, 0.0];
    s.sigma = sigma;
    s.height_function = hf;
    s.height_function_csf_fallback = fallback;
    s
}

/// A bubble of radius 0.25 at (0.5, 0.55) on a 20 x 20 grid of the unit square, warped (trapezoid, unequal cell volumes) or not.
fn solver(warped: bool, st: VofSettings) -> rcfd_core::Result<VofSolver> {
    let mesh = box_2d(20, 20, 1.0, 1.0, false, false).unwrap();
    let mesh = if warped {
        warp(&mesh, |p| [p[0], p[1] * (1.0 + 0.2 * p[0]), p[2]]).unwrap()
    } else {
        mesh
    };
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p_rgh = ScalarField::uniform("p_rgh", &geom, 0.0);
    let mut alpha = ScalarField::uniform("alpha", &geom, 1.0);
    alpha.values = indicator_fraction(&geom, 10, |x| (x[0] - 0.5).hypot(x[1] - 0.55) > 0.25);
    VofSolver::new(geom, u, p_rgh, alpha, st)
}

/// Velocity, p_rgh and alpha after 3 steps of 1e-3.
fn fields(mut s: VofSolver) -> (Vec<[f64; 3]>, Vec<f64>, Vec<f64>) {
    for _ in 0..3 {
        s.step(1e-3).unwrap();
    }
    (s.u.values, s.p_rgh.values, s.transport.alpha.values.clone())
}

/// F1: warped mesh with surface tension: refused without the fallback; with it, CSF curvature and fields bit for bit.
#[test]
fn f1_warped_mesh_with_tension_falls_back_to_csf() {
    assert!(
        solver(true, settings(24.5, true, false)).is_err(),
        "strict height function must refuse the warped mesh"
    );
    let plain = solver(true, settings(24.5, false, false)).unwrap();
    let fb = solver(true, settings(24.5, true, true)).unwrap();
    assert_eq!(fb.curvature(), plain.curvature());
    assert_eq!(fb.curvature_fallbacks(), [0, 0]);
    assert_eq!(fields(fb), fields(plain));
}

/// F2: warped mesh without surface tension (the cavitating-venturi situation): constructs, fields bit for bit.
#[test]
fn f2_warped_mesh_without_tension_constructs() {
    let plain = solver(true, settings(0.0, false, false)).unwrap();
    let fb = solver(true, settings(0.0, true, true)).unwrap();
    assert_eq!(fields(fb), fields(plain));
}

/// F3: uniform grid with surface tension: the fallback leaves the height-function path unchanged.
#[test]
fn f3_cartesian_grid_keeps_height_function() {
    let hf = solver(false, settings(24.5, true, false)).unwrap();
    let fb = solver(false, settings(24.5, true, true)).unwrap();
    assert_eq!(fb.curvature(), hf.curvature());
    assert_eq!(fb.curvature_fallbacks(), hf.curvature_fallbacks());
    assert_ne!(
        fb.curvature(),
        solver(false, settings(24.5, false, false)).unwrap().curvature()
    );
}

/// F4: uniform grid without surface tension: fields bit for bit with the plain solver.
#[test]
fn f4_cartesian_grid_without_tension() {
    let plain = solver(false, settings(0.0, false, false)).unwrap();
    let fb = solver(false, settings(0.0, true, true)).unwrap();
    assert_eq!(fields(fb), fields(plain));
}
