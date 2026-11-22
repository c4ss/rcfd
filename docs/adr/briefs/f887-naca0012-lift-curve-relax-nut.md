# Brief: F-887, steady Spalart-Allmaras without eddy-viscosity relaxation over the NACA 0012 lift curve

Branch: f887-naca0012-lift-curve-relax-nut
Items: F-887
ADR: 0772
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 1; 1; 110418

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f887-naca0012-lift-curve-relax-nut`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f887-naca0012-lift-curve-relax-nut-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0770-f884-registration-naca0012-17-degrees-steady-iteration-diagnosis.md 44085 (the cause at 17 degrees, the setting relax_nut 1.0 and the maintainer's answer)
- docs/adr/0760-f868-registration-naca0012-lift-curve-against-ladson.md 40909 (the twelve angles, the band and the steady set-up)
- scripts/f884_naca0012_17_steady.py 12433 (the driver that ran the setting)

Also in the repository, as needed: `crates/rcfd-api/tests/naca0012_lift_curve_f868.rs`, `scripts/f868_naca0012_lift_curve.py`.

## What the maintainer decided (2026-11-22)

ADR 0770 showed that the steady solver reaches the attached state at 17 degrees when the eddy viscosity is not relaxed (relax_nut 1.0). The maintainer kept every default as it is and asked first for the whole lift curve of ADR 0760 with that setting; whether it becomes a default is asked when this reports. The sentence this work must be able to write: with relax_nut 1.0 the twelve angles end steady and within the band of ADR 0760, or these do not, with their numbers.

## Task

One registration, `docs/adr/0772-f887-registration-<slug>.md`, the first commit of this work, alone.

1. **Register before any run** the twelve angles, the band and the window rule exactly as ADR 0760 and ADR
   0167 have them, with the one change `relax_nut = 1.0`, and what the result means for a default change (the question
   for the maintainer, not decided by you).
2. **Run** the twelve angles and report each against ADR 0760's result with the default setting beside it.
3. **Hold it**: a fast test on the gate, the runs as a heavy record. Change no default.

Decision rule to register: done if all twelve angles are run and reported against the registered band (a miss is recorded as a miss); partial if the runs cannot be made as registered.

Open questions for the maintainer, with options and a recommendation, not decided by you: whether relax_nut 1.0 becomes the default of the steady Spalart-Allmaras route.

Files you own: a new test file and new case texts of your own, a new driver script if needed. Do not change the tests of ADR 0760, 0764 or 0770, or `scripts/f884_naca0012_17_steady.py`.

## Common to this work

The acceptance of F-887 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). A band or a
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0772-F-887.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 450 words)

The registration's commit and the others; what was registered before the run; the result with its numbers against what
was registered; what the gate runs and what is a heavy record; mutations or negative controls; deviations; what was not
run; open questions with options and a recommendation; the result of `python scripts/process_report.py --check`; your
verdict by the registered rule.
