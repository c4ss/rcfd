# Brief: F-978, Claude's canvas: the process view's undecided rows, and the tests that leave a runner behind

Branch: f978-process-view-rows
Items: F-978
ADR: 0828
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 66953

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f978-process-view-rows`, on the branch `f978-process-view-rows`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f978-process-view-rows-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f978-process-view-rows.md (F-978, ADR 0828) is carried out on the branch f978-process-view-rows, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f978-process-view-rows, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0828 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `npm run typecheck` and `npm test` in apps/studio-web, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0826-f978-registration-the-process-view-of-a-case.md 25736
- crates/rcfd-mcp/tests/canvas_process_f978.rs 10134
- apps/studio-web/tests/e2e/canvas-f979-a1.spec.ts 16394

Also in the repository, as needed: `crates/rcfd-workspace/src/view.rs` (the process view), `apps/studio-web/src/renderer/canvas/`,
`apps/studio-web/src/shared/runner-lifetime.ts` (which runners the Studio stops at a close), `crates/rcfd-runner/src/main.rs`
(`rcfd-runner shutdown`), `apps/studio-web/tests/support/rcfd.ts`, the other e2e specs and MCP tests that start a run.

## What the lead decided (2026-11-27)

ADR 0826 (F-978, renumbered from 0825 at the merge; ADR 0825 is the maintainer's G22 record) is partial: B1 to B3 did not
decide, their one run refused by the operating system at the first `edit_case` (Access is denied, os error 5, CPU 82 %),
and M4 and M5 were not caught because their edits did not build. On its question the lead takes the recommendation, on the
maintainer's standing instruction of 2026-11-26 (go with the recommended option always): `export_view` drawing a process
panel is the new item F-996.

The maintainer asked (2026-11-27) that the tests stop leaving runners behind. A run started through the MCP tools starts
`rcfd-runner serve` detached; it has no idle exit, and the Studio stops at its close only a runner its record says this
Studio started. `canvas-f979-a1.spec.ts` (its `afterAll`, line 153) and `canvas_process_f978.rs` (lines 221 and 222) end
the client, the Studio and the MCP processes but not the runner: one runner was left per Studio test run (17 were ended by
hand on 2026-11-27), and the one B1 left held the output pipe of a push for nine hours (push_main no longer waits for such
a process, 52ea1458). The engine side (a runner started without inherited handles; an exit when its project folder is
gone) is F-997, another agent's; this work owns the tests.

## Task

One registration, `docs/adr/0828-f978-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any code or run**: B1 to B3 run again as ADR 0826 registered them, once, with the CPU load recorded,
   an OS refusal recorded as not deciding; M4 and M5 made again as edits that build and change what their tests read (the
   mutation unchanged in intent: no `opens` for a `results` step; the Studio does not draw a picked step's `opens`); the
   teardown: every e2e spec and every Rust test that starts a run (found by a registered search, listed with file and line)
   ends its project's runner with `rcfd-runner shutdown --project <folder>` (or the run tools' equivalent) before its folder
   is removed, in a teardown that runs when the test fails too; the check: the number of `rcfd-runner` processes started by
   the run and alive after it, counted before and after `npm test` and after each named Rust test file, is 0; negative
   control: the check reads 1 or more with the teardown of one spec removed; mutations (rule 16).
2. **Change only the tests**, no engine or Studio source beyond ADR 0826's own files if M4 or M5 needs a type that builds.
3. Run as registered; the committed mutation patch's expect-fail command uses `--target-dir target/mutation`.

Decision rule to register: done for F-978 if B1 to B3 hold, M4 and M5 are caught and every ADR 0826 row holds; the
teardown rows are reported beside it and do not change F-978's verdict; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: the named tests and specs (their teardowns), `apps/studio-web/tests/support/`, ADR 0826's code files where
M4 or M5 needs it (named in the registration), ADR 0828, `docs/roadmap/proposals/0828-F-978.json`. No engine files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0828-F-978.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
