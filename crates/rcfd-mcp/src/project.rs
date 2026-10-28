//! Project tools (ADR 0135): tools on one project folder (`rcfd-mcp --project <dir>`), its run records, stored
//! fields and the run service. They are listed only when the server was given a project; without one the tool list
//! is unchanged (F1).
//!
//! Paths are project-relative and resolved with `rcfd_project::paths::resolve_in` (refusals are error 1004); run
//! arguments must be run ids. Results stay under [`TEXT_BUDGET`] bytes of text: lists are paged, series decimated
//! (the min/max rule of the frozen GUI's monitors, ADR 0025 M3), files read in pages, and a residual set or image
//! over the budget is written under `runs/<id>/` and returned as a path. Edits use the base-hash compare-and-replace
//! of `rcfd_project::atomic::replace_if_hash` (W48-15 (a)).

use crate::case_edit;
use crate::policy::Access;
use crate::{obj, ok_result, Tool};
use rcfd_api::case::{Case, Diagnostic};
use rcfd_api::service::{base64, RpcError};
use rcfd_project::records::{hash_bytes, RunState};
use rcfd_project::{atomic, paths, Project, ProjectError, RunSummary, Watcher};
use rcfd_runner::client::Client;
use rcfd_runner::records::RunSettings;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Error code: the file changed since its base hash was read (`data.current_hash`, null when absent).
pub const CONFLICT: i64 = 1200;
/// Error code: another cooperating writer held the file's edit lock.
pub const LOCK_BUSY: i64 = 1201;
/// Error code: no such run, file, field or snapshot.
pub const NOT_FOUND: i64 = 1202;
const BAD_STATE: i64 = 1002;
const VALIDATION: i64 = 1003;
const REFUSED: i64 = 1004;
const INTERNAL: i64 = -32000;
const PARAMS: i64 = -32602;

/// Largest result text (bytes of the JSON text) a project tool returns; ADR 0135 F8 limits the whole result to 40 000.
pub const TEXT_BUDGET: usize = 36_000;
/// Largest page of `read_project_file`.
pub const FILE_PAGE: usize = 32_000;
/// Longest `wait_run`.
pub const MAX_WAIT_S: f64 = 60.0;

const STATES: [&str; 6] = ["queued", "running", "paused", "finished", "failed", "cancelled"];

fn err(code: i64, message: impl Into<String>) -> RpcError {
    RpcError {
        code,
        message: message.into(),
        data: None,
    }
}

/// A project-library error as a tool error.
fn perr(e: ProjectError) -> RpcError {
    let code = match &e {
        ProjectError::PathRefused { .. } => REFUSED,
        ProjectError::Io { source, .. } | ProjectError::Retry { source, .. }
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            NOT_FOUND
        }
        ProjectError::Conflict { current, .. } => {
            return RpcError {
                code: CONFLICT,
                message: e.to_string(),
                data: Some(json!({"current_hash": current})),
            }
        }
        ProjectError::LockBusy { .. } => LOCK_BUSY,
        _ => INTERNAL,
    };
    err(code, e.to_string())
}

fn run_schema() -> Value {
    json!({"type": "string", "description": "run id (r-<yyyymmddThhmmss>-<4 hex>, see list_runs)"})
}

fn state_schema() -> Value {
    json!({"type": "string", "enum": STATES})
}

fn snapshot_props(extra: Value) -> Value {
    let mut p = json!({
        "run": run_schema(),
        "field": {"type": "string", "description": "stored field name (default store: ux, uy, uz, p)"},
        "step": {"type": "integer", "minimum": 0, "description": "snapshot by step (with pick nearest or floor)"},
        "time": {"type": "number", "description": "snapshot by simulated time"},
        "pick": {"type": "string", "enum": ["nearest", "floor", "linear"], "description": "how a step or time is matched (default nearest; linear interpolates in time)"}
    });
    for (k, v) in extra.as_object().into_iter().flatten() {
        p[k] = v.clone();
    }
    p
}

/// The project tools, in `tools/list` order.
pub(crate) fn tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "project_info",
            description: "The project folder: manifest, counts of cases, meshes, designs, notes and runs (by state), and the problems validate finds.",
            access: Access::Read,
            schema: || obj(json!({}), &[]),
        },
        Tool {
            name: "list_runs",
            description: "Runs of the project, oldest first, with state, step, time and numeric metrics; run folders that cannot be read are listed as skipped with the reason. Filter by state or case; paged (offset, limit; next_offset when more).",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "state": state_schema(),
                        "case": {"type": "string", "description": "project-relative case path, e.g. cases/cavity.toml"},
                        "offset": {"type": "integer", "minimum": 0},
                        "limit": {"type": "integer", "minimum": 1, "maximum": 100}
                    }),
                    &[],
                )
            },
        },
        Tool {
            name: "run_status",
            description: "One run: its record (case, created, solver version, settings), status (state, step, time, reason, exit code), the last residual line and the last events.",
            access: Access::Read,
            schema: || obj(json!({"run": run_schema(), "events": {"type": "integer", "minimum": 0, "maximum": 50}}), &["run"]),
        },
        Tool {
            name: "read_metrics",
            description: "metrics.json (final report values) of one or more runs (at most 50).",
            access: Access::Read,
            schema: || obj(json!({"runs": {"type": "array", "items": {"type": "string"}}}), &["runs"]),
        },
        Tool {
            name: "read_residuals",
            description: "Residual and monitor series of a run (residuals.<equation>, monitors.<name>) against the step, each decimated to at most max_points (default 200) keeping first, last and every bucket's minimum and maximum. A result over the size budget is written to runs/<id>/mcp/ and returned as a path.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "run": run_schema(),
                        "names": {"type": "array", "items": {"type": "string"}, "description": "series to return (default all)"},
                        "max_points": {"type": "integer", "minimum": 2, "maximum": 2000},
                        "since_step": {"type": "integer", "minimum": 0}
                    }),
                    &["run"],
                )
            },
        },
        Tool {
            name: "field_summary",
            description: "Minimum, maximum and mean (cell-volume weighted for cell fields) of a field stored in a run's field store (runs/<id>/fields), at a step or time (nearest, floor or linear in time); default the last snapshot.",
            access: Access::Read,
            schema: || obj(snapshot_props(json!({})), &["run", "field"]),
        },
        Tool {
            name: "render_field",
            description: "PNG image of a stored cell field of a 2D case at a step or time (default the last snapshot), drawn by the CPU renderer of render_image. An image over the size budget is written to runs/<id>/renders/ and returned as a path.",
            access: Access::Read,
            schema: || {
                obj(
                    snapshot_props(json!({"width": {"type": "integer", "minimum": 64, "maximum": 4096}})),
                    &["run", "field"],
                )
            },
        },
        Tool {
            name: "compare_runs",
            description: "Metrics of 2 to 10 runs side by side with the differences to the first run, and each run's state and residual history length.",
            access: Access::Read,
            schema: || obj(json!({"runs": {"type": "array", "items": {"type": "string"}}}), &["runs"]),
        },
        Tool {
            name: "list_changes",
            description: "Project-relative paths created, changed or deleted since a change token. Call without since to get the current token; .rcfd/ is not watched.",
            access: Access::Read,
            schema: || obj(json!({"since": {"type": "integer", "minimum": 0}}), &[]),
        },
        Tool {
            name: "read_project_file",
            description: "Text of a project file (a case, mesh recipe or note) with the hash of the whole file, which is the base_hash for edit_case and write_note. Read in pages of at most 32000 bytes (offset, limit; next_offset when more).",
            access: Access::Read,
            schema: || {
                obj(
                    json!({
                        "path": {"type": "string", "description": "project-relative path"},
                        "offset": {"type": "integer", "minimum": 0},
                        "limit": {"type": "integer", "minimum": 1, "maximum": 32000}
                    }),
                    &["path"],
                )
            },
        },
        Tool {
            name: "edit_case",
            description: "Set dotted settings (e.g. physics.nu, solver.max_iterations) in a .toml or .json case file. The edited case is validated as set_setting does; the file is replaced atomically only if valid and only if it still has base_hash (else error 1200 with the current hash). The changed lines are edited in place (comments kept) when possible. dry_run validates without writing.",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "path": {"type": "string", "description": "project-relative case file (.toml or .json)"},
                        "base_hash": {"type": "string", "description": "hash from read_project_file"},
                        "set": {"type": "object", "description": "{dotted setting path: value}"},
                        "dry_run": {"type": "boolean"}
                    }),
                    &["path", "base_hash", "set"],
                )
            },
        },
        Tool {
            name: "write_note",
            description: "Create or replace a text file under notes/. Without base_hash the file must not exist yet; with it the file is replaced only if it still has that hash (else error 1200 with the current hash).",
            access: Access::WriteFiles,
            schema: || {
                obj(
                    json!({
                        "path": {"type": "string", "description": "project-relative path under notes/"},
                        "text": {"type": "string"},
                        "base_hash": {"type": "string"}
                    }),
                    &["path", "text"],
                )
            },
        },
        Tool {
            name: "start_run",
            description: "Queue a run of a project case with the project's run service and start the service if none is alive. set holds rcfd run --set overrides (path=value). Returns the run id.",
            access: Access::Execute,
            schema: || {
                obj(
                    json!({
                        "case": {"type": "string", "description": "project-relative case file"},
                        "set": {"type": "array", "items": {"type": "string"}, "description": "overrides path=value, in order"},
                        "priority": {"type": "integer"},
                        "threads": {"type": "integer", "minimum": 1, "maximum": 256},
                        "mem_mb": {"type": "integer", "minimum": 1}
                    }),
                    &["case"],
                )
            },
        },
        Tool {
            name: "pause_run",
            description: "Hold a run: a queued run is not launched, a running one stops between two steps.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema()}), &["run"]),
        },
        Tool {
            name: "resume_run",
            description: "Release a held run.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema()}), &["run"]),
        },
        Tool {
            name: "cancel_run",
            description: "Cancel a run; a running solver stops cleanly and writes its results.",
            access: Access::Execute,
            schema: || obj(json!({"run": run_schema()}), &["run"]),
        },
        Tool {
            name: "wait_run",
            description: "Wait until a run's state differs from from (default: its state now) or the run is finished, failed or cancelled, for at most timeout_s seconds (default 30, at most 60). Call again to keep waiting.",
            access: Access::Read,
            schema: || {
                obj(
                    json!({"run": run_schema(), "timeout_s": {"type": "number", "minimum": 0.0, "maximum": MAX_WAIT_S}, "from": state_schema()}),
                    &["run"],
                )
            },
        },
    ]
}

/// The project tools' state: the open project, its run-service client and the change watcher (made on first use).
pub struct ProjectTools {
    project: Project,
    client: Client,
    watcher: Mutex<Option<Watcher>>,
}

impl std::fmt::Debug for ProjectTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectTools")
            .field("root", &self.project.root())
            .finish()
    }
}

fn state_name(s: RunState) -> String {
    rcfd_runner::client::state_name(s)
}

fn summary_json(s: &RunSummary) -> Value {
    json!({"id": s.id, "case": s.case, "created": s.created, "state": state_name(s.state), "step": s.step,
        "time": s.time, "metrics": s.metrics})
}

fn text_len(v: &Value) -> usize {
    v.to_string().len()
}

/// Neumaier-compensated sum.
fn neumaier(xs: impl Iterator<Item = f64>) -> f64 {
    let (mut s, mut c) = (0.0f64, 0.0f64);
    for x in xs {
        let t = s + x;
        c += if s.abs() >= x.abs() { (s - t) + x } else { (x - t) + s };
        s = t;
    }
    s + c
}

/// Min/max decimation to at most `max_points`: first and last kept, the interior split into buckets that each keep
/// their minimum and maximum. Copied from `rcfd-gui/src/monitors.rs` (`decimate_minmax`, ADR 0025 M3; the frozen GUI
/// is not a dependency of this crate).
pub fn decimate_minmax(points: &[[f64; 2]], max_points: usize) -> Vec<[f64; 2]> {
    let n = points.len();
    if n <= max_points {
        return points.to_vec();
    }
    if max_points < 4 {
        return vec![points[0], points[n - 1]];
    }
    let nb = (max_points - 2) / 2;
    let inner = n - 2;
    let mut keep: Vec<usize> = Vec::with_capacity(2 * nb + 2);
    keep.push(0);
    for b in 0..nb {
        let (s, e) = (1 + b * inner / nb, 1 + (b + 1) * inner / nb);
        if s >= e {
            continue;
        }
        let (mut imin, mut imax) = (s, s);
        for i in s..e {
            if points[i][1] < points[imin][1] {
                imin = i;
            }
            if points[i][1] > points[imax][1] {
                imax = i;
            }
        }
        if imin == imax {
            keep.push(imin);
        } else {
            keep.push(imin.min(imax));
            keep.push(imin.max(imax));
        }
    }
    keep.push(n - 1);
    keep.into_iter().map(|i| points[i]).collect()
}

/// Series of `residuals.jsonl` bytes: `residuals.<eq>` and `monitors.<name>` against the step, decimated to
/// `max_points` each (`None`: no decimation). Returns (series, complete lines read, lines that did not parse).
pub fn residual_series(
    bytes: &[u8],
    since_step: u64,
    max_points: Option<usize>,
) -> (BTreeMap<String, Vec<[f64; 2]>>, usize, usize) {
    let committed = bytes.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let mut series: BTreeMap<String, Vec<[f64; 2]>> = BTreeMap::new();
    let (mut lines, mut bad) = (0, 0);
    for line in bytes[..committed].split(|&b| b == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(line) else {
            bad += 1;
            continue;
        };
        let Some(step) = v.get("step").and_then(Value::as_u64) else {
            bad += 1;
            continue;
        };
        lines += 1;
        if step < since_step {
            continue;
        }
        for group in ["residuals", "monitors"] {
            for (k, x) in v.get(group).and_then(Value::as_object).into_iter().flatten() {
                series
                    .entry(format!("{group}.{k}"))
                    .or_default()
                    .push([step as f64, x.as_f64().unwrap_or(f64::NAN)]);
            }
        }
    }
    if let Some(m) = max_points {
        for s in series.values_mut() {
            *s = decimate_minmax(s, m);
        }
    }
    (series, lines, bad)
}

fn series_json(series: &BTreeMap<String, Vec<[f64; 2]>>) -> Value {
    Value::Object(
        series
            .iter()
            .map(|(k, s)| {
                (
                    k.clone(),
                    json!({"x": s.iter().map(|p| p[0]).collect::<Vec<_>>(), "y": s.iter().map(|p| p[1]).collect::<Vec<_>>()}),
                )
            })
            .collect(),
    )
}

/// The last `n` complete lines of a file, read from its last `max_bytes` bytes (missing file: none).
fn tail_lines(path: &Path, n: usize, max_bytes: u64) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(max_bytes);
    let mut buf = Vec::new();
    if f.seek(SeekFrom::Start(start)).is_err() || f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let committed = buf.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let text = String::from_utf8_lossy(&buf[..committed]);
    let mut lines: Vec<&str> = text.split('\n').collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0); // a partial first line
    }
    let lines: Vec<String> = lines
        .into_iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();
    lines[lines.len().saturating_sub(n)..].to_vec()
}

fn u64_arg(args: &Value, k: &str) -> Option<u64> {
    args.get(k).and_then(Value::as_u64)
}

fn str_arg<'a>(args: &'a Value, k: &str) -> &'a str {
    args.get(k).and_then(Value::as_str).unwrap_or_default()
}

/// File name part made of `[A-Za-z0-9_-]` only.
fn file_part(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl ProjectTools {
    /// Open the project folder `dir` (a located error when it is not a project). Runs are started with the
    /// `rcfd-runner` next to the current executable or named by `RCFD_RUNNER_BIN`.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let project =
            Project::open(dir).map_err(|e| format!("{} is not an rcfd project folder: {e}", dir.display()))?;
        let client = Client::with_default_runner(project.root());
        Ok(ProjectTools {
            project,
            client,
            watcher: Mutex::new(None),
        })
    }

    /// Start runs with the runner executable `exe` instead.
    pub fn with_runner_exe(mut self, exe: impl Into<PathBuf>) -> Self {
        self.client = Client::new(self.project.root(), exe);
        self
    }

    /// Canonical project root.
    pub fn root(&self) -> &Path {
        self.project.root()
    }

    fn rel(&self, p: &Path) -> String {
        paths::rel_string(self.project.root(), p).unwrap_or_else(|| p.display().to_string())
    }

    /// A run argument: a run id whose folder holds `run.json`.
    fn run_arg(&self, args: &Value, key: &str) -> Result<(String, PathBuf), RpcError> {
        let id = str_arg(args, key);
        let dir = self.project.run_dir(id).map_err(perr)?;
        if !dir.join("run.json").is_file() {
            return Err(err(
                NOT_FOUND,
                format!("run {id}: no such run (runs/{id}/run.json is missing)"),
            ));
        }
        Ok((id.to_string(), dir))
    }

    /// Execute project tool `name`; `None` if it is not one.
    pub(crate) fn call(&self, name: &str, args: &Value) -> Option<Result<Value, RpcError>> {
        Some(match name {
            "project_info" => self.project_info(),
            "list_runs" => self.list_runs(args),
            "run_status" => self.run_status(args),
            "read_metrics" => self.read_metrics(args),
            "read_residuals" => self.read_residuals(args),
            "field_summary" => self.field_summary(args),
            "render_field" => self.render_field(args),
            "compare_runs" => self.compare_runs(args),
            "list_changes" => self.list_changes(args),
            "read_project_file" => self.read_project_file(args),
            "edit_case" => self.edit_case(args),
            "write_note" => self.write_note(args),
            "start_run" => self.start_run(args),
            "pause_run" | "resume_run" | "cancel_run" => self.control(name, args),
            "wait_run" => self.wait_run(args),
            _ => return None,
        })
    }

    fn project_info(&self) -> Result<Value, RpcError> {
        let p = &self.project;
        let m = p.manifest();
        let runs = p.runs().map_err(perr)?;
        let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
        for r in &runs.runs {
            *by_state.entry(state_name(r.state)).or_default() += 1;
        }
        let issues = p.validate().map_err(perr)?;
        let notes = count_files(&p.root().join("notes"));
        let mut v = json!({
            "name": m.name, "root": p.root().display().to_string(), "schema_version": m.schema_version,
            "created": m.created, "solver_version_created": m.solver_version_created, "migrated_from": p.migrated_from(),
            "counts": {"cases": p.cases().map_err(perr)?.len(), "meshes": p.meshes().map_err(perr)?.len(),
                "designs": p.designs().map_err(perr)?.len(), "notes": notes, "runs": runs.runs.len(),
                "skipped_runs": runs.skipped.len()},
            "runs_by_state": by_state,
            "issues": [],
            "issues_total": issues.len(),
        });
        let mut list = Vec::new();
        for i in issues.iter().take(50) {
            list.push(json!({"path": self.rel(&i.path), "message": i.message}));
            v["issues"] = json!(list);
            if text_len(&v) > TEXT_BUDGET {
                list.pop();
                v["issues"] = json!(list);
                break;
            }
        }
        Ok(ok_result(v))
    }

    fn list_runs(&self, args: &Value) -> Result<Value, RpcError> {
        let all = self.project.runs().map_err(perr)?;
        let norm = |s: &str| {
            s.replace('\\', "/")
                .split('/')
                .filter(|c| !c.is_empty() && *c != ".")
                .collect::<Vec<_>>()
                .join("/")
        };
        let state = args.get("state").and_then(Value::as_str);
        let case = args.get("case").and_then(Value::as_str).map(norm);
        let rows: Vec<&RunSummary> = all
            .runs
            .iter()
            .filter(|r| state.is_none_or(|s| state_name(r.state) == s))
            .filter(|r| case.as_ref().is_none_or(|c| &norm(&r.case) == c))
            .collect();
        let offset = u64_arg(args, "offset").unwrap_or(0) as usize;
        let limit = u64_arg(args, "limit").unwrap_or(50) as usize;
        let skipped: Vec<Value> = all
            .skipped
            .iter()
            .take(20)
            .map(|i| json!({"path": self.rel(&i.path), "message": i.message}))
            .collect();
        let mut v = json!({"runs": [], "total": rows.len(), "offset": offset, "next_offset": null,
            "skipped": skipped, "skipped_total": all.skipped.len()});
        let mut size = text_len(&v);
        let mut out = Vec::new();
        for r in rows.iter().skip(offset).take(limit) {
            let row = summary_json(r);
            let n = text_len(&row) + 1;
            if size + n > TEXT_BUDGET {
                break;
            }
            size += n;
            out.push(row);
        }
        let end = offset + out.len();
        v["runs"] = json!(out);
        if end < rows.len() {
            v["next_offset"] = json!(end);
        }
        Ok(ok_result(v))
    }

    fn run_status(&self, args: &Value) -> Result<Value, RpcError> {
        let (id, dir) = self.run_arg(args, "run")?;
        let rec = self.project.read_run(&id).map_err(perr)?;
        let st = self.project.read_status(&id).map_err(perr)?;
        let last = tail_lines(&dir.join("residuals.jsonl"), 1, 64 * 1024)
            .pop()
            .and_then(|l| serde_json::from_str::<Value>(&l).ok());
        let n = u64_arg(args, "events").unwrap_or(10) as usize;
        let mut events: Vec<Value> = tail_lines(&dir.join("events.jsonl"), n, 256 * 1024)
            .iter()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        let mut v = json!({"run": id, "case": rec.case, "created": rec.created, "solver_version": rec.solver_version,
            "settings": rec.settings, "status": serde_json::to_value(&st).unwrap_or(Value::Null),
            "last_residual": last, "events": events});
        while text_len(&v) > TEXT_BUDGET && !events.is_empty() {
            events.remove(0);
            v["events"] = json!(events);
        }
        if text_len(&v) > TEXT_BUDGET {
            v["settings"] = json!("(omitted: over the size budget; read runs/<id>/run.json)");
        }
        Ok(ok_result(v))
    }

    fn run_list_arg(&self, args: &Value, lo: usize, hi: usize) -> Result<Vec<(String, PathBuf)>, RpcError> {
        let ids = args["runs"].as_array().cloned().unwrap_or_default();
        if ids.len() < lo || ids.len() > hi {
            return Err(err(
                PARAMS,
                format!("runs: give {lo} to {hi} run ids, got {}", ids.len()),
            ));
        }
        ids.iter().map(|i| self.run_arg(&json!({"run": i}), "run")).collect()
    }

    fn read_metrics(&self, args: &Value) -> Result<Value, RpcError> {
        let runs = self.run_list_arg(args, 1, 50)?;
        let mut metrics = Map::new();
        let mut omitted = Vec::new();
        let mut size = 64;
        for (id, dir) in &runs {
            let p = dir.join("metrics.json");
            let m = if p.is_file() {
                atomic::read_json::<Value>(&p).map_err(perr)?
            } else {
                Value::Null
            };
            let n = text_len(&m) + id.len() + 4;
            if size + n > TEXT_BUDGET {
                omitted.push(id.clone());
                continue;
            }
            size += n;
            metrics.insert(id.clone(), m);
        }
        Ok(ok_result(json!({"metrics": metrics, "omitted": omitted})))
    }

    fn read_residuals(&self, args: &Value) -> Result<Value, RpcError> {
        let (id, dir) = self.run_arg(args, "run")?;
        let bytes = match std::fs::read(dir.join("residuals.jsonl")) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(err(INTERNAL, format!("runs/{id}/residuals.jsonl: {e}"))),
        };
        let max_points = u64_arg(args, "max_points").unwrap_or(200) as usize;
        let since = u64_arg(args, "since_step").unwrap_or(0);
        let (mut series, lines, bad) = residual_series(&bytes, since, None);
        let longest = series.values().map(Vec::len).max().unwrap_or(0);
        let mut unknown = Vec::new();
        if let Some(names) = args.get("names").and_then(Value::as_array) {
            let want: BTreeSet<&str> = names.iter().filter_map(Value::as_str).collect();
            unknown = want
                .iter()
                .filter(|n| !series.contains_key(**n))
                .map(|n| n.to_string())
                .collect();
            series.retain(|k, _| want.contains(k.as_str()));
        }
        for s in series.values_mut() {
            *s = decimate_minmax(s, max_points);
        }
        let mut v = json!({"run": id, "lines": lines, "skipped_lines": bad, "max_points": max_points,
            "decimated": longest > max_points, "unknown_names": unknown, "series": series_json(&series)});
        if text_len(&v) > TEXT_BUDGET {
            let rel = format!("runs/{id}/mcp/residuals-{max_points}-from{since}.json");
            let path = self.project.root().join(&rel);
            std::fs::create_dir_all(path.parent().expect("has a parent"))
                .map_err(|e| err(INTERNAL, format!("{rel}: {e}")))?;
            let text = v.to_string();
            atomic::atomic_write(&path, text.as_bytes()).map_err(perr)?;
            v = json!({"run": id, "lines": lines, "skipped_lines": bad, "max_points": max_points,
                "decimated": longest > max_points, "unknown_names": unknown,
                "series_names": series.keys().collect::<Vec<_>>(), "path": rel, "bytes": text.len(),
                "note": "the series are over the size budget and were written to path; lower max_points or pick names to get them inline"});
        }
        Ok(ok_result(v))
    }

    /// The stored field a snapshot tool asks for: (reader, values, field info, snapshot json).
    fn sample(&self, args: &Value) -> Result<Sampled, RpcError> {
        use rcfd_tsstore::{Mode, Pick, Reader, STORE_FILE};
        let (id, rundir) = self.run_arg(args, "run")?;
        let store = rundir.join("fields");
        if !store.join(STORE_FILE).is_file() {
            return Err(err(
                NOT_FOUND,
                format!("run {id} has no stored fields (runs/{id}/fields/{STORE_FILE}); runs store fields when the case sets output.store_interval or output.store_time_interval"),
            ));
        }
        let ts = |e: &dyn std::fmt::Display| err(INTERNAL, format!("runs/{id}/fields: {e}"));
        let mut r = Reader::open(&store).map_err(|e| ts(&e))?;
        let n_snaps = r.snapshots().len();
        if n_snaps == 0 {
            return Err(err(NOT_FOUND, format!("runs/{id}/fields holds no snapshot yet")));
        }
        let field = str_arg(args, "field").to_string();
        let pick = match args.get("pick").and_then(Value::as_str).unwrap_or("nearest") {
            "floor" => Some(Pick::Floor),
            "linear" => None,
            _ => Some(Pick::Nearest),
        };
        let none = |what: &str| err(NOT_FOUND, format!("runs/{id}/fields: no snapshot for {what}"));
        let (values, lower, upper, weight, interpolated) =
            match (u64_arg(args, "step"), args.get("time").and_then(Value::as_f64)) {
                (Some(_), Some(_)) => return Err(err(PARAMS, "give step or time, not both")),
                (Some(s), None) => {
                    let Some(p) = pick else {
                        return Err(err(PARAMS, "pick linear needs a time, not a step"));
                    };
                    let i = r.by_step(s, p).ok_or_else(|| none(&format!("step {s}")))?;
                    (None, i, i, 0.0, false)
                }
                (None, Some(t)) => {
                    let mode = pick.map_or(Mode::Linear, Mode::Pick);
                    let i = match mode {
                        Mode::Pick(p) => r.by_time(t, p).ok_or_else(|| none(&format!("time {t}")))?,
                        Mode::Linear => r.by_time(t, Pick::Floor).unwrap_or(0),
                    };
                    self.check_field(&mut r, i, &id, &field)?;
                    let s = r
                        .field_at_time(&field, t, mode)
                        .map_err(|e| err(NOT_FOUND, format!("runs/{id}/fields: {e}")))?;
                    (Some(s.values), s.lower, s.upper, s.weight, s.interpolated)
                }
                (None, None) => (None, n_snaps - 1, n_snaps - 1, 0.0, false),
            };
        let info = self.check_field(&mut r, lower, &id, &field)?;
        let values = match values {
            Some(v) => v,
            None => r.read_field(lower, &field).map_err(|e| ts(&e))?,
        };
        let snaps = r.snapshots();
        let snap = |i: usize| json!({"index": i, "step": snaps[i].step, "time": snaps[i].time});
        let mut snapshot = snap(lower);
        if interpolated {
            snapshot["upper"] = snap(upper);
            snapshot["weight"] = json!(weight);
        }
        let step = snaps[lower].step;
        Ok(Sampled {
            id,
            reader: r,
            values,
            info,
            snapshot,
            interpolated,
            step,
        })
    }

    fn check_field(
        &self,
        r: &mut rcfd_tsstore::Reader,
        i: usize,
        id: &str,
        field: &str,
    ) -> Result<rcfd_tsstore::FieldInfo, RpcError> {
        let fields = r
            .fields(i)
            .map_err(|e| err(INTERNAL, format!("runs/{id}/fields: {e}")))?;
        let names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
        fields.into_iter().find(|f| f.name == field).ok_or_else(|| RpcError {
            code: NOT_FOUND,
            message: format!("runs/{id}/fields: snapshot {i} has no field '{field}'"),
            data: Some(json!({"fields": names})),
        })
    }

    fn field_summary(&self, args: &Value) -> Result<Value, RpcError> {
        let mut s = self.sample(args)?;
        let finite: Vec<usize> = (0..s.values.len()).filter(|&i| s.values[i].is_finite()).collect();
        let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
        for &i in &finite {
            min = min.min(s.values[i]);
            max = max.max(s.values[i]);
        }
        let cell = s.info.location == rcfd_tsstore::Location::Cell;
        let vols = if cell {
            let m = s
                .reader
                .mesh()
                .map_err(|e| err(INTERNAL, format!("runs/{}/fields: {e}", s.id)))?;
            Some(m.cell_volumes().to_vec()).filter(|v| v.len() == s.values.len())
        } else {
            None
        };
        let (mean, weighting) = match &vols {
            Some(v) => (
                neumaier(finite.iter().map(|&i| s.values[i] * v[i])) / neumaier(finite.iter().map(|&i| v[i])),
                "cell_volume",
            ),
            None => (
                neumaier(finite.iter().map(|&i| s.values[i])) / finite.len() as f64,
                "none",
            ),
        };
        let none = finite.is_empty();
        Ok(ok_result(json!({
            "run": s.id, "field": s.info.name, "unit": s.info.unit, "location": s.info.location,
            "precision": s.info.precision, "snapshot": s.snapshot, "interpolated": s.interpolated,
            "n": s.values.len(), "finite": finite.len(),
            "min": if none { Value::Null } else { json!(min) }, "max": if none { Value::Null } else { json!(max) },
            "mean": if none { Value::Null } else { json!(mean) }, "weighting": weighting,
        })))
    }

    fn render_field(&self, args: &Value) -> Result<Value, RpcError> {
        use rcfd_post::render::{render_scalar, RenderOptions};
        let mut s = self.sample(args)?;
        if s.info.location != rcfd_tsstore::Location::Cell {
            return Err(err(
                VALIDATION,
                format!("render_field draws cell fields; '{}' is a point field", s.info.name),
            ));
        }
        let mesh = s
            .reader
            .mesh()
            .map_err(|e| err(INTERNAL, format!("runs/{}/fields: {e}", s.id)))?;
        let width = u64_arg(args, "width").unwrap_or(640).clamp(64, 4096) as usize;
        let opt = RenderOptions {
            width,
            ..Default::default()
        };
        let r = render_scalar(&mesh, &s.values, &opt).map_err(|e| err(VALIDATION, e.to_string()))?;
        if r.cells_drawn != mesh.n_cells() {
            return Err(err(
                VALIDATION,
                format!(
                    "render_field draws 2D cases only (one cell layer): the view plane cuts {} of {} cells of runs/{}",
                    r.cells_drawn,
                    mesh.n_cells(),
                    s.id
                ),
            ));
        }
        let png = r.image.to_png().map_err(|e| err(INTERNAL, e.to_string()))?;
        let b64 = base64(&png);
        let mut meta = json!({"run": s.id, "field": s.info.name, "snapshot": s.snapshot, "interpolated": s.interpolated,
            "min": r.vmin, "max": r.vmax, "width": r.image.width, "height": r.image.height, "png_bytes": png.len()});
        if b64.len() + text_len(&meta) > TEXT_BUDGET {
            let rel = format!(
                "runs/{}/renders/{}-step{}-w{width}{}.png",
                s.id,
                file_part(&s.info.name),
                s.step,
                if s.interpolated { "-interp" } else { "" }
            );
            let path = self.project.root().join(&rel);
            std::fs::create_dir_all(path.parent().expect("has a parent"))
                .map_err(|e| err(INTERNAL, format!("{rel}: {e}")))?;
            atomic::atomic_write(&path, &png).map_err(perr)?;
            meta["path"] = json!(rel);
            meta["note"] = json!("the image is over the size budget and was written to path");
            return Ok(ok_result(meta));
        }
        Ok(json!({
            "content": [
                {"type": "image", "data": b64, "mimeType": "image/png"},
                {"type": "text", "text": meta.to_string()}
            ],
            "structuredContent": meta,
            "isError": false
        }))
    }

    fn compare_runs(&self, args: &Value) -> Result<Value, RpcError> {
        let runs = self.run_list_arg(args, 2, 10)?;
        let mut rows = Vec::new();
        let mut sums = Vec::new();
        for (id, dir) in &runs {
            let s = self.project.run_summary(id).map_err(perr)?;
            let lines = std::fs::read(dir.join("residuals.jsonl"))
                .map(|b| b.iter().filter(|&&c| c == b'\n').count())
                .unwrap_or(0);
            rows.push(
                json!({"id": id, "case": s.case, "state": state_name(s.state), "step": s.step, "time": s.time,
                "residual_lines": lines}),
            );
            sums.push(s);
        }
        let names: BTreeSet<&String> = sums.iter().flat_map(|s| s.metrics.keys()).collect();
        let base = &sums[0];
        let mut metrics = Map::new();
        let mut diffs = Map::new();
        let mut v = json!({"baseline": base.id, "runs": rows, "metrics": {}, "differences": {}, "metrics_total": names.len(),
            "metrics_omitted": 0});
        let mut size = text_len(&v);
        let mut omitted = 0;
        for name in &names {
            let vals: Map<String, Value> = sums
                .iter()
                .filter_map(|s| s.metrics.get(*name).map(|x| (s.id.clone(), json!(x))))
                .collect();
            let d: Map<String, Value> = match base.metrics.get(*name) {
                Some(b) => sums[1..]
                    .iter()
                    .filter_map(|s| s.metrics.get(*name).map(|x| (s.id.clone(), json!(x - b))))
                    .collect(),
                None => Map::new(),
            };
            let n = text_len(&Value::Object(vals.clone())) + text_len(&Value::Object(d.clone())) + 2 * name.len() + 8;
            if size + n > TEXT_BUDGET {
                omitted += 1;
                continue;
            }
            size += n;
            metrics.insert((*name).clone(), Value::Object(vals));
            diffs.insert((*name).clone(), Value::Object(d));
        }
        v["metrics"] = Value::Object(metrics);
        v["differences"] = Value::Object(diffs);
        v["metrics_omitted"] = json!(omitted);
        Ok(ok_result(v))
    }

    fn list_changes(&self, args: &Value) -> Result<Value, RpcError> {
        let mut g = self.watcher.lock().unwrap_or_else(|e| e.into_inner());
        if g.is_none() {
            *g = Some(Watcher::new(self.project.root()).map_err(perr)?);
        }
        let w = g.as_mut().expect("made above");
        let token = w.poll().map_err(perr)?;
        let Some(since) = u64_arg(args, "since") else {
            return Ok(ok_result(
                json!({"token": token, "changes": [], "total": 0, "truncated": false}),
            ));
        };
        let all = w.changes_since(since);
        let mut out = Vec::new();
        let mut size = 128;
        for p in &all {
            size += p.len() + 3;
            if size > TEXT_BUDGET {
                break;
            }
            out.push(p.clone());
        }
        Ok(ok_result(json!({"since": since, "token": token, "total": all.len(),
            "truncated": out.len() < all.len(), "changes": out})))
    }

    fn read_project_file(&self, args: &Value) -> Result<Value, RpcError> {
        let rel = str_arg(args, "path");
        let path = self.project.resolve(rel).map_err(perr)?;
        if path.is_dir() {
            return Err(err(NOT_FOUND, format!("{rel} is a directory")));
        }
        let bytes = atomic::read(&path).map_err(perr)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|e| err(VALIDATION, format!("{rel} is not UTF-8 text ({e})")))?;
        let offset = u64_arg(args, "offset").unwrap_or(0) as usize;
        if offset > text.len() || !text.is_char_boundary(offset) {
            return Err(err(
                PARAMS,
                format!(
                    "offset {offset} is not a character boundary of {rel} ({} bytes)",
                    text.len()
                ),
            ));
        }
        let limit = u64_arg(args, "limit").unwrap_or(FILE_PAGE as u64) as usize;
        let mut end = (offset + limit.min(FILE_PAGE)).min(text.len());
        loop {
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            // JSON escapes can grow the text (a control character takes 6 bytes): halve the page until it fits
            if serde_json::to_string(&text[offset..end]).map_or(0, |s| s.len()) <= TEXT_BUDGET - 1000 || end == offset {
                break;
            }
            end = offset + (end - offset) / 2;
        }
        Ok(ok_result(json!({
            "path": self.rel(&path), "hash": hash_bytes(&bytes), "size": bytes.len(), "offset": offset,
            "text": &text[offset..end], "next_offset": if end < text.len() { json!(end) } else { Value::Null },
        })))
    }

    fn edit_case(&self, args: &Value) -> Result<Value, RpcError> {
        let rel = str_arg(args, "path");
        let path = self.project.resolve(rel).map_err(perr)?;
        let norm = self.rel(&path);
        if norm.starts_with("runs/") || norm.starts_with(".rcfd/") || norm == "rcfd-project.json" {
            return Err(err(
                REFUSED,
                format!("edit_case does not write {norm}: run records, .rcfd/ and the manifest are not case files"),
            ));
        }
        let is_json = match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => false,
            Some("json") => true,
            _ => {
                return Err(err(
                    VALIDATION,
                    format!("{norm}: edit_case edits .toml or .json case files"),
                ))
            }
        };
        let base = str_arg(args, "base_hash");
        let set = args["set"].as_object().cloned().unwrap_or_default();
        if set.is_empty() {
            return Err(err(PARAMS, "set: give at least one {setting path: value}"));
        }
        let bytes = atomic::read(&path).map_err(perr)?;
        let old_hash = hash_bytes(&bytes);
        if !rcfd_project::records::hash_matches(base, &bytes) {
            return Err(RpcError {
                code: CONFLICT,
                message: format!("{norm}: changed since it was read (now {old_hash}); read it again"),
                data: Some(json!({"current_hash": old_hash})),
            });
        }
        let text =
            std::str::from_utf8(&bytes).map_err(|e| err(VALIDATION, format!("{norm} is not UTF-8 text ({e})")))?;
        let root = self.project.root();
        let base_dir = path.parent().unwrap_or(root).to_path_buf();
        let parse = |t: &str| {
            if is_json {
                Case::parse_json_with(t, &base_dir, Some(root))
            } else {
                Case::parse_toml_with(t, &base_dir, Some(root))
            }
        };
        let invalid = |what: String, diags: Vec<Diagnostic>| RpcError {
            code: VALIDATION,
            message: format!(
                "{norm}: {what}: {}",
                diags
                    .iter()
                    .map(|d| format!("{}: {}", d.path, d.message))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            data: Some(json!({"diagnostics": diags})),
        };
        let orig = parse(text).map_err(|d| invalid("the file is not a valid case before the edit".into(), d))?;
        let mut expected = orig.clone();
        for (k, v) in &set {
            expected.set(k, v.clone()).map_err(|e| {
                invalid(
                    format!("{k} = {v} refused"),
                    vec![Diagnostic {
                        path: k.clone(),
                        line: None,
                        message: e.to_string(),
                    }],
                )
            })?;
        }
        let sem = expected.semantic_errors();
        if !sem.is_empty() {
            return Err(invalid("the edited case is not valid".into(), sem));
        }
        let want = expected.tree.to_json();
        let minimal = if is_json {
            serde_json::from_str::<Value>(text).ok().and_then(|mut d| {
                for (k, v) in &set {
                    case_edit::json_set(&mut d, k, v)?;
                }
                serde_json::to_string_pretty(&d).ok().map(|s| s + "\n")
            })
        } else {
            let mut t = Some(text.to_string());
            for (k, v) in &set {
                t = t.and_then(|t| case_edit::toml_set(&t, k, v));
            }
            t
        };
        let (new_text, form) = match minimal.filter(|t| parse(t).is_ok_and(|c| c.tree.to_json() == want)) {
            Some(t) => (t, "minimal"),
            None => {
                let t = if is_json {
                    expected.to_json_string() + "\n"
                } else {
                    expected.to_toml()
                };
                match parse(&t) {
                    Ok(c) if c.tree.to_json() == want => (t, "canonical"),
                    Ok(_) => {
                        return Err(err(
                            INTERNAL,
                            format!("{norm}: the canonical form does not re-parse to the edited case"),
                        ))
                    }
                    Err(d) => return Err(invalid("the canonical form does not parse".into(), d)),
                }
            }
        };
        let values: Map<String, Value> = set
            .keys()
            .map(|k| (k.clone(), expected.get(k).unwrap_or(Value::Null)))
            .collect();
        let new_hash = hash_bytes(new_text.as_bytes());
        if args.get("dry_run").and_then(Value::as_bool).unwrap_or(false) {
            return Ok(ok_result(
                json!({"path": norm, "written": false, "valid": true, "form": form,
                "old_hash": old_hash, "new_hash": new_hash, "values": values}),
            ));
        }
        let r = atomic::replace_if_hash(root, &norm, Some(base), new_text.as_bytes(), "edit_case").map_err(perr)?;
        Ok(ok_result(
            json!({"path": r.rel, "written": true, "valid": true, "form": form, "old_hash": r.old_hash,
            "new_hash": r.new_hash, "values": values}),
        ))
    }

    fn write_note(&self, args: &Value) -> Result<Value, RpcError> {
        let rel = str_arg(args, "path");
        let path = self.project.resolve(rel).map_err(perr)?;
        let norm = self.rel(&path);
        if !norm.starts_with("notes/") || norm.len() <= "notes/".len() {
            return Err(err(REFUSED, format!("write_note writes under notes/ only, not {norm}")));
        }
        if path.is_dir() {
            return Err(err(REFUSED, format!("{norm} is a directory")));
        }
        let base = args.get("base_hash").and_then(Value::as_str);
        let r = atomic::replace_if_hash(
            self.project.root(),
            &norm,
            base,
            str_arg(args, "text").as_bytes(),
            "write_note",
        )
        .map_err(perr)?;
        Ok(ok_result(
            json!({"path": r.rel, "created": r.old_hash.is_none(), "old_hash": r.old_hash,
            "new_hash": r.new_hash}),
        ))
    }

    fn start_run(&self, args: &Value) -> Result<Value, RpcError> {
        let rel = str_arg(args, "case");
        let path = self.project.resolve(rel).map_err(perr)?;
        if !path.is_file() {
            return Err(err(NOT_FOUND, format!("{rel}: no such case file")));
        }
        let set: Vec<String> = args
            .get("set")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|s| s.as_str().map(String::from))
            .collect();
        if let Some(bad) = set.iter().find(|s| !s.contains('=')) {
            return Err(err(PARAMS, format!("set: '{bad}' is not path=value")));
        }
        let settings = RunSettings {
            priority: args
                .get("priority")
                .and_then(Value::as_i64)
                .unwrap_or(0)
                .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            threads: u64_arg(args, "threads").unwrap_or(1) as usize,
            mem_mb: u64_arg(args, "mem_mb"),
            set,
        };
        let id = self.client.submit(&path, &settings).map_err(|e| err(INTERNAL, e))?;
        if let Err(e) = self.client.ensure_runner() {
            return Err(RpcError {
                code: INTERNAL,
                message: format!("run {id} is queued, but the run service did not start: {e}"),
                data: Some(json!({"run": id})),
            });
        }
        let st = self.client.status(&id).map_err(|e| err(INTERNAL, e))?;
        Ok(ok_result(
            json!({"run": id, "case": self.rel(&path), "state": state_name(st.state)}),
        ))
    }

    fn control(&self, name: &str, args: &Value) -> Result<Value, RpcError> {
        let (id, _) = self.run_arg(args, "run")?;
        let (r, what) = match name {
            "pause_run" => (self.client.pause(&id), "pause"),
            "resume_run" => (self.client.resume(&id), "resume"),
            _ => (self.client.cancel(&id), "cancel"),
        };
        r.map_err(|e| err(if e.contains("is already") { BAD_STATE } else { INTERNAL }, e))?;
        let st = self.client.status(&id).map_err(|e| err(INTERNAL, e))?;
        Ok(ok_result(
            json!({"run": id, "requested": what, "state": state_name(st.state)}),
        ))
    }

    fn wait_run(&self, args: &Value) -> Result<Value, RpcError> {
        let (id, _) = self.run_arg(args, "run")?;
        let timeout = Duration::from_secs_f64(
            args.get("timeout_s")
                .and_then(Value::as_f64)
                .unwrap_or(30.0)
                .clamp(0.0, MAX_WAIT_S),
        );
        let t0 = Instant::now();
        let status = |id: &str| self.client.status(id).map_err(|e| err(INTERNAL, e));
        let first = status(&id)?;
        let from = match args.get("from") {
            Some(f) => serde_json::from_value::<RunState>(f.clone()).map_err(|e| err(PARAMS, format!("from: {e}")))?,
            None => first.state,
        };
        let mut st = first;
        loop {
            if st.state != from || st.state.is_terminal() || t0.elapsed() >= timeout {
                break;
            }
            std::thread::sleep(Duration::from_millis(50).min(timeout.saturating_sub(t0.elapsed())));
            st = status(&id)?;
        }
        Ok(ok_result(
            json!({"run": id, "from": state_name(from), "state": state_name(st.state),
            "changed": st.state != from, "terminal": st.state.is_terminal(), "waited_s": t0.elapsed().as_secs_f64(),
            "status": serde_json::to_value(&st).unwrap_or(Value::Null)}),
        ))
    }
}

struct Sampled {
    id: String,
    reader: rcfd_tsstore::Reader,
    values: Vec<f64>,
    info: rcfd_tsstore::FieldInfo,
    snapshot: Value,
    interpolated: bool,
    step: u64,
}

fn count_files(dir: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() && !atomic::is_temp_name(&name) => n += 1,
                _ => {}
            }
        }
    }
    n
}
