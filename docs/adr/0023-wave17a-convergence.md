# ADR 0023: Wave 17A convergence: residual-floor diagnosis (known misses), pseudo-transient controller (F-106), adaptive stepping (F-209)

Status: accepted. Everything under "Diagnosis experiments", "Decisions" and "Registered checks" was written before any of
the new experiments or tests was compiled or run (only the existing code and records were read). Limits and decision rules are not changed, widened, replaced or deleted after a run.
A miss is reported as measured and the assertion moves to an ignored `*_known_miss` test with the registered limit
intact; causes are diagnosed, not tuned away. A set-up defect gets a NEW corrected test and the old one stays as a
record. Addenda (measured values, file names, deviations) may be appended at the end without touching registered limits.

Roadmap: F-106 (pseudo-transient), F-209 (adaptive stepping, accelerators; partial), F-210 (only if time: not
registered here, see "Not registered"). Touches F-032, F-027, F-050 only by the audit notes below.
Clean-room: published literature only; no vendor code, text or data; no vendor names in the repository. Formulas quoted
from memory are marked "(from memory, unverified)".

## Context (audit of what exists)

* `rcfd-fv/src/simple.rs`: the SIMPLE/SIMPLEC pseudo-transient term (`V/dt` added to the momentum diagonal with the
  explicit old-iterate source, before relaxation) exists with `PseudoMode::{Physical, Local, Auto}` and a `factor`;
  `solver.pseudo_mode/pseudo_cfl/pseudo_dt/pseudo_factor` are in the settings tree (wave 5). There is NO automatic
  controller: the step is a fixed number chosen by the user. The adaptive transient step exists only as the Courant
  controller `ddt::CflController` (`max_courant`); there is no error-estimator step.
* The momentum residual of `SimpleSolver::iterate` is `normalised_residual(sys, u_old)` evaluated before the linear
  solve: `sum|b - A x| / (sum(|A x - A xbar| + |b - A xbar|) + 1e-30)`, `xbar` the mean of `x` (the usual
  reference-subtracted normalisation, from memory, unverified as a literature citation). The pressure residual
  uses the same function on the first non-orthogonal pass.
* Registered known misses sharing "the residual does not get below the target": ADR 0022 addendum (C2, C3, D1, D2 plain
  `simplec` from rest diverges on a frictionless duct driven by two pressure ends; the control with existing conditions
  shows normalised residuals 0.3 to 0.7 for 1500 iterations; E2 stalls at `res_u = 4.4e-12` with stationary fields),
  `variable_density.rs::subsonic_nozzle_residual_target_1e8_known_miss` (res_u 2.6e-5, res_p 6e-13 at 3000 iterations;
  note: variable-density kernel `varden.rs`, same `normalised_residual`), wave-13 D5b / D5 n_r = 128 (residual about
  1.7e-11 against a target 1e-11, `wall_motion.rs`, ADR 0016).

## Diagnosis experiments (Task 1), registered with decision rules

Notation. For a momentum system `A x = b` at the iterate `x`: `num = sum|b - A x|`;
`N_ref = sum(|A x - A xbar| + |b - A xbar|)` (denominator of the existing residual); `S = sum_i (sum_j |a_ij||x_j| + |b_i|)`
(the magnitude of the terms that are added up, so `eps * S` is the round-off size of `num`, `eps = 2.2e-16`);
`res_ref = num / N_ref` (existing), `res_bwd = num / S` (normwise backward error). These are computed by a
diagnostic that does not alter the solve (`SimpleSolver::record_parts`, off by default). Everything is the largest
over the active velocity components, as the existing residual is. Fields are read from a SETTLED state (largest change
of ux, uy, uz, p between iterations <= 1e-10 for 20 iterations; the post-hoc settle criterion of ADR 0022) and from the
stall point for E2 and D5b.

* **X1 (H1 normalisation denominator vanishes with the iterate).** Cases: the ADR 0022 control (frictionless
  `box_2d` 20 x 4 duct, existing conditions: inlet velocity 1, static outlet 0, symmetry sides, `pseudo_dt = 0.2` so that
  it runs) and C2/D1/D2 settled with the post-hoc set-ups. Prediction if H1: at the settled state `x` is uniform to
  round-off, so `A x - A xbar ~ 0`, `N_ref ~ num` and `res_ref` is O(1) however small `num` is. Decision rule: H1 holds
  for a case iff, at the settled state, `res_ref >= 1e-3` AND `res_bwd <= 1e-13` AND `N_ref / S <= 1e-9`. H1 is rejected
  for the case iff `res_ref <= 1e-9` or `res_bwd > 1e-11`.
* **X2 (H1' round-off floor amplified by the normalisation, non-uniform fields).** Cases: E2 at its stall and D5b
  (`n_r = 128`, ADR 0016 set-up, tolerance 1e-11) at 6000 iterations. Rule: the floor is round-off in the residual
  evaluation, amplified because `N_ref << S`, iff at the stall `res_bwd <= 1e-13` (that is, `num <= ~500 eps S`) AND
  `res_ref / res_bwd >= 30` (`S / N_ref`). It is a genuine slow or non-contracting iteration iff `res_bwd >= 1e-11`.
  In between (1e-13 < res_bwd < 1e-11) the verdict is "inconclusive", reported as such.
* **X3 (H2 linear-solver tolerance floor).** E2 (and D5b at 6000 iterations where affordable) rerun with the momentum
  and pressure linear solvers set to `tol_abs = 0`, `tol_rel = 1e-9`, `max_iter` 5000 (instead of
  `1e-12 / 1e-3 / 200` and `1e-14 / 1e-3 / 2000`). Rule: H2 confirmed iff the `res_ref` at the end of the same iteration
  count is lower by a factor >= 10 AND keeps decreasing (the last-500-iteration geometric slope < -1e-3 per iteration in
  log10 units); H2 rejected iff the ratio of final residuals is within [1/3, 3].
* **X4 (H3 round-off accumulation in the corrected face flux / state drift).** On the settled E2 state, run 500 more
  iterations and record the largest cell-value change per iteration. Rule: pure round-off noise iff the change is
  `<= 1e-13` and does not decrease monotonically (a flat noise band); a slow mode iff the change decreases monotonically
  by more than a factor 2 over the 500 iterations. The continuity imbalance of the same state is recorded (limit 1e-9
  for the M check, not a decision input).
* **X5 (H4 relaxation / pseudo-time interplay; divergence from rest).** For the ADR 0022 control duct and
  `alpha_u = 0.9`, plain simplec, `nu in {1e-3, 1e-2, 1e-1}`: record the largest velocity after the first iteration
  `u1`. Closed-form expectation: at rest the momentum diagonal is the diffusion plus relaxation only, so the SIMPLE
  step is an implicit pseudo time step `dt_eff = alpha/(1-alpha) V/a_diff` which scales as `1/nu`, and the first
  pressure-driven velocity is `u1 ~ dp dt_eff / L`. Rule: the over-shoot is the lack of a convective/inertial scale in
  the diagonal (not a coding defect) iff `u1 * nu` is constant within 15 % over the three viscosities AND `|u1| >> 1`
  at `nu = 1e-3`; the pseudo step is then the cure and the registered A3 uses it.
* **X6 (H5 pressure reference on all-pressure-boundary domains).** Rule by construction and by one run: the
  `p_ref` branch of `solve_pressure` is active only without a Dirichlet pressure patch (`bc.has_dirichlet()`); C2, C3,
  D1, D2, E2 all have a fixed-pressure patch, so H5 is rejected for them by inspection. A closed lid-driven cavity
  (pressure level fixed by `p_ref`) at 32 x 32, Re 100, simplec 0.9/1, is run with the residual targets 1e-12 and
  `res_bwd` recorded; H5 is rejected for the registered cases iff that closed case also reaches `res_ref < 1e-12`
  (no floor arises from the reference-cell handling).
* **X7 (nozzle: floor or slow convergence).** The variable-density nozzle case of `variable_density.rs`, 3000
  iterations with parts recorded every 100 iterations. Rule: a floor iff `res_bwd <= 1e-13` or `res_ref` is flat (last
  1000 iterations drop by < factor 2); slow convergence iff `res_ref` falls by >= factor 10 in the last 1000 iterations
  (then more iterations would meet the target, no residual defect); neither: "limit-cycle / unconverged".

Verdict format: for each known miss, which hypothesis the measured numbers satisfy under these rules, with the numbers.
No experiment may be rerun with other thresholds.

### Options a genuine defect may motivate (default = old behaviour)

* `ResidualNorm::{Reference, Backward}` on `SimpleSolver` (`solver.residual_norm = "reference" | "backward"` in the
  tree), applied to the MOMENTUM residual only (the pressure residual keeps the reference normalisation; the pressure
  residual of the registered cases is not the stalled one, X2 will show if not). `Reference` is the present code
  path and must remain bit-identical. `Backward` = `num / S`. Provided only if X1 or X2 supports H1/H1'. It changes what
  "converged" means for a given number (a stricter-looking number measures something different), which is why it
  is an option and the registered tests keep their original residual definition.
* Nothing else is changed in the linear solvers or the pressure-velocity core for Task 1 unless a hypothesis other than
  H1 is confirmed, in which case the fix goes behind its own option and a new registered check is added in an addendum.

## Decisions (Task 2)

### D1 Pseudo-transient controller (F-106), `rcfd-fv/src/simple.rs`

`SimpleSolver.pseudo_control: Option<PseudoController>` (default `None`; with `None` the code path is unchanged). The
controller multiplies the pseudo time step (every cell, any mode) by a scale `s`, `s_0 = 1`, updated after every
iteration from `r_n = max(res_u, res_p)` with the switched-evolution-relaxation idea (Mulder and van Leer 1985, from
memory, unverified: step proportional to the inverse of the residual) with safeguards:

```
if !r_n finite or r_n > rise * r_{n-1}:  s <- max(min_scale, backoff * s)
else:                                    s <- min(max_scale, s * clamp(r_{n-1} / r_n, 1, growth))
```

Registered constants (library defaults, not tuned after any run): `growth = 2.0`, `backoff = 0.5`, `rise = 2.0`,
`min_scale = 1/64`, `max_scale = 1000`. The first iteration uses `s = 1`. The base step is the existing mode's step
times `factor`, so the starting local Courant number is `pseudo_cfl * pseudo_factor`, the cap is that times
`max_scale`. Settings tree: `solver.pseudo_control = "off" | "ser"` (default off; any other pseudo_mode `off` with
control on is a located error) and `solver.pseudo_scale_max` (default 1000). With control on, `alpha_u` is whatever the
user sets (the checks use 1.0). The history of `s` is kept in `SimpleSolver.pseudo_scale_history`.

Not part of this decision (not delivered): per-cell step limiting by a local residual, residual smoothing, extrapolated
step initialisation, nonlinear (Aitken-type) acceleration, stretched-mesh pseudo-time acceleration (F-209 remainder).

### D2 Error-estimator transient step controller (F-209, partial), `rcfd-fv/src/ddt.rs`, `pimple.rs`

`ErrorController { tol, safety = 0.9, growth = 1.5, shrink_min = 0.3, min_dt, max_dt, order = 1 }`. The local error
estimate of step `n` is `e = max_c |u_n - u_pred| / U_ref` where `u_pred` is the linear extrapolation of the two
previous levels to `t_n`, `U_ref = max(max|u_n|, 1e-30)`; `0.5` of that difference estimates the local error of a
first-order (implicit Euler) step (the extrapolation error of a smooth solution is `O(dt^2)`, same order as the
Euler local error; the factor is the usual Milne-type constant for the pair, from memory, unverified, used as stated
and not fitted). Step update `dt_new = dt * clamp(safety * (tol / e)^(1/2), shrink_min, growth)` bounded by
`[min_dt, max_dt]`. A step with `e > 2 tol` is rejected (state restored, `dt` reduced) only in the scalar utility; the
`PimpleSolver::run_error_controlled` accepts every step (no restore of PIMPLE history: documented limitation) and uses
the update for the next step. A Courant cap `max_cfl` may be combined (minimum of both). Not wired into the settings
tree unless stated in an addendum.

## Registered checks (limits fixed before any run)

### R regression and defaults
* **R1** `crates/rcfd-api/tests/baseline_hashes.rs` unchanged and passing (bit-identical default numerics; hashes not
  re-recorded).
* **R2** with `pseudo_control` off and `residual_norm = reference` the iteration histories (`res_u`, `res_p`, `max_du`)
  of a lid-driven cavity run are bitwise identical to a run built without touching the new fields (compared through the
  settings tree: new keys at their defaults versus the keys absent).

### X diagnosis (rules above); reported, each as a numeric table. A diagnosis experiment that cannot run is reported as such.

### P controller (F-106), lid-driven cavity Re 1000, 32 x 32 `cavity_2d`, linear upwind, SIMPLEC, `alpha_u = 1`, `alpha_p = 1`, residual `max(res_u,res_p) < 1e-6`, h from solution (default)
* **P1** `pseudo_mode = local`, `pseudo_cfl = 1`, controller on: converges within 2500 iterations; the converged
  state differs from the fixed `cfl = 5` converged state by at most `5e-4` in `max |u - u_ref|` (lid speed 1); the
  iteration count is not larger than that of the fixed `cfl = 2` run in the same test (fixed `cfl = 1` is also run and
  reported). The scale history is bounded by `[1/64, 1000]` and every step finite.
* **P2** the same with `pseudo_mode = physical`, `pseudo_dt = 0.02` (start) and `auto` with `factor 0.05`: each
  converges within 3000 iterations to the same state within `5e-4` (reported separately, each its own assertion).
* **P3** robustness to the starting step: local `cfl` in {0.5, 1, 2, 5} with the controller all converge within 3000
  iterations (the fixed `cfl >= 10` runs of the wave-5 record failed; this is the test that the controller can start
  large, so also local `cfl = 10` and `20` are run: the criterion is that they converge within 3500 iterations;
  if they do not it is a miss recorded as `p3_large_start_known_miss`).
* **P4 (step, Re_h = 400, F-106 acceptance part not met in wave 6)** the `tests/step_laminar.rs` set-up (parabolic inlet,
  ER 2, 120 x 16 mesh, vanLeer, SIMPLEC `alpha_u = 1`, local start `cfl = 1`, controller on) reaches the same residual
  criterion as that file's other converging recipes (`< 1e-6` there; read from the file, not changed) within 4000
  iterations. Expected possibly a miss (the fixed `cfl = 5` run did not converge); a miss becomes
  `p4_step_re400_known_miss`.

### F frictionless-duct and E2 registered residual convergence with the corrected set-ups (new tests; the old `*_as_registered_known_miss` stay untouched)
Same set-ups, accuracy limits and residual targets (1e-12, 6000 iterations) as ADR 0022 C2, D1, D2, E2, with plain
`simplec` at `alpha_u = 0.9, alpha_p = 1` (the registered one) but with `pseudo_mode = local`, `pseudo_cfl = 1`, controller
on, and `residual_norm = backward` ONLY IF X1/X2 supports H1/H1' (otherwise `reference`).
* **F1** C2: residual targets met within 6000 iterations; mean |u_x| = 1 within 1e-5.
* **F2** D1: residual targets met within 6000; mean |u_x| = 1 within 1e-5.
* **F3** D2: residual targets met within 6000; magnitude within 1e-5 and angle within 1e-6 rad.
* **F4** E2: residual targets met within 6000 with the registered `E2_CASE` (no pseudo-time unless X5 shows the
  need; `residual_norm` per the rule above): outlet pressure error <= 3 % of 1.5 Pa, outlet u_theta <= 3 % of 2,
  mid-length pressure <= 3 %, discrete balance <= 1e-8 of range, imbalance <= 1e-9.
A miss is recorded as `f?_known_miss` and not retried with other constants.

### W api wiring
* **W1** each of `pseudo_control = "ser"`, `pseudo_scale_max`, `residual_norm = "backward"` through the settings tree
  equals the library run to 1e-12 in the fields after 200 iterations.
* **W2** located validation errors: `pseudo_control = "ser"` with `pseudo_mode = "off"` reports the path
  `solver.pseudo_control`; unknown enumeration values are rejected by the schema.

### E adaptive transient stepping (F-209, partial)
* **E1** linear decay `du/dt = -lambda u`, `lambda = 3`, `u(0) = 1`, `t_end = 1`, implicit Euler through `add_ddt`
  (uniform-field 8-cell box as in `ddt.rs` tests): the error-controlled run with `tol = 1e-3`, `min_dt = 1e-4`,
  `max_dt = 0.2`, `dt0 = 1e-3` ends with error `E_c`; the fixed-step run that first achieves `error <= E_c` over step
  counts N = 1, 2, 3, ... (all integers, uniform steps) needs `N_f`; the controlled run uses `N_c <= 0.8 N_f` steps.
  `E_c` itself must also be <= `10 tol` (the estimator is meaningful).
* **E2** Taylor-Green vortex, 2D, `box_2d` 32 x 32, `lx = ly = 2 pi`, both directions periodic, `nu = 0.1`,
  `PimpleSettings::piso`, linear (central) convection, `t_end = 1`: exact `u = -cos x sin y exp(-2 nu t)`,
  `v = sin x cos y exp(-2 nu t)`. The fixed run uses `Co = 0.5` (`dt = 0.5 h / 1` with `U = 1`), the controlled run
  `tol = 1e-3` with the same Courant cap. The checks: the controlled run's L-infinity velocity error at `t_end`
  <= 1.2 x the fixed run's, AND its step count <= 0.8 x the fixed run's. A miss is recorded as `e2_tg_known_miss`
  (the spatial error of the 32 x 32 mesh may dominate; this is a statement about whether the target error is reached in
  fewer steps, nothing more).
* **E3** `max_courant`-controlled and error-controlled stepping never exceed their Courant cap (existing guarantee,
  re-asserted for the combined controller).

## Not registered (not delivered unless an addendum says so)
F-210 fractional-step (no time after the above), the high-aspect-ratio flat-plate acceleration of F-209 (needs the
turbulence stack; separate wave), residual smoothing, Aitken acceleration, error-estimator wiring in the settings tree.
The coupled solver (F-032) and AMG (F-027) are audited, not modified: F-027's V-cycle gate is not in scope.

## Addendum A1: post-hoc experiments registered after the first diagnosis runs (rules written before these runs)

The first runs (X1, X5, X6, X7; E2 with a wrongly typed mesh that lacked `nx = 48`, discarded, never reported as an E2
result) showed that the registered X1 rule does not separate two things: a residual that is at round-off, and a
reference-normalised residual that stays near 0.95 while the numerator falls by six orders (the numerator at the settle point
of C2 was 1e-9, not yet round-off, because the settle criterion (change <= 1e-10) fires early). The following controlled
experiments are therefore registered now, before they are run; they do not replace X1-X7.

* **X1b (continue past the settle point, two normalisations).** C2, D1, D2 post-hoc set-ups (physical pseudo step 0.2,
  `component_diagonal = corrected`, targets 1e-12, cap 6000) run with `residual_norm = reference` and with `backward`.
  Rule: the reference normalisation is the cause of the non-convergence of the residual target iff the `reference`
  run does NOT meet the targets within 6000 iterations and the `backward` run does. If both meet them, the normalisation is
  not the cause; if neither meets them, another cause is present and is reported (with the parts at the end).
* **X7b (nozzle variants).** Baseline (res_u 2.619e-5 flat) and five single changes, 3000 iterations each, final
  `res_u` compared: (a) `momentum_rel_tol = 1e-8` and `momentum_max_iter = 2000` (H2), (b) `h_operator = previous_iterate`,
  (c) `convection = upwind`, (d) `alpha_u = 0.5`, (e) `pressure_rel_tol = 1e-8`. Rule per variant: it is implicated iff
  `res_u` changes by a factor >= 10 downwards; it is excluded iff the final `res_u` is within [1/3, 3] of the baseline.
* **X8 (pressure-equation residual in a closed domain).** The X6 closed cavity: the pressure-equation residual parts
  `(num, N_ref, S)` at 6000 iterations; the X2 rule is applied to them (the same thresholds).

## Addendum A2: third normalisation `scaled`, registered after the X1b / E2 / X8 results and before any use (no limit changed)

Reason. E2 with the registered mesh (X2/X4) is a round-off floor amplified by the reference normalisation
(`res_bwd` 6e-17, `N_ref/S` 1.4e-5); X1b shows that `backward` fixes the stall of a component whose exact value is
non-zero but not of a component whose exact value is zero (`uy`, and the pressure `p`, in the uniform-flow ducts), for which
every iterate-relative denominator is itself round-off. An iterate-independent common scale is needed:

* `ResidualNorm::Scaled` (`solver.residual_norm = "scaled"`): momentum `res_u = max_k num_k / max_j S_j` (the
  largest summed-term magnitude over the active components, common to all components); pressure
  `res_p = num_p / max(S_p, T)` with `T = 0.5 sum_f |phiHbyA_f|` (half the total absolute face flux, a throughput
  scale that does not vanish with the pressure level). `Reference` stays the default and bit-identical.
* Registered checks F1-F4 of this ADR use `residual_norm = scaled` (instead of `backward`; the sentence "backward ONLY IF X1/X2
  supports" is superseded by this addendum; limits, targets, caps and accuracy limits are unchanged). A diagnostic that the
  scaled residual is not weaker than meaningful: the F tests also assert the accuracy limits of ADR 0022, and the
  scaled residual of the cavity P1 run must satisfy `res_scaled <= 1e-3` at the iteration where the reference residual first
  drops below `1e-6` (the scaled number does not hide an unconverged state; reported for P1 only).

## Addendum A3: controller law v1 missed P1-P4 as registered; symmetric law `ser2` registered before its first run

Observation (first run of D1 as registered; limits untouched). With the law of D1 (`ser`) the scale ratchets to its cap: P1 (cavity,
start cfl 1) ends after 6000 iterations with residual 0.18 and the scale at 1000; P2 physical, P3 starts 0.5 and 10 and P4 (step,
res 8.9e-5 after 4000 iterations, scale in [1, 1000]) also miss. The fixed-step references converge (cfl 5 in 346, cfl 2
in 637, cfl 1 in 1148 iterations), so the controller, not the problem, is at fault. Diagnosis: the registered law grows the scale on
every iteration in which the residual falls and reduces it only when one iteration raises the residual by more than a factor 2. A
residual that oscillates with an amplitude below 2 therefore gains scale on every falling half-cycle and never loses it (a one-way
ratchet), whatever the mean trend; the published switched-evolution-relaxation rule is symmetric (the step follows the
inverse of the residual in both directions), which the D1 safeguard clause broke. The v1 tests are kept as ignored
`*_known_miss` records with their registered limits.

Registered now, before any run of it: `pseudo_control = "ser2"` (library `PseudoLaw::Symmetric`, otherwise the same controller
type and constants): `s <- clamp(s * clamp(r_{n-1} / r_n, backoff, growth), min_scale, max_scale)` with `growth = 2`,
`backoff = 0.5`, `min_scale = 1/64`, `max_scale = pseudo_scale_max` (1000); a non-finite `r_n` halves `s`; the first iteration
leaves `s = 1`. No other clause. Checks P1, P2, P3 (both), P4 and F1-F4 are repeated with `ser2` with the identical limits
(tests `*_ser2`); the F tests are run with both laws when practical, and only the `ser2` ones are expected to be informative. If
`ser2` also misses, the misses are recorded as `*_ser2_known_miss` and nothing is tuned (constants growth, backoff, scale
caps stay).

## Addendum A4: `ser2` also missed P1-P4; diagnosis experiment X9 registered before its run

Observation (first run of `ser2`, limits and constants untouched): P1 ends after 6000 iterations at residual 1.6e-2 with the
scale between 57 and 107 (max 107, final 57); P2 physical, P3 starts 0.5 (7.9e-3 after 3000) and 10 (9.0e-2 after 3500) and P4
(7.7e-5 after 4000, scale near its cap 988) miss. All are recorded as ignored `*_ser2_known_miss` tests. No constant is changed.

* **X9 (is there a stability limit of the underlying iteration below the scales the controller reaches?).** Fixed local pseudo-Courant
  numbers 5, 10, 20, 50, 100, 1000 on the P1 problem (no controller, 4000 iterations, tolerance 1e-6), and the same on the P4 step
  problem. Rule: the controller's limit cycle is explained by a stability limit of the fixed-step iteration iff the fixed runs
  converge for cfl <= 5 and fail to converge for every cfl >= the controller's final scale times the start cfl (57 for P1); the
  controller's residual-ratio law has no information about that limit (it grows the step whenever the residual falls). If the fixed
  runs converge up to cfl 100 the limit-cycle explanation is rejected.

## Addendum A5: X9 result, F1-F3 as registered missed (local pseudo mode from rest); corrected set-up F1b-F3b registered before its run

X9 (rule of addendum A4 applied to the measured numbers). Cavity (P1): fixed local cfl 5, 10, 20, 50 converge (346, 333, 458, 979
iterations), cfl 100 and 1000 do not (residual 9.0e-2, 0.19 after 4000): the stability limit of the fixed-step iteration lies
between cfl 50 and 100, which is where the `ser2` controller ends up (scale 57 to 107): the limit-cycle explanation holds for the
cavity (cfl 57 itself was not run; the nearest runs bracket it). Step (P4): fixed cfl 5 does NOT converge (4.9e-2), cfl 10 converges
(603), cfl 20, 50, 100, 1000 do not (4.2e-1, 2.9e-5, 3.5e-5, 3.1e-3): the first clause of the rule (converge for cfl <= 5) fails, so
the explanation is REJECTED for the step: its convergence is not monotone in the step (the wave-6 observation that Re_h = 400 converges only for
particular scheme/coupling combinations is of the same kind) and no residual-ratio law can find a "largest stable" step there.

F1, F2, F3 (C2, D1, D2 with `pseudo_mode = local`, `pseudo_cfl = 1`, controller on, `residual_norm = scaled`; plain `simplec`
otherwise) MISS as registered: F1 ends at residual 0.21 (res_p 3.2e-2) after 6000 iterations, F2 and F3 end with a non-finite
pressure residual. F4 (E2) is met (see results). Diagnosis (same as X5): at rest the local pseudo step has no convective rate,
`dt_c = cfl V / (0.5 sum|phi| + nu lap)` is set by the viscosity 1e-3 alone and is thousands of times the step that works
(0.2), so the first iteration over-shoots exactly like the plain run (u1 = 366 in X5); the controller reacts after the damage. The three
registered tests are kept as ignored `*_known_miss` records.

Corrected set-up (a set-up defect of the check, not of the limits), registered before it is run: F1b, F2b, F3b are F1-F3 with
`pseudo_mode = physical`, `pseudo_dt = 0.2` (the start step of ADR 0022 C2/D1/D2), `pseudo_control = ser2`,
`residual_norm = scaled`; limits, targets (1e-12), cap (6000 iterations) and accuracy limits unchanged. If a limit is missed it is
recorded as `f?b_known_miss`.

## Addendum A6: F1b-F3b missed (controller diverges); F1c-F3c (scaled norm, fixed physical step, no controller) registered

F1b, F2b, F3b (physical start 0.2 with the `ser2` controller) MISS: the run ends with non-finite fields (res_p NaN) in all three. A
trace of the first iterations of C2 (the only look taken before this paragraph; it covers four set-ups, 400 iterations each) shows
why: with `ser2` the scale doubles in the first iterations (the residual falls fast in the start-up transient), reaches about 31 in
12 iterations and the iteration blows up at about iteration 39 (the step 0.2 x 31 is beyond the stability limit of this
frictionless pressure-driven duct), the same ratchet-to-instability mechanism as for the cavity (addendum A4). The same trace shows
that with the fixed step 0.2 and `residual_norm = scaled` and NO controller the residual falls monotonically by about a factor 5
per 50 iterations (3e-8 at iteration 400) while the reference normalisation sits at 0.95. The three tests are kept as ignored
`f?b_*_known_miss` records.

Registered now, before the full runs: F1c, F2c, F3c = F1-F3 with `pseudo_mode = physical`, `pseudo_dt = 0.2`, no controller,
`residual_norm = scaled`, identical targets (1e-12), cap (6000 iterations) and accuracy limits. A miss becomes `f?c_*_known_miss`.
These pass or fail on the normalisation alone; they say nothing in favour of the controller.
