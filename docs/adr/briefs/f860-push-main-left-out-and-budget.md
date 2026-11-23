# Brief: F-860, push_main.py: the left-out run names its commits, the ADR's tests count in the budget, changed-file binaries first

Branch: f860-push-main-left-out-and-budget
Items: F-860
ADR: 0776
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 0; 194023

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f860-push-main-left-out-and-budget`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f860-push-main-left-out-and-budget-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- scripts/push_main.py 56559 (the per-merge selection, the budget, the left-out run and the ledger line)
- scripts/test_push_main.py 56719 (the unit tests to extend)
- docs/test-process.md 67754 (section 7.4, what is checked per merge)

Also in the repository, as needed: `docs/process/ledger.tsv` (read its header and recent lines with a script), `scripts/adr_test_index.py` and its tests for the index of ADRs to tests, `docs/process/lead-process.md`.

## What the maintainer decided (2026-11-22)

The lead's push script selects the test binaries of a merge within a budget and leaves the rest to a left-out run that builds from the working tree. Three gaps were found in use: a left-out run does not record which commit it built (the checkout can move during the run); the tests of the merge's own ADRs can be cut by the budget; and inside a touched crate the binaries are not ordered by what changed. The maintainer approved closing all three. The sentence this work must be able to write: each of the three points has a unit test that failed before the change and passes after, and nothing else the script writes changed.

## Task

One registration, `docs/adr/0776-f860-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** each of the three points of the acceptance as a unit test that fails
   before the change, the rule of naming for point 3, and the check that the ledger's columns, every existing line and
   what a run that passes at the first attempt writes are unchanged.
2. **Change the script** and run all script tests (`python -m unittest discover -s scripts -p "test_*.py"`) and
   `python scripts/push_main.py` as a dry run on the current tip.
3. Do not run `--run` or `--push`: the lead runs them.

Decision rule to register: done if the three registered tests pass after the change and the unchanged-output check holds; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `scripts/push_main.py` and `scripts/test_push_main.py`. Do not edit `docs/process/ledger.tsv` or any other script; the lead uses the script on main while you work, so nothing of yours reaches main before the merge.

## Common to this work

The acceptance of F-860 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. This item runs no solve. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0776-F-860.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
