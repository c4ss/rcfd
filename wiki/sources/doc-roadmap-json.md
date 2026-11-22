---
type: source
tags: [roadmap]
sources: []
origin: docs/roadmap.json
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# docs/roadmap.json (+ categories in scripts/roadmap_progress.py)

**Origin:** [docs/roadmap.json](../../docs/roadmap.json) and [scripts/roadmap_progress.py](../../scripts/roadmap_progress.py) at snapshot `2e94e5ef`
**Kind:** machine-readable source of truth for the roadmap (about 1.9 MB)

## Summary

A JSON list of 786 [[f-item]]s, each with `id`, `title`, `phase`, `deps`, `status` (`done` / `partial` / `todo`), `acceptance`, `gap`, `note` and `milestones`. `docs/ROADMAP.md` and the counts in `docs/roadmap-overview.md` are generated from it ([[doc-roadmap-overview]]). The file holds phases only; the 15 lines of work (categories) are an id list in `scripts/roadmap_progress.py`, in 4 families (Solver, Data, Use, Project). An item in no category, or in two, makes that script exit with an error.

## Key points

- Status counts at the snapshot: 389 done, 297 partial, 100 todo. They match the overview table of 2026-11-18, line of work by line of work.
- The per-area pages `entities/roadmap-<area>` are generated from this file by `wiki/tools/gen_roadmap_pages.py`. It rewrites only the marked generated block, so the hand-written synthesis on those pages survives a rerun.
- `note` holds the dated history entries of an item (often long). `gap` (at most 500 characters) says what is missing now and replaces the previous gap ([[doc-process-agent-rules]], rule 9). The generated pages show `gap` and leave out `note`.
- Only the lead writes this file, on main. Agents propose entries as `docs/roadmap/proposals/<ADR>-<item>.json` ([[doc-process-agent-rules]]).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[roadmap]]
- [[roadmap-numerics-and-solvers]], [[roadmap-turbulence]], [[roadmap-heat-materials-and-chemistry]], [[roadmap-compressible-flow-and-acoustics]], [[roadmap-multiphase-and-particles]], [[roadmap-moving-meshes-rotors-and-coupling]], [[roadmap-structural-solver]], [[roadmap-parallel-and-gpu]], [[roadmap-meshing-and-mesh-formats]], [[roadmap-post-processing-and-output]], [[roadmap-design-studies-and-optimisation]], [[roadmap-command-line-python-and-api]], [[roadmap-claude-and-the-mcp-server]], [[roadmap-studio-desktop-app]], [[roadmap-quality-docs-and-validation]]
