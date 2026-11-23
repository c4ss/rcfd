---
type: entity
tags: [crate, architecture]
sources: [doc-architecture]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Crate graph

**What it is:** the dependency graph of the 19 workspace crates under `crates/` and its layering rule ([[doc-architecture]]).
**Where:** `docs/architecture.md`; checked by `crates/rcfd-cli/tests/workspace_graph.rs` (acyclicity and conformance).

## Layering rule

Lower layers never depend on higher ones:

`core` < `mesh`/`linalg`/`project` < `gpu`/`dist`/`runner`/`fem` < `io`/`fv`/`particles` < `turb`/`post`/`tsstore` < `api` < `workspace` < `mcp`/`cli` ([[doc-architecture]]).

No new third-party crate and no unsafe code outside [[rcfd-gpu]]; the graph stays acyclic ([[doc-process-agent-rules]], rule 5).

## Crates by layer

| Layer | Crate | Role |
|---|---|---|
| 0 | [[rcfd-core]] | Base crate with no rcfd dependencies: scalar type, units, errors, logging, the settings tree, hashing |
| 1 | [[rcfd-linalg]] | Linear algebra and linear solvers (incl |
| 1 | [[rcfd-mesh]] | Mesh data structures |
| 1 | [[rcfd-project]] | Studio layer L1, the project folder: run records, atomic files, path safety, poll watcher |
| 2 | [[rcfd-dist]] | Transport-agnostic distributed-memory layer: communicator, halo exchange, distributed matrix and Krylov solvers |
| 2 | [[rcfd-fem]] | Finite-element structural solver: the solid data model and structured builders |
| 2 | [[rcfd-gpu]] | Optional CUDA device solvers behind cargo feature `cuda` |
| 2 | [[rcfd-runner]] | Studio layer L2, the run service: runs the `rcfd` binary as child processes and owns every run of a project |
| 3 | [[rcfd-fv]] | The finite-volume core: discretisation, gradients and limiters, SIMPLE/PISO/PIMPLE kernels |
| 3 | [[rcfd-io]] | Mesh and field file IO |
| 3 | [[rcfd-particles]] | One-way coupled Lagrangian particle tracking |
| 4 | [[rcfd-post]] | Post-processing and VTK output, including VTK output of a solid result and the solid glue modules |
| 4 | [[rcfd-tsstore]] | Studio layer L3, the time-series field store of a run (playback of stored fields) |
| 4 | [[rcfd-turb]] | Turbulence models |
| 5 | [[rcfd-api]] | The library API over the solver crates |
| 6 | [[rcfd-workspace]] | The project folder's commands, queries and single write rule, shared by the MCP server and the Studio; the project journal and change feed |
| 7 | [[rcfd-cli]] | The `rcfd` command-line binary (`rcfd check`, `rcfd run`, `rcfd project init`,  |
| 7 | [[rcfd-mcp]] | The MCP server: project tools behind `--project`, as MCP descriptors over the workspace layer |
| — | [[rcfd-reactor]] | 0D reactor toolkit with a stiff ODE integrator |

## Outside the workspace

- The [[studio]] (`apps/studio-web/`, Electron) runs the `rcfd`, `rcfd-runner` and `rcfd-mcp` binaries and depends on no crate ([[doc-architecture]]).
- Removed crates (git history keeps them): `rcfd-gui`, `rcfd-studio` (F-449); `rcfd-assistant`, `rcfd-render` (F-838); `rcfd-claude` (2026-11-17; its allow-list rules moved into `crates/rcfd-mcp/tests/claude_allow/`) ([[doc-architecture]]).
