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
