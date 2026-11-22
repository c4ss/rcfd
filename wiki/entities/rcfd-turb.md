---
type: entity
tags: [crate]
sources: [doc-architecture, adr-thread-transient-rans, adr-thread-periodic-seam-diffusivity]
created: 2026-11-22
updated: 2026-11-22
status: stub
---

# rcfd-turb

**What it is:** Turbulence models. ([[doc-architecture]])
**Where:** `crates/rcfd-turb/` — layer 4 (turb / post / tsstore) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-fv]]
- Used by: no workspace crate (binary / top of graph)

## Details

- **Models:** k-epsilon (`kepsilon.rs`), SST (`sst.rs`), Spalart-Allmaras (`sa.rs`), wall functions (`wall.rs`, `WallConstants`, Spalding law) and `wall_distance`; `model.rs` holds `supports_transient`, `begin_time_step`, `updates_in_outer_iterations` ([[adr-thread-transient-rans]], [[k-omega-sst]], [[k-epsilon]], [[spalart-allmaras]]).
- **Periodic seam pair diffusivity** for every model (`turbulence.periodic_seam.diffusivity = "pair"`) ([[adr-thread-periodic-seam-diffusivity]]).
- The F-040 channel test `crates/rcfd-turb/tests/channel_wall_function.rs` predates the ADR process (ADR 0707) ([[adr-thread-transient-rans]]).

## Open questions

_Filled in by P1-P3 ingests._
