---
name: start-and-watch
description: Check an rcfd case of this project, start a run of it with the project's run service and watch it until it ends, reporting residuals and the final state. Use when the user asks to run a case and see how it goes.
argument-hint: <case path, e.g. cases/cavity.toml> [setting=value ...]
allowed-tools: mcp__plugin_rcfd_project__read_project_file, mcp__plugin_rcfd_project__edit_case, mcp__plugin_rcfd_project__start_run, mcp__plugin_rcfd_project__wait_run, mcp__plugin_rcfd_project__run_status, mcp__plugin_rcfd_project__read_residuals
---

# Start and watch a run

Arguments: $ARGUMENTS (a project-relative case path, then optional `setting=value` overrides).

1. `read_project_file` the case to get its `hash`.
2. Check it without writing: `edit_case` with `path`, `base_hash` = that hash, `dry_run: true`, and `set` holding the
   overrides as `{setting: value}` (an empty `set` checks the file as it is). A refusal (error 1003) names the setting
   and the reason: report it and stop.
3. `start_run` with `case` and the overrides as `set: ["setting=value", ...]`. Note the run id.
4. `wait_run` with the run id (at most 60 s per call) until `terminal` is true. Between calls, `read_residuals` with
   `max_points` 50 and report how the residuals fall.
5. At the end `run_status`: report the final state, step and, for a failure, `status.reason`.

Do not edit the case file in this skill; the overrides go to the run only.
