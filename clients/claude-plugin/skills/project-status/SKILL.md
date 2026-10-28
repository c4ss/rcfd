---
name: project-status
description: Summarise the runs of the rcfd project in this folder - how many are queued, running, finished, failed or cancelled, why failed runs failed, and the latest metrics. Use when the user asks how the runs are going or why a run failed.
allowed-tools: mcp__plugin_rcfd_project__project_info, mcp__plugin_rcfd_project__list_runs, mcp__plugin_rcfd_project__run_status, mcp__plugin_rcfd_project__read_metrics
---

# Project status

1. Call `project_info` for the run counts by state and any problems `validate` found in the folder.
2. Call `list_runs` (page with `offset` while `next_offset` is set; filter with `state` when the user asks about one
   kind of run). Runs listed under `skipped` could not be read: report their path and message.
3. For every failed or cancelled run (at most the five newest), call `run_status` and report its `status.reason`,
   `status.exit_code` and the last events.
4. For finished runs the user cares about, call `read_metrics` with their ids.

Report a short table: run id, case, state, step, and for failures the reason in one line. Say which numbers come from
`metrics.json` (final report values). Do not start, pause or cancel anything in this skill.
