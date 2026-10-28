//! Process tests of the run service (ADR 0123, checks S1-S9). Every test has its own project folder
//! (`<temp>/rcfd_runner_w47b_<pid>_<name>`), its own runner and its own solver processes; processes are
//! killed only by pids the test recorded.
//!
//! The solver binary is `rcfd` next to `rcfd-runner` in the same target directory: `cargo test --workspace`
//! builds it; `cargo test -p rcfd-runner` alone needs `cargo build -p rcfd-cli` first (checked below).

use rcfd_runner::client::Client;
use rcfd_runner::records::{RunSettings, RunState, RunStatus};
use rcfd_runner::sys;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

/// Steps of a long run (a few seconds) and of a short one, per build profile (ADR 0123 smoke).
fn long_steps() -> u64 {
    if cfg!(debug_assertions) {
        2500
    } else {
        LONG_RELEASE
    }
}
const LONG_RELEASE: u64 = 8_000;
const SHORT: u64 = 60;

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

fn rcfd_bin() -> PathBuf {
    let p = Path::new(RUNNER).with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX));
    let help = Command::new(&p)
        .arg("--help")
        .output()
        .unwrap_or_else(|e| panic!("{}: {e} (build it: cargo build -p rcfd-cli)", p.display()));
    assert!(
        String::from_utf8_lossy(&help.stdout).contains("--pause-file"),
        "{} is stale (no --pause-file): rebuild it with cargo build -p rcfd-cli",
        p.display()
    );
    p
}

fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < timeout {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    f()
}

/// Live process ids with executable `name`, from the OS (independent of the runner's records).
fn os_pids(name: &str) -> HashSet<u32> {
    if cfg!(windows) {
        let o = Command::new("tasklist")
            .args(["/FI", &format!("IMAGENAME eq {name}.exe"), "/FO", "CSV", "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|l| l.split(',').nth(1))
            .filter_map(|p| p.trim_matches('"').parse().ok())
            .collect()
    } else {
        let o = Command::new("ps").args(["-e", "-o", "pid=,comm="]).output().unwrap();
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| l.trim_end().ends_with(name) && !l.trim_end().ends_with(&format!("{name}-runner")))
            .filter_map(|l| l.split_whitespace().next()?.parse().ok())
            .collect()
    }
}

struct Project {
    dir: PathBuf,
    client: Client,
}

impl Project {
    fn new(name: &str, config: Value) -> Project {
        rcfd_bin();
        let dir = std::env::temp_dir().join(format!("rcfd_runner_w47b_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cases")).unwrap();
        std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
        std::fs::write(dir.join("cases/cavity.toml"), CASE).unwrap();
        std::fs::write(dir.join(".rcfd/runner-config.json"), config.to_string()).unwrap();
        Project {
            client: Client::new(&dir, RUNNER),
            dir,
        }
    }

    fn case(&self) -> PathBuf {
        self.dir.join("cases/cavity.toml")
    }

    fn submit(&self, steps: u64, priority: i32) -> String {
        self.client.submit(&self.case(), &settings(steps, priority)).unwrap()
    }

    fn status(&self, id: &str) -> RunStatus {
        self.client.status(id).unwrap()
    }

    fn run_dir(&self, id: &str) -> PathBuf {
        self.dir.join("runs").join(id)
    }

    fn events(&self, id: &str) -> Vec<Value> {
        std::fs::read_to_string(self.run_dir(id).join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{id} events line ({e}): {l}")))
            .collect()
    }

    fn launches(&self, id: &str) -> Vec<Value> {
        self.events(id)
            .into_iter()
            .filter(|e| e["event"] == "launched")
            .collect()
    }

    fn residual_steps(&self, id: &str) -> Vec<u64> {
        std::fs::read_to_string(self.run_dir(id).join("residuals.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap()["step"].as_u64().unwrap())
            .collect()
    }

    fn runner_pid(&self) -> u32 {
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(self.dir.join(".rcfd/runner.json")).unwrap()).unwrap();
        v["pid"].as_u64().unwrap() as u32
    }

    fn wait_state(&self, id: &str, timeout: Duration, pred: impl Fn(&RunStatus) -> bool) -> RunStatus {
        let mut last = None;
        let ok = wait_until(timeout, || {
            let s = self.client.status(id).ok();
            let hit = s.as_ref().is_some_and(&pred);
            last = s;
            hit
        });
        assert!(ok, "{id}: condition not met within {timeout:?}; last status {last:?}");
        last.unwrap()
    }

    /// Solver pids this project's runners launched.
    fn launched_pids(&self) -> HashSet<u32> {
        let ids = self.client.list().unwrap();
        ids.iter()
            .flat_map(|(id, _)| self.launches(id))
            .filter_map(|e| e["pid"].as_u64().map(|p| p as u32))
            .collect()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = self.client.shutdown(Duration::from_secs(10));
        if let Ok(runs) = self.client.list() {
            for (_, st) in runs {
                if let (Some(p), false) = (st.pid, st.state.is_terminal()) {
                    sys::kill(p);
                }
            }
        }
        for p in self.launched_pids() {
            if sys::pid_alive(p, "rcfd") {
                sys::kill(p);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn settings(steps: u64, priority: i32) -> RunSettings {
    RunSettings {
        priority,
        set: vec![format!("solver.max_iterations={steps}")],
        ..RunSettings::default()
    }
}

fn base_config(max_runs: usize) -> Value {
    json!({"max_runs": max_runs, "cpu_threads": 8, "mem_budget_mb": 100_000, "poll_ms": 100,
           "adopt_probe_ms": 500, "cancel_grace_ms": 10_000})
}

fn running(s: &RunStatus) -> bool {
    matches!(s.state, RunState::Running | RunState::Paused)
}

/// Sample a project until every run is terminal: (max runs in running/paused by status.json every ~20 ms,
/// max live solver processes launched by this project by the OS every ~200 ms, samples).
fn sample_concurrency(p: &Project, timeout: Duration) -> (usize, usize, usize) {
    let (mut max_status, mut max_os, mut n) = (0, 0, 0);
    let t0 = Instant::now();
    let mut next_os = Instant::now();
    loop {
        let runs = p.client.list().unwrap();
        max_status = max_status.max(runs.iter().filter(|(_, s)| running(s)).count());
        if Instant::now() >= next_os {
            let launched = p.launched_pids();
            max_os = max_os.max(os_pids("rcfd").intersection(&launched).count());
            next_os = Instant::now() + Duration::from_millis(200);
        }
        n += 1;
        if runs.iter().all(|(_, s)| s.state.is_terminal()) {
            return (max_status, max_os, n);
        }
        assert!(t0.elapsed() < timeout, "runs not done within {timeout:?}: {runs:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// S1: concurrency never exceeds max_runs, sampled over the whole life of the queue; negative control: the
/// same sampler sees 3 under max_runs = 3.
#[test]
fn s1_concurrency_never_exceeds_max_runs() {
    let p = Project::new("s1", base_config(2));
    let ids: Vec<String> = (0..5).map(|_| p.submit(long_steps() / 4, 0)).collect();
    p.client.ensure_runner().unwrap();
    let (st, os, n) = sample_concurrency(&p, Duration::from_secs(300));
    eprintln!("S1 max_runs 2: max by status {st}, max by OS {os}, {n} samples");
    assert!(st <= 2 && os <= 2, "over-subscribed: status {st}, OS {os}");
    assert_eq!(st, 2, "the queue never filled both slots (set-up defective)");
    for id in &ids {
        assert_eq!(p.status(id).state, RunState::Finished, "{id}");
    }
    let q = Project::new("s1neg", base_config(3));
    for _ in 0..3 {
        q.submit(long_steps() / 4, 0);
    }
    q.client.ensure_runner().unwrap();
    let (st3, os3, _) = sample_concurrency(&q, Duration::from_secs(300));
    eprintln!("S1 negative control max_runs 3: status {st3}, OS {os3}");
    assert!(
        st3 == 3 && os3 > 2,
        "the sampler does not see an over-subscription of 2"
    );
}

/// S2: strict queue order: priority descending, then submission order; negative control without priorities.
#[test]
fn s2_queue_order_and_priorities() {
    let order = |prios: [i32; 4], name: &str| {
        let p = Project::new(name, base_config(1));
        let ids: Vec<String> = prios.iter().map(|&pr| p.submit(SHORT, pr)).collect();
        p.client.ensure_runner().unwrap();
        for id in &ids {
            p.client.wait(id, Duration::from_secs(120)).unwrap();
        }
        let mut seq: Vec<(u64, usize)> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (p.launches(id)[0]["launch_seq"].as_u64().unwrap(), i))
            .collect();
        seq.sort_unstable();
        seq.into_iter().map(|(_, i)| i).collect::<Vec<_>>()
    };
    let with = order([0, 0, 5, 0], "s2");
    let without = order([0, 0, 0, 0], "s2neg");
    eprintln!("S2 launch order with priorities {with:?}, without {without:?}");
    assert_eq!(with, [2, 0, 1, 3]);
    assert_eq!(without, [0, 1, 2, 3]);
}

/// S3: a killed solver is detected and marked failed with a located reason within 2.0 s; negative
/// control: a normal run of the same project ends finished.
#[test]
fn s3_killed_child_is_detected_and_marked_failed() {
    let p = Project::new("s3", base_config(2));
    let ok = p.submit(SHORT, 0);
    let id = p.submit(long_steps(), 0);
    p.client.ensure_runner().unwrap();
    let st = p.wait_state(&id, Duration::from_secs(60), |s| {
        s.state == RunState::Running && s.step >= 5
    });
    let pid = st.pid.unwrap();
    assert!(sys::kill(pid));
    let t0 = Instant::now();
    let st = p.wait_state(&id, Duration::from_secs(30), |s| s.state.is_terminal());
    let dt = t0.elapsed().as_secs_f64();
    let reason = st.reason.clone().unwrap_or_default();
    eprintln!("S3 detected in {dt:.3} s: {reason}");
    assert!(
        st.step < long_steps(),
        "the run ended before the kill (set-up defective)"
    );
    assert_eq!(st.state, RunState::Failed);
    assert!(dt <= 2.0, "detection took {dt} s");
    assert!(reason.contains(&format!("solver process {pid}")) && reason.contains("without a finish event"));
    assert!(reason.contains("last step") && reason.contains("log.txt"), "{reason}");
    assert_eq!(
        p.client.wait(&ok, Duration::from_secs(60)).unwrap().state,
        RunState::Finished
    );
}

fn assert_once_and_contiguous(p: &Project, ids: &[String]) {
    assert_eq!(p.client.list().unwrap().len(), ids.len(), "runs lost or added");
    for id in ids {
        assert_eq!(p.launches(id).len(), 1, "{id} launched {} times", p.launches(id).len());
        let steps = p.residual_steps(id);
        let want: Vec<u64> = (1..=steps.len() as u64).collect();
        assert_eq!(steps, want, "{id}: residual steps duplicated or missing");
    }
}

/// S4a: a killed runner is recovered by the next start: the live solver is adopted (not relaunched),
/// queued runs run, nothing is lost or duplicated.
#[test]
fn s4a_killed_runner_is_recovered_without_loss_or_duplicates() {
    let p = Project::new("s4a", base_config(1));
    let ids = vec![p.submit(long_steps() / 2, 0), p.submit(SHORT, 0), p.submit(SHORT, 0)];
    p.client.ensure_runner().unwrap();
    let st = p.wait_state(&ids[0], Duration::from_secs(60), |s| {
        s.state == RunState::Running && s.step >= 5
    });
    let old = p.runner_pid();
    assert!(sys::kill(old));
    assert!(wait_until(Duration::from_secs(5), || !p.client.runner_running()));
    assert!(
        sys::pid_alive(st.pid.unwrap(), "rcfd"),
        "the solver died with the runner"
    );
    p.client.ensure_runner().unwrap();
    assert_ne!(p.runner_pid(), old);
    for id in &ids {
        let s = p.client.wait(id, Duration::from_secs(300)).unwrap();
        assert_eq!(s.state, RunState::Finished, "{id}: {s:?}");
    }
    assert!(p.events(&ids[0]).iter().any(|e| e["event"] == "adopted"));
    assert_once_and_contiguous(&p, &ids);
    assert_eq!(p.residual_steps(&ids[0]).len() as u64, long_steps() / 2);
}

/// S4b: a solver that died while no runner was alive is marked failed by the next runner, with a
/// located reason, within adopt_probe_ms + 2 s of the start.
#[test]
fn s4b_solver_dead_while_runner_dead_is_marked_failed() {
    let p = Project::new("s4b", base_config(1));
    let a = p.submit(long_steps(), 0);
    let b = p.submit(SHORT, 0);
    p.client.ensure_runner().unwrap();
    let st = p.wait_state(&a, Duration::from_secs(60), |s| {
        s.state == RunState::Running && s.step >= 5
    });
    assert!(sys::kill(p.runner_pid()));
    assert!(wait_until(Duration::from_secs(5), || !p.client.runner_running()));
    assert!(sys::kill(st.pid.unwrap()));
    assert!(wait_until(Duration::from_secs(5), || !sys::pid_alive(
        st.pid.unwrap(),
        "rcfd"
    )));
    p.client.ensure_runner().unwrap();
    let t0 = Instant::now();
    let s = p.wait_state(&a, Duration::from_secs(30), |s| s.state.is_terminal());
    let dt = t0.elapsed().as_secs_f64();
    let reason = s.reason.clone().unwrap_or_default();
    eprintln!("S4b marked {:?} {dt:.3} s after the start: {reason}", s.state);
    assert_eq!(s.state, RunState::Failed);
    assert!(dt <= 0.5 + 2.0, "{dt}");
    assert!(reason.contains("found not running by the liveness probe") && reason.contains("last step"));
    assert_eq!(
        p.client.wait(&b, Duration::from_secs(60)).unwrap().state,
        RunState::Finished
    );
    assert_once_and_contiguous(&p, &[a, b]);
}

/// S4c: a clean shutdown leaves the solver running and the queue intact; the next runner adopts.
#[test]
fn s4c_clean_shutdown_and_restart_adopts() {
    let p = Project::new("s4c", base_config(1));
    let ids = vec![p.submit(long_steps() / 2, 0), p.submit(SHORT, 0)];
    p.client.ensure_runner().unwrap();
    let st = p.wait_state(&ids[0], Duration::from_secs(60), |s| {
        s.state == RunState::Running && s.step >= 5
    });
    let t0 = Instant::now();
    p.client.shutdown(Duration::from_secs(10)).unwrap();
    let dt = t0.elapsed().as_secs_f64();
    eprintln!("S4c shutdown took {dt:.3} s");
    assert!(dt <= 2.0);
    assert!(sys::pid_alive(st.pid.unwrap(), "rcfd"), "shutdown stopped the solver");
    assert_eq!(p.status(&ids[1]).state, RunState::Queued);
    assert!(p.events(&ids[0]).iter().any(|e| e["event"] == "runner_shutdown"));
    p.client.ensure_runner().unwrap();
    for id in &ids {
        assert_eq!(
            p.client.wait(id, Duration::from_secs(300)).unwrap().state,
            RunState::Finished
        );
    }
    assert_once_and_contiguous(&p, &ids);
}

/// S5: the runner survives the client process that started it; the client's captured output reaches EOF
/// (the runner holds no handle of it). Negative control: the lock probe reads false before and after.
/// Known miss (ADR 0123 amendments 2 and 3, ADR 0143): on Windows the direct start, the only route since ADR 0143,
/// hands the runner the client's inheritable handles, so the client's piped stdout does not reach EOF while the runner
/// lives; the survival part is S5a.
#[test]
#[ignore = "ADR 0123 amendment 3, ADR 0143: on Windows the direct start (the only route) hands the runner the client's piped stdout"]
fn s5_runner_survives_the_client_known_miss() {
    let p = Project::new("s5", base_config(1));
    assert!(!p.client.runner_running(), "probe reads a runner before any start");
    let mut client = Command::new(RUNNER)
        .args(["start", "--project"])
        .arg(&p.dir)
        .arg(p.case())
        .args(["--set", &format!("solver.max_iterations={}", long_steps() / 4)])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut out = client.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut out, &mut s);
        let _ = tx.send(s);
    });
    let code = client.wait().unwrap();
    assert!(code.success());
    let id = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the client's stdout did not reach EOF: the runner inherited the pipe")
        .trim()
        .to_string();
    assert!(p.client.runner_running(), "the runner died with its client");
    let s = p.client.wait(&id, Duration::from_secs(300)).unwrap();
    assert_eq!(s.state, RunState::Finished);
    assert!(p.client.runner_running(), "the runner did not outlive the run");
    p.client.shutdown(Duration::from_secs(10)).unwrap();
    assert!(!p.client.runner_running(), "probe reads a runner after the shutdown");
}

/// S6: pause holds the solver between two steps (no step while paused), resume continues, and the result
/// is bitwise equal to an uninterrupted run. Negative controls: the run moves while not paused, and the
/// comparison tells a different run apart.
#[test]
fn s6_pause_resume_is_bitwise_identical() {
    let p = Project::new("s6", base_config(3));
    let m = long_steps();
    let reference = p.submit(m, 0);
    let paused = p.submit(m, 0);
    let other = p.submit(SHORT, 0);
    p.client.ensure_runner().unwrap();
    p.wait_state(&paused, Duration::from_secs(60), |s| {
        s.state == RunState::Running && s.step >= m / 5
    });
    let n0 = p.residual_steps(&paused).len();
    std::thread::sleep(Duration::from_millis(300));
    let moved = p.residual_steps(&paused).len() - n0;
    p.client.pause(&paused).unwrap();
    let t0 = Instant::now();
    let st = p.wait_state(&paused, Duration::from_secs(10), |s| s.state == RunState::Paused);
    let dt = t0.elapsed().as_secs_f64();
    assert!(st.step < m, "the run finished before the pause (set-up defective)");
    let held = p.residual_steps(&paused).len();
    std::thread::sleep(Duration::from_millis(1000));
    let after = p.residual_steps(&paused).len();
    let st2 = p.status(&paused);
    eprintln!(
        "S6 moved {moved} steps in 300 ms running; paused in {dt:.3} s at step {}; {} steps in 1 s paused",
        st.step,
        after - held
    );
    assert!(moved > 0, "negative control: the run did not move while running");
    assert!(dt <= 2.0);
    assert_eq!(
        (after, st2.step, st2.state),
        (held, st.step, RunState::Paused),
        "steps while paused"
    );
    assert_eq!(held as u64, st.step);
    p.client.resume(&paused).unwrap();
    for id in [&reference, &paused, &other] {
        assert_eq!(
            p.client.wait(id, Duration::from_secs(300)).unwrap().state,
            RunState::Finished
        );
    }
    let sol = |id: &str| std::fs::read(p.run_dir(id).join("output/solution.vtu")).unwrap();
    assert!(sol(&reference).len() > 1000);
    assert!(
        sol(&reference) == sol(&paused),
        "paused run differs from the uninterrupted one"
    );
    assert!(
        sol(&reference) != sol(&other),
        "negative control: a different run compares equal"
    );
    let steps = p.residual_steps(&paused);
    assert_eq!(steps, (1..=m).collect::<Vec<_>>());
}

/// A run folder is consistent: records parse, the state is terminal and matches everywhere, no
/// temporary or request files are left, residual steps are contiguous.
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

/// S7: cancel (running, paused, queued) leaves a consistent folder and no solver process.
#[test]
fn s7_cancel_leaves_a_consistent_folder() {
    let p = Project::new("s7", json!({"max_runs": 2, "cpu_threads": 8, "mem_budget_mb": 100000}));
    let run = p.submit(long_steps(), 0);
    let held = p.submit(long_steps(), 0);
    let queued = p.submit(SHORT, 0);
    p.client.pause(&queued).unwrap(); // held before launch: never runs
    p.client.ensure_runner().unwrap();
    let st = p.wait_state(&run, Duration::from_secs(60), |s| {
        s.state == RunState::Running && s.step >= 5
    });
    let st_held = p.wait_state(&held, Duration::from_secs(60), |s| {
        s.state == RunState::Running && s.step >= 5
    });
    p.client.pause(&held).unwrap();
    p.wait_state(&held, Duration::from_secs(10), |s| s.state == RunState::Paused);
    let t0 = Instant::now();
    for id in [&run, &held, &queued] {
        p.client.cancel(id).unwrap();
    }
    for id in [&run, &held, &queued] {
        let s = p.wait_state(id, Duration::from_secs(30), |s| s.state.is_terminal());
        assert_eq!(s.state, RunState::Cancelled, "{id}: {s:?}");
    }
    let dt = t0.elapsed().as_secs_f64();
    eprintln!("S7 three cancels done in {dt:.3} s");
    assert!(dt <= 5.0);
    for id in [&run, &held, &queued] {
        let bad = folder_problems(&p.run_dir(id));
        assert!(bad.is_empty(), "{id}: {bad:?}");
    }
    for pid in [st.pid.unwrap(), st_held.pid.unwrap()] {
        assert!(!sys::pid_alive(pid, "rcfd"), "solver {pid} still alive");
    }
    for id in [&run, &held] {
        assert!(
            p.run_dir(id).join("output/solution.vtu").exists(),
            "{id}: clean stop wrote no results"
        );
        assert!(p.status(id).step < long_steps());
    }
    assert!(p.launches(&queued).is_empty() && !p.run_dir(&queued).join("output").exists());
    // negative control: the checker sees a left-over temporary file and a broken status.json
    let copy = p.dir.join("broken");
    std::fs::create_dir_all(&copy).unwrap();
    for f in ["status.json", "events.jsonl", "residuals.jsonl", "metrics.json"] {
        let _ = std::fs::copy(p.run_dir(&run).join(f), copy.join(f));
    }
    assert!(folder_problems(&copy).is_empty());
    std::fs::write(copy.join("status.json.tmp-1-0"), "{").unwrap();
    assert_eq!(folder_problems(&copy).len(), 1);
    std::fs::write(copy.join("status.json"), "{\"state\": \"canc").unwrap();
    assert!(!folder_problems(&copy).is_empty());
}

/// S8: two start attempts on one project leave exactly one runner; the loser exits 3. Negative control:
/// two projects get two runners.
#[test]
fn s8_two_starts_leave_one_runner() {
    let p = Project::new("s8", base_config(1));
    let q = Project::new("s8neg", base_config(1));
    let serve = |dir: &Path| {
        Command::new(RUNNER)
            .args(["serve", "--project"])
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    };
    let mut a = serve(&p.dir);
    let mut b = serve(&p.dir);
    std::thread::sleep(Duration::from_millis(1500));
    let (ea, eb) = (a.try_wait().unwrap(), b.try_wait().unwrap());
    let alive = usize::from(ea.is_none()) + usize::from(eb.is_none());
    let loser_code = ea.or(eb).and_then(|s| s.code());
    let mut c = serve(&q.dir);
    std::thread::sleep(Duration::from_millis(1000));
    let other_alive = c.try_wait().unwrap().is_none() && (ea.is_none() || eb.is_none());
    eprintln!("S8 alive {alive}, loser exit {loser_code:?}, second project alive {other_alive}");
    for ch in [&mut a, &mut b, &mut c] {
        let _ = ch.kill();
        let _ = ch.wait();
    }
    assert_eq!(alive, 1);
    assert_eq!(loser_code, Some(3));
    assert!(other_alive, "negative control: a second project's runner did not start");
}

/// S9: budgets: refusal with a located reason (memory declared, memory estimated, threads, invalid case);
/// two runs that fit only one at a time never run together. Negative control: a run within the budget runs.
#[test]
fn s9_memory_and_thread_budget_refusals_are_located() {
    let p = Project::new(
        "s9",
        json!({"max_runs": 2, "cpu_threads": 2, "mem_budget_mb": 100, "adopt_probe_ms": 500}),
    );
    let cfg = p.dir.join(".rcfd").join("runner-config.json").display().to_string();
    let with = |mem: Option<u64>, threads: usize, extra: &[&str]| {
        let mut s = settings(SHORT, 0);
        s.mem_mb = mem;
        s.threads = threads;
        s.set.extend(extra.iter().map(|x| x.to_string()));
        p.client.submit(&p.case(), &s).unwrap()
    };
    let declared = with(Some(200), 1, &[]);
    let estimated = with(None, 1, &["mesh.nx=200", "mesh.ny=200"]);
    let threads = with(Some(10), 4, &[]);
    let invalid = with(None, 1, &["physics.nu=-1.0"]);
    let fits = with(None, 1, &[]);
    let pair = [with(Some(60), 1, &[]), with(Some(60), 1, &[])];
    p.client.ensure_runner().unwrap();
    let (max_status, _, _) = sample_concurrency(&p, Duration::from_secs(300));
    let reason = |id: &str| {
        let s = p.status(id);
        assert_eq!(s.state, RunState::Failed, "{id}: {s:?}");
        s.reason.unwrap()
    };
    let (r1, r2, r3, r4) = (
        reason(&declared),
        reason(&estimated),
        reason(&threads),
        reason(&invalid),
    );
    eprintln!("S9 reasons:\n {r1}\n {r2}\n {r3}\n {r4}\n max concurrent of the 60+60 MB pair {max_status}");
    assert!(r1.contains("memory budget") && r1.contains("200 MB") && r1.contains("mem_budget_mb = 100"));
    assert!(r1.contains("settings.mem_mb") && r1.contains(&cfg), "{r1}");
    assert!(
        r2.contains("memory budget") && r2.contains("estimated") && r2.contains("events.jsonl"),
        "{r2}"
    );
    let est: Value = p
        .events(&estimated)
        .into_iter()
        .find(|e| e["event"] == "estimate")
        .unwrap();
    assert_eq!(est["cells"], 40_000);
    assert!(
        r3.contains("CPU budget") && r3.contains("threads = 4") && r3.contains("cpu_threads = 2"),
        "{r3}"
    );
    assert!(
        r4.contains("pre-launch check failed") && r4.contains("physics.nu"),
        "{r4}"
    );
    assert_eq!(p.status(&fits).state, RunState::Finished, "negative control");
    for id in &pair {
        assert_eq!(p.status(id).state, RunState::Finished);
    }
    assert_eq!(max_status, 1, "two 60 MB runs ran together under a 100 MB budget");
    let mut seen = BTreeSet::new();
    for id in [&declared, &estimated, &threads, &invalid] {
        assert!(p.launches(id).is_empty(), "{id} was launched");
        seen.insert(id.clone());
    }
    assert_eq!(seen.len(), 4);
}

/// S10: a JSON case runs from its snapshot (`rcfd run --case-dir` reads JSON when the text starts with `{`)
/// and run.json records the snapshot hash; negative control: a TOML text that is not a case fails located.
#[test]
fn s10_json_case_snapshot_runs() {
    let p = Project::new("s10", base_config(2));
    let v: Value = json!({
        "title": "json cavity", "mesh": {"generator": "cavity_2d", "nx": 8, "ny": 8}, "physics": {"nu": 0.01},
        "boundaries": {"lid": {"velocity": "fixed", "velocity_value": [1.0, 0.0, 0.0]}, "walls": {"velocity": "fixed"}},
        "solver": {"algorithm": "simplec", "convection": "upwind", "max_iterations": 30},
    });
    let case = p.dir.join("cases/cavity.json");
    std::fs::write(&case, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    let bad = p.dir.join("cases/bad.toml");
    std::fs::write(&bad, "[mesh]\ngenerator = 7\n").unwrap();
    let id = p.client.start(&case, &RunSettings::default()).unwrap();
    let bad_id = p.client.start(&bad, &RunSettings::default()).unwrap();
    let s = p.client.wait(&id, Duration::from_secs(120)).unwrap();
    assert_eq!(s.state, RunState::Finished, "{s:?}");
    assert_eq!(s.step, 30);
    let rec = std::fs::read_to_string(p.run_dir(&id).join("run.json")).unwrap();
    // ADR 0130 amendment 1: the same FNV-1a 64 value, in the `fnv64:` spelling of rcfd-project (D2)
    let hash = format!(
        "fnv64:{:016x}",
        rcfd_runner::records::fnv1a64(&std::fs::read(&case).unwrap())
    );
    assert!(rec.contains(&hash), "{rec}");
    let b = p.client.wait(&bad_id, Duration::from_secs(120)).unwrap();
    let reason = b.reason.unwrap_or_default();
    eprintln!("S10 bad case: {reason}");
    assert_eq!(b.state, RunState::Failed);
    assert!(
        reason.contains("pre-launch check failed") && reason.contains("mesh.generator"),
        "{reason}"
    );
}

/// S11 (ADR 0123 amendment 1): a case in `cases/` that references `../meshes/...` runs from its snapshot
/// in `runs/<id>/` (`--case-dir`); negative control: the snapshot without `--case-dir` cannot find the mesh.
#[test]
fn s11_relative_mesh_path_resolves_against_the_case_folder() {
    let p = Project::new("s11", base_config(1));
    let o = Command::new(rcfd_bin())
        .arg("convert")
        .arg(p.case())
        .arg(p.dir.join("meshes/cavity"))
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = CASE.replace(
        "generator = \"cavity_2d\"\nnx = 12\nny = 12",
        "source = \"file\"\nfile = \"../meshes/cavity/polyMesh\"",
    );
    assert!(text.contains("polyMesh"));
    let case = p.dir.join("cases/file.toml");
    std::fs::write(&case, text).unwrap();
    let id = p.client.start(&case, &settings(30, 0)).unwrap();
    let s = p.client.wait(&id, Duration::from_secs(120)).unwrap();
    assert_eq!((s.state, s.step), (RunState::Finished, 30), "{s:?}");
    let o = Command::new(rcfd_bin())
        .arg("run")
        .arg(p.run_dir(&id).join("case.snapshot"))
        .args(["--check-only"])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&o.stderr);
    eprintln!("S11 without --case-dir: exit {:?}: {}", o.status.code(), err.trim());
    assert_eq!(o.status.code(), Some(3));
    assert!(err.contains("mesh.file"), "{err}");
}

/// S5a (ADR 0123 amendment 3): the runner survives the client process that started it. The client's output
/// goes to a file, so the check does not depend on handle inheritance. Negative control: the lock probe
/// reads false before the start and after the shutdown.
#[test]
fn s5a_runner_survives_the_client_process() {
    let p = Project::new("s5a", base_config(1));
    assert!(!p.client.runner_running(), "probe reads a runner before any start");
    let out = p.dir.join("client-out.txt");
    let code = Command::new(RUNNER)
        .args(["start", "--project"])
        .arg(&p.dir)
        .arg(p.case())
        .args(["--set", &format!("solver.max_iterations={}", long_steps() / 4)])
        .stdout(std::fs::File::create(&out).unwrap())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(code.success());
    let id = std::fs::read_to_string(&out).unwrap().trim().to_string();
    assert!(p.client.runner_running(), "the runner died with its client");
    let s = p.client.wait(&id, Duration::from_secs(300)).unwrap();
    assert_eq!(s.state, RunState::Finished);
    assert!(p.client.runner_running(), "the runner did not outlive the run");
    p.client.shutdown(Duration::from_secs(10)).unwrap();
    assert!(!p.client.runner_running(), "probe reads a runner after the shutdown");
}
