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
