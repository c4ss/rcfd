---
type: concept
tags: [interface, mcp, assistant]
sources: [doc-mcp-tools, doc-tutorial-mcp-design-loop, doc-tutorial-desktop-assistant]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# MCP tool access class

Every tool of [[rcfd-mcp]] has an access class: `read` tools never change state, `mutate` tools change the session, `execute` tools start computation, `write_files` tools write files inside the sandbox ([[doc-mcp-tools]]); `delete` is the class of `empty_trash` alone (ADR 0736) ([[doc-mcp-tools]]).

## How rcfd uses it

- `tools/list` marks each tool with `annotations.readOnlyHint` and `_meta."rcfd/access"`; `_meta."rcfd/needsApproval"` is true for every class except `read` ([[doc-mcp-tools]], [[doc-tutorial-mcp-design-loop]]).
- Policy on top: read-only mode (`--read-only`) refuses every non-`read` tool with error 1100 `permission denied`; an approval hook is asked before every non-`read` call; an allow list restricts the callable tools ([[doc-mcp-tools]]).
- `empty_trash` has `destructiveHint` true and "no allow-list holds it, by default or by a rule added later" ([[doc-mcp-tools]]).
- The class can depend on the mode: `design_run_study`, `design_evaluate`, `design_run_operating_map` are `write_files` without `--project` and `execute` with it, because they start runner runs (ADR 0196 B, ADR 0200 S9) ([[doc-mcp-tools]]).
- In the Studio flow, Claude Code's own permission prompts govern each call; the Studio keeps no allow-list (ADR 0801 Addendum 23) ([[doc-mcp-tools]], [[doc-tutorial-desktop-assistant]]).

## Related

- [[mcp]], [[rcfd-mcp]], [[studio]]
