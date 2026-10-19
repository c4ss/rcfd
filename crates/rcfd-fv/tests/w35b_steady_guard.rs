//! ADR 0077 part A, checks A1a-A1c: the steady divergence guard (`SteadyGuard`) on synthetic unstable SIMPLE/SIMPLEC runs at fv level.
//! Limits are the registered ones.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver, SteadyDivergence, SteadyGuard};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{cavity_2d, channel, warp, Grading};
use std::sync::Arc;
use std::time::Instant;

/// The S2 cavity of ADR 0077 at fv level: 12 x 12, lid 1, SIMPLE, upwind, alpha_u = alpha_p = 1.
fn cavity(nu: f64) -> SimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(12, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = SimpleSettings::new(nu, Coupling::Simple);
    st.convection = ConvScheme::Upwind;
    st.alpha_u = 1.0;
    st.alpha_p = 1.0;
    SimpleSolver::new(geom, u, p, st)
}

/// The S3 sheared channel: 40 x 10, points mapped x' = x + y tan(theta), SIMPLEC (0.9/1.0), upwind, nu 0.01.
fn sheared(deg: f64) -> SimpleSolver {
    let t = deg.to_radians().tan();
    let m = channel([40, 10, 1], [4.0, 1.0, 0.1], [Grading::Uniform; 3], true).unwrap();
    let m = warp(&m, |p| [p[0] + t * p[1], p[1], p[2]]).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(m)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "inlet", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut st = SimpleSettings::new(0.01, Coupling::Simplec);
    st.convection = ConvScheme::Upwind;
    SimpleSolver::new(geom, u, p, st)
}

/// Iterate with the guard observing until it trips or `cap` iterations; (trip, wall seconds).
fn guarded(mut s: SimpleSolver, cap: usize) -> (Option<SteadyDivergence>, f64, SimpleSolver) {
    let t0 = Instant::now();
    let mut g = SteadyGuard::new(true);
    for _ in 0..cap {
        let info = s.iterate();
        if let Some(d) = g.check(&s.geom.clone(), &info, &s.u, &s.p.values, &s.phi) {
            return (Some(d), t0.elapsed().as_secs_f64(), s);
        }
    }
    (None, t0.elapsed().as_secs_f64(), s)
}

fn finite_state(s: &SimpleSolver) -> bool {
    s.u.values.iter().all(|v| v.iter().all(|x| x.is_finite()))
        && s.p.values.iter().all(|x| x.is_finite())
        && s.phi.iter().all(|x| x.is_finite())
}

#[test]
fn a1a_finite_runaway_cavity_trips_within_ten_iterations() {
    let (d, secs, s) = guarded(cavity(0.01), 3000);
    let d = d.expect("A1a: the guard must trip");
    println!("A1a: {} ({secs:.3} s)", d.message(Some(s.geom.mesh.cell_centres())));
    assert!(
        d.iteration <= 10,
        "A1a: tripped at iteration {} (limit 10)",
        d.iteration
    );
    assert_ne!(
        d.kind, "non_finite",
        "A1a: a finite runaway must be caught before it is non-finite"
    );
    assert!(d.cell.is_some(), "A1a: located");
    assert!(secs < 5.0, "A1a: {secs} s (limit 5 s)");
}

#[test]
fn a1b_guard_trips_before_the_unguarded_twin_is_non_finite() {
    let mut twin = cavity(1e-3);
    let mut n_nan = None;
    for i in 1..=3000 {
        twin.iterate();
        if !finite_state(&twin) {
            n_nan = Some(i);
            break;
        }
    }
    let n_nan = n_nan.expect("A1b set-up: the unguarded twin must become non-finite within 3000 iterations");
    let (d, _, s) = guarded(cavity(1e-3), 3000);
    let d = d.expect("A1b: the guard must trip");
    println!(
        "A1b: twin non-finite at iteration {n_nan}; guard: {}",
        d.message(Some(s.geom.mesh.cell_centres()))
    );
    assert!(d.iteration < n_nan, "A1b: guard at {} not before {n_nan}", d.iteration);
}

#[test]
fn a1c_sheared_channel_trips_within_sixty_iterations() {
    let (d, secs, s) = guarded(sheared(79.0), 600);
    let d = d.expect("A1c: the guard must trip");
    println!("A1c: {} ({secs:.3} s)", d.message(Some(s.geom.mesh.cell_centres())));
    assert!(
        d.iteration <= 60,
        "A1c: tripped at iteration {} (limit 60)",
        d.iteration
    );
}

/// Not registered (plumbing): a converging run is not stopped and only K1 is left with `runaway = false`.
#[test]
fn converging_cavity_is_not_stopped_and_off_keeps_only_k1() {
    let mut s = cavity(0.01);
    s.settings.alpha_u = 0.7;
    s.settings.alpha_p = 0.3;
    let (d, _, s) = guarded(s, 300);
    assert!(d.is_none(), "{d:?}");
    assert!(s.history.last().unwrap().res_u < 1e-6);
    let mut off = SteadyGuard::new(false);
    let mut c = cavity(0.01);
    for _ in 0..30 {
        let info = c.iterate();
        assert!(off.check(&c.geom.clone(), &info, &c.u, &c.p.values, &c.phi).is_none());
    }
}
