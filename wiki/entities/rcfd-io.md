---
type: entity
tags: [crate]
sources: [doc-architecture, doc-mesh-import, doc-meshing, doc-component-diagonal-default]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# rcfd-io

**What it is:** File input and output. (The architecture page gives no description; to confirm in P1.) ([[doc-architecture]])
**Where:** `crates/rcfd-io/` — layer 3 (io / fv / particles) of the [[crate-graph]].

## Dependencies

- Depends on: [[rcfd-core]], [[rcfd-mesh]]
- Used by: [[rcfd-post]], [[rcfd-tsstore]], [[rcfd-api]], [[rcfd-workspace]], [[rcfd-cli]]

## Details

- **Mesh readers:** Fluent `.msh` / `.cas` (`crates/rcfd-io/src/fluent.rs`, F-118; mesh only, binary unverified, HDF5 and gzip refused), alongside Gmsh and OpenFOAM meshes; the family is chosen from the first byte ([[doc-mesh-import]]).
- **Surface readers:** `rcfd_io::stl`, `obj`, `gltf` (F-119, F-280) feed the surface-meshing pipeline ([[doc-meshing]]).
- **Benchmark registry:** `cases/*/case.json` descriptors are read by `rcfd-io::cases` ([[doc-component-diagonal-default]]); see [[benchmark-descriptor]].
- Mesh importers: see [[mesh-generators]].

## Open questions

_Filled in by P1-P3 ingests._
