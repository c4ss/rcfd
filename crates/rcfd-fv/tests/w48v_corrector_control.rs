//! ADR 0156 (job 48V): the adaptive pressure-corrector count of the PISO/PIMPLE loop, library-level checks.
//!
//! * A count that stays at `n_correctors` gives the fixed-count solution bit for bit (the estimates run on copies).
//! * A target of 0 drives the search to the cap; the run then equals a fixed run with the cap's count, bit for bit, and every
//!   estimate is recorded as cap-bound.
//! * The first estimate of a BDF2 run is made at step 2 (step 1 is implicit Euler).
//! * The growth monitor counts only consecutive growth above its factor and above the noise floor.
//! * The pre-run estimate (`lag_preflight`) leaves the solver unchanged.

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::pimple::{CorrectorControl, CorrectorRecord, PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::cavity_2d;
use std::sync::Arc;

fn fingerprint(s: &PimpleSolver) -> Vec<u64> {
    let mut v: Vec<u64> = Vec::new();
    for u in &s.u.values {
        v.push(u[0].to_bits());
        v.push(u[1].to_bits());
    }
    v.extend(s.p.values.iter().map(|x| x.to_bits()));
    v.extend(s.phi.iter().map(|x| x.to_bits()));
    v
}

/// The lid-driven cavity of `w33b_default_bits` (16 x 16, nu 0.01, linear upwind) with `f` applied to the settings.
fn cavity(f: impl FnOnce(&mut PimpleSettings)) -> PimpleSolver {
    let mesh = Arc::new(cavity_2d(16, 1.0).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = PimpleSettings::piso(0.01);
    st.convection = ConvScheme::LinearUpwind;
    f(&mut st);
    PimpleSolver::new(geom, u, p, st)
}

#[test]
fn adaptive_count_that_stays_equals_the_fixed_run_bit_for_bit() {
    let mut fixed = cavity(|_| {});
    let mut adaptive = cavity(|s| s.corrector_control = Some(CorrectorControl::registered()));
    fixed.run_fixed(1.0, 0.05);
    adaptive.run_fixed(1.0, 0.05);
    let r = &adaptive.correctors;
    println!("count {} (mean {}), estimates {:?}", r.last, r.mean(), r.estimates);
    assert_eq!(r.estimates[0].step, 1, "the first estimate at step 1");
    assert!(
        r.estimates.iter().all(|e| e.correctors == 2 && e.rho <= 0.5),
        "{:?}",
        r.estimates
    );
    assert_eq!((r.steps, r.sum, r.max, r.last), (20, 40, 2, 2));
    assert!(
        fingerprint(&fixed) == fingerprint(&adaptive),
        "the adaptive run differs from the fixed one"
    );
    assert!(
        fixed.correctors.estimates.is_empty() && fixed.correctors.steps == 0,
        "fixed run keeps no record"
    );
}

#[test]
fn unreachable_target_drives_the_count_to_the_cap() {
    let control = CorrectorControl {
        target: 0.0,
        every: 0,
        growth_steps: 0,
        ..CorrectorControl::registered()
    };
    let mut capped = cavity(|s| s.corrector_control = Some(control));
    let mut ten = cavity(|s| s.n_correctors = 10);
    capped.run_fixed(0.25, 0.05);
    ten.run_fixed(0.25, 0.05);
    let r = &capped.correctors;
    println!("estimates {:?}", r.estimates);
    assert_eq!(r.estimates.len(), 1);
    let e = r.estimates[0];
    assert!(e.cap_bound && e.correctors == 10 && e.evaluations == 9, "{e:?}");
    assert_eq!((r.cap_bound, r.last, r.max), (1, 10, 10));
    assert!(
        fingerprint(&capped) == fingerprint(&ten),
        "the capped run differs from a fixed run with 10 correctors"
    );
}

#[test]
fn bdf2_makes_its_first_estimate_at_step_2() {
    let mut s = cavity(|s| {
        s.time_scheme = TimeScheme::Bdf2;
        s.corrector_control = Some(CorrectorControl {
            start: 2,
            ..CorrectorControl::registered()
        });
    });
    s.run_fixed(0.25, 0.05);
    let r = &s.correctors;
    assert_eq!(r.estimates.len(), 1, "{:?}", r.estimates);
    assert_eq!(r.estimates[0].step, 2);
    assert_eq!(r.steps, 5);
}

#[test]
fn growth_monitor_counts_consecutive_growth_above_its_factor() {
    let c = CorrectorControl::registered();
    let fire = |changes: &[f64], range: f64| {
        let mut r = CorrectorRecord::default();
        for &d in changes {
            r.observe(d, range, &c);
        }
        r.monitor_fired()
    };
    assert!(fire(&[1e-3, 2e-3, 4e-3, 8e-3], 1.0), "three growths of 2x fire");
    assert!(!fire(&[1e-3, 2e-3, 4e-3], 1.0), "two growths do not");
    assert!(
        !fire(&[1e-3, 1.4e-3, 1.96e-3, 2.74e-3, 3.8e-3], 1.0),
        "growth 1.4 per step is below 1.5"
    );
    assert!(
        !fire(&[1e-3, 2e-3, 1e-3, 2e-3, 4e-3], 1.0),
        "the run of growth restarts"
    );
    assert!(
        !fire(&[1e-14, 2e-14, 4e-14, 8e-14], 1.0),
        "below 1e-10 of the pressure range"
    );
    let off = CorrectorControl { growth_steps: 0, ..c };
    let mut r = CorrectorRecord::default();
    for d in [1e-3, 2e-3, 4e-3, 8e-3, 16e-3] {
        r.observe(d, 1.0, &off);
    }
    assert!(!r.monitor_fired(), "growth_steps 0 is no monitor");
}

#[test]
fn preflight_leaves_the_solver_unchanged() {
    let mut a = cavity(|_| {});
    let b = cavity(|_| {});
    a.run_fixed(0.25, 0.05);
    let before = fingerprint(&a);
    let w = a.lag_preflight(0.05, 20).expect("an estimate");
    println!("preflight {w:?}");
    assert_eq!(w.step, 6);
    assert!(fingerprint(&a) == before, "the preflight changed the state");
    assert!(a.lag_estimates.is_empty() && a.history.len() == 5 && a.time == 0.25);
    // and the next steps are those of a solver that never made one
    let mut c = b;
    c.run_fixed(0.25, 0.05);
    a.run_fixed(0.5, 0.05);
    c.run_fixed(0.5, 0.05);
    assert!(fingerprint(&a) == fingerprint(&c));
}
