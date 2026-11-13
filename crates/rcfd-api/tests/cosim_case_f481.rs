//! F-481 (ADR 0381): the `[cosim]` smoothing sees a fixed patch against a sliding one, when the wall displacement has a
//! component tangential to that patch (the mutation MU5 of ADR 0380 is caught). Limits registered in ADR 0381 before this
//! file existed.

use rcfd_api::case::Case;
use rcfd_api::cosim_case::fluid_from_case;
use rcfd_core::vec3::Vec3;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::patch_points;
use rcfd_mesh::Mesh;
use serde_json::{json, Value};
use std::f64::consts::PI;

const A: f64 = 0.005;

fn m0() -> Mesh {
    box_mesh([8, 8, 2], [0.4, 0.4, 0.1], [Grading::Uniform; 3]).unwrap()
}

/// The displacement of the 27 wall points, ascending mesh point index.
fn wall_displacement(mesh0: &Mesh, tangential: bool) -> Vec<Vec3> {
    patch_points(mesh0, "xmin")
        .unwrap()
        .iter()
        .map(|&q| {
            if tangential {
                let j = (mesh0.points()[q][1] / 0.05).round() as i64;
                let dy = if j == 0 || j == 8 {
                    0.0
                } else {
                    A * (PI * j as f64 / 8.0).sin()
                };
                [0.0, dy, 0.0]
            } else {
                [0.002, 0.0, 0.0]
            }
        })
        .collect()
}

fn case_json(fixed: &[&str], tangential: bool) -> Value {
    let d: Vec<f64> = wall_displacement(&m0(), tangential).iter().flatten().copied().collect();
    json!({
        "mesh": {"generator": "box_3d", "nx": 8, "ny": 8, "nz": 2, "lx": 0.4, "ly": 0.4, "lz": 0.1},
        "physics": {"nu": 1e-4, "rho": 1.0},
        "boundaries": {
            "xmin": {"velocity": "zero_gradient"},
            "xmax": {"velocity": "zero_gradient", "pressure": "fixed", "pressure_value": 0.0},
            "ymin": {"velocity": "symmetry"},
            "ymax": {"velocity": "symmetry"},
            "zmin": {"velocity": "zero_gradient"},
            "zmax": {"velocity": "zero_gradient"}
        },
        "solver": {
            "algorithm": "piso", "convection": "central", "n_correctors": 2, "alpha_u": 1.0, "alpha_p": 1.0,
            "momentum_rel_tol": 1e-13, "pressure_rel_tol": 1e-13, "pressure_abs_tol": 1e-16,
            "momentum_max_iter": 5000, "pressure_max_iter": 5000, "dt": 0.01
        },
        "cosim": {"patch": "xmin", "fixed_patches": fixed, "smoothing_tolerance": 1e-13, "initial_displacement": d}
    })
}

/// The points of the initial mesh of the participant built from the variant.
fn mesh_points(fixed: &[&str], tangential: bool) -> Vec<Vec3> {
    let case = Case::from_json(&case_json(fixed, tangential)).unwrap_or_else(|e| panic!("CWf does not parse: {e:?}"));
    let f = fluid_from_case(&case).unwrap_or_else(|e| panic!("CWf does not build: {e:?}"));
    f.flow.geom.mesh.points().to_vec()
}

fn bits_differ(a: &[Vec3], b: &[Vec3]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .flat_map(|(p, q)| (0..3).map(move |c| (p[c], q[c])))
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count()
}

fn max_diff(a: &[Vec3], b: &[Vec3], ids: &[usize], comp: usize) -> f64 {
    ids.iter().map(|&i| (a[i][comp] - b[i][comp]).abs()).fold(0.0, f64::max)
}

fn all_ids(n: usize) -> Vec<usize> {
    (0..n).collect()
}

#[test]
fn s1_f1_n3_the_fixed_patch_does_not_move_and_the_wall_does() {
    let mesh0 = m0();
    let xmax = patch_points(&mesh0, "xmax").unwrap();
    let wall = patch_points(&mesh0, "xmin").unwrap();
    assert_eq!(wall.len(), 27);
    let fixed = mesh_points(&["xmax"], true);
    let slide = mesh_points(&[], true);
    let p0 = mesh0.points();
    // F1: the points of the fixed patch: 0 bits.
    let moved: usize = xmax
        .iter()
        .map(|&i| (0..3).filter(|&c| fixed[i][c].to_bits() != p0[i][c].to_bits()).count())
        .sum();
    println!("F1: xmax points {}, bits moved {moved}", xmax.len());
    assert_eq!(moved, 0, "F1: a point of the fixed patch moved");
    // S1: the wall is prescribed in both variants.
    let wall_bits: usize = wall
        .iter()
        .map(|&i| {
            (0..3)
                .filter(|&c| fixed[i][c].to_bits() != slide[i][c].to_bits())
                .count()
        })
        .sum();
    println!("S1: wall bits differing between the variants {wall_bits}");
    assert_eq!(wall_bits, 0, "S1: the wall differs between the variants");
    // N3: the wall moved in y.
    let dy = max_diff(&fixed, p0, &wall, 1);
    println!("N3: largest wall |dy| {dy:e}");
    assert!(dy >= 1e-3, "N3: the wall displacement is not seen ({dy:e})");
}

#[test]
fn m1_m2_a_sliding_patch_moves_along_its_plane_and_a_fixed_one_does_not() {
    let mesh0 = m0();
    let xmax = patch_points(&mesh0, "xmax").unwrap();
    let fixed = mesh_points(&["xmax"], true);
    let slide = mesh_points(&[], true);
    let n = bits_differ(&fixed, &slide);
    let points = fixed
        .iter()
        .zip(&slide)
        .filter(|(p, q)| bits_differ(&[**p], &[**q]) > 0)
        .count();
    let dy = max_diff(&fixed, &slide, &xmax, 1);
    let dx = max_diff(&slide, mesh0.points(), &xmax, 0);
    println!("M1: points differing {points} (values {n}); largest |dy| on xmax {dy:e}; M2: slide xmax |dx| {dx:e}");
    assert!(points > 0, "M1: the two meshes are the same");
    assert!(dy >= 1e-5, "M1: sliding and fixed xmax differ by only {dy:e} m");
    assert!(dx <= 1e-12, "M2: the sliding patch left its plane ({dx:e})");
}

#[test]
fn n1_an_x_only_wall_displacement_cannot_tell_them_apart() {
    let fixed = mesh_points(&["xmax"], false);
    let slide = mesh_points(&[], false);
    let ids = all_ids(fixed.len());
    let worst = (0..3).map(|c| max_diff(&fixed, &slide, &ids, c)).fold(0.0, f64::max);
    println!("N1: largest coordinate difference {worst:e}");
    assert!(
        worst <= 1e-12,
        "N1: the x-only set-up already tells them apart ({worst:e})"
    );
}

#[test]
fn n2_the_variant_is_built_the_same_twice() {
    let a = mesh_points(&["xmax"], true);
    let b = mesh_points(&["xmax"], true);
    assert_eq!(bits_differ(&a, &b), 0);
}
