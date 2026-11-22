---
type: entity
tags: [crate]
sources: [doc-architecture]
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

## Open questions

_Filled in by P1-P3 ingests._
