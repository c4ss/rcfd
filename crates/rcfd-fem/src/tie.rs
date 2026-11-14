//! Node-to-surface search, tied non-matching meshes, shell and beam joints. Declared by ADR 0440 (F-408, the scaffold of ADR
//! 0311); filled by ADR 0456 (F-426, the package of ADR 0329).
//!
//! A tie joins a slave surface set to a master surface set of parts meshed apart ([`assemble_parts`]). Every row it makes
//! is a [`Constraint::Linear`] of the transformation `x = T q + g` (ADR 0296): nothing in the elimination changes.
//!
//! - [`TieMethod::NodeToSurface`]: each slave node is projected onto the nearest master face (a bucket grid of the master
//!   faces, Gauss-Newton on the face map) and its displacement is the master interpolation there, `u_s = sum N_a u_a`. It
//!   does not pass the patch test on non-matching meshes (ADR 0456 arithmetic 1).
//! - [`TieMethod::Mortar`]: `int N_j (u_s - u_m) dA = 0` for each slave node `j` with the standard multiplier space (the
//!   slave face functions), integrated on the segments of each slave face cut by the master faces (Sutherland-Hodgman
//!   clipping in the slave face plane, a collapsed 4 x 4 Gauss rule per triangle); `D u_s = M u_m` is solved for the slave
//!   unknowns, `u_s = D^-1 M u_m`, per component with the prescribed slave components moved to the right side. It passes the
//!   patch test on flat interfaces with straight-edged faces (ADR 0456 arithmetic 2).
//!
//! A slave node farther than the tolerance from every master face is refused ([`FemError::TieGap`]).
//!
//! Joints ([`beam_to_solid`], [`shell_to_solid`]): a node with six unknowns follows the least-squares rigid motion of a set
//! of solid nodes (unit weights; ADR 0456 arithmetic 4), with the rows in solved form; for a set on one line (the column
//! of a shell node) the rotation about the line stays free.

use crate::error::{FemError, Result, SetKind};
use crate::loads::face_shape_functions;
use crate::mesh::{FaceRef, SolidMesh};
use crate::model::{Constraint, LinearTerm, SolidModel};
use rcfd_core::vec3::{cross, dot, norm, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

/// The default gap tolerance, a fraction of the mean master face size `mean sqrt(area)` (ADR 0456 arithmetic 8).
pub const GAP_FACTOR: f64 = 0.05;

/// Slack of the inside test in face coordinates (ADR 0456 D1).
pub const INSIDE_SLACK: f64 = 1e-8;

/// Relative threshold of the null space of a coupling (ADR 0456 D3).
pub const COUPLING_NULL_TOLERANCE: f64 = 1e-10;

/// How the slave side follows the master side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TieMethod {
    /// Each slave node on the master interpolation at its projection.
    NodeToSurface,
    /// The segment-integrated mortar condition with the slave face functions as multipliers.
    Mortar,
}

/// The options of a tie.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct TieOptions {
    /// The method.
    pub method: TieMethod,
    /// The absolute gap tolerance, m; `None`: [`GAP_FACTOR`] times the mean master face size.
    pub tolerance: Option<f64>,
}

impl TieOptions {
    /// The options of `method` with the default tolerance.
    pub fn new(method: TieMethod) -> TieOptions {
        TieOptions {
            method,
            tolerance: None,
        }
    }
    /// The same with an absolute tolerance.
    pub fn tolerance(mut self, tolerance: f64) -> TieOptions {
        self.tolerance = Some(tolerance);
        self
    }
}

/// The projection of one slave node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    /// The slave node.
    pub node: usize,
    /// The master face.
    pub face: FaceRef,
    /// The face coordinates.
    pub uv: [f64; 2],
    /// The distance from the node to the projected point, m.
    pub distance: f64,
}

/// What a tie made.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TieReport {
    /// The rows, as linear constraints.
    pub constraints: Vec<Constraint>,
    /// The projection of every slave node, ascending in the node.
    pub projections: Vec<Projection>,
    /// The slave (node, component) pairs prescribed by the model and so without a row of their own.
    pub skipped: Vec<(usize, usize)>,
    /// The tolerance used, m.
    pub tolerance: f64,
    /// The mean master face size, m.
    pub face_size: f64,
    /// The number of terms of all rows (the fill of the transformation).
    pub terms: usize,
}

impl TieReport {
    /// The largest projection distance.
    pub fn max_distance(&self) -> f64 {
        self.projections.iter().fold(0.0, |m, p| m.max(p.distance))
    }
}

fn tie_error(reason: String) -> FemError {
    FemError::InvalidValue { key: "tie", reason }
}

// ------------------------------------------------------------------------------------------------------------ faces

/// The geometry of one face: its nodes and their positions (possibly mapped).
#[derive(Debug, Clone)]
struct Face {
    face: FaceRef,
    nodes: Vec<usize>,
    x: Vec<Vec3>,
}

impl Face {
    fn new(mesh: &SolidMesh, f: FaceRef, map: &dyn Fn(Vec3) -> Vec3) -> Face {
        let nodes = mesh.face_nodes(f);
        let x = nodes.iter().map(|&n| map(mesh.nodes()[n])).collect();
        Face { face: f, nodes, x }
    }

    fn n(&self) -> usize {
        self.nodes.len()
    }

    fn is_triangle(&self) -> bool {
        matches!(self.n(), 3 | 6)
    }

    fn corners(&self) -> usize {
        if self.is_triangle() {
            3
        } else {
            4
        }
    }

    fn centre_uv(&self) -> [f64; 2] {
        if self.is_triangle() {
            [1.0 / 3.0, 1.0 / 3.0]
        } else {
            [0.0, 0.0]
        }
    }

    /// The position, the tangents and the shape functions at `uv`.
    fn eval(&self, uv: [f64; 2]) -> Result<(Vec3, Vec3, Vec3, Vec<f64>)> {
        let (n, d) = face_shape_functions(self.n(), uv)?;
        let (mut x, mut xu, mut xv) = ([0.0; 3], [0.0; 3], [0.0; 3]);
        for a in 0..self.n() {
            for i in 0..3 {
                x[i] += n[a] * self.x[a][i];
                xu[i] += d[a][0] * self.x[a][i];
                xv[i] += d[a][1] * self.x[a][i];
            }
        }
        Ok((x, xu, xv, n))
    }

    fn inside(&self, uv: [f64; 2], slack: f64) -> bool {
        let [u, v] = uv;
        if self.is_triangle() {
            u >= -slack && v >= -slack && u + v <= 1.0 + slack
        } else {
            u.abs() <= 1.0 + slack && v.abs() <= 1.0 + slack
        }
    }

    fn clamp(&self, uv: [f64; 2]) -> [f64; 2] {
        let [mut u, mut v] = uv;
        if self.is_triangle() {
            u = u.max(0.0);
            v = v.max(0.0);
            let s = u + v;
            if s > 1.0 {
                u /= s;
                v /= s;
            }
            [u, v]
        } else {
            [u.clamp(-1.0, 1.0), v.clamp(-1.0, 1.0)]
        }
    }

    /// Gauss-Newton on `(x(u, v) - p) . a = 0`, `(x(u, v) - p) . b = 0`, with `(a, b)` the tangents (closest point) or two
    /// fixed in-plane axes (projection along their normal).
    fn solve(&self, p: Vec3, axes: Option<(Vec3, Vec3)>) -> Result<[f64; 2]> {
        let mut uv = self.centre_uv();
        for _ in 0..25 {
            let (x, xu, xv, _) = self.eval(uv)?;
            let (a, b) = axes.unwrap_or((xu, xv));
            let r = [x[0] - p[0], x[1] - p[1], x[2] - p[2]];
            let (j11, j12, j21, j22) = (dot(a, xu), dot(a, xv), dot(b, xu), dot(b, xv));
            let det = j11 * j22 - j12 * j21;
            if det == 0.0 || !det.is_finite() {
                break;
            }
            let (f1, f2) = (dot(a, r), dot(b, r));
            let du = (j22 * f1 - j12 * f2) / det;
            let dv = (j11 * f2 - j21 * f1) / det;
            uv = [uv[0] - du, uv[1] - dv];
            if du.abs().max(dv.abs()) < 1e-14 {
                break;
            }
        }
        Ok(uv)
    }

    /// The closest point: face coordinates (clamped), distance, whether the unclamped point is inside.
    fn project(&self, p: Vec3) -> Result<([f64; 2], f64, bool)> {
        let uv = self.solve(p, None)?;
        let inside = self.inside(uv, INSIDE_SLACK);
        let uv = self.clamp(uv);
        let (x, _, _, _) = self.eval(uv)?;
        Ok((uv, distance(x, p), inside))
    }

    fn bbox(&self) -> (Vec3, Vec3) {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for x in &self.x {
            for i in 0..3 {
                lo[i] = lo[i].min(x[i]);
                hi[i] = hi[i].max(x[i]);
            }
        }
        (lo, hi)
    }

    fn area_vector(&self) -> Result<Vec3> {
        // The integral of x_u x x_v by the face rule.
        let mut a = [0.0; 3];
        for (uv, w) in crate::loads::face_rule(self.n())? {
            let (_, xu, xv, _) = self.eval(uv)?;
            let c = cross(xu, xv);
            for i in 0..3 {
                a[i] += w * c[i];
            }
        }
        Ok(a)
    }
}

fn distance(a: Vec3, b: Vec3) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn unit(a: Vec3) -> Vec3 {
    let n = norm(a);
    [a[0] / n, a[1] / n, a[2] / n]
}

/// A uniform bucket grid of face boxes.
struct Buckets {
    lo: Vec3,
    cell: f64,
    cells: BTreeMap<[i64; 3], Vec<usize>>,
}

impl Buckets {
    fn new(faces: &[Face], cell: f64, pad: f64) -> Buckets {
        let mut lo = [f64::INFINITY; 3];
        for f in faces {
            let (l, _) = f.bbox();
            for i in 0..3 {
                lo[i] = lo[i].min(l[i] - pad);
            }
        }
        let mut b = Buckets {
            lo,
            cell,
            cells: BTreeMap::new(),
        };
        for (k, f) in faces.iter().enumerate() {
            let (l, h) = f.bbox();
            let a = b.key([l[0] - pad, l[1] - pad, l[2] - pad]);
            let z = b.key([h[0] + pad, h[1] + pad, h[2] + pad]);
            for i in a[0]..=z[0] {
                for j in a[1]..=z[1] {
                    for m in a[2]..=z[2] {
                        b.cells.entry([i, j, m]).or_default().push(k);
                    }
                }
            }
        }
        b
    }

    fn key(&self, p: Vec3) -> [i64; 3] {
        [0, 1, 2].map(|i| ((p[i] - self.lo[i]) / self.cell).floor() as i64)
    }

    fn near(&self, p: Vec3) -> &[usize] {
        self.cells.get(&self.key(p)).map_or(&[], |v| v.as_slice())
    }

    /// The faces of every cell the box `[lo, hi]` touches, ascending.
    fn near_box(&self, lo: Vec3, hi: Vec3) -> BTreeSet<usize> {
        let (a, z) = (self.key(lo), self.key(hi));
        let mut out = BTreeSet::new();
        for (k, v) in self.cells.range(a..=z) {
            if (0..3).all(|i| k[i] >= a[i] && k[i] <= z[i]) {
                out.extend(v.iter().copied());
            }
        }
        out
    }
}

/// The faces of a surface set with their nodes mapped by `map`.
fn faces_of(mesh: &SolidMesh, set: &str, map: &dyn Fn(Vec3) -> Vec3) -> Result<Vec<Face>> {
    let faces = mesh.surface_set(set).ok_or_else(|| FemError::UnknownSet {
        kind: SetKind::Surface,
        name: set.to_string(),
    })?;
    Ok(faces.iter().map(|&f| Face::new(mesh, f, map)).collect())
}

// ------------------------------------------------------------------------------------------------------------ weights

/// One row of a tie: slave node, component, terms `(node, coefficient)` of `u_s = sum c u` on that component.
pub(crate) type WeightRow = (usize, usize, Vec<(usize, f64)>);

/// The weights of a tie: per slave node and component, the terms `(node, coefficient)` of `u_s = sum c u`, plus the report
/// fields.
pub(crate) struct Weights {
    /// (slave node, component, terms over (node, component-free) unknowns).
    pub(crate) rows: Vec<WeightRow>,
    pub(crate) projections: Vec<Projection>,
    pub(crate) skipped: Vec<(usize, usize)>,
    pub(crate) tolerance: f64,
    pub(crate) face_size: f64,
}

/// The weights of the slave surface `slave` on the master surface `master` with its nodes mapped by `map` (the identity for
/// a tie, the rotation by `2 pi / N` for a cyclic boundary). `prescribed` holds the slave (node, component) pairs without
/// a row of their own; `components` the components that get rows.
pub(crate) fn weights(
    mesh: &SolidMesh,
    slave: &str,
    master: &str,
    map: &dyn Fn(Vec3) -> Vec3,
    options: &TieOptions,
    prescribed: &BTreeSet<(usize, usize)>,
    exclude: &BTreeSet<usize>,
) -> Result<Weights> {
    let masters = faces_of(mesh, master, map)?;
    if masters.is_empty() {
        return Err(tie_error(format!("the master surface '{master}' has no face")));
    }
    let slave_faces = faces_of(mesh, slave, &|x| x)?;
    let master_nodes: BTreeSet<usize> = masters.iter().flat_map(|f| f.nodes.iter().copied()).collect();
    let mut slave_nodes: Vec<usize> = mesh
        .surface_set_nodes(slave)?
        .into_iter()
        .filter(|p| !exclude.contains(p))
        .collect();
    slave_nodes.sort_unstable();
    if let Some(&p) = slave_nodes.iter().find(|p| master_nodes.contains(p)) {
        return Err(tie_error(format!(
            "node {p} is on both the slave surface '{slave}' and the master surface '{master}'"
        )));
    }
    let mut size = 0.0;
    for f in &masters {
        size += norm(f.area_vector()?).sqrt();
    }
    let face_size = size / masters.len() as f64;
    let tolerance = match options.tolerance {
        Some(t) if t > 0.0 && t.is_finite() => t,
        Some(t) => {
            return Err(FemError::InvalidValue {
                key: "tolerance",
                reason: format!("tie tolerance {t} must be positive and finite"),
            })
        }
        None => GAP_FACTOR * face_size,
    };
    let buckets = Buckets::new(&masters, face_size, tolerance);
    // The projection of every slave node.
    let mut projections = Vec::with_capacity(slave_nodes.len());
    let mut weights_of: Vec<Vec<(usize, f64)>> = Vec::with_capacity(slave_nodes.len());
    for &s in &slave_nodes {
        let p = mesh.nodes()[s];
        let mut best: Option<(bool, f64, usize, [f64; 2])> = None;
        for &k in buckets.near(p) {
            let (uv, d, inside) = masters[k].project(p)?;
            let better = match best {
                None => true,
                Some((bi, bd, _, _)) => (inside && !bi) || (inside == bi && d < bd),
            };
            if better {
                best = Some((inside, d, k, uv));
            }
        }
        let Some((_, d, k, uv)) = best.filter(|b| b.1 <= tolerance) else {
            let d = best.map_or_else(
                || {
                    masters
                        .iter()
                        .map(|f| f.project(p).map(|r| r.1).unwrap_or(f64::INFINITY))
                        .fold(f64::INFINITY, f64::min)
                },
                |b| b.1,
            );
            return Err(FemError::TieGap {
                node: s,
                distance: d,
                tolerance,
            });
        };
        let (_, _, _, n) = masters[k].eval(uv)?;
        weights_of.push(
            masters[k]
                .nodes
                .iter()
                .zip(&n)
                .filter(|(_, &w)| w != 0.0)
                .map(|(&a, &w)| (a, w))
                .collect(),
        );
        projections.push(Projection {
            node: s,
            face: masters[k].face,
            uv,
            distance: d,
        });
    }
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    match options.method {
        TieMethod::NodeToSurface => {
            for (i, &s) in slave_nodes.iter().enumerate() {
                for c in 0..3 {
                    if prescribed.contains(&(s, c)) {
                        skipped.push((s, c));
                        continue;
                    }
                    rows.push((s, c, weights_of[i].clone()));
                }
            }
        }
        TieMethod::Mortar => {
            let (d, m, mnodes) = mortar_matrices(&slave_faces, &slave_nodes, &masters, &buckets, tolerance)?;
            let ns = slave_nodes.len();
            for (j, &s) in slave_nodes.iter().enumerate() {
                if d[j * ns + j] <= 0.0 {
                    return Err(tie_error(format!(
                        "slave node {s} of '{slave}': no segment of a master face of '{master}' covers its faces"
                    )));
                }
            }
            for c in 0..3 {
                let free: Vec<usize> = (0..ns)
                    .filter(|&j| !prescribed.contains(&(slave_nodes[j], c)))
                    .collect();
                let held: Vec<usize> = (0..ns).filter(|&j| prescribed.contains(&(slave_nodes[j], c))).collect();
                for &j in &held {
                    skipped.push((slave_nodes[j], c));
                }
                if free.is_empty() {
                    continue;
                }
                let nf = free.len();
                let nm = mnodes.len();
                let mut a = vec![0.0; nf * nf];
                let ncol = nm + held.len();
                let mut b = vec![0.0; nf * ncol];
                for (r, &j) in free.iter().enumerate() {
                    for (q, &k) in free.iter().enumerate() {
                        a[r * nf + q] = d[j * ns + k];
                    }
                    for q in 0..nm {
                        b[r * ncol + q] = m[j * nm + q];
                    }
                    for (q, &k) in held.iter().enumerate() {
                        b[r * ncol + nm + q] = -d[j * ns + k];
                    }
                }
                let x = solve_dense(&mut a, nf, &mut b, ncol)
                    .ok_or_else(|| tie_error(format!("the mortar matrix D of '{slave}' is singular")))?;
                for (r, &j) in free.iter().enumerate() {
                    let mut terms = Vec::new();
                    for (q, &a) in mnodes.iter().enumerate() {
                        let w = x[r * ncol + q];
                        if w != 0.0 {
                            terms.push((a, w));
                        }
                    }
                    for (q, &k) in held.iter().enumerate() {
                        let w = x[r * ncol + nm + q];
                        if w != 0.0 {
                            terms.push((slave_nodes[k], w));
                        }
                    }
                    rows.push((slave_nodes[j], c, terms));
                }
            }
            rows.sort_by_key(|r| (r.0, r.1));
        }
    }
    skipped.sort_unstable();
    Ok(Weights {
        rows,
        projections,
        skipped,
        tolerance,
        face_size,
    })
}

// Four-point Gauss-Legendre rule on [0, 1].
const G4_X: [f64; 4] = [
    0.069_431_844_202_973_71,
    0.330_009_478_207_571_9,
    0.669_990_521_792_428_1,
    0.930_568_155_797_026_3,
];
const G4_W: [f64; 4] = [
    0.173_927_422_568_726_9,
    0.326_072_577_431_273_1,
    0.326_072_577_431_273_1,
    0.173_927_422_568_726_9,
];

/// Clips the convex polygon `subject` by the convex counter-clockwise polygon `clip` (Sutherland-Hodgman).
fn clip_polygon(subject: &[[f64; 2]], clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = subject.to_vec();
    let nc = clip.len();
    for e in 0..nc {
        if out.is_empty() {
            break;
        }
        let (a, b) = (clip[e], clip[(e + 1) % nc]);
        let side = |p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let input = std::mem::take(&mut out);
        let n = input.len();
        for i in 0..n {
            let (p, q) = (input[i], input[(i + 1) % n]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 {
                out.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                out.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]);
            }
        }
    }
    out
}

fn signed_area(poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    (0..n)
        .map(|i| {
            let (p, q) = (poly[i], poly[(i + 1) % n]);
            p[0] * q[1] - q[0] * p[1]
        })
        .sum::<f64>()
        / 2.0
}

/// The mortar matrices `D` (slave x slave, dense, row-major) and `M` (slave x master nodes), and the master nodes.
fn mortar_matrices(
    slave_faces: &[Face],
    slave_nodes: &[usize],
    masters: &[Face],
    buckets: &Buckets,
    tolerance: f64,
) -> Result<(Vec<f64>, Vec<f64>, Vec<usize>)> {
    let mnodes: Vec<usize> = masters
        .iter()
        .flat_map(|f| f.nodes.iter().copied())
        .collect::<BTreeSet<usize>>()
        .into_iter()
        .collect();
    let mindex: BTreeMap<usize, usize> = mnodes.iter().enumerate().map(|(i, &a)| (a, i)).collect();
    let sindex: BTreeMap<usize, usize> = slave_nodes.iter().enumerate().map(|(i, &a)| (a, i)).collect();
    let (ns, nm) = (slave_nodes.len(), mnodes.len());
    let mut d = vec![0.0; ns * ns];
    let mut m = vec![0.0; ns * nm];
    for sf in slave_faces {
        // Faces with a node outside the slave list (an excluded node) take part with the listed nodes only.
        let nrm = unit(sf.area_vector()?);
        let nc = sf.corners();
        let c: Vec3 = {
            let mut c = [0.0; 3];
            for x in &sf.x[..nc] {
                for i in 0..3 {
                    c[i] += x[i] / nc as f64;
                }
            }
            c
        };
        let e1 = unit({
            let t = sub(sf.x[1], sf.x[0]);
            let tn = dot(t, nrm);
            [t[0] - tn * nrm[0], t[1] - tn * nrm[1], t[2] - tn * nrm[2]]
        });
        let e2 = cross(nrm, e1);
        let to2 = |x: Vec3| {
            let r = sub(x, c);
            [dot(r, e1), dot(r, e2)]
        };
        let mut poly_s: Vec<[f64; 2]> = sf.x[..nc].iter().map(|&x| to2(x)).collect();
        if signed_area(&poly_s) < 0.0 {
            poly_s.reverse();
        }
        let area_s = signed_area(&poly_s);
        // Candidate master faces: those of the buckets the slave face's box touches.
        let (lo, hi) = sf.bbox();
        let cand = buckets.near_box(lo, hi);
        for &k in &cand {
            let mf = &masters[k];
            // Only faces near the slave plane.
            let mc = mf.corners();
            if mf.x[..mc].iter().all(|&x| dot(sub(x, c), nrm).abs() > tolerance) {
                continue;
            }
            let mut poly_m: Vec<[f64; 2]> = mf.x[..mc].iter().map(|&x| to2(x)).collect();
            if signed_area(&poly_m) < 0.0 {
                poly_m.reverse();
            }
            let q = clip_polygon(&poly_s, &poly_m);
            if q.len() < 3 || signed_area(&q) <= 1e-14 * area_s {
                continue;
            }
            for t in 1..q.len() - 1 {
                let (q0, q1, q2) = (q[0], q[t], q[t + 1]);
                let tri = [q0, q1, q2];
                let at = signed_area(&tri);
                if at <= 0.0 {
                    continue;
                }
                for (&s, &ws) in G4_X.iter().zip(&G4_W) {
                    for (&r, &wr) in G4_X.iter().zip(&G4_W) {
                        let (u, v) = (s * (1.0 - r), s * r);
                        let p2 = [
                            q0[0] + u * (q1[0] - q0[0]) + v * (q2[0] - q0[0]),
                            q0[1] + u * (q1[1] - q0[1]) + v * (q2[1] - q0[1]),
                        ];
                        let w = 2.0 * at * ws * wr * s;
                        let p = [
                            c[0] + p2[0] * e1[0] + p2[1] * e2[0],
                            c[1] + p2[0] * e1[1] + p2[1] * e2[1],
                            c[2] + p2[0] * e1[2] + p2[1] * e2[2],
                        ];
                        let us = sf.solve(p, Some((e1, e2)))?;
                        let um = mf.solve(p, Some((e1, e2)))?;
                        let (_, _, _, ns_f) = sf.eval(us)?;
                        let (_, _, _, nm_f) = mf.eval(um)?;
                        for (ja, &j) in sf.nodes.iter().enumerate() {
                            let Some(&jj) = sindex.get(&j) else { continue };
                            let wj = w * ns_f[ja];
                            for (ka, &kn) in sf.nodes.iter().enumerate() {
                                if let Some(&kk) = sindex.get(&kn) {
                                    d[jj * ns + kk] += wj * ns_f[ka];
                                }
                            }
                            for (aa, &an) in mf.nodes.iter().enumerate() {
                                m[jj * nm + mindex[&an]] += wj * nm_f[aa];
                            }
                        }
                    }
                }
            }
        }
    }
    Ok((d, m, mnodes))
}

/// Solves `A X = B` (`A` n x n, `B` n x m, row-major) by Gaussian elimination with partial pivoting; `None` if singular.
fn solve_dense(a: &mut [f64], n: usize, b: &mut [f64], m: usize) -> Option<Vec<f64>> {
    let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs()));
    for k in 0..n {
        let p = (k..n).max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))?;
        if a[p * n + k].abs() <= 1e-14 * scale {
            return None;
        }
        if p != k {
            for j in 0..n {
                a.swap(k * n + j, p * n + j);
            }
            for j in 0..m {
                b.swap(k * m + j, p * m + j);
            }
        }
        for i in k + 1..n {
            let f = a[i * n + k] / a[k * n + k];
            if f == 0.0 {
                continue;
            }
            for j in k..n {
                a[i * n + j] -= f * a[k * n + j];
            }
            for j in 0..m {
                b[i * m + j] -= f * b[k * m + j];
            }
        }
    }
    let mut x = vec![0.0; n * m];
    for i in (0..n).rev() {
        for j in 0..m {
            let mut s = b[i * m + j];
            for k in i + 1..n {
                s -= a[i * n + k] * x[k * m + j];
            }
            x[i * m + j] = s / a[i * n + i];
        }
    }
    Some(x)
}

/// The slave (node, component) pairs prescribed by the model's `Displacement` constraints.
pub(crate) fn prescribed_pairs(model: &SolidModel) -> BTreeSet<(usize, usize)> {
    let mut out = BTreeSet::new();
    for c in model.constraints() {
        if let Constraint::Displacement { set, component, .. } = c {
            if let Some(nodes) = model.mesh().node_set(set) {
                for &n in nodes {
                    out.insert((n, *component));
                }
            }
        }
    }
    out
}

/// The rows of a tie of the slave surface set `slave` to the master surface set `master` of `model` (ADR 0456 D1, D2).
/// A slave component prescribed by a `Displacement` constraint of the model gets no row of its own (add the tie after
/// the displacement constraints). Refuses a slave node farther than the tolerance from every master face
/// ([`FemError::TieGap`]) and a node on both surfaces.
pub fn tie(model: &SolidModel, slave: &str, master: &str, options: &TieOptions) -> Result<TieReport> {
    let prescribed = prescribed_pairs(model);
    let w = weights(
        model.mesh(),
        slave,
        master,
        &|x| x,
        options,
        &prescribed,
        &BTreeSet::new(),
    )?;
    let dofs = model.dof_map();
    let mut constraints = Vec::with_capacity(w.rows.len());
    let mut terms = 0;
    for (s, c, ws) in &w.rows {
        let mut t = vec![LinearTerm::new(*s, *c, 1.0)];
        for &(a, coef) in ws {
            if dofs.equation(a, *c).is_none() {
                return Err(FemError::MissingComponent {
                    role: "tie",
                    node: a,
                    component: *c,
                    available: dofs.components(a),
                });
            }
            t.push(LinearTerm::new(a, *c, -coef));
        }
        terms += t.len();
        constraints.push(Constraint::linear(t, 0.0));
    }
    Ok(TieReport {
        constraints,
        projections: w.projections,
        skipped: w.skipped,
        tolerance: w.tolerance,
        face_size: w.face_size,
        terms,
    })
}

/// [`tie`], then adds its constraints to the model.
pub fn add_tie(model: &mut SolidModel, slave: &str, master: &str, options: &TieOptions) -> Result<TieReport> {
    let report = tie(model, slave, master, options)?;
    for c in &report.constraints {
        model.add_constraint(c.clone())?;
    }
    Ok(report)
}

/// The meshes side by side without merging any node: nodes and elements numbered part after part, every set renamed
/// `name.set`, block properties kept (ADR 0456 D1).
pub fn assemble_parts(parts: &[(&str, &SolidMesh)]) -> Result<SolidMesh> {
    let mut nodes = Vec::new();
    for (_, m) in parts {
        nodes.extend_from_slice(m.nodes());
    }
    let mut out = SolidMesh::new(nodes)?;
    let (mut node0, mut elem0) = (0usize, 0usize);
    for (name, m) in parts {
        for b in m.blocks() {
            let conn = b.connectivity().iter().map(|&n| n + node0).collect();
            let k = out.add_block(b.kind(), conn)?;
            if let Some(p) = b.property() {
                out.set_block_property(k, p.clone())?;
            }
        }
        for s in m.node_set_names() {
            let v = m.node_set(s).expect("listed").iter().map(|&n| n + node0).collect();
            out.add_node_set(&format!("{name}.{s}"), v)?;
        }
        for s in m.element_set_names() {
            let v = m.element_set(s).expect("listed").iter().map(|&e| e + elem0).collect();
            out.add_element_set(&format!("{name}.{s}"), v)?;
        }
        for s in m.surface_set_names() {
            let v = m
                .surface_set(s)
                .expect("listed")
                .iter()
                .map(|f| FaceRef {
                    element: f.element + elem0,
                    face: f.face,
                })
                .collect();
            out.add_surface_set(&format!("{name}.{s}"), v)?;
        }
        node0 += m.nodes().len();
        elem0 += m.n_elements();
    }
    Ok(out)
}

// ------------------------------------------------------------------------------------------------------------ joints

/// The rows making node `b` (six unknowns) follow the least-squares rigid motion of the solid nodes `set` (unit weights;
/// ADR 0456 arithmetic 4), in solved form.
pub fn coupling(model: &SolidModel, b: usize, set: &[usize]) -> Result<Vec<Constraint>> {
    let dofs = model.dof_map();
    let mesh = model.mesh();
    if b >= mesh.nodes().len() {
        return Err(FemError::MissingNode {
            role: "coupling",
            node: b,
            n_nodes: mesh.nodes().len(),
        });
    }
    if dofs.components(b) != 6 {
        return Err(tie_error(format!(
            "coupling node {b} has {} unknowns: a joint couples a beam or shell node (six unknowns)",
            dofs.components(b)
        )));
    }
    if set.is_empty() || set.contains(&b) {
        return Err(tie_error(format!(
            "coupling of node {b}: empty set or the node itself in it"
        )));
    }
    let xb = mesh.nodes()[b];
    let r: Vec<Vec3> = set.iter().map(|&i| sub(mesh.nodes()[i], xb)).collect();
    let ell = (r.iter().map(|v| dot(*v, *v)).sum::<f64>() / r.len() as f64).sqrt();
    let ell = if ell > 0.0 { ell } else { 1.0 };
    // A = sum B_i^T B_i in the scaled unknowns y = (t, ell theta), B_i = [I, -[r_i/ell]x].
    let cross_m = |v: Vec3| [[0.0, -v[2], v[1]], [v[2], 0.0, -v[0]], [-v[1], v[0], 0.0]];
    let mut a = [0.0f64; 36];
    let mut bt: Vec<[[f64; 3]; 6]> = Vec::with_capacity(set.len());
    for ri in &r {
        let rs = [ri[0] / ell, ri[1] / ell, ri[2] / ell];
        let x = cross_m(rs);
        // B (3 x 6) = [I, -X] with X = [r']x; B^T = [I; -X^T] = [I; X] (X skew): (B^T)_{3+k, c} = X_{k, c}.
        let mut btm = [[0.0; 3]; 6];
        for c in 0..3 {
            btm[c][c] = 1.0;
            for k in 0..3 {
                btm[3 + k][c] = x[k][c];
            }
        }
        let mut bm = [[0.0; 6]; 3];
        for c in 0..3 {
            for k in 0..6 {
                bm[c][k] = btm[k][c];
            }
        }
        for p in 0..6 {
            for q in 0..6 {
                a[6 * p + q] += (0..3).map(|c| btm[p][c] * bm[c][q]).sum::<f64>();
            }
        }
        bt.push(btm);
    }
    let eig = rcfd_linalg::eigen::symmetric_eigen(&a, 6).map_err(|e| FemError::Solver {
        solver: "symmetric_eigen",
        reason: e.to_string(),
    })?;
    let lmax = eig.values[5];
    let mut pinv = [0.0f64; 36];
    let mut null: Vec<[f64; 6]> = Vec::new();
    for k in 0..6 {
        let v = &eig.vectors[k];
        if eig.values[k] <= COUPLING_NULL_TOLERANCE * lmax {
            null.push([v[0], v[1], v[2], v[3], v[4], v[5]]);
        } else {
            for p in 0..6 {
                for q in 0..6 {
                    pinv[6 * p + q] += v[p] * v[q] / eig.values[k];
                }
            }
        }
    }
    if null.len() > 1 {
        return Err(tie_error(format!(
            "coupling of node {b}: the {} nodes leave {} rigid motions undetermined (a point set)",
            set.len(),
            null.len()
        )));
    }
    // Free components: pivot on the null basis (largest entry).
    let mut free: Vec<usize> = Vec::new();
    let mut k_dep = [[0.0f64; 6]; 6]; // K[d][f]: y_d += K y_f
    if let Some(nv) = null.first() {
        let f = (0..6)
            .max_by(|&i, &j| nv[i].abs().total_cmp(&nv[j].abs()))
            .expect("six entries");
        free.push(f);
        for d in 0..6 {
            if d != f {
                k_dep[d][f] = nv[d] / nv[f];
            }
        }
    }
    let scale = |k: usize| if k < 3 { 1.0 } else { ell };
    let mut out = Vec::new();
    for d in 0..6 {
        if free.contains(&d) {
            continue;
        }
        // H_d = pinv_d - sum_f K_df pinv_f; y_d = H_d . b + sum_f K_df y_f; x = y / scale.
        let mut h = [0.0f64; 6];
        for q in 0..6 {
            h[q] = pinv[6 * d + q];
            for &f in &free {
                h[q] -= k_dep[d][f] * pinv[6 * f + q];
            }
        }
        let mut terms = vec![LinearTerm::new(b, d, 1.0)];
        for &f in &free {
            let coef = k_dep[d][f] * scale(f) / scale(d);
            if coef.abs() > 1e-14 {
                terms.push(LinearTerm::new(b, f, -coef));
            }
        }
        for (i, &node) in set.iter().enumerate() {
            for c in 0..3 {
                let coef: f64 = (0..6).map(|q| h[q] * bt[i][q][c]).sum::<f64>() / scale(d);
                if coef.abs() > 1e-14 {
                    if dofs.equation(node, c).is_none() {
                        return Err(FemError::MissingComponent {
                            role: "coupling",
                            node,
                            component: c,
                            available: dofs.components(node),
                        });
                    }
                    terms.push(LinearTerm::new(node, c, -coef));
                }
            }
        }
        out.push(Constraint::linear(terms, 0.0));
    }
    Ok(out)
}

/// A beam node continuing a solid: the node follows the least-squares rigid motion of every node of the surface set
/// (ADR 0456 D3).
pub fn beam_to_solid(model: &SolidModel, beam_node: usize, surface: &str) -> Result<Vec<Constraint>> {
    let set = model.mesh().surface_set_nodes(surface)?;
    coupling(model, beam_node, &set)
}

/// The shell nodes of the node set `shell_nodes` continuing a solid: each follows the least-squares rigid motion of the
/// column of nodes of the surface set within `tolerance` of the line through it along its normal (the mean unit normal of
/// its shell elements), the rotation about the line left free (ADR 0456 D3). A node with fewer than two column nodes is
/// refused naming it.
pub fn shell_to_solid(model: &SolidModel, shell_nodes: &str, surface: &str, tolerance: f64) -> Result<Vec<Constraint>> {
    let mesh = model.mesh();
    let nodes = mesh.node_set(shell_nodes).ok_or_else(|| FemError::UnknownSet {
        kind: SetKind::Node,
        name: shell_nodes.to_string(),
    })?;
    let set = mesh.surface_set_nodes(surface)?;
    let mut normal: BTreeMap<usize, Vec3> = BTreeMap::new();
    for e in 0..mesh.n_elements() {
        let kind = mesh.element_kind(e);
        if !matches!(
            kind,
            crate::mesh::ElementKind::Shell4 | crate::mesh::ElementKind::Shell3
        ) {
            continue;
        }
        let en = mesh.element_nodes(e);
        let x: Vec<Vec3> = en.iter().map(|&n| mesh.nodes()[n]).collect();
        let nv = if x.len() == 4 {
            cross(sub(x[2], x[0]), sub(x[3], x[1]))
        } else {
            cross(sub(x[1], x[0]), sub(x[2], x[0]))
        };
        let nv = unit(nv);
        for &n in en {
            let acc = normal.entry(n).or_insert([0.0; 3]);
            for i in 0..3 {
                acc[i] += nv[i];
            }
        }
    }
    let mut out = Vec::new();
    for &s in nodes {
        let nv = normal
            .get(&s)
            .ok_or_else(|| tie_error(format!("node {s} of '{shell_nodes}' is not a shell node")))?;
        let nv = unit(*nv);
        let xs = mesh.nodes()[s];
        let column: Vec<usize> = set
            .iter()
            .copied()
            .filter(|&i| {
                let r = sub(mesh.nodes()[i], xs);
                let a = dot(r, nv);
                norm([r[0] - a * nv[0], r[1] - a * nv[1], r[2] - a * nv[2]]) <= tolerance
            })
            .collect();
        if column.len() < 2 {
            return Err(tie_error(format!(
                "shell node {s}: {} node(s) of '{surface}' within {tolerance:e} of its normal line (at least two needed)",
                column.len()
            )));
        }
        out.extend(coupling(model, s, &column)?);
    }
    Ok(out)
}
