//! Client library: submit runs, control them and start the project's runner when none is alive.
//!
//! Every call works on the project folder; only [`Client::ensure_runner`] starts a process. Control calls
//! write request files the solver and the runner watch, so they also work while no runner is alive.

use crate::records::{RunSettings, RunState, RunStatus};
use crate::runner::{dot_dir, sibling_exe, EXIT_ALREADY_RUNNING};
use crate::store::{extra, FsStore, RunStore};
use crate::sys;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Where a runner that [`Client::ensure_runner`] starts writes its standard output and error (ADR 0146). Its standard
/// input is the null device in every mode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RunnerOutput {
    /// Appended to `<project>/.rcfd/runner.log` (created when missing), where the runner also writes its own log lines.
    #[default]
    ProjectLog,
    /// Appended to this file (created when missing; its folder must exist). A relative path is taken from the caller's
    /// working folder. The runner's own log lines stay in `.rcfd/runner.log`.
    File(PathBuf),
    /// The caller's own standard output and error. A caller that reads them to the end waits until the runner exits.
    Caller,
    /// Discarded: the null device.
    Null,
}

/// The standard output and error a runner is started with, and for a file its path and its length before the start (the
/// early-exit report reads what the runner appended).
struct StartOutput {
    stdout: Stdio,
    stderr: Stdio,
    file: Option<(PathBuf, u64)>,
}

/// A client of one project's run service.
#[derive(Debug, Clone)]
pub struct Client {
    store: FsStore,
    runner_exe: PathBuf,
    output: RunnerOutput,
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
            output: RunnerOutput::default(),
        }
    }

    /// The same client, with the output of the runners it starts going to `output` (default:
    /// [`RunnerOutput::ProjectLog`]).
    pub fn with_output(mut self, output: RunnerOutput) -> Self {
        self.output = output;
        self
    }

    /// Where the runners this client starts write their output.
    pub fn output(&self) -> &RunnerOutput {
        &self.output
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
        self.ensure_runner_pid().map(|_| ())
    }

    /// [`Client::ensure_runner`], returning the pid of the process this call started (`None`: a runner was alive).
    /// That process is `rcfd-runner serve` itself, started directly (ADR 0143: no PowerShell or shell in between), with
    /// its output where [`Client::output`] says (ADR 0146). It may have lost a start race and exited (exit 3) while the
    /// other runner holds the lock. A runner that exits otherwise before the lock is seen held is reported at once, with
    /// its exit code and the end of its output. The started process is reaped when it exits (`sys::keep_started`).
    // shortcut (Windows): the direct start hands the runner the caller's inheritable handles, whatever its output
    // (ADR 0146 I1-I3).
    pub fn ensure_runner_pid(&self) -> Result<Option<u32>, String> {
        if self.runner_running() {
            return Ok(None);
        }
        let dot = dot_dir(self.project());
        std::fs::create_dir_all(&dot).map_err(err)?;
        let project = std::env::current_dir().map_err(err)?.join(self.project());
        let args = ["serve".into(), "--project".into(), project.display().to_string()];
        let StartOutput {
            stdout,
            stderr,
            file: log,
        } = self.start_output(&dot)?;
        let mut child = sys::spawn_detached(&self.runner_exe, &args, stdout, stderr)
            .map_err(|e| format!("cannot start {}: {e}", self.runner_exe.display()))?;
        let pid = child.id();
        // The child stays here while the lock is awaited: `runner_running` reaps the kept children, which would take its
        // exit status away.
        let t0 = Instant::now();
        let mut exited: Option<ExitStatus> = None;
        let held = loop {
            if self.runner_running() {
                break true;
            }
            if exited.is_none() {
                if let Ok(Some(status)) = child.try_wait() {
                    exited = Some(status);
                    // exit 3: another runner holds the lock; it is awaited like this one
                    if status.code() != Some(EXIT_ALREADY_RUNNING) && !self.runner_running() {
                        break false;
                    }
                }
            }
            if t0.elapsed() >= Duration::from_secs(10) {
                break self.runner_running();
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        if exited.is_none() {
            sys::keep_started(child);
        }
        if held {
            return Ok(Some(pid));
        }
        let see = log.as_ref().map_or_else(|| dot.join("runner.log"), |(p, _)| p.clone());
        Err(match exited {
            Some(status) if status.code() == Some(EXIT_ALREADY_RUNNING) => format!(
                "the runner (pid {pid}) exited with exit code {EXIT_ALREADY_RUNNING} (another runner held the project's \
                 lock), and no runner holds the lock now (see {})",
                see.display()
            ),
            Some(status) => self.early_exit(pid, status, log.as_ref()),
            None => format!("the runner did not take the lock in time (see {})", see.display()),
        })
    }

    /// The started runner's stdout and stderr for [`Client::output`]; a file is opened (append, create) before the start.
    fn start_output(&self, dot: &Path) -> Result<StartOutput, String> {
        let streams = |stdout, stderr| StartOutput {
            stdout,
            stderr,
            file: None,
        };
        let path = match &self.output {
            RunnerOutput::Caller => return Ok(streams(Stdio::inherit(), Stdio::inherit())),
            RunnerOutput::Null => return Ok(streams(Stdio::null(), Stdio::null())),
            RunnerOutput::ProjectLog => dot.join("runner.log"),
            RunnerOutput::File(p) => p.clone(),
        };
        let open = || -> std::io::Result<(std::fs::File, std::fs::File, u64)> {
            let f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
            let len = f.metadata()?.len();
            let g = f.try_clone()?;
            Ok((f, g, len))
        };
        let (f, g, len) = open().map_err(|e| format!("the runner's output file {}: {e}", path.display()))?;
        Ok(StartOutput {
            stdout: Stdio::from(f),
            stderr: Stdio::from(g),
            file: Some((path, len)),
        })
    }

    /// The report of a runner that exited (not with exit 3) before the lock was seen held (ADR 0146 decision 2).
    fn early_exit(&self, pid: u32, status: ExitStatus, log: Option<&(PathBuf, u64)>) -> String {
        let how = status
            .code()
            .map_or_else(|| status.to_string(), |c| format!("exit code {c}"));
        let output = match (log, &self.output) {
            (Some((path, from)), _) => match tail_since(path, *from) {
                Some(t) => format!("its output ({}): {t}", path.display()),
                None => format!("it wrote nothing to {}", path.display()),
            },
            (None, RunnerOutput::Caller) => "its output went to this program's standard output and error".into(),
            (None, _) => "its output was discarded".into(),
        };
        format!("the runner (pid {pid}) exited with {how} before it held the project's lock; {output}")
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

/// The lines appended to `path` after its first `from` bytes: at most the last 8 lines and 2000 bytes, trimmed, blank
/// lines left out, joined with ` | `. `None` when nothing but blanks was appended.
fn tail_since(path: &Path, from: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(from.min(len).max(len.saturating_sub(2000))))
        .ok()?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let tail = &lines[lines.len().saturating_sub(8)..];
    (!tail.is_empty()).then(|| tail.join(" | "))
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
