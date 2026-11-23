---
type: entity
tags: [claude-code, plugin, mcp]
sources: [doc-claude-plugin-skills, doc-ui-restart-plan, doc-security, adr-thread-workspace-layer, adr-thread-studio-electron]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd Claude Code plugin

**What it is:** the Claude Code plugin `rcfd` that packages the project tools of [[rcfd-mcp]] (its MCP server appears as `mcp__plugin_rcfd_project__*`) with three skills; layer L7 of the Studio plan ([[doc-claude-plugin-skills]], [[doc-ui-restart-plan]]).
**Where:** [clients/claude-plugin/](../../clients/claude-plugin/) (`.claude-plugin/plugin.json`, `skills/`, `hooks/`).

## Details

- Planned on 2026-10-26 as "A plugin packaging an MCP server of project tools (extending `rcfd-mcp`), skills and slash commands; file watching so edits show in the app and conflicts are detected" ([[doc-ui-restart-plan]]).
- Skills: **compare** (2 to 10 runs, differences as stored, no rounding), **project-status** (counts, failures and their reasons, read-only), **start-and-watch** (dry-run check with `edit_case`, `start_run` with overrides, `wait_run` at most 60 s per call) ([[doc-claude-plugin-skills]]).
- Each skill lists its `allowed-tools`, so a skill reaches only the tools it needs ([[doc-claude-plugin-skills]]).
- Separately, the Studio registers the engine's project server with Claude Code for a folder (`claude mcp add|get|remove rcfd-project --scope local`); Claude Code's own permission settings decide what it may call without asking ([[doc-security]]).

### From the P3 ADR threads (2026-11-22)

- The edit hook is F-275 (ADR 0195, built on ADR 0173 D5 (b)): it refused a Write and an Edit under `cases/` "in a real Claude Code 2.1.319 session". Its gaps were accepted on 2026-11-06 "since the hook is a guide rail": it fails open when it cannot decide, writes by Bash or PowerShell are not guarded, and its behaviour under `bypassPermissions` and `--dangerously-skip-permissions` is NOT VERIFIED; on 2026-11-18 the maintainer asked that this be stated in the plugin's documentation ([[adr-thread-workspace-layer]]).
- Known misses that "were not asked and stand as recorded": the plugin installed from the marketplace, and `NotebookEdit` and `MultiEdit` in a real session (ADR 0195) ([[adr-thread-workspace-layer]]).
- The Studio's Claude Code bridge crate `rcfd-claude` was removed on 2026-11-17 (F-838 follow-up), and the Studio's embedded panel the same day (ADR 0801 Addendum 23) ([[adr-thread-studio-electron]]).
- Roadmap: F-267 "Claude Code plugin with project tools" is `todo` while the plugin exists in `clients/claude-plugin` ([[roadmap-claude-and-the-mcp-server]]).

## Related

- [[mcp]]
- [[studio]]
- [[run]]
