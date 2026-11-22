# Brief: F-882, the loss of total pressure of the simulated rotor, where it arises and what a finer mesh makes of it

Branch: f882-fan01-rotor-loss
Items: F-882
ADR: 0768
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): partial; 1; 2; 1; 191123

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f882-fan01-rotor-loss`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f882-fan01-rotor-loss-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0765-f878-registration-fan01-lda-profiles-euler-work-and-the-division-of-the-missing-pressure-rise.md 65998 (the identity, the measured plane, the loss on B3, B2, B1 and the maintainer's answers)
- docs/adr/0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md 63870 (the design-point state, the meshes and the ranked gap)
- crates/rcfd-api/tests/f878_fan01_lda_profiles.rs 48264 (the profiles and the loss as computed today)

Also in the repository, as needed: `crates/rcfd-api/tests/f870_fan01_design_point.rs`; `docs/milestone-fan.md`; the mesh gates of ADR 0762.

## What the maintainer decided (2026-11-22)

ADR 0765 showed that the missing pressure rise of FAN-01 (52.47 Pa) is not in the rotor's work or its outflow velocity but in loss or recovery, and that velocity profiles cannot divide it between rotor and installation. The simulated loss up to the measured plane falls with the mesh (104.34, 78.75, 62.87 Pa on B3, B2, B1) and is not converged. The maintainer chose this loss budget before the outlet as on the rig (F-880). The sentence this work must be able to write: the simulated rotor loses so much, here, and a mesh family extrapolates it to so much with this band, which leaves so much of the gap for the installation.

## Task

One registration, `docs/adr/0768-f882-registration-<slug>.md`, the first commit of this work, alone.

1. **Divide the loss** of total pressure between the inlet and the measured plane of ADR 0765, by a rule
   registered before the run, into the parts that arise at the blade surfaces, at hub and casing, in the tip gap and in
   the passage interior; show the parts sum to the whole within a registered tolerance.
2. **A finer mesh**: one mesh finer than B1 that also refines the wall-normal direction, within the mesh gates of ADR
   0762; run the design point on it.
3. **Extrapolate** the loss over the family with a band, by a rule registered before the run (the observed order and
   the conditions for a valid extrapolation), and state what the identity of ADR 0765 then leaves for the installation.
4. **Hold it**: a fast test on the gate, the heavy runs as records.

Decision rule to register: done only if the loss is divided by the registered rule and the family gives a valid extrapolation with its band; partial if the run does not decide, with what it would take.

Open questions for the maintainer, with options and a recommendation, not decided by you: whether F-880 is then issued, with what the result says about it.

Files you own: a new test file and new case texts of your own. Do not change `crates/rcfd-api/tests/f878_fan01_lda_profiles.rs` or `f870_fan01_design_point.rs`; another agent works on the wall-pressure data of the same record (F-883) and owns its own files.

## Common to this work

The acceptance of F-882 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0768-F-882.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
