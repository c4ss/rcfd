# ADR 0038: Wave 22, GPU linear solvers through CUDA (F-101)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave existed.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (file names, measured numbers, deviations) may be appended at the
end without touching a registered limit.

Scope. F-101 is `todo`; this wave delivers a first slice (device linear solvers for the pressure Poisson equation) and stops at
`partial` at best. Named gaps, none of which is attempted: device-resident finite-volume assembly (flux assembly stays on the
CPU, the matrix is uploaded every solve), AMG on the device, ILU/IC on the device, GMRES on the device, multi-GPU and MPI
(F-100), non-NVIDIA vendors (wgpu was not chosen), and every flow/energy/species/turbulence/radiation equation other than the
pressure equation of the segregated incompressible algorithms. Clean-room: published algorithms only (CSR SpMV, CG, BiCGStab,
Chebyshev polynomial preconditioning, tree reductions); no vendor sample code, text or data. NVIDIA and CUDA appear only as
technology names. The user decided: CUDA through the `cudarc` crate, linear solvers first.

## Decisions

### D1 Crate, feature, dependency

New crate `crates/rcfd-gpu`. Cargo feature `cuda` (OFF by default, in every crate that forwards it: `rcfd-fv`, `rcfd-api`,
`rcfd-cli` forward `cuda = ["rcfd-gpu/cuda"]`). Without the feature `rcfd-gpu` contains no CUDA code and no `cudarc`
dependency: `cudarc` is an optional dependency pinned `=0.19.10` with `default-features = false` and features
`std, driver, nvrtc, dynamic-loading, cuda-13040`. Dynamic loading means no link-time dependency on the toolkit and no `nvcc`
and no MSVC `cl.exe` are needed: kernels are CUDA C source strings compiled at run time by NVRTC (options `--fmad=false`,
`--gpu-architecture=compute_<major><minor>` of the device, PTX loaded by the driver). A default `cargo build/test` never loads
a CUDA library.

### D2 Unsafe policy (changes a workspace policy)

The workspace sets `unsafe_code = "forbid"`. Kernel launches (`LaunchArgs::launch`) are `unsafe` in the binding and cannot be
avoided. Decision: `forbid` stays workspace-wide for every crate except `rcfd-gpu`, which does not inherit the workspace
`unsafe_code` entry (a `forbid` cannot be relaxed from inside a crate) and declares its own `[lints]`: the same `missing_docs`
and clippy entries as the workspace, `unsafe_code = "deny"`, clippy `undocumented_unsafe_blocks = "deny"`. All `unsafe` lives in
exactly one module of `rcfd-gpu` (`launch.rs`), under a module-level `#[allow(unsafe_code)]`, every block carries a `SAFETY:`
comment, and the module is only compiled with the feature `cuda`; `lib.rs` has `#![cfg_attr(not(feature = "cuda"), forbid(unsafe_code))]`.
A test scans the sources of every other crate and fails on any `unsafe` keyword outside comments and string literals.

### D3 Layering and the backend-neutral boundary

`rcfd-linalg::backend` (new, additive, nothing existing changes): `LinearDevice {Cpu, Cuda, Auto}` (parse/print, `Cpu` default),
`GpuPrecond {Jacobi, Chebyshev}`, `LinearOptions {device, precond, chebyshev_degree}` (Copy), `BackendError` (located
message), and the trait `LinearBackend: Send + Sync` with `name()`, `cg(a, b, x, ctl, opts)` and `bicgstab(...)` on
`CsrMatrix`, returning `Result<SolveStats, BackendError>`. `CpuBackend` implements it with the existing CPU `cg`/`bicgstab` and the
CPU twin of the device preconditioners. `rcfd-linalg::precond` gains `Chebyshev` (CSR, host, same recurrence and the same
Gershgorin bound as the device version, so iteration counts are comparable). `rcfd-gpu` depends on `rcfd-linalg` and provides
`CudaBackend` plus `resolve(device, n) -> Resolved` (a CPU decision carries a reason string). `rcfd-fv` calls
`rcfd_gpu::resolve` from the pressure solve only when `device != Cpu`; the CPU branch is the unchanged code, so `device = cpu`
(the default) is bit-identical. The numerics of the existing CPU code are not edited.

### D4 Runtime discovery

`rcfd_gpu::probe()` returns `DeviceStatus::{Available{name, compute_capability, total_mem, driver_version, toolkit}, Unavailable{reason}}`
and never panics. Order: (1) `RCFD_GPU_DISABLE` set to anything but `0` or empty gives `Unavailable("disabled by RCFD_GPU_DISABLE")`
(this is the simulated-absent-device override); (2) feature `cuda` not compiled gives `Unavailable("built without feature cuda")`;
(3) locate the toolkit directory holding `nvrtc64_*.dll` (Windows): `RCFD_CUDA_BIN`, then `CUDA_PATH`, then `CUDA_PATH_V*`, then
the newest `C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v*` , each trying `bin\x64` then `bin`; the directory is prepended
to the PATH of the running process only (never persisted) before the first library load, and is recorded in the status;
(4) load the driver, count devices, create the context of device 0 (`RCFD_GPU_DEVICE` selects another ordinal), compile the
kernels. Each failing step produces a located reason ("nvrtc not found; searched: ...", "no CUDA device", the driver error).
The result is cached in a process-wide `OnceLock`.

### D5 Kernels (f64, NVRTC)

CSR matrix with `i32` row pointers and columns (error above 2^31-1 nonzeros, located), `f64` values. Kernels: SpMV (one thread
per row, sequential accumulation in row order), `axpy`, `xpby`, `scale`, `copy`, elementwise multiply (diagonal
preconditioner), fused updates, and a two-stage reduction for `dot`/`sum`/`norm`: stage 1 a fixed grid (`min(ceil(n/256), 1024)`
blocks of 256 threads) with grid-stride accumulation and a fixed-shape shared-memory tree, stage 2 one block that sums the block
partials in fixed order. The order depends only on `n`, never on timing, so a reduction is reproducible run to run on one
device; it differs from the CPU order (`rcfd-linalg::vecops` sums 4096-element chunks), therefore bitwise equality with the
CPU is NOT required or claimed for reductions. `--fmad=false` keeps elementwise kernels and SpMV free of fused multiply-add so
they follow the CPU's two-rounding arithmetic.

Preconditioners: `jacobi` (inverse diagonal; a zero diagonal is treated as 1, as on the CPU) and `chebyshev(k)` of the
Jacobi-scaled operator `D^-1 A` (degree `k >= 1`, default 4; interval `[lmax/30, lmax]` with `lmax` the Gershgorin bound
`max_i sum_j |a_ij| / |a_ii|`, computed on the host at upload). A fixed polynomial in `D^-1 A` is a legal CG preconditioner for
SPD matrices.

Solvers: CG and BiCGStab entirely on the device. Matrix, right-hand side, iterate, work vectors stay resident; per iteration
only scalars (dot products, norms) cross to the host, where the scalar recurrences (alpha, beta, omega) are evaluated and passed
back as kernel arguments. Same stopping rule as the CPU (`tol_abs`, `tol_rel * initial_residual`, `max_iter`) and the same
breakdown exits as `rcfd_linalg::solvers`. The one-shot API (`CudaBackend::cg`) uploads A, b, x and downloads x (transfers timed
separately); a resident API (`DeviceSystem`) allows repeated solves with the matrix kept.

Allocator accounting: every device buffer goes through `DeviceBuf`, which increments/decrements a process-wide live-bytes and
live-count counter (exposed as `rcfd_gpu::alloc_stats()`); the counters are the leak check.

### D6 Case settings (additive, unset sentinels)

New section `linear_solver`:
`device = "cpu"` (`cpu | cuda | auto`; unset is `cpu`), `preconditioner = "jacobi"` (`jacobi | chebyshev`, device only),
`chebyshev_degree = 0` (0 = unset = 4; range 0..32). Meaning: `cpu` is the unchanged CPU path; `cuda` runs the pressure
CG on the device and is a located error (`linear_solver.device`, with the discovery reason) if the device is unavailable or the
build lacks the feature; `auto` uses the device only if it is available and the system has at least 100000 unknowns, otherwise
the unchanged CPU path (silently, the reason is available from `rcfd_gpu::status()` and printed by `rcfd run` in the run header);
the threshold is a registered parameter, fixed before any measurement and not tuned from the benchmark. The setting applies to the
pressure equation of `simple/simplec/piso/pimple` (including the non-orthogonal-corrector systems); with the device selected the
pressure CG uses the device preconditioner (Jacobi or Chebyshev) instead of DIC or AMG (so the iteration count differs from the
CPU path and the converged fields agree only to solver tolerance). `device != cpu` together with `coupled`, `compressible` or a variable-density fluid is a located semantic error at
`linear_solver.device` (those solvers do not use the segregated pressure solve). The non-orthogonal "extras" systems (periodic
pairs) are plain CSR and also run on the device.

### D7 Benchmarks

`cargo run -p rcfd-gpu --features cuda --release --example gpu_bench`: 3-D 7-point Poisson, cube sizes `16^3` (4096),
`32^3`, `64^3`, `100^3` (1e6), `171^3` (5 000 211). Per size: CPU single thread (rayon pool of 1) CG+Jacobi, CPU all threads
CG+Jacobi, CPU single thread AMG-preconditioned CG (the best available CPU path, reported so the speedup is not flattered),
GPU CG+Jacobi resident (matrix already on the device), GPU CG+Jacobi one-shot including upload and download, and the
transfer costs alone (A+b up, x down, bytes and ms). Tolerance `tol_rel = 1e-6`, `tol_abs = 0`, `max_iter = 5000`,
right-hand side from a fixed manufactured solution. 1 warm-up run, then median of N = 5 (wall clock, device synchronised
before the clock stops). Speedup = CPU median / GPU median, printed whether above or below one. Iteration counts are printed
(GPU and CPU run the identical algorithm and tolerance). The first-call cost (toolkit location, context creation, NVRTC compile) is reported as its own line. Results go in `docs/gpu.md` as measured.

## Registered checks

All GPU tests print `GPU TEST SKIPPED: <reason>` and return when no device/toolkit is present (a visible message; run with
`--nocapture` to see it; `RCFD_GPU_REQUIRE=1` turns a skip into a failure). Tolerance statements use 2-norm relative
differences `||a-b||_2 / ||b||_2` unless stated.

- **P1 unsafe confinement.** Occurrences of the `unsafe` keyword outside comments/strings in the sources of all crates other than
  `rcfd-gpu`: exactly 0. In `rcfd-gpu` every `unsafe` is inside `launch.rs`; every `unsafe` block has a preceding `SAFETY:` line.
- **B1 baseline.** `crates/rcfd-api/tests/baseline_hashes.rs` is byte-identical to HEAD and passes in the default build; the
  `cuda` feature compiled in with `device` unset also passes it.
- **B2 default build.** `cargo test --workspace --release` passes without `cuda`; `cargo tree -p rcfd-api -e normal` (no features)
  contains no `cudarc` line.
- **D1 discovery.** With a device present, `probe()` is `Available`, reports a name and compute capability >= 7.0 and the toolkit
  directory used, with `CUDA_PATH` unset in the test process. Failure of the reason text to be located (names the step) is a failure.
- **D2 fallback.** Run in a CHILD PROCESS (the test binary re-invoked with `RCFD_GPU_DISABLE=1` and a name filter; the
  variable is never set in-process because the probe result is cached process-wide). In the child: `probe()` is `Unavailable`
  with a reason containing `RCFD_GPU_DISABLE`; `resolve(Auto, 1_000_000)` (above the 100000 threshold, so only the missing
  device can decide) returns the CPU decision with a reason containing `RCFD_GPU_DISABLE`; `resolve(Cuda, n)` is an error with the
  same reason; a cavity run with `linear_solver.device = "auto"` has all `ux, uy, p` and flux bits equal to the `cpu` run (0 differing
  bits, second assertion); `device = "cuda"` yields a located diagnostic at path `linear_solver.device`. The default build (no
  feature) must produce the same located error for `device = "cuda"` (default-build test).
- **K1 SpMV.** GPU vs CPU `CsrMatrix::matvec`, relative 2-norm difference <= 1e-13 for each of: 3-D Poisson 20^3; 2-D Poisson
  300x300; nonsymmetric convection-diffusion 100x100; a pseudo-random sparse matrix of 50000 rows with 1..40 entries per row
  including rows with only a diagonal; the 1x1 matrix `[3.5]`. (Expected to be 0 with `--fmad=false`; the limit is not
  tightened.) Max over the set is reported.
- **K2 reductions.** For `n in {1, 2, 255, 256, 257, 1000, 65537, 1000003, 4194304}` and data kinds (a) uniform in [-1,1] x
  uniform in [-1,1] (heavy cancellation), (b) positive, (c) magnitudes spanning 1e-6..1e6 with random signs: GPU dot differs from a
  Neumaier-compensated CPU sum of the (rounded) products by at most `1e-14 * sum_i |x_i y_i|`; GPU norm2 within relative 1e-14
  of the compensated reference; GPU sum within `1e-14 * sum|x_i|`; two consecutive GPU dots of the same data are bit-identical
  (run-to-run reproducibility; not claimed against the CPU). Elementwise `axpy` agrees with the CPU `axpy` to <= 2.5e-16
  componentwise relative (one ulp; expected exact).
- **S1 CG.** Matrices: 2-D Poisson 64x64, 3-D Poisson 24^3, a symmetric diagonally dominant pseudo-random sparse matrix of
  20000 rows. Preconditioners: none, Jacobi, Chebyshev(4). Control `tol_rel = 1e-12`, `tol_abs = 0`, `max_iter = 5000`, x0 = 0.
  For every combination: both converge; GPU solution vs CPU solution (CPU `cg` with the CPU twin of the same preconditioner)
  relative difference <= 1e-10; |iterations_gpu - iterations_cpu| <= 2; true residual of the GPU solution evaluated on the host
  <= 1e-10 `||b||`.
- **S2 BiCGStab.** Matrices: nonsymmetric convection-diffusion 100x100 (cell Peclet 2, upwind, diagonally dominant) and 20^3 3-D.
  Same preconditioners, same control, same three limits as S1 (the limit on the solution difference 1e-10, iterations +-2).
- **C1 full case.** 2-D lid cavity `cavity_2d` 32x32, `nu = 0.01`, steady `simple`, `linear_upwind`, 150 iterations, `solver.pressure_rel_tol
  = 1e-10` and `solver.pressure_max_iter = 5000` pinned in both runs, and a transient `piso` cavity 20x20, 20 steps; `device = cuda` (Jacobi and Chebyshev(4)) vs `device = cpu`:
  `max|a-b| / max|b|` <= 1e-6 for each of `ux, uy, p` (fixed iteration/step counts, no convergence claim).
- **C2 cpu unchanged.** The cavity case with `linear_solver.device = "cpu"` explicit has every field and flux bit equal to the
  case without the section; baseline hashes (B1) unchanged.
- **M1 leaks.** Runs in its own integration-test binary containing a single test (the counters are process-wide). The baseline is
  taken after one warm-up solve (cached module/scratch allowed). After 200 repeated one-shot CG solves (n = 10000, new buffers each time) and 50 repeated resident-system
  create/solve/drop cycles, `alloc_stats()` live bytes == value before the loop and live buffer count == value before (exact
  equality); the driver free-memory difference is reported, not a criterion.
- **E1 error paths.** `linear_solver.device = "gpu"` is a schema error at `linear_solver.device`; `chebyshev_degree = 99` is
  a schema range error at that path; `device = cuda` with `solver.algorithm = "compressible"` is a located semantic error;
  the rcfd-gpu API returns `BackendError` (message names the argument and the lengths) for a b/x length mismatch, a non-square
  matrix, and the size-validation step (a pure function of the counts, tested with synthetic counts and no allocation) rejects a
  nonzero count above `i32::MAX` and a row count above `i32::MAX` with a located message; a solve whose `max_iter` is exhausted returns `converged = false`
  without panic; a right-hand side containing NaN returns without panic or hang (converged = false, iterations <= max_iter).
- **Commands.** Default gate: `cargo test --workspace --release` (P1, B1, B2, default-build halves of D2/E1; GPU tests skip visibly).
  GPU gate: `cargo test -p rcfd-gpu --features cuda --release -- --nocapture` (D1, K1, K2, S1, S2, M1, E1 API half, D2 child) and
  `cargo test -p rcfd-api --features cuda --release --test gpu_case --test baseline_hashes -- --nocapture` (C1, C2, B1 with the
  feature, D2/E1 case halves). `rcfd-gpu` has no dev-dependency on `rcfd-api`. F1 and the benchmark table are the example
  `gpu_bench` (`required-features = ["cuda"]`) and the ignored test `f1_gpu_vs_one_thread` (`-- --ignored`), several minutes.
- **F1 performance (the only performance criterion).** At 171^3 unknowns, the GPU one-shot CG+Jacobi solve (including upload
  and download) median of 5 (1 warm-up) is faster than the CPU single-thread CG+Jacobi median of 5 (1 warm-up), same
  tolerance 1e-6. A miss is recorded as a known miss, not tuned. All other benchmark numbers (including sizes where the GPU is
  slower, the F-101 "3x vs single thread" figure, the speedup against the single-thread CPU AMG path) are REPORTED, never
  criteria.

## Roadmap

F-101 moves to `partial` with the gaps above and the measured numbers (honest, including the 3x figure whether met or not and the
fact that the acceptance wording says "5M-cell Poisson solve", whereas the CPU's best path is AMG). F-027 and F-100 are not
touched except that F-100 notes that the device partition work is not started. `docs/case-schema.md` gains the
`linear_solver` section; `docs/gpu.md` holds setup (toolkit and NVRTC location, PATH requirement, `RCFD_*` variables), the benchmark
command and the results table.

## Addendum: results and deviations (end of wave 22)

Measured on RTX 5080 (sm_120, 16275 MiB), driver 617.42, toolkit 13.4 (found by the Program Files scan; `CUDA_PATH` unset), 16 CPU threads.
No registered limit was changed.

| Check | Limit | Measured |
| --- | --- | --- |
| P1 | 0 `unsafe` outside `launch.rs`, SAFETY comments | pass |
| B1 | baseline hashes identical | pass, default and `--features cuda`, never re-recorded |
| B2 | no `cudarc` in `cargo tree -p rcfd-api -e normal` | 0 matching lines |
| D1 | device found, cc >= 7.0, toolkit dir | RTX 5080, cc 12.0; first-call cost 0.29 s (0.9 s in a cold test process) |
| D2 | child with `RCFD_GPU_DISABLE=1` | pass (probe reason, `auto` bitwise equal to cpu, located error, default build same error) |
| K1 | SpMV rel2 <= 1e-13 | 0.0 (bitwise) on all five matrices |
| K2 | dot 1e-14, norm2 1e-14, sum 1e-14, axpy 2.5e-16 | worst 1.6e-16, 1.9e-16, 1.9e-16, 0; reproducible bitwise |
| S1/S2 | rel diff <= 1e-10, iterations within 2, residual <= 1e-10 | iterations identical in all 15 cases (3 preconditioners, 5 systems); difference <= 1.1e-14; residual <= ~1e-12 |
| C1 | <= 1e-6 | worst 2.07e-12 (steady 32x32 and piso 20x20, Jacobi and Chebyshev4) |
| C2 | bit-identical | 0 differing bits (simple, piso, also with device-only keys) |
| M1 | live bytes and buffers equal | equal after 200 + 50 cycles |
| E1 | located errors | pass |
| F1 | GPU one-shot faster than 1-thread CPU at 171^3 | pass: 0.2009 s vs 1.9446 s in the benchmark (9.7x); 0.187 s vs 2.839 s (15.2x) in the ignored test |

The full benchmark table (sizes 16^3 to 171^3, 16 threads, AMG-CG, transfer costs) is in `docs/gpu.md`. The GPU is slower than one CPU thread at
4096 unknowns (0.22x one-shot).

Deviations and set-up defects, disclosed:

1. **C1 set-up defect.** The first C1 test ran the steady `simple` case with the default relaxation factors; the case diverges (fields ~1e60) on the CPU and on
   the device, so the comparison meant nothing. That test is kept as the ignored `c1_full_case_cuda_matches_cpu_diverged_setup`; the corrected test
   `c1_full_case_cuda_matches_cpu` sets `alpha_u = 0.7`, `alpha_p = 0.3` for the steady case. The limit (1e-6) is unchanged.
2. **`GpuPrecond::None`** exists in the library API only, so that the registered S1/S2 "none" preconditioner can be run; the case schema offers `jacobi` and `chebyshev`.
3. **Run header.** D6 mentioned printing the device decision in the `rcfd run` header; this is NOT implemented. The decision is available through
   `rcfd_gpu::probe`, `resolve` and `fallbacks()`, and errors are located diagnostics.
4. **Runtime failure.** A device failure during a run with `device = cuda` (after the up-front diagnostic) panics with a located message; `auto` falls back and records it.
5. **Unsupported combinations.** `device` other than `cpu` with `coupled`, compressible/energy or non-constant density is a located semantic error.
6. **Timing variation.** CPU 1-thread time at 171^3 was 1.94 s in the benchmark and 2.84 s in the F1 test; both runs pass F1.
7. **Unsafe policy.** The workspace lint `unsafe_code = "forbid"` stays for every other crate; `rcfd-gpu` has its own lint table (`deny`, allowed only in `src/cuda/launch.rs`).
8. **Layering test.** The existing test `layering_matches_architecture_doc` (rcfd-cli/tests/workspace_graph.rs) failed on the first full-workspace gate run with `unknown crate rcfd-gpu`. `rcfd-gpu` is now a layer of its own between `linalg` and `fv` (the later layers are renumbered, their relative order is unchanged), and `docs/architecture.md` lists it. No rule was loosened.
9. **Benchmark reporting defect.** The first version of the transfer table in `docs/gpu.md` labelled a single cold upload sample as the matrix upload time next to medians, and it exceeded the one-shot median at two sizes. The column is relabelled as a single cold sample and a derived column (one-shot minus resident, from medians) is added. The benchmark was not rerun.

Not delivered: multi-GPU, device-resident finite-volume assembly, AMG/ILU/GMRES on the device, other vendors, non-segregated algorithms on the device.
F-101 is `partial`.
