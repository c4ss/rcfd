//! Reverse Cuthill-McKee reordering and bandwidth measures (F-236).
//!
//! The graph is given as a face list (`lower[f]`, `upper[f]`) like [`crate::LduAddressing`]. An ordering is a
//! vector `order` with `order[i]` = old index placed at new position `i`.

use crate::matrix::CsrMatrix;

fn adjacency(n: usize, lower: &[usize], upper: &[usize]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for (&a, &b) in lower.iter().zip(upper) {
        if a != b {
            adj[a].push(b);
            adj[b].push(a);
        }
    }
    for l in adj.iter_mut() {
        l.sort_unstable();
        l.dedup();
    }
    adj
}

/// Breadth-first levels from `root` over the nodes not yet `seen` (a scratch copy is used): returns the
/// level of every reached node as a list of levels.
fn levels_from(adj: &[Vec<usize>], root: usize, mark: &mut [u32], stamp: u32) -> Vec<Vec<usize>> {
    let mut levels = vec![vec![root]];
    mark[root] = stamp;
    loop {
        let mut next = Vec::new();
        for &u in levels.last().expect("non-empty") {
            for &v in &adj[u] {
                if mark[v] != stamp {
                    mark[v] = stamp;
                    next.push(v);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        levels.push(next);
    }
    levels
}

/// Reverse Cuthill-McKee ordering of the graph with `n` nodes and edges `(lower[f], upper[f])`.
///
/// Every connected component starts at a pseudo-peripheral node (repeated breadth-first search from a
/// minimum-degree node towards the minimum-degree node of the last level until the number of levels stops
/// growing); nodes are numbered breadth-first with neighbours in ascending degree (ties by index); the
/// complete sequence is reversed. Deterministic.
pub fn rcm_order(n: usize, lower: &[usize], upper: &[usize]) -> Vec<usize> {
    let adj = adjacency(n, lower, upper);
    let degree: Vec<usize> = adj.iter().map(|l| l.len()).collect();
    let mut visited = vec![false; n];
    let mut mark = vec![0u32; n];
    let mut stamp = 0u32;
    let mut order = Vec::with_capacity(n);
    // Components are started from the unvisited node of smallest degree (ties by index).
    let mut by_degree: Vec<usize> = (0..n).collect();
    by_degree.sort_by_key(|&i| (degree[i], i));
    for &seed in &by_degree {
        if visited[seed] {
            continue;
        }
        // pseudo-peripheral search
        let mut root = seed;
        stamp += 1;
        let mut levels = levels_from(&adj, root, &mut mark, stamp);
        loop {
            let last = levels.last().expect("non-empty");
            let cand = *last.iter().min_by_key(|&&i| (degree[i], i)).expect("non-empty level");
            stamp += 1;
            let lv = levels_from(&adj, cand, &mut mark, stamp);
            if lv.len() > levels.len() {
                root = cand;
                levels = lv;
            } else {
                break;
            }
        }
        // Cuthill-McKee numbering from `root`
        let start = order.len();
        order.push(root);
        visited[root] = true;
        let mut head = start;
        while head < order.len() {
            let u = order[head];
            head += 1;
            let mut nb: Vec<usize> = adj[u].iter().copied().filter(|&v| !visited[v]).collect();
            nb.sort_by_key(|&v| (degree[v], v));
            for v in nb {
                visited[v] = true;
                order.push(v);
            }
        }
    }
    order.reverse();
    order
}

/// Inverse of a permutation: `inv[order[i]] = i`.
pub fn inverse_permutation(order: &[usize]) -> Vec<usize> {
    let mut inv = vec![0; order.len()];
    for (i, &o) in order.iter().enumerate() {
        inv[o] = i;
    }
    inv
}

/// Bandwidth `max |new(a) - new(b)|` over the edges `(lower[f], upper[f])` after renumbering with `order`
/// (`None` = keep the labels).
pub fn bandwidth(lower: &[usize], upper: &[usize], order: Option<&[usize]>) -> usize {
    let inv = order.map(inverse_permutation);
    lower
        .iter()
        .zip(upper)
        .map(|(&a, &b)| match &inv {
            Some(inv) => inv[a].abs_diff(inv[b]),
            None => a.abs_diff(b),
        })
        .max()
        .unwrap_or(0)
}

impl CsrMatrix {
    /// Largest `|row - col|` over the stored entries.
    pub fn bandwidth(&self) -> usize {
        let mut bw = 0;
        for r in 0..self.n {
            for k in self.row_ptr[r]..self.row_ptr[r + 1] {
                bw = bw.max(r.abs_diff(self.col[k]));
            }
        }
        bw
    }

    /// Symmetric permutation `B = P A P^T` with `B[i][j] = A[order[i]][order[j]]` (square matrices).
    pub fn permute_symmetric(&self, order: &[usize]) -> CsrMatrix {
        assert_eq!(order.len(), self.n, "permutation length");
        let inv = inverse_permutation(order);
        let mut t = Vec::with_capacity(self.nnz());
        for r in 0..self.n {
            for k in self.row_ptr[r]..self.row_ptr[r + 1] {
                t.push((inv[r], inv[self.col[k]], self.val[k]));
            }
        }
        CsrMatrix::from_triplets(self.n, t)
    }
}
