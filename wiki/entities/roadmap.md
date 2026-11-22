---
type: entity
tags: [roadmap, process]
sources: [doc-roadmap-overview, doc-roadmap-json, doc-process-agent-rules, doc-glossary, doc-process-lead-process, doc-process-documentation-review, doc-process-framework-reconciliation, doc-audit-report, doc-partials-audit]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Roadmap

**What it is:** the list of all [[f-item]]s with dependencies, acceptance and status. `docs/roadmap.json` is the source of truth; `docs/ROADMAP.md` and the overview counts are generated from it ([[doc-roadmap-overview]]).
**Where:** `docs/roadmap.json`, `docs/ROADMAP.md`, `docs/roadmap-overview.md`, proposals in `docs/roadmap/proposals/`.

## Details

- Written on main only, by the lead; agents propose entries as `docs/roadmap/proposals/<ADR>-<item>.json` ([[doc-process-agent-rules]], rule 9).
- Counts (2026-11-18): 786 items, 389 done, 297 partial, 100 not started ([[doc-roadmap-overview]]). At origin/main 5b96a1f8 (2026-11-22): 791 items, 391 done, 297 partial, 103 not started ([[doc-roadmap-overview]], [[doc-roadmap-json]]).
- Lines of work (priority order of 2026-11-18): Claude and the MCP server; Studio; moving meshes, rotors and coupling; structural solver; quality, docs and validation; design studies; numerics; compressible + acoustics; multiphase + particles; turbulence; heat/materials/chemistry; parallel + GPU; meshing; post-processing; CLI/Python/API ([[doc-roadmap-overview]]).
- Per-area pages `roadmap-<area>` come in P2 ([[ingest-plan]]).
- The **priority statement** at the head of `docs/ROADMAP.md` (above its marker line) is the maintainer's acceptance: dated, restated at each new instruction, naming the lines of work in order with a weekly budget in registrations ([[doc-process-lead-process]], [[maintainer]]).
- The [[lead]] writes an item's status in `docs/roadmap.json` in a commit of its own after the push script's commit, and applies agents' proposal files with `scripts/roadmap_proposals.py apply` without rewording them (the maintainer, 2026-11-21) ([[doc-process-lead-process]]).
- A partial item carries a `gap` key (at most 500 characters, what is missing now); all 319 partial items got one on 2026-11-18, drafted by a model from acceptance and note alone, unchecked ([[doc-process-documentation-review]]).
- History: ROADMAP.md was hand-kept beside roadmap.json until 2026-11-18, when the revived `scripts/render_roadmap.py` made its item list generated (48 disagreements reconciled, the duplicate F-847 removed) ([[doc-process-documentation-review]]). The audit of 2026-10-14 had already found notes of 33 items drifted and recommended generating the markdown ([[doc-audit-report]]; unreviewed model output).
- Counts over time: 114 items on 2026-10-14 ([[doc-audit-report]]); 754 on 2026-11-17 and 768 on 2026-11-21 with no duplicate id and no missing dependency ([[doc-process-framework-reconciliation]]).
- Pilot (F-864): an item-owned roadmap in shadow mode; `docs/roadmap.json` stays the one source until a cutover the maintainer decides ([[doc-process-framework-reconciliation]]).
- The 2026-10-21 partials audit graded 100 partial items against their acceptance text: 35 DONE, 64 PARTIAL, 1 OVERSTATED ([[doc-partials-audit]]).

## Related

- [[roadmap-status]]
- [[simulation-milestones]]
- [[adr]]
- [[lead]]
- [[process-report]]
