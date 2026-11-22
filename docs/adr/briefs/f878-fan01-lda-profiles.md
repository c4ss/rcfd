# Brief: F-878, the FAN-01 rotor alone against the measured LDA profiles behind the trailing edge

Branch: f878-fan01-lda-profiles
Items: F-878
ADR: 0765
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 2; 142879

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f878-fan01-lda-profiles`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f878-fan01-lda-profiles-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md 63470 (the design-point state, the ranked gap, the questions and the maintainer's answers)
- crates/rcfd-api/tests/f870_fan01_design_point.rs 22833 (the design-point test and its heavy records)
- docs/validation-data.md 43585 (the FAN-01 sources: what of the published record was read, lines 68 to 140; note line 98 on the LDA datum)

Also in the repository, as needed: `docs/adr/0086-wave37c-fan01-solution-verification.md`, `docs/reference-data.d/0762-F-870.part`,
`docs/milestone-fan.md`, the FAN-01 tests `crates/rcfd-api/tests/fan01_*.rs`.

## What the maintainer decided (2026-11-22)

ADR 0762 ran FAN-01 at its design point: shaft power within 2.0 per cent of the measurement, pressure rise 41.5 per cent
low (73.99 Pa against 126.46 Pa). Its reading is that 63 per cent of the missing 52.5 Pa is outlet dynamic pressure
that a rotor-only passage discards, so that the rotor's work may be right and the comparison is between a rotor and an
installed fan. That is an inference; no run shows it. The maintainer approved this comparison as the next step, and
decided that the compared quantity of F-870 and whether the outlet model (F-880) is built are settled only after it. So
the one sentence this work must be able to write, with numbers, is: of the missing pressure rise, this part is the
rotor's and this part the installation's, or it cannot be told and why.

## Task

One registration, `docs/adr/0765-f878-registration-<slug>.md`, the first commit of this work, alone.

1. **The measured profiles, traced.** Which LDA data the published record holds (planes, radii, components, operating
   point, time-averaged or phase-resolved), as read from the record itself, with checksum and citation, recorded as a
   fragment of the reference page. `docs/validation-data.md` line 98 says the datum of the axial positions (20 mm and 90
   or 110 mm) is not settled by the sources read: settle it from a published source, or carry both readings through the
   comparison and say what each gives. If the data cannot be obtained or read, stop and report.
2. **Register before the comparison**: the plane or planes, the circumferential average of the solve and how it is
   taken at each measured radius, the band at each radius and what it is from (the LDA's stated uncertainty if a source
   gives it, the mesh family of ADR 0086 for the numerical part; where no uncertainty is stated say so, as ADR 0762 did),
   the Euler work integral and its discretisation, and the rule that divides the missing pressure rise into the rotor's
   and the installation's share, with the condition under which it cannot be divided.
3. **Compare** with the recorded B1 state of ADR 0762 (and B2, B3 where the heavy records exist; do not rerun a solve
   whose state is recorded unless the fields are not kept, and then say so): meridional and circumferential velocity at
   each radius, the Euler work of the measured and the simulated profile, each beside the measured shaft power.
4. **Mind the limits ADR 0762 recorded**: the solve is not converged (a limit cycle, `res_u` 1.2e-3) and the mesh misses
   two quality gates. Say what that does to the profiles (the spread over the cycle, the difference between the meshes)
   and whether the division of task 2 survives it.
5. **Hold it** as a test on the recorded state, of the form ADR 0762 used.

Decision rule to register: done only if the profiles are compared at every measured radius within registered bands and
the division of the missing pressure rise is given with numbers by the registered rule; partial if the comparison is
made and the division cannot be given, or the datum stays open and the two readings disagree, with what each gives;
stopped and reported if the measured profiles cannot be traced to a source that was read.

Open questions for the maintainer, with options and a recommendation, not decided by you: what the result means for
the compared quantity of F-870 and for F-880.

Files you own: a new test file of your own and your reference fragment. Another agent runs check H on the same passage
(F-879) and owns its own files; do not change `crates/rcfd-api/tests/f870_fan01_design_point.rs`.

## Common to this work

The acceptance of F-878 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, as ADR 0760 to 0762 did, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run heavy solves on this machine at the same time and own other files; a test that fails on a file refused by the operating system under that load is reported, not retried until it passes. Register no
limit on wall-clock time. Change no solver source; if a defect of the solver appears, stop and report it. Delete nothing
outside your worktree and your scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0765-F-878.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
