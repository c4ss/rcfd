---
type: entity
tags: [crate]
sources: [doc-architecture, doc-mcp-tools, doc-tutorial-mcp-design-loop, doc-tutorial-desktop-assistant, doc-run-control, adr-thread-workspace-layer, adr-thread-mcp-tools, adr-thread-studio-engine-entry-points, adr-thread-runner-records-and-run-sites]
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

> [!warning] Contradiction (within the repo docs)
> The "25 project tools (42 listed)" count is the prose of [[doc-mcp-tools]]; the same page's project-tool table names 36 tools, and ADR 0738 stated "27 project tools, 44 with `--project`" before the trash tools and `list_sites` were added. The table is the better count. See [[adr-thread-mcp-tools]] and [[mcp]].

- Options: `--root` (sandbox, 1004 outside), `--read-only` (1100), `--max-cells`, `--max-steps`, `--max-run-seconds`, `--max-design-evaluations`, `--client <name>` (journal actor), `--edit-hook` ([[doc-mcp-tools]], [[doc-tutorial-mcp-design-loop]]).
- Every tool has an [[mcp-access-class]]; results stay under 36 000 bytes in project mode ([[doc-mcp-tools]]).
- The run tools need `rcfd-runner` and `rcfd` in the same folder as `rcfd-mcp`; on Windows a runner it starts inherits its stdio pipes (ADR 0146 C2) ([[doc-mcp-tools]], [[doc-run-control]]).
- The Claude Code plugin in `clients/claude-plugin` starts `rcfd-mcp --project ${CLAUDE_PROJECT_DIR} --root ${CLAUDE_PROJECT_DIR}` and runs `--edit-hook` before Claude Code's edit tools, refusing writes under `cases/`, `runs/`, `designs/`; the hook fails open ([[doc-mcp-tools]]).
- Changes are recorded in the [[project-journal]] ([[doc-mcp-tools]]).

### From the P3 ADR threads (2026-11-22)

- Since F-270 (ADR 0185) the project tools are an adapter over [[rcfd-workspace]]: "`rcfd-mcp/src/project.rs` keeps the 17 descriptors byte-for-byte and is now an adapter around them (272 lines, from 1,286)" ([[adr-thread-workspace-layer]]).
- Project-mode tools added by the workspace series: `new_case` (F-271), `import_mesh`, `check_case_mesh`, `preview_mesh`, `list_meshes` (F-272), `read_feed` and `reason` on eight tools (F-273) ([[adr-thread-workspace-layer]]); later `create_project`, `runner_config`, `set_runner_config`, `hash_project_file`, the six trash tools, `case_facts`, `refine_case_mesh`, `derive_picture`, `list_sites` ([[adr-thread-mcp-tools]]).
- `start_run` applies the field-store plan by default since ADR 0741 (2026-11-19), `store: "none"` is the way back; it also takes `new_case`, `run_time`, `dry_run`, `continue_from`, `max_steps`, `from_run` and `site` ([[adr-thread-workspace-layer]], [[adr-thread-runner-records-and-run-sites]], [[adr-thread-studio-engine-entry-points]]).
- `--client <name>` (1 to 32 of `a-z0-9_-`, default `mcp`) was added for the Studio's activity line (ADR 0801 Addendum 23, A23-D1 (a)) ([[adr-thread-studio-electron]]).
- After a restart the server adopts runs of a design study with the same tags, case snapshot hash and entries instead of submitting them again (F-288, ADR 0201) ([[adr-thread-workspace-layer]]).
- The allow-list support module `crates/rcfd-mcp/tests/claude_allow/` (moved from `rcfd-claude` on 2026-11-17) was removed with the Studio panel the same day ([[adr-thread-studio-electron]]).

## Related

- [[roadmap-claude-and-the-mcp-server]] — the roadmap area of the MCP server (F-090, F-091) and the workspace series F-270 to F-275.
- [[rcfd-cli]] — the `rcfd` binary its run tools need in the same folder as `rcfd-mcp`.

## Open questions

_Filled in by P1-P3 ingests._
