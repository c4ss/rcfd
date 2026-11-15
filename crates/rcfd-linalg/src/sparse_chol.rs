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
//! - [`nested_dissection_order`] and [`nested_dissection_order_with_points`]: nested dissection by level structures or by
//!   coordinates (George, SIAM J. Numer. Anal. 10 (1973) 345; George and Liu, SIAM J. Numer. Anal. 15 (1978) 1053), ADR 0503
//!   D1 and D1b.
//! - [`SupernodalLdlt`]: the multifrontal `L D L^T` by fundamental supernodes (Duff and Reid, ACM Trans. Math. Softw. 9
//!   (1983) 302; Liu, SIAM Review 34 (1992) 82; Liu, Ng and Peyton, SIAM J. Matrix Anal. Appl. 14 (1993) 242), ADR 0503 D2
//!   and D3.
//! - [`ComplexSupernodalLdlt`]: the same kernel in complex symmetric arithmetic without conjugation (ADR 0509 D1, D2), for
//!   the direct harmonic solve.
//!
//! Every operation of the orderings and of [`SparseLdlt`] is sequential in a fixed order, so the bits of the ordering, the
//! factor and a solution do not depend on the number of threads. [`SupernodalLdlt`] splits only the update of a large
//! front's later columns over the rayon pool, each entry with a fixed summation order (ADR 0503 D3), so its bits do not
//! depend on the number of threads either; nor do those of [`ComplexSupernodalLdlt`].

use crate::complex::C64;
use crate::cskyline::{ComplexCsr, ComplexSkylineError};
use crate::matrix::CsrMatrix;
use crate::reorder::inverse_permutation;
use crate::skyline::{check_permutation, validate, PivotRule, SkylineError};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, Sub, SubAssign};

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
    /// Nested dissection by level structures ([`nested_dissection_order`], ADR 0503 D1).
    NestedDissection,
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
struct Permuted<T = f64> {
    ptr: Vec<usize>,
    idx: Vec<usize>,
    val: Vec<T>,
    diag: Vec<T>,
}

fn stored(row_ptr: &[usize], col: &[usize], r: usize, c: usize) -> bool {
    let (s, e) = (row_ptr[r], row_ptr[r + 1]);
    col[s..e].binary_search(&c).is_ok()
}

fn permuted_lower(a: &CsrMatrix, order: &[usize]) -> Permuted {
    permuted_lower_parts(a.n, &a.row_ptr, &a.col, &a.val, order, 0.0)
}

/// [`permuted_lower`] of a square matrix given by its arrays, of either scalar (ADR 0509 D1), or of the indices of the
/// stored entries (the value map of ADR 0566 D1): values are copied, `zero` where nothing is stored.
fn permuted_lower_parts<T: Copy>(
    n: usize,
    row_ptr: &[usize],
    col: &[usize],
    values: &[T],
    order: &[usize],
    zero: T,
) -> Permuted<T> {
    let inv = inverse_permutation(order);
    let mut count = vec![0usize; n + 1];
    let mut diag = vec![zero; n];
    let take = |r: usize, k: usize| -> Option<(usize, usize)> {
        let c = col[k];
        let (i, j) = (inv[r], inv[c]);
        if j < i {
            Some((i, j))
        } else if j > i && !stored(row_ptr, col, c, r) {
            Some((j, i))
        } else {
            None
        }
    };
    for r in 0..n {
        for k in row_ptr[r]..row_ptr[r + 1] {
            if col[k] == r {
                diag[inv[r]] = values[k];
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
    let mut val = vec![zero; ptr[n]];
    for r in 0..n {
        for k in row_ptr[r]..row_ptr[r + 1] {
            if col[k] == r {
                continue;
            }
            if let Some((i, j)) = take(r, k) {
                idx[fill[i]] = j;
                val[fill[i]] = values[k];
                fill[i] += 1;
            }
        }
    }
    for i in 0..n {
        let (s, e) = (ptr[i], ptr[i + 1]);
        let mut pairs: Vec<(usize, T)> = idx[s..e].iter().copied().zip(val[s..e].iter().copied()).collect();
        pairs.sort_unstable_by_key(|&(j, _)| j);
        for (q, (j, v)) in pairs.into_iter().enumerate() {
            idx[s + q] = j;
            val[s + q] = v;
        }
    }
    Permuted { ptr, idx, val, diag }
}

/// The elimination tree and the column counts of `L` (strict lower) of the permuted matrix, by the row subtrees.
fn symbolic<T>(c: &Permuted<T>) -> (Vec<usize>, Vec<usize>) {
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
        SparseOrdering::NestedDissection => nested_dissection_order(a),
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

// ------------------------------------------------------------------------------------- nested dissection (ADR 0503 D1)

/// Parts of at most this many vertices are leaves of the nested dissections, ordered by minimum degree (ADR 0503 D1).
pub const ND_LEAF: usize = 16;

/// How a part is split (ADR 0503 D1 or D1b).
enum Splitter<'a> {
    Levels,
    Points(&'a [[f64; 3]]),
}

/// A part split into `(P1, P2, separator)`, each sorted.
type Split = (Vec<usize>, Vec<usize>, Vec<usize>);

/// The work of a nested dissection: the adjacency, the positions and the markers.
struct Dissection<'a> {
    adj: Vec<Vec<usize>>,
    splitter: Splitter<'a>,
    pos: Vec<usize>,
    in_set: Vec<u64>,
    set_gen: u64,
    seen: Vec<u64>,
    seen_gen: u64,
    side: Vec<bool>,
}

impl Dissection<'_> {
    /// Mark the vertices of `set` as the current part.
    fn enter(&mut self, set: &[usize]) {
        self.set_gen += 1;
        for &v in set {
            self.in_set[v] = self.set_gen;
        }
    }

    /// Breadth-first search inside the current part from `root`: the vertices in visit order with their levels.
    fn bfs(&mut self, root: usize) -> Vec<(usize, usize)> {
        self.seen_gen += 1;
        let g = self.seen_gen;
        self.seen[root] = g;
        let mut out = vec![(root, 0usize)];
        let mut head = 0;
        while head < out.len() {
            let (u, l) = out[head];
            head += 1;
            for &w in &self.adj[u] {
                if self.in_set[w] == self.set_gen && self.seen[w] != g {
                    self.seen[w] = g;
                    out.push((w, l + 1));
                }
            }
        }
        out
    }

    /// Give the vertices of `set` (sorted) the positions `start..` in the order of minimum degree on the induced pattern.
    fn leaf(&mut self, set: &[usize], start: usize) {
        let m = set.len();
        let mut t = Vec::with_capacity(m);
        for (i, &v) in set.iter().enumerate() {
            t.push((i, i, 1.0));
            for &w in &self.adj[v] {
                if let Ok(j) = set.binary_search(&w) {
                    t.push((i, j, 1.0));
                }
            }
        }
        let local = minimum_degree_order(&CsrMatrix::from_triplets(m, t));
        for (q, &i) in local.iter().enumerate() {
            self.pos[set[i]] = start + q;
        }
    }

    /// D1 steps 3 and 4: the least-ratio level of a level structure rooted at a pseudo-peripheral vertex; `None` for a leaf.
    fn split_levels(&mut self, set: &[usize]) -> Option<Split> {
        let ecc = |l: &[(usize, usize)]| l.last().map_or(0, |&(_, h)| h);
        let mut levels = self.bfs(set[0]);
        loop {
            let h = ecc(&levels);
            let next = levels
                .iter()
                .filter(|&&(_, l)| l == h)
                .map(|&(v, _)| v)
                .min()
                .expect("a last level");
            let cand = self.bfs(next);
            if ecc(&cand) > h {
                levels = cand;
            } else {
                break;
            }
        }
        let h = ecc(&levels);
        if h < 2 {
            return None;
        }
        let mut cnt = vec![0u128; h + 1];
        for &(_, l) in &levels {
            cnt[l] += 1;
        }
        let total = set.len() as u128;
        // (|L_l|, below, above, l) of least |L_l| / (below above); ties: least |below - above|, then least l
        let mut best: Option<(u128, u128, u128, usize)> = None;
        let mut below = cnt[0];
        for l in 1..h {
            let above = total - below - cnt[l];
            let better = match best {
                None => true,
                Some((bs, bb, ba, _)) => {
                    let (lhs, rhs) = (cnt[l] * bb * ba, bs * below * above);
                    lhs < rhs || (lhs == rhs && below.abs_diff(above) < bb.abs_diff(ba))
                }
            };
            if better {
                best = Some((cnt[l], below, above, l));
            }
            below += cnt[l];
        }
        let (_, _, _, l) = best.expect("an interior level");
        levels.sort_unstable_by_key(|&(v, _)| v);
        let pick = |keep: &dyn Fn(usize) -> bool| -> Vec<usize> {
            levels.iter().filter(|&&(_, x)| keep(x)).map(|&(v, _)| v).collect()
        };
        Some((pick(&|x| x < l), pick(&|x| x > l), pick(&|x| x == l)))
    }

    /// D1b: the median plane of the axis of largest extent, the smaller one-sided boundary as the separator; `None` for a
    /// leaf.
    fn split_points(&mut self, set: &[usize], p: &[[f64; 3]]) -> Option<Split> {
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for &v in set {
            for d in 0..3 {
                lo[d] = lo[d].min(p[v][d]);
                hi[d] = hi[d].max(p[v][d]);
            }
        }
        let mut axis = 0;
        for d in 1..3 {
            if hi[d] - lo[d] > hi[axis] - lo[axis] {
                axis = d;
            }
        }
        let key = |v: usize| p[v][axis];
        let mut sorted = set.to_vec();
        sorted.sort_unstable_by(|&a, &b| key(a).total_cmp(&key(b)).then(a.cmp(&b)));
        let v = key(sorted[set.len() / 2]);
        let mut in_a: Vec<bool> = set.iter().map(|&x| key(x) < v).collect();
        if !in_a.iter().any(|&b| b) {
            in_a = set.iter().map(|&x| key(x) <= v).collect();
        }
        if in_a.iter().all(|&b| b) {
            return None;
        }
        for (&x, &a) in set.iter().zip(&in_a) {
            self.side[x] = a;
        }
        let g = self.set_gen;
        let touches = |x: usize, want: bool, s: &Self| s.adj[x].iter().any(|&w| s.in_set[w] == g && s.side[w] == want);
        let mut on_sep_a = vec![false; set.len()];
        let mut on_sep_b = vec![false; set.len()];
        let (mut na, mut nb) = (0usize, 0usize);
        for (q, (&x, &a)) in set.iter().zip(&in_a).enumerate() {
            if a && touches(x, false, self) {
                on_sep_a[q] = true;
                na += 1;
            } else if !a && touches(x, true, self) {
                on_sep_b[q] = true;
                nb += 1;
            }
        }
        let on_sep = if na < nb { on_sep_a } else { on_sep_b };
        let (mut p1, mut p2, mut sep) = (Vec::new(), Vec::new(), Vec::new());
        for (q, (&x, &a)) in set.iter().zip(&in_a).enumerate() {
            if on_sep[q] {
                sep.push(x);
            } else if a {
                p1.push(x);
            } else {
                p2.push(x);
            }
        }
        Some((p1, p2, sep))
    }

    /// The work list of D1 step 5.
    fn run(mut self) -> Vec<usize> {
        let n = self.pos.len();
        let mut tasks: Vec<(Vec<usize>, usize)> = vec![((0..n).collect(), n)];
        while let Some((set, end)) = tasks.pop() {
            let start = end - set.len();
            if set.is_empty() {
                continue;
            }
            if set.len() <= ND_LEAF {
                self.leaf(&set, start);
                continue;
            }
            self.enter(&set);
            let reached = self.bfs(set[0]);
            if reached.len() < set.len() {
                let mut p1: Vec<usize> = reached.iter().map(|&(v, _)| v).collect();
                p1.sort_unstable();
                let g = self.seen_gen;
                let p2: Vec<usize> = set.iter().copied().filter(|&v| self.seen[v] != g).collect();
                let e1 = start + p1.len();
                tasks.push((p1, e1));
                tasks.push((p2, end));
                continue;
            }
            let split = match self.splitter {
                Splitter::Levels => self.split_levels(&set),
                Splitter::Points(p) => self.split_points(&set, p),
            };
            match split {
                None => self.leaf(&set, start),
                Some((p1, p2, sep)) => {
                    let s0 = end - sep.len();
                    for (q, &v) in sep.iter().enumerate() {
                        self.pos[v] = s0 + q;
                    }
                    let e1 = start + p1.len();
                    tasks.push((p1, e1));
                    tasks.push((p2, s0));
                }
            }
        }
        let mut order = vec![0usize; n];
        for (v, &p) in self.pos.iter().enumerate() {
            order[p] = v;
        }
        order
    }
}

fn dissect(a: &CsrMatrix, splitter: Splitter<'_>) -> Vec<usize> {
    let n = a.n;
    Dissection {
        adj: adjacency(a),
        splitter,
        pos: vec![0; n],
        in_set: vec![0; n],
        set_gen: 0,
        seen: vec![0; n],
        seen_gen: 0,
        side: vec![false; n],
    }
    .run()
}

/// Nested dissection of the pattern of the square matrix `a` (both triangles are read) by level structures: each part is
/// split by the level of least `|L_l| / (|below| |above|)` of a level structure rooted at a pseudo-peripheral vertex, the
/// separator numbered last; parts of at most [`ND_LEAF`] vertices are ordered by minimum degree (ADR 0503 D1). `order[i]` =
/// original index eliminated at position `i`.
pub fn nested_dissection_order(a: &CsrMatrix) -> Vec<usize> {
    dissect(a, Splitter::Levels)
}

/// Nested dissection of the pattern of `a` by the coordinates `points[i]` of each unknown: each part is split at the median
/// of its axis of largest extent, the smaller one-sided boundary is the separator (ADR 0503 D1b).
///
/// # Panics
/// If `points.len() != a.n` or a coordinate is not finite.
pub fn nested_dissection_order_with_points(a: &CsrMatrix, points: &[[f64; 3]]) -> Vec<usize> {
    assert_eq!(points.len(), a.n, "one point per unknown");
    assert!(points.iter().flatten().all(|x| x.is_finite()), "finite points");
    dissect(a, Splitter::Points(points))
}

// --------------------------------------------------------------------------------- supernodal (ADR 0503 D2, D3)

/// Panel width of the supernodal partial factorisation (ADR 0503 D3).
const NB: usize = 32;

/// The later-column update of a panel is split over the rayon pool from this many multiply-adds (ADR 0503 D3).
const SPLIT_WORK: usize = 1 << 20;

/// The scalar of the supernodal kernel (ADR 0509 D1): `f64` or [`C64`] (complex symmetric, no conjugation).
trait FrontScalar:
    Copy
    + Send
    + Sync
    + PartialEq
    + std::fmt::Debug
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + AddAssign
    + SubAssign
    + DivAssign
{
    /// Zero.
    const ZERO: Self;
}

impl FrontScalar for f64 {
    const ZERO: f64 = 0.0;
}

impl FrontScalar for C64 {
    const ZERO: C64 = C64::ZERO;
}

/// Offset of column `c` in a front of order `m` packed by lower columns.
fn off(c: usize, m: usize) -> usize {
    c * (2 * m - c + 1) / 2
}

/// The dot product with the fixed summation order of ADR 0503 D3: four partial sums, `(s0 + s1) + (s2 + s3)`, the rest in
/// order.
fn dot4<T: FrontScalar>(a: &[T], b: &[T]) -> T {
    let n4 = a.len() / 4 * 4;
    let mut s = [T::ZERO; 4];
    for (x, y) in a[..n4].chunks_exact(4).zip(b[..n4].chunks_exact(4)) {
        s[0] += x[0] * y[0];
        s[1] += x[1] * y[1];
        s[2] += x[2] * y[2];
        s[3] += x[3] * y[3];
    }
    let mut t = (s[0] + s[1]) + (s[2] + s[3]);
    for q in n4..a.len() {
        t += a[q] * b[q];
    }
    t
}

/// The elimination tree, the column counts and the row counts of `L` of the permuted matrix (ADR 0503 D2).
fn symbolic_rows<T>(c: &Permuted<T>) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let n = c.diag.len();
    let mut parent = vec![NONE; n];
    let mut counts = vec![0usize; n];
    let mut rows = vec![0usize; n];
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
                rows[k] += 1;
                flag[i] = k;
                i = parent[i];
            }
        }
    }
    (parent, counts, rows)
}

/// The supernodes, fronts and values of a supernodal factor of either scalar (ADR 0503 D2, D3; ADR 0509 D1).
#[derive(Debug, Clone, PartialEq)]
struct Supernodal<T> {
    /// First column of each supernode, and `n` at the end.
    sn_ptr: Vec<usize>,
    /// The supernode of each column.
    col_sn: Vec<usize>,
    /// The rows of each supernode's front: its columns, then the rows below them.
    rows: Vec<Vec<usize>>,
    /// The packed lower trapezoid of each supernode (the first `n_s` columns of its front; the diagonal slot holds `d`).
    values: Vec<Vec<T>>,
    d: Vec<T>,
    max_front: usize,
    split_fronts: usize,
    peak_front_bytes: usize,
}

/// The column structure of the strict lower triangle of `C`, rows increasing.
struct Columns<T> {
    ptr: Vec<usize>,
    idx: Vec<usize>,
    val: Vec<T>,
}

fn columns_of<T: Copy>(c: &Permuted<T>, zero: T) -> Columns<T> {
    let n = c.diag.len();
    let mut ptr = vec![0usize; n + 1];
    for &j in &c.idx {
        ptr[j + 1] += 1;
    }
    for j in 0..n {
        ptr[j + 1] += ptr[j];
    }
    let mut fill = ptr.clone();
    let mut idx = vec![0usize; c.idx.len()];
    let mut val = vec![zero; c.idx.len()];
    for k in 0..n {
        for q in c.ptr[k]..c.ptr[k + 1] {
            let j = c.idx[q];
            idx[fill[j]] = k;
            val[fill[j]] = c.val[q];
            fill[j] += 1;
        }
    }
    Columns { ptr, idx, val }
}

/// A refused pivot: its elimination position, the pivot, the threshold and the scale of the test.
type Refusal<T> = (usize, T, f64, f64);

/// The partial factorisation of a front's first `ns` columns in panels (ADR 0503 D3 step 2): the pivots go to `d[f..]`;
/// `test(a_kk, d_k, w_k)` returns the threshold and the scale when it refuses the pivot, and the refusal is returned with
/// the front position of the pivot. Returns whether a panel's later-column update was split over the pool.
#[allow(clippy::too_many_arguments)]
fn partial_factor<T: FrontScalar, P: Fn(T, T, usize) -> Option<(f64, f64)>>(
    front: &mut [T],
    m: usize,
    ns: usize,
    f: usize,
    diag: &[T],
    wrow: &[usize],
    test: &P,
    d: &mut [T],
) -> Result<bool, Refusal<T>> {
    let mut split = false;
    let mut wtmp = vec![T::ZERO; m];
    let mut p0 = 0;
    while p0 < ns {
        let p1 = (p0 + NB).min(ns);
        let nb = p1 - p0;
        let tail = m - p1;
        let mut lp = vec![T::ZERO; tail * nb];
        let mut wp = vec![T::ZERO; tail * nb];
        for k in p0..p1 {
            let ok = off(k, m);
            let dk = front[ok];
            let g = f + k;
            if let Some((threshold, scale)) = test(diag[g], dk, wrow[g]) {
                return Err((k, dk, threshold, scale));
            }
            d[g] = dk;
            for t in 1..m - k {
                let w = front[ok + t];
                wtmp[t] = w;
                front[ok + t] = w / dk;
            }
            for i in p1..m {
                lp[(i - p1) * nb + (k - p0)] = front[ok + (i - k)];
                wp[(i - p1) * nb + (k - p0)] = wtmp[i - k];
            }
            for j in k + 1..p1 {
                let wjk = wtmp[j - k];
                let oj = off(j, m);
                let (left, right) = front.split_at_mut(oj);
                let colk = &left[ok + (j - k)..ok + (m - k)];
                for (x, &lik) in right[..m - j].iter_mut().zip(colk) {
                    *x -= lik * wjk;
                }
            }
        }
        if tail > 0 {
            let mut cols: Vec<(usize, &mut [T])> = Vec::with_capacity(tail);
            let mut rest = &mut front[off(p1, m)..];
            for j in p1..m {
                let (col, more) = rest.split_at_mut(m - j);
                cols.push((j, col));
                rest = more;
            }
            let update = |(j, col): (usize, &mut [T])| {
                let wj = &wp[(j - p1) * nb..(j - p1 + 1) * nb];
                for (t, x) in col.iter_mut().enumerate() {
                    let i = j + t;
                    *x -= dot4(&lp[(i - p1) * nb..(i - p1 + 1) * nb], wj);
                }
            };
            if tail * (tail + 1) / 2 * nb >= SPLIT_WORK {
                use rayon::prelude::*;
                split = true;
                cols.into_par_iter().for_each(update);
            } else {
                cols.into_iter().for_each(update);
            }
        }
        p0 = p1;
    }
    Ok(split)
}

/// The symbolic phase of the supernodal factor (ADR 0503 D2), from the pattern of the permuted matrix only (ADR 0566 D1):
/// the row counts, the column structure of the strict lower triangle, the fundamental supernodes, their children and the
/// rows of each front.
#[derive(Debug, Clone, PartialEq)]
struct Symbolic {
    wrow: Vec<usize>,
    cols_ptr: Vec<usize>,
    cols_idx: Vec<usize>,
    sn_ptr: Vec<usize>,
    col_sn: Vec<usize>,
    children: Vec<Vec<usize>>,
    rows: Vec<Vec<usize>>,
}

/// The supernodal factorisation of the permuted matrix `c` (ADR 0503 D2, D3) with the pivot test `test`; a refusal carries
/// the elimination position of the refused pivot. The symbolic phase ([`analyse`]) then the numeric phase ([`numeric`]).
fn supernodal_factor<T: FrontScalar, P: Fn(T, T, usize) -> Option<(f64, f64)>>(
    c: Permuted<T>,
    test: &P,
) -> Result<Supernodal<T>, Refusal<T>> {
    let cols = columns_of(&c, T::ZERO);
    let sym = analyse(&c, cols.ptr, cols.idx);
    let (values, d, max_front, split_fronts, peak) = numeric(&sym, &c.diag, &cols.val, test)?;
    let Symbolic {
        sn_ptr, col_sn, rows, ..
    } = sym;
    Ok(Supernodal {
        sn_ptr,
        col_sn,
        rows,
        values,
        d,
        max_front,
        split_fronts,
        peak_front_bytes: peak,
    })
}

/// The symbolic phase of ADR 0503 D2 on the permuted matrix `c` whose strict lower triangle has the column structure
/// `cols_ptr`, `cols_idx` ([`columns_of`]).
fn analyse<T>(c: &Permuted<T>, cols_ptr: Vec<usize>, cols_idx: Vec<usize>) -> Symbolic {
    let n = c.diag.len();
    let (parent, counts, wrow) = symbolic_rows(c);
    // fundamental supernodes (D2)
    let mut nchild = vec![0usize; n];
    for &p in &parent {
        if p != NONE {
            nchild[p] += 1;
        }
    }
    let mut sn_ptr = vec![0usize];
    for j in 0..n.saturating_sub(1) {
        let joins = parent[j] == j + 1 && counts[j] == counts[j + 1] + 1 && nchild[j + 1] == 1;
        if !joins {
            sn_ptr.push(j + 1);
        }
    }
    if n > 0 {
        sn_ptr.push(n);
    }
    let nsn = sn_ptr.len() - 1;
    let mut col_sn = vec![0usize; n];
    for s in 0..nsn {
        col_sn[sn_ptr[s]..sn_ptr[s + 1]].fill(s);
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nsn];
    for s in 0..nsn {
        let p = parent[sn_ptr[s + 1] - 1];
        if p != NONE {
            children[col_sn[p]].push(s);
        }
    }
    // the rows of each front
    let mut rows: Vec<Vec<usize>> = Vec::with_capacity(nsn);
    let mut mark = vec![NONE; n];
    for s in 0..nsn {
        let (f, l) = (sn_ptr[s], sn_ptr[s + 1]);
        let mut below = Vec::new();
        for j in f..l {
            for &i in &cols_idx[cols_ptr[j]..cols_ptr[j + 1]] {
                if i >= l && mark[i] != s {
                    mark[i] = s;
                    below.push(i);
                }
            }
        }
        for &ch in &children[s] {
            for &i in &rows[ch][sn_ptr[ch + 1] - sn_ptr[ch]..] {
                if i >= l && mark[i] != s {
                    mark[i] = s;
                    below.push(i);
                }
            }
        }
        below.sort_unstable();
        debug_assert_eq!(below.len(), counts[l - 1], "the rows below a supernode");
        let mut r: Vec<usize> = (f..l).collect();
        r.extend(below);
        rows.push(r);
    }
    drop(mark);
    Symbolic {
        wrow,
        cols_ptr,
        cols_idx,
        sn_ptr,
        col_sn,
        children,
        rows,
    }
}

/// The values, pivots, largest front, split fronts and peak front bytes of a numeric factorisation.
type Numeric<T> = (Vec<Vec<T>>, Vec<T>, usize, usize, usize);

/// The numeric phase of ADR 0503 D3 on the symbolic phase `sym`, with the diagonal `diag` of the permuted matrix and the
/// values `colval` of its strict lower triangle in the column structure of `sym`.
fn numeric<T: FrontScalar, P: Fn(T, T, usize) -> Option<(f64, f64)>>(
    sym: &Symbolic,
    diag: &[T],
    colval: &[T],
    test: &P,
) -> Result<Numeric<T>, Refusal<T>> {
    let n = diag.len();
    let Symbolic {
        wrow,
        cols_ptr,
        cols_idx,
        sn_ptr,
        children,
        rows,
        ..
    } = sym;
    let nsn = sn_ptr.len() - 1;
    // numeric (D3)
    let bytes = std::mem::size_of::<T>();
    let mut d = vec![T::ZERO; n];
    let mut values: Vec<Vec<T>> = Vec::with_capacity(nsn);
    let mut pending: Vec<Option<Vec<T>>> = vec![None; nsn];
    let (mut live, mut peak, mut max_front, mut split_fronts) = (0usize, 0usize, 0usize, 0usize);
    let mut local: Vec<usize> = Vec::new();
    for s in 0..nsn {
        let (f, l) = (sn_ptr[s], sn_ptr[s + 1]);
        let ns = l - f;
        let r = &rows[s];
        let m = r.len();
        max_front = max_front.max(m);
        let mut front = vec![T::ZERO; off(m, m)];
        live += bytes * front.len();
        peak = peak.max(live);
        // 1. assembly: the entries of C, then the children's update matrices in increasing order
        for q in 0..ns {
            let j = f + q;
            let oq = off(q, m);
            front[oq] += diag[j];
            for p in cols_ptr[j]..cols_ptr[j + 1] {
                let g = cols_idx[p];
                let li = if g < l {
                    g - f
                } else {
                    ns + r[ns..].binary_search(&g).expect("a row of the front")
                };
                front[oq + (li - q)] += colval[p];
            }
        }
        for &ch in &children[s] {
            let upd = pending[ch].take().expect("a child's update");
            let rc = &rows[ch][sn_ptr[ch + 1] - sn_ptr[ch]..];
            let mc = rc.len();
            local.clear();
            let mut t = ns;
            for &g in rc {
                if g < l {
                    local.push(g - f);
                } else {
                    while r[t] != g {
                        t += 1;
                    }
                    local.push(t);
                }
            }
            for tc in 0..mc {
                let oc = off(tc, mc);
                let lt = local[tc];
                let ol = off(lt, m);
                for u in tc..mc {
                    front[ol + (local[u] - lt)] += upd[oc + (u - tc)];
                }
            }
            live -= bytes * upd.len();
        }
        // 2. partial factorisation
        match partial_factor(&mut front, m, ns, f, diag, wrow, test, &mut d) {
            Ok(split) => split_fronts += usize::from(split),
            Err((k, pivot, threshold, scale)) => return Err((f + k, pivot, threshold, scale)),
        }
        // 3. hand-over: the trapezoid to the factor, the rest to the parent
        live -= bytes * front.len();
        let at = off(ns, m);
        if m > ns {
            let upd = front.split_off(at);
            live += bytes * upd.len();
            pending[s] = Some(upd);
        }
        front.shrink_to_fit();
        values.push(front);
    }
    Ok((values, d, max_front, split_fronts, peak))
}

impl<T: FrontScalar> Supernodal<T> {
    fn factor_entries(&self) -> usize {
        self.values.iter().map(Vec::len).sum::<usize>() - self.d.len()
    }

    fn supernodes(&self) -> usize {
        self.sn_ptr.len() - 1
    }

    fn column(&self, j: usize) -> (&[usize], &[T]) {
        let s = self.col_sn[j];
        let q = j - self.sn_ptr[s];
        let m = self.rows[s].len();
        let o = off(q, m);
        (&self.rows[s][q + 1..], &self.values[s][o + 1..o + (m - q)])
    }

    /// Solve `A x = b` into `x` with the elimination order `order` (ADR 0503 D3, solve).
    fn solve_into(&self, order: &[usize], b: &[T], x: &mut [T]) {
        let n = self.d.len();
        assert_eq!(b.len(), n, "right-hand side length");
        assert_eq!(x.len(), n, "solution length");
        let mut z: Vec<T> = order.iter().map(|&o| b[o]).collect();
        let nsn = self.supernodes();
        for s in 0..nsn {
            let (rows, vals) = (&self.rows[s], &self.values[s]);
            let m = rows.len();
            for q in 0..self.sn_ptr[s + 1] - self.sn_ptr[s] {
                let zj = z[rows[q]];
                let o = off(q, m);
                for t in 1..m - q {
                    z[rows[q + t]] -= vals[o + t] * zj;
                }
            }
        }
        for (zj, dj) in z.iter_mut().zip(&self.d) {
            *zj /= *dj;
        }
        for s in (0..nsn).rev() {
            let (rows, vals) = (&self.rows[s], &self.values[s]);
            let m = rows.len();
            for q in (0..self.sn_ptr[s + 1] - self.sn_ptr[s]).rev() {
                let o = off(q, m);
                let mut acc = z[rows[q]];
                for t in 1..m - q {
                    acc -= vals[o + t] * z[rows[q + t]];
                }
                z[rows[q]] = acc;
            }
        }
        for (i, &o) in order.iter().enumerate() {
            x[o] = z[i];
        }
    }
}

/// The nested dissection order and the symbolic phase of [`SupernodalLdlt`] for one pattern (ADR 0566 D1): made once by
/// [`SupernodalAnalysis::nested_dissection`] and used by [`SupernodalLdlt::factor_analysed`] for every matrix of that
/// pattern. The order and the symbolic phase read the pattern only, and the values are placed where
/// [`SupernodalLdlt::factor`] places them, so the factor's bits are those of [`SupernodalLdlt::factor`].
#[derive(Debug, Clone, PartialEq)]
pub struct SupernodalAnalysis {
    n: usize,
    ncols: usize,
    row_ptr: Vec<usize>,
    col: Vec<usize>,
    order: Vec<usize>,
    sym: Symbolic,
    /// For each elimination position, the index in `val` of its diagonal entry (`NONE`: not stored, the value 0).
    diag_src: Vec<usize>,
    /// For each entry of the column structure of `sym`, the index in `val` it is copied from.
    col_src: Vec<usize>,
}

impl SupernodalAnalysis {
    /// The analysis of the pattern of `a` with the nested dissection order by level structures ([`nested_dissection_order`],
    /// the order of [`SupernodalLdlt::factor`]). Refuses a matrix that is not square (`NotSquare`); the values are not read.
    pub fn nested_dissection(a: &CsrMatrix) -> Result<SupernodalAnalysis, SkylineError> {
        if a.ncols != a.n {
            return Err(SkylineError::NotSquare {
                rows: a.n,
                cols: a.ncols,
            });
        }
        let order = nested_dissection_order(a);
        let index: Vec<usize> = (0..a.col.len()).collect();
        let c = permuted_lower_parts(a.n, &a.row_ptr, &a.col, &index, &order, NONE);
        let cols = columns_of(&c, NONE);
        let sym = analyse(&c, cols.ptr, cols.idx);
        Ok(SupernodalAnalysis {
            n: a.n,
            ncols: a.ncols,
            row_ptr: a.row_ptr.clone(),
            col: a.col.clone(),
            order,
            sym,
            diag_src: c.diag,
            col_src: cols.val,
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

    /// Whether `a` has the analysed pattern: the same dimensions, row pointers and column indices.
    pub fn matches(&self, a: &CsrMatrix) -> bool {
        a.n == self.n && a.ncols == self.ncols && a.row_ptr == self.row_ptr && a.col == self.col
    }
}

/// Supernodal multifrontal `L D L^T` of a symmetric matrix after a fill-reducing ordering (ADR 0503 D2, D3).
#[derive(Debug, Clone, PartialEq)]
pub struct SupernodalLdlt {
    n: usize,
    order: Vec<usize>,
    f: Supernodal<f64>,
}

impl SupernodalLdlt {
    /// Factorise a symmetric positive definite matrix after nested dissection by level structures (positive pivots,
    /// symmetry tolerance 1e-12).
    pub fn factor(a: &CsrMatrix) -> Result<SupernodalLdlt, SkylineError> {
        Self::factor_with(
            a,
            &SparseOptions {
                ordering: SparseOrdering::NestedDissection,
                ..SparseOptions::default()
            },
        )
    }

    /// Factorise `a` with the given options: validated as [`SparseLdlt::factor_with`]; the lower triangle is used, a missing
    /// lower entry taken from the upper (ADR 0503 D2, D3).
    pub fn factor_with(a: &CsrMatrix, opts: &SparseOptions) -> Result<SupernodalLdlt, SkylineError> {
        validate(a, opts.symmetry_tolerance)?;
        let n = a.n;
        let order = order_of(a, &opts.ordering)?;
        let c = permuted_lower(a, &order);
        let pivots = opts.pivots;
        // the pivot test of ADR 0503 D3
        let test = |akk: f64, dk: f64, w: usize| -> Option<(f64, f64)> {
            let theta = (w as f64 + 1.0) * f64::EPSILON * (akk.abs() + (akk - dk).abs());
            let refused = match pivots {
                PivotRule::Positive => dk <= theta,
                PivotRule::NonZero => dk.abs() <= theta,
            };
            (refused || !dk.is_finite()).then_some((theta, 0.0))
        };
        match supernodal_factor(c, &test) {
            Ok(f) => Ok(SupernodalLdlt { n, order, f }),
            Err((k, pivot, threshold, _)) => Err(SkylineError::Pivot {
                row: order[k],
                position: k,
                pivot,
                threshold,
            }),
        }
    }

    /// [`SupernodalLdlt::factor`] of `a` with the order and the symbolic phase of `analysis` (ADR 0566 D1): `a` is validated
    /// as by [`SupernodalLdlt::factor`] (square, finite, symmetric within 1e-12), its values placed through the analysis'
    /// value map, and the numeric phase run with positive pivots. The result and every refusal are bit for bit those of
    /// [`SupernodalLdlt::factor`].
    ///
    /// # Panics
    /// If `analysis` is not of the pattern of `a` ([`SupernodalAnalysis::matches`]).
    pub fn factor_analysed(a: &CsrMatrix, analysis: &SupernodalAnalysis) -> Result<SupernodalLdlt, SkylineError> {
        validate(a, 1e-12)?;
        assert!(analysis.matches(a), "the analysis of another pattern");
        let diag: Vec<f64> = analysis
            .diag_src
            .iter()
            .map(|&k| if k == NONE { 0.0 } else { a.val[k] })
            .collect();
        let colval: Vec<f64> = analysis.col_src.iter().map(|&k| a.val[k]).collect();
        let test = |akk: f64, dk: f64, w: usize| -> Option<(f64, f64)> {
            let theta = (w as f64 + 1.0) * f64::EPSILON * (akk.abs() + (akk - dk).abs());
            (dk <= theta || !dk.is_finite()).then_some((theta, 0.0))
        };
        let order = &analysis.order;
        match numeric(&analysis.sym, &diag, &colval, &test) {
            Ok((values, d, max_front, split_fronts, peak)) => Ok(SupernodalLdlt {
                n: a.n,
                order: order.clone(),
                f: Supernodal {
                    sn_ptr: analysis.sym.sn_ptr.clone(),
                    col_sn: analysis.sym.col_sn.clone(),
                    rows: analysis.sym.rows.clone(),
                    values,
                    d,
                    max_front,
                    split_fronts,
                    peak_front_bytes: peak,
                },
            }),
            Err((k, pivot, threshold, _)) => Err(SkylineError::Pivot {
                row: order[k],
                position: k,
                pivot,
                threshold,
            }),
        }
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
        &self.f.d
    }

    /// Counts of negative and positive pivots.
    pub fn inertia(&self) -> (usize, usize) {
        let neg = self.f.d.iter().filter(|&&v| v < 0.0).count();
        (neg, self.n - neg)
    }

    /// Entries of the strict lower factor `L` (`nnz(L)`; no amalgamation, so equal to [`factor_count`]).
    pub fn factor_entries(&self) -> usize {
        self.f.factor_entries()
    }

    /// Number of supernodes.
    pub fn supernodes(&self) -> usize {
        self.f.supernodes()
    }

    /// The largest front order.
    pub fn max_front(&self) -> usize {
        self.f.max_front
    }

    /// Fronts of which a later-column update was split over the rayon pool.
    pub fn split_fronts(&self) -> usize {
        self.f.split_fronts
    }

    /// The most bytes of fronts and pending update matrices alive at once during the factorisation.
    pub fn peak_front_bytes(&self) -> usize {
        self.f.peak_front_bytes
    }

    /// Bytes of the factor's arrays, 8 each: values and row indices of the supernodes, supernode pointers, `D` and order.
    pub fn storage_bytes(&self) -> usize {
        let v: usize = self.f.values.iter().map(Vec::len).sum();
        let r: usize = self.f.rows.iter().map(Vec::len).sum();
        8 * (v + r + self.f.sn_ptr.len() + 2 * self.n)
    }

    /// Column `j` of `L` (positions in elimination order, increasing): its row positions and values.
    pub fn column(&self, j: usize) -> (&[usize], &[f64]) {
        self.f.column(j)
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
        self.f.solve_into(&self.order, b, x);
    }
}

// ------------------------------------------------------------------------- complex supernodal (ADR 0509 D2)

/// Options of [`ComplexSupernodalLdlt::factor_with`] (ADR 0509 D2).
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexSupernodalOptions {
    /// Elimination order, computed on the pattern (default nested dissection by level structures).
    pub ordering: SparseOrdering,
    /// Relative symmetry tolerance `delta`: `|a_rc - a_cr| <= delta sqrt(|a_rr| |a_cc|)` (no conjugation). Default 1e-12.
    pub symmetry_tolerance: f64,
    /// `tau`: a pivot with `|d_k| <= tau s_k` is refused besides the rounding test (default 0; ADR 0509 D2).
    pub min_relative_pivot: f64,
}

impl Default for ComplexSupernodalOptions {
    fn default() -> Self {
        ComplexSupernodalOptions {
            ordering: SparseOrdering::NestedDissection,
            symmetry_tolerance: 1e-12,
            min_relative_pivot: 0.0,
        }
    }
}

/// Supernodal multifrontal `L D L^T` of a complex symmetric matrix (`A = A^T`, not Hermitian), no conjugation, no pivoting
/// (ADR 0509 D2): the kernel of [`SupernodalLdlt`] in complex arithmetic. A pivot `d_k` is refused when `|d_k| <=
/// max((w_k + 1) eps s_k, tau s_k)` or is not finite, `s_k = |a_kk| + |a_kk - d_k|`, `w_k` the row count of `L`. For a real
/// matrix the real parts are those of [`SupernodalLdlt`] in the same order and every imaginary part is zero.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplexSupernodalLdlt {
    n: usize,
    order: Vec<usize>,
    f: Supernodal<C64>,
}

impl ComplexSupernodalLdlt {
    /// Factorise with the default options (nested dissection by level structures, symmetry tolerance 1e-12, the rounding
    /// pivot test only).
    pub fn factor(a: &ComplexCsr) -> Result<ComplexSupernodalLdlt, ComplexSkylineError> {
        Self::factor_with(a, &ComplexSupernodalOptions::default())
    }

    /// Factorise `a` with the given options. The matrix is validated first (finite, complex symmetric within the
    /// tolerance, as [`crate::cskyline::ComplexSkylineLdlt`]); the lower triangle is used, a missing lower entry taken from
    /// the upper.
    pub fn factor_with(
        a: &ComplexCsr,
        opts: &ComplexSupernodalOptions,
    ) -> Result<ComplexSupernodalLdlt, ComplexSkylineError> {
        crate::cskyline::validate(a, opts.symmetry_tolerance)?;
        let tau = opts.min_relative_pivot;
        if !(tau.is_finite() && tau >= 0.0) {
            return Err(ComplexSkylineError::Malformed {
                reason: format!("minimum relative pivot {tau:e} (must be finite and >= 0)"),
            });
        }
        let n = a.n;
        // the orderings read the pattern only
        let pattern = CsrMatrix {
            n,
            ncols: n,
            row_ptr: a.row_ptr.clone(),
            col: a.col.clone(),
            val: vec![1.0; a.col.len()],
        };
        let order = order_of(&pattern, &opts.ordering).map_err(|e| match e {
            SkylineError::BadOrdering { reason } => ComplexSkylineError::BadOrdering { reason },
            other => ComplexSkylineError::Malformed {
                reason: other.to_string(),
            },
        })?;
        drop(pattern);
        let c = permuted_lower_parts(n, &a.row_ptr, &a.col, &a.val, &order, C64::ZERO);
        let test = |akk: C64, dk: C64, w: usize| -> Option<(f64, f64)> {
            let scale = akk.abs() + (akk - dk).abs();
            let theta = ((w as f64 + 1.0) * f64::EPSILON * scale).max(tau * scale);
            (dk.abs() <= theta || !dk.is_finite()).then_some((theta, scale))
        };
        match supernodal_factor(c, &test) {
            Ok(f) => Ok(ComplexSupernodalLdlt { n, order, f }),
            Err((k, pivot, threshold, scale)) => Err(ComplexSkylineError::Pivot {
                row: order[k],
                position: k,
                pivot,
                threshold,
                scale,
            }),
        }
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
    pub fn pivots(&self) -> &[C64] {
        &self.f.d
    }

    /// Entries of the strict lower factor `L`.
    pub fn factor_entries(&self) -> usize {
        self.f.factor_entries()
    }

    /// Number of supernodes.
    pub fn supernodes(&self) -> usize {
        self.f.supernodes()
    }

    /// The largest front order.
    pub fn max_front(&self) -> usize {
        self.f.max_front
    }

    /// Fronts of which a later-column update was split over the rayon pool.
    pub fn split_fronts(&self) -> usize {
        self.f.split_fronts
    }

    /// The most bytes of fronts and pending update matrices alive at once during the factorisation (16 per value).
    pub fn peak_front_bytes(&self) -> usize {
        self.f.peak_front_bytes
    }

    /// Column `j` of `L` (positions in elimination order, increasing): its row positions and values.
    pub fn column(&self, j: usize) -> (&[usize], &[C64]) {
        self.f.column(j)
    }

    /// Solve `A x = b`.
    pub fn solve(&self, b: &[C64]) -> Vec<C64> {
        let mut x = vec![C64::ZERO; self.n];
        self.solve_into(b, &mut x);
        x
    }

    /// Solve `A x = b` into `x`.
    pub fn solve_into(&self, b: &[C64], x: &mut [C64]) {
        self.f.solve_into(&self.order, b, x);
    }
}
