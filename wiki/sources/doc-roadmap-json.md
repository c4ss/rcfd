---
type: source
tags: [roadmap]
sources: []
origin: docs/roadmap.json
origin_commit: d2cc5641
created: 2026-11-22
updated: 2026-11-25
status: stable
---

# docs/roadmap.json (+ categories in scripts/roadmap_progress.py)

**Origin:** [docs/roadmap.json](../../docs/roadmap.json) and [scripts/roadmap_progress.py](../../scripts/roadmap_progress.py) at `origin/main` 709e302e (first read at snapshot `2e94e5ef`; re-read at 5b96a1f8, 8ee32b1c, 09647baa and 930153a4)
**Kind:** machine-readable source of truth for the roadmap (about 1.9 MB)

## Summary

A JSON list of ~~786~~ ~~791 (at 5b96a1f8)~~ ~~792 (at 8ee32b1c)~~ ~~794 (at 09647baa; 795 on the merged tree d0fb2b2e, see below)~~ ~~795 (at 8732f2ae, F-886 on main with the wiki; at 930153a4 F-888 done, 404 / 299 / 92)~~ 798 (at 709e302e: 409 done, 300 partial, 89 todo) [[f-item]]s, each with `id`, `title`, `phase`, `deps`, `status` (`done` / `partial` / `todo`), `acceptance`, `gap`, `note` and `milestones`. `docs/ROADMAP.md` and the counts in `docs/roadmap-overview.md` are generated from it ([[doc-roadmap-overview]]). The file holds phases only; the 15 lines of work (categories) are an id list in `scripts/roadmap_progress.py`, in 4 families (Solver, Data, Use, Project). An item in no category, or in two, makes that script exit with an error.

## Key points

- Status counts at the snapshot: 389 done, 297 partial, 100 todo. They match the overview table of 2026-11-18, line of work by line of work.
- ~~Status counts at 5b96a1f8: 791 items, 391 done, 297 partial, 103 todo~~; at 8ee32b1c: 792 items, 391 done, 297 partial, 104 todo ([[doc-roadmap-overview]]). F-886 (todo, Quality, docs and validation) was entered on 2026-11-22 on the maintainer's answer to the wiki's citation question: sources read for the Schnerr-Sauer model (ADR 0056), the effectiveness-NTU relations (ADR 0039) and the GCI of ADR 0762, from the open sources the foundations pass found ([[cavitation-and-mass-transfer-models]], [[heat-exchanger-effectiveness-ntu]], [[gci-richardson]]). The difference: F-868 and F-869 partial to done, F-878 and F-879 todo to partial, and five new todo items F-881 to F-885.
- The per-area pages `entities/roadmap-<area>` are generated from this file by `wiki/tools/gen_roadmap_pages.py`. It rewrites only the marked generated block, so the hand-written synthesis on those pages survives a rerun.
- `note` holds the dated history entries of an item (often long). `gap` (at most 500 characters) says what is missing now and replaces the previous gap ([[doc-process-agent-rules]], rule 9). The generated pages show `gap` and leave out `note`.
- Only the lead writes this file, on main. Agents propose entries as `docs/roadmap/proposals/<ADR>-<item>.json` ([[doc-process-agent-rules]]).

## Changes from 2e94e5ef to 5b96a1f8

All on 2026-11-22, from the merges of F-868 (ADR 0764), F-878 (ADR 0765), F-879 (ADR 0766) and the maintainer's answers to ADRs 0763 to 0766 (commit 5b96a1f8 and its predecessors, `git log 2e94e5ef..origin/main`).

- **F-868** (NACA 0012 lift curve at Re 6e6, milestone `aerofoil-lift`): partial to **done**, its gap removed. The note records ADR 0764: at 17 degrees a time-accurate run of the same model on the same mesh (Spalart-Allmaras without DES on pimple, bdf2, dt 0.02 c/U to t = 200, mesh M1w-far500) "settles to a steady attached flow with cl 1.5516 over [100, 200], 0.0541 under Ladson's 1.6057 and within the unchanged band 0.0803"; nothing sheds (range 0.0012); the control at 15 degrees gives 1.4917 against the steady 1.4932. The lead first wrote it partial pending the maintainer's reading; then "Maintainer (2026-11-22): "Yes, all six", option (a) of the question of ADR 0764: the time-accurate arm meets the acceptance; done. The steady route at 17 degrees is F-884." See [[naca0012]].
- **F-869** (cylinder at Re 100, mean drag, milestone `vortex-shedding`): partial to **done**, its gap removed: "Maintainer (2026-11-22): "Yes, option (c)" to question 1 of ADR 0763: done as it stands, the order miss kept as a known miss; the diagnosis of the order is F-881." See [[cylinder-re100]].
- **F-870** (FAN-01 design point): stays partial; acceptance extended "by the maintainer's answer of 2026-11-22 to question 1 of ADR 0765": beside pressure rise and shaft power, the Euler head and the kinetic energy leaving the rotor over the measured span (220.02 and 99.20 Pa as ADR 0765 reads the record) and the circumferential-mean profiles of $c_x$ and $c_t$ at the measured plane, each within a registered band; "the pressure rise stays a compared quantity and its miss stays a miss". Note: "the convergence rule waits on F-885 (ADR 0766, question 2)".
- **F-878** (FAN-01 rotor against the measured LDA profiles and the Euler work): todo to **partial** (ADR 0765). Gap: "the division of the missing 52.47 Pa" cannot be made; the rotor's share is only bounded (-34.4 to 59.1 Pa); $c_x$ and $c_t$ each miss their band at 12 of 26 radii (worst at hub and tip). Euler work 297.66 W measured against 312.03 W simulated (shaft power 333.30 W); Euler head 220.02 against 226.86 Pa; kinetic energy leaving the rotor 99.20 against 102.28 Pa. Follow-ups F-882 and F-883. See [[doc-reference-data-f878-fan01-lda-profiles]].
- **F-879** (FAN-01 hub pocket, check H of ADR 0183): todo to **partial** (ADR 0766). Run on B2 (13 627 cells, SST, pimple with 5 outer iterations, BDF2) as H(dt) with 4.605e-5 s and H(dt/2), 0.2532 s or 6.27 revolutions each; the outer-iteration rule, read on the recorded pressure ratio, is met by 43.8 % of the window's steps of H(dt) against 99 %, so "the class is NOT EVALUABLE". Observed (not a class): the oscillation at the six site cells decays in both runs, below 1e-4 m/s in the last flow-through time. Acceptance reworded from "from the stalled steady state" to "from the start state registered there" (ADR 0766, question 3). Follow-up F-885.
- **F-880** (FAN-01 outlet as on the rig): still todo; dependencies now F-133, F-878, F-882, F-883 ("by the maintainer's answer to question 2 of ADR 0765, F-880 waits on F-882 and F-883").
- New, all todo, phase 9, entered 2026-11-22 on maintainer answers: **F-881** "Why the mean drag of the cylinder at Re 100 converges at an order near 1.4" (deps F-869, milestone `vortex-shedding`; "No solver change is made by this item"); **F-882** the loss of total pressure of the simulated FAN-01 rotor up to the measured plane (62.87 Pa on B1, 78.75 Pa on B2, 104.34 Pa on B3), divided by where it arises and recomputed on a mesh finer than B1 (deps F-878); **F-883** a mean static pressure behind the rotor read from the wall-pressure data of the FAN-01 record (deps F-878); **F-884** "NACA 0012 at 17 degrees: the steady solver reaches the attached state that the time-accurate run finds" (deps F-868, milestone `aerofoil-lift`); **F-885** a read-only, opt-in record of the velocity change of each outer iteration of the pimple step, with defaults bit-identical, and check H read by ADR 0183's own outer-iteration rule (deps F-879, F-286).

## Changes from 5b96a1f8 to 09647baa

From the merges of 2026-11-22 and 2026-11-23 (ADRs 0767 to 0785) and the maintainer's answers recorded with them (`git log 5b96a1f8..09647baa`). Counts at 09647baa: 794 items, 403 done, 299 partial, 92 todo ([[doc-roadmap-overview]]). 09647baa is the lead's main and does not hold F-886, which was entered at 8ee32b1c on the maintainer's checkout; the tree that merges the two (d0fb2b2e, on which the `entities/roadmap-*` pages were regenerated) has 795 items, 403 done, 299 partial and 93 todo.

- **Done at the merge:** F-883 (ADR 0769), F-884 (ADR 0770), F-881 (ADR 0771), F-887 (ADR 0772), F-876 (ADR 0773), F-862 (ADR 0774), F-871 (ADR 0775), F-860 (ADR 0776), F-850 (ADR 0778), F-875 (ADR 0779), F-616 (ADR 0785). **F-856** was written partial at the merge and done by the lead's reading once the Rust 1.90 type-check was green in CI at 47ade6bf (ADR 0782).
- **Partial:** F-885 (ADR 0767), F-882 (ADR 0768). **Unchanged todo:** F-842 (ADR 0780, registered and stopped before code).
- **F-870** stays partial; its acceptance gains the casing pressure behind the rotor as a compared quantity ("Maintainer (2026-11-22): the casing pressure behind the rotor is a compared quantity (ADR 0769)").
- **New items, all todo:** **F-887** "Steady Spalart-Allmaras without eddy-viscosity relaxation over the NACA 0012 lift curve" (deps F-884, milestone `aerofoil-lift`; entered 2026-11-22 on the maintainer's answer to ADR 0770, done the next day by ADR 0772); **F-888** "Studio: the end-to-end test of the quick run by keyboard fails where the Studio window lacks the system's focus" (deps F-807; entered 2026-11-23 on the maintainer's answer "Yes, one over", one agent beyond the limit of 4 for this item alone, issued by the brief `f888-studio-quick-run-focus.md`, ADR 0786); **F-889** "Case-file keys for the cutoff, the load's residual vector and the load patterns of the reduction" (deps F-616; entered 2026-11-23 on the maintainer's answer "Yes to both F-616 questions", not issued).
- At 709e302e three of the four items of the next bullet are merged and done (F-861, F-852, F-451; see the section below); F-339 (ADR 0784) is still todo, and a reader was briefed for it before its merge, "the item changing two goldens by an additive edit" (commit 709e302e).
- **Issued on 2026-11-23, registrations not on main at 09647baa:** F-861 (process-report convention checks, brief `f861-process-report-convention-checks.md`, ADR 0777), F-852 (Studio "Close session cannot be undone" time-out, brief `f852-studio-close-session-timeout.md`, ADR 0781), F-451 (`rcfd duct-modes`, brief `f451-duct-modes-command.md`, ADR 0783), F-339 (MCP tools for tolerance and Sobol studies, brief `f339-mcp-uq-sobol-tools.md`, ADR 0784). All four are todo; the note of F-888 says its test "holds the merges of F-861, F-852, F-451 and F-339".
- **Milestone memberships:** 81 memberships of 24 items written for the six milestones of ADR 0775 and 39 for `fan-structure` of ADR 0779 ([[doc-milestones-json]]); F-881, F-884 and F-887 carry `vortex-shedding` and `aerofoil-lift`.
- F-880 (FAN-01 outlet as on the rig) is still todo and not issued at 09647baa; whether it is issued is question 1 of ADR 0768 ([[adr-thread-fan01-design-point]]).

## Changes from 930153a4 to 709e302e

From the merges of 2026-11-23 and 2026-11-24 (ADRs 0777, 0781, 0783, 0787, 0788, 0789; `git log 930153a4..709e302e`). Counts at 709e302e: 798 items, 409 done, 300 partial, 89 todo, 70 % counting a partial item as half ([[doc-roadmap-overview]]). Seven items changed and nothing else.

- **New items, entered 2026-11-23** "on the maintainer's answer of that day (\"Yes, issue all three\") with the title, dependencies and acceptance" of the follow-ups of `docs/roadmap/proposals/0786-F-888.json` "as written", and issued the same day: **F-890** "Studio: responsiveness.spec.ts:260 fails on the lead's machine with one task of 400 to 440 ms when the workspace tab opens" (deps F-832; ADR 0787; **partial**), **F-891** "Studio: f837.spec.ts:198 fails in 2 of 6 whole runs, at two different lines" (deps F-837; ADR 0788; **done**), **F-892** "Studio: name the test that leaves an rcfd serve process running after npm test on Windows" (deps F-888; ADR 0789; **done**). The note of F-888 gains the entry that names them. See [[adr-thread-studio-electron]].
- **F-890's gap:** "The cause is shown as a mechanism and fixed, and responsiveness.spec.ts:260 passed 10 of 10 runs alone and 3 of 3 whole runs. Missing: a record that this mechanism was the task of 400 to 440 ms. It needs the machine under 0.75 busy: the unchanged test on the tree before ADR 0787's fix must show the task again, and on the tree with it must pass. In the whole runs undo.spec.ts:268 and run-control.spec.ts:320 each failed once in three, outside this item."
- **F-861** (the process report checks the rules that rest on convention; ADR 0777): todo to **done**. "Lead (2026-11-23): merged again on main at f8e433bb after the Studio fixes of F-890 to F-892 and pushed at 1e6dcaa0; npm test passed all 873 tests in the first run (the three earlier push runs of the first merge had failed on Studio tests outside this item). Done as proposed. 734 binaries left out for a left-out run." See [[adr-thread-process-measures-and-roadmap-form]].
- **F-852** (Studio: "Close session cannot be undone" times out on Windows in CI; ADR 0781) and **F-451** (`rcfd duct-modes`; ADR 0783): todo to **done**, merged "as one joint check (merges 4255f915, 01726464) and pushed at 9c1d3817, after the Studio fixes of F-890 to F-892; the merge also regenerates the Studio's capability inventory for rcfd duct-modes and maps it in coverage.json as the other acoustics tools (F-809, unplanned)". See [[adr-thread-acoustic-wave-solver]].
- **Follow-ups proposed and not entered**, each the maintainer's (scope): the undo key pressed while a step is busy (F-852), a second Write after a refusal (F-891), `data.open`'s length unit and `run-control.spec.ts:320` (F-892).
- Lines of work that changed: Studio desktop app 44 / 13 / 5 (was 41 / 12 / 6), Quality, docs and validation 30 / 22 / 15 (was 29 / 22 / 16), Compressible flow and acoustics 10 / 8 / 8 (was 9 / 8 / 9) ([[doc-roadmap-overview]]).

## Contradictions with the wiki

_None found at ingest._ None at the 5b96a1f8 update either. At 09647baa: the item counts of the two lines (792 at 8ee32b1c with F-886, 794 at 09647baa without it) are two branches, not a conflict; the merged tree has 795.

## Changes at 975beadb (F-339)

F-339 todo to partial: 798 items, 409 done, 301 partial, 88 not started, about 70 %; Design studies and optimisation 20 / 17 / 7; the milestone Assistant-driven fan design 92 items, 49 / 32 / 11, 71 %, not met.

## Changes at 2514444c (F-893)

F-893 entered (2026-11-24, the maintainer's answer) and done: 799 items, 410 done, 301 partial, 88 not started, about 70 % ([[adr-thread-studio-electron]]).

## Changes at a1fdeb8c (F-339, ADR 0791)

F-339 stays partial; its gap is K1 of ADR 0791. Counts unchanged: 799 items, 410 done, 301 partial, 88 not started ([[adr-thread-mcp-tools]]).

## Changes at d2cc5641 (F-894, F-339)

F-894 entered and done, F-339 partial to done: 800 items, 412 done, 300 partial, 88 not started, about 70 % ([[adr-thread-mcp-tools]]).

## Pages this source informs

- [[roadmap]]
- [[roadmap-numerics-and-solvers]], [[roadmap-turbulence]], [[roadmap-heat-materials-and-chemistry]], [[roadmap-compressible-flow-and-acoustics]], [[roadmap-multiphase-and-particles]], [[roadmap-moving-meshes-rotors-and-coupling]], [[roadmap-structural-solver]], [[roadmap-parallel-and-gpu]], [[roadmap-meshing-and-mesh-formats]], [[roadmap-post-processing-and-output]], [[roadmap-design-studies-and-optimisation]], [[roadmap-command-line-python-and-api]], [[roadmap-claude-and-the-mcp-server]], [[roadmap-studio-desktop-app]], [[roadmap-quality-docs-and-validation]]
