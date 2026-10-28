//! ADR 0143 N3 and N4: the runner started natively by `Client::ensure_runner_pid` (no PowerShell or shell in between).
//! N3: two starts at the same moment leave one runner. N4 (a): the process the client starts is the runner itself;
//! (c): no PowerShell or shell program is named in the non-test sources of rcfd-runner and rcfd-studio. Processes are
//! killed only by pids the test recorded.

use rcfd_runner::client::Client;
use rcfd_runner::sys;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

const RUNNER: &str = env!("CARGO_BIN_EXE_rcfd-runner");

/// N3: when the started processes are counted, after both starts returned (ADR 0123 S8's settle time).
const SETTLE: Duration = Duration::from_millis(1500);

fn project(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rcfd_runner_w48i_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".rcfd")).unwrap();
    let cfg = json!({"max_runs": 1, "cpu_threads": 8, "mem_budget_mb": 100_000, "poll_ms": 100,
                     "adopt_probe_ms": 500, "cancel_grace_ms": 10_000});
    std::fs::write(dir.join(".rcfd/runner-config.json"), cfg.to_string()).unwrap();
    dir
}

/// The pid in `runner.json`, read until the file exists (the runner writes it just after it takes the lock, which is
/// when `ensure_runner_pid` may return), at most 5 s.
fn holder(dir: &Path) -> Option<u32> {
    let read = || -> Option<u32> {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(".rcfd/runner.json")).ok()?).ok()?;
        v["pid"].as_u64().and_then(|p| u32::try_from(p).ok())
    };
    let t0 = Instant::now();
    loop {
        if let Some(p) = read() {
            return Some(p);
        }
        if t0.elapsed() > Duration::from_secs(5) {
            return None;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// `runner.log` lines of a runner that took the lock ("runner <pid> started").
fn started_lines(dir: &Path) -> usize {
    std::fs::read_to_string(dir.join(".rcfd/runner.log"))
        .unwrap_or_default()
        .lines()
        .filter(|l| l.trim_end().ends_with(" started"))
        .count()
}

/// Shuts the projects' runners down and kills the recorded pids that are still runners, also on a failed assertion.
struct Cleanup {
    dirs: Vec<PathBuf>,
    pids: Vec<u32>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        for d in &self.dirs {
            let _ = Client::new(d, RUNNER).shutdown(Duration::from_secs(10));
        }
        for &p in &self.pids {
            if sys::pid_alive(p, "rcfd-runner") {
                sys::kill(p);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
        for d in &self.dirs {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}

/// `ensure_runner_pid` on `a` and on `b` from two threads released by one barrier.
fn race(a: &Path, b: &Path) -> [Result<Option<u32>, String>; 2] {
    let barrier = Arc::new(Barrier::new(2));
    let start = |dir: PathBuf| {
        let barrier = barrier.clone();
        std::thread::spawn(move || {
            let client = Client::new(&dir, RUNNER);
            barrier.wait();
            client.ensure_runner_pid()
        })
    };
    let (ta, tb) = (start(a.to_path_buf()), start(b.to_path_buf()));
    [ta.join().unwrap(), tb.join().unwrap()]
}

fn started(r: &[Result<Option<u32>, String>; 2]) -> Vec<u32> {
    r.iter().filter_map(|x| x.as_ref().ok().copied().flatten()).collect()
}

/// N3: two starts on one project at the same moment leave exactly one runner, the lock holder; negative control: two
/// projects get two runners.
#[test]
fn n3_two_starts_at_once_leave_one_runner() {
    let p = project("n3");
    let (q1, q2) = (project("n3neg1"), project("n3neg2"));
    let mut clean = Cleanup {
        dirs: vec![p.clone(), q1.clone(), q2.clone()],
        pids: Vec::new(),
    };
    let r = race(&p, &p);
    let pids = started(&r);
    clean.pids.extend(&pids);
    std::thread::sleep(SETTLE);
    let alive: Vec<u32> = pids
        .iter()
        .copied()
        .filter(|&x| sys::pid_alive(x, "rcfd-runner"))
        .collect();
    let lock_pid = holder(&p);
    let lines = started_lines(&p);
    let lock = Client::new(&p, RUNNER).runner_running();
    eprintln!(
        "N3: results {r:?}; started {pids:?}; alive after {SETTLE:?} {alive:?}; runner.json {lock_pid:?}; runner.log \
         started lines {lines}; lock probe {lock}"
    );
    // negative control: the same two starts on two projects
    let n = race(&q1, &q2);
    let npids = started(&n);
    clean.pids.extend(&npids);
    std::thread::sleep(SETTLE);
    let nalive: Vec<u32> = npids
        .iter()
        .copied()
        .filter(|&x| sys::pid_alive(x, "rcfd-runner"))
        .collect();
    let nholders = [holder(&q1), holder(&q2)];
    eprintln!("N3 control (two projects): results {n:?}; alive {nalive:?}; runner.json {nholders:?}");
    assert!(r.iter().all(Result::is_ok), "{r:?}");
    assert_eq!(
        pids.len(),
        2,
        "set-up: the two calls did not both start a process (the race did not happen): {r:?}"
    );
    assert_eq!(alive.len(), 1, "live runners {alive:?} of the started {pids:?}");
    assert_eq!(Some(alive[0]), lock_pid, "the live runner is not runner.json's");
    assert_eq!(lines, 1, "runner.log: runners that took the lock");
    assert!(lock, "the lock probe reads no runner");
    assert!(n.iter().all(Result::is_ok), "{n:?}");
    assert_eq!(
        nalive.len(),
        2,
        "negative control: two projects do not show two runners"
    );
    for (i, h) in nholders.iter().enumerate() {
        assert_eq!(*h, n[i].clone().ok().flatten(), "project {i}: runner.json {h:?}");
    }
}

/// N4 (a): the process `ensure_runner_pid` starts is the runner itself (no launcher in between): its pid is
/// `runner.json`'s and it is alive as `rcfd-runner`. Also: with the runner alive the next call starts nothing.
#[test]
fn n4_the_started_process_is_the_runner() {
    let p = project("n4");
    let mut clean = Cleanup {
        dirs: vec![p.clone()],
        pids: Vec::new(),
    };
    let client = Client::new(&p, RUNNER);
    let r = client.ensure_runner_pid();
    let pid = r.clone().ok().flatten();
    clean.pids.extend(pid);
    let lock_pid = holder(&p);
    let alive = pid.is_some_and(|x| sys::pid_alive(x, "rcfd-runner"));
    let again = client.ensure_runner_pid();
    eprintln!("N4 (a): started {r:?}; runner.json {lock_pid:?}; alive {alive}; second call {again:?}");
    assert!(pid.is_some(), "{r:?}");
    assert_eq!(pid, lock_pid, "the started process is not the runner");
    assert!(alive, "the started process is not alive as rcfd-runner");
    assert_eq!(again, Ok(None), "a second call with the runner alive started a process");
}

/// N4 (c): program names a start path must not use, as they would appear in Rust source (compared in lower case).
const SHELLS: [&str; 9] = [
    "\"powershell\"",
    "\"powershell.exe\"",
    "\"pwsh\"",
    "\"pwsh.exe\"",
    "\"cmd\"",
    "\"cmd.exe\"",
    "\"sh\"",
    "\"/bin/sh\"",
    "\"bash\"",
];

/// Lines (1-based) of `text` before its first `#[cfg(test)]` that name a shell or `Start-Process`.
fn shell_lines(text: &str) -> Vec<(usize, String)> {
    let end = text.find("#[cfg(test)]").unwrap_or(text.len());
    text[..end]
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let low = l.to_ascii_lowercase();
            SHELLS.iter().any(|s| low.contains(s)) || low.contains("start-process")
        })
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// N4 (c): no PowerShell, `cmd` or POSIX shell is named in the non-test sources of rcfd-runner and rcfd-studio.
#[test]
fn n4_no_shell_in_the_start_path() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
    let mut files = Vec::new();
    for c in ["rcfd-runner", "rcfd-studio"] {
        rs_files(&crates.join(c).join("src"), &mut files);
    }
    let mut found = Vec::new();
    for f in &files {
        for (line, text) in shell_lines(&std::fs::read_to_string(f).unwrap()) {
            found.push(format!("{}:{line}: {text}", f.display()));
        }
    }
    eprintln!("N4 (c): {} source files; shell names: {found:#?}", files.len());
    assert!(files.len() > 20, "set-up: too few files under {}", crates.display());
    assert!(found.is_empty(), "{found:#?}");
    // negative control: the pre-change launcher line is found, and ignored after a #[cfg(test)]
    let old = "fn launcher_cmd(script: &str) -> (Command, u32) {\n    let (mut c, flags) = console(\"powershell\", \
               DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);\n}\n";
    assert_eq!(shell_lines(old).iter().map(|x| x.0).collect::<Vec<_>>(), [2]);
    assert!(shell_lines(&format!("#[cfg(test)]\n{old}")).is_empty());
}
