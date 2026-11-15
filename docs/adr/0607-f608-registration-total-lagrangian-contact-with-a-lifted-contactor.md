# ADR 0607: F-608 registration: total Lagrangian contact with a lifted contactor, the convexified tangent on a refused pivot and a line search on the energy

Status: **REGISTERED 2026-11-15, written before any code of F-608 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text the base `c64046db` was not changed. Three probes were made first with the existing API only (an
uncommitted test file in the worktree, since removed and kept as `w49i-work/f608-scratch/zz_probe_f608.rs`; logs
`probe1.log`, `probe2.log`, `probe3.log` there); they are quoted under "Arithmetic and probes done beforehand" and are not
registered runs. No library line was written before this commit. Results go in the addenda at the end; nothing above them is
changed after a run.

Roadmap item F-608 (deps F-427, F-600), the follow-up proposed by
[ADR 0519](0519-f600-registration-contact-newton-order-reading-for-short-solves.md) (its remaining question 1). Number 0607
given by the lead. Branch `f608-work` from `c64046db` (main), worktree `w49i-work/f608`, scratch `w49i-work/f608-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test or
ignored record edited; no new dependency, no unsafe code, the crate graph unchanged; clean-room; `docs/ROADMAP.md` and
`docs/roadmap.json` not edited. Every choice the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation) and marked so.

## What F-608 asks

The acceptance, verbatim, binding:

> Every limit not stated here is registered in the ADR before the run. The total Lagrangian contact driver of ADR 0507
> converges the punch of T2 with an initial gap of +3e-4 at the four nodes with x > 1.0, and the reading Q(10, 1e-12) of ADR
> 0519 holds on it (limits registered).

Clauses: **c1** every other limit registered here; **c2** the total Lagrangian contact driver converges the T2 punch with the
initial gap `+3e-4` at the four contactor nodes with `x > 1.0`; **c3** Q(10, 1e-12) of ADR 0519 D1 holds on that run. The
title adds the means: a convexified tangent on a refused pivot.

## What was read

ADR 0507 (D1 to D7, amendment 2: the T2 and I diagnoses, the fallback `eps G G^T` of the linear body; amendment 3), ADR 0519
(D1 the reading, D3 the models, probe 2 and remaining question 1, which recorded "singular tangent: pivot .. at node 67
component 2" for every positive initial gap and left it undiagnosed), ADR 0570 (F-639: the opt-in
`ContactOptions::curvature_fallback`, the trait methods `has_fallback_tangent`/`add_fallback_tangent`, the fallback in
`Problem::attempt_with`), ADR 0542 (P: the refused pivot of the total Lagrangian hub, the pivot tests of both factors). Code:
`crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions`, `Contribution`, `contribute_with`, `contribute_split`,
`Problem::attempt_with`, `factor_solve`, `Equilibrium`, `LoadStep`), `crates/rcfd-fem/src/contact.rs` (`ContactOptions`,
`ContactSet::{add, record, force_and_tangent, add_fallback_tangent}`, `solve_with`, `NewtonSolve`),
`crates/rcfd-fem/src/arclength.rs` (`ArcLengthOptions::check`). Tests: `sa_contact_order.rs` (the T2 punch builder and the
reader of ADR 0519), `sa_contact_tl_fallback_f639.rs` (the cube FD and its spring contribution).

## Arithmetic and probes done beforehand (disclosed)

Nodes. The plate `hex_box([0,0,0], [2, 0.5, 0.25], [8, 2, 2])` has 81 nodes (0 to 80); the punch's bottom nodes are 81 to 88,
`81 + i + 4 j` with `x = 0.8 + 0.4 i/3`, `y = 0.2 j`; the lifted ones (`x > 1.0`) are 83, 84, 87, 88. Node 67 is the plate's
top node at `(1.0, 0.25, 0.25)`. Node areas: a corner of the punch's bottom `(0.4/3)(0.2)/4 = 6.67e-3`, the others
`1.33e-2`; node penalties `eps A_s = 5.33` and `10.7`.

**Probe 1** (`probe1.log`; `sa_contact_order.rs`'s punch with `ContactOptions::curvature_fallback`, two steps, `p = 0.01`):

| initial gap | default | `curvature_fallback(true)` |
|---|---|---|
| `+3e-4` | `singular tangent: pivot -1.447e1 at node 67 component 2 (diagonal 7.97)`, load step 1, augmentation 0 | active set cycled 4 times (more than 3) at Newton iteration 12, nodes switching `[82]` |
| `+1e-4` | (ADR 0519: the same refusal) | `without the curvature part: singular tangent: pivot -2.6e-4 at node 84 component 0` |
| `+1e-3`, `+5e-3` | (ADR 0519: the same) | cycled at iteration 6 and 7, nodes `[83, 84, 87, 88]` |
| `+2e-2` | (ADR 0519: the same) | `without the curvature part: .. pivot -2.3e-3 at node 82 component 0` |
| `0`, `-3e-4` | converge (ADR 0519 M0, M3) | converge, bit for bit the default numbers, no fallback iteration |

**Probe 2** (`probe2.log`; a recording contribution after the contact set, the gaps of every evaluation recomputed with
`ContactSet::evaluate`). Default, `+3e-4`: at `u = 0` the four nodes with `x <= 0.933` are active (gap 0, `lambda = 0`: active
by `>=`, ADR 0507 D2), the four lifted ones are not. The first correction moves the plate's centre `u_z(67) = -1.93e-2` and the
punch's lifted nodes `-3.0e-2 .. -3.9e-2`: they enter the plate by `9.8e-3 .. 2.13e-2`, 33 to 71 times the gap; their forces
`t = eps A_s |g|` are `0.105 .. 0.114`, about 1000 times the equilibrium forces (`3.7e-5 .. 1.4e-4`, probe 3). The tangent of
that iterate fails the pivot test (`-14.5` at node 67, z). With the fallback the same first iterate is taken, the second
iteration factors `K_e + eps G G^T`, and the iterates then alternate: the active sets of the first 12 evaluations are `{81, 82,
85, 86}`, all, `{81, 82, 85, 86}`, `{82, 83, 84, 86, 87, 88}`, `{81, 85, 86}`, all, `{85, 86}`, .., and each set without the
lifted nodes is followed by an overshoot of the same kind (the lifted nodes 1.8e-2 to 4.2e-2 into the plate); by the twelfth
evaluation the gaps are within `1.4e-5` but D6 has counted 4 returns to an earlier set.

**Probe 3** (`probe3.log`; fallback and `max_cycles(100)`, `max_iterations(100)`, a probe only, not a proposal): converges.
First solve 16 iterations (8 with the fallback), residuals `1.0, 1.0, 0.98, 1.0, .., 0.127, 0.998, 2.1e-2, 2.1e-3, 1.1e-9,
4.7e-14`; the later solves 3 to 4 iterations, Newton's; all 8 nodes active at the end with forces `3.70e-5 .. 1.44e-4`, their
sum `8.00e-4`, the pressure resultant `p A = 0.01 x 0.4 x 0.2 = 8e-4`.

## Diagnosis (from the probes and first principles)

1. **The tangent at `u = 0` is regular.** The four supported nodes lie on two rows 0.133 apart; with the punch's top `u_x =
   0` and `u_y = 0` on `ymin` every rigid motion is held, and the factor accepts it. The lifted nodes carry neither force nor
   stiffness (`K_c = 0` for an inactive node, ADR 0507 D2), so the first correction is the minimiser of the quadratic model of a
   punch standing on two close rows of springs under a plate that sags `1.9e-2`: the punch tips about them and its lifted
   half goes `2e-2` into the plate.
2. **The refused pivot is at the second iterate**, from the curvature part `- t d2g/dq2` with forces 1000 times their
   equilibrium (the mechanism of ADR 0507 amendment 2, I and T2): the exact tangent of the penalty energy is indefinite there.
   ADR 0519's "singular tangent at node 67" is this refusal; it is not a singular matrix from inactive nodes (the inactive
   nodes leave no free mode).
3. **The convexified tangent is necessary but not sufficient.** ADR 0570's `eps G G^T` (`ContactOptions::curvature_fallback`)
   makes the second iteration factor; the full Newton step then overshoots the other way (the lifted nodes leave), and the next
   tangent, again without them, overshoots as in 1. The iteration is a semi-smooth Newton iteration without a globalisation;
   its active set returns to earlier sets and D6 stops it (probe 1, `+3e-4`), and at some gaps an extreme iterate makes even
   the element tangent indefinite ("without the curvature part", `+1e-4`, `+2e-2`). Given enough iterations it converges
   (probe 3): the difficulty is the length of the step, not its direction.
4. **The remedy is a line search on the energy.** During one Newton solve of the contact driver the multipliers are fixed, the
   loads are dead and the material is hyperelastic (St Venant-Kirchhoff in total Lagrangian form, ADR 0404), so the total
   potential `Pi(u) = W(u) + sum_s W_s(u) - lambda f . u` exists, with `W_s = max(0, lambda_s - eps_s g_s)^2/(2 eps_s)` (ADR
   0507 D2); it is `C^1` and its gradient is `-r(u)`, the driver's residual. A correction `d` with `K d = r` and `K` accepted by
   the factor (every pivot positive, so `K` is positive definite) is a descent direction: `phi'(0) = -d . r = -d . K d < 0`
   for `phi(alpha) = Pi(u + alpha d)`. Damping the step to where `phi'` has fallen enough keeps the lifted half from being
   thrown 70 gaps into the plate. Near the solution the full step passes the test (below), so the converged iterations stay
   Newton's.

## Design

### D1. The line search (`crates/rcfd-fem/src/nonlinear.rs`)

Line searches for the Newton iteration of finite element equations: Matthies and Strang, *Int. J. Numer. Meth. Engng* 14
(1979) 1613; Crisfield, *Non-linear Finite Element Analysis of Solids and Structures*, vol. 1, Wiley 1991, chapter 9 (both
from memory; the algorithm below is derived here, not taken from either). Only residuals are needed, not the energy:

    s(alpha) = phi'(alpha) = - sum over the free equations of d_i r_i(u + alpha d).

`NewtonOptions` (`#[non_exhaustive]`) gains `pub line_search: bool`, **default `false`**, and the builder `line_search(bool)`.
In `Problem::attempt_with`, with the option, without a load-factor control, without the unsymmetric tangent, after the
correction `d` of iteration `k` has been computed (by the exact tangent or the fallback of ADR 0570) and only when every value
of the prescribed increment of that iteration is `0.0` (else `alpha = 1`, see Limits):

1. `s_0 = - sum d_i r_i(u)` from the iteration's own residual (no evaluation). If `s_0` is not `< 0` (not finite, or not a
   descent direction), `alpha = 1`.
2. Trial `alpha = 1`: `s_1`. A **trial evaluation** is the internal force at `u + alpha d` (the same `force_and_tangent` of the
   driver from the committed states; the fixed equations at the values they take after the step) plus every contribution's
   `add_trial_force` (D2), and `s` from the residual on the free equations. A failed evaluation (any error) or a non-finite `s`
   counts as an overshoot (`s = +inf`).
3. **Accept** `alpha` when `s(alpha) <= eta |s_0|`, `eta = 0.5`. For `alpha = 1` this accepts every undershoot (`s_1 < 0`) and
   every overshoot whose line minimum lies in `[1/(1 + eta), 1/(1 - eta)] = [2/3, 2]` for a quadratic `phi` (`s(alpha) = s_0 (1 -
   alpha/alpha*)`, `|s(1)| <= eta |s_0|` iff `|1 - 1/alpha*| <= eta`). For `alpha < 1` the test is `|s(alpha)| <= eta |s_0|`.
4. Otherwise the root of `s` is bracketed in `[a, b] = [0, 1]` with `s_a = s_0 < 0`, `s_b = s_1 > 0`; up to **8** further
   trials by the Illinois variant of regula falsi (Dowell and Jarratt, *BIT* 11 (1971) 168, from memory: the secant point of
   the bracket, and the value kept at an end retained twice in a row halved): `alpha = (a s_b - b s_a)/(s_b - s_a)` when `s_b`
   is finite, else `(a + b)/2`; the new point replaces `b` if `s > 0` or the evaluation failed, `a` otherwise; accepted by the
   test of 3. When the 8 trials are spent the last trial's `alpha` is taken. `alpha` stays in `(0, 1)`.
5. `u` moves by `alpha d` on the free equations (with `alpha = 1` by `u_i += d_i`, exactly as without the option); the
   energy of the iteration is unchanged in definition (`|d . b|` of the full correction, ADR 0404 D5); `alpha` is recorded.

Why `eta = 0.5`: derived in 3 (the full step is kept whenever it is within a factor 2 of the line minimum of a quadratic
model); near the solution `s_1/s_0` is of the order of the residual ratio `r_{k+1}/r_k`, far below 0.5, so a converging Newton
iteration takes `alpha = 1` and keeps its bits. Why 8 trials: the Illinois method converges superlinearly (order about 1.44)
and is exact in one step for a linear `s`; 8 bounds the cost of a step at 9 extra evaluations. Both fixed here, not searched.

Records: `LoadStep`, `Equilibrium` (and the internal `Attempt`) gain `step_lengths: Vec<f64>`, one `alpha` per correction with
the option, empty without it.

Refusals (key `"line_search"`, `FemError::InvalidValue`): `NewtonOptions::check` with `unsymmetric` (the descent argument needs
a symmetric positive definite factor); `ArcLengthOptions::check` with `newton.line_search` (the controlled correction is not
given it, it would be ignored); `ContactOptions::check` with `Kinematics::Linear` and `newton.line_search` (the linear body's
own loop does not read it).

### D2. The trial force of a contribution (`Contribution::add_trial_force`)

A defaulted trait method `fn add_trial_force(&self, u: &[f64], load_factor: f64, force: &mut [f64]) -> Result<()>`: the force
of `add` at `(u, load_factor)` for a trial evaluation of the line search, without side effects. The default calls `add` with a
scratch tangent that is dropped (correct for a contribution without memory). `ContactSet` overrides it: the force of `add` at
the same state (the same `evaluate_all` and the same loop of `force_and_tangent`, without the tangent), and **nothing recorded**
(no active set, no cycle event), so that `NewtonSolve::active_sets` keeps one set per residual (ADR 0519 D1) and D6 sees only
the driver's iterates.

### D3. The contact driver (`crates/rcfd-fem/src/contact.rs`)

The total Lagrangian path passes `options.newton` to `solve_from_reusing` as now, so `ContactOptions::newton(NewtonOptions::..
.line_search(true))` turns D1 on. `NewtonSolve` gains `step_lengths: Vec<f64>` (the `Equilibrium`'s, empty on the linear body
and without the option). No other change of the driver; `curvature_fallback` (ADR 0570) is the convexified tangent on a
refused pivot and is used as it is.

### D4. The models of the checks

- **L1** (c2, c3): the punch of ADR 0519 D3 M0 (`sa_contact_order.rs`'s `punch`, copied: plate, punch, constraints, dead
  pressure `p = 0.01` on the punch top, pair `punch.zmin` on `plate.zmax`, `eps = 800`, 2 steps, tolerance `1e-8`,
  `NewtonOptions::new(1).max_iterations(30)`, `Kinematics::TotalLagrangian`, `max_cycles` 3, 20 augmentations) with the initial
  gap `+3e-4` at the four contactor nodes with `x > 1.0`, and `.curvature_fallback(true)` and `.line_search(true)`.
- **L1-f**, **L1-l** (reports): L1 with only the fallback, with only the line search.
- **L1-g** (reports): L1 with the initial gaps `1e-4, 1e-3, 5e-3, 2e-2`.
- **NL0** (negative control, the default): L1 without either option.
- **LS0** (D1 keeps Newton's bits): ADR 0570's cube FD (`hex_box([0;3], [1;3], [1,1,1])`, `E = 1`, `nu = 0.3`, `zmin` fixed,
  `0.01` on `u_z` of the node `(1,1,1)`, equation `q`) with the honest spring (`k = 1`, exact tangent), `solve_from` from zero,
  `NewtonOptions::new(1)`, with and without the line search.
- **LS1** (D1 finds the line minimum): the same cube with the load `f = 1e-4` and a spring of force `k u_q`, `k = 10`, whose
  reported tangent is `0` (a tangent too soft at `q`), and a recording contribution (zero force; `add` records `u`,
  `add_trial_force` does nothing), `max_iterations(1)`. Without the line search the second evaluation is at `u = d` (the
  correction from `u = 0`, `K_e d = f e_q`); with it at `alpha_1 d`. For the small-strain part of the energy `s(alpha) = -d . f
  + alpha (d . K_e d + k d_q^2)` and `d . K_e d = d . f = f d_q`, so `alpha* = f/(f + k d_q)`. Read: `alpha_1 = u_q(on)/d_q`.
  **LS1n**: the same spring, the line search off, `max_iterations(20)`: the step's amplification at `q` is `k d_q/f` (about
  30, arithmetic 2), so the iteration diverges.
- **LT** (D2): the L1 model's `ContactSet` (`ContactSet::new` with L1's pair and options) at the state `u_i = 1e-3 sin(1.7 i +
  0.3)` on every equation: `add_trial_force` against the force of `add` at the same state, and `active_sets().len()` before
  and after `add_trial_force` and after `add`.
- **RF** (D1 refusals): the three refusals of D1.

## Arithmetic done before the code (disclosed)

1. **L1 with the line search.** The first correction (probe 2) closes the gap at `alpha_c = 3e-4/2.1e-2 = 0.014` of the step;
   beyond it the lifted nodes add `eps_s (G . d)^2` to `phi''`, with `G . d` about `2e-2` per node and `eps_s` 5 to 11: `sum
   about 4 x 8 x 4e-4 = 1.3e-2` against the elastic `phi''(0) = d . K d = |s_0|`, not known before the run. The accepted `alpha`
   is where `s` has fallen to half of `|s_0|`; the lifted nodes are then in contact by a fraction of the full overshoot, and
   the forces by the same fraction. Whether the tangent there factors (or the fallback is needed) and how many iterations
   follow cannot be settled without a run; **the outcome of c2 is uncertain**, probe 3 shows that the problem has a solution
   that the iteration reaches once the overshoot stops.
2. **LS1.** `d_q`: the corner of a unit cube with `E = 1`, `nu = 0.3` under a point load has a compliance of order 1 to 5 (a
   single trilinear cell), so `k d_q/f` is 10 to 50 and `alpha*` 0.02 to 0.1. The total Lagrangian part adds terms of relative
   size the strain at `alpha = 1`, `d_q` of order `3e-4`: the root of the secant through `(0, s_0)` and `(1, s_1)` is off the
   small-strain `alpha*` by about `3e-4` relative, and it passes the test of D1 3 at once. Limit `1e-2`.
3. **LS0.** Strain `1e-2`: `s_1/s_0` of the order of `r_1/r_0`, about `1e-2`, far below `eta`; every `alpha` is 1 and the
   iterates are those of the run without the option, bit for bit (`u_i += d_i` in both).
4. **The force balance of L1** (A3). The punch has no z constraint; the sum over its nodes of the internal force vanishes
   (translation invariance of each element), so at a converged iterate the contact forces' z components balance the dead
   pressure resultant `p A = 8e-4` up to the residual: `1e-10` relative to a reference of order `1e-3`, `1e-13` absolute,
   `1.3e-10` of `p A`. Limit `1e-8`.
5. **Q on L1.** Pairs damped by the line search (`alpha < 1`) are read when the set does not change; far from the solution
   `r_k` is of order 1 and the bound `10 r_k^2` does not bind. A damped pair at `r_k` between `1e-2` and `3e-1` with an
   unchanged set could violate it; arithmetic 3 says such pairs take `alpha = 1`. The later solves of each step (after an
   augmentation) start near the solution, as in ADR 0519 M0, ratio up to 2.6. **Uncertain** for the first solve.

## Checks (limits fixed now)

Every aggregated quantity is the maximum of finite values (each asserted finite first); "bit for bit" compares `f64::to_bits`.

| id | clause | quantity | limit | estimate |
|---|---|---|---|---|
| A1 | c2 | L1: the contact solve returns `Ok` | converges | arithmetic 1 (uncertain) |
| A2 | c3 | L1: violations of Q(10, 1e-12) (ADR 0519 D1, the reader of `sa_contact_order.rs` copied: pairs `k >= 1` of one solve, exempt when the set of evaluation `k + 1` differs from that of `k`) | 0 | arithmetic 5 |
| A2c | c3 | L1: on every solve `active_sets.len() == residuals.len()`, every residual finite | all | D2 |
| A2i | c3 | L1: every load step has a read pair with `r_k >= 1e-6` (ADR 0519 Q2) | every step | ADR 0519 |
| A2r | c3 | L1: the largest `r_{k+1}/r_k^2` of the read pairs above `1e-12` | finite, `<= 10` | ADR 0519 Q3 |
| A3 | c2 | L1 at load factor 1: `abs(abs(sum_s t_s n_z) / (p A) - 1)` over the contactor nodes (`ContactResult::nodes`, `n` the target normal), `p A = 8e-4` | `<= 1e-8` | arithmetic 4 |
| LSr | D1 | L1: every solve has `step_lengths.len() == energies.len()`, every length in `(0, 1]`, and some length `< 1` in the run | all; some | arithmetic 1 |
| LS0 | D1 | LS0: displacement, residuals and energies with the option against without; `step_lengths` with it all `1.0` and as many as the energies; without it empty | bit for bit; all 1, equal count; empty | arithmetic 3 |
| LS1 | D1 | LS1: `alpha_1 = u_q(on)/d_q` against `alpha* = f/(f + k d_q)` (`d_q` from the run without the option), relative; and `0 < alpha_1 < 1` | `<= 1e-2`; holds | arithmetic 2 |
| LS1n | D1 | LS1n: the solve fails (any `Err`) | fails | arithmetic 2 |
| LT | D2 | LT: `add_trial_force`'s force against `add`'s; the count of active sets unchanged by `add_trial_force`, raised by 1 by `add` | bit for bit; unchanged, +1 | D2 |
| RF | D1 | the three refusals of D1, each `FemError::InvalidValue { key: "line_search" }` | refused | D1 |
| NL0 | defaults | NL0: `FemError::Solver { solver: "contact" }`, the reason containing "load step 1 of 2, augmentation 0", "singular tangent: pivot" and "at node 67 component 2" | refused, the texts | probe 1 |
| B | defaults | `cargo test -p rcfd-fem --release` (every existing test and bit record, among them `sa_contact_order.rs`, `sa_contact_tl_fallback_f639.rs`, `sa_contact_default_bits.rs`, `sa_newton_f618_default_bits.rs`, `sa_follower_default_bits.rs`); `cargo test -p rcfd-api --release --test baseline_hashes`; `cargo test -p rcfd-cli --release --test docs_site` | pass | D1 (off by default) |

Recorded without a limit: for L1 per solve the iterations, the fallback iterations, the step lengths, the active sets, the
penetration; the augmentations per step; the final gaps and forces of the eight nodes. L1-f (expected: probe 1's cycle error),
L1-l and L1-g: outcome, iterations, fallback iterations, the reading Q's violations where they converge. N5 of ADR 0519 (the
order of ADR 0404 on L1).

Negative controls: NL0 (the default keeps the refusal), LS1n (without the line search the soft-tangent iteration fails), and
L1-f as a report.

### Planned mutations (on a `git archive` copy of the final tree in `f608-scratch/mut`, target `f608-scratch/target-mut`, each alone by exact text replacement; the new test file run once, the file restored)

| id | mutation | expected to be caught by |
|---|---|---|
| ML1 | the line search computes `alpha` but the step is always taken with `alpha = 1` | LS1, LSr, (A1 if the undamped iteration cycles as in probe 1) |
| ML2 | `s` with the wrong sign (`+ sum d_i r_i`): `s_0 > 0`, no line search | LS1, LSr |
| ML3 | `ContactSet::add_trial_force` calls `add` (it records) | LT, A2c |
| ML4 | the default of `NewtonOptions::line_search` is `true` | LS0 (the run "without" has lengths), NL0 or B |
| ML5 | the trial state ignores `alpha` (always `u + d`) | LS1 |

## Decision rule

- **done**: A1 to A3 (c2, c3), LSr, LS0, LS1, LS1n, LT, RF, NL0 and B met on their registered runs; every planned mutation
  caught by a check named for it; fmt and clippy clean.
- **partial**: B, LS0 and NL0 met (defaults unchanged) and any other check missed (recorded as a `*_known_miss` test with its
  limit and the measured number, and diagnosed) or a mutation not caught.
- **todo**: otherwise, in particular any change of the default bits.

## D5. Files

`crates/rcfd-fem/src/nonlinear.rs` (D1: `NewtonOptions::line_search`, its check, `Contribution::add_trial_force`, the line
search in `Problem::attempt_with`, `step_lengths` of `Attempt`, `Equilibrium`, `LoadStep`), `crates/rcfd-fem/src/arclength.rs`
(the refusal), `crates/rcfd-fem/src/contact.rs` (the refusal, `ContactSet::add_trial_force`, `NewtonSolve::step_lengths`, the
module doc); the new test file `crates/rcfd-fem/tests/sa_contact_lifted_f608.rs`. No existing test or record edited.

## Limits (not covered)

- The line search acts only in the static attempt without a load-factor control and with a symmetric factor; the arc-length
  path refuses it, the linear contact body refuses it. The quasi-static and nonlinear transient drivers reach the same attempt
  through `solve_from`/`solve_with` and would apply it; they are not checked with it.
- An iteration whose prescribed increment is not zero (iteration 0 of a step with non-zero prescribed values) takes the full
  step: a trial state for a partly prescribed step is not defined here.
- The descent argument needs a hyperelastic material and dead loads (an energy); with a history material, a follower load or a
  contribution without an energy the line search is a damping heuristic, not checked.
- The cycle detector D6 is unchanged; a damped iteration can still return to an earlier set.
- Both options stay opt-in; making them the default of the total Lagrangian contact body changes default results (ADR 0542 P,
  ADR 0570's open question) and is not taken.
- Friction, several pairs, the mortar contactor and the supernodal factor are not run with the line search.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **What to add beside the convexified tangent**: (a) a line search on the energy in the Newton driver (D1, generic,
   opt-in); (b) a contact-specific step limit (scale the correction so that no inactive node penetrates beyond a set depth: a
   new parameter without a derivation); (c) raise `max_cycles` (probe 3: it converges with 100, but that disables the guard and
   was found by a probe). Taken (a).
2. **The convexified tangent**: (a) ADR 0570's `curvature_fallback` as it is (it is exactly "the contact matrix without its
   curvature part on a refused pivot"); (b) a new one. Taken (a): the diagnosis shows the direction is not the defect.
3. **Opt-in**: taken opt-in (a default would change ADR 0519's and ADR 0542's runs that refuse or cycle today).

Open (not taken): making the line search or the fallback the default of the total Lagrangian contact body.

## Addendum 1: results (2026-11-15)

The library and the tests were committed (8045d2d3) before any run. Each registered run made once (`w49i-work/f608-scratch`:
`run1.log` the new test file; `mut-ML*.log`, `run-mut.log` the mutations; `run-fem-full.log`, `run-baseline.log`,
`run-docs.log`, `clippy.log`). No limit changed, no amendment, no diagnostic run after the registered one.

| id | limit | measured | met |
|---|---|---|---|
| A1 L1 converges | converges | converges: step 1 two augmentations (first solve 7 corrections, then 3 and 2), step 2 two augmentations (4, 3, 2); penetration `2.15e-10` at the end | yes |
| A2 Q(10, 1e-12) violations on L1 | 0 | 0 of 13 read pairs (2 exempt) | yes |
| A2c one active set per residual, residuals finite | all | all | yes |
| A2i an informative read pair per load step | every step | `[4, 3]` | yes |
| A2r largest `r_{k+1}/r_k^2` of the read pairs above `1e-12` | finite, `<= 10` | 2.61 | yes |
| A3 force balance `abs(abs(sum t n_z)/(p A) - 1)` | `<= 1e-8` | 0.0 (equal to the last bit) | yes |
| LSr one length per correction, in `(0, 1]`; some `< 1` | all; some | all; the first solve `[0.0160, 0.0418, 0.3848, 1, 1, 1, 1]`, every later solve all 1 | yes |
| LS0 bits with and without the option; lengths | bit for bit; all 1; empty without | bit for bit; `[1, 1, 1]`; empty | yes |
| LS1 `abs(alpha_1/alpha* - 1)`; `0 < alpha_1 < 1` | `<= 1e-2`; holds | `1.15e-5` (`d_q = 6.834e-4`, `alpha_1 = 1.44219e-2`, `alpha* = 1.44221e-2`); holds | yes |
| LS1n without the line search the solve fails | fails | fails: element 0 inverted (`det F = -1.51`) | yes |
| LT trial force against `add`; records | bit for bit; unchanged, +1 | bit for bit (24 non-zero entries); `0 -> 0 -> 1` | yes |
| RF the three refusals, key `"line_search"` | refused | refused, all three | yes |
| NL0 the default refused with the texts | refused | "load step 1 of 2, augmentation 0: solver newton: singular tangent: pivot -1.4474e1 at node 67 component 2 (diagonal 7.97)" | yes |
| B `rcfd-fem` release suite; `baseline_hashes`; `docs_site` | pass | 318 test binaries, 1315 passed, 0 failed, 329 ignored; `baseline_hashes` 3 passed, 1 ignored (table not edited); `docs_site` 2 passed | yes |

Recorded without a limit. L1: no fallback iteration in any solve (`convex_iterations` 0 everywhere): with the line search the
exact tangent factors at every iterate. All 8 contactor nodes end active (forces `3.70e-5 .. 1.44e-4`, the numbers of probe
3 to four digits). ADR 0404's order on L1 `[2.26, 1.15, 1.00, 1.13]`. **L1-f** (fallback only): the cycle error of probe 1
at iteration 12, nodes `[82]`, reproduced. **L1-l** (line search only): converges with exactly L1's residuals, sets and
lengths (the fallback is never asked for). **L1-g** with both options: `1e-4`, `1e-3`, `2e-2` converge with 0 Q violations
(largest ratios 2.7, 9.3, 6.9); `5e-3` converges with one Q violation (largest ratio 14.3; two nodes end lifted, gaps
`2.0e-3` and `4.7e-4`); at `2e-2` four nodes end lifted. LS1 with the line search and 20 iterations converges in 4
corrections (the iteration without it inverts the element).

### Notes on the results

1. The diagnosis holds: the defect recorded by ADR 0519's probe 2 is the length of the first Newton step with the lifted nodes
   inactive, not the tangent. The first step is cut to 1.6 percent (close to the gap's share of the overshoot, `3e-4/2.1e-2 =
   1.4` percent), the next two to 4 and 38 percent while the set grows `4 -> 6 -> 8`, then every step is Newton's.
2. **The convexified tangent is on but not exercised in L1**: L1 and L1-l give the same bits. The title's means (ADR 0570's
   `curvature_fallback`, a refused pivot taking `eps G G^T`) stays available for an iterate the line search does not prevent;
   no run here needed it. The acceptance's clauses (convergence and Q) are met by the line search.
3. A3 is met to the last bit: the punch's z balance holds to rounding at the converged state.
4. L1-g at `5e-3` shows that the reading can fail on a lifted start where nodes stay out of contact (one pair above `10
   r_k^2`); it is a report, outside the acceptance.

### Mutations

Each alone on a `git archive` copy of 8045d2d3 (`f608-scratch/mut`, target `target-mut`), the new test file run once.

| id | caught by |
|---|---|
| ML1 (`alpha` always 1 in the step) | A1 (the cycle error of L1-f), LS1 (`alpha_1 = 1`) |
| ML2 (`s` with the wrong sign) | A1 (cycle error), LS1 (`alpha_1 = 1`) |
| ML3 (`add_trial_force` records) | A2c, A2i (`[0, 0]`), LT (`0, 1, 2`) |
| ML4 (default `line_search` true) | LS0 (lengths without the option), LS1n (the solve no longer fails) |
| ML5 (the trial ignores `alpha`) | A1 ("without the curvature part: singular tangent"), LS1 (error 1.0) |

Five of five caught by checks named for them (LSr is not reached under ML1 and ML2: A1 ends the test first).

### Crate runs

`cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings`: clean. `cargo test -p rcfd-fem --release`: 0 failed. `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored. `cargo test -p rcfd-cli --release --test docs_site`: 2 passed. Not run: the workspace suite (the lead's gate).

Files: `crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions::line_search` field, default, builder and check at 1170 to 1175,
1191, 1219 to 1223, 1270 to 1276; `LoadStep::step_lengths` 1300 to 1301; `Attempt` 1343 to 1344; `Contribution::add_trial_force`
1421 to 1427; `attempt_with` 1679 to 1681, 1750, 1835 to 1855; `trial_slope` and `step_length` 1865 to 1979; `step_lengths`
passed on at 2390, 2464 to 2465, 2649), `crates/rcfd-fem/src/arclength.rs` (117 to 123), `crates/rcfd-fem/src/contact.rs`
(module doc 35 to 37, refusal 297 to 303, `NewtonSolve::step_lengths` 360 to 362, `add_trial_force` 2066 to 2071, `Solved`
2081, 2193, 2350, 2414); the new test file `crates/rcfd-fem/tests/sa_contact_lifted_f608.rs`. No existing test or record edited.

### Deviations

None from the registration.

### Decision

**done** by the registered rule: A1 to A3, LSr, LS0, LS1, LS1n, LT, RF, NL0 and B met on their registered runs; five of five
mutations caught by checks named for them; fmt and clippy clean. Plainly: the convergence comes from the line search; the
convexified tangent of the title (ADR 0570) is switched on in L1 but never taken.

### Remaining questions

1. **Defaults.** Options: (a) keep both opt-in; (b) make the line search the default of the total Lagrangian contact body (it
   changes ADR 0519 probe 2's refusal and any default run with `alpha < 1`; full steps keep their bits, LS0).
   Recommendation: (a) now; (b) as a maintainer decision with a superseding ADR, after the line search has been run on the ADR
   0542 hub and the ADR 0551 Hertz cases.
2. **The cycle detector D6** counts returns to the final set as cycles (probe 1). Options: (a) leave (with the line search it
   did not fire); (b) count only returns while the residual does not fall. Recommendation: (a).
3. **The line search on the transient and quasi-static drivers** (reached through `solve_from`, not checked). Options: (a)
   leave unchecked; (b) refuse it there as on the arc-length path. Recommendation: (a) until a case needs it.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-427 and F-600 (ADR 0507, ADR 0519). DONE. ADR 0607, 2026-11-15: diagnosis of the refused pivot with a lifted contactor: with the four lifted nodes inactive the first Newton correction (regular tangent) tips the punch about the two supported rows and throws the lifted half 33 to 71 gaps into the plate; the next tangent, with forces 1000 times their equilibrium, is indefinite through the contact curvature part; ADR 0570's convexified tangent (curvature_fallback) makes it factor but the undamped iteration then cycles (D6, iteration 12). rcfd_fem::nonlinear::NewtonOptions::line_search (opt-in): each correction of an attempt without a load-factor control takes the step length at which s = -d . r(u + alpha d), the derivative of the energy, has fallen to half its value (full step when it passes, else Illinois regula falsi, at most 8 trials); Contribution::add_trial_force (default add with the tangent dropped; the contact set records nothing); step lengths in LoadStep, Equilibrium and NewtonSolve; refused with the unsymmetric tangent, the arc-length path and the linear contact body. The T2 punch with the gap +3e-4 at the four nodes with x > 1.0 (fallback and line search on) converges (first solve lengths 0.016, 0.042, 0.38, then 1), Q(10, 1e-12) of ADR 0519 holds (0 violations in 13 read pairs, largest ratio 2.6), the punch's z force balance to the last bit; the fallback is never taken (the line search alone gives the same bits). Full steps keep Newton's bits; the line minimum of a soft-tangent cube within 1.2e-5; defaults and records unchanged; 5 of 5 mutations caught. Gaps 1e-4 to 2e-2 converge too (5e-3 with one Q violation, two nodes ending lifted; a report). No case-file key.

> Text F-600's note should gain: "Follow-up F-608 done (ADR 0607, 2026-11-15): the lifted-contactor punch converges with NewtonOptions::line_search; Q(10, 1e-12) holds on it."

> Follow-up proposed: line search on the other total Lagrangian contact cases. Deps F-608, F-639. Acceptance: "Every limit not stated here is registered in the ADR before the run. The total Lagrangian contact cases ADR 0542 P (hub, d = 1e-3 a) and ADR 0551 HS20t and HS10t are run with NewtonOptions::line_search with and without curvature_fallback; each run converges or its failure is diagnosed, and the fallback iterations and step lengths are recorded (limits registered)."
