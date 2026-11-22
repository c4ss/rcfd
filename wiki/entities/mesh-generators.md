---
type: entity
tags: [mesh, meshing]
sources: [doc-meshing, doc-mesh-import, doc-mesh-quality, doc-dynamic-mesh, doc-acoustics, doc-structural-solver]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Mesh generators and mesh tools

**What it is:** the catalogue of rcfd's own mesh generators and mesh tools, with their status, entry points and measured quality ([[doc-meshing]]).
**Where:** `crates/rcfd-mesh/src/` (`surface`, `surface_remesh`, `tetmesh`, `predicates`, `sweep`, `octree`, `adapt`, `polydual`, `gen`, `fan`, `fan_mesh`, `fan_cgrid`, `edit`, `layering`, `remesh`, `overset`); readers in `rcfd-io`.

## Details

| Tool | Entry | Status / key result |
|---|---|---|
| STL / OBJ / glTF surface readers | `rcfd_io::stl, obj, gltf` (F-119, F-280, ADR 0184) | feed the surface pipeline ([[doc-meshing]]) |
| Surface diagnose / repair | `rcfd_mesh::surface` (F-232, partial) | no self-intersection detection or wrapping ([[doc-meshing]]) |
| Surface remeshing | `rcfd_mesh::surface_remesh` (F-131, partial) | brute-force closest point, "a few thousand triangles" ([[doc-meshing]]) |
| Tetrahedral mesher | `rcfd_mesh::tetmesh` (F-132, partial; ADR 0030, 0104) | library only, no CLI or case hook; Delaunay with exact predicates; `quality_sweeps` default 0 ([[doc-meshing]]) |
| Sweep / thin-volume / multizone hex | `rcfd_mesh::sweep` (F-235, ADR 0047) | all hex; `pipe_mesh` is the case-tree entry (`pipe_sweep`) ([[doc-meshing]]) |
| Octree Cartesian core + prism layers | `rcfd_mesh::octree::fill` (F-234, partial) | sphere non-orthogonality 77.4 deg vs 70 (known miss) ([[doc-meshing]]) |
| Adaptation (isotropic / directional) | `rcfd_mesh::adapt`, `adapt::aniso` (F-102, F-348) | library only; see [[mesh-adaptation]] ([[doc-meshing]]) |
| Polyhedral dual | `rcfd_mesh::polydual::tet_to_polydual` | volume to about 1e-15 ([[doc-meshing]]) |
| Exact annular sector | `gen::annulus_3d_sector` (ADR 0116), `mesh.generator = "annulus_3d_sector"` | no blade; planar faces, exact MRF per cell ([[doc-meshing]]) |
| Axial-fan passage (H-slit, C-grid) | `rcfd_mesh::fan_mesh`, `fan_cgrid` (F-133 partial, ADR 0065, 0068 ...) | gates missed; see [[fan-passage-mesh]] ([[doc-meshing]]) |
| Circular duct O-grid | `rcfd_mesh::gen::circular_duct_ogrid` (F-453, ADR 0366) | used by the acoustic wave solver ([[doc-acoustics]]) |
| Boundary-layer inflation | `rcfd_mesh::inflate::inflate_patch`, `rcfd inflate` (F-108, ADR 0021) | exit 4 on quality errors ([[doc-mesh-quality]]) |
| Mesh editing | `rcfd_mesh::edit` | library only, no CLI / case entry ([[doc-mesh-quality]]) |
| Layer stacks, local tet remeshing | `rcfd_mesh::layering`, `rcfd_mesh::remesh` | moving-mesh tools; see [[dynamic-mesh]] ([[doc-dynamic-mesh]]) |
| Fluent `.msh` / `.cas` import | `mesh.source = "file"` (F-118) | mesh only, binary unverified ([[doc-mesh-import]]) |

- "Most tools are partial and several exist in the library only" ([[doc-meshing]]).
- All literature attributions of the meshing page "are from memory and unverified" ([[doc-meshing]]).
- The remesher depends on input vertex numbering, so the same geometry from an STL file and from code remeshes to different surfaces; the tetrahedral mesher does not ([[doc-meshing]]).
- The tetrahedral mesher does not mesh a thin fan blade reliably (smallest dihedral 0.16 deg); the structural solver uses a structured blade split instead ([[doc-structural-solver]]).

## Related

- [[rcfd-mesh]]
- [[mesh-quality]]
- [[mesh-adaptation]]
- [[fan-passage-mesh]]
- [[rcfd-io]]

## Open questions

- The surface-to-tet pipeline has no case hook; the plan that connects it to a case is the M-FAN-B design (`docs/designs/m-fan-b.md`, not ingested here) ([[doc-meshing]]).
