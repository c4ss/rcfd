//! Tolerance / uncertainty study driver (ADR 0121, part F1 of `docs/fan-project-breakdown.md`) over any [`Model`].
//!
//! * Inputs: named parameters with units and distributions (uniform, normal truncated to `[lo, hi]` (infinite bounds allowed),
//!   beta on `[lo, hi]`), sampled by inverse CDF from plain Monte Carlo or from replicated Latin hypercubes, seeded (bit-identical
//!   for a seed, whatever the worker count).
//! * Stopping rule: batches until the standard error of the mean of every controlled metric is at most
//!   `max(se_abs, se_rel |mean|)` (and at least `min_batches` batches), or `max_samples` is reached (then `converged = false`).
//!   Monte Carlo: `SE = s / sqrt(n)`. Latin hypercube: one LHS of `batch` points per replicate, `SE = sd(replicate means) / sqrt(r)`
//!   (an LHS cannot be extended and stay stratified).
//! * Per metric: mean, standard deviation, standard error, quantiles `X_(ceil(n p))` with the distribution-free order-statistic
//!   interval at the stated confidence (exact coverage `>= confidence` for independent samples, i.e. Monte Carlo; for Latin
//!   hypercube samples it is the i.i.d. interval, approximate), and standardised regression coefficients (SRC) with R^2.
//! * SRC limits: a linear fit; it ranks by linear effect only, misses interactions and non-monotone effects (R^2 well below 1
//!   says so), and assumes independent inputs (all inputs here are independent).
//! * Requirements: probability of violation with the Clopper-Pearson interval. A failed sample (verdict not `ok`) counts as a
//!   violation of every requirement (conservative); statistics of the metrics are over the `ok` samples only.
//! * Every sample is a ledger entry (`kind = evaluation`), and one `uq_summary` entry closes the study.

use super::ledger::{entry_of, Entry, Ledger, Model, Outcome, Quantity, Verdict};
use super::rng::Rng;
use super::sampling;
use super::study::{parallel_map, Cmp};
use rcfd_reactor::mixfrac::{erfc, erfc_inv, reg_inc_beta};
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Standard normal CDF.
pub fn norm_cdf(x: f64) -> f64 {
    0.5 * erfc(-x / std::f64::consts::SQRT_2)
}

/// Standard normal quantile for `0 < p < 1`.
pub fn norm_inv(p: f64) -> f64 {
    -std::f64::consts::SQRT_2 * erfc_inv(2.0 * p)
}

/// Inverse of the regularised incomplete beta function in `x` (bisection, 200 halvings of [0, 1]).
pub fn beta_inv(p: f64, a: f64, b: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if reg_inc_beta(mid, a, b) < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Binomial CDF `P(B <= k)` for `B ~ Bin(n, p)` (`k < 0` gives 0, `k >= n` gives 1).
pub fn binom_cdf(k: i64, n: usize, p: f64) -> f64 {
    if k < 0 {
        0.0
    } else if k as usize >= n {
        1.0
    } else {
        reg_inc_beta(1.0 - p, (n as i64 - k) as f64, (k + 1) as f64)
    }
}

/// Clamp into the open unit interval so that every inverse CDF is finite.
fn open_unit(u: f64) -> f64 {
    u.clamp(f64::MIN_POSITIVE, 1.0 - f64::EPSILON / 2.0)
}

/// Input distribution.
#[derive(Debug, Clone, PartialEq)]
pub enum Dist {
    /// Uniform on `[lo, hi]`.
    Uniform {
        /// Lower end.
        lo: f64,
        /// Upper end.
        hi: f64,
    },
    /// Normal truncated to `[lo, hi]` (`-inf`/`inf` for no truncation).
    Normal {
        /// Mean of the parent normal.
        mean: f64,
        /// Standard deviation of the parent normal.
        sd: f64,
        /// Lower truncation.
        lo: f64,
        /// Upper truncation.
        hi: f64,
    },
    /// Beta(a, b) mapped to `[lo, hi]`.
    Beta {
        /// Shape a.
        a: f64,
        /// Shape b.
        b: f64,
        /// Lower end.
        lo: f64,
        /// Upper end.
        hi: f64,
    },
}

impl Dist {
    /// Check the parameters.
    pub fn validate(&self) -> Result<(), String> {
        match *self {
            Dist::Uniform { lo, hi } | Dist::Beta { lo, hi, .. } if !(lo.is_finite() && hi.is_finite() && lo < hi) => {
                Err(format!("bounds must be finite with lo < hi (got {lo}, {hi})"))
            }
            Dist::Beta { a, b, .. } if !(a.is_finite() && b.is_finite() && a > 0.0 && b > 0.0) => {
                Err(format!("beta shapes must be finite and > 0 (got {a}, {b})"))
            }
            Dist::Normal { mean, sd, lo, hi } => {
                if !(mean.is_finite() && sd.is_finite() && sd > 0.0) {
                    return Err(format!("normal needs a finite mean and sd > 0 (got {mean}, {sd})"));
                }
                if lo.is_nan() || hi.is_nan() || lo >= hi {
                    return Err(format!("truncation needs lo < hi (got {lo}, {hi})"));
                }
                let mass = norm_cdf((hi - mean) / sd) - norm_cdf((lo - mean) / sd);
                if mass < 1e-12 {
                    return Err(format!(
                        "truncation [{lo}, {hi}] keeps a probability of {mass:e}; the inverse CDF is not accurate there"
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Inverse CDF at `u` in (0, 1). Truncated normal: `mean + sd norm_inv(Phi(a) + u (Phi(b) - Phi(a)))`, which loses
    /// relative accuracy when both bounds lie far out in the same tail (refused below a kept mass of 1e-12).
    pub fn inv_cdf(&self, u: f64) -> f64 {
        let u = open_unit(u);
        match *self {
            Dist::Uniform { lo, hi } => lo + (hi - lo) * u,
            Dist::Normal { mean, sd, lo, hi } => {
                if lo == f64::NEG_INFINITY && hi == f64::INFINITY {
                    return mean + sd * norm_inv(u);
                }
                let (fa, fb) = (norm_cdf((lo - mean) / sd), norm_cdf((hi - mean) / sd));
                let x = mean + sd * norm_inv(open_unit(fa + u * (fb - fa)));
                x.clamp(lo, hi)
            }
            Dist::Beta { a, b, lo, hi } => lo + (hi - lo) * beta_inv(u, a, b),
        }
    }

    fn to_json(&self) -> Value {
        let n = |x: f64| if x.is_finite() { json!(x) } else { json!(x.to_string()) };
        match *self {
            Dist::Uniform { lo, hi } => json!({"type": "uniform", "lo": lo, "hi": hi}),
            Dist::Normal { mean, sd, lo, hi } => {
                json!({"type": "normal", "mean": mean, "sd": sd, "lo": n(lo), "hi": n(hi)})
            }
            Dist::Beta { a, b, lo, hi } => json!({"type": "beta", "a": a, "b": b, "lo": lo, "hi": hi}),
        }
    }
}

/// An uncertain input.
#[derive(Debug, Clone, PartialEq)]
pub struct UncertainParam {
    /// Name.
    pub name: String,
    /// Unit.
    pub unit: String,
    /// Distribution.
    pub dist: Dist,
}

/// Sampling scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sampler {
    /// Independent uniform draws.
    MonteCarlo,
    /// Independent Latin hypercubes of `batch` points (replicated LHS).
    Lhs,
}

/// A requirement on a metric; a sample violates it when the comparison fails.
#[derive(Debug, Clone, PartialEq)]
pub struct Requirement {
    /// Metric name.
    pub metric: String,
    /// `Le`: metric <= limit is required; `Ge`: metric >= limit.
    pub cmp: Cmp,
    /// Limit.
    pub limit: f64,
}

/// Stopping rule on the standard error of the mean.
#[derive(Debug, Clone, PartialEq)]
pub struct StopRule {
    /// Samples per batch (Latin hypercube: points per replicate).
    pub batch: usize,
    /// Batches before the rule may stop (>= 2 for Latin hypercube).
    pub min_batches: usize,
    /// Hard cap on samples.
    pub max_samples: usize,
    /// Absolute SE target.
    pub se_abs: f64,
    /// Relative SE target (times |mean|).
    pub se_rel: f64,
    /// Metrics the rule controls (empty: all).
    pub metrics: Vec<String>,
}

/// Definition of a tolerance study.
#[derive(Debug, Clone, PartialEq)]
pub struct UqSpec {
    /// Study name (the `study` of the ledger entries).
    pub name: String,
    /// Uncertain inputs.
    pub params: Vec<UncertainParam>,
    /// Sampler.
    pub sampler: Sampler,
    /// Seed.
    pub seed: u64,
    /// Stopping rule.
    pub stop: StopRule,
    /// Quantile probabilities.
    pub quantiles: Vec<f64>,
    /// Confidence of the quantile and probability intervals.
    pub confidence: f64,
    /// Requirements.
    pub requirements: Vec<Requirement>,
    /// Concurrent evaluations.
    pub workers: usize,
}

impl UqSpec {
    /// Check the definition; located messages.
    pub fn validate(&self) -> Result<(), String> {
        if self.params.is_empty() {
            return Err("params: no uncertain parameter".into());
        }
        for (i, p) in self.params.iter().enumerate() {
            if p.name.is_empty() || p.unit.is_empty() {
                return Err(format!("params[{i}]: name and unit are required"));
            }
            if self.params[..i].iter().any(|q| q.name == p.name) {
                return Err(format!("parameter '{}': defined twice", p.name));
            }
            p.dist.validate().map_err(|e| format!("parameter '{}': {e}", p.name))?;
        }
        let s = &self.stop;
        if s.batch == 0 || s.max_samples < s.batch {
            return Err("stop: batch >= 1 and max_samples >= batch are required".into());
        }
        if self.sampler == Sampler::Lhs && s.min_batches < 2 {
            return Err("stop.min_batches: a Latin hypercube SE needs at least 2 replicates".into());
        }
        if !(s.se_abs >= 0.0 && s.se_rel >= 0.0 && s.se_abs.is_finite() && s.se_rel.is_finite()) {
            return Err("stop: se_abs and se_rel must be finite and >= 0".into());
        }
        if self.quantiles.iter().any(|p| !(*p > 0.0 && *p < 1.0)) {
            return Err("quantiles: probabilities must lie in (0, 1)".into());
        }
        if !(self.confidence > 0.0 && self.confidence < 1.0) {
            return Err("confidence: must lie in (0, 1)".into());
        }
        if self.requirements.iter().any(|r| !r.limit.is_finite()) {
            return Err("requirements: limits must be finite".into());
        }
        if self.workers == 0 {
            return Err("workers: must be >= 1".into());
        }
        Ok(())
    }
}

/// Quantile estimate with its order-statistic interval.
#[derive(Debug, Clone, PartialEq)]
pub struct QuantileEst {
    /// Probability.
    pub p: f64,
    /// Estimate `X_(ceil(n p))`.
    pub value: f64,
    /// Lower end (`-inf` when n is too small for the confidence).
    pub lo: f64,
    /// Upper end (`+inf` when n is too small).
    pub hi: f64,
    /// Exact binomial coverage of `[lo, hi]` for independent samples.
    pub coverage: f64,
}

/// Statistics of one metric.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricStats {
    /// Metric name.
    pub name: String,
    /// Unit.
    pub unit: String,
    /// Usable samples.
    pub n: usize,
    /// Mean.
    pub mean: f64,
    /// Sample standard deviation (n - 1).
    pub sd: f64,
    /// Standard error of the mean (see the module notes).
    pub se: f64,
    /// Quantiles.
    pub quantiles: Vec<QuantileEst>,
    /// SRC per parameter (in parameter order).
    pub src: Vec<f64>,
    /// Raw regression coefficients per parameter (metric unit per parameter unit).
    pub coef: Vec<f64>,
    /// R^2 of the linear fit.
    pub r2: f64,
    /// Parameter names ranked by |SRC|, largest first.
    pub ranking: Vec<String>,
}

/// Probability of violating one requirement.
#[derive(Debug, Clone, PartialEq)]
pub struct ReqStats {
    /// The requirement.
    pub req: Requirement,
    /// Violations among `ok` samples.
    pub violations: usize,
    /// Failed samples (counted as violations).
    pub failed: usize,
    /// All samples.
    pub n: usize,
    /// (violations + failed) / n.
    pub p: f64,
    /// Clopper-Pearson interval at the stated confidence.
    pub lo: f64,
    /// Upper end.
    pub hi: f64,
}

/// Result of a tolerance study.
#[derive(Debug, Clone, PartialEq)]
pub struct UqResult {
    /// Samples evaluated.
    pub n_samples: usize,
    /// Usable (`ok`) samples.
    pub n_ok: usize,
    /// Batches run.
    pub batches: usize,
    /// Whether the stopping rule was met (false: `max_samples` reached first).
    pub converged: bool,
    /// Statistics per metric (in the order of the first usable sample).
    pub metrics: Vec<MetricStats>,
    /// Requirements.
    pub requirements: Vec<ReqStats>,
    /// Ledger index of the first sample entry and of the summary entry (when a ledger was given).
    pub ledger_range: Option<(usize, usize)>,
}

fn mean_sd(y: &[f64]) -> (f64, f64) {
    let n = y.len() as f64;
    let m = y.iter().sum::<f64>() / n;
    let v = if y.len() > 1 {
        y.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (n - 1.0)
    } else {
        f64::NAN
    };
    (m, v.sqrt())
}

/// Order-statistic interval for the p-quantile from `n` sorted values: returns (lo, hi, coverage).
pub fn quantile_interval(sorted: &[f64], p: f64, confidence: f64) -> (f64, f64, f64) {
    let n = sorted.len();
    let a2 = 0.5 * (1.0 - confidence);
    let f = |k: i64| binom_cdf(k, n, p);
    // l = largest rank in 1..=n with F(l - 1) <= a2 (0 when none); u = smallest rank with F(u - 1) >= 1 - a2 (n + 1 when none)
    let (mut l, mut u) = (0usize, n + 1);
    let (mut a, mut b) = (1usize, n);
    while a <= b {
        let m = (a + b) / 2;
        if f(m as i64 - 1) <= a2 {
            l = m;
            a = m + 1;
        } else if m == 0 {
            break;
        } else {
            b = m - 1;
        }
    }
    let (mut a, mut b) = (1usize, n);
    while a <= b {
        let m = (a + b) / 2;
        if f(m as i64 - 1) >= 1.0 - a2 {
            u = m;
            if m == 1 {
                break;
            }
            b = m - 1;
        } else {
            a = m + 1;
        }
    }
    let coverage = f(u as i64 - 1) - f(l as i64 - 1);
    let lo = if l == 0 { f64::NEG_INFINITY } else { sorted[l - 1] };
    let hi = if u > n { f64::INFINITY } else { sorted[u - 1] };
    (lo, hi, coverage)
}

/// Clopper-Pearson interval for `k` successes in `n` trials.
pub fn clopper_pearson(k: usize, n: usize, confidence: f64) -> (f64, f64) {
    let a2 = 0.5 * (1.0 - confidence);
    let lo = if k == 0 {
        0.0
    } else {
        beta_inv(a2, k as f64, (n - k + 1) as f64)
    };
    let hi = if k == n {
        1.0
    } else {
        beta_inv(1.0 - a2, (k + 1) as f64, (n - k) as f64)
    };
    (lo, hi)
}

/// Least-squares coefficients of `y` on the columns of `x` (with an intercept): (coefficients, R^2).
pub fn linear_fit(x: &[Vec<f64>], y: &[f64]) -> Option<(Vec<f64>, f64)> {
    let n = y.len();
    let d = x.first().map_or(0, Vec::len);
    if n <= d + 1 {
        return None;
    }
    let mx: Vec<f64> = (0..d).map(|j| x.iter().map(|r| r[j]).sum::<f64>() / n as f64).collect();
    let my = y.iter().sum::<f64>() / n as f64;
    // normal equations of the centred data, augmented with the right-hand side
    let mut a = vec![vec![0.0; d + 1]; d];
    for (r, &yy) in x.iter().zip(y) {
        for i in 0..d {
            let ci = r[i] - mx[i];
            for j in 0..d {
                a[i][j] += ci * (r[j] - mx[j]);
            }
            a[i][d] += ci * (yy - my);
        }
    }
    for c in 0..d {
        let p = (c..d).max_by(|&i, &j| a[i][c].abs().total_cmp(&a[j][c].abs()))?;
        if a[p][c].abs() < 1e-300 {
            return None;
        }
        a.swap(c, p);
        for r in 0..d {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..=d {
                    a[r][k] -= f * a[c][k];
                }
            }
        }
    }
    let b: Vec<f64> = (0..d).map(|i| a[i][d] / a[i][i]).collect();
    let (mut ss_res, mut ss_tot) = (0.0, 0.0);
    for (r, &yy) in x.iter().zip(y) {
        let fit = my + (0..d).map(|j| b[j] * (r[j] - mx[j])).sum::<f64>();
        ss_res += (yy - fit) * (yy - fit);
        ss_tot += (yy - my) * (yy - my);
    }
    Some((b, if ss_tot > 0.0 { 1.0 - ss_res / ss_tot } else { f64::NAN }))
}

fn evaluate_guarded(model: &dyn Model, params: &[Quantity]) -> Outcome {
    let mut out = match catch_unwind(AssertUnwindSafe(|| model.evaluate(params))) {
        Ok(o) => o,
        Err(p) => Outcome {
            verdict: Verdict::Diverged,
            reason: format!(
                "internal error: the evaluator panicked: {}",
                p.downcast_ref::<String>()
                    .cloned()
                    .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_default()
            ),
            ..Default::default()
        },
    };
    if out.verdict == Verdict::Ok {
        if let Some(q) = out.metrics.iter().find(|q| !q.value.is_finite()) {
            out.reason = format!("metric '{}' is {}", q.name, q.value);
            out.verdict = Verdict::Diverged;
        }
    } else if out.reason.trim().is_empty() {
        out.reason = format!("the evaluator returned '{}' without a reason", out.verdict.as_str());
    }
    out
}

/// Run a tolerance study of `model` and record every sample (and a summary) in `ledger` when given.
pub fn run(spec: &UqSpec, model: &dyn Model, ledger: Option<&Ledger>) -> Result<UqResult, String> {
    spec.validate()?;
    let d = spec.params.len();
    let first = match ledger {
        Some(l) => Some(l.read()?.entries.len()),
        None => None,
    };
    let mut rng = Rng::new(spec.seed);
    let mut xs: Vec<Vec<f64>> = Vec::new();
    let mut outs: Vec<Outcome> = Vec::new();
    let mut batch_of: Vec<usize> = Vec::new();
    let mut names: Option<Vec<(String, String)>> = None;
    let s = &spec.stop;
    let mut batches = 0;
    let mut converged = false;
    while xs.len() + s.batch <= s.max_samples {
        let unit: Vec<Vec<f64>> = match spec.sampler {
            Sampler::MonteCarlo => (0..s.batch)
                .map(|_| {
                    (0..d)
                        .map(|_| ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64)
                        .collect()
                })
                .collect(),
            Sampler::Lhs => sampling::latin_hypercube(s.batch, d, rng.next_u64()),
        };
        let bx: Vec<Vec<f64>> = unit
            .iter()
            .map(|u| u.iter().zip(&spec.params).map(|(&u, p)| p.dist.inv_cdf(u)).collect())
            .collect();
        let named = |x: &Vec<f64>| -> Vec<Quantity> {
            spec.params
                .iter()
                .zip(x)
                .map(|(p, &v)| Quantity::new(&p.name, v, &p.unit))
                .collect()
        };
        let bout = parallel_map(&bx, spec.workers, &|x: &Vec<f64>| evaluate_guarded(model, &named(x)));
        for (k, o) in bout.iter().enumerate() {
            if o.verdict != Verdict::Ok {
                continue;
            }
            let these: Vec<(String, String)> = o.metrics.iter().map(|q| (q.name.clone(), q.unit.clone())).collect();
            match &names {
                None => names = Some(these),
                Some(n) if *n != these => {
                    return Err(format!(
                        "sample {}: the evaluator returned metrics {these:?}, earlier samples {n:?}",
                        xs.len() + k
                    ))
                }
                _ => {}
            }
        }
        if let Some(l) = ledger {
            let entries: Vec<Entry> = bx
                .iter()
                .zip(&bout)
                .enumerate()
                .map(|(k, (x, o))| Entry {
                    study: spec.name.clone(),
                    variant: format!("{}#{}", spec.name, xs.len() + k),
                    seed: Some(spec.seed),
                    provenance: json!({"driver": "uq", "sampler": sampler_name(spec.sampler), "sample": xs.len() + k, "batch": batches}),
                    ..entry_of(model, named(x), o.clone())
                })
                .collect();
            l.append(&entries)?;
        }
        batch_of.extend(std::iter::repeat(batches).take(s.batch));
        xs.extend(bx);
        outs.extend(bout);
        batches += 1;
        if batches >= s.min_batches.max(1) {
            if let Some(n) = &names {
                let ses = metric_ses(spec, n, &outs, &batch_of, batches);
                converged = ses.iter().all(|&(name, mean, se)| {
                    !(s.metrics.is_empty() || s.metrics.iter().any(|m| m == name))
                        || se <= s.se_abs.max(s.se_rel * mean.abs())
                });
                if converged {
                    break;
                }
            }
        }
    }
    let names = names.unwrap_or_default();
    let ok: Vec<usize> = (0..outs.len()).filter(|&i| outs[i].verdict == Verdict::Ok).collect();
    let ses = metric_ses(spec, &names, &outs, &batch_of, batches);
    let mut metrics = Vec::new();
    for (k, (name, unit)) in names.iter().enumerate() {
        let y: Vec<f64> = ok.iter().map(|&i| outs[i].metrics[k].value).collect();
        let (mean, sd) = mean_sd(&y);
        let mut sorted = y.clone();
        sorted.sort_by(f64::total_cmp);
        let quantiles = spec
            .quantiles
            .iter()
            .map(|&p| {
                let r = ((sorted.len() as f64 * p).ceil() as usize).clamp(1, sorted.len());
                let (lo, hi, coverage) = quantile_interval(&sorted, p, spec.confidence);
                QuantileEst {
                    p,
                    value: sorted[r - 1],
                    lo,
                    hi,
                    coverage,
                }
            })
            .collect();
        let xo: Vec<Vec<f64>> = ok.iter().map(|&i| xs[i].clone()).collect();
        let (coef, r2) = linear_fit(&xo, &y).unwrap_or((vec![f64::NAN; d], f64::NAN));
        let src: Vec<f64> = (0..d)
            .map(|j| {
                let col: Vec<f64> = xo.iter().map(|r| r[j]).collect();
                coef[j] * mean_sd(&col).1 / sd
            })
            .collect();
        let mut order: Vec<usize> = (0..d).collect();
        order.sort_by(|&a, &b| src[b].abs().total_cmp(&src[a].abs()));
        metrics.push(MetricStats {
            name: name.clone(),
            unit: unit.clone(),
            n: y.len(),
            mean,
            sd,
            se: ses[k].2,
            quantiles,
            src,
            coef,
            r2,
            ranking: order.iter().map(|&j| spec.params[j].name.clone()).collect(),
        });
    }
    let failed = outs.len() - ok.len();
    let requirements = spec
        .requirements
        .iter()
        .map(|r| {
            let k = names.iter().position(|(n, _)| *n == r.metric);
            let violations = ok
                .iter()
                .filter(|&&i| {
                    let v = k.map_or(f64::NAN, |k| outs[i].metrics[k].value);
                    !match r.cmp {
                        Cmp::Le => v <= r.limit,
                        Cmp::Ge => v >= r.limit,
                    }
                })
                .count();
            let n = outs.len();
            let (lo, hi) = clopper_pearson(violations + failed, n, spec.confidence);
            ReqStats {
                req: r.clone(),
                violations,
                failed,
                n,
                p: (violations + failed) as f64 / n as f64,
                lo,
                hi,
            }
        })
        .collect();
    let mut res = UqResult {
        n_samples: outs.len(),
        n_ok: ok.len(),
        batches,
        converged,
        metrics,
        requirements,
        ledger_range: None,
    };
    if let (Some(l), Some(first)) = (ledger, first) {
        let summary_index = first + res.n_samples;
        res.ledger_range = Some((first, summary_index));
        l.append(&[summary_entry(spec, model, &res)])?;
    }
    Ok(res)
}

fn sampler_name(s: Sampler) -> &'static str {
    match s {
        Sampler::MonteCarlo => "monte_carlo",
        Sampler::Lhs => "lhs",
    }
}

/// (name, mean, SE) of every metric over the `ok` samples.
fn metric_ses<'a>(
    spec: &UqSpec,
    names: &'a [(String, String)],
    outs: &[Outcome],
    batch_of: &[usize],
    batches: usize,
) -> Vec<(&'a str, f64, f64)> {
    names
        .iter()
        .enumerate()
        .map(|(k, (name, _))| {
            let y: Vec<f64> = outs
                .iter()
                .filter(|o| o.verdict == Verdict::Ok)
                .map(|o| o.metrics[k].value)
                .collect();
            let (mean, sd) = mean_sd(&y);
            let se = match spec.sampler {
                Sampler::MonteCarlo => sd / (y.len() as f64).sqrt(),
                Sampler::Lhs => {
                    let mut sum = vec![(0.0, 0usize); batches];
                    for (o, &b) in outs.iter().zip(batch_of) {
                        if o.verdict == Verdict::Ok {
                            sum[b].0 += o.metrics[k].value;
                            sum[b].1 += 1;
                        }
                    }
                    let means: Vec<f64> = sum.iter().filter(|s| s.1 > 0).map(|s| s.0 / s.1 as f64).collect();
                    mean_sd(&means).1 / (means.len() as f64).sqrt()
                }
            };
            (name.as_str(), mean, if se.is_nan() { f64::INFINITY } else { se })
        })
        .collect()
}

/// JSON of a result (also the provenance of the summary entry).
pub fn result_json(spec: &UqSpec, res: &UqResult) -> Value {
    let f = |x: f64| if x.is_finite() { json!(x) } else { json!(x.to_string()) };
    json!({
        "spec": {
            "name": spec.name, "sampler": sampler_name(spec.sampler), "seed": spec.seed,
            "params": spec.params.iter().map(|p| json!({"name": p.name, "unit": p.unit, "dist": p.dist.to_json()})).collect::<Vec<_>>(),
            "stop": {"batch": spec.stop.batch, "min_batches": spec.stop.min_batches, "max_samples": spec.stop.max_samples,
                     "se_abs": spec.stop.se_abs, "se_rel": spec.stop.se_rel, "metrics": spec.stop.metrics},
            "quantiles": spec.quantiles, "confidence": spec.confidence,
            "requirements": spec.requirements.iter().map(|r| json!({"metric": r.metric, "cmp": match r.cmp { Cmp::Le => "<=", Cmp::Ge => ">=" }, "limit": r.limit})).collect::<Vec<_>>(),
        },
        "n_samples": res.n_samples, "n_ok": res.n_ok, "batches": res.batches, "converged": res.converged,
        "ledger_range": res.ledger_range,
        "metrics": res.metrics.iter().map(|m| json!({
            "name": m.name, "unit": m.unit, "n": m.n, "mean": f(m.mean), "sd": f(m.sd), "se": f(m.se), "r2": f(m.r2),
            "quantiles": m.quantiles.iter().map(|q| json!({"p": q.p, "value": f(q.value), "lo": f(q.lo), "hi": f(q.hi), "coverage": f(q.coverage)})).collect::<Vec<_>>(),
            "src": spec.params.iter().zip(&m.src).map(|(p, s)| json!({"param": p.name, "src": f(*s)})).collect::<Vec<_>>(),
            "ranking": m.ranking, "sensitivity_method": "SRC (standardised regression coefficients of a linear least-squares fit)",
        })).collect::<Vec<_>>(),
        "requirements": res.requirements.iter().map(|r| json!({
            "metric": r.req.metric, "limit": r.req.limit, "violations": r.violations, "failed": r.failed, "n": r.n,
            "p": r.p, "lo": r.lo, "hi": r.hi,
        })).collect::<Vec<_>>(),
    })
}

fn summary_entry(spec: &UqSpec, model: &dyn Model, res: &UqResult) -> Entry {
    let mut metrics = Vec::new();
    for m in &res.metrics {
        for (k, v) in [("mean", m.mean), ("sd", m.sd), ("se", m.se)] {
            metrics.push(Quantity::new(&format!("{}.{k}", m.name), v, &m.unit));
        }
        for q in &m.quantiles {
            metrics.push(Quantity::new(&format!("{}.q{}", m.name, q.p), q.value, &m.unit));
        }
    }
    for (i, r) in res.requirements.iter().enumerate() {
        metrics.push(Quantity::new(
            &format!("requirement{i}.{}.p_violation", r.req.metric),
            r.p,
            "1",
        ));
    }
    let ok = metrics.iter().all(|q| q.value.is_finite()) && res.n_ok > 0;
    Entry {
        kind: "uq_summary".into(),
        study: spec.name.clone(),
        variant: format!("{}#summary", spec.name),
        params: Vec::new(),
        metrics,
        verdict: if ok { Verdict::Ok } else { Verdict::NotEvaluated },
        reason: if ok {
            String::new()
        } else {
            format!(
                "{} usable samples of {}; statistics incomplete",
                res.n_ok, res.n_samples
            )
        },
        seed: Some(spec.seed),
        provenance: result_json(spec, res),
        ..entry_of(model, Vec::new(), Outcome::default())
    }
}

#[cfg(test)]
#[path = "uq_tests.rs"]
mod tests;
