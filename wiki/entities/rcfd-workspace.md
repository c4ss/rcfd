---
type: entity
tags: [crate]
sources: [doc-architecture, doc-roadmap-overview]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-workspace

**What it is:** The project folder's commands, queries and single write rule, shared by the MCP server and the Studio; the project journal and change feed. ([[doc-architecture]])
**Where:** `crates/rcfd-workspace/` — layer 6 (workspace) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-api]], [[rcfd-project]], [[rcfd-runner]], [[rcfd-tsstore]], [[rcfd-post]], [[rcfd-io]], [[rcfd-mesh]]
- Used by: [[rcfd-mcp]], [[rcfd-cli]]

## Details

- Decision records: ADR 0173, ADR 0185, ADR 0187, ADR 0188 ([[doc-architecture]]).
- The 'one workspace layer' priority of 2026-11-04 (F-270 to F-275) ([[doc-roadmap-overview]]).

## Open questions

_Filled in by P1-P3 ingests._
