//! Anisotropic (directional) refinement of box meshes: a lattice with one refinement level per axis (F-348, ADR 0267).
//!
//! * [`AnisoMesh`]: a tiling of a tensor box by dyadic boxes ([`AxisLeaf`]); a leaf is split along any set of axes
//!   ([`AxisMark`]). [`AnisoMesh::to_mesh`] builds the polyhedral [`Mesh`]: the face between two leaves is the intersection of
//!   their sides, and every face loop contains all lattice vertices on its edges, so every cell is closed for any tiling.
//! * [`AnisoMesh::adapt`] refines and coarsens, with an optional balance per axis (face neighbours differ by at most one level
//!   along every refined axis), and returns the [`Transfer`] of the isotropic lattice, used by [`super::transfer_field`].
//!
//! [`super::AmrMesh`] is not changed by this module; on isotropic states both types build the same mesh bit for bit.

use super::{AdaptReport, AmrMesh, Source, Transfer};
use crate::mesh::{Mesh, PatchKind};
use rcfd_core::vec3::Vec3;
use rcfd_core::{Error, Result};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// A leaf of the directional lattice: along axis `a` it is cell `ijk[a]` of the grid of level `levels[a]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AxisLeaf {
    /// Refinement level along each axis (0 = base cell).
    pub levels: [u8; 3],
    /// Cell index on the grid of those levels.
    pub ijk: [i64; 3],
}

/// Directional adaptation mark of a cell. Any `refine` axis makes it a refinement (refine wins).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AxisMark {
    /// Split along these axes.
    pub refine: [bool; 3],
    /// Merge with the buddy along one of these axes (only when the buddy agrees).
    pub coarsen: [bool; 3],
}

impl AxisMark {
    /// Leave the cell as it is.
    pub const KEEP: AxisMark = AxisMark {
        refine: [false; 3],
        coarsen: [false; 3],
    };

    /// Split along `axes`.
    pub fn refine(axes: [bool; 3]) -> AxisMark {
        AxisMark {
            refine: axes,
            coarsen: [false; 3],
        }
    }

    /// Merge along one of `axes`.
    pub fn coarsen(axes: [bool; 3]) -> AxisMark {
        AxisMark {
            refine: [false; 3],
            coarsen: axes,
        }
    }
}

/// The leaves as a set, with the level triples present (for point location).
#[derive(Debug, Clone, Default)]
struct LeafSet {
    set: HashSet<AxisLeaf>,
    combos: BTreeMap<[u8; 3], usize>,
}

impl LeafSet {
    fn insert(&mut self, l: AxisLeaf) {
        if self.set.insert(l) {
            *self.combos.entry(l.levels).or_insert(0) += 1;
        }
    }

    fn remove(&mut self, l: &AxisLeaf) -> bool {
        if !self.set.remove(l) {
            return false;
        }
        if let Some(n) = self.combos.get_mut(&l.levels) {
            *n -= 1;
            if *n == 0 {
                self.combos.remove(&l.levels);
            }
        }
        true
    }

    fn contains(&self, l: &AxisLeaf) -> bool {
        self.set.contains(l)
    }
}

/// A tiling of a tensor box by dyadic boxes with one refinement level per axis.
#[derive(Debug, Clone)]
pub struct AnisoMesh {
    base: [usize; 3],
    axis_x: [Vec<f64>; 3],
    refine: [bool; 3],
    max_level: [u8; 3],
    patches: [(String, PatchKind); 6],
    leaves: Vec<AxisLeaf>,
    index: HashMap<AxisLeaf, usize>,
    set: LeafSet,
}

/// An integer box `[lo, hi)` of the finest lattice.
type IBox = ([i64; 3], [i64; 3]);

impl AnisoMesh {
    /// Uniform level-0 mesh. As [`AmrMesh::new`], with one finest level per axis: `max_level[a]` (at most 10) is the finest
    /// level along axis `a` and must be 0 on an axis that is not refined.
    pub fn new(
        coords: [Vec<f64>; 3],
        refine: [bool; 3],
        max_level: [u8; 3],
        patches: [(String, PatchKind); 6],
    ) -> Result<AnisoMesh> {
        for a in 0..3 {
            if max_level[a] > 10 {
                return err(format!("AnisoMesh::new: max_level[{a}] must be at most 10"));
            }
            if !refine[a] && max_level[a] != 0 {
                return err(format!(
                    "AnisoMesh::new: max_level[{a}] must be 0 on an axis that is not refined"
                ));
            }
        }
        for (a, c) in coords.iter().enumerate() {
            if c.len() < 2 {
                return err(format!("AnisoMesh::new: coords[{a}] needs at least two nodes"));
            }
            if c.iter().any(|x| !x.is_finite()) || c.windows(2).any(|w| w[1] <= w[0]) {
                return err(format!(
                    "AnisoMesh::new: coords[{a}] must be finite and strictly increasing"
                ));
            }
        }
        for i in 0..6 {
            for j in 0..i {
                if patches[i].0 == patches[j].0 && patches[i].1 != patches[j].1 {
                    return err(format!(
                        "AnisoMesh::new: patch name '{}' is used with two different kinds",
                        patches[i].0
                    ));
                }
            }
        }
        let base = [coords[0].len() - 1, coords[1].len() - 1, coords[2].len() - 1];
        let mut axis_x: [Vec<f64>; 3] = [vec![], vec![], vec![]];
        for a in 0..3 {
            // the formula of AmrMesh::new: the same lattice point gets the same bits
            if refine[a] {
                let n = base[a];
                let ml = max_level[a];
                let denom = (1u64 << ml) as f64;
                for p in 0..=(n << ml) {
                    let q = p >> ml;
                    let rem = p & ((1usize << ml) - 1);
                    axis_x[a].push(if q == n {
                        coords[a][n]
                    } else {
                        coords[a][q] + (coords[a][q + 1] - coords[a][q]) * (rem as f64 / denom)
                    });
                }
            } else {
                axis_x[a] = coords[a].clone();
            }
        }
        let mut leaves = Vec::new();
        for k in 0..base[2] {
            for j in 0..base[1] {
                for i in 0..base[0] {
                    leaves.push(AxisLeaf {
                        levels: [0; 3],
                        ijk: [i as i64, j as i64, k as i64],
                    });
                }
            }
        }
        let mut m = AnisoMesh {
            base,
            axis_x,
            refine,
            max_level,
            patches,
            leaves: vec![],
            index: HashMap::new(),
            set: LeafSet::default(),
        };
        m.set_leaves(leaves);
        Ok(m)
    }

    /// The same cells as an isotropic lattice: every leaf of level `l` becomes a leaf of level `l` along each refined axis.
    pub fn from_amr(amr: &AmrMesh) -> AnisoMesh {
        let ml = amr.max_level;
        let max_level = [0, 1, 2].map(|a| if amr.refine[a] { ml } else { 0 });
        let leaves = amr
            .leaves
            .iter()
            .map(|l| AxisLeaf {
                levels: [0, 1, 2].map(|a| if amr.refine[a] { l.level } else { 0 }),
                ijk: l.ijk,
            })
            .collect();
        let mut m = AnisoMesh {
            base: amr.base,
            axis_x: amr.axis_x.clone(),
            refine: amr.refine,
            max_level,
            patches: amr.patches.clone(),
            leaves: vec![],
            index: HashMap::new(),
            set: LeafSet::default(),
        };
        m.set_leaves(leaves);
        m
    }

    fn set_leaves(&mut self, mut leaves: Vec<AxisLeaf>) {
        leaves.sort_by_key(|l| {
            let lo = self.lo(l);
            (lo[2], lo[1], lo[0])
        });
        self.index = leaves.iter().enumerate().map(|(i, l)| (*l, i)).collect();
        let mut set = LeafSet::default();
        for l in &leaves {
            set.insert(*l);
        }
        self.set = set;
        self.leaves = leaves;
    }

    /// Number of leaves (cells).
    pub fn n_cells(&self) -> usize {
        self.leaves.len()
    }

    /// The leaves in cell order.
    pub fn leaves(&self) -> &[AxisLeaf] {
        &self.leaves
    }

    /// Per-axis levels of every cell.
    pub fn levels(&self) -> Vec<[u8; 3]> {
        self.leaves.iter().map(|l| l.levels).collect()
    }

    /// Finest allowed level along each axis.
    pub fn max_levels(&self) -> [u8; 3] {
        self.max_level
    }

    /// Which axes are refined.
    pub fn refined_axes(&self) -> [bool; 3] {
        self.refine
    }

    fn step(&self, a: usize, level: u8) -> i64 {
        1i64 << (self.max_level[a] - level)
    }

    fn lo(&self, l: &AxisLeaf) -> [i64; 3] {
        [0, 1, 2].map(|a| l.ijk[a] * self.step(a, l.levels[a]))
    }

    fn hi(&self, l: &AxisLeaf) -> [i64; 3] {
        [0, 1, 2].map(|a| (l.ijk[a] + 1) * self.step(a, l.levels[a]))
    }

    fn ext(&self, a: usize) -> i64 {
        (self.base[a] as i64) << self.max_level[a]
    }

    fn pos(&self, p: [i64; 3]) -> Vec3 {
        [
            self.axis_x[0][p[0] as usize],
            self.axis_x[1][p[1] as usize],
            self.axis_x[2][p[2] as usize],
        ]
    }

    /// The leaf of `set` that contains the lattice cell `p`.
    fn locate(&self, set: &LeafSet, p: [i64; 3]) -> Option<AxisLeaf> {
        for levels in set.combos.keys() {
            let k = AxisLeaf {
                levels: *levels,
                ijk: [0, 1, 2].map(|a| p[a] >> (self.max_level[a] - levels[a])),
            };
            if set.contains(&k) {
                return Some(k);
            }
        }
        None
    }

    /// The distinct leaves of `set` that meet the box `[lo, hi)`, in the order found.
    fn overlapping(&self, set: &LeafSet, lo: [i64; 3], hi: [i64; 3]) -> Vec<AxisLeaf> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack: Vec<IBox> = vec![(lo, hi)];
        while let Some((lo, hi)) = stack.pop() {
            let Some(n) = self.locate(set, lo) else {
                continue;
            };
            let nh = self.hi(&n);
            match (0..3).find(|&a| nh[a] < hi[a]) {
                None => {
                    if seen.insert(n) {
                        out.push(n);
                    }
                }
                Some(a) => {
                    let (mut low_hi, mut up_lo) = (hi, lo);
                    low_hi[a] = nh[a];
                    up_lo[a] = nh[a];
                    stack.push((up_lo, hi));
                    stack.push((lo, low_hi));
                }
            }
        }
        out
    }

    /// The leaves of `set` across side `s` (`2 a + dir`) of `l`; empty on the domain boundary.
    fn side_neighbours(&self, set: &LeafSet, l: &AxisLeaf, s: usize) -> Vec<AxisLeaf> {
        let (a, dir) = (s / 2, s % 2);
        let (mut lo, mut hi) = (self.lo(l), self.hi(l));
        if dir == 1 {
            if hi[a] >= self.ext(a) {
                return vec![];
            }
            lo[a] = hi[a];
            hi[a] += 1;
        } else {
            if lo[a] <= 0 {
                return vec![];
            }
            hi[a] = lo[a];
            lo[a] -= 1;
        }
        self.overlapping(set, lo, hi)
    }

    fn split(&self, l: &AxisLeaf, axes: [bool; 3]) -> Vec<AxisLeaf> {
        let mut out = vec![*l];
        for a in 0..3 {
            if !axes[a] {
                continue;
            }
            let mut next = Vec::with_capacity(out.len() * 2);
            for o in &out {
                for b in 0..2 {
                    let mut c = *o;
                    c.levels[a] += 1;
                    c.ijk[a] = 2 * o.ijk[a] + b;
                    next.push(c);
                }
            }
            out = next;
        }
        out
    }

    /// Whether `l` differs by at most one level along every refined axis from all its face neighbours in `set`.
    fn balanced(&self, set: &LeafSet, l: &AxisLeaf) -> bool {
        for s in 0..6 {
            for n in self.side_neighbours(set, l, s) {
                for t in 0..3 {
                    if self.refine[t] && l.levels[t].abs_diff(n.levels[t]) > 1 {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Refine or coarsen. `marks` has one entry per cell. A cell with `refine` axes is split along those that the lattice
    /// refines and that are below their `max_level` (if there are none the mark is ignored and counted); with `balance`,
    /// leaves that would differ by more than one level along any refined axis from a face neighbour are split along that axis
    /// as well. A cell marked `coarsen` along `t` merges with its buddy along `t` when the buddy is a leaf of this mesh marked
    /// `coarsen` along `t` too and (with `balance`) the merged leaf stays within one level of its face neighbours; one merge
    /// per cell and call, axes tried in the order x, y, z.
    pub fn adapt(&self, marks: &[AxisMark], balance: bool) -> Result<(AnisoMesh, Transfer, AdaptReport)> {
        if marks.len() != self.leaves.len() {
            return err(format!(
                "AnisoMesh::adapt: marks has {} entries, the mesh has {} cells",
                marks.len(),
                self.leaves.len()
            ));
        }
        let mut rep = AdaptReport::default();
        let mut cur = self.set.clone();
        let mut queue: VecDeque<(AxisLeaf, [bool; 3])> = VecDeque::new();
        for (l, m) in self.leaves.iter().zip(marks) {
            let asked = [0, 1, 2].map(|a| m.refine[a] && self.refine[a]);
            if !asked.contains(&true) {
                continue;
            }
            let axes = [0, 1, 2].map(|a| asked[a] && l.levels[a] < self.max_level[a]);
            if axes.contains(&true) {
                queue.push_back((*l, axes));
            } else {
                rep.ignored_at_max_level += 1;
            }
        }
        while let Some((x, axes)) = queue.pop_front() {
            let targets = if cur.contains(&x) {
                vec![x]
            } else {
                self.overlapping(&cur, self.lo(&x), self.hi(&x))
            };
            for y in targets {
                let todo = [0, 1, 2].map(|t| axes[t] && y.levels[t] <= x.levels[t]);
                if !todo.contains(&true) {
                    continue;
                }
                cur.remove(&y);
                let kids = self.split(&y, todo);
                for k in &kids {
                    cur.insert(*k);
                }
                rep.refined += 1;
                if !balance {
                    continue;
                }
                for c in &kids {
                    let mut need_c = [false; 3];
                    for s in 0..6 {
                        for n in self.side_neighbours(&cur, c, s) {
                            let mut need_n = [false; 3];
                            for t in 0..3 {
                                if !self.refine[t] {
                                    continue;
                                }
                                if c.levels[t] >= n.levels[t] + 2 {
                                    need_n[t] = true;
                                }
                                if n.levels[t] >= c.levels[t] + 2 {
                                    need_c[t] = true;
                                }
                            }
                            if need_n.contains(&true) {
                                queue.push_back((n, need_n));
                            }
                        }
                    }
                    if need_c.contains(&true) {
                        queue.push_back((*c, need_c));
                    }
                }
            }
        }
        // coarsening: pairs of buddies along one axis
        let mut used: HashSet<AxisLeaf> = HashSet::new();
        for (l, m) in self.leaves.iter().zip(marks) {
            if m.refine.contains(&true) || used.contains(l) || !cur.contains(l) {
                continue;
            }
            for t in 0..3 {
                if !(m.coarsen[t] && self.refine[t] && l.levels[t] > 0) {
                    continue;
                }
                let mut buddy = *l;
                buddy.ijk[t] ^= 1;
                let agrees = self
                    .index
                    .get(&buddy)
                    .is_some_and(|&j| marks[j].coarsen[t] && !marks[j].refine.contains(&true));
                if !agrees || used.contains(&buddy) || !cur.contains(&buddy) {
                    continue;
                }
                let mut parent = *l;
                parent.levels[t] -= 1;
                parent.ijk[t] >>= 1;
                cur.remove(l);
                cur.remove(&buddy);
                cur.insert(parent);
                if balance && !self.balanced(&cur, &parent) {
                    cur.remove(&parent);
                    cur.insert(*l);
                    cur.insert(buddy);
                    continue;
                }
                used.insert(*l);
                used.insert(buddy);
                rep.coarsened += 1;
                break;
            }
        }
        let mut next = AnisoMesh {
            base: self.base,
            axis_x: self.axis_x.clone(),
            refine: self.refine,
            max_level: self.max_level,
            patches: self.patches.clone(),
            leaves: vec![],
            index: HashMap::new(),
            set: LeafSet::default(),
        };
        next.set_leaves(cur.set.into_iter().collect());
        let mut sources = Vec::with_capacity(next.leaves.len());
        for n in &next.leaves {
            if let Some(&i) = self.index.get(n) {
                sources.push(Source::Same(i));
                continue;
            }
            let (lo, hi) = (next.lo(n), next.hi(n));
            let Some(old) = self.locate(&self.set, lo) else {
                return err("AnisoMesh::adapt: internal error, a new cell outside the old mesh");
            };
            let (ol, oh) = (self.lo(&old), self.hi(&old));
            if (0..3).all(|a| ol[a] <= lo[a] && hi[a] <= oh[a]) {
                sources.push(Source::Descendant(self.index[&old]));
            } else {
                let mut idx: Vec<usize> = self
                    .overlapping(&self.set, lo, hi)
                    .iter()
                    .map(|k| self.index[k])
                    .collect();
                idx.sort_unstable();
                sources.push(Source::Merged(idx));
            }
        }
        Ok((next, Transfer { sources }, rep))
    }

    /// Build the polyhedral mesh. Cells are the leaves in [`AnisoMesh::leaves`] order.
    pub fn to_mesh(&self) -> Result<Mesh> {
        let mut verts: HashSet<[i64; 3]> = HashSet::new();
        for l in &self.leaves {
            let (lo, hi) = (self.lo(l), self.hi(l));
            for p in corners(0, lo[0], lo, hi) {
                verts.insert(p);
            }
            for p in corners(0, hi[0], lo, hi) {
                verts.insert(p);
            }
        }
        struct Face {
            owner: usize,
            neigh: usize,
            flip: bool,
            a: usize,
            plane: i64,
            lo: [i64; 3],
            hi: [i64; 3],
        }
        let mut internal: Vec<Face> = Vec::new();
        let mut bnd: Vec<Vec<usize>> = vec![Vec::new(); 6];
        for (bi, b) in self.leaves.iter().enumerate() {
            let (lo, hi) = (self.lo(b), self.hi(b));
            for s in 0..6 {
                let (a, dir) = (s / 2, s % 2);
                let plane = if dir == 1 { hi[a] } else { lo[a] };
                if plane == 0 || plane == self.ext(a) {
                    bnd[s].push(bi);
                    continue;
                }
                if dir == 0 {
                    continue;
                }
                // internal faces are made from their low side: one rectangle per leaf across the plane
                for n in self.side_neighbours(&self.set, b, s) {
                    let ni = self.index[&n];
                    let (nl, nh) = (self.lo(&n), self.hi(&n));
                    let flo = [0, 1, 2].map(|c| lo[c].max(nl[c]));
                    let fhi = [0, 1, 2].map(|c| hi[c].min(nh[c]));
                    for p in corners(a, plane, flo, fhi) {
                        verts.insert(p);
                    }
                    internal.push(Face {
                        owner: bi.min(ni),
                        neigh: bi.max(ni),
                        flip: ni < bi,
                        a,
                        plane,
                        lo: flo,
                        hi: fhi,
                    });
                }
            }
        }
        internal.sort_by_key(|f| (f.owner, f.neigh));
        let mut ids: BTreeMap<[i64; 3], usize> = BTreeMap::new();
        for p in &verts {
            ids.insert(*p, 0);
        }
        let mut points = Vec::with_capacity(ids.len());
        for (n, (p, id)) in ids.iter_mut().enumerate() {
            *id = n;
            points.push(self.pos(*p));
        }
        let conv = |lp: &[[i64; 3]]| -> Vec<usize> { lp.iter().map(|p| ids[p]).collect() };
        let mut faces = Vec::new();
        let mut owner = Vec::new();
        let mut neighbour = Vec::new();
        for f in &internal {
            let mut l = boundary_loop(f.a, f.plane, f.lo, f.hi, &verts);
            if f.flip {
                l.reverse();
            }
            faces.push(conv(&l));
            owner.push(f.owner);
            neighbour.push(f.neigh);
        }
        // patches in order of first appearance, equal names merged
        let mut groups: Vec<(String, PatchKind, Vec<usize>)> = Vec::new();
        for s in 0..6 {
            let (name, kind) = &self.patches[s];
            match groups.iter_mut().find(|g| &g.0 == name) {
                Some(g) => g.2.push(s),
                None => groups.push((name.clone(), kind.clone(), vec![s])),
            }
        }
        // every loop is made here, after the corners of all face rectangles are in the vertex set
        let mut patches = Vec::new();
        for (name, kind, sides) in groups {
            let mut n = 0;
            for s in sides {
                let (a, dir) = (s / 2, s % 2);
                for c in &bnd[s] {
                    let b = &self.leaves[*c];
                    let (lo, hi) = (self.lo(b), self.hi(b));
                    let plane = if dir == 1 { hi[a] } else { lo[a] };
                    let mut l = boundary_loop(a, plane, lo, hi, &verts);
                    if dir == 0 {
                        l.reverse();
                    }
                    faces.push(conv(&l));
                    owner.push(*c);
                    n += 1;
                }
            }
            patches.push((name, kind, n));
        }
        Mesh::from_face_lists(points, faces, owner, neighbour, patches)
    }
}

/// The four corners of the rectangle of the box `[lo, hi]` in the plane `p[a] = plane`.
fn corners(a: usize, plane: i64, lo: [i64; 3], hi: [i64; 3]) -> [[i64; 3]; 4] {
    let (u, v) = ((a + 1) % 3, (a + 2) % 3);
    let pt = |pu: i64, pv: i64| {
        let mut p = [0i64; 3];
        p[a] = plane;
        p[u] = pu;
        p[v] = pv;
        p
    };
    [pt(lo[u], lo[v]), pt(hi[u], lo[v]), pt(hi[u], hi[v]), pt(lo[u], hi[v])]
}

/// The loop of the rectangle with every vertex of `verts` on its edges (normal along `+a`), in the order of the isotropic
/// lattice.
fn boundary_loop(a: usize, plane: i64, lo: [i64; 3], hi: [i64; 3], verts: &HashSet<[i64; 3]>) -> Vec<[i64; 3]> {
    let (u, v) = ((a + 1) % 3, (a + 2) % 3);
    let cs = corners(a, plane, lo, hi);
    let mut out = Vec::new();
    for e in 0..4 {
        let (s, t) = (cs[e], cs[(e + 1) % 4]);
        out.push(s);
        let ax = if s[u] != t[u] { u } else { v };
        let dir = if t[ax] > s[ax] { 1 } else { -1 };
        let mut p = s;
        p[ax] += dir;
        while p[ax] != t[ax] {
            if verts.contains(&p) {
                out.push(p);
            }
            p[ax] += dir;
        }
    }
    out
}

/// Largest level difference along each axis across any internal face of `mesh` (built from `amr`).
pub fn max_axis_level_jump(amr: &AnisoMesh, mesh: &Mesh) -> [u8; 3] {
    let mut worst = [0u8; 3];
    for f in 0..mesh.n_internal_faces() {
        let (o, n) = (&amr.leaves[mesh.owner()[f]], &amr.leaves[mesh.neighbour()[f]]);
        for t in 0..3 {
            worst[t] = worst[t].max(o.levels[t].abs_diff(n.levels[t]));
        }
    }
    worst
}
