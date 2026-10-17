//! G2 of docs/adr/0055-wave29a-rotor-interfaces-bcs.md: the guarded secant update of the target-mass-flow outlet.

use rcfd_fv::outlets::{PressureUpdate, PressureUpdater};

/// Linear response m(p) = 0.3 - 0.04 p, target 0.1, rho 1, area 0.2, relaxation 1. The dynamic-head slope of the first step is
/// 6.5 (true slope 25), the first step changes m by less than half its error, so the guard passes and the guarded rule
/// equals the plain secant rule (and reaches the root p = 5) to 1e-14.
#[test]
fn g2_guarded_equals_secant_when_the_guard_passes() {
    let (rho, a, t) = (1.0, 0.2, 0.1);
    let m_of = |p: f64| 0.3 - 0.04 * p;
    let root = (0.3 - t) / 0.04;
    let mut sec = PressureUpdater::new(PressureUpdate::Secant, 1.0);
    let mut grd = PressureUpdater::new(PressureUpdate::GuardedSecant, 1.0);
    let (mut ps, mut pg) = (1.0, 1.0);
    for k in 0..3 {
        ps = sec.step(ps, m_of(ps), t, rho, a);
        pg = grd.step(pg, m_of(pg), t, rho, a);
        println!("G2 step {k}: secant {ps:.15}, guarded {pg:.15}");
        assert!((ps - pg).abs() <= 1e-14 * ps.abs().max(1.0), "{ps} {pg}");
    }
    assert!((pg - root).abs() <= 1e-12 * root, "{pg} {root}");
}

/// A response so steep that the second step would be accepted by the plain secant although the mass-flow change is larger
/// than half the error (spin-up like): the guarded rule falls back to the dynamic-head slope.
#[test]
fn g2_guarded_falls_back_to_the_dynamic_head_slope() {
    let (rho, a, t) = (1.0, 0.1, 0.1);
    let m_of = |p: f64| 0.5 - 0.04 * p;
    let mut grd = PressureUpdater::new(PressureUpdate::GuardedSecant, 1.0);
    let mut ber = PressureUpdater::new(PressureUpdate::Bernoulli, 1.0);
    let p1 = grd.step(1.0, m_of(1.0), t, rho, a);
    let q1 = ber.step(1.0, m_of(1.0), t, rho, a);
    assert!((p1 - q1).abs() <= 1e-14 * q1.abs());
    // second step: |dm| = 1.164 > 0.5 |e| = 0.40, so the Bernoulli slope is used again
    let p2 = grd.step(p1, m_of(p1), t, rho, a);
    let q2 = ber.step(p1, m_of(p1), t, rho, a);
    println!("G2 fallback: guarded {p2}, bernoulli {q2}");
    assert!((p2 - q2).abs() <= 1e-14 * q2.abs().max(1.0), "{p2} {q2}");
}
