# Headless launch, run control and workflows (F-120, F-121)

This page is the contract for scripts, CI jobs, the service, the MCP server and the assistant.
Everything here is implemented in `rcfd-api` (`runctl`, `workflow`) and shared by the CLI, the
service (`docs/service-protocol.md`) and the MCP tools, so all of them behave the same way.

## `rcfd run`

```
rcfd run <case> [--out dir] [--max-iterations n] [--quiet]
         [--check-only]
         [--status-json] [--status-file file] [--status-every n]
         [--continue-from checkpoint] [--checkpoint file] [--checkpoint-every n]
         [--stop-file file] [--max-elapsed seconds]
         [--threads n] [--precision double] [--journal file]
```

| flag | meaning |
| --- | --- |
| `--check-only` | validate the settings, build the simulation, check the mesh quality, print the result and exit without running |
| `--status-json` | write the status stream (below) to stdout, one JSON object per line; all human text goes to stderr |
| `--status-file f` | additionally write the stream to a file (for another process to tail) |
| `--status-every n` | iteration event every n steps (default 10); iterations 1-5 and the last one are always reported |
| `--continue-from f` | restore a checkpoint and continue; the iteration counter and the iteration cap continue from the checkpoint |
| `--checkpoint f` | where to write the checkpoint (default `<out>/restart.rcfd`); written when the run is interrupted, hits the time limit or the step cap, and at the end |
| `--checkpoint-every n` | also write it every n steps |
| `--stop-file f` | the run stops cleanly before the next step when this file exists |
| `--max-elapsed s` | stop cleanly after s wall-clock seconds |
| `--threads n` | number of worker threads (`RAYON_NUM_THREADS`); results are bit-identical for any thread count (measured, see below) |
| `--precision double` | the only supported value; `single` is rejected with exit 2 |
| `--journal f` | run the case recorded in a service journal (`journal.get`) instead of a case file |

`--hostfile` is rejected with exit 2: there is no distributed-memory run yet (F-100).

Results (`solution.vtu`, `monitors.json`, `monitors.csv`, `report.html`) are written to `--out`
also after an interrupted or failed run.

### Exit codes

| code | meaning | `state` in the final event |
| --- | --- | --- |
| 0 | converged, or a transient run reached its end time (also `--check-only` success) | `converged`, `finished` |
| 1 | runtime or I/O failure (including an unusable checkpoint) | `failed` |
| 2 | usage error | |
| 3 | invalid case or settings | |
| 4 | the mesh failed its quality check | |
| 5 | steady run stopped at its iteration cap without meeting the tolerances | `not_converged` |
| 6 | residuals became non-finite | `diverged` |
| 7 | interrupted through the stop file | `interrupted` |
| 8 | `--max-elapsed` reached | `time_limit` |

## Status stream

Every line is a JSON object with `format: "rcfd-status"`, `version: 1`, `event`, `seq` (1, 2, 3, ...
within one launch) and `wall_s`. Events:

- `start`: `cells`, `algorithm`, `steady`, `start_iteration`, `case_hash`, `info` (`driver`, `threads`, `precision`, `continue_from`)
- `iteration`: `index`, `time`, `res_u`, `res_p`, `continuity`, `done`
- `checkpoint`: `path`, `iteration`, `why` (`periodic`, `interrupted`, `time_limit`, `step_limit`, `converged`, ...)
- `warning`: `message` (for example a run that cannot be checkpointed)
- `check` (only from `--check-only`): `ok`, `exit_code`, `cells`, `diagnostics`, `mesh` (quality report)
- `finish`: `state`, `exit_code`, `ok`, `iterations`, `time`, `reason`, `elapsed_s`

The MCP tool `read_status_stream` (service method `status.read`) summarises such a file; the
tool `run_headless` (service method `run.headless`) produces the same stream through the service.

## Checkpoints

A checkpoint is a checksummed `rcfd-io` container holding velocity, pressure, face fluxes, the
iteration counter and (with the energy equation) temperature, tied to the mesh by its content hash.
It is available for **steady SIMPLE/SIMPLEC laminar runs, with or without the energy equation**.
Transient, compressible and turbulence-model runs report a `warning` event instead of writing one,
and `--continue-from` fails with exit 1. A checkpoint of a different mesh, or a damaged file, is
refused.

Measured on a 12x12 lid-driven cavity (tolerances 1e-7): a run stopped after 30/40 iterations and
continued from its checkpoint stops after the same 111 iterations as the uninterrupted run, and the
solution files are byte-identical; 25 steps after a restore are bit-identical to 25 uninterrupted
steps. `--threads 1` and `--threads 4` give byte-identical solution files. Note that the residual
normalisation history restarts at the restore, so for runs that are restored in the middle of a
transient phase the stopping iteration is not guaranteed to be identical.

## Workflows

A workflow is an ordered list of tasks with dependencies over a session (`workflow.*` service
methods, `rcfd workflow`, and the MCP tools `create_workflow`, `list_workflow_tasks`,
`run_workflow_task`, `edit_workflow`).

Task kinds: `check_mesh`, `validate`, `set {path, value}`, `initialize`, `solve {max_steps?}`,
`report {format?, path?}`, `export_vtk {dir?, name?}`. Templates: `mesh_to_results`
(mesh_check, setup, initialize, solve, report), `channel_flow` (the same plus `export_results`) and
`empty`. A task has the state `not_run`, `done`, `failed` or `stale`.

- Running a task also runs the prerequisites that are not `done` (unless `with_prerequisites` is false, then
  an unfinished prerequisite is an error). Running without a task runs everything outstanding.
- The run stops at the first failing task; its message and result say why. A `solve` task counts as done
  only when the run converged (steady) or reached its end time (transient); stopping at a step cap fails it.
- Re-running a task marks the tasks that depend on it `stale`. Any change of a setting outside the
  workflow (`settings.set`/`settings.remove`) marks every finished task `stale`. A `set` task changes the case
  under every other result, so it makes all other tasks stale and stays `done` itself.
- Insert and delete validate the result: unique ids, known kinds, existing dependencies, no cycles; a task other
  tasks depend on cannot be deleted.
- Documents are `{format: "rcfd-workflow", version: 1, name, tasks}`; loaded documents never claim results.

`rcfd workflow run <case> [--template t | --workflow file] [--task id] [--no-prerequisites] [--max-steps n]`
exits with the code of the failing task: 4 (mesh check), 3 (validation), the run code of a failing solve
(5 when it stopped at the step cap), 1 otherwise.

## Not implemented / deviations

- No control-port socket as such: remote control of a running job is the service (`rcfd serve`,
  `run.pause/resume/stop/status`, `events.poll`), which also runs the same drivers.
- No signal handler: interrupt a CLI run with `--stop-file` (a clean stop with checkpoint and results);
  killing the process loses the unwritten results.
- No `--hostfile` / distributed runs, no single precision.
- Checkpoints cover steady laminar SIMPLE/SIMPLEC only (see above).
