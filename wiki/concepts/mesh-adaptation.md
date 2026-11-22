---
type: concept
tags: [mesh, adaptation, amr]
sources: [doc-meshing]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Mesh adaptation (AMR and directional refinement)

Refining and coarsening a mesh during a solve where an error indicator asks for it. In rcfd this is hanging-node octree/quadtree refinement producing conforming polyhedral cells, plus a per-axis (directional) lattice ([[doc-meshing]]).

## How rcfd uses it

- **`AmrMesh`** (F-102, ADR 0054, partial; library only, no case-file hook): forest of octrees over a tensor box; faces next to finer leaves are replaced by the finer faces, so cells are closed hanging-node polyhedra; 2:1 balance optional ([[doc-meshing]]).
- **Indicators:** `gradient_indicator`, `hessian_indicator`, `zz_indicator` (`eta = |g* - g| sqrt(V)`), `region_indicator`; `MarkSpec` with a dead band (`coarsen_below < refine_above`) and level limits ([[doc-meshing]]).
- **Transfer:** `Injection` (conservative) or `Linear` (exact for linear fields; measured error 6.7e-16) ([[doc-meshing]]).
- **Solve test:** adapted beats coarse with fewer cells than fine, but the registered "adapted within 3x of the uniform fine error" missed (3.71, `a4_adapted_vs_fine_known_miss`); a wider-band set-up registered afterwards passes (`a4b`). "adapted cavity matches fine uniform" was NOT tested on a cavity ([[doc-meshing]]).
- **Directional (`AnisoMesh`, F-348, ADR 0267):** one level per axis; isotropic states give `AmrMesh::to_mesh` bit for bit; box merge opt-in (F-439, ADR 0341); coarsening in the adaptive SIMPLE loop opt-in (F-441, ADR 0343) with `phi <= theta / 4`; `phi = 0` is the loop without coarsening bit for bit ([[doc-meshing]]).
- **Measured on the suction layer** (ADR 0247 set-up): directional 1547 cells error 0.00298; isotropic 3841 cells 0.01159; inverted axis choice 1.18 ([[doc-meshing]]).
- Non-orthogonal correction on adapted meshes needs deferred iterations (ADR 0054 addendum 1) ([[doc-meshing]]).

## Related

- [[mesh-generators]]
- [[rcfd-mesh]]
- [[rcfd-fv]]
- [[known-miss]]

## Open questions

- No case-file hook for adaptation; anisotropic, target-node-count and multigrid precoarsening criteria "NOT done" ([[doc-meshing]]).
