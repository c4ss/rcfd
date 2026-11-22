---
type: entity
tags: [crate]
sources: [doc-architecture]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-fv

**What it is:** The finite-volume core. (The architecture page gives no description; ADR 0115 places the gradient limiters in `rcfd_fv::gradient`.) ([[doc-architecture]])
**Where:** `crates/rcfd-fv/` — layer 3 (io / fv / particles) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-linalg]], [[rcfd-gpu]]
- Used by: [[rcfd-turb]], [[rcfd-post]], [[rcfd-api]], [[rcfd-cli]]

## Open questions

_Filled in by P1-P3 ingests._
