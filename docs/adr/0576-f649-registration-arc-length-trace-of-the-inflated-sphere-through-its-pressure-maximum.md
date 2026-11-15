# ADR 0576: F-649 registration: arc-length trace of the inflated hyperelastic sphere through its pressure maximum

Status: **REGISTERED 2026-11-15, written before any code of F-649 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-649 was written, compiled or run in the repository. The arithmetic below was done by
hand and with two plain numpy probes in the scratch folder (no rcfd code; disclosed, their numbers quoted below):
`w49i-work/f649-scratch/probe_f649.py` (log `probe_f649.log`: the closed form and its maximum, and the spherically symmetric
one-dimensional mean-dilatation model of ADR 0541's `probe_sphere1d.py`, copied from ADR 0557's `probe_lock1d.py`, evaluated
from `l_a = 1.5` to `3`) and `probe_ds.py` (log `probe_ds.log`: the norm of the incompressible inflation field on the nodes of
the mesh, to size the arc length). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-649 (deps F-622, F-424, F-572), structural solver, advanced tier: the follow-up proposed by
[ADR 0557](0557-f622-registration-full-hex8-locking-control-of-the-inflated-sphere.md) (its open question 2). Number 0576 given
by the lead. Branch `f649-work` from 5faf5266 (main), worktree `w49i-work/f649`, scratch `w49i-work/f649-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test
edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room; `docs/ROADMAP.md` and `docs/roadmap.json`
not edited. Every choice below that the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation).

## What F-649 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. The mean-dilatation Hex8 sphere of ADR 0541 under follower pressure traced by the arc-length step through its pressure maximum near l_a = 1.83, the maximum pressure within 1 percent of the closed form (mu/2)((1/l_b^4 + 4/l_b) - (1/l_a^4 + 4/l_a)) maximised over l_a, and every point after it on the descending branch within 1 percent; existing arc-length results bit-identical.

Clauses: **c0** every other limit registered here; **c1** the BBar (mean-dilatation, the F-bar Hex8 of ADR 0506) sphere of ADR
0541 D6 under follower pressure traced by `arclength::trace` through its pressure maximum near `l_a = 1.83`; **c2** the maximum
pressure of the trace within `1e-2` of `max_(l_a) p_c(l_a)`; **c3** every point after the maximum on the descending branch
within `1e-2`; **c4** existing arc-length results bit-identical.

Readings, registered:

- **c1** "Traced through": the trace runs without error from the unloaded state until the first point with the inner stretch
  `l_mean >= 2.5` (the registered end of the descending branch, arithmetic 3), and the point of the largest load factor is an
  interior point of the trace, with at least three points after it.
- **c2** "The maximum pressure of the trace": the largest `lambda_k p_ref` over the converged points (raw, no interpolation; the
  sampling error is bounded in arithmetic 4). The vertex of the parabola through the largest point and its two neighbours in
  `(l_mean, p)` is reported as well.
- **c3** "Within 1 percent" at a point: `|lambda_k p_ref - mu p_c(l_mean,k)| / (mu p_c(l_mean,k))`, the pressure of the point
  against the closed form at its measured inner stretch. "Every point after it": every converged point after the point of the
  largest load factor, up to and including the stop point.
- **c4** No library line is changed (the audit below finds no gap), so every existing result is bit-identical by construction;
  it is checked by the existing records and tests of the release suite (S1) and `baseline_hashes` (S2).

## Audit of what exists (done before this registration)

ADR 0560 (F-572) gave `ArcLengthOptions::contribution_load` (the load direction `f - sum_c d g_c / d lambda` at the iterate),
`Contribution::add_load_factor_derivative` implemented by `FollowerPressure` (`out -= force(u, 1)`), and
`NewtonOptions::unsymmetric`. Read: `src/arclength.rs` (whole: `trace` takes a zero dead load when `contribution_load` is on and
refuses one otherwise; `Spherical::correct`), `src/nonlinear.rs` (`load_direction`, `LoadFactorControl`, `controlled_attempt`:
the same `Problem::attempt_with` as `solve_with`, hence the same `is_fbar` dispatch to the F-bar element for a BBar block and
the same hyperelastic material path that ADR 0557 P3 converged on), `src/follower.rs` (`FollowerPressure::add`,
`add_load_factor_derivative`, `add_full`, `face_force`, `positions`), the test `sa_f572.rs` (the call pattern of `trace` with a
follower-only reference load), `sa_hyper_lock.rs` (the sphere helpers of ADR 0557). Nothing missing was found: `trace` accepts
the zero prescribed values of the symmetry planes, an empty force list, the BBar hyperelastic Hex8 block and the follower
pressure as the only load. **No library code is planned**; F-649 is one new test file. If a gap shows up in the run it is
recorded as a miss, diagnosed, and the library change is a follow-up (not made after the run under this registration).

## What was read (no run)

`AGENT_RULES.md`; the brief; ADR 0541 (arithmetic 5: the closed form, the maximum near `1.83` at `0.8162 mu`, the 1D probe;
D6 the sphere; addendum 1: SP1 `2.61e-3` at `1.5`); ADR 0557 (whole); ADR 0560 (the arithmetic, D1 to D3); the roadmap entries
F-649, F-622, F-572, F-424; the files of the audit.

## Arithmetic done before the code (disclosed)

1. **The closed form and its maximum** (`probe_f649.log`, golden section on the closed form of ADR 0541 arithmetic 5):
   `p_c,max = 0.81642365 mu` at `l_a = 1.828147`; `p_c'' = -0.77245 mu` there (`p''/p = -0.946`). On the descending branch
   `p_c(2.0) = 0.806927`, `p_c(2.5) = 0.726482`, `p_c(3.0) = 0.634507` (times `mu`); the pressure falls monotonically to zero as
   `l_a -> infinity` (`0.002 mu` at `1000`).
2. **The radial discretisation error** (`probe_f649.log`, the 1D model, 8 geometric layers, two-point Gauss, element-mean `J`,
   `kappa/mu = 1e4`): relative to the closed form at the same `l_a`, `+6.8e-4` (`1.5`), `+1.9e-4` (`1.8`), `+2.1e-5` (`1.95`),
   then negative, `-3.4e-4` (`2.5`), `-5.0e-4` (`3.0`). 1D maximum `0.81655090 mu` at `1.82687`: `+1.56e-4` relative to
   `p_c,max`. **The angular error of the 3D mesh** (from numbers known before this registration): SP1 of ADR 0541 at `1.5`
   `2.61e-3` (1D `6.8e-4`), SPLd of ADR 0541 / A2 of ADR 0557 at `1.125` and `1.10` `3.93e-3` and `4.06e-3` (1D `1.83e-3`,
   `1.95e-3`), and ADR 0557 P3 under the follower pressure at `p_c(1.5)`: `l_mean = 1.4955904`, which is a pressure error
   `p_c(1.5)/p_c(1.49559) - 1 = 2.6e-3` with the measure of c3. So the 3D error is the 1D error plus an angular part of about
   `+2e-3`, nearly constant from `1.10` to `1.5`. Estimates (the angular part held at `2e-3`, an extrapolation beyond `1.5`,
   the main uncertainty of this registration): at the maximum `2.2e-3`; on the descending branch `1.5e-3` to `2.2e-3`; on the
   ascending branch at most about `4.5e-3` at the first points (`l_a ~ 1.02`, 1D `2.4e-3` at `1.01`). Compressibility at
   `kappa/mu = 1e4`: `|J - 1| ~ p/kappa <= 1e-4`, a pressure change of the same relative order (ADR 0541 arithmetic 5).
3. **The end of the descending branch.** `l_mean >= 2.5`: the pressure has fallen `11` percent below its maximum, a clear
   descending branch, where the 1D radial error is still `3.4e-4`; beyond, the radial stretch of the inner layer is
   `1/l_a^2 <= 0.16` and the elements are thin, so the end is not taken further (a limit of this item, not a need of the
   acceptance, which names no end).
4. **The arc length and the sampling of the maximum** (`probe_ds.log`). The incompressible field `r^3 = R^3 + l_a^3 - 1` on the
   `127 x 9` nodes of D6: `|du/dl_a| = 20.36` at `l_a = 1`, `28.19` at `1.83`, `30.95` at `2.5`. With `p_ref = 0.8164 mu`
   (`= p_c,max`, so that `lambda ~ 1` at the maximum), the default load scale is `c = |K_0^-1 f_lambda(0, 0)|^2 ~ (0.8164/3.5
   x 20.36)^2 = 22.6` (the small-strain slope `dp_c/dl_a = 3.5 mu`, `probe_lock1d.log`). Registered `Ds = 0.65`: the first
   point at `dl_a ~ 0.0226`, `0.023` per point near the maximum, `0.021` at `2.5`, about 64 points to `2.5` (cap 200). The
   largest point is within `dl_a / 2 = 0.0116` of the discrete model's maximum, so the raw maximum is below the model's maximum
   by at most `0.473 x 0.0116^2 = 6.4e-5` relative (`|p''|/(2p) = 0.473`), far below the limit.
5. **The tangent.** The driver's default for a contribution is the symmetric part of the load stiffness. The inner surface
   is bounded by edges on the symmetry planes; on such an edge the normal component of the displacement is held, and the
   boundary term of the antisymmetric part (`p (dx x Du) . dv` along the edge, ADR 0480 arithmetic 3) vanishes in the continuum
   when `Du` and `dv` have no component normal to the plane and the edge lies in it. ADR 0557 P3 (the same load, the symmetric
   part) converged in 5 to 6 iterations per step. Registered: the default symmetric part, **not** `NewtonOptions::unsymmetric`
   (it would be a second capability on the critical run without need). The controlled correction factors the indefinite
   tangent (`|d| > PIVOT_TOLERANCE |a_ii|`), so the states past the maximum (one negative eigenvalue of the pressure-controlled
   tangent expected for the spherically symmetric mode) are reachable. Iteration cap `40` (as ADR 0557 P3), residual
   tolerance `1e-8` (ADR 0541 arithmetic 7), the default energy tolerance, 5 halvings, `threads(4)`.
6. **The iteration count** (I1). With the exact load direction the augmented Newton iteration converges as ADR 0557 P3 did
   (5 to 6 iterations); limit `<= 20` iterations at every converged point (a wrong load direction converges at best linearly).
7. **The fast test** (F1). A coarse sphere (`n = 3`, `m = 4`, 180 BBar Hex8, `kappa/mu = 100`, the default tolerances
   `1e-10`), `p_ref = 0.5 mu`, `Ds = 0.24`, 3 points (`l_a` to about `1.06`, ascending branch). At each point the state is
   compared with a load-controlled `solve_with` (follower pressure `lambda_k p_ref`, 5 steps): both are converged equilibria
   of the same equations on the unique ascending branch, and ADR 0560 arithmetic 3 found converged states path-independent to
   `2e-15`; with `kappa/mu = 100` a residual `1e-10` maps to a displacement error of order `1e-8`. Limit `1e-6`.
8. **The negative control** (NC1). Load control with the follower pressure `1.02 p_c,max` (the FE maximum is `p_c,max (1 +
   2.2e-3)` by arithmetic 2, so `1.8` percent above it): no equilibrium on the symmetric branch exists, `solve_with` must fail.

## Design

New test file `crates/rcfd-fem/tests/sa_arclength_sphere_f649.rs`, with copies of the sphere helpers of `sa_hyper_lock.rs`
(`l_b`, `p_closed`, `sphere_model` generalised to the bulk modulus, `boundary`, `deformed`, `cavity_volume`, `Checks`; that
file is not edited). No library code.

- **D1 (heavy, `#[ignore]`, run once).** The D6 sphere (`n = 6`, `m = 8`, 864 Hex8, BBar block, `neo_hookean(mu = 1e6,
  kappa = 1e10)`, density 1000), the symmetry conditions (zero), `FollowerPressure::new(model, "inner", p_ref)` with `p_ref =
  0.81642365 mu` as the only load, `trace(model, prescribed, &[], ArcLengthOptions::new(0.65).max_points(200)
  .newton(NewtonOptions::new(1).threads(4).max_iterations(40), residual 1e-8).contribution_load(true), [&pressure], stop at the
  first point with l_mean >= 2.5)`. Per point: `lambda`, `l_mean` (mean `|x_a|` over the 127 inner nodes), the cavity-volume
  stretch, the iterations, the halvings, the negative pivots, printed.
- **D2 (heavy, `#[ignore]`, run once).** NC1: `solve_with` on the D6 BBar sphere with `FollowerPressure(1.02 p_c,max mu)`,
  `NewtonOptions::new(10).threads(4).max_iterations(40)`, residual `1e-8`.
- **D3 (fast).** F1 to F3 on the coarse sphere of arithmetic 7.

## Checks (limits fixed now)

Every measure is required finite before it is compared (a non-finite value misses); a failed trace misses every check of D1.

| Id | Clause | Measure | Limit | Estimate |
|---|---|---|---|---|
| T1 | c1 | D1: the trace returns `Ok`, stopped by `l_mean >= 2.5` within 200 points; the largest load factor at an interior point with `>= 3` points after it | holds | about 64 points, the maximum near point 37 |
| M1 | c2 | D1: `|max_k lambda_k p_ref - p_c,max mu| / (p_c,max mu)` | `<= 1e-2` | `2.2e-3` |
| D1c | c3 | D1: `max` over the points after the maximum of `|lambda_k p_ref - mu p_c(l_mean,k)| / (mu p_c(l_mean,k))` | `<= 1e-2` | `2.2e-3` |
| M2 | c1 (companion, "near 1.83") | D1: `|l_mean` at the largest point `- 1.828147|` | `<= 0.05` | `0.012` (half a point) |
| A0 | companion | D1: the measure of D1c over the points before the maximum (the ascending branch) | `<= 1e-2` | `4.5e-3` |
| C1 | companion | D1: the constraint at every point, `|(|Du|^2 + c Dlambda^2) - Ds^2| / Ds^2` on the free equations | `<= 1e-10` | `1e-14` |
| I1 | companion | D1: the iterations of every converged point (`residuals.len()`) | `<= 20` | 5 to 7 |
| R1 | report | D1: the parabola vertex through the largest point and its neighbours (`l`, `p / p_c,max`), the negative pivots per point, the cavity-volume stretch, the spread of `|x_a|`, points and halvings | reported | one negative pivot after the maximum |
| NC1 | control | D2: `solve_with` at `1.02 p_c,max` returns `Err` | holds | fails |
| F1 | c1 (fast path) | D3: `max_k max |u_al - u_lc| / max |u_lc|` over the 3 points | `<= 1e-6` | `1e-8` |
| F2 | control (fast) | D3: the same trace with `contribution_load(false)` refused, `FemError::InvalidValue` key `"arc length"` | holds | refused |
| F3 | companion (fast) | D3: the constraint at each of the 3 points as C1 | `<= 1e-10` | `1e-14` |
| S1 | c4 | `cargo test -p rcfd-fem --release` (includes `sa_arclength.rs`, `sa_f572.rs`, the default-bit records) | 0 failed | |
| S2 | c4 | `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | |
| S3 | | `cargo fmt -p rcfd-fem -- --check`; `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` | clean | |

### Planned mutations (on a `git archive` copy of the final tree in `f649-scratch/mut`, target `f649-scratch/target-mut`, each alone by exact text replacement, the new test file run once with `--include-ignored`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `follower.rs` `add_load_factor_derivative`: `let f = self.force(u, 1.0)?;` replaced by a zero vector (no load direction from the follower) | T1 (the refusal), F1 |
| MU2 | `follower.rs` `face_force`: the traction sign flipped (`t = +p c`) | T1, M1, D1c, F1 |
| MU3 | `follower.rs` `positions`: the displacement ignored (a dead pressure on the reference faces) | M1 (the dead load has its maximum elsewhere), D1c, F1 |
| MU4 | `arclength.rs` `Spherical::correct`: the root of the smaller score taken (`>=` replaced by `<`) | T1 |
| MU5 | `nonlinear.rs` `is_fbar` returns `false` (the BBar block evaluated by the Full element, which locks and loses stability at `1.133`, ADR 0557) | T1, M1 |

Outside the rule, declared now as a probe whose result is reported only (it may converge to the right points at a slower rate,
so it is not a registered mutation): **MP1**, `add_load_factor_derivative` with half the derivative (`0.5 * force(u, 1)`):
reported whether I1 catches it.

A mutation is caught if any registered check of the new test file misses (a trace that fails misses its checks).

## Decision rule

- **done**: T1, M1, D1c met in their single registered run (c1 to c3), M2, A0, C1, I1, NC1, F1 to F3 met, S1 to S3 met, and
  every planned mutation MU1 to MU5 caught.
- **partial**: T1 met, and M1 or D1c missed (a `*_known_miss` test, diagnosed), or a companion or control missed, or a mutation
  not caught, or a check met only after an amendment.
- **todo**: T1 missed (the trace does not pass the maximum), or S1 or S2 fails.

## Limits (not covered)

- One mesh; the descending branch only to `l_mean = 2.5`; the trace of the Full element is not attempted (ADR 0557: no stable
  state beyond `1.133`).
- No aspherical bifurcation is sought (the octant model restricts the modes to those with its symmetry; the negative pivots are
  reported only).
- The unsymmetric factor of ADR 0560 is not used on the sphere (arithmetic 5).
- The case-file keys of the follower load direction (F-652 of ADR 0560) are not touched.

## Open questions

None taken. (The end `2.5` of the descending branch and the use of the symmetric part are registered choices, taken on the
maintainer's standing instruction of 2026-11-11, take the recommendation.)
