//! ADR 0067 section 0: the PIMPLE/PISO divergence guard. Checks G1 (synthetic unstable run stops, located, cheaply),
//! G4 (finite states are bit-identical with the guard on or effectively off). The limits are the registered ones.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::cavity_2d;
use std::sync::Arc;
use std::time::Instant;

fn cavity(n: usize, settings: PimpleSettings) -> PimpleSolver {
    let mesh = Arc::new(cavity_2d(n, 1.0).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    PimpleSolver::new(geom, u, p, settings)
}

/// The registered candidates, tried in order: (name, settings, dt).
fn candidates() -> Vec<(&'static str, PimpleSettings, f64)> {
    let mut a = PimpleSettings::piso(1e-5);
    a.convection = ConvScheme::Central;
    a.ddt_correction = true;
    let mut b = PimpleSettings::pimple(1e-5, 4);
    b.convection = ConvScheme::Central;
    b.alpha_u = 1.9;
    b.alpha_p = 1.5;
    let mut c = a.clone();
    c.time_scheme = TimeScheme::Bdf2;
    vec![("A", a, 2.0), ("B", b, 2.0), ("C", c, 5.0)]
}

/// Run until the guard trips or 500 steps; returns (steps taken, solver).
fn run_until_trip(mut s: PimpleSolver, dt: f64) -> (usize, PimpleSolver) {
    let mut n = 0;
    while n < 500 && s.diverged.is_none() {
        s.step(dt);
        n += 1;
    }
    (n, s)
}

#[test]
fn g1_unstable_run_stops_located_and_cheaply() {
    let mut found = false;
    for (name, st, dt) in candidates() {
        let t0 = Instant::now();
        let (n, mut s) = run_until_trip(cavity(12, st), dt);
        let Some(d) = s.diverged.clone() else {
            eprintln!("candidate {name}: no divergence within 500 steps");
            continue;
        };
        found = true;
        let secs = t0.elapsed().as_secs_f64();
        eprintln!(
            "candidate {name}: tripped at step {n}: {}",
            d.message(Some(s.geom.mesh.cell_centres()))
        );
        assert!(n <= 300, "candidate {name}: guard needed {n} steps (limit 300)");
        assert!(secs < 20.0, "candidate {name}: {secs} s (limit 20 s)");
        assert!(
            d.info.p_iters <= 3000,
            "stopping step spent {} pressure iterations (limit 3000)",
            d.info.p_iters
        );
        assert_eq!(d.step, n);
        let msg = d.message(Some(s.geom.mesh.cell_centres()));
        assert!(
            msg.contains("divergence guard") && msg.contains(&format!("step {n}")),
            "{msg}"
        );
        // later calls do no work
        let t = s.time;
        for _ in 0..5 {
            let i = s.step(dt);
            assert_eq!(i.p_iters, d.info.p_iters, "stale record expected");
        }
        assert_eq!(s.time, t, "time must not advance after the trip");
        // restore clears the guard
        let snap = s.snapshot();
        s.restore(&snap).unwrap();
        assert!(s.diverged.is_none());
        break;
    }
    assert!(found, "no registered candidate blew up; see the ADR 0067 addendum");
}

#[test]
fn courant_limit_trips_a_finite_state_and_infinity_does_not() {
    // a stable run, but a limit below the actual Courant number
    let mut st = PimpleSettings::piso(0.01);
    st.convection = ConvScheme::LinearUpwind;
    st.divergence_courant_limit = 1e-6;
    let mut s = cavity(8, st);
    let i = s.step(0.05);
    let d = s.diverged.clone().expect("tripped on Courant");
    assert_eq!(d.quantity, "courant");
    assert!(d.value > 1e-6 && i.courant == d.value);
    assert!(d.message(None).contains("Courant"));
    // limit = infinity: the same step is accepted
    let mut st = PimpleSettings::piso(0.01);
    st.convection = ConvScheme::LinearUpwind;
    st.divergence_courant_limit = f64::INFINITY;
    let mut s = cavity(8, st);
    s.step(0.05);
    assert!(s.diverged.is_none());
}

#[test]
fn g4_finite_states_are_bit_identical_with_and_without_the_courant_limit() {
    let mk = |limit: f64| {
        let mut st = PimpleSettings::piso(0.01);
        st.convection = ConvScheme::LinearUpwind;
        st.divergence_courant_limit = limit;
        let mut s = cavity(16, st);
        for _ in 0..200 {
            s.step(0.02);
        }
        s
    };
    let (a, b) = (mk(1e6), mk(1e300));
    assert!(a.diverged.is_none() && b.diverged.is_none());
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    let ub = |s: &PimpleSolver| {
        s.u.values
            .iter()
            .flat_map(|v| v.iter().map(|x| x.to_bits()))
            .collect::<Vec<_>>()
    };
    assert_eq!(ub(&a), ub(&b));
    assert_eq!(bits(&a.p.values), bits(&b.p.values));
    assert_eq!(bits(&a.phi), bits(&b.phi));
}
