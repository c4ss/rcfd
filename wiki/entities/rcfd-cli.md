---
type: entity
tags: [crate]
sources: [doc-architecture, doc-mesh-quality, doc-mesh-import, doc-acoustics, doc-reactor-and-acoustics, doc-cosimulation-protocol, doc-gpu, doc-run-control, doc-tutorial-quick-start-cli, doc-case-schema-decomposition-postprocessing, doc-service-protocol, doc-interchange-formats, doc-studies, doc-python]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-cli

**What it is:** The `rcfd` command-line binary (`rcfd check`, `rcfd run`, `rcfd project init`, ...). Its tests include the crate-graph check and the tutorials. ([[doc-architecture]])
**Where:** `crates/rcfd-cli/` — layer 7 (mcp / cli) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-io]], [[rcfd-post]], [[rcfd-fv]], [[rcfd-api]], [[rcfd-workspace]]
- Used by: no workspace crate (binary / top of graph)

## Details

- Decision records: ADR 0731 ([[doc-architecture]]).
- Commands named in the P1 interface docs: `run`, `restart`, `check`, `validate`, `schema` (`--ui`), `serve` (`--stdio`, `--addr`, `--ui`, `--journal-dir`, `--cosim`), `call`, `study`, `map`, `workflow run`, `sample`, `post`, `partition`, `inflate`, `convert`, `cases`, `check-mesh`, `reactor`, `acoustics`, `project init` / `hash` / `trash`, `site`, `help glossary` / `help errors` ([[doc-run-control]], [[doc-tutorial-quick-start-cli]], [[doc-case-schema-decomposition-postprocessing]], [[doc-service-protocol]], [[doc-interchange-formats]], [[doc-studies]]).
- `rcfd run` is the headless contract: [[exit-codes]], [[status-stream]], [[checkpoint]], [[stop-file]] ([[doc-run-control]]).
- `rcfd serve` hosts the solver service [[rcfd-service-protocol]] on loopback only by default ([[doc-service-protocol]]).
- `crates/rcfd-cli/tests/tutorials` runs the [[executable-tutorials]]; `python_w20c.rs` runs the [[rcfd-python]] tests ([[doc-tutorial-quick-start-cli]], [[doc-python]]).
- Numerics/physics subcommands named in the P1 docs: `rcfd check-mesh <mesh|case> [--json]` (exit 4 on quality errors), `rcfd inflate`, `rcfd partition` ([[doc-mesh-quality]]); `rcfd convert` ([[doc-mesh-import]]); `rcfd fwh` (`crates/rcfd-cli/src/cmd_fwh.rs`) ([[doc-acoustics]]); `rcfd acoustics` and `rcfd reactor` (exit 3 invalid input, 1 I/O, 2 usage) ([[doc-reactor-and-acoustics]]); `rcfd serve --cosim <case>` ([[doc-cosimulation-protocol]]).
- GPU support is built with `cargo build --release -p rcfd-cli --features cuda` ([[doc-gpu]]).

## Related

- [[rcfd-runner]] — starts `rcfd run` as child processes and owns the runs of a project (ADR 0123).
- [[rcfd-project]] — `rcfd project init <dir>` is one route to `Project::create` (ADR 0731).

## Open questions

_Filled in by P1-P3 ingests._
