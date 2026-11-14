//! Fill-reducing ordering, elimination tree and sparse `L D L^T` with inertia (F-431, ADR 0447). Declared by ADR 0440
//! (F-408, the scaffold of ADR 0311); filled by ADR 0447 (the plan's ADR 0334).
//!
//! - [`minimum_degree_order`]: approximate minimum degree on the quotient graph (Amestoy, Davis and Duff, SIAM J. Matrix
//!   Anal. Appl. 17 (1996) 886; element absorption, mass elimination and supervariables after George and Liu, SIAM Review 31
//!   (1989) 1, and Liu, ACM Trans. Math. Softw. 11 (1985) 141), ADR 0447 D1.
//! - [`elimination_tree`] and [`factor_count`]: the elimination tree and the column counts of `L` from the row subtrees
//!   (Liu, ACM Trans. Math. Softw. 12 (1986) 127; SIAM J. Matrix Anal. Appl. 11 (1990) 134), ADR 0447 D2.
//! - [`SparseLdlt`]: the up-looking `L D L^T` by rows (Davis, ACM Trans. Math. Softw. 31 (2005) 587), factor once and solve
//!   many, the pivot test and the refusals of the envelope solver (ADR 0274 D4), the inertia, ADR 0447 D3.
//!
//! Every operation is sequential in a fixed order, so the bits of the ordering, the factor and a solution do not depend on the
//! number of threads.

use crate::matrix::CsrMatrix;
use crate::reorder::inverse_permutation;
use crate::skyline::{check_permutation, validate, PivotRule, SkylineError};

/// The work package that fills this module; `None` once it has landed.
pub const PENDING: Option<&'static str> = None;

const NONE: usize = usize::MAX;

/// Order in which the unknowns are eliminated by [`SparseLdlt`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SparseOrdering {
    /// Approximate minimum degree ([`minimum_degree_order`]).
    #[default]
    MinimumDegree,
    /// The input order.
    Natural,
    /// A given permutation: `order[i]` = original index eliminated at position `i`.
    Given(Vec<usize>),
}

/// Options of [`SparseLdlt::factor_with`].
#[derive(Debug, Clone, PartialEq)]
pub struct SparseOptions {
    /// Elimination order.
    pub ordering: SparseOrdering,
    /// Pivot acceptance (as the envelope solver's).
    pub pivots: PivotRule,
    /// Relative symmetry tolerance `delta`: `|a_rc - a_cr| <= delta sqrt(|a_rr a_cc|)`.
    pub symmetry_tolerance: f64,
}

impl Default for SparseOptions {
    fn default() -> Self {
        SparseOptions {
            ordering: SparseOrdering::MinimumDegree,
            pivots: PivotRule::Positive,
            symmetry_tolerance: 1e-12,
        }
    }
}

// ------------------------------------------------------------------------------------------------------- ordering (D1)

/// Status of an index of the quotient graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    /// An uneliminated principal variable.
    Variable,
    /// A variable merged into a supervariable or eliminated with a pivot (its `members` owner lists it).
    Merged,
    /// An element (an eliminated pivot) that is live.
    Element,
    /// An element absorbed into another.
    Absorbed,
}

/// Degree lists: one doubly linked list per degree, insertion at the head.
struct Buckets {
    head: Vec<usize>,
    next: Vec<usize>,
    prev: Vec<usize>,
    degree: Vec<usize>,
}

impl Buckets {
    fn new(n: usize) -> Buckets {
        Buckets {
            head: vec![NONE; n + 1],
            next: vec![NONE; n],
            prev: vec![NONE; n],
            degree: vec![0; n],
        }
    }
    fn insert(&mut self, i: usize, d: usize) {
        self.degree[i] = d;
        let h = self.head[d];
        self.next[i] = h;
        self.prev[i] = NONE;
        if h != NONE {
            self.prev[h] = i;
        }
        self.head[d] = i;
    }
    fn remove(&mut self, i: usize) {
        let (p, q) = (self.prev[i], self.next[i]);
        if p != NONE {
            self.next[p] = q;
        } else {
            self.head[self.degree[i]] = q;
        }
        if q != NONE {
            self.prev[q] = p;
        }
        self.next[i] = NONE;
        self.prev[i] = NONE;
    }
}

/// The symmetric adjacency of the stored pattern of `a` (both triangles, no diagonal), sorted rows.
fn adjacency(a: &CsrMatrix) -> Vec<Vec<usize>> {
    let n = a.n;
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for r in 0..n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            let c = a.col[k];
            if c != r && c < n {
                adj[r].push(c);
                adj[c].push(r);
            }
        }
    }
    for row in adj.iter_mut() {
        row.sort_unstable();
        row.dedup();
    }
    adj
}

/// Approximate minimum degree order of the pattern of the square matrix `a` (both triangles are read): `order[i]` = original
/// index eliminated at position `i` (ADR 0447 D1).
pub fn minimum_degree_order(a: &CsrMatrix) -> Vec<usize> {
    let n = a.n;
    let mut vars = adjacency(a);
    let mut elems: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut clique: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut clique_weight = vec![0usize; n];
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut nv = vec![1usize; n];
    let mut status = vec![Status::Variable; n];
    let mut buckets = Buckets::new(n);
    for (i, v) in vars.iter().enumerate() {
        buckets.insert(i, v.len());
    }
    // Markers by generation (no reset): `mark` for the members of L_p and the set comparisons, `wmark` for w(e).
    let mut mark = vec![0u64; n];
    let mut gen = 0u64;
    let mut wmark = vec![0u64; n];
    let mut wgen = 0u64;
    let mut w = vec![0usize; n];
    let mut ext = vec![0usize; n];
    let mut hash = vec![0usize; n];
    let mut order = Vec::with_capacity(n);
    let mut eliminated = 0usize;
    let mut mindeg = 0usize;
    let mut stack = Vec::new();
    while eliminated < n {
        // 1. the pivot of least approximate degree
        while buckets.head[mindeg] == NONE {
            mindeg += 1;
        }
        let p = buckets.head[mindeg];
        buckets.remove(p);
        // 2. L_p = (A_p U the cliques of E_p) \ {p}; the elements of E_p are absorbed
        gen += 1;
        mark[p] = gen;
        let mut lp = Vec::new();
        for &i in &vars[p] {
            if status[i] == Status::Variable && mark[i] != gen {
                mark[i] = gen;
                lp.push(i);
            }
        }
        let ep = std::mem::take(&mut elems[p]);
        for &e in &ep {
            if status[e] != Status::Element {
                continue;
            }
            for &i in &clique[e] {
                if status[i] == Status::Variable && mark[i] != gen {
                    mark[i] = gen;
                    lp.push(i);
                }
            }
            status[e] = Status::Absorbed;
            clique[e] = Vec::new();
        }
        vars[p] = Vec::new();
        status[p] = Status::Element;
        eliminated += nv[p];
        for &i in &lp {
            buckets.remove(i);
        }
        // 3. w(e) = |L_e \ L_p| (weighted) for the live elements adjacent to L_p
        wgen += 1;
        for &i in &lp {
            for &e in &elems[i] {
                if status[e] != Status::Element {
                    continue;
                }
                if wmark[e] != wgen {
                    wmark[e] = wgen;
                    w[e] = clique_weight[e];
                }
                w[e] -= nv[i];
            }
        }
        // 4. prune E_i and A_i, the external part, mass elimination
        let mut kept = Vec::with_capacity(lp.len());
        for &i in &lp {
            let mut sum = 0usize;
            let mut h = 0usize;
            let mut ei = std::mem::take(&mut elems[i]);
            ei.retain(|&e| {
                if status[e] != Status::Element {
                    return false;
                }
                if w[e] == 0 {
                    // aggressive absorption: L_e lies inside L_p
                    status[e] = Status::Absorbed;
                    clique[e] = Vec::new();
                    return false;
                }
                sum += w[e];
                h = h.wrapping_add(e);
                true
            });
            ei.push(p);
            h = h.wrapping_add(p);
            elems[i] = ei;
            let mut ai = std::mem::take(&mut vars[i]);
            ai.retain(|&j| status[j] == Status::Variable && mark[j] != gen);
            for &j in &ai {
                sum += nv[j];
                h = h.wrapping_add(j);
            }
            vars[i] = ai;
            if sum == 0 {
                // mass elimination: i is adjacent to p's clique only
                status[i] = Status::Merged;
                eliminated += nv[i];
                nv[i] = 0;
                elems[i] = Vec::new();
                members[p].push(i);
            } else {
                ext[i] = sum;
                hash[i] = h % n.max(1);
                kept.push(i);
            }
        }
        // 5. supervariables among the rest of L_p: equal hash, then equal sets
        let mut by_hash = kept.clone();
        by_hash.sort_unstable_by_key(|&i| (hash[i], i));
        let mut s = 0;
        while s < by_hash.len() {
            let mut e_ = s + 1;
            while e_ < by_hash.len() && hash[by_hash[e_]] == hash[by_hash[s]] {
                e_ += 1;
            }
            for a_ in s..e_ {
                let i = by_hash[a_];
                if status[i] != Status::Variable {
                    continue;
                }
                gen += 1;
                for &x in vars[i].iter().chain(&elems[i]) {
                    mark[x] = gen;
                }
                for &j in &by_hash[a_ + 1..e_] {
                    if status[j] != Status::Variable
                        || vars[j].len() != vars[i].len()
                        || elems[j].len() != elems[i].len()
                        || !vars[j].iter().chain(&elems[j]).all(|&x| mark[x] == gen)
                    {
                        continue;
                    }
                    nv[i] += nv[j];
                    nv[j] = 0;
                    status[j] = Status::Merged;
                    vars[j] = Vec::new();
                    elems[j] = Vec::new();
                    members[i].push(j);
                }
            }
            s = e_;
        }
        // 6. the approximate external degrees, the new element
        kept.retain(|&i| status[i] == Status::Variable);
        let lpw: usize = kept.iter().map(|&i| nv[i]).sum();
        let left = n - eliminated;
        for &i in &kept {
            let d = (ext[i] + lpw - nv[i]).min(left - nv[i]);
            buckets.insert(i, d);
            if d < mindeg {
                mindeg = d;
            }
        }
        clique_weight[p] = lpw;
        clique[p] = kept;
        // 7. the order: p, then its members (recursively, in merge order)
        stack.push(p);
        while let Some(v) = stack.pop() {
            order.push(v);
            for &m in members[v].iter().rev() {
                stack.push(m);
            }
            members[v] = Vec::new();
        }
    }
    debug_assert_eq!(order.len(), n);
    order
}

// ------------------------------------------------------------------------------------------------- symbolic (D2)

/// The strict lower triangle of `P A P^T` by rows: for each position `k`, the positions `j < k` with their values (the entry
/// `(order[k], order[j])` if stored, else its transpose), sorted; and the diagonal.
struct Permuted {
    ptr: Vec<usize>,
    idx: Vec<usize>,
    val: Vec<f64>,
    diag: Vec<f64>,
}

fn stored(a: &CsrMatrix, r: usize, c: usize) -> bool {
    let (s, e) = (a.row_ptr[r], a.row_ptr[r + 1]);
    a.col[s..e].binary_search(&c).is_ok()
}

fn permuted_lower(a: &CsrMatrix, order: &[usize]) -> Permuted {
    let n = a.n;
    let inv = inverse_permutation(order);
    let mut count = vec![0usize; n + 1];
    let mut diag = vec![0.0; n];
    let take = |r: usize, k: usize| -> Option<(usize, usize)> {
        let c = a.col[k];
        let (i, j) = (inv[r], inv[c]);
        if j < i {
            Some((i, j))
        } else if j > i && !stored(a, c, r) {
            Some((j, i))
        } else {
            None
        }
    };
    for r in 0..n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            if a.col[k] == r {
                diag[inv[r]] = a.val[k];
            } else if let Some((i, _)) = take(r, k) {
                count[i + 1] += 1;
            }
        }
    }
    for i in 0..n {
        count[i + 1] += count[i];
    }
    let ptr = count.clone();
    let mut fill = count;
    let mut idx = vec![0usize; ptr[n]];
    let mut val = vec![0.0; ptr[n]];
    for r in 0..n {
        for k in a.row_ptr[r]..a.row_ptr[r + 1] {
            if a.col[k] == r {
                continue;
            }
            if let Some((i, j)) = take(r, k) {
                idx[fill[i]] = j;
                val[fill[i]] = a.val[k];
                fill[i] += 1;
            }
        }
    }
    for i in 0..n {
        let (s, e) = (ptr[i], ptr[i + 1]);
        let mut pairs: Vec<(usize, f64)> = idx[s..e].iter().copied().zip(val[s..e].iter().copied()).collect();
        pairs.sort_unstable_by_key(|&(j, _)| j);
        for (q, (j, v)) in pairs.into_iter().enumerate() {
            idx[s + q] = j;
            val[s + q] = v;
        }
    }
    Permuted { ptr, idx, val, diag }
}

/// The elimination tree and the column counts of `L` (strict lower) of the permuted matrix, by the row subtrees.
fn symbolic(c: &Permuted) -> (Vec<usize>, Vec<usize>) {
    let n = c.diag.len();
    let mut parent = vec![NONE; n];
    let mut counts = vec![0usize; n];
    let mut flag = vec![NONE; n];
    for k in 0..n {
        flag[k] = k;
        for &j in &c.idx[c.ptr[k]..c.ptr[k + 1]] {
            let mut i = j;
            while flag[i] != k {
                if parent[i] == NONE {
                    parent[i] = k;
                }
                counts[i] += 1;
                flag[i] = k;
                i = parent[i];
            }
        }
    }
    (parent, counts)
}

fn order_of(a: &CsrMatrix, ordering: &SparseOrdering) -> Result<Vec<usize>, SkylineError> {
    Ok(match ordering {
        SparseOrdering::MinimumDegree => minimum_degree_order(a),
        SparseOrdering::Natural => (0..a.n).collect(),
        SparseOrdering::Given(o) => {
            check_permutation(o, a.n)?;
            o.clone()
        }
    })
}

/// The elimination tree of `P A P^T` (`order[i]` = original index at position `i`): `parent[j]` is the position of the
/// least `k > j` with `l_kj != 0`, `None` for a root (ADR 0447 D2). The pattern of both triangles is used.
///
/// # Panics
/// If `order` is not a permutation of `0..a.n` or `a` is not square.
pub fn elimination_tree(a: &CsrMatrix, order: &[usize]) -> Vec<Option<usize>> {
    assert_eq!(a.n, a.ncols, "square matrix");
    check_permutation(order, a.n).expect("a permutation");
    let (parent, _) = symbolic(&permuted_lower(a, order));
    parent.into_iter().map(|p| (p != NONE).then_some(p)).collect()
}

/// The number of entries of the strict lower factor `L` of `P A P^T` (ADR 0447 D2), from the pattern only.
///
/// # Panics
/// If `order` is not a permutation of `0..a.n` or `a` is not square.
pub fn factor_count(a: &CsrMatrix, order: &[usize]) -> usize {
    assert_eq!(a.n, a.ncols, "square matrix");
    check_permutation(order, a.n).expect("a permutation");
    symbolic(&permuted_lower(a, order)).1.iter().sum()
}

// -------------------------------------------------------------------------------------------------- numeric (D3)

/// Sparse `L D L^T` factor of a symmetric matrix after a fill-reducing ordering.
#[derive(Debug, Clone, PartialEq)]
pub struct SparseLdlt {
    n: usize,
    order: Vec<usize>,
    parent: Vec<usize>,
    col_ptr: Vec<usize>,
    row_idx: Vec<usize>,
    values: Vec<f64>,
    d: Vec<f64>,
}

impl SparseLdlt {
    /// Factorise a symmetric positive definite matrix with the default options (approximate minimum degree, positive
    /// pivots, symmetry tolerance 1e-12).
    pub fn factor(a: &CsrMatrix) -> Result<SparseLdlt, SkylineError> {
        Self::factor_with(a, &SparseOptions::default())
    }

    /// Factorise `a` with the given options. The matrix is validated first (square, finite, symmetric within the
    /// tolerance; the envelope solver's refusals); the lower triangle is used, a missing lower entry taken from the upper.
    pub fn factor_with(a: &CsrMatrix, opts: &SparseOptions) -> Result<SparseLdlt, SkylineError> {
        validate(a, opts.symmetry_tolerance)?;
        let n = a.n;
        let order = order_of(a, &opts.ordering)?;
        let c = permuted_lower(a, &order);
        let (parent, counts) = symbolic(&c);
        let mut col_ptr = vec![0usize; n + 1];
        for j in 0..n {
            col_ptr[j + 1] = col_ptr[j] + counts[j];
        }
        let nnz = col_ptr[n];
        let mut row_idx = vec![0usize; nnz];
        let mut values = vec![0.0; nnz];
        let mut fill = vec![0usize; n];
        let mut d = vec![0.0; n];
        let mut y = vec![0.0; n];
        let mut flag = vec![NONE; n];
        let mut pattern = vec![0usize; n];
        for k in 0..n {
            // the row subtree of k in topological order: pattern[top..n]
            let mut top = n;
            flag[k] = k;
            let (s, e) = (c.ptr[k], c.ptr[k + 1]);
            for q in s..e {
                let mut i = c.idx[q];
                y[i] += c.val[q];
                let mut len = 0;
                while flag[i] != k {
                    pattern[len] = i;
                    len += 1;
                    flag[i] = k;
                    i = parent[i];
                }
                while len > 0 {
                    top -= 1;
                    len -= 1;
                    pattern[top] = pattern[len];
                }
            }
            let a_kk = c.diag[k];
            let mut dk = a_kk;
            let mut sabs = 0.0;
            let width = n - top;
            for t in top..n {
                let i = pattern[t];
                let yi = y[i];
                y[i] = 0.0;
                let (p0, p1) = (col_ptr[i], col_ptr[i] + fill[i]);
                for p in p0..p1 {
                    y[row_idx[p]] -= values[p] * yi;
                }
                let l = yi / d[i];
                let prod = l * yi;
                dk -= prod;
                sabs += prod.abs();
                row_idx[p1] = k;
                values[p1] = l;
                fill[i] += 1;
            }
            let theta = (width as f64 + 1.0) * f64::EPSILON * (a_kk.abs() + sabs);
            let refused = match opts.pivots {
                PivotRule::Positive => dk <= theta,
                PivotRule::NonZero => dk.abs() <= theta,
            };
            if refused || !dk.is_finite() {
                return Err(SkylineError::Pivot {
                    row: order[k],
                    position: k,
                    pivot: dk,
                    threshold: theta,
                });
            }
            d[k] = dk;
        }
        Ok(SparseLdlt {
            n,
            order,
            parent,
            col_ptr,
            row_idx,
            values,
            d,
        })
    }

    /// Dimension.
    pub fn n(&self) -> usize {
        self.n
    }

    /// Elimination order: `order()[i]` = original index at position `i`.
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// Pivots `d_i` in elimination order.
    pub fn pivots(&self) -> &[f64] {
        &self.d
    }

    /// The elimination tree: the parent position of each position (`None` for a root).
    pub fn parent(&self) -> Vec<Option<usize>> {
        self.parent.iter().map(|&p| (p != NONE).then_some(p)).collect()
    }

    /// Entries of the strict lower factor `L` (`nnz(L)`).
    pub fn factor_entries(&self) -> usize {
        self.values.len()
    }

    /// Column `j` of `L` (positions in elimination order, increasing): its row positions and values.
    pub fn column(&self, j: usize) -> (&[usize], &[f64]) {
        let (s, e) = (self.col_ptr[j], self.col_ptr[j + 1]);
        (&self.row_idx[s..e], &self.values[s..e])
    }

    /// Counts of negative and positive pivots (by Sylvester's law of inertia, of eigenvalues when the factorisation is
    /// exact).
    pub fn inertia(&self) -> (usize, usize) {
        let neg = self.d.iter().filter(|&&v| v < 0.0).count();
        (neg, self.n - neg)
    }

    /// Bytes of the factor's arrays, 8 each: values and row indices of `L`, column pointers, `D`, order and parent.
    pub fn storage_bytes(&self) -> usize {
        8 * (2 * self.values.len() + (self.n + 1) + 3 * self.n)
    }

    /// Solve `A x = b`.
    pub fn solve(&self, b: &[f64]) -> Vec<f64> {
        let mut x = vec![0.0; self.n];
        self.solve_into(b, &mut x);
        x
    }

    /// Solve `A x = b` for each right-hand side.
    pub fn solve_many(&self, bs: &[Vec<f64>]) -> Vec<Vec<f64>> {
        bs.iter().map(|b| self.solve(b)).collect()
    }

    /// Solve `A x = b` into `x`.
    pub fn solve_into(&self, b: &[f64], x: &mut [f64]) {
        assert_eq!(b.len(), self.n, "right-hand side length");
        assert_eq!(x.len(), self.n, "solution length");
        let mut z: Vec<f64> = self.order.iter().map(|&o| b[o]).collect();
        for j in 0..self.n {
            let zj = z[j];
            for p in self.col_ptr[j]..self.col_ptr[j + 1] {
                z[self.row_idx[p]] -= self.values[p] * zj;
            }
        }
        for (zj, dj) in z.iter_mut().zip(&self.d) {
            *zj /= dj;
        }
        for j in (0..self.n).rev() {
            let mut s = z[j];
            for p in self.col_ptr[j]..self.col_ptr[j + 1] {
                s -= self.values[p] * z[self.row_idx[p]];
            }
            z[j] = s;
        }
        for (i, &o) in self.order.iter().enumerate() {
            x[o] = z[i];
        }
    }
}
