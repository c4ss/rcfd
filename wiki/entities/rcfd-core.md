---
type: entity
tags: [crate]
sources: [doc-architecture, adr-thread-founding-decisions]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-core

**What it is:** Base crate with no rcfd dependencies: scalar type, units, errors, logging, the settings tree, hashing. ([[doc-architecture]])
**Where:** `crates/rcfd-core/` — layer 0 (core) of the [[crate-graph]].

## Dependencies

- Depends on: no workspace crate
- Used by: [[rcfd-mesh]], [[rcfd-linalg]], [[rcfd-project]], [[rcfd-dist]], [[rcfd-fem]], [[rcfd-io]], [[rcfd-fv]], [[rcfd-particles]], [[rcfd-turb]], [[rcfd-post]], [[rcfd-tsstore]], [[rcfd-api]], [[rcfd-cli]]

## Founding ADRs (added by p3-records-a)

- ADR 0001: the settings tree `rcfd_core::settings::Schema` is "a schema plus data"; the data is a `serde_json::Value` "*always* validated against the schema on `set`", dimensioned floats are stored in SI, access is by dotted path, JSON Schema export comes from the same schema, and "The content hash is FNV-1a over key-sorted JSON" ([[adr-thread-founding-decisions]]).
- ADR 0002: scalar kernels that must support f32 are generic over `rcfd_core::Scalar`, solver state defaults to `Real = f64`; fields are flat structure-of-arrays vectors ([[adr-thread-founding-decisions]]).
- ADR 0003: deterministic chunking and a fixed combine order make reductions bit-reproducible for a given thread count and chunk size ([[adr-thread-founding-decisions]]). See [[bit-identical]].

## Open questions

_Filled in by P1-P3 ingests._
