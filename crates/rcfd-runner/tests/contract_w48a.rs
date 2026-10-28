//! ADR 0130 A1: a project whose runs were made by `rcfd-runner serve` (started directly, as S8) is read by
//! `rcfd-project`: six runs in six situations listed, none skipped, `validate()` clean.
//!
//! `a1_*` uses only interfaces that existed before ADR 0130 (it reads `reason` from `status.json` as JSON), so that it
//! also runs on the pre-change commit with `rcfd-project` added as a dev-dependency: there it shows the negative
//! control (the runs are skipped). Processes are killed only by pids the test recorded.

use rcfd_project::Project;
use rcfd_runner::client::Client;
use rcfd_runner::records::{RunSettings, RunState, RunStatus};
use rcfd_runner::sys;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");
/// Iteration cap of the runs that must still be running (or held) when the folder is read.
const ENDLESS: u64 = 10_000_000; // the solver's cap of solver.max_iterations

const CASE: &str = r#"
title = "w48a contract cavity"
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
    assert!(p.exists(), "{} missing (cargo build -p rcfd-cli)", p.display());
    p
}

/// A project folder (manifest from `rcfd-project`), its runner and every solver it launched; all stopped on drop.
struct Fixture {
    dir: PathBuf,
    client: Client,
    serve: Option<Child>,
}

impl Fixture {
    fn new(name: &str, config: Value) -> Fixture {
        rcfd_bin();
        let dir = std::env::temp_dir().join(format!("rcfd_runner_w48a_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Project::create(&dir, name, "test").unwrap();
        std::fs::write(dir.join("cases/cavity.toml"), CASE).unwrap();
        std::fs::write(dir.join(".rcfd/runner-config.json"), config.to_string()).unwrap();
        Fixture {
            client: Client::new(&dir, RUNNER),
            dir,
            serve: None,
        }
    }

    fn submit(&self, case: &Path, steps: u64) -> String {
        let s = RunSettings {
            mem_mb: Some(10),
            set: vec![format!("solver.max_iterations={steps}")],
            ..RunSettings::default()
        };
        self.client.submit(case, &s).unwrap()
    }

    fn serve(&mut self) {
        let c = Command::new(RUNNER)
            .args(["serve", "--project"])
            .arg(&self.dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        self.serve = Some(c);
    }

    fn wait(&self, id: &str, what: &str, pred: impl Fn(&RunStatus) -> bool) -> RunStatus {
        let t0 = Instant::now();
        loop {
            let s = self.client.status(id).unwrap();
            if pred(&s) {
                return s;
            }
            assert!(
                t0.elapsed() < Duration::from_secs(120),
                "{id}: not {what} in 120 s: {s:?}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn running(&self, id: &str) -> RunStatus {
        self.wait(id, "running", |s| s.state == RunState::Running && s.step >= 5)
    }

    fn launched_pids(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(self.dir.join("runs")).into_iter().flatten().flatten() {
            let text = std::fs::read_to_string(e.path().join("events.jsonl")).unwrap_or_default();
            for l in text.lines() {
                if let Ok(v) = serde_json::from_str::<Value>(l) {
                    if v["event"] == "launched" {
                        out.extend(v["pid"].as_u64().map(|p| p as u32));
                    }
                }
            }
        }
        out
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::write(self.dir.join(".rcfd/shutdown"), b"");
        if let Some(mut c) = self.serve.take() {
            let t0 = Instant::now();
            while c.try_wait().ok().flatten().is_none() && t0.elapsed() < Duration::from_secs(5) {
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = c.kill();
            let _ = c.wait();
        }
        for p in self.launched_pids() {
            if sys::pid_alive(p, "rcfd") {
                sys::kill(p);
            }
        }
        std::thread::sleep(Duration::from_millis(300));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The `status.json` spelling of a state (of either crate's type, so this file builds on the pre-change commit too).
fn state_name(s: impl serde::Serialize) -> Value {
    serde_json::to_value(s).unwrap()
}

/// A1: six runner-made runs (finished, failed by kill, cancelled queued, cancelled running, paused, running) are all
/// listed by `Project::runs()`, none skipped; `validate()` is empty; each summary's state equals `status.json`; the
/// failed and cancelled runs carry a `reason`.
#[test]
fn a1_runner_made_runs_are_listed_by_rcfd_project() {
    let mut f = Fixture::new(
        "a1",
        json!({"max_runs": 5, "cpu_threads": 8, "mem_budget_mb": 100000, "poll_ms": 50, "adopt_probe_ms": 500}),
    );
    let case = f.dir.join("cases/cavity.toml");
    let finished = f.submit(&case, 60);
    let killed = f.submit(&case, ENDLESS);
    let cancelled_queued = f.submit(&case, 60);
    f.client.cancel(&cancelled_queued).unwrap(); // before any runner: cancelled before launch
    let cancelled_running = f.submit(&case, ENDLESS);
    let paused = f.submit(&case, ENDLESS);
    let running = f.submit(&case, ENDLESS);
    f.serve();

    let pid = f.running(&killed).pid.unwrap();
    assert!(sys::kill(pid));
    f.wait(&killed, "failed", |s| s.state.is_terminal());
    f.running(&cancelled_running);
    f.client.cancel(&cancelled_running).unwrap();
    f.wait(&cancelled_running, "cancelled", |s| s.state.is_terminal());
    f.running(&paused);
    f.client.pause(&paused).unwrap();
    f.wait(&paused, "paused", |s| s.state == RunState::Paused);
    f.running(&running);
    f.wait(&finished, "finished", |s| s.state.is_terminal());
    f.wait(&cancelled_queued, "cancelled", |s| s.state.is_terminal());

    // ADR 0130 amendment 2: a clean runner shutdown before the folder is read (the solvers keep running, ADR 0123
    // decision 8), so that no status write of the runner is in flight while `validate` looks for temp files
    f.client.shutdown(Duration::from_secs(10)).unwrap();
    let want = [
        (&finished, "finished"),
        (&killed, "failed"),
        (&cancelled_queued, "cancelled"),
        (&cancelled_running, "cancelled"),
        (&paused, "paused"),
        (&running, "running"),
    ];
    let p = Project::open(&f.dir).unwrap();
    let runs = p.runs().unwrap();
    let issues = p.validate().unwrap();
    eprintln!(
        "A1: listed {}, skipped {}, validate issues {}",
        runs.runs.len(),
        runs.skipped.len(),
        issues.len()
    );
    for s in &runs.skipped {
        eprintln!("A1 skipped: {}: {}", s.path.display(), s.message);
    }
    for i in &issues {
        eprintln!("A1 issue: {}: {}", i.path.display(), i.message);
    }
    let mut bad = Vec::new();
    for (id, state) in want {
        let status: Value =
            serde_json::from_str(&std::fs::read_to_string(f.dir.join("runs").join(id).join("status.json")).unwrap())
                .unwrap();
        let summary = runs.runs.iter().find(|r| &r.id == id);
        let reason = status["reason"].as_str().unwrap_or_default().to_string();
        eprintln!(
            "A1 {state:>9} {id}: status.json {} step {}, summary {:?}, reason '{reason}'",
            status["state"],
            status["step"],
            summary.map(|s| (s.state, s.step, s.case.clone()))
        );
        if status["state"] != state {
            bad.push(format!("{id}: status.json says {}, want {state}", status["state"]));
        }
        match summary {
            Some(s) if state_name(s.state) == status["state"] => {}
            other => bad.push(format!(
                "{id}: summary {:?} differs from status.json {}",
                other.map(|s| s.state),
                status["state"]
            )),
        }
        if matches!(state, "failed" | "cancelled") && reason.is_empty() {
            bad.push(format!("{id}: {state} without a reason"));
        }
    }
    assert_eq!(
        (runs.runs.len(), runs.skipped.len()),
        (6, 0),
        "skipped: {:?}",
        runs.skipped
    );
    assert!(issues.is_empty(), "{issues:?}");
    assert!(bad.is_empty(), "{bad:?}");
    for id in [&paused, &running] {
        f.client.cancel(id).unwrap();
    }
}

/// The `lid_driven_cavity` template of `crates/rcfd-mcp/src/lib.rs` with the field store on.
const LID_CASE: &str = r#"title = "lid-driven cavity Re=100"
[mesh]
generator = "cavity_2d"
nx = 16
ny = 16
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
max_iterations = 600
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
[output]
store_interval = 20
"#;

/// ADR 0130 D3 and D5 through the run service: the store of a runner-made run is at `runs/<id>/fields` (not
/// `output/fields`), and every `residuals.jsonl` line carries the step's `lid_drag`, the last one bit-equal to the final
/// value of `output/monitors.json`.
#[test]
fn runner_stores_fields_in_the_run_folder_and_forwards_monitors() {
    let mut f = Fixture::new(
        "d3d5",
        json!({"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100000, "poll_ms": 50}),
    );
    let case = f.dir.join("cases/lid.toml");
    std::fs::write(&case, LID_CASE).unwrap();
    let id = f.submit(&case, 600);
    f.serve();
    let st = f.wait(&id, "finished", |s| s.state.is_terminal());
    assert_eq!(st.state, RunState::Finished, "{st:?}");
    let rd = f.dir.join("runs").join(&id);
    let lines: Vec<Value> = std::fs::read_to_string(rd.join("residuals.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let monitors: Value =
        serde_json::from_str(&std::fs::read_to_string(rd.join("output/monitors.json")).unwrap()).unwrap();
    let recorded = monitors["series"]["lid_drag"]["y"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .as_f64()
        .unwrap();
    let last = lines.last().unwrap()["monitors"]["lid_drag"].as_f64().unwrap();
    eprintln!(
        "D3/D5 via the runner: {} residual lines, last step {}, lid_drag {last:e} (monitors.json {recorded:e}); store {}",
        lines.len(),
        lines.last().unwrap()["step"],
        rd.join("fields/store.rts").display()
    );
    assert!(lines.iter().all(|l| l["monitors"]["lid_drag"].is_f64()));
    assert_eq!(last.to_bits(), recorded.to_bits());
    assert!(rd.join("fields/store.rts").is_file());
    assert!(!rd.join("output/fields").exists());
}
