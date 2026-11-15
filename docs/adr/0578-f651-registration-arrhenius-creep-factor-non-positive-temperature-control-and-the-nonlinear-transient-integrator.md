# ADR 0578: F-651 registration: the Arrhenius creep factor, a registered control of every non-positive temperature, and the factor material in the nonlinear transient integrator

Status: **REGISTERED 2026-11-15, written before any code of F-651 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text no library or test code of F-651 was written, compiled or run. The files under "What was read" were
read, and the scalar arithmetic below was evaluated with one plain python script (`w49i-work/f651-scratch/probe.py`, log
`probe.log`; scalars only, no rcfd call; disclosed, its numbers quoted below). The bit record B0 (D5) is a test file written and
run on the unchanged base e348eb34 after this commit and before any library code. Results go in the addenda at the end; nothing
above the addenda is changed after a run.

Roadmap item F-651 (dep F-581), the follow-up proposed by [ADR 0558](0558-f581-registration-arrhenius-temperature-factor-for-creep-on-the-time-stepping-driver.md)
(its remaining questions 1 and 3). Number 0578 given by the lead. Branch `f651-work` from e348eb34, worktree `w49i-work/f651`,
scratch `w49i-work/f651-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored `*_known_miss`
test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no edit of an
existing test, of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe
code; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation). A case-file key is out
of scope (maintainer's call, ADR 0558 question 2).

## What F-651 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A creep material with the Arrhenius factor is refused with the key temperature at every non-positive temperature, including T = -1e10 K, by a control registered before its run, and mutation MU5 of ADR 0558 is caught by it; Integrator::new refuses a factor material without a temperature option; a one-element bar of the factor material under a step load in the nonlinear transient integrator at a uniform temperature T creeps, after the vibration has decayed, at the closed-form rate with A exp(-Q/(R T)) within 1 percent. Materials without the factor keep their bits.

Clauses: (a0) every limit registered here; (a1) refusal with key `"temperature"` at every non-positive temperature including
`-1e10 K`, by a control registered here (NP1, NP2, NP3), and MU5 of ADR 0558 caught by it (mutation MU1 below); (a2)
`nl_transient::Integrator::new` refuses a factor material without a temperature option (NI1); (a3) one Hex8 of the factor
material under a step load in the nonlinear transient integrator at a uniform temperature creeps, after the vibration has
decayed, at `A exp(-Q/(R T)) sigma^n` within `1e-2` (CR1, CR2, with the decay itself checked, DV); (a4) materials without the
factor keep their bits (B0, B1, the existing default-bits files, `baseline_hashes`).

Readings, registered:

- **(a1) "every non-positive temperature"** is the set `T <= 0` of `f64` (with `-0.0`), checked on a registered finite set that
  covers every regime of the floating-point evaluation of `exp(-Q/(R T))` (arithmetic 2): zero of both signs, subnormal,
  smallest normal, tiny, the band where the mutant's exponent overflows, the boundary `|T| = Q/(R ln(f64::MAX)) = 33.89 K`, the
  band where the mutant gives a finite factor `> 1`, `-1e10`, and `-f64::MAX`. The refusal is checked where it lives (the factor,
  `mat_creep::arrhenius_factor`, and the point update, `mat_creep::update`) and, new here, up front at the two drivers that take
  a temperature option (D2): a refusal at a point inside a driver step reaches the caller as a step error (`solve_in_time_with`:
  a solver error after its halvings, ADR 0558 MU8 record; `nl_transient`: an element error), not as the key, so (a1) at the
  drivers needs the up-front check.
- **(a2) "a temperature option"**: any of `NlTransientOptions::temperature`, `temperature_history`, `temperature_field`, the same
  three as the driver check of ADR 0558 D3.
- **(a3) "a one-element bar"**: the unit cube of one Hex8 with three symmetry planes of `sa_creep_arrhenius.rs` (`cube`), its
  `xmax` face under a uniform traction `sigma` applied as a step at `t = 0` (a `LoadHistory` with the constant factor 1 on the
  pattern of the traction). "After the vibration has decayed": the rate is measured on a window that starts after a decay time
  fixed by arithmetic 4, and the decay is itself checked (DV). "The closed-form rate": with `m = 0` the steady uniaxial creep rate
  under the constant stress `sigma` is `d eps_c/dt = A f(T) sigma^n`, `f(T) = exp(-Q/(R T))` (ADR 0558 arithmetic 1).

## What was read (no run)

- `AGENT_RULES.md`, the brief; ADR 0558 (whole: D1 to D5, NA1, NA2, NA2b, MU5, MU8, the remaining questions); ADR 0489 (the law,
  D1, the remark that the nonlinear transient integrator takes the creep material); ADR 0499 (D3, the temperature in the
  integrator, NT2, NT3, V1T); ADR 0479 (the step, the halvings, `retryable`); ADR 0573 (form of the registration and of the
  roadmap entries).
- `crates/rcfd-fem/src/mat_creep.rs` (`arrhenius_factor`, `update`, `temperature_error`), `nl_transient.rs` (module text,
  `NlTransientOptions`, `Integrator::new`, `start`, `advance`, `substep`, `load`, `temperature_at`, `retryable`),
  `nonlinear.rs` (`point_update`, `TimeSteppingOptions`, `solve_in_time_with` up to its time loop, the D3 check of ADR 0558),
  `material.rs` (`PointContext::with_temperature`), `transient.rs` (`Method`, `LoadTable`, `LoadHistory`), `model.rs` (`DofMap`,
  `Material::creep_arrhenius`).
- Tests: `sa_creep_arrhenius.rs` (helpers, P1, NA1 to NA3, NA2b), `sa_creep_arrhenius_default_bits.rs`, `sa_qs_visco.rs` (the
  rod and its modal frequency, NT2, NT3, B2).

## Design

### D1. Nothing changes in the factor or the point update

`mat_creep::arrhenius_factor` already refuses `!(T.is_finite() && T > 0)` (key `"temperature"`) and the underflow; `update` reads
the factor only on a creeping step. F-651 adds no change to either: the missing piece of ADR 0558 is the registered control (NP1),
which the planned mutation MU5 of ADR 0558 must fail.

### D2. An up-front temperature check of a factor material at both drivers

A new crate function `mat_creep::check_factor_temperature(model, constant, history, field) -> Result<()>`: if no material of the
model is a creep material with the factor, `Ok(())` and nothing else is read. Otherwise refused with
`FemError::InvalidValue { key: "temperature" }` naming the material's index and element set: (i) no temperature option at all
(`constant`, `history` both `None` and no field); (ii) a constant temperature that is not finite or not `> 0`; (iii) a history
with a point value that is not finite or not `> 0` (the table is piecewise linear, so its minimum over its whole range is the
minimum of its point values; the whole table is checked, not only the run's range, which is not known to `Integrator::new`). A
nodal field is not checked up front (its values reach a point through shape functions, which are not a convex combination for
the quadratic elements); a non-positive point temperature of a field is refused at the point (D1) and reaches the caller as a
step error (Limits).

- `nl_transient::Integrator::new`: called after the existing temperature checks (so a `NaN` keeps its message of ADR 0499 NT3)
  with `options.temperature`, `options.temperature_history`, `options.temperature_field.is_some()`. Before F-651 a factor
  material without a temperature was accepted by `new` and failed at its first creeping step.
- `nonlinear::solve_in_time_with`: the existing D3 block of ADR 0558 is kept unchanged (its message is the record of NA1 (ii));
  the new call follows it, so only (ii) and (iii) are new there.

Bits: the function returns before reading anything when the model has no factor material, and for a factor material with
positive temperatures it only returns `Ok`; no arithmetic of any run changes (B0, B1).

### D3. The creep rate in the integrator (CR1, CR2): set-up fixed now

The cube of `sa_creep_arrhenius.rs` (unit cube, one Hex8, `xmin`/`ymin`/`zmin` symmetry planes), `E = 2e9 Pa`, `nu = 0.3`,
`rho = 1200 kg/m^3`, `n = 3`, `m = 0`, `Q = 2e5 J/mol`, the traction `sigma = 2e3 Pa` on `xmax` (elastic strain `sigma/E =
1e-6`), as `LoadHistory::new(nodal_load_vector(pulled model, &[]), LoadTable::constant(1.0, 0.0, t_end))` on the model without
the load. `omega_1` is the lowest eigenvalue's square root of `modal_analysis` of the cube with the elastic base (the creep
material is refused by the modal path), `P = 2 pi / omega_1`. Integrator: `NlTransientOptions::new(P/40)`, method
`GeneralisedAlpha { rho_inf: 0.5 }`, Rayleigh mass damping `alpha_R = omega_1`, stiffness part 0, Newton `residual_tolerance =
1e-12` (else `NewtonOptions::new(1)`), `temperature(T)`; start at `t0 = 0` from rest; 1280 steps (`32 P`). Measured on the
window `[t_a, t_b] = [16 P, 32 P]` (steps 640 and 1280).

The pre-exponential factor: `A = A_2 / f(T2)` with `A_2 = 1/(E t_c sigma^(n-1))`, `t_c = 1e4 P` (the creep time `(sigma/E)/(A_2
sigma^n)` at `T2`), `T1 = 700 K`, `T2 = 750 K`. CR2 runs at `T2` (`d eps_c/dt = A f(T2) sigma^3 = sigma/(E t_c)`), CR1 at `T1`
with the same material (`f(T1)/f(T2) = 1/9.8838`, so the rate is ten times lower; the measured window still holds a creep
displacement `1.6e-4 u_s`, five decades above every noise below).

Measures (every one against its closed form, relative error):
- **rate by the displacement**: the mean `u_x` of the four `xmax` nodes, `(u(t_b) - u(t_a))/(t_b - t_a)` against `L A f(T)
  sigma^3`, `L = 1 m`;
- **rate by the creep state**: at each of the 8 points `(eps_c,xx(t_b) - eps_c,xx(t_a))/(t_b - t_a)` against `A f(T) sigma^3`,
  the worst point;
- **DV (the vibration has decayed)**: the mean `v_x` of the `xmax` nodes at every step end of the window against `L A f(T)
  sigma^3`, the worst step.

### D4. Bits

B0: a new file `sa_creep_arrhenius_nlt_default_bits.rs`, written and run on the base e348eb34 before any library code, its
digests recorded then: (1) `nl_transient` of the plain creep cube (`Material::creep`, the same law and `A f(T1)` as its `a`) under
the step load, 60 steps, without a temperature; (2) the same with `temperature(700)`; (3) the same with `temperature(-5)` (a
plain creep material ignores the temperature; accepted before and after); (4) the factor cube with `temperature(700)`, 60 steps
(it ran on the base already); (5) `solve_in_time_with` of the pulled factor cube with `temperature(700)`, 20 steps; (6) the
elastic cube in `nl_transient` with `temperature_history`. Each digest over `u, v, a, internal force` of every step end and the
state bytes (`solve_in_time_with`: every field of every `TimeStep`). The file is committed with the digests from the base and
re-run unchanged on the tip (B1).

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| NP1 | a1 | the factor material (`Q = 2e5`): `arrhenius_factor(Q, T)` and `update` with an increment at `T` (point context `with_temperature(T)`), for every `T` of the set `S` = {`0.0`, `-0.0`, `-5e-324`, `-f64::MIN_POSITIVE`, `-1e-300`, `-1e-3`, `-1`, `-33`, `-34`, `-35`, `-100`, `-700`, `-1e3`, `-1e5`, `-1e10`, `-1e100`, `-1e300`, `-f64::MAX`}; and `-inf` (`arrhenius_factor`, and `with_temperature(-inf)` itself): key `"temperature"` | every one refused | refused by `T > 0` (D1) |
| NP2 | a1 | `Integrator::new` of the pulled factor cube with `temperature(T)`, `T` in `S_d` = {`0.0`, `-0.0`, `-5e-324`, `-1`, `-34`, `-1e10`, `-f64::MAX`}, and with `temperature_history` `[(0, 700), (1, 700), (2, T)]` for the same `T`: `InvalidValue` key `"temperature"` at the top level | all 14 refused | D2 (ii), (iii) |
| NP3 | a1 | `solve_in_time_with` of the pulled factor cube, times `[0.1, 0.2]`, the same 14 options: `InvalidValue` key `"temperature"` at the top level | all 14 refused | D2 |
| NI1 | a2 | `Integrator::new` of the pulled factor cube with `NlTransientOptions::new(1e-3)` (no temperature option): `InvalidValue` key `"temperature"` | refused | D2 (i) |
| PC1 | a1, a2, a4 | positive controls: `Integrator::new` accepts the factor cube with `temperature(700)` and with the history `[(0, 700), (2, 750)]`; `Integrator::new` and `solve_in_time_with` accept the plain creep cube without a temperature and with `temperature(-1)`; `arrhenius_factor(Q, 700)` is `Ok`, positive and finite | all accepted | D2 returns before reading for a plain material |
| CR2 | a3 | the rate at `T2 = 750 K` (D3): by the displacement; by the creep state, worst of 8 points | `<= 1e-2` each | `<= 9e-4` (arithmetic 5) |
| CR1 | a3 | the same at `T1 = 700 K` with the same material | `<= 1e-2` each | `<= 1e-4` |
| DV | a3 | the mean `v_x` of `xmax` at every step end of the window against the creep velocity, at `T2` and at `T1` | `<= 1e-2` each | `<= 9e-4` |
| CRr | a3 | the ratio of the displacement rates at `T2` and `T1` against `f(T2)/f(T1) = 9.8838` (the factor is read from the integrator's temperature) | `<= 1e-2` | `<= 1e-3` |
| B1 | a4 | B0 re-run on the tip: the six digests | bits equal | D2 |
| B2 | a4 | `sa_creep_default_bits`, `sa_creep_arrhenius_default_bits`, `sa_nl_transient_default_bits`, `sa_qs_visco_default_bits`, `sa_temp_history_default_bits`, `sa_temp_field_default_bits`; the existing tests of `sa_creep_arrhenius.rs`, `sa_creep.rs`, `sa_qs_visco.rs`; `cargo test -p rcfd-api --release --test baseline_hashes` | all pass unchanged | no default arithmetic touched |

## Negative controls

NP1 to NP3 and NI1 are refusals; PC1 is their positive side (a check that refuses everything fails PC1). CRr is the control that
the rate reads the integrator's temperature (a factor fixed at one temperature gives `1`, not `9.88`).

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | MU5 of ADR 0558: the check `T > 0` removed from `arrhenius_factor` (finiteness kept) | NP1 (`T <= -34 K`: the mutant's factor is finite, arithmetic 2) |
| MU2 | the missing-temperature part (i) of D2 removed from `check_factor_temperature` | NI1 |
| MU3 | the call of D2 removed from `Integrator::new` | NI1, NP2 |
| MU4 | the call of D2 removed from `solve_in_time_with` | NP3 |
| MU5 | D2 (ii), (iii) refuse only `t < 0.0` (zero of both signs accepted) | NP2, NP3 (`0.0`, `-0.0`) |
| MU6 | D2 (iii) removed (the history not checked) | NP2, NP3 (history rows) |
| MU7 | D2 refuses every factor material (the condition of (i) always true) | PC1, CR1, CR2 |
| MU8 | the factor dropped in `update` (`a_t = a`) | CR1, CR2, CRr |
| MU9 | the factor in `update` taken at `T1 = 700 K` whatever the context | CR2, CRr |

## Decision rule

**done**: every check NP1 to B2 met on its first run and every mutation caught. **partial**: any registered check missed
(recorded as a `*_known_miss`), any mutation uncaught, or a check met only after a post-run amendment. **todo**: CR1, CR2 not run.

## Arithmetic (disclosed, `probe.py`)

1. `f(700) = 1.19154e-15`, `f(750) = 1.17770e-14`, `f(750)/f(700) = 9.88385` (as ADR 0558 arithmetic 3).
2. **Where MU5 escapes.** Without `T > 0`, `arrhenius_factor` computes `exp(-Q/(R T))` and refuses only a factor that is zero or
   not finite. For `T < 0` the exponent is `Q/(R |T|) > 0`; it overflows `exp` when `Q/(R |T|) > ln(f64::MAX) = 709.78`, i.e.
   `|T| < 33.89 K`. So the mutant refuses `0.0` (`exp(-inf) = 0`), `-0.0` (`exp(+inf)`), the subnormal, the smallest normal,
   `-1e-300`, `-1e-3`, `-1`, `-33` (all overflow) and accepts every `T <= -34 K`: `-34` (`1.8e307`), `-35`, `-100`, `-700`
   (`8.4e14`), `-1e3`, `-1e5` (`1.27`), `-1e10` (`1.0000024`), `-1e100`, `-1e300`, `-f64::MAX` (`1.0`). Ten of the 18 values of
   `S` (and `-inf`, which `exp(-(Q/(R(-inf)))) = exp(0) = 1` accepts too, but which `with_temperature` refuses before the update)
   catch the mutant; NA2 of ADR 0558 tried `0` and `-1` only, both in the overflow band.
3. **The period** is not fixed by hand: `omega_1` is read from `modal_analysis` of the elastic cube, a closed derivation of the
   time step, the damping and the window (D3), not a search. (One Hex8, `c = sqrt(E/rho) = 1291 m/s`; `omega_1 L/c` is of order
   `sqrt(3)`, so `P` is a few ms; nothing below depends on its value.)
4. **Decay.** Mass-proportional damping `alpha_R` damps every underdamped mode with the envelope `exp(-alpha_R t/2)`; with
   `alpha_R = omega_1` every mode is underdamped (`zeta_k = omega_1/(2 omega_k) <= 1/2`). Over `16 P` the envelope is
   `exp(-16 pi) = 1.5e-22`. With the trapezoidal rule and `h = P/40` the per-step decay of a mode with `omega_k = r omega_1` is
   `|lambda|^2 = (1 - alpha h/2 + (omega h)^2/4)/(1 + alpha h/2 + (omega h)^2/4)`: over 640 steps `1.8e-22` (`r = 1`), `1.3e-20`
   (`4`), `3.1e-14` (`10`), `5.0e-7` (`20`); the generalised-alpha `rho_inf = 0.5` adds numerical dissipation for `omega h >~ 1`,
   which removes the slow tail of the high modes. The generalised-alpha step is exact for the steady state (`a = 0`, `v`
   constant, `f_int,n = f_int,n+1 = f`). Effect on the rate by the displacement: `2 x residual / creep displacement over the
   window` `<= 2 t_c/W x 3.1e-14 = 4e-11` (`r <= 10`); on DV: the residual velocity `omega_k u_k e^...` against `u_s/t_c`: `<=
   2 pi r x 1e4 x 3.1e-14 = 2e-8` (`r = 10`).
5. **The damping bias.** In the steady state the damping force `alpha_R M v` opposes the creep velocity and lowers the stress the
   load leaves to the material. Bound: the whole mass at the axial velocity `L eps_dot`: `|Delta sigma|/sigma <= alpha_R rho L^2
   eps_dot/sigma = (omega_1 L/c)^2/(2 pi t_c/P)`, with `(omega_1 L/c)^2 <= 12` (generous for one Hex8) `<= 1.9e-4`; the rate goes
   as `q^3` and `q` takes the lateral components too (`x 1.5`): `<= 8.6e-4` at `T2`, `8.7e-5` at `T1` (`t_c` ten times larger).
   The Green-Lagrange strain and the second Piola-Kirchhoff stress of the total Lagrangian solid differ from the small-strain
   ones by `O(1e-6)` at the strain `1e-6`: `3e-6` in the rate. Newton: residual `1e-12`, per step an error `~1e-12` of the
   stress against a creep increment per step `h/t_c = 2.5e-6` of `u_s`: negligible. Estimate CR2, DV `<= 9e-4`; CR1 `<= 1e-4`;
   CRr the difference of the two `<= 1e-3`.

## Limits (what is not covered)

- A nodal temperature field is not checked up front (D2): a non-positive point temperature of a field is refused at the point
  (key `"temperature"` inside the step error), not as a top-level key.
- A history is checked over all its points, also those outside the run's times (a table that goes non-positive after the run's
  end is refused).
- The rate is checked for `m = 0` (steady creep), one Hex8, uniform temperature, at two temperatures; not under a temperature
  history in the integrator, not for `m != 0` (whose rate depends on the time of the load), not at finite strain.
- No case-file key (maintainer's call).

## Addendum 1: results (2026-11-15)

Commits: e78c80e6 (this registration, alone), db81ebf5 and 019ec212 (the bit record B0, its digests printed by the file run on
the unchanged base e348eb34, `f651-scratch/b0-base.log`, then rustfmt), b267ec03 (D2 in `mat_creep.rs`, `nl_transient.rs`,
`nonlinear.rs`, and the tests `sa_creep_arrhenius_nlt.rs`), c8b90ba6 (rustfmt). Each registered check ran once, at b267ec03
(`f651-scratch/run1.log`), release profile. No limit was changed and no test was re-run to pick a result.

Library lines: `mat_creep.rs` module text (4 lines), `use` (2 lines), `check_factor_temperature` (new, 49 lines after
`arrhenius_factor`); `nl_transient.rs` the doc of `Integrator::new` (2 lines) and the call after the field checks (7 lines);
`nonlinear.rs` the call after the D3 block of ADR 0558 in `solve_in_time_with` (7 lines, 2694 to 2700).

| Check | Limit | Measured | Met |
|---|---|---|---|
| NP1 `arrhenius_factor` and `update` at the 18 values of `S`, `arrhenius_factor(-inf)`, `with_temperature(-inf)` | key `"temperature"`, all 38 | all 38 | yes |
| NP2 `Integrator::new`, 7 constants and 7 history points | top-level `InvalidValue` key `"temperature"`, all 14 | all 14 | yes |
| NP3 `solve_in_time_with`, the same 14 | the same | all 14 | yes |
| NI1 `Integrator::new` without a temperature option | refused, key `"temperature"` | refused | yes |
| PC1 the 7 positive controls | all accepted | all 7 | yes |
| CR2 rate at 750 K by the displacement | `<= 1e-2` | `6.010e-5` | yes |
| CR2 rate at 750 K by the creep state, worst of 8 points | `<= 1e-2` | `5.809e-5` | yes |
| DV at 750 K, worst step of the window | `<= 1e-2` | `6.010e-5` | yes |
| CR1 rate at 700 K by the displacement | `<= 1e-2` | `1.057e-5` | yes |
| CR1 rate at 700 K by the creep state | `<= 1e-2` | `8.575e-6` | yes |
| DV at 700 K | `<= 1e-2` | `1.058e-5` | yes |
| CRr ratio of the rates against `f(T2)/f(T1) = 9.883849` | `<= 1e-2` | `9.883359` (`4.95e-5`) | yes |
| B1 the six digests of B0 on the tip | bits equal | equal | yes |
| B2 the listed default-bits files, `sa_creep_arrhenius`, `sa_creep`, `sa_qs_visco`, `baseline_hashes` | all pass | all pass; beyond the list also the `rcfd-fem` unit tests, `sa_nl_transient`, `sa_temp_history`, `sa_temp_field`, `sa_visco`, `sa_emc`, `sa_dg`, `sa_scaffold` and the default-bits files of corot_shell_dyn, dg, dgm, emc, hyper, hyper_paths, temp_predictor, visco, si_fbar | yes |

Set-up values printed by the run: `omega_1 = 1961.161 rad/s`, `P = 3.2038e-3 s`, `A = 3.3129e-4`, `A f(T2) = 3.9016e-18`, window
`[0.05126, 0.10252] s`. The measured rate errors are below the bound of arithmetic 5 (`8.6e-4` at `T2`, `8.7e-5` at `T1`) and
scale with the creep rate as the damping bias does (`6.0e-5 / 1.06e-5 = 5.7`; the bound's `9.88` assumes the whole mass at the
full velocity). B0 note: four of its digests are equal (a plain creep material does not read the temperature, and `A_EFF / f1 *
f1` rounds back to `A_EFF`, so the factor material at 700 K does the same arithmetic); so it was on the base too.

Mutations (scratch copy of b267ec03, `f651-scratch/mut`, own target dir; `mutate.py`, logs `MU*.log`):

| Id | Caught by |
|---|---|
| MU1 (MU5 of ADR 0558, `T > 0` removed) | NP1, 20 rows: `arrhenius_factor` accepted every `T <= -34 K` (`-1e10 K` gives `1.0000024`, `-34 K` gives `1.8e307`); `update` at those temperatures returned a stress (`-700 K` and below) or failed with the key `"creep iteration"` (`-34`, `-35`, `-100 K`) |
| MU2 (D2 (i) removed) | NI1 |
| MU3 (D2 call removed from `Integrator::new`) | NP2 (14 rows), NI1 |
| MU4 (D2 call removed from `solve_in_time_with`) | NP3 (14 rows: a solver error after 5 halvings in place of the key) |
| MU5 (zero accepted) | NP2, NP3 at `0.0` and `-0.0` (8 rows) |
| MU6 (history not checked) | NP2, NP3 history rows (14) |
| MU7 (every factor material refused) | PC1 (2 rows), and the CR run (refused at `Integrator::new`) |
| MU8 (factor dropped in `update`) | CR1, CR2, DV (errors `5.4e4`, `5.4e5`) |
| MU9 (factor at 700 K whatever the context) | CR2 (`0.899`), CRr (`0.899`), NP1 `update` at `0` and `-0.0` |

Nine of nine caught. `cargo fmt -p rcfd-fem --check` clean; `cargo clippy -p rcfd-fem --all-targets -- -D warnings` clean.

Deviations: none from the registered set-up. Not run: the whole workspace suite (the lead's gate).

**Decision by the registered rule: done.** Every check NP1 to B2 met on its first run, every planned mutation caught, no
post-run amendment.

### Remaining questions

1. **A nodal temperature field with a non-positive value** reaches the caller as a step error, with the key `"temperature"` only
   inside it (D2, Limits). Options: (a) leave; (b) check the field's nodal values up front for meshes of linear elements only;
   (c) make the drivers surface a point refusal with the key `"temperature"` as a top-level error (not retryable).
   Recommendation: (a) now, (c) if a field-driven creep case appears; (c) changes the error of other materials too and is a
   separate item.
2. **A history refused for a point outside the run's times** (D2 (iii)). Options: (a) keep (simple, conservative); (b) check only
   `[0, t_last]` in `solve_in_time_with`, where the times are known. Recommendation: (a), taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation).
3. **Case-file key** (ADR 0558 question 2) stays the maintainer's call.

## For the lead: roadmap entries

> Proposed status: done.

> Structural solver, advanced tier. Follow-up of F-581 (ADR 0558, 2026-11-15). DONE. ADR 0578, 2026-11-15: mat_creep::check_factor_temperature, called by nl_transient::Integrator::new and nonlinear::solve_in_time_with, refuses a creep material with the Arrhenius factor before any step (key temperature) without a temperature option, with a constant temperature that is not positive and finite, or with a temperature history with such a point; a model without the factor returns before the options are read. Met on the first run: NP1, the registered control of every non-positive temperature (18 values from 0, -0.0 and the subnormal through -34 K and -1e10 K to -f64::MAX, and -inf), refused at arrhenius_factor and the point update, 38 of 38; the same as a top-level refusal at Integrator::new and at solve_in_time_with, 14 of 14 each; Integrator::new without a temperature option refused; seven positive controls accepted; one Hex8 of the factor material under a step load in the nonlinear transient integrator (generalised-alpha rho_inf 0.5, mass damping omega_1, 1280 steps of P/40) at 750 K creeps on the window [16 P, 32 P] at A exp(-Q/(R T)) sigma^3 within 6.0e-5 (displacement), 5.8e-5 (creep state), the velocity within 6.0e-5, at 700 K within 1.1e-5, the ratio of the rates against f(750)/f(700) = 9.88 within 5.0e-5; default bits (six new digests from the base, the default-bits files of the creep, nl_transient, visco and temperature paths) and baseline hashes unchanged; nine of nine mutations caught, among them MU5 of ADR 0558 (caught by NP1 at every T <= -34 K). A nodal temperature field is not checked up front (a non-positive point value is refused at the point, inside the step error). Case-file key not added (maintainer's call).

No follow-up is proposed (remaining question 1 recommends leaving the field case until a field-driven creep case appears).
