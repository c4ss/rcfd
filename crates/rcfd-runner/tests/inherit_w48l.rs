//! ADR 0146 I1-I4, an investigation run by hand: what keeps a capturing caller waiting after a short-lived client has
//! started a long-lived process. Pure std, no rcfd code. This test binary plays every role through `i_child_entry`, which
//! does nothing without its environment:
//! * the client: started with stdin null and stdout, stderr piped (the capture);
//! * an intermediate stage (I3);
//! * a daemon that lives until a stop file appears (at most 30 s).
//!
//! The variants:
//! * I1: the client starts the daemon with null stdio (ADR 0143's start).
//! * I2: the client starts it with all three standard handles given as files (`Stdio::from(File)`).
//! * I3: the client starts the intermediate (null stdio) and waits for it; the intermediate starts the daemon and exits.
//! * I4: the client starts nothing (the harness control).
//!
//! `cargo test --release -p rcfd-runner --test inherit_w48l -- --ignored --nocapture --test-threads 1`

mod capture;

use capture::Capture;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const ROLE: &str = "RCFD_W48L_ROLE";
const DIR: &str = "RCFD_W48L_DIR";
/// "Held": an EOF still pending this long after the client's exit (ADR 0146).
const HOLD: Duration = Duration::from_secs(3);

/// This test binary in `role`, working in `dir`.
fn stage(role: &str, dir: &Path) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", "i_child_entry", "--nocapture", "--test-threads", "1"])
        .env(ROLE, role)
        .env(DIR, dir);
    c
}

/// Child entry point of I1-I4 (does nothing without its environment).
#[test]
#[allow(clippy::zombie_processes)] // the daemon must outlive the client and the intermediate that start it
fn i_child_entry() {
    let (Some(role), Some(dir)) = (std::env::var_os(ROLE), std::env::var_os(DIR)) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let daemon = || stage("daemon", &dir);
    match role.to_str().unwrap_or_default() {
        "client-i1" | "intermediate" => {
            daemon()
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
        }
        "client-i2" => {
            let log = || {
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join("daemon.log"))
                    .unwrap()
            };
            daemon()
                .stdin(File::open(dir.join("empty.txt")).unwrap())
                .stdout(log())
                .stderr(log())
                .spawn()
                .unwrap();
        }
        "client-i3" => {
            let s = stage("intermediate", &dir)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(s.success(), "the intermediate stage failed: {s}");
        }
        "client-i4" => {}
        "daemon" => {
            let me = std::process::id();
            std::fs::write(dir.join(format!("daemon-{me}.started")), "").unwrap();
            let t0 = Instant::now();
            while !dir.join("stop").exists() && t0.elapsed() < Duration::from_secs(30) {
                std::thread::sleep(Duration::from_millis(20));
            }
            std::fs::write(dir.join(format!("daemon-{me}.done")), "").unwrap();
        }
        other => panic!("unknown role {other}"),
    }
}

/// Daemons that started and have not finished, in `dir`.
fn open_daemons(dir: &Path) -> Vec<String> {
    let names: Vec<String> = std::fs::read_dir(dir)
        .map(|r| {
            r.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default();
    names
        .iter()
        .filter_map(|n| n.strip_suffix(".started"))
        .filter(|s| !names.contains(&format!("{s}.done")))
        .map(String::from)
        .collect()
}

/// I1-I4: held or released 3.0 s after the client's exit, per variant. Asserted: I4 is released (the harness sees a prompt
/// EOF) and no daemon is left; the other outcomes are reported against ADR 0146's predictions.
#[test]
#[ignore = "ADR 0146 investigation I1-I4: run by hand (see the file's doc comment)"]
fn i1_i4_inheritance_matrix() {
    let predicted_held = |v: &str| cfg!(windows) && v != "i4";
    let mut rows = Vec::new();
    let mut left = Vec::new();
    for v in ["i1", "i2", "i3", "i4"] {
        let dir = std::env::temp_dir().join(format!("rcfd_runner_w48l_inherit_{}_{v}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("empty.txt"), "").unwrap();
        let mut cmd = stage(&format!("client-{v}"), &dir);
        cmd.stdin(Stdio::null());
        let mut cap = Capture::start(cmd);
        let held = cap.held_after_exit(HOLD);
        let stop = Instant::now();
        std::fs::write(dir.join("stop"), "").unwrap();
        cap.until(stop + Duration::from_secs(30));
        let t = Instant::now();
        while !open_daemons(&dir).is_empty() && t.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
        let open = open_daemons(&dir);
        let line = format!(
            "{v}: {} (predicted {}); {}; EOFs after the stop file: {:?} s, {:?} s; daemons left {open:?}",
            if held { "held" } else { "released" },
            if predicted_held(v) { "held" } else { "released" },
            cap.describe(),
            cap.eof_after(0, stop),
            cap.eof_after(1, stop),
        );
        eprintln!("I {line}");
        rows.push((v, held, cap.code()));
        left.extend(open);
        let _ = std::fs::remove_dir_all(&dir);
    }
    let differ: Vec<&str> = rows
        .iter()
        .filter(|(v, held, _)| *held != predicted_held(v))
        .map(|r| r.0)
        .collect();
    eprintln!("I1-I4: outcomes that differ from the prediction: {differ:?}");
    assert!(rows.iter().all(|r| r.2 == Some(0)), "a client failed: {rows:?}");
    assert!(
        !rows[3].1,
        "I4: the harness did not see the prompt EOF of a client that starts nothing"
    );
    assert!(left.is_empty(), "daemons left running: {left:?}");
}
