//! Wave 34D (ADR 0075 amendment 2, falsifier G1): Galilean invariance of the k-omega SST wall treatment with moving walls.
//!
//! The periodic channel of ADR 0070 (4 cells in x, body force 1, u_tau = 1, Re_tau = 395, both walls) is solved (A) with both walls at
//! rest and (B) with both walls moving at `-U_f` and the initial field shifted by `-U_f`. In a fully developed x-uniform periodic channel
//! the discrete convection cancels in any frame, so `u_B + U_f = u_A` must hold. Registered limits: `max |u_B + U_f - u_A| <= 1e-6 U_cl,A`
//! and `max |k_B - k_A| <= 1e-6 max k_A`, on the first-cell y+ = 1 and y+ = 30 meshes.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_turb::sst::SstWallMode;
use rcfd_turb::KOmegaSst;
use std::sync::Arc;

const RE_TAU: f64 = 395.0;
const U_F: f64 = 10.0;

/// (u_x of every cell, k of every cell, iterations, converged) of the channel with both walls moving at `wall` (x velocity).
fn channel(y_plus_centre: f64, ny: usize, layers: usize, wall: f64) -> (Vec<f64>, Vec<f64>, usize, bool) {
    let nu = 1.0 / RE_TAU;
    let first = 2.0 * y_plus_centre / RE_TAU;
    let grading = Grading::WallLayers {
        first,
        growth: 1.1,
        layers,
        both: true,
    };
    let mesh = box_2d_graded(4, ny, 1.0, 2.0, [Grading::Uniform, grading], true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [20.0 + wall, 0.0, 0.0]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([wall, 0.0, 0.0])))
            .unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    s.h_from_solution = true;
    let mut flow = SimpleSolver::new(geom.clone(), u, p, s);
    flow.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0]));
    let mut model = KOmegaSst::new(geom.clone(), nu, &["ymin", "ymax"], 1.0, 1.0 / (10.0 * nu)).unwrap();
    model.set_wall_mode(SstWallMode::Automatic).unwrap();
    let (mut iterations, mut converged) = (0, false);
    for it in 0..60000 {
        let change = model.update(&flow.u, &flow.phi);
        flow.nu_face = Some(model.nu_face());
        let info = flow.iterate();
        iterations = it + 1;
        if it > 10 && info.max_du < 1e-11 && change < 1e-10 {
            converged = true;
            break;
        }
    }
    (
        flow.u.values.iter().map(|v| v[0]).collect(),
        model.k.values.clone(),
        iterations,
        converged,
    )
}

/// (max |u_B + U_f - u_A| / U_cl,A, max |k_B - k_A| / max k_A, both converged).
fn compare(y_plus_centre: f64, ny: usize, layers: usize) -> (f64, f64, bool) {
    let (ua, ka, ia, ca) = channel(y_plus_centre, ny, layers, 0.0);
    let (ub, kb, ib, cb) = channel(y_plus_centre, ny, layers, -U_F);
    let ucl = ua.iter().cloned().fold(0.0, f64::max);
    let kmax = ka.iter().cloned().fold(0.0, f64::max);
    let du = ua.iter().zip(&ub).map(|(a, b)| (b + U_F - a).abs()).fold(0.0, f64::max) / ucl;
    let dk = ka.iter().zip(&kb).map(|(a, b)| (b - a).abs()).fold(0.0, f64::max) / kmax;
    println!(
        "G1 y+ {y_plus_centre}: A {ia} iterations (converged {ca}), B {ib} iterations (converged {cb}), U_cl {ucl:.4}, max |u_B + U_f - u_A| / U_cl {du:.3e}, max |k_B - k_A| / max k {dk:.3e}"
    );
    (du, dk, ca && cb)
}

#[test]
fn g1_sst_wall_treatment_is_galilean_invariant_first_cell_y_plus_1() {
    let (du, dk, conv) = compare(1.0, 60, 20);
    assert!(conv, "not converged");
    assert!(du <= 1e-6 && dk <= 1e-6, "du {du:.3e}, dk {dk:.3e}");
}

#[test]
fn g1_sst_wall_treatment_is_galilean_invariant_first_cell_y_plus_30() {
    let (du, dk, conv) = compare(30.0, 12, 3);
    assert!(conv, "not converged");
    assert!(du <= 1e-6 && dk <= 1e-6, "du {du:.3e}, dk {dk:.3e}");
}
