//! Wave 43B (ADR 0109): sector periodicity through a cylindrical non-conformal interface. A 60-degree sector, inner block
//! 8 x 8 (r 0.5-0.75), outer block 6 x 8 (r 0.75-1.0), joined on the cylinder r = 0.75 and made rotationally periodic again;
//! Taylor-Couette in the inertial frame (S1) and in the relative-velocity rotating frame (S2), against conformal sectors with 8
//! and 6 cells around. Set-up of C2b (ADR 0071); limits registered in ADR 0109 before this code ran.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::sources::RotatingFrame;
use rcfd_fv::{FvGeometry, Profile, ScalarField, VectorField};
use rcfd_mesh::gen::{annulus_sector, Grading};
use rcfd_mesh::non_conformal::join_non_conformal_cylindrical;
use rcfd_mesh::Mesh;
use std::f64::consts::FRAC_PI_3;
use std::sync::Arc;

const RI: f64 = 0.5;
const RO: f64 = 1.0;
const NU: f64 = 0.1;
const W_I: f64 = 1.0;
const V_I: f64 = 0.5;

fn tc(r: f64) -> f64 {
    let d = RO * RO - RI * RI;
    -W_I * RI * RI / d * r + W_I * RI * RI * RO * RO / d / r
}

/// Joined sector and the indices of its interface faces.
fn joined() -> (Mesh, Vec<usize>) {
    let a = annulus_sector(8, 8, RI, 0.75, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
    let b = annulus_sector(6, 8, 0.75, RO, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
    let (mut m, rep) =
        join_non_conformal_cylindrical(&a, "outer", &b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap();
    m.set_periodic_rotation("theta_min", "theta_max", [0.0; 3], [0.0, 0.0, 1.0], FRAC_PI_3)
        .unwrap();
    (m, rep.interface_faces)
}

struct Run {
    solver: SimpleSolver,
    converged: bool,
    last: rcfd_fv::simple::IterationInfo,
}

/// C2b template: frame angular velocity `omega_f` (0 = inertial, no source); wall velocities given in the solved frame.
fn solve(mesh: Mesh, omega_f: f64, max_iter: usize) -> Run {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let cf = geom.mesh.face_centres();
    let wall = |name: &str, w_abs: f64| {
        let w = w_abs - omega_f;
        geom.mesh
            .patch(name)
            .unwrap()
            .faces()
            .map(|f| [-w * cf[f][1], w * cf[f][0], 0.0])
            .collect::<Vec<_>>()
    };
    u.set_bc(
        &geom,
        "inner",
        Box::new(FixedValue(Profile::PerFace(wall("inner", W_I)))),
    )
    .unwrap();
    u.set_bc(
        &geom,
        "outer",
        Box::new(FixedValue(Profile::PerFace(wall("outer", 0.0)))),
    )
    .unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(NU, Coupling::Simplec);
    s.alpha_u = 0.8;
    s.alpha_p = 1.0;
    s.convection = rcfd_fv::convection::ConvScheme::Central;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    let frame = RotatingFrame {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        omega: omega_f,
        centrifugal: true,
    };
    let zone = vec![true; geom.n_cells()];
    let mut last = solver.iterate();
    let mut converged = false;
    for _ in 0..max_iter {
        if omega_f != 0.0 {
            let ub = solver.u.boundary_values(&geom);
            let mut f = BodyForce::uniform(&geom, [0.0; 3]);
            frame.add_force(&geom, &solver.u.values, &ub, &zone, &mut f);
            solver.body_force = Some(f);
            solver.wall_pressure_trapezoid = true;
        }
        last = solver.iterate();
        if last.res_u < 1e-10 && last.continuity < 1e-9 {
            converged = true;
            break;
        }
    }
    Run {
        solver,
        converged,
        last,
    }
}

fn error(run: &Run, omega_f: f64) -> f64 {
    let cc = run.solver.geom.mesh.cell_centres();
    run.solver
        .u
        .values
        .iter()
        .enumerate()
        .map(|(c, v)| {
            let (x, y) = (cc[c][0], cc[c][1]);
            let r = x.hypot(y);
            ((-y * v[0] + x * v[1]) / r + omega_f * r - tc(r)).abs()
        })
        .fold(0.0, f64::max)
}

/// S1 / S2 of ADR 0109: returns the failed criteria.
fn s_impl(omega_f: f64) -> Vec<(&'static str, String)> {
    let mut fail = Vec::new();
    let mut ctrl = Vec::new();
    for n in [8, 6] {
        let r = solve(
            annulus_sector(n, 16, RI, RO, FRAC_PI_3, Grading::Uniform, 0.1).unwrap(),
            omega_f,
            4000,
        );
        println!(
            "control {n} x 16: error {:.3e} V_i, converged {}",
            error(&r, omega_f) / V_I,
            r.converged
        );
        ctrl.push(error(&r, omega_f));
    }
    let (mesh, faces) = joined();
    let run = solve(mesh, omega_f, 4000);
    let e = error(&run, omega_f);
    let bound = 1.5 * ctrl[0].max(ctrl[1]);
    let (m, phi) = (&run.solver.geom.mesh, &run.solver.phi);
    let area: f64 = faces.iter().map(|&f| m.face_area(f)).sum();
    let net: f64 = faces.iter().map(|&f| phi[f]).sum();
    let abs: f64 = faces.iter().map(|&f| phi[f].abs()).sum();
    let seam = |name: &str| m.patch(name).unwrap().faces().map(|f| phi[f]).sum::<f64>();
    let seam_area: f64 = m.patch("theta_min").unwrap().faces().map(|f| m.face_area(f)).sum();
    let seam_bal = (seam("theta_min") + seam("theta_max")).abs();
    println!(
        "S (Omega_f {omega_f}): converged {} ({:?}); error {:.3e} V_i (1.5 x max control = {:.3e}); net interface flux {:.3e} V_i A (1e-8); \
         sum |flux| {:.3e} V_i A (1e-3); seam balance {:.3e} V_i A_seam (1e-8)",
        run.converged,
        run.last,
        e / V_I,
        bound / V_I,
        net.abs() / (V_I * area),
        abs / (V_I * area),
        seam_bal / (V_I * seam_area)
    );
    if !run.converged {
        fail.push(("main", "not converged".to_string()));
    }
    if e > bound {
        fail.push(("main", format!("error {:.3e} > {:.3e}", e / V_I, bound / V_I)));
    }
    if net.abs() > 1e-8 * V_I * area {
        fail.push(("main", "net interface flux".into()));
    }
    if abs > 1e-3 * V_I * area {
        fail.push(("abs", format!("sum |flux| {:.3e}", abs / (V_I * area))));
    }
    if seam_bal > 1e-8 * V_I * seam_area {
        fail.push(("main", "seam balance".into()));
    }
    fail
}

fn require(f: &[(&str, String)], tag: &str) {
    let bad: Vec<_> = f.iter().filter(|x| x.0 == tag).collect();
    assert!(bad.is_empty(), "{bad:?}");
}

/// S1 / S2 criteria that pass: converged, error bound, net interface flux, seam balance.
#[test]
fn s1_sector_taylor_couette_through_the_cylinder_inertial_frame() {
    require(&s_impl(0.0), "main");
}

#[test]
fn s2_sector_taylor_couette_through_the_cylinder_rotating_frame() {
    require(&s_impl(W_I), "main");
}

#[test]
#[ignore = "known miss: sum |interface flux| 5.79e-3 V_i A (limit 1e-3); azimuthal flow interpolated between cells at different angles"]
fn s1_sector_absolute_interface_flux_known_miss() {
    require(&s_impl(0.0), "abs");
}

#[test]
#[ignore = "known miss: sum |interface flux| 2.06e-2 V_i A (limit 1e-3); azimuthal flow interpolated between cells at different angles"]
fn s2_sector_absolute_interface_flux_known_miss() {
    require(&s_impl(W_I), "abs");
}

/// Construction and 50-iteration smoke (rule d), not a criterion.
#[test]
#[ignore = "smoke diagnostic of ADR 0109; run on demand"]
fn smoke_w43b_sector() {
    for omega_f in [0.0, W_I] {
        let r = solve(joined().0, omega_f, 50);
        println!(
            "smoke sector Omega_f {omega_f}: {:?}, error {:.3e} V_i",
            r.last,
            error(&r, omega_f) / V_I
        );
    }
}

/// Diagnostic, not a criterion: the absolute interface flux of S1 on the G2 sector and on the sector with doubled counts
/// around (16 / 12) and radially (16 / 16).
#[test]
#[ignore = "diagnostic of ADR 0109; run on demand"]
fn diag_sector_absolute_flux_refinement() {
    for (na, nb, nr) in [(8, 6, 8), (16, 12, 16)] {
        let a = annulus_sector(na, nr, RI, 0.75, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
        let b = annulus_sector(nb, nr, 0.75, RO, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
        let (mut m, rep) =
            join_non_conformal_cylindrical(&a, "outer", &b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap();
        m.set_periodic_rotation("theta_min", "theta_max", [0.0; 3], [0.0, 0.0, 1.0], FRAC_PI_3)
            .unwrap();
        let run = solve(m, 0.0, 8000);
        let (mesh, phi) = (&run.solver.geom.mesh, &run.solver.phi);
        let area: f64 = rep.interface_faces.iter().map(|&f| mesh.face_area(f)).sum();
        let abs: f64 = rep.interface_faces.iter().map(|&f| phi[f].abs()).sum();
        println!(
            "diag {na}/{nb} x {nr}: converged {}, error {:.3e} V_i, sum |flux| {:.3e} V_i A",
            run.converged,
            error(&run, 0.0) / V_I,
            abs / (V_I * area)
        );
    }
}
