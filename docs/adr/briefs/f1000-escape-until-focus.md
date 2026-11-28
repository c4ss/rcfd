# Brief: F-1000, KK2 under load: a traced control of KK4 on RC's tree and an Escape held until the dialog has the focus

Branch: f1000-escape-until-focus
Items: F-1000
ADR: 0845
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 76111

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f1000-escape-until-focus`, on the branch `f1000-escape-until-focus`; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f1000-escape-until-focus-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f1000-escape-until-focus.md (F-1000, ADR 0845) is carried out on the branch f1000-escape-until-focus, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f1000-escape-until-focus, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat branch f1000-kk2-under-load (b5fceafa) merged with origin/main..HEAD` shows the registration of ADR 0845 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, and the Studio's `npm test`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0843-f1000-registration-kk2-under-load.md 27334
- apps/studio-web/src/renderer/provenance/Provenance.tsx 21938
- apps/studio-web/src/renderer/provenance/registry.ts 12150

Also in the repository, as needed: ADR 0813 (V2, the provenance registry), `provenance-escape-f988.spec.ts`, the specs KK3
(`provenance-stall-f1000.spec.ts`) and KK4 (`provenance-gap-f1000.spec.ts`) and their mutation patches, ADR 0843's control
script and load L in `E:/projects/cfd/w49i-work/f1000-kk2-under-load-scratch`, ADR 0828 (the runner teardowns).

## What the lead decided (2026-11-28)

ADR 0843 found the cause of KK2's failure under load (T-C: an Escape in the 8 to 27 ms between a stepped popover's removal and
the next one's mount closes nothing) but its remedy RC held only until `setOpened`, not until the dialog has the focus: KK2
8 of 10 alone and 1 of 5 in whole `npm test` under load, KK4 0 of 5, KK3 3 of 5. Its remedies are not merged; they are on
branch f1000-kk2-under-load at b5fceafa. The lead issues the next step under F-1000 by rule 13, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always). The worktree is made
from b5fceafa with origin/main merged into it by the lead, so ADR 0843's code and specs are in it.

## Task

One registration, `docs/adr/0845-f1000-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: a traced control of KK4 (and KK3) on RC's tree under ADR 0843's load L, with what the trace
   records (the keys, requests, popover mounts and the focus owner, each with its time); the hypotheses for RC's miss (the
   window between `setOpened` and the dialog's focus, and RA's 3 of 5 on KK3) with what each predicts; the remedy each points
   to (for example an Escape that cancels until the dialog has taken the focus); the runs: KK2 alone at least 10 under L,
   KK3 and KK4 at least 5 each, and whole `npm test` runs under L (at least 5), every one passing; negative controls and
   mutations (rule 16), one committed as a patch whose expect-fail command builds and packages the app first; the numbers
   fixed in advance (rule 15). The case is not loosened: no wait lengthened past a limit an earlier ADR fixed, no assertion
   removed. A whole `npm test` under L takes about 1.3 h: plan the runs so.
2. **Run** as registered. A defect found in the renderer is fixed only if the registration names the fix and its check.
3. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-1000 if the remedy holds in every registered run, KK2, KK3 and KK4 pass in every one and
each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: `Provenance.tsx`, `registry.ts`, the specs of ADR 0843 and new tests, ADR 0845,
`docs/roadmap/proposals/0845-F-1000.json`. No solver files. You are the one agent running the Studio's `npm test` now.

## Common to this work

The acceptance of F-1000 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0845-F-1000.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
