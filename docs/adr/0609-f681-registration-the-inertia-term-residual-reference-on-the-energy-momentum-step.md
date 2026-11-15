# ADR 0609: F-681 registration: the inertia-term residual reference on the energy-momentum step

Status: **REGISTERED 2026-11-12, written before any code of F-681 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-681 was written in the branch. Disclosed: two probe runs on an
instrumented scratch copy of the base (`w49i-work/f681-scratch/probe`, a `git archive` of f5cdf93a; the instrumented
`emc.rs` is described below; the probe test `f681-scratch/zz_probe_f681.rs`; logs `probe1.log`, `probe2.log`). They ran the
energy-momentum step at the residual tolerance `1e-7` and, after each acceptance, eight more corrections, logging the smallest
absolute residual reached (the attained floor `rn`), the step's present reference and the inertia-term reference of D1 below.
None of them is a run of a check of the table (they ran at another tolerance and changed the iteration); their numbers are
quoted where used. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-681 (deps F-631, F-611), the follow-up proposed by
[ADR 0603](0603-f631-registration-residual-reference-of-the-newton-driver-in-free-vibration.md) (its remaining question 2).
Number 0609 given by the lead. Branch `f681-work` from the local main tip f5cdf93a (which holds F-631, ADR 0603, and F-571,
ADR 0604), worktree `w49i-work/f681`, scratch `w49i-work/f681-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited (one exception,
D3, disclosed); defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden
file; no new dependency, no unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every
choice below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11
(take the recommendation).

## What F-681 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. ResidualReference::InertiaTerms taken by the energy-momentum step with the products of its midpoint inertia force; a free vibration of a solid without external load over a registered number of periods at the default tolerance with no step rejected at a rounding floor and the energy conserved within a registered limit; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** `ResidualReference::InertiaTerms` taken by the energy-momentum step (no
longer refused) with the products of its midpoint inertia force in the reference (D1, D2); **c2** a free vibration of a solid
without external load over a registered number of periods at the default tolerance (`NewtonOptions::new(1)`: residual and
energy tolerances `1e-10`, 20 iterations, 5 halvings) with no step rejected at a rounding floor (0 halvings, every step
converged) and **c3** the energy conserved within a registered limit; **c4** the default bits unchanged.

Readings, registered:

- **c2** "a free vibration of a solid": two runs, both required. **V** the free bending vibration of a clamped Hex8 cantilever
  (the solid of the reading, released from a modal velocity, 100 periods of its first mode); **S** the free spinning bar of ADR
  0479 and ADR 0514 (a free solid without external load, ten turns), the model on which the present reference of the step is
  known to fail at the default tolerance (ADR 0514 arithmetic 2 and its Limits: the bar was registered at `1e-7` because the
  driver default `1e-10` cannot converge it). "Rejected at a rounding floor" is shown to be a real effect on this step by two
  negative controls with the present reference (NZ on V's model, NS on S), see "The floor" below: the probe shows that V itself
  at its registered step does not reach the floor with the present reference, so V alone would not show c2's point.
- **c1** "the products of its midpoint inertia force": the terms of which the step's inertia and damping force is computed, D1.

## What was read

`AGENT_RULES.md`; the brief; ADR 0603 whole (D1 to D3, the checks, addendum 1, remaining question 2); ADR 0604 whole (D1 to D4,
the audit, addendum 1); ADR 0514 (D1, D2, arithmetic 2 the rounding floor, D6 the bar's Newton options, the checks S0e to S3e,
Limits); ADR 0523 and ADR 0544 (headings and D4, D5: the neo-Hookean and F-425 materials run through the same `emc::substep`);
`crates/rcfd-fem/src/nl_transient.rs` (`ResidualReference`, `NlTransientOptions`, `Integrator::new` with the refusal of ADR
0603 D2, `Integrator::advance`, `substep`, `InertiaTerms`, `Inertia`); `crates/rcfd-fem/src/nl_transient/emc.rs` whole;
`crates/rcfd-fem/src/nonlinear.rs` (the F-571 bound in `Problem::attempt_with`, `contribute_with`); the tests
`sa_emc.rs` (the spinning bar `spin`, `run_spin`, `bar_newton`), `sa_emc_default_bits.rs`, `sa_nl_inertia_reference.rs`
(RF1, FT1, NC1, the helpers).

## The step and its reference as they are

The energy-momentum sub-step (`emc::substep`, ADR 0514 D1, D2) has its own Newton loop; it does **not** call the driver
`nonlinear::solve_from*`. Its residual over the free equations is

    R = (2/h^2) M (u_{n+1} - u_n - h v_n) + C (u_{n+1} - u_n)/h + f_alg(u_n, u_{n+1}) - (f_n + f_{n+1})/2,

and its reference is `max(|f_m|_free, |f_alg|, |g|)`, `f_m` the mean load, `g` the computed inertia and damping force (the
first two terms of `R`), the last two norms over all equations; the test is `k >= 1 && r <= tol && e_last <= tol_e e_first`.

**Interaction with F-571 (ADR 0604).** F-571's rounding bound is a branch of `Problem::attempt_with` in `nonlinear.rs`, read
only by the driver. The energy-momentum loop never reaches it (ADR 0604's Limits says the step's "driver call ... gets the
bound"; the step has no driver call, so that sentence does not hold for it; recorded here, not changed there). So on this step
**the inertia-term reference is what accepts**, there is no rounding bound behind it; F-681 does not add one (not asked; open
question 2). Nothing of `nonlinear.rs` is changed by F-681.

## The floor (derived; probes disclosed)

The computed inertia force `g` is `(2/h^2) M w` with `w = u_{n+1} - u_n - h v_n` formed before the product. Even the float
vector `u_{n+1}` nearest the solution leaves in `w` a rounding of about `eps (|u_{n+1}| + |u_n| + h |v_n|)` per component
(Higham, *Accuracy and Stability of Numerical Algorithms*, ch. 3, the error of a computed sum, from memory, NOT re-read;
derived here: each of the three terms is rounded at `eps/2` relative and so is each partial sum), so the computed residual has a
floor of the order of `eps max((2/h^2) |M u_{n+1}|, (2/h^2) |M u_n|, (2/h) |M v_n|)`, and with damping of `eps (1/h)
max(|C u_{n+1}|, |C u_n|)`; the internal force adds its own rounding (under a rotation of order 1 about `1e-9 .. 1e-8` of the
present reference, ADR 0514 arithmetic 2). The present reference follows `|f_alg| ~ |g|`, the elastic force, while the floor
follows the products: their ratio is about `2/(omega h)^2` times the ratio of the displacement to the elastic displacement.

- **V** (clamped cantilever, h = `T_1/40`): the root reactions of the clamped solid are in `|f_alg|` (all equations) and keep
  the present reference large at every phase. Probe 1 (800 steps, 20 periods): relative floor with the present reference at
  most `8.7e-12`, median `3.5e-13`; with the reference of D1 at most `9.1e-13`. So V does not need the option at this step (no
  negative control is run on V itself).
- **NZ** (the same model, one step from `u_0 = 0` at `h_f = T_1/(40 x 1024)`): `|f_alg| ~ |K| h_f |v_0|/2` shrinks with `h_f`
  while the floor `eps (2/h_f) |M v_0|` grows as `1/h_f`. Probe 2 (the model at `h = T_1/400`, 800 steps): the relative floor
  against D1's reference is median `1.0e-14`, and D1's reference is `34` to `71` times the present one. At `h_f`, `102.4` times
  smaller, the ratio grows as `1/h^2` to at least `50 x 102.4^2 = 5.2e5`: the relative floor with the present reference is about
  `1.0e-14 x 5.2e5 = 5e-9`, a factor 50 above `1e-10`, and each halving multiplies it by about 4. Predicted: refused after the 5
  halvings. With D1's reference the relative floor is about `1e-14`.
- **S** (spinning bar, `h = 0.01`, `omega h = 0.063`, displacements of the order of the bar's half length from the rotation):
  probe 1 (1000 steps): the relative floor with the present reference is above `1e-10` at **every** step (median `2.5e-8`, at
  most `7.0e-8`); with D1's reference at most `7.2e-12`, a factor 14 below `1e-10`; D1's reference is `34` to `1.05e4` times the
  present one. Predicted: NS (the present reference at the default tolerance) refused at step 1; S converges every step.

What the looser reference accepts (as ADR 0603): a residual `dR <= tol R1` changes `u_{n+1}` by about `(h^2/2) M^{-1} dR`, of
mass-norm size `<= tol max(|u_{n+1}|, |u_n|, h |v_n|)`: the test is a relative tolerance on the step's displacement. The energy
of the step changes by exactly `(u_{n+1} - u_n) . R` (ADR 0514's conservation argument with `R` added), at most `|du| tol R1`; with
the mass-norm estimates `|du| ~ omega h |u|`, `R1 ~ (2/h^2) m |u|`, `E ~ omega^2 m |u|^2/2`, at most `4 tol/(omega h) = 2.5e-9`
of `E` per step for V, `1e-5` over 4000 steps if every step erred by the whole tolerance with one sign. In practice the step is
accepted after a quadratic correction far below the tolerance and the energy test `e_last <= 1e-10 e_first` still applies:
probe 1 measured `4.0e-15` on V over 800 steps, ADR 0514 `1.6e-13` on S at `1e-7`.

## Design

### D1. The inertia-term reference of the energy-momentum step

With `ResidualReference::InertiaTerms` the reference of each iterate of `emc::substep` is `max(R0, R1)`, `R0` the present
reference unchanged and

    R1 = max( (2/h^2) |M u_{n+1}|, (2/h^2) |M u_n|, (2/h) |M v_n|, (1/h) |C u_{n+1}|, (1/h) |C u_n| ),

the norms over all equations (as `R0`'s `|f_alg|` and `|g|`), `C` the damping of the sub-step (absent: its terms omitted),
`u_{n+1}` the iterate (re-taken at every iterate as `R0` is), the products of `u_n` and `v_n` formed once per sub-step. These are
the products the midpoint inertia and damping force `(2/h^2) M (u_{n+1} - u_n - h v_n) + C (u_{n+1} - u_n)/h` is a sum of. The
largest norm, not the sum (as ADR 0603 D1). Derivation: the floor above (first principles; the rounding of a sum).

### D2. The option

The refusal of ADR 0603 D2 (`InertiaTerms` with the energy-momentum step, key `"nl_transient"`) is removed; the option and its
default are unchanged (`ResidualReference::Driver`). `Driver` executes exactly the operations it executed: `R1` is computed only
with `InertiaTerms`, and only then is `reference = reference.max(R1)` executed. The neo-Hookean and F-425 materials of the step
(ADR 0523, ADR 0544) run through the same loop and get the same reference (not separately checked, Limits). The doc of
`ResidualReference::InertiaTerms` and of the module are updated.

### D3. The superseded test

`sa_nl_inertia_reference.rs::rf1_inertia_terms_refused_with_the_energy_momentum_step` asserts the refusal D2 removes. It is
marked `#[ignore = "superseded by ADR 0609 (F-681 lifts the refusal of ADR 0603 D2) ..."]`, its body unchanged (the precedent of
`sa_corot_shell.rs` for ADR 0522 and `sa_contact_supernodal_f615.rs` for ADR 0542). Its control half (the model taken with
`Driver`) is repeated in RT1.

## Checks (limits fixed now)

File `crates/rcfd-fem/tests/sa_emc_inertia_reference.rs` (new; helpers copied from `sa_emc.rs` and `sa_nl_inertia_reference.rs`,
which are not edited except D3).

Models:

- **cantilever**: `hex_box([0, -0.01, -0.02], [1, 0.02, 0.04], [10, 2, 2])` Hex8 (Full), `E = 2e9`, `nu = 0.3`, `rho = 7850`, the
  nodes of `x = 0` clamped in the three components; `f_1` and `phi` the first mode of `modal_analysis` with `ModalOptions::new(1)`,
  `max_iterations = 2000`, `tolerance = 1e-8` (probe: `f_1 = 5.331 Hz`); `u_0 = 0`, `v_0 = omega_1 a phi/phi_max`, `a = 0.05 L`,
  `phi_max` the component of `phi` of largest magnitude (with its sign); no load, no damping.
- **spinning bar**: `spin()` of `sa_emc.rs` (ADR 0514 c1), copied.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| RT1 | c1 | `Integrator::new` of the cantilever with `.energy_momentum().residual_reference(InertiaTerms)`, and with `Driver` | both `Ok` | holds (D2) |
| FZ | c1 | cantilever, one step from the start at `h_f = 1/(40 f_1 1024)`, energy-momentum, `InertiaTerms`, defaults: converges with 0 halvings; final relative residual; `max_i |u_1,i - h_f v_0,i/(1 + (omega_1 h_f)^2/4)| / max_i |h_f v_0,i|` (the linear midpoint first step of the mode) | converges, 0 halvings; `<= 1e-10`; `<= 1e-8` | holds (floor `~1e-14`); `<= 1e-10` by construction; `~1e-10` (the accepted relative displacement error, D1; nonlinearity `(h_f v_0)^2` negligible) |
| NZ | c2 (negative control) | the same step with `Driver` | refused: `Solver { solver: "nl_transient" }` after the 5 halvings | relative floor `~5e-9` at `h_f`, `x4` per halving |
| S | c2, c3 | spinning bar, `NlTransientOptions::new(0.01).energy_momentum().residual_reference(InertiaTerms)`, the defaults, 1000 steps (ten turns): every step converged; total halvings; largest final residual; `max_n |P_n - P_0|/|P_0|`; `max_n |J_n - J_0|/|J_0|`; `max_n |E_n - E_0|/|E_0|` | 1000 `Ok`; `0`; `<= 1e-10`; `<= 1e-10`; `<= 1e-6`; `<= 1e-6` | holds (floor `<= 7.2e-12`, factor 14); 0; by construction; ADR 0514: `1.8e-13`, `6.6e-13`, `1.6e-13` |
| NS | c2 (negative control) | the same with `Driver` | refused at step 1 (`Solver { solver: "nl_transient" }`) | the present floor `> 1e-10` at every step of probe 1 (median `2.5e-8`); halvings make it worse |
| V1 | c2, c3 (fast companion of V) | cantilever at `h = 1/(40 f_1)`, energy-momentum, `InertiaTerms`, defaults, 40 steps (one period): every step converged; halvings; `max_n |H_n - H_0|/H_0`, `H = T + W` | 40 `Ok`; `0`; `<= 1e-6` | holds; 0; `~1e-14` (probe 1: `4.0e-15` over 800 steps at `1e-7`) |
| V | c2, c3 | the same over 4000 steps (100 periods of the first mode): every step converged; total halvings; largest final residual; `max_n |H_n - H_0|/H_0` | 4000 `Ok`; `0`; `<= 1e-10`; `<= 1e-6` | holds (floor `<= 9.1e-13`); 0; by construction; `~1e-13` (worst-case bound `1e-5`, above) |
| DB1 | c4 | the default energy-momentum paths: (i) the spinning bar, `emc_options(bar_newton())` of `sa_emc.rs` (residual `1e-7`), 20 steps, and (ii) the cantilever at `h = 1/(40 f_1)` with `NlTransientOptions::new(h).energy_momentum()`, 40 steps: FNV-1a 64 of the bit patterns of `u`, `v`, `a` after each run, against the hashes recorded on the unchanged library (the test written and run once at the base library before any library line changes, its printed hashes then fixed as the constants) | equal | equal (D2) |
| DB2 | c4 | the same two runs with `.residual_reference(ResidualReference::Driver)` explicitly | equal to DB1's constants | equal |
| DB3 | c4 | existing tests: `cargo test -p rcfd-fem --release --test sa_emc --test sa_emc_default_bits --test sa_dg --test sa_dg_default_bits --test sa_dgm --test sa_dgm_default_bits --test sa_nl_inertia_reference --test sa_nl_transient --test sa_nl_rounding_floor`; `cargo test -p rcfd-api --release --test baseline_hashes` | pass (RF1 ignored by D3) | pass |

V is heavy (4000 steps, about 30 s in release on the probe's timing, several minutes in the debug profile): an ignored record run
once with `--ignored`; V1, FZ, NZ, S, NS, RT1, DB1, DB2 run on every crate run (S is the cost of the existing `s0e` test of
`sa_emc`, which is not ignored).

## Negative controls

NZ and NS above (the present reference refuses at the floor). If either converges, the predicted floor is wrong for that set-up:
recorded; the decision is then partial (c2 not shown on that model).

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `R1` not taken with `InertiaTerms` (the reference left `R0`) | FZ, S |
| MU2 | the products without their factors (`2/h^2`, `2/h`, `1/h` replaced by `1`) | FZ, S |
| MU3 | only the velocity product `(2/h) |M v_n|` kept (the displacement and damping products dropped) | S (estimate: with `(2/h) |M v|` alone the ratio to `R0` is about 30, the floor `2.5e-8/30 ~ 8e-10 > 1e-10`); not FZ (at the first step `(2/h_f) |M v_0|` is the largest product) |
| MU4 | `R1` taken also with `Driver` | NZ, NS (converge); DB1 (doubt: the earlier stop may or may not change a bit of the default runs) |
| MU5 | the refusal of ADR 0603 D2 restored | RT1, FZ, S |
| MU6 | the damping products dropped | none expected (no damped check; registered as a doubt, Limits) |

## Decision rule

- **done:** RT1, FZ, NZ (refused), S, NS (refused), V1, V, DB1, DB2, DB3 met; MU1 to MU5 caught; fmt and clippy clean.
- **partial:** DB1, DB2, DB3, RT1 and FZ met, and V or S missed or not run, or NZ or NS converges, or a mutation of MU1 to MU5 not
  caught.
- **todo:** DB1, DB2 or DB3 missed (the default bits changed), or RT1 or FZ missed.

## Limits (what is not covered)

- The damping products are in D1 but not checked (MU6); the neo-Hookean and F-425 materials of the step get the reference by the
  same loop but are not run with it.
- The internal-force rounding of the total Lagrangian solid under rotation is not reduced; the option only measures the residual
  against the inertia terms. A step whose floor is in the internal force with negligible inertia gains nothing.
- No rounding bound (F-571 style) on the energy-momentum step: with `Driver` the step keeps failing where it failed.
- With a large rigid drift `R1` grows with the drifted displacement (ADR 0603 Limits, the same here): looser in absolute terms.
- The option is not exposed in the case schema, CLI or MCP (goldens unchanged).
