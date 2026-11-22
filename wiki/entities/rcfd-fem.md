---
type: entity
tags: [crate]
sources: [doc-architecture, doc-roadmap-overview]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-fem

**What it is:** Finite-element structural solver: the solid data model and structured builders. ([[doc-architecture]])
**Where:** `crates/rcfd-fem/` — layer 2 (gpu / dist / runner / fem) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-linalg]]
- Used by: [[rcfd-post]]

## Details

- Decision records: ADR 0271, ADR 0270 ([[doc-architecture]]).
- Plan ADR 0270 (approved 2026-11-12, with ADR 0290 and 0310); not validated against a measured case ([[doc-roadmap-overview]]).

## Open questions

_Filled in by P1-P3 ingests._
