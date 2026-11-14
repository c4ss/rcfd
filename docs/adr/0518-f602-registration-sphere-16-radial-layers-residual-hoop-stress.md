# ADR 0518: F-602 registration: thick sphere with J2 on a 4 x 4-per-face cubed-sphere octant with 16 radial Hex20 layers, residual hoop stress after unloading

Status: **REGISTERED 2026-11-14, written before any code of F-602 and before any registered run of its checks.** Not yet
accepted by the maintainer. Disclosure: the configuration of this item (4 x 4 per face, 16 radial layers) was already run
once, unregistered, as the ignored diagnostic `diag_radial_layers` of ADR 0512 (addendum 1: residual error `4.52e-3`,
pointwise residual error `5.86e-3 sigma_y`, elastic largest `9.30e-3`, elastic mean `2.44e-3`). The numbers below that
come from it are estimates, not registered results; the run of this ADR is the registered one. Results go in the addendum
at the end; nothing above the addendum is changed after a run.

Roadmap item F-602 (deps F-561), the follow-up of ADR 0512. Number 0518 given by the lead. Branch `f602-work` from main
2f8c1d57, worktree `w49i-work/f602`, scratch `w49i-work/f602-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; no case-file key; clean-room. This is a test item: no library file is edited.

## What F-602 asks

The acceptance of the roadmap entry, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. The partly plastic sphere of ADR 0475 on a 4 x 4-per-face cubed-sphere octant with 16 radial Hex20 layers: the residual hoop stress at the bore after unloading from p(1.25 a) within 2 percent of sigma_y - (12/7) p, the elastic Lame hoop stress at every integration point of the inner layer within 1 percent of the local value, S1 and S2 within 1 percent, with the 8-layer misses of ADR 0512 kept as a negative control.

Clauses: (a1) residual hoop stress at the bore, 2 percent; (a2) elastic Lame hoop stress at every integration point of the
inner layer, 1 percent of the local value; (a3) S1 and S2 of ADR 0475, 1 percent; (a4) the 8-layer misses of ADR 0512 kept as
a negative control.

## What was read

- ADR 0512 (F-561): the closed form `sigma_y - (12/7) p1` derived by hand and exact for `b/a = 2` (arithmetic 1 there, not
  repeated here), the readings (stress recovery at the 27 integration points of each Hex20, hoop stress from the second
  Piola-Kirchhoff stress, quadratic least-squares fit over the inner layer evaluated at `r = 1`, pointwise error relative to the
  local Lame value), the controls N1 to N4, mutations MT1 to MT7, the two misses at 8 layers (R1 `2.7005e-2`, E1 `1.1623e-2`)
  and the unregistered 16-layer diagnostic. ADR 0475 (F-422): the sphere, S1, S2, C1, the S4 path.
- `crates/rcfd-fem/tests/sa_j2_f561.rs`: the generator `sphere(n, layers, mat, p)` already takes the layer count as a
  parameter, the reader, `Ramp`, `fit_at`, `lame_hoop`, `residual_hoop`, `Checks`, `max_finite`. Taken: copied into the new
  test file `sa_j2_f602.rs`; `sa_j2_f561.rs` is not edited.

## Arithmetic done beforehand (disclosed)

1. **Mesh.** `n = 4`, 16 layers: `3 n^2 = 48` Hex20 per layer, 768 elements; inner layer = layer 0, radii `[1, 1.0625]`,
   elements `0 .. 47`, 1296 integration points (27 per element). Nodes by count: vertices `61 x 17 = 1037` (61 = cubed-sphere
   surface vertices of the octant, `3 (n+1)^2 - 3 (n+1) + 1`), tangential edges `108 x 17 = 1836` (108 = `3 * 2 n (n+1) - 12`),
   radial edges `61 x 16 = 976`: total `3849`. The same count for 8 layers (`549 + 972 + 488`) gives 2009, the registered value
   of ADR 0512, so the count formula is checked. Volume against `7 pi / 6`: the same angular mesh as ADR 0512 (`9.92e-5`).
2. **The residual formula.** Unchanged from ADR 0512: loaded bore state `sigma_theta = sigma_y - p1`, elastic unloading by
   `-p1` removes `(5/7) p1`; residual `sigma_y - (12/7) p1 = -0.628903 sigma_y`, `p1 = 0.9501934 sigma_y`. The plastic zone of
   the loading reaches `c = 1.25`, beyond the inner layer `[1, 1.0625]`, so the closed residual field holds over the whole
   layer; the unloaded bore state has `|sigma_theta - sigma_r| = 0.629 sigma_y < sigma_y`, so the unloading is elastic.
3. **Expected size of the errors, from the diagnostic only.** R1 `4.5e-3` (limit 2e-2, 4.4 times margin). E1 largest `9.3e-3`
   (limit 1e-2: **7 percent margin, thin**), mean `2.4e-3`. The largest E1 error comes from the angular scatter (the 2 x 2
   octant at 16 layers: `2.89e-2`); the radial part was halved by the doubled layer count. S1, S2 and C1 on 16 layers were
   not run before; the 8-layer values (`8.0e-5`, `1.0e-3`, 0 wrong) are the guide; thinner elements near the elastic-plastic
   front should not worsen the radius reading. The thin E1 margin is a known risk and reported as such whichever way it
   falls.
4. **Fit.** The quadratic fit is over the three Gauss radii of one 16-layer element thickness (the inner layer `[1, 1.0625]`,
   1296 points); the centre `t = r - 1.125` of the 8-layer fit is kept unchanged in the reading (a shift of the fit variable
   does not change the fitted polynomial, only the conditioning).

## Design

The new test file `crates/rcfd-fem/tests/sa_j2_f602.rs`; no library file is touched. Octant, material, reading, ramp and
fit as in ADR 0512 (copied, parameter `layers = 16` for the item, `layers = 8` for the controls); the registered limits are
the item's. The C1 band `0.05 a` and the S1 and S2 step counts (8 and 10) are as in ADR 0512. Every aggregate is asserted
finite (`max_finite` panics on a NaN; `Checks` treats a non-finite value as missed).

## Checks (limits fixed now)

| Id | Clause | Check | Limit | Estimate behind it |
|---|---|---|---|---|
| M1 | mesh | `n = 4`, 16 layers: 768 elements, 3849 nodes; octant volume (sum of the weights) vs `7 pi/6` | counts exact; volume `<= 5e-4` relative | arithmetic 1; ADR 0512 `9.9e-5` |
| R1 | a1 | residual hoop stress at the bore (fit at `r = 1` of the 1296 points of the inner layer after the 16-step load-unload ramp from `p1`) vs `sigma_y - (12/7) p1 = -0.628903 sigma_y`, `abs(FE - closed) / abs(closed)` | `<= 2e-2` | item; diagnostic `4.5e-3` |
| E1 | a2 | elastic hoop stress of every one of the 1296 points of the inner layer vs Lame `(p1/7)(1 + 4/r^3)`, the largest `abs(hoop - Lame) / Lame` | `<= 1e-2` | item; diagnostic `9.30e-3` (thin margin) |
| S1 | a3 | `c = 1.25 a`, 8 steps: `abs(p(c_FE) - p) / p` | `<= 1e-2` | item; 8 layers `8.0e-5` |
| S2 | a3 | `c = 1.5 a`, 10 steps: same | `<= 1e-2` | item; 8 layers `1.0e-3` |
| C1 | a3 comp. | points wrong at S1 and S2 (`r < c - 0.05 a` elastic, `r > c + 0.05 a` yielded) | 0 | 8 layers 0 of 10368 |
| F1 | all | every aggregated error finite; the ramp, elastic and loading solves return `Ok` | all | an `f64::max` over NaN would hide a failure |

Recorded without a limit (printed, quoted in the addendum, not checks): the loaded fit at the bore against `sigma_y - p1`; the
elastic fit at the bore against `5/7 p1`; the largest pointwise error of the unloaded hoop against the closed residual field
over the inner layer; the mean error of E1; the number of increments; the largest alpha of the inner layer; the elapsed time.

### Negative controls

| Id | Control | Expected |
|---|---|---|
| N1 | the 8-layer misses of ADR 0512 kept (the same code with `layers = 8`, `n = 4`): residual at the bore reproduces `-0.611919 sigma_y` (limit `2e-5` on `sigma_y`); the R1 error is above `2e-2` (ADR 0512 `2.7005e-2`); the E1 largest error is above `1e-2` (`1.1623e-2`) | all three: the limits discriminate 8 from 16 layers and the reader reproduces ADR 0512 |
| N2 | the residual is not the loaded hoop: `abs(FE residual - (sigma_y - p1)) / sigma_y` | `>= 0.5` (the closed gap is `0.679`) |
| N3 | the unloading leaves nothing without plasticity: the elastic material (`sigma_y = 1e30`) through the same `Ramp`: the largest absolute hoop stress of the inner layer at the end | `<= 1e-6 sigma_y` |
| N4 | the unloading is elastic: the equivalent plastic strain of every inner-layer point after the unloading equals that after the loading (up-only run, 8 steps), the largest difference | `<= 1e-12` |
| N5 | angular resolution matters for E1: `n = 2`, 16 layers (diagnostic `2.89e-2`): the E1 largest error | above `1e-2` |

### Planned mutations (on a scratch copy with its own target directory, one at a time, each running the registered test once after the registered run)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MT1 | the closed form `10/7` for `12/7` | R1 |
| MT2 | the reader takes `sigma_rr` for the hoop stress | R1, E1, N2 |
| MT3 | the fit evaluated at `r = 1.03125` (mid-layer) | R1, N2 |
| MT4 | the unloading leg removed (load only) | R1, N2, N3 |
| MT5 | the library: `alpha` not updated in the return map | S1, S2, C1 |
| MT6 | the library: the driver keeps the committed states | S1, S2, R1 |
| MT7 | the library: the yield stress used in the return map `1.05 sigma_y` | S1, S2, R1 |
| MT8 | the layer count of the item set back to 8 | R1, E1 (the item's own checks against the control N1) |

MT1 to MT4 and MT8 are test-file mutations; MT5 to MT7 edit `mat_j2.rs` and `nonlinear.rs` in a scratch copy; the worktree's
library is never edited. A mutation not caught is reported.

## Decision rule

- **done**: R1, E1, S1, S2, C1, F1 and M1 met, N1 to N5 as expected, every planned mutation caught by at least one check,
  fmt and clippy clean, the rcfd-fem suite, the baseline hashes and the docs-site test passing.
- **partial**: S1, S2, C1, F1, M1 and N1 to N5 met and a miss of R1 or E1 diagnosed and recorded as a `*_known_miss` test with
  the limit intact (in particular, an E1 miss in the thin margin: reported with its number, not tuned).
- **todo**: otherwise (in particular a changed default bit).

## Limits (not covered)

- One mesh density of the item (`n = 4`, 16 layers), the 8-layer controls and `n = 2` at 16 layers; no convergence study.
- Small-strain closed forms against the total-Lagrangian driver (Green-Lagrange, second Piola-Kirchhoff): the difference is
  of the order of the strain (`<= 3e-3`), included in the measured numbers.
- Only the unloading from `p(1.25 a)`, which is elastic. Perfect plasticity only; no case-file key.
- The "inner layer" is layer 0 (`r` from 1 to 1.0625), as the 8-layer layer 0 was `[1, 1.125]`; the two are not the same
  radii, so N1 compares codes and limits, not a like-for-like pointwise field.

## Open questions

None taken beforehand.

## Addendum 1: results (2026-11-14)

Registered run: `f602_registered_run`, made once (155.4 s in release on the shared machine; log
`f602-scratch/registered_run.log`) from the committed test file (commit 622f9ec2, the file before the split
below). It took more than two minutes, so after the run the test was marked `#[ignore]` as a heavy record and a fast test
`f602_fast_rows` (M1, E1 and N5: mesh and the elastic solve, 2 s) was added with the registered limits; no limit or reading
changed. The library is unchanged (no file under `crates/rcfd-fem/src` edited), so no default-bits record was needed.
The disclosed earlier unregistered run of this configuration (ADR 0512 diagnostic) and the registered run agree to the
digits quoted there (residual error 4.52e-3, elastic largest 9.30e-3).

| Id | Check | Limit | Measured | Met |
|---|---|---|---|---|
| M1 | 768 elements, 3849 nodes; volume vs `7 pi/6` | exact; `5e-4` | exact; `9.92e-5` | yes |
| R1 | residual hoop stress at the bore vs `-0.628903 sigma_y` | `2e-2` | `-0.626058 sigma_y`, `4.524e-3` | yes (4.4 times margin) |
| E1 | elastic hoop vs Lame, largest relative error over the 1296 points of the inner layer | `1e-2` | `9.296e-3` (mean `2.439e-3`) | **yes, thin: 7.0 percent margin** |
| S1 | `c = 1.25 a` | `1e-2` | `2.752e-4` | yes |
| S2 | `c = 1.5 a` | `1e-2` | `1.148e-3` | yes |
| C1 | points wrong at S1, S2 | 0 | 0 of 20736 each | yes |
| F1 | every aggregate finite, the solves `Ok` | all | all | yes |
| N1 | 8 layers: bore residual vs ADR 0512; R1 error above `2e-2`; E1 above `1e-2` | `2e-5`; above; above | `-0.611919` (diff `3.7e-7`); `2.7005e-2`; `1.1623e-2` | yes |
| N2 | gap of the residual to `sigma_y - p1` | `>= 0.5` | `0.676` | yes |
| N3 | elastic material through the Ramp, largest hoop after unloading | `1e-6 sigma_y` | `2.4e-182` | yes |
| N4 | alpha after unloading vs after loading, inner layer | `1e-12` | `0` (bit equal) | yes |
| N5 | `n = 2`, 16 layers, E1 error above `1e-2` | above | `2.891e-2` | yes |

Recorded without a limit: the 16 ramp increments; the loaded fit at the bore `0.055722 sigma_y` against `sigma_y - p1 =
0.049807`; the elastic fit at the bore `0.717281 p1` against `5/7 = 0.714286` (8 layers: `0.7248`); largest pointwise error of
the unloaded hoop against the closed residual field `5.858e-3 sigma_y` (8 layers `1.159e-2`); largest alpha of the inner
layer `1.284e-3`. One structural row was added to the registered run beyond the table (the inner layer holds 1296 points,
met), it is the count of arithmetic 1.

**Reading of the thin E1 margin.** The largest error `9.296e-3` is a pointwise maximum over 1296 points of a field whose
scatter at 4 x 4 per face is angular (the 2 x 2 octant gives `2.891e-2` at the same 16 layers, N5) and radial (8 layers
`1.162e-2`); the pass is real under the registered reading but 7 percent from the limit, so it is not evidence for a margin
at other loads or meshes. The R1 pass has a 4.4 times margin.

**Mutations** (`f602-scratch/mutate.py`, git-archive copy of the committed state with its own target directory, log
`mutations.log`, the registered test each time; MT0 reproduces a clean pass):

| Id | Caught by |
|---|---|
| MT1 `10/7` | R1 (`7.52e-1`) |
| MT2 hoop = `sigma_rr` | R1 (`1.002`), N2 (`4.8e-2`), E1 (`2.39`), N1 |
| MT3 fit at `r = 1.03125` | R1 (`1.67e-1`), N1 (`8.9e-2`); not N2 (the estimate in the table was wrong, harmless) |
| MT4 loading only | R1 (`1.089`), N2, N3 (`0.673`), N1 |
| MT5 alpha not updated | S1 (`4.06e-2`), S2 (`9.07e-2`), C1 (4320 and 9504 wrong); not R1 |
| MT6 committed states kept | R1 (`1.0`), N2, S1, S2, C1, N1 |
| MT7 yield `1.05 sigma_y` | R1 (`8.39e-2`), S1 (`2.13e-2`), S2 (`2.98e-2`), C1 (432 wrong at S2), N1 |
| MT8 item at 8 layers | M1, R1 (`2.7005e-2`), E1 (`1.1623e-2`): the item's own checks fail at 8 layers, the controls of N1 stay met |

Every planned mutation was caught.

**Other runs.** `cargo fmt -p rcfd-fem` makes no change; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean;
`cargo test -p rcfd-fem --release` (188 result lines, all `0 failed`); `cargo test -p rcfd-api --release --test
baseline_hashes` (3 passed, 1 ignored); `cargo test -p rcfd-cli --release --test docs_site` (2 passed). The whole-workspace
suite was not run (the lead's gate).

**Decision by the registered rule: done.** R1, E1, S1, S2, C1, F1, M1 met, N1 to N5 as expected, every mutation caught, fmt,
clippy, the fem suite, the baseline hashes and docs-site passing; library and baselines unchanged. The E1 margin is thin
(7 percent) and is reported as such.

**Open questions.**
1. The thin E1 margin. Options: (a) accept as registered; (b) a follow-up on a convergence study in the layer count and `n`
   (for example 24 layers) to show the maximum falls monotonically. Recommendation: (a); (b) only if a later item relies on
   this mesh.
2. The run exceeds two minutes (155 s) and is an ignored record with a 2 s fast test of the elastic part; the load-unload
   path of R1 is covered in the fast suite by `sa_j2_f561.rs` at 8 layers. Recommendation: keep as is.

## For the lead: roadmap entries

Status proposed for F-602: done.

> ADR 0518, 2026-11-14: DONE by the registered rule. The octant of ADR 0475 on 4 x 4 cells per face with 16 radial Hex20 layers (test-side cubed-sphere generator, crates/rcfd-fem/tests/sa_j2_f602.rs; no library change): residual hoop stress at the bore after unloading from p(1.25 a) -0.626058 sigma_y against sigma_y - (12/7) p1 = -0.628903 (error 4.52e-3, limit 2e-2); elastic Lame hoop stress at every one of the 1296 points of the inner layer, largest error 9.30e-3 (limit 1e-2, a 7 percent margin); S1 2.8e-4 and S2 1.1e-3 (limit 1e-2), C1 0 wrong of 20736 each. Negative controls: the 8-layer misses of ADR 0512 reproduced (-0.611919 sigma_y, 2.70e-2 and 1.16e-2), the n = 2 octant at 16 layers (elastic largest 2.89e-2), N2 to N4; 8 mutations caught. The registered run (155 s) is an ignored heavy record, the elastic rows run in a 2 s test.
