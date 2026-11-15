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
