# Brief: F-976, Claude's canvas: a view specification and show_view, which the open Studio renders

Branch: f976-canvas-view-spec
Items: F-976
ADR: 0805
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 1; 108650

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f976-canvas-view-spec`, on the branch `f976-canvas-view-spec`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f976-canvas-view-spec-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f976-canvas-view-spec.md (F-976, ADR 0805) is carried out on the branch f976-canvas-view-spec, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f976-canvas-view-spec, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0805 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `baseline_hashes` if a shared solver file changed, `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 15516 (with the change of 2026-11-26: G15 retired, G23 added)
- docs/adr/0803-the-studio-as-claudes-canvas-record.md 5545 (the decision this item comes from)
- docs/mcp-tools.md 46481 (the project-mode tools and how they are listed)
- apps/studio-web/src/main/project/session.ts 26419 (the feed poll the Studio already runs)

Also in the repository, as needed, by section: ADR 0173 and ADR 0188 (the workspace layer and the feed: how a change made
by one process reaches another, and its limit), ADR 0195 (the project-mode tool list), `crates/rcfd-mcp/src/project.rs`
(`render_field` and the tool list), `crates/rcfd-workspace/src/feed.rs`, the Studio's views under
`apps/studio-web/src/renderer/` (`plots`, `field`, `runs`, `theme`), `apps/studio-web/tests/e2e/claude-code.spec.ts` (an e2e
test that drives the open Studio from an MCP client).

## What the lead decided (2026-11-26)

ADR 0803 records the maintainer's direction of 2026-11-26: the hand-navigated Studio ends; the Studio window becomes
Claude's canvas, driven by MCP tools and live through the feed. F-976 is the first item of the line, because every other
one (F-977 to F-981) shows through it. The specification's fields, the tool names beyond `show_view`, the way a request
reaches the open Studio (through the project folder and the feed, G12, or another path the registration argues) and the
time limit are this registration's, each with its source (rule 15: ADR 0188's feed limit is a source; a figure of your own
is not). The engine's interfaces change only by addition (G4); the MCP tool list golden changes in the additive class.

## Task

One registration, `docs/adr/0805-f976-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the specification (its version, its kinds: case, run, field, mesh, plot, monitor, table;
   the reference to a case or run; the layout), its schema in the engine and its validation with located errors; the path
   from `show_view` to the open Studio; what the Studio renders for each kind with which existing view; what happens with
   no Studio open, with two open, and with an invalid specification; the rows of F-976's acceptance, each with its limit and
   source; negative controls and mutations (rule 16).
2. **Build** the schema and validation (engine side), the tool `show_view` and its service method, and the Studio's
   renderer of a specification, using the existing views and theme. No hand-built panel for it.
3. An e2e test that runs the rcfd-mcp binary as a client of the project, calls `show_view` with a valid and an invalid
   specification, and reads what the open Studio shows (by the test hooks the Studio's other e2e tests use).
4. Run the checks rule 7 lists for the files touched, `npm test` in `apps/studio-web`, and `docs_site`.

Decision rule to register: done if every row of the acceptance holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: any question of scope (for
example what the canvas shows beyond F-976's kinds).

Files you own: new modules in `crates/rcfd-workspace` or `crates/rcfd-mcp` for the specification and the tool (named in
the registration), the additive entries of `crates/rcfd-mcp/tests/golden/tools_list*` (reported as an additive golden edit
for the lead), the Studio's main and renderer files of the canvas (named in the registration), new tests, `docs/mcp-tools.md`
(the new tool), ADR 0805, `docs/roadmap/proposals/0805-F-976.json`.

## Common to this work

The acceptance of F-976 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0805-F-976.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
