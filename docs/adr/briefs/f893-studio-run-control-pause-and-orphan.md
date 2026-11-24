# Brief: F-893, run-control.spec.ts:320, which misses the pause request in some whole runs, and the rcfd run solver a run-control test leaves running

Branch: f893-studio-run-control-pause-and-orphan
Items: F-893
ADR: 0790
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-24
Size (lead): done; 0; 0; 0; 103419

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f893-studio-run-control-pause-and-orphan`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f893-studio-run-control-pause-and-orphan-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 14395 (the rules of Studio work)
- docs/adr/0789-f892-registration-serve-left-behind.md 37744 (addendum 1 and the follow-up run-control-pause-request: the failures of :320 recorded, and how F-892 counted processes per spec file)
- apps/studio-web/tests/e2e/run-control.spec.ts 26620 (the test at line 320, the LONG overrides at line 37, the temporary rcfd-f807- folder at line 159 and the teardown)
- apps/studio-web/tests/support/rcfd.ts 4571 (alive and killPid, how the tests end the processes they start)
- apps/studio-web/tests/support/studio.ts 7098 (launchStudio, how the Electron window and its engine are started)

Also in the repository, as needed: the runner's start and end of an `rcfd run` child (find where `--pause-file` and `--stop-file` are passed, under `crates/` and `apps/studio-web/src/`), `crates/rcfd-cli` where `rcfd run` reads its pause and stop files, and the lead's logs of runs where :320 failed, read only: `E:/projects/cfd/w49i-work/push-f861d.log` and the push log of F-890 to F-892 under `E:/projects/cfd/w49i-work/` (search for `run-control`).

## What the maintainer decided (2026-11-24)

The maintainer answered on 2026-11-24 "Yes, issue it": the follow-up run-control-pause-request of ADR 0789 and the orphaned solver, as one item. On 2026-11-24 the lead found an `rcfd.exe run` process left running with `--set solver.max_iterations=1000000 --set solver.tolerance_u=1e-14 --set solver.tolerance_p=1e-14`, a `--case-dir` and `--pause-file`/`--stop-file` under `%TEMP%/rcfd-f807-<random>/quick-start/runs/r-<time>/`, its parent gone; it held `target/release/rcfd.exe` so the next build failed, and the lead ended it. F-892 made a quit end an unconnected `rcfd serve` and the teardown sweep leftover serve processes; it did not cover `rcfd run`. Use your own `CARGO_TARGET_DIR` in your scratch folder for the engine binaries the Studio packages, and end only processes you started. The sentence this work must be able to write: the cause of the :320 failure and the path that leaves the solver are each shown by a recorded run before the fix; after it, :320 passes the registered number of consecutive runs and the registered whole runs leave no `rcfd run` process.

## Task

One registration, `docs/adr/0790-f893-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** the hypotheses of each cause. For :320 at least: the pause file written before
   the solver watches it; the status line of the pause lost or not flushed in the feed; a load on the machine past the 5 s
   wait. For the solver at least: a test that fails or times out before its teardown with a run in flight; a Studio or
   runner that quits without ending its `rcfd run` child; a process tree ended without its children on Windows. Register
   the recorded runs that tell them apart (for the solver, count the `rcfd` processes with their command lines and parent
   ids before and after each spec file), the number of consecutive runs of :320 alone and of whole runs of `npm test`
   after the fix (among them runs with a run-control test made to fail or time out), and a committed mutation that a
   registered test must catch.
2. **Fix the causes** in the Studio, the runner, `rcfd run` or the tests, whichever they name. No test is skipped,
   retried or given a longer limit, and what :320 checks (the pause requested from the Studio and acted on) is kept.
3. Run the registered checks, the Studio's type checks and `npm test` in `apps/studio-web`; report every other test that
   failed in those runs by name, without fixing it.

Decision rule to register: done if each cause is shown by a recorded run before the fix, :320 then passes the registered consecutive runs with its checks and limits unchanged, the registered whole runs leave no rcfd run process, and the mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `apps/studio-web/tests/e2e/run-control.spec.ts`, `apps/studio-web/tests/support/`, the Studio's and the runner's start and end of an `rcfd run` child, and `rcfd run`'s pause and stop handling only where a cause is in it. No golden.

## Common to this work

The acceptance of F-893 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0790-F-893.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.

Model changed (2026-11-24): Claude Opus 5.5, on the maintainer's answer "Issue on Opus now"; the agent first issued on Claude Fable 5.2 stopped at the model's usage limit before any work.
