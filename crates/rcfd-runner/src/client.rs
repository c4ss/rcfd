//! Client library: submit runs, control them and start the project's runner when none is alive.
//!
//! Every call works on the project folder; only [`Client::ensure_runner`] starts a process. Control calls
//! write request files the solver and the runner watch, so they also work while no runner is alive.

use crate::records::{RunSettings, RunState, RunStatus};
use crate::runner::{dot_dir, sibling_exe};
use crate::store::{extra, FsStore, RunStore};
use crate::sys;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// A client of one project's run service.
#[derive(Debug, Clone)]
pub struct Client {
    store: FsStore,
    runner_exe: PathBuf,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Client {
    /// A client of `project` that starts runners from `runner_exe`.
    pub fn new(project: impl Into<PathBuf>, runner_exe: impl Into<PathBuf>) -> Self {
        Client {
            store: FsStore::new(project),
            runner_exe: runner_exe.into(),
        }
    }

    /// Like [`Client::new`] with the runner executable from `RCFD_RUNNER_BIN`, else next to the current
    /// executable.
    pub fn with_default_runner(project: impl Into<PathBuf>) -> Self {
        let exe = std::env::var_os("RCFD_RUNNER_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| sibling_exe("rcfd-runner"));
        Client::new(project, exe)
    }

    /// The project folder.
    pub fn project(&self) -> &Path {
        self.store.project()
    }

    /// Does a live runner hold the project's lock?
    pub fn runner_running(&self) -> bool {
        sys::lock_held(&dot_dir(self.project()).join("runner.lock"))
    }

    /// Start the project's runner, detached, unless one is alive; waits up to 10 s for it to hold the lock.
    pub fn ensure_runner(&self) -> Result<(), String> {
        if self.runner_running() {
            return Ok(());
        }
        let dot = dot_dir(self.project());
        std::fs::create_dir_all(&dot).map_err(err)?;
        let project = std::env::current_dir().map_err(err)?.join(self.project());
        let args = ["serve".into(), "--project".into(), project.display().to_string()];
        let cannot = |e: std::io::Error| format!("cannot start {}: {e}", self.runner_exe.display());
        let mut launcher = sys::spawn_detached(&self.runner_exe, &args).map_err(cannot)?;
        if self.wait_lock(Duration::from_secs(if cfg!(windows) { 5 } else { 10 })) {
            let _ = launcher.try_wait();
            return Ok(());
        }
        if cfg!(windows) {
            // PowerShell did not start the runner in time (seen hung on a loaded host, ADR 0123 amendment 2):
            // start it directly. shortcut: this route hands the runner the caller's inheritable handles.
            let _ = launcher.kill();
            let _ = launcher.wait();
            let _ = sys::spawn_direct(&self.runner_exe, &args).map_err(cannot)?;
            if self.wait_lock(Duration::from_secs(10)) {
                return Ok(());
            }
        }
        Err(format!(
            "the runner did not take the lock in time (see {})",
            dot.join("runner.log").display()
        ))
    }

    fn wait_lock(&self, timeout: Duration) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < timeout {
            if self.runner_running() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        false
    }

    /// Queue a run of `case` without starting a runner; returns the run id.
    pub fn submit(&self, case: &Path, settings: &RunSettings) -> Result<String, String> {
        let version = version_cmd(&sibling_of(&self.runner_exe, "rcfd"))
            .0
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        self.store
            .create_run(case, settings, version)
            .map_err(|e| format!("{}: {e}", case.display()))
    }

    /// Queue a run and make sure a runner is alive.
    pub fn start(&self, case: &Path, settings: &RunSettings) -> Result<String, String> {
        let id = self.submit(case, settings)?;
        self.ensure_runner()?;
        Ok(id)
    }

    /// Every run with its status.
    pub fn list(&self) -> Result<Vec<(String, RunStatus)>, String> {
        let ids = self.store.list_runs().map_err(err)?;
        ids.into_iter().map(|id| self.status(&id).map(|s| (id, s))).collect()
    }

    /// Status of run `id`.
    pub fn status(&self, id: &str) -> Result<RunStatus, String> {
        check_id(id)?;
        self.store.read_status(id).map_err(|e| format!("run {id}: {e}"))
    }

    fn request(&self, id: &str, name: &str, on: bool) -> Result<(), String> {
        let st = self.status(id)?;
        if st.state.is_terminal() {
            return Err(format!("run {id} is already {}", state_name(st.state)));
        }
        let p = self.store.run_dir(id).join(name);
        if on {
            std::fs::write(&p, b"").map_err(err)
        } else {
            match std::fs::remove_file(&p) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(err(e)),
                _ => Ok(()),
            }
        }
    }

    /// Hold run `id`: a queued run is not launched, a running one stops between two steps (in memory).
    pub fn pause(&self, id: &str) -> Result<(), String> {
        self.request(id, extra::PAUSE, true)
    }

    /// Release a held run.
    pub fn resume(&self, id: &str) -> Result<(), String> {
        self.request(id, extra::PAUSE, false)
    }

    /// Cancel run `id` (a running solver stops cleanly and writes its results).
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        self.request(id, extra::CANCEL, true)
    }

    /// Wait until run `id` is finished, failed or cancelled.
    pub fn wait(&self, id: &str, timeout: Duration) -> Result<RunStatus, String> {
        let t0 = Instant::now();
        loop {
            let st = self.status(id)?;
            if st.state.is_terminal() {
                return Ok(st);
            }
            if t0.elapsed() > timeout {
                return Err(format!("run {id} is still {} after {timeout:?}", state_name(st.state)));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Ask the runner to exit cleanly (solvers keep running and are adopted by the next runner) and wait
    /// until it has released the lock.
    pub fn shutdown(&self, timeout: Duration) -> Result<(), String> {
        if !self.runner_running() {
            return Ok(());
        }
        std::fs::write(dot_dir(self.project()).join("shutdown"), b"").map_err(err)?;
        let t0 = Instant::now();
        while self.runner_running() {
            if t0.elapsed() > timeout {
                return Err(format!("the runner still holds the lock after {timeout:?}"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
}

/// `solver --version` (a console program: no window, ADR 0128 finding 17) and its creation flags.
pub(crate) fn version_cmd(solver: &Path) -> (Command, u32) {
    let (mut c, flags) = sys::console(solver, 0);
    c.arg("--version").stdin(Stdio::null()).stderr(Stdio::null());
    (c, flags)
}

fn sibling_of(exe: &Path, name: &str) -> PathBuf {
    exe.with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// Run ids come from users and become path components: only the run-id form (`rcfd_project::runid::is_run_id`).
fn check_id(id: &str) -> Result<(), String> {
    if rcfd_project::runid::is_run_id(id) {
        Ok(())
    } else {
        Err(format!("'{id}' is not a run id"))
    }
}

/// The `status.json` spelling of `state`.
pub fn state_name(state: RunState) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}
