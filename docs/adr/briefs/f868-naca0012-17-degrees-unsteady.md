# Brief: F-868, the NACA 0012 at 17 degrees by an unsteady arm

Branch: f868-naca0012-17-degrees-unsteady
Items: F-868
ADR: 0764
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 2; 82873

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f868-naca0012-17-degrees-unsteady`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f868-naca0012-17-degrees-unsteady-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0760-f868-registration-naca0012-lift-curve-against-ladson.md 40909 (the first registration: the band, the sweep, the miss at 17 degrees, the lead's reading and the maintainer's answer)
- crates/rcfd-api/tests/naca0012_lift_curve_f868.rs 17817 (the tests of ADR 0760)
- scripts/f868_naca0012_lift_curve.py 11156 (the driver of the heavy record)

Also in the repository, as needed: `docs/adr/0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md`,
`docs/adr/0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md` (what was tried through stall, read the passages
on unsteady runs in full), `docs/reference-data.d/0760-F-868.part`, the section `naca0012-tmr` of `docs/validation-data.md`.

## What the maintainer decided (2026-11-22)

ADR 0760 found the lift within its band at the eleven angles from 0 to 16 degrees and no steady value at 17 degrees (the
iteration mean 1.0476 against the measured 1.6057, band 0.0803; a known miss). The measured maximum lift is at 17.13
degrees, so 17 is still below it. The maintainer took option (a) of question 1: the item stays partial and an unsteady
arm at 17 degrees is registered under it.

Be clear about what this can and cannot give. A steady solve that does not settle near maximum lift may be an unsteady
flow that a time-accurate RANS run resolves to a time mean near the measurement, or a premature stall of the model on
this mesh that no time-stepping repairs. This work finds out which, and either is a result.

## Task

One registration, `docs/adr/0764-f868-registration-<slug>.md`, the first commit of this work, alone. It is a second
registration under F-868 and does not edit ADR 0760.

1. **Read first** what ADR 0159 and ADR 0167 ran near and beyond stall, steady and unsteady, and what they concluded;
   do not repeat a run that exists.
2. **Register before any run**: the band of ADR 0760 at 17 degrees unchanged (0.0803 on the lift coefficient against
   1.6057); the unsteady set-up (time scheme, time step in chord flow times and why it resolves the shedding you expect,
   start state, time run, the window of the mean and the rule that says the mean has settled); the turbulence model and
   mesh of ADR 0760 unchanged unless a registered reason says otherwise; and a control that the same unsteady set-up at
   an angle where the steady value is in the band (for example 14 or 15 degrees) returns that value, so that a
   difference at 17 is the flow and not the set-up.
3. **Run and report**: the time mean of the lift coefficient at 17 degrees with its settled-mean evidence, the
   amplitude and frequency of the oscillation if there is one, the control angle, and the same for drag for the record
   without a claim.
4. **Say which it is**: an unsteady flow whose mean is within the band, a mean outside the band (premature stall of
   this model and mesh, with the number), or a run that does not settle in the registered time.
5. **Hold it** as a heavy record with a fast test on the same path, and leave the known miss of ADR 0760 as it is: it
   is the record of the steady arm.

Decision rule to register: done only if the time mean at 17 degrees is within the band, the mean is settled by the
registered rule and the control angle reproduces its steady value within a registered tolerance; partial if the control
holds and the mean at 17 degrees is outside the band or not settled, with the numbers; stopped and reported if the
control angle does not reproduce its steady value.

Files you own: `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs`, `scripts/f868_naca0012_lift_curve.py`, new case
texts and a new test file of your own.

## Common to this work

The acceptance of F-868 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0764-F-868.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 400 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
