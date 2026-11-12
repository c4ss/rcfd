# ADR 0247: F-343 registration: target-count, edge-length-weighted and register-based adaptation criteria, and a case where adaptation pays

Status: **REGISTERED 2026-11-12**, written before any code of F-343 and before any run of its checks. Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0242](0242-f102-registration-adapted-cavity-and-solver-loop-hook.md) (F-102: `rcfd_fv::adaptive`,
`run_adaptive_simple`, the Re 100 cavity acceptance, recommendations R2 and R3, the surviving mutation MU6) and
[ADR 0054](0054-wave28c-adaptation-turbo-post.md) (wave 28C: `rcfd_mesh::adapt`, indicators, marks, hanging-node refinement,
transfer; D7 lists the criteria not built). Roadmap item F-343 (depends on F-102).

Number: the lead gave 0247 to F-343. Branch `f343-adapt-criteria` from main 37f7c43c, worktree `w49i-work/f343`.

House rules (as ADR 0242):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written (and committed) before the corrected run, and the old one
  stays as an ignored record;
- each registered run is made once, and nothing is re-run to pick a result;
- defaults stay bit-identical: the existing indicators, `fixed_fraction_marks`, `run_adaptive_simple` and
  `run_adaptive_simple_with` with their present arguments give exactly what they gave; the new criteria are opt-in (new public
  items only); no existing test is edited (`adapt_cavity_f102.rs`, `amr_solve.rs`, `amr_diag.rs`, the `rcfd-mesh` `adapt.rs`
  tests stay as they are);
- `crates/rcfd-api/src/sim.rs` and `schema.rs` are not touched; neither `crates/rcfd-api/tests/baseline_hashes.rs` nor any hash
  table nor any golden is edited or re-recorded;
- no new third-party crate, `Cargo.lock` unchanged, no `unsafe`, the crate graph unchanged (only `rcfd-fv` is touched:
  `src/adaptive.rs` and a new test file), unsigned commits, no trailers, no push.

## What F-343 asks

The roadmap entry (title): "Further adaptation criteria (target cell count, edge-length weighting, anisotropic, register-based)
and a case where adaptation pays". Acceptance (quoted, binding): "Each criterion has a closed-form marking check, and a
target-count run lands within 10 percent of its target on the ADR 0242 cavity; on a case with a localised feature the adapted
mesh reaches the fine solution's error with at most half the cells of the uniform mesh that reaches it."

## What was read and computed before this text (disclosed)

- **Read:** the roadmap entries F-343 and F-102 (title, deps, acceptance, note); ADR 0242 in full; ADR 0054 in full (D1 to D7,
  A1 to A6, addenda 1 to 3); `crates/rcfd-mesh/src/adapt.rs` in full; `crates/rcfd-fv/src/adaptive.rs` in full;
  `crates/rcfd-fv/tests/adapt_cavity_f102.rs` in full; in `crates/rcfd-fv/src/bc.rs` `FaceCoeffs`, `Bc`, `Profile`,
  `FixedValue`, `ZeroGradient`; in `crates/rcfd-fv/src/pressure.rs` `pressure_bc` (a face whose velocity is fixed gets a
  zero-gradient pressure, every other face the pressure's own condition); in `simple.rs` the module text, `SimpleSettings`,
  `SimpleSettings::new` and the public fields of `SimpleSolver`; in `convection.rs` the `LinearUpwind` face value
  (`phi_U + g . (x_f - x_U)`, unlimited); the `PatchKind` enum and `Mesh::patches`; the `PerFace` set-up of `amr_solve.rs`.
- **Searched:** no solver uses `PatchKind` beyond `Empty` and `Cyclic` for the choice of the default condition; nothing outside
  `adaptive.rs` and `adapt_cavity_f102.rs` calls `rcfd_fv::adaptive`.
- **No run** of anything was made for this ADR (no calibration run). Costs are taken from ADR 0242's addendum: the 96 x 96 cavity
  took 853 SIMPLEC iterations and 77.9 s in release (about 91 ms per iteration at 9216 cells).
- **Arithmetic on paper (no run of anything below):** the cell counts of the target-count runs, the truncation error of the
  scheme on the suction layer and the cell budget of the adapted layer mesh, all given with the design below.

No code of this ADR existed when this text was written, and no check below was run.

## What exists and what is missing

Exists: `AmrMesh` (one refinement level per leaf, every refined axis split at once), `adapt` with balance and the `Transfer` map,
`transfer_field`, the indicators of ADR 0054 D3, `MarkSpec`, `combine_marks`, `region_indicator`; `rcfd_fv::adaptive` with
`velocity_gradient_indicator` (`|grad U|_F`, times `h` = `cell_lengths` when `scale_by_length`), `fixed_fraction_marks` and the
loop `run_adaptive_simple` / `run_adaptive_simple_with` whose marks are always fixed-fraction.

Missing (this item): a target-cell-count criterion; an edge-length-weighted indicator; an anisotropic (directional) criterion;
register-based marks; a way to give the loop any marks (today only fixed fraction); a case where adaptation pays (ADR 0242 R2:
on the Re 100 cavity a uniform 48 x 48 mesh is nearly as good per cell); a check that the length scaling of the indicator is
applied (ADR 0242 MU6 survived every test).

## Design (registered)

All new items are public functions or types in `crates/rcfd-fv/src/adaptive.rs`. The existing functions keep their signatures and
results; the only internal change is that `run_adaptive_simple_with` becomes a call of the new general loop with a marker that
does exactly what its body did (indicator, the length check with the same error text, `fixed_fraction_marks`), so the sequence
of floating-point operations is the same and the results are bit-identical (checked by K5 and by the unedited ADR 0242 tests H1
to H4).

### D1. The general loop: `run_adaptive_simple_marked(amr, setup, settings, marker)`

`marker: FnMut(&SimpleSolver, &AmrMesh, usize) -> Result<Vec<Mark>>` gets the last solution, the current lattice and the cycle
number `k >= 1` and returns one mark per cell; everything else (adapt with `settings.balance`, transfer, `setup`, re-solve,
records) is the loop of ADR 0242. `settings.refine_fraction` is not used by it. An error of the marker is returned with
`adaptive cycle k:` in front.

### D2. Target cell count

Source: first principles. A split of a leaf replaces one cell by `c = 2^d` cells (`d` the number of refined axes; 4 for the
slab, 8 in 3-D), so it adds `c - 1` cells. To reach `N_T` from `N` cells by splits only, the smallest number of splits is
`k = ceil((N_T - N) / (c - 1))` (zero when `N_T <= N`); the result has `N + k (c - 1)` cells, in `[N_T, N_T + c - 2]`, before
the balance closure. Which cells: the `k` largest indicators among the cells below the maximum level, ties to the lower index
(the ranking of `fixed_fraction_marks`; this is the "fixed number" strategy of the adaptive finite-element literature, e.g.
Bangerth and Rannacher 2003, cited from memory: it is the fixed-fraction strategy with the count set from the target instead of
a fraction). If fewer than `k` cells are eligible, every eligible cell is marked (the target cannot be met at `max_level`; the
caller sees it in the count).

- `children_per_split(axes) -> usize` = `2^(number of true axes)`.
- `target_count_marks(indicator, levels, max_level, n_target, children) -> Result<Vec<Mark>>`: as above; `children >= 2`, the
  indicator finite and of the length of `levels`, otherwise a located error naming the argument.
- `target_schedule(n0, n_target, cycles, k) -> usize`: the intermediate target of cycle `k` of a run of `cycles` steps,
  `round(n0 (n_target / n0)^(k / cycles))` for `0 < k < cycles`, `n0` for `k = 0`, `n_target` for `k >= cycles`. Geometric
  steps give the same growth factor per cycle (each cycle's indicator then acts on a mesh of comparable change). Because each
  cycle aims at its scheduled count from the actual current count, the balance overshoot of earlier cycles is absorbed; only the
  last cycle's balance overshoot remains.
- `run_adaptive_simple_target(amr, setup, settings, n_target, indicator)`: the loop with marks `target_count_marks(indicator,
  levels, max_level, target_schedule(N_0, n_target, settings.cycles, k), children_per_split(axes))`, `N_0` the cells of the
  given lattice.

### D3. Edge-length weighting

Source: first principles (Taylor expansion) and the equidistribution principle (e.g. de Boor 1973 for the equidistribution of a
monitor function in mesh selection; cited from memory, the formula used is derived here). The change of a smooth field `U`
across a box cell of edge lengths `h_a` along axis `a` is to first order `dU_a = (dU/dx_a) h_a`; its root-sum-square over the
active axes,

`E_c = ( sum_a sum_k (d u_k / d x_a)^2 h_{c,a}^2 )^(1/2)`,

is the estimated variation of `U` inside the cell measured along its edges. On a square cell (`h_a = h`) it is `|grad U|_F h`,
the scaled indicator of ADR 0242; on a stretched cell it weights each derivative by the edge along which it acts (the
geometric-mean length of `cell_lengths` does not). The weighted indicator is

`eta_c = E_c (prod_a h_{c,a})^((r - 1) / d)`,

with a weighting exponent `r >= 0`: `r = 1` is `E_c`; `r = 0` removes one power of length (`|grad U|`-like); `r = 1 + d/2` is
`E_c sqrt(A_c)` with `A_c` the cell measure in the active axes, the cell's contribution to the `L2` norm of the variation
(`||h grad U||_{L2}^2 = sum_c E_c^2 A_c`), so equal `eta` equidistributes that norm. A split halves every `h_a` and multiplies
`eta` of a cell with the same gradient by `2^(-r)`.

- `cell_extents(mesh) -> Vec<Vec3>`: per cell the extent of its vertices along each axis (exact edge lengths for the boxes of
  `AmrMesh`).
- `edge_length_weighted_indicator(mesh, grads, axes, r) -> Result<Vec<f64>>`: `grads` one gradient list per field component
  (each of `n_cells` entries); located errors for a wrong length, a non-finite gradient, `r` negative or non-finite, no active
  axis.
- `velocity_edge_length_indicator(solver, axes, r) -> Result<Vec<f64>>`: the gradients of the three components of `U` from
  `rcfd_fv::gradient::least_squares` with the boundary values (as `velocity_gradient_indicator`), then the function above.

### D4. Register-based marks

Source: first principles (set algebra on cell sets). A register is a set of cells, defined by a rule, that drives or confines the
adaptation. `Register` holds one `bool` per cell:

- constructors: `Register::from_mask(Vec<bool>)`; `Register::from_indicator(&[f64])` (cells with value `>= 0.5`; the 0/1 output
  of `rcfd_mesh::adapt::region_indicator` for box, sphere, slab and surface distance); `Register::in_range(field, lo, hi)`
  (cells with `lo <= field <= hi`; finite `lo <= hi` and a finite field, else a located error); `Register::levels_in(levels, lo,
  hi)`;
- algebra: `and`, `or` (located error for different lengths), `not`; `grow(mesh, layers)` adds `layers` times every face
  neighbour of a member (dilation by face adjacency, a buffer around a feature);
- queries: `len`, `count`, `contains`, `mask`;
- `register_marks(register, levels, max_level) -> Result<Vec<Mark>>`: `Refine` for members below `max_level`, `Keep`
  otherwise (refine a region);
- `restrict_marks(marks, register) -> Result<Vec<Mark>>`: every mark of a non-member becomes `Keep`, members keep their mark
  (confine any other criterion to the region).

### D5. Anisotropic: not built (registered as not delivered)

On a hanging-node quadtree or octree, anisotropic adaptation means a directional split (a cell split along one axis only). The
present lattice cannot represent it: a `Leaf` is `(level: u8, ijk)` with one level for every axis, `children` splits every
refined axis at once, and `covering`, the balance closure, `to_mesh`'s neighbour search and the `Transfer` map all assume that a
leaf and its neighbour differ by a whole level on every axis. A directional split needs per-axis levels in the leaf key (a change
of the public `Leaf` type, or a second lattice type), a new neighbour and face search (the face loops of ADR 0054 D1 would still
close cells: every face of an axis-aligned box tiling, with all lattice vertices on its boundary edges inserted, is shared by
exactly two cells), a per-axis balance and a new transfer. That is a redesign of `rcfd_mesh::adapt`, outside this item's file
and scope. As instructed, it is not faked (no directional mark that nothing consumes): the item is at most **partial**, and the
follow-up item below carries it. The per-axis variations `(d U / d x_a) h_a` of D3 are the quantities a directional criterion
would compare.

### D6. The target-count cavity runs (registered)

The cavity of ADR 0242 unchanged (unit square, slab 0.1, Re 100, SIMPLEC, linear upwind, one non-orthogonal corrector with the
compensation, `run(20000, 1e-7)`, base 24 x 24, `max_level = 2`, balance on, linear transfer), the indicator
`velocity_gradient_indicator` with `scale_by_length = true` (the ADR 0242 indicator), `cycles = 2`, through
`run_adaptive_simple_target`:

- **T1**: `N_T = 2304` (the cells of the uniform 48 x 48 of ADR 0242's R2). Paper: cycle 1 target
  `round(576 * 4^(1/2)) = 1152`, 192 splits, `1152 + 3 b_1` cells (`b_1` balance splits); cycle 2 `ceil((2304 - N_1)/3)` splits;
  final `2304 + {0, 1, 2} + 3 b_2`. ADR 0242's A had 0 and 2 balance splits at 231 and 508 marks, so `N` about 2304 to 2320,
  0 to 1 percent above the target.
- **T2**: `N_T = 4608` (half of the 96 x 96). Cycle 1 `round(576 * 8^(1/2)) = 1629`, 351 splits; cycle 2 about 993 splits
  among about 1629 eligible cells; final about 4608 to 4640.
- **F** the uniform 96 x 96 and **U48** the uniform 48 x 48, for the recorded centreline differences only (ADR 0242's `D`).

### D7. The localised-feature case: the suction layer (registered)

Choice, from reasoning: plane Couette flow with uniform wall-normal through-flow (blowing through the lower wall, suction
through the moving upper wall). It is a steady exact solution of the incompressible Navier-Stokes equations with a thin
exponential layer at the suction wall, so the reference is a closed form, not a number from the literature. Derivation (first
principles; the flow is the classical Couette flow with transpiration, e.g. in Schlichting's "Boundary-Layer Theory" and White's
"Viscous Fluid Flow", cited from memory): take `v = V` constant and `u = u(y)`; continuity holds; the y-momentum equation gives
`dp/dy = 0`; with `dp/dx = 0` the x-momentum equation is `V u' = nu u''` with `u(0) = 0`, `u(H) = U`, so

`u*(y) = U (exp(R y / H) - 1) / (exp(R) - 1) = U (exp(R (y/H - 1)) - exp(-R)) / (1 - exp(-R))`, `v* = V`, `p* = const`,

`R = V H / nu`. The layer has the e-folding thickness `H / R` at `y = H`; below it `u*` is exponentially small. The layer runs along
the whole upper wall, so it is a thin (one-dimensional) structure rather than a point feature; it is "localised" in the sense
that only a fraction of order `1/R` of the domain needs fine cells.

Set-up: unit square, slab of thickness 0.1, `U = 1`, `V = 1`, `H = 1`, **`R = 80`** (`nu = 0.0125`). Patches: `inlet` (xmin,
Inlet: `U = (u*(y_f), 1, 0)` per face, `FixedValue(Profile::PerFace)`), `outlet` (xmax, Outlet: `U` zero gradient, `p = 0`
fixed), `bottom` (ymin, Inlet: `U = (0, 1, 0)`), `top` (ymax, Wall: `U = (1, 1, 0)`, the moving porous wall), `frontAndBack`
(Empty). Pressure zero-gradient elsewhere (by `pressure_bc` on fixed-velocity faces); the fixed outlet pressure sets the level.
The exact solution satisfies every one of these conditions (zero `x`-gradient at the outlet, `p* = 0`). Initial fields
`U = (0, 1, 0)`, `p = 0`. Solver: `SimpleSettings::new(0.0125, Simplec)` with `LinearUpwind`, `non_orth_correctors = 1`,
`simplec_non_orth_compensation = true` (as the cavity), `run(20000, 1e-7)`.

Error of a mesh `M`: `E(M) = max over cells c of max(|u_c - u*(y_c)|, |v_c - 1|)` (cell-centre values against the closed form at
the cell centre; units of `U`).

Why `R = 80` and the meshes (paper):

- Truncation of the scheme on the layer (uniform `h`, derived): the linear-upwind face value with the central (least-squares)
  cell gradient gives for the convective derivative `(1/h)(phi_i - phi_{i-1} + (phi_{i+1} - phi_i - phi_{i-1} + phi_{i-2})/4) =
  phi' - (h^2/12) phi''' + O(h^3)`; central diffusion `phi'' + (h^2/12) phi''''`. On `phi = exp(R y)` (with `nu R = V`) the
  truncation is `-(h^2/6) V R^3 phi`. The error equation `V e' - nu e'' = tau` with a source proportional to `exp(R (y - 1))` is
  resonant: `L(y e^{R y}) = -V e^{R y}`, so `e ~ (h^2 R^2 / 6) s exp(-R s)` with `s = 1 - y`, maximum at `s = 1/R`:
  `E ~ (h R)^2 / (6 e) = 0.061 (h R)^2`.
- Fine uniform mesh **F** = 128 x 128 (16384 cells): `h R = 0.625`, `E(F)` about 0.024 (estimate band 0.01 to 0.05).
- **U90** = 90 x 90 (8100 cells, at most half of `N_F`, `8100 <= 8192`): `h R = 0.889`, `E` about 0.048, about 2 `E(F)`.
- Recorded uniform meshes **U32**, **U64** (coarse references; `E(U32)` of order 0.3 or above).
- **A** = adapted: base 16 x 16 (256 cells), `max_level = 4` (finest cell `1/256`, half of F's), `cycles = 5` (four to reach the
  finest level, one more for the bands to settle), balance on, linear transfer, target count **`N_T = 3840`**, indicator
  `velocity_edge_length_indicator` with **`r = 2`** (`= 1 + d/2`, D3). Why `r = 2`: on the layer `|u'| = R exp(-R s)` and the
  ranking `eta > t` puts the boundary between refinement levels `l` and `l + 1` at `s_l` with `R h_l^r exp(-R s_l) = t`, so
  successive boundaries are `r ln 2 / R` apart; the error weight of the layer (above) needs the level-boundaries about
  `1.4 / R` or more apart, which `r = 2` gives (`r = 1`: `0.69 / R`). Cell budget for the converged ranking: a level-4 band to
  `s = a/R` costs `819 a` cells (`a/80 * 256` rows of 256), the level-3, -2, -1 bands about 284, 71 and 32 cells and the rest of
  the base about 230: `N` about `819 a + 617`, so `N_T = 3840` gives `a` about 3.9. Error of that mesh relative to F, weighting the
  squared cell size by the weight `R^2 s exp(-R s)` of the resonant error (cumulative `W(s) = (1 + R s) exp(-R s)`): about 0.55 to
  0.6 `E(F)` (the level-0 cells beyond `s = 8.4/R` contribute about a quarter of it).
- Why `N_T = 3840` (not nearer 8192): the strict reading of the acceptance (check L5) compares with the coarsest uniform mesh that
  reaches `E(F)`; if U90 does not reach it (NC-L1), that mesh has at least `91^2 = 8281` cells and half of it is 4140; 3840 leaves
  300 cells (100 splits) for the last cycle's balance overshoot.
- **Ainv** = A with the inverted indicator `max(eta) - eta` (refines the quietest cells): must fail.
- **A1** = A with `r = 1` (recorded only).
- Cost (paper): F about 16384 cells at about 200 ms per iteration; through-flow converges faster than the cavity (no
  recirculation), a few hundred iterations, 1 to 3 minutes; U90 under a minute; the adapted runs a few seconds each.

## Registered checks

New test file `crates/rcfd-fv/tests/adapt_criteria_f343.rs`. The K checks are ordinary tests. The cavity checks (T) share one
computation (`OnceLock`: F, U48, T1, T2) and the layer checks (L) another (`OnceLock`: U32, U64, U90, F, A, Ainv, A1); both are
`#[ignore = "heavy ..."]` and run once, in release, with `--include-ignored`.

| Id | Check | Limit | Estimate behind the limit |
| --- | --- | --- | --- |
| T1 (acceptance: "target-count run lands within 10 percent of its target on the ADR 0242 cavity") | `N(T1)` against 2304 | `|N - 2304| <= 230.4` | 2304 to 2320 |
| T2 | `N(T2)` against 4608 | `|N - 4608| <= 460.8` | 4608 to 4640 |
| T3 | T1 and T2 sound | every solve converged; final `Mesh::validate` Ok; `max_face_level_jump <= 1`; cells strictly increase over the cycles | met, as ADR 0242's A |
| K1 (closed-form marking check, target count) | `target_count_marks` on 100 cells with indicator `(37 i) mod 100`, all level 0, `children = 4` | target 160: exactly the 20 largest marked; 161: the 21 largest; 100 and 50: none; with cells 0..9 at `max_level`, target 400 (`k = 100 > 90` eligible): exactly cells 10..99; on a 10 x 10 slab `AmrMesh` (balance off) the adapted count for targets 160, 161, 162, 163 is 160, 163, 163, 163; on a 4 x 4 x 4 box (`children = 8`), target 100: `k = 6`, 106 cells; `target_schedule(576, 2304, 2, k)` = 576, 1152, 2304 for `k` = 0, 1, 2 and `target_schedule(256, 3840, 5, k)` = `round(256 * 15^(k/5))` for `k = 1..4`; located errors for `children < 2`, a NaN indicator, mismatched lengths | exact (integers) |
| K2 (closed-form marking check, edge-length weighting) | graded slab `AmrMesh`, x nodes `0, 0.1, 0.3, 0.6, 1`, y nodes `0, 0.2, 0.25, 0.5, 1`, cell 5 refined once (balance off); `cell_extents` | every extent equals the node spacing (level 0) or half of it (level 1) | `1e-14` absolute |
| K2 (cont.) | `edge_length_weighted_indicator` with the constant gradients `grad u = (2, -3, 0)`, `grad v = (-1, 4, 0)`, `grad w = 0` | `eta_c = sqrt(5 h_x^2 + 25 h_y^2) (h_x h_y)^((r - 1)/2)` for `r = 0, 1, 2, 3.5` | relative `1e-13` |
| K2 (cont.) | `velocity_edge_length_indicator` on the same mesh with the linear `U = (1 + 2x - 3y, 0.5 - x + 4y, 0)` in the cells and on every non-empty boundary face (`PerFace`) | the closed form above (least squares exact for a linear field) | relative `1e-9` |
| K2 (cont.) | marks | on a uniform 8 x 8 slab with cell 0 refined (67 cells, balance off) and the linear `U` above (constant gradient): with `r = 1` the 16 largest of `velocity_edge_length_indicator` (`target_count_marks`, target `67 + 48`) are all level-0 cells (the four level-1 cells have exactly half the indicator); with `r = 1` on the uniform 8 x 8 it equals `velocity_gradient_indicator(scale_by_length = true)` | the level-0 / level-1 ratio 2 to `1e-9`; equality `1e-12` relative |
| K3 (closed-form marking check, register) | 10 x 10 uniform slab, `phi = x + 10 y` at the cell centres, `Register::in_range(phi, 2, 3)` | exactly `{(i >= 5, j = 1)} U {(i <= 4, j = 2)}` (10 cells) | exact |
| K3 (cont.) | algebra and growth | `and`, `or`, `not` of the range register and the box register `x <= 0.5` (from `region_indicator`) equal the set operations on the index sets; `grow(1)` of cell `(5, 5)` is the five-cell cross, `grow(2)` the 13 cells with `|di| + |dj| <= 2` | exact |
| K3 (cont.) | `register_marks`, `restrict_marks` | `Refine` exactly on members below `max_level`; restriction keeps exactly the members' marks and gives `Keep` elsewhere; located errors for mismatched lengths, `lo > hi`, NaN | exact |
| K3b | the register in the loop | `run_adaptive_simple_marked`, cavity set-up on a 12 x 12 base, `max_level = 2`, `cycles = 2`, `max_iter = 20`, balance off, marker `restrict_marks(fixed_fraction_marks(velocity indicator, 0.4), register of cells with centre y >= 0.5)` | every leaf with level > 0 has its centre at `y > 0.5`; more than 144 cells |
| K0 (would have caught ADR 0242 MU6) | `velocity_gradient_indicator` on a uniform 8 x 8 slab with cell 0 refined, linear `U` with exact boundary values | `scale_by_length = true`: `eta_c = |G|_F h_c` (`|G|_F = sqrt(30)`, `h_c` the cell side), relative `1e-9`, and the level-0 / level-1 ratio 2 to `1e-9`; `false`: `|G|_F` in every cell | as stated |
| K0b | the scaling reaches the loop | base 12 x 12, `max_level = 2`, `cycles = 2`, `max_iter = 0`, the setup's first call gives `U = (x^2 + y^2, x y, 0)`: `run_adaptive_simple` (scale on) and `run_adaptive_simple_with` with the explicit scaled indicator give the same leaves; the same with scale off gives different leaves | exact (leaf sets) |
| K5 (defaults bit-identical) | `run_adaptive_simple_marked` with the marker "velocity indicator then `fixed_fraction_marks(0.4)`" against `run_adaptive_simple` | base 12 x 12, `cycles = 2`, `max_iter = 30`: leaves, `u`, `p`, `phi` and the records equal (`==`) | bit for bit |
| K6 | the ADR 0242 tests | `adapt_cavity_f102.rs` H1 to H4 pass unedited (the heavy cavity checks are not re-run; their path changes only by the refactor that K5 and H1 to H4 cover) | pass |
| L1 (acceptance: "reaches the fine solution's error") | `E(A)` against `E(F)` | `E(A) <= E(F)` | `E(A)/E(F)` about 0.55 to 0.6 |
| L2 (acceptance: "at most half the cells") | `N_A` against `N_F = 16384` | `N_A <= 8192` | 3840 to 3940 |
| L3 | the target-count run lands | `|N_A - 3840| <= 384` | 3840 to 3940 |
| L4 | A sound | every solve of A, F and U90 converged; final `Mesh::validate` Ok; `max_face_level_jump <= 1`; cells at level 4 present; cells strictly increase | met |
| L5 (strict reading: half of the coarsest uniform mesh that reaches `E(F)`) | with NC-L1 holding, that mesh has at least `91^2 = 8281` cells | `N_A <= 4140` and L1 | 3840 to 3940 |
| NC-L1 | the uniform mesh with at most half of F's cells does not reach `E(F)` | `E(U90) > E(F)` | `E(U90)/E(F)` about 2 |
| NC-L2 | the criterion matters | `E(Ainv) > E(F)` | `E(Ainv)` of the order of `E(U16)`, above 0.2 |
| NC-L3 | the uniform sequence is monotone (the reading of "the uniform mesh that reaches it" needs it) | `E(U32) > E(U64) > E(U90) > E(F)` | orders near 2 |

Recorded, not counted: `D(T1)`, `D(T2)`, `D(U48)` (the ADR 0242 centreline difference to F); every `E` and cell count; the
observed orders of the uniform layer sequence; `E(A1)`, `N(A1)`; the cycle records (cells, iterations, splits, balance count) and
level histograms of T1, T2, A, Ainv, A1; every solve's iteration count and time.

### Negative controls

NC-L1, NC-L2 and NC-L3 above. If NC-L1 fails, the comparison with F does not show that uniform refinement needs more cells (L5
is then void and the item is not done whatever L1 gives); if NC-L2 fails, `E(F)` is too loose for the set-up to show anything;
if NC-L3 fails, "the uniform mesh that reaches it" is not determined by the sequence. In each case the set-up is defective.

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final commit (in `w49i-work/f343-scratch`) by an exact text replacement
(each pattern found once), the K tests run once in release in their own target directory; the heavy tests are not run under
mutations.

| Id | Mutation | Must |
| --- | --- | --- |
| MU1 | `target_count_marks` divides by `children` instead of `children - 1` | K1 fails |
| MU2 | `target_schedule` linear in `k` instead of geometric | K1 fails |
| MU3 | `edge_length_weighted_indicator` weights the derivative along x by `h_y` and along y by `h_x` | K2 fails |
| MU4 | the exponent `r` ignored (treated as 1) | K2 fails |
| MU5 | `Register::and` computes `or` | K3 fails |
| MU6 (ADR 0242's) | `velocity_gradient_indicator` never scales (`h = 1`) | K0 fails |
| MU6b | `run_adaptive_simple` passes `scale_by_length = false` | K0b fails |
| MU7 | `restrict_marks` returns the marks unchanged | K3 and K3b fail |
| MU8 | `run_adaptive_simple_marked` ignores the marker and uses fixed-fraction 0.4 marks of the velocity indicator | K3b fails |

### Predictions (mine)

| Id | Prediction |
| --- | --- |
| PR1 | K0 to K6, T1 to T3 met; T1 and T2 within 1 percent of the target |
| PR2 | `E(F)` between 0.01 and 0.05; `E(U90)/E(F)` between 1.7 and 2.2; NC-L1 to NC-L3 hold |
| PR3 | L1 met with `E(A)/E(F)` between 0.4 and 0.9; L2, L3, L5 met with `N_A` between 3840 and 3990 |
| PR4 | `E(A1)/E(A)` above 1 (`r = 1` puts the levels too close together) |
| PR5 | `D(T1)` about `D(U48)` (0.0030): on the cavity the target-count mesh of 2304 cells is about as good as the uniform one |

## Decision rule (registered)

The anisotropic criterion is not built (D5), so **done** is not reachable by this item; the rule is:

- **partial**: K1, K2, K3, K3b, K0, K0b, K5, K6 and T1 met (the three built criteria with their closed-form checks, the
  target-count acceptance number, defaults bit-identical), no negative control failing; and the layer acceptance either met (L1,
  L2, L5, L3, L4) or missed with each miss a `*_known_miss` with its number and a diagnosis. The note then states which part of
  the acceptance is met. MU1 to MU8 not caught are reported as gaps of the checks.
- **todo**: K5 or K6 fails (a default changed), a K check of a built criterion fails, T1 is missed, or a negative control fails
  (the layer comparison cannot discriminate); or any existing test changes.

## Limits of this item

- One layer flow (`R = 80`), one base, one target, five cycles, one exponent; chosen here from the arithmetic above, not from a
  run, and not tuned after it. The layer is one-dimensional, so the 2:1 faces of A are (except near the inlet and outlet)
  horizontal faces across which the solution does not vary in `x`: the case does not test the face-offset error of hanging faces
  on a two-dimensional feature (the cavity does). An isotropic quadtree spends cells along the layer that a directional split
  would save (D5).
- The error is the cell-centre max norm against the closed form; the cavity target runs compare counts, not accuracy.
- Steady SIMPLE only; no coarsening in the loop; no case key (the loop stays reachable from Rust only, F-342).
- Edge-length weighting is defined for box cells (`cell_extents` is the vertex bounding box); on other cells it is the bounding
  box measure.

## Open questions and choices for the maintainer

- **Q1. Reading of "edge-length weighting".** Options: (a) the per-axis edge-length variation with a length exponent (D3);
  (b) a target edge-length field (a metric) with "refine where `h > sqrt(2) h_target`"; (c) both. Taken on the maintainer's
  standing instruction of 2026-11-11 (take the recommendation): (a) now; (b) with the anisotropic follow-up, where a per-axis
  target length is what a directional split needs.
- **Q2. Reading of "register-based".** Options: (a) cell sets from region, field range and level, with set algebra and growth,
  used to mark or to confine marks (D4); (b) only the region masks of ADR 0054. Taken on the same standing instruction: (a).
- **Q3. Reading of the layer acceptance.** "The uniform mesh that reaches it" read as the fine uniform mesh F (L1, L2), and in
  the strict form as the coarsest uniform mesh reaching `E(F)` (L5, with NC-L1 and NC-L3). Taken on the same standing
  instruction: both registered and both counted.
- **Q4. Anisotropic adaptation.** Not taken here (D5): a redesign of `rcfd_mesh::adapt` (per-axis levels); proposed as a
  follow-up item. No default is affected.
- **Q5. Status of F-343.** By the rule (partial at best), with the maintainer's acceptance.
