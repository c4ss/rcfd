---
type: entity
tags: [crate]
sources: [doc-architecture]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-dist

**What it is:** Transport-agnostic distributed-memory layer: communicator, halo exchange, distributed matrix and Krylov solvers. In-process threads transport, no MPI. ([[doc-architecture]])
**Where:** `crates/rcfd-dist/` — layer 2 (gpu / dist / runner / fem) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-linalg]]
- Used by: no workspace crate (binary / top of graph)

## Details

- Decision records: ADR 0058 ([[doc-architecture]]).

## Open questions

_Filled in by P1-P3 ingests._
