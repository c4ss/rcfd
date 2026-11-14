# ADR 0515: F-560 registration: a reading of the Newton convergence order that fits an active-set change, on the J2 solves of ADR 0475

Status: **REGISTERED 2026-11-14, written before any code of F-560 and before any registered run of its checks.** Not yet
accepted by the maintainer. Before this text the base `d41012db` was not changed. A probe was made first, in a copy of the
base in the scratch folder (`w49i-work/f560-scratch/probe`, with the diagnostic of D2 and, for the control, the one-line
mutation MU1 of ADR 0475), to choose the constants `C` and `F` of D1 from the residuals of the four solves; its numbers are
quoted under "Arithmetic done beforehand" and are not a registered run. The registered checks are made once, after this commit.
Results go in the addenda at the end; nothing above them is changed after a run.

Roadmap item F-560 (deps F-422), follow-up 1 of [ADR 0475](0475-f422-registration-j2-plasticity-radial-return.md); the order
measure it replaces is that of [ADR 0404](0404-f397-registration-geometric-nonlinearity-total-lagrangian-newton.md) (a4).
Number 0515 given by the lead. Branch `f560-work` from main d41012db, worktree `w49i-work/f560`, scratch
`w49i-work/f560-scratch`. House rules (`w49i-work/AGENT_RULES.md`) apply; the brief of the lead overrides rule 9 for the F-560
entry of `docs/ROADMAP.md` and `docs/roadmap.json` (a separate last commit; the svgs are not regenerated).

## What F-560 asks

> Every limit not stated here is registered in the ADR before the run. On the sphere and beam solves of ADR 0475 (S1, S2, S3,
> P1), a reading of the convergence order fit for an active-set change, registered before the run (for example the order of
> the last three residuals counted from the first correction that does not change the set of yielded points, with a floor of
> 1e-14, or r_{k+1} <= C r_k^2 over the last two corrections with C registered), gives an order >= 1.8 or the bound on every
> increment, and the continuum tangent (mutation MU1 of ADR 0475) fails it.

## What was read

ADR 0475 (D3, D5, D6, the O1 row, the diagnosis of O1, MU1), ADR 0404 (a4), `nonlinear.rs` (`attempt_with`, `LoadStep`,
`NewtonOptions`, `StateLayout`), `mat_j2.rs` (the tangent, line 131), `tests/sa_j2.rs` (the sphere and beam builders, `o1`,
`o1_order_known_miss`), `tests/sa_j2_default_bits.rs` (the form of a bit record).

## Arithmetic done beforehand (disclosed)

Why the reading of ADR 0404 misses. Newton's method with the consistent tangent converges quadratically *while the map is
smooth*, that is while the set `S_k` of integration points whose trial state differs from the committed one (the points that
yield in this evaluation) does not change. A correction that moves a point across the yield surface changes `S`, the tangent
used for that correction belonged to the old set, and the residual that follows carries no order. In ADR 0475 the residuals of
a plastic increment of S1 are `[0.143, 0.153, 3.74e-2, 1.43e-5, 1.99e-10, 8.4e-15]`: the first two corrections change the set
(yielded points 123, 540, 648, 648, ...), the rest is quadratic (`r_4 / r_3^2 = 0.97`) and ends at the rounding floor. The
last three residuals above `1e-12` are `r_2, r_3, r_4`: one pre-asymptotic, giving `1.28`.

Why an order from three residuals cannot be the reading. Under displacement control (the beam) the increments converge in
three or four corrections, `[~0, 0.0808, 2.0e-4, 3.4e-8, 5.6e-15]`: with the first residual (not a Newton residual, see D1),
the set change after the first correction, and the floor, fewer than three residuals remain that are not pre-asymptotic. An
order needs three. The probe measured the order of the last three residuals above `1e-13` counted from the last set change:
`1.27 .. 1.36` (S1, S2, S3, P1) and undefined (fewer than three) for four or five increments of every P1 solve. So the reading
registered is the second example of the item, the bound.

The bound. For a smooth map with a Lipschitz tangent, `||r_{k+1}|| <= C ||r_k||^2` for iterates in one smooth region; the
constant is not derivable here (it depends on the model), it is read off the four solves (probe, below). A quantity that
converges linearly with ratio `rho` violates the bound as soon as `r_k < rho / C`: for `rho >= 0.1` and `C = 10`, from
`r_k = 1e-2` on; every increment reaches `1e-10`, so the violation is certain.

Probe (consistent tangent; the four solves of D3; records `w49i-work/f560-scratch/probe-base.log`, analysed by `an2.py`). The
rounding floor of the residual, the value that follows a residual below `1e-9`, peaks at `1.45e-14` (S3) and `1.9e-15`
(beams): the floor `1e-14` of the item's example is below it. With the floor `F = 1e-13`, the least `C` for which every read
pair satisfies the bound is: S1 `0.969`, S2 `1.18`, S3 `1.96`, P1 `0.22, 1.08, 1.47, 1.7, 1.56, 3.32` (kappa/kappa_y = 1.25,
1.5, 2, 3, 4, 5). Under MU1 (probe, `probe-mu1.log`): S1, S2 and each P1 solve violate the bound (`C` needed `2.2e9 ..
4.7e9`: the residual falls from `1e-10` to `4e-11`, a ratio of 0.3 to 0.5); S3 does not converge (load step 19, 5 halvings,
relative residual `2.7e-9`). The constants below were chosen from these numbers; the registered runs are made afresh after
this commit.

## Design

### D1. The reading (registered)

Input per converged increment of a solve: the residuals `r_0 .. r_n` and the flags `c_0 .. c_n`, `c_k` true if the set of
yielded integration points at iteration `k` differs from the set at iteration `k - 1` (`c_0` is false). The set at iteration
`k` is the set of integration points whose trial state, evaluated at the iterate `u_k` from the committed states, differs
(bitwise, in any value) from the committed state: for J2 the points with `dg > 0` in that evaluation.

**The reading Q(C, F).** Every pair `(r_k, r_{k+1})` with `k >= 1` and `c_{k+1}` false must satisfy

    r_{k+1} <= max(C r_k^2, F),     C = 10, F = 1e-13.

Pairs with `k = 0` are not read: `r_0` is the residual of the previous converged state against the new load, and under
displacement control the prescribed increment enters only with the first correction, so `r_0` is not an iterate of a Newton
series (beam: `r_0 ~ 1e-15`). Pairs with `c_{k+1}` true are not read: the correction from `u_k` changed the active set, which
no quadratic estimate covers. This is "counted from the first correction that does not change the set", applied to every
stretch of unchanged set and not only to the last. An increment *passes* if no read pair violates the bound; a solve passes if
every increment passes and every residual is finite; a solve that does not converge fails.

`C = 10`: the largest least-`C` of the probe is `3.32` (P1, kappa = 5 kappa_y); the margin is a factor 3. `F = 1e-13`: seven
times the rounding peak `1.45e-14`; the floor stands for the residual of a converged state, which stops at `1e-10` and is
not driven to rounding. The item's `1e-14` lies below the measured rounding and is not used; the order of three residuals
(secondary report only, not a check) uses the same `F`.

The reading uses only the residual list and a per-iteration set-changed flag; nothing in it is specific to J2 or to solids.
It applies unchanged to any Newton solve whose nonsmooth events are a change of a discrete set (the contact status of nodes:
F-600 can read the same pairs with the contact status as the set).

**The informativeness of a pair.** A pair is *informative* if `r_k >= 1e-6` (then a linear convergence of ratio `>= 0.01` is
above the bound `10 r_k^2`, as `0.01 r_k > 10 r_k^2` iff `r_k < 1e-3`, and the pair is not at the floor). Every increment must
have at least one informative read pair, so that an exemption that swallows everything (all flags true) cannot pass.

### D2. The diagnostic (the only library change)

`NewtonOptions::record_active_set(bool)` (default false) and two fields of `LoadStep`: `yielded: Vec<usize>` (per iteration,
the number of points of D1's set) and `set_changed: Vec<bool>` (per iteration, `c_k`); both empty unless asked for. Computed
in `attempt_with` after the trial states of the iteration exist, from `states_c` and `trial` through `StateLayout`, with no
arithmetic of the solve involved (the guard is one `if`). Results with the record on or off are bit-identical in
displacement, reactions, residuals, energies and states (check B1). No case-file key.

### D3. The solves

As ADR 0475 D5, D6: S1 (`c = 1.25 a`, 8 steps), S2 (`c = 1.5 a`, 10 steps), S3 (`0.98 p_lim`, 20 steps), P1 (six beams,
kappa/kappa_y = 1.25, 1.5, 2, 3, 4, 5, `ceil(4 kappa/kappa_y)` steps), driver defaults, Hex20, perfectly plastic. The builders
are copied into the new test file (an integration test cannot import another; ADR 0475's files are not edited).

## Checks

| id | clause | check | limit | estimate behind it |
|---|---|---|---|---|
| B0 | default bits | `tests/sa_j2_order_default_bits.rs` on the base `d41012db` before any library change: FNV digests of displacement, reactions, per-increment load factor, halvings, residuals and energies, and states of S1, S2, S3 and the P1 solves at kappa = 1.25 and 5 | the digests printed on the base are written into the file; equal after the change | the change adds fields only |
| B1 | opt-in | the same solves with `record_active_set(true)`: the digests of B0 are reproduced; with it off, `yielded` and `set_changed` are empty | equal; empty | D2 |
| D1 | the diagnostic | on every increment of the solves: both vectors as long as `residuals`; `set_changed[0]` false; `yielded[k] != yielded[k-1]` implies `set_changed[k]`; in S1 and S2 the last `yielded` of the final increment equals the number of points with `alpha > 0` in the final states (monotonic loading: every plastic point yields again) | all | the set at the converged iterate is the set of the last increment |
| Q1 | acceptance (the bound on every increment) | Q(10, 1e-13) on S1, S2, S3 and the six P1 solves with the consistent tangent: violations; the least `C` that would pass is printed | 0 violations in every increment; every residual finite | probe: least C `<= 3.32` |
| Q2 | informativeness | every increment of those solves has at least one informative read pair; the number of increments with a set change is `> 0` in S1, S2, S3 and every P1 solve | all | probe: `r_1 >= 2e-5` in every increment |
| Q3 | aggregation | the maximum over the nine solves of the largest ratio `r_{k+1} / r_k^2` of the read pairs above `F` is finite and `<= 10` (every term is asserted finite before aggregation: `f64::max` over NaN hides failures) | `<= 10` and finite | probe: `3.32` |
| N1 | negative control, acceptance | the continuum tangent (MU1: `theta_bar = 1 / (1 + H/(3 mu))`, the `(1 - theta)` dropped) in a scratch copy: Q(10, 1e-13) on S1, S2, S3 and the six P1 solves | fails on every one of the nine solves (S3 by not converging counts as failing) | probe: ratios of `2e9` and more |
| N2 | the exemption is needed | in the consistent tangent, the reading without the exemption of `c_{k+1}` (every pair `k >= 1`, `C = 10`, `F = 1e-13`) fails on at least S3 | at least one solve violates | S3 at lambda 0.9: `r_1 = 0.0764, r_2 = 0.0998 > 10 r_1^2 = 0.058` |
| N3 | `C` is not slack | in the consistent tangent Q(1, 1e-13) fails on S2 and S3 | both violate | least C `1.18` and `1.96` |
| N4 | the order is not the reading | the order of the last three residuals above `F` counted from the last set change, consistent tangent: reported, with the number of increments where it is undefined | report only | diagnosis above |

Not registered, on purpose: a limit on the order itself (the item allows "order or bound"; the order does not exist for four or
five increments of every P1 solve).

## Mutations

Each on a scratch copy with its own target dir, running `sa_j2_order.rs` and the default-bits file.

| id | mutation | expected catcher |
|---|---|---|
| MU1 | `mat_j2.rs` line 131 as above (the control N1 is run in this copy; in the copy the file asserts the failure instead) | N1; also the existing T1, T2 plastic rows |
| MR1 | `set_changed` always false in `attempt_with` | D1 (`yielded` changes, flag false) and Q1 (S3) |
| MR2 | `set_changed` always true for `k >= 1` | Q2 (no informative pair) |
| MR3 | the recording on although the option is off | B1 (vectors not empty) |
| MR4 | `yielded_points` compares the trial with the previous trial instead of the committed state | D1 (the last `yielded` of S1, S2) |

## Decision rule

**Done** if B0, B1, D1, Q1, Q2, Q3, N1, N2, N3 are met, every mutation is caught and the gates (rcfd-fem tests, the
baseline-hash test, clippy, fmt, docs_site) pass. **Partial** if any check is missed (a miss becomes an ignored `*_known_miss`
test with the limit intact and the number recorded) or a clause of the acceptance is not run. N4 is a report.

## Limits of this item

The reading is a bound on residuals, not an order: it does not say `p >= 1.8`; the item allows that. `C = 10` is read off the
four solves (disclosed), not derived. The set flag is the set of points with a changed state; it does not see a change of the
set that leaves the states equal. The diagnostic is not wired into the case file or the CLI. The solves are those of ADR 0475
on the 2 x 2 octant and the 16-element beam only. F-600 (contact) is not done here; it can reuse D1 with its own flag.

## Addendum 1: results (2026-11-14)

Commits: registration `fd41a52b`; B0 record on the base `941aa3ff` (before any library code); library and tests `d79632e3`
and `654147ab` (a clippy form of one comparison: `if b.is_nan() || b > ...` for `!(b <= ...)`, same meaning). Every registered
run was made once, on `d79632e3`; N1 and the mutations on a scratch copy of the same files.

Library change, `crates/rcfd-fem/src/nonlinear.rs`, insertions only (47 lines, no existing line changed): `NewtonOptions`
field `record_active_set` (lines 963 to 965), its default and builder (978, 981 to 985), `LoadStep` fields `yielded` and
`set_changed` (1038 to 1043), `Attempt` fields and the function `yielded_points` (1082 to 1084, 1089 to 1104), the locals and
the guarded record in `attempt_with` (1313 to 1314, 1342 to 1348, 1355 to 1356), the two fields in the `LoadStep` literal of
`solve_with` (1694 to 1695).

| check | limit | measured | met |
|---|---|---|---|
| B0 default bits (5 solves) | digests printed on the base, equal after | equal (S1 `fa47eed38c0fdef8`, S2 `342556fa7f12f899`, S3 `54fdcd5cd089c65b`, P1 1.25 `1c361cb47319fc47`, P1 5 `88556e1fcea7bf0f`) | yes |
| B1 opt-in, record off and on | digests of B0; vectors empty off, filled on | all ten equal; empty / filled | yes |
| D1 diagnostic, nine solves | all | all; last `yielded` 648 of 648 (S1) and 1296 of 1296 (S2) plastic points | yes |
| Q1 Q(10, 1e-13), nine solves | 0 violations, finite | 0 of 19, 23, 49 (S1, S2, S3) and 11, 14, 20, 31, 43, 55 (P1) read pairs | yes |
| Q1 least C that would pass | (report) | S1 0.969, S2 1.181, S3 1.958, P1 0.220, 1.083, 1.473, 1.701, 1.561, 3.321 | n/a |
| Q2 informative, with set changes | all | 0 uninformative increments; increments with a set change 3 of 8, 5 of 10, 12 of 20, 1 of 5, 2 of 6, 4 of 8, 8 of 12, 12 of 16, 16 of 20 | yes |
| Q3 largest ratio | finite, `<= 10` | 3.3206 | yes |
| N1 continuum tangent (MU1, scratch copy) | fails on all nine | fails on all nine: S1 128 of 147 pairs violate (least C `2.4e9`), S2 554 of 596 (`3.3e9`), P1 39 of 50, 132 of 151, 371 of 405, 885 of 948, 1390 of 1513, 1875 of 2069 (`2.2e9 .. 4.7e9`); S3 does not converge (load step 19, 5 halvings, residual `2.7e-9`) | yes |
| N2 no exemption (consistent tangent) | fails on at least S3 | 3 pairs violate | yes |
| N3 Q(1, 1e-13) | fails on S2 and S3 | S2 1 pair, S3 11 | yes |
| N4 order of the last three above `1e-13` from the last set change (report) | none | least 1.279, 1.279, 1.274 (S1, S2, S3), 1.435, 1.415, 1.354, 1.332, 1.354, 1.361 (P1); undefined (fewer than three) in 0, 1, 0, 4, 4, 4, 5, 5, 5 increments | n/a |

The acceptance is met by the second alternative of the item (the bound on every increment). The order of three residuals is not
>= 1.8 under the counting tried (N4: it stays at 1.27 to 1.44, since the residuals before the asymptotic stretch are included,
and it does not exist in 4 to 5 increments of each P1 solve); the reading does not claim it.

Mutations (scratch copy `f560-scratch/mut`, own target dir):

| mutation | caught by |
|---|---|
| MU1 | N1 (the control, passes there); Q1 S1 (128 of 147) and S2 (554 of 596), B1 digests; Q1 and D1 panic at S3 (no convergence) |
| MR1 `set_changed` always false | D1 (all nine), Q2 (no set change), Q1 S3 (3 of 61 pairs) |
| MR2 `set_changed` always true | Q2 (every increment uninformative) |
| MR3 recording although off | B1 (vectors not empty, five solves) |
| MR4 trial compared with the previous trial | **not caught** (all registered checks pass) |

MR4 survives. Diagnosis: states are compared bitwise, and in these solves a point that yields at iteration `k - 1` or `k`
differs from the other iterate's trial in some bit (the plastic strain follows `u`), so the mutated set is the union of two
consecutive yielded sets; in monotonic loading no point leaves the set between iterations, and union and set coincide. A check
that separates them needs a point that leaves the set during an increment (unloading; S4 of ADR 0475). Not added after the
run (limits are not changed after a run); follow-up below.

Gates (2026-11-14, release, worktree `f560`): `cargo test -p rcfd-fem --release` all test binaries ok (185 result lines, none
failed), `cargo test -p rcfd-api --release --test baseline_hashes` 3 passed (unchanged), `cargo clippy -p rcfd-fem
--all-targets -- -D warnings` clean, `cargo fmt -p rcfd-fem --check` clean, `cargo test -p rcfd-cli --release --test
docs_site` 2 passed.

**Deviations.** (1) The probe before registration (disclosed above) was run with the diagnostic in a scratch copy; the
registered runs are separate. (2) The item's floor `1e-14` is not used: the measured rounding peak is `1.45e-14` (D1). (3) N1
is an ignored test that passes only in the MU1 copy; on the real tangent it fails by design. (4) MR4 was not caught.

**Decision by the registered rule: partial.** Every check is met, N1 included; one mutation (MR4) was not caught, and the rule
requires every mutation to be caught for done.

### Remaining questions

1. **MR4 (a set that loses a point).** Options: (a) accept; (b) a follow-up check on a load and unload solve (S4 of ADR 0475
   or the bar U4 with reversal) asserting `yielded[k]` against an independent count from the states, which separates the
   union from the set. Recommendation: (b), follow-up 1 below.
2. **F-600.** The reading Q(C, F) needs only residuals and a per-iteration changed flag; for contact the flag is the change of
   the contact status set. Recommendation: F-600 registers the same form with its own `C` read off its solves.
3. **The order itself.** Not attainable on these solves (N4); keep the bound as the reading of record for plasticity.

## For the lead: roadmap entries

Proposed status of F-560: **partial**. Proposed note:

> ADR 0515, 2026-11-14: reading Q(C, F) of the Newton convergence order for an active-set change: every pair of consecutive residuals r_k, r_{k+1} (k >= 1) whose correction did not change the set of yielded integration points satisfies r_{k+1} <= max(C r_k^2, F), C = 10, F = 1e-13 (the rounding of the residual peaks at 1.45e-14), with an informative pair in every increment; opt-in record on the driver (NewtonOptions::record_active_set, LoadStep::yielded and set_changed, 47 inserted lines in nonlinear.rs, results bit-identical with it on or off). Met on S1, S2, S3 and the six P1 beams of ADR 0475: no violation in 265 read pairs, least C 0.22 .. 3.32 (S1 0.97, S2 1.18, S3 1.96); the continuum tangent (MU1) violates it on all nine solves (S3 does not converge); without the exemption of the set-change pairs S3 fails, with C = 1 S2 and S3 fail. The order of the last three residuals stays 1.27 .. 1.44 and does not exist in 4 to 5 increments of each P1 solve, so the reading is a bound, not an order. Not caught: the mutation that compares the trial with the previous trial instead of the committed state (equivalent in monotonic loading); follow-up F-NEW. The reading is general; F-600 can reuse it with the contact status as the set.

Follow-up 1. Title: "Active-set record of the Newton driver on a load and unload solve". Deps: F-560. Acceptance:

> Every limit not stated here is registered in the ADR before the run. On a J2 solve in which integration points leave the yielded set during an increment (the load and unload sphere S4 of ADR 0475 or the bar of ADR 0475 U4 with a reversal), the recorded yielded count of every iteration equals the number of points whose trial state differs from the committed state counted independently from the states, some iteration of some increment records a smaller count than the one before it, and the mutation of ADR 0515 MR4 (trial compared with the previous trial) is caught.
