//! Transient energy equation and buoyancy with PISO/PIMPLE (F-050 gaps).
//!
//! Checks fixed before running:
//! * transient conduction in a slab, `T(0,t) = 1`, `T(x,0) = 0`, alpha = 1, 200 cells over L = 1 (the far end stays at
//!   zero to 1e-6 for `t <= 0.02`), against `erfc(x / (2 sqrt(alpha t)))` at `t = 0.02`: implicit Euler with
//!   `dt = 2e-5` within 1 percent of the jump, BDF2 with `dt = 2e-4` within 0.2 percent, and Euler's error ratio for
//!   a halved step between 1.6 and 2.4 (first order);
//! * the differentially heated cavity at Ra = 1e4 (40x40 graded 4:1, Pr = 0.71, vanLeer) marched with PIMPLE to the
//!   steady state: Nu, u_max, v_max within 2 percent of de Vahl Davis (1983) and the Nu within 1 percent of the
//!   steady SIMPLEC value 2.2434 of the same discretisation; hot and cold wall fluxes balanced to 1e-6;
//! * a stably stratified fluid at rest (T = y, Ra-equivalent forcing 1e4 Pr) keeps a spurious face flux below 2e-3
//!   (the bound of the steady solver's test).

use rcfd_fv::convection::{ConvScheme, TvdLimiter};
use rcfd_fv::ddt::TimeScheme;
use rcfd_fv::energy::{BuoyantPimple, EnergySolver};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::{FixedValue, FvGeometry, ScalarField, VectorField, ZeroGradient};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use std::sync::Arc;

fn erfc(x: f64) -> f64 {
    // series for |x| < 3, continued fraction beyond
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    if x < 3.0 {
        let (mut term, mut sum, mut n) = (x, x, 0.0);
        while term.abs() > 1e-17 {
            n += 1.0;
            term *= -x * x / n;
            sum += term / (2.0 * n + 1.0);
        }
        1.0 - 2.0 / std::f64::consts::PI.sqrt() * sum
    } else {
        let mut f = 0.0;
        for k in (1..60).rev() {
            f = (k as f64 / 2.0) / (x + f);
        }
        (-x * x).exp() / ((x + f) * std::f64::consts::PI.sqrt())
    }
}

fn conduction_error(scheme: TimeScheme, dt: f64) -> f64 {
    let mesh = Arc::new(box_2d_graded(200, 2, 1.0, 0.01, [Grading::Uniform; 2], false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut t = ScalarField::uniform("T", &geom, 0.0);
    t.set_bc(&geom, "xmin", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    t.set_bc(&geom, "ymin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "ymax", Box::new(ZeroGradient)).unwrap();
    let mut e = EnergySolver::new(geom.clone(), t, 1.0);
    e.solver.tol_rel = 1e-12;
    e.solver.tol_abs = 1e-300;
    e.solver.max_iter = 5000;
    let phi = vec![0.0; geom.mesh.n_faces()];
    let steps = (0.02 / dt).round() as usize;
    for _ in 0..steps {
        e.advance(&phi, dt, scheme);
    }
    let cc = geom.mesh.cell_centres();
    (0..cc.len())
        .map(|c| (e.t.values[c] - erfc(cc[c][0] / (2.0 * 0.02f64.sqrt()))).abs())
        .fold(0.0, f64::max)
}

#[test]
fn transient_conduction_matches_the_erfc_solution() {
    let e1 = conduction_error(TimeScheme::ImplicitEuler, 2e-5);
    let e2 = conduction_error(TimeScheme::ImplicitEuler, 1e-5);
    let eb = conduction_error(TimeScheme::Bdf2, 2e-4);
    println!(
        "Euler dt 2e-5: {e1:.3e}, dt 1e-5: {e2:.3e} (ratio {:.2}); BDF2 dt 2e-4: {eb:.3e}",
        e1 / e2
    );
    assert!(e1 < 0.01, "{e1}");
    assert!(eb < 0.002, "{eb}");
    assert!(e1 / e2 > 1.6 && e1 / e2 < 2.4, "ratio {}", e1 / e2);
}

const PR: f64 = 0.71;

fn cavity(ra: f64, n: usize, grading: f64) -> (BuoyantPimple, Arc<FvGeometry>) {
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
    let mut s = PimpleSettings::piso(PR);
    s.convection = ConvScheme::Tvd(TvdLimiter::VanLeer);
    s.n_correctors = 2;
    s.amg_pressure = true;
    let flow = PimpleSolver::new(geom.clone(), u, p, s);
    let mut energy = EnergySolver::new(geom.clone(), t, 1.0);
    energy.convection = ConvScheme::Tvd(TvdLimiter::VanLeer);
    energy.solver.tol_rel = 1e-10;
    energy.solver.tol_abs = 1e-300;
    energy.solver.max_iter = 2000;
    (
        BuoyantPimple {
            flow,
            energy,
            beta: 1.0,
            gravity: [0.0, -ra * PR, 0.0],
            t_ref: 0.5,
        },
        geom,
    )
}

fn wall_nu(sim: &BuoyantPimple, geom: &FvGeometry, patch: &str, sign: f64) -> f64 {
    let gr = sim.energy.wall_gradient(patch);
    let pt = geom.mesh.patch(patch).unwrap();
    let area: f64 = pt.faces().map(|f| geom.mesh.face_area(f)).sum();
    let tot: f64 = pt.faces().zip(&gr).map(|(f, g)| g * geom.mesh.face_area(f)).sum();
    sign * tot / area
}

/// Maximum of component `comp` along the mid line `across = 0.5` (average of the two cell lines next to it).
fn mid_max(sim: &BuoyantPimple, geom: &FvGeometry, comp: usize, across: usize) -> f64 {
    let cc = geom.mesh.cell_centres();
    let along = 1 - across;
    let width = |c: usize| {
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for &f in geom.mesh.cell_faces(c) {
            let x = geom.mesh.face_centres()[f][across];
            lo = lo.min(x);
            hi = hi.max(x);
        }
        hi - lo
    };
    let mut v: Vec<(f64, f64)> = (0..cc.len())
        .filter(|&c| (cc[c][across] - 0.5).abs() <= 0.5 * width(c) + 1e-12)
        .map(|c| (cc[c][along], sim.flow.u.values[c][comp]))
        .collect();
    v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    v.chunks(2).map(|w| 0.5 * (w[0].1 + w[1].1)).fold(0.0, f64::max)
}

#[test]
fn pimple_marches_the_differentially_heated_cavity_to_the_benchmark() {
    let (mut sim, geom) = cavity(1e4, 40, 4.0);
    let dt = 2e-3;
    let mut steps = 0;
    let mut change = f64::INFINITY;
    while steps < 3000 {
        let before = sim.energy.t.values.clone();
        sim.step(dt);
        steps += 1;
        change = before
            .iter()
            .zip(&sim.energy.t.values)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max)
            / dt;
        if change < 1e-7 {
            break;
        }
    }
    let nu_h = wall_nu(&sim, &geom, "xmin", 1.0);
    let nu_c = wall_nu(&sim, &geom, "xmax", -1.0);
    let (um, vm) = (mid_max(&sim, &geom, 0, 0), mid_max(&sim, &geom, 1, 1));
    println!(
        "{steps} steps (t = {:.2}), max dT/dt {change:.1e}: Nu {nu_h:.4} (cold {nu_c:.4}), u_max {um:.3}, v_max {vm:.3}",
        steps as f64 * dt
    );
    assert!(change < 1e-7, "not steady: {change}");
    assert!((nu_h - nu_c).abs() / nu_h < 1e-6, "wall flux imbalance");
    assert!((nu_h / 2.243 - 1.0).abs() < 0.02, "Nu {nu_h}");
    assert!((um / 16.178 - 1.0).abs() < 0.02, "u_max {um}");
    assert!((vm / 19.617 - 1.0).abs() < 0.02, "v_max {vm}");
    assert!((nu_h / 2.2434 - 1.0).abs() < 0.01, "Nu vs steady SIMPLEC {nu_h}");
}

#[test]
fn stratified_fluid_at_rest_stays_at_rest_with_pimple() {
    let mesh = Arc::new(box_2d_graded(12, 12, 1.0, 1.0, [Grading::Uniform; 2], false, false).unwrap());
    let geom = Arc::new(FvGeometry::new(mesh).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for p in ["xmin", "xmax", "ymin", "ymax"] {
        u.set_bc(&geom, p, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut t = ScalarField::uniform("T", &geom, 0.0);
    t.set_bc(&geom, "ymin", Box::new(FixedValue::scalar(0.0))).unwrap();
    t.set_bc(&geom, "ymax", Box::new(FixedValue::scalar(1.0))).unwrap();
    t.set_bc(&geom, "xmin", Box::new(ZeroGradient)).unwrap();
    t.set_bc(&geom, "xmax", Box::new(ZeroGradient)).unwrap();
    for (c, x) in geom.mesh.cell_centres().iter().enumerate() {
        t.values[c] = x[1];
    }
    let flow = PimpleSolver::new(geom.clone(), u, p, PimpleSettings::piso(PR));
    let energy = EnergySolver::new(geom.clone(), t, 1.0);
    let mut sim = BuoyantPimple {
        flow,
        energy,
        beta: 1.0,
        gravity: [0.0, -1.0e4 * PR, 0.0],
        t_ref: 0.5,
    };
    for _ in 0..100 {
        sim.step(0.01);
    }
    let phimax = sim.flow.phi.iter().fold(0.0f64, |m, p| m.max(p.abs()));
    println!("max |phi| {phimax:.2e}");
    assert!(phimax < 2e-3, "spurious face flux {phimax}");
}
