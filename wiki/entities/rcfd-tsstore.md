---
type: entity
tags: [crate]
sources: [doc-architecture, doc-ui-restart-plan, doc-studio-guardrails]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-tsstore

**What it is:** Studio layer L3, the time-series field store of a run (playback of stored fields). ([[doc-architecture]])
**Where:** `crates/rcfd-tsstore/` — layer 4 (turb / post / tsstore) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-io]]
- Used by: [[rcfd-api]], [[rcfd-workspace]], [[rcfd-mcp]]

## Details

- Decision records: ADR 0124 ([[doc-architecture]]).
- Planned as wave 47C, layer "L3 Time-series store: mesh stored once, field snapshots per saved time (chunked, compressed, memory-mappable, indexed), plus residual and monitor series; random access for scrubbing; reader and writer in the solver path", tested by round-trip and bit-exactness tests ([[doc-ui-restart-plan]]). In the project folder it is `runs/<run-id>/fields/` (index.json, mesh once, one snapshot per saved time) ([[doc-ui-restart-plan]]).
- The Studio reads stored fields through a TypeScript reader of `store.rts` (R2, ADR 0350 addendum 1); guardrail G3 requires every field value the renderer gets to equal the Rust reader's value bit for bit, kept by a test ([[doc-studio-guardrails]]).

## Open questions

_Filled in by P1-P3 ingests._
