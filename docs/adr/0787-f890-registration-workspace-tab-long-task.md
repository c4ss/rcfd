# ADR 0787: F-890 registration: the long task when the workspace tab opens that fails the Studio's responsiveness test on the lead's machine (the hypotheses of the cause, the recorded run that tells them apart, the test that must fail without the fix, the consecutive runs)

Status: **REGISTERED 2026-11-23, written before any code of F-890 and before any run of its checks.** Before this text no
Studio source or test of F-890 was written and no Studio test was run. What was done: reading (below), searches of the
source tree, and two preparations that run no check: the release build of `rcfd-cli`, `rcfd-mcp`, `rcfd-runner` and the
example `f447_bench` into the scratch folder's own target folder (started, not yet finished when this was written), and
`npm ci` in `apps/studio-web`. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f890-studio-responsiveness-long-task.md`
([the file](briefs/f890-studio-responsiveness-long-task.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5, and
[docs/studio-guardrails.md](../studio-guardrails.md) (G9, G21, G13). Branch `f890-studio-responsiveness-long-task` from
main c4bd79f3, scratch `w49i-work/f890-studio-responsiveness-long-task-scratch`. Three agents beyond the limit of 4 at
once were allowed by the maintainer on 2026-11-23 ("Yes, three over") for F-890, F-891 and F-892 alone; that is the
lead's record in the brief, repeated here.

## What is asked

Roadmap item F-890 (deps F-832), the binding acceptance: "Every limit not stated here is registered in the ADR before
the run. The cause of the task of 400 to 440 ms in workspace.tab that apps/studio-web/tests/e2e/responsiveness.spec.ts:260
reports in each of its three passes (5 of 5 whole runs of npm test and 5 runs alone on 2026-11-23 after 10:20, ADR 0786
addendum 1, where the same tree passed at 09:39 with 53 ms) is found and shown by a recorded run; the test then passes a
registered number of consecutive runs on that machine. The limits of F-832 are not raised and the test is not skipped or
retried."

Clauses: **c1** the cause of the task is found and shown by a recorded run; **c2** the test then passes the registered
number of consecutive runs on this machine; **c3** the limits of F-832 (`LONG_TASK_LIMIT_MS` 100 ms,
`LONG_FRAME_LIMIT_MS` 150 ms, `apps/studio-web/src/shared/work.ts`) are not raised, and the test is not skipped or
retried.

## What was read

- The rules, the guardrails and the brief, in full.
- [ADR 0786](0786-f888-registration-quick-run-by-keyboard-focus.md): its head, the hypotheses and the checks (for the
  form of a Studio registration), and addendum 1 with the answers after it, in full.
- ADR 0801, [addendum 9](0801-studio-web-dependencies-and-layout.md) (F-832: the registered limits, what was built, the
  measurements and decision 7 on which long frames count).
- `apps/studio-web/tests/e2e/responsiveness.spec.ts`, `tests/support/studio.ts`, `tests/support/rcfd.ts`,
  `tests/support/longtasks.ts`, `tests/support/launch.ts`, `playwright.config.ts`, `package.json`, `vite.config.ts`,
  the head of `scripts/package-dir.mjs`, `src/shared/work.ts`.
- The source the phase `workspace.tab` drives: `src/renderer/workspace/RunWorkspace.tsx` and
  `src/renderer/field/FieldCanvas.tsx` in full; the lazy imports of `src/renderer` by search; of `src/main/index.ts` the
  test mode's window (lines 520 to 560) and the lines that name the app's scheme and the user data folder.
- The lead's logs `w49i-work/push-f888-2.log` and `w49i-work/push-f861d.log`: every line "F-832 pass" and the test's
  result lines.
- The records of the earlier work, read only: `w49i-work/f888-studio-quick-run-focus-scratch/resp-*.log` and
  `n1-*.log` (the lines "F-832 pass"), and the files `test-results-visual/f832/long-tasks.json` left in three
  worktrees (F-888's of 11:20, the lead's of 18:34, F-852's of 04:17, all 2026-11-23).

## Findings of the reading (before any run)

1. **Two different pictures fail the test today, and the item names the first.** (a) In the records of ADR 0786 (30
   passes between 10:20 and 11:20, machine 0.28 to 0.72 busy) each pass has exactly one `longtask` of 334 to 438 ms in
   `workspace.tab`, inside a long animation frame of 464 to 505 ms with a blocking duration of 286 to 389 ms. (b) In the
   lead's push runs of 17:07 and 18:38 (machine 0.86 to 1.00 busy) the tasks of `workspace.tab` are 60 to 92 ms in most
   passes, and the test fails on one held frame of 162 to 208 ms with a blocking duration of 16 to 44 ms; two passes of
   one run have tasks of 508 and 572 ms. In the same logs the test passed three times with the same tasks (60 to 71 ms)
   where the frame of about 200 ms had blocking duration 0.
2. **The long task carries no script.** In F-888's `long-tasks.json` the task of 406 to 428 ms has the attribution
   `unknown:window`, and the long animation frame around it lists no script at all (the recorder keeps the five longest
   of the scripts the frame names; the list is empty), with the frame's rendering starting at its very end (475 of
   476 ms). The frame's time is therefore not in a script entry point that the long-animation-frame entry names. The
   back end is "Drawn by WebGPU" in every record.
3. **What `workspace.tab` does.** The click on the Workspace tab mounts `RunWorkspace`, a lazy chunk
   (`ProjectHome.tsx` line 40), which brings `FieldViewport`, `FieldCanvas` and three's WebGPU renderer with it
   (addendum 9 of ADR 0801: about 1.6 MB of script, compiled at the first mount, and the GPU device created). The
   workspace selects the newest run on its own, the test then clicks the small run, and the phase lasts until that run
   is drawn. In it, `FieldCanvas` makes the renderer (`new WebGPURenderer`, `await renderer.init()`), and the first
   picture is drawn by a synchronous `renderer.render(scene, cam)` inside a React effect (`FieldCanvas.tsx` line 619),
   which is where three builds the node material's shader and asks the GPU for the pipeline the first time.
4. **The state is of the machine or of a folder, not of the tree.** ADR 0786 records that the same tree passed at 09:39
   with 53 ms and failed from 10:20 with three builds of the bench, a Studio built again and a fresh user data folder.
   The packaged copy shares its user data folder with an installed Studio (`tests/support/studio.ts`, the comment of
   `launchStudio`), so what Chromium keeps there between launches (the compiled-script cache, the GPU's shader and
   pipeline caches) is shared by every run of every worktree's tests on this machine, and a fresh folder has none of it.
5. **The test's own rule.** Up to three passes in fresh windows; the test passes on the first pass with nothing over a
   limit. A task of the Studio's own code is expected in every pass, which is what picture (a) shows.

Arithmetic done beforehand: the ranges of finding 1, taken from the lines "F-832 pass" of the logs named (30 passes of
F-888's records: tasks 334 to 438 ms; 9 passes and 3 single-pass runs of the lead's two logs). The task of picture (a)
is 3.3 to 4.4 times `LONG_TASK_LIMIT_MS`.

## Hypotheses of the cause and what tells them apart

| Id | Hypothesis | What it predicts in the record of the long task |
|---|---|---|
| H1 | Work of the Studio's own script on the main thread when the workspace tab mounts (React's render of the workspace, building the scene and its arrays, three's node builder making the shader's text) | At least half of the task's time is in JavaScript functions of the Studio's bundles (samples of the processor profile whose leaf or whose callers are functions of `dist/renderer/assets`, not in a native call of the GPU and not in the compiler); it is there whatever the state of the user data folder |
| H2 | A first compile of a shader or a pipeline that the main thread waits for (a synchronous creation of the shader module or the render pipeline on WebGPU, or compile and link on WebGL 2) | At least half of the task's time is inside one call into the GPU's interface (the profile's leaf is a native function such as the pipeline's creation, or the trace shows the thread waiting on the GPU process inside it); it differs between a fresh user data folder and one that holds the GPU's caches |
| H3 | Garbage collection, or the load of the machine | At least half of the task's time is in the trace's garbage-collection events; or the task's processor time on its thread is less than half of its wall-clock time (the thread was not scheduled), with no single callee holding it |
| H4 | Parsing and compiling the lazily loaded script of the workspace on the main thread (the compiled-script cache absent or refused) | At least half of the task's time is in the trace's script compile and module evaluation events of the workspace's chunks, with no cache consumed; it differs between a fresh user data folder and one launched before |
| H5 | None of these | The record fits no row above |

H1 to H3 are the three the brief names; H4 is added from findings 2 to 4.

The records, made on the unchanged source and test of main c4bd79f3, in the packaged Studio built from it, with the
engine and the bench of this work's own build:

- **D0, the unchanged test.** `npx playwright test tests/e2e/responsiveness.spec.ts --project=electron`, 3 times in a
  row; each pass's largest task and largest held frame of `workspace.tab` are recorded. No limit: it says which picture
  of finding 1 this machine shows now.
- **D1, the recorded run.** A scratch spec (kept in the scratch folder, copied into `tests/e2e` for the run and removed,
  not committed) that repeats the test's steps up to the small run drawn, with the same recorder, and around the phase
  `workspace.tab` records (i) a processor profile of the page with call stacks (the DevTools protocol's `Profiler`
  domain through Playwright's CDP session, sampling every 200 microseconds or the finest the session gives) and (ii) a
  trace of the renderer's main thread with its top-level tasks, script compile and evaluation, garbage collection and
  GPU events (Electron's `contentTracing` from the main process, or the protocol's `Tracing` domain). A scratch script
  reads both: for the longest `longtask` of the phase it finds the top-level task that holds it, lists the task's
  nested events by their own time, the profile's samples in its time range by leaf function and by call stack, and the
  task's thread time against its wall-clock time. 3 runs with the profile and the trace, and 3 with the recorder alone
  beside them (the profile slows the page; the task's length without it is the one that counts).
- **D2, the state of the user data folder,** on the same scratch spec without profile or trace: (a) a fresh
  `--user-data-dir`, 3 launches each with its own fresh folder; (b) one fresh folder launched 4 times in a row (the
  first is cold, the later ones hold whatever the first wrote); (c) the default folder, 3 launches. The largest task
  of `workspace.tab` in each launch is recorded.

Reading of the records. A hypothesis is **shown** when D1's three profiled runs agree on the row of the table above
(the same callee or kind of event holds at least half of the task in each), and D2 agrees with what that row says of
the folder. That the task found is the task of the item rests on its phase, its length (over `LONG_TASK_LIMIT_MS`
and within a factor of 2 of 400 ms without the profile) and its place in the frame (the frame's rendering at its end,
no script named), and the addendum says so in those words. If the machine today shows only picture (b) in the default
folder (no task over 100 ms in D0), the cause can still be shown if D2 (a) or (b) produces the task in a state the
record names; that the failures of 10:20 to 11:20 were this state then rests on the agreement of the task's length,
phase and attribution with F-888's `long-tasks.json`, and what put the machine into that state at 10:20 is reported as
found or as not found. If no state produces a task over the limit, C1 is not met and the result is partial. Any other
pattern is H5 and is recorded as not deciding; an amendment then registers the next record before it is made.

The held frame of picture (b) (162 to 208 ms, blocking 16 to 44 ms, at a machine 0.9 to 1.0 busy) is recorded in every
run below. It is in the same phase and D1's trace covers it; what the record shows of it is reported. It is not the
task the item names, but the test's consecutive runs (N1, N2) count every failure of the test, whichever picture.

## Design registered

The fix follows the hypothesis shown, in the Studio's source (`apps/studio-web/src/`), and the limits of F-832 stay as
they are:

- **If H2:** the first picture no longer waits on the main thread for the shader and the pipeline. The renderer is
  asked to compile the scene's materials asynchronously before the first synchronous draw (three's own asynchronous
  compile of a scene, which on WebGPU creates the pipeline without blocking), or the creation is moved out of the task
  by whatever the record shows the blocking call to be; the picture is shown when it is ready, as today (`onDrawn` after
  the GPU has finished), and every pixel is unchanged (G3: the same material, table and draw).
- **If H4:** the workspace's script is compiled outside the task that mounts it: the chunk is split so that no single
  compile exceeds the limit, or it is fetched and compiled ahead on a path the trace shows to be off the main thread,
  or the compiled-script cache is made to hold for the app's scheme; the choice is the one the trace supports, and the
  addendum gives the trace's numbers for it.
- **If H1:** the function the profile names is split, deferred or moved to the existing worker so that no task exceeds
  the limit.
- **If H3:** there is no cause in the Studio's code. The record is reported with the machine's load; the test is left
  as it is (its limits, its passes), and the result is partial unless the record shows a defect of the test itself,
  which is then corrected in `responsiveness.spec.ts` by an amendment written before the edit.
- Where the fix the record supports is none of the forms above, an amendment registers it before it is written.

No new dependency (G5), no change of a bridge, a golden or the engine. `responsiveness.spec.ts` is edited only if the
cause is in it.

**The test that fails without the fix (T1).** A new spec `apps/studio-web/tests/e2e/workspace-first-open.spec.ts`
(F-890, ADR 0787). It makes the state the record shows to produce the task itself and does not depend on what the
machine's shared folder holds: the packaged Studio launched with a fresh `--user-data-dir` (if D2 shows the folder's
state matters; else the launch of `responsiveness.spec.ts`), the bench project of the two small boxes (`tiny` and
`1e5`, as `responsiveness.spec.ts` makes it; skipped with the reason where the bench or `rcfd-mcp` is missing, as that
spec is), the recorder of `tests/support/longtasks.ts` with its probe of 130 ms, the click on the Workspace tab and the
small run drawn. It holds the phase `workspace.tab` to the two limits of F-832 imported from `src/shared/work.ts`, by
the rule of `responsiveness.spec.ts`: a `longtask` over `LONG_TASK_LIMIT_MS` or a held frame over
`LONG_FRAME_LIMIT_MS` fails a pass, up to three passes in fresh windows each with its own fresh folder, and the test
passes on the first clean pass. No retry of Playwright, no `skip` beyond the missing binaries, no longer limit. With
the fix removed (the committed mutation) it must fail with a task over `LONG_TASK_LIMIT_MS` in every pass. If the
record shows a state other than the folder's, an amendment registers the test's set-up before the test is written.

## Checks

Kinds as rule 14 names them; none of these is a validation against a measured case, and the acceptance claims none.
The limits of time are those of F-832, unchanged; this registration adds no limit on wall-clock time.

| Check | Clause | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| C1: the cause shown by D1 and D2 as read above (one of H1 to H4) | c1 | one hypothesis shown, by the reading registered above | acceptance of F-890 | H2 or H4 | not a number | code verification |
| C2: before the fix, the largest `longtask` of `workspace.tab` in the state the record names, in each of the 3 unprofiled runs of D1 (or of D2, where the state is the folder's) | c1 | over `LONG_TASK_LIMIT_MS` (100 ms) in 3 of 3 | ADR 0801 addendum 9 (F-832), the registered limits | 400 ms | 0.25 (the limit is a floor here: 400 ms against 100 ms) | code verification |
| T1: the new spec passes on the fixed tree | c1 | pass | acceptance of F-890 | pass | not a number | code verification |
| T1m: the committed mutation (the fix removed) makes T1 fail with a `longtask` over `LONG_TASK_LIMIT_MS` in `workspace.tab` in every pass | c1 | fail | the brief ("a committed mutation that a registered test must catch") | fail, about 400 ms | not a number | code verification |
| C3: after the fix, the largest `longtask` of `workspace.tab` in the state of C2, in 3 runs | c1 | at most `LONG_TASK_LIMIT_MS` (100 ms) in 3 of 3 | ADR 0801 addendum 9 (F-832), the registered limits | 70 ms | 1.4 | code verification |
| N1: consecutive runs of `responsiveness.spec.ts` alone on the fixed tree in which the test at line 260 passes | c2 | 10 of 10 | acceptance of F-890 ("a registered number of consecutive runs"); the number is registered here | 10 of 10 | 1 | code verification |
| N2: consecutive whole runs of `npm test` on the fixed tree in which the test at line 260 passes | c2 | 3 of 3 | the brief ("of whole runs of `npm test` after it"); the number is registered here | 3 of 3 | 1 | code verification |
| K1: `src/shared/work.ts` is unchanged, the diff of `responsiveness.spec.ts` (if any) removes no assertion and adds no `retries`, `skip`, `fixme`, `slow`, time-out or further pass, and `playwright.config.ts` keeps `retries: 0` | c3 | 0 such lines | acceptance of F-890 | 0 (no diff of the spec) | not a number | code verification |
| G1: `npm run typecheck` and the whole `npm test` (the runs of N2) | rule 7 | exit 0 | agent rules, rule 7 | exit 0 | not a number | unchanged default output |
| G2: `cargo test -p rcfd-cli --release --test docs_site` and `python scripts/process_report.py --check` | rule 7 | pass | agent rules, rule 7 | pass | not a number | unchanged default output |

N1 and N2 count the test's own verdict (its three passes are its registered rule of F-832, not a retry added here);
for each run the number of passes it needed and each pass's largest task and largest held frame of `workspace.tab` are
recorded with the machine's load, so a run that passed only on its second or third pass is visible. Ten runs alone:
picture (a) failed 30 passes of 30, so a fix that does not hold fails the first run; ten runs is what the machine's
picture (b) asks for, which failed 2 of the 5 runs of the test in the lead's two logs. Three whole runs: each is
about 11 minutes on a machine three other agents share. A failure of another test in a run of N2 is recorded with its
test and error and reported, not fixed; it does not count against N2, whose subject is line 260, but G1 then fails for
that run and the addendum says so. A test that fails on a file the operating system refuses under the load of the
other agents is reported, not retried until it passes. A run of N1 or N2 that fails at line 260 is a miss, recorded
with its numbers; the runs are not started again to obtain a clean series.

**Negative control and mutations (rule 16).** C2 is the negative control of the fix: the unfixed Studio in the state
named. Planned mutations, each applied to the worktree, built, run and removed:

- M1 (committed, `apps/studio-web/tests/e2e/workspace-first-open.spec.mutation.patch`): the fix removed from the
  Studio's source. T1 must fail (T1m).
- M2: a deliberate synchronous wait of 150 ms in the workspace's first mount (a busy loop in an effect of
  `RunWorkspace`). T1 and `responsiveness.spec.ts:260` must fail with a task over the limit in `workspace.tab`: the two
  tests still see a task of the Studio's own code after the fix.

Size (agent): done; 0; 4; 0; 150000

The heavy runs are the whole `npm test`, about 11 minutes each in the lead's log: the three of N2 and one allowed for
a repeat. They are run once each, one at a time. Processes: this work ends only processes its own runs started.

## Decision rule

As the brief gives it: **done** if the cause is shown by a recorded run (C1 and C2), the test then passes the
registered number of consecutive runs with the limits of F-832 unchanged (N1, N2, K1), and the mutation is caught (T1,
T1m). **Partial** otherwise, and in particular if the records give H3 or H5, if no state produces the task, if a run
of N1 or N2 fails at line 260, or if T1m does not fail. **Todo** if no record could be made.

## What is not covered

- What put the machine into the failing state at about 10:20 on 2026-11-23, if the record does not show it: it is
  reported as not found, and the cause shown is then the state, not the event that made it.
- CI: the test needs the bench and `rcfd-mcp` and is skipped where they are missing (the maintainer's answer (a) to Q1
  of ADR 0786: CI stays as it is). The new spec is skipped there in the same way.
- Linux, macOS and the WebGL 2 back end: the record is of this machine, Windows 11, WebGPU.
- The run of 1e6 cells (`RCFD_F832_SIZE=1e6`, a heavy record of F-832) is not repeated.
- The other Studio tests that failed in the lead's runs (`f837.spec.ts:198`, `project.spec.ts:245`,
  `run-control.spec.ts:264` and `:320`): F-891 and F-892 and the lead hold them; a failure seen here is reported by
  name.
- G9's limits (P2 to P10) are not measured again; the fix must not touch what they measure beyond the first picture's
  path, and the addendum says what it touched.

## Open questions for the maintainer

None at registration.

## Amendment 1 (2026-11-23): what D0, D1 and D2 gave, a hypothesis H6 that they point to, and the record D3 that must show it; written before D3 and before any code of the fix

Written after D0, D1 and D2 were made as registered and before anything else. No Studio source and no test of F-890
is written yet. Records are in the scratch folder (`d0-*.log`, `rec/`, `read_trace.py`, `tasks.py`, `gpuwait.py`).

**What the records gave** (machine 0.92 to 1.00 busy throughout: other agents' builds).

- D0: the unchanged test passed 3 of 3 (on its first, second and third pass). Largest task of `workspace.tab` 55 to
  70 ms; held frames of 157 to 184 ms (blocking 8 to 22 ms) failed three of its six passes. This is picture (b). The
  task of 400 ms did not show.
- D2: no launch had a task over 100 ms. Default folder, 3 launches: tasks none, 65 ms, none. A fresh folder each, 3
  launches: none, 69 ms, 89 ms. So the state of the user data folder does not produce the task; D2 (b) was not made,
  since (a) already says so. H4 and the folder's part of H2 are excluded as the state.
- D1 (trace through the DevTools protocol's `Tracing` domain, with the processor profile in it; three launches with
  it, tasks 79 to 83 ms): the phase holds about 320 ms of main-thread work in nine or ten tasks, none over 85 ms. The
  largest is an animation-frame callback of the plots' chunk (`PlotFrame`) in which three's node builder makes a
  shader (H1's kind of work, 65 to 68 ms, under the limit). **One task in each of the three traces is not work at
  all:** a React effect in which the main thread waits 19.4 to 32.8 ms inside `GpuChannel::CreateCommandBuffer` with 0.1
  to 0.6 ms of thread time, while the GPU process's main thread runs one flush of WebGPU commands (`WebGPU`,
  `CommandBufferStub::OnAsyncFlush`) for the whole of the wait. The same GPU thread has WebGPU flushes of 151, 86, 98
  and 192 ms in the phase (trace 1), which are the Studio's own: the devices and pipelines of the field canvas and of
  the plots.
- F-888's own log of the run that passed at 09:39 says the machine was 0.97 busy; its failing runs from 10:20 to 11:20
  say 0.28 to 0.72. Every record with the task of about 400 ms is of a machine less than 0.75 busy, and every record of
  a machine over 0.85 busy has tasks of 50 to 92 ms (but for two passes of the lead's log, 508 and 572 ms).

**H6 (a form of H2 that the registration did not word).** The first use of a canvas in the page makes Chromium create
the renderer's shared GPU context by a synchronous call to the GPU process (`GpuChannel::CreateCommandBuffer`), on the
main thread. The call is answered only when the GPU process's main thread is free, and at that moment the Studio has
just sent it WebGPU work (device creation, the first pipelines). The length of the task is therefore the length of
the GPU process's queue when the call is made, which depends on the order in which the workspace's parts arrive: on a
busy machine the call falls at the tail of the device creation (20 to 33 ms today); on a machine with free processors
the field's values arrive when the renderer is already initialised, the first draw sends the pipelines, and the call
falls behind them. That order is what the record of 10:20 to 11:20 would then be, and "the state of the machine after
10:20" would be that the other agents' builds had ended.

**D3, the record that must show it.** The scratch spec of D1 with one change of the environment only:
`RCFD_STUDIO_TEST_DELAY={"data":1000}` (the tests' own delay of F-832, `src/main/work/delay.ts`: every request of the
data door waits 1000 ms in the main process before the engine is asked). The field viewport asks for the store, the
surface and the field in turn, so the field's values reach the page 2 to 3 s after the canvas is mounted, when its
renderer is initialised whatever the machine's load: the order of a machine with free processors, made on a busy one.
1000 ms: the renderer's initialisation ended within 0.5 s of the mount in the three traces at a machine fully busy, a
factor of 4 or more inside the 2 s. Three launches with the trace and three without. The reading:

- **H6 is shown** if, in each of the three traced launches, the largest task of the phase holds one synchronous wait
  on the GPU process (an event of the main thread with wall-clock time at least half of the task and thread time under
  a tenth of its wall-clock time) during which the GPU process's main thread runs WebGPU work, and the three launches
  without the trace each have a `longtask` over `LONG_TASK_LIMIT_MS` (C2, as registered). The profile's leaf in that
  task names the call of the Studio's bundle that made the wait, and the addendum gives its source line.
- That this is the task of the item then rests on its phase, its kind (one task, no further one over 100 ms, the
  frame's rendering at its end) and its length within a factor of 2 of 400 ms; the addendum says so in those words,
  and says that the record of 10:20 itself was not made again (the load of that hour cannot be made here).
- If D3 has no task over the limit, or one of another composition, H6 is not shown; the result is then partial unless
  a further amendment registers another record first.

**The fix, if H6 is shown** (it replaces the line "If H2" of the design, which assumed the wait was inside a pipeline's
creation): the synchronous call is made once, at a moment the Studio chooses, before it sends any WebGPU work: a
module of the renderer (`apps/studio-web/src/renderer/`) that creates the shared GPU context ahead by the smallest use
of a canvas, called by every place that starts a WebGPU renderer (the field canvas and the plots) before it starts
it, and by the legend before its own canvas. Whichever comes first then pays the call while the GPU process has none
of the Studio's WebGPU work queued, and no later canvas use waits. Nothing drawn changes (G3): no pixel of the field,
the plots or the legend's bar is made differently. If D3's profile shows the wait is made by a call that this cannot
precede, an amendment registers the other fix before it is written.

**T1 as now registered** (it replaces the folder's state of the registration, which D2 excluded): the new spec
launches the packaged Studio with `RCFD_STUDIO_TEST_DELAY={"data":1000}`, the default user data folder as
`responsiveness.spec.ts`, and is otherwise as registered: the bench project, the recorder with its probe, the click on
the Workspace tab, the small run drawn, the two limits of F-832 held in `workspace.tab` by the rule of
`responsiveness.spec.ts` with up to three passes. C2, C3, T1m, M1 and M2 are read in this state. The checks, their
limits, N1, N2 and the decision rule are unchanged.

**Picture (b),** the held frame of 157 to 184 ms that fails single passes on the busy machine, is not the task of the
item. Whether the fix changes it is recorded by N1 and N2 and reported; no fix for it is registered here.

## Amendment 2 (2026-11-23): D3 did not show H6 as worded; what it gave, the record D4, the fix and the test as now registered; written before D4 and before any code of the fix

Written after D3 was made as amendment 1 registered it and before anything else. No Studio source and no test of
F-890 is written yet.

**D3 gave no task over the limit:** tasks 51 to 65 ms without the trace and 77 to 97 ms with it, three launches each,
machine 0.96 to 1.00 busy. By amendment 1's reading H6 is not shown by D3, and C2 is not met in D3's state. The traces
of D1 and D3 say why the delay did not make the wait, and correct amendment 1 in two places (scratch `ccb.py`):

1. The page makes **two** synchronous creations of a GPU command buffer, not one. (A) The first WebGPU call of the
   page (in a microtask of the renderer's initialisation; 1.2 ms in D1, 8.5 ms in D3), before any WebGPU work is
   queued. (B) The first use of a 2D canvas, which is the field legend's bar (`src/renderer/field/Legend.tsx`, the
   effect that calls `getContext('2d')` and `putImageData`): `GpuChannelHost::CreateViewCommandBuffer`, 19.4 to 32.8 ms
   in D1's three traces, the whole of it waiting while the GPU process's main thread finished a WebGPU flush of 151 to
   156 ms (the first device); 0.4 ms in D3, where the delay moved the legend to 5 s after the mount, when the GPU
   process was idle.
2. The legend's effect runs before the field's first draw, not after it. With the values late, as in D3, (B) falls
   where nothing is queued. So the order of amendment 1 ("the values arrive when the renderer is initialised") does not
   make the wait; what makes it is the legend arriving while the GPU process's main thread is inside one of the
   Studio's own WebGPU tasks (in D1: 151, 86, 45, 98, 192, 82, 47 and 66 ms, 0.9 s of the first second after the mount).
   The wait is the rest of the task the call falls into, plus what is queued before it.

**H6 as now worded.** The task of the item is the wait (B): the legend's first use of a 2D canvas makes a synchronous
call to the GPU process from a React effect on the main thread, and it lasts as long as the GPU process's main thread
is held by the WebGPU work the Studio has just sent. On the busy machine of today it falls at the end of the first
device's creation (19 to 33 ms). The order that made 400 ms on the machine with free processors of 10:20 to 11:20
cannot be made on this machine today (three other agents hold every processor, and amendment 1's delay moves the
legend the wrong way), and this work does not claim to have made that run again.

**D4, the record that must show it,** in the unchanged packaged Studio, with the scratch spec of D1 and one
instrument: the test holds the GPU process's main thread for a known time from the moment the legend's canvas enters
the document. In the page, before the Workspace tab is clicked, the test makes a WebGPU device of its own and
calibrates a compute shader (a chain of arithmetic statements with a fresh random constant, so no cache holds it),
doubling its length until the GPU process takes at least 300 ms to make its pipeline (from the call to the queue's
`onSubmittedWorkDone`); a `MutationObserver` then makes one such pipeline, with a new constant, in the microtask in
which `canvas[data-legend-bar]` is added. The instrument stands in for the Studio's own WebGPU tasks at the moment
the legend arrives; it changes no source and no environment of the Studio. Three launches with the trace and three
without; the time L the instrument's pipeline took is recorded for each. 300 ms: three times `LONG_TASK_LIMIT_MS`, and
inside the range the item names.

- **H6 is shown** if in each of the three launches without the trace `workspace.tab` has a `longtask` over
  `LONG_TASK_LIMIT_MS` (C2) whose length is within L plus or minus 100 ms, and in each of the three traced launches
  that task holds one event `GpuChannel::CreateCommandBuffer` of the main thread with wall-clock time at least half of
  the task and thread time under a tenth of its wall-clock time, during which the GPU process's main thread runs
  WebGPU work, and the profile's leaf in the task is the legend's effect (the addendum gives the position in the bundle
  and the source line it maps to).
- That the task of 10:20 to 11:20 was this wait then rests on: the wait measured in the Studio's own mount in D1 (3 of
  3 traces), its being the only event of the phase whose wall-clock time is not thread time, the 0.9 s of WebGPU tasks
  the Studio itself queues in that second, the agreement of the picture (one task, the frame's rendering at its end,
  no other task over 100 ms) with F-888's `long-tasks.json`, and the machine's load in every record (amendment 1). The
  addendum says so in those words and says that the run of 10:20 itself was not made again.
- If D4 has no such task, or one of another composition, H6 is not shown and the result is partial.
- **D5, without a limit:** if the machine's load falls under 0.75 while this work runs, D0 and one traced launch of D1
  are repeated once on the unchanged Studio (a scratch copy of the tree at c4bd79f3 if the fix is already written),
  and what they show is reported. If the load does not fall, D5 is reported as not made.

**The fix, if H6 is shown** (it replaces the fix of amendment 1, which would have paid the call lazily and so behind
whatever was queued): the renderer creates the 2D canvas's GPU context ahead, once, when the page has shown its first
view and is idle (a small module under `apps/studio-web/src/renderer/`, called from the page's entry; the smallest use
of a 2D canvas, one pixel, never added to the document). The synchronous call is then made when the Studio has sent
the GPU process no WebGPU work, and the legend's bar and every later 2D canvas find the context there. Nothing drawn
changes (G3): the legend's bar is written as before, and `field.spec.ts`'s test of its pixels is untouched. The
creation (A) is left as it is: it precedes all WebGPU work by construction. The wall-clock time of the call at the
start is read from a trace and reported, without a limit.

**T1 as now registered** (it replaces T1's set-up of amendment 1; the file's name and the rest are as registered):
`workspace-first-open.spec.ts` launches the packaged Studio as `responsiveness.spec.ts` does, with the bench project
and the recorder with its probe, and uses D4's instrument (calibrated in the test, at least 300 ms, started when the
legend's canvas enters the document). It asserts that the instrument worked (L at least 300 ms, and the pipeline was
started before the small run was drawn), and that `workspace.tab` has no `longtask` over `LONG_TASK_LIMIT_MS`. It
holds the task limit only: while the test itself keeps the GPU process busy, frames wait for the GPU by the test's
doing, so `LONG_FRAME_LIMIT_MS` is not held in this state (it stays held by `responsiveness.spec.ts`, unchanged). One
pass, since the state is made by the test and does not depend on the machine's load; no retry. C2 (before the fix)
and C3 (after it) are read in D4's state; T1m, M1 and M2 as registered, M2's deliberate wait being caught by T1 as a
task over the limit.

The checks' limits, N1, N2, K1 and the decision rule are unchanged. Picture (b) as in amendment 1.
