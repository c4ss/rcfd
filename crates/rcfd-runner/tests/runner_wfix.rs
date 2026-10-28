//! ADR 0128 process tests of the run service fixes. Runners are started directly (`serve`), never through PowerShell;
//! processes are killed only by pids the test recorded.

use rcfd_runner::client::Client;
use rcfd_runner::records::RunSettings;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

const CASE: &str = r#"
title = "wfix cavity"
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

fn project(name: &str, config: serde_json::Value) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd_runner_wfix_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("cases")).unwrap();
    std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
    std::fs::write(dir.join("cases/cavity.toml"), CASE).unwrap();
    std::fs::write(dir.join(".rcfd/runner-config.json"), config.to_string()).unwrap();
    dir
}

fn serve(dir: &Path) -> Command {
    let mut c = Command::new(RUNNER);
    c.args(["serve", "--project"])
        .arg(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    c
}

/// F1: a config that names `rcfd_bin` is rejected with a located error and the named program never runs.
#[test]
fn f01_config_naming_rcfd_bin_is_rejected_and_never_run() {
    let dir = project("f01", json!({}));
    let marker = dir.join("marker.txt");
    let script = if cfg!(windows) {
        let s = dir.join("evil.cmd");
        std::fs::write(&s, format!("@echo off\r\necho ran> \"{}\"\r\n", marker.display())).unwrap();
        s
    } else {
        let s = dir.join("evil.sh");
        std::fs::write(&s, format!("#!/bin/sh\necho ran > '{}'\n", marker.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&s, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        s
    };
    let cfg = json!({"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100_000, "poll_ms": 50, "rcfd_bin": script});
    std::fs::write(dir.join(".rcfd/runner-config.json"), cfg.to_string()).unwrap();
    // a queued run without mem_mb: a runner that accepted the config would start the pre-launch check at once
    Client::new(&dir, RUNNER)
        .submit(&dir.join("cases/cavity.toml"), &RunSettings::default())
        .unwrap();
    let mut child = serve(&dir).spawn().unwrap();
    let t0 = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break Some(s);
        }
        if t0.elapsed() > Duration::from_secs(10) {
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let alive = status.is_none();
    if alive {
        let _ = child.kill();
        let _ = child.wait();
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = std::io::Read::read_to_string(&mut e, &mut err);
    }
    std::thread::sleep(Duration::from_secs(2));
    let ran = marker.exists();
    eprintln!(
        "F1: runner exit {status:?} (still serving after 10 s: {alive}), script ran {ran}, stderr: {}",
        err.trim()
    );
    assert!(!ran, "the program named by rcfd_bin was run");
    assert!(!alive, "the runner accepted a config naming rcfd_bin");
    assert!(!status.unwrap().success());
    assert!(err.contains("runner-config.json") && err.contains("rcfd_bin"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

fn rcfd_bin() -> PathBuf {
    Path::new(RUNNER).with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX))
}

fn events(dir: &Path, id: &str) -> Vec<serde_json::Value> {
    std::fs::read_to_string(dir.join("runs").join(id).join("events.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn count(dir: &Path, id: &str, kind: &str) -> usize {
    events(dir, id).iter().filter(|e| e["event"] == kind).count()
}

fn append_event(dir: &Path, id: &str, ev: serde_json::Value) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(dir.join("runs").join(id).join("events.jsonl"))
        .unwrap();
    writeln!(f, "{ev}").unwrap();
}

fn stop(dir: &Path, mut child: std::process::Child) {
    let _ = std::fs::write(dir.join(".rcfd/shutdown"), b"");
    let t0 = Instant::now();
    while child.try_wait().unwrap().is_none() && t0.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn fast_config() -> serde_json::Value {
    json!({"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100_000, "poll_ms": 50, "adopt_probe_ms": 200})
}

/// F3 scan leg: a run left `queued` with a `launched` event is adopted (live pid) or failed (dead pid), never relaunched.
#[test]
fn f03_queued_run_with_launched_event_is_not_relaunched() {
    let settings = RunSettings {
        mem_mb: Some(10),
        set: vec!["solver.max_iterations=100000".into()],
        ..RunSettings::default()
    };
    let launched = |pid: u32| json!({"event": "launched", "pid": pid, "launch_seq": 1, "threads": 1, "mem_mb": 10});
    // (a) dead pid
    let dir = project("f03dead", fast_config());
    let id = Client::new(&dir, RUNNER)
        .submit(&dir.join("cases/cavity.toml"), &settings)
        .unwrap();
    let mut short = Command::new(rcfd_bin())
        .arg("--version")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let dead = short.id();
    short.wait().unwrap();
    append_event(&dir, &id, launched(dead));
    let child = serve(&dir).spawn().unwrap();
    let client = Client::new(&dir, RUNNER);
    let t0 = Instant::now();
    while !client.status(&id).unwrap().state.is_terminal() && t0.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(50));
    }
    let st = client.status(&id).unwrap();
    let relaunched = count(&dir, &id, "launched") - 1;
    let pids: Vec<u64> = events(&dir, &id)
        .iter()
        .filter(|e| e["event"] == "launched")
        .filter_map(|e| e["pid"].as_u64())
        .collect();
    stop(&dir, child);
    for p in pids.iter().skip(1) {
        rcfd_runner::sys::kill(*p as u32);
    }
    eprintln!(
        "F3 dead pid: state {:?} after {:?}, relaunched {relaunched}: {:?}",
        st.state,
        t0.elapsed(),
        st.reason
    );
    assert_eq!(relaunched, 0, "a launched run was put back in the queue");
    assert_eq!(st.state, rcfd_runner::records::RunState::Failed);
    let _ = std::fs::remove_dir_all(&dir);
    // (b) live pid
    let dir = project("f03live", fast_config());
    let id = Client::new(&dir, RUNNER)
        .submit(&dir.join("cases/cavity.toml"), &settings)
        .unwrap();
    let mut solver = Command::new(rcfd_bin())
        .arg("run")
        .arg(dir.join("cases/cavity.toml"))
        .args(["--quiet", "--set", "solver.max_iterations=100000", "--out"])
        .arg(dir.join("own-output"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    append_event(&dir, &id, launched(solver.id()));
    let child = serve(&dir).spawn().unwrap();
    std::thread::sleep(Duration::from_secs(2));
    let st = client_status(&dir, &id);
    let relaunched = count(&dir, &id, "launched") - 1;
    let adopted = count(&dir, &id, "adopted");
    let pids: Vec<u64> = events(&dir, &id)
        .iter()
        .filter(|e| e["event"] == "launched")
        .filter_map(|e| e["pid"].as_u64())
        .collect();
    stop(&dir, child);
    let _ = solver.kill();
    let _ = solver.wait();
    for p in pids.iter().skip(1) {
        rcfd_runner::sys::kill(*p as u32);
    }
    eprintln!(
        "F3 live pid {}: state {:?} pid {:?}, adopted events {adopted}, relaunched {relaunched}",
        solver.id(),
        st.state,
        st.pid
    );
    assert_eq!(relaunched, 0, "a launched run was put back in the queue");
    assert_eq!(
        (st.state, st.pid),
        (rcfd_runner::records::RunState::Running, Some(solver.id()))
    );
    assert_eq!(adopted, 1);
    std::thread::sleep(Duration::from_millis(300));
    let _ = std::fs::remove_dir_all(&dir);
}

fn client_status(dir: &Path, id: &str) -> rcfd_runner::records::RunStatus {
    Client::new(dir, RUNNER).status(id).unwrap()
}

/// F4: a solver whose binary is not named `rcfd` stays adopted across a runner restart (the probe uses the real name).
#[test]
fn f04_renamed_solver_survives_a_runner_restart() {
    let dir = project(
        "f04",
        json!({"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100_000, "poll_ms": 50, "adopt_probe_ms": 500}),
    );
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    let solver = dir
        .join("bin")
        .join(format!("mysolver{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(rcfd_bin(), &solver).unwrap();
    let steps = if cfg!(debug_assertions) { 10_000 } else { 40_000 };
    let settings = RunSettings {
        mem_mb: Some(10),
        set: vec![format!("solver.max_iterations={steps}")],
        ..RunSettings::default()
    };
    let client = Client::new(&dir, RUNNER);
    let id = client.submit(&dir.join("cases/cavity.toml"), &settings).unwrap();
    let start = || {
        serve(&dir)
            .env("RCFD_BIN", &solver)
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
    };
    let mut a = start();
    let t0 = Instant::now();
    let pid = loop {
        let s = client.status(&id).unwrap();
        if s.state == rcfd_runner::records::RunState::Running && s.step >= 5 {
            break s.pid.unwrap();
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "not running: {s:?}");
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = a.kill();
    let _ = a.wait();
    let b = start();
    std::thread::sleep(Duration::from_secs(3));
    let mid = client.status(&id).unwrap();
    let alive = rcfd_runner::sys::pid_alive(pid, "mysolver");
    let end = if mid.state.is_terminal() {
        mid.clone()
    } else {
        client.wait(&id, Duration::from_secs(300)).unwrap()
    };
    stop(&dir, b);
    if rcfd_runner::sys::pid_alive(pid, "mysolver") {
        rcfd_runner::sys::kill(pid);
    }
    eprintln!(
        "F4: 3 s after the restart: {:?} step {} (solver {pid} alive {alive}); end {:?} step {}: {:?}",
        mid.state, mid.step, end.state, end.step, end.reason
    );
    assert!(alive, "set-up: the solver died with the first runner");
    assert_eq!(mid.state, rcfd_runner::records::RunState::Running, "{:?}", mid.reason);
    assert_eq!(end.state, rcfd_runner::records::RunState::Finished, "{:?}", end.reason);
    std::thread::sleep(Duration::from_millis(300));
    let _ = std::fs::remove_dir_all(&dir);
}

/// F11: a negative, NaN, infinite or huge `wait --timeout` is a clean error, not a panic.
#[test]
fn f11_bad_wait_timeouts_are_clean_errors() {
    let dir = project("f11", json!({}));
    let mut bad = Vec::new();
    for v in ["-1", "NaN", "inf", "1e300"] {
        let o = Command::new(RUNNER)
            .args(["wait", "r-20260101T000000-abcd", "--project"])
            .arg(&dir)
            .args(["--timeout", v])
            .output()
            .unwrap();
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        eprintln!("F11 --timeout {v}: exit {:?}, stderr {}", o.status.code(), err.trim());
        if o.status.code() != Some(1) || !err.contains("--timeout") || !err.contains(v) || err.contains("panicked") {
            bad.push(v);
        }
    }
    assert!(bad.is_empty(), "not a clean error: {bad:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// F15: the arguments of the detached start parse back, by the real argv rules of the runner binary, into exactly the
/// arguments given: trailing backslashes, `C:\`, quotes, spaces, an empty argument. ADR 0143: the start is the runner
/// itself through `sys::detached_command` (std's quoting); the hand-built PowerShell command line and its script are
/// gone with that route.
#[cfg(windows)]
#[test]
fn f15_command_line_round_trips_through_the_real_argv_parser() {
    let dir = project("f15", json!({}));
    let proj = dir.display().to_string();
    let mut bad = Vec::new();
    for arg in [
        r"C:\tmp\a b\",
        r"C:\",
        r"C:\x\\",
        r#"a"b"#,
        r#"a\"b"#,
        r#"a\\"b c\"#,
        "it's",
        "",
        "plain",
    ] {
        let args: Vec<String> = vec![
            "start".into(),
            arg.into(),
            "--no-runner".into(),
            "--project".into(),
            proj.clone(),
        ];
        let (mut cmd, _) = rcfd_runner::sys::detached_command(Path::new(RUNNER), &args);
        let o = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).output().unwrap();
        let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
        // `start` reads the case file first; its error names the case path exactly as parsed
        let ok = o.status.code() == Some(1) && err.starts_with(&format!("rcfd-runner: {arg}: "));
        eprintln!("F15 {arg:?}: {err} ({ok})");
        if !ok {
            bad.push(arg);
        }
    }
    assert!(bad.is_empty(), "arguments that do not round-trip: {bad:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// F15: `ensure_runner` starts the runner for a project path that ends in a backslash.
#[cfg(windows)]
#[test]
fn f15_runner_starts_with_a_trailing_backslash_project_path() {
    let dir = project("f15start", json!({}));
    let with_sep = PathBuf::from(format!("{}\\", dir.display()));
    let client = Client::new(&with_sep, RUNNER);
    let started = client.ensure_runner();
    let running = client.runner_running();
    let stopped = client.shutdown(Duration::from_secs(10));
    eprintln!(
        "F15 start with {}: {started:?}, running {running}, shutdown {stopped:?}",
        with_sep.display()
    );
    assert!(started.is_ok() && running, "{started:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
