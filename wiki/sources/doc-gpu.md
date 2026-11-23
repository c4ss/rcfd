---
type: source
tags: [gpu, cuda, linear-solver, performance]
sources: []
origin: docs/gpu.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# GPU linear solvers (F-101, ADR 0038)

**Origin:** [docs/gpu.md](../../docs/gpu.md) at `origin/main` 2e94e5ef
**Kind:** user page with benchmark (137 lines)

## Summary

The optional CUDA path runs the pressure-solve linear solvers on the device. Small systems do not pay; `auto` uses a 100000-unknown threshold. F-101 is `partial`. The default build, default case setting (`device = "cpu"`) and every recorded baseline hash are unchanged.

## Key points

- Wave 22: CSR SpMV, dot/axpy/norm, Jacobi and Chebyshev preconditioning, CG and BiCGStab entirely on the device, f64 throughout; used by `simple`, `piso`, `pimple` through `linear_solver.device`.
- **Not delivered:** multi-GPU, device-resident FV assembly (matrix assembled on CPU and uploaded per solve), AMG/ILU/GMRES on the device (Jacobi or Chebyshev replace DIC/AMG), other vendors, coupled / compressible / variable-density cases, run header device decision, more than one measured machine.
- `device = "cpu"` never touches GPU code; `"auto"` uses the device at >= 100000 unknowns and records why not (`rcfd_gpu::fallbacks()`); `"cuda"` without a device is a located error at `linear_solver.device`.
- Build: `cargo build --release -p rcfd-cli --features cuda`; `cudarc` dynamic loading, NVRTC at run time, no toolkit or C++ compiler needed to build. All `unsafe` is in `crates/rcfd-gpu/src/cuda/launch.rs`.
- Tested: driver 617.42, toolkit 13.4, RTX 5080 (sm_120), `cudarc` feature `cuda-13040`; compute capability >= 7.0 required. Env: `RCFD_GPU_DISABLE`, `RCFD_GPU_DEVICE`, `RCFD_CUDA_BIN`, `RCFD_GPU_REQUIRE`.
- **Benchmark** (3-D 7-point Poisson, CG-Jacobi, tol 1e-6, median of 5): one-shot vs 1 CPU thread 0.22x at 4096, 1.15x at 32768, 4.98x at 262144, 5.48x at 1M, 9.68x at 5000211 unknowns; vs 16 threads 6.1x at 5M; 16 threads faster at 32768 and
  4096. Device per-iteration cost about 90 us at small sizes. Matrix upload is 63 to 74 percent of one-shot time at large sizes; "the one-shot figure is the relevant one" because the matrix changes every outer iteration. Noise: 5M speedup "roughly 10x to 15x one-shot, 31x to 45x resident".

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[rcfd-gpu]]
- [[rcfd-linalg]]
- [[bit-identical]]
