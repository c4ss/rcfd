---
type: source
tags: [architecture, crate]
sources: []
origin: docs/architecture.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Architecture and crate graph

**Origin:** [docs/architecture.md](../../docs/architecture.md) at `origin/main` 2e94e5ef
**Kind:** how-and-why page

## Summary

The workspace crate dependency graph and its layering rules, for contributors adding or moving code.

## Key points

- 19 workspace crates under `crates/`; see [[crate-graph]] for the full table.
- Layering: `core` < `mesh`/`linalg`/`project` < `gpu`/`dist`/`runner`/`fem` < `io`/`fv`/`particles` < `turb`/`post`/`tsstore` < `api` < `workspace` < `mcp`/`cli`; checked by `crates/rcfd-cli/tests/workspace_graph.rs`.
- [[rcfd-gpu]] is the only crate with unsafe code (ADR 0038).
- The Electron [[studio]] (`apps/studio-web/`, ADR 0800) is outside the workspace and depends on no crate; it runs the `rcfd`, `rcfd-runner`, `rcfd-mcp` binaries.
- Removed: `rcfd-gui`, `rcfd-studio` (F-449); `rcfd-assistant`, `rcfd-render`, `rcfd-claude` (F-838, ADR 0800 §10; `rcfd-claude` gone 2026-11-17). The Studio ports the Claude Code bridge in `apps/studio-web/src/main/claude/`.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[crate-graph]]
- [[studio]]
- [[rcfd-core]]
- [[rcfd-mesh]]
- [[rcfd-linalg]]
- [[rcfd-reactor]]
- [[rcfd-project]]
- [[rcfd-gpu]]
- [[rcfd-dist]]
- [[rcfd-fem]]
- [[rcfd-io]]
- [[rcfd-fv]]
- [[rcfd-particles]]
- [[rcfd-turb]]
- [[rcfd-post]]
- [[rcfd-tsstore]]
- [[rcfd-api]]
- [[rcfd-workspace]]
- [[rcfd-mcp]]
- [[rcfd-cli]]
- [[rcfd-runner]]
