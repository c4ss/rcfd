//! ADR 0135 W48-15 (a): cooperative compare-and-replace under an edit lock (`atomic::replace_if_hash`).
//!
//! `two_writers_*` is the library-level stand-in for F6 that amendment 1 item 6 reports (not registered): F6 itself
//! names the Studio model and runs after 48B.

mod common;

use rcfd_project::atomic::{edit_lock_path, replace_if_hash, EDIT_LOCK_STALE, EDIT_LOG};
use rcfd_project::records::hash_bytes;
use rcfd_project::{Project, ProjectError};
use std::path::Path;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant, SystemTime};

fn log_lines(root: &Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(root.join(EDIT_LOG))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn create_replace_and_conflict() {
    let base = common::scratch("w48f-cas");
    let p = Project::create(&base.join("p"), "cas", "0").unwrap();
    let root = p.root();
    // create: base None, the file must not exist
    let r = replace_if_hash(root, "notes/a.md", None, b"one", "test").unwrap();
    assert_eq!((r.old_hash.clone(), r.new_hash.clone()), (None, hash_bytes(b"one")));
    assert_eq!(r.rel, "notes/a.md");
    match replace_if_hash(root, "notes/a.md", None, b"again", "test") {
        Err(ProjectError::Conflict { current, .. }) => assert_eq!(current, Some(hash_bytes(b"one"))),
        other => panic!("{other:?}"),
    }
    // replace with the right base, then a stale base is a conflict naming the current hash, file unchanged
    let h1 = hash_bytes(b"one");
    replace_if_hash(root, "./notes/a.md", Some(&h1), b"two", "test").unwrap();
    match replace_if_hash(root, "notes/a.md", Some(&h1), b"three", "test") {
        Err(ProjectError::Conflict { current, path }) => {
            assert_eq!(current, Some(hash_bytes(b"two")));
            assert!(path.ends_with("notes/a.md") || path.ends_with(r"notes\a.md"));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(std::fs::read(root.join("notes/a.md")).unwrap(), b"two");
    // the legacy spelling of the same hash is accepted
    let legacy = hash_bytes(b"two").replace("fnv64:", "fnv1a64:");
    replace_if_hash(root, "notes/a.md", Some(&legacy), b"four", "test").unwrap();
    // a base for a file that does not exist is a conflict with current None; subfolders are created
    match replace_if_hash(root, "notes/sub/b.md", Some(&h1), b"x", "test") {
        Err(ProjectError::Conflict { current: None, .. }) => {}
        other => panic!("{other:?}"),
    }
    replace_if_hash(root, "notes/sub/b.md", None, b"x", "test").unwrap();
    // escapes are refused by the path check
    assert!(matches!(
        replace_if_hash(root, "../x.md", None, b"x", "test"),
        Err(ProjectError::PathRefused { .. })
    ));
    // no lock is left behind; one log line per accepted edit
    assert_eq!(std::fs::read_dir(root.join(".rcfd/locks")).unwrap().count(), 0);
    let log = log_lines(root);
    assert_eq!(log.len(), 4, "{log:?}");
    assert_eq!(log[1]["kind"], "edit");
    assert_eq!(log[1]["path"], "notes/a.md");
    assert_eq!(log[1]["old_hash"], hash_bytes(b"one"));
    assert_eq!(log[1]["new_hash"], hash_bytes(b"two"));
    assert_eq!(log[1]["who"], "test");
}

#[test]
fn busy_lock_waits_then_gives_up_and_a_stale_lock_is_broken() {
    let base = common::scratch("w48f-lock");
    let p = Project::create(&base.join("p"), "lock", "0").unwrap();
    let root = p.root();
    let lock = edit_lock_path(root, "notes/a.md");
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    std::fs::write(&lock, b"held").unwrap();
    let t0 = Instant::now();
    match replace_if_hash(root, "notes/a.md", None, b"x", "test") {
        Err(ProjectError::LockBusy { lock: l, .. }) => assert_eq!(l, lock),
        other => panic!("{other:?}"),
    }
    let waited = t0.elapsed();
    assert!(
        waited >= Duration::from_secs(2) && waited < Duration::from_secs(6),
        "{waited:?}"
    );
    assert!(!root.join("notes/a.md").exists());
    // the same lock made older than the stale age is removed, with a log line
    let f = std::fs::OpenOptions::new().write(true).open(&lock).unwrap();
    f.set_modified(SystemTime::now() - EDIT_LOCK_STALE - Duration::from_secs(5))
        .unwrap();
    drop(f);
    let r = replace_if_hash(root, "notes/a.md", None, b"x", "test").unwrap();
    assert!(r.broke_stale_lock);
    let log = log_lines(root);
    assert_eq!(log[0]["kind"], "stale_lock_broken");
    assert!(log[0]["age_s"].as_f64().unwrap() > 30.0);
    assert_eq!(log[1]["kind"], "edit");
    assert!(!lock.exists());
}

/// Each writer appends one line `<who> <n>` to the shared file per accepted edit, re-reading on a conflict.
fn writer(root: &Path, who: &str, edits: usize, barrier: &Barrier, cooperative: bool) -> (usize, usize) {
    barrier.wait();
    let (mut accepted, mut conflicts) = (0, 0);
    let mut n = 0;
    while n < edits {
        let cur = std::fs::read(root.join("cases/c.txt")).unwrap();
        let mut next = cur.clone();
        next.extend_from_slice(format!("{who} {n}\n").as_bytes());
        if cooperative {
            match replace_if_hash(root, "cases/c.txt", Some(&hash_bytes(&cur)), &next, who) {
                Ok(_) => {
                    accepted += 1;
                    n += 1;
                }
                Err(ProjectError::Conflict { current, .. }) => {
                    assert!(current.is_some());
                    conflicts += 1;
                }
                Err(e) => panic!("{who}: {e}"),
            }
        } else {
            rcfd_project::atomic::atomic_write(&root.join("cases/c.txt"), &next).unwrap();
            accepted += 1;
            n += 1;
        }
        std::thread::yield_now();
    }
    (accepted, conflicts)
}

fn two_writers(cooperative: bool) -> (usize, usize, usize) {
    let base = common::scratch(if cooperative { "w48f-two" } else { "w48f-two-ctl" });
    let p = Project::create(&base.join("p"), "two", "0").unwrap();
    let root = p.root().to_path_buf();
    std::fs::write(root.join("cases/c.txt"), b"").unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let hs: Vec<_> = ["studio", "mcp"]
        .into_iter()
        .map(|who| {
            let (root, barrier) = (root.clone(), barrier.clone());
            std::thread::spawn(move || writer(&root, who, 200, &barrier, cooperative))
        })
        .collect();
    let res: Vec<(usize, usize)> = hs.into_iter().map(|h| h.join().unwrap()).collect();
    let lines = std::fs::read_to_string(root.join("cases/c.txt"))
        .unwrap()
        .lines()
        .count();
    let accepted = res.iter().map(|r| r.0).sum();
    let conflicts = res.iter().map(|r| r.1).sum();
    if cooperative {
        // every accepted edit's base equals the previous accepted result: the log chains old_hash -> new_hash
        let log = log_lines(&root);
        assert_eq!(log.len(), accepted);
        for w in log.windows(2) {
            assert_eq!(w[1]["old_hash"], w[0]["new_hash"], "{w:?}");
        }
    }
    (accepted, conflicts, lines)
}

#[test]
fn two_writers_with_base_hashes_lose_no_update() {
    let (accepted, conflicts, lines) = two_writers(true);
    eprintln!(
        "F6 stand-in (library): accepted {accepted}, conflicts {conflicts}, lines {lines}, lost {}",
        accepted - lines
    );
    assert_eq!(accepted, 400);
    assert_eq!(lines, 400, "lost updates");
}

#[test]
fn two_writers_without_lock_and_hash_lose_updates_control() {
    let (accepted, _, lines) = two_writers(false);
    eprintln!(
        "F6 stand-in control (no lock, no hash): accepted {accepted}, lines {lines}, lost {}",
        accepted - lines
    );
    assert_eq!(accepted, 400);
    // reported control: expected to lose at least one update; not a registered limit (amendment 1 item 6)
    if lines == accepted {
        eprintln!("F6 stand-in control: no update lost in this run (the interleaving did not race)");
    }
}
