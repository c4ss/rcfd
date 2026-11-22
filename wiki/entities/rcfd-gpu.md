---
type: entity
tags: [crate]
sources: [doc-architecture]
created: 2026-11-22
updated: 2026-11-22
status: stub
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

## Open questions

_Filled in by P1-P3 ingests._
