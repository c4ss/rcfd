---
type: entity
tags: [crate]
sources: [doc-architecture, doc-acoustics, doc-meshing, doc-cosimulation-protocol, doc-structural-solver]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-post

**What it is:** Post-processing and VTK output, including VTK output of a solid result and the solid glue modules. ([[doc-architecture]])
**Where:** `crates/rcfd-post/` — layer 4 (turb / post / tsstore) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-io]], [[rcfd-fv]], [[rcfd-fem]]
- Used by: [[rcfd-workspace]], [[rcfd-mcp]], [[rcfd-cli]]

## Details

- Decision records: ADR 0278 ([[doc-architecture]]).
- **Acoustics:** `fwh.rs` (FW-H 1A), `acoustics.rs` (metrics), `rotor.rs` (Gutin-type tones), `noise_proxies.rs`, `surface_series.rs`, `duct_modes.rs`, `wave_drive.rs`, `solid_acoustic.rs`; see [[aeroacoustics]] and [[fw-h]] ([[doc-acoustics]]).
- **Turbomachinery post** `rcfd_post::turbo` (F-123, ADR 0054, partial, library only): span and streamwise surfaces, circumferential and mass-flow averages, pump and compressor performance; "NO turbomachinery flow solution was computed" ([[doc-meshing]]).
- **Solid glue:** `solid_coupling::StaticSolid` (ADR 0286), `solid_dynamic::DynamicSolid` (ADR 0410), `solid_fsi::ChannelMotion` as co-simulation pieces ([[doc-cosimulation-protocol]]); radiation impedance of a baffled plane (F-579) ([[doc-structural-solver]]). See [[cosimulation-protocol]], [[structural-solver]].

## Open questions

_Filled in by P1-P3 ingests._
