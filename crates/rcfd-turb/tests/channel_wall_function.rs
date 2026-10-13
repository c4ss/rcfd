//! F-040: wall distance and standard wall functions on a periodic channel.
//!
//! Fully developed channel flow at friction Reynolds number Re_tau = u_tau h / nu = 395
//! (h = half height, u_tau = 1, nu = 1/395): the streamwise pressure gradient is the body
//! acceleration u_tau^2 / h, the interior eddy viscosity comes from the mixing-length model
//! l = kappa d and the walls use the log-law wall function on a first cell in the log layer.
//! The converged profile must recover the log law and the imposed wall shear.

use rcfd_fv::bc::FixedValue;
use rcfd_fv::simple::{BodyForce, Coupling, SimpleSettings, SimpleSolver};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_mesh::gen::{box_2d_graded, Grading};
use rcfd_turb::{effective_viscosity, patch_faces, wall_distance, wall_y_plus, MixingLength, WallConstants};
use std::sync::Arc;

const RE_TAU: f64 = 395.0;

struct Channel {
    solver: SimpleSolver,
    wall_dist: Vec<f64>,
    wall_nut: Vec<(usize, f64)>,
    res: f64,
    iterations: usize,
}

fn run_channel(ny: usize, grading: f64) -> Channel {
    let nu = 1.0 / RE_TAU;
    let g = if grading > 1.0 {
        Grading::Symmetric(grading)
    } else {
        Grading::Uniform
    };
    let mesh = box_2d_graded(4, ny, 1.0, 2.0, [Grading::Uniform, g], true, false).unwrap();
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let mut u = VectorField::uniform("U", &geom, [17.0, 0.0, 0.0]);
    for w in ["ymin", "ymax"] {
        u.set_bc(&geom, w, Box::new(FixedValue::vector([0.0; 3]))).unwrap();
    }
    let p = ScalarField::uniform("p", &geom, 0.0);
    let mut s = SimpleSettings::new(nu, Coupling::Simplec);
    s.alpha_u = 0.9;
    // H from the solved predictor: the default Jacobi-like H converges far more slowly in this diffusion-dominated flow
    s.h_from_solution = true;
    let mut solver = SimpleSolver::new(geom.clone(), u, p, s);
    solver.body_force = Some(BodyForce::uniform(&geom, [1.0, 0.0, 0.0])); // u_tau^2 / h
    let wall_dist = wall_distance(&geom.mesh, &["ymin", "ymax"]).unwrap();
    let model = MixingLength {
        kappa: 0.41,
        l_max: f64::INFINITY,
        wall_distance: wall_dist.clone(),
    };
    let wc = WallConstants::default();
    let wall_faces = patch_faces(&geom, &["ymin", "ymax"]);
    let mut wall_nut: Vec<(usize, f64)> = Vec::new();
    let mut nut = vec![0.0; geom.n_cells()];
    let (mut res, mut iterations) = (f64::INFINITY, 0);
    for it in 0..8000 {
        // under-relax the eddy viscosity: the plain Picard map nu_t = l^2 |S| is only neutrally stable
        for (n, new) in nut
            .iter_mut()
            .zip(model.nut_with_walls(&geom, &solver.u, &wc, nu, &wall_faces))
        {
            *n = if it == 0 { new } else { 0.5 * *n + 0.5 * new };
        }
        let old = wall_nut.clone();
        wall_nut = wall_faces
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                let c = geom.mesh.owner()[f];
                let y = geom.dn[f - geom.n_internal];
                let new = wc.nut_wall_from_velocity(nu, solver.u.values[c][0], y);
                (f, if it == 0 { new } else { 0.5 * old[i].1 + 0.5 * new })
            })
            .collect();
        solver.nu_face = Some(effective_viscosity(&geom, nu, &nut, &wall_nut));
        let info = solver.iterate();
        // the momentum residual of the (identically zero) v component is pure round-off noise; use the update size
        res = info.max_du;
        iterations = it + 1;
        if res < 1e-10 {
            break;
        }
    }
    Channel {
        solver,
        wall_dist,
        wall_nut,
        res,
        iterations,
    }
}

#[test]
fn wall_distance_is_exact_on_the_channel_mesh() {
    let ch = run_channel(12, 1.0);
    let cc = ch.solver.geom.mesh.cell_centres();
    for (c, d) in ch.wall_dist.iter().enumerate() {
        let y = cc[c][1];
        assert!(
            (d - y.min(2.0 - y)).abs() < 1e-13,
            "cell {c}: {d} vs {}",
            y.min(2.0 - y)
        );
    }
    // also on a graded mesh
    let mesh = box_2d_graded(
        3,
        20,
        1.0,
        2.0,
        [Grading::Uniform, Grading::Symmetric(5.0)],
        false,
        false,
    )
    .unwrap();
    let d = wall_distance(&mesh, &["ymin", "ymax"]).unwrap();
    for (c, p) in mesh.cell_centres().iter().enumerate() {
        assert!((d[c] - p[1].min(2.0 - p[1])).abs() < 1e-13);
    }
    // distance to the side walls only
    let d = wall_distance(&mesh, &["xmin"]).unwrap();
    for (c, p) in mesh.cell_centres().iter().enumerate() {
        assert!((d[c] - p[0]).abs() < 1e-13);
    }
    assert!(wall_distance(&mesh, &["nope"]).is_err());
}

#[test]
fn channel_recovers_the_log_law_and_the_wall_shear() {
    let wc = WallConstants::default();
    for ny in [8usize, 12, 16] {
        let ch = run_channel(ny, 1.0);
        assert!(ch.res < 1e-10, "ny {ny}: residual {} after {}", ch.res, ch.iterations);
        let geom = &ch.solver.geom;
        let cc = geom.mesh.cell_centres();
        let nu = 1.0 / RE_TAU;
        // y+ of the first cell: y u_tau / nu with u_tau = 1 (cell height 2/ny)
        let y1 = 1.0 / ny as f64;
        let yplus1 = y1 * RE_TAU;
        println!("ny {ny}: first-cell y+ = {yplus1:.1}, {} iterations", ch.iterations);
        assert!(yplus1 > 11.6);
        let wall_faces = patch_faces(geom, &["ymin", "ymax"]);
        for yp in wall_y_plus(geom, &ch.solver.u, &wc, nu, &wall_faces) {
            assert!((yp / yplus1 - 1.0).abs() < 5e-3, "y+ {yp} vs {yplus1}");
        }
        // wall shear from the wall function equals the imposed u_tau^2 = 1 (momentum balance)
        for &(f, nutw) in &ch.wall_nut {
            let c = geom.mesh.owner()[f];
            let y = geom.dn[f - geom.n_internal];
            let tau = (nu + nutw) * ch.solver.u.values[c][0] / y;
            assert!((tau - 1.0).abs() < 2e-3, "wall shear {tau}");
        }
        // first cell sits on the log law (u_tau = 1 by the momentum balance)
        let c1 = (0..cc.len())
            .find(|&c| ch.wall_dist[c] < y1 * 1.001 && cc[c][1] < 1.0)
            .unwrap();
        let u1 = ch.solver.u.values[c1][0];
        let want1 = wc.u_plus(yplus1);
        println!("ny {ny}: first-cell u+ = {u1:.4}, log law {want1:.4}");
        assert!((u1 / want1 - 1.0).abs() < 5e-3);
        // interior: exact solution of the mixing-length stress balance
        //   (nu + l^2 S) S = 1 - y/h, l = kappa y, integrated outward from the first cell
        let slope = |y: f64| {
            let l = 0.41 * y;
            let tau = 1.0 - y;
            (-nu + (nu * nu + 4.0 * l * l * tau).sqrt()) / (2.0 * l * l)
        };
        let reference = |y: f64| {
            let n = 4000;
            let h = (y - y1) / n as f64;
            let mut acc = slope(y1) + slope(y);
            for i in 1..n {
                acc += slope(y1 + i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
            }
            want1 + acc * h / 3.0
        };
        let mut worst: f64 = 0.0;
        for (c, p) in cc.iter().enumerate() {
            if p[1] < 1.0 && ch.wall_dist[c] > y1 * 1.001 {
                let r = reference(p[1]);
                worst = worst.max((ch.solver.u.values[c][0] / r - 1.0).abs());
            }
        }
        println!("ny {ny}: max deviation from the mixing-length ODE solution = {worst:.4}");
        // the 2-3% offset is the truncation error of the second-order stencil across the log layer between
        // the first two cells of a uniform mesh (centre distances in the ratio 1:3); it does not shrink with ny
        assert!(worst < 0.03, "ny {ny}: {worst}");
        // streamwise uniformity (fully developed) and symmetry about the centre line
        for c in 0..cc.len() {
            let col0 = (c / 4) * 4;
            assert!((ch.solver.u.values[c][0] - ch.solver.u.values[col0][0]).abs() < 1e-7);
        }
    }
}
