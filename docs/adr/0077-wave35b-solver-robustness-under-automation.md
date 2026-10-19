# ADR 0077: Wave 35B, solver robustness under automation (steady divergence guard, the linear-upwind divergence on the FAN-01 mesh, the pressure-lag check in the API, the corrected component diagonal on the heat-exchanger channel)

Status: registered before any code of this wave was written. Everything under "Registered" was committed before the code and the runs it
governs. Limits are not changed, widened or deleted after a run; a miss becomes an ignored `*_known_miss` test with the limit intact, a defective
set-up gets a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded. Clean-room: first principles only; nothing
in this ADR is taken from another CFD code or its documentation. Written at HEAD 0cef8f5.

Parents: ADR 0067 (PIMPLE/PISO divergence guard, design-loop infrastructure), ADR 0073 B4 (`PimpleSettings::pressure_lag_check`), ADR 0074 part C and
`docs/component-diagonal-default.md` (the corrected diagonal and the `lumped` z4 failure), ADR 0075 amendment 3 and addendum (FAN-01: a diverged
SIMPLEC run continued for about 2 h; `linear_upwind` diverges laminar and with SST, `upwind` is stable), `docs/milestone-fan.md` ("solver robustness
under automation").

## Disclosure: what was run before this registration (smoke runs, rule (d) of wave 35)

All smoke runs were made on 0cef8f5 with scratch test files that are not committed (`w35b_smoke.rs`, `w35b_smoke_fv.rs`), release build.

* S1 FAN-01 attempt-2 set-up (ADR 0075 amendment 2: H-slit, 13 332 cells, first cell 2e-4 m, design point, `rotating_frame`, SIMPLEC), 40 iterations,
  about 0.1 s per iteration. Maximum boundary-face speed 40.03 m/s.
  * laminar, `linear_upwind`: max cell speed 262 (iteration 1), 6 954 (2), 5.1e4 (3), 1.1e6 (4), 3.6e6 (5), 4.2e8 (7), 3.7e11 (10), 9.3e21 (20), 2.7e43 (40);
    continuity 1.1e-3, 7.6e-3, 0.11, 0.41, 8.8, ..., 2.0e6 (10), 1.1e38 (40); pressure range 585 (1) to 2.2e83 (40); res_u and res_p between 0.68 and 0.98
    throughout; nothing non-finite within 40 iterations.
  * SST, `linear_upwind` (the 34D attempt-2 run): continuity 2.431 (5), 7.295e4 (10), 7.525e15 (20), 1.271e27 (30): the 34D probe numbers to three or four
    digits; max cell speed 237 (1), 5 771 (2), 1.3e8 (7), 6.2e10 (10).
  * laminar, `upwind`: max cell speed 80 (1), 47 (40); continuity between 8e-5 and 1.2e-3; res_u 1.3e-2 and res_p 7.0e-3 at 40 (stable).
* S2 lid cavity 12 x 12 from the case tree, SIMPLE, `upwind`: alpha_u/alpha_p 0.7/0.3 converges (tolerance 1e-6) in 71 (nu 0.01) and 83 (nu 1e-3) iterations;
  1.0/1.0 diverges: nu 0.01 grows to max speed 1.4e14 at iteration 10 and then stays FINITE at 1e78 to 1e80 for 3000 iterations (continuity about 1e74,
  res_u about 1.0, res_p 0.3 to 0.8: the run ends `not_converged` at its cap without the guard); nu 1e-3 becomes non-finite between iterations 100
  and 500 (velocity and pressure NaN; the reported `continuity` and `res_u` then read 0, only `res_p` is NaN). The same at fv level (12 x 12, nu 1e-5):
  SIMPLE central 1/1 finite 1e100 at 500 iterations; SIMPLEC central alpha_u 1.0, alpha_p 1.5 NaN between 300 and 325; SIMPLE upwind 1/1 finite.
  `SimpleSettings::alpha_u > 1` panics in the matrix relaxation (it is not a usable unstable set-up). The fv-level form of the tree cavity (`SimpleSettings::new`
  with 1.0/1.0, upwind): nu 0.01 finite at 6.7e79 after 500 iterations (max speed 1.8e5 at iteration 5); nu 1e-3 non-finite between iterations 350 and 375.
* S3 uniformly sheared 2-D channel (`channel` 40 x 10, length 4, height 1, points mapped x' = x + y tan(theta), inlet 1, walls, outlet; nu 0.01, SIMPLEC
  0.9/1.0), 600 iterations: converged for theta = 0, 30, 40, 45, 50 degrees with both `upwind` and `linear_upwind`; diverged (max speed above 1e6) for
  theta = 55, 60, 70, 75, 79, 85 with BOTH schemes (iterations 30 to 56), also with `non_orth_correctors = 1` at 60 and 79 degrees. This channel does not
  discriminate between the schemes; it is a separate finding (part B, item B-X), not the FAN-01 mechanism.
* S4 the `lumped` z4 fixed-duty heat-exchanger channel (ADR 0012 set-up), 3000 iterations: `averaged` stationary on max|ux - 2| = 1.886e-5, max|p| = 7.238e-4
  (kinematic); `corrected` max|ux - 2| 1.9e-13 to 4.7e-13, max|p| 3.5e-14 to 1.6e-13, max|uy| below 1.2e-13, pressure change over 250 iterations 9e-14 to 2.5e-13
  from iteration 750 on. Both runs have res_u 0.7 to 0.9 throughout (a uniform field makes the normalised residual meaningless).

What these smoke runs already tell, stated before the registration: (i) the steady divergences are finite for many iterations (a non-finite check alone does not
stop S1 or S2 for tens to thousands of iterations; the normalised residuals stay O(1) and cannot serve as a growth criterion); (ii) the FAN-01 set-up reproduces
34D cheaply; (iii) S4 suggests that `corrected` reaches the exact plug flow and the 34C "non-convergence" is a relative criterion applied to a zero pressure
field (registered as hypothesis H-D1 below, to be checked by a new test).

## Registered

### Part A: divergence guard of the steady solvers

Design (fixed now):

* New `rcfd_fv::simple::SteadyGuard` (read-only monitor) and `SteadyDivergence` (record). The API builds one guard per `Simulation` whose engine is the steady
  SIMPLE/SIMPLEC kernel or the steady block-coupled kernel (not the frozen-flow energy-only iterations) and observes it after every iteration in `Simulation::step`.
* Inputs of iteration i: `res_u`, `res_p`, `continuity` of the iteration, cell velocities, cell pressures, face fluxes; at the first observed iteration also the
  boundary-face velocities as built (fixed values, expressions, extrapolated values).
* K1 `non_finite`: any non-finite `res_u`, `res_p`, `continuity`, cell velocity component, cell pressure or face flux (fields scanned with `is_finite`, not through a
  `max` fold, which drops NaN). Always active, not switchable (as ADR 0067).
* References, fixed at the first observed iteration (iteration 1 of this simulation, also after a warm start or a restart): `U_ref` = largest speed over the cells and
  the non-empty boundary faces; `P_ref` = max(`U_ref`^2, max p - min p). If `U_ref` = 0, K2 and K3 are inactive for the run; if `P_ref` = 0, K4 is inactive.
* K2 `continuity_growth` (from iteration 2): `c_i > G max(min(c_j, j = max(1, i - W) .. i - 1), eps U_ref)` with G = 1e6, W = 50, eps = 1e-6
  (`continuity` is in velocity units; the floor `eps U_ref` keeps a converged run at round-off from tripping on a round-off jump: the threshold is at least `U_ref`).
* K3 `velocity_runaway` (from iteration 2): max cell speed `> A_u U_ref`, A_u = 1e6.
* K4 `pressure_runaway` (from iteration 2): `max p - min p > A_p P_ref`, A_p = 1e12.
* The record: iteration, kind, quantity (`res_u`, `res_p`, `continuity`, `velocity`, `pressure`, `flux`), cell (first bad cell, the cell of the largest speed or of the
  largest |p|, the owner cell of a face; none for a residual), value, limit (the threshold that was exceeded), `last_good` = the iteration with the smallest continuity
  among iterations 1 .. i - 1 (0 for a trip at iteration 1).
* On a trip the `Simulation` sets `failure` to the located message `steady divergence guard: <kind>: <quantity> is <value> (limit <limit>) at iteration <i>, cell <c> at (x, y, z);
  last good iteration <j> (continuity <c_j>); lower the relaxation factors or use solver.convection = "upwind"`, ends the run, and every later `step()` returns the same
  summary without iterating (as the PIMPLE guard returns its stale record). `Simulation::diverged()` is true, so `runctl::drive` ends with state `diverged`, exit code 6.
* Machine-readable: the `finish` event of a diverged run (steady or transient) gets an object `divergence` with `solver` (`steady`/`transient`), `kind`, `quantity`,
  `iteration` (steady) or `step`, `outer`, `stage` (transient), `cell`, `position`, `value`, `limit`, `last_good` (transient: step - 1). The PIMPLE kinds are
  `non_finite`, `courant`, `lag_amplification`. `summarize_stream` passes the object through.
* Key `solver.steady_divergence_guard`, enumeration `on` (default) / `off`; `off` disables K2 to K4 (K1 stays).
* The checks only read; for runs that do not trip every result is bit-identical (`baseline_hashes.rs` unchanged).
* Design loop: a diverged run is already `PointStatus::Diverged` / a failed `CondResult` with `Fitness::FAILED`; the change is that a steady divergence now reaches that
  state with a reason that names the kind and the last good iteration, instead of running to its cap as `not_converged`.

Justification of the constants (rule (a)): they are trip thresholds, not accuracy limits. They are justified by the headroom study A3b below (every converging run of the
suite must stay at least a factor 100 below each threshold) and by A1/A2 (the diverging runs exceed them within the registered iteration counts). If A3a finds a trip on a
converging run, the criterion that tripped is REMOVED (not re-tuned) and K1 stays; this is the registered fallback.

Checks (tests in `crates/rcfd-fv/tests/w35b_steady_guard.rs` and `crates/rcfd-api/tests/w35b_steady_guard.rs`):

| id | set-up | limit |
|----|--------|-------|
| A1a | fv level, cavity 12 x 12, nu 0.01, SIMPLE, upwind, alpha 1.0/1.0 (S2 finite runaway), guard observing after every iteration | trips at iteration <= 10, kind not `non_finite`; wall time of the guarded run < 5 s; located cell given |
| A1b | the same with nu 1e-3 (S2: non-finite later); an unguarded twin runs to its first non-finite state (iteration N_nan, at most 3000) | the guard trips at an iteration < N_nan |
| A1c | fv level, sheared channel theta = 79 degrees, SIMPLEC, upwind (S3) | trips at iteration <= 60 |
| A2a | API + `runctl::drive`, FAN-01 attempt-2 mesh, laminar, `linear_upwind` (S1) | state `diverged`, exit 6, reason contains `steady divergence guard` and `last good iteration`; the finish event has `divergence.kind` and `divergence.iteration` <= 15; wall time < 60 s |
| A2b | the same with SST (the 34D attempt-2 run that ran for 2 h) | as A2a, iteration <= 15 |
| A3a | never trips: the full `cargo test --workspace --release` run once with a scratch instrumentation (environment variable, not committed) that logs every guard trip | zero trips outside the tests of this wave that provoke one (A1, A2, A5, A6 by name) |
| A3b | headroom: the same instrumented run logs, for every steady `Simulation` that does not trip, the largest ratios K2 `c_i / max(window min, eps U_ref)`, K3 `max speed / U_ref`, K4 `range / P_ref` | the largest over the whole suite <= 1e4, 1e4, 1e10 (a factor 100 below G, A_u, A_p) |
| A3c | `baseline_hashes.rs` | unchanged, all registered hashes match |
| A3d | FAN-01 attempt-3 set-up (SST, `upwind`, design point, ADR 0075; converged in 8000 iterations there), first 1000 iterations | no trip |
| A4 | cavity 20 x 20, SIMPLE, `linear_upwind`, 60 iterations, guard `on` against `off` | u, p, phi bit-identical (`to_bits`) |
| A5 | design loop: study with one parameter x in [0.7, 1.0] mapped to `solver.alpha_u = x` and `solver.alpha_p = (7/3) x - 4/3` (0.3 at x = 0.7, 1.0 at x = 1.0) on the S2 cavity (nu 0.01, SIMPLE, upwind, cap 3000, tolerances 1e-6), full factorial with 2 levels | record x = 1.0: condition status `diverged`, not ok, message contains `steady divergence guard` and `last good iteration`, `Fitness::FAILED`, at most 10 iterations; record x = 0.7 converged; `study.best()` is x = 0.7 |
| A6 | the A5 unstable cavity with `solver.steady_divergence_guard = "off"` | runs to its cap (3000), state `not_converged` (K1 does not see the finite runaway); an invalid value is a located error at `solver.steady_divergence_guard` |

### Part B: `linear_upwind` on the FAN-01 H-slit mesh

Known (ADR 0075, S1): laminar and SST `linear_upwind` diverge from iteration 1 (max speed 6.5 times the boundary maximum after one iteration, about a factor 10 per
iteration afterwards); `upwind` is stable. In the steady kernel a scheme that needs a gradient also switches on the explicit non-orthogonal correction of the momentum
Laplacian (`simple.rs`: `grad` is passed to `laplacian` when `convection.needs_gradient()`), and `linear_upwind` extrapolates the face value
`u_U + grad(u)_U . (x_f - x_U)` with the unlimited least-squares gradient; on translationally or rotationally periodic (coupled) faces its central correction is applied in
full (`periodic_correction() = 1`).

Hypotheses and falsifiers (each a diagnostic run on the FAN-01 attempt-2 set-up, laminar unless stated, guard on):

| id | hypothesis | diagnostic | reading rule |
|----|------------|-----------|--------------|
| H0 | the least-squares gradient is wrong at some cells of the initial field (for example rotational-periodic vector values not rotated) | D0: at iteration 0 the gradient of the initial field `(Omega y, -Omega x, U_in)` (exact gradient constant) compared cell by cell, cells classified as wall (a face on blade/hub/shroud), outlet, seam (a face on a periodic patch), interior | H0 stands if a seam or interior cell has a gradient error above 1e-6 of the exact gradient norm; wall and outlet cells are reported only (their boundary values legitimately differ from the linear field) |
| L1 | (localisation) | D1: the cell of the largest speed at iterations 1, 2, 3 of the `linear_upwind` run: position, nearest patch, largest face non-orthogonality, skewness, aspect ratio; and the faces with the largest `abs(ho - ud)` at iteration 1 | reported |
| H1 | the explicit non-orthogonal correction of the momentum Laplacian (switched on by the gradient scheme) | D2: `linear_upwind` with the momentum Laplacian correction off; D2s: `upwind` with it on | H1 is necessary iff D2 is stable; sufficient iff D2s is unstable |
| H2 | the unbounded linear-upwind extrapolation | D3: `linear_upwind` with the Barth-Jespersen-limited cell gradient in the face extrapolation (Laplacian unchanged) | H2 stands iff D3 is stable |
| H2b | the off-centre-line part of the extrapolation vector (skewness) | D4: `linear_upwind` with `x_f - x_U` replaced by its projection on the centre line `d` | H2b stands iff D4 is stable |
| H2p | the full central correction on periodic coupled faces | D5: `linear_upwind` with `periodic_correction` 0 (upwind on coupled faces) | H2p stands iff D5 is stable |
| H3 | the rotating frame (frame source, moving shroud) | D6: the FAN-01 mesh at rest: no frame source, inlet `(0, 0, U_in)`, shroud at rest, `linear_upwind` | H3 is necessary iff D6 is stable |
| H4 | the pressure equation without non-orthogonal correctors (the S3 mode) | D7: `linear_upwind` with `non_orthogonal_correctors = 1` and `simplec_non_orthogonal_compensation = true` | H4 alone stands iff D7 is stable |
| H5 | start-up only (impulsive start from the rigid-rotation field), not an instability of the converged operator | D8: 300 iterations `upwind`, then 300 `linear_upwind` from that state | H5 stands iff D8 is stable |
| - | (information) | D9: `tvd_van_leer` and `tvd_minmod` | reported |

Stability criterion of every diagnostic run (fixed now): 300 iterations; STABLE iff no guard trip, the largest cell speed over iterations 10 to 300 is at most 3 times the
largest boundary-face speed (FAN-01: 40.03 m/s), and the continuity is finite with its maximum over iterations 200 to 300 not above its value at iteration 10 (the ADR 0075
criterion). Otherwise UNSTABLE. D0 and D1 are not runs of 300 iterations (D0 needs no iteration, D1 three).

Diagnostic code (switches of D2 to D5) lives only in a scratch tree and is not committed (as the 34B Y1 to Y3 code); D0, D1, D6 to D9 are expressible through the
API and are committed as ignored diagnostic tests in `crates/rcfd-api/tests/w35b_linear_upwind.rs`.

Smallest reproducing case (fixed now): among (a) `cases/axial_fan_sector` (the shipped laminar smoke case, 13 332 cells) with `convection = "linear_upwind"` and (b) the
FAN-01 attempt-2 set-up with `n_span = 6` (instead of 16), the one with fewer cells whose `linear_upwind` run is UNSTABLE and whose `upwind` run is STABLE by the criterion
above; if neither reproduces, FAN-01 itself is the reproducing case. Item B-X: the S3 sheared channel (both schemes unstable from 55 degrees) is reported as a separate finding;
no non-orthogonality threshold of part B is derived from it.

Decision rules (fixed now):
* Exactly the hypotheses whose reading rule says "stands" are reported as standing; the others are reported as falsified, also if that leaves no explanation.
* A fix is made only if a single hypothesis among H0, H1, H2, H2b, H2p stands, its diagnostic switch removes the instability, and it can be made opt-in. Its acceptance
  (FAN-01 laminar and SST stable by the criterion, and a no-harm check against an exact solution, rule (b)) is registered in an amendment BEFORE the fix is written.
  H0 standing is a defect in the gradient and is fixed regardless of the opt-in rule only if no baseline moves (otherwise opt-in).
* Otherwise (no single standing hypothesis, or the fix fails its acceptance and is removed): the safe-scheme rule is documented and enforced by a located check. The
  metric and the threshold of that check are registered in an amendment before the check is written, from the D1 localisation (the metric that singles out the cells
  where the divergence starts); the check is a set-up rule (located at `solver.convection`, severity registered in the amendment) evaluated by `--check-only`, the
  `run.headless` service and the design loop, plus a hint in the steady-guard message when the guard trips on a run that uses the scheme on such a mesh.

### Part C: the pressure-lag check through the API

Design (fixed now):
* Key `solver.pressure_lag_check`, enumeration `warn` (default), `raise`, `off`; applies to `solver.algorithm = "piso"` and `"pimple"` on the iterative path (not the
  fractional step, not dynamic meshes, where the fv check is not run).
* One estimate per run (cost): at step 1 for `euler`, at step 2 for `bdf2` and `bdf2_bounded` (step 1 of a BDF2 run is an implicit Euler step; ADR 0073 amendment 2 and the B4b
  false positive). This needs `LagCheck` to start at a given step: a new field `start` (`LagCheck::new` sets 1, so every existing fv use is unchanged).
* `warn`: a `rho >= 1` estimate is appended to the solver's `lag_warnings` (no change to the solution); `Simulation` hands each new warning to `runctl::drive`, which emits a
  `warning` event with `message` (located: step, rho, cell and position, remedy), `kind = "pressure_lag"`, `step`, `rho`, `cell`, `position`. `raise`: the ADR 0073 B4
  trip through the divergence guard (state `diverged`, exit 6, `divergence.kind = "lag_amplification"`).
* The estimate only reads the solver state (`lag_amplification` takes `&self`; no solver cache exists in `rcfd-linalg`/`pressure.rs`), so results are bit-identical in `warn`
  and `off`.

Checks (`crates/rcfd-api/tests/w35b_lag_check.rs`):

| id | check | limit |
|----|-------|-------|
| C1a | `baseline_hashes.rs` (its PISO/PIMPLE cases now run one estimate) | unchanged |
| C1b | PISO cavity 48 x 48 (2 304 cells, AMG pressure on), linear_upwind, dt 0.01, 20 steps, `warn` against `off` | u, p, phi bit-identical |
| C1c | the C2 set-up, 3 steps, `warn` against `off` | u, p, phi bit-identical |
| C2 | tree form of ADR 0073 B4d: cavity 32 x 32, nu 0.01, lid 1, `pimple`, n_outer 2, n_correctors 1, alpha_u 0.8, alpha_p 0.5, `linear_upwind`, `euler`, dt 0.125 (Co_t 4), default key, `runctl::drive` with 3 steps | exactly one `warning` event of kind `pressure_lag` at step 1 with rho >= 1 and the message naming `solver.n_correctors >= 2`; rho within 10 % of the fv-level B4d value 2.251 |
| C3 | C2 with `raise` | state `diverged`, exit 6, stops at step 1, reason contains `pressure-lag check`, `divergence.kind = "lag_amplification"`, the state finite |
| C4 | no false positive on the existing validation set: the instrumented A3a run also logs every lag warning issued through `Simulation` | zero warnings outside C2/C3/C6; the number of API-level PISO/PIMPLE runs that made an estimate is reported |
| C5 | cost of one estimate on the C1b case | wall time of the step with the estimate against a step without, reported (no limit) |
| C6 | BDF2 handling, uniform 32 x 32 Re = 100 PIMPLE-2 (ADR 0073 B3): `bdf2` Co_t 3.4 (stable, model G 0.68) and Co_t 4.6 (unstable, G 1.49), 3 steps | no warning for 3.4; exactly one warning, at step 2, for 4.6 |

The C2 and C6 tree set-ups are smoke-run (construction and the first step) before their limits are used; a set-up defect found there is an amendment.

### Part D: `corrected` component diagonal on the `lumped` z4 heat-exchanger channel

Hypothesis H-D1: the z4 channel (`channel_2d` 100 x 2, `symmetry` on both walls, uniform inlet 2 m/s, a heat exchanger without pressure loss, buoyancy off, temperature-
independent properties) has the exact solution u = (2, 0, 0), p = 0 (kinematic). The `corrected` iteration reaches it to round-off; the 34C "not stationary" verdict is the
`run()` helper's relative criterion (change divided by `max(max|p|, 1e-12)`) applied to a pressure field that is round-off (about 1e-13, so a change of about 1.8e-13 reads as
0.18). The `averaged` iteration is stationary on a wrong solution (the symmetry-plane sink of ADR 0010). This is not a solver defect; no solver change follows from it.

Falsifier (registered): the three z4 set-ups of `lumped.rs` (fixed duty -500 W; effectiveness 0.7 with C_sec 0 and 20) with `solver.component_diagonal = "corrected"`, run to
10 000 iterations. H-D1 is FALSIFIED if at iteration 1 500, 5 000 or 10 000 any of max|ux - 2|/2, max|uy|/2, max|p|/(U^2/2) (U = 2), or the change of ux over the previous 500
iterations divided by U, or of p divided by U^2/2, exceeds 1e-10 (round-off tolerance against the exact solution, a factor of about 1000 above the S4 values). Also measured with
`corrected`, limits of ADR 0012 unchanged: stream enthalpy change against the set duty 1e-6, reported duty 1e-9, effectiveness rise against the reported duty 1e-6, the
supplementary duty against eps C_min (T_sec - T_in) 1e-9. Reported for `averaged`: the same error measures (S4: 9.4e-6 and 3.6e-4). Also checked: the arithmetic of the 34C reading
(the corrected p change over 300 iterations after 1500 divided by `max(max|p|, 1e-12)`).

New test `crates/rcfd-api/tests/w35b_hx_corrected.rs`: `z4_corrected_reaches_the_exact_plug_flow` (the falsifier with its limits, all three set-ups). `lumped.rs` and the default
are not changed. `docs/component-diagonal-default.md` gets a dated section with the outcome (the 34C table row is not rewritten).

### Process

Own scratch clone `<scratch dir>`, target `<target dir>`. Commits of own hunks only (temporary index). Final gate on a fresh `git archive` of the
HEAD at hand-back: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast`. Documentation:
`docs/case-schema.md` (the two keys), `docs/run-control.md` (the `divergence` object, the `pressure_lag` warning), `docs/design-loop.md` (steady guard), `docs/milestone-fan.md`
(robustness row), `docs/component-diagonal-default.md` (part D).

## Amendment 1 (2026-10-19): the steady guard stops runs in the run-control driver, not in `Simulation::step`; A5 set-up defect

Written after the first runs of A1a-A1c, A2a, A2b, A4, A5 and A6 on the registered design, BEFORE A3 and before the corrected A5 test below was run.

What was run and what it showed (registered limits unchanged):
* A1a trips at iteration 6 (`continuity_growth`), A1b at iteration 5 against the twin's first non-finite state at 375, A1c at 26; A2a at 7 (`velocity_runaway`, 0.7 s),
  A2b at 8 (`continuity_growth`, 1.2 s); A6 runs to its cap (3000, `not_converged`). These meet their limits.
* A4: the registered set-up (cavity 20 x 20, SIMPLE, `linear_upwind`, schema defaults alpha_u 0.9 / alpha_p 1.0, 60 iterations) is the `baseline_hashes` case
  `cavity-simple`, and that case DIVERGES: a smoke trace with the guard off gives max speed 0.55 (iteration 1), 2.3e4 (10), 1.9e8 (20), 2.3e25 (60) and pressure range 1.2e50 at
  iteration 60; the registered hash (60, 07e7461c36893384) fingerprints iteration 60 of a divergence. With the guard on the run trips at iteration 14 (`continuity_growth`).
  The A4 test failed on an assertion I had added that is NOT part of the registration (`!diverged()`); its registered limit (bit identity on/off) was not reached.
* A5: set-up defect. `(7/3) x - 4/3` at x = 1.0 is 1.0000000000000002 in floating point, outside the schema range of `solver.alpha_p`; the candidate was recorded `failed`
  with that located error after 0 iterations, so the divergence path was not exercised.

Consequence for the design: with the registered design the run of `cavity-simple` would end at iteration 14 and `baseline_hashes::cavity_paths_match_registered_hashes`
would change from (60, 07e7461c36893384) to (14, ...). Hashes are never re-recorded, so the design changes as follows (D-A1):
* `Simulation::step` keeps observing and RECORDS the first trip (`Simulation::steady_divergence()`, `diverged()` true, `divergence_json()`); it does not set `failure` or end the
  run, and later `step()` calls keep iterating (no stale return; the guard stops checking after its first trip). Every result of a direct caller (`Simulation::step`/`run`,
  the GUI, library users, `baseline_hashes`) is bit-identical with and without the guard.
* `runctl::drive` ends the run after the step that tripped: state `diverged`, exit code 6, reason = the located message, `finish.divergence` = the record. Every automated
  path goes through it: `rcfd run`, the `run.headless` service method and its MCP tool, the design loop (`case_eval::run_case`, hence studies and operating maps).
* The PIMPLE/PISO guard of ADR 0067 is unchanged (it stops inside the solver).

Amended checks:
* A3a: a trip on a run that diverges is correct. Every trip outside this wave's tests is logged by the instrumentation with the test binary, the kind, the trip iteration,
  the final iteration, `is_converged()` at the end, and the final max speed over `U_ref`. A trip is a FALSE POSITIVE if the simulation is converged at the end or its final fields
  are finite with max speed <= 10 `U_ref`; otherwise it is a true divergence. Limit: zero false positives. (Already known when writing this: `cavity-simple` trips at 14 and
  diverges to 2.3e25 `U_ref` units by 60; it will be listed as a true divergence.) A3b applies to the simulations without a trip.
* A4 is run as registered (bit identity of u, p, phi with `on` and `off` after 60 iterations of the `cavity-simple` set-up); the trip iteration with `on` is reported.
* A5 corrected (new test `a5_corrected_diverged_candidate_is_a_failed_evaluation`): x in [0, 1], `solver.alpha_u = 0.25 x + 0.75`, `solver.alpha_p = 0.75 x + 0.25`
  (0.75/0.25 at x = 0 and exactly 1.0/1.0 at x = 1); smoke run of the (0.75, 0.25) point: converged in 60 iterations (res_u 1.8e-6 at 50). Limits as registered for A5, with
  "x = 0.7" read as x = 0 and "x = 1.0" as x = 1. The registered A5 test is kept, ignored, as `a5_registered_setup_defect` (limits unchanged).

## Amendment 2 (2026-10-19): part B outcome of the registered diagnostics; the H0 fix and the safe-scheme check registered before either is written

Written after D0 to D9 and the smallest-case runs, BEFORE any code of the H0 fix or of the check below, and before the A3/C4 instrumented suite run.
The diagnostic switches of D2 to D5 were environment variables read by scratch-tree solver code (not committed).

Measured (FAN-01 attempt-2 set-up, laminar, the registered criterion; U_b = 40.03 m/s):

| run | outcome |
|-----|---------|
| control `linear_upwind` | UNSTABLE (guard trip at iteration 7, `velocity_runaway`) |
| control `upwind` | STABLE (max speed 10..300 = 47.7, continuity 8.2e-5 at 10, max 6.1e-5 over 200..300) |
| D0 gradient of the initial field (exact gradient constant) | interior 9 792 cells: max error 2.1e-13 Omega; seam 1 092 cells: 0.173 Omega; inlet 4.0e-2, outlet 4.3e-2 (corner cells next to the seams), wall 2.2e3 (the no-slip jump, legitimate). H0 STANDS |
| D1 localisation | the largest speed at iterations 1 and 2 is in wall cell 295 (hub, r = 0.124, z = 0.333, downstream of the blade; aspect ratio 781, largest face non-orthogonality 73.3 deg), at iteration 3 in wall cell 13023 (shroud, r = 0.250, aspect ratio 944 = the mesh maximum, non-orthogonality 62.9 deg); the largest linear-upwind face corrections at iteration 1 (630 to 734 m/s against a boundary maximum of 40 m/s) are in wall cells next to the blade (aspect ratio 150 to 200) |
| D2 `linear_upwind` without the momentum non-orthogonal correction | UNSTABLE (trip at 7): H1 not necessary |
| D2s `upwind` with the momentum non-orthogonal correction | STABLE: H1 not sufficient; H1 FALSIFIED |
| D3 `linear_upwind` with the Barth-Jespersen-limited gradient | STABLE (max speed 51.0): H2 STANDS |
| D4 centre-line extrapolation | UNSTABLE (trip at 7): H2b FALSIFIED |
| D5 upwind on the periodic coupled faces | UNSTABLE (trip at 7): H2p FALSIFIED |
| D6 the mesh at rest (no frame) | UNSTABLE (trip at 7): H3 FALSIFIED |
| D7 one non-orthogonal pressure corrector with the compensation | UNSTABLE (trip at 7): H4 FALSIFIED |
| D8 `linear_upwind` after 300 `upwind` iterations | UNSTABLE (trip at 6 of the second phase): H5 FALSIFIED |
| D9 `tvd_van_leer` / `tvd_minmod` | UNSTABLE (trip at 40) / STABLE (max speed 52.9) |
| smallest case (a) `cases/axial_fan_sector` | `linear_upwind` and `upwind` both STABLE (laminar at Re_c about 5): does not reproduce |
| smallest case (b) FAN-01 with `n_span = 6` | refused by the generator (`mesh.fan.n_span`: the boundary layers and the tip gap leave no room): set-up defect, not run |

By the registered rule the reproducing case is FAN-01 itself (13 332 cells). Two hypotheses stand, H0 and H2, so the registered decision rule gives: no fix of the
linear-upwind face value; the safe-scheme rule is documented and enforced by a located check (below); and H0, a defect of the gradient, is fixed (opt-in, below).

Mechanism read from D0 to D9 (stated as the reading, not a further test): the unlimited linear-upwind extrapolation `grad(u)_U . (x_f - x_U)` from wall cells of very high
aspect ratio whose faces are strongly non-orthogonal. The least-squares gradient of such a cell is dominated by the near-wall jump, and the extrapolation over the long side of
the cell turns it into face values tens of times the largest boundary speed (D1). Limiting the gradient removes the instability (D3); projecting the extrapolation vector on
the centre line does not (D4); neither the rotating frame (D6) nor the start-up (D8) nor the pressure equation (D7) nor the viscous non-orthogonal correction (D2) is needed.

### H0 fix (opt-in), registered now

D0 shows that on rotationally periodic seams whose cells are not aligned across the seam, the least-squares gradient takes the coupled face value (the distance-weighted
mean of the owner and the rotated partner cell value) at the face centre, which for a linear field is not the value at the face centre: the error of the seam-cell gradient
is O(1) of the gradient (0.17 Omega). Fix F-H0: with the opt-in key `solver.periodic_gradient = "partner_cell"` (default `"face_value"`, the historical form) the steady
SIMPLE/SIMPLEC momentum gradient uses, on every coupled (periodic) boundary face, the partner cell itself: displacement `(x_f - x_o) + R (x_q - x_g)` (f the face, o its
owner, g the partner face, q its owner, R the rotation of the pair or the identity for a translation) and value `(R u_q)_k - u_o,k`, which is exact for linear fields. Only
the momentum gradient of the steady kernel uses it (not the transient kernel, not scalars, not turbulence); stated.
Acceptance (all): H0-a D0 rerun with `partner_cell`: interior and seam max gradient error <= 1e-10 Omega (an exact solution: the exact gradient of a linear field); inlet,
outlet and wall reported. H0-b with the key unset or `face_value`, FAN-01 `upwind` 20 iterations bit-identical (`to_bits` of u, p, phi) between the key absent and
`face_value`, and the full suite unchanged. H0-c (reported, no limit) FAN-01 laminar `linear_upwind` with `partner_cell` classified by the registered criterion. If H0-a or
H0-b fails the fix is removed (rule (b) of wave 35). Default: opt-in, because a gradient change moves every periodic case that uses a gradient scheme (the registered rule
allowed a default only if no baseline moves; that was not tested, so the conservative choice is taken and stated).

### The safe-scheme rule and its located check, registered now

Metric (from D1): per cell `M_c = AR_c tan(theta_c)`, AR the cell aspect ratio of `rcfd_mesh::quality::compute_metrics`, theta the largest non-orthogonality of the
cell's faces (capped at 89.9 deg); the mesh value `M = max_c M_c`. On FAN-01 M = 2 759; the two cells where the blow-up starts have M_c = 2 597 and 1 842 and rank 15 and
38 of 13 332 cells by M_c (by aspect ratio alone 140 and 1, by non-orthogonality alone 572 and 2 897).
Threshold study (registered, run with the A3a instrumentation): every steady SIMPLE/SIMPLEC simulation of the suite that uses `linear_upwind` or `tvd_van_leer` logs M and
whether the guard tripped. T = 2 x the largest M of those that did not trip, rounded up to two significant digits. The check is written only if T < 2 759 (it separates
FAN-01); otherwise the metric does not separate and no check is written (the rule is documented with the FAN-01 numbers, and the guard message carries it).
Check (if written): the mesh quality report gets `max_aspect_non_orthogonality` (= M; `serde(default)` for old reports); a setup rule `SV106`, severity WARNING, category
`solver`, path `solver.convection`, when `solver.algorithm` is `simple` or `simplec` (`coupled` not included: not tested), `solver.convection` is `linear_upwind` or
`tvd_van_leer` (the two schemes measured unstable on FAN-01) and `$mesh.max_aspect_non_orthogonality > T`; the message names the metric, its value and the remedy
(`upwind` or `tvd_minmod`, both measured stable on FAN-01). It is evaluated where the rules already are (`--check-only`, the `run.headless` warnings, the design loop). The
steady-guard message gets the same hint when it trips on a run with one of these schemes on a mesh above T. Severity warning, not error: the rule rests on one case, and the
guard stops the divergence within the first iterations; making it an error is the user's decision (recorded).
Checks: S-a FAN-01 laminar `linear_upwind` gives exactly one SV106 warning at `solver.convection`; S-b the same case with `upwind` and with `tvd_minmod` gives none; S-c
`cases/axial_fan_sector` (M reported) gives none unless its M exceeds T (reported); S-d the A2a reason contains the hint.

## Amendment 3 (2026-10-19): results of the instrumented suite run; consequences registered before the code they govern

Written after ONE run of `cargo test --workspace --release --no-fail-fast` on the scratch tree (wave code up to the H0 fix, without the SV106 check, with the
instrumentation of A3a) and before any of the changes below. Totals of that run: 1846 passed, 2 failed, 490 ignored.

What it showed:
* The two failures are `particle_run::w2_steady_run_path_equals_track_frozen` and `w3_flow_untouched_and_baseline_files_unchanged`: their SIMPLE set-up (20 x 20 box, schema
  relaxation 0.9/1.0, `linear_upwind`) diverges like `cavity-simple` (max speed 5.5e5 `U_ref` when stopped), they drive it through `runctl::drive` and accept only exit 0 or 5;
  with the guard the run ends `diverged` (exit 6) at iteration 14.
* A3a: 1181 steady simulations were logged, 22 tripped. Every trip outside this wave's tests is on a diverging set-up: `cavity-simple` and copies of it (`baseline_hashes`,
  `gpu_case` c2 and d2, `particles_case` pt14, `particles_two_way` pl8 on a 12 x 12 variant, `particle_run` w2/w3; final max speed 5.5e5 to 2.3e25 `U_ref`), the `eval` suite
  (trip at 5, 8.4e80 `U_ref` at its end), and one CLI run of the `rcfd` binary with `central` convection (trip at 95, `velocity_runaway`, 1.0e6 `U_ref`). Zero false positives by
  the amendment 1 rule: A3a met.
* A3b headroom over the 1159 steady simulations without a trip: K2 4 980 (`pipe_sweep_compensation` r3_r4), K3 234 (`mhd` a5), K4 396 (`sources` rotating_channel), against the limits
  1e4, 1e4, 1e10: met. A3c: `baseline_hashes` passed (all registered hashes).
* Threshold study for SV106: 514 steady `linear_upwind` simulations without a trip; the largest M is 18 725 (the non-conformal rotor-interface and MRF Taylor-Couette annulus meshes
  of `rotor_interface.rs` and `mrf.rs`, 3 120 cells, converging), then 4 998 and 3 705, all above FAN-01's 2 759. By the registered rule the metric does NOT separate: no SV106 check
  is written. The rule is documented with the FAN-01 numbers and the steady-guard message carries it (below). S-a to S-c are therefore not applicable; S-d is kept.
* C4: 233 transient simulations made an estimate; 10 warnings outside C2/C3/C6, in 3 existing tests, all with `n_correctors = 2` (the schema default): `user_scalars` u3 (8 runs of a
  fluid AT REST, closed box, rho 1.46 to 8.14), `rheology` transient_runs_use_the_law_too (rho 4.17), `transient_sources` t5 (rho 5.37). C4 as registered (zero warnings) is MISSED.
  The u3 runs cannot grow (no flow, no pressure excitation): they are false positives by the ADR 0073 B4b definition (a stable run warned). The other two were not classified.
  The ADR 0073 model and its accepted checks (B4a to B4d) are for one pressure corrector; with two correctors B4c measured rho 0.13 to 0.21 at moderate diffusion numbers, and these
  warnings come from two-corrector runs at large diffusion numbers where the estimate is not validated.

Consequences, registered now:
* C-R (restriction of the default): `solver.pressure_lag_check` `warn` and `raise` make the estimate only when `solver.n_correctors = 1` (the regime of the ADR 0073 model and of
  B4a-B4d); with more correctors no estimate is made. The key, values and default are unchanged. C4' (corrected C4): the same instrumented accounting on the final tree: zero
  warnings outside C2/C3/C6. Stated now: no existing API-level test uses one corrector (searched: only `w26a_couette.rs`, fv level, sets it), so C4' on the existing suite is met
  by construction and says nothing about one-corrector validation cases; it is reported as such. C1a-C1c, C2, C3, C5, C6 are re-run on the final tree (their set-ups use one
  corrector except C1b, which then makes no estimate and stays bit-identical trivially).
* The warning message names the configured correctors (it said "with one pressure corrector" for every run).
* `particle_run` w2/w3: their SIMPLE text gets `steady_divergence_guard = "off"` (a set-up change of an existing test, no limit touched): it restores the previous behaviour of
  that diverging set-up bit for bit; both tests test particle tracking, not the flow.
* The service background run loop (`run.start`, `service.rs`) does not use `runctl::drive` (amendment 1 said every automated path does; that was wrong for this loop): it now ends
  a run whose steady guard tripped with state `failed` and the located message (`error`), as it does for non-finite residuals. Check SV-1 (new test): `run.start` on the S2
  unstable cavity ends `failed` with an error containing `steady divergence guard` within 30 s.
* Guard hint (from the threshold result): when the steady guard trips on a run with `linear_upwind` or `tvd_van_leer`, the message adds that these schemes diverged on the
  high-aspect, strongly non-orthogonal wall cells of the FAN-01 sector mesh where `upwind` and `tvd_minmod` were stable (ADR 0077 part B). S-d: the A2a reason contains it.
