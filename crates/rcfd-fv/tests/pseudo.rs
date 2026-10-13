//! Pseudo-transient steady solves (F-106): lid-driven cavity at Re = 1000 and a
//! backward-facing step, compared with plain SIMPLE/SIMPLEC under-relaxation.

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::simple::{Coupling, PseudoMode, PseudoTransient, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{backward_step, cavity_2d, StepSpec};
use std::sync::Arc;

#[derive(Clone, Copy)]
struct Setup {
    coupling: Coupling,
    alpha_u: f64,
    alpha_p: f64,
    pseudo: Option<PseudoTransient>,
    /// H operator from the solved predictor (the default since wave 6) or from the previous iterate.
    h_sol: bool,
}

/// Outcome of one run.
struct Out {
    iterations: usize,
    converged: bool,
    res_u: f64,
    solver: SimpleSolver,
}

fn run(solver: &mut SimpleSolver, max_iter: usize, tol: f64) -> (usize, bool, f64) {
    let mut last = f64::INFINITY;
    for i in 1..=max_iter {
        let info = solver.iterate();
        last = info.res_u.max(info.res_p);
        let finite = solver.u.values.iter().all(|v| v.iter().all(|x| x.is_finite()));
        if !finite || !last.is_finite() || last > 1e8 {
            return (i, false, last);
        }
        if last < tol {
            return (i, true, last);
        }
    }
    (max_iter, false, last)
}

fn settings(nu: f64, s: Setup, scheme: ConvScheme) -> SimpleSettings {
    let mut st = SimpleSettings::new(nu, s.coupling);
    st.alpha_u = s.alpha_u;
    st.alpha_p = s.alpha_p;
    st.convection = scheme;
    st.h_from_solution = s.h_sol;
    st
}

fn cavity(re: f64, n: usize, s: Setup, scheme: ConvScheme, max_iter: usize, tol: f64) -> Out {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(n, 1.0).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut solver = SimpleSolver::new(geom, u, p, settings(1.0 / re, s, scheme));
    solver.pseudo = s.pseudo;
    let (iterations, converged, res_u) = run(&mut solver, max_iter, tol);
    Out {
        iterations,
        converged,
        res_u,
        solver,
    }
}

fn step_case(re_h: f64, s: Setup, scheme: ConvScheme, max_iter: usize, tol: f64) -> Out {
    // Backward-facing step, expansion ratio 2 (inlet height 1, step 1), inlet U = 1 (parabolic is not needed
    // for the robustness comparison; uniform inlet is used), Re_h = U h / nu with h the step height.
    let spec = StepSpec {
        l_in: 4.0,
        l_out: 20.0,
        h_in: 1.0,
        step: 1.0,
        depth: 0.1,
        n: [16, 80, 8, 8],
    };
    let geom = Arc::new(FvGeometry::new(Arc::new(backward_step(spec).unwrap())).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "inlet", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    u.set_bc(&geom, "outlet", Box::new(ZeroGradient)).unwrap();
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    p.set_bc(&geom, "outlet", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut solver = SimpleSolver::new(geom, u, p, settings(1.0 / re_h, s, scheme));
    solver.pseudo = s.pseudo;
    let (iterations, converged, res_u) = run(&mut solver, max_iter, tol);
    Out {
        iterations,
        converged,
        res_u,
        solver,
    }
}

fn local(cfl: f64) -> Option<PseudoTransient> {
    Some(PseudoTransient {
        mode: PseudoMode::Local { cfl },
        factor: 1.0,
    })
}

fn simplec(alpha_u: f64, pseudo: Option<PseudoTransient>) -> Setup {
    Setup {
        coupling: Coupling::Simplec,
        alpha_u,
        alpha_p: 1.0,
        pseudo,
        h_sol: true,
    }
}

fn simple_low() -> Setup {
    Setup {
        coupling: Coupling::Simple,
        alpha_u: 0.7,
        alpha_p: 0.3,
        pseudo: None,
        h_sol: true,
    }
}

/// Cavity at Re = 1000 on 32x32 cells with linear-upwind convection. Plain SIMPLEC
/// with weak momentum relaxation (0.9 .. 1.0) and the legacy lagged-H operator diverges
/// (measured in wave 5: no convergence in 5300/4150/3300 iterations, residual ~0.7); with H from the
/// solved predictor it converges, as does every pseudo-transient mode at alpha_u = 1 and classic
/// SIMPLE 0.7/0.3.
#[test]
fn cavity_re1000_pseudo_vs_plain_relaxation() {
    let sch = ConvScheme::LinearUpwind;
    let base = cavity(1000.0, 32, simple_low(), sch, 6000, 1e-6);
    println!("SIMPLE 0.7/0.3: {} its converged {}", base.iterations, base.converged);
    assert!(base.converged);
    let plain = cavity(1000.0, 32, simplec(0.95, None), sch, 6000, 1e-6);
    println!(
        "plain SIMPLEC 0.95: {} its converged {}",
        plain.iterations, plain.converged
    );
    // Wave 6 diagnosis: that failure came from the lagged (previous-iterate) H operator. With H
    // from the solved predictor (now the default) plain SIMPLEC 0.95 converges as well.
    assert!(plain.converged);
    let legacy = Setup {
        h_sol: false,
        ..simplec(0.95, None)
    };
    let old = cavity(1000.0, 32, legacy, sch, 6000, 1e-6);
    println!(
        "legacy lagged-H SIMPLEC 0.95: {} its converged {}",
        old.iterations, old.converged
    );
    assert!(!old.converged);
    let modes = [
        ("local cfl 5", PseudoMode::Local { cfl: 5.0 }),
        ("physical dt 0.2", PseudoMode::Physical { dt: 0.2 }),
        ("auto", PseudoMode::Auto),
    ];
    let mut umins = vec![];
    for (name, mode) in modes {
        let p = Some(PseudoTransient { mode, factor: 1.0 });
        let o = cavity(1000.0, 32, simplec(1.0, p), sch, 8000, 1e-6);
        println!("pseudo {name}: {} its converged {}", o.iterations, o.converged);
        assert!(o.converged, "pseudo {name} did not converge");
        umins.push(o.solver.u.values.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min));
    }
    let r = base.solver.u.values.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min);
    for u in umins {
        // all solves reach the same steady state (to the 1e-6 residual level)
        assert!((u - r).abs() < 2e-3, "u_min {u} vs {r}");
    }
}

/// The pseudo time-scale factor stretches the step: a larger factor at the same
/// CFL is the same as a larger CFL, so a factor of 0.4 on cfl 5 must equal cfl 2.
#[test]
fn factor_scales_the_pseudo_step() {
    let geom = Arc::new(FvGeometry::new(Arc::new(cavity_2d(8, 1.0).unwrap())).unwrap());
    let u = VectorField::uniform("U", &geom, [0.3, 0.1, 0.0]);
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut a = SimpleSolver::new(
        geom.clone(),
        u.clone(),
        p.clone(),
        SimpleSettings::new(1e-3, Coupling::Simplec),
    );
    let mut b = SimpleSolver::new(geom, u, p, SimpleSettings::new(1e-3, Coupling::Simplec));
    a.pseudo = Some(PseudoTransient {
        mode: PseudoMode::Local { cfl: 5.0 },
        factor: 0.4,
    });
    b.pseudo = local(2.0);
    for _ in 0..3 {
        a.iterate();
        b.iterate();
    }
    let (da, db) = (a.pseudo_time_steps().unwrap(), b.pseudo_time_steps().unwrap());
    for (x, y) in da.iter().zip(&db) {
        assert!((x - y).abs() <= 1e-12 * y.abs());
    }
    assert!(SimpleSolver::new(
        a.geom.clone(),
        a.u.clone(),
        a.p.clone(),
        SimpleSettings::new(1e-3, Coupling::Simplec)
    )
    .pseudo_time_steps()
    .is_none());
}

/// Backward-facing step (expansion ratio 2) at Re_h = 100: plain and pseudo-transient
/// runs all converge (about 560-790 iterations on 16+80 x 8+8 cells). This is only
/// a working check, not a robustness demonstration (see the roadmap note on
/// Re_h >= 200 with second-order schemes).
#[test]
fn step_re100_converges_with_pseudo() {
    let sch = ConvScheme::Tvd(TvdLimiter::VanLeer);
    let base = step_case(100.0, simple_low(), sch, 3000, 1e-6);
    let ps = step_case(100.0, simplec(1.0, local(5.0)), sch, 3000, 1e-6);
    println!(
        "BFS Re100: SIMPLE {} its; pseudo {} its",
        base.iterations, ps.iterations
    );
    assert!(base.converged && ps.converged);
}

#[test]
#[ignore = "exploration"]
fn explore_step() {
    let list = [
        ("SIMPLE 0.7/0.3", simple_low()),
        ("SIMPLEC 0.9", simplec(0.9, None)),
        ("pseudo local 5", simplec(1.0, local(5.0))),
        ("pseudo local 20", simplec(1.0, local(20.0))),
    ];
    for (sn, sch) in [("upwind", ConvScheme::Upwind), ("linUp", ConvScheme::LinearUpwind)] {
        for (label, s, re) in list.iter().flat_map(|(l, s)| [(*l, *s, 200.0), (*l, *s, 400.0)]) {
            let o = step_case(re, s, sch, 4000, 1e-6);
            println!(
                "BFS {sn} Re_h={re} {label:18}: its {:5} conv {} res {:.2e}",
                o.iterations, o.converged, o.res_u
            );
        }
    }
}
