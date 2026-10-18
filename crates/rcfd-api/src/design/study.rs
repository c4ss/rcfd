//! Design studies (ADR 0067): parameters with bounds, operating conditions, objectives with robust scalarisation,
//! constraints, concurrent evaluation with a worker limit, and a resumable store. The optimisers of [`super::optim`] and the
//! samplers of [`super::sampling`] are driven through [`Study`].

use super::optim::{self, DeOptions, Fitness, NmOptions, OptResult, Stop};
use super::pareto;
use super::sampling;
use super::store::{key_of, CondResult, Record, Store};
use serde_json::Value;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Largest number of design points a DOE may define in one call.
pub const MAX_DOE_POINTS: usize = 100_000;

/// One place a parameter value is written to: `scale * x + offset` into a settings-tree path.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    /// Dotted settings path.
    pub path: String,
    /// Multiplier.
    pub scale: f64,
    /// Offset.
    pub offset: f64,
}

/// A design parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDef {
    /// Name (unique within the study).
    pub name: String,
    /// Lower bound.
    pub lo: f64,
    /// Upper bound.
    pub hi: f64,
    /// Initial value (optimisers start there when all parameters have one).
    pub init: Option<f64>,
    /// Settings paths the value is written to (may be empty when a generator hook is used).
    pub targets: Vec<Target>,
}

/// An operating condition: overrides applied to every candidate before it runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    /// Label.
    pub label: String,
    /// Overrides `(path, value)`.
    pub set: Vec<(String, Value)>,
}

/// Direction of an objective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sense {
    /// Smaller is better.
    Min,
    /// Larger is better.
    Max,
}

/// How the values of one metric over the operating conditions are combined (on the minimised quantity).
#[derive(Debug, Clone, PartialEq)]
pub enum Robust {
    /// Arithmetic mean.
    Mean,
    /// Worst case (largest minimised value).
    Worst,
    /// Mean plus `k` population standard deviations.
    MeanPlusKSigma(f64),
    /// Weighted sum (weights must sum to 1, one per condition).
    Weighted(Vec<f64>),
}

impl Robust {
    /// Combine per-condition values of the minimised quantity.
    pub fn apply(&self, q: &[f64]) -> f64 {
        let n = q.len() as f64;
        match self {
            Robust::Mean => q.iter().sum::<f64>() / n,
            Robust::Worst => q.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            Robust::MeanPlusKSigma(k) => {
                let m = q.iter().sum::<f64>() / n;
                let var = q.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / n;
                m + k * var.sqrt()
            }
            Robust::Weighted(w) => q.iter().zip(w).map(|(v, w)| v * w).sum(),
        }
    }
}

/// An objective: a metric, a direction, a weight in the scalarisation and its robust mode.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectiveDef {
    /// Metric name (a report of the case, or any key of recorded metrics).
    pub metric: String,
    /// Direction.
    pub sense: Sense,
    /// Weight in the scalar objective (default 1).
    pub weight: f64,
    /// Robust combination over the conditions.
    pub robust: Robust,
}

/// Comparison of a constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmp {
    /// metric <= limit.
    Le,
    /// metric >= limit.
    Ge,
}

/// A constraint on a metric; it must hold at every operating condition (worst case).
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintDef {
    /// Metric name.
    pub metric: String,
    /// Comparison.
    pub cmp: Cmp,
    /// Limit.
    pub limit: f64,
    /// Violation normaliser (violation = excess / scale).
    pub scale: f64,
}

/// Definition of a study.
#[derive(Debug, Clone, PartialEq)]
pub struct StudyDef {
    /// Name.
    pub name: String,
    /// Parameters in a fixed order.
    pub params: Vec<ParamDef>,
    /// Operating conditions (at least one).
    pub conditions: Vec<Condition>,
    /// Objectives (at least one).
    pub objectives: Vec<ObjectiveDef>,
    /// Constraints.
    pub constraints: Vec<ConstraintDef>,
    /// Concurrent evaluations (>= 1).
    pub workers: usize,
}

impl StudyDef {
    /// Check the definition; located messages.
    pub fn validate(&self) -> Result<(), String> {
        if self.params.is_empty() {
            return Err("parameters: the study has no parameters".into());
        }
        for (i, p) in self.params.iter().enumerate() {
            if p.name.is_empty() {
                return Err(format!("parameters[{i}].name: empty name"));
            }
            if self.params[..i].iter().any(|q| q.name == p.name) {
                return Err(format!("parameter '{}': defined twice", p.name));
            }
            if !(p.lo.is_finite() && p.hi.is_finite() && p.lo < p.hi) {
                return Err(format!(
                    "parameter '{}': bounds must be finite with lo < hi (got {}, {})",
                    p.name, p.lo, p.hi
                ));
            }
            if let Some(v) = p.init {
                if !(p.lo..=p.hi).contains(&v) {
                    return Err(format!(
                        "parameter '{}': init {v} is outside [{}, {}]",
                        p.name, p.lo, p.hi
                    ));
                }
            }
            for t in &p.targets {
                if t.path.is_empty() || !t.scale.is_finite() || !t.offset.is_finite() {
                    return Err(format!("parameter '{}': bad target '{}'", p.name, t.path));
                }
            }
        }
        if self.conditions.is_empty() {
            return Err("conditions: at least one operating condition is needed".into());
        }
        for (i, c) in self.conditions.iter().enumerate() {
            if self.conditions[..i].iter().any(|q| q.label == c.label) {
                return Err(format!("condition '{}': defined twice", c.label));
            }
        }
        if self.objectives.is_empty() {
            return Err("objectives: at least one objective is needed".into());
        }
        let nc = self.conditions.len();
        for o in &self.objectives {
            if !o.weight.is_finite() || o.weight < 0.0 {
                return Err(format!("objective '{}': weight must be finite and >= 0", o.metric));
            }
            match &o.robust {
                Robust::MeanPlusKSigma(k) if !(k.is_finite() && *k >= 0.0) => {
                    return Err(format!("objective '{}': k must be finite and >= 0", o.metric));
                }
                Robust::Weighted(w) => {
                    if w.len() != nc {
                        return Err(format!(
                            "objective '{}': {} robust weights for {nc} conditions",
                            o.metric,
                            w.len()
                        ));
                    }
                    if (w.iter().sum::<f64>() - 1.0).abs() > 1e-9 || w.iter().any(|x| !x.is_finite() || *x < 0.0) {
                        return Err(format!(
                            "objective '{}': robust weights must be >= 0 and sum to 1",
                            o.metric
                        ));
                    }
                }
                _ => {}
            }
        }
        for c in &self.constraints {
            if !c.limit.is_finite() || !(c.scale.is_finite() && c.scale > 0.0) {
                return Err(format!("constraint '{}': limit must be finite and scale > 0", c.metric));
            }
        }
        if self.workers == 0 {
            return Err("workers: must be >= 1".into());
        }
        Ok(())
    }

    /// Box bounds in parameter order.
    pub fn bounds(&self) -> Vec<(f64, f64)> {
        self.params.iter().map(|p| (p.lo, p.hi)).collect()
    }

    /// Parameter vector from a name -> value lookup (every parameter must be given and inside its bounds).
    pub fn vector(&self, get: &dyn Fn(&str) -> Option<f64>) -> Result<Vec<f64>, String> {
        self.params
            .iter()
            .map(|p| {
                let v = get(&p.name).ok_or_else(|| format!("parameter '{}': no value given", p.name))?;
                if !v.is_finite() || v < p.lo || v > p.hi {
                    return Err(format!(
                        "parameter '{}': value {v} is outside [{}, {}]",
                        p.name, p.lo, p.hi
                    ));
                }
                Ok(v)
            })
            .collect()
    }
}

/// Evaluates one candidate at all operating conditions of a study.
pub trait Evaluator: Sync {
    /// Results for `x` (parameter values in `def.params` order), one per condition of `def`, in order.
    fn evaluate(&self, def: &StudyDef, x: &[f64]) -> Vec<CondResult>;
}

/// Evaluator from a closure evaluating one (candidate, condition) pair; conditions are independent.
pub struct FnEvaluator<F>(pub F);

impl<F: Fn(&[f64], &Condition) -> CondResult + Sync> Evaluator for FnEvaluator<F> {
    fn evaluate(&self, def: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        def.conditions.iter().map(|c| (self.0)(x, c)).collect()
    }
}

/// Map `f` over `items` with at most `workers` threads; the result order is the item order whatever the thread count.
pub fn parallel_map<T: Sync, R: Send>(items: &[T], workers: usize, f: &(dyn Fn(&T) -> R + Sync)) -> Vec<R> {
    let n = items.len();
    let w = workers.max(1).min(n.max(1));
    if w <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<R>>> = Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|sc| {
        for _ in 0..w {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= n {
                    break;
                }
                let r = f(&items[i]);
                out.lock().unwrap_or_else(|e| e.into_inner())[i] = Some(r);
            });
        }
    });
    out.into_inner()
        .unwrap_or_else(|e| e.into_inner())
        .into_iter()
        .map(|r| r.expect("every item was processed"))
        .collect()
}

/// A design-of-experiments request.
#[derive(Debug, Clone, PartialEq)]
pub enum Doe {
    /// Full factorial with `levels` per parameter.
    Factorial {
        /// Levels per parameter.
        levels: Vec<usize>,
    },
    /// Latin hypercube of `n` points.
    Lhs {
        /// Points.
        n: usize,
        /// Seed.
        seed: u64,
    },
    /// First `n` points of the Sobol sequence.
    Sobol {
        /// Points.
        n: usize,
    },
}

/// What a study runs: a DOE or an optimiser.
#[derive(Debug, Clone, PartialEq)]
pub enum Method {
    /// Sampled design.
    Doe(Doe),
    /// Nelder-Mead.
    NelderMead {
        /// Evaluation budget.
        budget: usize,
        /// Restarts.
        restarts: usize,
        /// Seed.
        seed: u64,
    },
    /// Differential evolution.
    Differential {
        /// Evaluation budget.
        budget: usize,
        /// Population (default `max(10 d, 20)`).
        pop: Option<usize>,
        /// Seed.
        seed: u64,
    },
}

/// Outcome of asking for the next candidates.
#[derive(Debug, Clone, PartialEq)]
pub enum Proposal {
    /// These points have no stored evaluation yet (the next batch of the method).
    Next(Vec<Vec<f64>>),
    /// The method has nothing left to ask: every request is answered by the store.
    Complete,
}

/// A running study.
pub struct Study<'a> {
    /// Definition.
    pub def: StudyDef,
    /// Evaluation store.
    pub store: Store,
    evaluator: &'a dyn Evaluator,
    /// Number of NEW evaluations still allowed (`None` = unlimited); used for interruption and per-call caps.
    pub allowance: Option<usize>,
    propose_only: bool,
    /// New evaluations made through this object.
    pub evaluated: usize,
}

fn sorted_params(def: &StudyDef, x: &[f64]) -> Vec<(String, f64)> {
    let mut v: Vec<(String, f64)> = def.params.iter().zip(x).map(|(p, &v)| (p.name.clone(), v)).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

impl<'a> Study<'a> {
    /// Study over `def` with `store` and `evaluator`.
    pub fn new(def: StudyDef, store: Store, evaluator: &'a dyn Evaluator) -> Result<Study<'a>, String> {
        def.validate()?;
        Ok(Study {
            def,
            store,
            evaluator,
            allowance: None,
            propose_only: false,
            evaluated: 0,
        })
    }

    /// Evaluate (or look up) a batch of candidates; returns the record positions in request order.
    pub fn eval_points(&mut self, xs: &[Vec<f64>], source: &str) -> Result<Vec<usize>, Stop> {
        let keys: Vec<String> = xs.iter().map(|x| key_of(x)).collect();
        let mut fresh: Vec<usize> = Vec::new(); // positions in xs of the first request of each new key
        for (i, k) in keys.iter().enumerate() {
            if self.store.get(k).is_none() && !fresh.iter().any(|&j| keys[j] == *k) {
                fresh.push(i);
            }
        }
        if self.propose_only && !fresh.is_empty() {
            return Err(Stop::Missing(fresh.iter().map(|&i| xs[i].clone()).collect()));
        }
        let take = self.allowance.map_or(fresh.len(), |a| a.min(fresh.len()));
        let to_run: Vec<&Vec<f64>> = fresh[..take].iter().map(|&i| &xs[i]).collect();
        let def = &self.def;
        let ev = self.evaluator;
        let results = parallel_map(&to_run, def.workers, &|x: &&Vec<f64>| {
            let r = catch_unwind(AssertUnwindSafe(|| ev.evaluate(def, x)));
            let mut res = match r {
                Ok(v) => v,
                Err(p) => {
                    let msg = p
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| p.downcast_ref::<&str>().map(|s| (*s).to_string()))
                        .unwrap_or_else(|| "panic".into());
                    def.conditions
                        .iter()
                        .map(|c| CondResult::failure(&c.label, "failed", format!("internal error: {msg}")))
                        .collect()
                }
            };
            if res.len() != def.conditions.len() {
                res = def
                    .conditions
                    .iter()
                    .map(|c| {
                        CondResult::failure(&c.label, "failed", "the evaluator returned a wrong number of results")
                    })
                    .collect();
            }
            for c in &mut res {
                if c.ok && c.metrics.values().any(|v| !v.is_finite()) {
                    c.ok = false;
                    c.status = "failed".into();
                    c.message = format!("{} non-finite metric", c.message).trim().to_string();
                }
            }
            res
        });
        let base = self.store.next_index();
        let recs: Vec<Record> = to_run
            .iter()
            .zip(results)
            .enumerate()
            .map(|(j, (x, conditions))| Record {
                index: base + j,
                key: key_of(x),
                params: sorted_params(&self.def, x),
                source: source.to_string(),
                message: conditions
                    .iter()
                    .filter(|c| !c.ok)
                    .map(|c| format!("{}: {}", c.label, c.message))
                    .collect::<Vec<_>>()
                    .join("; "),
                conditions,
            })
            .collect();
        self.evaluated += recs.len();
        if let Some(a) = &mut self.allowance {
            *a -= recs.len();
        }
        self.store.append(recs).map_err(Stop::Error)?;
        if take < fresh.len() {
            return Err(Stop::Interrupted);
        }
        keys.iter()
            .map(|k| {
                self.store
                    .records()
                    .iter()
                    .position(|r| r.key == *k)
                    .ok_or_else(|| Stop::Error("internal error: evaluated record not found".into()))
            })
            .collect()
    }

    /// Fitness (scalar objective and violation) of a record; a record with an unusable condition is `FAILED`.
    pub fn fitness_of(&self, rec: &Record) -> Fitness {
        let Some(obj) = self.objective_vector(rec) else {
            return Fitness::FAILED;
        };
        let f: f64 = obj.iter().zip(&self.def.objectives).map(|(v, o)| v * o.weight).sum();
        Fitness::new(f, self.violation_of(rec))
    }

    /// Robust value of every objective in minimised form (`None` when a condition failed or a metric is missing).
    pub fn objective_vector(&self, rec: &Record) -> Option<Vec<f64>> {
        if !rec.ok() {
            return None;
        }
        self.def
            .objectives
            .iter()
            .map(|o| {
                let q: Option<Vec<f64>> = rec
                    .conditions
                    .iter()
                    .map(|c| {
                        c.metrics.get(&o.metric).map(|&v| match o.sense {
                            Sense::Min => v,
                            Sense::Max => -v,
                        })
                    })
                    .collect();
                q.map(|q| o.robust.apply(&q))
            })
            .collect()
    }

    /// Total constraint violation (worst case over the conditions, normalised); infinite when a metric is missing.
    pub fn violation_of(&self, rec: &Record) -> f64 {
        self.def
            .constraints
            .iter()
            .map(|c| {
                rec.conditions
                    .iter()
                    .map(|r| match r.metrics.get(&c.metric) {
                        Some(&v) => {
                            let excess = match c.cmp {
                                Cmp::Le => v - c.limit,
                                Cmp::Ge => c.limit - v,
                            };
                            (excess / c.scale).max(0.0)
                        }
                        None => f64::INFINITY,
                    })
                    .fold(0.0, f64::max)
            })
            .sum()
    }

    /// Fitness of a batch of candidates (evaluating the missing ones).
    pub fn fitness_batch(&mut self, xs: &[Vec<f64>]) -> Result<Vec<Fitness>, Stop> {
        let pos = self.eval_points(xs, "evaluated")?;
        Ok(pos.iter().map(|&p| self.fitness_of(&self.store.records()[p])).collect())
    }

    /// Candidates of a DOE, scaled to the bounds.
    pub fn doe_points(&self, doe: &Doe) -> Result<Vec<Vec<f64>>, String> {
        let d = self.def.params.len();
        let unit = match doe {
            Doe::Factorial { levels } => {
                if levels.len() != d {
                    return Err(format!("levels: {} entries for {d} parameters", levels.len()));
                }
                let mut total = 1usize;
                for &l in levels {
                    total = total.saturating_mul(l);
                }
                if total > MAX_DOE_POINTS {
                    return Err(format!(
                        "the factorial has {total} points, the limit is {MAX_DOE_POINTS}"
                    ));
                }
                sampling::full_factorial(levels)
            }
            Doe::Lhs { n, seed } => {
                if *n == 0 || *n > MAX_DOE_POINTS {
                    return Err(format!("n must be in 1..={MAX_DOE_POINTS}"));
                }
                sampling::latin_hypercube(*n, d, *seed)
            }
            Doe::Sobol { n } => {
                if *n == 0 || *n > MAX_DOE_POINTS {
                    return Err(format!("n must be in 1..={MAX_DOE_POINTS}"));
                }
                sampling::sobol(*n, d)?
            }
        };
        Ok(sampling::scale(&unit, &self.def.bounds()))
    }

    fn start_point(&self) -> Option<Vec<f64>> {
        self.def.params.iter().map(|p| p.init).collect()
    }

    /// Run a method to completion (or until `Stop`). A DOE evaluates all its points and reports the best.
    pub fn run(&mut self, method: &Method) -> Result<OptResult, Stop> {
        let bounds = self.def.bounds();
        match method {
            Method::Doe(doe) => {
                let pts = self.doe_points(doe).map_err(Stop::Error)?;
                let fits = self.fitness_batch(&pts)?;
                let mut bi = 0;
                for i in 1..fits.len() {
                    if fits[i].better(&fits[bi]) {
                        bi = i;
                    }
                }
                Ok(OptResult {
                    x: pts[bi].clone(),
                    fitness: fits[bi],
                    evaluations: pts.len(),
                    converged: false,
                })
            }
            Method::NelderMead { budget, restarts, seed } => {
                let opts = NmOptions {
                    max_evals: *budget,
                    restarts: *restarts,
                    seed: *seed,
                    x0: self.start_point(),
                    ..Default::default()
                };
                optim::nelder_mead(&bounds, &opts, &mut |xs| self.fitness_batch(xs))
            }
            Method::Differential { budget, pop, seed } => {
                let opts = DeOptions {
                    max_evals: *budget,
                    pop: *pop,
                    seed: *seed,
                    x0: self.start_point(),
                    ..Default::default()
                };
                optim::differential_evolution(&bounds, &opts, &mut |xs| self.fitness_batch(xs))
            }
        }
    }

    /// The next candidates the method asks for, given the store (no evaluation is made).
    pub fn propose(&mut self, method: &Method) -> Result<Proposal, String> {
        self.propose_only = true;
        let r = self.run(method);
        self.propose_only = false;
        match r {
            Ok(_) => Ok(Proposal::Complete),
            Err(Stop::Missing(points)) => Ok(Proposal::Next(points)),
            Err(Stop::Error(e)) => Err(e),
            Err(Stop::Interrupted) => Err("internal error: interrupted while proposing".into()),
        }
    }

    /// Record metrics supplied by the caller for the candidate `x` (one result per condition). Refuses a candidate that
    /// already has a record.
    pub fn record(&mut self, x: &[f64], conditions: Vec<CondResult>, message: &str) -> Result<usize, String> {
        if x.len() != self.def.params.len() {
            return Err("wrong number of parameter values".into());
        }
        if conditions.len() != self.def.conditions.len() {
            return Err(format!(
                "{} condition results for {} conditions",
                conditions.len(),
                self.def.conditions.len()
            ));
        }
        for (r, c) in conditions.iter().zip(&self.def.conditions) {
            if r.label != c.label {
                return Err(format!(
                    "condition results must follow the study order; expected '{}', got '{}'",
                    c.label, r.label
                ));
            }
            if r.metrics.values().any(|v| !v.is_finite()) {
                return Err(format!("condition '{}': metrics must be finite", r.label));
            }
        }
        let key = key_of(x);
        if self.store.get(&key).is_some() {
            return Err("this candidate already has a record".into());
        }
        let index = self.store.next_index();
        self.store.append(vec![Record {
            index,
            key,
            params: sorted_params(&self.def, x),
            source: "recorded".into(),
            conditions,
            message: message.to_string(),
        }])?;
        Ok(index)
    }

    /// Parameter vector of a record in definition order.
    pub fn x_of(&self, rec: &Record) -> Vec<f64> {
        self.def
            .params
            .iter()
            .map(|p| {
                rec.params
                    .iter()
                    .find(|(n, _)| *n == p.name)
                    .map_or(f64::NAN, |(_, v)| *v)
            })
            .collect()
    }

    /// Position of the best usable record (Deb's rules on the scalar objective), if any.
    pub fn best(&self) -> Option<usize> {
        let recs = self.store.records();
        let mut best: Option<(usize, Fitness)> = None;
        for (i, r) in recs.iter().enumerate() {
            let f = self.fitness_of(r);
            if f.f.is_infinite() && !f.feasible() {
                continue;
            }
            if best.as_ref().map_or(true, |(_, b)| f.better(b)) {
                best = Some((i, f));
            }
        }
        best.map(|(i, _)| i)
    }

    /// Positions of the non-dominated feasible records in the objective space.
    pub fn pareto(&self) -> Vec<usize> {
        let recs = self.store.records();
        let mut idx = Vec::new();
        let mut objs = Vec::new();
        let mut viol = Vec::new();
        for (i, r) in recs.iter().enumerate() {
            if let Some(o) = self.objective_vector(r) {
                idx.push(i);
                objs.push(o);
                viol.push(self.violation_of(r));
            }
        }
        pareto::front(&objs, &viol)
            .into_iter()
            .filter(|&k| viol[k] <= 0.0)
            .map(|k| idx[k])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::atomic::AtomicUsize;

    pub(crate) fn def1(workers: usize, conds: usize) -> StudyDef {
        StudyDef {
            name: "t".into(),
            params: vec![
                ParamDef {
                    name: "x".into(),
                    lo: -2.0,
                    hi: 2.0,
                    init: Some(-1.2),
                    targets: vec![],
                },
                ParamDef {
                    name: "y".into(),
                    lo: -2.0,
                    hi: 2.0,
                    init: Some(1.0),
                    targets: vec![],
                },
            ],
            conditions: (0..conds)
                .map(|i| Condition {
                    label: format!("c{i}"),
                    set: vec![],
                })
                .collect(),
            objectives: vec![ObjectiveDef {
                metric: "f".into(),
                sense: Sense::Min,
                weight: 1.0,
                robust: Robust::Mean,
            }],
            constraints: vec![],
            workers,
        }
    }

    fn rosen(x: &[f64]) -> f64 {
        100.0 * (x[1] - x[0] * x[0]).powi(2) + (1.0 - x[0]).powi(2)
    }

    fn fn_eval(x: &[f64], c: &Condition) -> CondResult {
        let mut m = BTreeMap::new();
        m.insert("f".to_string(), rosen(x));
        CondResult::success(&c.label, m)
    }

    #[test]
    fn robust_scalarisation_matches_hand_values() {
        let q = [0.0, 1.0, 9.0]; // (x - c)^2 at x = 0 for c = 0, 1, 3
        assert_eq!(Robust::Mean.apply(&q), 10.0 / 3.0);
        assert_eq!(Robust::Worst.apply(&q), 9.0);
        let m = 10.0 / 3.0;
        let var = (q.iter().map(|v| v * v).sum::<f64>()) / 3.0 - m * m;
        assert!((Robust::MeanPlusKSigma(2.0).apply(&q) - (m + 2.0 * var.sqrt())).abs() < 1e-12);
        assert!((Robust::Weighted(vec![0.5, 0.25, 0.25]).apply(&q) - 2.5).abs() < 1e-15);
    }

    #[test]
    fn r1_robust_optimum_over_conditions() {
        // f(x; c) = (x - c)^2 over c in {0, 1, 3}: mean optimum 4/3 (value 14/9), worst-case optimum 1.5 (value 2.25)
        let cs = [0.0, 1.0, 3.0];
        let ev = FnEvaluator(|x: &[f64], c: &Condition| {
            let cv = cs[c.label.trim_start_matches('c').parse::<usize>().unwrap()];
            let mut m = BTreeMap::new();
            m.insert("f".to_string(), (x[0] - cv).powi(2));
            CondResult::success(&c.label, m)
        });
        for (robust, x_star, tol, f_star) in [
            (Robust::Mean, 4.0 / 3.0, 1e-4, 14.0 / 9.0),
            (Robust::Worst, 1.5, 1e-3, 2.25),
        ] {
            for seed in 1..=5u64 {
                let mut d = def1(1, 3);
                d.params.truncate(1);
                d.params[0].init = None;
                d.params[0].lo = -5.0;
                d.params[0].hi = 5.0;
                d.objectives[0].robust = robust.clone();
                let mut s = Study::new(d, Store::memory("t"), &ev).unwrap();
                let r = s
                    .run(&Method::NelderMead {
                        budget: 300,
                        restarts: 2,
                        seed,
                    })
                    .unwrap();
                assert!((r.x[0] - x_star).abs() <= tol, "{robust:?} seed {seed}: x = {}", r.x[0]);
                assert!(
                    (r.fitness.f - f_star).abs() <= 1e-5,
                    "{robust:?} seed {seed}: f = {}",
                    r.fitness.f
                );
            }
        }
    }

    #[test]
    fn s1_resume_after_interruption_is_identical_for_both_optimisers() {
        for method in [
            Method::NelderMead {
                budget: 120,
                restarts: 1,
                seed: 3,
            },
            Method::Differential {
                budget: 400,
                pop: Some(20),
                seed: 3,
            },
        ] {
            let calls = AtomicUsize::new(0);
            let ev = FnEvaluator(|x: &[f64], c: &Condition| {
                calls.fetch_add(1, Ordering::SeqCst);
                fn_eval(x, c)
            });
            // uninterrupted reference
            let mut full = Study::new(def1(1, 1), Store::memory("t"), &ev).unwrap();
            full.run(&method).unwrap();
            let total = calls.swap(0, Ordering::SeqCst);
            assert_eq!(total, full.store.records().len());
            for k in [7usize, 61] {
                let dir = std::env::temp_dir().join(format!("rcfd_resume_{}_{k}", std::process::id()));
                let _ = std::fs::remove_dir_all(&dir);
                let path = dir.join("evaluations.jsonl");
                let mut s = Study::new(def1(1, 1), Store::open(&path, "t").unwrap(), &ev).unwrap();
                s.allowance = Some(k);
                assert_eq!(s.run(&method).unwrap_err(), Stop::Interrupted);
                assert_eq!(calls.swap(0, Ordering::SeqCst), k);
                // resume in a fresh object over the reopened file
                let mut s2 = Study::new(def1(1, 1), Store::open(&path, "t").unwrap(), &ev).unwrap();
                assert_eq!(s2.store.records().len(), k);
                s2.run(&method).unwrap();
                assert_eq!(calls.swap(0, Ordering::SeqCst), total - k, "evaluations in the resume");
                assert_eq!(s2.store.records(), full.store.records());
                let again = Study::new(def1(1, 1), Store::open(&path, "t").unwrap(), &ev).unwrap();
                assert_eq!(again.store.records(), full.store.records());
                // a third launch evaluates nothing
                let mut s3 = Study::new(def1(1, 1), Store::open(&path, "t").unwrap(), &ev).unwrap();
                s3.run(&method).unwrap();
                assert_eq!(calls.swap(0, Ordering::SeqCst), 0);
                // a truncated last line resumes identically
                let text = std::fs::read_to_string(&path).unwrap();
                std::fs::write(&path, &text[..text.len() - 30]).unwrap();
                let mut s4 = Study::new(def1(1, 1), Store::open(&path, "t").unwrap(), &ev).unwrap();
                s4.run(&method).unwrap();
                assert_eq!(
                    calls.swap(0, Ordering::SeqCst),
                    1,
                    "only the damaged record is repeated"
                );
                assert_eq!(s4.store.records(), full.store.records());
            }
        }
    }

    #[test]
    fn s2_worker_limit_and_worker_independence() {
        let running = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let ev = FnEvaluator(|x: &[f64], c: &Condition| {
            let now = running.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(3));
            running.fetch_sub(1, Ordering::SeqCst);
            fn_eval(x, c)
        });
        let method = Method::Differential {
            budget: 200,
            pop: Some(20),
            seed: 5,
        };
        let mut one = Study::new(def1(1, 1), Store::memory("t"), &ev).unwrap();
        one.run(&method).unwrap();
        assert_eq!(peak.load(Ordering::SeqCst), 1);
        peak.store(0, Ordering::SeqCst);
        let mut three = Study::new(def1(3, 1), Store::memory("t"), &ev).unwrap();
        three.run(&method).unwrap();
        let p = peak.load(Ordering::SeqCst);
        assert!((2..=3).contains(&p), "peak concurrency {p}");
        assert_eq!(one.store.records(), three.store.records());
        // O4: four workers give the same history as one, for both optimisers
        let mut four = Study::new(def1(4, 1), Store::memory("t"), &ev).unwrap();
        four.run(&method).unwrap();
        assert_eq!(one.store.records(), four.store.records());
        let nm = Method::NelderMead {
            budget: 100,
            restarts: 1,
            seed: 5,
        };
        let mut n1 = Study::new(def1(1, 1), Store::memory("t"), &ev).unwrap();
        n1.run(&nm).unwrap();
        let mut n4 = Study::new(def1(4, 1), Store::memory("t"), &ev).unwrap();
        n4.run(&nm).unwrap();
        assert_eq!(n1.store.records(), n4.store.records());
    }

    #[test]
    fn propose_replays_the_store_and_record_feeds_it() {
        let ev = FnEvaluator(|_x: &[f64], _c: &Condition| -> CondResult { panic!("must not be called") });
        let mut s = Study::new(def1(1, 1), Store::memory("t"), &ev).unwrap();
        let method = Method::NelderMead {
            budget: 150,
            restarts: 2,
            seed: 1,
        };
        let mut n_rounds = 0;
        loop {
            match s.propose(&method).unwrap() {
                Proposal::Complete => break,
                Proposal::Next(points) => {
                    n_rounds += 1;
                    assert!(n_rounds < 400);
                    for p in points {
                        let mut m = BTreeMap::new();
                        m.insert("f".to_string(), rosen(&p));
                        s.record(&p, vec![CondResult::success("c0", m)], "from the test")
                            .unwrap();
                    }
                }
            }
        }
        // the proposal loop reproduces an evaluator-driven run bit for bit
        let ev2 = FnEvaluator(fn_eval);
        let mut direct = Study::new(def1(1, 1), Store::memory("t"), &ev2).unwrap();
        direct.run(&method).unwrap();
        let strip = |s: &Study| {
            s.store
                .records()
                .iter()
                .map(|r| (r.index, r.params.clone(), r.conditions[0].metrics.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(strip(&s), strip(&direct));
        let b = s.best().unwrap();
        assert!(s.fitness_of(&s.store.records()[b]).f < 1e-2);
        // recording the same candidate twice is refused
        let x = s.x_of(&s.store.records()[0]);
        assert!(s
            .record(&x, vec![CondResult::success("c0", BTreeMap::new())], "")
            .unwrap_err()
            .contains("already"));
    }

    #[test]
    fn constraints_pareto_and_doe_through_the_study() {
        // two objectives f1 = x, f2 = (1 - sqrt x)^2 ... use the analytic front f2 = 1 - sqrt(f1) with parameter t
        let ev = FnEvaluator(|x: &[f64], c: &Condition| {
            let mut m = BTreeMap::new();
            m.insert("f1".to_string(), x[0]);
            m.insert("f2".to_string(), 1.0 - x[0].sqrt() + x[1]); // x[1] >= 0 lifts the point off the front
            m.insert("g".to_string(), x[0]);
            CondResult::success(&c.label, m)
        });
        let mut d = def1(2, 1);
        d.params[0].lo = 0.0;
        d.params[0].hi = 1.0;
        d.params[0].init = None;
        d.params[1].lo = 0.0;
        d.params[1].hi = 1.0;
        d.params[1].init = None;
        d.objectives = ["f1", "f2"]
            .iter()
            .map(|m| ObjectiveDef {
                metric: (*m).into(),
                sense: Sense::Min,
                weight: 1.0,
                robust: Robust::Mean,
            })
            .collect();
        d.constraints = vec![ConstraintDef {
            metric: "g".into(),
            cmp: Cmp::Le,
            limit: 0.8,
            scale: 1.0,
        }];
        let mut s = Study::new(d, Store::memory("t"), &ev).unwrap();
        s.run(&Method::Doe(Doe::Factorial { levels: vec![11, 3] })).unwrap();
        assert_eq!(s.store.records().len(), 33);
        let front = s.pareto();
        // non-dominated feasible points: y = 0 and f1 <= 0.8, i.e. x = 0.0 ... 0.8 (9 points)
        assert_eq!(front.len(), 9);
        for p in &front {
            let r = &s.store.records()[*p];
            assert_eq!(r.params.iter().find(|(n, _)| n == "y").unwrap().1, 0.0);
            assert!(r.params.iter().find(|(n, _)| n == "x").unwrap().1 <= 0.8 + 1e-12);
        }
        let b = s.best().unwrap();
        assert!(s.violation_of(&s.store.records()[b]) <= 0.0);
        // Latin hypercube and Sobol DOEs
        let mut s2 = Study::new(s.def.clone(), Store::memory("t"), &ev).unwrap();
        s2.run(&Method::Doe(Doe::Lhs { n: 16, seed: 2 })).unwrap();
        s2.run(&Method::Doe(Doe::Sobol { n: 16 })).unwrap();
        assert_eq!(s2.store.records().len(), 32); // 16 LHS + 16 Sobol, no shared point
    }
}
