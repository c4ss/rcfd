# ADR 0665: F-728 registration: coupled held metrics and the held-tolerance resolution floor for hypotheses

Status: **REGISTERED 2026-11-16, written before any code of F-728 and before any run of its checks.** Written by the
implementing agent on branch `f728-work` (from c10a3519, which holds F-701 / ADR 0624 and F-727 / ADR 0662), worktree
`w49i-work/f728`, scratch `w49i-work/f728-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer
would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is
marked "(standing instruction)".

Roadmap item F-728 (deps F-701), the follow-up proposed by ADR 0624 (addendum 1, remaining question 4). The binding acceptance:

> two held metrics (flow and speed) solved together on a synthetic evaluator with a known joint solution within the registered tolerance, the floor from the held tolerance computed against a closed form, a registration below that floor refused with both values named, and every ADR 0624 check keeps its result.

Every limit not stated there is registered here. House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed
after a run (a miss becomes an ignored `*_known_miss` test with the limit intact); each registered run made once; existing tests
not edited; defaults bit-identical; no golden, hash table or `baseline_hashes.rs` edit; no new dependency, no unsafe code;
clean-room. The library change is confined to `crates/rcfd-api/src/design/hypothesis.rs`; the checks go in a new
`crates/rcfd-api/tests/f728_coupled_held.rs`. `f701_hypothesis.rs`, `f727_rerun_sample.rs` and their ignored records are not
edited. ADR 0662's record-format rule is kept: a new key is written only when used, so old records and entries keep their bytes
and hashes.

## What was read

`AGENT_RULES.md`; the brief; ADR 0624 (D1 to D9; the stub's resolution-floor paragraph "below the change of the response caused
by the held-metric tolerance"; amendment 1 with the record, validation, the bisection of a held metric, `verify`, the W3
equal-flow set-up and E1 to E3; addendum 1, remaining question 4 and the proposed follow-up); ADR 0662 (D1 record-format rule, C2
byte literals, B1 parameter guard); `design/hypothesis.rs` (`Held`, `HeldTarget`, `validate`, `registration_entry`,
`register_batch`, `params_for`, `eval_arm`, `check_member`, `member_result`, `verify_slot`, `run`, `statement`);
`crates/rcfd-api/tests/f701_hypothesis.rs` (W3, E1 to E3, V1 with its case "held: 2 held metrics") and
`crates/rcfd-api/tests/f727_rerun_sample.rs` (helpers, B1, C2).

## Disclosed: runs and arithmetic before this registration

1. **Prototype (Python, `f728-scratch/proto.py`, about 130 lines, the formulas of D1 and D2 below and the closed forms of set-up
   WC; not rcfd code, it evaluates no rcfd function).** On WC with the registered tolerances, the damped Newton of D1 from the
   bracket midpoints reaches both holds in 10 evaluations per arm at s = 0 and s = 1 (residuals q 1.5e-11 and 8.4e-9, n 3.9e-12
   and 1.2e-9; parameter errors against the closed form a 8.4e-13 and 5.8e-11, b 3.0e-13 and 1.1e-10). Closed-form numbers:
   `d* = y*(s = 1) - y*(s = 0) = 1.0283760683760683 - 1.004 = 0.024376068376068`; sensitivities at s = 0 `S_q = 1.008`,
   `S_n = -8e-5`, at s = 1 `S_q = 1.0311111`, `S_n = -5.4700855e-5`; floor `F = 2.040458e-6`, `10 F = 2.040458e-5`. The
   one-sweep decoupled solve (NCJ1) leaves `q - 1 = -0.0876` (s = 0) and `-0.299` (s = 1). With only `q` held and `a` uniform on
   [45, 55] the mean change is 0.023727 (range 0.023520 to 0.023907), 6.5e-4 from `d*` (NCJ2). The J5 variant (gamma 1, bracket
   of `a` [10, 90]) cannot match the low arm for c above 0.7928 (Newton stops at a = 90 with both q and n off) and matches every
   high arm and the probe at c = 0.5. The coupled `AsLowArm` case: low arm at a = 50, b = 0.02 gives q 0.99601594, n 49.8007968,
   y 1.0; `d*_low = 0.0242789526`, floor 1.0316581e-6 (high arm only).
2. **Smoke run on the base code (temporary test `zz_f728_smoke.rs` in the worktree before any library change, never committed,
   moved to `f728-scratch/`; log `smoke.log`).** It writes two single-hold ledgers (set-up W3s below, `Value(1)` and
   `AsLowArm { at: 0.8 }`) with the code of c10a3519 and prints their byte lengths, the FNV-1a 64 of their bytes, the record
   hashes and the lessons statements. Those are the literals of J0 below. No other check's code ran.

## Design (fixed now; names are the implementation's)

### D1. Coupled holds: a damped Newton on the held residuals

A record may hold up to `MAX_HELD = 2` held metrics, each with its own compensating parameter (`Held` is unchanged; the record
already holds a list, so the record format and its hash are unchanged: a record with two holds is a list of two). With two holds
the two compensating parameters `u = (u_1, u_2)` (in held order) are solved together for the residuals `r_j(u) = g_j(u) - t_j`,
`g_j` the held metric and `t_j` its target, with the factor and the domain point fixed. Derivation (first principles): the
first-order Taylor expansion `r(u + delta) = r(u) + J delta + O(|delta|^2)`, `J_jk = d g_j / d u_k`, is zero at
`delta = -J^{-1} r` (Newton's method for systems; Dennis and Schnabel 1983, chapter 5 and 6, cited from memory). Details:

* Start: the bracket midpoints (`Value` targets); for the high arm of `AsLowArm` targets, the `at` values (the low arm's
  setting).
* Converged when `|r_j| <= tolerance_j` for every j (each hold's own registered tolerance); that evaluation is the evidence entry.
* `J` by forward differences, `h_k = sqrt(eps) max(|u_k|, w_k)` (`w_k` the bracket width; `-h_k` when `u_k + h_k` is above the
  bracket); the step is the solution of `J delta = -r` by Gaussian elimination with partial pivoting.
* Damping and the bracket box: the trial point `P(u + lambda delta)`, `P` the projection onto the bracket box, is accepted when the
  scaled residual norm `||(r_j / tolerance_j)_j||_2` falls below `(1 - 1e-4 lambda)` times its value at `u`; `lambda` starts at 1
  and is halved at most 30 times. The scaling makes the test independent of the units of the two metrics; Newton's step itself
  is invariant under a scaling of the residuals.
* Stops (the last accepted iterate's evaluation is the evidence entry; it does not match, so the result is void as with an
  exhausted bisection): 50 Newton iterations; no acceptable step; a singular or non-finite `J` or step; a failed or
  metric-less evaluation at a difference point. A failed evaluation at an iterate is the evidence entry (a failed arm), as with
  the bisection.
* `provenance.iterations` counts every evaluation of the arm, as for the bisection.
* Targets: both `Value`, or both `AsLowArm` (the low arm at both `at` values, one evaluation; the high arm solved to the low arm's
  two held metric values). A mix is refused (not covered).
* One hold: exactly ADR 0624's bisection, unchanged. No hold: unchanged.

Evidence parameters: factor, domain, then the compensating parameters in held order (for one hold this is ADR 0624's order).
`verify` checks the count (`1 + domain + holds`), and each compensating parameter by ADR 0624's rule (its name and unit, inside
its bracket, equal to `at` in the low arm of `AsLowArm`). The recomputed result is void when any ok arm has any held metric off
its target by more than its tolerance; the reason names the pair, the arm and the metric, holds checked in held order (for each
hold: low then high for `Value`, high against the low arm for `AsLowArm`; ADR 0624's order for one hold).

### D2. The held-tolerance floor (first principles)

An arm accepted with residuals `r` (`|r_j| <= tol_j`) is evaluated at `u* = u° + delta`, where `u°` is the exact solution
(`r(u°) = 0`) at the same factor level and domain point. To first order `r = J delta` and the response error is
`y(u*) - y(u°) = grad_u y . delta = grad_u y . J^{-1} r = S . r`, with `S = J^{-T} grad_u y`, the sensitivity of the response to
each held metric along the hold (`S_j = dy / dg_j` with the other held metrics fixed). So the response error of an arm is at most

`phi = sum_j |S_j| tol_j`

(attained when each residual sits at its tolerance with the sign of `S_j`). The paired difference `d = y_high - y_low` is in error
by at most `phi_low + phi_high` for `Value` targets and by `phi_high` for `AsLowArm` targets (the low arm is evaluated at `at`
with no residual, and the claim compares with the low arm's own held values). A mean of such differences is in error by at most
the largest per-pair bound. The floor is

`F = phi_low + phi_high` (`Value`), `F = phi_high` (`AsLowArm`),

evaluated at the probe point: the domain midpoint (each bound `lo + (hi - lo) / 2`), both factor levels, each arm solved by the
solver of D1 (or the bisection for one hold) exactly as the evidence would be. Derivatives at the solved point by central
differences, `h_k = eps^(1/3) max(|u_k|, w_k)` (the step that balances the `h^2` truncation and `eps / h` rounding errors), and
one-sided with `h_k = sqrt(eps) max(|u_k|, w_k)` into the bracket when `u_k +- h_k` leaves it; `S` from `J^T S = grad_u y` (Gaussian
elimination with partial pivoting; for one hold `S = y' / g'`).

Rule: `register_with_floor` refuses `min_effect < K_RES F` (`K_RES = 10`, the factor ADR 0624 uses for the declared resolution;
standing instruction), with a message naming `min_effect`, its value, the floor and its value. The declared-resolution rule of ADR
0624 stays as it is and is checked first.

### D3. API and storage

* `held_floor(h, model) -> Result<HeldFloor, String>`, `HeldFloor { value, probe: Vec<f64>, arms: Vec<ArmFloor { arm, by:
  Vec<f64>, sensitivity: Vec<f64>, bound }> }` (one `ArmFloor` per solved arm: two for `Value`, the high arm for `AsLowArm`).
  Refused (located) when the probe of an arm does not match its holds, an evaluation fails, or `J` is singular.
* `register_with_floor(ledger, h, model) -> Result<Registered, String>`: the validation of ADR 0624 with up to two holds, then
  the model's id, version and tier equal to the record's, at least one hold, the floor, the floor rule, then one registration
  entry as `register` writes it plus `settings.held_floor = {value, k, probe, arms: [{arm, by, sensitivity, bound}]}`. The record
  and its hash are not changed by the floor (it is a property of the model, not of the claim); the registration entry, which every
  evidence entry references by hash, carries it.
* `register` and `register_batch` keep ADR 0624's behaviour: at most one hold, no floor, no new key. Their message for two holds
  becomes "held: 2 held metrics; `register` takes one, `register_with_floor` takes two (ADR 0665)" (ADR 0624's V1 asserts only
  "held: 2 held metrics").
* New validation in `register_with_floor` (located, nothing appended): more than two holds; a held metric given twice; a
  compensating parameter given twice; mixed `Value` and `AsLowArm` targets; no hold; a model of another id, version or tier;
  the probe not matched.
* `verify`: a registration whose record has two holds and whose entry has no `held_floor` is a problem ("no held-tolerance
  floor"); a stored floor that is not finite and non-negative, or with `min_effect < K_RES value`, is a problem naming both values;
  with a re-run model (the rule of the evidence re-run: same id and version, tier analytic or reduced order, or cfd with a
  registered sample) the floor is recomputed and a value differing in its bits is a problem ("held-tolerance floor ... not
  reproduced"). Registrations without `held_floor` and with at most one hold are checked as before.
* `lessons` statement: each hold as ADR 0624 prints one (" with `q` held within 0.000001 m3/s by `b`"), joined by ", "; for one
  hold the text is unchanged.

## Set-ups (fixed now)

Every ledger uses the fixed clock `1_700_000_000_000`.

* **WC (flow and speed).** Parameters `s` [1] (factor 0 to 1), `c` [1] (domain [0, 1]), `b` [1] (throttle, compensating for the
  flow, bracket [0.001, 0.1]), `a` [V] (drive, compensating for the speed, bracket [10, 100]). With `A = 1 + 0.5 s`,
  `B = 1 + 0.3 s`, `C = 1 + beta s`, `G = 1 + gamma c`: speed `n = a B / (G (1 + 0.2 b))` [1/s], flow `q = b n A` [m3/s],
  response `power = a b C` [W]. Holds in this order: `q` target `Value(1)` tolerance 1e-6 by `b`; `n` target `Value(50)` tolerance
  1e-5 by `a`. Each metric depends on both compensating parameters (`n` on `a` and `b`, `q` through `n` on both), so the holds are
  coupled. `beta = 1`, `gamma = 0` unless stated. Claim "raising s raises the power at equal flow and speed", `Increases`,
  `min_effect` 0.01, resolution 0, 32 pairs; evaluator `synthetic.wc` version `1`, deterministic, tier analytic (reduced_order in
  J9).
* **Closed forms of WC.** Joint solution for targets `(q*, n*)`: `b* = q* / (n* A)`, `a* = n* G (1 + 0.2 b*) / B`; the response
  on the hold `y = (G C / B) (q / A + 0.2 q^2 / (n A^2))`; sensitivities `S_q = (G C / B) (1 / A + 0.4 q / (n A^2))`,
  `S_n = -(G C / B) 0.2 q^2 / (n^2 A^2)`; partials of the solution `db/dq = 1 / (n A)`, `db/dn = -q / (n^2 A)`,
  `da/dq = 0.2 G / (A B)`, `da/dn = G / B`. All are written in the test from these formulas.
* **W3s (single hold, J0, J2b).** `s` [1] factor 0 to 1, `c` [1] domain [0, 1], `n` [1/s] compensating; `q = n (1 + 0.5 s)`
  [m3/s], `tone = q^2 (1 - 0.1 s)` [Pa]; held `q` tolerance 1e-13 by `n` bracket [0.1, 10]; `Decreases`, `min_effect` 0.01;
  16 pairs; evaluator `synthetic.w3s` 1, deterministic, tier reduced_order; target `Value(1)` seed 728_900, `AsLowArm { at: 0.8 }`
  seed 728_901 (the smoke's set-up). Closed-form floor: `S = dtone/dq = 2 q (1 - 0.1 s)`; `Value(1)`:
  `F = (2 + 1.8) 1e-13 = 3.8e-13`; `AsLowArm`: `F = 2 (0.8) (0.9) 1e-13 = 1.44e-13`.

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f728_coupled_held.rs`, release profile. Clause numbers: (1) joint solve, (2) floor against the
closed form, (3) refusal below the floor, (4) every ADR 0624 check keeps its result.

| Id | Clause | What | Limit |
|---|---|---|---|
| J1 | (1) | WC, seed 728_001, `register_with_floor`, `run` (workers 1): (a) every one of the 64 evidence entries ok with `|q - 1| <= 1e-6` and `|n - 50| <= 1e-5` (from the stored metrics); (b) `a` and `b` of every arm against the closed form `a*`, `b*`; (c) evaluations per arm; (d) mean change against `d*`; verdict; `verify` | (a) 64 of 64; (b) within twice the first-order bound `|dx/dq| 1e-6 + |dx/dn| 1e-5` of each (s = 0: a 2.04e-5, b 4.8e-8; s = 1: a 1.559e-5, b 3.2e-8); (c) at most 40; (d) `|mean - d*| <= F` (closed form, 2.0405e-6); confirmed; no problem, the result genuine |
| NCJ1 | (1) | the evaluator is coupled: one sweep of the single-hold bisection (ADR 0624's rule, written in the test) on `b` for `q` with `a = 55`, then on `a` for `n` with that `b`, at c = 0.5, s = 0 and s = 1 | `|q - 1| > 1e-6` at both (the control must fail J1 (a)) |
| NCJ2 | (1) | only `q` held (plain `register`, `a` in the domain on [45, 55]), seed 728_002 | `|mean - d*| > 100 F` = 2.04e-4 (holding the speed matters) |
| J2 | (2) | the floor stored by the J1 registration (and `held_floor` of the record and model) against the closed form `F = sum_arms sum_j |S_j| tol_j` at c = 0.5; each `S_j` against the closed form | `F` relative <= 1e-6; each `S_j` within `1e-6 max_j |S_j|` of the closed form; two arms; `by` of each arm within the J1 (b) bound of `a*`, `b*` |
| J2b | (2) | W3s single hold: `held_floor` for `Value(1)` and for `AsLowArm { at: 0.8 }` | relative <= 1e-6 of 3.8e-13 and of 1.44e-13 |
| J2c | (2) | WC with both targets `AsLowArm` (`b` at 0.02, `a` at 50): floor against `phi_high` from the closed form at the low arm's `q`, `n` | relative <= 1e-6; one arm (high) |
| J3 | (3) | WC, `min_effect` 2.0e-5 (below `10 F` = 2.0405e-5) with `register_with_floor`; then 2.1e-5 | refused, the message contains "min_effect", "held-tolerance floor", "0.00002" and the library's floor value as printed (`{}`); the file unchanged; 2.1e-5 accepted (slot 1) |
| J4 | — | guards: (a) the J3 accepted registration with `settings.held_floor` removed from the file text; (b) the stored floor set to 0.01 (`10 x 0.01 > min_effect`); (c) the stored floor multiplied by `1 + 1e-9`; (d) a completed J1 ledger with `a` of pair 3 low arm set to 150 in the file; (e) a completed J1 ledger with the metric `n` of pair 3 high arm set to 50.001 and the result's parent hash of that entry rewritten to its new `entry_hash` | (a) a problem containing "held-tolerance floor", `run` refused; (b) a problem naming `min_effect` and the floor; (c) no problem without a model, a problem containing "not reproduced" with the model; (d) a problem containing "compensating parameter 'a'"; (e) every parent of the result resolves, the recomputation is void naming pair 3, arm high and `'n'`, the stored result flagged; 5 of 5 |
| J5 | (1) | WC with `gamma = 1`, bracket of `a` [10, 90], seed 728_005 | registration accepted (the probe at c = 0.5 matches); void, the reason names a pair, `arm low` and `'q'` or `'n'`; the next registration gets slot 2 |
| J6 | — | refusals: plain `register` with the two WC holds; `register_with_floor` with three holds, `q` held twice, `b` the compensating parameter of both, mixed targets, no hold, a model of id `synthetic.other`, the bracket of `a` [10, 20] (the probe cannot reach n = 50) | 8 of 8 refused with the field named ("held: 2 held metrics" and "register_with_floor"; "held: 3 held metrics"; "given twice"; "compensating parameter 'b'"; "targets"; "no held metric"; "model"; "probe"); the file unchanged |
| J7 | (1) | WC with both targets `AsLowArm` (`b` at 0.02, `a` at 50), seed 728_007 | every high arm within the tolerances of its own low arm's `q` and `n`; `|mean - d*_low| <= F_low` (closed forms, 0.0242789526 and 1.0316581e-6); confirmed |
| J8 | — | the J7 ledger cut after the registration and k evidence lines, k = 0, 1, 33, 63, `run` again; the J1 ledger with workers 8 | each file byte-identical to the uninterrupted one; equal to the workers-1 bytes |
| J9 | — | WC at tier reduced_order, seed 728_009: `verify` with the model; `lessons` (current `synthetic.wc` 1) | no problem, `evidence_rerun = Some(true)` (the floor recomputed); one row whose statement contains "`q` held within 0.000001 m3/s by `b`" and "`n` held within 0.00001 1/s by `a`" |
| J0 | (4) | the two W3s ledgers of the smoke written by the new code with plain `register` | byte length, FNV-1a 64 of the bytes, record hash and statement equal to the smoke literals of c10a3519 (below); no `held_floor` key |
| C1 | (4) | `cargo test -p rcfd-api --release --test f701_hypothesis` (19 fast) and once `-- --ignored` (the two heavy records) | 19 pass; heavy records pass with ADR 0624's numbers: S1 1, S2 1592 of 1595, NC1 400, NC2 61, S4 118 |
| C2 | (4) | `--test f727_rerun_sample` (fast) and once `-- --ignored` | pass; heavy A1 394, NC6 0, NC7 116 (ADR 0662 addendum 1) |
| C3 | — | `cargo test -p rcfd-api --release --lib`; `cargo test -p rcfd-cli --release --test docs_site` (docs edited) | pass |
| R3 | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

Smoke literals of J0 (c10a3519): `Value(1)`: 34 entries, 29785 bytes, FNV-1a 64 of the bytes `aaefeee2393a6515`, record hash `fnv1a64:8036707366931a92`; `AsLowArm { at: 0.8 }`: 34 entries, 29351 bytes, `d0af079ac415b42b`, record hash `fnv1a64:2ec4743bbbfe4537`; both confirmed, no problem with the re-run. The statements (`smoke.log`) are compared in full; the `Value(1)` one ends "... with `q` held within 0.0000000000001 m3/s by `n` over the registered domain. Confirmed as hypothesis H-1 of the family (slot 1, level 3.040e-2). Model error of this tier unknown. Not tested at cfd." The W3s model of the smoke computes `q = n (1 + 0.5 s)` and `tone = q q (1 + b s)` with `b = -0.1`, and the test copies it verbatim.

Estimates behind the limits: J1 (a) is the solver's own stopping rule, so any accepted arm meets it; (b) twice the first-order
bound leaves room for the second-order term, which at these tolerances is below 1e-10 of the bound; (c) the prototype needs 10
evaluations, 40 allows three Newton iterations with full backtracking of one; (d) the floor bounds the error of every pair, and
the prototype's error is below 1e-9. J2: the central-difference error for WC is dominated by rounding, `eps / (h / u) ~ 1e-11`
relative, so 1e-6 is a wide margin; a floor missing the `n` term is off by 8e-4 relative and one missing an arm by about 0.5.
NCJ2: the prototype's distance 6.5e-4 is three times the limit. J8: the bytes do not depend on the worker count or the cut
because the design, the solver and the append order do not (ADR 0624 G4, R1).

## Negative controls and mutations (registered now)

Negative controls: NCJ1 (a decoupled solve does not meet the holds: the joint solve is needed); NCJ2 (holding the flow alone
gives another answer: the second hold matters); J4 (a), (b) and (e) are guards against a registration or evidence that skips
the floor or the second hold.

Mutations (scratch copy, one at a time; each must fail a registered fast check):

| Id | Mutation | Expected to fail |
|---|---|---|
| M24 | the coupled solve replaced by one sweep of the single-hold bisection over the holds in order | J1 |
| M25 | the floor sums the first held metric only | J2 |
| M26 | the floor of `Value` targets counts the high arm only | J2 |
| M27 | the floor rule not applied in `register_with_floor` | J3 |
| M28 | `verify` does not require a stored floor for two holds | J4 (a) |
| M29 | the floor not recomputed by `verify` with the model | J4 (c) |
| M30 | only the first compensating parameter checked against its bracket | J4 (d) |
| M31 | only the first held metric's tolerance checked in the recomputation | J4 (e) |
| M32 | `settings.held_floor` written as null by plain `register` | J0 |
| M33 | the `AsLowArm` high arm's targets taken in reversed held order | J7 |
| M34 | the backtracking removed (the full projected Newton step always accepted) | recorded, not judged (WC is smooth enough that a full step may converge) |

## Decision rule

* **done:** every check above met, NCJ1 and NCJ2 failing as stated, every mutation M24 to M33 caught.
* **partial:** J1, J2, J3, J0 and C1 met, and another check missed (an ignored `*_known_miss` record with the limit intact), or a
  control does not fail as stated, or a mutation survives.
* **todo** (the lead does not take the code): J1, J2, J3, J0 or C1 missed.

## Limits (not covered)

* At most two holds; three or more are refused. The solver is general in the number of holds, but only two are checked.
* Mixed `Value` and `AsLowArm` targets are refused.
* The floor is first order and is evaluated at the domain midpoint only. A model whose sensitivity to the held metrics varies
  strongly over the domain, or tolerances large enough that the second-order term matters, is not bounded by it elsewhere; the
  stored floor names its probe point.
* Newton finds a solution inside the bracket box from the registered start; a model with several solutions in the box gets the
  one Newton reaches, and one without a solution in the box gives a void result (as ADR 0624's bisection does).
* `register` and `register_batch` keep ADR 0624's behaviour (one hold, no floor); a batch with two holds is not built. The floor
  for one hold exists only through `register_with_floor` (opt-in).
* The floor is in the registration entry, not in the record: the record hash is the claim's, and the floor is reproduced by
  `verify` only with the model at hand.
* Library only; no MCP tool (F-702). Synthetic evaluators only; no fan, no CFD.

## References

* Newton's method for nonlinear systems, line-search damping and finite-difference Jacobians: J. E. Dennis and R. B. Schnabel,
  Numerical Methods for Unconstrained Optimization and Nonlinear Equations, Prentice-Hall 1983, chapters 5 and 6 (cited from
  memory; every formula used is derived above).
* The finite-difference steps `sqrt(eps)` and `eps^(1/3)`: balance of truncation and rounding error (first principles, above).
* First-order error propagation through the implicit function theorem: first principles, above.
