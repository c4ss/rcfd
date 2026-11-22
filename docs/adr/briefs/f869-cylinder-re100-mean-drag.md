# Brief: F-869, the cylinder at Re 100: the mean drag coefficient against the published values

Branch: f869-cylinder-re100-mean-drag
Items: F-869
ADR: 0761
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 2; 48075

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f869-cylinder-re100-mean-drag`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f869-cylinder-re100-mean-drag-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/adr/0060-wave30b-incompressible-validation.md 26984 (the checks K1 to K4 and their results)
- crates/rcfd-api/tests/validation_cylinder_re100.rs 8100 (the tests; note which are ignored and why)

Also in the repository, as needed: `crates/rcfd-api/tests/validation_cylinder_re100_w31.rs`,
`validation_cylinder_re100_w33b.rs`, `w48t_cylinder_spokes.rs` and the ADRs they name, `docs/reference-data.md` (315 kB:
read the section "Cylinder wake" from line 200 with a script), `docs/test-process.md`, `docs/milestones.json`, the
acceptance and note of F-029 in `docs/roadmap.json`.

## What the maintainer decided

The milestone "Vortex shedding behind a cylinder" had one headline item, F-029 (transient PISO/PIMPLE), whose acceptance
is the Strouhal number alone. On 2026-11-21 the maintainer approved a second headline item for the mean drag, F-869, and
asked for it to be issued once a full gate is green; the gate of 5d2bb6b1 is. Of the 19 items of the milestone F-869 is
the only open one that is about the cylinder itself, so this item decides whether the milestone reads "met".

`validation_cylinder_re100.rs` compares the drag today under no item. Its tests are ignored: one as heavy (about 7
minutes), one as a known miss of K1 and K3 (Strouhal number -3.8 per cent, GCI 4.1 per cent), one as a diagnosis. So the
comparison exists and the gate does not run it, and one of its checks is a recorded miss.

## Task

One registration, `docs/adr/0761-f869-registration-<slug>.md`, the first commit of this work, alone.

1. **Read what exists against the acceptance.** What K1 to K4 are, their limits and their recorded results; what the
   later cylinder tests (w31, w33b, w48t) changed and found; what the mean drag coefficient is on current main on each
   mesh, and against which published values with which citations the page records. Say plainly whether the known miss
   of K1 and K3 touches the drag or only the Strouhal number and the mesh convergence.
2. **Register the band before any run of yours**: the published values the page records for the mean drag coefficient
   at Re 100 with their sources, what the band is taken from (the spread of the published values, a stated uncertainty)
   and its width, the mesh or meshes and the averaging window, and the convergence evidence you will ask for (the acceptance
   says "what the band is taken from and its width are stated"). If the page records too few sources to justify a band,
   find published sources, read them, and record them on the page with their citations before use.
3. **Run it on current main** and report the mean drag coefficient against the band, with the Strouhal number and the
   lift amplitude beside it for the record.
4. **Make it a test the gate runs.** The acceptance asks for "tests that the full gate runs": a heavy ignored test does
   not meet it. Either the comparison runs in the gate within the gate's registered time limits, or a registered heavy
   record carries it with a fast test on the same path, and you say which and why. The existing ignored tests are not
   un-ignored or removed without a registration of what that changes; a known miss stays a known miss.
5. **A negative control or mutation** shows the check fails for a drag that is wrong by more than the band.

Decision rule to register: done only if the mean drag coefficient is within the registered band on current main by a
check the gate runs or a registered heavy record with its fast test, the band's sources are recorded, and the negative
control fails; partial if the drag is within the band but the gate-side check is not in place, or the result depends on a
mesh whose convergence is the recorded miss, with what is missing stated; stopped and reported if the drag misses the
band, with the number.

Files you own: a new test file of your own and the section "Cylinder wake" of `docs/reference-data.md`. Change no solver
source; if a defect of the solver appears, stop and report it.

## Common to this work

The acceptance of F-869 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). The item is
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0761-F-869.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 400 words)

The registration's commit and the others; the reference with its citation and the page that records it; the band and
what it was registered from; the result with its numbers against the band, at every point compared; what is run by the
gate and what is a heavy record; mutations or negative controls; deviations; what was not run; open questions with
options and a recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered
rule.
