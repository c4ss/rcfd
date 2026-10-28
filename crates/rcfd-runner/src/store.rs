//! The trait boundary between the run service and the project folder (ADR 0123, "Switching to rcfd-project").
//!
//! The runner and the client reach the run records only through [`RunStore`]; [`FsStore`] implements it on
//! the section-8 folder layout with `rcfd-project`'s records and helpers (`atomic`, `jsonl`, `runid`; ADR 0130 D1).

use crate::records::{self, io_error, RunRecord, RunSettings, RunState, RunStatus};
use rcfd_project::{atomic, jsonl, paths, runid};
use serde_json::{json, Value};
use std::io;
use std::path::{Path, PathBuf};

/// Files of `runs/<id>/` used by the run service in addition to the section-8 contract (additive).
pub mod extra {
    /// Request file: the run holds (queued: not launched; running: between two steps) while it exists.
    /// Also the solver's `--pause-file`.
    pub const PAUSE: &str = "pause";
    /// Written by the solver while it holds (`pause.ack`, the iteration count).
    pub const PAUSE_ACK: &str = "pause.ack";
    /// Request file: cancel the run. Also the solver's `--stop-file`.
    pub const CANCEL: &str = "cancel";
    /// The solver's status stream (`rcfd run --status-file`, format `rcfd-status`).
    pub const SOLVER_STREAM: &str = "solver-status.jsonl";
    /// The solver's `--out` directory (solution.vtu, monitors, restart.rcfd).
    pub const OUTPUT: &str = "output";
    /// Output of the pre-launch check (`rcfd run --check-only --status-json`).
    pub const CHECK: &str = "check.json";
    /// The time-series field store of the run (`rcfd run --store-dir`, ADR 0130 D3).
    pub const FIELDS: &str = "fields";
}

/// Access to the run records of one project.
pub trait RunStore {
    /// The project folder.
    fn project(&self) -> &Path;
    /// `runs/<id>/`.
    fn run_dir(&self, id: &str) -> PathBuf {
        self.project().join("runs").join(id)
    }
    /// Run ids present, sorted (ids sort by creation second).
    fn list_runs(&self) -> io::Result<Vec<String>>;
    /// Snapshot `case` into a new queued run; the folder appears complete or not at all.
    fn create_run(&self, case: &Path, settings: &RunSettings, solver_version: Option<String>) -> io::Result<String>;
    /// `run.json`.
    fn read_record(&self, id: &str) -> io::Result<RunRecord>;
    /// `status.json`.
    fn read_status(&self, id: &str) -> io::Result<RunStatus>;
    /// Replace `status.json` atomically (the `updated` time is set here).
    fn write_status(&self, id: &str, status: &RunStatus) -> io::Result<()>;
    /// Append to `events.jsonl` (a `t` field is added).
    fn append_event(&self, id: &str, event: Value) -> io::Result<()>;
    /// Append to `residuals.jsonl`.
    fn append_residual(&self, id: &str, line: &Value) -> io::Result<()>;
    /// Append several lines to `residuals.jsonl` (one drain of the solver stream); by default one at a time.
    fn append_residuals(&self, id: &str, lines: &[Value]) -> io::Result<()> {
        lines.iter().try_for_each(|l| self.append_residual(id, l))
    }
    /// Write `metrics.json` atomically.
    fn write_metrics(&self, id: &str, metrics: &Value) -> io::Result<()>;
}

/// [`RunStore`] on the project folder.
#[derive(Debug, Clone)]
pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    /// The store of the project folder `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FsStore { root: root.into() }
    }
}

fn bad(e: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
}

/// An event line in both tag forms: `kind` (rcfd-project) and `event` (read by clients of the 1248a76 runner),
/// for one release (ADR 0130 amendment 1).
pub fn event(kind: &str) -> Value {
    json!({"kind": kind, "event": kind})
}

/// The tag of an event line: `kind`, else the legacy `event`.
pub fn event_kind(v: &Value) -> Option<&str> {
    v["kind"].as_str().or_else(|| v["event"].as_str())
}

/// A state-change event line in both forms (`from`, `to`; legacy `state`), with its reason.
pub fn state_event(from: Option<RunState>, to: RunState, reason: &str) -> Value {
    let mut ev = event("state");
    ev["from"] = json!(from);
    ev["to"] = json!(to);
    ev["state"] = json!(to);
    ev["reason"] = json!(reason);
    ev
}

/// `case` of `run.json`: the project-relative path when the case file is inside the project, else the absolute
/// path (ADR 0130 amendment 1).
fn case_in_project(root: &Path, case_abs: &Path) -> String {
    match (paths::canonicalize(root), paths::canonicalize(case_abs)) {
        (Ok(r), Ok(c)) => paths::rel_string(&r, &c),
        _ => None,
    }
    .unwrap_or_else(|| case_abs.display().to_string())
}

impl RunStore for FsStore {
    fn project(&self) -> &Path {
        &self.root
    }

    fn list_runs(&self) -> io::Result<Vec<String>> {
        let dir = self.root.join("runs");
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut v: Vec<String> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| runid::is_run_id(n))
            .collect();
        v.sort();
        Ok(v)
    }

    fn create_run(&self, case: &Path, settings: &RunSettings, solver_version: Option<String>) -> io::Result<String> {
        let text = std::fs::read(case)?;
        let case_abs = std::env::current_dir()?.join(case);
        let case_dir = case_abs.parent().map(Path::to_path_buf).unwrap_or_default();
        let runs = self.root.join("runs");
        std::fs::create_dir_all(&runs)?;
        let mut id = records::new_run_id();
        while runs.join(&id).exists() || runs.join(format!(".tmp-{id}")).exists() {
            id = records::new_run_id();
        }
        let tmp = runs.join(format!(".tmp-{id}"));
        std::fs::create_dir(&tmp)?;
        let rec = RunRecord {
            id: id.clone(),
            case: case_in_project(&self.root, &case_abs),
            case_snapshot_hash: rcfd_project::records::hash_bytes(&text),
            mesh: None,
            mesh_hash: None,
            solver_version: solver_version.unwrap_or_else(|| rcfd_project::records::UNKNOWN_SOLVER_VERSION.into()),
            settings: serde_json::to_value(settings).map_err(bad)?,
            created: records::now_iso(),
            case_source: Some(case_abs.display().to_string()),
            case_dir: Some(case_dir.display().to_string()),
        };
        atomic::write_new(&tmp.join("case.snapshot"), &text).map_err(io_error)?;
        atomic::write_json_new(&tmp.join("run.json"), &rec).map_err(io_error)?;
        let st = RunStatus {
            updated: rec.created.clone(),
            ..RunStatus::new(RunState::Queued)
        };
        atomic::write_json(&tmp.join("status.json"), &st).map_err(io_error)?;
        let mut ev = state_event(None, RunState::Queued, "created");
        ev["t"] = json!(rec.created);
        jsonl::append_one(&tmp.join("events.jsonl"), &ev).map_err(io_error)?;
        records::retry(|| std::fs::rename(&tmp, runs.join(&id)))?;
        Ok(id)
    }

    fn read_record(&self, id: &str) -> io::Result<RunRecord> {
        let rec: RunRecord = atomic::read_json(&self.run_dir(id).join("run.json")).map_err(io_error)?;
        // a record whose settings the runner cannot schedule is not read, as before the switch
        RunSettings::of(&rec).map_err(|e| bad(format!("{}: {e}", self.run_dir(id).join("run.json").display())))?;
        Ok(rec)
    }

    fn read_status(&self, id: &str) -> io::Result<RunStatus> {
        atomic::read_json(&self.run_dir(id).join("status.json")).map_err(io_error)
    }

    fn write_status(&self, id: &str, status: &RunStatus) -> io::Result<()> {
        let mut s = status.clone();
        s.updated = records::now_iso();
        // replaced up to every poll period; without fsync, as before the switch (ADR 0123 limits: a killed process
        // loses nothing). An fsync after the rename widened the window in which a reader sees the final status
        // before its state event (S7 on Linux).
        let path = self.run_dir(id).join("status.json");
        let mut bytes = serde_json::to_vec_pretty(&s).map_err(bad)?;
        bytes.push(b'\n');
        atomic::atomic_write_unsynced(&path, &bytes).map_err(io_error)
    }

    fn append_event(&self, id: &str, mut event: Value) -> io::Result<()> {
        event["t"] = json!(records::now_iso());
        // without an fsync, as before the switch: the state event follows the status write closely (a reader that
        // sees the final status.json finds the event and no request files a moment later, ADR 0123 S7)
        let (mut a, _) = jsonl::Appender::open(&self.run_dir(id).join("events.jsonl")).map_err(io_error)?;
        a.append(&event).map_err(io_error)
    }

    fn append_residual(&self, id: &str, line: &Value) -> io::Result<()> {
        self.append_residuals(id, std::slice::from_ref(line))
    }

    fn append_residuals(&self, id: &str, lines: &[Value]) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        // one line per solver step, one open per drain: appended without an fsync, as before (a killed process
        // loses nothing; a torn last line left by a crash is cut by the next open)
        let (mut a, _) = jsonl::Appender::open(&self.run_dir(id).join("residuals.jsonl")).map_err(io_error)?;
        lines.iter().try_for_each(|l| a.append(l).map_err(io_error))
    }

    fn write_metrics(&self, id: &str, metrics: &Value) -> io::Result<()> {
        atomic::write_json(&self.run_dir(id).join("metrics.json"), metrics).map_err(io_error)
    }
}
