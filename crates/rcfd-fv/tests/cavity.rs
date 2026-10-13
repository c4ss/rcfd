//! Lid-driven cavity validation against Ghia, Ghia & Shin (1982).
//!
//! Acceptance (F-028): absolute error of the centreline velocity at the
//! tabulated Ghia points is at most 2 percent of the lid speed.
//! Re = 100 runs in the normal test suite; Re = 400 and 1000 are heavier and
//! are marked `#[ignore]` (run with `cargo test --release -- --ignored`).

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::cavity_2d;
use std::path::Path;
use std::sync::Arc;

fn read_csv(name: &str) -> Vec<Vec<f64>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../cases/cavity/reference")
        .join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').map(|v| v.trim().parse().unwrap()).collect())
        .collect()
}

/// 4-point Lagrange interpolation on sorted abscissae.
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

struct Result {
    solver: SimpleSolver,
    iterations: usize,
    converged: bool,
}

fn solve_cavity(re: f64, n: usize, coupling: Coupling, scheme: ConvScheme, max_iter: usize) -> Result {
    let mesh = Arc::new(cavity_2d(n, 1.0).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    u.set_bc(&geom, "lid", Box::new(FixedValue::vector([1.0, 0.0, 0.0])))
        .unwrap();
    u.set_bc(&geom, "walls", Box::new(FixedValue::vector([0.0; 3])))
        .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(1.0 / re, coupling);
    s.convection = scheme;
    let mut solver = SimpleSolver::new(geom, u, p, s);
    let t0 = std::time::Instant::now();
    let sum = solver.run(max_iter, 1e-7);
    println!(
        "cavity Re={re} n={n} {coupling:?} {scheme:?}: {} iterations, converged={}, max_du={:.2e}, continuity={:.2e}, {:.1}s",
        sum.iterations,
        sum.converged,
        sum.last.max_du,
        sum.last.continuity,
        t0.elapsed().as_secs_f64()
    );
    Result {
        solver,
        iterations: sum.iterations,
        converged: sum.converged,
    }
}

/// Velocity component `comp` sampled on the centreline `x = 0.5` (comp 0, as function of y)
/// or `y = 0.5` (comp 1, as function of x).
fn centreline(solver: &SimpleSolver, comp: usize, at: f64) -> f64 {
    let g = &solver.geom;
    let n = (g.n_cells() as f64).sqrt().round() as usize;
    let cc = g.mesh.cell_centres();
    // line of cell values along the varying axis, interpolated to 0.5 across it
    let (vary, fixed) = if comp == 0 { (1, 0) } else { (0, 1) };
    let mut line = vec![0.0; n];
    let mut count = vec![0.0; n];
    let h = 1.0 / n as f64;
    for c in 0..g.n_cells() {
        let along = (cc[c][vary] / h).floor() as usize;
        let across = cc[c][fixed];
        // linear weight to x = 0.5 from the two columns bracketing it
        let w = (1.0 - (across - 0.5).abs() / h).max(0.0);
        if w > 0.0 {
            line[along] += w * solver.u.values[c][comp];
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

fn max_error(solver: &SimpleSolver, comp: usize, rows: &[Vec<f64>], col: usize) -> (f64, f64) {
    let mut worst = (0.0f64, 0.0);
    for r in rows {
        let (pos, reference) = (r[0], r[col]);
        let e = (centreline(solver, comp, pos) - reference).abs();
        if e > worst.0 {
            worst = (e, pos);
        }
    }
    worst
}

fn check(re: f64, col: usize, n: usize, coupling: Coupling, scheme: ConvScheme, max_iter: usize, tol: f64) {
    let r = solve_cavity(re, n, coupling, scheme, max_iter);
    assert!(r.converged, "did not converge in {} iterations", r.iterations);
    let u_ref = read_csv("ghia1982_u_centerline.csv");
    let (eu, at) = max_error(&r.solver, 0, &u_ref, col);
    println!("  Re={re}: max |u - Ghia| = {eu:.5} (at y={at}); 2 percent of lid speed = 0.02");
    let mut ev = None;
    if re == 100.0 {
        let v_ref = read_csv("ghia1982_v_centerline_Re100.csv");
        let (e, at) = max_error(&r.solver, 1, &v_ref, 1);
        println!("  Re={re}: max |v - Ghia| = {e:.5} (at x={at})");
        ev = Some(e);
    }
    assert!(eu <= tol, "u error {eu} exceeds {tol}");
    if let Some(e) = ev {
        assert!(e <= tol, "v error {e} exceeds {tol}");
    }
}

#[test]
fn cavity_re100_simple_second_order() {
    check(100.0, 1, 40, Coupling::Simple, ConvScheme::LinearUpwind, 3000, 0.02);
}

#[test]
fn cavity_re100_simplec() {
    check(100.0, 1, 40, Coupling::Simplec, ConvScheme::LinearUpwind, 3000, 0.02);
}

#[test]
#[ignore = "heavy: run with cargo test --release -- --ignored"]
fn cavity_re400() {
    check(400.0, 2, 80, Coupling::Simplec, ConvScheme::LinearUpwind, 6000, 0.02);
}

#[test]
#[ignore = "heavy: run with cargo test --release -- --ignored"]
fn cavity_re1000() {
    check(
        1000.0,
        3,
        128,
        Coupling::Simplec,
        ConvScheme::Tvd(TvdLimiter::VanLeer),
        12000,
        0.02,
    );
}
