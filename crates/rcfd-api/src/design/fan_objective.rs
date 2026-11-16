//! Fan objective layer (ADR 0620, F-697, fan breakdown H1): one stated objective over a set of system curves (impedance
//! points), metric terms and an optional tolerance plan, built as an ordinary [`StudyDef`] so that [`Study`] and its
//! optimisers (Nelder-Mead, CMA-ES, NSGA-II) and the design ledger consume it unchanged. Library only and opt-in: no case key,
//! no service method, no MCP tool; nothing outside this module calls it.
//!
//! * Two levels of aggregation, in this order (ADR 0620 Q3 (a)): a statistic over the tolerance specimens at each impedance
//!   point ([`TolStat`], in the [`TolerantEvaluator`]), then a combination over the points ([`Over`], by `Study` through
//!   `study::Robust`). The sample standard deviation over specimens has the divisor `n - 1`; `Robust::MeanPlusKSigma` over
//!   points keeps the population divisor `n` of ADR 0067.
//! * Common random numbers: every candidate sees the specimens drawn from the plan's one seed, so the objective is a
//!   deterministic function of the candidate (Law and Kelton, cited from memory).
//! * A limit is held at every point (the worst point counts) or on an aggregate over the points; the aggregate is written into
//!   every point's result, so `Study::violation_of` gives exactly its violation.
//! * [`FanRomTarget`] is the first real target: the reduced-order fan of ADR 0119 on a `ComputerFan` (ADR 0118), with tonal
//!   noise proxies and the static-unbalance force. Every number is a stated proxy; no measured fan data exists, so the
//!   objective ranks variants of the model, not real fans.
//! * Ledger: [`FanObjective::ledger_sink`] gives the ADR 0623 study sink with every unit; [`FanObjective::score_entries`] writes
//!   one `objective_score` entry per candidate with the F-700 fields `objective` and `parents` (ADR 0632), and
//!   [`ObjectiveModel`] re-runs one for `ledger::rerun_check`.

use super::fan_rom_eval::{FanRomEvaluator, RomCondition};
use super::ledger::{self, Entry, Fidelity, Identity, Ledger, Model, Outcome, Quantity, Verdict};
use super::store::{CondResult, Store};
use super::study::{
    parallel_map, Cmp, Condition, ConstraintDef, Evaluator, ObjectiveDef, ParamDef, Robust, Sense, Study, StudyDef,
};
use super::study_ledger::{self, LedgerSink};
use rcfd_mesh::fan_design::{ComputerFan, Dist as TolDist, Tolerances};
use rcfd_post::acoustics::{a_weighting_db, spl_db};
use rcfd_post::fan_rom::{FanRom, RomFan, RomModel, SystemCurve};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Metric values by name.
pub type Metrics = BTreeMap<String, f64>;

/// Evaluator id of the objective's score entries and of [`ObjectiveModel`].
pub const OBJECTIVE_ID: &str = "rcfd.fan_objective";
/// Version of the objective layer (change it whenever the layer's results may change).
pub const OBJECTIVE_VERSION: &str = "1";
/// Metric of the failed-specimen count written per point when a plan is set.
pub const SPECIMENS_FAILED: &str = "specimens_failed";

/// One system curve the fan must work against.
#[derive(Debug, Clone, PartialEq)]
pub struct ImpedancePoint {
    /// Label (unique; the study condition's label).
    pub label: String,
    /// System curve `dp = k Q^2 + l Q`.
    pub system: SystemCurve,
    /// Weight, used only by [`Over::Weighted`].
    pub weight: f64,
}

/// Combination over the impedance points (on the minimised quantity).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Over {
    /// Arithmetic mean.
    Mean,
    /// Worst point.
    Worst,
    /// Mean plus `k` population standard deviations.
    MeanPlusKSigma(f64),
    /// Weighted sum with the points' weights.
    Weighted,
}

impl Over {
    /// Name used in metric names (`pts.<name>`) and labels.
    pub fn name(&self) -> String {
        match self {
            Over::Mean => "mean".into(),
            Over::Worst => "worst".into(),
            Over::MeanPlusKSigma(k) => format!("mean+{k}sigma"),
            Over::Weighted => "weighted".into(),
        }
    }

    /// The study's combination for `points`.
    pub fn robust(&self, points: &[ImpedancePoint]) -> Robust {
        match self {
            Over::Mean => Robust::Mean,
            Over::Worst => Robust::Worst,
            Over::MeanPlusKSigma(k) => Robust::MeanPlusKSigma(*k),
            Over::Weighted => Robust::Weighted(points.iter().map(|p| p.weight).collect()),
        }
    }

    fn json(&self) -> Value {
        match self {
            Over::MeanPlusKSigma(k) => json!({"over": "mean_plus_k_sigma", "k": k}),
            o => json!({"over": o.name()}),
        }
    }
}

/// Statistic of a metric over the tolerance specimens at one point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TolStat {
    /// The nominal specimen.
    Nominal,
    /// Sample mean.
    Mean,
    /// Sample standard deviation (divisor `n - 1`).
    Sd,
    /// Mean plus `k` sample standard deviations of the minimised quantity (`mean - k sd` for a maximised metric).
    MeanPlusKSigma(f64),
}

impl TolStat {
    /// Name of the reduced metric of `metric` (`minimise`: a `Min` term or an `Le` limit).
    pub fn reduced_name(&self, metric: &str, minimise: bool) -> String {
        match self {
            TolStat::Nominal => format!("{metric}@nominal"),
            TolStat::Mean => format!("{metric}@mean"),
            TolStat::Sd => format!("{metric}@sd"),
            TolStat::MeanPlusKSigma(k) if minimise => format!("{metric}@mean+{k}sd"),
            TolStat::MeanPlusKSigma(k) => format!("{metric}@mean-{k}sd"),
        }
    }

    fn json(&self) -> Value {
        match self {
            TolStat::Nominal => json!({"stat": "nominal"}),
            TolStat::Mean => json!({"stat": "mean"}),
            TolStat::Sd => json!({"stat": "sd"}),
            TolStat::MeanPlusKSigma(k) => json!({"stat": "mean_plus_k_sd", "k": k}),
        }
    }
}

/// One objective term.
#[derive(Debug, Clone, PartialEq)]
pub struct Term {
    /// Metric of the model.
    pub metric: String,
    /// Direction.
    pub sense: Sense,
    /// Statistic over the specimens.
    pub tol: TolStat,
    /// Combination over the points.
    pub over: Over,
    /// Weight in the study's scalar objective.
    pub weight: f64,
}

impl Term {
    /// Reduced metric name (the study objective's metric).
    pub fn reduced(&self) -> String {
        self.tol.reduced_name(&self.metric, self.sense == Sense::Min)
    }

    /// Label of the term's aggregate (`<reduced>@<over>`), the metric name of score entries.
    pub fn label(&self) -> String {
        format!("{}@{}", self.reduced(), self.over.name())
    }
}

/// A limit.
#[derive(Debug, Clone, PartialEq)]
pub struct Limit {
    /// Metric of the model.
    pub metric: String,
    /// Comparison.
    pub cmp: Cmp,
    /// Limit value.
    pub limit: f64,
    /// Violation normaliser (violation = excess / scale).
    pub scale: f64,
    /// Statistic over the specimens.
    pub tol: TolStat,
    /// `None`: held at every point (the worst counts); `Some`: held on the aggregate over the points.
    pub over: Option<Over>,
}

impl Limit {
    /// Reduced metric name at one point.
    pub fn reduced(&self) -> String {
        self.tol.reduced_name(&self.metric, self.cmp == Cmp::Le)
    }

    /// Name of the study constraint's metric (the reduced name, with `@pts.<over>` for an aggregated limit).
    pub fn study_metric(&self) -> String {
        match &self.over {
            None => self.reduced(),
            Some(o) => format!("{}@pts.{}", self.reduced(), o.name()),
        }
    }
}

/// Tolerance plan: `n` perturbed specimens from one seed for every candidate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TolerancePlan {
    /// Specimens (>= 2).
    pub n: usize,
    /// Seed (the same for every candidate: common random numbers).
    pub seed: u64,
    /// Largest failed fraction of the specimens at a point before the point fails (0: any failed specimen fails it).
    pub max_failed_fraction: f64,
}

/// A design candidate as a set of specimens evaluated at impedance points.
pub trait SpecimenModel: Sync {
    /// One specimen of a candidate.
    type Specimen: Send + Sync;
    /// Evaluator id.
    fn id(&self) -> &str;
    /// Evaluator version.
    fn version(&self) -> &str;
    /// Fidelity tier.
    fn fidelity(&self) -> Fidelity;
    /// Metrics offered, with units.
    fn metrics(&self) -> Vec<(String, String)>;
    /// Record of the model's settings (tolerances, conditions) for the objective record.
    fn describe(&self) -> Value;
    /// The nominal specimen of candidate `x`.
    fn nominal(&self, x: &[f64]) -> Result<Self::Specimen, String>;
    /// The `n` perturbed specimens of `x`, drawn from `seed`.
    fn perturbed(&self, x: &[f64], n: usize, seed: u64) -> Result<Vec<Self::Specimen>, String>;
    /// Metrics (at least `needed`) of a specimen at a point, or the located failure.
    fn evaluate(
        &self,
        spec: &Self::Specimen,
        point: &ImpedancePoint,
        needed: &BTreeSet<String>,
    ) -> Result<Metrics, String>;
}

/// The objective: parameters, impedance points, terms, limits and an optional tolerance plan.
#[derive(Debug, Clone, PartialEq)]
pub struct FanObjective {
    /// Study name.
    pub name: String,
    /// Parameters with their units.
    pub params: Vec<(ParamDef, String)>,
    /// Impedance points.
    pub points: Vec<ImpedancePoint>,
    /// Objective terms.
    pub terms: Vec<Term>,
    /// Limits.
    pub limits: Vec<Limit>,
    /// Tolerance plan.
    pub plan: Option<TolerancePlan>,
    /// Workers (study candidates and the specimen-point pairs of one candidate).
    pub workers: usize,
}

/// Mean and sample standard deviation; a bit-constant sample gives the value and 0 exactly.
pub fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len();
    if n > 0 && v.iter().all(|x| x.to_bits() == v[0].to_bits()) {
        return (v[0], 0.0);
    }
    let m = v.iter().sum::<f64>() / n as f64;
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n as f64 - 1.0);
    (m, var.sqrt())
}

/// Statistic `tol` of the specimen values `v` (`minimise` as in [`TolStat::reduced_name`]); not for `Nominal`.
pub fn tol_stat(tol: TolStat, v: &[f64], minimise: bool) -> f64 {
    let (m, sd) = mean_sd(v);
    match tol {
        TolStat::Nominal | TolStat::Mean => m,
        TolStat::Sd => sd,
        TolStat::MeanPlusKSigma(k) if minimise => m + k * sd,
        TolStat::MeanPlusKSigma(k) => m - k * sd,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// One reduced metric the evaluator computes at every point.
#[derive(Debug, Clone, PartialEq)]
struct Reduced {
    name: String,
    metric: String,
    tol: TolStat,
    minimise: bool,
}

impl FanObjective {
    fn reduced(&self) -> Vec<Reduced> {
        let mut out: Vec<Reduced> = Vec::new();
        let mut push = |metric: &str, tol: TolStat, minimise: bool| {
            let name = tol.reduced_name(metric, minimise);
            if !out.iter().any(|r| r.name == name) {
                out.push(Reduced {
                    name,
                    metric: metric.to_string(),
                    tol,
                    minimise,
                });
            }
        };
        for t in &self.terms {
            push(&t.metric, t.tol, t.sense == Sense::Min);
        }
        for l in &self.limits {
            push(&l.metric, l.tol, l.cmp == Cmp::Le);
        }
        out
    }

    /// Base metrics the model must return.
    pub fn needed(&self) -> BTreeSet<String> {
        self.terms
            .iter()
            .map(|t| t.metric.clone())
            .chain(self.limits.iter().map(|l| l.metric.clone()))
            .collect()
    }

    /// The study definition: one condition per point, one objective per term, one constraint per limit.
    pub fn study_def(&self) -> StudyDef {
        StudyDef {
            name: self.name.clone(),
            params: self.params.iter().map(|(p, _)| p.clone()).collect(),
            conditions: self
                .points
                .iter()
                .map(|p| Condition {
                    label: p.label.clone(),
                    set: vec![
                        ("system_k".to_string(), json!(p.system.k)),
                        ("system_l".to_string(), json!(p.system.l)),
                    ],
                })
                .collect(),
            objectives: self
                .terms
                .iter()
                .map(|t| ObjectiveDef {
                    metric: t.reduced(),
                    sense: t.sense,
                    weight: t.weight,
                    robust: t.over.robust(&self.points),
                })
                .collect(),
            constraints: self
                .limits
                .iter()
                .map(|l| ConstraintDef {
                    metric: l.study_metric(),
                    cmp: l.cmp,
                    limit: l.limit,
                    scale: l.scale,
                })
                .collect(),
            workers: self.workers,
        }
    }

    /// Located check against `model`.
    pub fn validate<M: SpecimenModel>(&self, model: &M) -> Result<(), String> {
        if self.points.is_empty() {
            return Err("points: at least one impedance point is needed".into());
        }
        for (i, p) in self.points.iter().enumerate() {
            if p.label.is_empty() {
                return Err(format!("points[{i}].label: empty label"));
            }
            if self.points[..i].iter().any(|q| q.label == p.label) {
                return Err(format!("point '{}': defined twice", p.label));
            }
            if !(p.system.k.is_finite() && p.system.l.is_finite()) {
                return Err(format!("point '{}': system k and l must be finite", p.label));
            }
            if !(p.weight.is_finite() && p.weight >= 0.0) {
                return Err(format!("point '{}': weight must be finite and >= 0", p.label));
            }
        }
        let uses_weights = self.terms.iter().any(|t| t.over == Over::Weighted)
            || self.limits.iter().any(|l| l.over == Some(Over::Weighted));
        if uses_weights && (self.points.iter().map(|p| p.weight).sum::<f64>() - 1.0).abs() > 1e-9 {
            return Err("points: the weights of a weighted combination must sum to 1".into());
        }
        let offered: BTreeSet<String> = model.metrics().into_iter().map(|(n, _)| n).collect();
        let stats = self
            .terms
            .iter()
            .map(|t| (format!("term '{}'", t.metric), &t.metric, t.tol, Some(t.over)))
            .chain(
                self.limits
                    .iter()
                    .map(|l| (format!("limit '{}'", l.metric), &l.metric, l.tol, l.over)),
            );
        for (what, metric, tol, over) in stats {
            if !offered.contains(metric) {
                return Err(format!(
                    "{what}: the model {} does not offer the metric '{metric}'",
                    model.id()
                ));
            }
            if tol != TolStat::Nominal && self.plan.is_none() {
                return Err(format!(
                    "{what}: a tolerance statistic other than nominal needs a tolerance plan"
                ));
            }
            if let TolStat::MeanPlusKSigma(k) = tol {
                if !(k.is_finite() && k >= 0.0) {
                    return Err(format!("{what}: k must be finite and >= 0"));
                }
            }
            if let Some(Over::MeanPlusKSigma(k)) = over {
                if !(k.is_finite() && k >= 0.0) {
                    return Err(format!("{what}: k over the points must be finite and >= 0"));
                }
            }
        }
        if let Some(p) = &self.plan {
            if p.n < 2 {
                return Err(format!("plan: n must be >= 2 (got {})", p.n));
            }
            if !(p.max_failed_fraction.is_finite() && (0.0..1.0).contains(&p.max_failed_fraction)) {
                return Err("plan: max_failed_fraction must lie in [0, 1)".into());
            }
        }
        for (p, unit) in &self.params {
            if unit.is_empty() {
                return Err(format!("parameter '{}': no unit (use \"1\" for dimensionless)", p.name));
            }
        }
        self.study_def().validate()
    }

    /// Unit of a reduced, aggregated or base metric name.
    fn unit_of<M: SpecimenModel>(&self, model: &M, metric: &str) -> String {
        model
            .metrics()
            .into_iter()
            .find(|(n, _)| n == metric)
            .map_or_else(|| "1".into(), |(_, u)| u)
    }

    /// Record of the objective (sorted keys).
    pub fn to_json<M: SpecimenModel>(&self, model: &M) -> Value {
        json!({
            "format": "rcfd-fan-objective-1",
            "layer": {"id": OBJECTIVE_ID, "version": OBJECTIVE_VERSION},
            "name": self.name,
            "model": {"id": model.id(), "version": model.version(), "fidelity": model.fidelity().as_str(),
                      "settings": model.describe()},
            "params": self.params.iter().map(|(p, u)| json!({"name": p.name, "lo": p.lo, "hi": p.hi, "unit": u})).collect::<Vec<_>>(),
            "points": self.points.iter().map(|p| json!({"label": p.label, "k": p.system.k, "l": p.system.l, "weight": p.weight})).collect::<Vec<_>>(),
            "terms": self.terms.iter().map(|t| json!({"metric": t.metric, "sense": if t.sense == Sense::Min {"min"} else {"max"},
                "tol": t.tol.json(), "over": t.over.json(), "weight": t.weight, "name": t.label()})).collect::<Vec<_>>(),
            "limits": self.limits.iter().map(|l| json!({"metric": l.metric, "cmp": if l.cmp == Cmp::Le {"le"} else {"ge"},
                "limit": l.limit, "scale": l.scale, "tol": l.tol.json(), "over": l.over.map(|o| o.json()),
                "name": l.study_metric()})).collect::<Vec<_>>(),
            "plan": self.plan.map(|p| json!({"n": p.n, "seed": p.seed, "max_failed_fraction": p.max_failed_fraction})),
        })
    }

    /// Identity of the objective (F-700 `objective` field): id, version and the FNV-1a 64 hash of [`Self::to_json`].
    pub fn identity<M: SpecimenModel>(&self, model: &M) -> Identity {
        Identity {
            id: OBJECTIVE_ID.into(),
            version: OBJECTIVE_VERSION.into(),
            hash: format!("fnv1a64:{:016x}", fnv1a64(self.to_json(model).to_string().as_bytes())),
        }
    }

    /// The ADR 0623 study sink with a unit for every parameter and every metric the evaluator writes.
    pub fn ledger_sink<'l, M: SpecimenModel>(&self, model: &M, ledger: &'l Ledger) -> LedgerSink<'l> {
        let mut metric_units = BTreeMap::new();
        for r in self.reduced() {
            metric_units.insert(r.name.clone(), self.unit_of(model, &r.metric));
        }
        for l in &self.limits {
            metric_units.insert(l.study_metric(), self.unit_of(model, &l.metric));
        }
        if self.plan.is_some() {
            metric_units.insert(SPECIMENS_FAILED.into(), "1".into());
        }
        LedgerSink {
            ledger,
            evaluator: model.id().to_string(),
            evaluator_version: format!("{}; {OBJECTIVE_ID} {OBJECTIVE_VERSION}", model.version()),
            fidelity: model.fidelity(),
            param_units: self.params.iter().map(|(p, u)| (p.name.clone(), u.clone())).collect(),
            metric_units,
        }
    }

    /// The outcome of one candidate from its study record's objective vector and violation (`None`: failed record).
    fn outcome_of<M: SpecimenModel>(
        &self,
        model: &M,
        objective: Option<Vec<f64>>,
        violation: f64,
        message: &str,
    ) -> Outcome {
        let settings = json!({"objective": self.to_json(model)});
        match objective {
            Some(o) => Outcome {
                metrics: self
                    .terms
                    .iter()
                    .zip(o)
                    .map(|(t, v)| {
                        let v = if t.sense == Sense::Max { -v } else { v };
                        Quantity::new(&t.label(), v, &self.unit_of(model, &t.metric))
                    })
                    .chain(std::iter::once(Quantity::new("violation", violation, "1")))
                    .collect(),
                settings,
                ..Outcome::ok(Vec::new())
            },
            None => Outcome {
                verdict: Verdict::Diverged,
                reason: message.to_string(),
                settings,
                ..Default::default()
            },
        }
    }

    /// One `objective_score` entry per store record of `study` (record order): `objective` set, `parents` the references
    /// with hashes to the record's point entries among `entries` (a ledger read), the term aggregates and `violation` as
    /// metrics. A record whose point entries are not all in `entries` is refused, located.
    pub fn score_entries<M: SpecimenModel>(
        &self,
        model: &M,
        study: &Study,
        entries: &[Entry],
    ) -> Result<Vec<Entry>, String> {
        let def = &study.def;
        let om = ObjectiveModel { objective: self, model };
        let pairs: Vec<Option<(String, String)>> =
            entries.iter().map(|e| study_ledger::pair_of(e, &def.name)).collect();
        let units: BTreeMap<&str, &str> = self.params.iter().map(|(p, u)| (p.name.as_str(), u.as_str())).collect();
        let mut out = Vec::new();
        for rec in study.store.records() {
            let mut parents = Vec::new();
            for c in &def.conditions {
                let i = pairs
                    .iter()
                    .position(|p| p.as_ref().is_some_and(|(k, l)| *k == rec.key && *l == c.label))
                    .ok_or_else(|| {
                        format!(
                            "store record {}: no ledger entry of point '{}' (run the study with the ledger sink)",
                            rec.index, c.label
                        )
                    })?;
                parents.push(ledger::entry_ref(i, &entries[i]));
            }
            let x = study.x_of(rec);
            let params: Vec<Quantity> = def
                .params
                .iter()
                .zip(&x)
                .map(|(p, &v)| Quantity::new(&p.name, v, units.get(p.name.as_str()).copied().unwrap_or("1")))
                .collect();
            let out_c = self.outcome_of(
                model,
                study.objective_vector(rec),
                study.violation_of(rec),
                &rec.message,
            );
            let mut e = ledger::entry_of(&om, params, out_c);
            e.kind = "objective_score".into();
            e.study = def.name.clone();
            e.variant = format!("{}#{}", def.name, rec.index);
            e.objective = Some(self.identity(model));
            e.parents = parents;
            e.provenance = json!({"driver": "fan_objective", "store_index": rec.index, "store_key": rec.key});
            out.push(e);
        }
        Ok(out)
    }

    /// Read `ledger`, build [`Self::score_entries`] and append them; returns how many.
    pub fn append_scores<M: SpecimenModel>(&self, model: &M, study: &Study, ledger: &Ledger) -> Result<usize, String> {
        let entries = ledger.read()?.entries;
        let s = self.score_entries(model, study, &entries)?;
        ledger.append(&s)?;
        Ok(s.len())
    }
}

/// Raw results of one candidate: `[specimen][point]`, specimen 0 the nominal, then the perturbed ones in draw order.
pub type SpecimenTable = Vec<Vec<Result<Metrics, String>>>;

/// The objective's evaluator: specimens at every point, reduced per point.
pub struct TolerantEvaluator<'a, M: SpecimenModel> {
    /// The objective.
    pub objective: &'a FanObjective,
    /// The model.
    pub model: &'a M,
}

impl<M: SpecimenModel> TolerantEvaluator<'_, M> {
    /// Raw results of candidate `x` for every (specimen, point) pair; `Err` when the specimens cannot be built.
    pub fn specimen_table(&self, x: &[f64]) -> Result<SpecimenTable, String> {
        let o = self.objective;
        let mut specs = vec![self.model.nominal(x).map_err(|e| format!("nominal: {e}"))?];
        if let Some(p) = &o.plan {
            specs.extend(
                self.model
                    .perturbed(x, p.n, p.seed)
                    .map_err(|e| format!("specimens: {e}"))?,
            );
        }
        let needed = o.needed();
        let np = o.points.len();
        let pairs: Vec<(usize, usize)> = (0..specs.len()).flat_map(|s| (0..np).map(move |q| (s, q))).collect();
        let flat = parallel_map(&pairs, o.workers, &|&(s, q): &(usize, usize)| {
            let r = self.model.evaluate(&specs[s], &o.points[q], &needed)?;
            for m in &needed {
                match r.get(m) {
                    Some(v) if v.is_finite() => {}
                    Some(v) => return Err(format!("metric '{m}' is not finite ({v})")),
                    None => return Err(format!("metric '{m}' was not returned")),
                }
            }
            Ok(r)
        });
        let mut it = flat.into_iter();
        Ok((0..specs.len()).map(|_| it.by_ref().take(np).collect()).collect())
    }

    /// The reduced metrics of every point (or the located failure of the point).
    pub fn reduce(&self, table: &SpecimenTable) -> Vec<Result<Metrics, String>> {
        let o = self.objective;
        let reduced = o.reduced();
        let mut out: Vec<Result<Metrics, String>> = o
            .points
            .iter()
            .enumerate()
            .map(|(q, p)| {
                let nominal = table[0][q]
                    .as_ref()
                    .map_err(|e| format!("point '{}': nominal: {e}", p.label))?;
                let pert = &table[1..];
                let failed: Vec<(usize, &String)> = pert
                    .iter()
                    .enumerate()
                    .filter_map(|(i, row)| row[q].as_ref().err().map(|e| (i, e)))
                    .collect();
                let mut m = Metrics::new();
                if let Some(plan) = &o.plan {
                    if failed.len() as f64 > plan.max_failed_fraction * plan.n as f64 || pert.len() - failed.len() < 2 {
                        let (i, e) = failed[0];
                        return Err(format!(
                            "point '{}': {} of {} specimens failed (first: specimen {i}: {e})",
                            p.label,
                            failed.len(),
                            plan.n
                        ));
                    }
                    m.insert(SPECIMENS_FAILED.into(), failed.len() as f64);
                }
                for r in &reduced {
                    let v = if r.tol == TolStat::Nominal {
                        nominal[&r.metric]
                    } else {
                        let vals: Vec<f64> = pert
                            .iter()
                            .filter_map(|row| row[q].as_ref().ok().map(|mm| mm[&r.metric]))
                            .collect();
                        tol_stat(r.tol, &vals, r.minimise)
                    };
                    m.insert(r.name.clone(), v);
                }
                Ok(m)
            })
            .collect();
        if out.iter().all(Result::is_ok) {
            for l in o.limits.iter().filter(|l| l.over.is_some()) {
                let over = l.over.expect("filtered");
                let red = l.reduced();
                let sign = if l.cmp == Cmp::Le { 1.0 } else { -1.0 };
                let q: Vec<f64> = out.iter().map(|r| sign * r.as_ref().expect("all ok")[&red]).collect();
                let v = sign * over.robust(&o.points).apply(&q);
                for r in out.iter_mut() {
                    r.as_mut().expect("all ok").insert(l.study_metric(), v);
                }
            }
        }
        out
    }
}

impl<M: SpecimenModel> Evaluator for TolerantEvaluator<'_, M> {
    fn evaluate(&self, def: &StudyDef, x: &[f64]) -> Vec<CondResult> {
        let o = self.objective;
        let per_point: Vec<Result<Metrics, String>> = match self.specimen_table(x) {
            Ok(t) => self.reduce(&t),
            Err(e) => o.points.iter().map(|_| Err(e.clone())).collect(),
        };
        def.conditions
            .iter()
            .map(|c| match o.points.iter().position(|p| p.label == c.label) {
                None => CondResult::failure(&c.label, "failed", format!("point '{}': not in the objective", c.label)),
                Some(q) => match &per_point[q] {
                    Ok(m) => CondResult::success(&c.label, m.clone()),
                    Err(e) => CondResult::failure(&c.label, "failed", e.clone()),
                },
            })
            .collect()
    }
}

/// The objective as a [`Model`] over the named parameters: one candidate, its objective values and violation (the
/// re-run of a score entry).
pub struct ObjectiveModel<'a, M: SpecimenModel> {
    /// The objective.
    pub objective: &'a FanObjective,
    /// The model.
    pub model: &'a M,
}

impl<M: SpecimenModel> Model for ObjectiveModel<'_, M> {
    fn id(&self) -> &str {
        OBJECTIVE_ID
    }
    fn version(&self) -> &str {
        OBJECTIVE_VERSION
    }
    fn fidelity(&self) -> Fidelity {
        self.model.fidelity()
    }
    fn evaluate(&self, params: &[Quantity]) -> Outcome {
        let o = self.objective;
        let def = o.study_def();
        let x = match def.vector(&|n| params.iter().find(|q| q.name == n).map(|q| q.value)) {
            Ok(x) => x,
            Err(e) => {
                return Outcome {
                    verdict: Verdict::NotEvaluated,
                    reason: e,
                    ..Default::default()
                }
            }
        };
        let ev = TolerantEvaluator {
            objective: o,
            model: self.model,
        };
        let mut s = match Study::new(def, Store::memory(OBJECTIVE_ID), &ev) {
            Ok(s) => s,
            Err(e) => {
                return Outcome {
                    verdict: Verdict::NotEvaluated,
                    reason: e,
                    ..Default::default()
                }
            }
        };
        match s.eval_points(std::slice::from_ref(&x), "rerun") {
            Ok(pos) => {
                let rec = &s.store.records()[pos[0]];
                o.outcome_of(self.model, s.objective_vector(rec), s.violation_of(rec), &rec.message)
            }
            Err(e) => Outcome {
                verdict: Verdict::NotEvaluated,
                reason: format!("{e:?}"),
                ..Default::default()
            },
        }
    }
}

/// Evaluator id of [`StallPreCheck`].
pub const STALL_PRECHECK_ID: &str = "rcfd.fan_objective.stall_precheck";
/// Version of [`StallPreCheck`].
pub const STALL_PRECHECK_VERSION: &str = "1";

/// A stall pre-check of the objective (ADR 0645): the nominal specimen of the candidate at the named impedance points only,
/// with the objective's metrics, through the same [`SpecimenModel::evaluate`] call and the same finiteness test as
/// [`TolerantEvaluator::specimen_table`]. A candidate it fails fails the objective too, with the same located reason
/// (`point '<label>': nominal: ...`); one it passes may still fail at the other points or in its specimens.
pub struct StallPreCheck<'a, M: SpecimenModel> {
    /// The objective.
    pub objective: &'a FanObjective,
    /// The model.
    pub model: &'a M,
    /// Labels of the points checked, in this order (each must be a point of the objective).
    pub points: Vec<String>,
}

impl<M: SpecimenModel> Model for StallPreCheck<'_, M> {
    fn id(&self) -> &str {
        STALL_PRECHECK_ID
    }
    fn version(&self) -> &str {
        STALL_PRECHECK_VERSION
    }
    fn fidelity(&self) -> Fidelity {
        self.model.fidelity()
    }
    fn evaluate(&self, params: &[Quantity]) -> Outcome {
        let o = self.objective;
        let refuse = |reason: String| Outcome {
            verdict: Verdict::NotEvaluated,
            reason,
            ..Default::default()
        };
        let x = match o
            .study_def()
            .vector(&|n| params.iter().find(|q| q.name == n).map(|q| q.value))
        {
            Ok(x) => x,
            Err(e) => return refuse(e),
        };
        let mut pts = Vec::with_capacity(self.points.len());
        for l in &self.points {
            match o.points.iter().find(|p| &p.label == l) {
                Some(p) => pts.push(p),
                None => return refuse(format!("stall pre-check: point '{l}' is not a point of the objective")),
            }
        }
        if pts.is_empty() {
            return refuse("stall pre-check: no point named".into());
        }
        let failed = |label: &str, e: String| Outcome {
            verdict: Verdict::Diverged,
            reason: format!("point '{label}': nominal: {e}"),
            ..Default::default()
        };
        let spec = match self.model.nominal(&x) {
            Ok(s) => s,
            Err(e) => {
                return Outcome {
                    verdict: Verdict::Diverged,
                    reason: format!("nominal: {e}"),
                    ..Default::default()
                }
            }
        };
        let needed = o.needed();
        let mut metrics = Vec::new();
        for p in pts {
            let r = match self.model.evaluate(&spec, p, &needed) {
                Ok(r) => r,
                Err(e) => return failed(&p.label, e),
            };
            for m in &needed {
                match r.get(m) {
                    Some(v) if v.is_finite() => metrics.push(Quantity::new(
                        &format!("{m}@{}", p.label),
                        *v,
                        &o.unit_of(self.model, m),
                    )),
                    Some(v) => return failed(&p.label, format!("metric '{m}' is not finite ({v})")),
                    None => return failed(&p.label, format!("metric '{m}' was not returned")),
                }
            }
        }
        Outcome::ok(metrics)
    }
}

// ---------------------------------------------------------------- the reduced-order fan target

/// Candidate -> fan of the schema of ADR 0118.
pub type ComputerFanGeometry = Box<dyn Fn(&[f64]) -> Result<ComputerFan, String> + Send + Sync>;

/// Metrics of [`FanRomTarget`] with units.
pub const FAN_ROM_TARGET_METRICS: [(&str, &str); 12] = [
    ("q", "m3/s"),
    ("dp_s", "Pa"),
    ("dp_t", "Pa"),
    ("power", "W"),
    ("torque", "N m"),
    ("eta_s", "1"),
    ("eta_t", "1"),
    ("tone1_pa", "Pa"),
    ("tone1_db", "dB"),
    ("tone_a3_db", "dB(A)"),
    ("unbalance", "kg m"),
    ("unbalance_force", "N"),
];

const AERO: [&str; 10] = [
    "q",
    "dp_s",
    "dp_t",
    "power",
    "torque",
    "eta_s",
    "eta_t",
    "tone1_pa",
    "tone1_db",
    "tone_a3_db",
];

/// The reduced-order fan of ADR 0119 on a `ComputerFan` (ADR 0118) as a [`SpecimenModel`]. Aero metrics are those of
/// [`FanRomEvaluator::metrics`] for `RomFan::from_axial_fan(specimen.axial_fan(0, 0, 0))` at the point (the base condition
/// with the point's system curve), plus `tone1_db` (the level of the first blade-passing tone) and `tone_a3_db` (the
/// A-weighted sum of the first three tones), all steady-loading tonal proxies. `unbalance` is `|U|` of
/// `ComputerFan::static_unbalance` (blades only, hub balanced) and `unbalance_force` `|U| Omega^2`, a force on the rotor.
pub struct FanRomTarget {
    /// Candidate -> nominal fan.
    pub geometry: ComputerFanGeometry,
    /// Manufacturing tolerances of the specimens.
    pub tolerances: Tolerances,
    /// Section and loss model.
    pub model: RomModel,
    /// Base condition (its system curve is replaced by each point's).
    pub base: RomCondition,
}

fn tol_json(d: &TolDist) -> Value {
    serde_json::to_value(d).unwrap_or(Value::Null)
}

impl FanRomTarget {
    /// Located refusal of a tolerance the one-row model cannot represent.
    pub fn check_tolerances(&self) -> Result<(), String> {
        let t = &self.tolerances;
        for (name, d) in [
            ("stagger_blade_deg", &t.stagger_blade_deg),
            ("chord_rel", &t.chord_rel),
            ("eccentricity", &t.eccentricity),
            ("roughness_ks", &t.roughness_ks),
        ] {
            if !matches!(d, TolDist::Fixed { value } if *value == 0.0) {
                return Err(format!(
                    "tolerances.{name}: must be Fixed 0 (the one-row reduced-order model cannot represent it)"
                ));
            }
        }
        Ok(())
    }

    /// The first three blade-passing tones A-weighted and summed [dB(A)] at the operating point of `fan` at `c`.
    pub fn tone_a3_db(&self, fan: RomFan, c: &RomCondition) -> Result<f64, String> {
        let rom = FanRom::new(fan, self.model)?;
        let q_hi = rom.area() * c.op.omega() * rom.fan.r_tip;
        let (_, p) = rom.operating_point(&c.op, &c.system, 1e-3 * q_hi, q_hi, c.n_scan)?;
        let mut sum = 0.0;
        for m in 1..=3usize {
            let pm = rom.tone(&c.op, &p, c.c0, m, c.observer_r, c.observer_theta)?;
            let f = m as f64 * rom.fan.n_blades as f64 * c.op.omega() / std::f64::consts::TAU;
            let l = spl_db(pm / std::f64::consts::SQRT_2).map_err(|e| format!("tone {m}: {e}"))?
                + a_weighting_db(f).map_err(|e| format!("tone {m}: {e}"))?;
            sum += 10f64.powf(l / 10.0);
        }
        Ok(10.0 * sum.log10())
    }
}

impl SpecimenModel for FanRomTarget {
    type Specimen = ComputerFan;
    fn id(&self) -> &str {
        "rcfd.fan_objective.rom"
    }
    fn version(&self) -> &str {
        "1 (ADR 0119 model on the ADR 0118 schema, ADR 0620 metrics)"
    }
    fn fidelity(&self) -> Fidelity {
        Fidelity::ReducedOrder
    }
    fn metrics(&self) -> Vec<(String, String)> {
        FAN_ROM_TARGET_METRICS
            .iter()
            .map(|(n, u)| (n.to_string(), u.to_string()))
            .collect()
    }
    fn describe(&self) -> Value {
        let t = &self.tolerances;
        let c = &self.base;
        json!({
            "tolerances": {
                "stagger_common_deg": tol_json(&t.stagger_common_deg), "stagger_blade_deg": tol_json(&t.stagger_blade_deg),
                "chord_rel": tol_json(&t.chord_rel), "thickness_rel": tol_json(&t.thickness_rel),
                "tip_gap_delta": tol_json(&t.tip_gap_delta), "eccentricity": tol_json(&t.eccentricity),
                "mass_rel": tol_json(&t.mass_rel), "roughness_ks": tol_json(&t.roughness_ks),
            },
            "condition": {"rpm": c.op.rpm, "rho": c.op.rho, "nu": c.op.nu, "c0": c.c0, "observer_r": c.observer_r,
                          "observer_theta": c.observer_theta, "n_scan": c.n_scan},
            "model": format!("{:?}", self.model),
        })
    }
    fn nominal(&self, x: &[f64]) -> Result<ComputerFan, String> {
        (self.geometry)(x).map_err(|e| format!("geometry: {e}"))
    }
    fn perturbed(&self, x: &[f64], n: usize, seed: u64) -> Result<Vec<ComputerFan>, String> {
        self.check_tolerances()?;
        let f = self.nominal(x)?;
        f.ensemble(&self.tolerances, n, seed)
            .map_err(|e| e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("; "))
    }
    fn evaluate(
        &self,
        spec: &ComputerFan,
        point: &ImpedancePoint,
        needed: &BTreeSet<String>,
    ) -> Result<Metrics, String> {
        let mut out = Metrics::new();
        if needed.iter().any(|m| AERO.contains(&m.as_str())) {
            let rf = RomFan::from_axial_fan(&spec.axial_fan(0, 0.0, 0.0))?;
            let mut c = self.base;
            c.system = point.system;
            let ev = FanRomEvaluator {
                geometry: Box::new(|_| Err("unused".into())),
                model: self.model,
                base: self.base,
            };
            out = ev.metrics(rf.clone(), &c)?;
            let p1 = out["tone1_pa"];
            out.insert(
                "tone1_db".into(),
                spl_db(p1 / std::f64::consts::SQRT_2).map_err(|e| format!("tone1_db: {e}"))?,
            );
            if needed.contains("tone_a3_db") {
                out.insert("tone_a3_db".into(), self.tone_a3_db(rf, &c)?);
            }
        }
        if needed.contains("unbalance") || needed.contains("unbalance_force") {
            let u = spec
                .static_unbalance()
                .map_err(|e| e.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("; "))?;
            let a = u[0].hypot(u[1]);
            let w = self.base.op.omega();
            out.insert("unbalance".into(), a);
            out.insert("unbalance_force".into(), a * w * w);
        }
        Ok(out)
    }
}
