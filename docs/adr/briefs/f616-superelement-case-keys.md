# Brief: F-616, case-file keys for superelements and interface enrichment

Branch: f616-superelement-case-keys
Items: F-616
ADR: 0785
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 159926

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f616-superelement-case-keys`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f616-superelement-case-keys-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0469-f430-registration-substructuring-craig-bampton.md 35833 (the reduced part files)
- docs/adr/0532-f603-registration-craig-bampton-interface-enrichment.md 34925 (the interface enrichment)
- docs/adr/0536-f553-registration-reduced-parts-as-superelements.md 33419 (the bar run from the library, whose results a case file must give in bits)
- crates/rcfd-api/src/solid_case.rs 38769 (the solid case file and its schema entries)
- docs/solid-case-file.md 3989 (the case file's page)

Also in the repository, as needed: `crates/rcfd-fem/src/cms.rs` and `crates/rcfd-fem/src/cms/superelement.rs`, the schema golden and the test that reads it (find it from `crates/rcfd-api/tests/`), `crates/rcfd-api/tests/baseline_hashes.rs`, and the tests of ADR 0536 (`python scripts/adr_test_index.py --adr 0536`).

## What the maintainer decided (2026-11-23)

ADRs 0469, 0532 and 0536 built reduced parts, superelements and interface enrichment in the library; a case file cannot yet ask for them. The maintainer approved on 2026-11-18 the schema golden edit this item needs under the class rule of rule 3: a change that only adds entries is re-recorded by the lead, with the diff in the ledger; you report the additive diff and do not edit the golden. The sentence this work must be able to write: a solid case file names reduced part files and the host node sets they attach to (`superelements`) and the interface enrichment (`interface_enrichment`), the bar of ADR 0536 run from a case file gives the library's results in bits, and a case without the keys gives every result as before.

## Task

One registration, `docs/adr/0785-f616-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before the change** the two keys with their schema entries and errors, the bit-for-bit
   check of the bar of ADR 0536 run from a case file against the library, the check that a case without the keys is
   unchanged in bits (baseline hashes and the case hash tables), and a committed mutation that a registered test must
   catch.
2. **Add the keys** and their tests; document them in `docs/solid-case-file.md`. Run the registered checks, the solid
   case tests, `baseline_hashes`, fmt and clippy with warnings as errors for the crates you touch.
3. Report the schema golden diff as additive, with the diff; do not edit the golden or a hash table.

Decision rule to register: done if the case-file bar equals the library's in bits, a case without the keys is unchanged and the schema diff is additive; partial otherwise.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: `crates/rcfd-api/src/solid_case.rs`, new test files under `crates/rcfd-api/tests/`, and `docs/solid-case-file.md`. `crates/rcfd-fem/src/cms*` only to make an existing function public, additively. No golden or hash table.

## Common to this work

The acceptance of F-616 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. The solver's results must not change: the default bits are held by the baseline hashes. Change no numerics; if a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0785-F-616.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
