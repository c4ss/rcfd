---
type: entity
tags: [crate]
sources: [doc-architecture, doc-roadmap-overview, doc-process-agent-rules]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-api

**What it is:** The API crate over the solver crates. Hosts the milestone validation tests (cylinder, FAN-01) and `tests/baseline_hashes.rs`. ([[doc-architecture]])
**Where:** `crates/rcfd-api/` — layer 5 (api) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-fv]], [[rcfd-io]], [[rcfd-particles]], [[rcfd-tsstore]], [[rcfd-project]], [[rcfd-runner]]
- Used by: [[rcfd-workspace]], [[rcfd-mcp]], [[rcfd-cli]]

## Details

- Decision records: ADR 0737 ([[doc-architecture]]).
- ADR 0737: case.save outside a project and the runner's memory rule.

## Open questions

_Filled in by P1-P3 ingests._
