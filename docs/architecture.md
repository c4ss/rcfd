# Architecture and crate graph

Workspace members (`crates/`), arrows mean "depends on":

```
rcfd-core   (no rcfd deps: scalar, units, errors, log, settings tree, hash)
rcfd-mesh   -> core
rcfd-linalg -> core
rcfd-reactor -> (no workspace crates; 0D reactor toolkit, stiff ODE integrator)
rcfd-gpu    -> linalg   (optional CUDA device solvers, cargo feature cuda; the only crate with unsafe code, ADR 0038)
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
```

Rules: lower layers never depend on higher ones (`core` < `mesh`/`linalg` <
`gpu` < `io`/`fv`/`particles` < `turb`/`post` < `api` < `mcp`/`cli` < `assistant` < `gui`). The graph is checked for
acyclicity and conformance by `crates/rcfd-cli/tests/workspace_graph.rs`.
