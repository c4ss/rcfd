//! ADR 0188 C3-S and C3-O, the append side: every journal line written under the journal's lock has a `seq`, one more
//! than the last, with no number repeated or missed by two writers; a held lock makes a writer append without `seq`
//! after the wait; a stale lock is removed; lines written before ADR 0188 (no `seq`), a line that is not JSON and a torn
//! line do not stop the numbering, and the next line stands on a line of its own.

mod common;

use rcfd_project::atomic::{replace_if_hash, EDIT_LOG};
use rcfd_project::journal::{append, LOCK, LOCK_STALE, LOCK_WAIT};
use rcfd_project::records::hash_bytes;
use rcfd_project::{Project, ProjectError};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant, SystemTime};

fn lines(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join(EDIT_LOG))
        .unwrap_or_default()
        .lines()
        .map(String::from)
        .collect()
}

fn parsed(root: &Path) -> Vec<Value> {
    lines(root)
        .iter()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

#[test]
fn c3s_two_writers_number_every_line_once_in_file_order() {
    let base = common::scratch("f273-seq");
    let p = Project::create(&base.join("p"), "seq", "0").unwrap();
    let root = p.root().to_path_buf();
    std::fs::write(root.join("cases/c.txt"), b"").unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let hs: Vec<_> = ["studio", "mcp"]
        .into_iter()
        .map(|who| {
            let (root, barrier) = (root.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                let mut n = 0;
                while n < 200 {
                    let cur = std::fs::read(root.join("cases/c.txt")).unwrap();
                    let mut next = cur.clone();
                    next.extend_from_slice(format!("{who} {n}\n").as_bytes());
                    match replace_if_hash(&root, "cases/c.txt", Some(&hash_bytes(&cur)), &next, who) {
                        Ok(_) => n += 1,
                        Err(ProjectError::Conflict { .. }) => {}
                        Err(e) => panic!("{who}: {e}"),
                    }
                }
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    let log = parsed(&root);
    let seqs: Vec<u64> = log
        .iter()
        .map(|l| l["seq"].as_u64().expect("every line numbered"))
        .collect();
    eprintln!(
        "C3-S: {} lines, seq {}..{}, first {:?}",
        log.len(),
        seqs.first().unwrap(),
        seqs.last().unwrap(),
        &seqs[..5]
    );
    assert_eq!(log.len(), 400);
    assert_eq!(seqs, (1..=400).collect::<Vec<u64>>(), "seq not 1..=400 in file order");
    for w in log.windows(2) {
        assert_eq!(w[1]["old_hash"], w[0]["new_hash"], "{w:?}");
    }
}

/// Added to C3-S (ADR 0188 addendum 1): two writers of different files take different edit locks, so only the journal's
/// lock keeps their numbers apart. Two threads append 300 lines each directly.
#[test]
fn c3s_two_appenders_without_a_shared_edit_lock_number_every_line_once() {
    let base = common::scratch("f273-seq2");
    let p = Project::create(&base.join("p"), "seq2", "0").unwrap();
    let root = p.root().to_path_buf();
    let barrier = Arc::new(Barrier::new(2));
    let hs: Vec<_> = ["a", "b"]
        .into_iter()
        .map(|who| {
            let (root, barrier) = (root.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                (0..300)
                    .filter(|i| append(&root, json!({"kind": "test", "who": who, "i": i})).is_none())
                    .count()
            })
        })
        .collect();
    let unnumbered: usize = hs.into_iter().map(|h| h.join().unwrap()).sum();
    let log = parsed(&root);
    let mut seqs: Vec<u64> = log.iter().filter_map(|l| l["seq"].as_u64()).collect();
    let in_order = seqs.windows(2).all(|w| w[0] < w[1]);
    seqs.sort_unstable();
    seqs.dedup();
    eprintln!(
        "C3-S two appenders: {} lines, {} distinct seq, {unnumbered} without seq (lock not had in {LOCK_WAIT:?}), in file \
         order: {in_order}",
        log.len(),
        seqs.len()
    );
    assert_eq!(log.len(), 600);
    assert_eq!(unnumbered, 0, "a writer waited longer than the lock's wait");
    assert_eq!(seqs, (1..=600).collect::<Vec<u64>>(), "a seq repeated or missing");
    assert!(in_order, "seq not increasing in file order");
}

#[test]
fn c3s_a_held_lock_is_waited_for_then_passed_and_a_stale_one_is_removed() {
    let base = common::scratch("f273-lock");
    let p = Project::create(&base.join("p"), "lock", "0").unwrap();
    let root = p.root();
    assert_eq!(append(root, json!({"kind": "test", "n": 1})), Some(1));
    let lock = root.join(LOCK);
    std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
    std::fs::write(&lock, b"held").unwrap();
    let t0 = Instant::now();
    let r = append(root, json!({"kind": "test", "n": 2}));
    let waited = t0.elapsed();
    eprintln!("C3-S held lock: waited {waited:?}, seq {r:?}");
    assert_eq!(r, None);
    assert!(waited >= LOCK_WAIT && waited < Duration::from_secs(5), "{waited:?}");
    let l = parsed(root);
    assert_eq!(l.len(), 2);
    assert!(l[1].get("seq").is_none(), "{}", l[1]);
    assert!(lock.exists(), "the held lock was removed");
    // the same lock made older than the stale age is removed; the line says so
    let f = std::fs::OpenOptions::new().write(true).open(&lock).unwrap();
    f.set_modified(SystemTime::now() - LOCK_STALE - Duration::from_secs(5))
        .unwrap();
    drop(f);
    assert_eq!(append(root, json!({"kind": "test", "n": 3})), Some(2));
    let l = parsed(root);
    assert_eq!(l[2]["seq"], 2);
    assert!(l[2]["journal_lock_broken_s"].as_f64().unwrap() > LOCK_STALE.as_secs_f64());
    assert!(!lock.exists());
}

#[test]
fn c3o_old_lines_a_bad_line_and_a_torn_line_do_not_stop_the_numbering() {
    let base = common::scratch("f273-old");
    let p = Project::create(&base.join("p"), "old", "0").unwrap();
    let root = p.root();
    // byte for byte as main 5fdde2d writes them (rcfd_project::atomic::log_edit and acquire)
    let old = concat!(
        r#"{"kind":"edit","new_hash":"fnv64:00000000000000b2","old_hash":"fnv64:00000000000000a1","path":"cases/a.toml","t":"2026-11-05T10:00:00.000Z","who":"edit_case"}"#,
        "\n",
        r#"{"age_s":31.5,"kind":"stale_lock_broken","path":"cases/a.toml","t":"2026-11-05T10:00:01.000Z","who":"save_case"}"#,
        "\n",
        "this is not json\n",
        r#"{"kind":"edit","who":"torn"#
    );
    std::fs::create_dir_all(root.join(".rcfd")).unwrap();
    std::fs::write(root.join(EDIT_LOG), old).unwrap();
    assert_eq!(append(root, json!({"kind": "test"})), Some(1));
    let all = lines(root);
    eprintln!("C3-O: {} lines after the append: {:?}", all.len(), all.last());
    assert_eq!(all.len(), 5, "{all:?}");
    assert_eq!(all[3], r#"{"kind":"edit","who":"torn"#, "the torn line was changed");
    let last: Value = serde_json::from_str(&all[4]).unwrap();
    assert_eq!((last["kind"].clone(), last["seq"].clone()), (json!("test"), json!(1)));
    assert_eq!(append(root, json!({"kind": "test"})), Some(2));
}
