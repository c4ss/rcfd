//! The save of the Studio's case editor through the write rule of [`crate::write`] (ADR 0185 E4, E5): a base-hash
//! compare-and-replace under the file's edit lock, with one `.rcfd/edits.jsonl` line (`who` [`SAVE_CASE`]; with
//! [`save_case_by`] also the actor and the settings the save changed, ADR 0188 J5).
//! [`save_case`] writes the text it is given ([`SaveForm::Whole`], what the Studio writes) or, in [`SaveForm::Lines`],
//! only the lines of the settings that differ from the file (ADR 0173 D6 (a)); the Studio keeps `Whole` until the
//! maintainer answers ADR 0185 Q1.

use crate::case_text::apply_sets;
use crate::journal::Origin;
use rcfd_api::case::Case;
use rcfd_project::records::{hash_bytes, hash_matches};
use rcfd_project::{atomic, paths, ProjectError};
use serde_json::Value;
use std::path::Path;

/// The `who` of a [`save_case`] line in `.rcfd/edits.jsonl` (ADR 0185 Q3).
pub const SAVE_CASE: &str = "save_case";

/// How [`save_case`] writes a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SaveForm {
    /// The text given, as it is (the Studio's save: the whole settings tree written by `Case::to_toml` or
    /// `to_json_string`).
    #[default]
    Whole,
    /// The file's own text with the lines of the settings whose values differ from the text given changed in place
    /// (comments and layout kept), when that text re-parses to the same settings as the text given; otherwise, or when
    /// a setting was removed, the text given (ADR 0185 Q2 (a)).
    Lines,
}

/// What [`save_case`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    /// Project-relative path (`/` separators).
    pub rel: String,
    /// Hash of the replaced content (`None`: the file was created).
    pub old_hash: Option<String>,
    /// Hash of the content written.
    pub new_hash: String,
    /// The form written: [`SaveForm::Lines`] only when the file's own lines were kept.
    pub form: SaveForm,
}

/// Why [`save_case`] wrote nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveError {
    /// The file no longer has the base hash: its hash now (`None`: no file).
    Conflict {
        /// The current hash.
        current: Option<String>,
    },
    /// Any other failure (a refused path, a busy edit lock, an I/O error), located.
    Failed(String),
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Conflict { current } => write!(
                f,
                "changed since it was read (now {})",
                current.as_deref().unwrap_or("absent")
            ),
            SaveError::Failed(m) => f.write_str(m),
        }
    }
}

fn save_error(e: ProjectError) -> SaveError {
    match e {
        ProjectError::Conflict { current, .. } => SaveError::Conflict { current },
        e => SaveError::Failed(e.to_string()),
    }
}

fn is_json_case(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

/// Save the case file `rel` (project-relative) of the project at `root` with `bytes`, only if it still has the hash
/// `base` (`hash_bytes` form; an empty `base`: only if there is no file), under its edit lock, with one
/// `.rcfd/edits.jsonl` line (`who` [`SAVE_CASE`], no actor). `form` says what is written ([`SaveForm`]).
pub fn save_case(root: &Path, rel: &str, base: &str, bytes: &[u8], form: SaveForm) -> Result<Saved, SaveError> {
    save_case_by(root, rel, base, bytes, form, &Origin::new(SAVE_CASE, None, None))
}

/// [`save_case`] whose journal line names `origin` (its command, actor and reason) and lists the settings whose values
/// the save changed (ADR 0188 J5: both texts read as the case is read; no list when either does not read as a case).
pub fn save_case_by(
    root: &Path,
    rel: &str,
    base: &str,
    bytes: &[u8],
    form: SaveForm,
    origin: &Origin,
) -> Result<Saved, SaveError> {
    let base = (!base.is_empty()).then_some(base);
    // the file as it is now, when it still has the base hash (else the write below reports the conflict)
    let mut before: Option<(std::path::PathBuf, Vec<u8>)> = None;
    let (text, written) = match (form, base) {
        (SaveForm::Lines, Some(b)) => {
            let path = paths::resolve_in(root, rel).map_err(save_error)?;
            let disk = atomic::read(&path).map_err(|e| match e {
                ProjectError::Io { ref source, .. } if source.kind() == std::io::ErrorKind::NotFound => {
                    SaveError::Conflict { current: None }
                }
                e => save_error(e),
            })?;
            if !hash_matches(b, &disk) {
                return Err(SaveError::Conflict {
                    current: Some(hash_bytes(&disk)),
                });
            }
            let out = match lines_text(root, &path, &disk, bytes) {
                Some(t) => (t.into_bytes(), SaveForm::Lines),
                None => (bytes.to_vec(), SaveForm::Whole),
            };
            before = Some((path, disk));
            out
        }
        (_, Some(b)) => {
            if let Ok(path) = paths::resolve_in(root, rel) {
                before = atomic::read(&path)
                    .ok()
                    .filter(|d| hash_matches(b, d))
                    .map(|d| (path, d));
            }
            (bytes.to_vec(), SaveForm::Whole)
        }
        _ => (bytes.to_vec(), SaveForm::Whole),
    };
    let mut extra = origin.fields();
    if let Some((path, disk)) = &before {
        if let Some(list) = saved_changes(root, path, disk, &text) {
            crate::journal::put_list(&mut extra, "changes", list);
        }
    }
    let r = atomic::replace_if_hash_with(root, rel, base, &text, &origin.who, extra).map_err(save_error)?;
    Ok(Saved {
        rel: r.rel,
        old_hash: r.old_hash,
        new_hash: r.new_hash,
        form: written,
    })
}

/// The settings whose values differ between the case file `disk` (at `path`) and the text `saved`, both read as the case
/// is read; `None` when either is not a case.
fn saved_changes(root: &Path, path: &Path, disk: &[u8], saved: &[u8]) -> Option<Vec<serde_json::Value>> {
    let is_json = is_json_case(path);
    let dir = path.parent().unwrap_or(root);
    let parse = |b: &[u8]| {
        let t = std::str::from_utf8(b).ok()?;
        if is_json {
            Case::parse_json_with(t, dir, Some(root)).ok()
        } else {
            Case::parse_toml_with(t, dir, Some(root)).ok()
        }
    };
    Some(crate::journal::setting_changes(&parse(disk)?, &parse(saved)?))
}

/// The file `disk` (the case at `path`) with the settings of `saved` placed in its own lines, if that text re-parses to
/// the settings of `saved`; `None` otherwise (a text that does not parse, a removed setting, a value that cannot be
/// placed).
fn lines_text(root: &Path, path: &Path, disk: &[u8], saved: &[u8]) -> Option<String> {
    let is_json = is_json_case(path);
    let dir = path.parent().unwrap_or(root);
    let parse = |t: &str| {
        if is_json {
            Case::parse_json_with(t, dir, Some(root))
        } else {
            Case::parse_toml_with(t, dir, Some(root))
        }
    };
    let text = std::str::from_utf8(disk).ok()?;
    let old = parse(text).ok()?;
    let want = parse(std::str::from_utf8(saved).ok()?).ok()?.tree.to_json();
    let mut set = Vec::new();
    changed_settings(&old, &old.tree.to_json(), &want, "", &mut set)?;
    let out = apply_sets(text, is_json, set.iter().map(|(k, v)| (k.as_str(), v)))?;
    parse(&out).ok().filter(|c| c.tree.to_json() == want).map(|_| out)
}

/// The settings of `new` (a settings tree, at `prefix`) whose values differ from the case `old` (a setting under an
/// optional table that `old` does not hold is compared with the default `old` reads for it), as `(dotted path, value)`;
/// `None` when a setting of `old_tree` is missing from `new`.
fn changed_settings(
    old: &Case,
    old_tree: &Value,
    new: &Value,
    prefix: &str,
    out: &mut Vec<(String, Value)>,
) -> Option<()> {
    let join = |k: &str| {
        if prefix.is_empty() {
            k.to_string()
        } else {
            format!("{prefix}.{k}")
        }
    };
    let new = new.as_object()?;
    if let Some(o) = old_tree.as_object() {
        if o.keys().any(|k| !new.contains_key(k)) {
            return None; // a removal: the line rule cannot write it
        }
    }
    for (k, v) in new {
        let path = join(k);
        match (v.as_object(), old_tree.get(k)) {
            // a table both hold: compare inside it (a key of the old one missing from the new one is a removal)
            (Some(_), Some(sub)) if sub.is_object() => changed_settings(old, sub, v, &path, out)?,
            // a table only the new tree holds (an optional table, a new named entry): its settings one by one
            (Some(m), _) if !m.is_empty() => changed_settings(old, &Value::Null, v, &path, out)?,
            _ => {
                if old.get(&path).ok().as_ref() != Some(v) {
                    out.push((path, v.clone()));
                }
            }
        }
    }
    Some(())
}
