# ADR 0253: F-352 registration: the donor search of the overset coupling on cells with non-planar faces

Status: **REGISTERED 2026-11-12**, written before any code of F-352 and before any run of its checks. Not yet accepted by the
maintainer. Before this text no check was run: the code and the documents listed under "What was read" were read, the
arithmetic under "Arithmetic before this registration" was done by hand, and the unchanged tree of main was compiled once
(`cargo test --release -p rcfd-mesh --no-run`, to fill the target directory of the new worktree; nothing was executed).
Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: the donor search M3,
its tolerance, the bins, the unit tests K2 and K4) and
[ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (F-336: the set-ups B3, B3d, B3s, the check
D1, amendment 1 with the finding, question Q2 whose recommendation is this item). Roadmap item F-352 (depends on F-134).

Number: the lead gave 0253 to F-352. Branch `f352-donor-search-nonplanar` from main 0d43f602, worktree `w49i-work/f352`.

House rules (ADR 0130, ADR 0213, ADR 0219, ADR 0251):

- limits are fixed here, before the code and the runs, and are not changed after a run; the limits of F-134, F-314, F-334,
  F-335, F-336 and F-338 are not changed, their tests and ignored records are not edited (the record `p_three_dimensions`
  of ADR 0251 stays as it is and is not run again; a new test shows B3d passing);
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- existing results stay bit-identical; if the corrected search changed a donor on a mesh with planar faces the work stops
  and the change is reported, not accepted;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no unsafe code, no commit trailers, no push.

Clean-room. The method is derived in this text from first principles. Named from general knowledge, from memory (no
paper, book, code or vendor document was opened for this work): the solid angle of a closed surface seen from a point as
the test of containment (Gauss's integral of `r / |r|^3`; in computer graphics the "winding number" of a triangle mesh,
A. Jacobson, L. Kavan and O. Sorkine-Hornung, ACM Trans. Graph. 32 (2013)); the formula for the solid angle of a plane
triangle (A. van Oosterom and J. Strackee, IEEE Trans. Biomed. Eng. 30 (1983) 125-126; F. Eriksson, Math. Mag. 63 (1990)
184-187), which is derived below because I cite it from memory; the distance of a point from a triangle by regions (C.
Ericson, "Real-Time Collision Detection", 2005, section 5.1.5; a simpler form is derived and used here); the practice of
finite-volume codes of decomposing a polygonal face into triangles about its centre for the face area and centre (the
mesh geometry of this library does so already, `polygon_geometry`).

## What F-352 asks

The roadmap entry (acceptance, quoted, binding): "On the set-up B3d of ADR 0251 (two hexahedral parts whose interior
points are displaced by up to 0.15 of the cell size in all three directions) every fringe centre gets a donor that agrees
with a full search, the linear and the quadratic stencils reproduce linear and quadratic fields to 1e-11, and on meshes
with planar faces the donors and stencils are bit-identical to those of today (the unit tests K2 and K4 of ADR 0219 and D1
of ADR 0251)."

**Reading (registered here).**

1. "Every fringe centre gets a donor": `Overset::connectivity` of B3d at the pose of ADR 0251 succeeds with the default
   and with the quadratic interpolation, with one stencil for every fringe cell (check A1).
2. "Agrees with a full search": for every fringe centre the cell found through the bins (`CellLocator::locate`) is the
   cell found by the same rule applied to every cell of the part (`locate_brute_force`), and is the first cell of the
   stencil (A1). Because a rule can agree with itself and still be wrong, the donor is also checked against an
   independent test of containment written in the test file by another method (A1r).
3. "The linear and the quadratic stencils reproduce linear and quadratic fields to 1e-11": the field `3 + 2 x - 5 y + 1.5
   z` from every default stencil (A2), and the field `q3` of ADR 0251 (all ten monomials) from every stencil built with
   `DonorInterpolation::Quadratic` that is not a fallback of ADR 0251 D4, both as the error over the largest absolute value
   of the field over the cells, as in ADR 0251 P4 (A3). A fallback is the linear stencil by design and reproduces the
   linear field; the number of fallbacks is printed and at least 95 percent of the stencils must be quadratic, so that
   the check is not empty.
4. "On meshes with planar faces the donors and stencils are bit-identical to those of today": the unit tests K2 and K4 and
   the test D1 of ADR 0251 pass unedited; and, stronger, because those tests compare the locator with itself: the donors
   and the default stencils of the registered planar set-ups are compared with the rule of ADR 0219 M3 written out again
   in the test file from the public `containment_margin`, which this item does not change (A4).

## What was read (no run)

- ADR 0251 in full (the set-ups B3, B3d, B3s, the checks D1 and P4, amendments 1 and 2, the addendum and its question Q2);
  of ADR 0219 the model M1 to M4, the checks K1 to K5 and the mutation MU5.
- `crates/rcfd-mesh/src/overset.rs` in full (`containment_margin`, `cell_box`, `CellLocator`, `Overset::connectivity`,
  the unit tests); `crates/rcfd-mesh/tests/overset_f336.rs` in full (the set-ups, `p_three_dimensions` and its record);
  of `crates/rcfd-mesh/src/mesh.rs` `polygon_geometry` and `compute_geometry`; `rcfd_mesh::gen::warp`; where
  `containment_margin` and the locator are used outside the module (`crates/rcfd-fv/tests/overset_f314.rs`, the tests of
  F-336).

What the code read shows:

1. `containment_margin(mesh, c, x)` is `min_f ( -S_f . (x - fc_f) / |S_f| )` over the faces of the cell with the outward
   area vector: one plane for each face, through the face centre `fc_f`, normal to the area vector `S_f`.
2. `CellLocator` accepts a cell when that margin is at least `-tol_c`, `tol_c = 1e-10` of the diagonal of the bounding box
   of the cell's points; of the accepted cells of a bin it takes the largest margin, and of equal margins the first, which
   is the lowest cell number because the bins are filled in increasing cell order. `locate_brute_force` applies the same
   rule to every cell of the range.
3. A cell is in every bin that its bounding box, widened by `tol_c`, overlaps.
4. `polygon_geometry`: the face centre is the area-weighted mean of the centroids of the triangles `(p_i, p_{i+1}, e)`
   about the mean `e` of the face's points, a combination of the points with positive weights, so it lies in their convex
   hull; `S_f` is half the sum of the cross products of those triangles, which is the vector area of the closed polygon
   and does not depend on the point the triangles are drawn about. The cell volume is `sum_f sigma_f S_f . (fc_f - e_c) /
   3`.
5. The mesh is validated: the faces of a cell form a closed surface (every directed edge is matched by its reverse once),
   and the stored order of a face's points gives `S_f` pointing from the owner to the neighbour.
6. The locators are built once on the composite mesh at the reference position and are always asked on that mesh.

## Why one plane for each face leaves gaps and overlaps (derivation)

Let `F` be a face with centre `fc`, unit normal `n = S_f / |S_f|` and points `p_i`. Its **warp** is

`w_F = max_i | n . (p_i - fc) |`,

the largest distance of a point of the face from the plane `E_F` through `fc` normal to `n`. `w_F = 0` exactly when the
face is planar.

*Across a face there is no gap.* The two cells `A` and `B` of a face use the same plane `E_F` with opposite signs: `A`
asks `n . (x - fc) <= tol`, `B` asks `n . (x - fc) >= -tol`. Every point satisfies one of them.

*Along an edge there is.* Take an edge `e` of the mesh with the four cells `A`, `B`, `A'`, `B'` around it (a hexahedral
mesh; `A` below `B`, `A'` below `B'`, the primed cells on the other side). Four faces meet in `e`: `F` between `A` and `B`,
`F'` between `A'` and `B'`, `G` between `A` and `A'`, `G'` between `B` and `B'`. On a mesh with planar faces `E_F` and
`E_F'` both contain `e`, and so do `E_G` and `E_G'`, so in a section across the edge the four planes are four half lines
from one point and the four wedges tile the section. With warped faces **no plane contains the edge**: `E_F` passes the
two end points of `e` at the signed distances `n . (p - fc)` of those points, which are of size `w_F` and in general of
opposite sign for a twisted quadrilateral, and `E_F'` passes them at its own distances. In the section across `e` at a
point of the edge, with `z` across `F` and `x` across `G`, let `E_F` cut at the height `z = a`, `E_F'` at `z = b`, `E_G` at
`x = c` and `E_G'` at `x = d` (to first order; the tilt of the planes changes nothing of what follows). The cells accept

`A: z <= a, x <= c`;  `A': z <= b, x >= c`;  `B: z >= a, x <= d`;  `B': z >= b, x >= d`.

For `c < d` and `b < a` the points with `c < x < d`, `b < z < a` are refused by all four: by `A` (`x > c`), by `A'` (`z > b`),
by `B` (`z < a`), by `B'` (`x < d`). That rectangle is the **sliver**, a gap. For `c < d` and `a < b` the same rectangle is
accepted by `A'` and by `B`: an overlap. The other two sign cases mirror these. So along every edge at which the four
planes do not meet in one line there is a sliver of the cross-section `|a - b| |c - d|`, a gap or an overlap with equal
chance, and the tolerance `tol` (`1e-10` of the cell size) closes it only when both differences are below about `2 tol`.

*Its size for a given warp.* `|a|` and `|b|` are at most `w_F` and `w_F'`, `|c|` and `|d|` at most `w_G` and `w_G'`, so

`cross-section <= (w_F + w_F') (w_G + w_G')`,

and it changes along the edge as the four distances do (linearly between their values at the two end points, so it
vanishes where the planes cross). For faces with the warp `w` on cells of size `h` the slivers of one cell (three edges
for each cell of a hexahedral mesh, each of length `h`) fill at most `3 (2 w)^2 h = 12 (w / h)^2` of its volume, half of
it gaps. The gap is of second order in the warp, which is why a mildly warped mesh shows it only for one point in very
many, and why it is a hard failure when it shows: the point lies in the mesh and in no cell.

## Arithmetic before this registration (disclosed, by hand)

- B3d: both parts are boxes of hexahedra of size `h = 0.1` (background `24^3 = 13824` cells, component `14^3 = 2744`),
  every interior point displaced in each direction by `0.015 jitter`, `jitter` a fixed function of the position with
  values in `[-1, 1]`, taken as uniform. The points on the sides of the boxes stay, so the outer boundary is the box.
- The warp of a quadrilateral face whose four points are displaced along the normal by `d_1 .. d_4`: the plane through
  the centre with the mean normal leaves the points at about `+t, -t, +t, -t` with the twist `t = (d_1 - d_2 + d_3 -
  d_4) / 4`. For `d_i` uniform in `[-0.015, 0.015]` (standard deviation `0.015 / sqrt(3)`): `t` has the standard deviation
  `0.015 / sqrt(3) * 2 / 4 = 0.0043 = 0.043 h` and is at most `0.015 = 0.15 h`.
- The sliver along an edge: the heights `a` and `b` of the two planes at the fraction `u` of the edge differ by `(1 - 2
  u) (t - t')`; `F` and `F'` share the two points of the edge, so `t - t' = (d_3 - d_4 - d_5 + d_6) / 4` with the four
  other points, again of standard deviation `0.0043`, mean absolute value about `0.8` of it: `0.0035`. The same for `c -
  d`. Mean of `(1 - 2 u)^2` over the edge: `1 / 3`. Mean cross-section `0.0035^2 / 3 = 4.0e-6 = 4.0e-4 h^2`; three edges
  of length `h` for each cell: `1.2e-3` of the volume, of which half are gaps: **about `6e-4` of the volume in gaps and
  `6e-4` in overlaps**. The estimate is crude (first order, the displacement along the faces and the vertices are left
  out); I take it as good to a factor of three.
- Fringe cells of B3d: the `14^3 - 12^3 = 1016` component cells on the outer patches and about 50 background cells (B3
  has 1067 stencils). `1067 * 6e-4 = 0.64` expected fringe centres in a gap; the chance of at least one is `1 - exp(-0.64)
  = 0.47`. ADR 0251 found one (the component fringe cell 16035) before the search stopped. The estimate agrees with that
  in size.
- 200000 random points in the background: about `120` in gaps and `120` in overlaps by the estimate.
- A cell's bounding box on B3d: `h` plus the largest minus the smallest of four displacements on each side, about `h +
  2 * 0.015 * 0.6 = 1.18 h`, so a point lies in about `1.18^3 = 1.64` boxes.
- The band of the tolerance: `tol = 1e-10 * 0.17 = 1.7e-11` on each side of the six faces of a cell of size `0.1`: a
  fraction `6 * 2 * 1.7e-11 / 0.1 = 2e-9` of the volume; `2.5e5` points give `5e-4` expected points in the band.
- Round-off of a "planar" face of the existing set-ups: coordinates up to 25 (TR) give deviations of about `25 * 2.2e-16 =
  6e-15` from the plane; the smallest `tol` there is `1e-10 * 0.1 = 1e-11` (the cells of the 2-D meshes are `0.1` deep).
  A factor of more than a thousand between the two.

## Design

### G1. The interior of a cell

The **triangulated cell** `T_c` is the polyhedron bounded by the triangles `(fc_f, p_i, p_{i+1})` of every face `f` of
the cell `c`, `p_i` the points of the face in their stored order and `fc_f` the face centre of the mesh.

- *The same from both sides.* The triangles of a face are a function of the face alone (its stored points and its stored
  centre), so the two cells of a face have the same triangles in their boundaries, with opposite orientation.
- *Closed.* The faces of a cell form a closed surface (what was read, 5), and inside a face the triangles share the
  spokes `fc_f p_i`; so the boundary of `T_c` is a closed triangle surface.
- *The cells tile the mesh.* For a closed oriented surface `S` and a point `x` not on it, the **winding number** `W_S(x)
  = (1 / 4 pi) sum_T Omega_T(x)` (the sum of the signed solid angles of its triangles seen from `x`) is an integer: it is
  Gauss's flux integral of `r / (4 pi |r|^3)`, which is `1` inside a simple closed surface and `0` outside. Sum `W` over
  all cells of a part: every internal triangle occurs twice with opposite orientation and cancels, what remains is the
  boundary of the part, so `sum_c W_c(x) = 1` for a point inside the part and `0` outside. If every `T_c` is a simple
  polyhedron (does not intersect itself), every `W_c(x)` is `0` or `1`, and therefore **exactly one cell contains a point
  of the part that lies on no triangle, and no cell contains a point outside**. No gap and no overlap, whatever the warp.
- *It is the cell the finite-volume geometry already means.* The vector area of the triangles of a face is `S_f` (what
  was read, 4), and the volume of `T_c` is `sum_T (1/3) S_T . (q_T - o)` with any point `q_T` of each triangle; all
  triangles of a face contain `fc_f`, so it is `sum_f sigma_f S_f . (fc_f - o) / 3`, the cell volume of the mesh.
- A triangular face is split into three coplanar triangles; the surface is the same.
- On a mesh with planar faces and convex cells `T_c` is the cell of ADR 0219 M3.

The alternatives (questions S1 below): a split of each quadrilateral along a diagonal (the choice of the diagonal must be
made the same from both sides and does not extend to polygons), and the bilinear patch (a curved surface: no closed
formula for the containment of a general polyhedral cell).

### G2. The margin of a triangulated cell

`triangulated_margin(mesh, c, x) = s d`, with

- `d` the distance of `x` from the boundary of `T_c`, the smallest distance from `x` to its triangles;
- `s = +1` when `W_c(x) > 1/2`, else `-1`.

This is the signed distance from the boundary, positive inside: the same quantity as `containment_margin`, which for a
convex cell with planar faces is exactly the distance from the boundary for a point inside. So the two margins can be
compared with each other, and the tolerance means the same for both.

*Distance from a triangle `(a, b, c)`.* `|x - q|^2` is a convex function of `q`, so its minimum over the triangle is at
the foot `x'` of the perpendicular on the plane if `x'` lies in the triangle, and otherwise on one of the three edges. `x'`
lies in the triangle exactly when `((b - a) x (x - a)) . n`, `((c - b) x (x - b)) . n` and `((a - c) x (x - c)) . n` are
all `>= 0` (`n = (b - a) x (c - a)`). The distance from an edge `(p, q)` is `|x - (p + t (q - p))|` with `t = (x - p) . (q
- p) / |q - p|^2` clamped to `[0, 1]`; at `t <= 0` the nearest point is `p` itself and at `t >= 1` it is `q` itself, so a
point that *is* a triangle point has the distance exactly zero. The code takes the smallest of the three edge distances
and, when `x'` is in the triangle, of the plane distance `|(x - a) . n| / |n|` (which is then the smallest). A triangle of
zero area is treated by its edges.

*Solid angle of a triangle (derived; the formula is cited from memory).* With `A = a - x`, `B = b - x`, `C = c - x`:

`Omega = 2 atan2( A . (B x C),  |A| |B| |C| + (A . B) |C| + (B . C) |A| + (C . A) |B| )`.

Derivation. Let `a, b, c` be the unit vectors of `A, B, C`, read as pure quaternions (`u v = -u . v + u x v`, `u^2 = -1`).
`1 - b a = 1 + a . b + a x b` is, up to a positive factor, `cos(theta/2) + sin(theta/2) m` with `theta` the angle from
`a` to `b` and `m` the unit vector of `a x b` (because `1 + cos theta = 2 cos^2(theta/2)` and `sin theta = 2 sin(theta/2)
cos(theta/2)`): the rotation that carries `a` to `b` along their great circle, that is, the parallel transport of the
sphere from `a` to `b`. The transport around the triangle is the product `(1 - a c)(1 - c b)(1 - b a)`. Multiplying out
with `c c = b b = -1`: `(1 - a c)(1 - c b) = 1 - c b - a c - a b`, and times `(1 - b a)`: `2 - c b - a c - c a - a b - b a +
a c b a`. Here `a b + b a = -2 a . b`, `a c + c a = -2 a . c`, `c b = -b . c - b x c`, and for a pure `v`, `a v a = v - 2 (a
. v) a`, so `a c b a = a (-b . c - b x c) a = b . c - b x c + 2 (a . (b x c)) a`. The sum is `2 (1 + a . b + b . c + c . a) + 2
(a . (b x c)) a`: a rotation about `a` by the angle `phi` with `tan(phi / 2) = a . (b x c) / (1 + a . b + b . c + c . a)`.
The rotation that parallel transport around a geodesic triangle of the unit sphere produces is by the area of the
triangle (the theorem of Gauss and Bonnet for curvature one; equivalently Girard's theorem that the area is the sum of
the angles less `pi`); the area is the solid angle. Multiplying numerator and denominator by `|A| |B| |C|` gives the
formula; `atan2` gives the branch in `(-2 pi, 2 pi]`, and a triangle with a point at `x` gives `atan2(0, 0) = 0`.

The sign of the triangle of a face is that of the stored point order for the owner of the face and the opposite for its
neighbour (what was read, 5). Off the surface `W` is an integer, so the comparison with `1/2` is safe against the
round-off of the sum; within round-off of the surface the sign is arbitrary, and there `d` is far below the tolerance,
so the cell is accepted whatever the sign.

*A shortcut that changes nothing.* The face centres lie in the convex hull of the points (what was read, 4), so `T_c` lies
in the bounding box of its points. A point more than `tol_c` outside that box has `W = 0` and `d > tol_c`: it would be
refused. The locator refuses it without evaluating the triangles.

### G3. Which cells use it: the planar ones do not

`CellLocator::new` gives every cell of its range a flag:

**warped** when one of its faces has `w_F > tol_c` (the warp defined above against the tolerance of the locator,
`1e-10` of the cell's box diagonal); **flat** otherwise.

- A **flat** cell is tested as today: `containment_margin(mesh, c, x) >= -tol_c`, the same function, the same comparison,
  the same order of evaluation. The code path of a flat cell is the present one, textually, so on a mesh all of whose
  cells are flat the locator returns bit for bit what it returns today, for every point. This is how the bit-identity is
  kept: not by showing that a new test gives the same numbers, but by not using it.
- A **warped** cell is accepted when `triangulated_margin(mesh, c, x) >= -tol_c` (after the box shortcut).
- The choice among several accepted cells is unchanged: the largest margin, and of equal margins the lowest cell number.

*Why the threshold is the locator's own tolerance.* If every face of a convex cell has a warp of at most `tol_c`, every
point of the cell (of the hull of its points) has `n_F . (x - fc_F) <= w_F <= tol_c` for every face, so the plane test
accepts it: a warp below the tolerance cannot open a gap that the tolerance does not close, and such a cell needs nothing
new. Above it the plane test can lose points. The round-off of the planar faces of the existing set-ups is more than a
thousand times below the threshold (arithmetic above), and the check A4 counts the warped cells of every registered
planar set-up: the count must be zero.

*A flat cell beside a warped cell* share a face that is planar to the tolerance; on it the plane of the flat cell and the
triangles of the warped cell agree to `tol`, both margins are signed distances from it, and the rule "largest margin"
gives the point to the cell it lies in.

### G4. What "agrees with a full search" means, and the point on a shared triangle

`locate` and `locate_brute_force` apply one rule, the first to the cells of the bin of `x`, the second to all cells of the
part. They agree for every `x` if every cell that accepts `x` is in the bin of `x`: an accepting flat cell as today; an
accepting warped cell has `x` in its box widened by `tol_c` (the shortcut), and it is in every bin that box overlaps.

A point on a triangle shared by two cells (or on an edge or at a point shared by more) has the distance zero from both,
so both accept it with a margin of zero up to round-off; the sign is arbitrary. The rule is then deterministic, a
function of the point and the mesh alone:

1. the larger margin wins (two margins that differ by round-off are different numbers, and both searches compute the same
   numbers);
2. of equal margins the lowest cell number wins. `+0` and `-0` compare equal.

The triangles of a shared face are evaluated in the same order with the same numbers from both sides (the stored points
and centre of the face; only the sign of the solid angle differs), so the distance from a shared triangle is the same
number, bit for bit, in both cells. In particular a point that *is* a face centre or a mesh point has the distance
exactly zero in every cell around it, and the donor is the lowest-numbered of those cells (checks T1, T2). No point of
the part is without a cell because it lies on a triangle.

### G5. Interface and cost

`rcfd_mesh::overset` gets the public function `triangulated_margin(mesh, c, x)`, the public function `face_warp(mesh, f)`,
and on `CellLocator` the methods `n_warped()` (the number of warped cells of its range) and `containing_cells(mesh, x)`
(every cell of the range that accepts `x`, by a search over all of them; for checks). `containment_margin`, `locate`,
`locate_brute_force`, `Overset` and `connectivity` keep their signatures; `containment_margin` is not changed at all (it
stays the plane margin, and the tests of F-314 and F-336 that call it mean that). Nothing of `rcfd-fv` is touched.

Cost. For a flat cell nothing changes. At construction: one pass over the faces of every cell for the flag (a dot
product for each point of each face) and, when there is a warped cell, the storage of the boxes. For a warped candidate
cell: six comparisons for the box, and when the point is in the box, for each triangle (24 for a hexahedron) three edge
distances, one plane test and one solid angle (three square roots and one `atan2`): of the order of a hundred
nanoseconds for each triangle, a few microseconds for each cell, against about fifty nanoseconds for the six planes. A
point lies in about `1.64` boxes on B3d (arithmetic above), so a search costs a few microseconds more than today; the
weights of the stencil that follows cost as much. The counts and times are checks C1 and C2.

### Questions settled by taking the recommendation

Each is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); none is a decision the
maintainer made in person.

- **S1. The surface of a warped face**: (a) triangles about the face centre; (b) a diagonal split; (c) a bilinear patch.
  Recommendation (a): the same from both sides for any polygon, and the cell of the mesh's own area vectors and volumes.
- **S2. Inside or outside**: (a) the winding number; (b) tetrahedra about the cell centre (needs cells that are
  star-shaped about their centre); (c) counting the crossings of a ray (needs rules for a ray through an edge).
  Recommendation (a): no assumption on the shape and no special case; (b) is used in the test file as the independent
  check.
- **S3. The margin of a warped cell**: (a) the signed distance from the triangles; (b) no margin, inside or not.
  Recommendation (a): the tolerance and the choice among cells keep their meaning, and flat and warped cells can be
  compared.
- **S4. The threshold between flat and warped**: (a) the locator's tolerance; (b) a larger number. Recommendation (a), by
  the argument of G3.
- **S5. `containment_margin`**: (a) left as it is, a new function beside it; (b) changed. Recommendation (a): tests of
  F-314 and F-336 use it for what it is.

Not taken, because it could change a default result: using the triangulated test for **every** cell. It would also
repair the search on non-convex cells with planar faces (for which M3 is not exact either), but the margins of flat cells
would be computed by other arithmetic and a donor on a planar mesh could change at a tie. Listed as open in the
addendum.

## Registered checks

New test file `crates/rcfd-mesh/tests/overset_f352.rs`, release (`cargo test --release -p rcfd-mesh --test
overset_f352`). No existing test is edited. The set-ups B3, B3d, B3s, RL, TG_L, U, TR, DC and FB are those of ADR 0251,
built by the same code (copied into the new file). "The rule of M3" in the checks is written in the test file from the
public parts: `tol = 1e-10` of the cell's box diagonal, `containment_margin(mesh, c, x) >= -tol`, the largest margin, the
lowest number, over every cell of the part. "The reference" is the independent containment test of the test file: `x` is
in the cell when it lies in one of the tetrahedra `(cc, fc_f, p_i, p_{i+1})` of the cell (`cc` the cell centre), all four
barycentric coordinates at least `-1e-9`. Random points come from the fixed linear congruential sequence of the unit
tests of ADR 0219 with the seed `0x5eed_f352`.

### A. The acceptance

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| A1 | B3d at the pose of ADR 0251, default and quadratic interpolation: `connectivity` succeeds; the number of stencils equals the number of fringe cells; for every fringe centre `locate`, `locate_brute_force` and the first cell of the stencil | `Ok`; equal; the three equal for every fringe cell (number that differ `0`) | G4 |
| A1r | the same fringe centres: the donor contains the centre by the reference | number that do not: `0` | G1; the tolerance band holds `1e-6` expected points |
| A2 | B3d, default stencils: `abs(sum_k w_k - 1)`; the linear field `3 + 2 x - 5 y + 1.5 z` at every fringe centre from its stencil, over the largest absolute value of the field over the cells | `<= 1e-12`; `<= 1e-11` | the weights of M4 are exact for linear fields on any stencil; K3 of ADR 0219 measured `1e-13` and below |
| A3 | B3d, `Quadratic`: `abs(sum_k w_k - 1)` of every stencil; `q3` from every stencil that is not a fallback, relative as above; the linear field from every stencil; the share of the stencils that are not fallbacks; every stencil cell calculated and of the other part, the donor first, no cell twice (P5 of ADR 0251) | `<= 1e-12`; `<= 1e-11`; `<= 1e-11`; `>= 0.95`; exact | ADR 0251 measured `1e-15` on B3 and B3s; the weights depend on the cell centres only. Printed, no limit: the fallbacks, the stencil sizes, `Lambda` |
| A4a | the unit tests of `rcfd_mesh::overset` (K1 to K5) and `crates/rcfd-mesh/tests/overset_f336.rs` (D1 and the others that are not ignored) | pass, unedited | the path of a flat cell is the present one |
| A4b | the number of warped cells of both locators of TR, RL (levels 1, 2, 4), TG_L (levels 1, 2, 4), U, DC, FB, B3 and B3s | `0` each | round-off `6e-15` against a threshold of `1e-11` or more |
| A4c | donors on planar meshes against the rule of M3: every fringe cell of RL (levels 1, 2), TG_1 (the 200 poses), U (80 poses), TR (5 poses), DC, B3, B3s: the first cell of the default stencil; and on TR the 2000 points of K2 for each part: `locate` | equal to the rule of M3 for every cell and point (number that differ `0`; `None` equal to `None`) | A4b |
| A4d | default stencils on planar meshes: the same set-ups, the whole connectivity against stencils built in the test from the donor of the rule of M3, its calculated face neighbours and `linear_weights` | cells and weights equal bit for bit | the same |
| A5 | the existing overset tests of `rcfd-fv` that are not ignored (`overset_f134`, `overset_f134_records`, `overset_f314`, `overset_f334`, `overset_f335`, `overset_f335_records`, `overset_f336`, `overset_f338`): Q0 and the hash R1 of ADR 0219, the records R2 and R2' of ADR 0238, the checks of ADR 0251 | pass, unedited | `rcfd-fv` is not touched and its set-ups are planar |

### S. The cells tile a warped mesh (random points)

On the two parts of B3d at the reference position: 200000 points uniform in `[0.05, 2.35]^3` of the background and 50000
in `[0.05, 1.35]^3` of the component (the parts are the boxes `[0, 2.4]^3` and `[0, 1.4]^3`, so every point is inside its
part and more than the tolerance from the outer boundary).

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| S1 | background: the number of cells that accept each point (`containing_cells`) | exactly one for every point: points with none `0`, points with two or more `0` | G1; `5e-4` expected points in a tolerance band |
| S2 | component: the same | the same | the same |
| S3 | every point of S1 and S2: `locate` against `locate_brute_force`; the located cell against the reference | number that differ `0`; number refused by the reference `0` | G4; G1 |
| S4 | set-up: the smallest volume of a tetrahedron `(cc, fc_f, p_i, p_{i+1})` over all cells of B3d (the cells are star-shaped about their centres, so the reference is valid); the share of the cells of each part that are warped | `> 0`; `>= 0.99` | a displacement of `0.15 h`; every cell has a displaced point |
| S5 | every cell centre of both parts of B3d located by its part's locator | its own cell, for every cell | the centre of a mildly distorted hexahedron is inside it |

### T. Points on shared triangles (the tie-break), background of B3d

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| T1 | the centre of every internal face of the background: `locate` and `locate_brute_force` | equal, and the owner of the face (the lower-numbered of its two cells), for every face | G4: both distances are exactly zero |
| T2 | every interior point of the background mesh: the same | equal, and the lowest-numbered of the cells around the point, for every point | the same |
| T3 | 20000 points on shared triangles (a random internal face, a random triangle of it, random barycentric coordinates): the same | equal, and one of the two cells of the face, for every point | G4; which of the two is not fixed by the registration (the point is on the triangle only up to round-off) |

### C. Cost

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| C1 | the mean number of cells of the background of B3d whose bounding box (widened by the tolerance) contains a point of S1: the number of triangulated tests of a full search, an upper bound for the search through the bins | `<= 3.0` | `1.64` (arithmetic above) |
| C2 | the time of `connectivity` (default interpolation) of B3d over that of B3, each the mean of five calls | `<= 20` | about `3`; the limit is wide because the machine is shared and timings on it are not reproducible to better than a factor of two |

Printed, no limit: the times of C2 in milliseconds, the time of `CellLocator::new` for the background of B3 and of B3d,
the time of `locate` for each point of S1, the numbers of fringe cells, the largest and the mean warp over the cell size.

### N. Negative controls

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| N1 | the rule of M3 (the present search) on the fringe centres of B3d: the component fringe cell 16035, whose centre must print as `(2.009494, 1.010188, 1.499172)`; the number of fringe centres for which the rule finds no cell | no cell for 16035, as recorded by ADR 0251 amendment 1; `>= 1` | the record; `0.64` expected, one is known |
| N2 | the rule of M3 on the 200000 points of S1: the number of points accepted by no cell, and by two or more (the rule applied to the cells whose centre is within `0.25` of the point; the planes of a cell enclose a region within `0.2` of its centre) | `>= 20` each | about `120` each, good to a factor of three; it shows that S1 tells the two rules apart |

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f352-scratch` (own target
directory), the new test file is run once and the named check must fail; the numbers go in the addendum.
`scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MW1 | `CellLocator::new`: no cell is flagged warped (the present search) | A1 (no donor on B3d) and S1 |
| MW2 | `triangulated_margin`: the neighbour of a face draws the triangles about the first point of the face instead of the face centre (the two sides disagree) | S1 or S3 |
| MW3 | `triangulated_margin`: the sign of the solid angle is not reversed for the neighbour of a face | S1 |
| MW4 | `CellLocator::locate`: of equal margins the last cell wins (`>=`) | T1 or T2 |
| MW5 | `CellLocator::new`: every cell is flagged warped | A4b |
| MW6 | `CellLocator::accepts`: the box of a warped cell is shrunk by 5 percent of its size on every side | S1 |

### Classes

A miss of A1 to A3, S1 to S5, T1 to T3, C1 or C2 is reported with its number and kept as an ignored `*_known_miss` test
with the limit intact; its cause is diagnosed. A miss of A4 or A5 means that the change is not to be merged, and the work
stops there with a report. A control (N1, N2) that does not hold, or a mutation that does not make its check fail, is a
defective set-up: corrected by an amendment before any re-run, never counted as a pass.

## Predictions (mine, before any run)

A1 to A5 met; B3d has about 1070 stencils, no fallback, `q3` near `1e-15`. S1 to S5 and T1 to T3 met with zeros. C1 near
`1.6`, C2 between 1 and 4. N1 holds with one to three fringe centres without a cell; N2 holds with a few tens to a few
hundred points of each kind. I am least sure of N2 (the estimate is crude) and of T3 being free of surprises.

## Decision rule

- **done**: A1 to A5 are met, S1 to S5, T1 to T3, C1 and C2 are met, the controls held and the mutations failed their
  checks.
- **partial**: B3d gets its donors (A1 met) and the planar meshes are bit-identical (A4 and A5 met), but another check is
  missed with its cause diagnosed, or a control or mutation did not hold and was not corrected.
- **todo**: A1 is missed, or A4 or A5 cannot be met (the change is then not merged), or no registered measurement could
  be run.

## Limits of this registration

- A cell is tested as the polyhedron of the triangles about its face centres. A cell that intersects itself has no
  interior in this sense; such a cell is not a valid finite-volume cell either.
- Non-convex cells with planar faces keep the plane test of M3, which is not exact for them; this is as today and is
  left so to keep the planar results bit-identical (the open question above).
- The warped set-up is one mesh family (hexahedra with randomly displaced points, warp up to `0.15 h`), at one pose, and
  kinematic: no flow is solved on a warped overset mesh.
- The flags are set once, on the mesh the locator is built on; the locators of `Overset` are built at the reference
  position and used there (a rigid motion does not change a warp).
- The cost is measured on a shared machine; the count C1 is deterministic, the ratio C2 is not.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
