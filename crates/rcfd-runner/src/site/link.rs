//! The local end of a site: the agent `rcfd-runner site --stdio --work <work>` started through `ssh` (kind `ssh`) or
//! directly (kind `exec`, tests), spoken to in `rcfd-site/1` (JSON-RPC 2.0, one object per line) on its standard
//! streams. Nothing listens on either machine (ADR 0739 P3 A1). `ssh` gets an argument list, never a shell; the remote
//! command is the fixed agent call with the entry's `engine` and `work`, quoted for a POSIX shell when they need it.
//!
//! The agent's output is read by a thread into a channel, and a call waits on the channel while it watches the agent
//! process: on Windows a runner the agent starts inherits its handles (ADR 0146), so the end of the agent is seen by its
//! exit, not by the end of its output.

use super::sites::SiteEntry;
use super::{
    b64_decode, b64_encode, BlobRef, Missing, RemoteFile, RemoteId, RemoteRun, RemoteState, Request, Site, SiteError,
    SiteFacts, Tail,
};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The longest a call waits for its answer.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);

/// A value as one word of a POSIX shell command: as it is when it holds only plain characters, else in single quotes
/// (the sites file refuses quotes in `engine` and `work`).
fn sh_word(s: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "/_.:\\-+=@%,~".contains(c);
    if s.chars().all(plain) {
        s.to_string()
    } else {
        format!("'{s}'")
    }
}

/// The agent's program in `engine`: `<engine>/rcfd-runner` (`\` when the engine is written with `\` only).
fn agent_path(engine: &str, suffix: &str) -> String {
    let sep = if engine.contains('\\') && !engine.contains('/') {
        "\\"
    } else {
        "/"
    };
    let base = engine.trim_end_matches(['/', '\\']);
    format!("{base}{sep}rcfd-runner{suffix}")
}

/// The command that starts the agent of `entry` (nothing is started).
pub fn agent_command(entry: &SiteEntry) -> Command {
    match entry.kind.as_str() {
        "exec" => {
            let (mut c, _) = crate::sys::console(agent_path(&entry.engine, std::env::consts::EXE_SUFFIX), 0);
            c.args(["site", "--stdio", "--work", &entry.work]);
            // the site's runners are not this machine's surface's (Q-RL-2, ADR 0801): no started_by goes there
            c.env_remove(crate::started_by::ENV);
            c
        }
        _ => {
            let remote = format!(
                "{} site --stdio --work {}",
                sh_word(&agent_path(&entry.engine, "")),
                sh_word(&entry.work)
            );
            let (mut c, _) = crate::sys::console("ssh", 0);
            c.args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ServerAliveInterval=15",
                "-T",
                "-e",
                "none",
                "--",
            ])
            .arg(entry.host.as_deref().unwrap_or_default())
            .arg(remote);
            c
        }
    }
}

/// A live link to one site's agent.
pub struct AgentLink {
    name: String,
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<String>,
    err: Arc<Mutex<String>>,
    next: u64,
}

impl AgentLink {
    /// Start the agent of site `name` (`entry` checked by the caller) and wait for nothing: the first call is `hello`.
    pub fn open(name: &str, entry: &SiteEntry) -> Result<AgentLink, SiteError> {
        let mut cmd = agent_command(entry);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| SiteError::Link(format!("site {name}: cannot start {:?}: {e}", cmd.get_program())))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SiteError::Link("no agent output".into()))?;
        let stderr = child.stderr.take();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(l) = line else { break };
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        let err = Arc::new(Mutex::new(String::new()));
        if let Some(mut s) = stderr {
            let keep = Arc::clone(&err);
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n) = s.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    if let Ok(mut k) = keep.lock() {
                        k.push_str(&String::from_utf8_lossy(&buf[..n]));
                        let over = k.len().saturating_sub(4000);
                        if over > 0 {
                            let cut = (over..k.len()).find(|&i| k.is_char_boundary(i)).unwrap_or(0);
                            k.drain(..cut);
                        }
                    }
                }
            });
        }
        Ok(AgentLink {
            name: name.to_string(),
            stdin: child.stdin.take(),
            child,
            rx,
            err,
            next: 0,
        })
    }

    /// The agent process's id (the `ssh` process for an ssh site).
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    fn lost(&mut self, what: &str) -> SiteError {
        let said = self.err.lock().map(|s| s.trim().to_string()).unwrap_or_default();
        let exit = self
            .child
            .try_wait()
            .ok()
            .flatten()
            .map(|s| format!(" ({s})"))
            .unwrap_or_default();
        let said = if said.is_empty() {
            String::new()
        } else {
            format!("; it said: {}", said.lines().last().unwrap_or_default())
        };
        SiteError::Link(format!("site {}: {what}{exit}{said}", self.name))
    }

    /// One call: `method` with `params`; the result, or the agent's error as [`SiteError::Refused`].
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, SiteError> {
        self.next += 1;
        let id = self.next;
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
        let wrote = match self.stdin.as_mut() {
            Some(w) => w
                .write_all(line.as_bytes())
                .and_then(|()| w.write_all(b"\n"))
                .and_then(|()| w.flush()),
            None => Err(std::io::Error::other("closed")),
        };
        if wrote.is_err() {
            return Err(self.lost("the agent does not read its input"));
        }
        let t0 = Instant::now();
        loop {
            match self.rx.recv_timeout(Duration::from_millis(50)) {
                Ok(l) => {
                    let Ok(v) = serde_json::from_str::<Value>(&l) else {
                        continue; // a line that is not ours (a login banner): skipped
                    };
                    if v["id"] != json!(id) {
                        continue;
                    }
                    if let Some(e) = v.get("error") {
                        return Err(SiteError::Refused(format!(
                            "site {}: {}",
                            self.name,
                            e["message"].as_str().unwrap_or("error")
                        )));
                    }
                    return Ok(v["result"].clone());
                }
                Err(RecvTimeoutError::Disconnected) => return Err(self.lost("the agent closed its output")),
                Err(RecvTimeoutError::Timeout) => {
                    if matches!(self.child.try_wait(), Ok(Some(_))) {
                        // what it wrote before it exited is already in the channel
                        if let Ok(l) = self.rx.try_recv() {
                            let v: Value = serde_json::from_str(&l).unwrap_or(Value::Null);
                            if v["id"] == json!(id) && v.get("result").is_some() {
                                return Ok(v["result"].clone());
                            }
                        }
                        return Err(self.lost("the agent exited"));
                    }
                    if t0.elapsed() > CALL_TIMEOUT {
                        return Err(self.lost(&format!("no answer to {method} in {CALL_TIMEOUT:?}")));
                    }
                }
            }
        }
    }

    fn decode<T: serde::de::DeserializeOwned>(&self, v: Value) -> Result<T, SiteError> {
        serde_json::from_value(v).map_err(|e| SiteError::Refused(format!("site {}: bad answer: {e}", self.name)))
    }
}

impl Drop for AgentLink {
    fn drop(&mut self) {
        // the end of its input ends the agent; a link to an agent that does not go is killed (it is our child)
        drop(self.stdin.take());
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(2) {
            if !matches!(self.child.try_wait(), Ok(None)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn tail_of(v: &Value) -> Result<Tail, SiteError> {
    let data = b64_decode(v["data"].as_str().unwrap_or_default())
        .ok_or_else(|| SiteError::Refused("bad bytes in a read answer".into()))?;
    Ok(Tail {
        exists: v["exists"].as_bool().unwrap_or(false),
        len: v["len"].as_u64().unwrap_or(0),
        data,
    })
}

impl Site for AgentLink {
    fn hello(&mut self) -> Result<SiteFacts, SiteError> {
        let v = self.call("hello", json!({"protocol": super::PROTOCOL}))?;
        self.decode(v)
    }

    fn missing(&mut self, blobs: &[BlobRef]) -> Result<Vec<Missing>, SiteError> {
        let v = self.call("missing", json!({"blobs": blobs}))?;
        self.decode(v["missing"].clone())
    }

    fn put(&mut self, blob: &BlobRef, from: u64, bytes: &[u8]) -> Result<u64, SiteError> {
        let v = self.call(
            "put",
            json!({"hash": blob.hash, "bytes": blob.bytes, "from": from, "data": b64_encode(bytes)}),
        )?;
        Ok(v["len"].as_u64().unwrap_or(0))
    }

    fn submit(&mut self, run: &RemoteRun) -> Result<String, SiteError> {
        let v = self.call("submit", serde_json::to_value(run).unwrap_or_default())?;
        Ok(v["remote"].as_str().unwrap_or_default().to_string())
    }

    fn read(&mut self, run: &RemoteId, file: &str, from: u64, len: u64) -> Result<Tail, SiteError> {
        let v = self.call(
            "read",
            json!({"project": run.project, "run": run.id, "file": file, "from": from, "len": len}),
        )?;
        tail_of(&v)
    }

    fn files(&mut self, run: &RemoteId, dirs: &[&str], top: bool) -> Result<Vec<RemoteFile>, SiteError> {
        let v = self.call(
            "files",
            json!({"project": run.project, "run": run.id, "dirs": dirs, "top": top}),
        )?;
        self.decode(v["files"].clone())
    }

    fn request(&mut self, run: &RemoteId, what: Request) -> Result<(), SiteError> {
        self.call("request", json!({"project": run.project, "run": run.id, "what": what}))
            .map(|_| ())
    }

    fn list(&mut self, project: &str) -> Result<Vec<RemoteState>, SiteError> {
        let v = self.call("list", json!({"project": project}))?;
        self.decode(v["runs"].clone())
    }

    fn ensure(&mut self, project: &str) -> Result<(), SiteError> {
        self.call("ensure", json!({"project": project})).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ssh_command_is_an_argument_list_with_the_fixed_agent_call() {
        let e = SiteEntry {
            kind: "ssh".into(),
            host: Some("me@hpc".into()),
            engine: "/opt/rcfd 1/bin/".into(),
            work: "~/rcfd".into(),
            max_runs: None,
            cpu_threads: None,
            mem_budget_mb: None,
            assistant: false,
            confirmed: None,
            accept_engines: vec![],
        };
        let c = agent_command(&e);
        let args: Vec<String> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(c.get_program(), "ssh");
        assert_eq!(&args[args.len() - 3..args.len() - 1], ["--", "me@hpc"]);
        assert_eq!(
            args.last().unwrap(),
            "'/opt/rcfd 1/bin/rcfd-runner' site --stdio --work ~/rcfd"
        );
        assert_eq!(agent_path("C:\\rcfd\\bin", ".exe"), "C:\\rcfd\\bin\\rcfd-runner.exe");
    }
}
