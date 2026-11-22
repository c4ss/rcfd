# Brief: F-876, the replace of run.json refused with access denied in adopt_f288

Branch: f876-adopt-f288-access-denied
Items: F-876
ADR: 0773
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 1; 114152

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f876-adopt-f288-access-denied`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f876-adopt-f288-access-denied-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- crates/rcfd-mcp/tests/adopt_f288.rs 21107 (the test that failed)
- crates/rcfd-project/src/atomic.rs 27409 (the replace with its retries)
- docs/adr/0758-f855-registration-a-stall-of-the-waiter-or-of-the-holder-separated-from-a-fault-of-the-edit-lock.md 52645 (how a fault of the edit lock was separated from load on this machine)

Also in the repository, as needed: the ledger line of the left-out run of d08569e7 in `docs/process/ledger.tsv` and its log if it is still under `E:/projects/cfd/w49i-work/`; the note of F-876 in `docs/roadmap.json`.

## What the maintainer decided (2026-11-22)

In one left-out run, beside three heavy solves of other agents, the test `adopt_f288::a_n_a_run_that_differs_in_one_item_is_not_adopted` failed with a replace of run.json refused as access denied after 12 attempts; it passed in the next runs and never failed in a gate. The maintainer approved finding the cause. The sentence this work must be able to write: the refusal comes from this, shown by a registered control, and the test or the code now passes its registered repeats while still catching an adopted run that differs.

## Task

One registration, `docs/adr/0773-f876-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run** the candidate causes (another handle of the test or of the server on the file,
   a scanner of the machine, the load beside it) and a control for each, and the number of repeats with and without a
   registered load.
2. **Run the controls**; name the cause only as far as a control shows it.
3. **Change the test or the code** under the registration so that the registered repeats pass, with a mutation showing
   the test still catches an adopted run that differs. The test is not ignored, skipped or retried to make it pass, and
   the number of attempts is not raised without the cause.

Decision rule to register: done if the cause is shown and the repeats pass after the change; partial if the failure cannot be reproduced in the registered repeats (that is recorded with the numbers).

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-mcp/tests/adopt_f288.rs`, and `crates/rcfd-project/src/atomic.rs` or the code of rcfd-mcp that holds run.json if the cause is there, but not `crates/rcfd-mcp/src/project.rs`, which another agent changes (F-862): if the cause is there, stop and report. Other agents run solves; touch nothing of theirs.

## Common to this work

The acceptance of F-876 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change the solver nowhere. A change of `atomic.rs` or of rcfd-mcp is allowed only as the task says, with every other test of those crates passing. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0773-F-876.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
