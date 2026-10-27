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
    eprintln!("F1: runner exit {status:?} (still serving after 10 s: {alive}), script ran {ran}, stderr: {}", err.trim());
    assert!(!ran, "the program named by rcfd_bin was run");
    assert!(!alive, "the runner accepted a config naming rcfd_bin");
    assert!(!status.unwrap().success());
    assert!(err.contains("runner-config.json") && err.contains("rcfd_bin"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}
