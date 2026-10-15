//! ADR 0030 T10: solver smoke test on generated tetrahedral meshes. Harmonic Dirichlet problems with closed-form
//! solutions. Limits are those registered in docs/adr/0030-wave19b-meshing.md before this file was compiled or run.

use rcfd_core::vec3::Vec3;
use rcfd_fv::bc::{FixedValue, Profile};
use rcfd_fv::diffusion::{laplacian, uniform_gamma};
use rcfd_fv::gradient::least_squares;
use rcfd_fv::{FvGeometry, ScalarField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::quality::check_mesh;
use rcfd_mesh::surface::{cube, icosphere, TriSurface};
use rcfd_mesh::tetmesh::{tetrahedralize, TetOptions};
use rcfd_mesh::Mesh;
use std::collections::BTreeMap;
use std::sync::Arc;

fn mesh_of(s: &TriSurface, opt: &TetOptions) -> Mesh {
    let tm = tetrahedralize(s, opt).unwrap();
    tm.to_mesh(&BTreeMap::new()).unwrap()
}

/// Relative volume-weighted L2 error against the RMS of the exact solution, and the max point error.
fn solve(mesh: Mesh, exact: impl Fn(Vec3) -> f64) -> (f64, f64, usize) {
    assert!(check_mesh(&mesh).is_ok());
    let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
    let cc = geom.mesh.cell_centres().to_vec();
    let vol = geom.mesh.cell_volumes().to_vec();
    let mut phi = ScalarField::from_fn("phi", &geom, |_| 0.0);
    for p in geom.mesh.patches() {
        let vals = p
            .faces()
            .map(|f| [exact(geom.mesh.face_centres()[f]), 0.0, 0.0])
            .collect();
        phi.set_bc(&geom, &p.name, Box::new(FixedValue(Profile::PerFace(vals))))
            .unwrap();
    }
    let gam = uniform_gamma(&geom, 1.0);
    let ctl = SolveControl {
        tol_abs: 1e-300,
        tol_rel: 1e-13,
        max_iter: 5000,
    };
    for _ in 0..200 {
        let cells = phi.values.clone();
        let bc = phi.boundary_coeffs(&geom);
        let bvals = phi.boundary_values(&geom);
        let grad = least_squares(&geom, &cells, &bvals);
        let m = laplacian(&geom, &gam, &bc, Some(&grad));
        let mut x = cells.clone();
        m.solve(&mut x, &ctl);
        let change = x.iter().zip(&cells).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        phi.values = x;
        if change < 1e-12 {
            break;
        }
    }
    let (mut e2, mut r2, mut tot, mut emax) = (0.0, 0.0, 0.0, 0.0f64);
    for c in 0..cc.len() {
        let ex = exact(cc[c]);
        let e = phi.values[c] - ex;
        e2 += e * e * vol[c];
        r2 += ex * ex * vol[c];
        tot += vol[c];
        emax = emax.max(e.abs());
    }
    ((e2 / tot).sqrt() / (r2 / tot).sqrt(), emax, cc.len())
}

#[test]
fn t10_sphere_harmonic() {
    let s = icosphere(1.0, 2);
    let opt = TetOptions {
        size: Some(0.25),
        ..Default::default()
    };
    let m = mesh_of(&s, &opt);
    let (rel, emax, n) = solve(m, |p| p[0] * p[0] - p[1] * p[1]);
    eprintln!("T10 sphere: {n} cells, relative L2 error {rel:.4e}, max error {emax:.4e}");
    assert!(n >= 1000, "cells {n}");
    assert!(rel <= 0.05, "relative L2 error {rel}");
}

#[test]
fn t10_cube_linear() {
    let s = cube(2.0, 4);
    let opt = TetOptions {
        size: Some(0.5),
        ..Default::default()
    };
    let m = mesh_of(&s, &opt);
    let (rel, emax, n) = solve(m, |p| p[0] + 2.0 * p[1] - p[2]);
    eprintln!("T10 cube: {n} cells, relative L2 error {rel:.4e}, max error {emax:.4e}");
    assert!(rel <= 0.05, "relative L2 error {rel}");
}
