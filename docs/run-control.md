# Headless launch, run control and workflows (F-120, F-121)

This page is the reference for launching and controlling runs without the desktop app: the command-line flags, exit codes, the
status stream, checkpoints and restarts, output policy, workflows and studies. It is the contract for scripts, CI jobs, the
service, the MCP server and the assistant. Several parts are not delivered yet; they are listed in "Not implemented / deviations"
at the end. For a first run step by step, follow the [command-line quick start](tutorials/quick-start-cli.md); for the keys of the
case file itself, see the [case schema](case-schema.md).

Everything here is implemented in `rcfd-api` (`runctl`, `workflow`) and shared by the CLI, the
service (`docs/service-protocol.md`) and the MCP tools, so all of them behave the same way.

On this page:

- rcfd run: flags and exit codes
- Status stream
- Checkpoints
- Chains
- Setup check
- Output policy and statistics, with the field store
- Workflows
- Parametric studies
- Not implemented / deviations

## `rcfd run`

```text
rcfd run <case> [--out dir] [--max-iterations n] [--quiet]
         [--check-only]
         [--status-json] [--status-file file] [--status-every n]
         [--continue-from checkpoint] [--checkpoint file] [--checkpoint-every n]
         [--stop-file file] [--pause-file file] [--case-dir dir] [--store-dir dir] [--status-monitors]
         [--max-elapsed seconds]
         [--threads n] [--precision double] [--journal file]
         [--set path=json]... [--print-settings] [--restart-from file] [--source-case case] [--method m] [--no-chain]
rcfd restart <case> --from file [--mesh mesh] [--source-case case] [--method m]   (plus every run flag)
rcfd check <case> [--json] [--set path=json]...
```

| flag | meaning |
| --- | --- |
| `--check-only` | validate the settings, build the simulation, check the mesh quality, print the result and exit without running |
| `--status-json` | write the status stream (below) to stdout, one JSON object per line; all human text goes to stderr |
| `--status-file f` | additionally write the stream to a file (for another process to tail) |
| `--status-every n` | iteration event every n steps (default 10); iterations 1-5 and the last one are always reported |
| `--continue-from f` | restore a checkpoint and continue; the iteration counter and the iteration cap continue from the checkpoint |
| `--interpolate-case c --interpolate-checkpoint f [--interpolate-mode conservative\|nonconservative]` | initialise from a solution of another mesh: `c` is the source case, `f` a checkpoint written for it (F-115). Each cell takes the nearest source cell value plus the least-squares gradient times the offset, clipped to the neighbour range; `conservative` adds one uniform shift so the volume average equals the source's. Velocity, pressure and temperature only; steady laminar SIMPLE/SIMPLEC; not combinable with `--continue-from` (exit 2); a checkpoint of another mesh is exit 1 |
| `--checkpoint f` | where to write the checkpoint (default `<out>/restart.rcfd`); written when the run is interrupted, hits the time limit or the step cap, and at the end |
| `--checkpoint-every n` | also write it every n steps |
| `--stop-file f` | the run stops cleanly before the next step when this file exists |
| `--pause-file f` | hold between two steps while `f` exists, writing `f.ack` (the iteration count) while held; the stop file ends the hold. Paused time counts towards `--max-elapsed` and `wall_s`. Plain runs only (not chains). Used by the run service (ADR 0123) |
| `--case-dir d` | read the case from its text whatever the file name (JSON when the text starts with `{`, TOML otherwise) and resolve its relative paths against `d`; the run service runs `runs/<id>/case.snapshot` this way |
| `--store-dir d` | write the field store (Field store, below) to `d` instead of `<out>/fields`; only used when the case turns the store on. The run service passes `runs/<id>/fields` (ADR 0130) |
| `--status-monitors` | add `monitors`, the value of every report of the case at that step (`{"lid_drag": 0.31}`), to each `iteration` event; the values are the ones `monitors.json` records, a report that could not be evaluated at that step is left out. Without the flag the events are unchanged. Plain runs only (not chains). The run service passes it and forwards the values into `residuals.jsonl` (ADR 0130) |
| `--max-elapsed s` | stop cleanly after s wall-clock seconds |
| `--threads n` | number of worker threads (`RAYON_NUM_THREADS`); results are bit-identical for any thread count (measured, see below) |
| `--precision double` | the only supported value; `single` is rejected with exit 2 |
| `--journal f` | run the case recorded in a service journal (`journal.get`) instead of a case file |
| `--set path=json` | override one setting before anything else (repeatable): `--set solver.dt=0.05`, `--set solver.algorithm=piso` (a bare word is a string). Malformed argument: exit 2; unknown path or invalid value: exit 3 |
| `--print-settings` | print the resolved settings (defaults filled in, overrides applied) as JSON and exit 0 without running or writing anything |
| `--restart-from f` | start from the solution in restart file `f`; on another mesh the fields are transferred (`--method`, default `gradient`; `--source-case` for files that do not embed their mesh). `rcfd restart <case> --from f` is the same; its `--mesh m` replaces the case mesh by a mesh file. Not combinable with `--continue-from` (which restores the exact state) |
| `--no-chain` | ignore `chain.sequence` and run the base case alone |

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
| 6 | residuals became non-finite, or a divergence guard stopped the run (PISO/PIMPLE, ADR 0067; steady, ADR 0077) | `diverged` |
| 7 | interrupted through the stop file | `interrupted` |
| 8 | `--max-elapsed` reached | `time_limit` |

## Status stream

Every line is a JSON object with `format: "rcfd-status"`, `version: 1`, `event`, `seq` (1, 2, 3, ...
within one launch) and `wall_s`. Events:

- `start`: `cells`, `algorithm`, `steady`, `start_iteration`, `case_hash`, `info` (`driver`, `threads`, `precision`, `continue_from`)
- `iteration`: `index`, `time`, `res_u`, `res_p`, `continuity`, `done`; with `--status-monitors` also `monitors` (report name to
  value, `null` when not finite)
- `checkpoint`: `path`, `iteration`, `why` (`periodic`, `interrupted`, `time_limit`, `step_limit`, `converged`, ...)
- `snapshot`: `path`, `index`, `time` (a numbered result file, see Output policy)
- `warning`: `message` (for example a run that cannot be checkpointed); a setup-rule warning also has `rule` (id), `path` and `category`; a
  pressure-lag warning of PISO/PIMPLE (ADR 0077, `solver.pressure_lag_check = "warn"`) also has `kind = "pressure_lag"`, `step`, `rho`, `cell`, `position`
- `check` (only from `--check-only`): `ok`, `exit_code`, `cells`, `diagnostics`, `rules` (setup-rule diagnostics, `id severity category path message [fix]`), `mesh` (quality report)
- `finish`: `state`, `exit_code`, `ok`, `iterations`, `time`, `reason`, `elapsed_s`; a run stopped by a divergence guard (state `diverged`) also has
  `divergence` (ADR 0077): `solver` (`steady` or `transient`), `kind` (steady: `non_finite`, `continuity_growth`, `velocity_runaway`, `pressure_runaway`;
  transient: `non_finite`, `courant`, `lag_amplification`), `quantity`, `iteration` (steady) or `step`, `outer`, `stage` (transient), `cell`, `position`,
  `value`, `limit`, `last_good` (steady: the iteration with the smallest continuity before the trip; transient: the previous step). `summarize_stream`
  passes it on as `divergence`.

The MCP tool `read_status_stream` (service method `status.read`) summarises such a file; the
tool `run_headless` (service method `run.headless`) produces the same stream through the service.

## Checkpoints

A checkpoint (restart file) is a checksummed `rcfd-io` container holding velocity, pressure, face fluxes, the
iteration counter and (with the energy equation) temperature, tied to the mesh by its content hash. Since ADR 0031 it also
holds the mesh itself (section `mesh`) and, for transient runs, the time, the step counter and the BDF2 history
(`meta.engine` is `steady` or `transient`; old files stay readable).
It is available for **steady SIMPLE/SIMPLEC laminar runs, with or without the energy equation**, and for **transient
laminar constant-density PISO/PIMPLE runs without energy equation, scalars, statistics, frozen flow, solids, rheology
or user functions**. Other runs (compressible, turbulence models, energy or scalars in transient runs) report a `warning`
event instead of writing one, and `--continue-from` fails with exit 1. A checkpoint of a different mesh, or a damaged file, is
refused by `--continue-from`; `--restart-from` transfers the solution onto the new mesh instead. A transient restart on the
same mesh continues bit for bit like the uninterrupted run (measured: PISO Euler, PISO BDF2, PIMPLE BDF2); after a mesh change
the time is kept but the BDF2 history starts fresh.

Field transfer methods (`rcfd-post::remap`): `nearest`, `inverse_distance`, `least_squares`, `gradient`, `conservative` and
`conservative_overlap`; measured orders and conservation are in the addendum of `docs/adr/0031-wave19c-restart-chain.md`.

Measured on a 12x12 lid-driven cavity (tolerances 1e-7): a run stopped after 30/40 iterations and
continued from its checkpoint stops after the same 111 iterations as the uninterrupted run, and the
solution files are byte-identical; 25 steps after a restore are bit-identical to 25 uninterrupted
steps. `--threads 1` and `--threads 4` give byte-identical solution files. The residual
normalisation history restarts at the restore, so for runs that are restored in the middle of a
transient phase the stopping iteration is not guaranteed to be identical.

## Chains

`rcfd run` on a case with `chain.sequence` runs the stages in order (case-schema.md, `chain`). Every status event carries a
`stage` field; additional events: `stage_start` (`stage`, `index`, `of`), `restart` (`stage`, `report`), `stage_finish`
(`stage`, `exit_code`, `state`, `accepted`) and a closing `chain_finish` (`ok`, `exit_code`, `state`, `stages_run`, `stages`). The
exit code is that of the first stage that did not end with 0 (a steady stage at its iteration cap, exit 5, is accepted with
`allow_unconverged`). `summarize_stream` is unchanged and reports the last `finish` event, which is that of the last stage run.

## Setup check

`rcfd check <case> [--json]` validates the settings, builds the simulation, checks the mesh and evaluates the setup rules; exit 0
(also with warnings and infos), 3 (invalid case or an error-level rule) or 4 (mesh failed). `--json` prints one object with the keys
`ok`, `exit_code`, `diagnostics`, `rules`, `mesh`. `rcfd validate` prints the rule diagnostics too; `rcfd run` stops with exit 3 before
solving on an error-level rule and reports warnings as `warning` events. For a chain, `rcfd check` checks the base case; each stage is
checked when it starts.

## Output policy and statistics (F-116)

Settings of the case, honoured by `rcfd run` and by `run.headless` with an `out_dir`:

- `output.write_interval = N`: every N steps write `<out>/snapshots/snapshot_<step>.vtu` (and a `snapshot` event);
  `output.retain_snapshots = K` keeps only the newest K (0 keeps all).
- `output.retain_checkpoints = K` (backup retention, default 1): with `--checkpoint-every n` and K > 1 the periodic
  restart files are `<checkpoint>.<step>` and only the newest K remain; the end-of-run, interrupt and time-limit
  checkpoint is always written to `<checkpoint>` itself. K = 1 keeps the old behaviour (one file, overwritten).
- `output.fields = ["U", "p", ...]` limits the variables written to `solution.vtu` and the snapshots (empty = all;
  `statistics` selects all statistics fields, single ones such as `ux_mean` are accepted).
- `statistics.enabled` (transient piso/pimple runs): time-weighted mean, rms, std, min and max of `statistics.fields`
  (default `ux uy uz p`; also `speed`) from `statistics.start_time`. Fields `<name>_mean/_rms/_std/_min/_max` appear
  in the VTK files and can be probed and reported like any other field. Definitions are in `crates/rcfd-api/src/stats.rs`
  (end-of-step values weighted by the step width after the start time).
- The wall-clock limit (`--max-elapsed`) and the stop file (`--stop-file`) are the F-121 controls described above and
  work for transient runs too.

Not covered: statistics are not part of a restart file (a transient run with statistics.enabled cannot be restarted), no
statistics for the compressible solver or steady runs, no cumulative time-averaging restarts, no time-based
(rather than step-based) write interval.

### Field store (ADR 0124)

Off by default. With `output.store_interval = N` (steps or iterations) or `output.store_time_interval = dt`
(simulated seconds, transient runs only) and an output directory, `rcfd run` and `run.headless` (any caller of
`runctl::drive` that passes an output directory) add a snapshot of the cell fields `output.store_fields` (default
`ux uy uz p`) to `<out>/fields/store.rts` (`--store-dir d`: `d/store.rts`), and the final state when the run ends. `store_time_interval` stores the
first step at or after each multiple of `dt`; it does not change the time steps (unlike `output.write_time_interval`).
The store keeps the mesh once (with its hash), each snapshot field by field in blocks with checksums; a run that is
killed leaves every earlier snapshot readable. The Rust reader is `rcfd_tsstore::Reader` (by step or time, nearest,
floor or linear in time, one field or a cell subset). Units are recorded for velocity (`m/s`) and `T` (`K`); other
fields carry `unknown`. `output.store_precision` is `"f64"` (default: every value bit-exact) or `"f32"` (opt-in, ADR
0133: each value rounded to the nearest single precision, half the size, error at most `|x| 2^-24 + 2^-150`; the run
itself is unchanged; a finite value beyond the single-precision range disables the store with a warning). The Studio
proposes `"f32"` for the runs it starts. A fresh run replaces a store left in the same output directory; a continued run appends.
Located errors: `store_time_interval` on a steady algorithm, either key with `dynamic_mesh.enabled`. An unknown
field name gives a warning and no store. Library callers that step a `Simulation` themselves are not covered.

Runs the Studio starts (Run, F5, Runs > Start run...) store fields unless the case sets its own store, and a store key
counts as the case's own only when its value differs from its default: `store_interval` 0, `store_time_interval` 0.0,
`store_fields` `[]`, `store_precision` `"f64"` (ADR 0152). A case saved by the Studio, or written by `rcfd-api`'s writer,
spells out every key at its default, so an explicit `store_interval = 0` cannot be told from the default and does not
turn the Studio's store off. To start a run without stored fields, untick "Store fields" in Runs > Start run... (or give
the override `output.store_interval=0` there); for an explicit `"f64"`, pick "f64 (exact)" there. Run and F5 always store.
`rcfd run`, `rcfd-runner submit` and Claude Code's `start_run` read the case as written: 0 is off.

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

## Parametric studies

`rcfd study <case>` runs the design points of the `study` section, each as an `rcfd run` of the base case with settings overrides, and
exits with the code of the lowest-index failed point (the per-run codes above) or 3 for an invalid sweep. See `docs/studies.md`.
The Python package (`docs/python.md`) drives `run`, `check` and `study`.

## Run service (`rcfd-runner`)

The Studio, the `rcfd-mcp` project tools (`start_run`) and `rcfd-runner start` and `ensure` queue runs in a project folder. When
no runner holds the project's lock they start one detached `rcfd-runner serve`, which runs each queued run as an `rcfd run`
child process ([ADR 0123](adr/0123-wave47b-run-service.md), [ADR 0143](adr/0143-job48i-native-runner-start-unix-zombies-linux-known-misses.md)).
`rcfd-runner --help` lists the commands.

Where a started runner writes its standard output and error ([ADR 0146](adr/0146-job48l-runner-output-early-exit-windows-inherited-handles.md));
its standard input is always the null device:

| `ensure`, `start` option | Library (`Client::with_output`) | Output |
|---|---|---|
| none | `RunnerOutput::ProjectLog` (the default; the Studio and `rcfd-mcp` use it) | appended to `<project>/.rcfd/runner.log` |
| `--log-file <file>` | `RunnerOutput::File` | appended to `<file>`; the runner's own log lines stay in `.rcfd/runner.log` |
| `--stdout` (or `--foreground`) | `RunnerOutput::Caller` | the starting command's own standard output and error: a caller that reads them to the end waits until the runner exits |
| `--quiet` | `RunnerOutput::Null` | discarded |

A runner that exits before it holds the project's lock, for example on a bad `.rcfd/runner-config.json`, is reported at once,
with its exit code and the end of its output. Before ADR 0146 the start waited 10 s and said only that the lock was not taken.

**Windows: a capturing caller still waits.** std starts every process with handle inheritance on, and the workspace allows no
unsafe code. So the started runner, and the solvers it starts, also hold every inheritable handle of the program that started
the runner, whatever the output option. Measured with a program that reads the output through pipes (std's `Command`;
ADR 0146 C1, C2, I1-I3):
* the reader of `rcfd-runner start`'s output waits until the runner exits, although the command itself exits at once;
* a runner that `rcfd-mcp` started keeps the server's standard output pipe open after the server has exited;
* the handles pass through intermediate processes too, so the same holds for the reader of a process that runs the command.

A shell that stores the command's output, or a CI step, reads it the same way; this was not run.

On Linux the capturing caller returns as soon as the command exits.

## Not implemented / deviations

- No control-port socket as such: remote control of a running job is the service (`rcfd serve`,
  `run.pause/resume/stop/status`, `events.poll`), which also runs the same drivers.
- No signal handler: interrupt a CLI run with `--stop-file` (a clean stop with checkpoint and results);
  killing the process loses the unwritten results.
- No `--hostfile` / distributed runs, no single precision.
- Restart files cover steady laminar SIMPLE/SIMPLEC and transient laminar constant-density PISO/PIMPLE only (see above).
- Not delivered (F-121 stays partial): MPI (`--hostfile`), per-core affinity, commands run at start or on a schedule (`--set` is
  the safe subset), automatic case-modification strategies, single precision, a control socket; chains and `restart.from` are
  not driven through the service method `run.headless`.

## Where to go next

- [Case schema](case-schema.md): the restart, chain, output and study keys this page refers to.
- [Service protocol](service-protocol.md): the service methods that run the same drivers remotely.
- [Parametric studies](studies.md): the study command and its result table.
- [Python access](python.md): driving run, check and study from Python.
