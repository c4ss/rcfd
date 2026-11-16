# ADR 0650: F-712 registration: symmetry planes and a uniform in-plane flow on the annulus interface pair

Status: **REGISTERED 2026-11-16, written before any code of F-712 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-712 was written and nothing of F-712 was run; the code of the base
was read only, the numbers quoted from ADR 0637 were measured there (disclosed below), and the arithmetic below was done in
`w49i-work/f712-scratch/arith.py` (disclosed). Results go in the addenda at the end; nothing above the addenda is changed
after a run.

Roadmap item F-712 (no deps), proposed by [ADR 0637](0637-f303-registration-sliding-interface-in-the-case-tree.md) (F-303)
question Q7. Parents: ADR 0637 (the set-ups TP, TS, UT, UTc and its amendment 1 diagnostic),
[ADR 0216](0216-f250-registration-sliding-interface-on-a-cylinder.md) (F-250, the uniform-flow checks U2, U3 with fixed
planes), [ADR 0638](0638-f710-registration-sliding-interface-at-coinciding-nodes.md) (F-710, the coinciding-node fix of
`sliding_step`), [ADR 0010](0010-component-diagonal.md) (the component-consistent diagonal, opt-in
`solver.component_diagonal = "corrected"`) and [ADR 0074](0074-wave34c-symmetry-and-sst-convergence.md) (its transient
wiring W1 and the analysis `docs/component-diagonal-default.md` of what the flipped default would change). Number 0650 given
by the lead. Branch `f712-work` from d40cd665 (local main, F-303 and F-710 merged), worktree `w49i-work/f712`, scratch
`w49i-work/f712-scratch`.

House rules (`w49i-work/AGENT_RULES.md`) apply: limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests and goldens
not edited; defaults bit-identical; `baseline_hashes.rs` and the hash tables not touched; no new dependency, no unsafe code;
clean-room; the roadmap files not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where listed as open.

Clean-room: the diagnosis is first-principles arithmetic on the discrete momentum and pressure equations of the repository's
own PISO kernel (read in `crates/rcfd-fv/src/pimple.rs`, `bc.rs`, `pressure.rs`); the remedy is the repository's existing
ADR 0010 correction, which ADR 0010 describes as the split `fvMatrix::H` uses (a published, documented property of the
segregated-solver H operator; from memory, not read for this item). No vendor code, text or binary was used.

## What F-712 asks

The acceptance, binding:

> the uniform in-plane flow of ADR 0637 UT on the frozen join of annulus_3d_cylindrical_interface with zmin, zmax symmetry (2.4e-2 after one step) is diagnosed; either it is kept to 1e-10 with the cause fixed and every changed default result listed for the maintainer, or the limitation is documented with its cause.

Clauses:

- **a1** the miss is diagnosed: the cause named, with evidence that separates it from the frozen join and the mesh;
- **a2** either the flow is kept to `1e-10` (velocity in `|U|`, pressure in `|U|^2`) with the cause removed, and every
  default result that would change if that removal were the default is listed; or the limitation is documented with its cause.

The brief of the lead adds: a fix that changes default results is not made the default (opt-in, measured, the changed
default results listed: `baseline_hashes` rows and existing tests).

## Disclosed before this registration

- ADR 0637 amendment 1 measured (a scratch diagnostic, five steps): UT's largest velocity deviation `2.36e-2` after step 1
  and `6.22e-2` after step 5, the same with `angular_velocity = 0` and without the table (pressure `2.9e1`); with `zmin`,
  `zmax` fixed at `U` `3.9e-15` (table) and `4.4e-16` (no table). The registered UT (40 steps, table) measured `7.24e-2`,
  `2.35e1`.
- ADR 0010, ADR 0074 and `docs/component-diagonal-default.md` (read): the averaged component diagonal is a known sink of the
  tangential velocity next to a symmetry plane; `corrected` removes it exactly in the cases measured there (half-channel
  Poiseuille, plug, Couette with symmetry side planes, a transient plug over 40 PISO steps to `2e-16`). Flipping the default
  in wave 34C moved one `baseline_hashes` row (`packed_bed`) and changed the outcome of five tests (the list is quoted in
  B below). That analysis predates many tests now in the tree.
- No run of F-712 was made before this text.

## What the code read shows (base d40cd665, read only)

1. `rcfd_fv::bc::Symmetry::coeffs` for a vector: `FaceCoeffs::from_value(1 - n_k^2, -n_k sum_{j != k} n_j u_P,j, dn)`. On
   the planes `zmin`, `zmax` (`n = (0, 0, -+1)`) the in-plane components get `a = 1` (face value = cell value, no diagonal
   term) and `u_z` gets `a = 0` (Dirichlet 0, diagonal term `nu A / dn`).
2. `PimpleSolver::step_with` (`pimple.rs`, the fixed-mesh corrector loop near line 2040, and the moving-mesh loop near line
   2500 that the sliding step runs): `a_p = (1/n_active) sum_k diag_k`, `rAU = V / a_p`, and
   `HbyA_k = H_k / a_p` by default, `(H_k + (a_p - diag_k) u_k) / a_p` with `PimpleSettings::component_diagonal` (ADR 0069
   F3), which `solver.component_diagonal = "corrected"` sets for `piso` / `pimple` (`sim.rs` near line 2019, ADR 0074 W1).
3. The new cell velocity is `HbyA - rAU grad p` with `grad p` the Green-Gauss gradient (`green_gauss(.., 0)`) of the new
   pressure with its boundary values; on a fixed-velocity wall the pressure condition is zero gradient (`pressure_bc`), so
   the wall face value is the cell value.
4. `[mesh.sliding]` (`sim/sliding.rs`) refuses neither `component_diagonal` nor `zero_gradient` planes; without the table the
   generator's frozen join runs the ordinary fixed-mesh path.

## Diagnosis (derived, before any run)

**D-a. The averaged diagonal is a uniform spurious drag.** The pair is one cell deep (`nz = 1`, `lz = 0.1`), so every cell
touches both planes. For the uniform state `u = U = (1, 0.5, 0)`: the time, convection and in-plane diffusion parts of the
three component matrices are equal, so `H_x = diag_x U_x`, `H_y = diag_y U_y`, `H_z = 0`, and `diag_z - diag_x = 2 nu A_z /
(lz / 2) = 4 nu V / lz^2`. With the average `a_p = diag_x + (diag_z - diag_x) / 3`,
`HbyA = lambda U`, `1 - lambda = (diag_z - diag_x) / (3 a_p)`: the in-plane momentum carries a drag `k u` with
`k = 4 nu / (3 lz^2) = 13.33 s^-1` (`nu = 0.1`), absent from the continuous problem. With `corrected`,
`H_x + (a_p - diag_x) U_x = a_p U_x`, so `HbyA = U` exactly and the drag is gone.

**D-b. Where the velocity error comes from.** The pressure equation balances the drag: a linear pressure `p = -k U . x + c`
gives face fluxes `interp(lambda) U . S + interp(rAU) k U . S = U . S` on every internal face (`lambda + rAU k = 1` in every
cell, the same linear interpolation) and satisfies the prescribed wall fluxes, so it is the discrete solution of the pressure
equation (to the non-orthogonal convergence). The face fluxes are then exact; the **cell** velocity `lambda U - rAU grad p`
is exact only where the Green-Gauss gradient of that linear pressure is exact. In a wall cell the wall face value is the cell
value (zero-gradient pressure on a fixed-velocity wall), which is wrong by `-(grad p . n) dr / 2`; with `A / V = 1 / dr` the
cell gradient is wrong by about half its normal component, so the cell velocity is wrong by about `(1 - lambda) |U . n| / 2`.
Arithmetic (arith.py): `1 - lambda = 0.0335` (rotor wall cells, `r = 1`), `0.045` to `0.048` (next to the interface),
`0.042` (stator wall cells, `r = 2`); the wall-cell error `(1 - lambda) / 2` = `0.017` (inner) to `0.021` (outer). The
disclosed `2.36e-2` after one step is of that size. The skewed and the non-conformal interface faces add Green-Gauss errors
of the same mechanism. So the cause is the averaged diagonal (the drag that needs a pressure gradient), revealed by the cell
gradient of that gradient; neither the symmetry condition's face values nor the frozen join is needed for it.

**D-c. Predictions.** (i) With `corrected` the drag is gone, the pressure stays uniform and the uniform state is a fixed
point to round-off, on the frozen join, on a conformal annulus and through the sliding table. (ii) With `zero_gradient`
planes (no diagonal term on any component, `u_z` stays 0 by the uniform state) the averaged default keeps the flow too. (iii)
On a conformal `annulus_3d` (no interface) with symmetry planes and the default the miss persists, with the largest deviation
in a wall cell. (iv) After step 1 of the default the volume-weighted least-squares pressure slope is `-k U` =
`(-13.33, -6.67)`.

## Design

No library change is planned: the cause is removed by the existing opt-in `solver.component_diagonal = "corrected"` (ADR
0010, transient wiring ADR 0074 W1). It is not made the default (it changes default results, B below). F-712 adds one test
file, `crates/rcfd-api/tests/symmetry_uniform_f712.rs`, the measurements B1 and B3 on an uncommitted scratch copy, and a
paragraph in `docs/component-diagonal-default.md` (the topic page of the key). If a check of A fails because the opt-in does
not reach a path (for example the sliding step), the wiring defect is diagnosed and fixed opt-in in an amendment registered
before the fix.

## Set-ups

All from ADR 0637's TP and TS (generator `annulus_3d_cylindrical_interface`, `nx = 144`, `ny = 8`, `nx_right = 120`,
`ny_right = 8`, `nz = nz_right = 1`, `r_in = 1`, `r_interface = 1.5`, `r_out = 2`, `lz = 0.1`; `piso`, `central`,
`non_orthogonal_correctors = 6`, tolerances `1e-13`, `1e-13`, `1e-16`, `dt = 0.00545415391248228`), with UT's flow:
`nu = 0.1`, `inner`, `outer` fixed at `U = (1, 0.5, 0)`, `[initial] velocity = U`; `zmin`, `zmax` as stated.

- **UF** (UT on the frozen join): UT without `[mesh.sliding]`, planes `symmetry`, default diagonal.
- **UFc**: UF with `solver.component_diagonal = "corrected"`.
- **UFz**: UF with `zmin`, `zmax` `velocity = "zero_gradient"` (default diagonal).
- **UA**, **UAc** (conformal, no interface): generator `annulus_3d`, `nx = 144`, `ny = 16`, `nz = 1`, `r_in = 1`,
  `r_out = 2`, `lz = 0.1`; otherwise UF, UFc.
- **USc** (through the sliding table): ADR 0637's UT (`[mesh.sliding] angular_velocity = 1`) with `corrected`; the step
  lands on node coincidences every 8 steps (F-710's fix in).
- **C3** (3-D walls only, for B2): `box_3d` `4 x 4 x 4` unit cube, lid `ymax` fixed at `(1, 0, 0)`, the other five patches
  fixed 0, `nu = 0.01`, `piso`, `dt = 0.01`, 20 steps; default against `corrected`.

Measures as ADR 0637 U1: after a step `du = max_c |u_c - U| / |U|`, `dp = max_c |p_c - pbar| / |U|^2` (`pbar` the volume
mean); "every step" is the largest over the steps. All runs in release, each made once.

## Checks (limits fixed now)

### A. The acceptance

| Id | Check | Limit | Estimate |
|----|-------|-------|----------|
| A1 (a2) | UFc, 40 steps: every step `du`, `dp`; every step `continuity` | `<= 1e-10` each; `<= 1e-9` | D-a: `HbyA = U` exactly, round-off `~1e-15` (UTc measured `4e-16`) |
| A2 (a1) | UF reproduces the miss: `du` after step 1 | in `[2.35e-2, 2.37e-2]` | ADR 0637 printed `2.36e-2` (same set-up, deterministic) |
| A3 (a1) | UF after step 1: volume-weighted least-squares fit `p = c + b . (x, y)`; `abs(b - (-k U)) / (k abs(U))` | `<= 0.10` | D-b: linear pressure `-k U . x`, `k = 13.33` |
| A4 (a1, not the symmetry face values) | UFz, 40 steps: every step `du`, `dp` | `<= 1e-10` each | D-c (ii) |
| A5 (a1, not the join) | UA after step 1: `du` | in `[1e-2, 4e-2]` | D-b: `0.017` to `0.020` in the wall cells (arith.py), a factor 2 for the other Green-Gauss errors |
| A6 (a1, not the join) | UA after step 1: the cell of the largest deviation is an owner of a face of `inner` or `outer` | yes | D-b |
| A7 (a1, a2 on a conformal mesh) | UAc, 40 steps: every step `du`, `dp` | `<= 1e-10` each | D-c (i) |
| A8 (extension: the sliding path) | USc, 40 steps (five coincidence steps): every step `du`, `dp` | `<= 1e-10` each | D-c (i) and ADR 0638 (UTc `6.0e-15`) |

### B. What `corrected` as the default would change (measured, for the maintainer)

| Id | Measurement | Limit / form |
|----|-------------|--------------|
| B1 | Scratch copy with the default flipped (`solver.component_diagonal` treated as `corrected` unless a case writes `averaged`, in `sim.rs` (both places), `vd.rs` and `sim/adapt.rs`); `baseline_hashes.rs` of that copy changed to print every row instead of asserting (scratch only); `cargo test -p rcfd-api --release --test baseline_hashes -- --include-ignored` once | Prediction: exactly the row `packed_bed` moves (ADR 0074 C1), the 2-D and symmetry-free rows (`cavity-*`, `heated-*`, `sod`, `wedge`, `kepsilon_channel`, `rotating_channel`) do not. Reported row by row. |
| B2 | C3, default against `corrected` (in tree, no flip): largest cell difference of `u` and `p` after 20 steps; bit equality | `<= 1e-12` (relative to the lid speed and to the pressure range). The transient split is applied without the bit-equality guard of the steady kernel, so a 3-D symmetry-free case may change in its last bits; reported whether it does. |
| B3 | The flipped scratch copy of B1: every non-ignored test in the 49 `crates/rcfd-api/tests/*.rs` files that name `symmetry`, `partial_slip`, `"shear"` or `marangoni` (the component-specific conditions), `--release --no-fail-fast`, once | Reported: every test whose outcome changes. No limit (an enumeration); compared with ADR 0074's list. |

### Negative controls

- **NC1** = A2: the default on the same frozen join misses (shows that A1's measure sees the defect).
- **NC2** = A5: the default on the conformal mesh misses (shows that A7's measure sees it there).

### Mutations (scratch copy, not committed; each run against the new test file)

| Id | Mutation | Must fail |
|----|----------|-----------|
| MU1 | `pimple.rs`, the fixed-mesh corrector loop: the `component_diagonal` branch of `hby_a` replaced by `h[c] / a_p[c]` | A1, A7 |
| MU2 | `pimple.rs`, the moving-mesh corrector loop: the same | A8 |
| MU3 | `sim.rs`: the transient `s.component_diagonal = ...` set to `false` | A1, A7, A8 |
| MU4 | `bc.rs` `Symmetry::coeffs`: the normal component's `a` set to 1 like the others (no diagonal term) | A2, A5 (the controls lose the defect) |

## Decision rule

**done**: A1 to A8 met, every mutation caught, B1 to B3 measured and reported, no golden, hash table or existing test edited.
**partial**: A1 met (the acceptance's "kept to 1e-10 with the cause fixed") but a diagnostic check (A2 to A6) or A8 missed,
or B not measured in full; the miss named. **todo**: A1 missed and no cause established.

## Limits (not covered)

- The default is not changed: the remedy stays the opt-in `corrected`; making it the default is the maintainer's decision
  (open question), with B as the evidence. B3 runs only the 49 test files that name a component-specific condition; the
  other 179 files of `rcfd-api` and the other crates are not run flipped (they can change only through the unguarded
  transient split, B2's size).
- Only the uniform in-plane flow on one-cell-deep pairs; deeper meshes, steady SIMPLE, variable density and VOF are not
  examined here (ADR 0010, 0074 and 0103 cover them).
- The Green-Gauss wall-cell gradient error (D-b) is a property of the default pressure gradient; it is named, not changed.
