//! Triangulated surfaces: generators, diagnostics and repair (F-232, ADR 0030 part A).
//!
//! Triangles are counter-clockwise seen from outside, so the outward normal is `(b - a) x (c - a)` and the divergence
//! theorem gives the enclosed volume as `1/6 sum a . (b x c)` for a closed component. All iteration is over sorted keys
//! so results do not depend on hash order.

use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, Vec3};
use std::collections::{BTreeMap, HashMap};

/// An indexed triangle surface with one integer tag (region id) per triangle.
#[derive(Debug, Clone, PartialEq)]
pub struct TriSurface {
    /// Vertex coordinates.
    pub points: Vec<Vec3>,
    /// Triangles as vertex ids, counter-clockwise seen from outside.
    pub tris: Vec<[usize; 3]>,
    /// Region tag of every triangle.
    pub tags: Vec<u32>,
}

impl TriSurface {
    /// Surface with all tags 0.
    pub fn new(points: Vec<Vec3>, tris: Vec<[usize; 3]>) -> Self {
        let tags = vec![0; tris.len()];
        TriSurface { points, tris, tags }
    }

    /// Weld a triangle soup. `tol = 0` welds bitwise-equal vertices (`-0.0 == 0.0`); `tol > 0` welds vertices that fall
    /// in the same cell of a grid of that size (vertices closer than the cell size may or may not weld; deterministic).
    pub fn from_soup(soup: &[[Vec3; 3]], tol: f64) -> Self {
        let mut map: HashMap<[i64; 3], usize> = HashMap::new();
        let mut map_bits: HashMap<[u64; 3], usize> = HashMap::new();
        let mut points: Vec<Vec3> = Vec::new();
        let mut tris = Vec::with_capacity(soup.len());
        for t in soup {
            let mut ids = [0usize; 3];
            for (k, v) in t.iter().enumerate() {
                ids[k] = if tol > 0.0 {
                    let key = [
                        (v[0] / tol).round() as i64,
                        (v[1] / tol).round() as i64,
                        (v[2] / tol).round() as i64,
                    ];
                    *map.entry(key).or_insert_with(|| {
                        points.push(*v);
                        points.len() - 1
                    })
                } else {
                    let key = [(v[0] + 0.0).to_bits(), (v[1] + 0.0).to_bits(), (v[2] + 0.0).to_bits()];
                    *map_bits.entry(key).or_insert_with(|| {
                        points.push(*v);
                        points.len() - 1
                    })
                };
            }
            tris.push(ids);
        }
        TriSurface::new(points, tris)
    }

    /// Number of triangles.
    pub fn n_tris(&self) -> usize {
        self.tris.len()
    }

    /// Vertices of triangle `i`.
    pub fn tri_points(&self, i: usize) -> [Vec3; 3] {
        let t = self.tris[i];
        [self.points[t[0]], self.points[t[1]], self.points[t[2]]]
    }

    /// Area vector of triangle `i` (half the cross product).
    pub fn tri_area_vector(&self, i: usize) -> Vec3 {
        let [a, b, c] = self.tri_points(i);
        scale(cross(sub(b, a), sub(c, a)), 0.5)
    }

    /// Total area.
    pub fn area(&self) -> f64 {
        (0..self.tris.len()).map(|i| norm(self.tri_area_vector(i))).sum()
    }

    /// Volume enclosed by the surface by the divergence theorem (signed: positive when outward-oriented).
    pub fn enclosed_volume(&self) -> f64 {
        self.tris
            .iter()
            .map(|t| dot(self.points[t[0]], cross(self.points[t[1]], self.points[t[2]])) / 6.0)
            .sum()
    }

    /// Append another surface (its tags are kept).
    pub fn append(&mut self, other: &TriSurface) {
        let off = self.points.len();
        self.points.extend_from_slice(&other.points);
        self.tris
            .extend(other.tris.iter().map(|t| [t[0] + off, t[1] + off, t[2] + off]));
        self.tags.extend_from_slice(&other.tags);
    }

    /// Set every tag.
    pub fn set_tag(&mut self, tag: u32) {
        self.tags.iter_mut().for_each(|t| *t = tag);
    }

    /// Reverse the orientation of every triangle.
    pub fn flip_all(&mut self) {
        self.tris.iter_mut().for_each(|t| t.swap(1, 2));
    }

    /// Number of used vertices, distinct edges and the Euler characteristic `V - E + F` (used vertices only).
    pub fn euler(&self) -> (usize, usize, i64) {
        let mut used = vec![false; self.points.len()];
        for t in &self.tris {
            for &v in t {
                used[v] = true;
            }
        }
        let v = used.iter().filter(|&&u| u).count();
        let e = edge_groups(&self.tris).len();
        (v, e, v as i64 - e as i64 + self.tris.len() as i64)
    }
}

/// One undirected edge with the triangles using it: `(triangle, forward)` where `forward` means the triangle traverses the
/// edge from the smaller to the larger vertex id.
pub type EdgeUse = Vec<(usize, bool)>;

/// All undirected edges (sorted by `(min, max)`) with their uses.
pub fn edge_groups(tris: &[[usize; 3]]) -> BTreeMap<(usize, usize), EdgeUse> {
    let mut m: BTreeMap<(usize, usize), EdgeUse> = BTreeMap::new();
    for (i, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            if a == b {
                continue;
            }
            m.entry((a.min(b), a.max(b))).or_default().push((i, a < b));
        }
    }
    m
}

// ------------------------------------------------------------------------------------------------------------
// generators
// ------------------------------------------------------------------------------------------------------------

fn orient_outward(s: &mut TriSurface) {
    if s.enclosed_volume() < 0.0 {
        s.flip_all();
    }
}

/// Icosphere of radius `r`: regular icosahedron subdivided `level` times (midpoints projected to the sphere).
pub fn icosphere(r: f64, level: usize) -> TriSurface {
    let phi = (1.0 + 5f64.sqrt()) / 2.0;
    let mut pts: Vec<Vec3> = vec![
        [-1.0, phi, 0.0],
        [1.0, phi, 0.0],
        [-1.0, -phi, 0.0],
        [1.0, -phi, 0.0],
        [0.0, -1.0, phi],
        [0.0, 1.0, phi],
        [0.0, -1.0, -phi],
        [0.0, 1.0, -phi],
        [phi, 0.0, -1.0],
        [phi, 0.0, 1.0],
        [-phi, 0.0, -1.0],
        [-phi, 0.0, 1.0],
    ];
    for p in pts.iter_mut() {
        let n = norm(*p);
        *p = scale(*p, r / n);
    }
    let mut tris: Vec<[usize; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    for _ in 0..level {
        let mut cache: BTreeMap<(usize, usize), usize> = BTreeMap::new();
        let mut next = Vec::with_capacity(tris.len() * 4);
        let mut mid = |a: usize, b: usize, pts: &mut Vec<Vec3>| -> usize {
            let key = (a.min(b), a.max(b));
            if let Some(&m) = cache.get(&key) {
                return m;
            }
            let m = scale(add(pts[a], pts[b]), 0.5);
            let n = norm(m);
            pts.push(scale(m, r / n));
            cache.insert(key, pts.len() - 1);
            pts.len() - 1
        };
        for t in &tris {
            let (a, b, c) = (t[0], t[1], t[2]);
            let ab = mid(a, b, &mut pts);
            let bc = mid(b, c, &mut pts);
            let ca = mid(c, a, &mut pts);
            next.push([a, ab, ca]);
            next.push([b, bc, ab]);
            next.push([c, ca, bc]);
            next.push([ab, bc, ca]);
        }
        tris = next;
    }
    let mut s = TriSurface::new(pts, tris);
    orient_outward(&mut s);
    s
}

/// Axis-aligned box surface with `n[d]` quads along axis `d`, every quad split by one diagonal; outward oriented.
pub fn box_surface(lo: Vec3, hi: Vec3, n: [usize; 3]) -> TriSurface {
    let mut id: BTreeMap<[usize; 3], usize> = BTreeMap::new();
    let mut pts: Vec<Vec3> = Vec::new();
    let mut get = |i: usize, j: usize, k: usize, pts: &mut Vec<Vec3>| -> usize {
        *id.entry([i, j, k]).or_insert_with(|| {
            let f = |d: usize, q: usize| lo[d] + (hi[d] - lo[d]) * (q as f64) / (n[d] as f64);
            pts.push([f(0, i), f(1, j), f(2, k)]);
            pts.len() - 1
        })
    };
    let mut tris = Vec::new();
    // faces: fixed axis a at index 0 or n[a]; the other two axes b, c
    for a in 0..3 {
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        for side in 0..2 {
            let fixed = if side == 0 { 0 } else { n[a] };
            for ib in 0..n[b] {
                for ic in 0..n[c] {
                    let mut q = [[0usize; 3]; 4];
                    for (m, (db, dc)) in [(0, 0), (1, 0), (1, 1), (0, 1)].iter().enumerate() {
                        q[m][a] = fixed;
                        q[m][b] = ib + db;
                        q[m][c] = ic + dc;
                    }
                    let v: Vec<usize> = q.iter().map(|g| get(g[0], g[1], g[2], &mut pts)).collect();
                    // (b, c, a) is a right-handed frame, so the loop b -> c has normal +a
                    let mut t1 = [v[0], v[1], v[2]];
                    let mut t2 = [v[0], v[2], v[3]];
                    if side == 0 {
                        t1.swap(1, 2);
                        t2.swap(1, 2);
                    }
                    tris.push(t1);
                    tris.push(t2);
                }
            }
        }
    }
    let mut s = TriSurface::new(pts, tris);
    orient_outward(&mut s);
    s
}

/// Cube of the given side centred at the origin with `n x n` quads per face.
pub fn cube(side: f64, n: usize) -> TriSurface {
    let h = side / 2.0;
    box_surface([-h; 3], [h; 3], [n; 3])
}

/// Torus about the z axis: major radius `big_r`, minor radius `r`, `nu` steps around, `nv` around the tube.
pub fn torus(big_r: f64, r: f64, nu: usize, nv: usize) -> TriSurface {
    let tp = 2.0 * std::f64::consts::PI;
    let mut pts = Vec::with_capacity(nu * nv);
    for i in 0..nu {
        let u = tp * i as f64 / nu as f64;
        for j in 0..nv {
            let v = tp * j as f64 / nv as f64;
            pts.push([
                (big_r + r * v.cos()) * u.cos(),
                (big_r + r * v.cos()) * u.sin(),
                r * v.sin(),
            ]);
        }
    }
    let mut tris = Vec::new();
    for i in 0..nu {
        for j in 0..nv {
            let a = i * nv + j;
            let b = ((i + 1) % nu) * nv + j;
            let c = ((i + 1) % nu) * nv + (j + 1) % nv;
            let d = i * nv + (j + 1) % nv;
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    let mut s = TriSurface::new(pts, tris);
    orient_outward(&mut s);
    s
}

/// Closed cylinder about the z axis, `|z| <= h/2`, `nu` around, `nz` along, caps as fans about a centre vertex.
/// Outward oriented.
pub fn cylinder(r: f64, h: f64, nu: usize, nz: usize) -> TriSurface {
    let tp = 2.0 * std::f64::consts::PI;
    let mut pts = Vec::new();
    for k in 0..=nz {
        let z = -h / 2.0 + h * k as f64 / nz as f64;
        for i in 0..nu {
            let u = tp * i as f64 / nu as f64;
            pts.push([r * u.cos(), r * u.sin(), z]);
        }
    }
    let bot = pts.len();
    pts.push([0.0, 0.0, -h / 2.0]);
    let top = pts.len();
    pts.push([0.0, 0.0, h / 2.0]);
    let mut tris = Vec::new();
    for k in 0..nz {
        for i in 0..nu {
            let a = k * nu + i;
            let b = k * nu + (i + 1) % nu;
            let c = (k + 1) * nu + (i + 1) % nu;
            let d = (k + 1) * nu + i;
            tris.push([a, b, c]);
            tris.push([a, c, d]);
        }
    }
    for i in 0..nu {
        let j = (i + 1) % nu;
        tris.push([bot, j, i]);
        tris.push([top, nz * nu + i, nz * nu + j]);
    }
    let mut s = TriSurface::new(pts, tris);
    orient_outward(&mut s);
    s
}

// ------------------------------------------------------------------------------------------------------------
// diagnostics
// ------------------------------------------------------------------------------------------------------------

/// Thresholds for [`diagnose`].
#[derive(Debug, Clone, Copy)]
pub struct DiagOptions {
    /// A triangle with area below `area_tol * (longest edge)^2` is degenerate.
    pub area_tol: f64,
    /// A non-degenerate triangle with a smaller angle (degrees) is a sliver.
    pub min_angle_deg: f64,
    /// An edge shorter than this fraction of the mean edge length is small.
    pub small_edge: f64,
}

impl Default for DiagOptions {
    fn default() -> Self {
        DiagOptions {
            area_tol: 1e-12,
            min_angle_deg: 1.0,
            small_edge: 1e-3,
        }
    }
}

/// Result of [`diagnose`]; all counts are exact integers.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostics {
    /// Used vertices.
    pub n_points_used: usize,
    /// Triangles.
    pub n_tris: usize,
    /// Distinct edges.
    pub n_edges: usize,
    /// `V - E + F`.
    pub euler: i64,
    /// Edges used by exactly one triangle.
    pub free_edges: usize,
    /// Edges used by three or more triangles.
    pub nonmanifold_edges: usize,
    /// Vertices whose incident triangles form more than one edge-connected fan.
    pub multi_connected_vertices: usize,
    /// Edges used by two triangles that traverse them in the same direction.
    pub misoriented_edges: usize,
    /// Triangles that repeat the vertex triple of an earlier triangle.
    pub duplicate_faces: usize,
    /// Triangles with a repeated vertex id or (almost) zero area.
    pub degenerate_tris: usize,
    /// Non-degenerate triangles with a very small angle.
    pub sliver_tris: usize,
    /// Edges much shorter than the mean.
    pub small_edges: usize,
    /// Closed loops of free edges, each starting at its smallest vertex id and following the free half-edges.
    pub open_loops: Vec<Vec<usize>>,
    /// Free edges that could not be chained into a closed loop.
    pub unclosed_chains: usize,
    /// Edge-connected components.
    pub components: usize,
    /// Signed enclosed volume of the whole surface.
    pub enclosed_volume: f64,
    /// Total area.
    pub area: f64,
}

impl Diagnostics {
    /// True when the surface is a closed, consistently oriented 2-manifold.
    pub fn is_closed_manifold(&self) -> bool {
        self.free_edges == 0
            && self.nonmanifold_edges == 0
            && self.misoriented_edges == 0
            && self.multi_connected_vertices == 0
    }
}

fn is_degenerate(s: &TriSurface, i: usize, area_tol: f64) -> bool {
    let t = s.tris[i];
    if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
        return true;
    }
    let [a, b, c] = s.tri_points(i);
    let l = norm(sub(b, a)).max(norm(sub(c, b))).max(norm(sub(a, c)));
    norm(s.tri_area_vector(i)) <= area_tol * l * l
}

fn min_angle_deg(p: [Vec3; 3]) -> f64 {
    let mut m = 180.0f64;
    for k in 0..3 {
        let (a, b, c) = (p[k], p[(k + 1) % 3], p[(k + 2) % 3]);
        let (u, v) = (sub(b, a), sub(c, a));
        let ang = norm(cross(u, v)).atan2(dot(u, v)).to_degrees();
        m = m.min(ang);
    }
    m
}

/// Chain free half-edges (as traversed by their triangle) into loops. Returns `(loops, unclosed_edge_count)`.
fn chain_loops(tris: &[[usize; 3]]) -> (Vec<Vec<usize>>, usize) {
    let groups = edge_groups(tris);
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for ((lo, hi), uses) in &groups {
        if uses.len() == 1 {
            let (_, fwd) = uses[0];
            let (a, b) = if fwd { (*lo, *hi) } else { (*hi, *lo) };
            out.entry(a).or_default().push(b);
        }
    }
    let mut loops = Vec::new();
    let mut unclosed = 0usize;
    let starts: Vec<usize> = out.keys().copied().collect();
    for s in starts {
        while out.get(&s).is_some_and(|v| !v.is_empty()) {
            let mut chain = vec![s];
            let mut cur = s;
            let mut closed = false;
            loop {
                let nxt = match out.get_mut(&cur) {
                    Some(v) if !v.is_empty() => v.remove(0),
                    _ => break,
                };
                if nxt == s {
                    closed = true;
                    break;
                }
                chain.push(nxt);
                cur = nxt;
            }
            if closed {
                loops.push(chain);
            } else {
                unclosed += chain.len();
            }
        }
    }
    (loops, unclosed)
}

/// Diagnose a surface: exact defect counts (see [`Diagnostics`]).
pub fn diagnose(s: &TriSurface, opt: &DiagOptions) -> Diagnostics {
    let groups = edge_groups(&s.tris);
    let (nv, ne, chi) = s.euler();
    let mut free = 0;
    let mut nonman = 0;
    let mut misor = 0;
    for uses in groups.values() {
        match uses.len() {
            1 => free += 1,
            2 => {
                if uses[0].1 == uses[1].1 {
                    misor += 1;
                }
            }
            _ => nonman += 1,
        }
    }
    // duplicates
    let mut keys: Vec<([usize; 3], usize)> = s
        .tris
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut k = *t;
            k.sort_unstable();
            (k, i)
        })
        .collect();
    keys.sort_unstable();
    let duplicate_faces = keys
        .windows(2)
        .filter(|w| w[0].0 == w[1].0 && w[0].0[0] != w[0].0[1] && w[0].0[1] != w[0].0[2])
        .count();
    // degenerate, sliver
    let mut degenerate = 0;
    let mut sliver = 0;
    for i in 0..s.tris.len() {
        if is_degenerate(s, i, opt.area_tol) {
            degenerate += 1;
        } else if min_angle_deg(s.tri_points(i)) < opt.min_angle_deg {
            sliver += 1;
        }
    }
    // small edges
    let lens: Vec<f64> = groups
        .keys()
        .map(|&(a, b)| norm(sub(s.points[a], s.points[b])))
        .collect();
    let mean = if lens.is_empty() {
        0.0
    } else {
        lens.iter().sum::<f64>() / lens.len() as f64
    };
    let small = lens.iter().filter(|&&l| l < opt.small_edge * mean).count();
    // multi-connected vertices
    let mut inc: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (i, t) in s.tris.iter().enumerate() {
        for &v in t {
            inc.entry(v).or_default().push(i);
        }
    }
    let mut multi = 0;
    for (v, ts) in &inc {
        // union-find over the incident triangles; adjacent when they share an edge through v
        let n = ts.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(p: &mut Vec<usize>, x: usize) -> usize {
            let mut r = x;
            while p[r] != r {
                r = p[r];
            }
            let mut y = x;
            while p[y] != r {
                let nx = p[y];
                p[y] = r;
                y = nx;
            }
            r
        }
        for a in 0..n {
            for b in a + 1..n {
                let ta = s.tris[ts[a]];
                let tb = s.tris[ts[b]];
                let shared = ta.iter().filter(|&&x| x != *v && tb.contains(&x)).count();
                if shared >= 1 {
                    let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                    parent[ra] = rb;
                }
            }
        }
        let roots: std::collections::BTreeSet<usize> = (0..n).map(|k| find(&mut parent, k)).collect();
        if roots.len() > 1 {
            multi += 1;
        }
    }
    let (loops, unclosed) = chain_loops(&s.tris);
    // components
    let mut parent: Vec<usize> = (0..s.tris.len()).collect();
    fn find2(p: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while p[r] != r {
            r = p[r];
        }
        let mut y = x;
        while p[y] != r {
            let nx = p[y];
            p[y] = r;
            y = nx;
        }
        r
    }
    for uses in groups.values() {
        for w in uses.windows(2) {
            let (ra, rb) = (find2(&mut parent, w[0].0), find2(&mut parent, w[1].0));
            if ra != rb {
                parent[ra] = rb;
            }
        }
    }
    let comps: std::collections::BTreeSet<usize> = (0..s.tris.len()).map(|i| find2(&mut parent, i)).collect();
    Diagnostics {
        n_points_used: nv,
        n_tris: s.tris.len(),
        n_edges: ne,
        euler: chi,
        free_edges: free,
        nonmanifold_edges: nonman,
        multi_connected_vertices: multi,
        misoriented_edges: misor,
        duplicate_faces,
        degenerate_tris: degenerate,
        sliver_tris: sliver,
        small_edges: small,
        open_loops: loops,
        unclosed_chains: unclosed,
        components: comps.len(),
        enclosed_volume: s.enclosed_volume(),
        area: s.area(),
    }
}

// ------------------------------------------------------------------------------------------------------------
// repair
// ------------------------------------------------------------------------------------------------------------

/// Options for [`repair`].
#[derive(Debug, Clone, Copy)]
pub struct RepairOptions {
    /// Degenerate-triangle threshold as in [`DiagOptions`].
    pub area_tol: f64,
    /// Flip closed components with negative enclosed volume (turn off for domain surfaces with inner shells).
    pub orient_closed_outward: bool,
    /// Fill open loops by ear clipping.
    pub fill_holes: bool,
    /// Do not fill loops longer than this (0 means no limit).
    pub max_hole_edges: usize,
}

impl Default for RepairOptions {
    fn default() -> Self {
        RepairOptions {
            area_tol: 1e-12,
            orient_closed_outward: true,
            fill_holes: true,
            max_hole_edges: 0,
        }
    }
}

/// What [`repair`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepairReport {
    /// Degenerate triangles removed.
    pub degenerate_removed: usize,
    /// Duplicate triangles removed.
    pub duplicates_removed: usize,
    /// Triangles flipped for orientation consistency.
    pub flipped: usize,
    /// Closed components turned inside out because their volume was negative.
    pub components_inverted: usize,
    /// Holes filled.
    pub holes_filled: usize,
    /// Triangles added by hole filling.
    pub tris_added: usize,
    /// Holes left (too long or not closed).
    pub holes_left: usize,
    /// Unused vertices dropped.
    pub points_dropped: usize,
}

fn ear_clip(pts: &[Vec3], w: &[usize]) -> Vec<[usize; 3]> {
    // w is in the direction the fill triangles must traverse
    let mut w = w.to_vec();
    let mut out = Vec::new();
    let mut nrm = [0.0; 3];
    for i in 0..w.len() {
        let c = cross(pts[w[i]], pts[w[(i + 1) % w.len()]]);
        nrm = add(nrm, c);
    }
    while w.len() > 3 {
        let n = w.len();
        let mut best: Option<(f64, usize)> = None;
        let mut best_loose: Option<(f64, usize)> = None;
        for i in 0..n {
            let (a, b, c) = (w[(i + n - 1) % n], w[i], w[(i + 1) % n]);
            let (pa, pb, pc) = (pts[a], pts[b], pts[c]);
            let diag = norm(sub(pc, pa));
            let convex = dot(cross(sub(pb, pa), sub(pc, pb)), nrm) > 0.0;
            if best_loose.is_none_or_lt(diag) {
                best_loose = Some((diag, i));
            }
            if !convex {
                continue;
            }
            let tn = cross(sub(pb, pa), sub(pc, pa));
            let mut inside = false;
            for &o in &w {
                if o == a || o == b || o == c {
                    continue;
                }
                let p = pts[o];
                let s1 = dot(cross(sub(pb, pa), sub(p, pa)), tn);
                let s2 = dot(cross(sub(pc, pb), sub(p, pb)), tn);
                let s3 = dot(cross(sub(pa, pc), sub(p, pc)), tn);
                if s1 > 0.0 && s2 > 0.0 && s3 > 0.0 {
                    inside = true;
                    break;
                }
            }
            if inside {
                continue;
            }
            if best.is_none_or_lt(diag) {
                best = Some((diag, i));
            }
        }
        let i = best.or(best_loose).map(|b| b.1).unwrap_or(0);
        let (a, b, c) = (w[(i + n - 1) % n], w[i], w[(i + 1) % n]);
        out.push([a, b, c]);
        w.remove(i);
    }
    out.push([w[0], w[1], w[2]]);
    out
}

trait OptLt {
    fn is_none_or_lt(&self, v: f64) -> bool;
}
impl OptLt for Option<(f64, usize)> {
    fn is_none_or_lt(&self, v: f64) -> bool {
        match self {
            None => true,
            Some((d, _)) => v < *d,
        }
    }
}

/// Repair a surface (see ADR 0030 A1 for the fixed order of steps). Deterministic and idempotent.
pub fn repair(s: &TriSurface, opt: &RepairOptions) -> (TriSurface, RepairReport) {
    let mut rep = RepairReport::default();
    let mut tris: Vec<[usize; 3]> = Vec::new();
    let mut tags: Vec<u32> = Vec::new();
    // 1. degenerate
    for i in 0..s.tris.len() {
        if is_degenerate(s, i, opt.area_tol) {
            rep.degenerate_removed += 1;
        } else {
            tris.push(s.tris[i]);
            tags.push(s.tags[i]);
        }
    }
    // 2. duplicates (keep first)
    {
        let mut seen: BTreeMap<[usize; 3], ()> = BTreeMap::new();
        let mut t2 = Vec::new();
        let mut g2 = Vec::new();
        for (t, g) in tris.iter().zip(tags.iter()) {
            let mut k = *t;
            k.sort_unstable();
            if seen.insert(k, ()).is_some() {
                rep.duplicates_removed += 1;
            } else {
                t2.push(*t);
                g2.push(*g);
            }
        }
        tris = t2;
        tags = g2;
    }
    // 3. orientation by flooding across manifold (2-use) edges
    {
        let groups = edge_groups(&tris);
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); tris.len()];
        for uses in groups.values() {
            if uses.len() == 2 {
                adj[uses[0].0].push(uses[1].0);
                adj[uses[1].0].push(uses[0].0);
            }
        }
        let mut visited = vec![false; tris.len()];
        let mut comp_of = vec![usize::MAX; tris.len()];
        let mut ncomp = 0;
        for seed in 0..tris.len() {
            if visited[seed] {
                continue;
            }
            visited[seed] = true;
            comp_of[seed] = ncomp;
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(seed);
            while let Some(t) = queue.pop_front() {
                for &n in &adj[t] {
                    if visited[n] {
                        continue;
                    }
                    visited[n] = true;
                    comp_of[n] = ncomp;
                    // shared edge: find direction in both
                    let dir = |tri: [usize; 3], a: usize, b: usize| -> bool {
                        (0..3).any(|k| tri[k] == a && tri[(k + 1) % 3] == b)
                    };
                    let (ta, tb) = (tris[t], tris[n]);
                    let mut consistent = true;
                    for k in 0..3 {
                        let (a, b) = (ta[k], ta[(k + 1) % 3]);
                        if tb.contains(&a) && tb.contains(&b) {
                            consistent = !dir(tb, a, b);
                        }
                    }
                    if !consistent {
                        tris[n].swap(1, 2);
                        rep.flipped += 1;
                    }
                    queue.push_back(n);
                }
            }
            ncomp += 1;
        }
        if opt.orient_closed_outward {
            // volume per component; closed = no free edge in it
            let mut vol = vec![0.0; ncomp];
            let mut open = vec![false; ncomp];
            for (i, t) in tris.iter().enumerate() {
                vol[comp_of[i]] += dot(s.points[t[0]], cross(s.points[t[1]], s.points[t[2]])) / 6.0;
            }
            for uses in edge_groups(&tris).values() {
                if uses.len() == 1 {
                    open[comp_of[uses[0].0]] = true;
                }
            }
            for c in 0..ncomp {
                if !open[c] && vol[c] < 0.0 {
                    rep.components_inverted += 1;
                    for (i, t) in tris.iter_mut().enumerate() {
                        if comp_of[i] == c {
                            t.swap(1, 2);
                        }
                    }
                }
            }
        }
    }
    // 4. holes
    if opt.fill_holes {
        let (loops, unclosed) = chain_loops(&tris);
        rep.holes_left += usize::from(unclosed > 0);
        // tag source: tag of the triangle owning the first half-edge of each loop
        let tag_of = |tris: &[[usize; 3]], tags: &[u32], a: usize, b: usize| -> u32 {
            for (i, t) in tris.iter().enumerate() {
                for k in 0..3 {
                    if t[k] == a && t[(k + 1) % 3] == b {
                        return tags[i];
                    }
                }
            }
            0
        };
        let mut add_t = Vec::new();
        let mut add_g = Vec::new();
        for lp in loops {
            if opt.max_hole_edges > 0 && lp.len() > opt.max_hole_edges {
                rep.holes_left += 1;
                continue;
            }
            let tg = tag_of(&tris, &tags, lp[0], lp[1 % lp.len()]);
            let rev: Vec<usize> = lp.iter().rev().copied().collect();
            let fill = ear_clip(&s.points, &rev);
            rep.holes_filled += 1;
            rep.tris_added += fill.len();
            for f in fill {
                add_t.push(f);
                add_g.push(tg);
            }
        }
        tris.extend(add_t);
        tags.extend(add_g);
    }
    // 5. compact
    let mut used = vec![false; s.points.len()];
    for t in &tris {
        for &v in t {
            used[v] = true;
        }
    }
    let mut remap = vec![usize::MAX; s.points.len()];
    let mut pts = Vec::new();
    for (i, &u) in used.iter().enumerate() {
        if u {
            remap[i] = pts.len();
            pts.push(s.points[i]);
        }
    }
    rep.points_dropped = s.points.len() - pts.len();
    let tris = tris.iter().map(|t| [remap[t[0]], remap[t[1]], remap[t[2]]]).collect();
    (
        TriSurface {
            points: pts,
            tris,
            tags,
        },
        rep,
    )
}
