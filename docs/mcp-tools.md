# MCP and API tools

This page lists every tool of the MCP (Model Context Protocol) server: its access class, the service method it calls and
what it does, and the rules all tools share (access classes, policy, errors, sandbox and limits). It is for people who connect
a client or an assistant to the server, or who add a tool. For a guided first session, follow the
[MCP design-loop tutorial](tutorials/mcp-design-loop.md); for the parameters and results of the methods behind the tools, see
the [service protocol](service-protocol.md).

## Conventions for every tool

The MCP server (`rcfd-mcp`, crate `rcfd-mcp`) exposes the solver service (`docs/service-protocol.md`) as tools. The assistant crate
lists the same tools, so a tool added to the server is available to the assistant without further wiring. Conventions (F-090, F-091,
F-147, F-148):

* Every tool has an access class. `read` tools never change state, `mutate` tools change the session, `execute` tools start
  computation and `write_files` tools write files inside the sandbox. `tools/list` marks every tool with `annotations.readOnlyHint` and
  `_meta."rcfd/access"`; `_meta."rcfd/needsApproval"` is true for every class except `read`.
* The server enforces a policy on top: read-only mode refuses every tool that is not `read` (error 1100, `permission denied`),
  an approval hook is asked before every call to a tool that is not `read`, and an allow list restricts the callable tools.
* Arguments that do not match a tool's `inputSchema` are JSON-RPC errors (-32602). Failures while executing a valid call (validation,
  wrong run state, limits, a path outside the sandbox (1004), a denied permission (1100)) are tool results with `isError: true` and the
  text `{"error": {"code", "message", "data"?}}`. Messages are located: they name the parameter, setting path or file.
* The service resolves file paths against its sandbox root (`rcfd-mcp --root`). It refuses `..` and absolute paths outside the
  root with code 1004 and creates nothing. The limits `--max-cells`, `--max-steps`, `--max-run-seconds` and
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
| `design_run_operating_map` | write_files (execute with `--project`) | `design.run_map` | chained operating-point map with a metrics table |
| `design_run_study` | write_files (execute with `--project`) | `design.run_study` | run the study's sampler or optimiser, resumable |
| `design_evaluate` | write_files (execute with `--project`) | `design.evaluate` | evaluate and store one candidate |
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

With `--project` the 16 tools above that need a session of the server's memory, or make one, are not offered (see
[Project mode](#project-mode-the-session-tools-that-are-not-offered)); the other 17 are.

## Design tools

The design tools (ADR 0067, `docs/design-loop.md`) keep their state in a study directory inside the sandbox: `study.json` and
`evaluations.jsonl`. Typical assistant loop:

1. `design_create_study {dir, base, parameters, objectives, method, ...}` once.
2. `design_propose {dir}` returns `candidates` (objects `{parameter: value}`) or `complete: true`.
3. For each candidate either `design_evaluate {dir, params}` (rcfd runs the case at every operating condition and stores the result), or
   `design_create_variant {dir, params, path}` followed by the ordinary run tools and `design_record {dir, params, metrics}`.
4. `design_list_study`, `design_get_metrics` and `design_compare_variants` report the best feasible candidate, the Pareto front and
   exact metric differences; `design_run_operating_map` runs a performance map of a case or of a chosen candidate.

Repeating a call is safe:

- `design_evaluate` returns a stored candidate without running (`cached: true`, `evaluations_run: 0`).
- `design_run_operating_map` returns the stored table when the case, the points and the options are unchanged.
- `design_record` refuses a candidate that already has a record.

Metric values must be finite numbers. An unknown or missing parameter, a value outside its bounds, a name with separators or a
leading dot, and a base case that changed since the study was created are located errors (-32602).

With `--project`, `design_run_study` and `design_evaluate` evaluate each candidate at each operating condition as one run of the
project's runner, tagged in its `run.json` with `study`, `variant` and `condition`, and add `evaluator` (`runner`), `runs` (at most
50), `runs_total` and `runs_adopted` to their result. A chained condition (`chain`, the default, with two or more conditions)
continues from the run of the condition before (`settings.continue_from`), and a study's own `max_steps` caps each run
(`settings.max_steps`); `design_run_operating_map` runs each point the same way, untagged, continuing from the run of the
converged point before ([ADR 0200](adr/0200-f287-registration-runner-runs-that-continue-from-a-previous-run-with-a-step-cap.md)).
Before a study's run is submitted, a run of the project that already is that evaluation (the same tags, case, case bytes,
entries, launch settings and solver version, not failed or cancelled) is adopted instead, so a study resumed after the server
was stopped makes no second run for a candidate that has one; an adopted run's object in `runs` has `adopted: true`
([ADR 0201](adr/0201-f288-registration-adopt-matching-runs-after-an-mcp-server-restart.md)). A study or map whose base case is
outside the project is evaluated in the server process, with `evaluator: process` and `evaluator_note`
([ADR 0189](adr/0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md), `docs/design-loop.md`).
Because they start runs there, as `start_run` does, the three have the access class `execute` with `--project`; without it they
stay `write_files` ([ADR 0196](adr/0196-three-code-changes-decided-2026-11-06-f246-r2-record-design-tool-class-sv202-text.md) B,
ADR 0200 S9).

## Design ledger tools

The design ledger (ADR 0121, `docs/design-loop.md`) is one JSON-lines file (for example `fan/ledger.jsonl`) that keeps every evaluated
variant across studies, evaluators and sessions. A line torn by a killed writer is reported in `skipped_lines` and never joins the
next entry.

- `design_ledger_append` takes the parameters and metrics as lists of `{name, value, unit}`, the evaluator id and version, the
  verdict (`ok`, `diverged`, `constraint_rejected`, `not_evaluated`; a located `reason` is required unless `ok`) and the tier
  (`analytic`, `reduced_order`, `cfd`), and stamps the server's rcfd version and the time.
- `design_ledger_query` filters and pages the entries and, on request, returns the Pareto set, the best entry with its running
  best and the history of one parameter. Pareto and best refuse a set that mixes evaluators, tiers or units of a metric, so filter
  by `evaluator` and `fidelity` first.
- `design_ledger_export` writes CSV or JSON.
- `design_ledger_rerun_check` compares the metrics of a re-run with an entry (tolerances default to 0, bit-identical).

## Project tools (`--project`)

`rcfd-mcp --project <dir>` adds 23 tools that work on one Studio project folder ([ADR 0135](adr/0135-wave48f-plugin-and-project-tools.md),
folder contract in [ui-restart-plan.md](ui-restart-plan.md) section 8), and leaves out 16 session tools (below), so it lists 40.
Without `--project` the tool list is unchanged. The server
refuses to start (exit 2, a message naming the folder) when `<dir>` holds no `rcfd-project.json`. The Claude Code plugin in
`clients/claude-plugin` starts `rcfd-mcp --project ${CLAUDE_PROJECT_DIR} --root ${CLAUDE_PROJECT_DIR}` from PATH (PATH only).
Without `--root`, `--project` also confines the existing tools' file access to the project. The run tools also need `rcfd-runner`
and `rcfd` in the same folder as `rcfd-mcp` (as `cargo install` or a build directory gives).

The tools are the commands and queries of the project workspace, the crate `crates/rcfd-workspace` that the Studio uses too
([ADR 0173](adr/0173-workspace-layer-between-the-project-folder-the-mcp-server-and-the-studio.md),
[ADR 0185](adr/0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md),
[ADR 0186](adr/0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md),
[ADR 0187](adr/0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md),
[ADR 0188](adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md),
[ADR 0195](adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md)).
`rcfd-mcp` keeps their descriptors.

Paths are project-relative (`cases/cavity.toml`) and checked like every project path (no `..`, no absolute paths, no links; refused
with 1004 before anything is created). Run arguments are run ids (`r-<yyyymmddThhmmss>-<4 hex>`).

Every result's text stays under 36 000 bytes: lists are paged, series decimated and files read in pages. A residual set or image
over that is written under `runs/<id>/mcp/` or `runs/<id>/renders/`, and the result gives its `path`.

Errors added by these tools:

- 1200: the file changed since its hash was read (`data.current_hash`, null when the file is absent).
- 1201: another cooperating writer held the file's edit lock for 2 s.
- 1202: no such run, file, field or snapshot.

The tools that change the project (`edit_case`, `write_note`, `new_case`, `import_mesh`, `start_run`, `pause_run`, `resume_run`,
`cancel_run`) take an optional `reason`: a string of at most 500 characters (longer, or not a string: -32602, nothing done),
kept with the change in the project journal (below) and shown in the Studio and by `read_feed`. Results do not change with it.

| Tool | Access | Result |
|---|---|---|
| `project_info` | read | `name`, `root`, `schema_version`, `created`, `solver_version_created`, `migrated_from`, `counts` {`cases`, `meshes`, `designs`, `notes`, `runs`, `skipped_runs`}, `runs_by_state` {state: count}, `issues` [{`path`, `message`}] (at most 50), `issues_total` |
| `list_runs` | read | `runs` [{`id`, `case`, `created`, `state`, `step`, `time`, `metrics` {name: number}}] oldest first (`metrics`: the final report values, `reports` of `metrics.json`; for an older record without it, the numeric top-level entries), `total`, `offset`, `next_offset` (null on the last page), `skipped` [{`path`, `message`}], `skipped_total`; arguments `state`, `case`, `offset`, `limit` (1-100, default 50) |
| `run_status` | read | `run`, `case`, `created`, `solver_version`, `settings`, `status` {`state`, `step`, `time`, `pid`, `updated`, `reason`?, `exit_code`?}, `last_residual` (the last line of `residuals.jsonl` or null), `events` (the last `events` lines, default 10) |
| `read_metrics` | read | `metrics` {run id: `metrics.json` or null}, `omitted` [run ids over the size budget]; 1 to 50 runs |
| `read_residuals` | read | `run`, `lines`, `skipped_lines`, `max_points`, `decimated`, `unknown_names`, `series` {`residuals.<eq>`, `monitors.<name>`, and for adaptive-corrector runs `correctors` and `corrector_cap` (ADR 0160, 0172): {`x` steps, `y` values}}; each series at most `max_points` (default 200, at most 2000) keeping first, last and each bucket's minimum and maximum (ADR 0025 M3); over the budget `series_names`, `path`, `bytes` instead of `series` |
| `field_summary` | read | `run`, `field`, `unit`, `location`, `precision`, `snapshot` {`index`, `step`, `time`, `upper`?, `weight`?}, `interpolated`, `n`, `finite`, `min`, `max`, `mean` (cell-volume weighted for cell fields: `weighting` `cell_volume`, else `none`); `step` or `time` with `pick` `nearest` (default), `floor` or `linear` (time only); default the last snapshot |
| `render_field` | read | an image block (PNG) and the text {`run`, `field`, `snapshot`, `interpolated`, `min`, `max`, `width`, `height`, `png_bytes`}; or, over the budget, the text with `path` and `note`. Cell fields of 2D (one cell layer) meshes only; same renderer and options as `render_image` |
| `compare_runs` | read | `baseline` (the first id), `runs` [{`id`, `case`, `state`, `step`, `time`, `residual_lines`}], `metrics` {report name: {run id: value}} (as in `list_runs`), `differences` {name: {run id: value minus the baseline's}}, `metrics_total`, `metrics_omitted`; 2 to 10 runs |
| `list_changes` | read | `token`, `changes` [paths], `total`, `truncated`, and `since` when given; call without `since` to get a token (`.rcfd/` is not watched) |
| `read_feed` | read | without `since`: `cursor`, `entries` [], `total` 0 (the server's feed starts at its first call); with `since`: `since`, `cursor`, `entries` (numbered after `since`, oldest first, at most `limit`, 1-200, default 50, and within the size budget), `total`, `next` (the number to pass as `since` to continue, null when none), `dropped` (entries after `since` no longer kept; the feed keeps 1 000), `skipped_lines`. An entry has `n`, `source` (`journal`, `run` or `external`), `kind`, `t`, `seq`, `path`, `run`, `who`, `actor` {`client`, `instance` {`pid`, `started`}}, `reason`, `changes` [{`path`, `old`, `new`}], the line's other fields, and `text` (one line: who changed what and why); an entry over half the size budget comes without `changes` and the line's other fields, with `omitted` (the journal keeps the whole line). Journal entries are the lines of the project journal; run entries are a run's `state`, `warning` and `stop` events (`run_state`, `run_warning`, `run_stop`); an `external` entry is a file outside `runs/` that changed with no journal line for its new content, 500 ms after its modification ([ADR 0188](adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) J6, J7). With `from_seq` (at least 1; not with `since`) instead: `from_seq`, `cursor`, `entries` (the journal's lines whose `seq` is at least `from_seq`, read from the start of the file, in file order, each as a journal entry without `n`; lines without `seq` are not replayed), `total`, `next_seq` (to continue, null when none are left), `skipped_lines`; then `since` = `cursor` continues live ([ADR 0195](adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md) P7) |
| `read_project_file` | read | `path`, `hash` (of the whole file; the `base_hash` for edits), `size`, `offset`, `text`, `next_offset` (null at the end); pages of at most 32 000 bytes |
| `edit_case` | write_files | `path`, `written`, `valid`, `form` (`minimal`: the changed lines edited in place, comments kept; an edit that cannot be placed in the file's own lines, such as a value written over several lines, is refused with 1003 and nothing is written), `old_hash`, `new_hash`, `values` {setting: stored value}; arguments `path` (.toml or .json), `base_hash`, `set` {setting: value}, `dry_run`; when `set` holds a `mesh.*` setting the edited mesh is built and checked before anything is written (a refused mesh file 1004, a mesh that cannot be built 1003 with located `data.diagnostics`) and the result has `mesh` {`ok`, `report`, `patches`, `patches_total`} as `check_case_mesh`; a mesh that builds but fails the check is written with `mesh.ok` false ([ADR 0187](adr/0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) M7, Q2) |
| `write_note` | write_files | `path`, `created`, `old_hash`, `new_hash`; under `notes/` only; no `base_hash` means "create, the file must not exist" |
| `new_case` | write_files | `path`, `hash` (the `base_hash` for `edit_case`), `bytes`, `from` (`example <source>`, `template <name>`, `toml`, `settings` or `copy <path>`), `data_files` [{`path`, `written`}]; arguments `name` (letters, digits, `-`, `_`; not taken in `cases/` in any letter case, else 1004) and exactly one of `example` (a shipped example of the repository's `cases/`, written byte for byte with the data files it reads), `template`, `toml`, `settings` (written as `cases/<name>.json`) or `copy` (a project case, byte for byte); the case is validated before anything is written (1003 with located `data.diagnostics`) and no file is ever replaced ([ADR 0186](adr/0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) G3) |
| `check_case_mesh` | read | `case`, `mesh` {`source` `generator` with `generator`, or `file` with `file`}, `ok`, `report` (the quality report `rcfd check-mesh --json` prints), `patches` [{`name`, `faces`, `kind`}] (the names `boundaries.*` must use), `patches_total`; argument `case` (.toml or .json). The case's `mesh.file` must resolve inside the project (else 1004, located at `mesh.file`); a mesh that cannot be built is 1003 with located `data.diagnostics`. Over the size budget patches are dropped from the end, then the report's `poor_list` (`poor_list_omitted`) |
| `preview_mesh` | read | an image block (PNG) and the text {`case`, `measure`, `min`, `max`, `non_finite`, `range`, `cells`, `cells_drawn`, `axis`, `slice`, `width`, `height`, `png_bytes`, `reduced_from`?}; arguments `case`, `measure` (`non_orthogonality`, the default: the largest of the cell's faces, degrees; `skewness`, the largest of its faces; `aspect_ratio`), `width` (64-4096, default 640), `axis` (`x`, `y`, `z`) and `slice` (default: the middle of the thinnest direction, the plane `render_field` draws); colours run from the measure's ideal value (0, or 1 for the aspect ratio) to its largest value (ideal + 1 when the mesh is within 1e-9 of ideal); an image over the budget is drawn again at half the width, down to 64 pixels ([ADR 0187](adr/0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) M4, Q4, Q5) |
| `import_mesh` | write_files | `source`, `path` (`meshes/<name>`), `copied`, `format` (`gmsh`, `fluent` or `openfoam`), `files` [{`path`, `bytes`, `hash`, `written`}], `cells`, `faces`, `internal_faces`, `points`, `patches`, `patches_total`, `zones` [{`name`, `kind`, `size`}], `zones_total`, `bounds` {`min`, `max`}, `notes`, `mesh_file` (the `mesh.file` of a case directly in `cases/`); arguments `path` (a Gmsh or Fluent `.msh`/`.cas` file, or an OpenFOAM folder: its `polyMesh` subfolder or itself) and `name` (default the source's name; letters, digits, `-`, `_`, `.`; a file keeps its extension). Read with the readers a case's mesh build uses before anything is written (1003 when none reads it); never replaces a file (the same bytes already there: `copied` false; other bytes or a name differing only in letter case: 1004); one `mesh_imported` journal line when it wrote a file |
| `list_meshes` | read | `meshes` [{`path`, `kind` (`file`, or `openfoam` for a folder holding `points`, `faces`, `owner`, `neighbour` and `boundary`), `bytes`, `files`, `used_by` [cases whose `mesh.file` names it]}], `total`, `offset`, `next_offset`, `outside` [{`case`, `mesh_file`, `path`, `exists`, `refused`}] (case mesh files that are not an entry of `meshes/` or that leave the project; at most 20), `outside_total`, `unread` [{`path`, `message`}] (cases that are not TOML or JSON; at most 20), `unread_total`; arguments `offset`, `limit` (1-100, default 50) |
| `start_run` | execute | `run`, `case`, `state`, plus `created` (the `new_case` result) when it made the case and `store` {`applied`, `set`, `snapshots`, `estimated_bytes`, `note`} when the start plan was asked for; arguments `case` or `new_case` (an object with `new_case`'s arguments: the case is made first, then run, in one call), `set` [`path=value`], `priority`, `threads`, `mem_mb`, `store` (`plan`: the Studio's field-store proposal of [ADR 0140](adr/0140-wave48g3-store-fields-by-default.md), about 100 snapshots in f32, put before `set` unless the case or a `set` entry gives `output.store_interval` or `output.store_time_interval`; `none`: only what the case asks for; default `plan` with `new_case` and `none` with `case`, ADR 0186 Q2); starts the project's `rcfd-runner` when none is alive, with its output appended to `.rcfd/runner.log`; a runner that exits before it holds the project's lock gives an error result at once (code -32000, `data.run` the queued run) whose message has the runner's exit code and the end of that log ([ADR 0146](adr/0146-job48l-runner-output-early-exit-windows-inherited-handles.md)) |
| `pause_run`, `resume_run`, `cancel_run` | execute | `run`, `requested`, `state`; 1002 for a run that already ended |
| `wait_run` | read | `run`, `from`, `state`, `changed`, `terminal`, `waited_s`, `status`; returns when the state differs from `from` (default: the state at the call) or the run ended, at the latest after `timeout_s` (default 30, at most 60) |

On Windows the runner that `start_run` starts (and the solvers it starts) also inherit `rcfd-mcp`'s standard input and output
pipes, because std starts processes with handle inheritance on and the workspace allows no unsafe code. Measured: after the
server has exited, its standard output reaches EOF only when that runner exits ([ADR 0146](adr/0146-job48l-runner-output-early-exit-windows-inherited-handles.md)
C2). What this does to Claude Code when it closes or restarts the server has not been checked. On Linux the pipe closes with the
server.

Edits (`edit_case`, `write_note`) are compare-and-replace under a cooperative lock (`rcfd_project::atomic::replace_if_hash`). The
file is replaced only if it still has `base_hash`, while `.rcfd/locks/<hash of the path>.lock` is held. Writers that do not take the
lock (an editor, a shell command, Claude Code's own Edit and Write tools outside the folders the plugin's hook guards) are not
stopped; the feed reports their edits afterwards as `external`.

The project journal, `.rcfd/edits.jsonl`, has one JSON line per accepted write, case creation, mesh import, run submission and
accepted run request, from the MCP server and from the Studio alike ([ADR 0188](adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md)).
Each line has `t`, `kind`, `who` (the command), `seq` (one more than the last, given under the short lock
`.rcfd/locks/journal.lock`; lines written before ADR 0188 have none), `actor` {`client` (`mcp` or `studio`), `instance` {`pid`,
`started`}} when the writer has one, and `reason` when given. By kind:

- `edit`: `path`, `old_hash`, `new_hash`, and `changes` [{`path`, `old`, `new`}]: for `edit_case` each setting set, for a Studio
  save every setting whose value changed (values as the case reads them, defaults included). At most 100 entries
  (`changes_total` when cut); a value over 1 000 bytes of JSON is written as {`bytes`, `hash`}.
- `case_created`: `path`, `hash`, `bytes`, `from`, `data_files` [{`path`, `hash`, `written`}].
- `mesh_imported`: `path`, `source`, `format`, `files` [{`path`, `bytes`, `hash`}], `cells`, `faces`, `patches` (their count).
- `run_submitted`: `run`, `case`, `set`, `priority`, `threads`, `mem_mb`.
- `run_control`: `run`, `requested` (`pause`, `resume` or `cancel`).
- `stale_lock_broken`: `path`, `age_s` (an edit lock older than 30 s was removed).

Which Claude Code session an `mcp` actor belongs to is not recorded: no way to link the server's process to a session has been
checked (ADR 0188 Q4).

### Project mode: the session tools that are not offered

With `--project`, a session tool whose every use needs a session of the server's memory, or makes one, is neither listed nor
callable (a call is the JSON-RPC error -32602 `unknown tool`), because in project mode cases, meshes and runs live in the
project folder ([ADR 0195](adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md) P1).
The 17 others (`list_cases`, `check_case_headless`, `read_status_stream` and the design and ledger tools) stay, with the same
descriptors except the access class `execute` of `design_run_study`, `design_evaluate` and `design_run_operating_map` (ADR 0196
B, ADR 0200 S9); their `session`
arguments cannot be used there. `initialize`'s first sentence then names no session tool.

| Not offered with `--project` | Use instead |
|---|---|
| `create_case` | `new_case`, or `start_run` with `new_case` |
| `set_setting`, `get_setting` | `edit_case`; `read_project_file` (and `edit_case` with `dry_run`, whose `values` give the stored values) |
| `load_mesh`, `check_mesh` | `import_mesh` then `edit_case` of `mesh.source` and `mesh.file`; `check_case_mesh` |
| `run`, `status` | `start_run`, `wait_run`, `run_status` |
| `get_residuals`, `get_field_stats`, `render_image` | `read_residuals`, `field_summary`, `render_field` |
| `export_vtk` | every run of the run service writes `runs/<id>/output/solution.vtu` |
| `run_headless` | `start_run`; no equivalent for `continue_from`, interpolation, checkpoints, `out_dir` and `max_elapsed_s` |
| `list_workflow_tasks`, `create_workflow`, `edit_workflow`, `run_workflow_task` | none yet |

The Studio's Claude Code sidebar allows the project's read tools without asking: in a project whose allow-list file
(`.rcfd/claude-allow.json`) the Studio makes after this change, those are the 15 read-class project tools, `read_feed`,
`check_case_mesh`, `preview_mesh` and `list_meshes` among them; an existing allow-list file is not changed (ADR 0195 P5).

### The plugin's edit hook

The plugin's `hooks/hooks.json` runs `rcfd-mcp --edit-hook ${CLAUDE_PROJECT_DIR}` before every Claude Code `Edit`, `Write`,
`MultiEdit` and `NotebookEdit` call ([ADR 0195](adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md)
P6, ADR 0173 D5). In an rcfd project (a folder with `rcfd-project.json`) a call that writes a file under `cases/`, `runs/` or
`designs/` is refused (exit 2) with a message that names `edit_case` and `new_case`; Claude sees it as the tool's error.
`meshes/`, `notes/` and every other file stay writable, so Claude can still place a mesh file for `import_mesh`. Paths are
compared by whole components, with `..` applied, and again with links resolved (on Windows without letter case). Outside an
rcfd project the hook allows everything. It fails open: a missing `rcfd-mcp`, unreadable input (exit 1) or a timeout lets the
call go ahead, by Claude Code's rule. Shell commands and `.rcfd/` are not covered. Checked on Claude Code 2.1.319 with the
plugin loaded by `--plugin-dir`; its behaviour under `bypassPermissions` is not verified. To turn it off, disable the plugin
(which also removes its tools) or turn hooks off with Claude Code's `disableAllHooks` setting.

## Where to go next

- [Design loop](design-loop.md): the study, map and ledger formats the design tools read and write.
- [Run control](run-control.md): exit codes and the status stream behind the headless tools.
- [Service protocol](service-protocol.md): the methods, error codes and sandbox rules of the service.
