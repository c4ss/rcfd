# Brief: F-842, the inertia and the retry of the nonlinear driver in the solid case's output

Branch: f842-solid-inertia-in-output
Items: F-842
ADR: 0780
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 135478

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f842-solid-inertia-in-output`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f842-solid-inertia-in-output-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0719-f780-registration-driver-retry-with-the-indefinite-tangent-after-a-refused-factor.md 33802 (NewtonOptions::indefinite_retry, the inertia and the retry)
- docs/adr/0694-f774-registration-the-corner-element-of-rule-l-at-the-punch-corner-and-the-equilibrium-past-the-predictor.md 45927 (the 16^3 block that reports nu = 1 at full load)
- crates/rcfd-api/src/solid_case.rs 38769 (where the solid case reports its increments)
- docs/solid-case-file.md 3989 (the case file and its output)

Also in the repository, as needed: `crates/rcfd-fem/src/nonlinear.rs` (the driver and its options), `crates/rcfd-api/src/schema.rs` and its schema golden, the CLI's solid summary under `crates/rcfd-cli/src/`, and `crates/rcfd-api/tests/baseline_hashes.rs` with the hash tables.

## What the maintainer decided (2026-11-23)

F-780 gave the nonlinear driver a retry with the indefinite tangent after a refused factor, behind `NewtonOptions::indefinite_retry`. The maintainer approved on 2026-11-18 the golden, schema or hash-table edit this item needs under the class rule of rule 3: a change that only adds entries is re-recorded by the lead, with the diff in the ledger; you report the additive diff and do not edit the golden. The sentence this work must be able to write: with the case's opt-in, every accepted increment reports its inertia and retry flag in the case output and the CLI summary, with a warning when an accepted state has nu > 0, and the 16^3 block reports nu = 1 at full load; by default nothing changes.

## Task

One registration, `docs/adr/0780-f842-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** the case-file key of the opt-in, where the inertia and the retry flag
   are written, the warning's text and when it is given, the check that the 16^3 block of ADR 0694 run through the case
   file reports nu = 1 at full load, and the checks that the default bits, the schema golden and the hash tables are
   unchanged with the opt-in off.
2. **Make the change** and run the registered checks, the baseline hashes, the schema golden test, the unit tests of
   the crates you touch, and a committed mutation that a registered test must catch.
3. Report any golden or schema diff as additive, with the diff; do not edit the golden, the schema golden or a hash
   table.

Decision rule to register: done if the opt-in reports nu = 1 for the 16^3 block, the warning is given as registered and the default is unchanged in bits; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-api/src/solid_case.rs`, the CLI's solid summary, `docs/solid-case-file.md`, and new test files. `crates/rcfd-fem/src/nonlinear.rs` only if the inertia is not yet returned to the caller, and then only additively. No golden, schema golden or hash table.

## Common to this work

The acceptance of F-842 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. The solver's results must not change: the default bits are held by the baseline hashes. Change no numerics of the driver; if a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0780-F-842.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
