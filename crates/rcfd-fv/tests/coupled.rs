//! Coupled pressure-velocity solver (F-032) against the segregated SIMPLE/SIMPLEC solver.
//!
//! Comparison fixed before running: lid-driven cavity, Re = 100, 40x40, linearUpwind (the F-028
//! configuration), default under-relaxation of both solvers. The algorithm-independent measure is
//! the number of outer iterations after which the velocity stays within 1e-5 of the lid speed of
//! the solver's own converged state (one shared function for both). The coupled solution must pass
//! the unchanged Ghia gate (max abs error <= 0.02 U_lid for u and v).

use rcfd_fv::convection::ConvScheme;
use rcfd_fv::coupled::{CoupledSettings, CoupledSolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::cavity_2d;
use std::path::Path;
use std::sync::Arc;

fn read_csv(name: &str) -> Vec<Vec<f64>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../cases/cavity/reference")
        .join(name);
    std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|v| v.trim().parse().unwrap()).collect())
        .collect()
}

fn lagrange(xs: &[f64], ys: &[f64], x: f64) -> f64 {
    let n = xs.len();
    let mut j = xs.partition_point(|&v| v < x).clamp(1, n - 1);
    j = j.saturating_sub(2).min(n - 4);
    let (px, py) = (&xs[j..j + 4], &ys[j..j + 4]);
    let mut s = 0.0;
    for a in 0..4 {
        let mut l = py[a];
        for b in 0..4 {
            if a != b {
                l *= (x - px[b]) / (px[a] - px[b]);
            }
        }
        s += l;
    }
    s
}

fn centreline(g: &FvGeometry, u: &[[f64; 3]], comp: usize, at: f64) -> f64 {
    let n = (g.n_cells() as f64).sqrt().round() as usize;
    let cc = g.mesh.cell_centres();
    let (vary, fixed) = if comp == 0 { (1, 0) } else { (0, 1) };
    let h = 1.0 / n as f64;
    let mut line = vec![0.0; n];
    let mut count = vec![0.0; n];
    for c in 0..g.n_cells() {
        let along = (cc[c][vary] / h).floor() as usize;
        let w = (1.0 - (cc[c][fixed] - 0.5).abs() / h).max(0.0);
        if w > 0.0 {
            line[along] += w * u[c][comp];
            count[along] += w;
        }
    }
    let mut xs = vec![0.0];
    let mut ys = vec![0.0];
    for i in 0..n {
        xs.push((i as f64 + 0.5) * h);
        ys.push(line[i] / count[i]);
    }
    xs.push(1.0);
    ys.push(if comp == 0 { 1.0 } else { 0.0 });
    lagrange(&xs, &ys, at)
}

fn ghia_errors(g: &FvGeometry, u: &[[f64; 3]]) -> (f64, f64) {
    let u_ref = read_csv("ghia1982_u_centerline.csv");
    let v_ref = read_csv("ghia1982_v_centerline_Re100.csv");
    let eu = u_ref
        .iter()
        .map(|r| (centreline(g, u, 0, r[0]) - r[1]).abs())
        .fold(0.0, f64::max);
    let ev = v_ref
        .iter()
        .map(|r| (centreline(g, u, 1, r[0]) - r[1]).abs())
        .fold(0.0, f64::max);
    (eu, ev)
}

fn setup(n: usize) -> (Arc<FvGeometry>, VectorField, ScalarField) {
    let mesh = Arc::new(cavity_2d(n, 1.0).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    (geom, u, p)
}

/// Shared measure: first iteration after which every snapshot stays within `tol` of the final one.
fn settled_iteration(snaps: &[Vec<[f64; 3]>], tol: f64) -> usize {
    let last = snaps.last().unwrap();
    let err = |s: &Vec<[f64; 3]>| {
        s.iter()
            .zip(last)
            .map(|(a, b)| (0..3).map(|k| (a[k] - b[k]).abs()).fold(0.0, f64::max))
            .fold(0.0, f64::max)
    };
    let mut first = 0;
    for (i, s) in snaps.iter().enumerate() {
        if err(s) > tol {
            first = i + 1;
        }
    }
    first + 1
}

struct Outcome {
    settled: usize,
    total: usize,
    linear: usize,
    secs: f64,
    ghia: (f64, f64),
}

fn run_simple(coupling: Coupling, alpha_u: f64) -> Outcome {
    let (geom, u, p) = setup(40);
    let mut s = SimpleSettings::new(0.01, coupling);
    s.convection = ConvScheme::LinearUpwind;
    s.alpha_u = alpha_u;
    let mut sol = SimpleSolver::new(geom.clone(), u, p, s);
    let t0 = std::time::Instant::now();
    let mut snaps = Vec::new();
    let mut lin = 0;
    for _ in 0..6000 {
        let i = sol.iterate();
        lin += i.p_iters;
        snaps.push(sol.u.values.clone());
        if i.max_du < 1e-9 {
            break;
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    Outcome {
        settled: settled_iteration(&snaps, 1e-5),
        total: snaps.len(),
        linear: lin,
        secs,
        ghia: ghia_errors(&geom, &sol.u.values),
    }
}

fn run_coupled(alpha_u: f64, amg: bool) -> Outcome {
    let (geom, u, p) = setup(40);
    let mut s = CoupledSettings::new(0.01);
    s.convection = ConvScheme::LinearUpwind;
    s.alpha_u = alpha_u;
    s.amg = amg;
    let mut sol = CoupledSolver::new(geom.clone(), u, p, s).unwrap();
    let t0 = std::time::Instant::now();
    let mut snaps = Vec::new();
    for _ in 0..1500 {
        let i = sol.iterate();
        snaps.push(sol.u.values.clone());
        if i.max_du < 1e-9 {
            break;
        }
    }
    let secs = t0.elapsed().as_secs_f64();
    Outcome {
        settled: settled_iteration(&snaps, 1e-5),
        total: snaps.len(),
        linear: sol.linear_iterations,
        secs,
        ghia: ghia_errors(&geom, &sol.u.values),
    }
}

fn show(name: &str, o: &Outcome) {
    println!(
        "{name:<28} settled@{:>5} total {:>5} linear its {:>7} {:>6.1}s  Ghia u {:.5} v {:.5}",
        o.settled, o.total, o.linear, o.secs, o.ghia.0, o.ghia.1
    );
}

#[test]
fn coupled_matches_ghia_and_needs_fewer_outer_iterations_than_simple() {
    let simplec = run_simple(Coupling::Simplec, 0.9);
    let simple = run_simple(Coupling::Simple, 0.7);
    let coupled = run_coupled(0.9, true);
    show("SIMPLEC alpha_u 0.9", &simplec);
    show("SIMPLE alpha_u 0.7", &simple);
    show("coupled alpha_u 0.9 (AMG)", &coupled);
    assert!(coupled.total < 1500, "coupled solver did not converge");
    assert!(
        coupled.ghia.0 <= 0.02 && coupled.ghia.1 <= 0.02,
        "Ghia gate: {:?}",
        coupled.ghia
    );
    assert!(
        coupled.settled < simplec.settled && coupled.settled < simple.settled,
        "coupled {} vs SIMPLEC {} / SIMPLE {}",
        coupled.settled,
        simplec.settled,
        simple.settled
    );
}

#[test]
#[ignore]
fn coupled_variants_report() {
    for a in [0.95, 0.98] {
        show(&format!("SIMPLEC alpha_u {a}"), &run_simple(Coupling::Simplec, a));
    }
    for a in [0.7, 0.9, 0.95, 0.98] {
        show(&format!("coupled alpha_u {a} AMG"), &run_coupled(a, true));
        show(&format!("coupled alpha_u {a} diag"), &run_coupled(a, false));
    }
}

#[test]
fn coupled_rejects_open_domains() {
    let (geom, mut u, p) = setup(8);
    u.set_bc(&geom, "lid", Box::new(rcfd_fv::bc::ZeroGradient)).unwrap();
    assert!(CoupledSolver::new(geom, u, p, CoupledSettings::new(0.01)).is_err());
}
