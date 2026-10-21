//! Registered checks Q1 to Q4 of ADR 0018 (F-233): the extended mesh-quality measures against closed forms
//! derived in the ADR. Limits are those registered there.

use rcfd_core::vec3::Vec3;
use rcfd_mesh::gen::{box_mesh, warp, Grading};
use rcfd_mesh::measures::{
    cell_orthogonality, compute_measures, polygon_angles, polygon_edge_ratio, polygon_warp, CellFace, Measures,
};
use rcfd_mesh::mesh::Mesh;
use rcfd_mesh::quality::{check_mesh, check_mesh_with, QualityReport, Thresholds};

const U: [Grading; 3] = [Grading::Uniform; 3];
const TOL: f64 = 1e-12;
const ATOL: f64 = 1e-9;

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b} (tol {tol})");
}

fn rel(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol * b.abs(), "{what}: {a} vs {b} (rel tol {tol})");
}

/// Hex faces for corners numbered with binary xyz (x fastest).
const HEX_FACES: [[usize; 4]; 6] = [
    [0, 2, 6, 4],
    [1, 3, 7, 5],
    [0, 1, 5, 4],
    [2, 3, 7, 6],
    [0, 1, 3, 2],
    [4, 5, 7, 6],
];

fn single_cell(points: Vec<Vec3>, faces: Vec<Vec<usize>>) -> Mesh {
    Mesh::from_cell_faces(points, &[faces], &[]).unwrap()
}

fn tet(p: [Vec3; 4]) -> Mesh {
    single_cell(
        p.to_vec(),
        vec![vec![0, 1, 2], vec![0, 1, 3], vec![0, 2, 3], vec![1, 2, 3]],
    )
}

fn hex(points: [Vec3; 8]) -> Mesh {
    single_cell(points.to_vec(), HEX_FACES.iter().map(|f| f.to_vec()).collect())
}

fn unit_corners() -> [Vec3; 8] {
    let mut p = [[0.0; 3]; 8];
    for (i, q) in p.iter_mut().enumerate() {
        *q = [(i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64];
    }
    p
}

fn sheared_hex(s: f64) -> Mesh {
    let mut p = unit_corners();
    for q in p.iter_mut() {
        q[0] += s * q[1];
    }
    hex(p)
}

fn shifted_stack() -> Mesh {
    let m = box_mesh([1, 2, 1], [1.0, 2.0, 1.0], U).unwrap();
    warp(&m, |p| {
        let dx = if p[1] > 1.5 {
            1.0
        } else if p[1] > 0.5 {
            0.5
        } else {
            0.0
        };
        [p[0] + dx, p[1], p[2]]
    })
    .unwrap()
}

fn neighbour_pair() -> Mesh {
    let m = box_mesh([2, 1, 1], [2.0, 1.0, 1.0], U).unwrap();
    warp(&m, |p| if p[0] > 1.5 { [p[0], p[1] + 1.0, p[2]] } else { p }).unwrap()
}

fn all_finite_or_inf(m: &Measures) {
    for v in [
        &m.face_warp,
        &m.face_edge_ratio,
        &m.cell_warp,
        &m.cell_face_angle_min,
        &m.cell_face_angle_max,
        &m.cell_ortho_skew,
        &m.cell_inverse_orthogonal,
        &m.cell_size_change,
        &m.cell_edge_length_ratio,
    ] {
        assert!(v.iter().all(|x| !x.is_nan()), "NaN in a measure array");
    }
}

/// ADR 0093: the four pre-existing `check_mesh` measures on a single-cell (or equal-cube) mesh: skewness 0, volume ratio 1.
fn existing_measures(m: &Mesh, non_orth_max: f64, non_orth_mean: f64, aspect: f64, aspect_tol: f64) {
    let r = check_mesh(m);
    near(r.max_non_orthogonality, non_orth_max, ATOL, "max non-orthogonality");
    near(r.mean_non_orthogonality, non_orth_mean, ATOL, "mean non-orthogonality");
    near(r.max_skewness, 0.0, TOL, "max skewness");
    near(r.max_aspect_ratio, aspect, aspect_tol, "max aspect ratio");
    near(r.max_volume_ratio, 1.0, TOL, "max volume ratio");
}

fn cell_measures(m: &Mesh) -> Measures {
    let r = compute_measures(m);
    all_finite_or_inf(&r);
    r
}

#[test]
fn q1a_unit_cube() {
    for n in [1usize, 2] {
        let m = box_mesh([n, n, n], [n as f64; 3], U).unwrap();
        let r = cell_measures(&m);
        for c in 0..m.n_cells() {
            near(r.cell_warp[c], 0.0, TOL, "warp");
            near(r.cell_face_angle_min[c], 90.0, ATOL, "amin");
            near(r.cell_face_angle_max[c], 90.0, ATOL, "amax");
            near(r.cell_ortho_skew[c], 0.0, TOL, "ortho skew");
            near(r.cell_inverse_orthogonal[c], 0.0, TOL, "inverse OQ");
            near(r.cell_size_change[c], 1.0, TOL, "size change");
            near(r.cell_edge_length_ratio[c], 1.0, TOL, "edge ratio");
        }
        existing_measures(&m, 0.0, 0.0, 1.0, TOL);
    }
}

#[test]
fn q1b_right_corner_tetrahedron() {
    let m = tet([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let r = cell_measures(&m);
    let v = 1.0 - 3.0 / 11f64.sqrt();
    near(r.cell_face_angle_min[0], 45.0, ATOL, "amin");
    near(r.cell_face_angle_max[0], 90.0, ATOL, "amax");
    near(r.cell_ortho_skew[0], v, TOL, "ortho skew");
    near(r.cell_inverse_orthogonal[0], v, TOL, "inverse OQ");
    near(r.cell_edge_length_ratio[0], 2f64.sqrt(), TOL, "edge ratio");
    near(r.cell_warp[0], 0.0, TOL, "warp");
    near(r.cell_size_change[0], 1.0, TOL, "size change");
    let a = (2f64.sqrt() / 3.0).atan().to_degrees();
    existing_measures(&m, a, 0.75 * a, 2f64.sqrt(), TOL);
    println!("Q1b ortho skew {} (exact {v})", r.cell_ortho_skew[0]);
}

#[test]
fn q1c_regular_tetrahedron() {
    let m = tet([[1.0, 1.0, 1.0], [1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0]]);
    let r = cell_measures(&m);
    near(r.cell_face_angle_min[0], 60.0, ATOL, "amin");
    near(r.cell_face_angle_max[0], 60.0, ATOL, "amax");
    near(r.cell_inverse_orthogonal[0], 0.0, TOL, "inverse OQ");
    near(r.cell_ortho_skew[0], 0.0, TOL, "ortho skew");
    near(r.cell_edge_length_ratio[0], 1.0, TOL, "edge ratio");
    near(r.cell_warp[0], 0.0, TOL, "warp");
    near(r.cell_size_change[0], 1.0, TOL, "size change");
    existing_measures(&m, 0.0, 0.0, 1.0, TOL);
}

#[test]
fn q1d_sheared_hexahedron() {
    for s in [1.0f64, 2.0] {
        let m = sheared_hex(s);
        let r = cell_measures(&m);
        let a = (s / (1.0 + s * s).sqrt()).acos().to_degrees();
        near(r.cell_face_angle_min[0], a, ATOL, "amin");
        near(r.cell_face_angle_max[0], 180.0 - a, ATOL, "amax");
        let v = 1.0 - 1.0 / (1.0 + s * s).sqrt();
        near(r.cell_ortho_skew[0], v, TOL, "ortho skew");
        near(r.cell_inverse_orthogonal[0], v, TOL, "inverse OQ");
        near(r.cell_edge_length_ratio[0], (1.0 + s * s).sqrt(), TOL, "edge ratio");
        near(r.cell_warp[0], 0.0, TOL, "warp");
        println!(
            "Q1d s={s}: angles {a} / {}, skew {} (exact {v})",
            180.0 - a,
            r.cell_ortho_skew[0]
        );
    }
}

#[test]
fn q1e_shifted_stack() {
    let m = shifted_stack();
    let r = cell_measures(&m);
    let v = 1.0 - 2.0 / 5f64.sqrt();
    for c in 0..2 {
        near(r.cell_inverse_orthogonal[c], v, TOL, "inverse OQ");
        near(r.cell_ortho_skew[c], v, TOL, "ortho skew");
        near(r.cell_size_change[c], 1.0, TOL, "size change");
    }
}

#[test]
fn q1f_neighbour_only_skew() {
    let m = neighbour_pair();
    let r = cell_measures(&m);
    let a = 1.0 - 2.0 / 5f64.sqrt();
    let b = 1.0 - 1.0 / 2f64.sqrt();
    near(r.cell_ortho_skew[0], 0.0, TOL, "cell 0 ortho skew");
    near(r.cell_inverse_orthogonal[0], a, TOL, "cell 0 inverse OQ");
    near(r.cell_ortho_skew[1], b, TOL, "cell 1 ortho skew");
    near(r.cell_inverse_orthogonal[1], b, TOL, "cell 1 inverse OQ");
    near(r.cell_face_angle_min[0], 90.0, ATOL, "cell 0 amin");
    near(r.cell_face_angle_max[0], 90.0, ATOL, "cell 0 amax");
    near(r.cell_face_angle_min[1], 45.0, ATOL, "cell 1 amin");
    near(r.cell_face_angle_max[1], 135.0, ATOL, "cell 1 amax");
}

#[test]
fn q1g_warped_hexahedron() {
    for (h, exact) in [(0.5f64, 0.8f64.acos().to_degrees()), (1.0, 60.0)] {
        let mut p = unit_corners();
        p[6][2] += h; // corner (0,1,1)
        let m = hex(p);
        let r = cell_measures(&m);
        near(r.cell_warp[0], exact, ATOL, "cell warp");
        let nonzero = r.face_warp.iter().filter(|&&w| w > 1e-12).count();
        assert_eq!(nonzero, 1, "only the top face is non-planar: {:?}", r.face_warp);
        let top = r.face_warp.iter().copied().fold(0.0, f64::max);
        near(top, exact, ATOL, "top face warp");
        println!("Q1g h={h}: warp {top} (exact {exact})");
    }
    // The pure polygon function: both split angles, max taken.
    let quad = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 1.0]];
    near(polygon_warp(&quad), 60.0, ATOL, "quad warp");
    near(
        (1.0f64 / 3f64.sqrt()).acos().to_degrees(),
        54.7356103,
        1e-6,
        "other split value",
    );
}

/// Record: coded with relative 1e-9, looser than the registered absolute 1e-12 for dimensionless ratios.
/// The corrected test is `q1h_size_change_registered_tolerance`.
#[test]
fn q1h_size_change() {
    let m = box_mesh(
        [2, 1, 1],
        [1.0; 3],
        [Grading::Expansion(4.0), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let r = cell_measures(&m);
    rel(r.cell_size_change[0], 4.0, 1e-9, "cell 0");
    rel(r.cell_size_change[1], 0.25, 1e-9, "cell 1");
    let mx = r.cell_size_change.iter().copied().fold(0.0, f64::max);
    near(
        mx,
        check_mesh(&m).max_volume_ratio,
        TOL,
        "max equals existing volume ratio",
    );
}

#[test]
fn q1h_size_change_registered_tolerance() {
    let m = box_mesh(
        [2, 1, 1],
        [1.0; 3],
        [Grading::Expansion(4.0), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let r = cell_measures(&m);
    near(r.cell_size_change[0], 4.0, TOL, "cell 0");
    near(r.cell_size_change[1], 0.25, TOL, "cell 1");
    let mx = r.cell_size_change.iter().copied().fold(0.0, f64::max);
    near(
        mx,
        check_mesh(&m).max_volume_ratio,
        TOL,
        "max equals existing volume ratio",
    );
}

type Map = Box<dyn Fn(Vec3) -> Vec3>;

#[test]
fn q1i_invariance_under_rigid_and_uniform_maps() {
    let base = shifted_stack();
    let r0 = compute_measures(&base);
    let axis = {
        let a = [1.0f64, -2.0, 0.5];
        let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        [a[0] / n, a[1] / n, a[2] / n]
    };
    let ang: f64 = 0.83;
    let (c, s) = (ang.cos(), ang.sin());
    let rot = move |p: Vec3| -> Vec3 {
        // Rodrigues
        let k = axis;
        let kxp = [
            k[1] * p[2] - k[2] * p[1],
            k[2] * p[0] - k[0] * p[2],
            k[0] * p[1] - k[1] * p[0],
        ];
        let kp = k[0] * p[0] + k[1] * p[1] + k[2] * p[2];
        [
            p[0] * c + kxp[0] * s + k[0] * kp * (1.0 - c),
            p[1] * c + kxp[1] * s + k[1] * kp * (1.0 - c),
            p[2] * c + kxp[2] * s + k[2] * kp * (1.0 - c),
        ]
    };
    let maps: Vec<(&str, Map)> = vec![
        ("translation", Box::new(|p| [p[0] + 3.0, p[1] - 1.5, p[2] + 0.25])),
        ("rotation", Box::new(rot)),
        ("scale 3.7", Box::new(|p| [3.7 * p[0], 3.7 * p[1], 3.7 * p[2]])),
    ];
    for (name, f) in maps {
        let m = warp(&base, f).unwrap();
        let r = compute_measures(&m);
        for (label, a, b) in [
            ("warp", &r0.cell_warp, &r.cell_warp),
            ("amin", &r0.cell_face_angle_min, &r.cell_face_angle_min),
            ("amax", &r0.cell_face_angle_max, &r.cell_face_angle_max),
            ("ortho_skew", &r0.cell_ortho_skew, &r.cell_ortho_skew),
            (
                "inverse_orthogonal",
                &r0.cell_inverse_orthogonal,
                &r.cell_inverse_orthogonal,
            ),
            ("size_change", &r0.cell_size_change, &r.cell_size_change),
            ("edge_ratio", &r0.cell_edge_length_ratio, &r.cell_edge_length_ratio),
        ] {
            for (x, y) in a.iter().zip(b) {
                assert!((x - y).abs() <= 1e-9 * x.abs().max(1.0), "{name} {label}: {x} vs {y}");
            }
        }
    }
}

#[test]
fn q2a_sliver_hexahedron() {
    let m = box_mesh([1, 1, 1], [1.0, 1.0, 0.001], U).unwrap();
    m.validate().unwrap();
    let r = cell_measures(&m);
    rel(r.cell_edge_length_ratio[0], 1000.0, 1e-9, "edge ratio");
    near(r.cell_face_angle_min[0], 90.0, ATOL, "amin");
    near(r.cell_face_angle_max[0], 90.0, ATOL, "amax");
    near(r.cell_inverse_orthogonal[0], 0.0, TOL, "inverse OQ");
    let sides = r.face_edge_ratio.iter().filter(|&&e| e > 10.0).count();
    assert_eq!(sides, 4);
    // ADR 0093: the remaining ADR 0018 measures and the four pre-existing ones on the degenerate cell.
    near(r.cell_warp[0], 0.0, TOL, "warp");
    assert!(
        r.face_warp.iter().all(|&w| w.abs() <= TOL),
        "face warp {:?}",
        r.face_warp
    );
    near(r.cell_ortho_skew[0], 0.0, TOL, "ortho skew");
    near(r.cell_size_change[0], 1.0, TOL, "size change");
    existing_measures(&m, 0.0, 0.0, 1000.0, 1e-9 * 1000.0);
}

#[test]
fn q2b_collapsed_quad() {
    let q = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    assert_eq!(polygon_edge_ratio(&q), f64::INFINITY);
    let (lo, hi) = polygon_angles(&q);
    near(lo, 45.0, ATOL, "min angle");
    near(hi, 90.0, ATOL, "max angle");
    near(polygon_warp(&q), 0.0, TOL, "warp");
}

#[test]
fn q2c_collinear_triangle() {
    let t = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
    let (lo, hi) = polygon_angles(&t);
    near(lo, 0.0, ATOL, "min angle");
    near(hi, 180.0, ATOL, "max angle");
    near(polygon_edge_ratio(&t), 2.0, TOL, "edge ratio");
    near(polygon_warp(&t), 0.0, TOL, "warp");
}

/// Set-up defect, kept as a record: the neighbour centre of the second face is on the INWARD side of
/// its outward area vector, so `cos beta = -1` and the inverse orthogonal quality is 2, not 1. The
/// corrected test is `q2d_zero_volume_cell_corrected`.
#[test]
#[ignore = "set-up defect (inconsistent neighbour position); superseded by q2d_zero_volume_cell_corrected"]
fn q2d_zero_volume_cell() {
    let c = [0.5, 0.5, 0.5];
    let faces = [
        CellFace {
            centre: c,
            area_out: [0.0, 0.0, 1.0],
            neighbour_centre: None,
        },
        CellFace {
            centre: c,
            area_out: [0.0, 0.0, -1.0],
            neighbour_centre: Some([0.5, 0.5, 1.5]),
        },
    ];
    let (os, io) = cell_orthogonality(c, &faces);
    assert_eq!(os, 1.0);
    assert_eq!(io, 1.0);
}

#[test]
fn q2d_zero_volume_cell_corrected() {
    let c = [0.5, 0.5, 0.5];
    let faces = [
        CellFace {
            centre: c,
            area_out: [0.0, 0.0, 1.0],
            neighbour_centre: None,
        },
        CellFace {
            centre: c,
            area_out: [0.0, 0.0, -1.0],
            neighbour_centre: Some([0.5, 0.5, -0.5]),
        },
    ];
    let (os, io) = cell_orthogonality(c, &faces);
    assert_eq!(os, 1.0);
    assert_eq!(io, 1.0);
}

#[test]
fn q2e_no_nan_and_report_round_trip() {
    let sliver = box_mesh([1, 1, 1], [1.0, 1.0, 0.001], U).unwrap();
    let r = check_mesh(&sliver);
    let back: QualityReport = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(r, back);
}

#[test]
fn q3_poor_mesh_list() {
    // P1: sliver with default limits.
    let sliver = box_mesh([1, 1, 1], [1.0, 1.0, 0.001], U).unwrap();
    let r = check_mesh(&sliver);
    assert_eq!(r.poor_counts.len(), 1, "{:?}", r.poor_counts);
    assert_eq!(r.poor_counts["edge_length_ratio"], 4);
    assert_eq!(r.poor_list.len(), 4);
    for e in &r.poor_list {
        assert_eq!(e.measure, "edge_length_ratio");
        assert_eq!(e.element, "face");
        rel(e.value, 1000.0, 1e-9, "value");
        assert_eq!(e.limit, 100.0);
    }
    assert_eq!(r.poor_warnings.len(), 1);
    // P2: shifted stack and sheared hex with default limits are clean.
    for m in [shifted_stack(), sheared_hex(1.0)] {
        let r = check_mesh(&m);
        assert!(r.poor_list.is_empty() && r.poor_warnings.is_empty() && r.poor_counts.is_empty());
    }
    // P3: neighbour-only pair with lowered orthogonality limits: exactly cell 1.
    let th = Thresholds {
        inverse_orthogonal_warn: 0.25,
        ortho_skew_warn: 0.25,
        ..Thresholds::default()
    };
    let r = check_mesh_with(&neighbour_pair(), &th);
    assert_eq!(r.poor_list.len(), 2, "{:?}", r.poor_list);
    for e in &r.poor_list {
        assert!(e.measure == "inverse_orthogonal" || e.measure == "ortho_skew");
        assert_eq!((e.element.as_str(), e.index), ("cell", 1));
        near(e.value, 1.0 - 1.0 / 2f64.sqrt(), TOL, "value");
    }
    // P4: cap and order.
    let th = Thresholds {
        edge_length_ratio_warn: 1.5,
        ..Thresholds::default()
    };
    let m = box_mesh([2, 2, 2], [4.0, 2.0, 2.0], U).unwrap();
    let r = check_mesh_with(&m, &th);
    assert_eq!(r.poor_counts["edge_length_ratio"], 24);
    assert_eq!(r.poor_list.len(), 10);
    assert!(r.poor_list.iter().all(|e| (e.value - 2.0).abs() < TOL));
    // P5: strongly graded two-cell mesh.
    let m = box_mesh(
        [2, 1, 1],
        [1.0; 3],
        [Grading::Expansion(500.0), Grading::Uniform, Grading::Uniform],
    )
    .unwrap();
    let r = check_mesh(&m);
    let e = r
        .poor_list
        .iter()
        .find(|e| e.measure == "size_change")
        .expect("size_change offender");
    assert_eq!(e.index, 0);
    rel(e.value, 500.0, 1e-9, "value");
    assert!(!r.warnings.is_empty());
}

#[test]
fn q4_old_schema_json_parses() {
    let m = box_mesh([2, 1, 1], [2.0, 1.0, 1.0], U).unwrap();
    let r = check_mesh(&m);
    let mut v: serde_json::Value = serde_json::to_value(&r).unwrap();
    let obj = v.as_object_mut().unwrap();
    for k in ["measures", "poor_counts", "poor_list", "poor_warnings"] {
        assert!(obj.remove(k).is_some(), "{k} missing from the new report");
    }
    let old: QualityReport = serde_json::from_value(v).unwrap();
    assert!(old.poor_list.is_empty() && old.poor_counts.is_empty() && old.poor_warnings.is_empty());
    assert_eq!(old.n_cells, r.n_cells);
    // Thresholds from a partial object use the defaults.
    let th: Thresholds = serde_json::from_str("{\"warp_warn\": 5.0}").unwrap();
    assert_eq!(th.warp_warn, 5.0);
    assert_eq!(th.poor_list_cap, 10);
}
