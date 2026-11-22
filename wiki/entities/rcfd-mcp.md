---
type: entity
tags: [crate]
sources: [doc-architecture]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-mcp

**What it is:** The MCP server: project tools behind `--project`, as MCP descriptors over the workspace layer. ([[doc-architecture]])
**Where:** `crates/rcfd-mcp/` — layer 7 (mcp / cli) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-api]], [[rcfd-workspace]], [[rcfd-project]], [[rcfd-runner]], [[rcfd-tsstore]], [[rcfd-post]]
- Used by: no workspace crate (binary / top of graph)

## Details

- Decision records: ADR 0135, ADR 0185 ([[doc-architecture]]).

## Open questions

_Filled in by P1-P3 ingests._
