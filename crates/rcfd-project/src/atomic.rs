//! Atomic file replacement: write a temp file in the same directory, fsync it, rename it over the
//! target. A reader sees the old or the new complete file, never a partial one (ADR 0122 A1, A2).
//!
//! On Windows a rename onto a file that another process holds open without delete sharing fails
//! (sharing violation or access denied); [`atomic_write`] retries such errors with bounded
//! exponential backoff and then returns a located [`ProjectError::Retry`].

use crate::{io_err, ProjectError, Result};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Retry policy for transient replace and read errors.
#[derive(Debug, Clone, Copy)]
pub struct Retry {
    /// Total attempts (at least 1).
    pub attempts: u32,
    /// Sleep after the first failure; doubled each time.
    pub first_backoff: Duration,
    /// Cap of a single sleep.
    pub max_backoff: Duration,
}

/// Default: 12 attempts, 1 ms doubling to a 100 ms cap, about 0.63 s of sleeping in total.
pub const DEFAULT_RETRY: Retry = Retry {
    attempts: 12,
    first_backoff: Duration::from_millis(1),
    max_backoff: Duration::from_millis(100),
};

/// True for errors that a concurrent open handle can cause and that clear once it closes.
pub fn is_transient(e: &io::Error) -> bool {
    // 32 ERROR_SHARING_VIOLATION, 33 ERROR_LOCK_VIOLATION; 5 ERROR_ACCESS_DENIED maps to PermissionDenied.
    e.kind() == io::ErrorKind::PermissionDenied || (cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)))
}

fn with_retry<T>(op: &'static str, path: &Path, r: &Retry, mut f: impl FnMut() -> io::Result<T>) -> Result<T> {
    let mut sleep = r.first_backoff;
    let attempts = r.attempts.max(1);
    for attempt in 1..=attempts {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) if is_transient(&e) && attempt < attempts => {
                std::thread::sleep(sleep);
                sleep = (sleep * 2).min(r.max_backoff);
            }
            Err(e) if is_transient(&e) => {
                return Err(ProjectError::Retry {
                    op,
                    path: path.to_path_buf(),
                    attempts,
                    source: e,
                })
            }
            Err(e) => {
                return Err(ProjectError::Io {
                    op,
                    path: path.to_path_buf(),
                    source: e,
                })
            }
        }
    }
    unreachable!("loop returns on the last attempt")
}

/// True for names of temp files made by this module (`.<name>.<pid>.<n>.tmp`).
pub fn is_temp_name(name: &str) -> bool {
    name.starts_with('.') && name.ends_with(".tmp")
}

fn temp_path(path: &Path) -> Result<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| ProjectError::Invalid {
            path: path.to_path_buf(),
            msg: "no UTF-8 file name".into(),
        })?;
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id())))
}

fn write_temp(path: &Path, bytes: &[u8]) -> Result<PathBuf> {
    write_temp_sync(path, bytes, true)
}

fn write_temp_sync(path: &Path, bytes: &[u8], sync: bool) -> Result<PathBuf> {
    let tmp = temp_path(path)?;
    let res = (|| {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        if sync {
            f.sync_all()?;
        }
        Ok(())
    })();
    if let Err(e) = res {
        let _ = fs::remove_file(&tmp);
        return Err(ProjectError::Io {
            op: "write temp file",
            path: tmp,
            source: e,
        });
    }
    Ok(tmp)
}

#[cfg(unix)]
fn sync_parent(path: &Path) {
    // Durability of the rename itself; best effort.
    if let Some(dir) = path.parent() {
        let _ = fs::File::open(dir).and_then(|d| d.sync_all());
    }
}
#[cfg(not(unix))]
fn sync_parent(_: &Path) {
    // shortcut: std cannot open a directory handle on Windows without FFI; the rename is atomic but its
    // durability across power loss is not forced. Upgrade if power-loss durability is required.
}

/// Atomically replace `path` with `bytes` using [`DEFAULT_RETRY`].
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    atomic_write_with(path, bytes, &DEFAULT_RETRY)
}

/// Atomically replace `path` with `bytes` using the given retry policy.
pub fn atomic_write_with(path: &Path, bytes: &[u8], retry: &Retry) -> Result<()> {
    let tmp = write_temp(path, bytes)?;
    let res = with_retry("replace", path, retry, || fs::rename(&tmp, path));
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res?;
    sync_parent(path);
    Ok(())
}

/// Atomically replace `path` with `bytes` like [`atomic_write`], but without any fsync: a reader still sees the old
/// or the new complete file, and a killed process loses nothing, but a power loss can lose the write. For files
/// replaced many times a second whose last version a reader polls (the run service's `status.json`, ADR 0130).
pub fn atomic_write_unsynced(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = write_temp_sync(path, bytes, false)?;
    let res = with_retry("replace", path, &DEFAULT_RETRY, || fs::rename(&tmp, path));
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

/// Atomically create `path` with `bytes`; fails with [`ProjectError::Exists`] if it exists.
/// Used for immutable files (`run.json`, `case.snapshot`). Needs hard-link support (NTFS, ext4, APFS).
pub fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = write_temp(path, bytes)?;
    let res = fs::hard_link(&tmp, path);
    let _ = fs::remove_file(&tmp);
    match res {
        Ok(()) => {
            sync_parent(path);
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(ProjectError::Exists {
            path: path.to_path_buf(),
        }),
        Err(e) => Err(ProjectError::Io {
            op: "create",
            path: path.to_path_buf(),
            source: e,
        }),
    }
}

/// Read a whole file, retrying transient errors (a reader racing a Windows rename can see one).
pub fn read(path: &Path) -> Result<Vec<u8>> {
    with_retry("read", path, &DEFAULT_RETRY, || fs::read(path))
}

/// Read and parse a JSON file.
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read(path)?;
    serde_json::from_slice(&bytes).map_err(|source| ProjectError::Json {
        path: path.to_path_buf(),
        line: None,
        source,
    })
}

/// Serialize as pretty JSON and replace atomically.
pub fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    atomic_write(path, &to_json(path, value)?)
}

/// Serialize as pretty JSON and create atomically; never replaces.
pub fn write_json_new<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    write_new(path, &to_json(path, value)?)
}

fn to_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<Vec<u8>> {
    let mut v = serde_json::to_vec_pretty(value).map_err(|source| ProjectError::Json {
        path: path.to_path_buf(),
        line: None,
        source,
    })?;
    v.push(b'\n');
    Ok(v)
}

/// Create a directory and its parents.
pub(crate) fn mkdirs(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(io_err("create directory", path))
}
