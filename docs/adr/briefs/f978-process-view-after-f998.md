# Brief: F-978, Claude's canvas: the process view's B1 to B3 on main after F-998, as F-978's closing run

Branch: f978-process-view-after-f998
Items: F-978
ADR: 0836
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 72191

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f978-process-view-after-f998`, on the branch `f978-process-view-after-f998`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f978-process-view-after-f998-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f978-process-view-after-f998.md (F-978, ADR 0836) is carried out on the branch f978-process-view-after-f998, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f978-process-view-after-f998, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0836 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0826-f978-registration-the-process-view-of-a-case.md 25736
- docs/adr/0835-f998-registration-the-plain-write-for-new-case-files-only.md 21010
- crates/rcfd-mcp/tests/canvas_process_f978.rs 10756

Also in the repository, as needed: docs/adr/0828-f978-registration-process-view-rows-and-runner-teardowns.md (the rows, the teardowns), ADRs 0830 and 0832 (the refused replace, its trace),
`crates/rcfd-workspace/src/view.rs` (the process view), the committed mutation patches of F-978's tests.

## What the lead decided (2026-11-27)

F-978's gap was the refused first `edit_case` under load; F-998 is done (ADRs 0834 and 0835: new case files by a plain write
on Windows, 0 of 8900 `edit_case` refused). B1 to B3 held in the runs of ADRs 0830 and 0832. The lead issues F-978's closing
run, on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always): B1 to B3 as ADR 0826 registered them, on main with F-998's remedy, under a registered load, with every committed
mutation of F-978's tests run again.

## Task

One registration, `docs/adr/0836-f978-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: B1 to B3 as ADRs 0826 and 0828 fixed them, under a load the registration fixes (as ADR
   0832's arm H, 16 spinning threads) with the CPU recorded; the number of runs (at least 3 of B1, fixed now) and that every
   one must hold; the F-978 acceptance read clause by clause against them (each step opens the view of its state; none
   missing against the journal); F-978's committed mutation patches run with their expect-fail commands; the numbers fixed
   in advance (rule 15). No code change unless the registration names it and its check.
2. **Run** as registered. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-978 if every registered run of B1 to B3 holds, the acceptance's clauses are met and
each committed mutation fails as expected; partial otherwise.

Open questions for the maintainer, with options and a recommendation: any question of scope.

Files you own: `canvas_process_f978.rs` (additions only), new investigation files, ADR 0836,
`docs/roadmap/proposals/0836-F-978.json`. No solver files, no Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0836-F-978.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
