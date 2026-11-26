# Brief: F-988, the provenance popover of a Runs grid cell closes on Escape every time

Branch: f988-provenance-popover-escape
Items: F-988
ADR: 0813
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 0; 0; 0; 60105

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f988-provenance-popover-escape`, on the branch `f988-provenance-popover-escape`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f988-provenance-popover-escape-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f988-provenance-popover-escape.md (F-988, ADR 0813) is carried out on the branch f988-provenance-popover-escape, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f988-provenance-popover-escape, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0813 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- apps/studio-web/tests/e2e/provenance-f828.spec.ts 45416 (KR KK, and its helpers byHover and byKeys)

Also in the repository, as needed: ADR 0808 Addenda 7 and 8 (control K: KR KK fails 2 of 5 on 2327d4f1, the popover left
open after Escape at `shut`), the provenance popover and its key handling in `apps/studio-web/src/renderer` (F-828), the
Runs grid (`runs/`) and its roving focus, the logs in `E:/projects/cfd/w49i-work/f981-retire-hand-navigation-scratch`
(k.out).

## What the lead decided (2026-11-26)

KR KK of `provenance-f828.spec.ts` fails on main by itself (2 of 5 runs alone on 2327d4f1; ADR 0808 Addendum 8), and in
the push of F-823 it failed in all three runs of npm test, so it now blocks every push that runs the Studio's tests. F-988
is issued at once, before the other Studio items. The case is not loosened: the remedy is in the cause, whether that is the
app (the popover's Escape handling, a second Ctrl+I, focus leaving the popover) or the test (a step that does not wait for
a state the app gives), each shown by a control.

## Task

One registration, `docs/adr/0813-f988-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any change**: the hypotheses from the code, each with its control (KR KK's steps traced one by one:
   which element has focus and whether the popover is open before and after each key, over a registered number of runs on
   main's tree, with a rule fixed for each outcome); the remedy the control points to; rows: the spec alone passes in 10 of
   10 runs, npm test passes once; the machine's load recorded with each run.
2. **Change** only what the registration names. No check of the case removed or loosened; an added wait asserts a state.
3. Mutations on scratch copies (an e2e patch's expect-fail command builds and packages first: `npm run build && npm run
   package:dir && npx playwright test ...`), each recorded with what caught it; one committed as a patch (rule 16).
4. Run `npm run typecheck`, `npm test` in `apps/studio-web` and `docs_site`.

Decision rule to register: done if every row holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: `apps/studio-web/tests/e2e/provenance-f828.spec.ts`, the provenance popover's files and the Runs grid's key
handling as the registration names them, ADR 0813, `docs/roadmap/proposals/0813-F-988.json`. No other agent runs the
Studio's tests while you do.

## Common to this work

The acceptance of F-988 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0813-F-988.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
