//! Run ids `r-<UTC yyyymmddThhmmss>-<4 hex>`.
//!
//! Uniqueness does not rest on randomness: an id is owned by whoever creates its directory with
//! `create_dir`, which is atomic and fails if the directory exists, across threads and processes.
//! On a clash the suffix is probed linearly from a random start, so a free suffix is always found
//! while the second has one. Ids sort by time to the second; within a second the listing orders by
//! `created` (milliseconds) and then id.

use crate::{ProjectError, Result};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// True if `s` has the run-id form.
pub fn is_run_id(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 22
        && s.starts_with("r-")
        && b[2..10].iter().all(u8::is_ascii_digit)
        && b[10] == b'T'
        && b[11..17].iter().all(u8::is_ascii_digit)
        && b[17] == b'-'
        && b[18..].iter().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(c))
}

fn random_u64() -> u64 {
    // std's RandomState is seeded from the OS; mixed with the clock for good measure.
    let mut h = RandomState::new().build_hasher();
    h.write_u128(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.finish()
}

/// Claim a fresh run directory under `runs_dir` at time `now`. Returns the id and its directory.
pub fn claim(runs_dir: &Path, now: SystemTime) -> Result<(String, PathBuf)> {
    claim_with(runs_dir, &crate::time::compact(now), 16).map(|(id, dir, _)| (id, dir))
}

/// [`claim`] with an explicit stamp and suffix space of `2^bits` values (`bits <= 16`); also returns
/// the number of clashes met. Small spaces exist for the forced-contention check (ADR 0122 R2).
#[doc(hidden)]
pub fn claim_with(runs_dir: &Path, stamp: &str, bits: u32) -> Result<(String, PathBuf, u32)> {
    let space = 1u32 << bits.min(16);
    let start = (random_u64() % u64::from(space)) as u32;
    for i in 0..space {
        let id = format!("r-{stamp}-{:04x}", (start + i) % space);
        let dir = runs_dir.join(&id);
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok((id, dir, i)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(ProjectError::Io {
                    op: "create run directory",
                    path: dir,
                    source: e,
                })
            }
        }
    }
    Err(ProjectError::Invalid {
        path: runs_dir.to_path_buf(),
        msg: format!("all {space} run ids of second {stamp} are taken"),
    })
}
