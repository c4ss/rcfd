---
type: entity
tags: [crate]
sources: [doc-architecture, doc-dynamic-mesh, doc-cosimulation-protocol, doc-acoustics, doc-bfs-convergence-findings, doc-component-diagonal-default, doc-meshing, adr-thread-piso-stability-consistent-correctors, adr-thread-bdf2-step-consistent-face-flux, adr-thread-steady-kernel-warped-mesh, adr-thread-gradient-options, adr-thread-compressible-acoustic-boundaries, adr-thread-acoustic-wave-solver]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-fv

**What it is:** The finite-volume core. (The architecture page gives no description; ADR 0115 places the gradient limiters in `rcfd_fv::gradient`.) ([[doc-architecture]])
**Where:** `crates/rcfd-fv/` — layer 3 (io / fv / particles) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]], [[rcfd-linalg]], [[rcfd-gpu]]
- Used by: [[rcfd-turb]], [[rcfd-post]], [[rcfd-api]], [[rcfd-cli]]

## Details

- **Pressure-based solvers:** `SimpleSolver` (SIMPLE / SIMPLEC, collocated, Rhie-Chow; `h_from_solution` default true since wave 6) ([[doc-bfs-convergence-findings]]); `PimpleSolver` (PISO / PIMPLE) ([[doc-dynamic-mesh]]). See [[simple]], [[piso]]; the component-diagonal key is on [[component-diagonal]] ([[doc-component-diagonal-default]]).
- **Moving meshes:** ALE step, `sliding`, `sliding_periodic`, `overset::OversetSolver`, `fsi` (`strong_step`, `relaxation_from_added_mass`), `sixdof`, `ale` ([[doc-dynamic-mesh]]); see [[dynamic-mesh]], [[sliding-mesh]], [[overset-mesh]].
- **Co-simulation:** `cosim` (`coupled_step`, `FlowWall`, `SprungWall`, `InterfaceMap`) ([[doc-cosimulation-protocol]]); see [[cosimulation-protocol]].
- **Compressible solver and acoustics:** `compressible.rs`, `compressible_bc.rs` (impedance, quiet, `MsdWall`), `sponge.rs`, `acoustic_wave.rs` ([[doc-acoustics]]); see [[aeroacoustics]].
- **Adaptive SIMPLE loops:** `rcfd_fv::adaptive` (`axis_variations`, `directional_axes`, `run_adaptive_simple_directional*`) ([[doc-meshing]]); see [[mesh-adaptation]].
- **P3 ADR threads touching this crate:** PISO/PIMPLE consistent correctors and outer criteria in `pimple.rs` ([[adr-thread-piso-stability-consistent-correctors]]); the step-consistent face flux ([[adr-thread-bdf2-step-consistent-face-flux]]); `implicit_non_orth_pressure` in `simple.rs` ([[adr-thread-steady-kernel-warped-mesh]]); `GradientOptions` helpers in `diffusion.rs`, `cht.rs`, `electric.rs` ([[adr-thread-gradient-options]]); the sponge and acoustic boundaries of `CompressibleSolver` ([[adr-thread-compressible-acoustic-boundaries]]); `acoustic_wave.rs` ([[adr-thread-acoustic-wave-solver]]).

## Open questions

_Filled in by P1-P3 ingests._
