---
type: entity
tags: [crate]
sources: [doc-architecture, doc-particles, adr-thread-particle-wall-and-droplets]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-particles

**What it is:** One-way coupled Lagrangian particle tracking. ([[doc-architecture]])
**Where:** `crates/rcfd-particles/` — layer 3 (io / fv / particles) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]]
- Used by: [[rcfd-api]]

## Details

- Decision records: ADR 0028 ([[doc-architecture]]).
- See `docs/particles.md`.
- Method and scope: [[lagrangian-particles]] ([[doc-particles]]). F-056 stays `partial`; F-128 done ([[doc-particles]]).
- The case section `particles` is configured by `rcfd-api::particles`; library entry points `check`, `build_tracker`, `track_frozen`, `track_transient`; the run loop drives the tracker itself since ADR 0037 ([[doc-particles]]).
- Modules / types named: `Tracker`, `Coupling`, `Dispersion`, `drw_msd`, `film`, `erosion` (`EROSION_PRESETS`, `PRESET_PROVENANCE`), `rough` ([[doc-particles]]).
- Despite the one-way default, opt-in paths exist: the exchange record (observation only) and the droplet feedback of F-326 (ADR 0348) in the transient two-way path ([[doc-particles]]).

> [!warning] Contradiction — layer number
> [[doc-particles]] calls the crate "layer 2: depends on `rcfd-core` and `rcfd-mesh` only"; [[doc-architecture]] puts it in layer 3 (io / fv / particles). The dependencies agree; only the layer number differs (between repo docs).
- **P3 ADR thread:** wall film, erosion preset, dispersion from turbulence fields, droplet evaporation and breakup, `Tracker::enable_droplet_exchange()` ([[adr-thread-particle-wall-and-droplets]]).

## Open questions

_Filled in by P1-P3 ingests._
