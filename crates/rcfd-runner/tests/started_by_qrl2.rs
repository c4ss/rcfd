//! Q-RL-2 (ADR 0801, "Addendum: Runner closes with the Studio", answered by the maintainer 2026-11-17): the runner
//! records who started it, as the additive `started_by` (and `studio_pid`) of `.rcfd/runner.json` (G4).
//! * S1: `rcfd-runner ensure` records `cli`, with no `studio_pid`.
//! * S2: `RCFD_RUNNER_STARTED_BY=studio:<pid>` in the starter's environment records `studio` and that pid.
//! * S3: a malformed value is ignored, and the surface's own value is recorded.
//! * S4: a library client with its surface (`mcp`) records it.
//! * S5: `rcfd-runner serve` started by hand, with no value, writes the record's keys of before, and nothing else.
//! * S6: the record keeps every field of before beside the new ones.
//!
//! Every test has its own project folder (`<temp>/rcfd_runner_qrl2_<pid>_<name>`); its runner is shut down, and the pid
//! it recorded is killed if it is still alive.

use rcfd_runner::client::{Client, RunnerOutput};
use rcfd_runner::started_by::{StartedBy, ENV};
use rcfd_runner::sys;
use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

/// The keys `runner.json` had before Q-RL-2.
const KEYS_BEFORE: [&str; 5] = ["config", "config_source", "pid", "rcfd_bin", "started"];

struct Proj {
    dir: PathBuf,
    pid: Option<u32>,
}

impl Proj {
    fn new(name: &str) -> Proj {
        let dir = std::env::temp_dir().join(format!("rcfd_runner_qrl2_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
        std::fs::write(
            dir.join(".rcfd/runner-config.json"),
            r#"{"max_runs": 1, "poll_ms": 50}"#,
        )
        .unwrap();
        Proj { dir, pid: None }
    }

    fn cli(&self, args: &[&str], by: Option<&str>) {
        let mut c = Command::new(RUNNER);
        c.args(args).args(["--project", &self.dir.display().to_string()]);
        match by {
            Some(v) => c.env(ENV, v),
            None => c.env_remove(ENV),
        };
        // not captured: on Windows the runner it starts inherits the caller's pipes (ADR 0146), and a reader of them
        // would wait until the runner exits
        let status = c
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "rcfd-runner {args:?}: {status}");
    }

    /// `runner.json`, read until it parses (at most 10 s); its pid is kept for the clean-up.
    fn record(&mut self) -> Value {
        let t0 = Instant::now();
        loop {
            let v = std::fs::read_to_string(self.dir.join(".rcfd/runner.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok());
            if let Some(v) = v {
                self.pid = v["pid"].as_u64().and_then(|p| u32::try_from(p).ok());
                return v;
            }
            assert!(
                t0.elapsed() < Duration::from_secs(10),
                "no runner.json in {}",
                self.dir.display()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Proj {
    fn drop(&mut self) {
        let _ = Command::new(RUNNER)
            .args(["shutdown", "--project", &self.dir.display().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let t0 = Instant::now();
        while self.dir.join(".rcfd/runner.json").exists() && t0.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(20));
        }
        if let Some(pid) = self.pid {
            if sys::pid_alive(pid, "rcfd-runner") {
                sys::kill(pid);
            }
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn keys(v: &Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

#[test]
fn s1_ensure_from_the_cli_records_cli() {
    let mut p = Proj::new("s1");
    p.cli(&["ensure", "--quiet"], None);
    let v = p.record();
    assert_eq!(v["started_by"], "cli");
    assert!(v.get("studio_pid").is_none(), "{v}");
}

#[test]
fn s2_the_studio_records_itself_and_its_pid() {
    let mut p = Proj::new("s2");
    p.cli(&["ensure", "--quiet"], Some("studio:4242"));
    let v = p.record();
    assert_eq!(v["started_by"], "studio");
    assert_eq!(v["studio_pid"], 4242);
}

#[test]
fn s3_a_malformed_value_is_ignored_for_the_surface() {
    let mut p = Proj::new("s3");
    p.cli(&["ensure", "--quiet"], Some("studio:nope"));
    let v = p.record();
    assert_eq!(v["started_by"], "cli");
    assert!(v.get("studio_pid").is_none(), "{v}");
}

#[test]
fn s4_a_library_client_records_its_surface() {
    // this process's own variable would win (the Studio's case); the test runs without it
    if std::env::var_os(ENV).is_some() {
        eprintln!("skipped: {ENV} is set in the test's environment");
        return;
    }
    let mut p = Proj::new("s4");
    let c = Client::new(&p.dir, RUNNER)
        .with_output(RunnerOutput::Null)
        .with_started_by(Some(StartedBy::Mcp));
    assert_eq!(c.started_by(), Some(StartedBy::Mcp));
    c.ensure_runner().unwrap();
    let v = p.record();
    assert_eq!(v["started_by"], "mcp");
    assert!(v.get("studio_pid").is_none(), "{v}");
}

#[test]
fn s5_serve_by_hand_writes_the_record_of_before() {
    let mut p = Proj::new("s5");
    let mut child = Command::new(RUNNER)
        .args(["serve", "--project", &p.dir.display().to_string()])
        .env_remove(ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let v = p.record();
    assert_eq!(keys(&v), KEYS_BEFORE, "{v}");
    drop(p);
    let _ = child.wait();
}

#[test]
fn s6_the_new_fields_are_beside_every_field_of_before() {
    let mut p = Proj::new("s6");
    p.cli(&["ensure", "--quiet"], Some("studio:7"));
    let v = p.record();
    let mut want: Vec<String> = KEYS_BEFORE.iter().map(|s| s.to_string()).collect();
    want.extend(["started_by".to_string(), "studio_pid".to_string()]);
    want.sort();
    assert_eq!(keys(&v), want, "{v}");
}
