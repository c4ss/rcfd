//! Path safety: a project-relative path must stay inside the project root (ADR 0122 P1).
//!
//! Refused: empty paths, absolute paths and drive or UNC prefixes, `..` components, components with
//! `:` (drive-relative paths and NTFS alternate streams), NUL, and any existing component that is a
//! symlink or junction. The deepest existing ancestor is then canonicalized and must lie under the
//! canonical root. Limit: this is a check-then-use test; a symlink created between the check and the
//! use is not caught (the project folder is trusted not to be raced by a hostile local process).

use crate::{ProjectError, Result};
use std::path::{Component, Path, PathBuf};

fn refuse(input: &str, component: impl Into<String>, reason: &'static str) -> ProjectError {
    ProjectError::PathRefused {
        input: input.to_string(),
        component: component.into(),
        reason,
    }
}

/// Resolve `rel` (with `/` or the platform separator) under `root`, refusing any escape.
pub fn resolve_in(root: &Path, rel: &str) -> Result<PathBuf> {
    if rel.trim().is_empty() {
        return Err(refuse(rel, "", "empty path"));
    }
    if rel.contains('\0') {
        return Err(refuse(rel, rel, "NUL byte"));
    }
    let mut out = root.to_path_buf();
    let mut existing = true;
    for c in Path::new(rel).components() {
        match c {
            Component::Prefix(p) => {
                return Err(refuse(
                    rel,
                    p.as_os_str().to_string_lossy(),
                    "absolute path (drive or UNC prefix)",
                ))
            }
            Component::RootDir => return Err(refuse(rel, std::path::MAIN_SEPARATOR.to_string(), "absolute path")),
            Component::ParentDir => return Err(refuse(rel, "..", "parent-directory component")),
            Component::CurDir => {}
            Component::Normal(s) => {
                let s = s.to_string_lossy();
                if s.contains(':') {
                    return Err(refuse(
                        rel,
                        s,
                        "colon in component (drive-relative path or alternate stream)",
                    ));
                }
                out.push(&*s);
                if existing {
                    match std::fs::symlink_metadata(&out) {
                        // is_symlink is true for Windows junctions too (name-surrogate reparse points).
                        Ok(m) if m.file_type().is_symlink() => return Err(refuse(rel, s, "symlink or junction")),
                        Ok(_) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => existing = false,
                        Err(e) => {
                            return Err(ProjectError::Io {
                                op: "inspect path",
                                path: out,
                                source: e,
                            })
                        }
                    }
                }
            }
        }
    }
    // Defence in depth: the deepest existing ancestor must canonicalize under the canonical root.
    let canon_root = root.canonicalize().map_err(crate::io_err("canonicalize root", root))?;
    let mut probe = out.as_path();
    while !probe.exists() {
        match probe.parent() {
            Some(p) => probe = p,
            None => break,
        }
    }
    let canon = probe.canonicalize().map_err(crate::io_err("canonicalize", probe))?;
    if !canon.starts_with(&canon_root) {
        return Err(refuse(
            rel,
            probe.to_string_lossy(),
            "resolves outside the project root",
        ));
    }
    Ok(out)
}

/// `path` relative to `root` with `/` separators, if it is under `root`.
pub fn rel_string(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}
