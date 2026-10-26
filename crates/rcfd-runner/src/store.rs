//! The trait boundary between the run service and the project folder (ADR 0123, "Switching to rcfd-project").
//!
//! The runner and the client reach the run records only through [`RunStore`]; [`FsStore`] implements it on
//! the section-8 folder layout with the local types of [`crate::records`].

use crate::records::{self, RunRecord, RunSettings, RunState, RunStatus};
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

fn read_json<T: serde::de::DeserializeOwned>(p: &Path) -> io::Result<T> {
    let text = records::retry(|| std::fs::read_to_string(p))?;
    serde_json::from_str(&text).map_err(|e| bad(format!("{}: {e}", p.display())))
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
            .filter(|n| n.starts_with("r-"))
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
        while runs.join(&id).exists() {
            id = records::new_run_id();
        }
        let tmp = runs.join(format!(".tmp-{id}"));
        std::fs::create_dir(&tmp)?;
        let rec = RunRecord {
            id: id.clone(),
            created: records::now_iso(),
            case_source: case_abs.display().to_string(),
            case_dir: case_dir.display().to_string(),
            case_snapshot_hash: format!("fnv1a64:{:016x}", records::fnv1a64(&text)),
            mesh_hash: None,
            solver_version,
            settings: settings.clone(),
        };
        std::fs::write(tmp.join("case.snapshot"), &text)?;
        std::fs::write(tmp.join("run.json"), serde_json::to_vec_pretty(&rec).map_err(bad)?)?;
        let st = RunStatus::new(RunState::Queued);
        std::fs::write(tmp.join("status.json"), serde_json::to_vec_pretty(&st).map_err(bad)?)?;
        records::append_line(
            &tmp.join("events.jsonl"),
            &json!({"t": rec.created, "event": "state", "state": "queued", "reason": "created"}).to_string(),
        )?;
        records::retry(|| std::fs::rename(&tmp, runs.join(&id)))?;
        Ok(id)
    }

    fn read_record(&self, id: &str) -> io::Result<RunRecord> {
        read_json(&self.run_dir(id).join("run.json"))
    }

    fn read_status(&self, id: &str) -> io::Result<RunStatus> {
        read_json(&self.run_dir(id).join("status.json"))
    }

    fn write_status(&self, id: &str, status: &RunStatus) -> io::Result<()> {
        let mut s = status.clone();
        s.updated = records::now_iso();
        records::atomic_write(
            &self.run_dir(id).join("status.json"),
            &serde_json::to_vec_pretty(&s).map_err(bad)?,
        )
    }

    fn append_event(&self, id: &str, mut event: Value) -> io::Result<()> {
        event["t"] = json!(records::now_iso());
        records::append_line(&self.run_dir(id).join("events.jsonl"), &event.to_string())
    }

    fn append_residual(&self, id: &str, line: &Value) -> io::Result<()> {
        records::append_line(&self.run_dir(id).join("residuals.jsonl"), &line.to_string())
    }

    fn write_metrics(&self, id: &str, metrics: &Value) -> io::Result<()> {
        records::atomic_write(
            &self.run_dir(id).join("metrics.json"),
            &serde_json::to_vec_pretty(metrics).map_err(bad)?,
        )
    }
}
