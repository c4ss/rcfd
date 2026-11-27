# Brief: F-978, Claude's canvas: how long the System process holds a refused case file, and the replace budget it calls for

Branch: f978-replace-hold
Items: F-978
ADR: 0832
Rules version: 5
Model: Claude Opus 5.5
Issued: 2026-11-26
Size (lead): done; 1; 0; 0; 63643

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f978-replace-hold`, on the branch `f978-replace-hold`, which the lead made at the commit that adds this brief; you start in it. Do not touch the main checkout or another worktree. Scratch folder: `E:/projects/cfd/w49i-work/f978-replace-hold-scratch`.

## Goal

The lead starts your work as a background subagent, with this line as its prompt:

    /goal The brief docs/adr/briefs/f978-replace-hold.md (F-978, ADR 0832) is carried out on the branch f978-replace-hold, checked out in E:/projects/cfd/rcfd/.claude/worktrees/f978-replace-hold, and this conversation shows each of these with the command and its output: (1) AGENTS.md, docs/process/agent-rules.md (version 5) and the brief were read in full before any other work, and read again in full after any summary of the conversation; (2) `git log --reverse --stat origin/main..HEAD` shows the registration of ADR 0832 as the first commit of this work, alone, and every amendment before the code or run it covers; (3) every registered check is listed with its limit, its measured result and met or missed as it came out, and a miss recorded as a miss meets this goal: no limit changed after a run, no test ignored, skipped, removed, loosened or run again to pass; (4) every registered mutation is listed with the test that caught it, or as not caught; (5) the output of the registered run is shown of fmt and clippy of the crates touched, their `--lib` tests and the named test files, `cargo test -p rcfd-cli --release --test docs_site`, `python scripts/roadmap_proposals.py check` and `python scripts/process_report.py --check`, where only a generated file that is not current may fail; a run already shown is not made again; (6) `git status --short` is empty, nothing is pushed or merged, docs/roadmap.json, the ledger, goldens and existing tests are unchanged except as the brief's list of owned files says, and no process this work started is left running; (7) the last message is the report the brief asks for, with the head sha and the verdict done or partial by the registered rule. Stop before that, with the reason reported, where the brief or the rules say stop and report, or after 100 turns.

You run without a person to ask: an action you are denied is left undone and is in your report. End only processes you started. A command of more than two minutes runs in the background and is waited for. Name your model in the registration.

## Reading list

- AGENTS.md 1698
- docs/process/agent-rules.md 12991
- docs/adr/0830-f978-registration-the-refused-edit-case.md 21545
- crates/rcfd-project/src/atomic.rs 27409

Also in the repository, as needed: ADR 0826 and ADR 0828 (B1 to B3, the teardowns), the trace and arms of ADR 0830 (its test
files and investigation folder), `crates/rcfd-mcp/tests/canvas_process_f978.rs`, the existing tests of the atomic replace
(`hammer`, `edit_lock_f755`, `edit_lock_f855` with the `test-hooks` feature), every caller of the replace in the workspace.

## What the lead decided (2026-11-27)

ADR 0830 did not decide the refused `edit_case`: every refusal used the whole 12-attempt budget (587 to 619 ms), no rcfd
process was seen holding a case file, and in 3 of 5 refusals the System process (a kernel-mode handle) held the file during
the refusal; how long that hold lasts was not measured, because the trace stopped looking at a file once a newer one existed.
B1 to B3 held in their one run. On its question the lead takes the recommendation, on the maintainer's standing instruction
of 2026-11-26 (go with the recommended option always): option (a), a follow-up under F-978 whose trace keeps looking at a
refused file until its holder lets go, then F1 (a longer replace budget in `atomic.rs`) if the hold is under 2 s.

## Task

One registration, `docs/adr/0832-f978-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run**: the trace kept on a refused file until no process but the replacing one holds it, with its
   look interval and its control (it sees a hold the test makes, of a registered length); the load and the number of rounds
   (as ADR 0830's arms, or more, fixed now); the measure (the hold's length after a refusal, its largest value and its
   distribution, the holder's process); the rule for F1 (the budget that covers the largest hold measured with a registered
   margin, under 2 s, else no F1 and the finding reported); F1's checks (the replace under the same load refuses none in the
   registered rounds; every existing replace test and caller unchanged; the budget's change does not alter a file's bytes or
   its atomicity); then B1 to B3 once; negative controls and mutations (rule 16), one committed as a patch with
   `--target-dir target/mutation`; the numbers fixed in advance (rule 15).
2. **Run** as registered. F1, if its rule holds, in `crates/rcfd-project/src/atomic.rs` only; no other default changes.
3. Every test that starts a runner ends it (ADR 0828's helpers).

Decision rule to register: done for F-978 if the hold is measured, F1 is made by its rule and its checks hold, B1 to B3 hold
in their one run and each mutation is caught; partial otherwise.

Open questions for the maintainer, with options and a recommendation: a hold of 2 s or more, and any question of scope.

Files you own: `crates/rcfd-project/src/atomic.rs` (F1 only), the trace's test files, new tests and investigation files,
ADR 0832, `docs/roadmap/proposals/0832-F-978.json`. No solver files, no Studio files.

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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0832-F-978.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; mutations or negative controls; deviations; what was not run; open questions with options and a
recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered rule.
