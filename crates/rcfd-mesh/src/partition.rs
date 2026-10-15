//! Mesh partitioning and halo (ghost-cell) structures (F-016).
//!
//! The [`Partitioner`] trait is the seam for wrapping an external graph
//! partitioner (METIS-like) later; the built-in implementation is weighted
//! recursive coordinate bisection (RCB), which handles any number of parts
//! with proportional splits. [`Decomposition`] builds per-part local numbering
//! (owned cells first, then ghost cells grouped by owning peer) and
//! send/receive lists; [`Decomposition::exchange`] performs the halo exchange
//! for in-process data, mirroring what an MPI implementation would do.

use crate::mesh::Mesh;
use rcfd_core::vec3::{self, Vec3};
use rcfd_core::{Error, Result};
use std::collections::BTreeMap;

/// Strategy that assigns every cell to one of `n_parts` parts.
pub trait Partitioner {
    /// Return the part of each cell (values in `0..n_parts`).
    fn partition(&self, mesh: &Mesh, n_parts: usize) -> Result<Vec<usize>>;
}

/// Recursive coordinate bisection along the longest extent.
#[derive(Debug, Clone, Copy, Default)]
pub struct Rcb;

impl Partitioner for Rcb {
    fn partition(&self, mesh: &Mesh, n_parts: usize) -> Result<Vec<usize>> {
        if n_parts == 0 || n_parts > mesh.n_cells() {
            return Err(Error::Mesh(format!(
                "cannot split {} cells into {} parts",
                mesh.n_cells(),
                n_parts
            )));
        }
        let mut part = vec![0; mesh.n_cells()];
        let cells: Vec<usize> = (0..mesh.n_cells()).collect();
        bisect(mesh, cells, 0, n_parts, &mut part);
        Ok(part)
    }
}

fn bisect(mesh: &Mesh, mut cells: Vec<usize>, first: usize, n_parts: usize, out: &mut [usize]) {
    if n_parts == 1 {
        for c in cells {
            out[c] = first;
        }
        return;
    }
    let cc = mesh.cell_centres();
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for &c in &cells {
        for d in 0..3 {
            lo[d] = lo[d].min(cc[c][d]);
            hi[d] = hi[d].max(cc[c][d]);
        }
    }
    let axis = (0..3)
        .max_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b])))
        .unwrap_or(0);
    cells.sort_by(|&a, &b| cc[a][axis].total_cmp(&cc[b][axis]).then(a.cmp(&b)));
    let left_parts = n_parts / 2;
    let n_left = (cells.len() * left_parts + n_parts / 2) / n_parts;
    let n_left = n_left.clamp(left_parts, cells.len() - (n_parts - left_parts));
    let right = cells.split_off(n_left);
    bisect(mesh, cells, first, left_parts, out);
    bisect(mesh, right, first + left_parts, n_parts - left_parts, out);
}

/// Halo information of one part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubDomain {
    /// Part index.
    pub part: usize,
    /// Global ids of owned cells (local ids `0..n_owned`).
    pub owned: Vec<usize>,
    /// Global ids of ghost cells (local ids `n_owned..`), grouped by peer in ascending peer order.
    pub ghosts: Vec<usize>,
    /// For each peer, local ids (owned) whose values are sent to it.
    pub send: BTreeMap<usize, Vec<usize>>,
    /// For each peer, local ids (ghost) that are filled from it, in the
    /// order that matches the peer's `send` list for this part.
    pub recv: BTreeMap<usize, Vec<usize>>,
}

impl SubDomain {
    /// Number of owned cells.
    pub fn n_owned(&self) -> usize {
        self.owned.len()
    }
    /// Owned plus ghost cells.
    pub fn n_local(&self) -> usize {
        self.owned.len() + self.ghosts.len()
    }
}

/// Cell-to-part assignment with halo layers (one layer of face neighbours).
#[derive(Debug, Clone)]
pub struct Decomposition {
    /// Number of parts.
    pub n_parts: usize,
    /// Part of each global cell.
    pub part_of_cell: Vec<usize>,
    /// Per-part local structure.
    pub subdomains: Vec<SubDomain>,
    /// Number of internal faces whose cells lie in different parts.
    pub edge_cut: usize,
}

impl Decomposition {
    /// Partition `mesh` with `partitioner` and build halo lists.
    pub fn new(mesh: &Mesh, n_parts: usize, partitioner: &dyn Partitioner) -> Result<Self> {
        let part_of_cell = partitioner.partition(mesh, n_parts)?;
        Self::from_assignment(mesh, n_parts, part_of_cell)
    }

    /// Build halo lists from an explicit assignment.
    pub fn from_assignment(mesh: &Mesh, n_parts: usize, part_of_cell: Vec<usize>) -> Result<Self> {
        if part_of_cell.len() != mesh.n_cells() || part_of_cell.iter().any(|&p| p >= n_parts) {
            return Err(Error::Mesh("invalid part assignment".into()));
        }
        let mut owned: Vec<Vec<usize>> = vec![vec![]; n_parts];
        for (c, &p) in part_of_cell.iter().enumerate() {
            owned[p].push(c);
        }
        // ghosts[p][q] = sorted global cells owned by q that touch part p
        let mut ghosts: Vec<BTreeMap<usize, Vec<usize>>> = vec![BTreeMap::new(); n_parts];
        let mut cut = 0;
        for f in 0..mesh.n_internal_faces() {
            let (a, b) = (mesh.owner()[f], mesh.neighbour()[f]);
            let (pa, pb) = (part_of_cell[a], part_of_cell[b]);
            if pa != pb {
                cut += 1;
                ghosts[pa].entry(pb).or_default().push(b);
                ghosts[pb].entry(pa).or_default().push(a);
            }
        }
        for g in ghosts.iter_mut() {
            for v in g.values_mut() {
                v.sort_unstable();
                v.dedup();
            }
        }
        let mut subs = Vec::new();
        for p in 0..n_parts {
            let mut local_of: BTreeMap<usize, usize> = owned[p].iter().enumerate().map(|(i, &c)| (c, i)).collect();
            let mut gl = Vec::new();
            let mut recv = BTreeMap::new();
            for (&q, cells) in &ghosts[p] {
                let mut ids = Vec::new();
                for &c in cells {
                    let lid = owned[p].len() + gl.len();
                    local_of.insert(c, lid);
                    ids.push(lid);
                    gl.push(c);
                }
                recv.insert(q, ids);
            }
            subs.push((local_of, gl, recv));
        }
        let mut out = Vec::new();
        for p in 0..n_parts {
            // what p sends to q = what q's ghost list from p contains, as p-local ids
            let mut send = BTreeMap::new();
            for &q in subs[p].2.keys() {
                // q's ghost cells from p are global ids owned by p
                let wanted = ghosts[q].get(&p).cloned().unwrap_or_default();
                send.insert(q, wanted.iter().map(|c| subs[p].0[c]).collect());
            }
            let (_, gl, recv) = subs[p].clone();
            out.push(SubDomain {
                part: p,
                owned: owned[p].clone(),
                ghosts: gl,
                send,
                recv,
            });
        }
        Ok(Decomposition {
            n_parts,
            part_of_cell,
            subdomains: out,
            edge_cut: cut,
        })
    }

    /// Load imbalance: `max(owned) / mean(owned) - 1`.
    pub fn imbalance(&self) -> f64 {
        let max = self.subdomains.iter().map(|s| s.n_owned()).max().unwrap_or(0) as f64;
        let mean = self.part_of_cell.len() as f64 / self.n_parts as f64;
        max / mean - 1.0
    }

    /// Scatter a global cell field into per-part local arrays (ghosts zero).
    pub fn scatter(&self, global: &[f64]) -> Vec<Vec<f64>> {
        self.subdomains
            .iter()
            .map(|s| {
                let mut v: Vec<f64> = s.owned.iter().map(|&c| global[c]).collect();
                v.resize(s.n_local(), 0.0);
                v
            })
            .collect()
    }

    /// Halo exchange: fill every part's ghost entries from the owners.
    pub fn exchange(&self, locals: &mut [Vec<f64>]) {
        // Pack all messages first (as an MPI implementation would post sends).
        let mut msgs: Vec<(usize, usize, Vec<f64>)> = Vec::new();
        for s in &self.subdomains {
            for (&q, ids) in &s.send {
                msgs.push((s.part, q, ids.iter().map(|&i| locals[s.part][i]).collect()));
            }
        }
        for (from, to, data) in msgs {
            let ids = &self.subdomains[to].recv[&from];
            debug_assert_eq!(ids.len(), data.len());
            for (&i, v) in ids.iter().zip(data) {
                locals[to][i] = v;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Partitioning strategies (F-236): weighted and rotated bisection variants, radial and
// circumferential sectors, a refinement-based graph partitioner, partition smoothing.
// ---------------------------------------------------------------------------------------------

fn check_args(mesh: &Mesh, n_parts: usize, weights: &Option<Vec<f64>>) -> Result<()> {
    if n_parts == 0 || n_parts > mesh.n_cells() {
        return Err(Error::Mesh(format!(
            "cannot split {} cells into {} parts",
            mesh.n_cells(),
            n_parts
        )));
    }
    if let Some(w) = weights {
        if w.len() != mesh.n_cells() {
            return Err(Error::Mesh(format!(
                "partition weights: {} values for {} cells",
                w.len(),
                mesh.n_cells()
            )));
        }
        if let Some(i) = w.iter().position(|&x| !(x.is_finite() && x > 0.0)) {
            return Err(Error::Mesh(format!(
                "partition weights must be positive and finite (cell {i} has {})",
                w[i]
            )));
        }
    }
    Ok(())
}

fn weight_of(weights: &Option<Vec<f64>>, c: usize) -> f64 {
    weights.as_ref().map_or(1.0, |w| w[c])
}

/// Number of leading entries of `order` that go to the left child of a split with `left_parts` of
/// `n_parts` parts. Without weights the count-proportional rounding of [`Rcb`] is used; with weights
/// the prefix whose weight is nearest to the proportional target (ties to the shorter prefix).
fn split_index(order: &[usize], weights: &Option<Vec<f64>>, left_parts: usize, n_parts: usize) -> usize {
    let len = order.len();
    let (lo, hi) = (left_parts, len - (n_parts - left_parts));
    if weights.is_none() {
        let n_left = (len * left_parts + n_parts / 2) / n_parts;
        return n_left.clamp(lo, hi);
    }
    let total: f64 = order.iter().map(|&c| weight_of(weights, c)).sum();
    let target = total * left_parts as f64 / n_parts as f64;
    let mut prefix = 0.0;
    let mut best = lo;
    let mut best_err = f64::INFINITY;
    for (k, &c) in order.iter().enumerate() {
        prefix += weight_of(weights, c);
        let take = k + 1;
        if take < lo || take > hi {
            continue;
        }
        let err = (prefix - target).abs();
        if err < best_err {
            best_err = err;
            best = take;
        }
    }
    best
}

/// How a [`Bisection`] chooses the cutting direction of each recursion step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BisectionRule {
    /// Coordinate axis of the largest extent of the sub-set (the rule of [`Rcb`]).
    Longest,
    /// Coordinate axes in turn (x, y, z, x, ...) by recursion depth.
    Cycle,
    /// Always the given coordinate axis (0, 1 or 2).
    Fixed(usize),
    /// Principal axis of inertia (largest eigenvector of the weighted covariance of the cell centres).
    Inertial,
}

/// Recursive bisection with a proportional split, optional cell weights and a choice of cutting direction.
#[derive(Debug, Clone)]
pub struct Bisection {
    /// Direction rule.
    pub rule: BisectionRule,
    /// Positive cell weights (all 1 when `None`).
    pub weights: Option<Vec<f64>>,
}

impl Bisection {
    /// Unweighted bisection with the given rule.
    pub fn new(rule: BisectionRule) -> Self {
        Bisection { rule, weights: None }
    }
    /// Attach cell weights.
    pub fn with_weights(mut self, weights: Vec<f64>) -> Self {
        self.weights = Some(weights);
        self
    }
}

/// Largest-eigenvalue unit eigenvector of a symmetric 3x3 matrix (cyclic Jacobi rotations). The sign
/// is fixed so that the component of largest magnitude is positive.
fn principal_axis(m: [[f64; 3]; 3]) -> [f64; 3] {
    let mut a = m;
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..60 {
        let off = a[0][1].abs() + a[0][2].abs() + a[1][2].abs();
        if off < 1e-300 {
            break;
        }
        for (p, q) in [(0usize, 1usize), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-300 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = if theta == 0.0 {
                1.0
            } else {
                theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt())
            };
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for row in a.iter_mut() {
                let (akp, akq) = (row[p], row[q]);
                row[p] = c * akp - s * akq;
                row[q] = s * akp + c * akq;
            }
            for k in 0..3 {
                let (apk, aqk) = (a[p][k], a[q][k]);
                a[p][k] = c * apk - s * aqk;
                a[q][k] = s * apk + c * aqk;
            }
            for row in v.iter_mut() {
                let (vkp, vkq) = (row[p], row[q]);
                row[p] = c * vkp - s * vkq;
                row[q] = s * vkp + c * vkq;
            }
        }
    }
    let imax = (0..3).max_by(|&i, &j| a[i][i].total_cmp(&a[j][j])).unwrap_or(0);
    let mut e = [v[0][imax], v[1][imax], v[2][imax]];
    let big = (0..3).max_by(|&i, &j| e[i].abs().total_cmp(&e[j].abs())).unwrap_or(0);
    if e[big] < 0.0 {
        e = [-e[0], -e[1], -e[2]];
    }
    e
}

impl Bisection {
    fn key_direction(&self, mesh: &Mesh, cells: &[usize], depth: usize) -> ([f64; 3], [f64; 3]) {
        let cc = mesh.cell_centres();
        let axis_dir = |a: usize| {
            let mut d = [0.0; 3];
            d[a] = 1.0;
            d
        };
        match self.rule {
            BisectionRule::Fixed(a) => (axis_dir(a.min(2)), [0.0; 3]),
            BisectionRule::Cycle => (axis_dir(depth % 3), [0.0; 3]),
            BisectionRule::Longest => {
                let mut lo = [f64::INFINITY; 3];
                let mut hi = [f64::NEG_INFINITY; 3];
                for &c in cells {
                    for d in 0..3 {
                        lo[d] = lo[d].min(cc[c][d]);
                        hi[d] = hi[d].max(cc[c][d]);
                    }
                }
                let axis = (0..3)
                    .max_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b])))
                    .unwrap_or(0);
                (axis_dir(axis), [0.0; 3])
            }
            BisectionRule::Inertial => {
                let mut wsum = 0.0;
                let mut mean = [0.0; 3];
                for &c in cells {
                    let w = weight_of(&self.weights, c);
                    wsum += w;
                    for d in 0..3 {
                        mean[d] += w * cc[c][d];
                    }
                }
                for m in mean.iter_mut() {
                    *m /= wsum;
                }
                let mut cov = [[0.0; 3]; 3];
                for &c in cells {
                    let w = weight_of(&self.weights, c);
                    let r = [cc[c][0] - mean[0], cc[c][1] - mean[1], cc[c][2] - mean[2]];
                    for i in 0..3 {
                        for j in 0..3 {
                            cov[i][j] += w * r[i] * r[j];
                        }
                    }
                }
                (principal_axis(cov), mean)
            }
        }
    }

    fn recurse(
        &self,
        mesh: &Mesh,
        mut cells: Vec<usize>,
        first: usize,
        n_parts: usize,
        depth: usize,
        out: &mut [usize],
    ) {
        if n_parts == 1 {
            for c in cells {
                out[c] = first;
            }
            return;
        }
        let cc = mesh.cell_centres();
        let (dir, origin) = self.key_direction(mesh, &cells, depth);
        let key = |c: usize| -> f64 {
            let p = cc[c];
            (p[0] - origin[0]) * dir[0] + (p[1] - origin[1]) * dir[1] + (p[2] - origin[2]) * dir[2]
        };
        cells.sort_by(|&a, &b| key(a).total_cmp(&key(b)).then(a.cmp(&b)));
        let left_parts = n_parts / 2;
        let n_left = split_index(&cells, &self.weights, left_parts, n_parts);
        let right = cells.split_off(n_left);
        self.recurse(mesh, cells, first, left_parts, depth + 1, out);
        self.recurse(mesh, right, first + left_parts, n_parts - left_parts, depth + 1, out);
    }
}

impl Partitioner for Bisection {
    fn partition(&self, mesh: &Mesh, n_parts: usize) -> Result<Vec<usize>> {
        check_args(mesh, n_parts, &self.weights)?;
        let mut part = vec![0; mesh.n_cells()];
        self.recurse(mesh, (0..mesh.n_cells()).collect(), 0, n_parts, 0, &mut part);
        Ok(part)
    }
}

/// Key used by [`Sectors`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorKind {
    /// Distance from the axis line: concentric shells.
    Radial,
    /// Angle about the axis: wedges.
    Circumferential,
}

/// Radial or circumferential partitioning about an axis line: cells are ordered by distance from the
/// axis or by angle about it and cut into bands of equal weight.
#[derive(Debug, Clone)]
pub struct Sectors {
    /// Key.
    pub kind: SectorKind,
    /// A point on the axis.
    pub origin: Vec3,
    /// Axis direction (need not be normalised).
    pub axis: Vec3,
    /// Direction from which the angle is measured (projected into the plane normal to the axis).
    pub reference: Vec3,
    /// Positive cell weights (all 1 when `None`).
    pub weights: Option<Vec<f64>>,
}

impl Sectors {
    /// Sectors about the z axis through the origin, angle measured from +x.
    pub fn about_z(kind: SectorKind) -> Self {
        Sectors {
            kind,
            origin: [0.0; 3],
            axis: [0.0, 0.0, 1.0],
            reference: [1.0, 0.0, 0.0],
            weights: None,
        }
    }

    /// Sort key of every cell (distance from the axis, or angle in `[0, 2 pi)`).
    pub fn keys(&self, mesh: &Mesh) -> Result<Vec<f64>> {
        if vec3::norm(self.axis) == 0.0 {
            return Err(Error::Mesh("sector partitioning: zero axis".into()));
        }
        let a = vec3::unit(self.axis);
        let r0 = vec3::sub(self.reference, vec3::scale(a, vec3::dot(self.reference, a)));
        if vec3::norm(r0) < 1e-12 * vec3::norm(self.reference).max(1e-300) && self.kind == SectorKind::Circumferential {
            return Err(Error::Mesh(
                "sector partitioning: the reference direction is parallel to the axis".into(),
            ));
        }
        let e1 = vec3::unit(r0);
        let e2 = vec3::cross(a, e1);
        Ok(mesh
            .cell_centres()
            .iter()
            .map(|&p| {
                let d = vec3::sub(p, self.origin);
                let along = vec3::dot(d, a);
                let perp = vec3::sub(d, vec3::scale(a, along));
                match self.kind {
                    SectorKind::Radial => vec3::norm(perp),
                    SectorKind::Circumferential => {
                        let th = vec3::dot(perp, e2).atan2(vec3::dot(perp, e1));
                        if th < 0.0 {
                            th + 2.0 * std::f64::consts::PI
                        } else {
                            th
                        }
                    }
                }
            })
            .collect())
    }
}

impl Partitioner for Sectors {
    fn partition(&self, mesh: &Mesh, n_parts: usize) -> Result<Vec<usize>> {
        check_args(mesh, n_parts, &self.weights)?;
        let keys = self.keys(mesh)?;
        let mut order: Vec<usize> = (0..mesh.n_cells()).collect();
        order.sort_by(|&a, &b| keys[a].total_cmp(&keys[b]).then(a.cmp(&b)));
        let n = order.len();
        let total: f64 = order.iter().map(|&c| weight_of(&self.weights, c)).sum();
        // Boundary after part p at the prefix whose weight is nearest to (p + 1) W / n_parts, strictly increasing.
        let mut part = vec![0; n];
        let mut start = 0usize;
        let mut prefix_before = 0.0;
        for p in 0..n_parts - 1 {
            let target = total * (p + 1) as f64 / n_parts as f64;
            let lo = start + 1;
            let hi = n - (n_parts - 1 - p);
            let mut acc = prefix_before;
            let mut best = lo;
            let mut best_err = f64::INFINITY;
            let mut best_acc = acc;
            for (k, &c) in order.iter().enumerate().skip(start) {
                acc += weight_of(&self.weights, c);
                let take = k + 1;
                if take < lo {
                    continue;
                }
                if take > hi {
                    break;
                }
                let err = (acc - target).abs();
                if err < best_err {
                    best_err = err;
                    best = take;
                    best_acc = acc;
                }
            }
            for &c in &order[start..best] {
                part[c] = p;
            }
            start = best;
            prefix_before = best_acc;
        }
        for &c in &order[start..] {
            part[c] = n_parts - 1;
        }
        Ok(part)
    }
}

/// Number of internal faces whose two cells lie in different parts.
pub fn edge_cut(mesh: &Mesh, part: &[usize]) -> usize {
    (0..mesh.n_internal_faces())
        .filter(|&f| part[mesh.owner()[f]] != part[mesh.neighbour()[f]])
        .count()
}

/// Total cell weight of each part (unit weights when `None`).
pub fn part_weights(part: &[usize], n_parts: usize, weights: Option<&[f64]>) -> Vec<f64> {
    let mut w = vec![0.0; n_parts];
    for (c, &p) in part.iter().enumerate() {
        w[p] += weights.map_or(1.0, |w| w[c]);
    }
    w
}

/// `max_p W_p / (W / n) - 1`.
pub fn weighted_imbalance(part: &[usize], n_parts: usize, weights: Option<&[f64]>) -> f64 {
    let w = part_weights(part, n_parts, weights);
    let total: f64 = w.iter().sum();
    w.iter().copied().fold(0.0, f64::max) / (total / n_parts as f64) - 1.0
}

/// What [`smooth_partition`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmoothReport {
    /// Number of cell moves made.
    pub moved: usize,
    /// Number of passes run (the last one made no move unless the limit was reached).
    pub passes: usize,
}

/// Greedy cell-move refinement: a cell moves to the neighbouring part holding most of its face
/// neighbours when that is strictly more than it has in its own part (gain > 0), the destination
/// stays within `(1 + tolerance) W / n` and the source keeps a cell. Every move reduces the edge cut.
fn refine(
    mesh: &Mesh,
    part: &mut [usize],
    n_parts: usize,
    weights: Option<&[f64]>,
    tolerance: f64,
    max_passes: usize,
) -> SmoothReport {
    let n = mesh.n_cells();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for f in 0..mesh.n_internal_faces() {
        let (a, b) = (mesh.owner()[f], mesh.neighbour()[f]);
        adj[a].push(b);
        adj[b].push(a);
    }
    let w_of = |c: usize| weights.map_or(1.0, |w| w[c]);
    let mut pw = part_weights(part, n_parts, weights);
    let limit = (1.0 + tolerance) * pw.iter().sum::<f64>() / n_parts as f64;
    let mut count = vec![0usize; n_parts];
    for p in part.iter() {
        count[*p] += 1;
    }
    let mut moved = 0;
    let mut passes = 0;
    while passes < max_passes {
        passes += 1;
        let mut changed = 0;
        for c in 0..n {
            let own = part[c];
            if adj[c].iter().all(|&q| part[q] == own) {
                continue;
            }
            let mut tally: Vec<(usize, usize)> = Vec::new();
            for &q in &adj[c] {
                match tally.iter_mut().find(|t| t.0 == part[q]) {
                    Some(t) => t.1 += 1,
                    None => tally.push((part[q], 1)),
                }
            }
            let in_own = tally.iter().find(|t| t.0 == own).map_or(0, |t| t.1);
            let best = tally
                .iter()
                .filter(|t| t.0 != own)
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .copied();
            let Some((dest, k)) = best else { continue };
            if k > in_own && count[own] > 1 && pw[dest] + w_of(c) <= limit {
                pw[own] -= w_of(c);
                pw[dest] += w_of(c);
                count[own] -= 1;
                count[dest] += 1;
                part[c] = dest;
                changed += 1;
            }
        }
        moved += changed;
        if changed == 0 {
            break;
        }
    }
    SmoothReport { moved, passes }
}

/// Clean up a partition: move cells (including isolated islands) to the part holding most of their
/// face neighbours when that lowers the edge cut and keeps the destination within
/// `(1 + tolerance)` times the mean part weight. Never increases the edge cut.
pub fn smooth_partition(
    mesh: &Mesh,
    part: &mut [usize],
    n_parts: usize,
    weights: Option<&[f64]>,
    tolerance: f64,
    max_passes: usize,
) -> Result<SmoothReport> {
    if part.len() != mesh.n_cells() || part.iter().any(|&p| p >= n_parts) {
        return Err(Error::Mesh("invalid part assignment".into()));
    }
    if weights.is_some_and(|w| w.len() != mesh.n_cells()) {
        return Err(Error::Mesh("partition weights: wrong length".into()));
    }
    Ok(refine(mesh, part, n_parts, weights, tolerance, max_passes))
}

/// Graph partitioner: a weighted recursive coordinate bisection seed (identical to [`Rcb`] for unit
/// weights) followed by cut-reducing boundary refinement on the cell-face graph. Not multilevel.
#[derive(Debug, Clone)]
pub struct GraphPartitioner {
    /// Positive cell weights (all 1 when `None`).
    pub weights: Option<Vec<f64>>,
    /// Allowed excess of a part weight over the mean during refinement.
    pub tolerance: f64,
    /// Maximum number of refinement passes.
    pub max_passes: usize,
}

impl Default for GraphPartitioner {
    fn default() -> Self {
        GraphPartitioner {
            weights: None,
            tolerance: 0.03,
            max_passes: 20,
        }
    }
}

impl Partitioner for GraphPartitioner {
    fn partition(&self, mesh: &Mesh, n_parts: usize) -> Result<Vec<usize>> {
        check_args(mesh, n_parts, &self.weights)?;
        let seed = Bisection {
            rule: BisectionRule::Longest,
            weights: self.weights.clone(),
        };
        let mut part = seed.partition(mesh, n_parts)?;
        refine(
            mesh,
            &mut part,
            n_parts,
            self.weights.as_deref(),
            self.tolerance,
            self.max_passes,
        );
        Ok(part)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{box_mesh, Grading};

    fn mesh() -> Mesh {
        box_mesh([20, 10, 5], [2.0, 1.0, 0.5], [Grading::Uniform; 3]).unwrap()
    }

    #[test]
    fn balance_within_five_percent_for_many_part_counts() {
        let m = mesh();
        for n in [2, 3, 4, 5, 7, 8, 13, 16] {
            let d = Decomposition::new(&m, n, &Rcb).unwrap();
            assert_eq!(d.subdomains.iter().map(|s| s.n_owned()).sum::<usize>(), m.n_cells());
            assert!(d.imbalance() < 0.05, "n={n} imbalance {}", d.imbalance());
            assert!(d.subdomains.iter().all(|s| s.n_owned() > 0));
        }
    }

    #[test]
    fn rcb_cuts_a_slab_minimally() {
        let m = mesh();
        let d = Decomposition::new(&m, 2, &Rcb).unwrap();
        // split across the long x-axis: cut face count = ny*nz
        assert_eq!(d.edge_cut, 10 * 5);
        assert!(Decomposition::new(&m, 0, &Rcb).is_err());
        assert!(Decomposition::new(&m, m.n_cells() + 1, &Rcb).is_err());
    }

    #[test]
    fn halo_exchange_reproduces_global_values() {
        let m = mesh();
        let d = Decomposition::new(&m, 6, &Rcb).unwrap();
        let global: Vec<f64> = (0..m.n_cells())
            .map(|c| 3.0 * m.cell_centres()[c][0] + m.cell_centres()[c][1] + 100.0 * c as f64)
            .collect();
        let mut locals = d.scatter(&global);
        // ghosts start at zero
        assert!(d
            .subdomains
            .iter()
            .all(|s| s.ghosts.is_empty() || locals[s.part][s.n_owned()] == 0.0));
        d.exchange(&mut locals);
        for s in &d.subdomains {
            for (i, &g) in s.ghosts.iter().enumerate() {
                assert_eq!(locals[s.part][s.n_owned() + i], global[g]);
            }
            // every cell adjacent to a foreign cell has that neighbour among ghosts
            for (li, &c) in s.owned.iter().enumerate() {
                for nb in m.cell_neighbours(c) {
                    if d.part_of_cell[nb] != s.part {
                        assert!(s.ghosts.contains(&nb), "cell {c} (local {li}) missing ghost {nb}");
                    }
                }
            }
            // send/recv sizes are symmetric between peers
            for (&q, ids) in &s.send {
                assert_eq!(d.subdomains[q].recv[&s.part].len(), ids.len());
            }
        }
    }

    #[test]
    fn explicit_assignment_validation() {
        let m = mesh();
        assert!(Decomposition::from_assignment(&m, 2, vec![0; 3]).is_err());
        assert!(Decomposition::from_assignment(&m, 2, vec![5; m.n_cells()]).is_err());
    }
}
