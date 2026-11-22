---
type: entity
tags: [crate]
sources: [doc-architecture, doc-roadmap-overview, doc-mcp-tools, doc-run-control, adr-thread-workspace-layer, adr-thread-studio-engine-entry-points, adr-thread-runner-records-and-run-sites]
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

### From the P3 ADR threads (2026-11-22)

- Why it exists (ADR 0173, 2026-11-04): in project mode the MCP server and the egui Studio reached the same folder "through separate code, with separate rules for writing a case"; Studio saves took no edit lock and left no journal line. D1 (a) chose a new crate "placed above `api` and `render` and below `mcp` and `cli`" over growing `rcfd-project` (which "breaks the layering rules") ([[adr-thread-workspace-layer]]).
- Built in six steps on 2026-11-05 and 2026-11-06: the crate and the MCP adapter (F-270), `new_case` and the shared start plan `plan.rs` (F-271), mesh commands in `meshes.rs` (F-272), the journal and `feed.rs` (F-273), runner evaluations for design studies (F-274), the project-mode tool list (F-275) ([[adr-thread-workspace-layer]]).
- `PLAN_FOR_PROJECT_CASES` is `true` since ADR 0741 (2026-11-19), an approved change of a default (ADR 0173 D7, ADR 0186 Q2 (a)) ([[adr-thread-workspace-layer]]).
- Later commands: `create_project` (`init.rs`, ADR 0731), `runner_config.rs` (ADR 0733), the trash commands (`trash.rs`, ADR 0736, appended to `COMMANDS` "after `wait_run` (25 to 31)"), `derive` (ADR 0738); `rcfd_workspace::filehash` re-exports the hash of ADR 0734 ([[adr-thread-studio-engine-entry-points]], [[adr-thread-runner-records-and-run-sites]]).
- Two behaviours F-270 found (ADR 0185): `replace_if_hash` recreates a missing parent folder; a writer waits for a held edit lock up to 2 s, and a lock left by a crashed writer is broken only at 30 s ([[adr-thread-workspace-layer]]).

## Open questions

_Filled in by P1-P3 ingests._
