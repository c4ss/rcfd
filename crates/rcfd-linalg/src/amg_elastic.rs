//! Smoothed-aggregation multigrid for elasticity (ADR 0305, F-402): the near-null space is given as vectors (for solids
//! the six rigid-body modes from the node coordinates, [`rigid_body_modes`]); the tentative prolongator carries them on each
//! aggregate by a local QR with a column-drop rule; the prolongator is smoothed by one damped block-Jacobi step; the
//! cycle is a V(1,1)-cycle with forward / backward block Gauss-Seidel and a dense LU on the coarsest level. Method:
//! Vanek, Mandel and Brezina, Computing 56 (1996) 179 (cited from memory; every step is derived in ADR 0305).
//!
//! Separate from the scalar AMG of [`crate::amg`] (which is not used or changed here). Setup and cycle are sequential
//! except the deterministic per-row `matmul` and `matvec`, so the result does not depend on the thread count.

use crate::matrix::CsrMatrix;
use crate::precond::Preconditioner;
use crate::vecops::norm2;

/// Settings of [`ElasticAmg`] (ADR 0305 D5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElasticAmgSettings {
    /// Strength threshold on the block graph (halved on each coarser level); 0 makes every stored neighbour strong.
    pub theta: f64,
    /// A level of at most this order is the coarsest (dense LU).
    pub max_coarse: usize,
    /// Maximum number of levels.
    pub max_levels: usize,
    /// Forward block Gauss-Seidel sweeps before the coarse correction.
    pub pre_sweeps: usize,
    /// Backward block Gauss-Seidel sweeps after the coarse correction.
    pub post_sweeps: usize,
    /// A near-null column is dropped from an aggregate when its norm after orthogonalisation is at most this fraction of
    /// its norm before.
    pub drop_tolerance: f64,
}

impl Default for ElasticAmgSettings {
    fn default() -> Self {
        ElasticAmgSettings {
            theta: 0.0,
            max_coarse: 1000,
            max_levels: 10,
            pre_sweeps: 1,
            post_sweeps: 1,
            drop_tolerance: 1e-10,
        }
    }
}

/// Largest coarsest order factored densely.
pub const MAX_DENSE: usize = 6000;

/// Refusals of the setup.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ElasticAmgError {
    /// The matrix is not square.
    NotSquare,
    /// The block partition does not cover the rows `0..n` in increasing, non-empty blocks.
    BadBlocks,
    /// A near-null vector does not have the length of the matrix.
    NearNullLength {
        /// The column.
        column: usize,
    },
    /// A diagonal block is not positive definite.
    BlockNotPositive {
        /// The level.
        level: usize,
        /// The block.
        block: usize,
    },
    /// The coarsest level is too large for the dense solve.
    CoarseTooLarge {
        /// Its order.
        n: usize,
    },
}

impl std::fmt::Display for ElasticAmgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ElasticAmgError::NotSquare => write!(f, "the matrix is not square"),
            ElasticAmgError::BadBlocks => write!(f, "the block partition does not cover the rows"),
            ElasticAmgError::NearNullLength { column } => {
                write!(f, "near-null vector {column} does not have the length of the matrix")
            }
            ElasticAmgError::BlockNotPositive { level, block } => {
                write!(f, "diagonal block {block} of level {level} is not positive definite")
            }
            ElasticAmgError::CoarseTooLarge { n } => {
                write!(f, "the coarsest level has order {n} > {MAX_DENSE}")
            }
        }
    }
}

impl std::error::Error for ElasticAmgError {}

/// The six rigid-body modes of rows with a point and a component `0..3` (ADR 0305 D1): columns 0 to 2 the translations,
/// 3 to 5 the rotations `e_a x (x - x_0)` about the axes, `x_0` the mean of the rows' points.
pub fn rigid_body_modes(points: &[[f64; 3]], components: &[usize]) -> Vec<Vec<f64>> {
    let n = points.len();
    let mut x0 = [0.0; 3];
    for p in points {
        for d in 0..3 {
            x0[d] += p[d];
        }
    }
    if n > 0 {
        for v in x0.iter_mut() {
            *v /= n as f64;
        }
    }
    let mut modes = vec![vec![0.0; n]; 6];
    for r in 0..n {
        let c = components[r];
        let d = [points[r][0] - x0[0], points[r][1] - x0[1], points[r][2] - x0[2]];
        modes[c][r] = 1.0;
        // e_0 x d = (0, -d_z, d_y); e_1 x d = (d_z, 0, -d_x); e_2 x d = (-d_y, d_x, 0)
        let rot = [[0.0, -d[2], d[1]], [d[2], 0.0, -d[0]], [-d[1], d[0], 0.0]];
        for a in 0..3 {
            modes[3 + a][r] = rot[a][c];
        }
    }
    modes
}

/// The local QR of ADR 0305 D3: `b` is `m x k` row-major; modified Gram-Schmidt with one reorthogonalisation pass, a
/// column dropped when its norm after is `<= tolerance x` its norm before (or that norm is 0). Returns `Q` (`m x k_a`,
/// row-major, orthonormal columns) and `k_a`.
pub fn tentative_block(b: &[f64], m: usize, k: usize, tolerance: f64) -> (Vec<f64>, usize) {
    let mut cols: Vec<Vec<f64>> = Vec::new();
    for j in 0..k {
        let mut v: Vec<f64> = (0..m).map(|i| b[i * k + j]).collect();
        let before = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if before == 0.0 {
            continue;
        }
        for _ in 0..2 {
            for q in &cols {
                let s: f64 = q.iter().zip(&v).map(|(p, x)| p * x).sum();
                for (x, p) in v.iter_mut().zip(q) {
                    *x -= s * p;
                }
            }
        }
        let after = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if after <= tolerance * before {
            continue;
        }
        v.iter_mut().for_each(|x| *x /= after);
        cols.push(v);
    }
    let ka = cols.len();
    let mut q = vec![0.0; m * ka];
    for (j, c) in cols.iter().enumerate() {
        for i in 0..m {
            q[i * ka + j] = c[i];
        }
    }
    (q, ka)
}

struct Level {
    a: CsrMatrix,
    block_ptr: Vec<usize>,
    /// Dense inverses of the diagonal blocks, row-major, at `inv_off[block]`.
    inv: Vec<f64>,
    inv_off: Vec<usize>,
    near_null: Vec<Vec<f64>>,
    tentative: Option<CsrMatrix>,
    p: Option<CsrMatrix>,
    r: Option<CsrMatrix>,
    aggregates: Option<Vec<usize>>,
    omega: Option<f64>,
}

/// The multigrid hierarchy; one V-cycle from zero is its [`Preconditioner::apply`].
pub struct ElasticAmg {
    levels: Vec<Level>,
    coarse: DenseLu,
    settings: ElasticAmgSettings,
}

struct DenseLu {
    n: usize,
    lu: Vec<f64>,
    piv: Vec<usize>,
}

impl DenseLu {
    fn new(a: &CsrMatrix) -> Self {
        let n = a.n;
        let mut lu = vec![0.0; n * n];
        for i in 0..n {
            for j in a.row_ptr[i]..a.row_ptr[i + 1] {
                lu[i * n + a.col[j]] = a.val[j];
            }
        }
        let mut piv = vec![0; n];
        for k in 0..n {
            let (mut p, mut best) = (k, lu[k * n + k].abs());
            for i in k + 1..n {
                if lu[i * n + k].abs() > best {
                    best = lu[i * n + k].abs();
                    p = i;
                }
            }
            piv[k] = p;
            if p != k {
                for j in 0..n {
                    lu.swap(k * n + j, p * n + j);
                }
            }
            let d = lu[k * n + k];
            if d == 0.0 {
                continue;
            }
            for i in k + 1..n {
                let f = lu[i * n + k] / d;
                lu[i * n + k] = f;
                if f != 0.0 {
                    for j in k + 1..n {
                        lu[i * n + j] -= f * lu[k * n + j];
                    }
                }
            }
        }
        DenseLu { n, lu, piv }
    }

    fn solve(&self, b: &[f64], x: &mut [f64]) {
        let n = self.n;
        x.copy_from_slice(b);
        for k in 0..n {
            x.swap(k, self.piv[k]);
            for i in k + 1..n {
                x[i] -= self.lu[i * n + k] * x[k];
            }
        }
        for k in (0..n).rev() {
            for j in k + 1..n {
                x[k] -= self.lu[k * n + j] * x[j];
            }
            x[k] /= self.lu[k * n + k];
        }
    }
}

/// Dense inverse of an SPD block by Cholesky; `None` on a non-positive pivot.
fn spd_inverse(a: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                if s.is_nan() || s <= 0.0 {
                    return None;
                }
                l[i * n + i] = s.sqrt();
            } else {
                l[i * n + j] = s / l[j * n + j];
            }
        }
    }
    let mut inv = vec![0.0; n * n];
    let mut y = vec![0.0; n];
    for c in 0..n {
        for i in 0..n {
            let mut s = if i == c { 1.0 } else { 0.0 };
            for k in 0..i {
                s -= l[i * n + k] * y[k];
            }
            y[i] = s / l[i * n + i];
        }
        for i in (0..n).rev() {
            let mut s = y[i];
            for k in i + 1..n {
                s -= l[k * n + i] * inv[k * n + c];
            }
            inv[i * n + c] = s / l[i * n + i];
        }
    }
    Some(inv)
}

fn block_inverses(a: &CsrMatrix, bp: &[usize], level: usize) -> Result<(Vec<f64>, Vec<usize>), ElasticAmgError> {
    let nb = bp.len() - 1;
    let mut inv = Vec::new();
    let mut off = Vec::with_capacity(nb);
    for b in 0..nb {
        let (s, e) = (bp[b], bp[b + 1]);
        let bs = e - s;
        let mut d = vec![0.0; bs * bs];
        for r in s..e {
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                let c = a.col[k];
                if c >= s && c < e {
                    d[(r - s) * bs + (c - s)] = a.val[k];
                }
            }
        }
        let bi = spd_inverse(&d, bs).ok_or(ElasticAmgError::BlockNotPositive { level, block: b })?;
        off.push(inv.len());
        inv.extend(bi);
    }
    Ok((inv, off))
}

/// `y_I <- D_I^{-1} y_I` for every block.
fn apply_block_inverse(bp: &[usize], inv: &[f64], off: &[usize], y: &mut [f64]) {
    let mut t = Vec::new();
    for b in 0..bp.len() - 1 {
        let (s, e) = (bp[b], bp[b + 1]);
        let bs = e - s;
        t.clear();
        t.extend_from_slice(&y[s..e]);
        for i in 0..bs {
            y[s + i] = (0..bs).map(|j| inv[off[b] + i * bs + j] * t[j]).sum();
        }
    }
}

fn block_of_rows(bp: &[usize], n: usize) -> Vec<usize> {
    let mut of = vec![0; n];
    for b in 0..bp.len() - 1 {
        of[bp[b]..bp[b + 1]].fill(b);
    }
    of
}

/// Greedy aggregation on the block graph (ADR 0305 D2). Returns the aggregate of each block and the count.
fn aggregate_blocks(a: &CsrMatrix, bp: &[usize], theta: f64) -> (Vec<usize>, usize) {
    const NONE: usize = usize::MAX;
    let nb = bp.len() - 1;
    let of = block_of_rows(bp, a.n);
    // block graph with Frobenius weights
    let mut w2 = vec![0.0f64; nb];
    let mut mark = vec![NONE; nb];
    let mut touched: Vec<usize> = Vec::new();
    let mut nbrs: Vec<Vec<(usize, f64)>> = Vec::with_capacity(nb);
    let mut dnorm = vec![0.0; nb];
    for b in 0..nb {
        touched.clear();
        for r in bp[b]..bp[b + 1] {
            for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                let c = of[a.col[k]];
                if mark[c] != b {
                    mark[c] = b;
                    w2[c] = 0.0;
                    touched.push(c);
                }
                w2[c] += a.val[k] * a.val[k];
            }
        }
        touched.sort_unstable();
        let mut list = Vec::with_capacity(touched.len());
        for &c in &touched {
            if c == b {
                dnorm[b] = w2[c].sqrt();
            } else {
                list.push((c, w2[c].sqrt()));
            }
        }
        nbrs.push(list);
    }
    let strong: Vec<Vec<(usize, f64)>> = (0..nb)
        .map(|b| {
            nbrs[b]
                .iter()
                .copied()
                .filter(|&(c, w)| w >= theta * (dnorm[b] * dnorm[c]).sqrt())
                .collect()
        })
        .collect();
    let mut agg = vec![NONE; nb];
    let mut count = 0;
    // pass 1: seeds whose strong neighbours are all free
    for b in 0..nb {
        if agg[b] != NONE {
            continue;
        }
        if strong[b].iter().all(|&(c, _)| agg[c] == NONE) {
            agg[b] = count;
            for &(c, _) in &strong[b] {
                agg[c] = count;
            }
            count += 1;
        }
    }
    // pass 2: join the pass-1 aggregate of the strongest strong neighbour
    let snapshot = agg.clone();
    for b in 0..nb {
        if agg[b] != NONE {
            continue;
        }
        let mut best = (-1.0, NONE);
        for &(c, w) in &strong[b] {
            if snapshot[c] != NONE && w > best.0 {
                best = (w, snapshot[c]);
            }
        }
        if best.1 != NONE {
            agg[b] = best.1;
        }
    }
    // pass 3: singletons
    for slot in agg.iter_mut() {
        if *slot == NONE {
            *slot = count;
            count += 1;
        }
    }
    (agg, count)
}

/// Spectral radius of `D^{-1} A` by power iteration (15 steps, times 1.1; the start vector of `amg.rs`).
fn spectral_radius(a: &CsrMatrix, bp: &[usize], inv: &[f64], off: &[usize]) -> f64 {
    let n = a.n;
    let mut x: Vec<f64> = (0..n)
        .map(|i| 1.0 + ((i * 2654435761usize) % 1000) as f64 * 1e-3)
        .collect();
    let mut y = vec![0.0; n];
    let mut rho = 1.0;
    for _ in 0..15 {
        let nx = norm2(&x);
        x.iter_mut().for_each(|v| *v /= nx);
        a.matvec(&x, &mut y);
        apply_block_inverse(bp, inv, off, &mut y);
        rho = norm2(&y);
        std::mem::swap(&mut x, &mut y);
    }
    rho * 1.1
}

/// `P = P_t - omega D^{-1} (A P_t)` (ADR 0305 D4).
fn smooth_prolongator(
    a: &CsrMatrix,
    pt: &CsrMatrix,
    bp: &[usize],
    inv: &[f64],
    off: &[usize],
    omega: f64,
) -> CsrMatrix {
    let ap = a.matmul(pt);
    let nc = pt.ncols;
    let mut row_ptr = vec![0usize; a.n + 1];
    let mut cols: Vec<usize> = Vec::new();
    let mut vals: Vec<f64> = Vec::new();
    let mut pos = vec![usize::MAX; nc];
    for b in 0..bp.len() - 1 {
        let (s, e) = (bp[b], bp[b + 1]);
        let bs = e - s;
        let mut union: Vec<usize> = Vec::new();
        for r in s..e {
            union.extend_from_slice(&ap.col[ap.row_ptr[r]..ap.row_ptr[r + 1]]);
            union.extend_from_slice(&pt.col[pt.row_ptr[r]..pt.row_ptr[r + 1]]);
        }
        union.sort_unstable();
        union.dedup();
        for (i, &c) in union.iter().enumerate() {
            pos[c] = i;
        }
        let w = union.len();
        let mut dense = vec![0.0; bs * w];
        for r in s..e {
            for k in ap.row_ptr[r]..ap.row_ptr[r + 1] {
                dense[(r - s) * w + pos[ap.col[k]]] = ap.val[k];
            }
        }
        for i in 0..bs {
            let r = s + i;
            let mut row = vec![0.0; w];
            for k in pt.row_ptr[r]..pt.row_ptr[r + 1] {
                row[pos[pt.col[k]]] = pt.val[k];
            }
            for (cidx, v) in row.iter_mut().enumerate() {
                let mut s2 = 0.0;
                for j in 0..bs {
                    s2 += inv[off[b] + i * bs + j] * dense[j * w + cidx];
                }
                *v -= omega * s2;
            }
            for (cidx, &v) in row.iter().enumerate() {
                cols.push(union[cidx]);
                vals.push(v);
            }
            row_ptr[r + 1] = cols.len();
        }
        for &c in &union {
            pos[c] = usize::MAX;
        }
    }
    CsrMatrix::rect(a.n, nc, row_ptr, cols, vals)
}

/// The tentative prolongator, the coarse near-null space and the coarse block partition (ADR 0305 D3).
fn tentative(
    n: usize,
    bp: &[usize],
    agg: &[usize],
    n_agg: usize,
    near_null: &[Vec<f64>],
    tolerance: f64,
) -> (CsrMatrix, Vec<Vec<f64>>, Vec<usize>) {
    let k = near_null.len();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); n_agg];
    for (b, &g) in agg.iter().enumerate() {
        members[g].push(b);
    }
    // per row: (aggregate, local index)
    let mut row_loc = vec![(0usize, 0usize); n];
    let mut qs: Vec<(Vec<f64>, usize)> = Vec::with_capacity(n_agg);
    let mut offsets = vec![0usize; n_agg + 1];
    let mut coarse_b: Vec<Vec<f64>> = vec![Vec::new(); k];
    for g in 0..n_agg {
        let rows: Vec<usize> = members[g].iter().flat_map(|&b| bp[b]..bp[b + 1]).collect();
        let m = rows.len();
        let mut bl = vec![0.0; m * k];
        for (i, &r) in rows.iter().enumerate() {
            row_loc[r] = (g, i);
            for c in 0..k {
                bl[i * k + c] = near_null[c][r];
            }
        }
        let (q, ka) = tentative_block(&bl, m, k, tolerance);
        // coarse representation Q^T B
        for j in 0..ka {
            for c in 0..k {
                let s: f64 = (0..m).map(|i| q[i * ka + j] * bl[i * k + c]).sum();
                coarse_b[c].push(s);
            }
        }
        offsets[g + 1] = offsets[g] + ka;
        qs.push((q, ka));
    }
    let nc = offsets[n_agg];
    let mut row_ptr = vec![0usize; n + 1];
    let mut cols = Vec::new();
    let mut vals = Vec::new();
    for r in 0..n {
        let (g, i) = row_loc[r];
        let (q, ka) = (&qs[g].0, qs[g].1);
        for j in 0..ka {
            cols.push(offsets[g] + j);
            vals.push(q[i * ka + j]);
        }
        row_ptr[r + 1] = cols.len();
    }
    let mut cbp = vec![0usize];
    for g in 0..n_agg {
        if offsets[g + 1] > offsets[g] {
            cbp.push(offsets[g + 1]);
        }
    }
    (CsrMatrix::rect(n, nc, row_ptr, cols, vals), coarse_b, cbp)
}

fn check_blocks(bp: &[usize], n: usize) -> bool {
    bp.first() == Some(&0) && bp.last() == Some(&n) && bp.windows(2).all(|w| w[0] < w[1])
}

impl ElasticAmg {
    /// Build the hierarchy of `a` with the block partition `block_ptr` and the near-null vectors `near_null`.
    pub fn new(
        a: &CsrMatrix,
        block_ptr: &[usize],
        near_null: &[Vec<f64>],
        settings: &ElasticAmgSettings,
    ) -> Result<Self, ElasticAmgError> {
        if a.n != a.ncols {
            return Err(ElasticAmgError::NotSquare);
        }
        if !check_blocks(block_ptr, a.n) && a.n > 0 {
            return Err(ElasticAmgError::BadBlocks);
        }
        for (column, v) in near_null.iter().enumerate() {
            if v.len() != a.n {
                return Err(ElasticAmgError::NearNullLength { column });
            }
        }
        let mut levels: Vec<Level> = Vec::new();
        let mut cur = a.clone();
        let mut bp = block_ptr.to_vec();
        let mut nn = near_null.to_vec();
        loop {
            let l = levels.len();
            let (inv, inv_off) = block_inverses(&cur, &bp, l)?;
            let mut coarsest = cur.n <= settings.max_coarse || l + 1 >= settings.max_levels || nn.is_empty();
            let mut built = None;
            if !coarsest {
                let theta = settings.theta * 0.5f64.powi(l as i32);
                let (agg, n_agg) = aggregate_blocks(&cur, &bp, theta);
                let (pt, cb, cbp) = tentative(cur.n, &bp, &agg, n_agg, &nn, settings.drop_tolerance);
                if pt.ncols == 0 || pt.ncols as f64 > 0.85 * cur.n as f64 {
                    coarsest = true;
                } else {
                    built = Some((agg, pt, cb, cbp));
                }
            }
            if coarsest {
                levels.push(Level {
                    a: cur,
                    block_ptr: bp,
                    inv,
                    inv_off,
                    near_null: nn,
                    tentative: None,
                    p: None,
                    r: None,
                    aggregates: None,
                    omega: None,
                });
                break;
            }
            let (agg, pt, cb, cbp) = built.expect("built");
            let rho = spectral_radius(&cur, &bp, &inv, &inv_off);
            let omega = (4.0 / 3.0) / rho;
            let p = smooth_prolongator(&cur, &pt, &bp, &inv, &inv_off, omega);
            let r = p.transpose();
            let ac = r.matmul(&cur.matmul(&p));
            levels.push(Level {
                a: cur,
                block_ptr: bp,
                inv,
                inv_off,
                near_null: nn,
                tentative: Some(pt),
                p: Some(p),
                r: Some(r),
                aggregates: Some(agg),
                omega: Some(omega),
            });
            cur = ac;
            bp = cbp;
            nn = cb;
        }
        let last = &levels.last().expect("at least one level").a;
        if last.n > MAX_DENSE {
            return Err(ElasticAmgError::CoarseTooLarge { n: last.n });
        }
        let coarse = DenseLu::new(last);
        Ok(ElasticAmg {
            levels,
            coarse,
            settings: *settings,
        })
    }

    /// Number of levels.
    pub fn n_levels(&self) -> usize {
        self.levels.len()
    }

    /// Sum of the nonzeros of all levels over those of the finest.
    pub fn operator_complexity(&self) -> f64 {
        self.levels.iter().map(|l| l.a.nnz()).sum::<usize>() as f64 / self.levels[0].a.nnz().max(1) as f64
    }

    /// The matrix of level `l`.
    pub fn matrix(&self, l: usize) -> &CsrMatrix {
        &self.levels[l].a
    }

    /// The block partition of level `l`.
    pub fn block_ptr(&self, l: usize) -> &[usize] {
        &self.levels[l].block_ptr
    }

    /// The near-null vectors of level `l`.
    pub fn near_null(&self, l: usize) -> &[Vec<f64>] {
        &self.levels[l].near_null
    }

    /// The tentative prolongator from level `l + 1` to level `l` (`None` on the coarsest).
    pub fn tentative(&self, l: usize) -> Option<&CsrMatrix> {
        self.levels[l].tentative.as_ref()
    }

    /// The smoothed prolongator from level `l + 1` to level `l` (`None` on the coarsest).
    pub fn prolongator(&self, l: usize) -> Option<&CsrMatrix> {
        self.levels[l].p.as_ref()
    }

    /// The aggregate of each block of level `l` (`None` on the coarsest).
    pub fn aggregates(&self, l: usize) -> Option<&[usize]> {
        self.levels[l].aggregates.as_deref()
    }

    /// The damping of the prolongator smoothing of level `l` (`None` on the coarsest).
    pub fn omega(&self, l: usize) -> Option<f64> {
        self.levels[l].omega
    }

    fn sweep(&self, l: usize, b: &[f64], x: &mut [f64], forward: bool) {
        let lev = &self.levels[l];
        let a = &lev.a;
        let nb = lev.block_ptr.len() - 1;
        let mut t: Vec<f64> = Vec::new();
        let mut one = |blk: usize, x: &mut [f64]| {
            let (s, e) = (lev.block_ptr[blk], lev.block_ptr[blk + 1]);
            let bs = e - s;
            t.clear();
            for r in s..e {
                let mut sum = b[r];
                for k in a.row_ptr[r]..a.row_ptr[r + 1] {
                    let c = a.col[k];
                    if c < s || c >= e {
                        sum -= a.val[k] * x[c];
                    }
                }
                t.push(sum);
            }
            let o = lev.inv_off[blk];
            for i in 0..bs {
                x[s + i] = (0..bs).map(|j| lev.inv[o + i * bs + j] * t[j]).sum();
            }
        };
        if forward {
            for blk in 0..nb {
                one(blk, x);
            }
        } else {
            for blk in (0..nb).rev() {
                one(blk, x);
            }
        }
    }

    fn cycle(&self, l: usize, b: &[f64], x: &mut [f64]) {
        let lev = &self.levels[l];
        let (Some(p), Some(r)) = (&lev.p, &lev.r) else {
            self.coarse.solve(b, x);
            return;
        };
        let n = lev.a.n;
        for _ in 0..self.settings.pre_sweeps {
            self.sweep(l, b, x, true);
        }
        let mut res = vec![0.0; n];
        lev.a.residual(b, x, &mut res);
        let nc = p.ncols;
        let mut rc = vec![0.0; nc];
        r.matvec(&res, &mut rc);
        let mut xc = vec![0.0; nc];
        self.cycle(l + 1, &rc, &mut xc);
        let mut corr = vec![0.0; n];
        p.matvec(&xc, &mut corr);
        x.iter_mut().zip(&corr).for_each(|(xi, c)| *xi += c);
        for _ in 0..self.settings.post_sweeps {
            self.sweep(l, b, x, false);
        }
    }

    /// One V-cycle on `A x = b` from the current `x`.
    pub fn vcycle(&self, b: &[f64], x: &mut [f64]) {
        self.cycle(0, b, x);
    }
}

impl Preconditioner for ElasticAmg {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        z.iter_mut().for_each(|v| *v = 0.0);
        if !r.is_empty() {
            self.vcycle(r, z);
        }
    }
}
