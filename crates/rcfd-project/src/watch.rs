//! Poll-based change tracking with change tokens (no file-system notification dependency).
//!
//! [`Watcher::poll`] rescans the tree and returns a token; [`Watcher::changes_since`] lists the
//! project-relative paths created, modified or deleted after a token. A file's signature is its
//! length, modification time and, up to [`DEFAULT_HASH_LIMIT`] bytes, an FNV-1a hash of its content,
//! so a same-length rewrite inside one timestamp tick is still seen for small files. The hash is reused, not
//! re-read, for a file whose length and time are unchanged and were already [`REUSE_AGE`] old when it was
//! hashed; a rewrite that restores an old time on purpose is not seen. Larger files are
//! compared by length and modification time only. `.rcfd/` and atomic-write temp files are ignored.

use crate::atomic::is_temp_name;
use crate::{io_err, paths, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Files up to this size (1 MiB) are content-hashed (again only while their time is recent, see [`REUSE_AGE`]).
pub const DEFAULT_HASH_LIMIT: u64 = 1 << 20;

/// A file whose length and time are unchanged keeps its hash once its time was this much older than the start of
/// the scan that hashed it (ADR 0128 finding 9).
pub const REUSE_AGE: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sig {
    len: u64,
    mtime: Option<SystemTime>,
    hash: Option<u64>,
}

/// Change tracker for one project root.
#[derive(Debug)]
pub struct Watcher {
    root: PathBuf,
    hash_limit: u64,
    generation: u64,
    snapshot: BTreeMap<String, Sig>,
    changed_at: BTreeMap<String, u64>,
    bytes_hashed: u64,
    /// Start of the latest scan (kept out of `Sig`, so it never reads as a change).
    scanned_at: Option<SystemTime>,
}

impl Watcher {
    /// Start watching `root`; the initial scan is token 0.
    pub fn new(root: &Path) -> Result<Self> {
        Self::with_hash_limit(root, DEFAULT_HASH_LIMIT)
    }

    /// [`Watcher::new`] with a different content-hash size limit (0 disables hashing).
    pub fn with_hash_limit(root: &Path, hash_limit: u64) -> Result<Self> {
        let mut w = Watcher {
            root: root.to_path_buf(),
            hash_limit,
            generation: 0,
            snapshot: BTreeMap::new(),
            changed_at: BTreeMap::new(),
            bytes_hashed: 0,
            scanned_at: None,
        };
        w.snapshot = w.scan()?;
        Ok(w)
    }

    /// Bytes read for content hashes since the watcher was created (cost measure).
    pub fn bytes_hashed(&self) -> u64 {
        self.bytes_hashed
    }

    /// The token of the latest poll.
    pub fn token(&self) -> u64 {
        self.generation
    }

    /// Rescan; returns the new token (unchanged if nothing changed).
    pub fn poll(&mut self) -> Result<u64> {
        let now = self.scan()?;
        let mut changed: Vec<String> = now
            .iter()
            .filter(|(p, s)| self.snapshot.get(*p) != Some(*s))
            .map(|(p, _)| p.clone())
            .collect();
        changed.extend(self.snapshot.keys().filter(|p| !now.contains_key(*p)).cloned());
        if !changed.is_empty() {
            self.generation += 1;
            for p in changed {
                self.changed_at.insert(p, self.generation);
            }
        }
        self.snapshot = now;
        Ok(self.generation)
    }

    /// Paths (relative, `/`-separated, sorted) changed after `token`, as of the latest poll.
    pub fn changes_since(&self, token: u64) -> Vec<String> {
        self.changed_at
            .iter()
            .filter(|(_, g)| **g > token)
            .map(|(p, _)| p.clone())
            .collect()
    }

    fn scan(&mut self) -> Result<BTreeMap<String, Sig>> {
        // A hash taken when the file's time was already REUSE_AGE older than that scan's start stays valid while
        // length and time are unchanged: a later write would have moved the time (granularity <= 2 s).
        let reuse_before = self.scanned_at.and_then(|t| t.checked_sub(REUSE_AGE));
        self.scanned_at = Some(SystemTime::now());
        let mut out = BTreeMap::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                // A directory deleted during the scan: its files show up as deleted.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io_err("list", &dir)(e)),
            };
            for e in entries {
                let Ok(e) = e else { continue };
                let name = e.file_name().to_string_lossy().into_owned();
                let path = e.path();
                if is_temp_name(&name) || (dir == self.root && name == ".rcfd") {
                    continue;
                }
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_dir() {
                    stack.push(path);
                    continue;
                }
                let rel = paths::rel_string(&self.root, &path).unwrap_or(name);
                let mut hashed = 0;
                let sig = std::fs::metadata(&path).ok().and_then(|md| {
                    let (len, mtime) = (md.len(), md.modified().ok());
                    let old = self.snapshot.get(&rel).filter(|o| {
                        o.hash.is_some()
                            && (o.len, o.mtime) == (len, mtime)
                            && matches!((mtime, reuse_before), (Some(m), Some(b)) if m <= b)
                    });
                    let hash = if let Some(o) = old {
                        o.hash
                    } else if len <= self.hash_limit {
                        let data = crate::atomic::read(&path).ok()?;
                        hashed = data.len() as u64;
                        Some(rcfd_core::hash::fnv64(&data))
                    } else {
                        None
                    };
                    Some(Sig { len, mtime, hash })
                });
                self.bytes_hashed += hashed;
                // A file that cannot be read right now (replaced or removed mid-scan) keeps its old
                // signature; the next poll settles it.
                if let Some(sig) = sig.or_else(|| self.snapshot.get(&rel).cloned()) {
                    out.insert(rel, sig);
                }
            }
        }
        Ok(out)
    }
}
