//! ADR 0742 (F-262), check E1: the cancel of a held run, read as soon as its terminal status is visible. The set-up is
//! the held run of ADR 0123 S7, repeated, with a reader that polls `status.json` every millisecond and checks the run
//! folder at the first terminal read. The test has its own project folder (`<temp>/rcfd_runner_f262_<pid>_e1`), its own
//! runner and its own solver processes; processes are killed only by pids the test recorded.
//!
//! `cargo test -p rcfd-runner` alone needs `cargo build -p rcfd-cli` first (the solver binary, as `runner_w47b.rs`).

use rcfd_runner::client::Client;
use rcfd_runner::records::{RunSettings, RunState, RunStatus};
use rcfd_runner::sys;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");
const CYCLES: usize = 20;

/// Steps of a run that lasts a few seconds when it is not stopped (the values of `runner_w47b.rs`).
fn long_steps() -> u64 {
    if cfg!(debug_assertions) {
        2500
    } else {
        8000
    }
}

const CASE: &str = r#"
title = "run-service cavity"
[mesh]
generator = "cavity_2d"
nx = 12
ny = 12
[physics]
nu = 0.01
[boundaries.lid]
velocity = "fixed"
velocity_value = [1.0, 0.0, 0.0]
[boundaries.walls]
velocity = "fixed"
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 400
tolerance_u = 1e-14
tolerance_p = 1e-14
"#;

struct Project {
    dir: PathBuf,
    client: Client,
    /// Solver pids this test saw in a `running` status.
    pids: Vec<u32>,
}

impl Project {
    fn new(name: &str) -> Project {
        let rcfd = Path::new(RUNNER).with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX));
        let help = Command::new(&rcfd)
            .arg("--help")
            .output()
            .unwrap_or_else(|e| panic!("{}: {e} (build it: cargo build -p rcfd-cli)", rcfd.display()));
        assert!(
            String::from_utf8_lossy(&help.stdout).contains("--pause-file"),
            "{} is stale (no --pause-file): rebuild it with cargo build -p rcfd-cli",
            rcfd.display()
        );
        let dir = std::env::temp_dir().join(format!("rcfd_runner_f262_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cases")).unwrap();
        std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
        std::fs::write(dir.join("cases/cavity.toml"), CASE).unwrap();
        let config = json!({"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100_000});
        std::fs::write(dir.join(".rcfd/runner-config.json"), config.to_string()).unwrap();
        Project {
            client: Client::new(&dir, RUNNER),
            dir,
            pids: Vec::new(),
        }
    }

    fn run_dir(&self, id: &str) -> PathBuf {
        self.dir.join("runs").join(id)
    }

    /// Poll the status of `id` every `every` until `pred` holds.
    fn wait_state(&self, id: &str, timeout: Duration, every: Duration, pred: impl Fn(&RunStatus) -> bool) -> RunStatus {
        let t0 = Instant::now();
        let mut last = None;
        loop {
            // a read can meet the replacement of the file: it is tried again
            if let Ok(s) = self.client.status(id) {
                if pred(&s) {
                    return s;
                }
                last = Some(s);
            }
            assert!(
                t0.elapsed() < timeout,
                "{id}: condition not met within {timeout:?}; last status {last:?}"
            );
            std::thread::sleep(every);
        }
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.client.shutdown(Duration::from_secs(10));
        for &p in &self.pids {
            if sys::pid_alive(p, "rcfd") {
                sys::kill(p);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The folder check of ADR 0123 S7 (`folder_problems` of `runner_w47b.rs`, copied): the records parse, the state is
/// terminal and the same in the status, the last state event and the metrics, the residual steps are contiguous and end
/// at the status step, and no temporary or request file is left.
fn folder_problems(dir: &Path) -> Vec<String> {
    let mut bad = Vec::new();
    let status: Option<RunStatus> = std::fs::read_to_string(dir.join("status.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    let Some(status) = status else {
        return vec!["status.json does not parse".into()];
    };
    if !status.state.is_terminal() || status.pid.is_some() {
        bad.push(format!("status {status:?}"));
    }
    let mut last_state = None;
    for l in std::fs::read_to_string(dir.join("events.jsonl"))
        .unwrap_or_default()
        .lines()
    {
        match serde_json::from_str::<Value>(l) {
            Ok(v) if v["event"] == "state" => last_state = Some(v["state"].clone()),
            Ok(_) => {}
            Err(_) => bad.push(format!("events line does not parse: {l}")),
        }
    }
    if last_state != Some(serde_json::to_value(status.state).unwrap()) {
        bad.push(format!("last state event {last_state:?} differs from status.json"));
    }
    let metrics: Option<Value> = std::fs::read_to_string(dir.join("metrics.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok());
    if metrics.map(|m| m["state"].clone()) != Some(serde_json::to_value(status.state).unwrap()) {
        bad.push("metrics.json missing or with another state".into());
    }
    let mut prev = 0;
    for l in std::fs::read_to_string(dir.join("residuals.jsonl"))
        .unwrap_or_default()
        .lines()
    {
        match serde_json::from_str::<Value>(l).ok().and_then(|v| v["step"].as_u64()) {
            Some(s) if s == prev + 1 => prev = s,
            _ => bad.push(format!("residual line out of order: {l}")),
        }
    }
    if prev != status.step {
        bad.push(format!("residuals end at {prev}, status says {}", status.step));
    }
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if n.contains(".tmp-") || ["pause", "pause.ack", "cancel"].contains(&n.as_str()) {
            bad.push(format!("left over: {n}"));
        }
    }
    bad
}

/// E1: 20 held runs, each cancelled and read at the first terminal status: every folder is consistent at that read.
/// Negative control: the checker reports a planted request file.
#[test]
fn e1_a_cancelled_held_run_is_consistent_at_the_first_terminal_read() {
    let mut p = Project::new("e1");
    let settings = RunSettings {
        set: vec![format!("solver.max_iterations={}", long_steps())],
        ..RunSettings::default()
    };
    let ms = Duration::from_millis;
    let mut failures = Vec::new();
    let mut slowest = 0.0f64;
    let mut last_dir = None;
    for cycle in 0..CYCLES {
        let id = p.client.submit(&p.dir.join("cases/cavity.toml"), &settings).unwrap();
        p.client.ensure_runner().unwrap();
        let st = p.wait_state(&id, Duration::from_secs(60), ms(10), |s| {
            s.state == RunState::Running && s.step >= 5
        });
        let pid = st.pid.unwrap();
        p.pids.push(pid);
        p.client.pause(&id).unwrap();
        p.wait_state(&id, Duration::from_secs(10), ms(10), |s| s.state == RunState::Paused);
        p.client.cancel(&id).unwrap();
        let t0 = Instant::now();
        let end = p.wait_state(&id, Duration::from_secs(30), ms(1), |s| s.state.is_terminal());
        let bad = folder_problems(&p.run_dir(&id));
        let dt = t0.elapsed().as_secs_f64();
        slowest = slowest.max(dt);
        if end.state != RunState::Cancelled || end.step >= long_steps() || !bad.is_empty() || dt > 5.0 {
            failures.push(format!(
                "cycle {cycle} ({id}): {:?} at step {} in {dt:.3} s: {bad:?}",
                end.state, end.step
            ));
        }
        // the solver of a cancelled run is gone (it stopped before the runner ended the run)
        let t1 = Instant::now();
        while sys::pid_alive(pid, "rcfd") && t1.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(ms(20));
        }
        if sys::pid_alive(pid, "rcfd") {
            failures.push(format!("cycle {cycle} ({id}): solver {pid} still alive"));
        }
        last_dir = Some(p.run_dir(&id));
    }
    eprintln!(
        "E1: {} of {CYCLES} cycles with a problem; slowest cancel {slowest:.3} s",
        failures.len()
    );
    for f in &failures {
        eprintln!("E1 {f}");
    }
    // negative control: a request file in an ended folder is reported
    let dir = last_dir.unwrap();
    let before = folder_problems(&dir);
    std::fs::write(dir.join("cancel"), b"").unwrap();
    let planted = folder_problems(&dir);
    assert!(before.is_empty(), "the last folder after its end: {before:?}");
    assert_eq!(planted, ["left over: cancel"], "negative control");
    assert!(failures.is_empty(), "{failures:#?}");
}
