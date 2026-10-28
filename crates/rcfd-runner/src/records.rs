//! Run records of the project folder contract (`docs/ui-restart-plan.md` section 8).
//!
//! The record types are `rcfd-project`'s (ADR 0130 D1, the switch that ADR 0123 "Switching to rcfd-project" lists);
//! only the typed scheduling settings, which `rcfd-project` keeps as free JSON, live here. Everything is reached
//! through [`crate::store::RunStore`].

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, SystemTime};

pub use rcfd_project::records::{RunRecord, RunState, RunStatus};

fn one() -> usize {
    1
}

/// Scheduling and solver settings of a run (`settings` of `run.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSettings {
    /// Higher runs first; equal priorities run in submission order.
    #[serde(default)]
    pub priority: i32,
    /// Solver worker threads (`rcfd run --threads`).
    #[serde(default = "one")]
    pub threads: usize,
    /// Declared memory need in MB; when absent the runner estimates it from the cell count.
    #[serde(default)]
    pub mem_mb: Option<u64>,
    /// `rcfd run --set path=value` overrides, in order.
    #[serde(default)]
    pub set: Vec<String>,
}

impl Default for RunSettings {
    fn default() -> Self {
        RunSettings {
            priority: 0,
            threads: 1,
            mem_mb: None,
            set: Vec::new(),
        }
    }
}

impl RunSettings {
    /// The settings of a run record (`settings` of `run.json`; every key optional).
    pub fn of(rec: &RunRecord) -> Result<RunSettings, String> {
        serde_json::from_value(rec.settings.clone()).map_err(|e| format!("run.json settings: {e}"))
    }
}

/// FNV-1a 64-bit hash (not cryptographic; identifies a snapshot's content): `rcfd_core::hash::fnv64`.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    rcfd_project::records::fnv64(bytes)
}

/// UTC now as `yyyy-mm-ddThh:mm:ss.nnnnnnnnnZ` (sorts in time order).
pub fn now_iso() -> String {
    rcfd_project::time::rfc3339_ns(SystemTime::now())
}

/// A new run id `r-<UTC yyyymmddThhmmss>-<4 hex>` (not claimed; see [`rcfd_project::runid::candidate`]).
pub fn new_run_id() -> String {
    rcfd_project::runid::candidate(SystemTime::now())
}

/// Retry `f` for up to about a second: on Windows a rename over a file another process has open without
/// delete sharing (a virus scanner, an editor) fails for a moment.
pub fn retry<T>(mut f: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut last = None;
    for _ in 0..40 {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => last = Some(e),
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("retry")))
}

/// An `rcfd-project` error as an I/O error of the same kind, its text naming the file.
pub(crate) fn io_error(e: rcfd_project::ProjectError) -> std::io::Error {
    let kind = match &e {
        rcfd_project::ProjectError::Io { source, .. } | rcfd_project::ProjectError::Retry { source, .. } => {
            source.kind()
        }
        rcfd_project::ProjectError::Exists { .. } => std::io::ErrorKind::AlreadyExists,
        _ => std::io::ErrorKind::InvalidData,
    };
    std::io::Error::new(kind, e.to_string())
}

/// Write `bytes` to `path` through a temporary file in the same directory and a rename, so readers see the
/// old or the new content, never a part (`rcfd_project::atomic::atomic_write`).
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    rcfd_project::atomic::atomic_write(path, bytes).map_err(io_error)
}

/// Append one line (`line` without its newline) in a single write.
pub fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(format!("{line}\n").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_and_id_format() {
        // ADR 0130 amendment 1: the runner's copy of the civil date is gone; the same dates through rcfd-project's
        let civil = rcfd_project::time::civil_from_days;
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(11_016), (2000, 2, 29));
        assert_eq!(civil(20_752), (2026, 10, 26));
        let id = new_run_id();
        assert_eq!(id.len(), "r-20261026T120000-abcd".len(), "{id}");
        assert!(id.starts_with("r-20") && id.as_bytes()[10] == b'T');
        assert_ne!(new_run_id(), new_run_id(), "ids differ within one second");
        let a = now_iso();
        assert!(a.ends_with('Z') && a.len() == 30, "{a}");
        assert!(now_iso() >= a);
    }

    #[test]
    fn atomic_write_replaces_and_leaves_no_temporary() {
        let d = std::env::temp_dir().join(format!("rcfd_runner_records_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("status.json");
        atomic_write(&p, b"one").unwrap();
        atomic_write(&p, b"two").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"two");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
        std::fs::remove_dir_all(&d).ok();
    }
}
