# Brief: F-879, the FAN-01 hub pocket: check H of ADR 0183

Branch: f879-fan01-check-h
Items: F-879
ADR: 0766
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 2; 166176

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f879-fan01-check-h`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f879-fan01-check-h-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0183-exp-rot-registration.md 66882 (check H as registered; read all of it)
- docs/adr/0762-f870-registration-fan01-design-point-state-band-and-ranked-gap.md 63470 (the design-point state, the limit cycle, the ranked gap and the maintainer's answers)
- crates/rcfd-api/tests/f870_fan01_design_point.rs 22833 (the design-point test and its heavy records)

Also in the repository, as needed: the FAN-01 stall diagnoses ADR 0175 to ADR 0181 (`docs/adr/017[5-9]-*.md`, `0180`,
`0181`) and their tests `crates/rcfd-api/tests/fan01_*.rs`; the acceptance and note of F-286 in `docs/roadmap.json`;
`docs/milestone-fan.md`.

## What the maintainer decided (2026-11-22)

ADR 0762 ran FAN-01 at its design point and the steady solve did not converge: a limit cycle, `res_u` 1.2e-3 against
1e-5, with a reversed-flow pocket at the hub trailing edge that a dozen diagnoses have narrowed without closing. Check H
of ADR 0183 was registered to class that pocket and has not been run. The maintainer approved running it, and decided
that the convergence rule of F-870 (a steady state, or a rule on a time mean) is settled only after it. So the one
sentence this work must be able to write is: the pocket is steady, physically unsteady or numerical, by the rule
registered before the run, or the run does not decide.

## Task

One registration, `docs/adr/0766-f879-registration-<slug>.md`, the first commit of this work, alone.

1. **Check H as ADR 0183 registered it**, read in full and restated: the set-up, the start state, what is measured and
   the rule that classes the result. If ADR 0183 leaves something open that the run needs (a time step, the time run, a
   threshold), register it here before the run with its reason; if you must depart from ADR 0183, say where and why,
   before the run. If F-286 or anything else check H depends on is not on main in the form it needs, stop and report.
2. **Register before the run** the three classes and what separates them as numbers: for a steady pocket, the decay of
   the oscillation; for physical unsteadiness, an amplitude and frequency of torque and pressure rise that persist and
   are stable under a halved time step; for a numerical cause, a dependence on the time step, the scheme or the mesh
   beyond a registered bound. Name the outcome in which the run does not decide.
3. **Run it** and report the time step, the time run in rotor revolutions, the amplitude and frequency of torque and of
   the pressure rise, the time means of both beside the measured 126.46 Pa and 333.30 W and beside the steady iteration
   means of ADR 0762, and the class.
4. **Say what the class means for F-870**: whether a steady state exists to converge to, and if not, what a rule on a
   time mean would have to state (the window, the settled-mean rule) to be registered; this is a proposal for the
   maintainer, not a change you make.
5. **Hold it** as a heavy record with a fast test on the same path.

Decision rule to register: done only if the run classes the pocket by the registered rule, with the halved time step
where the class needs it; partial if the run is made and does not decide, with what it would take; stopped and reported
if check H cannot be run as registered on current main.

Open questions for the maintainer, with options and a recommendation, not decided by you: the convergence rule of F-870.

Files you own: a new test file of your own and new case texts. Another agent compares the same passage with the measured
profiles (F-878) and owns its own files; do not change `crates/rcfd-api/tests/f870_fan01_design_point.rs`.

## Common to this work

The acceptance of F-879 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0766-F-879.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
