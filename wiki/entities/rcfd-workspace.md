---
type: entity
tags: [crate]
sources: [doc-architecture, doc-roadmap-overview, doc-mcp-tools, doc-run-control]
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
- The MCP project tools "are the commands and queries of the project workspace, the crate `crates/rcfd-workspace` that the Studio uses too" (ADR 0173, 0185-0188, 0195, 0733) ([[doc-mcp-tools]]).
- Single write rule: compare-and-replace under a cooperative lock (`rcfd_project::atomic::replace_if_hash`, `.rcfd/locks/<hash of the path>.lock`); errors 1200 (changed since its hash was read), 1201 (lock held 2 s), 1202 (missing) ([[doc-mcp-tools]]).
- Project paths are project-relative and refuse `..`, absolute paths and links (1004) ([[doc-mcp-tools]]).
- The start plan `rcfd_workspace::plan` (ADR 0186 G4) stores fields unless the case sets its own store ([[doc-run-control]]).
- Journal and feed: [[project-journal]].

## Open questions

_Filled in by P1-P3 ingests._
