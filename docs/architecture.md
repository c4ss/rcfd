# Architecture and crate graph

This page shows how the workspace crates depend on each other and which layering rules a new crate or a new dependency must
follow. It is for contributors who add or move code between crates. For how to contribute see CONTRIBUTING.md at the
repository root, and for the reasons behind a single crate read the ADR named next to it, where one is named.

## Crate dependency graph

Workspace members (`crates/`), arrows mean "depends on":

```text
rcfd-core   (no rcfd deps: scalar, units, errors, log, settings tree, hash)
rcfd-mesh   -> core
rcfd-linalg -> core
rcfd-reactor -> (no workspace crates; 0D reactor toolkit, stiff ODE integrator)
rcfd-project -> core   (Studio L1 project folder: run records, atomic files, path safety, poll watcher; ADR 0122)
rcfd-gpu    -> linalg   (optional CUDA device solvers, cargo feature cuda; the only crate with unsafe code, ADR 0038)
rcfd-dist   -> core, mesh, linalg   (transport-agnostic distributed-memory layer: communicator, halo exchange, distributed matrix and Krylov solvers; in-process threads transport, no MPI, ADR 0058)
rcfd-io     -> core, mesh
rcfd-fv     -> core, mesh, linalg, gpu
rcfd-particles -> core, mesh   (one-way Lagrangian particle tracking, ADR 0028)
rcfd-turb   -> core, mesh, fv
rcfd-post   -> core, mesh
rcfd-tsstore -> core, mesh, io   (time-series field store of a run, Studio L3, ADR 0124)
rcfd-api    -> core, fv, io, particles, tsstore (and the crates above)
rcfd-mcp    -> api, project, runner, tsstore, post   (project tools behind --project: folder, run service, stored fields, CPU render; ADR 0135)
rcfd-cli    -> core, mesh, io, post, fv, api
rcfd-assistant -> api, mcp   (provider adapters, agent loop, setup tools; no UI)
rcfd-gui    -> api, mcp, assistant   (legacy eframe/egui desktop app on glow (OpenGL), frozen (docs/ui-restart-plan.md); egui 0.36 as the Studio, ADR 0165; needs Rust 1.95)
rcfd-runner -> project   (Studio L2 run service: runs the `rcfd` binary as child processes, ADR 0123; on rcfd-project's records, ADR 0130)
rcfd-render -> core, mesh, post   (Studio L4: wgpu offscreen render core, ADR 0125)
rcfd-claude -> project   (Studio L6: Claude Code bridge, ADR 0126; atomic allow-list file, ADR 0134)
rcfd-studio -> project, runner, render, tsstore, mesh, claude, api, core   (Studio L5: eframe/egui on wgpu, ADR 0131; run control through the runner's client, ADR 0133; the viewport renders a run's stored fields with render on eframe's device, ADR 0132; the Claude Code sidebar, ADR 0134; needs Rust 1.95)
```

## Layering rules

Rules: lower layers never depend on higher ones (`core` < `mesh`/`linalg`/`project` <
`gpu`/`dist`/`runner`/`claude` < `io`/`fv`/`particles` < `turb`/`post`/`tsstore` < `api`/`render` < `mcp`/`cli` < `assistant` < `gui` < `studio`).
No crate depends on `studio`, and `studio` does not depend on `gui`, `assistant` or `mcp`. The graph is checked for
acyclicity and conformance by `crates/rcfd-cli/tests/workspace_graph.rs`.

## Where to go next

* [index.md](index.md): the list of all documentation pages.
* [gpu.md](gpu.md) and [particles.md](particles.md): the two optional or one-way crates in more detail.
