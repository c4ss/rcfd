//! Shared set-up of the ADR 0131 tests: temporary folders, the B1 fixture project, headless frames.
#![allow(dead_code)]

use rcfd_project::{Project, RunState, RunStatus};
use rcfd_studio::StudioApp;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The lid-driven cavity case (the `rcfd-mcp` template).
pub const CAVITY: &str = include_str!("../fixtures/cavity.toml");

/// Id of B1's half-created run folder (valid run id, only `case.snapshot`).
pub const HALF_RUN: &str = "r-20261028T120000-beef";

/// A fresh, empty temporary folder unique to this process and call.
pub fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir()
        .join("rcfd-studio-tests")
        .join(format!("{name}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// ADR 0131 amendment 1 item 4: 3 cases, 2 meshes, 8 runs (2 each queued, running, finished, failed) and one
/// half-created run folder.
pub fn b1_project(root: &Path) -> Project {
    let p = Project::create(root, "B1 fixture", "rcfd test").unwrap();
    std::fs::create_dir_all(root.join("cases/sub")).unwrap();
    std::fs::write(root.join("cases/a.toml"), CAVITY).unwrap();
    std::fs::write(root.join("cases/b.toml"), CAVITY.replace("nx = 16", "nx = 12")).unwrap();
    std::fs::write(root.join("cases/sub/c.toml"), CAVITY.replace("ny = 16", "ny = 20")).unwrap();
    std::fs::write(root.join("meshes/m1.msh"), b"mesh one").unwrap();
    std::fs::write(root.join("meshes/m2.msh"), b"mesh two").unwrap();
    let states = [
        RunState::Queued,
        RunState::Running,
        RunState::Finished,
        RunState::Failed,
    ];
    for i in 0..8u64 {
        let case = if i % 2 == 0 { "cases/a.toml" } else { "cases/b.toml" };
        let rec = p.create_run(case, None, "rcfd test", serde_json::json!({})).unwrap();
        let state = states[(i / 2) as usize];
        if state != RunState::Queued {
            let st = RunStatus {
                step: 10 * i,
                time: 0.5 * i as f64,
                reason: (state == RunState::Failed).then(|| "test failure".to_string()),
                ..RunStatus::new(state)
            };
            p.write_status(&rec.id, &st).unwrap();
        }
    }
    let half = root.join("runs").join(HALF_RUN);
    std::fs::create_dir_all(&half).unwrap();
    std::fs::write(half.join("case.snapshot"), CAVITY).unwrap();
    p
}

// ---------------------------------------------------------------- ADR 0133 (48D) process tests

/// The 12 x 12 lid-driven cavity of ADR 0123 with the `lid_drag` report of `rcfd-mcp`'s template; the step count comes
/// from `--set solver.max_iterations` (tolerances too small to stop earlier).
pub const RUN_CASE: &str = r#"title = "w48d cavity"
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
[reports.lid_drag]
type = "force"
patch = "lid"
direction = [1.0, 0.0, 0.0]
component = "viscous"
"#;

/// `rcfd-runner` and `rcfd` of this test's profile, next to the `rcfd-studio` binary (amendment 1 item 7). A stale or
/// missing solver fails loudly.
pub fn binaries() -> (PathBuf, PathBuf) {
    let studio = PathBuf::from(env!("CARGO_BIN_EXE_rcfd-studio"));
    let exe = |n: &str| studio.with_file_name(format!("{n}{}", std::env::consts::EXE_SUFFIX));
    let (runner, rcfd) = (exe("rcfd-runner"), exe("rcfd"));
    assert!(
        runner.is_file(),
        "{} missing: cargo build -p rcfd-runner -p rcfd-cli with this profile",
        runner.display()
    );
    let help = std::process::Command::new(&rcfd)
        .arg("--help")
        .output()
        .unwrap_or_else(|e| panic!("{}: {e} (cargo build -p rcfd-cli with this profile)", rcfd.display()));
    assert!(
        String::from_utf8_lossy(&help.stdout).contains("--status-monitors"),
        "{} is stale (no --status-monitors): rebuild it with cargo build -p rcfd-cli",
        rcfd.display()
    );
    (runner, rcfd)
}

/// A project folder with `cases/cavity.toml` and runner budgets, served by `rcfd-runner serve` started directly (ADR
/// 0128 rule). Dropping it shuts the runner down and kills only the solver pids of its own runs' `launched` events.
pub struct RunnerProject {
    pub dir: PathBuf,
    pub runner: PathBuf,
    pub client: rcfd_runner::client::Client,
    serve: Option<std::process::Child>,
}

impl RunnerProject {
    pub fn new(name: &str, max_runs: usize) -> RunnerProject {
        let (runner, _) = binaries();
        let dir = temp(name).join("proj");
        Project::create(&dir, name, "rcfd test").unwrap();
        std::fs::write(dir.join("cases/cavity.toml"), RUN_CASE).unwrap();
        let cfg = serde_json::json!({"max_runs": max_runs, "cpu_threads": 8, "poll_ms": 100});
        std::fs::write(dir.join(".rcfd/runner-config.json"), cfg.to_string()).unwrap();
        let dir = rcfd_project::paths::canonicalize(&dir).unwrap();
        RunnerProject {
            client: rcfd_runner::client::Client::new(&dir, &runner),
            runner,
            dir,
            serve: None,
        }
    }

    /// Start `rcfd-runner serve --project <dir>` directly and wait until it holds the lock.
    pub fn serve(&mut self) {
        let c = std::process::Command::new(&self.runner)
            .args(["serve", "--project"])
            .arg(&self.dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        self.serve = Some(c);
        let t0 = Instant::now();
        while !self.client.runner_running() {
            assert!(
                t0.elapsed() < Duration::from_secs(20),
                "the runner did not take the lock"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The start plan of the cavity for `steps` steps, 10 MB declared, the store proposal `store` (on or off).
    pub fn plan(&self, steps: u64, store: bool) -> rcfd_studio::runs::StartPlan {
        let mut p = rcfd_studio::runs::StartPlan::new("cases/cavity.toml", RUN_CASE, &self.dir.join("cases")).unwrap();
        p.extra_set = vec![format!("solver.max_iterations={steps}")];
        p.mem_mb = Some(10);
        if let Some(s) = p.store.as_mut() {
            s.on = store;
        }
        p
    }

    pub fn run_dir(&self, id: &str) -> PathBuf {
        self.dir.join("runs").join(id)
    }

    pub fn ids(&self) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(self.dir.join("runs"))
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| e.file_name().into_string().ok())
                    .filter(|n| rcfd_project::runid::is_run_id(n))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }

    pub fn status(&self, id: &str) -> Option<RunStatus> {
        std::fs::read(self.run_dir(id).join("status.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
    }

    /// Event lines of a run as JSON values.
    pub fn events(&self, id: &str) -> Vec<serde_json::Value> {
        std::fs::read_to_string(self.run_dir(id).join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    /// Solver pids of the run's `launched` events.
    pub fn launched(&self, id: &str) -> Vec<u32> {
        self.events(id)
            .iter()
            .filter(|e| e["kind"] == "launched" || e["event"] == "launched")
            .filter_map(|e| e["pid"].as_u64().map(|p| p as u32))
            .collect()
    }

    /// The `to` states of the run's state events, in order, consecutive repeats merged.
    pub fn state_sequence(&self, id: &str) -> Vec<RunState> {
        let mut v: Vec<RunState> = Vec::new();
        for e in self.events(id) {
            if e["kind"] == "state" {
                if let Ok(s) = serde_json::from_value::<RunState>(e["to"].clone()) {
                    if v.last() != Some(&s) {
                        v.push(s);
                    }
                }
            }
        }
        v
    }
}

impl Drop for RunnerProject {
    fn drop(&mut self) {
        let _ = self.client.shutdown(Duration::from_secs(10));
        for id in self.ids() {
            for pid in self.launched(&id) {
                if rcfd_runner::sys::pid_alive(pid, "rcfd") {
                    rcfd_runner::sys::kill(pid);
                }
            }
        }
        if let Some(mut c) = self.serve.take() {
            let t0 = Instant::now();
            while t0.elapsed() < Duration::from_secs(5) {
                if c.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// A Studio that runs `runner` and opens `dir`, drained until the project is open and the run worker ticked once.
pub fn studio_on(dir: &Path, runner: &Path) -> rcfd_studio::Studio {
    let mut st = rcfd_studio::Studio::in_memory();
    st.runner_exe = runner.to_path_buf();
    st.open_project(dir);
    assert!(st.drain_until(Duration::from_secs(30), |s| s.browser().is_some()
        && s.runs.last_tick.is_some()));
    st
}

/// Drain until the model holds a tick that started after `t` (the "next poll"); returns its number.
pub fn next_tick(st: &mut rcfd_studio::Studio, t: Instant, timeout: Duration) -> u64 {
    assert!(
        st.drain_until(timeout, |s| s.runs.last_tick.is_some_and(|(_, at)| at > t)),
        "no tick after the mark within {timeout:?}"
    );
    st.runs.last_tick.unwrap().0
}

/// One headless frame of the app at 1280 x 800.
pub fn frame(app: &mut StudioApp, ctx: &egui::Context) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| app.ui(ui))
}

/// Draw frames until `done` holds (draining the model each frame) or the timeout passes.
pub fn frames_until(
    app: &mut StudioApp,
    ctx: &egui::Context,
    timeout: Duration,
    mut done: impl FnMut(&StudioApp) -> bool,
) -> bool {
    let end = Instant::now() + timeout;
    loop {
        let _ = frame(app, ctx);
        if done(app) {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
