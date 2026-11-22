---
type: source
tags: [mesh, meshing, fan, how-and-why]
sources: []
origin: docs/meshing.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# Meshing: surfaces, volume meshes, adaptation and fan passages

**Origin:** [docs/meshing.md](../../docs/meshing.md) at `origin/main` 2e94e5ef
**Kind:** how-and-why page (447 lines)

## Summary

How each mesh generator and mesh tool of rcfd builds its mesh, what it assumes and what quality was measured. Most tools are partial and several are library-only; many sections record registered quality gates that were missed, and none of the fan meshes is validated for fan performance prediction. Literature attributions are "from memory and unverified".

## Key points

- **Pipeline** (wave 19B, ADR 0030; library-level, no CLI or case hook): STL / OBJ / glTF + unit -> `TriSurface` with named patches (`rcfd_io::stl, obj, gltf`; F-119, F-280) -> diagnose / repair (`surface.rs`, F-232) -> remesh with a size field (`surface_remesh.rs`, F-131) -> tetrahedralize (`tetmesh.rs`, F-132) -> `TetMesh::to_mesh` -> `rcfd_mesh::Mesh`. F-232, F-131, F-132 are `partial`. The plan that connects it to a case is the M-FAN-B design.
- **Surface diagnostics/repair:** free, non-manifold, misoriented edges, duplicates, slivers, holes (ear clipping). Not done: self-intersection detection, wrapping of leaky models, spike removal.
- **Surface remeshing:** `SizeField` (`Uniform`, `Curvature`, `Proximity`, `Box`, `Min`), split/collapse/flip/smooth with vertices on the input surface to 1e-9; closest-point queries brute force ("a few thousand triangles"). Depends on input vertex numbering (ADR 0104 D0 to D2).
- **Tet mesher:** exact `orient3d`/`insphere`, Bowyer-Watson Delaunay, Steiner boundary recovery, Delaunay refinement (`max_radius_edge` default 2.0), bistellar improvement. Opt-in quality pass `TetOptions::quality_sweeps` (ADR 0104, default 0, output unchanged); with `quality_sweeps: 10` the cylinder in a box gives min dihedral 17.7 deg. Torus: 0.634 of tets within radius-edge 2.0 (known miss). Laplace smoke on a 1277-cell sphere: relative L2 error 1.1e-2.
- **Sweep / thin-volume / multizone hex** (F-235, ADR 0047): `Path::Line`, `Revolve`, `NormalOffset`; all hex. Annulus skewness limit 1e-6 was "a derivation error" (ignored `s8_annulus_skewness_known_miss`).
- **Octree Cartesian core + snapped prism layers** (F-234, ADR 0047, partial): sphere case non-orthogonality 77.4 deg (limit 70, known miss); cavity on a generated mesh within 0.008 of Ghia (limit 0.03).
- **Adaptation** (F-102, ADR 0054, partial, library only): `AmrMesh` hanging-node polyhedra, gradient / Hessian / ZZ / region indicators, `MarkSpec` hysteresis, conservative field transfer; "adapted within 3x of fine" missed (3.71, `a4_adapted_vs_fine_known_miss`). `polydual` tet-to-polyhedral dual.
- **Directional refinement** (F-348, ADR 0267): `AnisoMesh` with per-axis levels; box merge (F-439, ADR 0341, opt-in); coarsening in the loop (F-441, ADR 0343, opt-in; $\phi \le \theta / 4$ required). Suction layer: directional 1547 cells error 0.00298 vs isotropic 3841 cells 0.01159.
- **Turbomachinery post** (`rcfd_post::turbo`, F-123, ADR 0054, partial): span/streamwise surfaces, averages, pump and compressor performance; "NO turbomachinery flow solution was computed".
- **Exact annular sector** (`gen::annulus_3d_sector`, ADR 0116): no blade, so it "does not open MRF on the fan passage".
- **Axial-fan H-slit sector mesh** (F-133 partial, ADR 0065; `mesh.generator = "axial_fan_sector"`): registered gates missed (A: 73.4 deg / 7.57 / 810 vs 70 / 2.0 / 300). Absolute `mrf` source refused on it; smoke solves use `rotating_frame`.
- **C-grid** (ADR 0068, `mesh.fan.topology = "c_grid"`) and its constructions: `smoothed` (ADR 0072), `camber_seam` (ADR 0079), `camber_seam_h` (ADR 0083), and library-only S (ADR 0692), S' (ADR 0696), S'' (ADR 0699), S''' (ADR 0702), S'''' (ADR 0713). S'''' at the s4 specs: reference 84.12 deg / 2.54 / 525.6, second blade 83.89 / 2.72 / 551.7 (within 84.5 / 4.0 / 1000). Details on [[fan-passage-mesh]].

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[mesh-generators]]
- [[fan-passage-mesh]]
- [[mesh-adaptation]]
- [[mesh-quality]]
- [[rcfd-mesh]]
- [[rcfd-post]]
- [[rcfd-io]]
- [[mrf]]
- [[fan-01]]
- [[known-miss]]
