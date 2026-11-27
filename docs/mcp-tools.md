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
| `design_uq` | write_files | (library, `design::uq::run`) | tolerance study of a named evaluator, recorded in a design ledger |
| `design_sobol` | write_files | (library, `design::sensitivity::run`) | Sobol sensitivity indices of a named evaluator, recorded in a design ledger |
| `render_image` | read | `render.png` | PNG image of a field |
| `create_project` | write_files | (local, `rcfd_workspace::create_project`) | make a project folder; only without `--project` |

With `--project` the 16 tools above that need a session of the server's memory, or make one, are not offered (see
[Project mode](#project-mode-the-session-tools-that-are-not-offered)), nor is `create_project` (the server already has a
project); the other 19 are.

`create_project {path, name?}` (ADR 0731) makes a project folder with `rcfd_project::Project::create`, as
`rcfd project init <dir> [--name <name>]` does: `rcfd-project.json` and the folders `cases`, `meshes`, `runs`, `designs`,
`notes` and `.rcfd`. The folder may exist and hold other files; nothing in it is replaced, and the result
`{root, name, schema_version, created, solver_version_created, existing}` lists the top-level entries that were already
there. `name` defaults to the folder's name; the solver version recorded is `rcfd <version>`. Errors: 1200 when the folder
already holds `rcfd-project.json` (`data.root`, nothing written), 1004 for a file or a path outside `--root`, 1100 with
`--read-only`, -32602 for an empty name, one over 200 characters or one with a control character, and -32000 for any other
failure (naming the folders already made). No journal line is written: the manifest's `created` is the record.

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
Each submitted run, each stored evaluation or map point and each study or variant made inside the project has its line in the
project journal (below, `run_submitted`, `design_recorded`, `design_created`); an adopted run and a cached evaluation have none.
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

## Tolerance and Sobol study tools

`design_uq` and `design_sobol`
([ADR 0784](adr/0784-f339-registration-mcp-tools-for-tolerance-and-sobol-studies-of-the-reduced-order-fan.md)) run the two
study drivers of the library, `design::uq::run` and `design::sensitivity::run` (`docs/design-loop.md`), on a named evaluator
and record the study in a design ledger. They add nothing of their own to the numbers: the reply's `result` is the
driver's result, bit for bit. They are not service methods; the server calls the library.

The one named evaluator is `rcfd.fan_rom`, the reduced-order axial fan (ADR 0119, ADR 0623) with three inputs:
`stagger_common_deg` (unit `deg`, added to every section's stagger), `chord_rel` (unit `1`, every chord times
`1 + chord_rel`) and `tip_gap_delta` (unit `m`, added to the tip gap). Its metrics are `q`, `dp_s`, `dp_t`, `power`,
`torque`, `eta_s`, `eta_t` and `tone1_pa` at the operating point on the system curve.

Arguments of both tools:

- `ledger`: the ledger file inside the sandbox; `name`: the study name, which must be new in that ledger.
- `evaluator`: `rcfd.fan_rom`. `evaluator_settings`: `fan` `{n_blades, r_hub, r_tip, tip_gap, family, sections}` with
  `family` `naca4` or `circular_arc` and each section `{radius, chord, stagger, camber, camber_pos}` (metres, stagger in
  radians from the axis); `condition` `{rpm, rho, nu, system_k, system_l, c0, observer_r, observer_theta, n_scan}` (all
  nine, SI units, the angle in radians); and optionally `model` with any of `n_strips`, `cl_max`, `cd_normal`,
  `friction`, `annulus_drag`, `secondary_drag`, `tip_cd` (absent keys keep the registered values of ADR 0119).
- `params`: the uncertain inputs, each `{name, unit, dist}` with `dist` one of `{type: "uniform", lo, hi}`,
  `{type: "normal", mean, sd, lo, hi}` (`lo` and `hi` optional: no truncation on that side) and
  `{type: "beta", a, b, lo, hi}`. A key of `dist` that its type does not have is refused, whatever its value.
- `seed` (default 1) and `workers` (default 1; the result does not depend on it).

`design_uq` also takes `sampler` (`monte_carlo`, the default, or `lhs`), `stop` `{batch, max_samples, min_batches,
se_abs, se_rel, metrics}` (the first two required), `quantiles` (default 0.05, 0.5, 0.95), `confidence` (default 0.95)
and `requirements` (a list of `{metric, cmp, limit}` with `cmp` either `<=` or `>=`). `design_sobol` takes `n`, the rows
of each of its two sample matrices; it makes `n (d + 2)` evaluations for `d` inputs.

Both return `{ledger, evaluator, evaluator_version, fidelity, entries_appended, result}`. Every sample or evaluation is a
ledger entry of kind `evaluation`, and one entry of kind `uq_summary` or `sobol_summary` closes the study, so
`design_ledger_query` reads the study afterwards.

Limits and errors: one call makes at most `--max-design-evaluations` evaluations (`stop.max_samples`, or `n (d + 2)`),
otherwise 1004; a ledger path outside the sandbox is 1004; an unknown evaluator, a setting, parameter or distribution
that is missing, unknown or invalid, and a study name the ledger already holds are -32602 with the key named. A refused
call writes nothing. Neither tool is idempotent: a repeated call needs a new name. With `--project` both stay, with the
class `write_files`; they run in the server process and start no run of the project's runner.

## Project tools (`--project`)

`rcfd-mcp --project <dir>` adds 38 tools that work on one Studio project folder ([ADR 0135](adr/0135-wave48f-plugin-and-project-tools.md),
folder contract in [ui-restart-plan.md](ui-restart-plan.md) section 8), and leaves out 16 session tools (below), so it lists 57.
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
[ADR 0195](adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md),
[ADR 0733](adr/0733-f837-proposal-set-runner-config-the-runner-writes-its-own-budgets.md)).
`rcfd-mcp` keeps their descriptors.

Paths are project-relative (`cases/cavity.toml`) and checked like every project path (no `..`, no absolute paths, no links; refused
with 1004 before anything is created). Run arguments are run ids (`r-<yyyymmddThhmmss>-<4 hex>`).

Every result's text stays under 36 000 bytes: lists are paged, series decimated and files read in pages. A residual set or image
over that is written under `runs/<id>/mcp/` or `runs/<id>/renders/`, and the result gives its `path`.

Errors added by these tools:

- 1200: the file changed since its hash was read (`data.current_hash`, null when the file is absent).
- 1201: another cooperating writer held the file's edit lock for 2 s.
- 1202: no such run, file, field or snapshot.

The tools that change the project (`edit_case`, `write_note`, `new_case`, `import_mesh`, `set_runner_config`, `refine_case_mesh`, `start_run`, `pause_run`,
`resume_run`, `cancel_run`, and the trash tools `trash_run`, `trash_results`, `trash_case`, `restore_trash`, `empty_trash`) take an optional `reason`: a string of at most 500 characters (longer, or not a string: -32602, nothing done),
kept with the change in the project journal (below) and shown in the Studio and by `read_feed`. Results do not change with it.

`empty_trash` is the one tool of the access class `delete` ([ADR 0736](adr/0736-f829-proposal-a-project-trash-with-restore-and-compare-and-set-for-settings-set.md)): it removes for good, its `destructiveHint` is
true, and no allow-list holds it, by default or by a rule added later (a rule naming a whole server is refused too), so
the client asks for every call.

| Tool | Access | Result |
|---|---|---|
| `project_info` | read | `name`, `root`, `schema_version`, `created`, `solver_version_created`, `migrated_from`, `counts` {`cases`, `meshes`, `designs`, `notes`, `runs`, `skipped_runs`}, `runs_by_state` {state: count}, `issues` [{`path`, `message`}] (at most 50), `issues_total` |
| `list_runs` | read | `runs` [{`id`, `case`, `created`, `state`, `step`, `time`, `metrics` {name: number}}] oldest first (`metrics`: the final report values, `reports` of `metrics.json`; for an older record without it, the numeric top-level entries), `total`, `offset`, `next_offset` (null on the last page), `skipped` [{`path`, `message`}], `skipped_total`, `time_unit` (`"s"`: the unit of each `time`, simulated seconds; 0 for a steady run, [ADR 0734](adr/0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md)); arguments `state`, `case`, `offset`, `limit` (1-100, default 50) |
| `run_status` | read | `run`, `case`, `created`, `solver_version`, `settings`, `status` {`state`, `step`, `time`, `pid`, `updated`, `reason`?, `exit_code`?}, `last_residual` (the last line of `residuals.jsonl` or null), `events` (the last `events` lines, default 10), `launch` (the run's latest launch record `runs/<id>/launch.json` or `launch-<n>.json`: {`launch_seq`, `argv` (the solver's command line, program first), `cwd`, `solver` {`path`, `version`, `build` {`exe_hash` (`fnv64:` of the executable), `git_rev` (or null)}}, `started`}; null before the launch and for a run launched before [ADR 0734](adr/0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md)), `time_unit` (`"s"`, the unit of `status.time`) |
| `read_metrics` | read | `metrics` {run id: `metrics.json` or null}, `omitted` [run ids over the size budget]; 1 to 50 runs |
| `read_residuals` | read | `run`, `lines`, `skipped_lines`, `max_points`, `decimated`, `unknown_names`, `series` {`residuals.<eq>`, `monitors.<name>`, and for adaptive-corrector runs `correctors` and `corrector_cap` (ADR 0160, 0172): {`x` steps, `y` values}}; each series at most `max_points` (default 200, at most 2000) keeping first, last and each bucket's minimum and maximum (ADR 0025 M3); over the budget `series_names`, `path`, `bytes` instead of `series` |
| `field_summary` | read | `run`, `field`, `unit`, `location`, `precision`, `snapshot` {`index`, `step`, `time`, `upper`?, `weight`?}, `interpolated`, `n`, `finite`, `min`, `max`, `mean` (cell-volume weighted for cell fields: `weighting` `cell_volume`, else `none`); `step` or `time` with `pick` `nearest` (default), `floor` or `linear` (time only); default the last snapshot |
| `render_field` | read | an image block (PNG) and the text {`run`, `field`, `snapshot`, `interpolated`, `min`, `max`, `width`, `height`, `png_bytes`}; or, over the budget, the text with `path` and `note`. Cell fields of 2D (one cell layer) meshes only; same renderer and options as `render_image` |
| `derive_picture` | read | {`run`, `path`, `reused`, `kind`, `snapshot`, `triangles` or `lines` and `points`, `bounds`, `length_unit` (`"m"`), `range`, `stats`, `mesh_hash`, `bytes`}: a derived picture of a stored snapshot written to `runs/<id>/derived/<kind>-<hash>.glb` (reused for the same arguments); `kind` `iso` (`field`, `level`, `colour_by`), `streamlines` (`seeds` `{rake: {p0, p1, n}}`, `{points}` or `{patch, n}`; `direction`, `max_steps`, `max_length`, `min_speed`, `step_fraction`), `slice` (`field`, `axis`, `at`, `vectors`) or `cut_surface` (the Studio's picture: `field`, `axis`, `at`, `keep`, `vectors`); fields are the stored cell fields and `speed`; 1004 over 2 000 000 triangles, 1 000 seeds or 10 000 steps; `rcfd post derive` makes the same file without a project ([ADR 0738](adr/0738-f809-proposal-derived-pictures-of-a-stored-field-iso-surfaces-streamlines-slices-and-the-length-unit.md)) |
| `compare_runs` | read | `baseline` (the first id), `runs` [{`id`, `case`, `state`, `step`, `time`, `residual_lines`}], `metrics` {report name: {run id: value}} (as in `list_runs`), `differences` {name: {run id: value minus the baseline's}}, `metrics_total`, `metrics_omitted`; 2 to 10 runs |
| `list_changes` | read | `token`, `changes` [paths], `total`, `truncated`, and `since` when given; call without `since` to get a token (`.rcfd/` is not watched) |
| `read_feed` | read | without `since`: `cursor`, `entries` [], `total` 0 (the server's feed starts at its first call); with `since`: `since`, `cursor`, `entries` (numbered after `since`, oldest first, at most `limit`, 1-200, default 50, and within the size budget), `total`, `next` (the number to pass as `since` to continue, null when none), `dropped` (entries after `since` no longer kept; the feed keeps 1 000), `skipped_lines`. An entry has `n`, `source` (`journal`, `run` or `external`), `kind`, `t`, `seq`, `path`, `run`, `who`, `actor` {`client`, `instance` {`pid`, `started`}}, `reason`, `changes` [{`path`, `old`, `new`}], the line's other fields, and `text` (one line: who changed what and why); an entry over half the size budget comes without `changes` and the line's other fields, with `omitted` (the journal keeps the whole line). Journal entries are the lines of the project journal; run entries are a run's `state`, `warning` and `stop` events (`run_state`, `run_warning`, `run_stop`); an `external` entry is a file outside `runs/` that changed with no journal line for its new content, 500 ms after its modification ([ADR 0188](adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md) J6, J7). With `from_seq` (at least 1; not with `since`) instead: `from_seq`, `cursor`, `entries` (the journal's lines whose `seq` is at least `from_seq`, read from the start of the file, in file order, each as a journal entry without `n`; lines without `seq` are not replayed), `total`, `next_seq` (to continue, null when none are left), `skipped_lines`; then `since` = `cursor` continues live ([ADR 0195](adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md) P7) |
| `read_project_file` | read | `path`, `hash` (of the whole file; the `base_hash` for edits), `size`, `offset`, `text`, `next_offset` (null at the end); pages of at most 32 000 bytes |
| `hash_project_file` | read | `path`, `kind` (`file` or `folder`), `hash`, `bytes`, `files`: a file is `fnv64:` of its bytes, a folder (an OpenFOAM mesh) `fnv64:` over the sorted lines `<relative path>\0<file hash>\n` of every file below it, the rule of `run.json` `mesh_hash` (the runner fills `mesh` and `mesh_hash` at submit for a case with `mesh.source = "file"`); symbolic links refused; at most 4 GiB and 100 000 files (1003 names the limit); 1004 outside the project, 1202 missing; the same from the command line: `rcfd project hash <path>` ([ADR 0734](adr/0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md)) |
| `edit_case` | write_files | `path`, `written`, `valid`, `form` (`minimal`: the changed lines edited in place, comments kept; an edit that cannot be placed in the file's own lines, such as a value written over several lines, is refused with 1003 and nothing is written), `old_hash`, `new_hash`, `values` {setting: stored value}; arguments `path` (.toml or .json), `base_hash`, `set` {setting: value}, `dry_run`; when `set` holds a `mesh.*` setting the edited mesh is built and checked before anything is written (a refused mesh file 1004, a mesh that cannot be built 1003 with located `data.diagnostics`) and the result has `mesh` {`ok`, `report`, `patches`, `patches_total`} as `check_case_mesh`; a mesh that builds but fails the check is written with `mesh.ok` false ([ADR 0187](adr/0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) M7, Q2) |
| `write_note` | write_files | `path`, `created`, `old_hash`, `new_hash`; under `notes/` only; no `base_hash` means "create, the file must not exist" |
| `new_case` | write_files | `path`, `hash` (the `base_hash` for `edit_case`), `bytes`, `from` (`example <source>`, `template <name>`, `toml`, `settings` or `copy <path>`), `data_files` [{`path`, `written`}]; arguments `name` (letters, digits, `-`, `_`; not taken in `cases/` in any letter case, else 1004) and exactly one of `example` (a shipped example of the repository's `cases/`, written byte for byte with the data files it reads), `template`, `toml`, `settings` (written as `cases/<name>.json`) or `copy` (a project case, byte for byte); the case is validated before anything is written (1003 with located `data.diagnostics`) and no file is ever replaced ([ADR 0186](adr/0186-adr-0173-amendment-2-f271-registration-new-simulations-from-the-mcp-server.md) G3) |
| `check_case_mesh` | read | `case`, `mesh` {`source` `generator` with `generator`, or `file` with `file`}, `ok`, `report` (the quality report `rcfd check-mesh --json` prints), `patches` [{`name`, `faces`, `kind`}] (the names `boundaries.*` must use), `patches_total`; argument `case` (.toml or .json). The case's `mesh.file` must resolve inside the project (else 1004, located at `mesh.file`); a mesh that cannot be built is 1003 with located `data.diagnostics`. Over the size budget patches are dropped from the end, then the report's `poor_list` (`poor_list_omitted`) |
| `preview_mesh` | read | an image block (PNG) and the text {`case`, `measure`, `min`, `max`, `non_finite`, `range`, `cells`, `cells_drawn`, `axis`, `slice`, `width`, `height`, `png_bytes`, `reduced_from`?}; arguments `case`, `measure` (`non_orthogonality`, the default: the largest of the cell's faces, degrees; `skewness`, the largest of its faces; `aspect_ratio`), `width` (64-4096, default 640), `axis` (`x`, `y`, `z`) and `slice` (default: the middle of the thinnest direction, the plane `render_field` draws); colours run from the measure's ideal value (0, or 1 for the aspect ratio) to its largest value (ideal + 1 when the mesh is within 1e-9 of ideal); an image over the budget is drawn again at half the width, down to 64 pixels ([ADR 0187](adr/0187-adr-0173-amendment-3-f272-registration-mesh-set-up-from-the-mcp-server.md) M4, Q4, Q5) |
| `import_mesh` | write_files | `source`, `path` (`meshes/<name>`), `copied`, `format` (`gmsh`, `fluent` or `openfoam`), `files` [{`path`, `bytes`, `hash`, `written`}], `cells`, `faces`, `internal_faces`, `points`, `patches`, `patches_total`, `zones` [{`name`, `kind`, `size`}], `zones_total`, `bounds` {`min`, `max`}, `notes`, `mesh_file` (the `mesh.file` of a case directly in `cases/`); arguments `path` (a Gmsh or Fluent `.msh`/`.cas` file, or an OpenFOAM folder: its `polyMesh` subfolder or itself) and `name` (default the source's name; letters, digits, `-`, `_`, `.`; a file keeps its extension). Read with the readers a case's mesh build uses before anything is written (1003 when none reads it); never replaces a file (the same bytes already there: `copied` false; other bytes or a name differing only in letter case: 1004); one `mesh_imported` journal line when it wrote a file |
| `list_meshes` | read | `meshes` [{`path`, `kind` (`file`, or `openfoam` for a folder holding `points`, `faces`, `owner`, `neighbour` and `boundary`), `bytes`, `files`, `used_by` [cases whose `mesh.file` names it]}], `total`, `offset`, `next_offset`, `outside` [{`case`, `mesh_file`, `path`, `exists`, `refused`}] (case mesh files that are not an entry of `meshes/` or that leave the project; at most 20), `outside_total`, `unread` [{`path`, `message`}] (cases that are not TOML or JSON; at most 20), `unread_total`; arguments `offset`, `limit` (1-100, default 50) |
| `runner_config` | read | the run service's budgets: `path` (`.rcfd/runner-config.json`), `exists`, `hash` (`fnv64:`, null without a file: the `base_hash` of `set_runner_config`), `values` (the keys the file sets), `problem` (why the runner would refuse the file, or null), `needs_replace` (the runner cannot read the file as it is: not a JSON object, an unknown key), `defaults` (the runner's defaults on this machine), `in_use` {`values`, `source`, `started`} (what the runner running now started with, from `.rcfd/runner.json`; null when none runs), `applies` (`next_start`); no arguments ([ADR 0733](adr/0733-f837-proposal-set-runner-config-the-runner-writes-its-own-budgets.md)) |
| `set_runner_config` | write_files | the budgets as `runner_config` gives them, after the write; arguments `set` {key: number, or null for the runner's default} (keys `max_runs`, `cpu_threads`, `mem_budget_mb`, `mem_base_mb`, `mem_kb_per_cell`, `poll_ms`, `adopt_probe_ms`, `cancel_grace_ms`), `base_hash` (required: the hash from `runner_config`, or null when there is no file), `replace` (write exactly `set`, every key given, over a file the runner cannot read; without it such a file is refused), `reason`. The runner's own checks (`rcfd_runner::runner::Config`; refused: 1003 with `data.key`, nothing written), written with `replace_if_hash` under the file's edit lock (1200 with `data.current_hash` when the file changed since `base_hash`), one journal line with each key's old and new value. The runner reads the file when it starts: the change applies at the next runner start, and the runner is not restarted. The same from the command line: `rcfd-runner config` and `rcfd-runner config set <key>=<number>... [--unset <key>]... [--base <hash>] [--replace]` (exit 1 refused, 4 conflict) |
| `case_facts` | read | what a project case's time step depends on, as the service's `case.facts` ([service protocol](service-protocol.md)) for the file: the facts, `run_time` (the plan with the `run_time` argument, as `start_run` takes it), `memory` with the project's budgets (`budgets: "project"`); arguments `case`, `run_time`; 1003 when the case does not build ([ADR 0737](adr/0737-f837-proposal-case-facts-and-a-dry-run-start-a-mesh-refine-and-saving-a-case-outside-a-project.md)) |
| `refine_case_mesh` | write_files | Refine x2 of the case's generator mesh, written through `edit_case`'s rule: `edits`, `cells`, `cells_after`, `two_d`, `memory_after`, `case`, `written` (`edit_case`'s result); arguments `case`, `factor` (2), `base_hash`, `reason`; 1009 with the reason for a mesh file, 1004 with the reason for `axial_fan_sector` and a count over its maximum (nothing written), 1200 on a changed file. `rcfd check <case> --refine 2` prints the same edits without writing |
| `list_sites` | read | [ADR 0739](adr/0739-f848-proposal-run-sites-a-remote-simulation-service-for-the-studio-cli-and-mcp.md): the run sites of the user's sites file (`RCFD_SITES_FILE`, else `%APPDATA%
cfd\sites.toml` or `~/.config/rcfd/sites.toml`): `file`, `sites` [{`name`, `kind`, `host`, `engine`, `work`, `budgets`, `assistant`, `confirmed`, `accept_engines`}]; nothing is started and nothing is sent; 1003 when the file does not read. `rcfd site list --json` prints the same |
| `start_run` | execute | ADR 0737: `run_time` {`simulate_s`, `courant`, `max_dt`, `mode`, `adaptive_correctors`} puts the engine's time-step entries before the store plan's and `set`'s, and adds `run_time` (the plan) to the reply; `dry_run: true` writes nothing and returns {`dry_run`, `case`, `set`, `store`, `run_time`, `check` (the check-only reply of the case with those entries), `memory_mb`, `memory`, `runner` {`running`, `would_start`}}. With `from_run`, `run_time` and `dry_run` are -32602 too (a rerun keeps its run's settings). Otherwise: `run`, `case`, `state`, plus `created` (the `new_case` result) when it made the case and `store` {`applied`, `set`, `snapshots`, `estimated_bytes`, `note`} unless `store` is `none` (`applied` false with the reason in `note` when the case or a `set` entry gives the store; a case file that does not read as a case is refused with 1003 before anything is queued); arguments `case` or `new_case` (an object with `new_case`'s arguments: the case is made first, then run, in one call), `set` [`path=value`], `priority`, `threads`, `mem_mb`, `continue_from` (a run id of the project: the run starts from that ended run's `output/restart.rcfd`), `max_steps` (at least 1), `from_run` (instead of `case` or `new_case`: a new run of that run's `case.snapshot` with its `run.json` settings and `case_dir`, `priority`, `threads` and `mem_mb` overriding them; the reply has `rerun_of`, and so does the new `run.json`; -32602 with `case`, `new_case`, `set`, `store`, `continue_from` or `max_steps`, 1202 no such run, 1003 its settings do not read; [ADR 0734](adr/0734-f828-proposal-run-records-launch-argv-build-id-time-unit-rerun-and-mesh-hash.md); the same from the command line: `rcfd-runner rerun <id>` and `rcfd-runner start ... --continue-from <id> --max-steps <n>`), `store` (`plan`: the Studio's field-store proposal of [ADR 0140](adr/0140-wave48g3-store-fields-by-default.md), about 100 snapshots in f32, put before `set` unless the case or a `set` entry gives `output.store_interval` or `output.store_time_interval`; `none`: only what the case asks for; default `plan`, also for an existing `case` since [ADR 0741](adr/0741-f271-registration-the-field-store-plan-as-the-default-of-start-run.md), on the maintainer's approval of ADR 0186 Q2 (a); `none` is the way back to a run that stores only what its case asks for; the tool's own description of `store` says so since [ADR 0774](adr/0774-f862-registration-the-description-of-start-run-store-argument.md)); `site` (ADR 0739: a site of `list_sites` with `assistant = true` that the user confirmed with `rcfd site check <name> --confirm`; the run's inputs are uploaded there, `run.json` has `site`, the run folder here is kept up to date from the site and `site.json` and `site.jsonl` say where and how; a site without `assistant = true`, an unknown one or one not confirmed is refused before anything is written or sent, code 1002; the reply has `site`); starts the project's `rcfd-runner` when none is alive, with its output appended to `.rcfd/runner.log`; a runner that exits before it holds the project's lock gives an error result at once (code -32000, `data.run` the queued run) whose message has the runner's exit code and the end of that log ([ADR 0146](adr/0146-job48l-runner-output-early-exit-windows-inherited-handles.md)) |
| `pause_run`, `resume_run`, `cancel_run` | execute | `run`, `requested`, `state`; 1002 for a run that already ended |
| `list_trash` | read | the project trash (`.rcfd/trash/`), newest first: `items` [{`token`, `kind` (`run`, `results` or `case`), `path`, `parts`, `trashed`, `who`, `reason`, `hash` (case files only), `bytes`, `actor`}], `bytes` (all items), `total`, `offset`, `next_offset`, `skipped` [{`path`, `message`}]; arguments `offset`, `limit` ([ADR 0736](adr/0736-f829-proposal-a-project-trash-with-restore-and-compare-and-set-for-settings-set.md)) |
| `trash_run` | write_files | moves an ended run's folder `runs/<id>/` into the trash: `token`, `kind`, `path`, `bytes`; argument `run`. 1002 when the run has not ended; 1003 with `data.runs` while an unfinished run continues from it (`settings.continue_from`); one journal line `trashed` |
| `trash_results` | write_files | moves a run's `fields/` and `output/` (argument `parts`, required, from those two; a part the run does not hold is refused) into the trash; the run's record, log and residuals stay; argument `run` |
| `trash_case` | write_files | moves a case file under `cases/` into the trash; arguments `path` and `base_hash` (required: the file's hash from `read_project_file`; 1200 with `data.current_hash` when it changed); 1004 outside `cases/` or for a folder, 1202 when absent |
| `restore_trash` | write_files | moves a trashed item back where it was: argument `token`; 1200 with `data.path` when that path is taken now, 1202 for no such token, -32602 for a malformed one; one journal line `restored` |
| `empty_trash` | delete | removes trashed items for good: `removed` [tokens], `count`, `bytes`; arguments `tokens` (default: all), `older_than_days`; one journal line `trash_emptied`. There is no automatic emptying. The same from the command line: `rcfd project trash`, `trash-list`, `restore`, `empty-trash` |
| `wait_run` | read | `run`, `from`, `state`, `changed`, `terminal`, `waited_s`, `status`; returns when the state differs from `from` (default: the state at the call) or the run ended, at the latest after `timeout_s` (default 30, at most 60) |
| `show_view` | write_files | [ADR 0805](adr/0805-f976-registration-a-view-specification-and-show-view.md): shows a view in the Studio that has the project open (Claude's canvas, guardrail G23): `shown`, `id` (the hash of the written file), `path` (`.rcfd/views/current.json`), `hash`, `studios` (how many Studios have the project open), `panels`; arguments `view` (the specification below) and `reason`. The view is checked against the project first: an invalid one is error 1003 with `data.errors` [{`key`, `reason`}] (every error, at most 20; `errors_total`) and nothing changes; with no Studio open, error 1002 says so and nothing is written. Otherwise the file is written and one journal line `view_shown` (`path`, `hash`, `panels` the kinds, `studios`) tells every open Studio, through its change feed, to show it |
| `read_view` | read | the view shown last: `exists`, `path`, `id`, `hash`, `shown`, `actor`, `reason`, `view`; `exists` false and `view` null when none was shown |
| `export_view` | write_files | [ADR 0823](adr/0823-f980-registration-a-view-exported-as-one-self-contained-html-page.md): writes a view as one self-contained HTML page: `exported`, `id`, `path`, `hash`, `bytes`, `panels`; arguments `view` (as `show_view` takes it; a view of a run is written as its category's layout; default the view shown last, 1202 when none was shown), `out` (`exports/<name>.html`, default `exports/view-<id>.html`; 1003 at `out` otherwise) and `reason`. The page has no script and fetches nothing (a Content-Security-Policy of `default-src 'none'`); it carries the Studio's stylesheets (`tokens.css`, `base.css`, `canvas.css`, `export.css`) inline, draws fields and a case's mesh as inline SVG of the store's surface in the Studio's colour table, plots as inline SVG lines, and names in each panel the files it read with their hashes and the engine version. An invalid view or `out` writes nothing; no Studio need be open. One journal line `view_exported` (`path`, `hash`, `bytes`, `panels`) |

The view specification of `show_view` (`rcfd-view`, version 1; checked by `crates/rcfd-workspace/src/view.rs`) is
`{version: 1, title?, layout?: {columns}, panels}`: `title` 1 to 200 characters, `columns` 1 to 4 (default the smaller of
2 and the number of panels; panels fill the grid row by row), `panels` 1 to 8, each `{kind, title?}` with the keys of its
kind and no other: `case` {`case`: a `.toml` or `.json` file under `cases/`}, `run` {`run`}, `field` {`run`, `field` (stored
in the run's field store), `snapshot`? (an index; default the last)}, `mesh` {`mesh`: an entry of `list_meshes`}, `plot`
{`run`: its residuals}, `monitor` {`run`, `monitor`: a name of its `monitors.<name>` series}, `table` {`runs`: 1 to 10
distinct run ids}. Each run, case, mesh, field, snapshot and monitor must exist. An error's `key` is the dotted path of
the value (`version`, `layout.columns`, `panels[2].kind`, `panels[1].colour` for a key that is not allowed). The Studio
draws each kind with its existing view: the case's and the mesh's rows of the project home, the run workspace, the field
view, the residual and monitor plots of the time panel, the project home's rows of the runs.

Which Studios have the project open: the Studio starts its reading server with `--canvas` (with `--project` only; without
`--project` the server exits 2), which registers the process in `<temp>/rcfd-canvas/<hash of the project root>/<pid>.json`
while it runs; `show_view` counts the files whose process is alive (`rcfd-mcp`) and removes the others. Nothing of this is
in the project folder. A Studio that opens the project later shows the last view (`read_view`).

On Windows the runner that `start_run` starts (and the solvers it starts) also inherit `rcfd-mcp`'s standard input and output
pipes, because std starts processes with handle inheritance on and the workspace allows no unsafe code. Measured: after the
server has exited, its standard output reaches EOF only when that runner exits ([ADR 0146](adr/0146-job48l-runner-output-early-exit-windows-inherited-handles.md)
C2). What this does to Claude Code when it closes or restarts the server has not been checked. On Linux the pipe closes with the
server.

Edits (`edit_case`, `write_note`) are compare-and-replace under a cooperative lock (`rcfd_project::atomic::replace_if_hash`). The
file is replaced only if it still has `base_hash`, while `.rcfd/locks/<hash of the path>.lock` is held. Writers that do not take the
lock (an editor, a shell command, Claude Code's own Edit and Write tools outside the folders the plugin's hook guards) are not
stopped; the feed reports their edits afterwards as `external`.

The project journal, `.rcfd/edits.jsonl`, has one JSON line per accepted write, case creation, mesh import, run submission,
stored design evaluation and accepted run request, from the MCP server and from the Studio alike ([ADR 0188](adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md)).
Each line has `t`, `kind`, `who` (the command), `seq` (one more than the last, given under the short lock
`.rcfd/locks/journal.lock`; lines written before ADR 0188 have none), `actor` {`client` (`mcp` or `studio`), `instance` {`pid`,
`started`}} when the writer has one, and `reason` when given. By kind:

- `edit`: `path`, `old_hash`, `new_hash`, and `changes` [{`path`, `old`, `new`}]: for `edit_case` each setting set, for a Studio
  save every setting whose value changed (values as the case reads them, defaults included). At most 100 entries
  (`changes_total` when cut); a value over 1 000 bytes of JSON is written as {`bytes`, `hash`}.
- `case_created`: `path`, `hash`, `bytes`, `from`, `data_files` [{`path`, `hash`, `written`}].
- `mesh_imported`: `path`, `source`, `format`, `files` [{`path`, `bytes`, `hash`}], `cells`, `faces`, `patches` (their count).
- `run_submitted`: `run`, `case`, `set`, `priority`, `threads`, `mem_mb`, `continue_from` and `max_steps` when set; for a run of
  `design_run_study` or `design_evaluate` also its tags `study`, `variant` and `condition`
  ([ADR 0794](adr/0794-f274-registration-journal-lines-of-design-runs-and-records.md)).
- `run_control`: `run`, `requested` (`pause`, `resume` or `cancel`).
- `design_recorded` (ADR 0794): one line per evaluation a design tool stored. A record of a study: `path`
  (`<study>/evaluations.jsonl`), `hash` (of that file up to the end of the record's line), `study`, `variant`, `index`, `source`
  (`evaluated` or `recorded`), `ok`, `conditions` [{`condition`, `status`}]. A point of an operating map: `path`
  (`<out_dir>/map.json`), `hash`, `files` [{`path`, `hash`}] (`map.csv`, and `map.meta.json` in the last one), `point`, `label`,
  `ok`, `status`. `ok` is false for a run that failed or did not converge.
- `design_created` (ADR 0794): `path`, `hash`; for a study (`path` is its `study.json`) also `study`, and `files` when it was made
  from a session.
- `stale_lock_broken`: `path`, `age_s` (an edit lock older than 30 s was removed).
- `edit_lock_remove_failed`: `path`, `error`, `attempts` (the edit was written, but its edit lock could not be removed after
  retrying a transient error, for example a handle another program holds on the lock file; the lock is left until it is 30 s
  old, [ADR 0676](adr/0676-f755-registration-a-forced-control-of-the-edit-locks-starvation-and-of-a-lock-left-behind.md)).

Which Claude Code session an `mcp` actor belongs to is not recorded: no way to link the server's process to a session has been
checked (ADR 0188 Q4).

### Project mode: the session tools that are not offered

With `--project`, a session tool whose every use needs a session of the server's memory, or makes one, is neither listed nor
callable (a call is the JSON-RPC error -32602 `unknown tool`), because in project mode cases, meshes and runs live in the
project folder ([ADR 0195](adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md) P1).
The 19 others (`list_cases`, `check_case_headless`, `read_status_stream` and the design, ledger and study tools) stay, with the same
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

The desktop Studio (`apps/studio-web/`) embeds no Claude Code and keeps no allow-list (ADR 0801 Addendum 23; its Claude
Code panel and the allow-list file `.rcfd/claude-allow.json` it read were removed). Its Use with Claude Code registers
this server for the project folder with Claude Code itself (`claude mcp add rcfd-project --scope local -- <rcfd-mcp>
--project <folder>`), with full access; Claude Code's own permission prompts govern each call. The removed egui Studio
`rcfd-studio` (F-449) wrote that file with the 15 tools of its day when a project had none (ADR 0195 P5).

`--client <name>` names the actor client of the project journal's lines (default `mcp`; 1 to 32 of `a-z`, `0-9`, `_`,
`-`). The Studio's own servers run with `--client studio`, so a change Claude Code makes through its server (client
`mcp`) can be told from the Studio's own in the change feed (`read_feed`, each entry's `actor.client`). The default
changes nothing.

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
