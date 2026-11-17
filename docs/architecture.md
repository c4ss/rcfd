# Architecture and crate graph

This page shows how the workspace crates depend on each other and which layering rules a new crate or a new dependency must
follow. Lower layers never depend on higher ones. The page is for contributors who add or move code between crates. For how
to contribute see CONTRIBUTING.md at the repository root. For the reasons behind a single crate read the ADR named next to
it, where one is named.

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
rcfd-fem    -> core, mesh, linalg   (finite-element structural solver: the solid data model and the structured builders, ADR 0271; plan ADR 0270)
rcfd-io     -> core, mesh
rcfd-fv     -> core, mesh, linalg, gpu
rcfd-particles -> core, mesh   (one-way Lagrangian particle tracking, ADR 0028)
rcfd-turb   -> core, mesh, fv
rcfd-post   -> core, mesh, io, fv, fem   (fem: the VTK output of a solid result and the solid glue modules, ADR 0278)
rcfd-tsstore -> core, mesh, io   (time-series field store of a run, Studio L3, ADR 0124)
rcfd-api    -> core, fv, io, particles, tsstore (and the crates above)
rcfd-workspace -> api, project, runner, tsstore, post, io, mesh   (the project folder's commands, queries and write rule, shared by the MCP server and the Studio; ADR 0173, ADR 0185; io and mesh for the mesh commands, ADR 0187; the project journal's entries and the change feed, ADR 0188)
rcfd-mcp    -> api, workspace, project, runner, tsstore, post   (project tools behind --project: their MCP descriptors over the workspace; ADR 0135, ADR 0185)
rcfd-cli    -> core, mesh, io, post, fv, api, workspace   (workspace for `rcfd project init`, ADR 0731)
rcfd-runner -> project   (Studio L2 run service: runs the `rcfd` binary as child processes, ADR 0123; on rcfd-project's records, ADR 0130)
```

The desktop Studio is the Electron app in `apps/studio-web/` (ADR 0800), outside the Cargo workspace: it runs the `rcfd`,
`rcfd-runner` and `rcfd-mcp` binaries and depends on no crate. The two egui desktop apps that were workspace crates,
`rcfd-gui` and `rcfd-studio`, were removed by roadmap item F-449 (the git history keeps them). The crates they left without
users went with F-838 (ADR 0800 §10): `rcfd-assistant` (the local-model assistant of `rcfd-gui`), `rcfd-render` (the wgpu
render core of `rcfd-studio`) and the Claude Code bridge part of `rcfd-claude` (session discovery, transcript and stream
readers, chat driver, folder trust). The rest of `rcfd-claude`, the allow-list rules that only `rcfd-mcp`'s tests used, moved
into those tests (`crates/rcfd-mcp/tests/claude_allow/`) and the crate was removed (2026-11-17). The Studio has its own port
of that bridge and of the allow-list rules in `apps/studio-web/src/main/claude/`.

## Layering rules

Lower layers never depend on higher ones (`core` < `mesh`/`linalg`/`project` < `gpu`/`dist`/`runner`/`fem` <
`io`/`fv`/`particles` < `turb`/`post`/`tsstore` < `api` < `workspace` < `mcp`/`cli`). The test
`crates/rcfd-cli/tests/workspace_graph.rs` checks the graph for acyclicity and conformance.

## Where to go next

* [index.md](index.md): the list of all documentation pages.
* [gpu.md](gpu.md) and [particles.md](particles.md): the two optional or one-way crates in more detail.
