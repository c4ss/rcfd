# MCP and API tools

This page lists every tool of the MCP server: its access class, the service method it calls and what it does, and the
rules all tools share (access classes, policy, errors, sandbox and limits). It is for people who connect a client or an assistant
to the server, or who add a tool. For a guided first session, follow the [MCP design-loop tutorial](tutorials/mcp-design-loop.md);
for the parameters and results of the methods behind the tools, see the [service protocol](service-protocol.md).

## Conventions for every tool

The MCP server (`rcfd-mcp`, crate `rcfd-mcp`) exposes the solver service (`docs/service-protocol.md`) as tools. The assistant crate
lists the same tools, so a tool added to the server is available to the assistant without further wiring. Conventions (F-090, F-091,
F-147, F-148):

* Every tool has an access class. `read` tools never change state. `mutate` changes the session, `execute` starts computation,
  `write_files` writes files inside the sandbox. `tools/list` marks every tool with `annotations.readOnlyHint` and
  `_meta."rcfd/access"`; `_meta."rcfd/needsApproval"` is true for every class except `read`.
* The server enforces a policy on top: read-only mode refuses every tool that is not `read` (error 1100, `permission denied`), an approval
  hook is asked before every other call, and an allow list restricts the callable tools.
* Arguments that do not match a tool's `inputSchema` are JSON-RPC errors (-32602). Failures while executing a valid call (validation,
  wrong run state, limits, a path outside the sandbox (1004), a denied permission (1100)) are tool results with `isError: true` and the
  text `{"error": {"code", "message", "data"?}}`. Messages are located: they name the parameter, setting path or file.
* File paths are resolved by the service against its sandbox root (`rcfd-mcp --root`): `..` and absolute paths outside the root are
  refused with code 1004 and create nothing. The limits `--max-cells`, `--max-steps`, `--max-run-seconds` and
  `--max-design-evaluations` apply to every tool that starts runs.

## Tool list

| Tool | Access | Service method(s) | Purpose |
|---|---|---|---|
| `list_cases` | read | (local) | benchmark cases and built-in templates |
| `create_case` | mutate | `session.create` | session from a template, TOML, settings or a case file |
| `set_setting` | mutate | `settings.set` | set one setting by dotted path |
| `get_setting` | read | `settings.get` | read one setting or the whole tree |
| `load_mesh` | mutate | `settings.set` | use a mesh file |
| `check_mesh` | read | `mesh.check` | mesh quality report |
| `run` | execute | `run.start` | start the solver, wait up to `wait_seconds` |
| `status` | read | `run.status` | state, iteration, residuals, report values |
| `get_residuals` | read | `monitors.get` | monitor histories |
| `get_field_stats` | read | `field.stats` | min, max, mean of a field |
| `export_vtk` | write_files | `export.vtk` | write the solution as `.vtu` |
| `list_workflow_tasks` | read | `workflow.list` | tasks of the set-up/run workflow |
| `create_workflow` | mutate | `workflow.create` | create the workflow from a template or a file |
| `edit_workflow` | mutate | `workflow.insert`, `workflow.delete` | insert or delete a task |
| `run_workflow_task` | write_files | `workflow.run` | run a task (and prerequisites) |
| `check_case_headless` | read | `run.check` | check-only launch |
| `run_headless` | write_files | `run.headless` | run under the run-control contract |
| `read_status_stream` | read | `status.read` | summarise a status stream file |
| `design_create_study` | write_files | `design.create_study` | define a design study (parameters, conditions, objectives, constraints, method) |
| `design_create_variant` | write_files | `design.create_variant` | variant case file from a base case or a study point |
| `design_run_operating_map` | write_files | `design.run_map` | chained operating-point map with a metrics table |
| `design_run_study` | write_files | `design.run_study` | run the study's sampler or optimiser, resumable |
| `design_evaluate` | write_files | `design.evaluate` | evaluate and store one candidate |
| `design_record` | write_files | `design.record` | store externally computed metrics for a candidate |
| `design_get_metrics` | read | `design.get_metrics` | stored metrics of a candidate or a map |
| `design_compare_variants` | read | `design.compare` | side by side metrics, differences, ranking |
| `design_propose` | read | `design.propose` | next candidates of the method, no side effects |
| `design_list_study` | read | `design.list_study` | definition, counts, best record, Pareto indices |
| `design_ledger_append` | write_files | `design.ledger_append` | append one evaluated variant to a design ledger |
| `design_ledger_query` | read | `design.ledger_query` | filter the ledger; Pareto set, best so far, parameter history |
| `design_ledger_export` | write_files | `design.ledger_export` | ledger entries to a CSV or JSON file |
| `design_ledger_rerun_check` | read | `design.ledger_rerun_check` | compare a re-run's metrics with a ledger entry |
| `render_image` | read | `render.png` | PNG image of a field |

## Design tools

The design tools (ADR 0067, `docs/design-loop.md`) keep their state in a study directory inside the sandbox: `study.json` and
`evaluations.jsonl`. Typical assistant loop:

1. `design_create_study {dir, base, parameters, objectives, method, ...}` once.
2. `design_propose {dir}` returns `candidates` (objects `{parameter: value}`) or `complete: true`.
3. For each candidate either `design_evaluate {dir, params}` (rcfd runs the case at every operating condition and stores the result), or
   `design_create_variant {dir, params, path}` followed by the ordinary run tools and `design_record {dir, params, metrics}`.
4. `design_list_study`, `design_get_metrics` and `design_compare_variants` report the best feasible candidate, the Pareto front and
   exact metric differences; `design_run_operating_map` runs a performance map of a case or of a chosen candidate.

Repeating a call is safe: `design_evaluate` returns a stored candidate without running (`cached: true`, `evaluations_run: 0`),
`design_run_operating_map` returns the stored table when the case, the points and the options are unchanged, and `design_record` refuses a
candidate that already has a record. Metric values must be finite numbers; an unknown or missing parameter, a value outside its bounds, a
name with separators or a leading dot, and a base case that changed since the study was created are located errors (-32602).

## Design ledger tools

The design ledger (ADR 0121, `docs/design-loop.md`) is one JSON-lines file (for example `fan/ledger.jsonl`) that keeps every evaluated
variant across studies, evaluators and sessions. `design_ledger_append` takes the parameters and metrics as lists of
`{name, value, unit}`, the evaluator id and version, the verdict (`ok`, `diverged`, `constraint_rejected`, `not_evaluated`; a located
`reason` is required unless `ok`) and the tier (`analytic`, `reduced_order`, `cfd`), and stamps the server's rcfd version and the time.
`design_ledger_query` filters and pages the entries and, on request, returns the Pareto set, the best entry with its running best and
the history of one parameter; Pareto and best refuse a set that mixes evaluators, tiers or units of a metric, so filter by `evaluator`
and `fidelity` first. `design_ledger_export` writes CSV or JSON, and `design_ledger_rerun_check` compares the metrics of a re-run with an
entry (tolerances default to 0, bit-identical). A line torn by a killed writer is reported in `skipped_lines` and never joins the next
entry.

## Project tools (`--project`)

`rcfd-mcp --project <dir>` adds 17 tools that work on one Studio project folder ([ADR 0135](adr/0135-wave48f-plugin-and-project-tools.md),
folder contract in [ui-restart-plan.md](ui-restart-plan.md) section 8). Without `--project` the tool list is unchanged. The server
refuses to start (exit 2, a message naming the folder) when `<dir>` holds no `rcfd-project.json`. The Claude Code plugin in
`clients/claude-plugin` starts `rcfd-mcp --project ${CLAUDE_PROJECT_DIR} --root ${CLAUDE_PROJECT_DIR}` from PATH (PATH only; without `--root`, `--project` also confines the existing tools' file access to the project); the run tools
also need `rcfd-runner` and `rcfd` in the same folder as `rcfd-mcp` (as `cargo install` or a build directory gives).

Paths are project-relative (`cases/cavity.toml`) and checked like every project path (no `..`, no absolute paths, no links; refused
with 1004 before anything is created). Run arguments are run ids (`r-<yyyymmddThhmmss>-<4 hex>`). Errors added by these tools:
1200 the file changed since its hash was read (`data.current_hash`, null when the file is absent), 1201 another cooperating writer
held the file's edit lock for 2 s, 1202 no such run, file, field or snapshot. Every result's text stays under 36 000 bytes: lists
are paged, series decimated, files read in pages; a residual set or image over that is written under `runs/<id>/mcp/` or
`runs/<id>/renders/` and the result gives its `path`.

| Tool | Access | Result |
|---|---|---|
| `project_info` | read | `name`, `root`, `schema_version`, `created`, `solver_version_created`, `migrated_from`, `counts` {`cases`, `meshes`, `designs`, `notes`, `runs`, `skipped_runs`}, `runs_by_state` {state: count}, `issues` [{`path`, `message`}] (at most 50), `issues_total` |
| `list_runs` | read | `runs` [{`id`, `case`, `created`, `state`, `step`, `time`, `metrics` {name: number}}] oldest first (`metrics`: the final report values, `reports` of `metrics.json`; for an older record without it, the numeric top-level entries), `total`, `offset`, `next_offset` (null on the last page), `skipped` [{`path`, `message`}], `skipped_total`; arguments `state`, `case`, `offset`, `limit` (1-100, default 50) |
| `run_status` | read | `run`, `case`, `created`, `solver_version`, `settings`, `status` {`state`, `step`, `time`, `pid`, `updated`, `reason`?, `exit_code`?}, `last_residual` (the last line of `residuals.jsonl` or null), `events` (the last `events` lines, default 10) |
| `read_metrics` | read | `metrics` {run id: `metrics.json` or null}, `omitted` [run ids over the size budget]; 1 to 50 runs |
| `read_residuals` | read | `run`, `lines`, `skipped_lines`, `max_points`, `decimated`, `unknown_names`, `series` {`residuals.<eq>` or `monitors.<name>`: {`x` steps, `y` values}}; each series at most `max_points` (default 200, at most 2000) keeping first, last and each bucket's minimum and maximum (ADR 0025 M3); over the budget `series_names`, `path`, `bytes` instead of `series` |
| `field_summary` | read | `run`, `field`, `unit`, `location`, `precision`, `snapshot` {`index`, `step`, `time`, `upper`?, `weight`?}, `interpolated`, `n`, `finite`, `min`, `max`, `mean` (cell-volume weighted for cell fields: `weighting` `cell_volume`, else `none`); `step` or `time` with `pick` `nearest` (default), `floor` or `linear` (time only); default the last snapshot |
| `render_field` | read | an image block (PNG) and the text {`run`, `field`, `snapshot`, `interpolated`, `min`, `max`, `width`, `height`, `png_bytes`}; or, over the budget, the text with `path` and `note`. Cell fields of 2D (one cell layer) meshes only; same renderer and options as `render_image` |
| `compare_runs` | read | `baseline` (the first id), `runs` [{`id`, `case`, `state`, `step`, `time`, `residual_lines`}], `metrics` {report name: {run id: value}} (as in `list_runs`), `differences` {name: {run id: value minus the baseline's}}, `metrics_total`, `metrics_omitted`; 2 to 10 runs |
| `list_changes` | read | `token`, `changes` [paths], `total`, `truncated`, and `since` when given; call without `since` to get a token (`.rcfd/` is not watched) |
| `read_project_file` | read | `path`, `hash` (of the whole file; the `base_hash` for edits), `size`, `offset`, `text`, `next_offset` (null at the end); pages of at most 32 000 bytes |
| `edit_case` | write_files | `path`, `written`, `valid`, `form` (`minimal`: the changed lines edited in place, comments kept; an edit that cannot be placed in the file's own lines, such as a value written over several lines, is refused with 1003 and nothing is written), `old_hash`, `new_hash`, `values` {setting: stored value}; arguments `path` (.toml or .json), `base_hash`, `set` {setting: value}, `dry_run` |
| `write_note` | write_files | `path`, `created`, `old_hash`, `new_hash`; under `notes/` only; no `base_hash` means "create, the file must not exist" |
| `start_run` | execute | `run`, `case`, `state`; arguments `case`, `set` [`path=value`], `priority`, `threads`, `mem_mb`; starts the project's `rcfd-runner` when none is alive, with its output appended to `.rcfd/runner.log`; a runner that exits before it holds the project's lock fails the call at once (-32000, `data.run` names the queued run) with its exit code and the end of that log ([ADR 0146](adr/0146-job48l-runner-output-early-exit-windows-inherited-handles.md)) |
| `pause_run`, `resume_run`, `cancel_run` | execute | `run`, `requested`, `state`; 1002 for a run that already ended |
| `wait_run` | read | `run`, `from`, `state`, `changed`, `terminal`, `waited_s`, `status`; returns when the state differs from `from` (default: the state at the call) or the run ended, at the latest after `timeout_s` (default 30, at most 60) |

On Windows the runner that `start_run` starts (and the solvers it starts) also inherit `rcfd-mcp`'s standard input and output
pipes, because std starts processes with handle inheritance on and the workspace allows no unsafe code. Measured: after the
server has exited, its standard output reaches EOF only when that runner exits ([ADR 0146](adr/0146-job48l-runner-output-early-exit-windows-inherited-handles.md)
C2). What this does to Claude Code when it closes or restarts the server has not been checked. On Linux the pipe closes with the
server.

Edits (`edit_case`, `write_note`) are compare-and-replace under a cooperative lock (`rcfd_project::atomic::replace_if_hash`): the
file is replaced only if it still has `base_hash`, while `.rcfd/locks/<hash of the path>.lock` is held; each accepted edit appends
`{t, kind: "edit", who, path, old_hash, new_hash}` to `.rcfd/edits.jsonl`, so the Studio can say who changed a file. Writers that do
not take the lock (Claude Code's own Edit and Write tools, an editor) are not stopped; the Studio's watcher sees their edits afterwards.

## Where to go next

- [Design loop](design-loop.md): the study, map and ledger formats the design tools read and write.
- [Run control](run-control.md): exit codes and the status stream behind the headless tools.
- [Service protocol](service-protocol.md): the methods, error codes and sandbox rules of the service.
