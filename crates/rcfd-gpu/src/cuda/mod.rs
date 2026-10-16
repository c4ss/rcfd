//! The CUDA backend (compiled only with the feature `cuda`).

mod launch;
mod runtime;
mod system;

pub(crate) use runtime::runtime;
pub use system::{ops, DeviceSystem, Timings};

use rcfd_linalg::{check_system, BackendError, CsrMatrix, LinearBackend, LinearOptions, SolveControl, SolveStats};
use std::sync::Mutex;

/// Time spent on the first use of the device (toolkit location, context, kernel compilation) [s]; `None` if the
/// runtime is not initialised or failed.
pub fn init_seconds() -> Option<f64> {
    runtime().as_ref().ok().map(|r| r.init_seconds)
}

/// Free device memory reported by the driver [bytes] (diagnostic; shared with other processes).
pub fn device_free_bytes() -> Option<usize> {
    let rt = runtime().as_ref().ok()?;
    rt.stream.context().mem_get_info().ok().map(|(free, _total)| free)
}

/// Solves are serialised: they share one stream, and concurrent solves would only disturb each other's timing.
static SOLVE_LOCK: Mutex<()> = Mutex::new(());

/// The CUDA implementation of [`LinearBackend`].
pub struct CudaBackend;

impl LinearBackend for CudaBackend {
    fn name(&self) -> &str {
        "cuda"
    }

    fn cg(
        &self,
        a: &CsrMatrix,
        b: &[f64],
        x: &mut [f64],
        c: &SolveControl,
        o: &LinearOptions,
    ) -> Result<SolveStats, BackendError> {
        check_system(a, b, x)?;
        let _g = SOLVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = DeviceSystem::new(a, o)?;
        Ok(s.cg(b, x, c)?.0)
    }

    fn bicgstab(
        &self,
        a: &CsrMatrix,
        b: &[f64],
        x: &mut [f64],
        c: &SolveControl,
        o: &LinearOptions,
    ) -> Result<SolveStats, BackendError> {
        check_system(a, b, x)?;
        let _g = SOLVE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = DeviceSystem::new(a, o)?;
        Ok(s.bicgstab(b, x, c)?.0)
    }
}
