---
type: entity
tags: [crate]
sources: [doc-architecture, adr-thread-amg-coarse-lu]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-linalg

**What it is:** Linear algebra and linear solvers. (The architecture page gives no description; to confirm in P1/P3.) ([[doc-architecture]])
**Where:** `crates/rcfd-linalg/` — layer 1 (mesh / linalg / project) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]]
- Used by: [[rcfd-gpu]], [[rcfd-dist]], [[rcfd-fem]], [[rcfd-fv]]

## Details

- **Scalar AMG** (`rcfd_linalg::amg`, smoothed aggregation) and `amg_elastic.rs`; the coarsest dense LU applies row interchanges before the forward substitution by default since ADR 0743 (F-471); `AmgSettings::coarse_lu_interchanges_first` ([[adr-thread-amg-coarse-lu]], [[amg]]).

## Open questions

_Filled in by P1-P3 ingests._
