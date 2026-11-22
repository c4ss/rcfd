---
type: source
tags: [roadmap]
sources: []
origin: docs/roadmap.json
origin_commit: 5b96a1f8
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# docs/roadmap.json (+ categories in scripts/roadmap_progress.py)

**Origin:** [docs/roadmap.json](../../docs/roadmap.json) and [scripts/roadmap_progress.py](../../scripts/roadmap_progress.py) at `origin/main` 5b96a1f8 (first read at snapshot `2e94e5ef`)
**Kind:** machine-readable source of truth for the roadmap (about 1.9 MB)

## Summary

A JSON list of ~~786~~ 791 (at 5b96a1f8) [[f-item]]s, each with `id`, `title`, `phase`, `deps`, `status` (`done` / `partial` / `todo`), `acceptance`, `gap`, `note` and `milestones`. `docs/ROADMAP.md` and the counts in `docs/roadmap-overview.md` are generated from it ([[doc-roadmap-overview]]). The file holds phases only; the 15 lines of work (categories) are an id list in `scripts/roadmap_progress.py`, in 4 families (Solver, Data, Use, Project). An item in no category, or in two, makes that script exit with an error.

## Key points

- Status counts at the snapshot: 389 done, 297 partial, 100 todo. They match the overview table of 2026-11-18, line of work by line of work.
- Status counts at 5b96a1f8: 791 items, 391 done, 297 partial, 103 todo ([[doc-roadmap-overview]]). The difference: F-868 and F-869 partial to done, F-878 and F-879 todo to partial, and five new todo items F-881 to F-885.
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

## Contradictions with the wiki

_None found at ingest._ None at the 5b96a1f8 update either.

## Pages this source informs

- [[roadmap]]
- [[roadmap-numerics-and-solvers]], [[roadmap-turbulence]], [[roadmap-heat-materials-and-chemistry]], [[roadmap-compressible-flow-and-acoustics]], [[roadmap-multiphase-and-particles]], [[roadmap-moving-meshes-rotors-and-coupling]], [[roadmap-structural-solver]], [[roadmap-parallel-and-gpu]], [[roadmap-meshing-and-mesh-formats]], [[roadmap-post-processing-and-output]], [[roadmap-design-studies-and-optimisation]], [[roadmap-command-line-python-and-api]], [[roadmap-claude-and-the-mcp-server]], [[roadmap-studio-desktop-app]], [[roadmap-quality-docs-and-validation]]
