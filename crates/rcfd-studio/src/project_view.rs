//! The browser's view of a project folder (ADR 0131 B1-B3): a snapshot of its lists, built on a worker thread from
//! `rcfd-project` and handed to the UI, which only reads it.

use rcfd_project::{Issue, Project, RunSummary};
use std::path::{Path, PathBuf};

/// Everything the browser and run list show about one project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Browser {
    /// Canonical project root.
    pub root: PathBuf,
    /// Manifest name.
    pub name: String,
    /// Solver version that created the project.
    pub solver_version_created: String,
    /// On-disk schema version when the manifest was migrated in memory on open (not saved).
    pub migrated_from: Option<u32>,
    /// Case files (`cases/...`), sorted.
    pub cases: Vec<String>,
    /// Mesh files (`meshes/...`), sorted.
    pub meshes: Vec<String>,
    /// Top-level entries of `designs/`.
    pub designs: Vec<String>,
    /// Complete runs, oldest first.
    pub runs: Vec<RunSummary>,
    /// Run folders that could not be summarised, with the reason (for example one being created right now).
    pub skipped: Vec<Issue>,
    /// Lists that could not be read, located.
    pub errors: Vec<String>,
}

impl Browser {
    /// Read every list of `p`. Never fails: a list that cannot be read is empty and its error is in [`Browser::errors`].
    pub fn load(p: &Project) -> Browser {
        let mut errors = Vec::new();
        let mut take = |r: rcfd_project::Result<Vec<String>>| {
            r.unwrap_or_else(|e| {
                errors.push(e.to_string());
                Vec::new()
            })
        };
        let cases = take(p.cases());
        let meshes = take(p.meshes());
        let designs = take(p.designs());
        let (runs, skipped) = match p.runs() {
            Ok(r) => (r.runs, r.skipped),
            Err(e) => {
                errors.push(e.to_string());
                (Vec::new(), Vec::new())
            }
        };
        Browser {
            root: p.root().to_path_buf(),
            name: p.manifest().name.clone(),
            solver_version_created: p.manifest().solver_version_created.clone(),
            migrated_from: p.migrated_from(),
            cases,
            meshes,
            designs,
            runs,
            skipped,
            errors,
        }
    }

    /// A notice for the header, if any: the in-memory migration that has not been saved.
    pub fn notice(&self) -> Option<String> {
        self.migrated_from.map(|v| {
            format!("unsaved migration (schema {v}): the manifest is read as the current schema but not rewritten")
        })
    }
}

/// The solver version a new project records: `rcfd <version>` of the linked `rcfd-api`.
pub fn solver_version() -> String {
    rcfd_api::design::ledger::solver_version()
}

/// Open a project folder. The error names the file (for example a manifest of a newer schema). Nothing is written.
pub fn open(root: &Path) -> Result<Project, String> {
    Project::open(root).map_err(|e| e.to_string())
}

/// Create a project folder with `name` and `solver_version`. Creating into a folder that already holds a project is a
/// located error.
pub fn create(root: &Path, name: &str, solver_version: &str) -> Result<Project, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a project needs a name".into());
    }
    Project::create(root, name, solver_version).map_err(|e| e.to_string())
}
