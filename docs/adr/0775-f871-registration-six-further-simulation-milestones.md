# ADR 0775: F-871 registration: six further simulation milestones

Status: **REGISTERED 2026-11-23, written before any code of F-871 and before any run of its checks.** Before this text no
definition, mapping or sample of F-871 was written. What was done: reading (below) and reading scripts over
`docs/roadmap.json` kept in the scratch folder (`items.py`: id, status, line of work, title, deps and acceptance of every
item, the dependency closures of the headline items and keyword counts; `show.py`; `grepn.py`: the text around the first
match of a pattern in title, acceptance, gap and note). Results go in addenda at the end; nothing above them is changed
after a run.

It answers the brief `docs/adr/briefs/f871-six-further-milestones.md` and follows `docs/process/agent-rules.md`,
version 5. Branch `f871-six-further-milestones` from main ece2e34d, scratch
`w49i-work/f871-six-further-milestones-scratch`. This work writes no library, script or test code and runs no solve.

Size (agent): done; 0; 0; 2; 300000

## What is asked

Roadmap item F-871, entered on the maintainer's answer of 2026-11-21 to ADR 0756 question 4. Six milestones (the
lid-driven cavity, the backward-facing step, the flat plate, the heated cavity, the shock tube and the wedge, the packed
bed) are defined in `docs/milestones.json` with their cases, their published reference as a page of the repository
records it, and their headline items; every item of the roadmap is read against the membership rule of ADR 0756 for
each, every membership has a reason, a sample drawn by seed is read a second time and its disagreements are reported,
and one command adds the six on main and leaves the existing milestones byte for byte as they are. A milestone for which
the roadmap holds no headline item is reported with the item that would have to be entered, and is not added.

## What was read

Whole: the brief; `docs/process/agent-rules.md` (version 5); ADR 0756 and ADR 0757; `scripts/roadmap_milestones.py`;
`docs/milestones.json`; the head (comment lines) of `docs/adr/0756-f865-mapping.tsv` and `docs/adr/0757-f867-mapping.tsv`
and the 61 rows of the first; `docs/roadmap/proposals/0757-F-867.json`. Of `docs/reference-data.md`: lines 1 to 262
(from the head to "Not verifiable online", which hold the sections "Lid-driven cavity", "Differentially heated cavity",
"Exact Riemann solver, Sod problem and oblique shock", "Backward-facing step", "Laminar backward-facing step", "Flat
plate"), the section "Spalart-Allmaras (F-041, ADR 0014)" and the table of "Wave 30B: incompressible validation data
actually read". Of `docs/validation-data.md`: the two headings of `flat-plate-tmr` and `backward-step-tmr`. Of
`docs/audit/partials-group-6.md`: the section of F-031. Of `docs/case-schema.md`: the row `porous`. Of ADR 0008: the
section "Test files". Of ADR 0060: the head to the rules. Of `scripts/roadmap_progress.py`: the lines of work. Of
`scripts/process_report.py`: `sec_c`, `sec_e`, `checks_table`, `sec_r`. The dataset ids and file names of
`validation-data/manifest.json`. The leading comments and the `use` lines of `crates/rcfd-fv/tests/cavity.rs`,
`crates/rcfd-api/tests/validation_cavity_ghia.rs`, `crates/rcfd-fv/tests/natural_convection.rs`,
`crates/rcfd-fv/tests/validation_natural_convection.rs`, `crates/rcfd-fv/tests/diag_cavity_ra1e6.rs`,
`crates/rcfd-fv/tests/validation_sod.rs`, `crates/rcfd-fv/tests/compressible_bc.rs`, `crates/rcfd-fv/tests/step_laminar.rs`,
`crates/rcfd-api/tests/validation_bfs_step.rs`, `crates/rcfd-api/tests/validation_blasius_plate.rs`,
`crates/rcfd-api/tests/w39c_coarse_step.rs`, `crates/rcfd-api/tests/compressible_bc_case.rs`,
`crates/rcfd-turb/tests/step_kepsilon.rs`, `step_kepsilon_corrected.rs`, `step_realizable_a0.rs`, `step_sst.rs`,
`flat_plate_sa.rs`, `flat_plate_sst.rs`, `flat_plate_bsl_sst.rs`, `near_wall_catalogue.rs`,
`validation_flat_plate_wall_law.rs`, `validation_w33c_sst_plate.rs`, `w34c_sst_plate_hypotheses.rs`, and
`crates/rcfd-turb/tests/common/step.rs` and `common/plate.rs`; of `crates/rcfd-api/tests/sources.rs` lines 1 to 75 and
120 to 150; of `crates/rcfd-api/tests/component_diagonal.rs` lines 1 to 40; the `use` lines and the wedge and Sod tests
of the test module of `crates/rcfd-fv/src/compressible.rs` (lines 1490 to 1700, in part) and its search for gradients;
the head of `crates/rcfd-fv/src/sources.rs`; the case files `cases/sod/sod.toml`, `cases/wedge/wedge.toml`,
`cases/packed_bed/packed_bed.toml`, `cases/backward_step/case.json`, the head of `cases/flat_plate/case.json`, the list
of `cases/`; a search of the tests for the paths of reference files they read, and for `amg` and `pseudo`; the Sod and
wedge test names of `crates/rcfd-api/tests/physics_schema.rs`. Of `docs/roadmap.json`, by script: for all 792 items the
id, status, line of work and title; for the 484 items outside the line "Structural solver" the acceptance cut at 110
characters for the eight lines Numerics and solvers, Turbulence, Heat, Compressible, Meshing, Post-processing, Command
line, Parallel, and the titles of the line "Quality, docs and validation"; the acceptance and gap, cut at 900 or 1200
characters, of F-871, F-028, F-042, F-043, F-041, F-203, F-050, F-060, F-031, F-738, F-712, F-096; the note of F-006 from
"WAVE 30B" (2500 characters), of F-050 from "WAVE 30C", of F-060 (3000 characters), of F-028 and F-030 (700); and for
every item 60 to 100 characters around the first match of the keywords of each milestone (below) and of "ADR 0060",
"ADR 0061", "ADR 0062", "ADR 0063", "ADR 0070", "ADR 0074", "ADR 0014", "ADR 0015", "ADR 0017", "Van Albada",
"Ra 1e6", "packed", "Ergun", "component diagonal".

## Readings (before any mapping)

* `docs/roadmap.json` at ece2e34d holds 792 items: 393 done, 298 partly done, 101 not started. The keys name three
  milestones: `vortex-shedding` (20 members), `fan` and `aerofoil-lift` (24 members).
* `--apply MAPPING --definitions FILE` replaces the members of every milestone the mapping names and of no other, and
  copies FILE over `docs/milestones.json` whole. A mapping that names only the six new ids therefore leaves the keys of
  the three, and the new lists are written in the order of the definitions, so an item that is a member of an old and a
  new milestone keeps the old ids first and in their order. The definitions file must carry the three objects unchanged
  in bytes. No defect of the script stops this work; the script is not changed.
* Headline items the roadmap holds, by their acceptance: the cavity F-028 (done; "Lid-driven cavity Re=100/400/1000
  matches Ghia et al. centerline profiles within 2 percent"); the turbulent step F-042 (partial; its second clause, the
  step reattachment) and F-043 (partial; its clause "BFS"); the turbulent plate F-041 (partial; "Flat plate Cf within 5
  percent of correlation"), F-043 (its clause "Flat plate") and F-203 (done; Prandtl-Schlichting within 5 percent, rough
  wall within 10 percent); the heated cavity F-050 (done; de Vahl Davis Ra 1e4 to 1e6 within 2 percent); the shock tube
  and wedge F-060 (done; "Sod shock tube L1 error vs exact; 2D oblique shock angle within 1 percent"); the packed bed F-031
  (done; its first clause, "Packed-bed Ergun pressure drop"). `cases/backward_step/case.json` names F-042 and F-043 as
  its roadmap items.
* Compared in the repository but held by no item's acceptance: the laminar step of expansion ratio 2 at Re_D 800
  (ADR 0060, checks S1 to S3, `validation_bfs_step.rs`, passing; `step_laminar.rs`) and the laminar Blasius plate
  (ADR 0060, checks B1 to B4, `validation_blasius_plate.rs`). Their results are in the note of F-006, whose acceptance
  is the case registry. The keyword search found no other item for them.
* Dependency closures: F-028 10 items; F-042 with F-043 19; F-041, F-043 and F-203 21; F-050 11; F-060 8; F-031 11. The
  closures of the turbulent headline items hold F-106 and F-107, which no turbulent run of record below uses (the search
  for `pseudo` in those files found nothing; no periodic pair is set).
* What the runs use, from their text: every incompressible run of record but the packed bed builds `SimpleSolver` from
  `rcfd_fv` directly, with a generator of `rcfd_mesh::gen` (`cavity_2d`, `box_2d_graded`, `backward_step_graded`), and
  does not go through `Case`; the packed bed (`sources.rs`) and the settings-tree Sod and wedge runs
  (`physics_schema.rs`) go through `Case::parse_toml` and `Simulation`. No run of record uses AMG or the
  pseudo-transient option. The compressible solver takes its own least-squares gradients and its own boundary states
  (`compressible.rs`, line 601) and `TvdLimiter` from `rcfd_fv::convection`; it assembles no matrix. The turbulent step
  and plate meshes use `Grading::WallLayers` and `rcfd_mesh::layers`. The step reads the measured inlet profile
  `validation-data/data/backward-step-tmr/profiles.exp.dat`; `validation_cavity_ghia.rs` reads
  `validation-data/reference/cavity_re1000_ghia_botella_u.csv`; `validation_natural_convection.rs` reads
  `cases/heated_cavity/reference/devahldavis1983_tableV.csv`; `validation_sod.rs` reads
  `validation-data/reference/w30c_sod_exact_marzouk2020.csv`; the plate runs read the w30c and w33c TMR files of
  `validation-data/reference/`; `cavity.rs` reads `cases/cavity/reference/`.
* The packed bed's reference is the Ergun equation with the constants 150 and 1.75. `docs/reference-data.md` has no row
  for it. `docs/case-schema.md` (row `porous`) states the map of the model `ergun` onto the Darcy-Forchheimer
  coefficients, and `docs/audit/partials-group-6.md` (section F-031) records that the constants were checked only
  against a secondary web source and that Ergun (1952) was not read; the citation with its journal is in the head of
  `crates/rcfd-fv/src/sources.rs`. The case file `cases/packed_bed/packed_bed.toml` stops at its iteration cap
  (symmetry side walls, ADR 0010, `component_diagonal.rs`); the run of record `sources.rs` compares at the same set-up
  with its own iteration settings.

## The definitions (fixed now)

Six objects, appended in this order after `aerofoil-lift`; no `page` key. The ids have the form the script requires.

| id | title | headline |
| --- | --- | --- |
| `lid-driven-cavity` | Lid-driven cavity | F-028 |
| `backward-facing-step` | Turbulent backward-facing step | F-042, F-043 |
| `flat-plate` | Turbulent flat plate skin friction | F-041, F-043, F-203 |
| `heated-cavity` | Differentially heated cavity | F-050 |
| `shock-tube-and-wedge` | Shock tube and wedge | F-060 |
| `packed-bed` | Packed bed pressure drop | F-031 |

The `result`, `cases` and `reference` of each, as they will stand in the definitions file:

* **lid-driven-cavity.** Result: steady laminar flow in a square cavity driven by its lid at Re 100, 400 and 1000,
  SIMPLE or SIMPLEC on uniform meshes: the centreline velocity profiles against the published tables. Cases:
  `crates/rcfd-fv/tests/cavity.rs`, `crates/rcfd-api/tests/validation_cavity_ghia.rs`, `cases/cavity/case.json`.
  Reference: Ghia, Ghia and Shin, J. Comput. Phys. 48 (1982) 387-411, the u and v centreline tables, as recorded in
  `docs/reference-data.md`, section "Lid-driven cavity" (verified against secondary copies; the printed original is
  recorded as not read), and for Re 1000 Botella and Peyret (1998) next to Ghia in arXiv:1807.04896 Table A-1, as
  recorded in the section "Wave 30B: incompressible validation data actually read".
* **backward-facing-step.** Result: turbulent flow over a backward-facing step of expansion ratio 1.125 (the Driver and
  Seegmiller geometry, Re_h about 36000) in steady RANS with k-epsilon and k-omega SST: the reattachment length against
  the measured value. Cases: `crates/rcfd-turb/tests/step_kepsilon.rs`, `crates/rcfd-turb/tests/step_kepsilon_corrected.rs`,
  `crates/rcfd-turb/tests/step_realizable_a0.rs`, `crates/rcfd-turb/tests/step_sst.rs`, `cases/backward_step/case.json`.
  Reference: x_r/h = 6.26 +/- 0.10, Driver and Seegmiller, AIAA J. 23 (1985) 163, as recorded in
  `docs/reference-data.md`, section "Backward-facing step" (from the NASA TMR backstep validation page, verified), and
  in `cases/backward_step/case.json`; the measured inlet profile is the TMR file `profiles.exp.dat` of the dataset
  `backward-step-tmr` of `docs/validation-data.md`.
* **flat-plate.** Result: the zero-pressure-gradient turbulent flat plate of the NASA TMR (Re 5e6 per unit length) in
  steady RANS with Spalart-Allmaras, k-omega SST and k-epsilon: the skin friction against the correlations and the TMR
  data. Cases: `crates/rcfd-turb/tests/flat_plate_sa.rs`, `flat_plate_sst.rs`, `flat_plate_bsl_sst.rs`,
  `near_wall_catalogue.rs`, `validation_flat_plate_wall_law.rs`, `validation_w33c_sst_plate.rs` (all under
  `crates/rcfd-turb/tests/`, written there in full), `cases/flat_plate/case.json`. Reference: the correlations of
  `docs/reference-data.md`, section "Flat plate" (power law 0.0576 Re_x^-0.2, White 0.455/ln^2(0.06 Re_x)); the TMR
  CFL3D values Cf(0.97) = 0.00268299 (SST, section "k-omega SST and wall treatment") and 2.70565e-3 (SA, section
  "Spalart-Allmaras (F-041, ADR 0014)"); Karman-Schoenherr and the Coles law from the TMR flat-plate validation page
  (the wave 30C table of section "Differentially heated cavity"); the TMR files of the dataset `flat-plate-tmr` of
  `docs/validation-data.md`.
* **heated-cavity.** Result: steady natural convection in a square cavity with a hot and a cold side wall (Boussinesq,
  Pr 0.71) at Ra 1e3 to 1e6: the average Nusselt number and the velocity maxima against the benchmark solution. Cases:
  `crates/rcfd-fv/tests/natural_convection.rs`, `crates/rcfd-fv/tests/validation_natural_convection.rs`,
  `cases/heated_cavity/case.json`. Reference: de Vahl Davis, Int. J. Numer. Meth. Fluids 3 (1983) 249-264, Table V,
  read as a page scan (Nu_avg 1.118, 2.243, 4.519, 8.800), as recorded in `docs/reference-data.md`, section
  "Differentially heated cavity" (wave 30C: verified, primary).
* **shock-tube-and-wedge.** Result: inviscid compressible flow with the density-based solver: the Sod shock tube against
  the exact Riemann solution (L1 error) and the Mach 2 flow over a 10 degree wedge against the oblique-shock angle.
  Cases: `crates/rcfd-fv/tests/validation_sod.rs`, `crates/rcfd-fv/src/compressible.rs` (its Sod and oblique-shock
  tests), `crates/rcfd-fv/src/riemann.rs` (the exact solver and the theta-beta-M relation), `crates/rcfd-api/tests/physics_schema.rs`
  (the Sod and wedge runs through the settings tree), `cases/sod/sod.toml`, `cases/wedge/wedge.toml`. Reference: the
  exact solution of the Riemann problem (p* 0.30313, u* 0.92745; arXiv:2108.01709, and Marzouk, Sci. Afr. 10 (2020)
  e00573, Table 2, read) and the theta-beta-M relation (weak beta 39.31 degrees at M 2 and 10 degrees, solved
  independently), as recorded in `docs/reference-data.md`, section "Exact Riemann solver, Sod problem and oblique shock"
  and the wave 30C table of section "Differentially heated cavity"; Sod (1978) is recorded there as not obtained.
* **packed-bed.** Result: steady laminar flow of air through a packed bed of 5 mm spheres (porosity 0.4) as a porous
  zone in the superficial-velocity form: the pressure gradient against the Ergun equation. Cases:
  `crates/rcfd-api/tests/sources.rs` (the packed-bed tests), `cases/packed_bed/packed_bed.toml`. Reference: the Ergun
  equation with the constants 150 and 1.75, as recorded in `docs/case-schema.md` (source type `porous`, model `ergun`)
  and `docs/audit/partials-group-6.md` (section F-031: the constants checked only against a secondary source; Ergun
  (1952) recorded as not read).

**Two scopes are fixed here and named.** The step and the plate are the turbulent cases, because the roadmap holds
headline items only for them. The laminar step (Re_D 800) and the Blasius plate are not in the definitions and their
files are not runs of record; each is reported under the open questions with the item that would have to be entered.

**Headline items with several clauses.** F-031 (packed bed and rotating channel), F-042 (channel and step) and F-043
(plate, step and NACA 0012; already a headline of `aerofoil-lift`) carry the milestone's comparison as one clause of
several. The script reads an item's status, not a clause, so the word of the step and of the plate stays "not met"
while any clause of F-042 or F-043 is open, and the gap printed with them names the other cases as well. This is
recorded, not changed: it is the reading ADR 0756 gave F-043 for the aerofoil.

## The membership rule (fixed before the mapping)

The classes of ADR 0756 apply to each of the six, with its runs of record being the files of its `cases` that run and
compare (the case files `cases/.../case.json` are case definitions and are read by M3).

* M1, headline: the item is a headline item of the milestone.
* M2, used: a run of record uses what the item delivered: a solver algorithm, a discretisation, a turbulence model, a
  boundary condition, a mesh generator or wall-layer grading, the matrices or linear solvers, the under-relaxation and
  convergence controls, the settings layer it runs through. Evidence: the run's `use` lines or set-up name it, or the
  item is in the dependency closure of a headline item and its subject is in the run (the gradients of the Rhie-Chow
  step and of linear upwind; the mesh geometry; the under-relaxation factors of `SimpleSettings`, which the note of F-030
  names as its delivery). Being in the closure alone is not enough. `F-005` and `F-080` are members only of a milestone
  with a run through `Case`.
* M3, data: the item holds the case definition or the reference data the comparison reads (a case directory of the
  registry, a dataset of the validation pack that a run reads).
* M4, trust: the item's subject is a defect, a refused set-up, a divergence or a missed limit that the milestone's own
  run met, open or closed, or a mesh or time-step study of that run.
* Not a member: N1 an item that uses the milestone's case as a test of something else (a coupled solver, adaptation,
  interpolation, an initialisation, a viewer, a tool or a language binding run on the cavity; a mesher or a time-step
  controller checked on a plate); N2 an item that consumes the milestone's result; N3 workspace, CI, process,
  documentation, Studio and assistant items, F-001 and F-003 among them (the reading of ADR 0756); N4 an alternative
  that the runs of record do not use (another turbulence model or transition model, another pressure algorithm, AMG,
  the pseudo-transient option, the characteristic compressible boundaries, a held default change); N5 no relation.
* The 308 items of the line "Structural solver" are read by title only; an item of that line is a member only if its
  title names the milestone's flow simulation ("flat plate" in a structural title is a plate in bending, N5).
* Diagnosis files that print and assert nothing on the comparison (`diag_cavity_ra1e6.rs`) and hypothesis files
  (`w34c_sst_plate_hypotheses.rs`) are not runs of record; an item whose subject they diagnose is M4 only if its subject
  is a defect of a run of record.

Keywords used to find candidates (a candidate is read; a match decides nothing): cavity `lid.driven`, `ghia`, `cavity`;
step `backward.facing`, `backward.step`, `BFS`, `reattach`; plate `flat.plate`, `skin friction`, `Cf`, `blasius`,
`karman.schoenherr`, `2DZP`; heated cavity `heated cavity`, `de vahl davis`, `natural convection`, `differentially
heated`, `Ra`, `rayleigh`; shock `sod`, `shock tube`, `oblique shock`, `wedge`, `riemann`; packed bed `packed`, `ergun`,
`porous`. Every item is read for every milestone: the members by the classes above, every other item by its line of
work, title and, outside the structural line, its acceptance.

Every membership gets one line `<item>` tab `<milestone>` tab `<reason>` whose reason starts with M1, M2, M3 or M4. If a
class of items cannot be placed by M1 to M4 and N1 to N5, the work stops and reports the class.

## The check of the mapping (fixed before the mapping exists)

The mapping file is committed first. Then `sample.py` (scratch) draws with `random.Random(871775)`, in this order: 20
pairs by `sample` from the member pairs (item, milestone) of the six, sorted by item number and then by the order of
the definitions; 10 pairs by `sample` from all non-member pairs of the six sorted the same way; 10 by `sample` from the
non-member pairs not yet drawn whose item is in one of the lines Numerics and solvers, Turbulence, Heat, materials and
chemistry, Compressible flow and acoustics, Multiphase and particles, Moving meshes, rotors and coupling, Parallel and
GPU, Meshing and mesh formats, Post-processing and output, Command line, Python and API (the doubtful half). The 40
pairs, sorted by item number, are read a second time by a fresh agent that is given this section's rule and the
definitions as a file, and for each pair the item's id, title, status, deps, acceptance and gap and the milestone, in a
folder of its own, with the repository's code to consult; it is told not to open this ADR, the mapping,
`docs/roadmap.json`, `docs/milestones.json`, the mappings of ADR 0756 and 0757 or the scratch folder, and not which half
a pair came from. Reported: the disagreements in each half, every disagreeing pair with both readings and the reading
taken with its reason. A correction is a separate commit of the mapping; the pairs are not redrawn.

## Table of checks

Every clause of the acceptance of F-871 first. No row has a numeric limit of my own; each limit is a clause.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | each of the six is defined with cases, a reference that names the page and section of the repository that records it, and headline items; a milestone with no headline item is reported with the item to enter and left out | 6 of 6 defined, or each missing one reported | acceptance of F-871 (first and last sentence) | 6 of 6 | n/a | code verification |
| C2 | every item is read for every milestone; each membership has a one-line reason that starts M1 to M4 | 792 of 792 items read for 6 milestones; 0 memberships without a reason | acceptance of F-871 ("every item ... is read", "every membership has a reason") | as stated | n/a | code verification |
| C3 | the sample check as registered: 40 pairs by seed 871775, second reading, disagreements listed and decided | made and reported; no limit on the number | acceptance of F-871 ("a sample drawn by seed is read a second time") | under 8 of 40 | n/a | code verification |
| C4 | the command on a scratch copy of the tree of main ece2e34d: the three existing definitions equal in bytes and the file outside the six new objects equal; for every item its list of the three existing ids equal; every item with no new membership equal in bytes; the table rows and headline lines of the three in the generated block equal; a second run writes nothing; `roadmap_milestones.py --check` and `render_roadmap.py --check` (after the plain commands) exit 0 | 0 bytes differ | acceptance of F-871 (last clause but one); rule 3 | 0 | n/a | unchanged default output |
| C5 | the script's validation is clean: every headline item is an item and a member, every milestone has a member | no validation line | the brief, task 3; ADR 0756 V2 to V5 | clean | n/a | code verification |
| C6 | figures for each new milestone (members, done, partly done, not started, per cent, the state of each headline item) and for the whole roadmap | stated | the brief, task 2 | 10 to 25 members each | n/a | code verification |
| P | `python scripts/roadmap_proposals.py check`; `python scripts/process_report.py --check`; `cargo test -p rcfd-cli --release --test docs_site` | the proposal passes; no failure but a generated file not current; passes | rules 7 and 9, version 5 | as stated | n/a | code verification |

No row is of kind validation: the acceptance claims no agreement with a measured case.

## Negative controls and mutations

This work adds no code and no test, so no mutation patch is committed (rule 16 has no test file to sit beside; the
precedent of ADR 0757). Negative controls of C4, on scratch copies: (K1) with one character of the title of
`vortex-shedding` changed in the definitions file, the comparison of C4 reports a difference; (K2) with the mapping
extended by every present member of `vortex-shedding` and one more line `F-001`, `vortex-shedding`, the comparison
reports exactly F-001 as an item whose list of existing ids changed. Negative control of C2: with one membership's
reason not starting M1 to M4, the build script refuses.

## Decision rule

Done only if every milestone with a headline item is defined (C1) and mapped by the registered rule with reasons (C2),
the sample check is made and reported (C3), and the command is shown on a copy leaving the three existing milestones
unchanged with validation clean (C4, C5). Partial if a milestone cannot be defined as registered (no headline item the
roadmap holds, or no page of the repository that records its reference) or the sample check was not made. Stopped and
reported if a class of items cannot be placed by the rule.

## What is not covered

No edit of `docs/roadmap.json`, `docs/milestones.json`, `scripts/roadmap_milestones.py`, `docs/ROADMAP.md`,
`docs/roadmap-overview.md`, the SVGs or `docs/adr/index.md` (rule 9): the lead runs the command. The definitions file is
built from `docs/milestones.json` of main ece2e34d; if main's file changes before the command is run, it is rebuilt
(the addendum gives the way). The notes of the items are read only around keyword matches and for the items named
above. The second reading is of 40 pairs. The published references are named from the pages of the repository; no paper
was read for this work, and no new reference value is entered. No cargo command but `docs_site`; no solve.

## Addendum 1: results (2026-11-23)

Commits: 67e1c319 (this registration, alone, on main ece2e34d), 5392fa4a (the definitions and the mapping, first
reading), and the commit of this addendum with the proposal file. No amendment was needed and no correction of the
mapping was taken.

| Id | Measured | Met |
| --- | --- | --- |
| C1 | 6 of 6 defined as registered, each with a headline item the roadmap holds and a reference that names its page and section; none left out | yes |
| C2 | 792 of 792 items read for the six (308 structural by title); 81 memberships of 24 items, each with a reason that starts M1, M2 or M3 (the build script asserts M1 to M4); no M4 membership: the defects the runs met (the Ra 1e6 stall, the Van Albada overflow, the Re 1000 GCI miss, the plate and step misses) are recorded in the notes and gaps of F-006, F-042, F-043, F-050 and F-060, not as items of their own | yes |
| C3 | 40 pairs by seed 871775 (from 81 member pairs, 4671 non-member pairs, 1687 of them in the doubtful lines), second reading by a fresh agent: 0 disagreements of 20 member pairs, 0 of 10 uniform, 0 of 10 doubtful | yes |
| C4 | on a copy of `docs/` and `scripts/` of main ece2e34d (`proof.py`, scratch): `--apply` exit 0, 81 memberships written; the three existing definitions equal in bytes and the file outside the six new objects equal; 0 of 792 items with a changed list of the three; 0 items outside the mapping changed; the 24 mapped items differ by the key alone; the table rows and headline lines of the three in the block equal; a second `--apply` writes nothing; `roadmap_milestones.py --check` 0; `render_roadmap.py --check` 0 after a render, and `roadmap_milestones.py --check` 0 after it. K1 (one character of the title of `vortex-shedding`): both comparisons of the definitions report a difference. K2 (the 20 present members of `vortex-shedding` and F-001 added to the mapping): exactly F-001 reported as changed in its list of the three. C2 control: the assertion of `build.py` rejects a reason that does not start M1 to M4 | yes |
| C5 | validation clean on the copy (no line printed; every headline item an item and a member) | yes |
| C6 | below | yes |
| P | `python scripts/roadmap_proposals.py check docs/roadmap/proposals/0775-F-871.json` passes; `cargo test -p rcfd-cli --release --test docs_site` 2 passed; `python scripts/process_report.py --check`: in the report to the lead (run after this commit) | yes |

**Figures** on a copy of main ece2e34d after the command (the whole roadmap: 792 items, 393 done, 298 partly done, 101
not started, 68 %; the rows of the three existing milestones unchanged).

| Milestone | Headline | Members | Done | Partly done | Not started | Per cent | Headline result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `lid-driven-cavity` | F-028 (done) | 12 | 11 | 1 | 0 | 96 % | met |
| `backward-facing-step` | F-042, F-043 (partial) | 16 | 12 | 4 | 0 | 88 % | not met |
| `flat-plate` | F-041, F-043 (partial), F-203 (done) | 18 | 13 | 5 | 0 | 86 % | not met |
| `heated-cavity` | F-050 (done) | 13 | 12 | 1 | 0 | 96 % | met |
| `shock-tube-and-wedge` | F-060 (done) | 8 | 7 | 1 | 0 | 94 % | met |
| `packed-bed` | F-031 (done) | 14 | 14 | 0 | 0 | 100 % | met |

By class: M1 9 (six items), M2 63, M3 9 (F-006 for the cavity, the step, the plate and the heated cavity; F-125 for
those four and the shock tube). The open members are F-125 (five milestones), F-108 (step, plate), F-042 and F-043
(step, plate) and F-041 (plate). The per cents are high for the reason ADR 0756 gave: the members are mostly foundation
items done long ago. Four of the six read "met" because their headline items are done; the step and the plate read
"not met" through F-042, F-043 and F-041, whose gaps also name the channel and NACA 0012 clauses (registered, not
changed).

The non-member candidates (a keyword match or the closure of a headline item: 159 pairs, `nonmembers.tsv` in scratch)
by class: N3 F-001, F-003, F-871 and the process, Studio and assistant items; N1 the items that try something else on
the cavity or the plate (F-032, F-102, F-115, F-143, F-205, F-209, F-210, F-234, F-241, F-342, F-343, F-454 and the
like); N4 F-005 for the library-level runs, F-046, F-062, F-106, F-107, F-738; N5 the rest, among them the structural
items that match "Rayleigh" or "flat plate" and, for the shock tube, F-020, F-021 and F-024 (the compressible solver has
its own boundary states, gradients and explicit stepping). Borderline and left out: F-044 (the turbulence-model
interface, left out as ADR 0756 left it out), F-104 (the expression check that `Case` always runs; the second reader
named it as hard and read it as not used), F-240 (the setup rules on the case path). F-082 is a member of the packed bed
and the shock tube: their runs build and step the case with `Case::parse_toml` and `Simulation` from Rust.

**The sample check.** The reader was given `rule.md` (the sections "The definitions" and "The membership rule" of this
ADR), the definitions file and the sheet in a folder of its own; it reports that it opened none of the files it was told
not to open and consulted the `use` lines of the runs of record, the packed-bed case file, `common/step.rs` and
`case.rs`. Hard pairs it named: F-104 and the packed bed, the M2 members by closure that it could not check against
`docs/roadmap.json`, F-006 for the plate, F-754 and F-132 for the cavity (N5 against N4 and N1). It found the rule
unclear on a code path passed without use, on the depth of the closure for foundation items, and on N4 against N5 for
an opt-in on a feature the run lacks. What the check shows: on these milestones the rule is sharp because the runs of
record are library-level tests whose `use` lines say what they use; a uniform draw of non-members is mostly structural
and unrelated items, so the 0 disagreements say little about the borderline items named above.

**The command the lead runs on main**, then the usual regeneration (`python scripts/render_roadmap.py`; the block of the
overview is written by the command itself):

`python scripts/roadmap_milestones.py --apply docs/adr/0775-f871-mapping.tsv --definitions docs/adr/0775-f871-milestones.json`

The definitions file is `docs/milestones.json` of main ece2e34d with the six objects appended and every other byte
kept. If main's file changes before the command is run (F-875 would add a milestone), the file is rebuilt the same way:
the six objects after `aerofoil-lift` in `docs/adr/0775-f871-milestones.json` appended to main's list and written with
`json.dumps(..., indent=2, ensure_ascii=False)` and one newline. No `--install` is needed.

**Deviations.** (1) The registration estimated 10 to 25 members a milestone; the shock tube has 8 (an estimate, not a
limit). (2) F-082 was added to the packed bed and the shock tube before the mapping's commit, when the non-member
candidates were read: their runs use the Rust API by name. (3) The C2 control was shown on the assertion of `build.py`
alone, not by a full build. (4) `validation-data/data/backward-step-tmr/profiles.exp.dat` is fetched by the pack and not
committed; it is named as the file the run reads. Not run: any cargo command but `docs_site`; any solve.

Actual size: done; 0; 0; 2; about 330000.

Decision by the registered rule: **done**. Every milestone has a headline item the roadmap holds and is defined and
mapped with reasons; the sample check was made (0 disagreements of 40); the command leaves the three existing
milestones byte for byte on a copy of main, with validation and both checks clean.

## Open questions for the maintainer

1. **The laminar backward-facing step.** The step of expansion ratio 2 at Re_D 800 is compared and passes (ADR 0060,
   S1 to S3, x_r/h 11.80 against 11.48 to 12.20), but no item's acceptance holds it, so it is not in the milestone.
   Options: (a) leave the milestone turbulent; (b) enter the first follow-up below as a second headline item of
   `backward-facing-step`, with the two laminar files as runs of record. Recommendation: (b): it puts a passing,
   registered comparison under the milestone, as F-869 did for the cylinder's drag.
2. **The Blasius plate.** The laminar plate is compared (ADR 0060, B1 to B4, passing with a corrected set-up) under no
   item. Options: (a) leave the milestone turbulent; (b) enter the second follow-up below as a headline of `flat-plate`.
   Recommendation: (b), with question 1.

## For the lead: roadmap entries

> Status proposed for F-871: done.

> Note entry: ADR 0775, 2026-11-23, done: six milestones defined in docs/adr/0775-f871-milestones.json (lid-driven cavity F-028, turbulent step F-042 and F-043, turbulent flat plate F-041, F-043 and F-203, heated cavity F-050, shock tube and wedge F-060, packed bed F-031) with 81 memberships of 24 items in docs/adr/0775-f871-mapping.tsv (12, 16, 18, 13, 8 and 14 members; 96, 88, 86, 96, 94 and 100 per cent; four met, the step and the plate not met); sample of 40 pairs read a second time with 0 disagreements; the command of addendum 1 leaves the three existing milestones byte for byte, shown on a copy of main ece2e34d.

> Follow-up (proposed, open question 1): "Laminar backward-facing step at Re_D 800: the reattachment length against the published values", deps F-028, F-006; acceptance: Every limit not stated here is registered in the ADR before the run. The reattachment length of the step of expansion ratio 2 at Re_D 800 lies within the published values 11.48 to 12.20 recorded in docs/reference-data.md (section "Wave 30B: incompressible validation data actually read"), by the checks S1 to S3 of ADR 0060 or their registered successors, on current main, in tests the gate runs. Milestone backward-facing-step, headline.

> Follow-up (proposed, open question 2): "Laminar flat plate: the skin friction and the velocity profile against the Blasius solution", deps F-028, F-006; acceptance: Every limit not stated here is registered in the ADR before the run. The laminar plate reproduces the Blasius values of arXiv:2204.02215 recorded in docs/reference-data.md (section "Wave 30B: incompressible validation data actually read"), by the checks B1 to B4 of ADR 0060 or their registered successors, on current main, in tests the gate runs. Milestone flat-plate, headline.

The same entries are in `docs/roadmap/proposals/0775-F-871.json` (form 1 of ADR 0755, base ece2e34d).
