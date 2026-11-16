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
//! * ADR 0632 (F-700): optional top-level keys `parents`, `source`, `fidelity_level`, `uncertainty`, `objective`, `fitted`,
//!   omitted when unset (a ledger without them is byte-identical to the ADR 0121 format); the `measured` tier, written only to
//!   a private ledger ([`Ledger::private`]); the `fidelity_pair` record of ADR 0622 ([`fidelity_pair`]); the pairing and
//!   training-set queries ([`pair_by_params`], [`training_set`]).

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
    /// Measured data (ADR 0632): written only to a private ledger ([`Ledger::private`]) and skipped by a public reader.
    Measured,
}

impl Fidelity {
    /// Text form.
    pub fn as_str(self) -> &'static str {
        match self {
            Fidelity::Analytic => "analytic",
            Fidelity::ReducedOrder => "reduced_order",
            Fidelity::Cfd => "cfd",
            Fidelity::Measured => "measured",
        }
    }

    /// Parse the text form of a public tier (`analytic`, `reduced_order`, `cfd`); `measured` is refused, as by the ADR 0121
    /// reader (the MCP tools call this, so they cannot write a measured entry, ADR 0632 D2).
    pub fn parse(s: &str) -> Option<Fidelity> {
        Some(match s {
            "analytic" => Fidelity::Analytic,
            "reduced_order" => Fidelity::ReducedOrder,
            "cfd" => Fidelity::Cfd,
            _ => return None,
        })
    }

    /// Parse any tier, `measured` included (private ledgers).
    pub fn parse_any(s: &str) -> Option<Fidelity> {
        match s {
            "measured" => Some(Fidelity::Measured),
            _ => Fidelity::parse(s),
        }
    }
}

/// Reference to a ledger entry (ADR 0632 P1): the line position and, optionally, the FNV-1a 64 hash of the entry's canonical
/// text (`fnv1a64:` and 16 hex digits; see [`entry_hash`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryRef {
    /// Entry index (line position among the valid entries).
    pub index: usize,
    /// Hash of the entry's canonical text, when given.
    pub hash: Option<String>,
}

/// Where the values of an entry come from (ADR 0632 P3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    /// Data set or source id chosen by the caller (never a file path of a private file).
    pub id: String,
    /// `computed`, `measured` or `literature`.
    pub kind: String,
    /// Citation, when any.
    pub citation: Option<String>,
    /// SHA-256 of the source file (64 lower-case hex digits) as computed by the caller; the ledger does not open the file.
    pub file_sha256: Option<String>,
}

/// Identity of an objective or a fitted model (ADR 0632 P7, P8): id, version and a hash of its definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// Id.
    pub id: String,
    /// Version.
    pub version: String,
    /// Hash of the definition (free text, for example `fnv1a64:...`).
    pub hash: String,
}

/// The words allowed in [`Source::kind`].
pub const SOURCE_KINDS: [&str; 3] = ["computed", "measured", "literature"];

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
    /// Parent entries (ADR 0632 P2); key `parents`, omitted when empty.
    pub parents: Vec<EntryRef>,
    /// Source of the values (P3); key `source`, omitted when `None`.
    pub source: Option<Source>,
    /// Finer level inside the tier (P4); key `fidelity_level`, omitted when `None`.
    pub fidelity_level: Option<String>,
    /// Standard uncertainty of named metrics, in the metric's unit (P5); key `uncertainty`, omitted when empty.
    pub uncertainty: Vec<Quantity>,
    /// Objective that scored the entry (P7); key `objective`, omitted when `None`.
    pub objective: Option<Identity>,
    /// Fitted surrogate or correction the entry records (P8); key `fitted`, omitted when `None`.
    pub fitted: Option<Identity>,
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
            parents: Vec::new(),
            source: None,
            fidelity_level: None,
            uncertainty: Vec::new(),
            objective: None,
            fitted: None,
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

fn is_hex(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn check_hash(h: &str, what: &str) -> Result<(), String> {
    match h.strip_prefix("fnv1a64:") {
        Some(x) if is_hex(x, 16) => Ok(()),
        _ => Err(format!("{what}: '{h}' is not fnv1a64: and 16 lower-case hex digits")),
    }
}

fn check_identity(x: &Identity, what: &str) -> Result<(), String> {
    for (k, v) in [("id", &x.id), ("version", &x.version), ("hash", &x.hash)] {
        if v.trim().is_empty() {
            return Err(format!("{what}.{k}: empty"));
        }
    }
    Ok(())
}

fn ref_json(r: &EntryRef) -> Value {
    let mut o = Map::new();
    o.insert("index".into(), json!(r.index));
    if let Some(h) = &r.hash {
        o.insert("hash".into(), json!(h));
    }
    Value::Object(o)
}

fn identity_json(x: &Identity) -> Value {
    json!({"id": x.id, "version": x.version, "hash": x.hash})
}

fn source_json(s: &Source) -> Value {
    let mut o = Map::new();
    o.insert("id".into(), json!(s.id));
    o.insert("kind".into(), json!(s.kind));
    if let Some(c) = &s.citation {
        o.insert("citation".into(), json!(c));
    }
    if let Some(h) = &s.file_sha256 {
        o.insert("file_sha256".into(), json!(h));
    }
    Value::Object(o)
}

fn str_of(v: &Value, k: &str, what: &str) -> Result<String, String> {
    v.get(k)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("{what}.{k}: missing or not text"))
}

fn opt_str_of(v: &Value, k: &str, what: &str) -> Result<Option<String>, String> {
    match v.get(k) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{what}.{k}: not text")),
    }
}

fn identity_from(v: &Value, what: &str) -> Result<Identity, String> {
    if !v.is_object() {
        return Err(format!("{what}: expected {{id, version, hash}}"));
    }
    Ok(Identity {
        id: str_of(v, "id", what)?,
        version: str_of(v, "version", what)?,
        hash: str_of(v, "hash", what)?,
    })
}

fn refs_from(v: &Value) -> Result<Vec<EntryRef>, String> {
    let list = v.as_array().ok_or("parents: expected a list of {index, hash}")?;
    list.iter()
        .enumerate()
        .map(|(i, r)| {
            let at = format!("parents[{i}]");
            let index = r
                .get("index")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("{at}.index: missing or not a non-negative integer"))?;
            Ok(EntryRef {
                index: usize::try_from(index).map_err(|_| format!("{at}.index: too large"))?,
                hash: opt_str_of(r, "hash", &at)?,
            })
        })
        .collect()
}

fn source_from(v: &Value) -> Result<Source, String> {
    if !v.is_object() {
        return Err("source: expected {id, kind, citation, file_sha256}".into());
    }
    Ok(Source {
        id: str_of(v, "id", "source")?,
        kind: str_of(v, "kind", "source")?,
        citation: opt_str_of(v, "citation", "source")?,
        file_sha256: opt_str_of(v, "file_sha256", "source")?,
    })
}

impl Entry {
    /// Whether the entry is private content (ADR 0632 D2): fidelity `measured` or a source of kind `measured`.
    pub fn is_private_content(&self) -> bool {
        self.fidelity == Fidelity::Measured || self.source.as_ref().is_some_and(|s| s.kind == "measured")
    }

    /// Check the fields of ADR 0632 (D1); located messages.
    fn validate_new_fields(&self) -> Result<(), String> {
        for (i, r) in self.parents.iter().enumerate() {
            if let Some(h) = &r.hash {
                check_hash(h, &format!("parents[{i}].hash"))?;
            }
        }
        if let Some(s) = &self.source {
            if s.id.trim().is_empty() {
                return Err("source.id: empty".into());
            }
            if !SOURCE_KINDS.contains(&s.kind.as_str()) {
                return Err(format!(
                    "source.kind: '{}' is not computed, measured or literature",
                    s.kind
                ));
            }
            if let Some(h) = &s.file_sha256 {
                if !is_hex(h, 64) {
                    return Err("source.file_sha256: not 64 lower-case hex digits".into());
                }
            }
        }
        if self.fidelity_level.as_ref().is_some_and(|l| l.trim().is_empty()) {
            return Err("fidelity_level: empty".into());
        }
        check_quantities(&self.uncertainty, "uncertainty", false)?;
        for u in &self.uncertainty {
            let m = self
                .metric(&u.name)
                .ok_or_else(|| format!("uncertainty '{}': not a metric of the entry", u.name))?;
            if m.unit != u.unit {
                return Err(format!(
                    "uncertainty '{}': unit '{}' differs from the metric's '{}'",
                    u.name, u.unit, m.unit
                ));
            }
            if !(u.value.is_finite() && u.value >= 0.0) {
                return Err(format!("uncertainty '{}': not a finite non-negative number", u.name));
            }
        }
        if let Some(x) = &self.objective {
            check_identity(x, "objective")?;
        }
        if let Some(x) = &self.fitted {
            check_identity(x, "fitted")?;
        }
        Ok(())
    }

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
        self.validate_new_fields()
    }

    /// JSON form (one ledger line; keys sorted, so the text is deterministic). The keys of ADR 0632 are written only when set,
    /// so an entry without them gives the ADR 0121 text byte for byte.
    pub fn to_json(&self) -> Value {
        let mut v = self.base_json();
        if let Some(o) = v.as_object_mut() {
            if !self.parents.is_empty() {
                o.insert(
                    "parents".into(),
                    Value::Array(self.parents.iter().map(ref_json).collect()),
                );
            }
            if let Some(s) = &self.source {
                o.insert("source".into(), source_json(s));
            }
            if let Some(l) = &self.fidelity_level {
                o.insert("fidelity_level".into(), json!(l));
            }
            if !self.uncertainty.is_empty() {
                o.insert("uncertainty".into(), quantities_json(&self.uncertainty));
            }
            if let Some(x) = &self.objective {
                o.insert("objective".into(), identity_json(x));
            }
            if let Some(x) = &self.fitted {
                o.insert("fitted".into(), identity_json(x));
            }
        }
        v
    }

    fn base_json(&self) -> Value {
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

    /// Parse the JSON form of a public ledger line (a `measured` line is refused, as by the ADR 0121 reader).
    pub fn from_json(v: &Value) -> Result<Entry, String> {
        Entry::from_json_with(v, false)
    }

    /// Parse the JSON form of a private ledger line (`measured` accepted).
    pub fn from_json_private(v: &Value) -> Result<Entry, String> {
        Entry::from_json_with(v, true)
    }

    fn from_json_with(v: &Value, private: bool) -> Result<Entry, String> {
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
            fidelity: if private {
                Fidelity::parse_any(&fidelity)
            } else {
                Fidelity::parse(&fidelity)
            }
            .ok_or_else(|| {
                if fidelity == "measured" {
                    "fidelity: 'measured' is written only to a private ledger (ADR 0632); read it with Ledger::private".into()
                } else {
                    format!("fidelity: '{fidelity}' is unknown")
                }
            })?,
            recorded_at_ms: v
                .get("recorded_at_ms")
                .and_then(Value::as_u64)
                .ok_or("recorded_at_ms: missing")?,
            wall_s: v.get("wall_s").and_then(Value::as_f64),
            seed: v.get("seed").and_then(Value::as_u64),
            provenance: v.get("provenance").cloned().unwrap_or(Value::Null),
            parents: v.get("parents").map_or(Ok(Vec::new()), refs_from)?,
            source: v.get("source").map(source_from).transpose()?,
            fidelity_level: opt_str_of(v, "fidelity_level", "entry")?,
            uncertainty: v
                .get("uncertainty")
                .map_or(Ok(Vec::new()), |u| quantities_from(u, "uncertainty"))?,
            objective: v.get("objective").map(|x| identity_from(x, "objective")).transpose()?,
            fitted: v.get("fitted").map(|x| identity_from(x, "fitted")).transpose()?,
        })
        .and_then(|e: Entry| e.validate_new_fields().map(|()| e))
        .and_then(|e: Entry| {
            if !private && e.is_private_content() {
                Err("source: kind 'measured' is written only to a private ledger (ADR 0632); read it with Ledger::private".into())
            } else {
                Ok(e)
            }
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

/// Parse ledger text: blank lines are ignored, every other line is an entry or is reported as skipped. Public reader: a
/// `measured` line is skipped (ADR 0632 D2).
pub fn parse_text(text: &str) -> Contents {
    parse_text_with(text, false)
}

/// Parse the text of a private ledger (`measured` lines are entries).
pub fn parse_text_private(text: &str) -> Contents {
    parse_text_with(text, true)
}

fn parse_text_with(text: &str, private: bool) -> Contents {
    let mut out = Contents::default();
    for (n, line) in text.split('\n').enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.trim().is_empty() {
            continue;
        }
        let r = serde_json::from_str::<Value>(line)
            .map_err(|e| format!("not JSON ({e}); a torn line"))
            .and_then(|v| Entry::from_json_with(&v, private));
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
    private: bool,
}

/// The ancestor directory of `path` that makes it part of a repository tree (holds `.git` or a workspace `Cargo.toml`).
pub fn repository_ancestor(path: &Path) -> Option<PathBuf> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    // canonicalise the nearest existing ancestor, keep the rest as given
    let mut base = abs.as_path();
    let mut rest = Vec::new();
    let canon = loop {
        if let Ok(c) = base.canonicalize() {
            break c;
        }
        match (base.file_name(), base.parent()) {
            (Some(n), Some(p)) => {
                rest.push(n.to_os_string());
                base = p;
            }
            _ => break abs.clone(),
        }
    };
    let full = rest.iter().rev().fold(canon, |p, n| p.join(n));
    full.ancestors()
        .skip(1)
        .find(|d| {
            d.join(".git").exists()
                || std::fs::read_to_string(d.join("Cargo.toml")).is_ok_and(|t| t.contains("[workspace]"))
        })
        .map(Path::to_path_buf)
}

impl Ledger {
    /// Ledger at `path` (the file is created by the first append).
    pub fn new(path: &Path) -> Ledger {
        Ledger {
            path: path.to_path_buf(),
            lock: Mutex::new(()),
            clock: system_clock_ms,
            private: false,
        }
    }

    /// Private ledger at `path` (ADR 0632 D2, ADR 0622 D7): may hold `measured` content. Refused, before anything is written,
    /// when `path` lies inside a repository tree (see [`repository_ancestor`]).
    pub fn private(path: &Path) -> Result<Ledger, String> {
        if let Some(a) = repository_ancestor(path) {
            return Err(format!(
                "private ledger: {} lies inside the repository tree at {}; give a path outside any repository",
                path.display(),
                a.display()
            ));
        }
        let mut l = Ledger::new(path);
        l.private = true;
        Ok(l)
    }

    /// Whether this is a private ledger.
    pub fn is_private(&self) -> bool {
        self.private
    }

    /// File path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read every entry (a missing file is an empty ledger).
    pub fn read(&self) -> Result<Contents, String> {
        match std::fs::read(&self.path) {
            Ok(b) => Ok(parse_text_with(&String::from_utf8_lossy(&b), self.private)),
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
            if !self.private && e.is_private_content() {
                return Err(format!(
                    "entry {i}: measured content (fidelity or source kind 'measured') is written only to a private ledger \
                     (Ledger::private, ADR 0632); {} is public",
                    self.path.display()
                ));
            }
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
    /// Variant label (ADR 0632 P8).
    pub variant: Option<String>,
    /// Condition label, `settings.condition` as the study sink writes it (ADR 0632 P8).
    pub condition: Option<String>,
}

impl Filter {
    /// Whether `e` passes.
    pub fn matches(&self, e: &Entry) -> bool {
        self.kind.as_ref().is_none_or(|k| *k == e.kind)
            && self.verdict.is_none_or(|v| v == e.verdict)
            && self.study.as_ref().is_none_or(|s| *s == e.study)
            && self.evaluator.as_ref().is_none_or(|s| *s == e.evaluator)
            && self.fidelity.is_none_or(|f| f == e.fidelity)
            && self.variant.as_ref().is_none_or(|s| *s == e.variant)
            && self
                .condition
                .as_ref()
                .is_none_or(|c| e.settings.get("condition").and_then(Value::as_str) == Some(c.as_str()))
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

// ---------------------------------------------------------------- ADR 0632: references, pairs, training sets

fn fnv1a64_bytes(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// FNV-1a 64 of the entry's canonical text (`to_json().to_string()`, the line `Ledger::append` writes, without its newline),
/// as `fnv1a64:<16 hex digits>`.
pub fn entry_hash(e: &Entry) -> String {
    format!("fnv1a64:{:016x}", fnv1a64_bytes(e.to_json().to_string().as_bytes()))
}

/// Reference with hash to entry `i`.
pub fn entry_ref(i: usize, e: &Entry) -> EntryRef {
    EntryRef {
        index: i,
        hash: Some(entry_hash(e)),
    }
}

/// Resolve a reference: the index must exist and, when the reference has a hash, the entry's hash must match.
pub fn resolve(entries: &[Entry], r: &EntryRef) -> Result<usize, String> {
    let e = entries
        .get(r.index)
        .ok_or_else(|| format!("reference {}: the ledger has {} entries", r.index, entries.len()))?;
    if let Some(h) = &r.hash {
        let got = entry_hash(e);
        if got != *h {
            return Err(format!(
                "reference {}: hash {h} does not match the entry ({got})",
                r.index
            ));
        }
    }
    Ok(r.index)
}

/// Entries of `idx` grouped by `variant`, groups in first-seen order.
pub fn group_by_variant(entries: &[Entry], idx: &[usize]) -> Vec<(String, Vec<usize>)> {
    let mut out: Vec<(String, Vec<usize>)> = Vec::new();
    for &i in idx {
        let v = &entries[i].variant;
        match out.iter_mut().find(|g| g.0 == *v) {
            Some(g) => g.1.push(i),
            None => out.push((v.clone(), vec![i])),
        }
    }
    out
}

/// A fidelity level: the tier and, optionally, the finer `fidelity_level` inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    /// Tier.
    pub fidelity: Fidelity,
    /// Finer level (`None` matches only entries without one).
    pub level: Option<String>,
}

impl Level {
    /// Level of a tier without a finer level.
    pub fn tier(fidelity: Fidelity) -> Level {
        Level { fidelity, level: None }
    }

    fn of(e: &Entry) -> Level {
        Level {
            fidelity: e.fidelity,
            level: e.fidelity_level.clone(),
        }
    }

    fn text(&self) -> String {
        match &self.level {
            Some(l) => format!("{}/{l}", self.fidelity.as_str()),
            None => self.fidelity.as_str().to_string(),
        }
    }
}

/// Result of [`pair_by_params`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pairing {
    /// (low entry, high entry), ascending low index.
    pub pairs: Vec<(usize, usize)>,
    /// Entries not used because an earlier entry of the same level has the same parameters, ascending.
    pub duplicates: Vec<usize>,
}

/// Pairing key: parameters sorted by name, (name, unit, value bits).
type Key<'a> = Vec<(&'a str, &'a str, u64)>;

fn param_key(e: &Entry) -> Key<'_> {
    let mut k: Key<'_> = e
        .params
        .iter()
        .map(|q| (q.name.as_str(), q.unit.as_str(), q.value.to_bits()))
        .collect();
    k.sort_unstable();
    k
}

/// Units of every parameter among `sel` must agree; located refusal otherwise.
fn check_param_units(entries: &[Entry], sel: &[usize]) -> Result<(), String> {
    let mut seen: Vec<(&str, &str, usize)> = Vec::new();
    for &i in sel {
        for q in &entries[i].params {
            match seen.iter().find(|s| s.0 == q.name) {
                None => seen.push((&q.name, &q.unit, i)),
                Some(&(_, u, j)) if u != q.unit => {
                    return Err(format!(
                        "parameter '{}' has unit '{u}' in entry {j} and '{}' in entry {i}: mixed units",
                        q.name, q.unit
                    ))
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn one_evaluator(entries: &[Entry], sel: &[usize], what: &str) -> Result<(), String> {
    if let Some(&first) = sel.first() {
        let a = &entries[first];
        for &i in sel {
            let b = &entries[i];
            if b.evaluator != a.evaluator || b.evaluator_version != a.evaluator_version {
                return Err(format!(
                    "{what}: entries {first} and {i} come from different evaluators or versions ({} {}, {} {})",
                    a.evaluator, a.evaluator_version, b.evaluator, b.evaluator_version
                ));
            }
        }
    }
    Ok(())
}

/// Pairs of `ok` `evaluation` entries of `idx` at the levels `low` and `high` with bit-identical parameters (ADR 0632 D4).
/// The first entry of a parameter set in ledger order on each side is paired; later ones are listed as duplicates. Refused:
/// equal levels, a parameter in two units, more than one evaluator or version on a side.
pub fn pair_by_params(entries: &[Entry], idx: &[usize], low: &Level, high: &Level) -> Result<Pairing, String> {
    if low == high {
        return Err(format!(
            "pairing: both levels are {}; give two different levels",
            low.text()
        ));
    }
    let side = |lv: &Level| -> Vec<usize> {
        idx.iter()
            .copied()
            .filter(|&i| {
                let e = &entries[i];
                e.kind == "evaluation" && e.verdict == Verdict::Ok && Level::of(e) == *lv
            })
            .collect()
    };
    let (lo, hi) = (side(low), side(high));
    let all: Vec<usize> = {
        let mut a: Vec<usize> = lo.iter().chain(&hi).copied().collect();
        a.sort_unstable();
        a
    };
    check_param_units(entries, &all)?;
    one_evaluator(entries, &lo, &format!("pairing, level {}", low.text()))?;
    one_evaluator(entries, &hi, &format!("pairing, level {}", high.text()))?;
    let mut out = Pairing::default();
    let firsts = |s: &[usize], dup: &mut Vec<usize>| -> Vec<(Key, usize)> {
        let mut f: Vec<(Key, usize)> = Vec::new();
        for &i in s {
            let k = param_key(&entries[i]);
            if f.iter().any(|(fk, _)| *fk == k) {
                dup.push(i);
            } else {
                f.push((k, i));
            }
        }
        f
    };
    let lf = firsts(&lo, &mut out.duplicates);
    let hf = firsts(&hi, &mut out.duplicates);
    for (k, i) in &lf {
        if let Some((_, j)) = hf.iter().find(|(hk, _)| hk == k) {
            out.pairs.push((*i, *j));
        }
    }
    out.pairs.sort_unstable();
    out.duplicates.sort_unstable();
    Ok(out)
}

/// The fidelity pair and discrepancy record of ADR 0622 D6 between entries `low` and `high` (ADR 0632 D3): per metric `m`,
/// `m.low`, `m.high`, `m.diff = high - low`, `m.rel = diff / max(|high|, floor)` and, when `preds` is given, `m.pred`;
/// `parents` names both entries with their hashes. Private content when either side is.
pub fn fidelity_pair(
    entries: &[Entry],
    low: usize,
    high: usize,
    metrics: &[&str],
    floors: &[f64],
    preds: Option<&[f64]>,
) -> Result<Entry, String> {
    let get = |i: usize, what: &str| -> Result<&Entry, String> {
        let e = entries.get(i).ok_or_else(|| {
            format!(
                "fidelity_pair: {what} entry {i}: the ledger has {} entries",
                entries.len()
            )
        })?;
        if e.kind != "evaluation" || e.verdict != Verdict::Ok {
            return Err(format!("fidelity_pair: {what} entry {i} is not an ok evaluation"));
        }
        Ok(e)
    };
    let (l, h) = (get(low, "low")?, get(high, "high")?);
    if param_key(l) != param_key(h) {
        return Err(format!(
            "fidelity_pair: entries {low} and {high} do not have bit-identical parameters"
        ));
    }
    if metrics.is_empty() || floors.len() != metrics.len() || preds.is_some_and(|p| p.len() != metrics.len()) {
        return Err(
            "fidelity_pair: give one floor (and one prediction, when any) per metric, at least one metric".into(),
        );
    }
    let mut ms = Vec::new();
    let mut us = Vec::new();
    for (k, &m) in metrics.iter().enumerate() {
        let floor = floors[k];
        if !(floor.is_finite() && floor > 0.0) {
            return Err(format!("fidelity_pair: floor of '{m}' is not finite and positive"));
        }
        let (a, b) = match (l.metric(m), h.metric(m)) {
            (Some(a), Some(b)) => (a, b),
            _ => return Err(format!("fidelity_pair: metric '{m}' missing in entry {low} or {high}")),
        };
        if a.unit != b.unit {
            return Err(format!(
                "fidelity_pair: metric '{m}' has unit '{}' in entry {low} and '{}' in entry {high}",
                a.unit, b.unit
            ));
        }
        if !(a.value.is_finite() && b.value.is_finite()) {
            return Err(format!(
                "fidelity_pair: metric '{m}' is not finite in entry {low} or {high}"
            ));
        }
        let diff = b.value - a.value;
        ms.push(Quantity::new(&format!("{m}.low"), a.value, &a.unit));
        ms.push(Quantity::new(&format!("{m}.high"), b.value, &b.unit));
        ms.push(Quantity::new(&format!("{m}.diff"), diff, &a.unit));
        ms.push(Quantity::new(&format!("{m}.rel"), diff / b.value.abs().max(floor), "1"));
        if let Some(p) = preds {
            ms.push(Quantity::new(&format!("{m}.pred"), p[k], &a.unit));
        }
        for (e, tag) in [(l, "low"), (h, "high")] {
            if let Some(u) = e.uncertainty.iter().find(|u| u.name == m) {
                us.push(Quantity::new(&format!("{m}.{tag}"), u.value, &u.unit));
            }
        }
    }
    let side = |e: &Entry| {
        json!({"evaluator": e.evaluator, "evaluator_version": e.evaluator_version, "fidelity": e.fidelity.as_str(),
               "fidelity_level": e.fidelity_level})
    };
    let measured = l.fidelity == Fidelity::Measured || h.fidelity == Fidelity::Measured;
    let e = Entry {
        kind: "fidelity_pair".into(),
        study: h.study.clone(),
        variant: h.variant.clone(),
        params: l.params.clone(),
        evaluator: "ledger.fidelity_pair".into(),
        evaluator_version: "1".into(),
        settings: json!({"low": side(l), "high": side(h)}),
        metrics: ms,
        verdict: Verdict::Ok,
        fidelity: if measured { Fidelity::Measured } else { h.fidelity },
        fidelity_level: h.fidelity_level.clone(),
        parents: vec![entry_ref(low, l), entry_ref(high, h)],
        source: h.source.clone().or_else(|| l.source.clone()),
        uncertainty: us,
        ..Default::default()
    };
    e.validate().map_err(|m| format!("fidelity_pair: {m}"))?;
    Ok(e)
}

/// A training set (ADR 0632 D4): rows are `ok` evaluation entries, columns the requested parameters and metrics.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TrainingSet {
    /// Parameter columns (name, unit), in the requested order.
    pub params: Vec<(String, String)>,
    /// Metric columns (name, unit), in the requested order.
    pub metrics: Vec<(String, String)>,
    /// Entry index of each row, ledger order.
    pub rows: Vec<usize>,
    /// `x[r][c]`: parameter `c` of row `r`.
    pub x: Vec<Vec<f64>>,
    /// `y[r][c]`: metric `c` of row `r`.
    pub y: Vec<Vec<f64>>,
    /// Evaluation entries that are not `ok` (failed points), ledger order.
    pub failed: Vec<usize>,
    /// Evaluator id, version, tier and level of every row (empty when there is none).
    pub evaluator: String,
    /// Evaluator version.
    pub evaluator_version: String,
    /// Tier and level.
    pub level: Option<Level>,
}

impl TrainingSet {
    /// References with hashes to the rows (the `parents` of a fit record).
    pub fn refs(&self, entries: &[Entry]) -> Vec<EntryRef> {
        self.rows.iter().map(|&i| entry_ref(i, &entries[i])).collect()
    }
}

/// Training set of the `evaluation` entries of `idx` (ADR 0632 D4). Refused: more than one evaluator, version, tier or level;
/// a parameter or metric in two units; an `ok` entry without a requested parameter or metric; no parameter or no metric.
pub fn training_set(
    entries: &[Entry],
    idx: &[usize],
    params: &[&str],
    metrics: &[&str],
) -> Result<TrainingSet, String> {
    if params.is_empty() || metrics.is_empty() {
        return Err("training_set: give at least one parameter and one metric".into());
    }
    let sel: Vec<usize> = idx
        .iter()
        .copied()
        .filter(|&i| entries[i].kind == "evaluation")
        .collect();
    let mut ts = TrainingSet::default();
    if let Some(&f) = sel.first() {
        let a = &entries[f];
        for &i in &sel {
            let b = &entries[i];
            let pairs = [
                ("evaluator", &a.evaluator, &b.evaluator),
                ("evaluator_version", &a.evaluator_version, &b.evaluator_version),
            ];
            for (what, x, y) in pairs {
                if x != y {
                    return Err(format!(
                        "training_set: {what} is '{x}' in entry {f} and '{y}' in entry {i}: mixed {what}s"
                    ));
                }
            }
            if Level::of(a) != Level::of(b) {
                return Err(format!(
                    "training_set: level is {} in entry {f} and {} in entry {i}: mixed levels",
                    Level::of(a).text(),
                    Level::of(b).text()
                ));
            }
        }
        ts.evaluator = a.evaluator.clone();
        ts.evaluator_version = a.evaluator_version.clone();
        ts.level = Some(Level::of(a));
    }
    let mut pu: Vec<Option<(String, usize)>> = vec![None; params.len()];
    let mut mu: Vec<Option<(String, usize)>> = vec![None; metrics.len()];
    let unit_check = |slot: &mut Option<(String, usize)>, name: &str, unit: &str, i: usize| -> Result<(), String> {
        match slot {
            None => {
                *slot = Some((unit.to_string(), i));
                Ok(())
            }
            Some((u, j)) if u != unit => Err(format!(
                "training_set: '{name}' has unit '{u}' in entry {j} and '{unit}' in entry {i}: mixed units"
            )),
            _ => Ok(()),
        }
    };
    for &i in &sel {
        let e = &entries[i];
        for (c, &p) in params.iter().enumerate() {
            if let Some(q) = e.param(p) {
                unit_check(&mut pu[c], p, &q.unit, i)?;
            }
        }
        for (c, &m) in metrics.iter().enumerate() {
            if let Some(q) = e.metric(m) {
                unit_check(&mut mu[c], m, &q.unit, i)?;
            }
        }
        if e.verdict != Verdict::Ok {
            ts.failed.push(i);
            continue;
        }
        let row = |names: &[&str], get: &dyn Fn(&str) -> Option<f64>, what: &str| -> Result<Vec<f64>, String> {
            names
                .iter()
                .map(|n| get(n).ok_or_else(|| format!("training_set: entry {i} has no {what} '{n}'")))
                .collect()
        };
        ts.x.push(row(params, &|n| e.param(n).map(|q| q.value), "parameter")?);
        ts.y.push(row(metrics, &|n| e.metric(n).map(|q| q.value), "metric")?);
        ts.rows.push(i);
    }
    let cols = |names: &[&str], units: &[Option<(String, usize)>]| -> Vec<(String, String)> {
        names
            .iter()
            .zip(units)
            .map(|(n, u)| (n.to_string(), u.as_ref().map_or(String::new(), |u| u.0.clone())))
            .collect()
    };
    ts.params = cols(params, &pu);
    ts.metrics = cols(metrics, &mu);
    Ok(ts)
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

#[cfg(test)]
#[path = "ledger_f700_tests.rs"]
mod f700_tests;
