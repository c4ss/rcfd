//! ADR 0669 (F-751): the edit lock's claim (D2) and the resolve under the lock (D1), as registered (F1, F2, F3a, F3b).

mod common;

use rcfd_project::atomic::{edit_claim_path, edit_lock_path, replace_if_hash, EDIT_CLAIM_STALE};
use rcfd_project::records::hash_bytes;
use rcfd_project::{Project, ProjectError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

fn claims_left(root: &std::path::Path) -> usize {
    std::fs::read_dir(root.join(".rcfd/locks"))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "claim"))
                .count()
        })
        .unwrap_or(0)
}

/// F1: a cooperating holder that obeys the claim, holds 20 ms and takes the lock again microseconds after it lets go.
#[test]
fn f1_a_waiter_is_not_starved_by_a_holder_that_takes_the_lock_again_at_once() {
    let base = common::scratch("f751-f1");
    let p = Project::create(&base.join("p"), "f1", "0").unwrap();
    let root = p.root().to_path_buf();
    let mut times = Vec::new();
    let mut bad = Vec::new();
    for k in 0..10 {
        let rel = format!("cases/t{k}.txt");
        let lock = edit_lock_path(&root, &rel);
        let claim = edit_claim_path(&root, &rel);
        std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let holder = {
            let (lock, claim, stop) = (lock.clone(), claim.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut cycles = 0usize;
                while !stop.load(Ordering::SeqCst) {
                    if claim.exists() {
                        std::thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    if std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&lock)
                        .is_err()
                    {
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(20));
                    // a busy remove (Windows, a waiter's handle open) is retried
                    while std::fs::remove_file(&lock).is_err() && lock.exists() {}
                    cycles += 1;
                }
                cycles
            })
        };
        std::thread::sleep(Duration::from_millis(100));
        let t0 = Instant::now();
        let r = replace_if_hash(&root, &rel, None, b"x", "f1");
        let dt = t0.elapsed();
        stop.store(true, Ordering::SeqCst);
        let cycles = holder.join().unwrap();
        eprintln!(
            "F1 trial {k}: {:?} after {:.1} ms, holder cycles {cycles}",
            r.as_ref().map(|_| "accepted"),
            dt.as_secs_f64() * 1e3
        );
        if r.is_err() || dt >= Duration::from_secs(1) {
            bad.push((k, format!("{r:?}"), dt));
        }
        times.push(dt);
    }
    let left = claims_left(&root);
    eprintln!(
        "F1: {} of 10 accepted in under 1.0 s, max {:.1} ms; claim files left {left}",
        10 - bad.len(),
        times.iter().max().unwrap().as_secs_f64() * 1e3
    );
    assert!(bad.is_empty(), "F1: {bad:?}");
    assert_eq!(left, 0, "F1: a claim file was left");
}

/// F2: a cooperating racer replaces the file continuously; every call of a probe with a wrong base is a conflict.
#[test]
fn f2_a_cooperating_writers_renames_never_refuse_or_fail_another_writers_path() {
    let base = common::scratch("f751-f2");
    let p = Project::create(&base.join("p"), "f2", "0").unwrap();
    let root = p.root().to_path_buf();
    std::fs::write(root.join("cases/c.txt"), b"").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let racer = {
        let (root, stop) = (root.clone(), stop.clone());
        std::thread::spawn(move || {
            let (mut ok, mut errs) = (0usize, Vec::new());
            while !stop.load(Ordering::SeqCst) {
                let cur = std::fs::read(root.join("cases/c.txt")).unwrap();
                let mut next = cur.clone();
                next.extend_from_slice(format!("r {ok}\n").as_bytes());
                match replace_if_hash(&root, "cases/c.txt", Some(&hash_bytes(&cur)), &next, "racer") {
                    Ok(_) => ok += 1,
                    Err(e) => errs.push(e.to_string()),
                }
            }
            (ok, errs)
        })
    };
    let mut other = Vec::new();
    let mut conflicts = 0usize;
    for _ in 0..10000 {
        match replace_if_hash(&root, "cases/c.txt", Some("fnv64:0000000000000000"), b"p", "probe") {
            Err(ProjectError::Conflict { .. }) => conflicts += 1,
            r => other.push(format!("{r:?}")),
        }
    }
    stop.store(true, Ordering::SeqCst);
    let (ok, errs) = racer.join().unwrap();
    eprintln!(
        "F2: probe {conflicts} of 10000 conflicts, {} other (first {:?}); racer {ok} accepted, {} errors (first {:?})",
        other.len(),
        other.first(),
        errs.len(),
        errs.first()
    );
    assert_eq!(conflicts, 10000, "F2: {other:?}");
    assert!(errs.is_empty(), "F2 racer: {errs:?}");
}

fn claim_set_up(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let base = common::scratch(name);
    let p = Project::create(&base.join("p"), "f3", "0").unwrap();
    let root = p.root().to_path_buf();
    let claim = edit_claim_path(&root, "notes/a.md");
    std::fs::create_dir_all(claim.parent().unwrap()).unwrap();
    std::fs::write(&claim, b"1 other 0").unwrap();
    (root, claim)
}

/// F3a: a fresh claim of another writer is a busy lock.
#[test]
fn f3a_a_fresh_claim_of_another_writer_is_waited_for_then_busy() {
    let (root, claim) = claim_set_up("f751-f3a");
    let t0 = Instant::now();
    let r = replace_if_hash(&root, "notes/a.md", None, b"x", "f3a");
    let waited = t0.elapsed();
    eprintln!("F3a: {r:?} after {waited:?}");
    assert!(matches!(r, Err(ProjectError::LockBusy { .. })), "{r:?}");
    assert!(
        waited >= Duration::from_secs(2) && waited < Duration::from_secs(30),
        "{waited:?}"
    );
    assert!(!root.join("notes/a.md").exists());
    assert!(claim.exists(), "F3a: the other writer's claim was removed");
}

/// F3b: a claim older than EDIT_CLAIM_STALE is removed and the edit goes through.
#[test]
fn f3b_a_stale_claim_is_removed() {
    let (root, claim) = claim_set_up("f751-f3b");
    let f = std::fs::OpenOptions::new().write(true).open(&claim).unwrap();
    f.set_modified(SystemTime::now() - EDIT_CLAIM_STALE - Duration::from_secs(5))
        .unwrap();
    drop(f);
    let r = replace_if_hash(&root, "notes/a.md", None, b"x", "f3b");
    eprintln!("F3b: {r:?}");
    assert!(r.is_ok(), "{r:?}");
    assert!(!claim.exists(), "F3b: the stale claim is still there");
    assert_eq!(std::fs::read_dir(root.join(".rcfd/locks")).unwrap().count(), 0);
}
