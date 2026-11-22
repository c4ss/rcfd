---
type: entity
tags: [claude-code, plugin, mcp]
sources: [doc-claude-plugin-skills, doc-ui-restart-plan, doc-security]
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

## Related

- [[mcp]]
- [[studio]]
- [[run]]
