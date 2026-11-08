//! CMA-ES, the covariance matrix adaptation evolution strategy (ADR 0222 D1), with optional restarts of increasing population
//! size.
//!
//! The algorithm is the (mu/mu_W, lambda)-CMA-ES of N. Hansen, "The CMA Evolution Strategy: A Tutorial" (arXiv:1604.00772v2,
//! 2023): Fig. 6, eqs. (38) to (47), with the default parameters of its Table 1 (eqs. 48 to 58, `alpha_cov = 2`, `c_m = 1`),
//! the termination criteria of its Appendix B.3 and, for the box, its rule (62) of Appendix B.5: a sample outside the box is not
//! evaluated and ranks below every evaluated one, by its distance to the centre of the box. The restarts double the population
//! as in A. Auger and N. Hansen, "A Restart CMA Evolution Strategy With Increasing Population Size" (CEC 2005). No source code
//! of any CMA-ES implementation was read (ADR 0222).
//!
//! The method runs in the unit cube `u_i = (x_i - lo_i)/(hi_i - lo_i)` with `C(0) = I` and `sigma(0) = sigma0` (default 0.3,
//! the tutorial's `0.3 (b - a)`). It asks for each generation's evaluations as one batch through the closure of the other
//! optimisers, so evaluation, resume and proposal from a store work the same way, and it is a deterministic function of (bounds,
//! options, evaluation results).

use super::optim::{clamp_into, Bounds, Eval, Fitness, OptResult, Stop};
use super::rng::Rng;
use std::cmp::Ordering;

/// Default initial step size in the unit cube (the tutorial's `sigma = 0.3 (b - a)`).
pub const SIGMA0: f64 = 0.3;
/// TolFun of the tutorial's Appendix B.3.
pub const TOL_FUN: f64 = 1e-12;
/// TolX of Appendix B.3, as a factor of the initial step size.
pub const TOL_X: f64 = 1e-12;
/// ConditionCov of Appendix B.3.
pub const CONDITION_COV: f64 = 1e14;
/// TolXUp of Appendix B.3.
pub const TOL_X_UP: f64 = 1e4;

/// Why a CMA-ES run ended (ADR 0222 D1 step 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// A non-finite state or a non-positive eigenvalue of the covariance matrix (a guard of ADR 0222).
    NonFinite,
    /// Adding 0.1 standard deviations along one principal axis does not change the mean.
    NoEffectAxis,
    /// Adding 0.2 standard deviations in one coordinate does not change the mean.
    NoEffectCoord,
    /// The condition number of the covariance matrix is above `CONDITION_COV`.
    ConditionCov,
    /// The best values of the last `10 + ceil(30 n / lambda)` generations are equal.
    EqualFunValues,
    /// The best and the median values stopped improving.
    Stagnation,
    /// `sigma max(D)` grew by more than `TOL_X_UP` times the initial step size.
    TolXUp,
    /// The recent best values and the last generation's values lie within `TOL_FUN`.
    TolFun,
    /// Every standard deviation and every component of `sigma p_c` is below `TOL_X sigma0`.
    TolX,
    /// The evaluation budget (or the generation cap) ended the method.
    Budget,
}

/// The tutorial's default population size `4 + floor(3 ln n)` (Table 1, eq. 48).
pub fn default_lambda(n: usize) -> usize {
    4 + (3.0 * (n as f64).ln()).floor() as usize
}

/// The strategy parameters of the tutorial's Table 1 for `n` variables and population `lambda`.
#[derive(Debug, Clone)]
pub struct Params {
    /// Number of variables.
    pub n: usize,
    /// Population size.
    pub lambda: usize,
    /// Number of positive weights, `floor(lambda / 2)`.
    pub mu: usize,
    /// Recombination weights `w_1 .. w_lambda` (eq. 53): positive ones sum to 1, negative ones are scaled.
    pub weights: Vec<f64>,
    /// Variance effective selection mass of the positive weights.
    pub mu_eff: f64,
    /// The same of the negative weights.
    pub mu_eff_minus: f64,
    /// Learning rate of the mean, 1.
    pub c_m: f64,
    /// Cumulation constant of the step-size path.
    pub c_sigma: f64,
    /// Damping of the step-size update.
    pub d_sigma: f64,
    /// Cumulation constant of the covariance path.
    pub c_c: f64,
    /// Learning rate of the rank-one update.
    pub c1: f64,
    /// Learning rate of the rank-mu update.
    pub c_mu: f64,
    /// `alpha_mu^- = 1 + c1 / c_mu` (eq. 50).
    pub alpha_mu_minus: f64,
    /// `alpha_mu_eff^- = 1 + 2 mu_eff^- / (mu_eff + 2)` (eq. 51).
    pub alpha_mu_eff_minus: f64,
    /// `alpha_posdef^- = (1 - c1 - c_mu) / (n c_mu)` (eq. 52).
    pub alpha_posdef_minus: f64,
    /// `E norm(N(0, I))` by the approximation `sqrt n (1 - 1/(4n) + 1/(21 n^2))`.
    pub chi_n: f64,
}

impl Params {
    /// Table 1 with `alpha_cov = 2` and the `c_c` of eq. (56).
    pub fn new(n: usize, lambda: usize) -> Params {
        let nf = n as f64;
        let lf = lambda as f64;
        let wp: Vec<f64> = (1..=lambda)
            .map(|i| ((lf + 1.0) / 2.0).ln() - (i as f64).ln())
            .collect();
        let mu = lambda / 2;
        let sum_sq = |w: &[f64]| -> (f64, f64) { (w.iter().sum(), w.iter().map(|x| x * x).sum()) };
        let (sp, sp2) = sum_sq(&wp[..mu]);
        let mu_eff = sp * sp / sp2;
        let (sn, sn2) = sum_sq(&wp[mu..]);
        let mu_eff_minus = if sn2 > 0.0 { sn * sn / sn2 } else { 0.0 };
        let alpha_cov = 2.0;
        let c_sigma = (mu_eff + 2.0) / (nf + mu_eff + 5.0);
        let d_sigma = 1.0 + 2.0 * f64::max(0.0, ((mu_eff - 1.0) / (nf + 1.0)).sqrt() - 1.0) + c_sigma;
        let c_c = (4.0 + mu_eff / nf) / (nf + 4.0 + 2.0 * mu_eff / nf);
        let c1 = alpha_cov / ((nf + 1.3).powi(2) + mu_eff);
        let c_mu = f64::min(
            1.0 - c1,
            alpha_cov * (0.25 + mu_eff + 1.0 / mu_eff - 2.0) / ((nf + 2.0).powi(2) + alpha_cov * mu_eff / 2.0),
        );
        let alpha_mu_minus = 1.0 + c1 / c_mu;
        let alpha_mu_eff_minus = 1.0 + 2.0 * mu_eff_minus / (mu_eff + 2.0);
        let alpha_posdef_minus = (1.0 - c1 - c_mu) / (nf * c_mu);
        let pos: f64 = wp.iter().filter(|w| **w >= 0.0).sum();
        let neg: f64 = -wp.iter().filter(|w| **w < 0.0).sum::<f64>();
        let amin = alpha_mu_minus.min(alpha_mu_eff_minus).min(alpha_posdef_minus);
        let weights = wp
            .iter()
            .map(|&w| if w >= 0.0 { w / pos } else { amin / neg * w })
            .collect();
        Params {
            n,
            lambda,
            mu,
            weights,
            mu_eff,
            mu_eff_minus,
            c_m: 1.0,
            c_sigma,
            d_sigma,
            c_c,
            c1,
            c_mu,
            alpha_mu_minus,
            alpha_mu_eff_minus,
            alpha_posdef_minus,
            chi_n: nf.sqrt() * (1.0 - 1.0 / (4.0 * nf) + 1.0 / (21.0 * nf * nf)),
        }
    }
}

/// The state of one CMA-ES run, in unit-cube coordinates.
#[derive(Debug, Clone)]
pub struct State {
    /// Mean of the distribution.
    pub m: Vec<f64>,
    /// Step size.
    pub sigma: f64,
    /// Covariance matrix (`c[i][j]`).
    pub c: Vec<Vec<f64>>,
    /// Evolution path of the step size.
    pub p_sigma: Vec<f64>,
    /// Evolution path of the covariance matrix.
    pub p_c: Vec<f64>,
    /// Eigenvectors of `c` as columns (`b[i][j]` is component `i` of eigenvector `j`), eigenvalues ascending.
    pub b: Vec<Vec<f64>>,
    /// Square roots of the eigenvalues (the diagonal of `D`), ascending.
    pub d: Vec<f64>,
    /// Eigenvalues of `c`, ascending.
    pub eig: Vec<f64>,
    /// Updates made in this run.
    pub g: usize,
}

fn identity(n: usize) -> Vec<Vec<f64>> {
    (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect()
}

impl State {
    /// A new run at mean `m` with step size `sigma`: `C = I`, both paths 0, `g = 0`.
    pub fn new(m: Vec<f64>, sigma: f64) -> State {
        let n = m.len();
        State {
            m,
            sigma,
            c: identity(n),
            p_sigma: vec![0.0; n],
            p_c: vec![0.0; n],
            b: identity(n),
            d: vec![1.0; n],
            eig: vec![1.0; n],
            g: 0,
        }
    }

    /// Recompute `b`, `d` and `eig` from `c` (ADR 0222 D2); an error (and nothing changed) when an eigenvalue is not positive
    /// and finite.
    pub fn decompose(&mut self) -> Result<(), String> {
        let (e, v) = sym_eigen(&self.c);
        if let Some(x) = e.iter().find(|x| !(x.is_finite() && **x > 0.0)) {
            return Err(format!("the covariance matrix has the eigenvalue {x}"));
        }
        self.d = e.iter().map(|x| x.sqrt()).collect();
        self.eig = e;
        self.b = v;
        Ok(())
    }
}

/// Eigenvalues (ascending) and eigenvectors (as columns, `v[i][j]`) of the symmetric matrix `a`, by cyclic Jacobi rotations
/// (ADR 0222 D2): sweeps over the pairs `p < q` in row order until the off-diagonal Frobenius norm is at most `1e-15` times the
/// Frobenius norm, at most 100 sweeps.
pub fn sym_eigen(a: &[Vec<f64>]) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = a.len();
    let mut a: Vec<Vec<f64>> = a.to_vec();
    let mut v = identity(n);
    let fro = a.iter().flatten().map(|x| x * x).sum::<f64>().sqrt();
    for _ in 0..100 {
        let off = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| a[i][j] * a[i][j])
            .sum::<f64>()
            .sqrt();
        if off <= 1e-15 * fro {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                if a[p][q] == 0.0 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = if theta >= 0.0 {
                    1.0 / (theta + (theta * theta + 1.0).sqrt())
                } else {
                    -1.0 / (-theta + (theta * theta + 1.0).sqrt())
                };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for row in a.iter_mut() {
                    let (x, y) = (row[p], row[q]);
                    row[p] = c * x - s * y;
                    row[q] = s * x + c * y;
                }
                for k in 0..n {
                    let (x, y) = (a[p][k], a[q][k]);
                    a[p][k] = c * x - s * y;
                    a[q][k] = s * x + c * y;
                }
                for row in v.iter_mut() {
                    let (x, y) = (row[p], row[q]);
                    row[p] = c * x - s * y;
                    row[q] = s * x + c * y;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&i, &j| a[i][i].total_cmp(&a[j][j]).then(i.cmp(&j)));
    let e = order.iter().map(|&i| a[i][i]).collect();
    let vs = (0..n).map(|r| order.iter().map(|&j| v[r][j]).collect()).collect();
    (e, vs)
}

/// One standard normal draw: `sqrt(-2 ln(1 - u1)) cos(2 pi u2)` from two uniforms (Box-Muller, the cosine branch).
pub fn normal(rng: &mut Rng) -> f64 {
    let u1 = rng.uniform();
    let u2 = rng.uniform();
    (-2.0 * (1.0 - u1).ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

/// `B D z`.
fn bdz(s: &State, z: &[f64]) -> Vec<f64> {
    let n = z.len();
    (0..n)
        .map(|r| (0..n).map(|j| s.b[r][j] * s.d[j] * z[j]).sum())
        .collect()
}

/// One update of the tutorial's Fig. 6, eqs. (41) to (47), from the `z` of the samples in rank order (best first), followed by
/// the eigendecomposition of the new `C` and `g + 1` (ADR 0222 D1 step 4). The `y` of the samples are `B D z` with the state's
/// `B` and `D` before the update. An error when the new `C` has an eigenvalue that is not positive and finite.
pub fn update(p: &Params, s: &mut State, ranked_z: &[Vec<f64>]) -> Result<(), String> {
    let n = p.n;
    let ys: Vec<Vec<f64>> = ranked_z.iter().map(|z| bdz(s, z)).collect();
    // (41) and the matching sum of the z (C^-1/2 <y>_w = B sum w_i z_i)
    let mut yw = vec![0.0; n];
    let mut zw = vec![0.0; n];
    for i in 0..p.mu {
        for j in 0..n {
            yw[j] += p.weights[i] * ys[i][j];
            zw[j] += p.weights[i] * ranked_z[i][j];
        }
    }
    // (42)
    for j in 0..n {
        s.m[j] += p.c_m * s.sigma * yw[j];
    }
    // (43)
    let cs = (p.c_sigma * (2.0 - p.c_sigma) * p.mu_eff).sqrt();
    for r in 0..n {
        let bzw: f64 = (0..n).map(|j| s.b[r][j] * zw[j]).sum();
        s.p_sigma[r] = (1.0 - p.c_sigma) * s.p_sigma[r] + cs * bzw;
    }
    // (44)
    let norm_ps = s.p_sigma.iter().map(|x| x * x).sum::<f64>().sqrt();
    s.sigma *= ((p.c_sigma / p.d_sigma) * (norm_ps / p.chi_n - 1.0)).exp();
    // h_sigma, with g the number of updates before this one
    let var = 1.0 - (1.0 - p.c_sigma).powf(2.0 * (s.g as f64 + 1.0));
    let h_sigma = norm_ps / var.sqrt() < (1.4 + 2.0 / (n as f64 + 1.0)) * p.chi_n;
    // (45)
    let cc = (p.c_c * (2.0 - p.c_c) * p.mu_eff).sqrt();
    for j in 0..n {
        s.p_c[j] = (1.0 - p.c_c) * s.p_c[j] + if h_sigma { cc * yw[j] } else { 0.0 };
    }
    // (46)
    let w_o: Vec<f64> = p
        .weights
        .iter()
        .zip(ranked_z)
        .map(|(&w, z)| {
            if w >= 0.0 {
                w
            } else {
                w * n as f64 / z.iter().map(|x| x * x).sum::<f64>()
            }
        })
        .collect();
    // (47)
    let delta = if h_sigma { 0.0 } else { p.c_c * (2.0 - p.c_c) };
    let sum_w: f64 = p.weights.iter().sum();
    let factor = 1.0 + p.c1 * delta - p.c1 - p.c_mu * sum_w;
    let mut c = vec![vec![0.0; n]; n];
    for r in 0..n {
        for q in 0..n {
            let rank_mu: f64 = w_o.iter().zip(&ys).map(|(w, y)| w * y[r] * y[q]).sum();
            c[r][q] = factor * s.c[r][q] + p.c1 * s.p_c[r] * s.p_c[q] + p.c_mu * rank_mu;
        }
    }
    for r in 0..n {
        for q in 0..r {
            let x = 0.5 * (c[r][q] + c[q][r]);
            c[r][q] = x;
            c[q][r] = x;
        }
    }
    s.c = c;
    s.g += 1;
    s.decompose()
}

/// The values of the generations of one run that the termination criteria read (ADR 0222 D1 step 5).
#[derive(Debug, Clone, Default)]
pub struct History {
    /// Per generation: `f` of the rank-1 sample when it was evaluated, is feasible and finite.
    pub best: Vec<Option<f64>>,
    /// Per generation: the mean of `f` of the samples of ranks `floor((lambda + 1)/2)` and `ceil((lambda + 1)/2)` when both were
    /// evaluated, are feasible and finite.
    pub median: Vec<Option<f64>>,
    /// `f` of the evaluated, feasible, finite samples of the last generation.
    pub current: Vec<f64>,
}

/// The last `k` entries of `v` when there are `k` of them and every one is a value.
fn last_values(v: &[Option<f64>], k: usize) -> Option<Vec<f64>> {
    if v.len() < k {
        return None;
    }
    v[v.len() - k..].iter().copied().collect()
}

/// The middle value of `v` (the mean of the two middle values for an even count).
fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let k = s.len();
    if k % 2 == 1 {
        s[k / 2]
    } else {
        0.5 * (s[k / 2 - 1] + s[k / 2])
    }
}

fn range_of(v: &[f64]) -> f64 {
    let (lo, hi) = v
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), x| (l.min(*x), h.max(*x)));
    hi - lo
}

/// The first termination criterion of ADR 0222 D1 step 5 that holds for the state `s` after an update and the history `h`, in
/// the order `NonFinite`, `NoEffectAxis`, `NoEffectCoord`, `ConditionCov`, `EqualFunValues`, `Stagnation`, `TolXUp`, `TolFun`,
/// `TolX`; `sigma0` is the run's initial step size.
pub fn termination(p: &Params, s: &State, h: &History, sigma0: f64) -> Option<Termination> {
    let n = p.n;
    let finite = s.sigma.is_finite()
        && s.m.iter().all(|x| x.is_finite())
        && s.p_sigma.iter().all(|x| x.is_finite())
        && s.p_c.iter().all(|x| x.is_finite())
        && s.c.iter().flatten().all(|x| x.is_finite())
        && s.eig.iter().all(|x| x.is_finite() && *x > 0.0);
    if !finite {
        return Some(Termination::NonFinite);
    }
    let axis = s.g % n;
    if (0..n).all(|r| s.m[r] + 0.1 * s.sigma * s.d[axis] * s.b[r][axis] == s.m[r]) {
        return Some(Termination::NoEffectAxis);
    }
    if (0..n).any(|r| s.m[r] + 0.2 * s.sigma * s.c[r][r].sqrt() == s.m[r]) {
        return Some(Termination::NoEffectCoord);
    }
    let emin = s.eig.iter().copied().fold(f64::INFINITY, f64::min);
    let emax = s.eig.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if emax / emin > CONDITION_COV {
        return Some(Termination::ConditionCov);
    }
    let lambda = p.lambda;
    let ceil_30 = (30 * n).div_ceil(lambda);
    let w = 10 + ceil_30;
    let recent = if s.g >= w { last_values(&h.best, w) } else { None };
    if let Some(r) = &recent {
        if range_of(r) == 0.0 {
            return Some(Termination::EqualFunValues);
        }
    }
    let l = (120 + ceil_30).max(s.g.div_ceil(5)).min(20_000);
    if s.g >= l {
        if let (Some(b), Some(md)) = (last_values(&h.best, l), last_values(&h.median, l)) {
            let k = (3 * l).div_ceil(10);
            let worse = |v: &[f64]| median(&v[l - k..]) >= median(&v[..k]);
            if worse(&b) && worse(&md) {
                return Some(Termination::Stagnation);
            }
        }
    }
    let dmax = s.d.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if s.sigma * dmax > TOL_X_UP * sigma0 {
        return Some(Termination::TolXUp);
    }
    if let Some(r) = recent {
        let mut all = r;
        all.extend_from_slice(&h.current);
        if range_of(&all) < TOL_FUN {
            return Some(Termination::TolFun);
        }
    }
    let tol_x = TOL_X * sigma0;
    if (0..n).all(|r| s.sigma * s.c[r][r].sqrt() < tol_x && s.sigma * s.p_c[r].abs() < tol_x) {
        return Some(Termination::TolX);
    }
    None
}

/// Options of [`run`].
#[derive(Debug, Clone)]
pub struct Options {
    /// Evaluation budget.
    pub max_evals: usize,
    /// Population size of the first run (default [`default_lambda`]).
    pub pop: Option<usize>,
    /// Initial step size in the unit cube (default [`SIGMA0`]).
    pub sigma0: f64,
    /// Restarts after a run ends by a termination criterion, each with the population doubled.
    pub restarts: usize,
    /// Seed.
    pub seed: u64,
    /// Start point, the mean of every run (default: a uniform random point in the box per run).
    pub x0: Option<Vec<f64>>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            max_evals: 1000,
            pop: None,
            sigma0: SIGMA0,
            restarts: 0,
            seed: 1,
            x0: None,
        }
    }
}

/// One run of a CMA-ES method (the first one and each restart).
#[derive(Debug, Clone, PartialEq)]
pub struct RunInfo {
    /// Population size.
    pub lambda: usize,
    /// Start point of the mean, in parameter space.
    pub m0: Vec<f64>,
    /// Generations evaluated (a generation that ends the method by the budget is not counted).
    pub generations: usize,
    /// Evaluations requested.
    pub evaluations: usize,
    /// Why the run ended.
    pub stop: Termination,
}

fn cmp_fitness(a: &Fitness, b: &Fitness) -> Ordering {
    if a.better(b) {
        Ordering::Less
    } else if b.better(a) {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

/// CMA-ES in the box `b` (ADR 0222 D1): [`run_report`] without the per-run report.
pub fn run(b: &Bounds, opts: &Options, eval: &mut Eval<'_>) -> Result<OptResult, Stop> {
    run_report(b, opts, eval).map(|(r, _)| r)
}

/// CMA-ES in the box `b` with the restarts of `opts` (ADR 0222 D1): the best evaluated point and one [`RunInfo`] per run.
pub fn run_report(b: &Bounds, opts: &Options, eval: &mut Eval<'_>) -> Result<(OptResult, Vec<RunInfo>), Stop> {
    let n = b.len();
    if n == 0 {
        return Err(Stop::Error("no parameters".into()));
    }
    for (i, &(lo, hi)) in b.iter().enumerate() {
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(Stop::Error(format!(
                "parameter {i}: the bounds must be finite with lo < hi (got {lo}, {hi})"
            )));
        }
    }
    let lambda0 = opts.pop.unwrap_or_else(|| default_lambda(n));
    if lambda0 < 2 {
        return Err(Stop::Error(format!("the population must be at least 2, got {lambda0}")));
    }
    if !(opts.sigma0.is_finite() && opts.sigma0 > 0.0) {
        return Err(Stop::Error(format!(
            "sigma0 must be positive and finite, got {}",
            opts.sigma0
        )));
    }
    if opts.max_evals < lambda0 {
        return Err(Stop::Error(format!(
            "the evaluation budget {} is smaller than the population {lambda0}",
            opts.max_evals
        )));
    }
    let x0 = match &opts.x0 {
        Some(x) if x.len() != n => return Err(Stop::Error("x0 does not match the bounds".into())),
        Some(x) => {
            let mut x = x.clone();
            clamp_into(&mut x, b);
            Some(x)
        }
        None => None,
    };
    let range: Vec<f64> = b.iter().map(|&(lo, hi)| hi - lo).collect();
    let to_x = |u: &[f64]| -> Vec<f64> {
        let mut x: Vec<f64> = (0..n).map(|i| b[i].0 + u[i] * range[i]).collect();
        clamp_into(&mut x, b);
        x
    };
    let mut rng = Rng::new(opts.seed);
    let (mut evals, mut generations) = (0usize, 0usize);
    let mut best: Option<(Vec<f64>, Fitness)> = None;
    let mut runs = Vec::new();
    let mut lambda = lambda0;
    let mut budget_end = false;
    for _ in 0..=opts.restarts {
        let (m0, u0) = match &x0 {
            Some(x) => (x.clone(), (0..n).map(|i| (x[i] - b[i].0) / range[i]).collect()),
            None => {
                let u: Vec<f64> = (0..n).map(|_| rng.uniform()).collect();
                (to_x(&u), u)
            }
        };
        let p = Params::new(n, lambda);
        let mut s = State::new(u0, opts.sigma0);
        let mut h = History::default();
        let mut info = RunInfo {
            lambda,
            m0,
            generations: 0,
            evaluations: 0,
            stop: Termination::Budget,
        };
        loop {
            if generations >= opts.max_evals {
                budget_end = true;
                break;
            }
            let mut zs = Vec::with_capacity(lambda);
            let mut us = Vec::with_capacity(lambda);
            for _ in 0..lambda {
                let z: Vec<f64> = (0..n).map(|_| normal(&mut rng)).collect();
                let y = bdz(&s, &z);
                us.push((0..n).map(|i| s.m[i] + s.sigma * y[i]).collect::<Vec<f64>>());
                zs.push(z);
            }
            let inside: Vec<usize> = (0..lambda)
                .filter(|&k| us[k].iter().all(|v| (0.0..=1.0).contains(v)))
                .collect();
            if evals + inside.len() > opts.max_evals {
                budget_end = true;
                break;
            }
            let mut fit: Vec<Option<Fitness>> = vec![None; lambda];
            if !inside.is_empty() {
                let xs: Vec<Vec<f64>> = inside.iter().map(|&k| to_x(&us[k])).collect();
                let f = eval(&xs)?;
                if f.len() != xs.len() {
                    return Err(Stop::Error(format!(
                        "the evaluator returned {} results for {} points",
                        f.len(),
                        xs.len()
                    )));
                }
                for ((x, fi), &k) in xs.iter().zip(&f).zip(&inside) {
                    if best.as_ref().is_none_or(|(_, bf)| fi.better(bf)) {
                        best = Some((x.clone(), *fi));
                    }
                    fit[k] = Some(*fi);
                }
            }
            evals += inside.len();
            generations += 1;
            info.evaluations += inside.len();
            info.generations += 1;
            // ranking: evaluated samples by Deb's rules, then the others by their distance to the centre of the box
            let mut ranked = inside.clone();
            ranked.sort_by(|&a, &c| cmp_fitness(fit[a].as_ref().unwrap(), fit[c].as_ref().unwrap()));
            let dist = |k: usize| us[k].iter().map(|v| (v - 0.5) * (v - 0.5)).sum::<f64>().sqrt();
            let mut out: Vec<usize> = (0..lambda).filter(|k| fit[*k].is_none()).collect();
            out.sort_by(|&a, &c| dist(a).total_cmp(&dist(c)).then(a.cmp(&c)));
            ranked.extend(out);
            let value = |k: usize| fit[k].filter(|f| f.feasible() && f.f.is_finite()).map(|f| f.f);
            h.best.push(value(ranked[0]));
            // ranks floor((lambda + 1)/2) and ceil((lambda + 1)/2), 0-based
            let (ma, mb) = (lambda.div_ceil(2) - 1, (lambda + 1).div_ceil(2) - 1);
            h.median.push(match (value(ranked[ma]), value(ranked[mb])) {
                (Some(x), Some(y)) => Some(0.5 * (x + y)),
                _ => None,
            });
            h.current = inside.iter().filter_map(|&k| value(k)).collect();
            let rz: Vec<Vec<f64>> = ranked.iter().map(|&k| zs[k].clone()).collect();
            let stop = match update(&p, &mut s, &rz) {
                Ok(()) => termination(&p, &s, &h, opts.sigma0),
                Err(_) => Some(Termination::NonFinite),
            };
            if let Some(t) = stop {
                info.stop = t;
                break;
            }
        }
        runs.push(info);
        if budget_end {
            break;
        }
        lambda = lambda.saturating_mul(2);
    }
    let (x, fitness) = best.ok_or_else(|| Stop::Error("no point inside the box was evaluated".into()))?;
    Ok((
        OptResult {
            x,
            fitness,
            evaluations: evals,
            converged: !budget_end,
        },
        runs,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_evaluate_nothing() {
        let mut calls = 0;
        let mut ev = |xs: &[Vec<f64>]| -> Result<Vec<Fitness>, Stop> {
            calls += xs.len();
            Ok(xs.iter().map(|x| Fitness::new(x[0], 0.0)).collect())
        };
        let b = [(0.0, 1.0), (0.0, 1.0)];
        for o in [
            Options {
                pop: Some(1),
                ..Default::default()
            },
            Options {
                sigma0: 0.0,
                ..Default::default()
            },
            Options {
                max_evals: 3,
                ..Default::default()
            },
        ] {
            assert!(matches!(run(&b, &o, &mut ev), Err(Stop::Error(_))));
        }
        assert!(matches!(run(&[], &Options::default(), &mut ev), Err(Stop::Error(_))));
        assert!(matches!(
            run(&[(1.0, 1.0)], &Options::default(), &mut ev),
            Err(Stop::Error(_))
        ));
        assert_eq!(calls, 0);
    }
}
