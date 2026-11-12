# ADR 0232: F-314 registration: the observed-order miss of the overset coupling (diagnosis of T1, T2 at level 2 and T1c of ADR 0219)

Status: **REGISTERED 2026-11-12**, written before any code of F-314 and before any run of its checks. Not yet accepted by the
maintainer. Before this text nothing was compiled or run: the code and the logs listed under "What was read" were read, and
the hand arithmetic under "Arithmetic before this registration" was done on paper. Results go in the addendum at the end.

Parents: [ADR 0219](0219-f134-registration-overset-coupling-of-a-moving-component-mesh.md) (F-134: the overset coupling, its
checks T1, T2, T1c, amendments 1 and 2, addendum 1 and question Q5), [ADR 0033](0033-wave20b-dynamic-mesh.md) (the ALE step),
[ADR 0129](0129-test-time-limit-jobs5-measurement-limit-audit.md) (test time limits, `limit_audit.py`),
[ADR 0228](0228-f306-registration-tetmesh-neighbour-link-panic-on-a-thin-box.md) (the form of this registration). Roadmap item
F-314 (depends on F-134).

Number: the lead gave 0232 to F-314. Branch `f314-overset-order` from main 0d250c75, worktree `w49i-work/f314`.

House rules (ADR 0130, ADR 0213, ADR 0219):

- limits are fixed here, before the code and the runs, and are not changed after a run; the limits of F-134 (T1, T2, T1c) are
  not changed and its known-miss tests stay as recorded;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, and the cause is
  diagnosed, not tuned away;
- a defective set-up gets a new corrected test by an amendment written and committed before the corrected run, and the old
  one stays as an ignored record;
- a corrective design that the diagnosis makes necessary is registered in an amendment committed before any run with it;
- defaults stay bit-identical: this registration changes no library code at all (tests only); a later corrective is opt-in;
- `crates/rcfd-api/tests/baseline_hashes.rs`, the hash tables and the goldens are not edited or re-recorded; no existing test
  is edited;
- each registered run is made once, and nothing is re-run to pick a result (a deterministic run that two tests share is the
  same run);
- no new third-party crate, no unsafe code, no commit trailers.

Clean-room: everything below is derived from the finite-volume definitions of the solver, Taylor's theorem and the
truncation error of the backward Euler difference. No paper, book, code or vendor document was read for this work.

## What F-314 asks

The roadmap entry (acceptance, quoted): "The Taylor-Green order (T1 0.747 against 1.7, T2 level 2 3.31 against 2, T1c 1.618
against 1.7, the F-134 ignored known-miss tests) reaches the registered limits, or the ADR names the donor-interpolation cause
with a number." Note: "From ADR 0219 question 4, recommendation taken; the known-miss tests stay as recorded" (the question
meant is Q5, whose recommendation (b) is an order check on a third level or on a norm that does not pick single fringe cells,
with the fringe-cell error printed).

**Reading (registered here).**

1. The three misses are separate numbers with possibly separate causes. ADR 0219 amendment 2 already assigns T1 and T2 at
   level 2 to the time error of the moving cells (a diagnostic, not a registered check); T1c is undiagnosed (Q5).
2. "Names the cause with a number" is read as: for each miss, a hypothesis below is confirmed by its registered measurement
   and its deciding number, both fixed here.
3. The acceptance words the cause as "the donor-interpolation cause". If the measurement shows that a miss has another cause
   (the time error for T1 and T2 at level 2), naming that cause with its number is read as meeting the second branch for that
   miss. This reading is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation); it is not a
   decision the maintainer made in person, and it is listed again as an open question in the addendum.
4. This is a diagnosis first. No library code is changed under this registration. If a confirmed cause calls for a
   corrective (for example a donor interpolation of higher order, opt-in), it is registered in an amendment before it runs.

## What was read (no run)

- ADR 0219 in full. `crates/rcfd-mesh/src/overset.rs` (`linear_weights`, `CellLocator`, `Overset::connectivity`),
  `crates/rcfd-fv/src/overset.rs` (`prepare`, `advance`, `piso`, `overset_pressure`), `crates/rcfd-fv/tests/overset_f134.rs`.
- The logs of the F-134 runs in `E:/projects/cfd/w49i-work`: `f134-t.log`, `f134-tc.log`, `f134-diag-tg.log`,
  `f134-diag-loc.log`. Numbers taken from them (they are also in ADR 0219):

| Run (conservative) | Level 1 | Level 2 | Order |
| --- | --- | --- | --- |
| background alone (TGB), `dt` proportional to `h^2` | `7.68e-4` | `1.93e-4` | 1.99 |
| overset at rest, largest error | `1.496e-3` (component fringe) | `3.512e-4` (component fringe) | 2.09 |
| overset moving, `dt` proportional to `h^2` (T1c), largest error | `1.0785e-3` (component fringe) | `3.514e-4` (component fringe) | 1.618 |
| the same, largest background error | `8.94e-4` | `2.27e-4` | 1.98 |
| overset moving, `dt = 0.0025` at both levels (T1) | `1.0785e-3` | `6.43e-4` | 0.747 |
| overset moving, level 2, `dt = 0.00125` | | `3.83e-4` | |

What the code read shows, relevant to the candidates:

1. The fringe rows are implicit in the momentum and the pressure systems with the stencils of the new pose (M5, M7), and after
   every corrector the fringe velocity is set to `sum_k w_k u_k` (M8). The donors and weights are those of the new pose; there
   is no explicit lag of the donor set by design.
2. Every stencil is the donor cell and its calculated face neighbours (five cells on these Cartesian parts when no neighbour
   is a fringe or hole cell). The weights reproduce linear fields only (M4), so the interpolation of a smooth field has a
   truncation error of second order whose constant depends on where the fringe centre falls in the stencil: it is zero when
   the centre coincides with the donor centre and largest near a donor corner.
3. The error of T is the largest pointwise error over the calculated and the fringe cells at one time (`t = 0.5`), at one
   pose, on two levels.
4. Read from the numbers above: the moving run at level 1 (`1.0785e-3`) is *smaller* than the run at rest (`1.496e-3`), while
   at level 2 both are `3.51e-4`. The order 1.618 is low because the level-1 number is small, not because the level-2 number
   is large.

## Arithmetic before this registration (disclosed, by hand)

- Interpolation truncation of M4 on a uniform donor grid of spacing `h`, fringe centre at a donor corner (offset `(h/2,
  h/2)` from the donor centre): the stencil points in units of `h` are `(-1/2, -1/2)`, `(1/2, -1/2)`, `(-1/2, 1/2)`, `(-3/2,
  -1/2)`, `(-1/2, -3/2)` with `omega = 2, 2, 2, 2/5, 2/5`; by symmetry the weights are `c, a, a, b, b` with `c = -4 b`, `a =
  (1 + 2 b)/2`, and the minimum gives `b = -1/28`, `c = 1/7`, `a = 13/28`. Then `sum w x^2 = sum w y^2 = 0.1786 h^2`, `sum w
  x y = -0.25 h^2`, and the truncation of a field `f` is `I = (1/2) [0.1786 (f_xx + f_yy) - 0.5 f_xy] h^2`. For the
  Taylor-Green velocity near the rim of the component (`|x - pi/2|` and `|y - pi/2|` up to about 0.6) the second derivatives
  are up to about 0.56 to 1, so `|I|` is up to about `0.24 h^2` per velocity component: `1.5e-3` at level 1 (`h = pi/40`),
  `3.7e-4` at level 2, `1.6e-4` at level 3. These are of the size of the whole measured error (`1.08e-3` to `1.50e-3`,
  `3.51e-4`). The smallest weight, `-1/28 = -0.036`, agrees with the `-0.034` that K3 of ADR 0219 printed.
- Backward Euler along the path of a moving cell: local truncation `(dt/2) |d^2 u / dt^2|` with `d/dt = partial_t + w . grad`
  on a cell of velocity `w`. With `|w|` from 0.6 to 1.2, `|grad grad u|` up to 1 and `partial_t u = -0.2 u`: `|d^2 u/dt^2|`
  about 0.6 to 2. Over `T = 0.5` at `dt = 0.0025` an undamped accumulation would be `4e-4` to `1.2e-3`; the measured level-2
  number at that step is `6.43e-4`. In the background `|d^2 u/dt^2| = 0.04`, thirty times smaller.
- T1 as registered needs `e(TG_2) <= 1.0785e-3 / 2^1.7 = 3.32e-4`, which is below the level-2 error without any time error
  (`3.51e-4`). So T1 at this pose cannot be reached by removing the time error alone.
- Velocity change that the pressure offset `delta` can cause in a cell next to the background fringe: at most about `rAU
  delta / h <= dt delta / h`; with `delta <= 2.9e-3`, `dt = 0.0025`, `h = pi/40` (level 1): `9e-5`, 9 percent of the error.

## Hypotheses, measurements and the numbers that decide

Notation. `TG_L` is the set-up of ADR 0219 at level `L` (background `40L x 40L` on `[0, pi]^2`, component `20L x 20L` on
`[-0.6, 0.6]^2`, hole `max(|x_ref|, |y_ref|) < 0.25`, the pose `tg_pose(t)`, `nu = 0.1`, the settings PS, conservative), run
to `t = 0.5` in `200 L^2` steps (`dt` proportional to `h^2`; levels 1 and 2 are the runs of T1c, level 3 with 1800 steps is
new). The error of a cell is `eps_c = |u_c - u_ex(x_c, t)| / e^{-2 nu t}`. Cell classes at a pose: CF component fringe, CC
component calculated, BF background fringe, BC background calculated; "adjacent" means a calculated cell with a fringe face
neighbour. For a fringe cell `c` with stencil `(k, w_k)`: the **truncation** `I_c = sum_k w_k u_ex(x_k) - u_ex(x_c)` and the
**carried donor error** `D_c = sum_k w_k (u_k - u_ex(x_k))`; by M8 `u_c - u_ex(x_c) = D_c + I_c` exactly, and the shares are
`s_I = (e . I)/|e|^2`, `s_D = (e . D)/|e|^2` with `e = D + I` (`s_I + s_D = 1`). "The 200 poses" are `tg_pose(k * 0.0025)`,
`k = 1..200`. The kinematic measurements use `u_ex(., 0)` and no flow solve.

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H1 | T1c: the largest error sits in a component fringe cell and is mostly the truncation `I` of the linear donor interpolation of the exact field, not an error of the solved field | at the end of `TG_1`, `TG_2` (and `TG_3`): the class of the cell of the largest error, and `s_I` there; `max_CF abs(I)`, `max_CF abs(D)` and their orders | confirmed: the cell is a fringe cell and `s_I >= 0.5` at levels 1 and 2; ruled out: `s_I <= 0.25` at both; otherwise mixed |
| H2 | T1c: the two-level order of a pointwise maximum at one pose cannot resolve 1.7 from 2, because the constant of `I` depends on the pose | kinematic: at each of the 200 poses the single-pose order `o_k = log2(max_CF abs(I) at level 1 / at level 2)`; its smallest, median and largest value, the fraction with `o_k < 1.7`, and `o_200` (the end pose) | confirmed: at least 10 percent of the poses give `o_k < 1.7` and the median is at least 1.8; ruled out: under 2 percent |
| H3 | the donor search or the weights lose the order (degraded stencils, a wrong donor, weights that are not second order) | G1 and G2 below | ruled out when G1 and G2 are met; confirmed when G2 is missed |
| H4 | the single fringe layer or the operators next to the fringe lose the order in the solved field | F3 below, and the class of the calculated cell of the largest error at each level | confirmed: F3 missed and that cell is adjacent to the fringe at levels 2 and 3; ruled out: F3 met |
| H5 | the pressure coupling (the offset `delta`, M7) causes the miss | the bound `b_L = dt max_steps abs(delta) / h_B` against the largest error `e_L` at each level, and the order of `max abs(delta)` (printed) | ruled out as the dominant cause: `b_L <= 0.25 e_L` at levels 1 to 3 and F3 met; otherwise open |
| H6 | T1 and T2 at level 2: the excess at `dt = 0.0025` is the first-order time error of backward Euler along the paths of the moving cells (ADR 0219 amendment 2), not a lag of the moving donor set | level 2 with 200, 400 and 800 steps: the difference fields `d_200 = u_200 - u_800` and `d_400 = u_400 - u_800` at `t = 0.5` (same cells, same pose); F5 and F6 below; the class of the cell of `max abs(d_200)`; the speed test: the same with the motion at half speed (`tg_pose(t/2)`), the ratio `rho = max abs(d_200)` at full speed over that at half speed, against the kinematic ratio `rho_BE` of `max` over the component cells and the steps of the second difference of `u_ex` along the cell paths (a lag that is linear in the speed gives `rho = 2`) | confirmed: F5 and F6 met, the cell is a component cell, and `abs(rho / rho_BE - 1) <= 0.25`; if `abs(rho_BE / 2 - 1) <= 0.25` the speed test cannot tell the two apart and this is said; ruled out: F5 missed or `abs(rho/2 - 1) < abs(rho/rho_BE - 1)` with a discriminating `rho_BE` |
| H7 | the error norm or the reference of the check is at fault | the reference is the closed form (no measurement); the norm is covered by H2 and by F2 and F4 | F2 and F4 met while T1c is missed at the same runs: the pointwise maximum at one pose is the weak part of the check |

For T2 at level 2 and T1 the numbers to report under H6 are the time error at `dt = 0.0025`, `(4/3) max abs(d_200)`, and the
largest step at which `e(TG_2) <= 2 e(TGB_2)` would hold on that estimate.

## Registered checks

New test file `crates/rcfd-fv/tests/overset_f314.rs`; release. No library code and no existing test is edited. The tests
that take more than about two minutes are `#[ignore]` with their run time as the reason and are run once for the addendum.

### G. Kinematic (no flow solve)

| Id | Check | Limit |
| --- | --- | --- |
| G1 | levels 1, 2, 4 at the 200 poses: every stencil has five cells; the donor (first cell) contains the fringe centre (`containment_margin >= -1e-10` of the cell size); every weight in `[-0.05, 1 + 1e-12]` | exact; as stated |
| G2 | `I_sup(L)`, the largest `abs(I_c)` over all fringe cells and the 200 poses, levels 1, 2, 4: `log2(I_sup(1)/I_sup(2))` and `log2(I_sup(2)/I_sup(4))` | `>= 1.8` each |
| G3 (negative control) | G2 with every stencil replaced by its donor cell with weight 1 | both orders `<= 1.3` |

Printed, no limit: `I_sup` per class; the numbers of H2.

### F. Flow

| Id | Check | Limit |
| --- | --- | --- |
| F1 | at the end of `TG_1`, `TG_2`, `TG_3`: `abs(u_c - sum_k w_k u_k)` over every fringe cell (M8, on which the split `D + I` rests) | `<= 1e-12` |
| F2 | order of the root mean square of `eps_c`, weighted with the cell volumes, over the non-hole cells at `t = 0.5`: levels 1 to 2 and 2 to 3 | `>= 1.7` each |
| F3 | order of `max eps_c` over the calculated cells (CC and BC) at `t = 0.5`: levels 1 to 2 and 2 to 3 | `>= 1.7` each |
| F4 | order of the window maximum: the largest `max eps_c` over the non-hole cells over the steps with `t` in `[0.25, 0.5]`: levels 1 to 2 and 2 to 3 | `>= 1.7` each |
| F5 | level 2: `R = max_CC abs(d_200) / max_CC abs(d_400)` (first order in `dt` gives `(1 - 1/4)/(1/2 - 1/4) = 3`) | in `[2.5, 3.5]` |
| F6 (control) | level 2 with the component held at `tg_pose(0)`, 200 and 800 steps: `max abs(d_200)` over the non-hole cells against that of the moving run | `<= 0.2` of it |
| N5 (negative control) | levels 1 and 2 with every stencil replaced by its donor cell with weight 1 after every `prepare` (200 and 800 steps): the orders of F2, F3 and F4 between the two levels | `<= 1.3` each |

Printed, no limit: per level and class the largest and the root-mean-square error and their orders; the T1c number at levels
2 to 3 (`log2(e(TG_2)/e(TG_3))` with the pointwise maximum at `t = 0.5`); the cell, class and position of every maximum;
`s_I`, `max abs(I)`, `max abs(D)`; the range of `delta` and `b_L`; `rho`, `rho_BE`; the run times.

T1, T2 at level 2 and T1c keep their limits and their ignored known-miss tests in `overset_f134.rs`; they are not run again
here except where a run of this file is the same deterministic run (levels 1 and 2 with 200 and 800 steps).

### Mutations (scratch copy, not committed)

Each is applied alone to a `git archive` copy of the final tree, the named test is run once and must fail; the numbers go in
the addendum. `scripts/limit_audit.py` is run on the new test file and every hit is read.

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | `linear_weights`: inverse-distance weights (only `sum w = 1`) | G2 |
| MU2 | `rcfd_fv::overset`, `piso`: the fringe velocities are not set from their stencils after the corrector (M8) | F1 |

A corrective registered by an amendment brings its own mutations.

### Classes

A miss of G1, G2 or F1 to F6 is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. A
control (G3, F6, N5) that does not hold, or a mutation that does not make its check fail, is a defective set-up: corrected by
an amendment before any re-run, never counted as a pass. The hypotheses are decided by the numbers of the table above only.

## Predictions (mine, before any run)

H1 confirmed (`s_I` above 0.5; `max_CF abs(I)` about `1e-3` and `3e-4`). H2 confirmed, with single-pose orders between
about 1.4 and 2.5. G2 near 2.0, G3 near 1.0. F2 near 2.0 (the background root mean square converged at 1.98). F4 near 2.
F3 is the least certain: the error of the calculated component cells was never printed; I expect 1.8 to 2.1. F5 near 3, F6
well under 0.2. H5 ruled out. H6 confirmed, unless `rho_BE` turns out too close to 2 to discriminate. The T1c number at
levels 2 to 3 may again fall below 1.7 or rise above 2: it is a single-pose sample.

## Decision rule

- **done**: (a) T1, T2 at level 2 and T1c reach their registered limits (not expected: no library change is registered), or
  (b) each of the three misses has a hypothesis confirmed by its deciding number (T1c: H1 or H2; T1 and T2 at level 2: H6,
  under reading 3), every check G1, G2, F1 to F6 is met, the controls G3 and N5 held and the mutations failed their checks.
  Under (b) the note says that the misses are explained and not removed, and names the follow-up.
- **partial**: a miss is left without a confirmed cause (ruled out, mixed or not discriminated), or a check of this
  registration is missed, or a control or mutation did not hold and was not corrected.
- **todo**: no registered measurement could be run.

## Limits of this registration

- The diagnosis is made on the Taylor-Green set-up of ADR 0219 only (uniform Cartesian parts, one component, `nu = 0.1`,
  `t = 0.5`); nothing is concluded for the cylinder case TR beyond what ADR 0219 measured.
- Level 3 is the finest flow level; an asymptotic order is not shown by three levels. Level 4 is kinematic only.
- H6 separates a motion-induced first-order time error from none (F5, F6) and, only as far as `rho_BE` differs from 2, the
  backward Euler error from a lag that is linear in the speed. No run of the moving component without the overset coupling
  exists in this set-up, so the coupling cannot be switched off as a control.
- The exact solution is compared at cell centres; the difference between a cell mean and the centre value is of second order
  and the same in the reference runs (not measured).
- Not delivered under this registration: any change of the interpolation, the time scheme (BDF2 is refused on the overset
  path, ADR 0219 S2) or the fringe width.

## Open questions

Listed with options and a recommendation in the addendum, after the results.

## Amendment 1 (2026-11-12, written after the registered runs of G, F, N5 and H6 and before any further run)

The registered tests were committed (`overset_f314.rs`, compiled, formatted and linted without running) and then run once
each, release, on the library of main 0d250c75: the kinematic test (9.8 s), the three-level test (518 s), N5 (68 s) and the
time-error test (263 s). All numbers are in addendum 1. Met: G1, G2, G3, F1 (three levels), F2 and F4 (both pairs), F3 at
levels 1 to 2, F5, F6, and the calculated-maximum and window-maximum parts of N5. Three results need this amendment:

| Id | Limit | Measured |
| --- | --- | --- |
| F3, levels 2 to 3 | `>= 1.7` | `1.580` (calculated maximum `2.096e-4` at level 2, `1.104e-4` at level 3, both in a component calculated cell near `(1.10, 1.33)` that is not adjacent to the fringe) |
| N5, root mean square (negative control of F2) | `<= 1.3` | `1.463` |
| H6, speed test | `abs(rho / rho_BE - 1) <= 0.25` | `rho = 2.292`, `rho_BE = 4.783` (root mean square `3.700`): `0.52`; `abs(rho / 2 - 1) = 0.146` |

**N5 (root mean square).** With the donor cell alone the order of the root mean square is `1.463`, not below `1.3`: my
prediction of the control was wrong (the first-order error is concentrated in the fringe cells, and the root mean square
over all cells is dominated by the calculated cells, whose error also carries a second-order part). The control did not
hold at its registered limit and is kept as the ignored test `n5_root_mean_square_control_known_miss` with the limit intact.
No corrected control is run: the measured `1.463` lies below the limit `1.7` of F2, so the run shows that F2 fails for a
first-order interpolation; this is reported as that reading and not as a pass of N5.

**F3 at levels 2 to 3** is kept as the ignored test `f3_calculated_maximum_levels_2_to_3_known_miss` with the limit intact.
By the registered rules H4 is neither confirmed (the cell is not adjacent to the fringe) nor ruled out, and H5 stays open
(`b_L` is `0.085`, `0.062`, `0.021` of the error, under `0.25`, but F3 is missed). The miss is diagnosed by the measurements
below. What is known before them: the time-error test puts a first-order time error into the same component cells at
level 2 (`max abs(d_200) = 4.49e-4`, `max abs(d_400) = 1.47e-4` at `dt = 0.0025` and `0.00125`, so about `1.5e-4` at the
`dt = 6.25e-4` of the level-2 run), of the size of the whole calculated maximum there (`2.1e-4`). With `dt` proportional to
`h^2` that part is of second order, so the calculated maximum is the sum of two second-order parts of similar size.

| Id | Hypothesis for the F3 miss | Measurement | Deciding number |
| --- | --- | --- | --- |
| H8a | the static overset coupling (interpolation, `delta`) loses the order of the solved field | the component held at `tg_pose(0)`, levels 1, 2, 3 with `200 L` steps: the order of the calculated maximum, levels 2 to 3 | confirmed: `< 1.7`; ruled out: `>= 1.7` |
| H8b | the calculated maximum of the moving run is the sum of a spatial and a time part that are each of second order; the 1.58 is their mix on three levels | moving, levels 1, 2, 3, each with `100 L^2` and `200 L^2` steps: the spatial part, the error of the field `2 u_(200 L^2) - u_(100 L^2)` (extrapolated to `dt = 0`), and the time part `u_(100 L^2) - u_(200 L^2)`; the order of the largest spatial part and of the largest time part over the calculated cells, levels 2 to 3 | confirmed: both `>= 1.7`; ruled out: either `< 1.7` (then that part is named with its order) |
| H8c | the end time is an unlucky sample of a calculated maximum that fluctuates with the pose | the window maximum (steps with `t` in `[0.25, 0.5]`) of the largest error over the calculated cells, levels 2 to 3 | confirmed: `>= 1.7`; ruled out: `< 1.7` |

**H6.** By its registered rule H6 as worded is ruled out: the time error is first order in `dt` (F5: `R = 3.06`), induced by
the motion (F6: `0.027` at rest) and largest in a component calculated cell, but it grows with the speed by `2.29`, close to
linear, not by the `4.78` (`3.70` in the root mean square) of the backward Euler truncation along the cell paths. A fit of
`a s + b s^2` to the two speeds gives about two thirds linear at full speed. The code read named no lag of the donor set.
One lag that is linear in the speed exists in the step itself, with or without overset: the convecting flux of the momentum
predictor is the flux of the old time, taken through the faces at their old position and orientation (PISO, one outer
iteration: `phi_conv = phi_old - mesh flux`), which on a face that moves and turns differs from the flux at the new
position by about `dt [(w . grad) u . S + u . (Omega x S)]`. New hypotheses, decided on a single moving mesh without any
overset coupling:

Set-up **SM**: the component mesh of level 2 alone (`rectilinear_2d` 40 x 40 on `[-0.6, 0.6]^2`, depth 0.1) moved rigidly
by `tg_pose(s t)` (`s = 1` or `1/2`) with the library step `PimpleSolver::step_ale_with_walls` (`ale::move_mesh`, no wall
gradient), the settings PS, the exact velocity at the new time at the face centres of its four sides, `p` zero gradient with
the pin at cell 0, start from the exact fields, to `t = 0.5` in 200 and in 800 steps; `d_200` as before, over all cells.

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H6a | the first-order time error of the moving cells belongs to the ALE step of the library on a moving mesh, not to the overset coupling (a mesh without donors shows it too) | SM, `s = 1` and `1/2`, one outer iteration: `max abs(d_200)` and `rho_SM` | confirmed: `max abs(d_200)` of SM within a factor 2 of the overset value `4.49e-4` and `abs(rho_SM / 2.292 - 1) <= 0.25`; ruled out: `max abs(d_200)` of SM under a quarter of the overset value |
| H6b | its part that is linear in the speed is the lagged convecting flux of the single outer iteration | SM, `s = 1`, with three outer iterations (`n_outer = 3`, no under-relaxation): `q = max abs(d_200)` with three over that with one; and `rho` with three outer iterations at `s = 1` and `1/2` | confirmed: `q <= 0.7` and `rho` nearer to `rho_BE` (root mean square, `3.70`) than to 2; ruled out: `q >= 0.9` |

Checks with limits (new): **F7** SM, every step of every run: the continuity reported by the library `<= 1e-9`. **F8** SM
with one outer iteration and 800 steps at `s = 1`: the largest error `max eps_c` at `t = 0.5` `<= 1e-3` (the set-up is a
sound Taylor-Green run: the overset run of level 2 has `3.5e-4`).

Tests: the gate-run test of levels 1 and 2 is unchanged. The three-level test keeps F1, F2 and F4 and prints F3. New:
`f3_calculated_maximum_levels_2_to_3_known_miss` and `n5_root_mean_square_control_known_miss` (ignored, limits intact),
`a1_f3_miss_spatial_and_time_parts` (ignored, slow; H8a to H8c; it repeats the level runs, which are deterministic) and
`sm_single_moving_mesh_time_error` (H6a, H6b, F7, F8). The window maximum over the calculated cells is added to the run
record; nothing else of the registered runs changes. Each new run is made once.

Predictions (mine): H8a ruled out (at rest the order is near 2); H8b confirmed; H8c uncertain. H6a confirmed; H6b
confirmed with `q` near 0.4 to 0.6. If SM fails to run soundly (the exact velocity on a closed moving boundary is not
discretely free of net flux), this is reported and H6a and H6b stay open.

The decision rule is unchanged. Under it F3 at levels 2 to 3 is a missed check of this registration, and T1 and T2 at
level 2 need H6a (the cause named and confirmed in place of H6) to count as named.

## Amendment 2 (2026-11-12, written after the run of SM and before any corrected run; the run of H8a to H8c has not started)

SM ran once (57 s). F8 met (`6.41e-4`). **F7 missed: the largest continuity is `3.07e-5` against `1e-9`.** This is the
defect that amendment 1 named beforehand: the exact velocity at the face centres of the four moving sides has a net flux
that is not zero (midpoint rule on a closed boundary; the imbalance is `3.07e-5` of the largest face area, `9e-8` in flux
units, `2e-7` of `U` times the boundary area), the pressure equation of the closed mesh is then not consistent, and the soft
pin takes the defect in the pin cell. As amendment 1 says, the set-up is defective and H6a and H6b are not decided on it.
The numbers it printed are disclosed here and are not used for the decision: `max abs(d_200)` `3.726e-4` with one outer
iteration (`0.829` of the overset value), `rho_SM = 2.258`, `q = 0.153`, `rho` with three outer iterations `3.877`.

F7 stays as registered in the ignored test `sm_exact_boundary_velocity_known_miss` (limit intact).

Corrected set-up **SMc**, registered here before it runs: SM with the boundary velocity made discretely free of net flux.
After the exact velocities `v_f` of the boundary faces of the four sides are set on the new geometry, with `Q = sum_f v_f .
S_f` and `A = sum_f abs(S_f)` over those faces, every one gets `v_f - (Q / A) n_f` (`n_f` the outward unit normal). The
correction is the same for every step size up to the motion, and of the size `Q / A` (about `2e-7`). Nothing else changes.

| Id | Check | Limit |
| --- | --- | --- |
| F7c | SMc, every step of every run: the continuity reported by the library | `<= 1e-9` |
| F8c | SMc, one outer iteration, 800 steps, `s = 1`: the largest error at `t = 0.5` | `<= 1e-3` |
| F9c (set-up) | SMc: the largest `abs(Q) / A` over the steps of every run (the size of the correction) | `<= 1e-5` |

H6a and H6b are decided on SMc with the deciding numbers of amendment 1, unchanged. SMc is run once. Prediction (mine):
F7c near `1e-13`; the numbers of H6a and H6b within a few percent of those SM printed.

## Amendment 3 (2026-11-12, written after the runs of SMc and of H8a to H8c and before any further run)

SMc ran once (45 s): F7c `1.1e-13`, F8c `6.41e-4`, F9c `1.9e-7`, all met; H6a and H6b are confirmed by their deciding
numbers (addendum 1). The test of H8a to H8c ran once (780 s):

| Id | Deciding number | Measured (levels 2 to 3) | Verdict |
| --- | --- | --- | --- |
| H8a | at rest, order of the calculated maximum `< 1.7` | `1.336` (`2.504e-4`, `1.457e-4`; levels 1 to 2: `1.889`) | confirmed |
| H8b | moving, spatial part and time part each `>= 1.7` | `1.834` and `2.038` (levels 1 to 2: `1.982`, `2.049`) | confirmed |
| H8c | moving, window maximum over the calculated cells `>= 1.7` | `1.478` | ruled out |

So the moving calculated maximum is the sum of two parts that each keep an order above 1.7, the end time is not an unlucky
sample, and the component held at rest loses more order between levels 2 and 3 than the moving one. At rest the largest
truncation over the component fringe keeps its order (`1.707e-3`, `4.619e-4`, `2.046e-4`: 1.89 and 2.01), but the root
mean square of the error of the calculated component cells is `3.22e-4`, `9.37e-5`, `6.93e-5`: over `h^2` it is `3.2e-4`,
`3.7e-4`, `6.2e-4` (levels 1, 2, 3), a coefficient that jumps at level 3 instead of an error that stops falling.

What differs between the levels at rest, read from the set-up: the component is not turned (`angle = 0`), so its grid is
parallel to the background grid, and every fringe cell along one side falls at the same height in its donor row. That
height (the phase of the fringe in its donor cells) is fixed by the corner of the component in units of the background
cell, `(pi/2 - 0.3 - 0.6) / (pi / (40 L)) = 8.541 L`: its fractional part is 0.54, 0.08 and 0.62 at levels 1, 2 and 3, and
those of the other sides differ likewise. The truncation of the hand arithmetic is zero at a donor centre and largest at
a donor corner, so its mean along a side, which is the boundary data error of the component, has a coefficient that
changes from level to level by a factor of the order of two.

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H9 | the loss of order at rest (H8a) is the level-dependent phase of the fringe in its donor cells (a coefficient of the second-order truncation that differs between the levels), not a loss of order of the coupling | set-up **RL**: as the run at rest, but with the phase locked: the component has the half width `7.5 H` and the hole the half width `3.1 H` (`H = pi/40`, the background cell of level 1), its corner lies on the background node `(9 H, 9 H)` (pose: no rotation, translation `(16.5 H, 16.5 H)`), so its cells are 3/4 of the background cells and every fringe centre falls at the same place of its donor cell at every level; levels 1, 2, 3 with `200 L` steps; the orders of the calculated maximum and of the root mean square of the calculated component cells, levels 2 to 3 | confirmed: both `>= 1.7`; ruled out: the calculated maximum `< 1.7` (then the static coupling loses order on these levels and the cause stays open) |

Printed with it (kinematic, no limit): `kappa_L`, the root mean square of the truncation over the component fringe over
`h_B^2`, for the registered rest pose and for RL at levels 1 to 3 (prediction: it varies by more than 20 percent between
the levels at the registered pose and by less than 5 percent in RL).

Test `a3_rest_with_the_fringe_phase_locked` (ignored, slow, run once). The run helper takes the composite and the pose as
arguments since this amendment; the registered runs are unchanged. Prediction (mine): H9 confirmed, with orders between
1.8 and 2.1. The decision rule is unchanged.

## Amendment 4 (2026-11-12, written after the run of RL and before any further run; the last diagnostic of this registration)

RL ran once (147 s). `kappa_L` is `0.128`, `0.120`, `0.145` at the registered rest pose (a spread of 20 percent) and
`0.0887`, `0.0891`, `0.0892` in RL (under 1 percent): the phase is locked. With it locked the order of the calculated
maximum at levels 2 to 3 is `1.730` (at the registered rest pose `1.336`), of the largest error `1.788` (`1.264`), of the root
mean square `1.898` (`1.727`). But the root mean square over the calculated component cells is `1.549e-4`, `6.39e-5`,
`3.98e-5`: orders `1.277` and `1.166`. **H9 is neither confirmed nor ruled out** by its rule (the calculated maximum is at
`1.730`, above 1.7; the component root mean square is below it).

The three RL numbers fit `a (h/H)^2 + b (h/H)` with `a = 5.42e-5`, `b = 1.007e-4` from levels 1 and 2; that fit gives
`3.96e-5` at level 3 against the measured `3.98e-5`. So with the phase locked the error of the calculated component cells
has a clean first-order term of the coefficient `1.0e-4` beside its second-order term; it lies under the second-order
error of the background (`5.6e-4`, `1.41e-4`, `6.5e-5` in the root mean square) up to level 3. Two candidates are tested
before this registration closes; no further diagnostic is added after them.

| Id | Hypothesis | Measurement | Deciding number |
| --- | --- | --- | --- |
| H10 | the first-order term is a time error: the runs at rest take `200 L` steps, so `dt` is proportional to `h` | RL at level 3 with 1200 steps against the 600 steps of the run above: the root mean square over the calculated component cells | confirmed: the value with 1200 steps is at most 0.7 of that with 600; ruled out: at least 0.9 of it |
| H11 | the first-order term sits in the calculated component cells next to the fringe (the operators there see the interpolated values), not in the interior | RL, levels 1 to 3 (`200 L` steps): the calculated component cells by ring (ring 1 has a fringe face neighbour, ring `n + 1` a face neighbour in ring `n`); the root mean square of the error over rings 1 and 2 and over the rings from 4 on, and their orders at levels 2 to 3 | confirmed: the order over the rings from 4 on is `>= 1.7` and that over rings 1 and 2 is below it; ruled out: the order over the rings from 4 on is `< 1.5` (the term fills the interior) |

Test `a4_first_order_term_of_the_component_at_rest` (ignored, slow, run once; it repeats the three RL runs, which are
deterministic). Prediction (mine): H10 ruled out (the run at rest of the registered pose changed by `1.2e-5` at most
between 200 and 800 steps at level 2); H11 uncertain. Whatever the result, the first-order term goes into the addendum as a
finding with its number, and its removal is not part of this registration.
