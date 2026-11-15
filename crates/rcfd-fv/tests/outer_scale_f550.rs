//! F-550 (ADR 0617): the windowed scale `PimpleSettings::outer_scale_window` of the outer-iteration criterion of ADR 0458 and the
//! count of the pressure-lag estimate of a step with the criterion, on the small driven channel of ADR 0458 E1 to E4. W1: a
//! window of one is ADR 0458's scale bit for bit; W2: the scale is the windowed maximum of the first changes; W3: the window
//! alone changes nothing; W4: a fluid at rest; L1, L2: the B4 estimate and the adaptive corrector count of a step with the
//! criterion are those of the fixed count the step used. The limits were registered in ADR 0617 before this file existed.

use rcfd_core::vec3;
use rcfd_fv::bc::FixedValue;
use rcfd_fv::pimple::{CorrectorControl, LagAction, LagCheck, OuterRecord, PimpleSettings, PimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_mesh, Grading};
use std::f64::consts::PI;
use std::sync::Arc;

const NU: f64 = 1e-2;
const DT: f64 = 0.05;
const STEPS: usize = 8;

/// A channel of 12 x 6 x 1 cells over `2 x 1 x 0.2`: no-slip `ymin`, `ymax`, `p = 0` at `xmax`, the inflow at `xmin`.
fn solver(s: PimpleSettings) -> PimpleSolver {
    let mesh = box_mesh(
        [12, 6, 1],
        [2.0, 1.0, 0.2],
        [Grading::Uniform, Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
    for name in ["ymin", "ymax", "xmin"] {
        u.set_bc(&geom, name, Box::new(FixedValue::vector(vec3::ZERO))).unwrap();
    }
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    PimpleSolver::new(geom, u, p, s)
}

/// One step `k` (1-based); the inflow `sin(pi t)` when `driven`, none otherwise.
fn step(fl: &mut PimpleSolver, k: usize, driven: bool) {
    if driven {
        let g = fl.geom.clone();
        let v = (PI * k as f64 * DT).sin();
        fl.u.set_bc(&g, "xmin", Box::new(FixedValue::vector([v, 0.0, 0.0])))
            .unwrap();
    }
    fl.step(DT);
    assert!(fl.diverged.is_none(), "step {k} stopped");
}

fn run(mut fl: PimpleSolver, driven: bool) -> PimpleSolver {
    for k in 1..=STEPS {
        step(&mut fl, k, driven);
    }
    fl
}

fn settings(n_outer: usize, tol: Option<f64>, window: Option<usize>) -> PimpleSettings {
    let mut s = PimpleSettings::piso(NU);
    s.n_outer = n_outer;
    s.outer_pressure_tolerance = tol;
    s.outer_scale_window = window;
    s
}

fn bits(fl: &PimpleSolver) -> Vec<u64> {
    fl.u.values
        .iter()
        .flatten()
        .chain(&fl.p.values)
        .chain(&fl.phi)
        .map(|x| x.to_bits())
        .collect()
}

fn record_bits(r: &OuterRecord) -> (usize, usize, bool, u64, u64, u64) {
    (r.step, r.used, r.met, r.ratio.to_bits(), r.first.to_bits(), r.scale.to_bits())
}

#[test]
fn w1_window_of_one_is_the_adr_0458_scale_bit_for_bit() {
    let none = run(solver(settings(30, Some(1e-2), None)), true);
    let one = run(solver(settings(30, Some(1e-2), Some(1))), true);
    assert!(bits(&none) == bits(&one), "W1: the fields differ");
    assert_eq!(none.outer_record.len(), STEPS);
    for (a, b) in none.outer_record.iter().zip(&one.outer_record) {
        println!("W1 {a:?}");
        assert_eq!(record_bits(a), record_bits(b), "W1: the records differ");
        assert_eq!(a.first.to_bits(), a.scale.to_bits(), "W1: scale is not first without the window");
    }
}

#[test]
fn w2_scale_is_the_windowed_maximum_of_the_first_changes() {
    let fl = run(solver(settings(30, Some(1e-2), Some(3))), true);
    let r = &fl.outer_record;
    assert_eq!(r.len(), STEPS, "W2: one record per step");
    for (i, e) in r.iter().enumerate() {
        println!("W2 {e:?}");
        assert_eq!(e.step, i + 1);
        let lo = i.saturating_sub(2);
        let m = r[lo..=i].iter().fold(0.0f64, |m, x| m.max(x.first));
        assert_eq!(e.scale.to_bits(), m.to_bits(), "W2: step {} scale {} against {m}", e.step, e.scale);
        assert!(e.scale >= e.first, "W2: scale below first");
        assert!(e.met, "W2: met");
        assert!(e.used < 30, "W2: used {}", e.used);
        assert!(e.ratio <= 1e-2, "W2: ratio {}", e.ratio);
    }
    // the window slides: some step's scale is not its own first change
    println!(
        "W2: {} of {STEPS} steps scaled by an earlier step",
        r.iter().filter(|e| e.scale > e.first).count()
    );
}

#[test]
fn w3_window_without_the_criterion_changes_nothing() {
    let fixed = run(solver(settings(6, None, None)), true);
    let window = run(solver(settings(6, None, Some(3))), true);
    assert!(bits(&fixed) == bits(&window), "W3: the fields differ");
    assert!(window.outer_record.is_empty(), "W3: a record without the criterion");
}

#[test]
fn w4_rest_meets_at_once_with_the_window() {
    let fl = run(solver(settings(6, Some(1e-3), Some(3))), false);
    assert_eq!(fl.outer_record.len(), STEPS, "W4: one record per step");
    for (i, r) in fl.outer_record.iter().enumerate() {
        println!("W4 {r:?}");
        assert_eq!(r.step, i + 1);
        assert_eq!(r.used, 2, "W4: used");
        assert!(r.met, "W4: met");
    }
}

/// L1, L2: the solver with the criterion and a second one without it, in lockstep, the second's `n_outer` set before each step
/// to the count the first used in it.
fn lockstep(extra: impl Fn(&mut PimpleSettings)) -> (PimpleSolver, PimpleSolver) {
    let mut a_s = settings(30, Some(1e-2), Some(3));
    extra(&mut a_s);
    let mut b_s = settings(30, None, None);
    extra(&mut b_s);
    let mut a = solver(a_s);
    let mut b = solver(b_s);
    for k in 1..=STEPS {
        step(&mut a, k, true);
        let used = a.outer_record.last().unwrap().used;
        b.settings.n_outer = used;
        step(&mut b, k, true);
        assert!(bits(&a) == bits(&b), "step {k}: the fields differ (used {used})");
    }
    (a, b)
}

#[test]
fn l1_lag_estimate_uses_the_count_the_step_used() {
    let (a, b) = lockstep(|s| {
        s.pressure_lag_check = Some(LagCheck {
            iterations: 6,
            ..LagCheck::new(1, LagAction::Warn)
        })
    });
    assert_eq!(a.lag_estimates.len(), STEPS, "L1: one estimate per step");
    assert_eq!(b.lag_estimates.len(), STEPS);
    for (x, y) in a.lag_estimates.iter().zip(&b.lag_estimates) {
        println!("L1 {x:?} {y:?}");
        assert_eq!(x.step, y.step);
        assert_eq!(x.rho.to_bits(), y.rho.to_bits(), "L1: step {} rho {} against {}", x.step, x.rho, y.rho);
        assert_eq!(x.cell, y.cell, "L1: cell");
    }
    let fewer = a.outer_record.iter().filter(|r| r.used < 30).count();
    println!("L1: {fewer} of {STEPS} steps below the cap");
    assert!(fewer >= 1, "L1: no step below the cap");
}

#[test]
fn l2_adaptive_corrector_count_uses_the_count_the_step_used() {
    let (a, b) = lockstep(|s| {
        s.corrector_control = Some(CorrectorControl {
            every: 1,
            ..CorrectorControl::registered()
        })
    });
    let (ea, eb) = (&a.correctors.estimates, &b.correctors.estimates);
    assert!(!ea.is_empty(), "L2: no estimate");
    assert_eq!(ea.len(), eb.len(), "L2: estimate counts");
    for (x, y) in ea.iter().zip(eb) {
        println!("L2 {x:?}");
        assert_eq!(x.step, y.step);
        assert_eq!(x.correctors, y.correctors, "L2: correctors");
        assert_eq!(x.rho.to_bits(), y.rho.to_bits(), "L2: step {} rho {} against {}", x.step, x.rho, y.rho);
    }
}
