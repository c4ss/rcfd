# ADR 0215: F-247 registration: local remeshing on unstructured tetrahedral meshes

Status: **REGISTERED 2026-11-07**, written before any code of F-247 and before any run of its checks. Not yet accepted by
the maintainer. Before this text, existing code of main 08316f7 was run in a scratch crate outside the repository, to
choose the initial mesh (listed under "Run before this registration"). No code of F-247 existed. Results go in the
addendum at the end.

Parents: [ADR 0090](0090-wave38c-dynamic-mesh-layering.md) (F-230: the piston set-up P and its limits A1, A2, B1 to B3,
`CompressibleSolver::step_ale` and `replace_mesh`, the conservative remap by cell totals, the "not delivered" list that
names local remeshing), [ADR 0213](0213-f257-registration-prism-layering-and-incompressible-layering.md) (F-257: the
piston loop on another cell shape, its checks P0 to P7, its negative control N1, its mutation and gate procedure),
[ADR 0033](0033-wave20b-dynamic-mesh.md) (F-063: swept volumes and the discrete GCL),
[ADR 0030](0030-wave19b-meshing.md) and [ADR 0104](0104-wave41d-tet-sliver-removal.md) (the tetrahedral mesher
`tetmesh::tetrahedralize` and its opt-in quality pass), [ADR 0137](0137-linux-gate-platform-values.md) (platform values).
Roadmap item F-247 (depends on F-230 and F-063).

Number: the lead gave 0215 to F-247. Main 08316f7 holds 0213 as its highest; 0212 is on the branch
`f296-pair-diffusivity` and 0214 on `f299-piso-thin-gap`. No branch holds a 0215.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), ADR 0207, ADR 0213):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- the remesher's own parameters (S1) are fixed here too; changing one needs an amendment written before the run it
  affects;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the measured number recorded, and the
  cause is diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as
  an ignored record;
- every new code path is opt-in: the remesher is a new module that no existing function calls, so a run that does not use
  it runs as before, bit for bit (D1);
- no case-schema key is added (S3), so neither `crates/rcfd-api/tests/baseline_hashes.rs` nor the record R0
  (`case_hashes_w48u.rs`) is touched, no golden is re-recorded, and no existing test is edited;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, no commit trailers.

Development runs (registered): while the code is written, its functions may be run on scratch meshes and scratch motions
that are not the set-ups below (P-T, M_s), to find coding errors. They are listed in the addendum. They cannot change a
limit or a parameter of S1 without an amendment.

Clean-room: the operators (edge split, edge collapse, face and edge flips, vertex relocation), the validity conditions
and the conservative remap by intersection volumes are derived below from the definition of a tetrahedral tiling and of
cell totals (ADR 0090 D3). The mean-ratio quality is the textbook normalised ratio of volume to squared edge lengths,
written down from its definition. No source was read for this work, and no vendor code, text or install was used.

## What F-247 asks

The roadmap entry: dynamic mesh, local remeshing on unstructured tetrahedral meshes. Every limit not stated there is
registered here, before the run. Local remeshing:

- the ADR 0090 piston in a closed cylinder on an unstructured tetrahedral mesh, with remeshing instead of layering, keeps
  `p_bar V^gamma` within 2e-2 at full compression and after re-expansion,
- conserves volume, mass, momentum and energy through every remeshing event to 1e-13 relative (conservative remap),
- triggers at least 5 events,
- and keeps the minimum cell quality above the ADR threshold.

Out of scope there: sliding interfaces (F-250), elastic-analogy smoothing (F-063), six-DOF coupling (F-244), more than one
moving layered boundary, non-planar layer surfaces, turbulence or energy in the incompressible moving-mesh path. The note:
split from F-230 on 2026-10-21; the user's split of 2026-10-22 left this item the remeshing part (A), size L.

**Reading of the acceptance (registered here).**

1. "The ADR 0090 piston in a closed cylinder" is ADR 0090's set-up P: the closed box `[0, 1] x [0, 0.1] x [0, 0.1]`
   (ADR 0090 calls it the cylinder), slip walls, the gas, the state at rest, the solver settings, the piston motion with
   `T = 16` and the loop with `dt = max_dt()` landed on `T` and `2T`. Only the mesh changes: an unstructured tetrahedral
   mesh from the existing Delaunay mesher (M0 below), and the remesher of M1 to M7 instead of the layer stack.
2. "Full compression" is `t = T` (piston at `x = 0.4`, at rest), "after re-expansion" is `t = 2T` (piston back at `x = 1`);
   the measure is ADR 0090's A1 and A2: `p_bar V^gamma / (p0 V0^gamma) - 1` with `p_bar` the volume-weighted mean pressure.
3. "Through every remeshing event": the totals of the solver state before an event against those after the remap onto the
   new mesh. "Relative" is `abs(after - before) / abs(before)` for the volume, the mass and the energy. The total momentum
   of a closed piston passes through zero (at `t = 0`, `T`, `2T` and by symmetry in y and z at all times), so a ratio to
   it is not defined; the momentum is registered against the momentum magnitude in the cells,
   `abs(P_k after - P_k before) <= 1e-13 sum_c |m_c| V_c` for each component `k` (`m_c` the momentum density). This scale
   is at least `abs(P_k)`, and the summation round-off of `P_k` is relative to it.
4. "Triggers at least 5 events": events that changed the mesh (an event that tried and changed nothing is counted apart).
5. "The minimum cell quality": the mean ratio `eta` of M2, over every cell, after every step and after every event. The
   threshold is `Q_min = 0.2` (S1).
6. "Local": registered as L1 below (the cells far from the piston's range are never touched).
7. The acceptance is library work. The case tree is not in it; the wiring is Q1.

## What the code read shows (main 08316f7, read only)

1. **`tetmesh::tetrahedralize(surface, opt)`** (`crates/rcfd-mesh/src/tetmesh.rs`) meshes the inside of a closed triangle
   surface: Delaunay insertion with exact predicates, boundary recovery by Steiner points, refinement to a radius-edge
   bound and an optional size, and the opt-in quality pass `quality_sweeps` (ADR 0104). It returns a `TetMesh` (points,
   positively oriented cells `orient3d > 0`, the conforming surface with its tags). `TetMesh::to_mesh` builds the solver
   `Mesh` with one patch per surface tag.
2. **`surface::box_surface(lo, hi, n)`** gives a box surface with every tag 0, so the six sides are not told apart.
3. **`predicates::orient3d`** is exact (adaptive expansion), with the sign of `dot(cross(b - a, c - a), d - a)`.
4. **`CompressibleSolver::step_ale(geom, dt)`** needs the same points and cells (one topology); **`replace_mesh(geom, u)`**
   installs any topology with the same number of patches and one state per cell (ADR 0090). The slip-wall flux of a wall
   moving in its own plane is zero, and the swept volume of a face moving in its own plane is zero to round-off.
5. **`Mesh::from_face_lists`** builds and validates a mesh (closure, positive volumes, owner-to-neighbour normals).
6. Nothing in the code base remeshes a tetrahedral mesh during a run; `adapt.rs` refines a hexahedral octree and
   `surface_remesh.rs` remeshes triangle surfaces.

## Run before this registration (disclosed)

A scratch crate outside the repository (`w49i-work/f247-scratch/explore`, path dependencies on `rcfd-mesh`, `rcfd-core`
and `rcfd-fv` of main 08316f7, release, Windows) ran existing code only:

- `tetrahedralize` on `box_surface([0, 0, 0], [1, 0.1, 0.1], n)` for `n` in `[20, 2, 2]`, `[40, 4, 4]`, `[30, 3, 3]`,
  `[10, 1, 1]`, with `size` none, 0.025, 0.0333, 0.035 or 0.05 and `quality_sweeps` 0 or 3 (15 meshes), printing the
  cell and point counts, the mean ratio (min, 1 %, 5 %, median), the edge lengths, the smallest altitude and `2V/A`. The
  choice: `n = [20, 2, 2]`, `size = 0.035`, `quality_sweeps = 3` (M0), the best smallest mean ratio among the meshes of
  under 4000 cells.
- One of them panicked inside the existing mesher: `n = [10, 1, 1]`, `size = 0.035`, `quality_sweeps = 3` stops at
  `tetmesh.rs:386` with `neighbour link`. Not investigated (outside F-247; Q7).
- A probe of M0: two calls give the same points and cells; every boundary point lies exactly (bit for bit) on its side
  plane and every surface triangle lies exactly on one side; and the existing fixed-mesh `CompressibleSolver` (default
  settings, state at rest) on `M0.to_mesh` has `max_dt = 9.054e-4` and takes 1.81 s for 1000 steps.

No piston run, no moving-mesh run and no run of any check below was made.

**M0** (the initial mesh, recorded): 1711 cells, 522 points (88 not on the boundary), FNV-1a hash of the point bits and
the cell vertex lists `aaf8f4c36f6446e4`; mean ratio min `0.5646`; edge lengths in `[0.0250, 0.0669]`; total volume 0.01.

## The model (derived)

**M1. The mesh and its boundary.** A tetrahedral mesh (points, cells as four point indices with `orient3d > 0`) whose
boundary lies on a list of axis-aligned planes `x_axis = value`, each with a patch name (for the box: `xmin`, `xmax`,
`ymin`, `ymax`, `zmin`, `zmax`, in that order). Every point carries the set of planes it lies on, by exact equality of
its coordinate (M0's boundary points lie exactly on their planes). A plane fixes one coordinate of its points; the other
coordinates of a point are free. A boundary face is a cell face shared with no other cell; its patch is the plane that
holds its three points (exactly one plane for a non-degenerate face; none is an error). Moving a plane (the piston, `xmax`)
sets that coordinate of its points; the other points do not move.

**M2. Cell quality.** The mean ratio `eta = 12 (3 |V|)^(2/3) / sum_e l_e^2` with the sign of `V` (six edges, `V` the
signed volume): 1 for the regular tetrahedron, tends to 0 for a degenerate one, negative for an inverted one.

**M3. Operators.** Each replaces a set `C` of cells (the cavity) with a set `N` of new cells.

- *Split* of the edge `(a, b)`: a new point `m` at the midpoint, on the planes common to `a` and `b` (so it lies on them
  exactly, since both coordinates are equal); each cell holding `a` and `b` gives the two cells with `a` and with `b`
  replaced by `m`. `C` is the ring of the edge.
- *Collapse* of `b` onto `a` (`a` and `b` joined by an edge): only when every plane of `b` is a plane of `a`, so `b`'s
  boundary faces stay on their planes; the cells holding `a` and `b` are removed and in the other cells holding `b`, `b`
  becomes `a`. `C` is the star of `b`; `b` is removed.
- *Edge removal* of an interior edge `(a, b)` (no common plane) with a ring of `n` cells, `3 <= n <= max_ring`: the ring
  vertices `r_0 .. r_(n-1)` in order; the fan of triangles `(r_k, r_(k+i), r_(k+i+1))`, `i = 1 .. n-2`, from each `k`
  gives `2 (n - 2)` cells (each triangle with `a` and with `b`); the `k` with the largest smallest `eta` is the candidate
  (`n = 3` is the 3-2 flip). `C` is the ring.
- *2-3 flip* of an interior face `(a, b, c)` with the cells `(a, b, c, d)` and `(a, b, c, e)`: the three cells around the
  new edge `(d, e)`. `C` is the two cells.
- *Relocation* of a point `v` with a free coordinate: its free coordinates go to the mean of its neighbours' (points
  joined to `v` by an edge), or halfway there; the fixed coordinates stay. `C` is the star of `v` and `N` the same cells
  with `v` moved.

**M4. Validity of a candidate (all required).** (V1) every new cell has `orient3d > 0` (exact). (V2) the faces of `C`
shared with cells outside `C` are faces of `N`, with the same orientation seen from inside, and `N` has no other face
shared with a cell outside. (V3) every other boundary face of `N` (a face of no other cell of `N`) has its three points on
one plane. (V4) `abs(sum V(N) - sum V(C)) <= 1e-12 sum V(C)`. With V1 and V2 the new cells cover the inside of the
cavity's interface faces with winding number one, and V3 and V4 keep its faces on the boundary planes and its volume,
so `N` tiles the region of `C` and the mesh stays conforming.

**M5. Conservative remap of an operator.** For `c` in `C` and `n` in `N`, `x_cn = |c ∩ n|`, the volume of the
intersection of two tetrahedra (clipping one by the four half-spaces of the other; zero for disjoint bounding boxes).
Every old cell's weights are normalised to sum to one, `f_cn = x_cn / sum_n' x_cn'`, so the total of every old cell goes
exactly to the new cells (to round-off). The raw sums are checked: `abs(sum_n x_cn - V_c) <= 1e-9 V_c` and
`abs(sum_c x_cn - V_n) <= 1e-9 V_n`, else the operator returns an error (a remap that does not see the geometry is a
bug, not a result). The largest of these raw mismatches over an event is reported (R2).

**M6. An event.** Every cell carries its sources: a list of `(cell at the start of the event, fraction of that cell's
total)`; at the start, cell `t` has `[(t, 1)]`. An operator gives every new cell `n` the sources
`sum_c f_cn sources(c)`, so the fractions of each start cell sum to one over the cells at the end. At the end the cells
and points are renumbered (survivors in their order, then new cells in the order they were made; unused points dropped)
and the event returns the sources of every cell. The solver's new cell density is the sum of `fraction x total` of its
sources divided by its new volume (ADR 0090's remap with these fractions): mass, momentum, energy and every other total
are carried over to round-off, and a uniform state stays uniform.

**M7. The trigger and the event procedure.** After each step the remesher checks the mesh. An event runs when a cell has
`eta < q_trigger`, or an edge is shorter than `l_min` or longer than `l_max`. The event makes passes (at most
`max_passes`); a pass does, in order:

1. *splits*: the edges longer than `l_max`, longest first; a split is applied when its new cells have
   `min eta >= min(q_split, min eta(C))`;
2. *collapses*: the edges shorter than `l_min`, shortest first; both directions allowed by M3 are candidates, and the one
   with the larger `min eta` among those with `min eta >= min(q_collapse, min eta(C))` is applied;
3. *improvement*: the cells with `eta < q_good`, worst first; the candidates are the relocations of each of the cell's
   points (to the mean and halfway), the edge removals of its interior edges, the 2-3 flips of its interior faces and the
   collapses of each of its points onto each neighbour allowed by M3; the candidate with the largest `min eta` is applied
   when that is larger than `min eta(C) + 1e-6`.

Every candidate must also be valid (M4) and keep the edge lengths of its cells within
`[min(l_min, shortest edge of C), max(l_max, longest edge of C)]`, so no operator makes the size range of its cavity
worse. Ties go to the first candidate in the order above (cells, points and edges in index order). A pass that applies
nothing ends the event. An event that applied an operator returns its change; one that applied none returns nothing and
the mesh is unchanged. The procedure is deterministic.

The motion moves the piston plane only, so the cells and edges that the trigger can see changing are those touching the
piston; M0 itself triggers nothing (P0), so every event is driven by the motion.

## Settings and code

**S1. Library (`rcfd-mesh`, new module `remesh`, additive).** A new file `crates/rcfd-mesh/src/remesh.rs` and one
`pub mod remesh;` line in `crates/rcfd-mesh/src/lib.rs`; no other file of `rcfd-mesh` is edited.

- `AxisPlane { name, axis, value }` and `box_planes(lo, hi)` (the six planes in the order of M1);
- `RemeshSettings { l_min, l_max, q_trigger, q_good, q_collapse, q_split, max_passes, max_ring }` with
  `RemeshSettings::for_size(h0)`: `l_min = 0.5 h0`, `l_max = 2 h0`, `q_trigger = 0.3`, `q_good = 0.4`,
  `q_collapse = 0.3`, `q_split = 0.25`, `max_passes = 10`, `max_ring = 7` (registered values);
- `TetRemesher` (`new(points, cells, planes, settings)`, `from_tet_mesh`, `points`, `cells`, `plane_index`, `set_plane`,
  `mesh`, `quality`, `min_quality`, `needs_event`, `remesh` (the event when the trigger fires), `event` (the event
  procedure regardless of the trigger));
- `RemeshChange { sources, ops, removed, created, lo, hi, mismatch }` (the sources of M6, the counts of each operator,
  the cells removed and created, the bounding box of their points, the largest raw mismatch of M5);
- `mean_ratio`, `tet_intersection_volume` and `remap` (M5, M6).

`mesh()` gives the solver mesh: internal faces ordered by (owner, neighbour), boundary faces by patch in the plane order
and then by (owner, local face), every patch `Wall`, the face order fixed by the cells alone, so a mesh rebuilt after a
motion has the same faces in the same order (as `step_ale` needs). For the run: `h0 = 0.035` (M0's `size`), so
`l_min = 0.0175` and `l_max = 0.07`, which hold M0's edges `[0.0250, 0.0669]`.

**S2. `rcfd-fv`.** Not edited. The run uses `step_ale` and `replace_mesh` as they are.

**S3. Case tree and CLI.** Not changed; no key is added. The documents `docs/dynamic-mesh.md` and `docs/case-schema.md`
may say what the library does and that the tree does not use it (prose only; no TOML block is edited).

## Registered checks

All tests run in release. The new checks are in `crates/rcfd-fv/tests/remesh_f247.rs` and the unit tests of
`crates/rcfd-mesh/src/remesh.rs`.

### P. The tetrahedral piston with local remeshing (the acceptance)

Set-up P-T: M0 with the six planes of M1, `RemeshSettings::for_size(0.035)`; ADR 0090's P otherwise: every patch a slip
wall (the solver's default for `Wall`), `Gas::AIR`, `rho0 = 1`, `u = 0`, `p0 = 1`, `CompSettings::default()` (HLLC, van
Leer, SSP-RK3, CFL 0.4), the piston `x_p(t) = 1 - 0.3 (1 - cos(pi t / T))`, `T = 16`, to `2T`. The loop of ADR 0213's P:
`dt = max_dt()` of the current mesh, landed on `T` and `2T` (`t + dt >= target - 1e-9`), `set_plane(xmax, x_p(t + dt))`,
`step_ale` with the geometry of `mesh()`, then `remesh()`; on a change: the geometry of the new mesh, the remap of M6 with
the solver's cell volumes, and `replace_mesh`.

| Id | Check | Limit |
| --- | --- | --- |
| P0 (set-up) | M0: hash `aaf8f4c36f6446e4`, 1711 cells, mean ratio min `>= 0.56`, edges in `[l_min, l_max]`, `needs_event()` false; after every step and event `mesh()` is a valid mesh with the six patches of M1 in order | as stated |
| A1 (acceptance) | `p_bar V^gamma / (p0 V0^gamma) - 1` at `t = T` | `abs <= 2e-2` |
| A2 (acceptance) | the same at `t = 2T` | `abs <= 2e-2` |
| B1v (acceptance) | every event: `sum V` after against before | relative `<= 1e-13` |
| B1m (acceptance) | every event: `sum rho V` after against before | relative `<= 1e-13` |
| B1p (acceptance) | every event: each momentum component, `abs(P_k after - P_k before)` against `sum_c abs(m_c) V_c` before | `<= 1e-13` |
| B1e (acceptance) | every event: `sum rho E V` after against before | relative `<= 1e-13` |
| B4 (acceptance) | events that changed the mesh | `>= 5` |
| Q (acceptance) | the smallest mean ratio over all cells, after every step and after every event | `>= 0.2` |
| B2s | every step: the total mass against the step before | relative `<= 1e-13` |
| B3 | after every step and every event: `sum V` against `0.01 x_p(t)` | relative `<= 1e-13` |
| L1 | every event: the smallest x of a point of a removed or created cell (`RemeshChange::lo`) | `>= 0.2` |
| R1 | no step and no event returns an error (M5's raw-sum check included) | as stated |
| R2 | the largest raw mismatch of M5 over all events | `<= 1e-11` |

L1 shows that the remeshing stays local: the piston never comes closer than `x = 0.4`, and the half of the compressed
cylinder nearest the fixed end (`x < 0.2`, a fifth of the initial length) is never touched.

Printed, no limit: the events and their operator counts, the triggered events that changed nothing, the events after
which a trigger condition remained, the largest `abs(p_bar V^gamma / (p0 V0^gamma) - 1)` over the steps, the cell spread
of `p / rho^gamma` at `T`, the cells at `T` and `2T`, the smallest mean ratio and dihedral angle of the final mesh, the
mass drift over the run against the initial mass (ADR 0090's B2; not a check here, because this run takes several times
ADR 0090's 9192 steps and that drift is round-off that grows with the step count), the steps and the wall time.

### N. Negative controls

| Id | Control | Limit |
| --- | --- | --- |
| N1 | P-T: at every event, the totals of a remap that gives every new cell the density of its largest source; the largest relative change of `sum rho V` over the events | `>= 1e-10` |
| N2 | P-T with `remesh()` never called: the time at which the smallest mean ratio first falls below `Q_min` or `mesh()` / `step_ale` first fails | `< T` |

N1 shows that B1m sees a remap that does not carry the cell totals (the density next to the accelerating piston differs
from its neighbours' by the pressure gradient of the gas column). N2 shows that Q and the run need the remeshing: the
piston moves 0.6 while M0's cells are at most 0.067 long.

### U. Unit checks (`rcfd-mesh`)

| Id | Check | Limit |
| --- | --- | --- |
| U1 | `tet_intersection_volume`: the unit cube cut into 6 cells (the main diagonal) and into 5 cells (a centre cell and four corners): all 30 pairs; every row sum against its cell of the 6, every column sum against its cell of the 5; a cell with itself; a cell against a disjoint one and against its face neighbour | relative `1e-13`; `1e-13` of `V`; `0` and `<= 1e-15` |
| U2 | M_s = `tetrahedralize(box_surface([0, 0, 0], [0.3, 0.1, 0.1], [6, 2, 2]), size 0.035, quality_sweeps 3)` with the six planes: for each operator of M3 (split, collapse, edge removal, 2-3 flip, relocation) the first valid candidate in index order (M4 only, no quality rule), applied as a one-operator event, and two events of the procedure after `xmax` is set to 0.285 and then to 0.33 (`event()`); each time (a) the fractions of every start cell sum to one, (b) `sum V` of `mesh()` after against before, (c) a uniform state `(1, 2, 3)` after the remap, every cell, (d) the total of the field `1 + x + 2y + 3z` (cell centroids) after against before, (e) `mesh()` is valid with the six patches | (a) to (d) relative `<= 1e-13`; (e) as stated |
| U3 | M0 through `TetRemesher::mesh()`: 1711 cells with 4 faces each, the six patches in order with sizes summing to M0's surface triangles, total volume 0.01 | as stated; relative `1e-13` |

A type of operator with no valid candidate on M_s is a defective set-up (Classes).

### D. Defaults

| Id | Check | Limit |
| --- | --- | --- |
| D1 | `git diff 08316f7 -- crates` shows only the new files and the one `pub mod` line | as stated |
| D2 | the existing suites, unedited: `rcfd-mesh` unit tests and its `tetmesh` and `surface_remesh` tests; `rcfd-fv` `layering_w38c`, `layering_f257`, `layering_f257_records`, `ale_flows`, `ale_gcl`, `compressible_ale_f258`, `compressible_ale_f258_records`, `mesh_smoothing`; `rcfd-api` `layering_ale_f259`, `dynamic_mesh`, `baseline_hashes`, H1 of `case_hashes_w48u`; `rcfd-cli` `layering_ale_f259` | pass |

### Mutations (scratch, not committed)

Each is applied alone to the final tree by an exact text replacement, the named tests are run once in their own target
dir, and they must fail; the file is restored and checked byte for byte against a kept copy. Their numbers go in the
addendum. `scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | M5's weights normalised per new cell (`x_cn / sum_c' x_c'n`) instead of per old cell | B1m (P-T), U2 (a) |
| MU2 | the clip of `tet_intersection_volume` uses three of the four half-spaces | R1 (P-T), U1 |
| MU3 | the trigger ignores the mean ratio (edge lengths only) | Q or R1 (P-T) |
| MU4 | the improvement takes every cell with `eta < 1` instead of `q_good` | L1 (P-T) |

### The gate

`cargo fmt` per package, `cargo clippy --workspace --all-targets -- -D warnings`, and every test in release on a
`git archive` copy of the gated commit, one process per test, three jobs (`scripts/gate_test.py`), in the target dir
`E:/projects/cfd/w49i-work/target-f247-gate`, taken in turn through the machine's gate lock (the atomic `mkdir` lock of
`gate-lock.sh`), with the free physical memory at the start disclosed. A failure of a known flaky test is run again alone
on the same build and reported with its name.

### Classes

D1 and D2 are preconditions of landing: a miss means the change is not merged and the cause is recorded. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A set-up check
(P0, U2's existence of each operator) or a negative control (N1, N2) that does not hold, or a mutation that does not make
its check fail, means the check is not shown to be able to fail: it is reported as a defective set-up, corrected by an
amendment before any re-run, and never counted as a pass.

**Decision rule.** Every registered check met (P0, A1, A2, B1v, B1m, B1p, B1e, B4, Q, B2s, B3, L1, R1, R2, U1 to U3, with
D1 and D2), the negative controls held and every mutation failed its tests: F-247 is set done (`[x]`), the acceptance
still the maintainer's (Q6). A miss of any check: partial with the miss named. The piston does not run through `2T` with
remeshing at all: todo.

## Not delivered

The case-tree and CLI wiring (Q1); remeshing in the incompressible PISO/PIMPLE path, which needs a face-flux map through
an event (Q3); boundaries that are not axis-aligned planes (curved or oblique walls, Q4); a size field other than one
uniform `h0`; a remap of higher order than cell averages (Q5); remeshing of prism, hexahedral or polyhedral cells;
everything outside F-247 (the roadmap's out-of-scope list).

## Open questions and choices for the maintainer

- **Q1. The case-tree wiring.** Not in the acceptance. Options: (a) a follow-up registration under F-259's checks
  (located errors, a tree run equal to the library run to 1e-12, a case without the new behaviour bit-identical): a value
  `remeshing` of `dynamic_mesh.deformation` for the compressible solver, with the tetrahedral generator and `h0` from an
  optional table, so that H1's record R0 needs no edit; (b) keep the library only. Recommendation (a), as a new item.
- **Q2. The quality measure and threshold.** Options: (a) the mean ratio with `Q_min = 0.2` (as registered); (b) another
  measure (radius ratio, smallest dihedral angle) or threshold. Recommendation (a): it is scale-free, it is 0 only for a
  degenerate cell, and 0.2 excludes slivers while M0 starts at 0.565.
- **Q3. Remeshing in the incompressible path.** Options: (a) a new item with a face-flux map through an event (the
  intersection weights carry cell totals, not face fluxes; a flux that satisfies the discrete continuity would need a
  local projection); (b) none. Recommendation (a) only when a case needs it.
- **Q4. Non-planar boundaries.** The remesher keeps boundary points on axis-aligned planes, which keeps the volume exact.
  Options: (a) keep; (b) oblique planes (a point keeps the planes it lies on, its free directions are their common
  null space); (c) curved walls with projection, where the volume changes with the facets. Recommendation (a) until a case
  needs (b); (c) conflicts with exact volume conservation.
- **Q5. The remap order.** Options: (a) cell averages (first order, conservative, positivity-preserving; as registered);
  (b) a linear reconstruction integrated over the intersections (second order, conservative, needs a limiter for
  positivity). Recommendation (a).
- **Q6. Status of F-247.** If every check is met, the acceptance as written is met in the library. Options: (a) done,
  with the wiring tracked by Q1; (b) partial until the tree uses it. Recommendation (a), as for F-257 and F-258. The
  roadmap follows the decision rule until the maintainer decides.
- **Q7. The mesher panic.** `tetrahedralize(box_surface([0, 0, 0], [1, 0.1, 0.1], [10, 1, 1]))` with `size = 0.035` and
  `quality_sweeps = 3` panics at `tetmesh.rs:386` (`neighbour link`), seen while choosing M0. Options: (a) a new item to
  reproduce and fix it; (b) note only. Recommendation (a).

## Amendment 1 (2026-11-07, before any code)

Written after the registration commit and before any code of F-247 was written or run. U2's two events of the procedure
set `xmax` of M_s from 0.3 to 0.285 in one jump. That moves the piston plane by 0.015 while the smallest cell altitude of
meshes from this generator is about 0.012 (the probe of M0: 0.0124), so the jump can invert a cell next to the piston
before any event runs, and the check would test a mesh that no run produces. Corrected U2 (replaces those two events;
everything else in U2 unchanged): the piston plane of M_s is moved inward in steps of 0.001 to `x = 0.27` and then
outward in steps of 0.001 to `x = 0.33`, with `remesh()` after each step; at every event, (a) to (e) as registered. At
least one event inward and one outward are required (set-up). No limit changes.

## Amendment 2 (2026-11-07, before any registered run)

Written after the code of S1 was first written and before any registered check (P-T, N1, N2, U1 to U3) was run. The
development runs allowed by the house rules, on scratch set-ups only, found four defects of M1 and M7. No limit changes and
no threshold of S1 changes; the operators and the motion change as below.

Development set-ups (scratch crate `w49i-work/f247-scratch/dev`, outside the repository; neither is P-T or M_s):

- D-a: `tetrahedralize(box_surface([0, 0, 0], [0.5, 0.12, 0.08], [10, 3, 2]))` with `size` and `h0` 0.035, 0.04 and 0.05
  (`quality_sweeps` 3), the remesher alone (no solver): `xmax` moved in steps of 2e-4 to 0.2 and back to 0.5, `remesh()`
  after each step;
- D-b: `tetrahedralize(box_surface([0, 0, 0], [0.8, 0.1, 0.12], [16, 2, 3]))`, `size` and `h0` 0.035, 1742 cells, the
  compressible loop of P-T with `T = 4` (a piston four times faster than P-T's) to `2T`.

Defects found on D-a and their changes:

1. *A point is caught by the piston.* With only the piston plane moving, a point just behind it (in one run a side point
   0.00015 from the piston) sits on cells that flatten as caps faster than the operators remove it, and a cell inverts
   (D-a at `h0` 0.035, 0.04 and 0.05). Change of M1: the motion carries a band. `move_plane_with_band(k, value, w)` moves
   the points on the plane to `value` and every point at a distance `d < w` on its inner side (and on no plane of the same
   axis) along the axis by `(value - old) (1 - d / w)`. The map is monotone along the axis with slope `1 + Δ/w > 0`, so the
   motion alone inverts no cell and overtakes no point; the cells next to the piston thin gradually and the triggers of M7
   see them. P-T, N2 and U2's piston use `w = 2 h0 = 0.07`. `set_plane` is unchanged (plane points only).
2. *A split and a collapse undo each other.* In the expansion a split leaves a cell below `q_good`, and the improvement
   collapses the new point back, which makes the long edge again: thousands of operators per run, ten passes per event.
   Change of M7 step 3: a collapse candidate makes no edge longer than `l_max`.
3. *A short edge whose end points cannot collapse onto each other* (a point of a box-edge line next to a piston corner)
   stays until its cells degrade, and the size rule rejects the good collapse onto the point behind it. Change of M7 step 2:
   the candidates are the collapses of either end point onto any of its neighbours allowed by M3, and a step-2 collapse may
   make edges up to `max(2 l_max, longest edge of C)` (step 1 splits them in the next pass into halves of at most `l_max`).
4. *A flat cell whose three points on a side plane become collinear* (two static points of a box-edge line and a piston
   edge point between them) can be repaired only by flipping the boundary edge on that plane. Change of M3: edge removal
   also takes an edge on exactly one plane, whose ring is open (`n >= 2` cells, `n + 1` ring points, the two ends on the
   plane); its polygon is closed by the segment of the two ends, and the fans are formed as for a closed ring (`n = 2` is
   the 2-2 flip of the boundary edge). An edge on two planes (a box-edge line) is not removed.

With these changes the development runs gave (numbers of scratch set-ups, no limit applies to them): D-a at `h0` 0.035,
0.04, 0.05 with the band: no failure, smallest mean ratio over the run 0.345, 0.321, 0.373, the largest raw mismatch of M5
2.2e-14; D-b: `p_bar V^gamma` 9.4e-5 at `T` and 2.3e-4 at `2T`, per-event volume, mass and energy within 1.5e-15,
smallest mean ratio 0.288, raw mismatch 1.9e-13, the smallest x of a touched point 0.249 (the piston at rest at 0.32 at `T`), 14613
steps in 53 s. Events after which a trigger condition remained were frequent (D-b: 1368 of 1433; mostly edges just over
`l_max` whose split the quality rule refused), which costs time but is not a check (it is printed in P-T).

The text of M1, M3 and M7 above is read with these changes; S1 gains `TetRemesher::move_plane_with_band`. The record M0,
the limits, the checks, the negative controls and the mutations are unchanged.
