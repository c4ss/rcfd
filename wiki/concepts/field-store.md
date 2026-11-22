---
type: concept
tags: [run, studio]
sources: [doc-glossary, doc-product]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Field, field store, snapshot, field view

A field is a value in every cell (`ux`, `uy`, `p`). The field store is what a run stores as it goes (`output.store_interval`, in `runs/<run id>/fields`); a snapshot is one stored state with number, step and time ([[doc-glossary]]).

## How rcfd uses it

- `start_run` with `store: plan` stores about 100 snapshots over the run, in single precision ([[doc-glossary]]).
- Field view: the Studio draws the mesh boundary as triangles coloured by the owning cell's value on viridis, min to max (ADR 0801) ([[doc-glossary]]).
- Implemented by [[rcfd-tsstore]]; the Studio reads `store.rts` locally with a Node reader ([[doc-product]]).

## Related

- [[run]]
- [[studio]]
- [[rcfd-tsstore]]
