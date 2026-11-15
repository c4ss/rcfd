# ADR 0619: F-690 registration: early acceptance at the rounding bound of the Newton residual (a stagnation test)

Status: **REGISTERED 2026-11-15, written before any code of F-690 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-690 was written in the branch. Disclosed: an instrumented scratch
copy of the base for the audit (`w49i-work/f690-scratch/audit`, script `tools/instrument.py`; logs `audit-run.log`,
`audit-events.log`, `audit-p1-run.log`, `audit-p1.log`; analysis `tools/analyse.py`); its numbers are quoted below. It changes
no value and computes no check of the table; it measures the base (ADR 0604's iterations) and where a candidate rule would act.
Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-690 (deps F-571), the follow-up proposed by [ADR 0604](0604-f571-registration-rounding-floor-of-the-newton-residual-under-translation-and-rotation.md)
("For the lead", and its open question 3 (c)). Number 0619 given by the lead. Branch `f690-work` from the local main tip
462e7f81 (F-571 / ADR 0604 with its amendments, F-608's line search, F-596, F-631 / F-681), worktree `w49i-work/f690`,
scratch `w49i-work/f690-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited (NC1b of
`sa_nl_inertia_reference_after_f571.rs` among them); defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and
mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The roadmap files are not edited (the
entries go in "For the lead"). Every choice below that the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), except where stated as open.

## What F-690 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. A step at the rounding floor of ADR 0604 is accepted in at most a registered number of corrections below max_iterations; S3 and the pendulum of ADR 0479 at the defaults run in at most half of ADR 0604's iterations; the bits of every existing default-bits test are unchanged, or the changed ones are listed for the maintainer's decision.

Clauses: **c0** every limit registered here; **c1** a step at the floor accepted in at most `N_c` corrections, `N_c <
max_iterations = 20`; **c2** S3 (ADR 0479 amendment 2) and the pendulum (ADR 0479 P0 to P2) "at the defaults" in at most half
of ADR 0604's iterations; **c3** the bits of every existing default-bits test unchanged, or the changed ones listed.

Readings, registered (taken on the maintainer's standing instruction, take the recommendation, except item 1's default flip):

1. **"At the defaults" and c3.** A stagnation test that is on by default necessarily changes the bits of every step ADR 0604
   accepts at its last iterate (that is its purpose), and the audit below finds two such steps in the crate's non-ignored tests:
   S3's step 50 (`sa_nl_rounding_floor` A1, whose limits would still hold) and NC1b (`sa_nl_inertia_reference_after_f571`,
   which asserts `iterations = max_iterations = 20` and would fail). House rule 3 and the brief ("prefer an opt-in and list
   them"; NC1b "keep it opt-in or list it as an open question, do not edit the test") therefore make the test **opt-in**
   (`NewtonOptions::stagnation`, default `false`). c2 is measured with every option at its default except the opt-in switched
   on. Whether to flip the default is listed as an open question with the tests it would change (c3's "listed" branch); it is
   not taken here (it changes default results).
2. **"ADR 0604's iterations".** Measured on the base 462e7f81 with the driver defaults (the audit, disclosed): the sum of
   `StepReport::iterations` (no halvings occur, so every attempt counts). S3: **118** (steps 1 to 49 converge by the relative
   test in exactly 2 corrections each, 98; step 50 is accepted at the bound at its 20th). Pendulum: **37367** over 1940 steps
   (1854 attempts accepted at the bound with 20 corrections, 86 by the relative test).
3. **S3's half cannot be met literally.** Half of 118 is 59 < 98: the 49 steps that converge by the relative test already take
   2 corrections each, the least the energy test of ADR 0404 D5 allows (it compares the last correction with the first, so
   `k >= 2` for a step that moves), and any test that keeps their bits (c3) keeps those 98. So for S3 c2 is read as the
   iterations of the steps ADR 0604 accepted at the bound (the cost its open question 3 names): at most half of theirs (20, so
   `<= 10`). S3's total is checked against what the reading implies (`<= 98 + 10`), and the literal total is recorded. The
   pendulum is checked literally (`<= 37367 / 2`).

## What was read

`AGENT_RULES.md`; the brief; ADR 0604 whole (D1, D2: the bound `b = eps |(|K| |u|)|_free` tested at the last iterate only,
because tested at every iterate it would have stopped 12 iterates of ADR 0514's `n1e_n2e_trapezoidal_negative_control` earlier
than the relative test did (MB4: that test's printed N1e, N2e change); amendments 1 and 2; addendum 2: NC1b accepted at 20
iterations, relative residual `1.392e-10`, `rn = 4.46e-16`, `b = 4.29e-15`; open question 3); ADR 0479 (S3, the pendulum P0 to
P2); ADR 0514 (N1e, N2e); `crates/rcfd-fem/src/nonlinear.rs` (`NewtonOptions`, `contribute_with`, `contribute_split`,
`rounding_bound`, `Problem::attempt_with`); `nl_transient.rs` (`StepReport::iterations`: the corrections of the converged
sub-steps); the tests `sa_nl_rounding_floor.rs`, `sa_nl_inertia_reference_after_f571.rs`, `sa_emc.rs` (the spinning bar and
N1e).

## The base, measured (audit, disclosed)

The instrumented copy logs, for every attempt that has a contribution in the residual reference at some iterate, per iterate
`k`: `rn`, `b` of ADR 0604 D1 (computed at every such iterate; a read only), the relative residual and the energy test, and how
the attempt ended (relative test, bound at the last iterate, failure). `cargo test -p rcfd-fem --release --no-fail-fast` on it:
1355 passed, 0 failed, 347 ignored; 17752 logged attempts: 17744 ended by the relative test, 2 at the bound (S3 step 50, NC1b),
6 failed (`sa_nl_transient` `h1_h2_halvings_and_the_error_with_the_time`, `max_iterations = 1`, as registered there). The
ignored P1 of ADR 0604 run once on the same copy (159 s): 1940 attempts, 1854 at the bound, 86 by the relative test, 37367
iterations, the P1 numbers of ADR 0604 to the printed digits.

What a step at the floor looks like. S3 step 50: `rn` `2.73`, `1.4507e-7`, then `1.451095165409503e-7` at every iterate 3 to 20
(the iterate is a fixed point of the arithmetic: the correction computed from a residual at the rounding level leaves `u`
unchanged or cycles it in its last bits), `b = 1.82e-6`, relative `1.64e-9`. NC1b: within the bound from `k = 2` (`rn` `2.41e-15`,
`b` `4.27e-15`), then `4.458e-16` with variations in the fifth digit at iterates 3 to 20. The pendulum's floor attempts: within
the bound from `k = 3` (`rn/b` about 0.1), then `rn` varying by up to 15% from iterate to iterate.

## Design

### D1. The stagnation acceptance (opt-in)

With `NewtonOptions::stagnation` on and a contribution in the residual reference at the iterate (ADR 0604's `bounded`), at an
iterate `k` with `3 <= k < max_iterations`, after the relative test of ADR 0404 D5 has failed at that iterate, the attempt is
accepted if

    rn_j <= b_j  for j = k-2, k-1, k     (three consecutive iterates within the rounding bound of ADR 0604 D1),
    rn_k >  rn_{k-2} / 2                 (the last two corrections did not halve the residual),
    e_last <= tol_e e_first              (the energy test, as in ADR 0604 D2).

The relative test is tried first at every iterate (unchanged), and ADR 0604's acceptance at `k = max_iterations` is kept as it is.
The residuals and step lengths recorded are unchanged in meaning; an attempt accepted by this test reports its relative residual
above the tolerance, as one accepted by ADR 0604 D2 does.

**Derivation (first principles).** Within the rounding bound, `u` is the exact solution of a problem perturbed by one unit
roundoff per component plus an equal evaluation error (ADR 0604 D1), so accepting there is ADR 0604's acceptance; the question is
only when to stop trying for a smaller residual. A Newton iteration in its convergence region contracts the residual: in the
quadratic phase `rn_{k+1} <= C rn_k^2`, and with `rn_k` at the rounding level the factor `C rn_k` is far below 1/2; a convergent
modified-Newton or line-searched iteration contracts by a fixed factor below 1. An iteration that does not halve its residual
over two corrections while every iterate is already within the rounding bound has no contraction left to show: the corrections
are computed from a residual dominated by rounding and the iterate stays at the floor (S3: the same `rn` to 16 digits at 18
iterates). The factor 1/2 is the conventional separation of a contraction from stagnation (the monotonicity test with contraction
factor `Theta < 1/2` in Deuflhard, *Newton Methods for Nonlinear Problems*, 2004, ch. 3, cited from memory, not re-read; used
here only as the threshold of the derivation above). No tuned constant otherwise: `eps` is `f64::EPSILON` as in ADR 0604.

**Why two corrections and not one (disclosed: the audit changed the window).** The first form written down before the audit
compared consecutive iterates (`rn_{k-1}, rn_k <= b`, `rn_k > rn_{k-1}/2`, `k >= 2`). The audit found it would end one attempt
earlier than the relative test does: ADR 0514 N1e (`sa_emc` `n1e_n2e_trapezoidal_negative_control`, Newmark, residual tolerance
`1e-7`, in its blow-up), iterates `rn` `4.14e-7`, `3.93e-7`, `3.13e-7` at `k = 4, 5, 6` with `b = 3.79e-6`: at the floor, the
relative test passed at `k = 6` by the variation of the rounding (relative `8.03e-8`), and the one-correction form would have
accepted at `k = 5` (the same event that made ADR 0604 gate its bound to the last iterate). With the window of two corrections
the relative test gets one more iterate, and the audit finds no attempt of the non-ignored tests ended before the relative test
(B3). This is the same kind of audit-motivated gate as ADR 0604 D2; the factor 1/2 was not changed.

What the opt-in still does to an attempt that the relative test would accept later (disclosed, ignored P1): 4 of the
pendulum's 86 attempts that end by the relative test are at the floor first (`rn/b` about 0.1 at three consecutive iterates,
relative residual `1.1e-10` to `1.2e-10`, then a lucky `8.6e-11` at `k = 6` or `8`); with the opt-in they are accepted at `k = 5`.
These are floor attempts by the bound's definition; they are why the window cannot prove "never before the relative test" in
general, only measure it (B3, ST4).

### D2. Where (shared solver file `nonlinear.rs`)

A field `stagnation: bool` of `NewtonOptions` (after `line_search`; `#[non_exhaustive]` struct, so no external literal breaks),
`false` in `NewtonOptions::new`, a builder `stagnation(on)`. In `Problem::attempt_with`: a vector of `(rn, within)` per iterate,
pushed only when the option is on, and the acceptance of D1 between the relative test and ADR 0604's last-iterate test. With the
option off no new operation is executed (the vector stays empty, `rounding_bound` is not called before the last iterate), so
every run without the opt-in has its bits. `nl_transient.rs` is not changed: `NlTransientOptions::newton(NewtonOptions::new(1)
.stagnation(true))` reaches it. Not offered: a case-file or CLI key, schema or MCP changes; the static driver without a
contribution in the reference (no `bounded`, as ADR 0604).

## Checks (limits fixed now)

File `crates/rcfd-fem/tests/sa_nl_stagnation.rs` (new; helpers copied from `sa_nl_rounding_floor.rs`,
`sa_nl_inertia_reference_after_f571.rs`, `sa_emc.rs`, none edited). "Opt-in" = `NewtonOptions::new(1).stagnation(true)`, every
other option at its default.

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| ST1a | c2 (set-up) | S3, opt-in, `dt = T_lin/200`, 50 steps: every step `Ok`; `max |P_n - P_0|/|P_0|`; halvings | all `Ok`; `<= 1e-11`; `0` | ADR 0604 A1: `1.3e-14`, 0 |
| ST1b | c2 | S3 opt-in total iterations (the base run in the same test must measure 118, a set-up check) | `<= 108` (98 + 10, reading 3); recorded against the literal 59 | audit: step 50 accepted at `k = 4` by the rule: 102 |
| ST1c | c1, c2 | S3: the iterations of the steps the base accepts at the bound (the base's steps with final residual `> 1e-10`) with the opt-in, against half of the base's | `<= 10` | 4 |
| ST1d | c1 | S3: corrections of each such step | `<= N_c = 10` | 4 |
| ST1e | c3 (opt-in) | S3: the displacements of steps 0 to 49 (those before the first floor step) bit-equal to the base run | equal | the rule needs three iterates within the bound; in the audit no iterate of steps 1 to 49 before their last is within it |
| ST1f | accuracy | S3 opt-in against S3 at residual tolerance `1e-6` (no opt-in): `max_n |u_n - u'_n|_inf / max_n |u'_n|_inf` | `<= 1e-9` | ADR 0604 A2 `3.6e-18`: both at the floor |
| ST2 | c1 | NC1b's step (the Shell3 strip at `h_f = h/1024` from its zero crossing), opt-in: `Ok`, halvings, corrections, final relative residual | `Ok`; `0`; `<= N_c = 10`; `<= 1e-7` | audit: within the bound from `k = 2`, the rule at `k = 5`; `1.39e-10` |
| ST3 | c1, c2 | the pendulum of ADR 0479 (`dt = 0.005`, 1940 steps), opt-in: every step `Ok`; crossings; period; energy; halvings; total iterations; corrections of a step | all `Ok`; `>= 9`; `<= 1e-2`; `<= 1e-3`; `<= 19`; `<= 18683` (`37367 / 2`, rounded down); `<= N_c = 10` | ADR 0604 P1 numbers; emulation on the audit log: 10568, worst 7 |
| ST4 | c3 (opt-in) | ADR 0514 N1e set-up (spinning bar, Newmark, residual `1e-7`, 600 steps) with the opt-in against without: the final state bytes | equal | audit: no attempt of it meets D1 before its relative test (the one-correction form would at one) |
| ST5 | c3 | `NewtonOptions::new(1).stagnation` and `NlTransientOptions::new(dt).newton.stagnation` are `false` | `false` | D2 |
| B1 | c3 | `cargo test -p rcfd-fem --release` of the 72 `*_default_bits` files and `sa_nl_rounding_floor`, `sa_nl_inertia_reference_after_f571`, `sa_nl_inertia_reference`, `sa_nl_transient`, `sa_emc`, `sa_dg`, `sa_dgm` (NC1b's 20 iterations among them) | pass | D2: no operation with the option off |
| B2 | c3 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no case path changes |
| B3 | c3, design | audit (before registration, on the base, not re-run): attempts of the crate's non-ignored tests that D1 would end before the relative test | `0` | measured `0` |
| B4 | c3 (list) | audit: attempts of the non-ignored tests D1 would change if it were on by default (the list for the maintainer) | recorded | measured 2: S3 step 50 (`sa_nl_rounding_floor` A1 test), NC1b (`sa_nl_inertia_reference_after_f571`) |

ST1, ST2, ST4, ST5 run in seconds in release; ST3 is an ignored heavy record (ADR 0604's P1 took 67 s alone and 159 s on the
shared machine), run once with `--ignored`; the fast tests on the same code path are ST1 and ST2.

## Mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MS1 | the opt-in ignored (the test on whenever `bounded`) | B1 (NC1b's `iterations = 20`) |
| MS2 | the window of one correction (the pre-audit form: `k-1, k` within the bound, `rn_k > rn_{k-1}/2`, `k >= 2`) | ST4 (N1e's attempt at `k = 5`) |
| MS3 | the halving condition dropped (three iterates within the bound suffice) | none expected (doubt: of the audit's 17744 relative-converging attempts none has three consecutive iterates within the bound before its last iterate, one has two (N1e's), 3296 have one) |
| MS4 | `b` replaced by 0 in the stagnation branch | ST1c, ST1d, ST2 (back to 20 corrections) |
| MS5 | the energy test dropped from the stagnation branch | none expected (doubt: at `k >= 3` within the bound the energy ratio is `1e-27` or less in the audit) |
| MS6 | the stagnation branch tested after ADR 0604's last-iterate branch with `k <= max_iterations` (an off-by-one of the gate) | none expected (the last iterate is accepted by either branch with the same `u`) |

## Decision rule

- **done:** ST1a to ST1f, ST2, ST3, ST4, ST5, B1, B2, B3 met; MS1, MS2, MS4 caught; fmt and clippy clean. (Under readings 1 to
  3; the default flip stays open, c3's listed branch.)
- **partial:** ST1a, ST2, ST4, ST5, B1, B2, B3 met and any of ST1b to ST1f, ST3, or a catch of MS1, MS2, MS4 missed (diagnosed,
  a known miss).
- **todo:** ST1a or ST2 missed, or any of ST4, ST5, B1, B2 missed (a bit of a default or of a relative-converging attempt
  changed).

## Limits (what is not covered)

- Opt-in: the driver defaults keep ADR 0604's cost (20 corrections per floor step) until the maintainer flips the default.
- The window and the factor are argued, not proved: an attempt at the floor whose relative test would pass by chance later can be
  accepted earlier with the opt-in (4 of the pendulum's 86); B3 measures this only on the crate's tests.
- The energy-momentum and discrete-gradient steps have their own Newton loops (ADR 0604 amendment 2) and do not get the test.
- The static driver without a contribution in the reference keeps the ADR 0404 test; S3's literal half is not reachable (reading
  3).

## Open questions

1. **Flip the default?** (Not taken: it changes default results.) Options: (a) keep opt-in (recommended until the maintainer
   decides; no bit changes); (b) on by default: changes S3 step 50 of `sa_nl_rounding_floor`'s A1 test (its limits still hold
   by the audit's numbers) and NC1b of `sa_nl_inertia_reference_after_f571` (asserts 20 iterations: it would need an ignored
   record and a copy, the scheme of ADR 0604 amendment 1), plus the ignored records P1 (ADR 0604) and the ADR 0479 records;
   the audit finds no other attempt of the non-ignored tests changed. Recommendation: (b) after the maintainer's decision, with
   the record-and-copy of NC1b.
2. **Reading of "ADR 0604's iterations" for S3** (taken on the maintainer's standing instruction, take the recommendation): the
   floor steps' iterations, because the literal total cannot be halved by a bit-keeping test (reading 3).
