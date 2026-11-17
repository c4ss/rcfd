//! ADR 0676 (F-755): a forced control of the edit lock's starvation (S1, S2) and of a lock left behind (D1 to D5), with the
//! test-only hooks of `rcfd_project::atomic::test_hooks`. Built only with `--features test-hooks`.
#![cfg(feature = "test-hooks")]

mod common;

use rcfd_project::atomic::test_hooks::{self, Callback, Hooks};
use rcfd_project::atomic::{edit_claim_path, edit_lock_path, replace_if_hash};
use rcfd_project::records::hash_bytes;
use rcfd_project::{Project, ProjectError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
#[cfg(windows)]
use std::sync::Mutex;
use std::time::{Duration, Instant};

const NOT_HOLDING: u64 = u64::MAX;

fn project(name: &str) -> PathBuf {
    let base = common::scratch(name);
    let p = Project::create(&base.join("p"), name, "0").unwrap();
    p.root().to_path_buf()
}

fn claims_left(root: &Path) -> usize {
    std::fs::read_dir(root.join(".rcfd/locks"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "claim"))
                .count()
        })
        .unwrap_or(0)
}

fn failure_lines(root: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(root.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|v| v["kind"] == "edit_lock_remove_failed")
        .collect()
}

/// The holder of S1 / S2: a cooperating writer that holds the lock 100 ms and, without a claim, re-takes it within one
/// remove and one create (ADR 0669 F1c); with a claim it releases, yields until the claim is gone, and takes it again. It
/// publishes the start of its current hold (microseconds since `epoch`, `NOT_HOLDING` between holds) and its yield.
struct Holder {
    hold_since: Arc<AtomicU64>,
    yielded: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    epoch: Instant,
    thread: std::thread::JoinHandle<usize>,
}

fn start_holder(lock: PathBuf, claim: PathBuf) -> Holder {
    let hold_since = Arc::new(AtomicU64::new(NOT_HOLDING));
    let yielded = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let epoch = Instant::now();
    let thread = {
        let (hold_since, yielded, stop) = (hold_since.clone(), yielded.clone(), stop.clone());
        std::thread::spawn(move || {
            let us = || epoch.elapsed().as_micros() as u64;
            let take = || {
                while std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&lock)
                    .is_err()
                {}
                hold_since.store(us(), Ordering::SeqCst);
            };
            let release = || {
                hold_since.store(NOT_HOLDING, Ordering::SeqCst);
                while std::fs::remove_file(&lock).is_err() && lock.exists() {}
            };
            let mut cycles = 0usize;
            take();
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
                cycles += 1;
                if claim.exists() || stop.load(Ordering::SeqCst) {
                    release();
                    yielded.store(true, Ordering::SeqCst);
                    while claim.exists() && !stop.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    yielded.store(false, Ordering::SeqCst);
                    if stop.load(Ordering::SeqCst) {
                        return cycles;
                    }
                    take();
                } else {
                    release();
                    take();
                }
            }
            release();
            cycles
        })
    };
    Holder {
        hold_since,
        yielded,
        stop,
        epoch,
        thread,
    }
}

/// The adversarial delay of the waiter's attempt (ADR 0676): return once the holder is in the first 50 ms of a hold, or has
/// yielded to a claim, or after 150 ms.
fn adversarial_delay(h: &Holder) -> Callback {
    let (hold_since, yielded, epoch) = (h.hold_since.clone(), h.yielded.clone(), h.epoch);
    Box::new(move |_lock: &Path| {
        let t = Instant::now();
        loop {
            if yielded.load(Ordering::SeqCst) {
                return;
            }
            let hs = hold_since.load(Ordering::SeqCst);
            let now = epoch.elapsed().as_micros() as u64;
            if hs != NOT_HOLDING && now.saturating_sub(hs) < 50_000 {
                return;
            }
            if t.elapsed() >= Duration::from_millis(150) {
                return;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    })
}

struct Trial {
    result: Result<(), String>,
    busy: bool,
    dt: Duration,
    claims: usize,
}

fn starvation_trials(name: &str, claim_off: bool) -> Vec<Trial> {
    let root = project(name);
    let mut out = Vec::new();
    for k in 0..10 {
        let rel = format!("cases/s{k}.txt");
        let lock = edit_lock_path(&root, &rel);
        std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
        let h = start_holder(lock, edit_claim_path(&root, &rel));
        std::thread::sleep(Duration::from_millis(150));
        test_hooks::set(Hooks {
            before_attempt: Some(adversarial_delay(&h)),
            claim_off,
            ..Hooks::default()
        });
        let t0 = Instant::now();
        let r = replace_if_hash(&root, &rel, None, b"x", name);
        let dt = t0.elapsed();
        test_hooks::clear();
        h.stop.store(true, Ordering::SeqCst);
        let cycles = h.thread.join().unwrap();
        let claims = claims_left(&root);
        let busy = matches!(r, Err(ProjectError::LockBusy { .. }));
        eprintln!(
            "{name} trial {k}: {} after {:.1} ms, holder cycles {cycles}, claim files {claims}",
            match &r {
                Ok(_) => "accepted".to_string(),
                Err(e) => format!("{e}"),
            },
            dt.as_secs_f64() * 1e3
        );
        out.push(Trial {
            result: r.map(|_| ()).map_err(|e| e.to_string()),
            busy,
            dt,
            claims,
        });
    }
    out
}

/// S1: the old acquire (the `claim_off` knob, ADR 0669 M1) starves to `LockBusy` under the adversarial delay.
#[test]
fn s1_the_old_acquire_starves_to_lock_busy_under_an_adversarial_delay() {
    let trials = starvation_trials("f755-s1", true);
    let busy = trials.iter().filter(|t| t.busy).count();
    let busy_times: Vec<f64> = trials.iter().filter(|t| t.busy).map(|t| t.dt.as_secs_f64()).collect();
    let claims: usize = trials.iter().map(|t| t.claims).sum();
    eprintln!("S1: LockBusy in {busy} of 10 trials, times {busy_times:?} s; claim files seen {claims}");
    assert!(busy >= 9, "S1: LockBusy in only {busy} of 10");
    assert!(
        trials
            .iter()
            .filter(|t| t.busy)
            .all(|t| t.dt >= Duration::from_secs(2) && t.dt < Duration::from_secs(30)),
        "S1: {busy_times:?}"
    );
    assert_eq!(claims, 0, "S1: a claim file with the claim off");
}

/// S2: the claim of ADR 0669 under the same delay and the same holder: never starved.
#[test]
fn s2_the_claim_is_never_starved_under_the_same_delay() {
    let trials = starvation_trials("f755-s2", false);
    let bad: Vec<_> = trials
        .iter()
        .enumerate()
        .filter(|(_, t)| t.result.is_err() || t.dt >= Duration::from_secs(1))
        .map(|(k, t)| (k, t.result.clone(), t.dt))
        .collect();
    let max = trials.iter().map(|t| t.dt).max().unwrap();
    let claims = trials.last().unwrap().claims;
    eprintln!(
        "S2: {} of 10 accepted in under 1.0 s, max {:.1} ms; claim files left {claims}",
        10 - bad.len(),
        max.as_secs_f64() * 1e3
    );
    assert!(bad.is_empty(), "S2: {bad:?}");
    assert_eq!(claims, 0, "S2: a claim file was left");
}

/// The handle's thread and the time it was opened.
#[cfg(windows)]
type HoldSlot = Arc<Mutex<Option<(std::thread::JoinHandle<()>, Instant)>>>;

/// An `after_lock` hook that holds a handle without delete sharing on the lock for `hold`.
#[cfg(windows)]
fn holding(hold: Duration) -> (Callback, HoldSlot) {
    let slot: HoldSlot = Arc::new(Mutex::new(None));
    let s = slot.clone();
    let f = Box::new(move |lock: &Path| {
        let t = Instant::now();
        let j = test_hooks::hold_without_delete_sharing(lock, hold).expect("open the lock without delete sharing");
        *s.lock().unwrap() = Some((j, t));
    });
    (f, slot)
}

/// D1: with the old drop, a lock held open without delete sharing is left behind and the next edit is `LockBusy`.
#[cfg(windows)]
#[test]
fn d1_the_old_drop_leaves_a_lock_held_open_without_delete_sharing() {
    let root = project("f755-d1");
    let lock = edit_lock_path(&root, "notes/a.md");
    let (f, slot) = holding(Duration::from_millis(300));
    test_hooks::set(Hooks {
        old_drop: true,
        after_lock: Some(f),
        ..Hooks::default()
    });
    let r = replace_if_hash(&root, "notes/a.md", None, b"one", "d1");
    test_hooks::clear();
    let (j, _) = slot.lock().unwrap().take().expect("the hook ran");
    j.join().unwrap();
    let left = lock.exists();
    let t0 = Instant::now();
    let r2 = replace_if_hash(&root, "notes/a.md", Some(&hash_bytes(b"one")), b"two", "d1b");
    let waited = t0.elapsed();
    let lines = failure_lines(&root);
    eprintln!(
        "D1: {r:?}; lock left after the handle closed {left}; second edit {r2:?} after {waited:?}; failure lines {}",
        lines.len()
    );
    assert!(r.is_ok(), "{r:?}");
    assert!(left, "D1: the old drop removed the lock");
    assert!(matches!(r2, Err(ProjectError::LockBusy { .. })), "{r2:?}");
    assert!(
        waited >= Duration::from_secs(2) && waited < Duration::from_secs(30),
        "{waited:?}"
    );
    assert!(lines.is_empty(), "{lines:?}");
}

/// D2: the retried drop removes the same lock once the handle is closed.
#[cfg(windows)]
#[test]
fn d2_the_retried_drop_removes_a_lock_held_open_without_delete_sharing() {
    let root = project("f755-d2");
    let lock = edit_lock_path(&root, "notes/a.md");
    let (f, slot) = holding(Duration::from_millis(300));
    test_hooks::set(Hooks {
        after_lock: Some(f),
        ..Hooks::default()
    });
    let r = replace_if_hash(&root, "notes/a.md", None, b"one", "d2");
    let returned = Instant::now();
    test_hooks::clear();
    let present = lock.exists();
    let (j, opened) = slot.lock().unwrap().take().expect("the hook ran");
    j.join().unwrap();
    let after_open = returned - opened;
    let t0 = Instant::now();
    let r2 = replace_if_hash(&root, "notes/a.md", Some(&hash_bytes(b"one")), b"two", "d2b");
    let second = t0.elapsed();
    let lines = failure_lines(&root);
    eprintln!(
        "D2: {r:?}; returned {:.1} ms after the handle opened, lock present at return {present}; second edit {:?} in {:.1} ms; failure lines {}",
        after_open.as_secs_f64() * 1e3,
        r2.as_ref().map(|_| "accepted"),
        second.as_secs_f64() * 1e3,
        lines.len()
    );
    assert!(r.is_ok(), "{r:?}");
    assert!(!present, "D2: the lock is still there");
    assert!(after_open >= Duration::from_millis(300), "{after_open:?}");
    assert!(lines.is_empty(), "{lines:?}");
    assert!(r2.is_ok() && second < Duration::from_secs(1), "{r2:?} {second:?}");
}

/// D3: a handle held past the retry budget: the lock is left and the failure logged.
#[cfg(windows)]
#[test]
fn d3_a_removal_that_keeps_failing_is_logged() {
    let root = project("f755-d3");
    let lock = edit_lock_path(&root, "notes/a.md");
    let (f, slot) = holding(Duration::from_secs(2));
    test_hooks::set(Hooks {
        after_lock: Some(f),
        ..Hooks::default()
    });
    let r = replace_if_hash(&root, "notes/a.md", None, b"one", "d3");
    test_hooks::clear();
    let lines = failure_lines(&root);
    let (j, _) = slot.lock().unwrap().take().expect("the hook ran");
    j.join().unwrap();
    let left = lock.exists();
    eprintln!("D3: {r:?}; failure lines {lines:?}; lock left after the handle closed {left}");
    assert!(r.is_ok(), "{r:?}");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["path"], "notes/a.md");
    assert_eq!(lines[0]["who"], "d3");
    assert_eq!(lines[0]["attempts"], 12);
    assert!(lines[0]["seq"].is_u64(), "{lines:?}");
    assert!(left, "D3: the lock was removed");
}

/// D4 (every platform): the retry and its log with a synthetic transient error.
#[test]
fn d4_a_synthetic_transient_removal_error_is_retried_then_logged() {
    let root = project("f755-d4");
    let edit = |rel: &str, fail: u32, old_drop: bool| {
        test_hooks::set(Hooks {
            fail_removals: fail,
            old_drop,
            ..Hooks::default()
        });
        let r = replace_if_hash(&root, rel, None, b"x", "d4");
        test_hooks::clear();
        assert!(r.is_ok(), "{r:?}");
        (edit_lock_path(&root, rel).exists(), failure_lines(&root))
    };
    let (a_left, a_lines) = edit("notes/a.md", 3, false);
    let (b_left, b_lines) = edit("notes/b.md", 12, false);
    let (c_left, c_lines) = edit("notes/c.md", 1, true);
    eprintln!(
        "D4: 3 failures: lock left {a_left}, lines {}; 12 failures: lock left {b_left}, lines {b_lines:?}; old drop, 1 failure: lock left {c_left}, lines {}",
        a_lines.len(),
        c_lines.len()
    );
    assert!(!a_left && a_lines.is_empty(), "D4 (3)");
    assert!(b_left && b_lines.len() == 1, "D4 (12)");
    assert_eq!(b_lines[0]["path"], "notes/b.md");
    assert_eq!(b_lines[0]["attempts"], 12);
    assert!(c_left && c_lines.len() == 1, "D4 (old drop)");
}

/// D5: a lock already removed (another writer broke it) is not a failure.
#[test]
fn d5_a_lock_already_gone_is_not_a_failure() {
    let root = project("f755-d5");
    test_hooks::set(Hooks {
        after_lock: Some(Box::new(|lock: &Path| std::fs::remove_file(lock).unwrap())),
        ..Hooks::default()
    });
    let r = replace_if_hash(&root, "notes/a.md", None, b"x", "d5");
    test_hooks::clear();
    let lines = failure_lines(&root);
    let locks = std::fs::read_dir(root.join(".rcfd/locks")).unwrap().count();
    eprintln!(
        "D5: {r:?}; failure lines {}; entries in .rcfd/locks {locks}",
        lines.len()
    );
    assert!(r.is_ok(), "{r:?}");
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(locks, 0);
}
