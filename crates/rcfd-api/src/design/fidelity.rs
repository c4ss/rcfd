//! Fidelity management (ADR 0622, fan breakdown H7): screen many candidates with a cheap model, promote a few to a
//! higher-fidelity source, log the disagreement as fidelity pairs and correct the screening with a fitted discrepancy model.
//! Opt-in library only: no service method, MCP tool or case key calls it.
//!
//! * [`run`] is the driver: round 1 screens every candidate with the low [`Model`]; each round promotes candidates (exploit
//!   by the corrected objective, explore by greedy max-min distance, or random as a negative control), evaluates them with a
//!   [`HighSource`], writes one `fidelity_pair` per ok promotion and one `fidelity_summary` per round, and refits the
//!   correction.
//! * [`Correction`] is the discrepancy ladder R0 (none), R1 (additive or multiplicative constant), R2 (`rho y_L + b0 + c . x`)
//!   chosen by leave-one-out error with the one-standard-error rule, trusted only when it helps on held-out points and the
//!   discrepancy exceeds the source's stated noise, applied only inside the calibrated region (outside: the constant only).
//!   Opt-in rung R3 (ADR 0652, `TrustRule::r3`): R2 plus a zero-mean Gaussian process on the R2 residual, with the nugget
//!   from the stated uncertainty, hyperparameters fixed per fit and leave-one-out residuals from the closed form.
//! * Privacy (ADR 0622 D7, ADR 0632): a private source (for example [`MeasuredTable`]) sends measured values, pairs, the
//!   correction and corrected predictions to a private ledger outside any repository; the public ledger gets the low
//!   entries and rounded aggregates over at least `n_pub` pairs only.
//!
//! Every result is verified on synthetic sources only; it says nothing about how well the reduced-order model predicts a real
//! fan.

use super::ledger::{self, Entry, Fidelity, Identity, Ledger, Model, Outcome, Quantity, Source, Verdict};
use super::rng::Rng;
use super::sampling;
use super::study::{parallel_map, Sense};
use super::surrogate::{self, Gp, Kernel, ZeroMeanOptions};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Evaluator id of the summaries.
pub const DRIVER_ID: &str = "rcfd.fidelity";
/// Version of the driver and of its summaries.
pub const DRIVER_VERSION: &str = "1";
/// Id of the fitted correction in the summaries' `fitted` field.
pub const CORRECTION_ID: &str = "rcfd.fidelity.correction";

// ---------------------------------------------------------------- high-fidelity sources

/// A high-fidelity observation: the outcome and the stated standard uncertainty of named metrics (same units).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Observation {
    /// Outcome.
    pub outcome: Outcome,
    /// Standard uncertainty per metric (missing: 0).
    pub uncertainty: Vec<Quantity>,
}

/// A higher-fidelity source (a model, CFD, or a measured table).
pub trait HighSource: Sync {
    /// Source id.
    fn id(&self) -> &str;
    /// Version.
    fn version(&self) -> &str;
    /// Tier.
    fn fidelity(&self) -> Fidelity;
    /// Level inside the tier (written as `fidelity_level`).
    fn level(&self) -> Option<String> {
        None
    }
    /// Whether the values are private (measured): they then go to a private ledger only.
    fn private(&self) -> bool {
        false
    }
    /// Source of the values (written as `source`).
    fn source(&self) -> Option<Source> {
        None
    }
    /// Whether the source can answer for these parameters (a measured table only for variants that were built).
    fn available(&self, _params: &[Quantity]) -> bool {
        true
    }
    /// Observe one variant.
    fn observe(&self, params: &[Quantity]) -> Observation;
}

/// Any [`Model`] as a public [`HighSource`] with zero uncertainty, always available.
pub struct ModelSource<'m> {
    /// The model.
    pub model: &'m dyn Model,
    /// Id written to the ledger.
    pub id: String,
    /// Version written to the ledger.
    pub version: String,
    /// Tier written to the ledger.
    pub fidelity: Fidelity,
    /// Level written to the ledger.
    pub level: Option<String>,
}

impl<'m> ModelSource<'m> {
    /// The model with its own identity and no level.
    pub fn of(model: &'m dyn Model) -> ModelSource<'m> {
        ModelSource {
            model,
            id: model.id().to_string(),
            version: model.version().to_string(),
            fidelity: model.fidelity(),
            level: None,
        }
    }
}

impl HighSource for ModelSource<'_> {
    fn id(&self) -> &str {
        &self.id
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn fidelity(&self) -> Fidelity {
        self.fidelity
    }
    fn level(&self) -> Option<String> {
        self.level.clone()
    }
    fn observe(&self, params: &[Quantity]) -> Observation {
        Observation {
            outcome: self.model.evaluate(params),
            uncertainty: Vec::new(),
        }
    }
}

/// Maps a candidate's parameters to the variant id of a measured table.
pub type VariantOf = Box<dyn Fn(&[Quantity]) -> Option<String> + Send + Sync>;

/// One mapped column of a [`MeasuredTable`].
#[derive(Debug, Clone, PartialEq)]
pub struct TableColumn {
    /// Column name in the header.
    pub column: String,
    /// Metric name it gives.
    pub metric: String,
    /// Unit of the metric.
    pub unit: String,
    /// Stated standard uncertainty of the column (same unit).
    pub uncertainty: f64,
}

/// A measured table (ADR 0622 D7, decision B1): the CSV of `docs/fan-measurement-protocol.md` "Data delivery" with a
/// `variant` column added. Fields are split on `,` and trimmed (no quoting). A private source of tier `measured`. Every error
/// names the file, the row and the column, never a cell's text.
pub struct MeasuredTable {
    id: String,
    version: String,
    columns: Vec<TableColumn>,
    rows: BTreeMap<String, Vec<f64>>,
    variant_of: VariantOf,
}

impl MeasuredTable {
    /// Read the table at `path`. The first row of each variant is used.
    pub fn read(
        path: &Path,
        columns: Vec<TableColumn>,
        variant_of: VariantOf,
        id: &str,
        version: &str,
    ) -> Result<MeasuredTable, String> {
        let at = format!("measured table {}", path.display());
        if id.trim().is_empty() || version.trim().is_empty() {
            return Err(format!("{at}: give an id and a version"));
        }
        for c in &columns {
            if !(c.uncertainty.is_finite() && c.uncertainty >= 0.0) {
                return Err(format!(
                    "{at}: column '{}': the stated uncertainty is not finite and non-negative",
                    c.column
                ));
            }
        }
        let bytes = std::fs::read(path).map_err(|e| format!("{at}: cannot read ({})", e.kind()))?;
        let text = String::from_utf8(bytes).map_err(|_| format!("{at}: not UTF-8 text"))?;
        let mut lines = text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .enumerate()
            .filter(|(_, l)| !l.trim().is_empty());
        let (_, header) = lines.next().ok_or_else(|| format!("{at}: empty (no header row)"))?;
        let head: Vec<&str> = header.split(',').map(str::trim).collect();
        let col = |name: &str| -> Result<usize, String> {
            head.iter()
                .position(|h| *h == name)
                .ok_or_else(|| format!("{at}: the header has no column '{name}'"))
        };
        let vi = col("variant")?;
        let idx: Vec<usize> = columns.iter().map(|c| col(&c.column)).collect::<Result<_, _>>()?;
        let mut rows = BTreeMap::new();
        for (n, line) in lines {
            let row = n + 1;
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            if f.len() != head.len() {
                return Err(format!(
                    "{at}: row {row} has {} fields, the header has {}",
                    f.len(),
                    head.len()
                ));
            }
            if f[vi].is_empty() {
                return Err(format!("{at}: row {row}, column 'variant': empty"));
            }
            let mut v = Vec::with_capacity(idx.len());
            for (c, &k) in columns.iter().zip(&idx) {
                let x: f64 = f[k]
                    .parse()
                    .map_err(|_| format!("{at}: row {row}, column '{}': not a number", c.column))?;
                if !x.is_finite() {
                    return Err(format!("{at}: row {row}, column '{}': not finite", c.column));
                }
                v.push(x);
            }
            rows.entry(f[vi].to_string()).or_insert(v);
        }
        Ok(MeasuredTable {
            id: id.to_string(),
            version: version.to_string(),
            columns,
            rows,
            variant_of,
        })
    }

    /// Number of variants in the table.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the table has no variant.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl HighSource for MeasuredTable {
    fn id(&self) -> &str {
        &self.id
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::Measured
    }
    fn private(&self) -> bool {
        true
    }
    fn source(&self) -> Option<Source> {
        Some(Source {
            id: self.id.clone(),
            kind: "measured".into(),
            citation: None,
            file_sha256: None,
        })
    }
    fn available(&self, params: &[Quantity]) -> bool {
        (self.variant_of)(params).is_some_and(|v| self.rows.contains_key(&v))
    }
    fn observe(&self, params: &[Quantity]) -> Observation {
        let fail = |reason: String| Observation {
            outcome: Outcome {
                verdict: Verdict::NotEvaluated,
                reason,
                ..Default::default()
            },
            uncertainty: Vec::new(),
        };
        let Some(v) = (self.variant_of)(params) else {
            return fail("measured table: no variant for these parameters".into());
        };
        let Some(row) = self.rows.get(&v) else {
            return fail(format!("measured table: variant '{v}' has no row"));
        };
        Observation {
            outcome: Outcome::ok(
                self.columns
                    .iter()
                    .zip(row)
                    .map(|(c, &x)| Quantity::new(&c.metric, x, &c.unit))
                    .collect(),
            ),
            uncertainty: self
                .columns
                .iter()
                .map(|c| Quantity::new(&c.metric, c.uncertainty, &c.unit))
                .collect(),
        }
    }
}

// ---------------------------------------------------------------- the specification

/// A design parameter and its box.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamBox {
    /// Name.
    pub name: String,
    /// Unit.
    pub unit: String,
    /// Lower bound.
    pub lo: f64,
    /// Upper bound.
    pub hi: f64,
}

impl ParamBox {
    /// A parameter box.
    pub fn new(name: &str, unit: &str, lo: f64, hi: f64) -> ParamBox {
        ParamBox {
            name: name.into(),
            unit: unit.into(),
            lo,
            hi,
        }
    }
}

/// The candidate set.
#[derive(Debug, Clone, PartialEq)]
pub enum Candidates {
    /// Raw parameter vectors.
    List(Vec<Vec<f64>>),
    /// A seeded Latin hypercube of `n` points in the box.
    Lhs {
        /// Number of points.
        n: usize,
        /// Seed.
        seed: u64,
    },
}

/// A metric to correct and the floor of its relative difference.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectedMetric {
    /// Metric name (of the low model and the high source).
    pub name: String,
    /// Floor of `|high|` in `rel = diff / max(|high|, floor)`.
    pub floor: f64,
}

/// Promotion rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Promotion {
    /// Exploit by the corrected objective, then explore by greedy max-min distance (ADR 0622 D3).
    Rule,
    /// Random promotion with the same budget (a negative control).
    Random {
        /// Seed.
        seed: u64,
    },
}

/// When a correction is trusted (ADR 0622 D5, amendment 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrustRule {
    /// The corrected leave-one-out RMSE must be at most `f_trust` times the uncorrected RMSE.
    pub f_trust: f64,
    /// The uncorrected RMSE must exceed `k_u` times the rms stated uncertainty of the high source.
    pub k_u: f64,
    /// Growth of the calibrated bounding box in the unit box.
    pub margin: f64,
    /// Largest distance (unit box) from the nearest pair of a query inside the calibrated region.
    pub h_max: f64,
    /// Negative control only: in-sample residuals in place of leave-one-out ones.
    pub in_sample: bool,
    /// One-standard-error rule (off: the smallest error wins; a negative control).
    pub one_se: bool,
    /// Rung R3, the Gaussian process on the R2 residual (ADR 0652); `None` (the default) leaves the ladder R0 to R2.
    pub r3: Option<R3Rule>,
}

/// Settings of rung R3 (ADR 0652 D2, D4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct R3Rule {
    /// R3 needs at least `n_per_d * d` pairs.
    pub n_per_d: usize,
    /// Smallest 2-sigma leave-one-out coverage of a trusted R3.
    pub coverage_min: f64,
    /// Starts of the likelihood search (the centre of the log box and `starts - 1` Latin-hypercube points).
    pub starts: usize,
    /// Starts refined by the gradient search.
    pub refine: usize,
    /// Gradient iterations per refined start.
    pub iters: usize,
    /// Length-scale box (unit-box lengths).
    pub l_bounds: (f64, f64),
    /// Box of the process variance over the mean squared R2 residual.
    pub tau_bounds: (f64, f64),
    /// Seed of the multi-start design.
    pub seed: u64,
}

impl Default for R3Rule {
    fn default() -> Self {
        R3Rule {
            n_per_d: 10,
            coverage_min: 0.8,
            starts: 10,
            refine: 3,
            iters: 40,
            l_bounds: (0.05, 10.0),
            tau_bounds: (1e-6, 1e2),
            seed: 1,
        }
    }
}

impl Default for TrustRule {
    fn default() -> Self {
        TrustRule {
            f_trust: 0.8,
            k_u: 2.0,
            margin: 0.05,
            h_max: 0.25,
            in_sample: false,
            one_se: true,
            r3: None,
        }
    }
}

/// Where private output goes (ADR 0622 D7).
#[derive(Debug, Clone, PartialEq)]
pub struct Privacy {
    /// Path of the private ledger (refused inside a repository tree).
    pub path: PathBuf,
    /// Id of the private source named in the public summaries (never a file path).
    pub id: String,
    /// Smallest pair count of a public aggregate.
    pub n_pub: usize,
    /// Significant digits of a public aggregate.
    pub digits: usize,
    /// Aggregates are published again only after `block` more pairs.
    pub block: usize,
}

impl Privacy {
    /// The defaults of the maintainer's decision D3: at least 5 pairs, 3 significant digits, blocks of 5.
    pub fn new(path: &Path, id: &str) -> Privacy {
        Privacy {
            path: path.to_path_buf(),
            id: id.into(),
            n_pub: 5,
            digits: 3,
            block: 5,
        }
    }
}

/// The driver's specification.
#[derive(Debug, Clone, PartialEq)]
pub struct FidelitySpec {
    /// Study name (ledger `study`).
    pub name: String,
    /// Parameters with their boxes.
    pub params: Vec<ParamBox>,
    /// Candidate set.
    pub candidates: Candidates,
    /// Metrics to correct.
    pub metrics: Vec<CorrectedMetric>,
    /// Ranking objective: a metric of the low model (ranked by its corrected prediction when it is a corrected metric).
    pub objective: String,
    /// Sense of the objective.
    pub sense: Sense,
    /// Rounds.
    pub rounds: usize,
    /// Promotions per round.
    pub promote_per_round: usize,
    /// Of them, explore picks per round.
    pub explore_per_round: usize,
    /// Promotion rule.
    pub promotion: Promotion,
    /// Fit and apply a correction (off: R0 everywhere).
    pub correct: bool,
    /// Trust rule.
    pub trust: TrustRule,
    /// Worker threads.
    pub workers: usize,
    /// Private output; required when the high source is private.
    pub privacy: Option<Privacy>,
}

impl FidelitySpec {
    /// A specification with the defaults: 4 rounds of 6 (2 explore), the rule, correction on, the default trust rule, 1 worker.
    pub fn new(
        name: &str,
        params: Vec<ParamBox>,
        candidates: Candidates,
        metrics: Vec<CorrectedMetric>,
        objective: &str,
        sense: Sense,
    ) -> FidelitySpec {
        FidelitySpec {
            name: name.into(),
            params,
            candidates,
            metrics,
            objective: objective.into(),
            sense,
            rounds: 4,
            promote_per_round: 6,
            explore_per_round: 2,
            promotion: Promotion::Rule,
            correct: true,
            trust: TrustRule::default(),
            workers: 1,
            privacy: None,
        }
    }

    fn to_json(&self, with_path: bool) -> Value {
        let cands = match &self.candidates {
            Candidates::List(v) => json!({"list": v.len()}),
            Candidates::Lhs { n, seed } => json!({"lhs": {"n": n, "seed": seed}}),
        };
        let t = &self.trust;
        let mut trust = json!({"f_trust": t.f_trust, "k_u": t.k_u, "margin": t.margin, "h_max": t.h_max, "in_sample": t.in_sample, "one_se": t.one_se});
        if let Some(r) = &t.r3 {
            trust["r3"] = json!({
                "n_per_d": r.n_per_d, "coverage_min": r.coverage_min, "starts": r.starts, "refine": r.refine, "iters": r.iters,
                "l_bounds": [r.l_bounds.0, r.l_bounds.1], "tau_bounds": [r.tau_bounds.0, r.tau_bounds.1], "seed": r.seed,
            });
        }
        json!({
            "name": self.name,
            "params": self.params.iter().map(|p| json!({"name": p.name, "unit": p.unit, "lo": p.lo, "hi": p.hi})).collect::<Vec<_>>(),
            "candidates": cands,
            "metrics": self.metrics.iter().map(|m| json!({"name": m.name, "floor": m.floor})).collect::<Vec<_>>(),
            "objective": self.objective,
            "sense": match self.sense { Sense::Min => "min", Sense::Max => "max" },
            "rounds": self.rounds, "promote_per_round": self.promote_per_round, "explore_per_round": self.explore_per_round,
            "promotion": match self.promotion { Promotion::Rule => json!("rule"), Promotion::Random { seed } => json!({"random": seed}) },
            "correct": self.correct,
            "trust": trust,
            "privacy": self.privacy.as_ref().map(|p| {
                let mut v = json!({"id": p.id, "n_pub": p.n_pub, "digits": p.digits, "block": p.block});
                if with_path {
                    v["path"] = json!(p.path.display().to_string());
                }
                v
            }),
        })
    }

    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("spec.name: empty".into());
        }
        if self.params.is_empty() {
            return Err("spec.params: none".into());
        }
        for (i, p) in self.params.iter().enumerate() {
            if p.name.trim().is_empty() {
                return Err(format!("spec.params[{i}]: empty name"));
            }
            if self.params[..i].iter().any(|q| q.name == p.name) {
                return Err(format!("spec.params '{}': given twice", p.name));
            }
            if !(p.lo.is_finite() && p.hi.is_finite() && p.lo < p.hi) {
                return Err(format!("spec.params '{}': need finite lo < hi", p.name));
            }
        }
        if self.metrics.is_empty() {
            return Err("spec.metrics: none".into());
        }
        for (i, m) in self.metrics.iter().enumerate() {
            if self.metrics[..i].iter().any(|q| q.name == m.name) {
                return Err(format!("spec.metrics '{}': given twice", m.name));
            }
            if !(m.floor.is_finite() && m.floor > 0.0) {
                return Err(format!("spec.metrics '{}': floor not finite and positive", m.name));
            }
        }
        if self.objective.trim().is_empty() {
            return Err("spec.objective: empty".into());
        }
        if self.rounds == 0 || self.promote_per_round == 0 {
            return Err("spec: rounds and promote_per_round must be at least 1".into());
        }
        if self.explore_per_round > self.promote_per_round {
            return Err("spec.explore_per_round: more than promote_per_round".into());
        }
        let t = &self.trust;
        for (n, v) in [
            ("f_trust", t.f_trust),
            ("k_u", t.k_u),
            ("margin", t.margin),
            ("h_max", t.h_max),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(format!("spec.trust.{n}: not finite and non-negative"));
            }
        }
        if let Some(r) = &t.r3 {
            let (l0, l1) = r.l_bounds;
            let (t0, t1) = r.tau_bounds;
            if r.n_per_d == 0 || r.starts == 0 || r.refine == 0 {
                return Err("spec.trust.r3: n_per_d, starts and refine must be at least 1".into());
            }
            if !(l0 > 0.0 && l1 > l0 && l1.is_finite() && t0 > 0.0 && t1 > t0 && t1.is_finite()) {
                return Err("spec.trust.r3: need 0 < lo < hi, finite, for l_bounds and tau_bounds".into());
            }
            if !(0.0..=1.01).contains(&r.coverage_min) {
                return Err("spec.trust.r3.coverage_min: outside [0, 1.01]".into());
            }
        }
        match &self.candidates {
            Candidates::Lhs { n, .. } if *n == 0 => return Err("spec.candidates: Lhs with n 0".into()),
            Candidates::List(v) if v.is_empty() => return Err("spec.candidates: empty list".into()),
            Candidates::List(v) => {
                for (i, x) in v.iter().enumerate() {
                    if x.len() != self.params.len() {
                        return Err(format!(
                            "spec.candidates[{i}]: {} values, {} parameters",
                            x.len(),
                            self.params.len()
                        ));
                    }
                    for (p, &xv) in self.params.iter().zip(x) {
                        if !(xv.is_finite() && p.lo <= xv && xv <= p.hi) {
                            return Err(format!(
                                "spec.candidates[{i}]: '{}' is not finite or outside [{}, {}]",
                                p.name, p.lo, p.hi
                            ));
                        }
                    }
                }
            }
            Candidates::Lhs { .. } => {}
        }
        if let Some(p) = &self.privacy {
            if p.id.trim().is_empty() {
                return Err("spec.privacy.id: empty".into());
            }
            if p.n_pub < 1 || p.block < 1 || !(1..=17).contains(&p.digits) {
                return Err("spec.privacy: need n_pub >= 1, block >= 1 and digits in 1..=17".into());
            }
        }
        Ok(())
    }

    /// The candidates as raw parameter vectors (a `List` as given; `Lhs` scaled to the box as `lo + u (hi - lo)`).
    pub fn candidate_points(&self) -> Vec<Vec<f64>> {
        match &self.candidates {
            Candidates::List(v) => v.clone(),
            Candidates::Lhs { n, seed } => sampling::latin_hypercube(*n, self.params.len(), *seed)
                .into_iter()
                .map(|u| {
                    u.iter()
                        .zip(&self.params)
                        .map(|(&u, p)| p.lo + u * (p.hi - p.lo))
                        .collect()
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------- the discrepancy model

/// A rung of the discrepancy ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rung {
    /// No correction.
    R0,
    /// `y_L + b`.
    R1Add,
    /// `a y_L`.
    R1Mul,
    /// `rho y_L + b0 + c . x`.
    R2,
    /// R2 plus a zero-mean Gaussian process on the R2 residual (ADR 0652; opt-in).
    R3,
}

impl Rung {
    /// Name.
    pub fn as_str(self) -> &'static str {
        match self {
            Rung::R0 => "R0",
            Rung::R1Add => "R1add",
            Rung::R1Mul => "R1mul",
            Rung::R2 => "R2",
            Rung::R3 => "R3",
        }
    }

    fn parse(s: &str) -> Option<Rung> {
        [Rung::R0, Rung::R1Add, Rung::R1Mul, Rung::R2, Rung::R3]
            .into_iter()
            .find(|r| r.as_str() == s)
    }

    /// The rungs of ADR 0622 (the keys of `MetricFit::mse`); R3 is reported in `MetricFit::r3`.
    const ALL: [Rung; 4] = [Rung::R0, Rung::R1Add, Rung::R1Mul, Rung::R2];
}

/// One pair used by a fit: raw parameters and, per corrected metric, the low and high values and the high source's stated
/// standard uncertainty.
#[derive(Debug, Clone, PartialEq)]
pub struct PairData {
    /// Raw parameters.
    pub x: Vec<f64>,
    /// Low values per metric.
    pub low: Vec<f64>,
    /// High values per metric.
    pub high: Vec<f64>,
    /// Stated standard uncertainty of the high values per metric.
    pub u_high: Vec<f64>,
}

/// The fit of one metric.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricFit {
    /// Metric.
    pub metric: String,
    /// Chosen rung.
    pub rung: Rung,
    /// Coefficients of the chosen rung in raw parameters: R1add `[b]`, R1mul `[a]`, R2 `[rho, b0, c_1..c_d]`, R0 `[]`.
    pub coef: Vec<f64>,
    /// Whether the chosen rung is applied inside the calibrated region.
    pub trusted: bool,
    /// Why the chosen rung is not trusted (empty when trusted).
    pub why: String,
    /// The additive constant (R1add), applied outside the region when trusted.
    pub constant: f64,
    /// Whether the constant is trusted.
    pub constant_trusted: bool,
    /// Number of pairs.
    pub n: usize,
    /// Mean squared (leave-one-out) error per rung, `None` when the rung was not available.
    pub mse: [Option<f64>; 4],
    /// RMS of `high - low`.
    pub rmse0: f64,
    /// RMS stated uncertainty of the high values.
    pub u_rms: f64,
    /// Rung R3 (ADR 0652), `None` when `TrustRule::r3` is off.
    pub r3: Option<R3Fit>,
}

impl MetricFit {
    /// Mean squared (leave-one-out) error of the chosen rung.
    pub fn chosen_mse(&self) -> Option<f64> {
        match self.rung {
            Rung::R3 => self.r3.as_ref().map(|r| r.mse),
            r => self.mse[Rung::ALL.iter().position(|&x| x == r).unwrap_or(0)],
        }
    }
}

/// The R3 part of a metric's fit (ADR 0652). Process inputs are the region's points (the pairs' unit-box points, in pair
/// order); the R2 coefficients are the fit's `coef` when R3 is chosen.
#[derive(Debug, Clone, PartialEq)]
pub struct R3Fit {
    /// Whether R3 was available.
    pub available: bool,
    /// Why not (empty when available).
    pub why: String,
    /// Length scales (unit box).
    pub l: Vec<f64>,
    /// Process variance (output units squared).
    pub sf2: f64,
    /// Jitter relative to the process variance.
    pub jitter: f64,
    /// `C^-1 r`, `r` the in-sample R2 residual (output units).
    pub beta: Vec<f64>,
    /// Mean squared leave-one-out (or, with `in_sample`, in-sample) error.
    pub mse: f64,
    /// Its standard error.
    pub se: f64,
    /// Fraction of leave-one-out residuals within two predictive standard deviations.
    pub coverage: f64,
    /// Zero-mean log marginal likelihood of the scaled residual (without the constant).
    pub loglik: f64,
    /// Raw R2 coefficients `[rho, b0, c_1..c_d]`.
    pub r2: Vec<f64>,
}

/// The calibrated region (unit box).
#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    /// The pairs' points in the unit box.
    pub points: Vec<Vec<f64>>,
    /// Growth of the bounding box.
    pub margin: f64,
    /// Largest distance to the nearest point.
    pub h_max: f64,
}

impl Region {
    /// Whether a unit-box point lies inside.
    pub fn contains(&self, u: &[f64]) -> bool {
        if self.points.is_empty() {
            return false;
        }
        for (j, &uj) in u.iter().enumerate() {
            let lo = self.points.iter().map(|p| p[j]).fold(f64::INFINITY, f64::min) - self.margin;
            let hi = self.points.iter().map(|p| p[j]).fold(f64::NEG_INFINITY, f64::max) + self.margin;
            if !(lo <= uj && uj <= hi) {
                return false;
            }
        }
        self.points.iter().map(|p| dist(p, u)).fold(f64::INFINITY, f64::min) <= self.h_max
    }
}

fn dist(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt()
}

/// A fitted correction (ADR 0622 D4, D5).
#[derive(Debug, Clone, PartialEq)]
pub struct Correction {
    /// Low model id and version.
    pub low: (String, String),
    /// High source id and version.
    pub high: (String, String),
    /// Parameter boxes (the scaling to the unit box).
    pub params: Vec<ParamBox>,
    /// Calibrated region.
    pub region: Region,
    /// Per corrected metric.
    pub fits: Vec<MetricFit>,
}

fn unit_of(params: &[ParamBox], x: &[f64]) -> Vec<f64> {
    params.iter().zip(x).map(|(p, &v)| (v - p.lo) / (p.hi - p.lo)).collect()
}

/// Householder least squares of `a` (rows) against `y`: coefficients, in-sample residuals and leverages; `None` when the
/// matrix is rank deficient (`|R_jj| <= 1e-12 max |R_kk|`) or a leverage reaches `1 - 1e-12`.
fn lstsq(a: &[Vec<f64>], y: &[f64]) -> Option<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let n = a.len();
    let p = a.first()?.len();
    if n < p {
        return None;
    }
    let mut cols: Vec<Vec<f64>> = (0..p).map(|j| a.iter().map(|r| r[j]).collect()).collect();
    let mut qy = y.to_vec();
    let mut vs: Vec<Vec<f64>> = Vec::with_capacity(p);
    let mut rdiag = vec![0.0; p];
    for k in 0..p {
        let norm = cols[k][k..].iter().map(|v| v * v).sum::<f64>().sqrt();
        let alpha = if cols[k][k] > 0.0 { -norm } else { norm };
        let mut v: Vec<f64> = cols[k][k..].to_vec();
        v[0] -= alpha;
        let vn = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if vn > 0.0 {
            for x in &mut v {
                *x /= vn;
            }
            let reflect = |c: &mut [f64]| {
                let s: f64 = c.iter().zip(&v).map(|(a, b)| a * b).sum();
                for (ci, vi) in c.iter_mut().zip(&v) {
                    *ci -= 2.0 * s * vi;
                }
            };
            for c in cols.iter_mut().skip(k) {
                reflect(&mut c[k..]);
            }
            reflect(&mut qy[k..]);
        }
        rdiag[k] = cols[k][k];
        vs.push(v);
    }
    let rmax = rdiag.iter().fold(0.0f64, |m, r| m.max(r.abs()));
    if rmax == 0.0 || rdiag.iter().any(|r| r.abs() <= 1e-12 * rmax) {
        return None;
    }
    let mut c = vec![0.0; p];
    for k in (0..p).rev() {
        let s: f64 = (k + 1..p).map(|j| cols[j][k] * c[j]).sum();
        c[k] = (qy[k] - s) / cols[k][k];
    }
    let r: Vec<f64> = a
        .iter()
        .zip(y)
        .map(|(row, &yi)| yi - row.iter().zip(&c).map(|(x, b)| x * b).sum::<f64>())
        .collect();
    let mut h = vec![0.0; n];
    for j in 0..p {
        let mut e = vec![0.0; n];
        e[j] = 1.0;
        for k in (0..p).rev() {
            let v = &vs[k];
            let s: f64 = e[k..].iter().zip(v).map(|(a, b)| a * b).sum();
            for (ei, vi) in e[k..].iter_mut().zip(v) {
                *ei -= 2.0 * s * vi;
            }
        }
        for (hi, ei) in h.iter_mut().zip(&e) {
            *hi += ei * ei;
        }
    }
    if h.iter().any(|&x| x >= 1.0 - 1e-12) {
        return None;
    }
    Some((c, r, h))
}

/// Residuals of one rung on the pairs of metric `m`: (leave-one-out, in-sample, coefficients in the unit box).
fn rung_residuals(rung: Rung, u: &[Vec<f64>], yl: &[f64], yh: &[f64]) -> Option<(Vec<f64>, Vec<f64>, Vec<f64>)> {
    let n = yl.len();
    let d = u.first().map_or(0, Vec::len);
    let nf = n as f64;
    match rung {
        Rung::R0 => {
            let e: Vec<f64> = yh.iter().zip(yl).map(|(h, l)| h - l).collect();
            Some((e.clone(), e, Vec::new()))
        }
        Rung::R1Add => {
            if n < 3 {
                return None;
            }
            let b = yh.iter().zip(yl).map(|(h, l)| h - l).sum::<f64>() / nf;
            let r: Vec<f64> = yh.iter().zip(yl).map(|(h, l)| h - l - b).collect();
            let e = r.iter().map(|x| x * nf / (nf - 1.0)).collect();
            Some((e, r, vec![b]))
        }
        Rung::R1Mul => {
            let s2: f64 = yl.iter().map(|x| x * x).sum();
            if n < 3 || s2 <= 0.0 {
                return None;
            }
            let a = yl.iter().zip(yh).map(|(l, h)| l * h).sum::<f64>() / s2;
            let r: Vec<f64> = yh.iter().zip(yl).map(|(h, l)| h - a * l).collect();
            let mut e = Vec::with_capacity(n);
            for (ri, l) in r.iter().zip(yl) {
                let hi = l * l / s2;
                if hi >= 1.0 - 1e-12 {
                    return None;
                }
                e.push(ri / (1.0 - hi));
            }
            Some((e, r, vec![a]))
        }
        Rung::R2 => {
            if n < d + 4 {
                return None;
            }
            let a: Vec<Vec<f64>> = (0..n)
                .map(|i| {
                    let mut row = vec![yl[i], 1.0];
                    row.extend_from_slice(&u[i]);
                    row
                })
                .collect();
            let (c, r, h) = lstsq(&a, yh)?;
            let e = r.iter().zip(&h).map(|(ri, hi)| ri / (1.0 - hi)).collect();
            Some((e, r, c))
        }
        // R3 has its own path (`r3_core`)
        Rung::R3 => None,
    }
}

fn mean_sq(e: &[f64]) -> f64 {
    e.iter().map(|x| x * x).sum::<f64>() / e.len() as f64
}

fn se_of_sq(e: &[f64]) -> f64 {
    let n = e.len();
    if n < 2 {
        return 0.0;
    }
    let m = mean_sq(e);
    let v = e.iter().map(|x| (x * x - m) * (x * x - m)).sum::<f64>() / (n - 1) as f64;
    v.sqrt() / (n as f64).sqrt()
}

// ---------------------------------------------------------------- rung R3 (ADR 0652)

/// Jitter of R3 relative to the process variance (F-698's constant).
pub const R3_JITTER: f64 = surrogate::JITTER;

/// Inverse from a Cholesky factor, column by column (row-major).
fn chol_inverse(chol: &[f64], n: usize) -> Vec<f64> {
    let mut g = vec![0.0; n * n];
    for k in 0..n {
        let mut e = vec![0.0; n];
        e[k] = 1.0;
        let col = surrogate::solve_upper_t(chol, n, &surrogate::solve_lower(chol, n, &e));
        for (i, v) in col.into_iter().enumerate() {
            g[i * n + k] = v;
        }
    }
    g
}

/// Value and gradient of R3's zero-mean log marginal likelihood (without its constant) in scaled units: unit-box inputs
/// `u`, residual `r`, stated standard uncertainties `sigma` (same scale), `theta = (ln l_1, .., ln l_d, ln tau)`
/// (ADR 0652 D2; check K1); F-698's zero-mean mode (ADR 0663). `None` when the factorisation fails or the value is not
/// finite.
pub fn r3_log_likelihood(u: &[Vec<f64>], r: &[f64], sigma: &[f64], theta: &[f64]) -> Option<(f64, Vec<f64>)> {
    let sig2: Vec<f64> = sigma.iter().map(|s| s * s).collect();
    let d = theta.len() - 1;
    let l: Vec<f64> = theta[..d].iter().map(|v| v.exp()).collect();
    let gp = Gp::zero_mean_with_hyper(u, r, &sig2, Kernel::SquaredExponential, &l, theta[d].exp()).ok()?;
    let (v, g) = gp.log_likelihood_and_gradient(false);
    v.is_finite().then_some((v, g))
}

/// The search options of R3's process (ADR 0652 D2, ADR 0663 D3): the squared exponential, `tau` as the process variance
/// of the scaled residual, one worker.
fn r3_options(rule: &R3Rule) -> ZeroMeanOptions {
    ZeroMeanOptions {
        kernel: Kernel::SquaredExponential,
        starts: rule.starts,
        refine: rule.refine,
        iters: rule.iters,
        l_bounds: rule.l_bounds,
        sf2_bounds: rule.tau_bounds,
        seed: rule.seed,
        workers: 1,
    }
}

/// R3's leave-one-out result for one metric (ADR 0652 D3), in output units.
#[derive(Debug, Clone, PartialEq)]
pub struct R3Loo {
    /// Leave-one-out residuals `e3` (R2 refitted without the pair, the process with the fit's hyperparameters).
    pub e: Vec<f64>,
    /// Their predictive standard deviations `sqrt(a_i^T C a_i)`, evaluated as `sqrt(1 / G_ii + |L^T q_i|^2)` (ADR 0656).
    pub sd: Vec<f64>,
    /// In-sample residuals of R3, `r - s_f^2 R beta`.
    pub in_sample: Vec<f64>,
    /// In-sample R2 residuals `r`.
    pub r: Vec<f64>,
    /// Scale `sqrt(mean(r^2))`.
    pub s: f64,
    /// Length scales.
    pub l: Vec<f64>,
    /// Process variance.
    pub sf2: f64,
    /// `C^-1 r`.
    pub beta: Vec<f64>,
    /// Log likelihood of the scaled residual at the hyperparameters.
    pub loglik: f64,
    /// R2 coefficients in the unit box `[rho, b0_u, b_1..b_d]`.
    pub r2_unit: Vec<f64>,
}

fn r3_core(
    u: &[Vec<f64>],
    yl: &[f64],
    yh: &[f64],
    sigma: &[f64],
    rule: &R3Rule,
    hyper: Option<(&[f64], f64)>,
) -> Result<R3Loo, String> {
    let n = yl.len();
    let d = u.first().map_or(0, Vec::len);
    let n_min = (rule.n_per_d * d).max(d + 4);
    if n < n_min {
        return Err(format!("{n} pairs; R3 needs at least {n_min}"));
    }
    let x: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let mut row = vec![yl[i], 1.0];
            row.extend_from_slice(&u[i]);
            row
        })
        .collect();
    let (coef, r, h) = lstsq(&x, yh).ok_or("R2 not available")?;
    let s = mean_sq(&r).sqrt();
    if !(s.is_finite() && s > 0.0) {
        return Err("the R2 residual is zero".into());
    }
    // ADR 0652 amendment 1: a residual at the rounding level of least squares is not a discrepancy
    if s <= 1e-10 * mean_sq(yh).sqrt() {
        return Err("the R2 residual is at the rounding level".into());
    }
    let rs: Vec<f64> = r.iter().map(|v| v / s).collect();
    let sig2: Vec<f64> = sigma.iter().map(|v| (v / s) * (v / s)).collect();
    // ADR 0663 D3: the process is F-698's zero-mean mode with the absolute noise `sig2` (scaled units)
    let gp = match hyper {
        Some((l, sf2)) => {
            if l.len() != d || l.iter().any(|v| !(v.is_finite() && *v > 0.0)) || !(sf2.is_finite() && sf2 > 0.0) {
                return Err("hyperparameters not finite and positive".into());
            }
            Gp::zero_mean_with_hyper(u, &rs, &sig2, Kernel::SquaredExponential, l, sf2 / (s * s))
        }
        None => Gp::fit_zero_mean(u, &rs, &sig2, &r3_options(rule)),
    }
    .map_err(|e| e.trim_start_matches("gp: ").to_string())?;
    let (l, tau) = (gp.length_scales().to_vec(), gp.process_variance());
    let chol = gp.cholesky_factor();
    let g = chol_inverse(chol, n);
    let alpha = gp.weights();
    let loglik = gp.log_likelihood();
    // hat matrix, column by column: P e_k = e_k - (residual of regressing e_k on X)
    let mut p = vec![0.0; n * n];
    // ADR 0656: the coefficients `b_k = H x_k` of the same regressions, `H = (X^T X)^-1 = sum_k b_k b_k^T`
    let mut bk = Vec::with_capacity(n);
    for k in 0..n {
        let mut ek = vec![0.0; n];
        ek[k] = 1.0;
        let (b, rk, _) = lstsq(&x, &ek).ok_or("R2 not available")?;
        for i in 0..n {
            p[i * n + k] = ek[i] - rk[i];
        }
        bk.push(b);
    }
    let mut e = Vec::with_capacity(n);
    let mut sd = Vec::with_capacity(n);
    for i in 0..n {
        let gii = g[i * n + i];
        let gp_ii: f64 = (0..n).map(|j| g[i * n + j] * p[j * n + i]).sum();
        let e2 = rs[i] / (1.0 - h[i]);
        e.push(s * (alpha[i] + e2 * gp_ii) / gii);
        // ADR 0656 D1: var(e3_i) = 1 / G_ii + q^T C q, both terms sums of squares (no cancellation of a^T C a):
        // G_ii = |L^-1 e_i|^2; g = -(X^T G e_i) / G_ii; q_j = x_j^T H g + P_ji (x_i^T H g) / (1 - h_ii), q_i = 0
        let mut ei = vec![0.0; n];
        ei[i] = 1.0;
        let v = surrogate::solve_lower(&chol, n, &ei);
        let cond = 1.0 / v.iter().map(|t| t * t).sum::<f64>();
        let gv: Vec<f64> = (0..x[0].len())
            .map(|a| -(0..n).map(|j| x[j][a] * g[j * n + i]).sum::<f64>() / gii)
            .collect();
        let mut hg = vec![0.0; gv.len()];
        for b in &bk {
            let bg: f64 = b.iter().zip(&gv).map(|(s, t)| s * t).sum();
            for (o, s) in hg.iter_mut().zip(b) {
                *o += s * bg;
            }
        }
        let t: Vec<f64> = x
            .iter()
            .map(|row| row.iter().zip(&hg).map(|(s, t)| s * t).sum())
            .collect();
        let ti = t[i] / (1.0 - h[i]);
        let q: Vec<f64> = (0..n)
            .map(|j| if j == i { 0.0 } else { t[j] + p[j * n + i] * ti })
            .collect();
        let refit: f64 = (0..n)
            .map(|k| (k..n).map(|m| chol[m * n + k] * q[m]).sum::<f64>().powi(2))
            .sum();
        sd.push(s * (cond + refit).sqrt());
    }
    let in_sample: Vec<f64> = (0..n)
        .map(|i| {
            let mu: f64 = (0..n)
                .map(|j| {
                    let rij = if i == j {
                        1.0
                    } else {
                        surrogate::correlation(Kernel::SquaredExponential, &u[i], &u[j], &l)
                    };
                    tau * rij * alpha[j]
                })
                .sum();
            s * (rs[i] - mu)
        })
        .collect();
    Ok(R3Loo {
        e,
        sd,
        in_sample,
        r,
        s,
        l,
        sf2: tau * s * s,
        beta: alpha.iter().map(|a| a / s).collect(),
        loglik,
        r2_unit: coef,
    })
}

/// R3's leave-one-out residuals and standard deviations for metric index `m` of `pairs` (ADR 0652 D3); with `hyper`
/// `(l, s_f^2)` given, no search is made.
pub fn r3_loo(
    pairs: &[PairData],
    params: &[ParamBox],
    m: usize,
    rule: &R3Rule,
    hyper: Option<(&[f64], f64)>,
) -> Result<R3Loo, String> {
    let u: Vec<Vec<f64>> = pairs.iter().map(|p| unit_of(params, &p.x)).collect();
    let yl: Vec<f64> = pairs.iter().map(|p| p.low[m]).collect();
    let yh: Vec<f64> = pairs.iter().map(|p| p.high[m]).collect();
    let sg: Vec<f64> = pairs.iter().map(|p| p.u_high[m]).collect();
    r3_core(&u, &yl, &yh, &sg, rule, hyper)
}

/// Posterior mean of R3's process at the unit-box point `u`.
fn r3_mean(f: &R3Fit, points: &[Vec<f64>], u: &[f64]) -> f64 {
    points
        .iter()
        .zip(&f.beta)
        .map(|(p, b)| f.sf2 * surrogate::correlation(Kernel::SquaredExponential, p, u, &f.l) * b)
        .sum()
}

/// Leave-one-out residuals of `rung` for metric index `m` of `pairs` (`None` when the rung is not available).
pub fn loo_residuals(pairs: &[PairData], params: &[ParamBox], m: usize, rung: Rung) -> Option<Vec<f64>> {
    let u: Vec<Vec<f64>> = pairs.iter().map(|p| unit_of(params, &p.x)).collect();
    let yl: Vec<f64> = pairs.iter().map(|p| p.low[m]).collect();
    let yh: Vec<f64> = pairs.iter().map(|p| p.high[m]).collect();
    rung_residuals(rung, &u, &yl, &yh).map(|r| r.0)
}

fn raw_coef(rung: Rung, cu: &[f64], params: &[ParamBox]) -> Vec<f64> {
    match rung {
        Rung::R2 => {
            let mut b0 = cu[1];
            let mut c = Vec::with_capacity(params.len());
            for (p, &bj) in params.iter().zip(&cu[2..]) {
                let w = p.hi - p.lo;
                b0 -= bj * p.lo / w;
                c.push(bj / w);
            }
            let mut out = vec![cu[0], b0];
            out.extend(c);
            out
        }
        _ => cu.to_vec(),
    }
}

fn fit_metric(name: &str, pairs: &[PairData], params: &[ParamBox], m: usize, t: &TrustRule) -> MetricFit {
    let n = pairs.len();
    let u: Vec<Vec<f64>> = pairs.iter().map(|p| unit_of(params, &p.x)).collect();
    let yl: Vec<f64> = pairs.iter().map(|p| p.low[m]).collect();
    let yh: Vec<f64> = pairs.iter().map(|p| p.high[m]).collect();
    let d0: Vec<f64> = yh.iter().zip(&yl).map(|(h, l)| h - l).collect();
    let rmse0 = if n == 0 { 0.0 } else { mean_sq(&d0).sqrt() };
    let u_rms = if n == 0 {
        0.0
    } else {
        (pairs.iter().map(|p| p.u_high[m] * p.u_high[m]).sum::<f64>() / n as f64).sqrt()
    };
    let mut mse = [None; 4];
    let mut se = [0.0; 4];
    let mut coefs: [Vec<f64>; 4] = Default::default();
    if n > 0 {
        for (k, r) in Rung::ALL.into_iter().enumerate() {
            if let Some((loo, ins, c)) = rung_residuals(r, &u, &yl, &yh) {
                let e = if t.in_sample { ins } else { loo };
                mse[k] = Some(mean_sq(&e));
                se[k] = se_of_sq(&e);
                coefs[k] = c;
            }
        }
    }
    // rung R3 (ADR 0652): index 4 of the arrays, level 3
    let mut mse5 = [mse[0], mse[1], mse[2], mse[3], None];
    let mut se5 = [se[0], se[1], se[2], se[3], 0.0];
    let r3 = t.r3.as_ref().map(|rule| {
        let sg: Vec<f64> = pairs.iter().map(|p| p.u_high[m]).collect();
        match r3_core(&u, &yl, &yh, &sg, rule, None) {
            Ok(o) => {
                let e = if t.in_sample { &o.in_sample } else { &o.e };
                let inside = o.e.iter().zip(&o.sd).filter(|(e, s)| e.abs() <= 2.0 * **s).count();
                mse5[4] = Some(mean_sq(e));
                se5[4] = se_of_sq(e);
                R3Fit {
                    available: true,
                    why: String::new(),
                    l: o.l,
                    sf2: o.sf2,
                    jitter: R3_JITTER,
                    beta: o.beta,
                    mse: mean_sq(e),
                    se: se_of_sq(e),
                    coverage: inside as f64 / n as f64,
                    loglik: o.loglik,
                    r2: raw_coef(Rung::R2, &o.r2_unit, params),
                }
            }
            Err(why) => R3Fit {
                available: false,
                why,
                l: Vec::new(),
                sf2: 0.0,
                jitter: R3_JITTER,
                beta: Vec::new(),
                mse: 0.0,
                se: 0.0,
                coverage: 0.0,
                loglik: 0.0,
                r2: Vec::new(),
            },
        }
    });
    let mse = mse5;
    let se = se5;
    // levels: 0 R0, 1 the better R1 form (ties additive), 2 R2
    let lvl1 = match (mse[1], mse[2]) {
        (Some(a), Some(b)) => Some(if b < a { 2 } else { 1 }),
        (Some(_), None) => Some(1),
        (None, Some(_)) => Some(2),
        (None, None) => None,
    };
    let levels: Vec<usize> = [Some(0), lvl1, mse[3].map(|_| 3), mse[4].map(|_| 4)]
        .into_iter()
        .flatten()
        .filter(|&k| mse[k].is_some())
        .collect();
    let chosen = if levels.is_empty() {
        0
    } else {
        let kmin = *levels
            .iter()
            .min_by(|&&a, &&b| {
                mse[a]
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&mse[b].unwrap_or(f64::INFINITY))
            })
            .unwrap_or(&0);
        let bound = mse[kmin].unwrap_or(0.0) + if t.one_se { se[kmin] } else { 0.0 };
        if t.one_se {
            *levels
                .iter()
                .find(|&&k| mse[k].unwrap_or(f64::INFINITY) <= bound)
                .unwrap_or(&kmin)
        } else {
            kmin
        }
    };
    let rung = [Rung::R0, Rung::R1Add, Rung::R1Mul, Rung::R2, Rung::R3][chosen];
    let ok_trust = |k: usize| -> Result<(), String> {
        let m = mse[k].ok_or("not available")?;
        if m.sqrt() > t.f_trust * rmse0 {
            return Err(format!(
                "held-out RMSE {:.3e} above {} of the uncorrected {:.3e}",
                m.sqrt(),
                t.f_trust,
                rmse0
            ));
        }
        if rmse0 <= t.k_u * u_rms {
            return Err(format!(
                "the discrepancy {rmse0:.3e} is not above {} times the stated uncertainty {u_rms:.3e}",
                t.k_u
            ));
        }
        if k == 4 {
            if let (Some(rule), Some(f)) = (&t.r3, &r3) {
                if f.coverage < rule.coverage_min {
                    return Err(format!(
                        "leave-one-out 2-sigma coverage {:.3} below {}",
                        f.coverage, rule.coverage_min
                    ));
                }
            }
        }
        Ok(())
    };
    let (trusted, why) = if n == 0 {
        (false, "no pair".to_string())
    } else if rung == Rung::R0 {
        (false, "R0 chosen".to_string())
    } else {
        match ok_trust(chosen) {
            Ok(()) => (true, String::new()),
            Err(e) => (false, e),
        }
    };
    let constant_trusted = mse[1].is_some() && ok_trust(1).is_ok();
    let constant = coefs[1].first().copied().unwrap_or(0.0);
    let coef = if rung == Rung::R3 {
        raw_coef(Rung::R2, &coefs[3], params)
    } else {
        raw_coef(rung, &coefs[chosen], params)
    };
    MetricFit {
        metric: name.into(),
        rung,
        coef,
        trusted,
        why,
        constant,
        constant_trusted,
        n,
        mse: [mse[0], mse[1], mse[2], mse[3]],
        rmse0,
        u_rms,
        r3,
    }
}

fn apply(f: &MetricFit, x: &[f64], y: f64, points: &[Vec<f64>], u: &[f64]) -> f64 {
    match f.rung {
        Rung::R0 => y,
        Rung::R1Add => y + f.coef[0],
        Rung::R1Mul => f.coef[0] * y,
        Rung::R2 => f.coef[0] * y + f.coef[1] + f.coef[2..].iter().zip(x).map(|(c, v)| c * v).sum::<f64>(),
        Rung::R3 => {
            let r2 = f.coef[0] * y + f.coef[1] + f.coef[2..].iter().zip(x).map(|(c, v)| c * v).sum::<f64>();
            r2 + f.r3.as_ref().map_or(0.0, |g| r3_mean(g, points, u))
        }
    }
}

fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
}

fn f64_of(v: &Value, at: &str) -> Result<f64, String> {
    v.as_f64().ok_or_else(|| format!("correction: {at}: not a number"))
}

fn str_of(v: &Value, k: &str, at: &str) -> Result<String, String> {
    v.get(k)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("correction: {at}.{k}: missing"))
}

fn arr<'a>(v: &'a Value, k: &str, at: &str) -> Result<&'a Vec<Value>, String> {
    v.get(k)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("correction: {at}.{k}: missing"))
}

fn r3_fit_from_json(g: &Value) -> Result<R3Fit, String> {
    let num_of = |k: &str| f64_of(g.get(k).unwrap_or(&Value::Null), &format!("fits.r3.{k}"));
    let list =
        |k: &str| -> Result<Vec<f64>, String> { arr(g, k, "fits.r3")?.iter().map(|x| f64_of(x, "fits.r3")).collect() };
    Ok(R3Fit {
        available: g
            .get("available")
            .and_then(Value::as_bool)
            .ok_or("correction: fits.r3.available: missing")?,
        why: str_of(g, "why", "fits.r3")?,
        l: list("l")?,
        sf2: num_of("sf2")?,
        jitter: num_of("jitter")?,
        beta: list("beta")?,
        mse: num_of("mse")?,
        se: num_of("se")?,
        coverage: num_of("coverage")?,
        loglik: num_of("loglik")?,
        r2: list("r2")?,
    })
}

impl Correction {
    /// Fit every metric on the pairs (ADR 0622 D4, D5).
    pub fn fit(
        pairs: &[PairData],
        params: &[ParamBox],
        metrics: &[String],
        trust: &TrustRule,
        low: (&str, &str),
        high: (&str, &str),
    ) -> Correction {
        Correction {
            low: (low.0.into(), low.1.into()),
            high: (high.0.into(), high.1.into()),
            params: params.to_vec(),
            region: Region {
                points: pairs.iter().map(|p| unit_of(params, &p.x)).collect(),
                margin: trust.margin,
                h_max: trust.h_max,
            },
            fits: metrics
                .iter()
                .enumerate()
                .map(|(m, name)| fit_metric(name, pairs, params, m, trust))
                .collect(),
        }
    }

    /// Corrected values of `y_low` (one per metric) at raw parameters `x`, and per metric whether the query is outside the
    /// calibrated region (`extrapolated`; then only a trusted constant is applied).
    pub fn predict(&self, x: &[f64], y_low: &[f64]) -> (Vec<f64>, Vec<bool>) {
        let u = unit_of(&self.params, x);
        let inside = self.region.contains(&u);
        self.fits
            .iter()
            .zip(y_low)
            .map(|(f, &y)| {
                if inside {
                    (
                        if f.trusted {
                            apply(f, x, y, &self.region.points, &u)
                        } else {
                            y
                        },
                        false,
                    )
                } else {
                    (if f.constant_trusted { y + f.constant } else { y }, true)
                }
            })
            .unzip()
    }

    /// [`Correction::predict`] after checking that the correction was fitted for these evaluator identities (D5.5).
    pub fn predict_for(
        &self,
        low_id: &str,
        low_version: &str,
        high_id: &str,
        high_version: &str,
        x: &[f64],
        y_low: &[f64],
    ) -> Result<(Vec<f64>, Vec<bool>), String> {
        if (low_id, low_version) != (self.low.0.as_str(), self.low.1.as_str()) {
            return Err(format!(
                "correction: fitted for low model {} version {}, given {low_id} version {low_version}",
                self.low.0, self.low.1
            ));
        }
        if (high_id, high_version) != (self.high.0.as_str(), self.high.1.as_str()) {
            return Err(format!(
                "correction: fitted against high source {} version {}, given {high_id} version {high_version}",
                self.high.0, self.high.1
            ));
        }
        if y_low.len() != self.fits.len() || x.len() != self.params.len() {
            return Err(format!(
                "correction: needs {} parameters and {} low values",
                self.params.len(),
                self.fits.len()
            ));
        }
        Ok(self.predict(x, y_low))
    }

    /// JSON form (numbers round-trip bit for bit).
    pub fn to_json(&self) -> Value {
        json!({
            "low": {"id": self.low.0, "version": self.low.1},
            "high": {"id": self.high.0, "version": self.high.1},
            "params": self.params.iter().map(|p| json!({"name": p.name, "unit": p.unit, "lo": num(p.lo), "hi": num(p.hi)})).collect::<Vec<_>>(),
            "region": {
                "points": self.region.points.iter().map(|p| p.iter().map(|&v| num(v)).collect::<Vec<_>>()).collect::<Vec<_>>(),
                "margin": num(self.region.margin), "h_max": num(self.region.h_max),
            },
            "fits": self.fits.iter().map(|f| {
                let mut o = json!({
                    "metric": f.metric, "rung": f.rung.as_str(), "coef": f.coef.iter().map(|&v| num(v)).collect::<Vec<_>>(),
                    "trusted": f.trusted, "why": f.why, "constant": num(f.constant), "constant_trusted": f.constant_trusted,
                    "n": f.n,
                    "mse": Rung::ALL.iter().zip(&f.mse).map(|(r, m)| (r.as_str().to_string(), m.map_or(Value::Null, num))).collect::<Map<String, Value>>(),
                    "rmse0": num(f.rmse0), "u_rms": num(f.u_rms),
                });
                if let Some(g) = &f.r3 {
                    let v = |x: &[f64]| x.iter().map(|&v| num(v)).collect::<Vec<_>>();
                    o["r3"] = json!({
                        "available": g.available, "why": g.why, "l": v(&g.l), "sf2": num(g.sf2), "jitter": num(g.jitter),
                        "beta": v(&g.beta), "mse": num(g.mse), "se": num(g.se), "coverage": num(g.coverage),
                        "loglik": num(g.loglik), "r2": v(&g.r2),
                    });
                }
                o
            }).collect::<Vec<_>>(),
        })
    }

    /// Parse the JSON form.
    pub fn from_json(v: &Value) -> Result<Correction, String> {
        let ident = |k: &str| -> Result<(String, String), String> {
            let o = v.get(k).ok_or_else(|| format!("correction: {k}: missing"))?;
            Ok((str_of(o, "id", k)?, str_of(o, "version", k)?))
        };
        let params = arr(v, "params", "")?
            .iter()
            .map(|p| {
                Ok(ParamBox {
                    name: str_of(p, "name", "params")?,
                    unit: str_of(p, "unit", "params")?,
                    lo: f64_of(p.get("lo").unwrap_or(&Value::Null), "params.lo")?,
                    hi: f64_of(p.get("hi").unwrap_or(&Value::Null), "params.hi")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let reg = v.get("region").ok_or("correction: region: missing")?;
        let points = arr(reg, "points", "region")?
            .iter()
            .map(|p| {
                p.as_array()
                    .ok_or_else(|| "correction: region.points: not a list".to_string())?
                    .iter()
                    .map(|x| f64_of(x, "region.points"))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let fits = arr(v, "fits", "")?
            .iter()
            .map(|f| {
                let rung_s = str_of(f, "rung", "fits")?;
                let mut mse = [None; 4];
                for (k, r) in Rung::ALL.iter().enumerate() {
                    mse[k] = f.get("mse").and_then(|m| m.get(r.as_str())).and_then(Value::as_f64);
                }
                Ok(MetricFit {
                    metric: str_of(f, "metric", "fits")?,
                    rung: Rung::parse(&rung_s).ok_or_else(|| format!("correction: rung '{rung_s}' is unknown"))?,
                    coef: arr(f, "coef", "fits")?
                        .iter()
                        .map(|x| f64_of(x, "fits.coef"))
                        .collect::<Result<_, _>>()?,
                    trusted: f
                        .get("trusted")
                        .and_then(Value::as_bool)
                        .ok_or("correction: fits.trusted: missing")?,
                    why: str_of(f, "why", "fits")?,
                    constant: f64_of(f.get("constant").unwrap_or(&Value::Null), "fits.constant")?,
                    constant_trusted: f
                        .get("constant_trusted")
                        .and_then(Value::as_bool)
                        .ok_or("correction: fits.constant_trusted: missing")?,
                    n: f.get("n")
                        .and_then(Value::as_u64)
                        .ok_or("correction: fits.n: missing")? as usize,
                    mse,
                    rmse0: f64_of(f.get("rmse0").unwrap_or(&Value::Null), "fits.rmse0")?,
                    u_rms: f64_of(f.get("u_rms").unwrap_or(&Value::Null), "fits.u_rms")?,
                    r3: match f.get("r3") {
                        None => None,
                        Some(g) => Some(r3_fit_from_json(g)?),
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Correction {
            low: ident("low")?,
            high: ident("high")?,
            params,
            region: Region {
                points,
                margin: f64_of(reg.get("margin").unwrap_or(&Value::Null), "region.margin")?,
                h_max: f64_of(reg.get("h_max").unwrap_or(&Value::Null), "region.h_max")?,
            },
            fits,
        })
    }

    /// FNV-1a 64 of the JSON text, as `fnv1a64:<16 hex digits>`.
    pub fn hash(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for b in self.to_json().to_string().bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        format!("fnv1a64:{h:016x}")
    }
}

// ---------------------------------------------------------------- the driver

/// Why a candidate was promoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Best corrected objective.
    Exploit,
    /// Farthest from the promoted set.
    Explore,
    /// Random (negative control).
    Random,
}

impl Reason {
    /// Name.
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Exploit => "exploit",
            Reason::Explore => "explore",
            Reason::Random => "random",
        }
    }
}

/// One round.
#[derive(Debug, Clone, PartialEq)]
pub struct Round {
    /// Picks in pick order.
    pub picks: Vec<(usize, Reason)>,
    /// Picks whose high evaluation failed.
    pub failed_high: Vec<usize>,
    /// Predictions used to pick (per candidate, per metric; NaN when the low evaluation failed).
    pub predictions: Vec<Vec<f64>>,
    /// Extrapolation flags of those predictions.
    pub extrapolated: Vec<Vec<bool>>,
    /// The correction fitted at the end of the round.
    pub correction: Option<Correction>,
}

/// One fidelity pair.
#[derive(Debug, Clone, PartialEq)]
pub struct PairRecord {
    /// Candidate index.
    pub candidate: usize,
    /// Round (1-based).
    pub round: usize,
    /// Why it was promoted.
    pub reason: Reason,
    /// Low values per metric.
    pub low: Vec<f64>,
    /// High values per metric.
    pub high: Vec<f64>,
    /// Predictions made before the high result was known.
    pub pred: Vec<f64>,
}

/// What the driver returns. With a private source this holds private content: it is the caller's to keep private.
#[derive(Debug, Clone, PartialEq)]
pub struct FidelityResult {
    /// Candidates (raw parameters).
    pub candidates: Vec<Vec<f64>>,
    /// Low values of the corrected metrics per candidate (None when the low evaluation failed or lacks a metric).
    pub low: Vec<Option<Vec<f64>>>,
    /// Rounds.
    pub rounds: Vec<Round>,
    /// Pairs in ledger order.
    pub pairs: Vec<PairRecord>,
    /// The last correction.
    pub correction: Option<Correction>,
    /// Predictions with the last correction.
    pub final_predictions: Vec<Vec<f64>>,
    /// Their extrapolation flags.
    pub final_extrapolated: Vec<Vec<bool>>,
}

/// An ok promotion: candidate, reason, ledger index of the high entry, high values, stated uncertainties.
type OkPick = (usize, Reason, usize, Vec<f64>, Vec<f64>);

/// Absolute form of `p` with its nearest existing ancestor canonicalised.
fn normalised(p: &Path) -> PathBuf {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| p.to_path_buf(), |d| d.join(p))
    };
    let mut base = abs.as_path();
    let mut rest = Vec::new();
    let canon = loop {
        if let Ok(c) = base.canonicalize() {
            break c;
        }
        match (base.file_name(), base.parent()) {
            (Some(n), Some(q)) => {
                rest.push(n.to_os_string());
                base = q;
            }
            _ => break abs.clone(),
        }
    };
    rest.iter().rev().fold(canon, |q, n| q.join(n))
}

fn round_sig(v: f64, digits: usize) -> f64 {
    if !v.is_finite() {
        return v;
    }
    format!("{:.*e}", digits - 1, v).parse().unwrap_or(v)
}

fn stamp(mut e: Entry, l: &Ledger) -> Entry {
    if e.solver_version.is_empty() {
        e.solver_version = ledger::solver_version();
    }
    if e.recorded_at_ms == 0 {
        e.recorded_at_ms = (l.clock)();
    }
    e
}

/// The value of metric `m` with unit, when finite.
fn metric_of<'a>(o: &'a Outcome, m: &str) -> Option<&'a Quantity> {
    o.metrics.iter().find(|q| q.name == m && q.value.is_finite())
}

/// Run the driver (ADR 0622). `public` is the public ledger; with a private high source the private ledger is
/// `spec.privacy.path` (it takes `public.clock`).
pub fn run(
    spec: &FidelitySpec,
    low: &dyn Model,
    high: &dyn HighSource,
    public: &Ledger,
) -> Result<FidelityResult, String> {
    spec.validate()?;
    if (low.fidelity(), None::<String>) == (high.fidelity(), high.level()) {
        return Err(format!(
            "fidelity: the low model and the high source have the same level ({} without a level)",
            low.fidelity().as_str()
        ));
    }
    if high.private() && spec.privacy.is_none() {
        return Err(format!(
            "fidelity: the high source {} is private; give spec.privacy with a private ledger path outside the repository",
            high.id()
        ));
    }
    let private = match (&spec.privacy, high.private()) {
        (Some(p), true) => {
            let mut l = Ledger::private(&p.path).map_err(|e| format!("fidelity: {e}"))?;
            let dir = normalised(p.path.parent().unwrap_or(Path::new("")));
            let pubp = normalised(public.path());
            if pubp.starts_with(&dir) {
                return Err(format!(
                    "fidelity: the public ledger {} lies inside the private ledger's directory {}; give a public path outside it",
                    public.path().display(),
                    dir.display()
                ));
            }
            l.clock = public.clock;
            Some(l)
        }
        _ => None,
    };
    let full: &Ledger = private.as_ref().unwrap_or(public);
    let mut mirror = full.read()?.entries;
    let names: Vec<String> = spec.metrics.iter().map(|m| m.name.clone()).collect();
    let floors: Vec<f64> = spec.metrics.iter().map(|m| m.floor).collect();
    let obj_k = names.iter().position(|n| *n == spec.objective);
    let cands = spec.candidate_points();
    let units: Vec<Vec<f64>> = cands.iter().map(|x| unit_of(&spec.params, x)).collect();
    let named = |x: &[f64]| -> Vec<Quantity> {
        spec.params
            .iter()
            .zip(x)
            .map(|(p, &v)| Quantity::new(&p.name, v, &p.unit))
            .collect()
    };
    // ---- screening
    let outs: Vec<Outcome> = parallel_map(&cands, spec.workers, &|x: &Vec<f64>| low.evaluate(&named(x)));
    let mut low_idx = Vec::with_capacity(cands.len());
    let mut low_vals: Vec<Option<Vec<f64>>> = Vec::with_capacity(cands.len());
    let mut low_units: Vec<Vec<String>> = Vec::with_capacity(cands.len());
    let mut low_obj: Vec<f64> = Vec::with_capacity(cands.len());
    let mut entries = Vec::with_capacity(cands.len());
    for (i, (x, o)) in cands.iter().zip(&outs).enumerate() {
        let mut o = o.clone();
        let mut e = Entry {
            study: spec.name.clone(),
            variant: format!("{}#{i}", spec.name),
            provenance: json!({"driver": "fidelity", "candidate": i}),
            ..ledger::entry_of(low, named(x), o.clone())
        };
        if let Err(m) = e.validate() {
            o = Outcome {
                verdict: Verdict::NotEvaluated,
                reason: format!("low outcome refused by the ledger: {m}"),
                ..Default::default()
            };
            e = Entry {
                study: spec.name.clone(),
                variant: format!("{}#{i}", spec.name),
                provenance: json!({"driver": "fidelity", "candidate": i}),
                ..ledger::entry_of(low, named(x), o.clone())
            };
        }
        let vals: Option<Vec<&Quantity>> = if o.verdict == Verdict::Ok {
            names.iter().map(|m| metric_of(&o, m)).collect()
        } else {
            None
        };
        let objv = if o.verdict == Verdict::Ok {
            metric_of(&o, &spec.objective).map(|q| q.value)
        } else {
            None
        };
        match (vals, objv) {
            (Some(v), Some(ov)) => {
                low_vals.push(Some(v.iter().map(|q| q.value).collect()));
                low_units.push(v.iter().map(|q| q.unit.clone()).collect());
                low_obj.push(ov);
            }
            _ => {
                low_vals.push(None);
                low_units.push(Vec::new());
                low_obj.push(f64::NAN);
            }
        }
        let e = stamp(e, full);
        low_idx.push(mirror.len() + entries.len());
        entries.push(e);
    }
    if private.is_some() {
        public.append(&entries)?;
    }
    full.append(&entries)?;
    mirror.extend(entries);

    let predict_all = |c: &Option<Correction>| -> (Vec<Vec<f64>>, Vec<Vec<bool>>) {
        cands
            .iter()
            .zip(&low_vals)
            .map(|(x, lv)| match (lv, c) {
                (None, _) => (vec![f64::NAN; names.len()], vec![false; names.len()]),
                (Some(v), None) => (v.clone(), vec![false; names.len()]),
                (Some(v), Some(c)) => c.predict(x, v),
            })
            .unzip()
    };
    let mut correction: Option<Correction> = None;
    let (mut preds, mut flags) = predict_all(&correction);
    let mut promoted: Vec<usize> = Vec::new();
    let mut is_promoted = vec![false; cands.len()];
    let mut pairs: Vec<PairRecord> = Vec::new();
    let mut pair_data: Vec<PairData> = Vec::new();
    let mut rounds: Vec<Round> = Vec::new();
    let mut rng = match spec.promotion {
        Promotion::Random { seed } => Some(Rng::new(seed)),
        Promotion::Rule => None,
    };
    let mut n_failed_total = 0usize;
    let mut last_block = 0usize;
    let available: Vec<bool> = cands
        .iter()
        .zip(&low_vals)
        .map(|(x, lv)| lv.is_some() && high.available(&named(x)))
        .collect();
    for r in 1..=spec.rounds {
        // ---- promotion
        let avail: Vec<usize> = (0..cands.len()).filter(|&i| available[i] && !is_promoted[i]).collect();
        if avail.is_empty() {
            break;
        }
        let score = |i: usize| -> f64 {
            let v = obj_k.map_or(low_obj[i], |k| preds[i][k]);
            match spec.sense {
                Sense::Min => v,
                Sense::Max => -v,
            }
        };
        let mut picks: Vec<(usize, Reason)> = Vec::new();
        if let Some(g) = rng.as_mut() {
            let mut rest = avail.clone();
            while picks.len() < spec.promote_per_round && !rest.is_empty() {
                let k = g.below(rest.len());
                picks.push((rest.remove(k), Reason::Random));
            }
        } else {
            let mut order = avail.clone();
            order.sort_by(|&a, &b| score(a).total_cmp(&score(b)).then(a.cmp(&b)));
            for &i in order.iter().take(spec.promote_per_round - spec.explore_per_round) {
                picks.push((i, Reason::Exploit));
            }
            for _ in 0..spec.explore_per_round {
                let set: Vec<usize> = promoted.iter().copied().chain(picks.iter().map(|p| p.0)).collect();
                let mut best: Option<(usize, f64)> = None;
                for &i in &avail {
                    if picks.iter().any(|p| p.0 == i) {
                        continue;
                    }
                    let dmin = set
                        .iter()
                        .map(|&j| dist(&units[i], &units[j]))
                        .fold(f64::INFINITY, f64::min);
                    if best.is_none_or(|(_, b)| dmin > b) {
                        best = Some((i, dmin));
                    }
                }
                match best {
                    Some((i, _)) => picks.push((i, Reason::Explore)),
                    None => break,
                }
            }
        }
        // ---- high evaluations
        let xs: Vec<usize> = picks.iter().map(|p| p.0).collect();
        let obs: Vec<Observation> = parallel_map(&xs, spec.workers, &|&i: &usize| high.observe(&named(&cands[i])));
        let mut high_entries = Vec::new();
        let mut ok_picks: Vec<OkPick> = Vec::new();
        let mut failed = Vec::new();
        for (&(i, reason), ob) in picks.iter().zip(&obs) {
            let mut o = ob.outcome.clone();
            let mut unc = ob.uncertainty.clone();
            if o.verdict == Verdict::Ok {
                let lacks = names
                    .iter()
                    .zip(&low_units[i])
                    .find(|(m, u)| metric_of(&o, m).is_none_or(|q| q.unit != **u));
                if let Some((m, _)) = lacks {
                    o = Outcome {
                        verdict: Verdict::NotEvaluated,
                        reason: format!("high outcome lacks a finite metric '{m}' in the low model's unit"),
                        ..Default::default()
                    };
                }
            }
            unc.retain(|u| o.verdict == Verdict::Ok && o.metrics.iter().any(|q| q.name == u.name && q.unit == u.unit));
            let mk = |o: Outcome, unc: Vec<Quantity>| Entry {
                study: spec.name.clone(),
                variant: format!("{}#{i}", spec.name),
                params: named(&cands[i]),
                evaluator: high.id().to_string(),
                evaluator_version: high.version().to_string(),
                fidelity: high.fidelity(),
                fidelity_level: high.level(),
                source: high.source(),
                verdict: o.verdict,
                reason: o.reason,
                metrics: o.metrics,
                geometry_hash: o.geometry_hash,
                settings: o.settings,
                uncertainty: unc,
                provenance: json!({"driver": "fidelity", "candidate": i, "round": r, "reason": reason.as_str()}),
                ..Default::default()
            };
            let mut e = mk(o.clone(), unc.clone());
            if let Err(m) = e.validate() {
                let reason = if high.private() {
                    "high outcome refused by the ledger validation".to_string()
                } else {
                    format!("high outcome refused by the ledger: {m}")
                };
                o = Outcome {
                    verdict: Verdict::NotEvaluated,
                    reason,
                    ..Default::default()
                };
                e = mk(o.clone(), Vec::new());
            }
            let e = stamp(e, full);
            let hidx = mirror.len() + high_entries.len();
            if o.verdict == Verdict::Ok {
                let hv: Vec<f64> = names
                    .iter()
                    .map(|m| metric_of(&o, m).map_or(f64::NAN, |q| q.value))
                    .collect();
                let uv: Vec<f64> = names
                    .iter()
                    .map(|m| e.uncertainty.iter().find(|u| u.name == *m).map_or(0.0, |u| u.value))
                    .collect();
                ok_picks.push((i, reason, hidx, hv, uv));
            } else {
                failed.push(i);
            }
            high_entries.push(e);
        }
        full.append(&high_entries)?;
        mirror.extend(high_entries);
        for &(i, _) in &picks {
            promoted.push(i);
            is_promoted[i] = true;
        }
        n_failed_total += failed.len();
        // ---- pairs
        let mut pair_entries = Vec::new();
        let mut pair_refs = Vec::new();
        for (i, reason, hidx, hv, uv) in &ok_picks {
            let pred = preds[*i].clone();
            let mut e = ledger::fidelity_pair(
                &mirror,
                low_idx[*i],
                *hidx,
                &names.iter().map(String::as_str).collect::<Vec<_>>(),
                &floors,
                Some(&pred),
            )
            .map_err(|m| format!("fidelity: round {r}, candidate {i}: {m}"))?;
            e.study = spec.name.clone();
            e.variant = format!("{}#{i}", spec.name);
            e.provenance = json!({"driver": "fidelity", "round": r, "reason": reason.as_str(), "candidate": i});
            let e = stamp(e, full);
            let k = mirror.len() + pair_entries.len();
            pair_refs.push(ledger::entry_ref(k, &e));
            pair_entries.push(e);
            let lv = low_vals[*i].clone().unwrap_or_default();
            pairs.push(PairRecord {
                candidate: *i,
                round: r,
                reason: *reason,
                low: lv.clone(),
                high: hv.clone(),
                pred,
            });
            pair_data.push(PairData {
                x: cands[*i].clone(),
                low: lv,
                high: hv.clone(),
                u_high: uv.clone(),
            });
        }
        full.append(&pair_entries)?;
        mirror.extend(pair_entries);
        // ---- refit
        let round_preds = std::mem::take(&mut preds);
        let round_flags = std::mem::take(&mut flags);
        if spec.correct && !pair_data.is_empty() {
            correction = Some(Correction::fit(
                &pair_data,
                &spec.params,
                &names,
                &spec.trust,
                (low.id(), low.version()),
                (high.id(), high.version()),
            ));
        }
        let (p2, f2) = predict_all(&correction);
        preds = p2;
        flags = f2;
        // ---- summaries
        let n = pair_data.len();
        let mut ms: Vec<Quantity> = vec![
            Quantity::new("n_promoted", promoted.len() as f64, "1"),
            Quantity::new("n_failed_high", n_failed_total as f64, "1"),
        ];
        let mut agg: Vec<(String, f64, String)> = Vec::new();
        let mut rungs = Map::new();
        for (k, m) in names.iter().enumerate() {
            let unit = low_units
                .iter()
                .find(|u| !u.is_empty())
                .map_or("1".to_string(), |u| u[k].clone());
            let fit = correction.as_ref().map(|c| &c.fits[k]);
            let diffs: Vec<f64> = pair_data.iter().map(|p| p.high[k] - p.low[k]).collect();
            let rmse0 = if n == 0 { 0.0 } else { mean_sq(&diffs).sqrt() };
            let rung = fit.map_or(Rung::R0, |f| f.rung);
            let after = fit.and_then(MetricFit::chosen_mse).map_or(rmse0, f64::sqrt);
            let bias = if n == 0 {
                0.0
            } else {
                diffs.iter().sum::<f64>() / n as f64
            };
            let max_rel = pair_data
                .iter()
                .map(|p| ((p.high[k] - p.low[k]) / p.high[k].abs().max(floors[k])).abs())
                .fold(0.0, f64::max);
            let oos = if pairs.is_empty() {
                0.0
            } else {
                (pairs.iter().map(|p| (p.high[k] - p.pred[k]).powi(2)).sum::<f64>() / pairs.len() as f64).sqrt()
            };
            let trusted = fit.is_some_and(|f| f.trusted);
            let n_ext = flags.iter().zip(&low_vals).filter(|(f, l)| l.is_some() && f[k]).count();
            for (s, v, u) in [
                ("n_pairs", n as f64, "1".to_string()),
                ("rmse_before", rmse0, unit.clone()),
                ("rmse_after", after, unit.clone()),
                ("bias", bias, unit.clone()),
                ("max_abs_rel", max_rel, "1".to_string()),
                ("trusted", if trusted { 1.0 } else { 0.0 }, "1".to_string()),
            ] {
                agg.push((format!("{m}.{s}"), v, u));
            }
            ms.extend(agg.iter().skip(agg.len() - 6).map(|(a, v, u)| Quantity::new(a, *v, u)));
            ms.push(Quantity::new(&format!("{m}.n_extrapolated"), n_ext as f64, "1"));
            ms.push(Quantity::new(&format!("{m}.oos_rmse"), oos, &unit));
            rungs.insert(
                m.clone(),
                json!({"rung": rung.as_str(), "why": fit.map_or(if spec.correct { "no pair" } else { "correction off" }, |f| f.why.as_str())}),
            );
        }
        let summary = Entry {
            kind: "fidelity_summary".into(),
            study: spec.name.clone(),
            variant: format!("{}#round{r}", spec.name),
            evaluator: DRIVER_ID.into(),
            evaluator_version: DRIVER_VERSION.into(),
            settings: json!({"round": r, "rungs": rungs, "correction": correction.as_ref().map(Correction::to_json)}),
            metrics: ms,
            verdict: Verdict::Ok,
            fidelity: high.fidelity(),
            fidelity_level: high.level(),
            source: high.source(),
            parents: pair_refs,
            fitted: correction.as_ref().map(|c| Identity {
                id: CORRECTION_ID.into(),
                version: "1".into(),
                hash: c.hash(),
            }),
            provenance: json!({"driver": "fidelity", "round": r, "spec": spec.to_json(true)}),
            ..Default::default()
        };
        let summary = stamp(summary, full);
        full.append(std::slice::from_ref(&summary))?;
        mirror.push(summary);
        if let (Some(_), Some(p)) = (&private, &spec.privacy) {
            let publish = n >= p.n_pub && n / p.block > last_block;
            let mut pm = vec![
                Quantity::new("n_promoted", promoted.len() as f64, "1"),
                Quantity::new("n_failed_high", n_failed_total as f64, "1"),
            ];
            let mut prungs = Map::new();
            if publish {
                last_block = n / p.block;
                for (a, v, u) in &agg {
                    pm.push(Quantity::new(a, round_sig(*v, p.digits), u));
                }
                for (m, v) in &rungs {
                    prungs.insert(m.clone(), v["rung"].clone());
                }
            }
            let ps = Entry {
                kind: "fidelity_summary".into(),
                study: spec.name.clone(),
                variant: format!("{}#round{r}", spec.name),
                evaluator: DRIVER_ID.into(),
                evaluator_version: DRIVER_VERSION.into(),
                settings: json!({"round": r, "private_source": p.id, "public": true, "rungs": prungs}),
                metrics: pm,
                verdict: Verdict::Ok,
                fidelity: low.fidelity(),
                provenance: json!({"driver": "fidelity", "round": r}),
                ..Default::default()
            };
            public.append(&[stamp(ps, public)])?;
        }
        rounds.push(Round {
            picks,
            failed_high: failed,
            predictions: round_preds,
            extrapolated: round_flags,
            correction: correction.clone(),
        });
    }
    Ok(FidelityResult {
        candidates: cands,
        low: low_vals,
        rounds,
        pairs,
        correction,
        final_predictions: preds,
        final_extrapolated: flags,
    })
}
