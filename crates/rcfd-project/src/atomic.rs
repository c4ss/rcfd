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

pub(crate) fn with_retry<T>(
    op: &'static str,
    path: &Path,
    r: &Retry,
    mut f: impl FnMut() -> io::Result<T>,
) -> Result<T> {
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

/// Create the new case file `path` with `bytes`; fails with [`ProjectError::Exists`] if it exists (ADR 0835, F-998).
/// Only for a new case file under `cases/`, which `edit_case` replaces later; every other new file uses [`write_new`].
///
/// On Windows a plain write (ADR 0834): the file is created by `create_new` with share mode 0, written, synced and
/// closed, so no other open of it succeeds (a sharing violation, which [`read`] retries) until it is complete; on an
/// error of the write the file is removed. No temp file and no hard link, whose new file the System process was seen to
/// hold for up to 237 ms (ADR 0833). A writer killed in the middle leaves a prefix of the bytes under the name, which a
/// reader of the project sees as a malformed case file (ADR 0835 K1). On other systems [`write_new`].
#[cfg(windows)]
pub fn write_new_case(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::windows::fs::OpenOptionsExt;
    let mut f = match OpenOptions::new().write(true).create_new(true).share_mode(0).open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ProjectError::Exists {
                path: path.to_path_buf(),
            })
        }
        Err(e) => {
            return Err(ProjectError::Io {
                op: "create",
                path: path.to_path_buf(),
                source: e,
            })
        }
    };
    let res = f.write_all(bytes).and_then(|()| f.sync_all());
    drop(f);
    res.map_err(|e| {
        let _ = fs::remove_file(path);
        ProjectError::Io {
            op: "write",
            path: path.to_path_buf(),
            source: e,
        }
    })
}

/// Create the new case file `path` with `bytes` (ADR 0835): on this system [`write_new`].
#[cfg(not(windows))]
pub fn write_new_case(path: &Path, bytes: &[u8]) -> Result<()> {
    write_new(path, bytes)
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

/// A cooperative edit lock older than this is taken to be left by a crashed writer and is removed (ADR 0135 W48-15).
pub const EDIT_LOCK_STALE: Duration = Duration::from_secs(30);
/// How long [`replace_if_hash`] waits for a busy edit lock before it gives up with [`ProjectError::LockBusy`].
pub const EDIT_LOCK_WAIT: Duration = Duration::from_secs(2);
/// The most that the time between two looks of a waiter at a busy edit lock counts toward [`EDIT_LOCK_WAIT`] (ADR 0758
/// change W): a waiter that was not run for a while (a stopped process, a machine that slept) has not seen the lock held
/// in that time, so [`ProjectError::LockBusy`] is given only after looks no further apart than this add up to the wait.
pub const EDIT_LOCK_WAIT_STEP: Duration = Duration::from_millis(100);
/// A writer that has waited this long for a busy edit lock claims it ([`edit_claim_path`], ADR 0669 D2): while a fresh claim
/// exists, no other cooperating writer takes the lock, so the claimant gets it at the next release.
pub const EDIT_CLAIM_AFTER: Duration = Duration::from_millis(50);
/// A claim older than this is taken to be left by a crashed writer and is removed (a claimant waits at most
/// [`EDIT_LOCK_WAIT`] and removes its claim on every exit).
pub const EDIT_CLAIM_STALE: Duration = Duration::from_secs(4);
/// Project-relative edit log: one JSON line per accepted cooperative edit and per broken stale lock.
pub const EDIT_LOG: &str = ".rcfd/edits.jsonl";

/// What [`replace_if_hash`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replaced {
    /// The file written.
    pub path: PathBuf,
    /// Its project-relative form (`/` separators), as the edit log records it.
    pub rel: String,
    /// Hash of the replaced content (`None`: the file was created).
    pub old_hash: Option<String>,
    /// Hash of the new content.
    pub new_hash: String,
    /// True when a stale lock of another writer was removed first.
    pub broke_stale_lock: bool,
}

/// The edit lock of the project-relative file `rel` (normalised, `/` separators): `.rcfd/locks/<fnv64>.lock`. On
/// Windows the key is lower-cased, since names differing only in case are one file there.
pub fn edit_lock_path(root: &Path, rel: &str) -> PathBuf {
    let key = if cfg!(windows) {
        rel.to_lowercase()
    } else {
        rel.to_string()
    };
    root.join(".rcfd")
        .join("locks")
        .join(format!("{:016x}.lock", rcfd_core::hash::fnv64(key.as_bytes())))
}

/// The claim file beside the edit lock of `rel`: `.rcfd/locks/<fnv64>.claim` ([`EDIT_CLAIM_AFTER`], ADR 0669 D2).
pub fn edit_claim_path(root: &Path, rel: &str) -> PathBuf {
    edit_lock_path(root, rel).with_extension("claim")
}

/// A held edit lock; removed when dropped. The removal is retried on a transient error ([`DEFAULT_RETRY`]) and a removal
/// that still fails is logged as `edit_lock_remove_failed` (ADR 0676 R); `NotFound` (another writer broke it) is not a
/// failure.
struct EditLock {
    lock: PathBuf,
    root: PathBuf,
    rel: String,
    who: String,
}

impl Drop for EditLock {
    fn drop(&mut self) {
        if hooks::old_drop() {
            let _ = remove_lock(&self.lock);
            return;
        }
        let res = with_retry("remove edit lock", &self.lock, &DEFAULT_RETRY, || {
            remove_lock(&self.lock)
        });
        let attempts = match &res {
            Ok(()) => return,
            Err(ProjectError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => return,
            Err(ProjectError::Retry { attempts, .. }) => *attempts,
            Err(_) => 1,
        };
        let error = res.err().map(|e| e.to_string()).unwrap_or_default();
        log_edit(
            &self.root,
            serde_json::json!({"t": crate::time::now(), "kind": "edit_lock_remove_failed", "who": self.who,
                "path": self.rel, "error": error, "attempts": attempts}),
        );
    }
}

fn remove_lock(lock: &Path) -> io::Result<()> {
    if hooks::fail_removal() {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    fs::remove_file(lock)
}

/// A claim this writer made; removed when dropped (the lock is held, or the wait ended). The removal is retried on a
/// transient error ([`DEFAULT_RETRY`], ADR 0751 change B): a claim left behind refuses every writer until it is
/// [`EDIT_CLAIM_STALE`] old.
struct Claim(Option<PathBuf>);

impl Drop for Claim {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let _ = with_retry("remove edit claim", &p, &DEFAULT_RETRY, || fs::remove_file(&p));
        }
    }
}

/// True when another writer's claim of the lock is present and fresh; a stale one is removed (best effort).
fn claimed_by_other(claim: &Path) -> bool {
    match fs::metadata(claim).and_then(|m| m.modified()) {
        Ok(m) => {
            if m.elapsed().is_ok_and(|a| a > EDIT_CLAIM_STALE) {
                let _ = fs::remove_file(claim);
                false
            } else {
                true
            }
        }
        // absent, or being removed right now
        Err(_) => false,
    }
}

/// Append one line to the edit log in a single write, numbered under the journal's lock (`crate::journal::append`,
/// ADR 0188 J2). Best effort: the log is informative, a failure to write it does not undo the edit.
fn log_edit(root: &Path, line: serde_json::Value) {
    let _ = crate::journal::append(root, line);
}

fn acquire(root: &Path, rel: &str, file: &Path, who: &str) -> Result<(EditLock, bool)> {
    let lock = edit_lock_path(root, rel);
    let claim_path = edit_claim_path(root, rel);
    let mut claim = Claim(None);
    let t0 = std::time::Instant::now();
    // the wait as this waiter has seen it: the time between two looks at a busy lock, each step capped (ADR 0758 W)
    let (mut seen, mut last_look) = (Duration::ZERO, t0);
    let mut broke = false;
    let claim_on = !hooks::claim_off();
    loop {
        hooks::before_attempt(&lock);
        // ADR 0669 D2: a fresh claim of another writer is a busy lock
        let attempt = if claim_on && claim.0.is_none() && claimed_by_other(&claim_path) {
            Err(io::Error::from(io::ErrorKind::AlreadyExists))
        } else {
            OpenOptions::new().write(true).create_new(true).open(&lock)
        };
        match attempt {
            Ok(mut f) => {
                let _ = writeln!(f, "{} {} {}", std::process::id(), who, crate::time::now());
                drop(claim);
                let held = EditLock {
                    lock,
                    root: root.to_path_buf(),
                    rel: rel.to_string(),
                    who: who.to_string(),
                };
                return Ok((held, broke));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                if let Some(d) = lock.parent() {
                    fs::create_dir_all(d).map_err(io_err("create directory", d))?;
                }
                continue;
            }
            // AlreadyExists, or a lock being deleted right now (Windows reports that as access denied)
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists || is_transient(&e) => {
                let age = fs::metadata(&lock)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| m.elapsed().ok());
                if age.is_some_and(|a| a > EDIT_LOCK_STALE) && fs::remove_file(&lock).is_ok() {
                    broke = true;
                    log_edit(
                        root,
                        serde_json::json!({"t": crate::time::now(), "kind": "stale_lock_broken", "who": who,
                            "path": rel, "age_s": age.map(|a| a.as_secs_f64())}),
                    );
                    continue;
                }
                let now = std::time::Instant::now();
                seen += now.duration_since(last_look).min(EDIT_LOCK_WAIT_STEP);
                last_look = now;
                if seen >= EDIT_LOCK_WAIT {
                    return Err(ProjectError::LockBusy {
                        path: file.to_path_buf(),
                        lock,
                    });
                }
                if claim_on && claim.0.is_none() && t0.elapsed() >= EDIT_CLAIM_AFTER {
                    if let Ok(mut f) = OpenOptions::new().write(true).create_new(true).open(&claim_path) {
                        let _ = writeln!(f, "{} {} {}", std::process::id(), who, crate::time::now());
                        claim.0 = Some(claim_path.clone());
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) => {
                return Err(ProjectError::Io {
                    op: "create edit lock",
                    path: lock,
                    source: e,
                })
            }
        }
    }
}

/// Cooperative compare-and-replace (ADR 0135 W48-15 (a)): under the file's edit lock ([`edit_lock_path`]), replace the
/// project-relative file `rel` with `bytes` only if its content still has the hash `base` (`fnv64:` or the legacy
/// `fnv1a64:` spelling); `base = None` means the file must not exist yet. A mismatch is [`ProjectError::Conflict`]
/// naming the current hash; nothing is written. Parent directories are created. Every accepted write appends
/// `{t, kind: "edit", who, path, old_hash, new_hash}` to [`EDIT_LOG`], with its `seq` (`crate::journal::append`).
///
/// Limits: only writers that use this function take the lock. A writer that does not (an editor, Claude Code's own
/// file tools) can still replace the file between the check and the rename; the watcher sees such edits afterwards.
/// Breaking a stale lock is not atomic against a second writer breaking the same stale lock at the same moment.
///
/// A busy lock is waited for [`EDIT_LOCK_WAIT`], counted in looks no further apart than [`EDIT_LOCK_WAIT_STEP`] (ADR 0758).
///
/// ADR 0669: a writer that has waited [`EDIT_CLAIM_AFTER`] claims the lock ([`edit_claim_path`]) so that a writer taking the
/// lock again at once cannot starve it; a path whose first resolve fails in a way a concurrent rename can cause is resolved
/// once more under the lock.
pub fn replace_if_hash(root: &Path, rel: &str, base: Option<&str>, bytes: &[u8], who: &str) -> Result<Replaced> {
    replace_if_hash_with(root, rel, base, bytes, who, serde_json::Map::new())
}

/// [`replace_if_hash`] whose edit line also carries `extra` (ADR 0188 J1: the actor, the reason and the changed settings
/// of the write). Keys this function writes itself (`t`, `kind`, `who`, `path`, `old_hash`, `new_hash`, `seq`) are not
/// taken from `extra`.
pub fn replace_if_hash_with(
    root: &Path,
    rel: &str,
    base: Option<&str>,
    bytes: &[u8],
    who: &str,
    extra: serde_json::Map<String, serde_json::Value>,
) -> Result<Replaced> {
    let (path, norm, (held_lock, broke)) = match crate::paths::resolve_in(root, rel) {
        Ok(path) => {
            let norm = crate::paths::rel_string(root, &path).unwrap_or_else(|| rel.to_string());
            let held = acquire(root, &norm, &path, who)?;
            (path, norm, held)
        }
        // ADR 0669 D1: a cooperating writer's rename can make the resolve fail (on Windows the canonical path of the
        // replaced file is under `\$Extend\$Deleted`, or it is delete-pending); resolve once more under the lock
        Err(e) if may_have_raced(&e) => {
            let Some(norm) = crate::paths::lexical_rel(rel) else {
                return Err(e);
            };
            let held = acquire(root, &norm, &root.join(&norm), who)?;
            let path = crate::paths::resolve_in(root, rel)?;
            (path, norm, held)
        }
        Err(e) => return Err(e),
    };
    hooks::after_lock(&held_lock.lock);
    let current = match read(&path) {
        Ok(b) => Some(b),
        Err(ProjectError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let ok = match (base, &current) {
        (None, None) => true,
        (Some(h), Some(b)) => crate::records::hash_matches(h, b),
        _ => false,
    };
    let old_hash = current.as_deref().map(crate::records::hash_bytes);
    if !ok {
        return Err(ProjectError::Conflict {
            path,
            current: old_hash,
        });
    }
    if let Some(d) = path.parent() {
        mkdirs(d)?;
    }
    atomic_write(&path, bytes)?;
    let new_hash = crate::records::hash_bytes(bytes);
    let mut line = extra;
    for (k, v) in [
        ("t", serde_json::json!(crate::time::now())),
        ("kind", serde_json::json!("edit")),
        ("who", serde_json::json!(who)),
        ("path", serde_json::json!(norm)),
        ("old_hash", serde_json::json!(old_hash)),
        ("new_hash", serde_json::json!(new_hash)),
    ] {
        line.insert(k.into(), v);
    }
    line.remove("seq");
    log_edit(root, serde_json::Value::Object(line));
    Ok(Replaced {
        path,
        rel: norm,
        old_hash,
        new_hash,
        broke_stale_lock: broke,
    })
}

/// A held edit lock of one project-relative path, taken by [`lock_edit`]; released when dropped.
pub struct EditGuard {
    _held: EditLock,
    /// True when a stale lock of another writer was removed first.
    pub broke_stale_lock: bool,
}

impl std::fmt::Debug for EditGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditGuard").field("lock", &self._held.lock).finish()
    }
}

/// Take the edit lock of the project-relative path `rel` (normalised, `/` separators) as [`replace_if_hash`] does, for
/// a writer that changes the path otherwise (the project trash moves it, ADR 0736): the same wait, claim and stale-lock
/// rules; [`ProjectError::LockBusy`] after [`EDIT_LOCK_WAIT`] (counted as [`EDIT_LOCK_WAIT_STEP`] says).
pub fn lock_edit(root: &Path, rel: &str, who: &str) -> Result<EditGuard> {
    let (held, broke) = acquire(root, rel, &root.join(rel), who)?;
    Ok(EditGuard {
        _held: held,
        broke_stale_lock: broke,
    })
}

/// Errors of [`crate::paths::resolve_in`] that a concurrent replacement of the file can cause (ADR 0669 D1).
fn may_have_raced(e: &ProjectError) -> bool {
    match e {
        ProjectError::PathRefused { reason, .. } => *reason == crate::paths::OUTSIDE_ROOT,
        ProjectError::Io { .. } => true,
        _ => false,
    }
}

/// Create a directory and its parents.
pub(crate) fn mkdirs(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(io_err("create directory", path))
}

/// The test hook of the journal's append (ADR 0751): called with the path of the journal's lock right after it is
/// taken. Nothing in a build without the feature `test-hooks`.
#[inline(always)]
pub(crate) fn hook_journal_locked(lock: &Path) {
    hooks::journal_locked(lock);
}

/// The call sites of the test hooks; constant in a build without the feature `test-hooks`.
#[cfg(not(feature = "test-hooks"))]
mod hooks {
    use std::path::Path;
    #[inline(always)]
    pub(super) fn claim_off() -> bool {
        false
    }
    #[inline(always)]
    pub(super) fn old_drop() -> bool {
        false
    }
    #[inline(always)]
    pub(super) fn fail_removal() -> bool {
        false
    }
    #[inline(always)]
    pub(super) fn before_attempt(_: &Path) {}
    #[inline(always)]
    pub(super) fn after_lock(_: &Path) {}
    #[inline(always)]
    pub(super) fn journal_locked(_: &Path) {}
}

#[cfg(feature = "test-hooks")]
mod hooks {
    use super::test_hooks::HOOKS;
    use std::path::Path;
    pub(super) fn claim_off() -> bool {
        HOOKS.with(|h| h.borrow().claim_off)
    }
    pub(super) fn old_drop() -> bool {
        HOOKS.with(|h| h.borrow().old_drop)
    }
    pub(super) fn fail_removal() -> bool {
        HOOKS.with(|h| {
            let mut h = h.borrow_mut();
            if h.fail_removals > 0 {
                h.fail_removals -= 1;
                true
            } else {
                false
            }
        })
    }
    pub(super) fn before_attempt(lock: &Path) {
        // taken out while it runs, so that the callback may itself touch the hooks
        let f = HOOKS.with(|h| h.borrow_mut().before_attempt.take());
        if let Some(mut f) = f {
            f(lock);
            HOOKS.with(|h| {
                let mut h = h.borrow_mut();
                if h.before_attempt.is_none() {
                    h.before_attempt = Some(f);
                }
            });
        }
    }
    pub(super) fn after_lock(lock: &Path) {
        let f = HOOKS.with(|h| h.borrow_mut().after_lock.take());
        if let Some(mut f) = f {
            f(lock);
            HOOKS.with(|h| {
                let mut h = h.borrow_mut();
                if h.after_lock.is_none() {
                    h.after_lock = Some(f);
                }
            });
        }
    }
    pub(super) fn journal_locked(lock: &Path) {
        let f = HOOKS.with(|h| h.borrow_mut().journal_locked.take());
        if let Some(mut f) = f {
            f(lock);
            HOOKS.with(|h| {
                let mut h = h.borrow_mut();
                if h.journal_locked.is_none() {
                    h.journal_locked = Some(f);
                }
            });
        }
    }
}

/// Test-only hooks of the edit lock (ADR 0676 H), compiled only with the feature `test-hooks`, which no default build and no
/// other crate enables. They are thread-local: they act on the thread that calls [`replace_if_hash`].
#[cfg(feature = "test-hooks")]
pub mod test_hooks {
    use std::cell::RefCell;
    use std::path::Path;

    /// A callback of a hook, given the lock path.
    pub type Callback = Box<dyn FnMut(&Path)>;

    /// The hooks of the calling thread.
    #[derive(Default)]
    pub struct Hooks {
        /// Called with the lock path before every attempt of the acquire to take the lock (a delay of the waiter's
        /// attempt).
        pub before_attempt: Option<Callback>,
        /// The old acquire: another writer's claim is not respected and no claim is made (ADR 0669 mutation M1).
        pub claim_off: bool,
        /// The old drop of the edit lock: one removal, its error ignored.
        pub old_drop: bool,
        /// Called with the lock path right after the lock is held, before the read and the write.
        pub after_lock: Option<Callback>,
        /// The next this many removal attempts of the lock fail with a synthetic `PermissionDenied`.
        pub fail_removals: u32,
        /// Called with the path of the journal's lock right after `journal::append` has taken it (ADR 0751).
        pub journal_locked: Option<Callback>,
    }

    thread_local! {
        pub(super) static HOOKS: RefCell<Hooks> = RefCell::new(Hooks::default());
    }

    /// Set the calling thread's hooks.
    pub fn set(h: Hooks) {
        HOOKS.with(|c| *c.borrow_mut() = h);
    }

    /// Clear the calling thread's hooks.
    pub fn clear() {
        set(Hooks::default());
    }

    /// Open `path` for reading without delete sharing (`FILE_SHARE_READ | FILE_SHARE_WRITE`) and keep the handle open for
    /// `hold` on a thread; while it is open, a removal of the file fails with a sharing violation. Windows only (on other
    /// platforms an open file can be unlinked).
    #[cfg(windows)]
    pub fn hold_without_delete_sharing(
        path: &Path,
        hold: std::time::Duration,
    ) -> std::io::Result<std::thread::JoinHandle<()>> {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_SHARE_READ: u32 = 1;
        const FILE_SHARE_WRITE: u32 = 2;
        let f = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(path)?;
        Ok(std::thread::spawn(move || {
            std::thread::sleep(hold);
            drop(f);
        }))
    }
}
