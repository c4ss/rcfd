//! Coarse-level start of the laminar backward-facing step (F-205, ADR 0094 check C1). The fine set-up is that of ADR 0019
//! I2 (`rcfd-fv/tests/init_potential.rs`). The coarse start solves the same case on the mesh with every cell count halved
//! (uniform (1,0,0) start, stop at `max(res_u, res_p) < 1e-6` or 300 iterations), interpolates velocity and pressure with
//! `rcfd_post::interpolate` (gradient-corrected nearest cell, the transfer `initial.method = "coarse"` uses) and starts
//! the fine run from them. Registered limit: fine iterations of the coarse start `<` those of the uniform (1,0,0) start
//! and `<` those of the zero start, all three converged. Coarse iterations are not counted (as in I3); the equivalent work
//! with a coarse iteration at 1/4 of a fine one is reported only.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{backward_step_graded, StepGrading, StepSpec};
use rcfd_post::interpolate::{InterpMode, Interpolator};
use std::sync::Arc;

const TOL: f64 = 1e-6;

fn spec(f: usize) -> StepSpec {
    StepSpec {
        l_in: 2.0,
        l_out: 20.0,
        h_in: 1.0,
        step: 1.0,
        depth: 0.1,
        n: [8 / f, 80 / f, 8 / f, 8 / f],
    }
}

/// The I2 solver on the step mesh with cell counts divided by `f`, interior velocity `u0`.
fn solver(f: usize, u0: [f64; 3]) -> SimpleSolver {
    let spec = spec(f);
    let geom =
        Arc::new(FvGeometry::new(Arc::new(backward_step_graded(spec, StepGrading::uniform()).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, u0);
    let patch = geom.mesh.patch("inlet").unwrap().clone();
    let cf = geom.mesh.face_centres();
    let prof: Vec<[f64; 3]> = patch
        .faces()
        .map(|f| {
            let s = (cf[f][1] - spec.step) / spec.h_in;
            [6.0 * s * (1.0 - s), 0.0, 0.0]
        })
        .collect();
    u.set_bc(&geom, "inlet", Box::new(FixedValue(Profile::PerFace(prof))))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(spec.step / 100.0, Coupling::Simple);
    st.convection = ConvScheme::LinearUpwind;
    st.h_from_solution = true;
    SimpleSolver::new(geom, u, p, st)
}

/// Iterate until `max(res_u, res_p) < TOL` or `cap`; (iterations, converged).
fn iterate(s: &mut SimpleSolver, cap: usize) -> (usize, bool) {
    for i in 1..=cap {
        let info = s.iterate();
        let res = info.res_u.max(info.res_p);
        if !res.is_finite() {
            return (i, false);
        }
        if res < TOL {
            return (i, true);
        }
    }
    (cap, false)
}

/// Fine solver started from the coarse solution after `coarse_cap` coarse iterations; also returns the coarse count.
fn coarse_started(coarse_cap: usize) -> (SimpleSolver, usize, bool) {
    let mut c = solver(2, [1.0, 0.0, 0.0]);
    let (nc, ok) = iterate(&mut c, coarse_cap);
    let mut fine = solver(1, [1.0, 0.0, 0.0]);
    let ip = Interpolator::new(&c.geom.mesh, &fine.geom.mesh).unwrap();
    let u = ip.vector(&c.u.values, InterpMode::NonConservative).unwrap();
    let p = ip.scalar(&c.p.values, InterpMode::NonConservative).unwrap();
    fine.set_fields(u, p).unwrap();
    (fine, nc, ok)
}

#[test]
#[ignore = "ADR 0094 set-up smoke, run once before the limits were registered"]
fn c1_coarse_start_setup_smoke() {
    let (mut fine, nc, _) = coarse_started(5);
    let finite = fine.u.values.iter().flatten().all(|x| x.is_finite()) && fine.p.values.iter().all(|x| x.is_finite());
    let info = fine.iterate();
    println!(
        "C1 smoke: coarse cells {}, fine cells {}, {nc} coarse iterations, transferred fields finite {finite}, first fine res_u {:.3e} res_p {:.3e}",
        solver(2, [0.0; 3]).geom.n_cells(),
        fine.geom.n_cells(),
        info.res_u,
        info.res_p
    );
    assert!(finite && info.res_u.is_finite() && info.res_p.is_finite());
}

#[test]
fn c1_coarse_start_needs_fewer_iterations_on_the_backward_facing_step() {
    let cap = 3000;
    let (a, a_ok) = iterate(&mut solver(1, [1.0, 0.0, 0.0]), cap);
    let (b, b_ok) = iterate(&mut solver(1, [0.0; 3]), cap);
    let (mut fine, nc, c_conv) = coarse_started(300);
    let (c, c_ok) = iterate(&mut fine, cap);
    println!(
        "C1 iterations: uniform (1,0,0) {a} ({a_ok}), zero {b} ({b_ok}), coarse start {c} ({c_ok}) after {nc} coarse \
         iterations (coarse converged {c_conv}); equivalent work with a coarse iteration at 1/4: {:.0} (reported only)",
        c as f64 + nc as f64 / 4.0
    );
    assert!(a_ok && b_ok && c_ok, "all registered runs must converge");
    assert!(c < a, "coarse start {c} vs uniform (1,0,0) {a}");
    assert!(c < b, "coarse start {c} vs zero {b}");
}
