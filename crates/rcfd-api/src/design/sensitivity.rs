//! Variance-based (Sobol) sensitivity indices over any [`Model`] (ADR 0240, roadmap F-268).
//!
//! * Indices: first order `S_i = Var(E[Y | X_i]) / Var(Y)` and total `ST_i = E[Var(Y | X_~i)] / Var(Y)` (Sobol 1993; Homma and
//!   Saltelli 1996), for independent inputs with the distributions of [`super::uq::Dist`].
//! * Design: two independent Monte Carlo matrices `A`, `B` of `N` rows and, for each input `i`, `A_B^(i)` (`A` with column `i`
//!   from `B`) (Saltelli 2002); cost `N (d + 2)` evaluations for every index of every metric.
//! * Estimators (Saltelli et al. 2010): first order `(1/n) sum (b - m)(c_i - a) / V` (centred by the pooled mean `m`, which leaves
//!   the expectation unchanged), total `(1/2n) sum (a - c_i)^2 / V` (Jansen 1999), `V` the pooled variance of `a` and `b`.
//!   Standard errors by the delta method for a ratio of means. Estimates are not clipped to `[0, 1]`.
//! * A row is used only when all its `d + 2` evaluations are `ok`; with input-dependent failures the indices are those of the
//!   inputs conditioned on success.
//! * Opt-in: nothing else in the design loop calls this module. Every evaluation can be a ledger entry, and one `sobol_summary`
//!   entry closes the run.

use super::ledger::{entry_of, Entry, Ledger, Model, Outcome, Quantity, Verdict};
use super::rng::Rng;
use super::study::parallel_map;
use super::uq::{evaluate_guarded, UncertainParam};
use serde_json::{json, Value};

/// Definition of a Sobol-index study.
#[derive(Debug, Clone, PartialEq)]
pub struct SobolSpec {
    /// Study name (the `study` of the ledger entries).
    pub name: String,
    /// Independent uncertain inputs.
    pub params: Vec<UncertainParam>,
    /// Rows `N` of each of the matrices `A` and `B` (cost `N (d + 2)` evaluations).
    pub n: usize,
    /// Seed.
    pub seed: u64,
    /// Concurrent evaluations.
    pub workers: usize,
}

impl SobolSpec {
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
        if self.n < 2 {
            return Err(format!("n: at least 2 rows are needed (got {})", self.n));
        }
        if self.workers == 0 {
            return Err("workers: must be >= 1".into());
        }
        if self.n.checked_mul(self.params.len() + 2).is_none() {
            return Err("n: n (d + 2) evaluations overflow".into());
        }
        Ok(())
    }
}

/// Indices of one input for one metric.
#[derive(Debug, Clone, PartialEq)]
pub struct SobolIndex {
    /// Parameter name.
    pub param: String,
    /// First-order index `S_i`.
    pub first: f64,
    /// Standard error of `S_i` (delta method).
    pub first_se: f64,
    /// Total index `ST_i`.
    pub total: f64,
    /// Standard error of `ST_i`.
    pub total_se: f64,
}

/// Indices of one metric.
#[derive(Debug, Clone, PartialEq)]
pub struct SobolMetric {
    /// Metric name.
    pub name: String,
    /// Unit.
    pub unit: String,
    /// Pooled mean of `f(A)` and `f(B)`.
    pub mean: f64,
    /// Pooled variance of `f(A)` and `f(B)` (divisor `2n`).
    pub variance: f64,
    /// Indices in parameter order.
    pub indices: Vec<SobolIndex>,
}

/// Result of a Sobol-index study.
#[derive(Debug, Clone, PartialEq)]
pub struct SobolResult {
    /// Rows `N`.
    pub n_rows: usize,
    /// Rows whose `d + 2` evaluations are all `ok`.
    pub n_rows_used: usize,
    /// Evaluations made, `N (d + 2)`.
    pub n_evaluations: usize,
    /// Evaluations that were not `ok`.
    pub n_failed_evaluations: usize,
    /// Indices per metric (in the order of the first usable evaluation).
    pub metrics: Vec<SobolMetric>,
}

fn sd(x: &[f64]) -> f64 {
    let n = x.len() as f64;
    let m = x.iter().sum::<f64>() / n;
    (x.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / (n - 1.0)).sqrt()
}

/// Estimates from `a = f(A)`, `b = f(B)` and `c[i] = f(A_B^(i))` over the same usable rows: (mean, variance, per input
/// (S, SE of S, ST, SE of ST)). Fewer than 2 rows or a zero variance give NaN indices.
pub fn estimate(a: &[f64], b: &[f64], c: &[Vec<f64>]) -> (f64, f64, Vec<(f64, f64, f64, f64)>) {
    let n = a.len();
    let nan = (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    if n == 0 {
        return (f64::NAN, f64::NAN, vec![nan; c.len()]);
    }
    let nf = n as f64;
    let m = (a.iter().sum::<f64>() + b.iter().sum::<f64>()) / (2.0 * nf);
    let w: Vec<f64> = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| ((x - m) * (x - m) + (y - m) * (y - m)) / 2.0)
        .collect();
    let v = w.iter().sum::<f64>() / nf;
    if n < 2 || !(v > 0.0) {
        return (m, v, vec![nan; c.len()]);
    }
    // the ratio u-bar / v and its delta-method SE: psi_j = (u_j - u-bar - r (w_j - v)) / v
    let ratio = |u: &[f64]| -> (f64, f64) {
        let ub = u.iter().sum::<f64>() / nf;
        let r = ub / v;
        let psi: Vec<f64> = u.iter().zip(&w).map(|(&uj, &wj)| (uj - ub - r * (wj - v)) / v).collect();
        (r, sd(&psi) / nf.sqrt())
    };
    let idx = c
        .iter()
        .map(|ci| {
            let uf: Vec<f64> = (0..n).map(|j| (b[j] - m) * (ci[j] - a[j])).collect();
            let ut: Vec<f64> = (0..n).map(|j| (a[j] - ci[j]) * (a[j] - ci[j]) / 2.0).collect();
            let (s, s_se) = ratio(&uf);
            let (t, t_se) = ratio(&ut);
            (s, s_se, t, t_se)
        })
        .collect();
    (m, v, idx)
}

/// Run a Sobol-index study of `model`; every evaluation (and a summary) is recorded in `ledger` when given.
pub fn run(spec: &SobolSpec, model: &dyn Model, ledger: Option<&Ledger>) -> Result<SobolResult, String> {
    spec.validate()?;
    let d = spec.params.len();
    let per = d + 2;
    let mut rng = Rng::new(spec.seed);
    // points in evaluation order: row j gives A_j, B_j, A_B^(1)_j, ..., A_B^(d)_j
    let mut pts: Vec<Vec<f64>> = Vec::with_capacity(spec.n * per);
    for _ in 0..spec.n {
        let u: Vec<f64> = (0..2 * d)
            .map(|_| ((rng.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64)
            .collect();
        let x: Vec<f64> = u.iter().enumerate().map(|(k, &u)| spec.params[k % d].dist.inv_cdf(u)).collect();
        let (xa, xb) = (x[..d].to_vec(), x[d..].to_vec());
        pts.push(xa.clone());
        pts.push(xb.clone());
        for i in 0..d {
            let mut ab = xa.clone();
            ab[i] = xb[i];
            pts.push(ab);
        }
    }
    let named = |x: &Vec<f64>| -> Vec<Quantity> {
        spec.params
            .iter()
            .zip(x)
            .map(|(p, &v)| Quantity::new(&p.name, v, &p.unit))
            .collect()
    };
    let outs: Vec<Outcome> = parallel_map(&pts, spec.workers, &|x: &Vec<f64>| evaluate_guarded(model, &named(x)));
    let mut names: Option<Vec<(String, String)>> = None;
    for (k, o) in outs.iter().enumerate() {
        if o.verdict != Verdict::Ok {
            continue;
        }
        let these: Vec<(String, String)> = o.metrics.iter().map(|q| (q.name.clone(), q.unit.clone())).collect();
        match &names {
            None => names = Some(these),
            Some(nm) if *nm != these => {
                return Err(format!(
                    "evaluation {k}: the evaluator returned metrics {these:?}, earlier evaluations {nm:?}"
                ))
            }
            _ => {}
        }
    }
    if let Some(l) = ledger {
        let entries: Vec<Entry> = pts
            .iter()
            .zip(&outs)
            .enumerate()
            .map(|(k, (x, o))| {
                let (j, slot) = (k / per, k % per);
                let (tag, prov) = match slot {
                    0 => ("A".to_string(), json!({"driver": "sobol", "matrix": "A", "row": j})),
                    1 => ("B".to_string(), json!({"driver": "sobol", "matrix": "B", "row": j})),
                    s => (
                        format!("AB{}", s - 1),
                        json!({"driver": "sobol", "matrix": "AB", "column": s - 1, "row": j}),
                    ),
                };
                Entry {
                    study: spec.name.clone(),
                    variant: format!("{}#{j}.{tag}", spec.name),
                    seed: Some(spec.seed),
                    provenance: prov,
                    ..entry_of(model, named(x), o.clone())
                }
            })
            .collect();
        l.append(&entries)?;
    }
    let rows: Vec<usize> = (0..spec.n)
        .filter(|&j| outs[j * per..(j + 1) * per].iter().all(|o| o.verdict == Verdict::Ok))
        .collect();
    let names = names.unwrap_or_default();
    let metrics = names
        .iter()
        .enumerate()
        .map(|(k, (name, unit))| {
            let val = |j: usize, s: usize| outs[j * per + s].metrics[k].value;
            let a: Vec<f64> = rows.iter().map(|&j| val(j, 0)).collect();
            let b: Vec<f64> = rows.iter().map(|&j| val(j, 1)).collect();
            let c: Vec<Vec<f64>> = (0..d).map(|i| rows.iter().map(|&j| val(j, 2 + i)).collect()).collect();
            let (mean, variance, idx) = estimate(&a, &b, &c);
            SobolMetric {
                name: name.clone(),
                unit: unit.clone(),
                mean,
                variance,
                indices: spec
                    .params
                    .iter()
                    .zip(idx)
                    .map(|(p, (first, first_se, total, total_se))| SobolIndex {
                        param: p.name.clone(),
                        first,
                        first_se,
                        total,
                        total_se,
                    })
                    .collect(),
            }
        })
        .collect();
    let res = SobolResult {
        n_rows: spec.n,
        n_rows_used: rows.len(),
        n_evaluations: outs.len(),
        n_failed_evaluations: outs.iter().filter(|o| o.verdict != Verdict::Ok).count(),
        metrics,
    };
    if let Some(l) = ledger {
        l.append(&[summary_entry(spec, model, &res)])?;
    }
    Ok(res)
}

/// JSON of a result (also the provenance of the summary entry).
pub fn result_json(spec: &SobolSpec, res: &SobolResult) -> Value {
    let f = |x: f64| if x.is_finite() { json!(x) } else { json!(x.to_string()) };
    json!({
        "spec": {
            "name": spec.name, "n": spec.n, "seed": spec.seed, "design": "monte_carlo A, B, A_B^(i) (Saltelli 2002)",
            "params": spec.params.iter().map(|p| json!({"name": p.name, "unit": p.unit})).collect::<Vec<_>>(),
        },
        "n_rows": res.n_rows, "n_rows_used": res.n_rows_used, "n_evaluations": res.n_evaluations,
        "n_failed_evaluations": res.n_failed_evaluations,
        "estimators": "first order: centred Saltelli et al. (2010); total: Jansen (1999); SE: delta method",
        "metrics": res.metrics.iter().map(|m| json!({
            "name": m.name, "unit": m.unit, "mean": f(m.mean), "variance": f(m.variance),
            "indices": m.indices.iter().map(|i| json!({
                "param": i.param, "S": f(i.first), "S_se": f(i.first_se), "ST": f(i.total), "ST_se": f(i.total_se),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn summary_entry(spec: &SobolSpec, model: &dyn Model, res: &SobolResult) -> Entry {
    let mut metrics = Vec::new();
    for m in &res.metrics {
        let vunit = if m.unit == "1" { "1".to_string() } else { format!("({})^2", m.unit) };
        metrics.push(Quantity::new(&format!("{}.mean", m.name), m.mean, &m.unit));
        metrics.push(Quantity::new(&format!("{}.variance", m.name), m.variance, &vunit));
        for i in &m.indices {
            for (k, v) in [("S", i.first), ("S_se", i.first_se), ("ST", i.total), ("ST_se", i.total_se)] {
                metrics.push(Quantity::new(&format!("{}.{k}.{}", m.name, i.param), v, "1"));
            }
        }
    }
    let ok = !metrics.is_empty() && metrics.iter().all(|q| q.value.is_finite());
    Entry {
        kind: "sobol_summary".into(),
        study: spec.name.clone(),
        variant: format!("{}#summary", spec.name),
        params: Vec::new(),
        metrics,
        verdict: if ok { Verdict::Ok } else { Verdict::NotEvaluated },
        reason: if ok {
            String::new()
        } else {
            format!(
                "{} usable rows of {} (or a zero variance); indices incomplete",
                res.n_rows_used, res.n_rows
            )
        },
        seed: Some(spec.seed),
        provenance: result_json(spec, res),
        ..entry_of(model, Vec::new(), Outcome::default())
    }
}

#[cfg(test)]
#[path = "sensitivity_tests.rs"]
mod tests;
