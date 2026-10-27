# rcfd documentation

rcfd is a clean-room, open source (Apache-2.0 OR MIT) finite-volume CFD suite in Rust: a command-line solver, a solver service, an
MCP server for AI assistants and a desktop app with an assistant sidebar.

The HTML version of these pages is built by `cargo test -p rcfd-cli --test docs_site -- --nocapture` (it prints where the site was
written). The tutorials are executed by the test suite (`cargo test -p rcfd-cli --test tutorials` and
`cargo test -p rcfd-mcp --test tutorials`); steps that cannot run headlessly are marked as not executed on each page.

## Tutorials

1. [Quick start: the command line](tutorials/quick-start-cli.md)
2. [Run a lid-driven cavity and compare it with published data](tutorials/cavity.md)
3. [Example gallery](tutorials/gallery.md)
4. [Drive rcfd from an assistant: the MCP design loop](tutorials/mcp-design-loop.md)
5. [The desktop app and the assistant sidebar](tutorials/desktop-assistant.md)

## Reference

* [Case file schema](case-schema.md): every setting of a case file.
* [Meshing](meshing.md), [mesh import](mesh-import.md), [mesh quality](mesh-quality.md), [dynamic meshes](dynamic-mesh.md).
* [Run control](run-control.md): exit codes, status stream, checkpoints, stop files.
* [Studies](studies.md) and [the design loop](design-loop.md): parametric studies, operating maps, DOE and optimisation.
* [MCP tools](mcp-tools.md) and [the service protocol](service-protocol.md).
* [Interchange formats](interchange-formats.md), [Python](python.md), [UDF ABI](udf-abi.md), [GPU](gpu.md).
* [Particles](particles.md), [acoustics](acoustics.md), [reactor and acoustics](reactor-and-acoustics.md).

## Validation and status

* [Verification report](verification-report.md), [validation data](validation-data.md), [reference data](reference-data.md),
  [fan measurement protocol](fan-measurement-protocol.md).
* [Fan milestone](milestone-fan.md): the assistant-driven fan design goal and its current state.
* [Roadmap overview](roadmap-overview.md), [detailed roadmap](ROADMAP.md), [parity matrix](parity-matrix.md), [product suites](product-suites.md), [audit report](audit-report.md).
* [Architecture](architecture.md) and the decision records in `docs/adr/` (for example
  [ADR 0089, this documentation](adr/0089-wave38b-docs-site-tutorials-gallery.md)).
* Findings: [backward-step convergence](bfs-convergence-findings.md), [component-diagonal default](component-diagonal-default.md).
