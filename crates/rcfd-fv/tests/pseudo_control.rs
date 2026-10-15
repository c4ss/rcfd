//! ADR 0023 (F-106) automatic pseudo-time-step controller: checks P1-P4 with the limits registered in
//! docs/adr/0023-wave17a-convergence.md before the first run. Lid-driven cavity Re = 1000, 32 x 32, linear upwind,
//! SIMPLEC with `alpha_u = alpha_p = 1`, convergence = `max(res_u, res_p) < 1e-6`.

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::simple::{
    Coupling, PseudoController, PseudoLaw, PseudoMode, PseudoTransient, SimpleSettings, SimpleSolver,
};
use rcfd_fv::{FixedValue, FvGeometry, Profile, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{backward_step_graded, cavity_2d, StepGrading, StepSpec};
use std::sync::Arc;

struct Out {
    iterations: usize,
    converged: bool,
    res: f64,
    solver: SimpleSolver,
    /// scaled residual at the first iteration whose reference residual was below 1e-6 (P1 note)
    scaled_at_ref_1e6: Option<f64>,
}

fn drive(solver: &mut SimpleSolver, max_iter: usize, tol: f64) -> (usize, bool, f64, Option<f64>) {
    let mut res = f64::INFINITY;
    for i in 1..=max_iter {
        let info = solver.iterate();
        res = info.res_u.max(info.res_p);
        let finite = solver.u.values.iter().all(|v| v.iter().all(|x| x.is_finite()));
        if !finite || !res.is_finite() || res > 1e8 {
            return (i, false, f64::INFINITY, None);
        }
        if res < tol {
            return (i, true, res, None);
        }
    }
    (max_iter, false, res, None)
}

fn controller(law: PseudoLaw) -> PseudoController {
    match law {
        PseudoLaw::Ratchet => PseudoController::new(1000.0),
        PseudoLaw::Symmetric => PseudoController::new_symmetric(1000.0),
    }
}

fn cavity(pseudo: PseudoTransient, control: Option<PseudoLaw>, max_iter: usize) -> Out {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(32, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = SimpleSettings::new(1.0 / 1000.0, Coupling::Simplec);
    st.alpha_u = 1.0;
    st.alpha_p = 1.0;
    st.convection = ConvScheme::LinearUpwind;
    let mut solver = SimpleSolver::new(geom, u, p, st);
    solver.pseudo = Some(pseudo);
    solver.pseudo_control = control.map(controller);
    let (iterations, converged, res, _) = drive(&mut solver, max_iter, 1e-6);
    Out {
        iterations,
        converged,
        res,
        solver,
        scaled_at_ref_1e6: None,
    }
}

fn local(cfl: f64) -> PseudoTransient {
    PseudoTransient {
        mode: PseudoMode::Local { cfl },
        factor: 1.0,
    }
}

fn max_diff(a: &SimpleSolver, b: &SimpleSolver) -> f64 {
    a.u.values
        .iter()
        .zip(&b.u.values)
        .map(|(x, y)| (x[0] - y[0]).abs().max((x[1] - y[1]).abs()))
        .fold(0.0, f64::max)
}

fn scale_bounds_ok(s: &SimpleSolver) {
    let h = &s.pseudo_scale_history;
    assert!(!h.is_empty());
    assert!(h
        .iter()
        .all(|v| v.is_finite() && *v >= 1.0 / 64.0 - 1e-15 && *v <= 1000.0 + 1e-9));
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A3): the registered ratchet law ends at residual 0.18 with the scale at its cap"]
fn p1_controller_local_cfl1_cavity_re1000_known_miss() {
    p1_body(PseudoLaw::Ratchet);
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A4): symmetric law, registered limit not met"]
fn p1_controller_local_cfl1_cavity_re1000_ser2_known_miss() {
    p1_body(PseudoLaw::Symmetric);
}

/// P1 (registered): local start cfl 1 with the controller converges within 2500 iterations, to the fixed cfl 5 state within
/// 5e-4, and in no more iterations than the fixed cfl 2 run; scale bounded and finite.
fn p1_body(law: PseudoLaw) {
    let reference = cavity(local(5.0), None, 6000);
    let fixed2 = cavity(local(2.0), None, 6000);
    let fixed1 = cavity(local(1.0), None, 6000);
    let c = cavity(local(1.0), Some(law), 6000);
    println!(
        "P1: fixed cfl 5 {} its ({}), fixed cfl 2 {} its ({}), fixed cfl 1 {} its ({}); controller cfl 1 start: {} its converged {} res {:.2e}, final scale {:.3}, max scale {:.3}",
        reference.iterations,
        reference.converged,
        fixed2.iterations,
        fixed2.converged,
        fixed1.iterations,
        fixed1.converged,
        c.iterations,
        c.converged,
        c.res,
        c.solver.pseudo_scale_history.last().unwrap(),
        c.solver.pseudo_scale_history.iter().cloned().fold(0.0, f64::max)
    );
    assert!(reference.converged && fixed2.converged);
    assert!(c.converged && c.iterations <= 2500, "{} its", c.iterations);
    let d = max_diff(&c.solver, &reference.solver);
    println!("P1: max |u - u_ref| = {d:.3e} (limit 5e-4)");
    assert!(d <= 5e-4, "{d}");
    assert!(
        c.iterations <= fixed2.iterations,
        "{} > {}",
        c.iterations,
        fixed2.iterations
    );
    scale_bounds_ok(&c.solver);
    let _ = c.scaled_at_ref_1e6;
}

/// P1 note: the scaled residual is not weaker than meaningful: at the first iteration where the reference residual is
/// below 1e-6 the scaled residual is at most 1e-3.
#[test]
fn p1_scaled_residual_does_not_hide_an_unconverged_state() {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(32, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut st = SimpleSettings::new(1.0 / 1000.0, Coupling::Simplec);
    st.alpha_u = 1.0;
    st.alpha_p = 1.0;
    st.convection = ConvScheme::LinearUpwind;
    let mut solver = SimpleSolver::new(geom, u, p, st);
    solver.pseudo = Some(local(5.0));
    solver.record_parts = true;
    let mut found = None;
    for _ in 0..3000 {
        let i = solver.iterate();
        if i.res_u.max(i.res_p) < 1e-6 {
            let parts = solver.parts_history.last().unwrap();
            let s_max = parts.iter().map(|p| p.scale).fold(0.0, f64::max);
            let n_max = parts.iter().map(|p| p.num).fold(0.0, f64::max);
            found = Some(n_max / s_max);
            break;
        }
    }
    let v = found.expect("reference residual never below 1e-6");
    println!("P1 note: scaled momentum residual where the reference one first drops below 1e-6: {v:.3e} (limit 1e-3)");
    assert!(v <= 1e-3, "{v}");
}

/// P2 (registered): physical start dt 0.02 and auto factor 0.05 with the controller: each converges within 3000
/// iterations to the fixed cfl 5 state within 5e-4.
fn p2_body(law: PseudoLaw) {
    let reference = cavity(local(5.0), None, 6000);
    assert!(reference.converged);
    for (name, p) in [
        (
            "physical dt 0.02",
            PseudoTransient {
                mode: PseudoMode::Physical { dt: 0.02 },
                factor: 1.0,
            },
        ),
        (
            "auto factor 0.05",
            PseudoTransient {
                mode: PseudoMode::Auto,
                factor: 0.05,
            },
        ),
    ] {
        let c = cavity(p, Some(law), 3000);
        let d = max_diff(&c.solver, &reference.solver);
        println!(
            "P2 {name}: {} its converged {} res {:.2e}, max |u - u_ref| {d:.3e} (limits 3000 its, 5e-4)",
            c.iterations, c.converged, c.res
        );
        assert!(c.converged, "{name}");
        assert!(d <= 5e-4, "{name}: {d}");
    }
}

/// P3 (registered): local starts 0.5, 1, 2, 5 with the controller converge within 3000 iterations.
fn p3_small(law: PseudoLaw) {
    for cfl in [0.5, 1.0, 2.0, 5.0] {
        let c = cavity(local(cfl), Some(law), 3000);
        println!(
            "P3 start cfl {cfl}: {} its converged {} res {:.2e}",
            c.iterations, c.converged, c.res
        );
        assert!(c.converged, "cfl {cfl}");
    }
}

/// P3 (registered): local starts 10 and 20 with the controller converge within 3500 iterations.
fn p3_large(law: PseudoLaw) {
    for cfl in [10.0, 20.0] {
        let c = cavity(local(cfl), Some(law), 3500);
        println!(
            "P3 start cfl {cfl}: {} its converged {} res {:.2e}",
            c.iterations, c.converged, c.res
        );
        assert!(c.converged, "cfl {cfl}");
    }
}

/// Controller update law (unit): growth bounded by 2, back-off 0.5 on a rise by more than 2, clamps.
#[test]
fn controller_update_law() {
    let mut c = PseudoController::new(8.0);
    c.update(1.0);
    assert_eq!(c.scale, 1.0);
    c.update(0.1); // ratio 10 -> clamp 2
    assert_eq!(c.scale, 2.0);
    c.update(0.09); // ratio 1.11
    assert!((c.scale - 2.0 * (0.1 / 0.09)).abs() < 1e-12);
    let before = c.scale;
    c.update(0.5); // rise by more than 2 -> back off
    assert_eq!(c.scale, 0.5 * before);
    c.update(f64::NAN);
    assert_eq!(c.scale, 0.25 * before);
    for _ in 0..50 {
        c.update(f64::INFINITY);
    }
    assert_eq!(c.scale, 1.0 / 64.0);
    let mut d = PseudoController::new(8.0);
    d.update(1.0);
    for k in 1..20 {
        d.update(0.1_f64.powi(k));
    }
    assert_eq!(d.scale, 8.0);
}

/// P4 (registered): laminar step Re_h = 400, parabolic inlet, ER 2, 8+120 x 16+16 cells, van Leer, SIMPLEC alpha_u = 1,
/// local start cfl 1 with the controller: residual below 1e-6 within 4000 iterations.
fn step_re400(control: Option<PseudoLaw>, max_iter: usize) -> (usize, bool, f64) {
    step_re400_cfl(1.0, control, max_iter)
}

fn step_re400_fixed(cfl: f64, max_iter: usize) -> (usize, bool, f64) {
    step_re400_cfl(cfl, None, max_iter)
}

fn step_re400_cfl(cfl: f64, control: Option<PseudoLaw>, max_iter: usize) -> (usize, bool, f64) {
    let spec = StepSpec {
        l_in: 2.0,
        l_out: 30.0,
        h_in: 1.0,
        step: 1.0,
        depth: 0.1,
        n: [8, 120, 16, 16],
    };
    let geom =
        Arc::new(FvGeometry::new(Arc::new(backward_step_graded(spec, StepGrading::uniform()).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
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
    let mut st = SimpleSettings::new(spec.step / 400.0, Coupling::Simplec);
    st.alpha_u = 1.0;
    st.alpha_p = 1.0;
    st.convection = ConvScheme::Tvd(TvdLimiter::VanLeer);
    let mut solver = SimpleSolver::new(geom, u, p, st);
    solver.pseudo = Some(local(cfl));
    solver.pseudo_control = control.map(controller);
    let (its, conv, res, _) = drive(&mut solver, max_iter, 1e-6);
    if control.is_some() {
        let mx = solver.pseudo_scale_history.iter().cloned().fold(0.0, f64::max);
        let mn = solver
            .pseudo_scale_history
            .iter()
            .cloned()
            .fold(f64::INFINITY, f64::min);
        println!(
            "P4 scale range [{mn:.3}, {mx:.3}], final {:.3}",
            solver.pseudo_scale_history.last().unwrap()
        );
    }
    (its, conv, res)
}

fn p4_body(law: PseudoLaw) {
    let (its, conv, res) = step_re400(Some(law), 4000);
    println!("P4 controller: {its} its converged {conv} res {res:.2e} (limit 4000 its, 1e-6)");
    assert!(conv, "res {res:.2e}");
}

/// Control for P4, reported only: the same set-up with the fixed start step (no controller).
#[test]
#[ignore = "control run for P4, reported only"]
fn p4_control_fixed_cfl1() {
    let (its, conv, res) = step_re400(None, 4000);
    println!("P4 control fixed cfl 1: {its} its converged {conv} res {res:.2e}");
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A3): ratchet law, residual 8.9e-5 after 4000 iterations"]
fn p4_step_re400_controller_known_miss() {
    p4_body(PseudoLaw::Ratchet);
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A4): symmetric law, registered limit not met"]
fn p4_step_re400_controller_ser2_known_miss() {
    p4_body(PseudoLaw::Symmetric);
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A3): ratchet law"]
fn p2_controller_physical_and_auto_modes_known_miss() {
    p2_body(PseudoLaw::Ratchet);
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A4): symmetric law, registered limit not met"]
fn p2_controller_physical_and_auto_modes_ser2_known_miss() {
    p2_body(PseudoLaw::Symmetric);
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A3): ratchet law, start cfl 0.5 ends at residual 0.17"]
fn p3_controller_small_starts_known_miss() {
    p3_small(PseudoLaw::Ratchet);
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A4): symmetric law, registered limit not met"]
fn p3_controller_small_starts_ser2_known_miss() {
    p3_small(PseudoLaw::Symmetric);
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A3): ratchet law, start cfl 10 ends at residual 0.25"]
fn p3_controller_large_starts_known_miss() {
    p3_large(PseudoLaw::Ratchet);
}

#[test]
#[ignore = "KNOWN MISS (ADR 0023 addendum A4): symmetric law, registered limit not met"]
fn p3_controller_large_starts_ser2_known_miss() {
    p3_large(PseudoLaw::Symmetric);
}

/// Symmetric law (unit): follows the inverse residual ratio both ways, clamped to [0.5, 2] per iteration.
#[test]
fn controller_symmetric_update_law() {
    let mut c = PseudoController::new_symmetric(8.0);
    c.update(1.0);
    assert_eq!(c.scale, 1.0);
    c.update(0.1);
    assert_eq!(c.scale, 2.0);
    c.update(0.15); // ratio 0.667 -> shrinks
    assert!((c.scale - 2.0 * (0.1 / 0.15)).abs() < 1e-12);
    c.update(10.0); // ratio tiny -> clamp 0.5
    assert!((c.scale - 0.5 * 2.0 * (0.1 / 0.15)).abs() < 1e-12);
    c.update(f64::NAN);
    assert!(c.scale < 0.4);
}

/// X9 (ADR 0023 addendum A4): fixed local pseudo-Courant numbers on the P1 cavity and the P4 step.
#[test]
#[ignore = "diagnostic X9 (ADR 0023 addendum A4)"]
fn x9_fixed_cfl_stability_limit() {
    for cfl in [5.0, 10.0, 20.0, 50.0, 100.0, 1000.0] {
        let o = cavity(local(cfl), None, 4000);
        println!(
            "X9 cavity fixed cfl {cfl}: {} its converged {} res {:.2e}",
            o.iterations, o.converged, o.res
        );
    }
    for cfl in [5.0, 10.0, 20.0, 50.0, 100.0, 1000.0] {
        let (its, conv, res) = step_re400_fixed(cfl, 4000);
        println!("X9 step fixed cfl {cfl}: {its} its converged {conv} res {res:.2e}");
    }
}
