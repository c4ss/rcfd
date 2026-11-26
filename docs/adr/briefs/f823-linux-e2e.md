# Brief: F-823, the four Linux failures of the studio-web job, and a step against unexpected skips

Branch: f823-studio-ci-engine
Items: F-823
ADR: 0804
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 0; 0; 0; 52000

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f823-studio-ci-engine`, on the branch `f823-studio-ci-engine`; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f823-linux-e2e-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief E:/projects/cfd/rcfd/.claude/worktrees/documentation-process-framework-3f9271/docs/adr/briefs/f823-linux-e2e.md (the lead's checkout of main; not on this branch) (F-823, ADR 0804) is carried out on the branch f823-studio-ci-engine, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f823-studio-ci-engine, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat d5ec1145..HEAD` shows the registration of ADR 0804 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `npm run typecheck`, `npm test` in apps/studio-web on Windows and in the Ubuntu 24.04 container, `actionlint`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0802-f823-registration-the-studio-web-job-builds-the-engine.md (the whole ADR with its amendment and addendum 1; on this branch)
- .github/workflows/ci.yml 9557 (the studio-web job)

Also in the repository, as needed: the four failing specs (`claude-code.spec.ts:177`, `commands.spec.ts:90`,
`runner-lifetime.spec.ts:192`, `workspace-first-open.spec.ts:59` under `apps/studio-web/tests/e2e/`), the container of ADR
0802 and its Dockerfile, ADR 0801 addendum 19 (WebGL, the GPU and the container), and the logs in
`E:/projects/cfd/w49i-work/f823-studio-ci-engine-scratch`.

## What the lead decided (2026-11-26)

F-823 (ADR 0802) is partial: on Windows 880 of 880 passed, in the Ubuntu 24.04 container 4 failed and 16 did not run. The
branch is not merged: its job would turn CI on main red on Linux. Decided by the lead on ADR 0802's questions, under the
maintainer's standing direction (no test is skipped to make a run pass): Q1, a further registration of F-823 (rule 13) on
this same branch that makes the four tests run and pass on Linux by a change of the job's environment (the image's
packages: a terminal emulator, a software WebGPU adapter) or, where the diagnosis shows a defect of the test or the app,
by a registered change of it; no skip, and no Windows-only e2e. Q2, yes: `--ignore-gpu-blocklist` on Windows too if
`windows-latest` needs it. Q3, yes: a step of the job that fails on any skip not in the registered list. This work is
ADR 0804, `docs/adr/0804-f823-registration-<slug>.md`, its first commit, alone, on this branch after ADR 0802's
commits. The lead writes these decisions into ADR 0802 at the merge.

## Task

1. **Register before any change**: for each of the four failures and the 16 that did not run, the diagnosis read from the
   code and the logs (registered as a hypothesis, then the control that tests it); the change to the job or the image that
   should make it run, or the defect of the test or the app with the change it needs; the rows: on Windows and in the
   container, the job's steps as written run in order and pass with 0 failed and every skip in the registered list (which
   may be empty); the skip step fails the job on an added skip (a negative control on a scratch copy); the workflow is valid.
2. **Change** the studio-web job of `.github/workflows/ci.yml`, the image file of the container, and only where the
   registration shows a defect, the named test or app file, each change named in the registration. If a failure needs a
   change outside those, stop and report.
3. Mutations on scratch copies: the terminal package removed; the WebGPU adapter removed; a skip added to a spec; each is
   recorded with its output. A committed patch only if a Studio test file changed (rule 16).
4. Run the checks rule 7 lists and the job's steps as registered, on both platforms.

Decision rule to register: done for F-823 if every registered row holds on both platforms and each mutation is seen;
the lead's reading of the first run on GitHub Actions after the push decides the item.

Open questions for the maintainer, with options and a recommendation, not decided by you: any question of scope.

Files you own: the studio-web job of `.github/workflows/ci.yml`, the container's image file, the test or app files the
registration names, ADR 0804, `docs/roadmap/proposals/0804-F-823.json`.

## Common to this work

The acceptance of F-823 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0804-F-823.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
