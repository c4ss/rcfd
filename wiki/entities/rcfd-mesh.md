---
type: entity
tags: [crate]
sources: [doc-architecture, doc-meshing, doc-mesh-quality, doc-dynamic-mesh, doc-acoustics, adr-thread-tet-mesher-output-checks, adr-thread-mesh-adaptation, adr-thread-acoustic-wave-solver]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-mesh

**What it is:** Mesh data structures. ([[doc-architecture]])
**Where:** `crates/rcfd-mesh/` — layer 1 (mesh / linalg / project) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]]
- Used by: [[rcfd-dist]], [[rcfd-fem]], [[rcfd-io]], [[rcfd-fv]], [[rcfd-particles]], [[rcfd-turb]], [[rcfd-post]], [[rcfd-tsstore]], [[rcfd-workspace]], [[rcfd-cli]]

## Details

- Besides the mesh data structures it holds rcfd's mesh generators and tools; catalogue on [[mesh-generators]] ([[doc-meshing]]).
- Modules named in the docs: `surface`, `surface_remesh`, `tetmesh`, `predicates`, `sweep`, `octree`, `adapt` and `adapt::aniso`, `polydual`, `gen` (`annulus_3d_sector`, `circular_duct_ogrid`), `fan`, `fan_mesh`, `fan_cgrid`, `fan_design` (`ComputerFan`) ([[doc-meshing]], [[doc-acoustics]]); `quality`, `measures`, `edit`, `inflate` ([[doc-mesh-quality]]); `layering`, `remesh`, `overset`, `sliding_periodic`, `non_conformal` ([[doc-dynamic-mesh]]).
- Quality report: [[mesh-quality]]. Adaptation: [[mesh-adaptation]]. Fan passages: [[fan-passage-mesh]]. Moving meshes: [[dynamic-mesh]], [[sliding-mesh]], [[overset-mesh]].
- "Most tools are partial and several exist in the library only" ([[doc-meshing]]).
- **P3 ADR threads:** `tetmesh.rs` fold fix and output check ([[adr-thread-tet-mesher-output-checks]]); `rcfd_mesh::adapt` and the directional lattice ([[adr-thread-mesh-adaptation]]); `gen::circular_duct_ogrid` ([[adr-thread-acoustic-wave-solver]]).

## Open questions

_Filled in by P1-P3 ingests._
