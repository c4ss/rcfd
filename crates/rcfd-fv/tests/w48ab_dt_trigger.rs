//! ADR 0162 (job 48AB): the dt trigger of the adaptive pressure-corrector count, library-level checks L1-L6.
//!
//! * L1: a constant dt makes no dt-fired estimate and the run equals one without the trigger, bit for bit.
//! * L2: a step 1.4 times the last estimate's makes none; L3: twice makes one, at that step, with the new dt.
//! * L4: half the step makes one (the down direction).
//! * L5: the short last step of `run_fixed` (a landing step) makes none; the run equals one without the trigger.
//! * L6: `dt_factor` 0 is ADR 0156's schedule (no dt-fired estimate).

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::pimple::{CorrectorControl, PimpleSettings, PimpleSolver};
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

/// The lid-driven cavity of ADR 0156's library tests (16 x 16, nu 0.01, linear upwind) with the adaptive control `c`.
fn cavity(c: CorrectorControl) -> PimpleSolver {
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
    st.corrector_control = Some(c);
    PimpleSolver::new(geom, u, p, st)
}

fn off() -> CorrectorControl {
    CorrectorControl {
        dt_factor: 0.0,
        ..CorrectorControl::registered()
    }
}

/// `n1` steps of `dt1`, then `n2` steps of `dt2`.
fn two_steps(c: CorrectorControl, dt1: f64, n1: usize, dt2: f64, n2: usize) -> PimpleSolver {
    let mut s = cavity(c);
    for _ in 0..n1 {
        s.step(dt1);
    }
    for _ in 0..n2 {
        s.step(dt2);
    }
    s
}

fn dt_fired(s: &PimpleSolver) -> Vec<(usize, f64)> {
    s.correctors
        .estimates
        .iter()
        .filter(|e| e.dt_triggered)
        .map(|e| (e.step, e.dt))
        .collect()
}

#[test]
fn l1_constant_dt_makes_no_dt_fired_estimate() {
    assert_eq!(CorrectorControl::registered().dt_factor, 1.5);
    let on = two_steps(CorrectorControl::registered(), 0.05, 20, 0.05, 0);
    let without = two_steps(off(), 0.05, 20, 0.05, 0);
    println!("L1: {:?}", on.correctors.estimates);
    assert!(dt_fired(&on).is_empty());
    assert_eq!(on.correctors.dt_triggered, 0);
    assert!(
        fingerprint(&on) == fingerprint(&without),
        "L1: differs from the run without the trigger"
    );
}

#[test]
fn l2_l3_dt_growth_fires_at_its_factor() {
    let small = two_steps(CorrectorControl::registered(), 0.05, 5, 0.07, 5);
    println!("L2: {:?}", small.correctors.estimates);
    assert!(dt_fired(&small).is_empty(), "L2: x1.4 fired");
    let big = two_steps(CorrectorControl::registered(), 0.05, 5, 0.1, 5);
    println!("L3: {:?}", big.correctors.estimates);
    assert_eq!(dt_fired(&big), vec![(6, 0.1)], "L3");
    assert_eq!(big.correctors.dt_triggered, 1);
    assert_eq!(big.correctors.triggered, 0, "L3: the monitor's count is not touched");
    let e = big.correctors.estimates.iter().find(|e| e.dt_triggered).unwrap();
    assert!(e.work >= 2 * 20 && e.evaluations >= 1, "L3: {e:?}");
}

#[test]
fn l4_dt_fall_fires_too() {
    let c = CorrectorControl {
        target: 0.0,
        ..CorrectorControl::registered()
    };
    let s = two_steps(c, 0.1, 5, 0.05, 3);
    println!("L4: {:?}", s.correctors.estimates);
    assert!(s.correctors.estimates[0].correctors > 2);
    assert_eq!(dt_fired(&s), vec![(6, 0.05)], "L4");
}

#[test]
fn l5_short_last_step_of_run_fixed_is_a_landing_step() {
    let mut on = cavity(CorrectorControl::registered());
    let mut without = cavity(off());
    on.run_fixed(0.27, 0.05);
    without.run_fixed(0.27, 0.05);
    let last = on.history.last().unwrap().dt;
    println!("L5: last step {last}, {:?}", on.correctors.estimates);
    assert!(last < 0.05 / 1.5, "L5: the set-up needs a short last step ({last})");
    assert!(dt_fired(&on).is_empty(), "L5");
    assert!(
        fingerprint(&on) == fingerprint(&without),
        "L5: differs from the run without the trigger"
    );
}

#[test]
fn l6_factor_zero_is_adr_0156_schedule() {
    let s = two_steps(off(), 0.05, 5, 0.1, 5);
    println!("L6: {:?}", s.correctors.estimates);
    assert!(dt_fired(&s).is_empty());
    assert_eq!(s.correctors.estimates.len(), 1);
}
