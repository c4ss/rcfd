# Brief: F-273, Claude's canvas: the reason of a change shown with who and what, from the feed

Branch: f273-reason-on-canvas
Items: F-273
ADR: 0837
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 95670

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f273-reason-on-canvas`, on the branch `f273-reason-on-canvas`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f273-reason-on-canvas-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f273-reason-on-canvas.md (F-273, ADR 0837) is carried out on the branch f273-reason-on-canvas, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f273-reason-on-canvas, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0837 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, and the Studio's `npm test`, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0188-adr-0173-amendment-4-f273-registration-project-journal-and-change-feed.md 45186
- crates/rcfd-workspace/src/feed.rs 21664
- crates/rcfd-workspace/src/journal.rs 10897
- apps/studio-web/src/renderer/project/activity.ts 3234

Also in the repository, as needed: ADR 0803 (the Studio as Claude's canvas), docs/adr/0826-f978-registration-the-process-view-of-a-case.md and docs/adr/0828-f978-registration-process-view-rows-and-runner-teardowns.md (the process view, its rows),
`crates/rcfd-workspace/src/view.rs` (the process view's rows), `apps/studio-web/src/renderer/canvas/Canvas.tsx`, the Studio's
`claude-code.spec.ts` (the actor and the activity line), `crates/rcfd-mcp/src/project.rs` (the tools that take a reason).

## What the lead decided (2026-11-27)

F-273 stays partial because no view shows the reason (why) of a feed entry: the journal keeps an optional `reason`
(ADR 0188 J4), the Studio names the actor and its activity line shows the client and the command, but the renderer reads no
reason. It was held until F-976's view specification existed so that it is shown through it (ADR 0803); F-976 and F-978's
process view exist. The lead issues it, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): the reason of each change is shown with its who and what, in the process
view's rows and in the activity line, read from the feed.

## Task

One registration, `docs/adr/0837-f273-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: where the reason is shown (the process view's row of a journaled step, the activity line),
   its form when absent, its length cut (ADR 0188 J4's limit) and how text is escaped; that the reason is read from the feed
   only (additive fields, no new store); the e2e test: an MCP client makes changes with and without a reason, and the open
   Studio shows each with its who, what and why, within the limit ADR 0188 fixed for the feed; negative controls and
   mutations (rule 16), one committed as a patch whose expect-fail command builds and packages the app first, as every other
   e2e mutation patch does; the numbers fixed in advance (rule 15). The exported page's process panel is F-996's, not this
   work's.
2. **Build** in the workspace's view rows and the Studio's renderer; defaults unchanged; no golden changes.
3. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-273 if the Studio shows who, what and why of every change in the e2e test, its checks
hold, `npm test` passes and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: the process view's rows in `crates/rcfd-workspace/src/view.rs`, `activity.ts` and the canvas renderer files the
registration names, new tests, ADR 0837, `docs/roadmap/proposals/0837-F-273.json`. No solver files. You are the one agent
running the Studio's `npm test` now.

## Common to this work

The acceptance of F-273 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0837-F-273.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
