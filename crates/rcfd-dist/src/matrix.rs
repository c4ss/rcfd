//! Distributed sparse matrix, operator and block-Jacobi preconditioner (ADR 0058, D5, D7).

use crate::comm::{CommError, Communicator};
use crate::halo::HaloPlan;
use rcfd_core::{Error, Result};
use rcfd_linalg::precond::{Ilu0, Preconditioner};
use rcfd_linalg::solvers::LinearOperator;
use rcfd_linalg::CsrMatrix;
use rcfd_mesh::partition::Decomposition;
use std::collections::HashMap;
use std::sync::Mutex;

/// Owned rows of a global matrix with columns remapped to the local numbering (owned cells, then ghosts). Within each row the
/// entries keep the global column order, so the per-row summation order equals the serial one.
#[derive(Debug, Clone)]
pub struct DistCsr {
    /// This rank.
    pub rank: usize,
    /// Number of owned rows.
    pub n_owned: usize,
    /// Owned plus ghost columns.
    pub n_local: usize,
    /// Owned rows; `ncols = n_local` (columns are NOT sorted by local id, they follow the global order).
    pub local: CsrMatrix,
    /// Halo plan of this rank.
    pub plan: HaloPlan,
    /// Global ids of owned cells.
    pub owned: Vec<usize>,
}

impl DistCsr {
    /// Extract the rows of `a` owned by part `rank`. A column that is neither owned nor a ghost is a located error.
    pub fn from_global(a: &CsrMatrix, dec: &Decomposition, rank: usize) -> Result<Self> {
        let s = dec.subdomains.get(rank).ok_or_else(|| {
            Error::Mesh(format!(
                "rank {rank} is not a part of a {}-part decomposition",
                dec.n_parts
            ))
        })?;
        if a.n != dec.part_of_cell.len() || a.ncols != a.n {
            return Err(Error::Mesh(format!(
                "matrix is {}x{} but the decomposition has {} cells",
                a.n,
                a.ncols,
                dec.part_of_cell.len()
            )));
        }
        let mut local_of: HashMap<usize, usize> = HashMap::new();
        for (i, &c) in s.owned.iter().chain(s.ghosts.iter()).enumerate() {
            local_of.insert(c, i);
        }
        let mut row_ptr = vec![0usize];
        let mut col = Vec::new();
        let mut val = Vec::new();
        for (li, &g) in s.owned.iter().enumerate() {
            for j in a.row_ptr[g]..a.row_ptr[g + 1] {
                let Some(&lc) = local_of.get(&a.col[j]) else {
                    return Err(Error::Mesh(format!(
                        "matrix coupling outside the halo: part {rank}, row {g} (local {li}), global column {} is neither owned nor a ghost",
                        a.col[j]
                    )));
                };
                col.push(lc);
                val.push(a.val[j]);
            }
            row_ptr.push(col.len());
        }
        let n_owned = s.n_owned();
        let n_local = s.n_local();
        Ok(DistCsr {
            rank,
            n_owned,
            n_local,
            local: CsrMatrix::rect(n_owned, n_local, row_ptr, col, val),
            plan: HaloPlan::from_subdomain(s),
            owned: s.owned.clone(),
        })
    }

    /// Build from rows assembled on this rank in the local numbering: `rows[i]` lists `(local column, value)` of owned row `i`.
    /// Entries are ordered by the GLOBAL id of the column (like a global CSR matrix) and duplicates are summed in that order.
    pub fn from_rows(sub: &rcfd_mesh::partition::SubDomain, rows: Vec<Vec<(usize, f64)>>) -> Self {
        let global: Vec<usize> = sub.owned.iter().chain(sub.ghosts.iter()).copied().collect();
        let mut row_ptr = vec![0usize];
        let mut col = Vec::new();
        let mut val: Vec<f64> = Vec::new();
        for mut r in rows {
            r.sort_by_key(|&(c, _)| global[c]);
            let mut last = usize::MAX;
            for (c, v) in r {
                if c == last {
                    *val.last_mut().expect("entry") += v;
                } else {
                    col.push(c);
                    val.push(v);
                    last = c;
                }
            }
            row_ptr.push(col.len());
        }
        DistCsr {
            rank: sub.part,
            n_owned: sub.n_owned(),
            n_local: sub.n_local(),
            local: CsrMatrix::rect(sub.n_owned(), sub.n_local(), row_ptr, col, val),
            plan: HaloPlan::from_subdomain(sub),
            owned: sub.owned.clone(),
        }
    }

    /// Diagonal of the owned rows.
    pub fn diag(&self) -> Vec<f64> {
        let l = &self.local;
        (0..self.n_owned)
            .map(|i| {
                (l.row_ptr[i]..l.row_ptr[i + 1])
                    .find(|&j| l.col[j] == i)
                    .map_or(0.0, |j| l.val[j])
            })
            .collect()
    }

    /// The owned-owned diagonal block as a square CSR matrix (ghost columns dropped); local ids of owned cells ascend with global
    /// ids, so the columns are sorted.
    pub fn owned_block(&self) -> CsrMatrix {
        let l = &self.local;
        let mut row_ptr = vec![0usize];
        let mut col = Vec::new();
        let mut val = Vec::new();
        for i in 0..self.n_owned {
            for j in l.row_ptr[i]..l.row_ptr[i + 1] {
                if l.col[j] < self.n_owned {
                    col.push(l.col[j]);
                    val.push(l.val[j]);
                }
            }
            row_ptr.push(col.len());
        }
        CsrMatrix::rect(self.n_owned, self.n_owned, row_ptr, col, val)
    }

    /// `y = A x` for owned rows: `x_local` (length `n_local`, owned part filled) gets its ghosts updated first.
    pub fn matvec(
        &self,
        comm: &dyn Communicator,
        x_local: &mut [f64],
        y: &mut [f64],
    ) -> std::result::Result<(), CommError> {
        self.plan.exchange(comm, x_local)?;
        self.local.matvec(x_local, y);
        Ok(())
    }
}

/// A distributed matrix bound to its communicator. Implements [`LinearOperator`] on owned-length vectors.
pub struct DistOperator<'a> {
    /// The rank's rows.
    pub mat: &'a DistCsr,
    /// Communicator of this rank.
    pub comm: &'a dyn Communicator,
    scratch: Mutex<Vec<f64>>,
}

impl<'a> DistOperator<'a> {
    /// Bind `mat` to `comm`.
    pub fn new(mat: &'a DistCsr, comm: &'a dyn Communicator) -> Self {
        DistOperator {
            mat,
            comm,
            scratch: Mutex::new(vec![0.0; mat.n_local]),
        }
    }

    /// `y = A x` for owned-length `x`, `y`; communication failures are returned.
    pub fn try_apply(&self, x: &[f64], y: &mut [f64]) -> std::result::Result<(), CommError> {
        let mut s = self.scratch.lock().expect("scratch lock");
        s[..self.mat.n_owned].copy_from_slice(x);
        self.mat.matvec(self.comm, &mut s, y)
    }
}

impl LinearOperator for DistOperator<'_> {
    fn size(&self) -> usize {
        self.mat.n_owned
    }
    /// Panics on a communication failure (the trait has no error channel); the Krylov routines of this crate use
    /// [`DistOperator::try_apply`] instead.
    fn apply(&self, x: &[f64], y: &mut [f64]) {
        if let Err(e) = self.try_apply(x, y) {
            panic!("{e}");
        }
    }
}

/// Non-overlapping block-Jacobi (additive Schwarz without overlap): ILU(0) of the owned-owned diagonal block of each rank. No
/// communication; for one rank it equals the global ILU(0). Implements the existing [`Preconditioner`] on owned-length vectors.
pub struct BlockJacobi {
    ilu: Ilu0,
}

impl BlockJacobi {
    /// Factorise the owned block of `mat`.
    pub fn new(mat: &DistCsr) -> Self {
        BlockJacobi {
            ilu: Ilu0::new(&mat.owned_block()),
        }
    }
}

impl Preconditioner for BlockJacobi {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        self.ilu.apply(r, z);
    }
}
