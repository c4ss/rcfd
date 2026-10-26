//! Run records of the wave-47 project folder contract (`docs/ui-restart-plan.md` section 8).
//!
//! Local stand-in for the `rcfd-project` crate (ADR 0122), built in parallel. ADR 0123 lists what changes
//! when these types are replaced by `rcfd-project`'s: everything here is reached through [`crate::store::RunStore`].

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// `status.json` state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    /// Waiting for a slot.
    Queued,
    /// A solver process is working on it.
    Running,
    /// Held: before launch (not scheduled) or between two solver steps.
    Paused,
    /// The solver finished (converged, end time, or its iteration cap).
    Finished,
    /// The run cannot continue; `reason` says why.
    Failed,
    /// Cancelled on request.
    Cancelled,
}

impl RunState {
    /// Finished, failed or cancelled.
    pub fn is_terminal(self) -> bool {
        matches!(self, RunState::Finished | RunState::Failed | RunState::Cancelled)
    }
}

/// `runs/<id>/status.json`, replaced atomically.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStatus {
    /// State.
    pub state: RunState,
    /// Solver steps (iterations) completed.
    pub step: u64,
    /// Simulation time.
    pub time: f64,
    /// Solver process id while one exists.
    pub pid: Option<u32>,
    /// UTC time of this write.
    pub updated: String,
    /// Why the run is in this state (failures always carry one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Solver exit code (from its finish event or the process).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

impl RunStatus {
    /// A fresh status in `state`.
    pub fn new(state: RunState) -> Self {
        RunStatus {
            state,
            step: 0,
            time: 0.0,
            pid: None,
            updated: now_iso(),
            reason: None,
            exit_code: None,
        }
    }
}

fn one() -> usize {
    1
}

/// Scheduling and solver settings of a run (part of `run.json`).
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

/// `runs/<id>/run.json`, written once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Run id (`r-<UTC yyyymmddThhmmss>-<4 hex>`).
    pub id: String,
    /// UTC creation time with nanoseconds (submission order).
    pub created: String,
    /// The case file the snapshot was taken from.
    pub case_source: String,
    /// Directory the case's relative paths resolve against.
    pub case_dir: String,
    /// `fnv1a64:<hex>` of `case.snapshot`.
    pub case_snapshot_hash: String,
    /// Not computed by the runner (the mesh is resolved inside the solver); filled by `rcfd-project` later.
    pub mesh_hash: Option<String>,
    /// `rcfd --version` of the solver binary at submission, when it could be run.
    pub solver_version: Option<String>,
    /// Settings.
    pub settings: RunSettings,
}

/// FNV-1a 64-bit hash (not cryptographic; identifies a snapshot's content).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// (year, month, day) of a day count since 1970-01-01 (H. Hinnant's civil-from-days).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn split(t: SystemTime) -> (i64, u32, u32, u64, u32) {
    let d = t.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let s = d.as_secs();
    let (y, mo, da) = civil((s / 86_400) as i64);
    (y, mo, da, s % 86_400, d.subsec_nanos())
}

/// UTC now as `yyyy-mm-ddThh:mm:ss.nnnnnnnnnZ` (sorts in time order).
pub fn now_iso() -> String {
    let (y, mo, d, sod, ns) = split(SystemTime::now());
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}.{ns:09}Z",
        sod / 3600,
        sod / 60 % 60,
        sod % 60
    )
}

/// A new run id `r-<UTC yyyymmddThhmmss>-<4 hex>`.
pub fn new_run_id() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now();
    let (y, mo, d, sod, ns) = split(now);
    let salt = format!("{ns}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed));
    let hex = fnv1a64(salt.as_bytes()) & 0xffff;
    format!(
        "r-{y:04}{mo:02}{d:02}T{:02}{:02}{:02}-{hex:04x}",
        sod / 3600,
        sod / 60 % 60,
        sod % 60
    )
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

/// Write `bytes` to `path` through a temporary file in the same directory and a rename, so readers see the
/// old or the new content, never a part.
// shortcut: no fsync, so a power loss can lose the last write; a killed process cannot. Add sync_all if needed.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static N: AtomicU64 = AtomicU64::new(0);
    let mut tmp: PathBuf = path.to_path_buf().into_os_string().into();
    tmp.as_mut_os_string().push(format!(
        ".tmp-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&tmp, bytes)?;
    retry(|| std::fs::rename(&tmp, path)).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e
    })
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
