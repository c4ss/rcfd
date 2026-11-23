---
type: entity
tags: [crate]
sources: [doc-architecture, doc-gpu, doc-structural-solver, adr-thread-founding-decisions]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-gpu

**What it is:** Optional CUDA device solvers behind cargo feature `cuda`. The only crate with unsafe code. ([[doc-architecture]])
**Where:** `crates/rcfd-gpu/` — layer 2 (gpu / dist / runner / fem) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-linalg]]
- Used by: [[rcfd-fv]]

## Details

- Decision records: ADR 0038 ([[doc-architecture]]).
- See `docs/gpu.md`.
- **What runs on the device** (wave 22, F-101 `partial`): CSR SpMV, dot / axpy / norm, Jacobi and Chebyshev preconditioning, CG and BiCGStab, f64 throughout, through `cudarc` with f64 NVRTC kernels. Used by the segregated pressure solve (`simple`, `piso`, `pimple`) via `linear_solver.device` ([[doc-gpu]]).
- **Device selection:** `"cpu"` (default) never touches GPU code; `"auto"` uses the device at 100000 unknowns or more and records fallbacks (`rcfd_gpu::fallbacks()`); `"cuda"` without a device is a located error ([[doc-gpu]]).
- **Build:** `--features cuda`; CUDA libraries loaded at run time, kernels compiled with NVRTC, so building needs no toolkit. All `unsafe` is in `crates/rcfd-gpu/src/cuda/launch.rs` ([[doc-gpu]]).
- **Measured** (RTX 5080, one machine): one-shot GPU vs 1 CPU thread 0.22x at 4096 unknowns, 1.15x at 32768, about 10x to 15x at 5M; vs 16 threads 6.1x at 5M. Matrix upload dominates the one-shot cost; "the one-shot figure is the relevant one" in a case's pressure solve ([[doc-gpu]]).
- **Not delivered:** multi-GPU, device-resident FV assembly, AMG / ILU / GMRES on the device, other vendors, coupled / compressible / variable-density cases, device decision in the run header ([[doc-gpu]]).
- The structural solver's static solves also run on a CUDA device (F-433) ([[doc-structural-solver]]).
- Default build, default setting and every baseline hash are unchanged by the GPU path ([[doc-gpu]]); see [[bit-identical]].
- **Created by ADR 0038 (wave 22, added by p3-records-a):** "The user decided: CUDA through the `cudarc` crate, linear solvers first"; it carries out the GPU step that ADR 0003 limited to "only linear solves and flux assembly behind a trait"; the workspace keeps `unsafe_code = "forbid"` for every other crate ([[adr-thread-founding-decisions]]).

## Open questions

_Filled in by P1-P3 ingests._
