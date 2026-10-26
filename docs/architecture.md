# Architecture and crate graph

Workspace members (`crates/`), arrows mean "depends on":

```
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
rcfd-api    -> core, fv, io, particles (and the crates above)
rcfd-mcp    -> api
rcfd-cli    -> core, mesh, io, post, fv, api
rcfd-assistant -> api, mcp   (provider adapters, agent loop, setup tools; no UI)
rcfd-gui    -> api, mcp, assistant   (eframe/egui desktop app)
rcfd-runner -> (no workspace crates; run service: runs the `rcfd` binary as child processes, ADR 0123)
rcfd-render -> core, mesh, post   (Studio L4: wgpu offscreen render core, ADR 0125)
```

Rules: lower layers never depend on higher ones (`core` < `mesh`/`linalg`/`project` <
`gpu`/`dist` < `io`/`fv`/`particles` < `turb`/`post` < `api`/`render` < `mcp`/`cli` < `assistant` < `gui`). The graph is checked for
acyclicity and conformance by `crates/rcfd-cli/tests/workspace_graph.rs`.
