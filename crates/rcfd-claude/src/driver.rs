//! Headless chat driver: one `claude -p` process per message, in the project folder, the message on stdin as one
//! stream-json line (never a positional argument), events to a channel, cancel by killing the process tree.
//!
//! User decision 2026-10-26 (ADR 0126): the app types only into sessions no other process has open (resume a CLOSED
//! session, or start a new one with an id it chose); it mirrors OPEN sessions read-only. A session whose state cannot
//! be established is not written to. The check happens right before spawn; a session opened by another process in
//! the gap between the check and the spawn is not detected (registered limit).

use crate::allow::{check_args, suggest, AllowList};
use crate::discover::{classify, live_pids, parse_agents, AgentEntry, Discovery};
use crate::stream::{parse_line, StreamEvent};
use crate::{is_uuid, new_uuid, Error};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How the CLI is reached. Tests pass a replay double as `program`; the user's PATH is never edited.
#[derive(Clone, Debug)]
pub struct Claude {
    /// The `claude` executable (default `claude`, resolved on PATH by the OS).
    pub program: PathBuf,
    /// Claude Code's home (`CLAUDE_CONFIG_DIR`, else `~/.claude`); transcripts are under `projects/`.
    pub claude_home: PathBuf,
    /// `--model` value, e.g. `haiku`; `None` leaves the user's default.
    pub model: Option<String>,
    /// Time limit of `claude agents --json`.
    pub listing_timeout: Duration,
}

impl Claude {
    /// A bridge for an explicit executable and home.
    pub fn new(program: impl Into<PathBuf>, claude_home: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            claude_home: claude_home.into(),
            model: None,
            listing_timeout: Duration::from_secs(20),
        }
    }

    /// `claude` on PATH with the home from `CLAUDE_CONFIG_DIR`, `USERPROFILE` or `HOME`.
    pub fn from_env() -> Option<Self> {
        let home = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|h| PathBuf::from(h).join(".claude"))
        })?;
        Some(Self::new("claude", home))
    }

    /// `claude agents --json`, run in `cwd`, killed after `listing_timeout`.
    pub fn agents(&self, cwd: &Path) -> Result<Vec<AgentEntry>, String> {
        let mut child = Command::new(&self.program)
            .args(["agents", "--json"])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("{}: {e}", self.program.display()))?;
        let mut out = child.stdout.take().expect("piped");
        let reader = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = out.read_to_string(&mut s);
            s
        });
        let t0 = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break st,
                Ok(None) if t0.elapsed() < self.listing_timeout => std::thread::sleep(Duration::from_millis(20)),
                Ok(None) => {
                    kill_tree(&mut child);
                    return Err(format!("no answer within {:?}", self.listing_timeout));
                }
                Err(e) => return Err(e.to_string()),
            }
        };
        let text = reader.join().unwrap_or_default();
        if !status.success() {
            let mut err = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            return Err(format!("exit {:?}: {}", status.code(), err.trim()));
        }
        parse_agents(&text)
    }

    /// Sessions of `project` with their open state; either source may fail (see `Discovery::warnings`).
    pub fn discover(&self, project: &Path) -> Discovery {
        let listing = self.agents(project);
        let pids: Vec<u32> = listing.iter().flatten().filter_map(|a| a.pid).collect();
        let (listing, live) = match live_pids(&pids) {
            Ok(live) => (listing, live),
            Err(e) => (Err(format!("process listing: {e}")), Default::default()),
        };
        classify(project, &self.claude_home, listing, &live)
    }

    /// The full argument list for a chat process (checked by [`check_args`] before every spawn).
    pub fn chat_args(&self, session_flag: &str, session_id: &str, allow: &AllowList) -> Vec<String> {
        let mut a: Vec<String> = [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--input-format",
            "stream-json",
        ]
        .map(String::from)
        .to_vec();
        if let Some(m) = &self.model {
            a.extend(["--model".to_string(), m.clone()]);
        }
        a.extend(allow.permission_args());
        a.extend([session_flag.to_string(), session_id.to_string()]);
        a
    }

    /// Starts a new session with an id chosen here (`--session-id`) and sends `prompt`.
    pub fn new_session(&self, project: &Path, allow: &AllowList, prompt: &str) -> Result<Chat, Error> {
        self.spawn(project, "--session-id", new_uuid(), allow, prompt)
    }

    /// Resumes a CLOSED session (`--resume`) and sends `prompt`. Refuses a session another process has open
    /// ([`Error::SessionOpen`]) and one whose state cannot be established ([`Error::OpenStateUnknown`]).
    pub fn resume(&self, project: &Path, session_id: &str, allow: &AllowList, prompt: &str) -> Result<Chat, Error> {
        if !is_uuid(session_id) {
            return Err(Error::InvalidSessionId(session_id.to_string()));
        }
        let unknown = |reason: String| Error::OpenStateUnknown {
            session_id: session_id.to_string(),
            reason,
        };
        let listing = self.agents(project).map_err(unknown)?;
        let pids: Vec<u32> = listing
            .iter()
            .filter(|a| a.session_id == session_id)
            .filter_map(|a| a.pid)
            .collect();
        let live = live_pids(&pids).map_err(|e| unknown(format!("process listing: {e}")))?;
        // any cwd: the session is the unit of the one-writer rule
        if let Some(pid) = listing
            .iter()
            .filter(|a| a.session_id == session_id)
            .find_map(|a| a.pid.filter(|p| live.contains(p)))
        {
            return Err(Error::SessionOpen {
                session_id: session_id.to_string(),
                pid,
            });
        }
        self.spawn(project, "--resume", session_id.to_string(), allow, prompt)
    }

    fn spawn(
        &self,
        project: &Path,
        flag: &str,
        session_id: String,
        allow: &AllowList,
        prompt: &str,
    ) -> Result<Chat, Error> {
        if let Some(m) = &self.model {
            if m.is_empty() || m.starts_with('-') || m.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(Error::ForbiddenArgument(format!("--model {m}")));
            }
        }
        let args = self.chat_args(flag, &session_id, allow);
        check_args(&args)?;
        let mut cmd = Command::new(&self.program);
        cmd.args(&args)
            .current_dir(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0); // so cancel can kill the whole group
        }
        let mut child = cmd.spawn().map_err(|source| Error::Io {
            path: self.program.clone(),
            source,
        })?;
        let pid = child.id();
        if let Some(mut stdin) = child.stdin.take() {
            let msg = json!({"type": "user", "message": {"role": "user", "content": prompt}});
            let _ = writeln!(stdin, "{msg}"); // a child that died at once is reported through `Exited`
        } // stdin closed here: one message per process
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let tail = Arc::new(Mutex::new(Vec::<u8>::new()));
        let tail_w = Arc::clone(&tail);
        std::thread::spawn(move || {
            let mut r = BufReader::new(stderr);
            let mut buf = [0u8; 4096];
            while let Ok(n) = r.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let mut t = lock(&tail_w);
                t.extend_from_slice(&buf[..n]);
                let excess = t.len().saturating_sub(8192);
                t.drain(..excess);
            }
        });
        let child = Arc::new(Mutex::new(child));
        let reaper = Arc::clone(&child);
        let (tx, rx) = channel();
        let project = project.to_path_buf();
        std::thread::spawn(move || {
            let mut inputs: HashMap<String, Value> = HashMap::new();
            let mut r = BufReader::new(stdout);
            let mut line = Vec::new();
            loop {
                line.clear();
                match r.read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                for ev in parse_line(&String::from_utf8_lossy(&line)) {
                    let ev = match ev {
                        StreamEvent::ToolCall { ref id, ref input, .. } => {
                            inputs.insert(id.clone(), input.clone());
                            ev
                        }
                        StreamEvent::PermissionDenied {
                            tool_name,
                            tool_use_id,
                            message,
                            ..
                        } => {
                            let suggestion = suggest(&tool_name, inputs.get(&tool_use_id), &project);
                            StreamEvent::PermissionDenied {
                                tool_name,
                                tool_use_id,
                                message,
                                suggestion,
                            }
                        }
                        ev => ev,
                    };
                    let abort = matches!(&ev, StreamEvent::Init { permission_mode, .. } if permission_mode.as_deref() != Some("dontAsk"));
                    let _ = tx.send(ev);
                    if abort {
                        kill_tree(&mut lock(&reaper));
                        let _ = tx.send(StreamEvent::Aborted(
                            "the CLI did not apply --permission-mode dontAsk".into(),
                        ));
                    }
                }
            }
            let code = loop {
                match lock(&reaper).try_wait() {
                    Ok(Some(st)) => break st.code(),
                    Ok(None) => {}
                    Err(_) => break None,
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            let stderr_tail = String::from_utf8_lossy(&lock(&tail)).into_owned();
            let _ = tx.send(StreamEvent::Exited { code, stderr_tail });
        });
        Ok(Chat {
            session_id,
            events: rx,
            pid,
            child,
            args,
        })
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Kills `child` and every process it started, if it is still running. The pid is used while the `Child` handle
/// is held, so it cannot have been reused.
fn kill_tree(child: &mut Child) {
    if !matches!(child.try_wait(), Ok(None)) {
        return;
    }
    let pid = child.id().to_string();
    let quiet = |c: &mut Command| {
        let _ = c
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    };
    if cfg!(windows) {
        quiet(Command::new("taskkill").args(["/PID", &pid, "/T", "/F"]));
    } else {
        quiet(Command::new("kill").args(["-KILL", "--", &format!("-{pid}")]));
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// A running chat turn. Dropping it cancels the process.
#[derive(Debug)]
pub struct Chat {
    /// The session id (chosen here for a new session, the resumed one otherwise).
    pub session_id: String,
    /// Events in stream order; [`StreamEvent::Exited`] is the last one.
    pub events: Receiver<StreamEvent>,
    pid: u32,
    child: Arc<Mutex<Child>>,
    args: Vec<String>,
}

impl Chat {
    /// Pid of the `claude` process.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The arguments it was started with.
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// Stops the turn: kills the process and its children (tool shells, MCP servers). The transcript keeps what was
    /// written before.
    pub fn cancel(&self) {
        kill_tree(&mut lock(&self.child));
    }
}

impl Drop for Chat {
    fn drop(&mut self) {
        self.cancel();
    }
}
