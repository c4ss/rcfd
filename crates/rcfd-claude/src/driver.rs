//! Headless chat driver: one `claude -p` process per message, in the project folder, the message on stdin as one
//! stream-json line (never a positional argument), events to a channel, cancel by killing the process tree.
//!
//! User decision 2026-10-26 (ADR 0126): the app types only into sessions no other process has open (resume a CLOSED
//! session, or start a new one with an id it chose); it mirrors OPEN sessions read-only. A session whose state cannot
//! be established is not written to. The check happens right before spawn; a session opened by another process in
//! the gap between the check and the spawn is not detected (registered limit).

use crate::allow::{check_args_in, suggest, AllowList};
use crate::discover::{classify, live_pids, parse_agents, AgentEntry, Discovery};
use crate::stream::{parse_line, StreamEvent};
use crate::trust::{check_folder, TrustState, TrustStore};
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
    /// App-owned MCP config file (ADR 0134 W48-10 (a)). When set, every chat gets
    /// `--strict-mcp-config --mcp-config <file>`, so only the servers in that file start (not a project's
    /// `.mcp.json`); the file must lie outside the project folder ([`check_args_in`]). `None` (the default of
    /// [`Claude::new`]) leaves the argument list as before.
    pub mcp_config: Option<PathBuf>,
    /// The per-user record of confirmed folders ([`TrustStore`], a file in the Studio's config folder, never inside a
    /// project). `new_session` and `resume` start nothing in a folder whose control files are not confirmed there
    /// ([`Error::FolderNotConfirmed`]). `None` (the default of [`Claude::new`]): nothing can be confirmed, so only a
    /// folder without control files can start a chat.
    pub trust_store: Option<PathBuf>,
}

impl Claude {
    /// A bridge for an explicit executable and home.
    pub fn new(program: impl Into<PathBuf>, claude_home: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            claude_home: claude_home.into(),
            model: None,
            listing_timeout: Duration::from_secs(20),
            mcp_config: None,
            trust_store: None,
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
        let mut child = self
            .agents_cmd(cwd)
            .0
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

    /// The `claude agents --json` command (no window; stdio as before) and its creation flags.
    pub(crate) fn agents_cmd(&self, cwd: &Path) -> (Command, u32) {
        let (mut c, flags) = crate::console(&self.program);
        c.args(["agents", "--json"])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        (c, flags)
    }

    /// The chat command for `args` in `project` (no window; stdin, stdout and stderr piped as before) and its flags.
    pub(crate) fn chat_cmd(&self, project: &Path, args: &[String]) -> (Command, u32) {
        let (mut cmd, flags) = crate::console(&self.program);
        cmd.args(args)
            .current_dir(project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0); // so cancel can kill the whole group
        }
        (cmd, flags)
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

    /// The full argument list for a chat process (checked by [`check_args_in`], which includes
    /// [`check_args`](crate::allow::check_args),
    /// before every spawn). With [`Claude::mcp_config`] set, `--strict-mcp-config --mcp-config <file>` comes right
    /// before the permission flags.
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
        if let Some(cfg) = &self.mcp_config {
            a.extend([
                "--strict-mcp-config".to_string(),
                "--mcp-config".to_string(),
                cfg.to_string_lossy().into_owned(),
            ]);
        }
        a.extend(allow.permission_args());
        a.extend([session_flag.to_string(), session_id.to_string()]);
        a
    }

    /// Starts a new session with an id chosen here (`--session-id`) and sends `prompt`. Refuses a folder whose control
    /// files are not confirmed ([`Error::FolderNotConfirmed`]) before starting anything.
    pub fn new_session(&self, project: &Path, allow: &AllowList, prompt: &str) -> Result<Chat, Error> {
        self.spawn(project, "--session-id", new_uuid(), allow, prompt)
    }

    /// Resumes a CLOSED session (`--resume`) and sends `prompt`. Refuses a session another process has open
    /// ([`Error::SessionOpen`]) and one whose state cannot be established ([`Error::OpenStateUnknown`]); a folder
    /// whose control files are not confirmed is refused first, before `claude agents --json` runs.
    pub fn resume(&self, project: &Path, session_id: &str, allow: &AllowList, prompt: &str) -> Result<Chat, Error> {
        if !is_uuid(session_id) {
            return Err(Error::InvalidSessionId(session_id.to_string()));
        }
        // the argument and folder checks come before any process, including the listing
        self.prepare(project, "--resume", session_id, allow)?;
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

    /// Whether a chat may start in `project`: the scan of its control files against [`Claude::trust_store`]
    /// (ADR 0134 Amendment 2). `new_session` and `resume` refuse anything but [`TrustState::Confirmed`].
    pub fn check_folder(&self, project: &Path) -> Result<TrustState, Error> {
        let store = self.trust_store.as_ref().map(TrustStore::new);
        check_folder(project, store.as_ref())
    }

    /// The checked argument list of a chat: the model value, [`check_args_in`], then the folder check.
    fn prepare(&self, project: &Path, flag: &str, session_id: &str, allow: &AllowList) -> Result<Vec<String>, Error> {
        if let Some(m) = &self.model {
            if m.is_empty() || m.starts_with('-') || m.chars().any(|c| c.is_whitespace() || c.is_control()) {
                return Err(Error::ForbiddenArgument(format!("--model {m}")));
            }
        }
        let args = self.chat_args(flag, session_id, allow);
        check_args_in(&args, project)?; // the verdict of check_args when no MCP flag is present
        match self.check_folder(project)? {
            TrustState::Confirmed => Ok(args),
            TrustState::Pending(pending) => Err(Error::FolderNotConfirmed {
                project: project.to_path_buf(),
                pending,
            }),
        }
    }

    fn spawn(
        &self,
        project: &Path,
        flag: &str,
        session_id: String,
        allow: &AllowList,
        prompt: &str,
    ) -> Result<Chat, Error> {
        let args = self.prepare(project, flag, &session_id, allow)?;
        let mut child = self.chat_cmd(project, &args).0.spawn().map_err(|source| Error::Io {
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
        let stderr_reader = std::thread::spawn(move || {
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
            let mut checked = false;
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
                    let abort = if checked { None } else { unchecked(&ev) };
                    checked |= abort.is_none() && matches!(&ev, StreamEvent::Init { .. });
                    // the offending event itself is not forwarded, except an Init (it shows the mode the CLI applied)
                    if abort.is_none() || matches!(ev, StreamEvent::Init { .. }) {
                        let _ = tx.send(ev);
                    }
                    if let Some(why) = abort {
                        kill_tree(&mut lock(&reaper));
                        let _ = tx.send(StreamEvent::Aborted(why));
                        checked = true; // abort once; lines already buffered still reach the UI after the Aborted event
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
            // ADR 0134 Amendment 5: the tail is complete only once the stderr reader has seen EOF; the exit status
            // can be seen before the last bytes are read. A reader that panicked leaves the tail as read so far.
            let _ = stderr_reader.join();
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

/// Why `ev` aborts a session whose permission mode has not been checked yet; `None` when it may pass. Only a
/// `system/init` with a session id and `permissionMode` `dontAsk` checks the session. Unknown `system/*` lines
/// (e.g. hook output) carry no action and pass; anything else before the check aborts (ADR 0128 finding 5).
fn unchecked(ev: &StreamEvent) -> Option<String> {
    let before = |what: String| {
        Some(format!(
            "{what} arrived before a checked system/init (session_id and permissionMode dontAsk)"
        ))
    };
    match ev {
        StreamEvent::Init { permission_mode, .. } if permission_mode.as_deref() == Some("dontAsk") => None,
        StreamEvent::Init { permission_mode, .. } => Some(format!(
            "the CLI did not apply --permission-mode dontAsk (system/init reports {permission_mode:?})"
        )),
        StreamEvent::Unrecognised { kind: Some(k) } if k == "system/init" => {
            Some("the CLI's system/init line has no session_id, so its permission mode cannot be checked".into())
        }
        StreamEvent::Unrecognised { kind: Some(k) } if k.starts_with("system/") => None,
        StreamEvent::Unrecognised { kind } => before(format!("an unrecognised line ({kind:?})")),
        StreamEvent::ToolCall { name, .. } => before(format!("a tool call ({name})")),
        StreamEvent::TextDelta(_) => before("assistant text".into()),
        StreamEvent::PermissionDenied { tool_name, .. } => before(format!("a permission denial ({tool_name})")),
        StreamEvent::Result { .. } => before("a result".into()),
        StreamEvent::Aborted(_) | StreamEvent::Exited { .. } => None,
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
    let _ = kill_tree_cmd(child.id())
        .0
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// The command that kills process `pid` and its descendants (no window) and its creation flags.
pub(crate) fn kill_tree_cmd(pid: u32) -> (Command, u32) {
    let pid = pid.to_string();
    if cfg!(windows) {
        let (mut c, f) = crate::console("taskkill");
        c.args(["/PID", &pid, "/T", "/F"]);
        (c, f)
    } else {
        let (mut c, f) = crate::console("kill");
        c.args(["-KILL", "--", &format!("-{pid}")]);
        (c, f)
    }
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
