//! Poll-based change tracking with change tokens (no file-system notification dependency).
//!
//! [`Watcher::poll`] rescans the tree and returns a token; [`Watcher::changes_since`] lists the
//! project-relative paths created, modified or deleted after a token. A file's signature is its
//! length, modification time and, up to [`DEFAULT_HASH_LIMIT`] bytes, an FNV-1a hash of its content,
//! so a same-length rewrite inside one timestamp tick is still seen for small files. Larger files are
//! compared by length and modification time only. `.rcfd/` and atomic-write temp files are ignored.

use crate::atomic::is_temp_name;
use crate::{io_err, paths, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Files up to this size (1 MiB) are content-hashed on every poll.
pub const DEFAULT_HASH_LIMIT: u64 = 1 << 20;

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
        };
        w.snapshot = w.scan()?;
        Ok(w)
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

    fn scan(&self) -> Result<BTreeMap<String, Sig>> {
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
                let sig = std::fs::metadata(&path).ok().and_then(|md| {
                    let hash = if md.len() <= self.hash_limit {
                        Some(rcfd_core::hash::fnv64(&crate::atomic::read(&path).ok()?))
                    } else {
                        None
                    };
                    Some(Sig {
                        len: md.len(),
                        mtime: md.modified().ok(),
                        hash,
                    })
                });
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
