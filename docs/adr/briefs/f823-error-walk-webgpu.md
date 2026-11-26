# Brief: F-823, error-walk.spec.ts:177 under WebGPU in the Linux container

Branch: f823-studio-ci-engine
Items: F-823
ADR: 0809
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 0; 0; 0; 28646

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f823-studio-ci-engine`, on the branch `f823-studio-ci-engine`; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f823-error-walk-webgpu-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief E:/projects/cfd/rcfd/.claude/worktrees/documentation-process-framework-3f9271/docs/adr/briefs/f823-error-walk-webgpu.md (the lead's checkout of main; not on this branch) (F-823, ADR 0809) is carried out on the branch f823-studio-ci-engine, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f823-studio-ci-engine, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat b1e2b376..HEAD` shows the registration of ADR 0809 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `npm run typecheck`, `npm test` in apps/studio-web on Windows and in the Ubuntu 24.04 container, `actionlint`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- apps/studio-web/tests/e2e/error-walk.spec.ts 13957

Also in the repository, as needed: ADR 0802 and ADR 0804 with their addenda (on this branch, `docs/adr/`), the studio-web job
of `.github/workflows/ci.yml`, the container's image file, the logs in `E:/projects/cfd/w49i-work/f823-linux-e2e-scratch`.

## What the lead decided (2026-11-26)

ADR 0804 made the four Linux failures run and pass (xterm, `--enable-unsafe-webgpu` with SwiftShader, a polled focus check,
a wait for the signalled processes) and added the skip step; it is partial because `error-walk.spec.ts:177` now takes more
than its own 60 s wait in the container (28.7 s with WebGL 2 before, 47.3 s and over 60 s after). On ADR 0804's question 1
the lead takes the recommendation (a), on the maintainer's standing instruction of 2026-11-26: a further registration of
F-823 (rule 13) on this branch that measures where the time goes, with the 60 s limit kept. This work is ADR 0809,
`docs/adr/0809-f823-registration-<slug>.md`, its first commit, alone, after ADR 0804's commits. The branch is not merged
until the Linux job passes as registered; the lead writes the answers into ADRs 0802 and 0804 at the merge.

## Task

1. **Register before any change**: the hypothesis (WebGPU on SwiftShader draws the previews the test walks, and a draw costs
   more than under WebGL 2) and the control that tests it (the test's own steps timed one by one, in the container, with
   and without the switch, a registered number of runs); the remedy the result points to, from the code (for example a
   preview drawn once, a lighter draw, the switch scoped to the tests that need WebGPU), with its row: the test passes within
   its unchanged 60 s in the container and on Windows, and the whole job's rows of ADR 0804 hold.
2. **Change** only what the registration names (the job, the image, the test or the app file). No limit changes; no skip.
3. Mutations on scratch copies, each recorded with what caught it; a committed patch if a Studio file changed (rule 16).
4. Run the job's steps as registered on both platforms, alone on the machine as far as you can see.

Decision rule to register: done for F-823 if every row of ADR 0804 and of this registration holds on both platforms and each
mutation is caught; the lead's reading of the first run on GitHub Actions decides the item.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: the studio-web job of `.github/workflows/ci.yml`, the container's image file, `error-walk.spec.ts` and the
app files the registration names, ADR 0809, `docs/roadmap/proposals/0809-F-823.json`. Not the canvas files or the hand-only
surfaces: F-979 and F-981 work on them at the same time.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0809-F-823.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
