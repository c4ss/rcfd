# Brief: F-978, Claude's canvas: why edit_case is refused under load in the process view's B1, then B1 to B3

Branch: f978-edit-case-refused
Items: F-978
ADR: 0830
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 40375

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f978-edit-case-refused`, on the branch `f978-edit-case-refused`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f978-edit-case-refused-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f978-edit-case-refused.md (F-978, ADR 0830) is carried out on the branch f978-edit-case-refused, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f978-edit-case-refused, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0830 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0828-f978-registration-process-view-rows-and-runner-teardowns.md 14930
- crates/rcfd-mcp/tests/canvas_process_f978.rs 10756

Also in the repository, as needed: docs/adr/0826-f978-registration-the-process-view-of-a-case.md (B1 to B3 as registered), `crates/rcfd-workspace/src/` (the case file's replace and its
retries, the `--canvas` reading server's reads of the case), `crates/rcfd-mcp/src/project.rs` (`edit_case`), ADR 0807 (the live
canvas and what it watches), the existing tests of a replace refused by the operating system.

## What the lead decided (2026-11-27)

ADRs 0826 and 0828 ran B1 to B3 once each, as registered, and both runs failed at the first `edit_case` after `new_case`:
`replace ...process-cavity.toml: failed after 12 attempts: Access is denied. (os error 5)`, at 82 % and 99 % CPU. In four
mutated runs of the same build at 70 % or less the replace went through, and in one all of B1 to B3 held. ADR 0828 found no
cause; one candidate is the `--canvas` server holding the new case file open while `edit_case` replaces it. On its question
the lead takes the recommendation, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option
always): a diagnosis under F-978, then one run of B1 to B3.

## Task

One registration, `docs/adr/0830-f978-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the hypotheses for the refusal, each with its file and line and what it predicts (for
   example a reader of the case file without share-delete, in the `--canvas` server or the live watch; an antivirus or
   indexer scan of a newly written file; the replace's retry budget against the load), each with an arm that decides it
   (a run with the reader stopped, a trace of the handles open on the file at the refusal, the replace under a load the
   registration fixes); the remedy each points to; then B1 to B3 once, as ADR 0826 registered them, with the CPU load
   recorded; negative controls and mutations (rule 16); the numbers fixed in advance (rule 15).
2. **Run** as registered. A defect found in non-test code is a finding: fix it on this branch only if the registration
   names the fix and its check, with defaults unchanged; otherwise report it with the remedy as an option.
3. Every test or run that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-978 if the cause is found, B1 to B3 hold in their one run and each mutation is caught;
partial otherwise.

Open questions for the maintainer, with options and a recommendation: a remedy outside the named files, and any question of
scope.

Files you own: `canvas_process_f978.rs`, the fix in the files the registration names, new tests, ADR 0830,
`docs/roadmap/proposals/0830-F-978.json`. No solver files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0830-F-978.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
