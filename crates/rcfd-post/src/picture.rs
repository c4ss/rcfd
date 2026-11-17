//! Derived pictures of a stored field as geometry (ADR 0738 P3, P4; F-809): the boundary surface of a stored mesh as the
//! data channel hands it over, the Studio's cut of it by an axis plane, the vector arrows on it, a slice of the cells, and
//! the GLB (glTF 2.0 binary) file these are written to.
//!
//! The rules were the Studio's (ADR 0801 Addendum 7, T1 to T3) and are the engine's now; the Studio draws this output:
//!
//! - **T1**, [`mesh_layers`]: a store is 2D when every cell owns a surface triangle with all three corners on the low bound
//!   of the thinnest direction and one on its high bound (corners compared as the stored `f32`).
//! - **T2**, [`clip_surface`]: the cut keeps the points with `n . p <= d`, `n = e_a` (keep below) or `-e_a` (keep above),
//!   `d = ±at` (`at` an `f32`). A triangle across the plane is clipped to the kept side (Sutherland and Hodgman 1974,
//!   one plane); a new corner lies on the plane exactly (its coordinate along the axis is `at`).
//! - **T3**, [`vector_glyphs`]: one arrow per lattice bin of 24 per axis over the bounding box (the first triangle in
//!   surface order whose centroid falls in the bin), at the centroid, `0.8` lattice steps long for the largest stored
//!   vector, with a head `0.3` of the arrow back and `0.5` of that out to each side, in the triangle's plane. On a 2D
//!   case only the triangles on the face the fitted view looks at (the high side of x, the low side of y, the high side
//!   of z) carry arrows, of the two in-plane components. [`clip_segments`] cuts the arrows by the T2 plane.
//!
//! A GLB written here ([`GlbBuilder`]) has `POSITION` (`f32`, metres, `rcfd_tsstore::LENGTH_UNIT`), `_CELL` (`u32`, the
//! cell of each vertex) and, as the picture has them, `_VALUE` (`f32`), `_TRIANGLE` (`u32`, the triangle of the data
//! channel's surface a vertex came from) and `_VECTOR` (`f32` VEC3). Every file passes
//! [`crate::glb_check::validate_glb`]; [`read_primitives`] reads one back.

use crate::surfaces::cut_plane;
use rcfd_core::{Error, Result};
use rcfd_mesh::Mesh;
use serde_json::{json, Value};
use std::cmp::Ordering::Greater;
use std::collections::{BTreeMap, HashSet};

/// The arrows' lattice: bins per axis over the bounding box (`render.png`'s `overlay_vectors(.., 24, ..)`).
pub const VECTOR_DENSITY: usize = 24;
/// The arrow of the largest vector, in lattice steps.
pub const VECTOR_ARROW: f64 = 0.8;
/// The head: back along the arrow, as a part of the arrow's length.
pub const VECTOR_HEAD_BACK: f64 = 0.3;
/// The head: out to each side, as a part of the head's length back.
pub const VECTOR_HEAD_SIDE: f64 = 0.5;
/// The side of the thinnest direction the Studio's fitted view looks from (the camera along (1, -1.6, 1.1)).
pub const FRONT_SIGN: [f64; 3] = [1.0, -1.0, 1.0];

/// The boundary surface of a stored mesh as `data.surface` hands it over (F-447): the boundary faces cut into triangles,
/// a fan per face, points in first-use order and rounded to `f32`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoreSurface {
    /// Three per point.
    pub points: Vec<f32>,
    /// Three corners per triangle.
    pub triangles: Vec<u32>,
    /// The owner cell of each triangle.
    pub cells: Vec<u32>,
}

impl StoreSurface {
    /// Triangle count.
    pub fn n_triangles(&self) -> usize {
        self.cells.len()
    }

    /// Corner `k` of triangle `t` (the stored `f32` coordinates).
    pub fn corner(&self, t: usize, k: usize) -> [f32; 3] {
        let i = self.triangles[3 * t + k] as usize;
        [self.points[3 * i], self.points[3 * i + 1], self.points[3 * i + 2]]
    }
}

/// [`StoreSurface`] of a mesh (the data channel's `data.surface`). Fails when the mesh has `2^32 - 1` or more points or
/// cells.
pub fn store_surface(mesh: &Mesh) -> Result<StoreSurface> {
    let t = mesh.topology();
    if t.points.len() >= u32::MAX as usize || mesh.n_cells() >= u32::MAX as usize {
        return Err(Error::Other("the mesh has more than 2^32 - 1 points or cells".into()));
    }
    let mut new_id = vec![u32::MAX; t.points.len()];
    let mut s = StoreSurface::default();
    let mut n_points = 0u32;
    for f in t.neighbour.len()..t.owner.len() {
        let corners = &t.face_points[t.face_offsets[f]..t.face_offsets[f + 1]];
        for &c in corners {
            if new_id[c] == u32::MAX {
                new_id[c] = n_points;
                n_points += 1;
                s.points.extend(t.points[c].iter().map(|&x| x as f32));
            }
        }
        for k in 1..corners.len().saturating_sub(1) {
            s.triangles
                .extend_from_slice(&[new_id[corners[0]], new_id[corners[k]], new_id[corners[k + 1]]]);
            s.cells.push(t.owner[f] as u32);
        }
    }
    Ok(s)
}

/// T1: the layers of a stored mesh along its thinnest direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layers {
    /// One cell layer (the 2D case of `render_field`).
    pub two_d: bool,
    /// The thinnest direction of the surface's bounding box (the first of equals).
    pub thin: usize,
    /// Bounding box of the surface's points (the stored `f32`).
    pub min: [f64; 3],
    /// See `min`.
    pub max: [f64; 3],
}

/// Bounding box of the stored points (zero for none).
pub fn surface_bounds(s: &StoreSurface) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in s.points.chunks_exact(3) {
        for k in 0..3 {
            let v = p[k] as f64;
            if v < lo[k] {
                lo[k] = v;
            }
            if v > hi[k] {
                hi[k] = v;
            }
        }
    }
    // none (or a NaN): zero, as the Studio's bounds
    if matches!(lo[0].partial_cmp(&hi[0]), Some(Greater) | None) {
        return ([0.0; 3], [0.0; 3]);
    }
    (lo, hi)
}

/// T1 (see the module comment).
pub fn mesh_layers(s: &StoreSurface, n_cells: usize) -> Layers {
    let (min, max) = surface_bounds(s);
    let ext = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
    let mut thin = 0;
    for k in [1, 2] {
        if ext[k] < ext[thin] {
            thin = k;
        }
    }
    let (lo, hi) = (min[thin], max[thin]);
    let mut on_lo = vec![false; n_cells];
    let mut on_hi = vec![false; n_cells];
    for t in 0..s.n_triangles() {
        let c = s.cells[t] as usize;
        if c >= n_cells {
            continue;
        }
        let z = [0, 1, 2].map(|k| s.corner(t, k)[thin] as f64);
        if z.iter().all(|&v| v == lo) {
            on_lo[c] = true;
        }
        if z.iter().all(|&v| v == hi) {
            on_hi[c] = true;
        }
    }
    let two_d = n_cells > 0 && hi > lo && (0..n_cells).all(|c| on_lo[c] && on_hi[c]);
    Layers { two_d, thin, min, max }
}

/// The velocity components a vector picture draws: the two in-plane ones on a 2D case, all three otherwise.
pub fn vector_components(l: &Layers) -> Vec<usize> {
    if l.two_d {
        (0..3).filter(|&k| k != l.thin).collect()
    } else {
        vec![0, 1, 2]
    }
}

/// T2: an axis plane and the side kept.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cut {
    /// 0, 1, 2 for x, y, z.
    pub axis: usize,
    /// The plane's coordinate, an `f32`.
    pub at: f32,
    /// Keep `p[axis] <= at` (true) or `p[axis] >= at` (false).
    pub below: bool,
}

impl Cut {
    /// Whether the rule keeps a point: `n . p <= d` (exact: `n` is an axis).
    pub fn keeps(&self, p: [f32; 3]) -> bool {
        let x = p[self.axis] as f64;
        let at = self.at as f64;
        if self.below {
            x <= at
        } else {
            -x <= -at
        }
    }

    /// The point on the plane between `p` (kept) and `q` (cut away), on the plane exactly along the axis.
    fn cross(&self, p: [f32; 3], q: [f32; 3]) -> [f32; 3] {
        let a = self.axis;
        let (pa, qa, at) = (p[a] as f64, q[a] as f64, self.at as f64);
        let t = (at - pa) / (qa - pa);
        let mut out = [0f32; 3];
        for k in 0..3 {
            out[k] = if k == a {
                self.at
            } else {
                (p[k] as f64 + t * (q[k] as f64 - p[k] as f64)) as f32
            };
        }
        out
    }
}

/// Triangles as separate corners (nothing welded): what [`clip_surface`] and [`slice`] give.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Triangles {
    /// Three corners per triangle, three coordinates each.
    pub corners: Vec<[f32; 3]>,
    /// The cell of each triangle.
    pub cells: Vec<u32>,
    /// The triangle of the source surface each came from.
    pub source: Vec<u32>,
}

impl Triangles {
    /// Triangle count.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// No triangle.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

fn area2(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> f64 {
    let u = [0, 1, 2].map(|k| b[k] as f64 - a[k] as f64);
    let v = [0, 1, 2].map(|k| c[k] as f64 - a[k] as f64);
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()
}

/// T2: the surface's triangles on the kept side of `cut` (all of them for `None`), those across the plane clipped to it.
/// A triangle kept whole keeps its corners bit for bit; a clipped piece of no area is dropped.
pub fn clip_surface(s: &StoreSurface, cut: Option<&Cut>) -> Triangles {
    let mut out = Triangles::default();
    for t in 0..s.n_triangles() {
        let p = [s.corner(t, 0), s.corner(t, 1), s.corner(t, 2)];
        let Some(cut) = cut else {
            out.corners.extend_from_slice(&p);
            out.cells.push(s.cells[t]);
            out.source.push(t as u32);
            continue;
        };
        let kept = p.map(|q| cut.keeps(q));
        if kept.iter().all(|&k| k) {
            out.corners.extend_from_slice(&p);
            out.cells.push(s.cells[t]);
            out.source.push(t as u32);
            continue;
        }
        if !kept.iter().any(|&k| k) {
            continue;
        }
        // Sutherland-Hodgman against one plane
        let mut poly: Vec<[f32; 3]> = Vec::with_capacity(4);
        for i in 0..3 {
            let j = (i + 1) % 3;
            if kept[i] {
                poly.push(p[i]);
            }
            if kept[i] != kept[j] {
                poly.push(if kept[i] {
                    cut.cross(p[i], p[j])
                } else {
                    cut.cross(p[j], p[i])
                });
            }
        }
        for i in 1..poly.len().saturating_sub(1) {
            let (a, b, c) = (poly[0], poly[i], poly[i + 1]);
            if area2(a, b, c) > 0.0 {
                out.corners.extend_from_slice(&[a, b, c]);
                out.cells.push(s.cells[t]);
                out.source.push(t as u32);
            }
        }
    }
    out
}

/// T3: the arrows of a surface.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Glyphs {
    /// The surface triangle each arrow stands on.
    pub triangles: Vec<u32>,
    /// Its owner cell.
    pub cells: Vec<u32>,
    /// Three strokes per arrow (shaft, two head strokes), two points each.
    pub segments: Vec<[[f64; 3]; 2]>,
    /// The largest vector length over every cell (the arrows' scale).
    pub vmax: f64,
    /// One lattice step.
    pub step: f64,
}

fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn len3(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// T3 (see the module comment). `components[k]` is the stored cell field of velocity component k (as the `f32` the data
/// channel hands over), or `None` for a component a 2D case does not draw.
pub fn vector_glyphs(s: &StoreSurface, components: [Option<&[f32]>; 3], l: &Layers, n_cells: usize) -> Glyphs {
    let used = vector_components(l);
    let comp = |c: usize, k: usize| -> f64 {
        match components[k] {
            Some(v) if used.contains(&k) => v[c] as f64,
            _ => 0.0,
        }
    };
    let mag = |c: usize| len3([comp(c, 0), comp(c, 1), comp(c, 2)]);
    let mut vmax = 0.0f64;
    for c in 0..n_cells {
        let m = mag(c);
        if m > vmax {
            vmax = m;
        }
    }
    let (lo, hi) = (l.min, l.max);
    let big = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2]);
    let step = big / VECTOR_DENSITY as f64;
    let longest = VECTOR_ARROW * step;
    let front = if FRONT_SIGN[l.thin] > 0.0 {
        hi[l.thin]
    } else {
        lo[l.thin]
    };
    let mut taken: HashSet<[i64; 3]> = HashSet::new();
    let mut g = Glyphs {
        vmax,
        step,
        ..Default::default()
    };
    for t in 0..s.n_triangles() {
        let c = s.cells[t] as usize;
        if c >= n_cells {
            continue;
        }
        let q = [0, 1, 2].map(|k| s.corner(t, k).map(|x| x as f64));
        if l.two_d && q.iter().any(|p| p[l.thin] != front) {
            continue;
        }
        let centre = [0, 1, 2].map(|k| (q[0][k] + q[1][k] + q[2][k]) / 3.0);
        let bin = [0, 1, 2].map(|k| {
            let b = (((centre[k] - lo[k]) / (hi[k] - lo[k]).max(1e-300)) * VECTOR_DENSITY as f64).floor();
            b.max(0.0) as i64
        });
        if !taken.insert(bin) {
            continue;
        }
        let v = [comp(c, 0), comp(c, 1), comp(c, 2)];
        let m = len3(v);
        let pos = |x: f64| x.partial_cmp(&0.0) == Some(Greater);
        if !pos(m) || !m.is_finite() || !pos(vmax) {
            continue;
        }
        let d = v.map(|x| x / m);
        let len = longest * m / vmax;
        let tip = [0, 1, 2].map(|k| centre[k] + len * d[k]);
        let e1 = [0, 1, 2].map(|k| q[1][k] - q[0][k]);
        let e2 = [0, 1, 2].map(|k| q[2][k] - q[0][k]);
        let nrm = cross3(e1, e2);
        let mut side = cross3(nrm, d);
        let mut sl = len3(side);
        if sl.partial_cmp(&(1e-9 * len3(nrm))) != Some(Greater) {
            let mut ax = 0;
            for k in 1..3 {
                if d[k].abs() < d[ax].abs() {
                    ax = k;
                }
            }
            let mut e = [0.0; 3];
            e[ax] = 1.0;
            side = cross3(d, e);
            sl = len3(side);
        }
        let side = side.map(|x| x / sl);
        let back = VECTOR_HEAD_BACK * len;
        let out = VECTOR_HEAD_SIDE * back;
        let h1 = [0, 1, 2].map(|k| tip[k] - back * d[k] + out * side[k]);
        let h2 = [0, 1, 2].map(|k| tip[k] - back * d[k] - out * side[k]);
        g.segments.extend_from_slice(&[[centre, tip], [tip, h1], [tip, h2]]);
        g.triangles.push(t as u32);
        g.cells.push(c as u32);
    }
    g
}

/// Line segments as `f32` point pairs with the arrow (index into [`Glyphs::cells`]) each belongs to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Segments {
    /// Two points per segment.
    pub points: Vec<[f32; 2 * 3]>,
    /// The arrow of each segment.
    pub arrow: Vec<u32>,
}

/// T2 for the arrows: each stroke rounded to `f32`, then the part the cut keeps (a stroke across the plane ends on it
/// exactly; a stroke of no length left is dropped). `None` keeps every stroke.
pub fn clip_segments(g: &Glyphs, cut: Option<&Cut>) -> Segments {
    let mut out = Segments::default();
    for (i, seg) in g.segments.iter().enumerate() {
        let a = seg[0].map(|x| x as f32);
        let b = seg[1].map(|x| x as f32);
        let (a, b) = match cut {
            None => (a, b),
            Some(cut) => match (cut.keeps(a), cut.keeps(b)) {
                (true, true) => (a, b),
                (false, false) => continue,
                (true, false) => (a, cut.cross(a, b)),
                (false, true) => (cut.cross(b, a), b),
            },
        };
        if a == b {
            continue;
        }
        out.points.push([a[0], a[1], a[2], b[0], b[1], b[2]]);
        out.arrow.push((i / 3) as u32);
    }
    out
}

/// A slice of the cells (ADR 0738 P3): the exact plane cut `p[axis] = at` of the mesh (`cut_plane`), each triangle with
/// its cell (the values are piecewise constant, as `render_scalar` colours a cell).
pub fn slice(mesh: &Mesh, axis: usize, at: f64) -> Result<Triangles> {
    if axis > 2 {
        return Err(Error::Other(format!("slice: axis {axis} is not 0, 1 or 2")));
    }
    let mut point = [0.0; 3];
    point[axis] = at;
    let mut normal = [0.0; 3];
    normal[axis] = 1.0;
    let s = cut_plane(mesh, point, normal)?;
    let mut out = Triangles::default();
    for (t, tri) in s.tris.iter().enumerate() {
        let Some(c) = s.tri_cell[t] else { continue };
        for &v in tri {
            out.corners.push(s.points[v].map(|x| x as f32));
        }
        out.cells.push(c as u32);
        out.source.push(t as u32);
    }
    Ok(out)
}

/// A GLB under construction: accessors over one binary buffer, primitives of one mesh.
#[derive(Debug, Default)]
pub struct GlbBuilder {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
    prims: Vec<Value>,
}

fn f32_minmax(v: &[f32], comps: usize) -> (Value, Value) {
    let mut lo = vec![f32::INFINITY; comps];
    let mut hi = vec![f32::NEG_INFINITY; comps];
    for e in v.chunks_exact(comps) {
        for k in 0..comps {
            lo[k] = lo[k].min(e[k]);
            hi[k] = hi[k].max(e[k]);
        }
    }
    (json!(lo), json!(hi))
}

impl GlbBuilder {
    /// An empty file.
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    fn add(
        &mut self,
        bytes: &[u8],
        target: u64,
        comp: u64,
        ty: &str,
        count: usize,
        minmax: Option<(Value, Value)>,
    ) -> usize {
        let off = self.bin.len();
        self.bin.extend_from_slice(bytes);
        self.views
            .push(json!({"buffer": 0, "byteOffset": off, "byteLength": bytes.len(), "target": target}));
        let mut a = json!({"bufferView": self.views.len() - 1, "componentType": comp, "count": count, "type": ty});
        if let Some((mn, mx)) = minmax {
            a["min"] = mn;
            a["max"] = mx;
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }

    /// `f32` vectors of `comps` components (3 for `POSITION`, which gets its bounds); returns the accessor.
    pub fn floats(&mut self, v: &[f32], comps: usize, bounds: bool) -> usize {
        let ty = match comps {
            1 => "SCALAR",
            2 => "VEC2",
            3 => "VEC3",
            _ => "VEC4",
        };
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        let mm = bounds.then(|| f32_minmax(v, comps));
        self.add(&bytes, 34962, 5126, ty, v.len() / comps, mm)
    }

    /// A `u32` vertex attribute; returns the accessor.
    pub fn uints(&mut self, v: &[u32]) -> usize {
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        self.add(&bytes, 34962, 5125, "SCALAR", v.len(), None)
    }

    /// Indices; returns the accessor.
    pub fn indices(&mut self, v: &[u32]) -> usize {
        let bytes: Vec<u8> = v.iter().flat_map(|x| x.to_le_bytes()).collect();
        let hi = v.iter().copied().max().unwrap_or(0);
        let lo = v.iter().copied().min().unwrap_or(0);
        self.add(&bytes, 34963, 5125, "SCALAR", v.len(), Some((json!([lo]), json!([hi]))))
    }

    /// A primitive: `attributes` maps names to accessors, `mode` is glTF's (0 points, 1 lines, 3 line strip, 4
    /// triangles). Nothing is added for an empty `POSITION` accessor list.
    pub fn primitive(&mut self, attributes: &[(&str, usize)], indices: Option<usize>, mode: u64, extras: Value) {
        let attrs: serde_json::Map<String, Value> = attributes.iter().map(|(k, a)| (k.to_string(), json!(a))).collect();
        let mut p = json!({"attributes": attrs, "mode": mode});
        if let Some(i) = indices {
            p["indices"] = json!(i);
        }
        if !extras.is_null() {
            p["extras"] = extras;
        }
        self.prims.push(p);
    }

    /// Primitives added so far.
    pub fn n_primitives(&self) -> usize {
        self.prims.len()
    }

    /// The file; `extras` goes into `asset.extras` (a file without a primitive has no mesh and an empty scene).
    pub fn finish(mut self, name: &str, extras: Value) -> Result<Vec<u8>> {
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        let mut doc = json!({
            "asset": {"version": "2.0", "generator": "rcfd", "extras": extras},
            "scene": 0,
            "scenes": [{"nodes": []}],
        });
        if !self.prims.is_empty() {
            doc["scenes"] = json!([{"nodes": [0]}]);
            doc["nodes"] = json!([{"mesh": 0, "name": name}]);
            doc["meshes"] = json!([{"name": name, "primitives": self.prims}]);
        }
        if !self.bin.is_empty() {
            doc["buffers"] = json!([{"byteLength": self.bin.len()}]);
            doc["bufferViews"] = json!(self.views);
            doc["accessors"] = json!(self.accessors);
        }
        let mut js = serde_json::to_vec(&doc).map_err(|e| Error::Io(e.to_string()))?;
        while js.len() % 4 != 0 {
            js.push(b' ');
        }
        let total = 12 + 8 + js.len() + if self.bin.is_empty() { 0 } else { 8 + self.bin.len() };
        if total > u32::MAX as usize {
            return Err(Error::Other(format!("a GLB of {total} bytes is over 4 GiB")));
        }
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(js.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&js);
        if !self.bin.is_empty() {
            out.extend_from_slice(&(self.bin.len() as u32).to_le_bytes());
            out.extend_from_slice(b"BIN\0");
            out.extend_from_slice(&self.bin);
        }
        Ok(out)
    }
}

/// Triangles as one primitive (mode 4, three vertices each): `POSITION`, `_CELL`, `_TRIANGLE` and, with `values` (one
/// per triangle), `_VALUE`. Nothing for no triangle.
pub fn add_triangles(b: &mut GlbBuilder, t: &Triangles, values: Option<&[f32]>, extras: Value) {
    if t.is_empty() {
        return;
    }
    let pos: Vec<f32> = t.corners.iter().flatten().copied().collect();
    let per_vertex = |v: &[u32]| -> Vec<u32> { v.iter().flat_map(|&x| [x, x, x]).collect() };
    let p = b.floats(&pos, 3, true);
    let c = b.uints(&per_vertex(&t.cells));
    let s = b.uints(&per_vertex(&t.source));
    let mut attrs = vec![("POSITION", p), ("_CELL", c), ("_TRIANGLE", s)];
    if let Some(v) = values {
        let vv: Vec<f32> = v.iter().flat_map(|&x| [x, x, x]).collect();
        attrs.push(("_VALUE", b.floats(&vv, 1, false)));
    }
    b.primitive(&attrs, None, 4, extras);
}

/// One attribute of a primitive read back.
#[derive(Debug, Clone, PartialEq)]
pub enum Attribute {
    /// `f32` components (`comps` per element).
    F32(Vec<f32>, usize),
    /// `u32` scalars.
    U32(Vec<u32>),
}

/// A primitive read back by [`read_primitives`].
#[derive(Debug, Clone, PartialEq)]
pub struct Primitive {
    /// glTF mode.
    pub mode: u64,
    /// The attributes by name.
    pub attributes: BTreeMap<String, Attribute>,
    /// The indices, when given.
    pub indices: Option<Vec<u32>>,
    /// The primitive's extras.
    pub extras: Value,
}

/// Reads a GLB written here: its JSON document and its primitives (validated first by `validate_glb`).
pub fn read_primitives(bytes: &[u8]) -> std::result::Result<(Value, Vec<Primitive>), String> {
    let doc = crate::glb_check::validate_glb(bytes).map_err(|e| e.to_string())?;
    let jl = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let bin: &[u8] = if bytes.len() > 20 + jl + 8 {
        &bytes[28 + jl..]
    } else {
        &[]
    };
    let read = |ai: &Value| -> std::result::Result<Attribute, String> {
        let a = &doc["accessors"][ai.as_u64().ok_or("accessor index")? as usize];
        let v = &doc["bufferViews"][a["bufferView"].as_u64().ok_or("view")? as usize];
        let o = v["byteOffset"].as_u64().unwrap_or(0) as usize + a["byteOffset"].as_u64().unwrap_or(0) as usize;
        let comps = match a["type"].as_str() {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            _ => 4,
        };
        let n = a["count"].as_u64().ok_or("count")? as usize * comps;
        let word = |k: usize| -> [u8; 4] { bin[o + 4 * k..o + 4 * k + 4].try_into().expect("4 bytes") };
        Ok(match a["componentType"].as_u64() {
            Some(5126) => Attribute::F32((0..n).map(|k| f32::from_le_bytes(word(k))).collect(), comps),
            Some(5125) => Attribute::U32((0..n).map(|k| u32::from_le_bytes(word(k))).collect()),
            other => return Err(format!("component type {other:?} is not read here")),
        })
    };
    let mut out = Vec::new();
    if let Some(prims) = doc["meshes"][0]["primitives"].as_array() {
        for p in prims {
            let mut attributes = BTreeMap::new();
            for (k, ai) in p["attributes"].as_object().ok_or("attributes")? {
                attributes.insert(k.clone(), read(ai)?);
            }
            let indices = match p.get("indices") {
                Some(i) => match read(i)? {
                    Attribute::U32(v) => Some(v),
                    Attribute::F32(..) => return Err("float indices".into()),
                },
                None => None,
            };
            out.push(Primitive {
                mode: p["mode"].as_u64().unwrap_or(4),
                attributes,
                indices,
                extras: p.get("extras").cloned().unwrap_or(Value::Null),
            });
        }
    }
    Ok((doc, out))
}
