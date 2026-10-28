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

## Files

`crates/rcfd-studio/src/{runs,monitors,poll}.rs` and tests, this ADR.
