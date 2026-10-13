//! F-107: translational and rotational periodicity validated against analytic
//! solutions. The pressure equation couples the two faces of a periodic pair
//! implicitly, so fluxes through the pair are exactly opposite and the cell
//! balance closes to solver tolerance.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, MeanVelocityDriver, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{annulus_sector, box_2d, cylinder_ogrid, Grading};
use std::sync::Arc;

fn channel_solver(ny: usize, nu: f64) -> SimpleSolver {
    let mesh = box_2d(6, ny, 3.0, 1.0, true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    SimpleSolver::new(geom, u, p, s)
}

#[test]
fn periodic_channel_matches_plane_poiseuille() {
    // dp/dx imposed as body acceleration beta: u(y) = beta/(2 nu) y (H - y), H = 1
    let (nu, beta) = (0.1, 1.0);
    let mut s = channel_solver(24, nu);
    s.body_force = Some(BodyForce::uniform(&s.geom, [beta, 0.0, 0.0]));
    let mut last = s.iterate();
    for _ in 0..3000 {
        last = s.iterate();
        if last.res_u < 1e-10 {
            break;
        }
    }
    assert!(last.res_u < 1e-8, "{last:?}");
    // flux balance closes to round-off: the periodic pair carries exactly opposite fluxes
    assert!(last.continuity < 1e-10, "continuity {}", last.continuity);
    let mesh = &s.geom.mesh;
    let cc = mesh.cell_centres();
    let mut worst: f64 = 0.0;
    let umax = beta / (8.0 * nu);
    for (c, v) in s.u.values.iter().enumerate() {
        let y = cc[c][1];
        let exact = beta / (2.0 * nu) * y * (1.0 - y);
        worst = worst.max((v[0] - exact).abs() / umax);
        assert!(v[1].abs() < 1e-8);
    }
    println!("Poiseuille max |u - u_exact| / u_max = {worst:.3e}");
    assert!(worst < 2e-3, "{worst}");
    // streamwise uniformity (fully developed): all columns identical
    for (c, v) in s.u.values.iter().enumerate() {
        let col0 = (c / 6) * 6; // first cell of the same x-row (cells numbered x fastest)
        assert!((v[0] - s.u.values[col0][0]).abs() < 1e-7 * umax);
    }
}

#[test]
fn bulk_velocity_driver_reaches_the_target_and_the_analytic_gradient() {
    let nu = 0.05;
    let mut s = channel_solver(20, nu);
    s.driver = Some(MeanVelocityDriver {
        dir: [1.0, 0.0, 0.0],
        target: 1.0,
        accel: 0.0,
    });
    for _ in 0..3000 {
        s.iterate();
    }
    // Poiseuille: Ub = beta H^2 / (12 nu)  ->  beta = 12 nu Ub
    let beta = s.driver.unwrap().accel;
    let exact = 12.0 * nu * 1.0;
    println!("driver acceleration {beta:.6} vs analytic {exact:.6}");
    assert!((beta - exact).abs() / exact < 5e-3, "{beta} vs {exact}");
    let vol = s.geom.mesh.cell_volumes();
    let ub: f64 = s.u.values.iter().zip(vol).map(|(u, v)| u[0] * v).sum::<f64>() / vol.iter().sum::<f64>();
    assert!((ub - 1.0).abs() < 1e-6, "{ub}");
}

/// Couette flow between concentric cylinders, inner wall rotating with omega, outer fixed:
/// u_theta = A r + B / r with A = -omega r1^2 / (r2^2 - r1^2), B = omega r1^2 r2^2 / (r2^2 - r1^2).
fn couette_exact(r: f64, omega: f64, r1: f64, r2: f64) -> f64 {
    let a = -omega * r1 * r1 / (r2 * r2 - r1 * r1);
    let b = omega * r1 * r1 * r2 * r2 / (r2 * r2 - r1 * r1);
    a * r + b / r
}

fn rotating_solver(mesh: rcfd_mesh::Mesh, inner: &str, outer: &str, omega: f64) -> SimpleSolver {
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [0.0; 3]);
    let patch = geom.mesh.patch(inner).unwrap().clone();
    let cf = geom.mesh.face_centres();
    let wall: Vec<[f64; 3]> = patch
        .faces()
        .map(|f| [-omega * cf[f][1], omega * cf[f][0], 0.0])
        .collect();
    u.set_bc(&geom, inner, Box::new(FixedValue(rcfd_fv::Profile::PerFace(wall))))
        .unwrap();
    u.set_bc(&geom, outer, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(0.1, Coupling::Simplec);
    s.alpha_u = 0.8;
    s.alpha_p = 1.0;
    s.convection = rcfd_fv::convection::ConvScheme::Central;
    SimpleSolver::new(geom, u, p, s)
}

fn azimuthal_profile(s: &SimpleSolver) -> Vec<(f64, f64)> {
    let cc = s.geom.mesh.cell_centres();
    s.u.values
        .iter()
        .enumerate()
        .map(|(c, v)| {
            let (x, y) = (cc[c][0], cc[c][1]);
            let r = x.hypot(y);
            (r, (-y * v[0] + x * v[1]) / r)
        })
        .collect()
}

#[test]
fn sixty_degree_sector_matches_full_circle_and_taylor_couette() {
    let (r1, r2, omega) = (0.5, 1.0, 1.0);
    let n_r = 16;
    // full annulus
    let full = cylinder_ogrid(48, n_r, r1, r2, Grading::Uniform, 0.1).unwrap();
    let mut sf = rotating_solver(full, "cylinder", "farfield", omega);
    // 60 degree sector, rotationally periodic
    let sector = annulus_sector(8, n_r, r1, r2, std::f64::consts::FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
    assert_eq!(
        sector.patch("theta_min").unwrap().rotation.unwrap().angle,
        std::f64::consts::FRAC_PI_3
    );
    let mut ss = rotating_solver(sector, "inner", "outer", omega);
    let (mut lf, mut ls) = (sf.iterate(), ss.iterate());
    for _ in 0..600 {
        lf = sf.iterate();
        ls = ss.iterate();
        if lf.res_u < 1e-10 && ls.res_u < 1e-10 {
            break;
        }
    }
    println!("full: {lf:?}\nsector: {ls:?}");
    assert!(ls.continuity < 1e-9, "sector continuity {}", ls.continuity);
    // azimuthal velocity vs analytic (both meshes) and sector vs full at the same radii
    let err = |s: &SimpleSolver| {
        azimuthal_profile(s)
            .iter()
            .map(|&(r, ut)| (ut - couette_exact(r, omega, r1, r2)).abs() / (omega * r1))
            .fold(0.0, f64::max)
    };
    let (e_full, e_sector) = (err(&sf), err(&ss));
    println!("max |u_theta - exact| / (omega r1): full {e_full:.3e}, sector {e_sector:.3e}");
    assert!(e_full < 1e-2 && e_sector < 1e-2, "{e_full} {e_sector}");
    // sector vs full: compare u_theta(r) of every sector cell with the full-circle cell at the same ring
    let (pf, ps) = (azimuthal_profile(&sf), azimuthal_profile(&ss));
    let ring = |p: &[(f64, f64)], i: usize| p[i * (p.len() / n_r)].1; // ring i, first cell
    let mut worst: f64 = 0.0;
    for i in 0..n_r {
        // full-grid cells are numbered ring by ring (theta fastest), same for the sector
        worst = worst.max((ring(&pf, i) - ring(&ps, i)).abs() / (omega * r1));
    }
    println!("sector vs full ring values: max difference {worst:.3e}");
    assert!(worst < 2e-3, "{worst}");
    // no radial flow and no swirl-dependent spurious radial velocity
    let cc = ss.geom.mesh.cell_centres();
    let ur_max =
        ss.u.values
            .iter()
            .enumerate()
            .map(|(c, v)| ((cc[c][0] * v[0] + cc[c][1] * v[1]) / cc[c][0].hypot(cc[c][1])).abs())
            .fold(0.0, f64::max);
    println!("max radial velocity in sector {ur_max:.3e}");
    assert!(ur_max < 5e-3 * omega * r1, "{ur_max}");
}
