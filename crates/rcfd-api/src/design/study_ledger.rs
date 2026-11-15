//! Opt-in ledger sink of design studies (ADR 0623 D1): with a [`LedgerSink`] attached by [`Study::set_ledger_sink`], every
//! evaluated (candidate, condition) pair of the study becomes one `evaluation` entry of the design ledger (ADR 0121), failed
//! pairs included with a located reason.
//!
//! * Order: the store is written first, the ledger second (one `Ledger::append` per batch). A kill between the two leaves
//!   store records without entries; [`reconcile`] (run by `set_ledger_sink`) appends exactly those, so a resumed study has
//!   neither missing nor duplicate pairs. Candidates served from the store write nothing.
//! * A pair is identified by `provenance.store_key` and `provenance.condition` among the entries whose `study` is the
//!   definition's name and whose `provenance.driver` is `study`.
//! * Units: studies carry none, so the sink holds a unit for every parameter and metric; a missing one is refused.
//! * [`ConditionModel`] re-runs one condition of a study as a [`Model`], for [`super::ledger::rerun_check`].
//!
//! [`Study::set_ledger_sink`]: super::study::Study::set_ledger_sink

use super::ledger::{Entry, Fidelity, Ledger, Model, Outcome, Quantity, Verdict};
use super::store::{CondResult, Record};
use super::study::{Evaluator, StudyDef};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Value of `provenance.driver` of the entries a study writes.
pub const DRIVER: &str = "study";

/// Where and how a study logs its evaluations.
#[derive(Debug)]
pub struct LedgerSink<'l> {
    /// The ledger.
    pub ledger: &'l Ledger,
    /// Evaluator id of the entries.
    pub evaluator: String,
    /// Evaluator version (change it whenever the evaluator's results may change).
    pub evaluator_version: String,
    /// Fidelity tier.
    pub fidelity: Fidelity,
    /// Unit of every parameter, by name.
    pub param_units: BTreeMap<String, String>,
    /// Unit of every metric the evaluator returns, by name.
    pub metric_units: BTreeMap<String, String>,
}

fn unit_of<'u>(units: &'u BTreeMap<String, String>, name: &str, what: &str) -> Result<&'u str, String> {
    match units.get(name) {
        Some(u) if !u.is_empty() => Ok(u),
        _ => Err(format!(
            "{what} '{name}': no unit in the ledger sink (use \"1\" for dimensionless)"
        )),
    }
}

impl LedgerSink<'_> {
    /// Check the sink against a study definition: ids given, a unit for every parameter and for every metric of an
    /// objective or constraint.
    pub fn check(&self, def: &StudyDef) -> Result<(), String> {
        if self.evaluator.is_empty() || self.evaluator_version.is_empty() {
            return Err("ledger sink: evaluator and evaluator_version must be given".into());
        }
        for p in &def.params {
            unit_of(&self.param_units, &p.name, "parameter")?;
        }
        for o in &def.objectives {
            unit_of(&self.metric_units, &o.metric, "objective metric")?;
        }
        for c in &def.constraints {
            unit_of(&self.metric_units, &c.metric, "constraint metric")?;
        }
        Ok(())
    }
}

/// Verdict of one condition result.
pub fn verdict_of(c: &CondResult) -> Verdict {
    if c.ok {
        Verdict::Ok
    } else {
        match c.status.as_str() {
            "constraint_rejected" => Verdict::ConstraintRejected,
            "not_evaluated" => Verdict::NotEvaluated,
            _ => Verdict::Diverged,
        }
    }
}

/// Located reason of a condition result that is not ok (empty when ok).
pub fn reason_of(c: &CondResult) -> String {
    if c.ok {
        String::new()
    } else {
        format!("condition '{}': {}: {}", c.label, c.status, c.message)
    }
}

/// `settings` of the entries of condition `label`: the label and the condition's overrides.
fn settings_of(def: &StudyDef, label: &str) -> Value {
    let set: Map<String, Value> = def
        .conditions
        .iter()
        .find(|c| c.label == label)
        .map(|c| c.set.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    json!({"condition": label, "set": Value::Object(set)})
}

fn metrics_of(c: &CondResult, units: &BTreeMap<String, String>) -> Result<Vec<Quantity>, String> {
    c.metrics
        .iter()
        .map(|(k, &v)| Ok(Quantity::new(k, v, unit_of(units, k, "metric")?)))
        .collect()
}

fn params_of(def: &StudyDef, sink: &LedgerSink, rec: &Record) -> Result<Vec<Quantity>, String> {
    def.params
        .iter()
        .map(|p| {
            let v = rec
                .params
                .iter()
                .find(|(n, _)| *n == p.name)
                .map(|(_, v)| *v)
                .ok_or_else(|| format!("store record {}: no value of parameter '{}'", rec.index, p.name))?;
            Ok(Quantity::new(
                &p.name,
                v,
                unit_of(&sink.param_units, &p.name, "parameter")?,
            ))
        })
        .collect()
}

/// The entries of one store record: one per condition, in condition order.
pub fn entries_of(def: &StudyDef, sink: &LedgerSink, rec: &Record) -> Result<Vec<Entry>, String> {
    let params = params_of(def, sink, rec)?;
    rec.conditions
        .iter()
        .map(|c| {
            Ok(Entry {
                kind: "evaluation".into(),
                study: def.name.clone(),
                variant: format!("{}#{}@{}", def.name, rec.index, c.label),
                params: params.clone(),
                evaluator: sink.evaluator.clone(),
                evaluator_version: sink.evaluator_version.clone(),
                settings: settings_of(def, &c.label),
                metrics: metrics_of(c, &sink.metric_units)?,
                verdict: verdict_of(c),
                reason: reason_of(c),
                fidelity: sink.fidelity,
                provenance: json!({
                    "driver": DRIVER, "store_index": rec.index, "store_key": rec.key, "condition": c.label,
                    "source": rec.source, "status": c.status, "iterations": c.iterations,
                }),
                ..Default::default()
            })
        })
        .collect()
}

/// The entries of several records, in record order.
pub fn entries_of_records(def: &StudyDef, sink: &LedgerSink, recs: &[Record]) -> Result<Vec<Entry>, String> {
    let mut out = Vec::new();
    for r in recs {
        out.extend(entries_of(def, sink, r)?);
    }
    Ok(out)
}

/// The (store key, condition) pair of an entry the study `study` wrote, if it is one.
pub fn pair_of(e: &Entry, study: &str) -> Option<(String, String)> {
    if e.study != study || e.provenance.get("driver").and_then(Value::as_str) != Some(DRIVER) {
        return None;
    }
    let s = |k: &str| e.provenance.get(k).and_then(Value::as_str).map(str::to_string);
    Some((s("store_key")?, s("condition")?))
}

/// Append the entries of every (record, condition) pair of `records` that the ledger does not hold yet; returns how many.
pub fn reconcile(def: &StudyDef, sink: &LedgerSink, records: &[Record]) -> Result<usize, String> {
    let have: BTreeSet<(String, String)> = sink
        .ledger
        .read()?
        .entries
        .iter()
        .filter_map(|e| pair_of(e, &def.name))
        .collect();
    let mut missing = Vec::new();
    for r in records {
        for e in entries_of(def, sink, r)? {
            let pair = pair_of(&e, &def.name).expect("an entry of this study");
            if !have.contains(&pair) {
                missing.push(e);
            }
        }
    }
    sink.ledger.append(&missing).map_err(|e| format!("ledger: {e}"))?;
    Ok(missing.len())
}

/// One condition of a study as a [`Model`] over its named parameters (for the re-run check of study entries).
pub struct ConditionModel<'m> {
    /// Study definition.
    pub def: &'m StudyDef,
    /// The study's evaluator.
    pub evaluator: &'m dyn Evaluator,
    /// Condition label.
    pub condition: String,
    /// Evaluator id (as in the sink).
    pub id: String,
    /// Evaluator version (as in the sink).
    pub version: String,
    /// Tier.
    pub fidelity: Fidelity,
    /// Metric units (as in the sink).
    pub metric_units: BTreeMap<String, String>,
}

impl<'m> ConditionModel<'m> {
    /// The model of condition `label` with the ids and units of `sink`.
    pub fn of(def: &'m StudyDef, evaluator: &'m dyn Evaluator, sink: &LedgerSink, label: &str) -> ConditionModel<'m> {
        ConditionModel {
            def,
            evaluator,
            condition: label.to_string(),
            id: sink.evaluator.clone(),
            version: sink.evaluator_version.clone(),
            fidelity: sink.fidelity,
            metric_units: sink.metric_units.clone(),
        }
    }
}

impl Model for ConditionModel<'_> {
    fn id(&self) -> &str {
        &self.id
    }
    fn version(&self) -> &str {
        &self.version
    }
    fn fidelity(&self) -> Fidelity {
        self.fidelity
    }
    fn evaluate(&self, params: &[Quantity]) -> Outcome {
        let fail = |reason: String| Outcome {
            verdict: Verdict::NotEvaluated,
            reason,
            ..Default::default()
        };
        let Some(cond) = self.def.conditions.iter().find(|c| c.label == self.condition) else {
            return fail(format!("condition '{}': not in the study", self.condition));
        };
        let x: Option<Vec<f64>> = self
            .def
            .params
            .iter()
            .map(|p| params.iter().find(|q| q.name == p.name).map(|q| q.value))
            .collect();
        let Some(x) = x else {
            return fail("params: a parameter of the study is missing".into());
        };
        let one = StudyDef {
            conditions: vec![cond.clone()],
            ..self.def.clone()
        };
        let mut res = self.evaluator.evaluate(&one, &x);
        if res.len() != 1 {
            return fail("the evaluator returned a wrong number of results".into());
        }
        let mut c = res.remove(0);
        // the rule of `Study::eval_points`
        if c.ok && c.metrics.values().any(|v| !v.is_finite()) {
            c.ok = false;
            c.status = "failed".into();
            c.message = format!("{} non-finite metric", c.message).trim().to_string();
        }
        match metrics_of(&c, &self.metric_units) {
            Ok(metrics) => Outcome {
                verdict: verdict_of(&c),
                reason: reason_of(&c),
                metrics,
                settings: settings_of(self.def, &c.label),
                ..Default::default()
            },
            Err(e) => fail(e),
        }
    }
}

#[cfg(test)]
#[path = "study_ledger_tests.rs"]
mod tests;
