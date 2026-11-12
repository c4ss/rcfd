//! F-356 (ADR 0260): the donor search of the overset coupling on non-convex cells with planar faces. On two meshes of
//! non-convex prisms (P1: darts; P2: L-shapes whose fan about the face centre folds, with triangle prisms in their notches)
//! every one of 100000 random points lies in exactly one cell of the locator, agrees with a full search and with an
//! independent containment test (A1); an overset on P2 gets its donors (A3); the set-ups (S0); points on shared faces
//! (T1, T3, T4); the present rule fails as estimated (N1); on the convex set-ups of ADR 0253 no cell is reclassified and on
//! the warped B3d the donors are those of the rule of ADR 0253 (A4b', A4e).
//!
//! Release: `cargo test --release -p rcfd-mesh --test overset_f356`.

use rcfd_core::vec3::{self, Vec3};
use rcfd_mesh::gen::{axis_coords, box_mesh, cylinder_ogrid, expansion_for_end_cell, rectilinear_2d, warp, Grading};
use rcfd_mesh::mesh::PatchKind;
use rcfd_mesh::motion::{moved_mesh, Pose};
use rcfd_mesh::overset::{
    compose, containment_margin, face_ear_triangles, face_fan_folds, face_warp, pose_inverse_apply,
    triangulated_margin, CellKind, CellLocator, DonorInterpolation, Overset, OversetSpec,
};
use rcfd_mesh::Mesh;
use std::f64::consts::PI;
use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

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
    fn equal(&mut self, id: &str, value: usize, want: usize) {
        let ok = value == want;
        println!("{id}: {value} (limit = {want}) {}", if ok { "met" } else { "MISSED" });
        if !ok {
            self.0.push(format!("{id}: {value} != {want}"));
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

const SEED: u64 = 0x5eed_f356;

/// A fixed pseudo-random number in [-1, 1] of a position (the function of ADR 0253's test file).
fn jitter(x: Vec3, seed: f64) -> f64 {
    let v = (x[0] * 127.1 + x[1] * 311.7 + x[2] * 74.7 + seed).sin() * 43758.5453;
    2.0 * (v - v.floor()) - 1.0
}

// ---- the prism set-ups P1 and P2 (ADR 0260 "Set-ups") -----------------------------------------------------------------------

const A: [[f64; 3]; 3] = [[1.0, 0.15, -0.1], [0.05, 0.95, 0.12], [-0.08, 0.1, 1.05]];
const B: Vec3 = [0.3, -0.2, 0.1];

fn affine(y: Vec3) -> Vec3 {
    [0, 1, 2].map(|i| A[i][0] * y[0] + A[i][1] * y[1] + A[i][2] * y[2] + B[i])
}

/// The inverse of `affine` (Cramer's rule).
fn affine_inverse(x: Vec3) -> Vec3 {
    let r = vec3::sub(x, B);
    let (c0, c1, c2) = (
        [A[0][0], A[1][0], A[2][0]],
        [A[0][1], A[1][1], A[2][1]],
        [A[0][2], A[1][2], A[2][2]],
    );
    let det = vec3::dot(c0, vec3::cross(c1, c2));
    [
        vec3::dot(r, vec3::cross(c1, c2)) / det,
        vec3::dot(c0, vec3::cross(r, c2)) / det,
        vec3::dot(c0, vec3::cross(c1, r)) / det,
    ]
}

/// A mesh of prisms on a tiling of base polygons, with the construction kept for the reference.
struct Prisms {
    mesh: Mesh,
    /// Base points in the lattice plane.
    base: Vec<[f64; 2]>,
    /// Base polygons, counter-clockwise, as base point numbers.
    polys: Vec<Vec<usize>>,
    /// The triangles of each base polygon known from the construction (the reference).
    tris: Vec<Vec<[usize; 3]>>,
    /// Layer heights `z_0 .. z_K`.
    z: Vec<f64>,
    /// Side of the square base.
    side: f64,
}

fn layers(k: usize, hz: f64) -> Vec<f64> {
    (0..=k)
        .map(|i| {
            if i == 0 || i == k {
                hz * i as f64
            } else {
                hz * (i as f64 + 0.2 * jitter([i as f64, 0.0, 0.0], 5.0))
            }
        })
        .collect()
}

fn turn(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0])
}

fn area2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Build the prism mesh: cell `(c, k)` is `k n_base + c`; one side face for every base edge and layer, the base polygon at
/// every height; internal faces sorted by `(owner, neighbour)`; every boundary face in the patch `walls`.
fn prisms(base: Vec<[f64; 2]>, polys: Vec<Vec<usize>>, tris: Vec<Vec<[usize; 3]>>, z: Vec<f64>, side: f64) -> Prisms {
    let (nbp, n2, k) = (base.len(), polys.len(), z.len() - 1);
    let mut points = Vec::with_capacity(nbp * (k + 1));
    for &zk in &z {
        for p in &base {
            points.push(affine([p[0], p[1], zk]));
        }
    }
    let at = |q: usize, layer: usize| layer * nbp + q;
    let mut edge_cell = std::collections::HashMap::new();
    for (c, poly) in polys.iter().enumerate() {
        for i in 0..poly.len() {
            edge_cell.insert((poly[i], poly[(i + 1) % poly.len()]), c);
        }
    }
    let mut internal: Vec<(usize, usize, Vec<usize>)> = Vec::new();
    let mut boundary: Vec<(usize, Vec<usize>)> = Vec::new();
    for layer in 0..k {
        for (c, poly) in polys.iter().enumerate() {
            let id = layer * n2 + c;
            for i in 0..poly.len() {
                let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                let face = vec![at(a, layer), at(b, layer), at(b, layer + 1), at(a, layer + 1)];
                match edge_cell.get(&(b, a)) {
                    Some(&other) if other > c => internal.push((id, layer * n2 + other, face)),
                    Some(_) => {}
                    None => boundary.push((id, face)),
                }
            }
        }
    }
    for layer in 0..=k {
        for (c, poly) in polys.iter().enumerate() {
            let face: Vec<usize> = poly.iter().map(|&q| at(q, layer)).collect();
            if layer == 0 {
                boundary.push((c, face.into_iter().rev().collect()));
            } else if layer == k {
                boundary.push(((k - 1) * n2 + c, face));
            } else {
                internal.push(((layer - 1) * n2 + c, layer * n2 + c, face));
            }
        }
    }
    internal.sort_by_key(|f| (f.0, f.1));
    let (mut faces, mut owner, mut neighbour) = (Vec::new(), Vec::new(), Vec::new());
    for (o, n, f) in internal {
        faces.push(f);
        owner.push(o);
        neighbour.push(n);
    }
    let nb = boundary.len();
    for (o, f) in boundary {
        faces.push(f);
        owner.push(o);
    }
    let mesh = Mesh::from_face_lists(
        points,
        faces,
        owner,
        neighbour,
        vec![("walls".to_string(), PatchKind::Wall, nb)],
    )
    .unwrap();
    Prisms {
        mesh,
        base,
        polys,
        tris,
        z,
        side,
    }
}

/// P1: a lattice of 16 x 16 darts of size 0.1 (the points moved by 0.4 h along a diagonal chosen by their parities), five
/// layers of 0.12.
fn p1() -> Prisms {
    let (n, h, s) = (16usize, 0.1, 0.4);
    let mut base = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            let d = match (i % 2, j % 2) {
                (0, 0) => [1.0, 1.0],
                (1, 1) => [1.0, -1.0],
                (1, 0) => [-1.0, -1.0],
                _ => [-1.0, 1.0],
            };
            let mut p = [i as f64 + s * d[0], j as f64 + s * d[1]];
            if i == 0 || i == n {
                p[0] = i as f64;
            }
            if j == 0 || j == n {
                p[1] = j as f64;
            }
            base.push([h * p[0], h * p[1]]);
        }
    }
    let id = |i: usize, j: usize| i + (n + 1) * j;
    let mut polys = Vec::new();
    let mut tris = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let q = vec![id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)];
            let r = (0..4)
                .find(|&m| turn(base[q[(m + 3) % 4]], base[q[m]], base[q[(m + 1) % 4]]) < 0.0)
                .unwrap_or(0);
            tris.push(vec![
                [q[r], q[(r + 1) % 4], q[(r + 2) % 4]],
                [q[r], q[(r + 2) % 4], q[(r + 3) % 4]],
            ]);
            polys.push(q);
        }
    }
    prisms(base, polys, tris, layers(5, 0.12), h * n as f64)
}

/// P2: 6 x 6 blocks of 3 x 3 lattice units of 0.1, each an L-shape and the two triangles of its notch, every lattice point
/// moved by up to 0.1 h; eight layers of 0.2.
fn p2() -> Prisms {
    let (nb, a, h) = (6usize, 3usize, 0.1);
    let l = nb * a;
    let mut base = Vec::new();
    for j in 0..=l {
        for i in 0..=l {
            let key = [i as f64, j as f64, 0.0];
            let mut d = [0.1 * jitter(key, 1.0), 0.1 * jitter(key, 2.0)];
            if i == 0 || i == l {
                d[0] = 0.0;
            }
            if j == 0 || j == l {
                d[1] = 0.0;
            }
            base.push([h * (i as f64 + d[0]), h * (j as f64 + d[1])]);
        }
    }
    let mut polys = Vec::new();
    let mut tris = Vec::new();
    for bj in 0..nb {
        for bi in 0..nb {
            let p = |u: usize, v: usize| (bi * a + u) + (l + 1) * (bj * a + v);
            polys.push(vec![
                p(0, 0),
                p(1, 0),
                p(3, 0),
                p(3, 1),
                p(1, 1),
                p(1, 3),
                p(0, 3),
                p(0, 1),
            ]);
            let quads = [
                [p(0, 0), p(1, 0), p(1, 1), p(0, 1)],
                [p(1, 0), p(3, 0), p(3, 1), p(1, 1)],
                [p(0, 1), p(1, 1), p(1, 3), p(0, 3)],
            ];
            tris.push(
                quads
                    .iter()
                    .flat_map(|q| [[q[0], q[1], q[2]], [q[0], q[2], q[3]]])
                    .collect(),
            );
            let t1 = [p(1, 1), p(3, 1), p(3, 3)];
            let t2 = [p(1, 1), p(3, 3), p(1, 3)];
            polys.push(t1.to_vec());
            tris.push(vec![t1]);
            polys.push(t2.to_vec());
            tris.push(vec![t2]);
        }
    }
    prisms(base, polys, tris, layers(8, 0.2), h * l as f64)
}

impl Prisms {
    fn n_base(&self) -> usize {
        self.polys.len()
    }
    fn height(&self) -> f64 {
        *self.z.last().unwrap()
    }
    /// The reference (ADR 0260): `x` lies in cell `cell` when its pre-image lies in the cell's layer and in one of the
    /// triangles of its base polygon, every barycentric coordinate at least `-1e-9`.
    fn contains(&self, cell: usize, x: Vec3) -> bool {
        let y = affine_inverse(x);
        let (c, k) = (cell % self.n_base(), cell / self.n_base());
        let hz = self.z[k + 1] - self.z[k];
        if y[2] < self.z[k] - 1e-9 * hz || y[2] > self.z[k + 1] + 1e-9 * hz {
            return false;
        }
        let q = [y[0], y[1]];
        self.tris[c].iter().any(|t| {
            let (a, b, d) = (self.base[t[0]], self.base[t[1]], self.base[t[2]]);
            let v = area2(a, b, d);
            v > 0.0
                && [area2(q, b, d), area2(a, q, d), area2(a, b, q)]
                    .iter()
                    .all(|w| w / v >= -1e-9)
        })
    }
    /// The number of cells whose base polygon has a vertex with a negative turn.
    fn n_reflex(&self) -> usize {
        let n = self
            .polys
            .iter()
            .filter(|p| {
                (0..p.len()).any(|m| {
                    turn(
                        self.base[p[(m + p.len() - 1) % p.len()]],
                        self.base[p[m]],
                        self.base[p[(m + 1) % p.len()]],
                    ) < 0.0
                })
            })
            .count();
        n * (self.z.len() - 1)
    }
    /// The largest relative difference between the summed areas of the reference triangles and the polygon area, and
    /// the smallest triangle area.
    fn reference_areas(&self) -> (f64, f64) {
        let (mut worst, mut smallest) = (0.0f64, f64::INFINITY);
        for (p, ts) in self.polys.iter().zip(&self.tris) {
            let shoelace: f64 = (0..p.len())
                .map(|m| {
                    let (a, b) = (self.base[p[m]], self.base[p[(m + 1) % p.len()]]);
                    a[0] * b[1] - a[1] * b[0]
                })
                .sum::<f64>();
            let sum: f64 = ts
                .iter()
                .map(|t| {
                    let v = area2(self.base[t[0]], self.base[t[1]], self.base[t[2]]);
                    smallest = smallest.min(v);
                    v
                })
                .sum();
            worst = worst.max((sum - shoelace).abs() / shoelace.abs());
        }
        (worst, smallest)
    }
    /// A random point inside the mesh: the pre-image uniform in the box shrunk by 1e-6 (ADR 0260 "Random points").
    fn random_point(&self, g: &mut Lcg) -> Vec3 {
        let e = 1e-6;
        affine([
            e + (self.side - 2.0 * e) * g.next(),
            e + (self.side - 2.0 * e) * g.next(),
            e + (self.height() - 2.0 * e) * g.next(),
        ])
    }
}

// ---- the rule of ADR 0219 M3 and of ADR 0253, written from the public parts -------------------------------------------------

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

fn in_box(b: &(Vec3, Vec3), tol: f64, x: Vec3) -> bool {
    (0..3).all(|d| x[d] >= b.0[d] - tol && x[d] <= b.1[d] + tol)
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

/// The points of cell `c`, each once.
fn cell_points(mesh: &Mesh, c: usize) -> Vec<usize> {
    let mut p: Vec<usize> = mesh
        .cell_faces(c)
        .iter()
        .flat_map(|&f| mesh.face_points(f).iter().copied())
        .collect();
    p.sort_unstable();
    p.dedup();
    p
}

/// The largest distance of a point of a cell on the outer side of one of its face planes over the cell's tolerance, over
/// the cells `cells`.
fn largest_outward_ratio(mesh: &Mesh, tol: &[f64], cells: Range<usize>) -> f64 {
    let mut worst = f64::NEG_INFINITY;
    for c in cells {
        let pts = cell_points(mesh, c);
        for &f in mesh.cell_faces(c) {
            let s = mesh.face_area_vectors()[f];
            let s = if mesh.owner()[f] == c { s } else { vec3::scale(s, -1.0) };
            let a = vec3::norm(s);
            if a < 1e-300 {
                continue;
            }
            for &q in &pts {
                let d = vec3::dot(s, vec3::sub(mesh.points()[q], mesh.face_centres()[f])) / a;
                worst = worst.max(d / tol[c]);
            }
        }
    }
    worst
}

/// The rule of ADR 0253 over the cells `cells`: a cell with a face warped beyond its tolerance by `triangulated_margin`
/// inside its widened box, any other by `containment_margin`; at least `-tol`, the largest margin, the lowest number.
struct F352Rule {
    tol: Vec<f64>,
    boxes: Vec<(Vec3, Vec3)>,
    warped: Vec<bool>,
}

impl F352Rule {
    fn new(mesh: &Mesh) -> Self {
        let tol = m3_tolerances(mesh);
        let boxes = (0..mesh.n_cells()).map(|c| cell_box(mesh, c)).collect();
        let warped = (0..mesh.n_cells())
            .map(|c| mesh.cell_faces(c).iter().any(|&f| face_warp(mesh, f) > tol[c]))
            .collect();
        F352Rule { tol, boxes, warped }
    }
    fn locate(&self, mesh: &Mesh, cells: Range<usize>, x: Vec3) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for c in cells {
            let m = if self.warped[c] {
                if !in_box(&self.boxes[c], self.tol[c], x) {
                    continue;
                }
                triangulated_margin(mesh, c, x)
            } else {
                containment_margin(mesh, c, x)
            };
            if m >= -self.tol[c] && best.map_or(true, |(_, b)| m > b) {
                best = Some((c, m));
            }
        }
        best.map(|(c, _)| c)
    }
}

// ---- A1, S0, N1 ---------------------------------------------------------------------------------------------------------------

/// ADR 0260 A1, A1f, A1r on P1 and P2 (100000 random points each), the set-up checks S0a to S0e and the control N1.
#[test]
fn a1_random_points_lie_in_exactly_one_cell() {
    let mut c = Checks::new();
    for (name, pr, nonconvex, ears) in [("P1", p1(), 1125usize, 0usize), ("P2", p2(), 288, 324)] {
        let mesh = &pr.mesh;
        let n_cells = mesh.n_cells();
        let t0 = Instant::now();
        let loc = CellLocator::new(mesh, 0..n_cells);
        println!(
            "{name}: {n_cells} cells, {} faces; CellLocator::new {:.2} ms",
            mesh.n_faces(),
            1e3 * t0.elapsed().as_secs_f64()
        );
        // S0
        c.equal(&format!("S0a {name}: warped cells"), loc.n_warped(), 0);
        c.equal(
            &format!("S0b {name}: non-convex cells against the reflex base polygons"),
            loc.n_nonconvex(),
            pr.n_reflex(),
        );
        c.equal(
            &format!("S0b {name}: non-convex cells (registered)"),
            loc.n_nonconvex(),
            nonconvex,
        );
        c.equal(&format!("S0c {name}: ear-clipped faces"), loc.n_ear_clipped(), ears);
        c.equal(&format!("S0c {name}: unsupported faces"), loc.n_unsupported(), 0);
        let (mut folded, mut bad, mut worst) = (0, 0, 0.0f64);
        for f in 0..mesh.n_faces() {
            if !face_fan_folds(mesh, f) {
                continue;
            }
            folded += 1;
            let s = mesh.face_area_vectors()[f];
            let Some(tris) = face_ear_triangles(mesh, f) else {
                bad += 1;
                continue;
            };
            let mut sum = [0.0; 3];
            for t in &tris {
                let [a, b, q] = t.map(|k| mesh.points()[k]);
                let n = vec3::scale(vec3::cross(vec3::sub(b, a), vec3::sub(q, a)), 0.5);
                bad += usize::from(vec3::dot(n, s) <= 0.0);
                sum = vec3::add(sum, n);
            }
            worst = worst.max(vec3::norm(vec3::sub(sum, s)) / vec3::norm(s));
        }
        println!("{name}: {folded} faces whose fan folds");
        c.none(
            &format!("S0d {name}: ear triangles of non-positive orientation, or no ear"),
            bad,
            folded,
        );
        c.at_most(
            &format!("S0d {name}: vector area of the ear triangles against S_f"),
            worst,
            1e-12,
        );
        let (area_err, smallest) = pr.reference_areas();
        c.holds(
            &format!("S0e {name}: reference triangles of positive area"),
            smallest > 0.0,
            &format!("smallest {smallest:.3e}"),
        );
        c.at_most(
            &format!("S0e {name}: reference areas against the polygons"),
            area_err,
            1e-12,
        );
        // A1
        let mut g = Lcg(SEED);
        let n = 100_000;
        let points: Vec<Vec3> = (0..n).map(|_| pr.random_point(&mut g)).collect();
        let (mut none, mut several, mut differ, mut refused) = (0, 0, 0, 0);
        for &x in &points {
            let cells = loc.containing_cells(mesh, x);
            none += usize::from(cells.is_empty());
            several += usize::from(cells.len() > 1);
            let a = loc.locate(mesh, x);
            differ += usize::from(a != loc.locate_brute_force(mesh, x));
            refused += usize::from(!a.is_some_and(|k| pr.contains(k, x)));
        }
        c.none(&format!("A1 {name}: points in no cell"), none, n);
        c.none(&format!("A1 {name}: points in two or more cells"), several, n);
        c.none(
            &format!("A1f {name}: points whose cell differs from the full search"),
            differ,
            n,
        );
        c.none(
            &format!("A1r {name}: points whose cell is refused by the reference"),
            refused,
            n,
        );
        let t0 = Instant::now();
        let found = points.iter().filter(|&&x| loc.locate(mesh, x).is_some()).count();
        println!(
            "{name}: locate {:.3} microseconds for each point ({found} of {n} found)",
            1e6 * t0.elapsed().as_secs_f64() / n as f64
        );
        // N1: the rule of M3 on the same points
        let tol = m3_tolerances(mesh);
        let boxes: Vec<(Vec3, Vec3)> = (0..n_cells).map(|k| cell_box(mesh, k)).collect();
        let (mut m3_none, mut m3_several) = (0, 0);
        for &x in &points {
            let accepted = (0..n_cells)
                .filter(|&k| in_box(&boxes[k], tol[k], x) && containment_margin(mesh, k, x) >= -tol[k])
                .count();
            m3_none += usize::from(accepted == 0);
            m3_several += usize::from(accepted > 1);
        }
        println!("N1 {name}: the rule of M3: {m3_none} points in no cell, {m3_several} in two or more");
        let limit = if name == "P1" { 15000.0 } else { 30000.0 };
        c.at_least(
            &format!("N1 (negative control) {name}: points in no cell under the rule of M3"),
            m3_none as f64,
            limit,
        );
    }
    c.finish();
}

// ---- T1, T3, T4 ---------------------------------------------------------------------------------------------------------------

/// ADR 0260 T1 and T3 on P1 and P2, T4 on P2: points on shared faces.
#[test]
fn t_points_on_shared_faces() {
    let mut c = Checks::new();
    for (name, pr) in [("P1", p1()), ("P2", p2())] {
        let mesh = &pr.mesh;
        let loc = CellLocator::new(mesh, 0..mesh.n_cells());
        // T1
        let (mut differ, mut refused) = (0, 0);
        let ni = mesh.n_internal_faces();
        for f in 0..ni {
            let x = mesh.face_centres()[f];
            let a = loc.locate(mesh, x);
            differ += usize::from(a != loc.locate_brute_force(mesh, x));
            refused += usize::from(!a.is_some_and(|k| pr.contains(k, x)));
        }
        c.none(
            &format!("T1 {name}: face centres located otherwise than by the full search"),
            differ,
            ni,
        );
        c.none(
            &format!("T1 {name}: face centres whose cell is refused by the reference"),
            refused,
            ni,
        );
        // T3
        let mut around = vec![Vec::new(); mesh.n_points()];
        for k in 0..mesh.n_cells() {
            for q in cell_points(mesh, k) {
                around[q].push(k);
            }
        }
        let (mut differ, mut wrong, mut n) = (0, 0, 0);
        let eps = 1e-9;
        for (q, cells) in around.iter().enumerate() {
            let x = mesh.points()[q];
            let y = affine_inverse(x);
            if y[0] < eps
                || y[1] < eps
                || y[0] > pr.side - eps
                || y[1] > pr.side - eps
                || y[2] < eps
                || y[2] > pr.height() - eps
            {
                continue;
            }
            n += 1;
            let a = loc.locate(mesh, x);
            differ += usize::from(a != loc.locate_brute_force(mesh, x));
            wrong += usize::from(!a.is_some_and(|k| cells.contains(&k)));
        }
        c.none(
            &format!("T3 {name}: mesh points located otherwise than by the full search"),
            differ,
            n,
        );
        c.none(
            &format!("T3 {name}: mesh points located in a cell that does not have them"),
            wrong,
            n,
        );
        // T4
        if name == "P2" {
            let mut g = Lcg(SEED);
            let n = 20_000;
            let n2 = pr.n_base();
            let kk = pr.z.len() - 1;
            let (mut stray, mut differ) = (0, 0);
            for _ in 0..n {
                let block = ((g.next() * (n2 / 3) as f64) as usize).min(n2 / 3 - 1);
                let cb = 3 * block + 1 + usize::from(g.next() >= 0.5);
                let k = 1 + ((g.next() * (kk - 1) as f64) as usize).min(kk - 2);
                let t = pr.tris[cb][0];
                let (mut r1, mut r2) = (g.next(), g.next());
                if r1 + r2 > 1.0 {
                    (r1, r2) = (1.0 - r1, 1.0 - r2);
                }
                let (p0, p1, p2) = (pr.base[t[0]], pr.base[t[1]], pr.base[t[2]]);
                let y = [
                    p0[0] + r1 * (p1[0] - p0[0]) + r2 * (p2[0] - p0[0]),
                    p0[1] + r1 * (p1[1] - p0[1]) + r2 * (p2[1] - p0[1]),
                    pr.z[k],
                ];
                let x = affine(y);
                let allowed = [(k - 1) * n2 + cb, k * n2 + cb];
                let cells = loc.containing_cells(mesh, x);
                stray += usize::from(cells.is_empty() || cells.iter().any(|q| !allowed.contains(q)));
                differ += usize::from(loc.locate(mesh, x) != loc.locate_brute_force(mesh, x));
            }
            c.none(
                "T4 P2: notch points on a layer plane accepted by another cell than the two prisms of the notch, or by none",
                stray,
                n,
            );
            c.none(
                "T4 P2: notch points located otherwise than by the full search",
                differ,
                n,
            );
        }
    }
    c.finish();
}

// ---- A3 -------------------------------------------------------------------------------------------------------------------------

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

fn mesh_at(ov: &Overset, pose: &Pose) -> Mesh {
    moved_mesh(&ov.composite.mesh, ov.composite.points_at(pose)).unwrap()
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

/// ADR 0260 A3: an overset with P2 as the background and a box of 8^3 cells as the component.
#[test]
fn a3_overset_on_a_non_convex_background() {
    let mut c = Checks::new();
    let pr = p2();
    let comp = box_mesh([8; 3], [0.8; 3], [Grading::Uniform; 3]).unwrap();
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
            .map(String::from)
            .to_vec(),
        hole: Arc::new(|_| false),
    };
    let ov = Overset::new(compose(&pr.mesh, &comp, "c_").unwrap(), spec).unwrap();
    let pose = Pose {
        origin: [0.4; 3],
        axis: vec3::unit([1.0, 2.0, 3.0]),
        angle: 0.2,
        translation: vec3::sub(affine([0.9, 0.9, 0.8]), [0.4; 3]),
    };
    let reference = &ov.composite.mesh;
    let nb = ov.composite.n_background_cells;
    println!(
        "A3: background non-convex cells {}, ear-clipped faces {}",
        ov.background_locator().n_nonconvex(),
        ov.background_locator().n_ear_clipped()
    );
    let moved = mesh_at(&ov, &pose);
    let cc = moved.cell_centres();
    let fmax_lin = cc.iter().map(|&x| linear_field(x).abs()).fold(0.0, f64::max);
    let fmax_q3 = cc.iter().map(|&x| q3(x).abs()).fold(0.0, f64::max);
    let mut quad = ov.clone();
    quad.set_donor_interpolation(DonorInterpolation::Quadratic);
    for (mode, o) in [("default", &ov), ("quadratic", &quad)] {
        let conn = match o.connectivity(&moved, &pose) {
            Ok(conn) => conn,
            Err(e) => {
                c.holds(&format!("A3 {mode}: connectivity"), false, &format!("failed: {e}"));
                continue;
            }
        };
        let n_fringe = conn.kind.iter().filter(|k| **k == CellKind::Fringe).count();
        c.holds(
            &format!("A3 {mode}: connectivity"),
            conn.stencils.len() == n_fringe && n_fringe > 0,
            &format!(
                "{} stencils for {n_fringe} fringe cells ({} background, {} component)",
                conn.stencils.len(),
                conn.n_fringe_background(),
                conn.n_fringe_component()
            ),
        );
        let (mut differ, mut refused) = (0, 0);
        let (mut sum, mut lin, mut quadratic) = (0.0f64, 0.0f64, 0.0f64);
        for (cell, st) in &conn.stencils {
            let (x, _, loc) = donor_query(o, &pose, cc, *cell);
            let a = loc.locate(reference, x);
            differ += usize::from(a != loc.locate_brute_force(reference, x) || a != Some(st.cells[0]));
            // the background donors by the reference of P2; the component (a box) has no non-convex cell
            if st.cells[0] < nb {
                refused += usize::from(!pr.contains(st.cells[0], x));
            }
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
            if mode == "quadratic" && conn.fallbacks.binary_search(cell).is_err() {
                quadratic = quadratic.max(at(&q3) / fmax_q3);
            }
        }
        c.none(
            &format!("A3 {mode}: donors that differ from the full search or the stencil"),
            differ,
            conn.stencils.len(),
        );
        c.none(
            &format!("A3 {mode}: background donors that do not contain the centre by the reference"),
            refused,
            conn.stencils.len(),
        );
        if mode == "default" {
            c.at_most("A3 default |sum w - 1|", sum, 1e-12);
            c.at_most("A3 default linear field", lin, 1e-11);
        } else {
            c.at_most(
                "A3 quadratic q3 over the stencils that are not fallbacks",
                quadratic,
                1e-11,
            );
            println!(
                "A3 quadratic: {} fallbacks of {} stencils",
                conn.n_fallback(),
                conn.stencils.len()
            );
        }
    }
    c.finish();
}

// ---- A4b', A4e: the set-ups of ADR 0251 and ADR 0253 (the same code as crates/rcfd-mesh/tests/overset_f352.rs) ---------------

const DEPTH: f64 = 0.1;
/// Background cell size of level 1 of RL and TG_L.
const H1: f64 = PI / 40.0;

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

fn tg(level: usize) -> Overset {
    box_overset(&tg_background(level), 0.6, 20 * level, 0.25)
}

fn u_overset() -> Overset {
    let bs = uniform(-2.5, 5.0, 50);
    box_overset(&rectilinear_2d(&bs, &bs, DEPTH).unwrap(), 0.6, 16, 0.25)
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

fn b3s() -> Overset {
    let bg = sheared_box(28, [-0.08, 0.05, 0.1]);
    let comp = sheared_box(14, [0.1, 0.05, 0.08]);
    let spec = OversetSpec {
        outer_patches: ["c_xmin", "c_xmax", "c_ymin", "c_ymax", "c_zmin", "c_zmax"]
            .map(String::from)
            .to_vec(),
        hole: Arc::new(|x: Vec3| (0..3).map(|d| (x[d] - 0.7).abs()).fold(0.0, f64::max) < 0.15),
    };
    Overset::new(compose(&bg, &comp, "c_").unwrap(), spec).unwrap()
}

fn fb() -> Overset {
    box_overset(&tg_background(1), 7.5 * H1, 20, 6.1 * H1)
}

/// ADR 0260 A4b' (no cell of a convex set-up is reclassified) and A4e (B3d takes no new path; its donors are those of the
/// rule of ADR 0253).
#[test]
fn a4_convex_meshes_are_not_reclassified() {
    let mut c = Checks::new();
    let cases: Vec<(&str, Overset)> = vec![
        ("RL level 1", rl(1)),
        ("RL level 2", rl(2)),
        ("RL level 4", rl(4)),
        ("TG_1", tg(1)),
        ("TG_2", tg(2)),
        ("TG_4", tg(4)),
        ("U", u_overset()),
        ("TR", tr()),
        ("DC", dc()),
        ("FB", fb()),
        ("B3", b3(false).0),
        ("B3s", b3s()),
        ("B3d", b3(true).0),
    ];
    for (name, ov) in &cases {
        let m = &ov.composite.mesh;
        let tol = m3_tolerances(m);
        let (bl, cl) = (ov.background_locator(), ov.component_locator());
        let ratio = largest_outward_ratio(m, &tol, 0..m.n_cells());
        println!(
            "A4b' {name}: {} cells, warped {}, largest outward vertex distance over the tolerance {ratio:.3e}",
            m.n_cells(),
            bl.n_warped() + cl.n_warped()
        );
        c.none(
            &format!("A4b' {name}: non-convex cells"),
            bl.n_nonconvex() + cl.n_nonconvex(),
            m.n_cells(),
        );
    }
    // A4e: B3d
    let (ov, pose) = b3(true);
    let m = &ov.composite.mesh;
    let (bl, cl) = (ov.background_locator(), ov.component_locator());
    c.none(
        "A4e B3d: ear-clipped faces",
        bl.n_ear_clipped() + cl.n_ear_clipped(),
        m.n_faces(),
    );
    c.none(
        "A4e B3d: unsupported faces",
        bl.n_unsupported() + cl.n_unsupported(),
        m.n_faces(),
    );
    let rule = F352Rule::new(m);
    let moved = mesh_at(&ov, &pose);
    let cc = moved.cell_centres();
    let conn = ov.connectivity(&moved, &pose).unwrap();
    let mut differ = 0;
    for (cell, st) in &conn.stencils {
        let (x, range, _) = donor_query(&ov, &pose, cc, *cell);
        differ += usize::from(rule.locate(m, range, x) != Some(st.cells[0]));
    }
    c.none(
        "A4e B3d: fringe donors that differ from the rule of ADR 0253",
        differ,
        conn.stencils.len(),
    );
    let mut g = Lcg(SEED);
    let n = 20_000;
    let mut differ = 0;
    for _ in 0..n {
        let x = [0, 1, 2].map(|_| 0.05 + 2.3 * g.next());
        differ += usize::from(bl.locate(m, x) != rule.locate(m, ov.composite.background_cells(), x));
    }
    c.none(
        "A4e B3d: background points located otherwise than by the rule of ADR 0253",
        differ,
        n,
    );
    c.finish();
}
