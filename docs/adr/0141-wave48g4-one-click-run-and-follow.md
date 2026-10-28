# ADR 0141: Wave 48G item 4, one click from a case to a picture: Run, follow the run live, a start that says why it failed

Status: **registered** 2026-10-28, before any registered run (the code and the tests below were written and compiled
first; what was run before is listed under "What I already knew"); amendment 1 after the G1 miss; results in the addendum;
not yet accepted by the maintainer. The maintainer's request of 2026-10-28 ("48G, first run
to first picture", item 4 of 4; items 1-3, mesh preview from a case, case starter menu and the store-fields default, are
done by other agents in parallel and are not part of this ADR). House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md):
limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
intact; a defective set-up gets an amendment written before the corrected run; defaults of existing code stay
bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Parent: [plan](../ui-restart-plan.md)
section 2, ADRs [0131](0131-wave48b-studio-application-shell.md), [0132](0132-wave48c-viewport-integration.md) and
[0133](0133-wave48d-run-control-and-live-monitoring.md). Roadmap: F-266.

## Context (read at main f3bf005)

* The maintainer reports that nothing in the Studio window can be done end to end: there is no one-click path from a case
  to a picture. Read from the code: a run starts only through Runs > "Start run..." (choose the case again, read the plan,
  Start). The new run is not selected, so the viewport (which follows the selected run, ADR 0132) and the plots stay on
  whatever was shown; the user has to find the run in the Runs panel. The Viewport tab is not brought forward.
* `Client::start` is `submit` then `ensure_runner`; when the runner does not start it returns only the error, and the run
  it queued (its id) is not reported (`crates/rcfd-runner/src/client.rs` line 109).
* Programs: the Studio's runner is `RCFD_RUNNER_BIN`, else `rcfd-runner` next to the Studio (`default_runner_exe`); the run
  worker takes it when the project opens. The runner starts `RCFD_BIN`, else the `rcfd` next to itself (`Config::solver`);
  `Client::submit` reads that `rcfd --version`. `cargo run -p rcfd-studio` builds only the Studio (ADR 0132 checklist item
  1). With no runner next to the Studio, `ensure_runner` on Windows waits 5 s for the PowerShell route, then the direct
  start fails ("cannot start <path>: ..."): the run folder stays queued and the message names neither where else to look
  nor how to build (read from code; registered below as G7's control).
* `rcfd-runner serve` reads `.rcfd/runner-config.json` before it opens `runner.log` and exits 1 on a bad file; under the
  PowerShell route its stderr is discarded, so the Studio would wait up to 15 s and point at an empty log.
* A solver older than the flags the runner passes on every launch (`--store-dir`, `--status-monitors`, ADR 0130) fails
  every run at launch with a usage error.
* Follow: the viewport's `Snaps` handler moves to the newest snapshot when the last one was shown and linear mode is off
  (`at_end`, `viewport.rs`). There is no switch and no indication, linear mode never follows, and a scrub to the end that is
  still pending when a snapshot arrives is overridden.
* For a run without a store the viewport says "no stored fields: <path> not found (start the run with "Store fields" on)"
  (48C) also while a store-on run is queued or before its first snapshot, and after a run failed: the text is wrong in
  both cases.
* The field-store hook writes the final state for every outcome of `runctl::drive`, a diverged run included
  (`fieldstore::Hook::finish`), so a run that ends through the solver always has a snapshot. A run has none when the
  runner refuses it before launch (pre-launch check, budgets), when the solver dies (crash, kill) or when the store is off
  or disabled.

## Decisions (as implemented)

1. **Run (toolbar, F5).** A toolbar under the menu bar: a green Run button, the selected case, "F5", "Options..." (opens the
   existing Start run dialog on that case; the dialog is unchanged), where `rcfd-runner` and `rcfd` were found, and
   "Search again". The selected case is the one open in the case editor, else the one being loaded into it (a click in the
   Project panel opens a case there). Run is refused, with the reason on the button's hover and in the status bar, when no
   project is open, no case is selected, the case has unsaved edits or a conflict (a run uses the file on disk), or a start
   is in flight. F5 does what the button does.
2. **The start (`quick_run::run_selected_case`, the click-equivalent call).** On a short-lived control thread: find the
   programs (decision 3); run `rcfd --help` (no window, at most 10 s) and refuse a solver whose help lacks `--store-dir` or
   `--status-monitors`; when no runner holds the project's lock, read the runner budgets with the runner's own reader
   (`Config::load`) and refuse a file the runner would refuse; read the case and make the default plan exactly as the
   dialog does (`StartPlan::new`: priority 0, 1 thread, memory estimated by the runner's pre-launch check, the store
   proposal); `Client::submit`; report the id; `Client::ensure_runner` (ADR 0133's route, either way of starting the
   runner); report its result with the last lines of `runner.log` on failure (the run stays queued). Every refusal is a
   located message in the strip and the status bar; nothing is queued before the plan is made.
3. **Finding the programs (`bins::Search`).** `RCFD_RUNNER_BIN` when set (taken as given: a missing file is an error, not
   a reason to search elsewhere); else the Studio's folder, each folder of `PATH`, then `target/release` of every checkout
   (an ancestor holding `Cargo.toml`) of the Studio's folder and of the current folder, and `CARGO_TARGET_DIR/release`. A
   folder counts only when it holds both `rcfd-runner` and `rcfd` (or `rcfd-runner` and an existing `RCFD_BIN`). The error
   lists every folder searched in order, programs found alone, and the build command
   `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli -p rcfd-mcp`. `main` searches once at start (file
   checks only) and gives the run worker the runner found; every Run searches again, so Run uses a build made while the
   Studio is open. Known limit: the open project's run worker keeps the runner it was given when the project opened, so
   the Start run dialog and "Start runner" use that one until the project is opened again (question 3).
4. **After the start.** When the id arrives the run is selected (the run worker reads its series, events and log; the
   viewport follows it) and the Viewport panel is shown (it is opened again when it was closed; above the Plots since
   amendment 1) and put in front of its place; Plots is put in front of its place when it does not share the Viewport's. A run started by Run that fails before
   its first shown snapshot puts the Log panel in front the same way, once, and says why in the status bar.
5. **Follow latest (`Viewport::follow`).** On for a newly shown run. Each new snapshot of a live run is shown while it is on.
   A scrub to an older snapshot (slider, `|<`, `<`, play from the start) turns it off: the slider still extends and the view
   stays. A scrub to the newest snapshot (`>|`, the slider's right end, play reaching the end) or the "follow latest" box
   turns it on. It follows in linear mode too (the newest snapshot's time samples the stored values exactly).
6. **The strip.** Above the status bar, shown while a project is open: Pause, Resume and Cancel (the run worker's request
   files, ADR 0133, on the selected run; enabled as the Runs toolbar enables them), and the selected run's line: id, case,
   state (as `status.json` spells it, or "held"), step, time, the last `u`, `p` and continuity residuals, the queue
   position, the shown snapshot and whether it follows. Below it: a failed or cancelled run's reason (from `status.json`)
   and the last line of its log, the start in flight with its stage and age, and the start's or the runner's error.
7. **A run without a snapshot.** While the selected run is queued, running or paused and its store file does not exist
   yet, the viewport's "no stored fields" text is replaced by its state and step and, for a run started by Run, the store
   spacing of its plan (or that the plan stores no fields); once the solver has created the store file, 48C's "The store
   has no snapshots yet." stays, and the strip says "no stored snapshot yet". Once it has ended without one (checked on a control thread: the store file is missing or has no
   snapshot), the viewport says that it failed (or was cancelled, or finished without stored fields) before writing its
   first stored snapshot, with the reason from `status.json`, the last line of its log, and for a finished run a
   field-store warning from its events when there is one. The 48C texts stay for runs this does not apply to.
8. **Scripts.** `scripts/studio.ps1 [-NoBuild] [-DryRun] [PROJECT_DIR]` and `scripts/studio.sh [--no-build] [--dry-run]
   [PROJECT_DIR]`: build the four programs with `cargo build --manifest-path <root>/Cargo.toml --release -p rcfd-studio
   -p rcfd-runner -p rcfd-cli -p rcfd-mcp` (`CARGO` and `CARGO_TARGET_DIR` honoured), then run
   `<target>/release/rcfd-studio [PROJECT_DIR]` in the foreground. A failed build exits with its code and starts nothing; a
   missing Studio after the build is named. `--dry-run` prints both commands one argument per line (`build> `, `start> `).
   README gets the two one-liners.

## Placement and dependencies

New modules `crates/rcfd-studio/src/{bins,quick_run,quick_run_ui}.rs`. No new workspace edge (`rcfd-studio` already
depends on `rcfd-runner`, `rcfd-tsstore` and `egui_dock`); no new third-party crate; `Cargo.lock` unchanged. Hunks in shared
files, kept small for the merge of the parallel 48G items:

| File | Hunk |
|---|---|
| `src/lib.rs` | three `pub mod` lines |
| `src/studio.rs` | field `quick`, its default, `wake` made `pub(crate)`, `quick.close()` in `close_project`, `quick_run::drain` at both ends of `drain` |
| `src/ui.rs` | one line in `StudioApp::ui`: `quick_run_ui::show` after the status bar |
| `src/main.rs` | six lines: search once at start, set `runner_exe` |
| `src/viewport.rs` | field `follow` (default, `reset_run`), the `Snaps` condition, `set_pos` and `show_snapshot` set it, new `set_follow` |
| `src/viewport_ui.rs` | the "follow latest" box in the time row |
| `README.md` | the two script one-liners |

Merge notes for the parallel 48G items: G1 and G5 rely on the dialog's default store proposal (item 3 may change it). G1
needs stored snapshots while the 3000-step run runs; G5 needs none before its kill (about step 1000 of 1 000 000), and
asserts that before killing, so a changed proposal fails G5 loudly at that precondition, not as a regression. The
viewport texts of decision 7 and the Log focus apply only when the viewport shows the selected run
(`viewport.run == selected_run`); a case preview (item 1) must keep a selected run taking precedence over the preview.

## Registered checks

All in release, `--test-threads 1`, output to files; `rcfd` and `rcfd-runner` of the same build next to the test's
`rcfd-studio` (`common::binaries`, which also refuses a stale `rcfd`). Process tests use the 12 x 12 lid cavity of ADR 0133
(with its `lid_drag` report; the step count in the case text, `max_iterations`, because Run takes no overrides) and ADR
0133's project set-up (`RunnerProject`: runner budgets `max_runs` 3, `cpu_threads` 8, `poll_ms` 100; its drop guard shuts
the runner down and kills only its runs' solvers). They drive the drawn app (`StudioApp::ui`, headless frames of 1280 x 800,
about every 5 ms); "click" is a pointer press and release at the centre of the button's rectangle drawn in the previous
frame (`QuickRun::rects`). The Run button searches only the test build's runner (`Search::explicit`, as
`RCFD_RUNNER_BIN`). The plan is the dialog's default plan; its `--set` list is printed, not asserted (48G item 3 may change
the store proposal). Debug builds run a quarter of the steps (G1, G6, G1 control); the limits are registered for release.

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| G1 | End to end, no runner alive: click Run on the cavity case (3000 steps) | (a) run selected and listed 5.0 s after the click; (b) shown running 20.0 s after; (c) first snapshot shown 1.0 s after it is committed; (d) slider never shrinks, 3 or more lengths seen while running, 2 or more snapshots shown, final length = store count, newest shown 1.0 s after the final length is held; (e) plot marker = shown step at every frame; (f) final state, step, time (bits) = `status.json`, strip and a drawn text say "finished" and "step N", shown step = N; (g) GPU: no error, 1 upload, at least 1 render; (h) Viewport reopened and in front, Plots in front | (b) R9's 15 s plus the pre-launch check; (c), (d) four viewport poll periods of 250 ms, B6's 4-period rule | auto-select and focus off: no frame within 5.0 s of the first committed snapshot, nothing selected, Viewport still closed, Outputs still in front |
| G2 | Follow latest, a store written by the test while shown (no runner) | on: each of 6 new snapshots shown 1.0 s after its commit, marker = its step; after a scrub to snapshot 0: slider extends 1.0 s after each of 3 commits and the view stays at 0 for 0.5 s after; the box shows the newest 1.0 s after, the next commit is followed 1.0 s after; `<` off, `>|` on, slider middle off, right end on; the box is drawn | as G1 (c) | the on-check with follow off fails; the off-check with follow forced on fails |
| G3 | Finding the programs, fixture folders | exact: nothing found lists the Studio folder, both PATH folders and the checkout's `target/release` in order, the build command and `RCFD_RUNNER_BIN`; found in PATH, the Studio folder first, the checkout, programs alone named; `RCFD_RUNNER_BIN` taken as given; `RCFD_BIN` with a lone runner; the built `rcfd` passes the solver check, a stale one and a non-program are refused naming the file | the request: located, actionable, search order | the pre-change rule (runner next to the Studio only) finds nothing when the programs are on PATH |
| G4 | A refused start: a case whose mesh file does not exist, runner served | the strip shows the failed run's reason 5.0 s after the click; the reason names the pre-launch check and `missing.msh`; it is drawn; the viewport says "failed before writing its first stored snapshot" with it; Log in front | R3's 5.0 s | explanation off: not shown within 5.0 s, the viewport lacks the reason |
| G5 | Failed before the first snapshot: the cavity with 1 000 000 steps (store every 10 000), solver killed at step 20 or later with no snapshot | while running a drawn text says "no stored snapshot yet" (the strip) and none is the 48C text; 5.0 s after the kill the viewport says "failed before writing its first stored snapshot" with `status.json`'s reason (which names the missing finish event); the strip has the reason; the message is drawn, the 48C text is not | R3's 5.0 s | explanation off: not within 5.0 s, the viewport lacks the reason |
| G6 | Pause, Resume, Cancel clicked in the strip, cavity 15 000 steps | R3's limits: paused shown 2.0 s after the click; no new step for 2.0 s; a new step 5.0 s after Resume; cancelled 5.0 s after Cancel; enabled controls exactly: running (Pause, Cancel), paused (Resume, Cancel), cancelled (none) | ADR 0133 R3 | R3's switch (resume sends pause): no new step within 5.0 s |
| G7 | F5 with the programs missing (two empty folders searched) | the start's error shown 2.0 s after the key; drawn text has the build command and both folders; no run folder | the request | 20 frames without the key start nothing; the pre-change route (Start through the run worker with the runner missing) leaves a queued run folder and names no build command |
| G8 | Scripts, argument handling (`python -m unittest scripts/test_studio_scripts.py`, each of Windows PowerShell, PowerShell 7, POSIX sh that is installed; a missing one prints SKIP) | exact argument lists of `--dry-run` (the build, the start with no project, a project with a space and a quote as one argument, `--no-build`); two projects and an unknown option exit 2; `-h` exits 0; a fake cargo that fails with 3: exit 3, nothing started; a fake cargo that succeeds and no Studio: exit 1 naming it | | (the scripts' own refusals) |
| G9 | Every earlier `rcfd-studio` test (B1-B9, R1-R12, V2-V11, `viewport_live`, the chat tests, parity) | pass unchanged; R6 stays the ignored known miss | follow on by default keeps 48C's live behaviour | (their own) |

## Set-up details

* **G1** (`tests/oneclick_e2e_g4.rs`, its output to a file as ADR 0133 R9: the runner's direct-start fallback hands the
  runner the caller's inheritable handles). Before the click: the case loaded in the editor, `layout::hide(Viewport)`,
  Outputs in front of its place (Plots, Log, Outputs), the first adapter of `rcfd_render::adapters()` attached
  (`ViewGpu::from_adapter`; a SKIP line for (g) when there is none), no runner holding the lock. Each frame the test reads
  the model and, until it holds a snapshot, opens the store `runs/<id>/fields` itself (`Reader::open`); (c) is the time from
  the test first seeing a committed snapshot to the first frame showing a snapshot (a frame shown first counts as 0).
  "Listed" is the selection plus the run worker's row; "running" is the row running or terminal. (h) is read in the frame
  where the run is first listed. The end is the row terminal and the slider at the store's count; (d)'s last limit runs
  from the first frame holding both.
* **G1 control** (`tests/oneclick_g4.rs`, runner served directly, the same layout before the click,
  `QuickRun::switches.no_select` and `no_focus` on): the frames run until the store has a snapshot, then 5.0 s more.
* **G2** (`tests/follow_g4.rs`): ADR 0132's live fixture (`vp::run_project`, state running, the 8 x 8 cavity store written
  with `Writer`, each snapshot committed and flushed), Studio without watcher and run worker. "Scrub" is the model call the
  button makes (`show_snapshot(0)` for `|<`, `step(-1)` for `<`, `show_snapshot(last)` for `>|`, `set_pos` for the slider,
  `set_follow` for the box). The drawn check is one headless frame with the Viewport panel's time row.
* **G3** (`tests/bins_g4.rs`): folders `studio`, `p1`, `p 2` (PATH, in that order), `ws` with `Cargo.toml`, current folder
  `ws/sub`; programs are files named `rcfd-runner(.exe)` and `rcfd(.exe)`. The stale solver is a copy of `rcfd-runner`
  named `rcfd` (its help lacks both flags); the non-program is a text file named `rcfd(.exe)`.
* **G4**: `cases/missing_mesh.toml` (`mesh.source = "file"`, `file = "missing.msh"`, laminar `simplec`, 10 steps). The time
  runs from the click's release frame. The control waits a further 5.0 s before reading.
* **G5**: the solver pid is read from `status.json` once the row is running at step 20 or later and the test has checked
  that the store holds no snapshot; it is killed with `rcfd_runner::sys::kill` (the drop guard's call). The time runs from
  the kill.
* **G6**: R3's procedure through the strip's buttons; "steps" are the largest residual step plotted; the control switch is
  set before the project opens (the run worker reads it at start).
* **G7**: the search's Studio folder and PATH are two empty temporary folders; the key is one frame with F5 pressed and
  released. The pre-change control opens the project with the run worker's runner set to a missing file and calls
  `Studio::start_run` with the dialog's default plan, then reads the messages for up to 20 s.
* **G8**: cargo is never run (dry runs, or `CARGO` naming a fake that records its arguments); no Studio is started.

## What I already knew (rule d)

* The code reading above. The code and the tests of this ADR were written and compiled (`cargo build --release -p
  rcfd-studio`, `cargo test --release --no-run` of the four new test files, `cargo clippy --release -p rcfd-studio
  --all-targets -- -D warnings` clean, `cargo fmt -p rcfd-studio`); no new test had run.
* Explorations on this host, not checks: `rcfd run --check-only --status-json` of a case with a missing mesh file exits 3
  with the diagnostic `mesh.file: meshes/missing.msh: io error: ...`; `rcfd run <that case> --print-settings` exits 0, so
  the case text parses (`Case::load`) and `StartPlan::new` should accept it, the refusal coming from the runner's
  pre-launch check (not yet seen through the Studio); `rcfd validate` of it fails (it builds the simulation). Windows
  PowerShell 5.1 with `-File` hands `-NoBuild`, `--dry-run`, `-h` to a script's `$args` as strings. The release build of
  the four programs took 3 min 12 s (cold target folder, `-j 4`, other builds running on the host).
* Timings used for the step counts, from ADR 0133 amendment 2 (not measured here): the cavity takes about 0.87 ms per step
  in release, so 3000 steps run about 2.6 s and the 1 000 000-step case writes its first snapshot (step 10 000) about 9 s
  after launch.

## Amendment 1 (2026-10-28, after the G1 miss, before G1b ran)

**G1 missed limit (h)** in its registered run (commit ae3e85f, release, output to a file, NVIDIA RTX 5080 Vulkan): right
after the run was listed the Viewport was open and in front, but Plots was not in front ((true, true, false) against
(true, true, true)). Every other G1 limit was met in that run: (a) listed 0.314 s after the click; (b) running 5.339 s;
(c) first snapshot committed at 5.333 s, shown at 5.361 s (0.028 s); (d) slider lengths 0, 2, 17, 35, 55, 70, 71, 90, 100
(8 while running), shown snapshots 2, 17, 35, 55, 71, 90, 100, the newest shown in the frame that first held the final
length; (e) no marker difference, final marker 3000; (f) `status.json` finished, step 3000, time 0, equal in the model, the
strip "... finished  step 3000 ... snapshot 100/100 (following the latest)", shown step 3000; (g) one upload, one render at
the first shown snapshot, nine at the end, no error. The test becomes the ignored
`g1_one_click_from_case_to_picture_known_miss` with its limits intact; the reopening it measured stays reachable through the
test switch `QuickRun::switches.reopen_in_focused_leaf`, so the known-miss test measures what missed.

Diagnosis (read from egui_dock 0.21.1, `dock_state/tree/mod.rs`; not a separate run): `Tree::split` makes the new node the
focused one, so in the default layout the focused leaf is the last one split off, the leaf of Plots, Log and Outputs;
`remove_leaf` (closing the Viewport) passes the focus to the sibling, that same leaf; `layout::show` pushes a closed panel
into the focused leaf. The Viewport then shares the Plots' place, and decision 4 rightly does not put Plots in front of it.
In the app the focused leaf is the one last clicked, so the same can happen with any layout.

**Change (registered here before its run).** A closed Viewport is reopened in a new place above the Plots' place, as in the
default layout (`split_above` of the Plots' leaf, the Plots keeping 0.34 of the height, on the Plots' surface, docked or
floating); when Plots is closed too, where `layout::show` puts it (unchanged). Decision 4 otherwise unchanged.

**G1b** (new check id, `tests/oneclick_e2e_g4.rs`): the fixture, procedure and limits (a)-(h) of G1 with the change. The
G1 control (auto-select and focus off) is unchanged and is not run again.

## Addendum: results against the registered limits (2026-10-28, branch `worktree-agent-a9d885d943a633e09`)

Registered runs: release, `--test-threads 1`, output to files, this host (Windows 11, 16 threads; three other agents were
building at the same time). G2-G8 and the G1 control ran on ae3e85f (registration), G1 on ae3e85f, G1b on 6b94b98
(amendment 1). No new third-party crate, no new workspace edge, `Cargo.lock` unchanged.

| Id | Limit | Measured | Result |
|---|---|---|---|
| G1 | (a)-(h) | (a)-(g) met (amendment 1 lists the values); (h) Viewport open and in front, Plots **not** in front | **miss** (h): `g1_one_click_from_case_to_picture_known_miss`, ignored, limits intact |
| G1b | (a) 5.0 s; (b) 20.0 s; (c) 1.0 s; (d)-(h) | (a) 0.209 s; (b) 5.353 s; (c) committed at 5.346 s, shown at 5.473 s: 0.127 s; (d) lengths 0, 8, 24, 25, 42, 57, 58, 72, 88, 100 (9 while running), shown snapshots 8, 25, 42, 58, 72, 88, 100, the newest in the first frame holding the final length (0.0 s); (e) no marker difference, final marker 3000; (f) `status.json` finished, step 3000, time 0, the model equal (time bits), strip `run r-... cases/live.toml finished step 3000 t 0.0000e0 res u 5.10e-16 p 3.76e-13 continuity 3.86e-12 snapshot 100/100 (following the latest)`, shown step 3000; (g) NVIDIA GeForce RTX 5080 (Vulkan): one upload, one render at the first shown snapshot, ten at the end, no error; (h) (true, true, true) | pass |
| G1 control | no frame 5.0 s after the first committed snapshot; nothing selected; Viewport closed; Outputs in front | no frame in 5.004 s, nothing selected, Viewport closed, Outputs in front | fails as required |
| G2 | 1.0 s per new snapshot; the view held 0.5 s; box, `<`, `>|`, slider | on: 6 of 6 shown, worst 0.265 s, marker = step each time; off: slider extended 0.255, 0.011, 0.026 s after the commits, only snapshot 1 shown during each hold; the box showed the newest after 0.005 s and followed the next after 0.259 s; `<` off, `>|` on, slider middle off, right end on; "follow latest" drawn | pass |
| G2 controls | on-check fails with follow off; off-check fails with follow forced on | follow off: snapshot 2 not shown within 1.0 s (snapshot 1 shown, slider 2); forced on: the view moved to snapshots 4, 5, 6 | fail as required |
| G3 | exact | 4 tests pass: the not-found message lists `studio`, `p1`, `p 2`, `ws\target\release` in order with the build command and `RCFD_RUNNER_BIN`; found in `p 2` (PATH), the Studio folder first, the checkout (Dev); programs alone named; `RCFD_RUNNER_BIN` missing is an error naming it; `RCFD_BIN` with a lone runner found; the built `rcfd` accepted, the renamed runner refused ("does not name --store-dir and --status-monitors"), a text file refused ("cannot run it", os error 216) | pass |
| G3 control | the pre-change rule finds nothing in PATH's layout | nothing found | fails as required |
| G4 | 5.0 s | 0.255 s; reason `pre-launch check failed (exit 3): mesh.file: <project>\cases\missing.msh: io error: The system cannot find the file specified. (os error 2) (see <run>\check.json and <run>\log.txt)`, drawn; the viewport `Run r-... failed before writing its first stored snapshot: <reason>. Last line of its log: ...`; Log in front | pass |
| G4 control | not within 5.0 s | not shown in 5.0 s nor 5.0 s later; the viewport kept 48C's `no stored fields: <run>\fields\store.rts not found (start the run with "Store fields" on)` | fails as required |
| G5 | 5.0 s | 0.333 s after the kill at step 955; reason `solver process 33104 ended without a finish event (exit code 1); last step 955 at time 0; last log line: '144 cells, algorithm simplec' (log: ...)` in the viewport message and the strip, the message drawn, 48C's text not drawn; while running the strip read `... running step 411 ... no stored snapshot yet` | pass |
| G5 control | not within 5.0 s | not explained; the viewport kept 48C's "The store has no snapshots yet." (the store file existed with no snapshot) | fails as required |
| G6 | paused 2.0 s; held 2.0 s; moves 5.0 s after Resume; cancelled 5.0 s; enabled controls | paused 0.260 s after the click at step 134; still 134 after 2 s; resumed to 202; cancelled 0.279 s after the click; controls exactly as registered | pass |
| G6 control | no step 5.0 s after the mapped Resume | S0 195, none beyond it in 5.0 s; cancelled 0.329 s after the click | fails as required |
| G7 | 2.0 s; drawn build command and folders; no run folder | 0.003 s; drawn; no run folder | pass |
| G7 controls | without the key nothing starts; the pre-change route | 20 frames: nothing started; pre-change: one run folder left `queued`, the message 7.1 s after the call `the run was not started: cannot start <folder>\rcfd-runner.exe: The system cannot find the file specified. (os error 2)`, no build command, no folder searched | as required |
| G8 | exact | 24 of 24 (8 checks under Windows PowerShell 5.1, PowerShell 7 and Git Bash `sh`) | pass |
| G9 | earlier tests unchanged | 98 passed, 0 failed, 3 ignored (below) | pass |

G9 ran on 6b94b98: `cargo test --release -p rcfd-studio --no-fail-fast -- --test-threads 1`, output to a file, 3 min 41 s.
Every test binary of the crate passes: B1-B9 (`project_b1_b3`, `frames_b4`, `editor_b5`, `watch_b6`, `frame_cost_b7`,
`prefs_b8`, `parity`), R1-R12 (`run_control_r1_r3`, `ingest_r4_r7`, `monitors_plot`, `tick_cost_r6`, `monitors_r8`,
`runner_start_r9`, `start_plan_r10`, `browser_refresh_r12`), V2-V11 (`viewport_v2`, `viewport_model`, `viewport_store`),
`viewport_live`, the chat tests E1-E11 (`chat_e`) and the four new files (G1b-G7 again). That run captured no output, so
it shows pass or fail only, not the measured values. The three ignored tests are the known misses that were ignored
before or by amendment 1: ADR 0133's R6, ADR 0131's tutorials item, and G1.

After the addendum's runs (dcea11d):
* G1's known-miss test, run once with `--ignored` (output to a file): it fails at (h) with (true, true, false), as
  registered; (a)-(g) met again (listed 0.094 s, running 5.477 s, first snapshot shown 0.158 s after its commit). The switch
  therefore reproduces the missed behaviour.
* Leftover processes after all runs: none (`rcfd.exe`, `rcfd-runner.exe`, `fake-claude.exe` listed by command line).
* `scripts/limit_audit.py`, the hits in the new files, read: `bins_g4.rs:87` (class i) compares the constant `BUILD` with
  the literal command the request names, on purpose (a changed constant fails); `oneclick_g4.rs:311` (class i,
  sleep/elapsed) is G7's limit, whose time starts after the control's 20 frames with their sleeps.
* Changed after the registered runs (UI only, found in review): during a start nothing woke the UI, so the strip's age
  counter stood still while the runner started; the strip now asks for a repaint every 250 ms while a start is in flight.
  Re-run on that code (output to a file): G1b pass ((a) 0.089 s, (b) 5.464 s, (c) 0.159 s, (h) (true, true, true)), G4-G7
  and the G1 control pass (G4 0.509 s, G5 0.077 s, G6 paused 0.266 s and cancelled 0.265 s, G7 0.002 s), B4 pass, B7
  median 0.392 ms (min 0.333, p90 0.564; 48D measured 0.339 ms without the toolbar and the strip; limit 8 ms), its control
  13.5 ms.

Reported, not limited:
* The first Run of a session waited about 5.3 s from the click to the running run (G1 5.339 s, G1b 5.353 s), as ADR 0133 R9
  (5.166 s): consistent with `ensure_runner`'s 5 s wait for the PowerShell route and then the direct start (which route
  started the runner is not recorded). Later Runs of the session find the runner alive.
* G1b's first-snapshot delay (0.127 s) against G1's (0.028 s): both within one viewport poll period of 250 ms.

Deviations and notes:
* G1 missed (h); amendment 1 changed the reopening of a closed Viewport and registered G1b before it ran.
* G5's "fails before the first snapshot" is a killed solver, because the store hook writes the final state for every
  outcome of the solver (Context): a diverged run gets a snapshot (possibly of non-finite values, which the 48C viewport
  refuses with a located error), so only a refusal before launch (G4), a crash or a kill leaves none.
* The chat tests of 48E need the `fake-claude` double built for the profile (`cargo build --release -p rcfd-claude --bin
  fake-claude`); the first G9 run stopped at `chat_e.rs` for that reason (14 failed at the double's existence check,
  before any of their limits) and G9 was run again after building it.
* For a finished run without a store, the explanation (decision 7) replaces 48C's text, which named the store path; the
  new text names why there is no store but not the path.

Not run: the "Not in CI" items (a person at the window); Linux (none of the new process tests has run there); a debug
build of the new tests; the workspace gate and workspace-wide clippy (the maintainer runs them); `cargo deny` (not
installed; no dependency changed); the CI scripts job with the new script test (not added, question 9).

## Amendment 2 (2026-10-28, with ADR 0144's registration, before the corrected run)

ADR 0138 (the mesh preview of a case) and ADR 0140 (store fields by default) were merged after this ADR's registered
runs. ADR 0144's baseline run of the `rcfd-studio` suite on main e11c656 (before any ADR 0144 change; release,
`--test-threads 1`, output to a file) failed three of this ADR's tests. Limits unchanged.

1. **G1 control** (`oneclick_g4.rs`): "a frame within 5.0 s" is read from `viewport.frame`, which since ADR 0138 also
   holds the preview frame of the open case (every face at 0): true at once (0.000 s). Set-up defect. Corrected: a
   frame of a run, `viewport.run.is_some() && viewport.frame.is_some()`; registered as part of ADR 0144's J14.
2. **G1b** (`oneclick_e2e_g4.rs`): (g) failed with 2 uploads (the preview's surface, then the run's) and shown indices
   beginning with 0. Not a set-up defect: ADR 0144 decision 1 switches the viewport to the run in the drain that
   selects it, so the preview is not drawn in between. Rerun unchanged with ADR 0144.
3. **G4 control** (`oneclick_g4.rs`): with the explanation off the viewport still contains the reason, now through ADR
   0140's empty-state text (`Run <id> failed before it stored its first snapshot: <reason>`). It does not involve the
   preview or the toggle and is not changed here: ADR 0144 question 8.

## Not in CI (by hand)

The real window: the toolbar, the strip and the "follow latest" box on DX12 and Vulkan, at high DPI; F5 while a text field
has the focus; the scripts on the maintainer's shells; the Linux runs of every process test here (none has run on Linux).
`scripts/test_studio_scripts.py` is not added to the CI scripts job (question 9).

## View-and-report checklist for the maintainer

1. Build and start: `powershell -ExecutionPolicy Bypass -File scripts\studio.ps1 <project folder>` (or `sh
   scripts/studio.sh <project folder>`). The toolbar under the menu bar should name the folder where `rcfd-runner` and
   `rcfd` were found. To see the error: rename `target\release\rcfd-runner.exe`, press "Search again": the toolbar should
   list the folders searched and the build command; rename it back and press "Search again".
2. Click a case in the Project panel (the lid cavity, a few hundred steps), then Run (or F5). At once the run is selected
   in the Runs panel, the Viewport is in front and the strip at the bottom shows the start; the first Run of a session
   may wait about 5 s while the runner starts (question 11; the strip counts the seconds). Then the strip shows the run's
   state, step, time and residuals, and the field appears with the first stored snapshot and moves on by itself
   ("following the latest" in the strip, the "follow latest" box ticked).
3. While it runs, drag the slider back: the box clears, the view stays, the slider keeps growing. Tick the box (or press
   `>|`): it jumps to the newest and follows again. Check that the Plots panel's vertical line follows the shown step.
4. Pause, Resume and Cancel in the strip on a longer run: the state in the strip changes within about a second.
5. A failing start: a case whose `mesh.file` does not exist. Run: the strip should show "failed" with the runner's reason
   in red, the viewport should say the run failed before its first stored snapshot, and the Log panel should be in front.
6. With unsaved edits in the case editor, Run should be greyed out with "save it first" on hover.
7. Close the Viewport tab (View menu), press Run: the Viewport should come back in front, above the Plots (amendment 1),
   with the Plots tab in front below it. Report how the space is shared.
8. Report anything slow, confusing or wrong, with the run id and what was done.

## Questions for the maintainer

1. F5 is the Run shortcut. Keep it, or another key (Ctrl+R, Ctrl+Enter)?
2. Run refuses a case with unsaved edits ("save it first"). Should it save and run instead, or run the file on disk with a
   warning?
3. The Start run dialog is unchanged: its Start uses the 48D route (no auto-select, no focus, no program search per start;
   its runner is the one found at start-up). Should its Start go through the one-click path too?
4. The search includes `target/release` of a development checkout (an ancestor of the Studio's folder or of the current
   folder holding `Cargo.toml`). Keep that rule in every build, or only in debug builds?
5. Every Run runs `rcfd --help` (about 0.1 s, on the control thread) to refuse a stale solver. Accept, or check once per
   program file (path and modification time)?
6. A closed Viewport is reopened by Run above the Plots' place (amendment 1), or, with Plots closed too, in the focused
   place. Accept, or reset the whole layout to the default one?
7. A failed start brings the Log panel to the front (when it does not share the Viewport's place). Accept?
8. The not-found message is shown in full in the toolbar at start-up (several lines). Accept, or a one-line summary with the
   list on hover?
9. Add `scripts/test_studio_scripts.py` to the CI scripts job (`ubuntu-latest`, where `sh` and PowerShell 7 exist; not run
   on Linux here)?
10. The scripts start the Studio in the foreground (the terminal shows its messages and waits for the window to close).
    Accept, or start it detached?
11. The first Run of a session waits about 5.3 s on this host before the run starts (G1, G1b; ADR 0133 R9 measured
    5.166 s): `ensure_runner` gives the PowerShell route 5 s to take the lock, and on this host it does not, so the direct
    start follows (which route started the runner is not recorded). The strip counts the wait. Investigate the
    PowerShell route on this host, or shorten its wait (an `rcfd-runner` change, ADR 0123 amendment 2)?

## Files

This ADR; `crates/rcfd-studio/src/{bins,quick_run,quick_run_ui}.rs` (new); the hunks of the Placement table in
`crates/rcfd-studio/src/{lib,studio,ui,main,viewport,viewport_ui}.rs` and `README.md`; tests
`crates/rcfd-studio/tests/{bins_g4,follow_g4,oneclick_g4,oneclick_e2e_g4}.rs` and `tests/g4/mod.rs` (new);
`scripts/{studio.ps1,studio.sh,test_studio_scripts.py}` (new).
