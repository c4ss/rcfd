//! Halo exchange over the partitioning of `rcfd_mesh::partition` (ADR 0058, D4).

use crate::comm::{CommError, Communicator, TAG_HALO};
use rcfd_mesh::partition::SubDomain;

/// Send and receive lists of one rank (local ids), ascending by peer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HaloPlan {
    /// This rank (part).
    pub rank: usize,
    /// Number of owned cells (local ids `0..n_owned`).
    pub n_owned: usize,
    /// Owned plus ghost cells.
    pub n_local: usize,
    /// `(peer, owned local ids to send)`.
    pub send: Vec<(usize, Vec<usize>)>,
    /// `(peer, ghost local ids to fill)`, in the order that matches the peer's send list.
    pub recv: Vec<(usize, Vec<usize>)>,
}

impl HaloPlan {
    /// Plan of one sub-domain.
    pub fn from_subdomain(s: &SubDomain) -> Self {
        HaloPlan {
            rank: s.part,
            n_owned: s.n_owned(),
            n_local: s.n_local(),
            send: s.send.iter().map(|(&q, ids)| (q, ids.clone())).collect(),
            recv: s.recv.iter().map(|(&q, ids)| (q, ids.clone())).collect(),
        }
    }

    /// Fill the ghost entries of `local` (length `n_local`) from their owners. Owner values are copied bit-exactly; owned entries
    /// are not modified. All sends are posted before the first receive, receives are taken in ascending peer order.
    pub fn exchange(&self, comm: &dyn Communicator, local: &mut [f64]) -> Result<(), CommError> {
        assert_eq!(local.len(), self.n_local, "halo exchange: local vector length");
        let mut buf = Vec::new();
        for (q, ids) in &self.send {
            buf.clear();
            buf.extend(ids.iter().map(|&i| local[i]));
            comm.send(*q, TAG_HALO, &buf)?;
        }
        for (q, ids) in &self.recv {
            let m = comm.recv(*q, TAG_HALO)?;
            if m.len() != ids.len() {
                return Err(CommError {
                    rank: self.rank,
                    peer: *q,
                    tag: TAG_HALO,
                    reason: format!(
                        "halo message of length {} but {} ghost cells expected",
                        m.len(),
                        ids.len()
                    ),
                });
            }
            for (&i, v) in ids.iter().zip(m) {
                local[i] = v;
            }
        }
        Ok(())
    }
}
