---
type: concept
tags: [interface, mcp, assistant]
sources: [doc-mcp-tools, doc-tutorial-mcp-design-loop, doc-tutorial-desktop-assistant, adr-thread-mcp-tools, adr-thread-studio-engine-entry-points, adr-thread-workspace-layer]
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

### From the P3 ADR threads (2026-11-22)

- `delete` was added by ADR 0736 (Q3 (a)) as "a new tool class `delete`, which no allow-list holds by default, so Claude Code asks"; the allow-list rules also refuse a rule naming a whole MCP server (`mcp__<server>`), "which would allow its delete tools too" ([[adr-thread-studio-engine-entry-points]]).
- The design tools' move to `execute` under `--project` was the maintainer's answer to ADR 0195 Q6 on 2026-11-06 ("overturned" the registered (a)); "the other half of option (b), `start_run` with `new_case` also checked as `write_files`, was not part of the answer" ([[adr-thread-workspace-layer]]).
- `start_run {site}` adds a site-level gate on top of the class: an MCP client may name only a site with `assistant = true` (ADR 0739 Q6) ([[adr-thread-runner-records-and-run-sites]]).
- After the Studio's allow-list was removed (ADR 0801 Addendum 23), its tests were "restated on the engine's own access classes and annotations (the 17 read-class tools, read-only hints, and the delete class being `empty_trash` alone)" ([[adr-thread-mcp-tools]]).

## Related

- [[mcp]], [[rcfd-mcp]], [[studio]]
