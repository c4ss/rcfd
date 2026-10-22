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

## Diagnosis results (D0 to D3, default options, run after the registration above)

* **D0**: the welded STL surface equals the in-code surface as geometry, and their tetrahedralizations are equal as geometry (1137
  cells each). The input numbering does not reach the tetrahedral mesh.
* **D1**: the two remeshed surfaces both have 372 triangles and 190 points but are NOT equal as geometry: 272 of 372 triangles are
  common. H1 holds: the remesher depends on the vertex numbering. Mechanism (read from the code after the run, not isolated further):
  `surface_remesh` sorts the long edges to split by length and breaks ties by the vertex ids `(a, b)`; the box and cylinder grids have
  many edges of exactly equal length, so the tie-breaking, and from there the splits, collapses, flips and the Gauss-Seidel smoothing
  in vertex-id order, follow the numbering.
* **D2**: on the fixed in-code remeshed surface, reversing the vertex ids, the triangle order or both gives a tetrahedral mesh equal as
  geometry to the base (1061 cells, minimum dihedral 3.3395 deg, fraction 0.9274 in all four). H2 is falsified on this input. So the
  1.84 vs 3.34 deg difference of ADR 0093 comes entirely from the two different remeshed surfaces.
* **D3** (cells failing a T5 limit): in-code 77 of 1061, STL 71 of 1115. Every in-code failure and all but one STL failure is a
  radius-edge failure (ratio over 2 but dihedral at least 3 deg), not a sliver. They sit next to the surface: near box faces 18 / 28
  (the STL count includes the sliver), near the cylinder side 40 / 28, at the cylinder rim 17 / 9, cylinder cap 0 / 1, interior 2 / 5
  (in-code / STL). Most have one surface face and one interior Steiner vertex (49 / 35) or two surface vertices and two interior Steiner vertices. The single STL sliver (1.84 deg, ratio
  27.9) is a cell on a box-face triangle whose fourth vertex is an interior Steiner point 0.003 from that face. Every failing cell has
  at least one interior (movable) vertex. The cause of the radius-edge failures: their circumcentres lie close to the surface, the
  unconstrained Bowyer-Watson cavity of such a point crosses a surface face into outside tetrahedra, and the default refinement
  (`split_boundary` off) then skips them.

## Amendment 1 (written after the development runs, before the registered Q runs)

Development runs (an ignored `x_dev` print test on the target cases only, disclosed; removed after the freeze):

* `split_boundary = true` alone: in-code path fails (Steiner budget exhausted, the known runaway); STL path 1128 cells, 1.84 deg,
  fraction 0.935. Ingredient 1 is dropped.
* smoothing (volume-quality objective) plus flips below volume quality 0.06, 10 sweeps: in-code 16.4 deg / 0.987, STL 6.1 deg / 0.979;
  30 sweeps or flip thresholds 0.08 and 0.1 stay at 0.979 to 0.988. The remaining failures have their shortest edge on the surface
  and ratios 2.0 to 3.4; no remeshed surface triangle has a circumradius over shortest edge above 2.
* smoothing objective changed to the smaller of the normalised volume quality and the normalised radius-edge quality: 14.2 deg /
  0.989 and 10.5 deg / 0.980.
* plus constrained circumcentre insertion (cavity limited to inside tetrahedra, never crossing a surface face, inserted only if the
  cavity is star-shaped and contains the bad tetrahedron), 10 sweeps: in-code 1142 cells, 16.33 deg, fraction 0.9974; STL 1223
  cells, 17.66 deg, fraction 1.0000. 30 sweeps: the same fractions.

Frozen pass (what the registered tests run): `TetOptions { quality_sweeps: 10, ..Default::default() }`. Each sweep: (i) constrained
circumcentre insertion for every inside tetrahedron with circumradius over shortest edge above `max_radius_edge` (2.0); (ii) one
smoothing sweep over interior vertices in id order, candidates the neighbour centroid and the volume-weighted circumcentre mean, steps
1, 1/2, 1/4, 1/8, kept only if exact `orient3d` keeps every incident cell positive and the smallest incident objective grows by more
than 1e-6 relative, where the objective of a cell is the smaller of its volume quality times `6 sqrt 2` and `(sqrt 6 / 4)` divided by
its circumradius over shortest edge (both 1 on the regular tetrahedron); (iii) the existing 2-3 flips and edge removal for inside cells below volume quality 0.06. Stops early when a
sweep changes nothing. Ingredients 2 (dihedral-objective flips) and 4 (flat-cell split at a dihedral threshold) were not needed and
are not implemented. What I knew when writing this: the development numbers above; Q7 (held-out) has not been run with the pass.

## Results (release build, registered run after amendment 1)

| Check | Limit | Measured | Verdict |
| --- | --- | --- | --- |
| W0 default bit-identity | 9 fingerprints equal | all 9 equal | pass |
| Q1 STL path, watertight checks | closed, volume 1e-10, 2 patches, positive volumes, `check_mesh` ok | all hold (1223 cells) | pass |
| Q1 STL path, minimum dihedral | >= 3.0 deg | 17.66 deg (max 145.1 deg) | pass |
| Q2 STL path, radius-edge fraction | >= 0.99 | 1.0000 (max ratio 1.90) | pass |
| Q3 in-code path, watertight checks | as Q1 | all hold (1142 cells) | pass |
| Q3 in-code path, minimum dihedral | >= 3.0 deg | 16.33 deg (max 151.7 deg) | pass |
| Q4 in-code path, radius-edge fraction | >= 0.99 | 0.9974 (3 cells above 2; max ratio 2.19) | pass |
| Q5 patches, both paths | areas 1e-10, centroids on same-tag triangles 1e-9, Euler 2 | all hold | pass |
| Q6 determinism | bit equal | equal | pass |
| Q7a cube(2, 2) | T5 limits, volume, `check_mesh` | 108 cells, 29.5 deg, 1.0, volume error 0 | pass |
| Q7b icosphere(1, 2) | same | 320 cells, 55.05 deg, 1.0, 8.8e-16 | pass |
| Q7c remeshed torus (t5b) | same | 1265 cells, 16.73 deg, 1.0, 1.3e-15 | pass |

On the cube and sphere the pass inserted no point and moved no vertex (cell counts 108 and 320 as with the defaults); on the torus
it moved 52 vertices (defaults: 1297 cells, 5.26 deg, fraction 1.0).

Pass counters: STL 99 interior Steiner points (default 74), 530 moves, 501 flips in all; in-code 87 (default 67), 442 moves, 416
flips. Run time of each pipeline with the pass about 0.3 s (release).

The ADR 0093 records `s2_known_miss_pipeline_watertight_and_quality` and `s3_known_miss_radius_edge_fraction` stay ignored: they run
the default mesher, which is unchanged (W0), and still miss as recorded. Their doc comments now point to Q1 and Q2.

Verdicts: F-131 is done: Q1, Q2 and Q5 on the STL path pass as non-ignored tests with the ADR 0030 T5 limits and W0 passes, which
meets "a closed STL of a cylinder in a box meshes with curvature-based sizing; the result passes watertightness and quality checks"
when the quality pass is on. The default options still miss; a user must set `quality_sweeps`. F-132 stays partial: no test covers
boundary layers on a tetrahedral mesh of STL geometry.

Deviations: the pass was developed on the target cases (disclosed in amendment 1); the held-out cases were run only in the
registered run. Ingredients 1, 2 and 4 of Part 2 were not kept or not built. Ingredient 3 was replaced: the insertion uses the
radius-edge criterion, not a dihedral one, and a constrained cavity instead of the encroachment rule of ingredient 1. Part 2 said
both quality measures would be re-measured after every stage; the pass does not do that: the measures are taken once, on the final
mesh (the development runs measured them per variant, not per stage). The pass has no termination or quality guarantee (no
sliver exudation); it is a bounded heuristic (10 sweeps). No literature value enters a gate; the method names (Delaunay refinement,
constrained cavities, optimisation-based smoothing, bistellar flips) are from memory and UNVERIFIED.

Gate on a fresh archive of 82a76cc: `cargo fmt --check` and `cargo clippy --workspace --all-targets -D warnings` clean,
`cargo test --workspace --release --no-fail-fast` 1949 passed / 0 failed / 551 ignored (wave 40: 1939 / 0 / 549; this ADR adds 10
passed and 2 ignored diagnostics). The commit after the gate changes this ADR's text only.
