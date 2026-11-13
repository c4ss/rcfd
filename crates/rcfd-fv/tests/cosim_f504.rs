//! F-504 (ADR 0414): the opt-in bilinear projection on quadrilateral interface elements. The nearest point of a warped
//! bilinear patch against closed forms (W1, W2); rigid motion and the balances of the transposed map on the planar and
//! the cylindrical wall of F-362 (R-B, C-B, R2-B, G-B); the unmapped-point policy (U1-B, U2-B); the settings (V); the
//! negative control N2 (no offset correction on the cylinder).
//!
//! The limits, set-ups and mutations were registered in ADR 0414 before this file existed. The walls BP, BU, BC, the
//! rigid motions and the balance are copied from `cosim_f362.rs` (that file is not edited).

use rcfd_core::vec3::{self, Vec3};
use rcfd_fv::cosim::{interface_rows, InterfaceMap, MapMethod, MapReport, MapSettings, Unmapped};
use std::f64::consts::TAU;

type Rows = Vec<Vec<(usize, f64)>>;

/// Measured values against their limits; every value is printed, the test fails at the end if any missed.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:e} (limit <= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:e} (limit >= {limit:e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:#?}", self.0);
    }
}

// ---- the walls of F-362 (ADR 0263 S3), copied -------------------------------------------------------------------------

struct Wall {
    name: &'static str,
    nodes: Vec<Vec3>,
    elements: Vec<Vec<usize>>,
    points: Vec<Vec3>,
    faces: Vec<Vec<usize>>,
    max_distance: f64,
}

impl Wall {
    fn build(&self, settings: &MapSettings) -> Result<(InterfaceMap, MapReport), String> {
        InterfaceMap::build(&self.nodes, &self.elements, &self.points, self.faces.clone(), settings)
    }

    fn rows(&self, settings: &MapSettings) -> (Rows, MapReport) {
        interface_rows(&self.nodes, &self.elements, &self.points, settings).unwrap()
    }

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

fn centroid(pts: &[Vec3]) -> Vec3 {
    vec3::scale(
        pts.iter().fold(vec3::ZERO, |a, p| vec3::add(a, *p)),
        1.0 / pts.len() as f64,
    )
}

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

/// BP, or BU (`a_lo = -0.3 h`).
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
        max_distance: 0.1 * CYL_H,
    }
}

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

fn bilinear(wall: &Wall) -> MapSettings {
    MapSettings::bilinear_projection(wall.max_distance)
}

// ---- W1, W2: the warped patch (ADR 0414 D4) ---------------------------------------------------------------------------

/// `X(s, t) = (s, t, 0.4 s t)`.
fn warped(s: f64, t: f64) -> Vec3 {
    [s, t, 0.4 * s * t]
}

#[test]
fn w1_w2_nearest_point_on_a_warped_bilinear_patch() {
    let mut checks = Checks::new();
    let nodes = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.4], [0.0, 1.0, 0.0]];
    let elements = vec![vec![0, 1, 2, 3]];
    let settings = MapSettings {
        method: MapMethod::BilinearProjection { correct_offset: false },
        max_distance: 1.0,
        unmapped: Unmapped::Refuse,
    };
    let params = [0.15, 0.5, 0.85];
    // W1
    let mut points = Vec::new();
    let mut expected = Vec::new();
    for &s0 in &params {
        for &t0 in &params {
            let n = vec3::unit([-0.4 * t0, -0.4 * s0, 1.0]);
            for side in [1.0, -1.0] {
                points.push(vec3::add(warped(s0, t0), vec3::scale(n, side * 0.05)));
                expected.push([(1.0 - s0) * (1.0 - t0), s0 * (1.0 - t0), s0 * t0, (1.0 - s0) * t0]);
            }
        }
    }
    let (rows, report) = interface_rows(&nodes, &elements, &points, &settings).unwrap();
    let weight_error = |rows: &Rows, expected: &[[f64; 4]]| {
        rows.iter()
            .zip(expected)
            .map(|(row, e)| {
                let mut full = [0.0; 4];
                for &(i, w) in row {
                    full[i] = w;
                }
                (0..4).map(|k| (full[k] - e[k]).abs()).fold(0.0, f64::max)
            })
            .fold(0.0, f64::max)
    };
    let dist = |report: &MapReport, d: f64| report.distances.iter().map(|x| (x - d).abs()).fold(0.0, f64::max);
    checks.at_most("W1 weights", weight_error(&rows, &expected), 1e-12);
    checks.at_most("W1 distance", dist(&report, 0.05), 1e-13);
    // W2
    let points: Vec<Vec3> = params.iter().map(|&s0| [s0, -0.2, 0.0]).collect();
    let expected: Vec<[f64; 4]> = params.iter().map(|&s0| [1.0 - s0, s0, 0.0, 0.0]).collect();
    let (rows, report) = interface_rows(&nodes, &elements, &points, &settings).unwrap();
    checks.at_most("W2 weights", weight_error(&rows, &expected), 1e-12);
    checks.at_most("W2 distance", dist(&report, 0.2), 1e-13);
    checks.finish();
}

// ---- R-B, C-B, R2-B, G-B: the planar and the cylindrical wall ---------------------------------------------------------

#[test]
fn r_b_c_b_rigid_motion_and_balances_on_the_planar_and_the_cylindrical_wall() {
    let mut checks = Checks::new();
    for wall in [plane_wall(0.0), cylinder_wall()] {
        let (map, _) = wall.build(&bilinear(&wall)).unwrap();
        for (motion, t, r) in MOTIONS {
            let e = rigid_error(&map, &wall, &rigid(t, r), &wall.points);
            checks.at_most(&format!("R-B {} {motion}", wall.name), e, 1e-12);
        }
        for seed in [1, 2, 3] {
            let (f, m, w) = balance(&map, &wall, seed);
            checks.at_most(&format!("C-B {} seed {seed} resultant", wall.name), f, 1e-12);
            checks.at_most(&format!("C-B {} seed {seed} moment", wall.name), m, 1e-12);
            checks.at_most(&format!("C-B {} seed {seed} work", wall.name), w, 1e-12);
        }
        // R2-B
        let (rows, report) = wall.rows(&bilinear(&wall));
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
        checks.at_most(&format!("R2-B {} row sums", wall.name), row_sum, 1e-13);
        checks.at_most(&format!("R2-B {} coordinates", wall.name), precision, 1e-13);
        checks.at_most(
            &format!("R2-B {} report defect", wall.name),
            report.precision_defect / wall.size(),
            1e-13,
        );
        // G-B
        let (_, projection) = wall.rows(&MapSettings::projection(wall.max_distance));
        let g = report
            .distances
            .iter()
            .zip(&projection.distances)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        checks.at_most(
            &format!("G-B {} distances against the projection's [m]", wall.name),
            g,
            1e-15,
        );
    }
    checks.finish();
}

// ---- U1-B, U2-B: the unmapped-point policy ----------------------------------------------------------------------------

#[test]
fn u1_b_u2_b_a_point_outside_the_structure() {
    let mut checks = Checks::new();
    let wall = plane_wall(-0.3 * PLANE_H);
    match wall.build(&bilinear(&wall)) {
        Err(e) => {
            println!("U1-B: {e}");
            checks.holds(
                "U1-B",
                e.contains("wall point 0 ") && e.contains("0.024") && e.contains("0.008"),
                "the message names wall point 0, its distance and max_distance",
            );
        }
        Ok(_) => checks.holds("U1-B", false, "the build is refused"),
    }
    let column: Vec<Vec3> = steps(0.0, 0.15, 5).into_iter().map(|b| plane_point(0.0, b)).collect();
    let feet: Vec<Vec3> = wall
        .points
        .iter()
        .enumerate()
        .map(|(p, x)| if p < 6 { column[p] } else { *x })
        .collect();
    let mut settings = bilinear(&wall);
    settings.unmapped = Unmapped::Nearest;
    let (map, report) = wall.build(&settings).unwrap();
    checks.holds(
        "U2-B outside",
        report.outside == (0..6).collect::<Vec<_>>(),
        &format!("outside {:?} is 0..6", report.outside),
    );
    for (motion, t, r) in MOTIONS {
        let e = rigid_error(&map, &wall, &rigid(t, r), &feet);
        checks.at_most(&format!("U2-B {motion} against the feet"), e, 1e-12);
    }
    let e = rigid_error(&map, &wall, &rigid(true, false), &wall.points);
    checks.at_most("U2-B translation at the points", e, 1e-12);
    checks.finish();
}

// ---- V: the settings --------------------------------------------------------------------------------------------------

#[test]
fn v_the_settings_are_validated_as_the_projection() {
    let mut checks = Checks::new();
    checks.holds(
        "V 0 and 1",
        MapSettings::bilinear_projection(0.0).validate().is_ok()
            && MapSettings::bilinear_projection(1.0).validate().is_ok(),
        "max_distance 0 and 1 accepted",
    );
    for d in [-1.0, f64::NAN] {
        let a = MapSettings::bilinear_projection(d).validate();
        let b = MapSettings::projection(d).validate();
        checks.holds(
            &format!("V {d}"),
            a.is_err() && a == b,
            "refused with the projection's message",
        );
    }
    checks.finish();
}

// ---- N2: the negative control ----------------------------------------------------------------------------------------

#[test]
fn n2_without_the_offset_correction_the_cylinder_loses_rotation_and_moment() {
    let mut checks = Checks::new();
    let wall = cylinder_wall();
    let settings = MapSettings {
        method: MapMethod::BilinearProjection { correct_offset: false },
        max_distance: wall.max_distance,
        unmapped: Unmapped::Refuse,
    };
    let (map, _) = wall.build(&settings).unwrap();
    let e = rigid_error(&map, &wall, &rigid(false, true), &wall.points);
    checks.at_least("N2 BC rotation error", e, 1e-4);
    let m = (1..=3)
        .map(|seed| balance(&map, &wall, seed).1)
        .fold(f64::MAX, f64::min);
    checks.at_least("N2 BC moment defect (smallest of seeds 1 to 3)", m, 1e-5);
    checks.finish();
}
