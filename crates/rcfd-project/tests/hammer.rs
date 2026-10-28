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
/// Registered overlap minimums: the check is only meaningful if the readers and the writer really overlapped.
const MIN_WRITES: u64 = 20;
const MIN_READS: u64 = 200;
/// ADR 0122 amendment 3: after the 2 s the hammer continues until both minimums are reached, but writes no longer than this
/// (a slow disk makes each fsync of the atomic writer take about 130 ms).
const CAP: Duration = Duration::from_secs(30);

/// How long the writer runs.
#[derive(Clone, Copy)]
enum Until {
    /// The registered set-up as first written: exactly [`DURATION`].
    Fixed,
    /// Amendment 3: at least [`DURATION`], then until [`MIN_WRITES`] and [`MIN_READS`] are reached, at most [`CAP`].
    Minimums,
}

#[derive(Debug, Default)]
struct Counts {
    writes: u64,
    reads: u64,
    partial: u64,
    open_errors: u64,
}

fn hammer(tag: &str, naive: bool, until: Until) -> Counts {
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
    let more = |writes: u64, elapsed: Duration| match until {
        Until::Fixed => elapsed < DURATION,
        Until::Minimums => {
            elapsed < DURATION || (elapsed < CAP && (writes < MIN_WRITES || reads.load(Ordering::Relaxed) < MIN_READS))
        }
    };
    while more(writes, t.elapsed()) {
        writes += 1;
        let bytes = common::payload(writes, LEN);
        if naive {
            // In-place truncate and write: what atomic replacement avoids. Errors (Windows sharing) are skipped.
            let _ = std::fs::write(&path, &bytes);
        } else {
            atomic::atomic_write(&path, &bytes).unwrap();
        }
    }
    let elapsed = t.elapsed();
    stop.store(true, Ordering::Relaxed);
    readers.into_iter().for_each(|h| h.join().unwrap());
    let c = Counts {
        writes,
        reads: reads.load(Ordering::Relaxed),
        partial: partial.load(Ordering::Relaxed),
        open_errors: errs.load(Ordering::Relaxed),
    };
    eprintln!(
        "A1 {tag}: {c:?} in {:.2} s; reader open errors (not partial reads) {}",
        elapsed.as_secs_f64(),
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

fn a1_check(c: &Counts) {
    assert_eq!(c.partial, 0, "{c:?}");
    // The check is only meaningful if the readers and the writer really overlapped.
    assert!(c.writes >= MIN_WRITES && c.reads >= MIN_READS, "{c:?}");
}

/// ADR 0122 amendment 3: at least 2 s, then until the overlap minimums are reached (at most 30 s).
#[test]
fn a1_atomic_replace_never_exposes_a_partial_file() {
    a1_check(&hammer("a1", false, Until::Minimums));
}

/// The registered A1 as first written (a fixed 2 s): on a slow disk the fsyncing writer does not reach 20 writes (ADR 0122
/// amendment 3, defective set-up); kept with the same limits.
#[test]
#[ignore = "defective set-up (ADR 0122 amendment 3): a fixed 2 s holds about 15 fsynced writes on a slow disk"]
fn a1_atomic_replace_never_exposes_a_partial_file_known_miss() {
    a1_check(&hammer("a1km", false, Until::Fixed));
}

#[test]
fn a1_negative_control_in_place_write_exposes_partial_files() {
    let c = hammer("a1neg", true, Until::Minimums);
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
