---
type: entity
tags: [crate]
sources: [doc-architecture, adr-thread-founding-decisions]
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
- **Created by ADR 0058 (wave 29D, added by p3-records-a):** a layer-2 crate with "a communicator trait whose only required transport primitive is point-to-point, halo exchange, a distributed CSR matrix, distributed `cg` and `bicgstab`, and point Jacobi and block-Jacobi preconditioners"; only `SerialComm` and the in-process `ThreadComm`; "MPI and socket transports are not delivered", `rcfd run` is not wired; parallel efficiency "0.56, 0.38 and 0.20 at P = 2, 4 and 8 on 36864 cells" (reported, not asserted) ([[adr-thread-founding-decisions]]).

## Open questions

_Filled in by P1-P3 ingests._
