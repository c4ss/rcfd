//! Design ledger (ADR 0121, part H4 of `docs/fan-project-breakdown.md`): one JSON line per evaluated variant, across studies,
//! evaluators and fidelity tiers, so that the design history survives sessions and every result can be re-run and compared.
//!
//! * Entry: named parameters with units, evaluator id and version, solver version, geometry/mesh hash, settings, metrics with
//!   units, verdict with a located reason, fidelity tier, a clock stamp, seed and free provenance. The position of a valid line
//!   in the file is the entry index (nothing else numbers the entries, so concurrent writers cannot collide).
//! * Crash safety: an entry is serialised to one line and written with one `write_all` on a file opened for append. A kill can
//!   leave at most one torn line at the end. The reader skips (and reports) lines that are not complete ledger entries; the
//!   writer starts with a newline when the file does not end with one, so a torn fragment is never joined to the next entry.
//!   Nothing is ever truncated or rewritten. Single-writer-process assumption: threads of one process share a [`Ledger`] (an
//!   in-process lock); separate handles or processes rely on the append-mode write of one line being atomic (shortcut: no OS file
//!   lock, `File::lock` needs Rust 1.89 > MSRV 1.75; add one when the MSRV moves).
//! * Queries: filter, Pareto membership, best-so-far with its running trace, history of one parameter, CSV/JSON export, and the
//!   re-run check ([`rerun_check`], [`compare_metrics`]).

use super::pareto;
use super::study::Sense;
use serde_json::{json, Map, Value};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Value of the `format` member of every entry line.
pub const FORMAT: &str = "rcfd-ledger-1";

/// `rcfd <package version>` plus ` (<rev>)` when the build set `RCFD_GIT_REV` (a `git archive` build has no git revision).
pub fn solver_version() -> String {
    match option_env!("RCFD_GIT_REV") {
        Some(r) if !r.is_empty() => format!("rcfd {} ({r})", env!("CARGO_PKG_VERSION")),
        _ => format!("rcfd {}", env!("CARGO_PKG_VERSION")),
    }
}

/// Outcome class of an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verdict {
    /// Evaluated, every metric finite.
    Ok,
    /// The evaluation ran and diverged or failed.
    Diverged,
    /// Refused before evaluation by a constraint (geometry, bounds, requirement).
    ConstraintRejected,
    /// Proposed but not evaluated.
    #[default]
    NotEvaluated,
}

impl Verdict {
    /// Text form.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Diverged => "diverged",
            Verdict::ConstraintRejected => "constraint_rejected",
            Verdict::NotEvaluated => "not_evaluated",
        }
    }

    /// Parse the text form.
    pub fn parse(s: &str) -> Option<Verdict> {
        Some(match s {
            "ok" => Verdict::Ok,
            "diverged" => Verdict::Diverged,
            "constraint_rejected" => Verdict::ConstraintRejected,
            "not_evaluated" => Verdict::NotEvaluated,
            _ => return None,
        })
    }
}

/// Fidelity tier of the evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fidelity {
    /// A closed-form test function (verification only).
    Analytic,
    /// Reduced-order model (blade element, correlations, lumped).
    ReducedOrder,
    /// CFD solution.
    Cfd,
}

impl Fidelity {
    /// Text form.
    pub fn as_str(self) -> &'static str {
        match self {
            Fidelity::Analytic => "analytic",
            Fidelity::ReducedOrder => "reduced_order",
            Fidelity::Cfd => "cfd",
        }
    }

    /// Parse the text form.
    pub fn parse(s: &str) -> Option<Fidelity> {
        Some(match s {
            "analytic" => Fidelity::Analytic,
            "reduced_order" => Fidelity::ReducedOrder,
            "cfd" => Fidelity::Cfd,
            _ => return None,
        })
    }
}

/// A named value with a unit (`"1"` for dimensionless).
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    /// Name.
    pub name: String,
    /// Value (a metric of an entry that is not `ok` may be NaN, stored as null).
    pub value: f64,
    /// Unit text, for example `m3/s`, `Pa`, `dB(A)`, `1`.
    pub unit: String,
}

impl Quantity {
    /// Shorthand constructor.
    pub fn new(name: &str, value: f64, unit: &str) -> Quantity {
        Quantity {
            name: name.into(),
            value,
            unit: unit.into(),
        }
    }
}

/// One ledger entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// `evaluation` (one variant) or a summary kind such as `uq_summary`.
    pub kind: String,
    /// Study or campaign name.
    pub study: String,
    /// Variant label.
    pub variant: String,
    /// Parameters in the evaluator's order.
    pub params: Vec<Quantity>,
    /// Evaluator id.
    pub evaluator: String,
    /// Evaluator version.
    pub evaluator_version: String,
    /// Code version (see [`solver_version`]); stamped by [`Ledger::append`] when empty.
    pub solver_version: String,
    /// Geometry or mesh hash (empty when the evaluator has none).
    pub geometry_hash: String,
    /// Settings needed to re-run (case settings, overrides, operating point).
    pub settings: Value,
    /// Metrics.
    pub metrics: Vec<Quantity>,
    /// Verdict.
    pub verdict: Verdict,
    /// Located reason (required unless the verdict is `ok`).
    pub reason: String,
    /// Fidelity tier.
    pub fidelity: Fidelity,
    /// Clock stamp in ms since the Unix epoch; stamped by [`Ledger::append`] when 0.
    pub recorded_at_ms: u64,
    /// Wall time of the evaluation in seconds, when measured.
    pub wall_s: Option<f64>,
    /// Seed of the random stream that produced the variant, when any.
    pub seed: Option<u64>,
    /// Free provenance (driver, sample number, parent entries, notes).
    pub provenance: Value,
}

impl Default for Entry {
    fn default() -> Self {
        Entry {
            kind: "evaluation".into(),
            study: String::new(),
            variant: String::new(),
            params: Vec::new(),
            evaluator: String::new(),
            evaluator_version: String::new(),
            solver_version: String::new(),
            geometry_hash: String::new(),
            settings: Value::Null,
            metrics: Vec::new(),
            verdict: Verdict::NotEvaluated,
            reason: String::new(),
            fidelity: Fidelity::Analytic,
            recorded_at_ms: 0,
            wall_s: None,
            seed: None,
            provenance: Value::Null,
        }
    }
}

fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v).map_or(Value::Null, Value::Number)
}

fn quantities_json(q: &[Quantity]) -> Value {
    Value::Array(
        q.iter()
            .map(|x| json!({"name": x.name, "value": num(x.value), "unit": x.unit}))
            .collect(),
    )
}

pub(crate) fn quantities_from(v: &Value, what: &str) -> Result<Vec<Quantity>, String> {
    let list = v
        .as_array()
        .ok_or_else(|| format!("{what}: expected a list of {{name, value, unit}}"))?;
    list.iter()
        .enumerate()
        .map(|(i, q)| {
            Ok(Quantity {
                name: q
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("{what}[{i}].name: missing"))?
                    .to_string(),
                value: match q.get("value") {
                    None | Some(Value::Null) => f64::NAN,
                    Some(x) => x
                        .as_f64()
                        .ok_or_else(|| format!("{what}[{i}].value: expected a number"))?,
                },
                unit: q
                    .get("unit")
                    .and_then(Value::as_str)
                    .ok_or_else(|| format!("{what}[{i}].unit: missing"))?
                    .to_string(),
            })
        })
        .collect()
}

fn check_quantities(q: &[Quantity], what: &str, finite: bool) -> Result<(), String> {
    for (i, x) in q.iter().enumerate() {
        if x.name.is_empty() {
            return Err(format!("{what}[{i}].name: empty"));
        }
        if x.unit.is_empty() {
            return Err(format!("{what} '{}': empty unit (use \"1\" for dimensionless)", x.name));
        }
        if q[..i].iter().any(|y| y.name == x.name) {
            return Err(format!("{what} '{}': given twice", x.name));
        }
        if finite && !x.value.is_finite() {
            return Err(format!("{what} '{}': value {} is not finite", x.name, x.value));
        }
    }
    Ok(())
}

impl Entry {
    /// Check the entry; located messages.
    pub fn validate(&self) -> Result<(), String> {
        if self.kind.is_empty() {
            return Err("kind: empty".into());
        }
        if self.evaluator.is_empty() || self.evaluator_version.is_empty() {
            return Err("evaluator and evaluator_version must be given".into());
        }
        check_quantities(&self.params, "params", true)?;
        check_quantities(&self.metrics, "metrics", self.verdict == Verdict::Ok)?;
        if self.verdict != Verdict::Ok && self.reason.trim().is_empty() {
            return Err(format!(
                "reason: a '{}' entry needs a located reason",
                self.verdict.as_str()
            ));
        }
        if let Some(w) = self.wall_s {
            if !(w.is_finite() && w >= 0.0) {
                return Err(format!("wall_s: {w} is not a finite non-negative number"));
            }
        }
        Ok(())
    }

    /// JSON form (one ledger line; keys sorted, so the text is deterministic).
    pub fn to_json(&self) -> Value {
        json!({
            "format": FORMAT, "kind": self.kind, "study": self.study, "variant": self.variant,
            "params": quantities_json(&self.params),
            "evaluator": self.evaluator, "evaluator_version": self.evaluator_version,
            "solver_version": self.solver_version, "geometry_hash": self.geometry_hash, "settings": self.settings,
            "metrics": quantities_json(&self.metrics),
            "verdict": self.verdict.as_str(), "reason": self.reason, "fidelity": self.fidelity.as_str(),
            "recorded_at_ms": self.recorded_at_ms, "wall_s": self.wall_s.map(num), "seed": self.seed,
            "provenance": self.provenance,
        })
    }

    /// Parse the JSON form.
    pub fn from_json(v: &Value) -> Result<Entry, String> {
        if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return Err(format!("not a ledger entry (format is not {FORMAT})"));
        }
        let s = |k: &str| -> Result<String, String> {
            v.get(k)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("{k}: missing"))
        };
        let verdict = s("verdict")?;
        let fidelity = s("fidelity")?;
        Ok(Entry {
            kind: s("kind")?,
            study: s("study")?,
            variant: s("variant")?,
            params: quantities_from(v.get("params").unwrap_or(&Value::Null), "params")?,
            evaluator: s("evaluator")?,
            evaluator_version: s("evaluator_version")?,
            solver_version: s("solver_version")?,
            geometry_hash: s("geometry_hash")?,
            settings: v.get("settings").cloned().unwrap_or(Value::Null),
            metrics: quantities_from(v.get("metrics").unwrap_or(&Value::Null), "metrics")?,
            verdict: Verdict::parse(&verdict).ok_or_else(|| format!("verdict: '{verdict}' is unknown"))?,
            reason: s("reason")?,
            fidelity: Fidelity::parse(&fidelity).ok_or_else(|| format!("fidelity: '{fidelity}' is unknown"))?,
            recorded_at_ms: v
                .get("recorded_at_ms")
                .and_then(Value::as_u64)
                .ok_or("recorded_at_ms: missing")?,
            wall_s: v.get("wall_s").and_then(Value::as_f64),
            seed: v.get("seed").and_then(Value::as_u64),
            provenance: v.get("provenance").cloned().unwrap_or(Value::Null),
        })
    }

    /// Parameter value by name.
    pub fn param(&self, name: &str) -> Option<&Quantity> {
        self.params.iter().find(|q| q.name == name)
    }

    /// Metric by name.
    pub fn metric(&self, name: &str) -> Option<&Quantity> {
        self.metrics.iter().find(|q| q.name == name)
    }
}

/// System clock in ms since the Unix epoch.
pub fn system_clock_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A line of the file that is not a ledger entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    /// 1-based line number.
    pub line: usize,
    /// Why it was skipped.
    pub reason: String,
}

/// Contents of a ledger file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Contents {
    /// Entries; the position is the entry index.
    pub entries: Vec<Entry>,
    /// Lines that are not complete entries (a torn line after a kill, foreign text).
    pub skipped: Vec<Skipped>,
}

/// Parse ledger text: blank lines are ignored, every other line is an entry or is reported as skipped.
pub fn parse_text(text: &str) -> Contents {
    let mut out = Contents::default();
    for (n, line) in text.split('\n').enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        let r = serde_json::from_str::<Value>(line)
            .map_err(|e| format!("not JSON ({e}); a torn line"))
            .and_then(|v| Entry::from_json(&v));
        match r {
            Ok(e) => out.entries.push(e),
            Err(reason) => out.skipped.push(Skipped { line: n + 1, reason }),
        }
    }
    out
}

/// A ledger file. Threads may share one `Ledger` (appends are serialised by an internal lock).
#[derive(Debug)]
pub struct Ledger {
    path: PathBuf,
    lock: Mutex<()>,
    /// Clock used to stamp entries whose `recorded_at_ms` is 0 (tests use a fixed clock for byte-identical files).
    pub clock: fn() -> u64,
}

impl Ledger {
    /// Ledger at `path` (the file is created by the first append).
    pub fn new(path: &Path) -> Ledger {
        Ledger {
            path: path.to_path_buf(),
            lock: Mutex::new(()),
            clock: system_clock_ms,
        }
    }

    /// File path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read every entry (a missing file is an empty ledger).
    pub fn read(&self) -> Result<Contents, String> {
        match std::fs::read(&self.path) {
            Ok(b) => Ok(parse_text(&String::from_utf8_lossy(&b))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Contents::default()),
            Err(e) => Err(format!("cannot read {}: {e}", self.path.display())),
        }
    }

    /// Append entries (validated first; none is written when one is invalid). Empty `solver_version` and zero
    /// `recorded_at_ms` are stamped. The batch is one write: a kill loses at most the batch being written, and only the
    /// last line of it can be torn.
    pub fn append(&self, entries: &[Entry]) -> Result<(), String> {
        if entries.is_empty() {
            return Ok(());
        }
        let now = (self.clock)();
        let mut text = String::new();
        for (i, e) in entries.iter().enumerate() {
            e.validate().map_err(|m| format!("entry {i}: {m}"))?;
            let mut e = e.clone();
            if e.solver_version.is_empty() {
                e.solver_version = solver_version();
            }
            if e.recorded_at_ms == 0 {
                e.recorded_at_ms = now;
            }
            text.push_str(&e.to_json().to_string());
            text.push('\n');
        }
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(dir) = self.path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.path)
            .map_err(|e| format!("cannot open {}: {e}", self.path.display()))?;
        if !ends_with_newline(&mut f).map_err(|e| format!("cannot read {}: {e}", self.path.display()))? {
            // a torn line from a killed writer stays a separate (skipped) line
            text.insert(0, '\n');
        }
        f.write_all(text.as_bytes())
            .and_then(|()| f.sync_data())
            .map_err(|e| format!("cannot write {}: {e}", self.path.display()))
    }
}

/// Whether the file is empty or ends with `\n`.
fn ends_with_newline(f: &mut std::fs::File) -> std::io::Result<bool> {
    let len = f.seek(SeekFrom::End(0))?;
    if len == 0 {
        return Ok(true);
    }
    f.seek(SeekFrom::Start(len - 1))?;
    let mut b = [0u8; 1];
    f.read_exact(&mut b)?;
    Ok(b[0] == b'\n')
}

// ---------------------------------------------------------------- queries

/// Entry filter; `None` matches everything.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Filter {
    /// Kind (`evaluation`, `uq_summary`, ...).
    pub kind: Option<String>,
    /// Verdict.
    pub verdict: Option<Verdict>,
    /// Study.
    pub study: Option<String>,
    /// Evaluator id.
    pub evaluator: Option<String>,
    /// Fidelity tier.
    pub fidelity: Option<Fidelity>,
}

impl Filter {
    /// Whether `e` passes.
    pub fn matches(&self, e: &Entry) -> bool {
        self.kind.as_ref().is_none_or(|k| *k == e.kind)
            && self.verdict.is_none_or(|v| v == e.verdict)
            && self.study.as_ref().is_none_or(|s| *s == e.study)
            && self.evaluator.as_ref().is_none_or(|s| *s == e.evaluator)
            && self.fidelity.is_none_or(|f| f == e.fidelity)
    }

    /// Indices of the matching entries.
    pub fn apply(&self, entries: &[Entry]) -> Vec<usize> {
        (0..entries.len()).filter(|&i| self.matches(&entries[i])).collect()
    }
}

/// The comparable subset of `idx` for `metrics`: `ok` evaluation entries that carry every metric, all from one evaluator and
/// tier, each metric in one unit. Returns the subset and how many were left out for lacking a metric.
fn comparable(entries: &[Entry], idx: &[usize], metrics: &[&str]) -> Result<(Vec<usize>, usize), String> {
    let mut keep = Vec::new();
    let mut missing = 0;
    let mut group: Option<(&str, Fidelity)> = None;
    let mut units: Vec<Option<(&str, usize)>> = vec![None; metrics.len()];
    for &i in idx {
        let e = &entries[i];
        if e.verdict != Verdict::Ok || e.kind != "evaluation" {
            continue;
        }
        let ms: Vec<Option<&Quantity>> = metrics.iter().map(|m| e.metric(m)).collect();
        if ms.iter().any(Option::is_none) {
            missing += 1;
            continue;
        }
        match group {
            None => group = Some((&e.evaluator, e.fidelity)),
            Some((ev, fi)) if ev != e.evaluator || fi != e.fidelity => {
                return Err(format!(
                    "entries {} and {i} come from different evaluators or tiers ({ev} {}, {} {}): filter by evaluator and fidelity first",
                    keep.first().copied().unwrap_or(i),
                    fi.as_str(),
                    e.evaluator,
                    e.fidelity.as_str()
                ));
            }
            _ => {}
        }
        for (k, q) in ms.iter().enumerate() {
            let q = q.expect("checked");
            match units[k] {
                None => units[k] = Some((&q.unit, i)),
                Some((u, j)) if u != q.unit => {
                    return Err(format!(
                        "metric '{}' has unit '{u}' in entry {j} and '{}' in entry {i}",
                        metrics[k], q.unit
                    ))
                }
                _ => {}
            }
        }
        keep.push(i);
    }
    Ok((keep, missing))
}

/// Pareto-optimal entries of `idx` for `objectives` (metric, sense) among the comparable subset (see `comparable`).
pub fn pareto_front(entries: &[Entry], idx: &[usize], objectives: &[(String, Sense)]) -> Result<Vec<usize>, String> {
    if objectives.is_empty() {
        return Err("pareto: give at least one objective".into());
    }
    let names: Vec<&str> = objectives.iter().map(|o| o.0.as_str()).collect();
    let (keep, _) = comparable(entries, idx, &names)?;
    let objs: Vec<Vec<f64>> = keep
        .iter()
        .map(|&i| {
            objectives
                .iter()
                .map(|(m, s)| {
                    let v = entries[i].metric(m).expect("comparable").value;
                    if *s == Sense::Min {
                        v
                    } else {
                        -v
                    }
                })
                .collect()
        })
        .collect();
    let viol = vec![0.0; keep.len()];
    Ok(pareto::front(&objs, &viol).into_iter().map(|k| keep[k]).collect())
}

/// Best entry for one metric and the running best `(entry index, best value so far)` after each comparable entry.
pub type BestSoFar = (Option<usize>, Vec<(usize, f64)>);

/// Best-so-far of `metric` over the comparable subset of `idx`, in ledger order.
pub fn best_so_far(entries: &[Entry], idx: &[usize], metric: &str, sense: Sense) -> Result<BestSoFar, String> {
    let (keep, _) = comparable(entries, idx, &[metric])?;
    let mut best: Option<(usize, f64)> = None;
    let mut trace = Vec::new();
    for i in keep {
        let v = entries[i].metric(metric).expect("comparable").value;
        let better = best.is_none_or(|(_, b)| if sense == Sense::Min { v < b } else { v > b });
        if better {
            best = Some((i, v));
        }
        trace.push((i, best.expect("set").1));
    }
    Ok((best.map(|b| b.0), trace))
}

/// History of one parameter over `idx`: (entry index, value, unit, verdict).
pub fn history(entries: &[Entry], idx: &[usize], param: &str) -> Vec<(usize, f64, String, Verdict)> {
    idx.iter()
        .filter_map(|&i| {
            entries[i]
                .param(param)
                .map(|q| (i, q.value, q.unit.clone(), entries[i].verdict))
        })
        .collect()
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV of the entries `idx`: fixed columns, then one column per (parameter, unit) and per (metric, unit) in first-seen order.
/// Numbers are the shortest text that parses back to the same bits.
pub fn to_csv(entries: &[Entry], idx: &[usize]) -> String {
    let mut pcols: Vec<(String, String)> = Vec::new();
    let mut mcols: Vec<(String, String)> = Vec::new();
    for &i in idx {
        for q in &entries[i].params {
            if !pcols.iter().any(|c| c.0 == q.name && c.1 == q.unit) {
                pcols.push((q.name.clone(), q.unit.clone()));
            }
        }
        for q in &entries[i].metrics {
            if !mcols.iter().any(|c| c.0 == q.name && c.1 == q.unit) {
                mcols.push((q.name.clone(), q.unit.clone()));
            }
        }
    }
    let mut head: Vec<String> = [
        "index",
        "kind",
        "study",
        "variant",
        "evaluator",
        "evaluator_version",
        "solver_version",
        "fidelity",
        "geometry_hash",
        "verdict",
        "reason",
        "recorded_at_ms",
        "seed",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    head.extend(pcols.iter().map(|(n, u)| format!("param:{n} [{u}]")));
    head.extend(mcols.iter().map(|(n, u)| format!("metric:{n} [{u}]")));
    let mut out = head.iter().map(|h| csv_cell(h)).collect::<Vec<_>>().join(",") + "\n";
    let val = |qs: &[Quantity], (n, u): &(String, String)| {
        qs.iter()
            .find(|q| q.name == *n && q.unit == *u)
            .map_or(String::new(), |q| {
                if q.value.is_finite() {
                    format!("{:?}", q.value)
                } else {
                    String::new()
                }
            })
    };
    for &i in idx {
        let e = &entries[i];
        let mut row = vec![
            i.to_string(),
            e.kind.clone(),
            e.study.clone(),
            e.variant.clone(),
            e.evaluator.clone(),
            e.evaluator_version.clone(),
            e.solver_version.clone(),
            e.fidelity.as_str().into(),
            e.geometry_hash.clone(),
            e.verdict.as_str().into(),
            e.reason.clone(),
            e.recorded_at_ms.to_string(),
            e.seed.map_or(String::new(), |s| s.to_string()),
        ];
        row.extend(pcols.iter().map(|c| val(&e.params, c)));
        row.extend(mcols.iter().map(|c| val(&e.metrics, c)));
        out += &(row.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(",") + "\n");
    }
    out
}

// ---------------------------------------------------------------- evaluators and the re-run check

/// What an evaluator returns for one variant.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Outcome {
    /// Verdict.
    pub verdict: Verdict,
    /// Located reason when not `ok`.
    pub reason: String,
    /// Metrics with units.
    pub metrics: Vec<Quantity>,
    /// Geometry or mesh hash of the variant (empty when none).
    pub geometry_hash: String,
    /// Settings that reproduce the evaluation.
    pub settings: Value,
}

impl Outcome {
    /// An `ok` outcome.
    pub fn ok(metrics: Vec<Quantity>) -> Outcome {
        Outcome {
            verdict: Verdict::Ok,
            metrics,
            ..Default::default()
        }
    }
}

/// A generic evaluator over named parameters (analytic, reduced-order or CFD).
pub trait Model: Sync {
    /// Evaluator id.
    fn id(&self) -> &str;
    /// Evaluator version (change it whenever the evaluator's results may change).
    fn version(&self) -> &str;
    /// Fidelity tier.
    fn fidelity(&self) -> Fidelity;
    /// Evaluate one variant.
    fn evaluate(&self, params: &[Quantity]) -> Outcome;
}

/// [`Model`] from a closure.
pub struct FnModel<F> {
    /// Evaluator id.
    pub id: String,
    /// Evaluator version.
    pub version: String,
    /// Tier.
    pub fidelity: Fidelity,
    /// The evaluation.
    pub f: F,
}

impl<F: Fn(&[Quantity]) -> Outcome + Sync> Model for FnModel<F> {
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
        (self.f)(params)
    }
}

/// Ledger entry of one evaluation of `model`.
pub fn entry_of(model: &dyn Model, params: Vec<Quantity>, out: Outcome) -> Entry {
    Entry {
        params,
        evaluator: model.id().to_string(),
        evaluator_version: model.version().to_string(),
        fidelity: model.fidelity(),
        verdict: out.verdict,
        reason: out.reason,
        metrics: out.metrics,
        geometry_hash: out.geometry_hash,
        settings: out.settings,
        ..Default::default()
    }
}

/// Result of comparing an entry's metrics with new values.
#[derive(Debug, Clone, PartialEq)]
pub struct RerunReport {
    /// Whether every recorded metric is reproduced within `atol + rtol |recorded|` and the verdict is the same.
    pub pass: bool,
    /// Per metric: name, recorded, new, |new - recorded|, within tolerance.
    pub metrics: Vec<(String, f64, f64, f64, bool)>,
    /// Problems that are not metric differences (verdict, unit, missing metric, evaluator mismatch).
    pub problems: Vec<String>,
}

/// Compare the metrics of `e` with `new` (name, value, unit) and the verdict with `verdict`.
pub fn compare_metrics(e: &Entry, new: &[Quantity], verdict: Verdict, rtol: f64, atol: f64) -> RerunReport {
    let mut problems = Vec::new();
    if verdict != e.verdict {
        problems.push(format!(
            "verdict: recorded '{}', now '{}'",
            e.verdict.as_str(),
            verdict.as_str()
        ));
    }
    let mut metrics = Vec::new();
    for m in &e.metrics {
        match new.iter().find(|q| q.name == m.name) {
            None => problems.push(format!("metric '{}': not in the new result", m.name)),
            Some(q) if q.unit != m.unit => problems.push(format!(
                "metric '{}': recorded unit '{}', new unit '{}'",
                m.name, m.unit, q.unit
            )),
            Some(q) => {
                let d = (q.value - m.value).abs();
                let same_nan = m.value.is_nan() && q.value.is_nan();
                let ok = same_nan || d <= atol + rtol * m.value.abs();
                metrics.push((m.name.clone(), m.value, q.value, d, ok));
            }
        }
    }
    RerunReport {
        pass: problems.is_empty() && metrics.iter().all(|m| m.4),
        metrics,
        problems,
    }
}

/// Re-run entry `e` with `model` (same id and version required) and compare the metrics.
pub fn rerun_check(e: &Entry, model: &dyn Model, rtol: f64, atol: f64) -> RerunReport {
    if model.id() != e.evaluator || model.version() != e.evaluator_version {
        return RerunReport {
            pass: false,
            metrics: Vec::new(),
            problems: vec![format!(
                "evaluator: entry needs {} {}, given {} {}",
                e.evaluator,
                e.evaluator_version,
                model.id(),
                model.version()
            )],
        };
    }
    let out = model.evaluate(&e.params);
    let mut r = compare_metrics(e, &out.metrics, out.verdict, rtol, atol);
    if out.geometry_hash != e.geometry_hash {
        r.problems.push(format!(
            "geometry_hash: recorded '{}', now '{}'",
            e.geometry_hash, out.geometry_hash
        ));
        r.pass = false;
    }
    r
}

/// JSON of an entry with its index (tool output).
pub fn indexed_json(i: usize, e: &Entry) -> Value {
    let mut v = e.to_json();
    if let Some(o) = v.as_object_mut() {
        o.remove("format");
        o.insert("index".into(), json!(i));
    }
    v
}

/// Object form `{name: value}` of quantities (tool convenience).
pub fn values_obj(q: &[Quantity]) -> Value {
    Value::Object(
        q.iter()
            .map(|x| (x.name.clone(), num(x.value)))
            .collect::<Map<String, Value>>(),
    )
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
