# Brief: F-850, four supporting checks that reader trial 1 found weaker than their record says

Branch: f850-four-supporting-checks
Items: F-850
ADR: 0778
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-23
Size (lead): done; 0; 0; 0; 151526

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f850-four-supporting-checks`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f850-four-supporting-checks-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/process/trials/reader-trial-1/registration.md 10684 (the trial and its findings)
- docs/adr/0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md 47708 (A1' and A2', and the addendum of 2026-11-19)
- docs/adr/0585-f661-registration-pre-stress-on-the-warping-harmonic-path.md 31518 (check HB-, and the addendum of 2026-11-19)
- docs/adr/0570-f639-registration-curvature-free-fallback-of-the-total-lagrangian-contact-body.md 37517 (HS20m-M and HS10m-M, and the addendum of 2026-11-19)
- docs/adr/0453-f534-registration-rainflow-residue-guard-gerber-message.md 11108 (G2, and the addendum of 2026-11-19)

Also in the repository, as needed: the test files these four ADRs name under `crates/rcfd-fem/tests/` (find them with `python scripts/adr_test_index.py --adr NNNN`), and `crates/rcfd-fem/src/beam_warping.rs`, `geometric.rs`, `contact.rs` and `fatigue.rs` to read, not to change.

## What the maintainer decided (2026-11-23)

Reader trial 1 checked sixteen findings and agreed that four supporting checks are weaker than their record says; the lead recorded each in an addendum of 2026-11-19 to its ADR. None changes a result or a status. F-850 adds the four checks, test-only, in new test files. The sentence this work must be able to write: each of the four records now has a check that holds it as it says, and the mutation of check (2) is caught.

## Task

One registration, `docs/adr/0778-f850-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run** each of the four checks as the acceptance states it, with its limit, its
   negative control where the acceptance names one (the uniform pressure on the Hertz disc, 0.118, refused by the limit
   0.08), and the mutation of check (2) with the command that must catch it.
2. **Write the four checks in new test files** and run them; run the negative control and the mutation, and commit the
   mutation as the item's `*.mutation.patch` beside its test.
3. No existing test or record is edited and no library line changes.

Decision rule to register: done if the four checks pass with their registered limits, the negative control is refused and the mutation is caught; partial if one cannot be held as registered, with its number.

Open questions for the maintainer, with options and a recommendation, not decided by you: none expected; any that arises, with options and a recommendation.

Files you own: new test files under `crates/rcfd-fem/tests/` and their mutation patch. No existing file of a crate.

## Common to this work

The acceptance of F-850 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0778-F-850.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
