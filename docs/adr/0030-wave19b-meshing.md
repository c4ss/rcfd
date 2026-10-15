# ADR 0030: Wave 19B, surface diagnostics and repair (F-232), surface remeshing and sizing (F-131), tetrahedral volume meshing (F-132)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new code was compiled or
run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves
to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect
gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes that were not fixed here) may be
appended at the end without touching registered limits.

Roadmap order: F-232 (first), F-131, F-132. Each stops at `partial` with named gaps (section "Not attempted"). Clean-room:
published algorithms and first principles only (Bowyer-Watson incremental Delaunay insertion; adaptive-precision floating-point
expansion arithmetic for exact predicates; Delaunay refinement by circumcentre insertion; incremental isotropic remeshing by edge
split, collapse, flip and tangential smoothing); nothing from vendor material, vendor installs or manuals; no vendor
names in the repository. Every expected number below is derived from definitions, closed forms or invariants; no registered check
depends on a literature value. Attributions to the literature are marked "(from memory, unverified)".

New library code lives in new files of `rcfd-mesh`: `surface.rs` (triangle surface type, generators, diagnostics, repair),
`predicates.rs` (exact orientation and in-sphere tests), `surface_remesh.rs` (size fields, remeshing), `tetmesh.rs` (Delaunay,
boundary recovery, refinement, conversion to `Mesh`). `lib.rs` gains four `pub mod` lines. The solver smoke test is a NEW file in
`rcfd-fv/tests` (it needs the solver; `rcfd-mesh` cannot depend on it).

## Part A: F-232, surface diagnostics and repair

### A1 Decisions

`TriSurface { points: Vec<Vec3>, tris: Vec<[usize;3]>, tags: Vec<u32> }`; triangles counter-clockwise seen from outside (outward
normal `(b-a) x (c-a)`). Welding from a soup uses exact bitwise vertex equality by default or a tolerance on a hash grid.

`diagnose(&TriSurface, &DiagOptions) -> Diagnostics` reports exact integer counts:
1. `n_points_used`, `n_tris`, `n_edges` (distinct undirected edges of used vertices), `euler = V - E + F`;
2. `free_edges` (used by exactly one triangle), `nonmanifold_edges` (used by 3 or more triangles), `multi_connected_vertices`
   (vertices whose incident triangles form more than one edge-connected fan);
3. `misoriented_edges` (edges of manifold-2 usage that both triangles traverse in the same direction);
4. `duplicate_faces` (triangles beyond the first that share the same sorted vertex triple);
5. `degenerate_tris` (repeated vertex id or area below `area_tol * (longest edge)^2`, default `1e-12`), `sliver_tris` (non-degenerate
   with smallest angle below `min_angle_deg`, default 1 degree), `small_edges` (length below `small_edge * mean edge length`, default 1e-3);
6. `open_loops`: the free edges chained into closed loops (vertex lists, each loop starting at its smallest vertex id, walking in the
   direction of the free half-edge); a loop that does not close is reported as `unclosed_chains`;
7. `components` (edge-connected triangle components), `enclosed_volume` (divergence theorem, valid for closed outward-oriented
   components), `area`.

`repair(&TriSurface, &RepairOptions) -> (TriSurface, RepairReport)` applies, in this fixed order: (1) remove degenerate triangles
(repeated ids and zero-area); (2) remove duplicate faces; (3) orient consistently by flooding each edge-connected component with
orientation propagation across manifold edges (seed: smallest triangle id of the component), then, for a closed component with
negative enclosed volume, flip it; (4) fill each open loop by ear clipping (the ear with the shortest closing diagonal whose triangle
is convex relative to the loop normal and contains no other loop vertex; ties by smallest first vertex id), orientation matching the
adjacent triangles; (5) compact unused vertices. Non-manifold edges and unclosed chains are reported, not repaired. Steps are
deterministic and independent of hash iteration order (sorted keys everywhere).

### A2 Closed forms used

* Icosphere of level `L` from the regular icosahedron by 1 to 4 midpoint subdivision with projection to the sphere of radius `r`:
  `F = 20 4^L`, `E = 30 4^L`, `V = 10 4^L + 2`, `chi = 2`. `L=2`: `V=162, E=480, F=320`.
* Cube of side `s` with `n x n` quads per face, each split by one diagonal: `F = 12 n^2`, `V = 6 n^2 + 2`, `E = 18 n^2`, `chi = 2`,
  area `6 s^2`, volume `s^3` (exact, all coordinates dyadic when `s` and `n` are powers of two; otherwise 1e-12 relative).
  `s=2, n=3`: `V=56, E=162, F=108`.
* Torus (major radius `R`, minor `r`, `nu x nv` quads each split in two): `V = nu nv`, `E = 3 nu nv`, `F = 2 nu nv`, `chi = 0`.
  `nu=24, nv=12`: `V=288, E=864, F=576`.
* Removing one triangle of a closed manifold leaves 3 free edges and one loop of length 3; filling the 3-loop restores exactly that
  triangle, so the volume returns to the original with relative error at most `1e-12`.
* Reversing one triangle of a closed manifold gives 3 misoriented edges; reversing `k` pairwise non-adjacent (no shared vertex)
  triangles gives `3k`.
* Appending a copy of one triangle gives 1 duplicate face and 3 edges of usage 4 (non-manifold).

### A3 Registered checks (tests in `crates/rcfd-mesh/tests/surface_diag.rs`)

* **S1** generators: icosphere `L=2` has `V,E,F = 162,480,320`, `chi=2`; cube `s=2, n=3` has `56,162,108`, `chi=2`, area 24 and
  volume 8 within 1e-12 relative; torus `R=2, r=0.5, 24 x 12` has `288,864,576`, `chi=0`; all three: 0 free, 0 non-manifold, 0
  misoriented edges, enclosed volume positive, one component.
* **S2** detector counts, exact integers, on the icosphere `L=2`: (a) remove triangles 0, 100, 200 (chosen pairwise vertex-disjoint;
  asserted in the test) gives `free_edges=9`, 3 open loops of length 3; (b) reverse 5 pairwise vertex-disjoint triangles gives
  `misoriented_edges=15`; (c) append copies of 4 distinct triangles gives `duplicate_faces=4`, `nonmanifold_edges=12`; (d) append the
  degenerate triangle `[a,a,b]` and one zero-area triangle `[p,q,r]` with collinear points gives `degenerate_tris=2`; (e) append one
  fin triangle on an existing edge with a new apex gives `nonmanifold_edges=1`. Cube `s=2,n=3`: remove one triangle gives
  `free_edges=3`, one loop.
* **S3** repair outcomes: after (a) on the icosphere and on the cube: `free_edges=0`, `chi=2`, `F` restored, volume relative change
  at most 1e-12. After removing the 6 triangles around one icosphere vertex (vertex id 0) a hexagonal loop of length 6 is reported and
  filled with 4 triangles: `F=318`, `chi=2`, closed manifold, volume change at most 0.5 percent (the roadmap acceptance bound; the
  expected value is far smaller). After (b): `misoriented_edges=0`, volume equals the original within 1e-12. After (c): the original
  triangle count, `nonmanifold_edges=0`. After (d): the original triangle count.
* **S4** repair is idempotent: `repair(repair(x)) == repair(x)` bit-identically (points and triangles) for each seeded case; repair of
  an already clean surface returns it unchanged with an all-zero report; running the same repair twice is bit-identical.
* **S5** a surface with every triangle reversed (inside-out closed component) is repaired to positive volume equal to the original
  within 1e-12 relative.
* **S6** small features: a long thin triangle with smallest angle 0.1 degree is counted in `sliver_tris` exactly once and not in
  `degenerate_tris`; an edge of length 1e-5 times the mean edge length is counted in `small_edges` exactly once.
* **S7** open surface passed to the volume mesher returns `Err` whose message contains the exact free-edge count (see T8).

## Part B: F-131, size fields and surface remeshing

### B1 Decisions

Size field `SizeField` is a sum type evaluated at a point with an optional surface vertex index: `Uniform(h)`, `Curvature { theta,
h_min, h_max }`, `Proximity { cells_per_gap, h_min, h_max }`, `Box { lo, hi, h }` (body of influence, hard override inside), and
`Min(Vec<SizeField>)`. The combined target at a vertex is the minimum. Curvature estimate at vertex `i`: `kappa_i = max over incident
edges (i,j) of angle(n_i, n_j) / |x_i - x_j|`, where `n` are area-weighted vertex normals (on a smooth surface this is the largest
normal-curvature estimate of the edge direction, equal to `1/rho` for a sphere of radius `rho` to the discretisation error). Curvature
size `h = clamp(theta / kappa, h_min, h_max)` (`theta` is the angular resolution in radians: a chord of angle `theta` has length
`2 rho sin(theta/2)`, about `theta rho`). Proximity size `h = clamp(d / cells_per_gap, h_min, h_max)` where `d` is the distance from
the vertex along the inward normal to the first hit on the surface (brute force over triangles; a vertex without a hit uses `h_max`).
Vertices on the surface have their target; an edge's target is the mean of its end targets; after each pass sizes are smoothed by
`h_i <- min(h_i, min_j (h_j + grad |x_i - x_j|))` with growth gradient `grad=0.5` so that transitions are bounded.

`remesh(&TriSurface, &SizeField, &RemeshOptions) -> (TriSurface, RemeshReport)`, incremental isotropic remeshing (published method,
from memory unverified): repeat `iterations` times: split every edge longer than `4/3` of its target at the midpoint; collapse edges
shorter than `4/5` of the target when the result edges stay below `4/3`, the link condition holds and no incident triangle normal turns
by more than 90 degrees; flip edges that reduce the sum of squared valence excess `(val - 6)^2` (when both adjacent triangles' normal
angle stays below `flip_angle` 30 degrees and the flip does not create an existing edge); tangential smoothing, where each vertex moves
toward the area-weighted centroid of its neighbours restricted to the tangent plane, then is projected to the closest point of the
ORIGINAL surface (brute force). Feature edges: edges with dihedral angle above `feature_angle` (default 40 degrees) or between triangles
of different tags. Feature vertices with 2 feature edges move only by collapse along the feature and are never smoothed; vertices with
other than 0 or 2 feature edges (corners) are fixed; feature edges are split at the midpoint, never flipped. Vertices created by a split
lie on the old edge and so on the old surface; smoothed vertices are projected to the original surface. Tags are inherited by split
and flip. Output is renumbered by first use for determinism. No random numbers.

### B2 Derivations

* Sagitta: a chord of length `l` on a circle of radius `rho` deviates by at most `rho - sqrt(rho^2 - l^2/4) ~ l^2/(8 rho)`; a
  triangle with circumradius `R_c` inscribed in a sphere of radius `rho` deviates at its centroid by `~ R_c^2/(2 rho)`, which is
  `l^2/(6 rho)` for an equilateral triangle of side `l`. With side up to `4/3 h` this is `0.30 h^2/rho`. The registered Hausdorff bound
  `0.4 h^2/rho` leaves room for the non-equilateral shapes in the output and the (smaller) deviation of the fine input from the true sphere.
* An inscribed polyhedron with a local depth `delta` loses volume `V ~ A delta / ... `: using the bound `delta <= 0.4 h^2/rho` over the
  whole surface, `dV/V <= 3 delta / rho = 1.2 (h/rho)^2` for a ball.
* Cube side 2, `h=0.5`: every feature line has 4 segments; flat faces are projected exactly onto themselves, so area is 24 and volume
  8 up to rounding (1e-9 allowed).
* The curvature estimate of an icosphere of radius `r`: the normal angle between neighbours over the chord length is
  `~ (l/r) / l = 1/r` with relative error `O((l/r)^2)`; on level 3 (`l ~ 0.11 r`) below 1 percent in theory, 10 percent allowed.
* Planar slab of thickness `t` = 0.2: a vertex on a large face at interior grid position has a perpendicular ray reaching the opposite
  face at exactly `t`.

### B3 Registered checks (tests in `crates/rcfd-mesh/tests/surface_remesh.rs`)

* **R1** curvature estimate: icosphere `L=3, r=1`: every vertex estimate within `[0.9, 1.1]`. Cylinder `r=0.5`, height 3, 48 around
  and 12 along, vertices on the lateral surface away from the end rims (|z| < 1): estimate within `[1.8, 2.2]` (= `1/r` +-10 percent).
* **R2** proximity: slab `4 x 4 x 0.2` with 8 x 8 quads on the large faces: for large-face vertices with |x|,|y| < 1.9 the thickness
  estimate equals 0.2 within 1e-9.
* **R3** uniform remesh of the icosphere `L=4, r=1` to `h=0.25`, 10 iterations: closed manifold, `chi=2`, 0 misoriented edges,
  every vertex within 1e-9 of the input surface, Hausdorff (centroids and edge midpoints of the result to the input surface) at most
  `0.4 h^2/rho = 0.025`, volume ratio within `1.2 (h/rho)^2 = 0.075` of the input volume (and not greater than the input volume by more
  than 1e-3), fraction of edges with `length/h` in `[0.7, 1.4]` at least 0.90, fraction of valence-6 vertices at least 0.60,
  smallest triangle angle at least 10 degrees, and at least 80 percent of triangles with smallest angle 30 degrees or more.
* **R4** feature capture: cube `s=2, n=2` remeshed to `h=0.5` with feature angle 40 degrees: all 8 corner points still vertices (exact
  coordinates), every vertex lies on a face plane, every vertex that is on a cube edge lies on that edge line, area 24 and volume 8
  within 1e-9 relative, `chi=2`, fraction of edges in `[0.7,1.4] h` at least 0.80.
* **R5** curvature sizing of the torus (`R=2, r=0.5`, 48 x 24 input) with `theta=0.6, h_min=0.05, h_max=0.8`, 8 iterations: closed,
  `chi=0`, no misoriented edges, fraction of edges within `[0.7, 1.4]` of their target at least 0.75, every vertex within 1e-9 of the
  input surface, volume within 3 percent of the input volume.
* **R6** determinism: the R3 result run twice is bit-identical (points and triangles).
* **R7** tags: the cylinder in a box surface remeshed with curvature sizing keeps both tags, and the number of tag-1 triangles
  is positive; the result is watertight (0 free, 0 non-manifold, 0 misoriented), `chi=4` (two spheres topologically), and the
  enclosed volume (box minus cylinder, divergence theorem) equals the input volume within 3 percent.

## Part C: F-132, tetrahedral volume mesh from a closed surface

### C1 Decisions

Method: incremental Delaunay tetrahedralisation (Bowyer-Watson) of the surface vertices inside a large bounding tetrahedron, with
exact orientation and in-sphere predicates (Shewchuk-style floating-point expansions; a floating-point filter first, exact fallback),
ties in the in-sphere test treated as outside with the cavity then grown across any boundary face coplanar with the new point so the
cavity stays strictly star-shaped. Boundary recovery by conforming Steiner insertion: every surface edge missing from the tetrahedralisation
is split at its midpoint (both adjacent surface triangles split), then every surface triangle whose edges are present but which is not a
face of the tetrahedralisation has its longest edge split; repeated until all surface triangles are faces; capped at a stated number of
insertions, otherwise an error. So the boundary of the volume mesh is a conforming subdivision of the input surface (same geometry,
more vertices). Cells are classified by flood fill from the tetrahedra on the inner side of the outward-oriented surface faces, never
crossing a surface face. Quality: Delaunay refinement, circumcentre insertion for inside tetrahedra with circumradius over shortest
edge above `max_radius_edge` (default 2.0) or circumradius above the size function; a circumcentre outside the domain or whose cavity
contains an outside tetrahedron is not inserted and the encroached surface triangle's longest edge is split instead. Capped. No sliver
removal (flips, smoothing) is implemented; this is a named gap and slivers are measured and reported, not hidden.
Conversion to `Mesh` via `Mesh::from_cell_faces`; boundary faces form patches by surface tag (`wall` for tag 0, `wall<tag>` otherwise).
Error contract: an input that is not a closed, consistently oriented, manifold surface returns `Error::Mesh` with a message containing
`<n> free edge(s)`, `<n> non-manifold edge(s)` or `<n> misoriented edge(s)` as applicable (counts from `diagnose`). Output is
deterministic: no hashing order dependence (sorted or index-ordered iteration), no random numbers (the point-location walk uses a
fixed-seed linear congruential generator).

### C2 Derivations

* Divergence theorem on the (triangulated, outward) surface: `V = (1/6) sum dot(a, cross(b, c))`. The volume mesh is a subdivision of
  the region bounded by the conforming surface; Steiner points lie on the input edges (midpoints) or in the interior, so the bounded
  region differs from the input region only by rounding of Steiner coordinates (1e-16 relative), far below the 1e-10 limit.
* The Euler characteristic of a compact 3-manifold with boundary is half that of its boundary: `V - E + F - C = chi(boundary)/2`
  (ball 1, solid torus 0, ball with one cavity 2).
* Tetrahedral mesh face counts: `4C = 2 F_int + F_bnd`; every cell has exactly 4 faces.
* Regular tetrahedron of edge `h` has volume `h^3 / (6 sqrt 2)` and circumradius `h sqrt(6)/4 = 0.612 h`.

### C3 Registered checks (tests in `crates/rcfd-mesh/tests/tetmesh.rs`; solver test in `crates/rcfd-fv/tests/w19b_meshing_solve.rs`)

* **T1** cube `s=2, n=2`: sum of cell volumes equals the divergence-theorem volume of the INPUT surface (8) within 1e-10 relative;
  every cell volume positive; every cell has 4 faces; boundary faces are all triangles lying in a cube face plane; total boundary area
  24 within 1e-10; each boundary face centroid lies on an input triangle within 1e-9; number of boundary faces at least the number of
  input triangles; `Mesh::validate` Ok; `check_mesh(...).is_ok()`; every internal face has two distinct cells (owner < neighbour).
* **T2** icosphere `L=2, r=1`: same volume and positivity checks (volume relative error at most 1e-10 against the divergence volume of
  the input), boundary area equals input area within 1e-10, `V - E + F - C = 1`, boundary is a closed 2-manifold (each boundary edge in
  exactly 2 boundary faces) with `chi=2`, `check_mesh` ok.
* **T3** torus (24 x 12): volume within 1e-10 of the input divergence volume, positive cells, `V - E + F - C = 0`.
* **T4** cylinder in box: box `4 x 4 x 2` (side normals outward), cylinder `r=0.5` axis z, `|z| <= 0.5`, 16 around, 4 along, normals
  into the cylinder (outward from the domain): volume within 1e-10 of the input divergence volume, positive cells,
  `V - E + F - C = 2`, two boundary patches (box tag 0, cylinder tag 1), `check_mesh` ok.
* **T5** quality (cube, sphere, torus; recorded per case): after refinement with `max_radius_edge=2.0`, at least 99 percent of
  cells have radius-edge ratio at most 2.0; minimum dihedral angle over all cells at least 3 degrees; minimum cell volume
  positive. Misses (slivers are expected without a sliver-removal stage) are recorded as measured.
* **T6** size control: unit icosphere `L=2` with uniform size `h=0.3`: every cell circumradius at most `1.5 h`; cell count at least 0.3
  and at most 3.0 times `V_sphere / (h^3/(6 sqrt 2))` (nominal regular-tetrahedron count). The volume equality of T2 holds.
* **T7** determinism: two runs on the same input give equal `content_hash`, equal point and cell arrays bit for bit.
* **T8** located errors: the icosphere `L=1` with 1 triangle removed returns `Err` containing `3 free edge`; with 2 vertex-disjoint
  triangles removed `6 free edge`; a surface with one reversed triangle returns `Err` containing `3 misoriented edge`.
* **T9** exact predicates: `orient3d` and `insphere` agree with an exact big-rational reference on 2000 pseudo-random near-degenerate
  configurations (coplanar and cospherical integer points perturbed by one ulp, reference computed in `i128` on small integers);
  they return exactly 0 for exactly coplanar and exactly cospherical integer points.
* **T10** solver smoke (`rcfd-fv`, new test file): Laplace problem `div grad u = 0` on the tetrahedral unit-sphere mesh (T2 surface
  refined to `h=0.25`, at least 1000 cells), Dirichlet `u = x^2 - y^2` (harmonic) on all boundary faces evaluated at the face
  centre, solved with the existing diffusion operator and linear solver; cell-centre values against `x^2 - y^2` at cell centres:
  volume-weighted relative L2 error (against the RMS of the exact values) at most 0.05. The same on the cube mesh with the
  linear harmonic `u = x + 2y - z`: relative L2 error at most 0.05. The meshes pass `check_mesh`.

## Not attempted (stated in advance)

* F-102 polyhedral dual conversion and solution-based adaption; F-234 Cartesian-core fill with prism layers; F-235 sweep and
  multizone hex meshing; shrink-wrap/leak-tolerant wrapping and self-intersection detection (F-232 stays partial); sliver removal
  (flips, smoothing, optimisation) in the tetrahedral mesher; anisotropic size fields; CLI and case-file hooks (library level only).
* No claim is made that F-131/F-132/F-232 acceptance sentences are met; they stay `partial`.

## Hygiene

Format only own files with `rustfmt <file>`. Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) are not touched and not
re-recorded. On a `git archive` of the final commit: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace --release --no-fail-fast`. Scratch files stay in `<scratch dir>`.

## Results

Measured values against the registered limits (release build, all runs deterministic).

| Check | Registered limit | Measured | Verdict |
| --- | --- | --- | --- |
| S1, S2, S4-S7 (surface diagnostics, repair) | as registered | all pass | pass |
| S3 repair outcomes | F=318, chi=2, volume within 0.5 percent | see "Set-up defect" below | old test kept ignored, corrected test passes with the same limits |
| T1 cube | as registered | pass | pass |
| T2 sphere | volume equality, chi, `check_mesh` | 320 cells, volume error <= 1e-10, V-E+F-C = 1 | pass |
| T3 torus | V-E+F-C = 0 | 0 | pass |
| T4 cylinder in box | V-E+F-C = 2, two patches | 2, two patches | pass |
| T5 cube | radius-edge fraction >= 0.99, min dihedral, min volume | fraction 1.0, min dihedral 29.5 deg | pass |
| T5 sphere | same | fraction 1.0, min dihedral 55.05 deg | pass |
| T5 torus | radius-edge fraction >= 0.99 | 0.634 (min dihedral 6.8 deg, min volume 5.7e-4 positive) | MISS, `t5_torus_known_miss` (ignored) |
| T6 size | cells within 0.3-3.0 of nominal, circumradius <= 1.5 h | 823 cells vs nominal 1271.9, max circumradius within 1.5 h | pass |
| T7 determinism | bit equal | equal | pass |
| T8 located errors | "3 free edge", "6 free edge", "3 misoriented edge" | all three messages | pass |
| T9 exact predicates | 2000 near-degenerate cases, exact zeros | 4000 cases incl. many exact degeneracies | pass (file and reference differ, see below) |
| T10 sphere | relative L2 <= 0.05, >= 1000 cells | 1277 cells, 1.10e-2 (max error 1.24e-2) | pass |
| T10 cube | relative L2 <= 0.05 | 597 cells, 4.9e-14 | pass |
| R1 curvature | sphere 0.9-1.1, cylinder 1.8-2.2 | within | pass |
| R2 proximity | 0.2 within 1e-9 | within | pass |
| R3 uniform sphere | Hausdorff <= 0.4 h^2 = 0.025; edge fraction >= 0.90; valence 6 >= 0.60; min angle >= 10; fraction >= 30 deg >= 0.80 | 0.01473; 0.990; 0.815; 44.5 deg; 1.0; 536 triangles | pass |
| R4 cube features | corners kept, edge fraction >= 0.80 | 274 triangles, 0.990, area and volume exact | pass |
| R5 torus | closed, chi = 0, edge fraction >= 0.75, vertices within 1e-9 | closed, 0, 0.988, within | pass |
| R5 torus volume | change <= 3 percent | 4.18 percent | MISS, `r5_volume_known_miss` (ignored) |
| R6 determinism | bit equal | equal | pass |
| R7 tags | tags kept, chi = 4, two components, volume within 3 percent | 372 triangles, all hold | pass |

### Addenda and disclosures

* **S3 set-up defect.** The registered text says vertex 0 of the icosahedron-based sphere has a six-triangle fan with four fill
  triangles. Vertex 0 has valence 5 (five-triangle fan, three fill triangles; F = 318 coincidentally the same). The old test
  `s3_repair_outcomes` is kept `#[ignore]` as a record; the new test `s3c_repair_outcomes` uses vertex 12 (valence 6) with the
  registered limits unchanged (F = 318, chi = 2, volume within 0.5 percent) and passes.
* **T9 location and reference.** The test is in `crates/rcfd-mesh/tests/predicates.rs`, not `tetmesh.rs`. The reference is exact
  `i128` arithmetic on lattice points `k 2^-30` with small integer `k` (not "one-ulp perturbed" points); many cases are exactly
  degenerate. 4000 cases instead of 2000.
* **T5 torus miss, diagnosed not tuned.** Input is the coarse anisotropic 24x12 torus (edges 0.26-0.65 across a tube of diameter
  1.0). Circumcentres of bad tetrahedra encroach on the surface and boundary splitting is disabled by default (see next point),
  so the radius-edge bound cannot be reached. The limit is kept at 0.99. Non-registered extra test `t5b_torus_after_remesh`
  (surface first remeshed with F-131): 1297 cells, radius-edge fraction 1.0, min dihedral 5.26 deg.
* **R5 volume miss, diagnosed.** The size field is theta/kappa with theta = 0.6 rad per segment; an inscribed polygon with that
  angle has area ratio sin(0.6)/0.6 = 0.941, i.e. 5.9 percent loss in the tube cross-section, so the registered 3 percent was
  under-derived. The ignored test keeps the 3 percent. Post-hoc, non-registered test `r5b_volume_matches_the_angle_derived_bound`
  checks the change is at most 1 - sin(theta)/theta (passes at 4.18 percent).
* **Non-registered extra test** `p1_pipeline_cylinder_in_box` (remesh then tetrahedralize then `to_mesh`): 1061 cells, radius-edge
  fraction 0.927, min dihedral 3.34 deg, volume and `check_mesh` ok. Sliver tetrahedra with small dihedral angles remain; there is
  no sliver exudation or optimisation.
* **Algorithm additions beyond ADR text.** (i) `TetOptions::split_boundary` (default false): splitting surface edges during
  refinement ran away on small-angle surfaces (Steiner budget exhausted), so it is experimental and off; (ii) bistellar
  improvement (2-3 flips and edge removal for 3..7 incident tetrahedra, `sliver_q = 0.03`) and (iii) `split_flat_boundary`
  (flat tetrahedra with two surface faces: split the shared surface edge) were added after flat boundary slivers appeared on the
  torus (125 of 1061 tetrahedra, down to 3 after flips, 0 after the flat split). `TetReport` gained `flips` and `flat_split`.
* **Remesher choices.** Curvature uses vertex normals as registered; closest-point queries are brute force (O(n) per query, fine
  to a few thousand triangles, not for large surfaces); tag boundaries are treated as feature edges.
* **Existing clippy findings** (newer toolchain) in two lines of my earlier committed files were fixed in the F-131 commit.

### Unverified references

All literature attributions in the code and docs (Bowyer-Watson insertion, Shewchuk-style adaptive predicates, Delaunay
refinement radius-edge criterion, Botsch-Kobbelt style incremental remeshing, bistellar flips) are from memory; no primary source
was consulted. The implementations are tested against the registered closed-form and exact-arithmetic checks, not against the
papers.

### Not delivered

F-102 polyhedral dual and solution-based adaption, F-234 (Cartesian core with prism layers), F-235 (sweep and multizone hex),
shrink-wrap and self-intersection detection, sliver optimisation beyond flips and flat splitting, anisotropic sizes, boundary
layers, CLI and case-file hooks. F-131, F-132 and F-232 stay `partial`.
