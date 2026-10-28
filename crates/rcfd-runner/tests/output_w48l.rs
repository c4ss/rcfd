//! ADR 0146 (job 48L): the output of a started runner, early exits reported at once, and a capturing caller.
//! * E1: through the library default, a runner that exits before it takes the lock (a bad budgets file) is reported at once,
//!   with its exit code and the end of its output.
//! * O1-O5: the CLI's output options, probed with the same failing runner (the only way to make a runner write to stderr).
//! * C1: `rcfd-runner start` run by a capturing caller returns within 3 s while the runner stays alive. Its control holds the
//!   caller on purpose (`--stdout`). C1 is a known miss on Windows (ADR 0146: the runner inherits the caller's pipes).
//! * U1 (Unix): the started runner leads its own process group.
//!
//! Every test has its own project folder (`<temp>/rcfd_runner_w48l_<pid>_<name>`). Processes are killed only by pids the test
//! recorded. The file uses only what exists before the change (the CLI and `ensure_runner`), so ADR 0146's "before" runs use
//! it unchanged.

mod capture;

use capture::Capture;
use rcfd_runner::client::Client;
use rcfd_runner::records::RunState;
use rcfd_runner::sys;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

/// E1, O1-O4: the early-exit report's limit (ADR 0146).
const EARLY: f64 = 2.0;
/// C1: the capturing caller's limit (ADR 0146).
const RETURN: Duration = Duration::from_secs(3);

/// A budgets file the runner refuses before it takes the lock: `max_runs, cpu_threads and poll_ms must be positive`.
const BAD_BUDGETS: &str = r#"{"max_runs": 0}"#;

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

fn healthy() -> String {
    json!({"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100_000, "poll_ms": 100, "adopt_probe_ms": 500})
        .to_string()
}

/// `rcfd` next to `rcfd-runner` (the runs need it); `cargo test -p rcfd-runner` alone needs `cargo build -p rcfd-cli`.
fn rcfd_bin() -> PathBuf {
    let p = Path::new(RUNNER).with_file_name(format!("rcfd{}", std::env::consts::EXE_SUFFIX));
    let help = Command::new(&p)
        .arg("--help")
        .output()
        .unwrap_or_else(|e| panic!("{}: {e} (build it: cargo build -p rcfd-cli)", p.display()));
    assert!(
        String::from_utf8_lossy(&help.stdout).contains("--pause-file"),
        "{} is stale: rebuild it with cargo build -p rcfd-cli",
        p.display()
    );
    p
}

/// A project folder with `cases/cavity.toml` and the budgets file `config`; on drop its runner is shut down, the solvers
/// of its unfinished runs and the recorded runner pids still alive are killed, and the folder is removed.
struct Proj {
    dir: PathBuf,
    pids: Vec<u32>,
}

impl Proj {
    fn new(name: &str, config: &str) -> Proj {
        let dir = std::env::temp_dir().join(format!("rcfd_runner_w48l_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cases")).unwrap();
        std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
        std::fs::write(dir.join("cases/cavity.toml"), CASE).unwrap();
        std::fs::write(dir.join(".rcfd/runner-config.json"), config).unwrap();
        Proj { dir, pids: Vec::new() }
    }

    fn client(&self) -> Client {
        Client::new(&self.dir, RUNNER)
    }

    fn path(&self) -> String {
        self.dir.display().to_string()
    }

    fn case(&self) -> String {
        self.dir.join("cases/cavity.toml").display().to_string()
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.join(".rcfd/runner.log")).unwrap_or_default()
    }

    /// The pid in `runner.json`, read until the file exists (at most 5 s); recorded for the clean-up.
    fn holder(&mut self) -> Option<u32> {
        let t0 = Instant::now();
        loop {
            let pid = std::fs::read_to_string(self.dir.join(".rcfd/runner.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .and_then(|v| v["pid"].as_u64())
                .and_then(|p| u32::try_from(p).ok());
            if let Some(p) = pid {
                self.pids.push(p);
                return Some(p);
            }
            if t0.elapsed() > Duration::from_secs(5) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// The id of the project's only run.
    fn only_run(&self) -> Option<String> {
        self.client()
            .list()
            .ok()
            .and_then(|r| r.into_iter().next())
            .map(|(id, _)| id)
    }
}

impl Drop for Proj {
    fn drop(&mut self) {
        let c = self.client();
        let _ = c.shutdown(Duration::from_secs(10));
        if let Ok(runs) = c.list() {
            for (_, st) in runs {
                if let (Some(p), false) = (st.pid, st.state.is_terminal()) {
                    sys::kill(p);
                }
            }
        }
        for &p in &self.pids {
            if sys::pid_alive(p, "rcfd-runner") {
                sys::kill(p);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `rcfd-runner <args>` with stdin null and stdout, stderr to files in the project: (exit code, seconds, stdout, stderr).
/// Killed after 30 s.
fn cli(p: &Proj, tag: &str, args: &[&str]) -> (Option<i32>, f64, String, String) {
    let (out, err) = (
        p.dir.join(format!("cli-{tag}.out")),
        p.dir.join(format!("cli-{tag}.err")),
    );
    let t0 = Instant::now();
    let mut ch = Command::new(RUNNER)
        .args(args)
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&out).unwrap())
        .stderr(std::fs::File::create(&err).unwrap())
        .spawn()
        .unwrap();
    let status = loop {
        if let Some(s) = ch.try_wait().unwrap() {
            break Some(s);
        }
        if t0.elapsed() > Duration::from_secs(30) {
            let _ = ch.kill();
            let _ = ch.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let dt = t0.elapsed().as_secs_f64();
    let read = |f: &Path| std::fs::read_to_string(f).unwrap_or_default();
    (status.and_then(|s| s.code()), dt, read(&out), read(&err))
}

/// The runner's own line for the bad budgets file: `rcfd-runner: <...runner-config.json>: ...must be positive`.
fn runner_line(text: &str) -> bool {
    text.lines().any(|l| {
        l.trim_start().starts_with("rcfd-runner: ")
            && l.contains("runner-config.json")
            && l.contains("must be positive")
    })
}

/// E1: through the library default, a runner that exits before it takes the lock is reported within 2.0 s, with its exit
/// code and its own error line (from the end of its output in `.rcfd/runner.log`).
#[test]
fn e1_early_exit_is_reported_at_once() {
    let p = Proj::new("e1", BAD_BUDGETS);
    let c = p.client();
    let t0 = Instant::now();
    let r = c.ensure_runner();
    let dt = t0.elapsed().as_secs_f64();
    let (log, lock) = (p.log(), c.runner_running());
    eprintln!("E1: ensure_runner returned after {dt:.3} s: {r:?}; runner.log: {log:?}; lock probe after: {lock}");
    let e = r.expect_err("E1: a runner with a bad budgets file was reported as started");
    assert!(dt <= EARLY, "E1: reported after {dt:.3} s (limit {EARLY} s): {e}");
    assert!(e.contains("exit code 1"), "E1: no exit code in: {e}");
    assert!(
        e.contains("must be positive") && e.contains("runner-config.json"),
        "E1: the runner's own line is not in: {e}"
    );
    assert!(runner_line(&log), "E1: runner.log lacks the runner's line: {log:?}");
    assert!(!lock, "E1: the lock probe reads a runner after the failed start");
}

/// E1 control: with a valid budgets file the early-exit path does not fire; the runner starts and holds the lock.
#[test]
fn e1_control_a_valid_budgets_file_starts() {
    let mut p = Proj::new("e1ok", &healthy());
    let r = p.client().ensure_runner();
    let holder = p.holder();
    let lock = p.client().runner_running();
    eprintln!("E1 control: {r:?}; runner.json {holder:?}; lock probe {lock}");
    assert!(r.is_ok(), "{r:?}");
    assert!(lock && holder.is_some());
}

/// O1: the CLI's default sends the runner's output to `.rcfd/runner.log`; the early exit is reported within 2.0 s.
#[test]
fn o1_default_output_goes_to_the_project_log() {
    let p = Proj::new("o1", BAD_BUDGETS);
    let (code, dt, _, err) = cli(&p, "o1", &["ensure", "--project", &p.path()]);
    let log = p.log();
    eprintln!(
        "O1: exit {code:?} after {dt:.3} s; stderr: {}; runner.log: {log:?}",
        err.trim()
    );
    assert_eq!(code, Some(1), "{err}");
    assert!(dt <= EARLY, "O1: {dt:.3} s (limit {EARLY} s)");
    assert!(err.contains("exit code 1") && err.contains("must be positive"), "{err}");
    assert!(log.contains("must be positive"), "O1: runner.log: {log:?}");
}

/// O2: `--log-file <file>` sends it to that file instead, and the report names the file.
#[test]
fn o2_log_file_option() {
    let p = Proj::new("o2", BAD_BUDGETS);
    std::fs::create_dir_all(p.dir.join("out")).unwrap();
    let x = p.dir.join("out").join("x.log");
    let xs = x.display().to_string();
    let (code, dt, _, err) = cli(&p, "o2", &["ensure", "--project", &p.path(), "--log-file", &xs]);
    let (xlog, log) = (std::fs::read_to_string(&x).unwrap_or_default(), p.log());
    eprintln!(
        "O2: exit {code:?} after {dt:.3} s; stderr: {}; x.log: {xlog:?}; runner.log: {log:?}",
        err.trim()
    );
    assert_eq!(code, Some(1), "{err}");
    assert!(dt <= EARLY, "O2: {dt:.3} s (limit {EARLY} s)");
    assert!(runner_line(&xlog), "O2: x.log lacks the runner's line: {xlog:?}");
    assert!(!log.contains("must be positive"), "O2: runner.log: {log:?}");
    assert!(
        err.contains("exit code 1") && err.contains("must be positive") && err.contains("x.log"),
        "{err}"
    );
}

/// O3: `--stdout` (and its alias `--foreground`) keeps the runner's output on the caller's own streams.
#[test]
fn o3_stdout_option() {
    let p = Proj::new("o3", BAD_BUDGETS);
    for flag in ["--stdout", "--foreground"] {
        let (code, dt, _, err) = cli(&p, &flag[2..], &["ensure", "--project", &p.path(), flag]);
        let log = p.log();
        eprintln!("O3 {flag}: exit {code:?} after {dt:.3} s; stderr: {err:?}; runner.log: {log:?}");
        assert_eq!(code, Some(1), "{flag}: {err}");
        assert!(dt <= EARLY, "O3 {flag}: {dt:.3} s (limit {EARLY} s)");
        assert!(
            runner_line(&err),
            "O3 {flag}: the runner's own line is not on the caller's stderr: {err:?}"
        );
        assert!(err.contains("exit code 1"), "O3 {flag}: {err}");
        assert!(!log.contains("must be positive"), "O3 {flag}: runner.log: {log:?}");
    }
}

/// O4: `--quiet` discards it; the report says so and still gives the exit code.
#[test]
fn o4_quiet_option() {
    let p = Proj::new("o4", BAD_BUDGETS);
    let (code, dt, _, err) = cli(&p, "o4", &["ensure", "--project", &p.path(), "--quiet"]);
    let log = p.log();
    eprintln!(
        "O4: exit {code:?} after {dt:.3} s; stderr: {}; runner.log: {log:?}",
        err.trim()
    );
    assert_eq!(code, Some(1), "{err}");
    assert!(dt <= EARLY, "O4: {dt:.3} s (limit {EARLY} s)");
    assert!(err.contains("exit code 1") && err.contains("discarded"), "{err}");
    assert!(!err.contains("must be positive"), "O4: stderr: {err}");
    assert!(!log.contains("must be positive"), "O4: runner.log: {log:?}");
}

/// O5: two output options are refused, `--log-file` needs a value, and an option without a value does not take the case.
#[test]
fn o5_output_option_errors_and_parsing() {
    rcfd_bin();
    let mut p = Proj::new("o5", &healthy());
    let none_started = |p: &Proj| !p.client().runner_running() && !p.dir.join(".rcfd/runner.json").exists();
    let (a, _, _, aerr) = cli(&p, "o5a", &["ensure", "--project", &p.path(), "--stdout", "--quiet"]);
    let a_none = none_started(&p);
    let (b, _, _, berr) = cli(&p, "o5b", &["ensure", "--project", &p.path(), "--log-file"]);
    let b_none = none_started(&p);
    eprintln!("O5 (a): exit {a:?}: {}; none started {a_none}", aerr.trim());
    eprintln!("O5 (b): exit {b:?}: {}; none started {b_none}", berr.trim());
    assert_eq!(a, Some(1));
    assert!(
        aerr.contains("--log-file") && aerr.contains("--stdout") && aerr.contains("--quiet"),
        "{aerr}"
    );
    assert!(a_none, "O5 (a): a runner was started");
    assert_eq!(b, Some(1));
    assert!(berr.contains("--log-file"), "{berr}");
    assert!(b_none, "O5 (b): a runner was started");
    let case = p.case();
    let (c, dt, out, cerr) = cli(
        &p,
        "o5c",
        &[
            "start",
            "--quiet",
            &case,
            "--set",
            "solver.max_iterations=30",
            "--project",
            &p.path(),
        ],
    );
    p.holder();
    let id = out.trim().to_string();
    eprintln!(
        "O5 (c): exit {c:?} after {dt:.3} s; stdout {id:?}; stderr {}",
        cerr.trim()
    );
    assert_eq!(c, Some(0), "{cerr}");
    assert!(id.starts_with("r-") && !id.contains(char::is_whitespace), "{id:?}");
    let st = p.client().wait(&id, Duration::from_secs(120)).unwrap();
    assert_eq!((st.state, st.step), (RunState::Finished, 30), "{st:?}");
}

/// End a held capture: let the project's run finish, shut the runner down, and give the EOFs 30 s. The seconds from the
/// shutdown request to each EOF.
fn release(p: &Proj, cap: &mut Capture) -> [Option<f64>; 2] {
    let c = p.client();
    if let Some(id) = p.only_run() {
        let _ = c.wait(&id, Duration::from_secs(120));
    }
    let t = Instant::now();
    let _ = c.shutdown(Duration::from_secs(10));
    cap.until(t + Duration::from_secs(30));
    [cap.eof_after(0, t), cap.eof_after(1, t)]
}

/// `rcfd-runner start` of a 60-step cavity on `p` with stdin null and stdout, stderr piped (the capture), `extra` appended.
fn capture_start(p: &Proj, extra: &[&str]) -> Capture {
    let mut cmd = Command::new(RUNNER);
    cmd.args([
        "start",
        "--project",
        &p.path(),
        &p.case(),
        "--set",
        "solver.max_iterations=60",
    ])
    .args(extra)
    .stdin(Stdio::null());
    Capture::start(cmd)
}

/// C1: a capturing caller of `rcfd-runner start` returns (exit and both EOFs) within 3.0 s of the start, while the runner it
/// started stays alive; the run then finishes. Set-up: no runner before the start.
///
/// Known miss on Windows (ADR 0146, registered platform rule): the started runner inherits the client's piped stdout and
/// stderr (std spawns with handle inheritance on; no stable safe way to turn it off), so the capture ends only when the
/// runner exits. Linux: met.
#[test]
#[cfg_attr(
    windows,
    ignore = "known miss on Windows, ADR 0146: the started runner inherits the capturing caller's pipe handles"
)]
fn c1_capturing_caller_returns_within_3_s_known_miss_on_windows() {
    rcfd_bin();
    let mut p = Proj::new("c1", &healthy());
    let c = p.client();
    assert!(!c.runner_running(), "set-up: a runner holds the lock before the start");
    let mut cap = capture_start(&p, &[]);
    let returned = cap.until(cap.t0 + RETURN);
    let holder = p.holder();
    let alive = holder.is_some_and(|x| sys::pid_alive(x, "rcfd-runner"));
    let lock = c.runner_running();
    eprintln!(
        "C1: returned within {RETURN:?}: {returned} ({}, returned {:?}); then lock probe {lock}, runner.json {holder:?} \
         alive {alive}",
        cap.describe(),
        cap.returned_s()
    );
    if !returned {
        let after = release(&p, &mut cap);
        eprintln!(
            "C1: held: the EOFs (stdout, stderr) came {after:?} s after the shutdown request ({})",
            cap.describe()
        );
        panic!(
            "C1: the capturing caller did not return within {RETURN:?} of the start ({})",
            cap.describe()
        );
    }
    assert_eq!(cap.code(), Some(0), "{}", cap.text(1));
    let id = cap.text(0).trim().to_string();
    assert!(
        id.starts_with("r-") && !id.contains(char::is_whitespace),
        "stdout {id:?}"
    );
    assert!(lock, "C1: the lock probe reads no runner at the return");
    assert!(alive, "C1: runner.json's pid {holder:?} is not alive as rcfd-runner");
    let st = c.wait(&id, Duration::from_secs(120)).unwrap();
    assert_eq!(st.state, RunState::Finished, "{st:?}");
    assert!(c.runner_running(), "C1: the runner did not outlive the run");
}

/// C1 control: with `--stdout` the runner holds the caller's pipes by design, so the capture is held 3.0 s after the
/// client's exit (the harness sees a held pipe on every platform); both EOFs follow within 5 s of the shutdown.
#[test]
fn c1_control_stdout_mode_holds_the_caller() {
    rcfd_bin();
    let mut p = Proj::new("c1ctl", &healthy());
    let mut cap = capture_start(&p, &["--stdout"]);
    let held = cap.held_after_exit(RETURN);
    let holder = p.holder();
    let after = release(&p, &mut cap);
    eprintln!(
        "C1 control: held 3.0 s after the client's exit: {held} ({}); runner.json {holder:?}; after the shutdown request \
         the EOFs (stdout, stderr) came {after:?} s",
        cap.describe()
    );
    assert_eq!(cap.code(), Some(0), "{}", cap.text(1));
    assert!(
        held,
        "C1 control: the --stdout start did not hold the caller: the harness does not see a held pipe"
    );
    for (i, a) in after.iter().enumerate() {
        assert!(
            a.is_some_and(|s| s <= 5.0),
            "C1 control: pipe {i} EOF {a:?} s after the shutdown"
        );
    }
}

/// U1 (Unix): the runner `ensure_runner_pid` starts leads its own process group (`process_group(0)`); negative control: this
/// test process is in another group.
#[cfg(unix)]
#[test]
fn u1_the_started_runner_leads_its_own_process_group() {
    let mut p = Proj::new("u1", &healthy());
    let r = p.client().ensure_runner_pid();
    p.holder();
    let pid = r.clone().ok().flatten().expect("U1: no runner was started");
    let pgid = |x: u32| {
        Command::new("ps")
            .args(["-o", "pgid=", "-p", &x.to_string()])
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().ok())
    };
    let (theirs, ours) = (pgid(pid), pgid(std::process::id()));
    eprintln!("U1: runner {pid}: process group {theirs:?}; this test process's group {ours:?}");
    assert_eq!(theirs, Some(pid));
    assert_ne!(
        ours,
        Some(pid),
        "negative control: the test process is in the runner's group"
    );
}
