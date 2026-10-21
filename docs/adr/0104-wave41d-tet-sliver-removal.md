# ADR 0104: Wave 41D, numbering dependence and an opt-in quality pass for the tetrahedral mesher (F-131, F-132)

Status: registered before the diagnostics and the registered tests were run. Limits are not changed after a run; a miss becomes an
ignored `*_known_miss` test with the limit intact. Library change: an opt-in stage in `rcfd_mesh::tetmesh`, off by default; the default
output must stay bit-identical (W0 below). Clean-room: first principles only; any literature attribution is marked UNVERIFIED. Written
at HEAD a2e0bfd, target `<target dir>`.

## Context

Wave 39B (ADR 0093) fed a closed two-solid STL file of the cylinder in a box through the curvature remesh and `tetrahedralize`. The
result is watertight (S2b passes) but misses the ADR 0030 T5 limits: minimum dihedral 1.84 deg vs 3.0 (S2) and radius-edge fraction
0.936 vs 0.99 (S3). The in-code surface `p1_pipeline_cylinder_in_box` (same coordinates, other vertex numbering) gives 3.34 deg and
0.927. Both S2 and S3 stay ignored known misses: they describe the default mesher, which this ADR does not change.

Reading of the code (before any run): `tetrahedralize` inserts the surface vertices in Morton order with the input id only as a tie
break, so its internal vertex ids do not depend on the input numbering unless two Morton codes tie. The input triangle order does leak
in: it sets the `SubSurf` triangle ids, the order of face-recovery splits and the classification seeds. The remesher (`remesh`) is a
sequence of split, collapse, flip and smooth sweeps over vertex and edge orders taken from the input ids, so it can depend on them.

## Smoke (run before this ADR was written)

`w0_default_fingerprints` (ignored, prints), default options, at HEAD a2e0bfd: (cell count, `Mesh::content_hash` of `to_mesh`):

| case | cells | content hash |
| --- | --- | --- |
| cube(2, 2) | 108 | 2507394269494118308 |
| icosphere(1, 2) | 320 | 15939201240145194707 |
| torus(2, 0.5, 24, 12) | 899 | 826700040160655771 |
| cyl_in_box (in code, no remesh) | 1137 | 9960403326714801105 |
| p1: in-code surface, curvature remesh | 1061 | 1192961234831520497 |
| S2: STL surface, curvature remesh | 1115 | 7631245200477700787 |
| icosphere(1, 2), size 0.3 (t6) | 823 | 4586941130221034709 |
| icosphere(1, 2), size 0.25 (w19b T10) | 1277 | 9750081027263524416 |
| cube(2, 4), size 0.5 (w19b T10) | 597 | 11005660464813738162 |

The curvature remesh everywhere is ADR 0093's: `SizeField::Curvature { theta: 0.5, h_min: 0.1, h_max: 1.0 }`, default options.

## Part 1: diagnosis (ignored tests in `crates/rcfd-io/tests/w41d_tet_quality.rs`, print only)

Hypotheses and the result that falsifies each, fixed before the runs:

* **H1** the remesh output depends on the input vertex numbering. **D1**: remesh the in-code and the STL surface; compare the two
  remeshed surfaces as geometry: the sorted multiset of triangles, each a canonical (rotated to its smallest coordinate triple) triple of
  bitwise coordinates, plus the tag. H1 is falsified if the multisets are equal.
* **H2** `tetrahedralize` depends on the numbering of a fixed geometric surface. **D0**: tetrahedralize the welded STL surface without
  remeshing and compare to the in-code `cyl_in_box` (1137 cells). **D2**: on the in-code remeshed surface, (a) reverse the vertex ids
  only, (b) reverse the triangle order only, (c) both; tetrahedralize each. Outputs are compared as geometry (sorted multiset of cells,
  each the sorted bitwise coordinates of its 4 vertices). H2 is falsified for the vertex ids if D0 and D2a give equal multisets, and
  for the triangle order if D2b does.
* **D3** where the bad cells sit (default options, both paths): for every cell with minimum dihedral below 3 deg or radius-edge ratio
  above 2, record: number of faces on the surface (0, 1, 2), number of vertices on the surface, whether a vertex is an interior Steiner
  point, whether the edge with the smallest dihedral angle is a surface edge, the tag of the nearest surface, and the geometry class
  of its centroid (nearest of: cylinder side, cylinder rim, cylinder cap, box face, box edge; interior if farther than 0.3 from all).
  Descriptive, no limit. Expected from the code (stated before the run): the radius-edge failures are cells whose circumcentre
  encroaches on the surface, which the default refinement skips (`split_boundary` off); slivers pass the radius-edge test and are a
  separate failure.

## Part 2: the opt-in quality pass

New `TetOptions` fields, defaulting to the current behaviour, select an extra stage after the existing refinement and flips. Its
ingredients, in the order they will be tried (only those that D3 and development runs show to be needed are kept):

1. refinement with surface splitting (the existing `split_boundary`), checked for termination on this input: its input angles are
   all at least 90 deg seen from the domain (box edges 90 deg, the cylinder rim reflex);
2. a dihedral-angle objective for the existing 2-3 flips and edge removal, applied to cells below a target minimum dihedral;
3. circumcentre insertion for slivers (Delaunay refinement on a dihedral criterion), with the same encroachment rule as 1;
4. splitting the surface edge of flat cells with two surface faces at a dihedral threshold (today only below volume quality 1e-9);
5. last, smoothing of interior Steiner vertices only: a move is accepted only if exact `orient3d` keeps every incident cell positive
   and the worst incident quality improves. Surface vertices never move.

Constraints: surface edges are split only at midpoints and triangles keep their tags, so the boundary is a conforming subdivision of
the remeshed surface (volume to rounding, patches unchanged). No `HashMap`/`HashSet` iteration in the new code (per-process seeds).
Both quality measures are re-measured after every stage. The development of the pass uses the target cases (STL and in-code
cylinder in a box); this is disclosed, and the exact option values are frozen in an amendment before the registered run. Held-out
cases (Q7) are not run with the pass before that amendment.

## Registered tests (`crates/rcfd-io/tests/w41d_tet_quality.rs`)

Pipelines: STL path = ADR 0093 `from_stl_file` then the curvature remesh; in-code path = `cyl_in_box` then the curvature remesh.
"With the pass" = the frozen options of the amendment.

* **W0** default bit-identity: default options (and the three size options) give exactly the nine fingerprints of the smoke table.
* **Q1** STL path with the pass: remeshed surface closed manifold; volume of `to_mesh` equal to the remeshed surface volume within
  1e-10 relative; 2 patches; every cell volume positive and `tet_quality` min volume positive; `check_mesh(..).is_ok()`; minimum
  dihedral at least 3.0 deg (ADR 0030 T5).
* **Q2** STL path with the pass: fraction of cells with radius-edge ratio at most 2 at least 0.99 (ADR 0030 T5).
* **Q3, Q4** the same as Q1, Q2 on the in-code path.
* **Q5** patches preserved, both paths with the pass: per tag, the boundary area of the volume mesh equals the remeshed surface area
  of that tag within 1e-10 relative; every boundary triangle centroid lies on a remeshed input triangle of the same tag within 1e-9
  (plane distance and edge tests as T1 of ADR 0030); `V - E + F - C = 2`.
* **Q6** determinism with the pass: two runs of the STL path give equal points (bitwise) and cells.
* **Q7** held-out no-regression with the pass: cube(2, 2), icosphere(1, 2) and the t5b torus (torus(2, 0.5, 48, 24) remeshed with
  `Curvature { theta: 0.6, h_min: 0.05, h_max: 0.8 }`): volume within 1e-10 of the input surface volume, `check_mesh` ok, radius-edge
  fraction at least 0.99, minimum dihedral at least 3.0 deg, minimum volume positive.

Verdict rules: F-131 becomes done only if Q1, Q2 and Q5 (STL path) pass as non-ignored tests and W0 passes. F-132 cannot become done
here whatever the result: its acceptance names boundary layers, which nothing in this ADR tests; its note is extended.
Each limit is reported separately; a partial result (for example Q1 met, Q2 missed) is reported as such.
