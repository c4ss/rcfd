//! CUDA device linear solvers for rcfd (ADR 0038, F-101).
//!
//! The crate is always buildable: without the cargo feature `cuda` it contains no CUDA code, `probe()` reports
//! `Unavailable("built without feature cuda")` and `resolve` selects the CPU path. With the feature, the CUDA driver
//! and NVRTC are loaded dynamically at run time (no toolkit is needed to build) and CUDA C kernels are compiled by NVRTC.
//!
//! All `unsafe` of the workspace lives in one module of this crate (`cuda/launch.rs`, kernel launches); see ADR 0038 D2.
//! Reductions on the device use a fixed, size-dependent order, so they are reproducible run to run on one device but
//! differ from the CPU summation order; no bitwise equality with the CPU is claimed for them.
#![cfg_attr(not(feature = "cuda"), forbid(unsafe_code))]

pub mod toolkit;

#[cfg(feature = "cuda")]
mod cuda;
#[cfg(feature = "cuda")]
pub use cuda::{device_free_bytes, init_seconds, ops, CudaBackend, DeviceSystem, Timings};

use rcfd_linalg::{BackendError, LinearBackend, LinearDevice};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Systems with fewer unknowns than this stay on the CPU when the device is `auto` (ADR 0038 D6; fixed before any
/// measurement and not tuned from the benchmark).
pub const AUTO_MIN_UNKNOWNS: usize = 100_000;

/// What `probe` found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Device name.
    pub name: String,
    /// Compute capability `(major, minor)`.
    pub compute_capability: (i32, i32),
    /// Total device memory [bytes].
    pub total_mem: usize,
    /// Directory the NVRTC library was loaded from (empty if the system search path found it).
    pub toolkit_dir: String,
    /// How the toolkit was located.
    pub toolkit_source: String,
}

/// Result of device discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceStatus {
    /// A usable device with compiled kernels.
    Available(DeviceInfo),
    /// No usable device; the reason names the step that failed.
    Unavailable {
        /// Located reason.
        reason: String,
    },
}

impl DeviceStatus {
    /// One-line description.
    pub fn describe(&self) -> String {
        match self {
            DeviceStatus::Available(i) => format!(
                "cuda device available: {} (sm_{}{}, {} MiB, toolkit {})",
                i.name,
                i.compute_capability.0,
                i.compute_capability.1,
                i.total_mem >> 20,
                if i.toolkit_dir.is_empty() {
                    "search path"
                } else {
                    &i.toolkit_dir
                }
            ),
            DeviceStatus::Unavailable { reason } => format!("cuda device unavailable: {reason}"),
        }
    }
}

/// Environment variable that simulates an absent device when set to anything but empty or `0`.
pub const DISABLE_VAR: &str = "RCFD_GPU_DISABLE";

static PROBE: std::sync::OnceLock<DeviceStatus> = std::sync::OnceLock::new();

/// Discover the device (cached for the process). Never panics.
pub fn probe() -> &'static DeviceStatus {
    PROBE.get_or_init(|| {
        if let Ok(v) = std::env::var(DISABLE_VAR) {
            if !v.is_empty() && v != "0" {
                return DeviceStatus::Unavailable {
                    reason: format!("disabled by {DISABLE_VAR}={v}"),
                };
            }
        }
        probe_device()
    })
}

#[cfg(feature = "cuda")]
fn probe_device() -> DeviceStatus {
    match cuda::runtime() {
        Ok(rt) => DeviceStatus::Available(rt.info.clone()),
        Err(reason) => DeviceStatus::Unavailable { reason: reason.clone() },
    }
}

#[cfg(not(feature = "cuda"))]
fn probe_device() -> DeviceStatus {
    DeviceStatus::Unavailable {
        reason: "built without feature cuda (rebuild with --features cuda)".to_string(),
    }
}

/// Outcome of choosing a backend.
pub struct Resolution {
    /// The device backend, or `None` for the CPU path.
    pub backend: Option<&'static dyn LinearBackend>,
    /// Why this choice was made (always set).
    pub reason: String,
}

#[cfg(feature = "cuda")]
fn device_backend() -> &'static dyn LinearBackend {
    static B: CudaBackend = CudaBackend;
    &B
}

#[cfg(not(feature = "cuda"))]
fn device_backend() -> &'static dyn LinearBackend {
    // never reached: probe() is Unavailable without the feature
    &rcfd_linalg::CpuBackend
}

/// Choose the backend for a system of `n` unknowns. `Cpu` and `Auto` never fail; `Cuda` is an error with the located
/// discovery reason if the device is unavailable.
pub fn resolve(device: LinearDevice, n: usize) -> Result<Resolution, BackendError> {
    match device {
        LinearDevice::Cpu => Ok(Resolution {
            backend: None,
            reason: "linear_solver.device = cpu".to_string(),
        }),
        LinearDevice::Cuda => match probe() {
            DeviceStatus::Available(_) => Ok(Resolution {
                backend: Some(device_backend()),
                reason: "linear_solver.device = cuda".to_string(),
            }),
            DeviceStatus::Unavailable { reason } => Err(BackendError::new(format!(
                "linear_solver.device = \"cuda\" but no device is usable: {reason}"
            ))),
        },
        LinearDevice::Auto => match probe() {
            DeviceStatus::Unavailable { reason } => Ok(Resolution {
                backend: None,
                reason: format!("auto: falling back to cpu, {reason}"),
            }),
            DeviceStatus::Available(_) if n < AUTO_MIN_UNKNOWNS => Ok(Resolution {
                backend: None,
                reason: format!("auto: cpu, {n} unknowns is below the threshold {AUTO_MIN_UNKNOWNS}"),
            }),
            DeviceStatus::Available(_) => Ok(Resolution {
                backend: Some(device_backend()),
                reason: format!("auto: cuda, {n} unknowns"),
            }),
        },
    }
}

static FALLBACKS: AtomicUsize = AtomicUsize::new(0);
static LAST_FALLBACK: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Record that a device solve under `device = auto` failed and the CPU path was used instead.
pub fn note_fallback(reason: &str) {
    FALLBACKS.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut g) = LAST_FALLBACK.lock() {
        *g = reason.to_string();
    }
}

/// Number of device solves that fell back to the CPU, and the last reason.
pub fn fallbacks() -> (usize, String) {
    (
        FALLBACKS.load(Ordering::SeqCst),
        LAST_FALLBACK.lock().map(|g| g.clone()).unwrap_or_default(),
    )
}

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BUFS: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);
static TOTAL_ALLOCS: AtomicUsize = AtomicUsize::new(0);

/// Process-wide device-buffer accounting (every device buffer of this crate goes through it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllocStats {
    /// Bytes currently allocated.
    pub live_bytes: usize,
    /// Buffers currently allocated.
    pub live_buffers: usize,
    /// High-water mark of live bytes.
    pub peak_bytes: usize,
    /// Number of allocations ever made.
    pub total_allocations: usize,
}

/// Current allocator accounting.
pub fn alloc_stats() -> AllocStats {
    AllocStats {
        live_bytes: LIVE_BYTES.load(Ordering::SeqCst),
        live_buffers: LIVE_BUFS.load(Ordering::SeqCst),
        peak_bytes: PEAK_BYTES.load(Ordering::SeqCst),
        total_allocations: TOTAL_ALLOCS.load(Ordering::SeqCst),
    }
}

#[cfg_attr(not(feature = "cuda"), allow(dead_code))]
pub(crate) fn note_alloc(bytes: usize) {
    let now = LIVE_BYTES.fetch_add(bytes, Ordering::SeqCst) + bytes;
    LIVE_BUFS.fetch_add(1, Ordering::SeqCst);
    TOTAL_ALLOCS.fetch_add(1, Ordering::SeqCst);
    PEAK_BYTES.fetch_max(now, Ordering::SeqCst);
}

#[cfg_attr(not(feature = "cuda"), allow(dead_code))]
pub(crate) fn note_free(bytes: usize) {
    LIVE_BYTES.fetch_sub(bytes, Ordering::SeqCst);
    LIVE_BUFS.fetch_sub(1, Ordering::SeqCst);
}

/// Skip helper for tests: prints a visible message and returns `true` when there is no usable device. With
/// `RCFD_GPU_REQUIRE=1` a missing device panics instead (so CI that has a GPU cannot pass by skipping).
pub fn skip_without_device(test: &str) -> bool {
    match probe() {
        DeviceStatus::Available(_) => false,
        DeviceStatus::Unavailable { reason } => {
            if std::env::var("RCFD_GPU_REQUIRE")
                .map(|v| !v.is_empty() && v != "0")
                .unwrap_or(false)
            {
                panic!("RCFD_GPU_REQUIRE is set but there is no usable device: {reason}");
            }
            eprintln!("GPU TEST SKIPPED: {test}: {reason}");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_resolution_never_needs_a_device() {
        let r = resolve(LinearDevice::Cpu, 10).unwrap();
        assert!(r.backend.is_none());
    }

    #[test]
    fn alloc_accounting_balances() {
        let before = alloc_stats();
        note_alloc(100);
        note_alloc(50);
        assert_eq!(alloc_stats().live_bytes, before.live_bytes + 150);
        note_free(100);
        note_free(50);
        assert_eq!(alloc_stats().live_bytes, before.live_bytes);
        assert_eq!(alloc_stats().live_buffers, before.live_buffers);
    }
}
