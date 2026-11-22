# Brief: F-870, FAN-01 at its design point: pressure rise and shaft power against the measurement

Branch: f870-fan01-design-point
Items: F-870
ADR: 0762
Rules version: 5
Model: Claude Fable 5.2
Issued: 2026-11-22
Size (lead): done; 0; 0; 2; 175917

The rules are `docs/process/agent-rules.md`, version 5, in the repository; follow them exactly and cite them by that path and version in the
registration. Worktree: `E:/projects/cfd/rcfd/.claude/worktrees/f870-fan01-design-point`, a new branch from main at the commit that adds this brief. Scratch folder: `E:/projects/cfd/w49i-work/f870-fan01-design-point-scratch`.

## Reading list

- docs/process/agent-rules.md 12991
- docs/milestone-fan.md 20829 (the goal, the chain and the known limits; its chain table is stale, as ADR 0757 notes)
- crates/rcfd-api/tests/fan01_validation.rs 21520 (the present attempt and its known miss at line 490)
- docs/validation-data.md 42311 (section 1, FAN-01: the measured characteristic, its source and uncertainty)
- docs/adr/0181-job49r-a-mesh-change-at-the-fan01-hub-trailing-edge-site.md 39622 (the newest of the FAN-01 diagnoses)
- docs/adr/0757-f867-registration-the-fan-milestone-as-the-whole-design-goal.md 38644 (the fan milestone: headline items and chain)

Also in the repository, as needed: the FAN-01 diagnoses from ADR 0170 to ADR 0181 (`docs/adr/017*.md`, `0180`, `0181`)
and their tests `crates/rcfd-api/tests/fan01_*.rs`; `docs/fan-project-breakdown.md`; the acceptance, gap and note of
F-126 in `docs/roadmap.json`; `docs/test-process.md`.

## What the maintainer decided

The milestone "Assistant-driven fan design" (ADR 0757) has the validation of the fan solve as its first headline: F-870,
entered on 2026-11-21 on the maintainer's answer, the FAN-01 rotor at its design point (1.4032 m^3/s; measured 126.46 Pa
total-to-static and 333.30 W, as `docs/validation-data.md` records). The maintainer asked for it to be issued once a full
gate is green; the gate of 5d2bb6b1 is.

The lead told the maintainer what to expect, and you are held to the same: the present attempt, under F-126, misses
every limit (`fan01_validation.rs` line 490: torque about -37 to -45 per cent), and `docs/milestone-fan.md` names the
limits behind it (the rotating-frame discretisation on bladed sectors, the mesh quality gates that are missed, the
residual stall). A dozen diagnoses (ADR 0170 to 0181) have narrowed it without closing it. This work is not expected to
produce a pass. It is expected to produce the truth about where the design point stands, in a form the milestone can
show, and the shortest registered path to closing it.

## Task

One registration, `docs/adr/0762-f870-registration-<slug>.md`, the first commit of this work, alone.

1. **The state, read and not rerun first.** From ADR 0170 to 0181 and the tests: what was tried, what each showed, what
   the best recorded pressure rise and shaft power at the design point are and on which mesh, scheme and frame, and which
   limits of `docs/milestone-fan.md` each diagnosis confirmed or removed. One table. Say which of these records are held
   by a test on main and which are prose only.
2. **Register the band before any run**, as the acceptance asks: from the measurement's stated uncertainty as the page
   records it and a mesh-convergence estimate, with the mesh quality gates and the convergence rule the acceptance
   names, each stated as a number. If the measurement's uncertainty is not recorded, say so, find it in the published
   source, read it and record it; do not assume one.
3. **Run the design point on current main** with the best registered set-up of the diagnoses, and report pressure rise
   and shaft power against the band, the mesh gates met or missed, and whether the solve converged by its rule. A miss is
   recorded with its number.
4. **The gap, ranked.** For each cause still standing, the evidence for its share of the miss (a number where a
   diagnosis gives one), what would have to be built or changed to remove it, which roadmap item carries that or that no
   item does, and your estimate of its size. This is the part the maintainer will read: which one or two pieces of work
   move the fan milestone's first headline, in what order.
5. **Hold the state in a test.** The design-point comparison as a registered heavy record with a fast test on the same
   path, written so that it records the present miss as a miss (not a pass) and fails if the result moves by more than a
   registered tolerance in either direction, so that an improvement or a regression is seen. Nothing existing is
   un-ignored, ignored or removed without a registration.

You change no solver source in this work. If a diagnosis points to a small, registered fix you believe closes a share of
the miss, propose it as a follow-up with its acceptance; do not make it here.

Decision rule to register: done only if pressure rise and shaft power are both within the registered band on a mesh
that meets the registered gates with a converged solve; partial if the band, the run, the ranked gap and the test are
delivered and the result misses, with the numbers; stopped and reported if the measurement or its uncertainty cannot be
traced to a source that was read.

Open questions for the maintainer, with options and a recommendation, not decided by you: which follow-ups to enter and
in what order.

## Common to this work

The acceptance of F-870 in `docs/roadmap.json` is the measure (read it with a script, never the whole file). The item is
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
roadmap entries as quoted paragraphs and as the proposal file `docs/roadmap/proposals/0762-F-870.json`, checked with
`python scripts/roadmap_proposals.py check`. Check every link of your ADR against the file it names before you commit
(`cargo test -p rcfd-cli --release --test docs_site`): an ADR is append-only and a wrong link cannot be corrected.

## Report (under 500 words)

The registration's commit and the others; the reference with its citation and the page that records it; the band and
what it was registered from; the result with its numbers against the band, at every point compared; what is run by the
gate and what is a heavy record; mutations or negative controls; deviations; what was not run; open questions with
options and a recommendation; the result of `python scripts/process_report.py --check`; your verdict by the registered
rule.
