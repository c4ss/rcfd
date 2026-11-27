# Brief: F-990, Structural runs in a project: start_run of a solid case file, its records, and the structural layout shown from them

Branch: f990-structural-project-run
Items: F-990
ADR: 0840
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 98826

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f990-structural-project-run`, on the branch `f990-structural-project-run`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f990-structural-project-run-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f990-structural-project-run.md (F-990, ADR 0840) is carried out on the branch f990-structural-project-run, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f990-structural-project-run, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0840 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests, the named test files, `baseline_hashes`, and the Studio's `npm test`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0817-f977-registration-layouts-chosen-by-the-case-category.md 35665
- crates/rcfd-workspace/src/runs.rs 30673
- crates/rcfd-runner/src/store.rs 17799

Also in the repository, as needed: `crates/rcfd-api/tests/solid_cases/` (the solid case files), the structural solver's
entry point and its report (the `rcfd-api` structural run and the CLI command that runs a solid case), `crates/rcfd-mcp/src/project.rs`
(start_run), `crates/rcfd-workspace/src/view.rs` (the structural layout of ADR 0817), the Studio's e2e tests of F-977's layouts,
ADR 0828 (the runner teardowns).

## What the lead decided (2026-11-27)

F-977 is partial because no project run of a solid case can exist: its structural layout (case, run) is built, but E8 (the
structural category shown from a run in the Studio) could not run. F-990 makes that run. The lead issues it, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always), ahead of the
other Studio items because it closes F-977's last gap.

## Task

One registration, `docs/adr/0840-f990-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: how start_run recognises a solid case file and which solver path the runner takes for it
   (named by file and line); the run's records (`run.json`, `case.snapshot`, its report, its stored displacement and stress
   fields) and their form, read from the existing fluid runs' records where they share one; the solid case chosen from
   `solid_cases/` and its known values (the report's quantities against the case's own test, the same bytes); show_view with
   that run and no layout takes the structural layout, which shows those fields and the report; F-977's E8 run on it; the
   e2e test in the open Studio; that no fluid run's records or results change (`baseline_hashes`); negative controls and
   mutations (rule 16), one committed as a patch whose expect-fail command builds and packages the app first where it is an
   e2e mutation, else uses `--target-dir target/mutation`; the numbers fixed in advance (rule 15).
2. **Build** in the runner and the workspace's runs; the solver itself unchanged; defaults unchanged.
3. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-990 if the solid case's project run holds its records and values, show_view takes the
structural layout and the e2e test shows its fields and report, `baseline_hashes` and `npm test` pass and each mutation is
caught; partial otherwise. Report F-977's E8 with its result for the lead to record.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: the runner's and the workspace's run code the registration names, new tests, ADR 0840,
`docs/roadmap/proposals/0840-F-990.json`. No solver numerics. You are the one agent running the Studio's `npm test` now.

## Common to this work

The acceptance of F-990 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses (rule 2), with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Other agents run on
this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Delete nothing outside your worktree and your scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0840-F-990.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
