# rcfd documentation

rcfd is a clean-room, open source (AGPL-3.0) finite-volume CFD suite in Rust: a command-line solver, a solver service, an
MCP server for AI assistants and a desktop app with an assistant sidebar.

This page is the map of the documentation, grouped by what you want to do. rcfd is early-stage software and has not been validated
for engineering decisions: read the [roadmap overview](roadmap-overview.md) first to see what works today and what does not. If you
are new, then work through the first tutorial. To build from source or to contribute, see the README and the contribution guide in
the repository root.

## Learn by doing: tutorials

The tutorials are executed by the test suite (`cargo test -p rcfd-cli --test tutorials` and
`cargo test -p rcfd-mcp --test tutorials`); steps that cannot run headlessly are marked as not executed on each page.

1. [Quick start: the command line](tutorials/quick-start-cli.md)
2. [Run a lid-driven cavity and compare it with published data](tutorials/cavity.md)
3. [Example gallery](tutorials/gallery.md)
4. [Drive rcfd from an assistant: the MCP design loop](tutorials/mcp-design-loop.md)
5. [The desktop app and the assistant sidebar](tutorials/desktop-assistant.md)

## Do a task

* [Fan measurement protocol](fan-measurement-protocol.md): what to measure on a real fan so the fan models can be checked against it.

Most task steps live in the reference pages below for now.

## Look something up

* [Glossary](glossary.md): the project's own terms (wave, gate, ADR, F-xxx, PARTIAL, UNVERIFIED, FAN-01 and others).
* [Case file schema](case-schema.md): every setting of a case file.
* [Run control](run-control.md): exit codes, status stream, checkpoints, stop files.
* [Studies](studies.md): parametric studies over design points and their result table.
* [MCP tools](mcp-tools.md) and [the service protocol](service-protocol.md): the tools the assistant can call, their access classes,
  and the JSON-RPC protocol of the solver service underneath.
* [Interchange formats](interchange-formats.md): file conversion, registers and GLB validation, and the input limits every parser
  enforces.
* [Python](python.md): drive the command line from a Python script (there is no wheel yet).
* [UDF ABI](udf-abi.md): load your own code into a case as a sandboxed WebAssembly module.
* [Mesh import](mesh-import.md): read a mesh written by another tool.
* [Parity matrix](parity-matrix.md): commercial CFD capabilities set against the roadmap.

## Understand how and why

* [Architecture](architecture.md): the crate graph and the allowed dependency directions.
* [Meshing](meshing.md): how the mesh generators and mesh tools build their meshes, and the quality measured; several are
  library-only.
* [Mesh quality](mesh-quality.md): check a mesh, read its quality report and edit the mesh.
* [Dynamic meshes](dynamic-mesh.md): moving meshes and rigid-body dynamics (partial).
* [Particles](particles.md): one-way coupled Lagrangian particle tracking (partial).
* [Acoustics](acoustics.md): the fan-noise chain (FW-H solver, tonal estimate, broadband proxies, noise metrics).
* [Reactor and acoustics](reactor-and-acoustics.md): the reactor and FW-H acoustics commands, their inputs and outputs.
* [GPU](gpu.md): build and run the optional GPU backend for the linear solvers.
* [The design loop](design-loop.md): operating maps, DOE and optimisation, and the assistant tools that drive them.
* Findings: [backward-step convergence](bfs-convergence-findings.md), [component-diagonal default](component-diagonal-default.md).
* The decision records in `docs/adr/` (for example
  [ADR 0089, this documentation](adr/0089-wave38b-docs-site-tutorials-gallery.md)).

## Project status, plans and reports

* [Roadmap overview](roadmap-overview.md): the short status, what works, what does not and what comes next.
* [Detailed roadmap](ROADMAP.md): every item with its acceptance criteria and status.
* [Fan milestone](milestone-fan.md): the assistant-driven fan design goal and its current state.
* [Fan project breakdown](fan-project-breakdown.md): the plan for the computer-fan target, split into parts, with its limits.
* [Studio plan](ui-restart-plan.md): the plan for the new desktop application.
* [Validation data](validation-data.md): the open benchmark data pack, with fetch scripts, checksums and licence records.
* [Provenance record](provenance.md): where rcfd's knowledge comes from, and every recorded case in which vendor documentation was consulted.
* [Reference data](reference-data.md): benchmark values and published constants checked against public sources.
* [Verification report](verification-report.md): reference values that were marked unverified, re-checked against their sources.
* [Product suites](product-suites.md): a log of capabilities outside CFD; not a plan.
* [Audit report](audit-report.md): an independent model-run audit of the repository, not reviewed by a human.
* [Test process](test-process.md): what the gate runs, how long it takes, and the options to make it faster.
* [Releasing](releasing.md): what a version number means, what each automatic check proves, and the steps to cut a release.

## About these pages

The HTML version of these pages is built by `cargo test -p rcfd-cli --test docs_site -- --nocapture` (it prints where the site was
written).
