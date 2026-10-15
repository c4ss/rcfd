//! Tetrahedral volume meshing of a closed triangle surface (F-132, ADR 0030 part C).
//!
//! Incremental Delaunay tetrahedralisation (Bowyer-Watson insertion with exact predicates), boundary recovery by
//! conforming Steiner insertion on missing surface edges and faces, flood-fill classification of the inside, and
//! Delaunay refinement (circumcentre insertion) for radius-edge ratio and size. No sliver removal. Deterministic.

use crate::mesh::{Mesh, PatchKind};
use crate::predicates::{insphere, orient3d};
use crate::surface::{diagnose, DiagOptions, TriSurface};
use rcfd_core::vec3::{add, cross, dot, norm, scale, sub, Vec3};
use rcfd_core::{Error, Result};
use std::collections::{BTreeMap, HashSet};

const NONE: usize = usize::MAX;
/// Outward-oriented faces of a positively oriented tetrahedron, indexed by the opposite vertex.
const FACE: [[usize; 3]; 4] = [[1, 2, 3], [0, 3, 2], [0, 1, 3], [0, 2, 1]];

fn merr<T>(m: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(m.into()))
}

/// Options for [`tetrahedralize`].
#[derive(Debug, Clone)]
pub struct TetOptions {
    /// Delaunay refinement: split tetrahedra with circumradius over shortest edge above this (0 disables the radius-edge test).
    pub max_radius_edge: f64,
    /// Split tetrahedra with a circumradius above this size (`None` disables the size test).
    pub size: Option<f64>,
    /// Run the refinement stage at all.
    pub refine: bool,
    /// Allow refinement to split encroached surface edges (otherwise the surface is fixed after recovery and
    /// tetrahedra whose circumcentre would encroach are left as they are).
    pub split_boundary: bool,
    /// Interior tetrahedra with volume over rms-edge-cubed below this are reconnected by flips (0 disables).
    pub sliver_q: f64,
    /// Cap on Steiner points added by recovery and refinement, as a multiple of the input vertex count plus 10000.
    pub max_steiner_factor: usize,
}

impl Default for TetOptions {
    fn default() -> Self {
        TetOptions {
            max_radius_edge: 2.0,
            size: None,
            refine: true,
            split_boundary: false,
            sliver_q: 0.03,
            max_steiner_factor: 100,
        }
    }
}

/// Counters from a run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TetReport {
    /// Input vertices used.
    pub input_points: usize,
    /// Steiner points added on the surface (recovery and encroachment).
    pub steiner_surface: usize,
    /// Steiner points added in the interior (refinement).
    pub steiner_interior: usize,
    /// Refinement rounds.
    pub rounds: usize,
    /// Bad tetrahedra left when the refinement stopped.
    pub bad_left: usize,
    /// Bistellar flips made by the sliver reconnection.
    pub flips: usize,
    /// Surface edges split to remove flat tetrahedra lying against the surface.
    pub flat_split: usize,
}

/// A tetrahedral mesh with its conforming boundary surface.
#[derive(Debug, Clone)]
pub struct TetMesh {
    /// Vertices (input and Steiner).
    pub points: Vec<Vec3>,
    /// Positively oriented tetrahedra (`orient3d(a,b,c,d) > 0`).
    pub cells: Vec<[usize; 4]>,
    /// Boundary: a conforming subdivision of the input surface (same vertex numbering as `points`).
    pub surface: TriSurface,
    /// Counters.
    pub report: TetReport,
}

struct Cavity {
    tets: Vec<usize>,
    faces: Vec<(usize, usize)>,
}

struct Del {
    pts: Vec<Vec3>,
    tets: Vec<[usize; 4]>,
    nb: Vec<[usize; 4]>,
    alive: Vec<bool>,
    inside: Vec<u8>,
    free: Vec<usize>,
    vtet: Vec<usize>,
    mark: Vec<u8>,
    last: usize,
    rng: u64,
}

impl Del {
    fn new(lo: Vec3, hi: Vec3) -> Del {
        let c = scale(add(lo, hi), 0.5);
        let l = norm(sub(hi, lo)).max(1e-300);
        let r = 20.0 * l;
        let mut s = [
            add(c, [r, r, r]),
            add(c, [r, -r, -r]),
            add(c, [-r, r, -r]),
            add(c, [-r, -r, r]),
        ];
        let v = [0, 1, 2, 3];
        if orient3d(s[0], s[1], s[2], s[3]) < 0 {
            s.swap(1, 2);
        }
        Del {
            pts: s.to_vec(),
            tets: vec![v],
            nb: vec![[NONE; 4]],
            alive: vec![true],
            inside: vec![0],
            free: vec![],
            vtet: vec![0; 4],
            mark: vec![0],
            last: 0,
            rng: 0x2545F4914F6CDD1D,
        }
    }

    fn rand(&mut self) -> usize {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 33) as usize
    }

    fn tp(&self, t: usize) -> [Vec3; 4] {
        let v = self.tets[t];
        [self.pts[v[0]], self.pts[v[1]], self.pts[v[2]], self.pts[v[3]]]
    }

    fn face_pts(&self, t: usize, i: usize) -> [Vec3; 3] {
        let v = self.tets[t];
        let f = FACE[i];
        [self.pts[v[f[0]]], self.pts[v[f[1]]], self.pts[v[f[2]]]]
    }

    fn locate(&mut self, p: Vec3) -> Option<usize> {
        let mut t = self.last;
        if !self.alive[t] {
            t = (0..self.tets.len()).find(|&i| self.alive[i])?;
        }
        let cap = 20 * self.tets.len() + 1000;
        let mut steps = 0;
        'outer: loop {
            steps += 1;
            if steps > cap {
                break;
            }
            let start = self.rand() % 4;
            for k in 0..4 {
                let i = (start + k) % 4;
                let [a, b, c] = self.face_pts(t, i);
                if orient3d(a, b, c, p) > 0 {
                    let n = self.nb[t][i];
                    if n == NONE {
                        return None;
                    }
                    t = n;
                    continue 'outer;
                }
            }
            self.last = t;
            return Some(t);
        }
        // fallback: scan
        for t in 0..self.tets.len() {
            if !self.alive[t] {
                continue;
            }
            if (0..4).all(|i| {
                let [a, b, c] = self.face_pts(t, i);
                orient3d(a, b, c, p) <= 0
            }) {
                self.last = t;
                return Some(t);
            }
        }
        None
    }

    fn cavity(&mut self, p: Vec3, t0: usize) -> Result<Cavity> {
        let q = self.tp(t0);
        if q.iter().any(|x| *x == p) {
            return merr("duplicate point in tetrahedralisation");
        }
        self.mark.resize(self.tets.len(), 0);
        let mut touched: Vec<usize> = vec![t0];
        self.mark[t0] = 1;
        let mut cav = vec![t0];
        let mut stack = vec![t0];
        while let Some(t) = stack.pop() {
            for i in 0..4 {
                let n = self.nb[t][i];
                if n == NONE || self.mark[n] != 0 {
                    continue;
                }
                touched.push(n);
                let [a, b, c, d] = self.tp(n);
                if insphere(a, b, c, d, p) > 0 {
                    self.mark[n] = 1;
                    cav.push(n);
                    stack.push(n);
                } else {
                    self.mark[n] = 2;
                }
            }
        }
        let mut idx = 0;
        let mut failed = false;
        while idx < cav.len() {
            let t = cav[idx];
            idx += 1;
            for i in 0..4 {
                let n = self.nb[t][i];
                if n != NONE && self.mark[n] == 1 {
                    continue;
                }
                let [a, b, c] = self.face_pts(t, i);
                if orient3d(a, b, c, p) >= 0 {
                    if n == NONE {
                        failed = true;
                        continue;
                    }
                    if self.mark[n] == 0 {
                        touched.push(n);
                    }
                    self.mark[n] = 1;
                    cav.push(n);
                }
            }
        }
        let mut faces = Vec::new();
        for &t in &cav {
            for i in 0..4 {
                let n = self.nb[t][i];
                if n == NONE || self.mark[n] != 1 {
                    faces.push((t, i));
                }
            }
        }
        for t in touched {
            self.mark[t] = 0;
        }
        if failed {
            return merr("point insertion reached the bounding tetrahedron hull");
        }
        Ok(Cavity { tets: cav, faces })
    }

    fn alloc(&mut self, v: [usize; 4], flag: u8) -> usize {
        if let Some(s) = self.free.pop() {
            self.tets[s] = v;
            self.nb[s] = [NONE; 4];
            self.alive[s] = true;
            self.inside[s] = flag;
            s
        } else {
            self.tets.push(v);
            self.nb.push([NONE; 4]);
            self.alive.push(true);
            self.inside.push(flag);
            self.tets.len() - 1
        }
    }

    fn commit(&mut self, p: Vec3, cav: &Cavity, flag: u8) -> usize {
        let pidx = self.pts.len();
        self.pts.push(p);
        self.vtet.push(NONE);
        let mut recs: Vec<(usize, usize, usize, usize)> = Vec::new();
        let mut news = Vec::new();
        for &(t, i) in &cav.faces {
            let f = FACE[i];
            let v = self.tets[t];
            let (a, b, c) = (v[f[0]], v[f[1]], v[f[2]]);
            let outside = self.nb[t][i];
            let s = self.alloc([a, c, b, pidx], flag);
            news.push(s);
            self.nb[s][3] = outside;
            if outside != NONE {
                let j = (0..4).find(|&j| self.nb[outside][j] == t).expect("neighbour link");
                self.nb[outside][j] = s;
            }
            recs.push((c.min(b), c.max(b), s, 0));
            recs.push((a.min(b), a.max(b), s, 1));
            recs.push((a.min(c), a.max(c), s, 2));
        }
        recs.sort_unstable();
        for w in recs.chunks(2) {
            debug_assert!(w.len() == 2 && w[0].0 == w[1].0 && w[0].1 == w[1].1);
            self.nb[w[0].2][w[0].3] = w[1].2;
            self.nb[w[1].2][w[1].3] = w[0].2;
        }
        for &t in &cav.tets {
            self.alive[t] = false;
            self.free.push(t);
        }
        for &s in &news {
            for &v in &self.tets[s].clone() {
                self.vtet[v] = s;
            }
        }
        self.last = news[0];
        pidx
    }

    /// Insert a point; the new tetrahedra get `flag` unless the cavity mixes flags, then 0.
    fn insert(&mut self, p: Vec3) -> Result<usize> {
        let t0 = match self.locate(p) {
            Some(t) => t,
            None => return merr("point outside the bounding tetrahedron"),
        };
        let cav = self.cavity(p, t0)?;
        let all_in = cav.tets.iter().all(|&t| self.inside[t] == 1);
        Ok(self.commit(p, &cav, u8::from(all_in)))
    }

    fn vol_q(&self, v: [usize; 4]) -> f64 {
        let p = [self.pts[v[0]], self.pts[v[1]], self.pts[v[2]], self.pts[v[3]]];
        let vol = dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), sub(p[3], p[0])) / 6.0;
        let mut s2 = 0.0;
        for i in 0..4 {
            for j in i + 1..4 {
                let d = sub(p[i], p[j]);
                s2 += dot(d, d);
            }
        }
        let l = (s2 / 6.0).sqrt();
        vol / (l * l * l)
    }

    /// Replace the tetrahedra `old` by `new` (positively oriented, same boundary faces) and reconnect.
    fn replace(&mut self, old: &[usize], new: &[[usize; 4]], flag: u8) {
        let mut outer: std::collections::HashMap<[usize; 3], (usize, usize)> = std::collections::HashMap::new();
        for &t in old {
            for i in 0..4 {
                let n = self.nb[t][i];
                if n == NONE || !old.contains(&n) {
                    let f = FACE[i];
                    let v = self.tets[t];
                    outer.insert(sorted3(v[f[0]], v[f[1]], v[f[2]]), (n, t));
                }
            }
        }
        let mut slots = Vec::new();
        for &v in new {
            slots.push(self.alloc(v, flag));
        }
        let mut inner: std::collections::HashMap<[usize; 3], (usize, usize)> = std::collections::HashMap::new();
        for &s in &slots {
            for i in 0..4 {
                let f = FACE[i];
                let v = self.tets[s];
                let key = sorted3(v[f[0]], v[f[1]], v[f[2]]);
                if let Some(&(n, ot)) = outer.get(&key) {
                    self.nb[s][i] = n;
                    if n != NONE {
                        let j = (0..4).find(|&j| self.nb[n][j] == ot).expect("neighbour link");
                        self.nb[n][j] = s;
                    }
                } else if let Some((s2, i2)) = inner.remove(&key) {
                    self.nb[s][i] = s2;
                    self.nb[s2][i2] = s;
                } else {
                    inner.insert(key, (s, i));
                }
            }
        }
        for &t in old {
            self.alive[t] = false;
            self.free.push(t);
        }
        for &s in &slots {
            for &v in &self.tets[s].clone() {
                self.vtet[v] = s;
            }
        }
    }

    /// 2-3 flip across face `i` of `t`; `surf` lists faces that must not be removed.
    fn try_flip23(&mut self, t: usize, i: usize, surf: &HashSet<[usize; 3]>) -> bool {
        let n = self.nb[t][i];
        if n == NONE || self.inside[n] != 1 || self.inside[t] != 1 {
            return false;
        }
        let f = FACE[i];
        let v = self.tets[t];
        let (a, b, c, d) = (v[f[0]], v[f[1]], v[f[2]], v[i]);
        if surf.contains(&sorted3(a, b, c)) {
            return false;
        }
        let e = match self.tets[n].iter().find(|x| **x != a && **x != b && **x != c) {
            Some(&e) => e,
            None => return false,
        };
        let (pa, pb, pc, pd, pe) = (self.pts[a], self.pts[b], self.pts[c], self.pts[d], self.pts[e]);
        let o = [
            orient3d(pd, pe, pa, pb),
            orient3d(pd, pe, pb, pc),
            orient3d(pd, pe, pc, pa),
        ];
        if !(o.iter().all(|&x| x > 0) || o.iter().all(|&x| x < 0)) {
            return false;
        }
        let mut new = [[d, e, a, b], [d, e, b, c], [d, e, c, a]];
        if o[0] < 0 {
            for t in new.iter_mut() {
                t.swap(2, 3);
            }
        }
        let old_q = self.vol_q(self.tets[t]).min(self.vol_q(self.tets[n]));
        let new_q = new.iter().map(|&x| self.vol_q(x)).fold(f64::INFINITY, f64::min);
        if new_q <= old_q * (1.0 + 1e-9) + 1e-300 {
            return false;
        }
        self.replace(&[t, n], &new, 1);
        true
    }

    /// Edge removal around `(d, e)`: when 3 to 7 interior tetrahedra share the edge, re-triangulate the ring polygon
    /// (dynamic programming over triangulations, maximising the worst volume quality) and replace the ring by two
    /// tetrahedra per polygon triangle. The 3-2 flip is the case of three.
    fn try_edge_removal(&mut self, d: usize, e: usize) -> bool {
        let ring: Vec<usize> = self
            .tets_around(d)
            .into_iter()
            .filter(|&t| self.tets[t].contains(&e))
            .collect();
        let n = ring.len();
        if !(3..=7).contains(&n) || ring.iter().any(|&t| self.inside[t] != 1) {
            return false;
        }
        // polygon edges
        let mut pe: Vec<(usize, usize)> = Vec::new();
        for &t in &ring {
            let o: Vec<usize> = self.tets[t].iter().copied().filter(|&x| x != d && x != e).collect();
            pe.push((o[0], o[1]));
        }
        let mut poly = vec![pe[0].0, pe[0].1];
        let mut used = vec![false; n];
        used[0] = true;
        while poly.len() < n {
            let last = *poly.last().expect("poly");
            let mut found = false;
            for k in 0..n {
                if used[k] {
                    continue;
                }
                let (x, y) = pe[k];
                if x == last || y == last {
                    poly.push(if x == last { y } else { x });
                    used[k] = true;
                    found = true;
                    break;
                }
            }
            if !found {
                return false;
            }
        }
        if poly[n - 1] == poly[0]
            || pe
                .iter()
                .all(|&(x, y)| !(x == poly[n - 1] && y == poly[0] || y == poly[n - 1] && x == poly[0]))
        {
            return false;
        }
        let (pd, pe_) = (self.pts[d], self.pts[e]);
        let tri_tets = |me: &Del, i: usize, j: usize, k: usize| -> Option<([usize; 4], [usize; 4], f64)> {
            let (a, b, c) = (poly[i], poly[j], poly[k]);
            let (pa, pb, pc) = (me.pts[a], me.pts[b], me.pts[c]);
            let od = orient3d(pa, pb, pc, pd);
            let oe = orient3d(pa, pb, pc, pe_);
            if od == 0 || oe == 0 || od == oe {
                return None;
            }
            let t1 = if od > 0 { [a, b, c, d] } else { [a, c, b, d] };
            let t2 = if oe > 0 { [a, b, c, e] } else { [a, c, b, e] };
            let q = me.vol_q(t1).min(me.vol_q(t2));
            Some((t1, t2, q))
        };
        // best[i][j]: best worst-quality triangulation of the sub-polygon i..=j (j - i >= 2); choice[i][j] = k
        let neg = f64::NEG_INFINITY;
        let mut best = vec![vec![f64::INFINITY; n]; n];
        let mut choice = vec![vec![0usize; n]; n];
        for len in 2..n {
            for i in 0..n - len {
                let j = i + len;
                let mut bq = neg;
                let mut bk = 0;
                for k in i + 1..j {
                    let q = match tri_tets(self, i, k, j) {
                        Some((_, _, q)) => q,
                        None => neg,
                    };
                    let v = q.min(best[i][k]).min(best[k][j]);
                    if v > bq {
                        bq = v;
                        bk = k;
                    }
                }
                best[i][j] = bq;
                choice[i][j] = bk;
            }
        }
        let new_q = best[0][n - 1];
        let old_q = ring
            .iter()
            .map(|&t| self.vol_q(self.tets[t]))
            .fold(f64::INFINITY, f64::min);
        if !new_q.is_finite() || new_q <= old_q * (1.0 + 1e-9) + 1e-300 {
            return false;
        }
        let mut new: Vec<[usize; 4]> = Vec::new();
        let mut stack = vec![(0usize, n - 1)];
        while let Some((i, j)) = stack.pop() {
            if j - i < 2 {
                continue;
            }
            let k = choice[i][j];
            let (t1, t2, _) = tri_tets(self, i, k, j).expect("valid triangle");
            new.push(t1);
            new.push(t2);
            stack.push((i, k));
            stack.push((k, j));
        }
        self.replace(&ring.clone(), &new, 1);
        true
    }

    fn tets_around(&self, v: usize) -> Vec<usize> {
        let mut seen = vec![self.vtet[v]];
        let mut stack = vec![self.vtet[v]];
        while let Some(t) = stack.pop() {
            for i in 0..4 {
                if self.tets[t][i] == v {
                    continue;
                }
                let n = self.nb[t][i];
                if n != NONE && !seen.contains(&n) {
                    seen.push(n);
                    stack.push(n);
                }
            }
        }
        seen
    }

    fn has_edge(&self, a: usize, b: usize) -> bool {
        self.tets_around(a).iter().any(|&t| self.tets[t].contains(&b))
    }

    fn tets_with_face(&self, a: usize, b: usize, c: usize) -> Vec<usize> {
        self.tets_around(a)
            .into_iter()
            .filter(|&t| self.tets[t].contains(&b) && self.tets[t].contains(&c))
            .collect()
    }
}

fn sorted3(a: usize, b: usize, c: usize) -> [usize; 3] {
    let mut k = [a, b, c];
    k.sort_unstable();
    k
}

struct SubSurf {
    tris: Vec<[usize; 3]>,
    tags: Vec<u32>,
    alive: Vec<bool>,
    edges: BTreeMap<(usize, usize), Vec<usize>>,
    faces: HashSet<[usize; 3]>,
}

impl SubSurf {
    fn new() -> SubSurf {
        SubSurf {
            tris: vec![],
            tags: vec![],
            alive: vec![],
            edges: BTreeMap::new(),
            faces: HashSet::new(),
        }
    }

    fn add(&mut self, t: [usize; 3], tag: u32) {
        let id = self.tris.len();
        self.tris.push(t);
        self.tags.push(tag);
        self.alive.push(true);
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            self.edges.entry((a.min(b), a.max(b))).or_default().push(id);
        }
        self.faces.insert(sorted3(t[0], t[1], t[2]));
    }

    fn remove(&mut self, id: usize) {
        let t = self.tris[id];
        self.alive[id] = false;
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let key = (a.min(b), a.max(b));
            if let Some(v) = self.edges.get_mut(&key) {
                v.retain(|&x| x != id);
                if v.is_empty() {
                    self.edges.remove(&key);
                }
            }
        }
        self.faces.remove(&sorted3(t[0], t[1], t[2]));
    }

    fn split_edge(&mut self, a: usize, b: usize, m: usize) {
        let ids = self.edges.get(&(a.min(b), a.max(b))).cloned().unwrap_or_default();
        for id in ids {
            let t = self.tris[id];
            let tag = self.tags[id];
            let k = (0..3)
                .find(|&k| (t[k] == a && t[(k + 1) % 3] == b) || (t[k] == b && t[(k + 1) % 3] == a))
                .expect("edge in triangle");
            let (u, v, w) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
            self.remove(id);
            self.add([u, m, w], tag);
            self.add([m, v, w], tag);
        }
    }
}

fn midpoint(a: Vec3, b: Vec3) -> Vec3 {
    scale(add(a, b), 0.5)
}

struct Ctx {
    d: Del,
    s: SubSurf,
    dirty: bool,
    split_boundary: bool,
    budget: usize,
    rep: TetReport,
}

impl Ctx {
    fn split_surface_edge(&mut self, a: usize, b: usize) -> Result<()> {
        if self.rep.steiner_surface + self.rep.steiner_interior >= self.budget {
            return merr("Steiner point budget exhausted during boundary recovery or refinement");
        }
        let m = midpoint(self.d.pts[a], self.d.pts[b]);
        if std::env::var("W19B_DBG2").is_ok() && self.rep.steiner_surface % 200 == 0 {
            eprintln!(
                "split {a}-{b} len {:e} pts {} n={}",
                norm(sub(self.d.pts[a], self.d.pts[b])),
                self.d.pts.len(),
                self.rep.steiner_surface
            );
        }
        let mi = self.d.insert(m)?;
        self.rep.steiner_surface += 1;
        self.s.split_edge(a, b, mi);
        self.dirty = true;
        Ok(())
    }

    fn recover(&mut self) -> Result<()> {
        loop {
            let missing: Vec<(usize, usize)> = self
                .s
                .edges
                .keys()
                .copied()
                .filter(|&(a, b)| !self.d.has_edge(a, b))
                .collect();
            if std::env::var("W19B_DBG3").is_ok() {
                eprintln!(
                    "recover: missing edges {} surf tris {} pts {}",
                    missing.len(),
                    self.s.faces.len(),
                    self.d.pts.len()
                );
            }
            if !missing.is_empty() {
                for (a, b) in missing {
                    if self.s.edges.contains_key(&(a, b)) && !self.d.has_edge(a, b) {
                        self.split_surface_edge(a, b)?;
                    }
                }
                continue;
            }
            let mut any = false;
            let ids: Vec<usize> = (0..self.s.tris.len()).filter(|&i| self.s.alive[i]).collect();
            for id in ids {
                if !self.s.alive[id] {
                    continue;
                }
                let t = self.s.tris[id];
                if self.d.tets_with_face(t[0], t[1], t[2]).is_empty() {
                    let mut best = (0usize, -1.0f64);
                    for k in 0..3 {
                        let (a, b) = (t[k], t[(k + 1) % 3]);
                        let l = norm(sub(self.d.pts[a], self.d.pts[b]));
                        if l > best.1 {
                            best = (k, l);
                        }
                    }
                    let (a, b) = (t[best.0], t[(best.0 + 1) % 3]);
                    self.split_surface_edge(a, b)?;
                    any = true;
                }
            }
            if !any {
                return Ok(());
            }
        }
    }

    /// Local reconnection (2-3 and 3-2 flips) of low-quality interior tetrahedra. Returns the number of flips.
    fn improve(&mut self, thr: f64) -> usize {
        if thr <= 0.0 {
            return 0;
        }
        let mut total = 0;
        for _pass in 0..30 {
            let mut changed = false;
            let mut t = 0;
            while t < self.d.tets.len() {
                if self.d.alive[t] && self.d.inside[t] == 1 && self.d.vol_q(self.d.tets[t]) < thr {
                    let mut done = false;
                    for i in 0..4 {
                        if self.d.try_flip23(t, i, &self.s.faces) {
                            done = true;
                            break;
                        }
                    }
                    if !done {
                        let v = self.d.tets[t];
                        'e: for i in 0..4 {
                            for j in i + 1..4 {
                                if self.d.try_edge_removal(v[i], v[j]) {
                                    done = true;
                                    break 'e;
                                }
                            }
                        }
                    }
                    if done {
                        changed = true;
                        total += 1;
                    }
                }
                t += 1;
            }
            if !changed {
                break;
            }
        }
        total
    }

    /// Flat tetrahedra lying against the surface (two surface faces sharing an edge, volume quality below
    /// `flat`): split that surface edge and recover. Returns the number of edges split.
    fn split_flat_boundary(&mut self, flat: f64) -> Result<usize> {
        let mut edges: BTreeMap<(usize, usize), ()> = BTreeMap::new();
        for t in 0..self.d.tets.len() {
            if !self.d.alive[t] || self.d.inside[t] != 1 || self.d.vol_q(self.d.tets[t]).abs() >= flat {
                continue;
            }
            let v = self.d.tets[t];
            let mut sf = Vec::new();
            for i in 0..4 {
                let f = FACE[i];
                let key = sorted3(v[f[0]], v[f[1]], v[f[2]]);
                if self.s.faces.contains(&key) {
                    sf.push(key);
                }
            }
            if sf.len() >= 2 {
                // shared edge of the first two surface faces
                let shared: Vec<usize> = sf[0].iter().copied().filter(|x| sf[1].contains(x)).collect();
                if shared.len() == 2 {
                    edges.insert((shared[0].min(shared[1]), shared[0].max(shared[1])), ());
                }
            }
        }
        let n = edges.len();
        for (a, b) in edges.keys().copied().collect::<Vec<_>>() {
            if self.s.edges.contains_key(&(a, b)) {
                self.split_surface_edge(a, b)?;
            }
        }
        if n > 0 {
            self.recover()?;
            self.classify()?;
        }
        Ok(n)
    }

    fn classify(&mut self) -> Result<()> {
        let d = &mut self.d;
        d.inside.iter_mut().for_each(|f| *f = 0);
        let mut stack = Vec::new();
        for (id, t) in self.s.tris.iter().enumerate() {
            if !self.s.alive[id] {
                continue;
            }
            let ts = d.tets_with_face(t[0], t[1], t[2]);
            let mut found = false;
            for tt in ts {
                let v = d.tets[tt];
                let apex = *v
                    .iter()
                    .find(|x| **x != t[0] && **x != t[1] && **x != t[2])
                    .expect("apex");
                if orient3d(d.pts[t[0]], d.pts[t[1]], d.pts[t[2]], d.pts[apex]) < 0 {
                    if d.inside[tt] == 0 {
                        d.inside[tt] = 1;
                        stack.push(tt);
                    }
                    found = true;
                }
            }
            if !found {
                return merr("could not classify a boundary face (flat adjacent tetrahedra)");
            }
        }
        while let Some(t) = stack.pop() {
            if d.tets[t].iter().any(|&v| v < 4) {
                return merr("volume fill leaked to the bounding tetrahedron: the surface is not enclosing (self-intersecting or not closed)");
            }
            for i in 0..4 {
                let n = d.nb[t][i];
                if n == NONE {
                    return merr("volume fill reached the hull");
                }
                let f = FACE[i];
                let v = d.tets[t];
                if self.s.faces.contains(&sorted3(v[f[0]], v[f[1]], v[f[2]])) {
                    continue;
                }
                if d.inside[n] == 0 {
                    d.inside[n] = 1;
                    stack.push(n);
                }
            }
        }
        self.dirty = false;
        Ok(())
    }
}

/// Circumcentre and circumradius of a tetrahedron.
pub fn circumsphere(p: [Vec3; 4]) -> (Vec3, f64) {
    let u = sub(p[1], p[0]);
    let v = sub(p[2], p[0]);
    let w = sub(p[3], p[0]);
    let den = 2.0 * dot(u, cross(v, w));
    let num = add(
        add(scale(cross(v, w), dot(u, u)), scale(cross(w, u), dot(v, v))),
        scale(cross(u, v), dot(w, w)),
    );
    let off = scale(num, 1.0 / den);
    (add(p[0], off), norm(off))
}

fn shortest_edge(p: &[Vec3; 4]) -> f64 {
    let mut m = f64::INFINITY;
    for i in 0..4 {
        for j in i + 1..4 {
            m = m.min(norm(sub(p[i], p[j])));
        }
    }
    m
}

fn morton(p: Vec3, lo: Vec3, hi: Vec3) -> u64 {
    let mut q = [0u64; 3];
    for d in 0..3 {
        let w = (hi[d] - lo[d]).max(1e-300);
        q[d] = (((p[d] - lo[d]) / w).clamp(0.0, 1.0) * 2097151.0) as u64;
    }
    let mut code = 0u64;
    for bit in 0..21 {
        for d in 0..3 {
            code |= ((q[d] >> bit) & 1) << (3 * bit + d);
        }
    }
    code
}

/// Tetrahedralise the region bounded by a closed, outward-oriented surface (several shells allowed; inner shells
/// oriented into the hole). The surface must pass [`diagnose`] as a closed consistently oriented manifold.
pub fn tetrahedralize(surface: &TriSurface, opt: &TetOptions) -> Result<TetMesh> {
    let dg = diagnose(surface, &DiagOptions::default());
    if surface.tris.is_empty() {
        return merr("empty surface");
    }
    if dg.free_edges > 0 {
        return merr(format!(
            "surface is not closed: {} free edge(s) in {} open loop(s)",
            dg.free_edges,
            dg.open_loops.len()
        ));
    }
    if dg.nonmanifold_edges > 0 {
        return merr(format!("surface has {} non-manifold edge(s)", dg.nonmanifold_edges));
    }
    if dg.misoriented_edges > 0 {
        return merr(format!("surface has {} misoriented edge(s)", dg.misoriented_edges));
    }
    if dg.degenerate_tris > 0 || dg.duplicate_faces > 0 {
        return merr(format!(
            "surface has {} degenerate and {} duplicate triangle(s)",
            dg.degenerate_tris, dg.duplicate_faces
        ));
    }
    if dg.multi_connected_vertices > 0 {
        return merr(format!(
            "surface has {} non-manifold vertex(es)",
            dg.multi_connected_vertices
        ));
    }
    if dg.enclosed_volume <= 0.0 {
        return merr("surface encloses non-positive volume (inside-out orientation)");
    }
    let mut used: Vec<usize> = surface.tris.iter().flatten().copied().collect();
    used.sort_unstable();
    used.dedup();
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for &v in &used {
        for d in 0..3 {
            lo[d] = lo[d].min(surface.points[v][d]);
            hi[d] = hi[d].max(surface.points[v][d]);
        }
    }
    let mut order = used.clone();
    order.sort_by_key(|&v| (morton(surface.points[v], lo, hi), v));
    let mut map = vec![NONE; surface.points.len()];
    let mut ctx = Ctx {
        d: Del::new(lo, hi),
        s: SubSurf::new(),
        dirty: true,
        split_boundary: opt.split_boundary,
        budget: opt.max_steiner_factor * used.len() + 10000,
        rep: TetReport {
            input_points: used.len(),
            ..Default::default()
        },
    };
    for &v in &order {
        map[v] = ctx.d.insert(surface.points[v])?;
    }
    for (i, t) in surface.tris.iter().enumerate() {
        ctx.s.add([map[t[0]], map[t[1]], map[t[2]]], surface.tags[i]);
    }
    ctx.recover()?;
    ctx.classify()?;
    ctx.rep.flips += ctx.improve(opt.sliver_q);
    if opt.refine {
        refine(&mut ctx, opt)?;
        ctx.rep.flips += ctx.improve(opt.sliver_q);
    }
    if opt.sliver_q > 0.0 {
        for _ in 0..20 {
            if ctx.dirty {
                ctx.classify()?;
            }
            let n = ctx.split_flat_boundary(1e-9)?;
            ctx.rep.flat_split += n;
            if n == 0 {
                break;
            }
            ctx.rep.flips += ctx.improve(opt.sliver_q);
        }
    }
    if ctx.dirty {
        ctx.classify()?;
    }
    // compact
    let d = &ctx.d;
    let mut cells = Vec::new();
    for t in 0..d.tets.len() {
        if d.alive[t] && d.inside[t] == 1 {
            cells.push(d.tets[t]);
        }
    }
    let mut keep = vec![false; d.pts.len()];
    for c in &cells {
        for &v in c {
            keep[v] = true;
        }
    }
    let mut remap = vec![NONE; d.pts.len()];
    let mut points = Vec::new();
    for v in 0..d.pts.len() {
        if keep[v] {
            remap[v] = points.len();
            points.push(d.pts[v]);
        }
    }
    let cells: Vec<[usize; 4]> = cells
        .iter()
        .map(|c| [remap[c[0]], remap[c[1]], remap[c[2]], remap[c[3]]])
        .collect();
    let mut tris = Vec::new();
    let mut tags = Vec::new();
    for (i, t) in ctx.s.tris.iter().enumerate() {
        if ctx.s.alive[i] {
            tris.push([remap[t[0]], remap[t[1]], remap[t[2]]]);
            tags.push(ctx.s.tags[i]);
        }
    }
    Ok(TetMesh {
        points: points.clone(),
        cells,
        surface: TriSurface { points, tris, tags },
        report: ctx.rep,
    })
}

fn is_bad(p: &[Vec3; 4], opt: &TetOptions) -> bool {
    let (_, r) = circumsphere(*p);
    if opt.max_radius_edge > 0.0 && r > opt.max_radius_edge * shortest_edge(p) {
        return true;
    }
    if let Some(h) = opt.size {
        if r > h {
            return true;
        }
    }
    false
}

fn refine(ctx: &mut Ctx, opt: &TetOptions) -> Result<()> {
    for round in 0..2000 {
        ctx.rep.rounds = round + 1;
        if ctx.dirty {
            ctx.classify()?;
        }
        let bad: Vec<usize> = (0..ctx.d.tets.len())
            .filter(|&t| ctx.d.alive[t] && ctx.d.inside[t] == 1 && is_bad(&ctx.d.tp(t), opt))
            .collect();
        ctx.rep.bad_left = bad.len();
        if std::env::var("W19B_DBG").is_ok() {
            eprintln!(
                "round {round}: bad {} pts {} surf {} int {}",
                bad.len(),
                ctx.d.pts.len(),
                ctx.rep.steiner_surface,
                ctx.rep.steiner_interior
            );
        }
        if bad.is_empty() {
            return Ok(());
        }
        let mut progress = false;
        let mut splits: BTreeMap<(usize, usize), ()> = BTreeMap::new();
        for t in bad {
            if !ctx.d.alive[t] || ctx.d.inside[t] != 1 {
                continue;
            }
            let q = ctx.d.tp(t);
            if !is_bad(&q, opt) {
                continue;
            }
            let (c, _) = circumsphere(q);
            if !c.iter().all(|x| x.is_finite()) {
                continue;
            }
            let loc = match ctx.d.locate(c) {
                Some(l) => l,
                None => {
                    pick_split(ctx, c, &mut splits);
                    continue;
                }
            };
            if ctx.d.inside[loc] != 1 {
                pick_split(ctx, c, &mut splits);
                continue;
            }
            let cav = match ctx.d.cavity(c, loc) {
                Ok(cv) => cv,
                Err(_) => continue,
            };
            if cav.tets.iter().all(|&x| ctx.d.inside[x] == 1) {
                if ctx.rep.steiner_surface + ctx.rep.steiner_interior >= ctx.budget {
                    return merr("Steiner point budget exhausted during refinement");
                }
                ctx.d.commit(c, &cav, 1);
                ctx.rep.steiner_interior += 1;
                progress = true;
            } else {
                pick_split(ctx, c, &mut splits);
            }
        }
        if !splits.is_empty() {
            for (a, b) in splits.keys().copied().collect::<Vec<_>>() {
                if ctx.s.edges.contains_key(&(a, b)) {
                    ctx.split_surface_edge(a, b)?;
                    progress = true;
                }
            }
            ctx.recover()?;
        }
        if !progress {
            return Ok(());
        }
    }
    Ok(())
}

/// Queue the surface edge to split because `c` encroaches on it: first a surface edge whose diametral sphere contains
/// `c` (smallest distance ratio), else the longest edge of a surface triangle whose diametral ball (smallest
/// circumscribing sphere, centred at the triangle circumcentre) contains `c`. Does nothing when `c` encroaches nothing.
fn pick_split(ctx: &mut Ctx, c: Vec3, splits: &mut BTreeMap<(usize, usize), ()>) {
    if !ctx.split_boundary {
        return;
    }
    let mut best_edge: Option<(f64, (usize, usize))> = None;
    for &(a, b) in ctx.s.edges.keys() {
        let (pa, pb) = (ctx.d.pts[a], ctx.d.pts[b]);
        let r = norm(sub(pa, pb)) / 2.0;
        let dist = norm(sub(c, midpoint(pa, pb)));
        if dist < r {
            let k = dist / r;
            if best_edge.is_none_or_lt(k) {
                best_edge = Some((k, (a, b)));
            }
        }
    }
    if let Some((_, e)) = best_edge {
        splits.insert(e, ());
        return;
    }
    let mut best: Option<(f64, usize)> = None;
    for id in 0..ctx.s.tris.len() {
        if !ctx.s.alive[id] {
            continue;
        }
        let t = ctx.s.tris[id];
        let [a, b, cc] = [ctx.d.pts[t[0]], ctx.d.pts[t[1]], ctx.d.pts[t[2]]];
        let (u, v) = (sub(b, a), sub(cc, a));
        let n = cross(u, v);
        let nn = dot(n, n);
        if nn <= 0.0 {
            continue;
        }
        let off = scale(
            add(scale(cross(v, n), dot(u, u)), scale(cross(n, u), dot(v, v))),
            1.0 / (2.0 * nn),
        );
        let r = norm(off);
        let dist = norm(sub(c, add(a, off)));
        if dist < r {
            let k = dist / r;
            if best.is_none_or_lt(k) {
                best = Some((k, id));
            }
        }
    }
    if let Some((_, id)) = best {
        let t = ctx.s.tris[id];
        let mut e = (0usize, -1.0f64);
        for k in 0..3 {
            let l = norm(sub(ctx.d.pts[t[k]], ctx.d.pts[t[(k + 1) % 3]]));
            if l > e.1 {
                e = (k, l);
            }
        }
        let (a, b) = (t[e.0], t[(e.0 + 1) % 3]);
        splits.insert((a.min(b), a.max(b)), ());
    }
}

trait OptLt<T> {
    fn is_none_or_lt(&self, v: f64) -> bool;
}
impl<T> OptLt<T> for Option<(f64, T)> {
    fn is_none_or_lt(&self, v: f64) -> bool {
        match self {
            None => true,
            Some((d, _)) => v < *d,
        }
    }
}

impl TetMesh {
    /// Convert to the solver mesh. Boundary patches follow the surface tags: tag 0 is `wall`, tag `k` is `wall<k>`;
    /// `names` overrides the names by tag where given.
    pub fn to_mesh(&self, names: &BTreeMap<u32, String>) -> Result<Mesh> {
        // faces keyed by sorted vertex triple: (owner cell, outward loop, neighbour or NONE); orientation comes from the
        // exact predicates (every stored tetrahedron is positively oriented), not from floating-point geometry.
        let mut map: std::collections::HashMap<[usize; 3], (usize, [usize; 3], usize)> =
            std::collections::HashMap::new();
        for (ci, c) in self.cells.iter().enumerate() {
            for f in FACE.iter() {
                let loop_ids = [c[f[0]], c[f[1]], c[f[2]]];
                let key = sorted3(loop_ids[0], loop_ids[1], loop_ids[2]);
                match map.get_mut(&key) {
                    None => {
                        map.insert(key, (ci, loop_ids, NONE));
                    }
                    Some(e) => {
                        if e.2 != NONE {
                            return merr("face shared by more than two cells");
                        }
                        // e.0 < ci because cells are visited in order; the owner keeps its outward loop
                        e.2 = ci;
                    }
                }
            }
        }
        let mut internal: Vec<(usize, usize, [usize; 3])> =
            map.values().filter(|e| e.2 != NONE).map(|e| (e.0, e.2, e.1)).collect();
        internal.sort_unstable();
        let mut faces: Vec<Vec<usize>> = Vec::new();
        let mut owner = Vec::new();
        let mut neighbour = Vec::new();
        for (o, n, l) in internal {
            faces.push(l.to_vec());
            owner.push(o);
            neighbour.push(n);
        }
        let mut by_tag: BTreeMap<u32, Vec<[usize; 3]>> = BTreeMap::new();
        for (t, g) in self.surface.tris.iter().zip(self.surface.tags.iter()) {
            by_tag.entry(*g).or_default().push(*t);
        }
        let mut patches = Vec::new();
        for (tag, tris) in by_tag {
            for t in &tris {
                let key = sorted3(t[0], t[1], t[2]);
                match map.get(&key) {
                    Some(e) if e.2 == NONE => {
                        faces.push(e.1.to_vec());
                        owner.push(e.0);
                    }
                    _ => return merr("a boundary triangle is not a free face of exactly one cell"),
                }
            }
            let name = names.get(&tag).cloned().unwrap_or_else(|| {
                if tag == 0 {
                    "wall".to_string()
                } else {
                    format!("wall{tag}")
                }
            });
            patches.push((name, PatchKind::Wall, tris.len()));
        }
        let n_bnd_free = map.values().filter(|e| e.2 == NONE).count();
        let n_bnd: usize = patches.iter().map(|p| p.2).sum();
        if n_bnd_free != n_bnd {
            return merr(format!(
                "{} free tetrahedron faces but {} boundary triangles",
                n_bnd_free, n_bnd
            ));
        }
        Mesh::from_face_lists(self.points.clone(), faces, owner, neighbour, patches)
    }
}

/// Quality summary of a tetrahedral mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct TetQuality {
    /// Smallest dihedral angle over all cells, degrees.
    pub min_dihedral_deg: f64,
    /// Largest dihedral angle, degrees.
    pub max_dihedral_deg: f64,
    /// Largest circumradius over shortest edge.
    pub max_radius_edge: f64,
    /// Fraction of cells with radius-edge ratio at most the given limit.
    pub frac_radius_edge_ok: f64,
    /// Smallest cell volume (signed, from the vertex order).
    pub min_volume: f64,
    /// Largest circumradius.
    pub max_circumradius: f64,
}

/// Measure [`TetQuality`]; `re_limit` is the radius-edge limit for `frac_radius_edge_ok`.
pub fn tet_quality(m: &TetMesh, re_limit: f64) -> TetQuality {
    let mut q = TetQuality {
        min_dihedral_deg: 180.0,
        max_dihedral_deg: 0.0,
        max_radius_edge: 0.0,
        frac_radius_edge_ok: 0.0,
        min_volume: f64::INFINITY,
        max_circumradius: 0.0,
    };
    let mut ok = 0usize;
    for c in &m.cells {
        let p = [m.points[c[0]], m.points[c[1]], m.points[c[2]], m.points[c[3]]];
        let vol = dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), sub(p[3], p[0])) / 6.0;
        q.min_volume = q.min_volume.min(vol);
        let (_, r) = circumsphere(p);
        let re = r / shortest_edge(&p);
        q.max_radius_edge = q.max_radius_edge.max(re);
        q.max_circumradius = q.max_circumradius.max(r);
        if re <= re_limit {
            ok += 1;
        }
        let mut nrm = [[0.0; 3]; 4];
        for (i, f) in FACE.iter().enumerate() {
            let n = cross(sub(p[f[1]], p[f[0]]), sub(p[f[2]], p[f[0]]));
            let l = norm(n);
            nrm[i] = scale(n, 1.0 / l);
        }
        for i in 0..4 {
            for j in i + 1..4 {
                let ang = (std::f64::consts::PI - dot(nrm[i], nrm[j]).clamp(-1.0, 1.0).acos()).to_degrees();
                q.min_dihedral_deg = q.min_dihedral_deg.min(ang);
                q.max_dihedral_deg = q.max_dihedral_deg.max(ang);
            }
        }
    }
    q.frac_radius_edge_ok = ok as f64 / m.cells.len().max(1) as f64;
    q
}
