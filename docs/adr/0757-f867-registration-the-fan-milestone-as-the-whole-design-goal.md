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

## Addendum 1: results (2026-11-21)

Commits: fc9114a0 (this registration, alone, on main 059c9423), ef2900d4 (the definition and the mapping, first reading),
c281ac05 (one correction of the mapping), and the commit of this addendum with the proposal file. No amendment was
needed: the lead's message about F-870 came before the registration's commit and is in its head.

| Id | Measured | Met |
| --- | --- | --- |
| C1 | 14 of 14 links: the table "The capability chain and the items that carry each link" stands as registered; "no item" is said for the headline of L5 and for the geometry checker (L1), the steady guard and the stall diagnoses (L9) and the reduced-order model (B1) | yes |
| C2 | 777 of 777 items read (308 structural by title); 89 memberships, each with a reason that starts M1, M2, M3 or M5 (the build script asserts it); 688 items are not members | yes |
| C3 | 21 titles match; 12 members, 9 listed below; 0 unplaced. Control: with F-819 taken from the list the script names F-819 | yes |
| C4 | 40 pairs by seed 867757, second reading by a fresh agent: 1 disagreement (below) | yes |
| C5 | on a copy of the `docs/` of main 059c9423 (`proof.py`, scratch): the definitions of `vortex-shedding` and `aerofoil-lift` equal in bytes, and the whole file outside the fan object; 0 of 777 items with a changed list of other milestones; 66 items change their fan membership and differ by the key alone; 711 items equal; the table row and the three headline lines of each of the other two milestones in the generated block equal; a second run writes nothing; `roadmap_milestones.py --check` 0; `render_roadmap.py --check` 0 after a render. K1 (one character of the aerofoil title): three differences reported. K2: see deviation 2 | yes |
| C6 | the script's validation is clean with the headline F-870, F-243, F-260, F-702; each is a member | yes |
| C7 | below | yes |
| P | `python scripts/process_report.py --check`: 1 failure, "generated file is not current: docs/adr/index.md", left to the lead (rule 9). `cargo test -p rcfd-cli --release --test docs_site`: 2 passed | yes |

**Figures** on the roadmap of main 059c9423 (777 items, 387 done, 294 partly done, 96 not started, 69 %).

| Milestone fan | Members | Done | Partly done | Not started | Per cent | Headline result |
| --- | --- | --- | --- | --- | --- | --- |
| at 1a5bbac5 (ADR 0756) | 22 | 17 | 5 | 0 | 89 % | not met (F-126 partly met) |
| at 059c9423, before this work | 23 | 17 | 5 | 1 | 85 % | not met (F-126 partly met, F-870 not started) |
| first reading of this work | 88 | 49 | 29 | 10 | 72 % | not met |
| with the correction (delivered) | 89 | 49 | 30 | 10 | 72 % | not met |

Headline items: F-870 not started; F-243 partly met; F-260 partly met; F-702 not started. By class: M1 4 (2 partly done,
2 not started); M2 23 (19 done, 4 partly done); M3 1 (partly done); M4 none; M5 61 (30 done, 23 partly done, 8 not
started). F-126 is a member by M5 and no longer a headline item, as registered.

**The fan-titled items that are not members** (9 of 21): F-217, N4 (a heat-exchanger model whose fan boundary takes a
given curve; no link names it). F-387, F-406, F-436, F-593, F-604, N6 (i) (structural demonstrations and their blade
mesher and hub fit; the page leaves structural coupling to the user; open question 5). F-819, N1 (the Studio draws a
fan mesh as a large-mesh test). F-865, F-867, N3 (the process items about this figure).

**What was left out of what the brief names**, by N6 as registered: the structural checks of blade and rotor; the
sliding-interface and full-annulus route (F-250, F-303, F-304, F-704, F-710 to F-712, F-720, F-721, F-724, F-725, F-729,
F-730 and F-138); transition (F-046); degradation. Of the Studio only F-814 is a member; of the assistant line the chat
sidebar, the providers and the project bookkeeping are N3 (F-092, F-093, F-145, F-146, F-149, F-150, F-265, F-270, F-273,
F-275, F-862). The acoustic boundary, sponge and duct wave-solver follow-ups of F-243 (F-330 to F-333, F-340, F-341,
F-407, F-446, F-451 to F-453, F-473, F-478, F-480) are N4: the noise link names FW-H, proxies and tones, not duct
propagation. Borderline and left out: F-082, F-109, F-121 (no text names their use by an added run of record), F-006,
F-108, F-233, F-248, F-295.

**The sample check.** Drawn by `sample.py` (scratch) with `random.Random(867757)` from the mapping of ef2900d4: 88
members, 689 non-members, 288 of them in the doubtful lines. The reader was a fresh agent given `rule.md` (the sections
"The definition" and "The membership rule" of this ADR and the names of the links, without the table of carriers),
copies of the two pages and the sheet, with the repository's code and topic pages to consult; it reports that it opened
none of the files it was told not to open. Disagreements: 0 of 20 in the member half; 1 of 20 in the non-member half
(0 of the 10 uniform, 1 of the 10 doubtful).

| Pair | First reading | Second reading | Taken |
| --- | --- | --- | --- |
| F-200, fan | not (N4: a catalogue of boundary conditions, no use read) | member, M2 (close): `fan01_diagnosis_w35c.rs`, arm C3, runs FAN-01 with `radial_equilibrium_outlet` | second; added. Checked: line 427 of that file sets `bc_kind = "radial_equilibrium_outlet"`, the note of F-200 lists that condition, and the file is one of the five FAN-01 runs of record (the precedent of F-106 in ADR 0756) |

The reader's table held 39 rows; F-095 was in its text as a member by M5, which agrees. It named as hard: F-119 (a
member only because the breakdown names it; no run uses STL), F-791 and F-315 (computer-fan work against N6 (iv)),
F-715, F-297 and F-735 (follow-ups checked on test functions, members by M5 (c) alone), F-287 and F-329. It found the
rule unclear in six places: no criterion between N6 and N5 for structural items (membership not affected); no
precedence between M2 and M5; no depth limit of M5 (c), which makes the tails of B3 and B5 long; the labels B1 to B5
of this ADR collide with the breakdown's part letters B1 to B5; N6 (iv) against M5 for computer-fan geometry and
mesh; and the five FAN-01 files were not named in its folder. What the check shows: the M2 class carried over from
ADR 0756 is the part of the mapping most likely to miss a member, because it was not read again (F-200 was found by
the one sampled item of that kind); the M5 members were confirmed, 15 of 15.

**What the figure will and will not say** (for the maintainer). Of the 89 members, 24 are foundation items of class M2
and M3, general solver, mesh and command-line work that the fan runs use (19 done, 5 partly done; by class, since the
dates on which items became done were not read); the other 65 are items of the goal itself, of which 30 are done and 35
are open, and in all 40 members are open (30 partly done, 10 not started). The per cent now moves with the open fan
work, by a little over half a point for each step of one item, where before all of it sat in one headline item; it
still does not say whether a fan is predicted or designed: most of the goal's 30 done items are done on test functions, the
reduced-order model or channel cases, not on a fan solve, and the four headline items, none of them done, are what says "not met".

**The command the lead runs on main**, then the usual regeneration (`python scripts/render_roadmap.py`; the block of
the overview is written by the command):

`python scripts/roadmap_milestones.py --apply docs/adr/0757-f867-mapping.tsv --definitions docs/adr/0757-f867-milestones.json`

The definitions file is `docs/milestones.json` of main 059c9423 with the object of `fan` replaced and every other byte
kept. If main's file changes before the command is run, the file is rebuilt the same way: the fan object of
`docs/adr/0757-f867-milestones.json` put in place of the fan object of main's file. No `--install` is needed.

**Deviations.** (1) The branch was moved from main 1a5bbac5 to main 059c9423 before the registration's commit, on the
lead's message; F-870 is the validation headline and F-126 a member. (2) K2 as registered could not reach a
comparison: with F-029 as the only line for `vortex-shedding` the script refuses to write ("the headline item F-869 of
'vortex-shedding' is not a member of it"), which is the protection wanted; the comparison's sight of a changed item is
shown by K1 and by the 66 changed items it counts. (3) Read in addition to the registration's list: the acceptance, gap and
the note around one match of F-200, for the sample check. (4) `docs/milestone-fan.md` was not edited: its
chain table of 2026-10-18 names F-254 and F-257, which are not those items, and says "none" for the operating map
and the optimiser, which F-095, F-249 and F-329 have since delivered on channel cases; that is for the lead or a
documentation job. (5) The proposal file `docs/roadmap/proposals/0757-F-867.json` of rules version 5 was written and
checked, though the brief was issued under version 4.

Not run: any cargo command but `docs_site`; any solver. Not read: see "What was read".

Actual size: done; 0; 0; 5; about 330000.

Decision by the registered rule: **done**. Every link has an item or "no item"; every fan-titled item is placed; every
item is mapped with reasons; the sample check was made and is reported; the command leaves the other two milestones
unchanged. The goal has four headline items that can be met or missed.

## Open questions for the maintainer

1. **The operating map has no headline item.** No item's acceptance is the characteristic curve of a fan. Options: (a)
   leave the link without a headline; (b) enter the first follow-up below as a headline item. Recommendation: (b).
2. **The noise headline.** The acceptance of F-243 is an impedance tube and a quiet boundary; the fan result is only in
   its gap. Options: (a) keep F-243; (b) enter the second follow-up below as the headline and keep F-243 as a member.
   Recommendation: (b); it needs a measured fan noise reference that the repository does not record (breakdown D5),
   so it would stay partial until one is read.
3. **The assistant-driven iteration itself.** F-702 is the tools; no item is the loop run on a fan (wave 34 of the
   page). Options: (a) keep F-702 alone; (b) enter the third follow-up below as a headline beside it. Recommendation: (b).
4. **Done work of the chain under no item**: the parametric geometry and its checker and perturbation operators (ADR
   0065, ADR 0118), the reduced-order fan model (ADR 0119), the steady divergence guard (ADR 0077) and the stall
   diagnoses of the page. Options: (a) leave them to the page; (b) enter record items so that the figure counts them.
   Recommendation: (a): record items would raise the per cent without moving the goal.
5. **Structure and vibration.** The page leaves structural coupling as an open decision of the user; the breakdown's
   target names vibration, and five fan-titled structural items exist (F-387 done; F-593, F-604 partly done; F-406,
   F-436 not started). Options: (a) outside the milestone, as mapped; (b) the five as members by a class of their own
   (94 members, 50 done, 32 partly done, 12 not started, 70 %); (c) a milestone of its own for the structural fan
   demonstrations, with F-406 or F-436 as headline. Recommendation: (c), with the cyclic rotor that ADR 0756 proposed.
   The sliding-interface route stays with the rotor-stator milestone ADR 0756 proposed.

## For the lead: roadmap entries

> Status proposed for F-867: done.

> Note entry: ADR 0757, 2026-11-21, done: the milestone fan redefined as the design goal of docs/milestone-fan.md ("Assistant-driven fan design"), headline F-870, F-243, F-260 and F-702, with an item or the words "no item" for each of 14 links; 89 memberships with reasons in docs/adr/0757-f867-mapping.tsv (49 done, 30 partly done, 10 not started, 72 %, not met; before: 23 members, 85 %), all 21 fan-titled items placed (12 members, 9 listed), sample of 40 pairs read a second time with 1 disagreement, taken (F-200 added); the command of addendum 1 leaves vortex-shedding and aerofoil-lift byte for byte, shown on a copy of main 059c9423.

> Follow-up (proposed, open question 1): "FAN-01 characteristic curve: pressure rise and efficiency over the measured flow range by an operating map", deps F-870, F-095; acceptance: Every limit not stated here is registered in the ADR before the run. An operating map (rcfd map or design::opmap) of the FAN-01 rotor over at least five flow rates of the measured characteristic recorded in docs/validation-data.md (1. FAN-01), each point restarted from its neighbour and converged by the registered rule, gives the total-to-static pressure rise and the efficiency at each point within a band registered before the run; a point that stalls or diverges is recorded with its located reason and is not a pass. Milestone fan, headline for the link operating map.

> Follow-up (proposed, open question 2): "Fan noise estimate on the operating curve: blade-passing tones and a stated broadband proxy", deps F-243, F-136, new:fan-curve; acceptance: Every limit not stated here is registered in the ADR before the run. For at least three points of the FAN-01 operating map the tonal level at the first three blade-passing harmonics and one stated steady-RANS broadband proxy are reported from the flow solution, each with its distance from a measured fan noise reference that a page of the repository records, or, where no such reference is recorded, with the words that it is not validated and the item stays partial. Milestone fan, headline for the link noise.

> Follow-up (proposed, open question 3): "One assistant-driven design iteration on a fan through the MCP tools, with its measured limitations", deps F-702, F-260; acceptance: Every limit not stated here is registered in the ADR before the run. A recorded session in which an assistant, through the MCP tools alone, creates at least three variants of a parametric fan, runs each over at least three operating points, fetches flow, pressure rise and the noise estimate, compares them and proposes the next variant, with every evaluation in the design ledger and the run reproducible from the ledger within a registered tolerance; the tier of each evaluation (reduced-order or CFD) and what was not validated are stated. Milestone fan, headline for the link assistant tools.

The same entries are in `docs/roadmap/proposals/0757-F-867.json` (form 1 of ADR 0755, base 059c9423), checked with
`python scripts/roadmap_proposals.py check`.

## The lead's reading (2026-11-21), after the merge

Merged at e66e3cc3 and pushed at fc1f032f (ledger line of e66e3cc3: the report read one minute after it came, 12 minutes
of reading, proposed done, written done). Checked by the lead before the merge: the registration is the first commit and
alone; the definitions file equals main's for `vortex-shedding` and `aerofoil-lift`; the mapping has 89 rows, 89 distinct
items, a reason in each. The command of the addendum was run on main after the merge: 89 members, 49 done, 30 partly
done, 10 not started, 72 per cent, headline not met; `python scripts/roadmap_milestones.py --check` passes.

Status: **done**, as proposed. The first live use of form 1 (ADR 0755): `docs/roadmap/proposals/0757-F-867.json` was
applied with `python scripts/roadmap_proposals.py apply`, status and note as the agent wrote them, from a copy without
the three follow-ups. The follow-ups are new roadmap items, and entering them is a question of scope: they are questions
1 to 3 above and are put to the maintainer with the agent's recommendations, together with questions 4 and 5. That a
proposal's follow-ups can wait on the maintainer while its status is applied is a third point for the form, beside the
two of ADR 0755.

Not done by this item and noted for the next reader: the 21 memberships of the classes M2 and M3 were carried over from
ADR 0756 and not read again (the sample check found one missing member of that kind, F-200); the chain table of
`docs/milestone-fan.md` names F-254 and F-257 and is stale.
