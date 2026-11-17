//! The solver a launch record names (ADR 0734 P1, Q3 (a)): its path, the text of `rcfd --version`, and its build id,
//! `{exe_hash, git_rev}`: `fnv64:` of the executable's bytes and the `git_rev` of `rcfd version --json` (the build's
//! `RCFD_GIT_REV`; null when the build set none or the solver predates the command).
//!
//! Hashing the executable and asking it twice costs a moment, so the answer is kept per `(path, size, modified)` for the
//! runner's life: a rebuilt solver is hashed again at its next launch.

use crate::sys;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::SystemTime;

type Key = (PathBuf, u64, Option<SystemTime>);

/// The solver's facts, kept per executable file state.
#[derive(Debug, Default)]
pub struct SolverFacts {
    kept: Option<(Key, Value)>,
}

/// The output of `solver <args>` (no window, the null device for input and errors), trimmed; `None` unless it ran and
/// exited 0.
fn ask(solver: &Path, args: &[&str]) -> Option<String> {
    let (mut c, _) = sys::console(solver, 0);
    c.args(args).stdin(Stdio::null()).stderr(Stdio::null());
    c.output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// `{path, version, build: {exe_hash, git_rev}}` of the solver at `solver` now.
pub fn describe(solver: &Path) -> Value {
    let exe_hash = rcfd_project::filehash::hash_path(solver).ok().map(|h| h.hash);
    let version = ask(solver, &["--version"]);
    let git_rev = ask(solver, &["version", "--json"])
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["git_rev"].as_str().map(String::from));
    json!({"path": solver.display().to_string(), "version": version,
           "build": {"exe_hash": exe_hash, "git_rev": git_rev}})
}

impl SolverFacts {
    /// [`describe`] of `solver`, from the kept answer when the file has the same size and modification time.
    pub fn of(&mut self, solver: &Path) -> Value {
        let meta = std::fs::metadata(solver).ok();
        let key: Key = (
            solver.to_path_buf(),
            meta.as_ref().map_or(0, |m| m.len()),
            meta.and_then(|m| m.modified().ok()),
        );
        match &self.kept {
            Some((k, v)) if *k == key => v.clone(),
            _ => {
                let v = describe(solver);
                self.kept = Some((key, v.clone()));
                v
            }
        }
    }
}
