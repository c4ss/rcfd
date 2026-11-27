# Brief: F-978, Claude's canvas: the process of a simulation from its creation to its last result

Branch: f978-process-view
Items: F-978
ADR: 0825
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 214882

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f978-process-view`, on the branch `f978-process-view`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f978-process-view-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f978-process-view.md (F-978, ADR 0825) is carried out on the branch f978-process-view, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f978-process-view, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0825 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 15516 (G3, G12, G13, G16, G23)
- docs/adr/0803-the-studio-as-claudes-canvas-record.md 5545
- docs/adr/0805-f976-registration-a-view-specification-and-show-view.md 29560
- crates/rcfd-workspace/src/view.rs 106772
- apps/studio-web/src/renderer/canvas/Canvas.tsx 13304
- apps/studio-web/src/main/project/session.ts 29496

Also in the repository, as needed: ADR 0188 (the feed and its events), the project journal and the run records of
`crates/rcfd-workspace`, the MCP tools that create a case, make its mesh, change its settings and start a run (`crates/rcfd-mcp`),
ADR 0807 (the live canvas), ADR 0817 (category layouts), docs/adr/0823-f980-registration-a-view-exported-as-one-self-contained-html-page.md (export_view), the e2e tests
`apps/studio-web/tests/e2e/canvas-f979-a1.spec.ts` and `canvas-f977.spec.ts`.

## What the lead decided (2026-11-27)

F-976, F-979 and F-980 are done and F-977 is partial: the canvas shows views by `show_view`, follows changes live, chooses a
layout by category and exports a page. F-978 is the process view of ADR 0803: for a case, the ordered path from its creation
through its mesh, its set-up and each run (state changes, residuals, monitors) to its results, read from the journal, the run
records and their events (the feed of ADR 0188); each step opens the view of its state. How the steps are read and ordered,
what a step shows, and how a step opens its state's view are this registration's. The view specification changes only by
addition (G4); a new panel kind is added to `show_view`'s description and the tool list golden re-recorded additively.

## Task

One registration, `docs/adr/0825-f978-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the sources of the steps (journal entries, run records, feed events) with their files; the
   order (by what key, ties); what each kind of step shows and which view it opens; the panel kind and its specification;
   the rows: an e2e test builds a case through the MCP tools from new to a finished run and the view lists every journaled step
   in order, none missing against the journal (a registered comparison); each step opens its view; negative controls and
   mutations (rule 16); any limit with its source (rule 15).
2. **Build** the process view as a canvas panel and its engine side.
3. Tests: an engine test of the step list against the journal, and the e2e test of the acceptance.
4. Run the checks rule 7 lists for the files touched, `npm test` in `apps/studio-web` (record the CPU load with each timed
   run) and `docs_site`.

Decision rule to register: done if every row holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: additive parts of `crates/rcfd-workspace/src/view.rs` and of the MCP tool `show_view` (with its tool list
golden, additively), the canvas files under `apps/studio-web/src/renderer/canvas/`, the parts of
`apps/studio-web/src/main/project/session.ts` the panel reads (named in the registration), new tests, ADR 0825,
`docs/roadmap/proposals/0825-F-978.json`. No solver files.

## Common to this work

The acceptance of F-978 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0825-F-978.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
