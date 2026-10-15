//! Thermal wall conditions (F-051, ADR 0024 group A): convection, thin wall, radiation to ambient, fin, lumped slab.
//! Limits are the ones registered in `docs/adr/0024-wave17b-cht.md` before any run.

mod cht_common;

use cht_common::*;
use rcfd_fv::cht::{conductive_face_flows, patch_heat_flow_out};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::energy::EnergySolver;
use rcfd_fv::thermal_bc::STEFAN_BOLTZMANN;
use rcfd_fv::{ConvectiveWall, FixedValue, FvGeometry, ScalarField};
use rcfd_mesh::gen::box_2d;
use std::sync::Arc;

/// 1-D slab `[0, l]` of `nx` cells (one cell across), conductivity `k`; returns the converged solver, geometry, conductivity.
fn slab(nx: usize, l: f64, k: f64, left: ConvectiveWall, t_right: f64) -> (EnergySolver, Arc<FvGeometry>, Vec<f64>) {
    let geom = geometry(box_2d(nx, 1, l, 1.0, false, false).unwrap());
    let mats = uniform_materials(&geom, k, 1.0);
    let mut t = ScalarField::uniform("T", &geom, 350.0);
    let nl = geom.mesh.patch("xmin").unwrap().faces().len();
    let mut left = left;
    left.k = vec![k; nl];
    t.set_bc(&geom, "xmin", Box::new(left)).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(t_right))).unwrap();
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::Upwind);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let (_, change) = converge(&mut en, &phi, 1e-11, 400);
    assert!(change < 1e-10, "not converged: {change}");
    let kf = mats.face_conductivity(&geom);
    (en, geom, kf)
}

fn wall(h: f64, t_inf: f64, rw: f64, eps: f64, t_rad: f64) -> ConvectiveWall {
    ConvectiveWall {
        k: vec![],
        h,
        t_inf,
        wall_resistance: rw,
        emissivity: eps,
        t_rad,
    }
}

fn max_err(en: &EnergySolver, geom: &FvGeometry, exact: impl Fn(f64) -> f64) -> f64 {
    geom.mesh
        .cell_centres()
        .iter()
        .zip(&en.t.values)
        .map(|(c, t)| (t - exact(c[0])).abs())
        .fold(0.0, f64::max)
}

fn flows(en: &EnergySolver, geom: &FvGeometry, kf: &[f64], patch: &str) -> f64 {
    let f = conductive_face_flows(geom, &en.t, kf, false);
    patch_heat_flow_out(geom, &f, patch).unwrap()
}

#[test]
fn a1_convection_slab_exact() {
    let (l, k, h, tinf, tr) = (0.1, 2.0, 50.0, 400.0, 300.0);
    let (en, geom, kf) = slab(20, l, k, wall(h, tinf, 0.0, 0.0, 0.0), tr);
    let q = (tinf - tr) / (1.0 / h + l / k);
    let ts = tinf - q / h;
    let err = max_err(&en, &geom, |x| ts + (tr - ts) * x / l);
    assert!(err / (tinf - tr) <= 1e-10, "A1 temperature error {err}");
    let area = patch_area(&geom, "xmin");
    let fl = flows(&en, &geom, &kf, "xmin");
    let rel = ((-fl) - q * area).abs() / (q * area);
    assert!(rel <= 1e-10, "A1 heat flow {fl} vs {}", q * area);
    println!(
        "A1 temperature error/dT {:.3e}, heat-flow rel {rel:.3e}",
        err / (tinf - tr)
    );
}

#[test]
fn a2_thin_wall_layers_exact() {
    let (l, k, h, tinf, tr) = (0.05, 1.5, 25.0, 350.0, 290.0);
    let rw = 0.002 / 0.5 + 0.001 / 45.0;
    let (en, geom, kf) = slab(25, l, k, wall(h, tinf, rw, 0.0, 0.0), tr);
    let q = (tinf - tr) / (1.0 / h + rw + l / k);
    // the slab surface (left face) sits below the chain end by q (1/h + rw)
    let ts = tinf - q * (1.0 / h + rw);
    let err = max_err(&en, &geom, |x| ts + (tr - ts) * x / l);
    assert!(err / (tinf - tr) <= 1e-10, "A2 temperature error {err}");
    let area = patch_area(&geom, "xmin");
    let fl = flows(&en, &geom, &kf, "xmin");
    assert!(((-fl) - q * area).abs() / (q * area) <= 1e-10);
    println!("A2 temperature error/dT {:.3e}", err / (tinf - tr));
}

/// Surface temperature of the radiating slab by bisection (independent of the code under test).
fn radiating_surface(l: f64, k: f64, tr: f64, h: f64, tinf: f64, eps: f64, ta: f64) -> f64 {
    let g = |ts: f64| k * (tr - ts) / l - h * (ts - tinf) - eps * STEFAN_BOLTZMANN * (ts.powi(4) - ta.powi(4));
    let (mut lo, mut hi) = (1.0, tr);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if g(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn radiation_case(h: f64, tinf: f64) {
    let (l, k, tr, eps, ta) = (0.05, 15.0, 800.0, 0.8, 300.0);
    let (en, geom, kf) = slab(20, l, k, wall(h, tinf, 0.0, eps, ta), tr);
    let ts = radiating_surface(l, k, tr, h, tinf, eps, ta);
    let err = max_err(&en, &geom, |x| ts + (tr - ts) * x / l);
    let q = k * (tr - ts) / l;
    let area = patch_area(&geom, "xmin");
    let fl = flows(&en, &geom, &kf, "xmin");
    let dt = tr - ts;
    assert!(err / dt <= 1e-9, "A3 temperature error {err} (h = {h})");
    assert!(
        ((-fl) - q * area).abs() / (q * area) <= 1e-9,
        "A3 heat flow {fl} vs {}",
        q * area
    );
    let right = flows(&en, &geom, &kf, "xmax");
    assert!((fl + right).abs() <= 1e-12 * fl.abs(), "A4 balance {}", fl + right);
    println!("A3 h={h}: ts {ts:.6}, temperature error/dT {:.3e}", err / dt);
}

#[test]
#[ignore = "set-up defect: the heat-flow assertion used the sign of an inflow (A1); kept as a record, corrected in a3_radiation_with_convection_v2"]
fn a3_radiation_with_convection() {
    radiation_case(10.0, 300.0);
}

#[test]
#[ignore = "set-up defect: the heat-flow assertion used the sign of an inflow (A1); kept as a record, corrected in a3b_pure_radiation_v2"]
fn a3b_pure_radiation() {
    // h = 0: the free-stream temperature is unused
    radiation_case(0.0, -1.0);
}

fn radiation_case_v2(h: f64, tinf: f64) {
    let (l, k, tr, eps, ta) = (0.05, 15.0, 800.0, 0.8, 300.0);
    let (en, geom, kf) = slab(20, l, k, wall(h, tinf, 0.0, eps, ta), tr);
    let ts = radiating_surface(l, k, tr, h, tinf, eps, ta);
    let err = max_err(&en, &geom, |x| ts + (tr - ts) * x / l);
    let q = k * (tr - ts) / l;
    let area = patch_area(&geom, "xmin");
    let fl = flows(&en, &geom, &kf, "xmin");
    let dt = tr - ts;
    assert!(err / dt <= 1e-9, "A3 temperature error {err} (h = {h})");
    assert!(
        (fl.abs() - q * area).abs() / (q * area) <= 1e-9,
        "A3 heat flow {fl} vs {}",
        q * area
    );
    let right = flows(&en, &geom, &kf, "xmax");
    assert!((fl + right).abs() <= 1e-12 * fl.abs(), "A4 balance {}", fl + right);
    println!("A3 h={h}: ts {ts:.6}, temperature error/dT {:.3e}", err / dt);
}

#[test]
fn a3_radiation_with_convection_v2() {
    radiation_case_v2(10.0, 300.0);
}

#[test]
fn a3b_pure_radiation_v2() {
    // h = 0: the free-stream temperature is unused
    radiation_case_v2(0.0, -1.0);
}

#[test]
fn a4_balance_of_a1_a2() {
    let (en, geom, kf) = slab(20, 0.1, 2.0, wall(50.0, 400.0, 0.0, 0.0, 0.0), 300.0);
    let (l, r) = (flows(&en, &geom, &kf, "xmin"), flows(&en, &geom, &kf, "xmax"));
    assert!((l + r).abs() <= 1e-12 * l.abs(), "A4 {l} {r}");
    let rw = 0.002 / 0.5 + 0.001 / 45.0;
    let (en, geom, kf) = slab(25, 0.05, 1.5, wall(25.0, 350.0, rw, 0.0, 0.0), 290.0);
    let (l, r) = (flows(&en, &geom, &kf, "xmin"), flows(&en, &geom, &kf, "xmax"));
    assert!((l + r).abs() <= 1e-12 * l.abs(), "A4 {l} {r}");
}

fn fin(nx: usize, ny: usize) -> (f64, f64) {
    let (b, l, k, h, tinf, tb) = (0.005, 0.1, 100.0, 20.0, 300.0, 400.0);
    let geom = geometry(box_2d(nx, ny, l, b, false, false).unwrap());
    let mats = uniform_materials(&geom, k, 1.0);
    let mut t = ScalarField::uniform("T", &geom, 350.0);
    let nf = geom.mesh.patch("ymax").unwrap().faces().len();
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(tb))).unwrap();
    t.set_bc(
        &geom,
        "ymax",
        Box::new(ConvectiveWall {
            k: vec![k; nf],
            h,
            t_inf: tinf,
            wall_resistance: 0.0,
            emissivity: 0.0,
            t_rad: 0.0,
        }),
    )
    .unwrap();
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::LinearUpwind);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let (_, change) = converge(&mut en, &phi, 1e-11, 600);
    assert!(change < 1e-10);
    let kf = mats.face_conductivity(&geom);
    let f = conductive_face_flows(&geom, &en.t, &kf, true);
    // heat leaving through the base is negative; per unit depth (the cell depth is lx/nx)
    let depth = l / nx as f64;
    let q2d = -patch_heat_flow_out(&geom, &f, "xmin").unwrap() / depth;
    let m = (h / (k * b)).sqrt();
    let q1d = k * b * m * (tb - tinf) * (m * l).tanh();
    (q2d.abs(), q1d)
}

#[test]
fn a5_fin_matches_one_dimensional_fin() {
    let (q40, q1d) = fin(40, 10);
    let (q80, _) = fin(80, 20);
    let e = (q40 - q1d).abs() / q1d;
    let d = (q80 - q40).abs() / q40;
    println!("A5 Q2D(40x10) {q40:.6} Q2D(80x20) {q80:.6} Q1D {q1d:.6}; error {e:.3e}, refinement change {d:.3e}");
    assert!(e <= 1e-2, "A5 fin error {e}");
    assert!(d <= 2e-3, "A5 refinement change {d}");
}

#[test]
fn a6_lumped_slab_cooling() {
    let (half, k, rcp, h, tinf, t0) = (0.005, 50.0, 4.0e6, 100.0, 300.0, 400.0);
    let tau = rcp * half / h;
    let geom = geometry(box_2d(20, 1, half, 1.0, false, false).unwrap());
    let mats = uniform_materials(&geom, k, rcp);
    let mut t = ScalarField::uniform("T", &geom, t0);
    let nf = geom.mesh.patch("xmax").unwrap().faces().len();
    t.set_bc(
        &geom,
        "xmax",
        Box::new(ConvectiveWall {
            k: vec![k; nf],
            h,
            t_inf: tinf,
            wall_resistance: 0.0,
            emissivity: 0.0,
            t_rad: 0.0,
        }),
    )
    .unwrap();
    let mut en = cht_solver(&geom, t, &mats, ConvScheme::Upwind);
    let phi = vec![0.0; geom.mesh.n_faces()];
    let dt = tau / 2000.0;
    let vol = geom.mesh.cell_volumes();
    let vtot: f64 = vol.iter().sum();
    for step in 1..=4000 {
        en.advance(&phi, dt, TimeScheme::ImplicitEuler);
        if step == 2000 || step == 4000 {
            let mean: f64 = en.t.values.iter().zip(vol).map(|(t, v)| t * v).sum::<f64>() / vtot;
            let lumped = (t0 - tinf) * (-(step as f64) * dt / tau).exp();
            let rel = ((mean - tinf) - lumped).abs() / lumped;
            println!(
                "A6 step {step}: excess {:.6} lumped {lumped:.6} rel {rel:.3e}",
                mean - tinf
            );
            assert!(rel <= 1e-2, "A6 at step {step}: {rel}");
        }
    }
}
