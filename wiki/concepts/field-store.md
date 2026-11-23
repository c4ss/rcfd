---
type: concept
tags: [run, studio]
sources: [doc-glossary, doc-product, doc-run-control, doc-case-schema-statistics-output, doc-mcp-tools, doc-service-protocol, doc-tutorial-desktop-assistant, adr-thread-workspace-layer, adr-thread-studio-electron]
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
- Off by default (ADR 0124). Keys `output.store_interval` (steps), `output.store_time_interval` (simulated seconds, transient only; does not change the steps), `output.store_fields` (default `ux uy uz p`), `output.store_precision` `"f64"` (default, bit-exact) or `"f32"` (ADR 0133, error at most $\lvert x\rvert\, 2^{-24} + 2^{-150}$) ([[doc-run-control]], [[doc-case-schema-statistics-output]]).
- The store keeps the mesh once (with its hash) and each snapshot field by field in checksummed blocks; "A run that is killed leaves every earlier snapshot readable"; a continued run appends ([[doc-run-control]]).
- The start plan: a store key counts as the case's own only when it differs from its default (ADR 0152); `start_run` applies the plan by default, also to an existing case (ADR 0741); `store: "none"` stores only what the case asks for ([[doc-run-control]], [[doc-mcp-tools]]).
- Bulk read over the service: `data.open` / `data.field` / `data.surface` hand values as raw little-endian bytes (opt-in `rcfd-data/1`, ADR 0350) ([[doc-service-protocol]]); `mesh.preview` writes a store with no snapshot ([[doc-service-protocol]]).

> [!warning] Contradiction (within the repo docs)
> [[doc-run-control]] says the Studio's Run, Queue… and Run the example call `start_run` with `store: "plan"`; the desktop tutorial says "A run started this way stores the fields the case asks for, and no others" ([[doc-tutorial-desktop-assistant]]).

### From the P3 ADR threads (2026-11-22)

- The default start plan was ADR 0173 D7 (a), approved by the maintainer on 2026-11-18 (ADR 0186 Q2) and built by ADR 0741: "an approved change of a default, so rule 3's 'defaults stay bit-identical' is lifted for exactly the outputs listed". Measured: the store does not change the solution (`lid_drag` equal bits with and without it) ([[adr-thread-workspace-layer]]).
- The Electron Studio reads `store.rts` with its own TypeScript reader (R2), measured in the ADR 0350 spike at p50 14.2 ms, p99 22.4 ms for a 1e6-cell scalar, bit for bit with the Rust reader ([[adr-thread-studio-electron]]).

## Related

- [[run]]
- [[studio]]
- [[rcfd-tsstore]]
