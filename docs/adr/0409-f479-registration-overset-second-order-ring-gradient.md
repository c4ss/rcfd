# ADR 0409: F-479 registration: a second-order own-part pressure gradient of the ring cells under the prescribed fringe flux (opt-in)

Status: **REGISTERED 2026-11-13, written before any code of F-479 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-479 was written, compiled or run: the documents and the code under "What was read"
were read, and the arithmetic under "Arithmetic before this registration" was done by hand (no script). Results go in the
addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0376](0376-f450-registration-overset-prescribed-fringe-flux-with-a-moving-component.md) (F-450: G with the
moving, turning component of TG_L; the excess of G in the band of calculated background cells next to the background fringe,
`1.93` and `1.52` times the default coupling at levels 1 and 2; its question 2, recommendation (a), from which this item
comes), [ADR 0347](0347-f445-registration-overset-prescribed-fringe-flux.md) (F-445: D1 to D5, the checks UA, UA2, U1, G0),
[ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (TG_L, the settings PS, amendment 2: `dt`
proportional to `h^2`), [ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (the quadratic donor
interpolation). The later overset opt-ins [ADR 0382](0382-f472-registration-overset-geometry-corrections-with-a-varying-pressure.md)
(L), [ADR 0387](0387-f487-registration-overset-consistent-cell-pressure-gradient.md) (`set_consistent_geometry_corrections`)
and [ADR 0396](0396-f468-registration-overset-pose-continuous-donor-stencil.md) (`DonorInterpolation::Continuous`) are not
changed (the first two act only with the geometry corrections, which are refused with a prescribed fringe flux; the third is
a stencil of the coupling, which this item does not touch). Roadmap item F-479 (depends on F-450), low priority. Number 0409,
branch `f479-ring-own-part-gradient`, worktree `w49i-work/f479`, scratch `w49i-work/f479-scratch`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`, the campaign rules of 2026-11-12, applied on 2026-11-13): limits fixed here and not
changed after a run; a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded; a
defective set-up is corrected only by an amendment committed before the corrected run; each registered run is made once;
existing tests and records are not edited; defaults bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are not
touched; no new crate, no unsafe code; unsigned commits without trailers; no push.

Clean-room. Nothing outside this repository was opened. The method is the weighted least-squares fit of a Taylor polynomial
to the values of neighbouring cells, which is textbook finite-volume practice (the quadratic form of the least-squares
reconstruction used for higher-order unstructured schemes; named from memory, e.g. T. J. Barth, "Recent developments in high
order k-exact reconstruction on unstructured meshes", AIAA 93-0668, 1993). Nothing is taken from it: its exactness, its order
and its rank condition are derived below from the definitions.

## What F-479 asks

Acceptance (`docs/roadmap.json`, binding in full):

> An opt-in of FringeFlux::PrescribedOwnGradient whose ring cells take a pressure gradient of second order from the
> calculated cells of their own part (exact for a linear pressure, no fringe value); with it, on TG_L of ADR 0219 moving with
> the quadratic donor interpolation, the largest error of the calculated background cells within two cells of the background
> fringe is at most 1.2 times that of the default coupling at levels 1 and 2, the largest error over the calculated and fringe
> cells at most 1.2 times, continuity and the outflow balance hold to 1e-9 in every step, the checks of ADR 0347 (UA, UA2, U1)
> hold, and without the opt-in every result is bit-identical.

Note: low priority, from ADR 0376 question 2, recommendation (a), taken on the maintainer's standing instruction of
2026-11-11.

**Reading (registered here).** "Ring cells": the calculated cells with at least one face to a fringe cell of the same part
(the `RingCell`s of ADR 0347 D5), on both parts. "Second order": the gradient error of a smooth pressure is `O(h^2)`; with
the exactness for a quadratic pressure this is what the check Q0 measures. "Within two cells of the background fringe": the
band `B` of ADR 0376 amendment 1 (`band_errors` of `overset_f450.rs`: the calculated background cells within two face steps of
a background fringe cell at the final connectivity). "TG_L moving": the runs of ADR 0376 (`tg_pose(t)`, `200 L^2` steps to
`t = 0.5`, quadratic donor interpolation, closed domain, mass mode conservative), the opt-in run against the default
coupling with the same stencils. "The outflow balance": as ADR 0376 A3, the flux into the hole region and the net flux
through the four sides, each over the largest face area. "The checks of ADR 0347 (UA, UA2, U1)": those three set-ups with
their limits, run under G with the opt-in on. "Every result bit-identical": the default coupling and G without the opt-in.

## What was read (no run, nothing compiled)

- ADR 0376 in full (registration, amendment 1, addendum), ADR 0347 (arithmetic items 1 to 3, D1 to D5, the checks, the
  addendum's tables), ADR 0219 (TG_L, amendment 2), the heads of ADR 0382, 0387, 0396.
- `crates/rcfd-fv/src/overset.rs`: `FringeRing::new` and `own_gradient` (D5 and its refusal `inverse3`), the three calls of
  `own_gradient` (the predictor and the velocity correction in `piso`, the level condition of D3 in `prescribed_pressure`),
  `advance` (the refusals of the opt-ins), the unit test G0.
- `crates/rcfd-fv/src/gradient.rs` (`least_squares`, `solve_masked`, which solves 3 by 3 systems only).
- `crates/rcfd-fv/tests/overset_f450.rs` (the TG_L runs, `band_errors`, B1 and its constants) and
  `crates/rcfd-fv/tests/overset_f445.rs` (UA, UA2, U1).
- Numbers quoted from ADR 0376's addendum (not re-run): TG_L moving, quadratic stencils, band `B`: G `9.7081e-4` (level 1),
  `1.7016e-4` (level 2), default `5.0226e-4`, `1.1219e-4`; `e_max` default `7.6831e-4`, `1.9299e-4` (domain-boundary cell),
  G `9.7081e-4`, `1.9301e-4`; run times at level 2: G 115 s, default 203 s.

## Arithmetic before this registration (disclosed, by hand; no script)

Notation: a ring cell `C` on a uniform Cartesian part with spacing `h`, its fringe face in the direction `+n`, its own-part
neighbours `W` (at `-h n`), `WW` (at `-2 h n`); `H` = `HbyA`, `rAU = theta dt` with `theta = (1 / dt) / (1 / dt + 4 nu /
h^2)`; for TG_L `theta = 400 / 464.8 = 0.861` at level 1 and `1600 / 1859 = 0.861` at level 2 (`dt` proportional to `h^2`).

1. **Why D5 is first order and a linear one-sided least-squares gradient is no better.** D5 gives the normal part `(p_C -
   p_W) / h` (ADR 0347 item 2), whose error for a smooth pressure is `(h / 2) p_nn`. The linear least-squares gradient over
   the own-part face neighbours `W`, `N`, `S` (weighted or not) has, by the reflection symmetry `t -> -t` of the stencil
   along the ring, normal equations that decouple, and the only offset with a normal part is `W`: its normal part is again
   `(p_C - p_W) / h`. A linear fit over the cells within two face steps adds `WW`, `NW`, `SW`; its normal part is a weighted
   mean of one-sided first differences, still exact only for linear fields, error `O(h p_nn)`. **To be second order the fit
   must hold the quadratic terms.** (So `solve_masked` of `gradient.rs`, a 3 by 3 solve, is not the tool; the design takes a
   small dense solve of the quadratic fit instead.)
2. **What the first-order error does (the excess of ADR 0376).** In the velocity correction `u_C = H_C - rAU g_C`; the flux of
   the inner face is `m_in = (H_C + H_W) / 2 - rAU (p_C - p_W) / h`. With D5, `u_C . n = m_in + (H_C - H_W) / 2`; an interior
   cell has `u_C . n = (m_e + m_w) / 2 - (h^2 / 4) H_nn`. Taylor expansion about `x_C` with `m` the face flux field (a
   second-order approximation of `u . n`) and `H = u + rAU p_n + ...`: D5 minus interior is `(h / 2)(H_n - m_n) = (h / 2)
   rAU p_nn + O(h^2)`. An error `eps` of `u_C` re-enters through `H_C` with the weight `theta / 2`, so the excess settles at
   about `(h / 2) theta dt p_nn / (1 - theta / 2)`: with `abs(p_nn) <= e^{-0.2} = 0.82` at `t = 0.5`, divided by the error
   scale `e^{-0.1}`, `1.34e-4` at level 1 and `1.68e-5` at level 2 (ratio 8, **order 3**, since `h dt` falls by 8). Measured
   (ADR 0376): the excess of the band maximum is `9.7081e-4 - 5.0226e-4 = 4.69e-4` and `1.7016e-4 - 1.1219e-4 = 5.80e-5`,
   ratio `8.1`: the order matches; the size is `3.5` times my one-cell estimate (the prescribed flux `(u_C + u_F) / 2` of the
   ring face hands the error on to the region; not modelled). So the excess of G is the first-order ring gradient, within
   this model.
3. **The quadratic fit and its exactness.** For the ring cell `C`, the stencil `S_C` is the set of calculated cells of the
   same region (ADR 0347 D2: joined to `C` through calculated cells) within two face steps of `C`, without `C`; no fringe or
   hole cell is in it. With `m` the number of active directions (`FvGeometry::active`; 2 on these meshes), `d_k = x_k - x_C`,
   a length `l = min_k abs(d_k)` and `e_k = d_k / l`, the basis `phi(e) = (e_i ; e_i e_j, i <= j)` over the active directions
   (`n_b = m + m (m + 1) / 2` = 5 in two dimensions, 9 in three), the weights `w_k = 1 / abs(e_k)^2` and `M = sum_k w_k phi_k
   phi_k^T`, the fit `a = M^{-1} sum_k w_k phi_k (p_k - p_C)` minimises `sum_k w_k (phi_k . a - (p_k - p_C))^2`, and the
   gradient is `g_i = a_i / l` (the first `m` entries; zero in an inactive direction). It is precomputed as `g = sum_k c_k
   (p_k - p_C)` with `c_k = w_k (M^{-1} phi_k)_{1..m} / l`. **Exactness**: if `p` is a quadratic polynomial in the active
   directions, `p_k - p_C = phi(e_k) . a*` exactly with `a*` its scaled Taylor coefficients, the residual vanishes at `a =
   a*`, and when `M` is positive definite the minimiser is unique, so `a = a*` and `g` is the exact gradient at `x_C`; a linear
   pressure is the case `a*_{quadratic} = 0`. The value of a fringe cell never enters. **Order**: for a smooth `p` the
   residual of each point is the cubic Taylor remainder `O(abs(d_k)^3)`, the coefficients are `O(1 / h)`, so `g` is exact to
   `O(h^2)`. In the 1D model of item 2 (the points `W`, `WW` alone) it is `(3 p_C - 4 p_W + p_WW) / (2 h)`.
4. **The rank condition.** `M` is positive definite if and only if the matrix with the rows `phi(e_k)` has full column rank,
   that is, no nonzero polynomial `q` of degree at most two in the active directions with `q(0) = 0` vanishes at every point
   of `S_C`. So at least `n_b` points are needed, and they must not lie on one conic through `C` (in particular not on one or
   two lines through `C`). Cartesian side cell (fringe on one side): `W, WW, N, S, NN, SS, NW, SW` (eight points); a `q =
   a x + b y + c x^2 + d x y + e y^2` vanishing on `N, S, NN, SS` has `b = e = 0`, on `W, WW` `a = c = 0`, on `NW` `d = 0`:
   full rank. Corner cell of a convex ring (fringe on two adjacent sides): at least `W, WW, S, SS, SW`; the same elimination
   gives full rank with these five. A cell with fringe on opposite sides is refused by D5 before the fit (unchanged). The test
   in the code: a Cholesky factorisation of `M`, refused when a pivot is below `1e-8` times the diagonal entry it started from
   (the squared sine of the angle between the basis column and the span of the earlier ones; `0.1` and larger on the Cartesian
   stencils above by inspection of the side case, where the columns `x` and `x^2` share only `W`, `WW`, `NW`, `SW`). A refused
   cell refuses the step with a message naming the cell (as D5).
5. **What the quadratic gradient leaves (estimate for A1).** In the 1D model, `u_C . n = (3 / 2) m_in - (1 / 2) m_in2 + (1 /
   4)(H_C - 2 H_W + H_WW)`, `m_in2` the flux of the face `W WW`. Expanded as in item 2, minus the interior relation: `(h^2 /
   2) rAU p_nnn + O(h^3)`. With `abs(p_nnn) <= 2 abs(p_nn)` (wave number 2) and the feedback weight `theta / 4`: the ratio of
   the new excess to that of D5 is at most `h 2 (1 - theta / 2) / (1 - theta / 4) = 0.114` at level 1 and `0.057` at level 2.
   Scaled from the measured excess of D5: `5.3e-5` and `3.3e-6`, so the band ratio `(5.0226e-4 + 5.3e-5) / 5.0226e-4 = 1.11`
   at level 1 and `1.03` at level 2. The two-dimensional fit takes the diagonal points too, whose weights I did not work out;
   **estimate: band ratio `1.05` to `1.2` at level 1, `1.01` to `1.08` at level 2.** Not trusted beyond the model: the ring
   pressure under G carries the one-sided truncation (ADR 0347 item 2), which the quadratic fit sees through `p_C`.
6. **`e_max` (A2).** If the band maximum of the opt-in stays below the boundary error of the default (`7.6831e-4`,
   `1.9299e-4`), the ratio is the default's own boundary cell: **estimate `1.00` at both levels** (item 5: `5.6e-4` at level
   1, `1.15e-4` at level 2).
7. **Stability of the lagged prescription (item 2 of ADR 0347 redone).** The ring cell's normal velocity is now the linear
   extrapolation `(3 / 2) m_in - (1 / 2) m_in2` of its two inner face fluxes. For a disturbance of the ring flux `m` that the
   region answers harmonically, a mode of tangential wave number `k` gives `m_in = s m`, `m_in2 = s^3 m`, `s = e^{-k h / 2}`
   in `(0, 1]`, so `c / m = (3 s - s^3) / 2`, whose maximum on `(0, 1]` is `1` at `s = 1`: `abs(c) <= abs(m)` as for D5, and
   with the `H` part (weights `1/4, -1/2, 1/4` of `1 - theta`) the factor per corrector stays below `1/2 + (1 - theta) / 2 =
   0.57`. **Predicted stable**; not proven beyond this model (A3, A4 measure it).
8. **Exactness for UA, UA2, U1.** UA and UA2 have a linear pressure, U1 a constant one: the fit is exact (item 3), D3 uses the
   same gradient (it does not depend on the level constant, being a sum of differences). **Estimate: the numbers of ADR 0347
   (`1.5e-15`, `1.8e-15`, `3.6e-15`, `0`).**
9. **Round-off of Q0.** Coefficients `abs(c_k)` of the size `1 / h` (about `13` on TG_1's background, `17` on its component),
   pressures up to about `30` in the test, eight points: about `13 x 8 x 30 x 1.1e-16 = 3.4e-13`, with the conditioning of `M`
   (entries up to `16 / 4`, smallest pivot about `0.1`) at most `1e-11`. Limit `1e-10`.
10. **Cost.** The fit is a 5 by 5 solve per ring cell per step (some hundred cells). The runs: level 1 two runs of about 3 s
    each (ADR 0376: G 2.9 s, default 3.0 s), level 2 two runs of about 2 and 3.5 minutes (G 115 s, default 203 s): the level-2
    test is an ignored record of about 6 minutes, the level-1 test the fast companion.

## The design

**Opt-in.** `OversetSolver::set_second_order_ring_gradient(on: bool)` (default off) and the getter
`second_order_ring_gradient()`. It acts under `FringeFlux::PrescribedOwnGradient` only; a step with it on under another
fringe-flux mode is refused (as ADR 0382's least-squares gradient without the corrections). Taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation): a switch of G rather than a fourth `FringeFlux` variant, because
the acceptance names it "an opt-in of FringeFlux::PrescribedOwnGradient" and because a new variant of a public enum without
`#[non_exhaustive]` would break an exhaustive `match` of a user.

**D6 (with the opt-in). The ring gradient from the quadratic fit.** `FringeRing` is built as before; with the opt-in every
`RingCell` gets, after D5's matrix has passed its refusal, the coefficients `c_k` of item 3 over its stencil `S_C` (item 4's
refusal on a rank-deficient fit). `own_gradient` then sets `g_C = sum_k c_k (p_k - p_C)` for such a cell instead of D5's
solve, at all three places where D5 acts (the momentum predictor, the velocity correction, the level condition of D3), so the
step uses one ring gradient throughout. No other cell, flux or matrix changes; D1 to D4 are those of ADR 0347. The private
constructor keeps its signature (`FringeRing::new(g, conn)` is D5 alone, used by the unchanged G0) and gains a sibling taking
the switch.

**Defaults.** Without the opt-in no new line is reached: the default coupling does not build the ring (ADR 0347), and G
without the switch builds the ring of ADR 0347 with no coefficients, so `own_gradient` takes D5's branch with the same
arithmetic in the same order. `PimpleSettings`, the case tree, the schema and the goldens are not touched.

## Set-ups

- **TG_L moving** (`L = 1, 2`): exactly the runs `tg_run(L, mode, true)` of `overset_f450.rs` (copied into the new test file
  `crates/rcfd-fv/tests/overset_f479.rs`), with the mode `Q` = G plus the opt-in, and the default coupling; both with the
  quadratic donor interpolation.
- **UA, UA2, U1**: the set-ups of ADR 0347 (`ua_run`, `u_run` of `overset_f445.rs`, copied) under `Q`.
- **Q0 (unit, in `overset.rs`)**: the composite of TG_1 at `tg_pose(0.3)` (rotated by `0.3` and translated; ADR 0376 ran
  through this pose without a refusal), `p = 3 + 2 x - 5 y + 0.7 x^2 - 1.1 x y + 0.4 y^2` in the calculated cells, the fringe
  cells `10` times a fixed pseudo-random sequence in `-0.5..0.5` (G0b's), hole cells 0.
- **Q1 (unit)**: the fit alone on point sets: the five points of the corner stencil of item 4 (`h = 0.1`), and five points on
  one line through `C`.

## Registered checks

Release: `cargo test --release -p rcfd-fv --test overset_f479` and `cargo test --release -p rcfd-fv --lib overset`. Level 2
is an ignored heavy record, run once.

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| A1 (acceptance) | TG_L moving, `L = 1, 2`: `e_B(Q) / e_B(default)`, `e_B` the largest error `abs(u_c - u_ex(x_c, 0.5)) / e^{-2 nu 0.5}` over the band `B` | `<= 1.2` each | item 5: `1.05` to `1.2`, `1.01` to `1.08` |
| A2 (acceptance) | the same runs: `e_max(Q) / e_max(default)` over the calculated and fringe cells | `<= 1.2` each | item 6: `1.00` |
| A3 (acceptance) | `Q` moving, both levels, every step: `OversetInfo::continuity` | `<= 1e-9` | ADR 0376: `1.6e-13`, `6.2e-13` |
| A4 (acceptance) | `Q` moving, both levels, every step: `abs(hole_flux)`; the net flux through the four sides; each over the largest face area | `<= 1e-9` each | ADR 0376: `9e-15`, `0` |
| A5 (acceptance) | UA under `Q`: velocity, pressure, continuity; UA2 under `Q`: velocity, pressure after the first step, continuity; U1 under `Q`: velocity, pressure, continuity, flux through `Gamma_B`, outflow, kind changes, unconverged solves | as ADR 0347: `1e-10`, `1e-9`, `1e-9`; `1e-10`, `1e-9`, `1e-9`; `1e-10`, `1e-10`, `1e-9`, `1e-9`, `1e-10`, `>= 20`, `0` | item 8 |
| A6 (acceptance) | bit-identity: TG_1 moving, 40 steps (`b1_hash` of `overset_f450.rs`), default coupling with the quadratic and the linear stencils: the constants `e43a026a778e83b3`, `9c5ab1e5b21d5368` of ADR 0376; G without the opt-in, quadratic stencils, 40 steps: the hash measured once on the registration commit (unedited library) before any other run; the same after `set_second_order_ring_gradient(false)` | exact | design "Defaults" |
| A6b (acceptance) | `cargo test -p rcfd-api --release --test baseline_hashes`; the existing `overset_*.rs` test files of `rcfd-fv` (their tests that are not ignored) and `rcfd-fv --lib`, unedited, release | all pass | design "Defaults" |
| A7 | `Q` moving, both levels: every step returns `Ok` (no refusal of D5 or of the fit) | all 200 / 800 | items 4, 7 |
| C3 | both runs of each level: unconverged solves; the largest `n_fallback` | `0`; `0` | ADR 0376 |
| K (set-up) | `Q` moving, level 1: kind changes summed over the steps | `>= 20` | ADR 0376: 142 |
| Q0 (unit) | the gradient of every ring cell (both parts) against the exact gradient of the quadratic `p`; and the same of the linear part alone (`p = 3 + 2 x - 5 y`); control: D5's gradient of the quadratic `p`, its largest error | `<= 1e-10`; `<= 1e-10`; control `>= 1e-2` | item 9; D5's error `(h / 2) abs(p_nn) = 0.055` |
| Q1 (unit) | the corner stencil: the fitted gradient of a quadratic exact; the collinear stencil: refused | `<= 1e-12`; refused | items 3, 4 |
| R | a step with the opt-in on under the default coupling and under `Prescribed` returns `Err`; the getter reports the switch | refused | design |

Printed, no limit: the band ratio of the component band `B_C`, the largest error outside the bands (both runs), the class
maxima and root mean squares, the place of each maximum, run times; for comparison the D5 numbers of ADR 0376 (not re-run).

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f479-scratch` (its own target
directory), the named test run once; it must fail by the named check.

| Id | Mutation (in `crates/rcfd-fv/src/overset.rs`) | Must fail |
| --- | --- | --- |
| MQ1 | the fit without its quadratic columns (a one-sided linear least-squares gradient) | Q0 (quadratic) |
| MQ2 | the stencil admits fringe cells (no kind or region filter) | Q0 |
| MQ3 | G builds the coefficients without the opt-in | A6 (G hash) |
| MQ4 | the rank threshold `1e-8` raised to `1e30` (every fit refused) | A7 (fast test, level 1) and Q1 |
| MQ5 | the scale `1 / l` of the gradient dropped | A5 (UA velocity) and Q0 |

### Classes

A miss of A1, A2, A3, A4, A5, A7, C3 or K is reported with its number and kept as an ignored `*_known_miss` test (or record)
with the limit intact; the met checks of the same runs are asserted by a fast test. A control (A6, A6b, Q0's control) that does
not hold, or a mutation not caught, is a defective set-up or a fault, corrected only by an amendment committed before any
re-run.

## Predictions (mine, before any run)

All met: A1 at about `1.1` (level 1) and `1.03` (level 2), A2 `1.00`, A3 and A4 at round-off, A5 at the numbers of ADR 0347,
A6, A7, Q0 at `1e-12` or below. The likeliest miss: A1 at level 1, if the pressure of the ring cells carries a non-smooth part
that the quadratic fit turns into an `O(h)` gradient error (item 5's caveat).

## Decision rule

- **done**: A1 to A7, C3, K, Q0, Q1, R met and every mutation caught by its registered check.
- **partial**: the opt-in built with A3, A4, A5, A6, A6b met, and A1, A2, A7, C3 or K missed (known miss with a diagnosed
  cause), or a mutation not caught by its registered check.
- **todo**: the opt-in could not be built, or A6 / A6b fail (then the library change is withdrawn), or no registered run
  could be made.

## Limits of this registration

- One flow set-up (TG_L, two levels, uniform Cartesian parts, the square hole), implicit Euler, two correctors; no BDF2,
  outer iterations or `rhie_chow_step_consistent` with the opt-in (they compose as with G; not measured).
- No three-dimensional run; the fit is written for any number of active directions and Q1 covers only two.
- RE and RL of ADR 0347 are not rerun with the opt-in (the acceptance names TG_L).
- Level 3 is not run.
- The cause of the factor `3.5` between my one-cell estimate of D5's excess and the measured excess (item 2) is not
  modelled.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
