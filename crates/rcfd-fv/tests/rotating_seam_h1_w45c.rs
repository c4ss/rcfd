//! Wave 45C (ADR 0117): the ADR 0113 seam-coefficient fix F1 (`coupled_offdiag_in_h1`) with a `rotating_frame` body force and the
//! pair mean on, the combination the FAN-01 path uses and ADR 0113 never ran. The 44C K3 geometry: a 60-degree bent-box sector (8 x 8 x
//! 12) against the full annulus (48 x 8 x 12), axial through-flow, walls at rest in the rotating frame, Omega = 5 about +z with the
//! centrifugal term. R1 (sector with F1 against the annulus), negative control N1 (without F1). Limits registered in ADR 0117 before
//! this code ran.

use rcfd_fv::bc::{FixedValue, ZeroGradient};
use rcfd_fv::simple::{BodyForce, Coupling, IterationInfo, SimpleSettings, SimpleSolver};
use rcfd_fv::sources::RotatingFrame;
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{annulus_3d, box_mesh, warp, Grading};
use rcfd_mesh::Mesh;
use std::f64::consts::FRAC_PI_3;
use std::sync::Arc;

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const NU: f64 = 0.1;
const OMEGA: f64 = 5.0;
/// L_DIFF tier: the smallest frame-change difference of ADR 0008 M3 (the ADR 0113 basis), as a fraction of U = 1.
const L_DIFF: f64 = 4.3e-4;
/// F1-exactness tier (the ADR 0113 F1 falsifier level).
const EXACT: f64 = 1e-8;

/// The 44C K3 sector: patches xmin = inner, xmax = outer, ymin / ymax = the cyclic pair, zmin inlet, zmax outlet.
fn sector() -> Mesh {
    let b = box_mesh([8, 8, 12], [1.0, 1.0, 1.2], [Grading::Uniform; 3]).unwrap();
    let mut m = warp(&b, |p| {
        let (r, th) = (RI + p[0] * (RO - RI), p[1] * FRAC_PI_3);
        [r * th.cos(), r * th.sin(), p[2]]
    })
    .unwrap();
    m.set_periodic_rotation("ymin", "ymax", [0.0; 3], [0.0, 0.0, 1.0], FRAC_PI_3)
        .unwrap();
    m
}

fn annulus() -> Mesh {
    annulus_3d(48, 8, 12, RI, RO, 1.2, 0.0, Grading::Uniform).unwrap()
}

struct Run {
    solver: SimpleSolver,
    converged: bool,
    last: IterationInfo,
}

/// Steady SIMPLEC (the ADR 0113 C2b template: alpha_u 0.8, alpha_p 1, central convection) with the `rotating_frame` source wired as
/// `sim.rs` wires it in every step: the acceleration evaluated on the current velocity before each iteration, `wall_pressure_trapezoid`
/// on (a force is present), `coupled_body_pair_mean` = `pair_mean` (the API sets it equal to the trapezoid flag), no component-diagonal
/// correction (no frame flux on this path), F1 = `f1`. Converged when res_u < 1e-10 and continuity < 1e-9 (cap `iters`).
fn solve(mesh: Mesh, walls: [&str; 2], f1: bool, pair_mean: bool, iters: usize) -> Run {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0, 0.0, 1.0]);
    let mut p = ScalarField::uniform("p", &geom, 0.0);
    for w in walls {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    u.set_bc(&geom, "zmin", Box::new(FixedValue::vector([0.0, 0.0, 1.0])))
        .unwrap();
    u.set_bc(&geom, "zmax", Box::new(ZeroGradient)).unwrap();
    p.set_bc(&geom, "zmax", Box::new(FixedValue::scalar(0.0))).unwrap();
    let mut s = SimpleSettings::new(NU, Coupling::Simplec);
    s.alpha_u = 0.8;
    s.alpha_p = 1.0;
    s.convection = rcfd_fv::convection::ConvScheme::Central;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    let frame = RotatingFrame {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        omega: OMEGA,
        centrifugal: true,
    };
    let zone = vec![true; geom.n_cells()];
    let mut last = None;
    let mut converged = false;
    for _ in 0..iters {
        let ub = solver.u.boundary_values(&geom);
        let mut f = BodyForce::uniform(&geom, [0.0; 3]);
        frame.add_force(&geom, &solver.u.values, &ub, &zone, &mut f);
        solver.body_force = Some(f);
        solver.wall_pressure_trapezoid = true;
        solver.coupled_body_pair_mean = pair_mean;
        solver.coupled_offdiag_in_h1 = f1;
        let it = solver.iterate();
        let done = it.res_u < 1e-10 && it.continuity < 1e-9;
        last = Some(it);
        if done {
            converged = true;
            break;
        }
    }
    Run {
        solver,
        converged,
        last: last.unwrap(),
    }
}

fn sector_run(f1: bool, pair_mean: bool) -> Run {
    solve(sector(), ["xmin", "xmax"], f1, pair_mean, 4000)
}

fn annulus_run() -> Run {
    solve(annulus(), ["inner", "outer"], false, true, 4000)
}

/// max |u_a - u_b| over the cells of `a` and the cells of `b` at the same centre (within 1e-9).
fn difference(a: &Run, b: &Run) -> f64 {
    let (ca, cb) = (a.solver.geom.mesh.cell_centres(), b.solver.geom.mesh.cell_centres());
    let d = |x: [f64; 3], y: [f64; 3]| rcfd_core::vec3::norm(rcfd_core::vec3::sub(x, y));
    let mut worst: f64 = 0.0;
    for (i, x) in ca.iter().enumerate() {
        let j = (0..cb.len())
            .min_by(|&i, &j| d(cb[i], *x).total_cmp(&d(cb[j], *x)))
            .unwrap();
        assert!(d(cb[j], *x) <= 1e-9, "no matching cell");
        for k in 0..3 {
            let e = (a.solver.u.values[i][k] - b.solver.u.values[j][k]).abs();
            worst = if e.is_nan() { f64::NAN } else { worst.max(e) };
        }
    }
    worst
}

/// |sum phi(ymin) + sum phi(ymax)| / A(ymin), as a fraction of U = 1.
fn seam_balance(r: &Run) -> f64 {
    let (m, phi) = (&r.solver.geom.mesh, &r.solver.phi);
    let s = |n: &str| m.patch(n).unwrap().faces().map(|f| phi[f]).sum::<f64>();
    let area: f64 = m.patch("ymin").unwrap().faces().map(|f| m.face_area(f)).sum();
    (s("ymin") + s("ymax")).abs() / area
}

fn report(name: &str, r: &Run) {
    println!(
        "{name}: converged {} at iteration {} (res_u {:.2e}, continuity {:.2e})",
        r.converged, r.last.iteration, r.last.res_u, r.last.continuity
    );
}

/// N1 (negative control, run first): R1 without F1. Registered: it must miss the exactness tier by at least 10x (>= 1e-7 U);
/// whether it also misses the L_DIFF tier by 10x (>= 4.3e-3 U) is reported (otherwise that tier is non-discriminating).
#[test]
fn n1_rotating_sector_without_f1_differs_from_the_annulus() {
    let full = annulus_run();
    let s = sector_run(false, true);
    report("N1 annulus", &full);
    report("N1 sector without F1", &s);
    let d = difference(&s, &full);
    println!(
        "N1: sector without F1 - annulus {d:.3e} U; exactness tier discriminates (>= 1e-7) {}; L_DIFF tier discriminates (>= 4.3e-3) {}",
        d >= 10.0 * EXACT,
        d >= 10.0 * L_DIFF
    );
    assert!(d >= 10.0 * EXACT || !d.is_finite());
}

/// R1 / R1-exact: the sector with F1 (pair mean on, as the API sets it) against the annulus. Limits: both runs converged; sector -
/// annulus <= 1e-8 U (R1-exact; it implies the L_DIFF tier 4.3e-4 U); seam balance <= 1e-8 U. Reported, no limit: F1 with the pair
/// mean off against the annulus.
#[test]
fn r1_rotating_sector_with_f1_equals_the_annulus() {
    let full = annulus_run();
    let s = sector_run(true, true);
    report("R1 annulus", &full);
    report("R1 sector with F1", &s);
    let d = difference(&s, &full);
    let sb = seam_balance(&s);
    println!(
        "R1: sector with F1 - annulus {d:.3e} U (R1-exact limit {EXACT:.1e}; L_DIFF tier {L_DIFF:.1e}: {}); seam balance {sb:.3e} U (limit 1e-8)",
        if d <= L_DIFF { "pass" } else { "MISS" }
    );
    let off = sector_run(true, false);
    report("R1 reported, sector with F1 and pair mean off", &off);
    println!(
        "R1 reported: F1 with the pair mean off - annulus {:.3e} U, seam balance {:.3e} U",
        difference(&off, &full),
        seam_balance(&off)
    );
    let mut fail = Vec::new();
    if !(full.converged && s.converged) {
        fail.push("not converged".to_string());
    }
    if !(d <= EXACT) {
        fail.push(format!("sector - annulus {d:.3e} > {EXACT:.1e}"));
    }
    if !(sb <= 1e-8) {
        fail.push(format!("seam balance {sb:.3e} > 1e-8"));
    }
    assert!(fail.is_empty(), "{fail:?}");
}

/// Rule (d) smoke of ADR 0117 (run on demand): the annulus arm to convergence (it has no cyclic faces, so it is the same with and
/// without F1), and 30 iterations of each sector arm (finite, continuity). No sector-annulus difference is computed.
#[test]
#[ignore = "smoke of ADR 0117; run on demand"]
fn smoke_w45c_rotating_sector_arms() {
    report("smoke annulus", &annulus_run());
    for (f1, pm) in [(false, true), (true, true), (true, false)] {
        let r = solve(sector(), ["xmin", "xmax"], f1, pm, 30);
        let finite = r.solver.u.values.iter().all(|v| v.iter().all(|x| x.is_finite()));
        println!(
            "smoke sector F1 {f1} pair mean {pm}: finite {finite}, res_u {:.2e}, continuity {:.2e}",
            r.last.res_u, r.last.continuity
        );
    }
}
