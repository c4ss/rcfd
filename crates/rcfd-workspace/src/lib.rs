//! The workspace layer of a project folder (ADR 0173, registered for F-270 by ADR 0185): the commands, queries and write
//! rule that the MCP server's project tools and the Studio share, so that both reach the folder through one
//! implementation.
//!
//! [`Workspace`] is one open project with its run service and change watcher. [`Workspace::call`] runs one command or
//! query by the name of its MCP project tool (`project_info`, `list_runs`, ..., `edit_case`, `write_note`, `new_case`,
//! `start_run`, `wait_run`) with that tool's arguments and returns the tool's result object ([`Answer`]); errors are
//! [`RpcError`]s with the codes of ADR 0135 ([`CONFLICT`], [`LOCK_BUSY`], [`NOT_FOUND`] and the service codes). The MCP
//! descriptors (names, schemas, access classes) stay in `rcfd-mcp`, which wraps these answers as MCP results.
//!
//! Paths are project-relative and resolved with `rcfd_project::paths::resolve_in` (refusals are error 1004); run
//! arguments must be run ids. Results stay under [`TEXT_BUDGET`] bytes of text: lists are paged, series decimated
//! (the min/max rule of the frozen GUI's monitors, ADR 0025 M3), files read in pages, and a residual set or image
//! over the budget is written under `runs/<id>/` and returned as a path. Writes of existing files use the base-hash
//! compare-and-replace of `rcfd_project::atomic::replace_if_hash` (ADR 0135 W48-15 (a)): under the file's edit lock,
//! one `.rcfd/edits.jsonl` line per accepted write ([`write`]; the Studio's save, [`save`]). New case files are created
//! without ever replacing one ([`cases`]), from the shipped [`examples`], the built-in [`templates`], text, settings or a
//! copy (`new_case`, ADR 0186). Runs go through one handle on the project's run service ([`runs::RunService`]), with the
//! start plan of [`plan`] (ADR 0173 D7) when asked.
//!
//! The crate has no UI and no MCP types.

pub mod case_text;
pub mod cases;
mod create;
pub mod examples;
pub mod plan;
mod queries;
pub mod runs;
pub mod save;
pub mod series;
pub mod templates;
pub mod write;

use rcfd_project::{Project, ProjectError, Watcher};
use runs::RunService;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub use create::SOURCES;
pub use rcfd_api::service::RpcError;
pub use save::{save_case, SaveError, SaveForm, Saved};
pub use series::{decimate_minmax, residual_series};

/// Error code: the file changed since its base hash was read (`data.current_hash`, null when absent).
pub const CONFLICT: i64 = 1200;
/// Error code: another cooperating writer held the file's edit lock.
pub const LOCK_BUSY: i64 = 1201;
/// Error code: no such run, file, field or snapshot.
pub const NOT_FOUND: i64 = 1202;
pub(crate) const BAD_STATE: i64 = 1002;
pub(crate) const VALIDATION: i64 = 1003;
pub(crate) const REFUSED: i64 = 1004;
pub(crate) const INTERNAL: i64 = -32000;
pub(crate) const PARAMS: i64 = -32602;

/// Largest result text (bytes of the JSON text) a command or query returns; ADR 0135 F8 limits a whole MCP result to
/// 40 000.
pub const TEXT_BUDGET: usize = 36_000;
/// Largest page of `read_project_file`.
pub const FILE_PAGE: usize = 32_000;
/// Longest `wait_run`.
pub const MAX_WAIT_S: f64 = 60.0;

/// The names [`Workspace::call`] answers, in the order of the MCP project tools.
pub const COMMANDS: [&str; 18] = [
    "project_info",
    "list_runs",
    "run_status",
    "read_metrics",
    "read_residuals",
    "field_summary",
    "render_field",
    "compare_runs",
    "list_changes",
    "read_project_file",
    "edit_case",
    "write_note",
    "new_case",
    "start_run",
    "pause_run",
    "resume_run",
    "cancel_run",
    "wait_run",
];

/// The answer of a command or query.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// A result object (what the MCP tool returns as `structuredContent`).
    Value(Value),
    /// A PNG image (base64) with its metadata object (`render_field` under the size budget).
    Image {
        /// The PNG file, base64-encoded.
        png_base64: String,
        /// The metadata object.
        meta: Value,
    },
}

pub(crate) fn err(code: i64, message: impl Into<String>) -> RpcError {
    RpcError {
        code,
        message: message.into(),
        data: None,
    }
}

/// A project-library error as a command error: a refused path 1004, a missing file 1202, a conflict 1200 with
/// `data.current_hash`, a busy edit lock 1201, anything else -32000.
pub fn project_error(e: ProjectError) -> RpcError {
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

/// One open project folder: the project, its run service and the change watcher (made on first use).
pub struct Workspace {
    project: Project,
    runs: RunService,
    watcher: Mutex<Option<Watcher>>,
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace").field("root", &self.project.root()).finish()
    }
}

impl Workspace {
    /// Open the project folder `dir` (a located error when it is not a project). Runs are started with the
    /// `rcfd-runner` next to the current executable or named by `RCFD_RUNNER_BIN`.
    pub fn open(dir: &Path) -> Result<Self, String> {
        let project =
            Project::open(dir).map_err(|e| format!("{} is not an rcfd project folder: {e}", dir.display()))?;
        let runs = RunService::with_default_runner(project.root());
        Ok(Workspace {
            project,
            runs,
            watcher: Mutex::new(None),
        })
    }

    /// Start runs with the runner executable `exe` instead.
    pub fn with_runner_exe(mut self, exe: impl Into<PathBuf>) -> Self {
        self.runs = RunService::new(self.project.root(), exe);
        self
    }

    /// Canonical project root.
    pub fn root(&self) -> &Path {
        self.project.root()
    }

    /// The open project.
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// The project's run service.
    pub fn runs(&self) -> &RunService {
        &self.runs
    }

    /// Run command or query `name` (one of [`COMMANDS`]) with the arguments of its MCP project tool, which the caller
    /// has checked against that tool's input schema; `None` if `name` is not one.
    pub fn call(&self, name: &str, args: &Value) -> Option<Result<Answer, RpcError>> {
        let value = |r: Result<Value, RpcError>| r.map(Answer::Value);
        Some(match name {
            "project_info" => value(self.project_info()),
            "list_runs" => value(self.list_runs(args)),
            "run_status" => value(self.run_status(args)),
            "read_metrics" => value(self.read_metrics(args)),
            "read_residuals" => value(self.read_residuals(args)),
            "field_summary" => value(self.field_summary(args)),
            "render_field" => self.render_field(args),
            "compare_runs" => value(self.compare_runs(args)),
            "list_changes" => value(self.list_changes(args)),
            "read_project_file" => value(self.read_project_file(args)),
            "edit_case" => value(self.edit_case(args)),
            "write_note" => value(self.write_note(args)),
            "new_case" => value(self.new_case(args)),
            "start_run" => value(self.start_run(args)),
            "pause_run" | "resume_run" | "cancel_run" => value(self.control(name, args)),
            "wait_run" => value(self.wait_run(args)),
            _ => return None,
        })
    }

    /// Save a case file as the Studio's editor does ([`save::save_case`] on this project).
    pub fn save_case(&self, rel: &str, base: &str, bytes: &[u8], form: SaveForm) -> Result<Saved, SaveError> {
        save::save_case(self.project.root(), rel, base, bytes, form)
    }

    fn rel(&self, p: &Path) -> String {
        rcfd_project::paths::rel_string(self.project.root(), p).unwrap_or_else(|| p.display().to_string())
    }

    /// A run argument: a run id whose folder holds `run.json`.
    fn run_arg(&self, args: &Value, key: &str) -> Result<(String, PathBuf), RpcError> {
        let id = str_arg(args, key);
        let dir = self.project.run_dir(id).map_err(project_error)?;
        if !dir.join("run.json").is_file() {
            return Err(err(
                NOT_FOUND,
                format!("run {id}: no such run (runs/{id}/run.json is missing)"),
            ));
        }
        Ok((id.to_string(), dir))
    }
}

pub(crate) fn u64_arg(args: &Value, k: &str) -> Option<u64> {
    args.get(k).and_then(Value::as_u64)
}

pub(crate) fn str_arg<'a>(args: &'a Value, k: &str) -> &'a str {
    args.get(k).and_then(Value::as_str).unwrap_or_default()
}

pub(crate) fn text_len(v: &Value) -> usize {
    v.to_string().len()
}
