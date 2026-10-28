//! Where the Studio finds the run service `rcfd-runner` and the solver `rcfd` (ADR 0141): `RCFD_RUNNER_BIN` when set,
//! else the folder of the Studio, each folder of `PATH`, and `target/release` of a development checkout, in that order.
//! A folder counts only when it holds both programs: the runner starts the `rcfd` next to itself (or `RCFD_BIN`), and
//! `Client::submit` reads that `rcfd --version`. [`Search::find`] only looks at files; [`check_solver`] runs
//! `rcfd --help` (no window) to refuse a solver older than the flags the runner passes.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

/// The command that builds everything the Studio starts (the runner, the solver, the MCP server of the chat).
pub const BUILD: &str = "cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli -p rcfd-mcp";

/// Environment variable naming the runner program (an explicit choice: never silently replaced by a search).
pub const RUNNER_ENV: &str = "RCFD_RUNNER_BIN";

/// Environment variable naming the solver program (the runner reads it too).
pub const SOLVER_ENV: &str = "RCFD_BIN";

/// Flags of `rcfd run` that the runner passes on every launch (ADR 0130 D3, D5); a solver without them fails every run.
pub const SOLVER_FLAGS: [&str; 2] = ["--store-dir", "--status-monitors"];

/// How long `rcfd --help` may take before the solver counts as broken.
pub const HELP_TIMEOUT: Duration = Duration::from_secs(10);

/// The file name of program `name` on this platform (`rcfd-runner.exe` on Windows).
pub fn exe_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// Where a folder of the search comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// `RCFD_RUNNER_BIN`.
    Env,
    /// The folder of the Studio executable.
    Studio,
    /// A folder of `PATH`.
    Path,
    /// `target/release` of a development checkout.
    Dev,
}

impl Place {
    /// How the place is named in messages.
    pub fn label(self) -> &'static str {
        match self {
            Place::Env => RUNNER_ENV,
            Place::Studio => "next to the Studio",
            Place::Path => "PATH",
            Place::Dev => "development checkout",
        }
    }
}

/// The two programs, found together.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    /// `rcfd-runner`.
    pub runner: PathBuf,
    /// `rcfd`, the program the runner will start (`RCFD_BIN`, else the one next to the runner).
    pub solver: PathBuf,
    /// Where the runner was found.
    pub place: Place,
}

impl Found {
    /// One line for the toolbar.
    pub fn describe(&self) -> String {
        match self.runner.parent() {
            Some(d) => format!("rcfd-runner and rcfd: {} ({})", d.display(), self.place.label()),
            None => format!("rcfd-runner: {} ({})", self.runner.display(), self.place.label()),
        }
    }
}

/// What the search looks at: the process environment in the app ([`Search::from_env`]), folders of a test otherwise.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Search {
    /// `RCFD_RUNNER_BIN`.
    pub runner_env: Option<PathBuf>,
    /// `RCFD_BIN`.
    pub solver_env: Option<PathBuf>,
    /// The folder of the Studio executable.
    pub exe_dir: Option<PathBuf>,
    /// `PATH` as the OS gives it.
    pub path: Option<OsString>,
    /// The current folder (`cargo run` from a checkout keeps it at the checkout).
    pub cwd: Option<PathBuf>,
    /// `CARGO_TARGET_DIR`.
    pub target_dir: Option<PathBuf>,
}

fn non_empty(v: Option<OsString>) -> Option<OsString> {
    v.filter(|s| !s.is_empty())
}

impl Search {
    /// The search of this process.
    pub fn from_env() -> Search {
        Search {
            runner_env: non_empty(std::env::var_os(RUNNER_ENV)).map(PathBuf::from),
            solver_env: non_empty(std::env::var_os(SOLVER_ENV)).map(PathBuf::from),
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(Path::to_path_buf)),
            path: non_empty(std::env::var_os("PATH")),
            cwd: std::env::current_dir().ok(),
            target_dir: non_empty(std::env::var_os("CARGO_TARGET_DIR")).map(PathBuf::from),
        }
    }

    /// A search that takes `runner` as given (as `RCFD_RUNNER_BIN` does) and nothing else.
    pub fn explicit(runner: &Path) -> Search {
        Search {
            runner_env: Some(runner.to_path_buf()),
            ..Search::default()
        }
    }

    /// The folders searched without `RCFD_RUNNER_BIN`, in order, each once: the Studio's folder, each folder of
    /// `PATH`, then `target/release` of every checkout (a folder holding `Cargo.toml`) among the ancestors of the
    /// Studio's folder and of the current folder, and `CARGO_TARGET_DIR/release`.
    pub fn folders(&self) -> Vec<(PathBuf, Place)> {
        let mut out: Vec<(PathBuf, Place)> = Vec::new();
        let mut push = |d: PathBuf, p: Place| {
            if !d.as_os_str().is_empty() && !out.iter().any(|(x, _)| *x == d) {
                out.push((d, p));
            }
        };
        if let Some(d) = &self.exe_dir {
            push(d.clone(), Place::Studio);
        }
        if let Some(p) = &self.path {
            for d in std::env::split_paths(p) {
                push(d, Place::Path);
            }
        }
        for start in [&self.exe_dir, &self.cwd].into_iter().flatten() {
            for a in start.ancestors() {
                if a.join("Cargo.toml").is_file() {
                    push(a.join("target").join("release"), Place::Dev);
                }
            }
        }
        if let Some(t) = &self.target_dir {
            push(t.join("release"), Place::Dev);
        }
        out
    }

    /// The solver the runner at `runner` would start.
    fn solver_of(&self, runner: &Path) -> PathBuf {
        match &self.solver_env {
            Some(s) => s.clone(),
            None => runner.with_file_name(exe_name("rcfd")),
        }
    }

    /// Find both programs (file checks only). The error lists every folder searched, what was found alone, and the
    /// build command.
    pub fn find(&self) -> Result<Found, String> {
        let solver_note = |s: &Path| match &self.solver_env {
            Some(_) => format!("{SOLVER_ENV} names {}, which is not a file", s.display()),
            None => format!("no {} next to it", exe_name("rcfd")),
        };
        if let Some(r) = &self.runner_env {
            let s = self.solver_of(r);
            if r.is_file() && s.is_file() {
                return Ok(Found {
                    runner: r.clone(),
                    solver: s,
                    place: Place::Env,
                });
            }
            let what = if r.is_file() {
                format!("{RUNNER_ENV} names {}, but there is {}", r.display(), solver_note(&s))
            } else {
                format!("{RUNNER_ENV} names {}, which is not a file", r.display())
            };
            return Err(format!(
                "{what}. Unset {RUNNER_ENV} to search the usual folders, or point it at rcfd-runner with rcfd next to \
                 it. Build both with: {BUILD}"
            ));
        }
        let mut searched = Vec::new();
        let mut partial = Vec::new();
        for (dir, place) in self.folders() {
            let runner = dir.join(exe_name("rcfd-runner"));
            let solver = self.solver_of(&runner);
            match (runner.is_file(), solver.is_file()) {
                (true, true) => {
                    return Ok(Found { runner, solver, place });
                }
                (true, false) => partial.push(format!("{} ({})", runner.display(), solver_note(&solver))),
                (false, true) if self.solver_env.is_none() => partial.push(format!(
                    "{} (no {} next to it)",
                    solver.display(),
                    exe_name("rcfd-runner")
                )),
                _ => {}
            }
            searched.push(format!("  {}: {}", place.label(), dir.display()));
        }
        let mut msg = format!(
            "{} and {} were not found together (the runner starts the rcfd next to it). Searched, in order:\n{}",
            exe_name("rcfd-runner"),
            exe_name("rcfd"),
            if searched.is_empty() {
                "  (no folder: the Studio's own folder, PATH and the current folder are unknown)".to_string()
            } else {
                searched.join("\n")
            }
        );
        if !partial.is_empty() {
            msg.push_str(&format!("\nFound alone: {}", partial.join("; ")));
        }
        msg.push_str(&format!(
            "\nBuild them with: {BUILD}\nthen start target{s}release{s}{} from the checkout (or put rcfd-runner and \
             rcfd together in one folder on PATH, or set {RUNNER_ENV}).",
            exe_name("rcfd-studio"),
            s = std::path::MAIN_SEPARATOR
        ));
        Err(msg)
    }
}

/// Refuse a solver that cannot run or is older than the flags the runner passes ([`SOLVER_FLAGS`]): runs
/// `solver --help` as a console program without a window, at most [`HELP_TIMEOUT`].
pub fn check_solver(solver: &Path) -> Result<(), String> {
    let (mut c, _) = rcfd_runner::sys::console(solver, 0);
    c.arg("--help")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let child = c
        .spawn()
        .map_err(|e| format!("{}: cannot run it ({e}). Rebuild with: {BUILD}", solver.display()))?;
    // the help text is larger than a pipe buffer: read it while waiting, on a thread, with a time limit
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    let out = match rx.recv_timeout(HELP_TIMEOUT) {
        Ok(Ok(o)) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Ok(Err(e)) => {
            return Err(format!(
                "{}: cannot run it ({e}). Rebuild with: {BUILD}",
                solver.display()
            ))
        }
        Err(_) => {
            rcfd_runner::sys::kill(pid);
            return Err(format!(
                "{}: `--help` did not answer within {} s. Rebuild with: {BUILD}",
                solver.display(),
                HELP_TIMEOUT.as_secs()
            ));
        }
    };
    let missing: Vec<&str> = SOLVER_FLAGS.iter().copied().filter(|f| !out.contains(f)).collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{} is older than this Studio: its --help does not name {}, which the runner passes on every launch, so \
             every run would fail at launch. Rebuild with: {BUILD}",
            solver.display(),
            missing.join(" and ")
        ))
    }
}
