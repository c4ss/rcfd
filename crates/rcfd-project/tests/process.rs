//! ADR 0122 checks that need child processes: A2 (atomic replace across a kill), J2 (JSONL across a
//! kill), R2 (run-id uniqueness over threads and processes). Children re-run this test binary with
//! `RCFD_PROJECT_CHILD` set; `child_entry` does nothing without it.

mod common;

use rcfd_project::jsonl::{self, Appender};
use rcfd_project::{atomic, runid, Project, ResidualLine};
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Duration;

const LEN: usize = 1 << 20;
/// Registered: 20 kills per writer kind, kill delays 5..=62 ms on a fixed schedule.
const KILLS: u64 = 20;
fn delay(i: u64) -> Duration {
    Duration::from_millis(5 + (i * 23) % 58)
}

#[test]
fn child_entry() {
    let Ok(mode) = std::env::var("RCFD_PROJECT_CHILD") else {
        return;
    };
    let arg = PathBuf::from(std::env::var("RCFD_PROJECT_ARG").unwrap());
    // Writers stop by themselves after 30 s, so a parent that panics before its kill leaves no runaway.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let ready = || {
        println!("READY");
        std::io::stdout().flush().unwrap();
    };
    match mode.as_str() {
        "atomic" | "naive" => {
            for g in 1.. {
                let b = common::payload(g, LEN);
                if mode == "atomic" {
                    atomic::atomic_write(&arg, &b).unwrap();
                } else {
                    let _ = std::fs::write(&arg, &b);
                }
                if g == 1 {
                    ready();
                }
                if std::time::Instant::now() > deadline {
                    return;
                }
            }
        }
        "append" | "append-split" => {
            let (mut a, _) = Appender::open(&arg).unwrap();
            let mut f = std::fs::OpenOptions::new().append(true).open(&arg).unwrap();
            for step in 0.. {
                let line = residual(step);
                if mode == "append" {
                    a.append(&line).unwrap();
                } else {
                    // Two writes per line: a kill between them leaves a torn line.
                    let s = serde_json::to_vec(&line).unwrap();
                    f.write_all(&s[..s.len() / 2]).unwrap();
                    std::thread::sleep(Duration::from_millis(1));
                    f.write_all(&s[s.len() / 2..]).unwrap();
                    f.write_all(b"\n").unwrap();
                }
                if step == 0 {
                    ready();
                }
                if std::time::Instant::now() > deadline {
                    return;
                }
            }
        }
        "runs" => {
            let p = Project::open(&arg).unwrap();
            let n: usize = std::env::var("RCFD_PROJECT_N").unwrap().parse().unwrap();
            let mut ids = Vec::new();
            for i in 0..n {
                ids.push(
                    p.create_run(
                        "cases/a.json",
                        None,
                        "0",
                        serde_json::json!({"proc": std::process::id(), "i": i}),
                    )
                    .unwrap()
                    .id,
                );
            }
            println!("IDS {}", ids.join(","));
        }
        m => panic!("unknown child mode {m}"),
    }
}

fn residual(step: u64) -> ResidualLine {
    ResidualLine {
        step,
        time: step as f64 * 1e-3,
        residuals: (0..200)
            .map(|k| (format!("eq{k}"), 1.0 / (1.0 + (step + k) as f64)))
            .collect(),
        monitors: Default::default(),
    }
}

fn spawn(mode: &str, arg: &Path, n: Option<usize>) -> Child {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", "child_entry", "--nocapture", "--test-threads", "1"])
        .env("RCFD_PROJECT_CHILD", mode)
        .env("RCFD_PROJECT_ARG", arg)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(n) = n {
        c.env("RCFD_PROJECT_N", n.to_string());
    }
    c.spawn().unwrap()
}

fn wait_line(child: &mut Child, prefix: &str) -> String {
    let out = child.stdout.as_mut().unwrap();
    let mut r = BufReader::new(out);
    let mut line = String::new();
    loop {
        line.clear();
        assert!(r.read_line(&mut line).unwrap() > 0, "child exited before {prefix}");
        // libtest prints "test child_entry ... " before the child's own output on the same line.
        if let Some(i) = line.find(prefix) {
            return line[i..].trim_end().to_string();
        }
    }
}

/// Kill a writer child `KILLS` times while a reader thread hammers; returns (bad observations, reads).
fn kill_loop(mode: &str, tag: &str) -> (u64, u64) {
    let dir = common::scratch(tag);
    let path = dir.join("status.json");
    let mut bad = 0;
    let mut reads_total = 0;
    for i in 0..KILLS {
        let mut child = spawn(mode, &path, None);
        wait_line(&mut child, "READY");
        let stop = Arc::new(AtomicBool::new(false));
        let (reads, partial) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
        let reader = {
            let (path, stop, reads, partial) = (path.clone(), stop.clone(), reads.clone(), partial.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    if let Ok(b) = std::fs::read(&path) {
                        reads.fetch_add(1, Ordering::Relaxed);
                        if !common::payload_ok(&b) {
                            partial.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
        };
        std::thread::sleep(delay(i));
        child.kill().unwrap();
        child.wait().unwrap();
        stop.store(true, Ordering::Relaxed);
        reader.join().unwrap();
        let after = std::fs::read(&path).map(|b| common::payload_ok(&b)).unwrap_or(false);
        bad += partial.load(Ordering::Relaxed) + u64::from(!after);
        reads_total += reads.load(Ordering::Relaxed);
    }
    eprintln!("A2 {mode}: {bad} bad observations in {reads_total} reads + {KILLS} post-kill reads");
    (bad, reads_total)
}

#[test]
fn a2_atomic_replace_survives_kill_mid_write() {
    let (bad, reads) = kill_loop("atomic", "a2");
    assert_eq!(bad, 0);
    assert!(reads >= KILLS, "reader did not overlap the writer: {reads}");
}

#[test]
fn a2_negative_control_in_place_writer_killed_leaves_partial_files() {
    let (bad, _) = kill_loop("naive", "a2neg");
    assert!(bad >= 1);
}

/// Kill an appender child; the reader must never fail, and reopen-append must leave a clean file.
fn append_kills(mode: &str, tag: &str) -> u64 {
    let dir = common::scratch(tag);
    let mut torn_seen = 0;
    for i in 0..KILLS / 2 {
        let path = dir.join(format!("residuals{i}.jsonl"));
        let mut child = spawn(mode, &path, None);
        wait_line(&mut child, "READY");
        std::thread::sleep(delay(i));
        child.kill().unwrap();
        child.wait().unwrap();
        let l = jsonl::read::<ResidualLine>(&path).unwrap_or_else(|e| panic!("kill {i}: {e}"));
        assert!(!l.items.is_empty());
        assert!(
            l.items.iter().enumerate().all(|(k, r)| r == &residual(k as u64)),
            "kill {i}: committed lines are intact"
        );
        torn_seen += u64::from(l.torn.is_some());
        let (mut a, removed) = Appender::open(&path).unwrap();
        assert_eq!(removed, l.torn.map_or(0, |t| t.len));
        a.append(&residual(1_000_000)).unwrap();
        drop(a);
        let again = jsonl::read::<ResidualLine>(&path).unwrap();
        assert_eq!((again.items.len(), again.torn), (l.items.len() + 1, None));
    }
    eprintln!("J2 {mode}: torn tails after {} kills: {torn_seen}", KILLS / 2);
    torn_seen
}

#[test]
fn j2_jsonl_survives_kill_mid_append() {
    append_kills("append", "j2");
}

#[test]
fn j2_split_writer_produces_real_torn_tails_that_are_tolerated() {
    // Control that the kill test can produce torn lines at all; the reader tolerates them.
    assert!(append_kills("append-split", "j2split") >= 1);
}

/// Registered: 8 threads x 32 runs and 4 processes x 64 runs, concurrently, on one project.
#[test]
fn r2_run_ids_unique_over_threads_and_processes() {
    let dir = common::scratch("r2");
    let p = Project::create(&dir.join("proj"), "r2", "0").unwrap();
    std::fs::write(p.root().join("cases/a.json"), b"{}").unwrap();
    let children: Vec<Child> = (0..4).map(|_| spawn("runs", p.root(), Some(64))).collect();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let p = p.clone();
            std::thread::spawn(move || {
                (0..32)
                    .map(|i| {
                        p.create_run("cases/a.json", None, "0", serde_json::json!({"i": i}))
                            .unwrap()
                            .id
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut ids: Vec<String> = threads.into_iter().flat_map(|h| h.join().unwrap()).collect();
    for mut c in children {
        let line = wait_line(&mut c, "IDS ");
        assert!(c.wait().unwrap().success());
        ids.extend(line["IDS ".len()..].split(',').map(String::from));
    }
    let unique: HashSet<&String> = ids.iter().collect();
    assert_eq!((ids.len(), unique.len()), (512, 512));
    let runs = p.runs().unwrap();
    assert_eq!((runs.runs.len(), runs.skipped.len()), (512, 0));
    assert!(p.validate().unwrap().is_empty());
    let mut sorted = ids.clone();
    sorted.sort();
    assert!(
        runs.runs.windows(2).all(|w| w[0].created <= w[1].created),
        "listing is in time order"
    );
    eprintln!("R2: 512 unique ids, first {} last {}", sorted[0], sorted[511]);
}

/// Forced contention: 16 threads claim at one fixed second from a 16-value suffix space.
fn contention(naive: bool) -> (usize, u32) {
    let dir = common::scratch(if naive { "r2neg" } else { "r2c" });
    let barrier = Arc::new(Barrier::new(16));
    let hs: Vec<_> = (0..16u64)
        .map(|t| {
            let (dir, barrier) = (dir.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                if naive {
                    // Random suffix, non-exclusive create: the clash goes unnoticed.
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64;
                    let s = rcfd_core::hash::fnv64(&(nanos ^ t.wrapping_mul(0x9e37_79b9_7f4a_7c15)).to_le_bytes()) % 16;
                    let id = format!("r-20260101T000000-{s:04x}");
                    std::fs::create_dir_all(dir.join(&id)).unwrap();
                    (id, 0)
                } else {
                    let (id, _, clashes) = runid::claim_with(&dir, "20260101T000000", 4).unwrap();
                    (id, clashes)
                }
            })
        })
        .collect();
    let res: Vec<(String, u32)> = hs.into_iter().map(|h| h.join().unwrap()).collect();
    let unique: HashSet<&String> = res.iter().map(|r| &r.0).collect();
    (unique.len(), res.iter().map(|r| r.1).sum())
}

#[test]
fn r2_forced_contention_every_claim_succeeds_with_a_distinct_id() {
    let (unique, clashes) = contention(false);
    eprintln!("R2 contention: {unique} unique of 16, {clashes} clashes resolved");
    assert_eq!(unique, 16);
}

#[test]
fn r2_negative_control_non_exclusive_claims_collide() {
    let (unique, _) = contention(true);
    eprintln!("R2 negative: {unique} unique of 16");
    assert!(unique < 16);
}
