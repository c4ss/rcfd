# ADR 0526: F-592 registration: a temperature history on the quasi-static driver and the nonlinear transient integrator, with the Rayleigh stiffness tangent at the temperature

Status: **REGISTERED 2026-11-14, written before any code of F-592 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-592 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with one plain python script in the scratch folder
(`w49i-work/f592-scratch/probe.py`, log `probe.log`; the scalar Prony law and the WLF shift only, no rcfd code; disclosed, its
numbers quoted below). The bit record B0 of the paths F-592 touches is a test file written and run on the unchanged base
f168d994 after this commit and before any library code (D7). Results go in the addenda at the end; nothing above the addenda
is changed after a run.

Roadmap item F-592 (deps F-576), the follow-up of [ADR 0499](0499-f576-registration-quasi-static-viscoelastic-time-stepping-and-a-temperature-in-the-nonlinear-transient-integrator.md)
(its remaining questions 3 and 4). Number 0526 given by the lead. Branch `f592-work` from local main f168d994 (which contains
F-585, ADR 0522, and F-607, ADR 0523), worktree `w49i-work/f592`, scratch `w49i-work/f592-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; no existing test edited; clean-room. The brief asks for the F-592 entry of `docs/ROADMAP.md` and
`docs/roadmap.json` to be updated by this branch in a separate last commit (the svgs not regenerated); the proposed note is
also given under "For the lead: roadmap entries". Areas of other agents not touched: the contact press-fit tests (F-604), the
Hex8 formulations on the linear paths (F-588), the rotating sector split modes (F-584).

## What F-592 asks

The acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. The quasi-static driver and the nonlinear transient integrator take a uniform temperature against time (a piecewise linear table, the step-end value) and the Rayleigh stiffness tangent at that temperature. A relaxation under a temperature step equals the closed form with the reduced time of the two temperatures within 1e-10; a constant table gives the bits of ADR 0499. Models without a temperature keep their bits.

Roadmap note: the Rayleigh stiffness tangent at the temperature is part of the item (it is still refused for a shifted material
at the base). The brief adds: (b1) define how the reduced time is integrated across a temperature step within an increment
(the step-end value, as stated) and register the closed form; (b2) defaults bit-identical, a default-bits test first, before
library code, `baseline_hashes` unchanged; (b3) no case-file key (an open question with a recommendation); (b4) mutations the
checks catch (for example the step-start temperature, the shift not applied to the Rayleigh tangent); (b5) every aggregated
test error required finite.

Clauses: (a0) every limit registered here; (a1) the quasi-static driver (`nonlinear::solve_in_time_with`) takes a uniform
temperature against time, a piecewise linear table, evaluated at the end of every attempt; (a2) the nonlinear transient
integrator (`nl_transient::Integrator`) the same, at the end of every sub-step; (a3) the Rayleigh stiffness part of the
integrator's damping takes its tangent at the temperature (a shifted viscoelastic material with stiffness damping is no longer
refused when a temperature is given); (a4) a relaxation under a temperature step equals the closed form with the reduced time of
the two temperatures within `1e-10`; (a5) a constant table gives the bits of ADR 0499 (the run with the constant
temperature option of ADR 0499 D2, D3); (a6) models without a temperature keep their bits.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0499 (whole, with its addendum, remaining questions 3 and 4 and the follow-up that is this
  item); through it ADR 0485 (D1 the Prony update over a reduced step `dxi = dt/a_T`, S1b) and ADR 0489 (D5, the driver).
- `crates/rcfd-fem/src/mat_visco.rs` (`shift_factor`, `point_shift`, `update`), `material.rs` (`PointContext`,
  `Material::stress`, the tabulated material and its temperature), `transient.rs` (`LoadTable`: points strictly increasing in
  time, `value(t) = v_i + s (v_j - v_i)` between points, the point value at a point), `nonlinear.rs` (`internal_force_at`,
  `tangent`, `force_and_tangent`, `solve_from_at`, `TimeSteppingOptions`, `solve_in_time_with`, `Histories`), `nl_transient.rs`
  (whole: `NlTransientOptions`, `Integrator::new` with the damping `C = alpha_R M + beta_R K_0` formed once from
  `nonlinear::tangent` at `u = 0` and the default context, `start`, `advance`, `substep`, `strain_energy`, `restore`),
  `nl_transient/emc.rs` (`substep`: it reads `it.temperature` and `it.c`), `mat_ortho.rs` (a tabulated material needs the
  model's temperature load, which the integrator and the driver refuse).
- Tests: `sa_qs_visco.rs` (the cube, `pull_history`, `committed_sxx`, `exx`, `tight`, S1d, the rod, `rod_modes`, `vibrate`,
  `relative`, V1T, NT1, NT2, NT3, B2, RF2), `sa_qs_visco_default_bits.rs` (the form of a bit record).

## Arithmetic done before the code (disclosed)

1. **Which materials read the temperature at the damping tangent.** `K_0` is the tangent at `u = 0` with empty states and no time
   increment. The materials the integrator accepts are the isotropic elastic, J2, creep, viscoelastic and (energy-momentum step)
   neo-Hookean materials and shells; a tabulated material needs a temperature load in the model (`mat_ortho.rs`), which the
   integrator refuses (ADR 0479 D6), so it cannot reach the integrator. Of the rest only the shifted viscoelastic material reads
   the temperature, through `a_T` in `dxi = dt/a_T` (ADR 0485 D1); with no time increment `dxi = 0/a_T = 0` whatever `a_T` is,
   `gamma_i = 1` and the tangent is the instantaneous one `D_0`. So for every model that can reach the integrator, `K_0` at any
   temperature has the values of `K_0` at the default context; what the temperature changes is that a shifted material is no
   longer refused (`point_shift` needs a temperature, key `"temperature"`). Consequences: (i) the ADR 0499 runs with a constant
   temperature keep their bits when `K_0` is taken at that temperature (a5); (ii) a mutation that takes `K_0` at another
   temperature (the reference temperature, the step-start temperature, the table's first value) is an equivalent mutation for
   every reachable model and cannot be caught by any run (registered as E1, E2, not counted, D8); (iii) a mutation that takes
   `K_0` without the temperature (the shift "not applied", the context of ADR 0499) is caught: the shifted material is refused.
2. **The reduced time of the step-end rule.** An attempt (driver) or sub-step (integrator) from `t` to `t1` takes the
   temperature `T1 = T(t1)` of the table (D1) and the material relaxes over the reduced length `dxi = (t1 - t)/a_T(T1)`. The
   reduced time at a step end is the sum `xi_n = sum_k (t_k - t_{k-1})/a_T(T(t_k))`: the right-endpoint rule for `xi(t) = int_0^t
   ds/a_T(T(s))`. A temperature that changes inside an increment acts over the whole increment at its end value: a step of the
   temperature anywhere in `(t_{k-1}, t_k]` acts from `t_{k-1}`. The Prony update over a step with a constant strain is exact for
   any `dxi` (`h_i <- e_i h_i`, ADR 0485 D1: the products of `exp(-dxi_k/tau_i)` give `exp(-xi_n/tau_i)`), so under a held strain
   the computed stress is the closed form at `xi_n`, up to rounding.
3. **The temperature step and its closed form** (a4). A table has strictly increasing times, so a step is written as the ramp
   `(0, T_1), (t_s, T_1), (t_s + h_2, T_2), (t_far, T_2)` whose two inner points are step ends of the run: by arithmetic 2 every
   increment ending at or before `t_s` takes `T_1` (the value at `t_s` is the point value `T_1`; between `0` and `t_s` the
   interpolation `T_1 + s (T_1 - T_1) = T_1` exactly), every increment ending after `t_s` takes `T_2` (at `t_s + h_2` the point
   value, beyond it `T_2 + s 0 = T_2`). This is the temperature step at `t_s` ("the reduced time of the two temperatures"):
   `xi(t) = min(t, t_s)/a_1 + max(t - t_s, 0)/a_2`, `a_j = a_T(T_j)`, and the closed form of the relaxation under the held
   uniaxial Green-Lagrange strain `E_xx` of the R1 cube of ADR 0485 (equal shear and bulk weights, the stress uniaxial, ADR 0485
   arithmetic 5): `S_xx(t) = E_xx E_0 (w_inf + sum_i w_i exp(-xi(t)/tau_i))`. Set-up (S2): the cube, three branches `W3 = (0.3,
   0.2, 0.1)`, `TAU3 = (0.01, 0.1, 1) s` at `T_ref = 373 K`, the WLF shift of ADR 0485 S1 (`c1 = 17.44`, `c2 = 51.6 K`), `T_1 =
   383 K` (`a_1 = 1.475132967e-3`), `T_2 = 393 K` (`a_2 = 1.344285828e-5`, probe), the strain step `u_x = 1e-3` at `xmax` from
   `t = 0` (a motion history), 50 steps of `0.02 a_1` (reduced time 1 at `T_1`), then 50 steps of `0.02 a_2` (reduced time 1 more
   at `T_2`). Probe (`probe.py`, the scalar law with this time arithmetic): `max_n |g(xi_n) - g(xi(t_n))| = 2.8e-16`. The
   measure of S1d of ADR 0499 (the committed `S_xx` of all 8 points at the 101 step ends, normalised by `E_0 E_xx`) is then
   rounding, about `5e-16` (S1d measured `4.654e-16`). Limit `1e-10` (acceptance).
4. **The controls of the step.** (i) The step-start temperature (the increment `(t_s, t_s + h_2]` at `T_1`, the mutation MU1):
   probe `7.234e-4`. (ii) The constant `T_1` throughout (NC1): probe `2.293e-2`. Both many decades above `1e-10`.
5. **A temperature ramp** (companion S4, a1). The table `(0, 383), (t_end, 393)` with `t_end` such that `xi(t_end) = 2`
   (probe: `t_end = 1.167445e-4 s`), the R1 cube and strain step, `N` equal steps to `t_end`. The right-endpoint rule is first
   order: probe `max_n |g(xi_n) - g(xi(t_n))| = 3.4876e-3` at `N = 100`, `1.7484e-3` at `N = 200`, observed order `0.9961` (at
   `N = 50`: `6.930e-3`, order `0.9907` from 50 to 100). The exact `xi(t_n)` is computed in the test by composite three-point
   Gauss-Legendre quadrature of `1/a_T(T(s))` (200 panels; the integrand is smooth and varies by a factor 110 over the run; the
   quadrature error is many decades below the first-order error). Limits: `e_100 <= 7e-3` (twice the probe) and the observed
   order `log2(e_100/e_200)` in `[0.9, 1.1]`.
6. **The constant table** (a5). `LoadTable::value` gives the point value at a point and `v_i + s (v_j - v_i) = v_i + s 0 = v_i`
   between two equal points, so a constant table returns the bits of its value at every time; the contexts of every attempt
   are then those of the constant option of ADR 0499, and the damping matrix is formed once at that value (D4). Expected: bits
   equal (S3d on the driver, S3i on the integrator).
7. **The integrator under a temperature step** (a2, a4 carried to dynamics). There is no closed form of a free vibration under
   a temperature step, so the integrator is checked by an exact equivalence that follows from arithmetic 2: run A takes the
   table step `(0, T_1), (400 dt, T_1), (401 dt, T_2), (1600 dt, T_2)` (the inner points written as `400.0 * dt`, `401.0 * dt`,
   the expressions of the integrator's step times `t0 + step as f64 * dt` with `t0 = 0`); run B runs 400 steps with the constant
   temperature `T_1`, saves its state, restores it into a second integrator with the constant `T_2` and runs 400 more. Every
   sub-step of A then has the context of the same sub-step of B (no halving; checked), and the damping matrix of A (formed at
   `T_1`, re-formed at `T_2`) has the values of B's (arithmetic 1). Expected: every `u`, `v`, `a`, internal force and the state
   bytes bit-equal at all 800 step ends (V2T). The rod of ADR 0485 V1 / ADR 0499 V1T (20 Hex8, `nu = 0`, the lateral motion held,
   `dt = (2 pi/omega)/200`, the start `u = 0`, `v_0` of the first mode with a largest strain of about `1e-6`), the times `tau_i =
   TAU3_i/(0.1 omega a_1)` at `T_ref` (the run at `T_1` in the transition, as V1T), with Rayleigh damping `from_ratio(0.02,
   omega, omega)` (mass and stiffness parts, so that the re-formed damping tangent is exercised). Control NC2: run A against the
   constant-`T_1` run for 800 steps: at `T_2` the branches relax far faster than the period (`omega a_2 tau_i = 10 TAU3_i
   a_2/a_1 = 9.1e-4, 9.1e-3, 9.1e-2`; per step `dt/(a_2 tau_i) = 34, 3.4, 0.34`), the storage modulus drops from about `0.60 E_0`
   (ADR 0499 arithmetic 6) to about `0.40 E_0`, a frequency change of about 18 percent over the last two periods: order one.
   Pass: `M_u >= 1e-2`.
8. **The damping tangent at the temperature** (a3). Run A: the shifted rod (times `tau_i` as in 7) at the constant temperature
   `T_1` with the Rayleigh damping of 7; run B: the unshifted rod with the times `a_1 tau_i` (the product in the test) without a
   temperature and the same damping. The damping matrices have the same values (arithmetic 1: both are `alpha M + beta K_0`
   with the instantaneous tangent); the materials differ only in `(h/a_1)/tau_i` against `h/(a_1 tau_i)`, rounding (ADR 0499
   arithmetic 5, V1T measured `0`). Measures `M_u`, `M_S` of ADR 0499 D5 over 800 steps; limit `1e-10` each (RT1). At the base
   run A is refused at `Integrator::new` (key `"temperature"`, the K_0 assembly); with no temperature it stays refused (NT4).

## Design

### D1. The temperature history and its evaluation

A temperature history is a `transient::LoadTable` of the temperature in K against time (the type of the load histories of the
linear and nonlinear transient paths; at least two points, finite, the times strictly increasing). It is uniform: every point of
the model gets the same value. It is evaluated at the end time of each attempt (driver) or sub-step (integrator), halved or not
(the step-end value); the material relaxes over the increment with that temperature (arithmetic 2). A table must cover the times
of the run; a table and the constant temperature of ADR 0499 together are refused (key `"temperature"`, both forms).

### D2. On the quasi-static driver (`nonlinear.rs`)

`TimeSteppingOptions` (non-exhaustive) gains `temperature_history: Option<LoadTable>` (`None` by `new`) and the builder
`temperature_history(LoadTable)`. `check()` refuses a table together with `temperature` (key `"temperature"`).
`solve_in_time_with` refuses a table that does not cover `[0, t_last]` (key `"temperature"`), and the point context of every
attempt (the instantaneous one at `t = 0`, every sub-step to `t1`) takes `with_temperature(table.value(t1))`. With no table the
closure that forms the context does what it did (the bits of ADR 0499 D2, B0).

### D3. On the nonlinear transient integrator (`nl_transient.rs`, `nl_transient/emc.rs`)

`NlTransientOptions` (non-exhaustive) gains `temperature_history: Option<LoadTable>` (`None` by `new`) and the builder
`temperature_history(LoadTable)`; `Integrator::new` refuses it together with `temperature` (key `"temperature"`) and stores it.
`Integrator::temperature_at(t) -> Option<f64>` (private): the table's value at `t` when a table is given, else the constant
`temperature` (`None` when neither). Its readers: `start` (at `t0`), `substep` and `emc::substep` (at the sub-step end `t_b`),
`strain_energy` (at the time of the state). Coverage: `start` refuses a table that does not cover `t0`, and `run_with` a table
that does not cover the run `[t_n, t_n + steps dt]`, before any step (key `"temperature"`, as the load tables are checked). The
saved state does not carry the temperature (an option, as in ADR 0499 D3); a state restored into an integrator with another
temperature or table continues with that one (used by V2T).

### D4. The Rayleigh stiffness tangent at the temperature (`nl_transient.rs`, `nonlinear.rs`)

`nonlinear::tangent_at(model, u, states, threads, context)`: the tangent of `force_and_tangent` at the context; `tangent`
becomes `tangent_at` with the default context (the call it made: the same bits). In `Integrator::new` the stiffness part `K_0`
of the damping is taken at `u = 0`, the empty states and the context with the temperature `T_c`: none (the default context, the
call of the base) without a temperature; the constant `temperature` with it; the table's value at its first time with a table.
`T_c` is stored. Every sub-step (and the start) takes the damping matrix at its temperature `T = temperature_at(t)`: the stored
`C` when `T` has the bits of `T_c` (always without a table or with a constant one), else `alpha M + beta K_0(T)` formed for the
sub-step (only with a table, a stiffness part and a changed temperature). `C` stays `None` without damping. With the mass part
only, `K_0` is not formed at all, as before.

### D5. What does not change

The Prony update and the shift (ADR 0485), the driver's halving, increment limit and error text (ADR 0489, ADR 0499 D1), the
integrator's step, halvings and saved state (ADR 0479), the energy-momentum step's algebra (ADR 0514, ADR 0523): only the
temperature in the point context and the damping matrix of a sub-step are looked up by D3 and D4. No case-file key, no
temperature field, no thermal strain (a temperature load stays refused).

### D6. The checks (`crates/rcfd-fem/tests/sa_temp_history.rs`, no library code)

The cube, the rod, `committed_sxx`, `exx`, `relative` and the measures as `sa_qs_visco.rs` (copied, that file is not edited).
The closed forms of arithmetic 3 and 5; the equivalences of arithmetic 6, 7, 8. Every aggregated value is required finite.

### D7. The bit record of the paths touched (B0)

`crates/rcfd-fem/tests/sa_temp_history_default_bits.rs`, written after this commit and run on the base f168d994 before any
library code; FNV-1a 64 digests written into the file then. Content: `solve_in_time_with` of the shifted cube with the constant
temperature `383 K` and a motion history (20 steps); `nl_transient` (with the state bytes) of the unshifted three-branch rod with
Rayleigh mass and stiffness damping, without a temperature (60 steps) and with the temperature `383 K` (60 steps, the damping
tangent now at the temperature); of the shifted rod at `383 K` without damping (60 steps, the path of ADR 0499 V1T); of a J2 bar
with Rayleigh damping at `383 K` (30 steps); of an elastic bar with the energy-momentum step, Rayleigh damping and the temperature
`383 K` (30 steps); the strain energy of an elastic bar at `383 K`; the message of the refusal of the shifted rod with stiffness
damping and no temperature by `Integrator::new`. Together with the files of ADR 0499, ADR 0479, ADR 0514 and ADR 0523
(`sa_qs_visco_default_bits.rs`, `sa_nl_transient_default_bits.rs`, `sa_emc_default_bits.rs` and the others of the crate) and
`baseline_hashes`, these are the bits of (a5) and (a6).

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| S2 | a1, a4 | the relaxation under the temperature step of arithmetic 3 by one call of the driver with the table: `|S_xx - E_xx E_0 g(xi(t))| / (E_0 E_xx)` over the 101 step ends and 8 points, `S_xx` the committed stress at the context with `T(t_n)` | `<= 1e-10` | probe `2.8e-16`; ADR 0499 S1d `4.65e-16` |
| S2c | set-up of S2 | 808 point values; every step without halving | exact | arithmetic 3 |
| NC1 | negative control of S2 | the same measure with the constant temperature `T_1` (option of ADR 0499) against the step closed form | `>= 1e-3` | probe `2.29e-2` |
| S4 | a1 | the ramp of arithmetic 5, `N = 100`: `max_n |g(xi_n) - g(xi(t_n))|` in the measure of S2 | `<= 7e-3` | probe `3.49e-3` |
| S4o | a1 | the observed order `log2(e_100/e_200)` | in `[0.9, 1.1]` | probe `0.996` |
| S3d | a5 | the driver: the S1d set-up of ADR 0499 (the shifted cube, `T = 383 K`, the strain step, 100 steps of `0.02 a_T`, `tight()`) with `temperature_history(constant 383 on [0, 2 t_last])` against `temperature(383)`: every `u`, state and internal force | bits equal | arithmetic 6 |
| S3i | a5 | the integrator: run A of V1T (the shifted rod at `383 K`, 800 steps) with the constant table against the constant option: `u`, `v`, `a`, internal force at every step end and the state bytes | bits equal | arithmetic 6 |
| S3e | a5 | the energy-momentum step: an elastic Hex8 bar with Rayleigh damping, 30 steps, constant table against the constant option | bits equal | arithmetic 6 |
| V2T | a2, a3, a4 | arithmetic 7: run A (table step) against run B (constant `T_1`, restored into constant `T_2`), 800 steps, every `u`, `v`, `a`, internal force and the state bytes; every step of both without halving | bits equal | arithmetic 2, 7 |
| NC2 | negative control of V2T | run A against the constant-`T_1` run, `M_u` | `>= 1e-2` | arithmetic 7: order one |
| RT1 | a3 | arithmetic 8: `M_u` and `M_S`, the shifted rod with stiffness damping at `T_1` against the unshifted rod with `a_1 tau_i` and the same damping | each `<= 1e-10` | arithmetic 8, V1T `0` |
| NT4 | a3, a6 | the shifted rod with stiffness damping and no temperature: refused at `Integrator::new` with the key `"temperature"` | refused | arithmetic 1 |
| RF3 | a1, a2 | refusals: the driver with a table and a temperature (key `"temperature"`); the driver with a table ending before `t_last` (`"temperature"`); the driver with a table starting after 0 (`"temperature"`); `Integrator::new` with both (`"temperature"`); `start` at a `t0` outside the table (`"temperature"`); `run` beyond the table (`"temperature"`, and the state unchanged: the time of the state equal before and after) | each with its key | D1 to D3 |
| N1 | a5, a6 | B0: every digest of the base f168d994 | equal | D7 |
| N5 | a5, a6 | the default-bits files of the crate (among them `sa_qs_visco_default_bits.rs`, `sa_nl_transient_default_bits.rs`, `sa_emc_default_bits.rs`) and the tests of `sa_qs_visco.rs`, `sa_nl_transient.rs`, `sa_emc.rs` pass unchanged (`cargo test -p rcfd-fem --release`) | pass | D5 |
| N4 | a6 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no output path of a case changed |

Every aggregated error of a check is required finite (a NaN is a miss, b5). A check that would take more than about two minutes
in release is an ignored heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the driver's attempt at the step-start temperature `T(t)` instead of `T(t1)` | S2 (probe `7.2e-4`), S4 |
| MU2 | the integrator's sub-step at the step-start temperature `T(t_a)` | V2T (bits) |
| MU3 | the shift not applied to the Rayleigh tangent: `K_0` at the default context (the base's call) in `Integrator::new` | RT1, V2T (refused, key `"temperature"`) |
| MU4 | the driver's table ignored after `t = 0` (the value at `0` taken at every attempt) | S2 (as NC1, `2.3e-2`), S4 |
| MU5 | the integrator's start without the table's temperature (the default context) | V2T (refused at start, key `"temperature"`) |
| MU6 | the integrator's table ignored in the sub-steps (its first value taken) | V2T (bits), NC2 inverted |
| MU7 | the driver's coverage check of the table removed | RF3 (a panic of `value` or acceptance instead of the refusal) |
| MU8 | the integrator's run-coverage check of the table removed | RF3 |

Equivalent mutations, registered so that their outcome is not read as a defect (arithmetic 1; not counted by the decision
rule, run and recorded):

| Id | Mutation | Why no run can catch it |
|---|---|---|
| E1 | the damping matrix never re-formed (always at `T_c`, the table's first value) | `K_0` has the same values at every temperature for every reachable model |
| E2 | `K_0` at the reference temperature of the shift | the same |

## Decision rule

- **done**: every check of the table met, MU1 to MU8 each caught by at least one check, fmt and clippy clean, B0, the
  default-bits files and the baseline hashes unchanged.
- **partial**: B0, the default-bits files and the baseline hashes unchanged, and S2, S3d, S3i, NT4 met, with a miss of any other
  check diagnosed and recorded as a `*_known_miss` test, or a mutation not caught.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The temperature is uniform: one value for every point; no nodal temperature field, no thermal strain (a temperature load stays
  refused on both paths). It is piecewise linear in time and taken at the end of each increment: the reduced time is first order
  in the step under a ramp (S4) and exact under steps placed on step ends; a step inside an increment acts from the increment's
  start.
- The damping tangent is the instantaneous tangent at `u = 0` at the temperature: for every model the integrator accepts it has
  the values of the tangent without a temperature (arithmetic 1); the temperature dependence of the damping tangent is
  exercised only by the refusal it lifts. A material whose instantaneous tangent depends on the temperature (a tabulated base)
  does not reach the integrator.
- The halved sub-steps of the integrator and the driver read the table at their own end times; no check makes a halving inside a
  temperature ramp.
- One mesh per check; no polymer data (decision A1 of ADR 0310): every constant is an input of a check.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **The form of the history.** A `LoadTable` of the temperature (the type the load histories use) in a separate option
   `temperature_history` beside the constant `temperature` of ADR 0499 (both refused together), taken: the constant option and
   its bits stay as they are. The alternative, one enum option, would change the field of ADR 0499.
2. **The damping matrix with a history.** Re-formed at the sub-step's temperature when its bits change (D4), taken: it is what
   "the tangent at that temperature" says, costs nothing without a table and nothing for a constant table. The alternative, the
   tangent once at the start temperature, is equivalent for every reachable model (E1) but not in principle.
3. **Case-file key.** Not added (b3). Recommendation for the lead: `temperature = [[t, T], ...]` (a table) or a number in the
   quasi-static time study and in the nonlinear transient study proposed by ADR 0499 open question 3, offered when the maintainer
   opens the case file to the nonlinear studies.
