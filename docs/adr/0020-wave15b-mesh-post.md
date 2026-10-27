# ADR 0020: Wave 15B, partitioning and reordering (F-236), boundary-layer inflation on unstructured patches (F-108), derived fields and sampling (F-072)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any of the new tests was compiled
or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion
moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test
set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes that were not fixed
here) may be appended at the end without touching registered limits.

Roadmap order: F-236 (first), F-108, F-072. F-103 and F-125 are NOT attempted, see "Not attempted". Clean-room: published
methods and first principles only (recursive coordinate and inertial bisection, Kernighan-Lin / Fiduccia-Mattheyses style
boundary refinement, Cuthill-McKee ordering, least-squares gradients); nothing from vendor material, vendor
installs or vendor manuals; no vendor names in the repository. Every expected number below is derived here from definitions,
closed forms or invariants; no registered check depends on a literature value. Statements attributed to the literature are
marked "(from memory, unverified)" and are not used as limits.

Shared notation: a mesh partition assigns each cell `c` a part `P(c)` in `0..n`. The edge cut is the number of internal faces
whose two cells have different parts. Part weight `W_p` is the sum of the cell weights `w_c` over `P(c) = p` (unit weights
unless stated); imbalance is `max_p W_p / (W/n) - 1`.

## Part A: F-236, partitioning strategies and reordering

### A1 Decisions

All strategies implement the existing `Partitioner` trait (`rcfd-mesh::partition`), which is unchanged; `Rcb`, `Decomposition`
and their tests are untouched. New items (module `partition`, appended):

1. `Bisection { rule, weights }`: recursive bisection with a proportional split (`n/2` and `n - n/2` parts take the matching
   weight fraction; the cut is the prefix of the sorted order whose weight is nearest the target, with at least one cell per
   remaining part). `rule`: `Longest` (axis of largest extent of the sub-set, ties by lowest axis; the same rule as `Rcb`, now
   weighted), `Cycle` (axes x, y, z in turn by recursion depth), `Fixed(axis)`, `Inertial` (key = projection of the cell
   centres on the principal axis of largest eigenvalue of the weighted covariance matrix, found by Jacobi rotations of the 3x3
   matrix). Ties in the sort key are broken by cell id, so results are deterministic.
2. `Sectors { kind, origin, axis, reference, weights }`: `Radial` key = distance of the cell centre from the line through
   `origin` along `axis`; `Circumferential` key = angle in `[0, 2 pi)` of the centre projected on the plane normal to `axis`,
   measured from the unit `reference` direction (projected into the plane) counter-clockwise about `axis`. Cells are sorted
   by key (ties by cell id) and cut into `n` consecutive bands of equal weight (prefix sums, nearest cut).
3. `GraphPartitioner { weights, tolerance, max_passes }`: graph method on the cell-face adjacency graph. Seed: weighted
   `Bisection::Longest`. Then k-way boundary refinement in the manner of Kernighan-Lin / Fiduccia-Mattheyses (from memory,
   unverified): in each pass, visit boundary cells in a fixed order; moving cell `c` from part `a` to the neighbouring part `b`
   has gain `g` = (faces of `c` to `b`) minus (faces of `c` to `a`); the move is made if `g > 0` and the weight of `b` stays at
   most `(1 + tolerance) * W/n` (and part `a` keeps at least one cell). Passes repeat until no move or `max_passes`. Each move
   strictly reduces the cut, so the cut of the result never exceeds the seed's cut by construction. This is a refinement-based
   graph partitioner, not a multilevel one (no coarsening); this is a stated limitation.
4. `smooth_partition(mesh, &mut part, n, weights, tolerance, passes) -> SmoothReport { moved, passes }`: the same move rule as
   3, applied to every cell (also interior islands), used to clean ragged or disconnected pieces of any strategy's output. An
   island cell with all neighbours in another part has a gain equal to its number of faces.
5. Helpers: `edge_cut(mesh, part)`, `part_weights(part, n, weights)`, `weighted_imbalance(part, n, weights)`.

Linear algebra (`rcfd-linalg::reorder`, new module): `rcm_order(n, lower, upper) -> Vec<usize>`, reverse Cuthill-McKee on the
graph of an `LduAddressing`-style face list (`order[i]` is the old index placed at new position `i`): the start of every
connected component is a pseudo-peripheral node found by repeated breadth-first search from a minimum-degree node, moving to
the minimum-degree node of the last level until the eccentricity stops increasing; breadth-first numbering with neighbours
visited in ascending degree (ties by id); the final sequence is reversed. Also `bandwidth(lower, upper, new_to_old)`,
`CsrMatrix::bandwidth()` and `CsrMatrix::permute_symmetric(new_to_old)`.

The mesh and solvers are not renumbered anywhere (that would change summation order and baseline hashes): the ordering is a
library facility. No case-schema, CLI, GUI or service surface is added for partitioning or reordering in this wave.

### A2 Derivations used for the limits

* Partition cover: every cell has exactly one part in `0..n`.
* Count balance: sorted-prefix splitting gives each part `floor(N/n)` or `ceil(N/n)` cells for the sector strategies (single
  level band cut), so imbalance is at most `1/floor(N/n)`. For recursive bisection the rounding errors halve at each level,
  so the part size error is below 1 cell. With 576 cells and 16 parts (36 cells each) the bound is 0.028, below the
  registered 0.05.
* Weighted balance: with cell weights at most 4 and one cut per recursion level at the prefix nearest the target, the error of
  one cut is at most half of the largest weight (2); the error carried to a leaf is a geometric sum below
  `2 (1 + 1/2 + ...) = 4`, so for mean part weight above 100 the imbalance is below 0.04.
* Slab cut: for an `nx x ny x nz` hexahedral grid cut into two slabs across the longest axis, the cut is `ny * nz` (faces of
  the cross section). Box `[20,10,5]`: 50.
* Inertial split of a tilted slab: for a uniform grid box rotated rigidly, the covariance of cell centres is that of the box
  rotated, so its largest eigenvector is the long axis; splitting at the median of the projection separates the first half of
  the slices from the second half: the cut is the cross section `ny * nz` exactly when the slice count on each side is equal.
  Box `[32,4,2]` rotated by 30 degrees about z: cut `4 * 2 = 8`. A coordinate bisection of the same tilted mesh cuts through
  slices obliquely and must cut strictly more than 8 faces (positive control).
* Circumferential quadrants: the 8x8x1 grid with cell size 1 centred on the origin has centres `(i + 1/2, j + 1/2)`,
  `i, j` in `-4..3`; no centre lies on a coordinate axis, so the four sectors (reference `+x`, axis `+z`, 4 parts, 16 cells
  each) are exactly the four open quadrants: part 0 the cells with `x > 0, y > 0`, part 1 `x < 0, y > 0`, part 2
  `x < 0, y < 0`, part 3 `x > 0, y < 0`.
* Reverse Cuthill-McKee bandwidth bound: a breadth-first level ordering has bandwidth at most `max_k (|L_k| + |L_{k+1}|) - 1`.
  For an `n1 x n2 x n3` grid (`n1 >= n2 >= n3`) with the start at a corner, the level sets are the planes `i + j + k = s` and
  each meets every line in the `n1` direction at most once, so `|L_k| <= n2 * n3` and the bandwidth is at most
  `2 n2 n3 - 1`. (The start node is a corner when the pseudo-peripheral search converges to a node of maximum eccentricity; a
  start elsewhere can only be tested, not assumed. If the registered bound is exceeded that is a miss to be diagnosed.) Path
  graph: breadth-first search from an end gives bandwidth 1.
* Natural ordering of `box_mesh` is x fastest, then y, then z, so the natural bandwidth of an `nx x ny x nz` box is
  `nx * ny` (the z neighbour); the measured natural bandwidth is reported and used in the comparison, not assumed.
* Symmetric permutation identity: with `B = P A P^T` and `(P x)_i = x_{order[i]}`, `B (P x) = P (A x)` for every `x`.

### A3 Registered checks (F-236)

Reference meshes: `M1 = box_mesh([20,10,5],[2,1,0.5],Uniform)` (1000 cells),
`M2 = box_mesh([24,12,6],[3,1.5,0.75],[Expansion(5),Expansion(3),Uniform])` (1728 cells, graded),
`M3 = cylinder_ogrid(48,12,1,5,Expansion(4),1)` (576 cells, curved). Part counts `n = 2..=16`, every integer. Tests in
`crates/rcfd-mesh/tests/partition_strategies.rs`. Strategy set `S`: `Rcb`, `Bisection{Longest, Cycle, Fixed(0), Inertial}`,
`Sectors{Radial, Circumferential}` about the z axis through the bounding-box centre with reference `+x`, `GraphPartitioner`
(tolerance 0.03, 20 passes).

* **P1 cover**: for every strategy in `S`, mesh and `n`: length `N`, all values `< n`, every part non-empty,
  `Decomposition::from_assignment` succeeds, and its `edge_cut` equals an independently counted number of unordered
  neighbouring cell pairs with different parts (exact, difference 0).
* **P2 balance**: unit weights, imbalance `<= 0.05` for every strategy, mesh and `n` (all 3 x 15 x 8 cases).
* **P3 node weighting**: `w_c = 4` for cells whose centre x lies beyond the mid-plane of the bounding box, else 1, on M1 and M2:
  weighted `Bisection{Longest, Cycle, Inertial}`, `Sectors{Circumferential}` and `GraphPartitioner` give weighted imbalance
  `<= 0.05` for `n = 2..=16`; positive control: the unweighted `Rcb` on the same weights has weighted imbalance `> 0.05` for
  at least one `n` on M1.
* **P4 edge cut (the roadmap criterion)**: `cut(GraphPartitioner) <= cut(Rcb)` for every mesh and `n = 2..=16` (exact
  integers). Also `GraphPartitioner` on M1 with `n = 2`: cut `<= 50` (slab). The cuts of the other coordinate variants and of
  the sectors are printed, not asserted.
* **P5 smoothing**: (a) M1, two slabs (x < 1 and x >= 1), then 5 cells of each part, chosen deep inside the part (at least two
  cells from the interface and from each other, positions fixed in the test), assigned to the other part;
  `smooth_partition` (tolerance 0.05) restores the slab assignment exactly, `moved == 10`, cut 50. (b) for every strategy in
  `S`, every mesh, `n = 2..=16`: after smoothing the cut is `<=` before, the cover is intact and the weighted imbalance is
  `<= max(before, 0.05) + 1e-12`.
* **P6 sectors**: for `Radial` and `Circumferential` on M1 and M3, `n = 2..=16`: the key bands are monotone (max key of part
  `p` `<=` min key of part `p+1`, exact) and the part sizes differ by at most 1 (unit weights). The quadrant closed form of A2
  holds exactly (the 8x8x1 mesh is `box_mesh([8,8,1],[8,8,1])` shifted by `gen::warp`).
* **P7 inertial**: box `n=[32,4,2]`, lengths `[32,4,2]`, rotated 30 degrees about z via `gen::warp`: `Bisection::Inertial`
  with `n = 2` cut is exactly 8 and the parts are the two halves of the slices (64 cells each); positive control: `Rcb` cut
  `> 8`.
* **R1 order validity** (`crates/rcfd-linalg/tests/reorder.rs`): `rcm_order` returns a permutation of `0..n` for a path, a
  disconnected graph (two paths and an isolated node) and a grid; repeated calls give identical output.
* **R2 path**: a path graph of 200 nodes with labels shuffled by a fixed linear congruential sequence has RCM bandwidth 1; the
  shuffled bandwidth is at least 100 (positive control).
* **R3 stretched meshes** (`crates/rcfd-fv/tests/reorder.rs`, adjacency from the mesh internal faces): `box_mesh([40,8,8])`
  (sorted dims 40, 8, 8): RCM bandwidth `<= 2 * 8 * 8 - 1 = 127` and strictly less than the natural bandwidth of the mesh
  numbering (reported; expected `nx * ny = 320`); `box_mesh([40,6,1])`: bound `2 * 6 * 1 - 1 = 11` and strictly less than
  natural (expected 40). The same two meshes with the cell labels shuffled by a fixed sequence: RCM bandwidth satisfies the same
  bound and is `<=` shuffled bandwidth / 4.
* **R4 permutation identity**: CSR matrix from the LDU Laplacian of `box_mesh([10,6,3])`, a fixed non-constant `x`:
  `B (P x)` equals `P (A x)` to `1e-12` relative (infinity norm); `B.bandwidth()` equals the face-list bandwidth under the order;
  the bandwidth of `A` equals `max(upper - lower)` of its addressing.
* **H1** `baseline_hashes` passes unmodified.

## Part B: F-108, inflation on unstructured wall patches

### B1 Decisions

New module `rcfd-mesh::inflate` and a refactor of `extrude_patch` in `edit.rs` into a private core that takes per-layer,
per-point distances along the point normals (the `Normal` case of `extrude_patch` keeps its exact arithmetic,
`frac = k / layers`, so existing results do not change; the existing `edit.rs` tests must pass unchanged).

* Point normal `n_p`: unit vector of the sum of the unit outward area vectors of the patch faces incident to `p` (as
  `extrude_patch`). Layers are added on the outward side of the patch (the side outside the domain). Filling a domain
  interior with prisms from a wall patch and merging with the bulk mesh is the volume-fill task of F-234 and is NOT done here.
* Local size `s_p` = mean over the patch faces incident to `p` of `sqrt(face area)`.
* `LayerSizing`: `Uniform { total, layers }` (`h_i = total/layers`); `FirstGrowth { first, growth, layers }`
  (`h_i = first * growth^i`); `AspectRatio { aspect, growth, layers }` (first height at point `p`: `s_p / aspect`, then growth);
  `LastRatio { first, last_ratio, layers }` (heights `first * g^i` with `g = (last_ratio * s_p / first)^(1/(layers-1))`, so
  the last layer is `last_ratio * s_p`; needs `layers >= 2`). The cumulative distance after layer `k` at point `p` is the
  partial sum of its heights.
* `inflate_patch(mesh, patch, sizing) -> (Mesh, InflateReport)`; `InflateReport { layers, first_height_min/max,
  last_height_min/max, thickness_min/max, growth_min/max (ratio of successive layer heights over all points), quality:
  QualityReport }` with `Serialize`/`Deserialize`. Errors (located): zero layers, non-positive size, growth `<= 0`, missing
  patch, periodic mesh, `LastRatio` with one layer.
* The y+ target is turned into a first height by the existing `first_cell_height_from_y_plus`; the caller passes the result as
  `first`.

### B2 Derivations

* Flat patch of area `A` with all normals `n`: layer `k` lies at distance `cum_k = sum_{i<k} h_i` and the added volume is
  `A T`, `T = sum h_i`; for `FirstGrowth`, `T = first (g^n - 1)/(g - 1)`. The lateral (side) area is perimeter times `T`.
* Cylinder O-grid inner wall (outward normals point to the axis): the wall is a regular `N`-gon of circumradius `R`, vertex
  normals are exactly radial, so the layers are homothetic polygons of circumradius `R - cum_k`; the area of a regular `N`-gon
  is `(N/2) R^2 sin(2 pi/N)`; added volume `depth (N/2) sin(2 pi/N) (R^2 - (R - T)^2)`.
* Triangulated unit square (each square of a `6 x 6` grid split by a diagonal): 72 triangles of area 1/72 each;
  `s_p = sqrt(1/72)` at every point.
* Last ratio, box case (`s = 1`, `first = 0.01`, `last_ratio = 0.5`, 6 layers): `g = 50^(1/5)`.
* First-layer cell of a unit-cross-section box with first height `h`: edge ratio `1/h`, so `max_aspect_ratio = 100` for
  `h = 0.01` when no other cell is more stretched (the original cells are 1x1x1 and the other layers are less stretched).

### B3 Registered checks (F-108), tests in `crates/rcfd-mesh/tests/inflate.rs`

* **I1 block patch**: `box_mesh([4,3,2],[4,3,2],Uniform)` patch `zmax` (area 12), `FirstGrowth{0.01, 1.2, 8}`: cells
  `24 + 96`, total volume `24 + 12 T` (relative `1e-12`), the 8 layer planes at heights `2 + cum_k` (maximum error
  `<= 1e-12`), cap area 12 and `zmax_side` area `2 (4 + 3) T` (relative `1e-12`), report first and last heights `0.01` and
  `0.01 * 1.2^7` (relative `1e-12`), growth min and max `1.2` (absolute `1e-12`), `quality.is_ok()`,
  `quality.max_aspect_ratio` equals 100 (relative `1e-9`), `quality.max_non_orthogonality <= 1e-4` degrees (exact 0; the
  existing measure carries `acos` noise of order 1e-6 degrees, disclosed in ADR 0018) and `max_skewness <= 1e-9`.
* **I2 unstructured (triangle) patch**: prism mesh from the 6x6 triangulated unit square, thickness 1 (built with
  `Mesh::from_cell_faces`), patch `zmax` of 72 triangles. `FirstGrowth{0.02, 1.3, 5}`: cells `72 + 360`, volume `1 + T`
  (relative `1e-12`), cap area 1, `zmax_side` area `4 T` (relative `1e-12`), every new cell has 5 faces and 6 distinct points,
  `validate()` ok and `check_mesh` ok.
* **I3 curved patch**: `cylinder_ogrid(24,3,1,2,Uniform,0.5)` patch `cylinder`, `FirstGrowth{0.05, 1.25, 4}`: added volume
  equals the closed form of B2 (relative `1e-10`); every point of layer `k` has radius `1 - cum_k` (absolute `1e-12`);
  `check_mesh` ok.
* **I4 sizing types** on the box patch of I1 and on the triangle patch of I2: `Uniform{0.3, 6}`: all layers `0.05` (absolute
  `1e-12`); `AspectRatio{20, 1.15, 5}`: first height `s/20` with `s = 1` (box) and `sqrt(1/72)` (triangles) (absolute
  `1e-12`), growth `1.15` (absolute `1e-12`); `LastRatio{0.01, 0.5, 6}` on the box patch: first `0.01`, last `0.5` (absolute
  `1e-12`), growth `50^0.2` (absolute `1e-12`).
* **I5 invalid input**: zero layers, `first <= 0`, `growth <= 0`, unknown patch, periodic mesh, `LastRatio` with
  `layers = 1` all return `Err` (positive control: the happy path is `Ok`).
* **I6 y+ target**: `first = first_cell_height_from_y_plus(0.8, 0.05, 1e-5) = 3.2e-4`; inflating the triangle patch with it,
  the report first height is `3.2e-4` (relative `1e-12`) and `y_plus_of_first_cell(report.first_height_min, 0.05, 1e-5) = 0.8`
  (relative `1e-12`).
* **I7 report**: JSON round trip equal; for the I1 mesh `thickness_min == thickness_max == T` (absolute `1e-12`).
* **H1** the existing `edit.rs`, `layers.rs` tests and `baseline_hashes` pass unchanged.

The F-108 roadmap acceptance mixes a solved check (met in wave 7 for block meshes) with "works on unstructured wall patches"
and "layer quality metrics reported". After this wave the status is decided against the full text and title, not these tests
alone; the volume fill with a bulk mesh (F-234) and a CLI/schema report surface are not attempted.

## Part C: F-072, derived fields and sampling

### C1 Decisions

New module `rcfd-post::derived` (post already depends on fv and mesh). Fields are cell arrays with boundary-face value arrays
(index `f - n_internal`); pressure is kinematic where the solver's is.

* `velocity_gradient(geom, u, u_bnd) -> Vec<[Vec3; 3]>`: `G[c][k]` = gradient of component `k` from
  `rcfd_fv::gradient::least_squares`; `A_kj = G[c][k][j] = d u_k / d x_j`.
* `vorticity = (A_32 - A_23, A_13 - A_31, A_21 - A_12)` (1-based `A_kj`); `q_criterion = 0.5 (|Omega|^2 - |S|^2)
  = -0.5 sum_{kj} A_kj A_jk`, with `S`, `Omega` the symmetric and skew parts.
* `wall_shear(geom, patch, u, rho, nu)` per patch face: force per unit area of the fluid on the wall in the wall tangent plane,
  `tau = rho nu (du/dn)_wall` with the one-sided estimate `(u_c - u_wall)/dn` (`dn` = `geom.dn`) and the normal component
  removed; `y_plus` per wall face `= dn sqrt(|tau|/rho) / nu`.
* `mach(u, c)` with `c = sqrt(gamma R T)` per cell; `total_pressure_incompressible = p + 0.5 rho |u|^2`;
  `total_pressure_isentropic = p (1 + (gamma-1)/2 M^2)^(gamma/(gamma-1))`.
* Sampling: `locate_cell(geom, point) -> Option<usize>` (a convex cell contains the point when
  `(x - x_f) . S_f <= 1e-12 |S_f| L` for every face with outward `S_f`; nearest-centre cell first, then all cells);
  `sample_line(geom, values, grad, p0, p1, n)` and `sample_plane(geom, values, grad, origin, e1, e2, extent1, extent2, n1, n2)`
  (cell midpoints), value `phi_c + grad_c . (x - x_c)`; `SampleSet::to_csv()` writes a header `s,x,y,z,<name>,inside` (line)
  or `x,y,z,<name>,inside` (plane), one row per sample, values with Rust's shortest round-trip format; points outside the mesh
  get an empty value field and `inside` 0. No case-schema, CLI, GUI or service surface in this wave (library level).

### C2 Derivations

* Linear velocity `u = A x + b`: the least-squares gradient with exact face values is exact at every cell, so vorticity and
  `Q` are constants. With `A = [[0.3,-1.2,0.5],[0.9,0.1,-0.4],[-0.7,0.6,-0.2]]`:
  `omega = (A_32 - A_23, A_13 - A_31, A_21 - A_12) = (0.6 + 0.4, 0.5 + 0.7, 0.9 + 1.2) = (1.0, 1.2, 2.1)`;
  `Q = -0.5 sum A_kj A_jk = -0.5 (0.09 + 0.01 + 0.04 + 2 (-1.08 - 0.35 - 0.24)) = -0.5 (0.14 - 3.34) = 1.6`.
* Taylor-Green `u = (sin x cos y, -cos x sin y, 0)`: `omega_z = 2 sin x sin y`, `Q = sin^2 x sin^2 y - cos^2 x cos^2 y`.
  On a uniform grid with cell-centre point values the least-squares gradient away from boundaries equals the central
  difference (diagonal normal matrix), whose error is at most `h^2/6 max|f'''|` per entry: `|delta omega_z| <= h^2/3`,
  `|delta Q| <= h^2` (first-order propagation `4 h^2/6 = 2 h^2/3` plus higher orders). Interior = cells at least 2 away from
  the x and y boundaries. Second order: the error ratio from `h` to `h/2` is at least 3.5 (asymptotically 4).
* Couette `u_x = U y/H` between a fixed lower and a moving upper plate, `rho = 1.2`, `nu = 1e-3`, `U = 2`, `H = 0.2`, 16
  cells across: `tau = rho nu U/H = 0.012` (x-force of the fluid on the lower wall, `-0.012` on the upper); `dn = H/32 =
  0.00625`, `u_tau = sqrt(tau/rho) = 0.1`, `y+ = dn u_tau / nu = 0.625`.
* Poiseuille `u = G/(2 nu) y (H - y)`, `G = 0.5`: the one-sided estimate at the cell centre `y = dn` gives
  `tau_est = rho G (H - dn)/2`, exact wall value `rho G H/2`; relative difference `dn/H = 1/(2 N)`.
* Uniform flow `u = 120 m/s`, `T = 288.15 K`, `gamma = 1.4`, `R = 287`, `p = 101325 Pa`: `c = sqrt(gamma R T)`, `M = 120/c`;
  `p0 = p (1 + 0.2 M^2)^3.5`. Low-Mach consistency: `p0/p = 1 + gamma M^2/2 + gamma M^4/8 + O(M^6)` (binomial expansion of
  `(1 + a)^m`, `a = (gamma-1) M^2 / 2`, `m = gamma/(gamma-1)`), and `0.5 rho u^2 = gamma p M^2 / 2`, so
  `|p0 - (p + 0.5 rho |u|^2)| / p <= 0.13 gamma M^4` for `M = 0.05` (`rho = p/(R T)`).
* Line sampling of the linear field `phi = 2x - 3y + 0.5z + 1`: exact. Plane `z = 0.5` over the unit square, midpoint rule
  with `n1 = n2 = 20`: `sum phi dA = 1 - 1.5 + 0.25 + 1 = 0.75` (the midpoint rule is exact for linear integrands).

### C3 Registered checks (F-072), tests in `crates/rcfd-post/tests/derived.rs`

* **D1 linear field**: graded `box_mesh([10,8,6],[1,1,1],[Expansion(3),Symmetric(2),Uniform])`, `u = A x + b`: vorticity
  `(1.0, 1.2, 2.1)` and `Q = 1.6` in every cell (absolute `1e-10`), including boundary cells.
* **D2 Taylor-Green**: `box_mesh([N,N,1],[2 pi,2 pi,1])`, N = 16 and 32, point values at cell centres, exact values on
  boundary faces: interior `|omega_z - 2 sin x sin y| <= h^2/3` and `|Q - Q_exact| <= h^2` for both N, and the ratio of the
  maximum interior vorticity errors N=16 to N=32 is `>= 3.5`.
* **D3 Couette**: wall shear on the lower wall `(0.012, 0, 0)`, upper `(-0.012, 0, 0)` (relative `1e-12`), `y+ = 0.625` on
  every face of both walls (relative `1e-12`).
* **D4 Poiseuille**: `tau_est = rho G (H - dn)/2` (relative `1e-12`) and `|tau_est - rho G H/2| / (rho G H/2) <=
  1/(2 N) (1 + 1e-9)` with N = 16.
* **D5 Mach and total pressure**: `M` and `p0` equal the formulas (relative `1e-12`); the incompressible total pressure
  equals its definition; the low-Mach difference limit above at `M = 0.05`.
* **S1 line**: 41 points from `(0.05,0.1,0.07)` to `(0.95,0.8,0.55)` on the graded mesh, gradient reconstruction with
  least-squares gradients: `|phi - phi_exact| <= 1e-10`, all located, `s_i = i/(n-1) |p1 - p0|` (absolute `1e-12`); a point
  outside the unit cube is reported not inside with an empty CSV field; CSV header `s,x,y,z,phi,inside`, 41 data rows, values
  parsed back are bit-equal to the in-memory ones.
* **S2 plane**: the midpoint sum equals `0.75` (absolute `1e-12`), all 400 samples inside.
* **H1** `baseline_hashes` passes unmodified (post is not on the solver path).

## Not attempted

* **F-103** (CGNS/EnSight export, HDF5 read): the acceptance is "exports open in third-party viewers". No viewer is available
  here and the container formats (CGNS over HDF5/ADF) cannot be written correctly without a library; format layouts from
  memory could not be verified. Status stays `todo`.
* **F-125** (public-data validation pack): the remaining gaps are an unresolved source of open compressor data, a
  transcription of published rotor tables and wiring of downloaded benchmark files into comparisons; each needs network
  access or published numbers that cannot be checked here. Status stays `partial`, unchanged.

## Hygiene

* H2 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release
  --no-fail-fast` on a `git archive` copy of the final commit, `CARGO_TARGET_DIR=<target dir>`.
* H3 every tuning and deviation disclosed under "Results"; unverifiable references marked in `docs/reference-data.md`.
* H4 roadmap statuses stay `partial` unless the whole acceptance text and title of the item is met.
* H5 baseline hashes unchanged and not re-recorded.

## Results

Measured on the final code (2026-10). Limits are the registered ones above; none was changed, widened, replaced or deleted. Every
registered check passed at its first run; there are no `*_known_miss` tests.

### F-236 (`rcfd-mesh/tests/partition_strategies.rs`, `rcfd-linalg/tests/reorder.rs`, `rcfd-fv/tests/reorder.rs`)

* P1 cover and cut consistency: pass for 8 strategies x 3 meshes x 15 part counts (360 partitions); `Decomposition::edge_cut`
  equals the independent pair count in every case.
* P2 balance (limit 0.05): worst imbalance 0.0300 (M1, `GraphPartitioner`, n = 10; its refinement tolerance is 0.03, the other
  strategies are below that).
* P3 node weighting (limit 0.05): worst weighted imbalance 0.0296; positive control: unweighted `Rcb` on the same weights
  reaches 0.613 (n = 2..=16, M1), so the weights matter.
* P4 edge cut: `GraphPartitioner` cut `<=` `Rcb` cut in all 45 cases (M1, M2, M3 x n = 2..=16); strictly lower in 31, equal in
  14 (equal where the coordinate seed is already a local optimum of the move rule, for example n = 2, 4 on all three meshes). Examples
  (rcb / graph): M1 n=3 167 / 117, M1 n=16 598 / 511, M2 n=5 388 / 316, M3 n=5 94 / 79. The cuts of the cycle and inertial
  variants are printed by the test (cycle is much worse on the O-grid: M3 n=16 564, inertial 144, rcb 204, graph 196);
  inertial beats the graph partitioner on several O-grid counts (M3 n=7: 100 against 114), so the registered criterion (graph
  against `Rcb` only) says nothing about which strategy is best. M1 n=2: graph cut 50 (limit 50).
* P5 smoothing: (a) the slab assignment is restored exactly, 10 moves, cut 50; (b) after smoothing the cut never increased and
  the imbalance stayed within `max(before, 0.05)` for all 360 partitions (unit weights; weighted smoothing is covered only
  through P3's graph partitioner, which shares the code).
* P6 sectors: key bands monotone and sizes within 1 for M1 and M3, n = 2..=16; the 8x8x1 quadrant closed form holds exactly.
* P7 inertial: the tilted slab is cut across, cut 8 and 128 / 128 cells; positive control `Rcb` cut 12 (> 8).
* R1, R2: permutations valid and deterministic; the shuffled 200-node path has RCM bandwidth 1 (shuffled labels: at least 100,
  positive control).
* R3: `box_mesh([40,8,8])` natural bandwidth 320, RCM 72 (derived bound 127); shuffled labels: bandwidth 2526, RCM 65.
  `box_mesh([40,6,1])` natural 40, RCM 7 (bound 11); shuffled 231, RCM 6. All limits met.
* R4: `B (P x)` against `P (A x)` relative error 3.0e-16 (limit 1e-12); the bandwidths agree; permuting back with the inverse
  gives the original CSR matrix.
* Acceptance (balance within 5 percent for 2 to 16 parts; graph cut not worse than coordinate bisection; RCM reduces bandwidth
  on a stretched mesh): met. Not done and not claimed: a multilevel graph partitioner (the graph method is seed plus greedy
  refinement), any use of the partitioners or the ordering from the case file, CLI, solvers or GUI, renumbering of a `Mesh`.

### F-108 (`rcfd-mesh/tests/inflate.rs`)

* I1 box patch: volume relative error 1.1e-15 (limit 1e-12), layer plane error 0 (1e-12), side area error 0, first/last/growth
  within 1e-12, quality: `max_aspect_ratio` 100.00000000000213 (relative 2.1e-14 against 1e-9), `max_non_orthogonality`
  1.2e-6 degrees (limit 1e-4; the acos noise known from ADR 0018), `max_skewness` 5.7e-14 (limit 1e-9).
* I2 triangle prism patch (72 triangles, 5 layers): volume error 1.5e-14, side area error 6.1e-16, every new cell has 5 faces
  and 6 points, valid, `check_mesh` ok.
* I3 O-grid inner wall: added volume 0.7662955179398452 against the closed form 0.766295517939847 (relative 2.3e-15, limit
  1e-10); layer radii error 2.2e-16 (1e-12).
* I4 sizing types, I5 invalid inputs, I6 y+ target (0.8 recovered to 1e-12), I7 report (JSON round trip, thickness spread 0):
  pass.
* H1: all earlier `rcfd-mesh` tests (including `edit.rs`) pass unchanged after the `extrude_patch` refactor.
* Status: the acceptance sentence is met for block meshes (wave 7 solved flat-plate check), and layers are now generated and
  reported on unstructured wall patches at the shell level. The item stays `partial`: the layers are added on the outward side
  of a boundary patch, not carved into a fluid domain (no merge with a bulk mesh, F-234), the report is not exposed through
  the CLI, case schema or GUI, and the solved y+ check was not repeated on an unstructured layered mesh.

### F-072 (`rcfd-post/tests/derived.rs`)

* D1 linear field on the graded mesh: vorticity error 3.3e-15, Q error 2.9e-15 (limit 1e-10) in every cell including boundary cells.
* D2 Taylor-Green: N = 16 vorticity error 4.91e-2 (limit h^2/3 = 5.14e-2), Q error 4.65e-2 (limit h^2 = 0.154); N = 32
  vorticity error 1.270e-2 (limit 1.285e-2, a margin of 1.2 percent: the derived bound is nearly tight), Q error 1.254e-2
  (limit 3.86e-2); error ratio 3.86 (limit 3.5).
* D3 Couette: wall shear and y+ worst relative error 7.5e-15 (limit 1e-12) on both walls.
* D4 Poiseuille: estimate error 2.4e-16; relative difference to the exact wall shear 0.03125000000000026 against the limit
  0.03125 (1 + 1e-9).
* D5: Mach and total pressure formulas match to 1e-12; low-Mach difference 1.094e-6 against the limit 1.1375e-6 (96 percent of
  the limit; the limit is the first neglected series term, so a small margin is expected).
* S1: line values error 3.3e-16 (1e-10), arc length exact, CSV header and 41 rows, bit-exact parse back; a line leaving the
  cube flags exactly the 5 outside points (empty field, `inside` 0). S2: plane integral 0.75 exactly (error 0), 400 of 400 inside.
* Status `partial`: derived fields and sampling exist as library functions with CSV text output; they are not reachable from
  the CLI, case schema, service or GUI, there is no VTK writer for the derived fields, further vortex criteria (lambda2) are
  not provided, and wall shear uses the one-sided first-cell estimate (first order, error `dn/H` in D4), not a wall-function value.

### Deviations and disclosures

* ADR typo: A3 P7 says "64 cells each" for the two halves of the `[32,4,2]` box (256 cells); the correct halves are 128 cells
  each (16 slices of 8). The test asserts 128 / 128; the registered substance (cut exactly 8, parts are the halves of the
  slices) is unchanged.
* A3 P5(b) is registered as "weighted imbalance"; the sweep over strategies uses unit weights (weighted smoothing is exercised
  only through the weighted graph partitioner of P3). This is narrower than the wording suggests.
* A1 point 3 says the graph seed is weighted `Bisection::Longest`; with unit weights this is identical to `Rcb` by
  construction (same rounding rule and tie-breaking), so P4 holds trivially where no move helps; P4 measures the refinement only.
* A1 point 4 says smoothing visits every cell while the graph partitioner visits boundary cells; both use one code path that
  skips cells without foreign neighbours (an interior cell has gain below 0 anyway).
* `extrude_patch` was refactored into a private core shared with `inflate_patch` (planned in B1); its arithmetic is unchanged and
  the existing tests pass.
* `is_none_or` (stable since Rust 1.82) tripped the workspace MSRV lint (1.75) in a test and was replaced by `map_or`; no
  criterion involved.
* Scratch files were kept in `<scratch dir>` and the session scratchpad, not in system temp.

### Unverified references

All literature attributions are from memory and listed in `docs/reference-data.md` (wave 15B section); none of the registered
checks depends on them.

## Addendum 1: layer order defect found in review (F-108), fixes and further disclosures

Limits are unchanged. Review of the first F-108 implementation found a specification and code defect: `extrude_core` keeps the patch
name and kind on the FAR cap (the new wall), but the first version stacked the layers with the first (thinnest) layer against the
ORIGINAL surface, so the cell against the wall was the thickest, `first * g^(layers-1)`, and a y+ target would have been missed (the
y+ 0.8 case would have given about 2.0). ADR B2 and the registered checks I1, I3, I4 measured distances from the original surface and
therefore encoded the wrong orientation; I6 only passed the input value through the report, so none of them could see it. The code
now orders the layers so that layer 1 (height `first`) lies against the cap (wall) and the last layer against the original surface;
total thickness, added volume, side areas, report fields and all closed forms of B2 are unchanged. As with ADR 0018 Q2d, the old tests
`i1_block_patch`, `i3_curved_patch`, `i4_sizing_types` are kept as ignored records (they fail on the corrected code, which confirms the
defect) and new tests assert the same limits with the layer planes, radii and heights counted from the wall:
`i1c_block_patch_thinnest_layer_at_wall` (planes `2 + T - cum_k`, error 0; every cap-cell height 0.01 to 1e-12; max aspect ratio 100),
`i3c_curved_patch_radii_from_the_wall` (radii `1 - T + cum_k`, error 2.2e-16; volume closed form unchanged), `i4c_sizing_types_from_the_wall`,
and the post-registration check `i6c_wall_adjacent_cell_meets_the_y_plus_target` (owner-cell height of every cap face equals 3.2e-4 and its
y+ equals 0.8, both to relative 1e-9). The old `i6` is kept and passes but only checks the report value. The earlier result lines for I1, I3
above were measured on the defective ordering; the corrected numbers are those quoted here.

Further disclosures:
* P4 (graph cut `<=` RCB cut) is guaranteed by construction (unit-weight seed is `Rcb`, every move lowers the cut), in every case, not only
  where no move helps; the evidence of the refinement is the 31 strict reductions out of 45.
* R3 "stretched mesh": the meshes are elongated index spaces of cubic cells. Bandwidth depends only on connectivity, so a geometrically
  graded or stretched mesh with the same index space gives identical numbers.

## Addendum 2: hygiene results

On a `git archive` of commit a4cef09 (HEAD at that time, which includes the wave 15A commits then committed): `cargo fmt --check` clean,
`cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast`: 138 test binaries
reported `ok`, no FAILED line, no panic. `baseline_hashes`: 3 passed, 1 ignored (the pre-existing ignored rotating-channel test), unmodified
and not re-recorded. Later commits touch only this ADR.
