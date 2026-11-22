---
type: entity
tags: [crate]
sources: [doc-architecture, doc-mcp-tools, doc-tutorial-mcp-design-loop, doc-tutorial-desktop-assistant, doc-run-control]
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
- Tool reference [[doc-mcp-tools]]: 34 session tools over the solver service; `--project <dir>` drops 16 session tools and `create_project` and adds 25 project tools (42 listed); refuses to start (exit 2) without `rcfd-project.json` ([[doc-mcp-tools]]).
- Options: `--root` (sandbox, 1004 outside), `--read-only` (1100), `--max-cells`, `--max-steps`, `--max-run-seconds`, `--max-design-evaluations`, `--client <name>` (journal actor), `--edit-hook` ([[doc-mcp-tools]], [[doc-tutorial-mcp-design-loop]]).
- Every tool has an [[mcp-access-class]]; results stay under 36 000 bytes in project mode ([[doc-mcp-tools]]).
- The run tools need `rcfd-runner` and `rcfd` in the same folder as `rcfd-mcp`; on Windows a runner it starts inherits its stdio pipes (ADR 0146 C2) ([[doc-mcp-tools]], [[doc-run-control]]).
- The Claude Code plugin in `clients/claude-plugin` starts `rcfd-mcp --project ${CLAUDE_PROJECT_DIR} --root ${CLAUDE_PROJECT_DIR}` and runs `--edit-hook` before Claude Code's edit tools, refusing writes under `cases/`, `runs/`, `designs/`; the hook fails open ([[doc-mcp-tools]]).
- Changes are recorded in the [[project-journal]] ([[doc-mcp-tools]]).

## Open questions

_Filled in by P1-P3 ingests._
