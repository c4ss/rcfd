# ADR 0418: F-503 registration: the second-order ring gradient of ADR 0409 on RE and RL of ADR 0347

Status: **REGISTERED 2026-11-13, written before any code of F-503 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-503 was written, compiled or run: the documents, logs and code under "What was
read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand (no script). Results go in
the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0409](0409-f479-registration-overset-second-order-ring-gradient.md) (F-479: the opt-in
`OversetSolver::set_second_order_ring_gradient`, D6; its question 2, recommendation (a), from which this item comes),
[ADR 0347](0347-f445-registration-overset-prescribed-fringe-flux.md) (F-445: RE, RL, the runs `base` and `Qb`, `R_b`, the
checks C1, C2, U1, A2, S3 and the hypothesis HT of amendment 2), [ADR 0340](0340-f349-registration-overset-background-fringe-response.md)
(the set-ups and `R_b`), [ADR 0251](0251-f336-registration-overset-quadratic-donor-interpolation.md) (the quadratic donor
interpolation). Roadmap item F-503 (depends on F-479), low priority. Number 0418, branch `f503-ring-gradient-re-rl`, worktree
`w49i-work/f503`, scratch `w49i-work/f503-scratch`, given by the lead.

House rules (`w49i-work/AGENT_RULES.md`, the campaign rules of 2026-11-12, applied on 2026-11-13): limits fixed here and not
changed after a run; a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded; a
defective set-up is corrected only by an amendment committed before the corrected run; each registered run is made once;
existing tests and records are not edited; defaults bit-identical; `baseline_hashes.rs`, the hash tables and the goldens are
not touched; no new crate, no unsafe code; unsigned commits without trailers; no push.

Clean-room. Nothing outside this repository was opened. This item is a measurement: no algorithm is added. The methods
measured are those of ADR 0347 (D1 to D5), ADR 0409 (D6: a weighted least-squares quadratic fit, derived there) and ADR
0251 (the quadratic donor interpolation, derived there).

## What F-503 asks

Acceptance (`docs/roadmap.json`, binding in full):

> With OversetSolver::set_second_order_ring_gradient(true) under PrescribedOwnGradient and the quadratic donor
> interpolation, RE of ADR 0347 at levels 1 to 3 and RL at levels 1 to 3: the root mean square of the velocity error over
> the calculated cells at t = 0.5 is at most 1.05 times that of the default coupling with the same stencils, C1, C2 and U1
> of ADR 0347 hold, and the orders of R_b on RE are at least 1.7; or the ADR names with a number why not.

**Reading (registered here).**

- `Q`: `set_fringe_flux(PrescribedOwnGradient)` and `set_second_order_ring_gradient(true)`; `G`: the first alone; default:
  `PressureEquation`.
- "The quadratic donor interpolation" and "the same stencils": `Overset::set_donor_interpolation(Quadratic)` on both
  fringes, in the run of `Q` and in the run of the default coupling, exactly as ADR 0347 amendment 2 (HT, `error_run` of
  `overset_f445.rs`: `200 L` steps of `OversetSolver::step` to `t = 0.5`, closed domain, mass mode conservative).
- "The root mean square of the velocity error over the calculated cells at t = 0.5": `velocity_error(s, 0.5).1` of
  `overset_f445.rs` over the decay factor `e^{-2 nu 0.5}` (the measure of S3 and HT), the calculated cells of both parts,
  volume-weighted.
- "C1, C2 of ADR 0347": in every step of every run under `Q` of this item: continuity `<= 1e-9`; `abs(hole_flux)` and the
  net flux through the four sides, each over the largest face area, `<= 1e-9`; in U the outflow `abs(Q_xmax - 0.5) / 0.5
  <= 1e-10`.
- "U1 of ADR 0347": the set-up U (ADR 0219, 80 steps, moving and turning component, linear stencils as registered there)
  under `Q`: velocity and pressure `<= 1e-10`, with the set-up check U4 (kind changes `>= 20`).
- "The orders of R_b on RE": `R_b` exactly as A2 of ADR 0347 (ADR 0340): the base run with the library's linear stencils
  against the run `Qb` whose background fringe takes the quadratic stencils, the root mean square over the calculated
  component cells of the velocity difference at `t = 0.5` over the decay factor; both runs under `Q`; the orders between
  levels 1 and 2 (ratio 2) and 2 and 3 (ratio 1.5). `R_b` cannot be formed with the quadratic stencils in the base run (the
  Qb substitution would then change nothing), so the clause "with the quadratic donor interpolation" governs the error
  ratio, not `R_b`. Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).
- "Or the ADR names with a number why not": registered below as the hypothesis HX with its deciding number, fixed now.

## What was read (no run, nothing compiled)

- ADR 0409 in full (registration and addendum), ADR 0347 in full (registration, amendments 1 and 2, addendum), the part of
  ADR 0376 on the root mean square by class (lines 75 to 95 and 295 to 305).
- `crates/rcfd-fv/tests/overset_f445.rs` in full (RE, RL, `run`, `pair`, `r_b`, `error_run`, `u_run`, the HT test),
  `crates/rcfd-fv/tests/overset_f479.rs` (the mode `Q`, `apply`, A5, A6), `OversetSolver::set_second_order_ring_gradient`
  and `OversetInfo::n_fallback` in `crates/rcfd-fv/src/overset.rs`.
- The logs of ADR 0347 in `w49i-work/f445-scratch` (existing output, nothing re-run): `f445-ht.log` (RE, root mean square
  velocity error at `t = 0.5`: G linear `9.522792e-4`, `2.293455e-4`, `1.064533e-4`; G quadratic `5.991924e-4`,
  `1.376739e-4`, `6.007299e-5`; default quadratic `5.234643e-4`, `1.303540e-4`, `5.802028e-5`; no fallback; run times G
  quadratic 4.5, 31.8, 86.2 s, default quadratic 4.4, 28.0, 174.1 s) and `f445-a_re_levels_1_to_3.log` (`R_b` under G at `t =
  0.5`: `5.765037e-4`, `1.479331e-4`, `7.162776e-5`).

## Arithmetic before this registration (disclosed, by hand; no script)

1. **Where G stood on RE with the same stencils.** `e(G, quad) / e(default, quad)` = `1.1447`, `1.0562`, `1.0354` (HT): the
   limit `1.05` of this item is missed by G at levels 1 and 2 and met at level 3.
2. **The excess of G is of third order.** `x_L = e(G, quad) - e(default, quad)`: `7.573e-5`, `7.332e-6`, `2.053e-6`;
   orders `log2(7.573e-5 / 7.332e-6) = log2(10.33) = 3.37` and `ln(7.332e-6 / 2.053e-6) / ln 1.5 = ln(3.571) / 0.4055 =
   3.14`. The error itself converges at second order (`log2(5.2346 / 1.3035) = 2.01`, `ln(1.3035 / 0.5802) / ln 1.5 =
   2.00`). An excess of third order is what the truncation of the quadratic donor interpolation (third order, ADR 0251)
   handed on in full by the prescribed flux would give (HT, confirmed), and also what the first-order ring gradient would
   give (ADR 0409 item 2: `O(h dt)`, with `dt` proportional to `h` here: `O(h^2)` in the step, the excess of the band order
   3 on TG_L, where `dt` is proportional to `h^2`). The two causes are not separated by the orders.
3. **What the opt-in did on TG_L (ADR 0409 addendum, the root mean square by class at level 1, Q / G of ADR 0376):** CF
   `3.89 / 3.73 = 1.043`, CC `3.61 / 3.56 = 1.014`, BF `3.57 / 3.41 = 1.047`, BC `5.485 / 5.51 = 0.995`: the opt-in removed
   the local maximum of the band (`1.93` to `0.89` of the default) but did not lower the root mean square. If RE behaves
   the same, `e(Q) ~ e(G)` and the ratio stays near `1.14`, `1.06`, `1.04`. If instead the whole excess of item 2 were the
   ring gradient, `e(Q) ~ e(default)`. The ring cells are a larger share of the calculated cells on RE (component: `15 L`
   cells per side, the calculated block `(15 L - 2)^2`, its ring `4 (15 L - 3)`: `48` of `169` at level 1, 28 percent)
   than on TG_L, which favours a larger effect. **Estimate E (RE): `1.05` to `1.14` at level 1, `1.02` to `1.06` at
   level 2, `1.01` to `1.04` at level 3; predicted missed at level 1, near the limit at level 2, met at level 3.**
4. **RL.** No number with the quadratic stencils exists for RL under G or the default coupling. G with the linear stencils
   on RL: `1.468e-3`, `2.059e-4`, `9.60e-5` (ADR 0347 addendum), a fall by `7.1` from level 1 to 2: RL at level 1 is not in
   the asymptotic range (component spacing `0.75 H`, fringe at changing fractions of a donor cell). **No trusted
   estimate; level 1 the likelier miss.**
5. **The orders of `R_b` under Q.** Under G `1.962`, `1.789` (ADR 0347 A2). `R_b` is the response to the truncation of the
   background fringe; D6 changes the gradient of the ring cells of both runs alike, and changes the response only through
   the ring cells' answer to the changed fringe velocity, which item 2 of ADR 0347 makes independent of the ring gradient
   (the ring cell takes the flux of its inner face). **Estimate: `1.85` to `2.0`, `1.7` to `1.85`; the second near the
   limit.**
6. **C1, C2, U1.** As G with D6 on TG_L (ADR 0409: continuity `6.0e-13`, fluxes `1.1e-14`, `0`; U1 `3.6e-15`, `0`, 245
   kind changes): round-off. U is a uniform flow with a constant pressure, for which D6 is exact.
7. **Cost (one process, a loaded machine; ADR 0347 times).** RE per level: Q, default and G with the quadratic stencils
   and the Q pair (base, Qb): about `25 s`, `4 min`, `12 min` at levels 1 to 3. RL per level: Q and default with the
   quadratic stencils: about `25 s`, `3 min`, `8 min`. Fast test at level 1 about one minute; RE at levels 1 to 3 and RL at
   levels 1 to 3 are two ignored heavy records, each run once.

## The design

No library change is planned. A new test file `crates/rcfd-fv/tests/overset_f503.rs` copies the helpers of
`overset_f445.rs` (set-ups RE, RL, U, `run`, `pair`, `r_b`, `velocity_error`, `Checks`) and the mode `Q` of
`overset_f479.rs`. The error run (`error_run` of HT) is copied with two changes that do not change its arithmetic: a step
that returns `Err` ends the run and is recorded (instead of a panic), and C1, C2, C3 and the largest `n_fallback` over the
steps are recorded. If a run shows that a library change is needed, it is registered by an amendment before its code and
runs, opt-in, and `baseline_hashes` is then run.

## Registered checks

Release: `cargo test --release -p rcfd-fv --test overset_f503`. The heavy records RE and RL are ignored, run once. The
fast test repeats the level-1 runs of both records (deterministic); the registered numbers at level 1 are those of the
heavy records, and the fast test must print the same digits (control DT).

| Id | Check | Limit | Estimate behind it |
| --- | --- | --- | --- |
| E1 (acceptance) | RE, `L = 1, 2, 3`: `e(Q, quad) / e(default, quad)` | `<= 1.05` each | item 3: `1.05` to `1.14`, `1.02` to `1.06`, `1.01` to `1.04` |
| E2 (acceptance) | RL, `L = 1, 2, 3`: `e(Q, quad) / e(default, quad)` | `<= 1.05` each | item 4: none trusted |
| C1 (acceptance) | continuity, every step of every run under Q (E1, E2, the R_b pairs, U) | `<= 1e-9` | item 6 |
| C2 (acceptance) | every step of every Q run: `abs(hole_flux)` / largest face area; in RE and RL the net flux through the four sides / largest face area; in U the outflow error | `<= 1e-9`; `<= 1e-9`; `<= 1e-10` | item 6 |
| U1 (acceptance) | U under Q, every step, non-hole cells: velocity, pressure; U4 kind changes | `<= 1e-10` each; `>= 20` | item 6 |
| A2 (acceptance) | Q on RE: orders of `R_b(0.5)` (linear base against Qb), levels 1 to 2 and 2 to 3 | `>= 1.7` each | item 5 |
| F | every run (Q and default, RE and RL, the pairs) takes every step (no `Err`, no velocity above 10) | all | ADR 0347, 0409 |
| C3 | unconverged linear solves, every run (Q, G and default) | `0` | ADR 0347 |
| QD (control) | the error runs (Q, G, default, RE and RL): the largest `n_fallback` over the steps | `0` | HT: no fallback on RE |
| QS (control) | the Q pairs: as ADR 0347 QS | exact | ADR 0347 |
| QG (control) | RE level 1: the state hash after the Q error run differs from that after the G error run | differ | D6 changes the ring gradient |
| DR (control) | RE: `e(default, quad)` and `e(G, quad)` at levels 1 to 3 equal the HT numbers of `f445-ht.log` (`5.234643e-4`, `1.303540e-4`, `5.802028e-5`; `5.991924e-4`, `1.376739e-4`, `6.007299e-5`) | relative `<= 1e-6` | same code, deterministic (the library changed since ADR 0347 only by opt-ins) |
| DT (control) | the level-1 numbers of the fast test equal those of the heavy records | relative `<= 1e-12` | deterministic |

Printed, no limit: `e(Q) / e(G)` (quadratic) on RE; the excess `x_L = e(Q) - e(default)` and its orders on RE and RL;
`R_b` at the five times of ADR 0347; the pressure error; run times.

### HX: the "why not" of the acceptance (registered now, before any run)

If E1 or E2 misses at a level, the alternative clause is taken as met for that set-up **only** if this deciding number
holds on it: the excess `x_L = e(Q, quad) - e(default, quad)` is positive at levels 1, 2 and 3 and its orders between
levels 1 and 2 (ratio 2) and 2 and 3 (ratio 1.5) are both `>= 2.7`. Meaning: the excess of the opt-in over the default
coupling vanishes at third order, faster than the error itself (second order), so the ratio tends to 1 and falls below
`1.05` with refinement; the number named is the excess at each level and its order. Estimate (item 2): `3.37`, `3.14` for
G; no estimate for RL. If the excess is not positive at every level or an order is below `2.7`, HX is not met and the miss
stands unexplained by the registered rule. Printed with HX: the level `L*` at which the ratio would reach `1.05` if the
excess kept its last order and the error its last order (extrapolated, no limit).

### Mutations (scratch copy, not committed)

Each applied alone to a `git archive` copy of the final tree in `E:/projects/cfd/w49i-work/f503-scratch` (its own target
directory), the fast test run once; it must fail by the named check.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MS1 | `crates/rcfd-fv/src/overset.rs`: the D6 branch of `own_gradient` not taken (the opt-in ignored, D5 used) | QG |
| MS2 | `crates/rcfd-fv/src/overset.rs`: D2 `q_R = 0` (as ADR 0347 MU1) | C1 or C2 |
| MS3 | `crates/rcfd-mesh/src/overset.rs`: `set_donor_interpolation` does nothing (the linear stencils stay) | DR (level 1) |

### Classes

A miss of E1, E2, C1, C2, U1, A2, F or C3 is reported with its number and kept as an ignored `*_known_miss` test (or record)
with the limit intact; the met checks of the same runs are asserted by a fast test. A control (QD, QS, QG, DR, DT) that
does not hold, or a mutation not caught, is a defective set-up or a fault, corrected only by an amendment committed before
any re-run.

## Predictions (mine, before any run)

C1, C2, U1, F, C3, QD, QS, QG, DR, DT met. A2 met, the order 2 to 3 within `0.15` of the limit. E1 missed at level 1
(about `1.1`), uncertain at level 2, met at level 3; HX met on RE. E2 unknown, level 1 the likeliest miss.

## Decision rule

- **done**: C1, C2, U1, A2, F, C3 met, the controls held, every mutation caught, and on each of RE and RL either E met at
  the three levels or HX met (the alternative clause of the acceptance, as registered above).
- **partial**: C1, C2 and U1 met, and E missed on a set-up without HX met, or A2, F or C3 missed (known miss with the
  measured number), or a control or mutation did not hold and was not corrected.
- **todo**: no registered run could be made, or C1, C2 or U1 missed.

## Limits of this registration

- Only RE and RL (uniform Cartesian parts, the component at rest, `nu = 0.1`, implicit Euler, two correctors); no moving
  component beyond U, no BDF2, outer iterations or `rhie_chow_step_consistent` with the opt-in.
- `R_b` with the linear base stencils only (see the reading); the orders of `R_b` on RL under Q are not registered (the
  acceptance names RE); `R_b` on RL is not run.
- Level 4 and above not run.
- HX names a rate, not a mechanism: it does not separate the hand-on of the interpolation truncation (HT) from the ring
  gradient (item 2). `e(Q) / e(G)` is printed for that, without a limit.

## Open questions

Listed with options and a recommendation in the addendum, after the results.
