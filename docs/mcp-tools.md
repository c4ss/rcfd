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

## Where to go next

- [Design loop](design-loop.md): the study, map and ledger formats the design tools read and write.
- [Run control](run-control.md): exit codes and the status stream behind the headless tools.
- [Service protocol](service-protocol.md): the methods, error codes and sandbox rules of the service.
