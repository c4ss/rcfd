# ADR 0161: Job 48AA, Stop in Run's place on the Studio toolbar: the runner's graceful stop, the final state stored as the last snapshot, Ctrl+., the run shown "stopped"

Status: **registered 2026-10-29**, before any registered run (the code and the tests below were written and compiled
first; nothing of them had run). Job 48AA, on local main 4e795b1 (wave 48V merged). House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed here before the registered runs; a missed
limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up gets an amendment written before the
corrected run; defaults of existing code stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not
re-recorded; no new crate, no unsafe code, the crate graph stays acyclic. Clean-room: our own Studio code and the egui
public API. Job 48X (ADR 0158) edits the same crate at the same time (`time_step`, `runs`, `quick_run`, `quick_run_ui`,
`ui`); this job keeps its code in two new files and touches shared files in a few small hunks (Placement).

## The maintainer's request (2026-10-29, relayed)

"How do I stop the run? There is no obvious way to stop a running run." Today a run is ended by "Cancel" in the status
strip at the bottom and in the Runs panel ([ADR 0133](0133-wave48d-run-control-and-live-monitoring.md),
[ADR 0141](0141-wave48g4-one-click-run-and-follow.md)). Asked: while a run started from the toolbar is active, the Run button turns
into a prominent Stop (same place, red, the label "Stop"), with a keyboard shortcut that cannot fire by accident (not
Escape; Ctrl+. or the button alone, decided here); Stop is a graceful stop that stores the current state as a final
snapshot and marks the run "stopped", reusing the runner's graceful stop if there is one (else the hard cancel, and a
question); the hover says what is kept; afterwards the run is listed "stopped" in the Runs panel with its stored
snapshots viewable; Run is disabled while the run stops; Stop before the runner has started and after the run exited is a
no-op without an error.

## Context (read at 4e795b1)

* **The runner's stop is graceful already.** `Client::cancel(id)` writes `runs/<id>/cancel` (it refuses a run whose
  `status.json` is terminal: "run X is already finished"). The runner launches the solver with `--stop-file
  runs/<id>/cancel` (`runner.rs` `launch`). `rcfd_api::runctl::drive` checks the stop file between two steps and ends
  the run `interrupted` (exit code 7, `exit::INTERRUPTED`); after the loop the field store's `Hook::finish` writes the
  state of the last step as a snapshot (unless that step was stored already) and closes the store with its footer. The
  runner's `finalize` records `cancelled` with the reason `cancelled on request; the solver stopped cleanly after step N`
  and `exit_code` 7, and `metrics.json` `solver_finish.state = "interrupted"`. A queued run with a cancel request is
  ended `cancelled before launch` (no exit code). If the solver has not exited `cancel_grace_ms` (10 000 by default)
  after the request, the runner kills it (`killed` event; then no finish event, no final snapshot, no footer). A paused
  solver leaves its pause loop when the stop file appears (`rcfd-cli` `main.rs`). The runner passes no `--checkpoint`,
  so a stopped run cannot be continued.
* So the brief's fallback (only a hard cancel) does not apply: no runner change is needed, and none is made.
* **`status.json` has no "stopped" state**: `RunState` (`rcfd-project` `records.rs`) is queued, running, paused,
  finished, failed, cancelled; the Studio's `RunRow::state_label` spells the state as `status.json` does ("held" for a
  queued run with a pause request) and feeds the strip, the Runs panel's live lines and the viewport's waiting text.
* **The toolbar** (`quick_run_ui::toolbar`): the green Run button (greyed out with `quick_run::blocked`'s reason), the
  case label, "F5", Options..., the binaries line; F5 is consumed there and calls `quick_run::run_selected_case`, which
  does not refuse a case whose run is going (a second run is queued). `QuickRun::run` is the last run Run queued and its
  plan; `QuickRun::starting` the start in flight (until `ensure_runner` returned; the run is queued before that).
  `QuickRun::rects` records the toolbar's button rectangles by label for the tests and is never cleared.
* **The field store of Run's plan**: the store proposal of ADR 0140 (a transient case: every `end_time / 100` s of
  simulated time, f32), unless the case sets its own store keys.

## What I already knew (rule d)

No probe was run for this job. From ADR 0153's addendum (b): the shipped cylinder (64 x 48, 80 s) through Run with its
defaults took 4,663 steps in 78.1 s on this host (release, about 16.7 ms per step), so its store (every 0.8 s simulated)
writes a snapshot about every 47 steps, about 0.8 s of wall time; a stop request is seen within one step. The Studio
library compiled (`cargo check -p rcfd-studio --lib`), and the test file compiled before this registration was committed.

## Decisions

**D1. Stop is the runner's graceful stop.** Stop calls `Client::cancel` for the run (the request ADR 0133 maps Cancel
onto). Nothing in `rcfd-runner`, `rcfd-cli`, `rcfd-api` or `rcfd-project` changes. The request is written on a
short-lived control thread (the UI thread does no file I/O), which first reads the run's `status.json`: a run that has
already ended is left alone (no request, no message), and so is one that ended between that read and the request (the
refusal of `Client::cancel` is followed by a second read).

**D2. "stopped" is a Studio label.** `status.json` keeps `cancelled`. The Studio shows "stopped" for a run whose state
is `cancelled` and whose `exit_code` is 7 (the solver's interrupted exit, `stop::stopped_cleanly`): a clean stop by Stop,
by the strip's or the Runs panel's Cancel, or by any client. A run cancelled before launch (no exit code) or killed after
the grace stays "cancelled". `RunRow` gains `exit_code` (from `status.json`); `RunRow::state_label` returns "stopped" for
such a run, so the strip, the Runs panel and the waiting text say it. A `stopped` state in the run record is a question
(question 1).

**D3. Which run Stop stops.** The selected case's newest live run that the toolbar's Run (its click or F5) queued. The
Studio records those run ids with their case (`StopState::started`, forgotten when the project closes). While such a run
exists, the toolbar draws Stop in Run's place; with another case selected the toolbar draws Run for that case, as
before. "Live" is a state that is not terminal, or a run the run worker does not list yet.

**D4. The states in Run's place**, for the selected case:
1. no live toolbar run, no start in flight: Run, unchanged;
2. a start of the case in flight whose run is not queued yet (looking for the programs, checking the solver, reading the
   case, setting the time step): Stop, red, greyed out; hover "The run of <case> is being prepared (<stage>); Stop is
   available once it is queued (Ctrl+.)"; the click and Ctrl+. do nothing;
3. the run queued, held, running or paused: Stop, red with a white bold label, enabled;
4. Stop pressed and the run not ended: "Stopping...", greyed out (hover: how long ago it was asked);
5. the run ended: Run again (enabled unless `blocked` says otherwise).
While Stop or "Stopping..." is shown for the case, F5 (the toolbar's handler) does not start a second run of it and the
status bar says why ("Run: run <id> of <case> is going; press Stop (Ctrl+.) first, or start another run with
Options..." or "... is stopping; Run is available again once it has stopped"). `quick_run::run_selected_case` itself is
unchanged (tests and other callers call it directly), and so is `quick_run::blocked`. Options... still starts any run.

**D5. The shortcut is Ctrl+. (Cmd+. on macOS)**, `consume_key(Modifiers::COMMAND, Key::Period)`, active only in state
3. Why: it needs a modifier, so typing a full stop in any field, a stray key or Escape (which closes popups and dialogs
in this app) never stops a run; Cmd+. is the macOS convention for "stop"; egui binds no default to it (zoom is
Ctrl+Plus/Minus/0); the maintainer named it. Considered: Shift+F5 (the debugger convention, next to F5 = Run) is one
key away from F5 and would stop a run when Shift is held while starting another; Escape rejected by the brief; the button
alone rejected because the maintainer asked for a keyboard route. The hover names it.

**D6. The hover says what is kept.** For a running or paused run: "Stop run <id> of <case> (Ctrl+.). The solver stops
cleanly after the step it is on. That state is stored as the run's last snapshot; every snapshot stored before is kept,
and so are the residuals, monitors, log and outputs. The run stays in the Runs panel as "stopped"; select it there to
view its snapshots. The case file is not changed, and the run cannot be continued (no restart file is written). If the
solver has not stopped <grace> s after Stop, the runner kills it and only the snapshots stored before are kept." (the
grace from the runner budgets the Studio read, else the default 10 s). When Run's plan has the store switched off, the
second and third sentences read "This run stores no fields: its residuals, monitors, log and outputs up to that step
are kept." For a queued or held run: Stop takes it out of the queue ("cancelled, before launch"), nothing computed or
stored.

**D7. Messages.** A Stop request says "stop requested for run <id>: the solver stops after the step it is on and stores
that state as the run's last snapshot". A request that fails for another reason than the run having ended says
"Stop: run <id>: <error>" and the toolbar shows Stop again. Nothing else is said.

**D8. Unchanged.** The strip's Pause, Resume and Cancel and the Runs panel's controls (same labels, same requests); the
Start run dialog; `quick_run::blocked`; every file the runner or the solver writes. `QuickRun::rects` keeps its stale
"Run" entry while Stop is drawn (the tests use it as the toolbar's place; Stop is drawn in the same place under the key
"Stop", "Stopping..." under its label).

**Placement.** New: `crates/rcfd-studio/src/stop.rs` (model: `StopState`, `View`, `view`, `stop`, `run_refusal`,
`hover`, `drain`, `stopped_cleanly`) and `src/stop_ui.rs` (drawing, the shortcut, `allow_run`). Hunks in shared files:
`lib.rs` (two `mod` lines), `studio.rs` (the field, its default, `close_project`, `drain` calls `stop::drain`),
`quick_run_ui.rs` (the Run button's lines inside `if !stop_ui::button(ui, st) { ... }`; F5 asks `stop_ui::allow_run`),
`runs.rs` (`RunRow::exit_code`, two lines in `state_label`), `poll.rs` (the row's `exit_code` from `status.json`, and in
`same_rows`). The edge `rcfd-studio -> rcfd-runner` and `-> rcfd-api` exist; no new crate or dependency line.

## Registered checks

Release builds of this crate's tests with `rcfd` and `rcfd-runner` of the same profile (`common::binaries`), `--test-threads
1`, on the gate host, which other agents' builds and runs share. Test file `crates/rcfd-studio/tests/stop_aa.rs`. The
drawn app is the headless 1280 x 800 frame of the existing tests (`g4::app_with`, tooltips without delay); clicks are
the recorded button rectangles (`g4::click`); "drawn" means a text shape of the frame equal to the button's text
(`"  Stop  "`, `"  Stopping...  "`, `"  Run  "`). Project: `g4::project` (`max_runs` 3, `cpu_threads` 8, `poll_ms`
100), `cases/cyl.toml` = the wind tunnel's shipped `cylinder-re100.toml` (64 x 48, PISO, `end_time` 80, `ts::cylinder(64,
48)`), Run's default plan (the automatic time step of ADR 0153, the store proposal every 0.8 s, f32). The runner is
started directly (ADR 0128 rule) except in AA3. Each test kills, at its end, only solver pids of its own runs' `launched`
events (`RunnerProject`'s drop).

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| AA1 | Stop by the drawn button: Run clicked, wait (at most 60 s) until the run is shown running with at least 2 committed snapshots in `runs/<id>/fields`, then (a) and (b), then a click on Stop | (a) while it runs the toolbar draws Stop and not Run; hovering Stop draws `stop::hover`'s text, which names "last snapshot", "\"stopped\"" and "Ctrl+."; (b) F5 starts no second run (one run folder after 1.0 s of frames) and the last message is `stop::run_refusal`'s; (c) within 5.0 s of the click's release the model labels the run "stopped" and the toolbar draws Run again; (d) every frame drawn after the release and before (c)'s draws "Stopping..." and not Run, and `run_refusal` refuses, with at least one such frame; (e) `status.json`: `cancelled`, `exit_code` 7, reason exactly `cancelled on request; the solver stopped cleanly after step N` with N = its `step` >= 1, no pid; `events.jsonl` exactly one `cancel_requested` and no `killed`; `metrics.json` `solver_finish.state` `interrupted`; (f) the store opens with its footer (not a scanned index), holds at least 3 snapshots, its last snapshot's step is N and its time bit-equal to `status.json` `time`; the last line of `residuals.jsonl` is step N; (g) within 5.0 s of (c) the viewport shows the run with as many snapshots as the store and a frame, and the Runs panel draws the run's line with "stopped"; (h) after the click exactly one message starts with "stop requested for run", none with "Stop: " or "cancel run", and no start error is shown | 5.0 s: ADR 0133 R3's cancel limit (the runner's 100 ms poll, one step of about 17 ms, the store's last snapshot and footer, the run worker's 250 ms) | C1: the solver's process killed instead of Stop (G5's route): (c) not met within 5.0 s and (f) fails (no footer, or a last snapshot that is not the status step). C2: the route before (`stop::Switches::no_stop`): while the run runs the toolbar draws Run and no Stop ((a) fails), and a click on Run queues a second run (two run folders within 10 s) while the first is still running. C3: Run drawn while stopping (`Switches::run_while_stopping`): (d) fails (at least one frame between the release and (c) draws Run) |
| AA2 | Ctrl+.: as AA1 up to the 2 snapshots; Escape (key) in one frame and a plain full stop (the key without modifiers and its text) in the next, 2.0 s of frames; then Ctrl+. (the key `Period` with `Modifiers::COMMAND`) | after Escape and ".": the run still running, no `runs/<id>/cancel`; after Ctrl+.: AA1 (c), (e) and (f) | D5 | (Escape and the plain key are this check's own control of the shortcut) |
| AA3 | Stop before the run is queued and before the runner runs: no runner alive (lock probe false); Run clicked; in every frame while the toolbar shows the start in flight (D4.2), `stop::stop` and Ctrl+. are tried; once the run is queued (D4.3), Stop is clicked (whether the runner then held the lock is reported) | while in flight: neither asks for anything (`stop` returns false, no message, nothing stopping); after the click: within 20.0 s the run is terminal and `cancelled`; launched no solver: reason `cancelled before launch`, label "cancelled"; launched one (the runner was faster): label "stopped"; either way no start error, no message "Stop: " or "cancel run", and the toolbar draws Run again | 15 s: ADR 0133 R9's runner start budget (PowerShell, then the direct start), plus the stop limit | (the in-flight frames are the control: Stop is drawn but asks nothing) |
| AA4 | Stop after the run ended: "Simulate for" 0.5 s, Run, the run finished (at most 60 s), 0.5 s of frames; then `stop::stop` and Ctrl+., 1.0 s of frames | the toolbar's state is Run before (D4.1); `stop` returns false; no message added; `status.json` byte-equal to before; no `runs/<id>/cancel`; the toolbar draws Run and no Stop | the brief | (none) |
| AA5 | The label (model, no runner) | `state_label`: cancelled with exit code 7 "stopped"; cancelled without one, or with 1, "cancelled"; failed with 7 "failed"; finished "finished"; queued held "held"; running "running"; with no project `stop::view` is Run and `stop::stop` returns false and says nothing | D2 | (the table's other rows) |
| AA6 | The rest | the whole `rcfd-studio` suite as at 4e795b1 (its ignored tests ignored); `cargo clippy -p rcfd-studio --all-targets -- -D warnings`; `cargo fmt -p rcfd-studio -- --check`; `rcfd-cli`'s `workspace_graph` | unchanged behaviour outside D1-D8 | (their own) |

Reported without a limit: AA1's time from the click to the run's end on disk, its number of frames in (d), the step N
and the number of snapshots; AA3's frames in flight and which branch it took.

## Not in CI

* With a real window: the look of the red Stop next to the case label, the tooltip's width, Ctrl+. on a keyboard layout
  where the full stop needs a modifier, Cmd+. on macOS.
* A run that does not stop within the grace (a step longer than 10 s): the hover's last sentence; not run.

## Out of scope

A `stopped` state in `status.json` (question 1); continuing a stopped run (a restart file at stop, question 2); renaming
the strip's and the Runs panel's Cancel (question 3); a Stop for runs that the toolbar did not start (the strip and the
Runs panel have Cancel).

## Files

`crates/rcfd-studio/src/{stop,stop_ui}.rs`, `crates/rcfd-studio/tests/stop_aa.rs` (new); hunks in
`crates/rcfd-studio/src/{lib,studio,quick_run_ui,runs,poll}.rs`; this ADR.

## Amendment 1 (2026-10-29, after the first registered run, before any corrected run)

The first registered run of `stop_aa` (release, `--test-threads 1`, at 8f35938) passed AA1 C1, C2, C3, AA4 and AA5 and
failed AA1, AA2 and AA3. What I knew when writing this: that run's output, and one diagnostic run of AA3 alone with a
throwaway print (not kept, not counted).

1. **AA1 and AA2 missed (e)'s events limit.** Both runs had 0 `cancel_requested` events against the registered "exactly
   one" (and 0 `killed`). Every other item of AA1 that ran before the failed assertion was within its limit (the run's
   numbers: "stopped" shown 0.213 s after the click, 35 frames between, all drawing "Stopping...", F5 refused; exit code
   7, the clean-stop reason at step 240, no pid; `solver_finish` interrupted; the store closed with its footer, 4
   snapshots, the last at step 240 with the status time's bits; `residuals.jsonl` at 240; viewable after 0.6 ms; the
   Runs panel's line `... stopped 240 ...`; one "stop requested" message). Diagnosis, read from code after the miss:
   the runner's tick (`runner.rs`, the active-run loop) handles a solver that has exited (`finalize`, then `continue`)
   before it looks at the cancel request, and writes `cancel_requested` only when a tick sees the request while the
   solver is still alive. A solver that stops within one step (about 17 ms here) of the request is usually gone at the
   runner's next tick (100 ms), so the event is absent. My registration misread the runner; the stop itself is as
   intended. Per the house rules AA1 and AA2 stay as registered, as the ignored `aa1_..._known_miss` and
   `aa2_..._known_miss` tests with the limit intact.
   **AA1b, AA2b (new ids, registered here before they run):** AA1 and AA2 unchanged (procedure, every other limit and
   the controls) except (e)'s events: "at most one `cancel_requested`, and no `killed`". Reason: the kill is the
   event that matters (a killed solver writes no final snapshot); C1 still fails AA1b (its (c) and (f)).
2. **AA3 set-up defect (test code).** The run failed before reaching any limit ("the run was not queued" after 60 s).
   The diagnostic run showed why: the test pressed Ctrl+. in every frame while the start was in flight, and the frame
   in which the run was queued drew Stop enabled (D4.3), so that Ctrl+. stopped the queued run, correctly; the test
   then never saw state D4.3 and waited. Corrected: while the start is in flight only `stop::stop` (the model call,
   which reads the view it acts on) is tried, and frames are drawn without keys. Limits unchanged. (In the diagnostic
   run the run so stopped ended, no error was shown, and Run was drawn again; not counted.)

## Amendment 2 (2026-10-29, during AA6's suite run, before the corrected run of J2)

AA6's suite run (release, `--test-threads 1`, at 995dd42) failed ADR 0144's J2
(`live_preview_run_j::j2_a_run_start_switches_to_run_and_a_case_click_back_to_mesh`): "the run was not selected within
60 s" in its step (d). Read from J2's code: (d) waits only until `quick_run::blocked` is empty and clicks "Run" at the
recorded rectangle; the first run of the same case (300 steps) was queued or running then (`blocked` is empty as soon
as `ensure_runner` returned), so with D3/D4 the toolbar drew Stop in that place, the click was a Stop of run 1 and no
second run was queued. This is D4's intended change (Run of a case whose toolbar run is live is Stop), not a defect of
this job's code; J2's own set-up assumed Run there. **Corrected set-up of J2 (ADR 0144's limits unchanged):** (d) and
its control wait until `blocked` is empty *and* the toolbar's place holds Run (`stop::view` is `View::Run`), i.e. the
case's earlier run has ended, before the click. Recorded before J2 runs again. Every other failure of that suite run is
listed in the addendum with its handling.

## Addendum: results (2026-10-29, branch `worktree-agent-a0f10be20ce1b479a`, release builds on the gate host)

Commits: 8f35938 (registration, code and tests), 995dd42 (amendment 1), b38949f (amendment 2), this addendum. All runs
release, `--test-threads 1`, `rcfd` and `rcfd-runner` of the same build, own target folder; other agents' builds and
runs shared the host. First run of `stop_aa` at 8f35938 (AA1, AA2, AA3 failed: amendment 1); the counted run of every
AA check below is the second, at 995dd42, unless said.

| Id | Limit | Measured | Result |
|---|---|---|---|
| AA1 (registered) | (a)-(h), (e) with exactly one `cancel_requested` | first run: 0 `cancel_requested`; every item checked before it within its limit (amendment 1) | **miss**: `aa1_..._known_miss`, ignored, limit intact |
| AA1b | AA1 with (e)'s events "at most one, no `killed`" | (a) Stop drawn, Run not; hover drawn, names the three phrases; (b) F5: 1 run folder, the refusal message; (c) "stopped" and Run 0.779 s after the click; (d) 35 frames, all "Stopping...", F5 refused; (e) exit code 7, "cancelled on request; the solver stopped cleanly after step 158", no pid, 1 `cancel_requested`, 0 `killed`, `solver_finish` interrupted; (f) footer, 3 snapshots (2 before the stop), the last at step 158 with the status time's bits, `residuals.jsonl` at 158; (g) viewport 3 of 3 snapshots and a frame after 1.4 ms, Runs panel `... stopped 158 2.3189e0 ...`; (h) one "stop requested" message, no error | pass |
| AA1 C1 (kill) | (c) not met and (f) fails | no "stopped" in 5.0 s (803 frames; `failed`, "ended without a finish event (exit code 1)"); store without footer, last snapshot step 138 against status step 164 | fails as required |
| AA1 C2 (route before) | (a) fails; a second run queued while the first runs | Stop not drawn, Run drawn; second run folder within 10 s, first `running` | fails as required |
| AA1 C3 (Run while stopping) | (d) fails | 54 of 54 frames between drew Run | fails as required |
| AA2 (registered) | as AA1's (e) | first run: 0 `cancel_requested` | **miss**: `aa2_..._known_miss`, ignored, limit intact |
| AA2b | Escape and "." stop nothing; Ctrl+.: (c), (e) as AA1b, (f) | after Escape and ".", 2 s: running, no cancel file; Ctrl+.: "stopped" after 1.240 s (63 frames), step 152, exit 7, 1 `cancel_requested`, 0 `killed`, footer, 3 snapshots, last at 152 with the status time's bits | pass |
| AA3 | in flight nothing asked; ended within 20.0 s, `cancelled`, no error, Run drawn | 148 frames in flight, `stop` never asked; Stop clicked with the run queued and no runner holding the lock; ended 0.458 s later, "cancelled before launch", label "cancelled", 0 launched; no start error, no "Stop: " or "cancel run" message; Run drawn | pass (after amendment 1 item 2) |
| AA4 | no-op after the end | view Run; `stop` false; messages, `status.json` unchanged; no cancel file; Run drawn, Stop not | pass (both runs) |
| AA5 | the label table; no project: Run, false, silent | as registered | pass (both runs) |
| AA6 | the `rcfd-studio` suite; clippy; fmt; `workspace_graph` | suite at 995dd42: 62 test binaries, 280 passed, 1 failed (J2, amendment 2), 15 ignored (the 13 ignored before and this job's 2 known misses); J2 after amendment 2 (b38949f), run alone: pass (run 1 selected 0.134 s, run 2 0.103 s after the click; its control as before); `cargo clippy -p rcfd-studio --all-targets -- -D warnings` clean; `cargo fmt -p rcfd-studio -- --check` clean; `workspace_graph` 3 passed | pass (J2 after amendment 2) |

Reported without a limit: AA1b's run had stored 2 snapshots when Stop was clicked and stopped at step 158 (about 2.3 s
simulated); AA1's first (missed) run stopped 0.213 s after the click at step 240 with 4 snapshots; `cancel_requested`
was present in the second runs of AA1b and AA2b and absent in the first runs of AA1 and AA2, as amendment 1's
diagnosis predicts (it depends on whether a runner tick falls between the request and the solver's exit).

Deviations and notes:
* AA1 and AA2 missed a limit that misread the runner (amendment 1); AA1b and AA2b were registered after the miss. The
  limit that mattered (no kill, the final snapshot) was met in all four runs.
* AA3's first run had a test-code defect (amendment 1 item 2); its in-flight check is weaker than registered (Ctrl+. is
  not pressed in flight, only the model call `stop::stop` is tried there; the drawn Stop's greyed-out state is not
  checked by a click).
* J2 (ADR 0144) needed a set-up change because D4 changes what its click hits (amendment 2). The whole suite was not
  run again after that change; J2 was.
* `quick_run_ui.rs`: the Run button's 12 lines are re-indented inside the new `if` (the merge with job 48X may conflict
  there; the intent is "draw Run unless `stop_ui::button` drew Stop").
* The race "the run ends between the click and the request" (D1's second status read) is read from code, not tested.

Not run: the Studio tests in a debug build; Linux (the new process tests have never run there); the "Not in CI" items
(a real window, Cmd+. on macOS, keyboard layouts); a stop whose solver step exceeds the 10 s grace (the hover's kill
sentence); the workspace gate and workspace-wide clippy; other crates' tests (no other crate changed).

## Questions for the maintainer

1. **A `stopped` state in `status.json`?** Today "stopped" is the Studio's label for `cancelled` with exit code 7.
   Options: (a) keep it a Studio label; (b) add `RunState::Stopped` in `rcfd-project`, written by the runner for a clean
   stop (touches the record contract, the runner, `rcfd-mcp`, the CLI's `wait`, and ADR 0133 R3 / ADR 0141 G6, which
   expect `cancelled`). Recommendation: (a) now; (b) only if another client (MCP, CLI) needs to tell the two apart.
2. **Continue a stopped run?** The runner passes no `--checkpoint`, so a stopped run cannot be continued. Options: (a)
   leave it; (b) the runner passes `--checkpoint runs/<id>/restart.rcfd` so a clean stop writes a restart file (only for
   the cases `save_restart` supports) and the Studio offers "Continue". Recommendation: (b) as its own job.
3. **Rename the strip's and the Runs panel's "Cancel" to "Stop"?** They send the same request and end the same way.
   Recommendation: yes, labels only, in a small job (G6 of ADR 0141 clicks "Cancel" and needs the same set-up change).
4. **F5 while the selected case's run is going** now refuses with a message (before: a second run of the case was
   queued). Options... still starts any run. Recommendation: accept.
5. **The runner's `cancel_requested` event** is missing when the solver stops before the runner's next tick (amendment
   1). Should `finalize` write it when it finds the cancel request? Recommendation: yes, a one-line runner change in its
   own job, so the event log always records who ended the run.
6. **`QuickRun::rects` keeps a stale "Run" entry** while Stop is drawn (tests use it as the toolbar's place).
   Recommendation: accept; clear it only if a test should fail loudly when it clicks Run under Stop.
