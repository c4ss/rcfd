# Brief: F-861, the process report checks the rules that rest on convention

Branch: f861-process-report-convention-checks
Items: F-861
ADR: 0777
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 1; 98730

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f861-process-report-convention-checks`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f861-process-report-convention-checks-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- scripts/process_report.py 52203 (the checks of --check and how an exceptions list is read)
- scripts/test_process_report.py 29849 (the unit tests to extend)
- docs/process/exceptions/test-root-without-adr.txt 2235 (the form of a dated exceptions list that may only shrink)
- docs/adr/briefs/README.md 1452 (the form of a brief, its reading list and its stated total)

Also in the repository, as needed: `docs/process/documentation-process-review.md` (the line on rules that still rest on convention), the existing lists under `docs/process/exceptions/` (`test-root-without-adr.txt` and `studio-spec-without-record.txt` above all), `scripts/adr_test_index.py` for the index of ADRs to tests, and `git log` for the history of a test file.

## What the maintainer decided (2026-11-23)

The process review of 2026-11-18 lists rules that still rest on convention. The maintainer entered F-861 to make four of them checks of `process_report.py --check`: the one-line note entry of the roadmap, the place in a citation, the reading limit and stated total of a brief, and the record in the leading comment of a test root file or a Studio spec. What exists before a registered date goes on a dated exceptions list that may only shrink. The weekly audit is not part of this item. The sentence this work must be able to write: each of the four checks fails on a case built to break it, shown by a unit test that failed before the change, main passes with its exceptions listed, and no test's code and no default output changed.

## Task

One registration, `docs/adr/0777-f861-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** the date from which each check applies, the form each check accepts
   (taken from the agent rules and the brief README as they stand; where they do not say it exactly, the form you
   register and why), the reading limit of a brief, and one unit test per check that fails before the change.
2. **Change the script and its tests.** Put what exists before the date on dated exceptions lists. Give a header to the
   test root files and Studio specs without one where the record can be found from git history or the ADR index; list
   the rest with the reason. A header is a leading comment only: no test's code changes.
3. Run all script tests (`python -m unittest discover -s scripts -p "test_*.py"`), `python scripts/process_report.py
   --check` and the docs_site test; compile every changed test crate (`cargo test --no-run` for those crates).

Decision rule to register: done if the four checks fail on their registered cases and pass on main with the exceptions listed, and the headers change no code; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: where a rule's wording does not fix the form a check accepts, the options and a recommendation.

Files you own: `scripts/process_report.py`, `scripts/test_process_report.py`, new or shrinking lists under `docs/process/exceptions/`, and the leading comments of test root files under `crates/*/tests` and of Studio specs. Do not edit `docs/roadmap.json`, a brief, an ADR or a generated page.

## Common to this work

The acceptance of F-861 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0777-F-861.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
