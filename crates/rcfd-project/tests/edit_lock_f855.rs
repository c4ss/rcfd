//! ADR 0751 (F-855): the diagnostic F2d (F2 of ADR 0669 with a record of every refused probe call) and the forced controls
//! CJ (a journal lock whose removal fails once) and CC (a claim whose removal fails once), with the test-only hooks of
//! `rcfd_project::atomic::test_hooks`. Built only with `--features test-hooks`.
#![cfg(feature = "test-hooks")]

mod common;

#[cfg(windows)]
use rcfd_project::atomic::test_hooks::{self, Hooks};
use rcfd_project::atomic::{edit_claim_path, edit_lock_path, replace_if_hash};
use rcfd_project::records::hash_bytes;
use rcfd_project::{Project, ProjectError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
#[cfg(windows)]
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WRONG_BASE: &str = "fnv64:0000000000000000";

fn project(name: &str) -> PathBuf {
    let base = common::scratch(name);
    let p = Project::create(&base.join("p"), name, "0").unwrap();
    p.root().to_path_buf()
}

fn journal_lines(root: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(root.join(".rcfd/edits.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .collect()
}

fn age_s(path: &Path) -> Option<f64> {
    let m = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    Some(m.elapsed().map(|a| a.as_secs_f64()).unwrap_or(0.0))
}

/// One call of the racer: start and end in seconds since the test began, and whether it was accepted.
type RacerCall = (f64, f64, bool);

/// A probe call that did not end in `Conflict`, with the snapshot taken at once after it.
struct Refused {
    call: usize,
    p0: f64,
    p1: f64,
    result: String,
    lock_busy: bool,
    claim: bool,
    lock_text: Option<String>,
    lock_age_s: Option<f64>,
    journal_lock_age_s: Option<f64>,
}

/// The class of a refused probe call (ADR 0751, the table of the diagnostic).
fn class(r: &Refused, racer: &[RacerCall]) -> char {
    if !r.lock_busy {
        'X'
    } else if r.journal_lock_age_s.is_some_and(|a| a >= 1.0) {
        'J'
    } else if r.claim {
        'C'
    } else if racer.iter().any(|&(s, e, ok)| ok && s <= r.p0 + 0.1 && e > r.p1) {
        'S'
    } else {
        'V'
    }
}

/// F2d: F2 of ADR 0669 (the same project, racer, 10000 probe calls and assertions) with a record of every refused call.
#[test]
#[ignore = "heavy record (ADR 0751): F2 of ADR 0669 with a record of every refused probe call; run with --ignored"]
fn f2d_f2_with_a_record_of_every_refused_probe_call() {
    let root = project("f855-f2d");
    std::fs::write(root.join("cases/c.txt"), b"").unwrap();
    let lock = edit_lock_path(&root, "cases/c.txt");
    let claim = edit_claim_path(&root, "cases/c.txt");
    let journal_lock = root.join(rcfd_project::journal::LOCK);
    let t0 = Instant::now();
    let stop = Arc::new(AtomicBool::new(false));
    let racer = {
        let (root, stop) = (root.clone(), stop.clone());
        std::thread::spawn(move || {
            let (mut ok, mut errs, mut calls) = (0usize, Vec::new(), Vec::<RacerCall>::new());
            while !stop.load(Ordering::SeqCst) {
                let cur = std::fs::read(root.join("cases/c.txt")).unwrap();
                let mut next = cur.clone();
                next.extend_from_slice(format!("r {ok}\n").as_bytes());
                let s = t0.elapsed().as_secs_f64();
                let r = replace_if_hash(&root, "cases/c.txt", Some(&hash_bytes(&cur)), &next, "racer");
                calls.push((s, t0.elapsed().as_secs_f64(), r.is_ok()));
                match r {
                    Ok(_) => ok += 1,
                    Err(e) => errs.push(e.to_string()),
                }
            }
            (ok, errs, calls)
        })
    };
    let mut refused = Vec::new();
    let mut conflicts = 0usize;
    let mut max_probe = 0.0f64;
    for call in 0..10000 {
        let p0 = t0.elapsed().as_secs_f64();
        let r = replace_if_hash(&root, "cases/c.txt", Some(WRONG_BASE), b"p", "probe");
        let p1 = t0.elapsed().as_secs_f64();
        max_probe = max_probe.max(p1 - p0);
        match r {
            Err(ProjectError::Conflict { .. }) => conflicts += 1,
            r => refused.push(Refused {
                call,
                p0,
                p1,
                lock_busy: matches!(r, Err(ProjectError::LockBusy { .. })),
                result: format!("{r:?}"),
                claim: claim.exists(),
                lock_text: std::fs::read_to_string(&lock).ok().map(|t| t.trim().to_string()),
                lock_age_s: age_s(&lock),
                journal_lock_age_s: age_s(&journal_lock),
            }),
        }
    }
    stop.store(true, Ordering::SeqCst);
    let (ok, errs, calls) = racer.join().unwrap();
    for r in &refused {
        eprintln!(
            "F2D refused call={} class={} wait_s={:.3} p0={:.3} claim={} lock={:?} lock_age_s={:?} journal_lock_age_s={:?} \
             racer_calls_ended_in_wait={} result={}",
            r.call,
            class(r, &calls),
            r.p1 - r.p0,
            r.p0,
            r.claim,
            r.lock_text,
            r.lock_age_s,
            r.journal_lock_age_s,
            calls.iter().filter(|c| c.1 > r.p0 && c.1 <= r.p1).count(),
            r.result
        );
    }
    let lines = journal_lines(&root);
    let kind = |k: &str| lines.iter().filter(|l| l["kind"] == k).count();
    let mut left: Vec<String> = std::fs::read_dir(root.join(".rcfd/locks"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    left.sort();
    eprintln!(
        "F2D run conflicts={conflicts} refused={} classes={:?} racer_ok={ok} racer_errs={} first_err={:?} lines={} \
         no_seq={} journal_lock_broken={} remove_failed={} stale_broken={} left={left:?} max_probe_s={max_probe:.3} \
         max_racer_s={:.3} total_s={:.1}",
        refused.len(),
        refused.iter().map(|r| class(r, &calls)).collect::<String>(),
        errs.len(),
        errs.first(),
        lines.len(),
        lines.iter().filter(|l| l.get("seq").is_none()).count(),
        lines
            .iter()
            .filter(|l| l.get("journal_lock_broken_s").is_some())
            .count(),
        kind("edit_lock_remove_failed"),
        kind("stale_lock_broken"),
        calls.iter().map(|c| c.1 - c.0).fold(0.0, f64::max),
        t0.elapsed().as_secs_f64()
    );
    assert_eq!(conflicts, 10000, "F2d: {} refused", refused.len());
    assert!(errs.is_empty(), "F2d racer: {errs:?}");
}

/// CJ: the removal of the journal's lock fails once (a handle without delete sharing, 300 ms). The lock must not be left
/// behind: the next edit and a probe that waits for it are served in under 1.0 s, and every journal line has its `seq`.
#[cfg(windows)]
#[test]
fn cj_a_journal_lock_whose_removal_fails_once_is_not_left_behind() {
    let root = project("f855-cj");
    std::fs::write(root.join("cases/c.txt"), b"").unwrap();
    let journal_lock = root.join(rcfd_project::journal::LOCK);
    let opened = Arc::new(Mutex::new(None));
    let slot = opened.clone();
    test_hooks::set(Hooks {
        journal_locked: Some(Box::new(move |lock: &Path| {
            let mut g = slot.lock().unwrap();
            if g.is_none() {
                let t = Instant::now();
                let h = test_hooks::hold_without_delete_sharing(lock, Duration::from_millis(300))
                    .expect("open the journal lock without delete sharing");
                *g = Some((t, h));
            }
        })),
        ..Hooks::default()
    });
    let r1 = replace_if_hash(&root, "cases/c.txt", Some(&hash_bytes(b"")), b"one\n", "w");
    let returned = Instant::now();
    let present = journal_lock.exists();
    test_hooks::clear();
    let (t_open, handle) = opened.lock().unwrap().take().expect("CJ: the handle was not opened");
    handle.join().unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let probe = {
        let root = root.clone();
        std::thread::spawn(move || {
            rx.recv().unwrap();
            let t = Instant::now();
            let r = replace_if_hash(&root, "cases/c.txt", Some(WRONG_BASE), b"p", "probe");
            (
                matches!(r, Err(ProjectError::Conflict { .. })),
                format!("{:?}", r.map(|_| ())),
                t.elapsed(),
            )
        })
    };
    let mut tx = Some(tx);
    test_hooks::set(Hooks {
        after_lock: Some(Box::new(move |_: &Path| {
            if let Some(tx) = tx.take() {
                tx.send(()).unwrap();
            }
            std::thread::sleep(Duration::from_millis(100));
        })),
        ..Hooks::default()
    });
    let t2 = Instant::now();
    let r2 = replace_if_hash(&root, "cases/c.txt", Some(&hash_bytes(b"one\n")), b"two\n", "w");
    let d2 = t2.elapsed();
    test_hooks::clear();
    let (probe_conflict, probe_text, probe_d) = probe.join().unwrap();
    let lines = journal_lines(&root);
    let no_seq = lines.iter().filter(|l| l.get("seq").is_none()).count();
    let waited = returned.duration_since(t_open);
    eprintln!(
        "CJ: edit 1 {:?}, returned {:.1} ms after the handle opened, journal lock present {present}; edit 2 {:?} in {:.1} \
         ms; probe {probe_text} in {:.1} ms; journal lines {} without seq {no_seq}",
        r1.as_ref().map(|_| "accepted"),
        waited.as_secs_f64() * 1e3,
        r2.as_ref().map(|_| "accepted"),
        d2.as_secs_f64() * 1e3,
        probe_d.as_secs_f64() * 1e3,
        lines.len()
    );
    let mut bad = Vec::new();
    if r1.is_err() {
        bad.push("edit 1 failed");
    }
    if present {
        bad.push("the journal lock was left behind");
    }
    if waited < Duration::from_millis(300) {
        bad.push("edit 1 returned before the handle closed");
    }
    if r2.is_err() || d2 >= Duration::from_secs(1) {
        bad.push("edit 2 was not accepted in under 1.0 s");
    }
    if !probe_conflict || probe_d >= Duration::from_secs(1) {
        bad.push("the probe was not answered with a conflict in under 1.0 s");
    }
    if no_seq != 0 {
        bad.push("a journal line has no seq");
    }
    assert!(bad.is_empty(), "CJ: {bad:?}");
}

/// CC: the removal of a writer's claim fails once (a handle without delete sharing, 300 ms). The claim must not be left
/// behind: the next edit of the file is accepted in under 1.0 s.
#[cfg(windows)]
#[test]
fn cc_a_claim_whose_removal_fails_once_is_not_left_behind() {
    let root = project("f855-cc");
    let rel = "notes/a.md";
    let lock = edit_lock_path(&root, rel);
    let claim = edit_claim_path(&root, rel);
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    std::fs::write(&lock, b"0 test 0").unwrap();
    let writer = {
        let root = root.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            let r = replace_if_hash(&root, rel, None, b"x", "w");
            (format!("{:?}", r.as_ref().map(|_| "accepted")), r.is_ok(), t.elapsed())
        })
    };
    let t = Instant::now();
    while !claim.exists() {
        assert!(
            t.elapsed() < Duration::from_millis(1500),
            "CC: the writer made no claim"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let handle = test_hooks::hold_without_delete_sharing(&claim, Duration::from_millis(300))
        .expect("open the claim without delete sharing");
    std::fs::remove_file(&lock).unwrap();
    let (text1, ok1, d1) = writer.join().unwrap();
    handle.join().unwrap();
    let claim_left = claim.exists();
    let t2 = Instant::now();
    let r2 = replace_if_hash(&root, rel, Some(&hash_bytes(b"x")), b"y", "w2");
    let d2 = t2.elapsed();
    eprintln!(
        "CC: edit 1 {text1} in {:.1} ms; claim left {claim_left}; edit 2 {:?} in {:.1} ms",
        d1.as_secs_f64() * 1e3,
        r2.as_ref().map(|_| "accepted"),
        d2.as_secs_f64() * 1e3
    );
    let mut bad = Vec::new();
    if !ok1 {
        bad.push("edit 1 failed");
    }
    if claim_left {
        bad.push("the claim was left behind");
    }
    if r2.is_err() || d2 >= Duration::from_secs(1) {
        bad.push("edit 2 was not accepted in under 1.0 s");
    }
    assert!(bad.is_empty(), "CC: {bad:?}");
}
