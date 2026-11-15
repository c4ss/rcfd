# ADR 0625: F-691 registration: a negative control of the energy-momentum inertia-term reference off rest

Status: **REGISTERED 2026-11-15, written before any code of F-691 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-691 was written in the branch and nothing of F-691 was compiled or
run: no probe was made. Every number used below is either a closed form derived here or a number recorded in ADR 0609 (its
probe 2 and its addendum 1); the arithmetic is shown. Results go in the addenda at the end; nothing above the addenda is changed
after a run.

Roadmap item F-691 (deps F-681), the follow-up proposed by
[ADR 0609](0609-f681-registration-the-inertia-term-residual-reference-on-the-energy-momentum-step.md) (its remaining question
1 (b) and its "For the lead" follow-up line). Number 0625 given by the lead. Branch `f691-work` from the local main tip 9cf95fc3
(which holds F-681, ADR 0609, and F-571, ADR 0604), worktree `w49i-work/f691`, scratch `w49i-work/f691-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below
that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-691 asks

The acceptance (docs/roadmap.json, identical to the follow-up line of ADR 0609's "For the lead"), binding:

> Every limit not stated here is registered in the ADR before the run. One energy-momentum step of a solid in free vibration centred on a zero crossing (not from rest) at a registered fraction of the period is refused with the driver reference at the default tolerance and converges without halving with ResidualReference::InertiaTerms; the default bits unchanged.

Clauses: **c0** every limit registered here; **c1** one energy-momentum step of a solid in free vibration, centred on a zero
crossing, not from rest, at a registered fraction of the period; **c2** that step refused with `ResidualReference::Driver` at
the default tolerance (`NlTransientOptions::new(h)`: `NewtonOptions::new(1)`, residual and energy tolerances `1e-10`, 20
iterations, 5 halvings); **c3** the same step converges with 0 halvings with `ResidualReference::InertiaTerms`; **c4** the
default bits unchanged.

Readings, registered:

- **c1 "a solid in free vibration"**: the clamped Hex8 cantilever of ADR 0609 (`cantilever()` of
  `sa_emc_inertia_reference.rs`, copied: `hex_box([0, -0.01, -0.02], [1, 0.02, 0.04], [10, 2, 2])`, `E = 2e9`, `nu = 0.3`,
  `rho = 7850`, the nodes of `x = 0` clamped; `f_1`, `phi` its first mode, `v_0 = omega_1 a phi/phi_max`, `a = 0.05 L`), no load,
  no damping.
- **c1 "centred on a zero crossing (not from rest)"**: the step starts at `u_0 = -(h/2) v_0`, `v_0` as above, so that its
  midpoint `t_0 + h/2` is the zero crossing. Derivation: for one mode (mass 1, stiffness `omega^2`) the linear midpoint step
  `(2/h^2)(u_1 - u_0 - h v_0) + omega^2 (u_0 + u_1)/2 = 0` has `u_1 = -u_0` (midpoint displacement exactly zero) if and only if
  `u_0 (4/h^2) = -(2/h) v_0`, that is `u_0 = -h v_0/2`; then `v_1 = 2 (u_1 - u_0)/h - v_0 = v_0`. The energy-momentum step of the
  total Lagrangian solid departs from this only by its quadratic strain terms, of relative size of the displacement gradient
  (about `1e-7` here, below). The exact modal state half a step before the crossing (`u_0 = -(v_0/omega) tan(omega h/2)`) was
  the alternative; not taken, because it differs from `-h v_0/2` by `(omega h)^2/12` relative and its discrete midpoint is then
  not the zero crossing (`u_0` is not zero: "not from rest" holds for both).
- **c1 "a registered fraction of the period"**: `h_c = T_1/(40 x 1024 x 64) = T_1/2 621 440`, `T_1 = 1/f_1`, that is ADR
  0609's `h_f` divided by 64; `omega_1 h_c = 2 pi/2 621 440 = 2.397e-6`. Derived in "The floor off rest" below.
- **c2 "refused"**: `Integrator::step` returns `FemError::Solver { solver: "nl_transient" }` (the halvings exhausted). That the
  refusal is at a rounding floor (not, say, a divergence) is read from the error of the last level: the energy-momentum message
  "not converged in 20 iterations (relative residual r, energy ratio e)" with `r > 1e-10` and `e <= 1e-20` (N1b): the corrections
  have shrunk to rounding while the residual stays above the tolerance.
- **c4**: F-691 is tests only (no line under `crates/*/src` changes); shown by the diff and by the default-bits tests of the step.

## What was read

`AGENT_RULES.md`; the brief; ADR 0609 whole (D1 to D3, "The floor", the checks, addendum 1 with the NZ diagnosis, the remaining
questions, "For the lead"); ADR 0603 (D1, D2); `crates/rcfd-fem/src/nl_transient/emc.rs` (`substep`: the residual, the
reference with and without D1 of ADR 0609, the test `k >= 1 && r <= tol && e_last <= tol_e e_first`, the error text);
`crates/rcfd-fem/src/nl_transient.rs` (`NlTransientOptions::new`, `Integrator::new`, `start`, `advance`: level `l` restarts from
the committed state with `2^l` sub-steps of `h/2^l`, up to 5; `kinetic_energy`, `strain_energy`);
`crates/rcfd-fem/tests/sa_emc_inertia_reference.rs` whole (the helpers, FZ, the NZ record, DB1, DB2).

## The floor off rest (derived)

Notation: `R` the residual of `emc::substep` over the free equations; `R0 = max(|f_m|_free, |f_alg|, |g|)` the driver reference
(`g` the computed inertia force, norms over all equations); `R1` the products of ADR 0609 D1; `R0 <= max(R0, R1)` the
inertia-term reference. The step is accepted with the driver reference only if some iterate has `|R| <= 1e-10 R0`.

**The floor of `|R|`.** `u_{n+1}` is a float vector: whatever the arithmetic of `w = u_{n+1} - u_n - h v_n`, the nearest float
iterate to the solution leaves each component off by up to half an ulp, and the inertia force `(2/h^2) M w` then carries
`(2/h^2) M e`, `|e_i| <= ulp(u_{n+1,i})/2`. Its norm is of the order `0.1 .. 1 eps (2/h^2) |M u_{n+1}| <= 0.1 .. 1 eps R1`
(`eps = 2.2e-16`; the factor below 1 from the random signs of `e` against the consistent mass, whose diagonal is `8/27` of its
row sum for the Hex8; first principles). Where `w` is formed with rounding (an addition whose operands are not within a factor
2, Sterbenz's lemma), the rounding of `w` adds a term of the same order. Anchor from the repository: ADR 0609 NZ accepted an
iterate at `1.64e-11 R0` from rest at `h_f`, with `R1/R0 = 4/(rho_r (omega h_f)^2)` (below), that is at `1.1e-17 .. 2.3e-17 R1`;
an accepted iterate is at or above the floor, so the floor there is at most that, consistent with the quantization estimate
`~ 2e-17 R1`. Registered as the pessimistic lower bound for the predictions: **floor `>= 1e-18 R1`** (an order below the
anchor). Reading of the NZ miss that follows (a revision of ADR 0609's diagnosis, recorded here, ADR 0609 not edited): Sterbenz's
lemma made `w` exact from rest, but the quantization floor of `u_{n+1}` remains; NZ converged because `R1/R0` at `h_f` was only
about `1e6`, so `2e-17 R1 ~ 2e-11 R0 < 1e-10 R0`. The diagnostic DZ below separates the two readings.

**The reaction factor `rho_r`.** `R0` contains `|f_alg|` over all equations, with the root reactions of the clamped solid. Write
`|f_alg| ~ rho_r omega^2 |M u_mid|` for a displacement in the shape of the mode. ADR 0609 probe 2 (at `h = T_1/400`, generic
phases, `|u| >> h |v|` so `R1 ~ (2/h^2) |M u|`) recorded `R1/R0 = 34 .. 71`, so `rho_r = 2/((omega h)^2 x (34 .. 71)) = 8106/(34
.. 71) = 114 .. 238`, `(omega h)^2 = (2 pi/400)^2 = 2.467e-4`. Registered pessimistic value `rho_r = 238`.

**Each halving level of the centred step with the driver reference.** `h` the registered step, `u_n = -h v/2`, level `l` has
sub-steps of `h_l = h/2^l` from the committed start. The first sub-step of each level decides it (it is taken first):

- **Level 0** (the centred step): `u_{n+1} ~ +h v/2`, the midpoint displacement is zero to the quadratic terms. `R0 ~ rho_r
  omega^2 |M u| (gamma + c_K eps)`, `gamma ~ 1e-7` the displacement gradient (`|u|_max = pi a/N = pi x 0.05/2 621 440 = 6.0e-8`
  of `L = 1`), `c_K eps` the rounding of the internal force; `R1 = (2/h) |M v| = (4/h^2) |M u|`. Ratio `R1/R0 >= 4/(238 x 5.745e-12
  x 1e-7) = 2.9e16`: the driver's relative floor is `>= 1e-18 x 2.9e16 = 29`. Refused.
- **Level 1**: from `-h v/2` over `h/2` to about 0 (the crossing), midpoint `-h v/4`: `R0 ~ rho_r omega^2 (h/4) |M v|`, `R1 =
  max((2/h_1^2) |M u_n|, (2/h_1) |M v|) = (4/h) |M v|`; ratio `16/(rho_r (omega h)^2)`. Here `u_{n+1} - u_n` adds a small
  `u_{n+1}` to `h v/2`, rounded. The weakest level. At `h_c`: `16/(238 x 5.745e-12) = 1.17e10`, the relative floor `>= 1e-18 x
  1.17e10 = 1.2e-8`, a factor 120 above `1e-10` with the pessimistic floor and reaction factor (a factor about 2600 with the NZ
  anchor `2e-17` and `rho_r = 114`).
- **Levels 2 to 5**: from `-h v/2` over `h/2^l`, midpoint near `-h v/2`: `R0 ~ rho_r omega^2 (h/2) |M v|`, `R1 = (2/h_l^2) (h/2)
  |M v| = (4^l/h) |M v|`; ratio `2 x 4^l/(rho_r (omega h)^2) >= 2.3e10` (level 2), growing 4 times per level. Here `w` is exact
  (Sterbenz: `u_{n+1}` and `u_n` within a factor 2), as from rest; the quantization floor remains. Refused. At level 5 the ratio
  is `2048/(rho_r (omega h_c)^2) = 1.5e12 .. 3.1e12`; with the floor `1e-17 .. 1e-16 R1` the last level's relative residual is
  about `1.5e-5 .. 3e-4` (printed, limit only `> 1e-10`).

**Why not ADR 0609's `h_f`.** At `h_f = 64 h_c` the level-1 ratio is `16/(rho_r (omega h_f)^2) = 2.9e6 .. 6.0e6`; with the NZ
anchor `2e-17` the relative floor is `6e-11 .. 1.2e-10`, at the tolerance: the prediction would be a coin toss. The ratio grows
as `1/h^2`; `h_c = h_f/64` puts the weakest level a factor `64^2 = 4096` further, which gives the margins above. Smaller is not
needed and would only shrink the displacement further (`6e-8 m`, still 9 orders above the smallest normal float, and the
quadratic terms, which the step is meant to exercise, shrink with it).

**The energy ratio at the last level (N1b).** The first correction of a sub-step from the guess `u_{n+1} = u_n` is about
`h_l v`, against the residual `(2/h_l) M v`: `e_first ~ 2 v.Mv`. At the floor the correction and the residual both scale with the
floor, `e_last/e_first ~ (|R_floor|/((2/h_l) |M v|))^2 = (floor_rel x R1 h_l/(2 |M v|))^2`; at level 5, `R1 h_5/(2 |M v|) = 16`,
so `(16 x 1e-16)^2 ~ 3e-30` with the floor `1e-16 R1`. Limit `<= 1e-20`, ten orders of margin; it fails only if the corrections
did not shrink (a divergence or a stall above the floor, which N1b is there to exclude).

**The inertia-term reference (C1).** With `R1` in the reference the relative floor is `<= 1e-18 .. 1e-16`, six orders below the
tolerance, at level 0: converges without halving. The internal-force rounding against `R1`: `eps c_K rho_r (omega h)^2/4`,
negligible. Newton from `u_{n+1} = u_n`: the first correction solves the step linearized at `u_n`, the second removes the
quadratic terms (relative `gamma (omega h)^2 ~ 1e-18`); accepted at iteration 2 (as FZ of ADR 0609), with `e_last/e_first`
about the square of the relative size of the second correction, far below `1e-10`.

**What C1's state must show (derived limits).** Linear: `u_1 = -u_0` and `v_1 = v_0` exactly (above). Departures: the quadratic
terms, `<= gamma (omega h)^2/2 ~ 1e-19` of `|h v_0|`; the accepted residual, `|dR| <= tol R1` changes `u_1` by `(h^2/2) M^{-1}
dR`, at most about `tol x 2 |u|` in the mass norm, `2e-10` of `|h v_0|` (ADR 0609's argument), and `v_1` by twice that over `h`,
`4e-10` of `|v_0|`; rounding `~ 1e-15`. Limits `1e-8` for both (a factor 25 above the worst case, the FZ limit of ADR 0609). The
energy `H = T + W`: the step conserves it up to `(u_1 - u_0) . R` (ADR 0514's argument), `<= |h v_0| tol (2/h) |M v_0|`, that is
`<= 4 tol = 4e-10` of `T = v.Mv/2` (norm ratios of order 1); rounding of `T` about `1e-16`. Limit `1e-8`.

## Design

Tests only. New file `crates/rcfd-fem/tests/sa_emc_inertia_reference_off_rest.rs`; the helpers (`Checks`, `worst`, `max_abs`,
`near`, `soft`, `is_step_refusal`, `cantilever`, `emc`) copied from `sa_emc_inertia_reference.rs`, which is not edited. Each
check builds the cantilever (the modal analysis of ADR 0609, deterministic) and one integrator with
`NlTransientOptions::new(h_c).energy_momentum().residual_reference(r)`, `start(0.0, &u_0, &v_0)` with `u_0 = -(h_c/2) v_0`
(computed as `-0.5 * (h_c * v_0[i])`; zero on the clamped equations since `phi` is), and one `step()`.

No library change. If C1 missed with a diagnosed defect of the library, a change would be opt-in with defaults bit-identical
and registered by an amendment before its run; none is planned.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| C1 | c1, c3 | the centred step at `h_c` with `InertiaTerms`: converges; halvings; final relative residual; `max_i |u_1,i + u_0,i| / max_i |h_c v_0,i|`; `max_i |v_1,i - v_0,i| / max_i |v_0,i|`; `|H_1 - H_0|/H_0` (`kinetic_energy + strain_energy`) | `Ok`; `0`; `<= 1e-10`; `<= 1e-8`; `<= 1e-8`; `<= 1e-8` | holds (floor `<= 1e-16` relative); 0; `~1e-16`; `<= 2e-10` (`~1e-15`); `<= 4e-10`; `<= 4e-10` (`~1e-15`) |
| N1 | c1, c2 | the same step with `Driver` | refused: `FemError::Solver { solver: "nl_transient" }`, the message naming "after 5 halvings" | each level's relative floor `>= 1.2e-8` (level 1, pessimistic), `>= 29` (level 0) |
| N1b | c2 (the floor) | the last level's energy-momentum message in N1's error: its "relative residual" and "energy ratio" | relative residual `> 1e-10`; energy ratio `<= 1e-20` | `1.5e-5 .. 3e-4`; `~3e-30` |
| DB | c4 | (i) `git diff --stat 9cf95fc3 -- crates/*/src crates/*/Cargo.toml Cargo.toml` empty at the tip; (ii) existing tests: `cargo test -p rcfd-fem --release --test sa_emc_inertia_reference --test sa_emc_default_bits` (DB1 and DB2 of ADR 0609 against the hashes recorded on the library before F-681, and the default-bits file of the step) | empty; pass (the known-miss and heavy records stay ignored) | empty; pass |
| DZ | diagnostic, not in the decision rule | the step from rest (`u_0 = 0`) at `h_c` with `Driver` | predicted refused (`nl_transient`); recorded either way | the level-0 ratio `4/(rho_r (omega h_c)^2) >= 2.9e9`, relative floor `>= 2.9e-9` (pessimistic), `~6e-8` with the NZ anchor. A convergence would support ADR 0609's Sterbenz reading of NZ against the quantization reading above |

All five run on every crate run (one step each, a modal analysis each; expected well under the two-minute rule). No heavy record.

## Negative controls

N1 is the negative control the item asks for; DZ is a second one, diagnostic. If N1 converges (at some level), the floor
estimate above is wrong for this set-up: recorded as `*_known_miss` with the measured level and residual; the decision is then
partial.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`, one at a time, library mutations of ADR 0609's code)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `R1` not taken with `InertiaTerms` (the reference left `R0`) | C1 (refused, as N1) |
| MU2 | the products without their factors (`2/h^2`, `2/h`, `1/h` replaced by `1`) | C1, a doubt: the mutated `R1` is `|M v|` (SI units), still far above `R0 ~ 0.03 |M u|`, but `2/h_c = 2.8e7` (1/s) times smaller than D1's; the level-0 relative floor against it is `1e-18 .. 1e-17 x 2.8e7 = 2.8e-11 .. 2.8e-10`, at the tolerance, so level 0 may converge or be refused (if refused, the step halves and C1's "0 halvings" catches it). Registered: expected caught, with that doubt |
| MU3 | only the velocity product `(2/h) |M v_n|` kept (the displacement and damping products dropped) | not caught: for the centred step `(2/h) |M v|` is the largest product (`(2/h^2) |M u_n| = |M v|/h`, half), as registered for FZ in ADR 0609 |
| MU4 | `R1` taken also with `Driver` | N1 (converges), DZ (converges, not counted) |
| MU5 | the refusal of ADR 0603 D2 restored (`Integrator::new` refuses `InertiaTerms` with the energy-momentum step) | C1 |

## Decision rule

- **done:** C1, N1, N1b and DB met; MU1, MU4, MU5 caught; fmt and clippy clean. DZ, MU2 (a registered doubt) and MU3 (not expected) do not enter.
- **partial:** C1 and DB met, and N1 or N1b missed, or one of MU1, MU4, MU5 not caught.
- **todo:** C1 or DB missed.

## Limits (what is not covered)

- One model (the clamped cantilever), one mode, one step size; the floor estimate rests on ADR 0609's recorded reaction factor
  and NZ anchor, not on a measurement of this branch before the run.
- The step size is far below any step used in practice (`T_1/2.6e6`): the control shows the floor of the driver reference at a
  zero crossing, where the elastic force and so `R0` vanish; at practical step sizes the cantilever does not reach the floor (ADR
  0609 probe 1), the spinning bar does (ADR 0609 NS).
- The damping products (MU6 of ADR 0609), the neo-Hookean and F-425 materials of the step are not exercised.
- No rounding bound on the energy-momentum step (ADR 0609 remaining question 2) and no default change (question 3): unchanged.

## Addendum 1: results (2026-11-15)

Runs, each once, release profile, own `CARGO_TARGET_DIR`: the registration committed alone (67634b9c); the test file committed
(621887c4) before its run; run 1 of C1, N1, N1b, DZ at 621887c4 (`f691-scratch/run1.log`, 0.63 s for the three tests); DB
(`db.log`); mutations on a `git archive` copy of 621887c4 with its own target (`mutate.py`, `mutations.log`, `MU1.log` to
`MU5.log`). No library line changed (tests only).

Measured: `f_1 = 5.330987204 Hz`, `h_c = 7.1557e-8 s`, `omega_1 h_c = 2.3968e-6`.

| Check | Limit | Measured | Met |
|---|---|---|---|
| C1 converges; halvings | `Ok`; 0 | `Ok`; 0 (2 iterations) | yes |
| C1 final relative residual | `<= 1e-10` | `2.09e-17` | yes |
| C1 `max |u_1 + u_0| / max |h_c v_0|` | `<= 1e-8` | `1.07e-14` | yes |
| C1 `max |v_1 - v_0| / max |v_0|` | `<= 1e-8` | `2.17e-14` | yes |
| C1 `|H_1 - H_0|/H_0` | `<= 1e-8` | `2.02e-16` | yes |
| N1 `Driver` refused (`nl_transient`, "after 5 halvings") | refused | refused after 5 halvings | yes |
| N1b last level relative residual | `> 1e-10` | `2.31e-5` (estimate `1.5e-5 .. 3e-4`) | yes |
| N1b last level energy ratio | `<= 1e-20` | `1.31e-31` (estimate `~3e-30`) | yes |
| DB (i) `git diff --stat 9cf95fc3 -- crates/*/src crates/*/Cargo.toml Cargo.toml` | empty | empty | yes |
| DB (ii) `sa_emc_inertia_reference` (DB1, DB2 with the rest), `sa_emc_default_bits` | pass | 6 passed, 2 ignored (the NZ known miss and V, as before); 1 passed | yes |
| DZ (diagnostic) from rest at `h_c`, `Driver` | predicted refused | refused after 5 halvings, last level relative residual `9.99e-5`, energy ratio `1.7e-33` | as predicted |

`cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets --release -- -D warnings` clean.

**DZ and ADR 0609's NZ.** The step from rest is refused at `h_c` as the quantization estimate predicts, so the start at rest
does not by itself remove the floor: ADR 0609's NZ converged because at `h_f` the ratio `R1/R0` was only about `1e6`, putting the
floor (`~2e-17 R1`) just under `1e-10 R0`. Sterbenz's lemma makes `w` exact from rest, but the quantization of `u_{n+1}` stays.
This revises the diagnosis in ADR 0609 addendum 1 (ADR 0609 not edited). The centred step still adds something: at level 0 its
driver reference is set by the quadratic terms alone, the largest ratio of any level.

**Mutations:**

| Id | Caught by |
|---|---|
| MU1 `R1` not taken | C1 (refused after 5 halvings, `2.31e-5`, the N1 failure) |
| MU2 the products without their factors | C1 (refused after 5 halvings, last level `2.11e-7`); the registered doubt resolved: caught |
| MU3 only the velocity product | not caught, as registered (C1 bit-equal: 2 iterations, `2.09e-17`) |
| MU4 `R1` also with `Driver` | N1 and N1b (the step converges, 0 halvings, `2.09e-17`); DZ converges too (not counted) |
| MU5 the refusal of ADR 0603 D2 restored | C1 (`Integrator::new` refused) |

MU1, MU2, MU4 and MU5 caught; MU3 not caught, as predicted.

**Deviations:** none. **Not run:** `baseline_hashes` (no shared solver file changed; rule 3 asks for it only then).

**Decision by the registered rule: done.** C1, N1, N1b and DB met; MU1, MU4 and MU5 caught (MU2 caught as well); fmt and clippy
clean.

### Remaining questions

1. **ADR 0609's NZ record and diagnosis.** DZ shows a step from rest is also refused once `h` is small enough, so the Sterbenz
   reading in ADR 0609 is not the whole cause. Options: (a) leave ADR 0609 as written and let this addendum carry the
   correction; (b) add an amendment note to ADR 0609 pointing here. Recommendation (b), one line, for the lead, since the roadmap
   note of F-681 quotes the old diagnosis. Taken here as (a) only: ADR 0609 not edited, on the house rules.
2. **F-681's status.** F-681 was partial only because NZ, its negative control, converged. With N1 here, the negative control
   is now shown off rest. Options: (a) F-681 stays partial (its own registered rule); (b) the lead moves F-681 to done on the
   strength of F-691. Recommendation (b) is the maintainer's call; the rule of ADR 0609 says partial, and that is not changed
   here.
3. **Remaining questions 2 and 3 of ADR 0609** (a rounding bound on the step; the default) are unchanged and still open.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-681 (ADR 0609, 2026-11-15). DONE. ADR 0625, 2026-11-15, tests only (no library line changed): one energy-momentum step of the clamped Hex8 cantilever of ADR 0609 in free vibration of its first mode, centred on a zero crossing (start u_0 = -(h/2) v_0, so the midpoint displacement is zero), at h = T_1/2621440 (ADR 0609's h/1024 step divided by 64, from a derivation of the weakest halving level). With ResidualReference::InertiaTerms it converges with 0 halvings in 2 iterations, relative residual 2.1e-17, u_1 = -u_0 to 1.1e-14 and v_1 = v_0 to 2.2e-14, energy 2.0e-16. With the driver reference at the default tolerance it is refused after the 5 halvings. Its last level stays at a rounding floor: relative residual 2.3e-5, energy ratio 1.3e-31. A diagnostic step from rest at the same h is also refused (1.0e-4), which revises ADR 0609's diagnosis of its NZ miss: the floor is the quantization of u_{n+1}, and NZ's step was not small enough for that floor to exceed the tolerance. Default bits unchanged (DB1, DB2 and the energy-momentum default-bits test pass). Four of five registered mutations caught; the velocity-product-only mutation is not caught, as registered.

> F-681 note, appended sentence proposed: Its negative control off rest is F-691 (ADR 0625, 2026-11-15): refused with the driver reference after 5 halvings at a rounding floor, converged with 0 halvings with InertiaTerms; the from-rest miss NZ is explained by the step size (the ratio of the references), not by Sterbenz's lemma alone.
