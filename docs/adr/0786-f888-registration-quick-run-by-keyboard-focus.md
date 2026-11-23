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
