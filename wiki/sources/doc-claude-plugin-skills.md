---
type: source
tags: [claude-code, plugin, mcp]
sources: []
origin: clients/claude-plugin/skills/*/SKILL.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Claude Code plugin skills (compare, project-status, start-and-watch)

**Origin:** [clients/claude-plugin/skills/](../../clients/claude-plugin/skills/) at `origin/main` 2e94e5ef (3 files)
**Kind:** Claude Code skill definitions

## Summary

The three skills of the `rcfd` Claude Code plugin (`clients/claude-plugin/`, plugin version 0.1.0, AGPL-3.0-only, "needs rcfd-mcp, rcfd-runner and rcfd on PATH" per its `plugin.json`). Each skill restricts itself to a few tools of the plugin's MCP server `mcp__plugin_rcfd_project__*`, the project tools of [[rcfd-mcp]]. They are layer L7 of the [[doc-ui-restart-plan]]. See [[claude-plugin]].

## Key points

- **compare** (`[run id ...]`): `compare_runs` with 2 to 10 run ids; the first is the baseline, `differences` are other minus baseline, "exactly as stored (no rounding)"; `read_residuals` with `max_points` 50 per run; `read_metrics` for a metric `compare_runs` lacks. Default: the two newest finished runs. If runs used different cases, say so: "their metrics are then not like for like".
- **project-status**: `project_info` for counts by state and `validate` problems; `list_runs` paged by `offset` / `next_offset`; runs under `skipped` reported with path and message; `run_status` for at most the five newest failed or cancelled runs (`status.reason`, `status.exit_code`, last events); `read_metrics` for finished runs. "Do not start, pause or cancel anything in this skill."
- **start-and-watch** (`<case path> [setting=value ...]`): `read_project_file` for the `hash`; `edit_case` with `base_hash` and `dry_run: true` to check overrides without writing (a refusal is error 1003, naming the setting); `start_run` with `set: ["setting=value", ...]`; `wait_run` (at most 60 s per call) until `terminal`, with `read_residuals` between calls; `run_status` at the end. "Do not edit the case file in this skill; the overrides go to the run only."

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[claude-plugin]]
- [[mcp]]
- [[rcfd-mcp]]
- [[run]]
