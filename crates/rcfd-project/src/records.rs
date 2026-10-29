//! Typed records of the project folder contract (docs/ui-restart-plan.md section 8).
//!
//! Readers ignore unknown fields so the contract can grow additively. Hashes are strings
//! (`fnv64:<16 hex>`), not JSON numbers, so JavaScript and Python clients keep every bit.
//!
//! Tolerant readers (ADR 0130 D2): `run.json` written by the run service before it used this crate
//! (`case_source` instead of `case`, a null `solver_version`) reads; events tagged `event` (the run
//! service) as well as `kind` read, and kinds this crate does not model become [`EventKind::Other`];
//! a `null` number (serde_json writes NaN and infinity as `null`) in residuals, `status.json` time and
//! metrics reads as NaN. Writers are unchanged.

use serde::de::Error as _;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub use rcfd_core::hash::fnv64;

/// Current manifest schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// `rcfd-project.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema version of the project folder.
    pub schema_version: u32,
    /// Display name.
    pub name: String,
    /// Creation time, RFC 3339 UTC.
    pub created: String,
    /// Version of the solver that created the project.
    pub solver_version_created: String,
}

/// State of a run (`status.json`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    /// Waiting for the runner.
    Queued,
    /// Running.
    Running,
    /// Paused by request.
    Paused,
    /// Finished normally.
    Finished,
    /// Stopped by an error.
    Failed,
    /// Stopped by request.
    Cancelled,
}

impl RunState {
    /// Finished, failed or cancelled.
    pub fn is_terminal(self) -> bool {
        matches!(self, RunState::Finished | RunState::Failed | RunState::Cancelled)
    }
}

/// Value of `solver_version` when `run.json` has none (null or missing).
pub const UNKNOWN_SOLVER_VERSION: &str = "unknown";

/// `runs/<id>/run.json`: written once at creation, never replaced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RunRecordWire")]
pub struct RunRecord {
    /// Run id (`r-<yyyymmddThhmmss>-<4 hex>`).
    pub id: String,
    /// Project-relative path of the case that was snapshotted (read from `case_source` when `case` is absent).
    pub case: String,
    /// Hash of `case.snapshot`.
    pub case_snapshot_hash: String,
    /// Project-relative path of the mesh, if the run names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<String>,
    /// Hash of the mesh file bytes at creation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_hash: Option<String>,
    /// Solver version that the run is for ([`UNKNOWN_SOLVER_VERSION`] when the record has none).
    pub solver_version: String,
    /// Run settings (free JSON).
    pub settings: serde_json::Value,
    /// Creation time, RFC 3339 UTC with milliseconds (the run service writes nanoseconds).
    pub created: String,
    /// Absolute path of the case file, as the run service wrote it before `case` (kept for older readers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_source: Option<String>,
    /// Directory the case's relative paths resolve against (run service).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub case_dir: Option<String>,
}

/// On-disk form of [`RunRecord`]: `case` or its legacy name `case_source`, optional `solver_version`.
#[derive(Deserialize)]
struct RunRecordWire {
    id: String,
    #[serde(default)]
    case: Option<String>,
    #[serde(default)]
    case_source: Option<String>,
    #[serde(default)]
    case_dir: Option<String>,
    case_snapshot_hash: String,
    #[serde(default)]
    mesh: Option<String>,
    #[serde(default)]
    mesh_hash: Option<String>,
    #[serde(default)]
    solver_version: Option<String>,
    settings: Value,
    created: String,
}

impl TryFrom<RunRecordWire> for RunRecord {
    type Error = String;
    fn try_from(w: RunRecordWire) -> Result<Self, String> {
        let case = w
            .case
            .or_else(|| w.case_source.clone())
            .ok_or("missing field `case` (or its legacy name `case_source`)")?;
        Ok(RunRecord {
            id: w.id,
            case,
            case_snapshot_hash: w.case_snapshot_hash,
            mesh: w.mesh,
            mesh_hash: w.mesh_hash,
            solver_version: w.solver_version.unwrap_or_else(|| UNKNOWN_SOLVER_VERSION.into()),
            settings: w.settings,
            created: w.created,
            case_source: w.case_source,
            case_dir: w.case_dir,
        })
    }
}

/// `runs/<id>/status.json`: replaced atomically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatus {
    /// State.
    pub state: RunState,
    /// Last completed step.
    pub step: u64,
    /// Simulated time of the last step (`null` reads as NaN).
    #[serde(deserialize_with = "nan::f64")]
    pub time: f64,
    /// Process id of the owner while running.
    #[serde(default)]
    pub pid: Option<u32>,
    /// Time of this update, RFC 3339 UTC.
    pub updated: String,
    /// Why the run is in this state (failures always carry one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Solver exit code (from its finish event or the process).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

impl RunStatus {
    /// A fresh status in `state`: step 0, time 0, no pid, updated now.
    pub fn new(state: RunState) -> Self {
        RunStatus {
            state,
            step: 0,
            time: 0.0,
            pid: None,
            updated: crate::time::now(),
            reason: None,
            exit_code: None,
        }
    }
}

/// One line of `runs/<id>/events.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunEvent {
    /// Time of the event, RFC 3339 UTC.
    pub t: String,
    /// What happened.
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Kinds of run events. On disk the kind is the `kind` field; lines written by the run service before it used this
/// crate name it `event` instead, and are read as well (ADR 0130 D2).
#[derive(Debug, Clone, PartialEq)]
pub enum EventKind {
    /// State change (`from` is null for the creation event).
    State {
        /// Previous state.
        from: Option<RunState>,
        /// New state.
        to: RunState,
        /// Why (run service).
        reason: Option<String>,
    },
    /// A warning.
    Warning {
        /// Text.
        message: String,
    },
    /// Why the run stopped.
    Stop {
        /// Text.
        reason: String,
    },
    /// A kind this crate does not model (for example the run service's `launched`), with its other fields.
    Other {
        /// The kind's name.
        kind: String,
        /// The line's fields other than `t` and the tag (an object).
        body: Value,
    },
}

impl Serialize for EventKind {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        match self {
            EventKind::State { from, to, reason } => {
                m.serialize_entry("kind", "state")?;
                m.serialize_entry("from", from)?;
                m.serialize_entry("to", to)?;
                if let Some(r) = reason {
                    m.serialize_entry("reason", r)?;
                }
            }
            EventKind::Warning { message } => {
                m.serialize_entry("kind", "warning")?;
                m.serialize_entry("message", message)?;
            }
            EventKind::Stop { reason } => {
                m.serialize_entry("kind", "stop")?;
                m.serialize_entry("reason", reason)?;
            }
            EventKind::Other { kind, body } => {
                m.serialize_entry("kind", kind)?;
                match body.as_object() {
                    Some(o) => {
                        for (k, v) in o.iter().filter(|(k, _)| !matches!(k.as_str(), "kind" | "t")) {
                            m.serialize_entry(k, v)?;
                        }
                    }
                    None if body.is_null() => {}
                    None => m.serialize_entry("body", body)?,
                }
            }
        }
        m.end()
    }
}

#[derive(Deserialize)]
struct StateBody {
    #[serde(default)]
    from: Option<RunState>,
    to: RunState,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
struct LegacyStateBody {
    state: RunState,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
struct WarningBody {
    message: String,
}

#[derive(Deserialize)]
struct StopBody {
    reason: String,
}

impl<'de> Deserialize<'de> for EventKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut m = Map::<String, Value>::deserialize(d)?;
        m.remove("t");
        let (tag, from_kind) = match (m.get("kind"), m.get("event")) {
            (Some(Value::String(k)), _) => (k.clone(), true),
            (Some(other), _) => return Err(D::Error::custom(format!("event `kind` is not a string: {other}"))),
            (None, Some(Value::String(e))) => (e.clone(), false),
            (None, Some(other)) => return Err(D::Error::custom(format!("event `event` is not a string: {other}"))),
            (None, None) => return Err(D::Error::custom("event line has neither `kind` nor `event`")),
        };
        // the tag field, and the other tag field when it repeats the same name (the run service writes both)
        m.remove(if from_kind { "kind" } else { "event" });
        let twin = if from_kind { "event" } else { "kind" };
        if m.get(twin).and_then(Value::as_str) == Some(tag.as_str()) {
            m.remove(twin);
        }
        let body = Value::Object(m);
        let parse = |what: &str, e: serde_json::Error| D::Error::custom(format!("`{what}` event: {e}"));
        Ok(match tag.as_str() {
            "state" if !from_kind && body.get("to").is_none() => {
                let b: LegacyStateBody = serde_json::from_value(body).map_err(|e| parse("state", e))?;
                EventKind::State {
                    from: None,
                    to: b.state,
                    reason: b.reason,
                }
            }
            "state" => {
                let b: StateBody = serde_json::from_value(body).map_err(|e| parse("state", e))?;
                EventKind::State {
                    from: b.from,
                    to: b.to,
                    reason: b.reason,
                }
            }
            "warning" => {
                let b: WarningBody = serde_json::from_value(body).map_err(|e| parse("warning", e))?;
                EventKind::Warning { message: b.message }
            }
            "stop" => {
                let b: StopBody = serde_json::from_value(body).map_err(|e| parse("stop", e))?;
                EventKind::Stop { reason: b.reason }
            }
            _ => EventKind::Other { kind: tag, body },
        })
    }
}

/// One line of `runs/<id>/residuals.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualLine {
    /// Step.
    pub step: u64,
    /// Simulated time (`null` reads as NaN).
    #[serde(deserialize_with = "nan::f64")]
    pub time: f64,
    /// Residual by equation (`null` reads as NaN).
    #[serde(deserialize_with = "nan::map")]
    pub residuals: BTreeMap<String, f64>,
    /// Monitor values by name (`null` reads as NaN).
    #[serde(default, deserialize_with = "nan::map")]
    pub monitors: BTreeMap<String, f64>,
    /// Pressure correctors of the step's final outer iteration, only in runs with `solver.corrector_control = "adaptive"`
    /// (ADR 0160 D4; absent otherwise, and not written when `None`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correctors: Option<u64>,
    /// Estimates so far whose corrector search ended at the cap (same runs only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corrector_cap: Option<u64>,
}

/// `runs/<id>/metrics.json`: final report values by name.
pub type Metrics = BTreeMap<String, f64>;

/// The numeric entries of a `metrics.json` document: numbers as they are, `null` as NaN (a non-finite value written
/// by serde_json); strings, objects and other values are left out.
pub fn metrics_from_json(v: &Value) -> Metrics {
    v.as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| match v {
            Value::Null => Some((k.clone(), f64::NAN)),
            v => v.as_f64().map(|x| (k.clone(), x)),
        })
        .collect()
}

/// Hash string of bytes with the repository's FNV-1a 64 (`rcfd_core::hash`).
pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("fnv64:{:016x}", fnv64(bytes))
}

/// Legacy spelling of the same FNV-1a 64 hash, written by the run service before it used this crate.
pub const LEGACY_HASH_PREFIX: &str = "fnv1a64:";

/// Does the recorded hash string name `bytes`? `fnv64:<hex>` and the legacy `fnv1a64:<hex>` are the same hash.
pub fn hash_matches(recorded: &str, bytes: &[u8]) -> bool {
    let want = hash_bytes(bytes);
    match recorded.strip_prefix(LEGACY_HASH_PREFIX) {
        Some(hex) => want.strip_prefix("fnv64:") == Some(hex),
        None => recorded == want,
    }
}

/// Readers of numbers that serde_json wrote as `null` (NaN, infinity).
mod nan {
    use serde::{Deserialize, Deserializer};
    use std::collections::BTreeMap;

    pub(super) fn f64<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
    }

    pub(super) fn map<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, f64>, D::Error> {
        Ok(BTreeMap::<String, Option<f64>>::deserialize(d)?
            .into_iter()
            .map(|(k, v)| (k, v.unwrap_or(f64::NAN)))
            .collect())
    }
}
