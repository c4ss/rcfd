//! Gaussian-process regression for the design loop (ADR 0621, F-698): ordinary kriging with an unknown constant mean, one
//! length scale per parameter, the process variance and the mean profiled out, and the length scales (and, opt-in, a nugget)
//! fitted by maximum likelihood with the analytic gradient (Rasmussen and Williams 2006, chapters 2, 4 and 5; Jones, Schonlau
//! and Welch 1998, section 2). Dense Cholesky factorisation; inputs are points of the unit cube, outputs are standardised for
//! the fit and mapped back. The model knows nothing of the ledger or the optimisers (`bayes` does).
//!
//! Opt-in (ADR 0663, F-735): a zero-mean mode with an absolute noise variance per observation and the process variance a
//! hyperparameter, no standardisation (`Gp::zero_mean_with_hyper`, `Gp::fit_zero_mean`); rung R3 of `fidelity` uses it.
//!
//! Deterministic: a fit is a function of (inputs, outputs, options) only; the multi-start draws come from `Rng::new(seed)` in
//! a fixed order and the starts are evaluated in index order (in parallel when `workers > 1`, results kept in index order).

use super::optim::{projected_gradient, PgOptions, Stop};
use super::sampling::latin_hypercube;
use super::study::parallel_map;

/// Correlation function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kernel {
    /// Matern, smoothness 5/2: `(1 + r + r^2/3) exp(-r)`, `r = sqrt(5 sum (d_i / l_i)^2)` (Rasmussen and Williams eq. 4.17).
    #[default]
    Matern52,
    /// Squared exponential `exp(-sum (d_i / l_i)^2 / 2)` (opt-in).
    SquaredExponential,
}

/// Observation noise.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Noise {
    /// Interpolation: no nugget, the jitter only (default).
    #[default]
    None,
    /// One nugget ratio `g` (noise variance over process variance), fitted with the length scales in `[g_lo, g_hi]`.
    Fitted,
    /// A fixed nugget ratio `g >= 0`.
    Nugget(f64),
}

/// Diagonal jitter relative to the unit prior variance, always added.
pub const JITTER: f64 = 1e-8;
/// The jitter is multiplied by 100 at most this many times when the factorisation fails (to `1e-2`).
pub const JITTER_STEPS: usize = 3;
/// Largest number of observations of one fit.
pub const N_MAX: usize = 2000;

/// Options of [`Gp::fit`].
#[derive(Debug, Clone, PartialEq)]
pub struct GpOptions {
    /// Kernel.
    pub kernel: Kernel,
    /// Noise.
    pub noise: Noise,
    /// Starts of the likelihood search (the centre of the log box and `starts - 1` Latin-hypercube points).
    pub starts: usize,
    /// Starts refined by the gradient search (the best by likelihood, ties by start order).
    pub refine: usize,
    /// Gradient iterations per refined start.
    pub iters: usize,
    /// Length-scale box (unit-cube lengths).
    pub l_bounds: (f64, f64),
    /// Nugget-ratio box of `Noise::Fitted`.
    pub g_bounds: (f64, f64),
    /// Seed of the multi-start design.
    pub seed: u64,
    /// Threads for the start evaluations (results in index order; any count gives the same fit).
    pub workers: usize,
}

impl Default for GpOptions {
    fn default() -> Self {
        GpOptions {
            kernel: Kernel::Matern52,
            noise: Noise::None,
            starts: 10,
            refine: 3,
            iters: 40,
            l_bounds: (0.05, 10.0),
            g_bounds: (1e-8, 1.0),
            seed: 1,
            workers: 1,
        }
    }
}

/// Options of [`Gp::fit_zero_mean`] (ADR 0663 D2): the search of [`Gp::fit`] over `(l, s_f^2)`, the process variance
/// box in output units squared (the mode does not scale its outputs).
#[derive(Debug, Clone, PartialEq)]
pub struct ZeroMeanOptions {
    /// Kernel.
    pub kernel: Kernel,
    /// Starts of the likelihood search (the centre of the log box and `starts - 1` Latin-hypercube points).
    pub starts: usize,
    /// Starts refined by the gradient search (the best by likelihood, ties by start order).
    pub refine: usize,
    /// Gradient iterations per refined start.
    pub iters: usize,
    /// Length-scale box (unit-cube lengths).
    pub l_bounds: (f64, f64),
    /// Process-variance box (output units squared).
    pub sf2_bounds: (f64, f64),
    /// Seed of the multi-start design.
    pub seed: u64,
    /// Threads for the start evaluations (results in index order; any count gives the same fit).
    pub workers: usize,
}

impl Default for ZeroMeanOptions {
    fn default() -> Self {
        ZeroMeanOptions {
            kernel: Kernel::Matern52,
            starts: 10,
            refine: 3,
            iters: 40,
            l_bounds: (0.05, 10.0),
            sf2_bounds: (1e-6, 1e2),
            seed: 1,
            workers: 1,
        }
    }
}

/// Dense Cholesky `A = L L^T` of the symmetric `n x n` row-major `a` (lower triangle read). The lower factor, row-major; a
/// pivot `<= (row + 1) eps A_ii` refuses with `(row, pivot)`.
pub fn cholesky(a: &[f64], n: usize) -> Result<Vec<f64>, (usize, f64)> {
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = a[i * n + j];
            for k in 0..j {
                s -= l[i * n + k] * l[j * n + k];
            }
            if i == j {
                if s.is_nan() || s <= (i + 1) as f64 * f64::EPSILON * a[i * n + i].abs() {
                    return Err((i, s));
                }
                l[i * n + i] = s.sqrt();
            } else {
                l[i * n + j] = s / l[j * n + j];
            }
        }
    }
    Ok(l)
}

/// Solve `L x = b` (forward substitution).
pub fn solve_lower(l: &[f64], n: usize, b: &[f64]) -> Vec<f64> {
    let mut x = b.to_vec();
    for i in 0..n {
        let mut s = x[i];
        for k in 0..i {
            s -= l[i * n + k] * x[k];
        }
        x[i] = s / l[i * n + i];
    }
    x
}

/// Solve `L^T x = b` (back substitution).
pub fn solve_upper_t(l: &[f64], n: usize, b: &[f64]) -> Vec<f64> {
    let mut x = b.to_vec();
    for i in (0..n).rev() {
        let mut s = x[i];
        for k in i + 1..n {
            s -= l[k * n + i] * x[k];
        }
        x[i] = s / l[i * n + i];
    }
    x
}

/// Inverse from a Cholesky factor, column by column (row-major).
fn chol_inverse(chol: &[f64], n: usize) -> Vec<f64> {
    let mut g = vec![0.0; n * n];
    for k in 0..n {
        let mut e = vec![0.0; n];
        e[k] = 1.0;
        let col = solve_upper_t(chol, n, &solve_lower(chol, n, &e));
        for (i, v) in col.into_iter().enumerate() {
            g[i * n + k] = v;
        }
    }
    g
}

fn check_noise(n: usize, v: &[f64]) -> Result<(), String> {
    if v.len() != n {
        return Err(format!("gp: {} noise variances for {n} observations", v.len()));
    }
    if let Some(i) = v.iter().position(|x| !(x.is_finite() && *x >= 0.0)) {
        return Err(format!("gp: noise variance {i} ({}) must be finite and >= 0", v[i]));
    }
    Ok(())
}

/// Squared scaled distance `sum ((a_i - b_i) / l_i)^2`.
fn dist2(a: &[f64], b: &[f64], l: &[f64]) -> f64 {
    a.iter().zip(b).zip(l).map(|((x, y), s)| ((x - y) / s).powi(2)).sum()
}

/// Correlation of two points.
pub fn correlation(kernel: Kernel, a: &[f64], b: &[f64], l: &[f64]) -> f64 {
    let q = dist2(a, b, l);
    match kernel {
        Kernel::Matern52 => {
            let r = (5.0 * q).sqrt();
            (1.0 + r + r * r / 3.0) * (-r).exp()
        }
        Kernel::SquaredExponential => (-0.5 * q).exp(),
    }
}

/// `d k / d ln l_i` divided by `((a_i - b_i) / l_i)^2`: the factor common to every dimension.
fn dcorr_common(kernel: Kernel, a: &[f64], b: &[f64], l: &[f64]) -> f64 {
    let q = dist2(a, b, l);
    match kernel {
        Kernel::Matern52 => {
            let r = (5.0 * q).sqrt();
            5.0 / 3.0 * (1.0 + r) * (-r).exp()
        }
        Kernel::SquaredExponential => (-0.5 * q).exp(),
    }
}

/// A fitted Gaussian process.
#[derive(Debug, Clone)]
pub struct Gp {
    kernel: Kernel,
    u: Vec<Vec<f64>>,
    ys: Vec<f64>,
    l: Vec<f64>,
    g: f64,
    jitter: f64,
    y_mean: f64,
    y_sd: f64,
    mu: f64,
    s2: f64,
    chol: Vec<f64>,
    alpha: Vec<f64>,
    kinv1: Vec<f64>,
    one_kinv_one: f64,
    loglik: f64,
    /// ADR 0663: the zero-mean mode (`y ~ N(0, s2 (R + J I) + diag(noise))`, no standardisation, `s2` not profiled).
    zero_mean: bool,
    /// Absolute noise variances of the zero-mean mode (empty otherwise).
    noise: Vec<f64>,
}

/// Value and gradient of the concentrated log likelihood at one hyperparameter vector, or `None` when the factorisation
/// fails at every jitter.
struct LikEval {
    value: f64,
    grad: Vec<f64>,
}

fn standardise(y: &[f64]) -> (f64, f64, Vec<f64>) {
    let n = y.len() as f64;
    let m = y.iter().sum::<f64>() / n;
    let v = y.iter().map(|v| (v - m).powi(2)).sum::<f64>() / n;
    let sd = if v > 0.0 { v.sqrt() } else { 1.0 };
    (m, sd, y.iter().map(|v| (v - m) / sd).collect())
}

fn check_inputs(u: &[Vec<f64>], y: &[f64]) -> Result<usize, String> {
    let n = u.len();
    if n == 0 || y.len() != n {
        return Err(format!(
            "gp: {} inputs and {} outputs (need the same number, at least 1)",
            n,
            y.len()
        ));
    }
    if n > N_MAX {
        return Err(format!("gp: {n} observations exceed the cap {N_MAX}"));
    }
    let d = u[0].len();
    if d == 0 {
        return Err("gp: inputs have no coordinate".into());
    }
    for (i, p) in u.iter().enumerate() {
        if p.len() != d {
            return Err(format!("gp: input {i} has {} coordinates, input 0 has {d}", p.len()));
        }
        if let Some(c) = p.iter().position(|v| !v.is_finite()) {
            return Err(format!("gp: input {i} coordinate {c} is not finite"));
        }
    }
    if let Some(i) = y.iter().position(|v| !v.is_finite()) {
        return Err(format!("gp: output {i} ({}) is not finite", y[i]));
    }
    Ok(d)
}

impl Gp {
    /// The process with given length scales and nugget ratio (no search): the mean and the process variance are profiled.
    pub fn with_hyper(u: &[Vec<f64>], y: &[f64], kernel: Kernel, l: &[f64], g: f64) -> Result<Gp, String> {
        let d = check_inputs(u, y)?;
        if l.len() != d || l.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
            return Err(format!(
                "gp: {} length scales for {d} coordinates, each finite and > 0 needed",
                l.len()
            ));
        }
        if !(g.is_finite() && g >= 0.0) {
            return Err(format!("gp: nugget ratio {g} must be finite and >= 0"));
        }
        let (y_mean, y_sd, ys) = standardise(y);
        Self::build(u, &ys, kernel, l, g, y_mean, y_sd)
    }

    fn build(
        u: &[Vec<f64>],
        ys: &[f64],
        kernel: Kernel,
        l: &[f64],
        g: f64,
        y_mean: f64,
        y_sd: f64,
    ) -> Result<Gp, String> {
        let n = u.len();
        let r = corr_matrix(u, kernel, l);
        let mut jitter = JITTER;
        let mut last = (0, 0.0);
        for _ in 0..=JITTER_STEPS {
            let mut k = r.clone();
            for i in 0..n {
                k[i * n + i] += g + jitter;
            }
            match cholesky(&k, n) {
                Ok(chol) => {
                    let ones = vec![1.0; n];
                    let kinv1 = solve_upper_t(&chol, n, &solve_lower(&chol, n, &ones));
                    let kinvy = solve_upper_t(&chol, n, &solve_lower(&chol, n, ys));
                    let one_kinv_one: f64 = kinv1.iter().sum();
                    let mu = kinvy.iter().sum::<f64>() / one_kinv_one;
                    let alpha: Vec<f64> = kinvy.iter().zip(&kinv1).map(|(a, b)| a - mu * b).collect();
                    let s2 = ys.iter().zip(&alpha).map(|(y, a)| (y - mu) * a).sum::<f64>() / n as f64;
                    let logdet_half: f64 = (0..n).map(|i| chol[i * n + i].ln()).sum();
                    let s2c = s2.max(f64::MIN_POSITIVE);
                    let loglik = -0.5 * n as f64 * s2c.ln() - logdet_half;
                    return Ok(Gp {
                        kernel,
                        u: u.to_vec(),
                        ys: ys.to_vec(),
                        l: l.to_vec(),
                        g,
                        jitter,
                        y_mean,
                        y_sd,
                        mu,
                        s2,
                        chol,
                        alpha,
                        kinv1,
                        one_kinv_one,
                        loglik,
                        zero_mean: false,
                        noise: Vec::new(),
                    });
                }
                Err(e) => last = e,
            }
            jitter *= 100.0;
        }
        Err(format!(
            "gp: the correlation matrix is not positive definite at jitter {:e} (row {}, pivot {:e}): near-duplicate inputs or length scales too long",
            jitter / 100.0,
            last.0,
            last.1
        ))
    }

    /// The zero-mean process with absolute noise (ADR 0663 D1): `y ~ N(0, C)`, `C = sf2 (R + J I) + diag(noise_var)`, the
    /// outputs neither centred nor scaled, `noise_var` in output units squared; given length scales and process variance.
    pub fn zero_mean_with_hyper(
        u: &[Vec<f64>],
        y: &[f64],
        noise_var: &[f64],
        kernel: Kernel,
        l: &[f64],
        sf2: f64,
    ) -> Result<Gp, String> {
        let d = check_inputs(u, y)?;
        check_noise(u.len(), noise_var)?;
        if l.len() != d || l.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
            return Err(format!(
                "gp: {} length scales for {d} coordinates, each finite and > 0 needed",
                l.len()
            ));
        }
        if !(sf2.is_finite() && sf2 > 0.0) {
            return Err(format!("gp: process variance {sf2} must be finite and > 0"));
        }
        Self::build_zero_mean(u, y, noise_var, kernel, l, sf2)
            .map_err(|(i, p)| format!("gp: covariance not positive definite (row {i}, pivot {p:e})"))
    }

    /// The covariance, its factor, `alpha = C^-1 y` and the log likelihood `-y^T alpha / 2 - sum ln L_ii` of the zero-mean
    /// mode (fixed jitter, no escalation: ADR 0663 D1).
    fn build_zero_mean(
        u: &[Vec<f64>],
        y: &[f64],
        noise_var: &[f64],
        kernel: Kernel,
        l: &[f64],
        sf2: f64,
    ) -> Result<Gp, (usize, f64)> {
        let n = u.len();
        let mut c = vec![0.0; n * n];
        for i in 0..n {
            c[i * n + i] = sf2 * (1.0 + JITTER) + noise_var[i];
            for j in 0..i {
                let v = sf2 * correlation(kernel, &u[i], &u[j], l);
                c[i * n + j] = v;
                c[j * n + i] = v;
            }
        }
        let chol = cholesky(&c, n)?;
        let alpha = solve_upper_t(&chol, n, &solve_lower(&chol, n, y));
        let loglik = -0.5 * y.iter().zip(&alpha).map(|(a, b)| a * b).sum::<f64>()
            - (0..n).map(|i| chol[i * n + i].ln()).sum::<f64>();
        Ok(Gp {
            kernel,
            u: u.to_vec(),
            ys: y.to_vec(),
            l: l.to_vec(),
            g: 0.0,
            jitter: JITTER,
            y_mean: 0.0,
            y_sd: 1.0,
            mu: 0.0,
            s2: sf2,
            chol,
            alpha,
            kinv1: Vec::new(),
            one_kinv_one: 0.0,
            loglik,
            zero_mean: true,
            noise: noise_var.to_vec(),
        })
    }

    /// Fit the length scales and the process variance of the zero-mean mode by maximum likelihood (ADR 0663 D2), the
    /// multi-start search of [`Gp::fit`] over the log box of `(l, sf2)`.
    pub fn fit_zero_mean(u: &[Vec<f64>], y: &[f64], noise_var: &[f64], o: &ZeroMeanOptions) -> Result<Gp, String> {
        let d = check_inputs(u, y)?;
        check_noise(u.len(), noise_var)?;
        if o.starts == 0 || o.refine == 0 {
            return Err("gp: starts and refine must be >= 1".into());
        }
        for (name, (a, b)) in [("length-scale", o.l_bounds), ("process-variance", o.sf2_bounds)] {
            if !(a > 0.0 && b > a && b.is_finite()) {
                return Err(format!("gp: {name} bounds ({a}, {b}) need 0 < lo < hi"));
            }
        }
        let mut bx: Vec<(f64, f64)> = vec![(o.l_bounds.0.ln(), o.l_bounds.1.ln()); d];
        bx.push((o.sf2_bounds.0.ln(), o.sf2_bounds.1.ln()));
        let hyper = |t: &[f64]| -> (Vec<f64>, f64) { (t[..d].iter().map(|v| v.exp()).collect(), t[d].exp()) };
        let eval = |t: &[f64]| -> Option<Gp> {
            let (l, sf2) = hyper(t);
            Self::build_zero_mean(u, y, noise_var, o.kernel, &l, sf2)
                .ok()
                .filter(|gp| gp.loglik.is_finite())
        };
        let value = |t: &[f64]| -> f64 { eval(t).map_or(f64::NEG_INFINITY, |gp| gp.loglik) };
        let mut starts: Vec<Vec<f64>> = vec![bx.iter().map(|(a, b)| 0.5 * (a + b)).collect()];
        for p in latin_hypercube(o.starts - 1, d + 1, o.seed) {
            starts.push(p.iter().zip(&bx).map(|(v, (a, b))| a + (b - a) * v).collect());
        }
        let vals = parallel_map(&starts, o.workers, &|t: &Vec<f64>| value(t));
        let mut order: Vec<usize> = (0..starts.len()).collect();
        order.sort_by(|&a, &b| vals[b].total_cmp(&vals[a]).then(a.cmp(&b)));
        let chosen: Vec<usize> = order.into_iter().take(o.refine).collect();
        let refine = |&s: &usize| -> (Vec<f64>, f64) {
            let opts = PgOptions {
                max_iters: o.iters,
                step: 0.1,
                armijo: 1e-4,
                max_halvings: 30,
                x0: starts[s].clone(),
            };
            let mut ev = |t: &[f64], grad: bool| -> Result<(f64, Vec<f64>), Stop> {
                match eval(t) {
                    None => Ok((f64::INFINITY, vec![0.0; d + 1])),
                    Some(gp) => {
                        if grad {
                            let le = gp.zero_mean_gradient();
                            Ok((-le.value, le.grad.iter().map(|v| -v).collect()))
                        } else {
                            Ok((-gp.loglik, Vec::new()))
                        }
                    }
                }
            };
            match projected_gradient(&bx, &opts, &mut ev) {
                Ok(r) if r.fitness.f.is_finite() => (r.x, -r.fitness.f),
                _ => (starts[s].clone(), vals[s]),
            }
        };
        let refined = parallel_map(&chosen, o.workers, &refine);
        let mut best: Option<(Vec<f64>, f64)> = None;
        for (t, v) in refined {
            if best.as_ref().is_none_or(|b| v > b.1) {
                best = Some((t, v));
            }
        }
        let (t, v) = best.expect("refine >= 1");
        if !v.is_finite() {
            return Err("gp: no start of the likelihood search factorised".into());
        }
        let (l, sf2) = hyper(&t);
        Self::build_zero_mean(u, y, noise_var, o.kernel, &l, sf2)
            .map_err(|(i, p)| format!("gp: covariance not positive definite (row {i}, pivot {p:e})"))
    }

    /// Gradient of the zero-mean log likelihood in `(ln l_1, .., ln l_d, ln sf2)` (ADR 0663 D1; ADR 0652 D2):
    /// `dL/dt = (alpha^T dC alpha - tr(C^-1 dC)) / 2`.
    fn zero_mean_gradient(&self) -> LikEval {
        let n = self.u.len();
        let d = self.l.len();
        let (u, l, tau, alpha) = (&self.u, &self.l, self.s2, &self.alpha);
        let g = chol_inverse(&self.chol, n);
        let mut out = vec![0.0; d + 1];
        // sf2: dC = sf2 (R + J I); l_k: dC_ij = sf2 (dk_ij / d ln l_k) off the diagonal
        let mut quad_t = 0.0;
        let mut tr_t = 0.0;
        for i in 0..n {
            let dii = tau * (1.0 + self.jitter);
            quad_t += alpha[i] * alpha[i] * dii;
            tr_t += g[i * n + i] * dii;
            for j in 0..i {
                let dij = tau * correlation(self.kernel, &u[i], &u[j], l);
                quad_t += 2.0 * alpha[i] * alpha[j] * dij;
                tr_t += 2.0 * g[i * n + j] * dij;
                let dl = match self.kernel {
                    Kernel::SquaredExponential => dij,
                    Kernel::Matern52 => tau * dcorr_common(self.kernel, &u[i], &u[j], l),
                };
                for k in 0..d {
                    let t = dl * ((u[i][k] - u[j][k]) / l[k]).powi(2);
                    out[k] += 2.0 * (alpha[i] * alpha[j] - g[i * n + j]) * t;
                }
            }
        }
        out[d] = quad_t - tr_t;
        for v in &mut out {
            *v *= 0.5;
        }
        LikEval {
            value: self.loglik,
            grad: out,
        }
    }

    /// Fit the length scales (and the nugget of `Noise::Fitted`) by maximum likelihood (ADR 0621 D4).
    pub fn fit(u: &[Vec<f64>], y: &[f64], o: &GpOptions) -> Result<Gp, String> {
        let d = check_inputs(u, y)?;
        if u.len() < d + 2 {
            return Err(format!(
                "gp: {} observations, at least d + 2 = {} needed",
                u.len(),
                d + 2
            ));
        }
        if o.starts == 0 || o.refine == 0 {
            return Err("gp: starts and refine must be >= 1".into());
        }
        let (llo, lhi) = o.l_bounds;
        if !(llo > 0.0 && lhi > llo && lhi.is_finite()) {
            return Err(format!("gp: length-scale bounds ({llo}, {lhi}) need 0 < lo < hi"));
        }
        let fitted_g = o.noise == Noise::Fitted;
        let fixed_g = match o.noise {
            Noise::None => 0.0,
            Noise::Nugget(g) => {
                if !(g.is_finite() && g >= 0.0) {
                    return Err(format!("gp: nugget ratio {g} must be finite and >= 0"));
                }
                g
            }
            Noise::Fitted => {
                let (a, b) = o.g_bounds;
                if !(a > 0.0 && b > a && b.is_finite()) {
                    return Err(format!("gp: nugget bounds ({a}, {b}) need 0 < lo < hi"));
                }
                0.0
            }
        };
        let (y_mean, y_sd, ys) = standardise(y);
        let dim = d + usize::from(fitted_g);
        let mut bx: Vec<(f64, f64)> = vec![(llo.ln(), lhi.ln()); d];
        if fitted_g {
            bx.push((o.g_bounds.0.ln(), o.g_bounds.1.ln()));
        }
        let hyper = |t: &[f64]| -> (Vec<f64>, f64) {
            let l: Vec<f64> = t[..d].iter().map(|v| v.exp()).collect();
            let g = if fitted_g { t[d].exp() } else { fixed_g };
            (l, g)
        };
        let value = |t: &[f64]| -> f64 {
            let (l, g) = hyper(t);
            Self::build(u, &ys, o.kernel, &l, g, y_mean, y_sd).map_or(f64::NEG_INFINITY, |gp| gp.loglik)
        };
        let mut starts: Vec<Vec<f64>> = vec![bx.iter().map(|(a, b)| 0.5 * (a + b)).collect()];
        for p in latin_hypercube(o.starts - 1, dim, o.seed) {
            starts.push(p.iter().zip(&bx).map(|(v, (a, b))| a + (b - a) * v).collect());
        }
        let vals = parallel_map(&starts, o.workers, &|t: &Vec<f64>| value(t));
        let mut order: Vec<usize> = (0..starts.len()).collect();
        order.sort_by(|&a, &b| vals[b].total_cmp(&vals[a]).then(a.cmp(&b)));
        let chosen: Vec<usize> = order.into_iter().take(o.refine).collect();
        let refine = |&s: &usize| -> (Vec<f64>, f64) {
            let opts = PgOptions {
                max_iters: o.iters,
                step: 0.1,
                armijo: 1e-4,
                max_halvings: 30,
                x0: starts[s].clone(),
            };
            let mut ev = |t: &[f64], grad: bool| -> Result<(f64, Vec<f64>), Stop> {
                let (l, g) = hyper(t);
                match Self::build(u, &ys, o.kernel, &l, g, y_mean, y_sd) {
                    Err(_) => Ok((f64::INFINITY, vec![0.0; dim])),
                    Ok(gp) => {
                        if grad {
                            let le = gp.lik_gradient(fitted_g);
                            Ok((-le.value, le.grad.iter().map(|v| -v).collect()))
                        } else {
                            Ok((-gp.loglik, Vec::new()))
                        }
                    }
                }
            };
            match projected_gradient(&bx, &opts, &mut ev) {
                Ok(r) if r.fitness.f.is_finite() => (r.x, -r.fitness.f),
                _ => (starts[s].clone(), vals[s]),
            }
        };
        let refined = parallel_map(&chosen, o.workers, &refine);
        let mut best: Option<(Vec<f64>, f64)> = None;
        for (t, v) in refined {
            if best.as_ref().is_none_or(|b| v > b.1) {
                best = Some((t, v));
            }
        }
        let (t, v) = best.expect("refine >= 1");
        if !v.is_finite() {
            return Err("gp: no start of the likelihood search gave a positive definite correlation matrix".into());
        }
        let (l, g) = hyper(&t);
        Self::build(u, &ys, o.kernel, &l, g, y_mean, y_sd)
    }

    /// Value and gradient (in `ln l_i`, and `ln g` when `with_g`) of the concentrated log likelihood:
    /// `dL/dt = (a^T dK a / s2 - tr(K^-1 dK)) / 2`, `a = K^-1 (y - mu 1)` (mu and s2 at their profile values).
    fn lik_gradient(&self, with_g: bool) -> LikEval {
        if self.zero_mean {
            return self.zero_mean_gradient();
        }
        let n = self.u.len();
        let d = self.l.len();
        // K^-1 from the factor: column by column
        let mut kinv = vec![0.0; n * n];
        for c in 0..n {
            let mut e = vec![0.0; n];
            e[c] = 1.0;
            let col = solve_upper_t(&self.chol, n, &solve_lower(&self.chol, n, &e));
            for r in 0..n {
                kinv[r * n + c] = col[r];
            }
        }
        let a = &self.alpha;
        let s2 = self.s2.max(f64::MIN_POSITIVE);
        let mut quad = vec![0.0; d];
        let mut trace = vec![0.0; d];
        for i in 0..n {
            for j in 0..i {
                let c = dcorr_common(self.kernel, &self.u[i], &self.u[j], &self.l);
                if c == 0.0 {
                    continue;
                }
                let w_quad = 2.0 * a[i] * a[j];
                let w_tr = 2.0 * kinv[i * n + j];
                for k in 0..d {
                    let t = ((self.u[i][k] - self.u[j][k]) / self.l[k]).powi(2) * c;
                    quad[k] += w_quad * t;
                    trace[k] += w_tr * t;
                }
            }
        }
        let mut grad: Vec<f64> = (0..d).map(|k| 0.5 * (quad[k] / s2 - trace[k])).collect();
        if with_g {
            let q: f64 = a.iter().map(|v| v * v).sum::<f64>() * self.g;
            let tr: f64 = (0..n).map(|i| kinv[i * n + i]).sum::<f64>() * self.g;
            grad.push(0.5 * (q / s2 - tr));
        }
        LikEval {
            value: self.loglik,
            grad,
        }
    }

    /// Value and gradient of the concentrated log likelihood (check G4); in the zero-mean mode the zero-mean log likelihood
    /// and its gradient in `(ln l, ln sf2)`, `with_g` ignored (ADR 0663).
    pub fn log_likelihood_and_gradient(&self, with_g: bool) -> (f64, Vec<f64>) {
        let e = self.lik_gradient(with_g);
        (e.value, e.grad)
    }

    /// Concentrated log likelihood `-(n/2) ln s2 - sum ln L_ii` of the standardised outputs.
    pub fn log_likelihood(&self) -> f64 {
        self.loglik
    }

    /// Mean and standard deviation of the latent function at `u` (output units). The variance is
    /// `s2 [1 - k^T K^-1 k + (1 - 1^T K^-1 k)^2 / (1^T K^-1 1)]`, floored at 0 (the last term is the uncertainty of the mean).
    ///
    /// In the zero-mean mode (ADR 0663 D1): mean `k^T alpha`, `k_i = sf2 R(u_i, u)`, variance `sf2 - |L^-1 k|^2`, floored at 0.
    pub fn predict(&self, u: &[f64]) -> (f64, f64) {
        let n = self.u.len();
        if self.zero_mean {
            let k: Vec<f64> = self
                .u
                .iter()
                .map(|p| self.s2 * correlation(self.kernel, p, u, &self.l))
                .collect();
            let m: f64 = k.iter().zip(&self.alpha).map(|(a, b)| a * b).sum();
            let v = solve_lower(&self.chol, n, &k);
            let var = (self.s2 - v.iter().map(|x| x * x).sum::<f64>()).max(0.0);
            return (m, var.sqrt());
        }
        let k: Vec<f64> = self.u.iter().map(|p| correlation(self.kernel, p, u, &self.l)).collect();
        let m = self.mu + k.iter().zip(&self.alpha).map(|(a, b)| a * b).sum::<f64>();
        let v = solve_lower(&self.chol, n, &k);
        let vv: f64 = v.iter().map(|x| x * x).sum();
        let b = 1.0 - k.iter().zip(&self.kinv1).map(|(a, b)| a * b).sum::<f64>();
        let var = (self.s2 * (1.0 - vv + b * b / self.one_kinv_one)).max(0.0);
        (self.y_mean + self.y_sd * m, self.y_sd * var.sqrt())
    }

    /// Leave-one-out mean and standard deviation at every observation from the one factorisation, hyperparameters and `s2`
    /// held, the mean re-estimated (Dubrule 1983): with `Q = K^-1 - K^-1 1 1^T K^-1 / (1^T K^-1 1)`, the residual is
    /// `(Q y)_i / Q_ii` and the variance `s2 / Q_ii`, the variance of the observation `y_i` (latent plus `s2 (g + jitter)`).
    ///
    /// In the zero-mean mode (ADR 0663 D1): mean `y_i - alpha_i / G_ii`, variance `1 / G_ii`, `G_ii = |L^-1 e_i|^2`.
    pub fn loo(&self) -> Vec<(f64, f64)> {
        let n = self.u.len();
        let mut out = Vec::with_capacity(n);
        if self.zero_mean {
            for i in 0..n {
                let mut e = vec![0.0; n];
                e[i] = 1.0;
                let gii: f64 = solve_lower(&self.chol, n, &e).iter().map(|x| x * x).sum();
                out.push((self.ys[i] - self.alpha[i] / gii, (1.0 / gii).max(0.0).sqrt()));
            }
            return out;
        }
        for i in 0..n {
            let mut e = vec![0.0; n];
            e[i] = 1.0;
            let col = solve_upper_t(&self.chol, n, &solve_lower(&self.chol, n, &e));
            let qii = col[i] - self.kinv1[i] * self.kinv1[i] / self.one_kinv_one;
            // (Q y)_i = alpha_i, since alpha = K^-1 (y - mu 1) and Q 1 = 0
            let m = self.ys[i] - self.alpha[i] / qii;
            let var = (self.s2 / qii).max(0.0);
            out.push((self.y_mean + self.y_sd * m, self.y_sd * var.sqrt()));
        }
        out
    }

    /// Length scales.
    pub fn length_scales(&self) -> &[f64] {
        &self.l
    }

    /// Nugget ratio.
    pub fn nugget(&self) -> f64 {
        self.g
    }

    /// Jitter used.
    pub fn jitter(&self) -> f64 {
        self.jitter
    }

    /// Process variance in output units.
    pub fn process_variance(&self) -> f64 {
        self.s2 * self.y_sd * self.y_sd
    }

    /// Estimated constant mean in output units.
    pub fn mean(&self) -> f64 {
        self.y_mean + self.y_sd * self.mu
    }

    /// Number of observations.
    pub fn n(&self) -> usize {
        self.u.len()
    }

    /// Kernel.
    pub fn kernel(&self) -> Kernel {
        self.kernel
    }

    /// Whether the process is in the zero-mean mode (ADR 0663).
    pub fn is_zero_mean(&self) -> bool {
        self.zero_mean
    }

    /// Absolute noise variances of the zero-mean mode (empty otherwise).
    pub fn noise_variances(&self) -> &[f64] {
        &self.noise
    }

    /// Row-major lower Cholesky factor of the covariance (the zero-mean mode) or of the correlation matrix plus nugget.
    pub fn cholesky_factor(&self) -> &[f64] {
        &self.chol
    }

    /// `alpha`: `C^-1 y` in the zero-mean mode; `K^-1 (y - mu 1)` of the standardised outputs otherwise.
    pub fn weights(&self) -> &[f64] {
        &self.alpha
    }
}

fn corr_matrix(u: &[Vec<f64>], kernel: Kernel, l: &[f64]) -> Vec<f64> {
    let n = u.len();
    let mut r = vec![0.0; n * n];
    for i in 0..n {
        r[i * n + i] = 1.0;
        for j in 0..i {
            let c = correlation(kernel, &u[i], &u[j], l);
            r[i * n + j] = c;
            r[j * n + i] = c;
        }
    }
    r
}
