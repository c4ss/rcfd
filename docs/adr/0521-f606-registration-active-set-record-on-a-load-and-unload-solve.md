# ADR 0521: F-606 registration: the active-set record of the Newton driver on a J2 load and unload solve, against a count made independently from the states

Status: **REGISTERED 2026-11-14, written before any code of F-606 and before any registered run of its checks.** Not yet
accepted by the maintainer. A probe was made first (disclosed below, not a registered run) to choose the solves; the registered
checks are made once, after this commit. Results go in the addenda at the end; nothing above them is changed after a run.

Roadmap item F-606 (deps F-560), follow-up 1 of [ADR 0515](0515-f560-registration-newton-order-reading-for-an-active-set-change.md).
Number 0521 given by the lead. Branch `f606-work` from `d0cf9a05`, worktree `w49i-work/f606`, scratch `w49i-work/f606-scratch`.
House rules (`w49i-work/AGENT_RULES.md`) apply; the brief of the lead overrides rule 9 for the F-606 entry of `docs/ROADMAP.md`
and `docs/roadmap.json` (a separate last commit; the svgs are not regenerated).

## What F-606 asks

> Every limit not stated here is registered in the ADR before the run. On a J2 solve in which integration points leave the
> yielded set during an increment (the load and unload sphere S4 of ADR 0475 or the bar of ADR 0475 U4 with a reversal), the
> recorded yielded count of every iteration equals the number of points whose trial state differs from the committed state
> counted independently from the states, some iteration of some increment records a smaller count than the one before it, and
> the mutation of ADR 0515 MR4 (trial compared with the previous trial) is caught.

## What was read

ADR 0515 (D1, D2, the diagnosis of MR4: with bitwise comparison and monotonic loading the trial-versus-previous-trial set is
the union of two consecutive yielded sets, and union and set coincide), ADR 0475 (D3, D5 for S4, D6 for U4), `nonlinear.rs`
(`attempt_with`, `yielded_points`, `Contribution`, `DriverExtensions`, `element_trial_state`, `initial_states`),
`tests/sa_j2.rs` (`Ramp`, the S4 test, `sphere_points`), `tests/sa_j2_order.rs` (the sphere builder, `Checks`).

## Probe, before registration (disclosed, not a registered run)

A scratch test (`w49i-work/f606-scratch/probe606.rs`, untracked, removed from the worktree before this commit) solved S4
(ADR 0475 D5: the bore pressure `-g(lambda) f1` as a driver contribution, `g = 1 - |2 lambda - 1|`, `p1 = p(1.25 a)`) with
`record_active_set(true)` for 2, 3, 4, 5, 6, 7, 9, 10, 11, 15 and 16 load steps, captured the iterate `u_k` of every iteration
through a recording contribution, and counted per iteration the points whose trial state differs bitwise from the committed one,
chaining the committed states from `initial_states`. What it showed:

- With 16 steps and with 2 steps there are no halvings, the contribution is called once per iteration (plus one call after the
  last increment) and the independent count equals the record in every iteration (70 and 14 iterations). With the other step
  counts halvings occur (failed attempts call the contribution too and the calls cannot be matched to iterations without more
  bookkeeping), so those are not used.
- S4 with 16 steps: increments 1 to 5 yield nothing; 6 to 8 give `[0, 216, 216, 216, 216]`, `[69, 343, 432, ...]`,
  `[130, 540, 648, 648, 648, 648]`; increment 9, the first unloading increment, gives `[189, 0, 0, 0, 0]`; the rest `[0, 0, 0,
  0]`. The count falls from 189 to 0 at the first correction. The 189 at iteration 0 is the set of points at the yield surface
  whose re-evaluation at the committed displacement changes some bit of the state (the committed point sits at `f = 0` to
  rounding, the evaluation finds `dg > 0` of rounding size); the fall to 0 is the elastic unloading of every point in one
  correction. So the decrease exists in the sense of the item, but it starts at a count that is rounding chatter.
- S4 with 2 steps (one up, one down, no halving): increment 1 `[0, 432, 540, 648, 648, 648]`; increment 2 `[195, 88, 4, 3, 0,
  0, 0, 0]`. Here the unloading happens gradually, through real overshoot of the first corrections: 88, then 4, then 3, then 0.
  This is a decrease between two iterations that are both corrections.

The bar U4 with a reversal was not probed: the bar of ADR 0475 D6 loads by a prescribed displacement that is linear in the
load factor and has no reversal; a reversal needs a contribution or a displacement history the driver does not offer there.
S4 is the first alternative of the item and is used.

## Design

### D1. The independent count (registered)

The test reads the iterates and the states through public interfaces only, never through `LoadStep::yielded` or
`set_changed`:

1. A contribution `Recorder` (the `Ramp` of ADR 0475 D5, `f = -g(lambda) f1`, no tangent) that also appends a copy of `(u,
   lambda)` to a `RefCell<Vec>` at every call. The driver calls the contributions once per iteration at the attempt's `u` and
   load factor (`attempt_with`), so the call list is the list of iterates.
2. For every increment `s` of the result, in order, with `committed` starting at `initial_states(model)`: for each of its
   `residuals.len()` iterations take the next recorded `u`, compute the trial states of every element with
   `element_trial_state(Hex20, coords, material, u_e, committed[e])` (`u_e` gathered with `element_dofs(model.dof_map(), nodes)`),
   and count the points (13 state values each) with any value whose bits differ from the committed point (count A, the
   definition of ADR 0515 D1) and the points whose `alpha` (value 12) is greater than the committed one (count B, the plastic
   flow definition `dg > 0`). Then the trial of the last iteration becomes the committed state for the next increment.
3. The point sets of two consecutive iterations are compared point by point, giving the independent set-changed flag.

The material routine and the iterates are shared with the driver (nothing else is available from outside); what is independent
of the record is the path (iterate capture by a contribution, trial states by the public element function, comparison and
counting by the test, the chaining of committed states).

### D2. The solves

S4a: ADR 0475 D5 as written, 16 load steps (8 up, 8 down), `NewtonOptions::new(16).record_active_set(true)`, default driver
options, Hex20 octant of the sphere, perfectly plastic J2 (`sigma_y = 2e8`, `E = 2e11`, `nu = 0.3`), `p1 = p(1.25 a) sigma_y`
(`p(c) = 2 ln c + (2/3)(1 - c^3/8)`). S4b: the same with 2 load steps. The sphere builder and `Ramp` are copied (an integration
test cannot import another; ADR 0475's files are not edited); `nodal_load_vector` of the model with the bore pressure `p1`
gives `f1`.

## Checks (limits fixed now)

Counts are integers; a limit "equal" is exact. Every residual of every increment is asserted finite before anything is
aggregated, and the aggregates are sums of integer mismatches, so no `f64::max` over NaN is involved.

| id | clause | check | limit | estimate behind it |
|---|---|---|---|---|
| A0 | alignment | on S4a and S4b: every increment has `halvings == 0`; the number of recorded calls is the total number of iterations plus 1 (the call after the last increment); the load factor of every call of an increment equals the increment's `load_factor` to `1e-12` (the load factor of the extra call is not read) | all | probe: 71 calls for 70 iterations, 15 for 14, no halvings |
| A1 | acceptance, equality | the record `yielded[k]` equals the count A of D1, in every iteration of every increment of S4a and S4b | 0 mismatches of 84 iterations (70 + 14); `yielded.len() == residuals.len()` in every increment | probe: equal everywhere |
| A2 | equality, flag | the record `set_changed[k]` equals the independent flag of D1 step 3 (`set_changed[0]` false), in every iteration of S4a and S4b | 0 mismatches | the flag is the inequality of consecutive sets |
| A3 | equality, flow definition | the count B equals count A in every iteration of S4a and S4b | 0 mismatches | probe: A = B; a point whose trial state differs has `dg > 0` since the state values besides `alpha` follow from the flow |
| A4 | chain | the chained committed states after the last increment equal `result.states` bitwise, in S4a and S4b | equal | the driver returns the trial of the last iteration as the state |
| A5 | the count is not a proxy | in S4a and S4b the last increment records a last `yielded` of 0 while `result.states` holds more than 0 points with `alpha > 0` (the count of points that ever yielded is 648 in S4a) | `0` and `> 0` | probe: unloading ends elastic |
| D1 | acceptance, a smaller count than the one before | S4a: some iteration `k >= 1` of some increment records `yielded[k] < yielded[k-1]` (probe: 189 to 0 in increment 9); S4b: some increment has two consecutive recorded counts `yielded[k+1] < yielded[k]` with `k >= 1`, that is a decrease between two corrections (probe: 88 to 4 to 3 to 0 in increment 2) | both | probe |
| D2 | where the sets leave | S4a: the number of increments in which `set_changed` is true at some iteration, and the number of increments with a decrease of A, are both `> 0`; S4b: the independent A is strictly decreasing over at least 3 consecutive iterations of one increment | all | probe: in S4a increment 9 is the only increment with a decrease; S4b 195, 88, 4, 3, 0 |
| B1 | opt-in on the contribution path | S4b with `record_active_set` false and true: displacement, reactions, per-increment load factor, halvings, residuals, energies and states bit-identical (FNV-1a 64 digests equal); with it off, `yielded` and `set_changed` empty | equal; empty | ADR 0515 B1 covered the solves without a contribution; no library change here |
| F1 | finite | every residual of S4a and S4b finite | all | converged |

Not registered, on purpose: the bar U4 with a reversal (no reversal exists there, see the probe); a limit on the residuals or
the order (F-560 did that); any assertion on the 16-step run that rests on a count of rounding chatter at iteration 0 other than
that it is the same in the record and in the independent count (A1).

## Negative controls and mutations

Each mutation on a scratch copy of the worktree with its own `CARGO_TARGET_DIR`, running `sa_j2_unload.rs` (and, for
completeness, `sa_j2_order.rs`). In the mutated copy the new test file must fail; the unmutated tree passes.

| id | mutation | expected catcher |
|---|---|---|
| MR4 | `yielded_points` compared with the previous trial instead of the committed state (the mutation of ADR 0515; at iteration 0 the previous trial is the committed state) | A1 (and A2): mismatch at increment 9 of S4a (record 189 or more at iteration 1 against 0) and increment 2 of S4b |
| MR1 | `set_changed` always false | A2 |
| MR2 | `set_changed` true for every `k >= 1` | A2 |
| MR5 | the recorded count is the number of points with `alpha > 0` in the trial instead of those that differ from the committed state | A1, A5 |

MR4 is the one the acceptance requires to be caught. The reason it is expected to be caught: on the unloading increment the
points that are elastic at iteration `k` differ from the trial of iteration `k - 1` (where they were chattering or flowing) and
are equal to the committed state, so the mutated record counts them while the independent count does not.

## Decision rule

**Done** if A0 to A5, D1, D2, B1 and F1 are met, MR4, MR1, MR2 and MR5 are each caught, and the gates (the whole of `cargo
test -p rcfd-fem --release`, the baseline-hash test, clippy on `rcfd-fem`, `cargo fmt -p rcfd-fem --check`, the `docs_site`
test) pass. **Partial** if a check is missed (it becomes an ignored `*_known_miss` test with the limit intact and the number
recorded), a mutation is not caught, or a clause of the acceptance is not run. No library change is planned; if a read-only
accessor turned out to be needed, a default-bits test would be written first and this ADR amended before the run.

## Limits of this item

The independent count uses the same material routine and the same iterates as the driver, because nothing else can be had from
outside: it checks the bookkeeping of the record (what is compared with what, per iteration, across increments), not the J2
update. Only S4 is run, at two step counts; the bar U4 with a reversal is not (no reversal exists there). The record is read
through a contribution that observes `u`; the call sequence is relied on as in A0. The 16-step run has rounding chatter at
iteration 0 of the increments that start on the yield surface; it is counted as the definition of ADR 0515 D1 prescribes
(bitwise), and the 2-step run supplies the decrease between corrections. No case-file key is added.

## Addendum 1: results (2026-11-14)

Commits: registration `752d0e43`; test `3a9108bd` (`crates/rcfd-fem/tests/sa_j2_unload.rs`, 2 tests). No library change: the
record of ADR 0515 is read as it is, so no default-bits test is needed and `nonlinear.rs` is untouched. The registered run was
made once, on `3a9108bd`; the mutations on a scratch copy of the same tree (`f606-scratch/mut`, own target dir, script
`mutate.py`).

| check | limit | measured | met |
|---|---|---|---|
| F1 residuals finite | all | all, S4a and S4b | yes |
| A0 alignment | no halving, calls = iterations + 1, load factors equal | 71 calls for 70 iterations (S4a), 15 for 14 (S4b), no halving, load factors equal | yes |
| A1 record = independent count | 0 mismatches of 84 | 0 of 70 (S4a), 0 of 14 (S4b); vectors as long as the residuals | yes |
| A2 record of the flag = independent flag | 0 mismatches | 0 of 70, 0 of 14; `set_changed[0]` false everywhere | yes |
| A3 alpha-increase set = state-difference set | 0 mismatches | 0, 0 | yes |
| A4 chained states = result states (bits) | equal | equal, both | yes |
| A5 not a proxy | last yielded 0, points with `alpha > 0` more than 0 | 0 and 648, both | yes |
| D1 a smaller count than the one before | S4a: some iteration; S4b: between two corrections | S4a increment 9: `[189, 0, 0, 0, 0]`; S4b increment 2: `[195, 88, 4, 3, 0, 0, 0, 0]` (88 to 4 to 3) | yes |
| D2 decrease and set change | S4a: increments with decrease, with set change, independent decrease all > 0; S4b: strictly decreasing over 3 iterations | S4a 1, 4, 1; S4b 195, 88, 4, 3, 0 | yes |
| B1 opt-in on the contribution path | digests equal, iterates equal, vectors empty / filled | `0xa257e66efa0a5269` both; equal; empty / filled | yes |

Independent count and record agree in every iteration; in the 16-step run the only iteration-to-iteration decrease is the
unloading increment 9 (`189 -> 0`, the 189 being the rounding chatter at the yield surface described in the probe), and in the
2-step run the decrease is real overshoot: 88 points yield at the first correction of the unloading increment, 4 at the second,
3 at the third, none at the fourth.

Mutations (scratch copy, each run against `sa_j2_unload.rs` and `sa_j2_order.rs`):

| mutation | caught by |
|---|---|
| MR4 trial compared with the previous trial | **caught** by A1 (1 mismatch of 70 in S4a, 4 of 14 in S4b) and A2 (2 of 70, 1 of 14); `sa_j2_order.rs` does not see it (all its checks pass), as ADR 0515 found |
| MR1 `set_changed` always false | A2 (6 of 70, 7 of 14) and D2 (no increment with a set change); in `sa_j2_order.rs` D1, Q2, Q1 S3, Q3 |
| MR2 `set_changed` true for `k >= 1` | A2 (48 of 70, 5 of 14); in `sa_j2_order.rs` Q2 and N3 |
| MR5 count of points with `alpha > 0` | A1 (35 of 70, 8 of 14), A2, A5 (last yielded 648), D1, D2 |

Gates (2026-11-14, release, worktree `f606`): `cargo test -p rcfd-fem --release` all 194 test binaries ok, none failed; `cargo test -p rcfd-api --release --test baseline_hashes` 3 passed (unchanged); `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean; `cargo fmt -p rcfd-fem --check` clean; `cargo test -p rcfd-cli --release --test docs_site` 2 passed.

**Deviations.** (1) The solves S4a and S4b were chosen by the probe before the registration (disclosed). (2) The bar U4 with a
reversal was not run (no reversal exists there). (3) The decrease of the 16-step run starts from a count that is rounding
chatter of points at the yield surface; the real overshoot is in the 2-step run.

**Decision by the registered rule: done.** Every check is met, MR4, MR1, MR2 and MR5 are each caught, the gates pass.

### Remaining questions

1. **The chatter at iteration 0.** A point sitting on the yield surface after convergence re-evaluates at the committed
   displacement with `dg` of rounding size, so the bitwise definition of ADR 0515 D1 counts it as yielded at iteration 0 of the
   next increment (189 points in increment 9 of S4a, 69 and 130 in increments 7 and 8). Options: (a) keep the definition (the
   record is of the states; the independent count agrees); (b) a follow-up that defines the set by `dg > 0` above a tolerance.
   Recommendation: (a); a tolerance would change the record's meaning and no check needs it.
2. **The bar U4 with a reversal.** Needs a driver contribution or a displacement history; S4 covers the clause. Recommendation:
   leave it.

## For the lead: roadmap entries

Proposed status of F-606: **done**. Proposed note:

> ADR 0521, 2026-11-14: the opt-in active-set record of the Newton driver (ADR 0515) checked against a count made independently from the states on the load and unload sphere S4 of ADR 0475, with 16 steps and with 2 steps: the iterates are captured by a recording driver contribution, the trial states come from the public `element_trial_state` and the committed states are chained from `initial_states`; the recorded yielded count and the set-changed flag equal the independent ones in every iteration (0 mismatches of 70 and of 14), the chained states equal the result bits, the record is bit-neutral on the contribution path. The count falls within an increment: 189 to 0 at the first unloading increment of the 16-step run (rounding chatter of points on the yield surface), and 195, 88, 4, 3, 0 in the 2-step run (real overshoot of the corrections). The mutation of ADR 0515 MR4 (trial compared with the previous trial) is caught by the equality of the count and of the flag (1 and 2 mismatches of 70, 4 and 1 of 14), as are the flag always false, the flag always true and the count of points with alpha > 0. No library change; no case-file key. The bar U4 with a reversal was not run (it has no reversal); F-560 can be marked done since its uncaught MR4 is caught here.

Proposed note change for F-560: status partial to **done**, appending: "MR4 caught by F-606 (ADR 0521, 2026-11-14)." The lead decides whether the rule of ADR 0515 (every mutation caught) is read as met by this follow-up.
