//! The edit hook of the Claude Code plugin (ADR 0173 D5 (b), ADR 0195 P6): `rcfd-mcp --edit-hook [<folder>]` reads one
//! `PreToolUse` hook input on stdin and refuses Claude Code's own file tools under `cases/`, `runs/` and `designs/` of an
//! rcfd project, so that those folders change only through the project tools (validated, checked against the file's hash
//! and kept in the project journal). `meshes/`, `notes/` and every other file stay writable.
//!
//! The contract (Claude Code hooks reference, checked on Claude Code 2.1.319, ADR 0195): exit 2 blocks the call and
//! Claude sees stderr as the reason; exit 0 lets it through; any other exit is a non-blocking error (the call goes ahead
//! and a hook error notice is shown).

use serde_json::Value;
use std::path::{Component, Path, PathBuf};

/// Claude Code's tools that write a file named by a path.
pub const TOOLS: [&str; 4] = ["Edit", "Write", "MultiEdit", "NotebookEdit"];
/// The project folders those tools may not write.
pub const GUARDED: [&str; 3] = ["cases", "runs", "designs"];

/// Exit code: let the call through.
pub const ALLOW: u8 = 0;
/// Exit code: the input could not be read (a non-blocking error for Claude Code).
pub const NOT_READ: u8 = 1;
/// Exit code: refuse the call.
pub const REFUSE: u8 = 2;

/// What the hook answers: its exit code and the text for stderr (empty when nothing is to be said).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// [`ALLOW`], [`NOT_READ`] or [`REFUSE`].
    pub code: u8,
    /// The message for stderr.
    pub stderr: String,
}

fn allow() -> Outcome {
    Outcome {
        code: ALLOW,
        stderr: String::new(),
    }
}

fn not_read(why: &str) -> Outcome {
    Outcome {
        code: NOT_READ,
        stderr: format!("rcfd edit hook: {why}; the call is not checked"),
    }
}

/// The message of a refusal.
pub fn message(tool: &str, rel: &str) -> String {
    format!(
        "rcfd: {tool} of {rel} refused: in an rcfd project, cases/, runs/ and designs/ change only through the project \
         tools, so that every change is validated, checked against the file's hash and kept in the project journal. Use \
         edit_case to change a case's settings (read_project_file gives its base_hash) and new_case to create a case (from \
         an example, a template, TOML, settings or a copy); start_run writes runs/ and the design tools write designs/. \
         meshes/, notes/ and other files can still be written."
    )
}

/// `p` with `.` dropped and `..` removing one component (never above the root), separators `/` and `\` alike.
fn normalize(p: &str) -> PathBuf {
    let plain = rcfd_project::paths::strip_verbatim(p).unwrap_or_else(|| p.to_string());
    let plain = if cfg!(windows) { plain } else { plain.replace('\\', "/") };
    let mut out = PathBuf::new();
    let mut roots = 0;
    for c in Path::new(&plain).components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if out.components().count() > roots {
                    out.pop();
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                out.push(c.as_os_str());
                roots += 1;
            }
            Component::Normal(n) => out.push(n),
        }
    }
    out
}

/// The comparison keys of `p`'s components: on Windows without ASCII case and without trailing dots and spaces.
fn keys(p: &Path) -> Vec<String> {
    p.components()
        .map(|c| {
            let s = c.as_os_str().to_string_lossy().replace('\\', "/");
            if cfg!(windows) {
                s.trim_end_matches(['.', ' ']).to_ascii_lowercase()
            } else {
                s
            }
        })
        .collect()
}

/// `p` (normalised) with its deepest existing ancestor canonicalised and the rest appended; `None` when nothing of it
/// exists or it cannot be resolved.
fn resolved(p: &Path) -> Option<PathBuf> {
    let mut base = p.to_path_buf();
    let mut rest = Vec::new();
    loop {
        if base.exists() {
            let mut c = rcfd_project::paths::canonicalize(&base).ok()?;
            for r in rest.iter().rev() {
                c.push(r);
            }
            return Some(c);
        }
        rest.push(base.file_name()?.to_os_string());
        if !base.pop() {
            return None;
        }
    }
}

/// The project-relative path of `file` when it lies under `root` in a guarded folder (both already in one form).
fn guarded(file: &Path, root: &Path) -> Option<String> {
    let (f, r) = (keys(file), keys(root));
    if f.len() <= r.len() + 1 || f[..r.len()] != r[..] || !GUARDED.contains(&f[r.len()].as_str()) {
        return None;
    }
    let parts: Vec<String> = file
        .components()
        .skip(r.len())
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// The hook's answer to the input `input` (the bytes Claude Code writes on stdin). `folder` is the hook's argument and
/// `env_dir` the environment's `CLAUDE_PROJECT_DIR`; the project folder is the first of them that is given and not empty,
/// else the input's `cwd`.
pub fn decide(input: &[u8], folder: Option<&str>, env_dir: Option<&str>) -> Outcome {
    let v: Value = match serde_json::from_slice(input) {
        Ok(v) => v,
        Err(e) => return not_read(&format!("the input is not JSON ({e})")),
    };
    let Some(tool) = v.get("tool_name").and_then(Value::as_str) else {
        return not_read("the input has no tool_name (not a PreToolUse hook input)");
    };
    if !TOOLS.contains(&tool) {
        return allow();
    }
    let cwd = v.get("cwd").and_then(Value::as_str).filter(|s| !s.is_empty());
    let Some(dir) = [folder, env_dir, cwd]
        .into_iter()
        .flatten()
        .find(|s| !s.trim().is_empty())
    else {
        return allow();
    };
    let root = normalize(dir);
    if !root.join(rcfd_project::project::MANIFEST).is_file() {
        return allow(); // not an rcfd project: the plugin may be on in any folder
    }
    let key = if tool == "NotebookEdit" {
        "notebook_path"
    } else {
        "file_path"
    };
    let ti = &v["tool_input"];
    let Some(path) = ti
        .get(key)
        .or_else(|| ti.get("file_path"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    else {
        return not_read(&format!("{tool} without {key}"));
    };
    let joined = if Path::new(path).is_absolute() || path.starts_with(['/', '\\']) {
        PathBuf::from(path)
    } else {
        Path::new(cwd.unwrap_or(dir)).join(path)
    };
    let file = normalize(&joined.to_string_lossy());
    let hit = guarded(&file, &root).or_else(|| {
        let (f, r) = (resolved(&file)?, resolved(&root)?);
        guarded(&f, &r)
    });
    match hit {
        Some(rel) => Outcome {
            code: REFUSE,
            stderr: message(tool, &rel),
        },
        None => allow(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_and_keys() {
        let n = normalize("a/./b/../c");
        assert_eq!(keys(&n), ["a", "c"]);
        assert_eq!(keys(&normalize("../../x")), ["x"]);
        if cfg!(windows) {
            assert_eq!(keys(&normalize(r"C:\P\Cases.\x")), ["c:", "/", "p", "cases", "x"]);
            assert_eq!(keys(&normalize(r"\\?\C:\p\cases")), ["c:", "/", "p", "cases"]);
        }
        // a component compare, not a text prefix
        assert_eq!(guarded(&normalize("/p/casesx/a"), &normalize("/p")), None);
        assert_eq!(guarded(&normalize("/p/cases"), &normalize("/p")), None);
        assert_eq!(
            guarded(&normalize("/p/cases/a.toml"), &normalize("/p")).as_deref(),
            Some("cases/a.toml")
        );
    }
}
