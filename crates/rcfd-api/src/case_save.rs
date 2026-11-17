//! Save a case to one file outside a project (ADR 0737 P4, F-837 E-837c: rcfd-gui's open and save of a single case
//! file anywhere), the rule of the service's `case.save`.
//!
//! * A path inside a project (a folder above it holds `rcfd-project.json`) is refused (Q5 (a)): project files are
//!   written by the project's write rule (`edit_case`), with its edit lock and its journal line.
//! * The file is replaced only if it still has the base hash (`fnv64:`; none: there is no file yet), by
//!   write-then-rename ([`rcfd_project::atomic::atomic_write`]). The compare and the rename happen under an exclusive
//!   lock file beside the file, `<file>.rcfd-lock` (Q6 (a)), made with create-new and removed afterwards; a lock older
//!   than [`STALE_LOCK`] is taken as left by a writer that stopped, and broken.
//! * `.toml` or `.json` by the extension. [`Form::Lines`] keeps the file's own comments and layout where it can
//!   ([`crate::case_text::lines_text`], the rule of `save_case`'s `Lines` form); otherwise the whole case is written.

use crate::case::Case;
use rcfd_project::records::{hash_bytes, hash_matches};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The suffix of the lock file beside the file being written.
pub const LOCK_SUFFIX: &str = ".rcfd-lock";
/// A lock file older than this is broken.
pub const STALE_LOCK: Duration = Duration::from_secs(30);

/// What is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// The whole case (`Case::to_toml` or `to_json_string`).
    Whole,
    /// The file's own text with the changed settings' lines changed in place, when that reads back to the same case.
    Lines,
}

impl Form {
    /// `whole` or `lines`.
    pub fn name(self) -> &'static str {
        match self {
            Form::Whole => "whole",
            Form::Lines => "lines",
        }
    }
}

/// What [`save`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    /// The file.
    pub path: PathBuf,
    /// Hash of the content replaced (`None`: the file was made).
    pub old_hash: Option<String>,
    /// Hash of the content written.
    pub new_hash: String,
    /// The form written: [`Form::Lines`] only when the file's own lines were kept.
    pub form: Form,
}

/// Why [`save`] wrote nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// The file no longer has the base hash: its hash now (`None`: no file).
    Conflict {
        /// The current hash.
        current: Option<String>,
    },
    /// The path is inside the project at this folder.
    InProject(PathBuf),
    /// Not a `.toml` or `.json` path.
    Extension(PathBuf),
    /// Another writer holds the lock file.
    Busy(PathBuf),
    /// The write failed; nothing was replaced.
    Failed(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Conflict { current } => write!(
                f,
                "the file changed since it was read (now {}); read it again",
                current.as_deref().unwrap_or("absent")
            ),
            SaveError::InProject(p) => write!(
                f,
                "the path is inside the project {}: save a project case with edit_case (the project's write rule, \
                 its edit lock and journal)",
                p.display()
            ),
            SaveError::Extension(p) => write!(f, "{}: a case file ends in .toml or .json", p.display()),
            SaveError::Busy(l) => write!(
                f,
                "another writer is saving this file (lock {}); try again",
                l.display()
            ),
            SaveError::Failed(m) => f.write_str(m),
        }
    }
}

/// The project folder `path` is inside (the nearest folder above it with `rcfd-project.json`), if any.
pub fn project_of(path: &Path) -> Option<PathBuf> {
    let abs = crate::session_journal::absolute(path);
    let mut d = abs.parent();
    while let Some(dir) = d {
        if dir.join(rcfd_project::project::MANIFEST).is_file() {
            return Some(dir.to_path_buf());
        }
        d = dir.parent();
    }
    None
}

/// `path` with [`LOCK_SUFFIX`] added to its file name.
pub fn lock_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().map(|n| n.to_os_string()).unwrap_or_default();
    name.push(LOCK_SUFFIX);
    path.with_file_name(name)
}

struct Lock(PathBuf);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn take_lock(path: &Path) -> Result<Lock, SaveError> {
    let lock = lock_path(path);
    for attempt in 0..2 {
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(mut f) => {
                use std::io::Write;
                let _ = writeln!(f, "{{\"pid\": {}}}", std::process::id());
                return Ok(Lock(lock));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && attempt == 0 => {
                let stale = std::fs::metadata(&lock)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > STALE_LOCK);
                if !stale {
                    return Err(SaveError::Busy(lock));
                }
                let _ = std::fs::remove_file(&lock);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(SaveError::Busy(lock)),
            Err(e) => {
                return Err(SaveError::Failed(format!(
                    "cannot make the lock file {}: {e}",
                    lock.display()
                )))
            }
        }
    }
    Err(SaveError::Busy(lock))
}

/// Write `case` to `path` (outside any project) only if the file still has the hash `base` (`None`: only if there is no
/// file), in the form `form`.
pub fn save(path: &Path, base: Option<&str>, case: &Case, form: Form) -> Result<Saved, SaveError> {
    let is_json = match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase) {
        Some(e) if e == "json" => true,
        Some(e) if e == "toml" => false,
        _ => return Err(SaveError::Extension(path.to_path_buf())),
    };
    if let Some(p) = project_of(path) {
        return Err(SaveError::InProject(p));
    }
    let whole = if is_json { case.to_json_string() } else { case.to_toml() };
    let _lock = take_lock(path)?;
    let current = match std::fs::read(path) {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(SaveError::Failed(format!("cannot read {}: {e}", path.display()))),
    };
    let ok = match (base, &current) {
        (None, None) => true,
        (Some(h), Some(b)) => hash_matches(h, b),
        _ => false,
    };
    if !ok {
        return Err(SaveError::Conflict {
            current: current.as_deref().map(hash_bytes),
        });
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    let (bytes, written) = match (form, &current) {
        (Form::Lines, Some(disk)) => {
            match crate::case_text::lines_text(dir, case.sandbox.as_deref(), is_json, disk, whole.as_bytes()) {
                Some(t) => (t.into_bytes(), Form::Lines),
                None => (whole.into_bytes(), Form::Whole),
            }
        }
        _ => (whole.into_bytes(), Form::Whole),
    };
    rcfd_project::atomic::atomic_write(path, &bytes)
        .map_err(|e| SaveError::Failed(format!("{} not written (nothing replaced): {e}", path.display())))?;
    Ok(Saved {
        path: path.to_path_buf(),
        old_hash: current.as_deref().map(hash_bytes),
        new_hash: hash_bytes(&bytes),
        form: written,
    })
}
