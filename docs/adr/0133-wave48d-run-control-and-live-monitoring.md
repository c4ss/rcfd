# ADR 0133: Wave 48D, run control and live monitoring in the Studio (several background runs through `rcfd-runner`)

Status: **ACCEPTED 2026-10-28** (the maintainer accepted the ADR and took the recommended option of every open decision W48-1 to W48-16, scope cut (a)) (scoping only; nothing implemented or run). Needs ADR 0130 (48A: runner on `rcfd-project`, tolerant
readers, and per-step monitors if W48-6 (a) is chosen) and ADR 0131 (48B). House rules as in ADR 0130. Parent: plan section 2 (start
several runs, close the app and they continue), section 3 (L2, L5), section 6 (run with live residual and report monitors and run
control). Roadmap: F-266, F-262.

## Context (read at main 1248a76)

* `rcfd_runner::client::Client` (`crates/rcfd-runner/src/client.rs`): `start`, `submit`, `list`, `status`, `pause`, `resume`,
  `cancel`, `wait`, `shutdown`, `ensure_runner`, `runner_running`. Requests are files; the runner polls every 100 ms; budgets in
  `.rcfd/runner-config.json` (`max_runs` 2, half the cores, 4096 MB) (ADR 0123 decision 6).
* `ensure_runner` on Windows starts the runner through PowerShell and falls back to a direct start after 5 s (ADR 0123 amendment 2).
  On the gate host PowerShell has hung; the fallback hands inheritable handles to the runner (S5 known miss).
* Closing the app never stops a run: the runner is detached, and a new runner adopts running solvers (ADR 0123 decisions 5 and 8).
* The runner writes `status.json` (atomic), `events.jsonl`, `residuals.jsonl` (one line per step) and `metrics.json`; `log.txt` is the
  solver's output. Residual lines carry `res_u`, `res_p`, `continuity` as residuals, and `monitors: {}` unless 48A D5 lands.
* The frozen GUI's monitor store (`crates/rcfd-gui/src/monitors.rs`) has registered tests M1-M7 (`tests/monitors_plot.rs`): exact log
  mapping, decimation that keeps extrema and order, ingest that drops nothing, polling that does not block.
* `rcfd-project::jsonl::read` tolerates a torn last line; incremental reading from an offset is not in `rcfd-project` (the transcript
  reader of `rcfd-claude` has `read_new`, a pattern to follow).

## Decisions (proposed; no open user decision in this sub-wave beyond W48-6 of ADR 0130)

1. **D1 runs only through the runner.** The Studio never runs the solver in process. Start = `Client::start(case, settings)` with the
   case from `cases/` and settings from the start dialog (priority, threads, memory, `set` overrides including the store keys of ADR
   0130 D4). The run list, pause, resume and cancel map one to one onto the client.
2. **D2 polling model.** One worker thread per open project reads, every 250 ms, `status.json` of every non-terminal run and the
   new bytes of `residuals.jsonl` and `events.jsonl` of the runs shown (incremental, from a kept offset; a torn tail is held back until
   completed). Terminal runs are read once. The UI thread only reads the worker's snapshot.
3. **D3 plots.** The copied monitor store (ADR 0131 W48-9) per run: residuals on a log axis, monitors on linear axes, decimated for
   drawing (M3 rule). Non-finite values (NaN from 48A D2) show as gaps and a warning, not as a dropped run. A marker shows the viewport's
   current step (ADR 0132).
4. **D4 runner state shown.** "Runner not running" (lock probe false) with the queue still listed; a start button calls
   `ensure_runner`. When `ensure_runner` fails, its located error is shown; the Studio does not retry in a loop.
5. **D5 several runs.** The list shows queued, running, paused and finished runs with step, time, last residuals and, for queued
   runs, the queue position by the runner's order (priority, created, id; ADR 0123 decision 7). Budgets are shown from the runner
   config; editing them writes `.rcfd/runner-config.json` atomically (the runner reads it at start; a restart notice is shown).
6. **D6 logs and outputs.** `log.txt` tail (last 200 lines, read from the end), events as a table, `metrics.json` and the solver's
   `output/report.html` path.

## Placement

`rcfd-studio -> rcfd-runner (client only), rcfd-project`. No change to `rcfd-runner` beyond ADR 0130. No new dependency (plots use the
`egui_plot` chosen in ADR 0131 W48-2).

## Registered checks (proposal)

Process tests start `rcfd-runner serve` directly (ADR 0128 rule), except R9, which tests `ensure_runner` and accepts both start routes.

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| R1 | Studio model starts 5 runs, `max_runs` 2 | the model's states equal `status.json` at every worker snapshot after the next poll; at most 2 shown running or paused | the contract (ADR 0123 S1) | a model that does not re-read status shows a finished run as running |
| R2 | App closed while 2 runs run (model and worker dropped), reopened after 2 s | both solvers alive during the gap; reopened model lists them running, then finished; no run launched twice (one `launched` event each) | plan section 2 | (S4c pattern) |
| R3 | Pause, resume, cancel through the model | `paused` shown within 2.0 s; 0 new residual steps shown while paused; `cancelled` shown within 5.0 s | ADR 0123 S6, S7 limits | a model that maps resume to pause fails the step check |
| R4 | Incremental residual ingest: a writer appends 20 000 lines in bursts with torn tails | the model has steps 1..20 000 exactly once, in order; a torn tail is shown only once completed | parity (M4) | a whole-file re-read per tick that drops the torn line loses a step |
| R5 | Decimation and log mapping (ported M1-M3, M6) | the ported limits unchanged | parity | (ported controls) |
| R6 | Worker tick cost (release): 4 running runs each with a 100 MB `residuals.jsonl` already read, 10 new lines per tick | median tick <= 5 ms | 2 % of the 250 ms period | a tick that re-reads whole files exceeds 5 ms |
| R7 | Diverged run with `null` residuals (after 48A) | listed with state, plot gap at that step, warning shown | ADR 0122 defect | the pre-48A reader skips the run |
| R8 | Monitors (only with W48-6 (a)): cavity with `lid_drag` | plotted values at each step equal the residual lines; last value equals `metrics.json` / `monitors.json` bit-exactly | the colours-are-numbers rule of plan limit 2, applied to plots | a plot fed from `res_p` fails |
| R9 | Runner not running: lock released, then `ensure_runner` from the model | the model shows "runner not running" before; after the start it shows the runner and the queued run starts within 15 s | 5 s PowerShell wait plus 10 s direct start (ADR 0123 amendment 2) | `runner_running` false before the start |

## Not in CI

* With a real window: plots update smoothly while two runs stream; the app stays responsive with 4 runs.
* By hand: close the Studio while runs run, check in Task Manager that solvers continue, reopen.

## Out of scope

Run comparison side by side (wave 49); socket control; memory enforcement; pause by checkpoint; chains (`chain.sequence` ignores the
pause file, ADR 0123 limits); design-study runs from the UI (wave 50, design ledger view).

## Risks

* PowerShell hangs on the gate host (ADR 0123 amendments 2 and 3): R9 can take the fallback; S5's inherited-handle limit then applies
  to a Studio started from a capturing shell. The Studio is a GUI process and does not capture output, so the user-facing effect is
  UNVERIFIED but expected to be none.
* A recycled pid can keep a dead run shown as running until the runner's probe sees two misses (ADR 0128 finding 4).
* The parity check "report monitors" fails if W48-6 is declined; then R8 is not registered and the parity test is an ignored known
  miss with its reason.

## Amendment 1 (2026-10-28, 48D, before any registered run): set-up, the store-precision key and three new checks

Written by the 48D implementer at main 64732bb (48A, 48B, 48C library part, 48E library part and 48F merged) after reading the
code and before any 48D code was written or run. What I knew: the code reading below; no 48D test had run. Decisions applied as
accepted: W48-4 (b) (the runner already passes `--store-dir runs/<id>/fields`), W48-5 (a) (the start dialog proposes the store),
W48-6 (a) (the runner already passes `--status-monitors`). Maintainer decision of 2026-10-28: the opt-in `output.store_precision`
key lands with the start dialog, in this sub-wave (the question of ADR 0130 amendment 1 item 10). The limits of R1-R9 are
unchanged; this section fixes what the table leaves open and registers R10-R12.

**Placement (changed).** `rcfd-studio -> rcfd-runner` is a new workspace edge (layers 9 over 2; `workspace_graph.rs` unchanged,
`docs/architecture.md` updated). `rcfd-api` gets the opt-in case key `output.store_precision` (`schema.rs`, `fieldstore.rs`,
`docs/case-schema.md`, `docs/run-control.md`); "No change to `rcfd-runner`" holds. No new third-party crate.

**`output.store_precision`.** An enumeration `"f64"` (default) or `"f32"`. Absent or `"f64"`: the hook writes `Precision::F64`
exactly as before (the same code path). `"f32"`: every stored field is written with `Precision::F32` (ADR 0124 D4: rounded to the
nearest single, bound `f32_error_bound`). The solver is unchanged by the key (the hook only reads it). A continued run appends with
the precision of its own case. Consequence read from code, not changed here: like every schema addition (ADR 0124's three keys),
the new default enters `SettingsTree::content_hash`, so the settings hash of a case container (`rcfd-io` `save_case`) saved before
this commit no longer matches on `load_case`; no such file is committed in the repository. (The addendum, question 5, lists
every use of that hash found afterwards; checkpoints are not among them.)

**Model and threads (D1, D2 as implemented).**
1. Modules: `runs.rs` (run rows, queue order, start plan and store proposal, runner budgets), `poll.rs` (the run worker: one
   `tick`, the incremental tail reader), `monitors.rs` (an additive `push` and `segments`; the copied functions are unchanged).
2. The run worker is a second thread per open project, started when the project worker reports the project open (its canonical
   root). The 48B project worker keeps its 500 ms watcher. Control calls that start processes or wait (`Client::start`, which runs
   `rcfd --version`, and `ensure_runner`, up to 15 s) run on a short-lived thread each, never on the UI thread or the tick loop;
   closing the project does not wait for them. Pause, resume and cancel (request files) and budget writes run on the worker.
3. One `tick`, in this order: list `runs/` (`read_dir`); read `run.json` once per new run (cached); read `status.json` of every run
   not yet seen terminal (a run seen terminal is read once more for its final record and `metrics.json`, then not again); a read
   error keeps the last known state and is counted, not shown as a state; the lock probe `runner_running`; for each **shown** run
   (the runs the UI asks for: the selected run, or several), the new bytes of `residuals.jsonl` and `events.jsonl` from kept
   offsets, read after its `status.json` (so lines the runner wrote before a state change are read in the same tick); the last 200
   lines of `log.txt` when its length changed. Status is read per run (not `Client::list`, which fails the whole list on one
   transient read error). Bytes after the last `\n` are held back (the offset stops at the last newline). A complete line that does
   not parse is skipped with a warning and the offset moves past it. A file shorter than the kept offset (replaced) is read again
   from 0 and the UI's series of that run are reset.
4. The worker sends deltas, not copies: new residual and monitor points of the shown runs, new events, and the run rows when any
   row changed; it calls the UI's repaint only when it sent something other than its tick marker. The UI thread merges deltas into
   its stores (`push`, which ignores a step at or before the series' last, like `merge`) and reads nothing from disk.
5. Plots keep residuals and monitors apart: the selected run's residuals go to `Studio::plots` as `res_<equation>` (the runner's
   `u`, `p`; `continuity` kept), so `residual_names` and the ported M6 hold; its monitors go to a second store `Studio::monitors` under
   their report names. A non-finite value (`null` in the file) is kept as NaN; the drawn lines are split into segments at
   non-finite values (`segments`, each segment decimated by the unchanged `decimate_minmax`), so a gap is a gap and not a joined line;
   the first non-finite step of each series is a warning of the run.
6. The run list stays the 48B browser list (`Browser::runs`, which B7 measures) with the worker's rows laid over it by run id for
   the rows drawn (state, step, time, reason, queue position, last residuals); a run the worker knows and the browser does not yet
   list is appended until the browser lists it. B7's injected rows have ids no worker row has, so B7 draws exactly as before.
7. The worker's runner client is `Client::new(<canonical root>, runner_exe)`; the Studio's `runner_exe` is `RCFD_RUNNER_BIN`, else
   `rcfd-runner` next to the Studio binary (`Client::with_default_runner`'s rule); tests set it to `target/<profile>/rcfd-runner`,
   found next to the test's `rcfd-studio` binary, and assert that `rcfd --help` there names `--status-monitors` (a stale binary
   fails loudly).
8. Budgets are read with `rcfd_runner::runner::Config::load` and written by serialising `Config` (the reader has
   `deny_unknown_fields`) through `rcfd_project::atomic::atomic_write`, after checking `max_runs`, `cpu_threads`, `poll_ms` are
   positive; a restart notice is shown when a runner holds the lock.
9. Queue position: the queued runs that are not held (no `pause` file), sorted by (priority descending, `created`, id) as the
   runner's `schedule`; position 1 is next. Held queued runs show "held".
10. Paths under `runs/` that the watcher reports are kept out of the project's "changed on disk" list (streaming runs would push
    the other entries out of its 50 places); the run panels show them.

**Set-up of R1-R9.** All process tests are release or debug builds of this crate's tests with `rcfd` and `rcfd-runner` of the same
profile, the 12 x 12 cavity of ADR 0123 (with the `lid_drag` report of `rcfd-mcp`'s template for R8), `--set
solver.max_iterations=<steps>`, `mem_mb` 10 (no pre-launch estimate wait), threads 1, and `.rcfd/runner-config.json` with
`max_runs` 2 (R1) or 3 (others), `cpu_threads` 8, `poll_ms` 100. R1-R8 start `rcfd-runner serve --project <dir>` directly and wait
for `runner_running()` before the model starts runs, so the model's `ensure_runner` returns at once. Every test kills, at the end,
only solver pids taken from the `launched` events of its own runs. Step counts are fixed by the rule-d smoke, disclosed in the
addendum before the counted runs.
* **R1.** The model starts 5 runs. Snapshot check, repeated until all 5 are terminal: the test reads every `status.json` (A) at
  `t_a`, drains the model until it holds a tick that started after `t_a`, reads every `status.json` again (B), and reads each run's
  `events.jsonl`. For each run the shown state must lie in the run's state sequence (the `to` of its state events, in order) between
  the state of A and the state of B, inclusive; when A and B agree it must equal them. At most 2 runs shown running or paused at
  every snapshot. "Equals `status.json`" is defined this way because the runner replaces the file every 100 ms. Negative control: the
  same with the worker's test switch that reads each `status.json` only once: after all five are finished on disk, at least one is
  shown non-terminal (the check fails).
* **R2.** Model A starts 2 runs and waits until both are shown running; the solver pids from `status.json`. Model A (worker and
  project) is dropped at `t0`; for 2.0 s the test probes both pids with `sys::pid_alive` every 100 ms (all alive). Model B opens the
  project: both runs shown running in its first snapshot, then finished; each run has exactly one `launched` event. Negative control
  (the probe discriminates): `pid_alive` is false for both pids after they finished.
* **R3.** The model starts one run, waits until step >= 5 is shown, then `pause`: the first snapshot showing `paused` within 2.0 s of
  the call; the largest residual step shown at that snapshot is S0, and for 2.0 s of snapshots after it the largest step stays S0
  ("0 new residual steps"). Then `resume`: the step check is that the largest step shown exceeds S0 within 5.0 s. Then `cancel`:
  `cancelled` shown within 5.0 s of the call. Negative control: the model's test switch that sends `pause` for `resume`; the step
  check fails (no step beyond S0 within 5.0 s); the run is then cancelled.
* **R4.** No runner: a fixture run made with `Project::create_run`, status `running`, shown by the model, worker period 20 ms (the
  limit is about content, not time). The test thread writes 20 000 runner-shaped lines to `residuals.jsonl` in bursts of 1-500
  lines (LCG seed 48004); every second burst ends with a torn tail, alternating between a prefix of the next line cut at an LCG byte
  position and the complete JSON of the next line without its `\n` (valid JSON, not committed). After each burst it waits for two
  completed ticks and checks: the model's steps are exactly 1..=k (k the last committed step), each once, in order, and the torn
  line's step is absent. The next burst first completes the torn line. **Negative control (changed):** the table's "whole-file
  re-read per tick that drops the torn line" cannot lose a step with the store's merge rule (a line dropped once is read again on
  the next re-read; read from code). The control that discriminates is the reader that moves its offset to the end of the file,
  over a torn tail (worker test switch): it loses at least one step.
* **R5.** M1, M2, M3 and M6 of `tests/monitors_plot.rs` unchanged. Not registered, reported: `segments` splits at every non-finite
  value and each segment's decimation keeps the M3 properties.
* **R6.** Release only (the B7 skip line in debug). `RunPoller::tick` called directly (no thread, no UI, no project worker: the
  500 ms watcher and `Browser::load` are not part of the tick). 4 fixture runs, status `running`, all 4 shown, each
  `residuals.jsonl` of at least 100 000 000 bytes of runner-shaped lines (with a `lid_drag` monitor); one untimed tick reads them;
  then 50 timed ticks, each after 10 lines were appended to each file (untimed); the tick's deltas are dropped. Limit: median of the
  50 <= 5 ms. Negative control: the worker's test switch that re-reads every shown file from byte 0 each tick (parsing every line,
  sending only steps beyond the last): median of 5 ticks > 5 ms.
* **R7.** A fixture run (no runner), state `failed` with a reason, `time` null in `status.json`; `residuals.jsonl` steps 1-10, `u`
  null at step 6, finite at 7 and 8, every value null at 9 and 10. Limits: listed by the browser and the worker with state `failed`;
  the segments of `res_u` are steps [1..5] and [7, 8]; the run's warnings name step 6 of `u`; the Plots panel draws (headless
  frame) with the warning text in its output. Negative control: the pre-48A residual reader (strict `f64`, copied in the test)
  refuses step 6's line.
* **R8.** The model starts the lid cavity through the start plan with the store proposal on (R10) and shows it; after `finished`,
  undecimated `Studio::monitors` `lid_drag`: one point per step 1..=N, each y bit-equal to that line's `monitors.lid_drag` in
  `residuals.jsonl`, and the last bit-equal to the last y of `output/monitors.json` `series.lid_drag` and to `metrics.json`
  `last_residuals.monitors.lid_drag` (the runner's `metrics.json` keeps monitors only there). Negative control: the series fed from
  `res_p` differs at the last point. Also checked (R10 end to end): the store is at `runs/<id>/fields`, at most 100 snapshots,
  every field `f32`.
* **R9.** The model queues one run (`Client::submit`) with no runner alive and shows "runner not running" (lock probe false, the
  negative control); then the model's start-runner action (`ensure_runner`, either route). Limit: within 15.0 s of the call the model
  shows the runner running and the run running or terminal. The test's output goes to a file (S5a) and a drop guard asks the runner
  to shut down and kills only the run's solver.

**New checks.**

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| R10 | Start plan (W48-5 (a)), pure logic: the 12 x 12 cavity text (a) without store keys, max_iterations 400; (b) with `output.store_interval = 7` in its text; (c) the store switched off; (d) f64 picked; (e) a transient `piso` case with `end_time = 0.3`; (f) `dynamic_mesh.enabled = true` | `set` exactly: (a) `output.store_interval=4`, `output.store_precision=f32`; (b) nothing added; (c) nothing; (d) `output.store_interval=4` only; (e) `output.store_time_interval=0.003`, `output.store_precision=f32`; (f) nothing, with a note naming `dynamic_mesh.enabled`; (a) estimated size 144 cells x 4 fields x 100 x 4 bytes = 230 400 bytes; every planned case still parses with its `set` applied | the decision's text; at most 100 snapshots (`ceil(M / ceil(M/100)) <= 100`) | a plan that reads the resolved tree instead of the text adds a store to (b) only if the case sets 0; checked by (b) |
| R11 | `output.store_precision` (new key): the lid cavity, `store_interval = 7`, through `runctl::drive` | key absent: store files byte-equal to `"f64"`; `"f32"`: every stored value equals `f64::from(v as f32)` of the f64 store's value (bit-exact) and lies within `f32_error_bound`; the solver fields bit-equal with and without the key; `"f16"` refused naming `output.store_precision`; H1-H4, A6, A7 and `baseline_hashes` pass unchanged | defaults bit-identical; ADR 0124 D4 bound | the f32 store differs from the f64 store in at least one value |
| R12 | Browser refresh on a run change (the 48B known limit "every change re-reads all run summaries"): a project with 500 runs; one run's `status.json` replaced | the project worker re-summarises exactly 1 run; the browser it sends equals `Browser::load` of the folder | worker cost O(changed runs) while runs stream | the 48B full reload re-summarises 500 |

B1-B9 of ADR 0131 are re-run unchanged. B9's live-monitor item becomes a real test (R8 through the drawn app) instead of an
ignored `*_known_miss`.

## Amendment 2 (2026-10-28, 48D, after the code was written and before any R1-R12 run counted)

What I knew: R11 had run (it was written and run before the Studio code, results in the addendum); the Studio crate and
its tests compile; a rule-d smoke outside any test: `rcfd run` of the 12 x 12 lid cavity with `--status-every 1
--status-monitors`, 5000 steps in 4.34 s (release, 0.87 ms per step). No Studio test had run. Limits unchanged.

1. Step counts (from the smoke, debug builds use a quarter): R1 5 runs of 2000 steps (about 1.7 s each, so two overlap and
   three wait); R2 and R3 15 000 steps (about 13 s, longer than the 2 s gap plus reopening, and than pause, 2 s hold, resume
   and cancel); R8 400 steps (the case's `max_iterations`, so the proposal is `store_interval = 4`); R9 2000 steps; the B9
   live-monitor parity test 3000 steps.
2. Code set-up details not in amendment 1: `rcfd-studio` uses the workspace's `toml` (already locked for `rcfd-api`) to read
   which store keys a case text sets (R10), and `rcfd-tsstore` as a dev-dependency for R8 (an edge ADR 0131 Placement plans);
   `Cargo.lock` gains two dependency lines, no package. `Browser` gains `summaries_read` (R12's counter).
3. R2's "first snapshot" is the model's state when it first holds the browser and a worker tick after opening.

## Amendment 3 (2026-10-28, 48D, R10 set-up defect, before the corrected run)

The first R10 run failed before reaching any limit: case (f) of the test (the cavity plus `[dynamic_mesh] enabled = true`
alone) is not a valid case (`dynamic_mesh.moving_patches: name at least one moving patch`), so `StartPlan::new` refused it.
A test-code defect: (f) must be a valid moving-mesh case. Corrected: (f) is the piston case of
`crates/rcfd-api/tests/dynamic_mesh.rs` (`box_2d` 20 x 2, `piso`, `end_time = 0.5`, moving patch `xmin`, smoothing,
fixed patch `xmax`). Limits unchanged (nothing added, a note naming `dynamic_mesh.enabled`). Cases (a)-(e) had passed in that
run (printed before the failure) and are not changed.

## Amendment 4 (2026-10-28, 48D, after the R6 miss, before R6b ran)

**R6 missed its limit** in the registered run: median 45.7 ms against 5 ms (min 37.4, max 84.0; 4 files of 100 000 129
bytes, 542 470 lines each; control 2016 ms). It becomes the ignored `r6_..._known_miss` test with the limit intact; the set-up
it measured (every file opened again in every tick) stays reachable through the worker's test switch `reopen_files`, so
the known-miss test measures exactly what missed.

Diagnosis (an unregistered timing on this host, 30 repetitions each, not kept as a test): opening a file that another
handle has just appended to and closed costs a median 5.3 ms (100 MB file) and 6.5 ms (a 9 KB file), against 0.03 ms for
the same open of an unchanged file, 0.11 ms for its metadata, and 0.09 ms to read the new bytes through a handle kept open.
So the cost is the open of a just-modified file (consistent with on-access scanning by the host's anti-virus; not
verified which component), not the reading or parsing, and it does not depend on the file's size; four appended files per
tick give about 21 ms of the 45.7 ms. Read, not measured: `status.json` is replaced by the runner every 100 ms while a run
moves, so the real worker opens a fresh file per running run per tick too.

**Change (registered here before its run).** The worker keeps the read handles of the shown runs' `residuals.jsonl` and
`events.jsonl` open between ticks and reads new bytes through them; each tick first takes the path's metadata (length): equal
to the kept offset, nothing is read; shorter, the file was cut or replaced and is opened again from 0 (the series reset as
before). `status.json` and the pause file are read only when the status file's length or modification time changed since
the last read (a run that does not move costs one metadata call). Rust opens files on Windows with read, write and delete
sharing, so a kept handle does not block the runner's appends or its torn-tail truncation. Limit of the change: a file
replaced by a rename with a length not shorter than the offset is not noticed while the handle is kept (the runner never
replaces these two files).

**R6b** (new check id, the same fixture, procedure and limit as R6): median of 50 ticks <= 5 ms with the change, release
only. Negative control: R6's whole-file re-read control, unchanged. Not registered, reported: the same 50 ticks when the
appended lines also change every run's `status.json` (as a moving run does), to show the remaining per-run open cost.

## Addendum: 48D results (2026-10-28, branch `worktree-agent-a5ba74d44afbc1aaf`, release builds on the gate host)

Decisions applied as accepted: W48-4 (b), W48-5 (a), W48-6 (a); the maintainer's `output.store_precision` decision. Commits
e704a79 (amendment 1) to the commit of this addendum, on main 64732bb. All Studio tests ran in release with
`--test-threads 1`, `rcfd` and `rcfd-runner` of the same build; the whole `rcfd-studio` suite ran once more at the end with its
output going to a file (R9's rule), and the numbers below are from that final run unless a first run is named. Leftover
processes after the runs: none (`rcfd`, `rcfd-runner`). The workspace gate was not run (the maintainer runs it).

| Id | Limit | Measured | Result |
|---|---|---|---|
| R1 | shown state within the state sequence between the reads before and after the tick; at most 2 running or paused | 14 snapshots checked over 5 runs of 2000 steps, 0 failures, at most 2 shown running or paused, all 5 finished | pass |
| R1 control | status read once: a finished run shown non-terminal | 5 of 5 finished runs shown queued or running | fails as required |
| R2 | both solvers alive over the 2 s gap; reopened model lists them running, then finished; one `launched` each | alive in all 5 probe rounds; first snapshot after reopening running, running; both finished; 1 `launched` each | pass |
| R2 control | `pid_alive` false after the solvers finished | false for both | discriminates |
| R3 | `paused` within 2.0 s; 0 new steps while paused; moves after resume; `cancelled` within 5.0 s | paused shown after 0.261 s at step 800, still 800 after 2 s; resumed to 881; cancelled shown after 0.268 s | pass |
| R3 control | resume mapped to pause fails the step check | no step beyond 392 within 5 s | fails as required |
| R4 | steps 1..=20 000 exactly once, in order, after every burst; a torn line not shown before completed | 85 bursts, 42 torn tails (21 cut prefixes, 21 complete JSON without newline), 0 problems, 20 000 steps and 20 000 `lid_drag` points | pass |
| R4 control (amendment 1) | offset moved over the torn tail loses a step | 42 of 20 000 lost | fails as required |
| R5 | M1, M2, M3, M6 unchanged | pass (M1 worst inverse relative error 4.219e-15); M4 and the merge test pass too | pass |
| R6 | median of 50 ticks <= 5 ms | **45.7 ms** (min 37.4, max 84.0), registered run before amendment 4; 38.9 ms when its known-miss test ran again | **miss**: `r6_..._known_miss`, ignored, limit intact |
| R6b (amendment 4) | the same, kept read handles | median 0.710 ms (min 0.529, max 2.319); 4 files of 100 000 129 bytes, 542 470 lines each, read in 3 untimed ticks (2.3 s) | pass |
| R6 control | whole-file re-read > 5 ms | median 1829 ms of 5 ticks (first run 2016 ms) | fails as required |
| R7 | listed `failed`; segments of `res_u` [1..5], [7, 8]; warning naming step 6 of `u`; drawn | browser and worker `failed`, 0 skipped; segments exactly [1..5], [7, 8]; "residual u is not finite at step 6 (shown as a gap)" (and the same for `p`, `continuity` at step 9); the warning text is in the floated Plots panel's drawn shapes | pass |
| R7 control | the pre-48A reader refuses step 6 | "invalid type: null, expected f64 at line 1 column 43" | fails as required |
| R8 | `lid_drag` at each step bit-equal to `residuals.jsonl`; last bit-equal to `monitors.json` and `metrics.json` | 400 points (steps 1..400), 0 differ; last -1.0255672839856542e-2 in the plot, `monitors.json` and `metrics.json` | pass |
| R8 control | `res_p` differs | last `res_p` 3.75940151343793e-13 | fails as required |
| R9 | runner and run shown within 15.0 s | 5.166 s (runner running, run running); the time is consistent with the PowerShell wait of 5 s and the direct start (which route was taken is not recorded) | pass |
| R9 control | no runner before the start | lock probe false; the model showed "runner not running" with the run queued | as required |
| R10 | the `set` of (a)-(f) exactly; (a) 230 400 bytes; plans parse with their `set` | (a) `output.store_interval=4`, `output.store_precision=f32`, 100 snapshots, 230 400 bytes; (b) nothing (an explicit 0 also nothing); (c) nothing; (d) `output.store_interval=4`, 460 800 bytes; (e) `output.store_time_interval=0.003`, `output.store_precision=f32`; (f) nothing, note naming `dynamic_mesh.enabled`; at most 100 snapshots for M = 1..3000, 9 999-10 001, 10^6 | pass (after amendment 3) |
| R10 end to end (in R8) | store at `runs/<id>/fields`, <= 100 snapshots, f32 | 100 snapshots, every field `F32`, no `output/fields` | pass |
| R11 | absent byte-equal to `"f64"`; f32 bit-exact rounding within the bound; solver unchanged; `"f16"` refused; H1-H4, A6, A7, `baseline_hashes` unchanged | store files byte-equal; 6 snapshots, 3456 values, every f32 value equal to `f64::from(x as f32)`, worst error 0.984 of the bound; fields bit-equal with and without the key; `f16` refused at `output.store_precision`; H1-H4 4 passed, A6 and A7 pass, `baseline_hashes` 3 passed (1 ignored as before) | pass |
| R11 control | the f32 store differs | 2592 of 3456 values differ (`store.rts` 71 355 bytes f64, 57 531 f32) | fails as required |
| R12 | 1 run re-summarised; the browser equals `Browser::load` | 1; equal | pass |
| R12 control | the full reload reads 500 | 500 (open and `Browser::load`) | discriminates |

Before amendment 4 (the kept read handles) R1-R4, R7, R8, R10 (after amendment 3), R12 and the live parity test had run once
and passed with the same results in kind (R1 14 snapshots, R3 paused after 0.264 s, R4 42 lost in the control, R8 the same
bits); every one was run again on the final code (the table).

ADR 0131 checks re-run on the final code, limits unchanged: B1 (8 runs, 1 skipped), B2, B3, B4 (200 frames), B5 (569 leaves,
one more than 48B: `output.store_precision`; 477 defaults parsed), B6 (clean 0.296 s, dirty 0.302 s, control no report in 5 s),
B7 (median 0.339 ms, control 14.5 ms), B8, B9: all pass. B9's live-monitor item is now
`parity_run_with_live_residual_and_report_monitors` (3000 steps through the drawn app; residuals and `lid_drag` plotted while
running; last `lid_drag` bit-equal to `monitors.json`): pass; the other three items stay ignored as in 48B.

Also run: `rcfd-cli` `workspace_graph` (3), `run_flags_w48a` (A6, A7), `cli` (8), `w19c_run_control` (7); `rcfd-api` lib
(104 passed, 2 ignored as before), `physics_schema` (11), `rules_w19c` (5, 1 ignored as before), `runctl` (6); `rcfd-mcp`
`default_f1` (2), `project_w48f` (8); `rcfd-gui` `logic` (10; the frozen GUI reads the schema with the new key). `cargo clippy
-p rcfd-studio -p rcfd-api --all-targets -- -D warnings` and `cargo fmt -p rcfd-studio -p rcfd-api -- --check` clean;
`scripts/third_party_licenses.py --check` up to date.

Reported, not limited:
* R6b with every run's `status.json` replaced before each tick (as moving runs): median 6.700 ms (min 5.552, max 8.680). The
  fresh `status.json` of each moving run is still opened every tick (about 1.5 ms each here).

Deviations and notes:
* R2 set-up deviation: amendment 1 registered a probe every 100 ms; the test sleeps 100 ms between rounds, but each
  `pid_alive` runs `tasklist` (about 0.15-0.2 s), so the 2 s gap had 5 rounds of both pids, about one every 0.4 s. The
  limit was met; the evidence is coarser than registered (a solver dying and being replaced within 0.4 s would not be
  seen). Not re-run.
* R6 missed; amendment 4 changed the reader and registered R6b (same limit) before it ran. The known-miss test keeps the
  missed set-up through the `reopen_files` switch.
* Amendment 3: R10 case (f) was not a valid case (test-code defect), corrected before the counted run.
* Amendment 2 said `Cargo.lock` gains two dependency lines: it gains three (`rcfd-runner`, `rcfd-tsstore`, `toml` under
  `rcfd-studio`), no package.
* Code beyond the Files list of the table: `project_view.rs` and `watch.rs` (R12, the incremental browser refresh), and the
  run panels in `ui.rs` (start dialog, budgets dialog, run list with live columns, Plots with gaps and a marker hook
  `Studio::plot_marker` for the viewport, log tail and events table, outputs with `metrics.json`).
* The start dialog knows the cell count only for generator meshes with plain counts (`cavity_2d`, `box_2d`, `channel_2d`,
  `wedge_2d`, `nozzle_2d`, `cylinder_ogrid`, `annulus_3d`, `annulus_3d_sector`); otherwise it shows the snapshot count and
  "cell count unknown". Algorithms other than `piso` and `pimple` (for example `compressible`, transient or not) get the
  step rule from `max_iterations`.

Not run: the "Not in CI" items (a real window: smooth plots with two streaming runs, responsiveness with 4 runs; closing the
Studio by hand and checking Task Manager); the Studio tests in a debug build, the profile of a plain `cargo test
--workspace` (R6 skips there; R1-R3 use a quarter of the steps; the B9 live parity test and R8, R9 do not scale their 3000,
400 and 2000 steps); Linux (the new process tests R1-R3, R8, R9 and the live parity test have never run there: a CI risk);
the workspace gate and workspace-wide clippy; `cargo deny` (not installed).

Questions for the maintainer:
1. Is the R6 handling accepted: the registered R6 stays an ignored known miss, and R6b (kept read handles, registered in
   amendment 4 after the miss) stands for the implementation?
2. Each moving run's replaced `status.json` still costs an open per tick (about 1.5 ms here). Accept, or read the step from
   the residual tail instead and `status.json` less often (a later change)?
3. `rcfd-studio` uses the workspace's `toml` to find which store keys a case text sets. Accepted (no new crate), or should
   `rcfd-api` expose that test instead?
4. For `compressible` and other non-`piso`/`pimple` algorithms the store proposal uses `max_iterations` as the step count.
   Should the Studio ask the solver (a check run) for the step budget instead?
5. Adding a schema key changes `SettingsTree::content_hash` of every case (read from code; the same happened with ADR
   0124's three keys). What that touches, from the callers: the `case_hash` of the solver's `start` event and of
   `journal.get` changes for every case; `journal.replay` of a journal recorded before 7644fcc replays but reports
   `matches: false`; `rcfd_io::container::load_case` refuses a case container saved before ("settings hash mismatch"), but
   no code outside `rcfd-io`'s own tests calls `save_case` or `load_case`. Checkpoints and `--restart-from` are not
   touched: `restart.rcfd` and `rcfd-post` checkpoints compare only the mesh hash, so a run started before this commit
   can still be continued (read from code, not run). Leave as with ADR 0124, or make these hashes ignore keys at their
   default (a separate change)?

## Files

`crates/rcfd-studio/src/{runs,monitors,poll}.rs` and tests, this ADR. Amendment 1 adds: `crates/rcfd-studio/src/{studio,ui,watch,
project_view}.rs`, `crates/rcfd-studio/Cargo.toml`, `crates/rcfd-api/src/{schema,fieldstore}.rs`, `crates/rcfd-api/tests/` (R11),
`docs/case-schema.md`, `docs/run-control.md`, `docs/architecture.md`, `Cargo.lock`.
