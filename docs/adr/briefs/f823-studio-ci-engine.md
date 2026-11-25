# Brief: F-823, the Studio's CI job builds the engine, so the tests that need it run

Branch: f823-studio-ci-engine
Items: F-823
ADR: 0802
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-25
Size (lead): done; 0; 0; 0; 36943

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f823-studio-ci-engine`, which the lead made with the branch `f823-studio-ci-engine` at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f823-studio-ci-engine-scratch`.

## Goal

Your work is started with this line as its whole prompt:

    /goal The brief docs/adr/briefs/f823-studio-ci-engine.md (F-823, ADR 0802) is carried out on the branch f823-studio-ci-engine, checked out in this directory, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of the ADR as the first commit, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `npm run typecheck`, `npm test` in apps/studio-web, `actionlint` or the registered check of the workflow, and the tests the brief names, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 14395
- .github/workflows/ci.yml 9557 (the studio-web job and how the Rust jobs build)

Also in the repository, as needed: ADR 0800 (Q3) and ADR 0801, its addendum 1 and addendum 21 (the engine found without set-up; the `--no-engine` flag of `scripts/package-dir.mjs`), by section and not in full; `apps/studio-web/tests/support/f447.ts` (`findBench`) and the tests that skip without the engine or the bench; section 7 of `docs/test-process.md`; the logs of the CI run 45705107206 (`gh run view --repo c4ss/rcfd --job <id> --log`, read only).

## What the maintainer decided (2026-11-25)

F-823's acceptance asks that the job build rcfd-cli, rcfd-runner and rcfd-mcp first, so that the service e2e tests and the F-824 inventory check do not skip. The lead read the CI run 45705107206 at 2d125d55 (2026-11-25): the job is green on ubuntu-24.04 and windows-latest, with 596 passed and 284 skipped on each (239 of them electron, 44 unit), and the workflow says that the job builds no Rust and packages with `--no-engine`. Decided by the lead: the job builds the three binaries and the `f447_bench` example in release, packages the Studio with the engine, and every test that still skips is named with its reason, which is one the code states (the platform, for example), and not a missing build. The job's time limit and any cache are the registration's, argued from measured times; no test is skipped, removed or loosened to pass. The first run on GitHub Actions happens after the lead's push; the lead reads it and records it.

## Task

One registration, `docs/adr/0802-f823-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any change**: the skipped tests of the run 45705107206 per platform, each with its skip reason as the
   code gives it, grouped by reason; which of them the change must make run and which stay skipped, and why; the build steps, the
   time limit and any cache, with the times measured on this machine; the rows: on Windows here and in an Ubuntu 24.04 container
   (the container of ADR 0150), the steps of the job as written run in order and pass, with the counts of passed and skipped
   tests and the list of remaining skips equal to the registered list; the workflow is valid (a named check); the job still
   fails on a failing test (a negative control on a scratch copy, not committed).
2. **Change** `.github/workflows/ci.yml` (the studio-web job only) and the sentence of `docs/test-process.md` section 7 that
   names the job. No Studio source, no test and no other job changes; if a test cannot run in the job without a change to the
   test or the app, stop and report.
3. Mutations, each on a scratch copy: a binary not built (its tests skip again, seen by the count row); the bench not built;
   `--no-engine` kept. None is committed as a patch (the workflow is not a Rust or Studio test target); each is recorded with
   its output.
4. Run the checks rule 7 lists for the files changed, `cargo test -p rcfd-cli --release --test docs_site` and the job's steps as
   registered.

Decision rule to register: done if every registered row holds on both platforms here and each mutation is seen; the lead's reading of the first run on GitHub Actions after the push decides the item, and the agent's verdict is the one the rule gives for the local rows.

Open questions for the maintainer, with options and a recommendation, not decided by you: whether a remaining skip should be made to run by a change of the test or the app, with options and a recommendation; any other that arises.

Files you own: The studio-web job of `.github/workflows/ci.yml` and one sentence of `docs/test-process.md` section 7. No Studio source, no test, no other job.

## Common to this work

The acceptance of F-823 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0802-F-823.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
