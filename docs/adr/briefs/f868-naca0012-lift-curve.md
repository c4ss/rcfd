# Brief: F-868, the NACA 0012 lift curve against the measured lift at Re 6e6

Branch: f868-naca0012-lift-curve
Items: F-868
ADR: 0760
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 2; 130274

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f868-naca0012-lift-curve`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f868-naca0012-lift-curve-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md 30186 (the sweep with the registered comparison; limits from line 161, results from line 237)
- docs/adr/0159-job48y-naca0012-angle-of-attack-sweep-through-stall.md 36265 (the first sweep through stall)
- crates/rcfd-api/tests/aoa_sweep_w49d.rs 8521 (the tests of ADR 0167)
- docs/validation-data.md 42311 (the section `naca0012-tmr` from line 363: the measured data and their citations)

Also in the repository, as needed: `crates/rcfd-api/tests/aoa_sweep_w48y.rs`, `cases/naca0012/`, `docs/reference-data.md`
(315 kB: read the section "NACA 0012" from line 238 with a script), `docs/test-process.md`, `docs/milestones.json`.

## What the maintainer decided

The milestone "Aerofoil lift (NACA 0012)" had one headline item, F-043 (the k-omega SST model), which does not say
whether the lift is predicted. On 2026-11-21 the maintainer approved an item of its own for the lift curve, F-868, and on
that day asked for it to be issued once a full gate is green; the gate of 5d2bb6b1 is. ADR 0159 and ADR 0167 ran lift
curves under no roadmap item (the head of ADR 0167 states cl within 0.013 of the measured lift from 2 to 15 degrees with
steady Spalart-Allmaras); whether those runs, on current main, meet F-868 is what this work decides.

## Task

One registration, `docs/adr/0760-f868-registration-<slug>.md`, the first commit of this work, alone.

1. **Read what exists against the acceptance.** Which angles ADR 0167 compared, against which measured set, with which
   limits, on which mesh and model; which of its tests the full gate runs today and which are ignored as heavy or as known
   misses; what the lift curve does at and beyond stall. Say what of the acceptance of F-868 is already shown by a test
   on main and what is not.
2. **Register the band before any run of yours**, from ADR 0167's limits and from the stated uncertainty or scatter of
   the measured data as the page records it, per angle or per range of angles, with the angles at which no comparison is
   made (and why) named in advance.
3. **Run it on current main** and report the lift coefficient at every registered angle against the band. If the result
   on main differs from ADR 0167's record, say by how much and find the commit or the reason.
4. **Make the comparison a test the gate holds**, or a registered heavy record with a fast test on the same path, so
   that the headline cannot regress unseen. A negative control or mutation shows the check fails for a lift curve that is
   wrong by more than the band (for example the angle of attack offset, or the reference area changed).
5. **State the limits of the claim**: turbulence model, mesh, far-field distance, Reynolds number, the angles missed or
   not made, drag and moment not claimed unless you register them.

Decision rule to register: done only if every registered angle is within its band on current main by a check the gate
runs or a registered heavy record with its fast test, and the negative control fails; partial if some registered angles
miss or the gate-side check is not in place, with the misses listed with their numbers; stopped and reported if the
measured data cannot be traced to a source that was read.

Files you own: a new test file and case files of your own; `crates/rcfd-api/tests/aoa_sweep_w49d.rs` only if a registered
change needs it. Change no solver source unless a registered defect requires it; then stop and report first.

## Common to this work

The acceptance of F-868 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). The item is
a headline item of a simulation milestone (ADR 0756, ADR 0757): its state is what the roadmap shows the maintainer as
"met" or "not met" for that simulation, so a result that misses is recorded as a miss with its number and is never
written as a pass, and a band is registered before the run from stated sources and is not widened after it.

Clean-room: every reference value comes from a published source that was read and that a page of the repository records
with its citation; a value cited from memory is not used (rule 4). Vendor documentation is never a source.

No test is ignored, skipped, removed or quarantined to make a run pass. A heavy run that the full gate cannot hold is
registered as a heavy record with a fast test on the same path that the gate runs, as `docs/test-process.md` describes;
say which it is. Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`); two
other agents run heavy solves on this machine at the same time, so register wall-clock expectations with that in mind
and do not register a limit on time. Delete nothing outside your worktree and your scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0760-F-868.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 400 words)

The registration's commit and the others; the reference with its citation and the page that records it; the band and
what it was registered from; the result with its numbers against the band, at every point compared; what is run by the
gate and what is a heavy record; mutations or negative controls; deviations; what was not run; open questions with
options and a recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered
rule.
