# ADR 0786: F-888 registration: the Studio's end-to-end test of the quick run by keyboard that receives "inactive" on the Run button (the hypotheses of the cause, the runs that tell them apart, the test that must fail without the fix, the consecutive runs and the count of processes)

Status: **REGISTERED 2026-11-23, written before any code of F-888 and before any run of its checks.** Before this text no
Studio source or test of F-888 was written and no Studio test was run. What was done: reading (below), searches of the
source tree, one listing of the `rcfd` processes alive on the machine, and two preparations that run no check: the
release build of `rcfd-cli`, `rcfd-mcp` and `rcfd-runner` into the scratch folder's own target folder, and `npm ci` in
`apps/studio-web`. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f888-studio-quick-run-focus.md`
([the file](briefs/f888-studio-quick-run-focus.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5, and
[docs/studio-guardrails.md](../studio-guardrails.md). Branch `f888-studio-quick-run-focus` from main a4d32bff, scratch
`w49i-work/f888-studio-quick-run-focus-scratch`. One agent beyond the limit of 4 at once was allowed by the maintainer on
2026-11-23 ("Yes, one over") for this item alone; that is the lead's record in the roadmap note of F-888 and in the
brief, repeated here because the brief asks for it.

## What is asked

Roadmap item F-888 (deps F-807), the binding acceptance: "Every limit not stated here is registered in the ADR before
the run. The cause of the failure of apps/studio-web/tests/e2e/run-control.spec.ts:192 on the lead's Windows machine
(toBeFocused receives "inactive" on the Run button, in each of three runs of npm test, while the test passes on CI's
windows-latest) is found and shown by a test or a recorded run that fails without the fix; the test then passes a
registered number of consecutive runs of npm test on that machine, whatever window has the system's focus, and on CI.
The test is not skipped, retried or given a longer limit to make it pass, and what it checks of the tab order and the
focus ring is kept. The rcfd serve processes that the end-to-end tests leave running after npm test on Windows are
counted before and after the fix."

Clauses: **c1** the cause is found and shown by a test or a recorded run that fails without the fix; **c2** the test
passes the registered number of consecutive runs of `npm test` on this machine, whatever window has the system's focus;
**c3** it passes on CI; **c4** no skip, no retry, no longer limit, and the checks of the tab order and of the focus ring
are kept; **c5** the `rcfd serve` processes left after `npm test` are counted before and after the fix.

## What was read

- The rules, the guardrails and the brief, in full.
- `apps/studio-web/tests/e2e/run-control.spec.ts` lines 1 to 300, `tests/support/studio.ts`, `tests/support/rcfd.ts`,
  `tests/support/launch.ts`, `tests/support/project.ts` (lines 1 to 113), `playwright.config.ts` and `package.json`.
- The source the test drives: `src/renderer/runs/RunControls.tsx` (`RunBar`, lines 172 to 255),
  `src/renderer/runs/Sites.tsx` (`WhereSelect`), `src/renderer/ui/Select.tsx` (in full), `src/renderer/ui/primitives.css`
  (the popover), `src/main/index.ts` (the test mode's window, lines 520 to 560), `src/main/project/sites.ts`
  (`sitesFilePath`), `src/main/engine/locate.ts` (the head).
- The lead's log `w49i-work/push-f861c.log`: the three failures (lines 1732, 2735 and 3735, with the error at 1865 to
  1896) and the lines around them.
- Two libraries the Studio and its tests depend on, as installed (`apps/studio-web/node_modules` of the main checkout),
  read to use them, not as a source of an algorithm: Playwright 1.65.1 (`playwright-core/lib/coreBundle.js`: the
  expression `to.be.focused`, `_activelyFocused`, and the place that sends `Emulation.setFocusEmulationEnabled`) and
  react-aria 3.53.1 (`react-aria/dist/private/focus/FocusScope.mjs`, `useRestoreFocus`).
- `docs/adr/briefs/f852-studio-close-session-timeout.md` and the registration and the mutation patch of that work on its
  own branch, for the form of a Studio registration.

## Findings of the reading (before any run)

1. **"inactive" does not say that the window lacks the focus.** Playwright's `to.be.focused` computes
   `activeElement === node && node.ownerDocument.hasFocus()` and reports "focused" when that holds and "inactive" in
   every other case. "inactive" is therefore also what it reports when another element, or the body, is the document's
   active element. The lead's reading (the Run button is the active element of a document without the system's focus) is
   one of several states that give this word; the word alone does not tell them apart.
2. **Playwright asks Chromium to emulate the focus.** For the main frame of every page it sends
   `Emulation.setFocusEmulationEnabled { enabled: true }` unless the launch asks for no defaults. If that reaches the
   Studio's page, `document.hasFocus()` is true whatever window has the system's focus.
3. **The test window never takes the system's focus by design.** In test mode (`RCFD_TEST_HEADLESS=1`, set by
   `playwright.config.ts`) the window is shown transparent, passes mouse input through and is opened "without taking the
   focus" (`src/main/index.ts`). So the Studio window lacks the system's focus in every end-to-end test on every machine,
   CI's included.
4. **Only this test failed.** In the same three runs the other tests that press Tab and then call `toBeFocused` passed
   (for example `primitives.spec.ts:398` at log line 703 and `undo.spec.ts:302` at log line 835). The directory has 87
   calls of `toBeFocused` in 21 spec files. A window without the focus would be expected to fail them all alike.
5. **What is particular to line 209.** The Tab at line 207 follows a choice in a select: Enter opens the listbox, Enter
   on the option chooses it and closes the popover, line 206 waits for the select's text, line 207 presses Tab. When a
   react-aria overlay closes, the focus goes back to its trigger in a `requestAnimationFrame` callback, and only "if
   we've lost focus to the body" at that time (`useRestoreFocus`, the clean-up of its last layout effect). Between
   React's commit that removes the listbox (after which the select already shows the new text, so line 206 passes) and
   the next animation frame, the document's active element is the body. A Tab pressed in that interval starts from the
   body, not from the select, and the frame that follows finds the focus no longer on the body and restores nothing.
6. **The tab stops between the select and Run.** `RunBar` renders, in order: the lead content of F-837 (before the
   select), the case select, `WhereSelect`, Run, Queue…, Start the runner. `WhereSelect` renders nothing unless the sites
   file lists a site; `launchStudio` names a sites file that is not there
   (`<tmp>/rcfd-studio-tests-no-sites-file.toml`) unless the environment of the test run already has `RCFD_SITES_FILE`.
   On this machine that file is absent and no `RCFD_*` variable is set in the agent's environment.
7. **The processes left.** At the time of this text three `rcfd serve` processes are alive on the machine (pids 61492,
   62008 and 9196, started 04:15, 05:25 and 05:53 today), none of them started by this work. Each was started with
   `--ui --journal-dir <tmp>/rcfd-studio-state-…/journals`, the form of the service a Studio starts for its first
   project, from a `target/release` folder (the folder beside `rcfd-mcp`, which is where a Studio launched with
   `RCFD_STUDIO_MCP` takes its `rcfd` from). `run-control.spec.ts`, `runs.spec.ts`, `first-path.spec.ts` and
   `runner-lifetime.spec.ts` end a Studio by its process id (`killPid`) and then end the project servers
   (`mcpPids`), and nothing in those paths ends the service that Studio started. In `run-control.spec.ts` that path is
   the `afterAll`, taken with a Studio still open when a test before the closing ones fails.

## Hypotheses of the cause and what tells them apart

| Id | Hypothesis | What it predicts at the failing `toBeFocused` |
|---|---|---|
| H1 | The window lacks the system's focus (the lead's reading) | The Run button is the active element and `document.hasFocus()` is false; giving the window the focus makes the test pass with nothing else changed |
| H2 | The order of Tab stops differs on this machine | The case select had the focus when Tab was pressed, and the active element afterwards is a named control between the select and Run |
| H3 | A timing of the select's close: Tab is pressed before the focus is back on the select | When Tab is pressed the active element is not the case select; the focus returns to the select later or not at all; `document.hasFocus()` is true; a delay of the animation frames makes it fail every time, and waiting for the focus on the select makes it pass |
| H4 | None of these | The record fits no row above |

The records that tell them apart, made on the unchanged source and test of main a4d32bff, in the packaged Studio built
from it, with the engine of this branch's own build:

- **D1, the unchanged test.** `npx playwright test tests/e2e/run-control.spec.ts -g "quick run from the Runs view"
  --project=electron`, 10 times in a row; the number of failures at line 209 is recorded. No limit: it says whether the
  failure shows on this machine today.
- **D2, the instrumented record.** The same test with instrumentation only (a scratch patch of the spec, applied for the
  run and removed, kept in the scratch folder, not committed): before the first Enter the page starts a log of `focusin`,
  `focusout` and `keydown` events with their times and of each animation frame's time; at the place of line 209, in
  place of failing at once, it records the active element (tag, role, accessible name), `document.hasFocus()`,
  `document.visibilityState`, whether a listbox is in the document, `BrowserWindow.isFocused()` from the main process,
  the active element at the Tab's `keydown`, the time from the second Enter's `keyup` to the first `focusin` on the
  select after it, and the time from that `keyup` to the Tab's `keydown`; then it makes the unchanged assertion. 10 runs.
- **D3, the controls,** 5 runs each, on the same instrumented test:
  (a) every `requestAnimationFrame` callback of the page delayed by 300 ms from just before the choice (the page's own
  function wrapped; removed after the tab-order checks);
  (b) as (a), and before the Tab the test waits until the case select has the focus (`toBeFocused` on the select, the
  default limit of 5 s, no other change);
  (c) as (a), and before the keyboard sequence the main process calls `BrowserWindow.focus()` (the window is asked to
  take the system's focus; what `isFocused()` then says is recorded; `blur()` after the checks);
  (d) no delay, no wait, and `BrowserWindow.focus()` as in (c).

Reading of the records: H1 is shown if D2's failures have the Run button active with `hasFocus()` false and (d) passes
where D1 fails. H2 is shown if D2's failures have the select active at the Tab and a named control other than Run after
it. H3 is shown if D2's failures (or, where D1 and D2 do not fail today, the runs of (a)) have an active element other
than the select at the Tab's `keydown` with `hasFocus()` true, (a) fails in 5 of 5, (b) passes in 5 of 5 and (c) fails
in 5 of 5. Any other pattern is H4 and is recorded as not deciding; an amendment then registers the next record before
it is made. If D1 does not fail in any of its 10 runs, the cause can still be shown by (a), (b) and (c), which do not
depend on the machine's timing; that the lead's three failures were this cause then rests on the agreement of (a)'s
error text and line with the lead's log, and the addendum says so in those words.

## Design registered

The fix follows the hypothesis shown, as the brief asks ("whichever the cause names"):

- **If H3** (the reading's expectation, finding 5): the fix is in the test, because the Studio does what its component
  library does (the focus returns to the trigger one animation frame after the overlay closes) and no user presses
  Tab within that frame. A shared helper in `apps/studio-web/tests/support/` chooses an option of a select by keyboard
  and returns when the select has the focus again; `run-control.spec.ts:192` uses it. This adds a check (G8: the focus
  returns to the select after a choice) and removes none: the Tab from the select to Run, to Queue…, to Start the runner
  and to the filter, and the focus ring on Run, stay as they are, with their default limits. No retry, no `test.skip`,
  no `test.slow`, no longer time-out. Every end-to-end spec that checks focus is then read for the same exposure (a
  keyboard action that relies on the focus returned by a closing overlay, with nothing awaited in between), and the
  ones exposed are listed in the addendum and use the same helper or the same wait.
- **If H1**: the harness (`launchStudio` or a helper) makes the document's focus independent of the system's (the
  emulation of finding 2, or the window focused before keyboard checks), and every spec that checks focus is listed.
- **If H2**: the extra tab stop is named and removed at its origin (the harness's environment or the Studio's source).
- The first two edit an existing test file. Rule 2 says existing tests are not edited; the brief asks for exactly this
  edit ("Fix the test or the Studio") and gives the file to this work, so the edit is made in place and named as such
  in the addendum.

**The test that fails without the fix (T1), if H3.** A new spec `apps/studio-web/tests/e2e/run-bar-keyboard.spec.ts`
(F-888, ADR 0786): the packaged Studio on a copy of the quick-start project, the Runs view, every
`requestAnimationFrame` callback of the page delayed by 300 ms; it chooses the case with the shared helper, then
checks Tab to Run, Queue…, Start the runner and the filter, and the focus ring on Run, as line 192 does. It starts no
run and no runner. With the helper's wait removed (the committed mutation) it must fail at the `toBeFocused` on Run with
"inactive". 300 ms: the interval between the choice and the Tab is two round trips of Playwright (a few ms each on one
machine), so a frame delayed by 300 ms comes after the Tab by a factor of more than 10, and 300 ms is a factor of more
than 10 inside the default 5 s of the wait. If another hypothesis is shown, an amendment registers its test before the
test is written.

**The processes.** A scratch script lists, by `Get-CimInstance Win32_Process`, the processes named `rcfd.exe`,
`rcfd-mcp.exe`, `rcfd-runner.exe` and `rcfd-studio.exe` with their command lines. It is run before and 15 s after each
run below; "left" means a process in the second list and not in the first whose executable is under this work's scratch
target folder or this worktree (other agents' processes are not counted, and none is ended). Processes this work's
runs leave are ended by this work after they are counted (they are its own), by pid.

- **P1, before the fix:** one whole `npm test` on the unchanged tree, counted.
- **P2, attribution, before the fix:** each of `run-control.spec.ts`, `runs.spec.ts`, `first-path.spec.ts` and
  `runner-lifetime.spec.ts` run alone, counted; and `run-control.spec.ts` with `-g "quick run from the Runs view"`
  (its `afterAll` then runs with the Studio open, the path a failure of line 192 takes), counted. Expectation from
  finding 7: the last leaves 1 `rcfd serve`.
- **Fix, if the cause is in the harness as finding 7 expects:** the shared way the tests end a Studio by pid also ends
  the service that Studio started (its pid read from the page's service state before the kill, ended with `killIfRcfd`
  after the Studio's process has ended), in every spec file with that path. If the cause is elsewhere it is reported,
  not fixed here.
- **P3, after the fix:** the count around each of the five runs of N1, and around the forced path of P2
  (`-g "quick run from the Runs view"`).

## Checks

Kinds as rule 14 names them; none of these is a validation against a measured case, and the acceptance claims none.

| Check | Clause | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| C1: the cause shown by D2 and D3 as read above (one of H1, H2, H3) | c1 | one hypothesis shown, by the reading registered above | acceptance of F-888 | H3 | not a number | code verification |
| T1: the new spec passes on the fixed tree | c1 | pass | acceptance of F-888 | pass | not a number | code verification |
| T1m: the committed mutation makes T1 fail at the `toBeFocused` on Run | c1 | fail, "inactive" | acceptance of F-888 ("fails without the fix") | fail | not a number | code verification |
| N1: consecutive runs of `npm test` on the fixed tree in which `run-control.spec.ts:192` passes, the Studio window not having the system's focus | c2 | 5 of 5 | the brief ("at least 5") | 5 of 5 | 1 | code verification |
| N2: the fixed test (not the new spec) with `BrowserWindow.focus()` called before its keyboard sequence, as D3 (c) without the delay, by a scratch patch | c2 | 5 of 5 | acceptance of F-888 ("whatever window has the system's focus") | 5 of 5 | 1 | code verification |
| K1: the diff of `run-control.spec.ts` removes no assertion, adds no `retries`, `skip`, `fixme`, `slow` or time-out, and `playwright.config.ts` keeps `retries: 0` | c4 | 0 such lines | acceptance of F-888 | 0 | not a number | code verification |
| P3: processes left after each of the 5 runs of N1 and after the forced path | c5 | 0 in each | the brief ("the test leaves no rcfd serve process behind") | 0 | not a number | code verification |
| P1, P2: processes left before the fix | c5 | counted, no limit | acceptance of F-888 ("counted before and after") | 1 after the forced path | not a number | code verification |
| G1: `npm run typecheck` and the whole `npm test` (the last run of N1) | rule 7 | exit 0 | agent rules, rule 7 | exit 0 | not a number | unchanged default output |
| G2: `cargo test -p rcfd-cli --release --test docs_site` and `python scripts/process_report.py --check` | rule 7 | pass | agent rules, rule 7 | pass | not a number | unchanged default output |

For N1 the window that has the system's focus is recorded at the start and at the end of each run (the process name of
the foreground window, by a scratch script; no window title is recorded), together with `BrowserWindow.isFocused()` of
the Studio as the new spec logs it. No limit is registered on wall-clock time. A failure of another test in a run of N1
is recorded with its test and error and reported; it does not count against N1, whose subject is line 192, but G1 then
fails for that run and the addendum says so. A test that fails on a file the operating system refuses under the load
of the other agents is reported, not retried until it passes.

**Negative control and mutations (rule 16).** D3 (a) is the negative control of the fix: the unfixed sequence under the
delay. Planned mutations, each applied to the worktree, run and removed:

- M1 (committed, `apps/studio-web/tests/e2e/run-bar-keyboard.spec.mutation.patch`): the helper's wait for the focus on
  the select removed. T1 must fail.
- M2: Run and Queue… exchanged in `RunBar` (the tab order). `run-control.spec.ts:192` and T1 must fail at the
  `toBeFocused` on Run.
- M3: the focus ring of the button removed (`outline: none` on the focused button in `primitives.css`).
  `run-control.spec.ts:192` and T1 must fail at the check of the outline.

M2 and M3 show that the fixed test still checks the tab order and the focus ring (c4).

Size (agent): done; 0; 7; 0; 75000

The heavy runs are the whole `npm test`, about 11 minutes each in the lead's log: one before the fix and five after,
and one allowed for a repeat of the packaging if a mutation needs it. They are run once each, one at a time.

## Decision rule

As the brief gives it: **done** if the cause is shown by a run that fails without the fix (C1, T1 and T1m), the test
then passes the registered number of consecutive runs with the focus elsewhere and keeps its checks (N1, N2, K1, M2,
M3), and the processes are counted (P1 to P3 made). **Partial** otherwise, and in particular if the records give H4, if
a run of N1 fails at line 192, or if P3 is not 0 in a run and the cause is not one this work may fix. **Todo** if no
record could be made.

## What is not covered

- **c3, CI.** A branch that is not pushed has no CI run. The fix is read against CI's two jobs (windows-latest and
  Ubuntu under xvfb) and uses nothing particular to a platform, but the run on CI is made by the lead's push and read
  by the lead; the addendum names this clause as not run by this work.
- macOS, and a Studio window shown opaque (`RCFD_TEST_HEADLESS=0`).
- The other tests of the suite beyond what the reading for the same exposure finds; a failure of another test seen in
  the runs is reported, with a follow-up proposed if it has a cause of its own.
- The three processes of finding 7: they are counted and left alive; whose they are is the lead's to say.

## Open questions for the maintainer

None at registration.

## Amendment 1 (2026-11-23): the test does not run on CI, and a second test that does; written before any code of the fix

Written after the records D1, D2 and D3 and the attribution runs P2 were made (their numbers are in Addendum 1) and
before any line of the fix, of the helper or of the new spec. It changes no limit and removes no check; it adds one test
and one open question.

**What was read.** `.github/workflows/ci.yml`, the job studio-web: it builds no Rust and packages with
`npm run package:dir -- --no-engine`, and its comment says "The tests that need the engine are skipped with the reason".
`run-control.spec.ts` begins with `test.skip(!mcp, NO_MCP)`. The log of the job on windows-latest of the newest green
run on main (run 45134411685, job 137559834451, commit cda8e88e, fetched with `gh run view --log`; a copy is in the
scratch folder under `ci/`) lists test 728, `run-control.spec.ts:192`, with the mark of a skipped test, and ends with
"275 skipped, 577 passed". So the sentence of the brief and of the acceptance "the test passes on CI's windows-latest"
does not hold as written: on CI the test is skipped, on both systems, and CI says nothing about it. The difference
between the lead's machine and CI is first of all that only the lead's machine runs the test.

**Consequence for T1.** T1 as registered opens a project and so needs the engine; on CI it would be skipped like
line 192. It stays as registered.

**T2, added.** A second test in the same new spec, `run-bar-keyboard.spec.ts`, that needs no engine and therefore runs
on CI: the packaged Studio's primitives gallery (`#/dev/primitives`, the page `primitives.spec.ts` drives), the select
"Sample state filter". It first notes which control a Tab from that select reaches when no listbox was opened; then,
with every `requestAnimationFrame` callback delayed by 300 ms, it chooses the option "Running" with the shared helper
and presses Tab, and the same control must have the focus. Limit: pass on the fixed tree; with the committed mutation M1
it must fail, as T1. Source of the limit: the acceptance of F-888 (c1 and c3). Estimate: pass, and fail under M1. Kind:
code verification. T2 is the part of this work that CI runs; c3 for line 192 itself stays as "What is not covered" says,
and is now an open question.

**Open question for the maintainer, added (Q1).** It is put with options and a recommendation in Addendum 1; this
work does not decide it, because it changes what CI builds and how long the job takes (rule 6).

## Addendum 1: results (2026-11-23)

All runs were made on the lead's Windows machine, in the release profile of the engine (this work's own build in the
scratch target folder, from a4d32bff), in the packaged Studio, while three other agents ran on the machine. Logs and
records are in the scratch folder; the names in brackets are their files.

### The cause

**H3 is shown, as the reading registered it; H1 and H2 are excluded.** The Tab at line 207 of
`run-control.spec.ts` is sent while the document's active element is the body: the listbox has closed, and the focus
goes back to the select only in the next animation frame (react-aria's `useRestoreFocus`). A Tab from the body lands on
the first control of the page (an unnamed `ui-button`), the frame that follows finds the focus elsewhere and restores
nothing, and `toBeFocused` on Run reports "inactive", which is the word Playwright gives for every element that is not
the focused one.

| Record | Runs | Failed at the `toBeFocused` on Run | Active element at the Tab's keydown | `hasFocus()` | Window had the system's focus | Enter to focus back on the select | Enter to Tab |
|---|---|---|---|---|---|---|---|
| D1, unchanged test [d1-*.log] | 10 | 0 | not recorded | not recorded | not recorded | not recorded | not recorded |
| D2, instrumented [d2/] | 10 | 0 | the case select, 10 of 10 | true | no | 4.7 to 7.0 ms | 11.1 to 16.7 ms |
| D3 (a), frames delayed 300 ms [d3a/] | 5 | 5 | the body, 5 of 5 | true | no | never | 8.7 to 15.2 ms |
| D3 (b), delayed, wait for the select [d3b/] | 5 | 0 | the case select, 5 of 5 | true | no | 307 to 310 ms | 314 to 331 ms |
| D3 (c), delayed, window focused [d3c/] | 5 | 5 | the body, 5 of 5 | true | yes | never | 9.5 to 12.6 ms |
| D3 (d), no delay, window focused [d3d/] | 5 | 0 | the case select, 5 of 5 | true | yes | 4.7 to 5.8 ms | 11.8 to 13.6 ms |

Times are from the `keydown` of the Enter that chooses the option (the choice is made on the key's way down, not on
`keyup` as the registration's wording of D2 assumed; the script `read_recs.py` measures from the `keydown`).

- **The registered reading of H3 holds in every part:** (a) failed 5 of 5 with the body active at the Tab and
  `hasFocus()` true; (b) passed 5 of 5; (c) failed 5 of 5.
- **H1 is excluded.** `document.hasFocus()` was true in all 30 records while `BrowserWindow.isFocused()` was false in 20
  of them (finding 2: Playwright's focus emulation reaches the Studio's page). The test passed without the system's
  focus (D1, D2) and failed with it (c). Whether the window has the system's focus changes nothing.
- **H2 is excluded.** In every passing record the Tab went from the case select to Run; in the failing ones the select
  never had the focus at the Tab.
- **The error is the lead's.** The failures of (a) have the text of the lead's log word for word: `Received: inactive`,
  `14 × locator resolved to <button … aria-label="Run cavity-re100.toml" class="ui-button ui-button--primary">`, at the
  same assertion [d3a-1.log; `push-f861c.log` lines 1867 to 1878].
- **What was not shown.** The failure did not show on this machine today without the delay (0 of 20 in D1 and D2), so
  no record was made of the lead's three failures themselves. That they were this cause rests on the agreement of (a)'s
  error text and line with the lead's log, as the registration said it would in this case. Today the margin was small:
  the focus was back 4.7 to 7.0 ms after the choice and the Tab came at 11.1 to 16.7 ms, with an animation frame every
  2.6 to 4.7 ms (the median of each record; a display at about 240 Hz). At 60 frames a second a frame comes every
  16.7 ms, later than every Tab recorded. Why the frames were slower in the lead's runs between 04:15 and 05:53 (a
  display asleep, a locked session or the load are candidates) was not measured and is not claimed.
- **CI.** Amendment 1: the test is skipped on CI, so CI never showed it passing.

### The fix

In the test, as registered for H3. `tests/support/keyboard.ts` (new): `chooseByKeyboard(select, option)` makes the
choice and returns once the select shows it and has the focus again; `delayFrames` and `restoreFrames` delay the page's
animation frames for a test. `run-control.spec.ts` (the test is now at line 193, one import line above it): the four
lines of the choice are replaced by the call of the shared step; the `toContainText` after it, the four Tab checks and
the focus-ring check are unchanged. This is an edit of an existing test in place, on the brief's instruction (the
registration, "Design registered"). `run-bar-keyboard.spec.ts` (new) holds T1 and T2. No Studio source is changed.

**Other specs with the same sequence** (read: every `toBeFocused`, every Tab and every Escape or option choice by
keyboard of `tests/e2e`, by search and the lines around each hit, not every file line by line). Exposed in the same way
(a key that relies on the focus a closing overlay returns, with nothing awaited between): only
`run-control.spec.ts:207`. The same choice in a select by keyboard, not exposed: `compare.spec.ts:392` (already waits
for the focus on the select, with a comment saying why), `provenance.spec.ts:112` (the next step focuses its target
itself), `a11y.spec.ts:361` (the next step presses Tab until its target has the focus) and `runs.spec.ts:158` (a click
follows). A dialog closed with Escape and a key after it: `close-guard.spec.ts:113` and `:145` and
`run-control.spec.ts:372` wait for the focus on the opener first; the others are followed by a click, an explicit focus
or the end of the test. None of these was changed.

### Checks

| Check | Limit | Measured | Met |
|---|---|---|---|
| C1 | one hypothesis shown | H3, by D2 and D3 (a), (b), (c) as registered | yes |
| T1, the run bar under delayed frames | pass | pass, in the run of the two specs after the fix [t-fixed-1.log] and each of the five runs of N1 | yes |
| T2 (amendment 1), the gallery under delayed frames | pass | pass, in the same runs | yes |
| T1m and T2 under M1 | fail, "inactive" | both failed with `Received: inactive` at the `toBeFocused` after the Tab [m1-1.log] | yes |
| N1 | 5 of 5 | 5 of 5: `run-control.spec.ts:193` passed in each of five consecutive whole runs of `npm test` [n1-*.log] | yes |
| N2 | 5 of 5 | 5 of 5; `isFocused()` true in 4, false in 1 (Windows did not give it the focus) [n2-*.log] | yes |
| K1 | 0 | 0: the diff of `run-control.spec.ts` is one import and four lines of key presses replaced by the shared step and a comment; no assertion removed, no retry, skip or time-out; `playwright.config.ts` untouched (`retries: 0`) | yes |
| P3 | 0 in each | 0 after each of the five runs of N1; 0 after the forced path (the five runs of N2) | yes |
| P1, P2 | counted | 0 left after the whole `npm test` before the fix; 0 after each of the four spec files alone; 0 after the forced path (the 10 runs of D1) and after the 10 failing runs of D3 | counted |
| G1 | exit 0 | `npm run typecheck` exit 0 (inside every `npm test`); `npm test`: exit 1 in each of the five runs, by failures of two other tests (below) | no |
| G2 | pass | `docs_site`: 2 passed; `process_report.py --check`: see the end of this addendum | see the end |

**N1 in detail.** Each run is the whole `npm test` (type checks, build, packaging, every Playwright test) on the fixed
tree at 85d138d6. The window with the system's focus was another program's at the start and at the end of each run, and
the new spec logged `isFocused()` false for the Studio in each.

| Run | Start to end | Foreground window at start, at end | Line 193 | T1, T2 | Whole run | Processes left |
|---|---|---|---|---|---|---|
| 1 | 10:20 to 10:31 | firefox, firefox | pass | pass | 853 passed, 1 failed | 0 |
| 2 | 10:31 to 10:42 | firefox, WindowsTerminal | pass | pass | 853 passed, 1 failed | 0 |
| 3 | 10:43 to 10:54 | WindowsTerminal, Spotify | pass | pass | 853 passed, 1 failed | 0 |
| 4 | 10:54 to 11:05 | Spotify, Spotify | pass | pass | 853 passed, 1 failed | 0 |
| 5 | 11:05 to 11:15 | Spotify, WorkSpacesPersonal | pass | pass | 845 passed, 2 failed, 7 did not run | 0 |

**Mutations (rule 16),** each applied to the worktree, run and removed:

| Mutation | Run | Result |
|---|---|---|
| M1 (committed, `run-bar-keyboard.spec.mutation.patch`): the shared step without its wait | `run-bar-keyboard.spec.ts` | 2 of 2 tests failed, `Received: inactive` |
| M2: Run and Queue… exchanged in `RunBar` | both specs, the three tests | `run-control.spec.ts:193` and T1 failed at the `toBeFocused` on Run; T2 (the gallery) passed, as it must |
| M3: `.ui-button { outline: none !important }` | the same | `run-control.spec.ts:193` and T1 failed at the check of the outline; T2 passed |

D3 (a) is the negative control: the unfixed sequence under the delay, 5 failures of 5.

### The processes

Counted by the scratch script `procs.ps1` (processes named `rcfd.exe`, `rcfd-mcp.exe`, `rcfd-runner.exe`,
`rcfd-studio.exe`), before and 15 s after each run [procs-*.tsv].

| When | Run | Processes before | After | Left by the run |
|---|---|---|---|---|
| Before the fix | D1: the forced path, 10 runs | 3 | 3 | 0 |
| Before the fix | D2 and D3: 30 runs, 10 of them failing at the Run button | 3 | 3 | 0 |
| Before the fix | P2: `run-control.spec.ts`, `runs.spec.ts`, `first-path.spec.ts`, `runner-lifetime.spec.ts`, each alone | 3 | 3 | 0 each |
| Before the fix | P1: the whole `npm test` (814 passed, 3 failed, 35 did not run; line 192 passed) | 3 | 3 | 0 |
| After the fix | N2: the forced path with the window focused, 5 runs | 3 | 3 | 0 |
| After the fix | N1: the whole `npm test`, 5 runs | 3 | 3 | 0 in each |

The 3 are the `rcfd serve` processes of finding 7 (pids 61492, 62008 and 9196), alive before this work and left alive;
this work started none that stayed and ended none. **This work could not make the end-to-end tests leave a process:**
not by a whole run, not by a failure of line 192 (10 failing runs), not by the `afterAll` path with the Studio open.
A Studio ended by its pid takes its service with it here. The cause of the three processes found is therefore not
shown. What is known of them: each was started in the same second as its state folder
(`<tmp>/rcfd-studio-state-…`), from a `target/release` folder and not from `out/packaged/resources/bin`, so by a
Studio that a test launched with `RCFD_STUDIO_MCP` or `RCFD_STUDIO_RCFD`; one belongs to the runs of ADR 0781 (04:15),
two to the lead's runs (05:25 and 05:53). The lead's second run also had `project.spec.ts:245` time out after 60 s,
which no run here did; a test that times out is a candidate this work did not reproduce. A follow-up is proposed.

### Deviations and what was not run

- **P2's expectation was wrong.** The registration expected the forced path (`afterAll` with the Studio open) to leave
  one `rcfd serve`. It leaves none: 0 after 10 such runs before the fix. Finding 7's reading of the harness is therefore
  not the cause of the processes found, no fix of the harness was made, and the cause is reported as not found.
- **The first whole run before the fix (P1) had three failures that were this work's set-up, not the tree's:**
  `data-routes.spec.ts:94` and `data.spec.ts:73` took the example `f447_bench` from the main checkout's older
  `target/release/examples`, because this work's target folder did not have it; after
  `cargo build --release -p rcfd-api --example f447_bench` into the scratch target they passed in every later run. The
  third, `f837.spec.ts:198`, is named below.
- **Other tests that failed in the runs,** none of them changed, retried or skipped: 
  - `responsiveness.spec.ts:260` (F-832: no task over 100 ms while a run opens): passed in P1 at 09:39 (longest task
    53 ms) and failed in all five runs of N1 with one task of 400 to 440 ms in `workspace.tab` in each of its three
    passes. It imports nothing this work changed and the Studio's source is unchanged. Run alone afterwards it failed in
    the same way with three builds of `f447_bench` (this work's, the main checkout's of 2026-11-16 that P1 used, the
    lead's of today), with the Studio built again from the unchanged source, and with a fresh user-data folder
    [resp-*.log]. So it is the state of the machine after about 10:20 and not the tree; what changed was not found. It
    passed in the lead's three runs of this morning. A follow-up is proposed.
  - `f837.spec.ts:198` (the runner's budgets): failed in P1 (the alert "was changed by another writer" not found) and
    in run 5 of N1 (two alerts where one is expected), passed in the other four. 2 of 6 whole runs, at two different
    lines; before and after the fix alike. A follow-up is proposed.
  - Because of these, every whole `npm test` of this work ended with exit 1, so G1 is not met, for reasons outside the
    files of this item. Line 193, T1 and T2 passed in all five.
- **c3 was not run:** see amendment 1 and Q1.
- `cargo fmt` and `cargo clippy`: no crate was touched. `baseline_hashes`: no solver file was touched.
- The instrumented runs with `BrowserWindow.focus()` (D3 (c), (d) and N2, 15 runs) took the system's focus from the
  window in front for about a second each.

Size (agent), actual: done by the registered rule, partial proposed (rule 8); 0; 6 (one whole `npm test` before the fix, five after); 1 (Q1); about 190000 (the registered reading, and the two libraries, the lead's log, the CI workflow and one CI log, which the estimate left out)

### Decision

By the registered rule: **done.** The cause is shown by runs that fail without the fix (C1; T1 and T2 fail under the
committed mutation, which is the fix removed), the test passes five consecutive whole runs with the system's focus on
other programs and five with the window focused, keeps its checks (K1, M2, M3), and the processes are counted before
and after (P1 to P3).

Two things stand beside that, and the proposal file carries them: clause c3 (CI) was not run and, for line 193 itself,
cannot be met while CI skips the engine tests (amendment 1, Q1); and G1 is not met because two other tests fail on
this machine today. Rule 8 says to propose partial where a clause of the acceptance was not run. **The status proposed
is therefore partial,** with a gap the lead can close by reading T2 in the CI run of the merge and by the maintainer's
answer to Q1.

### Open questions for the maintainer

**Q1. `run-control.spec.ts` and every other test that needs the engine are skipped on CI (275 of 852 tests in the job
on windows-latest at cda8e88e). Should CI run them?**

- (a) Leave CI as it is. The engine tests of the Studio run only in the lead's push runs on this machine. The clause
  "and on CI" of F-888 is then met only by T2, which needs no engine.
- (b) Build `rcfd-cli`, `rcfd-mcp` and `rcfd-runner` in the job studio-web (or take them from a job that has built
  them) and package with the engine, on one system or both. The job grows from minutes by the time of a release build
  of three crates, and tests that hold timing limits measured on the maintainer's machine (P2, the frame limits) would
  run on shared runners.
- (c) As (b), in a separate job that is not required for a merge, weekly or on demand.

Recommendation: (c). It shows whether the engine tests hold anywhere else than on one machine without making every pull
request wait for a release build or fail on a runner's timing. It changes what CI builds, so it is not taken here.

## For the lead: roadmap entries

The same entries are in `docs/roadmap/proposals/0786-F-888.json`.

Status proposed for F-888:

> partial

Note entry:

> ADR 0786, 2026-11-23, partial: the cause is in the test and not in the window's focus: after the case is chosen in the select, the closed listbox gives the focus back to the select one animation frame later (react-aria), and the Tab of run-control.spec.ts was sent before that frame, from the body; "inactive" is Playwright's word for any element not focused, and the document has the focus whatever window has the system's (focus emulation). Shown by records: with the frames delayed by 300 ms the unchanged test failed 5 of 5 with the lead's error text, with and without the system's focus, and passed 5 of 5 once it waited for the focus on the select; without the delay it passed 20 of 20 here, the focus back 5 to 7 ms and the Tab 11 to 17 ms after the choice. Fix in the test (a shared step that waits for the focus on the select; tab order and focus ring kept, three mutations caught); the test passed 5 consecutive whole npm test runs with other programs in front and 5 runs with the window focused; 0 processes left by any run, the cause of the three found not reproduced. Not met: on CI the test is skipped (the job has no engine), so only the new engine-free test runs there.

Gap:

> Missing for done: the CI run of the merge read for run-bar-keyboard.spec.ts (the gallery test, the only part of this work CI runs), and the maintainer's answer to Q1 of ADR 0786 (whether CI builds the engine for the Studio's tests; until then run-control.spec.ts is skipped on CI and the clause "and on CI" cannot be met for it). Also open on the lead's machine: npm test ends with exit 1 by responsiveness.spec.ts:260 and at times f837.spec.ts:198, outside this item.

Follow-up 1 (key `responsiveness-long-task`), deps F-832:

> Studio: responsiveness.spec.ts:260 fails on the lead's machine with one task of 400 to 440 ms when the workspace tab opens. Acceptance: Every limit not stated here is registered in the ADR before the run. The cause of the task of 400 to 440 ms in workspace.tab that apps/studio-web/tests/e2e/responsiveness.spec.ts:260 reports in each of its three passes (5 of 5 whole runs of npm test and 5 runs alone on 2026-11-23 after 10:20, ADR 0786 addendum 1, where the same tree passed at 09:39 with 53 ms) is found and shown by a recorded run; the test then passes a registered number of consecutive runs on that machine. The limits of F-832 are not raised and the test is not skipped or retried.

Follow-up 2 (key `runner-budgets-alert`), deps F-837:

> Studio: f837.spec.ts:198 fails in 2 of 6 whole runs, at two different lines. Acceptance: Every limit not stated here is registered in the ADR before the run. The cause of the two failures of apps/studio-web/tests/e2e/f837.spec.ts:198 recorded in ADR 0786 addendum 1 (the alert "was changed by another writer" not found; two alerts where one is expected) is found and shown by a test that fails without the fix; the test then passes a registered number of consecutive runs on Windows. The test is not skipped, retried or given a longer limit to make it pass.

Follow-up 3 (key `serve-left-behind`), deps F-888:

> Studio: name the test that leaves an rcfd serve process running after npm test on Windows. Acceptance: Every limit not stated here is registered in the ADR before the run. The end-to-end test or the path of the harness that left the rcfd serve processes found on 2026-11-23 (started with --ui and a journal folder under a temporary rcfd-studio-state folder, from a target/release folder; none was left by any run of ADR 0786, a time-out of a test is the candidate not reproduced there) is shown by a recorded run that leaves one, and after the fix a registered number of whole runs of npm test, among them runs with a test made to time out, leave none.

### Checks at the end (G2)

- `cargo test -p rcfd-cli --release --test docs_site`, with this file as it stands: 2 passed.
- `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0786-F-888.json`: exit 0 (F-888 todo to partial, gap
  set, three new items).
- `python scripts/process_report.py --check`: exit 1 with 4 failures, none of them this work's to correct (rule 7 and
  rule 9): section k, "not in a category (add to CATEGORIES): F-888", a line of `scripts/roadmap_progress.py`, which is
  the lead's file and which main has had since 3ba05c7d, after this branch was made; and three generated files that are
  only not current, `docs/adr/index.md`, `docs/roadmap-overview.md` and `CHANGELOG.md`. They are left to the lead.

## The maintainer's answer (2026-11-23)

To Q1, the maintainer answered **(a)**: CI stays as it is. The Studio's tests that need the engine run only in the
lead's push runs; the clause "and on CI" of F-888 is met by the gallery test of `run-bar-keyboard.spec.ts`.

## The lead's reading (2026-11-23), at the merge

The first merge (3fbf5e7e) failed `npm test` in each of three push runs on tests outside this item while `rcfd serve`
processes left behind by earlier runs were alive; it was parked, and after those processes were ended it was merged
again on main at 8119447a and pushed at 0898008a (the report read at 11:23 UTC, 6 minutes of reading, proposed partial,
written partial). In that push run `npm test` failed once (`f837.spec.ts:198`, `project.spec.ts:245`,
`responsiveness.spec.ts:260`) and passed in the second run; both tests of `run-bar-keyboard.spec.ts` and
`run-control.spec.ts:193` passed in both runs, and the committed mutation was caught. One `rcfd serve` process was left
running after the run, as the follow-up serve-left-behind describes; the lead ended it.

Status: **done** by the lead's reading. The gallery test of `run-bar-keyboard.spec.ts` passed in CI on main at 0898008a
on windows-latest and ubuntu-24.04 (run 45224116370; the run-bar test is skipped there, as amendment 1 says), and Q1 is
answered (a). G1 stays unmet on this machine for tests outside this item. The three follow-ups of the proposal are
questions of scope for the maintainer and are not entered.
