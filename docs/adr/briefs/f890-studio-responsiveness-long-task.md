# Brief: F-890, the long task when the workspace tab opens that fails responsiveness.spec.ts:260 on the lead's machine

Branch: f890-studio-responsiveness-long-task
Items: F-890
ADR: 0787
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 99538

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f890-studio-responsiveness-long-task`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f890-studio-responsiveness-long-task-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 14395 (the rules of Studio work)
- docs/adr/0786-f888-registration-quick-run-by-keyboard-focus.md 45842 (addendum 1, 'The processes' and the follow-ups: what was recorded on 2026-11-23)
- apps/studio-web/tests/support/studio.ts 6188 (launchStudio, how the Electron window and its engine are started)
- apps/studio-web/tests/support/rcfd.ts 4571 (alive and killPid, how the tests end the processes they start)
- apps/studio-web/tests/e2e/responsiveness.spec.ts 15551 (the test at line 260 and its limits LONG_TASK_LIMIT_MS and LONG_FRAME_LIMIT_MS)

Also in the repository, as needed: the registration of F-832 (the responsiveness budget; find it with `grep -ln F-832 docs/adr/*.md`), the workspace tab's source under `apps/studio-web/src/` (find `workspace.tab`), and the lead's logs of the failing runs, read only: `E:/projects/cfd/w49i-work/push-f888-2.log` and `E:/projects/cfd/w49i-work/push-f861d.log` (search for `failed`, `✘` and the test's file name).

## What the maintainer decided (2026-11-23)

The maintainer answered on 2026-11-23 "Yes, issue all three" (F-890, F-891, F-892, the follow-ups of ADR 0786) and "Yes, three over": these three agents run beyond the limit of 4 at once, for these items alone. They hold the merges of F-861, F-852, F-451 and F-339: on 2026-11-23 the lead's push runs of F-888 and F-861 failed `npm test` in `apps/studio-web` on a different set of Studio end-to-end tests in each run (`f837.spec.ts:198`, `project.spec.ts:245`, `responsiveness.spec.ts:260`, `run-control.spec.ts:264` and `:320`), changes that cannot have caused them, and each run left one `rcfd serve` process running. Two other agents work on the Studio's tests at the same time (F-890 the long task, F-891 the runner-budgets alert, F-892 the process left behind); each keeps to its own files and names in its report any shared helper it had to change. Use your own `CARGO_TARGET_DIR` in your scratch folder for the engine binaries the Studio packages, so a process held by another run does not lock yours, and end only processes you started. The sentence this work must be able to write: the task of 400 to 440 ms has a named cause, shown by a recorded run before the fix; after the fix the test passes the registered number of consecutive runs on this machine with the limits of F-832 unchanged.

## Task

One registration, `docs/adr/0787-f890-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** the hypotheses of the cause (at least: work done on the main thread when the
   workspace tab mounts; a first compile of a shader or a pipeline; garbage collection or a load on the machine), the
   recorded run that tells them apart (a performance trace of the long task, with its call stack) and fails without the
   fix, the number of consecutive runs of the test alone and of whole runs of `npm test` after it, and a committed
   mutation that a registered test must catch.
2. **Fix the cause in the Studio** (or the test, if the cause is in it). The limits of F-832 are not raised, and the test
   is not skipped, retried or given a longer limit.
3. Run the registered checks, the Studio's type checks and `npm test` in `apps/studio-web`; report every other test that
   failed in those runs by name, without fixing it.

Decision rule to register: done if the cause is shown by a recorded run, the test then passes the registered number of consecutive runs with the limits of F-832 unchanged, and the mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `apps/studio-web/src/` where the cause is, `apps/studio-web/tests/e2e/responsiveness.spec.ts` only if the cause is in the test. No golden.

## Common to this work

The acceptance of F-890 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0787-F-890.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
