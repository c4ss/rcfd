# Brief: F-852, Studio: the end-to-end test "Close session cannot be undone" times out on Windows in CI

Branch: f852-studio-close-session-timeout
Items: F-852
ADR: 0781
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 65459

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f852-studio-close-session-timeout`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f852-studio-close-session-timeout-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 14395 (the rules of Studio work)
- apps/studio-web/tests/e2e/undo.spec.ts 17208 (the test at line 268 and the specs around it)
- apps/studio-web/playwright.config.ts 950 (the limits, the workers and the projects)
- docs/adr/0736-f829-proposal-a-project-trash-with-restore-and-compare-and-set-for-settings-set.md 19915 (what closing a session and the history do)

Also in the repository, as needed: the Studio source under `apps/studio-web/src/` that the test drives (find it from the test's locators), `.github/workflows/` for the job studio-web on windows-latest, the CI runs on 33125336 and 3dba04a3 (`gh run list` and `gh run view --log-failed` for their logs), and `docs/adr/0801-studio-web-dependencies-and-layout.md`.

## What the maintainer decided (2026-11-23)

The test "Close session cannot be undone" (`undo.spec.ts:268`) timed out twice on main in one day in the job studio-web on windows-latest, a click that waited 30 s, while the Ubuntu job and the runs before and after were green and neither commit changed the Studio. The sentence this work must be able to write: the cause is found and shown by a test that fails without the fix, and the test then passes a registered number of consecutive runs on Windows, with no skip, no retry and no longer limit.

## Task

One registration, `docs/adr/0781-f852-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run** the candidate causes you will test, the control that tells them apart for
   each, the number of consecutive Windows runs the test must pass after the fix, and the test that fails without the
   fix. This machine runs Windows: the runs are made here, under the load of the other agents, which you record.
2. **Reproduce** the time-out by the registered controls, find the cause, and fix it in the Studio source or the test's
   setup as the cause requires; then run the registered number of consecutive runs.
3. Run `npm test` and the end-to-end specs the fix touches in `apps/studio-web`.

Decision rule to register: done if the cause is shown by a test that fails without the fix and the registered runs pass; partial if the cause is shown but not fixed, or the time-out cannot be reproduced by the registered controls.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `apps/studio-web/` (the source the cause lies in, the spec and new specs). No crate source, no golden, no CI workflow file without a registered reason.

## Common to this work

The acceptance of F-852 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. This item runs no solve. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0781-F-852.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
