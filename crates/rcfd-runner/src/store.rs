//! The trait boundary between the run service and the project folder (ADR 0123, "Switching to rcfd-project").
//!
//! The runner and the client reach the run records only through [`RunStore`]; [`FsStore`] implements it on
//! the section-8 folder layout with `rcfd-project`'s records and helpers (`atomic`, `jsonl`, `runid`; ADR 0130 D1).

use crate::records::{self, io_error, LaunchSettings, RunRecord, RunSettings, RunState, RunStatus};
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
    /// Write the launch record `launch` of run `id` (ADR 0734 P1) to [`launch_file`], never replacing one; returns the
    /// file.
    fn write_launch(&self, id: &str, launch: &Value) -> io::Result<PathBuf> {
        let seq = launch["launch_seq"].as_u64().unwrap_or(0);
        let file = launch_file(&self.run_dir(id), seq);
        atomic::write_json_new(&file, launch).map_err(io_error)?;
        Ok(file)
    }
}

/// [`RunStore`] on the project folder.
#[derive(Debug, Clone)]
pub struct FsStore {
    root: PathBuf,
}

/// What a run of a design study is for (ADR 0189 S6): written to `run.json` as `study`, `variant` and `condition`, each only
/// when set. The default sets none.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunTags {
    /// The study's folder, project-relative.
    pub study: Option<String>,
    /// The study's candidate (`<parameter>=<value>,...`).
    pub variant: Option<String>,
    /// The label of the operating condition.
    pub condition: Option<String>,
}

impl FsStore {
    /// The store of the project folder `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FsStore { root: root.into() }
    }

    /// [`RunStore::create_run`] with the run's `tags` in `run.json` (ADR 0189 S6); without tags the run is the one
    /// `create_run` makes, byte for byte.
    pub fn create_run_tagged(
        &self,
        case: &Path,
        settings: &RunSettings,
        solver_version: Option<String>,
        tags: &RunTags,
    ) -> io::Result<String> {
        self.create_run_with(case, settings, &LaunchSettings::default(), solver_version, tags)
    }

    /// [`FsStore::create_run_tagged`] with the run's launch settings (`continue_from`, `max_steps`) in `run.json`
    /// `settings` (ADR 0200 S2); without them the run is the one `create_run_tagged` makes, byte for byte. The settings
    /// are not checked here (see [`crate::client::Client::submit_with`]).
    pub fn create_run_with(
        &self,
        case: &Path,
        settings: &RunSettings,
        launch: &LaunchSettings,
        solver_version: Option<String>,
        tags: &RunTags,
    ) -> io::Result<String> {
        self.create_run_at(case, settings, launch, solver_version, tags, None)
    }

    /// [`FsStore::create_run_with`] for a run on the site `site` (ADR 0739 P6): `run.json` also has `site`; with `None`
    /// the run is the one `create_run_with` makes, byte for byte.
    pub fn create_run_at(
        &self,
        case: &Path,
        settings: &RunSettings,
        launch: &LaunchSettings,
        solver_version: Option<String>,
        tags: &RunTags,
        site: Option<String>,
    ) -> io::Result<String> {
        let text = std::fs::read(case)?;
        let case_abs = std::env::current_dir()?.join(case);
        let case_dir = case_abs.parent().map(Path::to_path_buf).unwrap_or_default();
        let from = Source {
            text: &text,
            case: case_in_project(&self.root, &case_abs),
            name: &case_abs,
            case_source: Some(case_abs.display().to_string()),
            case_dir: Some(case_dir.display().to_string()),
            mesh_dir: &case_dir,
            rerun_of: None,
            site,
        };
        self.write_run(&from, settings, launch, solver_version, tags)
    }

    /// A new queued run of the case of run `src` as it was then (ADR 0734 P3): its `case.snapshot`, `case`, `case_source`
    /// and `case_dir`, so its relative paths resolve as the original's did, with `settings` and `launch` (the caller
    /// takes them from the record and applies its overrides) and `rerun_of: <src>`. The mesh hash is the mesh's now. The
    /// design-study tags are not copied: a rerun is not a run of the study.
    pub fn create_rerun(
        &self,
        src: &RunRecord,
        settings: &RunSettings,
        launch: &LaunchSettings,
        solver_version: Option<String>,
    ) -> io::Result<String> {
        self.create_rerun_at(src, settings, launch, solver_version, None)
    }

    /// [`FsStore::create_rerun`] on the site `site` (ADR 0739 P6; `None`: the same run as `create_rerun` makes).
    pub fn create_rerun_at(
        &self,
        src: &RunRecord,
        settings: &RunSettings,
        launch: &LaunchSettings,
        solver_version: Option<String>,
        site: Option<String>,
    ) -> io::Result<String> {
        let text = std::fs::read(self.run_dir(&src.id).join("case.snapshot"))?;
        let case_dir = match &src.case_dir {
            Some(d) => PathBuf::from(d),
            None => self
                .root
                .join(&src.case)
                .parent()
                .map_or_else(|| self.root.clone(), Path::to_path_buf),
        };
        let from = Source {
            text: &text,
            case: src.case.clone(),
            name: Path::new(&src.case),
            case_source: src.case_source.clone(),
            case_dir: src.case_dir.clone(),
            mesh_dir: &case_dir,
            rerun_of: Some(src.id.clone()),
            site,
        };
        self.write_run(&from, settings, launch, solver_version, &RunTags::default())
    }

    /// The mesh of a file-mesh case and its hash (ADR 0734 P4, Q4 (a)): `mesh` project-relative when it is in the
    /// project, else absolute; `mesh_hash` absent when the mesh cannot be hashed (missing, a link, over the limit: the
    /// solver's check reports a mesh it cannot read). `(None, None)` for a generator case.
    fn mesh_of(&self, from: &Source<'_>, set: &[String]) -> (Option<String>, Option<String>) {
        match crate::mesh::case_mesh(from.text, from.name, from.mesh_dir, set) {
            Some(m) => (
                Some(case_in_project(&self.root, &m)),
                rcfd_project::filehash::hash_path(&m).ok().map(|h| h.hash),
            ),
            None => (None, None),
        }
    }

    fn write_run(
        &self,
        from: &Source<'_>,
        settings: &RunSettings,
        launch: &LaunchSettings,
        solver_version: Option<String>,
        tags: &RunTags,
    ) -> io::Result<String> {
        let text = from.text;
        let (mesh, mesh_hash) = self.mesh_of(from, &settings.set);
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
            case: from.case.clone(),
            case_snapshot_hash: rcfd_project::records::hash_bytes(text),
            mesh,
            mesh_hash,
            solver_version: solver_version.unwrap_or_else(|| rcfd_project::records::UNKNOWN_SOLVER_VERSION.into()),
            settings: launch.merged(settings).map_err(bad)?,
            created: records::now_iso(),
            case_source: from.case_source.clone(),
            case_dir: from.case_dir.clone(),
            study: tags.study.clone(),
            variant: tags.variant.clone(),
            condition: tags.condition.clone(),
            rerun_of: from.rerun_of.clone(),
            site: from.site.clone(),
        };
        atomic::write_new(&tmp.join("case.snapshot"), text).map_err(io_error)?;
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
}

/// What a new run is made from: the case text and how `run.json` names it.
struct Source<'a> {
    text: &'a [u8],
    case: String,
    /// The case's file name (its extension says TOML or JSON).
    name: &'a Path,
    case_source: Option<String>,
    case_dir: Option<String>,
    /// The folder the case's mesh file resolves against.
    mesh_dir: &'a Path,
    rerun_of: Option<String>,
    /// The run site (ADR 0739); `None` for a local run.
    site: Option<String>,
}

/// The launch record of a run (ADR 0734 P1, Q1 (a), Q2 (a)): `runs/<id>/launch.json`, and `launch-<launch_seq>.json`
/// for a later launch of the same run; each written once, before the solver is spawned.
pub const LAUNCH_FILE: &str = "launch.json";

/// The file a launch with `launch_seq` writes in the run folder `dir`.
pub fn launch_file(dir: &Path, launch_seq: u64) -> PathBuf {
    let first = dir.join(LAUNCH_FILE);
    if first.exists() {
        dir.join(format!("launch-{launch_seq}.json"))
    } else {
        first
    }
}

fn is_launch_name(name: &str) -> bool {
    name == LAUNCH_FILE
        || name
            .strip_prefix("launch-")
            .and_then(|r| r.strip_suffix(".json"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The latest launch record of the run folder `dir` (the one started last), `None` when the run was never launched (or
/// was launched before ADR 0734). A record that does not read is an error naming its file.
pub fn latest_launch(dir: &Path) -> io::Result<Option<Value>> {
    let rd = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut best: Option<Value> = None;
    for e in rd.flatten() {
        if !is_launch_name(&e.file_name().to_string_lossy()) {
            continue;
        }
        let v: Value = atomic::read_json(&e.path()).map_err(io_error)?;
        let started = |x: &Value| x["started"].as_str().unwrap_or_default().to_string();
        if best.as_ref().map_or(true, |b| started(&v) > started(b)) {
            best = Some(v);
        }
    }
    Ok(best)
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
        self.create_run_tagged(case, settings, solver_version, &RunTags::default())
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
        // loses nothing). The terminal status is the last write of a run (ADR 0742): its state event is appended
        // before it.
        let path = self.run_dir(id).join("status.json");
        let mut bytes = serde_json::to_vec_pretty(&s).map_err(bad)?;
        bytes.push(b'\n');
        atomic::atomic_write_unsynced(&path, &bytes).map_err(io_error)
    }

    fn append_event(&self, id: &str, mut event: Value) -> io::Result<()> {
        event["t"] = json!(records::now_iso());
        // without an fsync, as before the switch. The state event of a run's end is appended before its terminal
        // status.json (ADR 0742; a reader that sees that status finds the event and no request files, ADR 0123 S7)
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
