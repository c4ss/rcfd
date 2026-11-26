# Brief: F-977, Claude's canvas: the layout of results chosen by the case's category

Branch: f977-category-layouts
Items: F-977
ADR: 0817
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 88675

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f977-category-layouts`, on the branch `f977-category-layouts`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f977-category-layouts-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f977-category-layouts.md (F-977, ADR 0817) is carried out on the branch f977-category-layouts, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f977-category-layouts, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0817 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 15516 (G3, G13, G16, G23)
- docs/adr/0803-the-studio-as-claudes-canvas-record.md 5545
- crates/rcfd-workspace/src/view.rs 36835
- apps/studio-web/src/renderer/canvas/model.ts 3013
- apps/studio-web/src/renderer/canvas/Canvas.tsx 13077

Also in the repository, as needed: ADR 0805 (the view specification and `show_view`, F-976), ADR 0807 (the live canvas, F-979),
the case schema and its example cases (`examples/`, the case kinds and their physics blocks), the run records, the e2e test
`apps/studio-web/tests/e2e/canvas-f976.spec.ts`.

## What the lead decided (2026-11-26)

F-976 gave the view specification and `show_view`; F-979 made the canvas live. F-977 chooses the views when Claude asks
for a run without a layout: each case category has a default layout derived from the case and run records, and a case of
no known category takes the general layout. The categories are fixed from the case schema (at least external aerodynamics,
internal flow and turbomachinery, heat transfer, structural, multiphase); how a case's category is read (a field of the
schema, or a rule over its physics blocks, named with its file and line) and which views each layout holds are this
registration's. The view specification changes only by addition (G4).

## Task

One registration, `docs/adr/0817-f977-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the categories and the rule that gives a case its category, from the schema; each
   category's layout (panels, the record each reads), the general layout; the example case of the repository for each
   category, with the views its layout must show; negative controls (a case of no known category; a run without the record
   a panel needs) and mutations (rule 16); any limit with its source (rule 15).
2. **Build** the category rule and the layouts, and `show_view` with a run and no layout.
3. An e2e test that shows each category's example run without a layout and reads the views chosen in the open Studio.
4. Run the checks rule 7 lists for the files touched, `npm test` in `apps/studio-web` (record the CPU load with each timed
   run) and `docs_site`.

Decision rule to register: done if every category's row holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: additive parts of `crates/rcfd-workspace/src/view.rs` and of the MCP tool `show_view`, the canvas files under
`apps/studio-web/src/renderer/canvas/`, new tests, ADR 0817, `docs/roadmap/proposals/0817-F-977.json`. No solver files.

## Common to this work

The acceptance of F-977 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0817-F-977.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
