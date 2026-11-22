# Brief: F-885, the velocity change of each outer iteration, and check H read by its own rule

Branch: f885-check-h-outer-record
Items: F-885
ADR: 0767
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 2; 1; 346865

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f885-check-h-outer-record`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f885-check-h-outer-record-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0766-f879-registration-fan01-check-h-the-hub-pocket-in-time.md 54006 (the first run of check H, its departures D-1 to D-7 and the maintainer's answers)
- docs/adr/0183-exp-rot-registration.md 66882 (check H and its outer-iteration rule as registered)
- crates/rcfd-api/tests/f879_fan01_check_h.rs 37189 (the runs to repeat)
- crates/rcfd-fv/src/pimple.rs 175797 (the outer loop that gets the record)

Also in the repository, as needed: the acceptance of F-286 in `docs/roadmap.json`; `crates/rcfd-fv/src/pimple/`; the baseline hashes test `crates/rcfd-api/tests/baseline_hashes.rs`.

## What the maintainer decided (2026-11-22)

ADR 0766 ran check H of ADR 0183 on FAN-01 and it did not decide: the solver keeps no velocity change per outer iteration, so the outer-iteration rule was read on a pressure ratio that does not measure time accuracy on a state that has become steady. The maintainer chose to add that record and run check H again by ADR 0183's own rule, rather than read the existing records as steady; the convergence rule of F-870 is decided after this. The sentence this work must be able to write: the hub pocket is steady, physically unsteady or numerical by ADR 0183's rule as written, or the run does not decide.

## Task

One registration, `docs/adr/0767-f885-registration-<slug>.md`, the first commit of this work, alone.

1. **The record.** The pimple step keeps, opt-in, the largest velocity change of each outer iteration of a step;
   with the option off every result is bit-identical (the baseline hashes do not change). Register before the code a
   check that shows the recorded values are the changes of the iterations: a closed form on a case small enough to
   compute by hand, or two routes that must agree.
2. **Check H again**, as ADR 0183 and ADR 0766 registered it (same set-up, both time steps, same window, same class
   thresholds), with the outer-iteration rule of ADR 0183 as written: the velocity change of the 5th outer iteration at
   most 1e-2 of the 1st in at least 99 per cent of the window's steps. Name in the registration every departure from
   ADR 0766 and why, before the run.
3. **Class** the pocket, or record that the run does not decide, and say what the class means for the convergence rule
   of F-870 as a proposal for the maintainer, with the caution of ADR 0766 (the time-marched pressure rise on B2 is 8 to
   9 per cent above the SIMPLEC sample).
4. **Hold it**: a fast test of the record and its check on the gate, the two runs as heavy records.

Decision rule to register: done only if the record is shown correct, the defaults are bit-identical and check H classes the pocket by ADR 0183's rule; partial if the record is in and the run does not decide, with what it would take.

Open questions for the maintainer, with options and a recommendation, not decided by you: the convergence rule of F-870, after the class.

Files you own: `crates/rcfd-fv/src/pimple.rs` and `crates/rcfd-fv/src/pimple/` for the record, the least plumbing in `crates/rcfd-api/src/sim.rs` and `crates/rcfd-api/src/schema.rs` to switch it on, and a new test file. Another agent may change `crates/rcfd-fv/src/simple.rs` (F-884); do not touch it. Do not change `crates/rcfd-api/tests/f879_fan01_check_h.rs` or `f870_fan01_design_point.rs`.

## Common to this work

The acceptance of F-885 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
decision rule is registered before the run and is not changed after it; a result that misses is recorded as a miss with
its number; a run that does not decide is recorded as not deciding. No test is ignored, skipped, removed or quarantined
to make a run pass; a registered miss is held as the repository holds known misses, with its limit intact.

Clean-room: reference values only from published sources that were read and that a page of the repository records with
their citation; nothing from memory (rule 4); vendor documentation is never a source. New reference values go in as a
fragment under `docs/reference-data.d/`, not as an edit of a generated page.

Use your own target folder under the scratch folder for every cargo command (`CARGO_TARGET_DIR`). Three other agents run
on this machine at the same time and own other files; a test that fails on a file refused by the operating system under
that load is reported, not retried until it passes. Register no limit on wall-clock time. You may change the transient solver only as the task says: opt-in, defaults bit-identical, the baseline hashes unchanged and run. If anything else in the solver needs changing, stop and report. Delete nothing
outside your worktree and your scratch folder. Commit large run records only as far as they are the evidence; prefer a
summary table with the full record in the scratch folder.

The lead merges with `scripts/push_main.py`. Do not push and do not merge. Commit messages are plain: no agent marker,
no trailers. Rule 9: you do not edit `docs/roadmap.json`, `docs/milestones.json` or a generated page; you write your
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0767-F-885.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
