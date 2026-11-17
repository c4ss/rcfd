//! Shared set-up of the F-834 tests: the engine checks that went with `crates/rcfd-studio/tests` (removed by F-449, ADR
//! 0800 §10), restored here. The case texts, the shipped-case listing and the process harness are those of the deleted
//! `crates/rcfd-studio/tests/common/mod.rs` and `dv/mod.rs`, without the Studio. Test files declare `mod f834;`.
#![allow(dead_code)]

use rcfd_project::{Project, RunState, RunStatus};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The 12 x 12 lid-driven cavity of ADR 0123 with the `lid_drag` report of `rcfd-mcp`'s template; tolerances too small
/// to stop earlier (`common::RUN_CASE` of the deleted Studio tests, byte for byte).
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

/// [`RUN_CASE`] running `steps` steps, the count in the text (`g4::cavity` of the deleted Studio tests).
pub fn cavity(steps: u64) -> String {
    let t = RUN_CASE.replace("max_iterations = 400", &format!("max_iterations = {steps}"));
    assert!(t.contains(&format!("max_iterations = {steps}")));
    t
}

/// The repository root.
pub fn repo() -> PathBuf {
    rcfd_project::paths::canonicalize(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).unwrap()
}

/// A fresh, empty temporary folder unique to this process and call.
pub fn temp(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let d = std::env::temp_dir()
        .join("rcfd-workspace-f834")
        .join(format!("{name}-{}-{nanos}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Files under `dir` (recursively) with extension `ext`, as (path relative to the repository root with `/`, absolute
/// path), sorted.
pub fn files_under(dir: &Path, ext: &str) -> Vec<(String, PathBuf)> {
    let root = repo();
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().map(|e| e.unwrap()) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                let rel = p.strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                out.push((rel, p));
            }
        }
    }
    out.sort();
    out
}

/// The shipped case files under `cases/` with extension `ext`.
pub fn shipped(ext: &str) -> Vec<(String, PathBuf)> {
    files_under(&repo().join("cases"), ext)
}

/// FNV-1a 64 of `bytes` (the hash of `shipped_store_hashes_f476.rs`).
pub fn fnv(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Committed snapshots of a store folder (0 when it does not exist or cannot be read yet).
pub fn store_snapshots(dir: &Path) -> usize {
    if !dir.join(rcfd_tsstore::STORE_FILE).is_file() {
        return 0;
    }
    rcfd_tsstore::Reader::open(dir).map_or(0, |r| r.snapshots().len())
}

/// Poll `f` every 10 ms until it holds or `timeout` passes; whether it held.
pub fn wait_until(timeout: Duration, mut f: impl FnMut() -> bool) -> bool {
    let t0 = Instant::now();
    while t0.elapsed() < timeout {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    f()
}

// ---------------------------------------------------------------- process tests (ADR 0133 harness)

/// `rcfd-runner` and `rcfd` of this test's profile: in the target folder of the test binary (`target/<profile>/`, the
/// parent of `deps/`). `cargo test --workspace` builds both; `cargo test -p rcfd-workspace` alone needs `cargo build -p
/// rcfd-runner -p rcfd-cli` of the same profile first. A missing or stale solver fails loudly (as the Studio's
/// `common::binaries` did).
pub fn binaries() -> (PathBuf, PathBuf) {
    let exe = std::env::current_exe().unwrap();
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("target/<profile>/deps/<test>");
    let bin = |n: &str| dir.join(format!("{n}{}", std::env::consts::EXE_SUFFIX));
    let (runner, rcfd) = (bin("rcfd-runner"), bin("rcfd"));
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

/// A project folder with `cases/cavity.toml` ([`RUN_CASE`]) and runner budgets, served by `rcfd-runner serve` started
/// directly (ADR 0128 rule). Dropping it cancels its own runs that have not ended (a clean stop through the runner, no
/// process is killed) and shuts the runner down.
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
        assert!(
            wait_until(Duration::from_secs(20), || self.client.runner_running()),
            "the runner did not take the lock"
        );
    }

    /// The project's workspace, its runs started by this project's runner.
    pub fn workspace(&self) -> rcfd_workspace::Workspace {
        rcfd_workspace::Workspace::open(&self.dir)
            .unwrap()
            .with_runner_exe(&self.runner)
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

    /// `run.json` of a run.
    pub fn record(&self, id: &str) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(self.run_dir(id).join("run.json")).unwrap()).unwrap()
    }

    /// Wait until run `id` ended (at most `timeout`) and return its status.
    pub fn wait_end(&self, id: &str, timeout: Duration) -> RunStatus {
        assert!(
            wait_until(timeout, || self.status(id).is_some_and(|s| s.state.is_terminal())),
            "run {id} did not end within {timeout:?}: {:?}",
            self.status(id)
        );
        self.status(id).unwrap()
    }
}

impl Drop for RunnerProject {
    fn drop(&mut self) {
        // runs of a failed test that are still queued or running: cancelled cleanly, never killed
        for id in self.ids() {
            if self.status(&id).is_none_or(|s| !s.state.is_terminal()) {
                let _ = self.client.cancel(&id);
            }
        }
        let _ = wait_until(Duration::from_secs(15), || {
            self.ids()
                .iter()
                .all(|id| self.status(id).is_some_and(|s| s.state.is_terminal()))
        });
        let _ = self.client.shutdown(Duration::from_secs(10));
        if let Some(mut c) = self.serve.take() {
            let _ = wait_until(Duration::from_secs(5), || c.try_wait().ok().flatten().is_some());
            let _ = c.wait();
        }
    }
}

/// The value of a workspace command (panics on a refusal or a non-value answer).
pub fn call(ws: &rcfd_workspace::Workspace, name: &str, args: serde_json::Value) -> serde_json::Value {
    match ws.call(name, &args) {
        Some(Ok(rcfd_workspace::Answer::Value(v))) => v,
        other => panic!("{name}: {other:?}"),
    }
}

/// The `RunState` of a status, for messages.
pub fn state_of(st: &Option<RunStatus>) -> Option<RunState> {
    st.as_ref().map(|s| s.state)
}
