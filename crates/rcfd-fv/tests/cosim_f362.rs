//! F-362 (ADR 0263): `InterfaceMap` weights built from the two interface meshes alone, by nearest-element projection
//! (with the offset correction) and by radial-basis interpolation (Wendland's C^2 function, degree-1 polynomial), on a
//! planar and a cylindrical wall with non-matching meshes; the unmapped-point policy; the model problem of ADR 0256 with
//! the built weights.
//!
//! The limits, set-ups, arithmetic and mutations were registered in ADR 0263 before this file existed. The channel, the
//! set-up CWs, the modal coordinates and the crossings are copied from `cosim_f139.rs` (that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::bc::{FixedValue, Symmetry, ZeroGradient};
use rcfd_fv::convection::ConvScheme;
use rcfd_fv::cosim::{
    coupled_step, interface_rows, piecewise_linear_rows, AtCap, CouplingSettings, FlowWall, Fluid, InterfaceMap,
    MapMethod, MapSettings, MeshMotion, SprungWall, Unmapped,
};
use rcfd_fv::fsi::{relaxation_from_added_mass, Relaxation};
use rcfd_fv::pimple::{PimpleSettings, PimpleSolver};
use rcfd_fv::smoothing::{smooth_displacement, PatchClass, SmoothingMethod};
use rcfd_fv::{FvGeometry, ScalarField, VectorField};
use rcfd_linalg::SolveControl;
use rcfd_mesh::gen::{box_mesh, Grading};
use rcfd_mesh::motion::{moved_mesh, patch_points};
use rcfd_mesh::Mesh;
use std::f64::consts::{PI, TAU};
use std::sync::Arc;

type Rows = Vec<Vec<(usize, f64)>>;

// ---- the two walls (ADR 0263 S3) -----------------------------------------------------------------------------------

/// The two interface meshes of a wall and the registered parameters of its checks.
struct Wall {
    name: &'static str,
    nodes: Vec<Vec3>,
    elements: Vec<Vec<usize>>,
    points: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    /// Support of the radial basis, `3 h`.
    support: f64,
    max_distance: f64,
}

impl Wall {
    fn settings(&self, method: &str) -> MapSettings {
        match method {
            "projection" => MapSettings::projection(self.max_distance),
            "radial basis" => MapSettings::radial_basis(self.support, self.max_distance),
            _ => panic!("no method {method}"),
        }
    }

    fn build(&self, settings: &MapSettings) -> Result<(InterfaceMap, rcfd_fv::cosim::MapReport), String> {
        InterfaceMap::build(&self.nodes, &self.elements, &self.points, self.faces.clone(), settings)
    }

    fn rows(&self, settings: &MapSettings) -> Rows {
        interface_rows(&self.nodes, &self.elements, &self.points, settings)
            .unwrap()
            .0
    }

    /// Largest distance of a wall point from the centroid of the wall points.
    fn size(&self) -> f64 {
        let c = centroid(&self.points);
        self.points
            .iter()
            .map(|p| vec3::norm(vec3::sub(*p, c)))
            .fold(0.0, f64::max)
    }

    fn face_centres(&self) -> Vec<Vec3> {
        self.faces
            .iter()
            .map(|f| centroid(&f.iter().map(|&q| self.points[q]).collect::<Vec<_>>()))
            .collect()
    }
}

const METHODS: [&str; 2] = ["projection", "radial basis"];

fn centroid(pts: &[Vec3]) -> Vec3 {
    vec3::scale(
        pts.iter().fold(vec3::ZERO, |a, p| vec3::add(a, *p)),
        1.0 / pts.len() as f64,
    )
}

/// Quadrilaterals of an `ni x nj` grid of points numbered `i nj + j`, closed around in `i` when `wrap`.
fn grid_quads(ni: usize, nj: usize, wrap: bool) -> Vec<Vec<usize>> {
    let mut quads = Vec::new();
    for i in 0..if wrap { ni } else { ni - 1 } {
        let i1 = (i + 1) % ni;
        for j in 0..nj - 1 {
            quads.push(vec![i * nj + j, i1 * nj + j, i1 * nj + j + 1, i * nj + j + 1]);
        }
    }
    quads
}

fn steps(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|k| lo + (hi - lo) * k as f64 / n as f64).collect()
}

const PLANE_H: f64 = 0.08;

fn plane_point(a: f64, b: f64) -> Vec3 {
    let n = vec3::scale([1.0, 2.0, 2.0], 1.0 / 3.0);
    let e1 = vec3::unit(vec3::cross(n, [0.0, 0.0, 1.0]));
    let e2 = vec3::cross(n, e1);
    vec3::add(vec3::add([0.02, 0.01, -0.03], vec3::scale(e1, a)), vec3::scale(e2, b))
}

/// BP, or BU (`a_lo = -0.3 h`: the first fluid column beyond the structure).
fn plane_wall(a_lo: f64) -> Wall {
    let nodes: Vec<Vec3> = steps(0.0, 0.4, 5)
        .iter()
        .flat_map(|&a| steps(0.0, 0.15, 3).into_iter().map(move |b| plane_point(a, b)))
        .collect();
    let points: Vec<Vec3> = steps(a_lo, 0.4, 13)
        .iter()
        .flat_map(|&a| steps(0.0, 0.15, 5).into_iter().map(move |b| plane_point(a, b)))
        .collect();
    Wall {
        name: if a_lo == 0.0 { "BP" } else { "BU" },
        nodes,
        elements: grid_quads(6, 4, false),
        points,
        faces: grid_quads(14, 6, false),
        support: 3.0 * PLANE_H,
        max_distance: 0.1 * PLANE_H,
    }
}

const CYL_R: f64 = 0.05;
const CYL_H: f64 = 0.025;

fn fluid_theta(j: usize) -> f64 {
    TAU * (j as f64 + 0.37) / 27.0
}

fn cylinder_wall() -> Wall {
    let on = |t: f64, z: f64| [CYL_R * t.cos(), CYL_R * t.sin(), z];
    let nodes: Vec<Vec3> = (0..16)
        .flat_map(|i| {
            steps(0.0, 0.1, 4)
                .into_iter()
                .map(move |z| on(TAU * i as f64 / 16.0, z))
        })
        .collect();
    let points: Vec<Vec3> = (0..27)
        .flat_map(|j| steps(0.0, 0.1, 8).into_iter().map(move |z| on(fluid_theta(j), z)))
        .collect();
    Wall {
        name: "BC",
        nodes,
        elements: grid_quads(16, 5, true),
        points,
        faces: grid_quads(27, 9, true),
        support: 3.0 * CYL_H,
        max_distance: 0.1 * CYL_H,
    }
}

// ---- rigid motion and random data ----------------------------------------------------------------------------------

const CENTRE: Vec3 = [0.1, -0.05, 0.02];

fn rotation() -> [Vec3; 3] {
    let k = vec3::unit([0.2, 0.9, -0.4]);
    let (s, c) = 0.3f64.sin_cos();
    let mut r = [[0.0; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            let kx = [[0.0, -k[2], k[1]], [k[2], 0.0, -k[0]], [-k[1], k[0], 0.0]];
            let kk: f64 = (0..3).map(|m| kx[i][m] * kx[m][j]).sum();
            *v = if i == j { 1.0 } else { 0.0 } + s * kx[i][j] + (1.0 - c) * kk;
        }
    }
    r
}

/// Rigid motion `u(x) = a + (R - I)(x - c)` with the translation and/or the rotation.
fn rigid(translate: bool, rotate: bool) -> impl Fn(Vec3) -> Vec3 {
    let r = rotation();
    move |x| {
        let mut u = if translate {
            [0.3e-3, -1.1e-3, 0.7e-3]
        } else {
            vec3::ZERO
        };
        if rotate {
            let d = vec3::sub(x, CENTRE);
            let rd = [vec3::dot(r[0], d), vec3::dot(r[1], d), vec3::dot(r[2], d)];
            u = vec3::add(u, vec3::sub(rd, d));
        }
        u
    }
}

const MOTIONS: [(&str, bool, bool); 3] = [
    ("translation", true, false),
    ("rotation", false, true),
    ("both", true, true),
];

/// The relative error of the mapped rigid motion at the wall points (against `u(target_p)`).
fn rigid_error(map: &InterfaceMap, wall: &Wall, u: &dyn Fn(Vec3) -> Vec3, target: &[Vec3]) -> f64 {
    let q: Vec<Vec3> = wall.nodes.iter().map(|x| u(*x)).collect();
    let d = map.displacement_to_points(&q).unwrap();
    let scale = wall.points.iter().map(|x| vec3::norm(u(*x))).fold(0.0, f64::max);
    d.iter()
        .zip(target)
        .map(|(d, x)| vec3::norm(vec3::sub(*d, u(*x))))
        .fold(0.0, f64::max)
        / scale
}

/// A fixed pseudo-random sequence in `(-1, 1)` (as `cosim_f139.rs`).
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
    fn vectors(&mut self, n: usize) -> Vec<Vec3> {
        (0..n).map(|_| [self.next(), self.next(), self.next()]).collect()
    }
}

/// Moment about `CENTRE` of forces at points.
fn moment(at: &[Vec3], forces: &[Vec3]) -> Vec3 {
    at.iter().zip(forces).fold(vec3::ZERO, |a, (x, f)| {
        vec3::add(a, vec3::cross(vec3::sub(*x, CENTRE), *f))
    })
}

fn sum(v: &[Vec3]) -> Vec3 {
    v.iter().fold(vec3::ZERO, |a, x| vec3::add(a, *x))
}

/// Relative errors of the resultant, the moment and the work of the transposed map for random data of `seed`.
fn balance(map: &InterfaceMap, wall: &Wall, seed: u64) -> (f64, f64, f64) {
    let mut rng = Lcg(seed);
    let faces = rng.vectors(wall.faces.len());
    let q = rng.vectors(wall.nodes.len());
    let nodal = map.forces_to_nodes(&faces).unwrap();
    let resultant = vec3::norm(vec3::sub(sum(&nodal), sum(&faces))) / faces.iter().map(|f| vec3::norm(*f)).sum::<f64>();
    let centres = wall.face_centres();
    let arms: f64 = centres
        .iter()
        .zip(&faces)
        .map(|(x, f)| vec3::norm(vec3::sub(*x, CENTRE)) * vec3::norm(*f))
        .sum();
    let moment_error = vec3::norm(vec3::sub(moment(&wall.nodes, &nodal), moment(&centres, &faces))) / arms;
    let d_faces = map
        .face_displacements(&map.displacement_to_points(&q).unwrap())
        .unwrap();
    let ws: f64 = nodal.iter().zip(&q).map(|(f, q)| vec3::dot(*f, *q)).sum();
    let terms: Vec<f64> = faces.iter().zip(&d_faces).map(|(f, d)| vec3::dot(*f, *d)).collect();
    let work = (ws - terms.iter().sum::<f64>()).abs() / terms.iter().map(|t| t.abs()).sum::<f64>();
    (resultant, moment_error, work)
}

// ---- acceptance: rigid motion and conservation on the two walls ----------------------------------------------------

#[test]
fn r1_rigid_motion_is_reproduced_on_both_walls_by_both_methods() {
    for wall in [plane_wall(0.0), cylinder_wall()] {
        for method in METHODS {
            let (map, report) = wall.build(&wall.settings(method)).unwrap();
            let rows = wall.rows(&wall.settings(method));
            let smallest = rows.iter().flatten().map(|r| r.1).fold(f64::MAX, f64::min);
            for (motion, t, r) in MOTIONS {
                let e = rigid_error(&map, &wall, &rigid(t, r), &wall.points);
                println!(
                    "R1 {} {method:12} {motion:11}: error {e:.2e} (smallest weight {smallest:.4}, condition {:?})",
                    wall.name, report.condition
                );
                assert!(e <= 1e-12, "{} {method} {motion}: {e:e}", wall.name);
            }
        }
    }
}

#[test]
fn r2_rows_sum_to_one_and_reproduce_the_coordinates() {
    for wall in [plane_wall(0.0), cylinder_wall()] {
        for method in METHODS {
            let settings = wall.settings(method);
            let (rows, report) = interface_rows(&wall.nodes, &wall.elements, &wall.points, &settings).unwrap();
            let row_sum = rows
                .iter()
                .map(|r| (r.iter().map(|x| x.1).sum::<f64>() - 1.0).abs())
                .fold(0.0, f64::max);
            let precision = rows
                .iter()
                .zip(&wall.points)
                .map(|(r, x)| {
                    let image = r
                        .iter()
                        .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(wall.nodes[i], w)));
                    vec3::norm(vec3::sub(image, *x))
                })
                .fold(0.0, f64::max)
                / wall.size();
            let defect = report.precision_defect / wall.size();
            println!(
                "R2 {} {method:12}: row sums {row_sum:.2e}, coordinates {precision:.2e}, report {defect:.2e}",
                wall.name
            );
            assert!(row_sum <= 1e-13);
            assert!(precision <= 1e-13);
            assert!(defect <= 1e-13);
        }
    }
}

#[test]
fn c1_c2_c3_the_transposed_map_conserves_force_moment_and_work() {
    for wall in [plane_wall(0.0), cylinder_wall()] {
        for method in METHODS {
            let (map, _) = wall.build(&wall.settings(method)).unwrap();
            for seed in [1, 2, 3] {
                let (f, m, w) = balance(&map, &wall, seed);
                println!(
                    "C {} {method:12} seed {seed}: resultant {f:.2e}, moment {m:.2e}, work {w:.2e}",
                    wall.name
                );
                assert!(f <= 1e-12, "C1");
                assert!(m <= 1e-12, "C2");
                assert!(w <= 1e-12, "C3");
            }
        }
    }
}

// ---- acceptance: the unmapped-point policy -------------------------------------------------------------------------

#[test]
fn u1_a_point_outside_the_structure_is_refused() {
    let wall = plane_wall(-0.3 * PLANE_H);
    for method in METHODS {
        let e = wall.build(&wall.settings(method)).unwrap_err();
        println!("U1 {method}: {e}");
        assert!(
            e.contains("wall point 0 ") && e.contains("0.024") && e.contains("0.008"),
            "{e}"
        );
    }
}

/// The nearest structural point of a BU wall point (the outside column moves to `a = 0`).
fn bu_feet(wall: &Wall) -> Vec<Vec3> {
    let column: Vec<Vec3> = steps(0.0, 0.15, 5).into_iter().map(|b| plane_point(0.0, b)).collect();
    wall.points
        .iter()
        .enumerate()
        .map(|(p, x)| if p < 6 { column[p] } else { *x })
        .collect()
}

#[test]
fn u2_u3_a_point_outside_the_structure_follows_its_nearest_structural_point() {
    let wall = plane_wall(-0.3 * PLANE_H);
    let feet = bu_feet(&wall);
    for method in METHODS {
        let mut settings = wall.settings(method);
        settings.unmapped = Unmapped::Nearest;
        let (map, report) = wall.build(&settings).unwrap();
        println!(
            "U2 {method}: outside {:?}, distances {:?}",
            report.outside,
            &report.distances[..7]
        );
        assert_eq!(report.outside, (0..6).collect::<Vec<_>>());
        for (motion, t, r) in MOTIONS {
            let e = rigid_error(&map, &wall, &rigid(t, r), &feet);
            println!("U2 {method:12} {motion:11}: error against the nearest points {e:.2e}");
            assert!(e <= 1e-12);
        }
        let e = rigid_error(&map, &wall, &rigid(true, false), &wall.points);
        assert!(e <= 1e-12, "translation at the points themselves: {e:e}");
        // U3
        let mut rng = Lcg(11);
        let faces = rng.vectors(wall.faces.len());
        let nodal = map.forces_to_nodes(&faces).unwrap();
        let total: f64 = faces.iter().map(|f| vec3::norm(*f)).sum();
        let resultant = vec3::norm(vec3::sub(sum(&nodal), sum(&faces))) / total;
        let mut shares = vec![vec3::ZERO; wall.points.len()];
        for (pts, f) in wall.faces.iter().zip(&faces) {
            for &q in pts {
                shares[q] = vec3::add(shares[q], vec3::scale(*f, 1.0 / pts.len() as f64));
            }
        }
        let predicted = report.outside.iter().fold(vec3::ZERO, |a, &p| {
            vec3::add(a, vec3::cross(vec3::sub(wall.points[p], feet[p]), shares[p]))
        });
        let centres = wall.face_centres();
        let arms: f64 = centres
            .iter()
            .zip(&faces)
            .map(|(x, f)| vec3::norm(vec3::sub(*x, CENTRE)) * vec3::norm(*f))
            .sum();
        let defect = vec3::sub(moment(&centres, &faces), moment(&wall.nodes, &nodal));
        let mismatch = vec3::norm(vec3::sub(defect, predicted)) / arms;
        println!(
            "U3 {method}: resultant {resultant:.2e}; moment defect {:.3e} against the predicted {:.3e} (mismatch {mismatch:.2e})",
            vec3::norm(defect) / arms,
            vec3::norm(predicted) / arms
        );
        assert!(resultant <= 1e-12);
        assert!(mismatch <= 1e-12);
        assert!(vec3::norm(predicted) / arms >= 1e-6);
    }
}

// ---- details: distances, conditioning, errors, the negative control ------------------------------------------------

#[test]
fn g1_distances_are_those_of_the_closed_form() {
    let wall = cylinder_wall();
    let (_, report) = wall.build(&wall.settings("projection")).unwrap();
    let dtheta = TAU / 16.0;
    let mut worst: f64 = 0.0;
    let mut largest: f64 = 0.0;
    for (p, d) in report.distances.iter().enumerate() {
        let theta = fluid_theta(p / 9);
        let mid = ((theta / dtheta).floor() + 0.5) * dtheta;
        let exact = CYL_R * (theta - mid).cos() - CYL_R * (0.5 * dtheta).cos();
        worst = worst.max((d - exact).abs());
        largest = largest.max(*d);
    }
    let plane = plane_wall(0.0);
    let (_, rp) = plane.build(&plane.settings("projection")).unwrap();
    let flat = rp.distances.iter().copied().fold(0.0, f64::max);
    println!("G1: BC distances against the closed form {worst:.2e} m (largest {largest:.4e}); BP largest {flat:.2e} m");
    assert!(worst <= 1e-15);
    assert!(flat <= 1e-15);
}

fn cw_nodes() -> Vec<Vec3> {
    cw_node_y().iter().map(|y| [0.0, *y, 0.5 * CW_LZ]).collect()
}

fn cw_elements() -> Vec<Vec<usize>> {
    (0..CW_NODES - 1).map(|i| vec![i, i + 1]).collect()
}

fn cw_map_settings(method: &str) -> MapSettings {
    match method {
        "projection" => MapSettings::projection(CW_MAX_DISTANCE),
        "radial basis" => MapSettings::radial_basis(CW_SUPPORT, CW_MAX_DISTANCE),
        _ => panic!("no method {method}"),
    }
}

fn cw_wall_points(ch: &Channel) -> Vec<Vec3> {
    ch.wall.iter().map(|&q| ch.mesh0.points()[q]).collect()
}

#[test]
fn k1_the_condition_estimate_matches_the_dense_eigenvalues() {
    let ch = cw_channel();
    let (_, model) = interface_rows(
        &cw_nodes(),
        &cw_elements(),
        &cw_wall_points(&ch),
        &cw_map_settings("radial basis"),
    )
    .unwrap();
    let plane = plane_wall(0.0);
    let cyl = cylinder_wall();
    let (_, bp) = plane.build(&plane.settings("radial basis")).unwrap();
    let (_, bc) = cyl.build(&cyl.settings("radial basis")).unwrap();
    for (name, report, reference) in [("model", model, 8.208), ("BP", bp, 85.20), ("BC", bc, 69.86)] {
        let k = report.condition.unwrap();
        println!("K1 {name}: condition estimate {k:.4} against {reference}");
        assert!((k - reference).abs() <= 0.05 * reference);
        assert!(k <= 1.0001 * reference);
    }
}

/// The registered E1 (ADR 0263 amendment 1): the coincident-node case was accepted by the absolute pivot test. Kept with
/// its limit as the record.
#[test]
#[ignore = "known miss of E1 (ADR 0263 amendment 1): coincident nodes accepted by the registered pivot test"]
fn e1_invalid_meshes_and_settings_are_refused_known_miss() {
    let wall = plane_wall(0.0);
    let proj = wall.settings("projection");
    let rbf = wall.settings("radial basis");
    let mut nan_node = wall.nodes.clone();
    nan_node[3][1] = f64::NAN;
    let mut five = wall.elements.clone();
    five[0].push(7);
    let mut out_of_range = wall.elements.clone();
    out_of_range[2][1] = wall.nodes.len();
    let mut twin = wall.nodes.clone();
    twin.push(wall.nodes[5]);
    let bad = |support: f64, max_distance: f64| MapSettings {
        method: MapMethod::RadialBasis { support },
        max_distance,
        unmapped: Unmapped::Refuse,
    };
    let cases: Vec<(&str, Result<_, String>)> = vec![
        ("no node", interface_rows(&[], &wall.elements, &wall.points, &proj)),
        (
            "NaN coordinate",
            interface_rows(&nan_node, &wall.elements, &wall.points, &proj),
        ),
        (
            "element of five nodes",
            interface_rows(&wall.nodes, &five, &wall.points, &proj),
        ),
        (
            "index out of range",
            interface_rows(&wall.nodes, &out_of_range, &wall.points, &proj),
        ),
        ("no element", interface_rows(&wall.nodes, &[], &wall.points, &proj)),
        (
            "support 0",
            interface_rows(&wall.nodes, &wall.elements, &wall.points, &bad(0.0, 0.008)),
        ),
        (
            "support NaN",
            interface_rows(&wall.nodes, &wall.elements, &wall.points, &bad(f64::NAN, 0.008)),
        ),
        (
            "max_distance -1",
            interface_rows(&wall.nodes, &wall.elements, &wall.points, &bad(0.24, -1.0)),
        ),
        (
            "max_distance NaN",
            interface_rows(&wall.nodes, &wall.elements, &wall.points, &bad(0.24, f64::NAN)),
        ),
        (
            "coincident nodes",
            interface_rows(&twin, &wall.elements, &wall.points, &rbf),
        ),
    ];
    for (name, r) in cases {
        match r {
            Err(e) => println!("E1 {name}: {e}"),
            Ok(_) => panic!("E1 {name}: accepted"),
        }
    }
}

/// E1b of ADR 0263 amendment 1: the cases of E1 with the amended pivot test.
#[test]
fn e1b_invalid_meshes_and_settings_are_refused_amended() {
    let wall = plane_wall(0.0);
    let proj = wall.settings("projection");
    let rbf = wall.settings("radial basis");
    let mut nan_node = wall.nodes.clone();
    nan_node[3][1] = f64::NAN;
    let mut five = wall.elements.clone();
    five[0].push(7);
    let mut out_of_range = wall.elements.clone();
    out_of_range[2][1] = wall.nodes.len();
    let mut twin = wall.nodes.clone();
    twin.push(wall.nodes[5]);
    let bad = |support: f64, max_distance: f64| MapSettings {
        method: MapMethod::RadialBasis { support },
        max_distance,
        unmapped: Unmapped::Refuse,
    };
    let cases: Vec<(&str, Result<_, String>)> = vec![
        ("no node", interface_rows(&[], &wall.elements, &wall.points, &proj)),
        (
            "NaN coordinate",
            interface_rows(&nan_node, &wall.elements, &wall.points, &proj),
        ),
        (
            "element of five nodes",
            interface_rows(&wall.nodes, &five, &wall.points, &proj),
        ),
        (
            "index out of range",
            interface_rows(&wall.nodes, &out_of_range, &wall.points, &proj),
        ),
        ("no element", interface_rows(&wall.nodes, &[], &wall.points, &proj)),
        (
            "support 0",
            interface_rows(&wall.nodes, &wall.elements, &wall.points, &bad(0.0, 0.008)),
        ),
        (
            "support NaN",
            interface_rows(&wall.nodes, &wall.elements, &wall.points, &bad(f64::NAN, 0.008)),
        ),
        (
            "max_distance -1",
            interface_rows(&wall.nodes, &wall.elements, &wall.points, &bad(0.24, -1.0)),
        ),
        (
            "max_distance NaN",
            interface_rows(&wall.nodes, &wall.elements, &wall.points, &bad(0.24, f64::NAN)),
        ),
        (
            "coincident nodes",
            interface_rows(&twin, &wall.elements, &wall.points, &rbf),
        ),
    ];
    for (name, r) in cases {
        match r {
            Err(e) => {
                println!("E1b {name}: {e}");
                if name == "coincident nodes" {
                    assert!(e.contains("radial-basis matrix"), "{e}");
                }
            }
            Ok(_) => panic!("E1b {name}: accepted"),
        }
    }
}

#[test]
fn n1_without_the_offset_correction_the_cylinder_loses_rotation_and_moment() {
    let wall = cylinder_wall();
    let settings = MapSettings {
        method: MapMethod::Projection { correct_offset: false },
        ..MapSettings::projection(wall.max_distance)
    };
    let (map, report) = wall.build(&settings).unwrap();
    let e = rigid_error(&map, &wall, &rigid(false, true), &wall.points);
    let (f, m, w) = balance(&map, &wall, 1);
    println!(
        "N1: rotation error {e:.3e}; resultant {f:.2e}, moment {m:.3e}, work {w:.2e}; precision defect {:.3e} m",
        report.precision_defect
    );
    assert!(e >= 1e-4);
    assert!(m >= 1e-5);
}

// ---- the channel of ADR 0256 (copied) ------------------------------------------------------------------------------

const RHO: f64 = 1.0;
const WALL: &str = "xmin";

fn tight(max_iter: usize) -> SolveControl {
    SolveControl {
        tol_abs: 1e-16,
        tol_rel: 1e-13,
        max_iter,
    }
}

struct Channel {
    mesh0: Mesh,
    g0: FvGeometry,
    classes: Vec<PatchClass>,
    wall: Vec<usize>,
    nu: f64,
}

impl Channel {
    fn new(cells: [usize; 3], size: [f64; 3], nu: f64) -> Arc<Self> {
        let mesh0 = box_mesh(cells, size, [Grading::Uniform; 3]).unwrap();
        let g0 = FvGeometry::new(Arc::new(mesh0.clone())).unwrap();
        let classes = mesh0
            .patches()
            .iter()
            .map(|pt| match pt.name.as_str() {
                "xmin" => PatchClass::Prescribed,
                "xmax" => PatchClass::Fixed,
                _ => PatchClass::Slip,
            })
            .collect();
        let wall = patch_points(&mesh0, WALL).unwrap();
        Arc::new(Channel {
            mesh0,
            g0,
            classes,
            wall,
            nu,
        })
    }

    fn points(&self, d: &[Vec3]) -> Result<Vec<Vec3>, String> {
        let mut presc = vec![vec3::ZERO; self.mesh0.n_points()];
        for (&q, v) in self.wall.iter().zip(d) {
            presc[q] = *v;
        }
        let sm = smooth_displacement(
            &self.g0,
            &self.classes,
            &presc,
            SmoothingMethod::Laplacian,
            &tight(5000),
        )
        .map_err(|e| e.to_string())?;
        Ok(self
            .mesh0
            .points()
            .iter()
            .zip(&sm.displacement)
            .map(|(p, d)| vec3::add(*p, *d))
            .collect())
    }

    fn settings(&self) -> PimpleSettings {
        let mut s = PimpleSettings::piso(self.nu);
        s.convection = ConvScheme::Central;
        s.momentum_solver = tight(5000);
        s.pressure_solver = tight(5000);
        s
    }

    /// Fluid at rest on the mesh with the wall points displaced by `d`; `ymin`, `ymax` slip walls (CWs).
    fn flow(&self, d: &[Vec3]) -> PimpleSolver {
        let mesh = moved_mesh(&self.mesh0, self.points(d).unwrap()).unwrap();
        let geom = Arc::new(FvGeometry::new(Arc::new(mesh)).unwrap());
        let mut u = VectorField::uniform("U", &geom, vec3::ZERO);
        for name in ["ymin", "ymax"] {
            u.set_bc(&geom, name, Box::new(Symmetry)).unwrap();
        }
        for name in ["zmin", "zmax"] {
            u.set_bc(&geom, name, Box::new(ZeroGradient)).unwrap();
        }
        let mut p = ScalarField::uniform("p", &geom, 0.0);
        p.set_bc(&geom, "xmax", Box::new(FixedValue::scalar(0.0))).unwrap();
        PimpleSolver::new(geom, u, p, self.settings())
    }

    fn wall_y(&self) -> Vec<f64> {
        self.wall.iter().map(|&q| self.mesh0.points()[q][1]).collect()
    }
}

fn motion(ch: &Arc<Channel>) -> MeshMotion {
    let ch = ch.clone();
    Box::new(move |d| ch.points(d))
}

// ---- set-up CWs (ADR 0256 P3, G1, amendment 1; copied) and the built weights (ADR 0263 S3) -------------------------

const CW_L: f64 = 1.0;
const CW_W: f64 = 0.4;
const CW_LZ: f64 = 0.1;
const CW_MU: f64 = 4.0;
const CW_X: f64 = 0.002;
const CW_NU: f64 = 1e-4;
const CW_NODES: usize = 6;
const CW_DT: f64 = 0.01;
const CW_STEPS: usize = 650;
/// `3 h`, `h = 0.08` (ADR 0263 D6).
const CW_SUPPORT: f64 = 0.24;
/// `0.6 Lz` (ADR 0263 D7).
const CW_MAX_DISTANCE: f64 = 0.06;

fn cw_m() -> f64 {
    RHO * CW_L / CW_MU
}

fn cw_k() -> f64 {
    TAU * TAU * (cw_m() + RHO * CW_L)
}

fn cw_omega(j: usize) -> f64 {
    let l = if j == 0 {
        CW_L
    } else {
        let kappa = j as f64 * PI / CW_W;
        (kappa * CW_L).tanh() / kappa
    };
    (cw_k() / (cw_m() + RHO * l)).sqrt()
}

fn cw_channel() -> Arc<Channel> {
    Channel::new([20, 8, 2], [CW_L, CW_W, CW_LZ], CW_NU)
}

fn cw_node_y() -> Vec<f64> {
    (0..CW_NODES).map(|i| CW_W * i as f64 / (CW_NODES - 1) as f64).collect()
}

fn cw_tributary() -> Vec<f64> {
    let h = CW_W / (CW_NODES - 1) as f64;
    (0..CW_NODES)
        .map(|i| if i == 0 || i == CW_NODES - 1 { 0.5 * h } else { h })
        .collect()
}

fn cw_q0() -> Vec<f64> {
    cw_node_y()
        .iter()
        .map(|y| CW_X * (1.0 + 0.5 * (PI * y / CW_W).cos()))
        .collect()
}

fn cw_structure() -> SprungWall {
    let t = cw_tributary();
    SprungWall::new(
        cw_nodes(),
        [1.0, 0.0, 0.0],
        t.iter().map(|t| cw_m() * CW_LZ * t).collect(),
        t.iter().map(|t| cw_k() * CW_LZ * t).collect(),
        cw_q0(),
    )
    .unwrap()
}

/// The rows of a CWs run: `piecewise_linear_rows` (ADR 0256) or a builder of ADR 0263.
fn cw_rows(ch: &Channel, method: &str) -> Rows {
    match method {
        "piecewise linear" => piecewise_linear_rows(&cw_node_y(), &ch.wall_y()).unwrap(),
        _ => {
            interface_rows(
                &cw_nodes(),
                &cw_elements(),
                &cw_wall_points(ch),
                &cw_map_settings(method),
            )
            .unwrap()
            .0
        }
    }
}

fn apply(rows: &Rows, q: &[Vec3]) -> Vec<Vec3> {
    rows.iter()
        .map(|row| {
            row.iter()
                .fold(vec3::ZERO, |a, &(i, w)| vec3::add(a, vec3::scale(q[i], w)))
        })
        .collect()
}

fn cw_settings() -> CouplingSettings {
    CouplingSettings {
        relaxation: Relaxation::Aitken {
            initial: relaxation_from_added_mass(cw_m(), RHO * CW_L),
            max: 2.0,
        },
        tolerance: 1e-8,
        force_scale: cw_k() * CW_X * CW_W * CW_LZ,
        max_iterations: 20,
        at_cap: AtCap::Stop,
    }
}

fn modal(q: &[f64]) -> (f64, f64) {
    let (t, y) = (cw_tributary(), cw_node_y());
    let c: Vec<f64> = y.iter().map(|y| (PI * y / CW_W).cos()).collect();
    let q0 = q.iter().zip(&t).map(|(q, t)| q * t).sum::<f64>() / t.iter().sum::<f64>();
    let q1 = q.iter().zip(&t).zip(&c).map(|((q, t), c)| q * t * c).sum::<f64>()
        / t.iter().zip(&c).map(|(t, c)| t * c * c).sum::<f64>();
    (q0, q1)
}

fn upward_crossings(t: &[f64], x: &[f64]) -> Vec<f64> {
    (1..x.len())
        .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
        .map(|i| t[i - 1] + (t[i] - t[i - 1]) * (-x[i - 1]) / (x[i] - x[i - 1]))
        .collect()
}

fn measured_omega(t: &[f64], x: &[f64]) -> (f64, usize) {
    let c = upward_crossings(t, x);
    assert!(c.len() >= 2, "fewer than two upward crossings");
    (TAU * (c.len() - 1) as f64 / (c[c.len() - 1] - c[0]), c.len())
}

struct CwRun {
    ratios: (f64, f64),
    crossings: (usize, usize),
    all_converged: bool,
    steps: usize,
    iterations: (usize, f64),
}

fn cw_run(method: &str) -> CwRun {
    let ch = cw_channel();
    let rows = cw_rows(&ch, method);
    let mut structure = cw_structure();
    let x0: Vec<Vec3> = cw_q0().iter().map(|x| [*x, 0.0, 0.0]).collect();
    let flow = ch.flow(&apply(&rows, &x0));
    let mut fluid = FlowWall::new(flow, WALL, &ch.mesh0, RHO, CW_NU, motion(&ch)).unwrap();
    let map = InterfaceMap::new(rows, CW_NODES, fluid.face_points()).unwrap();
    let settings = cw_settings();
    let (a, b) = modal(&structure.q);
    let (mut t, mut q0, mut q1) = (vec![0.0], vec![a], vec![b]);
    let mut run = CwRun {
        ratios: (0.0, 0.0),
        crossings: (0, 0),
        all_converged: true,
        steps: 0,
        iterations: (0, 0.0),
    };
    let mut total = 0;
    for n in 1..=CW_STEPS {
        let step = match coupled_step(&mut fluid, &mut structure, &map, CW_DT, &settings) {
            Ok(s) => s,
            Err(e) => {
                println!("CWs {method}: step {n}: {e}");
                run.all_converged = false;
                break;
            }
        };
        run.all_converged &= step.converged;
        run.steps = n;
        run.iterations.0 = run.iterations.0.max(step.iterations);
        total += step.iterations;
        let (a, b) = modal(&structure.q);
        t.push(structure.t);
        q0.push(a);
        q1.push(b);
    }
    run.iterations.1 = total as f64 / run.steps.max(1) as f64;
    let (w0, n0) = measured_omega(&t, &q0);
    let (w1, n1) = measured_omega(&t, &q1);
    run.ratios = (w0 / cw_omega(0), w1 / cw_omega(1));
    run.crossings = (n0, n1);
    println!(
        "CWs {method}: {} steps, converged {}, iterations max {} mean {:.3}; omega_m/omega Q0 {:.6} ({n0}), Q1 {:.6} ({n1})",
        run.steps, run.all_converged, run.iterations.0, run.iterations.1, run.ratios.0, run.ratios.1
    );
    run
}

#[test]
fn p1_projection_on_the_model_gives_the_piecewise_linear_rows() {
    let ch = cw_channel();
    let reference = cw_rows(&ch, "piecewise linear");
    let (rows, report) = interface_rows(
        &cw_nodes(),
        &cw_elements(),
        &cw_wall_points(&ch),
        &cw_map_settings("projection"),
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    for (a, b) in rows.iter().zip(&reference) {
        for i in 0..CW_NODES {
            let w = |r: &[(usize, f64)]| r.iter().filter(|x| x.0 == i).map(|x| x.1).sum::<f64>();
            worst = worst.max((w(a) - w(b)).abs());
        }
    }
    println!(
        "P1: largest weight difference {worst:.2e}; precision defect {:.17}",
        report.precision_defect
    );
    assert_eq!(rows.len(), reference.len());
    assert!(worst <= 1e-15);
    assert!((report.precision_defect - 0.05).abs() <= 1e-15);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "heavy: run in release")]
fn f0_f1_the_model_problem_gives_the_same_frequencies_with_the_built_weights() {
    let reference = cw_run("piecewise linear");
    let projection = cw_run("projection");
    let radial = cw_run("radial basis");
    for run in [&reference, &projection, &radial] {
        assert!(run.all_converged && run.steps == CW_STEPS, "F0: every step converged");
    }
    println!(
        "F0: piecewise linear {:.6} {:.6} against ADR 0256's 0.998618 1.011453 ({:+.2e}, {:+.2e})",
        reference.ratios.0,
        reference.ratios.1,
        reference.ratios.0 - 0.998618,
        reference.ratios.1 - 1.011453
    );
    assert!((reference.ratios.0 - 0.998618).abs() <= 1e-6);
    assert!((reference.ratios.1 - 1.011453).abs() <= 1e-6);
    for (name, run) in [("projection", &projection), ("radial basis", &radial)] {
        let (d0, d1) = (run.ratios.0 - reference.ratios.0, run.ratios.1 - reference.ratios.1);
        println!("F1 {name}: differences from piecewise linear {d0:+.3e} (Q0), {d1:+.3e} (Q1)");
        assert!(d0.abs() <= 1e-3, "F1 {name} Q0");
        assert!(d1.abs() <= 1e-3, "F1 {name} Q1");
    }
}
