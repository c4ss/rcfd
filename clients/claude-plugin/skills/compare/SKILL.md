---
name: compare
description: Compare two or more runs of the rcfd project in this folder - their metrics side by side with differences, and how their residuals converged. Use when the user asks which run is better or what changed between runs.
argument-hint: [run id ...]
allowed-tools: mcp__plugin_rcfd_project__list_runs, mcp__plugin_rcfd_project__compare_runs, mcp__plugin_rcfd_project__read_residuals, mcp__plugin_rcfd_project__read_metrics
---

# Compare runs

Arguments: $ARGUMENTS (run ids; when none are given, use the two newest finished runs from `list_runs` with
`state: finished`).

1. `compare_runs` with the run ids (2 to 10). The first run is the baseline; `differences` are other minus baseline,
   exactly as stored (no rounding).
2. `read_residuals` for each run with `max_points` 50 to compare how fast the residuals fell; the `residual_lines` of
   `compare_runs` give the history lengths.
3. If a metric the user asks about is missing from `compare_runs`, `read_metrics` shows the raw `metrics.json`.

Report the metrics as a table with the differences, then one or two sentences on convergence. Say when runs used
different cases (the `case` column): their metrics are then not like for like.
