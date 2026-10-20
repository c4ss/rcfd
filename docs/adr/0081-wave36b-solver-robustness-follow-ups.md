# ADR 0081: Wave 36B, solver robustness follow-ups of ADR 0077 (a limited linear-upwind gradient, the sheared-mesh SIMPLEC divergence, NaN in the steady reports, a coupled-solver guard trip, a testable one-corrector lag warning)

Status: registered before any code of this wave was written. Everything under "Registered" was committed before the code and the runs it governs.
Limits are not changed, widened or deleted after a run; a miss becomes an ignored `*_known_miss` test with the limit intact, a defective set-up gets a
NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded. Clean-room: first principles and published literature only;
nothing here is taken from another CFD code or its documentation. Written at HEAD c89495f (scratch clone `<scratch dir>`).

Parents: ADR 0077 (wave 35B: steady divergence guard, part B diagnostics D0-D9 with D3 "Barth-Jespersen-limited gradient STABLE", finding B-X, the
`f64::max` finding, the C4 miss and the one-corrector restriction), ADR 0073 (B3/B4 lag model and its measured classifications), ADR 0069 (the stability
criteria of the transient cavity harness), ADR 0050 (SIMPLEC non-orthogonal flux compensation), ADR 0008 (MRF Taylor-Couette checks M2/M4).

## Disclosure: what was run before this registration (rule (d))

All smoke runs: scratch files not committed (`crates/rcfd-api/tests/w36b_smoke.rs`, `crates/rcfd-fv/tests/w36b_smoke_fv.rs`), release build of c89495f.

* SM-K Kovasznay set-up of part L (below) with the limiter OFF (`linear_upwind` unlimited) on all three meshes: converged in 91, 196, 501 iterations
  (res_u, res_p below 1e-10); L2 velocity error 4.105e-2, 9.580e-3, 2.204e-3 (orders 2.10, 2.12), L-infinity 1.801e-1, 4.788e-2, 1.214e-2 (orders 1.91,
  1.98); 2.5 s on the finest mesh. The limited runs were NOT smoke-run.
* SM-C the converging set of part L, unlimited: C-a cavity 32 x 32 converged in 394 iterations (0.8 s); C-b MRF Taylor-Couette across the non-conformal
  interface (3 120 cells) converged in 223 iterations (1.8 s); C-c `cases/axial_fan_sector` with `linear_upwind`, 300 iterations, no trip, res 1.2e-8,
  max speed 6.38 (20 s).
* SM-X the sheared-channel set-ups of part X, 20 iterations each (construction): S3 at 50 and 55 degrees (55: max speed 13 at iteration 10, 70 at 20, the
  ADR 0077 S3 behaviour); the x-periodic body-forced walled channel at 0 and 60 degrees and the doubly periodic Kolmogorov box (nu 0.5) at 0 and 60
  degrees build and iterate (max speed 1.17, 1.03, 1.14, 1.03 at iteration 20).
* SM-N on c89495f, fv level, the ADR 0077 S2 cavity (12 x 12, nu 1e-3, SIMPLE, upwind, alpha 1/1, unguarded): the fields are first non-finite at
  iteration 375, where `IterationInfo` reports `max_du = 0`, `continuity = 0`, `res_u = 0.971`, `res_p = NaN`; at 376 `res_u = 0` too. `SimpleSolver::run(3000,
  1e-6)` on the same set-up returns `converged = true` at iteration 375 with non-finite fields (its test is `max_du < tol && continuity < 1e-6`).
* SM-G coupled solver (`solver.algorithm = "coupled"`, lid cavity, guard off, alpha_u 1.0): no finite runaway found in ten set-ups (32 x 32: nu 1e-3, 1e-4
  central; 1e-4 `linear_upwind`; 1e-5 central, `quick`, `tvd_superbee`, `compressive`; 64 x 64 nu 1e-6 central; 32 x 32 nu 1e-6 `quick`; 12 x 12 nu 1e-4
  central): all stay below 1.7 times the lid speed for 300 to 500 iterations (several converge). A non-finite initial field (`velocity_expression_x =
  "1 [m^2/s] / (x - x)"` or `"sqrt(-1) * 1 [m/s]"`) and an inviscid cavity (nu = 0, central or `linear_upwind`) give NaN fields at iteration 1, where the
  coupled solver reports `continuity = 0` and `res_u = 0` (the part N defect in `coupled.rs`) and the guard trips `non_finite` on `res_p` at iteration 1.
* SM-P the part P set-ups were constructed and stepped once (PIMPLE-2 Euler Co_t 1 and a PISO one-corrector variant): finite. No ground-truth (t_end = 10)
  run was made before this registration.

## Registered

### Part L: opt-in Barth-Jespersen-limited gradient for `linear_upwind` (task 1)

Design (fixed now):
* Key `solver.linear_upwind_limiter`, enumeration `none` (default) / `barth_jespersen`. With `barth_jespersen` the steady SIMPLE/SIMPLEC momentum equation
  extrapolates the `linear_upwind` face value with the cell gradient multiplied by the Barth-Jespersen factor of that cell and that velocity component
  (`rcfd_fv::gradient::limiter`, per component: the bounds are the component's own cell and neighbour values, boundary values included). Only the
  convection face extrapolation is limited; the explicit non-orthogonal correction of the momentum Laplacian keeps the unlimited gradient (the D3 switch of
  ADR 0077; the D3 code itself was scratch code and is not available, so this is a re-implementation of the description in ADR 0077 amendment 2).
* Field `SimpleSolver::linear_upwind_limiter: Option<Limiter>` (default `None`); with `None` the code path is the existing one (bit identity by construction).
* Validation: a value other than `none` is a located error at `solver.linear_upwind_limiter` unless `solver.algorithm` is `simple` or `simplec` and
  `solver.convection` is `linear_upwind` (it is not silently ignored). Turbulence, scalar and energy equations are not limited (stated).
* Default stays `none`: the limiter moves every `linear_upwind` result, and the registered rule allows no baseline to move.

Checks (`crates/rcfd-api/tests/w36b_limiter.rs`):

| id | set-up | limit |
|----|--------|-------|
| L0a | the A4 set-up of ADR 0077 (`cavity-simple`, 60 iterations): key absent against `none` | u, p, phi bit-identical (`to_bits`) |
| L0b | `baseline_hashes.rs` | all registered hashes match |
| L0c | the key with `upwind`, with `algorithm = "pimple"`, with `algorithm = "coupled"`; an unknown value | each a located error at `solver.linear_upwind_limiter` |
| L1a | FAN-01 attempt-2 set-up (ADR 0077 `w35b_common::laminar`), `linear_upwind` + `barth_jespersen` | STABLE by the ADR 0077 part B criterion (300 iterations; no guard trip, max cell speed over 10..300 <= 3 x the largest boundary-face speed, continuity finite with its maximum over 200..300 not above its value at 10) |
| L1b | the same with SST (`w35b_common::sst`) | STABLE by the same criterion (never measured before: D3 was laminar) |
| L2a | C-a: cavity 32 x 32, nu 0.01, lid 1, SIMPLE 0.7/0.3, `linear_upwind` + limiter, cap 3000, tolerances 1e-6 | no trip; converged within the cap |
| L2b | C-b: the ADR 0008 M4 set-up (Taylor-Couette across the non-conformal interface, `annulus_3d_interface` 48 x 16 x 2 + 11 x 3, nu 0.1, MRF lower block, SIMPLEC, 1 non-orthogonal corrector; the largest aspect-non-orthogonality metric M = 18 725 of the ADR 0077 converging set) + limiter, cap 4000, tolerances 1e-10 | no trip; converged within the cap; max abs(u_theta - analytic) <= 2e-2 V_i (the ADR 0008 M4 limit, against the exact solution) |
| L2c | C-c: `cases/axial_fan_sector` with `linear_upwind` + limiter | STABLE by the L1 criterion |
| L2s | suite sweep (scratch instrumentation, not committed): one `cargo test --workspace --release --no-fail-fast` with an environment variable that switches the limiter on for every steady `simple`/`simplec` simulation that uses `linear_upwind`, logging every guard trip with the test name | zero trips outside the set-ups that trip without the limiter (the ADR 0077 A3a run-2 list: `cavity-simple` and its copies, the `eval` suite, the CLI `central` run); a trip outside that list is re-run once without the variable: a trip there too is not counted, otherwise it is a limiter failure. Test failures of that run (accuracy limits of other waves under a changed scheme) are counted and reported, not limits |
| L3 | order on a smooth case: Kovasznay flow, Re = 40 (lambda = Re/2 - sqrt(Re^2/4 + 4 pi^2) = -0.96374), exact velocity on all four sides and as the initial field, domain [-0.5, 1] x [-0.5, 1.5] (`box_2d` 1.5 x 2 with x' = x - 0.5, y' = y - 0.5), meshes 12 x 16, 24 x 32, 48 x 64, nu = 0.025, SIMPLE 0.7/0.3, tolerances 1e-10, cap 20000; error = volume-weighted RMS of the velocity-vector error against the exact solution at the cell centres | limited: observed L2 order on the fine pair (24 x 32 -> 48 x 64) >= 1.8. Reported (no limit): the coarse-pair order, the L-infinity orders, the unlimited errors (SM-K), the ratio limited/unlimited of the fine-mesh error, the iterations and final residuals. A limited run that does not reach 1e-10 within the cap is evaluated at the cap and reported as such |

Justification (rule (a)): L1/L2 are stability and convergence classifications; L2b's velocity limit is the registered ADR 0008 M4 limit; L3 is an order
limit against an exact solution. Stated before the run: the Kovasznay velocity components have no interior critical points (`du/dx = 0` needs
`cos(2 pi y) = 0` and `du/dy = 0` needs `sin(2 pi y) = 0`; the same for v), so this case does not exercise the limiter at smooth interior extrema, where
Barth-Jespersen is known to clip (recalled, UNVERIFIED); the clipping it does exercise is at the Dirichlet boundaries and in cells where the
extrapolation leaves the neighbour range. A miss of L3 is reported as "second order not retained on this case".

Decision rules: if L1a, L1b, L2a-L2c and L3 are met (L2s with zero limiter failures), the key is kept opt-in and documented. If any is missed the key is
REMOVED (rule (b) of wave 35), the tests stay as ignored `*_known_miss` records with their limits, and the miss is reported.

### Part X: SIMPLEC divergence on uniformly sheared meshes from 55 degrees (task 2, ADR 0077 finding B-X)

Base set-up B (the ADR 0077 S3 channel, `rcfd_fv` level): `channel` 40 x 10, length 4, height 1, points mapped x' = x + y tan(theta); inlet u = (1, 0),
walls no-slip, outlet zero-gradient velocity and p = 0; nu 0.01; SIMPLEC, alpha_u 0.9, alpha_p 1.0, `upwind`, no non-orthogonal correctors.
Geometry (stated): every face of this mesh has non-orthogonality theta (the centre-to-centre vector across the slanted faces is (dx, 0), across the
horizontal faces (dy tan theta, dy)); `lap_coeff` is the over-relaxed form `|S|^2 / (S . d)` = `|S| / (|d| cos theta)`; with zero correctors the
pressure equation and the corrected flux use only that part, while the cell velocity correction uses the full Green-Gauss pressure gradient.

Run classification (fixed now): 1000 iterations with a `SteadyGuard` observing; UNSTABLE iff the guard trips, or the largest cell speed over iterations
10..1000 exceeds 3 U_ref (U_ref = the largest boundary-face speed, 1; for the periodic variants the exact largest speed), or the continuity is non-finite,
or its maximum over iterations 900..1000 exceeds its value at iteration 10; otherwise STABLE. Reported: the iteration at which res_u and res_p first fall
below 1e-6. Angle set Theta = {45, 50, 52.5, 55, 57.5, 60, 65, 70, 75, 79} degrees. Threshold theta_c(variant) = the smallest angle of Theta classified
UNSTABLE ("> 79" if none); a non-monotone pattern (stable above an unstable angle) is reported.

Variants (each over Theta, one run per angle; `crates/rcfd-fv/tests/w36b_sheared.rs`, ignored diagnostic tests, output recorded here):
* V0 base B.
* V1 alpha_p in {0.7, 0.5, 0.3} at alpha_u 0.9 (SIMPLEC).
* V2 alpha_u in {0.8, 0.7, 0.5} at alpha_p 1.0 (SIMPLEC).
* V3 SIMPLE and SIMPLEC at the same relaxation 0.7/0.3.
* V4a 2 non-orthogonal correctors, compensation off; V4b 2 correctors with `simplec_non_orth_compensation = true` (ADR 0050 D1).
* V5 B at nu = 0.5 (cell Peclet 0.2 instead of 10).
* V6 x-periodic walled channel (`box_2d` 40 x 10, 4 x 1, periodic in x, sheared the same way), body force (0.12, 0) (exact u = 6 y (1 - y), largest 1.5;
  U_ref 1.5), nu 0.01; and V6' the same at nu 0.5 with body force (6, 0) (same exact profile).
* V7 doubly periodic Kolmogorov box (`box_2d` 40 x 10, 4 x 1, periodic in x and y, sheared; the y-pair is matched by the translation (tan theta, 1)),
  body force (nu k^2 sin(k y), 0), k = 2 pi, exact u = (sin(k y), 0), U_ref 1, nu 0.5 (Re_k = 1/(nu k) = 0.32, below the sqrt(2) linear stability bound
  of Kolmogorov flow, recalled, UNVERIFIED; at nu 0.01 the flow itself would be unstable, so no wall-free variant at the S3 Reynolds number is possible).
* V9 mode at 60 degrees: the converged state x* of the first of (V1 alpha_p 0.3, V2 alpha_u 0.5, V3 SIMPLE 0.7/0.3) that is STABLE at 60 degrees and
  reaches res_u, res_p <= 1e-8 within 5000 iterations; from x* the base settings (0.9/1.0) iterate 400 times. rho = geometric-mean growth of `max_du` per
  iteration over the window where `max_du` lies in [1e-9, 1e-2] (all 400 iterations if it never leaves it). Localisation of the increment field
  `u_k - u_{k-1}` at the window end: fractions of its squared norm in the wall rows (j = 0, 9), in the first and last two columns (inlet/outlet), and in
  the rest; checkerboard index of the pressure increment `|sum (-1)^(i+j) dp_ij| / sum |dp_ij|`. If no variant gives x*, V9 is not runnable (reported).

Hypotheses and reading rules (fixed now; the 10-degree and one-step shifts are decision thresholds of this protocol, not accuracy limits):

| id | hypothesis | stands iff |
|----|------------|------------|
| H-X1 | the SIMPLEC compensation `(rAtU - rAU)` is applied in compact form on the faces (orthogonal part only) but through the full cell gradient in the cells; on non-orthogonal faces the mismatch, weighted by rAtU/rAU - 1 (about alpha_u/(1 - alpha_u) for upwind momentum), drives the outer iteration unstable (the ADR 0050 mode) | theta_c(V4b) >= theta_c(V4a) + 10 deg AND theta_c(SIMPLE 0.7/0.3) > theta_c(SIMPLEC 0.7/0.3) |
| H-X2 | the pressure equation without its non-orthogonal part (0 correctors) corrects with an operator that is wrong by up to 1/cos^2 theta and nothing damps it at alpha_p = 1 | theta_c(V4a) >= theta_c(V0) + 10 deg AND theta_c(V1, alpha_p 0.5) >= theta_c(V0) + 10 deg |
| H-X3 | the slanted inlet/outlet faces (boundary cells whose centre-to-face vector is not normal) | necessary iff V6 is STABLE at every angle of Theta |
| H-X4 | the walls (non-orthogonal wall cells, zero-gradient wall pressure without correction) | necessary iff V7 is STABLE at every angle while V5 or V6' is UNSTABLE at some angle (if V5 and V6' are STABLE everywhere: not testable at low Re, reported) |
| H-X5 | convection (cell Peclet) | necessary iff V5 is STABLE at every angle |
| H-X6 | start-up only (the converged operator is stable) | V9: rho < 1 |

Decision rules: exactly the hypotheses whose rule says "stands" are reported as standing, the others as falsified (also if that leaves no explanation).
A code fix is made only if exactly one of H-X1, H-X2 stands, no existing key (relaxation, correctors, compensation) keeps theta_c above 79 degrees, and the
fix can be opt-in; its acceptance is registered in an amendment before it is written. Otherwise the outcome is documented: theta_c per variant, the keys
that raise it, and the standing hypotheses (`docs/case-schema.md`, near the non-orthogonal corrector keys). No limit of another part is derived from it.

### Part N: NaN in the steady `continuity` / `max_du` reports (task 3)

Design (fixed now): in `rcfd_fv::simple` and `rcfd_fv::coupled` the per-iteration maxima `max_du`, `continuity` (the cell imbalance fold) and `res_u` (the
component fold, both kernels; and the `Scaled` norm folds of `simple.rs`) use a NaN-propagating maximum (`if a.is_nan() || b.is_nan() { NaN } else
{ a.max(b) }`; `f64::maximum` is not in MSRV 1.75). Bit-identical for finite values (the result of `max` of two finite numbers is unchanged). The geometric
`area_scale` fold is not changed. The steady guard's K1 scans the fields (velocity, pressure, flux) BEFORE the reported residuals, so that a NaN state is
still reported with its first bad cell (with the fix the reported continuity of such a state is NaN, which K1 would otherwise report first, without a
cell); the trip kind and iteration are unchanged.

Checks (`crates/rcfd-fv/tests/w36b_nan_reports.rs`, `crates/rcfd-api/tests/w36b_coupled_guard.rs`):

| id | set-up | limit |
|----|--------|-------|
| N1 | SM-N set-up, unguarded, iterated to the first iteration whose fields are non-finite (375 on c89495f) | at that iteration `max_du`, `continuity` and `res_u` are NaN (all three); the iteration number is reported |
| N2 | SM-N set-up, `SimpleSolver::run(3000, 1e-6)` | `converged == false` (it returned true at 375 on c89495f) |
| N3 | coupled solver, cavity 12 x 12, nu 0.01, one cell velocity set to NaN before the first iteration | the reported `max_du`, `continuity` and `res_u` of that iteration are NaN |
| N4 | the A1b guard of ADR 0077 on the SM-N set-up (guard on) and a K1 trip on a NaN state | the A1b limits unchanged (trip before the twin's NaN iteration); a `non_finite` trip on a NaN velocity field reports quantity `velocity` and a cell |
| N5 | `baseline_hashes.rs` and the full suite | all registered hashes match; no other test changes outcome |

### Part G: a test that makes the steady guard trip on the coupled solver (task 4)

SM-G found no finite runaway of the coupled solver. Registered: G1 is the fallback named before this registration was written, a non-finite initial
field; G2 is the natural non-finite state of the inviscid cavity.
* G1: `solver.algorithm = "coupled"`, cavity 12 x 12, nu 0.01, `[initial] velocity_expression_x = "1 [m^2/s] / (x - x)"`, through `runctl::drive`: state
  `diverged`, exit code 6, reason contains `steady divergence guard`, `finish.divergence.solver = "steady"`, `kind = "non_finite"`, `iteration = 1`,
  `quantity = "velocity"` with a cell (after the part N K1 reordering).
* G2: coupled cavity 32 x 32 with nu = 0 (accepted by the schema), `linear_upwind`, through `runctl::drive`: state `diverged`, exit code 6,
  `divergence.kind = "non_finite"` at iteration 1. Reported: the cause of the NaN (not diagnosed in this wave unless it is cheap).
* G0 (reported): the SM-G search for a finite runaway of the coupled solver, with its ten set-ups; K2-K4 on the coupled solver remain untested by a
  diverging run (stated).

### Part P: the one-corrector pressure-lag warning against known-stable and known-unstable set-ups (task 5)

Set-ups (tree form of ADR 0077 C2: cavity 32 x 32, nu 0.01, lid 1, `linear_upwind`, `pimple`, `n_correctors = 1`, alpha_u 0.8, alpha_p 0.5; dt = Co_t /
32), with their earlier classification on the ADR 0069 harness:

| id | n_outer | scheme | Co_t | earlier classification (source) |
|----|---------|--------|------|---------------------------------|
| P1 | 2 | euler | 1.0 | STABLE (ADR 0069 required set, measured there) |
| P2 | 2 | euler | 2.3 | STABLE, model G 0.71 (ADR 0073 B3, measured) |
| P3 | 4 | bdf2 | 6.0 | STABLE, G 0.69 (B3) |
| P4 | 2 | euler | 3.0 | UNSTABLE, G 1.42 (B3) |
| P5 | 4 | euler | 5.0 | UNSTABLE, G 1.55 (B3) |
| P6 | 4 | bdf2 | 7.5 | UNSTABLE, G 1.55 (B3) |

Re 1000 and the high-Courant convective instability (the ADR 0073 B4b unwarned runs) are not included: the check does not claim them.

Checks (`crates/rcfd-api/tests/w36b_lag_known.rs`):
* P-gt (ground truth, in the same test): each set-up with `pressure_lag_check = "off"`, stepped to t_end = 10, classified by the ADR 0069 criteria:
  STABLE iff (a) no divergence-guard trip and every field finite, (b) max abs(u) <= 1.5 over all cells and steps, (c) max abs(p) <= 50 over all cells and
  steps, (d) the reported continuity of every step <= 1e-3, (e) t_end reached.
* P-a: every P-gt classification equals the earlier one in the table. A difference is a set-up defect of the tree form (reported); P-b then uses P-gt.
* P-b: with the default key (`warn`) and `runctl::drive` over 3 steps, exactly one `pressure_lag` warning (at step 1 for Euler, step 2 for BDF2) iff the
  set-up is UNSTABLE by P-gt; no warning iff STABLE. All six.
* C4'' (reported): the accounting of ADR 0077 C4' extended by these six one-corrector runs: warnings on the STABLE ones are false positives. This is what
  makes C4 non-vacuous, and only for these six set-ups (no existing validation case of the suite uses one corrector).

### Process

Own scratch clone and target (`<target dir>`), commits of own hunks only (temporary index), one cargo at a time, two test threads.
Documentation: `docs/case-schema.md` (the new key, the part X outcome), `docs/milestone-fan.md` (robustness row). Final gate on a fresh `git archive`
of the HEAD at hand-back: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast`.
