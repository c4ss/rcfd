# ADR 0094: Wave 39C, partials close-out: coarse-start backward-facing step (F-205), solved flat-plate y+ (F-108), uniform step count on Taylor-Green (F-209)

Status: registered before the three comparison runs below. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss`
test with the limit unchanged, a defective set-up gets a new corrected test and the old one stays ignored. No library code
changes: test files only, so defaults and `crates/rcfd-api/tests/baseline_hashes.rs` are untouched. No roadmap acceptance text is
edited; rewordings are proposals for the user. Clean-room: every reference value here is rcfd's own solution or a closed form
derived below.

Sources of the gaps: docs/audit/partials-group-5.md (F-205), partials-group-4.md (F-108), partials-group-9.md (F-209). I re-read
the cited tests before building on the audit's claims: `rcfd-fv/tests/init_potential.rs` (I2), `rcfd-api/tests/init_methods.rs`
(I1, I3), `rcfd-api/src/init.rs` and `sim.rs` (the tree's coarse transfer), `rcfd-turb/tests/flat_plate_sst.rs`,
`rcfd-fv/tests/adaptive_step.rs` (S1/S3).

## What was run before this ADR was written (disclosed)

The test code of C1, Y1 and T1 below was written first. Only these runs were made, release build, HEAD 3143c71 plus these files:

* `t1_uniform_step_setup_smoke` (adaptive_step.rs, ignored): a 1-step uniform run to t = 10 (dt = 10) finishes with a finite
  field (0.013 s); 10 steps at the aimed step dt = 0.0892 finish finite (0.057 s, about 6 ms per step).
* `c1_coarse_start_setup_smoke` (w39c_coarse_step.rs, ignored): coarse step mesh 336 cells, fine 1344; after 5 coarse
  iterations the interpolated velocity and pressure are finite and the first fine iteration gives res_u 9.281e-2, res_p 3.105e-1.
* Y1 needs no new set-up: it reads the converged run that `sst_flat_plate_cf` already makes. Its outcome is known in advance:
  the audit re-ran that test and printed first-cell y+ 0.54, 0.53, 0.51, 0.50, 0.48 at the five stations below. Y1 is
  registered with that result in hand; it adds a numeric assertion, not a new measurement.
* T1's outcome is predicted below from a closed form, before the run.

## C1 (F-205): coarse-level start of the backward-facing step

Acceptance: "Potential-flow initialisation around a circular cylinder reproduces the analytic surface velocity distribution
within 3 percent; an initialised backward-facing-step case converges in fewer iterations than uniform initial values". I1
(cylinder, 9.7e-4 of 2A) and I3 (cavity coarse start, 112 against 165) are met (ADR 0019). I2 (the potential start on the step)
missed, 258 against 253 and 258. The acceptance says "an initialised" case, so a coarse-level start counts.

Set-up (`crates/rcfd-api/tests/w39c_coarse_step.rs`, test `c1_coarse_start_needs_fewer_iterations_on_the_backward_facing_step`).
The fine case is I2's, unchanged: `backward_step_graded`, `l_in = 2`, `l_out = 20`, `h_in = step = 1`, depth 0.1,
`n = [8, 80, 8, 8]`, uniform grading, parabolic inlet (mean 1), nu = 0.01 (Re_h = 100), `LinearUpwind`, SIMPLE with default
relaxation, `h_from_solution`. Stop when `max(res_u, res_p) < 1e-6`, cap 3000. Starts:

* A: uniform interior velocity (1, 0, 0), p = 0.
* B: zero velocity, p = 0.
* C: coarse start. The same case on `n = [4, 40, 4, 4]` from (1, 0, 0), run until `max(res_u, res_p) < 1e-6` or 300
  iterations. That is the tree's rule: `initial.coarse_iterations = 300` as in I3, and `Simulation::run` stops when the run is
  done. Velocity and pressure go to the fine mesh with `rcfd_post::interpolate::Interpolator` in `InterpMode::NonConservative`,
  then `SimpleSolver::set_fields`, which rebuilds the face flux and resets the counter. This is the code path of
  `initial.method = "coarse"` in `sim.rs`. The tree has no backward-step generator (`init::COARSENABLE`), so C1 is a library
  copy of that transfer rather than a case file.

Counting: the limit counts fine iterations only, as I3 does, because the acceptance counts the iterations of the case.
`n_fine + n_coarse / 4` (a coarse iteration costs about a quarter of a fine one) is reported, not limited.

Limit: `iterations(C) < iterations(A)` and `iterations(C) < iterations(B)`, all three converged.

Set-up fidelity: A and B are the same code and settings as I2. If they do not reproduce I2's 253 and 258, the set-up is
defective and C1 is not a result.

Verdict rule: F-205 becomes `done` if C1 passes, since I1 and I3 are met and I2 is a potential-start check that stays an
ignored known miss. This replaces ADR 0019's verdict rule only for the step clause: the acceptance asks for an initialised
case, not a potential-initialised one. If C1 misses, F-205 stays `partial` and the test becomes
`c1_..._known_miss`.

## Y1 (F-108): solved first-cell y+ on the flat plate

Acceptance: "Flat-plate mesh built from a target y+ meets it within 10 percent on the solved case; layer quality metrics
reported; works on block meshes and on unstructured wall patches".

Set-up: the existing `flat_plate_sst.rs` run, unchanged. SST, Re 5e6 per metre, plate 0..2, `Grading::WallLayers` (70 layers,
growth 1.1, ny 110, nx 140), first height from target y+ 0.5 with White's u_tau at x = 2. Here u_tau is smallest, so y+ rises
upstream; upstream of x = 0.4 it is not checked. First-cell y+ = u_tau y_c / nu with tau = (nu + nu_t,w) u_c / y_c, as the file
already computes it.

Limit (new test `sst_flat_plate_first_cell_y_plus_meets_the_target`): at x = 0.4, 0.6, 1.0, 1.4, 1.99 (interpolated along the
wall), `|y+ / 0.5 - 1| <= 0.10`, on a run that meets the file's convergence test (du < 1e-9, d nu_t < 1e-6).

Gating: the run takes about 131 s in release. Both `sst_flat_plate_cf` and Y1 move from `#[ignore]` to
`#[cfg_attr(debug_assertions, ignore)]`, so they run in the release gate and are skipped in debug builds. They share one run
through the existing `OnceLock`. Y1 is its own test, so a failure of the F-043 Cf checks does not hide or block it. Registered
rule: if `sst_flat_plate_cf` fails its F-043 Cf limits on this run, it goes back to plain `#[ignore]` with the measured values
in the reason, and Y1 stands alone.

Verdict: F-108 stays `partial` whatever Y1 gives. The "unstructured wall patches" clause is not met as a solved case, because
`inflate_patch` builds an outward shell that is not merged into a fluid domain. That waits on F-234 and is out of scope here.

## T1 (F-209): smallest uniform step count on Taylor-Green

Acceptance: "Linear transient decay and Taylor-Green vortex reach a target error in fewer steps than fixed-CFL stepping; a
high-aspect-ratio flat-plate boundary-layer mesh converges to Blasius and turbulent skin-friction values within the
unaccelerated tolerance".

Gap (audit, re-checked in `adaptive_step.rs`): S3 compares the Courant-controlled run (74 steps, L2 1.69e-2 against the
reference) with `n_fixed = ceil(t_end / dt_aim) = 113`, but never runs the fixed-step case or computes its error.

Set-up (new test `t1_courant_control_needs_fewer_steps_than_the_smallest_uniform_count_meeting_the_target`). The S1/S3 run is
unchanged: Taylor-Green, 32 x 32, periodic 2 pi box, nu = 0.05, t_end = 10, PISO, central convection, implicit Euler, Courant
target 0.5, start step 0.4 dt_aim. The reference is unchanged: BDF2, dt = 1/80, same mesh. The uniform runs use the same solver
with N steps of exactly dt = t_end / N (a plain loop, so no shortened last step). Their error is the same relative L2
difference to the reference. Search: N = 1, 2, 3, ... until the error is <= 0.02. No monotonicity is assumed, so the first N
found is the smallest. The search goes on to the smallest N with error <= the adaptive run's own error (equal-error count,
reported only). Cap N = 1000. The test runs in release and is ignored in debug (about 10^4 steps at about 6 ms).

Limit: adaptive error <= 0.02, and `n_adapt < N_min(0.02)`.

Reading of "fixed-CFL stepping": the audit and the wave task read it as uniform (fixed) steps, and T1 measures that. The
Courant controller is itself fixed-CFL stepping, so "adaptive vs fixed-CFL" is not a meaningful contrast for it. T1 measures
Courant control against uniform steps.

Prediction, before the run. Taylor-Green is one Fourier mode. Its velocity decays as exp(-2 nu t), and over the run
2 nu t_end = 1. Implicit Euler multiplies the amplitude by (1 + 2 nu dt)^-1 per step, so N uniform steps give
(1 + 1/N)^-N against e^-1, a relative error of 2.04 % at N = 24 and 1.96 % at N = 25. The discrete mode on 32 x 32 decays at a
rate within about 0.3 % of 2 nu (5-point Laplacian eigenvalue, h = 2 pi / 32), and the nonlinear term only feeds the pressure. So N_min(0.02) should be about 25, well below
the adaptive 74. T1 is expected to MISS. A Courant controller lengthens the step as the flow slows, which does not buy accuracy
in a single-mode decay. This is the same ill-posedness the audit found for the single-exponential linear-decay clause. The limit
is registered anyway, as the task asks, and a miss becomes `t1_..._known_miss` with the limit unchanged.

Verdict: F-209 stays `partial` whatever T1 gives. The linear-decay clause is a known miss (ADR 0023), and the flat-plate clause,
residual smoothing, local timescale factors and stretched-mesh acceleration are not delivered.

## Proposal to the user (not applied): F-209 acceptance rewording

The current text asks a single-timescale problem to show an adaptive-step gain. For one exponential mode (linear decay, or the
single-mode Taylor-Green vortex), a relative-error controller gives a constant step, and a Courant controller gives a
non-optimal step. Neither can beat the best uniform step count by a registered margin; the 81 vs 76 and 46 vs 36 misses of
ADR 0023 and T1 here are that fact. Proposed wording for the first clause:

"Error-controlled stepping (step doubling, `solver.error_tolerance`) on a two-timescale linear decay (rates differing by at
least 100, both modes of unit initial amplitude) reaches a final error E in fewer steps than the smallest uniform step count
with error <= E; on the Taylor-Green vortex the error-controlled global error scales with the tolerance
(E / (tol T) within [0.3, 3])."

The Taylor-Green half of that is already measured by S4 (E/(tol T) 0.87 to 0.90). The two-timescale half would be a new
registered test. The flat-plate clause is unchanged.

## Results

Addendum written after the runs: release build, HEAD d04301f plus the test files. Each registered test was run once.

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| C1 fidelity: A and B reproduce I2 | uniform (1,0,0) 253, zero 258 | 253, 258 | reproduced |
| C1 coarse start, fine iterations | 220 (coarse run: 137 iterations, converged to 1e-6) | < 253 and < 258, all converged | met |
| C1 equivalent work `220 + 137/4` | 254 against 253 | reported only | - |
| Y1 first-cell y+ at x = 0.4, 0.6, 1.0, 1.4, 1.99 | 0.5442, 0.5275, 0.5078, 0.4956, 0.4833 (worst +8.8 %) | within 10 % of 0.5 | met |
| Y1 run convergence | 1989 iterations, du 9.95e-10, d nu_t 2.63e-8 | du < 1e-9, d nu_t < 1e-6 | met |
| `sst_flat_plate_cf` (F-043 limits, now in the release gate) | Cf(0.97) 0.002663: -0.7 % TMR page, -1.0 % TMR file; power law -3.8 .. +0.6 % for x <= 1 | 5 % | met, stays un-ignored in release |
| T1 adaptive error | 1.6926e-2 | <= 0.02 | met |
| T1 `n_adapt < N_min(0.02)` | 74 against N_min = 64 (error 1.9675e-2) | 74 < 64 | MISS |
| T1 equal-error count | N = 72 meets the adaptive error 1.69e-2 | reported only | - |

Wall time: C1 2.4 s, T1 18.5 s, the plate pair 143 s (one shared run).

C1 passes, so F-205 is `done` under the verdict rule. The gain in fine iterations is real: 33 fewer than uniform, 13 %. Counted
as total work it is break-even: 254 against 253. I3 on the cavity has the same character (187 against 165 in work, 112 against
165 in fine iterations). The acceptance counts the iterations of the case, and that is what is limited.

The T1 miss is kept as `t1_courant_control_needs_fewer_steps_than_the_smallest_uniform_count_meeting_the_target_known_miss`
(ignored, limit unchanged). The prediction was wrong in size but right in direction. I predicted N_min of about 25 from the
amplitude factor of implicit Euler on one viscous mode; the measured N_min is 64. So the uniform-step error at a given N is
about 2.6 times what amplitude decay alone gives. This was not diagnosed. Likely sources, untested, are the PISO splitting
error and a phase or shape error that the L2 difference counts and the amplitude estimate does not. Even at equal error, the
Courant-controlled run needs more steps than uniform stepping (74 against 72). So the miss does not come from the 0.02 target
sitting between the two errors.

Statuses: F-205 `done`; F-108 `partial` (the unstructured-wall-patch clause waits on F-234); F-209 `partial` (T1 missed, plus
the open clauses listed above, and the rewording proposal is with the user).

Deviation from the plan: none in set-up or limits. The T1 test carries `#[cfg_attr(debug_assertions, ignore)]` as registered
until the miss, and a plain `#[ignore]` with the known-miss reason after it.
