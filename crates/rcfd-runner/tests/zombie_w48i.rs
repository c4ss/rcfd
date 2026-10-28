//! ADR 0143 Z1: a runner that this test process started (`Client::ensure_runner`, so this process is its parent) and
//! that is then killed reads as gone within 2.0 s (`runner_running()` and `sys::pid_alive` both false), and the next
//! `ensure_runner` starts another runner. On Unix a killed child stays a zombie until its parent reaps it, and `ps`
//! still names it (ADR 0137's S4 diagnosis). Only APIs that exist before ADR 0143's change are used, so the same file
//! runs on the code before the change (Z1c).

use rcfd_runner::client::Client;
use rcfd_runner::sys;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

/// (a): both probes read false within this of the kill.
const GONE_S: f64 = 2.0;

fn runner_pid(dir: &Path) -> Option<u32> {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".rcfd/runner.json")).ok()?).ok()?;
    v["pid"].as_u64().and_then(|p| u32::try_from(p).ok())
}

/// Shuts the project's runner down and kills the runner pids the test recorded, also on a failed assertion.
struct Guard {
    dir: PathBuf,
    client: Client,
    pids: Vec<u32>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.client.shutdown(Duration::from_secs(10));
        for &p in &self.pids {
            if sys::pid_alive(p, "rcfd-runner") {
                sys::kill(p);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn z1_a_killed_runner_child_reads_as_gone() {
    let dir = std::env::temp_dir().join(format!("rcfd_runner_w48i_{}_z1", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
    let cfg = json!({"max_runs": 1, "cpu_threads": 8, "poll_ms": 100, "adopt_probe_ms": 500});
    std::fs::write(dir.join(".rcfd/runner-config.json"), cfg.to_string()).unwrap();
    let mut g = Guard {
        client: Client::new(&dir, RUNNER),
        dir,
        pids: Vec::new(),
    };
    g.client.ensure_runner().unwrap();
    // runner.json is written just after the lock is taken (when ensure_runner may return): read it until it exists
    let t1 = Instant::now();
    let mut first = runner_pid(&g.dir);
    while first.is_none() && t1.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
        first = runner_pid(&g.dir);
    }
    let pid = first.expect("runner.json names the runner");
    g.pids.push(pid);
    // negative control: both probes see the live runner
    let before = (g.client.runner_running(), sys::pid_alive(pid, "rcfd-runner"));
    assert!(sys::kill(pid), "kill {pid}");
    let t0 = Instant::now();
    let mut gone = None;
    let mut last = (true, true);
    while t0.elapsed() < Duration::from_secs(10) {
        last = (g.client.runner_running(), sys::pid_alive(pid, "rcfd-runner"));
        if last == (false, false) {
            gone = Some(t0.elapsed().as_secs_f64());
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    eprintln!(
        "Z1: runner {pid}: before the kill (runner_running, pid_alive) {before:?}; gone after {gone:?} s; last \
         probes {last:?}"
    );
    assert_eq!(before, (true, true), "negative control: the probes do not see the live runner");
    let dt = gone.unwrap_or(f64::INFINITY);
    assert!(
        dt <= GONE_S,
        "(a) the killed runner still reads as alive (runner_running, pid_alive) = {last:?} {:.3} s after the kill",
        t0.elapsed().as_secs_f64()
    );
    // (b) the next start takes the project over; the new runner writes runner.json just after it takes the lock, which
    // is when ensure_runner may return, so the file is read until it changes (at most 5 s)
    let again = g.client.ensure_runner();
    let t1 = Instant::now();
    let mut next = runner_pid(&g.dir);
    while next == Some(pid) && t1.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
        next = runner_pid(&g.dir);
    }
    if let Some(n) = next {
        g.pids.push(n);
    }
    eprintln!("Z1 (b): ensure_runner {again:?}; runner.json pid {next:?}");
    assert!(again.is_ok(), "(b) {again:?}");
    assert!(
        next.is_some_and(|n| n != pid),
        "(b) runner.json names {next:?}, the killed runner was {pid}"
    );
}
