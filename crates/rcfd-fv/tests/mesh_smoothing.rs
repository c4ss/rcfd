//! Mesh smoothing checks S1-S4 of ADR 0033. The limits are those registered in the ADR before the code ran; where the
//! registered set-up contradicts its own expectation the corrected check is a new test and the original is kept ignored.

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::ale::move_mesh;
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::FvGeometry;
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_2d, box_mesh, Grading};
use rcfd_mesh::motion::{moved_mesh, patch_points, rotation_matrix};
use rcfd_mesh::Mesh;
use std::sync::Arc;

const TIGHT: SolveControl = SolveControl {
    tol_abs: 1e-16,
    tol_rel: 1e-14,
    max_iter: 20000,
};

fn geom(mesh: &Mesh) -> FvGeometry {
    FvGeometry::new(Arc::new(mesh.clone())).unwrap()
}

/// Prescribed displacement `f(x)` on the listed patches (zero elsewhere).
fn prescribe(mesh: &Mesh, patches: &[&str], f: impl Fn(Vec3) -> Vec3) -> Vec<Vec3> {
    let mut d = vec![vec3::ZERO; mesh.n_points()];
    for name in patches {
        for q in patch_points(mesh, name).unwrap() {
            d[q] = f(mesh.points()[q]);
        }
    }
    d
}

fn classes(mesh: &Mesh, f: impl Fn(&str) -> PatchClass) -> Vec<PatchClass> {
    mesh.patches().iter().map(|p| f(&p.name)).collect()
}

#[test]
fn s1_affine_displacement_is_reproduced() {
    let mesh = box_mesh(
        [6, 5, 4],
        [1.2, 1.0, 0.8],
        [Grading::Expansion(3.0), Grading::Expansion(2.0), Grading::Uniform],
    )
    .unwrap();
    let a = [[0.10, -0.05, 0.02], [0.03, 0.08, -0.04], [-0.02, 0.06, 0.05]];
    let b = [0.1, -0.2, 0.05];
    let exact = |x: Vec3| [0, 1, 2].map(|i| a[i][0] * x[0] + a[i][1] * x[1] + a[i][2] * x[2] + b[i]);
    let all = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
    let presc = prescribe(&mesh, &all, exact);
    let cls = classes(&mesh, |_| PatchClass::Prescribed);
    let r = smooth_displacement(&geom(&mesh), &cls, &presc, SmoothingMethod::Laplacian, &TIGHT).unwrap();
    let err = mesh
        .points()
        .iter()
        .zip(&r.displacement)
        .map(|(x, d)| vec3::norm(vec3::sub(*d, exact(*x))))
        .fold(0.0, f64::max);
    println!("S1: max |d - (A x + b)| = {err:.2e} (limit 1e-12)");
    assert!(err <= 1e-12, "{err}");
}

#[test]
fn s2_translation_exactness_and_slip() {
    // (i) every patch prescribed to the same constant: all points move by that constant
    let mesh = box_mesh(
        [5, 4, 3],
        [1.0, 0.8, 0.6],
        [Grading::Expansion(2.0), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let d0 = [0.13, -0.07, 0.21];
    let all = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
    let presc = prescribe(&mesh, &all, |_| d0);
    let cls = classes(&mesh, |_| PatchClass::Prescribed);
    for k in [1.0, 2.0] {
        let r = smooth_displacement(&geom(&mesh), &cls, &presc, SmoothingMethod::InverseDistance(k), &TIGHT).unwrap();
        let err = r
            .displacement
            .iter()
            .map(|d| vec3::norm(vec3::sub(*d, d0)))
            .fold(0.0, f64::max);
        println!("S2 (i) inverse distance k = {k}: max |d - d0| = {err:.2e} (limit 1e-12)");
        assert!(err <= 1e-12, "k = {k}: {err}");
    }
    // (ii) xmin and xmax prescribed (0.2, 0, 0), the other patches slip
    let mesh = box_mesh([6, 4, 3], [1.2, 0.8, 0.6], [Grading::Uniform; 3]).unwrap();
    let presc = prescribe(&mesh, &["xmin", "xmax"], |_| [0.2, 0.0, 0.0]);
    let cls = classes(&mesh, |n| {
        if n == "xmin" || n == "xmax" {
            PatchClass::Prescribed
        } else {
            PatchClass::Slip
        }
    });
    for (label, m) in [
        ("laplacian", SmoothingMethod::Laplacian),
        ("inverse distance k = 1", SmoothingMethod::InverseDistance(1.0)),
    ] {
        let r = smooth_displacement(&geom(&mesh), &cls, &presc, m, &TIGHT).unwrap();
        let err = r
            .displacement
            .iter()
            .map(|d| vec3::norm(vec3::sub(*d, [0.2, 0.0, 0.0])))
            .fold(0.0, f64::max);
        println!("S2 (ii) {label}: max |d - (0.2, 0, 0)| = {err:.2e} (limit 1e-12)");
        assert!(err <= 1e-12, "{label}: {err}");
    }
}

fn square() -> Mesh {
    box_2d(20, 20, 1.0, 1.0, false, false).unwrap()
}

fn tilted(side_class: PatchClass) {
    let mesh = square();
    let g = geom(&mesh);
    let r = rotation_matrix([0.0, 0.0, 1.0], 0.3);
    let centre = [0.5, 1.0, 0.0];
    let presc = prescribe(&mesh, &["ymax"], |x| {
        let q = vec3::sub(x, centre);
        let rq = [0, 1, 2].map(|i| r[i][0] * q[0] + r[i][1] * q[1] + r[i][2] * q[2]);
        vec3::sub(vec3::add(centre, rq), x)
    });
    let cls = classes(&mesh, |n| match n {
        "ymax" => PatchClass::Prescribed,
        "ymin" => PatchClass::Fixed,
        _ => side_class,
    });
    for (label, m) in [
        ("laplacian", SmoothingMethod::Laplacian),
        ("inverse distance k = 1", SmoothingMethod::InverseDistance(1.0)),
        ("inverse distance k = 2", SmoothingMethod::InverseDistance(2.0)),
    ] {
        let sm = smooth_displacement(&g, &cls, &presc, m, &TIGHT).unwrap();
        let pts: Vec<Vec3> = mesh
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect();
        let step = move_mesh(&g, pts, 1.0).unwrap_or_else(|e| panic!("{label}: {e}"));
        println!(
            "S3 (i) {label}: all volumes positive, min V_new/V_old = {:.4}",
            step.min_volume_ratio
        );
        assert!(step.min_volume_ratio > 0.0);
    }
}

/// As registered: the three other walls fixed. The corner of the tilted top wall moves 0.148 down, past the fixed
/// wall points above it, so the corner cells fold (a physical impossibility of the set-up, not a smoothing defect;
/// ADR 0033 addendum). Kept, ignored.
#[test]
#[ignore = "registered set-up folds the corner cells: fixed side walls cannot follow the tilted corner; superseded by s3c"]
fn s3_validity_of_a_tilted_top_wall_registered_fixed_sides_known_miss() {
    tilted(PatchClass::Fixed);
}

/// Side walls slip in their planes (the corner points of the prescribed wall still win).
#[test]
fn s3c_validity_of_a_tilted_top_wall_with_slip_sides() {
    tilted(PatchClass::Slip);
}

fn compression(side_class: PatchClass) -> (f64, f64) {
    let mesh = square();
    let g = geom(&mesh);
    let presc = prescribe(&mesh, &["ymax"], |_| [0.0, -0.5, 0.0]);
    let cls = classes(&mesh, |n| match n {
        "ymax" => PatchClass::Prescribed,
        "ymin" => PatchClass::Fixed,
        _ => side_class,
    });
    let sm = smooth_displacement(&g, &cls, &presc, SmoothingMethod::Laplacian, &TIGHT).unwrap();
    let pts: Vec<Vec3> = mesh
        .points()
        .iter()
        .zip(&sm.displacement)
        .map(|(p, d)| vec3::add(*p, *d))
        .collect();
    let pos_err = mesh
        .points()
        .iter()
        .zip(&pts)
        .map(|(p, q)| (q[1] - 0.5 * p[1]).abs().max((q[0] - p[0]).abs()))
        .fold(0.0, f64::max);
    let step = move_mesh(&g, pts, 1.0).unwrap();
    (step.min_volume_ratio, pos_err)
}

/// The registered text fixes the side walls; with zero displacement on them the compression is not uniform, so the
/// registered expectation cannot hold (set-up contradiction, ADR 0033 addendum). Kept, ignored.
#[test]
#[ignore = "registered set-up contradicts its expectation: fixed side walls prevent a uniform compression; superseded by s3b"]
fn s3_uniform_compression_registered_fixed_sides_known_miss() {
    let (ratio, pos) = compression(PatchClass::Fixed);
    println!("S3 (ii) fixed sides: min ratio {ratio:.6}, position error {pos:.2e}");
    assert!((ratio - 0.5).abs() <= 1e-9 && pos <= 1e-12);
}

#[test]
fn s3b_uniform_compression_with_slip_sides() {
    let (ratio, pos) = compression(PatchClass::Slip);
    println!(
        "S3 (ii): min V_new/V_old = {ratio:.12} (limit |. - 0.5| <= 1e-9), position error {pos:.2e} (limit 1e-12)"
    );
    assert!((ratio - 0.5).abs() <= 1e-9, "{ratio}");
    assert!(pos <= 1e-12, "{pos}");
}

#[test]
fn s4_invalid_motion_is_reported() {
    let mesh = square();
    let g = geom(&mesh);
    let presc = prescribe(&mesh, &["ymax"], |_| [0.0, -1.2, 0.0]);
    let cls = classes(&mesh, |n| match n {
        "ymax" => PatchClass::Prescribed,
        "ymin" => PatchClass::Fixed,
        _ => PatchClass::Slip,
    });
    let sm = smooth_displacement(&g, &cls, &presc, SmoothingMethod::Laplacian, &TIGHT).unwrap();
    let pts: Vec<Vec3> = mesh
        .points()
        .iter()
        .zip(&sm.displacement)
        .map(|(p, d)| vec3::add(*p, *d))
        .collect();
    let e = move_mesh(&g, pts.clone(), 1.0).unwrap_err();
    println!("S4: {e}");
    assert!(e.contains("non-positive volume"), "{e}");
    assert!(e.chars().any(|c| c.is_ascii_digit()), "no cell index in: {e}");
    assert!(moved_mesh(&mesh, pts).is_err());
}
