# ADR 0054: Wave 28C, mesh adaptation criteria, hanging-node refinement, polyhedral dual conversion, turbomachinery post-processing (F-102, F-123)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave and before any run
of this wave. Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up
defect gets a NEW corrected test and the old one stays as a record. Addenda (exact numbers, deviations) may be appended at the end.

Clean-room: first principles and published literature only. No vendor code, text or installs, nothing from
vendor material. Waves 28A (VOF wiring) and 28B (chemistry) work in the same tree: this wave adds new files
(`rcfd-mesh/src/adapt.rs`, `rcfd-mesh/src/polydual.rs`, `rcfd-post/src/turbo.rs`, tests) and only one-line hunks in shared
`lib.rs` files, `reference-data.md`, `meshing.md`, `roadmap.json`, committed through a temporary index. No existing solver or
mesh path is edited, so the baseline hashes and all existing tests are unchanged by construction (re-verified at the end).

## Audit (HEAD 9c75732)

* `rcfd_mesh::octree::fill` (wave 26B) builds a 2:1 balanced octree around a surface with a private `Tree`; the lattice, balance
  and hanging-node face-loop code is private to that module and tied to surface classification, so it is NOT reused: adaptation
  needs refinement and coarsening of an existing leaf set, which `fill` does not offer. The same ideas (integer lattice, face
  split into four, hanging nodes inserted into every face loop) are re-implemented in a smaller form in `adapt.rs`.
* `Mesh` (owner/neighbour polyhedra) already validates closed cells, positive volumes and owner < neighbour ordering. Face loops
  with collinear extra vertices are legal; `polygon_geometry` is exact for planar polygons.
* `rcfd-post` has `Surface` (marching-tetrahedra cuts of any implicit function, `clip_by_values`, `rotational_copies`),
  `PointCloud::sample` (cell field with gradient reconstruction), `wall_force_and_moment` and the stats helpers. There is no
  meridional (hub/shroud) geometry, no pitch averaging and no performance report.
* The MRF / rotating-frame sources use `omega` (rad/s) about `axis` through `origin` (`rcfd-fv::sources`); the turbo module uses
  the same convention (right-handed rotation about the unit axis).
* Case tree: the mesh section is a generator selector, there is no natural place for an adaptation loop (it needs a solution
  field and re-meshing between solves), and the post section has no object family for turbomachinery. Nothing is added to the
  schema (see "Not delivered").

## Decisions (F-102)

D1 Adaptation lattice (`adapt::AmrMesh`). A tensor box `[x_0, x_n] x [y_0, y_m] x [z_0, z_k]` with arbitrary monotone base
coordinates (graded boxes work) is a forest of octrees (quadtrees when an axis is not refined, which is how a one-cell-thick 2-D
slab keeps `empty` front/back faces). A leaf is `(level, i, j, k)`; coordinates of a lattice point at the finest level `L` are
linear interpolations of the base coordinates, so every level-`l` child is exactly half of its parent along each refined axis.
Hanging nodes are NOT removed by transition cells: a leaf face next to finer leaves is replaced by the finer leaves' faces (so the
coarse cell becomes a polyhedron with extra faces), and every face loop of every cell contains all existing lattice points lying on
its boundary edges (collinear extra vertices). Because every cell sees the same set of vertices on every shared segment, each cell
is closed and each face is shared by exactly two cells for ANY level distribution; the optional 2:1 balance (leaf face neighbours
differ by at most one level) is applied for quality, not for validity.

D2 Marks. `Mark::{Coarsen, Keep, Refine}` per cell of the CURRENT mesh. `MarkSpec { refine_above, coarsen_below, min_level,
max_level }` turns an indicator into marks with a dead band (`coarsen_below < refine_above`, validated with a located error):
`indicator > refine_above` refines (unless the cell is at `max_level`), `indicator < coarsen_below` coarsens (unless at
`min_level`), in between keeps. That dead band is the hysteresis: a cell refined because the indicator exceeded the upper
threshold is not coarsened again until the indicator falls below the (lower) coarsen threshold. A parent is merged only when all
its children are leaves and all marked `Coarsen`, and (balance on) merging does not create a level jump above 1. `Refine` always
wins in a conflict. Marks of several criteria combine with `combine_marks` (refine if any refines, coarsen only if all coarsen).

D3 Criteria (library functions on `&Mesh` and a cell field; indicators are non-negative):
* gradient: `|grad phi|` from the least-squares gradient (`lsq_gradient`, face neighbours, axes mask for 2-D); optional scaling by
  the cell length `h = V^(1/dim)` (`scale_by_length`) for a dimensionless jump `|grad phi| h`;
* curvature: Frobenius norm of the Hessian, `H_i = lsq_gradient(component i of grad phi)`, optionally times `h^2`;
* error indicator (Zienkiewicz-Zhu type): raw gradient `g_c` = least-squares gradient, recovered gradient `g*_c` = volume-weighted
  mean of `g` over the cell and its face neighbours, `eta_c = |g*_c - g_c| sqrt(V_c)` (an energy-norm-like size);
* region / geometry: box, sphere, slab (distance to a plane below a half-thickness), and distance to a `TriSurface` within a
  cut-off through `octree::SurfaceIndex::dist_within`; indicator 1 when the cell centre is inside, else 0.

D4 Refinement operator (`AmrMesh::adapt`): marks to a new leaf set (refine one level per call, closure for the optional balance
by iterating "refine leaves whose face neighbour is more than one level finer"), a `Transfer` map (each new cell is `Same(old)`,
`Child { parent, .. }` or `Parent { children }`), and `AmrMesh::to_mesh` building the `Mesh` (via `Mesh::from_face_lists`) with the
six box patches named and typed by the caller. Boundary faces of refined cells are split with the cell.

D5 Field transfer. Refinement: `Injection` (child = parent) or `Linear` (child = parent + grad . (x_child - x_parent) with the
least-squares gradient of the OLD mesh; conservative because the volume-weighted child centroid is the parent centroid and exact
for linear fields). Coarsening: volume-weighted restriction `q_parent = sum V_c q_c / sum V_c`. Both conserve `sum V q` to
round-off.

D6 Polyhedral conversion (`polydual::tet_to_polydual`): cell-centre-based dual of a `TetMesh` (merge-based conversion is not
implemented). One polyhedron per tet vertex. The dual face between vertices `v, w` is the closed ring through the centroids of the
tets around the edge for an interior edge; for a boundary edge the ring is open and closed through the two boundary-triangle
centroids and the edge midpoint. Boundary faces of a vertex cell are planar quads `(v, mid(v,a), centroid(t), mid(v,b))` per boundary
triangle `t`, carrying that triangle's tag, so the boundary patches are tiled exactly. Loops are oriented from the lower to the
higher cell index (owner to neighbour) by the sign of `S . (x_w - x_v)` and boundary quads away from their adjacent tetrahedron
centroid. A non-manifold edge ring or a vertex used by no tetrahedron is a located error. The twisted interior rings are
triangulated by the standard vertex-average fan of `Mesh`, shared by both cells, so closure and total volume are exact whatever
the warp.

D7 Not implemented: target-node-count, edge-length weighting, anisotropic and register-based criteria, pre-coarsening for multigrid,
tet red-green refinement, merge-based polyhedral conversion, adaptation of non-box (curved / unstructured) domains, a case-tree
adaptation loop, parallel adaptation, dynamic load balancing.

## Decisions (F-123)

D8 Geometry. Cylindrical frame `TurboAxis { origin, axis, ref_dir }`, `z = (p - o) . a`, `r = |(p - o) - z a|`,
`theta = atan2(...)` measured from `ref_dir` about `axis`. Meridional curves are polylines `(z_i, r_i)` with strictly increasing `z`
(axial and mixed-flow hub/shroud; radial impellers with `z(r)` curves are not supported).
* constant-span surface at span fraction `s`: revolution of `r = R_h(z) + s (R_t(z) - R_h(z))` (hub/shroud curves on the same `z`
  range, constant outside it) or a plain cylinder `r = R`; implicit `g = r - R_s(z)` cut by `surfaces::cut_implicit`, followed by a
  radial snap of every vertex to `r = R_s(z)` (exact radius; area then converges as the inscribed polygon, second order);
* constant-streamwise surface at fraction `m in [0,1]`: the straight meridional segment from the hub point to the shroud point at
  normalised arc length `m` of each curve, revolved (plane, cone or cylinder); implicit `g = n . ((z, r) - H)`, clipped between
  `H` and `S`, then snapped in the meridional plane (exact on the cone).
* 360-degree reconstruction through `Surface::rotational_copies`.

D9 Averages. Area-weighted `<f>_A = sum f_i A_i / sum A_i`; mass-flow-weighted `<f>_m = sum rho (u.n)_i f_i A_i / sum rho (u.n)_i A_i`
(`n` the unit normal of the station in the downstream direction). Circumferential (pitch) average at one `(z, r)` of a station:
`n_theta` equispaced samples over one pitch `2 pi / N` or the full circle (trapezoid rule in theta, which is exact for the
harmonics `cos(k N theta)` unless `k N` is a multiple of `n_theta`; with the non-aliased choice the mean of `A + B cos(N theta)` is
`A` to round-off). Spanwise integration of pitch-averaged values with weight `2 pi r dl` (`dl` along the station) by composite
Simpson with an even number of intervals (exact for cubic integrands). Sampling closures give the library-level exactness; the
mesh path (`PointCloud::sample`, linear reconstruction) is interpolation-limited and gets its own, looser limits.

D10 Performance (`PerformanceReport`, all closed forms stated in the docs):
* incompressible pump: total-pressure rise `dp0 = <p0>_out - <p0>_in` (mass-flow weighted, `p0 = p + rho |u|^2 / 2`), head
  `H = dp0 / (rho g)`, shaft power `P = T omega`, hydraulic efficiency `eta = rho g H Q / (T omega) = dp0 Q / (T omega)`;
* compressible stage (perfect gas): total-to-total pressure ratio `PR = p0_out / p0_in`, temperature ratio `tau = T0_out / T0_in`,
  isentropic efficiency (compressor) `eta_is = (PR^((g-1)/g) - 1) / (tau - 1)`, polytropic `eta_p = ((g-1)/g) ln PR / ln tau`;
* coefficients: head coefficient `psi = g H / u_t^2`, flow coefficient `phi = Q / (u_t A_ref)`, `u_t = omega r_ref`;
* torque on the fluid `T = - (M_wall . axis)` where `M_wall` is the moment of the fluid on the blade/hub walls from
  `integrals::wall_force_and_moment` (reaction torque; sign convention stated), power `T omega`;
* export: `to_csv` (`name,value,unit`, 17 significant digits) and `to_html` (escaped table).

D11 Not validated and not claimed: no turbomachinery flow solution is produced or compared with published pump or compressor data;
the checks are geometric and algebraic. Not implemented: blade-aligned, theta and cone-by-angle surfaces beyond the quadrics already
in `surfaces`, inlet-outlet / hub-shroud / circumferential line objects as case-tree entries, machine-type report templates in the
case tree, a real MRF rotor run, 360-degree reconstruction of fields (only surfaces).

## Registered checks (limits fixed before any run)

A1 Topology after refinement, on (i) a graded 6 x 5 x 4 box and (ii) a 10 x 10 x 1 slab (quadtree), after three rounds of
pseudo-random marks (fixed LCG; ~30 percent refine) with balance on and one round without balance: `Mesh::validate` is Ok; each
internal face occurs in exactly two cells' face lists, each boundary face in exactly one (independent count from the cell-face
lists); every cell volume > 0; `|sum V - Lx Ly Lz| <= 1e-13 Lx Ly Lz`; per cell `|sum +-S_f| <= 1e-13 sum |S_f|`; with balance on,
the maximum level difference across any internal face <= 1; refining one interior level-0 cell of an `n`-cell uniform mesh gives
exactly `n + 7` cells in 3-D (`n + 3` in the slab) and the neighbours become polyhedra with more than 6 faces.
A2 Transfer: after refine + coarsen cycles `|sum V q_new - sum V q_old| <= 1e-13 sum V |q|` (random field, both injection and
linear); a linear field `q = a + b . x` is reproduced by `Linear` refinement with `max |q_new - q(x_c)| <= 1e-12 (max q - min q)`,
and by restriction on coarsening to the same limit; refine then coarsen back of a field returns it to `1e-13` relative (linear);
coarsening is refused (no change) unless all siblings are marked.
A3 Criteria on a 40 x 40 x 1 slab (h = 0.025): (a) gradient, `phi = tanh((x - 0.5)/0.06)`, threshold `T = 2.0` on `|grad|`: every
cell whose analytic centre gradient is >= 1.3 T is marked, none whose analytic centre gradient is <= 0.7 T; (b) gradient of the
ramp (0 for x < 0.4, slope 5 on [0.4, 0.6], 1 beyond), threshold 2.5: no cell whose centre is farther than 1.01 h from the interval
[0.4, 0.6] is marked (gradient exactly 0), every cell with centre in [0.4 + h, 0.6 - h] is marked (gradient 5 to 1e-10); (c)
curvature, quadratic `phi = 0.5 a x^2 + b y^2 + c x y` (a = 3, b = 2, c = 1) on a uniform 12 x 12 slab: for cells at least two
cells from every wall `| |H|_F - exact | <= 1e-9 exact`; Gaussian `exp(-r^2 / (2 sigma^2))`, sigma = 0.1, 32 x 32: every cell whose
analytic `|H|_F` is >= 1.3 T is marked and none with <= 0.7 T (T = half of the peak analytic value); (d) ZZ: for a linear field
`eta <= 1e-12` in every cell (also boundary cells); for the tanh front the maximum of `eta` lies within 2 h of x = 0.5, the
maximum is >= 100 x the maximum over cells with |x - 0.5| > 6 w (w = 0.06), and all cells with `eta >= 0.5 eta_max` satisfy
|x - 0.5| <= 3 w; (e) region: box, sphere and slab marks equal the brute-force centre-inside set exactly; the `TriSurface` distance
indicator (unit cube surface, mesh inside the cube) marks exactly the cells with `min(x, 1-x, y, 1-y, z, 1-z) <= d` at d = 0.2;
(f) limiter: no cell at `max_level` is marked `Refine`, none at `min_level` marked `Coarsen`; (g) hysteresis: indicator
1.05 T refine, then 0.95 T and 0.6 T (dead band, coarsen_below = 0.5 T) give `Keep`, 0.4 T gives `Coarsen`; `coarsen_below >=
refine_above` and negative / non-finite thresholds are rejected with an error naming the argument; (h) combine: refine wins.
A4 Solve (rcfd-fv, steady diffusion with manufactured solution `phi = tanh((x - 0.5)/0.06) + 0.3 y`, `Gamma = 1`, Dirichlet on all four
sides, source `-Gamma lap phi` at the cell centres, deferred non-orthogonal correction iterations until the change <= 1e-12):
uniform 16 x 16 error E16, 32 x 32 E32, 64 x 64 E64 (volume-weighted L2); order `log2(E32/E64) >= 1.6`. Adapted mesh: 16 x 16 base,
two rounds of gradient-criterion refinement (balance on, level limit 2, the field being the coarse / interim solution); with
`E_a` its error: `E_a <= 0.35 E16`, `E_a <= 3 E64`, and `n_cells(adapted) <= 0.5 n_cells(64 x 64) = 2048`. (The criterion "adapted
cavity matches a fine uniform solution with fewer cells" is tested on this diffusion problem, not on the lid-driven cavity.)
A5 Polyhedral dual of (i) a Kuhn six-tet split of a 4 x 3 x 3 graded box and (ii) `tetrahedralize(icosphere(1, 2))`: `Mesh::validate`
Ok; number of cells = number of vertices; `|sum V_poly - sum V_tet| <= 1e-13 sum V_tet`; per cell closure `<= 1e-13 sum|S|`; every
internal face in exactly two cells' lists; per boundary tag the patch area equals the surface triangle area `<= 1e-13` relative;
bad inputs (a vertex used by no tet, a tet face shared by three tets) give an error naming the problem.
A6 Located errors for every public function: negative / non-finite thresholds, field length mismatch, `min_level > max_level`, marks
length mismatch, zero-size base grids, non-monotone coordinates.

T1 Span surfaces on `annulus_3d(n_theta, 6, 5, 0.3, 1.1, 2.0, 0, Uniform)`: cylinder `R = 0.75`: after snapping `max |r - R| <= 1e-12 R`,
area relative to `2 pi R L` (L = 2) `<= 0.005` for `n_theta = 48` and the error at 48 is `>= 3 x` the error at 96; curves
`R_h(z) = 0.4 + 0.1 z`, `R_t(z) = 1.0` at span 0.5 (`R = 0.7 + 0.05 z`): `max |r - R(z)| <= 1e-12`, area vs the cone frustum
`pi (R(0) + R(2)) sqrt(L^2 + (R(2) - R(0))^2)` relative `<= 0.005` at `n_theta = 48`.
T2 Streamwise surfaces: plane (hub and shroud points at the same z = 1): vertices `|z - 1| <= 1e-12` and area `(N/2) sin(2 pi/N) (Ro^2 - Ri^2)`
within `1e-11` relative with `Ri, Ro` the hub and shroud radii of the segment (clipped), N = 48; conical station from `H = (0.2, 0.5)` to
`S = (1.8, 1.0)` in `(z, r)`: vertices on the cone to `1e-12`, area `pi (r1 + r2) slant` within 0.005 relative.
T3 Pitch average (sampler path): `f(r, z) = 1 + 2 r + 3 z^2 r` returns `f` to `1e-14` relative for `n_theta` = 7; `A + B cos(N theta)` (A = 2, B = 0.7,
N = 12) returns `A` to `1e-14` absolute for (i) full circle with `n_theta` = 36, (ii) one pitch with 9 samples, (iii) full circle
`n_theta` = 5; the aliased case `n_theta` = 12 returns `A + B` to `1e-14` (documents the limitation). Mesh path (annulus 192 x 4 x 4):
`f = 1 + 2 z` gives `1 + 2 z` to `1e-10` and `A + B cos(12 theta)` with `n_theta` = 36 returns A to within `0.02 B`.
T4 Mass-flow weighting: uniform axial velocity `W = 3`, `rho = 2`, `f = 1 + 4 s` on `r_h = 0.5 .. r_t = 1.0`: `<f>_m` equals the closed form
`1 + 4 <s>`, `<s> = (r_h/2 + (r_t - r_h)/3)/(r_h + (r_t - r_h)/2)`, to `1e-13` relative; mass flow `rho W pi (r_t^2 - r_h^2)` to `1e-13`
(Simpson, 8 intervals); non-uniform `W = W0 (1 + s)` against the exact polynomial integrals `1e-13`; zero net mass flow is an error.
T5 Performance, pump `rho = 998.2`, `g = 9.81`, `Q = 0.05`, `dp0 = 2.5e5`, `omega = 300`, torque set from `eta = 0.8`: head, efficiency, power
recovered to `1e-14` relative; compressor `gamma = 1.4`, `PR = 2`, `eta_is = 0.85` and `eta_p = 0.88` closed forms round-trip to `1e-14`;
`eta_is > eta_p` relation is not asserted (it is `eta_is < eta_p` for a compressor; asserted instead); torque from
`wall_force_and_moment` on a box patch with uniform pressure matches `rho p A (y_c, -x_c, 0)` to `1e-12` relative; CSV round-trips
every value to `1e-15` relative; HTML escapes `<` and `&`; non-positive `Q`, `omega`, pressure ratio, non-finite inputs and
`tau <= 1` for compressor efficiency are located errors.

## Order of work

ADR (this file) committed first; then `adapt.rs` + tests, `polydual.rs` + tests, the rcfd-fv solve test, `turbo.rs` + tests; docs and
roadmap; the final gate on a `git archive` copy.

## Addendum 1 (written before the A4 run): details the registered A4 left open

* The A4 refinement threshold is relative: in each of the two rounds every cell with `|grad phi| >= 0.25 max|grad phi|` (least-squares
  gradient of the solution on the current mesh, `scale_by_length = false`) is marked `Refine`; no coarsening in A4; balance on.
* The solve is the steady diffusion operator `rcfd_fv::diffusion::laplacian` with the least-squares-gradient non-orthogonal correction
  re-evaluated each outer iteration (at most 200 outer iterations, stop at max change <= 1e-12), Dirichlet from the exact solution at
  the boundary face centres; `AmrMesh` base 16 x 16 on `[0,1]^2` with thickness 0.1 and `frontAndBack` empty.
* Error is the volume-weighted L2 norm of `phi_h(x_c) - phi(x_c)` at the cell centres.
* The cell count limit is read as `n_cells(adapted) <= 2048` (half of 64 x 64).

## Addendum 2 (written after the first A4 run, before the A4b run): A4 miss and the corrected set-up A4b

First A4 run (threshold 0.25 of the peak gradient, two rounds, balance on): uniform L2 errors E16 = 3.774e-2, E32 = 5.508e-3,
E64 = 1.329e-3 (order 32 to 64: 2.05); adapted mesh 1024 cells, E_a = 4.932e-3: `E_a/E16 = 0.131` (limit 0.35, met), cells 1024 (limit
2048, met), `E_a/E64 = 3.71` (limit 3, MISSED). The order check passed. The missed assertion moves unchanged into the ignored test
`a4_adapted_vs_fine_known_miss`.

Diagnosis (ignored test `a4_diag_error_by_level` in `rcfd-fv/tests/amr_diag.rs`): 72 percent of the squared error sits in the 192
unrefined level-0 cells, the largest cell errors (1.0e-2) are at |x - 0.5| = 0.086 where level 2 meets level 1. The gradient
criterion with a 0.25 relative threshold stops at |x - 0.5| of about 0.1, but the tanh shoulders still carry a large second
derivative (h/w = 1 on the base mesh), so the truncation error `h^2 phi''/12` of the coarse cells beside the band dominates. The
criterion's set-up (threshold fraction), not the refinement operator, is the cause: error per cell does not follow the gradient
magnitude but the second derivative.

Corrected set-up A4b (registered here, before it is run; the threshold fraction is chosen from the diagnosis, so this is NOT an
independent confirmation of the registered A4 factor): the same procedure with threshold `0.05 max|grad phi|` (|grad| >= 5 percent of the
peak extends the band to about |x - 0.5| = 3 w = 0.18). Limits identical to A4: `E_a <= 0.35 E16`, `E_a <= 3 E64`, `n_cells <= 2048`.
