# Brief: F-451, the rcfd duct-modes command

Branch: f451-duct-modes-command
Items: F-451
ADR: 0783
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 163188

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f451-duct-modes-command`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f451-duct-modes-command-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0351-f333-registration-acoustic-wave-solver-and-modal-export.md 48206 (the modal export, the X1 field and question 1, which gave this item)
- crates/rcfd-post/src/duct_modes.rs 41456 (the library's export the command must reproduce bit for bit)
- crates/rcfd-cli/src/cmd_fwh.rs 10680 (a post-processing command of the same kind, as the form to follow)
- docs/acoustics.md 49855 (where the command is documented)

Also in the repository, as needed: `crates/rcfd-cli/src/main.rs` for the command table and its help, the CLI tests under `crates/rcfd-cli/tests/` that hold the help text (find the golden from them), and the tests of ADR 0351 (`python scripts/adr_test_index.py --adr 0351`).

## What the maintainer decided (2026-11-23)

ADR 0351 built the modal export of a duct field in the library and asked (question 1) for a command. The maintainer approved on 2026-11-18 the golden edit this item needs under the class rule of rule 3: a change that only adds entries is re-recorded by the lead, with the diff in the ledger; you report the additive diff and do not edit the golden. The sentence this work must be able to write: `rcfd duct-modes` reads a saved pressure time series on a duct section and writes the export, equal bit for bit to the library's on the X1 field, and refuses a section outside the duct and a non-integer number of periods with located errors.

## Task

One registration, `docs/adr/0783-f451-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** the command's arguments and output, the bit-for-bit check against the
   library on the ADR 0351 X1 field, the two located errors with their texts, and a committed mutation that a
   registered test must catch.
2. **Add the command** and its tests; document it in `docs/acoustics.md`. Run the registered checks, the CLI tests,
   `baseline_hashes`, fmt and clippy with warnings as errors for the crates you touch.
3. Report the help-golden diff as additive, with the diff; do not edit the golden.

Decision rule to register: done if the export equals the library's bit for bit on X1 and both errors are located as registered; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-cli/src/` (a new command module and its entry in the command table), new test files under `crates/rcfd-cli/tests/`, and `docs/acoustics.md`. `crates/rcfd-post/src/duct_modes.rs` only to make an existing function public, additively. No golden.

## Common to this work

The acceptance of F-451 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. The solves it runs are the existing X1 field of ADR 0351. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0783-F-451.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
