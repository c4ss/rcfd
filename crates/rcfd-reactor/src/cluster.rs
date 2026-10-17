//! Clustering of similar cell states (roadmap F-223, ADR 0053 D3): leader clustering in the scaled max-norm; one representative per
//! cluster is integrated and the members receive its increment.
//!
//! Similarity metric: `d(a, b) = max_k |a_k - b_k| / s_k` with the error scales `s`. A cell joins the first cluster (in creation
//! order) whose representative is within `d <= tau`. With `tau = 0` only bitwise-equal states share a cluster, and a member then
//! receives the representative's mapped state itself, so the result is bit for bit that of the direct integration of every cell.
//! For `tau > 0` a member `m` of representative `r` gets `phi_m + (R(phi_r) - phi_r)`; the increment of an element-conserving map has
//! zero element sums, so the element masses of the member are preserved exactly (to round-off). The error of the transfer is
//! `(A - I)(phi_m - phi_r)` to first order.

use crate::isat::scaled_dist;

/// Result of a clustered advance.
#[derive(Debug, Clone, PartialEq)]
pub struct Clustered {
    /// Mapped state of every cell.
    pub out: Vec<Vec<f64>>,
    /// Cell index of the representative of every cluster.
    pub reps: Vec<usize>,
    /// Cluster index of every cell.
    pub cluster_of: Vec<usize>,
}

impl Clustered {
    /// Number of clusters (= number of integrations).
    pub fn clusters(&self) -> usize {
        self.reps.len()
    }
}

/// Group `states` (scaled max-norm tolerance `tau >= 0`) without integrating.
pub fn cluster(states: &[Vec<f64>], s: &[f64], tau: f64) -> (Vec<usize>, Vec<usize>) {
    let mut reps: Vec<usize> = Vec::new();
    let mut cluster_of = Vec::with_capacity(states.len());
    for (i, st) in states.iter().enumerate() {
        let hit = reps.iter().position(|&r| scaled_dist(st, &states[r], s) <= tau);
        match hit {
            Some(c) => cluster_of.push(c),
            None => {
                cluster_of.push(reps.len());
                reps.push(i);
            }
        }
    }
    (reps, cluster_of)
}

/// Advance all `states` with `map` (called once per cluster representative) and distribute the increments.
pub fn advance_clustered<E>(
    states: &[Vec<f64>],
    s: &[f64],
    tau: f64,
    map: &mut dyn FnMut(&[f64]) -> Result<Vec<f64>, E>,
) -> Result<Clustered, E> {
    let (reps, cluster_of) = cluster(states, s, tau);
    let mapped: Vec<Vec<f64>> = reps.iter().map(|&r| map(&states[r])).collect::<Result<_, E>>()?;
    let mut out = Vec::with_capacity(states.len());
    for (i, st) in states.iter().enumerate() {
        let c = cluster_of[i];
        let rep = &states[reps[c]];
        let r = &mapped[c];
        if reps[c] == i || scaled_dist(st, rep, s) == 0.0 {
            out.push(r.clone());
        } else {
            out.push((0..st.len()).map(|k| st[k] + (r[k] - rep[k])).collect());
        }
    }
    Ok(Clustered { out, reps, cluster_of })
}
