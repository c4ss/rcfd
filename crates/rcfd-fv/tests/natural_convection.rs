//! Differentially heated square cavity (F-050), validated against the
//! benchmark solution of de Vahl Davis, Int. J. Numer. Meth. Fluids 3 (1983)
//! 249-264 (Pr = 0.71, hot left wall, cold right wall, adiabatic top and bottom).
//!
//! NOTE: the reference values below (also in cases/heated_cavity/reference) were
//! originally typed from memory; they have since been checked against published
//! tables that quote the benchmark (Nu_avg, u_max, v_max by the MOOSE/Cardinal
//! V&V page; Nu_max and the Ra = 1e6 entries by a DUGKS paper, arXiv:1412.2866).
//! All twelve test values match. See docs/reference-data.md.
//!
//! Non-dimensionalisation: H = 1, alpha = 1 (so velocities are in units of
//! alpha/H), nu = Pr, T_hot = 1, T_cold = 0, beta = 1, |g| = Ra * Pr.

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::energy::{BuoyantSimple, EnergySolver};
use rcfd_fv::simple::{Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use std::sync::Arc;

const PR: f64 = 0.71;

/// (Ra, Nu_avg, u_max on x = 0.5, v_max on y = 0.5) from de Vahl Davis (1983).
const REFERENCE: [(f64, f64, f64, f64); 4] = [
    (1.0e3, 1.118, 3.649, 3.697),
    (1.0e4, 2.243, 16.178, 19.617),
    (1.0e5, 4.519, 34.73, 68.59),
    (1.0e6, 8.800, 64.63, 219.36),
];

struct Outcome {
    nu_avg: f64,
    nu_cold: f64,
    u_max: f64,
    v_max: f64,
    iterations: usize,
    res: f64,
    energy_imbalance: f64,
}

fn solve(ra: f64, n: usize, grading: f64, max_iter: usize) -> Outcome {
    let g = if grading > 1.0 {
        Grading::Symmetric(grading)
    } else {
        Grading::Uniform
    };
    let mesh = Arc::new(box_2d_graded(n, n, 1.0, 1.0, [g, g], false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut t = ScalarField::uniform("T", &geom, 0.5);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut s = SimpleSettings::new(PR, Coupling::Simplec);
    s.convection = ConvScheme::Tvd(TvdLimiter::VanLeer);
    s.alpha_u = 0.8;
    let flow = SimpleSolver::new(geom.clone(), u, p, s);
    let mut energy = EnergySolver::new(geom.clone(), t, 1.0);
    energy.convection = ConvScheme::Tvd(TvdLimiter::VanLeer);
    energy.relax = 0.9;
    let mut sim = BuoyantSimple {
        flow,
        energy,
        beta: 1.0,
        gravity: [0.0, -ra * PR, 0.0],
        t_ref: 0.5,
    };
    let mut res = f64::INFINITY;
    let mut it = 0;
    while it < max_iter {
        let info = sim.iterate();
        it += 1;
        res = info.flow.res_u.max(info.res_t);
        if res < 1e-9 {
            break;
        }
    }
    // Wall Nusselt numbers from the discrete wall flux (fixed-value faces): Nu = mean(dT/dn_out)
    let nu_wall = |patch: &str, sign: f64| {
        let gr = sim.energy.wall_gradient(patch);
        let mesh = &geom.mesh;
        let pt = mesh.patch(patch).unwrap();
        let area: f64 = pt.faces().map(|f| mesh.face_area(f)).sum();
        let tot: f64 = pt.faces().zip(&gr).map(|(f, g)| g * mesh.face_area(f)).sum();
        sign * tot / area
    };
    let nu_avg = nu_wall("xmin", 1.0);
    let nu_cold = nu_wall("xmax", -1.0);
    // mid-plane velocity maxima (average of the two cell columns/rows adjacent to the mid-plane)
    let cc = geom.mesh.cell_centres();
    let (mut u_max, mut v_max) = (0.0f64, 0.0f64);
    let sample = |comp: usize, across: usize| -> Vec<f64> {
        // values along the varying axis, averaged over the two cell lines next to 0.5
        let along = 1 - across;
        let mut v: Vec<(f64, f64)> = (0..cc.len())
            .filter(|&c| (cc[c][across] - 0.5).abs() <= 0.5 * cell_width(&geom, c, across) + 1e-12)
            .map(|c| (cc[c][along], sim.flow.u.values[c][comp]))
            .collect();
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        v.chunks(2).map(|w| 0.5 * (w[0].1 + w[1].1)).collect()
    };
    for x in sample(0, 0) {
        u_max = u_max.max(x);
    }
    for x in sample(1, 1) {
        v_max = v_max.max(x);
    }
    Outcome {
        nu_avg,
        nu_cold,
        u_max,
        v_max,
        iterations: it,
        res,
        energy_imbalance: (nu_avg - nu_cold).abs() / nu_avg,
    }
}

/// Width of cell `c` along axis `axis` (cells are axis-aligned boxes).
fn cell_width(geom: &FvGeometry, c: usize, axis: usize) -> f64 {
    let mesh = &geom.mesh;
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &f in mesh.cell_faces(c) {
        let x = mesh.face_centres()[f][axis];
        lo = lo.min(x);
        hi = hi.max(x);
    }
    hi - lo
}

fn report(ra: f64, n: usize, grading: f64, o: &Outcome) {
    let r = REFERENCE.iter().find(|r| r.0 == ra).unwrap();
    println!(
        "Ra={ra:.0e} n={n} grade={grading}: {} its res {:.1e} | Nu {:.4} (ref {}, {:+.2}%) cold {:.4} | umax {:.3} (ref {}, {:+.2}%) | vmax {:.3} (ref {}, {:+.2}%) | wall-flux imbalance {:.1e}",
        o.iterations,
        o.res,
        o.nu_avg,
        r.1,
        100.0 * (o.nu_avg / r.1 - 1.0),
        o.nu_cold,
        o.u_max,
        r.2,
        100.0 * (o.u_max / r.2 - 1.0),
        o.v_max,
        r.3,
        100.0 * (o.v_max / r.3 - 1.0),
        o.energy_imbalance
    );
}

fn check(ra: f64, n: usize, grading: f64, tol: f64) {
    let o = solve(ra, n, grading, 12000);
    report(ra, n, grading, &o);
    let r = REFERENCE.iter().find(|r| r.0 == ra).unwrap();
    assert!(o.res < 1e-9, "not converged: {}", o.res);
    assert!(
        o.energy_imbalance < 1e-6,
        "hot/cold wall flux imbalance {}",
        o.energy_imbalance
    );
    assert!((o.nu_avg / r.1 - 1.0).abs() < tol, "Nu {} vs {}", o.nu_avg, r.1);
    assert!((o.u_max / r.2 - 1.0).abs() < tol, "u_max {} vs {}", o.u_max, r.2);
    assert!((o.v_max / r.3 - 1.0).abs() < tol, "v_max {} vs {}", o.v_max, r.3);
}

/// Ra = 1e3 on 32x32 (graded 4:1): Nu -0.11%, u_max -0.73%, v_max -0.52%.
#[test]
fn de_vahl_davis_ra_1e3() {
    check(1e3, 32, 4.0, 0.02);
}

/// Ra = 1e4 on 40x40 (graded 4:1): Nu +0.02%, u_max -0.09%, v_max -0.51%.
#[test]
fn de_vahl_davis_ra_1e4() {
    check(1e4, 40, 4.0, 0.02);
}

/// Ra = 1e5 on 48x48 (graded 6:1): Nu +0.08%, u_max +0.30%, v_max -0.59%.
#[test]
fn de_vahl_davis_ra_1e5() {
    check(1e5, 48, 6.0, 0.02);
}

/// Ra = 1e6 on 64x64 (graded 8:1): Nu +0.44%, u_max +0.32%, v_max +0.46%.
#[test]
fn de_vahl_davis_ra_1e6() {
    check(1e6, 64, 8.0, 0.02);
}

/// A stably stratified fluid at rest (T = y, g large) produces almost no face flux (balanced-force buoyancy with wall pressure extrapolation).
#[test]
fn stably_stratified_fluid_stays_at_rest() {
    let mesh = Arc::new(box_2d_graded(12, 12, 1.0, 1.0, [Grading::Uniform; 2], false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut t = ScalarField::uniform("T", &geom, 0.0);
    // hot on top, cold at the bottom: linear conduction profile T = y
    t.set_bc(&geom, "ymin", Box::new(FixedValue::scalar(0.0))).unwrap();
    t.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(&geom, "xmin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    for (c, x) in geom.mesh.cell_centres().iter().enumerate() {
        t.values[c] = x[1];
    }
    let s = SimpleSettings::new(PR, Coupling::Simplec);
    let flow = SimpleSolver::new(geom.clone(), u, p, s);
    let energy = EnergySolver::new(geom.clone(), t, 1.0);
    let mut sim = BuoyantSimple {
        flow,
        energy,
        beta: 1.0,
        gravity: [0.0, -1.0e4 * PR, 0.0],
        t_ref: 0.5,
    };
    for _ in 0..500 {
        sim.iterate();
    }
    // The hydrostatic pressure balances the buoyancy: the spurious mass flux is ~1e-3 (2 orders below the
    // 0.54 measured without the fixed-flux wall pressure extrapolation), against forcing of order 3e3
    let phimax = sim.flow.phi.iter().fold(0.0f64, |m, p| m.max(p.abs()));
    println!("max |phi| {phimax:.2e}");
    assert!(phimax < 2e-3, "spurious face flux {phimax}");
}

#[test]
#[ignore = "exploration"]
fn explore() {
    for (ra, n, gr) in [(1e6, 64, 8.0), (1e6, 96, 8.0)] {
        let o = solve(ra, n, gr, 20000);
        report(ra, n, gr, &o);
    }
}
