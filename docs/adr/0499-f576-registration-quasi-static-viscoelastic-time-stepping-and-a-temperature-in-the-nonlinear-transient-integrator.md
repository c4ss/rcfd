# ADR 0499: F-576 registration: quasi-static viscoelastic time stepping on the Newton driver, with load and displacement histories, and a uniform temperature in the nonlinear transient integrator

Status: **REGISTERED 2026-11-14, written before any code of F-576 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-576 was written, compiled or run in the repository: the files listed under "What was
read" were read, and the arithmetic below was done by hand and evaluated with two plain python scripts in the scratch folder
(`w49i-work/f576-scratch/probe.py`, log `probe.log`, and `probe_h1.py`, log `probe_h1.log`; no rcfd code; disclosed, their
numbers quoted below). The bit record B0 of the default paths is a test file written and run on the unchanged base 39e7ef84
after this commit and before any library code (D6). Results go in the addenda at the end; nothing above the addenda is changed
after a run.

Roadmap item F-576 (deps F-410), follow-up 1 of [ADR 0485](0485-f410-registration-linear-viscoelasticity-prony-series.md).
Number 0499 given by the lead. Branch `f576-work` from 39e7ef84 (which contains F-583, ADR 0494), worktree `w49i-work/f576`,
scratch `w49i-work/f576-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; defaults bit-identical; no
edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new dependency, no unsafe code, the
crate graph unchanged; no existing test edited; clean-room. The brief asks for the roadmap entry in `docs/ROADMAP.md` and
`docs/roadmap.json` to be updated by this branch in a separate last commit (the svgs not regenerated); the proposed note is also
given under "For the lead: roadmap entries".

## What F-576 asks

The acceptance of the roadmap entry, binding:

> Every limit not stated here is registered in the ADR before the run. A time-stepping driver over nonlinear::solve_from_at with the halvings of ADR 0404 takes prescribed displacements and loads against time. It reproduces the relaxation and creep checks of ADR 0485 (R1, C1, C2) with the same limits, without a loop in the caller. A uniform temperature given to the nonlinear transient integrator reproduces S1b of ADR 0485 in a free vibration run against the run at the reference temperature with all relaxation times scaled by a_T, within 1e-10 relative. Models without the viscoelastic material keep their bits.

The brief adds: (b1) read ADR 0485, ADR 0404 and ADR 0489 (F-411), which may already have a driver to reuse or generalise, and
say what is reused; (b2) no case-file key (proposed in the open questions); (b3) a default-bits test at the base before any
library change, `baseline_hashes` unchanged; (b4) every aggregated error required finite (an `f64::max` over NaN hides a
failure); probe before tight limits; (b5) keep the edits of `thermal.rs`, `geometric.rs`, `damped.rs`, `harmonic_direct.rs`,
`estimate.rs`, `mesh.rs`, `assemble.rs`, `recover.rs` minimal (another branch edits them; this item edits none of them).

Clauses: (a0) every limit registered here; (a1) a time-stepping driver over `solve_from_at` with the halvings of ADR 0404 takes
prescribed displacements and loads against time; (a2) it reproduces R1, C1, C2 of ADR 0485 with their limits (`1e-10`; `1e-3`
at 20 steps per `tau`; observed order `>= 1.9`) without a loop in the caller; (a3) a uniform temperature given to the
nonlinear transient integrator: the free vibration run at `T` against the run at the reference temperature with every
relaxation time scaled by `a_T`, within `1e-10` relative (the S1b statement of ADR 0485 carried to a free vibration); (a4)
models without the viscoelastic material keep their bits.

Readings, registered:

- **(a1) "the halvings of ADR 0404"**: a failed attempt (Newton not converged, an inverted point) is discarded and its length
  halved, kept to the end of the step, with a located error after the last halving (ADR 0404 D5). The driver of ADR 0489 D5
  (`nonlinear::solve_in_time`) already does this, with constant loads and prescribed values; it is generalised (D1), not
  duplicated. "Against time": each history is a fixed pattern times a piecewise linear factor of time
  (`transient::LoadTable`), evaluated at the end time of each sub-step (also of a halved one).
- **(a2) "without a loop in the caller"**: R1, C1, C2 are each one call of the driver; the caller gives the histories and the
  step-end times. The R1 strain step is a displacement history (constant factor from `t = 0`), the C1/C2 dead load a load
  history (constant factor from `t = 0`), so that the clause exercises the new inputs; the measures and limits are those of
  ADR 0485 (R1 at every step end and point in the driver's own measures; C1 on `u_x(x = 1)`).
- **(a3)** run A: the rod of ADR 0485 V1 (20 Hex8, `nu = 0`, the lateral motion held) with the three-branch material of
  ADR 0485 H1 carrying the WLF shift of ADR 0485 S1 (`c1 = 17.44`, `c2 = 51.6 K`, `T_ref = 373 K`), the relaxation times at
  the reference `tau_i`, given the uniform temperature `T = 383 K` (`a_T = 1.475133e-3`). Run B: the same rod with the same
  shift and the times `a_T tau_i`, given `T_ref`. "Within `1e-10` relative": the displacement and the committed stress of
  every point at every step end (D5, check V1T). The times `tau_i` are chosen so that the run at `T` is in the transition
  (`a_T tau_2 omega = 1`, as H1 of ADR 0485), where a lost or doubled shift is an order-one change (negative control NT1).
- **(a4)**: B0 (D6), the default-bits files of F-410, F-411 and F-423 (`sa_visco_default_bits.rs`,
  `sa_creep_default_bits.rs`, `sa_nl_transient_default_bits.rs`), and `baseline_hashes`; and B2: an elastic model run by the
  nonlinear transient integrator with a temperature gives the bits of the run without one.

## What was read (no run)

- `AGENT_RULES.md`; the brief; ADR 0485 (whole, its amendment 1 and addendum: S1b, its remaining question 4 "a uniform
  temperature in `NlTransientOptions`", its follow-up 1 which is this item); ADR 0489 (D5 the driver `solve_in_time`, D8 the
  halving checks H1, H2, Q1 the driver against the loop, its remaining questions); ADR 0404 (D5 the driver and its halvings).
- `crates/rcfd-fem/src/nonlinear.rs` (the module doc, `evaluate`, `internal_force`, `force_and_tangent`, `NewtonOptions`,
  `Problem::attempt_with` with its residual reference, `solve_with`, `solve_from`, `solve_from_at`, `TimeSteppingOptions`,
  `TimeStep`, `TimeHistory`, `solve_in_time`), `nl_transient.rs` (whole), `transient.rs` (`LoadTable`, `LoadHistory`,
  `MotionHistory`), `material.rs` (`PointContext`), `mat_visco.rs` (`shift_factor`, `point_shift`, `update`),
  `constraint.rs` (`Prescribed`, `prescribed_equations`).
- Tests: `sa_visco.rs` (the cube, `cube_history`, `stresses`, R1, C1/C2, S1b, the rod V1, H1), `sa_creep.rs` (Q1),
  `sa_creep_default_bits.rs` (the form of a bit record).

## Arithmetic done before the code (disclosed)

1. **What is reused** (b1). The time stepping of ADR 0489 D5 (`solve_in_time`: the instantaneous step at `with_time(0, 0)`,
   sub-steps of one attempt of `solve_from_at` at `with_time(t1, t1 - t)`, the creep-increment limit, halving kept to the end
   of the step, the located creep error) is kept line for line; the histories enter only where the load vector and the
   prescribed values of an attempt are formed. With no history and no temperature the driver makes the operations it made
   (B0, and the checks of ADR 0489 in `sa_creep.rs`, which are not edited). The Prony update, the shift and the point context
   are those of ADR 0485 D1 to D5, unchanged; the integrator's step, its halvings and its saved state those of ADR 0479,
   unchanged.
2. **R1, C1, C2 through the driver** (a2). Each sub-step of the driver is an attempt of `solve_from_at` from the committed
   displacement and states with the prescribed values at the sub-step end written into the start, which is exactly what the
   loops of ADR 0485 did (`cube_history`, `creep_element`), except that the driver forms the increment as `t_k - t_{k-1}`
   (ADR 0489 D5) where those loops used `dt`; a constant factor gives `value * 1.0 = value` and a load `0.0 + p * 1.0 = p`
   bit for bit. So the measured values of ADR 0485 (R1 `4.65e-16`, C1 `1.209e-5`, order `1.996`) are expected to within
   rounding; check Q2 compares the driver with a loop written in the test with the driver's time arithmetic, bit for bit.
3. **Histories that change in time** (companions). (i) R3: the strain ramp `u_x(1, t) = r t`, `r = 1e-3 /s`, on the R1 cube
   (three branches), 100 steps of `0.01 s` to `1 s`. With equal weights the stress is uniaxial (ADR 0485 arithmetic 5) and
   `S_xx(t) = E_0 int_0^t g(t - s) dE_xx/ds ds`, `g(u) = w_inf + sum w_i exp(-u/tau_i)`, `E_xx = a + a^2/2`, `a = r t`, so
   `S_xx = E_0 (r G_1(t) + r^2 G_2(t))`, `G_1 = w_inf t + sum w_i tau_i (1 - e^{-t/tau_i})`, `G_2 = w_inf t^2/2 + sum w_i
   tau_i (t - tau_i (1 - e^{-t/tau_i}))`. The update is exact for a strain linear in the step; `E_xx` is quadratic in the step,
   its chord deviates by at most `r^2 dt^2/8 = 1.25e-11`, and the stress error is at most that times `int |g'| E_0 <= 0.6
   E_0`: `7.5e-12 E_0`, against `E_0 E_xx(1 s) = 1.0005e-3 E_0`, i.e. `7.5e-9`. Probe (`probe.py`, the scalar law): `4.650e-9`
   at 100 steps (`1.815e-8` at 50, `1.170e-9` at 200: second order). (ii) C3: the standard solid of C1 under a load ramp
   `sigma(t) = s' t` to `5 tau_c` (`s' = 1e-8 E_inf/(5 tau_c)`): `eps(t) = s' (t/E_inf - (1/E_inf - 1/E_0) tau_c (1 -
   e^{-t/tau_c}))`. Probe: `5.017e-5` at 20 steps per `tau`, `1.278e-5` at 40, order `1.9731`.
4. **The halving with a load history** (H1, H2). One Hex8 unit cube, `nu = 0`, the lateral motion held at every node, `u_x =
   0` on `x = 0`: the homogeneous stretch `u_x = a x` is exact for the element, `E_xx = a + a^2/2`, the face force `(1 + a)
   S_xx`. The three-branch material of R1 with `nu = 0`; over a step the update is linear, `S = c_n + E_t E`, `E_t = E_0
   (w_inf + sum w_i gamma_i)`. Dead load history on the four `x = 1` nodes, `P(t) = 0.01 E_0 t` (factor `(0, 0), (1, 1)`),
   times `[1 s]`; Newton options: one correction (`max_iterations = 1`), the residual criterion alone (`energy_tolerance =
   1`, ADR 0485 arithmetic 6), `residual_tolerance = 1.5e-2`. The driver's relative residual after one correction is `|R_F| /
   max(|f_F|, |f_int|)` with `|R_F| = |dP|/2`, `|f_F| = P/2` and `|f_int| = sqrt(8) (1 + a) S/4` (the `x = 0` reactions opposite,
   no lateral force). Probe (`probe_h1.py`): the full step `[0, 1] s` gives `2.130e-2` (fails: above `1.5e-2`), the halves
   `[0, 0.5]` and `[0.5, 1] s` give `1.000e-2` and `6.318e-3` (pass): one halving, two sub-steps; margins `1.42` and `1.50` (the
   tolerance near the geometric mean `1.4596e-2`). The halved run must then equal the driver at the times `[0.5, 1]` bit for bit
   (the sub-step `t1 = 0 + 0.5`, then `t1 = 1` by the end rule; the increments `0.5` both ways; the load factor `value(0.5)`
   and `value(1)` both ways).
5. **The temperature in the nonlinear transient step** (a3). The integrator gives every attempt `PointContext::new()
   .with_time(t_b, h)` (ADR 0485 D5); with a temperature `T` the context also carries `T`. The shifted material then takes
   `dxi = h/a_T` and `x_i = dxi/tau_i` (ADR 0485 D1): run A `x_i = (h/a_T)/tau_i`, run B (`a_T(T_ref) = 10^(-0) = 1` exactly,
   `h/1 = h`) `x_i = h/(a_T tau_i)` with `a_T tau_i` rounded: the two differ by a few units in the last place of `x_i` per step,
   and nothing else in the step reads the temperature or the times (the mass, the inertia contribution and the geometric terms
   are the same operations). The rounding difference of `e_i`, `gamma_i` (`1e-16` relative) enters the internal stresses and
   is carried by a damped oscillation; over 800 steps a linear growth gives at most `800 x 1e-16 x` a modest amplification,
   about `1e-13`. Both runs take the same Newton iterations (quadratic convergence at a strain of `1e-6`, ADR 0485 V1: the
   energy ratio `~1e-12` at the second correction, far from the threshold `1e-10`), so the tolerance does not enter. Limit
   `1e-10` (acceptance), three decades above the estimate. The start (`Integrator::start`) evaluates the internal force at `u
   = 0` with empty states; a shifted material refuses that without a temperature (ADR 0485 D1, key `"temperature"`), so the
   start takes the temperature too (D3).
6. **The negative control of (a3)** (NT1). Run A against run C, the material of run A at `T_ref` (times `tau_i`, unscaled): at
   `T_ref` the branches are near their glassy limit (`omega tau_2 = 1/a_T = 678`), at `T` they are in the transition (`omega a_T
   tau_i = 0.1, 1, 10`), `E'(omega)/E_0 = 0.4 + 0.3 x 0.01/1.01 + 0.2 x 0.5 + 0.1 x 100/101 = 0.602` against about `1`: the
   frequencies differ by a factor `0.78`, so over four periods the displacements differ by an order-one amount. Pass: the
   relative difference of the displacements `>= 1e-2`.

## Design

### D1. The histories on the driver (`nonlinear.rs`): `solve_in_time_with`

`pub struct TimeLoading { pub loads: Vec<LoadHistory>, pub motions: Vec<MotionHistory> }` (`Default`, `new()`, builders
`load(LoadHistory)`, `motion(MotionHistory)`; the types of `crate::transient`). `pub fn solve_in_time_with(model, prescribed,
loading: &TimeLoading, times, options) -> Result<TimeHistory>`: the driver of ADR 0489 D5 with, at every attempt (the
instantaneous one at `t = 0` and every sub-step to `t1`, halved or not):

- the load vector `f(t1) = f_dead + sum_j pattern_j s_j(t1)` (the model's dead loads `nodal_load_vector(model, &[])`, then each
  history in list order); with no load history the vector `f_dead` itself (no addition: the bits of ADR 0489);
- the prescribed values: the model's and `prescribed` (constant, as before) and each motion's `Prescribed { node, component,
  value * s_j(t1) }`, also written into the start displacement of the attempt (the start must carry them, ADR 0479 D2); with no
  motion the list `prescribed` itself and the committed displacement as the start.

Refused before any attempt: a load pattern of the wrong length (`FemError::FieldLength`, key `"load pattern"`) or with a non-finite
value (key `"load pattern"`); a table that does not cover `[0, t_last]` (keys `"load history"`, `"motion history"`); a motion of
an equation that is prescribed by the model or by `prescribed`, or by an earlier motion (key `"motion history"`); the other
refusals of `solve_in_time` (key `"times"`, the options). `solve_in_time(model, prescribed, times, options)` becomes
`solve_in_time_with(model, prescribed, &TimeLoading::default(), times, options)`; its documented behaviour is unchanged. The
error of the last halving keeps its solver name `"creep time stepping"` (the check H2 of ADR 0489 reads it; not renamed).

### D2. A temperature on the driver

`TimeSteppingOptions` (non-exhaustive) gains `temperature: Option<f64>` (`None` by `new`), builder `temperature(T)`; refused when
not finite (key `"temperature"`). With `Some(T)` every attempt's context is `with_time(..)` then `with_temperature(T)`; with
`None` the context of ADR 0489. Check S1d.

### D3. A uniform temperature in the nonlinear transient integrator (`nl_transient.rs`)

`NlTransientOptions` (non-exhaustive) gains `temperature: Option<f64>` (`None` by `new`), builder `temperature(T)`; `Integrator::new`
refuses a non-finite value (key `"temperature"`). With `Some(T)`: `substep` gives `PointContext::new().with_time(t_b, h)?
.with_temperature(T)?`; `start` evaluates the internal force at the context with the temperature
(`nonlinear::internal_force_at`, D4); `strain_energy` evaluates at it. With `None` every line runs as before (the default context
and `nonlinear::internal_force`). The temperature enters the materials only through the point context: the viscoelastic shift,
and the base of a tabulated material (`material.rs`), which then takes its value at `T`; no thermal strain (a temperature load
stays refused, ADR 0479 D6). The saved state does not carry the temperature (an option, like the method). The Rayleigh stiffness
part keeps the tangent at the default context (Limits).

### D4. `nonlinear::internal_force_at`

`pub fn internal_force_at(model, u, states, context)`: the body of `internal_force` with the solid element's force from
`evaluate(.., false, context)`; `internal_force` calls it with the default context, which is the call of
`element_internal_force` it made (the same function `evaluate` with the same arguments: the same bits, B0).

### D5. The checks of (a3) (`sa_qs_visco.rs`, no library code)

The rod `rod(material)` and its modes as ADR 0485 (`rod_modes`: `lambda_1`, the mass-normalised `phi_1`, `v_0 = A phi_1` with a
largest strain of about `1e-6`), `omega = sqrt(lambda_1)`, `dt = (2 pi/omega)/200`, 800 steps (four periods), the average
acceleration, the integrator's Newton defaults, start from `u = 0`, `v_0`. Times `tau_i^T = TAU3_i/(0.1 omega)` (so that
`tau_2^T omega = 1`), `a_T = mat_visco::shift_factor(&wlf, 383)`, run A times `tau_i = tau_i^T/a_T` at `T = 383 K`, run B times
`a_T tau_i` (the product computed in the test) at `T = 373 K`. Measures: `M_u = max_n |u_A,n - u_B,n|_inf / max_n |u_B,n|_inf`
and `M_S = max_n max_points |S_xx,A - S_xx,B| / max_n max_points |S_xx,B|` with the committed stress of each point as S1b of ADR
0485 (amendment 1): `Material::stress` at the committed strain (the first six state values) with the committed state and the
context with the run's temperature and no time increment. Every value required finite.

### D6. The bit record of the default (B0)

`crates/rcfd-fem/tests/sa_qs_visco_default_bits.rs`, written after this commit and run on the base 39e7ef84 before any library
code; digests (FNV-1a 64 over the bits) written into the file then. Content: `solve_in_time` of a creep cube under a held strain
(20 steps; every field of every `TimeStep`), of the three-branch viscoelastic cube (20 steps, residual criterion), and of the creep
cube with an increment limit that halves; `nl_transient` of an elastic Hex8 bar under a step load (30 steps, `u`, `v`, `a`, the
internal force, the state bytes and the strain energy), of a J2 bar started by a velocity (30 steps, with bytes) and of the
three-branch viscoelastic rod (60 steps, with bytes); `nonlinear::internal_force` of a J2 Hex8 bar from a non-zero committed
state and of the viscoelastic cube; the message of the refusal of a shifted viscoelastic material by `Integrator::start` (no
temperature).

## Checks (limits fixed now)

| Id | Clause | What | Limit | Estimate behind the limit |
|---|---|---|---|---|
| R1t | a2 | ADR 0485 R1 by one call of `solve_in_time_with`: the cube, three branches, the strain step `1e-3` as a motion history (factor 1 on `[0, 3] s`, covering the last time with room for its rounding), times `0.02 k`, `k = 1 .. 100`, residual criterion (`1e-13`, `tol_e = 1`); `|S_xx - E_xx E(t)| / (E_0 E_xx)` at every step end and point | `<= 1e-10` | ADR 0485: `4.65e-16` (arithmetic 2) |
| C1t | a2 | ADR 0485 C1 by one call: the standard solid, the dead load `sigma0 = 1e-8 E_inf` as a load history (factor 1 on `[0, 11] s`), times `k tau/20` to `5 tau_c`, driver defaults; `max |eps - eps_closed| / eps_closed`, `eps = u_x(x = 1)` | `<= 1e-3` | ADR 0485: `1.209e-5` |
| C2t | a2 | the same at 40 steps per `tau`; `log2(e_20/e_40)` | `>= 1.9` | ADR 0485: `1.996` |
| Q2 | a2 | the driver against a loop of `solve_from_at` written in the test with the driver's time arithmetic (`with_time(0, 0)`, then `with_time(t_k, t_k - t_{k-1})`, the prescribed values written into the start): the first 20 steps of R1t and of C1t, every `u`, state and internal force | bits equal | arithmetic 2 |
| R3 | a1 | strain ramp through a motion history (factor `(0, 0), (2, 2)`, amplitude `1e-3`: `s(t) = t`), the R1 cube, 100 steps of `0.01 s`: `max |S_xx - S_closed| / (E_0 E_xx(1 s))` over the step ends and points, `S_closed` of arithmetic 3 (i) | `<= 1e-7` | bound `7.5e-9`, probe `4.65e-9` |
| C3 | a1 | load ramp through a load history (factor `(0, 0), (20, 2)`, `s' = 1e-8 E_inf/(5 tau_c)`, `tau_c = 2 s`), the C1 cube, 20 steps per `tau` to `5 tau_c`: `max |eps - eps_closed| / eps_closed` (step ends `k >= 1`) | `<= 1e-3` | probe `5.02e-5` |
| C4 | a1 | the same at 40 steps per `tau`, observed order | `>= 1.9` | probe `1.973` |
| H1 | a1 | arithmetic 4: one halving, two sub-steps (`halvings == 1`, `substeps == 2` of the step), and the result (`u`, states, internal force of both entries) bit-equal to the driver at the times `[0.5, 1]` (no halving there) | exact | arithmetic 4, margins `1.42`, `1.50` |
| H2 | a1 | the same with `max_halvings = 0`: refused, `FemError::Solver { solver: "creep time stepping" }`, the reason containing `"step 1 of 1"` and `"not converged"` | refused | arithmetic 4 |
| S1d | companion | ADR 0485 S1b by one call of the driver with the temperature option (D2): the cube with the WLF material at `T = 383 K`, the strain step as a motion history, times `0.02 a_T k`, `k = 1 .. 100`, residual criterion; measure of S1b (committed stress at the context with `T`) | `<= 1e-10` | ADR 0485 S1b: `4.65e-16` |
| V1T | a3 | D5: `M_u` and `M_S` | each `<= 1e-10` | arithmetic 5, `~1e-13` |
| NT1 | negative control (a3) | run A against run C (the material of run A at `T_ref`, times unscaled): relative difference of the displacements as `M_u` | `>= 1e-2` | arithmetic 6: order 1 |
| NT2 | negative control | the shifted rod without a temperature: refused at `start` with the key `"temperature"` (the error or its located source) | refused | ADR 0485 D1 |
| NT3 | negative control | `NlTransientOptions` and `TimeSteppingOptions` with a temperature `NaN`: refused with the key `"temperature"` (`Integrator::new`, `solve_in_time_with`) | refused | D2, D3 |
| B2 | a4 | the elastic rod (`base(0)`), 100 steps of `nl_transient` with `temperature(383)` against the run without: `u`, `v`, `a`, the internal force at every step and the state bytes | bits equal | D3 |
| RF2 | a1 | refusals of D1: a load pattern of the wrong length (`FieldLength`, key `"load pattern"`) and with a NaN (key `"load pattern"`); a load table ending before `t_last` (key `"load history"`); a motion table starting after 0 (key `"motion history"`); a motion of an equation the model prescribes (key `"motion history"`); the same equation in two motions (key `"motion history"`) | each with its key | D1 |
| N1 | a4 | B0: every digest of the base 39e7ef84 | equal | D6 |
| N5 | a4 | the default-bits files of F-410, F-411, F-423 and the tests of `sa_creep.rs` (ADR 0489, among them H1, H2, Q1 on the driver) and `sa_visco.rs` pass unchanged | pass | arithmetic 1 |
| N4 | a4 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no output path of a case changed |

Every aggregated error of a check is required finite (a NaN is a miss, b4). Files: `crates/rcfd-fem/tests/sa_qs_visco.rs` (R1t
to RF2), `sa_qs_visco_default_bits.rs` (N1). A check that would take more than about two minutes in release is an ignored
heavy record run once, with a fast test on the same code path.

## Planned mutations (on a scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the load history evaluated at the start of the sub-step (`t`) instead of its end `t1` | C3, C4, H1 |
| MU2 | the motion history evaluated at the start of the sub-step | R3 |
| MU3 | the histories ignored at the instantaneous step (`t = 0`: the dead loads and constant values only) | R1t, Q2, C1t |
| MU4 | the motion values written into the prescribed list but not into the start displacement | R1t, R3, Q2 (refused by `solve_from_at`) |
| MU5 | the integrator's sub-step context without the temperature (the start keeps it) | V1T (refused at the first step) |
| MU6 | the integrator's temperature replaced by the shift's reference in the sub-step (`a_T = 1`) | V1T (`M_u` order 1, as NT1) |
| MU7 | the driver's temperature dropped (D2) | S1d (refused, key `"temperature"`) |
| MU8 | the load history added at the target of the step instead of the halved sub-step's end | H1 |

## Decision rule

- **done**: every check of the table met, every planned mutation caught by at least one check, fmt and clippy clean, B0, the
  default-bits files and the baseline hashes unchanged.
- **partial**: B0, the default-bits files and the baseline hashes unchanged, and the clauses (a2) (R1t, C1t, C2t) and (a4) met,
  with a miss of any other check diagnosed and recorded as a `*_known_miss` test.
- **todo**: otherwise (in particular any change of the default bits).

## Limits (not covered)

- The histories are piecewise linear factors of time on fixed patterns (the types of the linear transient path); the factor
  is taken at the end of each sub-step. No follower load, no contact, no prescribed velocity.
- The driver records the step ends only (no output between them); its temperature is uniform and constant in time; no
  temperature history, no temperature field, no thermal strain.
- In the nonlinear transient integrator the temperature is uniform and constant; the Rayleigh stiffness part keeps the tangent at
  `u = 0` at the default context, so a shifted viscoelastic material with a stiffness-proportional damping stays refused (key
  `"temperature"`) as before; a temperature load stays refused.
- One mesh per check; no convergence study beyond the probes; no polymer data (decision A1 of ADR 0310): every constant is an
  input of a check.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. **Extend or add a driver** (b1). The driver of ADR 0489 D5 is generalised (D1) and `solve_in_time` kept as its constant-load
   form, taken: one driver, as ADR 0489 open question 1 asked.
2. **Where the temperature lives in the integrator.** A uniform `temperature` in `NlTransientOptions` (ADR 0485 remaining
   question 4, option a), taken; a nodal temperature field is a follow-up.
3. **Case-file key.** Not added (b2). Recommendation for the lead: a study `quasi_static_time = { times = [...], loads = [{ set,
   table }], motions = [{ set, component, value, table }], temperature }` read into `solve_in_time_with`, and a `temperature` key
   in the nonlinear transient study, offered when the maintainer opens the case file to the nonlinear studies.
