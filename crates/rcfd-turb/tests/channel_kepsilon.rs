//! F-042: k-epsilon models with wall functions on the periodic channel at Re_tau = 395.
//!
//! DNS reference: Moser, Kim and Mansour (1999), Phys. Fluids 11, 943, Re_tau = 395: bulk Reynolds
//! number Re_b = 2 h U_b / nu = 13750 (U_b+ = 17.4) and centre-line velocity U_c+ = 20.1. Checked
//! against the published MKM profile file chan395.means (actual Re_tau = 392.24): trapezoid
//! integration gives U_c+ = 20.13, U_b+ = 17.54 and Re_b = 13764 (so U_b+ = 17.4 at the nominal
//! Re_tau = 395 used here). The log-layer comparison (20 < y+ < 150) uses Reichardt's (1951) law of
//! the wall, which agrees with the MKM DNS to within 2.3 percent there. See docs/reference-data.md.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_turb::{KEpsilon, KEpsilonVariant};
use std::sync::Arc;

const RE_TAU: f64 = 395.0;

struct Result {
    yplus: Vec<f64>,
    uplus: Vec<f64>,
    centre: f64,
    bulk: f64,
    iterations: usize,
    change: f64,
}

fn run(variant: KEpsilonVariant, ny: usize) -> Result {
    let nu = 1.0 / RE_TAU;
    let mesh = box_2d_graded(4, ny, 1.0, 2.0, [Grading::Uniform, Grading::Uniform], true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [17.0, 0.0, 0.0]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    s.h_from_solution = true;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, s);
    flow.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0]));
    let mut model = KEpsilon::new(variant, geom.clone(), nu, &["ymin", "ymax"], 1.0, 0.36).unwrap();
    let (mut iterations, mut change) = (0, f64::INFINITY);
    let mut last_u = f64::INFINITY;
    for it in 0..20000 {
        change = model.update(&flow.u, &flow.phi);
        flow.nu_face = Some(model.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        last_u = info.max_du;
        if it > 10 && last_u < 1e-9 && change < 1e-8 {
            break;
        }
    }
    let _ = last_u;
    let cc = geom.mesh.cell_centres();
    let mut pts: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| cc[c][0] < 0.3 && cc[c][1] < 1.0)
        .map(|c| (cc[c][1] * RE_TAU, flow.u.values[c][0]))
        .collect();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let centre_cells: Vec<f64> = (0..cc.len())
        .filter(|&c| cc[c][0] < 0.3)
        .filter(|&c| (cc[c][1] - 1.0).abs() < 1.0 / ny as f64)
        .map(|c| flow.u.values[c][0])
        .collect();
    // linear extrapolation of the two cells adjacent to the centre line to y = h
    let nn = pts.len();
    let (y1, u1) = (pts[nn - 1].0 / RE_TAU, pts[nn - 1].1);
    let (y0, u0) = (pts[nn - 2].0 / RE_TAU, pts[nn - 2].1);
    let centre = u1 + (u1 - u0) / (y1 - y0) * (1.0 - y1);
    let _ = centre_cells;
    let bulk = flow.u.values.iter().map(|v| v[0]).sum::<f64>() / flow.u.values.len() as f64;
    Result {
        yplus: pts.iter().map(|p| p.0).collect(),
        uplus: pts.iter().map(|p| p.1).collect(),
        centre,
        bulk,
        iterations,
        change,
    }
}

fn reichardt(yp: f64) -> f64 {
    (1.0 + 0.41 * yp).ln() / 0.41 + 7.8 * (1.0 - (-yp / 11.0).exp() - yp / 11.0 * (-yp / 3.0).exp())
}

fn check(variant: KEpsilonVariant, ny: usize) {
    let r = run(variant, ny);
    println!(
        "{variant:?} ny {ny}: {} its (change {:.1e}), U_c+ = {:.3} (DNS 20.1, {:+.1}%), U_b+ = {:.3} (DNS 17.4, {:+.1}%)",
        r.iterations,
        r.change,
        r.centre,
        100.0 * (r.centre / 20.1 - 1.0),
        r.bulk,
        100.0 * (r.bulk / 17.4 - 1.0)
    );
    for (y, u) in r.yplus.iter().zip(&r.uplus) {
        println!("   y+ {y:7.1}  U+ {u:7.3}  Reichardt {:7.3}", reichardt(*y));
    }
    assert!(r.change < 1e-6, "not converged: {}", r.change);
    assert!((r.centre / 20.1 - 1.0).abs() < 0.10, "centre line {}", r.centre);
    assert!((r.bulk / 17.4 - 1.0).abs() < 0.10, "bulk {}", r.bulk);
    for (y, u) in r.yplus.iter().zip(&r.uplus) {
        if *y > 20.0 && *y < 150.0 {
            assert!((u / reichardt(*y) - 1.0).abs() < 0.10, "y+ {y}: {u}");
        }
    }
}

#[test]
fn standard_k_epsilon_channel() {
    check(KEpsilonVariant::Standard, 14);
}

#[test]
fn rng_k_epsilon_channel() {
    check(KEpsilonVariant::Rng, 14);
}

#[test]
fn realizable_k_epsilon_channel() {
    check(KEpsilonVariant::Realizable, 14);
}
