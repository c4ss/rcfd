# ADR 0757: F-867 registration: the fan milestone as the whole fan design goal

Status: **REGISTERED 2026-11-21, written before any code of F-867 and before any run of its checks.** Before this text no
definition, mapping or sample of F-867 was written. What was done: reading (below) and reading scripts over
`docs/roadmap.json` kept in the scratch folder (`items.py`, `show.py`, `grepn.py`: id, status, line of work, title, deps,
acceptance and gap of every item, and the text around keyword matches in the notes). Results go in addenda at the end;
nothing above them is changed after a run.

It answers the brief `docs/adr/briefs/f867-fan-milestone-design-goal.md` and follows `docs/process/agent-rules.md`,
version 4, under which the brief was issued. Branch `f867-fan-milestone-design-goal` from main 1a5bbac5, moved forward
before this commit to main 059c9423 on the lead's message of 2026-11-21 (below); scratch
`w49i-work/f867-fan-milestone-design-goal-scratch`. This work writes no library, script or test code.

The lead's message, received while this text was being written and before its commit: the maintainer answered ADR 0756
question 3, and main 059c9423 holds F-870, "FAN-01 at its design point: pressure rise and shaft power against the
measured characteristic", not started, a member and headline of the milestone fan beside F-126; F-868, F-869 and F-871
are entered and are not this work's to map beyond the fan; the validation headline of this definition is to be F-870,
with F-126 a member or a headline as the rule decides; the other two milestones are to be left as main holds them when
the command is run; the rules are now version 5, whose rule 9 asks in addition for the roadmap entries as
`docs/roadmap/proposals/0757-F-867.json`, asked of this work, not required. Everything below is written for main
059c9423.

Size (agent): done; 0; 0; 5; 260000

## What is asked

Roadmap item F-867. The milestone `fan` of `docs/milestones.json` becomes the design goal of `docs/milestone-fan.md`,
with a headline item, or the words that no item carries it, for every link of that page's capability chain. Every item
of the roadmap is read against a registered membership rule, every membership has a reason, and every item that names a
fan in its title is a member or is listed with the reason it is not. A sample drawn by seed is read a second time and
its disagreements are reported. One command replaces the fan milestone on main and leaves the other milestones byte for
byte as they are. The maintainer's decision (ADR 0756, "The maintainer on the scope of the fan milestone", 2026-11-21):
"Whole design goal".

## What was read

Whole: the brief; `docs/process/agent-rules.md` (version 4); `docs/milestone-fan.md`; `docs/fan-project-breakdown.md`;
`docs/milestones.json`; `docs/adr/0756-f865-mapping.tsv`; `scripts/roadmap_milestones.py`; ADR 0756. Of
`docs/design-loop.md`: lines 1 to 40. Of `scripts/roadmap_progress.py`: lines 1 to 45 (the lines of work). The leading
comments of `crates/rcfd-api/tests/design_e2e_w32c.rs`, `crates/rcfd-api/tests/fan_sector.rs`,
`crates/rcfd-post/tests/fan_rom_w46b.rs` and `crates/rcfd-mcp/tests/design_f260.rs`; the list of test files whose name
holds `fan`, `design`, `fwh` or `f697` to `f701`. Of `docs/roadmap.json`, by script: for every one of the 777 items (773 at 1a5bbac5, and F-868 to F-871 with their acceptance and note at 059c9423) the
id, status, line of work and title; for the 469 items outside the line "Structural solver" the acceptance cut at 40 to
170 characters; the acceptance and gap in full or cut at 420 to 700 characters of 35 items (F-046, F-085, F-094, F-095,
F-109, F-113, F-119, F-121, F-123, F-126, F-131, F-133, F-136, F-137, F-147, F-148, F-217, F-218, F-232, F-243, F-249,
F-260, F-267, F-274, F-287, F-315, F-329, F-339, F-701, F-702, F-713, F-716, F-722, F-814, F-857) and the note of F-095
from "WAVE 32C"; for every item, 130 characters around the first match, in title, acceptance, gap and note, of "ADR
0065", "ADR 0067", "ADR 0077", "ADR 0118", "ADR 0119", "fan_rom", "ADR 0116", "FAN-01", "axial_fan_sector",
"ComputerFan", "operating map", "rcfd map", "Gutin", "tonal", "divergence guard" and "fan". Not read: `docs/acoustics.md`,
`docs/structural-solver.md`, `docs/validation-data.md`, the ADRs the two pages cite, the notes of the items beyond those
matches.

## Readings (before any mapping)

* `docs/roadmap.json` holds 777 items: 387 done, 294 partly done, 96 not started. 23 items name `fan` in the key
  `milestones` (17 done, 5 partly done, 1 not started: 85 per cent; headline F-126 partly done and F-870 not started).
  At 1a5bbac5, before F-870: 22 members, 89 per cent.
* 21 titles match the registered pattern for "names a fan" (below): F-113, F-217, F-260, F-315, F-387, F-406, F-436,
  F-593, F-604, F-697, F-699, F-701, F-713, F-716, F-722, F-733, F-778, F-819, F-865, F-867, F-870. ADR 0756 counts 19
  without a milestone; F-867 and F-870 were entered since.
* `roadmap_milestones.py --apply MAPPING --definitions FILE` replaces the members of every milestone the mapping names
  and of no other, and copies FILE over `docs/milestones.json` whole. A mapping that names only `fan` therefore leaves
  the keys of the other two milestones; the definitions file must carry the other two definitions unchanged in bytes.
  No defect of the script stops this work; the script is not changed.
* The notes of the items name the fan runs thinly: no item's title, acceptance, gap or note matched "ADR 0075", "ADR
  0078", "ADR 0086", "ADR 0110" or "ADR 0111" (the validation and stall records of the page). The record of those runs
  is the page and the test files, and F-126 by the words of `fan01_validation.rs` (ADR 0756).
* The page's chain table is dated 2026-10-18. Its rows "Operating map: none (F-254 candidate)" and "Optimiser: none
  (F-095 partial, F-257 candidate)" are older than the roadmap: the note of F-095 says the candidates F-254 and F-257
  "are not present as ids in this file" and records `rcfd map`, the library `design::opmap`, the samplers and
  optimisers, the store and ten `design_*` MCP tools (wave 32C, ADR 0067). F-254 is today a particle item.

## The definition (fixed now)

* `id` `fan`, `page` `docs/milestone-fan.md`: unchanged. `title`: "Assistant-driven fan design".
* `result`: "Claude, through the assistant tool layer, iterates on fan shape and size, sets up and runs the test cases,
  reads the results, and converges on designs that keep noise low while flow is high across a range of operating
  conditions." (the goal of `docs/milestone-fan.md`, section "Goal", in one sentence).
* `reference`: the measured characteristic of FAN-01 in the words `docs/milestones.json` carries today (I did not
  re-read `docs/validation-data.md`), followed by: no measured fan noise is recorded
  (`docs/fan-project-breakdown.md`, part D5: "missing (needs data)").
* `cases`: the five FAN-01 files of today, and for the other links `cases/axial_fan_sector`,
  `crates/rcfd-api/tests/fan_sector.rs`, `crates/rcfd-post/tests/fan_rom_w46b.rs`,
  `crates/rcfd-api/tests/design_opmap_w32c.rs`, `crates/rcfd-api/tests/design_e2e_w32c.rs`,
  `crates/rcfd-api/tests/f697_fan_objective.rs`. These are the runs of record of the rule below.
* `headline`: the items whose acceptance, or whose gap as the roadmap states it, is a result the goal needs and that
  can be met or missed. Fixed now, from the readings:

| Result the goal needs | Headline item | Why this item | What would replace it |
| --- | --- | --- | --- |
| The fan solve agrees with a measured fan | F-870 (not started) | its acceptance is the FAN-01 design point (126.46 Pa, 333.30 W) within a band registered before the run; entered on the maintainer's answer to ADR 0756 question 3 | none. F-126 stops being a headline item: its acceptance is the rotating interfaces on a public compressor case, and the result the goal needs from it is now the acceptance of F-870. It stays a member by M5 (L3: its gap holds MRF on the bladed FAN-01 passage) |
| Noise prediction | F-243 (partly done) | the page names it for the link; its gap names "comparison with a measured fan, proxy calibration" as not built | an item whose acceptance is a fan noise number against a stated reference (open question) |
| Operating map | no item | F-095 and F-329 carry the map mechanism and are done on channel cases; no item's acceptance is the characteristic curve of a fan from converged solves | such an item (open question) |
| Optimiser converging on a fan design | F-260 (partly done) | its acceptance holds a multi-objective study on the FAN-01 passage with the Pareto members re-run within 1 per cent | none |
| The design loop driven by the assistant | F-702 (not started) | its acceptance is that the objective, surrogate, fidelity driver, hypothesis record and ledger are reachable through MCP tools | an item for one full assistant-driven iteration on a fan (wave 34 of the page; open question) |

The milestone's word follows from these four items by the script's rule: "met" only when all four are done.

## The capability chain and the items that carry each link (read before the mapping)

Links L1 to L9 are the nine rows of the table "Capability chain and current state" of `docs/milestone-fan.md`; B1 to B5
are the parts of `docs/fan-project-breakdown.md` that the brief adds and the page's chain has no row for. "Carried by"
is what the readings above show; the mapping may add members to a link but changes no headline.

| Link | Carried by | Headline |
| --- | --- | --- |
| L1 parametric blade and hub geometry | F-133 (its note records `rcfd_mesh::fan`, ADR 0065); the schema fans F-713, F-722; export F-119 (breakdown A4). The constraint checker and the perturbation operators (breakdown A2, A5; ADR 0118): no item | none: no item's acceptance is the geometry |
| L2 volume mesh of a periodic blade passage | F-133; the C-grid constructions F-726, F-777, F-778, F-782, F-784, F-791; the quality measures F-011; breakdown B5 names F-131, F-232 | none of its own; the missed quality gates are in the gap of F-133 |
| L3 rotating-frame steady solve | F-031, F-107, F-043, F-040, F-028, F-030, F-106 (ADR 0756), F-126 (its gap: MRF on the bladed passage) | none of its own: the solve is judged by F-870 |
| L4 fan validation | F-870; the present attempt `fan01_validation.rs` under F-126; the data F-125 | F-870 |
| L5 operating map | F-095 (`rcfd map`, `design::opmap`), F-329, F-287 (the map on the runner); the metrics F-123 (breakdown C4); the system curve F-113 (breakdown C5) | no item |
| L6 noise | F-243, F-136 | F-243 |
| L7 optimiser | F-095, F-249, F-297, F-310, F-260, F-315; the surrogate F-698 and its follow-ups | F-260 |
| L8 assistant tools | F-090, F-091, F-147; the `design_*` tools of F-095; F-339, F-702, F-714 | F-702 |
| L9 solver robustness under automation | the PIMPLE guard in F-095 (ADR 0067); the limiter in F-021; the steady guard of ADR 0077 and the stall diagnoses of the page: no item has them as its subject | none |
| B1 reduced-order tier (breakdown C7) | the model of ADR 0119 has no item of its own (it is recorded in the note of F-697); the breakdown names F-218 and F-113 | none |
| B2 objective (H1) | F-697 | none |
| B3 surrogate and fidelity ladder (H3, H7) | F-698, F-699 and their follow-ups | none |
| B4 design ledger (H4) | F-268, F-700 | none |
| B5 long-horizon loop (H6) | F-701 and its follow-ups; F-702 | F-702 |

## The membership rule (fixed before the mapping)

The classes M1 to M4 and N1 to N5 of ADR 0756 stay; they are read against the design goal. The runs of record are the
`cases` of the definition above.

* M1, headline: the item is one of the four headline items.
* M2, used: a run of record uses what the item delivered. For the five FAN-01 files this is the reading of ADR 0756 with
  its three corrections: its 21 memberships of class M2 and M3 are carried over with their reasons and are not read
  again. For the added runs of record an item is M2 only where the test or case text, the page or the item's own text
  names the use; being in a dependency closure alone is not enough.
* M3, data: the item holds the reference data or the case definition a comparison of the goal reads.
* M4, trust: the item's subject is a defect, a refused set-up, a divergence or a missed limit that a run of record met,
  or a mesh or time-step study of such a run.
* M5, link (new): the item's own subject is a link of the chain. One of: (a) its title or acceptance delivers a
  capability that the column "Needed" of the page's chain names, or that a part of the breakdown inside the goal names
  (parts A1, A2, A4, A5, B1, B2, B5, C1, C4 to C7, D1 to D4, H1 to H7); (b) the page or the breakdown names the item
  by its id for such a link or part; (c) the item is a follow-up whose acceptance is a check, a repair, an extension or
  an exposure (case key, command, MCP tool, runner) of a capability that is a member by (a) or (b). The reason names
  the link (L1 to L9, B1 to B5).
* For L8 (assistant tools) an item is a member only if it delivers or changes tools through which an assistant creates
  a case or a variant, meshes, runs, sweeps, fetches metrics, compares or proposes, or the layer that grants and
  bounds those calls. A surface for a person (chat sidebar, panels, viewers), provider and key plumbing, packaging and
  project bookkeeping are N3. A Studio item is a member only if its subject is the assistant's driving of runs.
* Not a member: N1 an item that uses a fan case or mesh as a test of something else; N2 an item that consumes the
  goal's result; N3 workspace, CI, process, documentation, surfaces for a person, F-001 among them; N4 an alternative
  or an extension that no run of record uses and no link names (another turbulence model, a gradient-based optimiser,
  acoustic propagation in ducts, a held default change that "waits until the fan path needs it"); N5 no relation.
* N6 (new), left out of the goal by the pages: what `docs/milestone-fan.md` does not put in the goal or leaves to the
  user. (i) Structure and vibration: the structural solver, rotor dynamics, load export, FSI and co-simulation
  (breakdown E; the page: "this milestone iterates geometry, size and speed only unless the user later adds
  structural coupling", and lists structural coupling under "Open decisions for the user"). This includes the
  structural demonstrations on a fan blade and rotor that the brief names; they are listed, not mapped, and put as an
  open question. (ii) Degradation: dust, wear, ageing (breakdown F2, F3). (iii) The full-annulus and sliding-interface
  transient route (breakdown B3, B4, C2): the chain's solve is "MRF + periodic sector + SST", and ADR 0756 proposes the
  sliding interface as a milestone of its own. (iv) Centrifugal variants (A3), transition modelling for chord Reynolds
  numbers near 1e5 (C3) and the small-fan data (G1, G2): the computer-fan plan, not the page's goal. The tolerance
  study driver (F1) is inside, through the ledger item that carries it (H4).
* The 308 items of the line "Structural solver" are read by title only and are N6 or N5.
* "Names a fan in its title": the title matches `\bfans?\b` without regard to case, or holds "FAN-01".

Every membership gets one line that starts with M1 to M5. If a class of items cannot be placed, the work stops and
reports the class.

## The check of the mapping (fixed before the mapping exists)

The mapping file is committed first. Then `sample.py` (scratch) draws with `random.Random(867757)`, in this order: 20
items by `sample` from the members sorted by item number; 10 by `sample` from the non-members sorted the same way; 10
by `sample` from the non-members not yet drawn whose line of work is one of Design studies and optimisation, Claude
and the MCP server, Studio desktop app, Compressible flow and acoustics, Moving meshes, rotors and coupling, Meshing
and mesh formats, Numerics and solvers, Turbulence, Post-processing and output, Command line, Python and API (the
doubtful half). The 40 pairs (item, fan), sorted by item number, are read a second time by a fresh agent that is given
this section's rule and the definition as a file, the two pages, and for each item its id, title, status, deps,
acceptance and gap, in a folder of its own; it is told not to open this ADR, the mapping, `docs/roadmap.json`,
`docs/milestones.json`, ADR 0756 and its mapping, or the scratch folder, and not which half an item came from.
Independence rests on that instruction, not on isolation. If no agent can be started, the addendum says how the second
reading was made. Reported: the disagreements in each half and every disagreeing pair with both readings and the
reading taken, with its reason. A correction is a separate commit of the mapping; the pairs are not redrawn.

## Table of checks

Every clause of the acceptance of F-867 first. No row has a numeric limit of my own; each limit is a clause.

| Id | Check | Limit | Source of the limit | Estimate | Margin | Kind |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | every link L1 to L9 and B1 to B5 has an item or the words "no item", for its carriers and for its headline | 14 of 14 | acceptance of F-867 ("a headline item, or the words that no item carries it, for every link") | 14 | n/a | code verification |
| C2 | every item is read against the rule; each membership has a one-line reason that starts M1 to M5 | 777 of 777 read; 0 memberships without a reason | acceptance of F-867 ("every item ... is read", "every membership has a reason") | as stated | n/a | code verification |
| C3 | every item whose title matches the pattern is a member or is listed with its reason | 0 of 21 unplaced | acceptance of F-867 | 0 | n/a | code verification |
| C4 | the sample check as registered: 40 pairs by seed 867757, second reading, disagreements listed and decided | made and reported; no limit on the number | acceptance of F-867; the brief, task 4 | under 10 of 40 | n/a | code verification |
| C5 | the command on a scratch copy of the `docs/` of main 059c9423: the definitions of `vortex-shedding` and `aerofoil-lift` equal in bytes; for every item the list of its milestones without `fan` equal; every item whose fan membership does not change equal in bytes; the lines of the generated block for the other two milestones equal; a second run changes nothing; `roadmap_milestones.py --check` and `render_roadmap.py --check` (after a render) exit 0 | 0 bytes differ | acceptance of F-867 (last clause); rule 3 | 0 | n/a | unchanged default output |
| C6 | the headline items are items, are members, and each has an acceptance or gap that states a result (validation V2, V3, V5 of the script) | the script's validation is clean | the brief's decision rule ("headline items that can be met or missed") | clean | n/a | code verification |
| C7 | figures before and after: members, done, partly done, not started, per cent, the state of each headline item; the count of members done before the fan work began | stated | the brief, tasks 3 and 5 | members 80 to 110; per cent 65 to 80 | n/a | code verification |
| P | `python scripts/process_report.py --check`; `cargo test -p rcfd-cli --release --test docs_site` | no failure but a generated file not current; passes | rule 7, version 4 | as stated | n/a | code verification |

No row is of kind validation: the acceptance claims no agreement with a measured case.

## Negative controls and mutations

This work adds no code and no test, so no mutation patch is committed (rule 16 has no test file to sit beside).
Negative controls of C5, on scratch copies: (K1) with one character of the title of `aerofoil-lift` changed in the
definitions file, the comparison of C5 reports a difference; (K2) with one line `F-029<tab>vortex-shedding<tab>...`
added to the mapping with F-029 as its only member of that milestone, the comparison reports the changed items. Negative
control of C3: with one fan-titled non-member removed from the list, the placement script reports it.

## Decision rule

Done only if the definition names an item or "no item" for every link of the chain (C1), every item naming a fan in
its title is placed (C3), every item is mapped with reasons (C2), the sample check is made and reported (C4), and the
command leaves the other milestones unchanged (C5, C6). Partial if a link of the chain could not be read from the
pages or the sample check was not made. Stopped and reported if the goal cannot be given headline items that can be
met or missed.

## What is not covered

No edit of `docs/roadmap.json`, `docs/milestones.json`, `docs/ROADMAP.md`, `docs/roadmap-overview.md`, the SVGs or
`docs/adr/index.md` (rule 9): the lead runs the command. The definitions file is built from `docs/milestones.json` of
main 059c9423; if main's definitions of the other two milestones change before the lead runs the command, the file
must be rebuilt from main (the addendum gives the script). `docs/milestone-fan.md` is not edited, although its chain
table is dated; that is named for the lead. The 21 memberships of ADR 0756 of class M2 and M3 are not read again. The
notes of the items are read only around keyword matches. The second reading is of 40 pairs. The published references
are named from `docs/milestones.json` and the two pages; no paper was read. No cargo command but `docs_site`.
