# ADR 0519: F-600 registration: a reading of the contact Newton convergence order that fits short solves, on the total Lagrangian punch of ADR 0507 T2

Status: **REGISTERED 2026-11-14, written before any code of F-600 and before any registered run of its checks.** Not yet
accepted by the maintainer. Before this text the base `0d3ba7b9` was not changed. Probes were made first, in the worktree
(an uncommitted test file `crates/rcfd-fem/tests/sa_contact_order.rs`, later replaced by the registered file) and, for the
controls, in a scratch copy of the base with one mutated line; they printed the residuals and active sets of the punch and
are quoted under "Arithmetic done beforehand". They are not registered runs. The registered checks are made once, after this
commit. Results go in the addenda at the end; nothing above them is changed after a run.

Roadmap item F-600 (deps F-427), follow-up 2 of [ADR 0507](0507-f427-registration-frictionless-contact-augmented-lagrangian.md)
(its T2, a known miss). The reading reused is Q(C, F) of [ADR 0515](0515-f560-registration-newton-order-reading-for-an-active-set-change.md)
(F-560), which sits on the base. Number 0519 given by the lead. Branch `f600-work` at `0d3ba7b9` (the head of the F-560 branch,
which sits on the F-427 module), worktree `w49i-work/f600`, scratch `w49i-work/f600-scratch`. House rules
(`w49i-work/AGENT_RULES.md`) apply; the brief of the lead overrides rule 9 for the F-600 entry of `docs/ROADMAP.md` and
`docs/roadmap.json` (a separate last commit; the svgs are not regenerated).

## What F-600 asks

> Every limit not stated here is registered in the ADR before the run. A contact Newton order measure for short solves
> (r_{k+1}/r_k^2 bounded, or started near the solution), registered for the total Lagrangian contact punch of ADR 0507 T2.

## What was read

ADR 0507 (D2 the Uzawa update, D3 the driver, D6 cycling, D7 T2, amendment 3 and its addendum on T2, the question 2 that
this item answers), ADR 0515 (D1, D2, the reading and its informativeness clause), ADR 0404 (a4), `contact.rs`
(`NewtonSolve::{residuals, active_sets}`, `ContactSet::{record, active_sets, begin}`, `force_and_tangent` lines 792 to 834,
`solve_with`), and the T2 test of `tests/sa_contact.rs` (the ignored `t2_..._a2_known_miss`, lines 1388 to 1450).

## Arithmetic done beforehand (disclosed)

Why the measure of ADR 0404 misses T2. The residuals of the first solve of step 1 of T2 are `[1.0, 0.651, 2.97e-4, 1.53e-7,
4.6e-14]` and of the second `[0.369, 6.06e-6, 1.76e-11, 6.9e-14]`. The last three above `1e-12` of the first are `0.651,
2.97e-4, 1.53e-7`: the first pair is before the asymptotic range (its ratio `r_{k+1}/r_k^2` is `7e-4`, the order `0.98`
only because `r` fell by a factor 2000 while the quadratic estimate expects `r_k^2`), the second is quadratic (`r_{k+1}/r_k^2
= 1.73`); the last residual is at the rounding floor. The largest ratio `r_{k+1}/r_k^2` of a pair above the floor is `1.73` in step 1 and `2.61` in step 2.
The active set of T2 is the set of the 8 punch nodes; it never changes (8 active at every evaluation), so the exemption of ADR
0515 for a changed set is empty on T2 and the reading is a bound on every pair `k >= 1`. This is stated plainly: T2 does
not exercise the exemption, a fourth model (M3) was added that does.

Probe 1 (consistent tangent; `f600-scratch/probe1.log`, `probe3.log`): `NewtonSolve::active_sets` and `residuals` have the same length at
every solve (the contact set is evaluated once per iteration, in `add`, which the driver calls once per iteration, and the
residual of the same iteration is formed from that call). The largest ratio of a read
pair above `1e-12` (every solve): M0 `2.61`; 4 steps (M1) `2.27`; 8 steps `2.14`; `p = 0.02` (M2) `4.0`; M3 `4.38`. The
floor (the residual that follows one below `1e-9`) is at most `9e-14` (M0 `8.6e-14`, M1 `8.7e-14`, M2 `6.6e-14`, M3 `9e-14`);
the last residual of a solve is at most `1.0e-13` where the solve ends below `1e-9`.

Probe 2 (a start with an initial gap, to make the active set change): `initial_gap(+5e-3, +2e-2, +5e-2, +1e-3, +3e-4, +1e-4)`
on the four punch nodes with `x > 1.0` all stopped with `singular tangent: pivot ... at node 67 component 2` (the indefinite
curvature part of ADR 0507 amendment 2, absent from the total Lagrangian path); `-3e-4` (an initial penetration) converges:
residuals of the first solve `[1.0002, 0.7476, 0.9982, 0.0136, 8.1e-4, 2.4e-10, 4.2e-14]`, active counts `[8, 7, 8, 8, 8, 8,
8]`: the set changes at iterations 1 and 2 (a node leaves and returns). That is model M3. In M3 the pair `(r_1, r_2)` has
`c_2` true and is exempt; without the exemption it would still pass (`0.998 <= 10 * 0.559`), so the exemption is exercised
but not needed on these solves (reported, N4).

Probe 3 (the control; a scratch copy of the base with line 823 of `contact.rs` changed to `(false && curvature, &kn.hess)`,
the curvature part dropped, `K_c = eps G G^T`; `f600-scratch/probe-nc1.log`): every Newton solve of the four models converges linearly with
a ratio of `0.02 .. 0.06`; on T2: `2.7e-4 -> 1.06e-6` (`r_{k+1}/r_k^2 = 14.5`), `1.06e-6 -> 1.33e-8` (`1.2e4`), `1.33e-8
-> 1.6e-10` (`9e5`). Every one of the four models has pairs above `F` with a ratio far above 10: the largest ratio per model is `7.7e7`
(M0), `7.6e7` (M1), `8.8e8` (M2), `3.1e8` (M3) (the first pair already `14.5`).

## Design

### D1. The reading (registered)

Input per Newton solve of the driver (`NewtonSolve`, one per augmentation of a load step): the relative residuals `r_0 .. r_n`
and the active sets `S_0 .. S_n` (`S_k` the set of active contactor nodes of all pairs at the evaluation of iteration `k`,
`NewtonSolve::active_sets[k]`; the multipliers are fixed during a solve, so the set of a solve depends on the displacement
only). The flag `c_k` is `S_k != S_{k-1}` (`c_0` false). No library change records it: the driver already keeps the sets.

**The reading Q(C, F).** Every pair `(r_k, r_{k+1})` of one solve with `k >= 1` and `c_{k+1}` false satisfies

    r_{k+1} <= max(C r_k^2, F),     C = 10, F = 1e-12.

A solve passes if no read pair violates the bound and every residual is finite; a run passes if every solve of every step
passes and the run converges (a run that stops with an error fails).

**Pairs with `k = 0` are not read**: `r_0` is the residual of the previous converged state against the new load or the new
multipliers, not an iterate of one Newton series. **Pairs across an augmentation are excluded** (there are none): the Uzawa
update changes the multipliers, hence the problem, and a new Newton solve starts; the reading is applied per solve and no
pair joins the last residual of a solve to the first of the next. The augmentation boundary is thus treated as the start of
a new series, as `k = 0` pairs are; a change of the set of one iteration (D1) is the only exemption inside a solve.

`C = 10`: the largest of the largest ratios in the probes is `4.38` (M3), the margin `2.3`; the value is the one of
F-560 (disclosed: it was taken because it covers the probes, not derived). `F = 1e-12`: about ten times the rounding peak `9e-14`
of the probes; it is below the solves' tolerance (`1e-10`), the floor stands for the residual of a converged state.

**The informativeness of a pair.** A pair is informative if `r_k >= 1e-6` (a linear convergence of ratio `>= 0.01` is then
above the bound, and the pair is not at the floor). Every load step of every model has at least one informative read pair
(the exemption cannot swallow a step), and the registered T2 (M0) has at least 5 in all.

### D2. The diagnostic

None: `NewtonSolve::active_sets` and `residuals` exist. No library line is changed; `contact.rs` and `nonlinear.rs` are not
edited (the controls are mutations in scratch copies). The default bits cannot change; the existing B0 of ADR 0507
(`sa_contact_default_bits.rs`) and `baseline_hashes` are run as gates. No case-file key.

### D3. The models

All four are the punch of ADR 0507 D7 T2 (plate `[0,2] x [0,0.5] x [0,0.25]` of `[8,2,2]` Hex8, punch `[0.8,1.2] x [0,0.2] x
[0.25,0.45]` of `[3,1,1]`, `E = 1`, `nu = 0.3`, `eps = 800`, tolerance `1e-8`, `NewtonOptions::new(1).max_iterations(30)`,
`Kinematics::TotalLagrangian`, the constraints and the contact pair of that test), with:

| model | steps | dead pressure on the punch top | initial gap of the four punch nodes with `x > 1.0` |
|---|---|---|---|
| M0 | 2 | 0.01 | 0 (the registered T2) |
| M1 | 4 | 0.01 | 0 |
| M2 | 2 | 0.02 | 0 |
| M3 | 2 | 0.01 | `-3e-4` (an initial penetration; the set changes) |

M1 to M3 were chosen after the probes (disclosed), to put more solves under the reading and to exercise the exemption; their
parameters were not searched after the registered run.

## Checks

| id | clause | check | limit | estimate behind it |
|---|---|---|---|---|
| D1 | the record | on every solve of M0 to M3: `active_sets.len() == residuals.len()`, all residuals finite; M3 has at least one solve with a changed set; M0, M1, M2 have none (report of the count) | all | probe 1, probe 2 |
| Q1 | acceptance (the bound) | Q(10, 1e-12) on every solve of M0 to M3; every run converges | 0 violations | probes: largest ratio 4.38 |
| Q2 | informativeness | every load step of M0 to M3 has at least one informative read pair; M0 has at least 5 in all | all | probe 1: M0 has 6 |
| Q3 | aggregation | the maximum over M0 to M3 of `r_{k+1}/r_k^2` of the read pairs above `F` is finite and `<= 10` (each term asserted finite before the maximum: `f64::max` over NaN hides failures) | `<= 10` | `4.38` |
| N1 | negative control, acceptance | the contact tangent without its curvature part (MU1, a scratch copy) fails Q(10, 1e-12) on each of M0 to M3 (a run that does not converge counts as failing) | fails on all four | probe 3: largest ratios `7.6e7 .. 8.8e8` |
| N2 | negative control | the contact tangent with the stiffness term `eps G G^T` halved (MU2, a scratch copy) fails Q(10, 1e-12) on each of M0 to M3 | fails on all four | a tangent off by a factor 2 in the dominant term converges with a ratio near `0.5` |
| N3 | `C` is not slack | with the consistent tangent Q(1, 1e-12) fails on M0 | violated | ratio `2.61` |
| N4 | `F` is needed, the exemption | consistent tangent: Q(10, 0) fails on M0; Q(10, 1e-12) without the exemption on M3 (report: violations) | the first violated; the second report only | `6.9e-14 > 10 (1.8e-11)^2` |
| N5 | the order is not the reading | the order of ADR 0404 (a4) on M0 (last three residuals above `1e-12`, set unchanged), and the order of the last three read residuals above `F` on each solve with three or more: report | report only | ADR 0507 amendment 3: `0.171` |

Not registered, on purpose: a limit on the order itself (the item allows "bounded or started near the solution"; the order
does not exist for the solves of three residuals, and the pre-asymptotic residuals of the first solves give `0.98`).
"Started near the solution" is not used: the bound is registered.

## Mutations

Each in a scratch copy (a `git archive` of the final head, its own `CARGO_TARGET_DIR`), running `sa_contact_order.rs` and, for
the library mutations, the default-bits file.

| id | mutation | expected catcher |
|---|---|---|
| MU1 | `contact.rs` line 823: `(curvature, &kn.hess)` becomes `(false && curvature, &kn.hess)` (N1 is run here; the file asserts the failure in the copy) | N1; ADR 0507 T1 |
| MU2 | `contact.rs` line 822: `eps * kn.grad[r] * kn.grad[s]` becomes `0.5 * eps * kn.grad[r] * kn.grad[s]` (N2 is run here) | N2 (and the consistent tests Q1 fail, ADR 0507 T1) |
| MR1 | the reading takes the flag `c_{k+1}` true for every `k >= 1` | Q2 (no informative pair) |
| MR2 | the bound without the square: `r_{k+1} <= max(C r_k, F)` | N3 does not fail on M0 (the reading becomes slack) and N1 passes the reading |

## Decision rule

**Done** if D1, Q1, Q2, Q3, N1, N2, N3 and N4's first part are met, MU1, MU2, MR1 and MR2 are caught and the gates (rcfd-fem
tests, the baseline-hash test, clippy, fmt, docs_site) pass. **Partial** if any check is missed (a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded) or a clause of the acceptance is not run. N5 and the
second part of N4 are reports.

## Limits of this item

The reading is a bound on residuals, not an order: it does not say `p >= 1.8`; the item allows that. `C = 10` and `F = 1e-12`
are read off the probes (disclosed), not derived. T2's active set does not change; only the variant M3 exercises the
exemption and there it is not needed. The reading is of the total Lagrangian path only (the linear body's Newton iterates
use the Gauss-Newton matrix on a refused pivot; its solves are not read). M3 is an initial penetration; an initial gap
`> 0` stopped with a singular tangent (a defect of the total Lagrangian path with a lifted contactor, recorded here, not
diagnosed further). Friction, several pairs and `extra` contributions are not covered. The ADR 0507 T2 test stays an ignored
record; this item does not edit it.
