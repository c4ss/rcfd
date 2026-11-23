---
type: source
tags: [reference-data, model-constants]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Interchange formats, registers and GLB validation (F-119, F-124, F-117, ADR 0011)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Interchange formats, registers and GLB validation (F-119, F-124, F-117, ADR 0011)" (lines 524-560) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Interchange formats, registers and GLB validation (F-119, F-124, F-117, ADR 0011): VTK and Khronos oracles.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Limits were registered in the ADR before any code ran. "VTK" is VTK 9.6.2 installed with pip; its readers are the independent oracle. No commercial viewer was available.

| Check | Measured | Limit | Result |
|---|---|---|---|
| S1 STL ASCII vertex round trip | max abs diff 0 | 0 | pass |
| S2 STL binary round trip, relative error | 6.0e-8 or less | 6.0e-8 | pass |
| S3 STL area / enclosed volume, ASCII | within 1e-12 | 1e-12 | pass |
| S5 welded 20^3 box boundary | 2402 vertices, 4800 triangles, 7200 edges, Euler 2, 0 boundary, 0 non-manifold edges | same | pass |
| STL in VTK `vtkSTLReader` (binary and ASCII; hex, tet, prism meshes) | enclosed volume 2.0 on the test mesh, mixed patch 22 triangles | equal to written | pass |
| T1/T2 Tecplot round trip (gmsh tet, hex, prism samples and a hand-built mixed mesh) | bit exact, volume match 1e-13 | 0 / 1e-13 | pass |
| Tecplot in VTK `vtkTecplotReader` | cell counts 64 / 1139 / 336 and 4 (mixed) as written, volume sums exact; mixed mesh volume 1.515000004917 vs 1.515 (VTK stores float32 coordinates) | equal | pass (tolerance 1e-7 in the script for the float32 reader) |
| P1 Plot3D round trips | bit exact for binary double (both byte orders) and ASCII; single precision 6.0e-8 or less | 0 / 6.0e-8 | pass |
| P3 `block_to_mesh` graded 9x5x4 box | 96 cells, volume and area within 1e-12 | 1e-12 | pass |
| Plot3D sample files in VTK `vtkMultiBlockPLOT3DReader` | blocks (5,4,3), (2,2,2), (7,3,4) in binary and ASCII | equal sizes | pass |
| P5 public sample `bluntfinxyz.bin` (https://raw.githubusercontent.com/pyvista/vtk-data/master/Data/bluntfinxyz.bin, SHA-256 b0748b066152c7001d2979245e729da32b44eb6f171b0c49cf6ed0eb84fe0e6a, 491532 bytes, repository licence Apache-2.0, per-file NASA licence not found) | one block 40x32x32; bounds x [-7.815747261047363, 14.362203598022461], y [0, 8.327558517456055], z [0, 5.7242512702941895], equal to VTK's | equal | pass |
| `block_to_mesh` on the blunt fin | refused: degenerate face (collapsed edge) | n/a | recorded limitation, not a registered criterion |
| X1-X3 XDMF consistency, X4 VTK `vtkXdmfReader` | counts, volume 2.0, positive cell volumes, cell data p = 10..13 read | equal | pass after reversing the wedge triangle winding in the writer (first run gave negative wedge volumes) |
| H1 fuzz (all prefixes + 3000 mutations) STL, Tecplot, Plot3D, GLB | 0 panics | 0 | pass |
| H2 header claiming `u32::MAX` / `i32::MAX` items | `Err` in under 1 s | under 1 s | pass |
| H4 located errors | `line 7`, `byte offset`, JSON pointers found in messages | present | pass |
| G1-G8 GLB rules, one negative test each, fuzz, hand-built GLB | all pass | 0 issues on writer output | pass |
| Khronos gltf-validator 2.0.0-dev.3.10 on `contour_zmax.glb`, `contour_all.glb`, `plain.glb` (20^3 box) | errors 0, warnings 0, infos 0, hints 0 | errors 0 | pass |
| `vtkGLTFReader` on `contour_zmax.glb` | 1600 points, 800 triangles, arrays NORMAL, COLOR_0, _SCALAR | loads | pass; no viewer colour check, "viewer with correct colours" NOT demonstrated |
| R1 box on 20^3 | 1000 cells, volume rel err 8.9e-16 | 1e-12 | pass |
| R2 threshold x >= 0.5 | 4000 cells, mean 0.750000000000062, integral over cube 0.499999999999995, min 0.525, max 0.975 | 1e-12 | pass |
| R3 sphere r = 0.3 on 40^3 | volume 0.112625 vs 0.113097, rel err 4.2e-3 | 3e-2 | pass |
| R4 boundary adjacent xmin layers 1 / 2, all six patches | 400 / 800 / 2168 cells; interface area 0.999999999999995 | exact; 1e-12 | pass |
| R5 algebra, point-neighbour grow / shrink of 10^3 block | 1728 / 1000 | 1728 / 1000 | pass |
| R5 face-neighbour grow of the 10^3 block | 1600 | 1728 | MISS (wrong arithmetic in the ADR, see errata); `r5_face_grow_known_miss` |
| R6 patch changes (settings layer, 20x20 mesh) | exactly 100 cells, others bit identical | register count | pass |
| R7 porous sink on register (20x20 mesh) | 100 non-zero cells = register count, bit-exact zero elsewhere | equal | pass |
| R8 step field tau 0.5 / linear field | 800 cells (volume fraction 0.1) / 0 | 800 / 0 | pass |
| R9, R10 statistics and located errors | empty register `None`; every error names the setting path | as registered | pass |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
