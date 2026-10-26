//! Typed records of the project folder contract (docs/ui-restart-plan.md section 8).
//!
//! Readers ignore unknown fields so the contract can grow additively. Hashes are strings
//! (`fnv64:<16 hex>`), not JSON numbers, so JavaScript and Python clients keep every bit.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

/// `runs/<id>/run.json`: written once at creation, never replaced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Run id (`r-<yyyymmddThhmmss>-<4 hex>`).
    pub id: String,
    /// Project-relative path of the case that was snapshotted.
    pub case: String,
    /// Hash of `case.snapshot`.
    pub case_snapshot_hash: String,
    /// Project-relative path of the mesh, if the run names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<String>,
    /// Hash of the mesh file bytes at creation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_hash: Option<String>,
    /// Solver version that the run is for.
    pub solver_version: String,
    /// Run settings (free JSON).
    pub settings: serde_json::Value,
    /// Creation time, RFC 3339 UTC with milliseconds.
    pub created: String,
}

/// `runs/<id>/status.json`: replaced atomically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatus {
    /// State.
    pub state: RunState,
    /// Last completed step.
    pub step: u64,
    /// Simulated time of the last step.
    pub time: f64,
    /// Process id of the owner while running.
    #[serde(default)]
    pub pid: Option<u32>,
    /// Time of this update, RFC 3339 UTC.
    pub updated: String,
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

/// Kinds of run events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    /// State change (`from` is null for the creation event).
    State {
        /// Previous state.
        from: Option<RunState>,
        /// New state.
        to: RunState,
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
}

/// One line of `runs/<id>/residuals.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResidualLine {
    /// Step.
    pub step: u64,
    /// Simulated time.
    pub time: f64,
    /// Residual by equation.
    pub residuals: BTreeMap<String, f64>,
    /// Monitor values by name.
    #[serde(default)]
    pub monitors: BTreeMap<String, f64>,
}

/// `runs/<id>/metrics.json`: final report values by name.
pub type Metrics = BTreeMap<String, f64>;

/// Hash string of bytes with the repository's FNV-1a 64 (`rcfd_core::hash`).
pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("fnv64:{:016x}", rcfd_core::hash::fnv64(bytes))
}
