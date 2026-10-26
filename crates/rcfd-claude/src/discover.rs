//! Sessions of a project folder from two sources, each allowed to fail:
//!
//! 1. the transcript directory `<claude home>/projects/<cwd with every non-alphanumeric character replaced by '-'>`
//!    (documented naming; for example `D:\work\demo-scratch` -> `D--work-demo-scratch`), each session
//!    confirmed by the `cwd` inside its transcript because the naming is not injective;
//! 2. `claude agents --json` (observed fields: pid, cwd, kind, startedAt, sessionId, name, status; a running
//!    `claude -p` is listed with kind `interactive`).
//!
//! A session is OPEN when the listing has it with a live pid, CLOSED when the listing worked and does not have it
//! (or its pid is dead), and UNKNOWN when the listing failed: the driver writes only to CLOSED sessions.

use crate::transcript::transcript_cwd;
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

/// One row of `claude agents --json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentEntry {
    /// `sessionId`.
    pub session_id: String,
    /// `pid`.
    pub pid: Option<u32>,
    /// `cwd`.
    pub cwd: Option<String>,
    /// `name`.
    pub name: Option<String>,
}

/// Whether another process has a session open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenState {
    /// Listed by `claude agents --json` with a live pid: mirror only.
    Open {
        /// The process that has it.
        pid: u32,
    },
    /// Not open in any process: the app may resume it with `claude -p --resume`.
    Closed,
    /// The listing failed, so it is not known: treated as not writable.
    Unknown,
}

/// A session of the project folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionInfo {
    /// Session id (a UUID).
    pub session_id: String,
    /// Transcript file, when it exists.
    pub transcript: Option<PathBuf>,
    /// Open or closed.
    pub state: OpenState,
    /// Display name from the listing.
    pub name: Option<String>,
    /// Transcript modification time.
    pub modified: Option<SystemTime>,
}

/// Result of a discovery: sessions (newest transcript first, open sessions without a transcript last) and the
/// reasons any source failed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Discovery {
    /// Sessions.
    pub sessions: Vec<SessionInfo>,
    /// Problems with either source; discovery still returns what the other source gave.
    pub warnings: Vec<String>,
}

/// The transcript directory name for a working directory (documented rule: non-alphanumerics become '-').
pub fn project_dir_name(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Same folder? Separators and a trailing separator are ignored; ASCII case is ignored on Windows.
pub fn same_path(a: &str, b: &str) -> bool {
    let n = |s: &str| {
        let t = s.replace('\\', "/");
        let t = t.trim_end_matches('/');
        if cfg!(windows) {
            t.to_ascii_lowercase()
        } else {
            t.to_string()
        }
    };
    n(a) == n(b)
}

/// Parses `claude agents --json`; rows without a string `sessionId` are skipped.
pub fn parse_agents(json: &str) -> Result<Vec<AgentEntry>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("not JSON: {e}"))?;
    let arr = v.as_array().ok_or("not a JSON array")?;
    let s = |r: &Value, k: &str| r.get(k).and_then(Value::as_str).map(String::from);
    Ok(arr
        .iter()
        .filter_map(|r| {
            Some(AgentEntry {
                session_id: s(r, "sessionId")?,
                pid: r.get("pid").and_then(Value::as_u64).and_then(|p| u32::try_from(p).ok()),
                cwd: s(r, "cwd"),
                name: s(r, "name"),
            })
        })
        .collect())
}

/// The subset of `pids` that are live processes (Windows: one filtered `tasklist` per pid, about 0.3 s each on the
/// development machine where an unfiltered listing took 2.5 s; elsewhere: one `ps`). No `unsafe`, no extra dependency.
pub fn live_pids(pids: &[u32]) -> std::io::Result<HashSet<u32>> {
    let mut live = HashSet::new();
    let run = |c: &mut Command| -> std::io::Result<String> {
        let out = c.stdin(Stdio::null()).stderr(Stdio::null()).output()?;
        if !out.status.success() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "process listing failed"));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    if cfg!(windows) {
        for &pid in pids {
            let text = run(Command::new("tasklist").args(["/NH", "/FO", "CSV", "/FI", &format!("PID eq {pid}")]))?;
            // "image","pid",... ; "INFO: No tasks ..." when none
            if text.lines().any(|l| {
                l.split("\",\"")
                    .nth(1)
                    .and_then(|p| p.trim_matches('"').parse::<u32>().ok())
                    == Some(pid)
            }) {
                live.insert(pid);
            }
        }
    } else if !pids.is_empty() {
        // shortcut: the non-Windows path is not exercised in wave 47 (Windows-only machine); test it on the first Linux/macOS run.
        let list = pids.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let text = run(Command::new("ps").args(["-o", "pid=", "-p", &list])).unwrap_or_default(); // ps exits 1 when none match
        live.extend(text.lines().filter_map(|l| l.trim().parse::<u32>().ok()));
    }
    Ok(live)
}

/// Combines the two sources. `listing` is the parsed `claude agents --json` (or why it failed); `live` the live pids.
pub fn classify(
    project: &Path,
    claude_home: &Path,
    listing: Result<Vec<AgentEntry>, String>,
    live: &HashSet<u32>,
) -> Discovery {
    let mut d = Discovery::default();
    let proj = project.to_string_lossy();
    let dir = claude_home.join("projects").join(project_dir_name(project));
    let mut files: Vec<(String, PathBuf, Option<SystemTime>)> = Vec::new();
    match std::fs::read_dir(&dir) {
        Ok(rd) => {
            for e in rd.flatten() {
                let p = e.path();
                let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                if p.extension().and_then(|x| x.to_str()) != Some("jsonl") || !crate::is_uuid(stem) {
                    continue;
                }
                if transcript_cwd(&p).is_some_and(|c| !same_path(&c, &proj)) {
                    continue; // another folder whose name maps to the same directory
                }
                files.push((
                    stem.to_string(),
                    p.clone(),
                    e.metadata().ok().and_then(|m| m.modified().ok()),
                ));
            }
        }
        Err(e) => d.warnings.push(format!("{}: {e}", dir.display())),
    }
    files.sort_by(|a, b| b.2.cmp(&a.2));
    let agents: Option<Vec<AgentEntry>> = match listing {
        Ok(a) => Some(
            a.into_iter()
                .filter(|a| a.cwd.as_deref().is_some_and(|c| same_path(c, &proj)))
                .collect(),
        ),
        Err(e) => {
            d.warnings.push(format!("claude agents --json: {e}"));
            None
        }
    };
    let state_of = |id: &str| -> (OpenState, Option<String>) {
        match &agents {
            None => (OpenState::Unknown, None),
            Some(a) => match a.iter().find(|a| a.session_id == id) {
                Some(AgentEntry {
                    pid: Some(pid), name, ..
                }) if live.contains(pid) => (OpenState::Open { pid: *pid }, name.clone()),
                Some(a) => (OpenState::Closed, a.name.clone()),
                None => (OpenState::Closed, None),
            },
        }
    };
    for (id, path, modified) in &files {
        let (state, name) = state_of(id);
        d.sessions.push(SessionInfo {
            session_id: id.clone(),
            transcript: Some(path.clone()),
            state,
            name,
            modified: *modified,
        });
    }
    for a in agents.iter().flatten() {
        if !files.iter().any(|f| f.0 == a.session_id) && crate::is_uuid(&a.session_id) {
            let (state, name) = state_of(&a.session_id);
            if state != OpenState::Closed {
                d.sessions.push(SessionInfo {
                    session_id: a.session_id.clone(),
                    transcript: None,
                    state,
                    name,
                    modified: None,
                });
            }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dir_name_matches_the_observed_one() {
        assert_eq!(
            project_dir_name(Path::new("D:\\work\\demo-scratch")),
            "D--work-demo-scratch"
        );
        assert_eq!(project_dir_name(Path::new("D:\\work")), "D--work");
        // the naming is not injective: this is why the transcript cwd is checked
        assert_eq!(
            project_dir_name(Path::new("C:\\a_b")),
            project_dir_name(Path::new("C:\\a-b"))
        );
    }

    #[test]
    fn live_pids_sees_this_process_and_not_an_exited_one() {
        let me = std::process::id();
        assert_eq!(live_pids(&[me]).unwrap(), HashSet::from([me]));
        let mut c = if cfg!(windows) {
            Command::new("cmd")
        } else {
            Command::new("true")
        };
        if cfg!(windows) {
            c.args(["/C", "exit 0"]);
        }
        let mut child = c.spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(live_pids(&[pid]).unwrap().is_empty(), "exited pid {pid} still listed");
        assert_eq!(live_pids(&[pid, me]).unwrap(), HashSet::from([me]));
    }
}
