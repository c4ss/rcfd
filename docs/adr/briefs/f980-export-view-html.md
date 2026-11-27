# Brief: F-980, Claude's canvas: a view exported as one self-contained styled HTML page

Branch: f980-export-view-html
Items: F-980
ADR: 0823
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 145246

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f980-export-view-html`, on the branch `f980-export-view-html`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f980-export-view-html-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f980-export-view-html.md (F-980, ADR 0823) is carried out on the branch f980-export-view-html, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f980-export-view-html, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0823 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 15516 (G3, G13, G16, G23)
- docs/adr/0803-the-studio-as-claudes-canvas-record.md 5545
- docs/adr/0805-f976-registration-a-view-specification-and-show-view.md 29560
- crates/rcfd-workspace/src/view.rs 66632
- apps/studio-web/src/renderer/canvas/Canvas.tsx 13304

Also in the repository, as needed: ADR 0807 (the live canvas, F-979), docs/adr/0817-f977-registration-layouts-chosen-by-the-case-category.md (category layouts, F-977), the MCP tools of
`crates/rcfd-mcp` (`show_view` and its tool list golden), the Studio's theme and styles (`apps/studio-web/src/renderer`), the
views the canvas draws (plots, field and mesh views), the provenance of a value (F-828), the e2e test
`apps/studio-web/tests/e2e/canvas-f977.spec.ts`.

## What the lead decided (2026-11-27)

F-976 gave the view specification and `show_view`, F-979 the live canvas, F-977 the category layouts. F-980 is ability 5 of ADR
0803: `export_view` writes a view specification as one self-contained HTML file, with no network fetch, the Studio's theme and
styling, the data of the views it names and their provenance; the page opens offline and shows the values the Studio shows for
the same specification. How the page is built (the renderer's own views in a static bundle, or a page of its own drawn from the
same records; how a field or a mesh is drawn without the GPU, or as a picture with its values), and the registered set of
specifications compared, are this registration's. An engine interface changes only by addition (G4); a new MCP tool re-records
the tool list golden, additively.

## Task

One registration, `docs/adr/0823-f980-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the tool (`export_view`, its input, where it writes, its errors); the page (what each panel kind
   becomes, the theme, the provenance shown, no network fetch, checked by a registered means); the comparison (a registered set
   of specifications over the repository's example runs, each value the page shows against the value the Studio shows for the
   same specification, the tolerance and its source); the size of the file of a registered case, recorded; negative controls and
   mutations (rule 16).
2. **Build** `export_view` and the page.
3. Tests: an MCP test of the tool on the registered set, and an e2e test that opens each exported page offline (network blocked)
   and reads its values against the open Studio's.
4. Run the checks rule 7 lists for the files touched, `npm test` in `apps/studio-web` (record the CPU load with each timed run)
   and `docs_site`.

Decision rule to register: done if every row holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: the `export_view` tool in `crates/rcfd-mcp` and its tool list golden (additively), additive parts of
`crates/rcfd-workspace/src/view.rs`, the export's files under `apps/studio-web/src/` (named in the registration), new tests, ADR
0823, `docs/roadmap/proposals/0823-F-980.json`. No solver files.

## Common to this work

The acceptance of F-980 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0823-F-980.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
