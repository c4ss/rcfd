//! ADR 0146 C2: a runner that `rcfd-mcp` started (`start_run`) does not keep the server's stdout pipe open after the server
//! has exited, so a caller that reads that pipe to the end (Claude Code) sees EOF. Control: a process that starts the runner
//! with its own stdout given to the runner on purpose holds its caller's pipe, so the harness sees a held pipe.
//!
//! The harness never blocks on a pipe. A reader thread per pipe records the instant of EOF; the exit is polled with
//! `try_wait`. A guard shuts the runner down, which ends a held pipe. The tests need `rcfd` and `rcfd-runner` next to
//! `rcfd-mcp` (one cargo build profile). This file uses only what exists before ADR 0146's change.

mod common;

use common::{binaries, project};
use rcfd_runner::client::Client;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// C2's limit: the server's stdout reaches EOF this long after its exit (ADR 0146).
const LIMIT: Duration = Duration::from_secs(3);

const C2_DIR: &str = "RCFD_W48L_C2_DIR";
const C2_RUNNER: &str = "RCFD_W48L_C2_RUNNER";

fn runner_exe() -> PathBuf {
    binaries().with_file_name(format!("rcfd-runner{}", std::env::consts::EXE_SUFFIX))
}

/// Shuts the project's runner down, kills the solvers of its unfinished runs and removes its scratch folder, also when an
/// assertion failed.
struct Stop(PathBuf);

impl Drop for Stop {
    fn drop(&mut self) {
        let c = Client::new(&self.0, runner_exe());
        let _ = c.shutdown(Duration::from_secs(10));
        if let Ok(runs) = c.list() {
            for (_, st) in runs {
                if let (Some(p), false) = (st.pid, st.state.is_terminal()) {
                    rcfd_runner::sys::kill(p);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(200));
        if let Some(scratch) = self.0.parent() {
            let _ = std::fs::remove_dir_all(scratch);
        }
    }
}

enum Ev {
    Line(String),
    Eof(Instant),
}

/// A process with stdout read line by line and stderr read to the end, each on its own thread.
struct Proc {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: Receiver<Ev>,
    err: Receiver<Instant>,
    seen: Vec<String>,
    out_eof: Option<Instant>,
    err_eof: Option<Instant>,
    exit: Option<(Instant, ExitStatus)>,
}

impl Proc {
    fn start(mut cmd: Command) -> Proc {
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the process starts");
        let (tx, lines) = channel();
        let out = child.stdout.take().unwrap();
        std::thread::spawn(move || {
            for l in BufReader::new(out).lines() {
                match l {
                    Ok(l) => {
                        let _ = tx.send(Ev::Line(l));
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(Ev::Eof(Instant::now()));
        });
        let (etx, err) = channel();
        let mut e = child.stderr.take().unwrap();
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = e.read_to_end(&mut sink);
            let _ = etx.send(Instant::now());
        });
        Proc {
            stdin: child.stdin.take(),
            child,
            lines,
            err,
            seen: Vec::new(),
            out_eof: None,
            err_eof: None,
            exit: None,
        }
    }

    fn take(&mut self, ev: Ev) -> Option<String> {
        match ev {
            Ev::Line(l) => {
                self.seen.push(l.clone());
                Some(l)
            }
            Ev::Eof(t) => {
                self.out_eof = Some(t);
                None
            }
        }
    }

    fn tick(&mut self) {
        if self.exit.is_none() {
            if let Ok(Some(s)) = self.child.try_wait() {
                self.exit = Some((Instant::now(), s));
            }
        }
        while let Ok(ev) = self.lines.try_recv() {
            self.take(ev);
        }
        if let Ok(t) = self.err.try_recv() {
            self.err_eof = Some(t);
        }
    }

    /// The next stdout line, waiting at most `timeout`.
    fn next_line(&mut self, timeout: Duration) -> Option<String> {
        let ev = self.lines.recv_timeout(timeout).ok()?;
        self.take(ev)
    }

    /// Poll until the process has exited, at most `limit` (then it is killed). The exit instant.
    fn wait_exit(&mut self, limit: Duration) -> Instant {
        let t0 = Instant::now();
        let mut killed = false;
        loop {
            self.tick();
            if let Some((t, _)) = self.exit {
                return t;
            }
            if !killed && t0.elapsed() > limit {
                let _ = self.child.kill();
                killed = true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Poll until stdout has reached EOF or `deadline` has passed; the EOF instant.
    fn out_eof_by(&mut self, deadline: Instant) -> Option<Instant> {
        loop {
            self.tick();
            if self.out_eof.is_some() || Instant::now() >= deadline {
                return self.out_eof;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn code(&self) -> Option<i32> {
        self.exit.as_ref().and_then(|(_, s)| s.code())
    }

    /// A JSON-RPC request on stdin and its one-line response.
    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        let stdin = self.stdin.as_mut().expect("stdin is piped");
        writeln!(
            stdin,
            "{}",
            json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
        )
        .unwrap();
        stdin.flush().unwrap();
        let line = self
            .next_line(Duration::from_secs(120))
            .unwrap_or_else(|| panic!("no response to {method}"));
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"))
    }
}

fn secs(a: Instant, b: Option<Instant>) -> Option<f64> {
    b.map(|b| b.saturating_duration_since(a).as_secs_f64())
}

/// C2: `rcfd-mcp` starts a run (and so the runner) through `start_run`, then its stdin is closed. The server exits 0 within
/// 10 s (set-up); its stdout reaches EOF within 3.0 s of its exit, while the runner stays alive; the run then finishes.
#[test]
fn c2_mcp_stdout_reaches_eof_after_the_server_exits() {
    let mcp = binaries();
    let dir = project("c2-w48l");
    let _stop = Stop(dir.clone());
    let client = Client::new(&dir, runner_exe());
    assert!(
        !client.runner_running(),
        "set-up: a runner holds the lock before the start"
    );
    let d = dir.to_string_lossy().to_string();
    let mut cmd = Command::new(&mcp);
    cmd.args(["--project", &d, "--root", &d]).stdin(Stdio::piped());
    let mut p = Proc::start(cmd);
    let init = p.request(1, "initialize", json!({"protocolVersion": "2025-06-18"}));
    assert!(init.get("result").is_some(), "{init}");
    let r = p.request(
        2,
        "tools/call",
        json!({"name": "start_run", "arguments": {"case": "cases/cavity.toml"}}),
    );
    assert_eq!(r["result"]["isError"], false, "{r}");
    let run = r["result"]["structuredContent"]["run"].as_str().unwrap().to_string();
    let close = Instant::now();
    drop(p.stdin.take());
    let exit = p.wait_exit(Duration::from_secs(10));
    let eof = p.out_eof_by(exit + LIMIT);
    let lock = client.runner_running();
    eprintln!(
        "C2: run {run}; server exit {:?} s after the close (code {:?}); stdout EOF {:?} s after the exit; stderr EOF {:?} \
         s after the exit; lock probe then {lock}",
        secs(close, Some(exit)),
        p.code(),
        secs(exit, eof),
        secs(exit, p.err_eof)
    );
    assert_eq!(
        p.code(),
        Some(0),
        "set-up: the server did not exit 0 after its stdin closed"
    );
    assert!(
        exit.duration_since(close) <= Duration::from_secs(10),
        "set-up: the server took more than 10 s to exit"
    );
    if eof.is_none() {
        let _ = client.wait(&run, Duration::from_secs(120));
        let t = Instant::now();
        let _ = client.shutdown(Duration::from_secs(10));
        let late = p.out_eof_by(t + Duration::from_secs(30));
        eprintln!(
            "C2: held: stdout EOF {:?} s after the runner's shutdown request",
            secs(t, late)
        );
        panic!("C2: the server's stdout did not reach EOF within {LIMIT:?} of its exit");
    }
    assert!(lock, "C2: the runner did not stay alive");
    let st = client.wait(&run, Duration::from_secs(120)).unwrap();
    assert_eq!(st.state, rcfd_runner::records::RunState::Finished, "{st:?}");
}

/// Child entry point of the C2 control (does nothing without its environment). It starts the project's runner with this
/// process's stdout and stderr given to it on purpose, waits for the lock, prints one line and exits.
#[test]
fn c2_control_child_entry() {
    let (Some(dir), Some(runner)) = (std::env::var_os(C2_DIR), std::env::var_os(C2_RUNNER)) else {
        return;
    };
    let (dir, runner) = (PathBuf::from(dir), PathBuf::from(runner));
    let args = ["serve".to_string(), "--project".into(), dir.display().to_string()];
    let (mut cmd, _) = rcfd_runner::sys::detached_command(&runner, &args);
    let child = cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn().unwrap();
    let c = Client::new(&dir, &runner);
    let t0 = Instant::now();
    while !c.runner_running() && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(20));
    }
    println!(
        "C2 control: runner {} holds the lock: {}",
        child.id(),
        c.runner_running()
    );
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

/// C2 control: a process whose runner was given its stdout is held 3.0 s after it exits (on every platform: the harness
/// sees a held pipe); the EOF follows within 5 s of the runner's shutdown.
#[test]
fn c2_control_a_runner_given_the_callers_stdout_holds_it() {
    let _ = binaries();
    let dir = project("c2ctl-w48l");
    let _stop = Stop(dir.clone());
    let mut cmd = Command::new(std::env::current_exe().unwrap());
    cmd.args([
        "--exact",
        "c2_control_child_entry",
        "--nocapture",
        "--test-threads",
        "1",
    ])
    .env(C2_DIR, &dir)
    .env(C2_RUNNER, runner_exe())
    .stdin(Stdio::null());
    let mut p = Proc::start(cmd);
    let exit = p.wait_exit(Duration::from_secs(60));
    let held = p.out_eof_by(exit + LIMIT).is_none();
    let started = p.seen.iter().any(|l| l.contains("holds the lock: true"));
    let t = Instant::now();
    let _ = Client::new(&dir, runner_exe()).shutdown(Duration::from_secs(10));
    let late = p.out_eof_by(t + Duration::from_secs(30));
    eprintln!(
        "C2 control: child exit code {:?}; held 3.0 s after its exit: {held}; stdout EOF {:?} s after the shutdown \
         request; lines {:?}",
        p.code(),
        secs(t, late),
        p.seen
    );
    assert_eq!(p.code(), Some(0));
    assert!(
        started,
        "set-up: the child's runner did not take the lock: {:?}",
        p.seen
    );
    assert!(held, "C2 control: the harness did not see the held pipe");
    assert!(
        secs(t, late).is_some_and(|s| s <= 5.0),
        "C2 control: no EOF within 5 s of the shutdown"
    );
}
