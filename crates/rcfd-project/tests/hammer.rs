//! ADR 0122 A1 (atomic replace under concurrent readers, threads) and A3 (Windows retry on a
//! destination held open without delete sharing).

mod common;

use rcfd_project::atomic;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Registered: duration 2 s, payload 1 MiB, 1 writer, 3 readers.
const DURATION: Duration = Duration::from_secs(2);
const LEN: usize = 1 << 20;
const READERS: usize = 3;

#[derive(Debug, Default)]
struct Counts {
    writes: u64,
    reads: u64,
    partial: u64,
    open_errors: u64,
}

fn hammer(tag: &str, naive: bool) -> Counts {
    let dir = common::scratch(tag);
    let path = dir.join("status.json");
    atomic::atomic_write(&path, &common::payload(0, LEN)).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let (reads, partial, errs) = (
        Arc::new(AtomicU64::new(0)),
        Arc::new(AtomicU64::new(0)),
        Arc::new(AtomicU64::new(0)),
    );
    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let (path, stop, reads, partial, errs) =
                (path.clone(), stop.clone(), reads.clone(), partial.clone(), errs.clone());
            std::thread::spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    match std::fs::read(&path) {
                        Ok(b) => {
                            reads.fetch_add(1, Ordering::Relaxed);
                            if !common::payload_ok(&b) {
                                partial.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                        Err(_) => {
                            errs.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
        })
        .collect();
    let t = Instant::now();
    let mut writes = 0u64;
    while t.elapsed() < DURATION {
        writes += 1;
        let bytes = common::payload(writes, LEN);
        if naive {
            // In-place truncate and write: what atomic replacement avoids. Errors (Windows sharing) are skipped.
            let _ = std::fs::write(&path, &bytes);
        } else {
            atomic::atomic_write(&path, &bytes).unwrap();
        }
    }
    stop.store(true, Ordering::Relaxed);
    readers.into_iter().for_each(|h| h.join().unwrap());
    let c = Counts {
        writes,
        reads: reads.load(Ordering::Relaxed),
        partial: partial.load(Ordering::Relaxed),
        open_errors: errs.load(Ordering::Relaxed),
    };
    eprintln!(
        "A1 {tag}: {c:?}; reader open errors (not partial reads) {}",
        c.open_errors
    );
    if !naive {
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "temp files left: {names:?}");
    }
    c
}

#[test]
fn a1_atomic_replace_never_exposes_a_partial_file() {
    let c = hammer("a1", false);
    assert_eq!(c.partial, 0);
    // The check is only meaningful if the readers and the writer really overlapped.
    assert!(c.writes >= 20 && c.reads >= 200, "{c:?}");
}

#[test]
fn a1_negative_control_in_place_write_exposes_partial_files() {
    let c = hammer("a1neg", true);
    assert!(c.partial >= 1, "{c:?}");
}

#[cfg(windows)]
#[test]
fn a3_replace_retries_while_destination_is_held_open_and_then_fails_located() {
    use rcfd_project::ProjectError;
    use std::os::windows::fs::OpenOptionsExt;
    let dir = common::scratch("a3");
    let path = dir.join("status.json");
    std::fs::write(&path, b"old").unwrap();
    let hold = || {
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1 /* FILE_SHARE_READ, no delete */)
            .open(&path)
            .unwrap()
    };

    // Held for 50 ms: the default policy retries through it.
    let h = hold();
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        drop(h);
    });
    let t = Instant::now();
    atomic::atomic_write(&path, b"new").unwrap();
    let waited = t.elapsed();
    release.join().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    eprintln!("A3: replaced after {waited:?}");
    assert!(
        waited >= Duration::from_millis(40),
        "the hold was not effective: {waited:?}"
    );

    // Held throughout: bounded retries, then a located error; no temp file is left.
    let h = hold();
    let t = Instant::now();
    let e = atomic::atomic_write(&path, b"newer").unwrap_err();
    let spent = t.elapsed();
    drop(h);
    eprintln!("A3: gave up after {spent:?}: {e}");
    match &e {
        ProjectError::Retry {
            path: p,
            attempts,
            source,
            ..
        } => {
            assert_eq!((p, *attempts), (&path, atomic::DEFAULT_RETRY.attempts));
            eprintln!("A3: os error {:?}", source.raw_os_error());
        }
        other => panic!("{other:?}"),
    }
    assert!(spent < Duration::from_secs(2), "{spent:?}");
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);

    // Negative control: without retries the 50 ms hold is fatal.
    let h = hold();
    let one = atomic::Retry {
        attempts: 1,
        ..atomic::DEFAULT_RETRY
    };
    assert!(matches!(
        atomic::atomic_write_with(&path, b"x", &one),
        Err(ProjectError::Retry { attempts: 1, .. })
    ));
    drop(h);
}
