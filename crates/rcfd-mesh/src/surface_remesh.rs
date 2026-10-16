//! Size fields and isotropic surface remeshing (F-131, ADR 0030 part B).
//!
//! Incremental remeshing by edge split, collapse, flip and tangential smoothing with projection back onto the input
//! surface, in the manner of the published incremental isotropic remeshing scheme (from memory, unverified). Feature edges
//! (sharp dihedral angle or tag change) are kept. Deterministic: sorted work lists, no random numbers, brute-force
//! closest-point queries with ties resolved by the lowest triangle id.

use crate::surface::{edge_groups, TriSurface};
use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, Vec3};
use rcfd_core::{Error, Result};
use std::collections::BTreeSet;

/// A target edge-length field (ADR 0030 B1).
#[derive(Debug, Clone)]
pub enum SizeField {
    /// Constant size.
    Uniform(f64),
    /// `theta / curvature`, clamped; `theta` is the angular resolution in radians.
    Curvature {
        /// Angular resolution, radians.
        theta: f64,
        /// Lower clamp.
        h_min: f64,
        /// Upper clamp.
        h_max: f64,
    },
    /// Distance along the inward normal to the opposite surface divided by `cells_per_gap`, clamped.
    Proximity {
        /// Cells across a gap.
        cells_per_gap: f64,
        /// Lower clamp.
        h_min: f64,
        /// Upper clamp.
        h_max: f64,
    },
    /// Body of influence: size `h` inside the box, no constraint outside.
    Box {
        /// Lower corner.
        lo: Vec3,
        /// Upper corner.
        hi: Vec3,
        /// Size inside.
        h: f64,
    },
    /// Minimum of several fields.
    Min(Vec<SizeField>),
}

/// Remeshing options.
#[derive(Debug, Clone)]
pub struct RemeshOptions {
    /// Outer iterations of split, collapse, flip, smooth.
    pub iterations: usize,
    /// Edges whose dihedral angle exceeds this (degrees) are features.
    pub feature_angle_deg: f64,
    /// Flips are refused when the two triangles' normals differ by more than this (degrees).
    pub flip_angle_deg: f64,
    /// Smoothing passes per iteration.
    pub smooth_passes: usize,
    /// Size growth gradient used to smooth the size field (size change per unit length).
    pub gradient: f64,
}

impl Default for RemeshOptions {
    fn default() -> Self {
        RemeshOptions {
            iterations: 10,
            feature_angle_deg: 40.0,
            flip_angle_deg: 30.0,
            smooth_passes: 2,
            gradient: 0.5,
        }
    }
}

/// Counters from [`remesh`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemeshReport {
    /// Edge splits.
    pub splits: usize,
    /// Edge collapses.
    pub collapses: usize,
    /// Edge flips.
    pub flips: usize,
}

// ------------------------------------------------------------------------------------------------------------
// closest point
// ------------------------------------------------------------------------------------------------------------

/// Closest-point queries on a triangle surface (brute force, deterministic).
pub struct SurfaceQuery<'a> {
    s: &'a TriSurface,
}

pub(crate) fn closest_on_triangle(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> (Vec3, [f64; 3]) {
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let d1 = dot(ab, ap);
    let d2 = dot(ac, ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, [1.0, 0.0, 0.0]);
    }
    let bp = sub(p, b);
    let d3 = dot(ab, bp);
    let d4 = dot(ac, bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, [0.0, 1.0, 0.0]);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (add(a, scale(ab, v)), [1.0 - v, v, 0.0]);
    }
    let cp = sub(p, c);
    let d5 = dot(ab, cp);
    let d6 = dot(ac, cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, [0.0, 0.0, 1.0]);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (add(a, scale(ac, w)), [1.0 - w, 0.0, w]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (add(b, scale(sub(c, b), w)), [0.0, 1.0 - w, w]);
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    (add(a, add(scale(ab, v), scale(ac, w))), [1.0 - v - w, v, w])
}

impl<'a> SurfaceQuery<'a> {
    /// Wrap a surface.
    pub fn new(s: &'a TriSurface) -> Self {
        SurfaceQuery { s }
    }

    /// Closest point, its triangle and barycentric coordinates.
    pub fn closest(&self, p: Vec3) -> (Vec3, usize, [f64; 3]) {
        let mut best = (f64::INFINITY, [0.0; 3], 0usize, [0.0; 3]);
        for (i, t) in self.s.tris.iter().enumerate() {
            let (a, b, c) = (self.s.points[t[0]], self.s.points[t[1]], self.s.points[t[2]]);
            let (q, bc) = closest_on_triangle(p, a, b, c);
            let d = sub(p, q);
            let d2 = dot(d, d);
            if d2 < best.0 {
                best = (d2, q, i, bc);
            }
        }
        (best.1, best.2, best.3)
    }

    /// Distance to the surface.
    pub fn distance(&self, p: Vec3) -> f64 {
        norm(sub(p, self.closest(p).0))
    }
}

// ------------------------------------------------------------------------------------------------------------
// feature edges, curvature, proximity
// ------------------------------------------------------------------------------------------------------------

fn tri_normal(s: &TriSurface, i: usize) -> Vec3 {
    let n = s.tri_area_vector(i);
    let l = norm(n);
    if l > 0.0 {
        scale(n, 1.0 / l)
    } else {
        [0.0; 3]
    }
}

/// Feature edges `(min, max)`: dihedral angle above `angle_deg` or different tags on the two sides.
pub fn feature_edges(s: &TriSurface, angle_deg: f64) -> BTreeSet<(usize, usize)> {
    let cosmin = angle_deg.to_radians().cos();
    let mut out = BTreeSet::new();
    for (e, uses) in edge_groups(&s.tris) {
        if uses.len() == 2 {
            let (t1, t2) = (uses[0].0, uses[1].0);
            let c = dot(tri_normal(s, t1), tri_normal(s, t2));
            if c < cosmin || s.tags[t1] != s.tags[t2] {
                out.insert(e);
            }
        } else {
            out.insert(e);
        }
    }
    out
}

fn vertex_normals(s: &TriSurface) -> Vec<Vec3> {
    let mut n = vec![[0.0; 3]; s.points.len()];
    for (i, t) in s.tris.iter().enumerate() {
        let a = s.tri_area_vector(i);
        for &v in t {
            n[v] = add(n[v], a);
        }
    }
    n.iter()
        .map(|v| {
            let l = norm(*v);
            if l > 0.0 {
                scale(*v, 1.0 / l)
            } else {
                [0.0; 3]
            }
        })
        .collect()
}

/// Curvature estimate per vertex: the largest `angle(n_i, n_j) / |x_i - x_j|` over smooth edges (feature edges are
/// skipped); a vertex on a feature takes the largest estimate of its smooth neighbours.
pub fn curvature(s: &TriSurface, feature_angle_deg: f64) -> Vec<f64> {
    let feat = feature_edges(s, feature_angle_deg);
    let vn = vertex_normals(s);
    let nv = s.points.len();
    let mut on_feat = vec![false; nv];
    for &(a, b) in &feat {
        on_feat[a] = true;
        on_feat[b] = true;
    }
    let mut k = vec![0.0f64; nv];
    let edges: Vec<(usize, usize)> = edge_groups(&s.tris).keys().copied().collect();
    for &(a, b) in &edges {
        if on_feat[a] || on_feat[b] {
            continue;
        }
        let l = norm(sub(s.points[a], s.points[b]));
        if l <= 0.0 {
            continue;
        }
        let ang = norm(cross(vn[a], vn[b])).atan2(dot(vn[a], vn[b]));
        let c = ang / l;
        k[a] = k[a].max(c);
        k[b] = k[b].max(c);
    }
    let base = k.clone();
    for &(a, b) in &edges {
        if on_feat[a] && !on_feat[b] {
            k[a] = k[a].max(base[b]);
        }
        if on_feat[b] && !on_feat[a] {
            k[b] = k[b].max(base[a]);
        }
    }
    k
}

/// Local thickness per vertex: distance along the inward normal to the first hit on the surface (infinity without hit).
pub fn thickness(s: &TriSurface) -> Vec<f64> {
    let vn = vertex_normals(s);
    let mut out = vec![f64::INFINITY; s.points.len()];
    let mut used = vec![false; s.points.len()];
    for t in &s.tris {
        for &v in t {
            used[v] = true;
        }
    }
    for v in 0..s.points.len() {
        if !used[v] {
            continue;
        }
        let o = s.points[v];
        let d = scale(vn[v], -1.0);
        let mut best = f64::INFINITY;
        for t in &s.tris {
            if t.contains(&v) {
                continue;
            }
            let (a, b, c) = (s.points[t[0]], s.points[t[1]], s.points[t[2]]);
            let (e1, e2) = (sub(b, a), sub(c, a));
            let p = cross(d, e2);
            let det = dot(e1, p);
            if det.abs() < 1e-14 {
                continue;
            }
            let inv = 1.0 / det;
            let tv = sub(o, a);
            let u = dot(tv, p) * inv;
            if !(-1e-12..=1.0 + 1e-12).contains(&u) {
                continue;
            }
            let q = cross(tv, e1);
            let w = dot(d, q) * inv;
            if w < -1e-12 || u + w > 1.0 + 1e-12 {
                continue;
            }
            let tt = dot(e2, q) * inv;
            if tt > 1e-9 && tt < best {
                best = tt;
            }
        }
        out[v] = best;
    }
    out
}

fn eval_field(s: &TriSurface, f: &SizeField, feature_angle_deg: f64) -> Vec<f64> {
    let nv = s.points.len();
    match f {
        SizeField::Uniform(h) => vec![*h; nv],
        SizeField::Curvature { theta, h_min, h_max } => curvature(s, feature_angle_deg)
            .iter()
            .map(|&k| {
                if k > 0.0 {
                    (theta / k).clamp(*h_min, *h_max)
                } else {
                    *h_max
                }
            })
            .collect(),
        SizeField::Proximity {
            cells_per_gap,
            h_min,
            h_max,
        } => thickness(s)
            .iter()
            .map(|&d| {
                if d.is_finite() {
                    (d / cells_per_gap).clamp(*h_min, *h_max)
                } else {
                    *h_max
                }
            })
            .collect(),
        SizeField::Box { lo, hi, h } => s
            .points
            .iter()
            .map(|p| {
                if (0..3).all(|d| p[d] >= lo[d] && p[d] <= hi[d]) {
                    *h
                } else {
                    f64::INFINITY
                }
            })
            .collect(),
        SizeField::Min(v) => {
            let mut out = vec![f64::INFINITY; nv];
            for g in v {
                for (o, x) in out.iter_mut().zip(eval_field(s, g, feature_angle_deg)) {
                    *o = o.min(x);
                }
            }
            out
        }
    }
}

fn collect_boxes(f: &SizeField, out: &mut Vec<(Vec3, Vec3, f64)>) {
    match f {
        SizeField::Box { lo, hi, h } => out.push((*lo, *hi, *h)),
        SizeField::Min(v) => v.iter().for_each(|g| collect_boxes(g, out)),
        _ => {}
    }
}

/// Target size at every vertex of `s` for the field `f`, with the growth gradient applied.
pub fn vertex_sizes(s: &TriSurface, f: &SizeField, feature_angle_deg: f64, gradient: f64) -> Vec<f64> {
    let mut h = eval_field(s, f, feature_angle_deg);
    let diag = {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in &s.points {
            for d in 0..3 {
                lo[d] = lo[d].min(p[d]);
                hi[d] = hi[d].max(p[d]);
            }
        }
        norm(sub(hi, lo))
    };
    for x in h.iter_mut() {
        if !x.is_finite() {
            *x = diag;
        }
    }
    let edges: Vec<(usize, usize)> = edge_groups(&s.tris).keys().copied().collect();
    for _ in 0..200 {
        let mut changed = false;
        for &(a, b) in &edges {
            let l = norm(sub(s.points[a], s.points[b]));
            if h[a] > h[b] + gradient * l + 1e-15 {
                h[a] = h[b] + gradient * l;
                changed = true;
            }
            if h[b] > h[a] + gradient * l + 1e-15 {
                h[b] = h[a] + gradient * l;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    h
}

// ------------------------------------------------------------------------------------------------------------
// remeshing
// ------------------------------------------------------------------------------------------------------------

struct Mesh2 {
    pts: Vec<Vec3>,
    h: Vec<f64>,
    alive_v: Vec<bool>,
    tris: Vec<[usize; 3]>,
    tags: Vec<u32>,
    alive: Vec<bool>,
    vt: Vec<Vec<usize>>,
    feat: BTreeSet<(usize, usize)>,
}

fn ek(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

impl Mesh2 {
    fn add_tri(&mut self, t: [usize; 3], tag: u32) -> usize {
        let id = self.tris.len();
        self.tris.push(t);
        self.tags.push(tag);
        self.alive.push(true);
        for &v in &t {
            self.vt[v].push(id);
        }
        id
    }

    fn kill_tri(&mut self, id: usize) {
        self.alive[id] = false;
        for v in self.tris[id] {
            self.vt[v].retain(|&x| x != id);
        }
    }

    fn edge_tris(&self, a: usize, b: usize) -> Vec<usize> {
        self.vt[a]
            .iter()
            .copied()
            .filter(|&t| self.tris[t].contains(&b))
            .collect()
    }

    fn neighbours(&self, v: usize) -> Vec<usize> {
        let mut n: Vec<usize> = self.vt[v]
            .iter()
            .flat_map(|&t| self.tris[t].iter().copied())
            .filter(|&x| x != v)
            .collect();
        n.sort_unstable();
        n.dedup();
        n
    }

    fn feat_deg(&self, v: usize) -> usize {
        self.neighbours(v)
            .iter()
            .filter(|&&x| self.feat.contains(&ek(v, x)))
            .count()
    }

    fn normal(&self, t: [usize; 3]) -> Vec3 {
        cross(sub(self.pts[t[1]], self.pts[t[0]]), sub(self.pts[t[2]], self.pts[t[0]]))
    }

    fn target(&self, a: usize, b: usize) -> f64 {
        0.5 * (self.h[a] + self.h[b])
    }

    fn len(&self, a: usize, b: usize) -> f64 {
        norm(sub(self.pts[a], self.pts[b]))
    }

    fn valence(&self, v: usize) -> i64 {
        self.vt[v].len() as i64
    }
}

struct Ctx<'a> {
    m: Mesh2,
    q: SurfaceQuery<'a>,
    h_in: &'a [f64],
    boxes: Vec<(Vec3, Vec3, f64)>,
    opt: RemeshOptions,
    rep: RemeshReport,
}

impl<'a> Ctx<'a> {
    fn size_at(&self, tri: usize, bary: [f64; 3], p: Vec3) -> f64 {
        let t = self.q.s.tris[tri];
        let mut h = bary[0] * self.h_in[t[0]] + bary[1] * self.h_in[t[1]] + bary[2] * self.h_in[t[2]];
        for (lo, hi, bh) in &self.boxes {
            if (0..3).all(|d| p[d] >= lo[d] && p[d] <= hi[d]) {
                h = h.min(*bh);
            }
        }
        h
    }

    fn split(&mut self, a: usize, b: usize) -> bool {
        let ts = self.m.edge_tris(a, b);
        if ts.len() != 2 {
            return false;
        }
        let is_feat = self.m.feat.contains(&ek(a, b));
        let mid = scale(add(self.m.pts[a], self.m.pts[b]), 0.5);
        let (pos, tri, bary) = {
            let (c, t, bc) = self.q.closest(mid);
            (if is_feat { mid } else { c }, t, bc)
        };
        let h = self.size_at(tri, bary, pos);
        let mi = self.m.pts.len();
        self.m.pts.push(pos);
        self.m.h.push(h);
        self.m.alive_v.push(true);
        self.m.vt.push(Vec::new());
        for t in ts {
            let tr = self.m.tris[t];
            let tag = self.m.tags[t];
            let k = (0..3)
                .find(|&k| (tr[k] == a && tr[(k + 1) % 3] == b) || (tr[k] == b && tr[(k + 1) % 3] == a))
                .expect("edge in triangle");
            let (u, v, w) = (tr[k], tr[(k + 1) % 3], tr[(k + 2) % 3]);
            self.m.kill_tri(t);
            self.m.add_tri([u, mi, w], tag);
            self.m.add_tri([mi, v, w], tag);
        }
        if is_feat {
            self.m.feat.remove(&ek(a, b));
            self.m.feat.insert(ek(a, mi));
            self.m.feat.insert(ek(mi, b));
        }
        self.rep.splits += 1;
        true
    }

    /// Collapse `a` into `b` (vertex `a` disappears).
    fn collapse(&mut self, a: usize, b: usize) -> bool {
        let shared = self.m.edge_tris(a, b);
        if shared.len() != 2 || self.m.vt[a].len() < 3 || self.m.vt[b].len() < 3 {
            return false;
        }
        let edge_feat = self.m.feat.contains(&ek(a, b));
        let da = self.m.feat_deg(a);
        if da != 0 && !(edge_feat && da == 2) {
            return false;
        }
        // other feature edge at a must be re-attached to b without creating a loop
        let mut other_feat: Option<usize> = None;
        if da == 2 {
            for x in self.m.neighbours(a) {
                if x != b && self.m.feat.contains(&ek(a, x)) {
                    other_feat = Some(x);
                }
            }
            match other_feat {
                None => return false,
                Some(x) if self.m.neighbours(b).contains(&x) && !self.m.feat.contains(&ek(b, x)) => {
                    // b already adjacent to x by a non-feature edge: would change a smooth edge into a feature
                    return false;
                }
                _ => {}
            }
        }
        // link condition
        let na = self.m.neighbours(a);
        let nb = self.m.neighbours(b);
        let common: Vec<usize> = na.iter().copied().filter(|x| nb.contains(x)).collect();
        let opp: Vec<usize> = shared
            .iter()
            .map(|&t| *self.m.tris[t].iter().find(|&&x| x != a && x != b).expect("opposite"))
            .collect();
        if common.len() != 2 || !common.iter().all(|c| opp.contains(c)) {
            return false;
        }
        // geometry: no flipped or tiny triangles, no overlong edges
        let high = 4.0 / 3.0;
        for &t in &self.m.vt[a] {
            if shared.contains(&t) {
                continue;
            }
            let tr = self.m.tris[t];
            let new = [
                if tr[0] == a { b } else { tr[0] },
                if tr[1] == a { b } else { tr[1] },
                if tr[2] == a { b } else { tr[2] },
            ];
            let (n0, n1) = (self.m.normal(tr), self.m.normal(new));
            let (l0, l1) = (norm(n0), norm(n1));
            if l1 <= 1e-14 * l0.max(1e-300) || dot(n0, n1) <= 0.0 {
                return false;
            }
            for k in 0..3 {
                let (u, v) = (new[k], new[(k + 1) % 3]);
                if (u == b || v == b) && self.m.len(u, v) > high * self.m.target(u, v) {
                    return false;
                }
            }
        }
        // do it
        let tag_list: Vec<(usize, u32)> = self.m.vt[a].iter().map(|&t| (t, self.m.tags[t])).collect();
        for t in shared.clone() {
            self.m.kill_tri(t);
        }
        for (t, tag) in tag_list {
            if shared.contains(&t) {
                continue;
            }
            let tr = self.m.tris[t];
            self.m.kill_tri(t);
            let new = [
                if tr[0] == a { b } else { tr[0] },
                if tr[1] == a { b } else { tr[1] },
                if tr[2] == a { b } else { tr[2] },
            ];
            self.m.add_tri(new, tag);
        }
        if edge_feat {
            self.m.feat.remove(&ek(a, b));
        }
        if let Some(x) = other_feat {
            self.m.feat.remove(&ek(a, x));
            self.m.feat.insert(ek(b, x));
        }
        self.m.alive_v[a] = false;
        self.rep.collapses += 1;
        true
    }

    fn flip(&mut self, a: usize, b: usize) -> bool {
        if self.m.feat.contains(&ek(a, b)) {
            return false;
        }
        let ts = self.m.edge_tris(a, b);
        if ts.len() != 2 || self.m.tags[ts[0]] != self.m.tags[ts[1]] {
            return false;
        }
        let opp = |t: usize| *self.m.tris[t].iter().find(|&&x| x != a && x != b).expect("opposite");
        let (c, d) = (opp(ts[0]), opp(ts[1]));
        if c == d || !self.m.edge_tris(c, d).is_empty() {
            return false;
        }
        if self.m.valence(a) <= 3 || self.m.valence(b) <= 3 {
            return false;
        }
        let dev = |v: i64| (v - 6) * (v - 6);
        let (va, vb, vc, vd) = (
            self.m.valence(a),
            self.m.valence(b),
            self.m.valence(c),
            self.m.valence(d),
        );
        let before = dev(va) + dev(vb) + dev(vc) + dev(vd);
        let after = dev(va - 1) + dev(vb - 1) + dev(vc + 1) + dev(vd + 1);
        if after >= before {
            return false;
        }
        // orientation: t0 = (.., a -> b, c) with c opposite; build the two new triangles with the same orientation
        let t0 = self.m.tris[ts[0]];
        let k = (0..3).find(|&k| t0[k] == a && t0[(k + 1) % 3] == b);
        let (n1, n2) = if k.is_some() {
            ([c, a, d], [c, d, b])
        } else {
            ([c, b, d], [c, d, a])
        };
        let (old0, old1) = (self.m.normal(self.m.tris[ts[0]]), self.m.normal(self.m.tris[ts[1]]));
        let (m0, m1) = (self.m.normal(n1), self.m.normal(n2));
        let cosflip = self.opt.flip_angle_deg.to_radians().cos();
        let unit = |v: Vec3| {
            let l = norm(v);
            if l > 0.0 {
                scale(v, 1.0 / l)
            } else {
                [0.0; 3]
            }
        };
        if dot(unit(old0), unit(old1)) < cosflip {
            return false;
        }
        if dot(unit(m0), unit(old0)) < 0.5 || dot(unit(m1), unit(old0)) < 0.5 || dot(unit(m0), unit(m1)) < cosflip {
            return false;
        }
        let tag = self.m.tags[ts[0]];
        self.m.kill_tri(ts[0]);
        self.m.kill_tri(ts[1]);
        self.m.add_tri(n1, tag);
        self.m.add_tri(n2, tag);
        self.rep.flips += 1;
        true
    }

    fn smooth(&mut self) {
        let nv = self.m.pts.len();
        for v in 0..nv {
            if !self.m.alive_v[v] || self.m.vt[v].is_empty() || self.m.feat_deg(v) != 0 {
                continue;
            }
            let nb = self.m.neighbours(v);
            let mut c = [0.0; 3];
            for &x in &nb {
                c = add(c, self.m.pts[x]);
            }
            c = scale(c, 1.0 / nb.len() as f64);
            let mut n = [0.0; 3];
            for &t in &self.m.vt[v] {
                n = add(n, self.m.normal(self.m.tris[t]));
            }
            let nl = norm(n);
            if nl <= 0.0 {
                continue;
            }
            let n = scale(n, 1.0 / nl);
            let d = sub(c, self.m.pts[v]);
            let tang = sub(d, scale(n, dot(d, n)));
            let cand = add(self.m.pts[v], tang);
            let (proj, _, _) = self.q.closest(cand);
            let old = self.m.pts[v];
            self.m.pts[v] = proj;
            let ok = self.m.vt[v].iter().all(|&t| {
                let tr = self.m.tris[t];
                let nn = self.m.normal(tr);
                dot(nn, n) > 0.2 * norm(nn)
            });
            if !ok {
                self.m.pts[v] = old;
            }
        }
    }

    fn iteration(&mut self) {
        let high = 4.0 / 3.0;
        let low = 4.0 / 5.0;
        // split
        for _ in 0..8 {
            let mut list: Vec<(f64, usize, usize)> = Vec::new();
            for t in 0..self.m.tris.len() {
                if !self.m.alive[t] {
                    continue;
                }
                let tr = self.m.tris[t];
                for k in 0..3 {
                    let (a, b) = (tr[k], tr[(k + 1) % 3]);
                    if a < b {
                        let l = self.m.len(a, b);
                        if l > high * self.m.target(a, b) {
                            list.push((-l, a, b));
                        }
                    }
                }
            }
            if list.is_empty() {
                break;
            }
            list.sort_by(|x, y| x.partial_cmp(y).expect("finite"));
            list.dedup();
            let mut any = false;
            for (_, a, b) in list {
                if self.m.edge_tris(a, b).len() == 2 && self.m.len(a, b) > high * self.m.target(a, b) {
                    any |= self.split(a, b);
                }
            }
            if !any {
                break;
            }
        }
        // collapse
        for _ in 0..4 {
            let mut list: Vec<(f64, usize, usize)> = Vec::new();
            for t in 0..self.m.tris.len() {
                if !self.m.alive[t] {
                    continue;
                }
                let tr = self.m.tris[t];
                for k in 0..3 {
                    let (a, b) = (tr[k], tr[(k + 1) % 3]);
                    if a < b {
                        let l = self.m.len(a, b);
                        if l < low * self.m.target(a, b) {
                            list.push((l, a, b));
                        }
                    }
                }
            }
            if list.is_empty() {
                break;
            }
            list.sort_by(|x, y| x.partial_cmp(y).expect("finite"));
            list.dedup();
            let mut any = false;
            for (_, a, b) in list {
                if !self.m.alive_v[a] || !self.m.alive_v[b] || self.m.edge_tris(a, b).len() != 2 {
                    continue;
                }
                if self.m.len(a, b) >= low * self.m.target(a, b) {
                    continue;
                }
                if self.collapse(a, b) || self.collapse(b, a) {
                    any = true;
                }
            }
            if !any {
                break;
            }
        }
        // flip
        for _ in 0..4 {
            let mut list: Vec<(usize, usize)> = Vec::new();
            for t in 0..self.m.tris.len() {
                if !self.m.alive[t] {
                    continue;
                }
                let tr = self.m.tris[t];
                for k in 0..3 {
                    let (a, b) = (tr[k], tr[(k + 1) % 3]);
                    if a < b {
                        list.push((a, b));
                    }
                }
            }
            list.sort_unstable();
            list.dedup();
            let mut any = false;
            for (a, b) in list {
                if self.m.alive_v[a] && self.m.alive_v[b] && self.flip(a, b) {
                    any = true;
                }
            }
            if !any {
                break;
            }
        }
        for _ in 0..self.opt.smooth_passes {
            self.smooth();
        }
    }
}

/// Remesh a closed surface to the size field. The input must be a closed manifold (see `diagnose`); topology is preserved.
pub fn remesh(s: &TriSurface, field: &SizeField, opt: &RemeshOptions) -> Result<(TriSurface, RemeshReport)> {
    let groups = edge_groups(&s.tris);
    if groups.values().any(|u| u.len() != 2) {
        return Err(Error::Mesh(
            "remesh needs a closed manifold surface (every edge used by exactly two triangles)".to_string(),
        ));
    }
    let h_in = vertex_sizes(s, field, opt.feature_angle_deg, opt.gradient);
    let mut boxes = Vec::new();
    collect_boxes(field, &mut boxes);
    let mut vt = vec![Vec::new(); s.points.len()];
    for (i, t) in s.tris.iter().enumerate() {
        for &v in t {
            vt[v].push(i);
        }
    }
    let m = Mesh2 {
        pts: s.points.clone(),
        h: h_in.clone(),
        alive_v: vec![true; s.points.len()],
        tris: s.tris.clone(),
        tags: s.tags.clone(),
        alive: vec![true; s.tris.len()],
        vt,
        feat: feature_edges(s, opt.feature_angle_deg),
    };
    let mut ctx = Ctx {
        m,
        q: SurfaceQuery::new(s),
        h_in: &h_in,
        boxes,
        opt: opt.clone(),
        rep: RemeshReport::default(),
    };
    for _ in 0..opt.iterations {
        ctx.iteration();
    }
    // output: renumber by first use
    let m = &ctx.m;
    let mut remap = vec![usize::MAX; m.pts.len()];
    let mut pts = Vec::new();
    let mut tris = Vec::new();
    let mut tags = Vec::new();
    for (i, t) in m.tris.iter().enumerate() {
        if !m.alive[i] {
            continue;
        }
        let mut r = [0usize; 3];
        for k in 0..3 {
            if remap[t[k]] == usize::MAX {
                remap[t[k]] = pts.len();
                pts.push(m.pts[t[k]]);
            }
            r[k] = remap[t[k]];
        }
        tris.push(r);
        tags.push(m.tags[i]);
    }
    let rep = ctx.rep.clone();
    Ok((
        TriSurface {
            points: pts,
            tris,
            tags,
        },
        rep,
    ))
}

/// Per-edge ratio `length / target` of a surface against the size field evaluated through `field` on `reference`
/// (target at an edge = mean of the interpolated end targets). Used to measure size-field adherence.
pub fn edge_ratios(result: &TriSurface, reference: &TriSurface, field: &SizeField, opt: &RemeshOptions) -> Vec<f64> {
    let h_in = vertex_sizes(reference, field, opt.feature_angle_deg, opt.gradient);
    let mut boxes = Vec::new();
    collect_boxes(field, &mut boxes);
    let q = SurfaceQuery::new(reference);
    let hv: Vec<f64> = result
        .points
        .iter()
        .map(|&p| {
            let (c, t, bc) = q.closest(p);
            let tr = reference.tris[t];
            let mut h = bc[0] * h_in[tr[0]] + bc[1] * h_in[tr[1]] + bc[2] * h_in[tr[2]];
            for (lo, hi, bh) in &boxes {
                if (0..3).all(|d| c[d] >= lo[d] && c[d] <= hi[d]) {
                    h = h.min(*bh);
                }
            }
            h
        })
        .collect();
    let mut out = Vec::new();
    for (a, b) in edge_groups(&result.tris).keys() {
        let l = norm(sub(result.points[*a], result.points[*b]));
        out.push(l / (0.5 * (hv[*a] + hv[*b])));
    }
    out
}
