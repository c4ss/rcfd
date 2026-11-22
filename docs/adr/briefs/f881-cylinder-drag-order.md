# Brief: F-881, why the mean drag of the cylinder at Re 100 converges at an order near 1.4

Branch: f881-cylinder-drag-order
Items: F-881
ADR: 0771
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 2; 1; 117077

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f881-cylinder-drag-order`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f881-cylinder-drag-order-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0763-f869-registration-the-mesh-converged-mean-drag-of-the-cylinder-at-re-100.md 38969 (the sequence, the order 1.387, the reading of the miss and the maintainer's answer)
- docs/adr/0761-f869-registration-the-mean-drag-of-the-cylinder-at-re-100-against-the-published-spread.md 39949 (the first sequence and its order 1.38)
- crates/rcfd-api/tests/f869_cylinder_mesh_converged_drag.rs 25168 (the meshes, the window and the procedure)

Also in the repository, as needed: `crates/rcfd-api/tests/f869_cylinder_re100_mean_drag.rs`; the O-grid generator the tests call.

## What the maintainer decided (2026-11-22)

F-869 is done: the mean drag of the cylinder at Re 100 is in the published band on the fine mesh and at every order from 1 to 2.5. But on two mesh sequences that differ in time step, far field and mesh the observed order of the drag is the same, near 1.4, where a second-order scheme should give about 2. The maintainer made the reason its own item. The sentence this work must be able to write: the order is held down by this cause, shown by a run that changes it alone, or no candidate moves it.

## Task

One registration, `docs/adr/0771-f881-registration-<slug>.md`, the first commit of this work, alone.

1. **Name the candidates before any run**, among them the grading of the O-grid, the wall force integral and
   the far-field condition, with the run that tests each by changing it alone, and the amount by which a run must move
   the observed order for its cause to be named.
2. **Run them** on the sequence of ADR 0763 (or a cheaper registered one that reproduces the order near 1.4 first), and
   report the order each gives.
3. **Hold it**: a fast test on the gate, the heavy runs as records. Change no solver source: if a fix is found, it is a
   proposal for a further item.

Decision rule to register: done if a candidate is shown to move the order by more than the registered amount, or if every registered candidate is run and none does (that is a result); partial if the runs do not decide.

Open questions for the maintainer, with options and a recommendation, not decided by you: whether a fix, if one is indicated, is issued.

Files you own: a new test file and new case texts of your own. Do not change the tests of ADR 0761 or ADR 0763.

## Common to this work

The acceptance of F-881 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0771-F-881.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
