# ADR 0603: F-631 registration: the residual reference of the Newton driver in free vibration (an opt-in inertia-term reference)

Status: **REGISTERED 2026-11-15, written before any code of F-631 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-631 was written and nothing was run; the arithmetic below was done by
hand (no script; disclosed), from the numbers of ADR 0545 (its run 1 log `f541-scratch/run1-ignored.log`, read, and its
addendum). Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-631 (dep F-585), the follow-up proposed by
[ADR 0545](0545-f541-registration-shell3-on-the-remaining-paths.md) (its remaining question 3). Number 0603 given by the lead.
Branch `f631-work` from the local main tip 43092983, worktree `w49i-work/f631`, scratch `w49i-work/f631-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; existing tests not edited; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no
unsafe code; clean-room. The roadmap files are not edited (the entries go in "For the lead"). Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation).

## What F-631 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. An opt-in residual reference of the nonlinear transient with a registered inertia term, so that a converged step of a free vibration without external load is not rejected at a rounding floor; the Shell3 strip of ADR 0545 NT1 over 100 periods at the default tolerance with that reference; the default reference and its bits unchanged.

Clauses: **c0** every limit registered here; **c1** an opt-in residual reference of `nl_transient` with a registered inertia
term (D1, D2); **c2** a converged step of a free vibration without external load is not rejected at a rounding floor with it
(FT1, against the default NC1); **c3** the Shell3 strip of ADR 0545 NT1 over 100 periods at the default tolerance
(`NewtonOptions::new(1)`, `residual_tolerance = energy_tolerance = 1e-10`) with that reference (NT3); **c4** the default
reference and its bits unchanged (DB1, DB2, DB3).

## What was read

`AGENT_RULES.md`; the brief; ADR 0545 (NT1, amendment 1, addendum: NT1 stopped at step 3908 of 4000, `t = 794.89`, after 5
halvings with the relative residual `4.728e-10` against `1e-10` and the energy ratio `1.28e-27`; NT1b with `residual_tolerance =
1e-8` met F1 `2.05e-3`, F1b `8.0e-8`, F1c `7.9e-5`, E1 `2.84e-8`, EZ `5.14e-7`, 0 halvings, 11988 iterations, 35 s); ADR 0479 D1
to D4 (the step, `Contribution::in_residual_reference`, the halvings); ADR 0404 D5 (the relative residual `|R_F| / max(|lambda
f_F|, |f_int + g|)` and the energy test); `crates/rcfd-fem/src/nl_transient.rs` (`Inertia`, `Kinematics`, `substep`, `advance`,
the options); `crates/rcfd-fem/src/nonlinear.rs` (`Contribution`, `contribute_with`, `contribute_split`, `Problem::attempt_with`:
the reference is `max(|lt f|_free, |f_int + g|, |f_int|, |g_c|)` over the contributions `c` in the reference, the norms of the
internal and contribution forces taken over all equations); `crates/rcfd-fem/tests/sa_shell3_paths.rs` (the strip, NT1, NT1b,
`ez_clapeyron`).

## The floor, derived (disclosed arithmetic)

The step's residual (ADR 0479 D1, `f = 0`): `R(u) = -[f_int(u) + g(u)]`, `g = (M a_m + C v_f + alpha_f f_int,n)/(1 - alpha_f)`,
`a_{n+1} = (u - u~)/(beta h^2)`, `u~ = u_n + h v_n + h^2 (1/2 - beta) a_n`. The computed residual carries a rounding error
`|dR| <= c eps sum_i |t_i| + A_f`: the products `t_i` of which `g` is a sum (each of `M u`, `M u_n`, `h M v_n` ... rounded at
`eps` relative, `c` of the order of the row length of `M`, about 30 for the shell strip) and `A_f` the rounding of the internal
force of the corotated element, which does not scale with the displacement (the element frames and the local displacements are
differences of coordinates of size `l`, not of `u`).

The default reference `R0 = max(|f_int|, |g|)` is proportional to the instantaneous displacement in a free vibration (`g ~ -f_int
~ K u`), so it passes through a minimum at each zero crossing of the first mode, while `A_f` does not. With `F_pk` the peak of
`|f_int|` (reactions included), `R0 ~ F_pk |sin phi|`.

NT1's failure, located: the trapezoidal period elongation `(omega h)^2/12` with `omega h = 2 pi/40 = 0.157` is `2.06e-3`; at
`t = 794.89` the exact phase is `97.68` periods, the numerical `97.68/1.00206 = 97.48`, so `0.021` periods (`sin = 0.13`) from a
zero crossing. The floor then is `A_f ~ 4.7e-10 x 0.13 F_pk ~ 6e-11 F_pk` (an estimate: the phase is located to about `+-0.01`
period, so `A_f` within a factor 2). That a phase-dependent reference meets a constant floor explains why 3907 steps passed and
why the halvings did not help (they shrink `u_{n+1} - u_n`, not `A_f`).

The inertia-term reference (D1) `R1 >= (1/(beta h^2)) max(|M u_{n+1}|, |M u_n|, h |M v_n|, ...)`. In the free vibration `u_n = A
phi sin`, `h v_n = A phi omega h cos`: `max(|sin|, omega h |cos|) >= omega h / sqrt(1 + (omega h)^2) = 0.155` at every phase,
so `R1 >= 0.155 |M phi| A/(beta h^2) = 0.155 F_pk/(beta (omega h)^2) = 0.155 x 162 F_pk = 25 F_pk` (`|K phi| = omega^2 |M phi|`
for the mode; the reactions add to `F_pk` the same factor to both). The relative floor with `R1`: `c eps + A_f/R1 <= 30 x 2.2e-16
x 2 + 6e-11/25 = 1.3e-14 + 2.4e-12 ~ 2.5e-12`, a factor 40 below the default tolerance `1e-10`; with a sub-step `h/2^l` `R1`
grows by `2^l` at least. With `R0` the same relative floor at the phase `0.021` is `4.7e-10` (observed) and `6e-11/|sin phi|`
in general.

What the looser reference accepts: a residual `dR <= tol R1` changes `a_{n+1}` by `M^{-1} dR`, that is `u_{n+1}` by `beta h^2
M^{-1} dR`, of mass-norm size `<= tol max(|u_{n+1}|, |u_n|, h|v_n|, ...)`: the test becomes a relative tolerance `tol` on the
displacement of the step. The energy error per step is then `<= 2 tol ~ 2e-10`, over 4000 steps `<= 8e-7` (worst case, all of
one sign), below the E1 limit `1e-5`; the energy test of the driver (`1e-10` of the first correction's energy) is unchanged and
still applies.

## Design

### D1. The inertia-term reference

With the opt-in the reference of the relative residual of each attempt of the `Method` step is `max(R0, R1)`, `R0` the
reference of ADR 0404 and ADR 0479 D2 unchanged, and `R1` the largest norm (all equations, as the other references) of the
products that the inertia and damping force `g` of ADR 0479 D1 is a sum of, with `c_m = (1 - alpha_m)/((1 - alpha_f) beta h^2)`
and `c_c = gamma/(beta h)`:

- mass: `c_m M u_{n+1}`, `c_m M u_n`, `c_m h M v_n`, `c_m h^2 |1/2 - beta| M a_n`, `alpha_m/(1 - alpha_f) M a_n`;
- the internal force of the start: `alpha_f/(1 - alpha_f) f_int,n`;
- damping (`C v_f/(1 - alpha_f)`, `v_{n+1} = v_n + h (1 - gamma) a_n + c_c (u_{n+1} - u~)`): `C v_n`, `h (1 - gamma) C a_n`,
  `c_c C u_{n+1}`, `c_c C u_n`, `c_c h C v_n`, `c_c h^2 |1/2 - beta| C a_n`, `alpha_f/(1 - alpha_f) C v_n`.

`u_{n+1}` is the Newton iterate (the reference is re-taken at every iteration as `R0` is); the products of `u_n`, `v_n`, `a_n`
are formed once per sub-step. Derivation: first principles, the rounding bound of a sum (Higham, *Accuracy and Stability of
Numerical Algorithms*, ch. 3, from memory: the computed sum of `t_i` has an error `<= gamma_n sum |t_i|`); the reference is the
scale of the terms whose cancellation the residual is, the standard choice of a relative test (the same reason ADR 0479 D2 put
the inertia force in the reference). The largest norm, not their sum: within a factor of the term count (at most 13) of the
sum, and it keeps the reference equal to the inertia force's own scale when the terms do not cancel.

### D2. The option

`nl_transient::ResidualReference { Driver, InertiaTerms }` (`#[non_exhaustive]`, default `Driver`),
`NlTransientOptions::residual_reference` (field, `pub`, default `Driver`) and the builder
`NlTransientOptions::residual_reference(ResidualReference)`. `Driver` executes exactly the operations it executed. With
`InertiaTerms` and the energy-momentum step `Integrator::new` refuses (key `"nl_transient"`: the midpoint step has its own
driver call without the inertia contribution; not in F-631's scope).

### D3. The driver hook (`nonlinear.rs`, shared solver file)

`Contribution::residual_reference_terms(&self, u: &[f64], load_factor: f64) -> f64`, default `0.0`: an extra lower bound of the
reference, read only for a contribution with `in_residual_reference() == true`, and taken only when `> 0` (`if x > 0.0 {
reference = reference.max(x) }`), in `contribute_with` and `contribute_split`. With the default `0.0` no operation on the
reference changes, so every existing contribution and every default path keeps its bits. The `Inertia` contribution returns
`R1` with `InertiaTerms`, `0.0` with `Driver`.

## Checks (limits fixed now)

Model: the Shell3 strip of ADR 0545 (`strip3`, 32 Shell3, `L = 12`, `b = 1`, `t = 0.1`, `E = 1.2e6`, `nu = 0`, `rho = 1`,
clamped root), the start of NT1 (`u0 = 0`, `v0 = omega a phi/phi_z,tip`, `a = 1e-3 L`, the modal options of ADR 0522 amendment
4), the trapezoidal default, `NewtonOptions::new(1)` (default tolerances `1e-10`, 20 iterations, 5 halvings) unless stated. File
`crates/rcfd-fem/tests/sa_nl_inertia_reference.rs` (new; helpers copied from `sa_shell3_paths.rs`, not edited).

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| FT1 | c1, c2 | one step from the start at `h_f = h/1024` (`h = 1/(40 f_1)`) with `InertiaTerms`: converges with 0 halvings; final relative residual; `max_i |u_1,i - h_f v0_i/(1 + (omega h_f)^2/4)| / max_i |h_f v0_i|` (the linear trapezoidal first step of the mode) | converges, 0 halvings; `<= 1e-10`; `<= 1e-8` | holds; `<= 1e-10` by construction; `~1e-10` (the accepted relative displacement error, D1), nonlinearity `(a/L)^2` negligible |
| NC1 | c2 (negative control) | the same step with `Driver` | refused: `Solver { solver: "nl_transient" }` after the 5 halvings | `R0 ~ F_pk omega h_f = 1.5e-4 F_pk`, relative floor `6e-11/1.5e-4 = 4e-7` `>> 1e-10`, and smaller still at the halvings |
| FT2 | c1 (fast companion of NT3) | 40 steps (one period) at `h` with `InertiaTerms`: completes; halvings; `max |H_n - H_0|/H_0` | completes, 0 halvings; `<= 1e-5` | holds; `~3e-8` (NT1b E1) |
| NT3 | c3 | the NT1 run (4000 steps, 100 periods) with `InertiaTerms` at the default tolerance: completes; total halvings; F1, F1b, F1c, E1, EZ of ADR 0545 | completes, 0; `1e-2`, `1e-3`, `1e-2`, `1e-5`, `1e-4` | floor `~2.5e-12` (factor 40); NT1b values: `2.05e-3`, `8e-8`, `7.9e-5`, `2.8e-8`, `5.1e-7` |
| RF1 | D2 | `InertiaTerms` with the energy-momentum step | `InvalidValue { key: "nl_transient" }` | holds |
| DB1 | c4 | 40 default steps of the strip at `h`: FNV-1a 64 of the bit patterns of `u`, `v`, `a` after the run, against the hash recorded on the unchanged library (the test written and run once at the base library before any library line changes, its printed hash then fixed as the constant) | equal | equal (D3) |
| DB2 | c4 | the same 40 steps with `.residual_reference(ResidualReference::Driver)` explicitly | equal to DB1's constant | equal |
| DB3 | c4 | existing tests: `cargo test -p rcfd-fem --release --test sa_nl_transient` and the `*_default_bits` tests of the nonlinear driver (`sa_dg_default_bits`, `sa_dgm_default_bits`, `sa_emc_default_bits`), `sa_shell3_paths`; `cargo test -p rcfd-api --release --test baseline_hashes` | pass | pass |

NT3 is heavy (NT1b took 35 s in release; on the shared machine and in the debug profile several minutes): an ignored record,
run once with `--ignored`; FT1, FT2, NC1, RF1, DB1, DB2 run on every crate run (under a few seconds in release).

## Negative controls

NC1 above (the default reference refuses the converged step). If NC1 converges, the prediction of a constant floor is wrong for
this set-up: recorded, and FT1 then does not show c2 (decision partial).

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `Inertia::residual_reference_terms` returns `0.0` with `InertiaTerms` | FT1 (refused) |
| MU2 | the option not stored (the integrator always `Driver`) | FT1 |
| MU3 | `c_m` without `1/(beta h^2)` | FT1 (`R1` then `~ beta (omega h_f)^2` of the inertia force, below `R0`) |
| MU4 | the inertia-term reference also with `Driver` | NC1 (converges); DB1 (doubt: the earlier stop may or may not change a bit in 40 steps) |
| MU5 | the refusal with the energy-momentum step removed | RF1 |
| MU6 | in `contribute_with` the hook read for every contribution, the `> 0` guard removed (`reference.max(x)`) | none expected (equivalent for finite references): registered as a doubt, to show the guard is defensive only |

## Decision rule

- **done:** FT1, NC1 (refused as predicted), FT2, NT3, RF1, DB1, DB2, DB3 met; mutations MU1 to MU5 caught.
- **partial:** NT3 missed or not run, or NC1 converges (c2 not shown), or a mutation of MU1 to MU5 not caught; every other check
  met.
- **todo:** DB1, DB2 or DB3 missed (the default bits changed) or FT1 missed.

## Limits (what is not covered)

- The energy-momentum step (refused with the option, D2); the arc-length and static paths (no inertia).
- `A_f` itself (the corotated Shell3 rounding) is not reduced; the option only measures the residual against the scale of the
  inertia terms. A model whose floor is in `A_f` and with negligible inertia (a very slow transient, `beta (omega h)^2 >> 1`
  for every mode) gains nothing.
- With a large rigid drift (a free body far from its start) `|M u_{n+1}|/(beta h^2)` grows with the drift and the test becomes
  relative to the drifted displacement; that is the rounding of `a_{n+1}` in the same arithmetic, but it is looser than `R0` in
  absolute terms. Not checked.
- The floor estimate rests on the phase located from the period elongation (factor 2); NT3 measures the run, not the floor.
- The option is not exposed in the case schema, CLI or MCP (goldens unchanged).

## Addendum 1: results (2026-11-15)

Runs: DB1's hash recorded once on the unchanged library at 8333080a (`f631-scratch/db1-record.log`, `0xa1c3afbcfc5e3fc6`) before
any library line changed; run 1 at dd18e312 (`run1-fast.log`, the fast tests once at 832b1049, which differs from dd18e312 only
in the spacing of the RF1 refusal text; `run1-nt3.log`, NT3 once at dd18e312); B1 and N2 at dd18e312 (`b1-fem.log`,
`n2-baseline.log`); mutations on a `git archive` copy of dd18e312 with its own target (`MU1.log` to `MU6.log`,
`mutations.log`). The commit 0f6fbed6 after the runs is rustfmt only. Shared solver file `crates/rcfd-fem/src/nonlinear.rs`:
three insertions (the trait method, lines 1363 to 1368; `contribute_with`, 1493 to 1497; `contribute_split`, 1532 to 1536), no
line removed.

| Check | Limit | Measured | Met |
|---|---|---|---|
| FT1 converges, halvings; final residual; first step against the linear trapezoidal step | converges, 0; `1e-10`; `1e-8` | converges, 0 halvings, 2 iterations; `1.25e-16`; `8.9e-10` | yes |
| NC1 `Driver` refused | refused (`nl_transient`) | refused after 5 halvings: relative residual `1.48e-7` at `h_f/32`, energy ratio `8.9e-34` | yes (as predicted, floor estimate `4e-7` at `h_f`) |
| FT2 one period, halvings; E1 | completes, 0; `1e-5` | completes, 0 halvings, 120 iterations, worst residual `4.9e-13`; `2.77e-8` | yes |
| NT3 completes; halvings; F1, F1b, F1c, E1, EZ | 4000 steps, 0; `1e-2`, `1e-3`, `1e-2`, `1e-5`, `1e-4` | 4000 steps, 0 halvings, 11988 iterations, worst residual `6.1e-13`; `2.05e-3`, `7.98e-8`, `7.95e-5`, `2.84e-8`, `5.14e-7` (34.9 s) | yes |
| RF1 refusal with the energy-momentum step | `InvalidValue { key: "nl_transient" }` | refused naming ADR 0603 D2; the same model taken with `Driver` | yes |
| DB1 default hash | equal to the record | `0xa1c3afbcfc5e3fc6` equal | yes |
| DB2 explicit `Driver` | equal | equal | yes |
| DB3 `cargo test -p rcfd-fem --release` (includes `sa_nl_transient`, the `*_default_bits` tests, `sa_shell3_paths`); `baseline_hashes` | pass | 1294 passed, 0 failed, 329 ignored; `baseline_hashes` 3 passed, 1 ignored as before | yes |

`cargo fmt -p rcfd-fem --check` clean (after 0f6fbed6), `cargo clippy -p rcfd-fem --all-targets --release -- -D warnings` clean.

The estimate held: the worst relative residual of the 100 periods with the inertia-term reference, `6.1e-13`, sits below the
registered floor estimate `2.5e-12` and a factor 160 below the tolerance; the run reproduces NT1b of ADR 0545 (F1, F1c, E1, EZ
equal to three digits, the same iteration count) at the default tolerance `1e-10` instead of `1e-8`. NC1 confirms the floor that
does not scale with the displacement: at `h_f/32` the default relative residual stalls at `1.5e-7`, the order the constant floor
`A_f ~ 6e-11 F_pk` predicts.

**Mutations:**

| Id | Caught by |
|---|---|
| MU1 the reference returns 0 with `InertiaTerms` | FT1 (refused, `1.48e-7`) |
| MU2 the option not stored | FT1 |
| MU3 `c_m` without `1/(beta h^2)` | FT1 |
| MU4 the inertia-term reference also with `Driver` | NC1 (converged); DB1 and DB2 **not** (the bits of the 40 default steps unchanged: the registered doubt, the default run already converged at the same iterations) |
| MU5 the energy-momentum refusal removed | RF1 |
| MU6 the hook read for every contribution without the `> 0` guard | not caught, as registered (equivalent for finite references) |

MU1 to MU5 caught, as the rule requires; MU6 not caught as predicted.

**Deviations:** none. **Not run:** nothing registered was skipped.

**Decision by the registered rule: done.** FT1, NC1, FT2, NT3, RF1, DB1, DB2, DB3 met; MU1 to MU5 caught.

### Remaining questions

1. Default. (a) Keep `Driver` the default (bits); (b) make `InertiaTerms` the default of the `Method` step in a later item
   (changes default results: maintainer's decision, not taken here). Recommendation (a) now; (b) is listed as open, since it
   would change default bits.
2. The energy-momentum step. (a) Leave refused; (b) a follow-up giving the midpoint step the same reference (its residual has the
   same inertia terms `M (u_{n+1} - u_n - h v_n)` scaled by `2/h^2`). Recommendation (b) as a small follow-up, proposed below.
3. Exposure in the case schema / CLI / MCP. (a) Not now (goldens); (b) later with the nonlinear transient case keys.
   Recommendation (a).

## For the lead: roadmap entries

F-631: proposed status **done**. Note:

> Structural solver, advanced tier. Follow-up of F-541 (ADR 0545, 2026-11-15). DONE. ADR 0603, 2026-11-15: an opt-in residual reference of the nonlinear transient, NlTransientOptions::residual_reference(ResidualReference::InertiaTerms): the reference of each Newton attempt of the Method step is also at least the largest norm of the products the inertia and damping force is a sum of (c_m M u_{n+1}, c_m M u_n, c_m h M v_n, c_m h^2 |1/2 - beta| M a_n, alpha_m M a_n, alpha_f f_int,n and the damping products, c_m = (1 - alpha_m)/((1 - alpha_f) beta h^2)), so that the test is a relative tolerance on the step's displacement and the constant rounding floor of the corotated Shell3 internal force (about 6e-11 of its peak, derived from ADR 0545's stop near a zero crossing) no longer rejects a converged step. The Shell3 strip of ADR 0545 NT1 over 100 periods at the default tolerance 1e-10: 4000 steps, 0 halvings, worst residual 6.1e-13, F1 2.05e-3, F1b 8.0e-8, F1c 7.9e-5, E1 2.8e-8, EZ 5.1e-7 (limits of ADR 0522); a step at the zero crossing at h/1024 converges in 2 iterations to 1.3e-16 with the option and is refused by the default reference (stall at 1.5e-7). Refused with the energy-momentum step. A driver hook Contribution::residual_reference_terms (default 0, read only when positive); the default reference and its bits unchanged (a 40-step hash recorded before the change, every rcfd-fem test, baseline_hashes). Five of five registered mutations caught.

> Follow-up proposed: The inertia-term residual reference on the energy-momentum step. Deps F-631, F-611. Acceptance: Every limit not stated here is registered in the ADR before the run. ResidualReference::InertiaTerms taken by the energy-momentum step with the products of its midpoint inertia force; a free vibration of a solid without external load over a registered number of periods at the default tolerance with no step rejected at a rounding floor and the energy conserved within a registered limit; the default bits unchanged.
