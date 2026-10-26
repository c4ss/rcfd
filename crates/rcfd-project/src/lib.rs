//! Studio layer L1: the project folder (docs/ui-restart-plan.md section 8, ADR 0122).
//!
//! A project is a folder with a manifest (`rcfd-project.json`), case and mesh files, and one directory
//! per run holding an immutable `run.json`, a `case.snapshot`, an atomically replaced `status.json`
//! and an append-only `events.jsonl`. This crate reads and writes that folder; it has no UI and no
//! solver dependency.

pub mod atomic;
pub mod jsonl;
pub mod paths;
pub mod project;
pub mod records;
pub mod runid;
pub mod time;
pub mod watch;

use std::path::PathBuf;

pub use project::{Issue, Project, RunSummary, Runs};
pub use records::{Manifest, Metrics, ResidualLine, RunEvent, RunRecord, RunState, RunStatus};
pub use watch::Watcher;

/// Errors of this crate. Every variant names the file or input it is about.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    /// An I/O operation failed.
    #[error("{op} {}: {source}", path.display())]
    Io {
        /// What was being done.
        op: &'static str,
        /// The file or directory.
        path: PathBuf,
        /// The OS error.
        #[source]
        source: std::io::Error,
    },
    /// An operation kept failing with a transient error until the retry budget ran out.
    #[error("{op} {}: failed after {attempts} attempts: {source}", path.display())]
    Retry {
        /// What was being done.
        op: &'static str,
        /// The file.
        path: PathBuf,
        /// Attempts made.
        attempts: u32,
        /// The last OS error.
        #[source]
        source: std::io::Error,
    },
    /// A JSON document or JSONL line did not parse.
    #[error("{}{}: {source}", path.display(), line.map(|l| format!(" line {l}")).unwrap_or_default())]
    Json {
        /// The file.
        path: PathBuf,
        /// 1-based line for JSONL files.
        line: Option<usize>,
        /// The parse error (with column).
        #[source]
        source: serde_json::Error,
    },
    /// A project-relative path was refused.
    #[error("path {input:?} refused at {component:?}: {reason}")]
    PathRefused {
        /// The path as given.
        input: String,
        /// The offending component (or the prefix that resolves outside the root).
        component: String,
        /// Why.
        reason: &'static str,
    },
    /// The file exists and the operation must not replace it.
    #[error("{}: already exists", path.display())]
    Exists {
        /// The file.
        path: PathBuf,
    },
    /// Anything else, with its location.
    #[error("{}: {msg}", path.display())]
    Invalid {
        /// The file or directory.
        path: PathBuf,
        /// What is wrong.
        msg: String,
    },
}

/// Result alias.
pub type Result<T> = std::result::Result<T, ProjectError>;

pub(crate) fn io_err(op: &'static str, path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> ProjectError {
    let path = path.into();
    move |source| ProjectError::Io { op, path, source }
}
