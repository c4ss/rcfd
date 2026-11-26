# Brief: F-979, Claude's canvas: meshes and values shown live as MCP clients change them

Branch: f979-canvas-live-mesh-values
Items: F-979
ADR: 0807
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 125898

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f979-canvas-live-mesh-values`, on the branch `f979-canvas-live-mesh-values`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f979-canvas-live-mesh-values-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f979-canvas-live-mesh-values.md (F-979, ADR 0807) is carried out on the branch f979-canvas-live-mesh-values, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f979-canvas-live-mesh-values, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0807 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/studio-guardrails.md 15516 (G3, G13, G16, G23)
- docs/adr/0803-the-studio-as-claudes-canvas-record.md 5545
- docs/adr/0805-f976-registration-a-view-specification-and-show-view.md 29560 (the view specification, its kinds and its path to the open Studio; addendum 1)
- crates/rcfd-workspace/src/view.rs 27021
- apps/studio-web/src/renderer/canvas/Canvas.tsx 5613
- apps/studio-web/src/main/project/session.ts 27954 (the feed poll and `view_shown`)

Also in the repository, as needed, by section: ADR 0188 (the feed and its limit of 4 polls of 250 ms), ADR 0187 (the mesh
tools of F-272: `check_case_mesh`, `preview_mesh`, `import_mesh`, `refine_case_mesh`), the Studio's mesh and field views and
F-447's field transfer (`apps/studio-web/src/renderer/field`, `plots`), `apps/studio-web/tests/e2e/canvas-f976.spec.ts`.

## What the lead decided (2026-11-26)

F-976 (ADR 0805) gave the canvas: a view specification, `show_view`, and a Canvas tab that draws each panel with an existing
view; a `mesh` panel shows the mesh's row, not a picture. F-979 makes the canvas live (ability 4 of ADR 0803): a mesh made or
edited by an MCP tool is drawn again, and a setting or a monitored value that an MCP client or the runner changes is updated
in the shown view, each with its actor and reason. How the canvas learns of a change (the feed, G12), which panels follow
which changes, and how a mesh is drawn (an existing view of the Studio, the field transfer of F-447) are this
registration's. The feed's format is unchanged; an engine interface changes only by addition (G4).

## Task

One registration, `docs/adr/0807-f979-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code**: the changes the canvas follows (mesh made, imported, refined or edited; a case setting
   changed; a run's state, residuals and monitors) and the feed entries or files each comes from; what each panel kind does
   on such a change; the mesh picture (which view draws it, what it shows); the limit of each row with its source (ADR
   0188's poll limit; a figure of your own is not a source, rule 15); negative controls and mutations (rule 16).
2. **Build** the live update of the canvas, the mesh picture of a `mesh` panel, and the actor and reason shown with a change.
3. An e2e test that runs the rcfd-mcp binary as a client of the project, shows a view with a mesh, a setting and a monitor,
   then refines the mesh, edits the setting with a reason and runs a short case, and reads each change in the open Studio
   within its limit.
4. Run the checks rule 7 lists for the files touched, `npm test` in `apps/studio-web`, and `docs_site`.

Decision rule to register: done if every row of the acceptance holds and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: the canvas files under `apps/studio-web/src/renderer/canvas/`, the parts of
`apps/studio-web/src/main/project/session.ts` and `apps/studio-web/src/shared/project.ts` that the canvas reads (named in the
registration), additive parts of `crates/rcfd-workspace/src/view.rs`, new tests, ADR 0807,
`docs/roadmap/proposals/0807-F-979.json`. Not the files F-981 owns (the hand-only surfaces and the coverage test): another
agent works on them at the same time.

## Common to this work

The acceptance of F-979 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0807-F-979.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
