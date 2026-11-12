# ADR 0267: F-348 registration: anisotropic (directional) refinement on a lattice with per-axis levels

Status: **REGISTERED 2026-11-12**, written before any code of F-348 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0247](0247-f343-registration-adaptation-criteria-and-suction-layer.md) (F-343: the criteria, the suction layer,
its adapted error `E(A)`, D5 and R1: a `Leaf` keeps one level for every axis),
[ADR 0242](0242-f102-registration-adapted-cavity-and-solver-loop-hook.md) (the loop of `rcfd_fv::adaptive`),
[ADR 0054](0054-wave28c-adaptation-turbo-post.md) (the hanging-node lattice `AmrMesh`, 2:1 balance, conservative transfer) and
[ADR 0262](0262-f342-registration-case-key-for-the-adaptive-steady-loop.md) (`[mesh.adapt]`; no case key is added here).
Roadmap item F-348 (depends on F-343).

Number: the lead gave 0267 to F-348. Branch `f348-anisotropic-refinement` from main 617c3d70, worktree `w49i-work/f348`.

House rules (as ADR 0247):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written (and committed) before the corrected run, and the old one
  stays as an ignored record;
- each registered run is made once, and nothing is re-run to pick a result;
- defaults stay bit-identical: `AmrMesh`, `Leaf`, `Mark`, `Transfer`, `transfer_field`, every indicator and every function of
  `rcfd_fv::adaptive` keep their code, signatures and results; everything new is a new public item; no existing test or ignored
  record is edited; no hash table and no golden is touched; nothing in `rcfd-api` is touched;
- no new third-party crate, `Cargo.lock` unchanged, no `unsafe`, the crate graph unchanged (`rcfd-mesh`: `src/adapt.rs` gets one
  `pub mod aniso;` line and a doc line, new `src/adapt/aniso.rs`, new test file; `rcfd-fv`: `src/adaptive.rs` gets new items
  appended and its module text updated, new test file), unsigned commits, no trailers, no push.

## What F-348 asks

Acceptance (quoted, binding): "A lattice with per-axis refinement levels gives valid closed meshes for random directional marks
(validate, closure, volume) and transfers linear fields exactly; on the ADR 0247 suction layer a directional run reaches the
isotropic adapted error E(A) with at most half of its 3841 cells; the isotropic AmrMesh paths stay bit for bit."

## What was read and computed before this text (disclosed)

- **Read:** the roadmap entries F-348, F-343, F-342; ADR 0247 in full; ADR 0054 (status, rules, audit, D1 to D7); the section
  list of ADR 0242 and its design as restated in ADR 0247; ADR 0262 D1 to D6 (the keys of `[mesh.adapt]` and what the loop
  takes); `crates/rcfd-mesh/src/adapt.rs` in full; `crates/rcfd-fv/src/adaptive.rs` in full;
  `crates/rcfd-fv/tests/adapt_criteria_f343.rs` (helpers, the layer set-up and its runs); `crates/rcfd-mesh/tests/adapt.rs`
  (`check_topology`, A1, A2 and their limits); `Mesh::validate` (owner < neighbour, faces ordered by (owner, neighbour) with no
  duplicate pair, every directed edge of a cell matched by its reverse exactly once, area vectors summing to zero, positive
  volumes) and `Mesh::from_face_lists`.
- **No run** of any code of this repository was made for this ADR. Costs and numbers are ADR 0247's addendum: `E(A) = 0.01159`
  with `N_A = 3841` cells (levels {0: 185, 1: 165, 2: 220, 3: 275, 4: 2996}), `E(F) = 0.04386` on the uniform 128 x 128, observed
  order 1.86 between 90 and 128, A in 3.7 s.
- **Arithmetic (disclosed):** a paper model of the marking only (a Python script of 60 lines in the scratch folder,
  `f348-scratch/paper_model.py`; no solver, no rcfd code): 16 identical columns of the unit square, cells split along y only,
  ranked by `|u*(y_top) - u*(y_bottom)| (h_x h_y)^(1/2)` with the closed form `u*` of ADR 0247 D7 (the exact variation across the
  cell times the `r = 2` weight), the geometric target schedule, ties to the lower index, a 2:1 balance along y. It gave, for the
  registered run D below (`N_T = 1536`, y level up to 5, six cycles): 1539 cells, levels along y {0: 151, 1: 32, 2: 116, 3: 156,
  4: 212, 5: 872}, the level-5 band reaching `s = 8.1 / R` to `8.75 / R` from the wall; for `N_T = 960` with level 5: 960 cells,
  level-5 band to `4.4 / R` to `5 / R`; for `N_T = 960` with level 4 and five cycles: 969 cells, level-4 band to `10 / R`. The
  model has no x variation, so it says nothing about splits along x.

No code of this ADR existed when this text was written, and no check below was run.

## Design (registered)

### D1. The second lattice type: `rcfd_mesh::adapt::aniso::AnisoMesh`

A new module `crates/rcfd-mesh/src/adapt/aniso.rs` (declared by one line in `adapt.rs`). `AmrMesh` and `Leaf` are not changed.

- `AxisLeaf { levels: [u8; 3], ijk: [i64; 3] }`: a leaf is a dyadic box of the base lattice: along axis `a` it is cell `ijk[a]`
  of the grid of level `levels[a]` (`base[a] 2^levels[a]` cells). With per-axis finest levels `max_level[a]` the integer
  lattice has `base[a] 2^max_level[a]` cells along `a`, and the leaf occupies `[ijk[a] s_a, (ijk[a] + 1) s_a]` with
  `s_a = 2^(max_level[a] - levels[a])`. An unrefined axis has `max_level = 0`. The coordinates of a lattice point are the linear
  interpolation of the base coordinates by the same formula as `AmrMesh::new` (a dyadic fraction of a base interval), so the
  same point gets the same bits in both types.
- `AnisoMesh::new(coords, refine, max_level: [u8; 3], patches)` (each `max_level[a] <= 10`; a non-zero `max_level` on an
  unrefined axis is a located error; the other checks are those of `AmrMesh::new`); `AnisoMesh::from_amr(&AmrMesh)` (the same
  leaves with `levels = [l, l, l]` on the refined axes); `n_cells`, `leaves`, `levels`, `max_levels`, `refined_axes`.
- A box and its key are in one-to-one relation, and a split of a leaf along a set of axes `S` replaces it by `2^|S|` leaves
  (level + 1 along every axis of `S`). Every state reachable from the base by such splits is a tiling of the domain by dyadic
  boxes; the type keeps no history (a box with levels (1, 1) is the same leaf whether x or y was split first).
- Cell order: by the lower corner `(z, y, x)` on the integer lattice (the order of `AmrMesh`; the lower corners of the leaves of
  a tiling are distinct, so it is a total order).

**Marks.** `AxisMark { refine: [bool; 3], coarsen: [bool; 3] }` per cell (`AxisMark::KEEP`, `AxisMark::refine(axes)`,
`AxisMark::coarsen(axes)`); a mark with any `refine` axis is a refinement (refine wins, as in ADR 0054 D2). Axes that are not
refined by the lattice are dropped; a refinement all of whose axes are at their `max_level` is counted in
`ignored_at_max_level` and ignored.

**Point location.** Leaves are kept in a hash set together with the count of every level triple present. The leaf that contains
integer cell `p` is found by trying, for every level triple `L` present, the key `(L, p_a >> (max_level[a] - L_a))`: exactly one
is in the set (tiling). Cost: the number of distinct level triples present, at most `prod_a (max_level[a] + 1)`.

**Leaves over a box.** `overlapping(lo, hi)` returns the distinct leaves that meet an integer box: locate the leaf `n` at the
lower corner; if `n` covers the box it is the answer; otherwise cut the box at `n`'s upper bound along an axis where `n` ends
inside the box and recurse on the two parts. Every cut strictly shrinks a part, every part ends inside one leaf, and the parts
cover the box, so the result is complete for any tiling (first principles; no alignment is assumed).

### D2. Faces between leaves of different per-axis levels

Two leaves `A` (low side) and `B` (high side) share a face normal to `a` when `hi_a(A) = lo_a(B)` and their extents along the
two tangential axes overlap with positive length. The face is the rectangle `side(A) n side(B)`. Because the boxes are convex,
two leaves share at most one face (`Mesh::validate` requires one face per (owner, neighbour) pair). Faces are emitted from the low
side only: for each leaf and each `+a` side inside the domain, the neighbours are `overlapping` of the one-lattice-cell-thick box
just across the plane, and each gives the intersection rectangle. With different levels along the two tangential axes the
rectangle can be a proper part of both sides: `A` finer along `u` and coarser along `v` than `B` gives four faces around a
point where an edge of `A`'s side crosses an edge of `B`'s side; that point is a corner of no leaf. (This cannot happen on the
isotropic lattice, where the face is always the whole side of the finer leaf.)

**Vertices and loops.** `V` = the corners of all leaves and the corners of all internal face rectangles. The loop of a face is
its four corners in the order of `AmrMesh::boundary_loop` (`(lo_u, lo_v), (hi_u, lo_v), (hi_u, hi_v), (lo_u, hi_v)`, normal along
`+a`, reversed when the owner is the high-side cell or for a `-a` boundary face), with every point of `V` that lies on one of its
four edges inserted (collinear extra vertices, ADR 0054 D1).

**Why every cell is closed** (derivation). Each side of a leaf is tiled by the face rectangles on it (the neighbours across the
plane tile the plane, so their intersections with the side tile the side), or is one boundary face. Take a segment between two
consecutive points of `V` on an edge of some face of the cell. (i) If the segment lies inside a side of the cell, it lies on
edges of the rectangle tiling of that side; every corner of a rectangle is in `V` and every loop contains all points of `V` on
its edges, so the segment is a whole loop edge of exactly the two rectangles on either side of it, traversed in opposite
directions (both loops are counter-clockwise seen from outside the cell). (ii) If it lies on an edge of the box, it is a loop
edge of exactly one face on each of the two sides that meet there, again in opposite directions, and both loops contain the same
points of `V` on that box edge. So every directed edge of the cell's outward faces is matched by its reverse exactly once, which
is the closure test of `Mesh::validate`; the area vectors then sum to zero because collinear vertices do not change a polygon's
area vector. This holds for **any** tiling by dyadic boxes; the balance (D3) is for quality and for a bounded stencil.

**Same mesh as `AmrMesh` when the levels are isotropic.** If all leaves have equal levels on the refined axes, the face
rectangles are the whole sides of the finer leaves, `V` is the set of leaf corners, the cell order, the loops and their starting
corners are those of `AmrMesh::to_mesh`, faces are sorted by (owner, neighbour) and points by their integer coordinates in both;
with `max_level = [m, m, m]` on the refined axes the integer lattice is the same. So `AnisoMesh::to_mesh` returns the same
points, faces, owners, neighbours and patches bit for bit (check G5).

### D3. The balance rule per axis, and why it keeps the face matching finite

Rule (B): for every pair of leaves that share a face, `|l_t(A) - l_t(B)| <= 1` for **every** refined axis `t`, normal and
tangential. With isotropic levels it is ADR 0054's 2:1 rule.

- Tangential axes: the extents of `A` and `B` along `t` are dyadic intervals that overlap, so one contains the other; with (B)
  the larger is at most twice the smaller. The intersection along `t` is therefore the whole extent of `A` or one of its two
  halves: a side is cut by at most one mid-line per tangential axis, so it carries at most `2^(d-1)` faces (2 in a slab, 4 in
  3-D), and the neighbour search of D2 makes at most one cut per tangential axis. The possible matchings of a side are the nine
  pairs of tangential level differences in `{-1, 0, 1}^2` (three in a slab), a finite list independent of the depth of the
  lattice; without (B) a side can carry up to `2^(sum of the tangential level differences)` faces, bounded only by `max_level`.
- Normal axis: the ratio of the two cell sizes along the face normal is at most 2, the grading that the face interpolation and
  the non-orthogonal correction of the solvers were checked with on the isotropic lattice (ADR 0054, ADR 0242).

**Enforcement** (`adapt(marks, balance)`), a ripple as in ADR 0054 D4, generalised: a request is `(leaf x, axes S)`, "the region
of `x` must reach level `l_t(x) + 1` along every `t` in `S`". Requests are served in order (the marked leaves first, in cell
order): the leaves now covering `x` (`x` itself, or its descendants if a balance split came first) that are still at `l_t(x)`
along some `t` in `S` are split along those axes. With `balance`, each new child `c` is compared with each of its face
neighbours `n` (D2's search on its six sides): axes with `l_t(c) >= l_t(n) + 2` are requested of `n`, axes with
`l_t(n) >= l_t(c) + 2` of `c`. Each served request raises a level, levels are bounded by `max_level`, and a requested axis is
never at its `max_level` (the requested leaf is at least two levels coarser than an existing one), so the ripple ends; if the
mesh satisfied (B) before, it satisfies (B) after (every new face has a new child on one side and was checked). On isotropic
states with all-axes marks the requests are those of `AmrMesh::adapt`, so the leaf sets are equal (G5).

**Coarsening.** A leaf marked `coarsen` along `t` (and not refined, and with `l_t > 0`) merges with its buddy along `t` (same
levels, `ijk[t]` differing in the last bit) when the buddy is a leaf of the old mesh, is marked `coarsen` along `t` too, neither
has been used by another merge and (with `balance`) the merged leaf keeps (B) with all its face neighbours. One merge per leaf
and call, axes tried in the order x, y, z. A merge is the inverse of a single-axis split; any state has at least one pair of
buddies (the children of the last split of any history that leads to it), so repeated all-axes coarsening without balance
returns to the base (check G6).

### D4. Transfer

`adapt` returns the existing `Transfer` (`Source::Same`, `Descendant`, `Merged`) and fields are moved by the existing
`transfer_field`, unchanged. Why that is enough: a new leaf is an old leaf (`Same`), a part of exactly one old leaf (refinement
only subdivides: `Descendant(i)`, found by locating the new leaf's lower corner in the old set), or the union of old leaves
(`Merged`, the old leaves over its box by D1's search). For a `Descendant`, `q_new = q_i + g_i . (x_new - x_i)` with the
least-squares gradient of the old mesh: exact for a linear field wherever the gradient is (face neighbours spanning the active
axes), and conservative for any split along any set of axes because the descendants of an old leaf tile it, so
`sum_c V_c (x_c - x_i) = 0` (the centroid of a box is the volume-weighted mean of the centroids of any tiling of it). `Merged`
is the volume-weighted mean. Both conserve `sum V q` to round-off, as ADR 0054 D5.

### D5. The directional criterion and how marks choose axes (`rcfd_fv::adaptive`, new items)

Per-axis variation (ADR 0247 D3, first principles: the first-order change of `U` across a box cell along its edge of axis `a`):

`E_{c,a} = ( sum_k (d u_k / d x_a)^2 )^(1/2) h_{c,a}`.

- `axis_variations(mesh, grads, axes) -> Result<Vec<Vec3>>` (`h` from `cell_extents`; zero on inactive axes; the located errors
  of `edge_length_weighted_indicator`), `velocity_axis_variations(solver, axes)` (the least-squares gradients with the boundary
  values, as `velocity_edge_length_indicator`).
- **Which cells:** the scalar ranking is unchanged: ADR 0247's `eta_c = (sum_a E_{c,a}^2)^(1/2) (prod_a h_{c,a})^((r - 1) / d)`
  (`velocity_edge_length_indicator`), so a directional run and an isotropic run rank cells by the same quantity.
- **Which axes** (`directional_axes(variation, levels, max_level, axes, theta) -> Result<[bool; 3]>`): split along `a` when `a`
  is active, below its `max_level`, and `E_{c,a} >= theta max_b E_{c,b}` (the maximum over all active axes). Derivation of the
  rule and of `theta = 1/2`: for a fixed cell measure `h_x h_y` the first-order variation `sum_a (g_a h_a)^2` is smallest when
  `g_x h_x = g_y h_y` (minimise `g_x^2 h_x^2 + g_y^2 A^2 / h_x^2`), that is when the per-axis variations are equal: the
  equidistribution principle of ADR 0247 D3 applied per axis. Splits change `h_a` by factors of two, so equality can be reached
  only within a factor two. Splitting the axis of the largest variation halves it to `E_max / 2`; an axis with
  `E_a >= E_max / 2` would then carry the largest variation and is split too; an axis with `E_a < E_max / 2` is still below the
  split axis after the split and is left. So `theta = 1/2` keeps the per-axis variations within a factor two of each other and
  never splits an axis whose variation is already the smaller one after the step. Consequences of the formula: with no variation
  at all (`E_max = 0`) every eligible axis is split (no directional information: the isotropic split); if the axes that pass
  the test are all at their `max_level` the cell is not eligible (splitting only the axis of small variation adds cells without
  reducing the dominant term). `theta` must be finite and in `(0, 1]` (located error).
- **How many** (`directional_target_marks(indicator, variations, levels, max_level, axes, n_target, theta)`): cells are taken
  in the order of decreasing `eta` (ties to the lower index), skipping cells with no axis to split; a cell with split axes `S`
  adds `2^|S| - 1` cells; the list stops as soon as the planned count reaches `n_target`. Without balance the adapted mesh has
  between `n_target` and `n_target + 2^d - 2` cells whenever enough cells are eligible (ADR 0247 D2 with a per-cell split size).
- **The loop:** `run_adaptive_simple_directional(amr: AnisoMesh, setup, settings, marker)` with
  `marker: FnMut(&SimpleSolver, &AnisoMesh, usize) -> Result<Vec<AxisMark>>` is the loop of `run_adaptive_simple_marked` on the
  second lattice (same steps, same transfer calls, same records; a separate function, the existing one is not touched), returning
  `DirectionalRun { amr, solver, cycles }`; `run_adaptive_simple_directional_target(amr, setup, settings, n_target, r, theta)`
  uses the marks above with `target_schedule(N_0, n_target, cycles, k)`.

### D6. Why a directional run on a boundary layer should need about half the cells or fewer (estimate)

The suction layer has thickness `delta = H / R = 1/80` and varies along y only. A band of thickness `a delta` resolved with
cells of height `h` costs, per unit length of wall, `(a delta / h) (1 / h)` isotropic cells but `(a delta / h) (1 / h_x)`
directional cells with any `h_x`: the saving factor of a band is `h / h_x`, and on the base `h_x = 1/16` it is `2^-l` for the
band of level `l`. The isotropic run A of ADR 0247 spent, from its histogram, 2996 / 256 = 11.7 rows at level 4 (the band to
`3.7 delta`), 275 / 128 = 2.1 rows at level 3, 220 / 64 = 3.4 at level 2, 165 / 32 = 5.2 at level 1 and 185 / 16 = 11.6 at
level 0: about 34 rows. The same grading along y on 16 columns is `34 * 16 = 545` cells, 0.14 of 3841: 78 percent of A's cells
are the level-4 band, and directional refinement removes its factor 16. Half of A's cells (1920) is therefore about 3.5 times
what the same y grading needs, as long as the solution stays one-dimensional; the margin covers what the isotropic estimate
leaves out (splits along x where the discrete solution adjusts to the exact inlet profile, partial rows, balance).

### D7. The directional layer runs (registered)

The suction layer of ADR 0247 D7 unchanged (unit square, slab 0.1, `R = 80`, the patches, conditions, initial fields, solver
settings, `run(20000, 1e-7)`, the error `E`), set up in the new test file by a copy of the helpers of
`adapt_criteria_f343.rs` (a test file cannot import another; the copy is checked by S0).

- **A** (reference, the isotropic path): ADR 0247's run A again (`AmrMesh` 16 x 16, `max_level = 4`, 5 cycles, balance, linear
  transfer, `run_adaptive_simple_target` with `N_T = 3840` and `velocity_edge_length_indicator(r = 2)`).
- **D** (the acceptance run): `AnisoMesh` 16 x 16, `max_level = [5, 5, 0]` (x is free to refine as far as y), **6 cycles** (five
  to reach level 5 and one more, as A's "four plus one"), balance, linear transfer,
  `run_adaptive_simple_directional_target` with **`N_T = 1536`**, **`r = 2`** (A's ranking), **`theta = 1/2`** (D5). Scheduled
  counts `round(256 * 6^(k/6))` = 345, 465, 627, 845, 1139, 1536. Why `N_T = 1536`: it is two fifths of A's 3840 and leaves 384
  cells below the limit 1920 for the last cycle's overshoot (at most 2 from the target rule, and the balance splits; A had none
  in its last cycle). Why level 5: with the factor 16 saved along the wall, the cells go across the layer instead; D6's estimate
  says level 4 would need about 545 cells for A's error, with no margin on the error; one more level along y divides the
  truncation error of the fine band by about four (observed order 1.86). Paper model: the level-5 band reaches about `8 delta`,
  twice as far as A's finest band.
- **Dswap** (negative control): D with the two variations of every cell exchanged before the marks (`E_x` and `E_y` swapped, so
  the marks split the axis of the smaller variation): the cells go along the wall and `h_y` stays near 1/16.
- **D960** and **D4** (recorded only): D with `N_T = 960`; and `max_level = [4, 4, 0]`, 5 cycles, `N_T = 960` (A's finest size).
- Far from the wall `u*'` is below the solver tolerance (`80 exp(-R s) < 1e-5` for `s > 0.2`) and the axis choice there is
  decided by whatever the discrete solution has there (the under-resolved tail of the coarse meshes, or noise). The final
  ranking does not get there (the cells with `s < 0.2` at level 5 along y alone number `0.2 * 512 * 16 = 1638 > 1536`), but the
  first cycle marks 89 of 256 base cells, five to six base rows (`s` up to 0.34), as A's first cycle did; some of those early
  splits may be along x or along both axes. They are counted in `N_D` like any other cell; this is a known weakness of the
  estimate, not of the check.
- Cost (paper): each directional run has at most about 1540 cells and seven solves; A took 3.7 s; a few seconds to a minute each.

Estimate of `E(D)`: the fine band has `h_y = 1/512`, two halvings of F's `h = 1/128`; with ADR 0247's observed order 1.86 a
uniform mesh of that size would give `0.0439 / (2^1.86)^2 = 0.0033`; the coarser levels beyond `8 delta` and the faces between
columns of different y level add to it. Band for `E(D)`: 0.003 to 0.008, `E(D) / E(A)` 0.25 to
0.7.

## Registered checks

New test files `crates/rcfd-mesh/tests/adapt_aniso_f348.rs` (G) and `crates/rcfd-fv/tests/adapt_directional_f348.rs` (C, S, H).
G and C are ordinary tests. S0, H1 to H4 and NC-H1 share one computation (`OnceLock`: A, D, Dswap, D960, D4), are
`#[ignore = "heavy ..."]` and are run once, in release, with `--include-ignored`; C5 is the fast test of the same code path.

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| G1 (acceptance: "valid closed meshes for random directional marks (validate, closure, volume)") | random `AxisMark`s (per cell: refine along a random non-empty set of axes with probability 0.35, coarsen along one with 0.15, else keep; a fixed linear congruential generator), each with balance on and with balance off: (a) graded 3-D box 3 x 2 x 2, `max_level = [2, 2, 2]`, 4 rounds; (b) slab 5 x 4, `[3, 3, 0]`, 5 rounds; (c) 3-D box 2 x 3 x 2 refined along x and z only, `[2, 0, 3]`, 4 rounds. After every round the checks of ADR 0054's `check_topology` | `Mesh::validate` Ok; every internal face in exactly two cells' lists and every boundary face in one; no two faces with the same point set; total volume within `1e-13` relative of the box; every cell volume positive; per cell `|sum S_f| <= 1e-13 sum |S_f|`; `n_cells` = number of leaves; with balance on, the per-axis level jump across every internal face `<= 1` on every axis | exact topology; round-off |
| G1b | the crossing of D2: box 2 x 1 x 1, `[1, 1, 1]`, cell 0 split along y, cell 1 along z | 4 cells, 6 internal faces of which 4 lie in the plane between the base cells, 21 points (12 base corners, 4 + 4 from the two splits, 1 crossing point that is a corner of no leaf); `validate` Ok; the four faces in the plane have equal areas | exact |
| G2 | split sizes | on a 4 x 4 slab, `[2, 2, 0]`: one cell refined along x: 17 cells, two cells of half the volume; along y: 17; along both: 19; a mark along z only (unrefined axis): 16 cells, nothing refined or ignored; a cell at `max_level` along x marked along x: `ignored_at_max_level = 1`; marked along x and y with x at its maximum: split along y only | exact |
| G3 | balance per axis | 8 x 8 slab, `[3, 3, 0]`, the leaf at the origin split along y three times: with balance 65, 67, 70 cells and the level jump along y `<= 1` on every face (the x neighbours are split along y by the tangential rule); without balance 65, 66, 67 cells and a jump of 3 along y | exact (derived by hand: the second split forces the right neighbour once, the third forces two leaves) |
| G4 (acceptance: "transfers linear fields exactly") | on G1's (a) and (b) lattices, 4 rounds (two of refinement marks with probability 0.5, two of all-axes coarsening marks with probability 0.9), balance on, a linear field `1.3 + 0.7 x - 0.4 y + 0.2 z` and a random field, `Injection` and `Linear` | linear field after `Linear` transfer equal to the closed form at the new centres within `1e-12` of its range in every round; `sum V q` of both fields conserved within `1e-13` relative in both modes | ADR 0054 A2's limits; round-off |
| G5 (acceptance: the second type equals the first on isotropic marks) | `AnisoMesh` with all-axes marks against `AmrMesh` with the same `Refine` marks (random, probability 0.3), three rounds, balance on throughout and off throughout: slab 6 x 5 (`max_level` 3) and graded 3-D box 3 x 2 x 2 (`max_level` 2); and `from_amr` of the final `AmrMesh` | same number of leaves, the same `Transfer`, and `to_mesh` equal in points (`==` on the floats), face loops, owners, neighbours, patch names, kinds, starts and sizes, in every round; `from_amr(..).to_mesh()` equal likewise | bit for bit (D2) |
| G6 | coarsening | G1's (a) after 3 rounds of refinement marks (balance off): all-axes coarsen marks repeated, balance off | the base lattice (12 cells, all levels 0) is reached within `n - 12` rounds, `n` the cell count before the first coarsening round; the cell count decreases in every round until then; `sum V q` of a random field conserved within `1e-13` relative in every round | exact: at least one merge per round (D3); the number of rounds is recorded (expected about the sum of the levels, 6 to 12) |
| G7 | located errors | `max_level` above 10; a non-zero `max_level` on an unrefined axis; a mark list of the wrong length | an error naming the argument | exact |
| C1 (closed form, per-axis variations) | graded slab of ADR 0247 K2 (x nodes 0, 0.1, 0.3, 0.6, 1; y nodes 0, 0.2, 0.25, 0.5, 1) as an `AnisoMesh` with cell 5 split along x only; constant gradients `grad u = (2, -3, 0)`, `grad v = (-1, 4, 0)` | `E_x = sqrt(5) h_x`, `E_y = 5 h_y`, `E_z = 0`, relative `1e-13`; `velocity_axis_variations` with the linear `U = (1 + 2x - 3y, 0.5 - x + 4y, 0)` in the cells and on the boundary faces: the same within relative `1e-9`; the two children of cell 5 have half its `E_x` and its `E_y` | exact; least squares exact for a linear field |
| C2 (closed form, axis choice) | `directional_axes`, active axes x and y, levels 0, `max_level = [2, 2, 0]`, `theta = 0.5` | `(1, 0.49, 0)`: x; `(1, 0.5, 0)`: x and y; `(0.2, 1, 0)`: y; `(0, 0, 0)`: x and y; `(1, 0.2, 0)` with x at its maximum: none; `(1, 0.6, 0)` with x at its maximum: y; `theta = 1`: only the largest; a variation on the inactive z never selects z; `theta` = 0, 1.5, NaN: located error | exact |
| C3 (closed form, directional target count) | 10 x 10 slab `AnisoMesh`, `[2, 2, 0]`, indicator `(37 i) mod 100`, variations `(1, 0, 0)` for even `i` and `(1, 1, 0)` for odd `i` | target 108: exactly cells 27, 54, 81, 8 marked (27 and 81 along x and y, 54 and 8 along x), adapted count 108 (balance off); target 109: cell 35 as well, 111 cells; target 107: cells 27, 54, 81, 107 cells; target 100 and 50: none; located errors for mismatched lengths and a NaN indicator | exact (37 * 73 = 1 mod 100, so the indicators 99, 98, 97, 96, 95 belong to cells 27, 54, 81, 8, 35) |
| C4 (the directional loop is the isotropic loop on isotropic marks) | cavity set-up of ADR 0247 K5 (12 x 12, `max_level` 2, 2 cycles, `max_iter = 30`, balance, linear transfer): `run_adaptive_simple_directional` on `AnisoMesh [2, 2, 0]` with the marker "`target_count_marks` of the scaled velocity indicator at `target_schedule(144, 600, 2, k)`, every `Refine` turned into an all-axes mark" against `run_adaptive_simple_target(.., 600, ..)` on `AmrMesh` | cycle records, `u`, `p`, `phi` equal (`==`) and the meshes' points equal | bit for bit |
| C5 (fast test of the heavy path) | the layer, `AnisoMesh` 8 x 8, `[3, 3, 0]`, 3 cycles, `N_T = 100` (scheduled 74, 86, 100: the marks stay in the two base rows next to the wall), `r = 2`, `theta = 0.5`, `run(20000, 1e-7)` | final `validate` Ok; per-axis jump `<= 1`; `100 <= N <= 110`; cells strictly increase; `sum_c l_y(c) > sum_c l_x(c)` and the largest y level is 3 | the wall row is marked in every cycle (its variation is 0.9 against 0.007 for the next); splits along x only where the discrete solution adjusts to the inlet profile (the first of eight columns) |
| S0 | the copied set-up is ADR 0247's | `N_A = 3841` and `|E(A) - 0.01159| <= 1e-5` | the same code path, unchanged |
| H1 (acceptance: "reaches the isotropic adapted error E(A)") | `E(D)` against `E(A)` of the same run | `E(D) <= E(A)` | `E(D) / E(A)` 0.25 to 0.7 |
| H2 (acceptance: "with at most half of its 3841 cells") | `N_D` | `N_D <= 1920` | 1536 to 1560 |
| H3 | the target lands | `|N_D - 1536| <= 153.6` | 1536 to 1560 |
| H4 | D sound | every solve of D converged; final `validate` Ok; per-axis jump `<= 1`; cells strictly increase over the cycles; leaves at y level 5 present | met, as ADR 0247 L4 |
| NC-H1 | the axis choice matters | `E(Dswap) > E(A)` | `h_y` stays near 1/16: `E` above 0.2 |
| K-iso (acceptance: "the isotropic AmrMesh paths stay bit for bit") | `git diff main -- crates/rcfd-mesh/src/adapt.rs crates/rcfd-fv/src/adaptive.rs` removes no code line (doc lines of the module text only); `rcfd-mesh` `tests/adapt.rs`, `rcfd-fv` `tests/amr_solve.rs`, `amr_diag.rs`, `adapt_cavity_f102.rs` and `adapt_criteria_f343.rs` run unedited, the last two with `--include-ignored` (their recorded numbers are asserted or printed) | all pass; `adapt_criteria_f343` prints `N_A = 3841`, `E(A) = 1.159e-2` | no executed line of those paths changes |

Recorded, not counted: every `E` and cell count; the cycle records and the histograms of the x and y levels of D, Dswap, D960
and D4; the number of leaves of D with `l_x > 0` and where they are (the x of their centres); `E(D960)`, `E(D4)`; times.

### Negative controls

NC-H1 above. If it fails, the layer comparison does not show that the choice of axes does anything, and H1 proves nothing about
the criterion. G3's run without balance must show a jump of 3 along y (otherwise the balance checks are vacuous).

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final commit in `w49i-work/f348-scratch` by an exact text replacement (each
pattern found once), the G and C tests run once in release in their own target directory; the heavy tests are not run under
mutations.

| Id | Mutation | Must |
| --- | --- | --- |
| M1 | `to_mesh` does not add the corners of the face rectangles to the vertex set | G1b fails |
| M2 | the balance compares the normal axis only (tangential axes skipped) | G3 fails |
| M3 | `adapt` splits every refinable axis whatever the mark says | G2 fails |
| M4 | `overlapping` returns the leaf at the lower corner only (no recursion) | G1 fails |
| M5 | a merged cell's sources list only the first old leaf | G4 or G6 fails |
| M6 | `axis_variations` multiplies the x derivative by `h_y` and the y derivative by `h_x` | C1 fails |
| M7 | `directional_axes` ignores `theta` (every eligible axis) | C2 fails |
| M8 | `directional_target_marks` counts one added cell per marked cell | C3 fails |
| M9 | `run_adaptive_simple_directional` replaces every mark's axes by all axes | C5 fails |
| M10 | the directional loop transfers by injection whatever the settings say | C4 fails |

### Predictions (mine)

| Id | Prediction |
| --- | --- |
| P1 | G1 to G7 and C1 to C4 met at the first run |
| P2 | `N_D` between 1536 and 1560; `E(D) / E(A)` between 0.25 and 0.7; H1 to H4 met |
| P3 | fewer than 15 percent of D's leaves have `l_x > 0`, most of them in the first two base columns (`x < 1/8`) |
| P4 | `E(Dswap)` above 0.2 |
| P5 | `E(D4)` within 30 percent of `E(A)` with about a quarter of its cells; `E(D960)` below `E(A)` |

## Decision rule (registered)

- **done**: G1, G1b, G2 to G7, C1 to C5, S0, H1 to H4 and K-iso met and NC-H1 holding. Mutations not caught are reported as
  gaps of the checks.
- **partial**: the lattice clauses met (G1 to G7, C1 to C4, K-iso) but H1 or H2 missed, or a supporting check (C5, H3, H4, S0)
  missed; each miss a `*_known_miss` with its number and a diagnosis; or NC-H1 failing (the layer case then cannot discriminate).
- **todo**: K-iso fails or any existing test changes (a default moved), or G1, G4 or G5 fails (the lattice is not valid).

## Limits of this item

- Box lattices only (tensor base coordinates), as `AmrMesh`. One layer flow, one base, one target, one `theta`, one exponent,
  chosen here from the arithmetic above and not tuned after the run. The layer varies along y only, so the case tests the
  saving along a wall-aligned layer; a layer oblique to the axes gains nothing from axis-aligned splits.
- The balance rule is on faces only (as ADR 0054): leaves that touch along an edge only are not constrained.
- Coarsening merges one buddy pair along one axis per leaf and call; it is not used by the loop (as ADR 0242).
- Steady SIMPLE only. No case key: `[mesh.adapt]` (ADR 0262) is not touched; the addendum lists what the key would need.
- Point location costs the number of distinct level triples present per query; no tree is kept.

## Open questions and choices for the maintainer

- **Q1. A second type or a change of `Leaf`.** Taken on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation), as ADR 0247 R1 (a): a second type, `AmrMesh` untouched.
- **Q2. The balance on the normal axis as well as the tangential ones.** Options: (a) every refined axis (D3); (b) tangential
  only (fewer forced splits, unbounded size ratio along the normal). Taken on the same standing instruction: (a), which is ADR
  0054's rule on isotropic states.
- **Q3. Reading of "a directional run ... with at most half of its 3841 cells".** Read as: any registered directional run with
  `N <= 1920` and `E <= E(A)`; the finest level along y is not held at A's (D7 says why; D4 records the run at A's finest size).
  Taken on the same standing instruction.
- **Q4. The case key.** Not taken here (it changes the schema golden and `rcfd-api`, which this item must not touch): listed in
  the addendum for a follow-up.

## Amendment 1 (2026-11-12, written after the first run of the G checks and before the run of G6b and G6c)

**What happened.** The G checks were run once (`cargo test --release -p rcfd-mesh --test adapt_aniso_f348`): G1, G1b, G2, G3, G4,
G5 and G7 passed; **G6 missed**: on lattice (a) (3-D), 146 cells went 82, 51, 35, 26, 20, 16 in six rounds of all-axes
coarsening and the seventh round merged nothing, with 16 cells left instead of 12. The registered test stays, unchanged, as the
ignored record `g6_coarsening_returns_to_the_base_known_miss`.

**Diagnosis** (one diagnostic run that repeated G6's sequence and printed the leaves of the stuck state; no limit involved). Eleven
base cells are whole; the twelfth is tiled by five leaves: two octants at opposite corners and three half-slabs, one along each
axis (levels (1,1,0), (0,1,1), (1,0,1) and two (1,1,1)). No two of the five are buddies, so no pair merge applies. The cause is
the derivation of D3, not the code: the sentence "any state has at least one pair of buddies (the children of the last split of
any history that leads to it)" assumed that every state has a history of splits. That is true of the states made by splits, but
pair merges can leave the set of split-reachable tilings in three dimensions: starting from the eight octants of a cell, merging
one pair along x, one along y and one along z (each a legal merge of two buddies) gives the five-box tiling above, which has no
plane that cuts the cell in two and so cannot be made by splits. In two dimensions this cannot happen (derivation, first
principles; the fault-line property of dyadic tilings of the square, known from Lagarias, Spencer and Vinson, "Counting dyadic
equipartitions of the unit square", cited from memory, the argument is given here): a dyadic interval that contains the
mid-point of the cell in its interior is the whole cell extent, so a leaf that crosses the vertical mid-line has the full width
and a leaf that crosses the horizontal mid-line has the full height; two such leaves would overlap, so unless the cell is one
leaf at least one mid-line is crossed by no leaf and cuts the tiling in two; by induction every tiling of a slab cell is made by
splits, its last split gives a pair of buddies, and every round of all-axes coarsening merges at least one pair. In three
dimensions a full-x leaf, a full-y leaf and a full-z leaf can avoid each other pairwise, which is the stuck tiling.

**What it means.** The stuck tiling is a valid tiling by dyadic boxes: D2's closure derivation assumes nothing else, and point
location, the neighbour search, the balance and the transfer do not use a split history. So meshes stay valid; what is lost is
the guarantee that directional coarsening in 3-D can always undo a refinement. D3's sentence is replaced by: "in a slab (two
refined axes) repeated all-axes coarsening without balance returns to the base; with three refined axes pair merges can leave a
five-box tiling of a cell that they cannot merge further (a valid mesh; refinement still works on it)". The loop of
`rcfd_fv::adaptive` does not coarsen. Nothing in the code is changed by this amendment.

**Corrected checks** (new tests; G6 stays as the ignored record):

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| G6b | coarsening in a slab: lattice (b) (5 x 4, `[3, 3, 0]`) after 3 rounds of refinement marks (probability 0.5, random non-empty axis sets, balance off, the generator seed of G6), then all-axes coarsen marks repeated, balance off | the base (20 cells, all levels 0) is reached within `n - 20` rounds; the cell count decreases in every round until then; `check_topology` and `sum V q` of a random field within `1e-13` relative in every round | exact (the fault-line argument above); the number of rounds is recorded |
| G6c | the stuck tiling is a valid mesh and a fixed point of pair coarsening: one base cell `[0, 1]^3`, `max_level = [1, 1, 1]`, split into octants, then the three merges above (the octant pairs at (y = 1, z = 0) along x, at (z = 1, x = 0) along y, at (x = 1, y = 0) along z) | 5 cells, 3 merges; `check_topology` Ok with volume 1; all-axes coarsen marks then merge nothing (5 cells, the same leaves); all-axes refine marks then give the 8 octants again | exact |

Decision rule: unchanged. G6 is a missed registered check, so by the rule the item cannot be **done**; G6b and G6c stand in for it
in the list of supporting checks.

## Amendment 2 (2026-11-12, written after the registered run of the C, S and H checks and before the run of C5b)

**What happened.** The registered run of `adapt_directional_f348` (once, release, `--include-ignored`, at 22b48cc1) met C1 to C4,
S0, H1 to H4 and NC-H1 (numbers in the addendum). **C5 failed** after 277 s with the loop's error "adaptive cycle 3:
edge_length_weighted_indicator: grads[0][0] is not finite": the solution of the second adapted mesh was not finite. The registered
test stays, unchanged, as the ignored record `c5_directional_layer_loop_fast_known_miss`.

**Diagnosis** (four diagnostic runs with a temporary test file that was not committed; logs `f348-scratch/diag_c5.log`,
`diag_c5b.log`, `diag_iso.log`, `diag_cycle1.log`; no registered limit involved):

- The 8 x 8 base of C5 has cell Peclet number `V h / nu = 80 / 8 = 10`. Its converged solution (172 iterations) is not a coarse
  picture of the layer: `u` is -1.86 to -1.96 in the whole wall row where the closed form lies in `[0, 1]` (`E = 1.97`), the
  overshoot of the unlimited linear-upwind scheme. ADR 0247 and D7 of this ADR use a 16 x 16 base (cell Peclet 5), on which every
  solve of A, D, Dswap, D960 and D4 converged.
- From that solution the first directional step (target 74) splits six cells: the two wall-side cells of the first column along
  both axes and four wall cells (columns 1, 3, 4, 7) along y only, leaving columns 2, 5, 6 of the wall row unsplit. The mesh is
  valid (`validate` Ok, 74 cells), but SIMPLEC does not converge on it: `|u|` wanders between 10 and 270 for 6375 iterations,
  from the transferred solution (linear or injection) **and from the case's initial fields**. So neither the transfer nor the
  starting field is the cause.
- With the same six cells split along all axes (82 cells) the solve converges in about 290 iterations from all three starts (to
  an equally wrong solution, `E = 1.58`), and the isotropic loop on the same base converges in every solve (targets 100 and
  160). With other targets the directional loop on this base also converges (one cycle to 100: 101 cells; two cycles: 80 and
  107 cells).
- Conclusion: the set-up of C5 is defective. Its base is outside the range in which the scheme gives a bounded solution, and on
  such a solution a wall row split along y in alternate columns gives a mesh on which this solver set-up (unlimited linear
  upwind, SIMPLEC) has no stable iteration. It is a property of the solver on an unresolved layer, shown by the cold start, not
  an error of the lattice (the mesh is closed and valid, and the same code converges on the registered base); but it is also a
  real limit of directional refinement that the isotropic split does not share on this base, and it is reported as such in the
  addendum.

**Corrected check** (new test; C5 stays as the ignored record). Disclosed: this is written with the registered heavy run known;
in particular D960's first three cycles (321, 398 and 498 cells on the 16 x 16 base) converged in 76, 60 and 47 iterations and
its final level sums were 387 along x and 3142 along y.

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| C5b (fast test of the heavy path) | the layer on the registered base: `AnisoMesh` 16 x 16, `[3, 3, 0]`, 3 cycles, `N_T = 400` (scheduled 297, 345, 400), `r = 2`, `theta = 0.5`, `run(20000, 1e-7)` | every solve converged; final `validate` Ok; per-axis jump `<= 1`; `400 <= N <= 440`; cells strictly increase; `sum_c l_y(c) > sum_c l_x(c)`; the largest y level is 3 | the first cycles of D960; the wall row carries the largest indicator in every cycle |

Decision rule: unchanged. C5 is a missed supporting check (defective set-up), so by the rule the item cannot be **done**; C5b
stands in for it.

## Addendum 1: results (2026-11-12)

Commits: d06e3ed0 (this registration alone), a583aadb (amendment 1), 22b48cc1 (the code and the tests), 1dbee299 (amendment 2),
96a963da (C5b), b764e603 (lint fixes in the lattice tests). Runs, all in release:

- G checks: `cargo test --release -p rcfd-mesh --test adapt_aniso_f348`, once, on the code that became 22b48cc1 (run before
  that commit; the commit differs from the run state by `cargo fmt` only); G6b and G6c once after amendment 1; the whole file
  again at b764e603 after the lint fixes (9 passed, 1 ignored).
- C, S, H checks: `cargo test --release -p rcfd-fv --test adapt_directional_f348 -- --include-ignored --nocapture
  --test-threads=4`, once, at 22b48cc1 (10 passed, C5 failed, 277 s); C5b once after amendment 2 (2.5 s).
- No limit was changed after a run. Two registered checks missed (G6, C5); each has its amendment above, written and committed
  before the corrected check was run, and each registered test is kept unchanged as an ignored record.

### Results

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| G1 (acceptance) | validate, face counts, no duplicate face, volume `1e-13`, closure `1e-13`, jump `<= 1` with balance | all 26 rounds of the six sequences; cells (a) 12 to 226 (balance) and 162 (no balance, jumps 2, 2, 2), (b) 20 to 254 and 179 (jumps 3, 3), (c) 12 to 117 and 77 (jumps 2 along x, 3 along z); with balance every jump 1 | yes |
| G1b | 4 cells, 6 internal faces (4 in the plane, equal areas), 21 points, validate | all | yes |
| G2 | exact counts and levels | 17, 17, 19, 16 cells; ignored 1; the y-only split at the x maximum | yes |
| G3 | 65, 67, 70 with balance (jump 1); 65, 66, 67 without (jump 3) | as registered | yes |
| G4 (acceptance) | linear field within `1e-12` of its range; `sum V q` within `1e-13` | worst linear error 4.4e-16 (both lattices, every round); integrals conserved in both modes; merges 25 and 14 on (a), 26 and 13 on (b) | yes |
| G5 (acceptance) | same leaves, `Transfer` and mesh bit for bit | slab 60, 147, 330 cells (balance) and 60, 120, 228 (none); box 47, 208, 369 and 47, 173, 236; `from_amr` equal | yes |
| G6 | the base within `n - 12` rounds on the 3-D lattice (a) | 146 cells to 82, 51, 35, 26, 20, 16; the seventh round merged nothing (16 cells) | **no** (amendment 1) |
| G6b (amendment 1) | the slab returns to its base within `n - 20` rounds | 105 cells back to 20 in 6 rounds, integrals conserved | yes |
| G6c (amendment 1) | the stuck tiling: 5 cells, valid, no further merge, refinement restores the octants | all | yes |
| G7 | located errors | all | yes |
| C1 | `E_x = sqrt(5) h_x`, `E_y = 5 h_y` to `1e-13`; velocity form to `1e-9`; children | constant gradients exact to `1e-13`; velocity form worst 4.4e-16 | yes |
| C2 | exact axis choices and errors | all | yes |
| C3 | exact marks; 108, 111, 107, 100, 100 cells | all | yes |
| C4 | records, `u`, `p`, `phi`, points equal | equal (`==`) | yes |
| C5 | fast layer run on an 8 x 8 base | failed: "adaptive cycle 3: ... grads[0][0] is not finite" after 277 s | **no** (defective set-up, amendment 2) |
| C5b (amendment 2) | converged; validate; jump `<= 1`; `400 <= N <= 440`; `sum l_y > sum l_x`; y level 3 | 256, 297, 347, 402 cells in 89, 77, 68, 65 iterations, all converged; level sums 77 (x) and 414 (y); y levels {0: 226, 1: 42, 2: 30, 3: 104}; `E = 0.0438` | yes |
| S0 | `N_A = 3841`, `|E(A) - 0.01159| <= 1e-5` | 3841; `E(A) = 0.0115860` | yes |
| H1 (acceptance) | `E(D) <= E(A)` | `E(D) = 0.0029752`, `E(A) = 0.0115860` (ratio 0.257) | yes |
| H2 (acceptance) | `N_D <= 1920` | 1547 (0.403 `N_A`) | yes |
| H3 | `|N_D - 1536| <= 153.6` | 1547 | yes |
| H4 | converged, validate, jump `<= 1`, cells increase, y level 5 present | 256, 345, 465, 630, 853, 1142, 1547 cells in 89, 76, 60, 47, 50, 115, 71 iterations, all converged; jump 1, 1; 748 leaves at y level 5 | yes |
| NC-H1 | `E(Dswap) > E(A)` | 1.176 (1740 cells) | holds |
| K-iso (acceptance) | no code line removed; the existing tests pass unedited | `git diff main` of `adapt.rs`: 3 lines added, none removed; of `adaptive.rs`: two doc lines of the module text replaced, one `use` line and the new items added, no code line removed. `rcfd-mesh` `tests/adapt.rs` 12 passed; `rcfd-fv` `adapt_cavity_f102` 12 passed and `adapt_criteria_f343` 19 passed, both with `--include-ignored` (`N_A = 3841`, `E(A) = 1.15860e-2`, `E(F) = 4.38555e-2`, as ADR 0247); `amr_diag` 1 passed; `amr_solve` 2 passed | yes |

Recorded, not counted. D: x levels {0: 843, 1: 360, 2: 227, 3: 109, 4: 8}, y levels {0: 164, 1: 213, 2: 110, 3: 146, 4: 166,
5: 748}; 704 leaves (45.5 percent) have `l_x > 0`, 418 of them at `x < 1/8`, the farthest at `x = 0.484` and down to `y = 0.344`.
D960 (`N_T = 960`, level 5): 973 cells, `E = 0.0029759`, 247 leaves with `l_x > 0`. D4 (`N_T = 960`, level 4, A's finest size): 960
cells, `E = 0.0115845`, that is A's error with a quarter of A's cells. Dswap: 1740 cells, x levels up to 5 (912 leaves), y levels
at most 4. Times: A 2.6 s, D 2.0 s, Dswap 17.0 s, D960 1.5 s, D4 1.0 s. C5b's 402 cells have the error of ADR 0247's uniform
128 x 128 (0.0439 with 16384 cells).

Against the predictions: P1 not met (G6 missed; the others met at the first run). P2 met (`N_D = 1547`, ratio 0.257, at the low
end of the band). P3 **not met**: 45 percent of D's leaves are split along x, as far as mid-channel. They come from the early
cycles, as D7 warned: on the 16 x 16 and the first adapted meshes the layer is not resolved, the discrete solution differs from
the inlet profile over several columns and the criterion follows it; the loop does not coarsen, so those cells stay. D960 shows
what is needed for the same error: 973 cells. P4 met (1.18). P5 met (`E(D4) / E(A) = 0.9999` with 0.25 of the cells;
`E(D960) < E(A)`). `E(D)` and `E(D960)` agree to four digits: the error is that of the `h_y = 1/512` band (estimate 0.0033).

### Mutations (scratch copy `w49i-work/f348-scratch/mut`, a `git archive` of b764e603, target dir `f348-scratch/target-mut`, release)

Each applied alone by exact replacement (each pattern found once; `f348-scratch/mutate.py`), the files restored and compared with
the worktree afterwards (identical). M1 to M5: the lattice tests; M6 to M10: the non-ignored tests of
`adapt_directional_f348` (C1 to C4 and C5b; C5 is ignored since amendment 2, so M9 is judged by C5b).

| Id | Mutation | Failed tests | Caught |
| --- | --- | --- | --- |
| M1 | face-rectangle corners not added to the vertex set | G1b, G1, G4 | yes |
| M2 | balance on the normal axis only | G3, G1, G5 | yes |
| M3 | every refinable axis split whatever the mark | G2, G3, G1b | yes |
| M4 | `overlapping` without recursion | G1, G1b, G2, G4, G5, G6b, G6c | yes |
| M5 | a merged cell's sources truncated to one | G4, G6b, G6c | yes |
| M6 | edge lengths of x and y exchanged in `axis_variations` | C1 | yes |
| M7 | `theta` ignored | C2, C3, C5b | yes |
| M8 | one added cell counted per marked cell | C3 | yes |
| M9 | the loop turns every mark into an all-axes mark | C5b | yes |
| M10 | the loop transfers by injection | C4 | yes |

### Format and lint

`cargo fmt -p rcfd-mesh --check` and `cargo fmt -p rcfd-fv --check` clean; `cargo clippy --release -p rcfd-mesh -p rcfd-fv
--all-targets -- -D warnings` clean at b764e603 (the first clippy run found three lints in the new lattice test file, a complex
type and two needless vectors, fixed in b764e603 without changing what the tests do).

### Deviations and what was not run

- The registered sentence of D3 on coarsening is wrong in three dimensions and is replaced by amendment 1.
- The G checks were first run on the uncommitted code (committed afterwards as 22b48cc1 after `cargo fmt`); the fv checks were
  run on the commit.
- Beyond the registered text: G5 also compares the `AdaptReport`s; G4 also asserts that each round refined or merged something;
  `directional_axes` also refuses negative or non-finite variations.
- Diagnostic runs (not registered, temporary uncommitted test files, logs in the scratch folder): one for G6 (the leaves of the
  stuck state) and four for C5 (amendment 2).
- `amr_solve` was run with `--include-ignored` by mistake of the command line: its ignored record
  `a4_adapted_vs_fine_known_miss` (ADR 0054) fails by design, as its name says; the two ordinary tests passed.
- Not run: the whole workspace suite (by instruction); the rest of the `rcfd-mesh` and `rcfd-fv` suites; the heavy tests under
  mutation; a gate on an archive copy; clippy in the debug profile. `rcfd-api` is not touched; its `baseline_hashes` (3 passed, 1 ignored)
  and `adapt_case_f342` (6 passed, 2 heavy ignored) tests were run as a precaution, in release, and pass.

### What the `[mesh.adapt]` key would need (not built here)

The loop is reachable from Rust only. To reach it from a case (ADR 0262's table), without changing any case that does not ask:

- a key `lattice = "isotropic" | "directional"`, default `"isotropic"` (today's path, `AmrMesh`, untouched);
- with `"directional"`: `AnisoMesh::new` on the generator's coordinates with per-axis finest levels (`max_level` for every
  refined axis, or a new optional `max_level_axes = [lx, ly, lz]`); `marking = "target_count"` mapped to
  `run_adaptive_simple_directional_target(amr, setup, settings, target_cells, edge_length_exponent, axis_ratio)` with a new
  key `axis_ratio` (`theta`, default 0.5, in (0, 1]); `indicator` must be `"edge_length"`;
- not available on the directional lattice and to be refused with a located error until built: `marking = "fixed_fraction"`
  (needs a fixed-fraction form of `directional_target_marks`) and `register` (needs `restrict_marks` and `register_marks` for
  `AxisMark`; a few lines each);
- in `sim.rs` the pending run and `Simulation::adaptive_cycles` take either lattice (`CycleRecord` is shared);
- the stop reason should say when a solve of a cycle did not converge (amendment 2 shows it can happen on an unresolved base);
- new keys change the schema golden and `docs/case-schema.md`: that step needs the maintainer's approval.

### Decision by the registered rule

**partial.** Every clause of the acceptance is met: valid closed meshes for random directional marks (G1, G1b), linear fields
transferred exactly (G4), the directional run reaches `E(A)` with 1547 of 3841 cells (H1, H2; 0.26 of the error with 0.40 of the
cells), the isotropic paths bit for bit (G5, C4, K-iso), NC-H1 holds and all ten mutations are caught. The rule does not allow
**done** because two registered checks missed: G6 (my derivation of D3 assumed a split history; in 3-D pair merges can leave a
five-box tiling that they cannot undo; corrected by G6b and G6c) and C5 (a defective set-up: an 8 x 8 base at cell Peclet 10;
corrected by C5b). Neither is a clause of the acceptance; both are recorded as ignored known misses with their diagnosis.

### Remaining questions

- **R1. Status.** Options: (a) partial, as the rule gives, until R2 is settled; (b) done, since every acceptance clause is met
  and both misses were of supporting checks, corrected under amendments. Recommendation (a) by the house rule; (b) is the
  maintainer's to give.
- **R2. Directional coarsening in three dimensions.** Pair merges can leave a tiling they cannot merge further. Options: (a) a
  box merge: all leaves inside a dyadic box merge into it when every one of them is marked (no history needed, undoes the
  five-box tiling); (b) keep a split history per base cell and merge only what a split made (changes what the type stores); (c)
  leave it (the loop does not coarsen). Recommendation (a), as a follow-up item; no default is involved.
- **R3. Directional refinement of an unresolved layer.** On the 8 x 8 base the first directional mesh has no converged SIMPLEC
  solution with unlimited linear upwind, where the isotropic split has one (amendment 2). Options: (a) document it (done here
  and in `docs/meshing.md`); (b) when the case key is built, stop the loop with a located message when a cycle's solve does not
  converge; (c) look for the cause in the solver (the limiter, the face interpolation at faces between columns of different y
  level). Recommendation (a) now and (b) with the key; (c) only if a resolved case shows it.
- **R4. Cells spent along x in the early cycles** (45 percent of D's leaves). Options: (a) leave it (the acceptance is met with
  margin); (b) coarsening in the loop (needs R2 for 3-D); (c) a floor on the variation below which an axis is not split (a new
  parameter of the criterion). Recommendation (a) now, (b) later; no default is involved.
- **R5. The case key.** As listed above; it changes the schema golden. Recommendation: a follow-up item.

### For the lead: roadmap entries

Proposed status of F-348: **partial**. New note:

> PARTIAL (ADR 0267, 2026-11-12): rcfd_mesh::adapt::aniso::AnisoMesh, a second lattice with one refinement level per axis (AxisLeaf, AxisMark; AmrMesh untouched): faces are the intersections of the sides of two leaves, with the corners of all face rectangles in the vertex set, so cells are closed for any tiling by dyadic boxes; optional balance of one level per refined axis across every face (at most 2 faces per side in a slab, 4 in 3-D); the existing Transfer and transfer_field. Random directional marks on a graded 3-D box, a slab and a box refined along two of three axes give valid closed meshes (validate, closure and volume to 1e-13) and linear fields transfer to 4.4e-16; on isotropic marks the mesh, the transfer and the adaptive loop equal AmrMesh's bit for bit. Directional criterion in rcfd_fv::adaptive (axis_variations, directional_axes with theta = 1/2, directional_target_marks, run_adaptive_simple_directional and _target). On the ADR 0247 suction layer the directional run has error 0.00298 with 1547 cells against the isotropic 0.01159 with 3841 (limit: at most 1920 cells); held at the isotropic finest size it reaches 0.01158 with 960 cells; with the axis choice inverted 1.18. ADR 0054, 0242 and 0247 tests pass unedited (the heavy ones included); ten mutations each failed their tests. Partial by the rule because two registered supporting checks missed and were corrected under amendments: pair coarsening in 3-D can leave a five-box tiling of a cell that it cannot merge further (valid mesh; slabs return to the base), and the fast layer test on an 8 x 8 base (cell Peclet 10) had no converged solution on its first directional mesh (replaced by a 16 x 16 base). Open: a box merge for 3-D coarsening (R2), the case key (R5), cells spent along x in early cycles (R4).

Note of F-343 (one sentence added at the end; the rest unchanged):

> PARTIAL (ADR 0247): opt-in criteria in rcfd_fv::adaptive with closed-form marking checks: target cell count (target_count_marks, geometric target_schedule, run_adaptive_simple_target), edge-length weighting (cell_extents, edge_length_weighted_indicator, velocity_edge_length_indicator with exponent r), registers (Register from mask, region indicator, field range or levels, with and, or, complement and face-neighbour growth; register_marks, restrict_marks) and the loop with any marks (run_adaptive_simple_marked; run_adaptive_simple and _with unchanged bit for bit). Target-count runs on the ADR 0242 cavity land at 2307 for 2304 and 4611 for 4608 (limit 10 percent). Suction-layer case (Couette flow with transpiration, R = 80, closed form): the adapted mesh (16 x 16 base, five target-count cycles, 3841 cells) has error 0.0116 against 0.0439 for the uniform 128 x 128 (16384 cells); the uniform 90 x 90 (8100 cells) misses it (0.0846); the inverted indicator gives 1.20. ADR 0242's surviving MU6 is now caught. Not delivered: anisotropic (directional) refinement, which needs per-axis levels in the lattice (follow-up); merge-based polyhedral conversion, multigrid precoarsening and parallel adaptation stay listed here until split. Follow-up (ADR 0247 R1, recommendation taken on the maintainer's standing instruction of 2026-11-11): F-348; the new options join F-342's case key (R3). Follow-up (ADR 0247 R1, recommendation taken on the maintainer's standing instruction of 2026-11-11): F-348; the new options are case keys since ADR 0262 (2026-11-12, F-342): [mesh.adapt] marking = "target_count", indicator = "edge_length", register and register_use. The anisotropic criterion is delivered by F-348 (ADR 0267, 2026-11-12): a second lattice with per-axis levels and a directional criterion; on this suction layer 1547 cells give error 0.00298.

Follow-up item 1: title "Case key for directional adaptation ([mesh.adapt] lattice = directional)", deps F-348 and F-342, acceptance:

> With lattice = "directional" in [mesh.adapt] a steady box-mesh case runs the directional loop and equals the library run bit for bit on the ADR 0267 suction layer (1547 cells, error 0.00298); options the directional lattice does not take are located errors; cases without the key give identical hashes.

> TODO (from ADR 0267 R5): keys lattice, axis_ratio (theta) and per-axis finest levels; fixed_fraction and register on the directional lattice need small additions to rcfd_fv::adaptive; a cycle whose solve does not converge should be named in the stop reason (ADR 0267 amendment 2). Changes the schema golden.

Follow-up item 2: title "Directional coarsening that always undoes a refinement in three dimensions (box merge)", deps F-348, acceptance:

> On a lattice with three refined axes, repeated all-axes coarsening returns every state made by splits and merges, including the five-box tiling of ADR 0267 amendment 1, to the base with sum V q conserved; slab results and the isotropic AmrMesh paths stay bit for bit.

> TODO (from ADR 0267 R2 and amendment 1): pair merges along one axis can leave a tiling of a cell with no two buddies; merge all leaves inside a dyadic box when every one is marked. Needed before the adaptive loop can coarsen on the directional lattice (ADR 0267 R4).
