# ADR 0260: F-356 registration: the donor search of the overset coupling on non-convex cells with planar faces

Status: **REGISTERED 2026-11-12**, written before any code of F-356 and before any run of its checks. Not yet accepted by
the maintainer. Before this text no Rust code was compiled or run for F-356: the code and the documents listed under
"What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand and with four
small Python scripts on the planar geometry of the new set-ups (kept in `E:/projects/cfd/w49i-work/f356-scratch`:
`dart_precheck.py`, `dart_scan.py`, `dart_sub.py`, `p2_precheck.py`; they compute polygons in the lattice plane and do not
touch the repository's code). Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0253](0253-f352-registration-overset-donor-search-on-non-planar-faces.md) (F-352: the plane test kept for
flat cells, the triangulated test with the winding number for cells with a warped face, the tie-break, question Q1 not
taken, question Q2 whose recommendation is this item, the checks A4, S1 to S3, T1 to T3, the test file
`crates/rcfd-mesh/tests/overset_f352.rs`) and [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md)
(F-134: the donor search M3). Roadmap item F-356 (depends on F-352).

Number: the lead gave 0260 to F-356. Branch `f356-donor-search-nonconvex` from main 2e0eefd0, worktree `w49i-work/f356`.

House rules (ADR 0130, ADR 0213, ADR 0219, ADR 0253 and the campaign rules of 2026-11-12):

- limits are fixed here, before the code and the runs, and are not changed after a run; the limits of F-134, F-336 and
  F-352 are not changed, their tests and ignored records are not edited;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- existing results stay bit-identical; if the change moved a donor or a stencil on a mesh of convex cells with planar
  faces the work stops and the change is reported, not accepted;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. The method is derived in this text from first principles. Named from general knowledge, from memory (no
paper, book, code or vendor document was opened for this work): the characterisation of a convex polyhedron as the one
whose every vertex lies on the inner side of the plane of every face (elementary convex geometry); ear clipping and the
"two ears" theorem for simple polygons (G. H. Meisters, "Polygons have ears", Amer. Math. Monthly 82 (1975) 648-651),
the argument that is used being derived below; the kernel of a polygon (the set of points from which the whole polygon is
visible), a standard notion of computational geometry. The winding number and the solid angle of a triangle are those of
ADR 0253 G1, G2 (derived there).

## What F-356 asks

The roadmap entry (acceptance, quoted, binding): "On a mesh of non-convex polyhedral cells with planar faces every one of
100000 random points inside the mesh lies in exactly one cell of the locator and the located cell agrees with a full
search and with an independent containment test; on meshes of convex cells with planar faces the donors and stencils are
bit-identical to those of today (the checks A4 of ADR 0253)."

**Reading (registered here).**

1. "A mesh of non-convex polyhedral cells with planar faces": two set-ups, P1 and P2 below, built in the new test file,
   every face planar to round-off (checked: no warped cell), most cells non-convex (P1: 1125 of 1280 cells; P2: 288 of
   864, the others convex prisms between them). P1 has non-convex faces whose fan about the face centre stays in the face;
   P2 has non-convex faces whose fan about the face centre leaves the face (the case raised in the brief).
2. "Every one of 100000 random points inside the mesh lies in exactly one cell of the locator": for 100000 points in each
   of P1 and P2, `CellLocator::containing_cells` (every cell of the range that accepts the point, by a search over all of
   them) has exactly one element.
3. "Agrees with a full search": `locate` (through the bins) equals `locate_brute_force` for every point.
4. "And with an independent containment test": the located cell contains the point by a reference written in the test
   file that shares no code with the locator and does not use the mesh's geometry: the point's pre-image in the lattice of
   the construction, tested against a decomposition of the cell's base polygon into triangles known from the construction
   and against the cell's layer.
5. "On meshes of convex cells with planar faces the donors and stencils are bit-identical to those of today (the checks
   A4 of ADR 0253)": the test file `overset_f352.rs` (its four tests, which hold A4b to A4d, and the A, S and T checks of
   B3d), the unit tests K1 to K5 of `rcfd_mesh::overset`, `crates/rcfd-mesh/tests/overset_f336.rs` and the overset test
   files of `rcfd-fv` pass unedited; and, stronger, no cell of a registered convex set-up is reclassified (count zero, the
   largest outward distance of a vertex from a face plane printed over the tolerance), so the code path of every one of
   their cells is the present one, textually. The warped set-up B3d of ADR 0253 is checked too: no cell or face of it
   takes a new path, and its donors equal those of the F-352 rule written out in the new test file.

## What was read (no run)

- ADR 0253 in full; of ADR 0219 the model M1 to M4 and the checks K1 to K5.
- `crates/rcfd-mesh/src/overset.rs` in full; `crates/rcfd-mesh/tests/overset_f352.rs` in full (the set-ups, the rule of
  M3 written from the public parts, the reference by tetrahedra about the cell centre, the checks A4b to A4d);
  `crates/rcfd-mesh/src/mesh.rs`: `polygon_geometry`, `from_face_lists`, `from_cell_faces`, `compute_geometry`,
  `validate`; the head of `crates/rcfd-mesh/src/polydual.rs`; where `Overset` and `CellLocator` are used outside their
  module (only the overset solver of `rcfd-fv` and tests; no case of `rcfd-api` builds an overset).

What the code read shows (in addition to ADR 0253 "What the code read shows"):

1. `CellLocator::new` flags a cell *warped* when one of its faces has `face_warp > tol_c` (`tol_c = 1e-10` of the
   diagonal of the cell's box); a warped cell is tested by `triangulated_margin` after a box shortcut, every other cell
   by `containment_margin` (the planes of M3). Nothing else distinguishes cells.
2. `triangulated_margin` uses for every face the fan of triangles `(fc_f, p_i, p_{i+1})` about the face centre.
3. `polygon_geometry` (for more than three points) takes the face centre as the mean of the centroids of the triangles
   `(p_i, p_{i+1}, e)` about the vertex mean `e`, weighted by their **unsigned** areas, and the area vector as half the sum
   of their cross products (the vector area, independent of `e`). For a planar face the centre is a positive combination
   of points of the face's plane, so it lies in that plane; for a non-convex face it need not lie in the face.
4. `Mesh::validate` requires at most one face for each pair of cells (internal faces ordered by `(owner, neighbour)`
   without duplicates), closed cells (every directed edge matched by its reverse once), positive volumes, and for every
   internal face `S_f . (cc_neighbour - cc_owner) > 0`. So two cells meet in at most one face, and the face must be
   planar for the mesh to have planar faces: a test mesh of non-convex cells must be a tiling in which neighbours meet
   along one planar polygon each. `from_cell_faces` orients faces by the vertex mean of the cell, which a non-convex cell
   need not contain; the new set-ups are built with `from_face_lists`, oriented by construction.
5. `polydual::tet_to_polydual`: the dual faces are rings through tetrahedron centroids, in general not planar; a dual
   mesh is a warped mesh (F-352's case), not this item's.

## Why the plane test is exact for convex cells and wrong for non-convex ones (derivation)

Let `P` be a cell with planar faces `F`, each in its plane `E_F` through `fc_F` with outward unit normal `n_F`, and `H_F
= { x : n_F . (x - fc_F) <= 0 }` the inner half-space. The plane test of M3 accepts `x` when `x` lies in `H = cap_F H_F`
(to the tolerance); its margin `min_F ( -n_F . (x - fc_F) )` is the smallest distance from the planes.

*Convex.* A plane that contains a two-dimensional piece of the boundary of a convex body is a supporting plane, so `P` is
in every `H_F` and `P` is in `H`. Conversely a point `x` outside `P` is separated from `P` by a plane, and for a convex
polytope that plane can be taken as the plane of a face that `x` sees from outside, so `x` is outside some `H_F`. So `P =
H`: the test is exact, and for a point inside, the smallest distance from the face planes is the distance from the
boundary (the nearest boundary point lies on a face, and the face's plane is no farther than the face).

*Non-convex.* If `P` is not convex, some face plane cuts `P`: by the characterisation below there is a vertex `v` of `P`
and a face `F` with `n_F . (v - fc_F) > 0`. The points of `P` near `v` are then outside `H_F`, so the plane test refuses
points of its own cell, a region of positive volume. No other cell takes them in general: a neighbour's plane region
`H'` is the intersection of its own half-spaces, which (by the same argument applied to it) is a subset of the neighbour
if the neighbour is convex, and a subset of the neighbour's kernel otherwise. The plane region of every cell is the
cell's **kernel** (the points from which the whole cell is visible, which is the intersection of the inner half-spaces
of its faces), the kernels of different cells are disjoint, and `P` minus its kernel is in no cell: a gap.

*The smallest example.* The smallest non-convex polygon is a quadrilateral with one reflex vertex (a dart); the prism on
it is a non-convex hexahedron with planar faces. The dart of set-up P1 (before the scaling by `h` and the affine map)
has the points `LL = (0.4, 0.4)` (reflex), `LR = (0.6, -0.4)`, `UR = (1.4, 0.6)`, `UL = (-0.4, 1.4)`, area 1. The point
`x = (0.6, -0.2)` lies in its triangle `(LL, LR, UR)` (the three edge tests give `0.04`, `0.16`, `0.64`, all positive),
so in the dart; but for the edge `UL -> LL` the test gives `(0.8, -1.0) x (1.0, -1.6) = -0.28 < 0`: `x` is on the outer
side of the plane of the side face on that edge. The plane test refuses `x` for its own cell. Of the area of the darts of
P1, `0.265` lies outside their kernels (Monte Carlo, `p2_precheck.py`).

*The characterisation used for the flag.* `P` (a closed, non-self-intersecting polyhedral surface with planar faces) is
convex if and only if every vertex `v` lies in every `H_F`. Only if: shown above (every face plane supports a convex
body). If: then the convex hull `C` of the vertices is in every `H_F`; every face `F` lies in `C` and in the plane `E_F`,
which supports `C`, so `F` is in the boundary of `C`; the boundary of `P` is a closed surface contained in the boundary of
`C`, which is a sphere, and a closed surface embedded in a sphere is the whole sphere, so the boundaries agree and `P = C`
is convex.

## Design

### N1. The flag: a cell is non-convex when one of its points is outside one of its face planes

`CellLocator::new` gives every cell of its range that is not warped a second flag:

**non-convex** when for some face `F` of the cell with `|S_F| >= 1e-300` and some point `q` of the cell (of any of its
faces) `sigma_F S_F . (p_q - fc_F) / |S_F| > tol_c`, with `sigma_F = +1` for the owner and `-1` for the neighbour (the
outward area vector) and `tol_c` the locator's tolerance of the cell (`1e-10` of its box diagonal, ADR 0219 M3).

A cell that is warped or non-convex is a **triangulated** cell: it is tested by the triangulated margin of N2 after the
box shortcut of ADR 0253 G2. Every other cell (flat and convex) is tested by `containment_margin` exactly as today.

*Cheap.* One dot product for each pair of a face and a point of the cell: 48 for a hexahedron, at construction only.

*Why a convex cell with planar faces is never flagged.* Its vertices lie in every `H_F` exactly (the characterisation),
so the computed outward distance of a vertex is zero or negative up to round-off. Round-off: the face centre and the
area vector are sums of a few products of coordinates, so the computed distance of a point lying in the face plane is
of the order of `eps max|x|`; with coordinates up to 35 (TR, ADR 0219) that is below `1e-14`; the smallest `tol_c` of the
registered convex set-ups is about `1e-11` (the cells of the two-dimensional meshes are `0.1` deep, so their box diagonal
is at least `0.1`). A factor of more than a thousand. Points of the cell not in the face plane are on the inner side by at
least the smallest height of the cell over that face, far more than `tol_c`. The check A4b' counts the reclassified cells
of every registered convex set-up (limit `0`) and prints the largest outward vertex distance over `tol_c` (estimate below
`1e-3`).

*Why the threshold is `tol_c`.* A cell whose vertices are at most `tol_c` outside its face planes has all its points
within `tol_c` of every `H_F` (its points are in the hull of its vertices), so the plane test with the tolerance accepts
all of them: it loses no point of its own cell, which is the failure derived above. Above `tol_c` it can lose points.
What such a nearly convex cell may accept outside itself is within the reach of the tolerance band; this is not measured
(limits).

*The flag of a warped cell* is not computed: a warped cell is triangulated already (ADR 0253), and nothing about it
changes except the rule N3 for faces whose fan folds, which no registered set-up has (counted).

### N2. The triangulated margin of a non-convex cell with planar faces: is the fan of ADR 0253 correct?

The triangulated margin of ADR 0253 G2 is `s d`, `d` the distance from the triangles of the cell, `s` the sign from the
winding number of the triangles. For a cell with planar faces whose fan triangles `(fc_F, p_i, p_{i+1})` are used:

*The winding number is exact, folded fan or not.* All triangles of a planar face lie in its plane. The signed solid angle
seen from `x` is additive over signed areas in a plane: the solid angle of the fan is the integral over the plane of the
fan's signed coverage (the sum of the orientations of the triangles covering each point) against the kernel `n . r /
|r|^3`. The signed coverage of a fan about any point of the plane equals the winding number of the face's boundary curve
about each point, which is `1` in the face and `0` outside, because the fan's boundary is that curve (the spokes cancel).
So the fan has the solid angle of the face itself, and the sum over the faces is `4 pi` times the winding number of the
true cell: `1` inside, `0` outside, for every `x` not in a face plane. Points in a face plane but off the triangles see
those triangles at zero solid angle, as they see the face.

*The distance is not, when the fan folds.* If the face is not star-shaped about `fc_F` (or `fc_F` is not in the face,
which happens: item 3 of the code read), some fan triangles are inverted and cover parts of the plane **outside the
face**, cancelled in the coverage by other triangles. The distance from the triangles is then smaller than the distance
from the cell's boundary near those parts, down to zero on them. A point of a neighbouring cell lying in the plane of
such a face, in a part covered by the fold, is at distance zero from the triangles of a cell that does not contain it:
that cell accepts it with the margin `-0`, which ties with the margin of the true cell on its face and can win the tie
(lowest number). Random points hit a plane with probability zero, but face centres and points of other meshes that lie
in the same planes (layered meshes) do not: in P2, 32 of the 36 non-convex base polygons have their face centre (of
`polygon_geometry`) outside the polygon (`p2_precheck.py`). So the fan must not be used for a face that folds.

*When the fan folds (decided per face, from the face alone).* The fan of face `F` **folds** when some triangle has
`((p_i - fc_F) x (p_{i+1} - fc_F)) . S_F <= 0`. If no triangle folds, the triangles have positive orientation, their
angles at `fc_F` sum to `2 pi` times the winding number of the face's boundary about `fc_F`, which is then positive, so
it is `1` and the angles do not overlap: every ray from `fc_F` meets the boundary once, the face is star-shaped about
`fc_F`, and the fan tiles the face (for a planar face) exactly. If some triangle folds, the fan covers points outside the
face or `fc_F` is not inside.

### N3. A folded face is triangulated by ear clipping, the same from both sides

A face whose fan folds is triangulated by **ear clipping** of its stored point loop, with the orientation test `o(a, b,
c) = ((p_b - p_a) x (p_c - p_a)) . S_F` (the polygon projected on the plane normal to `S_F`, positive for the stored
order):

1. the loop `v` is the stored points of the face; `eps = 1e-12 |S_F|^2` (`o` is twice the area of the triangle times
   `|S_F|`, so `eps` is `1e-12` of the face area in relative terms);
2. while `v` has more than three points: the first index `i` (from `0`) whose triangle `(v_{i-1}, v_i, v_{i+1})` has `o >
   eps` and contains no other point of `v` (a point `q` is contained when `o(v_{i-1}, v_i, q)`, `o(v_i, v_{i+1}, q)` and
   `o(v_{i+1}, v_{i-1}, q)` are all at least `-eps`; points with the same index as a corner are skipped) is an **ear**:
   the triangle is emitted and `v_i` removed;
3. the last three points are emitted when `o > eps` (a degenerate remainder of collinear points is dropped: its edges are
   edges of emitted triangles or of the neighbouring faces of the cell, so neither the distance nor the solid angle
   loses anything);
4. if no ear is found, the face is **unsupported**: its fan is kept and the face is counted (`CellLocator::n_unsupported`;
   limits).

*Correct.* For a simple polygon, a vertex with a positive turn whose triangle contains no other vertex is an ear: no edge
of the polygon enters the triangle (an edge cannot cross the two polygon edges at `v_i`, and a segment meets the line of
the third side once, so an edge entering through it would end inside, which is excluded), so the diagonal `v_{i-1}
v_{i+1}` is inside and cutting the ear leaves a simple polygon with one point fewer. By the two-ears theorem a simple
polygon with more than three vertices has an ear (with collinear vertices, a strictly convex one; collinear points block
an ear through the closed test, as they must). So the triangles tile the face exactly, with the stored orientation; for a
planar face, the distance from them is the distance from the face and the winding number is that of N2.

*The same from both sides.* The triangulation is a function of the face alone (its stored point order, its area vector),
computed once for each face of a triangulated cell; the neighbour uses the same triangles with the opposite sign of the
solid angle, as in ADR 0253. A face shared by a triangulated cell and a convex flat cell is a convex planar polygon (a face
of a convex polyhedron), so its fan does not fold and the triangulated cell sees exactly the face the plane test of the
other cell sees: both describe the true cells, which tile the mesh.

*The order of evaluation of a face whose fan does not fold is that of `triangulated_margin`, textually*: the same
triangles, in the same order, with the same functions, so a triangulated cell without a folded face (every cell of B3d)
returns the same margin bit for bit (check A4e). `triangulated_margin` itself is not changed (it stays the margin of
ADR 0253).

*A folded warped face* is ear-clipped too (the rule is per face); its ear triangles form a closed surface shared by its
two cells, but whether that surface bounds a non-self-intersecting cell is not shown. No registered set-up has a folded
warped face (counted on B3d). Limits.

### N4. Which cells tile, and the tie

With N1 to N3 every cell of a mesh of planar faces is tested on its true polyhedron: a convex flat cell by its planes (exact,
the derivation above), a non-convex one by triangles that tile its faces (exact, N2, N3). The winding numbers sum to the
part's as in ADR 0253 G1, the regions tile the part, and a point of the part farther than the tolerance from every cell
boundary is accepted by exactly one cell. A point within the tolerance of a boundary may be accepted by several; the rule
"largest margin, then lowest number" chooses, as before. `locate` and `locate_brute_force` agree because an accepting
triangulated cell has the point in its box widened by `tol_c` (ADR 0253 G4); the ear triangles lie in the hull of the
face's points, so the box shortcut stays valid.

In a mesh that mixes plane-tested and triangulated cells, the margin of a plane-tested cell at one of its points is the
round-off of a plane distance and need not be exactly zero, so the donor of a mesh point is not always the lowest-numbered
cell around it; the checks T ask only that it is a cell around the point and that both searches agree.

### N5. Interface and cost

New public items of `rcfd_mesh::overset`: `face_fan_folds(mesh, f) -> bool`, `face_ear_triangles(mesh, f) ->
Option<Vec<[usize; 3]>>` (the triangles of N3 as point numbers in the stored orientation, `None` when no ear is found);
on `CellLocator` the counts `n_nonconvex()`, `n_ear_clipped()` (faces of triangulated cells of the range triangulated by
N3) and `n_unsupported()`. Unchanged: `containment_margin`, `triangulated_margin`, `face_warp`, `locate`,
`locate_brute_force`, `containing_cells`, `n_warped`, `Overset`, `connectivity`, their signatures and their code for every
cell that is flat and convex or warped without a folded face. Nothing of `rcfd-fv` is touched.

Cost: at construction, the flag of N1 for every flat cell (one pass over its faces and points) and, when some cell is
triangulated, the fold test for each face of the triangulated cells and the ear clipping of the folded ones (a few
microseconds each). The ear triangulations are stored by face, only when there is at least one. A search costs as today
for a convex flat cell and as a warped cell of ADR 0253 for a triangulated cell. Printed: the construction time and the
time of `locate` on P1 and P2; no limit (the cost of F-352 is the reference and is not changed for any existing cell).

### Questions settled by taking the recommendation

Each is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the
maintainer made in person.

- **R1. The test of non-convexity**: (a) a point of the cell outside one of its face planes by more than `tol_c`; (b) the
  dihedral angle of every edge; (c) a comparison of the cell volume with the volume of the hull. Recommendation (a): it is
  exact by the characterisation, cheap, needs no edge structure, and uses the locator's own tolerance, so a convex cell
  cannot reach it (N1).
- **R2. A face whose fan folds**: (a) ear clipping, from both sides alike; (b) the fan about a point of the kernel (needs a
  kernel, which an L-shape with long arms has only in its corner and a general polygon may not have); (c) refuse the mesh
  with a located message. Recommendation (a): it handles every simple polygon and is a function of the face alone; (c) is
  kept for the case where ear clipping finds no ear, as a count rather than a refusal (below, Q3).
- **R3. Where ear clipping applies**: (a) to every face of a triangulated cell whose fan folds (warped or not); (b) only to
  the faces of non-convex flat cells. Recommendation (a): the triangulation of a face must not depend on which of its two
  cells asks, and a face can belong to a warped and to a non-convex cell at once. It changes nothing for a warped cell
  without a folded face (every cell of every registered set-up).
- **R4. The test meshes**: (a) prisms on a constructed tiling of non-convex polygons, built in the test file; (b) the
  polyhedral dual of `rcfd_mesh::polydual`; (c) cells merged from a box mesh. Recommendation (a): the dual has non-planar
  faces (code read, 5), and merged cells of a box mesh meet their neighbours in several faces or in bent faces, which the
  mesh refuses (code read, 4).

Not taken, because it could change a default result: refusing in `Overset::new`, with a located message, a mesh that has
an unsupported face (no ear) or a folded warped face; listed as open in the addendum.

## Set-ups (built in the new test file `crates/rcfd-mesh/tests/overset_f356.rs`)

Both are prisms in a lattice space `y` (a base tiling of polygons in the plane `y_3 = 0`, layers at heights `z_0 < ... <
z_K`), mapped to the mesh by the affine map `x = A y + b`, `A = [[1.0, 0.15, -0.1], [0.05, 0.95, 0.12], [-0.08, 0.1,
1.05]]` (determinant `0.968`), `b = (0.3, -0.2, 0.1)`. An affine map keeps planes planar, so every face is planar to
round-off. The layer heights are `z_k = hz (k + 0.2 jitter((k, 0, 0), 5))` for `0 < k < K`, `z_0 = 0`, `z_K = K hz`, with
the `jitter` function of ADR 0253's test file (values in `[-1, 1]`). Cell numbering: cell `(c, k)` (base polygon `c`,
layer `k`) is `k n_base + c`. Faces: one side face (a quadrilateral, two points at `z_k`, two at `z_{k+1}`) for every
edge of the base tiling and layer, one face at each height for every base polygon (the polygon itself, with all its
points); internal faces sorted by `(owner, neighbour)`, all boundary faces in one patch `walls`. The construction makes
every pair of neighbouring base polygons share exactly one edge (P1 by its structure, P2 checked in `p2_precheck.py`), so
every pair of cells meets in one planar face. `Mesh::from_face_lists` validates the result.

- **P1 (darts).** Base: a lattice of `16 x 16` quadrilaterals of size `h = 0.1`, the point `(i, j)` moved by `0.4 h`
  along a diagonal chosen by the parities of `i` and `j` (`(even, even)` to `(+, +)`, `(odd, odd)` to `(+, -)`, `(odd,
  even)` to `(-, -)`, `(even, odd)` to `(-, +)`), the component normal to the outer boundary set to zero (so the base is the
  square `[0, 1.6]^2`). Every inner quadrilateral is a dart congruent to the example above (one reflex vertex); the 31 of
  the boundary ring are convex. `K = 5`, `hz = 0.12`. 1280 cells, 1125 non-convex. No fan folds (the face centre lies in
  the kernel of every dart, `dart_scan.py`, `dart_sub.py`).
- **P2 (L-shapes and triangles).** Base: `6 x 6` blocks of `3 x 3` lattice units of size `h = 0.1`; each block is an
  L-shape with the eight points `(0,0), (1,0), (3,0), (3,1), (1,1), (1,3), (0,3), (0,1)` and the two triangles `(1,1),
  (3,1), (3,3)` and `(1,1), (3,3), (1,3)` that fill its notch (offset by the block). Every lattice point is moved by
  `0.1 h (jitter((i, j, 0), 1), jitter((i, j, 0), 2))`, the component normal to the outer boundary set to zero (the base is
  `[0, 1.8]^2`). `K = 8`, `hz = 0.2`. 864 cells, 288 non-convex (the L-prisms); the fan of every L-face folds (36 of 36
  base polygons; 324 faces), and 32 of 36 L-faces have their centre outside the face.

**The reference (independent containment).** For P1 and P2 the test keeps the construction in lattice space: the base
points, the decomposition of each base polygon into triangles known from the construction (a dart into the two triangles
on the diagonal through its reflex vertex, found by the sign of the turn; a convex quadrilateral on the diagonal from its
first point; an L-shape into its three quadrilaterals `(0,0),(1,0),(1,1),(0,1)`, `(1,0),(3,0),(3,1),(1,1)`,
`(0,1),(1,1),(1,3),(0,3)`, each on its diagonal from the first point; a triangle as itself) and the heights `z_k`. A
point is generated as `y` and given to the locator as `x = A y + b`; the reference contains `y` in cell `(c, k)` when
`z_k - 1e-9 hz <= y_3 <= z_{k+1} + 1e-9 hz` and the base point of `y` lies in one of the triangles of `c` with every
barycentric coordinate at least `-1e-9`. It does not use the mesh, its centres or area vectors, the locator, or any code of
`rcfd_mesh::overset`. Check S0 shows that the triangles have positive areas and that, for each base polygon, their areas
sum to its area (shoelace) to `1e-12` relative.

**Random points.** `y` uniform in `[1e-6, L - 1e-6]^2 x [1e-6, K hz - 1e-6]` (`L` the side of the base), from the
linear congruential sequence of ADR 0219 with the seed `0x5eed_f356`; 100000 points for each of P1 and P2. Every point
is inside the mesh (the base is a square and the layers fill `[0, K hz]`).

**The convex set-ups** of ADR 0253 A4 (RL levels 1, 2, 4; TG_L levels 1, 2, 4; U; TR; DC; FB; B3; B3s) and the warped B3d,
built by the same code (copied from `overset_f352.rs`, as that file copied it from `overset_f336.rs`).

**A3 (an overset on a non-convex background).** P2 as the background, a box of `8^3` cells of `0.1` (`box_mesh`) as the
component (prefix `c_`, the outer patches the six sides), no hole (`hole = |_| false`), at the pose: origin `(0.4, 0.4,
0.4)`, axis `unit(1, 2, 3)`, angle `0.2`, translation `A (0.9, 0.9, 0.8) + b - (0.4, 0.4, 0.4)`, which puts the centre of
the component on the image of the centre of P2's lattice box. The component's half-extent after the turn is at most
`0.4 (1 + 0.2 sqrt 2) = 0.52`, inside the lattice half-extents `0.9, 0.9, 0.8` mapped by `A` (whose distortion of
lengths is below 25 percent).

## Arithmetic before this registration (disclosed)

- The dart of P1 (above): area 1, one reflex vertex for the displacement `s > 0.309` (the turn at `LL` is `1 - 2 s - 4
  s^2`, zero at `s = 0.309`); `s = 0.4`. On the `16 x 16` lattice: 225 darts, 31 convex boundary quadrilaterals, no cell
  of non-positive area, no folded fan, no side face that breaks the owner-to-neighbour orientation of `validate` with the
  cell centres of `compute_geometry` (`dart_precheck.py`). Darts with other displacements and aspect ratios, and with
  their edges subdivided by collinear points (1 to 6 pieces), never fold (`dart_scan.py`, `dart_sub.py`): a quadrilateral's
  kernel is large. Hence P2 for the folded faces.
- P2 (`p2_precheck.py`, jitter amplitude `0.1 h`): 108 base polygons, 36 non-convex, none of non-positive area, 36 folded
  fans, 32 L-faces with the centre outside the face, no side face breaking the orientation (one layer of height `1.5 h`),
  no pair of base polygons sharing more than one edge.
- Share of the base area that the plane test refuses (outside the kernels; Monte Carlo with 40000 points): P1 `0.265`,
  P2 `0.446` (for the L-shapes of arm 3; `0.374` for arm 4).
- Tolerance bands: `tol_c` is about `1e-10 x 0.2 = 2e-11` for the cells of P1 and P2; the faces of a cell within
  `tol_c` fill a fraction of about `2 tol_c` times the surface over the volume of a cell, `2 x 2e-11 x 60 = 2.4e-9` (a
  surface over volume of about `60` for P1's cells of `0.1 x 0.1 x 0.12`, less for P2): `2.4e-4` expected points of
  100000 within the tolerance of a boundary. So "exactly one cell" can be asked of every point.
- Round-off of the vertex test on the convex set-ups: below `1e-14` against `tol_c >= 1e-11` (N1).
- Expected counts under the present rule (the control N1): P1 about `0.265 x 100000 = 26500` points in no cell, P2 about
  `44600`; none in two cells (the kernels are disjoint).
- The fan of P2 under the mutation MU2 (fan always): the inverted triangles of an L-face cover about `0.2` of the area of
  its notch (two thin triangles of base 2 and height about 0.1 lattice units each, of 4 units of notch), so of 20000 points
  on the internal layer planes in the notch triangles (check T4) about 1000 are at distance zero from an L-cell's
  triangles.

## Registered checks

New test file `crates/rcfd-mesh/tests/overset_f356.rs`, release (`cargo test --release -p rcfd-mesh --test
overset_f356`). No existing test is edited. "The locator" of P1 and P2 is `CellLocator::new(&mesh, 0..n_cells)`.

### A. The acceptance

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| A1 (P1, P2) | 100000 random points each: `containing_cells` | exactly one cell for every point: points in none `0`, in two or more `0` | N4; `2.4e-4` expected points in a tolerance band |
| A1f (P1, P2) | the same points: `locate` against `locate_brute_force` | number that differ `0` | N4, ADR 0253 G4 |
| A1r (P1, P2) | the same points: the located cell contains the point by the reference | number refused `0` | N4; the reference is exact up to `1e-9` |
| A2a | `overset_f352.rs` (4 tests: A1 to A3, S1 to S5, T1 to T3, C1, C2, N1, N2 and A4b to A4d of ADR 0253), the unit tests of `rcfd_mesh::overset` (K1 to K5 and the pose test), `overset_f336.rs` | pass, unedited | the code path of every cell of those set-ups is the present one |
| A2b | the overset test files of `rcfd-fv` (`overset_f134`, `overset_f134_records`, `overset_f314`, `overset_f334`, `overset_f335`, `overset_f335_records`, `overset_f336`, `overset_f338`, `overset_f357`), the tests that are not ignored | pass, unedited | `rcfd-fv` is not touched |
| A4b' | the convex set-ups RL (1, 2, 4), TG_L (1, 2, 4), U, TR, DC, FB, B3, B3s and the warped B3d: `n_nonconvex` of both locators; the largest outward vertex distance over `tol_c` computed in the test from the public parts (printed) | `0` each | round-off below `1e-14` against `tol_c >= 1e-11` |
| A4e | B3d: `n_ear_clipped` and `n_unsupported` of both locators; the donors of every fringe centre at the pose of ADR 0251 and `locate` on 20000 random points of the background (seed `0x5eed_f356`, uniform in `[0.05, 2.35]^3`) against the rule of ADR 0253 written in the test (cells whose box widened by `tol_c` contains the point; `triangulated_margin` for a cell with a face of `face_warp > tol_c`, `containment_margin` otherwise; at least `-tol_c`; the largest margin; the lowest number) | `0`, `0`; number that differ `0` | N3: no fan of B3d folds, so its triangulated cells evaluate the same triangles in the same order |
| A3 | the overset on P2 (set-up above), default and quadratic interpolation: `connectivity` succeeds with a stencil for every fringe cell; for every fringe centre `locate`, `locate_brute_force` and the first stencil cell equal, and the donor contains the centre by the reference; default stencils: `abs(sum w - 1)` and the linear field `3 + 2 x - 5 y + 1.5 z` over the largest absolute value of the field over the cells; quadratic: `q3` of ADR 0251 from every stencil that is not a fallback | `Ok`; differ `0`, refused `0`; `<= 1e-12`, `<= 1e-11`; `<= 1e-11`. Printed: the number of fallbacks | ADR 0253 A2, A3 measured `1e-15`; the weights depend on the cell centres only |

### S. The set-ups

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| S0a | P1, P2: `Mesh::from_face_lists` succeeds; `n_warped` of the locator | `Ok`; `0` | affine map of planar faces; round-off far below `tol_c` |
| S0b | P1, P2: `n_nonconvex` against the number of cells whose base polygon has a vertex with a negative turn (computed in the test in lattice space) | equal: P1 `1125`, P2 `288` | N1: a reflex vertex is `0.2 h` or more out of the plane of an adjacent face |
| S0c | P1, P2: `n_ear_clipped`, `n_unsupported` | P1 `0`, `0`; P2 `324`, `0` | `p2_precheck.py` |
| S0d | P2: every ear-clipped face (`face_ear_triangles`): every triangle with positive orientation against `S_f`, the sum of the vector areas of its triangles against `S_f` | relative difference `<= 1e-12` | N3: the triangles tile the face |
| S0e | P1, P2: the reference triangles have positive areas and their areas sum to the area of their base polygon | relative difference `<= 1e-12` | by construction |

### T. Points on shared faces

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| T1 (P1, P2) | the centre of every internal face: `locate` against `locate_brute_force`; the located cell contains the centre by the reference | differ `0`; refused `0` | N4. In P2 the centres of 32 x 7 internal L-faces lie in the notch, on the faces of the triangle prisms |
| T3 (P1, P2) | every mesh point not on the outer boundary: `locate` against `locate_brute_force`; the located cell has the point among its points | differ `0`; not among them `0` | N4 |
| T4 (P2) | 20000 points on the internal layer planes in the notch triangles (a random notch triangle `c`, a random internal height `z_k`, `0 < k < K`, random barycentric coordinates in lattice space): `containing_cells`; `locate` against `locate_brute_force` | every accepting cell is `(c, k - 1)` or `(c, k)`, at least one accepts, for every point; differ `0` | N2, N3: no L-cell is within the tolerance of a point of the notch on its plane |

### N. Negative control

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| N1 (P1, P2) | the rule of M3 (written in the test from `containment_margin`, as in ADR 0253, over the cells whose box contains the point) on the points of A1: points in no cell; points in two or more (printed) | P1 `>= 15000`, P2 `>= 30000` | `26500` and `44600`, the Monte Carlo above |

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f356-scratch` (own target
directory), the new test file is run once and the named check must fail; the numbers go in the addendum.
`scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `CellLocator::new`: no cell is flagged non-convex (the present search) | A1 on P1 and P2 (points in no cell) |
| MU2 | the fold test returns `false` (the fan of ADR 0253 for every face) | T4 (an L-cell accepts points of the notch) and S0c |
| MU3 | ear-clipped faces: the sign of the solid angle is not reversed for the neighbour | A1 on P2 |
| MU4 | the non-convexity test with the sign reversed (`< -tol_c` in place of `> tol_c`: every cell with a point inside a face plane is flagged) | A4b' |
| MU5 | ear clipping: the test that no other point lies in the ear is left out | S0d or A1 or T4 on P2 |

### Classes

A miss of A1, A1f, A1r, A3, S0, T1, T3 or T4 is reported with its number and kept as an ignored `*_known_miss` test with
the limit intact; its cause is diagnosed. A miss of A2a, A2b, A4b' or A4e means that the change is not to be merged, and
the work stops there with a report. A control (N1) that does not hold, or a mutation that does not make its check fail, is
a defective set-up: corrected by an amendment before any re-run, never counted as a pass.

## Predictions (mine, before any run)

A1, A1f, A1r met with zeros on both set-ups; A2a, A2b met; A4b' zeros with the largest ratio about `1e-4`; A4e met; A3
met with a few hundred component fringe cells (the outer layer of `8^3`: 296), some fallbacks of the quadratic stencil
possible next to the large L-cells. S0 as registered; T1, T3, T4 met. N1 near `26500` and `44600`. MU1 to MU4 caught;
of MU5 I am least sure (an L-shape may be clipped correctly even without the test of the other points, depending on the
order of its points), which is why it names three checks.

## Decision rule

- **done**: A1, A1f, A1r met on P1 and P2; A2a, A2b, A4b', A4e met; A3, S0, T1, T3, T4 met; the control held and the
  mutations failed their checks.
- **partial**: A1, A1f, A1r met on at least one of P1, P2 and A2a, A2b, A4b', A4e met, but another check is missed with
  its cause diagnosed, or a control or mutation did not hold and was not corrected.
- **todo**: A1 is missed on both set-ups, or A2a, A2b, A4b' or A4e cannot be met (the change is then not merged), or no
  registered measurement could be run.

## Limits of this registration

- The new set-ups are prisms: the non-convexity is that of their base polygons (reflex vertical edges). Non-convex cells
  of other kinds (a reflex horizontal edge, a cell with several reflex edges meeting) are covered by the derivation, not by
  a measurement.
- A cell that is non-convex by less than `tol_c` keeps the plane test; what that test then accepts outside the cell is
  within the reach of the tolerance and is not measured.
- A warped face whose fan folds is ear-clipped in its projection; that the resulting surface bounds a non-self-intersecting
  cell is not shown, and no measured set-up has such a face. A face for which ear clipping finds no ear keeps its fan and
  is only counted.
- Random points in the interior test the winding number; the distances of a folded fan matter only within the tolerance
  of the face planes, which T1 and T4 test on planes, not in general position.
- Kinematic only: no flow is solved on a non-convex overset mesh. The finite-volume geometry of a cell whose centroid lies
  outside it (P2's L-cells) is outside this item.
- The flags are set once, on the mesh the locator is built on (as in ADR 0253).

## Open questions

Listed with options and a recommendation in the addendum, after the results.
