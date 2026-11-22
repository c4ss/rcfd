# Brief: F-869, the mesh-converged mean drag of the cylinder at Re 100

Branch: f869-cylinder-mesh-converged-drag
Items: F-869
ADR: 0763
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 2; 67580

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f869-cylinder-mesh-converged-drag`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f869-cylinder-mesh-converged-drag-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0761-f869-registration-the-mean-drag-of-the-cylinder-at-re-100-against-the-published-spread.md 39949 (the first registration: the band, V1, V2a, the missed V2b, the lead's reading and the maintainer's answers)
- crates/rcfd-api/tests/f869_cylinder_re100_mean_drag.rs 14640 (the tests of ADR 0761)

Also in the repository, as needed: `docs/reference-data.d/0761-F-869.part`, `docs/adr/0069-wave33b-transient-accuracy-coupling.md`,
`crates/rcfd-api/tests/validation_cylinder_re100.rs`.

## What the maintainer decided (2026-11-22)

ADR 0761 found the mean drag coefficient inside the published band [1.314, 1.370] on the fine mesh (1.35247 at dt 0.05,
1.34511 at dt 0.025) and the Richardson value of the mesh sequence at dt 0.05 with the far field at 20 D above it
(1.37479, the known miss V2b). The maintainer took option (a) of its question 1: the item stays partial until a mesh
sequence at dt 0.025 with the far field at 30 D or more shows the fine value and the Richardson value in the same band.
The comparison stays with the option `time_scheme = Bdf2` (question 2); the default step is not your business.

This item is the one open piece of work on the cylinder itself: it decides whether the milestone "Vortex shedding behind
a cylinder" reads "met".

## Task

One registration, `docs/adr/0763-f869-registration-<slug>.md`, the first commit of this work, alone. It is a second
registration under F-869 and does not edit ADR 0761.

1. **Register before any run**: the band of ADR 0761 unchanged, [1.314, 1.370]; the three meshes of the sequence, the
   far-field distance (30 D or more, one registered value), dt 0.025, the averaging window and how its start is chosen,
   the Richardson procedure with its observed order and the condition under which the extrapolation is not valid
   (non-monotone sequence, order outside a registered range) and what is concluded then. ADR 0761 says arithmetic
   "suggests about 1.351": that number is not a result and is not used to choose anything.
2. **Run the sequence** and report on each mesh the mean drag coefficient, the Strouhal number and the lift amplitude,
   the observed order, the Richardson value and the GCI.
3. **Hold it**: the heavy record as an ignored heavy test of the form ADR 0761 used, with the gate-side tests of ADR
   0761 left as they are or extended by a registered change; the known miss V2b of ADR 0761 stays as it is (it is the
   record of that sequence, not of this one). If the new Richardson value misses, it is a second known miss with its
   number.
4. **A negative control or mutation** on the new check.

Decision rule to register: done only if the fine value and the Richardson value of the registered sequence are both in
the band and the extrapolation is valid by the registered condition; partial if the fine value is in the band and the
Richardson value is not, or the extrapolation is not valid, with the numbers; stopped and reported if the fine value
itself leaves the band.

Files you own: `crates/rcfd-api/tests/f869_cylinder_re100_mean_drag.rs` and a new test file of your own.

## Common to this work

The acceptance of F-869 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0763-F-869.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 350 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
