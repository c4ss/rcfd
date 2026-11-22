---
type: entity
tags: [crate]
sources: [doc-architecture]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-runner

**What it is:** Studio layer L2, the run service: runs the `rcfd` binary as child processes and owns every run of a project. `rcfd-runner ensure` starts it. ([[doc-architecture]])
**Where:** `crates/rcfd-runner/` — layer 2 (gpu / dist / runner / fem) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-project]]
- Used by: [[rcfd-api]], [[rcfd-workspace]], [[rcfd-mcp]]

## Details

- Decision records: ADR 0123, ADR 0130 ([[doc-architecture]]).

## Open questions

_Filled in by P1-P3 ingests._
