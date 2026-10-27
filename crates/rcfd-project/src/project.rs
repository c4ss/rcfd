//! Create, open, validate and query a project folder.

use crate::atomic::{self, is_temp_name};
use crate::records::{hash_bytes, EventKind, Manifest, RunEvent, RunRecord, RunState, RunStatus, SCHEMA_VERSION};
use crate::{io_err, jsonl, paths, runid, ProjectError, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Manifest file name.
pub const MANIFEST: &str = "rcfd-project.json";
/// Standard sub-directories created with a project.
pub const DIRS: [&str; 6] = ["cases", "meshes", "runs", "designs", "notes", ".rcfd"];

/// A migration step from schema version `n` (its index) to `n + 1`.
pub type Migration = fn(Value) -> std::result::Result<Value, String>;

/// Migration hook: entry `n` turns a version-`n` manifest into version `n + 1`.
pub const MIGRATIONS: [Migration; 1] = [v0_to_v1];

/// Version 0 is a constructed format (no version-0 project was ever written by rcfd): it exists
/// to exercise the hook. Fields: `title`, `created_unix` (seconds).
fn v0_to_v1(mut v: Value) -> std::result::Result<Value, String> {
    let o = v.as_object_mut().ok_or("manifest is not an object")?;
    let title = o
        .remove("title")
        .and_then(|t| t.as_str().map(String::from))
        .ok_or("v0: missing string `title`")?;
    let secs = o
        .remove("created_unix")
        .and_then(|t| t.as_u64())
        .ok_or("v0: missing integer `created_unix`")?;
    let created = crate::time::rfc3339_ms(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs));
    o.insert("name".into(), title.into());
    o.insert("created".into(), created.into());
    o.insert(
        "solver_version_created".into(),
        "unknown (migrated from schema 0)".into(),
    );
    o.insert("schema_version".into(), 1.into());
    Ok(v)
}

/// Bring a manifest JSON value to [`SCHEMA_VERSION`] with `migrations`. Returns the original
/// version. A version newer than this library, or a failed step, is a located error.
pub fn migrate(path: &Path, mut v: Value, migrations: &[Migration]) -> Result<(Value, u32)> {
    let bad = |msg: String| ProjectError::Invalid {
        path: path.to_path_buf(),
        msg,
    };
    let from = v
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| bad("missing integer `schema_version`".into()))?;
    let from = u32::try_from(from).map_err(|_| bad(format!("schema_version {from} out of range")))?;
    if from > SCHEMA_VERSION {
        return Err(bad(format!(
            "schema_version {from} is newer than supported {SCHEMA_VERSION}"
        )));
    }
    for n in from..SCHEMA_VERSION {
        let step = migrations
            .get(n as usize)
            .ok_or_else(|| bad(format!("no migration from schema_version {n}")))?;
        v = step(v).map_err(|e| bad(format!("migration {n} -> {}: {e}", n + 1)))?;
    }
    Ok((v, from))
}

/// A problem found by [`Project::validate`] or skipped by [`Project::runs`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// File or directory.
    pub path: PathBuf,
    /// What is wrong.
    pub message: String,
}

/// One row of the run list.
#[derive(Debug, Clone, PartialEq)]
pub struct RunSummary {
    /// Run id.
    pub id: String,
    /// Case path from `run.json`.
    pub case: String,
    /// Creation time from `run.json`.
    pub created: String,
    /// State, step and time from `status.json`.
    pub state: RunState,
    /// Last step.
    pub step: u64,
    /// Last simulated time.
    pub time: f64,
    /// Numeric entries of `metrics.json` (empty if absent).
    pub metrics: BTreeMap<String, f64>,
}

/// Run list with the run directories that could not be summarised.
#[derive(Debug, Clone, Default)]
pub struct Runs {
    /// Complete runs, oldest first (by `created`, then id).
    pub runs: Vec<RunSummary>,
    /// Incomplete or unreadable run directories (for example one being created right now).
    pub skipped: Vec<Issue>,
}

/// An open project folder.
#[derive(Debug, Clone)]
pub struct Project {
    root: PathBuf,
    manifest: Manifest,
    migrated_from: Option<u32>,
}

impl Project {
    /// Create a project folder (the folder may exist but must not hold a manifest).
    pub fn create(root: &Path, name: &str, solver_version: &str) -> Result<Self> {
        atomic::mkdirs(root)?;
        for d in DIRS {
            atomic::mkdirs(&root.join(d))?;
        }
        let manifest = Manifest {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            created: crate::time::now(),
            solver_version_created: solver_version.into(),
        };
        atomic::write_json_new(&root.join(MANIFEST), &manifest)?;
        Self::open(root)
    }

    /// Open a project folder. An older manifest is migrated in memory only (see
    /// [`Project::migrated_from`] and [`Project::save_manifest`]).
    pub fn open(root: &Path) -> Result<Self> {
        Self::open_with(root, &MIGRATIONS)
    }

    /// [`Project::open`] with an explicit migration chain.
    pub fn open_with(root: &Path, migrations: &[Migration]) -> Result<Self> {
        let path = root.join(MANIFEST);
        let raw: Value = atomic::read_json(&path)?;
        let (v, from) = migrate(&path, raw, migrations)?;
        let manifest = serde_json::from_value(v).map_err(|source| ProjectError::Json {
            path,
            line: None,
            source,
        })?;
        let root = paths::canonicalize(root).map_err(io_err("canonicalize", root))?;
        Ok(Project {
            root,
            manifest,
            migrated_from: (from < SCHEMA_VERSION).then_some(from),
        })
    }

    /// Canonical root.
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// The (migrated) manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    /// The on-disk schema version if the manifest was migrated on open.
    pub fn migrated_from(&self) -> Option<u32> {
        self.migrated_from
    }
    /// Write the in-memory manifest (current schema) atomically; this is how a migration is persisted.
    pub fn save_manifest(&mut self) -> Result<()> {
        atomic::write_json(&self.root.join(MANIFEST), &self.manifest)?;
        self.migrated_from = None;
        Ok(())
    }

    /// Resolve a project-relative path, refusing escapes (see [`crate::paths`]).
    pub fn resolve(&self, rel: &str) -> Result<PathBuf> {
        paths::resolve_in(&self.root, rel)
    }

    /// Case files under `cases/`, as project-relative paths, sorted.
    pub fn cases(&self) -> Result<Vec<String>> {
        self.files_under("cases")
    }
    /// Mesh files under `meshes/`, as project-relative paths, sorted.
    pub fn meshes(&self) -> Result<Vec<String>> {
        self.files_under("meshes")
    }
    /// Top-level entries of `designs/` (files or directories), as project-relative paths, sorted.
    pub fn designs(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for (name, _) in read_dir(&self.root.join("designs"))?
            .into_iter()
            .filter(|(n, _)| !is_temp_name(n))
        {
            out.push(format!("designs/{name}"));
        }
        Ok(out)
    }

    fn files_under(&self, top: &str) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let mut stack = vec![self.root.join(top)];
        while let Some(dir) = stack.pop() {
            for (name, ft) in read_dir(&dir)?.into_iter().filter(|(n, _)| !is_temp_name(n)) {
                let p = dir.join(&name);
                if ft.is_dir() {
                    stack.push(p);
                } else if ft.is_file() {
                    out.extend(paths::rel_string(&self.root, &p));
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// Directory of run `id` (the id is validated, so it cannot name another path).
    pub fn run_dir(&self, id: &str) -> Result<PathBuf> {
        if !runid::is_run_id(id) {
            return Err(ProjectError::PathRefused {
                input: id.into(),
                component: id.into(),
                reason: "not a run id",
            });
        }
        Ok(self.root.join("runs").join(id))
    }

    /// Create a run: claim an id, copy the case to `case.snapshot` and hash it, hash the mesh if given,
    /// write `run.json` (immutable), `status.json` (queued) and the creation event.
    pub fn create_run(
        &self,
        case: &str,
        mesh: Option<&str>,
        solver_version: &str,
        settings: Value,
    ) -> Result<RunRecord> {
        let case_path = self.resolve(case)?;
        let mesh_path = mesh.map(|m| self.resolve(m)).transpose()?;
        // Hash the exact bytes written to the snapshot (one read), so hash and snapshot agree.
        let case_bytes = atomic::read(&case_path)?;
        let mesh_hash = mesh_path
            .as_deref()
            .map(atomic::read)
            .transpose()?
            .map(|b| hash_bytes(&b));
        let now = SystemTime::now();
        let (id, dir) = runid::claim(&self.root.join("runs"), now)?;
        atomic::write_new(&dir.join("case.snapshot"), &case_bytes)?;
        let record = RunRecord {
            id: id.clone(),
            case: case.replace('\\', "/"),
            case_snapshot_hash: hash_bytes(&case_bytes),
            mesh: mesh.map(|m| m.replace('\\', "/")),
            mesh_hash,
            solver_version: solver_version.into(),
            settings,
            created: crate::time::rfc3339_ms(now),
        };
        atomic::write_json_new(&dir.join("run.json"), &record)?;
        let status = RunStatus {
            state: RunState::Queued,
            step: 0,
            time: 0.0,
            pid: None,
            updated: record.created.clone(),
        };
        atomic::write_json(&dir.join("status.json"), &status)?;
        let ev = RunEvent {
            t: record.created.clone(),
            kind: EventKind::State {
                from: None,
                to: RunState::Queued,
            },
        };
        jsonl::append_one(&dir.join("events.jsonl"), &ev)?;
        Ok(record)
    }

    /// `run.json` of a run.
    pub fn read_run(&self, id: &str) -> Result<RunRecord> {
        atomic::read_json(&self.run_dir(id)?.join("run.json"))
    }
    /// `status.json` of a run.
    pub fn read_status(&self, id: &str) -> Result<RunStatus> {
        atomic::read_json(&self.run_dir(id)?.join("status.json"))
    }
    /// Replace `status.json` atomically.
    pub fn write_status(&self, id: &str, status: &RunStatus) -> Result<()> {
        atomic::write_json(&self.run_dir(id)?.join("status.json"), status)
    }
    /// Append to `events.jsonl` (fsynced). Returns torn-tail bytes removed first.
    pub fn append_event(&self, id: &str, ev: &RunEvent) -> Result<u64> {
        jsonl::append_one(&self.run_dir(id)?.join("events.jsonl"), ev)
    }
    /// Committed events and any torn tail.
    pub fn read_events(&self, id: &str) -> Result<jsonl::Lines<RunEvent>> {
        jsonl::read(&self.run_dir(id)?.join("events.jsonl"))
    }

    /// Summary of one run from `run.json`, `status.json` and `metrics.json`.
    pub fn run_summary(&self, id: &str) -> Result<RunSummary> {
        let dir = self.run_dir(id)?;
        let rec: RunRecord = atomic::read_json(&dir.join("run.json"))?;
        let st: RunStatus = atomic::read_json(&dir.join("status.json"))?;
        let mpath = dir.join("metrics.json");
        let metrics = if mpath.exists() {
            let m: BTreeMap<String, Value> = atomic::read_json(&mpath)?;
            m.into_iter().filter_map(|(k, v)| v.as_f64().map(|x| (k, x))).collect()
        } else {
            BTreeMap::new()
        };
        Ok(RunSummary {
            id: rec.id,
            case: rec.case,
            created: rec.created,
            state: st.state,
            step: st.step,
            time: st.time,
            metrics,
        })
    }

    /// Ids of run directories, sorted.
    pub fn run_ids(&self) -> Result<Vec<String>> {
        Ok(read_dir(&self.root.join("runs"))?
            .into_iter()
            .filter(|(n, ft)| ft.is_dir() && runid::is_run_id(n))
            .map(|(n, _)| n)
            .collect())
    }

    /// All complete runs, oldest first; unreadable ones are listed in [`Runs::skipped`].
    pub fn runs(&self) -> Result<Runs> {
        let mut out = Runs::default();
        for id in self.run_ids()? {
            match self.run_summary(&id) {
                Ok(s) => out.runs.push(s),
                Err(e) => out.skipped.push(Issue {
                    path: self.root.join("runs").join(&id),
                    message: e.to_string(),
                }),
            }
        }
        out.runs.sort_by(|a, b| (&a.created, &a.id).cmp(&(&b.created, &b.id)));
        Ok(out)
    }

    /// Runs in `state`, oldest first.
    pub fn runs_in_state(&self, state: RunState) -> Result<Vec<RunSummary>> {
        Ok(self.runs()?.runs.into_iter().filter(|r| r.state == state).collect())
    }

    /// The most recently created finished run of `case` (path as recorded in `run.json`).
    pub fn latest_finished(&self, case: &str) -> Result<Option<RunSummary>> {
        let case = case.replace('\\', "/");
        Ok(self
            .runs_in_state(RunState::Finished)?
            .into_iter()
            .rev()
            .find(|r| r.case == case))
    }

    /// Check the folder against the contract. An empty list means valid.
    pub fn validate(&self) -> Result<Vec<Issue>> {
        let mut issues = Vec::new();
        let mut issue = |path: PathBuf, message: String| issues.push(Issue { path, message });
        if let Some(v) = self.migrated_from {
            issue(
                self.root.join(MANIFEST),
                format!("schema_version {v} (migrated in memory, not saved)"),
            );
        }
        for d in DIRS {
            if !self.root.join(d).is_dir() {
                issue(self.root.join(d), "missing directory".into());
            }
        }
        if let Ok(entries) = read_dir(&self.root) {
            for (n, _) in entries.into_iter().filter(|(n, _)| is_temp_name(n)) {
                issue(self.root.join(n), "stale temp file".into());
            }
        }
        let runs = self.root.join("runs");
        for (name, ft) in read_dir(&runs)? {
            let dir = runs.join(&name);
            if !ft.is_dir() || !runid::is_run_id(&name) {
                issue(dir, "not a run directory".into());
                continue;
            }
            for (n, _) in read_dir(&dir)?.into_iter().filter(|(n, _)| is_temp_name(n)) {
                issue(dir.join(n), "stale temp file".into());
            }
            match atomic::read_json::<RunRecord>(&dir.join("run.json")) {
                Err(e) => issue(dir.join("run.json"), e.to_string()),
                Ok(rec) => {
                    if rec.id != name {
                        issue(dir.join("run.json"), format!("id {} does not match directory", rec.id));
                    }
                    match atomic::read(&dir.join("case.snapshot")) {
                        Err(e) => issue(dir.join("case.snapshot"), e.to_string()),
                        Ok(b) if hash_bytes(&b) != rec.case_snapshot_hash => issue(
                            dir.join("case.snapshot"),
                            format!(
                                "hash {} does not match run.json {}",
                                hash_bytes(&b),
                                rec.case_snapshot_hash
                            ),
                        ),
                        Ok(_) => {}
                    }
                }
            }
            if let Err(e) = atomic::read_json::<RunStatus>(&dir.join("status.json")) {
                issue(dir.join("status.json"), e.to_string());
            }
            match jsonl::read::<RunEvent>(&dir.join("events.jsonl")) {
                Err(e) => issue(dir.join("events.jsonl"), e.to_string()),
                Ok(l) => {
                    if let Some(t) = l.torn {
                        issue(
                            dir.join("events.jsonl"),
                            format!("torn last line: {} bytes at offset {}", t.len, t.offset),
                        );
                    }
                }
            }
        }
        Ok(issues)
    }
}

/// Entries of a directory (name, type), sorted by name.
fn read_dir(dir: &Path) -> Result<Vec<(String, std::fs::FileType)>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).map_err(io_err("list", dir))? {
        let e = e.map_err(io_err("list", dir))?;
        let name = e.file_name().to_string_lossy().into_owned();
        let ft = e.file_type().map_err(io_err("stat", e.path()))?;
        out.push((name, ft));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}
