//! ADR 0081 part N: the steady reports `max_du`, `continuity` and `res_u` keep NaN (they read 0 for a NaN state before), checks N1-N4.
//! Limits are the registered ones.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::coupled::{CoupledSettings, CoupledSolver};
use rcfd_fv::simple::{nan_max, Coupling, SimpleSettings, SimpleSolver, SteadyGuard};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::cavity_2d;
use std::sync::Arc;

fn lid_cavity(n: usize) -> (Arc<FvGeometry>, VectorField, ScalarField) {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(n, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    (geom, u, p)
}

/// The SM-N set-up (the ADR 0077 S2 cavity): 12 x 12, nu 1e-3, SIMPLE, upwind, alpha 1/1.
fn sm_n() -> SimpleSolver {
    let (geom, u, p) = lid_cavity(12);
    let mut st = SimpleSettings::new(1e-3, Coupling::Simple);
    st.convection = ConvScheme::Upwind;
    st.alpha_u = 1.0;
    st.alpha_p = 1.0;
    SimpleSolver::new(geom, u, p, st)
}

fn non_finite(u: &VectorField) -> bool {
    u.values.iter().any(|v| v.iter().any(|x| !x.is_finite()))
}

#[test]
fn nan_max_keeps_nan_and_equals_max_otherwise() {
    assert!(nan_max(f64::NAN, 1.0).is_nan() && nan_max(1.0, f64::NAN).is_nan());
    for (a, b) in [
        (0.0, 1.0),
        (2.0, -1.0),
        (f64::INFINITY, 3.0),
        (-0.0, 0.0),
        (1e-300, 1e300),
    ] {
        assert_eq!(nan_max(a, b).to_bits(), a.max(b).to_bits(), "{a} {b}");
    }
}

/// The registered N1: `res_u` is the residual of the iteration's finite starting state, so asking for NaN there was a defective limit
/// (ADR 0081 amendment 1; it measured 0.971). Kept with its limit; the corrected test is N1'.
#[test]
#[ignore = "set-up defect of ADR 0081 N1 (res_u belongs to the finite starting state); corrected test n1_corrected_*; limit unchanged"]
fn n1_registered_setup_defect() {
    let mut s = sm_n();
    for i in 1..=3000 {
        let info = s.iterate();
        if non_finite(&s.u) {
            println!("N1: fields first non-finite at iteration {i}: {info:?}");
            assert!(
                info.max_du.is_nan() && info.continuity.is_nan() && info.res_u.is_nan(),
                "N1: {info:?}"
            );
            return;
        }
    }
    panic!("N1 set-up: the fields stayed finite for 3000 iterations");
}

/// N1' (amendment 1): NaN `max_du` and `continuity` at the first iteration that ends non-finite; all three NaN at the next one.
#[test]
fn n1_corrected_reports_are_nan_from_the_first_non_finite_state() {
    let mut s = sm_n();
    for i in 1..=3000 {
        let info = s.iterate();
        if non_finite(&s.u) {
            let next = s.iterate();
            println!("N1': fields first non-finite at iteration {i}: {info:?}; next: {next:?}");
            assert!(info.max_du.is_nan() && info.continuity.is_nan(), "N1': {info:?}");
            assert!(
                next.max_du.is_nan() && next.continuity.is_nan() && next.res_u.is_nan(),
                "N1': {next:?}"
            );
            return;
        }
    }
    panic!("N1' set-up: the fields stayed finite for 3000 iterations");
}

#[test]
fn n2_run_does_not_report_a_nan_state_as_converged() {
    let mut s = sm_n();
    let r = s.run(3000, 1e-6);
    println!(
        "N2: converged {} after {} iterations, non-finite fields {}",
        r.converged,
        r.iterations,
        non_finite(&s.u)
    );
    assert!(!r.converged, "N2: {:?}", r.last);
}

#[test]
fn n3_coupled_reports_are_nan_for_a_nan_state() {
    let (geom, mut u, p) = lid_cavity(12);
    u.values[17][0] = f64::NAN;
    let mut s = CoupledSolver::new(geom, u, p, CoupledSettings::new(0.01)).unwrap();
    let info = s.iterate();
    println!("N3: {info:?}");
    assert!(
        info.max_du.is_nan() && info.continuity.is_nan() && info.res_u.is_nan(),
        "N3: {info:?}"
    );
}

#[test]
fn n4_non_finite_trip_is_located_at_a_cell() {
    let mut s = sm_n();
    let mut g = SteadyGuard::new(false);
    for _ in 0..3000 {
        let info = s.iterate();
        if let Some(d) = g.check(&s.geom.clone(), &info, &s.u, &s.p.values, &s.phi) {
            println!("N4: {}", d.message(Some(s.geom.mesh.cell_centres())));
            assert_eq!(d.kind, "non_finite");
            assert_eq!(d.quantity, "velocity", "N4: {d:?}");
            assert!(d.cell.is_some());
            assert!(non_finite(&s.u));
            return;
        }
    }
    panic!("N4 set-up: no trip in 3000 iterations");
}
