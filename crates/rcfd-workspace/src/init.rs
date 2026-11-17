//! Make a project folder (ADR 0731 P1): the one implementation behind `rcfd project init` and the MCP tool
//! `create_project`, over `rcfd_project::Project::create`, which it calls unchanged.

use crate::{err, CONFLICT, INTERNAL, PARAMS, REFUSED};
use rcfd_api::service::RpcError;
use rcfd_project::project::{DIRS, MANIFEST};
use rcfd_project::{Project, ProjectError};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The solver version a new project records (ADR 0731 Q6 (a)): the text `rcfd --version` prints and runs record.
pub const SOLVER_VERSION: &str = concat!("rcfd ", env!("CARGO_PKG_VERSION"));

/// Longest project name, in characters.
pub const MAX_NAME: usize = 200;

/// Make the project folder `dir` named `name` (default: the folder's last component) with [`Project::create`].
///
/// The folder may exist and hold other files (ADR 0731 Q3 (a)); nothing in it is replaced, and the result lists its
/// top-level entries that were there before (`existing`, sorted). The result is `{root, name, schema_version, created,
/// solver_version_created, existing}`. Errors: 1200 when the folder already holds `rcfd-project.json` (`data.root`),
/// 1004 when the path is a file, -32602 for an empty name, one over [`MAX_NAME`] characters or one with a control
/// character, and -32000 for any other failure, naming the folders already made.
pub fn create_project(dir: &Path, name: Option<&str>) -> Result<Value, RpcError> {
    let name = match name {
        Some(n) => n.to_string(),
        None => default_name(dir)?,
    };
    check_name(&name)?;
    if dir.is_file() {
        return Err(err(REFUSED, format!("{}: is a file, not a folder", dir.display())));
    }
    let manifest = dir.join(MANIFEST);
    if manifest.exists() {
        return Err(already(dir));
    }
    let existing = match std::fs::read_dir(dir) {
        Ok(rd) => {
            let mut v: Vec<String> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            v.sort();
            v
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(err(INTERNAL, format!("{}: cannot list the folder: {e}", dir.display()))),
    };
    let wanted: Vec<PathBuf> = std::iter::once(dir.to_path_buf())
        .chain(DIRS.iter().map(|d| dir.join(d)))
        .collect();
    let absent: Vec<&PathBuf> = wanted.iter().filter(|p| !p.exists()).collect();
    let project = match Project::create(dir, &name, SOLVER_VERSION) {
        Ok(p) => p,
        Err(ProjectError::Exists { .. }) => return Err(already(dir)),
        Err(e) => {
            let made: Vec<String> = absent
                .iter()
                .filter(|p| p.is_dir())
                .map(|p| p.display().to_string())
                .collect();
            let left = if made.is_empty() {
                "no folder was made".to_string()
            } else {
                format!("folders made and left in place: {}", made.join(", "))
            };
            return Err(err(INTERNAL, format!("{}: {e}; {left}", dir.display())));
        }
    };
    let m = project.manifest();
    Ok(json!({
        "root": project.root().display().to_string(),
        "name": m.name,
        "schema_version": m.schema_version,
        "created": m.created,
        "solver_version_created": m.solver_version_created,
        "existing": existing,
    }))
}

fn already(dir: &Path) -> RpcError {
    RpcError {
        code: CONFLICT,
        message: format!("{}: already holds {MANIFEST}; nothing was written", dir.display()),
        data: Some(json!({"root": dir.display().to_string()})),
    }
}

fn default_name(dir: &Path) -> Result<String, RpcError> {
    // an existing folder by its canonical path (so `.` and `..` name the folder itself), a new one as given
    let abs = dir
        .canonicalize()
        .or_else(|_| std::path::absolute(dir))
        .unwrap_or_else(|_| dir.to_path_buf());
    abs.components()
        .filter_map(|c| match c {
            std::path::Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .next_back()
        .ok_or_else(|| err(PARAMS, format!("{}: the folder has no name; give one", dir.display())))
}

fn check_name(name: &str) -> Result<(), RpcError> {
    if name.trim().is_empty() {
        return Err(err(PARAMS, "the project name is empty"));
    }
    if name.chars().count() > MAX_NAME {
        return Err(err(
            PARAMS,
            format!("the project name is longer than {MAX_NAME} characters"),
        ));
    }
    if name.chars().any(char::is_control) {
        return Err(err(PARAMS, "the project name has a control character"));
    }
    Ok(())
}
