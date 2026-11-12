# ADR 0343: F-441 registration: box coarsening inside the directional adaptive loop

Status: **REGISTERED 2026-11-12**, written before any code of F-441 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0267](0267-f348-registration-anisotropic-refinement-per-axis-levels.md) (F-348: `AnisoMesh`, the directional
criterion `directional_axes` with `theta = 1/2`, `directional_target_marks`, the loop `run_adaptive_simple_directional`, the
suction-layer run D with 1547 cells and error 0.0029752, 704 of its leaves split along x, R4),
[ADR 0341](0341-f439-registration-box-merge-for-directional-coarsening.md) (F-439: `AnisoMesh::adapt_with` and
`Coarsening::Box`, D3's return to the base, D5's transfer, R3), [ADR 0247](0247-f343-registration-adaptation-criteria-and-suction-layer.md)
(the suction layer, `target_schedule`, the edge-length indicator with `r = 2`) and
[ADR 0242](0242-f102-registration-adapted-cavity-and-solver-loop-hook.md) (the loop; "no coarsening in the loop").
Roadmap item F-441 (depends on F-439). Number 0343 given by the lead. Branch `f441-directional-loop-coarsening` from main
7459291d, worktree `w49i-work/f441`.

House rules (as ADR 0267 and 0341):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment committed before the corrected run, the old one stays as an ignored record; each registered
  run is made once;
- defaults stay bit-identical: no existing function, type or signature of `rcfd_fv::adaptive` or `rcfd_mesh::adapt` changes;
  everything new is a new public item appended to `crates/rcfd-fv/src/adaptive.rs` (and its module text);
  `crates/rcfd-mesh/src/adapt/aniso.rs` is expected not to change at all; no existing test or ignored record is edited; no hash
  table and no golden is touched; nothing in `rcfd-api` is touched;
- no new third-party crate, no `unsafe`, the crate graph unchanged; one new test file
  `crates/rcfd-fv/tests/adapt_directional_coarsen_f441.rs`; unsigned commits, no trailers, no push.

## What F-441 asks

Acceptance (quoted, binding): "On the ADR 0267 suction layer, a directional loop that also box-coarsens cells whose per-axis
variation is below a registered fraction of the largest reaches the error E(D) = 0.00298 of ADR 0267's run D with fewer than its
1547 cells and fewer leaves split along x than its 704; runs that do not ask for coarsening are bit for bit unchanged."

Reading of the clauses (each taken on the maintainer's standing instruction of 2026-11-11, take the recommendation; Q1 to Q3
below give the options):

- "per-axis variation below a registered fraction of the largest": `E_{c,a} < phi max_b E_{c,b}`, the largest of the same
  cell's per-axis variations, the quantity ADR 0267 D5's refinement test `E_{c,a} >= theta max_b E_{c,b}` compares (Q1);
- "reaches the error E(D) = 0.00298": `E(DC) <= 0.00298`, the number as the acceptance quotes it (3 significant digits; D's
  measured value was 0.0029752) (Q2);
- "with fewer than its 1547 cells and fewer leaves split along x than its 704": `N_DC < 1547` and the number of leaves with
  `l_x > 0` below 704, on the final mesh of the registered coarsening run DC;
- "runs that do not ask for coarsening are bit for bit unchanged": every existing function gives the bits it gave (no code line
  of them changes; their tests pass unedited), and the new loop with no coarsening asked (`phi = 0`) equals
  `run_adaptive_simple_directional_target` bit for bit.

## What was read and computed before this text (disclosed)

- **Read:** the roadmap entries F-441, F-439, F-348; ADR 0341 in full; ADR 0267 in full (D1 to D7, the checks, amendments 1 and
  2, addendum 1 with D's histograms, R4); ADR 0247 D1 to D3 and D7 (the layer); the section list of ADR 0242 and its limit "no
  coarsening in the loop"; `crates/rcfd-fv/src/adaptive.rs` in full; `crates/rcfd-mesh/src/adapt/aniso.rs` in full;
  `transfer_field`, `lsq_gradient`, `Source` and `AdaptReport` in `crates/rcfd-mesh/src/adapt.rs`;
  `crates/rcfd-fv/tests/adapt_directional_f348.rs` (the layer set-up, the D runs, C4, C5b).
- **No run** of any code of this repository was made for this ADR.
- **Known before this text (disclosed, from ADR 0267's addendum):** D: 1547 cells, `E = 0.0029752`, x levels {0: 843, 1: 360,
  2: 227, 3: 109, 4: 8}, 704 leaves with `l_x > 0` (418 at `x < 1/8`, the farthest at `x = 0.484`, down to `y = 0.344`),
  converged solves of 89 to 115 iterations, 2.0 s. D960 (`N_T = 960`, no coarsening): 973 cells, `E = 0.0029759`, 247 leaves
  with `l_x > 0`. `E(D)` and `E(D960)` agree to four digits: the error is set by the `h_y = 1/512` band at the wall. C5b
  (16 x 16, `[3, 3]`, 3 cycles, 400): 402 cells, all solves converged, 2.5 s.
- **Arithmetic (disclosed, by hand):** (i) what D's x splits cost: a leaf at x level `j` is `2^-j` of the cell it would be
  without x splits, so D's y grading without x splits is `843 + 360/2 + 227/4 + 109/8 + 8/16 = 1093.9` cells; about 453 of
  D's 1547 cells (29 percent) are spent on x splits; (ii) the outcomes of F3 below (by hand from ADR 0341 D2 and ADR 0267 D5).

## Design (registered)

### D1. The coarsening criterion and its threshold (derivation)

Per cell the per-axis variations `E_{c,a}` of ADR 0267 D5 (`axis_variations`: `|dU/dx_a| h_{c,a}`, the first-order change of
`U` across the cell along axis `a`), `E_max = max_b E_{c,b}` over the active axes. A cell is marked to **coarsen along** the set

`C(c) = { a active : l_a(c) > 0 and E_max > 0 and E_{c,a} < phi E_max }`

(`directional_coarsen_axes`; `AxisMark::coarsen(C)`, or `KEEP` when `C` is empty). With `E_max = 0` there is no directional
information and nothing is coarsened (the refinement test of D5 splits every eligible axis then; the two do not conflict, since
`E_max = 0` cells are ranked last). The axis of `E_max` is never in `C` (`E_max < phi E_max` is false for `phi <= 1`), so the
dominant axis of a cell is never coarsened by this rule.

**Why a threshold below `theta`, and which.** A split along `a` halves `h_a`, so (gradient unchanged) it halves `E_a`; a merge
that lowers the level along `a` by `m` multiplies `E_a` by `2^m`. Write `rho_a = E_a / E_max`. Refinement chooses `a` when
`rho_a >= theta`; coarsening when `rho_a < phi`.

- *A just-split axis is not coarsened back.* If `a` was split, `E_a >= theta E_max` before; after the split
  `E'_a = E_a / 2 >= (theta / 2) E_max >= (theta / 2) E'_max` (a split only lowers variations, so `E'_max <= E_max`). It is
  coarsened only if `E'_a < phi E'_max`, which is impossible for `phi <= theta / 2`.
- *A just-merged axis is not split again.* If `a` is in `C`, `E_a < phi E_max`, and `a` is not the axis of `E_max`. After a merge
  of `m` levels along `a`, `E''_a = 2^m E_a < 2^m phi E_max`; while `2^m phi <= theta <= 1`, `E''_a` stays below `E_max`, so
  `E''_max = E_max` and `E''_a < theta E''_max`: `a` is not chosen by the refinement test. This needs `phi <= theta / 2^m`.
- The box rule merges smallest box first (ADR 0341 D2), so `m = 1` is the usual case; `m = 2` happens when the buddy along `a`
  is itself split along `a` (the three-leaf case of F3). **Registered: `phi = theta / 4`** (`= 1/8` with ADR 0267's
  `theta = 1/2`), the largest threshold for which both statements hold for merges of up to two levels. For one-level steps it
  leaves a band of one lattice step on each side: a just-split axis has `rho >= theta / 2 = 2 phi` and is coarsened again only
  if its ratio then falls by a further factor of two; a just-merged axis has `rho < 2 phi = theta / 2` and is split again only
  if its ratio rises by a further factor of two. So a cell is not refined and coarsened along the same axis in alternate cycles
  unless the gradient estimate itself changes by a factor of two between them. `phi` is accepted in `[0, theta / 4]` (located
  error outside); `phi = 0` asks for no coarsening.

Both statements are about a single cell with an unchanged gradient; they are checked exactly on a grid of variations (F2). With
a changing solution they are not guarantees; alternations in the layer runs are recorded.

### D2. The step of a cycle: coarsen, then refine on the coarsened lattice

Cycle `k` of the new loop, from the last solution on the lattice `L` with mesh `M`:

1. The velocity gradients `g_j` (`j = 0, 1, 2`, least squares with the boundary values of `U`, as
   `velocity_axis_variations`), the variations `E = axis_variations(M, g)`, and the coarsening marks `C` (D1, or any marks a
   caller gives).
2. **Coarsen** (only if some mark has a coarsen axis): `(L', T1, rep1) = L.adapt_with(C, balance, Coarsening::Box)`, `M' =
   L'.to_mesh()`. If `rep1.coarsened = 0` or no mark has a coarsen axis, `L' = L`, `M' = M` and nothing below is recomputed
   (so a run that coarsens nothing is the run without coarsening, D4).
3. **Gradients on the coarsened lattice** (`merged_gradients(M, T1, g)`): `Same(i)` keeps `g_j[i]`; `Merged(S)` gets the
   volume-weighted mean `sum_{i in S} V_i g_j[i] / sum V_i` (`V` the volumes of `M`); `Descendant(i)` (not made by a
   coarsen-only call) keeps `g_j[i]`. For a linear field every child has the same gradient and the mean is exact; in general
   it is the gradient of the volume-weighted mean of the children's linear reconstructions, the consistent first-order
   gradient of the merged cell without a new solve.
4. **Refine** on `L'`: `eta' = edge_length_weighted_indicator(M', g', axes, r)`, `E' = axis_variations(M', g', axes)`,
   `marks = directional_target_marks(eta', E', L'.levels(), max_levels, axes, n_k, theta)` with `n_k` the scheduled count of
   cycle `k`; `(L'', T2, rep2) = L'.adapt(marks, balance)`. The planned count starts from `L'`'s cell count, so the cells freed
   by the merges are spent again by the refinement of the same cycle and the target is met as in ADR 0267 (the count lands in
   `[n_k, n_k + 2^d - 2]` plus balance splits).
5. **Fields:** `U` (each component) and `p` go `M -> M'` by `transfer_field(M, M', T1, .., settings.transfer, axes)` and then
   `M' -> M''` by `transfer_field(M', M'', T2, ..)`. A coarsened cell's value is therefore the **volume-weighted mean** of the
   old cells it replaces (`Source::Merged`, ADR 0341 D5: `sum V q` conserved to round-off, linear fields exact, whatever the
   mode); the refinement then moves it with the mode of the settings (linear: the least-squares gradient of the coarsened mesh,
   exact for linear fields; conservative, ADR 0267 D4). Then `setup` on `M''`, the transferred cell values, and the solve, as
   in the existing loop.

A cell can be merged along x and its merged cell split along y in the same cycle (step 4 runs on `L'`); by D1 it is not split
again along x in that step (for merges of up to two levels and an unchanged mean gradient).

Why this order and not one `adapt_with` call with both kinds of marks: `adapt_with` lets refine win in a cell, so a wall cell
that needs a y split and has a needless x split could not lose the x split while it is ranked for refinement, which is where
D's x splits sit; and the target count would have to guess how many cells the merges remove. Two calls cost one more
`to_mesh` per cycle (cheap against a solve) and no solve.

### D3. API (new public items of `rcfd_fv::adaptive`)

- `directional_coarsen_axes(variation: Vec3, levels: [u8; 3], axes: [bool; 3], phi) -> Result<[bool; 3]>` (D1; `phi` finite
  and in `[0, 1/4]`, the variations finite and non-negative, else a located error);
- `directional_coarsen_marks(variations, levels, axes, phi) -> Result<Vec<AxisMark>>` (per cell; length mismatch a located
  error);
- `merged_gradients(old: &Mesh, tr: &Transfer, grads: &[Vec<Vec3>]) -> Result<Vec<Vec<Vec3>>>` (D2 step 3);
- `DirectionalStep { coarsened: Option<(AnisoMesh, Mesh, Transfer)>, coarsen_report: AdaptReport, next: AnisoMesh,
  transfer: Transfer, report: AdaptReport }` and `directional_coarsen_refine_step(amr, mesh, grads, coarsen_marks, n_target,
  r, theta, balance) -> Result<DirectionalStep>` (D2 steps 2 to 4, no solver involved);
- `DirectionalCoarseningRun { amr, solver, cycles: Vec<CycleRecord>, coarsened: Vec<usize>, leaves: Vec<Vec<AxisLeaf>> }`
  (per solve: the merges of the step that made its mesh, 0 for the first; the leaves of its lattice);
- `run_adaptive_simple_directional_coarsening(amr, setup, settings, n_target, r, theta, coarsen)` with
  `coarsen: FnMut(&[Vec3], &AnisoMesh, usize) -> Result<Vec<AxisMark>>` (the variations of the last solution, the current
  lattice, the cycle): the loop of D2 with any coarsening marks; `CycleRecord::refined` and `ignored_at_max_level` are those of
  the refinement call;
- `run_adaptive_simple_directional_coarsen_target(amr, setup, settings, n_target, r, theta, phi)`: the loop with
  `directional_coarsen_marks(.., phi)`; `phi > theta / 4` (or not finite, or negative) is a located error.

No existing item changes. `aniso.rs` is not expected to change (it is listed as editable only for a defect found on the way,
which would be reported).

### D4. Runs that do not ask for coarsening

The existing functions keep every code line (only the module text gains a paragraph), so their results are their results.
The new loop with `phi = 0` makes no coarsen mark, so step 2 is skipped, `L' = L`, `M' = M`, `g' = g`; step 4 computes
`edge_length_weighted_indicator(M, g, axes, r)` and `axis_variations(M, g, axes)` from the same least-squares gradients, which
are exactly what `velocity_edge_length_indicator` and `velocity_axis_variations` compute, and the transfers and the solve are
those of `run_adaptive_simple_directional`; so the run equals `run_adaptive_simple_directional_target` bit for bit (K1).

### D5. The layer runs (registered)

The suction layer of ADR 0247 D7 and ADR 0267 D7 unchanged (unit square, slab 0.1, `R = 80`, patches, conditions, initial
fields, solver settings, `run(20000, 1e-7)`, the error `E` of ADR 0247 D7), base 16 x 16, `max_level = [5, 5, 0]`, 6 cycles,
balance on, linear transfer, `r = 2`, `theta = 1/2`, set up by a copy of the helpers of `adapt_directional_f348.rs`.

- **D** (reference, through the unchanged function): ADR 0267's D again, `N_T = 1536`, no coarsening.
- **DC** (the acceptance run): `run_adaptive_simple_directional_coarsen_target` with **`phi = 1/8`** (D1) and **`N_T = 1280`**.
  Why 1280: by the disclosed arithmetic D's y grading costs about 1094 cells without its x splits; 1280 is above that (the
  layer gets at least D's y resolution if the x splits go), and leaves 267 cells below the limit 1547 for the last cycle's
  overshoot (D overshot by 11). The error is set by the level-5 band at the wall, which a 960-cell run already completed
  (D960); so the error clause is expected to hold with margin. Scheduled counts `round(256 * 5^(k/6))` = 336, 439, 572, 749,
  978, 1280.
- **D1280** (the control): D with `N_T = 1280`, no coarsening: the same budget and schedule without the coarsening step.
  Comparing DC with D1280 is what shows the coarsening at work; the acceptance's numbers alone could be met by a smaller target
  without coarsening (D960 has 247 x-split leaves).
- **DCall** (negative control): the coarsening loop with every cell marked `coarsen([true; 3])` in every cycle (no criterion):
  each cycle merges about one level per leaf and refines one level, so the wall band cannot climb to level 5; the error must
  miss.
- **DC1536** (recorded only): DC with `N_T = 1536`, D's budget.

Estimates: `N_DC` 1280 to 1300; `E(DC)` about 0.002975 (the wall band of level 5 complete); D1280 between D960 and D (about 450
leaves with `l_x > 0`, estimated by interpolation in the number of cells); DC: the x splits inside the layer have `E_x` of the
order of the discrete inlet adjustment (`10^-3`) against `E_y` of order `10^-1` at the wall, so they pass `rho < 1/8` and merge
where consent and balance allow, one level per cycle; x splits far from the wall, where both variations are at the level of the
solver tolerance, have no reliable ratio and merge only by chance. Expected `l_x > 0` leaves of DC: about half of D1280's or
fewer. Cost: each run about 2 s in release (D: 2.0 s); Dswap of ADR 0267 took 17 s, DCall may be of that order.

## Registered checks

New file `crates/rcfd-fv/tests/adapt_directional_coarsen_f441.rs`. F1 to F4, K1 and CL are ordinary tests; S0 and H1 to H6 and
NC-H1 share one computation (`OnceLock`: D, D1280, DC, DCall, DC1536), are `#[ignore = "heavy ..."]` and are run once in
release with `--include-ignored`; CL is the fast test of the same code path.

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| F1 (closed form, coarsening axes) | `directional_coarsen_axes`, active x and y, `phi = 1/8` | `(0.1, 1, 0)` levels `(1, 1, 0)`: x; `(0.125, 1, 0)`: none (strict); `(0.2, 1, 0)`: none; `(0.1, 1, 0)` with levels `(0, 1, 0)`: none; `(1, 0.1, 0)` levels `(1, 1, 0)`: y; `(0, 0, 0)`: none; `(0, 1, 0)` levels `(2, 2, 0)`: x; a variation on inactive z never selects z; `phi = 0`: none for `(0, 1, 0)`; `phi` = -0.1, 0.3, NaN and a NaN or negative variation: located error; `directional_coarsen_marks` gives `AxisMark::coarsen(C)` or `KEEP` per cell and a located error for mismatched lengths | exact |
| F2 (the hysteresis of D1, exact on a grid) | `theta = 1/2`, `phi = 1/8`, active x and y, `E_x, E_y` in `{0, 1/64, 2/64, .., 2}` (129 x 129 pairs). (a) levels `(1, 1)`, `max_level (3, 3)`: `S = directional_axes(E)`; halve `E_a` and raise `l_a` on `S`; `directional_coarsen_axes` of the result contains no axis of `S`. (b) levels `(2, 2)`: `C = directional_coarsen_axes(E)`; for `m = 1` and `m = 2` multiply `E_a` by `2^m` and lower `l_a` by `m` on `C`; `directional_axes` of the result contains no axis of `C` | no violation in any pair; non-vacuous: (a) has pairs with `S` non-empty and (b) pairs with `C` non-empty | exact (D1; scaling by powers of two is exact in floating point) |
| F3 (closed form, the step) | slab 4 x 2 on the unit square, `[2, 2, 0]`, balance off: every base cell split along x, then the right half of base cell 0 along x again (17 leaves: A, R1, R2 in base cell 0, two halves in each other); one gradient list `g_i = (0, 1 + i/16, 0)` (`i` the leaf index); coarsening marks `directional_coarsen_marks(axis_variations(..), .., 1/8)`; `directional_coarsen_refine_step(.., n_target = 10, r = 2, theta = 1/2, balance off)` | every leaf marked coarsen x only; stage 1: 8 cells (the base), `coarsened = 8`, base cell 0 `Merged([0, 1, 2])`; merged gradients exact: base cell 0 `(0, 1 + 0.75/16, 0)`, base cell `b >= 1` `(0, 1 + (2b + 1.5)/16, 0)`; stage 2: base cells 6 and 7 split along y only, 10 cells, `refined = 2`; the field `1.3 + 0.7 x - 0.4 y` through `T1` then `T2` (`Linear`) equal to the closed form at the final centres within `1e-12` of its range; `sum V q` of a random field through `T1` within `1e-13` relative | exact (by hand: A's only candidate is base cell 0, exactly tiled by A, R1, R2, all consenting along x; the pair rule would leave R1 + R2 as one leaf and 9 cells; the two largest means are those of base cells 7 and 6; `E_x = 0` so y only) |
| F4 (located errors and the no-op) | `run_adaptive_simple_directional_coarsen_target` refuses `phi = 0.2` with `theta = 1/2` (above `theta / 4`), `phi` NaN and negative, before any solve; `directional_coarsen_refine_step` with all-`KEEP` coarsening marks returns `coarsened = None` and the lattice, transfer and report of `amr.adapt(directional_target_marks(..), balance)` | errors name the argument; equal | exact |
| K1 (acceptance: "runs that do not ask for coarsening are bit for bit unchanged") | (a) the cavity set-up of ADR 0267 C4 (12 x 12, `[2, 2, 0]`, 2 cycles, `max_iter = 30`, balance, linear transfer): `run_adaptive_simple_directional_coarsen_target(.., 600, 2, 1/2, phi = 0)` against `run_adaptive_simple_directional_target(.., 600, 2, 1/2)`; (b) `git diff main -- crates/rcfd-fv/src/adaptive.rs crates/rcfd-mesh/src/adapt/aniso.rs` | (a) cycle records, `u`, `p`, `phi`, the leaves and the mesh points equal (`==`), `coarsened` all 0; (b) no line removed except module text, `aniso.rs` unchanged | bit for bit (D4) |
| K2 (acceptance, same clause) | existing tests run unedited: `rcfd-fv` `adapt_directional_f348` (C1 to C4, C5b, and the heavy S0, H1 to H4, NC-H1 with `--ignored`, skipping only the 5-minute record `c5_..._known_miss`), `adapt_criteria_f343` and `adapt_cavity_f102` (ordinary tests), `rcfd-mesh` `adapt_aniso_f348` and `adapt_aniso_box_f439` | all pass; the heavy F-348 layer tests print `N_D = 1547` | no executed line of those paths changes |
| CL (fast test of the heavy path) | the layer on the 16 x 16 base, `[3, 3, 0]`, 3 cycles, `N_T = 400` (scheduled 297, 345, 400), `r = 2`, `theta = 1/2`, `phi = 1/8`, `run(20000, 1e-7)` | every solve converged; final `validate` Ok; per-axis jump `<= 1`; `400 <= N <= 440`; cells strictly increase; `sum l_y > sum l_x`; the largest y level 3 | C5b's run (402 cells, all converged) with merges added; merges per cycle recorded |
| S0 | the copied set-up reproduces ADR 0267's D | `N_D = 1547`, `abs(E(D) - 0.0029752) <= 1e-7`, 704 leaves with `l_x > 0` | the same code path, unchanged |
| H1 (acceptance: "reaches the error E(D) = 0.00298") | `E(DC)` | `<= 0.00298` | about 0.002975 |
| H2 (acceptance: "with fewer than its 1547 cells") | `N_DC` | `< 1547` | 1280 to 1300 |
| H3 (acceptance: "fewer leaves split along x than its 704") | leaves of DC with `l_x > 0` | `< 704` | half of D1280's or fewer |
| H4 | DC sound and coarsening exercised | every solve converged; final `validate` Ok; per-axis jump `<= 1`; cells strictly increase; leaves at y level 5 present; total merges over the cycles `>= 1` | as D (ADR 0267 H4) |
| H5 (the coarsening does it) | leaves with `l_x > 0`: DC against D1280 (same target and schedule) | `x(DC) < x(D1280)` | about half |
| H6 | the target lands | `abs(N_DC - 1280) <= 128` | 1280 to 1300 |
| NC-H1 | the criterion matters | `E(DCall) > 0.00298` | the wall band stays at low y levels: `E` above 0.03 |

Recorded, not counted: every `E`, count and cycle record; merges per cycle; the x and y histograms; the leaves with `l_x > 0`
and where they are; the alternations of DC (leaves of cycle `k + 1` that were leaves at cycle `k - 1` and not at cycle `k`);
DC1536 against D (same budget); times.

### Negative controls

NC-H1 above (all-axes coarsening without the criterion must lose the layer). H5 is the control of the acceptance numbers: if
DC's x splits are not fewer than those of the same-budget run without coarsening, the coarsening did not cause them. F1's
`(0.125, 1)` and level-0 cases and F4's no-op guard against a rule that coarsens too much.

### Mutations (scratch copy, not committed)

Each alone on a `git archive` copy of the final commit in `w49i-work/f441-scratch`, by an exact text replacement (each pattern
found once), the non-ignored tests of the new file run once in release in their own target directory; the heavy tests are not
run under mutations.

| Id | Mutation | Must |
| --- | --- | --- |
| MF1 | `E_a < phi E_max` becomes `<=` | F1 fails |
| MF2 | the `l_a > 0` condition dropped from `directional_coarsen_axes` | F1 fails |
| MF3 | `merged_gradients` gives a merged cell the gradient of its first source | F3 fails |
| MF4 | the step coarsens with `Coarsening::Pair` | F3 fails |
| MF5 | the `phi <= theta / 4` check dropped | F4 fails |
| MF6 | the coarsening loop moves the fields of the refinement call by injection whatever the settings | K1 fails |

### Predictions (mine)

| Id | Prediction |
| --- | --- |
| P1 | F1 to F4, K1, K2 and CL met at the first run |
| P2 | `N_DC` 1280 to 1300, `E(DC) <= E(D)` to four digits, H1 to H6 met |
| P3 | `x(DC) <= x(D1280) / 2`; most remaining x splits in DC lie at `y < 0.9` (outside the layer) or at `x < 1/8` |
| P4 | `E(DCall)` above 0.03 |
| P5 | fewer than 2 percent of DC's final leaves are alternations |

## Decision rule (registered)

- **done**: F1 to F4, K1, K2, CL, S0, H1 to H6 met and NC-H1 holding. Mutations not caught are reported as gaps of the checks.
- **partial**: K1, K2 and F3 met but one of H1, H2, H3 missed, or a supporting check (F1, F2, F4, CL, S0, H4, H5, H6) missed,
  each miss a `*_known_miss` with its number and a diagnosis; or NC-H1 failing.
- **todo**: K1 or K2 fails (a default moved) or F3 fails (the step is wrong).

## Limits of this item

- One layer flow, one base, one target, one `theta`, one `phi`, fixed here and not tuned after the run. The criterion is
  relative to the same cell's largest variation: a cell where every variation is negligible (the far field) has no reliable
  ratio, so this rule does not remove x splits there except by chance (Q1 option (b) would).
- The hysteresis of D1 is derived for an unchanged gradient and merges of up to two levels along an axis; a three-level merge
  can be re-split by one level in the same step. With balance the return of coarsening to the base is not derived (ADR 0341 D4);
  the loop does not need it.
- Steady SIMPLE only; box lattices only; no case key (`[mesh.adapt]` is not touched; ADR 0267's F-438 would carry the keys).
- The merged cell's gradient for the refinement ranking is the volume-weighted mean of its children's, not a new least-squares
  gradient on the coarsened mesh (that would need the boundary values of a field on that mesh, that is a `setup` call and a
  solver; D2 step 3 avoids both).

## Open questions and choices for the maintainer

- **Q1. "below a registered fraction of the largest".** Options: (a) the largest of the cell's own per-axis variations (D1,
  the quantity of the refinement test, which gives the hysteresis derivation); (b) the largest variation over all cells (would
  also coarsen the far field, but its relation to the refinement ranking is not a fixed ratio, so no hysteresis bound follows);
  (c) both. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a).
- **Q2. "reaches the error E(D) = 0.00298".** Options: (a) `E(DC) <= 0.00298`, the number as quoted; (b) `E(DC) <= E(D)` of the
  same run (0.0029752; D960 missed that by 7e-7 with the same wall band, so it would decide on round-off-level differences).
  Taken on the same standing instruction: (a); `E(DC) / E(D)` is recorded.
- **Q3. The target of DC.** Options: (a) 1280 (D5); (b) 1536, D's budget (the cell clause would then depend on an overshoot of
  11 against a margin of 11). Taken on the same standing instruction: (a), with D1280 as the same-budget control and DC1536
  recorded.
- **Q4. Coarsen then refine in one cycle, or one `adapt_with` call with both kinds of marks.** Taken on the same standing
  instruction: two calls (D2 gives why).
