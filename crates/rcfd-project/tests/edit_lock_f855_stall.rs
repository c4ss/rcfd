//! ADR 0758 (F-855): the forced controls of a stall. CW: a waiter stalled past the wait is not refused at its second look
//! (change W of `acquire`). CH: a holder stalled for the whole wait is class S of the amended F2. CV: a starved probe is
//! class V and fails. With the test-only hooks of `rcfd_project::atomic::test_hooks`; built only with
//! `--features test-hooks`.
#![cfg(feature = "test-hooks")]

mod common;
#[path = "edit_lock_f2/mod.rs"]
mod f2;

use rcfd_project::atomic::test_hooks::{self, Hooks};
use rcfd_project::atomic::{edit_claim_path, edit_lock_path, replace_if_hash};
use rcfd_project::{Project, ProjectError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn project(name: &str) -> PathBuf {
    let base = common::scratch(name);
    let p = Project::create(&base.join("p"), name, "0").unwrap();
    p.root().to_path_buf()
}

/// CW: the lock is busy at the waiter's first look, free for 1.8 s while the waiter is stalled for 2.2 s, and busy again
/// (another short hold) at its second look. The waiter must go on looking and get the lock.
#[test]
fn cw_a_waiter_stalled_past_the_wait_is_not_refused_at_its_second_look() {
    let root = project("f855-cw");
    let rel = "notes/a.md";
    let lock = edit_lock_path(&root, rel);
    let claim = edit_claim_path(&root, rel);
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    std::fs::write(&lock, b"0 test 0").unwrap();
    let looks = Arc::new(AtomicU32::new(0));
    let (tx, rx) = std::sync::mpsc::channel::<u32>();
    let writer = {
        let (root, looks) = (root.clone(), looks.clone());
        std::thread::spawn(move || {
            let mut n = 0u32;
            test_hooks::set(Hooks {
                before_attempt: Some(Box::new(move |_: &Path| {
                    n += 1;
                    looks.store(n, Ordering::SeqCst);
                    let _ = tx.send(n);
                    if n == 2 {
                        std::thread::sleep(Duration::from_millis(2200));
                    }
                })),
                ..Hooks::default()
            });
            let t = Instant::now();
            let r = replace_if_hash(&root, rel, None, b"x", "cw");
            let dt = t.elapsed();
            test_hooks::clear();
            let busy = matches!(r, Err(ProjectError::LockBusy { .. }));
            (r.map(|_| ()).map_err(|e| e.to_string()), busy, dt)
        })
    };
    // the stall begins at the hook's second call
    while rx.recv_timeout(Duration::from_secs(10)).expect("CW: no second look") < 2 {}
    std::thread::sleep(Duration::from_millis(100));
    std::fs::remove_file(&lock).unwrap();
    std::thread::sleep(Duration::from_millis(1800));
    std::fs::write(&lock, b"0 test 0").unwrap();
    // the second hold ends 100 ms after the waiter's third look, or after 3 s without one
    let t = Instant::now();
    while t.elapsed() < Duration::from_secs(3) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(n) if n >= 3 => break,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            _ => {}
        }
    }
    std::thread::sleep(Duration::from_millis(100));
    let _ = std::fs::remove_file(&lock);
    let (r, busy, dt) = writer.join().unwrap();
    let n = looks.load(Ordering::SeqCst);
    let claim_left = claim.exists();
    eprintln!(
        "CW: {r:?} (LockBusy {busy}) after {:.1} ms and {n} looks; claim left {claim_left}",
        dt.as_secs_f64() * 1e3
    );
    let mut bad = Vec::new();
    if r.is_err() {
        bad.push("the stalled waiter was refused");
    }
    if dt >= Duration::from_millis(3200) {
        bad.push("the edit took 3.2 s or more");
    }
    if n < 3 {
        bad.push("fewer than three looks");
    }
    if claim_left {
        bad.push("a claim file was left");
    }
    assert!(bad.is_empty(), "CW: {bad:?}");
}

/// CH: the racer of F2 is stalled for 2.4 s inside its 20th hold. One probe call is refused; it is class S, the rule of
/// ADR 0669 fails the run (the negative control) and the rule of ADR 0758 accepts it with count 1.
#[test]
fn ch_a_holder_stalled_for_the_whole_wait_is_class_s() {
    let root = project("f855-ch");
    let rec = f2::run(
        &root,
        300,
        || {
            let mut holds = 0u32;
            test_hooks::set(Hooks {
                after_lock: Some(Box::new(move |_: &Path| {
                    holds += 1;
                    if holds == 20 {
                        std::thread::sleep(Duration::from_millis(2400));
                    }
                })),
                ..Hooks::default()
            });
        },
        || {},
    );
    for l in f2::lines(&rec) {
        eprintln!("{l}");
    }
    let classes: String = rec.busy.iter().map(|b| f2::class(b, &rec.racer)).collect();
    let old = f2::verdict(&rec, f2::Rule::Adr0669);
    let new = f2::verdict(&rec, f2::Rule::Adr0758);
    eprintln!("CH: classes {classes:?}; verdict by ADR 0669 {old:?}; by ADR 0758 {new:?}");
    assert_eq!(classes, "S", "CH: the classes of the refused calls");
    assert_eq!(rec.conflicts, 299, "CH: conflicts");
    assert!(old.is_err(), "CH: the rule of ADR 0669 accepted the run");
    assert_eq!(new, Ok(1), "CH: the rule of ADR 0758");
}

/// CV: the probe of F2 makes no claim and, for 6 s, looks at the lock only while the racer is in the first 10 ms of a
/// hold of 30 ms (the forced schedule of ADR 0676 S1). It is starved while the lock changes hands; every refused call is
/// class V and the run fails.
#[test]
fn cv_a_starved_probe_is_class_v_and_fails() {
    const NOT_HOLDING: u64 = u64::MAX;
    let root = project("f855-cv");
    let epoch = Instant::now();
    let hold_since = Arc::new(AtomicU64::new(NOT_HOLDING));
    let published = hold_since.clone();
    let rec = f2::run(
        &root,
        200,
        move || {
            test_hooks::set(Hooks {
                after_lock: Some(Box::new(move |_: &Path| {
                    published.store(epoch.elapsed().as_micros() as u64, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(30));
                })),
                ..Hooks::default()
            });
        },
        move || {
            let mut calls = 0u32;
            let mut window: Option<Instant> = None;
            test_hooks::set(Hooks {
                claim_off: true,
                before_attempt: Some(Box::new(move |_: &Path| {
                    calls += 1;
                    if calls == 100 {
                        window = Some(Instant::now());
                    }
                    if !window.is_some_and(|w| w.elapsed() < Duration::from_secs(6)) {
                        return;
                    }
                    let t = Instant::now();
                    while t.elapsed() < Duration::from_millis(80) {
                        let hs = hold_since.load(Ordering::SeqCst);
                        let now = epoch.elapsed().as_micros() as u64;
                        if hs != NOT_HOLDING && now.saturating_sub(hs) < 10_000 {
                            return;
                        }
                        std::thread::sleep(Duration::from_micros(200));
                    }
                })),
                ..Hooks::default()
            });
        },
    );
    test_hooks::clear();
    for l in f2::lines(&rec) {
        eprintln!("{l}");
    }
    let classes: String = rec.busy.iter().map(|b| f2::class(b, &rec.racer)).collect();
    let new = f2::verdict(&rec, f2::Rule::Adr0758);
    eprintln!("CV: classes {classes:?}; verdict by ADR 0758 {new:?}");
    assert!(!classes.is_empty(), "CV: no probe call was refused");
    assert!(classes.chars().all(|c| c == 'V'), "CV: classes {classes:?}");
    assert!(new.is_err(), "CV: the rule of ADR 0758 accepted a starved probe");
}
