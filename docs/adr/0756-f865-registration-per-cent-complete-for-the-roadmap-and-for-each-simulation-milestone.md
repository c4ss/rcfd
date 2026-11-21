# ADR 0756: F-865 registration: per cent complete for the roadmap and for each simulation milestone

Status: **REGISTERED 2026-11-21, written before any code of F-865 and before any run of its checks.** Before this text no
script, test, definitions file or mapping of F-865 was written. What was done: reading (below), and reading scripts over
`docs/roadmap.json` kept in the scratch folder (`items.tsv`, the id, status, line of work, title, deps and acceptance of
every item; keyword counts; the dependency closures below). Results go in addenda at the end; nothing above them is
changed after a run.

It answers the brief `docs/adr/briefs/f865-simulation-milestones.md` and follows `docs/process/agent-rules.md`,
version 4. Branch `f865-simulation-milestones` from main c60965fc, scratch
`w49i-work/f865-simulation-milestones-scratch`.

Size (agent): done; 0; 0; 4; 150000

## What is asked

Roadmap item F-865. An item of `docs/roadmap.json` may name the simulation milestones it belongs to (key `milestones`);
`docs/milestones.json` defines each milestone with its cases, its published reference and its headline items. A script
validates the two against each other and writes, as a generated block of `docs/roadmap-overview.md` that CI checks, the
figures for the whole roadmap and for each milestone: items done, partly done and not started, the per cent with a
partly done item as one half, and the state of the headline items in words, so that a milestone whose headline
comparison misses never reads as met. The three milestones vortex shedding, the fan case and aerofoil lift are defined,
every item of the roadmap is mapped by a registered membership rule with a reason for each membership, and the mapping
is checked on a sample drawn by seed. Without the key and the file every generated file is unchanged byte for byte.
Further milestones are proposed for the maintainer's approval.

The lead's decisions in the brief (what a milestone is, the membership rule, where the mapping is written, the figures,
the three fixed milestones) are taken as given and are not repeated here.

## What was read

`docs/process/agent-rules.md` (version 4) and the brief, whole. `scripts/render_roadmap.py`,
`scripts/roadmap_progress.py`, `scripts/test_render_roadmap.py` (first 150 lines), `docs/roadmap-overview.md`, whole.
`docs/milestone-fan.md`: the head, "Goal" to "Capability chain" (the table rows cut at 600 characters). Of
`scripts/process_report.py`: `checks_table`, `sec_r`, `overview_block`, `replace_block`, `generated`. Of
`docs/roadmap.json`, by script: every item's id, title, status, deps; the acceptance of the items of ten lines of work
(below) cut at 90 to 230 characters; the acceptance, gap and the first 900 characters of the note of F-006, F-029,
F-041, F-043, F-062, F-116, F-123, F-125, F-126, F-133, F-240; for every item, the 180 characters around the first
match of each milestone's keywords in title, acceptance, gap and note. `docs/reference-data.md`: the sections "Cylinder
wake" and "NACA 0012", the rows "Cylinder Re 100", "Cylinder Re 100, Williamson (1989)", "NACA 0012 force and Cp data",
"DFG 2D-1" and "DFG 2D-2". `docs/validation-data.md` lines 76 to 100 (FAN-01). The leading comments of
`crates/rcfd-fv/tests/cylinder.rs`, `crates/rcfd-api/tests/validation_cylinder_re100.rs` (and `_w31`, `_w33b`),
`w48t_cylinder_spokes.rs`, `fan01_validation.rs`, `fan01_second_order_w49q.rs`; the imports of the first two and the
case text of `fan01_validation.rs` (by a search for its keys); the docstrings of `scripts/w48v_shedding.py`,
`w48y_aoa_sweep.py`, `w49d_aoa_sweep_fixed.py`, `w49h_te_divergence.py` and the case text of the second; the first 12
lines of ADR 0167; `cases/naca0012/case.json`; the list of `cases/`; ADR 0752 (its form). `.github/workflows/ci.yml`
line 113 (`python -m unittest discover -s scripts -p "test_*.py"`).

## Readings (before any code)

* **The file.** `docs/roadmap.json` is a list of 771 items (385 done, 293 partial, 93 todo), 1 930 622 bytes.
  `json.dumps(items, indent=2, ensure_ascii=False) + "\n"` reproduces it byte for byte (checked); with
  `ensure_ascii=True` it does not. A key appended to an item therefore changes only that item's lines.
* **Who reads an item's keys.** `render_roadmap.schema_problems` refuses a key outside `KEYS`; `process_report.py`
  imports `render_roadmap` and compares `docs/ROADMAP.md` with `render_page`. No other script in `scripts/` was found
  to refuse an unknown key (`roadmap_shards.py`, which another agent is changing, was searched for `KEYS` and
  `unknown` only, and is not covered).
* **The overview.** `process_report.py` owns the block between `<!-- process-report:overview begin ... -->` and
  `<!-- process-report:overview end -->` and replaces nothing outside it. Its per cent is
  `round(100 * (done + 0.5 * partial) / n)`, 69 today.
* **Headline items the roadmap holds.** Vortex shedding: F-029 (done), "cylinder Strouhal within 5 percent at Re=100";
  its test `crates/rcfd-fv/tests/cylinder.rs` builds `cylinder_ogrid`, runs `PimpleSolver` with the AMG pressure solve.
  The mean drag is compared by `validation_cylinder_re100.rs` (ADR 0060, checks K1 to K4), which no item's acceptance
  names. The fan case: F-126 (partial), "a public low-speed compressor/rotor case reproduces overall pressure ratio
  and efficiency within published scatter"; `fan01_validation.rs` calls itself "the F-126 acceptance attempt" on
  FAN-01 and `docs/milestone-fan.md` reads "F-126 not met". Aerofoil lift: F-043 (partial), "Flat plate / BFS /
  NACA0012 validation passes"; `cases/naca0012/case.json` names F-043 and F-062 as its roadmap items. The lift curves
  that were run (ADR 0159, 0164, 0167, 0169, 0171; jobs 48Y to 49H) are named by no item's title, acceptance, gap or
  note: the keyword search found none.
* **Dependency closures** (every item reached through `deps`): F-029: 12 items; F-126: 24; F-043: 19.
* **References, each read in the repository today with its place.** Cylinder: `docs/reference-data.md`, section
  "Cylinder wake" and the rows "Cylinder Re 100" (St 0.164 to 0.167, C_D 1.314 to 1.370, experiment St 0.165, from
  arXiv:2303.09262 Table 1 and arXiv:1401.6513 Table 3, "verified as quotations; the original papers were NOT read")
  and "Williamson (1989)" (J. Fluid Mech. 206, eq. (6): St(100) = 0.164335, primary). Fan: `docs/validation-data.md`,
  "1. FAN-01": Zenodo doi:10.5281/zenodo.20037409 (v5), `characteristic_n1ug.mat`, design point 1.4032 m^3/s,
  126.46 Pa, 333.30 W; the primary rig paper (SAE 2016-01-1805) is recorded there as NOT read. Aerofoil:
  `docs/reference-data.md`, row "NACA 0012 force and Cp data": Ladson, NASA TM 4074 (1988), tripped, M 0.15, Re 6e6,
  as the file `CLCD_Ladson_expdata.dat` of the NASA TMR page, read there; the report itself is not recorded as read. I
  read these rows, not the papers; the definitions cite the rows.

## Design (fixed now)

All from first principles; no published method is used.

**The key.** `render_roadmap.py`: `milestones` joins `KEYS`. Where an item has it, it must be a list of one or more
distinct strings of the form `[a-z][a-z0-9-]*`; anything else is a schema problem "<id>: milestones must be a list of
one or more distinct milestone ids". `item_block` writes one line `  - milestones: a, b` after "depends on" and only
when the key is present; `FIELD` learns the word. Nothing else of the script changes, so a roadmap without the key
renders the bytes it renders today.

**The definitions.** `docs/milestones.json`: `{"milestones": [{...}, ...]}`, each with exactly `id` (the form above,
unique), `title`, `result` (the stated result, one sentence), `cases` (a list of strings: paths or test files),
`reference` (a non-empty string), `headline` (a list of item ids) and optionally `page` (a path under `docs/`).

**The script, `scripts/roadmap_milestones.py`** (Python 3.9 standard library; it imports `render_roadmap` for `load`
and the schema).

* Validation, each a line "milestones: ...": V1 an item names a milestone id that is not defined; V2 a headline item
  is not a member of its milestone; V3 a milestone has no headline item; V4 a milestone has no member; V5 a headline
  id is not an item; V6 the definitions file is malformed (a missing or unknown key, a duplicate id, a bad id, an
  empty reference); V7 an item has the key and there is no definitions file.
* Figures. For a set of items: done, partly done, not started, and `round(100 * (done + 0.5 * partial) / n)`, the
  overview's formula. For each headline item one word from its status: done "met"; partial "partly met", followed by
  its `gap`; todo "not started". For the milestone one word: "met" only if every headline item is done; "not started"
  if every headline item is todo; "not met" otherwise. The word is never derived from the per cent. A headline item
  whose comparison ran and missed is a partial item with a gap in this roadmap, so "not met" for a single item cannot
  be told from its status; it is carried by the milestone's word, and the gap says what misses.
* The block, between `<!-- roadmap-milestones begin (generated by scripts/roadmap_milestones.py; do not edit) -->` and
  `<!-- roadmap-milestones end -->`: a heading "### Simulation milestones", one paragraph that says what the per cent
  measures and that only the column "Headline result" says whether the simulation agrees with its reference, a table
  (Milestone, Items, Done, Partly done, Not started, Per cent, Headline result) whose first row is the whole roadmap,
  then for each milestone its result, cases, reference and each headline item with its word and gap.
* Commands, all with `--root`: no option rewrites the block (the page must carry the markers); `--check` writes
  nothing and exits 1 on a validation line or a block that is not current; `--install` puts the two markers after
  the end marker of the process report's block, once; `--apply <mapping> [--definitions <file>]` sets the key in
  `docs/roadmap.json` from a mapping file (tab-separated: item id, milestone id, reason; lines that start with `#`
  are comments), replacing the membership of every milestone the mapping names and of no other, writes the list in
  the order of the definitions, copies the definitions to `docs/milestones.json` when given, validates before it
  writes, and refuses to write if the roadmap file as it stands is not what `json.dumps(..., indent=2,
  ensure_ascii=False)` gives (so that it never reformats). No figure file is registered.
* With no key anywhere and no definitions file, `--check` exits 0, the plain command writes nothing, and the markers
  are not required. Markers in the page without a definitions file are a failure of `--check`.

**Where the definitions are delivered.** Definitions without members fail V4, so `docs/milestones.json` cannot sit on
a tree whose roadmap has no key while the live check of CI passes. It is delivered beside this ADR as
`docs/adr/0756-f865-milestones.json`, the mapping as `docs/adr/0756-f865-mapping.tsv`, and the lead's command
installs both. This is the case the brief foresees ("if that cannot hold, deliver it beside the ADR").

**The live test.** `scripts/test_roadmap_milestones.py` has one test that runs `--check` on the repository itself, so
that CI (`unittest discover`) fails when the block is not current or the keys and definitions disagree. On this
branch it passes with nothing defined.

**How the membership rule is applied** (the rule is the lead's; this is its reading, fixed before the mapping).
Each definition names its run of record: the test files or scripts that set up and run the simulation and compare it.

* M1, headline: the item is a headline item.
* M2, used: the run of record uses what the item delivered: a solver algorithm, a discretisation, a turbulence model,
  a boundary condition, a mesh generator, a linear solver, the report that gives the compared quantity, the case or
  settings layer it is run through. Evidence: the test or case text names it, or the item is in the dependency
  closure of a headline item and its subject is in the run. Being in the closure alone is not enough.
* M3, data: the item holds the case definition or the reference data the comparison reads.
* M4, trust: the item's subject is a defect, a refused set-up, a divergence or a missed limit that the milestone's
  own run met, open or closed, or a mesh or time-step study of that run.
* Not a member: N1 an item that uses the milestone's case as a test of something else (an immersed boundary on the
  cylinder, a statistics check on a shedding run, a fingerprint of a fan mesh); N2 an item that consumes the
  milestone's result (a design study on the fan, a drawing of its mesh); N3 workspace, CI, process, documentation,
  Studio and assistant items, F-001 among them; N4 an alternative that the run of record does not use (another
  turbulence model, another pressure algorithm); N5 no relation.
* The 308 items of the line "Structural solver" are read by title only; an item of that line is a member only if
  its title names the milestone's flow simulation.

Every membership gets one line that starts with M1, M2, M3 or M4. If a class of items cannot be placed by M1 to M4
and N1 to N5, the work stops and reports the class, as the brief's rule says.

**The check of the mapping (fixed before the mapping exists).** The mapping file is committed first. Then
`scratch/sample.py` draws with `random.Random(865756)`, in this order: 20 pairs by `sample` from the member pairs
(item, milestone) sorted by item number and then by the order of the definitions; 10 pairs by `sample` from all
non-member pairs sorted the same way; 10 by `sample` from the non-member pairs not yet drawn whose item is in one of
the lines of work Numerics and solvers, Turbulence, Heat, Compressible, Multiphase, Moving meshes, Parallel, Meshing,
Post-processing, Command line (the doubtful half; a uniform draw of non-members is two fifths structural items).
The 40 pairs are read a second time against the rule by a reader that is given the rule as written above, the three
definitions, and each item's id, title, acceptance and gap, with the repository to consult, and is not given the
mapping, the reasons or which half a pair came from: a fresh agent, since no second person is at hand. Reported: the
number of disagreements in each half and every disagreeing pair with both readings. A disagreement is a result. A
correction I accept after it is a separate commit of the mapping, listed in the addendum with the figures before and
after; the pairs are not redrawn.

**The further milestones** are proposed under "Open questions for the maintainer" in the addendum with title,
reference, headline items and a rough member count by keyword search, marked rough; they are not mapped.

## Table of checks

Every clause of the acceptance of F-865 first. `T` rows are unit tests of `scripts/test_roadmap_milestones.py` or
`scripts/test_render_roadmap.py`, on roadmaps in temporary folders; each new test fails before the code (the module
or the behaviour does not exist).

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | the key: `schema_problems` accepts `milestones` as registered and names a malformed one; the item block shows the line | fails before, passes after | acceptance of F-865 ("an item may name the simulation milestones it belongs to") | passes | n/a | code verification |
| A2 | V1 to V7, one test each: the problem line is named and `--check` exits 1 | each fails before, passes after | acceptance of F-865 ("validates the two against each other"); the brief, task 1 | passes | n/a | code verification |
| A3 | figures on a sample of 6 items in 2 milestones: counts, 50 % for one done and one todo, 75 % for one done and one partial, whole-roadmap row equal to the overview's formula | equal to the hand values | acceptance of F-865 ("the per cent with a partly done item as one half") | equal | n/a | code verification |
| A4 | a milestone whose members are 9 done and 1 partial headline reads 95 % and "not met", with the gap; all headline done reads "met"; all todo "not started" | the words as registered | acceptance of F-865 ("never reads as met") | as registered | n/a | code verification |
| A5 | the block: written between its markers, `--check` 0 when current and 1 after a status change, the process report's block untouched byte for byte | as stated | acceptance of F-865 ("a generated block ... that CI checks") | as stated | n/a | code verification |
| A6 | the live test passes on this tree and is collected by `python -m unittest discover -s scripts -p "test_*.py"` | passes | the brief, task 1 | passes | n/a | code verification |
| A7 | without the key and the file: `render_roadmap.render_items` of the live roadmap, `docs/ROADMAP.md`, `docs/roadmap-overview.md` and the two SVGs are byte for byte what they are at c60965fc; `process_report.py --check` names no file made stale by this work | 0 bytes differ | acceptance of F-865, last clause but one; rule 3 | 0 | n/a | unchanged default output |
| A8 | `--apply` on a copy of the live roadmap with the delivered mapping and definitions: validation clean, every item outside the mapping unchanged byte for byte, a second `--apply` changes nothing, `render_roadmap.py --check` clean after a render | as stated | the brief, task 2 | as stated | n/a | code verification |
| A9 | the three milestones defined; every item mapped (member with a reason, or not); member counts and figures on c60965fc stated | each milestone has at least one headline item the roadmap holds | acceptance of F-865; the brief's decision rule | 3 of 3 | n/a | code verification |
| A10 | the sample check as registered: 40 pairs by seed 865756, second reading, disagreements listed | made and reported; no limit on the number | the brief, task 4 ("a disagreement is a result") | under 8 of 40 | n/a | code verification |
| A11 | further milestones proposed with title, reference, headline items and a rough count | at least the nine classes the brief names are answered | the brief, task 3 | 9 or more | n/a | code verification |
| E | every existing test of `scripts/test_render_roadmap.py` and the other script test files passes, unedited | all pass | rule 2 | pass | n/a | code verification |
| P | `python scripts/process_report.py --check`; `cargo test -p rcfd-cli --release --test docs_site` | 0 failures other than a generated file not current; passes | rule 7, version 4 | as stated | n/a | code verification |
| M | the mutations below | each caught by the named test | rule 16; the brief, task 5 | each caught | n/a | code verification |

No row is of kind validation: the item's acceptance claims no agreement with a measured case.

## Negative controls and mutations

Run on a scratch copy; the two marked are committed as patches beside the test file.

| Id | Mutation | Must be caught by | Committed |
| --- | --- | --- | --- |
| M1 | V2 is dropped: a headline item that is not a member passes | A2 | `test_roadmap_milestones.validation.mutation.patch` |
| M2 | a partly done item counts 1, not one half | A3 | `test_roadmap_milestones.figures.mutation.patch` |
| M3 | V1 is dropped: an unknown milestone id passes | A2 | no |
| M4 | the milestone's word is "met" when its per cent is 90 or more | A4 | no |
| M5 | `--check` never compares the block | A5 | no |
| M6 | `item_block` writes a line `milestones: none` for an item without the key | A7 (and A1) | no |

Negative control of A7: the same comparison after `--apply` on a copy must differ (the key is seen).

## Decision rule

Done only if A1 to A8, E, P and M hold (the mechanism has its tests and leaves today's output unchanged without the
key), A9 holds with every item mapped and each membership reasoned, A10 was made and reported, and A11 is proposed.
Partial if a milestone has no headline item that the roadmap holds (the addendum says which item would have to be
entered) or the sample check was not made. Stopped and reported if the membership rule cannot be applied to a class
of items (the class, with examples).

## What is not covered

No figure file. No edit of `docs/roadmap.json`, `docs/ROADMAP.md`, `docs/roadmap-overview.md`, the SVGs or
`docs/adr/index.md` (rule 9): the lead applies the mapping. `scripts/roadmap_shards.py` is not read against the new
key. The notes of the items are read only around keyword matches and for the eleven items named above; the second
reading is of 40 pairs, not of the mapping. The published references are cited from the repository's rows, not from
the papers. No cargo build or test other than `docs_site` (the brief).

## Addendum 1: results (2026-11-21)

Commits: 07e84341 (this registration, alone), a9da6717 (code and tests), 8e9c4999 (two mutation patches), ae779911
(definitions and mapping, first reading), 9588bfa1 (three corrections of the mapping), and the commit of this addendum.
No amendment was needed.

| Id | Measured | Met |
| --- | --- | --- |
| A1 | `Key`, 3 tests of `scripts/test_roadmap_milestones.py`, pass; before the code the file failed at import | yes |
| A2 | `Validation`, 8 tests (V1 to V7, ten forms of V6), and `Command.test_each_validation_problem_fails_the_check`, pass | yes |
| A3 | `Figures.test_counts_and_per_cent_...`: 50, 50, 75, 67 and 69 for 385/293/93 | yes |
| A4 | 9 done and a partial headline: 95 % and "not met" with the gap; the three words from the headline items alone | yes |
| A5 | install, write, check 0, check 1 after a status change, the process report's block byte for byte | yes |
| A6 | `Live` passes; `python -m unittest discover -s scripts -p "test_*.py"`: 304 tests, OK (5 skipped), 91 s | yes |
| A7 | `git status` after every run: no file under `docs/` changed but this ADR's three files; `render_roadmap.py --check`, `roadmap_progress.py --check`, `roadmap_milestones.py --check` each exit 0 on the tree; the plain command writes nothing. Negative control: after `--apply` on a copy `render_roadmap.py --check` exits 1 | yes |
| A8 | on a copy of the live roadmap: 61 memberships written, 32 items changed and each only by the key, the other 739 equal, validation and `--check` clean, `render_roadmap.py --check` 0 after a render, a second `--apply` byte-identical | yes |
| A9 | below | yes |
| A10 | below: 4 disagreements of 40 | yes |
| A11 | below, 12 proposed | yes |
| E | the discover run above; no existing test edited | yes |
| P | `python scripts/process_report.py --check`: 1 failure, "generated file is not current: docs/adr/index.md", left to the lead (rule 9); `cargo test -p rcfd-cli --release --test docs_site`: 2 passed | yes |
| M | M1 to M6 each fail `scripts/test_roadmap_milestones.py` (M1: the V2 test and two command tests; M2: three figure tests; M3: the V1 test and one command test; M4: the 95 % test; M5: the install test; M6: two key tests, and the live `render_roadmap.py --check` exits 1). M1 and M2 are committed as patches | yes |

**The three milestones on the roadmap of c60965fc** (the whole roadmap: 771 items, 385 done, 293 partly done, 93 not
started, 69 %).

| Milestone | Headline | Members | Done | Partly done | Not started | Per cent | Headline result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `vortex-shedding` | F-029 (done) | 18 | 15 | 3 | 0 | 92 % | met |
| `fan` | F-126 (partial) | 22 | 17 | 5 | 0 | 89 % | not met |
| `aerofoil-lift` | F-043 (partial) | 21 | 17 | 4 | 0 | 90 % | not met |

With the first reading (before the corrections): fan 22 members, 18/4/0, 91 %; aerofoil lift 20 members, 16/4/0, 90 %.
Every item is mapped: the 61 memberships of 32 items are in `docs/adr/0756-f865-mapping.tsv`, each with its reason;
the other 739 items are members of none. The non-members by class: all 308 structural items (N5, no title names one
of the three flows), the Studio, assistant, process and design-study lines (N3, N2), the immersed, overset, sliding
and moving-mesh items (N1 or N5), the models and algorithms the runs of record do not use (N4), and F-001 and F-003
(N3, workspace foundation; F-003 is the nearest to a member and is named here as the borderline).

The command the lead runs on main, then the usual regeneration:
`python scripts/roadmap_milestones.py --apply docs/adr/0756-f865-mapping.tsv --definitions docs/adr/0756-f865-milestones.json --install`
and `python scripts/render_roadmap.py`.

**The sample check.** Drawn by `sample.py` (scratch) with `random.Random(865756)` from the mapping of ae779911: 60
member pairs, 2253 non-member pairs, 826 of them in the doubtful lines. Second reading by a fresh agent with the rule,
the definitions and the sheet, not the mapping. Disagreements: 1 of 20 in the member half, 3 of 20 in the non-member
half (0 of the 10 uniform, 3 of the 10 doubtful).

| Pair | First reading | Second reading | Taken |
| --- | --- | --- | --- |
| F-295, fan | member, M4 (the seam diffusivity was made because of the seam of FAN-01) | not, N4: no listed fan run sets the option | second; removed |
| F-011, aerofoil-lift | not | member, M2: the sweep runs `rcfd check-mesh` on every O-grid | second; added |
| F-106, fan | not (closure only) | member, M2 (close): arms S8, S9 of `fan01_stall_w43c.rs` use the pseudo-transient step | second; added |
| F-082, aerofoil-lift | not (the subject of the item is programmatic use) | member, M2 (close): `Simulation` is the engine behind `rcfd run` | first; kept |

The reader also named as hard: F-210 for vortex shedding (a workaround arm, read as a member by both), and that the
aerofoil headline F-043 is the SST arm while the lift curve of record is the SA arm. What the check shows: the rule is
least sharp where a run of record has diagnostic arms, and where an item is read by its title or by what it delivered.

**Deviations.** (1) The registration says of the uniform non-member draw "two fifths structural"; the share is 308 of
771. (2) F-006 is not a member of vortex shedding: `cases/` holds no cylinder case. (3) `scripts/roadmap_shards.py`
was not run against a roadmap with the key. Not run: any cargo command but `docs_site`.

Actual size: done; 0; 0; 4; about 190000.

## Open questions for the maintainer

1. **The aerofoil headline.** The clause of F-043 is "NACA0012 validation passes" for k-omega SST, and its gap reads
   "NACA0012 not done"; the lift curves that were run (ADR 0159, 0167: steady SA, cl within 0.013 of Ladson from 2 to
   15 degrees, as the head of ADR 0167 states) are held by no item. Options: (a) keep F-043 as the only headline; (b)
   enter an item "NACA 0012 lift curve against Ladson at Re 6e6" whose acceptance is the registered comparison of
   ADR 0167, and make it the headline beside F-043. Recommendation: (b). Not taken: it changes the scope of the roadmap.
2. **The drag of the cylinder.** The acceptance of F-029 is the Strouhal number alone; the mean drag is compared by
   `validation_cylinder_re100.rs` (ADR 0060) under no item. Options: (a) leave it; (b) enter an item for the K1 to K4
   checks as a second headline. Recommendation: (b), with question 1.
3. **The fan headline.** F-126 is the rotating-interface item; its gap says the acceptance is "not attempted" while
   `fan01_validation.rs` is its attempt on FAN-01 and misses. Options: (a) keep; (b) enter an item whose acceptance is
   the FAN-01 design point (126.46 Pa, 333.30 W) within a registered band, as the headline. Recommendation: (b).
4. **Further milestones.** Proposed; the member counts are rough (a keyword search of titles and acceptances, not a
   mapping). Recommendation: approve the first six now, the others when their headline item exists.

| Proposed milestone | Reference (as the repository records it) | Headline items | Rough count |
| --- | --- | --- | --- |
| Lid-driven cavity | Ghia et al. (1982), `docs/reference-data.md` | F-028 | 7, plus the common core of about 12 |
| Backward-facing step | Gartling (1990) Re 800, secondary sources, `docs/reference-data.md`; the numbers of Armaly not obtained | F-042, F-043 (clauses) | 5 |
| Flat plate skin friction | NASA TMR and correlations, `docs/reference-data.md` | F-041, F-043, F-203 | 11 |
| Heated cavity | de Vahl Davis (1983) Table V, read (F-006 note) | F-050 | 2 |
| Shock tube and wedge | exact Riemann solution and oblique-shock relation (closed forms); Sod (1978) not obtained | F-060 | 3 |
| Packed bed | Ergun pressure drop; not read by me | F-031 | 2 |
| Conjugate heat transfer | analytic slab; no measured case | F-051 | 4 |
| Immersed cylinder, DFG 2D-2 | FeatFlow benchmark page, read 2026-11-07 (`docs/reference-data.md`) | F-219, F-346, F-354 | 24 |
| Overset moving cylinder | a body-fitted run of this repository; no published reference | F-134 | 26 |
| Rotor-stator sliding interface | none read | F-250, F-303 | 10 |
| Rotor with cyclic symmetry (structural) | not read | none found by title; an item would have to be named | 15 |
| Dam break and rising bubble | Hysing et al. (2009) named in F-055; not read by me | F-055 | 2 |

## For the lead: roadmap entries

> Status proposed for F-865: done.

> Note entry: ADR 0756, 2026-11-21, done: the key `milestones`, `scripts/roadmap_milestones.py` with validation, figures, a generated block of the overview and `--apply`; the three milestones defined and 61 memberships of 32 items delivered with reasons (vortex shedding 18 items, 92 %, met; fan 22, 89 %, not met; aerofoil lift 21, 90 %, not met); sample check of 40 pairs, 4 disagreements, 3 corrections taken; the mapping is applied by the lead with the command of addendum 1.

> Follow-up (proposed, question 1): "NACA 0012 lift curve against Ladson at Re 6e6", deps F-041, F-043; acceptance: the lift comparison registered in ADR 0167 (the limits of that ADR) holds as a recorded test; milestones aerofoil-lift, headline.

> Follow-up (proposed, question 3): "FAN-01 design point against the measured characteristic", deps F-126, F-133; acceptance: total-to-static pressure rise and shaft power at 1.4032 m^3/s against 126.46 Pa and 333.30 W within limits handed to the registration; milestones fan, headline.

## The lead's reading (2026-11-21), after the merge

Merged at 6637d902 (push run 4.1 minutes, two mutation patches caught). The lead then ran the command of the addendum on
main: 61 memberships of 32 items written to `docs/roadmap.json`, `docs/milestones.json` installed, the block of
`docs/roadmap-overview.md` generated. Status: done by the registered rule.

What the figures do and do not say, for the maintainer:

- The three per cents are 92, 89 and 90. They are high because the members of a milestone are, by the rule the lead gave,
  what its run of record uses, and those are mostly foundation items done long ago. The open work of the fan and of the
  aerofoil is not spread over many items: it sits inside one partly done headline item each (F-126, F-043). The per cent
  therefore moves little while the headline result stays "not met". The column "Headline result" is the one to read.
- The fan milestone is the validation of FAN-01 at its design point. The items of the fan design goal of
  `docs/milestone-fan.md` (noise, the operating map, the optimiser, the reduced-order model, the structural
  demonstrations) are not members: 19 items that name a fan in their title belong to no milestone.
- Questions 1 to 3 (a headline item of its own for the aerofoil's lift curve, for the cylinder's drag and for the FAN-01
  design point) and question 4 (the further milestones) are the maintainer's and are put with the agent's
  recommendations, together with one of the lead's: whether the fan milestone is the validation alone or the design goal.

## The maintainer on the scope of the fan milestone (2026-11-21)

The lead asked whether the fan milestone is the validation of FAN-01 alone or the whole design goal of
`docs/milestone-fan.md`, and recommended the whole goal. The maintainer answered: "Whole design goal". The milestone `fan`
is redefined by roadmap item F-867 (ADR 0757). Questions 1 to 4 of this ADR are not answered by this and stay open.

## Answers of the maintainer (2026-11-21)

The lead put the open questions of ADR 0754, 0755 and 0756 to the maintainer on 2026-11-21 as four decisions, each with its
recommendation, and asked whether to take all four. The maintainer answered: "Yes, all four".

For this ADR: questions 1 to 3, option (b) each: an item of its own for the NACA 0012 lift curve (F-868), for the mean
drag of the cylinder (F-869) and for the FAN-01 design point (F-870), each a headline of its milestone beside the present
one. Question 4: the first six further milestones are approved (the lid-driven cavity, the backward-facing step, the flat
plate, the heated cavity, the shock tube and the wedge, the packed bed); their definitions and mapping are roadmap item
F-871. The other six wait until a headline item exists for each.

The three new items are entered as not started. Registered runs of the first two exist (ADR 0167; ADR 0060 K1 to K4):
whether they meet the item is decided by a registration of that item, not by the lead on entry. So the headline of vortex
shedding reads "partly met" from this day, where it read "met" on the Strouhal number alone.
