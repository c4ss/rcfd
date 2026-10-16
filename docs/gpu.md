# GPU linear solvers (F-101, ADR 0038)

Wave 22 adds an optional CUDA backend for the linear solvers: CSR matrix-vector product, dot / axpy / norm, Jacobi and
Chebyshev preconditioning, and Conjugate Gradient and BiCGStab running entirely on the device (f64 throughout). It is used by the
segregated pressure solve (`simple`, `piso`, `pimple`) through `linear_solver.device` (see `docs/case-schema.md`).
The default build, the default case setting (`device = "cpu"`) and every recorded baseline hash are unchanged.

## Build and run

```
cargo build --release -p rcfd-cli --features cuda
```

The `cuda` feature (off by default) compiles the `rcfd-gpu` backend. The CUDA libraries are loaded at run time (`cudarc` with
dynamic loading) and the kernels are compiled at run time with NVRTC, so building needs no toolkit and no C++ compiler; running on a
device needs the NVIDIA driver and the toolkit's NVRTC library. All `unsafe` code of the project is confined to
`crates/rcfd-gpu/src/cuda/launch.rs` (the rest of the workspace keeps `unsafe_code = "forbid"`).

### Windows set-up

1. Install the NVIDIA driver (provides `nvcuda.dll`) and the CUDA toolkit (provides `nvrtc64_*.dll`). Tested: driver 617.42,
   toolkit 13.4, RTX 5080 (sm_120). The pinned `cudarc` feature is `cuda-13040`.
2. The backend looks for the NVRTC directory in this order: `RCFD_CUDA_BIN` (directory holding `nvrtc64_*.dll`), `CUDA_PATH`,
   `CUDA_PATH_V*`, then a scan of `C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v*\bin` (and `bin\x64`). The directory
   found is prepended to `PATH` of the running process only; nothing permanent changes. If your toolkit is elsewhere, set
   `RCFD_CUDA_BIN`.
3. Check: `rcfd-gpu` reports the device name, compute capability (>= 7.0 required), memory and toolkit directory; without a usable
   device it reports the reason (`GPU TEST SKIPPED ...` in tests).

### Environment variables

| Variable | Effect |
| --- | --- |
| `RCFD_GPU_DISABLE=1` | pretend there is no device (tests the fallback); the reason names this variable |
| `RCFD_GPU_DEVICE=n` | device ordinal (default 0) |
| `RCFD_CUDA_BIN` | directory with the NVRTC library |
| `RCFD_GPU_REQUIRE=1` | GPU tests panic instead of skipping when no device is usable |

### Behaviour without a device

`device = "cpu"` never touches the GPU code. `device = "auto"` uses the device for systems of 100000 unknowns or more (fixed
threshold) when one is usable; otherwise it runs the CPU path and records why (`rcfd_gpu::fallbacks()`). `device = "cuda"` without a
usable device (or in a build without the feature) is a located error at `linear_solver.device` naming the reason.

## Tests

```
cargo test --workspace --release                      # no GPU needed; GPU tests are not compiled
cargo test -p rcfd-gpu --features cuda --release -- --nocapture
cargo test -p rcfd-api --features cuda --release --test gpu_case --test baseline_hashes
cargo test -p rcfd-gpu --features cuda --release --test f1 -- --ignored --nocapture
```

GPU tests print `GPU TEST SKIPPED: ...` when there is no device (they do not pass silently); set `RCFD_GPU_REQUIRE=1` to make that a failure.

## Benchmark

```
cargo run -p rcfd-gpu --features cuda --release --example gpu_bench -- --sizes 16,32,64,100,171 --n 5
```

Problem: 3-D 7-point Poisson on an n^3 grid, CG, Jacobi, `tol_rel = 1e-6`, `x0 = 0`. Every number is the median of 5 runs after one
warm-up run. CPU runs use a rayon pool of 1 thread or 16 threads (all logical cores of the test machine). "GPU resident" has the matrix already
on the device and counts upload of b and x, the iterations and the download of x; "GPU one-shot" additionally counts building the
device system (matrix upload). AMG-CG is the existing CPU AMG preconditioner, 1 thread, setup plus solve.

Machine: RTX 5080 (16 GiB), 16 logical CPU threads, Windows 11, driver 617.42, toolkit 13.4. First-call cost of the device (toolkit
location, context, NVRTC compile, module load): 0.29 s in the benchmark process (0.9 s in a cold test process), paid once per process.

| unknowns | CPU 1 thr CG-Jacobi [s] (its) | CPU 16 thr [s] | CPU 1 thr AMG-CG setup+solve [s] (its) | GPU resident [s] | GPU one-shot [s] | speedup resident vs 1 thr | speedup one-shot vs 1 thr | one-shot vs 1 thr AMG-CG |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 4096 | 0.0008 (34) | 0.0007 | 0.0031 + 0.0011 = 0.0041 (6) | 0.0031 | 0.0035 | 0.24x | 0.22x | 1.18x |
| 32768 | 0.0044 (25) | 0.0026 | 0.0263 + 0.0100 = 0.0363 (6) | 0.0025 | 0.0038 | 1.76x | 1.15x | 9.61x |
| 262144 | 0.0551 (29) | 0.0131 | 0.2924 + 0.1169 = 0.4093 (6) | 0.0041 | 0.0111 | 13.45x | 4.98x | 36.98x |
| 1000000 | 0.1751 (21) | 0.0867 | 1.1265 + 0.4633 = 1.5897 (6) | 0.0084 | 0.0320 | 20.87x | 5.48x | 49.71x |
| 5000211 | 1.9446 (37) | 1.2342 | 14.7274 + 3.3795 = 18.1069 (7) | 0.0625 | 0.2009 | 31.13x | 9.68x | 90.14x |

Transfer and per-iteration costs:

| unknowns | nnz | first matrix upload incl. allocation [ms] (bytes), single cold sample, NOT a median | one-shot minus resident [ms], from the medians (estimate of the matrix upload cost) | b, x upload [ms] | x download [ms] | device iterations [ms] | GPU CG-Chebyshev(4) resident [s] (its) | per iteration GPU [us] | per iteration CPU 1 thr [us] |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 4096 | 27136 | 7.33 (374788) | 0.4 | 0.04 | 0.05 | 2.98 | 0.0021 (11) | 87.7 | 22.3 |
| 32768 | 223232 | 1.05 (3072004) | 1.3 | 0.20 | 0.06 | 2.23 | 0.0024 (10) | 89.1 | 174.4 |
| 262144 | 1810432 | 4.81 (24870916) | 7.0 | 0.67 | 0.20 | 3.17 | 0.0033 (10) | 109.5 | 1899.8 |
| 1000000 | 6940000 | 71.52 (95280004) | 23.6 | 2.24 | 0.65 | 5.41 | 0.0108 (10) | 257.7 | 8339.2 |
| 5000211 | 34826031 | 263.64 (477914908) | 138.4 | 9.21 | 2.78 | 48.67 | 0.0680 (11) | 1315.3 | 52558.0 |

Reading the table honestly:

* The GPU is slower than one CPU thread at 4096 unknowns (0.22x one-shot, 0.24x resident) and about equal at 32768 unknowns
  (1.15x one-shot). The per-iteration cost of the device is about 90 us at small sizes (launch and synchronisation latency), so small
  systems do not pay. This is why `auto` has a 100000-unknown threshold.
* Against 16 CPU threads the one-shot GPU gain is smaller: 5000211 unknowns 1.2342 s vs 0.2009 s (6.1x), 1000000 unknowns 0.0867 vs 0.0320 (2.7x),
  262144 unknowns 0.0131 vs 0.0111 (1.2x); at 32768 and 4096 unknowns the 16-thread CPU is faster (0.0026 vs 0.0038 s; 0.0007 vs 0.0035 s).
* The first-upload column is one cold sample taken before the warm-up (it includes first-touch allocation), so it can exceed the one-shot median
  (4096 and 1M unknowns); use the derived column. One-shot minus resident, from medians, is about 63% of the one-shot time at 262144 unknowns,
  74% at 1M and 69% at 5M: the matrix upload is the largest part of the one-shot cost. In the pressure solve of a case the matrix changes
  every outer iteration, so the one-shot figure is the relevant one there. The resident figure applies to repeated solves with one matrix.
* The CPU AMG-CG (1 thread) needs far fewer iterations (6 to 7) but its setup cost is large; the device Jacobi-CG is faster in wall time
  at every size measured (1.18x at 4096 unknowns, 90x at 5000211) including set-up. This compares different algorithms; it is not a statement that Jacobi-CG is the better method.
* Timing noise is real: a second run of the F1 test measured the 1-thread CPU solve at 2.84 s (this table: 1.94 s) and the one-shot GPU at 0.187 s (15x);
  the speedup range at 5M unknowns is therefore roughly 10x to 15x one-shot, 31x to 45x resident.
* The CPU 1-thread time per iteration jumps from 8.3 ms (1M) to 52.6 ms (5M), well above the 5x size ratio; the 5M matrix (about 480 MB)
  no longer fits the caches. Not investigated further.

## Gaps (F-101 is `partial`)

Delivered: CUDA through `cudarc`, f64 NVRTC kernels, CG and BiCGStab on the device, Jacobi and Chebyshev preconditioners, the case
setting and benchmarks on one device. Not delivered:

* multi-GPU and domain-decomposed device solves;
* device-resident finite-volume assembly (the matrix is assembled on the CPU and uploaded for each solve);
* AMG, ILU, GMRES on the device (the device path replaces the CPU's DIC/AMG preconditioning of the pressure CG with Jacobi or Chebyshev);
* other vendors (ROCm, oneAPI, Metal), and non-segregated (coupled), compressible or variable-density cases on the device;
* the run header does not print the device decision yet;
* the result is measured on a single machine (one GPU model, one CPU).
