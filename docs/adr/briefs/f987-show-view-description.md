# Brief: F-987, show_view's tool description names every panel kind and the run form

Branch: f987-show-view-description
Items: F-987
ADR: 0838
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 106049

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f987-show-view-description`, on the branch `f987-show-view-description`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f987-show-view-description-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f987-show-view-description.md (F-987, ADR 0838) is carried out on the branch f987-show-view-description, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f987-show-view-description, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0838 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0807-f979-registration-live-meshes-settings-and-monitors-on-the-canvas.md 34403
- docs/adr/0817-f977-registration-layouts-chosen-by-the-case-category.md 35665
- crates/rcfd-mcp/tests/tool_list_f275.rs 21292

Also in the repository, as needed: ADR 0803 and F-976's registration (the view kinds), docs/adr/0195-adr-0173-amendment-6-f275-registration-project-mode-tool-list-and-plugin-edit-hook.md (the project-mode tool list and
its golden), `crates/rcfd-mcp/src/project.rs` (show_view's description), `crates/rcfd-mcp/tests/golden/tools_list_project_f270.json`
and `tools_list_default.json`, `crates/rcfd-mcp/tests/tool_list_adr0730.rs`.

## What the lead decided (2026-11-27)

F-987 was entered on ADR 0807 question 1 and F-977's Q1: show_view's description names F-976's kinds but not the
mesh-of-a-case, setting and monitor-of-a-case forms of ADR 0807, nor the run form of ADR 0817. The acceptance has the lead
re-record the tool-list goldens for that one entry, with the maintainer's word recorded: the lead takes it on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always), and records it
so in the ADR. The re-record is its own merge.

## Task

One registration, `docs/adr/0838-f987-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the new description text, every form it names with its source ADR; a test that reads the
   description and finds each form and kind; that the golden files change in show_view's description only (a check that
   compares every other entry of each golden with origin/main's, byte for byte); the golden written by the test's own
   record mode or by hand, named; negative controls and mutations (rule 16), one committed as a patch with
   `--target-dir target/mutation`; the numbers fixed in advance (rule 15).
2. **Build** the description in `crates/rcfd-mcp/src/project.rs`; re-record the goldens' show_view entry only. No other tool's
   text changes.

Decision rule to register: done for F-987 if the description names every form and kind, the goldens differ from origin/main in
that entry only, every tool-list test passes and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: show_view's description in `crates/rcfd-mcp/src/project.rs`, the two tool-list goldens (that entry only), new
tests, ADR 0838, `docs/roadmap/proposals/0838-F-987.json`. No solver files, no Studio files.

## Common to this work

The acceptance of F-987 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0838-F-987.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
