//! Solution-adaptive refinement and coarsening of box meshes with hanging-node polyhedra (F-102, ADR 0054).
//!
//! * [`AmrMesh`]: a forest of octrees (quadtrees when an axis is not refined) over a tensor box with arbitrary monotone base
//!   coordinates; [`AmrMesh::to_mesh`] builds the polyhedral [`Mesh`]. A leaf face next to finer leaves is replaced by the finer
//!   leaves' faces and every face loop contains all existing lattice points on its boundary edges, so every cell is closed and
//!   every face is shared by exactly two cells for any level distribution (2:1 balance is optional, for quality).
//! * Criteria on a cell field: [`gradient_indicator`], [`hessian_indicator`], [`zz_indicator`], [`region_indicator`]; marks
//!   with a dead band and level limits from [`MarkSpec`]; [`combine_marks`].
//! * [`AmrMesh::adapt`] refines/coarsens and returns a [`Transfer`]; [`transfer_field`] moves a cell field conservatively.

use crate::mesh::{Mesh, PatchKind};
use crate::octree::SurfaceIndex;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::{BTreeMap, HashMap, HashSet};

fn err<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::Mesh(msg.into()))
}

/// Adaptation mark of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Merge with the siblings (only when all siblings agree).
    Coarsen,
    /// Leave as it is.
    Keep,
    /// Split into children.
    Refine,
}

/// A leaf of the forest: refinement level (0 = base cell) and the index of the cell on that level's grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Leaf {
    /// Refinement level.
    pub level: u8,
    /// Cell index on the level grid.
    pub ijk: [i64; 3],
}

/// Where a cell of the new mesh comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Unchanged cell (index in the old mesh).
    Same(usize),
    /// Created by refining the old cell with this index (any depth).
    Descendant(usize),
    /// Created by merging these old cells.
    Merged(Vec<usize>),
}

/// Map from the cells of an adapted mesh to the cells of the mesh it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    /// One entry per new cell.
    pub sources: Vec<Source>,
}

/// Counters of one adaptation step.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AdaptReport {
    /// Leaves that were split (marked and forced by the balance).
    pub refined: usize,
    /// Marked `Refine` but already at the finest lattice level (ignored).
    pub ignored_at_max_level: usize,
    /// Parent cells created by merging.
    pub coarsened: usize,
}

/// Forest of octrees over a tensor box.
#[derive(Debug, Clone)]
pub struct AmrMesh {
    base: [usize; 3],
    axis_x: [Vec<f64>; 3],
    refine: [bool; 3],
    max_level: u8,
    patches: [(String, PatchKind); 6],
    leaves: Vec<Leaf>,
    index: HashMap<Leaf, usize>,
}

fn order_key(m: &AmrMesh, l: &Leaf) -> (i64, i64, i64, u8) {
    let lo = m.lo(l);
    (lo[2], lo[1], lo[0], l.level)
}

impl AmrMesh {
    /// Uniform level-0 mesh. `coords[a]` are the base node coordinates (strictly increasing, at least two); `refine[a]` says
    /// whether axis `a` is split (a one-cell-thick slab keeps its third axis unrefined); `max_level` (at most 10) is the finest
    /// allowed level; `patches` name and type the box sides xmin, xmax, ymin, ymax, zmin, zmax (equal names are merged and must
    /// have the same kind).
    pub fn new(
        coords: [Vec<f64>; 3],
        refine: [bool; 3],
        max_level: u8,
        patches: [(String, PatchKind); 6],
    ) -> Result<AmrMesh> {
        if max_level > 10 {
            return err("AmrMesh::new: max_level must be at most 10");
        }
        for (a, c) in coords.iter().enumerate() {
            if c.len() < 2 {
                return err(format!("AmrMesh::new: coords[{a}] needs at least two nodes"));
            }
            if c.iter().any(|x| !x.is_finite()) || c.windows(2).any(|w| w[1] <= w[0]) {
                return err(format!(
                    "AmrMesh::new: coords[{a}] must be finite and strictly increasing"
                ));
            }
        }
        for i in 0..6 {
            for j in 0..i {
                if patches[i].0 == patches[j].0 && patches[i].1 != patches[j].1 {
                    return err(format!(
                        "AmrMesh::new: patch name '{}' is used with two different kinds",
                        patches[i].0
                    ));
                }
            }
        }
        let base = [coords[0].len() - 1, coords[1].len() - 1, coords[2].len() - 1];
        let mut axis_x: [Vec<f64>; 3] = [vec![], vec![], vec![]];
        for a in 0..3 {
            if refine[a] {
                let n = base[a];
                let denom = (1u64 << max_level) as f64;
                for p in 0..=(n << max_level) {
                    let q = p >> max_level;
                    let rem = p & ((1usize << max_level) - 1);
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
        let mut m = AmrMesh {
            base,
            axis_x,
            refine,
            max_level,
            patches,
            leaves: vec![],
            index: HashMap::new(),
        };
        let mut leaves = Vec::new();
        for k in 0..base[2] {
            for j in 0..base[1] {
                for i in 0..base[0] {
                    leaves.push(Leaf {
                        level: 0,
                        ijk: [i as i64, j as i64, k as i64],
                    });
                }
            }
        }
        m.set_leaves(leaves);
        Ok(m)
    }

    fn set_leaves(&mut self, mut leaves: Vec<Leaf>) {
        leaves.sort_by_key(|l| order_key(self, l));
        self.index = leaves.iter().enumerate().map(|(i, l)| (*l, i)).collect();
        self.leaves = leaves;
    }

    /// Number of leaves (cells).
    pub fn n_cells(&self) -> usize {
        self.leaves.len()
    }

    /// The leaves in cell order.
    pub fn leaves(&self) -> &[Leaf] {
        &self.leaves
    }

    /// Level of every cell.
    pub fn levels(&self) -> Vec<u8> {
        self.leaves.iter().map(|l| l.level).collect()
    }

    /// Finest allowed level.
    pub fn max_level(&self) -> u8 {
        self.max_level
    }

    /// Which axes are refined.
    pub fn refined_axes(&self) -> [bool; 3] {
        self.refine
    }

    fn ext(&self, a: usize, level: u8) -> i64 {
        if self.refine[a] {
            (self.base[a] as i64) << level
        } else {
            self.base[a] as i64
        }
    }

    fn step(&self, a: usize, level: u8) -> i64 {
        if self.refine[a] {
            1i64 << (self.max_level - level)
        } else {
            1
        }
    }

    fn lo(&self, l: &Leaf) -> [i64; 3] {
        [0, 1, 2].map(|a| l.ijk[a] * self.step(a, l.level))
    }

    fn hi(&self, l: &Leaf) -> [i64; 3] {
        [0, 1, 2].map(|a| (l.ijk[a] + 1) * self.step(a, l.level))
    }

    fn pos(&self, p: [i64; 3]) -> Vec3 {
        [
            self.axis_x[0][p[0] as usize],
            self.axis_x[1][p[1] as usize],
            self.axis_x[2][p[2] as usize],
        ]
    }

    fn parent(&self, l: &Leaf) -> Leaf {
        let mut ijk = l.ijk;
        for a in 0..3 {
            if self.refine[a] {
                ijk[a] >>= 1;
            }
        }
        Leaf {
            level: l.level - 1,
            ijk,
        }
    }

    fn children(&self, l: &Leaf) -> Vec<Leaf> {
        let mut out = vec![[0i64; 3]];
        for a in 0..3 {
            let mut next = Vec::new();
            for o in &out {
                if self.refine[a] {
                    for b in 0..2 {
                        let mut p = *o;
                        p[a] = 2 * l.ijk[a] + b;
                        next.push(p);
                    }
                } else {
                    let mut p = *o;
                    p[a] = l.ijk[a];
                    next.push(p);
                }
            }
            out = next;
        }
        out.into_iter()
            .map(|ijk| Leaf {
                level: l.level + 1,
                ijk,
            })
            .collect()
    }

    /// Leaf of `set` that is the cell `(level, ijk)` or one of its ancestors.
    fn covering(&self, level: u8, ijk: [i64; 3], set: &HashSet<Leaf>) -> Option<Leaf> {
        for lv in (0..=level).rev() {
            let sh = level - lv;
            let mut p = ijk;
            for a in 0..3 {
                if self.refine[a] {
                    p[a] >>= sh;
                }
            }
            let k = Leaf { level: lv, ijk: p };
            if set.contains(&k) {
                return Some(k);
            }
        }
        None
    }

    /// Refine or coarsen. `marks` has one entry per cell. Cells marked `Refine` split one level (those already at
    /// `max_level` are ignored and counted); with `balance`, face neighbours that would end up more than one level coarser are
    /// split as well. A parent is merged only when all its children are leaves marked `Coarsen` and (with `balance`) the merge
    /// leaves no face neighbour more than one level finer.
    pub fn adapt(&self, marks: &[Mark], balance: bool) -> Result<(AmrMesh, Transfer, AdaptReport)> {
        if marks.len() != self.leaves.len() {
            return err(format!(
                "AmrMesh::adapt: marks has {} entries, the mesh has {} cells",
                marks.len(),
                self.leaves.len()
            ));
        }
        let mut rep = AdaptReport::default();
        let mut cur: HashSet<Leaf> = self.leaves.iter().copied().collect();
        let mut queue: Vec<Leaf> = Vec::new();
        for (l, m) in self.leaves.iter().zip(marks) {
            if *m == Mark::Refine {
                if l.level >= self.max_level {
                    rep.ignored_at_max_level += 1;
                } else {
                    queue.push(*l);
                }
            }
        }
        queue.reverse();
        while let Some(x) = queue.pop() {
            if !cur.remove(&x) {
                continue;
            }
            rep.refined += 1;
            for c in self.children(&x) {
                cur.insert(c);
            }
            if balance {
                for s in 0..6 {
                    let (a, dir) = (s / 2, s % 2);
                    let mut nb = x.ijk;
                    nb[a] += if dir == 1 { 1 } else { -1 };
                    if nb[a] < 0 || nb[a] >= self.ext(a, x.level) {
                        continue;
                    }
                    if let Some(c) = self.covering(x.level, nb, &cur) {
                        if c.level < x.level {
                            queue.push(c);
                        }
                    }
                }
            }
        }
        // coarsening
        let mut parents: BTreeMap<(i64, i64, i64, u8), Leaf> = BTreeMap::new();
        for (l, m) in self.leaves.iter().zip(marks) {
            if *m == Mark::Coarsen && l.level > 0 && cur.contains(l) {
                let p = self.parent(l);
                parents.insert(order_key(self, &p), p);
            }
        }
        for p in parents.values() {
            let kids = self.children(p);
            let ok = kids
                .iter()
                .all(|k| cur.contains(k) && self.index.get(k).is_some_and(|&i| marks[i] == Mark::Coarsen));
            if !ok {
                continue;
            }
            if balance && !self.merge_keeps_balance(p, &cur) {
                continue;
            }
            for k in &kids {
                cur.remove(k);
            }
            cur.insert(*p);
            rep.coarsened += 1;
        }
        let mut next = AmrMesh {
            base: self.base,
            axis_x: self.axis_x.clone(),
            refine: self.refine,
            max_level: self.max_level,
            patches: self.patches.clone(),
            leaves: vec![],
            index: HashMap::new(),
        };
        next.set_leaves(cur.into_iter().collect());
        let mut sources = Vec::with_capacity(next.leaves.len());
        for n in &next.leaves {
            if let Some(&i) = self.index.get(n) {
                sources.push(Source::Same(i));
                continue;
            }
            let mut found = None;
            let mut a = *n;
            while a.level > 0 {
                a = self.parent(&a);
                if let Some(&i) = self.index.get(&a) {
                    found = Some(i);
                    break;
                }
            }
            match found {
                Some(i) => sources.push(Source::Descendant(i)),
                None => {
                    let kids = self.children(n);
                    let mut idx = Vec::new();
                    for k in kids {
                        match self.index.get(&k) {
                            Some(&i) => idx.push(i),
                            None => return err("AmrMesh::adapt: internal error, merged cell with a non-leaf child"),
                        }
                    }
                    sources.push(Source::Merged(idx));
                }
            }
        }
        Ok((next, Transfer { sources }, rep))
    }

    fn merge_keeps_balance(&self, p: &Leaf, cur: &HashSet<Leaf>) -> bool {
        for s in 0..6 {
            let (a, dir) = (s / 2, s % 2);
            let mut nb = p.ijk;
            nb[a] += if dir == 1 { 1 } else { -1 };
            if nb[a] < 0 || nb[a] >= self.ext(a, p.level) {
                continue;
            }
            if self.covering(p.level, nb, cur).is_some() {
                continue;
            }
            // neighbour region subdivided: its children next to the shared face must be leaves
            let nbl = Leaf {
                level: p.level,
                ijk: nb,
            };
            for k in self.children(&nbl) {
                let bit = if self.refine[a] { k.ijk[a] & 1 } else { 0 };
                let near = if dir == 1 { bit == 0 } else { bit == 1 };
                if near && !cur.contains(&k) {
                    return false;
                }
            }
        }
        true
    }

    fn boundary_loop(
        &self,
        a: usize,
        plane: i64,
        lo: [i64; 3],
        hi: [i64; 3],
        verts: &HashSet<[i64; 3]>,
    ) -> Vec<[i64; 3]> {
        let (u, v) = ((a + 1) % 3, (a + 2) % 3);
        let pt = |pu: i64, pv: i64| {
            let mut p = [0i64; 3];
            p[a] = plane;
            p[u] = pu;
            p[v] = pv;
            p
        };
        let corners = [pt(lo[u], lo[v]), pt(hi[u], lo[v]), pt(hi[u], hi[v]), pt(lo[u], hi[v])];
        let mut out = Vec::new();
        for e in 0..4 {
            let (s, t) = (corners[e], corners[(e + 1) % 4]);
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

    /// Build the polyhedral mesh. Cells are the leaves in [`AmrMesh::leaves`] order.
    pub fn to_mesh(&self) -> Result<Mesh> {
        let mut verts: HashSet<[i64; 3]> = HashSet::new();
        for l in &self.leaves {
            let (lo, hi) = (self.lo(l), self.hi(l));
            for c in 0..8 {
                verts.insert([
                    if c & 1 == 0 { lo[0] } else { hi[0] },
                    if c & 2 == 0 { lo[1] } else { hi[1] },
                    if c & 4 == 0 { lo[2] } else { hi[2] },
                ]);
            }
        }
        let set: HashSet<Leaf> = self.leaves.iter().copied().collect();
        struct Face {
            owner: usize,
            neigh: usize,
            lp: Vec<[i64; 3]>,
        }
        let mut internal: Vec<Face> = Vec::new();
        let mut bnd: Vec<Vec<(usize, Vec<[i64; 3]>)>> = vec![Vec::new(); 6];
        for (bi, b) in self.leaves.iter().enumerate() {
            let (lo, hi) = (self.lo(b), self.hi(b));
            for s in 0..6 {
                let (a, dir) = (s / 2, s % 2);
                let mut nb = b.ijk;
                nb[a] += if dir == 1 { 1 } else { -1 };
                let plane = if dir == 1 { hi[a] } else { lo[a] };
                if nb[a] < 0 || nb[a] >= self.ext(a, b.level) {
                    let mut l = self.boundary_loop(a, plane, lo, hi, &verts);
                    if dir == 0 {
                        l.reverse();
                    }
                    bnd[s].push((bi, l));
                    continue;
                }
                let ai = match self.covering(b.level, nb, &set) {
                    None => continue,
                    Some(al) => {
                        if al.level == b.level && dir == 0 {
                            continue;
                        }
                        self.index[&al]
                    }
                };
                let mut l = self.boundary_loop(a, plane, lo, hi, &verts);
                let low = if dir == 1 { bi } else { ai };
                let owner = bi.min(ai);
                if owner != low {
                    l.reverse();
                }
                internal.push(Face {
                    owner,
                    neigh: bi.max(ai),
                    lp: l,
                });
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
            faces.push(conv(&f.lp));
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
        let mut patches = Vec::new();
        for (name, kind, sides) in groups {
            let mut n = 0;
            for s in sides {
                for (c, lp) in &bnd[s] {
                    faces.push(conv(lp));
                    owner.push(*c);
                    n += 1;
                }
            }
            patches.push((name, kind, n));
        }
        Mesh::from_face_lists(points, faces, owner, neighbour, patches)
    }
}

/// Largest level difference across any internal face of `mesh` (built from `amr`).
pub fn max_face_level_jump(amr: &AmrMesh, mesh: &Mesh) -> u8 {
    let lv = amr.levels();
    let mut worst = 0u8;
    for f in 0..mesh.n_internal_faces() {
        let (o, n) = (mesh.owner()[f], mesh.neighbour()[f]);
        worst = worst.max(lv[o].abs_diff(lv[n]));
    }
    worst
}

// -------------------------------------------------------------------------------------------------------------------
// criteria
// -------------------------------------------------------------------------------------------------------------------

fn check_len(who: &str, mesh: &Mesh, field: &[f64]) -> Result<()> {
    if field.len() != mesh.n_cells() {
        return err(format!(
            "{who}: field has {} entries, the mesh has {} cells",
            field.len(),
            mesh.n_cells()
        ));
    }
    if field.iter().any(|x| !x.is_finite()) {
        return err(format!("{who}: field contains a non-finite value"));
    }
    Ok(())
}

fn solve3(a: [[f64; 3]; 3], b: Vec3, axes: [bool; 3]) -> Vec3 {
    let mut m = a;
    let mut r = b;
    let scale = (0..3).map(|i| a[i][i].abs()).fold(0.0, f64::max);
    for i in 0..3 {
        if !axes[i] {
            for j in 0..3 {
                m[i][j] = 0.0;
                m[j][i] = 0.0;
            }
            m[i][i] = 1.0;
            r[i] = 0.0;
        }
    }
    let mut x = [0.0; 3];
    let mut singular = [false; 3];
    for col in 0..3 {
        let mut piv = col;
        for row in col + 1..3 {
            if m[row][col].abs() > m[piv][col].abs() {
                piv = row;
            }
        }
        if m[piv][col].abs() <= 1e-10 * scale.max(1e-300) && axes[col] {
            singular[col] = true;
            m[piv][col] = 1.0;
            for j in 0..3 {
                if j != col {
                    m[piv][j] = 0.0;
                }
            }
            r[piv] = 0.0;
        }
        m.swap(col, piv);
        r.swap(col, piv);
        for row in col + 1..3 {
            let f = m[row][col] / m[col][col];
            for j in col..3 {
                m[row][j] -= f * m[col][j];
            }
            r[row] -= f * r[col];
        }
    }
    for i in (0..3).rev() {
        let mut s = r[i];
        for j in i + 1..3 {
            s -= m[i][j] * x[j];
        }
        x[i] = s / m[i][i];
    }
    for i in 0..3 {
        if singular[i] {
            x[i] = 0.0;
        }
    }
    x
}

/// Least-squares gradient over the face neighbours (weights `1/|d|^2`); exact for linear fields whenever the stencil spans the
/// active axes. Axes with `axes[a] == false` get a zero component.
pub fn lsq_gradient(mesh: &Mesh, field: &[f64], axes: [bool; 3]) -> Result<Vec<Vec3>> {
    check_len("lsq_gradient", mesh, field)?;
    let cc = mesh.cell_centres();
    let mut out = Vec::with_capacity(mesh.n_cells());
    for c in 0..mesh.n_cells() {
        let mut a = [[0.0; 3]; 3];
        let mut b = [0.0; 3];
        for n in mesh.cell_neighbours(c) {
            let d = vec3::sub(cc[n], cc[c]);
            let w = 1.0 / vec3::dot(d, d).max(1e-300);
            let df = field[n] - field[c];
            for i in 0..3 {
                for j in 0..3 {
                    a[i][j] += w * d[i] * d[j];
                }
                b[i] += w * d[i] * df;
            }
        }
        out.push(solve3(a, b, axes));
    }
    Ok(out)
}

/// Cell length: geometric mean of the vertex extents along the active axes.
pub fn cell_lengths(mesh: &Mesh, axes: [bool; 3]) -> Vec<f64> {
    let dim = axes.iter().filter(|&&x| x).count().max(1);
    let pts = mesh.points();
    (0..mesh.n_cells())
        .map(|c| {
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for &f in mesh.cell_faces(c) {
                for &p in mesh.face_points(f) {
                    for a in 0..3 {
                        lo[a] = lo[a].min(pts[p][a]);
                        hi[a] = hi[a].max(pts[p][a]);
                    }
                }
            }
            let mut v = 1.0;
            for a in 0..3 {
                if axes[a] {
                    v *= hi[a] - lo[a];
                }
            }
            v.powf(1.0 / dim as f64)
        })
        .collect()
}

/// `|grad phi|` (times the cell length when `scale_by_length`).
pub fn gradient_indicator(mesh: &Mesh, field: &[f64], axes: [bool; 3], scale_by_length: bool) -> Result<Vec<f64>> {
    let g = lsq_gradient(mesh, field, axes)?;
    let h = cell_lengths(mesh, axes);
    Ok(g.iter()
        .zip(&h)
        .map(|(g, h)| vec3::norm(*g) * if scale_by_length { *h } else { 1.0 })
        .collect())
}

/// Frobenius norm of the Hessian (gradient of the gradient components), times `h^2` when `scale_by_length`.
pub fn hessian_indicator(mesh: &Mesh, field: &[f64], axes: [bool; 3], scale_by_length: bool) -> Result<Vec<f64>> {
    let g = lsq_gradient(mesh, field, axes)?;
    let h = cell_lengths(mesh, axes);
    let mut sq = vec![0.0; mesh.n_cells()];
    for i in 0..3 {
        if !axes[i] {
            continue;
        }
        let gi: Vec<f64> = g.iter().map(|v| v[i]).collect();
        let hi = lsq_gradient(mesh, &gi, axes)?;
        for (s, v) in sq.iter_mut().zip(&hi) {
            *s += vec3::dot(*v, *v);
        }
    }
    Ok(sq
        .iter()
        .zip(&h)
        .map(|(s, h)| s.sqrt() * if scale_by_length { h * h } else { 1.0 })
        .collect())
}

/// Zienkiewicz-Zhu-type error indicator: `eta_c = |g*_c - g_c| sqrt(V_c)` with `g` the least-squares gradient and `g*` its
/// volume-weighted mean over the cell and its face neighbours.
pub fn zz_indicator(mesh: &Mesh, field: &[f64], axes: [bool; 3]) -> Result<Vec<f64>> {
    let g = lsq_gradient(mesh, field, axes)?;
    let vol = mesh.cell_volumes();
    Ok((0..mesh.n_cells())
        .map(|c| {
            let mut s = vec3::scale(g[c], vol[c]);
            let mut w = vol[c];
            for n in mesh.cell_neighbours(c) {
                s = vec3::add(s, vec3::scale(g[n], vol[n]));
                w += vol[n];
            }
            let rec = vec3::scale(s, 1.0 / w);
            vec3::norm(vec3::sub(rec, g[c])) * vol[c].sqrt()
        })
        .collect())
}

/// Geometric region for [`region_indicator`].
pub enum RegionShape<'a> {
    /// Axis-aligned box (closed).
    Box {
        /// Lower corner.
        lo: Vec3,
        /// Upper corner.
        hi: Vec3,
    },
    /// Ball (closed).
    Sphere {
        /// Centre.
        centre: Vec3,
        /// Radius.
        radius: f64,
    },
    /// Slab: distance to a plane at most `half_thickness`.
    Slab {
        /// Point on the plane.
        point: Vec3,
        /// Plane normal.
        normal: Vec3,
        /// Half thickness.
        half_thickness: f64,
    },
    /// Distance to a closed surface at most `cutoff`.
    SurfaceDistance {
        /// Index of the surface.
        index: &'a SurfaceIndex<'a>,
        /// Distance.
        cutoff: f64,
    },
}

/// 1 for cells whose centre lies in the region, else 0.
pub fn region_indicator(mesh: &Mesh, shape: &RegionShape<'_>) -> Result<Vec<f64>> {
    let cc = mesh.cell_centres();
    let inside: Vec<bool> = match shape {
        RegionShape::Box { lo, hi } => {
            if (0..3).any(|a| !lo[a].is_finite() || !hi[a].is_finite() || lo[a] > hi[a]) {
                return err("region_indicator: box needs lo <= hi in every axis");
            }
            cc.iter()
                .map(|p| (0..3).all(|a| p[a] >= lo[a] && p[a] <= hi[a]))
                .collect()
        }
        RegionShape::Sphere { centre, radius } => {
            if !(*radius >= 0.0 && radius.is_finite()) {
                return err("region_indicator: sphere radius must be finite and non-negative");
            }
            cc.iter()
                .map(|p| vec3::norm(vec3::sub(*p, *centre)) <= *radius)
                .collect()
        }
        RegionShape::Slab {
            point,
            normal,
            half_thickness,
        } => {
            if vec3::norm(*normal) == 0.0 || !(*half_thickness >= 0.0 && half_thickness.is_finite()) {
                return err("region_indicator: slab needs a non-zero normal and a finite non-negative half_thickness");
            }
            let n = vec3::unit(*normal);
            cc.iter()
                .map(|p| vec3::dot(vec3::sub(*p, *point), n).abs() <= *half_thickness)
                .collect()
        }
        RegionShape::SurfaceDistance { index, cutoff } => {
            if !(*cutoff >= 0.0 && cutoff.is_finite()) {
                return err("region_indicator: cutoff must be finite and non-negative");
            }
            cc.iter().map(|p| index.dist_within(*p, *cutoff).is_some()).collect()
        }
    };
    Ok(inside.iter().map(|&b| if b { 1.0 } else { 0.0 }).collect())
}

/// Thresholds with a dead band and level limits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkSpec {
    /// Indicator above this refines.
    pub refine_above: f64,
    /// Indicator below this coarsens (must be smaller than `refine_above`; in between the cell is kept: the hysteresis).
    pub coarsen_below: f64,
    /// Cells at this level are never coarsened.
    pub min_level: u8,
    /// Cells at this level are never refined.
    pub max_level: u8,
}

impl MarkSpec {
    /// Marks from an indicator and the level of every cell.
    pub fn marks(&self, indicator: &[f64], levels: &[u8]) -> Result<Vec<Mark>> {
        if !(self.refine_above.is_finite() && self.refine_above >= 0.0) {
            return err("MarkSpec: refine_above must be finite and non-negative");
        }
        if !(self.coarsen_below.is_finite() && self.coarsen_below >= 0.0) {
            return err("MarkSpec: coarsen_below must be finite and non-negative");
        }
        if self.coarsen_below >= self.refine_above {
            return err("MarkSpec: coarsen_below must be smaller than refine_above (dead band)");
        }
        if self.min_level > self.max_level {
            return err("MarkSpec: min_level must not exceed max_level");
        }
        if indicator.len() != levels.len() {
            return err(format!(
                "MarkSpec: indicator has {} entries but levels has {}",
                indicator.len(),
                levels.len()
            ));
        }
        let mut out = Vec::with_capacity(indicator.len());
        for (i, (&v, &l)) in indicator.iter().zip(levels).enumerate() {
            if !v.is_finite() {
                return err(format!("MarkSpec: indicator[{i}] is not finite"));
            }
            out.push(if v > self.refine_above && l < self.max_level {
                Mark::Refine
            } else if v < self.coarsen_below && l > self.min_level {
                Mark::Coarsen
            } else {
                Mark::Keep
            });
        }
        Ok(out)
    }
}

/// Combine several mark arrays: refine if any refines, coarsen only if all coarsen.
pub fn combine_marks(sets: &[&[Mark]]) -> Result<Vec<Mark>> {
    let Some(first) = sets.first() else {
        return err("combine_marks: no mark sets");
    };
    if sets.iter().any(|s| s.len() != first.len()) {
        return err("combine_marks: mark sets have different lengths");
    }
    Ok((0..first.len())
        .map(|i| {
            if sets.iter().any(|s| s[i] == Mark::Refine) {
                Mark::Refine
            } else if sets.iter().all(|s| s[i] == Mark::Coarsen) {
                Mark::Coarsen
            } else {
                Mark::Keep
            }
        })
        .collect())
}

/// How a refined cell takes its value from the parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefineTransfer {
    /// Child = parent value.
    Injection,
    /// Child = parent + least-squares gradient . (child centre - parent centre); exact for linear fields.
    Linear,
}

/// Transfer a cell field to an adapted mesh. Both modes conserve `sum V q` to round-off; merged cells take the
/// volume-weighted mean of the children.
pub fn transfer_field(
    old: &Mesh,
    new: &Mesh,
    tr: &Transfer,
    field: &[f64],
    mode: RefineTransfer,
    axes: [bool; 3],
) -> Result<Vec<f64>> {
    check_len("transfer_field", old, field)?;
    if tr.sources.len() != new.n_cells() {
        return err(format!(
            "transfer_field: transfer has {} entries, the new mesh has {} cells",
            tr.sources.len(),
            new.n_cells()
        ));
    }
    let grad = if mode == RefineTransfer::Linear {
        lsq_gradient(old, field, axes)?
    } else {
        vec![vec3::ZERO; old.n_cells()]
    };
    let (vo, co) = (old.cell_volumes(), old.cell_centres());
    let cn = new.cell_centres();
    let mut out = Vec::with_capacity(new.n_cells());
    for (n, s) in tr.sources.iter().enumerate() {
        out.push(match s {
            Source::Same(i) => field[*i],
            Source::Descendant(i) => field[*i] + vec3::dot(grad[*i], vec3::sub(cn[n], co[*i])),
            Source::Merged(kids) => {
                let (mut sv, mut sq) = (0.0, 0.0);
                for &k in kids {
                    sv += vo[k];
                    sq += vo[k] * field[k];
                }
                sq / sv
            }
        });
    }
    Ok(out)
}

/// Sorted list of the distinct levels present (diagnostics).
pub fn level_histogram(levels: &[u8]) -> BTreeMap<u8, usize> {
    let mut h = BTreeMap::new();
    for &l in levels {
        *h.entry(l).or_insert(0) += 1;
    }
    h
}
