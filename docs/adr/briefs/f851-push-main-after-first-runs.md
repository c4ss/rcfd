# Brief: F-851, push_main.py after its first real runs

Branch: f851-push-main-after-first-runs
Items: F-851
ADR: 0752
Rules version: 3
Model: Claude Fable 5.2
Issued: 2026-11-21
Size (lead): done; 0; 0; 12; 65752

The rules are `docs/process/agent-rules.md`, version 3, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f851-push-main-after-first-runs`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f851-push-main-after-first-runs-scratch`.

## Reading list

- docs/process/agent-rules.md 11903
- scripts/push_main.py 29989
- scripts/test_push_main.py 11421
- scripts/fmt_check.py 2136
- docs/process/lead-process.md 10303

Also in the repository, as needed: the item F-851 in `docs/roadmap.json` (its acceptance and its note with the findings 9 to
18, each from a named merge), `docs/process/ledger.tsv` (the lines the script wrote; the line of 26856a9027 was corrected
by hand), `scripts/process_report.py`. Outside the repository, for reading: the logs of the lead's runs, `push-*.log` in
`E:/projects/cfd/w49i-work`, and the gate times files in the folders `gate-*` there.

## Task

One registration, `docs/adr/0752-f851-registration-<slug>.md`, the first commit of this work, alone. The work is in
`scripts/` and `docs/process/lead-process.md`; no crate changes.

In scope, each with a unit test in `scripts/test_push_main.py` (or a test file beside `fmt_check.py`) that fails before the
change:

- The eight points of the item's acceptance, as written there.
- Finding 9: a branch that adds an ADR does not fail the process check for an ADR index that the script itself is about
  to regenerate.
- Finding 13: a merge that changes `scripts/` has the script tests of `scripts/` in its plan.
- Finding 14: `scripts/fmt_check.py` has a unit test (batches, a file that fails the check, an empty list).
- Finding 18: the ledger's item field is read from the head of the branch name (`f855-...` gives F-855) or from the
  brief; `-f2-` inside a name gives no item. The corrected line of 26856a9027 stays as it is.

Decide and register, with reasons, how each point is done; where the acceptance leaves a number open (the further runs of
a failed check in point 1, the recorded build figure of point 3), register it before the code. For point 1: a test that is
recorded as flaky is named in the ledger line and in the output, and a test that fails every time still stops the push;
nothing is ignored, skipped or left out of the plan to make a run pass.

Not to change, to report with options and a recommendation under "Open questions": finding 10 (the left-out tests run in
the checkout the lead merges in), finding 11 (the tests the index maps to the merge's ADRs are outside the budget) and
finding 16 (the plan takes part of a crate's test files for a change of one source file). Findings 12, 15 and 17 need
nothing more than what point 1 and the commits named in the note already did; say so if you find otherwise.

Kept as they are: the ledger's columns and every existing line; the meaning of `--run`, `--push`, `--gate-line` and
`--times` when given; what a run that passes at the first attempt writes. The lead uses the script on main while you work,
so nothing outside your worktree is run with `--push`, and no ledger line of main is written by you: tests use a
temporary repository or the script's functions directly.

Check before the report: `python scripts/test_push_main.py` and the other script tests of `scripts/` pass;
`python scripts/process_report.py --check` gives 0 failures; `cargo test -p rcfd-cli --release --test docs_site` passes.
Mutations as the rules ask: at least one committed patch with its `# expect-fail:` line for each of points 1, 2, 4 and
finding 18.

Decision rule to register: done only if every point in scope has a test that fails before its change and passes after,
every existing test of the script passes unedited or its edit is named with its reason, and every mutation is caught;
partial if a point is not done or rests on a manual run; stopped and reported if a point cannot be done without changing
the ledger's format, the gate workflow or a crate: that is a question for the lead, written under "Open questions".

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Run in the background anything that takes longer than a few minutes.

## Report (under 400 words)

The registration's commit, any amendment and the code commits; for each point and finding in scope, its test and whether
it failed before; the registered numbers; mutations; the options and recommendation for findings 10, 11 and 16;
deviations, each with the commit that precedes its corrected run; what was not run; open questions; your verdict by the
registered rule.
