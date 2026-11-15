# ADR 0558: F-581 registration: an Arrhenius temperature factor for power-law creep, read from the point context, on the time-stepping driver with load and displacement histories

Status: **REGISTERED 2026-11-15, written before any code of F-581 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-581 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with one plain python script in the scratch folder
(`w49i-work/f581-scratch/probe.py`, log `probe.log`; the scalar factor only, no rcfd code; disclosed, its numbers quoted
below). The bit record B0 of the paths F-581 touches is a test file written and run on the unchanged base 15f1fee9 after this
commit and before any library code (D5). Results go in the addenda at the end; nothing above the addenda is changed after a
run.

Roadmap item F-581 (deps F-411, F-410), follow-up 2 of [ADR 0489](0489-f411-registration-power-law-creep-time-hardening.md).
Number 0558 given by the lead. Branch `f581-work` from 15f1fee9, worktree `w49i-work/f581`, scratch `w49i-work/f581-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; no existing test edited; clean-room. `docs/ROADMAP.md` and `docs/roadmap.json` are not edited; the
proposed entries are at the end of the addendum.

## What F-581 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. Material::Creep takes an optional Arrhenius factor exp(-Q/(R T)) with the temperature of the point context; at a uniform temperature the uniaxial creep strain equals the closed form with A exp(-Q/(R T)) within 1e-8. solve_in_time takes load and prescribed-displacement histories against time (linear between points) and a uniform temperature; with constant histories it gives the bits of ADR 0489. The relaxation and creep checks of ADR 0485 (R1, C1, C2) run on it with their limits. Materials without the factor keep their bits.

Roadmap note: "Changes the data model: maintainer's call." (open question 1 below).

Clauses: (a0) every limit registered here; (a1) `Material::Creep` takes an optional factor `exp(-Q/(R T))` with the temperature of
the point context, and at a uniform temperature the uniaxial creep strain equals `A exp(-Q/(R T)) sigma^n t` (the closed form of
ADR 0489 with `A` replaced) within `1e-8`; (a2) the time-stepping driver takes load and prescribed-displacement histories against
time (piecewise linear) and a uniform temperature, and with constant histories gives the bits of ADR 0489; (a3) R1, C1, C2 of
ADR 0485 run on it with their limits (`1e-10`; `1e-3` at 20 steps per `tau`; order `>= 1.9`); (a4) materials without the factor
keep their bits.

Readings, registered:

- **(a2) is already in the repository.** ADR 0499 (F-576) generalised `nonlinear::solve_in_time` of ADR 0489 in place as
  `nonlinear::solve_in_time_with(model, prescribed, &TimeLoading, times, options)` with `transient::LoadHistory` and
  `MotionHistory` (a fixed pattern times a piecewise linear `LoadTable` factor, taken at the end of every attempt) and
  `TimeSteppingOptions::temperature` (uniform); ADR 0526 added a temperature history and ADR 0534 a nodal field.
  `solve_in_time` is its form without histories. F-581 adds no second driver and no new history type: (a2) is read as the
  property, checked here (CH1), that `solve_in_time_with` with **constant** histories that carry the loads and prescribed values
  of an ADR 0489 model gives the bits of `solve_in_time` on that model, for the creep material (with and without the factor),
  including a step that is halved.
- **(a3)** R1, C1, C2 of ADR 0485 are viscoelastic checks. They already run on the driver by one call each (ADR 0499 R1t, C1t,
  C2t, test `r1t_c1t_c2t_q2_relaxation_and_creep_by_one_call` of `sa_qs_visco.rs`, measured `4.654e-16`, `1.209e-5`, `1.996`).
  Here they are re-run unchanged on the tip of this branch with their limits, and their printed values are recorded (the test
  is not edited). A companion (RA) carries the relaxation check to the creep material with the factor through a motion history
  and a temperature (ADR 0489 R1, its limit `5e-3` at 800 steps).
- **(a1) "the temperature of the point context"**: `PointContext::temperature`, which the driver fills from
  `TimeSteppingOptions::temperature`, from the temperature history at the end time of an attempt, or from the nodal field at the
  point; the factor is evaluated once per point update at that temperature (the temperature held over the step, the
  backward-Euler reading of ADR 0526 D2 for a history). "Uniform temperature" (the clause's check): the option
  `temperature(T)`.
- **(a4)**: B0 (D5), the default-bits files of F-411, F-576, F-592, F-613 and F-580 (`sa_creep_default_bits.rs`,
  `sa_qs_visco_default_bits.rs`, `sa_temp_history_default_bits.rs`, `sa_temp_field_default_bits.rs`,
  `si_fbar_default_bits.rs`), every existing test of `rcfd-fem`, and `baseline_hashes`.

## Audit of refusals before registering

The tests were searched for the refusal keys and packages this change could lift (`mat_creep::FOLLOW_UP`, `"temperature"`,
`Creep` in `sa_creep.rs`, `sa_scaffold.rs`, `sa_qs_visco*.rs`, `sa_temp_*_default_bits.rs`, `si_fbar*.rs`). The refusals of the
creep material that tests assert are: the static load-factor driver (no time), the linear paths, the element-constant
elasticity matrix and an orthotropic base (`sa_creep.rs` N2/N3, `sa_scaffold.rs` lines 3210, 3632). None of them is lifted: the
factor is a new optional field, the constructor `Material::creep` is unchanged and builds a material without it, and every path
that refuses the creep material still refuses it (with or without the factor). No test asserts that a creep material ignores
the temperature as a refusal; the default-bits files that run creep at a temperature (`sa_temp_history_default_bits.rs`,
`sa_temp_field_default_bits.rs`) use materials without the factor, which keep their path (D1). No opt-in route beyond the
optional field is needed.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0489 (whole: the law, D1 to D5, the checks U1, U1e, U2, R1, T1, H1, Q1, its remaining
  question 2 and follow-up 2 which is this item); ADR 0485 (R1, C1, C2, D1 the Arrhenius shift of the Prony times); ADR 0499
  (whole: `solve_in_time_with`, `TimeLoading`, the temperature option, R1t, C1t, C2t, Q2); ADR 0526 (D2, the temperature at the
  end time of an attempt); ADR 0534 (head: the field puts `T_q` into the point context); ADR 0549 (head and readings: the creep
  material on the B-bar Hex8 through the same point update; F-580 added no library code).
- `crates/rcfd-fem/src/mat_creep.rs` (whole), `model.rs` (`Material`, `creep`, `name`, `base`), `material.rs` (`PointContext`,
  `TimeTemperatureShift`, `Material::stress`), `mat_visco.rs` (`GAS_CONSTANT`, `shift_factor`, `point_shift`), `nonlinear.rs`
  (`point_update`, `classify`, `TimeSteppingOptions`, `solve_in_time`, `solve_in_time_with`, its context closure,
  `creep_increment`), `transient.rs` (`LoadTable`), `loads.rs` (`nodal_load_vector`).
- Tests: `sa_creep.rs` (T1, U1/U1e, R1, H1, Q1, the helpers), `sa_qs_visco.rs` (the helpers, R1t/C1t/C2t),
  `sa_qs_visco_default_bits.rs` (the form of a bit record), `si_fbar.rs` (`with_formulation`).

## Arithmetic done before the code (disclosed)

1. **The law with the factor.** Power-law (Norton) creep with an Arrhenius temperature dependence of the rate, `d eps_bar/dt =
   A exp(-Q/(R T)) q^n t^m`: the form stated by the acceptance. Literature cited from memory, NOT re-read: F. H. Norton, The
   Creep of Steel at High Temperatures, McGraw-Hill, 1929 (the power law); the Arrhenius (thermally activated) temperature
   dependence of steady creep, e.g. H. J. Frost and M. F. Ashby, Deformation-Mechanism Maps, Pergamon, 1982, and the creep texts
   cited in ADR 0489 arithmetic 6. Nothing numerical is taken from them: the factor is the acceptance's closed form, `R` the
   constant `mat_visco::GAS_CONSTANT = 8.314462618 J/(mol K)` already in the repository (the 2019 SI value `N_A k_B`, cited from
   memory). With the temperature held over a step (the point context carries one value), the factor `f = exp(-Q/(R T))` is a
   constant of the step and multiplies `A` everywhere in the return of ADR 0489 arithmetic 1: the local equation `q_tr - q - 3
   G (A f) tau q^n = 0`, the increment, the stress and the tangent (`beta = 1/(1 + 3 G (A f) tau n q^(n-1))`). So the return of
   ADR 0489 at `A' = A f` is the return with the factor, exactly, and the closed forms of ADR 0489 hold with `A'`: uniaxial
   constant stress `eps_c = A' sigma^n t^(m+1)/(m+1)`, relaxation `sigma(t) = (sigma0^(1-n) + (n - 1) E A' t)^(1/(1-n))`.
2. **No increment, no temperature needed.** With no time increment the time factor `tau` is 0 and the return is the elastic
   response from the committed state whatever `A'` (ADR 0489 D1: `tau == 0` returns before `A` is read). So the factor (and the
   temperature) is read only when `tau != 0`; a factor material at the instantaneous step or at the default context gives the
   bits of the same material without the factor (P1). With an increment and no temperature the update is refused (key
   `"temperature"`), and the driver refuses such a model before any attempt (D3).
3. **The numbers** (`probe.py`). `Q = 2.0e5 J/mol`: at `T1 = 700 K` `Q/(R T1) = 34.3635`, `f1 = 1.19154e-15`; at `T2 = 750 K`
   `Q/(R T2) = 32.0726`, `f2 = 1.17770e-14`; `f2/f1 = 9.8838`: a factor lost or taken at the wrong temperature is an order-one
   change. The pre-exponential `A = A'/f1` is chosen in the tests so that `A'` is the `A` of the ADR 0489 set-up being repeated,
   so the measured errors of ADR 0489 are the estimates: U1 `5.8e-13`, U1e `4.5e-10`, U2 `2.0e-13`, T1 `1.3e-9`, R1 `1.717e-3`
   (probe of ADR 0489 at 800 steps). The product `A f1` rounds to `A'` within an ulp or two: a relative change `~2e-16`.
   Replacing `R` by `8.314` changes `f1` by `-1.91e-3` relative (mutation MU3).
4. **A temperature step** (A3). One Hex8 under the dead traction of U1e, `m = 0`, the temperature history `(0, T1), (5, T1), (5 +
   1e-9, T2), (11, T2)`, 100 steps of `0.1 s` (`50 x 0.1 = 5.0` exactly, so step 50 ends at `T1` and step 51 at `T2`). Closed form
   of the temperature step: `eps_c(t) = A sigma^n (f1 min(t, 5) + f2 max(t - 5, 0))`; the ramp of `1e-9 s` changes it by at most
   `1e-9 (f2 - f1)/(5 (f1 + f2)) ~ 1.6e-10` relative at `t = 10 s` (the scheme takes `f2` over the step that contains the ramp,
   ADR 0526 D2; the exact integral over the ramp differs by less than `1e-9 f2`). Estimate `<= 1.6e-10 + 4.5e-10`; limit
   `1e-8` (the acceptance's figure for the uniform case). For comparison: a ramp over a whole step `[5.0, 5.1]` would give a
   scheme-against-physics difference of `1.10e-2` (`probe.log`), which is why the step is sharp.
5. **Constant histories** (CH1). ADR 0499 arithmetic 2: an attempt of `solve_in_time_with` forms the load `f_dead + sum_j
   pattern_j s_j(t1)` and the prescribed values `value s_j(t1)`; with the model's dead loads moved into a history whose pattern is
   their nodal vector (`loads::nodal_load_vector(model, &[])` of the model with them, so the same floating-point vector) and a
   model without them, `f_dead = 0` and `0.0 + p * 1.0 = p` bit for bit; a prescribed value moved into a motion gives `v * 1.0 =
   v`. The rest of the attempt is the same operations. Expected: bits equal for every field of every `TimeStep`.

## Design

### D1. The data model and the material (`model.rs`, `mat_creep.rs`)

`Material::Creep` (non-exhaustive variant) gains `activation: Option<f64>`: the activation energy `Q`, J/mol, of the factor
`exp(-Q/(R T))` multiplying `a`; `None`: no factor (the material of ADR 0489). `Material::creep(base, a, n, m)` is unchanged and
gives `None`. New constructor `Material::creep_arrhenius(base, a, n, m, activation)`: the checks of `creep`, then `activation`
positive and finite (key `"activation"`, `positive_material`). In `mat_creep::update` with `activation: None` every line runs as
before (the borrowed `a`; no multiplication). With `Some(Q)`: the time factor `tau` first (as before); if `tau != 0`, `a' = a *
arrhenius_factor(Q, T)` with `T` the point context's temperature, else `a' = a` (arithmetic 2). New `pub fn
arrhenius_factor(activation, temperature) -> Result<f64>`: `(-(activation / (GAS_CONSTANT * temperature))).exp()`; refused (key
`"temperature"`): a temperature missing (at `update`), not finite or not positive, a factor that underflows to 0. `R` is
`mat_visco::GAS_CONSTANT` (one constant in the crate). The return `return_map` is unchanged.

### D2. Everything else in the material is unchanged

`Material::stress`, `point_update` and the B-bar point update call `mat_creep::update` and so take the factor at their point
context with no further change; the refusals of ADR 0489 D2/D3 (static driver, linear paths, element-constant matrix,
orthotropic base) are unchanged for the factor material. The `Debug` text of a creep material gains the field (`activation:
None`): disclosed; no golden, hash or test reads it (searched: `rcfd-api`, `rcfd-cli` do not name the creep variant).

### D3. The driver refuses a factor material without a temperature before any attempt (`nonlinear.rs`)

In `solve_in_time_with`, after `options.check()`: if a material of the model is a creep material with `activation: Some(_)` and
none of `temperature`, `temperature_history`, `temperature_field` is given, `FemError::InvalidValue { key: "temperature", .. }`
naming the material. Otherwise the failed attempt would be halved five times on a missing input and surface as a solver error
(`classify` of ADR 0489 D4). Models without the factor never enter the test (a match on the variant field only).

### D4. The checks (`crates/rcfd-fem/tests/sa_creep_arrhenius.rs`, no other library code)

Helpers copied from `sa_creep.rs` (not shared; that file is not edited): the `Checks` printer, `uniaxial_point` and the one-Hex8
cube under a dead traction, the stretched cube, the relaxation closed form, the flat history. `E = 2e9`, `nu = 0.3`, `rho =
1200`, `Q = 2e5`, `T1 = 700 K`, `T2 = 750 K`; the test's own `R = 8.314_462_618` and `f(T) = (-(Q / (R T))).exp()`.
Every aggregated error required finite (a NaN is a miss).

### D5. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_creep_arrhenius_default_bits.rs`, written after this commit and run on the base 15f1fee9 before any
library code; digests (FNV-1a 64 over the bits) written into the file then. API of the base only (`Material::creep`). Content:
`mat_creep::update` of a creep material (`n = 3`, `m = 0` and `n = 4`, `m = -0.5`) at a random strain and committed state with
and without a temperature in the context (stress, tangent, state); `solve_in_time` of the dead-traction creep cube (20 steps) with
`temperature(700)`; `solve_in_time_with` of the stretched creep cube under a temperature history `700 -> 750 K` (20 steps); the
halving creep cube of ADR 0489 H1 with `temperature(700)`; `solve_in_time_with` of the creep cube with a load history; the
messages of the static-driver and orthotropic-base refusals of the creep material.

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| A1 | a1 | material point, uniaxial stress control (U1 of ADR 0489), `n = 3`, `m = 0`, `sigma = 2e6 Pa`, `A' = 1e-25` (`A = A'/f1`), context `with_time(t, 0.1)` and `T1`, 100 steps of `0.1 s`: `max |eps_c,xx - A f1 sigma^n t| / (A f1 sigma^n t)` over the step ends | `<= 1e-8` | U1 `5.8e-13` |
| A1e | a1 | one Hex8 under the dead traction `0.2 Pa` by `solve_in_time` with `temperature(T1)`, `A' = 1.25e-9` (U1e), 100 steps of `0.1 s`: the same measure at every point | `<= 1e-8` | U1e `4.5e-10` |
| A2 | a1 companion | A1 with `m = -0.5`, against `A f1 sigma^n t^(1/2)/(1/2)` | `<= 1e-3` | U2 `2.0e-13` |
| A3 | a1 companion | arithmetic 4: one Hex8, dead traction, temperature step `T1 -> T2` at `5 s` by the temperature history, 100 steps: `max |eps_c,xx - A sigma^n (f1 min(t,5) + f2 max(t-5,0))| / (closed form)` over step ends and points | `<= 1e-8` | `<= 6.1e-10` |
| NA4 | a1 negative control | A1e at `T2` against A1e at `T1` (same `A`): the ratio of the final creep strains against `f2/f1 = 9.88` | relative `<= 1e-8` | arithmetic 1; a lost factor gives `1` |
| TA | a1 companion | the consistent tangent with the factor against central differences (T1 of ADR 0489, cases `n = 3, m = 0` and `n = 4, m = -0.5`, `A' = 1e-26`, `ctx(2, 0.5)` at `T1`, step `1e-10`): `max |C - C_fd| / max |C|`; set-up creeping | `<= 1e-6` | T1 `1.3e-9` |
| RA | a3 companion | ADR 0489 R1 with the factor: the creep cube, `n = 3`, `A' = 1/((n-1) E (E eps0)^(n-1))`, the strain `eps0 = 1e-3` held by a motion history (factor 1 on `[0, 11] s`), `temperature(T1)`, 800 steps to `10 s`: `max |S_xx - sigma(t)| / sigma(t)` over the step ends, `sigma0 = S_xx(0)` | `<= 5e-3` | `1.717e-3` |
| CH1 | a2 | constant histories against `solve_in_time` (arithmetic 5), every field of every `TimeStep` (time, `u`, internal force, states, halvings, sub-steps, creep increment, iterations): (i) the U1e cube, traction as a load history, 20 steps; (ii) the stretched cube of ADR 0489 R1 (`n = 3`), the held `u_x` as a motion history, 20 steps of `0.0125 s`; (iii) the halving set-up of ADR 0489 H1 (limit `1e-4`, one step to `1 s`, two halvings) with the motion history; (iv) (i) with the factor material and `temperature(T1)` | bits equal (and (iii) halved twice) | arithmetic 5 |
| R1t | a3 | ADR 0499 test `r1t_c1t_c2t_q2_relaxation_and_creep_by_one_call` re-run on the tip: R1 by one call | `<= 1e-10` | `4.654e-16` |
| C1t | a3 | the same test: C1 at 20 steps per `tau` | `<= 1e-3` | `1.209e-5` |
| C2t | a3 | the same test: observed order | `>= 1.9` | `1.996` |
| P1 | a4 / design | the factor material with no time increment (`ctx(0, 0)`, no temperature) and at the default context gives the bits (stress, tangent, state) of the material without the factor | bits equal | arithmetic 2 |
| NA1 | negative control | the factor material: (i) `update` with an increment and no temperature, key `"temperature"`; (ii) `solve_in_time` of the A1e cube without a temperature, `InvalidValue` key `"temperature"` (D3, not a solver error) | refused | D1, D3 |
| NA2 | negative control | `update` with an increment at `T = 0` and `T = -1 K`; `arrhenius_factor(Q, 1e-3)` (underflow): key `"temperature"` | refused | D1 |
| NA3 | negative control | `Material::creep_arrhenius` with activation `0`, `-1`, `NaN`, `inf`: key `"activation"` | refused | D1 |
| N1 | a4 | B0: every digest of the base 15f1fee9 | equal | D5 |
| N5 | a4 | `cargo test -p rcfd-fem --release` (every test binary, among them the default-bits files of (a4), `sa_creep.rs`, `sa_qs_visco.rs`, `si_fbar.rs`, `sa_scaffold.rs`) | pass | D2 |
| N4 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no case path changed |

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the exponent's sign flipped (`exp(+Q/(R T))`) | A1, A1e, NA4 |
| MU2 | the factor dropped (`a' = a`) | A1, A1e, A3, NA4 |
| MU3 | `R` replaced by `8.314` in `arrhenius_factor` | A1 (`1.9e-3`) |
| MU4 | a missing temperature taken as factor 1 in `update` and the driver's check D3 removed | NA1 |
| MU5 | the check `T > 0` removed from `arrhenius_factor` | NA2 |
| MU6 | the driver's temperature history taken at the start of the attempt (`t - h`) instead of its end (ADR 0526 line, existing) | A3 |
| MU7 | the load histories ignored at the instantaneous step (existing ADR 0499 line) | CH1 |
| MU8 | the driver's check D3 removed (the material refusal kept) | NA1 (ii) |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, B0, the
  default-bits files and the baseline hashes unchanged.
- **partial**: B0, the default-bits files and the baseline hashes unchanged, and A1, A1e, CH1, R1t, C1t, C2t met, with a miss of
  any other check diagnosed and recorded as a `*_known_miss` test, or a mutation not caught.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The factor is `exp(-Q/(R T))` with one activation energy; no threshold, no stress-dependent activation, no `sinh` law, no
  separate primary and secondary laws. Isotropic base only, `n >= 1` (as ADR 0489).
- The temperature is held over a step (the point context's one value); a temperature varying inside a step is taken at the
  attempt's end (ADR 0526 D2). No thermal strain except through the nodal field of ADR 0534 (not checked here with the factor).
- The factor is read wherever a point context carries a temperature (the nonlinear transient integrator with its temperature
  options, the nodal field, the B-bar Hex8); only the quasi-static driver and the material point are checked. The nonlinear
  transient integrator does not pre-check a factor material without a temperature (it fails at its first creeping step).
- No case-file key; no material data (every constant is an input of a check).

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The data model** (the roadmap note: maintainer's call). An optional field `activation: Option<f64>` in the non-exhaustive
   `Material::Creep` and a second constructor, taken (ADR 0489 remaining question 2, option b). It changes no default result, no
   golden and no hash table (the field is `None` for every existing material, D1); it changes the `Debug` text of a creep
   material (D2). Alternative not taken: a separate `TimeTemperatureShift`-like enum for creep (more general, not asked for).
2. **Reading the temperature only when a step creeps** (arithmetic 2), taken: the factor multiplies a zero time factor
   otherwise. Alternative: refuse a factor material at every context without a temperature (as the shifted viscoelastic
   material at `Integrator::start`).
3. **Case-file key.** Not added. Recommendation for the lead: `creep = { base, a, n, m, activation }` with the quasi-static time
   study of ADR 0499 remaining question 1.
