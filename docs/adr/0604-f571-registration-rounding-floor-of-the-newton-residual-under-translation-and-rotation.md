# ADR 0604: F-571 registration: the rounding floor of the Newton residual of the total Lagrangian step under large translation and rotation (a rounding bound of the residual in the reference path)

Status: **REGISTERED 2026-11-15, written before any code of F-571 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-571 was written in the branch. Disclosed: two scratch probes (a test
file `zz_probe_f571.rs` copied into the worktree, run, and removed, never committed; copy and logs in
`w49i-work/f571-scratch`: `zz_probe_f571.rs`, `probe1.log` to `probe3.log`) and an instrumented scratch copy of the base for the
default-bits audit (`f571-scratch/audit`, logs `audit-run.log`, `audit-events.log`); their numbers are quoted below. None of
them computes a check of the table. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-571 (deps F-423, F-397), the follow-up proposed by [ADR 0479](0479-f423-registration-nonlinear-transient-newton-in-newmark-generalised-alpha.md)
(amendment 2, addendum 1, "For the lead"), referred to by [ADR 0514](0514-f570-registration-energy-momentum-conserving-step-for-nonlinear-transient-dynamics.md)
(Limits). Number 0604 given by the lead. Branch `f571-work` from the local main tip 43092983, worktree `w49i-work/f571`,
scratch `w49i-work/f571-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited (the ignored
record `s3_free_body_momentum_known_miss` of ADR 0479 stays as it is); defaults bit-identical; no edit of `baseline_hashes.rs`,
of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code; clean-room. The roadmap files are not
edited (the entries go in "For the lead"). Every choice below that the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-571 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. S3 of ADR 0479 (free Hex8, 50 steps) converges every step with the driver defaults, and MU6 of ADR 0479 is caught.

Clauses: **c0** every limit registered here; **c1** S3 of ADR 0479 amendment 2 (one free Hex8 `[1, 0.1, 0.1]`, `E = 2e11`, `nu =
0`, `rho = 7850`, started undeformed with the uniform velocity `(0.3, 0.2, 0.1) m/s` plus `10 m/s` along `x` at the four nodes of
`x = L`, Newmark average acceleration, `dt = T_lin/200`, 50 steps) converges every step with `NlTransientOptions::new(dt)`, i.e.
`NewtonOptions::new(1)`: 20 iterations, residual and energy tolerances `1e-10`, 5 halvings; **c2** MU6 of ADR 0479
("`in_residual_reference` ignored by the driver") is caught by an active check.

Readings, registered:

- **c1** "S3" includes its registered second limit `max |P_n - P_0|/|P_0| <= 1e-11` (ADR 0479 amendment 2): the copy A1 below
  checks both. "The driver defaults": every option of S3 as registered there, nothing set.
- **c2** "MU6 of ADR 0479": two forms are run, both must be caught. **MU6** the registered text, every call
  `c.in_residual_reference()` in the driver (`nonlinear.rs`) read as `false`; **MU6s** the form that ADR 0479's mutation script
  (`f423-scratch/mutate.py`) applied, the per-contribution block `if c.in_residual_reference() { ... }` of `contribute_with`
  disabled (`if false`), which left the reference `|f_int|` of the entry and was the form not caught.
- The defaults stay bit-identical (house rule 3), so the change must not alter any default run that converged before: shown by
  the default-bits tests and an audit (B1 to B3).

## What was read

`AGENT_RULES.md`; the brief; ADR 0479 whole (arithmetic 3 and 5, D1 to D4, amendments 1 and 2, addendum 1: S3 stopped at step 50
of 50 after 5 halvings, relative residual `1.169e-10`, energy ratio `2.8e-30`; MU6 not caught, "the free body's internal force
dominates the reference"); ADR 0514 (Limits: "the rounding floor of the total Lagrangian force ... sets the residual tolerance of
the bar"); the registration ADR 0603 of F-631 (branch `f631-work`, its D3 diff of `nonlinear.rs`: a hook
`Contribution::residual_reference_terms` read in the block `if c.in_residual_reference() { ... }` of `contribute_with` and
`contribute_split`); `crates/rcfd-fem/src/nonlinear.rs` (the kinematics `displacement_gradient`, `green_lagrange`, `contribute`,
`contribute_with`, `contribute_split`, `Problem::attempt_with`, `NewtonOptions`), `nl_transient.rs` (`Inertia`, `Kinematics`,
the options), the test `s3_free_body_momentum_known_miss` and the pendulum tests of `tests/sa_nl_transient.rs`;
`f423-scratch/mutate.py`, `mut_MU6_s3.log`.

## The floor, diagnosed (probes disclosed)

**Where it is not.** The roadmap title points at the internal force. Under a rigid translation `t` the total Lagrangian `H =
sum_a u_a (x) g_a` cancels `t sum_a g_a = 0` in rounding; the error is about `eps |u| |g|` in `H`, a force of about `eps |u| E V
|g|^2` (`E V |g|^2 ~ 2e11 x 0.01 x 12.5 = 2.5e10 N/m` for this element), `~1e-8 N` at `|u| = 2.7e-3 m`. Probe 1 measures
`|f_int(u) - f_int(u - t)|` (the shift by the displacement of node 0): `3e-11` to `1.1e-8 N` over 60 steps. The Green strain is
already formed from `H` without `F^T F - I` (ADR 0404 amendment 1). So the internal force under translation is not the floor.

**Where it is.** The step's residual (ADR 0479 D1, Newmark, `f = 0`) is `R(u) = -[f_int(u) + M (u - u~)/(beta h^2)]`. Its
tangent is `K_eff = K_T + M/(beta h^2)`, here `M/(beta h^2)` about `1000` times `K_T` (`4/(omega h)^2`, `omega h = 2 pi/100`
for the free-free axial mode). A computed `u` carries the unit roundoff `eps/2` in each component, so even the float vector
nearest the solution has a residual of up to `(eps/2) (|K_eff| |u|)_i` per equation (a first-order perturbation bound;
componentwise backward error in the sense of Oettli and Prager, as in Higham, *Accuracy and Stability of Numerical Algorithms*,
ch. 7, from memory, NOT re-read; derived here). The translation grows `|u|` (about `5.3 m/s` of the mean velocity), while the
reference of the relative test, `max(|f_int|, |g|)` (ADR 0479 D2), follows the elastic force, which passes through zero every
half period of the free-free mode, and 50 steps of `T_lin/200` are exactly half that period (`T_ff = T_lin/2`: consistent mass,
free-free `omega^2 = 12 k/m` against fixed-free `3 k/m`).

Probe 1 (`probe1.log`): S3 run at the residual tolerance `1e-6`; after each step the converged `u` iterated again 12 times with
both tolerances `1e-300` (the attained floor `rn`, absolute), against `phi = (eps/2) |(|K_eff| |u|)|_free`. The reference
`max(|f_int|, |g|)` is `5.1e4` to `8.1e5 N` at steps 1 to 49 and `88.5 N` at step 50; `rn` is `3e-9` to `3.7e-7 N` and `rn/phi` is
`0.03` to `0.58` over 60 steps (step 50: `rn = 1.45e-7`, relative `1.64e-9`, `rn/phi = 0.156`). So at step 50 no float vector
reaches `1e-10` relative: the relative test sits below the floor by a factor 16, and the halvings make it worse (`M/(beta h^2)`
grows by 4 per halving, the reference does not). The ADR 0479 run's `1.169e-10` was the last halving's level.

Probe 2 (`probe2.log`): the pendulum of ADR 0479 (`E = 2e9`, 10 x 2 x 2 Hex8, pinned, `dt = 0.005`), residual tolerance `1e-7`, 400
steps, every tenth step measured as above (with the gravity load): `rn/phi` `0.115` to `0.242` while the rotation reaches about
`pi/2` and the relative floor reaches `1.2e-8`. There `|K_T| |u|` dominates `phi` (the axial stiffness over `L |H|`), i.e. this
is the internal-force rounding of ADR 0479 arithmetic 5 under rotation, and the same bound covers it.

Probe 3 (`probe3.log`): with the internal tangent only, `phi_int = (eps/2) |(|K_T| |u|)|_free`, S3's floor is `7` to `20` times
`phi_int` at the late steps (step 50: `7.7`): the inertia term carries S3's bound.

## Design

### D1. The rounding bound of the residual (first principles, D1 of the diagnosis above)

At an iterate of an attempt that has a contribution in the residual reference (ADR 0479 D2), the bound

    b = eps |(|K| |u|)|_free,      (|K| |u|)_i = sum_j |K_ij| |u_j|,

with `K` the assembled tangent of the iterate including the contributions (the matrix the correction is solved with, before the
reduction; for the fallback path of ADR 0570 D2 the same matrix), `u` the iterate over all equations, the norm over the free
equations as `rn`. The factor `eps = 2 (eps/2)`: the unit roundoff of the representation of `u` and an equal allowance for the
evaluation of `R` (the probes: `rn <= 0.58 phi = 0.29 b`, a margin of 3.4 at the worst of 60 steps of S3, 8 on the pendulum).
No tuned constant: `eps` is `f64::EPSILON`.

### D2. The acceptance

The convergence test of ADR 0404 D5 `k >= 1 && r <= tol && e_last <= tol_e e_first` is kept as it is at every iterate. Only
at the last iterate `k = max_iterations`, where the attempt failed before ("not converged in ... iterations"), the attempt is
accepted instead if `bounded && rn <= b && e_last <= tol_e e_first`. `bounded` is true when a contribution has entered the
reference in this iterate, set inside the block `if c.in_residual_reference() { ... }` of `contribute_with` and
`contribute_split` (so both MU6 and MU6s remove it). `b` is computed only there (no cost and no operation otherwise). So every
attempt that converged before converges at the same iterate with the same bits; the bound only turns an attempt that failed at
its last iteration (and was then halved, or ended the run with an error) into a converged one. The audit below motivated the
last-iterate gate: the bound tested at every iterate would have stopped 12 iterates of one existing test earlier than the
relative test did. The cost: an attempt at the floor takes all `max_iterations` corrections before it is accepted. The
residuals recorded (`Equilibrium::residuals`, `StepReport::residual`) are unchanged in meaning: a step accepted at the bound
reports its relative residual above `tol`. The energy test is kept, so a step is accepted at the bound only after the
corrections have converged (the energy ratio at the floor is `1e-27` to `1e-30` in ADR 0479).

What it accepts: a residual no larger than the one the rounding of `u` alone produces, i.e. `u` is the exact solution of a
problem whose displacement is perturbed by one unit roundoff per component plus an equal evaluation error. The relative test is
untouched; the bound only matters where the relative test fails for `max_iterations` corrections.

### D3. Where (shared solver file `nonlinear.rs`; F-631 overlap)

`contribute_with` and `contribute_split` return whether a contribution entered the reference (one line `bounded = true;` as the
first statement of their existing `if c.in_residual_reference() {` block, before the existing `reference = ...` line; F-631's ADR
0603 D3 adds its lines after that line: adjacent, not overlapping; the lead may see a merge to resolve by keeping both). The
private `contribute` and the two calls in `Problem::attempt_with` take the flag; the acceptance line of `attempt_with` and a
private function `rounding_bound(k, u, free)` are added. No public signature changes. `nl_transient.rs` is not changed (F-631
changes it). The internal-force evaluation is not changed (the diagnosis: it is not S3's floor).

Default bits (D2): the static driver without a contribution in the reference never sets `bounded`, so it executes exactly the
operations it executed. A run with such a contribution (`nl_transient`, its energy-momentum and discrete-gradient steps, a
`solve_with` with such a contribution) changes only where an attempt failed at its last iteration before and has `rn <= b` there; every run without such
an attempt keeps its bits. The audit (B3) counts those attempts over the crate's tests.

### D4. What is not offered

No option: the bound is part of the reference path of ADR 0479 D2 (the acceptance asks for the driver defaults). No change of the
static default path. No reduction of the floor itself (the unknown stays the total displacement; an incremental unknown would
change the bits of every `nl_transient` run, open question 2). The case schema, CLI and MCP are not touched.

## Default-bits audit before registration (disclosed)

The instrumented copy `f571-scratch/audit` of the base 43092983 computes, at every iterate of every attempt with a contribution in
the reference, `b` of D1 and logs the iterates where the D2 acceptance would accept and the ADR 0404 test rejects ("new only"),
and those where `b > 0.1 tol |reference|` ("near"); it changes no value. `cargo test -p rcfd-fem --release --no-fail-fast` on it
(the non-ignored tests): 1289 passed, 0 failed, 328 ignored. "New only" iterates: 12, all in `sa_emc`
`n1e_n2e_trapezoidal_negative_control` (ADR 0514 N1e, N2e: Newmark on the spinning bar at residual tolerance `1e-7`, in its
blow-up), at `k = 4` (11) and `k = 5` (1), `b/(tol |reference|)` `7.1` to `10.5`, each followed in the base by an acceptance of
the relative test at a later iterate. None is at `k = max_iterations`: no attempt of the non-ignored tests fails at its last
iteration with `rn <= b`, so with the last-iterate gate of D2 every non-ignored test of the crate keeps its bits (B3 = 0). The
unchanged base run of that test prints `600 steps, error None`, N1e `8.800399e-6`, N2e `3.896782e-6` (the B4 record). The audit
did not run the ignored tests (records, heavy runs), which may change where they failed at a floor (S3's record among them).

## Checks (limits fixed now)

File `crates/rcfd-fem/tests/sa_nl_rounding_floor.rs` (new; helpers copied from `sa_nl_transient.rs`, which is not edited).

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| A1 | c1 | S3 copy, `NlTransientOptions::new(T_lin/200)`: every step of 50 `Ok`; `max_n |P_n - P_0|/|P_0|` | all 50 `Ok`; `<= 1e-11` | probe 1: `rn <= 0.29 b` at every step; ADR 0479 measured `1.30e-14` over 49 steps |
| A1h | c1 (companion) | total halvings of the 50 steps | `0` | the bound holds at the full step (probe 1) |
| A1r | c1 (positive control) | the largest `StepReport::residual` of the 50 steps | `>= 1e-10` | probe 1: `1.64e-9` at step 50: the bound, not the relative test, accepts there |
| A2 | accuracy | S3 with the defaults against S3 at residual tolerance `1e-6` (accepted by the relative test, probe 1: 60 steps): `max_n |u_n - u'_n|_inf / max_n |u'_n|_inf` | `<= 1e-9` | both at the floor: `rn/|K_eff| ~ 1e-19 m` against `|u| ~ 3e-3 m` |
| P1 | companion (rotation) | the pendulum of ADR 0479 P0 to P2 as registered there (`NlTransientOptions::new(0.005)`, defaults, 1940 steps): every step `Ok`; crossings; period; energy; total halvings | all `Ok`; `>= 9`; `<= 1e-2`; `<= 1e-3`; `<= 19` | probe 2: `rn <= 0.12 b` at 40 steps; ADR 0479 P0c to P2c at tolerance `1e-7`: 10, `2.09e-4`, `2.76e-4` |
| H1 | not weakened | ADR 0479 H1 (`max_iterations = 1`: the step fails, the message as registered there) passes unchanged | pass | the energy test needs two corrections; D2 keeps it |
| B1 | defaults | `cargo test -p rcfd-fem --release` of every `*_default_bits` test of the crate and of `sa_nl_transient`, `sa_dg`, `sa_dgm`, `sa_emc` | pass | D3 and the audit |
| B2 | defaults | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no case output path changes |
| B3 | defaults | attempts of the crate's non-ignored tests that fail at their last iteration with `rn <= b` (the audit above, measured before registration on the base, disclosed; not re-run) | `0` | measured `0` |
| B4 | defaults | `sa_emc` `n1e_n2e_trapezoidal_negative_control` with `--nocapture` after the change prints the base record: `600 steps, error None`, `8.800399e-6`, `3.896782e-6` | equal text | the test whose iterates the ungated bound would have changed |

A1, A1h, A1r, A2 take well under a second in release (A1's step 50 takes its 20 corrections). P1: most pendulum steps from step 8
on sit at the floor (probe 2) and take 20 corrections each, several times ADR 0479's `23.5 s`; it is an ignored heavy record run
once with `--ignored`, the fast test on the same code path is A1. H1 is ADR 0479's `h1_h2_halvings_and_the_error_with_the_time`
in `sa_nl_transient` (B1 runs it).

## Mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU6 | every `c.in_residual_reference()` in `nonlinear.rs` read as `false` (ADR 0479 MU6 as registered) | A1 (the reference is `|f_int + g|`, `r = 1`) |
| MU6s | the block `if c.in_residual_reference() {` of `contribute_with` as `if false {` (ADR 0479's script form) | A1 (no bound; the reference as in ADR 0479's run, which failed at step 50) |
| MB1 | the bound factor `eps` replaced by `0` | A1 |
| MB2 | `b` from the tangent without the contributions (`|K_T| |u|`) | A1 (doubt: probe 3 puts S3's floor at `7.7 phi_int = 3.9 b_int` at step 50, but `rn/phi` ranges by a factor 20 from step to step, so an iterate below `b_int` within 20 iterations is possible) |
| MB3 | the energy test dropped from the bound branch | none expected (registered as a doubt: with `max_iterations = 1` H1's residual after one correction is far above `b`, so H1 still fails; the energy test is defensive here) |
| MB4 | the last-iterate gate removed (the bound tested at every iterate `k >= 1`) | B4 (the audit's 12 iterates) |

## Decision rule

- **done:** A1, A1h, A1r, A2, P1, H1, B1, B2, B3, B4 met; MU6 and MU6s caught; MB1 and MB4 caught; fmt and clippy clean.
- **partial:** A1, MU6, MU6s, B1, B2, B3, B4 met and any of A1h, A1r, A2, P1, H1, MB1 or MB4 missed (diagnosed, a known miss).
- **todo:** A1 missed, or MU6 or MU6s not caught, or any of B1 to B4 missed (a default bit changed: the change is then not kept
  as default, and the flip is listed for the maintainer).

## Limits (what is not covered)

- The floor is accepted, not lowered: the absolute residual at acceptance grows with `|u|` (a body drifting far from its start
  is accepted at a residual relative to its drifted displacement times `K_eff`, the rounding of `a_{n+1}` in this arithmetic).
- The free spinning bar of ADR 0479 (S0c to S2c) stays a known miss (the instability of the trapezoidal rule, not the floor);
  the energy-momentum step's own floor is not checked here (its driver call has a contribution in the reference and gets the
  bound; the audit covers its tests).
- The static driver without a contribution in the reference keeps the ADR 0404 test (its floor, e.g. of a slender static model
  at large rotation, is unchanged).
- One element, one pendulum; no convergence study.

## Open questions (taken on the maintainer's standing instruction of 2026-11-11, take the recommendation, unless stated)

1. **Bound or reference.** Taken: an acceptance at the rounding bound (D2) rather than a larger reference (F-631's
   inertia-term reference of ADR 0603, opt-in, accepts at `tol` times the terms, about `5e5` times looser than `b`). Both can
   coexist.
2. **An incremental unknown** (`u = u_n + du`, the inertia term from `du`), which would lower the floor of a translating body
   by `|u|/|du|`: not taken; it changes the bits of every `nl_transient` run (a maintainer decision; recommended only if a model
   needs a floor below `b`).
3. **The bound in the static driver without a contribution in the reference**: not taken (it would touch the static default
   path); recommended as a follow-up only with its own audit.

## Addendum 1: results (2026-11-15)

Code: commit `eba14e66` (`nonlinear.rs`: module doc lines 27 to 33; `contribute`, `contribute_with`, `contribute_split` and the
new `rounding_bound`, lines 1460 to 1551; `Problem::attempt_with` lines 1680 to 1693 and 1725 to 1745; the new test file
`tests/sa_nl_rounding_floor.rs`). Runs, each once, release profile: `cargo test -p rcfd-fem --release --test sa_nl_rounding_floor`
(`f571-scratch/run1.log`), the same with `--ignored` for P1 (`run1-p1.log`, 67 s), B1 (`b1.log`), B4 (`b4.log`), B2 (`b2.log`).

| Id | Measured | Limit | Result |
|---|---|---|---|
| A1 | 50 of 50 steps converge with `NlTransientOptions::new(T_lin/200)`; `max |P_n - P_0|/|P_0|` `1.297e-14` | all `Ok`; `<= 1e-11` | met |
| A1h | 0 halvings | `0` | met |
| A1r | largest final relative residual `1.640e-9` (step 50, accepted at the bound) | `>= 1e-10` | met |
| A2 | `3.56e-18` (the tolerance-`1e-6` run completes, 50 steps) | `<= 1e-9` | met |
| P1 | 1940 of 1940 steps with the driver defaults; 10 crossings; period `2.095e-4`; energy `2.762e-4`; 0 halvings; largest final relative residual `1.58e-8` | all `Ok`; `>= 9`; `<= 1e-2`; `<= 1e-3`; `<= 19` | met |
| H1 | `h1_h2_halvings_and_the_error_with_the_time` passes | pass | met |
| B1 | the 72 `*_default_bits` files of `rcfd-fem` with `sa_nl_transient`, `sa_dg`, `sa_dgm`, `sa_emc`, `sa_shell3_paths`, `sa_corot_shell_dyn`, `sa_creep_arrhenius_nlt`, `sa_hyper`, `sa_visco`, `sa_temp_field`, `sa_temp_history`, `sa_temp_predictor`: 155 passed, 0 failed, 53 ignored | pass | met |
| B2 | `baseline_hashes`: 3 passed, 1 ignored | pass | met |
| B3 | `0` (measured before registration, not re-run) | `0` | met |
| B4 | `600 steps, error None`, `8.800399e-6`, `3.896782e-6` | equal text | met |

P1 is the ADR 0479 pendulum P0 to P2 at the driver defaults (registered there at `1e-10`, the set-up that amendment 1 there
corrected to `1e-7`). It now completes at the defaults with P1c's numbers to the printed digits (`2.094897e-4`; energy
`2.761783e-4` against `2.761787e-4` at `1e-7`).

**Mutations** (`f571-scratch/mutate.py`, `mutations.log`, `mut_*.log`; scratch copy `f571-scratch/mut`, target `target-mut`):

| Id | Caught by |
|---|---|
| MU6 (every `c.in_residual_reference()` read as `false`) | A1 (step 1 fails: `r = 1`), A1r, A2; `sa_emc` also fails |
| MU6s (ADR 0479's script form: the block in `contribute_with` disabled) | A1 (step 50 fails after 5 halvings, as in ADR 0479's run), A1r, A2 |
| MB1 (`eps` replaced by 0) | A1, A1r, A2 |
| MB2 (the bound from the tangent without the contributions) | A1, A1r, A2 (the registered doubt did not happen) |
| MB3 (the energy test dropped from the bound branch) | H1. Registered as "none expected", but the first step was accepted after its halvings with one correction (residual `3.17e-10`, below the bound at `dt/32`). So the energy test carries weight; it is not just a safeguard |
| MB4 (the last-iterate gate removed) | B4 (N1e `8.800452e-6`, N2e `3.896805e-6`: that test's bits change) |

All six were caught. MB3 was caught by a check that was not expected to catch it; this is recorded here and does not change any
limit. `cargo fmt -p rcfd-fem --check` and `cargo clippy -p rcfd-fem --release --all-targets -- -D warnings` are clean.

**Deviations.** No limit or set-up changed. The library and the test file landed in one commit after the registration. The
whole `rcfd-fem` suite was not re-run after the change (that is the lead's gate). B3 rests on the pre-registration audit and on
the argument for the last-iterate gate: an attempt that converged before is accepted before its last iterate is examined.
Not run: the ignored records of other ADRs. ADR 0479's record `s3_free_body_momentum_known_miss` is unchanged and would now pass
with `--ignored`. NT1 of ADR 0545 and the checks of F-631 were not run.

**Decision: done** by the registered rule. A1, A1h, A1r, A2, P1, H1 and B1 to B4 are met; MU6, MU6s, MB1 and MB4 are caught;
fmt and clippy are clean.

**Remaining questions** (recommendations):

1. ADR 0479's ignored records S3 and P0 to P2 would now pass. Recommendation: leave them as records (house rule: not edited),
   and have the lead's note of F-423 say that ADR 0604 lifts the floor of S3 and of the pendulum at the defaults.
2. Overlap with F-631 (ADR 0603). `contribute_with` and `contribute_split` gain `bounded = true;` as the first line of the block
   that F-631 extends after its first line, so the merge keeps both (adjacent lines). The two changes do not depend on each
   other: F-631's opt-in raises the reference, and ADR 0604 accepts at the rounding bound at the last iterate.
   Recommendation: merge both. NT1 of ADR 0545 may now pass at the defaults as well; it was not run here and deserves one run.
3. Cost. A step at the floor takes all `max_iterations` (20) corrections before it is accepted (P1: 67 s, against about 20 s
   at `1e-7`). Options: (a) keep this, which is bit-safe (recommended); (b) test the bound at every iterate, which is faster
   but changes the bits of ADR 0514 N1e (MB4), so it is a maintainer decision; (c) add a stagnation test (follow-up below).
4. The static driver without a contribution in the reference keeps the ADR 0404 test (open question 3). The incremental unknown
   (open question 2) stays a maintainer decision.

## Amendment 1 (2026-11-15, written after the merge with main d8b6f3f5 and before any run of the merged code)

**What happened.** The base of this branch (43092983) predates F-631 (ADR 0603, merged in main with F-632 and F-680). The
lead merged F-571 with main for the gate and reported one failure: `crates/rcfd-fem/tests/sa_nl_inertia_reference.rs`
`ft1_nc1_a_step_at_the_zero_crossing`, check "NC1 Driver refused", now "converged: 0 halvings, residual
1.3921011428752842e-10". NC1 is ADR 0603's negative control: one step of the Shell3 strip from its zero crossing at `h_f =
h/1024` with the default reference must be refused (ADR 0603 addendum 1: refused after 5 halvings, stall `1.48e-7` at
`h_f/32`). With ADR 0604 the default reference path accepts that step at its last iteration at the rounding bound: the step's
floor (the constant corotated Shell3 rounding `A_f` of ADR 0603) is within `eps |(|K| |u|)|`. This is the behaviour ADR 0604
D2 registers (a step at a rounding floor of the reference path is accepted with the driver defaults), so the test's
expectation, not a limit of ADR 0604, is what changed. The number `1.392e-10` is the lead's and is disclosed here; nothing else
of the merged code was run before this amendment. The audit of ADR 0604 (on 43092983) could not see this test.

**How NC1 is handled** (the lead's instruction, the precedent of F-620 / ADR 0555, the records-and-copies scheme of ADR 0291):

1. `ft1_nc1_a_step_at_the_zero_crossing` becomes an ignored record: its body unchanged, only an `#[ignore = "..."]` naming ADR
   0603's registered numbers (NC1 refused after 5 halvings, `1.48e-7` at `h_f/32`; FT1 2 iterations, `1.25e-16`) and the
   reason (ADR 0604 accepts the NC1 step at the rounding bound). No other existing test is changed.
2. A copy in a new file `crates/rcfd-fem/tests/sa_nl_inertia_reference_after_f571.rs` (helpers copied, the F-631 file not
   otherwise edited): `ft1_nc1b_a_step_at_the_zero_crossing_after_f571` with FT1 unchanged (same limits: converges, 0 halvings;
   final relative residual `<= 1e-10`; first-step error `<= 1e-8`) and **NC1b**: the same default step is accepted with 0
   halvings, its iteration count equals `max_iterations` (20: acceptance at the last iterate, D2), its final relative residual
   is `> 1e-10` (above the tolerance, so the bound and not the relative test accepted it) and `<= 1e-7` (below the floor ADR
   0603 measured at `h_f/32`, estimate: `1.39e-10` at `h_f`). Recorded without a limit: the absolute residual `rn` and the bound
   `b = eps |(|K_eff| |u|)|_free` recomputed in the test from `nonlinear::internal_force`, `nonlinear::tangent` and the
   integrator's mass at the accepted `u` (a recomputation, not the driver's own numbers; printed as "not available" if the
   tangent function refuses the shell model).
3. ADR 0603 gets a short addendum pointing here. NC1's purpose (show that the default reference stalls at the floor) is
   recorded by the ignored test and by ADR 0603's addendum; FT1 still shows the inertia-term reference converging in 2
   iterations, which the bound does not affect (the bound is only consulted at the last iterate).

**Checks added (limits fixed now):**

| Id | What | Limit |
|---|---|---|
| NC1b | as item 2 above | `Ok`, 0 halvings, iterations `= 20`, `1e-10 < r <= 1e-7` |
| FT1c | FT1 in the copy | as ADR 0603 FT1 |
| M1 | `cargo test -p rcfd-fem --release --no-fail-fast` on the merged branch (every non-ignored test; the lead asked for the whole crate) | every failure reported; the only expected change is NC1 |
| M2 | ADR 0603 NT3 run once with `--ignored` on the merged branch, and FT2, DB1, DB2 in M1: their registered limits | as ADR 0603 (estimate: unchanged, their attempts converge by the relative test before the last iterate: worst residuals `4.9e-13`, `6.1e-13`) |
| M3 | `baseline_hashes` | pass |

The decision whether ADR 0603's NC1 should be read as superseded (the default path now accepts steps at the floor) is listed
for the maintainer in the addendum below; recommendation in Addendum 2.

## Amendment 2 (2026-11-15, written after the merge with main 2bc0141c and before the run of the merged code)

1. **Base.** On the lead's instruction the branch was merged again, with main 2bc0141c (F-608, ADR 0607:
   `NewtonOptions::line_search` and a field `Attempt::step_lengths`), before M1 to M3 of amendment 1 were run (a run of M1 on
   the d8b6f3f5 merge was started and stopped before it finished; none of its results was read). The text merge was clean. The
   bound branch of D2 builds an `Attempt` and so needs the new field: the merge-resolution commit adds `step_lengths` to it
   (the same value the relative-test branch returns). Nothing else of F-608 or F-571 changes. M1 to M3 and NC1b's file are run
   on this merged tip, each once (NC1b was run once on the d8b6f3f5 merge, `f571-scratch/am1-nc1b.log`: met; it is run
   again in M1 as part of the crate, and that is the run reported). Before M1 ran (a start on the 2bc0141c merge was stopped
   unread), the lead's main moved to a9efc22e (F-596, ADR 0608; F-683, ADR 0611): merged as well, clean, it builds without a
   further change; M1 to M3 run on that tip (`fead5505` and the commit of this paragraph).
2. **Correction (the lead, from ADR 0609 of F-681).** The energy-momentum step (`nl_transient/emc.rs`) and the
   discrete-gradient step (`nl_transient/dg.rs`) have their own Newton loops and never call the driver's `attempt_with`, so
   they never reach the bound. The sentences "A run with such a contribution (`nl_transient`, its energy-momentum and
   discrete-gradient steps, ...)" (D3) and "the energy-momentum step's own floor is not checked here (its driver call has a
   contribution in the reference and gets the bound; the audit covers its tests)" (Limits) are wrong for those steps: read
   them as "the `Method` step of `nl_transient` and a `solve_with` with a contribution in the reference"; the energy-momentum
   and discrete-gradient steps keep their own tests unchanged (their bits are untouched by F-571, consistent with the audit,
   which found no event on them). The audit's 12 events of `sa_emc` `n1e_n2e_trapezoidal_negative_control` are on its
   `Method` (Newmark) run, which does use the driver.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-423 (ADR 0479, 2026-11-14). DONE. ADR 0604, 2026-11-15: the floor is not in the total Lagrangian internal force under translation (its rounding is 3e-11 to 1.1e-8 N on S3). It comes from representing the total displacement through the step tangent K_eff = K_T + M/(beta h^2): the residual of the nearest float vector reaches (eps/2)|(|K_eff| |u|)| (probe: 0.03 to 0.58 of it over 60 steps), while the reference max(|f_int|, |g|) passes through zero every half period of S3's free-free mode (88.5 N at step 50, relative floor 1.64e-9). On the pendulum, the internal-tangent part of the same bound covers the rotation floor (0.12 to 0.24). Change in nonlinear.rs, with no option and no public signature change: when a contribution is in the residual reference, an attempt still above the tolerance at its last iteration is accepted if its residual is within the rounding bound eps |(|K| |u|)|_free and the energy test holds; every attempt that converged before converges at the same iterate. S3 of ADR 0479 with the driver defaults: 50 of 50 steps, 0 halvings, |dP|/|P_0| 1.3e-14, step 50 accepted at the bound (relative 1.64e-9), within 3.6e-18 of the run at tolerance 1e-6. The ADR 0479 pendulum at the defaults: 1940 steps, period 2.09e-4, energy 2.76e-4, 0 halvings (ignored heavy record, 67 s). MU6 of ADR 0479 is caught in both forms (the registered one and its script's), and MB1 to MB4 are caught (the energy test by H1, the last-iterate gate by ADR 0514 N1e's printed values). Default bits: the 72 default-bits files and the nl_transient families pass, the baseline hashes pass, and an instrumented audit of the crate's 1289 non-ignored tests found no attempt the bound would change. Not covered: the floor is accepted, not lowered (an incremental unknown would change every nl_transient bit); the static driver without a contribution in the reference; a step at the floor costs max_iterations corrections; the free spinning bar stays a known miss of the trapezoidal rule.

> Follow-up proposed: Early acceptance at the rounding bound of the Newton residual (a stagnation test). Deps F-571. Acceptance: Every limit not stated here is registered in the ADR before the run. A step at the rounding floor of ADR 0604 is accepted in at most a registered number of corrections below max_iterations; S3 and the pendulum of ADR 0479 at the defaults run in at most half of ADR 0604's iterations; the bits of every existing default-bits test are unchanged, or the changed ones are listed for the maintainer's decision.
