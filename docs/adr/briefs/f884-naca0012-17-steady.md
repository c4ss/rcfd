# Brief: F-884, the steady solver at 17 degrees reaching the attached state the time-accurate run finds

Branch: f884-naca0012-17-steady
Items: F-884
ADR: 0770
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): partial; 1; 1; 1; 214687

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f884-naca0012-17-steady`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f884-naca0012-17-steady-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0764-f868-registration-naca0012-17-degrees-by-an-unsteady-arm.md 44032 (the time-accurate state at 17 degrees and the maintainer's answer)
- docs/adr/0760-f868-registration-naca0012-lift-curve-against-ladson.md 40909 (the steady arm, its set-up and its miss at 17 degrees)
- docs/adr/0167-job49d-naca0012-aoa-sweep-with-the-sa-drag-fixes-at-500-chords.md 30186 (the window rule)
- crates/rcfd-fv/src/simple.rs 86569 (the steady route)

Also in the repository, as needed: `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs`, `naca0012_17_degrees_unsteady_f868.rs`, `scripts/f868_naca0012_lift_curve.py`.

## What the maintainer decided (2026-11-22)

ADR 0764 found that at 17 degrees a time-accurate run of the steady arm's model on its mesh settles to a steady attached flow (cl 1.5516), while the SIMPLEC iteration of ADR 0760 does not reach it. The maintainer counted the time-accurate state for F-868 and made the steady route's failure its own item. The sentence this work must be able to write: the steady solver reaches that state by this registered change, or it does not because of this cause, shown.

## Task

One registration, `docs/adr/0770-f884-registration-<slug>.md`, the first commit of this work, alone.

1. **Diagnose first.** Register before any run the candidate causes of the non-convergence of the SIMPLEC
   iteration at 17 degrees (for example under-relaxation, the start state, the turbulence-model coupling, the corrector
   settings) and a run for each that changes it alone; a cause is named only if its run moves the iteration by more than
   a registered amount.
2. **Fix if the diagnosis shows how**: a registered change of the steady route that leaves every default bit-identical
   (baseline hashes unchanged), after which a steady solve at 17 degrees on M1w-far500 with the set-up of ADR 0760 ends
   steady by ADR 0167's window rule with cl within 0.018 of 1.5516.
3. **Hold it**: a fast test on the gate, the heavy runs as records; ADR 0760's known miss stays as it is.

Decision rule to register: done if the steady solve meets the target by a registered opt-in change, or if the cause is diagnosed and recorded with its evidence; partial if the runs do not decide.

Open questions for the maintainer, with options and a recommendation, not decided by you: whether a change found here becomes a default, if one is found.

Files you own: `crates/rcfd-fv/src/simple.rs` for an opt-in change, a new test file and new case texts. Another agent changes `crates/rcfd-fv/src/pimple.rs`, `crates/rcfd-api/src/sim.rs` and `schema.rs` (F-885); do not touch them: if your change needs them, stop and report. Do not change the tests of ADR 0760 or ADR 0764.

## Common to this work

The acceptance of F-884 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. You may change the steady route only as the task says: opt-in, defaults bit-identical, the baseline hashes unchanged and run. If anything else in the solver needs changing, stop and report. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0770-F-884.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
