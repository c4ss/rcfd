//! Process-wide CUDA runtime: toolkit location, context, NVRTC-compiled module and the buffer wrapper with allocator
//! accounting.

use crate::{note_alloc, note_free, DeviceInfo};
use cudarc::driver::{CudaContext, CudaFunction, CudaSlice, CudaStream, DeviceRepr, DriverError, ValidAsZeroBits};
use cudarc::nvrtc::{compile_ptx_with_opts, CompileOptions};
use rcfd_linalg::BackendError;
use std::sync::{Arc, OnceLock};

const KERNEL_SRC: &str = include_str!("kernels.cu");

/// Kernel handles of the compiled module.
pub(super) struct Kernels {
    pub spmv: CudaFunction,
    pub axpy: CudaFunction,
    pub xpby: CudaFunction,
    pub rsub: CudaFunction,
    pub mul: CudaFunction,
    pub bicg_p: CudaFunction,
    pub sub_scaled: CudaFunction,
    pub cheb_init: CudaFunction,
    pub cheb_step: CudaFunction,
    pub dot_partial: CudaFunction,
    pub dot2_partial: CudaFunction,
    pub sum_partial: CudaFunction,
    pub reduce_final: CudaFunction,
}

/// The initialised runtime (one device, one stream, one module).
pub(crate) struct Runtime {
    pub info: DeviceInfo,
    pub stream: Arc<CudaStream>,
    pub(super) k: Kernels,
    /// Time spent locating the toolkit, creating the context and compiling the kernels [s].
    pub init_seconds: f64,
}

static RUNTIME: OnceLock<Result<Runtime, String>> = OnceLock::new();

/// The runtime, initialised on first use. The error text names the failing step.
pub(crate) fn runtime() -> &'static Result<Runtime, String> {
    RUNTIME.get_or_init(|| {
        // The driver/NVRTC loaders of cudarc panic when a library is missing; check first, and catch as a backstop.
        std::panic::catch_unwind(init)
            .unwrap_or_else(|_| Err("CUDA library loading panicked (library missing or incompatible)".to_string()))
    })
}

fn prepend_path(dir: &std::path::Path) {
    let mut parts: Vec<std::path::PathBuf> = vec![dir.to_path_buf()];
    if let Some(old) = std::env::var_os("PATH") {
        parts.extend(std::env::split_paths(&old));
    }
    if let Ok(joined) = std::env::join_paths(parts) {
        // In-process only (never persisted). Done once, before the first library load, while this thread is the
        // only one initialising the runtime.
        std::env::set_var("PATH", joined);
    }
}

fn init() -> Result<Runtime, String> {
    let t0 = std::time::Instant::now();
    // 1 toolkit (NVRTC) location; the directory goes on this process's PATH only
    let toolkit = crate::toolkit::locate();
    let (toolkit_dir, toolkit_source) = match &toolkit {
        Ok(t) => {
            prepend_path(&t.dll_dir);
            (t.dll_dir.display().to_string(), t.source.clone())
        }
        Err(_) => (String::new(), "search path".to_string()),
    };
    // 2 driver
    if !super::launch::driver_present() {
        return Err("step driver: the CUDA driver library (nvcuda.dll / libcuda.so) could not be loaded; install an NVIDIA driver".to_string());
    }
    if !super::launch::nvrtc_present() {
        let why = toolkit
            .err()
            .unwrap_or_else(|| "the located directory did not load".to_string());
        return Err(format!("step nvrtc: {why}"));
    }
    let n = CudaContext::device_count().map_err(|e| format!("step device count: {e}"))?;
    if n <= 0 {
        return Err("step device count: no CUDA device found".to_string());
    }
    let ordinal = std::env::var("RCFD_GPU_DEVICE")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0);
    if ordinal >= n as usize {
        return Err(format!(
            "step device select: RCFD_GPU_DEVICE={ordinal} but only {n} device(s) exist"
        ));
    }
    let ctx = CudaContext::new(ordinal).map_err(|e| format!("step context creation: {e}"))?;
    let name = ctx.name().map_err(|e| format!("step device name: {e}"))?;
    let cc = ctx
        .compute_capability()
        .map_err(|e| format!("step compute capability: {e}"))?;
    let total_mem = ctx.total_mem().map_err(|e| format!("step device memory: {e}"))?;
    // 3 kernels
    let arch = format!("--gpu-architecture=compute_{}{}", cc.0, cc.1);
    let opts = CompileOptions {
        fmad: Some(false),
        options: vec![arch],
        ..Default::default()
    };
    let ptx = compile_ptx_with_opts(KERNEL_SRC, opts).map_err(|e| format!("step nvrtc compile of kernels.cu: {e}"))?;
    let module = ctx
        .load_module(ptx)
        .map_err(|e| format!("step module load (PTX JIT for sm_{}{}): {e}", cc.0, cc.1))?;
    let f = |name: &str| {
        module
            .load_function(name)
            .map_err(|e| format!("step kernel lookup `{name}`: {e}"))
    };
    let k = Kernels {
        spmv: f("spmv")?,
        axpy: f("axpy")?,
        xpby: f("xpby")?,
        rsub: f("rsub")?,
        mul: f("mul")?,
        bicg_p: f("bicg_p")?,
        sub_scaled: f("sub_scaled")?,
        cheb_init: f("cheb_init")?,
        cheb_step: f("cheb_step")?,
        dot_partial: f("dot_partial")?,
        dot2_partial: f("dot2_partial")?,
        sum_partial: f("sum_partial")?,
        reduce_final: f("reduce_final")?,
    };
    let stream = ctx.default_stream();
    Ok(Runtime {
        info: DeviceInfo {
            name,
            compute_capability: cc,
            total_mem,
            toolkit_dir,
            toolkit_source,
        },
        stream,
        k,
        init_seconds: t0.elapsed().as_secs_f64(),
    })
}

/// The runtime or a located error.
pub(crate) fn rt() -> Result<&'static Runtime, BackendError> {
    runtime()
        .as_ref()
        .map_err(|e| BackendError::new(format!("cuda device unavailable: {e}")))
}

/// Map a driver error to a located backend error.
pub(crate) fn de(step: &str) -> impl Fn(DriverError) -> BackendError + '_ {
    move |e| BackendError::new(format!("cuda {step}: {e}"))
}

/// Device buffer with allocator accounting (every device allocation of this crate goes through it).
pub(crate) struct DeviceBuf<T: DeviceRepr> {
    pub(super) s: CudaSlice<T>,
    bytes: usize,
}

impl<T: DeviceRepr + ValidAsZeroBits> DeviceBuf<T> {
    /// Zero-initialised buffer of `len` elements (at least one element is allocated).
    pub fn zeros(rt: &Runtime, len: usize) -> Result<Self, BackendError> {
        let s = rt.stream.alloc_zeros::<T>(len.max(1)).map_err(|e| {
            BackendError::new(format!(
                "cuda allocation of {} bytes failed: {e}",
                len.max(1) * std::mem::size_of::<T>()
            ))
        })?;
        let bytes = len.max(1) * std::mem::size_of::<T>();
        note_alloc(bytes);
        Ok(DeviceBuf { s, bytes })
    }

    /// Buffer holding a copy of `host`.
    pub fn from_host(rt: &Runtime, host: &[T]) -> Result<Self, BackendError> {
        let mut b = Self::zeros(rt, host.len())?;
        if !host.is_empty() {
            rt.stream
                .memcpy_htod(host, &mut b.s)
                .map_err(de("host-to-device copy"))?;
        }
        Ok(b)
    }
}

impl<T: DeviceRepr> DeviceBuf<T> {
    /// Number of elements.
    pub fn len(&self) -> usize {
        self.s.len()
    }

    /// Copy into `host` (synchronises the stream).
    pub fn to_host(&self, rt: &Runtime, host: &mut [T]) -> Result<(), BackendError> {
        if host.is_empty() {
            return Ok(());
        }
        rt.stream
            .memcpy_dtoh(&self.s, host)
            .map_err(de("device-to-host copy"))?;
        rt.stream.synchronize().map_err(de("stream synchronise"))
    }
}

impl<T: DeviceRepr> Drop for DeviceBuf<T> {
    fn drop(&mut self) {
        note_free(self.bytes);
    }
}
