# ADR 0779: F-875 registration: a simulation milestone for the structure and vibration of a fan rotor

Status: **REGISTERED 2026-11-23, written before any code of F-875 and before any run of its checks.** Before this text no
definition, mapping or sample of F-875 was written. What was done: reading (below) and reading scripts over
`docs/roadmap.json` kept in the scratch folder (`items.py`: id, status, line of work, title, deps and acceptance of every
item and the dependency closures of F-387, F-406 and F-436; `show.py`: the acceptance and gap of named items; `grepn.py`:
the text around the first match of a pattern in title, acceptance, gap and note). Results go in addenda at the end;
nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f875-fan-structure-milestone.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f875-fan-structure-milestone` from main d10ea0bd, scratch
`w49i-work/f875-fan-structure-milestone-scratch`. This work writes no library, script or test code and runs no solve.

Size (agent): done; 0; 0; 2; 330000

## What is asked

Roadmap item F-875, entered on the maintainer's answer of 2026-11-21 to ADR 0757 question 5, option (c): the structure
and vibration of a fan rotor stay outside the milestone `fan` and get a milestone of their own, with the cyclic rotor
that ADR 0756 proposed. A milestone for the structural fan demonstrations is defined in `docs/milestones.json` with its
cases, its published reference as a page of the repository records it, and its headline items (F-406 or F-436, or the
reason for another); the five fan-titled structural items F-387, F-406, F-436, F-593 and F-604 and the cyclic rotor are
placed; every item of the roadmap is read against the membership rule of ADR 0756, every membership has a reason, a
sample drawn by seed is read a second time and its disagreements are reported, and one command adds the milestone on
main and leaves the others byte for byte as they are.

## What was read

Whole: the brief; `docs/process/agent-rules.md` (version 5); ADR 0756 (its lines cut at 1500 characters, the last 41 at
700), ADR 0757 and ADR 0775; `scripts/roadmap_milestones.py`; `docs/milestones.json`; the head and the first 36 rows of
`docs/adr/0775-f871-mapping.tsv`; `docs/roadmap/proposals/0775-F-871.json` (its first 1500 characters). Of
`docs/structural-solver.md`: lines 1 to 20, the sections "The fan-blade demonstration", "Limits" and "The blade mesher in
the library", the paragraphs on cyclic symmetry, modes and rotating parts of "What the solver does today" and the section
"Not there or partial" (lines cut at 600 characters). Of `docs/fan-project-breakdown.md`: the lines that name structure
or vibration (the parts E1 to E5 among them) and part G. Of `docs/milestone-fan.md`: the lines that name structure or
vibration. Of `docs/reference-data.md`: the lines that match fan blade, ADR 0500, F-387, Euler-Bernoulli, Campbell or
cyclic (cut at 330 characters), and the heading and the row "Published fan or blade data" of the section "Wave 32A: fan
geometry and sector mesh (ADR 0065)". Of ADR 0500: the lines that name a source, a closed form or the plate window. Of
ADR 0290: the section "ADR 0309 (SI-19)" and lines 500 to 507 and 972 to 979. Of ADR 0310: the section "ADR 0339
(SA-29)". Of ADR 0309 and ADR 0339: the lines that name a sector, the case file, the Coriolis term, a harmonic or the
unbalance. Of `crates/rcfd-fem/tests/sb_fan_blade.rs`: lines 1 to 60, 94 to 97, 242 to 283 and 681 to 708, its `use`
lines and the list of its functions and tests, and its lines that name `StaticSolver`, `ResidualTest` or `refinement`. Of
`crates/rcfd-fem/tests/sb_fan_blade_builder.rs`: lines 1 to 30, its `use` lines and the list of its functions and tests.
The head of `crates/rcfd-mesh/src/surface.rs`. Lines 1 to 45 of `scripts/roadmap_progress.py` (the lines of work). The
list of test files that name a fan or a blade. Of `docs/roadmap.json`, by script: for all 792 items the id, status,
line of work and title (the 308 titles of the line "Structural solver" cut at 125 characters, the 484 others at 78);
the acceptance and gap of F-875, F-387, F-406, F-436, F-593 and F-604 in full; the acceptance and gap cut at 300 to 520
characters of F-370 to F-377, F-380 to F-383, F-386, F-388, F-393 to F-399, F-403, F-408 to F-418, F-420, F-435, F-455,
F-457, F-489, F-498 to F-500, F-507, F-547, F-554, F-555, F-680, F-131 to F-133, F-237, F-306, F-324 and F-080; for
every item 60 to 80 characters around the first match of the names of the two fan-blade test files, the four ADRs and
the four items of the demonstrations, of `ComputerFan`, `fan_design` and "ADR 0118", of the tetrahedral mesher and the
surface and fan modules of `rcfd-mesh`, and, in the structural line, of iterative refinement and `ResidualTest`.

## Readings (before any mapping)

* `docs/roadmap.json` at d10ea0bd holds 792 items: 398 done, 299 partly done, 95 not started (69 per cent). 112 items
  carry the key `milestones`; `docs/milestones.json` defines nine milestones (`vortex-shedding`, `fan`, `aerofoil-lift`
  and the six of ADR 0775). No item of the line "Structural solver" carries the key.
* The commit d10ea0bd is the base of this branch and the tree every figure below is taken on. The lead's main has moved
  on since; between d10ea0bd and its tip of this hour `docs/milestones.json`, `docs/roadmap.json` and the two scripts
  `roadmap_milestones.py` and `render_roadmap.py` do not differ.
* `--apply MAPPING --definitions FILE` replaces the members of every milestone the mapping names and of no other, and
  copies FILE over `docs/milestones.json` whole. A mapping that names only the new id leaves the keys of the nine; the
  new id is written last in an item's list because the definitions are in that order. The definitions file must carry
  the nine objects unchanged in bytes. No defect of the script stops this work; the script is not changed.
* **The three demonstrations.** F-387 (done, ADR 0500): one blade of the parametric 120 mm fan as a clamped solid, the
  static response to the centrifugal and a pressure load and the modes at rest. F-406 (not started, stub ADR 0309): the
  Campbell table of that blade at speed, the bladed hub by cyclic symmetry against the full rotor, the response to
  blade-passing forcing. F-436 (not started, stub ADR 0339): a polymer blade (orthotropy, temperature, creep, fatigue at
  a Campbell crossing) and the radiated power of the frame. Each acceptance is a list of comparisons that can be met or
  missed. F-406 depends on F-387 and F-436 on F-406.
* **Runs.** Two test files run a fan blade as a solid today: `crates/rcfd-fem/tests/sb_fan_blade.rs` (F-387) and
  `crates/rcfd-fem/tests/sb_fan_blade_builder.rs` (F-593, the results of F-387 through the library blade mesher, bit for
  bit). F-406 and F-436 have no test file: the file ADR 0339 plans, `sa_fan_polymer.rs` under the tests of `rcfd-api`,
  does not exist. The solid case file cannot describe the blade or the centrifugal load (the gap of F-593).
* **What the run of F-387 uses, from its text.** `rcfd_fem::builders` (`tet_box`, `upgrade_to_tet10`),
  `convert::from_tet_mesh`, `loads::nodal_load_vector`, `mass`, `modal::modal_analysis`, `recover`, `solve::solve_static`
  with `StaticSolver::Direct` and `.refinement(1)`, `rcfd_linalg::eigen::ResidualTest::Backward`, `rcfd_linalg::CsrMatrix`,
  `rcfd_mesh::fan` (`AxialFan`, its blade surface), `rcfd_mesh::fan_design::ComputerFan::default()`,
  `rcfd_mesh::surface::TriSurface` and `rcfd_mesh::tetmesh::tetrahedralize` (the mesher route D). It reads no case file
  and writes no VTK file. `rcfd_mesh::surface` names F-232 in its head; the note of F-132 names `rcfd_mesh::tetmesh`; ADR
  0757 records `rcfd_mesh::fan` under F-133 (ADR 0065); `ComputerFan` (ADR 0118) is the subject of no item (ADR 0757,
  link L1), and F-713 leaves its default unchanged.
* **The reference.** No measured or published datum of this blade, rotor or frame is recorded in the repository:
  `docs/structural-solver.md` (head and section "Limits": "Not validated. No measured or published data exist for this
  blade"), `docs/fan-project-breakdown.md` part G (G1 and G2 missing), `docs/reference-data.md`, section "Wave 32A: fan
  geometry and sector mesh (ADR 0065)", row "Published fan or blade data: none read", and ADR 0310 ("No reference value
  exists for the blade or the frame"). The comparisons of the three acceptances are closed forms and identities derived
  in the registrations, and mesh change: for F-387 the centrifugal resultant `rho omega^2 V r_c`, the pressure force,
  the strain energy as half the work of the loads, and the first bending frequency of the flat plate between the
  Euler-Bernoulli beam value and that value divided by `sqrt(1 - nu^2)`; `docs/structural-solver.md`, section "The
  fan-blade demonstration", records the measured side and the two bounds (142.9 and 154.5 Hz). For F-406 and F-436 the
  comparisons are named by their acceptance (the Rayleigh quotients of the modes at rest, the full rotor of the same
  mesh, the modal estimate of the resonant amplitude, the scaling of the frequencies with the modulus, the Goodman
  formula, the equivalent radiated power recomputed from the output) and no page records a number for them, because
  they are not run. I read these pages, not a paper; no paper is cited by this work.
* **Dependency closures** (every item reached through `deps`): F-387 35 items; F-406 44; F-436 62; the union with the
  three items themselves 63, of which 36 are in the structural line.
* **The cyclic rotor.** ADR 0756 proposed "Rotor with cyclic symmetry (structural)" with the words "none found by
  title; an item would have to be named" for its headline. The acceptance of F-406 holds the clause "Bladed hub with Z
  blades: the sector frequencies for harmonic indices 0 to Z/2 equal those of the full rotor", so F-406 is the item
  that carries the cyclic rotor of a fan, and F-394 (cyclic symmetry by harmonic index, done) is its direct dependency.
  That clause names no speed: ADR 0290 says that the cyclic package "is at rest", and the rotating sector (F-500 and its
  follow-ups) is in no dependency list of the three demonstrations. 24 titles of the structural line match the
  registered pattern for the cyclic rotor (below), F-406 among them.
* **Structure and vibration beyond the demonstrations.** Part E of the breakdown also names the unbalance force (E1),
  the rotor on bearings (E2), the load export (E4) and coupling with the flow (E5). The roadmap holds items for them
  (F-416 and its follow-ups, F-237, F-384, F-385, F-400, F-401, F-421, F-434); none is in the closure of a demonstration.

## The definition (fixed now)

One object, appended after `packed-bed`; every byte of the nine existing objects is kept.

* `id` `fan-structure`. `title`: "Structure and vibration of a fan rotor". `page`: `docs/structural-solver.md`.
* `result`: "The rotor of the parametric 120 mm fan as an elastic solid: one blade clamped at the hub under the
  centrifugal and a pressure load with its modes at rest, its Campbell table at speed, the bladed hub by cyclic symmetry
  against the full rotor, the response to blade-passing forcing, and a polymer blade (orthotropy, temperature, creep,
  fatigue at a Campbell crossing) with the radiated power of the frame: demonstrations checked against closed forms,
  identities and mesh change, not against a measured blade."
* `cases`: `crates/rcfd-fem/tests/sb_fan_blade.rs`, `crates/rcfd-fem/tests/sb_fan_blade_builder.rs`. These are the runs
  of record of the rule below. No file is named for F-406 and F-436, which have none.
* `reference`: in the words of the reading above: that no measured or published datum of the blade, the rotor or the
  frame is recorded, with the three pages that say so; the closed forms and identities of F-387 with the two plate
  bounds, as recorded in `docs/structural-solver.md`, section "The fan-blade demonstration", and derived in ADR 0500;
  and that the comparisons of F-406 and F-436 are those their acceptance names and are not yet run.
* `headline`: F-387, F-406, F-436.

**Why three headline items, and F-387 among them.** The item asks for "F-406 or F-436, or the reason for another". The
milestone is the structural fan demonstrations, and the roadmap holds three, each with an acceptance of its own that
can be met or missed; F-436 does not replace F-406 (it takes the bits of F-406 as its starting point and adds the
material and the frame), so both are headline items. F-387 is added because it is the only demonstration that has been
run: without it the milestone would have no run of record and would read "not started" although the blade at rest is
done. With the three the milestone reads "not met" until F-406 and F-436 are done, by the script's rule. Taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation); the item leaves the choice to the
registration.

**What the word of this milestone can say.** Its headline comparisons are not with a published or measured reference.
When the three items are done the script will write "met", and that will mean that the demonstrations pass their
closed forms and identities, as the `result` and the `reference` say in words. Whether the milestone should also have a
headline item that compares with a published blade or bladed disc is an open question for the maintainer (the
addendum); it is not decided here and no such item is assumed.

**Scope fixed here and named.** The milestone is the three demonstrations and what they are built from. The rotor on
bearings, the unbalance response, the load export and the coupling with the flow are not in it: no demonstration's
acceptance or dependency list names them. They are listed in the addendum, not mapped.

## The membership rule (fixed before the mapping)

The classes M1 to M4 and N1 to N5 of ADR 0756 are kept; no class is added. They are read for this milestone as follows.

* M1, headline: F-387, F-406, F-436.
* M2, used. (a) By a run of record: the text of `sb_fan_blade.rs` or `sb_fan_blade_builder.rs` (a `use` line or the
  set-up) names what the item delivered, or the item's own text names the function the run calls. (b) By a
  demonstration that has not been run (F-406, F-436), where there is no run text to read: (b1) the item is a direct
  dependency of that headline item, the roadmap's own statement of what the demonstration is built from; or (b2) the
  item is deeper in its dependency closure and a member is built on what it delivered, so that the set-up the
  acceptance states cannot be computed without it: the data model, the elements, the assembly and static solve, the
  factorisation, the eigensolver, the constraint transformation of the cyclic sector, the geometric stiffness of the
  pre-stress, the Newton driver of the creep law, the temperature field of the tabulated modulus, the scaffold of a
  tier whose packages are members, the case file and the output the acceptance says the results are written from or
  to. Being in the closure alone is not enough: an analysis, a material, a solver or a mesh source that the closure
  holds and the stated set-ups do not use is not a member (the transient integrator, the viscoelastic material, random
  vibration, the iterative solver, the Gmsh reader, the flow solver's items).
* M3, data: the item holds the case definition or the reference data a comparison reads. None is expected: the blade
  is `ComputerFan::default()`, which no item holds, and the references are closed forms.
* M4, trust: the item's subject is a defect, a refused set-up or a missed limit that a run of record met, or a mesh
  study of that run. An item about the tetrahedral mesher is M4 only if its text names this blade.
* Not a member: N1 an item that uses a fan blade or the fan mesh as a test of something else (the flow meshes of the
  passage, the Studio drawing a fan mesh); N2 an item that consumes the milestone's result; N3 workspace, CI, process,
  documentation, Studio and assistant items, F-001 and F-003 among them (the reading of ADR 0756); N4 an alternative, an
  extension or another path of a member's capability that no run of record uses and no demonstration's acceptance or
  dependency list names: other elements (hexahedra, shells, beams), the same analysis on the transformation path or
  with another solver, the rotating and whirl paths of a cyclic sector, the rotor on bearings, contact and the press
  fit, materials the acceptances do not name, the coupling with the flow; N5 no relation.
* The structural line is not read by title only, as ADR 0756 and ADR 0775 read it for flow milestones: its 308 items
  are read by title, and by acceptance where the item is in a closure, matches a pattern below or is a candidate for
  M2 (a). The 484 other items are read by line of work and title, and by acceptance where a run names their module.
* An item of the milestone `fan` is not a member for that reason; the flow-side fan items are N1 or N5 unless a run of
  record uses them.

**The two lists that the item asks to be placed.** (i) The five fan-titled structural items F-387, F-406, F-436, F-593
and F-604. (ii) The cyclic rotor: every item of the line "Structural solver" whose title matches the pattern `cyclic`,
`sector` or `full.rotor` without regard to case. Each item of the two lists is a member with its reason or is listed
in the addendum with the class and the reason it is not.

Every membership gets one line `<item>` tab `fan-structure` tab `<reason>` whose reason starts with M1, M2, M3 or M4.
If a class of items cannot be placed by M1 to M4 and N1 to N5, the work stops and reports the class.

## The check of the mapping (fixed before the mapping exists)

The mapping file is committed first. Then `sample.py` (scratch) draws with `random.Random(875779)`, in this order: 20
items by `sample` from the members sorted by item number (all of them if there are fewer than 20); 10 by `sample` from
the non-members sorted the same way; 10 by `sample` from the non-members not yet drawn that are doubtful, sorted the
same way. An item is doubtful if it is in the dependency closure of F-387, F-406 or F-436, or if its title holds,
without regard to case, one of: cyclic, sector, rotor, rotating, blade, fan, campbell, whirl, coriolis, centrifugal,
spin, modal, modes, harmonic, fatigue, creep, orthotrop, radiat, `case.file` (235 items before any mapping). The 40
items, sorted by item number, are read a second time by a fresh agent that is given the two sections "The definition"
and "The membership rule" of this ADR as a file, and for each item its id, title, status, line of work, deps,
acceptance and gap, and whether it is in the closure of each headline item, in a folder of its own, with the
repository's code and topic pages to consult; it is told not to open this ADR, the mapping, `docs/roadmap.json`,
`docs/milestones.json`, the mappings of ADR 0756, 0757 and 0775 or the scratch folder, and not which draw an item came
from. Independence rests on that instruction, not on isolation. If no agent can be started, the addendum says how the
second reading was made. Reported: the disagreements in each draw, every disagreeing item with both readings and the
reading taken with its reason. A correction is a separate commit of the mapping; the items are not redrawn.

## Table of checks

Every clause of the acceptance of F-875 first. No row has a numeric limit of my own; each limit is a clause.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | the milestone is defined as registered: cases that exist, a reference that names the pages and sections of the repository that record it, headline items that are items of the roadmap | defined, or the missing part reported and the milestone not added | acceptance of F-875 (first clause); the brief, task 1 | defined | n/a | code verification |
| C2 | every item is read; each membership has a one-line reason that starts M1 to M4 | 792 of 792 items read; 0 memberships without a reason | acceptance of F-875 ("every item of the roadmap is read", "every membership has a reason") | as stated; 30 to 45 members | n/a | code verification |
| C3 | the two lists are placed: each of the five fan-titled structural items and each item whose title matches the cyclic pattern is a member or is listed with its class and reason | 0 unplaced | acceptance of F-875 ("are placed") | 0 of 5 and 0 of 24 | n/a | code verification |
| C4 | the sample check as registered: 40 items by seed 875779, second reading, disagreements listed and decided | made and reported; no limit on the number | acceptance of F-875 ("a sample drawn by seed is read a second time") | under 8 of 40 | n/a | code verification |
| C5 | the command on a scratch copy of `docs/` and `scripts/` of main d10ea0bd: the nine existing definitions equal in bytes and the file outside the new object equal; for every item its list of the nine existing ids equal; every item with no new membership equal in bytes; every mapped item differs by the key alone; the table rows and headline lines of the nine in the generated block equal; a second run writes nothing; `roadmap_milestones.py --check` and `render_roadmap.py --check` (after the plain commands) exit 0 | 0 bytes differ | acceptance of F-875 (last clause); rule 3 | 0 | n/a | unchanged default output |
| C6 | the script's validation is clean on the copy: every headline item is an item and a member, the milestone has a member | no validation line | the brief, task 3; ADR 0756 V2 to V5 | clean | n/a | code verification |
| C7 | figures for the new milestone (members, done, partly done, not started, per cent, the state of each headline item) and for the whole roadmap | stated | the brief, task 2 | about 80 per cent, "not met" | n/a | code verification |
| P | `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check`; `cargo test -p rcfd-cli --release --test docs_site` | the proposal passes; no failure but a generated file not current; passes | rules 7 and 9, version 5 | as stated | n/a | code verification |

No row is of kind validation: the acceptance claims no agreement with a measured case.

## Negative controls and mutations

This work adds no code and no test, so no mutation patch is committed (rule 16 has no test file to sit beside; the
precedent of ADR 0757 and ADR 0775). Negative controls of C5, on scratch copies: (K1) with one character of the title of
`packed-bed` changed in the definitions file, the comparison of C5 reports a difference; (K2) with the mapping extended
by every present member of `packed-bed` and one more line `F-001`, `packed-bed`, the comparison reports exactly F-001 as
an item whose list of existing ids changed. Negative control of C2: with one membership's reason not starting M1 to M4,
the build script refuses. Negative control of C3: with one listed cyclic item taken from the list of non-members, the
placement script names it.

## Decision rule

Done only if the milestone is defined as registered (C1) and mapped by the registered rule with reasons (C2), the two
lists are placed (C3), the sample check is made and reported (C4), and the command is shown on a copy leaving the nine
existing milestones unchanged with validation clean (C5, C6). Partial if the milestone cannot be defined as registered
(a headline item that is not an item of the roadmap, a case file that does not exist, or no page of the repository
that records the reference as the definition words it) or the sample check was not made. Stopped and reported if a
class of items cannot be placed by the rule or the command needs a change.

## What is not covered

No edit of `docs/roadmap.json`, `docs/milestones.json`, `scripts/roadmap_milestones.py`, `docs/ROADMAP.md`,
`docs/roadmap-overview.md`, the SVGs or `docs/adr/index.md` (rule 9): the lead runs the command. The definitions file is
built from `docs/milestones.json` of main d10ea0bd; if main's file changes before the command is run, it is rebuilt (the
addendum gives the way). The notes of the items are read only around keyword matches. The acceptance of the structural
items outside the closures and the patterns is not read. The second reading is of 40 items. No paper was read for this
work and no new reference value is entered; the references are named from the pages of the repository. The topic page
`docs/structural-solver.md` is not edited. No cargo command but `docs_site`; no solve.

## Addendum 1: results (2026-11-23)

Commits: e3caa284 (this registration, alone, on main d10ea0bd), bff7716a (the definition and the mapping, first
reading), and the commit of this addendum with the proposal file. No amendment was needed and no correction of the
mapping was taken.

| Id | Measured | Met |
| --- | --- | --- |
| C1 | defined as registered: the two case files and the page exist (asserted by `build.py`), the reference names three pages with their sections and ADR 0500, the three headline items are items of the roadmap; nothing left out | yes |
| C2 | 792 of 792 items read as the rule says (by title; by acceptance where in a closure, in a pattern or a candidate of M2 (a)); 39 memberships, each with a reason that starts M1 or M2 (the build script asserts M1 to M4); no M3 and no M4 membership | yes |
| C3 | fan-titled: 5 of 5 placed (F-387, F-406, F-436, F-593 members; F-604 listed). Cyclic pattern: 24 titles, 24 placed (F-394 and F-406 members; 22 listed below). Control: with F-500 taken from the list `build.py` names F-500 and exits 1 | yes |
| C4 | 40 items by seed 875779 (from 39 members, 753 non-members, 196 doubtful ones not yet drawn), second reading by a fresh agent: 0 disagreements of 20 members, 0 of 10 uniform, 0 of 10 doubtful | yes |
| C5 | on a copy of `docs/` and `scripts/` of main d10ea0bd made by `git archive` (`proof.py`, scratch): `--apply` exit 0, 39 memberships written; the nine existing definitions equal and the file outside the new object equal in bytes; 0 of 792 items with a changed list of the nine; 0 items outside the mapping changed; the 39 mapped items differ by the key alone; no line of the old block missing, five lines added (one table row, the milestone's line and its three headline lines), the old lines in their order and the page outside the block equal; a second `--apply` writes nothing; `roadmap_milestones.py --check` 0; `render_roadmap.py --check` 0 after a render and `roadmap_milestones.py --check` 0 after it. The same on the lead's main at 19debd2d: 0 differences. K1 (one character of the title of `packed-bed`): `packed-bed` reported as differing and the file outside the new object as not equal. K2 (the present members of `packed-bed` and F-001 added to the mapping): exactly F-001 reported. C2 control: `build.py` refuses a reason that does not start M1 to M4 | yes |
| C6 | validation clean on the copy (no line printed; every headline item an item and a member) | yes |
| C7 | below | yes |
| P | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0779-F-875.json` passes (F-875 to done, two new items); `cargo test -p rcfd-cli --release --test docs_site` 2 passed; `python scripts/process_report.py --check`: in the report to the lead (run after this commit) | yes |

**Figures** on a copy of main d10ea0bd after the command (the whole roadmap: 792 items, 398 done, 299 partly done, 95
not started, 69 %; the rows of the nine existing milestones unchanged).

| Milestone | Headline | Members | Done | Partly done | Not started | Per cent | Headline result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `fan-structure` | F-387 (done), F-406 and F-436 (not started) | 39 | 21 | 14 | 4 | 72 % | not met |

By class: M1 3; M2 (a) 16 (F-025, F-132, F-133, F-232, F-370 to F-376, F-382, F-383, F-455, F-457, F-593); M2 (b1) 11
(F-394, F-396, F-399, F-403 for F-406; F-409, F-411, F-412, F-413, F-415, F-418, F-435 for F-436); M2 (b2) 9 (F-377,
F-380, F-386, F-388, F-393, F-395, F-397, F-408, F-414). The four members not started are F-403, F-406, F-435 and F-436:
the two demonstrations and the two case-file items they are written from. By line of work: 35 structural, three of
the meshing line (F-132, F-133, F-232) and F-025 of numerics. The estimate of the registration was about 80 per cent.

**The two lists.** Fan-titled: F-387, F-406, F-436 M1; F-593 M2 (a); F-604 not a member, N4: a press fit of a hub on a
shaft with a bore-cell rule for the fan hub, which no run of record and no demonstration's acceptance or dependency
list uses (the hub of the demonstrations is a rigid clamp). It is the nearest of the five to a member and is named as
the borderline. The cyclic rotor: F-394 (M2 (b1)) and F-406 (M1) are members; the other 22 are N4, each for one of
four reasons:

| Reason (N4) | Items |
| --- | --- |
| the rotating, whirl or free-sector path of a sector: the sector clause of F-406 names no speed and no dependency list names the item | F-500, F-507, F-511, F-680, F-683, F-686 |
| the Coriolis split of a sector and its follow-ups: the Coriolis clause of F-436 is on the single blade | F-555, F-569, F-575, F-584, F-612 |
| shells or beams in the cyclic constraint: the demonstrations are solid Tet10 | F-628, F-647, F-648, F-662, F-672, F-674, F-675 |
| another load, boundary or material on a sector that no demonstration names | F-426, F-489, F-544, F-547 |

So the cyclic rotor is in this milestone through the at-rest clause of F-406 and through F-394 alone; 11 of the 22
listed items are about a sector at speed or with the Coriolis term (open question 2).

**Other non-members that were candidates.** In a closure and left out, as registered: F-381 (the iterative solver),
F-398 (transient), F-410 (viscoelastic), F-417 (random vibration), F-136 (the far-field integral, a dependency of
F-418 that the equivalent radiated power does not use), F-013, F-080, F-005, F-015, F-070 (the solid case file has a
schema of its own), F-010, F-012 (the runs use the builders of `rcfd-fem`), F-026, and the flow solver's items; F-001
and F-003 are N3. Structure and vibration beyond the demonstrations, N4: the rotor on bearings (F-416, F-562 to
F-564), the load export and the coupling (F-237, F-384, F-385, F-400, F-401, F-421, F-434), the unbalance term of the
fan objective (F-697, a member of `fan`). Borderline and left out: F-306 and F-324, repairs inside `tetrahedralize`,
which route D of the run calls: read by the registered M4 sentence (their text does not name this blade) and by the
practice of ADR 0756, which did not take a repair of a used module as a member; the letter of M2 (a) ("the item's own
text names the function the run calls") could be read to take F-324. F-131 (no run names its module), F-462 and F-498
(a default and a restated check of members, used by no run).

**The sample check.** The reader was given `rule.md` (the sections "The definition" and "The membership rule" of this
ADR) and the sheet in a folder of its own; it reports that it opened none of the files it was told not to open, and
consulted the two test files, `gyro.rs` and the stubs ADR 0339 and ADR 0310 for the dependencies of F-436, which the
sheet did not list for it. Its classes agree with the first reading on all 40, the subclasses of M2 included. Hard
items it named: F-455 (a member by one option call, partly done with a known miss), F-132 (the M4 sentence against the
`use` line), F-386 (a dependency of F-387 that its run does not use, a member only through F-406), F-414 ("the
eigensolver" of (b2) does not say which), F-534 and F-498. It found the rule unclear in these places: M2 (a) has no
closure condition; it does not say whether use through a member counts (F-025, F-373); a direct dependency of the run
headline that the run does not use has no class; a defect of a package under a demonstration that is not run cannot be
M4 and falls to N4, whose words do not describe a repair; N1 against N5 for flow-side items. What the check shows: 0
of 40 says the rule is applied as written; it does not test the judgements named above (F-604, and F-306 with F-324),
none of which was drawn, and half the non-member draw is far from the milestone.

**What the figure will and will not say** (for the maintainer). The 72 per cent counts what the three demonstrations
are built from: 21 of 39 done. The word is "not met" and stays so until F-406 and F-436 are done. When it reads "met"
it will mean that the demonstrations pass closed forms, identities and mesh change; nothing in this milestone compares
with a measured or published blade (open question 1).

**The command the lead runs on main**, then the usual regeneration (`python scripts/render_roadmap.py`; the block of the
overview is written by the command itself):

`python scripts/roadmap_milestones.py --apply docs/adr/0779-f875-mapping.tsv --definitions docs/adr/0779-f875-milestones.json`

The definitions file is `docs/milestones.json` of main d10ea0bd with the object `fan-structure` appended and every
other byte kept. If main's file changes before the command is run, the file is rebuilt the same way: the last object
of `docs/adr/0779-f875-milestones.json` appended to main's list and written with `json.dumps(..., indent=2,
ensure_ascii=False)` and one newline. No `--install` is needed.

**Deviations.** (1) The registration writes the cyclic pattern in the section of the two lists as three words; the
scripts use them as the alternatives of one regular expression, as meant. (2) The doubtful draw was from 196 items,
the 235 of the registration less the members and the uniform draw. (3) The second reader looked up the dependencies of
F-436 in ADR 0339 and ADR 0310, because the sheet gave the deps of each drawn item but not those of the headline
items. (4) The command was also shown on the lead's main at 19debd2d, which the registration did not ask. (5) The per
cent is 72 against an estimate of about 80 (an estimate, not a limit). Not run: any cargo command but `docs_site`; any
solve. No mutation patch (registered).

Actual size: done; 0; 0; 2; about 420000.

Decision by the registered rule: **done**. The milestone is defined as registered and mapped with reasons; the two
lists are placed; the sample check was made (0 disagreements of 40); the command leaves the nine existing milestones
byte for byte on a copy of main, with validation and both checks clean.

## Open questions for the maintainer

1. **A headline item against a published reference.** The three headline items compare with closed forms and
   identities; the repository records no measured or published blade, rotor or frame, so this milestone can read "met"
   without a validation. Options: (a) leave it: the `result` and the `reference` say in words that the demonstrations
   are not validated; (b) enter the first follow-up below as a fourth headline item: a published rotating blade or
   bladed disc is read and recorded and the solver is compared with it; the milestone then cannot read "met" before
   that comparison passes. Recommendation: (b), as the fan milestone was given F-870 and F-873.
2. **The cyclic rotor at speed.** The sector clause of F-406 is at rest, so of the 24 cyclic titles only F-394 and
   F-406 are members, and the rotating and Coriolis sector work (F-500, F-507, F-511, F-555 and their follow-ups, 11
   items) is under no headline item. Options: (a) leave it; (b) enter the second follow-up below as a headline item,
   which would bring F-500 and F-499 in as direct dependencies. Recommendation: (b): the maintainer's answer names the
   cyclic rotor as part of this milestone, and at speed it has no item that says so.

## For the lead: roadmap entries

> Status proposed for F-875: done.

> Note entry: ADR 0779, 2026-11-23, done: the milestone fan-structure ("Structure and vibration of a fan rotor") defined in docs/adr/0779-f875-milestones.json with the headline F-387, F-406 and F-436 and 39 memberships with reasons in docs/adr/0779-f875-mapping.tsv (21 done, 14 partly done, 4 not started, 72 per cent, not met); its reference is closed forms and identities, no measured or published blade being recorded; the five fan-titled structural items and the 24 cyclic titles placed (F-394 and F-406 members, 22 listed, F-604 listed); sample of 40 items read a second time with 0 disagreements; the command of addendum 1 leaves the nine existing milestones byte for byte, shown on a copy of main d10ea0bd.

> Follow-up (proposed, open question 1): "Structural fan rotor: a published rotating blade or bladed disc read, recorded and compared", deps F-387, F-394, F-396; acceptance: Every limit not stated here is registered in the ADR before the run. A published case of a rotating blade or a bladed disc with its natural frequencies (measured, or a benchmark solution) is read, and its geometry, material and values are recorded with their place in the source as a fragment under docs/reference-data.d; the solid solver reproduces those frequencies at rest and at speed within a band registered before the run from the stated uncertainty of the source; a source that cannot be obtained is recorded as not obtained and the item stays partial. Milestone fan-structure, headline.

> Follow-up (proposed, open question 2): "Fan rotor at speed by a cyclic sector: the Campbell table of the bladed hub against the full rotor", deps F-406, F-500, F-499; acceptance: Every limit not stated here is registered in the ADR before the run. One sector of the bladed hub of F-406 at registered speeds from rest to the registered maximum, with stress stiffening and spin softening and, as a second table, with the Coriolis term: for every harmonic index 0 to Z/2 the sector frequencies equal those of the full rotor on the coarse mesh within 1e-6 (heavy record), at rest they have the bits of the sector of F-406, and every crossing of the engine orders is listed with its nodal diameter. Milestone fan-structure, headline.

The same entries are in `docs/roadmap/proposals/0779-F-875.json` (form 1 of ADR 0755, base d10ea0bd).
