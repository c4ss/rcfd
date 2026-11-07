//! Local remeshing of a tetrahedral mesh during a run (F-247, ADR 0215).
//!
//! The mesh is bounded by axis-aligned planes (the sides of a box); a plane can move (a piston) and carries the points
//! that lie on it, and those of a band behind it in proportion ([`TetRemesher::move_plane_with_band`]). After each motion [`TetRemesher::needs_event`] checks the cells and edges; an event repairs what the
//! motion degraded with local operators (edge split, edge collapse, edge removal, 2-3 flip, point relocation). Every
//! operator is checked for validity (a positively oriented, conforming tiling of the same region, with its boundary faces
//! on the planes) and is followed by a conservative remap: the cells it removes give their totals to the cells it makes
//! in proportion to the intersection volumes. An event returns, for every cell after it, its sources in the cells before
//! it ([`RemeshChange`]), so [`remap`] carries cell totals (mass, momentum, energy, volume) over to round-off.
//!
//! shortcut: axis-aligned planar boundaries, one uniform target size, cell-average (first-order) remap; no face-flux map.

use crate::mesh::{Mesh, PatchKind};
use crate::predicates::orient3d;
use crate::tetmesh::TetMesh;
use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, unit, Vec3};
use rcfd_core::{Error, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Outward-oriented faces of a positively oriented tetrahedron, indexed by the opposite vertex.
const FACE: [[usize; 3]; 4] = [[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]];
/// The six edges of a tetrahedron.
const EDGES: [(usize, usize); 6] = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
const NONE: usize = usize::MAX;

fn merr<T>(m: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(m.into()))
}

/// A boundary plane `x[axis] = value` with the name of the patch of its faces.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisPlane {
    /// Patch name of the faces on the plane.
    pub name: String,
    /// Coordinate axis fixed by the plane (0, 1 or 2).
    pub axis: usize,
    /// Coordinate value.
    pub value: f64,
}

/// The six sides of the box `[lo, hi]`, in the order `xmin`, `xmax`, `ymin`, `ymax`, `zmin`, `zmax`.
pub fn box_planes(lo: Vec3, hi: Vec3) -> Vec<AxisPlane> {
    let names = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
    (0..6)
        .map(|k| AxisPlane {
            name: names[k].to_string(),
            axis: k / 2,
            value: if k % 2 == 0 { lo[k / 2] } else { hi[k / 2] },
        })
        .collect()
}

/// Thresholds of the trigger and of the event procedure (ADR 0215 M7 and S1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemeshSettings {
    /// Edges shorter than this trigger an event and are collapsed.
    pub l_min: f64,
    /// Edges longer than this trigger an event and are split.
    pub l_max: f64,
    /// A cell with a mean ratio below this triggers an event.
    pub q_trigger: f64,
    /// Cells with a mean ratio below this are improved during an event.
    pub q_good: f64,
    /// A collapse of a short edge keeps the smallest mean ratio of its cells at least `min(q_collapse, before)`.
    pub q_collapse: f64,
    /// A split of a long edge keeps the smallest mean ratio of its cells at least `min(q_split, before)`.
    pub q_split: f64,
    /// Largest number of passes of an event.
    pub max_passes: usize,
    /// Largest ring of an edge removal.
    pub max_ring: usize,
}

impl RemeshSettings {
    /// The registered values for the target edge length `h0` (ADR 0215 S1).
    pub fn for_size(h0: f64) -> Self {
        RemeshSettings {
            l_min: 0.5 * h0,
            l_max: 2.0 * h0,
            q_trigger: 0.3,
            q_good: 0.4,
            q_collapse: 0.3,
            q_split: 0.25,
            max_passes: 10,
            max_ring: 7,
        }
    }
}

/// Operators applied in an event.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OpCounts {
    /// Edge splits.
    pub splits: usize,
    /// Edge collapses.
    pub collapses: usize,
    /// Edge removals (3-2 flips and larger rings).
    pub removals: usize,
    /// 2-3 flips.
    pub flips: usize,
    /// Point relocations.
    pub relocations: usize,
}

impl OpCounts {
    /// All operators.
    pub fn total(&self) -> usize {
        self.splits + self.collapses + self.removals + self.flips + self.relocations
    }
}

/// What an event did (ADR 0215 M6).
#[derive(Debug, Clone)]
pub struct RemeshChange {
    /// `sources[new_cell] = [(old_cell, fraction)]`: the fraction of every old cell's total that the new cell receives;
    /// the fractions of every old cell sum to one.
    pub sources: Vec<Vec<(usize, f64)>>,
    /// Operators applied.
    pub ops: OpCounts,
    /// Cells before the event that are not in the mesh after it.
    pub removed: usize,
    /// Cells after the event that were not in the mesh before it.
    pub created: usize,
    /// Smallest corner of the bounding box of the points of every cell removed or made by an operator.
    pub lo: Vec3,
    /// Largest corner of that box.
    pub hi: Vec3,
    /// Largest raw mismatch of the intersection volumes against the cell volumes over the operators (relative).
    pub mismatch: f64,
}

fn signed_volume(p: &[Vec3; 4]) -> f64 {
    dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), sub(p[3], p[0])) / 6.0
}

/// Mean ratio of a tetrahedron, `12 (3 |V|)^(2/3) / sum_e l_e^2` with the sign of the signed volume `V`: 1 for the
/// regular tetrahedron, 0 for a degenerate one, negative for an inverted one (ADR 0215 M2).
pub fn mean_ratio(p: &[Vec3; 4]) -> f64 {
    let v = signed_volume(p);
    let mut s = 0.0;
    for (i, j) in EDGES {
        let d = sub(p[i], p[j]);
        s += dot(d, d);
    }
    if s <= 0.0 {
        return 0.0;
    }
    let q = 12.0 * (3.0 * v.abs()).powf(2.0 / 3.0) / s;
    if v < 0.0 {
        -q
    } else {
        q
    }
}

/// Clip a convex polyhedron (outward-oriented face loops) to the half-space `n . x <= d` (`n` a unit vector).
fn clip(faces: Vec<Vec<Vec3>>, n: Vec3, d: f64, tol: f64) -> Vec<Vec<Vec3>> {
    let s = |p: Vec3| dot(n, p) - d;
    let (mut any_out, mut any_in) = (false, false);
    for f in &faces {
        for &p in f {
            let x = s(p);
            if x > tol {
                any_out = true;
            } else if x < -tol {
                any_in = true;
            }
        }
    }
    if !any_out {
        return faces;
    }
    if !any_in {
        return Vec::new();
    }
    let mut kept = Vec::with_capacity(faces.len() + 1);
    let mut cap: Vec<Vec3> = Vec::new();
    for f in &faces {
        let m = f.len();
        let mut poly = Vec::with_capacity(m + 2);
        for i in 0..m {
            let (p, q) = (f[i], f[(i + 1) % m]);
            let (sp, sq) = (s(p), s(q));
            if sp <= tol {
                poly.push(p);
                if sp >= -tol {
                    cap.push(p);
                }
            }
            if (sp < -tol && sq > tol) || (sp > tol && sq < -tol) {
                let x = add(p, scale(sub(q, p), sp / (sp - sq)));
                poly.push(x);
                cap.push(x);
            }
        }
        if poly.len() >= 3 {
            kept.push(poly);
        }
    }
    let mut pts: Vec<Vec3> = Vec::new();
    for p in cap {
        if !pts.iter().any(|q| norm(sub(*q, p)) <= tol) {
            pts.push(p);
        }
    }
    if pts.len() >= 3 {
        let c = scale(pts.iter().fold([0.0; 3], |acc, p| add(acc, *p)), 1.0 / pts.len() as f64);
        let e = if n[0].abs() <= n[1].abs() && n[0].abs() <= n[2].abs() {
            [1.0, 0.0, 0.0]
        } else if n[1].abs() <= n[2].abs() {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        // (u, v, n) right-handed: increasing angle is counter-clockwise seen from +n, the outward side of the cap
        let u = unit(cross(n, e));
        let v = cross(n, u);
        let mut ang: Vec<(f64, Vec3)> = pts
            .iter()
            .map(|&p| {
                let r = sub(p, c);
                (dot(r, v).atan2(dot(r, u)), p)
            })
            .collect();
        ang.sort_by(|x, y| x.0.total_cmp(&y.0));
        kept.push(ang.into_iter().map(|x| x.1).collect());
    }
    kept
}

/// Volume of a closed polyhedron given by outward-oriented face loops (divergence theorem).
fn polyhedron_volume(faces: &[Vec<Vec3>]) -> f64 {
    let r = faces[0][0];
    let mut v = 0.0;
    for f in faces {
        for i in 1..f.len().saturating_sub(1) {
            v += dot(sub(f[0], r), cross(sub(f[i], r), sub(f[i + 1], r)));
        }
    }
    v / 6.0
}

/// Volume of the intersection of two positively oriented tetrahedra: `b` clipped by the four half-spaces of `a`
/// (ADR 0215 M5); zero for disjoint bounding boxes.
pub fn tet_intersection_volume(a: &[Vec3; 4], b: &[Vec3; 4]) -> f64 {
    let mut ext: f64 = 0.0;
    for d in 0..3 {
        let (mut alo, mut ahi, mut blo, mut bhi) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
        for k in 0..4 {
            alo = alo.min(a[k][d]);
            ahi = ahi.max(a[k][d]);
            blo = blo.min(b[k][d]);
            bhi = bhi.max(b[k][d]);
        }
        if ahi <= blo || bhi <= alo {
            return 0.0;
        }
        ext = ext.max(ahi - alo).max(bhi - blo);
    }
    let tol = 1e-13 * ext;
    let mut faces: Vec<Vec<Vec3>> = FACE.iter().map(|f| vec![b[f[0]], b[f[1]], b[f[2]]]).collect();
    for plane in FACE.iter() {
        let n = cross(sub(a[plane[1]], a[plane[0]]), sub(a[plane[2]], a[plane[0]]));
        let l = norm(n);
        if l <= 0.0 {
            return 0.0;
        }
        let n = scale(n, 1.0 / l);
        faces = clip(faces, n, dot(n, a[plane[0]]), tol);
        if faces.is_empty() {
            return 0.0;
        }
    }
    polyhedron_volume(&faces)
}

/// Carry cell densities `u` (per unit volume) through an event: new total `= sum fraction * old total`, divided by the
/// new cell volume. Cell totals are conserved to round-off.
pub fn remap<const N: usize>(change: &RemeshChange, u: &[[f64; N]], old_vol: &[f64], new_vol: &[f64]) -> Vec<[f64; N]> {
    change
        .sources
        .iter()
        .zip(new_vol)
        .map(|(src, v)| {
            let mut t = [0.0; N];
            for &(o, w) in src {
                for k in 0..N {
                    t[k] += w * old_vol[o] * u[o][k];
                }
            }
            t.map(|x| x / v)
        })
        .collect()
}

fn sorted3(a: [usize; 3]) -> [usize; 3] {
    let mut k = a;
    k.sort_unstable();
    k
}

/// Whether `q` beats the best so far (strictly larger; the first of equals is kept).
fn beats<T>(best: &Option<(f64, T)>, q: f64) -> bool {
    match best {
        None => true,
        Some((b, _)) => q > *b,
    }
}

/// The loop rotated so that its smallest index comes first (same orientation).
fn canonical(a: [usize; 3]) -> [usize; 3] {
    if a[0] <= a[1] && a[0] <= a[2] {
        a
    } else if a[1] <= a[2] {
        [a[1], a[2], a[0]]
    } else {
        [a[2], a[0], a[1]]
    }
}

/// A tetrahedral mesh bounded by axis-aligned planes, with local remeshing (ADR 0215).
#[derive(Debug, Clone)]
pub struct TetRemesher {
    points: Vec<Vec3>,
    on: Vec<u32>,
    cells: Vec<[usize; 4]>,
    planes: Vec<AxisPlane>,
    settings: RemeshSettings,
}

impl TetRemesher {
    /// A mesh of positively oriented cells (`orient3d > 0`) over `points`, bounded by `planes` (at most 32). A point lies
    /// on a plane when its coordinate equals the plane's value exactly; every boundary face must lie on a plane.
    pub fn new(
        points: Vec<Vec3>,
        cells: Vec<[usize; 4]>,
        planes: Vec<AxisPlane>,
        settings: RemeshSettings,
    ) -> Result<Self> {
        if planes.len() > 32 || planes.iter().any(|p| p.axis > 2) {
            return merr("remesh: at most 32 planes, each with an axis 0, 1 or 2");
        }
        for (i, c) in cells.iter().enumerate() {
            if c.iter().any(|&v| v >= points.len()) {
                return merr(format!("remesh: cell {i} refers to a point out of range"));
            }
            if orient3d(points[c[0]], points[c[1]], points[c[2]], points[c[3]]) <= 0 {
                return merr(format!("remesh: cell {i} is not positively oriented"));
            }
        }
        let on = points
            .iter()
            .map(|p| {
                planes
                    .iter()
                    .enumerate()
                    .filter(|(_, pl)| p[pl.axis] == pl.value)
                    .fold(0u32, |m, (k, _)| m | (1 << k))
            })
            .collect();
        let r = TetRemesher {
            points,
            on,
            cells,
            planes,
            settings,
        };
        r.mesh()?;
        Ok(r)
    }

    /// From the output of [`crate::tetmesh::tetrahedralize`].
    pub fn from_tet_mesh(tm: &TetMesh, planes: Vec<AxisPlane>, settings: RemeshSettings) -> Result<Self> {
        Self::new(tm.points.clone(), tm.cells.clone(), planes, settings)
    }

    /// Point coordinates.
    pub fn points(&self) -> &[Vec3] {
        &self.points
    }

    /// Cells as four point indices (positively oriented); cell `i` is cell `i` of [`Self::mesh`].
    pub fn cells(&self) -> &[[usize; 4]] {
        &self.cells
    }

    /// The boundary planes.
    pub fn planes(&self) -> &[AxisPlane] {
        &self.planes
    }

    /// The thresholds.
    pub fn settings(&self) -> &RemeshSettings {
        &self.settings
    }

    /// Index of the plane with the given patch name.
    pub fn plane_index(&self, name: &str) -> Option<usize> {
        self.planes.iter().position(|p| p.name == name)
    }

    /// Move plane `k` to `value`, with the points on it; the other points do not move.
    pub fn set_plane(&mut self, k: usize, value: f64) -> Result<()> {
        if k >= self.planes.len() || !value.is_finite() {
            return merr("remesh: set_plane needs a plane index and a finite value");
        }
        let axis = self.planes[k].axis;
        for (p, &m) in self.points.iter_mut().zip(&self.on) {
            if m & (1 << k) != 0 {
                p[axis] = value;
            }
        }
        self.planes[k].value = value;
        Ok(())
    }

    /// Move plane `k` to `value` with the points on it, and the points within `w` of it on its inner side in proportion
    /// (ADR 0215 amendment 2): a point at the distance `d < w` before the move goes along the plane's axis by
    /// `(value - old) (1 - d / w)`. Points on a plane of the same axis do not move. The map is monotone along the axis, so for
    /// a move smaller than `w` no point overtakes another and the motion alone inverts no cell.
    pub fn move_plane_with_band(&mut self, k: usize, value: f64, w: f64) -> Result<()> {
        if k >= self.planes.len()
            || !value.is_finite()
            || w.is_nan()
            || w <= 0.0
            || (value - self.planes[k].value).abs() >= w
        {
            return merr(
                "remesh: move_plane_with_band needs a plane index, a finite value and a band wider than the move",
            );
        }
        let axis = self.planes[k].axis;
        let old = self.planes[k].value;
        let mean = self.points.iter().map(|p| p[axis]).sum::<f64>() / self.points.len().max(1) as f64;
        let inward = if mean < old { -1.0 } else { 1.0 };
        let delta = value - old;
        let parallel = self
            .planes
            .iter()
            .enumerate()
            .filter(|(_, pl)| pl.axis == axis)
            .fold(0u32, |m, (j, _)| m | (1 << j));
        for (p, &m) in self.points.iter_mut().zip(&self.on) {
            if m & (1 << k) != 0 {
                p[axis] = value;
            } else if m & parallel == 0 {
                let d = (p[axis] - old) * inward;
                if d > 0.0 && d < w {
                    p[axis] += delta * (1.0 - d / w);
                }
            }
        }
        self.planes[k].value = value;
        Ok(())
    }

    /// The solver mesh: internal faces ordered by (owner, neighbour), boundary faces by patch in the plane order and then by
    /// (owner, local face); every patch a wall. The face order depends on the cells only, so the mesh of a moved state has
    /// the faces of the unmoved one in the same order.
    pub fn mesh(&self) -> Result<Mesh> {
        let mut map: HashMap<[usize; 3], (usize, usize, [usize; 3], usize)> =
            HashMap::with_capacity(4 * self.cells.len());
        for (ci, c) in self.cells.iter().enumerate() {
            for (li, f) in FACE.iter().enumerate() {
                let lp = [c[f[0]], c[f[1]], c[f[2]]];
                match map.get_mut(&sorted3(lp)) {
                    None => {
                        map.insert(sorted3(lp), (ci, li, lp, NONE));
                    }
                    Some(e) => {
                        if e.3 != NONE {
                            return merr("remesh: a face is shared by more than two cells");
                        }
                        e.3 = ci;
                    }
                }
            }
        }
        let mut internal: Vec<(usize, usize, [usize; 3])> = Vec::new();
        let mut bnd: Vec<Vec<(usize, usize, [usize; 3])>> = vec![Vec::new(); self.planes.len()];
        for e in map.values() {
            if e.3 != NONE {
                internal.push((e.0, e.3, e.2));
            } else {
                let m = self.on[e.2[0]] & self.on[e.2[1]] & self.on[e.2[2]];
                if m == 0 {
                    return merr(format!("remesh: the boundary face {:?} lies on no plane", e.2));
                }
                bnd[m.trailing_zeros() as usize].push((e.0, e.1, e.2));
            }
        }
        internal.sort_unstable();
        let mut faces = Vec::with_capacity(map.len());
        let mut owner = Vec::with_capacity(map.len());
        let mut neighbour = Vec::with_capacity(internal.len());
        for (o, n, l) in internal {
            faces.push(l.to_vec());
            owner.push(o);
            neighbour.push(n);
        }
        let mut patches = Vec::with_capacity(self.planes.len());
        for (k, b) in bnd.iter_mut().enumerate() {
            b.sort_unstable();
            for &(o, _, l) in b.iter() {
                faces.push(l.to_vec());
                owner.push(o);
            }
            patches.push((self.planes[k].name.clone(), PatchKind::Wall, b.len()));
        }
        Mesh::from_face_lists(self.points.clone(), faces, owner, neighbour, patches)
    }

    fn cell_points(&self, c: &[usize; 4]) -> [Vec3; 4] {
        [
            self.points[c[0]],
            self.points[c[1]],
            self.points[c[2]],
            self.points[c[3]],
        ]
    }

    /// Mean ratio of every cell.
    pub fn quality(&self) -> Vec<f64> {
        self.cells.iter().map(|c| mean_ratio(&self.cell_points(c))).collect()
    }

    /// Smallest mean ratio over the cells.
    pub fn min_quality(&self) -> f64 {
        self.quality().into_iter().fold(f64::INFINITY, f64::min)
    }

    /// Shortest and longest edge.
    pub fn edge_range(&self) -> (f64, f64) {
        let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
        for c in &self.cells {
            let p = self.cell_points(c);
            for (i, j) in EDGES {
                let l = norm(sub(p[i], p[j]));
                lo = lo.min(l);
                hi = hi.max(l);
            }
        }
        (lo, hi)
    }

    /// The trigger of ADR 0215 M7: a cell with a mean ratio below `q_trigger`, or an edge shorter than `l_min` or longer
    /// than `l_max`.
    pub fn needs_event(&self) -> bool {
        for c in &self.cells {
            let p = self.cell_points(c);
            if mean_ratio(&p) < self.settings.q_trigger {
                return true;
            }
            for (i, j) in EDGES {
                let l = norm(sub(p[i], p[j]));
                if l < self.settings.l_min || l > self.settings.l_max {
                    return true;
                }
            }
        }
        false
    }

    /// The event when the trigger fires ([`Self::needs_event`], then [`Self::event`]); `None` when it does not fire or
    /// the event changes nothing.
    pub fn remesh(&mut self) -> Result<Option<RemeshChange>> {
        if !self.needs_event() {
            return Ok(None);
        }
        self.event()
    }

    /// The event procedure of ADR 0215 M7, whether or not the trigger fires; `None` when no operator was applied (the mesh
    /// is then unchanged).
    pub fn event(&mut self) -> Result<Option<RemeshChange>> {
        let mut w = Work::new(self);
        w.run()?;
        Ok(self.finish(w))
    }

    fn finish(&mut self, w: Work) -> Option<RemeshChange> {
        if w.ops.total() == 0 {
            return None;
        }
        let keep: Vec<usize> = (0..w.tets.len()).filter(|&t| w.alive[t]).collect();
        let mut map = vec![NONE; w.pts.len()];
        for &t in &keep {
            for &v in &w.tets[t] {
                map[v] = 0;
            }
        }
        let (mut points, mut on) = (Vec::new(), Vec::new());
        for v in 0..w.pts.len() {
            if map[v] != NONE {
                map[v] = points.len();
                points.push(w.pts[v]);
                on.push(w.on[v]);
            }
        }
        let cells = keep.iter().map(|&t| w.tets[t].map(|v| map[v])).collect();
        let sources = keep.iter().map(|&t| w.src[t].clone()).collect();
        let removed = (0..w.n0).filter(|&t| !w.alive[t]).count();
        let created = keep.iter().filter(|&&t| t >= w.n0).count();
        self.points = points;
        self.on = on;
        self.cells = cells;
        Some(RemeshChange {
            sources,
            ops: w.ops,
            removed,
            created,
            lo: w.lo,
            hi: w.hi,
            mismatch: w.mismatch,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Split,
    Collapse,
    Removal,
    Flip,
    Relocation,
}

/// A candidate operator: the cells `old` are replaced by `new` (point `NEW`, the next index, is the added point).
#[derive(Debug, Clone)]
struct Cand {
    kind: Kind,
    old: Vec<usize>,
    new: Vec<[usize; 4]>,
    moved: Option<(usize, Vec3)>,
    added: Option<(Vec3, u32)>,
}

struct Eval {
    min_new: f64,
    min_old: f64,
    lo_new: f64,
    hi_new: f64,
    lo_old: f64,
    hi_old: f64,
}

/// The mutable mesh of an event: dead cells stay in place until the end; every cell carries its sources.
struct Work {
    pts: Vec<Vec3>,
    on: Vec<u32>,
    tets: Vec<[usize; 4]>,
    alive: Vec<bool>,
    src: Vec<Vec<(usize, f64)>>,
    vt: Vec<Vec<usize>>,
    planes: Vec<AxisPlane>,
    s: RemeshSettings,
    ops: OpCounts,
    lo: Vec3,
    hi: Vec3,
    mismatch: f64,
    n0: usize,
}

impl Work {
    fn new(r: &TetRemesher) -> Work {
        let mut vt = vec![Vec::new(); r.points.len()];
        for (t, c) in r.cells.iter().enumerate() {
            for &v in c {
                vt[v].push(t);
            }
        }
        Work {
            pts: r.points.clone(),
            on: r.on.clone(),
            tets: r.cells.clone(),
            alive: vec![true; r.cells.len()],
            src: (0..r.cells.len()).map(|t| vec![(t, 1.0)]).collect(),
            vt,
            planes: r.planes.clone(),
            s: r.settings,
            ops: OpCounts::default(),
            lo: [f64::INFINITY; 3],
            hi: [f64::NEG_INFINITY; 3],
            mismatch: 0.0,
            n0: r.cells.len(),
        }
    }

    fn len(&self, a: usize, b: usize) -> f64 {
        norm(sub(self.pts[a], self.pts[b]))
    }

    fn old_points(&self, t: usize) -> [Vec3; 4] {
        self.tets[t].map(|v| self.pts[v])
    }

    fn pos(&self, c: &Cand, v: usize) -> Vec3 {
        if let Some((p, _)) = c.added {
            if v == self.pts.len() {
                return p;
            }
        }
        if let Some((m, p)) = c.moved {
            if v == m {
                return p;
            }
        }
        self.pts[v]
    }

    fn mask(&self, c: &Cand, v: usize) -> u32 {
        if let Some((_, m)) = c.added {
            if v == self.pts.len() {
                return m;
            }
        }
        self.on[v]
    }

    fn new_points(&self, c: &Cand, t: &[usize; 4]) -> [Vec3; 4] {
        t.map(|v| self.pos(c, v))
    }

    /// Alive cells holding both points.
    fn ring(&self, a: usize, b: usize) -> Vec<usize> {
        self.vt[a]
            .iter()
            .copied()
            .filter(|&t| self.tets[t].contains(&b))
            .collect()
    }

    /// Points joined to `v` by an edge, in index order.
    fn neighbours(&self, v: usize) -> Vec<usize> {
        let mut s = BTreeSet::new();
        for &t in &self.vt[v] {
            for &u in &self.tets[t] {
                if u != v {
                    s.insert(u);
                }
            }
        }
        s.into_iter().collect()
    }

    /// Whether an alive cell outside `cav` has the face with these three points.
    fn shared_outside(&self, key: [usize; 3], cav: &BTreeSet<usize>) -> bool {
        if key.iter().any(|&v| v >= self.pts.len()) {
            return false;
        }
        self.vt[key[0]]
            .iter()
            .any(|&t| !cav.contains(&t) && self.tets[t].contains(&key[1]) && self.tets[t].contains(&key[2]))
    }

    /// Edges of the alive cells whose length passes `keep`, longest first when `desc`, else shortest first.
    fn edges_where(&self, keep: impl Fn(f64) -> bool, desc: bool) -> Vec<(usize, usize)> {
        let mut set = BTreeSet::new();
        for (t, c) in self.tets.iter().enumerate() {
            if !self.alive[t] {
                continue;
            }
            for (i, j) in EDGES {
                set.insert((c[i].min(c[j]), c[i].max(c[j])));
            }
        }
        let mut v: Vec<(f64, usize, usize)> = set
            .into_iter()
            .map(|(a, b)| (self.len(a, b), a, b))
            .filter(|e| keep(e.0))
            .collect();
        v.sort_by(|x, y| {
            let o = if desc { y.0.total_cmp(&x.0) } else { x.0.total_cmp(&y.0) };
            o.then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2))
        });
        v.into_iter().map(|e| (e.1, e.2)).collect()
    }

    fn cand_split(&self, a: usize, b: usize) -> Option<Cand> {
        let ring = self.ring(a, b);
        if ring.is_empty() {
            return None;
        }
        let mask = self.on[a] & self.on[b];
        let mut m = scale(add(self.pts[a], self.pts[b]), 0.5);
        for (k, pl) in self.planes.iter().enumerate() {
            if mask & (1 << k) != 0 {
                m[pl.axis] = pl.value;
            }
        }
        let id = self.pts.len();
        let mut new = Vec::with_capacity(2 * ring.len());
        for &t in &ring {
            let c = self.tets[t];
            new.push(c.map(|v| if v == a { id } else { v }));
            new.push(c.map(|v| if v == b { id } else { v }));
        }
        Some(Cand {
            kind: Kind::Split,
            old: ring,
            new,
            moved: None,
            added: Some((m, mask)),
        })
    }

    /// Collapse of `b` onto `a`.
    fn cand_collapse(&self, b: usize, a: usize) -> Option<Cand> {
        if a == b || self.on[b] & !self.on[a] != 0 {
            return None;
        }
        let star = self.vt[b].clone();
        if !star.iter().any(|&t| self.tets[t].contains(&a)) {
            return None;
        }
        let new: Vec<[usize; 4]> = star
            .iter()
            .filter(|&&t| !self.tets[t].contains(&a))
            .map(|&t| self.tets[t].map(|v| if v == b { a } else { v }))
            .collect();
        if new.is_empty() {
            return None;
        }
        Some(Cand {
            kind: Kind::Collapse,
            old: star,
            new,
            moved: None,
            added: None,
        })
    }

    /// Removal of the edge `(a, b)`, interior (a closed ring) or on exactly one plane (an open ring whose end points lie on
    /// it, closed by their segment; two cells give the 2-2 flip): the fan of its ring with the largest smallest mean ratio
    /// (ADR 0215 M3, amendment 2).
    fn cand_removal(&self, a: usize, b: usize) -> Option<Cand> {
        // an interior edge has a closed ring; an edge on exactly one plane has an open ring whose ends lie on that plane
        let common = self.on[a] & self.on[b];
        let open = common != 0;
        if common.count_ones() > 1 {
            return None;
        }
        let ring = self.ring(a, b);
        let n = ring.len();
        if n < if open { 2 } else { 3 } || n > self.s.max_ring {
            return None;
        }
        let mut next: BTreeMap<usize, usize> = BTreeMap::new();
        for &t in &ring {
            let o: Vec<usize> = self.tets[t].iter().copied().filter(|&v| v != a && v != b).collect();
            let (x, y) = (o[0], o[1]);
            let (from, to) = if orient3d(self.pts[a], self.pts[b], self.pts[x], self.pts[y]) > 0 {
                (x, y)
            } else {
                (y, x)
            };
            if next.insert(from, to).is_some() {
                return None;
            }
        }
        let start = if open {
            let targets: BTreeSet<usize> = next.values().copied().collect();
            let mut free = next.keys().copied().filter(|k| !targets.contains(k));
            let s0 = free.next()?;
            if free.next().is_some() {
                return None;
            }
            s0
        } else {
            *next.keys().next()?
        };
        let mut r = vec![start];
        let mut cur = start;
        for _ in 1..if open { n + 1 } else { n } {
            cur = *next.get(&cur)?;
            if r.contains(&cur) {
                return None;
            }
            r.push(cur);
        }
        if open {
            if next.contains_key(&cur) {
                return None;
            }
        } else if next.get(&cur) != Some(&start) {
            return None;
        }
        let n = r.len();
        let mut best: Option<(f64, Vec<[usize; 4]>)> = None;
        for k in 0..n {
            let mut tets = Vec::with_capacity(2 * (n - 2));
            let mut mn = f64::INFINITY;
            let mut ok = true;
            for i in 1..n - 1 {
                let (x, y, z) = (r[k], r[(k + i) % n], r[(k + i + 1) % n]);
                for t in [[a, x, y, z], [b, x, z, y]] {
                    let p = t.map(|v| self.pts[v]);
                    if orient3d(p[0], p[1], p[2], p[3]) <= 0 {
                        ok = false;
                        break;
                    }
                    mn = mn.min(mean_ratio(&p));
                    tets.push(t);
                }
                if !ok {
                    break;
                }
            }
            if ok && beats(&best, mn) {
                best = Some((mn, tets));
            }
        }
        best.map(|(_, new)| Cand {
            kind: Kind::Removal,
            old: ring,
            new,
            moved: None,
            added: None,
        })
    }

    /// 2-3 flip of face `i` of cell `t`.
    fn cand_flip(&self, t: usize, i: usize) -> Option<Cand> {
        let c = self.tets[t];
        let f = FACE[i];
        let (p, q, r, d) = (c[f[0]], c[f[1]], c[f[2]], c[i]);
        let t2 = self.vt[p]
            .iter()
            .copied()
            .find(|&u| u != t && self.tets[u].contains(&q) && self.tets[u].contains(&r))?;
        let e = self.tets[t2].iter().copied().find(|&v| v != p && v != q && v != r)?;
        Some(Cand {
            kind: Kind::Flip,
            old: vec![t, t2],
            new: vec![[p, q, d, e], [q, r, d, e], [r, p, d, e]],
            moved: None,
            added: None,
        })
    }

    /// Relocation of `v` a fraction `lambda` of the way to the mean of its neighbours, in its free coordinates.
    fn cand_relocate(&self, v: usize, lambda: f64) -> Option<Cand> {
        let mut fixed = [false; 3];
        for (k, pl) in self.planes.iter().enumerate() {
            if self.on[v] & (1 << k) != 0 {
                fixed[pl.axis] = true;
            }
        }
        if fixed.iter().all(|&x| x) {
            return None;
        }
        let nb = self.neighbours(v);
        if nb.is_empty() {
            return None;
        }
        let mean = scale(
            nb.iter().fold([0.0; 3], |acc, &u| add(acc, self.pts[u])),
            1.0 / nb.len() as f64,
        );
        let mut p = self.pts[v];
        for d in 0..3 {
            if !fixed[d] {
                p[d] += lambda * (mean[d] - p[d]);
            }
        }
        if p == self.pts[v] {
            return None;
        }
        let star = self.vt[v].clone();
        let new = star.iter().map(|&t| self.tets[t]).collect();
        Some(Cand {
            kind: Kind::Relocation,
            old: star,
            new,
            moved: Some((v, p)),
            added: None,
        })
    }

    /// Validity (ADR 0215 M4) and the quality and size figures of a candidate; `None` when it is not valid.
    fn evaluate(&self, c: &Cand) -> Option<Eval> {
        if c.new.is_empty() || c.old.is_empty() {
            return None;
        }
        // V1 and the new quality
        let (mut min_new, mut vol_new) = (f64::INFINITY, 0.0);
        for t in &c.new {
            let p = self.new_points(c, t);
            if orient3d(p[0], p[1], p[2], p[3]) <= 0 {
                return None;
            }
            min_new = min_new.min(mean_ratio(&p));
            vol_new += signed_volume(&p);
        }
        let (mut min_old, mut vol_old) = (f64::INFINITY, 0.0);
        for &t in &c.old {
            let p = self.old_points(t);
            min_old = min_old.min(mean_ratio(&p));
            vol_old += signed_volume(&p);
        }
        // V4
        if (vol_new - vol_old).abs() > 1e-12 * vol_old.abs() {
            return None;
        }
        // V2 and V3
        let cav: BTreeSet<usize> = c.old.iter().copied().collect();
        let mut old_faces: BTreeMap<[usize; 3], ([usize; 3], usize)> = BTreeMap::new();
        for &t in &c.old {
            let x = self.tets[t];
            for f in FACE {
                let lp = [x[f[0]], x[f[1]], x[f[2]]];
                old_faces.entry(sorted3(lp)).or_insert((canonical(lp), 0)).1 += 1;
            }
        }
        let mut interface: BTreeMap<[usize; 3], ([usize; 3], bool)> = BTreeMap::new();
        for (k, (o, n)) in &old_faces {
            if *n == 1 && self.shared_outside(*k, &cav) {
                interface.insert(*k, (*o, false));
            }
        }
        let mut new_faces: BTreeMap<[usize; 3], ([usize; 3], usize)> = BTreeMap::new();
        for x in &c.new {
            for f in FACE {
                let lp = [x[f[0]], x[f[1]], x[f[2]]];
                new_faces.entry(sorted3(lp)).or_insert((canonical(lp), 0)).1 += 1;
            }
        }
        for (k, (o, n)) in &new_faces {
            match *n {
                1 => match interface.get_mut(k) {
                    Some(e) => {
                        if e.0 != *o {
                            return None;
                        }
                        e.1 = true;
                    }
                    None => {
                        if self.shared_outside(*k, &cav) {
                            return None;
                        }
                        if self.mask(c, k[0]) & self.mask(c, k[1]) & self.mask(c, k[2]) == 0 {
                            return None;
                        }
                    }
                },
                2 => {}
                _ => return None,
            }
        }
        if interface.values().any(|e| !e.1) {
            return None;
        }
        // the edge-length range of the cavity before and after
        let range = |pts: &mut dyn Iterator<Item = [Vec3; 4]>| {
            let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
            for p in pts {
                for (i, j) in EDGES {
                    let l = norm(sub(p[i], p[j]));
                    lo = lo.min(l);
                    hi = hi.max(l);
                }
            }
            (lo, hi)
        };
        let (lo_old, hi_old) = range(&mut c.old.iter().map(|&t| self.old_points(t)));
        let (lo_new, hi_new) = range(&mut c.new.iter().map(|t| self.new_points(c, t)));
        Some(Eval {
            min_new,
            min_old,
            lo_new,
            hi_new,
            lo_old,
            hi_old,
        })
    }

    /// The size rule of ADR 0215 M7: no edge shorter than `min(l_min, shortest before)` and none longer than
    /// `max(cap, longest before)`.
    fn sizes_ok(&self, e: &Eval, cap: f64) -> bool {
        e.lo_new >= e.lo_old.min(self.s.l_min) && e.hi_new <= e.hi_old.max(cap)
    }

    /// Apply a candidate with its conservative remap (ADR 0215 M5, M6).
    fn apply(&mut self, c: Cand) -> Result<()> {
        let old_p: Vec<[Vec3; 4]> = c.old.iter().map(|&t| self.old_points(t)).collect();
        let new_p: Vec<[Vec3; 4]> = c.new.iter().map(|t| self.new_points(&c, t)).collect();
        let mut x = vec![vec![0.0; new_p.len()]; old_p.len()];
        for (i, a) in old_p.iter().enumerate() {
            for (j, b) in new_p.iter().enumerate() {
                x[i][j] = tet_intersection_volume(a, b);
            }
        }
        let rs: Vec<f64> = x.iter().map(|row| row.iter().sum()).collect();
        let cs: Vec<f64> = (0..new_p.len()).map(|j| x.iter().map(|row| row[j]).sum()).collect();
        let mut mm: f64 = 0.0;
        for (i, p) in old_p.iter().enumerate() {
            let v = signed_volume(p);
            mm = mm.max((rs[i] - v).abs() / v);
        }
        for (j, p) in new_p.iter().enumerate() {
            let v = signed_volume(p);
            mm = mm.max((cs[j] - v).abs() / v);
        }
        if mm.is_nan() || mm > 1e-9 {
            return merr(format!(
                "remesh: intersection volumes do not match the cell volumes (relative {mm:e}, {:?})",
                c.kind
            ));
        }
        self.mismatch = self.mismatch.max(mm);
        let mut srcs: Vec<BTreeMap<usize, f64>> = vec![BTreeMap::new(); new_p.len()];
        for i in 0..old_p.len() {
            for j in 0..new_p.len() {
                if x[i][j] > 0.0 {
                    let f = x[i][j] / rs[i];
                    for &(o, w) in &self.src[c.old[i]] {
                        *srcs[j].entry(o).or_insert(0.0) += f * w;
                    }
                }
            }
        }
        for p in old_p.iter().chain(new_p.iter()) {
            for q in p {
                for d in 0..3 {
                    self.lo[d] = self.lo[d].min(q[d]);
                    self.hi[d] = self.hi[d].max(q[d]);
                }
            }
        }
        if let Some((p, m)) = c.added {
            self.pts.push(p);
            self.on.push(m);
            self.vt.push(Vec::new());
        }
        if let Some((v, p)) = c.moved {
            self.pts[v] = p;
        }
        for &t in &c.old {
            self.alive[t] = false;
            for v in self.tets[t] {
                self.vt[v].retain(|&u| u != t);
            }
        }
        for (j, t) in c.new.iter().enumerate() {
            let idx = self.tets.len();
            self.tets.push(*t);
            self.alive.push(true);
            self.src.push(srcs[j].iter().map(|(&o, &w)| (o, w)).collect());
            for &v in t {
                self.vt[v].push(idx);
            }
        }
        match c.kind {
            Kind::Split => self.ops.splits += 1,
            Kind::Collapse => self.ops.collapses += 1,
            Kind::Removal => self.ops.removals += 1,
            Kind::Flip => self.ops.flips += 1,
            Kind::Relocation => self.ops.relocations += 1,
        }
        Ok(())
    }

    /// The improvement candidates of cell `t` in the registered order (ADR 0215 M7 step 3).
    fn improvement_candidates(&self, t: usize) -> Vec<Cand> {
        let c = self.tets[t];
        let mut out = Vec::new();
        for &v in &c {
            for lambda in [1.0, 0.5] {
                out.extend(self.cand_relocate(v, lambda));
            }
        }
        for (i, j) in EDGES {
            out.extend(self.cand_removal(c[i], c[j]));
        }
        for i in 0..4 {
            out.extend(self.cand_flip(t, i));
        }
        for &v in &c {
            for u in self.neighbours(v) {
                out.extend(self.cand_collapse(v, u));
            }
        }
        out
    }

    /// The passes of an event (ADR 0215 M7).
    fn run(&mut self) -> Result<()> {
        for _ in 0..self.s.max_passes {
            let mut applied = 0;
            // 1. splits of the long edges, longest first
            let l_max = self.s.l_max;
            for (a, b) in self.edges_where(|l| l > l_max, true) {
                if self.ring(a, b).is_empty() || self.len(a, b) <= l_max {
                    continue;
                }
                if let Some(c) = self.cand_split(a, b) {
                    if let Some(e) = self.evaluate(&c) {
                        if self.sizes_ok(&e, l_max) && e.min_new >= self.s.q_split.min(e.min_old) {
                            self.apply(c)?;
                            applied += 1;
                        }
                    }
                }
            }
            // 2. collapses of the short edges, shortest first: either end point onto any neighbour (amendment 2), edges up
            // to twice l_max (step 1 splits them in the next pass)
            let l_min = self.s.l_min;
            for (a, b) in self.edges_where(|l| l < l_min, false) {
                if self.ring(a, b).is_empty() || self.len(a, b) >= l_min {
                    continue;
                }
                let mut cands = Vec::new();
                for from in [b, a] {
                    for to in self.neighbours(from) {
                        cands.extend(self.cand_collapse(from, to));
                    }
                }
                let mut best: Option<(f64, Cand)> = None;
                for c in cands {
                    if let Some(e) = self.evaluate(&c) {
                        if self.sizes_ok(&e, 2.0 * l_max)
                            && e.min_new >= self.s.q_collapse.min(e.min_old)
                            && beats(&best, e.min_new)
                        {
                            best = Some((e.min_new, c));
                        }
                    }
                }
                if let Some((_, c)) = best {
                    self.apply(c)?;
                    applied += 1;
                }
            }
            // 3. improvement of the poor cells, worst first
            let mut bad: Vec<(f64, usize)> = Vec::new();
            for t in 0..self.tets.len() {
                if self.alive[t] {
                    let q = mean_ratio(&self.old_points(t));
                    if q < self.s.q_good {
                        bad.push((q, t));
                    }
                }
            }
            bad.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            for (_, t) in bad {
                if !self.alive[t] {
                    continue;
                }
                let mut best: Option<(f64, Cand)> = None;
                for c in self.improvement_candidates(t) {
                    if let Some(e) = self.evaluate(&c) {
                        // an improvement collapse makes no edge longer than l_max (amendment 2)
                        let long = c.kind == Kind::Collapse && e.hi_new > l_max;
                        if self.sizes_ok(&e, l_max) && !long && e.min_new > e.min_old + 1e-6 && beats(&best, e.min_new)
                        {
                            best = Some((e.min_new, c));
                        }
                    }
                }
                if let Some((_, c)) = best {
                    self.apply(c)?;
                    applied += 1;
                }
            }
            if applied == 0 {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::box_surface;
    use crate::tetmesh::{tetrahedralize, TetOptions};

    fn opts() -> TetOptions {
        TetOptions {
            size: Some(0.035),
            quality_sweeps: 3,
            ..TetOptions::default()
        }
    }

    /// U1: intersection volumes of two splits of the unit cube, a cell with itself, disjoint and face-neighbour cells.
    #[test]
    fn u1_intersection_volumes_of_two_cube_splits() {
        let c = |i: usize| -> Vec3 { [(i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64] };
        let pos = |t: [usize; 4]| -> [Vec3; 4] {
            let mut p = t.map(c);
            if signed_volume(&p) < 0.0 {
                p.swap(2, 3);
            }
            p
        };
        // six cells around the main diagonal 0-7, through the paths of the axes
        let mut six = Vec::new();
        for perm in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            let mut v = 0usize;
            let mut t = [0usize; 4];
            for (k, &ax) in perm.iter().enumerate() {
                v |= 1 << ax;
                t[k + 1] = v;
            }
            six.push(pos(t));
        }
        // five cells: the centre cell (0, 3, 5, 6) and the four corners
        let five = [
            pos([0, 3, 5, 6]),
            pos([1, 0, 3, 5]),
            pos([2, 0, 3, 6]),
            pos([4, 0, 5, 6]),
            pos([7, 3, 5, 6]),
        ];
        let tot6: f64 = six.iter().map(signed_volume).sum();
        let tot5: f64 = five.iter().map(signed_volume).sum();
        assert!((tot6 - 1.0).abs() < 1e-15 && (tot5 - 1.0).abs() < 1e-15);
        let x: Vec<Vec<f64>> = six
            .iter()
            .map(|a| five.iter().map(|b| tet_intersection_volume(a, b)).collect())
            .collect();
        let mut worst: f64 = 0.0;
        for (i, a) in six.iter().enumerate() {
            let s: f64 = x[i].iter().sum();
            worst = worst.max((s - signed_volume(a)).abs() / signed_volume(a));
        }
        for (j, b) in five.iter().enumerate() {
            let s: f64 = x.iter().map(|r| r[j]).sum();
            worst = worst.max((s - signed_volume(b)).abs() / signed_volume(b));
        }
        println!("U1 row and column sums: largest relative error {worst:.3e}");
        assert!(worst <= 1e-13, "U1 sums {worst:e}");
        let mut self_err: f64 = 0.0;
        for a in six.iter().chain(five.iter()) {
            self_err = self_err.max((tet_intersection_volume(a, a) - signed_volume(a)).abs() / signed_volume(a));
        }
        println!("U1 a cell with itself: largest relative error {self_err:.3e}");
        assert!(self_err <= 1e-13, "U1 self {self_err:e}");
        let far = six[0].map(|p| add(p, [3.0, 0.0, 0.0]));
        assert_eq!(tet_intersection_volume(&six[0], &far), 0.0);
        // face neighbours: cells 0 and 1 of the six share the face (0, v1, 7) only if their paths share a step
        let mut touching: f64 = 0.0;
        for i in 0..6 {
            for j in 0..6 {
                if i != j {
                    touching = touching.max(tet_intersection_volume(&six[i], &six[j]));
                }
            }
        }
        println!("U1 two different cells of the same split: largest intersection {touching:.3e}");
        assert!(touching <= 1e-15, "U1 neighbours {touching:e}");
    }

    struct Probe {
        vol: f64,
        lin: f64,
    }

    fn probe(m: &Mesh) -> Probe {
        let v = m.cell_volumes();
        let c = m.cell_centres();
        Probe {
            vol: v.iter().sum(),
            lin: (0..m.n_cells())
                .map(|i| (1.0 + c[i][0] + 2.0 * c[i][1] + 3.0 * c[i][2]) * v[i])
                .sum(),
        }
    }

    /// The checks (a) to (e) of U2 on one change.
    fn check_change(before: &Mesh, after_r: &TetRemesher, ch: &RemeshChange, what: &str) -> f64 {
        let after = after_r.mesh().expect("U2 (e): a valid mesh");
        let names: Vec<&str> = after.patches().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"], "U2 (e) {what}");
        let mut worst: f64 = 0.0;
        // (a)
        let mut sums = vec![0.0; before.n_cells()];
        for src in &ch.sources {
            for &(o, w) in src {
                sums[o] += w;
            }
        }
        for s in &sums {
            worst = worst.max((s - 1.0).abs());
        }
        // (b)
        let (pb, pa) = (probe(before), probe(&after));
        worst = worst.max((pa.vol - pb.vol).abs() / pb.vol);
        // (c)
        let u: Vec<[f64; 3]> = vec![[1.0, 2.0, 3.0]; before.n_cells()];
        let un = remap(ch, &u, before.cell_volumes(), after.cell_volumes());
        for x in &un {
            for k in 0..3 {
                worst = worst.max((x[k] - u[0][k]).abs() / u[0][k]);
            }
        }
        // (d)
        let c = before.cell_centres();
        let f: Vec<[f64; 1]> = (0..before.n_cells())
            .map(|i| [1.0 + c[i][0] + 2.0 * c[i][1] + 3.0 * c[i][2]])
            .collect();
        let fnew = remap(ch, &f, before.cell_volumes(), after.cell_volumes());
        let tot: f64 = fnew.iter().zip(after.cell_volumes()).map(|(a, v)| a[0] * v).sum();
        worst = worst.max((tot - pb.lin).abs() / pb.lin);
        println!(
            "U2 {what}: ops {:?}, cells {} -> {}, (a)-(d) largest {worst:.3e}, mismatch {:.3e}",
            ch.ops,
            before.n_cells(),
            after.n_cells(),
            ch.mismatch
        );
        assert!(worst <= 1e-13, "U2 {what}: {worst:e}");
        worst
    }

    impl TetRemesher {
        /// One operator as an event: the first candidate `pick` returns, applied with its remap.
        fn apply_one(&mut self, pick: impl Fn(&Work) -> Option<Cand>) -> Result<Option<RemeshChange>> {
            let mut w = Work::new(self);
            match pick(&w) {
                Some(c) => w.apply(c)?,
                None => return Ok(None),
            }
            Ok(self.finish(w))
        }
    }

    fn first_valid(w: &Work, kind: Kind) -> Option<Cand> {
        for t in 0..w.tets.len() {
            let c = w.tets[t];
            let cands: Vec<Cand> = match kind {
                Kind::Split => EDGES.iter().filter_map(|&(i, j)| w.cand_split(c[i], c[j])).collect(),
                Kind::Collapse => c
                    .iter()
                    .flat_map(|&v| w.neighbours(v).into_iter().filter_map(move |u| w.cand_collapse(v, u)))
                    .collect(),
                Kind::Removal => EDGES.iter().filter_map(|&(i, j)| w.cand_removal(c[i], c[j])).collect(),
                Kind::Flip => (0..4).filter_map(|i| w.cand_flip(t, i)).collect(),
                Kind::Relocation => c.iter().filter_map(|&v| w.cand_relocate(v, 0.5)).collect(),
            };
            for cand in cands {
                if w.evaluate(&cand).is_some() {
                    return Some(cand);
                }
            }
        }
        None
    }

    /// U2: each operator once, then the events of a gradual piston motion (amendment 1), on M_s.
    #[test]
    fn u2_each_operator_and_the_events_of_a_piston_carry_totals_on_m_s() {
        let s = box_surface([0.0; 3], [0.3, 0.1, 0.1], [6, 2, 2]);
        let tm = tetrahedralize(&s, &opts()).unwrap();
        let planes = box_planes([0.0; 3], [0.3, 0.1, 0.1]);
        let r0 = TetRemesher::from_tet_mesh(&tm, planes, RemeshSettings::for_size(0.035)).unwrap();
        for kind in [Kind::Split, Kind::Collapse, Kind::Removal, Kind::Flip, Kind::Relocation] {
            let mut r = r0.clone();
            let before = r.mesh().unwrap();
            let ch = r
                .apply_one(|w| first_valid(w, kind))
                .unwrap()
                .unwrap_or_else(|| panic!("U2 set-up: no valid {kind:?} on M_s"));
            check_change(&before, &r, &ch, &format!("{kind:?}"));
        }
        let mut r = r0.clone();
        let xmax = r.plane_index("xmax").unwrap();
        let (mut inward, mut outward) = (0, 0);
        let mut x = 0.3;
        for (target, step) in [(0.27, -0.001), (0.33, 0.001)] {
            while (target - x) * step > 1e-12 {
                x = ((x + step) * 1000.0f64).round() / 1000.0;
                r.set_plane(xmax, x).unwrap();
                let before = r.mesh().expect("U2: the moved mesh is valid");
                if let Some(ch) = r.remesh().unwrap() {
                    check_change(&before, &r, &ch, &format!("event at x = {x:.3}"));
                    if step < 0.0 {
                        inward += 1;
                    } else {
                        outward += 1;
                    }
                }
            }
        }
        println!(
            "U2 events: {inward} inward, {outward} outward; final min mean ratio {:.4}",
            r.min_quality()
        );
        assert!(
            inward >= 1 && outward >= 1,
            "U2 set-up: events {inward} inward, {outward} outward"
        );
    }

    /// U3: M0 through the remesher's mesh.
    #[test]
    fn u3_m0_through_the_remesher_mesh() {
        let s = box_surface([0.0; 3], [1.0, 0.1, 0.1], [20, 2, 2]);
        let tm = tetrahedralize(&s, &opts()).unwrap();
        let r = TetRemesher::from_tet_mesh(
            &tm,
            box_planes([0.0; 3], [1.0, 0.1, 0.1]),
            RemeshSettings::for_size(0.035),
        )
        .unwrap();
        let m = r.mesh().unwrap();
        assert_eq!(m.n_cells(), 1711);
        assert!((0..m.n_cells()).all(|c| m.cell_faces(c).len() == 4));
        let names: Vec<&str> = m.patches().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"]);
        let nb: usize = m.patches().iter().map(|p| p.size).sum();
        assert_eq!(nb, tm.surface.tris.len());
        let v = m.total_volume();
        println!(
            "U3: cells {}, patch sizes {:?}, volume error {:.3e}",
            m.n_cells(),
            m.patches().iter().map(|p| p.size).collect::<Vec<_>>(),
            (v / 0.01 - 1.0).abs()
        );
        assert!((v / 0.01 - 1.0).abs() <= 1e-13);
    }
}
