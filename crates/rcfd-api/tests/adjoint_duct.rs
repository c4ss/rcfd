//! ADR 0091 (wave 38D, F-137): adjoint shape sensitivities of a laminar 2D duct against finite differences.

use rcfd_api::design::adjoint::{
    adjoint_gradient, pressure_drop, shape_gradient, solve_primal, wall_drag, AdjointOptions, Duct, Objective,
    Sensitivity,
};
use rcfd_api::design::optim::{projected_gradient, PgOptions, Stop};
use std::time::Instant;

const A0: [f64; 3] = [0.10, 0.20, 0.05];

fn objectives(d: &Duct, a: &[f64]) -> ([f64; 2], usize, f64) {
    let mut s = d.solver(a).unwrap();
    let r = solve_primal(&mut s, 1e-14, 400, 30000).unwrap();
    ([pressure_drop(&s), wall_drag(&s)], r.iterations, r.last_res)
}

/// Set-up smoke (rule d): primal convergence, noise floor and the finite-difference step study of the reference derivative.
#[test]
#[ignore = "set-up smoke of ADR 0091, prints the measurements"]
fn duct_setup_smoke() {
    let d = Duct::default();
    let t = Instant::now();
    let mut s = d.solver(&A0).unwrap();
    let vmin = s.geom.mesh.cell_volumes().iter().cloned().fold(f64::INFINITY, f64::min);
    let r = solve_primal(&mut s, 1e-14, 400, 30000).unwrap();
    println!(
        "base: its {} best {:.3e} last {:.3e} dp {:.10e} drag {:.10e} vmin {:.3e} time {:.2?}",
        r.iterations,
        r.best_res,
        r.last_res,
        pressure_drop(&s),
        wall_drag(&s),
        vmin,
        t.elapsed()
    );
    let snap = s.snapshot();
    // one-iteration map: cost, inner iterations, fixed-point residual
    let t = Instant::now();
    let info = s.iterate();
    let dt = t.elapsed();
    let after = s.snapshot();
    let du = snap
        .u
        .iter()
        .zip(&after.u)
        .map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()))
        .fold(0.0, f64::max);
    let dp = snap
        .p
        .iter()
        .zip(&after.p)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    let dphi = snap
        .phi
        .iter()
        .zip(&after.phi)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max);
    println!(
        "one iterate {dt:.2?} p_iters {} |G(X)-X| u {du:.2e} p {dp:.2e} phi {dphi:.2e}",
        info.p_iters
    );
    let phimax = snap.phi.iter().map(|v| v.abs()).fold(0.0, f64::max);
    let g = s.geom.clone();
    let small = (0..g.n_internal)
        .filter(|&f| snap.phi[f].abs() < 10.0 * 1e-6 * phimax)
        .count();
    println!(
        "internal faces {} with |phi| < 1e-5 max|phi|: {small} (max|phi| {phimax:.3e})",
        g.n_internal
    );
    // inner iteration counts at perturbed states
    for k in [0usize, 100, 399] {
        let mut st = snap.clone();
        st.u[k][0] += 1e-6;
        s.restore(&st).unwrap();
        let i1 = s.iterate();
        st.u[k][0] -= 2e-6;
        s.restore(&st).unwrap();
        let i2 = s.iterate();
        println!("cell {k}: p_iters +h {} -h {}", i1.p_iters, i2.p_iters);
    }
    // noise floor: cold start vs warm start from the base solution at a perturbed shape
    let mut a = A0;
    a[1] += 1e-3;
    let (cold, ic, rc) = objectives(&d, &a);
    let mut w = d.solver(&a).unwrap();
    w.restore(&snap).unwrap();
    let rw = solve_primal(&mut w, 1e-14, 400, 30000).unwrap();
    let warm = [pressure_drop(&w), wall_drag(&w)];
    println!(
        "noise: cold its {ic} res {rc:.2e} warm its {} res {:.2e}; |dJ| dp {:.3e} drag {:.3e}",
        rw.iterations,
        rw.last_res,
        (cold[0] - warm[0]).abs(),
        (cold[1] - warm[1]).abs()
    );
    // FD step study
    for k in 0..3 {
        for h in [1e-3, 2e-3] {
            let (mut ap, mut am) = (A0, A0);
            ap[k] += h;
            am[k] -= h;
            let (jp, _, _) = objectives(&d, &ap);
            let (jm, _, _) = objectives(&d, &am);
            println!(
                "param {k} h {h:e}: D dp {:.10e} D drag {:.10e}",
                (jp[0] - jm[0]) / (2.0 * h),
                (jp[1] - jm[1]) / (2.0 * h)
            );
        }
    }
}

const OBJ: [Objective<'static>; 2] = [&pressure_drop, &wall_drag];
const NAMES: [&str; 2] = ["dp", "Fx"];

/// Adjoint gradient at A0 with the given state step; returns it with its wall time.
fn adjoint_at_a0(d: &Duct, state_step: f64) -> (Sensitivity, f64) {
    let mut s = d.solver(&A0).unwrap();
    let r = solve_primal(&mut s, 1e-14, 400, 30000).unwrap();
    assert!(r.last_res <= 1e-13, "A0: base primal res {:.3e}", r.last_res);
    let opts = AdjointOptions {
        state_step,
        ..Default::default()
    };
    let t = Instant::now();
    let sens = adjoint_gradient(&|a: &[f64]| d.solver(a), &A0, &s.snapshot(), &OBJ, &opts).unwrap();
    (sens, t.elapsed().as_secs_f64())
}

/// A0 + A1 (ADR 0091): the adjoint gradient of dp and Fx with respect to a1..a3 within 5 percent of the central finite
/// difference D(1e-3) of converged solutions, with the reference's preconditions (step study, noise floor) re-measured.
#[test]
fn duct_adjoint_matches_fd() {
    let d = Duct::default();
    let (sens, t_adj) = adjoint_at_a0(&d, 1e-6);
    println!(
        "adjoint: N {} |G(X*)-X*| {:.2e} time {t_adj:.2} s values dp {:.10e} Fx {:.10e}",
        sens.n_state, sens.fixed_point_residual, sens.values[0], sens.values[1]
    );
    let t = Instant::now();
    let mut fd = [[[0.0; 2]; 3]; 2]; // [step][param][objective]
    let mut at_plus_e2 = [0.0; 2];
    for (si, h) in [1e-3, 2e-3].into_iter().enumerate() {
        for k in 0..3 {
            let (mut ap, mut am) = (A0, A0);
            ap[k] += h;
            am[k] -= h;
            let (jp, _, rp) = objectives(&d, &ap);
            let (jm, _, rm) = objectives(&d, &am);
            assert!(rp.max(rm) <= 1e-13, "A0: FD primal res {:.3e}", rp.max(rm));
            if si == 0 && k == 1 {
                at_plus_e2 = jp;
            }
            for o in 0..2 {
                fd[si][k][o] = (jp[o] - jm[o]) / (2.0 * h);
            }
        }
    }
    let t_fd = t.elapsed().as_secs_f64();
    // noise floor: warm start from the base solution at a0 + 1e-3 e2
    let mut b = d.solver(&A0).unwrap();
    solve_primal(&mut b, 1e-14, 400, 30000).unwrap();
    let mut a = A0;
    a[1] += 1e-3;
    let mut w = d.solver(&a).unwrap();
    w.restore(&b.snapshot()).unwrap();
    solve_primal(&mut w, 1e-14, 400, 30000).unwrap();
    let warm = [pressure_drop(&w), wall_drag(&w)];
    println!("FD reference (12 converged solves): {t_fd:.2} s");
    let mut worst: f64 = 0.0;
    for o in 0..2 {
        let noise = (warm[o] - at_plus_e2[o]).abs() / 1e-3;
        for k in 0..3 {
            let (d1, d2, g) = (fd[0][k][o], fd[1][k][o], sens.gradients[o][k]);
            let rel = (g - d1).abs() / d1.abs();
            worst = worst.max(rel);
            println!(
                "{} a{}: adjoint {g:.10e} D(1e-3) {d1:.10e} D(2e-3) {d2:.10e} |adj-D|/|D| {rel:.3e} step study {:.2e} noise {:.2e}",
                NAMES[o],
                k + 1,
                (d1 - d2).abs() / d1.abs(),
                noise / d1.abs()
            );
            assert!((d1 - d2).abs() <= 5e-3 * d1.abs(), "A0 step study");
            assert!(noise <= 1e-3 * d1.abs(), "A0 noise floor");
        }
    }
    println!("A1 worst relative difference {worst:.3e} (limit 0.05)");
    for o in 0..2 {
        for k in 0..3 {
            let (d1, g) = (fd[0][k][o], sens.gradients[o][k]);
            assert!(
                (g - d1).abs() <= 0.05 * d1.abs(),
                "A1 {} a{}: {g} vs {d1}",
                NAMES[o],
                k + 1
            );
        }
    }
}

/// A2 (ADR 0091): the adjoint gradient does not depend on the state step (1e-6 against 1e-5) beyond 1e-4 relative.
#[test]
#[ignore = "ADR 0091 A2 smoothness diagnostic (two adjoint gradients)"]
fn duct_adjoint_state_step_diagnostic() {
    let d = Duct::default();
    let (s6, _) = adjoint_at_a0(&d, 1e-6);
    let (s5, _) = adjoint_at_a0(&d, 1e-5);
    for o in 0..2 {
        for k in 0..3 {
            let (a, b) = (s6.gradients[o][k], s5.gradients[o][k]);
            let rel = (a - b).abs() / a.abs();
            println!(
                "{} a{}: step 1e-6 {a:.12e} step 1e-5 {b:.12e} rel {rel:.3e}",
                NAMES[o],
                k + 1
            );
            assert!(rel <= 1e-4, "A2");
        }
    }
}

/// A3 (ADR 0091): projected steepest descent on dp with the adjoint gradient, bounds [-0.1, 0.25], at most 3 iterations:
/// every accepted iterate lowers dp and the final dp is at most 0.95 dp(a0).
#[test]
fn duct_gradient_descent() {
    let d = Duct::default();
    let build = |a: &[f64]| d.solver(a);
    let mut accepted: Vec<f64> = Vec::new();
    let mut ev = |x: &[f64], grad: bool| -> Result<(f64, Vec<f64>), Stop> {
        if grad {
            let s = shape_gradient(&build, x, &OBJ[..1], &AdjointOptions::default()).map_err(Stop::Error)?;
            println!("iterate {x:?}: dp {:.8e} grad {:?}", s.values[0], s.gradients[0]);
            accepted.push(s.values[0]);
            Ok((s.values[0], s.gradients[0].clone()))
        } else {
            let (j, _, _) = objectives(&d, x);
            println!("trial {x:?}: dp {:.8e}", j[0]);
            Ok((j[0], Vec::new()))
        }
    };
    let o = PgOptions {
        max_iters: 3,
        step: 0.05 / 0.35,
        armijo: 1e-4,
        max_halvings: 10,
        x0: A0.to_vec(),
    };
    let r = projected_gradient(&[(-0.1, 0.25); 3], &o, &mut ev).unwrap();
    println!("result {:?} dp {:.8e} evaluations {}", r.x, r.fitness.f, r.evaluations);
    for w in accepted.windows(2) {
        assert!(w[1] < w[0], "A3: dp did not decrease: {accepted:?}");
    }
    assert!(
        r.fitness.f <= 0.95 * accepted[0],
        "A3: {} vs {}",
        r.fitness.f,
        accepted[0]
    );
}
