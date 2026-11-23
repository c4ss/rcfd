---
type: source
tags: [adr-thread, architecture, settings, field-storage, parallel, gpu, distributed, crates, msrv]
sources: [doc-architecture, doc-gpu]
origin: docs/adr/ (ADR list below)
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# ADR thread: the founding architecture decisions and the crates the early ADRs created

**Origin:** the ADRs in the table below, `docs/adr/` at `origin/main` 2e94e5ef.
**Kind:** ADR thread (short decision records 0001 to 0004 without a date, then the wave ADRs that carried the parallel strategy out), 6 ADRs, plus a table of the crates whose creation an ADR of 0001 to 0129 records.

ADRs 0001 to 0003 are three short decision records with no checks: the settings tree, the field layout and the parallel strategy. They are the conventions every later wave builds on: typed settings validated on `set`, flat structure-of-arrays fields with boundary faces at the tail of the face range, and deterministic chunked reductions so that a result is bit-reproducible for a given thread count. ADR 0003 also names the two later steps, distributed memory (F-100, on top of F-016) and GPU (F-101, "only linear solves and flux assembly behind a trait"); waves 22 (ADR 0038) and 29D (ADR 0058) are those steps. ADR 0004 records the desktop app and assistant crates and their compiler floor. See [[crate-graph]], [[bit-identical]].

## Current state

- **The three conventions stand.** Settings are "a schema plus data", stored as `serde_json::Value` and "*always* validated against the schema on `set`"; dimensioned floats in SI; access by dotted path; JSON Schema export derived from the same schema; content hash "FNV-1a over key-sorted JSON" (ADR 0001). Fields are flat `Vec<f64>` per cell or face id, vector fields `Vec<[f64; 3]>`, boundary faces "the tail of the face range (faces `n_internal..n_faces`), grouped by patch", meshes immutable and referenced by `Arc` (ADR 0002). Shared memory with `rayon` first; "reductions use a fixed chunking and fixed combine order so results are bit-reproducible for a given thread count and chunk size" (ADR 0003).
- **GPU (F-101) is partial:** device linear solvers for the pressure Poisson equation through CUDA (`cudarc`), crate `rcfd-gpu`, feature `cuda` off by default, `device = cpu` bit-identical; all `unsafe` in one module, `launch.rs`, while the workspace keeps `unsafe_code = "forbid"` elsewhere (ADR 0038). See [[rcfd-gpu]], [[doc-gpu]].
- **Distributed memory (F-100) is a library only:** `rcfd-dist` with a communicator trait whose only required primitive is point-to-point, halo exchange, distributed CSR, `cg` and `bicgstab`, Jacobi and block-Jacobi; only `SerialComm` and the in-process `ThreadComm`; "MPI and socket transports are not delivered, and the roadmap F-100 stays `todo` for the cluster feature"; `rcfd run` not wired (ADR 0058). See [[rcfd-dist]].
- **The desktop crates of ADR 0004 are gone.** `rcfd-gui` and `rcfd-assistant` (egui 0.27.2, ureq 3, MSRV 1.88 against the workspace 1.75) were removed later (F-449, F-838), and the Electron [[studio]] replaced them ([[crate-graph]], [[doc-architecture]]).

## ADRs

| ADR | Date | Kind | Item | Question | Outcome | Superseded by |
|---|---|---|---|---|---|---|
| [0001](../../docs/adr/0001-settings-state-tree.md) | (none) | decision | | One typed, introspectable, serialisable, validated description of case settings | accepted: schema plus data, validated on `set`, FNV-1a hash | |
| [0002](../../docs/adr/0002-field-storage.md) | (none) | decision | | Memory layout of fields | accepted: flat SoA `Vec<f64>`, boundary faces at the tail, ghost cells after owned cells | |
| [0003](../../docs/adr/0003-parallel-strategy.md) | (none) | decision | F-100, F-101, F-016 | Shared memory, distributed memory, GPU | accepted: `rayon` first with fixed chunking; MPI later; GPU for linear solves and flux assembly behind a trait | 0038, 0058 (carried out) |
| [0004](../../docs/adr/0004-gui-assistant-msrv.md) | (none) | decision | F-140 | GUI stack, HTTP client, key storage and MSRV of `rcfd-gui` / `rcfd-assistant` | accepted: eframe/egui 0.27.2, ureq 3, `rust-version = "1.88"` for the two crates, solver crates keep 1.75 | crates removed (F-449, F-838) |
| [0038](../../docs/adr/0038-wave22-gpu-cuda.md) | (none) | registration + results | F-100, F-101 | Device linear solvers for the pressure equation through CUDA | met (every check inside its limit); F-101 partial | |
| [0058](../../docs/adr/0058-wave29d-distributed-parallel.md) | (none) | registration + results | F-100, F-848 | Transport-agnostic distributed-memory layer | met at first run, no known miss; F-100 acceptance not tested | |

## Crates created by an ADR of 0001 to 0129

| Crate | ADR | Wave | What it started as |
|---|---|---|---|
| `rcfd-gui`, `rcfd-assistant` | 0004 | | egui desktop app and provider-agnostic assistant library (removed later) |
| [[rcfd-reactor]] | [0013](../../docs/adr/0013-reactor-and-fwh.md) | | 0D reactor toolkit (F-112), "std only plus serde_json"; CHEMKIN reader |
| [[rcfd-particles]] | [0028](../../docs/adr/0028-wave18c-particles.md) | 18C | one-way Lagrangian tracking (F-056), layer 2, depends on `rcfd-core` and `rcfd-mesh` only |
| [[rcfd-gpu]] | [0038](../../docs/adr/0038-wave22-gpu-cuda.md) | 22 | CUDA device solvers (F-101) |
| [[rcfd-dist]] | [0058](../../docs/adr/0058-wave29d-distributed-parallel.md) | 29D | distributed-memory layer (F-100), layer 2, depends on `rcfd-core`, `rcfd-mesh`, `rcfd-linalg`, `rayon` |
| [[rcfd-project]] | [0122](../../docs/adr/0122-wave47a-project-folder-library.md) | 47A | Studio project folder, layer L1 |
| [[rcfd-runner]] | [0123](../../docs/adr/0123-wave47b-run-service.md) | 47B | run service, layer L2 |
| [[rcfd-tsstore]] | [0124](../../docs/adr/0124-wave47c-time-series-field-store.md) | 47C | time-series field store, layer L3 |
| `rcfd-render` | [0125](../../docs/adr/0125-wave47d-wgpu-render-core-v0.md) | 47D | offscreen wgpu render core, layer L4 (removed later, F-838) |
| `rcfd-claude` | [0126](../../docs/adr/0126-wave47e-claude-code-bridge-library.md) | 47E | Claude Code bridge, layer L6 (removed 2026-11-17) |

No ADR in this range records the creation of `rcfd-core`, `rcfd-mesh`, `rcfd-linalg`, `rcfd-fv`, `rcfd-turb`, `rcfd-io`, `rcfd-post`, `rcfd-api` or `rcfd-cli`; ADRs 0001 to 0003 fix the conventions they use (settings in `rcfd_core::settings`, `rcfd_core::Scalar`). The Studio crates are on [[adr-thread-studio-foundation-crates]].

## Key numbers and limits

- ADR 0004: workspace MSRV 1.75 "not verified on a 1.75 toolchain; development used rustc 1.99"; the 1.88 floor "comes from declared metadata". eframe `persistence` not enabled because "it serialises app memory to disk, which could leak keys".
- ADR 0038 F1: at 171^3 "the GPU one-shot solve takes 0.2009 s against 1.9446 s for one CPU thread in the benchmark (9.7x), and 0.187 s against 2.839 s (15.2x) in the ignored test"; "The GPU is slower than one CPU thread at 4096 unknowns (0.22x one-shot)". Not delivered: device-resident FV assembly, AMG / ILU / GMRES on the device, multi-GPU and MPI, non-NVIDIA vendors ("wgpu was not chosen").
- ADR 0058: the matrix-vector product "is bit-identical to the serial one at P = 1, 2, 4, 8"; strong scaling reported, never asserted: "parallel efficiency is 0.56, 0.38 and 0.20 at P = 2, 4 and 8 on 36864 cells"; the F-100 acceptance "(70 percent efficiency to 16 ranks on 4M cells) is not tested and not claimed".

## Open questions

- ADR 0058: "a converged distributed momentum/pressure (SIMPLE) laminar flow field was NOT attempted"; MPI transport not delivered.
- ADR 0038: the run-header device print of D6 "is NOT implemented".

## Related

- [[crate-graph]], [[rcfd-core]], [[rcfd-fv]], [[rcfd-gpu]], [[rcfd-dist]], [[rcfd-particles]], [[rcfd-reactor]], [[bit-identical]]
- [[adr-thread-studio-foundation-crates]]
- [[roadmap-parallel-and-gpu]], [[roadmap-studio-desktop-app]]
