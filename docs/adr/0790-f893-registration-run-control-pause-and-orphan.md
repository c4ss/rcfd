# ADR 0790: F-893 registration: run-control.spec.ts:320, which misses the pause request in some whole runs, and the rcfd run solver a run-control test leaves running (the hypotheses of each cause, the recorded runs that tell them apart, the fix, the consecutive runs, the whole runs and the mutation)

Status: **REGISTERED 2026-11-24, written before any code of F-893 and before any run of its checks.** Before this text no
Studio source or test of F-893 was written and no Studio test was run. What was done: reading (below), searches of the
source tree and of the lead's logs, and two preparations that run no check, started while this text was written: the
release build of `rcfd-cli`, `rcfd-mcp` and `rcfd-runner` into the scratch folder's own target folder
(`CARGO_TARGET_DIR`), and `npm ci` in `apps/studio-web`. Results go in addenda at the end; nothing above them is changed
afterwards.

It answers the brief `docs/adr/briefs/f893-studio-run-control-pause-and-orphan.md`
([the file](briefs/f893-studio-run-control-pause-and-orphan.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5, and
[docs/studio-guardrails.md](../studio-guardrails.md). Branch `f893-studio-run-control-pause-and-orphan` from main
0a268fea, scratch `w49i-work/f893-studio-run-control-pause-and-orphan-scratch`. Model: Claude Opus 5.5 (the brief's
last line: issued on Opus after the agent first issued on Claude Fable 5.2 stopped at its usage limit before any work).

## What is asked

The acceptance of F-893 in `docs/roadmap.json`: "Every limit not stated here is registered in the ADR before the run.
The cause of the failure of apps/studio-web/tests/e2e/run-control.spec.ts:320 recorded in ADR 0789 addendum 1 and in
the lead's push run of F-861 on 2026-11-23 is found and shown by a test that fails without the fix; the test then passes
a registered number of consecutive runs on Windows, not skipped, retried or given a longer limit. The run-control test
that left an rcfd run solver running on 2026-11-24 (started with solver.max_iterations=1000000 in a temporary
rcfd-f807- folder, its parent gone, holding target/release/rcfd.exe) is shown by a recorded run that leaves one, and
after the fix a registered number of whole runs of npm test, among them runs with a run-control test made to fail or
time out, leave no rcfd run process."

Clauses: **c1** the cause of the :320 failure is found and shown by a test that fails without the fix; **c2** :320 then
passes the registered number of consecutive runs on Windows, not skipped, retried or given a longer limit, and (the
brief) what it checks, the pause requested from the Studio and acted on, is kept; **c3** the path that leaves the solver
is shown by a recorded run that leaves one; **c4** after the fix the registered whole runs of `npm test`, among them
runs with a run-control test made to fail or time out, leave no `rcfd run` process. The brief adds: the hypotheses of
each cause are registered before the change; the `rcfd` processes are counted with their command lines and parent ids
before and after each spec file; a committed mutation is caught by a registered test.

## What was read

- The brief, the rules (version 5) and the guardrails (G7 as amended 2026-11-17 in full; G4, G9, G18), in full.
- [ADR 0789](0789-f892-registration-serve-left-behind.md): the registration, addendum 1 (its D1 counting, the failures
  it reports, `run-control.spec.ts:320` among them), the lead's reading at the merge and the maintainer's answer.
- `apps/studio-web/tests/e2e/run-control.spec.ts`, `tests/support/rcfd.ts`, `studio.ts`, `runner.ts`, `leftovers.ts`,
  `global-teardown.ts` and `playwright.config.ts`, in full; `until` of `tests/support/fake-service.ts`.
- The Studio: `src/renderer/runs/RunsView.tsx` (`RunTableRow`, the row's note `.rc-pending`),
  `src/renderer/runs/control-model.ts` (`pendingByRun`, `requestWords`), `src/main/project/control.ts` (`settle`),
  the status line of `src/renderer/runs/RunControls.tsx`; a search of `src/` for a spawn of `rcfd run` (none: every
  run is the runner's).
- The engine, read only: `crates/rcfd-runner/src/runner.rs` (`serve`, `launch`, `solver_cmd`, `tick_active`,
  `finalize`, `set_terminal`, `shutdown`), `crates/rcfd-runner/src/client.rs` (`request`, `cancel`, `shutdown`),
  `crates/rcfd-cli/src/main.rs` lines 700 to 780 (the hold on `--pause-file`) and `crates/rcfd-api/src/runctl.rs`
  lines 283 to 420 (`drive` and its stop file).
- The lead's logs, read only, searched for `run-control`: `push-f888.log`, `push-f861b.log`, `push-f861c.log`,
  `push-f861d.log`, `push-f890-892.log`, `push-f852-451.log` under `w49i-work/`.

## Findings of the reading (before any run)

1. **The failure of :320 is at line 329, always the same way.** In `push-f888.log`, `push-f861d.log` (the F-861 push run
   the acceptance names) and `push-f890-892.log` the test fails after 5.2 to 5.5 s at line 329 with
   `expect(locator).toContainText(/Pause requested from the Studio|acted/)` on the row's `.rc-pending`, "Timeout:
   5000ms", "Error: element(s) not found". Line 328, `measured(page, 'project.pause_run')`, passed in each: it returns
   only once `.rv-table .rc-pending[data-request-seq="<the pause>"]` has appeared in the document. So the row's note was
   there, and 5 s later there was no note in the row at all; the words were never compared.
2. **The row's note is removed once the runner has acted.** `pendingByRun` keeps a request only while it is accepted and
   not `settled`, and `RunTableRow` renders `.rc-pending` only for such a request. `settle` marks a pause settled when the
   run list shows the run `paused`. The words "the runner has acted on it" that the test's alternative `acted` looks for
   are written by `requestWords` only for a settled request, which the row never shows. The status line of the selected
   run's panel (`.rc-status`, with `data-request-seq` and `data-settled`) keeps the latest request, settled or not.
3. **So :320 holds only if the test reaches line 329 before the runner has acted.** Between the note's first commit and
   line 329 the test polls the page every 20 ms (`until`), then reads the project state over IPC and evaluates in the page.
   On the engine's side the solver holds between two steps of the 1/1000000 run (milliseconds each), writes
   `pause.ack`, the runner sees it on its next poll (`poll_ms` 100) and writes `paused`, and the project server's feed
   brings the list to the Studio. Both sides are a few hundred ms at most; which one wins depends on the load of the
   machine. A whole run on a machine shared with other agents' builds is where the test's side is the slower one.
4. **The solver of the queue test is not in the teardown's list until after the P2 check.** In the third test (:264)
   the long run (`LONG`, never converging) is started at line 278, `measured` checks P2 at line 279, and only at line
   281 is its id pushed to `started`. `push-f852-451.log` (2026-11-24, the push before which the lead found the solver)
   fails at line 279 in each of its three runs of `npm test` ("project.queue_case: engine answer to the renderer, ms",
   23.9 ms against 20). The `afterAll` cancels only the runs in `started` and then calls `runner.cleanUp(started.at(-1))`,
   which asks the runner to shut down. The runner's shutdown leaves its solvers running by design ("the solver keeps
   running; the next runner adopts it", `runner.rs` `shutdown`; G7); `cleanUp` then kills the runner's pid if it is still
   alive and the solver of the last run only. The long run's solver is then alive with its parent gone, `--set` with the
   three values of `LONG`, its `--case-dir`, `--pause-file` and `--stop-file` under
   `%TEMP%/rcfd-f807-<random>/quick-start/runs/r-<time>/`, from the `target/release` the run's engine came from: the
   process the lead found. Nothing at the end of the run ends it: the global teardown of F-892 ends `rcfd serve` only.
5. **The pause and stop handling of `rcfd run` looks sound for a run it knows.** The hold loop of `main.rs` waits while
   the pause file exists and the stop file does not, and `drive` checks the stop file before every step; `rcfd-runner
   cancel` writes the stop file (`cancel`) itself. A cancelled solver therefore ends within a step whether it is paused
   or not, and whether a runner is alive or not, as long as the stop file is there when it looks.

## Hypotheses of the causes and what tells them apart

For :320 (the brief's three, and the one of findings 1 to 3):

- **P1, the pause file is written before the solver watches it.** It would show as a run that is never `paused`: the
  note stays pending with the words "Pause requested from the Studio …; waiting for the runner to hold it", line 329
  passes and line 330 fails. Told by: the state of the run and of the request when line 329 starts, and line 330.
- **P2, the status line of the pause is lost or not flushed in the feed.** `status.json` says `paused` and the run list
  does not: the request stays pending, line 329 passes and line 330 fails. Told by: `status.json` and the list's state
  when line 329 starts.
- **P3, a load on the machine past the 5 s wait.** The note would appear late or its words would change late. It
  cannot give "element(s) not found" once `measured` has seen the note, unless the note goes; told by the record of when
  the note appears and goes.
- **P4, the runner acts before the test looks** (findings 1 to 3): the note goes when the request is settled, before
  line 329 starts. Told by: in a failing repetition the note's removal is recorded before line 329 starts, with the
  request settled and the run `paused`; and a test that waits for the runner to act before line 329 fails at line 329
  with "element(s) not found" every time.

For the solver (the brief's three, with finding 4 as the first one's case):

- **S1, a test that fails or times out before its teardown with a run in flight.** S1a: the run is not in `started`
  (a failure between its start and its push, finding 4). S1b: the run is in `started` and its cancel does not end it
  (the `afterAll`'s cancel is swallowed by its `catch`, or the folder is removed before the solver sees the stop file).
  S1c: the `afterAll` does not reach the cancel (a time-out of the test or of the hook).
- **S2, a Studio or runner that quits without ending its `rcfd run` child.** The runner's shutdown does so by design
  (G7); the Studio starts no `rcfd run` of its own. Told by: the parent id of the process left is the runner's, and the
  runner was shut down by the test's teardown.
- **S3, a process tree ended without its children on Windows.** `cleanUp` kills the runner by its pid; the solver is
  not ended with it. Told by: the same record, the runner's pid ended by `killPid`.

S2 and S3 are the steps by which an S1 run outlives the teardown; they leave a solver only for a run that nothing
cancels. More than one may hold.

## Design registered

**The processes.** A scratch script lists, by `Get-CimInstance Win32_Process`, the processes named `rcfd.exe`,
`rcfd-mcp.exe`, `rcfd-runner.exe` and `rcfd-studio.exe` with pid, parent pid, start time, executable and command line
(as ADR 0789). "Mine" is a process whose executable is under this work's scratch folder or this worktree. "Left by a
run" is a process of mine in the list taken 15 s after the run that is not in the list taken before it. Only `rcfd.exe`
with `run` as its command counts against a limit; the others are counted and reported. "Before and after each spec
file" is taken in a whole run by a scratch Playwright reporter (given with `--reporter`, not committed) that lists the
processes when the first test of each file begins and when the run ends. Processes of mine that a diagnostic run leaves
are ended by this work, by pid, after they are counted. No process of another agent is counted or ended.

":320 alone" is a run of the spec file, `npx playwright test tests/e2e/run-control.spec.ts --project=electron`: the
file is serial and :320 pauses the long run that :264 starts, so :320 cannot be run without the three tests before it.

**The diagnostic runs, on the unchanged tree (c1, c3).** Each patch below is a scratch file, applied for its run and
removed after it.

- **B0, one whole run of `npm test`** with the counting reporter: the processes before and after each spec file, and
  whatever fails. Counted, no limit.
- **PD1, 10 runs of the spec file with a recording patch** that changes no check: in the page, the time the pause's
  note first appears in the row (and its words), the time it leaves the document, and the time the state listener first
  sees the request settled; in the test, at line 329, the time, the request's `settled`, the list's state of the run and
  `status.json`'s state. Counted: the repetitions that fail at line 329, and in each the order of the four times.
- **PD2, "the runner acts before the test looks"**, one run of the spec file with a patch that adds before line 329 a
  wait until the panel's status line has the pause request settled (`data-settled="true"`, at most 15 s, the limit of
  line 330 on the same condition). Expected by P4: :320 fails at line 329 with "element(s) not found". This is the test
  that fails without the fix (c1).
- **SD1, the queue test made to fail between the long run's start and its push** (a `throw` added after line 279, as
  `push-f852-451.log` failed there), one run of the spec file. Expected by S1a: one `rcfd run` of mine left with the
  three `LONG` values, its parent the runner the teardown ended (S2, S3).
- **SD2, :320 made to fail at line 329** (the PD2 patch): the long run paused and in `started`. Expected: 0 left (S1b
  not shown).
- **SD3, :320 made to time out** (a wait without end added after the pause is acted on, so the test runs into its
  120 s), one run of the spec file. Expected: 0 left (S1c not shown for a test time-out; the hook still runs).

c1 is met by PD2 failing at line 329 with the error of finding 1. c3 is met by a run of B0, SD1, SD2 or SD3 that leaves
an `rcfd run` process of mine, with its command line, parent id and run folder recorded.

**The fix, if the records give P4 and S1a.** Derived from findings 2 and 4.

- **:320 (`run-control.spec.ts`).** `watchFor` also notes the words of the element when it first appears (in the same
  observer, at the commit it already times). Line 329 becomes: the words the row's note had when it appeared match
  "Pause requested from the Studio"; then the panel's status line names the pause request (`data-request-seq`) and is
  settled (`data-settled="true"`, at most 15 s, the limit line 330 already gives the same condition); it says "Pause
  requested from the Studio" and "the runner has acted on it"; and the row no longer has a pending note. Lines 330 on
  are kept as they are. What :320 checks is kept and made explicit: the request shown in the run list within P2 with its
  words, and the runner acting on it (the panel's settle, the list's and `status.json`'s `paused`, the step held, then
  the resume). No check is removed and no limit is longer.
- **The teardown of the spec file (`run-control.spec.ts`, `tests/support/runner.ts`).** `Runner.endAll()`: every run of
  the project folder whose `status.json` is not terminal, whether a test noted it or not, is cancelled (`rcfd-runner
  cancel`; the stop file written directly where the command fails); each is waited for until it is terminal or its
  solver's pid is not alive, at most 30 s for all (the bound `cleanUp` already has); a solver still alive then is ended
  by its pid if it is still an `rcfd` process (`killIfRcfd`); then the runner is shut down and killed if it is still
  alive, as `cleanUp` does. The `afterAll` calls it in place of its loop and `cleanUp`. `cleanUp` stays as it is for the
  other spec files.
- **The harness, whatever a test does (`tests/support/leftovers.ts`, `runner.ts`).** The `Runner` notes its project
  folder in the run's registry (a line with `project: true`); the sweep of the global teardown gains a third kind: a
  process named `rcfd` whose command is `run` and whose command line names a registered project folder is ended and
  printed ("leftover rcfd run ended after the run"). Project lines take no part in the rules of F-892 (the state
  folder, the record, whether the run launched a Studio). Runners and project servers are still not ended (G7).
- Not changed: the Studio's source, the runner, `rcfd run`, any golden, protocol, default or dependency; no existing
  test other than `run-control.spec.ts` (owned by the brief). G7 is kept: what is ended is what a test started under its
  own temporary project, at the end of the test file or of the run.

If the records give another hypothesis, an amendment registers its fix before it is written.

**The tests registered (committed).**

- **T1, `apps/studio-web/tests/unit/run-leftovers.spec.ts`** (unit project). (a) The choice, as a pure function: an
  `rcfd.exe run … --case-dir <project>\cases … --pause-file <project>\runs\r-1\pause` is chosen by a registered
  project folder; one under an unregistered folder is not; neither is `rcfd-runner.exe` or `rcfd-mcp.exe` naming the
  folder, nor a process of another name; a project line does not make a `serve` naming the folder chosen and does not
  turn on the packaged rules. (b) With the real engine where it is built (skipped where it is not, as the engine's tests
  are): a runner on a temporary copy of `quick-start`, a run with the three `LONG` values submitted and running, the
  runner then killed by its pid so that the solver's parent is gone (S3); a sweep whose registry does not name the folder
  leaves the solver alive; a sweep whose registry names it ends it and prints it. (c) The same set-up with the runner
  left alive: `endAll()`, told no run id, leaves no solver alive and no runner.
- **T0, `run-control.spec.ts:320` as fixed**, which the runs below run.

**After the fix (c2, c4).**

- **A1, 10 consecutive runs of the spec file** in which :320 runs. Limit: :320 passes in each of the 10. A run in which
  :320 does not run because an earlier test of the file failed is recorded with that test and its error, and neither
  counts nor breaks the series; the series stops after 15 runs whatever they gave. A failure of :320 is a miss and is
  recorded with its number of passes before it. The processes are counted after each run (limit as A2).
- **A2, four whole runs of `npm test`**, one at a time, with the counting reporter, the processes listed before and 15 s
  after each: W1 and W2 the tree as committed; W3 with the SD1 patch (the queue test made to fail with the long run in
  flight and not in `started`); W4 with the SD3 patch (:320 made to time out with the long run paused). Limit: 0 `rcfd
  run` processes of mine left after each of the four. The lines the teardown prints are recorded for each: a process it
  had to end is counted as "ended by the teardown", so that the spec file's part and the harness's part can be told
  apart. :320 in W1 and W2 is recorded with A1.
- **A3, PD2 again on the fixed tree** (the wait for the settle is then part of T0): :320 passes.
- **K1:** the diff adds no `skip`, `fixme`, `slow`, `retries` or longer time-out to an existing test;
  `playwright.config.ts` keeps `retries: 0`, `workers: 1` and `timeout: 60_000`; the 15 s of T0's settle wait is the
  15 s of line 330, for the same condition.

A failure of another test in these runs is recorded with its test and error and reported, not fixed. No limit is
registered on wall-clock time. A test that fails on a file the operating system refuses under the load of the other
agents is reported, not retried until it passes.

## Checks

Kinds as rule 14 names them; none is a validation against a measured case, and the acceptance claims none.

| Check | Clause | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| C1: PD2 on the unchanged tree fails at line 329 with "element(s) not found" | c1 | fail, that line and error | acceptance of F-893 ("shown by a test that fails without the fix") | fails | not a number | code verification |
| PD1: 10 recorded runs of the spec file before the fix | c1 | counted, no limit | the brief, task 1 (the records that tell the hypotheses apart) | 0 to 2 failures at line 329, each with the note gone before line 329 | not a number | code verification |
| C3: a run of B0, SD1, SD2 or SD3 leaves an `rcfd run` of mine | c3 | at least 1, with command line, parent id and run folder | acceptance of F-893 ("shown by a recorded run that leaves one") | SD1: 1; SD2, SD3: 0 | not a number | code verification |
| B0, SD2, SD3: processes left before the fix | c3 | counted, no limit | the brief, task 1 ("count the rcfd processes … before and after each spec file") | 0 | not a number | code verification |
| T1: `run-leftovers.spec.ts` passes | c4 | pass | the brief, task 1 ("a committed mutation that a registered test must catch") | pass | not a number | code verification |
| T1m: the committed mutation makes T1 fail | c4 | fail | agent rules, rule 16 | fail | not a number | code verification |
| A1: :320 in 10 consecutive runs of the spec file | c2 | 10 of 10 | acceptance of F-893 ("a registered number of consecutive runs") | 10 of 10 | not a number | code verification |
| A3: PD2 on the fixed tree | c1, c2 | pass | acceptance of F-893 (the test that fails without the fix passes with it) | pass | not a number | code verification |
| A2: `rcfd run` of mine left after each of 4 whole runs, 2 with a run-control test made to fail or time out | c4 | 0 after each of 4 | acceptance of F-893 ("a registered number of whole runs … leave no rcfd run process") | 0 | not a number | code verification |
| K1: no test skipped, retried or given a longer limit | c2 | 0 such lines | acceptance of F-893; the brief, task 2 | 0 | not a number | code verification |
| G1: `npm run typecheck` and the whole `npm test` (W1, W2) | rule 7 | exit 0 | agent rules, rule 7 | exit 0 for the first; the whole run may fail on tests outside this item | not a number | unchanged default output |
| G2: `cargo test -p rcfd-cli --release --test docs_site` and `python scripts/process_report.py --check` | rule 7 | pass | agent rules, rule 7 | pass, `docs/adr/index.md` not current left to the lead | not a number | unchanged default output |

**Negative controls and mutations (rule 16).** PD2 and SD1 on the unchanged tree are the negative controls of A3 and of
W3. Planned mutations, each applied to the worktree, run and removed:

- M1 (committed, `apps/studio-web/tests/unit/run-leftovers.spec.mutation.patch`): the sweep no longer chooses an
  `rcfd run` by its project folder. T1 (a) and (b) must fail.
- M2: `endAll()` cancels only the runs it is told of (the old `started` list): T1 (c) must fail.
- M3: `endAll()` neither cancels nor ends a solver (only the runner's shutdown and kill, as `cleanUp` for an unknown
  run): T1 (c) must fail.
- M4: the third rule chooses by folder without asking for the name `rcfd`: a case of T1 (a) must fail.
- M5: line 329 of :320 restored on the fixed tree (the row's note looked up after the settle): one run of the spec file,
  :320 must fail at that line.

Size (agent): done; 0; 7; 0; 230000

The heavy runs: B0 and the four whole runs of A2 (13 to 16 minutes each in the lead's logs), and the two series PD1 and
A1 (10 to 15 runs of the spec file, about 2 minutes each). The single runs of the spec file (PD2, SD1 to SD3, A3, M5)
are not counted as heavy.

## Decision rule

As the brief gives it: **done** if each cause is shown by a recorded run before the fix (C1 for :320, C3 for the
solver), :320 then passes the registered consecutive runs with its checks and limits unchanged (A1, A3, K1), the
registered whole runs leave no `rcfd run` process (A2), and the mutation is caught (T1m). **Partial** otherwise, in
particular if PD1 shows a failure of :320 that P4 does not explain, if A1 has a failure of :320, or if a run of A2 leaves
an `rcfd run` process. **Todo** if no record could be made.

## What is not covered

- The P2 limit of `measured` (20 ms, G9) that :264 missed in `push-f852-451.log` and `push-f861d.log`: it is the
  failure that leaves the solver (S1a), not a subject of this item; its misses are reported, not changed.
- A run of `npm test` ended from outside (its console closed, its process killed): neither the `afterAll` nor the global
  teardown runs then.
- Runners (`rcfd-runner`) and project servers (`rcfd-mcp`) left by a run: counted and reported, not ended by the
  teardown (G7).
- Linux and macOS: the code is written for all three and T1 (a) runs anywhere; the runs of this work are on Windows 11.
- CI: T1 runs there by the lead's push; this work has no CI run.

## Open questions for the maintainer

None at registration.

## Amendment 1 (2026-11-24): the place of the time-out of SD3 and W4; written before the corrected run and before any code of the fix

Made so far, on the unchanged tree, each once: PD2 (which is also SD2, as registered: the same patch, one run), SD1 and
SD3. SD3's patch put the wait without end after line 331 (`status.json` says `paused`); in its run :320 failed first at
line 329 with "element(s) not found" (5.2 s), so the wait was never reached and the test did not time out. That run does
not decide SD3; it is kept as a record (a failure of :320 on the unchanged tree without the recording patch).

The set-up was defective: a time-out placed after line 329 depends on the race that line 329 loses. Corrected: the wait
without end is placed right after `await measured(page, 'project.pause_run');` (line 328), before line 329, the same
line on the unchanged and on the fixed tree. The pause has been accepted by the engine there, and the runner acts on it
within the 120 s the test then runs into. **SD3** is made once more with this patch on the unchanged tree, and **W4**
(A2) uses it on the fixed tree. Expected and limits as registered: SD3 counted (expected 0 left), W4 0 left. Nothing
else changes.

## Addendum 1: results (2026-11-24)

Written after the runs. Records are in the scratch folder under `records/` (one folder per run: the log, the process
lists before and 15 s after, the per-file lists of the counting reporter, what was left); the tables below are their
summary. All runs on Windows 11, the engine built from this branch into the scratch target folder, other agents running
on the machine at the same time. Commits: 70f42cd4 (this registration, alone), 1f45969a (amendment 1), c7f0b918 (the
fix and T1), 80699f8d (the committed mutation), then this addendum and the proposal file.

### The cause of the :320 failure: P4, the runner acts before the test looks

| Run, unchanged tree | :320 | Record |
|---|---|---|
| PD2 (the wait for the runner's acting before line 329) | failed at line 329, "element(s) not found", 5.3 s | the error of finding 1, the same line |
| SD3 as first registered (its wait after line 331, never reached) | failed at line 329, "element(s) not found", 5.2 s | a natural failure, no recording patch |
| PD1, 10 runs with the recording patch | ran in 9 (in pd1-02 :264 failed first, below), passed in 9 | at line 329: the note there with "Pause requested from the Studio …; waiting for the runner to hold it", 26 to 96 ms after its commit; the request not yet settled and the list's state `running` in 9 of 9, while `status.json` already said `paused` in 6 of 9 |
| B0, whole run | passed | |

C1 is met: PD2 fails at line 329 with the error the lead's three logs show. PD1 tells the hypotheses apart: the runner
has the solver held within about 50 ms of the engine's answer (`status.json` `paused` when line 329 starts, in 6 of 9),
so P1 is not shown (the pause is acted on every time and line 330 passed in every run); P2 is not shown as a loss (the
run list follows `status.json` within the 15 s of line 330 every time); P3 is not shown (the note appears 15 to 24 ms
after the answer). What decides line 329 is only whether the feed brings the settled list to the window before the test,
26 to 96 ms after the note's commit, looks at the row; when it does, the note is gone. The 1 natural failure in 11 runs of
the file without a wait before line 329 (SD3) fits that margin.

### The path that leaves the solver: S1a, then S2 and S3

| Run, unchanged tree | `rcfd run` of mine left | Record |
|---|---|---|
| SD1, the queue test made to fail after line 279 | 1 | pid 32472, parent 51536 no longer alive: the test's `rcfd-runner serve --project <tmp>\rcfd-f807-WsN0Ew\quick-start`, shut down and killed by `cleanUp`; command line `…\target\release\rcfd.exe run <tmp>\rcfd-f807-WsN0Ew\quick-start\runs\r-20261124T104814-0b60\case.snapshot --case-dir …\cases --set solver.max_iterations=1000000 --set solver.tolerance_u=1e-14 --set solver.tolerance_p=1e-14 … --stop-file …\cancel --pause-file …\pause …`; its run folder still there |
| PD2 = SD2, :320 failed at line 329, the run paused and in `started` | 0 | |
| SD3 (amendment 1), :320 timed out after 2.0 min, the run paused and in `started` | 0 | |
| PD1, 10 runs (pd1-02: :264 failed at its cancel's P2 check, line 312, after the long run was in `started`) | 0 in each | |
| B0, whole run: before and after each of 97 spec-file steps | 0 after every file, 0 after the run | |

C3 is met. SD1 leaves the process the lead found, with its command line, from the run's own target folder, its parent
gone: the run is never in `started` (S1a), the teardown asks the runner to shut down, which leaves its solvers by
design (S2), and kills the runner by its pid, which does not end its child (S3). S1b and S1c are not shown: a run in
`started` was ended by the old teardown whether paused, failed at, or timed out in (SD2, SD3, pd1-02).
`push-f852-451.log`, which failed at line 279 in each of its three runs, is that path.

### The fix

As registered (c7f0b918), test and harness only: `run-control.spec.ts` (the words of the note as it appeared, the panel's
settle, the row without its note; the `afterAll` calls `endAll()`), `tests/support/runner.ts` (`endAll()`, the project
noted in the registry), `tests/support/leftovers.ts` (the third rule and the `project` lines). No Studio source, runner,
`rcfd run`, golden, protocol, default or dependency changed; no Rust crate touched.

### Checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| C1 | fail at line 329, "element(s) not found" | PD2: so, 5.3 s | yes |
| PD1 | counted | 0 failures of :320 in 9 runs where it ran; the timings above | made |
| C3 | at least 1 left, recorded | SD1: 1 (above) | yes |
| B0, SD2, SD3 | counted | 0 each | made |
| T1 | pass | 6 of 6 (with `leftover-sweep.spec.ts`, 12 of 12) | yes |
| T1m | fail | M1: T1 (a) twice and (b) fail, 3 failed, 3 passed | yes |
| A1 | :320 passes 10 of 10 consecutive | 15 of 15 runs of the file, :320 passed in each, every test of the file passed in each, 0 processes left after each | yes |
| A3 | pass | the settle wait is part of T0, so A3 is every run of A1 (15 of 15); PD2's patch no longer applies, as expected | yes |
| A2 | 0 `rcfd run` of mine after each of 4 | W1 0, W2 0, W3 0, W4 0; 0 after every spec file of each; the teardown ended none in any | yes |
| K1 | 0 | no skip, fixme, slow, retry or longer time-out; config unchanged; T0's 15 s is line 330's | yes |
| G1 | exit 0 | `npm run typecheck` exit 0; the whole `npm test` exit 1 in every run, on tests outside this item (below) | no for the whole run |
| G2 | pass | see the end | see the end |

| Whole run of `npm test` | Tree | Passed, failed, did not run | `rcfd run` left | Other processes of mine left | Ended by the teardown |
|---|---|---|---|---|---|
| B0 | 1f45969a, unchanged | 844, 2, 28 | 0 | 0 | 0 |
| W1 | 80699f8d | 850, 2, 28 | 0 | 0 | 0 |
| W2 | the same | 850, 2, 28 | 0 | 0 | 0 |
| W3, the queue test made to fail with the long run not in `started` | the same | 844, 3, 33 | 0 | 0 | 0 |
| W4, :320 made to time out (2.0 min) with the long run paused | the same | 845, 3, 32 | 0 | 0 | 0 |

In W3 and W4 `endAll()` ended the long run through its cancel (it printed no solver ended by its pid), so the teardown's
third rule had nothing to end; that rule is shown by T1 (b) and its mutation, not by a process it had to end in a whole
run. In W1 and W2 :320 passed.

**Mutations.** M1 (committed): T1 (a) and (b) fail. M2: T1 (c) fails. M3: T1 (c) fails. M4: T1 (a), "never a runner …",
fails. M5: :320 fails at the restored line with "element(s) not found" (5.4 s). Each was applied to the worktree, run
and removed.

### Deviations and what was not run

- SD3 was made twice, the second time by amendment 1, before the corrected run: its first patch sat after the line that
  loses the race.
- PD2 and SD2 are one run, as registered (the same patch).
- A1 ran 15 runs, not stopping at the tenth pass: the script's count of passes did not match the reporter's line, so the
  series ran to its registered bound of 15. All 15 passed; the first 10 decide.
- A3 was not made as a separate run: the wait it adds is in T0 itself.
- Not run: CI, Linux, macOS; `npm test` beyond the five whole runs.

### Other tests that failed in these runs (reported, not fixed)

- `tests/unit/data-routes.spec.ts:94` and `tests/e2e/data.spec.ts:73`: in B0 and in W1 to W4 (as in ADR 0789).
- `tests/e2e/run-control.spec.ts:264`, the P2 check of `project.cancel_run` (33.6 ms against 20): pd1-02.
- The tests the scratch patches made to fail or time out, in their runs, as intended.

### Size

Size (agent), actual: done; 0; 7; 0; about 200000

Seven heavy runs as registered: B0, the four whole runs, and the two series (PD1, A1).

### Decision

By the registered rule: **done.** Each cause is shown by a recorded run before the fix (C1: PD2 fails at line 329 with
the logged error, and one natural failure; C3: SD1 leaves the lead's process); :320 then passed 15 of 15 consecutive
runs of its file with its checks kept and no limit longer (A1, A3, K1); the four whole runs, two with a run-control test
made to fail or time out, left no `rcfd run` process (A2); the committed mutation is caught (T1m). G1 is not met for the
whole `npm test`, on the two data tests outside this item.

### Open questions for the maintainer

None.

## For the lead: roadmap entries

Status proposed: **done**.

> ADR 0790, 2026-11-24, done: run-control.spec.ts:320 lost a race: the run list's note of a request goes once the runner has acted on it, and the runner held the solver within about 50 ms, so when the feed brought the settled list before the test looked at the row (26 to 96 ms after the note appeared) line 329 found no note; a test that lets the runner act first fails there every time. The solver was left by the queue test failing between the long run's start and its entry in the clean-up's list (as in the F-852 push run): the runner's shutdown leaves its solvers and its kill does not end its child. Fix, in the tests only: :320 reads the note's words as they appeared and the acting from the panel; the clean-up ends every run of the project; the global teardown ends an rcfd run under a test's project. After it :320 passed 15 of 15 runs of its file and 4 whole runs of npm test, two with a run-control test made to fail or time out, left no rcfd run; 5 mutations caught, one committed.

The same entry as `docs/roadmap/proposals/0790-F-893.json`. No follow-up is proposed.

### Checks at the end (G2)

- `cargo test -p rcfd-cli --release --test docs_site`: 2 passed, 0 failed.
- `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0790-F-893.json`: accepted (F-893 todo to done).
- `python scripts/process_report.py --check`: 1 failure, "generated file is not current: docs/adr/index.md", which rule 7
  leaves to the lead; nothing else.
- No Rust crate was touched: `cargo fmt` and `cargo clippy` do not apply.

## The lead's reading (2026-11-24), at the merge

Merged at e474897d and pushed at 812556b8 (the report read at 12:27 UTC, 10 minutes of reading, proposed done, written
done). Checked by the lead before the merge: the registration is the first commit and alone, amendment 1 comes before the
corrected run, and only test and harness files change; no golden, default result, hash table or dependency is changed,
so no reader was issued. In the push run `npm test` passed all 880 tests in its first run, the committed mutation was
caught, and no binary was left out. `data-routes.spec.ts:94` and `data.spec.ts:73`, which the report names as failing in
its runs, did not fail in the lead's push run.

Status: **done**, as proposed.
