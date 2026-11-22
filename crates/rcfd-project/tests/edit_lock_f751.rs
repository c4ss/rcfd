//! ADR 0669 (F-751): the edit lock's claim (D2) and the resolve under the lock (D1), as registered (F1, F2, F3a, F3b).
//! ADR 0758 (F-855): F2 as amended there (a refused probe call is accepted only as class S), its old body as an ignored
//! record, and FS, the classes and the verdict over made-up records.

mod common;
#[path = "edit_lock_f2/mod.rs"]
mod f2;

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

/// F1c (ADR 0669 amendment 1): the holder looks for a claim while it holds the lock and, without one, re-takes the lock
/// within one remove and one create; hold 100 ms.
#[test]
fn f1c_a_waiter_is_not_starved_by_a_holder_that_re_takes_the_lock_within_one_create() {
    let base = common::scratch("f751-f1c");
    let p = Project::create(&base.join("p"), "f1c", "0").unwrap();
    let root = p.root().to_path_buf();
    let mut times = Vec::new();
    let mut bad = Vec::new();
    for k in 0..10 {
        let rel = format!("cases/t{k}.txt");
        let lock = edit_lock_path(&root, &rel);
        let claim = edit_claim_path(&root, &rel);
        std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let create = |l: &std::path::Path| std::fs::OpenOptions::new().write(true).create_new(true).open(l).is_ok();
        let holder = {
            let (lock, claim, stop) = (lock.clone(), claim.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut cycles = 0usize;
                while !create(&lock) {}
                while !stop.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(100));
                    cycles += 1;
                    if claim.exists() || stop.load(Ordering::SeqCst) {
                        while std::fs::remove_file(&lock).is_err() && lock.exists() {}
                        while claim.exists() && !stop.load(Ordering::SeqCst) {
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        if stop.load(Ordering::SeqCst) {
                            return cycles;
                        }
                        while !create(&lock) {}
                    } else {
                        while std::fs::remove_file(&lock).is_err() && lock.exists() {}
                        while !create(&lock) {}
                    }
                }
                while std::fs::remove_file(&lock).is_err() && lock.exists() {}
                cycles
            })
        };
        std::thread::sleep(Duration::from_millis(150));
        let t0 = Instant::now();
        let r = replace_if_hash(&root, &rel, None, b"x", "f1c");
        let dt = t0.elapsed();
        stop.store(true, Ordering::SeqCst);
        let cycles = holder.join().unwrap();
        eprintln!(
            "F1c trial {k}: {:?} after {:.1} ms, holder cycles {cycles}",
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
        "F1c: {} of 10 accepted in under 1.0 s, max {:.1} ms; claim files left {left}",
        10 - bad.len(),
        times.iter().max().unwrap().as_secs_f64() * 1e3
    );
    assert!(bad.is_empty(), "F1c: {bad:?}");
    assert_eq!(left, 0, "F1c: a claim file was left");
}

/// F2: a cooperating racer replaces the file continuously; every call of a probe with a wrong base is a conflict, or a
/// `LockBusy` of class S (ADR 0758: one accepted hold of the racer lasted the whole wait), at most `f2::STALL_LIMIT` times.
#[test]
fn f2_a_cooperating_writers_renames_never_refuse_or_fail_another_writers_path() {
    let base = common::scratch("f751-f2");
    let p = Project::create(&base.join("p"), "f2", "0").unwrap();
    let rec = f2::run(p.root(), 10000, || {}, || {});
    let lines = f2::lines(&rec);
    for l in &lines {
        eprintln!("{l}");
    }
    let v = f2::verdict(&rec, f2::Rule::Adr0758);
    assert!(v.is_ok(), "F2: {:?}\n{}", v.err(), lines.join("\n"));
}

/// FS (ADR 0758): the classes of a refused probe call and the verdict, over records written by hand.
#[test]
fn fs_the_classes_and_the_verdict_over_made_up_records() {
    use f2::{class, verdict, Busy, Record, Rule};
    // a probe call refused after a wait from 10.0 s to 12.0 s, the racer's lock 2.0 s old, the racer inside one accepted call
    let busy = Busy {
        call: 7,
        p0: 10.0,
        p1: 12.0,
        claim: false,
        lock_who: Some("racer".to_string()),
        lock_age_s: Some(2.0),
        journal_lock_age_s: None,
        journal_lock_before_lock: false,
    };
    let hold = vec![(9.0, 9.99, true), (9.995, 12.3, true), (12.31, 12.32, true)];
    let rec = |busy: Vec<Busy>, racer: &[f2::RacerCall]| Record {
        calls: 10,
        conflicts: 10 - busy.len(),
        busy,
        racer: racer.to_vec(),
        ..Record::default()
    };
    let one =
        |b: &Busy, racer: &[f2::RacerCall]| (class(b, racer), verdict(&rec(vec![b.clone()], racer), Rule::Adr0758));

    assert_eq!(one(&busy, &hold), ('S', Ok(1)), "a hold over the whole wait");
    assert!(
        verdict(&rec(vec![busy.clone()], &hold), Rule::Adr0669).is_err(),
        "ADR 0669 accepts no LockBusy"
    );
    assert_eq!(
        verdict(&rec(vec![busy.clone(); 4], &hold), Rule::Adr0758),
        Ok(4),
        "four of class S"
    );
    assert!(
        verdict(&rec(vec![busy.clone(); 5], &hold), Rule::Adr0758).is_err(),
        "five of class S"
    );
    for (what, journal) in [
        (
            "no seq",
            f2::Journal {
                no_seq: 1,
                ..Default::default()
            },
        ),
        (
            "broken",
            f2::Journal {
                broken: 1,
                ..Default::default()
            },
        ),
        (
            "remove failed",
            f2::Journal {
                remove_failed: 1,
                ..Default::default()
            },
        ),
        (
            "stale broken",
            f2::Journal {
                stale_broken: 1,
                ..Default::default()
            },
        ),
    ] {
        let mut r = rec(vec![busy.clone()], &hold);
        r.journal = journal;
        assert!(
            verdict(&r, Rule::Adr0758).is_err(),
            "class S with a journal that is not clean: {what}"
        );
    }

    let late = [(9.0, 10.14, true), (10.15, 12.3, true)];
    let (c, v) = one(
        &Busy {
            lock_age_s: Some(1.85),
            ..busy.clone()
        },
        &late,
    );
    assert!(c == 'V' && v.is_err(), "a hold that begins 150 ms into the wait: {c}");
    let (c, v) = one(&busy, &late);
    assert!(c == 'V' && v.is_err(), "a racer call that ends inside the wait: {c}");
    let (c, v) = one(&busy, &[(9.995, 12.3, false)]);
    assert!(c == 'V' && v.is_err(), "a racer call that failed: {c}");
    let (c, v) = one(
        &Busy {
            lock_who: Some("probe".to_string()),
            ..busy.clone()
        },
        &hold,
    );
    assert!(c == 'V' && v.is_err(), "the lock of another writer: {c}");
    let (c, v) = one(
        &Busy {
            lock_who: None,
            lock_age_s: None,
            ..busy.clone()
        },
        &hold,
    );
    assert!(c == 'V' && v.is_err(), "no lock file: {c}");
    let (c, v) = one(
        &Busy {
            lock_age_s: Some(0.4),
            ..busy.clone()
        },
        &hold,
    );
    assert!(c == 'V' && v.is_err(), "a young lock file: {c}");
    let (c, v) = one(
        &Busy {
            claim: true,
            ..busy.clone()
        },
        &hold,
    );
    assert!(c == 'C' && v.is_err(), "a claim present: {c}");
    let (c, v) = one(
        &Busy {
            p1: 10.5,
            lock_age_s: Some(0.5),
            ..busy.clone()
        },
        &[(9.995, 12.3, true)],
    );
    assert!(c == 'E' && v.is_err(), "refused before the wait was over: {c}");
    let (c, v) = one(
        &Busy {
            journal_lock_age_s: Some(4.0),
            journal_lock_before_lock: true,
            ..busy.clone()
        },
        &hold,
    );
    assert!(c == 'J' && v.is_err(), "a journal lock older than the edit lock: {c}");
    let (c, v) = one(
        &Busy {
            lock_who: None,
            lock_age_s: None,
            journal_lock_age_s: Some(1.5),
            ..busy.clone()
        },
        &hold,
    );
    assert!(c == 'J' && v.is_err(), "an old journal lock and no edit lock: {c}");
    let (c, v) = one(
        &Busy {
            journal_lock_age_s: Some(0.3),
            ..busy.clone()
        },
        &hold,
    );
    assert!(
        c == 'S' && v == Ok(1),
        "the racer's own journal lock, made in this hold: {c}"
    );

    let mut r = rec(vec![], &hold);
    assert_eq!(verdict(&r, Rule::Adr0758), Ok(0), "ten conflicts");
    assert_eq!(verdict(&r, Rule::Adr0669), Ok(0), "ten conflicts, ADR 0669");
    r.conflicts = 9;
    r.other = vec![(3, "Err(PathRefused)".to_string())];
    assert!(
        verdict(&r, Rule::Adr0758).is_err(),
        "a result that is neither Conflict nor LockBusy"
    );
    let mut r = rec(vec![], &hold);
    r.racer_errs = vec!["x".to_string()];
    assert!(verdict(&r, Rule::Adr0758).is_err(), "a racer error");
}

/// The body of F2 as ADR 0669 registered it (every probe call a conflict), kept as a record by ADR 0758; only the name of
/// its scratch folder differs. Its limit was missed twice in the full gate: 9996 of 10000 at e9378ecb (ADR 0751) and
/// 9999 of 10000 at f6e68b4a (ADR 0758), each other call a `LockBusy` after the whole wait, the racer without error.
#[test]
#[ignore = "record (ADR 0758): F2 as ADR 0669 registered it, 9996 and 9999 of 10000 in two gates; the amended F2 is the check"]
fn f2_adr_0669_body_known_miss() {
    let base = common::scratch("f751-f2-record");
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
