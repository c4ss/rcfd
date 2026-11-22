---
type: entity
tags: [roadmap, process]
sources: [doc-roadmap-overview, doc-process-agent-rules, doc-glossary]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Roadmap

**What it is:** the list of all [[f-item]]s with dependencies, acceptance and status. `docs/roadmap.json` is the source of truth; `docs/ROADMAP.md` and the overview counts are generated from it ([[doc-roadmap-overview]]).
**Where:** `docs/roadmap.json`, `docs/ROADMAP.md`, `docs/roadmap-overview.md`, proposals in `docs/roadmap/proposals/`.

## Details

- Written on main only, by the lead; agents propose entries as `docs/roadmap/proposals/<ADR>-<item>.json` ([[doc-process-agent-rules]], rule 9).
- Counts (2026-11-18): 786 items, 389 done, 297 partial, 100 not started ([[doc-roadmap-overview]]).
- Lines of work (priority order of 2026-11-18): Claude and the MCP server; Studio; moving meshes, rotors and coupling; structural solver; quality, docs and validation; design studies; numerics; compressible + acoustics; multiphase + particles; turbulence; heat/materials/chemistry; parallel + GPU; meshing; post-processing; CLI/Python/API ([[doc-roadmap-overview]]).
- Per-area pages `roadmap-<area>` come in P2 ([[ingest-plan]]).

## Related

- [[roadmap-status]]
- [[simulation-milestones]]
- [[adr]]
