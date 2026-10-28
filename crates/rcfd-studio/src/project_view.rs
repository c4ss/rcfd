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
    /// Run folders summarised to make this listing (a cost measure, ADR 0133 R12).
    pub summaries_read: usize,
}

/// The order of runs everywhere the Studio lists them, as `Project::runs` orders them: by `created` (`run.json`), then
/// by id, so that runs of the same second (whose ids sort by their random suffix) are in creation order, and runs of
/// the same `created` time in one fixed order (ADR 0147, the maintainer's answer to ADR 0144 question 12).
pub fn run_order(a: &RunSummary, b: &RunSummary) -> std::cmp::Ordering {
    (&a.created, &a.id).cmp(&(&b.created, &b.id))
}

/// The newest listed run by [`run_order`] (not the largest id).
pub fn newest_run(b: &Browser) -> Option<&str> {
    b.runs.iter().max_by(|x, y| run_order(x, y)).map(|r| r.id.as_str())
}

/// True for a case file: a `.toml` or `.json` file (any case of the extension). Other files under `cases/` (a table,
/// a profile the cases read) are data files (ADR 0148, the maintainer's answer to ADR 0147 question 3; the Studio's
/// lists only: `Project::cases` still lists every file).
pub fn is_case_file(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("toml") || e.eq_ignore_ascii_case("json"))
}

/// The files under `cases/` split into case files ([`is_case_file`]) and data files, each in the listing's order. With
/// `all_cases` (the route before ADR 0148), every file is a case file.
pub fn split_cases(files: &[String], all_cases: bool) -> (Vec<String>, Vec<String>) {
    files.iter().cloned().partition(|f| all_cases || is_case_file(f))
}

/// The run id of a project-relative path under `runs/<id>/`, if it names a run folder.
pub fn run_of(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("runs/")?;
    let id = rest.split('/').next()?;
    rcfd_project::runid::is_run_id(id).then_some(id)
}

/// Does a change of `path` (under `runs/<id>/`) change that run's summary? Only `run.json`, `status.json` and
/// `metrics.json` feed it (`Project::run_summary`).
fn feeds_summary(path: &str) -> bool {
    let mut parts = path.split('/');
    let (_, _, file, rest) = (parts.next(), parts.next(), parts.next(), parts.next());
    rest.is_none() && matches!(file, Some("run.json" | "status.json" | "metrics.json"))
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
        let summaries_read = runs.len() + skipped.len();
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
            summaries_read,
        }
    }

    /// Bring the lists up to date after the watcher reported `changed` (project-relative paths). When every path is
    /// under `runs/`, only the runs whose summary can have changed are summarised again (their `run.json`,
    /// `status.json` or `metrics.json` changed, or the folder appeared or went away), so the cost follows the runs that
    /// change, not the size of the project (ADR 0133 R12); any other path reloads everything ([`Browser::load`]).
    /// Returns whether anything was read.
    pub fn refresh(&mut self, p: &Project, changed: &[String]) -> bool {
        if changed.iter().any(|c| !c.starts_with("runs/")) {
            *self = Browser::load(p);
            return true;
        }
        let listed = |b: &Browser, id: &str| {
            b.runs.iter().any(|r| r.id == id) || b.skipped.iter().any(|s| s.path.file_name().is_some_and(|n| n == id))
        };
        let mut ids: Vec<&str> = changed
            .iter()
            .filter_map(|c| run_of(c).map(|id| (id, c)))
            .filter(|(id, c)| feeds_summary(c) || !listed(self, id) || !self.root.join("runs").join(id).is_dir())
            .map(|(id, _)| id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        self.summaries_read = 0;
        if ids.is_empty() {
            return false;
        }
        for id in ids {
            self.runs.retain(|r| r.id != id);
            self.skipped.retain(|s| s.path.file_name().is_none_or(|n| n != id));
            let dir = self.root.join("runs").join(id);
            if !dir.is_dir() {
                continue;
            }
            self.summaries_read += 1;
            match p.run_summary(id) {
                Ok(s) => self.runs.push(s),
                Err(e) => self.skipped.push(Issue {
                    path: p.root().join("runs").join(id),
                    message: e.to_string(),
                }),
            }
        }
        // the orders of Project::runs: runs by (created, id), skipped by id
        self.runs.sort_by(run_order);
        self.skipped.sort_by(|a, b| a.path.file_name().cmp(&b.path.file_name()));
        true
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
