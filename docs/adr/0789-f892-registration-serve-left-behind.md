# ADR 0789: F-892 registration: the path that leaves an rcfd serve process running after npm test on Windows (the hypotheses, the recorded runs that tell them apart, the fix in the Studio's quit and in the harness, the whole runs after it, and the mutation)

Status: **REGISTERED 2026-11-23, written before any code of F-892 and before any run of its checks.** Before this text no
Studio source or test of F-892 was written and no Studio test was run. What was done: reading (below), searches of the
source tree, and two preparations that run no check: the release build of `rcfd-cli`, `rcfd-mcp` and `rcfd-runner` into
the scratch folder's own target folder, and `npm ci` in `apps/studio-web`. Results go in addenda at the end; nothing
above them is changed afterwards.

It answers the brief `docs/adr/briefs/f892-studio-serve-left-behind.md`
([the file](briefs/f892-studio-serve-left-behind.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5, and
[docs/studio-guardrails.md](../studio-guardrails.md). Branch `f892-studio-serve-left-behind` from main c4bd79f3, scratch
`w49i-work/f892-studio-serve-left-behind-scratch`. This agent runs beyond the limit of 4 at once on the maintainer's
answer of 2026-11-23 ("Yes, three over"), for this item alone; that is the lead's record in the brief, repeated here.

## What is asked

The acceptance of F-892 in `docs/roadmap.json`: "Every limit not stated here is registered in the ADR before the run.
The end-to-end test or the path of the harness that left the rcfd serve processes found on 2026-11-23 (started with
--ui and a journal folder under a temporary rcfd-studio-state folder, from a target/release folder; none was left by
any run of ADR 0786, a time-out of a test is the candidate not reproduced there) is shown by a recorded run that leaves
one, and after the fix a registered number of whole runs of npm test, among them runs with a test made to time out,
leave none."

Its clauses: **c1** the test or the path is shown by a recorded run that leaves a process; **c2** after the fix a
registered number of whole runs of `npm test` leave none; **c3** among them are runs with a test made to time out. The
brief adds: the hypotheses are registered before the change; the processes are counted with their command lines and
parent ids before and after each spec file; no test is skipped or retried to reach c2; a committed mutation is caught
by a registered test.

## What was read

- The brief, the rules (version 5) and the guardrails, in full. G7 as amended 2026-11-17 is the rule of the subject:
  "The Studio's own `rcfd serve` closes with it", kept only while one of its sessions has a solve going; "A crash, a
  kill or a reload still stops nothing."
- [ADR 0786](0786-f888-registration-quick-run-by-keyboard-focus.md): finding 7, "The processes" of addendum 1 and the
  lead's reading at the merge.
- `apps/studio-web/tests/support/studio.ts`, `rcfd.ts`, `launch.ts` and `apps/studio-web/playwright.config.ts`, in full.
- The Studio's main process: `apps/studio-web/src/main/service/spawn.ts`, `client.ts` and `config.ts` in full, and the
  quit of `apps/studio-web/src/main/index.ts` (the `before-quit` handler and what it calls).
- `apps/studio-web/tests/e2e/project.spec.ts` (the head, the `afterAll`, and the tests from line 245 on), the test of
  `f837.spec.ts` at line 393, `service.spec.ts` lines 140 to 176, and the list of the files that launch Electron
  without `launchStudio` (`f837.spec.ts:393`, `primitives.spec.ts`, `shell.spec.ts`, `theme.spec.ts`).
- The lead's logs, read only: `w49i-work/push-f888-2.log` and `w49i-work/push-f861d.log`.
- Playwright's own code for a launched process, in `node_modules/playwright-core/lib/coreBundle.js` of the main
  checkout (`launchProcess`, `killProcessTree`): what the test runner in use does, read as the behaviour of a tool, not
  as a source of an algorithm.
- `crates/rcfd-cli` was searched for a watch of the parent process in `rcfd serve`: none was found.

## Findings of the reading (before any run)

1. **The two logs agree on one event.** In `push-f888-2.log` (run 1 of `npm test`) and in `push-f861d.log` (run 1),
   `project.spec.ts:245` fails with "Test timeout of 60000ms exceeded" after 1.0 minute, and the next line of the
   reporter is "Worker teardown timeout of 60000ms exceeded." The later runs of `npm test` in the same push runs have no
   time-out and no such line. The lead found one process per push run. A push run with one teardown time-out and one
   process left is what both logs show.
2. **The test that timed out quits a Studio just after it opened a project.** `project.spec.ts:245` launches a second
   Studio with `RCFD_STUDIO_MCP` (the `rcfd` beside `rcfd-mcp` in `target/release`, started with the first project;
   the state folder is `launchStudio`'s `<tmp>/rcfd-studio-state-<random>`), opens `quick-start` from the recent list,
   waits for the heading and calls `studio.app.close()` at once. That is the command line of the processes found:
   `--ui --journal-dir <tmp>/rcfd-studio-state-…/journals`, from a `target/release` folder. `f837.spec.ts:393` has the
   same shape (a project on the command line, the heading, a screenshot, `app.close()`), with a state folder of its own.
3. **The Studio's quit has two windows in which it leaves the service it has just started.** `service.start()` runs
   when the project state first has a project, about when the heading shows. `resolveTarget` (client.ts) awaits
   `startService` (spawn.ts), which spawns `rcfd serve` detached and polls the log file every 20 ms for the address.
   - **W1, spawned and not yet listening.** `shutdown()` sets `stopped` and reads `this.st.pid`, which is still null,
     so it stops nothing. The line that would end the child (`if (this.stopped) { s.child.kill(); … }`) runs only when
     `startService` returns; the quit goes on to `app.quit()` and the main process can end first. The child is
     detached and stays. A shutdown that comes during the awaits before the spawn (`readRecord`, `mkdir`, `stat`) has
     the same end: nothing checks `stopped` before the spawn.
   - **W2, listening and not yet connected.** The pid is set and the record is written, the two connections,
     `service.info` and `findHead` are under way, `this.link` is null. `shutdown()` then starts with `keep = true` and
     changes it only `if (link && this.st.status === 'connected')`: the service is left running ("cannot tell").
   Both windows are short on an idle machine and grow with its load; the push runs ran beside other agents' builds.
4. **A service that outlives the Studio's quit makes `close()` wait for ever on Windows.** `quitLeavingService` in
   `studio.ts` already says why: the service inherits the pipes Playwright gave the Studio as standard streams, and
   Playwright's `close()` waits for the `close` event of the child process, which needs those pipes closed
   (`launchProcess`: `waitForCleanup` is fulfilled from `spawnedProcess.once("close")`). So the test waits in
   `app.close()` until its time-out; the `afterAll` of `project.spec.ts` calls `studio?.app.close()` on the same
   application and waits again, until the worker's teardown time-out.
5. **Playwright's forced end cannot reach the service then.** A second `close()` and the worker's exit both end in
   `killProcessTree`: `taskkill /pid <Studio's pid> /T /F`. The Studio's process has already ended, so there is no
   process of that id whose children could be walked, and the detached `rcfd serve` stays. Nothing else knows its pid:
   in W1 not even `service.json` has it.
6. **What ADR 0786 saw fits.** A Studio ended by its pid, or by the tree, while it is alive takes the service with it
   (the tree is walked from a living parent); a failure of a test with a Studio that has been open for a while quits a
   Studio whose service is connected, which the quit stops. None of ADR 0786's runs had a time-out in `close()`.

## Hypotheses of the cause and what tells them apart

- **H1, the Studio's quit leaves the service it is starting** (findings 3 to 5; W1 or W2). The brief's "an Electron
  app closed without ending its engine", with its consequence "a test that times out" and its last step "a process
  tree ended without its children". Told by: D2 and D3 leave a process whose Studio has ended, `close()` does not
  settle, and in the state folder `service.json` is absent (W1) or present with the pid of the process left (W2).
- **H2, a test that times out or fails before its teardown**, with a Studio whose service is connected, leaves the
  service by the harness alone. Told by: D4 leaves a process.
- **H3, a Studio ended by its pid or by its tree leaves its service** (`killPid` in `run-control.spec.ts`,
  `runs.spec.ts`, `first-path.spec.ts`, `runner-lifetime.spec.ts`; Playwright's `taskkill` on a living Studio). Told
  by: D1 shows a process left after one of those files, or D4's tree kill leaves one.
- **H4, the quit of a Studio whose service is connected leaves it**, because `session.list` does not answer in 1500 ms
  or the whole quit passes its 3 s under load. It would look like H1 with `service.json` present. It is not forced by
  any run here; if D2 and D3 show nothing and D1 shows a process left with `service.json` present, the records give H4
  and the registered fix does not cover it.

More than one may hold. The cause of the processes found is the hypothesis whose record has the command line of
finding 2 and the time-outs of finding 1.

## Design registered

**The processes.** A scratch script lists, by `Get-CimInstance Win32_Process`, the processes named `rcfd.exe`,
`rcfd-mcp.exe`, `rcfd-runner.exe` and `rcfd-studio.exe` with pid, parent pid, start time, executable and command line.
"Mine" is a process whose executable is under this work's scratch folder or this worktree (the packaged copy,
`apps/studio-web/out/packaged`, is in the worktree). "Left by a run" is a process of mine in the list taken 15 s after
the run that is not in the list taken before it. Only `rcfd.exe` with `serve` in its command line counts against a
limit; the other three names are counted and reported. Processes of mine that a run leaves are ended by this work,
by pid, after they are counted. No process of another agent is counted or ended.

**The diagnostic runs, on the unchanged tree (c1).**

- **D1, each spec file alone.** `npm run build` and `npm run package:dir` once, then `npx playwright test <file>` for
  each file of `tests/e2e` in the reporter's order, and the `unit` project as one step; the list of processes before
  and after each. No limit: counted.
- **D2, the two windows with a stand-in engine.** A scratch program (Rust, the standard library only, compiled with
  `rustc` into the scratch folder as `rcfd.exe`; not committed) takes the arguments of `rcfd serve`, waits a number of
  milliseconds given in an environment variable, binds a loopback port, prints "rcfd service listening on <address>"
  and then accepts connections without ever answering. A scratch spec launches the packaged Studio by `launchStudio`
  with `RCFD_STUDIO_RCFD` naming it and calls `app.close()`: (a) with a wait of 10 s, as soon as the page reports the
  state `starting` (W1); (b) with no wait, as soon as the page reports a pid and the state is not `connected` (W2).
  Recorded for each of 3 repetitions of (a) and of (b): whether the Studio's process ended, whether `close()` settled
  within 20 s, whether the stand-in is alive 15 s later, and whether `service.json` is there. Then (c): the same as
  (a) with `await app.close()` and nothing else, so that the test runs into its time-out and the worker into its
  teardown, as in finding 1; the reporter's lines and the processes left are recorded. Expected by H1: the Studio ends,
  `close()` does not settle, the stand-in stays, in each repetition; (c) shows "Test timeout" and leaves 1.
- **D3, the first window with the real engine.** A scratch spec, 20 repetitions: the packaged Studio launched with
  `RCFD_STUDIO_RCFD` naming the real `rcfd.exe` of the scratch target folder (the service then starts at launch) and a
  fresh state folder, and `close()` called as soon as the first window exists, not awaited beyond 20 s. Recorded per
  repetition as in D2. No limit: the number of repetitions that leave a process is the record. This is the path of
  `project.spec.ts:245` with the wait for the project taken out; D1 has the test itself.
- **D4, a time-out with the service connected.** `project.spec.ts` run alone with line 245 made to wait for ever
  after the heading of the second Studio and after the page reports the service `connected` (a scratch patch), so
  that the test times out with a Studio open. Counted. Expected: 0 left (H2 and H3 not shown).

c1 is met by a run of D1, D2 (c) or D3 that leaves an `rcfd serve` process of mine, with its command line, its parent
id and the state of its folder recorded. The acceptance says "the end-to-end test or the path of the harness"; the
path found by the reading is in the Studio's quit and is reached by the tests of finding 2. If only D2 leaves a
process, the addendum says in those words that the path is shown with a stand-in engine and the test itself was not
caught leaving one.

**The fix, if the records give H1.** Derived from first principles: a process that the Studio spawned and that has not
yet answered it can hold nothing of anyone's (no session was made through it), so G7 does not ask for it to be kept;
and the three steps "decide to spawn, spawn, remember the child" must have no await between them, so that a quit sees
either no child or a child it can end.

- `spawn.ts`: the part of `startService` that spawns and waits for the address becomes a function that takes the
  command; `startService` builds the arguments as now and calls it. It gains two hooks: one asked just before the
  spawn, synchronously (a quit under way: no spawn, the start rejects), and one called with the child synchronously
  after the spawn.
- `client.ts`: the client remembers the child from that hook on. `shutdown()` ends a child that has not yet reported
  its address (W1), and a child of this start to which the client has never been connected (W2), in both modes of
  `keepWithSessions`. Everything else of `shutdown()` stays: a connected service is asked for its sessions as now; a
  service found running (adopted) and a service that was connected and is not any more are kept when it cannot be told
  (G7, G18). `ServiceClientOptions` gains the start function as an option, as `backoff` is one, for the unit tests.
- No change to `rcfd serve`, to any protocol, golden, default or setting; no new dependency. G4, G5 and G10 are not
  touched; G7 is kept as amended.

**The harness, whatever the tests do.** `playwright.config.ts` gains a global teardown and names a registry file for
the run in the environment; `launchStudio` writes to it the state folder (or the user data folder) before each launch
and the Studio's pid after it. The teardown, in a new module of `tests/support`, ends what the run left, and only
that: (1) processes named `rcfd-studio` whose executable is the packaged one of this checkout (`out/packaged.json`)
or whose pid the registry has and is still that executable; then (2) processes named `rcfd` with `serve` in the command
line whose command line names a state folder of the registry, or whose pid is in the `service.json` of such a folder,
or whose executable is in the packaged engine folder of this checkout. Each one ended is printed with its pid, its
command line and the spec file that launched its Studio where the registry has it. A process whose name is not `rcfd`
or `rcfd-studio` is never ended (a pid can be reused). The teardown prints and does not fail the run: taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation); a run that fails for a process the teardown
has already ended would hold merges for nothing more than the line it prints. Runners (`rcfd-runner`) and project
servers (`rcfd-mcp`) are not ended by it: G7 gives the runner a life of its own, and the acceptance names `rcfd serve`.

If the records give another hypothesis than H1, an amendment registers its fix before it is written.

**The tests registered (committed).**

- **T1, `apps/studio-web/tests/unit/service-quit.spec.ts`** (unit project, no Electron, no engine): the real
  `ServiceClient` and the real spawning function of `spawn.ts`, with the start option naming a stand-in command
  (`process.execPath -e <script>`, a Node process that prints the listening line after a wait, or at once and then
  never answers). (a) `shutdown()` while the stand-in has not printed its address: the stand-in's process is gone
  within 5 s. (b) `shutdown()` once the state has a pid and before `connected`: gone within 5 s, and `service.json`
  removed. (c) `shutdown()` in the same turn as `start()`: no stand-in is alive 2 s later. (d) the rule kept: a
  service found running (adopted) that does not answer is left running by `shutdown()` (`leftRunning: true`).
- **T2, `apps/studio-web/tests/unit/leftover-sweep.spec.ts`** (unit project): (a) the choice of what to end, as a pure
  function over a list of processes and a registry: the three rules of (2), the rule of (1), and that a process of
  another name, of another checkout's packaged folder or of a state folder not in the registry is not chosen; (b) with
  the real `rcfd` where it is built (skipped where it is not, as the engine's tests are): `rcfd serve` started
  detached with a journal folder under a registered state folder and no `service.json`, the sweep ends it and names
  it; a second one whose state folder is not registered is left alive (and ended by the test itself).

**After the fix (c2, c3).**

- **A1:** D2 (a), (b), (c) and D3 again on the fixed tree. Limit: 0 `rcfd serve` (stand-in or real) left in every
  repetition, and `close()` settles in every repetition.
- **A2: four whole runs of `npm test`,** one at a time, the processes listed before and 15 s after each:
  A2-1 and A2-2 the tree as committed; A2-3 with `project.spec.ts:245` made to time out as in D4 (a Studio open, its
  service connected); A2-4 with the test `service.spec.ts:145` made to wait for ever after its first `connected` (a
  Studio held in a local variable that no `afterAll` closes, its service started at launch from the target folder).
  The two patches are scratch files, applied for the run and removed after it. Limit: 0 `rcfd serve` of mine left
  after each of the four. The lines the teardown prints are recorded for each run: a process it had to end is counted
  in the addendum as "ended by the teardown", so that the Studio's part and the harness's part can be told apart.
- **K1:** the diff of the work adds no `skip`, `fixme`, `slow`, `retries` or time-out to an existing test and changes
  no existing spec file; `playwright.config.ts` keeps `retries: 0`, `workers: 1` and `timeout: 60_000`.

A failure of another test in these runs is recorded with its test and error and reported, not fixed; G1 below then
fails for that run and the addendum says so. No limit is registered on wall-clock time. A test that fails on a file
the operating system refuses under the load of the other agents is reported, not retried until it passes.

## Checks

Kinds as rule 14 names them; none of these is a validation against a measured case, and the acceptance claims none.

| Check | Clause | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| C1: a recorded run of D1, D2 (c) or D3 on the unchanged tree leaves an `rcfd serve` process of mine | c1 | at least 1 left, with command line, parent id and state folder recorded | acceptance of F-892 ("shown by a recorded run that leaves one") | D2 (c): 1; D3: more than half of 20 | not a number | code verification |
| D1, D2 (a) (b), D3, D4: processes left before the fix | c1 | counted, no limit | the brief, task 1 ("count the rcfd processes … before and after each spec file") | D1: 0; D2: 3 of 3 twice; D4: 0 | not a number | code verification |
| T1: `service-quit.spec.ts` passes on the fixed tree | c2 | pass | the brief, task 1 ("a committed mutation that a registered test must catch") | pass | not a number | code verification |
| T1m: the committed mutation makes T1 (a) fail | c2 | fail | agent rules, rule 16 | fail | not a number | code verification |
| T2: `leftover-sweep.spec.ts` passes on the fixed tree | c2 | pass | the brief, task 2 ("a run leaves no rcfd serve process whatever its tests do") | pass | not a number | code verification |
| A1: D2 and D3 on the fixed tree | c2 | 0 left and `close()` settled, in each repetition | acceptance of F-892 ("leave none") | 0 | not a number | code verification |
| A2: `rcfd serve` of mine left after each of 4 whole runs of `npm test`, 2 of them with a test made to time out | c2, c3 | 0 after each of 4 | acceptance of F-892 ("a registered number of whole runs of npm test, among them runs with a test made to time out, leave none") | 0 | not a number | code verification |
| K1: no existing test skipped, retried or given a longer limit | c2 | 0 such lines | the brief, task 2 ("No test is skipped or retried to reach that") | 0 | not a number | code verification |
| G1: `npm run typecheck`, the unit tests `service-client.spec.ts`, and the whole `npm test` (A2-1 and A2-2) | rule 7 | exit 0 | agent rules, rule 7 | exit 0 for the first two; the whole run may fail on the tests of F-890 and F-891 | not a number | unchanged default output |
| G2: `cargo test -p rcfd-cli --release --test docs_site` and `python scripts/process_report.py --check` | rule 7 | pass | agent rules, rule 7 | pass | not a number | unchanged default output |

**Negative control and mutations (rule 16).** D2 and D3 on the unchanged tree are the negative control of A1. Planned
mutations, each applied to a scratch copy or to the worktree, run and removed:

- M1 (committed, `apps/studio-web/tests/unit/service-quit.spec.mutation.patch`): in `client.ts`, `shutdown()` no
  longer ends a child that has not reported its address. T1 (a) must fail.
- M2: `shutdown()` no longer ends its own child to which it was never connected. T1 (b) must fail.
- M3: the hook before the spawn always answers "go on". T1 (c) must fail.
- M4: `shutdown()` ends a service found running that does not answer. T1 (d) must fail (G7 kept).
- M5: the sweep no longer matches the state folder in the command line. T2 (b) must fail, and a case of T2 (a).
- M6: the sweep chooses by state folder without asking for the name `rcfd`. A case of T2 (a) must fail.

Size (agent): done; 0; 5; 0; 260000

The heavy runs are D1 (every spec file once, about one whole run) and the four whole runs of A2, about 13 to 16
minutes each in the lead's logs. Each is made once, one at a time.

## Decision rule

As the brief gives it: **done** if the cause is shown by a recorded run that leaves a process (C1), the registered
whole runs after the fix leave none (A1 and A2, with K1), and the mutation is caught (T1m). **Partial** otherwise, and
in particular if C1 is met only by the stand-in engine and a later record shows a second path that the fix does not
cover, if a run of A2 leaves a process, or if the records give H4. **Todo** if no record could be made.

## What is not covered

- A run of `npm test` that is itself ended from outside (its console closed, its process killed): the global teardown
  does not run then. The fix in the Studio still holds for W1 and W2.
- H4 is not forced; the quit of a Studio that was connected to its service and cannot ask it keeps the service, as G7
  and G18 ask. Such a service, if a test run leaves one, is ended by the teardown and printed.
- `rcfd-runner` and `rcfd-mcp` processes: counted and reported, not ended by the teardown.
- The hang of Playwright's `close()` on a service that is meant to outlive the Studio (`quitLeavingService` covers the
  tests that mean it); the tests that fail in the lead's runs for other reasons (F-890, F-891, `run-control.spec.ts`).
- Linux and macOS: the harness code is written for all three and T2 (a) runs anywhere; the runs of this work are on
  Windows 11.
- CI: the unit tests T1 and T2 (a) run there by the lead's push; this work has no CI run.

## Open questions for the maintainer

None at registration.

## Addendum 1: results (2026-11-23)

Written after the runs. Records are in the scratch folder under `records/` (`diag-before`, `d1`, `diag-after`, `a2`,
`mutation-*.log`); the tables below are their summary. All runs on Windows 11, the engine built from this branch into
the scratch target folder, two other agents running Studio tests on the machine at the same time.

### The cause

**H1, both windows.** A Studio that quits while the `rcfd serve` it has just started is not yet connected leaves it
running: detached, with no living parent, and in W1 with no record of its pid anywhere. `close()` then never settles,
the test that awaits it runs into its time-out, the worker into its teardown time-out, and the process stays.

| Run, unchanged tree | Repetitions | Left an `rcfd serve` | `close()` settled in 20 s | `service.json` |
|---|---|---|---|---|
| D2 (a), stand-in, quit in the state `starting` (W1) | 3 | 3 of 3 | 0 of 3 | absent in 3 |
| D2 (b), stand-in, quit with a pid in the state `connecting` (W2) | 3 | 3 of 3 | 0 of 3 | present in 3, the pid of the process left |
| D3, the real engine, quit as soon as the first window exists | 20 | 9 of 20 | 11 of 20 (the 11 that left none) | absent in 5 of the 9 (W1), present in 4 (W2) |
| D2 (c), stand-in, the test awaits `close()` | 1 | 1 | no | absent |
| D4, `project.spec.ts:245` made to wait for ever, service connected | 1 | 0 | not applicable | not applicable |
| D1, each of the 42 spec files alone and the unit project | 43 steps | 0 in each | not applicable | not applicable |

D2 (c) printed the two lines of the lead's logs, in their order: the test "(1.0m)" with "Test timeout of 60000ms
exceeded", then "Worker teardown timeout of 60000ms exceeded." and "1 error was not a part of any test"; one process
left, `…\standin\release\rcfd.exe serve --addr 127.0.0.1:0 --ui --journal-dir <tmp>\rcfd-studio-state-f892c-mczCKD\journals`,
pid 54764, parent id 58096 (a process that no longer existed). The 9 of D3 are the real `rcfd.exe` of the scratch
target folder with `--ui --journal-dir <tmp>\rcfd-studio-state-f892r-…\journals`, each with a parent id that no longer
existed (pids and parent ids per repetition in `diag-before/rows.jsonl`). That is the command line of the processes
found on 2026-11-23, from a target folder, one per time-out.

H2 and H3 are not shown: D4 left none, and no file of D1 left one, the four that end a Studio by its pid among them.
H4 was not seen and was not forced.

**C1 is met with the real engine (D3) and with the stand-in (D2).** The test itself, `project.spec.ts:245`, was not
caught leaving one: it passed in D1 and in the plain whole runs. What is shown is its path, a quit just after the
service was started, taken by the real Studio and the real engine in 9 of 20 quits on this machine, and the lead's two
lines reproduced from that path.

**One detail of finding 5 corrected.** The pid Playwright has for the Studio on Windows is that of the `cmd.exe` it
launches the Studio through, the Studio's parent; `taskkill /T` on it finds nothing once the Studio and its shell have
ended, as finding 5 says of the Studio. The registry's pid is therefore used only to name the spec file.

**Seen and not explained:** in D2 (b) before the fix the Studio's process was still alive 20 s after `close()` in 2 of
the 3 repetitions (it ended when the stand-in was ended). After the fix it ended in 3 of 3.

### The fix

As registered. `spawn.ts`: `startProcess` (the spawn and the wait for the address) with the hooks `cancelled` and
`onSpawned`; `startService` builds the arguments and calls it. `client.ts`: the child is known from the spawn on;
`shutdown()` ends a child that has not reported its address, ends a child of this start it was never connected to (in
both modes), and nothing is spawned once it has begun; the option `startService` for the unit tests. Harness:
`tests/support/leftovers.ts` and `global-teardown.ts`, the registry written by `launchStudio`, the variable and the
teardown named in `playwright.config.ts`. No existing spec file is changed; no golden, protocol, default or dependency.
`studio.ts` is a shared helper of the Studio's tests: `launchStudio` gained the registry lines and nothing else.

### Checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| C1 | at least 1 left, recorded | D3: 9 of 20; D2 (a) (b): 6 of 6; D2 (c): 1 | yes |
| D1, D2, D3, D4 before the fix | counted | the table above | made |
| T1 | pass | 5 of 5 tests, alone and in the unit project | yes, after one correction of the test (below) |
| T1m | fail | M1: T1 (a) fails ("the stand-in to be gone", 5.1 s), 1 failed, 4 passed | yes |
| T2 | pass | 6 of 6, (b) with the real `rcfd` | yes |
| A1 | 0 left and `close()` settled in each | D2 (a) 3, D2 (b) 3, D3 20: 0 left and settled in 26 of 26; D2 (c): the test passes in 2.5 s, 0 left | yes |
| A2 | 0 `rcfd serve` of mine after each of 4 | 0 after each of 5 whole runs (table below); 0 of any of the four names | yes |
| K1 | 0 such lines | 0: no existing spec file in the diff; `retries: 0`, `workers: 1`, `timeout: 60_000` kept | yes |
| G1 | exit 0 | `npm run typecheck` exit 0; `service-client.spec.ts` 30 of 30; the whole `npm test` exit 1 in every run, on tests outside this item (below) | no for the whole run |
| G2 | pass | see the end | see the end |

| Whole run of `npm test` | Tree | Passed, failed, did not run | `rcfd serve` left | Other rcfd processes left | Ended by the teardown |
|---|---|---|---|---|---|
| A2-1 | 957f9d60 | 834, 3, 28 | 0 | 0 | 0 |
| A2-2 | the same | 833, 4, 28 | 0 | 0 | 0 |
| A2-3, `project.spec.ts:245` made to time out | the same | 830, 5, 30 | 0 | 0 | 0 |
| A2-4 as first run: its scratch patch did not apply, so a third plain run | the same | 820, 6, 39 | 0 | 0 | 0 |
| A2-4, `service.spec.ts:145` made to time out | 1d3a9a58 | 833, 4, 28 | 0 | 0 | 0 |

The teardown printed no line in any run: nothing was left for it to end, with or without a time-out. Its sweep is
therefore shown only by T2 (b), not by a process it had to end in a whole run.

**Mutations.** M1 (committed): T1 (a) fails. M2: T1 (b) fails in both modes. M3: T1 (c) fails. M4: T1 (d) fails. M5:
T2 (b) fails and two cases of T2 (a). M6: the case "never a process of another name" of T2 (a) fails. Each was applied
to the worktree, run and removed by `git checkout`; M1 and M4 were run again after the correction of T1 (d).

### Deviations and what was not run

- **T1 (d) had a defect of its own, found by the first whole run.** Its state listener called `shutdown()`, which
  changes the state in the same turn, so the listener was entered again without end; alone the test passed because the
  client swallows a listener's exception, in the whole unit project the worker ended with "Maximum call stack size
  exceeded" and the test was reported failed in A2-1 to A2-3 and the first A2-4. Corrected in the test (a flag set
  before the call, 1d3a9a58); no limit and no product code changed. The unit project then passed it (546 passed, 1
  failed, the failure being `data-routes.spec.ts:94`), and so did the second A2-4.
- **A2-4 was made twice.** The scratch patch for `service.spec.ts` matched two places and was not applied, so the
  first A2-4 was a plain run; it is kept in the table. The patch was corrected and the registered run made once.
- **The packaged-copy rules of the teardown are narrower than registered:** rules 1 and 2c apply only when the run's
  registry has a launch, so that a run of the unit tests alone never ends a Studio someone has open from
  `out/packaged`; of a Studio only the main process is ended. The registry variable is `RCFD_TEST_LAUNCH_REGISTRY`.
- **One line beyond the registration in `client.ts`:** the record of a child that was ended while its record was being
  written is removed (seen in A1 as a `service.json` naming a process that no longer existed, 3 of 20).
- An assertion on the client's state in T1 (c), not registered, was written and taken out before the first commit:
  the state says `starting` after a shutdown in the same turn, which this work did not change.
- Not run: CI, Linux, macOS; H4.

### Other tests that failed in these runs (reported, not fixed)

- `tests/unit/data-routes.spec.ts:94` and `tests/e2e/data.spec.ts:73`: in D1 on the unchanged tree and in every whole
  run. `data.open` gives `null` where the tests expect the length unit `"m"`. Not touched by this work.
- `tests/e2e/responsiveness.spec.ts:260`: A2-2, A2-3 and both A2-4 (the subject of F-890).
- `tests/e2e/f837.spec.ts:198`: D1 and the first A2-4 (the subject of F-891).
- `tests/e2e/run-control.spec.ts:320` ("Pause requested from the Studio|acted" not found in 5 s): D1 and the first A2-4.
- The tests the two scratch patches made to time out, in their runs, as intended.

### Size

Size (agent), actual: done; 0; 6; 0; about 270000

Six heavy runs against five registered: D1 and five whole runs, the fifth being the repeated A2-4.

### Decision

By the registered rule: **done.** The cause is shown by recorded runs that leave a process (C1: 9 of 20 with the real
engine, and the lead's two lines with one process left); the registered whole runs after the fix leave none, two of
them with a test made to time out (A1, A2, K1); the committed mutation is caught (T1m). G1 is not met for the whole
`npm test`, on tests outside this item, as the registration expected. No record has shown a second path.

### Open questions for the maintainer

None.

## For the lead: roadmap entries

Status proposed: **done**.

> ADR 0789, 2026-11-23, done: the path is in the Studio's quit: a Studio that quits while the rcfd serve it has just started is not yet connected (spawned and not yet listening, or listening and not yet connected) left it running, detached; on Windows Playwright's close() then waits for ever on the pipes the service inherited, the test times out, the worker's teardown times out, and the forced end finds no parent to walk. Shown by records: 9 of 20 quits with the real engine left one, 6 of 6 with a stand-in engine, and one run printed the lead's two time-out lines and left one; project.spec.ts:245 takes this path but was not itself caught. Fix: nothing is spawned once the quit has begun, the child is known from the spawn on and ended by the quit, a child never connected to is ended; the tests' global teardown ends and prints what a run leaves. After it 0 of 26 quits left one, and 0 processes after each of 5 whole runs of npm test, 2 with a test made to time out; 6 mutations caught, one committed.

Follow-ups, for the lead to decide:

> Studio: data.open gives no length unit where data-routes.spec.ts:94 and data.spec.ts:73 expect "m". Deps: F-892. Acceptance: Every limit not stated here is registered in the ADR before the run. The cause of the two failures recorded in ADR 0789 addendum 1 (null where the length unit "m" is expected, on main at c4bd79f3 with the engine built from it, in every run) is found and shown by a recorded run, and the two tests then pass without being skipped or changed in what they check.

> Studio: run-control.spec.ts:320 does not find the pause request within 5 s in some whole runs. Deps: F-892. Acceptance: Every limit not stated here is registered in the ADR before the run. The cause of the failure of apps/studio-web/tests/e2e/run-control.spec.ts:320 recorded in ADR 0789 addendum 1 and in the lead's push run of F-861 on 2026-11-23 is found and shown by a test that fails without the fix; the test then passes a registered number of consecutive runs on Windows, not skipped, retried or given a longer limit.

### Checks at the end (G2)

- `cargo test -p rcfd-cli --release --test docs_site`: 2 passed, 0 failed.
- `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0789-F-892.json`: accepted (F-892 todo to done, two new items).
- `python scripts/process_report.py --check`: exit 1 with 1 failure, "generated file is not current: docs/adr/index.md", which rule 7 leaves to the lead; nothing else.
- No Rust crate was touched: `cargo fmt` and `cargo clippy` do not apply.
