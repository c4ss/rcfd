# ADR 0018: Mesh-quality measure catalogue (F-233) and mesh editing and repair (F-231)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new tests was
compiled or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and
the assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned
away. A test set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes
that were not fixed here) may be appended at the end without touching registered limits.

Roadmap: F-233 (first), F-231. Clean-room: published literature and first principles only; nothing from
vendor material, vendor installs or vendor manuals; no vendor names in the repository. The definitions below are
project definitions in the spirit of common FV and FE practice. Where a definition or a rule-of-thumb limit is attributed to
common practice it is marked "(from memory, unverified)". The registered checks do NOT depend on any literature value: every
expected number is derived below from the definitions in this ADR by closed-form geometry.

## Part A: F-233, measure catalogue

### A1 Definitions

Notation: cell `c` with centroid `x_c`; face `f` with centre `x_f` and area vector `S_f` oriented OUT of the cell being
evaluated (the mesh stores it owner to neighbour; for the neighbour cell it is negated); neighbour centroid `x_n` across an
internal face. Existing measures (non-orthogonality, skewness, aspect ratio, volume ratio) are unchanged.

1. **Face warp** `w_f` [degrees], per face. Triangle: 0. Quadrilateral `P0 P1 P2 P3`: for each of the two diagonal splits
   (`P0P2`: triangles `(P0,P1,P2)`, `(P0,P2,P3)`; `P1P3`: `(P0,P1,P3)`, `(P1,P2,P3)`) the angle between the two triangle
   normals; `w_f` is the larger of the two angles. Polygon with `n >= 5` vertices: maximum over the fan triangles about the
   vertex average (the fan used by `polygon_geometry`) of the angle between the fan-triangle normal and the face area vector.
   Computed after removal of consecutive coincident vertices (and of a closing duplicate); a polygon with fewer than 3
   distinct vertices, or a zero area vector, has `w_f = 0`. The cell value is the maximum over its faces. Definition class:
   "angle between the normals of the two triangles of a split quad" is the usual FE warpage idea (from memory, unverified); the
   max over both diagonals and the `n >= 5` fan form are project choices.
2. **Face angle** [degrees], per cell: smallest and largest interior vertex angle over all faces of the cell. Interior angle at
   vertex `i` of a polygon with unit normal `n^` (direction of its own area vector, so every polygon is counter-clockwise about
   its own normal): with `a = v_{i+1} - v_i`, `b = v_{i-1} - v_i`, `theta = atan2(n^ . (a x b), a . b)` taken in `[0, 360)`
   degrees, so a reflex vertex gives more than 180. If the area vector vanishes (collinear points) the unsigned angle
   `acos` of the normalised dot product is used. Consecutive coincident vertices are removed first; a polygon with fewer than 3
   distinct vertices contributes min 0 and max 180.
3. **Orthogonal quality** `OQ_c` and **inverse orthogonal quality** `1 - OQ_c`, per cell:
   `OQ_c = min over faces of min(cos alpha_f, cos beta_f)`, `cos alpha_f = S_f . (x_f - x_c) / (|S_f| |x_f - x_c|)` (all faces)
   and `cos beta_f = S_f . (x_n - x_c) / (|S_f| |x_n - x_c|)` (internal faces only). A zero-length vector gives cosine 0 (the
   conservative choice; a cell whose face centres coincide with its centroid is a flat cell). Common FV practice (from memory,
   unverified). `1 - OQ_c` is in `[0, 1]` for a convex cell with an interior centroid; 0 is perfect.
4. **Ortho-skew**, per cell: `1 - min over faces of cos alpha_f` (cell-internal alignment only, no neighbour term). Project
   name for the intra-cell part of 3; `ortho_skew <= inverse_orthogonal` always, equal on boundary-only cells.
5. **Size change**, per cell: `max over internal faces of V_n / V_c` (growth of the neighbours relative to this cell); 1 for a
   cell without internal faces. The maximum over all cells equals the existing `max_volume_ratio`.
6. **Edge-length ratio**, per face: longest edge over shortest edge of the face loop (original loop, coincident vertices NOT
   removed, so a collapsed edge gives infinity). The existing cell aspect ratio is over all edges of the cell; this is the
   face-local version. Cell value: maximum over its faces.

Reporting: `infinity` is stored in reports as `1e30` (JSON has no infinity). Library functions return the true value.

### A2 Poor-mesh warning list

`QualityReport` gains, with `#[serde(default)]` so old JSON still parses: `measures` (summary: max warp, min and max face
angle, max ortho-skew, max inverse orthogonal quality, max size change, max edge-length ratio), `poor_counts` (number of
offenders per measure), `poor_list` (the worst `poor_list_cap` offenders per measure, sorted by value divided by limit,
descending: `{measure, element: "face"|"cell", index, value, limit}`) and `poor_warnings` (one text line per measure that has
offenders). These are SEPARATE from the existing `warnings`/`errors` lists, so the pass/fail behaviour (`is_ok`, the CLI exit
code 4, the existing warning texts and counts) cannot change. Default limits (new `Thresholds` fields; rules of thumb from
common practice, from memory, unverified): `warp_warn = 15` deg, `face_angle_min_warn = 20` deg, `face_angle_max_warn = 160` deg,
`inverse_orthogonal_warn = 0.9`, `ortho_skew_warn = 0.85`, `edge_length_ratio_warn = 100`, size change uses the existing
`volume_ratio_warn = 100`, `poor_list_cap = 10`. An element is an offender when its value is strictly beyond the limit.
Surface: the report is returned unchanged in structure by `mesh.check` (service, MCP `check_mesh`, workflow task), by the
run-control `--check-only` event and by `rcfd check-mesh --json`; `rcfd check-mesh` text prints the new summary and the
`poor_warnings` lines. GUI display is not attempted.

### A3 Closed forms (derivations)

All lengths dimensionless. Cell faces are given as listed; angle values in degrees.

* **Unit cube** (single cell, or any cell of a unit-cube grid): every face is a unit square; `x_f - x_c` is parallel to `S_f`; all
  cosines 1. `warp = 0`, face angles 90 and 90, ortho-skew 0, inverse OQ 0, size change 1, face edge ratio 1.
* **Right-corner tetrahedron** `O=(0,0,0), A=(1,0,0), B=(0,1,0), C=(0,0,1)` (single cell). `x_c = (1/4,1/4,1/4)`. Face `OBC`
  (`x=0`): `S = (-1/2,0,0)`, `x_f = (0,1/3,1/3)`, `x_f - x_c = (-1/4, 1/12, 1/12)`, `|.|^2 = 11/144`, so
  `cos alpha = (1/4)/(sqrt(11)/12) = 3/sqrt(11)`; the same for `OAC` and `OAB` by symmetry. Face `ABC`: `S` parallel to
  `(1,1,1)`, `x_f = (1/3,1/3,1/3)`, `x_f - x_c = (1/12)(1,1,1)`: `cos alpha = 1`. Hence ortho-skew = inverse OQ
  `= 1 - 3/sqrt(11) = 0.09546532...` (boundary-only cell). The three right faces have angles 45, 45, 90; `ABC` has sides
  `sqrt 2` thrice: 60, 60, 60. Face angles min 45, max 90. Face edge ratios: right faces `sqrt2/1`, `ABC` 1; cell value `sqrt 2`.
  Warp 0 (triangles).
* **Regular tetrahedron** `(1,1,1), (1,-1,-1), (-1,1,-1), (-1,-1,1)`: centroid at the origin, every face equilateral
  (60, 60, 60), `x_f - x_c` parallel to `S_f` by symmetry: inverse OQ 0, ortho-skew 0, face edge ratio 1, warp 0.
* **Sheared hexahedron** (one cell, edge vectors `(1,0,0), (s,1,0), (0,0,1)`, `s > 0`): faces normal to `z` are parallelograms
  with angles `acos(s/sqrt(1+s^2))` and 180 minus that; the other four faces are rectangles (90). `s = 1`: 45 and 135.
  `s = 2`: `acos(2/sqrt5) = 26.565051...` and `153.434948...`. Centroid `(1/2 + s/2, 1/2, 1/2)`. Face `y = 0`: `S = (0,-1,0)`,
  `x_f - x_c = (-s/2, -1/2, 0)`, `cos alpha = (1/2)/sqrt((s^2+1)/4) = 1/sqrt(1+s^2)`. Face on the `x=0` side (spanned by
  `(s,1,0)` and `(0,0,1)`): `S = (-1, s, 0)`, `x_f - x_c = (-s/2, 0, 0)`, `cos alpha = (s/2)/(sqrt(1+s^2) s/2) = 1/sqrt(1+s^2)`; the
  `z` faces give 1. So `OQ = 1/sqrt(1+s^2)`: `s=1`: `1 - 1/sqrt2 = 0.29289322...`; `s=2`: `1 - 1/sqrt5 = 0.55278640...`.
  Face edge ratios: parallelograms and the slanted rectangles `sqrt(1+s^2)`; cell value `sqrt 2` for `s = 1`. Warp 0.
* **Two-cell stack, shifted** (unit cubes stacked in `y`; the second cell, and the shared face, shifted by `0.5` in `x`, the top
  of cell 1 by 1.0; this is `box_mesh([1,2,1],[1,2,1])` with points `y > 0.5` shifted by 0.5 and `y > 1.5` shifted by 1.0 in
  `x`): `x_{c0} = (0.75,0.5,0.5)`, `x_{c1} = (1.25,1.5,0.5)`, shared face centre `(1.0,1.0,0.5)` on the segment between them
  (skewness 0). Every face of both cells has `cos = 2/sqrt5`: shared face `beta`: `d = (0.5,1,0)`, `cos = 1/sqrt(1.25)`;
  `alpha` of the shared face: `(0.25,0.5,0)` same direction; the slanted side faces `S = (-1, 0.5, 0)` with `v = (-0.5,0,0)`:
  `0.5/(sqrt1.25 * 0.5)`; `z` faces 1. Inverse OQ = ortho-skew = `1 - 2/sqrt5 = 0.10557281...` for both cells. Size change 1.
* **Two-cell, neighbour-only skew** (cell 0 = unit cube `[0,1]^3`; cell 1 = hexahedron on the shared face `x = 1` with edge
  vectors `(1,1,0), (0,1,0), (0,0,1)` from the corner `(1,0,0)`, i.e. its `x = 2` face is the unit square at `y in [1,2]`):
  `x_{c1} = (1.5,1,0.5)`, `d = (1,0.5,0)`. Cell 0: all `alpha` cosines 1 (it is a cube) but the shared face has
  `cos beta = 1/sqrt(1.25) = 2/sqrt5`: ortho-skew 0, inverse OQ `1 - 2/sqrt5`. Cell 1: this is the sheared hexahedron with
  `s = 1` (shear in `y` instead of `x`): all slanted-face `alpha` cosines `1/sqrt2` (the shared face: `S = (-1,0,0)`,
  `x_f - x_c = (-0.5,-0.5,0)` gives `1/sqrt2`), `beta` for the shared face is `2/sqrt5`: `OQ = 1/sqrt2`; ortho-skew and
  inverse OQ both `1 - 1/sqrt2`. Face angles: cell 0: 90/90; cell 1: 45/135. Cell 1's face edge ratio: parallelogram sides 1
  and `sqrt2`: `sqrt 2`.
* **Warped hexahedron**: unit cube with the vertex `(0,1,1)` raised to `(0,1,1+h)`. Only the top face `(0,0,1),(1,0,1),(1,1,1),
  (0,1,1+h)` is non-planar (the other faces containing the moved vertex lie in the planes `x = 0` and `y = 1`). With
  `P0..P3` in that order and the raised vertex relative to the plane as `(0,1,h)`: split `P0P2`: normals
  `(1,0,0)x(1,1,0) = (0,0,1)` and `(1,1,0)x(0,1,h) = (h,-h,1)`: `cos = 1/sqrt(1+2h^2)`. Split `P1P3`: normals
  `(1,0,0)x(0,1,h) = (0,-h,1)` and `(0,1,0)x(-1,1,h) = (h,0,1)`: `cos = 1/(1+h^2)`, which is smaller (`(1+h^2)^2 > 1+2h^2`), so
  the max is `acos(1/(1+h^2))`: `h = 0.5`: `acos(0.8) = 36.86989765` deg; `h = 1`: `acos(0.5) = 60` deg (the other split gives
  `acos(1/sqrt3) = 54.7356` deg, so the choice of the larger angle is visible). Cell warp equals the top-face value; the other
  five faces have warp 0.
* **Size change**: `box_mesh([2,1,1],[1,1,1],[Expansion(4),Uniform,Uniform])` has cell widths `0.2, 0.8`: cell 0 size change 4,
  cell 1 size change 0.25.
* **Sliver (degenerate, valid) hexahedron**: `box_mesh([1,1,1],[1,1,0.001])`: top and bottom faces unit squares (edge ratio 1);
  four side faces `1 x 0.001`: edge ratio 1000; cell value 1000; all angles 90; inverse OQ 0 (collinear centroid and face centres).
* **Collapsed quad** `(0,0,0),(1,0,0),(1,0,0),(0,1,0)` (pure polygon functions, not a valid mesh face): edge lengths `1, 0, sqrt2,
  1`: edge ratio infinity; after removing the coincident vertex the polygon is the triangle `(0,0,0),(1,0,0),(0,1,0)`: angles
  90, 45, 45, so min 45, max 90, warp 0.
* **Collinear triangle** `(0,0,0),(1,0,0),(2,0,0)`: zero area; unsigned angles 0, 180, 0: min 0, max 180; edge lengths 1, 1, 2:
  ratio 2; warp 0.
* **Zero-volume cell** (pure orthogonality function; face centres coincide with the cell centre): all cosines 0, so inverse
  OQ = 1 and ortho-skew = 1 exactly.

### A4 Registered checks (F-233), tests in `crates/rcfd-mesh/tests/measures.rs`, helpers via `box_mesh`, `gen::warp` and
`Mesh::from_cell_faces`

Tolerances: dimensionless values and ratios absolute `1e-12`; angles `1e-9` degrees; ratio of 1000 relative `1e-9`.
* **Q1a** unit cube (single cell and the 2x2x2 grid, every cell): the unit-cube values above.
* **Q1b** right-corner tetrahedron: face angle min 45, max 90, ortho-skew = inverse OQ = `1 - 3/sqrt(11)`, max face edge ratio
  `sqrt2`, warp 0, size change 1.
* **Q1c** regular tetrahedron: face angles 60/60, inverse OQ 0, ortho-skew 0, edge ratio 1, warp 0.
* **Q1d** sheared hexahedron `s = 1` and `s = 2`: face angles, ortho-skew = inverse OQ, edge ratio `sqrt(1+s^2)`, warp 0.
* **Q1e** two-cell shifted stack: both cells inverse OQ = ortho-skew = `1 - 2/sqrt5`, size change 1.
* **Q1f** neighbour-only skew pair: cell 0 ortho-skew 0 and inverse OQ `1 - 2/sqrt5`; cell 1 both `1 - 1/sqrt2`, face angles
  45/135.
* **Q1g** warped hexahedron `h = 0.5` (`36.86989765`) and `h = 1` (`60`): top-face warp, cell warp, the five other faces
  `<= 1e-12`; and the pure polygon function with the two split angles (`54.7356103` and `60`) to show max-of-both.
* **Q1h** size change `4` and `0.25`; the maximum over cells equals the existing `max_volume_ratio` of `check_mesh` (1e-12).
* **Q1i** invariance: on the sheared stack (Q1e) a translation, a rotation about an arbitrary axis (non-axis-aligned) and a
  uniform scaling by 3.7 leave every new measure unchanged to `1e-9` relative (absolute `1e-9` for values below 1).
* **Q2a** sliver hexahedron: face edge ratio 1000 (relative `1e-9`), face angles 90/90 (`1e-9`), inverse OQ `<= 1e-12`; the mesh
  passes `validate`.
* **Q2b** collapsed quad: edge ratio is `f64::INFINITY`; angles min 45, max 90 (`1e-9`); warp `<= 1e-12`.
* **Q2c** collinear triangle: angles min 0, max 180 (`1e-9`), edge ratio 2, warp `<= 1e-12`.
* **Q2d** zero-volume cell: inverse OQ == 1 and ortho-skew == 1 exactly.
* **Q2e** no NaN anywhere in any measure array for Q1/Q2 inputs; the serialized report of the sliver parses back equal.
* **Q3 poor-mesh list.** (P1) sliver with default limits: `poor_counts["edge_length_ratio"] = 4`, the 4 entries are side faces
  with value `1000 (1e-9)` and limit 100, and no other measure has offenders. (P2) the shifted stack (Q1e) and the sheared
  hex with default limits: `poor_list` empty, `poor_warnings` empty. (P3) the neighbour-only pair with
  `inverse_orthogonal_warn = ortho_skew_warn = 0.25`: exactly one entry per measure, cell 1, values `1 - 1/sqrt2` (`1e-12`).
  (P4) cap and order: `box_mesh([2,2,2],[4,2,2])` with `edge_length_ratio_warn = 1.5`: count 24 (the 12 y-normal and 12 z-normal
  faces, each 2x1), list length 10 (cap), values all 2. (P5) a two-cell graded mesh (`Expansion(500)`, widths ratio 500) with
  default limits has `size_change` offender cell 0 with value 500 (the old `warnings` carry the volume-ratio line too).
* **Q4 serialization**: the JSON of the new report round-trips equal; the JSON of the old schema (only the old keys, produced by
  removing the new keys) parses with empty new fields.
* **Q5 existing measures unchanged**: the four existing unit tests in `quality.rs` pass untouched; for the stretched two-cell
  mesh of the existing test the old `warnings` still has exactly 2 entries; `compute_metrics` is not edited. API:
  `mesh.check` report JSON contains the old keys with the same values and the new keys (`rcfd-api` test); CLI `check-mesh
  --json` contains `measures` and `poor_list`; CLI text output mentions `warp` (`rcfd-cli` test).
* **H1** `crates/rcfd-api/tests/baseline_hashes.rs` is not edited and passes (bit-identical hashes) before and after the
  change; the quality computation is read-only and no solver path calls the new code.

Acceptance statement of F-233 ("all measures on a reference tetrahedron, hexahedron and a degenerate cell match closed-form
values") is met only if Q1b, Q1d, Q1g, Q2a, Q2b, Q2c, Q2d pass.

## Part B: F-231, mesh editing and repair (library level, `rcfd-mesh/src/edit.rs`)

### B1 Scope decision, stated up front

Implemented: affine transforms (translate, rotate, scale, mirror) with handedness repair of mirrors; merge of two meshes with
stitching of coincident boundary faces and copy by offset; extrusion of a boundary patch by a vector or along the patch normal
(planar patches); repair of face orientation (inverted loops, whole-mesh inversion) on a raw topology; merging boundary patches
and separating a patch into connected components. NOT attempted: slit (baffle creation on internal faces), merging or
separating CELL zones, face-zone carry through merge/extrude (dropped and reported), non-planar `Normal` extrusion exactness,
repair of `owner > neighbour` ordering and of face order (reported as an error), a CLI or case-tree entry point, GUI.
F-231 is therefore `partial` whatever the numbers.

### B2 Definitions

* `Affine { m: [[f64;3];3], t: Vec3 }`, `x' = M x + t`; constructors `translation`, `rotation(axis, angle, origin)` (Rodrigues),
  `scaling([sx,sy,sz])` about the origin, `mirror(normal, point)`, `then(other)` composition. `transform_mesh(&Mesh, &Affine)`
  maps the points; if `det M < 0` every face loop is reversed (so owner-to-neighbour orientation is kept and volumes stay
  positive); topology, patches, zones unchanged. Zero `det` is an error. Area vectors transform with the cofactor matrix
  `det(M) M^{-T}`; volumes with `|det M|`.
* `merge_meshes(a, b, tol)`: concatenates the cells (a first), merges the points of `b` onto points of `a` closer than
  `tol` (absolute; `b`'s own points are never merged with each other), builds the cell-face lists and calls
  `Mesh::from_cell_faces`. Boundary faces of either mesh whose point set is shared by two cells become internal and leave their
  patch. Patches of equal name are combined (kind from `a`); cell zones of equal name are combined (cell ids of `b` offset);
  face zones are dropped. `copy_by_offset(&Mesh, n_copies, offset, tol)`: `n_copies` additional copies translated by
  `k * offset`, merged in sequence.
* `extrude_patch(&Mesh, patch, dir, layers, tol)` with `dir` either `Vector(v)` (total displacement `v`) or `Normal(d)` (total
  distance `d` along per-point averaged outward patch normals, exact only for planar patches). Each patch face becomes `layers`
  prisms/hexes stacked outward; the original patch is removed from the boundary list and replaced by a patch named like the
  original (the far cap) plus `<name>_side` (the new lateral faces) of kind `Generic`; points of the layer are shared across
  faces. The mesh is rebuilt with `from_cell_faces` (existing patch faces other than the extruded one keep their patch).
* `repair_face_orientation(&mut MeshTopology) -> RepairReport`: for every face, with the owner-cell vertex average `x_o` (average
  of all points of the cell's faces, as `from_cell_faces`), a loop whose area vector `S` has `S . (x_f - x_o) < 0` is reversed.
  The rule is per face, so a fully inverted mesh simply flips every face. The report counts flipped faces. `repair_mesh(topology)` calls it and then `Mesh::from_topology`.
  Owner/neighbour order and face order are not repaired.
* `merge_patches(&Mesh, names, new_name)` and `separate_patch(&Mesh, name)` reorder the boundary faces and remap face zones;
  geometry, cell order and internal faces unchanged; periodic patches refused with a located error. `separate_patch` splits
  a patch into face-edge-connected components named `<name>_0, <name>_1, ...` in order of the lowest face id.

### B3 Registered checks (F-231), tests in `crates/rcfd-mesh/tests/edit.rs`

Base meshes: `box_mesh([4,3,2],[2,1.5,1],[Expansion(3),Uniform,Uniform])` (graded, so cells differ), warped by
`gen::warp(x + 0.1 sin(2 pi y), y, z + 0.05 x y)` (a non-affine map so that faces are not all axis aligned),
`cylinder_ogrid(16, 4, 1, 2, Uniform, 1)` (prisms/hexes with curved geometry), `box_mesh([2,2,2],[2,2,2])`.
* **E1 transforms** (each base mesh; total volume, every cell volume, every face area vector magnitude and every patch area):
  translation, rotation by 0.7 rad about axis `(1,2,3)` through `(0.3,-0.2,0.5)`, uniform scale 2.5, anisotropic scale
  `(2, 0.5, 3)`, mirror `x -> -x`, mirror through a non-axis plane, and a composition of rotation, scaling and translation.
  Limits: cell volume ratio `V'_c / V_c = |det M|` and the total, relative `1e-12`; face area vectors equal `det(M) M^{-T} S`
  to relative `1e-12` of `|S|`; patch areas for rigid maps equal the original (rel `1e-12`); closed form on the plain box: the
  area of patch `xmin` after anisotropic scaling is `ly lz sy sz = 1.5 * 1 * 0.5 * 3`; mirrored meshes pass `Mesh::validate`
  and have positive volumes, loops reversed (`face_points` of every face are the reverse of the original) and equal
  owner/neighbour/patches. The base-mesh non-orthogonality and aspect ratio are unchanged by rigid and uniform maps
  (`1e-9` absolute / relative).
* **E2 merge and copy**: (a) `box_mesh([2,2,2],[1,1,1])` and a copy offset `(1,0,0)`, `tol = 1e-9`: result has 16 cells,
  28 internal faces, 40 boundary faces, total volume 2 (`1e-12`), patch set `xmin, xmax, ymin, ymax, zmin, zmax` with areas
  `xmin = xmax = 1`, `ymin = ymax = zmin = zmax = 2`; passes `check_mesh`; cell count, internal-face count and volume equal
  those of `box_mesh([4,2,2],[2,1,1])`. (b) offset `(1.5,0,0)` (gap): 16 cells, internal faces 2 * 12 = 24,
  volume 2, 48 boundary faces, patches `xmin` and `xmax` have 8 faces each (equal names are combined). (c) `copy_by_offset(unit box 2x2x2, 3, (1,0,0))`:
  32 cells, volume 4, `check_mesh` ok, internal faces equal `box_mesh([8,2,2],[4,1,1])`'s count. (d) cell zone of equal
  names combine with offset ids.
* **E3 extrude**: patch `zmax` of `box_mesh([3,2,1],[3,2,1])` extruded by vector `(0,0,2)` in 4 layers: cells `6 + 6*4 = 30`,
  total volume `6 + 12 = 18` (`1e-12`), cap patch `zmax` area 6, patch `zmax_side` area `2*(3+2)*2 = 20` (`1e-12`), passes
  `check_mesh`. `Normal(2)` on the same planar patch equals the vector result (point coordinates within `1e-14`).
  A slanted vector `(1,0,2)`: volume `6 + 6*2 = 18` (shear preserves volume), valid mesh.
* **E4 orientation repair**: for each base mesh, (a) invert every fifth face loop (face ids `f % 5 == 0`), (b) invert all faces,
  (c) invert a single boundary face. Positive control: `Mesh::from_topology` rejects every corrupted topology (asserted).
  After `repair_mesh`: Ok; `check_mesh(...).is_ok()`; flipped count equals the number of corrupted faces exactly; the repaired
  mesh `content_hash()` equals the original's (reversal is an involution). An already-correct topology reports 0 flips and
  returns an identical topology. A topology with `owner > neighbour` is reported as an error, not repaired.
* **E5 patch operations**: `merge_patches([ymin, ymax] -> ywalls)`: face count and area are the sums (`1e-12`), cell volumes
  bit-identical, face zone entries still point at faces with identical centres, a periodic patch is refused. `separate_patch`
  of the merged `ywalls` gives two patches with the original areas; of a two-box (gapped) merge `zmin` gives two patches of
  area 1 each. Round trip merge then separate preserves total boundary area and boundary face count.
* **H1 / H2 hygiene**: as Part A, plus all existing tests unchanged.

Acceptance of F-231 as written ("analytic transforms keep volumes and areas exact; repaired inverted-face test meshes pass the
quality check") is met by E1 and E4; the remaining title items not attempted keep the status `partial`.

## Hygiene

* H3 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release
  --no-fail-fast` on a `git archive` copy of the final commit, `CARGO_TARGET_DIR=<target dir>`.
* H4 every tuning and deviation disclosed in "Results"; unverifiable references marked in `docs/reference-data.md`.
* H5 roadmap statuses stay `partial` unless the whole acceptance text and title of the item is met.

## Results

Measured on the final code (2026-10). Limits are the registered ones above; none was changed.

### F-233 (`rcfd-mesh/tests/measures.rs`)

* Q1a, Q1c, Q1e, Q1f, Q1h, Q2a, Q2b, Q2c, Q2d (corrected), Q2e, Q3 (P1 to P5), Q4: pass at the registered limits.
* Q1b right-corner tetrahedron: ortho-skew = inverse OQ 0.09546596626670922 against `1 - 3/sqrt(11)` = 0.09546596626670911
  (difference 1.1e-16; limit 1e-12). Correction of a typo: the decimal expansion printed in A3 ("0.09546532...") is wrong in the
  fifth digit; the registered formula `1 - 3/sqrt(11)` is what the test uses and the right value is 0.095465966...
* Q1d sheared hexahedron: `s = 2` angles 26.565051177077994 / 153.434948822922 (exact 26.565051177078, 153.434948822922), ortho-skew
  0.5527864045000422 (exact 0.5527864045000421).
* Q1g warped hexahedron: `h = 0.5` warp 36.86989764584404 deg (exact 36.86989764584401), `h = 1` 59.99999999999999 (exact 60); only the top
  face is non-zero (asserted); polygon function gives 60 deg for the `h = 1` quad, the other split gives 54.7356 deg.
* Q1i invariance to 1e-9 for translation, rotation, scaling: pass (after the fix below).
* Q5: the four existing `quality.rs` unit tests pass untouched (the stretched mesh still gives exactly 2 old warnings);
  `mesh.check` and `rcfd check-mesh` carry the old keys and the new ones (`rcfd-api/tests/mesh_quality.rs`,
  `rcfd-cli/tests/cli.rs::check_mesh_reports_extended_measures`).
* H1: `baseline_hashes` passes unmodified (3 passed, the pre-existing ignored rotating-channel test unchanged).
* Acceptance mapping, checked cell by cell after the run (reviewer note): tetrahedron, all six new measures asserted on one cell (Q1b);
  hexahedron, angles, ortho-skew, inverse OQ, edge ratio, warp on the sheared cell (Q1d), warp on warped cells (Q1g), size change on
  unit-cube and graded hex cells (Q1a, Q1h); degenerate cell: only the sliver hexahedron (Q2a) is an actual cell, and it asserts edge
  ratio, angles and inverse OQ only. Warp, ortho-skew and size change are not asserted on a degenerate CELL (Q2b, Q2c are bare
  polygons, Q2d a hand-built `CellFace` list, and size change of a zero-volume cell is not defined by any check); the four
  existing measures are not asserted on the tetrahedron either. The roadmap sentence says "all measures ... a degenerate cell", so
  F-233 is `partial`, not `done`, and the missing pairs are named in the roadmap note. No check was added after the run to close the gap.

Defects found by the first run (disclosed, limits untouched):
1. Warp of planar faces came out 1.2e-6 degrees against the registered limit 1e-12 (Q1d, Q1i). Root cause: the angle between
   triangle normals was computed as `acos(cos)`, whose conditioning is 1/sin(angle): rounding noise 1e-16 in the cosine becomes
   1e-8 rad. This was a defect of the new code, not of the criterion; `angle_between` now uses `atan2(|a x b|, a . b)`. No
   limit was changed. (The same conditioning affects the EXISTING non-orthogonality metric, see E1 below, which is not edited.)
2. Q2d first version had a set-up defect: the neighbour centre was put on the inward side of the outward area vector (`cos beta`
   = -1, inverse OQ 2). The old test is kept as an ignored record (`q2d_zero_volume_cell`), the corrected test is
   `q2d_zero_volume_cell_corrected` (same registered limit, exact 1).
3. Q1h was coded with relative 1e-9 on 4 and 0.25, looser than the registered absolute 1e-12 for dimensionless ratios. The old test
   `q1h_size_change` is kept as a record; `q1h_size_change_registered_tolerance` asserts the registered absolute 1e-12 and passes.
   Other tolerances were re-scanned against A4: E1 normalises the area-vector error by `|S'|` (the image) where B3 says `|S|`
   (up to 6.25 times looser at the 2.5 scale; the measured worst error is 2.7e-15 against 1e-12, so the difference is immaterial but
   is disclosed); the others match.
4. The first `rcfd-api` mesh-check test had a wrong cell-size arithmetic (aspect 50 instead of the intended > 100); the test
   input was changed (`ly = 0.002`). This test only checks presence of keys and a poor-list entry, not a registered number.

### F-231 (`rcfd-mesh/tests/edit.rs`)

* E1 transforms (3 base meshes x 7 maps = 21 runs): worst cell volume ratio error 2.7e-15 (limit 1e-12), worst face area vector error
  2.7e-15 (limit 1e-12), patch areas, closed-form patch areas after anisotropic scaling (2.25, 12, 3), mirrors valid with reversed
  loops and unchanged owner/neighbour/patches, aspect ratio and the new inverse orthogonal quality unchanged to 1e-9: pass.
* E1 non-orthogonality invariance to 1e-9: MISS, kept as `e1_non_orthogonality_invariance_known_miss` (ignored, limit intact). Worst
  change 1.48e-6 degrees (for example 8.5e-7 degrees against exactly 0 for the same orthogonal face after a translation). Cause: the
  existing `compute_metrics` uses `acos(cos)` (conditioning 1/sin of the angle); the roadmap note says existing measures are
  unchanged, so it was not edited. `e1_non_orthogonality_noise_diagnosis` only prints the worst change (1.479e-6 degrees); an
  earlier version asserted `< 1e-5`, which was a limit chosen after seeing the number and was removed so that it cannot be read as a
  replacement criterion.
* E2 merge and copy: (a) 16 cells / 28 internal / 40 boundary, volume 2, patch areas 1, 1, 2, 2, 2, 2, equals the `box_mesh([4,2,2])`
  counts, `check_mesh` ok; (b) gap 16 / 24 / 48, `xmin`, `xmax` 8 faces each; (c) 32 cells, volume 4, internal faces equal the
  8x2x2 box; (d) zone `[0, 1, 8]`: pass.
* E3 extrude: 30 cells, volume 18, cap area 6, side area 20, `Normal(2)` equals the vector extrusion to 1e-14, slanted volume 18,
  inward vector refused: pass.
* E4 repair on all three bases for "every fifth face", "all faces", "one boundary face": rejected before (positive control), repaired
  mesh `content_hash` equals the original, flip counts equal the corrupted counts, `check_mesh` ok; already-correct topology 0 flips;
  swapped owner/neighbour stays an error: pass.
* E5 patch operations: merged `ywalls` area is the sum, cell volumes bit-identical (the summation order of faces per cell did not
  change the result), face-zone centres identical, separation returns the original areas, gapped two-box `zmin` separates into two
  patches of area 1, connected patch unchanged, periodic patches refused: pass.
* Acceptance sentence ("analytic transforms keep volumes and areas exact; repaired inverted-face test meshes pass the quality check"):
  met (E1, E4). F-231 stays `partial`: slit, cell-zone merge and separate, face zones through merge and extrude, ordering repair
  and any CLI, case-tree or GUI entry were not attempted (B1).

### Deviations and disclosures

* Code change after a registered miss: `angle_between` in `measures.rs` was changed from `acos` to `atan2(|a x b|, a . b)` after the
  first run missed the warp limit of 1e-12 (measured 1.2e-6 degrees on planar faces); root cause is the conditioning of `acos`, see
  defect 1. No limit changed.
* H1 was run after the change only (3 passed, 1 pre-existing ignored); it was not run on the previous HEAD first. The registered hashes
  are fixed constants inside the unmodified test file, so the after-run is still valid evidence, and no solver path calls the new code.
* B2 lists a `tol` argument for `extrude_patch`; the implemented signature is `extrude_patch(mesh, patch, dir, layers)` without it
  (points are created, not matched).
* Helper scripts used for editing and the `git archive` copy were placed in the system temp folder, not the scratchpad.

### Unverified references (see `docs/reference-data.md`)

All literature attributions in A1 (warpage idea, orthogonal quality cosines, default limits) are from memory and unverified;
none of the registered checks uses them.
