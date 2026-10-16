//! Cartesian-core volume fill with a snapped hex-dominant layer shell (F-234, vertical slice, ADR 0047).
//!
//! A 2:1 balanced octree over a box is filled into the fluid region of a closed triangulated surface. A leaf is kept when all of
//! its corners are in the fluid at least `margin` away from the surface. Hanging nodes are put into every face loop that has a
//! lattice node on one of its edges, so the transition cells are polyhedra and the rest are hexahedra. The staircase left by the
//! kept leaves is bridged to the true surface by a layer shell: every interface polygon is extruded along the closest-point
//! direction of its nodes in `layers` steps whose heights follow a [`LayerRule`] (uniform, aspect ratio, last ratio). The shell
//! shares its inner nodes with the core, so the interface is conforming (the non-conformal join of [`crate::non_conformal`]
//! handles planar patches only and is not used). Without layers the staircase itself is the `body` patch.

use crate::mesh::{BoundarySpec, Mesh, PatchKind};
use crate::surface::TriSurface;
use crate::surface_remesh::{closest_on_triangle, SurfaceQuery};
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::HashMap;

fn bad<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

// ------------------------------------------------------------------------------------------------------------
// point in solid, distance within a cutoff
// ------------------------------------------------------------------------------------------------------------

struct Grid3 {
    lo: Vec3,
    cell: f64,
    n: [usize; 3],
    bins: Vec<Vec<u32>>,
}

struct RayGrid {
    d: Vec3,
    u: Vec3,
    v: Vec3,
    lo: [f64; 2],
    cell: f64,
    n: [usize; 2],
    bins: Vec<Vec<u32>>,
}

/// Spatial index of a closed surface: distance queries with a cutoff and point-in-solid by ray-casting parity.
pub struct SurfaceIndex<'a> {
    s: &'a TriSurface,
    grid: Grid3,
    rays: Vec<RayGrid>,
}

const RAY_DIRS: [Vec3; 3] = [
    [0.332_614_8, 0.514_932_1, 0.791_845_6],
    [-0.610_327_4, 0.208_735_9, 0.764_612_3],
    [0.135_702_5, -0.841_236_8, 0.523_419_7],
];

impl<'a> SurfaceIndex<'a> {
    /// Index a surface.
    pub fn new(s: &'a TriSurface) -> Self {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for p in &s.points {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let ext = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max).max(1e-300);
        let mut mean_edge = 0.0;
        for t in &s.tris {
            mean_edge += vec3::norm(vec3::sub(s.points[t[1]], s.points[t[0]]));
        }
        mean_edge /= s.tris.len().max(1) as f64;
        let cell = (3.0 * mean_edge).max(ext / 48.0).max(1e-300);
        let n = [0, 1, 2].map(|k| ((hi[k] - lo[k]) / cell).floor() as usize + 1);
        let mut grid = Grid3 {
            lo,
            cell,
            n,
            bins: vec![Vec::new(); n[0] * n[1] * n[2]],
        };
        for (ti, t) in s.tris.iter().enumerate() {
            let pts = [s.points[t[0]], s.points[t[1]], s.points[t[2]]];
            let mut a = [usize::MAX; 3];
            let mut b = [0usize; 3];
            for k in 0..3 {
                let l = pts.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min);
                let h = pts.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max);
                a[k] = (((l - lo[k]) / cell).floor().max(0.0) as usize).min(n[k] - 1);
                b[k] = (((h - lo[k]) / cell).floor().max(0.0) as usize).min(n[k] - 1);
            }
            for i in a[0]..=b[0] {
                for j in a[1]..=b[1] {
                    for k in a[2]..=b[2] {
                        grid.bins[i + n[0] * (j + n[1] * k)].push(ti as u32);
                    }
                }
            }
        }
        let rays = RAY_DIRS.iter().map(|&d| Self::ray_grid(s, d, cell)).collect();
        SurfaceIndex { s, grid, rays }
    }

    fn ray_grid(s: &TriSurface, d: Vec3, cell: f64) -> RayGrid {
        let d = vec3::unit(d);
        let seed = if d[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let u = vec3::unit(vec3::cross(d, seed));
        let v = vec3::cross(d, u);
        let proj = |p: Vec3| [vec3::dot(p, u), vec3::dot(p, v)];
        let mut lo = [f64::INFINITY; 2];
        let mut hi = [f64::NEG_INFINITY; 2];
        for p in &s.points {
            let q = proj(*p);
            for k in 0..2 {
                lo[k] = lo[k].min(q[k]);
                hi[k] = hi[k].max(q[k]);
            }
        }
        let n = [0, 1].map(|k| ((hi[k] - lo[k]) / cell).floor() as usize + 1);
        let mut g = RayGrid {
            d,
            u,
            v,
            lo,
            cell,
            n,
            bins: vec![Vec::new(); n[0] * n[1]],
        };
        for (ti, t) in s.tris.iter().enumerate() {
            let q = [proj(s.points[t[0]]), proj(s.points[t[1]]), proj(s.points[t[2]])];
            let mut a = [0usize; 2];
            let mut b = [0usize; 2];
            for k in 0..2 {
                let l = q.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min);
                let h = q.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max);
                a[k] = (((l - lo[k]) / cell).floor().max(0.0) as usize).min(n[k] - 1);
                b[k] = (((h - lo[k]) / cell).floor().max(0.0) as usize).min(n[k] - 1);
            }
            for i in a[0]..=b[0] {
                for j in a[1]..=b[1] {
                    g.bins[i + n[0] * j].push(ti as u32);
                }
            }
        }
        g
    }

    /// Distance to the surface if it is at most `cutoff`, else `None`.
    pub fn dist_within(&self, p: Vec3, cutoff: f64) -> Option<f64> {
        let g = &self.grid;
        let mut a = [0usize; 3];
        let mut b = [0usize; 3];
        for k in 0..3 {
            let l = ((p[k] - cutoff - g.lo[k]) / g.cell).floor();
            let h = ((p[k] + cutoff - g.lo[k]) / g.cell).floor();
            if h < 0.0 || l > (g.n[k] - 1) as f64 {
                return None;
            }
            a[k] = l.max(0.0) as usize;
            b[k] = (h.max(0.0) as usize).min(g.n[k] - 1);
        }
        let mut best = f64::INFINITY;
        for i in a[0]..=b[0] {
            for j in a[1]..=b[1] {
                for k in a[2]..=b[2] {
                    for &ti in &g.bins[i + g.n[0] * (j + g.n[1] * k)] {
                        let t = self.s.tris[ti as usize];
                        let (q, _) =
                            closest_on_triangle(p, self.s.points[t[0]], self.s.points[t[1]], self.s.points[t[2]]);
                        best = best.min(vec3::norm(vec3::sub(p, q)));
                    }
                }
            }
        }
        (best <= cutoff).then_some(best)
    }

    fn parity(&self, r: &RayGrid, p: Vec3) -> bool {
        let (pu, pv) = (vec3::dot(p, r.u), vec3::dot(p, r.v));
        let (fi, fj) = ((pu - r.lo[0]) / r.cell, (pv - r.lo[1]) / r.cell);
        if fi < 0.0 || fj < 0.0 || fi >= r.n[0] as f64 || fj >= r.n[1] as f64 {
            return false;
        }
        let mut hits = 0usize;
        for &ti in &r.bins[fi as usize + r.n[0] * fj as usize] {
            let t = self.s.tris[ti as usize];
            let (a, b, c) = (self.s.points[t[0]], self.s.points[t[1]], self.s.points[t[2]]);
            let (e1, e2) = (vec3::sub(b, a), vec3::sub(c, a));
            let pvec = vec3::cross(r.d, e2);
            let det = vec3::dot(e1, pvec);
            if det.abs() < 1e-300 {
                continue;
            }
            let inv = 1.0 / det;
            let tvec = vec3::sub(p, a);
            let u = vec3::dot(tvec, pvec) * inv;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let qvec = vec3::cross(tvec, e1);
            let v = vec3::dot(r.d, qvec) * inv;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            if vec3::dot(e2, qvec) * inv > 0.0 {
                hits += 1;
            }
        }
        hits % 2 == 1
    }

    /// Whether `p` is inside the closed surface (majority of three generic ray directions, parity of the crossings).
    pub fn inside(&self, p: Vec3) -> bool {
        self.rays.iter().filter(|r| self.parity(r, p)).count() >= 2
    }

    /// Whether the three ray directions agree on `p`.
    pub fn rays_agree(&self, p: Vec3) -> bool {
        let c = self.rays.iter().filter(|r| self.parity(r, p)).count();
        c == 0 || c == 3
    }
}

// ------------------------------------------------------------------------------------------------------------
// layer rules
// ------------------------------------------------------------------------------------------------------------

/// How the heights of a layer column (total thickness `t`, local lateral size `s`) are chosen. Heights are listed from the wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerRule {
    /// `layers` equal heights `t / layers`.
    Uniform {
        /// Number of layers.
        layers: usize,
    },
    /// First height `s / aspect`, growth ratio from the total thickness.
    AspectRatio {
        /// Local surface size over first-layer height.
        aspect: f64,
        /// Number of layers (>= 2).
        layers: usize,
    },
    /// Last height (against the core) `last_ratio * s`; growth ratio and first height from the total thickness.
    LastRatio {
        /// Last-layer height over local surface size.
        last_ratio: f64,
        /// Number of layers (>= 2).
        layers: usize,
    },
}

impl LayerRule {
    /// Number of layers.
    pub fn layers(&self) -> usize {
        match *self {
            LayerRule::Uniform { layers }
            | LayerRule::AspectRatio { layers, .. }
            | LayerRule::LastRatio { layers, .. } => layers,
        }
    }
}

/// Sum of `q^i`, `i = 0..n`.
fn geo_sum(q: f64, n: usize) -> f64 {
    let mut s = 0.0;
    let mut p = 1.0;
    for _ in 0..n {
        s += p;
        p *= q;
    }
    s
}

/// Bisection (in log of the argument) of an increasing function `f(x) = target` on `[lo, hi]`.
fn bisect_log(f: impl Fn(f64) -> f64, target: f64, mut lo: f64, mut hi: f64) -> Option<f64> {
    if !(f(lo) <= target && f(hi) >= target) {
        return None;
    }
    for _ in 0..200 {
        let mid = (lo * hi).sqrt();
        if f(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some((lo * hi).sqrt())
}

/// Heights of a column, from the wall outward, summing to `t` (rescaled at round-off level). Formulas in ADR 0047 D6.
pub fn layer_heights(rule: LayerRule, t: f64, s: f64) -> Result<Vec<f64>> {
    if !(t > 0.0 && s > 0.0) {
        return bad("layer column needs a positive thickness and surface size");
    }
    let n = rule.layers();
    if n == 0 {
        return bad("layer rule needs at least one layer");
    }
    let mut h: Vec<f64> = match rule {
        LayerRule::Uniform { layers } => vec![t / layers as f64; layers],
        LayerRule::AspectRatio { aspect, layers } => {
            if layers < 2 || (aspect.is_nan() || aspect <= 0.0) {
                return bad("aspect-ratio rule needs a positive aspect ratio and at least two layers");
            }
            let h1 = s / aspect;
            if t <= h1 * (1.0 + 1e-12) {
                return bad(format!(
                    "aspect-ratio rule infeasible: total thickness {t:.6e} does not exceed the first-layer height {h1:.6e}"
                ));
            }
            let g = bisect_log(|g| h1 * geo_sum(g, layers), t, 1e-3, 1e3)
                .ok_or_else(|| Error::Mesh("aspect-ratio rule: growth ratio outside [1e-3, 1e3]".into()))?;
            (0..layers).map(|i| h1 * g.powi(i as i32)).collect()
        }
        LayerRule::LastRatio { last_ratio, layers } => {
            if layers < 2 || (last_ratio.is_nan() || last_ratio <= 0.0) {
                return bad("last-ratio rule needs a positive last ratio and at least two layers");
            }
            let hn = last_ratio * s;
            if t <= hn * (1.0 + 1e-12) {
                return bad(format!(
                    "last-ratio rule infeasible: total thickness {t:.6e} does not exceed the last-layer height {hn:.6e}"
                ));
            }
            let q = bisect_log(|q| geo_sum(q, layers), t / hn, 1e-6, 1e6)
                .ok_or_else(|| Error::Mesh("last-ratio rule: ratio outside [1e-6, 1e6]".into()))?;
            // h_i = h_N q^(N-i), i = 1..N from the wall
            (0..layers).map(|i| hn * q.powi((layers - 1 - i) as i32)).collect()
        }
    };
    let sum: f64 = h.iter().sum();
    let k = t / sum;
    for x in &mut h {
        *x *= k;
    }
    Ok(h)
}

// ------------------------------------------------------------------------------------------------------------
// octree
// ------------------------------------------------------------------------------------------------------------

type L3 = [i64; 3];

/// The fluid region to fill.
#[derive(Clone, Copy)]
pub enum Region<'a> {
    /// The whole box.
    Domain,
    /// Outside a closed body surface (fluid around a body).
    ExteriorOf(&'a TriSurface),
    /// Inside a closed surface (fluid in a vessel).
    InteriorOf(&'a TriSurface),
}

/// Fill specification.
pub struct FillSpec<'a> {
    /// Box corners.
    pub lo: Vec3,
    /// Box corners.
    pub hi: Vec3,
    /// Root cells per axis (the three spacings must agree).
    pub roots: [usize; 3],
    /// Finest level (a root has level 0).
    pub max_level: u8,
    /// Leaves within `band * h_finest` of the surface are refined to the finest level.
    pub band: f64,
    /// Smallest allowed corner distance to the surface, in finest cell sizes (0: corners on the surface are allowed).
    pub margin: f64,
    /// Region.
    pub region: Region<'a>,
    /// Layer shell between the staircase and the surface; `None` leaves the staircase as the `body` patch.
    pub layers: Option<LayerRule>,
    /// Names and kinds of the box faces xmin, xmax, ymin, ymax, zmin, zmax (equal names are merged).
    pub box_patches: [(String, PatchKind); 6],
    /// Name and kind of the body patch.
    pub body_patch: (String, PatchKind),
}

impl FillSpec<'_> {
    /// Default box patch names `xmin..zmax` (walls) and the body patch `body` (wall).
    pub fn default_patches() -> ([(String, PatchKind); 6], (String, PatchKind)) {
        let n = |s: &str| (s.to_string(), PatchKind::Wall);
        (
            [n("xmin"), n("xmax"), n("ymin"), n("ymax"), n("zmin"), n("zmax")],
            n("body"),
        )
    }
}

/// One layer column.
#[derive(Debug, Clone)]
pub struct Column {
    /// Mesh point ids from the core node (index 0) to the wall point (index `layers`).
    pub nodes: Vec<usize>,
    /// Total thickness (distance from the core node to the surface).
    pub thickness: f64,
    /// Local lateral size.
    pub lateral: f64,
    /// Layer heights from the wall.
    pub heights: Vec<f64>,
}

/// What the fill did.
#[derive(Debug, Clone, Default)]
pub struct FillReport {
    /// Leaves after balancing.
    pub n_leaves: usize,
    /// Leaves per level.
    pub leaves_per_level: Vec<usize>,
    /// Core (octree) cells and layer-shell cells.
    pub n_core: usize,
    /// Layer cells.
    pub n_layer: usize,
    /// Hexahedral cells (six quadrilateral faces).
    pub n_hex: usize,
    /// Core cells that are not hexahedra (transition cells).
    pub n_poly_core: usize,
    /// Number of interface polygons and how many of them are quadrilaterals.
    pub n_interface: usize,
    /// Interface polygons with four points.
    pub n_interface_quads: usize,
    /// Layer columns (empty without layers).
    pub columns: Vec<Column>,
    /// Finest cell size.
    pub h_finest: f64,
    /// Whether every pair of 26-neighbour leaves differs by at most one level.
    pub balanced: bool,
}

struct Tree {
    max: u8,
    lo: Vec3,
    unit: f64,
    roots: [i64; 3],
    leaves: Vec<(u8, L3)>,
    map: HashMap<(u8, L3), usize>,
}

impl Tree {
    fn size(&self, l: u8) -> i64 {
        1i64 << (self.max - l)
    }
    fn extent(&self, l: u8) -> L3 {
        [self.roots[0] << l, self.roots[1] << l, self.roots[2] << l]
    }
    fn pos(&self, p: L3) -> Vec3 {
        [
            self.lo[0] + p[0] as f64 * self.unit,
            self.lo[1] + p[1] as f64 * self.unit,
            self.lo[2] + p[2] as f64 * self.unit,
        ]
    }
    fn rebuild(&mut self) {
        self.map = self.leaves.iter().enumerate().map(|(i, &k)| (k, i)).collect();
    }
    /// Leaf covering the cell `(l, ijk)` at level <= `l`, if any.
    fn find(&self, l: u8, ijk: L3) -> Option<usize> {
        for ll in (0..=l).rev() {
            let s = l - ll;
            if let Some(&i) = self.map.get(&(ll, [ijk[0] >> s, ijk[1] >> s, ijk[2] >> s])) {
                return Some(i);
            }
        }
        None
    }
    fn split(&mut self, which: &[usize]) {
        let mut flag = vec![false; self.leaves.len()];
        for &i in which {
            flag[i] = true;
        }
        let mut out = Vec::with_capacity(self.leaves.len() + 7 * which.len());
        for (i, &(l, c)) in self.leaves.iter().enumerate() {
            if flag[i] {
                for b in 0..8i64 {
                    out.push((
                        l + 1,
                        [2 * c[0] + (b & 1), 2 * c[1] + ((b >> 1) & 1), 2 * c[2] + ((b >> 2) & 1)],
                    ));
                }
            } else {
                out.push((l, c));
            }
        }
        out.sort_unstable();
        self.leaves = out;
        self.rebuild();
    }
    fn corner(&self, i: usize, b: usize) -> L3 {
        let (l, c) = self.leaves[i];
        let s = self.size(l);
        [
            (c[0] + (b & 1) as i64) * s,
            (c[1] + ((b >> 1) & 1) as i64) * s,
            (c[2] + ((b >> 2) & 1) as i64) * s,
        ]
    }
}

/// Quadrilateral of edge `size` in the plane normal to `ax` whose lowest corner is `base`.
fn face_loop(base: L3, size: i64, ax: usize) -> [L3; 4] {
    let (t1, t2) = ((ax + 1) % 3, (ax + 2) % 3);
    let mut p = [base; 4];
    p[1][t1] += size;
    p[2][t1] += size;
    p[2][t2] += size;
    p[3][t2] += size;
    p
}

enum FaceKind {
    Domain(usize),
    Interface,
}

/// Insert registered lattice nodes lying on the edges of a polygon (recursively halving each axis-aligned edge).
fn with_hanging(poly: &[L3], reg: &HashMap<L3, usize>) -> Vec<L3> {
    fn edge(a: L3, b: L3, reg: &HashMap<L3, usize>, out: &mut Vec<L3>) {
        let mid = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
        let even = (0..3).all(|k| (a[k] + b[k]) % 2 == 0);
        if even && mid != a && mid != b && reg.contains_key(&mid) {
            edge(a, mid, reg, out);
            out.push(mid);
            edge(mid, b, reg, out);
        }
    }
    let mut out = Vec::new();
    for i in 0..poly.len() {
        out.push(poly[i]);
        edge(poly[i], poly[(i + 1) % poly.len()], reg, &mut out);
    }
    out
}

/// Fill the box (see the module documentation). Returns the mesh and a report.
pub fn fill(spec: &FillSpec<'_>) -> Result<(Mesh, FillReport)> {
    let h0 = (spec.hi[0] - spec.lo[0]) / spec.roots[0] as f64;
    for k in 0..3 {
        if spec.roots[k] == 0 || spec.hi[k].is_nan() || spec.lo[k].is_nan() || spec.hi[k] <= spec.lo[k] {
            return bad("octree fill needs a non-empty box and at least one root cell per axis");
        }
        let hk = (spec.hi[k] - spec.lo[k]) / spec.roots[k] as f64;
        if (hk - h0).abs() > 1e-9 * h0 {
            return bad(format!(
                "octree roots must be cubes: spacings {h0:.6e} and {hk:.6e} differ (adjust roots or the box)"
            ));
        }
    }
    if spec.max_level > 12 {
        return bad("octree max_level above 12 is not supported");
    }
    let surface = match spec.region {
        Region::Domain => None,
        Region::ExteriorOf(s) | Region::InteriorOf(s) => Some(s),
    };
    if spec.layers.is_some() && surface.is_none() {
        return bad("layers need a region bounded by a surface");
    }
    let interior = matches!(spec.region, Region::InteriorOf(_));
    let index = surface.map(SurfaceIndex::new);
    let query = surface.map(SurfaceQuery::new);
    let m = 1i64 << spec.max_level;
    let unit = h0 / m as f64;
    let h_f = h0 / (1u64 << spec.max_level) as f64;
    let roots = [spec.roots[0] as i64, spec.roots[1] as i64, spec.roots[2] as i64];
    let mut tree = Tree {
        max: spec.max_level,
        lo: spec.lo,
        unit,
        roots,
        leaves: Vec::new(),
        map: HashMap::new(),
    };
    for k in 0..roots[2] {
        for j in 0..roots[1] {
            for i in 0..roots[0] {
                tree.leaves.push((0, [i, j, k]));
            }
        }
    }
    tree.leaves.sort_unstable();
    tree.rebuild();
    let tol = 1e-9 * h0 * roots.iter().copied().max().unwrap_or(1) as f64;

    // refinement towards the surface
    if let Some(ix) = &index {
        loop {
            let mut which = Vec::new();
            for (i, &(l, c)) in tree.leaves.iter().enumerate() {
                if l >= spec.max_level {
                    continue;
                }
                let s = tree.size(l);
                let centre = tree.pos([c[0] * s + s / 2, c[1] * s + s / 2, c[2] * s + s / 2]);
                let half_diag = 0.866_025_403_784 * s as f64 * unit;
                if ix.dist_within(centre, spec.band * h_f + half_diag).is_some() {
                    which.push(i);
                }
            }
            if which.is_empty() {
                break;
            }
            tree.split(&which);
        }
    }
    // 2:1 balance over the 26-neighbourhood
    loop {
        let mut which = Vec::new();
        for &(l, c) in &tree.leaves {
            if l < 2 {
                continue;
            }
            let ext = tree.extent(l);
            for dx in -1i64..=1 {
                for dy in -1i64..=1 {
                    for dz in -1i64..=1 {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }
                        let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                        if (0..3).any(|k| n[k] < 0 || n[k] >= ext[k]) {
                            continue;
                        }
                        if let Some(ci) = tree.find(l, n) {
                            if tree.leaves[ci].0 + 1 < l {
                                which.push(ci);
                            }
                        }
                    }
                }
            }
        }
        if which.is_empty() {
            break;
        }
        which.sort_unstable();
        which.dedup();
        tree.split(&which);
    }
    let n_leaves = tree.leaves.len();
    let mut report = FillReport {
        n_leaves,
        leaves_per_level: (0..=spec.max_level)
            .map(|l| tree.leaves.iter().filter(|x| x.0 == l).count())
            .collect(),
        h_finest: h_f,
        ..Default::default()
    };
    // balanced check (reported, also used by the tests)
    report.balanced = true;
    for &(l, c) in &tree.leaves {
        let ext = tree.extent(l);
        for dx in -1i64..=1 {
            for dy in -1i64..=1 {
                for dz in -1i64..=1 {
                    let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                    if (dx, dy, dz) == (0, 0, 0) || (0..3).any(|k| n[k] < 0 || n[k] >= ext[k]) {
                        continue;
                    }
                    if let Some(ci) = tree.find(l, n) {
                        if tree.leaves[ci].0 + 1 < l {
                            report.balanced = false;
                        }
                    }
                }
            }
        }
    }

    // keep rule
    let margin = spec.margin * h_f;
    let mut node_ok: HashMap<L3, bool> = HashMap::new();
    let ok = |p: L3, node_ok: &mut HashMap<L3, bool>| -> bool {
        if let Some(&v) = node_ok.get(&p) {
            return v;
        }
        let v = match &index {
            None => true,
            Some(ix) => {
                let x = tree.pos(p);
                let fluid = ix.inside(x) == interior;
                if margin > 0.0 {
                    ix.dist_within(x, (margin - tol).max(0.0)).is_none() && fluid
                } else {
                    ix.dist_within(x, tol).is_some() || fluid
                }
            }
        };
        node_ok.insert(p, v);
        v
    };
    let mut kept = vec![false; n_leaves];
    for (i, k) in kept.iter_mut().enumerate() {
        *k = (0..8).all(|b| ok(tree.corner(i, b), &mut node_ok));
    }
    if !kept.iter().any(|&k| k) {
        return bad("octree fill kept no cell (region empty at this resolution)");
    }

    // faces
    let mut cell_loops: Vec<Vec<Vec<L3>>> = Vec::new();
    let mut boundary: Vec<(Vec<L3>, FaceKind)> = Vec::new();
    let mut kept_ids = Vec::new();
    for (i, &(l, c)) in tree.leaves.iter().enumerate() {
        if !kept[i] {
            continue;
        }
        kept_ids.push(i);
        let size = tree.size(l);
        let origin = [c[0] * size, c[1] * size, c[2] * size];
        let ext = tree.extent(l);
        let mut loops: Vec<Vec<L3>> = Vec::new();
        for d in 0..6usize {
            let (ax, side) = (d / 2, (d % 2) as i64);
            let sgn = 2 * side - 1;
            let mut face_origin = origin;
            face_origin[ax] += side * size;
            let whole = face_loop(face_origin, size, ax).to_vec();
            let mut n = c;
            n[ax] += sgn;
            if n[ax] < 0 || n[ax] >= ext[ax] {
                boundary.push((whole.clone(), FaceKind::Domain(d)));
                loops.push(whole);
                continue;
            }
            match tree.find(l, n) {
                Some(ci) => {
                    if !kept[ci] {
                        boundary.push((whole.clone(), FaceKind::Interface));
                    }
                    loops.push(whole);
                }
                None => {
                    // finer neighbours: the four children of n that touch this face
                    let (t1, t2) = ((ax + 1) % 3, (ax + 2) % 3);
                    let mut kids = Vec::new();
                    for b1 in 0..2i64 {
                        for b2 in 0..2i64 {
                            let mut ch = [2 * n[0], 2 * n[1], 2 * n[2]];
                            ch[ax] += if sgn > 0 { 0 } else { 1 };
                            ch[t1] += b1;
                            ch[t2] += b2;
                            let ci = tree.find(l + 1, ch).filter(|&ci| tree.leaves[ci].0 == l + 1);
                            let Some(ci) = ci else {
                                return bad("octree is not 2:1 balanced across a face (internal error)");
                            };
                            kids.push((b1, b2, kept[ci]));
                        }
                    }
                    if kids.iter().any(|k| k.2) {
                        let half = size / 2;
                        for (b1, b2, kp) in kids {
                            let mut o = face_origin;
                            o[t1] += b1 * half;
                            o[t2] += b2 * half;
                            let sub = face_loop(o, half, ax).to_vec();
                            if !kp {
                                boundary.push((sub.clone(), FaceKind::Interface));
                            }
                            loops.push(sub);
                        }
                    } else {
                        boundary.push((whole.clone(), FaceKind::Interface));
                        loops.push(whole);
                    }
                }
            }
        }
        cell_loops.push(loops);
    }

    // hanging nodes
    let mut reg: HashMap<L3, usize> = HashMap::new();
    for loops in &cell_loops {
        for lp in loops {
            for &p in lp {
                reg.insert(p, 0);
            }
        }
    }
    for (lp, _) in &boundary {
        for &p in lp {
            reg.insert(p, 0);
        }
    }
    let mut points: Vec<Vec3> = Vec::new();
    let mut ids: HashMap<L3, usize> = HashMap::new();
    let mut id_of = |p: L3, points: &mut Vec<Vec3>| -> usize {
        *ids.entry(p).or_insert_with(|| {
            points.push(tree.pos(p));
            points.len() - 1
        })
    };
    let mut cells: Vec<Vec<Vec<usize>>> = Vec::new();
    for loops in &cell_loops {
        let mut cell = Vec::new();
        for lp in loops {
            let h = with_hanging(lp, &reg);
            cell.push(h.iter().map(|&p| id_of(p, &mut points)).collect::<Vec<_>>());
        }
        report.n_core += 1;
        if cell.len() == 6 && cell.iter().all(|f| f.len() == 4) {
            report.n_hex += 1;
        } else {
            report.n_poly_core += 1;
        }
        cells.push(cell);
    }
    let mut box_faces: Vec<Vec<Vec<usize>>> = vec![Vec::new(); 6];
    let mut interface: Vec<Vec<usize>> = Vec::new();
    for (lp, kind) in &boundary {
        let h = with_hanging(lp, &reg);
        let v: Vec<usize> = h.iter().map(|&p| id_of(p, &mut points)).collect();
        match kind {
            FaceKind::Domain(d) => box_faces[*d].push(v),
            FaceKind::Interface => interface.push(v),
        }
    }
    report.n_interface = interface.len();
    report.n_interface_quads = interface.iter().filter(|p| p.len() == 4).count();

    // layer shell
    let mut cap: Vec<Vec<usize>> = Vec::new();
    let mut zone_layer = Vec::new();
    if let Some(rule) = spec.layers {
        let (Some(q), true) = (&query, surface.is_some()) else {
            return bad("layers need a surface");
        };
        let nl = rule.layers();
        // lateral size per node: mean of sqrt(polygon area) over the incident interface polygons
        let mut lat: HashMap<usize, (f64, f64)> = HashMap::new();
        for poly in &interface {
            let mut a = vec3::ZERO;
            for i in 0..poly.len() {
                a = vec3::add(a, vec3::cross(points[poly[i]], points[poly[(i + 1) % poly.len()]]));
            }
            let area = 0.5 * vec3::norm(a);
            for &p in poly {
                let e = lat.entry(p).or_insert((0.0, 0.0));
                e.0 += area.sqrt();
                e.1 += 1.0;
            }
        }
        let mut cols: HashMap<usize, usize> = HashMap::new();
        let mut order: Vec<usize> = lat.keys().copied().collect();
        order.sort_unstable();
        for &p in &order {
            let x = points[p];
            let (sp, _, _) = q.closest(x);
            let t = vec3::norm(vec3::sub(x, sp));
            let s = lat[&p].0 / lat[&p].1;
            let heights = layer_heights(rule, t, s).map_err(|e| Error::Mesh(format!("layer column at {x:?}: {e}")))?;
            // node k from the core: distance from the wall = sum of the wall-first heights 1..=N-k
            let mut nodes = vec![p];
            for k in 1..=nl {
                let from_wall: f64 = heights.iter().take(nl - k).sum();
                let frac = from_wall / t;
                let pos = vec3::add(sp, vec3::scale(vec3::sub(x, sp), frac));
                if k == nl {
                    // the wall point is the surface point itself
                    points.push(sp);
                } else {
                    points.push(pos);
                }
                nodes.push(points.len() - 1);
            }
            cols.insert(p, report.columns.len());
            report.columns.push(Column {
                nodes,
                thickness: t,
                lateral: s,
                heights,
            });
        }
        for poly in &interface {
            for k in 0..nl {
                let lo: Vec<usize> = poly.iter().map(|p| report.columns[cols[p]].nodes[k]).collect();
                let hi: Vec<usize> = poly.iter().map(|p| report.columns[cols[p]].nodes[k + 1]).collect();
                let mut faces = vec![lo.clone(), hi.clone()];
                for i in 0..poly.len() {
                    let j = (i + 1) % poly.len();
                    faces.push(vec![lo[i], lo[j], hi[j], hi[i]]);
                }
                if k + 1 == nl {
                    cap.push(hi);
                }
                zone_layer.push(cells.len());
                if faces.len() == 6 && faces.iter().all(|f| f.len() == 4) {
                    report.n_hex += 1;
                }
                cells.push(faces);
                report.n_layer += 1;
            }
        }
    } else {
        cap = std::mem::take(&mut interface);
    }

    // patches (equal names merged)
    let mut specs: Vec<BoundarySpec> = Vec::new();
    let mut push_patch = |name: &str, kind: &PatchKind, faces: Vec<Vec<usize>>| -> Result<()> {
        if faces.is_empty() {
            return Ok(());
        }
        if let Some(s) = specs.iter_mut().find(|s| s.name == name) {
            if s.kind != *kind {
                return bad(format!("patch '{name}' is given two different kinds"));
            }
            s.faces.extend(faces);
        } else {
            specs.push(BoundarySpec {
                name: name.to_string(),
                kind: kind.clone(),
                faces,
            });
        }
        Ok(())
    };
    for (d, faces) in box_faces.into_iter().enumerate() {
        push_patch(&spec.box_patches[d].0, &spec.box_patches[d].1, faces)?;
    }
    push_patch(&spec.body_patch.0, &spec.body_patch.1, cap)?;
    let mut mesh = match Mesh::from_cell_faces(points.clone(), &cells, &specs) {
        Ok(m) => m,
        Err(e) => {
            // locate a degenerate face for the message
            for (ci, cell) in cells.iter().enumerate() {
                for f in cell {
                    let (fc, sf) = crate::mesh::polygon_geometry(&points, f);
                    let mut est = vec3::ZERO;
                    let mut cnt = 0.0;
                    for g in cell {
                        for &i in g {
                            est = vec3::add(est, points[i]);
                            cnt += 1.0;
                        }
                    }
                    est = vec3::scale(est, 1.0 / cnt);
                    if vec3::dot(sf, vec3::sub(fc, est)) == 0.0 {
                        let pts: Vec<Vec3> = f.iter().map(|&i| points[i]).collect();
                        let all: Vec<usize> = cell.iter().map(|g| g.len()).collect();
                        return bad(format!(
                            "{e}; cell {ci} (face sizes {all:?}): face {pts:?} sf {sf:?} fc {fc:?} est {est:?}"
                        ));
                    }
                }
            }
            return Err(e);
        }
    };
    if !zone_layer.is_empty() {
        mesh.add_cell_zone("layers", zone_layer)?;
    }
    Ok((mesh, report))
}
