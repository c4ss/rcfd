//! ADR 0829 (F-997): a runner started without the caller's handles (A1), and its exit when its project folder is gone
//! (B1, B2).
//! * A1: a parent `rcfd-runner ensure` with its stdout and stderr piped starts a runner and exits; its stdout reaches EOF
//!   within 3.0 s of its exit while the runner lives. A known miss on Windows, where the runner inherits the caller's
//!   pipes (ADR 0146; ADR 0829 finding 1), with its limit intact.
//! * B1: a runner whose project folder is removed exits by itself, code 0, within 5.0 s of the removal.
//! * B2: a runner whose folder is kept still lives and holds the lock 3.5 s after it started, and a shutdown request ends
//!   it, code 0, within 5.0 s.
//!
//! Every test has its own project folder (`<temp>/rcfd_runner_f997_<pid>_<name>`). Its runner is ended by a `Drop` guard
//! that runs when the test fails too: a shutdown request, a kill of the recorded pid if it still lives 10 s later, a kill
//! of the test's own child, then the folder's removal.

mod capture;

use rcfd_runner::sys;
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

struct Proj {
    dir: PathBuf,
    pid: Option<u32>,
    child: Option<Child>,
}

impl Proj {
    fn new(name: &str) -> Proj {
        let dir = std::env::temp_dir().join(format!("rcfd_runner_f997_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
        std::fs::write(
            dir.join(".rcfd/runner-config.json"),
            r#"{"max_runs": 1, "poll_ms": 50}"#,
        )
        .unwrap();
        Proj {
            dir,
            pid: None,
            child: None,
        }
    }

    fn dot(&self) -> PathBuf {
        self.dir.join(".rcfd")
    }

    /// `rcfd-runner serve` on this project, as the test's own child with null stdio.
    fn serve(&mut self) {
        let child = Command::new(RUNNER)
            .args(["serve", "--project", &self.dir.display().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.child = Some(child);
    }

    /// The pid of `runner.json`, read until it parses (at most 10 s); kept for the teardown.
    fn record(&mut self) -> u32 {
        let t0 = Instant::now();
        loop {
            let pid = std::fs::read_to_string(self.dot().join("runner.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v["pid"].as_u64())
                .and_then(|p| u32::try_from(p).ok());
            if let Some(pid) = pid {
                self.pid = Some(pid);
                return pid;
            }
            assert!(
                t0.elapsed() < Duration::from_secs(10),
                "no runner.json in {}",
                self.dir.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The test's child's exit, polled every 10 ms for at most `limit`.
    fn exit_within(&mut self, limit: Duration) -> Option<ExitStatus> {
        let child = self.child.as_mut().expect("serve was started");
        let t0 = Instant::now();
        loop {
            if let Some(s) = child.try_wait().unwrap() {
                return Some(s);
            }
            if t0.elapsed() > limit {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// End this project's runner (idempotent): a shutdown request, then a kill of the recorded pid after 10 s.
    fn end(&mut self) {
        let alive = |p: u32| sys::pid_alive(p, "rcfd-runner");
        if let Some(pid) = self.pid.filter(|p| alive(*p)) {
            let _ = std::fs::create_dir_all(self.dot());
            let _ = std::fs::write(self.dot().join("shutdown"), b"");
            let t0 = Instant::now();
            while alive(pid) && t0.elapsed() < Duration::from_secs(10) {
                std::thread::sleep(Duration::from_millis(50));
            }
            if alive(pid) {
                sys::kill(pid);
            }
        }
        if let Some(c) = self.child.as_mut() {
            if matches!(c.try_wait(), Ok(None)) {
                let _ = c.kill();
            }
            let _ = c.wait();
        }
    }
}

impl Drop for Proj {
    fn drop(&mut self) {
        self.end();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// ADR 0829 A1 (and A1c: (a) is not built, so this run is the code before the change).
#[test]
#[cfg_attr(
    windows,
    ignore = "ADR 0829 A1: known miss on Windows, the runner inherits the caller's pipes (ADR 0146, ADR 0829 finding 1)"
)]
fn a1_a_capturing_parent_of_ensure_reaches_eof_within_3_s_known_miss_on_windows() {
    let mut p = Proj::new("a1");
    let mut cmd = Command::new(RUNNER);
    cmd.args(["ensure", "--project", &p.dir.display().to_string()])
        .stdin(Stdio::null());
    let mut cap = capture::Capture::start(cmd);
    let exit = cap.wait_exit(Duration::from_secs(30));
    let code = cap.code();
    let pid = p.record();
    let released = cap.until(exit + Duration::from_secs(3));
    let eof = cap.eof_after(0, exit);
    let lock = sys::lock_held(&p.dot().join("runner.lock"));
    let alive = sys::pid_alive(pid, "rcfd-runner");
    let t_end = Instant::now();
    p.end();
    let after_end = cap.until(Instant::now() + Duration::from_secs(5));
    eprintln!(
        "A1: ensure exit {code:?} at {:?} s; stdout EOF {eof:?} s after the exit (released within 3.0 s: {released}); \
         at 3.0 s the lock held {lock}, runner {pid} alive {alive}; after the teardown's shutdown: returned {after_end}, \
         stdout EOF {:?} s after the shutdown request",
        cap.exit_s(),
        cap.eof_after(0, t_end)
    );
    assert_eq!(code, Some(0), "set-up: ensure failed");
    assert!(
        released,
        "A1: the parent's stdout did not reach EOF within 3.0 s of its exit"
    );
    assert!(eof.is_some_and(|s| s <= 3.0), "A1: EOF {eof:?} s");
    assert!(lock && alive, "A1: the runner did not live at the EOF");
}

/// ADR 0829 B1.
#[test]
fn b1_a_runner_whose_folder_is_removed_exits_within_5_s() {
    let mut p = Proj::new("b1");
    p.serve();
    let pid = p.record();
    let removal = std::fs::remove_dir_all(&p.dir);
    let t_rm = Instant::now();
    let status = p.exit_within(Duration::from_secs(5));
    let took = t_rm.elapsed();
    eprintln!(
        "B1: runner {pid}; removal {:?}; left: folder {}, runner.json {}, lock {}; exit {status:?} {took:?} after the removal",
        removal.as_ref().map_err(|e| e.to_string()),
        p.dir.exists(),
        p.dot().join("runner.json").exists(),
        p.dot().join("runner.lock").exists()
    );
    assert!(
        status.is_some(),
        "B1: the runner did not exit within 5.0 s of its folder's removal"
    );
    assert!(status.is_some_and(|s| s.success()), "B1: exit {status:?}");
}

/// ADR 0829 B2.
#[test]
fn b2_a_runner_with_its_folder_keeps_serving() {
    let mut p = Proj::new("b2");
    p.serve();
    let pid = p.record();
    let t0 = Instant::now();
    std::thread::sleep(Duration::from_millis(3500));
    let lived = t0.elapsed();
    let alive = p.child.as_mut().unwrap().try_wait().unwrap().is_none();
    let held = sys::lock_held(&p.dot().join("runner.lock"));
    std::fs::write(p.dot().join("shutdown"), b"").unwrap();
    let t_req = Instant::now();
    let status = p.exit_within(Duration::from_secs(5));
    let took = t_req.elapsed();
    eprintln!(
        "B2: runner {pid}; at {lived:?} alive {alive}, lock held {held}; exit {status:?} {took:?} after the shutdown request"
    );
    assert!(alive && held, "B2: the runner with its folder did not keep serving");
    assert!(
        status.is_some_and(|s| s.success()),
        "B2: exit {status:?} within 5.0 s of the request"
    );
}
