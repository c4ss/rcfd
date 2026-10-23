//! Non-conformal mesh interfaces (F-114): two meshes whose interface patches do not match face by face are joined into
//! one ordinary conformal polyhedral mesh by intersecting the two patches.
//!
//! Design: the finite-volume solver is not touched. Every non-empty overlap of a face of patch A with a face of patch B
//! becomes one internal face with the A cell as owner and the B cell as neighbour, whose area vector is the overlap area
//! times the common normal and whose centre is the overlap centroid. The original interface faces are deleted. Since
//! the overlaps tile every original face, every cell stays closed (its face area vectors sum to zero to round-off) and
//! whatever flux leaves the A side through the interface enters the B side, by construction, to round-off.
//!
//! Method: both patches must be planar, coplanar and face each other. Their faces are projected to plane coordinates;
//! each A face is clipped against each B face (Sutherland-Hodgman, so faces must be convex, which holds for the
//! quadrilaterals of block meshes and for most polygons of unstructured meshes). Points of the two patches and the
//! intersection points are merged into one registry (merge distance `tol` times the patch size), the B points that
//! coincide with an A point are replaced by it, and every face edge lying in the plane receives the registry points that
//! lie on it (the hanging nodes), so that cells remain topologically closed in the sense of [`Mesh::validate`].
//!
//! Limits: the patches must cover each other completely (error otherwise); face zones are dropped; periodic
//! information of the joined patches is not carried over.

use crate::mesh::{Mesh, PatchKind};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::HashMap;

/// What the join did.
#[derive(Debug, Clone, Default)]
pub struct JoinReport {
    /// Number of overlap faces created.
    pub n_overlap_faces: usize,
    /// Overlaps dropped because their area was below `1e-10` of the smaller parent face.
    pub n_dropped: usize,
    /// Number of face edges that received hanging-node points.
    pub n_split_edges: usize,
    /// Total area of patch A and of patch B.
    pub area: [f64; 2],
    /// Worst relative difference between a parent face area and the sum of its overlap areas.
    pub max_tiling_error: f64,
    /// Indices (in the joined mesh) of the overlap faces.
    pub interface_faces: Vec<usize>,
    /// Cells of mesh A come first; the first cell index of mesh B in the joined mesh.
    pub b_cell_offset: usize,
}

type PatchFaces = Vec<(usize, Vec<usize>)>;

struct Registry {
    d: f64,
    bin: f64,
    /// Chart shifts to try in coordinate 0: `[0]`, or `[0, P, -P]` for a chart of period `P` (cylinder).
    shifts: Vec<f64>,
    coords: Vec<[f64; 2]>,
    gid: Vec<usize>,
    bins: HashMap<(i64, i64), Vec<usize>>,
    by_gid: HashMap<usize, usize>,
}

impl Registry {
    fn new(d: f64, bin: f64, period: Option<f64>) -> Self {
        Registry {
            d,
            bin: bin.max(4.0 * d),
            shifts: period.map_or(vec![0.0], |p| vec![0.0, p, -p]),
            coords: vec![],
            gid: vec![],
            bins: HashMap::new(),
            by_gid: HashMap::new(),
        }
    }
    fn key(&self, p: [f64; 2]) -> (i64, i64) {
        ((p[0] / self.bin).floor() as i64, (p[1] / self.bin).floor() as i64)
    }
    fn find(&self, p0: [f64; 2]) -> Option<usize> {
        let mut best: Option<(f64, usize)> = None;
        for &sh in &self.shifts {
            let p = if sh == 0.0 { p0 } else { [p0[0] + sh, p0[1]] };
            self.find_in(p, &mut best);
        }
        best.map(|b| b.1)
    }
    fn find_in(&self, p: [f64; 2], best: &mut Option<(f64, usize)>) {
        let (kx, ky) = self.key(p);
        for i in kx - 1..=kx + 1 {
            for j in ky - 1..=ky + 1 {
                for &k in self.bins.get(&(i, j)).map_or(&[][..], |v| v.as_slice()) {
                    let dist = (self.coords[k][0] - p[0]).hypot(self.coords[k][1] - p[1]);
                    if dist <= self.d && best.map_or(true, |(b, _)| dist < b) {
                        *best = Some((dist, k));
                    }
                }
            }
        }
    }
    fn insert(&mut self, p: [f64; 2], gid: usize) -> usize {
        let k = self.coords.len();
        self.coords.push(p);
        self.gid.push(gid);
        self.bins.entry(self.key(p)).or_default().push(k);
        self.by_gid.insert(gid, k);
        k
    }
    /// Registry points strictly inside the segment `p0 -> p1` (further than `d` from both ends and within `d` of the
    /// line), ordered from `p0`.
    fn on_segment(&self, q0: [f64; 2], q1: [f64; 2]) -> Vec<usize> {
        let mut found: Vec<(f64, usize)> = vec![];
        for &sh in &self.shifts {
            let (p0, p1) = if sh == 0.0 {
                (q0, q1)
            } else {
                ([q0[0] + sh, q0[1]], [q1[0] + sh, q1[1]])
            };
            self.on_segment_in(p0, p1, &mut found);
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().map(|f| f.1).collect()
    }
    fn on_segment_in(&self, p0: [f64; 2], p1: [f64; 2], found: &mut Vec<(f64, usize)>) {
        let e = [p1[0] - p0[0], p1[1] - p0[1]];
        let len = e[0].hypot(e[1]);
        if len <= 2.0 * self.d {
            return;
        }
        let (lo, hi) = (
            self.key([p0[0].min(p1[0]) - self.d, p0[1].min(p1[1]) - self.d]),
            self.key([p0[0].max(p1[0]) + self.d, p0[1].max(p1[1]) + self.d]),
        );
        for i in lo.0..=hi.0 {
            for j in lo.1..=hi.1 {
                for &k in self.bins.get(&(i, j)).map_or(&[][..], |v| v.as_slice()) {
                    let r = [self.coords[k][0] - p0[0], self.coords[k][1] - p0[1]];
                    let s = (r[0] * e[0] + r[1] * e[1]) / len;
                    let off = (r[0] * e[1] - r[1] * e[0]).abs() / len;
                    if off <= self.d && s > self.d && s < len - self.d {
                        found.push((s, self.gid[k]));
                    }
                }
            }
        }
    }
}

/// Coordinates on the interface surface: a plane (`o` a point, `e1`, `e2` in-plane, `n` normal) or a cylinder about the
/// axis through `o` (`(s, z) = (r theta, sign * axial)`, theta measured from `e1` towards `e2 = axis x e1`; `sign` makes
/// patch A's loops counter-clockwise).
#[derive(Clone, Copy)]
enum Chart {
    Plane {
        o: Vec3,
        e1: Vec3,
        e2: Vec3,
        n: Vec3,
    },
    Cylinder {
        o: Vec3,
        axis: Vec3,
        e1: Vec3,
        e2: Vec3,
        r: f64,
        sign: f64,
    },
}

impl Chart {
    /// Chart coordinates of `p` and its offset from the surface.
    fn coords(&self, p: Vec3) -> ([f64; 2], f64) {
        match *self {
            Chart::Plane { o, e1, e2, n } => {
                let r = vec3::sub(p, o);
                ([vec3::dot(r, e1), vec3::dot(r, e2)], vec3::dot(r, n))
            }
            Chart::Cylinder {
                o,
                axis,
                e1,
                e2,
                r,
                sign,
            } => {
                let d = vec3::sub(p, o);
                let (x, y) = (vec3::dot(d, e1), vec3::dot(d, e2));
                ([r * y.atan2(x), sign * vec3::dot(d, axis)], x.hypot(y) - r)
            }
        }
    }
    fn point(&self, c: [f64; 2]) -> Vec3 {
        match *self {
            Chart::Plane { o, e1, e2, .. } => vec3::add(o, vec3::add(vec3::scale(e1, c[0]), vec3::scale(e2, c[1]))),
            Chart::Cylinder {
                o,
                axis,
                e1,
                e2,
                r,
                sign,
            } => {
                let th = c[0] / r;
                let radial = vec3::add(vec3::scale(e1, r * th.cos()), vec3::scale(e2, r * th.sin()));
                vec3::add(o, vec3::add(radial, vec3::scale(axis, sign * c[1])))
            }
        }
    }
    /// Unit normal of patch A's side of the surface at `x`.
    fn normal(&self, x: Vec3) -> Vec3 {
        match *self {
            Chart::Plane { n, .. } => n,
            Chart::Cylinder { o, axis, sign, .. } => {
                let d = vec3::sub(x, o);
                vec3::scale(vec3::unit(vec3::sub(d, vec3::scale(axis, vec3::dot(d, axis)))), sign)
            }
        }
    }
    fn period(&self) -> Option<f64> {
        match *self {
            Chart::Plane { .. } => None,
            Chart::Cylinder { r, .. } => Some(2.0 * std::f64::consts::PI * r),
        }
    }
    /// Move coordinate 0 of every point by whole periods to within half a period of `s_ref` (no-op on a plane).
    fn unwrap(&self, v: &mut [[f64; 2]], s_ref: f64) {
        if let Some(per) = self.period() {
            for q in v {
                q[0] -= per * ((q[0] - s_ref) / per).round();
            }
        }
    }
}

fn shoelace(p: &[[f64; 2]]) -> f64 {
    let n = p.len();
    0.5 * (0..n)
        .map(|i| p[i][0] * p[(i + 1) % n][1] - p[(i + 1) % n][0] * p[i][1])
        .sum::<f64>()
}

/// Sutherland-Hodgman: `subject` clipped against the convex counter-clockwise polygon `clip`.
fn clip_polygon(subject: &[[f64; 2]], clip: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = subject.to_vec();
    for i in 0..clip.len() {
        if out.is_empty() {
            break;
        }
        let (c0, c1) = (clip[i], clip[(i + 1) % clip.len()]);
        let e = [c1[0] - c0[0], c1[1] - c0[1]];
        let len = e[0].hypot(e[1]);
        let side = |p: [f64; 2]| (e[0] * (p[1] - c0[1]) - e[1] * (p[0] - c0[0])) / len;
        let input = std::mem::take(&mut out);
        let mut prev = input[input.len() - 1];
        let mut sp = side(prev);
        for &cur in &input {
            let sc = side(cur);
            let cross = |a: [f64; 2], b: [f64; 2], sa: f64, sb: f64| {
                let t = sa / (sa - sb);
                [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
            };
            if sc >= 0.0 {
                if sp < 0.0 {
                    out.push(cross(prev, cur, sp, sc));
                }
                out.push(cur);
            } else if sp >= 0.0 {
                out.push(cross(prev, cur, sp, sc));
            }
            prev = cur;
            sp = sc;
        }
    }
    out
}

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// Join mesh `b` to mesh `a` across their interface patches `patch_a` and `patch_b` (see the module documentation).
///
/// `tol` is a relative geometric tolerance (a fraction of the interface size) for coplanarity and point merging,
/// typically `1e-8`. Cells of `a` keep their indices, cells of `b` follow (offset [`JoinReport::b_cell_offset`]).
/// Boundary patches of the same name in both meshes are merged (their kinds must agree); the two interface patches
/// disappear.
pub fn join_non_conformal(a: &Mesh, patch_a: &str, b: &Mesh, patch_b: &str, tol: f64) -> Result<(Mesh, JoinReport)> {
    join_impl(a, patch_a, b, patch_b, tol, None)
}

/// Join mesh `b` to mesh `a` across interface patches that lie on one circular cylinder about the axis through `origin`
/// along `axis` (F-126, ADR 0109), e.g. the outer surface of a rotor drum and the inner surface of the stator around it.
///
/// As [`join_non_conformal`], with the patches mapped to `(r theta, axial)` on the cylinder of radius `r` (the mean
/// radius of patch A's points) instead of a plane; overlap points are placed on the cylinder. Every overlap face is a
/// planar chord rectangle over one interval of the merged angles and axial breakpoints (for patches made of such
/// rectangles), so its normal is radial and the frame flux `(Omega x r) . S` of a rotation about the axis vanishes on it.
/// The interface becomes the polygon through the merged angles: cells on either side gain or lose a sliver (their faces
/// that end on the interface receive the new points). Conical surfaces and partial cover are not supported.
pub fn join_non_conformal_cylindrical(
    a: &Mesh,
    patch_a: &str,
    b: &Mesh,
    patch_b: &str,
    origin: Vec3,
    axis: Vec3,
    tol: f64,
) -> Result<(Mesh, JoinReport)> {
    if vec3::norm(axis) <= 1e-300 || !vec3::norm(axis).is_finite() {
        return bad("cylindrical interface: the axis must be a finite non-zero vector");
    }
    join_impl(a, patch_a, b, patch_b, tol, Some((origin, vec3::unit(axis))))
}

fn join_impl(
    a: &Mesh,
    patch_a: &str,
    b: &Mesh,
    patch_b: &str,
    tol: f64,
    cylinder: Option<(Vec3, Vec3)>,
) -> Result<(Mesh, JoinReport)> {
    let pa = a
        .patch(patch_a)
        .ok_or_else(|| Error::Mesh(format!("mesh A has no patch '{patch_a}'")))?
        .clone();
    let pb = b
        .patch(patch_b)
        .ok_or_else(|| Error::Mesh(format!("mesh B has no patch '{patch_b}'")))?
        .clone();
    if pa.size == 0 || pb.size == 0 {
        return bad("empty interface patch");
    }
    // plane from patch A, or the cylinder through patch A's points
    let basis = |n: Vec3| {
        let mut e1 = if n[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        e1 = vec3::unit(vec3::sub(e1, vec3::scale(n, vec3::dot(e1, n))));
        (e1, vec3::cross(n, e1))
    };
    let chart = match cylinder {
        None => {
            let mut nsum = vec3::ZERO;
            for f in pa.faces() {
                nsum = vec3::add(nsum, a.face_area_vectors()[f]);
            }
            let n = vec3::unit(nsum);
            let (e1, e2) = basis(n);
            Chart::Plane {
                o: a.face_centres()[pa.start],
                e1,
                e2,
                n,
            }
        }
        Some((o, axis)) => {
            let (e1, e2) = basis(axis);
            let unit_cyl = Chart::Cylinder {
                o,
                axis,
                e1,
                e2,
                r: 1.0,
                sign: 1.0,
            };
            let (mut rsum, mut cnt) = (0.0, 0.0);
            for f in pa.faces() {
                for &p in a.face_points(f) {
                    rsum += unit_cyl.coords(a.points()[p]).1 + 1.0;
                    cnt += 1.0;
                }
            }
            let r = rsum / cnt;
            if r <= 0.0 || !r.is_finite() {
                return bad("cylindrical interface: patch A has no positive radius about the axis");
            }
            let f0 = pa.start;
            let side = vec3::dot(a.face_area_vectors()[f0], unit_cyl.normal(a.face_centres()[f0]));
            Chart::Cylinder {
                o,
                axis,
                e1,
                e2,
                r,
                sign: if side < 0.0 { -1.0 } else { 1.0 },
            }
        }
    };
    let to_plane = |p: Vec3| chart.coords(p);
    // extent and planarity
    let np_a = a.n_points();
    let mut ext = [[f64::INFINITY; 2], [f64::NEG_INFINITY; 2]];
    let mut max_off: f64 = 0.0;
    let gather = |m: &Mesh, patch: &crate::mesh::Patch, sign: f64| -> Result<()> {
        for f in patch.faces() {
            let un = vec3::unit(m.face_area_vectors()[f]);
            if sign * vec3::dot(un, chart.normal(m.face_centres()[f])) < 1.0 - 1e-6 {
                return bad(match chart {
                    Chart::Plane { .. } => format!(
                        "interface face {f} is not parallel to the interface plane (planar, opposed patches required)"
                    ),
                    Chart::Cylinder { .. } => format!(
                        "interface face {f} is not radial (patches on one cylinder about the axis, facing each other, required)"
                    ),
                });
            }
        }
        Ok(())
    };
    gather(a, &pa, 1.0)?;
    gather(b, &pb, -1.0)?;
    for (m, patch) in [(a, &pa), (b, &pb)] {
        for f in patch.faces() {
            for &p in m.face_points(f) {
                let (c, off) = to_plane(m.points()[p]);
                max_off = max_off.max(off.abs());
                for k in 0..2 {
                    ext[0][k] = ext[0][k].min(c[k]);
                    ext[1][k] = ext[1][k].max(c[k]);
                }
            }
        }
    }
    let size = (ext[1][0] - ext[0][0]).hypot(ext[1][1] - ext[0][1]);
    let d = tol * size;
    if max_off > 1e3 * d {
        return bad(match chart {
            Chart::Plane { .. } => format!(
                "interface patches are not coplanar: max distance {max_off:.3e} from the plane (merge tolerance {d:.3e})"
            ),
            Chart::Cylinder { r, .. } => format!(
                "interface patches do not lie on one cylinder: max distance {max_off:.3e} from radius {r} (merge tolerance {d:.3e})"
            ),
        });
    }
    let n_pts_est = (pa.size + pb.size) as f64;
    let mut reg = Registry::new(d, size / n_pts_est.sqrt().max(1.0), chart.period());
    // combined points
    let mut points: Vec<Vec3> = a.points().to_vec();
    points.extend_from_slice(b.points());
    let mut remap: Vec<usize> = (0..points.len()).collect();
    for f in pa.faces() {
        for &p in a.face_points(f) {
            if !reg.by_gid.contains_key(&p) {
                reg.insert(to_plane(a.points()[p]).0, p);
            }
        }
    }
    for f in pb.faces() {
        for &p in b.face_points(f) {
            let g = np_a + p;
            if remap[g] != g || reg.by_gid.contains_key(&g) {
                continue;
            }
            let c = to_plane(b.points()[p]).0;
            match reg.find(c) {
                Some(k) => remap[g] = reg.gid[k],
                None => {
                    reg.insert(c, g);
                }
            }
        }
    }
    // overlaps
    struct Overlap {
        owner: usize,
        neighbour: usize,
        ids: Vec<usize>,
    }
    let n_cells_a = a.n_cells();
    let mut overlaps: Vec<Overlap> = vec![];
    let mut n_dropped = 0;
    let plane_loop = |m: &Mesh, f: usize, off: usize, rev: bool, reg: &Registry, remap: &[usize]| {
        let mut v: Vec<[f64; 2]> = m
            .face_points(f)
            .iter()
            .map(|&p| reg.coords[reg.by_gid[&remap[off + p]]])
            .collect();
        if rev {
            v.reverse();
        }
        chart.unwrap(&mut v, to_plane(m.face_centres()[f]).0[0]);
        v
    };
    let b_faces: Vec<(usize, Vec<[f64; 2]>)> = pb
        .faces()
        .map(|f| (f, plane_loop(b, f, np_a, true, &reg, &remap)))
        .collect();
    let b_box: Vec<([f64; 2], [f64; 2])> = b_faces
        .iter()
        .map(|(_, l)| {
            let mut lo = [f64::INFINITY; 2];
            let mut hi = [f64::NEG_INFINITY; 2];
            for p in l {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            (lo, hi)
        })
        .collect();
    let mut tiling_a = vec![0.0; pa.size];
    let mut tiling_b = vec![0.0; pb.size];
    let area_a_face: Vec<f64> = pa
        .faces()
        .map(|f| shoelace(&plane_loop(a, f, 0, false, &reg, &remap)))
        .collect();
    let area_b_face: Vec<f64> = b_faces.iter().map(|(_, l)| shoelace(l)).collect();
    if area_a_face.iter().chain(&area_b_face).any(|&x| x <= 0.0) {
        return bad("an interface face has non-positive area in the interface plane (non-convex or inverted face)");
    }
    let shifts = reg.shifts.clone();
    for (ia, fa) in pa.faces().enumerate() {
        let sa = plane_loop(a, fa, 0, false, &reg, &remap);
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in &sa {
            for k in 0..2 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        for ((ib, (fb, cb0)), sh) in b_faces
            .iter()
            .enumerate()
            .flat_map(|x| shifts.iter().map(move |&sh| (x, sh)))
        {
            let (mut blo, mut bhi) = b_box[ib];
            if sh != 0.0 {
                blo[0] += sh;
                bhi[0] += sh;
            }
            if blo[0] > hi[0] || bhi[0] < lo[0] || blo[1] > hi[1] || bhi[1] < lo[1] {
                continue;
            }
            let shifted: Vec<[f64; 2]>;
            let cb: &[[f64; 2]] = if sh == 0.0 {
                cb0
            } else {
                shifted = cb0.iter().map(|q| [q[0] + sh, q[1]]).collect();
                &shifted
            };
            let poly = clip_polygon(&sa, cb);
            if poly.len() < 3 {
                continue;
            }
            let area = shoelace(&poly);
            if area <= 1e-10 * area_a_face[ia].min(area_b_face[ib]) {
                n_dropped += 1;
                continue;
            }
            let mut ids: Vec<usize> = vec![];
            for p in &poly {
                let g = match reg.find(*p) {
                    Some(k) => reg.gid[k],
                    None => {
                        let g = points.len();
                        points.push(chart.point(*p));
                        remap.push(g);
                        reg.insert(*p, g);
                        g
                    }
                };
                if ids.last() != Some(&g) {
                    ids.push(g);
                }
            }
            if ids.len() > 1 && ids[0] == ids[ids.len() - 1] {
                ids.pop();
            }
            if ids.len() < 3 {
                n_dropped += 1;
                continue;
            }
            let mut ring = ids.iter().map(|g| reg.coords[reg.by_gid[g]]).collect::<Vec<_>>();
            let s0 = ring[0][0];
            chart.unwrap(&mut ring, s0);
            let area = shoelace(&ring);
            tiling_a[ia] += area;
            tiling_b[ib] += area;
            overlaps.push(Overlap {
                owner: a.owner()[fa],
                neighbour: n_cells_a + b.owner()[*fb],
                ids,
            });
        }
    }
    let mut max_tiling_error: f64 = 0.0;
    for (t, a0) in tiling_a
        .iter()
        .zip(&area_a_face)
        .chain(tiling_b.iter().zip(&area_b_face))
    {
        max_tiling_error = max_tiling_error.max((t - a0).abs() / a0);
    }
    if max_tiling_error > 1e-6 {
        return bad(format!(
            "the interface patches do not cover each other: worst face tiling error {max_tiling_error:.3e}"
        ));
    }
    // faces of the joined mesh: (owner, neighbour or usize::MAX, loop)
    let mut n_split = 0;
    let mut split = |ids: Vec<usize>, reg: &Registry| -> Vec<usize> {
        let n = ids.len();
        let mut out = Vec::with_capacity(n + 2);
        for i in 0..n {
            let (p, q) = (ids[i], ids[(i + 1) % n]);
            out.push(p);
            if let (Some(&kp), Some(&kq)) = (reg.by_gid.get(&p), reg.by_gid.get(&q)) {
                let mut pq = [reg.coords[kp], reg.coords[kq]];
                let s0 = pq[0][0];
                chart.unwrap(&mut pq, s0);
                let mids = reg.on_segment(pq[0], pq[1]);
                if !mids.is_empty() {
                    n_split += 1;
                    out.extend(mids);
                }
            }
        }
        out
    };
    let mut internal: Vec<(usize, usize, Vec<usize>, bool)> = vec![];
    for f in 0..a.n_internal_faces() {
        internal.push((
            a.owner()[f],
            a.neighbour()[f],
            split(a.face_points(f).to_vec(), &reg),
            false,
        ));
    }
    for f in 0..b.n_internal_faces() {
        let ids: Vec<usize> = b.face_points(f).iter().map(|&p| remap[np_a + p]).collect();
        internal.push((
            n_cells_a + b.owner()[f],
            n_cells_a + b.neighbour()[f],
            split(ids, &reg),
            false,
        ));
    }
    for ov in &overlaps {
        internal.push((ov.owner, ov.neighbour, split(ov.ids.clone(), &reg), true));
    }
    internal.sort_by_key(|x| (x.0, x.1));
    if internal.windows(2).any(|w| (w[0].0, w[0].1) == (w[1].0, w[1].1)) {
        return bad("two cells share more than one face (an A cell overlaps a B cell in two pieces)");
    }
    let interface_faces: Vec<usize> = internal
        .iter()
        .enumerate()
        .filter(|(_, x)| x.3)
        .map(|(i, _)| i)
        .collect();
    // boundary patches, merged by name
    let mut pnames: Vec<(String, PatchKind, PatchFaces)> = vec![];
    for (m, skip, off_c, off_p, is_b) in [(a, &pa.name, 0, 0, false), (b, &pb.name, n_cells_a, np_a, true)] {
        for p in m.patches() {
            if &p.name == skip {
                continue;
            }
            let faces: PatchFaces = p
                .faces()
                .map(|f| {
                    let ids: Vec<usize> = m
                        .face_points(f)
                        .iter()
                        .map(|&q| if is_b { remap[off_p + q] } else { q })
                        .collect();
                    (off_c + m.owner()[f], split(ids, &reg))
                })
                .collect();
            match pnames.iter_mut().find(|x| x.0 == p.name) {
                Some(x) => {
                    if x.1 != p.kind {
                        return bad(format!("patch '{}' has different kinds in the two meshes", p.name));
                    }
                    x.2.extend(faces);
                }
                None => pnames.push((p.name.clone(), p.kind.clone(), faces)),
            }
        }
    }
    // assemble with compacted points
    let mut used = vec![usize::MAX; points.len()];
    let mut new_points: Vec<Vec3> = vec![];
    let mut renum = |g: usize| -> usize {
        if used[g] == usize::MAX {
            used[g] = new_points.len();
            new_points.push(points[g]);
        }
        used[g]
    };
    let mut faces: Vec<Vec<usize>> = vec![];
    let mut owner = vec![];
    let mut neighbour = vec![];
    for (o_, n_, ids, _) in &internal {
        faces.push(ids.iter().map(|&g| renum(g)).collect());
        owner.push(*o_);
        neighbour.push(*n_);
    }
    let mut patches = vec![];
    for (name, kind, fl) in &pnames {
        patches.push((name.clone(), kind.clone(), fl.len()));
        for (oc, ids) in fl {
            faces.push(ids.iter().map(|&g| renum(g)).collect());
            owner.push(*oc);
        }
    }
    let mut mesh = Mesh::from_face_lists(new_points, faces, owner, neighbour, patches)?;
    // cell zones: merged by name, B ids shifted
    let mut zones: Vec<(String, Vec<usize>)> = vec![];
    for (m, off) in [(a, 0), (b, n_cells_a)] {
        for z in m.cell_zones() {
            let ids: Vec<usize> = z.ids.iter().map(|&c| c + off).collect();
            match zones.iter_mut().find(|x| x.0 == z.name) {
                Some(x) => x.1.extend(ids),
                None => zones.push((z.name.clone(), ids)),
            }
        }
    }
    for (name, ids) in zones {
        mesh.add_cell_zone(&name, ids)?;
    }
    let area = [
        pa.faces().map(|f| a.face_area(f)).sum::<f64>(),
        pb.faces().map(|f| b.face_area(f)).sum::<f64>(),
    ];
    Ok((
        mesh,
        JoinReport {
            n_overlap_faces: overlaps.len(),
            n_dropped,
            n_split_edges: n_split,
            area,
            max_tiling_error,
            interface_faces,
            b_cell_offset: n_cells_a,
        },
    ))
}

/// Overlap-face non-orthogonality (degrees) above which a cylindrical interface is rejected before a run (ADR 0112): the
/// smallest angle at which a steady run diverged in ADR 0109 (72.7 deg), rounded down.
pub const CYLINDRICAL_INTERFACE_MAX_NON_ORTHOGONALITY_DEG: f64 = 70.0;
/// Largest overlap-face non-orthogonality with a converged steady run in ADR 0109 (mesh K2, 48.745 deg, `face_flux`).
pub const CYLINDRICAL_INTERFACE_VERIFIED_DEG: f64 = 48.75;

/// Worst non-orthogonality of a set of interface faces and what it would take to bring it to a limit (ADR 0112).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct InterfaceOrthogonality {
    /// Worst angle between the face area vector and the owner-to-neighbour centre vector `d`, degrees.
    pub worst_deg: f64,
    /// Face where it occurs.
    pub face: usize,
    /// Owner cell of that face.
    pub owner: usize,
    /// Neighbour cell of that face.
    pub neighbour: usize,
    /// Centre of that face.
    pub centre: Vec3,
    /// Part of `d` normal to the face at the worst face.
    pub d_normal: f64,
    /// Part of `d` along the face at the worst face.
    pub d_tangential: f64,
    /// The limit, degrees.
    pub limit_deg: f64,
    /// Normal spacing `d_tangential / tan(limit)` that would meet the limit.
    pub needed_d_normal: f64,
    /// `needed_d_normal / d_normal`: the factor by which the spacing across the interface must grow.
    pub factor: f64,
    /// `worst_deg > limit_deg`.
    pub exceeds: bool,
    /// `worst_deg <= CYLINDRICAL_INTERFACE_VERIFIED_DEG` (inside the range with a converged run).
    pub verified: bool,
}

impl InterfaceOrthogonality {
    /// Human-readable cause and remedy.
    pub fn message(&self) -> String {
        format!(
            "cylindrical interface: overlap face {} is {:.1} deg non-orthogonal (limit {:.0} deg, ADR 0112; steady runs diverged at \
             72.7 deg and above in ADR 0109): the cell centres on the two sides are {:.3e} apart along the interface but only {:.3e} \
             across it, because the interface cells are thin radially compared with their size around and axially; the radial \
             spacing of the interface cells must grow by a factor of at least {:.2} (to {:.3e}), or their size around and axially \
             shrink by that factor",
            self.face,
            self.worst_deg,
            self.limit_deg,
            self.d_tangential,
            self.d_normal,
            self.factor,
            self.needed_d_normal
        )
    }
}

/// Pre-flight of a joined interface (ADR 0112): the worst non-orthogonality of the internal faces `faces` (normally
/// [`JoinReport::interface_faces`]), measured as in [`crate::quality`], against `limit_deg`.
pub fn interface_non_orthogonality(mesh: &Mesh, faces: &[usize], limit_deg: f64) -> Result<InterfaceOrthogonality> {
    if !(limit_deg > 0.0 && limit_deg < 90.0) {
        return bad("interface pre-flight: the limit must lie between 0 and 90 degrees");
    }
    let (cc, sfs) = (mesh.cell_centres(), mesh.face_area_vectors());
    let mut worst: Option<InterfaceOrthogonality> = None;
    for &f in faces {
        if f >= mesh.n_internal_faces() {
            return bad(format!("interface pre-flight: face {f} is not an internal face"));
        }
        let (o, nb) = (mesh.owner()[f], mesh.neighbour()[f]);
        let (d, n_hat) = (vec3::sub(cc[nb], cc[o]), vec3::unit(sfs[f]));
        let (dt, dn) = (vec3::norm(vec3::cross(d, n_hat)), vec3::dot(d, n_hat));
        let angle = dt.atan2(dn).to_degrees();
        if worst.as_ref().map_or(true, |w| angle > w.worst_deg) {
            let needed = dt / limit_deg.to_radians().tan();
            worst = Some(InterfaceOrthogonality {
                worst_deg: angle,
                face: f,
                owner: o,
                neighbour: nb,
                centre: mesh.face_centres()[f],
                d_normal: dn,
                d_tangential: dt,
                limit_deg,
                needed_d_normal: needed,
                factor: needed / dn,
                exceeds: angle > limit_deg,
                verified: angle <= CYLINDRICAL_INTERFACE_VERIFIED_DEG,
            });
        }
    }
    worst.ok_or_else(|| Error::Mesh("interface pre-flight: no interface faces".into()))
}

/// Radial size `h` of the interface-adjacent cells on both sides of a cylindrical interface of radius `radius` for which the
/// geometric bound of the overlap-face non-orthogonality is `max_deg` (ADR 0112). The blocks span `sector_angle` with
/// `n_around` cells and the same axial range `axial_length` with `n_axial` uniform layers; the centre offset along the
/// interface is below half the sum of the two pitches around (zero for equal counts) and axially (zero for equal layer
/// counts), the centre distance across is about `h`, so `h = hypot(offset around, offset axially) / tan(max_deg)`. At 45 deg
/// the radial size equals that in-surface size: the interface cells are near-isotropic. Returns 0 for a conformal pair.
pub fn cylindrical_interface_layer_size(
    radius: f64,
    sector_angle: f64,
    n_around: [usize; 2],
    axial_length: f64,
    n_axial: [usize; 2],
    max_deg: f64,
) -> f64 {
    let half_sum = |len: f64, n: [usize; 2]| {
        if n[0] == n[1] {
            0.0
        } else {
            0.5 * (len / n[0] as f64 + len / n[1] as f64)
        }
    };
    let t = half_sum(radius * sector_angle, n_around);
    let z = half_sum(axial_length, n_axial);
    t.hypot(z) / max_deg.to_radians().tan()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{channel, warp, Grading};

    fn block(nx: usize, ny: usize, x0: f64) -> Mesh {
        let m = channel([nx, ny, 1], [1.0, 1.0, 0.1], [Grading::Uniform; 3], true).unwrap();
        warp(&m, |p| [p[0] + x0, p[1], p[2]]).unwrap()
    }

    #[test]
    fn matching_blocks_reproduce_the_conformal_mesh() {
        let (m, rep) = join_non_conformal(&block(4, 5, 0.0), "outlet", &block(4, 5, 1.0), "inlet", 1e-8).unwrap();
        assert_eq!(rep.n_overlap_faces, 5);
        assert_eq!(m.n_cells(), 40);
        assert_eq!(m.n_internal_faces(), 2 * (3 * 5 + 4 * 4) + 5);
        assert!((m.total_volume() - 0.2).abs() < 1e-14);
    }

    #[test]
    fn mismatched_blocks_close_and_tile_to_round_off() {
        let a = block(6, 20, 0.0);
        let b = block(5, 27, 1.0);
        let (m, rep) = join_non_conformal(&a, "outlet", &b, "inlet", 1e-8).unwrap();
        println!(
            "{} overlap faces, {} split edges, tiling {:.2e}",
            rep.n_overlap_faces, rep.n_split_edges, rep.max_tiling_error
        );
        // 20 and 27 equal intervals of [0,1] overlap in 20 + 27 - 1 pieces
        assert_eq!(rep.n_overlap_faces, 46);
        assert_eq!(m.n_cells(), a.n_cells() + b.n_cells());
        assert!(rep.max_tiling_error < 1e-14, "{}", rep.max_tiling_error);
        assert!((m.total_volume() - a.total_volume() - b.total_volume()).abs() < 1e-14);
        // every cell's area vectors sum to zero to round-off
        let mut worst: f64 = 0.0;
        for c in 0..m.n_cells() {
            let (mut sum, mut mag) = (vec3::ZERO, 0.0);
            for &f in m.cell_faces(c) {
                let s = m.face_area_vectors()[f];
                let sgn = if m.owner()[f] == c { 1.0 } else { -1.0 };
                sum = vec3::add(sum, vec3::scale(s, sgn));
                mag += vec3::norm(s);
            }
            worst = worst.max(vec3::norm(sum) / mag);
        }
        println!("worst closure {worst:.2e}");
        assert!(worst < 1e-15, "{worst}");
        // the sum of the overlap area vectors equals the area vector of each original patch
        let total = rep
            .interface_faces
            .iter()
            .fold(vec3::ZERO, |s, &f| vec3::add(s, m.face_area_vectors()[f]));
        assert!((total[0] - 0.1).abs() < 1e-15 && total[1].abs() < 1e-15, "{total:?}");
        let area: f64 = rep.interface_faces.iter().map(|&f| m.face_area(f)).sum();
        assert!((area - rep.area[0]).abs() < 1e-15 && (area - rep.area[1]).abs() < 1e-15);
        // merged patches keep their total size
        assert_eq!(
            m.patch("walls").unwrap().size,
            a.patch("walls").unwrap().size + b.patch("walls").unwrap().size
        );
    }

    #[test]
    fn partial_cover_and_misaligned_planes_are_errors() {
        let a = block(4, 5, 0.0);
        let shifted = warp(&block(4, 5, 1.0), |p| [p[0], p[1] + 0.3, p[2]]).unwrap();
        let e = join_non_conformal(&a, "outlet", &shifted, "inlet", 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("cover"), "{e:?}");
        let e = join_non_conformal(&a, "outlet", &block(4, 5, 1.2), "inlet", 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("coplanar"), "{e:?}");
        let e = join_non_conformal(&a, "inlet", &block(4, 5, 1.0), "inlet", 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("parallel"), "{e:?}");
        assert!(join_non_conformal(&a, "nope", &block(4, 5, 1.0), "inlet", 1e-8).is_err());
    }

    #[test]
    fn three_dimensional_interface_with_different_cell_counts_in_both_directions() {
        let mk = |n: [usize; 3], x0: f64| {
            let m = channel(
                n,
                [1.0, 1.0, 1.0],
                [Grading::Uniform, Grading::Symmetric(3.0), Grading::Expansion(2.0)],
                false,
            )
            .unwrap();
            warp(&m, |p| [p[0] + x0, p[1], p[2]]).unwrap()
        };
        let (a, b) = (mk([3, 4, 5], 0.0), mk([2, 7, 3], 1.0));
        let (m, rep) = join_non_conformal(&a, "outlet", &b, "inlet", 1e-8).unwrap();
        println!(
            "3-D: {} overlaps, {} split edges, tiling {:.2e}",
            rep.n_overlap_faces, rep.n_split_edges, rep.max_tiling_error
        );
        assert!(rep.max_tiling_error < 1e-13);
        assert!((m.total_volume() - 2.0).abs() < 1e-13);
        let mut worst: f64 = 0.0;
        for c in 0..m.n_cells() {
            let (mut sum, mut mag) = (vec3::ZERO, 0.0);
            for &f in m.cell_faces(c) {
                let sgn = if m.owner()[f] == c { 1.0 } else { -1.0 };
                sum = vec3::add(sum, vec3::scale(m.face_area_vectors()[f], sgn));
                mag += m.face_area(f);
            }
            worst = worst.max(vec3::norm(sum) / mag);
        }
        assert!(worst < 1e-14, "{worst}");
    }

    // ---- ADR 0109 (wave 43B): cylindrical interface, limits registered in the ADR before this code ran ----

    fn worst_closure(m: &Mesh) -> f64 {
        let mut worst: f64 = 0.0;
        for c in 0..m.n_cells() {
            let (mut sum, mut mag) = (vec3::ZERO, 0.0);
            for &f in m.cell_faces(c) {
                let sgn = if m.owner()[f] == c { 1.0 } else { -1.0 };
                sum = vec3::add(sum, vec3::scale(m.face_area_vectors()[f], sgn));
                mag += m.face_area(f);
            }
            worst = worst.max(vec3::norm(sum) / mag);
        }
        worst
    }

    #[test]
    fn g0_planar_join_bit_identical_to_head_80c26e9() {
        let mk = |n: [usize; 3], x0: f64| {
            let m = channel(
                n,
                [1.0, 1.0, 1.0],
                [Grading::Uniform, Grading::Symmetric(3.0), Grading::Expansion(2.0)],
                false,
            )
            .unwrap();
            warp(&m, |p| [p[0] + x0, p[1], p[2]]).unwrap()
        };
        let (m, _) = join_non_conformal(&mk([3, 4, 5], 0.0), "outlet", &mk([2, 7, 3], 1.0), "inlet", 1e-8).unwrap();
        let a = crate::gen::annulus_3d(48, 16, 2, 0.5, 1.0, 0.2, 0.0, Grading::Uniform).unwrap();
        let b = crate::gen::annulus_3d(48, 11, 3, 0.5, 1.0, 0.2, 0.2, Grading::Uniform).unwrap();
        let (m2, _) = join_non_conformal(&a, "zmax", &b, "zmin", 1e-8).unwrap();
        // recorded at HEAD 80c26e9 before the cylindrical chart was added
        assert_eq!(m.content_hash(), 0xd055127ef92b9c59);
        assert_eq!(m2.content_hash(), 0x7c6b5d7b77039063);
    }

    #[test]
    fn g1_full_annulus_cylindrical_join_48_against_35() {
        use std::f64::consts::PI;
        let (ri, rm, ro, lz) = (0.5, 0.75, 1.0, 0.4);
        let a = crate::gen::annulus_3d(48, 6, 2, ri, rm, lz, 0.0, Grading::Uniform).unwrap();
        let b = crate::gen::annulus_3d(35, 6, 3, rm, ro, lz, 0.0, Grading::Uniform).unwrap();
        let (m, rep) =
            join_non_conformal_cylindrical(&a, "outer", &b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap();
        let closure = worst_closure(&m);
        let vol_exact =
            lz * (0.5 * 35.0 * ro * ro * (2.0 * PI / 35.0).sin() - 0.5 * 48.0 * ri * ri * (2.0 * PI / 48.0).sin());
        let vol_err = (m.total_volume() - vol_exact).abs() / vol_exact;
        let mut th: Vec<f64> = (0..48).map(|k| 2.0 * PI * k as f64 / 48.0).collect();
        th.extend((1..35).map(|j| 2.0 * PI * j as f64 / 35.0));
        th.sort_by(f64::total_cmp);
        th.push(2.0 * PI);
        let area_exact: f64 = th.windows(2).map(|w| lz * 2.0 * rm * (0.5 * (w[1] - w[0])).sin()).sum();
        let area: f64 = rep.interface_faces.iter().map(|&f| m.face_area(f)).sum();
        let area_err = (area - area_exact).abs() / area_exact;
        let (mut tang, mut axial): (f64, f64) = (0.0, 0.0);
        for &f in &rep.interface_faces {
            let (c, s) = (m.face_centres()[f], m.face_area_vectors()[f]);
            let t = vec3::unit([-c[1], c[0], 0.0]);
            tang = tang.max(vec3::dot(t, s).abs() / vec3::norm(s));
            axial = axial.max(s[2].abs() / vec3::norm(s));
        }
        println!(
            "G1: {} overlaps (328), tiling {:.2e} (1e-12), closure {closure:.2e} (1e-14), volume {vol_err:.2e} (1e-12), \
             interface area {area_err:.2e} (1e-12), tangential {tang:.2e} axial {axial:.2e} (1e-12); {} merged angles",
            rep.n_overlap_faces,
            rep.max_tiling_error,
            th.len() - 1
        );
        assert_eq!(rep.n_overlap_faces, 328);
        assert!(rep.max_tiling_error <= 1e-12);
        assert!(closure <= 1e-14);
        assert!(vol_err <= 1e-12);
        assert!(area_err <= 1e-12);
        assert!(tang <= 1e-12 && axial <= 1e-12);
        assert_eq!(m.n_cells(), a.n_cells() + b.n_cells());
        let names: Vec<&str> = m.patches().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["inner", "zmin", "zmax", "outer"]);
    }

    #[test]
    fn g2_sixty_degree_sector_cylindrical_join_keeps_the_periodic_pair() {
        use std::f64::consts::FRAC_PI_3;
        let a = crate::gen::annulus_sector(8, 8, 0.5, 0.75, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
        let b = crate::gen::annulus_sector(6, 8, 0.75, 1.0, FRAC_PI_3, Grading::Uniform, 0.1).unwrap();
        let (mut m, rep) =
            join_non_conformal_cylindrical(&a, "outer", &b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap();
        let closure = worst_closure(&m);
        println!(
            "G2: {} overlaps (12), tiling {:.2e} (1e-12), closure {closure:.2e} (1e-14)",
            rep.n_overlap_faces, rep.max_tiling_error
        );
        assert_eq!(rep.n_overlap_faces, 12);
        assert!(rep.max_tiling_error <= 1e-12);
        assert!(closure <= 1e-14);
        m.set_periodic_rotation("theta_min", "theta_max", [0.0; 3], [0.0, 0.0, 1.0], FRAC_PI_3)
            .unwrap();
        assert_eq!(m.patch("theta_min").unwrap().size, 16);
    }

    #[test]
    fn g3_cylindrical_join_located_errors() {
        let a = crate::gen::annulus_3d(12, 2, 2, 0.5, 0.75, 0.4, 0.0, Grading::Uniform).unwrap();
        let off = crate::gen::annulus_3d(10, 2, 2, 0.76, 1.0, 0.4, 0.0, Grading::Uniform).unwrap();
        let e =
            join_non_conformal_cylindrical(&a, "outer", &off, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("cylinder"), "{e:?}");
        let b = crate::gen::annulus_3d(10, 2, 2, 0.75, 1.0, 0.4, 0.0, Grading::Uniform).unwrap();
        let e = join_non_conformal_cylindrical(&a, "zmax", &b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("radial"), "{e:?}");
        let short = crate::gen::annulus_3d(10, 2, 2, 0.75, 1.0, 0.3, 0.0, Grading::Uniform).unwrap();
        let e =
            join_non_conformal_cylindrical(&a, "outer", &short, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap_err();
        assert!(format!("{e:?}").contains("cover"), "{e:?}");
        assert!(join_non_conformal_cylindrical(&a, "outer", &b, "inner", [0.0; 3], [0.0; 3], 1e-8).is_err());
    }

    // ---- ADR 0112 (wave 44B): pre-flight and interface-layer sizing, limits registered in the ADR before this code ran ----

    fn cyl(a: &Mesh, b: &Mesh) -> (Mesh, JoinReport) {
        join_non_conformal_cylindrical(a, "outer", b, "inner", [0.0; 3], [0.0, 0.0, 1.0], 1e-8).unwrap()
    }

    /// Rotor `n x nr x nz` on 0.5..0.75 and stator on 0.75..1.0, height 0.4.
    fn pair(a: (usize, usize, usize, Grading), b: (usize, usize, usize, Grading)) -> (Mesh, JoinReport) {
        cyl(
            &crate::gen::annulus_3d(a.0, a.1, a.2, 0.5, 0.75, 0.4, 0.0, a.3).unwrap(),
            &crate::gen::annulus_3d(b.0, b.1, b.2, 0.75, 1.0, 0.4, 0.0, b.3).unwrap(),
        )
    }

    fn g2_pair(ga: Grading, gb: Grading) -> (Mesh, JoinReport) {
        use std::f64::consts::FRAC_PI_3;
        cyl(
            &crate::gen::annulus_sector(8, 8, 0.5, 0.75, FRAC_PI_3, ga, 0.1).unwrap(),
            &crate::gen::annulus_sector(6, 8, 0.75, 1.0, FRAC_PI_3, gb, 0.1).unwrap(),
        )
    }

    /// Maximum of the quality-module angle over `faces` and the faces attaining it.
    fn quality_max(m: &Mesh, faces: &[usize]) -> (f64, Vec<usize>) {
        let no = crate::quality::compute_metrics(m).non_orthogonality;
        let mx = faces.iter().map(|&f| no[f]).fold(f64::NEG_INFINITY, f64::max);
        (mx, faces.iter().copied().filter(|&f| no[f] == mx).collect())
    }

    #[test]
    fn p1_p2_preflight_reproduces_the_adr_0109_angles() {
        let lim = CYLINDRICAL_INTERFACE_MAX_NON_ORTHOGONALITY_DEG;
        let u = Grading::Uniform;
        let cases = [
            ("M", pair((48, 8, 2, u), (35, 8, 3, u)), 79.2, true),
            ("M_z", pair((48, 8, 2, u), (35, 8, 2, u)), 75.7, true),
            ("M_t", pair((48, 8, 2, u), (48, 8, 3, u)), 72.7, true),
            (
                "K2",
                pair(
                    (96, 6, 4, Grading::Expansion(3.0)),
                    (70, 6, 6, Grading::Expansion(1.0 / 3.0)),
                ),
                48.7,
                false,
            ),
            ("I", pair((48, 8, 2, u), (48, 8, 2, u)), 0.0, false),
        ];
        for (name, (m, rep), adr0109, flagged) in &cases {
            let q = interface_non_orthogonality(m, &rep.interface_faces, lim).unwrap();
            println!(
                "P2 {name}: worst {:.3} deg (ADR 0109 {adr0109}), face {}, d_n {:.3e} d_t {:.3e}, factor {:.3}, exceeds {}, verified {}",
                q.worst_deg, q.face, q.d_normal, q.d_tangential, q.factor, q.exceeds, q.verified
            );
            assert!((q.worst_deg - adr0109).abs() <= 0.5, "{name}");
            assert_eq!(q.exceeds, *flagged, "{name}");
            if !flagged {
                assert!(q.verified, "{name}");
            }
            if ["M", "K2"].contains(name) {
                let (mx, at) = quality_max(m, &rep.interface_faces);
                println!("P1 {name}: quality max {mx:.12} vs {:.12}", q.worst_deg);
                assert!((mx - q.worst_deg).abs() <= 1e-9 && at.contains(&q.face), "{name}");
            }
            if *name == "M" {
                let expect = q.worst_deg.to_radians().tan() / lim.to_radians().tan();
                assert!((q.factor - expect).abs() <= 1e-12 * expect, "{} {expect}", q.factor);
                let msg = q.message();
                println!("{msg}");
                assert!(msg.contains("non-orthogonal") && msg.contains("factor"));
            }
        }
        let (m, rep) = g2_pair(u, u);
        let q = interface_non_orthogonality(&m, &rep.interface_faces, lim).unwrap();
        let (mx, at) = quality_max(&m, &rep.interface_faces);
        println!("P1 G2 (reported): worst {:.3} deg, quality max {mx:.12}", q.worst_deg);
        assert!((mx - q.worst_deg).abs() <= 1e-9 && at.contains(&q.face));
        assert!(interface_non_orthogonality(&m, &[m.n_internal_faces()], lim).is_err());
        assert!(interface_non_orthogonality(&m, &[], lim).is_err());
    }

    #[test]
    fn h1_sized_interface_layer_meets_the_45_degree_bound() {
        use std::f64::consts::{FRAC_PI_3, PI};
        let span = 0.25;
        // (name, around, radial, axial, sector)
        let cases = [
            ("G2", [8, 6], 8, [1, 1], true),
            ("K2", [96, 70], 6, [4, 6], false),
            ("M", [48, 35], 8, [2, 3], false),
        ];
        for (name, around, nr, axial, sector) in cases {
            let angle = if sector { FRAC_PI_3 } else { 2.0 * PI };
            let h = cylindrical_interface_layer_size(0.75, angle, around, 0.4, axial, 45.0);
            let ratio = crate::gen::expansion_for_end_cell(nr, span, h).unwrap();
            let (ga, gb) = (Grading::Expansion(ratio), Grading::Expansion(1.0 / ratio));
            let ra = crate::gen::axis_coords(nr, span, ga).unwrap();
            let rb = crate::gen::axis_coords(nr, span, gb).unwrap();
            let (ha, hb) = (ra[nr] - ra[nr - 1], rb[1] - rb[0]);
            let (m, rep) = if sector {
                g2_pair(ga, gb)
            } else {
                pair((around[0], nr, axial[0], ga), (around[1], nr, axial[1], gb))
            };
            let q = interface_non_orthogonality(&m, &rep.interface_faces, 45.0).unwrap();
            println!(
                "H1 {name}: h {h:.5e}, ratio {ratio:.4}, interface cells {:.2e} / {:.2e} relative, wall cell {:.3e}, worst {:.3} deg (45)",
                (ha - h).abs() / h,
                (hb - h).abs() / h,
                ra[1] - ra[0],
                q.worst_deg
            );
            assert!((ha - h).abs() <= 1e-12 * h && (hb - h).abs() <= 1e-12 * h, "{name}");
            assert!(q.worst_deg <= 45.0, "{name}");
        }
        assert!(crate::gen::expansion_for_end_cell(4, 1.0, 1.0).is_err());
        assert!(crate::gen::expansion_for_end_cell(1, 1.0, 1.5).is_err());
        assert_eq!(crate::gen::expansion_for_end_cell(1, 1.0, 0.5).unwrap(), 1.0);
        assert_eq!(
            cylindrical_interface_layer_size(0.75, 1.0, [8, 8], 0.4, [2, 2], 45.0),
            0.0
        );
    }
}
