//! F-352 (ADR 0253): the donor search of the overset coupling on cells with non-planar faces. On the set-up B3d of ADR
//! 0251 (hexahedra whose interior points are displaced in all three directions) every fringe centre gets a donor that
//! agrees with a full search and with an independent containment test, and the linear and quadratic stencils reproduce
//! linear and quadratic fields (A1 to A3); the cells of a warped mesh tile it, counted on random points (S1 to S5); a
//! point on a shared triangle goes to the lowest-numbered cell (T1 to T3); on meshes with planar faces no cell is tested
//! by the new rule and the donors and stencils are those of the rule of ADR 0219 M3, bit for bit (A4); the cost (C1,
//! C2); the present rule fails as recorded (N1, N2).
//!
//! Release: `cargo test --release -p rcfd-mesh --test overset_f352`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::gen::{axis_coords, box_mesh, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, warp, Grading};
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{
    compose, containment_margin, face_warp, linear_weights, pose_inverse_apply, CellKind, CellLocator, Connectivity,
    DonorInterpolation, Overset, OversetSpec, Stencil,
};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

const DEPTH: f64 = 0.1;
/// Background cell size of level 1 of RL and TG_L.
const H1: f64 = PI / 40.0;

/// Records the failed checks and prints every check.
struct Checks(Vec<String>);

impl Checks {
    fn new() -> Self {
        Checks(Vec::new())
    }
    fn at_most(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value <= limit;
        println!(
            "{id}: {value:.4e} (limit <= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} > {limit:e}"));
        }
    }
    fn at_least(&mut self, id: &str, value: f64, limit: f64) {
        let ok = value >= limit;
        println!(
            "{id}: {value:.4e} (limit >= {limit:.3e}) {}",
            if ok { "met" } else { "MISSED" }
        );
        if !ok {
            self.0.push(format!("{id}: {value:e} < {limit:e}"));
        }
    }
    /// A count that must be zero.
    fn none(&mut self, id: &str, count: usize, of: usize) {
        let ok = count == 0;
        println!("{id}: {count} of {of} (limit 0) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {count} of {of}"));
        }
    }
    fn holds(&mut self, id: &str, ok: bool, what: &str) {
        println!("{id}: {what} {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {what}"));
        }
    }
    fn finish(self) {
        assert!(self.0.is_empty(), "missed: {:?}", self.0);
    }
}

/// The fixed linear congruential sequence in [0, 1) of the unit tests of ADR 0219.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ---- the set-ups of ADR 0251 (the same code as crates/rcfd-mesh/tests/overset_f336.rs) ------------------------------------

fn uniform(x0: f64, len: f64, n: usize) -> Vec<f64> {
    (0..=n).map(|i| x0 + len * i as f64 / n as f64).collect()
}

fn square_spec(hole: f64) -> OversetSpec {
    OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax"].map(String::from).to_vec(),
        hole: Arc::new(move |x: Vec3| x[0].abs().max(x[1].abs()) < hole),
    }
}

fn box_overset(bg: &Mesh, half: f64, n: usize, hole: f64) -> Overset {
    let cs = uniform(-half, 2.0 * half, n);
    let comp = rectilinear_2d(&cs, &cs, DEPTH).unwrap();
    Overset::new(compose(bg, &comp, "c_").unwrap(), square_spec(hole)).unwrap()
}

fn tg_background(level: usize) -> Mesh {
    let bs = uniform(0.0, PI, 40 * level);
    rectilinear_2d(&bs, &bs, DEPTH).unwrap()
}

fn rl(level: usize) -> Overset {
    box_overset(&tg_background(level), 7.5 * H1, 20 * level, 3.1 * H1)
}

fn diagonal(d: f64) -> Pose {
    Pose::translation([d, d, 0.0])
}

fn rl_pose() -> Pose {
    diagonal(16.5 * H1)
}

fn tg(level: usize) -> Overset {
    box_overset(&tg_background(level), 0.6, 20 * level, 0.25)
}

fn tg_pose(t: f64) -> Pose {
    Pose {
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle: t,
        translation: [PI / 2.0 - 0.3 + 0.5 * t, PI / 2.0 - 0.2 + 0.3 * t, 0.0],
    }
}

fn tg_poses() -> Vec<Pose> {
    (1..=200).map(|k| tg_pose(k as f64 * 0.0025)).collect()
}

fn u_overset() -> Overset {
    let bs = uniform(-2.5, 5.0, 50);
    box_overset(&rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 16, 0.25)
}

fn u_poses() -> Vec<Pose> {
    (1..=80)
        .map(|k| {
            let t = k as f64 * 0.01;
            Pose {
                origin: [0.0; 3],
                axis: [0.0, 0.0, 1.0],
                angle: t,
                translation: [t, 0.5 * t, 0.0],
            }
        })
        .collect()
}

fn graded(x0: f64, len: f64, n: usize, h: f64, toward_end: bool) -> Vec<f64> {
    let r = expansion_for_end_cell(n, len, h).unwrap();
    let r = if toward_end { r } else { 1.0 / r };
    axis_coords(n, len, Grading::Expansion(r))
        .unwrap()
        .into_iter()
        .map(|x| x0 + x)
        .collect()
}

fn join(parts: &[Vec<f64>]) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for p in parts {
        let skip = usize::from(!out.is_empty());
        out.extend_from_slice(&p[skip..]);
    }
    out
}

fn tr() -> Overset {
    let xs = join(&[
        graded(-25.0, 17.0, 30, 0.1, true),
        uniform(-8.0, 12.5, 125),
        graded(4.5, 17.5, 30, 0.1, false),
    ]);
    let ys = join(&[
        graded(-20.0, 17.5, 30, 0.1, true),
        uniform(-2.5, 5.0, 50),
        graded(2.5, 17.5, 30, 0.1, false),
    ]);
    let bg = rectilinear_2d(&xs, &ys, 0.1).unwrap();
    let q: f64 = 1.08;
    let r_out = 0.5 + 0.01 * (q.powi(30) - 1.0) / (q - 1.0);
    let comp = cylinder_ogrid(96, 30, 0.5, r_out, Grading::Expansion(q.powi(29)), 0.1).unwrap();
    let spec = OversetSpec {
        outer_patches: vec!["c_farfield".into()],
        hole: Arc::new(|x: Vec3| x[0] * x[0] + x[1] * x[1] < 1.0),
    };
    Overset::new(compose(&bg, &comp, "c_").unwrap(), spec).unwrap()
}

fn tr_poses() -> Vec<Pose> {
    [0.0, -0.37, -1.234, -3.9, -7.5]
        .iter()
        .map(|s| Pose::translation([2.0 + s, 0.0, 0.0]))
        .collect()
}

/// A fixed pseudo-random number in [-1, 1] of a position.
fn jitter(x: Vec3, seed: f64) -> f64 {
    let v = (x[0] * 127.1 + x[1] * 311.7 + x[2] * 74.7 + seed).sin() * 43758.5453;
    2.0 * (v - v.floor()) - 1.0
}

fn distorted(mesh: &Mesh, lo: f64, hi: f64, dirs: &[usize], amp: f64, planar: bool) -> Mesh {
    let dirs = dirs.to_vec();
    warp(mesh, move |p| {
        let tol = 1e-9 * (hi - lo);
        if dirs.iter().any(|&d| p[d] - lo < tol || hi - p[d] < tol) {
            return p;
        }
        let key = if planar { [p[0], p[1], 0.0] } else { p };
        let mut q = p;
        for &d in &dirs {
            q[d] += amp * jitter(key, 1.0 + d as f64);
        }
        q
    })
    .unwrap()
}

fn dc() -> Overset {
    let level = 2;
    let bg = distorted(&tg_background(level), 0.0, PI, &[0, 1], 0.15 * H1 / level as f64, true);
    let half = 7.5 * H1;
    let n = 20 * level;
    let cs = uniform(-half, 2.0 * half, n);
    let comp = distorted(
        &rectilinear_2d(&cs, &cs, DEPTH).unwrap(),
        -half,
        half,
        &[0, 1],
        0.15 * 2.0 * half / n as f64,
        true,
    );
    Overset::new(compose(&bg, &comp, "c_").unwrap(), square_spec(3.1 * H1)).unwrap()
}

/// B3 of ADR 0251 (`distort`: B3d).
fn b3(distort: bool) -> (Overset, Pose) {
    let mut bg = box_mesh([24; 3], [2.4; 3], [Grading::Uniform; 3]).unwrap();
    let mut comp = box_mesh([14; 3], [1.4; 3], [Grading::Uniform; 3]).unwrap();
    if distort {
        bg = distorted(&bg, 0.0, 2.4, &[0, 1, 2], 0.015, false);
        comp = distorted(&comp, 0.0, 1.4, &[0, 1, 2], 0.015, false);
    }
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
            .map(String::from)
            .to_vec(),
        hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
    };
    let ov = Overset::new(compose(&bg, &comp, "c_").unwrap(), spec).unwrap();
    let pose = Pose {
        origin: [0.7; 3],
        axis: vec3::unit([1.0, 2.0, 3.0]),
        angle: 0.3,
        translation: [0.53, 0.52, 0.49],
    };
    (ov, pose)
}

fn sheared_box(n: usize, shear: [f64; 3]) -> Mesh {
    let len = 0.1 * n as f64;
    let m = 0.5 * len;
    let base = box_mesh([n; 3], [len; 3], [Grading::Uniform; 3]).unwrap();
    warp(&base, move |p| {
        let mut q = p;
        for d in 0..3 {
            if p[d] > 1e-9 && len - p[d] > 1e-9 {
                q[d] += 0.015 * jitter([p[d], 0.0, 0.0], 3.0 + d as f64);
            }
        }
        [
            q[0] + shear[0] * (q[1] - m) + shear[1] * (q[2] - m),
            q[1] + shear[2] * (q[2] - m),
            q[2],
        ]
    })
    .unwrap()
}

fn b3s() -> (Overset, Pose) {
    let bg = sheared_box(28, [-0.08, 0.05, 0.1]);
    let comp = sheared_box(14, [0.1, 0.05, 0.08]);
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
            .map(String::from)
            .to_vec(),
        hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
    };
    let ov = Overset::new(compose(&bg, &comp, "c_").unwrap(), spec).unwrap();
    let pose = Pose {
        origin: [0.7; 3],
        axis: vec3::unit([1.0, 2.0, 3.0]),
        angle: 0.3,
        translation: [0.73, 0.72, 0.69],
    };
    (ov, pose)
}

fn fb() -> (Overset, Pose) {
    (
        box_overset(&tg_background(1), 7.5 * H1, 20, 6.1 * H1),
        diagonal(16.9 * H1),
    )
}

fn mesh_at(ov: &Overset, pose: &Pose) -> Mesh {
    moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap()
}

/// All ten monomials of degree at most two in three directions (`q3` of ADR 0251).
fn q3(x: Vec3) -> f64 {
    1.0 + 0.7 * x[0] - 0.4 * x[1] + 0.3 * x[0] * x[0] - 0.6 * x[0] * x[1] + 0.5 * x[1] * x[1] + 0.2 * x[2]
        - 0.35 * x[2] * x[2]
        + 0.45 * x[0] * x[2]
        - 0.25 * x[1] * x[2]
}

fn linear_field(x: Vec3) -> f64 {
    3.0 + 2.0 * x[0] - 5.0 * x[1] + 1.5 * x[2]
}

fn same_bits(a: &[(usize, Stencil)], b: &[(usize, Stencil)]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|((ca, sa), (cb, sb))| {
            ca == cb
                && sa.cells == sb.cells
                && sa.weights.len() == sb.weights.len()
                && sa
                    .weights
                    .iter()
                    .zip(&sb.weights)
                    .all(|(x, y)| x.to_bits() == y.to_bits())
        })
}

// ---- the rule of ADR 0219 M3, written from the public parts ---------------------------------------------------------------

/// The bounding box of the points of cell `c`.
fn cell_box(mesh: &Mesh, c: usize) -> (Vec3, Vec3) {
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for &f in mesh.cell_faces(c) {
        for &q in mesh.face_points(f) {
            let p = mesh.points()[q];
            for d in 0..3 {
                lo[d] = lo[d].min(p[d]);
                hi[d] = hi[d].max(p[d]);
            }
        }
    }
    (lo, hi)
}

/// The tolerance of M3 of every cell: `1e-10` of the diagonal of its box.
fn m3_tolerances(mesh: &Mesh) -> Vec<f64> {
    (0..mesh.n_cells())
        .map(|c| {
            let (lo, hi) = cell_box(mesh, c);
            1e-10 * vec3::norm(vec3::sub(hi, lo))
        })
        .collect()
}

/// The rule of M3 over the cells `cells`: the plane margin at least `-tol`, the largest margin, the lowest number.
fn m3_locate(mesh: &Mesh, tol: &[f64], cells: impl Iterator<Item = usize>, x: Vec3) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for c in cells {
        let m = containment_margin(mesh, c, x);
        if m >= -tol[c] && best.map_or(true, |(_, b)| m > b) {
            best = Some((c, m));
        }
    }
    best.map(|(c, _)| c)
}

/// The point at which the donor of the fringe cell `c` is searched, and the range and the locator of the other part.
fn donor_query<'a>(ov: &'a Overset, pose: &Pose, centres: &[Vec3], c: usize) -> (Vec3, Range<usize>, &'a CellLocator) {
    if c < ov.composite.n_background_cells {
        (
            pose_inverse_apply(pose, centres[c]),
            ov.composite.component_cells(),
            ov.component_locator(),
        )
    } else {
        (centres[c], ov.composite.background_cells(), ov.background_locator())
    }
}

// ---- the reference: tetrahedra about the cell centre ----------------------------------------------------------------------

fn signed_volume(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> f64 {
    vec3::dot(vec3::sub(b, a), vec3::cross(vec3::sub(c, a), vec3::sub(d, a))) / 6.0
}

/// The tetrahedra `(cc, fc_f, p_i, p_{i+1})` of cell `c`, the triangle seen from outside the cell counter-clockwise.
fn cell_tetrahedra(mesh: &Mesh, c: usize) -> Vec<[Vec3; 4]> {
    let mut out = Vec::new();
    for &f in mesh.cell_faces(c) {
        let ids = mesh.face_points(f);
        for i in 0..ids.len() {
            let (mut p, mut q) = (mesh.points()[ids[i]], mesh.points()[ids[(i + 1) % ids.len()]]);
            if mesh.owner()[f] != c {
                std::mem::swap(&mut p, &mut q);
            }
            out.push([mesh.cell_centres()[c], mesh.face_centres()[f], p, q]);
        }
    }
    out
}

/// Whether `x` lies in one of the tetrahedra of cell `c` (every barycentric coordinate at least `-1e-9`).
fn reference_contains(mesh: &Mesh, c: usize, x: Vec3) -> bool {
    cell_tetrahedra(mesh, c).iter().any(|t| {
        let v = signed_volume(t[0], t[1], t[2], t[3]);
        v > 0.0
            && [
                signed_volume(x, t[1], t[2], t[3]),
                signed_volume(t[0], x, t[2], t[3]),
                signed_volume(t[0], t[1], x, t[3]),
                signed_volume(t[0], t[1], t[2], x),
            ]
            .iter()
            .all(|b| b / v >= -1e-9)
    })
}

/// Stencil cells that are not calculated, of the same part or twice in a stencil; and stencils whose first cell is not
/// the donor of the locator (P5 of ADR 0251).
fn inadmissible(ov: &Overset, pose: &Pose, centres: &[Vec3], conn: &Connectivity) -> usize {
    let nb = ov.composite.n_background_cells;
    let mut bad = 0;
    for (c, st) in &conn.stencils {
        let (x, _, loc) = donor_query(ov, pose, centres, *c);
        bad += usize::from(loc.locate(&ov.composite.mesh, x) != Some(st.cells[0]));
        let mut seen = st.cells.clone();
        seen.sort_unstable();
        seen.dedup();
        bad += st.cells.len() - seen.len();
        bad += st
            .cells
            .iter()
            .filter(|&&k| conn.kind[k] != CellKind::Calculated || (k < nb) == (*c < nb))
            .count();
    }
    bad
}

// ---- A1 to A3, N1, C2 -------------------------------------------------------------------------------------------------------

/// ADR 0253 A1, A1r, A2, A3 on B3d, the negative control N1 and the cost C2.
#[test]
fn a_b3d_every_fringe_centre_has_its_donor() {
    let mut c = Checks::new();
    let (ov, pose) = b3(true);
    assert_eq!(ov.dims, [true; 3]);
    let reference = &ov.composite.mesh;
    let nb = ov.composite.n_background_cells;
    let moved = mesh_at(&ov, &pose);
    let cc = moved.cell_centres();
    let fmax_lin = cc.iter().map(|&x| linear_field(x).abs()).fold(0.0, f64::max);
    let fmax_q3 = cc.iter().map(|&x| q3(x).abs()).fold(0.0, f64::max);
    let warps: Vec<f64> = (0..reference.n_faces()).map(|f| face_warp(reference, f)).collect();
    println!(
        "B3d: {} background and {} component cells; warp over the cell size: largest {:.4}, mean {:.4}; warped cells {} \
         and {}",
        nb,
        reference.n_cells() - nb,
        warps.iter().fold(0.0f64, |a, &b| a.max(b)) / 0.1,
        warps.iter().sum::<f64>() / warps.len() as f64 / 0.1,
        ov.background_locator().n_warped(),
        ov.component_locator().n_warped()
    );
    let mut quad = ov.clone();
    quad.set_donor_interpolation(DonorInterpolation::Quadratic);
    for (mode, o) in [("default", &ov), ("quadratic", &quad)] {
        let conn = match o.connectivity(&moved, &pose) {
            Ok(conn) => conn,
            Err(e) => {
                c.holds(&format!("A1 {mode}: connectivity"), false, &format!("failed: {e}"));
                continue;
            }
        };
        let n_fringe = conn.kind.iter().filter(|k| **k == CellKind::Fringe).count();
        c.holds(
            &format!("A1 {mode}: connectivity"),
            conn.stencils.len() == n_fringe,
            &format!(
                "{} stencils for {n_fringe} fringe cells ({} background, {} component)",
                conn.stencils.len(),
                conn.n_fringe_background(),
                conn.n_fringe_component()
            ),
        );
        let (mut differ, mut refused) = (0, 0);
        let (mut sum, mut lin, mut quadratic, mut lambda) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let (mut smallest, mut largest) = (usize::MAX, 0);
        for (cell, st) in &conn.stencils {
            let (x, _, loc) = donor_query(o, &pose, cc, *cell);
            let a = loc.locate(reference, x);
            differ += usize::from(a != loc.locate_brute_force(reference, x) || a != Some(st.cells[0]));
            refused += usize::from(!reference_contains(reference, st.cells[0], x));
            sum = sum.max((st.weights.iter().sum::<f64>() - 1.0).abs());
            let at = |f: &dyn Fn(Vec3) -> f64| -> f64 {
                (st.cells
                    .iter()
                    .zip(&st.weights)
                    .map(|(&k, w)| w * f(cc[k]))
                    .sum::<f64>()
                    - f(cc[*cell]))
                .abs()
            };
            lin = lin.max(at(&linear_field) / fmax_lin);
            if conn.fallbacks.binary_search(cell).is_err() {
                quadratic = quadratic.max(at(&q3) / fmax_q3);
                lambda = lambda.max(st.weights.iter().map(|w| w.abs()).sum());
                smallest = smallest.min(st.cells.len());
                largest = largest.max(st.cells.len());
            }
        }
        c.none(
            &format!("A1 {mode}: donors that differ from the full search"),
            differ,
            conn.stencils.len(),
        );
        c.none(
            &format!("A1r {mode}: donors that do not contain the centre by the reference"),
            refused,
            conn.stencils.len(),
        );
        let id = if mode == "default" { "A2" } else { "A3" };
        c.at_most(&format!("{id} |sum w - 1|"), sum, 1e-12);
        c.at_most(&format!("{id} linear field"), lin, 1e-11);
        if mode == "quadratic" {
            c.at_most("A3 q3 over the stencils that are not fallbacks", quadratic, 1e-11);
            c.at_least(
                "A3 share of the stencils that are not fallbacks",
                1.0 - conn.n_fallback() as f64 / conn.stencils.len() as f64,
                0.95,
            );
            c.none(
                "A3 inadmissible stencil cells and wrong donors",
                inadmissible(o, &pose, cc, &conn),
                conn.stencils.len(),
            );
        } else {
            println!("default stencils: q3 {quadratic:.3e} (no limit: they are linear)");
        }
        println!(
            "{mode}: {} fallbacks, stencils of {smallest} to {largest} cells, Lambda {lambda:.4}",
            conn.n_fallback()
        );
        if mode == "default" {
            // N1: the rule of M3 on the same fringe centres
            let tol = m3_tolerances(reference);
            let mut without = Vec::new();
            for (cell, _) in &conn.stencils {
                let (x, range, _) = donor_query(o, &pose, cc, *cell);
                if m3_locate(reference, &tol, range, x).is_none() {
                    without.push(*cell);
                }
            }
            println!("N1: fringe cells without a cell under the rule of M3: {without:?}");
            let centre = format!("({:.6}, {:.6}, {:.6})", cc[16035][0], cc[16035][1], cc[16035][2]);
            c.holds(
                "N1 (negative control) the component fringe cell 16035",
                conn.kind[16035] == CellKind::Fringe
                    && centre == "(2.009494, 1.010188, 1.499172)"
                    && without.contains(&16035),
                &format!("at {centre} has no cell under the rule of M3, as recorded"),
            );
            c.at_least(
                "N1 (negative control) fringe centres without a cell under the rule of M3",
                without.len() as f64,
                1.0,
            );
        }
    }
    // C2: the time of the connectivity of B3d over that of B3, and the construction of the locators (printed)
    let (flat, flat_pose) = b3(false);
    let flat_moved = mesh_at(&flat, &flat_pose);
    let time = |o: &Overset, m: &Mesh, p: &Pose| {
        let t0 = Instant::now();
        for _ in 0..5 {
            o.connectivity(m, p).unwrap();
        }
        t0.elapsed().as_secs_f64() / 5.0
    };
    let (t_flat, t_warped) = (time(&flat, &flat_moved, &flat_pose), time(&ov, &moved, &pose));
    println!(
        "connectivity, default interpolation: B3 {:.3} ms, B3d {:.3} ms",
        1e3 * t_flat,
        1e3 * t_warped
    );
    c.at_most("C2 time of connectivity, B3d over B3", t_warped / t_flat, 20.0);
    for (name, o) in [("B3", &flat), ("B3d", &ov)] {
        let t0 = Instant::now();
        let loc = CellLocator::new(&o.composite.mesh, o.composite.background_cells());
        println!(
            "CellLocator::new, background of {name}: {:.2} ms ({} warped cells)",
            1e3 * t0.elapsed().as_secs_f64(),
            loc.n_warped()
        );
    }
    c.finish();
}

// ---- S1 to S5, C1, N2 -------------------------------------------------------------------------------------------------------

/// ADR 0253 S1 to S5: the cells of the two parts of B3d tile them, counted on random points; C1; the negative control N2.
#[test]
fn s_the_cells_of_a_warped_mesh_tile_it() {
    let mut c = Checks::new();
    let (ov, _) = b3(true);
    let mesh = &ov.composite.mesh;
    let nb = ov.composite.n_background_cells;
    let centres = mesh.cell_centres();
    // S4: the set-up
    let mut smallest = f64::INFINITY;
    for cell in 0..mesh.n_cells() {
        for t in cell_tetrahedra(mesh, cell) {
            smallest = smallest.min(signed_volume(t[0], t[1], t[2], t[3]));
        }
    }
    println!("S4: smallest tetrahedron about a cell centre {smallest:.4e} (a cell has 1e-3)");
    c.holds(
        "S4 (set-up) the cells are star-shaped about their centres",
        smallest > 0.0,
        "",
    );
    c.at_least(
        "S4 (set-up) share of warped background cells",
        ov.background_locator().n_warped() as f64 / nb as f64,
        0.99,
    );
    c.at_least(
        "S4 (set-up) share of warped component cells",
        ov.component_locator().n_warped() as f64 / (mesh.n_cells() - nb) as f64,
        0.99,
    );
    // S5
    let mut wrong = 0;
    for (loc, range) in [
        (ov.background_locator(), ov.composite.background_cells()),
        (ov.component_locator(), ov.composite.component_cells()),
    ] {
        wrong += range.filter(|&k| loc.locate(mesh, centres[k]) != Some(k)).count();
    }
    c.none("S5 cell centres not located in their own cell", wrong, mesh.n_cells());
    // S1 to S3
    let mut g = Lcg(0x5eed_f352);
    let mut background_points = Vec::new();
    for (id, name, loc, len, n) in [
        ("S1", "background", ov.background_locator(), 2.4, 200_000usize),
        ("S2", "component", ov.component_locator(), 1.4, 50_000),
    ] {
        let points: Vec<Vec3> = (0..n)
            .map(|_| [0, 1, 2].map(|_| 0.05 + (len - 0.1) * g.next()))
            .collect();
        let (mut none, mut several, mut differ, mut refused) = (0, 0, 0, 0);
        for &x in &points {
            let cells = loc.containing_cells(mesh, x);
            none += usize::from(cells.is_empty());
            several += usize::from(cells.len() > 1);
            let a = loc.locate(mesh, x);
            differ += usize::from(a != loc.locate_brute_force(mesh, x));
            refused += usize::from(!a.is_some_and(|k| reference_contains(mesh, k, x)));
        }
        c.none(&format!("{id} {name}: points in no cell"), none, n);
        c.none(&format!("{id} {name}: points in two or more cells"), several, n);
        c.none(
            &format!("S3 {name}: points whose cell differs from the full search"),
            differ,
            n,
        );
        c.none(
            &format!("S3 {name}: points whose cell is refused by the reference"),
            refused,
            n,
        );
        let t0 = Instant::now();
        let found = points.iter().filter(|&&x| loc.locate(mesh, x).is_some()).count();
        println!(
            "{name}: locate {:.3} microseconds for each point ({found} of {n} found)",
            1e6 * t0.elapsed().as_secs_f64() / n as f64
        );
        if id == "S1" {
            background_points = points;
        }
    }
    // C1 and N2 on the points of S1
    let tol = m3_tolerances(mesh);
    let boxes: Vec<(Vec3, Vec3)> = (0..nb).map(|k| cell_box(mesh, k)).collect();
    let (mut in_boxes, mut none, mut several) = (0usize, 0, 0);
    for &x in &background_points {
        let mut accepted = 0;
        for k in 0..nb {
            let (lo, hi) = boxes[k];
            in_boxes += usize::from((0..3).all(|d| x[d] >= lo[d] - tol[k] && x[d] <= hi[d] + tol[k]));
            let r = vec3::sub(x, centres[k]);
            if vec3::dot(r, r) < 0.25 * 0.25 && containment_margin(mesh, k, x) >= -tol[k] {
                accepted += 1;
            }
        }
        none += usize::from(accepted == 0);
        several += usize::from(accepted > 1);
    }
    let n = background_points.len();
    c.at_most(
        "C1 mean number of cell boxes that contain a point",
        in_boxes as f64 / n as f64,
        3.0,
    );
    println!("N2: the rule of M3 on the {n} points of S1: {none} in no cell, {several} in two or more");
    c.at_least("N2 (negative control) points in no cell", none as f64, 20.0);
    c.at_least(
        "N2 (negative control) points in two or more cells",
        several as f64,
        20.0,
    );
    c.finish();
}

// ---- T1 to T3 ---------------------------------------------------------------------------------------------------------------

/// ADR 0253 T1 to T3: a point on a triangle shared by cells of the background of B3d goes to the lowest-numbered cell,
/// through the bins and by the full search.
#[test]
fn t_points_on_shared_triangles() {
    let mut c = Checks::new();
    let (ov, _) = b3(true);
    let mesh = &ov.composite.mesh;
    let nb = ov.composite.n_background_cells;
    let loc = ov.background_locator();
    let faces: Vec<usize> = (0..mesh.n_internal_faces()).filter(|&f| mesh.owner()[f] < nb).collect();
    // T1
    let wrong = faces
        .iter()
        .filter(|&&f| {
            let x = mesh.face_centres()[f];
            let a = loc.locate(mesh, x);
            a != loc.locate_brute_force(mesh, x) || a != Some(mesh.owner()[f])
        })
        .count();
    c.none("T1 face centres not given to the owner of the face", wrong, faces.len());
    // T2
    let mut lowest = vec![usize::MAX; ov.composite.n_background_points];
    for k in 0..nb {
        for &f in mesh.cell_faces(k) {
            for &q in mesh.face_points(f) {
                lowest[q] = lowest[q].min(k);
            }
        }
    }
    let (mut wrong, mut n) = (0, 0);
    for (q, &k) in lowest.iter().enumerate() {
        let x = mesh.points()[q];
        if (0..3).any(|d| x[d] < 1e-9 || x[d] > 2.4 - 1e-9) {
            continue;
        }
        n += 1;
        let a = loc.locate(mesh, x);
        wrong += usize::from(a != loc.locate_brute_force(mesh, x) || a != Some(k));
    }
    c.none(
        "T2 interior mesh points not given to the lowest-numbered cell around them",
        wrong,
        n,
    );
    // T3
    let mut g = Lcg(0x5eed_f352);
    let (mut wrong, mut owners) = (0, 0);
    let n = 20_000;
    for _ in 0..n {
        let f = faces[((g.next() * faces.len() as f64) as usize).min(faces.len() - 1)];
        let ids = mesh.face_points(f);
        let i = ((g.next() * ids.len() as f64) as usize).min(ids.len() - 1);
        let (fc, p, q) = (
            mesh.face_centres()[f],
            mesh.points()[ids[i]],
            mesh.points()[ids[(i + 1) % ids.len()]],
        );
        let (mut r1, mut r2) = (g.next(), g.next());
        if r1 + r2 > 1.0 {
            (r1, r2) = (1.0 - r1, 1.0 - r2);
        }
        let x = vec3::add(
            fc,
            vec3::add(vec3::scale(vec3::sub(p, fc), r1), vec3::scale(vec3::sub(q, fc), r2)),
        );
        let a = loc.locate(mesh, x);
        let (o, nbr) = (mesh.owner()[f], mesh.neighbour()[f]);
        wrong += usize::from(a != loc.locate_brute_force(mesh, x) || !(a == Some(o) || a == Some(nbr)));
        owners += usize::from(a == Some(o));
    }
    c.none(
        "T3 points on shared triangles not given to a cell of the face, or not as the full search",
        wrong,
        n,
    );
    println!("T3: {owners} of {n} points went to the owner of the face");
    c.finish();
}

// ---- A4 ---------------------------------------------------------------------------------------------------------------------

/// ADR 0253 A4b to A4d: on the planar set-ups of ADR 0251 no cell is warped, and the donors and the default stencils are
/// those of the rule of ADR 0219 M3 written out in this file, bit for bit.
#[test]
fn a4_planar_meshes_keep_their_donors_and_stencils() {
    let mut c = Checks::new();
    let warped = |ov: &Overset| ov.background_locator().n_warped() + ov.component_locator().n_warped();
    for (name, ov) in [("RL level 4", rl(4)), ("TG_2", tg(2)), ("TG_4", tg(4)), ("FB", fb().0)] {
        c.none(
            &format!("A4b {name}: warped cells"),
            warped(&ov),
            ov.composite.mesh.n_cells(),
        );
    }
    let (b3_ov, b3_pose) = b3(false);
    let (b3s_ov, b3s_pose) = b3s();
    let cases: Vec<(&str, Overset, Vec<Pose>)> = vec![
        ("RL level 1", rl(1), vec![rl_pose()]),
        ("RL level 2", rl(2), vec![rl_pose()]),
        ("TG_1, 200 poses", tg(1), tg_poses()),
        ("U, 80 poses", u_overset(), u_poses()),
        ("TR, 5 poses", tr(), tr_poses()),
        ("DC", dc(), vec![rl_pose()]),
        ("B3", b3_ov, vec![b3_pose]),
        ("B3s", b3s_ov, vec![b3s_pose]),
    ];
    for (name, ov, poses) in &cases {
        let reference = &ov.composite.mesh;
        c.none(&format!("A4b {name}: warped cells"), warped(ov), reference.n_cells());
        let tol = m3_tolerances(reference);
        let rc = reference.cell_centres();
        let (mut differ, mut n, mut equal) = (0, 0, true);
        for pose in poses {
            let moved = mesh_at(ov, pose);
            let conn = ov.connectivity(&moved, pose).unwrap();
            let cc = moved.cell_centres();
            let mut m4 = Vec::new();
            for (cell, st) in &conn.stencils {
                let (x, range, _) = donor_query(ov, pose, cc, *cell);
                let donor = m3_locate(reference, &tol, range, x);
                n += 1;
                differ += usize::from(donor != Some(st.cells[0]));
                let Some(d) = donor else { continue };
                let mut cells = vec![d];
                cells.extend(
                    ov.neighbours(d)
                        .iter()
                        .copied()
                        .filter(|&q| conn.kind[q] == CellKind::Calculated),
                );
                let pts: Vec<Vec3> = cells.iter().map(|&q| rc[q]).collect();
                if let Ok(weights) = linear_weights(&pts, x, ov.dims) {
                    m4.push((*cell, Stencil { cells, weights }));
                }
            }
            equal &= same_bits(&conn.stencils, &m4) && conn.fallbacks.is_empty();
        }
        c.none(
            &format!("A4c {name}: donors that differ from the rule of M3"),
            differ,
            n,
        );
        c.holds(
            &format!("A4d {name}"),
            equal,
            &format!("{n} stencils equal to those built from the rule of M3, bit for bit"),
        );
    }
    // A4c: the 2000 points of K2 of ADR 0219 for each part of TR
    let ov = &cases[4].1;
    let m = &ov.composite.mesh;
    let tol = m3_tolerances(m);
    for (part, loc, range) in [
        ("background", ov.background_locator(), ov.composite.background_cells()),
        ("component", ov.component_locator(), ov.composite.component_cells()),
    ] {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for k in range.clone() {
            for d in 0..3 {
                lo[d] = lo[d].min(m.cell_centres()[k][d]);
                hi[d] = hi[d].max(m.cell_centres()[k][d]);
            }
        }
        let mut g = Lcg(0x5eed_f134);
        let (mut differ, mut inside) = (0, 0);
        for _ in 0..2000 {
            let x = [
                lo[0] - 0.2 + (hi[0] - lo[0] + 0.4) * g.next(),
                lo[1] - 0.2 + (hi[1] - lo[1] + 0.4) * g.next(),
                0.05,
            ];
            let a = loc.locate(m, x);
            differ += usize::from(a != m3_locate(m, &tol, range.clone(), x));
            inside += usize::from(a.is_some());
        }
        c.none(
            &format!("A4c TR {part}: the points of K2 located otherwise than by the rule of M3 ({inside} inside)"),
            differ,
            2000,
        );
    }
    c.finish();
}
