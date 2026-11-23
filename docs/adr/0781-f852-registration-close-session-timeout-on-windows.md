# ADR 0781: F-852 registration: the time-out of the Studio test "Close session cannot be undone" on Windows (the candidate causes, the control for each, the test that must fail without the fix, and the number of consecutive runs)

Status: **REGISTERED 2026-11-23, written before any code of F-852 and before any run of its checks.** Before this text no
Studio source or test of F-852 was written, no Studio test was run and nothing was installed or built in
`apps/studio-web`. What was done: reading (below), searches of the source tree, two CI logs fetched with `gh`, and the
build of the link test `docs_site` for this file. Results go in addenda at the end; nothing above them is changed
afterwards.

It answers the brief `docs/adr/briefs/f852-studio-close-session-timeout.md`
([the file](briefs/f852-studio-close-session-timeout.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5, and
[docs/studio-guardrails.md](../studio-guardrails.md). Branch `f852-studio-close-session-timeout` from main e2fe571b,
scratch `w49i-work/f852-studio-close-session-timeout-scratch`.

## What is asked

Roadmap item F-852 (deps F-829), the binding acceptance: "Every limit not stated here is registered in the ADR before the
run. The cause of the time-out of apps/studio-web/tests/e2e/undo.spec.ts:268 (a click that waits 30 s on windows-latest)
is found and shown by a test that fails without the fix; the test then passes a registered number of consecutive runs on
Windows. The test is not skipped, retried or given a longer limit to make it pass."

Clauses: **c1** the cause is found and shown by a test that fails without the fix; **c2** the test at `undo.spec.ts:268`
then passes the registered number of consecutive runs on Windows; **c3** no skip, no retry, no longer limit.

## What was read

- The rules, the guardrails and the brief, in full.
- `apps/studio-web/tests/e2e/undo.spec.ts` and `apps/studio-web/playwright.config.ts`, in full.
- The source the test drives: `src/renderer/settings/SettingsPage.tsx` (in full), `src/renderer/history/UndoControls.tsx`
  (in full), `src/renderer/actions/index.ts` (in full), `src/preload/index.cts` (the service port, lines 150 to 262),
  `src/main/service/bridge.ts` (`PortBridge`, lines 150 to 240), `src/shared/actions/history.ts` (`move`, lines 330 to
  400), `src/renderer/settings/SettingsSurface.tsx` (the loading, lines 95 to 135), all under `apps/studio-web/`.
- `apps/studio-web/tests/support/fake-case-service.ts` and `tests/support/studio.ts`, in full.
- [ADR 0736](0736-f829-proposal-a-project-trash-with-restore-and-compare-and-set-for-settings-set.md) for the history
  (the lines on undo, redo and the events).
- The failed logs of the CI runs 44505925137 (commit 33125336) and 44530255642 (commit 3dba04a3), job studio-web on
  windows-latest, by `gh run view --log-failed`; copies are in the scratch folder under `ci/`.
- `.github/workflows/ci.yml`, the job studio-web (it packages with `--no-engine`).

## Findings of the reading (before any run)

1. **Where the test stops.** Both logs give the same thing: `TimeoutError: locator.click: Timeout 30000ms exceeded`, call
   log `waiting for getByRole('listbox', { name: 'Sections of the case' }).getByRole('option', { name: /^solver\b/ })`,
   at `openSolver` (`undo.spec.ts:66`), called from line 269, the first line of the test at 268. The call log has no line
   "locator resolved to": the option was never in the page. The click that waits is therefore the one on the section
   "solver", not the one on "Close session". The five tests before it passed in 0.35 to 1.3 s each; the whole job took
   1.5 minutes.
2. **What state the test starts from.** The file runs serially on one page. The test before (line 231) ends with: undo of
   "new session" (the session is closed), then Redo, which makes a session again under a new id; it checks the fake
   service, the location hash and the two undo buttons, and nothing of the settings surface. The section list is drawn by
   the surface only when the page counts the session as known, that is when its list of sessions (`session.list`) holds
   the id in the hash (`SettingsPage.tsx`, `known`); otherwise the page says "Session … is not on the service" and has no
   section list.
3. **How the page learns of the session Redo made.** Only by the service's event `created`: its listener calls
   `listSessions()`. (The page's own "New session" lists the sessions itself before it goes to the session; Redo goes
   through the history, which reports the new id by `renamed`, and the page then only sets the hash.)
4. **The listener is replaced when the session changes.** The effect that subscribes to the events has `session` among
   its dependencies. When the hash changes to the new id, React runs the effect's clean-up and the effect again:
   `unsubscribe(old)` and `subscribe(new)`. The preload script removes the old listener at once
   (`eventListeners.delete`), while the main process still holds the old subscription until the request arrives
   (`PortBridge.subs`). An `events` message the main process posted to the old subscription number in that interval finds
   no listener in the page and is dropped without a word (`onPortMessage`: `if (!l) return`). The new subscription does
   not get that batch: it was addressed to the old number.
5. **The two messages race.** The main process holds two connections to the service, one for calls and one for
   `events.poll` (`src/main/service/client.ts`, its head comment). The reply of `session.create` and the batch with
   `created` come by different connections, and the main process posts each to the page as it arrives. If the batch
   reaches the page after the page has replaced its listener, and was posted before the main process saw the
   replacement, it is lost, `listSessions()` is never called, and the page stays on "Session … is not on the service"
   until another event comes. None comes: the test waits for the section list.
6. **The 30 s.** `playwright.config.ts` sets the test limit to 60 s and no limit for an action; the 30 s is the click's
   own limit. Where it is set was not looked for: no limit is changed by this work (c3).
7. **The rate in CI.** `gh run list --workflow CI --created 2026-11-19` gives 18 runs on that day: 14 succeeded, 2 were
   cancelled, 2 failed, and both failures are this test (the two logs read). Rate p0 = 2/16 = 0.125 per completed run.

## Candidate causes and the control that tells them apart

| Id | Candidate | What it predicts at the time-out | Control |
|---|---|---|---|
| H1 | the `created` event of the session Redo made is dropped while the page replaces its events listener (findings 3 to 5) | the page shows "Session … is not on the service" and stays so; the fake service received no `session.list` after the last `session.create` | R1, R2: the events delayed in transit make it happen every time; R3: the same hook without delay does not |
| H2 | the session list is read but is stale (read before the service had the session, or answered out of order) | the same heading, but the fake service did receive a `session.list` after the last `session.create` | R2 reads the fake service's record of calls |
| H3 | the session is known but the surface has no section "solver" (a failed `settings.get`, the schema not read) | no heading "is not on the service"; the surface or its error is shown | R2 asserts the heading; a failure of R2 at that line with the surface present points here |
| H4 | nothing is lost, the machine is only slow | the state still moves at the time-out and the list arrives later | R2 asserts that nothing changes for 2 s and that no call followed; R0 under this machine's load |

The reading favours H1; the runs decide. If R1 passes on the unfixed code, H1 is not shown, the cause is recorded as not
found by these controls, and nothing is fixed on a guess.

## Design registered

**The delay (the control and the committed test).** The main process is made to deliver every `events` message late by a
fixed time, from the test, through Playwright's evaluation in Electron's main process: the `postMessage` of the main
side's message port is wrapped so that a message whose `type` is `events` is posted after D milliseconds and every other
message at once. No source file is changed for it. D = 300 ms, derived, not searched: it must exceed the time from the
reply of `session.create` to the replaced listener in the page (one render of the page; the neighbouring tests complete
whole dialogues in 350 to 1300 ms, a single render is some tens of milliseconds at most) and stay well under the 5 s an
expectation waits; 300 ms is about ten times the first and a seventeenth of the second. A late event is what a loaded
machine produces; the hook only makes the lateness certain. If the port's method cannot be wrapped this way, the
mechanism is replaced by an amendment committed before the run.

**The committed test** (c1): a new spec `apps/studio-web/tests/e2e/undo-session-events.spec.ts`, against the fake
service: a new session from defaults; undo of it, confirmed; the delay installed; Redo; then the page must show the new
session's settings (the heading "Set in this case" and the option "solver" of the section list) within the standard 5 s
of an expectation, and the fake service must have received a `session.list` after the last `session.create`. It is not
skipped and has no retry and no limit of its own.

**The fix registered for H1** (made only if R1 and R2 show H1): the page's events subscription no longer depends on the
session shown. The listener reads the session through a reference, the subscription is made once per connection and
instance of the service, and a change of session replaces no listener, so no interval exists in which an event has no
listener. From first principles: an event stream "from now on" may be re-subscribed only if what the gap can hold is read
again afterwards; not re-subscribing removes the gap. One file, `src/renderer/settings/SettingsPage.tsx`; no change of an
interface, of the preload script or of the main process; no new dependency (G5); the actions and their names are
unchanged (G16, G20); nothing is added to the screen (G22). Not covered by this fix: the same pattern in other pages, if
any has it (see "What is not covered").

**The load.** The runs are made on this machine while three other agents work on it. At the start and the end of each
series the processor load (`Win32_Processor.LoadPercentage`) and the number of `cargo`, `rustc` and `node` processes are
recorded in the addendum.

## Checks

The order is the order of the runs. R0 to R3 run on the unfixed source (built once), R4 to R6 on the fixed source. Each
is made once. A "run" of a spec is one `npx playwright test <file> --project=electron` on the packaged Studio.

| Id | Clause | Check | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| R0 | c1 | unfixed: `undo.spec.ts` as it is, 30 consecutive runs under the machine's load | none: the number of runs that fail at line 66 is recorded; 0 decides nothing | acceptance of F-852 (reproduce) | 0 to 4 failures | none | code verification |
| R1 | c1 | unfixed: the committed test with D = 300 ms | fails at the expectation of the heading "Set in this case" | acceptance of F-852 (a test that fails without the fix) | fails | exact | code verification |
| R2 | c1 | unfixed: the scratch variant of R1 that asserts H1's state: the heading "Session … is not on the service", no `session.list` after the last `session.create`, both unchanged after 2 s | passes | this registration (the table of candidates) | passes | exact | code verification |
| R3 | c1 | unfixed, negative control: R1's test with the hook installed and no message delayed | passes | this registration | passes | exact | code verification |
| R4 | c1 | fixed: the committed test with D = 300 ms, 30 consecutive runs | 30 of 30 pass | acceptance of F-852 | 30 | exact | code verification |
| R5 | c2 | fixed: `undo.spec.ts` unedited, 30 consecutive runs, the test at line 268 among them | 30 of 30 pass, every test of the file | acceptance of F-852; N from finding 7 | 30 | exact | code verification |
| R6 | c2 | fixed: `npm test` in `apps/studio-web`, once, with the engine built in the scratch target folder | the specs of the settings page and of undo pass; any other failure is reported with its cause, not retried | rule 7 | pass | exact | code verification |
| R7 | c3 | `git diff` of `playwright.config.ts` and of `undo.spec.ts` against main; a search of the new spec for skip, fixme, retries and a time-out | no difference; none found | acceptance of F-852 | none | exact | code verification |

**N = 30.** With the CI rate p0 = 0.125 (finding 7), 30 runs without a failure have the probability (1 − 0.125)^30 =
e^(30 × ln 0.875) = e^(−4.006) = 0.018 if the rate were unchanged. The rate on this machine may be lower than in CI, so
R5 alone would be weak evidence; R1 and R4 carry the cause, R5 carries the clause c2 as the acceptance words it.

Planned mutations (rule 16), on the fixed source: **M1** the subscription depends on the session again (the fix
reverted): R1's test must fail. **M2** the test's own expectation of `session.list` removed, with M1: M1 must still be
caught, by the heading. The committed one: M1, as
`apps/studio-web/tests/e2e/undo-session-events.spec.mutation.patch`. Negative control: R3, and R3's control once more on
the fixed source (passes).

Size (agent): done; 0; 1; 0; 140000

## Decision rule

**done** if the cause is shown by a test that fails without the fix (R1 fails on the unfixed source, R2 passes, R3
passes, R4 passes 30 of 30, M1 is caught) and R5 passes 30 of 30 with R7 clean. **partial** if the cause is shown but not
fixed, or the time-out cannot be reproduced by the registered controls (R1 passes on the unfixed source, or R2 fails), or
R4 or R5 has a failure. A failure in R6 outside the settings page and undo, caused by the load or by a file the
operating system refused, is reported and does not change the outcome; one inside them makes it partial.

## What is not covered

- The CI job itself: the runs are made on this Windows machine, not on windows-latest. The first CI runs after the merge
  are the lead's to read.
- Other pages that subscribe to the service's events (`CaseEditorPage.tsx`, `useRunFeed.ts`, `PlotsDev.tsx`). They are
  read after the fix and reported; none is changed under this item unless the same test shows the same loss there, which
  would be registered by an amendment first.
- An event lost between the page's first `session.list` and its first subscription at connection: the same kind of gap,
  not the one of this time-out, not observed; reported if the reading confirms it.
- Why the two CI failures fell on one day and on Windows only: H1 is a race whose odds depend on the machine; no claim
  about the runner is made.
- The artefacts of the two CI runs (the page snapshot `error-context.md`) were not downloaded.

## Open questions for the maintainer

None at registration.
