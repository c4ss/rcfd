//! Smoothed-aggregation algebraic multigrid (Vanek, Mandel, Brezina 1996).
//!
//! Setup: strength-of-connection filtering, greedy aggregation, tentative
//! piecewise-constant prolongator smoothed by one damped-Jacobi step on the
//! filtered matrix, Galerkin coarse operators `R A P`. Cycle: V-cycle with
//! forward / backward Gauss-Seidel smoothing (a symmetric operator when `A`
//! is symmetric, so it is a valid CG preconditioner) and a dense LU solve on
//! the coarsest level. The cycle is deterministic.

use crate::matrix::CsrMatrix;
use crate::precond::Preconditioner;
use crate::solvers::{SolveControl, SolveStats};
use crate::vecops::norm2;

/// AMG setup and cycling parameters.
#[derive(Debug, Clone, Copy)]
pub struct AmgSettings {
    /// Strength threshold: `|a_ij| >= theta sqrt(a_ii a_jj)` is a strong connection.
    pub theta: f64,
    /// Stop coarsening at or below this many unknowns (dense solve).
    pub max_coarse: usize,
    /// Maximum number of levels.
    pub max_levels: usize,
    /// Pre-smoothing sweeps (forward Gauss-Seidel).
    pub pre_sweeps: usize,
    /// Post-smoothing sweeps (backward Gauss-Seidel).
    pub post_sweeps: usize,
    /// Coarse-grid visits per level: 1 gives a V-cycle, 2 a W-cycle (default; mesh-independent rate).
    pub cycle_index: usize,
}

impl Default for AmgSettings {
    fn default() -> Self {
        AmgSettings {
            theta: 0.08,
            max_coarse: 150,
            max_levels: 25,
            pre_sweeps: 1,
            post_sweeps: 1,
            cycle_index: 2,
        }
    }
}

struct Level {
    a: CsrMatrix,
    /// Prolongator (n x n_coarse); `None` on the coarsest level.
    p: Option<CsrMatrix>,
    /// Restriction `P^T`.
    r: Option<CsrMatrix>,
    diag_pos: Vec<usize>,
}

/// Multigrid hierarchy.
pub struct Amg {
    levels: Vec<Level>,
    coarse_lu: DenseLu,
    settings: AmgSettings,
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
            let d = if lu[k * n + k].abs() < 1e-300 {
                1e-300
            } else {
                lu[k * n + k]
            };
            lu[k * n + k] = d;
            for i in k + 1..n {
                let f = lu[i * n + k] / d;
                lu[i * n + k] = f;
                for j in k + 1..n {
                    lu[i * n + j] -= f * lu[k * n + j];
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

/// Greedy aggregation on the strength graph. Returns `(aggregate_of_node, n_aggregates)`.
fn aggregate(a: &CsrMatrix, strong: &[bool]) -> (Vec<usize>, usize) {
    let n = a.n;
    const NONE: usize = usize::MAX;
    let mut agg = vec![NONE; n];
    let mut count = 0;
    // pass 1: seeds whose strong neighbours are all free
    for i in 0..n {
        if agg[i] != NONE {
            continue;
        }
        let nb = (a.row_ptr[i]..a.row_ptr[i + 1]).filter(|&k| strong[k] && a.col[k] != i);
        if nb.clone().all(|k| agg[a.col[k]] == NONE) {
            agg[i] = count;
            for k in nb {
                agg[a.col[k]] = count;
            }
            count += 1;
        }
    }
    // pass 2: attach remaining nodes to the strongest neighbouring aggregate
    let snapshot = agg.clone();
    for i in 0..n {
        if agg[i] != NONE {
            continue;
        }
        let mut best = (0.0, NONE);
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            let j = a.col[k];
            if j != i && snapshot[j] != NONE && a.val[k].abs() > best.0 {
                best = (a.val[k].abs(), snapshot[j]);
            }
        }
        if best.1 != NONE {
            agg[i] = best.1;
        }
    }
    // pass 3: isolated leftovers become singleton aggregates
    for slot in agg.iter_mut() {
        if *slot == NONE {
            *slot = count;
            count += 1;
        }
    }
    (agg, count)
}

/// Estimate the spectral radius of `D^-1 A` by power iteration.
fn spectral_radius_dinv_a(a: &CsrMatrix, diag: &[f64]) -> f64 {
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
        for i in 0..n {
            y[i] /= diag[i];
        }
        rho = norm2(&y);
        std::mem::swap(&mut x, &mut y);
    }
    // power iteration underestimates; add a safety margin
    rho * 1.1
}

fn build_level(a: &CsrMatrix, s: &AmgSettings, level: usize) -> Option<(CsrMatrix, CsrMatrix, CsrMatrix)> {
    // the strength threshold is halved on each coarser level: Galerkin operators of smoothed
    // prolongators have many small couplings that would otherwise all count as weak
    let theta = s.theta * 0.5f64.powi(level as i32);
    let n = a.n;
    let diag = a.diag();
    // strength of connection
    let mut strong = vec![false; a.nnz()];
    for i in 0..n {
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            let j = a.col[k];
            if j == i || a.val[k].abs() >= theta * (diag[i].abs() * diag[j].abs()).sqrt() {
                strong[k] = true;
            }
        }
    }
    let (agg, nc) = aggregate(a, &strong);
    if nc == 0 || nc as f64 > 0.85 * n as f64 {
        return None;
    }
    // filtered matrix: weak connections lumped to the diagonal
    let mut fdiag = diag.clone();
    for i in 0..n {
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            if !strong[k] {
                fdiag[i] += a.val[k];
            }
        }
    }
    // A row without any strong coupling and with a zero row sum (Neumann operator) has the filtered diagonal exactly 0
    // (found on a coarse level of a 1000:1 variable-density pressure matrix, ADR 0052): the Jacobi smoothing below would
    // divide by it. Such a row keeps its unfiltered coupling; every other row is untouched.
    let keep_row: Vec<bool> = (0..n).map(|i| fdiag[i] <= 1e-10 * diag[i].abs()).collect();
    for i in 0..n {
        if keep_row[i] {
            fdiag[i] = diag[i];
        }
    }
    // damped Jacobi smoothing of the tentative prolongator
    let mut filtered_val = vec![0.0; a.nnz()];
    for i in 0..n {
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            filtered_val[k] = if strong[k] || keep_row[i] { a.val[k] } else { 0.0 };
        }
    }
    let filtered = CsrMatrix {
        n,
        ncols: n,
        row_ptr: a.row_ptr.clone(),
        col: a.col.clone(),
        val: filtered_val,
    };
    let mut fm = filtered.clone();
    for i in 0..n {
        for k in fm.row_ptr[i]..fm.row_ptr[i + 1] {
            if fm.col[k] == i {
                fm.val[k] = fdiag[i];
            }
        }
    }
    let rho = spectral_radius_dinv_a(&fm, &fdiag);
    let omega = (4.0 / 3.0) / rho;
    let mut row_ptr = vec![0usize; n + 1];
    let mut cols: Vec<usize> = Vec::new();
    let mut vals: Vec<f64> = Vec::new();
    let mut entries: Vec<(usize, f64)> = Vec::new();
    for i in 0..n {
        entries.clear();
        entries.push((agg[i], 1.0));
        for k in fm.row_ptr[i]..fm.row_ptr[i + 1] {
            entries.push((agg[fm.col[k]], -omega * fm.val[k] / fdiag[i]));
        }
        entries.sort_by_key(|e| e.0);
        let mut last = usize::MAX;
        for &(c, v) in entries.iter() {
            if c == last {
                *vals.last_mut().expect("entry") += v;
            } else {
                cols.push(c);
                vals.push(v);
                last = c;
            }
        }
        row_ptr[i + 1] = cols.len();
    }
    let p = CsrMatrix::rect(n, nc, row_ptr, cols, vals);
    let r = p.transpose();
    let ac = r.matmul(&a.matmul(&p));
    Some((p, r, ac))
}

impl Amg {
    /// Build the hierarchy for `a`.
    pub fn new(a: &CsrMatrix, settings: AmgSettings) -> Self {
        let mut levels: Vec<Level> = Vec::new();
        let mut cur = a.clone();
        loop {
            let diag_pos = diag_positions(&cur);
            if cur.n <= settings.max_coarse || levels.len() + 1 >= settings.max_levels {
                levels.push(Level {
                    a: cur,
                    p: None,
                    r: None,
                    diag_pos,
                });
                break;
            }
            match build_level(&cur, &settings, levels.len()) {
                Some((p, r, ac)) => {
                    levels.push(Level {
                        a: cur,
                        p: Some(p),
                        r: Some(r),
                        diag_pos,
                    });
                    cur = ac;
                }
                None => {
                    levels.push(Level {
                        a: cur,
                        p: None,
                        r: None,
                        diag_pos,
                    });
                    break;
                }
            }
        }
        let last = &levels.last().expect("at least one level").a;
        let dummy = CsrMatrix::from_triplets(1, vec![(0, 0, 1.0)]);
        let coarse_lu = DenseLu::new(if last.n <= 2000 { last } else { &dummy });
        Amg {
            levels,
            coarse_lu,
            settings,
        }
    }

    /// Number of levels.
    pub fn n_levels(&self) -> usize {
        self.levels.len()
    }

    /// Sum of nonzeros over all levels divided by the fine-level nonzeros.
    pub fn operator_complexity(&self) -> f64 {
        self.levels.iter().map(|l| l.a.nnz()).sum::<usize>() as f64 / self.levels[0].a.nnz() as f64
    }

    fn cycle(&self, l: usize, b: &[f64], x: &mut [f64]) {
        let lev = &self.levels[l];
        let n = lev.a.n;
        let (Some(p), Some(r)) = (&lev.p, &lev.r) else {
            if n <= 2000 {
                self.coarse_lu.solve(b, x);
            } else {
                x.iter_mut().for_each(|v| *v = 0.0);
                for _ in 0..20 {
                    gauss_seidel(&lev.a, &lev.diag_pos, b, x, true);
                    gauss_seidel(&lev.a, &lev.diag_pos, b, x, false);
                }
            }
            return;
        };
        for _ in 0..self.settings.pre_sweeps {
            gauss_seidel(&lev.a, &lev.diag_pos, b, x, true);
        }
        let mut res = vec![0.0; n];
        lev.a.residual(b, x, &mut res);
        let nc = p.ncols;
        let mut rc = vec![0.0; nc];
        r.matvec(&res, &mut rc);
        let mut xc = vec![0.0; nc];
        // the coarsest level is solved directly, so one visit suffices there
        let visits = if l + 2 < self.levels.len() {
            self.settings.cycle_index.max(1)
        } else {
            1
        };
        for _ in 0..visits {
            self.cycle(l + 1, &rc, &mut xc);
        }
        let mut corr = vec![0.0; n];
        p.matvec(&xc, &mut corr);
        x.iter_mut().zip(&corr).for_each(|(xi, c)| *xi += c);
        for _ in 0..self.settings.post_sweeps {
            gauss_seidel(&lev.a, &lev.diag_pos, b, x, false);
        }
    }

    /// One V-cycle on `A x = b` starting from the current `x`.
    pub fn vcycle(&self, b: &[f64], x: &mut [f64]) {
        self.cycle(0, b, x);
    }

    /// Use AMG as a stand-alone solver (repeated V-cycles).
    pub fn solve(&self, b: &[f64], x: &mut [f64], ctl: &SolveControl) -> SolveStats {
        let a = &self.levels[0].a;
        let mut r = vec![0.0; a.n];
        a.residual(b, x, &mut r);
        let res0 = norm2(&r);
        let mut res = res0;
        let mut it = 0;
        while it < ctl.max_iter && res > ctl.tol_abs && res > ctl.tol_rel * res0 {
            self.vcycle(b, x);
            a.residual(b, x, &mut r);
            res = norm2(&r);
            it += 1;
        }
        SolveStats {
            iterations: it,
            initial_residual: res0,
            final_residual: res,
            converged: res <= ctl.tol_abs || res <= ctl.tol_rel * res0,
        }
    }
}

impl Preconditioner for Amg {
    fn apply(&self, r: &[f64], z: &mut [f64]) {
        z.iter_mut().for_each(|v| *v = 0.0);
        self.vcycle(r, z);
    }
}

fn diag_positions(a: &CsrMatrix) -> Vec<usize> {
    (0..a.n)
        .map(|i| {
            (a.row_ptr[i]..a.row_ptr[i + 1])
                .find(|&k| a.col[k] == i)
                .expect("AMG requires a stored diagonal")
        })
        .collect()
}

/// One Gauss-Seidel sweep (forward or backward).
fn gauss_seidel(a: &CsrMatrix, dpos: &[usize], b: &[f64], x: &mut [f64], forward: bool) {
    let row = |i: usize, x: &mut [f64]| {
        let mut s = b[i];
        for k in a.row_ptr[i]..a.row_ptr[i + 1] {
            let j = a.col[k];
            if j != i {
                s -= a.val[k] * x[j];
            }
        }
        x[i] = s / a.val[dpos[i]];
    };
    if forward {
        for i in 0..a.n {
            row(i, x);
        }
    } else {
        for i in (0..a.n).rev() {
            row(i, x);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precond::Identity;
    use crate::solvers::cg;
    use crate::vecops::norm2;

    /// 7-point (or 5-point in 2D) Dirichlet Laplacian on an n^d grid.
    pub(crate) fn laplace_grid(n: usize, dim: usize) -> CsrMatrix {
        let total = n.pow(dim as u32);
        let mut rp = vec![0usize; total + 1];
        let mut col = Vec::with_capacity(total * (2 * dim + 1));
        let mut val = Vec::with_capacity(total * (2 * dim + 1));
        for id in 0..total {
            let mut idx = [0usize; 3];
            let mut r = id;
            for d in 0..dim {
                idx[d] = r % n;
                r /= n;
            }
            let stride = |d: usize| n.pow(d as u32);
            for d in (0..dim).rev() {
                if idx[d] > 0 {
                    col.push(id - stride(d));
                    val.push(-1.0);
                }
            }
            col.push(id);
            val.push(2.0 * dim as f64);
            for d in 0..dim {
                if idx[d] + 1 < n {
                    col.push(id + stride(d));
                    val.push(-1.0);
                }
            }
            rp[id + 1] = col.len();
        }
        CsrMatrix {
            n: total,
            ncols: total,
            row_ptr: rp,
            col,
            val,
        }
    }

    fn rhs(n: usize) -> Vec<f64> {
        (0..n).map(|i| 1.0 + ((i * 31) % 17) as f64 / 17.0).collect()
    }

    #[test]
    fn hierarchy_shape_and_dense_lu() {
        let a = laplace_grid(40, 2);
        let amg = Amg::new(&a, AmgSettings::default());
        assert!(amg.n_levels() >= 3);
        assert!(amg.levels.windows(2).all(|w| w[1].a.n < w[0].a.n));
        assert!(amg.operator_complexity() < 1.8, "{}", amg.operator_complexity());
        // dense LU is exact on the coarsest level
        let last = &amg.levels.last().unwrap().a;
        let x: Vec<f64> = (0..last.n).map(|i| (i as f64).sin()).collect();
        let mut b = vec![0.0; last.n];
        last.matvec(&x, &mut b);
        let mut y = vec![0.0; last.n];
        amg.coarse_lu.solve(&b, &mut y);
        assert!(x.iter().zip(&y).all(|(p, q)| (p - q).abs() < 1e-9));
    }

    #[test]
    fn galerkin_operator_matches_definition() {
        let a = laplace_grid(16, 2);
        let (p, r, ac) = build_level(&a, &AmgSettings::default(), 0).unwrap();
        // (R A P) v == R (A (P v))
        let v: Vec<f64> = (0..p.ncols).map(|i| ((i * 7) % 5) as f64 - 2.0).collect();
        let (mut pv, mut apv, mut rapv, mut acv) =
            (vec![0.0; a.n], vec![0.0; a.n], vec![0.0; p.ncols], vec![0.0; p.ncols]);
        p.matvec(&v, &mut pv);
        a.matvec(&pv, &mut apv);
        r.matvec(&apv, &mut rapv);
        ac.matvec(&v, &mut acv);
        assert!(rapv.iter().zip(&acv).all(|(x, y)| (x - y).abs() < 1e-10));
        // the interior row sums of P are 1 (constants are in the range of P away from Dirichlet rows)
        let ones = vec![1.0; p.ncols];
        p.matvec(&ones, &mut pv);
        let interior =
            (0..a.n).filter(|&i| (a.row_ptr[i]..a.row_ptr[i + 1]).map(|k| a.val[k]).sum::<f64>().abs() < 1e-12);
        assert!(interior.clone().count() > 100);
        assert!(interior.into_iter().all(|i| (pv[i] - 1.0).abs() < 1e-12));
    }

    #[test]
    fn mesh_independent_convergence_2d_and_3d() {
        let mut counts = Vec::new();
        for (n, dim) in [(32usize, 2usize), (128, 2), (16, 3), (32, 3), (48, 3)] {
            let a = laplace_grid(n, dim);
            let b = rhs(a.n);
            let amg = Amg::new(
                &a,
                AmgSettings {
                    cycle_index: 2,
                    ..AmgSettings::default()
                },
            );
            let mut x = vec![0.0; a.n];
            let ctl = SolveControl {
                tol_abs: 0.0,
                tol_rel: 1e-8,
                max_iter: 100,
            };
            let st = amg.solve(&b, &mut x, &ctl);
            let mut x2 = vec![0.0; a.n];
            let sc = cg(
                &a,
                &b,
                &mut x2,
                &amg,
                &SolveControl {
                    tol_abs: 0.0,
                    tol_rel: 1e-8,
                    max_iter: 100,
                },
            );
            let mut r = vec![0.0; a.n];
            a.residual(&b, &x, &mut r);
            println!(
                "  level sizes {:?}",
                amg.levels.iter().map(|l| (l.a.n, l.a.nnz())).collect::<Vec<_>>()
            );
            println!(
                "{dim}D n={n} ({} unknowns): {} levels, op. complexity {:.2}, V-cycles {} (rate {:.3}), AMG-CG its {}",
                a.n,
                amg.n_levels(),
                amg.operator_complexity(),
                st.iterations,
                (st.final_residual / st.initial_residual).powf(1.0 / st.iterations as f64),
                sc.iterations
            );
            assert!(st.converged && sc.converged);
            assert!(norm2(&r) <= 1.1e-8 * st.initial_residual);
            counts.push((dim, st.iterations, sc.iterations));
        }
        // plain CG is far slower on the largest grid
        let a = laplace_grid(48, 3);
        let b = rhs(a.n);
        let mut x = vec![0.0; a.n];
        let plain = cg(
            &a,
            &b,
            &mut x,
            &Identity,
            &SolveControl {
                tol_abs: 0.0,
                tol_rel: 1e-8,
                max_iter: 2000,
            },
        );
        assert!(
            plain.iterations > 3 * counts.last().unwrap().2,
            "{} vs {:?}",
            plain.iterations,
            counts
        );
        // AMG-preconditioned CG iteration counts are essentially mesh independent;
        // the stand-alone V-cycle rate degrades only mildly with the number of levels
        for dim in [2, 3] {
            let c: Vec<(usize, usize)> = counts.iter().filter(|c| c.0 == dim).map(|c| (c.1, c.2)).collect();
            assert!(
                c.last().unwrap().1 <= c.first().unwrap().1 + 2,
                "dim {dim} AMG-CG: {c:?}"
            );
            assert!(c.iter().all(|&(v, _)| v <= 20), "dim {dim} V-cycles: {c:?}");
        }
    }

    #[test]
    fn works_as_preconditioner_for_nonsymmetric() {
        use crate::precond::Ilu0;
        use crate::solvers::bicgstab;
        let mut a = laplace_grid(40, 2);
        // add upwind-like convection x-direction: -1.5 on west neighbour, +0.5 on east
        for i in 0..a.n {
            for k in a.row_ptr[i]..a.row_ptr[i + 1] {
                if a.col[k] + 1 == i {
                    a.val[k] -= 0.5;
                }
                if a.col[k] == i + 1 {
                    a.val[k] += 0.5;
                }
            }
        }
        let b = rhs(a.n);
        let amg = Amg::new(&a, AmgSettings::default());
        let ctl = SolveControl {
            tol_abs: 0.0,
            tol_rel: 1e-8,
            max_iter: 200,
        };
        let mut x = vec![0.0; a.n];
        let st = bicgstab(&a, &b, &mut x, &amg, &ctl);
        let mut y = vec![0.0; a.n];
        let st_ilu = bicgstab(&a, &b, &mut y, &Ilu0::new(&a), &ctl);
        println!(
            "nonsymmetric 40x40: AMG-BiCGStab {} its, ILU0-BiCGStab {} its",
            st.iterations, st_ilu.iterations
        );
        assert!(st.converged && st.iterations < 30);
    }

    #[test]
    #[ignore = "1M unknowns: run with cargo test --release -p rcfd-linalg -- --ignored --nocapture"]
    fn poisson_one_million_cells() {
        let a = laplace_grid(100, 3);
        assert_eq!(a.n, 1_000_000);
        let b = rhs(a.n);
        let t0 = std::time::Instant::now();
        let amg = Amg::new(
            &a,
            AmgSettings {
                cycle_index: 2,
                ..AmgSettings::default()
            },
        );
        let t_setup = t0.elapsed().as_secs_f64();
        let mut x = vec![0.0; a.n];
        let t1 = std::time::Instant::now();
        let ctl = SolveControl {
            tol_abs: 0.0,
            tol_rel: 1e-8,
            max_iter: 100,
        };
        let st = amg.solve(&b, &mut x, &SolveControl { tol_rel: 1e-6, ..ctl });
        let t_vcycle = t1.elapsed().as_secs_f64();
        let mut y = vec![0.0; a.n];
        let t2 = std::time::Instant::now();
        let pc = cg(&a, &b, &mut y, &amg, &ctl);
        println!(
            "1M-cell Poisson: {} levels, op. complexity {:.2}, setup {:.1}s; stand-alone {} W(1,1)-cycles to 1e-6 in {:.1}s (rate {:.3}); AMG-CG {} iterations in {:.1}s",
            amg.n_levels(),
            amg.operator_complexity(),
            t_setup,
            st.iterations,
            t_vcycle,
            (st.final_residual / st.initial_residual).powf(1.0 / st.iterations as f64),
            pc.iterations,
            t2.elapsed().as_secs_f64()
        );
        assert!(pc.converged && pc.iterations < 15, "{pc:?}");
        assert!(st.converged && st.iterations < 15, "{st:?}");
    }
}
