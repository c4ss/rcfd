# ADR 0788: F-891 registration: the runner-budgets test of f837.spec.ts:198 that fails at two lines (the hypotheses of the cause, the records that tell them apart, the tests that must fail without the fix, the consecutive runs)

Status: **REGISTERED 2026-11-23, written before any code of F-891 and before any run of its checks.** Before this text no
Studio source or test of F-891 was written and no Studio test was run. What was done: reading (below), searches of the
source tree, and two preparations that run no check, started while this text was written: the release build of
`rcfd-cli`, `rcfd-mcp`, `rcfd-runner` and the example `f447_bench` into the scratch folder's own target folder, and
`npm ci` in `apps/studio-web`. Results go in addenda at the end; nothing above them is changed afterwards.

It answers the brief `docs/adr/briefs/f891-studio-runner-budgets-alert.md`
([the file](briefs/f891-studio-runner-budgets-alert.md)) and follows
[docs/process/agent-rules.md](../process/agent-rules.md), version 5, and
[docs/studio-guardrails.md](../studio-guardrails.md). Branch `f891-studio-runner-budgets-alert` from main c4bd79f3,
scratch `w49i-work/f891-studio-runner-budgets-alert-scratch`. Three agents beyond the limit of 4 at once were allowed by
the maintainer on 2026-11-23 ("Yes, three over") for F-890, F-891 and F-892 alone; that is the lead's record in the brief,
repeated here.

## What is asked

Roadmap item F-891 (deps F-837), the binding acceptance: "Every limit not stated here is registered in the ADR before
the run. The cause of the two failures of apps/studio-web/tests/e2e/f837.spec.ts:198 recorded in ADR 0786 addendum 1
(the alert "was changed by another writer" not found; two alerts where one is expected) is found and shown by a test
that fails without the fix; the test then passes a registered number of consecutive runs on Windows. The test is not
skipped, retried or given a longer limit to make it pass."

Clauses: **c1** the cause of the first failure (the alert not found, line 223) is found and shown by a test that fails
without the fix; **c2** the same for the second failure (two alerts, line 242); **c3** the test passes the registered
number of consecutive runs on Windows; **c4** no skip, no retry, no longer limit, and (the brief) what the test checks,
the refusal over another writer's change and the undo, is kept.

## What was read

- The rules, the guardrails and the brief, in full.
- [ADR 0786](0786-f888-registration-quick-run-by-keyboard-focus.md): the registration, and addendum 1 from "The cause"
  to the end, with the lead's reading and the maintainer's answers.
- `apps/studio-web/tests/e2e/f837.spec.ts`, `tests/support/studio.ts`, `tests/support/rcfd.ts`,
  `tests/support/project.ts` and `tests/support/launch.ts` in full; `playwright.config.ts`; the scripts of `package.json`.
- The source the test drives: `src/renderer/runs/RunExtras.tsx` in full (`BudgetsPanel`, lines 218 to 390),
  `src/renderer/runs/RunsView.tsx` lines 60 to 95 (where the panel gets `fileChange`), `src/main/run/extras.ts` in full
  (`RunnerBudgetsEngine`), `src/main/project/session.ts` lines 280 to 400 (the poll of the change feed),
  `src/shared/project.ts` (`ChangeEntry`, `FEED_POLL_MS`), `src/renderer/project/activity.ts` (the head).
- The engine, read to know what the feed reports, not changed: `crates/rcfd-workspace/src/feed.rs` (the module text,
  `journal_entry`, `Feed::open`, the head of `Feed::poll`), `crates/rcfd-workspace/src/journal.rs` (`Reader`),
  `crates/rcfd-workspace/src/runner_config.rs` (`set_runner_config`), and the head of
  [ADR 0733](0733-f837-proposal-set-runner-config-the-runner-writes-its-own-budgets.md).
- The lead's logs: `w49i-work/push-f888-2.log` (the failure at lines 1172 to 1197) and `w49i-work/push-f861d.log` (the
  failure at line 9127, the passes at 9936 and 10924); and, read only, the log of the second failure in the scratch
  folder of ADR 0786 (`n1-5.log`, lines 963 to 989).

## Findings of the reading (before any run)

1. **The two recorded errors.** First failure (the lead's two logs and P1 of ADR 0786), at line 223 after 5.5 s:
   `getByRole('region', { name: 'Runner budgets' }).getByRole('alert')`, "element(s) not found". Second failure (run 5
   of N1 of ADR 0786), at line 242: a strict mode violation, the locator resolved to 2 elements, the alert "The runner
   would refuse this file: unknown field …" and a second `<p role="alert" data-note="conflict">`.
2. **How the panel hears of a change.** `RunsView` gives the panel `fileChange`, the number of the newest entry of the
   project's change feed whose path is `.rcfd/runner-config.json`. When that number moves while the panel is open, the
   panel's effect does one of two things: with nothing typed it reads the file again (`load`), with values typed it shows
   the note "… was changed by another writer; the values typed here are kept. Read it again to see its values."
   (`role="alert"`, `data-note="conflict"`). "Typed" is `typedRef.current`, set at each render to `changed`, which is
   whether `changes()` has a key.
3. **What moves that number.** The engine's feed does not watch `.rcfd/` (`feed.rs`, the module text: "`.rcfd/` is not
   watched"). The test's own `writeFile` on the budgets file therefore gives no feed entry. What does give one is a line
   of the project journal `.rcfd/edits.jsonl` that names the path: `set_runner_config` writes such a line for every
   accepted write (`runner_config.rs`, ADR 0733 P1), and the feed reports each completed journal line as an entry with
   its `path` (`feed.rs`, `journal_entry` and `Feed::poll`). So the entries the panel hears of in this test are the
   journal lines of **this window's own writes**: the Write at line 207, the "Undo anyway" at line 233 and the "Replace
   the file" at line 245.
4. **When it moves.** The main process asks the feed every `FEED_POLL_MS` = 400 ms (`src/shared/project.ts:79`,
   `session.ts`, `schedule`). An entry of a write therefore reaches the panel between 0 and about 400 ms after the
   write, at a time that has nothing to do with the test's steps.
5. **The first failure, as the reading expects it.** After the Write of line 207 the test reads two files, takes a
   screenshot, and at line 219 writes `{"max_runs": 3}` as the other writer. If the entry of the Write of line 207
   reaches the panel after line 219 and before the value typed at line 220 is committed (a number field commits on
   blur, the Tab of line 221), nothing is typed, the panel reads the file again and now holds the other writer's file
   and its hash. The Write of line 222 then has nothing to refuse: the engine writes, the panel shows the status
   "Written to …", and no alert ever comes. That is "element(s) not found" after the 5 s of the assertion.
6. **The second failure, as the reading expects it.** After "Undo anyway" (line 233) the test closes the panel, writes
   `{"gpu": 1}` and opens the panel again (lines 237 to 241). A new panel takes the feed's number as it is when it
   mounts. If the entry of the undo reaches it after that, the number moves under a panel that already shows the file.
   For a file the runner would refuse (`needsReplace`), `changes()` lists every key whether or not a value was typed,
   so `changed` is true and the panel takes the second way of finding 2: the conflict note, beside the alert of the
   refused file. Two alerts, the second with `data-note="conflict"`, as recorded.
7. **What the Studio says in the second failure is not true.** The note says another writer changed the file and that
   typed values are kept. The entry was the journal line of this window's own undo, the file is as the panel read it,
   and nothing was typed. So the second failure has its cause in the Studio's source; the first has its cause in the
   order of the test's steps (the Studio's read after the other writer's change is what
   [ADR 0801](0801-studio-web-dependencies-and-layout.md) addendum 23 asks of an open panel with nothing typed).

Arithmetic done beforehand: with one poll every 400 ms and an entry that is still to come, the chance that it lands in
a window of w ms is about w/400. The window of the first failure is the time from the `writeFile` of line 219 to the
commit of the typed value at line 221 (a fill and a key press through Playwright; tens of ms, more under load): for w
of 40 to 130 ms, 10 % to 33 %. The lead's logs have it failing in 2 of 4 whole runs and ADR 0786 in 1 of 6. No limit is
set on this rate.

## Hypotheses of the cause and what tells them apart

| Id | Hypothesis | What it predicts |
|---|---|---|
| H1 | A race between the test's second writer and the form's own read (findings 5 and 6): a feed entry of this window's own earlier write reaches the panel at a time the test does not control | Failure at 223: an entry about the file is received by the Studio between the write of line 219 and the commit of the typed value, the form shows `max_runs` 3 before the Write, and the Write is answered with the status "Written to". Failure at 242: an entry is received after the reopened panel has read the file; the second alert is created after that entry, under the reopened panel |
| H2 | An alert left from an earlier step | The second alert of line 242 exists before the panel is reopened or is created with it, with no feed entry between; at 223 a note of an earlier step hides or replaces the expected one |
| H3 | A timing of the runner's reply: the reply to the Write of line 222 comes after the 5 s of the assertion, or the replies of two requests cross | At 223 no note at all appears within 5 s and the expected alert appears later; the times do not depend on when the feed entry is received |
| H4 | None of these | The records fit no row above |

The records that tell them apart are made on the unchanged source and test of main c4bd79f3, in the packaged Studio
built from it, with the engine of this work's own build. Each is a run of
`npx playwright test tests/e2e/f837.spec.ts -g "the runner’s budgets" --project=electron` (the file's `beforeAll` and
this one test).

- **D1, the unchanged test,** 10 runs in a row; the failures and their lines are counted. No limit.
- **D2, the instrumented record,** 20 runs: the same test with instrumentation only (a scratch patch of the spec,
  applied for the runs and removed, kept in the scratch folder, not committed). It records the test's clock at each
  step of lines 207 to 245; from the main process's project state the feed entries about the budgets file with the
  time the Studio received each (`seen`); and in the page, by a mutation observer on the panel's region, the time and
  text of every note and alert that appears or goes and of every change of the `max_runs` field. The assertions are
  unchanged.
- **D3, the forced orders,** 5 runs each, by scratch patches that move no assertion and only wait:
  (a) the other writer's write of line 219 is made directly after the status of line 208, before the entry of the
  Write of line 207 has been received (whether it had been received is read from the project state and recorded as the
  premise of the run), and the test then waits until the form shows `max_runs` 3 before it goes on with lines 220 to
  223 unchanged;
  (b) before line 219 the test waits until the Studio has received the entry of the Write of line 207, and 500 ms more;
  (c) after "Undo anyway" the test closes the panel, writes `{"gpu": 1}` and opens the panel at once, records as the
  premise that the entry of the undo had not been received when the refused-file alert showed, waits until it has been
  received and 500 ms more, and then makes the assertion of line 242 unchanged;
  (d) before the Escape of line 237 the test waits until the Studio has received the entry of the undo, and 500 ms more.

Reading of the records. **H1 is shown** if (a) fails at line 223 with "element(s) not found" in every run whose
premise held, (b) passes line 223 in 5 of 5, (c) fails at line 242 with two alerts, the second `data-note="conflict"`,
in every run whose premise held, (d) passes line 242 in 5 of 5, and every failure of D1 and D2, if there is one, has
its entry in the window H1 names. A run of (a) or (c) whose premise did not hold decides nothing, is recorded as such
and is made again, up to 10 runs for 5 with the premise; fewer than 5 is recorded as not deciding. **H2 is shown** if
the second alert of a failure at 242 is in the observer's record before the panel was reopened or without an entry
before it. **H3 is shown** if a failure at 223 has no note within 5 s, or if (a) passes. Any other pattern is H4 and
is recorded as not deciding; an amendment then registers the next record before it is made. If D1 and D2 do not fail
in any run, the cause rests on D3 and on the agreement of its error texts and lines with the lead's logs, and the
addendum says so in those words.

## Design registered

If H1 is shown, the fix has two parts, one for each failure, as the brief asks ("Fix the test or the Studio, whichever
the cause names"). If another hypothesis is shown, an amendment registers its fix and its test before either is
written.

**Part 1, the Studio (the second failure).** In `BudgetsPanel` of `src/renderer/runs/RunExtras.tsx`, with its
decisions in a new module without React beside it (`src/renderer/runs/budgetsFeed.ts`) so that a unit test checks them:

- A feed entry about the file is a reason to look, not a change. When the number moves, the panel reads the file
  through the engine (`runner_config`, as now) and compares the hash it gets with the hash it holds. The same hash:
  nothing is said and nothing is changed (the entry was this window's own write, or a change the panel has read
  already). Another hash and nothing typed: the values read are shown, as now. Another hash and values typed: the
  note of finding 2, as now, and the typed values are kept. Whether values are typed is asked when the read has come
  back, not when it was sent. Derivation: the engine's own test of "changed since it was read" is the hash
  (ADR 0733 P1: `base_hash`, conflict 1200); the panel uses the same test, so what it says agrees with what a Write
  would be answered.
- "Typed" is that a field's value differs from the value read from the file, for the same eight keys. For a file the
  runner accepts this is what `changed` is today. For a file the runner would refuse it is no longer always true. The
  button "Replace the file" stays enabled as today.
- The panel's region carries `data-file-change`, the number of the feed entry it has finished with (at mount: the
  number it mounted with). No visible word is added (G22); the attribute is what lets a test wait for the panel
  without waiting for time (G13).

Nothing else of the Studio is changed: not the main process, not the feed's poll, not the engine. No guardrail is
touched in what it forbids; G18 is served (the panel no longer names a conflict that is not one) and G6 (it says
nothing untrue).

**Part 2, the test (the first failure).** `f837.spec.ts:198`: the value of `cpu_threads` is typed and committed
before the other writer writes the file, in place of after it. Lines 219 to 222 become: fill, Tab, the Write button
enabled (the typed value is in the form), the other writer's `writeFile`, the click on Write. The Studio's read can
then no longer come between the other writer's change and a form without typed values. Every assertion is kept with
its default limit: the alert "was changed by another writer since it was read: nothing was written", the file
unchanged, Read again and `max_runs` 3, the undo that asks first, Keep it, Undo anyway, the status "is as it was", the
file `{}`, the refused file and its replacement. Added, none removed: after the panel is reopened at line 241 the
test waits until the panel has finished with the feed entry of the undo (`data-file-change`) and then makes the
assertion of line 242, so the state the second failure met by chance is met in every run. No retry, no `test.skip`,
no `test.slow`, no longer time-out. This edits an existing test in place; rule 2 says existing tests are not edited,
the brief asks for exactly this edit and gives the file to this work, and the addendum names it as such.

**The tests that fail without the fix.**

- **U1**, new, `apps/studio-web/tests/unit/f891.spec.ts` (F-891, ADR 0788; it needs no engine and so runs on CI): the
  decisions of `budgetsFeed.ts`. The same hash, typed or not: nothing. Another hash, nothing typed: the values read.
  Another hash, typed: the note. Nothing held yet: the values read. Typed: false for a refused file with every field
  as read, true once one field differs; for an accepted file equal to "a key would be written".
- **E1**, new, `apps/studio-web/tests/e2e/f891.spec.ts` (F-891, ADR 0788), in the packaged Studio on a copy of the
  quick-start project, set up as `f837.spec.ts` is. A feed entry is produced when the test chooses, by appending to
  the copy's `.rcfd/edits.jsonl` a copy of the journal line of this window's own write (a fixture, as `f837.spec.ts`
  appends a line to `events.jsonl`); the test then waits until the project state has the new entry and the panel's
  `data-file-change` has its number, and only then asserts. Four tests:
  (a) values typed, then an entry that leaves the file as the panel holds it: no alert, the typed value kept, Write
  still enabled;
  (b) the sequence of the second failure: a refused file, the panel opened, then an entry: exactly one alert, the
  refused file's;
  (c) a refused file, nothing typed, the file changed by another writer with an entry: exactly one alert, and the
  note about typed values is absent;
  (d) the first failure's mechanism and its counterpart: nothing typed, the file changed with an entry: the form shows
  the other writer's value and no alert, and a Write of a value typed afterwards is written; values typed, the file
  changed with an entry: the note "the values typed here are kept", and the Write is refused with "since it was read:
  nothing was written".
- **N3**, the fixed `f837.spec.ts:198` under the order that failed: by a scratch patch that only waits, the entry of
  the Write of line 207 is made to arrive after the other writer's write (the premise recorded as in D3 (a)) and the
  test waits for `data-file-change` before it clicks Write.

"Fails without the fix" is shown three ways: D3 (a) and (c) on the unchanged tree; the committed mutation M1, which is
part 1's hash test removed; and M2 and M4 below.

## Checks

Kinds as rule 14 names them; none of these is a validation against a measured case, and the acceptance claims none.
`npm test` builds and packages the Studio and runs every Playwright test; the engine it uses is this work's own build
(`CARGO_TARGET_DIR` in the scratch folder).

| Check | Clause | Limit | Source of the limit | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|
| C1: the cause shown by D1, D2 and D3 as read above (one of H1, H2, H3) | c1, c2 | one hypothesis shown, by the reading registered above | acceptance of F-891 | H1 | not a number | code verification |
| U1: the unit test of the panel's decisions passes on the fixed tree | c2 | pass | acceptance of F-891 | pass | not a number | code verification |
| E1: the four tests of the new spec pass on the fixed tree | c1, c2 | pass | acceptance of F-891 | pass | not a number | code verification |
| M1 (committed): U1 and E1 (a) fail with the hash test removed | c2 | fail | acceptance of F-891 ("fails without the fix") | fail | not a number | code verification |
| N1: consecutive runs of the fixed test alone (the command of D1) in which it passes | c3 | 20 of 20 | the brief (a registered number); 20 runs at the 10 % of the arithmetic above would pass by chance in 12 % of cases, at 33 % in 0.03 % | 20 of 20 | 1 | code verification |
| N2: consecutive whole runs of `npm test` on the fixed tree in which `f837.spec.ts:198`, U1 and E1 pass | c3 | 5 of 5 | the brief (a registered number); the number of ADR 0786 | 5 of 5 | 1 | code verification |
| N3: the fixed test under the forced order, runs whose premise held | c1 | 5 of 5 | acceptance of F-891 | 5 of 5 | 1 | code verification |
| K1: the diff of `f837.spec.ts` removes no assertion, adds no `retries`, `skip`, `fixme`, `slow` or time-out, and `playwright.config.ts` keeps `retries: 0` | c4 | 0 such lines | acceptance of F-891 | 0 | not a number | code verification |
| G1: `npm run typecheck` and the whole `npm test` (the runs of N2) | rule 7 | exit 0 | agent rules, rule 7 | exit 0 | not a number | unchanged default output |
| G2: `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check` | rule 7 | pass | agent rules, rule 7 | pass | not a number | unchanged default output |

Before the fix one whole `npm test` is run on the unchanged tree (B0), for the list of tests that fail on this machine
today without this work; it has no limit. No limit is registered on wall-clock time. A failure of another test in a
run of N2 is recorded with its test and its error and reported by name, not fixed; it does not count against N2,
whose subject is `f837.spec.ts:198`, U1 and E1, but G1 then fails for that run and the addendum says so. A test that
fails on a file the operating system refuses under the load of the other agents is reported, not retried until it
passes. Each registered run is made once. Processes: this work ends only processes it started, and lists the `rcfd`
processes of its own target folder before and after the whole runs.

**Negative controls and mutations (rule 16).** D3 (a) and (c) are the negative controls: the unfixed sequences under
the forced order. Planned mutations, each applied to the worktree, run and removed:

- M1 (committed, `apps/studio-web/tests/unit/f891.spec.mutation.patch`; the command of its first line runs U1): in
  `budgetsFeed.ts` the same hash no longer means "nothing". U1 must fail; E1 (a) must fail.
- M2: "typed" as before the fix (true for every refused file). U1 and E1 (c) must fail.
- M3: in `BudgetsPanel` a conflict answered to a Write is shown as a written status. `f837.spec.ts:198` must fail at
  the alert of the refusal (the fixed test still checks the refusal, c4).
- M4: the fixed `f837.spec.ts` with the old order of lines 219 to 222 under the forced order of N3. It must fail at
  the alert of the refusal (part 2 is what removes the first failure).
- M5: in `RunnerBudgetsEngine.undo` the forced undo no longer reads the file's hash (it sends the hash the last write
  left). `f837.spec.ts:198` must fail at the status "is as it was" (the fixed test still checks the undo, c4).

Size (agent): done; 0; 6; 0; 140000

The heavy runs are the whole `npm test`, 11 to 16 minutes each in the lead's logs: one before the fix (B0) and five
after (N2). They are run one at a time.

## Decision rule

As the brief gives it: **done** if the cause is shown by a test that fails without the fix (C1 with H1 for both
failures; U1 and E1 pass and M1 is caught; N3 passes and M4 fails), the test then passes the registered numbers of
consecutive runs (N1, N2) and keeps its checks (K1, M3, M5). **Partial** otherwise, and in particular if the records
give H4 or show the cause of only one of the two failures, if a run of N1 or N2 fails at `f837.spec.ts:198`, or if a
mutation is not caught. **Todo** if no record could be made. G1 failing by tests outside this item's files does not
by itself make the item partial; it is reported, as ADR 0786 did.

## What is not covered

- **CI.** `f837.spec.ts` and E1 need the engine, which CI's job does not build (ADR 0786 amendment 1; the maintainer's
  answer (a) of 2026-11-23 keeps CI as it is). On CI only U1 runs. The acceptance asks for Windows, which is this
  machine.
- The feed's poll and the engine: unchanged. An entry of this window's own write still reaches the panel up to about
  400 ms after the write; after the fix it changes nothing there.
- What a second press of Write does after a refusal, with the form still showing its values of before the other
  writer's change: unchanged and not examined here.
- The other tests of the suite: a failure seen in the runs is reported by name, not fixed (the brief).
- macOS and Linux.

## Open questions for the maintainer

None at registration.
