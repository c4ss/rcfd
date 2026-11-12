//! The project journal's append (`.rcfd/edits.jsonl`, [`crate::atomic::EDIT_LOG`]; ADR 0173 W6, ADR 0188 J2).
//!
//! Every line is one JSON object appended in a single write. [`append`] numbers it: under a short project-wide lock,
//! [`LOCK`], it reads the last `seq` in the file and writes the line with `seq` one more (1 for the first numbered line).
//! Lines written before ADR 0188, and lines appended without the lock, have no `seq`. The journal is best effort, as it
//! was before: a failure to append never undoes the write the line records.
//!
//! Limits: a writer waits at most [`LOCK_WAIT`] for the lock and then appends its line without `seq`; a lock older than
//! [`LOCK_STALE`] is taken to be left by a crashed writer and removed (the line then carries `journal_lock_broken_s`). A
//! writer may hold a file's edit lock while it waits for this lock, never the reverse. Only writers that use this
//! function take the lock; a line appended by an older build at the same moment can share a number with one of ours.

use crate::atomic::{is_transient, EDIT_LOG};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Project-relative lock of the journal's append.
pub const LOCK: &str = ".rcfd/locks/journal.lock";
/// Longest wait for [`LOCK`] before a line is appended without `seq`. The lock is not fair: a writer that appends in a
/// loop can retake it before a waiting one wakes (on Windows a 1 ms sleep can last a whole timer tick), so 250 ms let
/// two busy writers leave lines unnumbered; 2 s, the wait of the edit lock, is the bound now (ADR 0188 addendum 2).
pub const LOCK_WAIT: Duration = Duration::from_secs(2);
/// A [`LOCK`] older than this is removed.
pub const LOCK_STALE: Duration = Duration::from_secs(5);
/// How much of the journal's end is read to find the last `seq` (the whole file only when this holds none).
pub const TAIL: u64 = 64 * 1024;

struct Held(PathBuf);

impl Drop for Held {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Take [`LOCK`]: the guard and, when a stale lock was removed first, its age in seconds; `None` after [`LOCK_WAIT`].
fn lock(root: &Path) -> Option<(Held, Option<f64>)> {
    let path = root.join(LOCK);
    let t0 = Instant::now();
    let mut broke = None;
    loop {
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                let _ = writeln!(f, "{} {}", std::process::id(), crate::time::now());
                return Some((Held(path), broke));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                fs::create_dir_all(path.parent()?).ok()?;
                if t0.elapsed() >= LOCK_WAIT {
                    return None;
                }
            }
            // AlreadyExists, or a lock being deleted right now (Windows reports that as access denied)
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists || is_transient(&e) => {
                let age = fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| m.elapsed().ok());
                if age.is_some_and(|a| a > LOCK_STALE) && fs::remove_file(&path).is_ok() {
                    broke = age.map(|a| a.as_secs_f64());
                    continue;
                }
                if t0.elapsed() >= LOCK_WAIT {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            Err(_) => return None,
        }
    }
}

/// The `seq` of the last complete line of `bytes` that has one (searching from the end).
fn last_seq(bytes: &[u8]) -> Option<u64> {
    let end = bytes.iter().rposition(|&b| b == b'\n')?;
    bytes[..end]
        .split(|&b| b == b'\n')
        .rev()
        .filter_map(|l| serde_json::from_slice::<Value>(l.strip_suffix(b"\r").unwrap_or(l)).ok())
        .find_map(|v| v.get("seq").and_then(Value::as_u64))
}

/// The next `seq` of the journal at `path` and whether its last byte is not a newline (a torn line).
fn next_seq(path: &Path) -> io::Result<(u64, bool)> {
    let mut f = match fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((1, false)),
        Err(e) => return Err(e),
    };
    let len = f.metadata()?.len();
    let start = len.saturating_sub(TAIL);
    f.seek(SeekFrom::Start(start))?;
    let mut tail = Vec::new();
    f.read_to_end(&mut tail)?;
    let torn = tail.last().is_some_and(|&b| b != b'\n');
    // a first line cut by the start of the tail does not parse and is passed over
    let found = match last_seq(&tail) {
        Some(s) => Some(s),
        None if start > 0 => last_seq(&fs::read(path)?),
        None => None,
    };
    Ok((found.map_or(1, |s| s + 1), torn))
}

/// Append `line` (a JSON object) to the journal of the project at `root`, numbered under [`LOCK`]. Returns the `seq`
/// written, or `None` when the line was appended without one (the lock not had within [`LOCK_WAIT`], or the journal
/// unreadable) or not appended at all. Best effort: errors are not reported.
pub fn append(root: &Path, mut line: Value) -> Option<u64> {
    let path = root.join(EDIT_LOG);
    if let Some(d) = path.parent() {
        let _ = fs::create_dir_all(d);
    }
    let held = lock(root);
    let mut seq = None;
    let mut torn = false;
    if let Some((_, broke)) = &held {
        if let Ok((s, t)) = next_seq(&path) {
            seq = Some(s);
            torn = t;
            if let Some(o) = line.as_object_mut() {
                o.insert("seq".into(), s.into());
                if let Some(age) = broke {
                    o.insert("journal_lock_broken_s".into(), (*age).into());
                }
            }
        }
    }
    let text = format!("{}{line}\n", if torn { "\n" } else { "" });
    let written = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(text.as_bytes()));
    drop(held);
    written.ok().and(seq)
}
