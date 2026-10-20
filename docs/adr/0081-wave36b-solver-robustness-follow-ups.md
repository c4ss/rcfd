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

## Amendment 1 (2026-10-20): N1 set-up defect; written after the first run of N1-N4, before the corrected N1 test ran

Run on the scratch tree with the part N code (NaN-propagating folds, K1 fields first): N2, N3, N4 met (N2 `converged = false` after 3000 iterations; N3
`max_du`, `continuity`, `res_u` NaN; N4 trip `non_finite`, quantity `velocity`, cell 0, iteration 375). N1 as registered failed: at iteration 375 (the
first iteration that ends with non-finite fields) `max_du` and `continuity` are NaN, but `res_u = 0.971` is finite. That is not a reporting defect:
`res_u` is the normalised momentum residual of the iteration's STARTING state (assembled before the solve), which is still finite at 375; the fields become
non-finite inside that iteration (the pressure solve, `res_p` NaN). The registered N1 limit asked for a NaN residual of a finite state: a defective limit.
The registered test stays as the ignored `n1_registered_setup_defect` (limit unchanged). Corrected N1' (new test `n1_corrected_reports_are_nan_from_the_
first_non_finite_state`): at the first iteration whose fields end non-finite, `max_du` and `continuity` are NaN; at the next iteration (whose starting
state is non-finite) `max_du`, `continuity` and `res_u` are all NaN (on c89495f that iteration, 376, reported 0, 0, 0; SM-N).

## Amendment 2 (2026-10-20): part X controls without shear; written after the registered scan and V9 ran once, before the runs below

What I knew when writing this (the registered runs, once each; `w36b_sheared.rs` `part_x_scan`, `part_x_v9_mode_at_60_degrees`):
* theta_c: V0 50 (the 50 and 52.5 degree runs overshoot to 6.6 and 14 times U and then converge; by the registered criterion they are UNSTABLE);
  V1 alpha_p 0.7: 60, 0.5: 70, 0.3: > 79; V2 alpha_u 0.8, 0.7, 0.5 (alpha_p 1.0): 45 for all three; V3 SIMPLE 0.7/0.3: 45, SIMPLEC 0.7/0.3: 75;
  V4a: 45; V4b: 70; V5: 45; V6: 45; V6': 45; V7: 60 (STABLE 45 to 57.5).
* V9: from the SIMPLEC 0.9/0.3 state converged at 60 degrees (res 1e-8 at 537), the base iteration's increments decay from 4.1e-12 to 3.1e-15 in 400
  iterations, rho = 0.982 (the increments never entered the registered window [1e-9, 1e-2]; rho is over all 400 iterations); increment field 90 %
  interior, 7.5 % wall rows, 3 % inlet/outlet columns; no pressure checkerboard (index 0.001).
* By the registered rules: H-X1 not standing (V4b 70 >= V4a 45 + 10, but SIMPLE 45 is not above SIMPLEC 75); H-X2 not standing (V4a 45 < V0 50 + 10);
  H-X3, H-X4, H-X5 not necessary (V6, V7, V5 UNSTABLE somewhere); H-X6 stands (rho < 1).

Defect found in my registration (rule (d)): a variant whose threshold is 45 (the smallest scanned angle) may be unstable WITHOUT shear, and then its
theta_c says nothing about the non-orthogonality. No variant was run at 0 degrees. Readings that rest on such variants: H-X1 (SIMPLE 0.7/0.3), H-X3 (V6),
H-X4 (V5, V6'), H-X5 (V5). V4a at 0 degrees is the V0 run (the correctors do nothing on an orthogonal mesh), so H-X2 is not affected; V2 enters no reading.

Registered now (one run each, same classification): C0 controls at theta = 0, 15, 30 degrees for V2 (all three), V3 SIMPLE 0.7/0.3, V4a, V5, V6, V6' and,
as a reference, V0 and V7. Reading: a variant UNSTABLE at 0 degrees is "unstable without shear" and every hypothesis reading that uses it becomes NOT TESTABLE
(it is neither standing nor falsified); a variant STABLE at 0 degrees keeps its registered reading. V9b (confirmation, reported, no reading): V9 repeated
with the converged state perturbed by 1e-6 (u_x of cell c multiplied by 1 + 1e-6 sin(c)), rho over the iterations where `max_du` lies in [1e-9, 1e-2].
Decision rule unchanged (no code fix unless exactly one of H-X1, H-X2 stands).

## Addendum (measured values, misses, deviations)

Release builds, scratch tree on top of 5ff3b7c (rebased onto 413e09e before the gate; the rebase touched no file of this wave except the two docs).
One run per registered set-up unless stated. Limits are the registered ones (with amendments 1 and 2), unchanged.

Outcome in one paragraph. (1) `solver.linear_upwind_limiter = "barth_jespersen"` (opt-in, default `none`) makes `linear_upwind` stable on the FAN-01
mesh, laminar and SST, keeps every converging case checked converging (also with the limiter forced on in all 542 steady `linear_upwind` simulations
of the suite: no new guard trip), and keeps second order on the Kovasznay flow (fine-pair L2 order 2.17; fine-mesh error 0.6 % above the unlimited
scheme): every limit met, the key is kept. (2) The sheared-mesh SIMPLEC divergence needs no wall, no open boundary and no convection (a doubly periodic
box at cell Peclet 0.2 diverges from 60 degrees). The two defect hypotheses (SIMPLEC compact/wide compensation, omitted non-orthogonal pressure part) are
falsified by their registered rules, and the converged state at 60 degrees is stable under the same iteration: the only standing hypothesis is "start-up
only" (H-X6). By the registered rule no code fix; the measured thresholds and the remedy (lower `alpha_p`) are documented. The mechanism of the start-up
growth itself is not identified. (3) The steady `max_du`, `continuity` and `res_u` reports keep NaN; `SimpleSolver::run` no longer reports a NaN state as
converged (it did, at iteration 375 of the S2 cavity); a NaN trip of the guard names its cell. (4) Two tests make the guard trip on the coupled solver
through the run-control driver; both are non-finite (K1) trips: no finite runaway of the coupled solver was found, so K2-K4 stay untested there. (5) The
one-corrector lag warning was checked against six set-ups whose stability was re-measured to t_end = 10: it warned exactly on the three unstable ones and
was silent on the three stable ones.

### Part L

| Check | Measured | Limit | Result |
|---|---|---|---|
| L0a key absent vs `none` (`cavity-simple`, 60 iterations) | bit-identical | bit-identical | pass |
| L0b `baseline_hashes.rs` | all registered hashes match (targeted run and the gate) | unchanged | pass |
| L0c `upwind`, `pimple`, `coupled`, unknown value | each a located error at `solver.linear_upwind_limiter` | located | pass |
| L1a FAN-01 laminar + limiter | STABLE: max speed 10..300 51.0 against 3 x 40.03; continuity 2.93e-4 at 10, max 2.74e-4 over 200..300; res_u 9.7e-3 at 300 (28 s) | STABLE | pass |
| L1b FAN-01 SST + limiter | STABLE: max speed 49.9; continuity 1.56e-4 at 10, max 5.5e-5 over 200..300; res_u 1.8e-3 (33 s) | STABLE | pass |
| L2a cavity 32 x 32 + limiter | converged in 397 iterations (unlimited 394), no trip | converged, no trip | pass |
| L2b MRF Taylor-Couette across the interface + limiter | converged in 223 iterations (unlimited 223), max abs(u_theta - analytic) 7.92e-3 V_i | converged; <= 2e-2 V_i | pass |
| L2c `cases/axial_fan_sector` + `linear_upwind` + limiter | STABLE: max speed 6.37 against 3 x 6.2; continuity 1.3e-4 at 10, 6.4e-9 over 200..300 | STABLE | pass |
| L2s suite sweep, limiter forced on | 542 steady `linear_upwind` simulations logged; 17 trips, all on the ADR 0077 diverging set-ups (`cavity-simple` and its copies in `baseline_hashes`, `gpu_case` c2/d2, `particles_case` pt14, `particles_two_way` pl8, ADR 0077 A4, this wave's L0a, all `continuity_growth` at 12 or 13; the `eval` suite at 5); zero limiter failures. Suite: 1863 passed, 6 failed, 514 ignored; the 6 failures are tests whose expected values depend on the unlimited scheme (`baseline_hashes` and `particles_case` pt14 stored hashes; `coupled` coupled-vs-SIMPLEC comparison 1.16e-2 above its tolerance with only one side limited; ADR 0077 A2a, A2b, S-d: FAN-01 `linear_upwind` no longer diverges) | zero limiter failures | pass |
| L3 Kovasznay, limited | L2 4.201e-2, 9.999e-3, 2.217e-3 (orders 2.07, **2.17**); L-inf orders 1.83, 2.08; converged in 89, 197, 499 iterations | fine-pair order >= 1.8 | pass |
| L3 reported | unlimited L2 4.105e-2, 9.580e-3, 2.204e-3 (orders 2.10, 2.12; L-inf 1.91, 1.98); fine-mesh ratio limited/unlimited 1.006 | - | - |

Decision (registered): every limit met, the key is kept opt-in. The re-implementation reproduces the D3 measurement of ADR 0077 (max speed 51.0 on
FAN-01 laminar). The steady-guard note for an unlimited `linear_upwind` run now also names the limiter as a remedy.

### Part X (registered scan, amendment 2 controls; thresholds theta_c in degrees)

| Variant | theta_c | 0 / 15 / 30 deg control |
|---|---|---|
| V0 base SIMPLEC 0.9/1.0 | 50 (50 and 52.5 overshoot to 6.6 and 14 U, then converge; diverges from 55) | S / S / S |
| V1 alpha_p 0.7 / 0.5 / 0.3 | 60 / 70 / > 79 | - |
| V2 alpha_u 0.8 / 0.7 / 0.5 (alpha_p 1.0) | 45 / 45 / 45 | all S |
| V3 SIMPLE 0.7/0.3; SIMPLEC 0.7/0.3 | 45; 75 | SIMPLE: S / S / S |
| V4a 2 correctors without compensation | 45 | S / **U** / U (unstable from 15 degrees) |
| V4b 2 correctors with compensation | 70 | - |
| V5 base at nu 0.5 (cell Peclet 0.2) | 45 | S / S / S |
| V6 x-periodic walled channel, nu 0.01; V6' nu 0.5 | 45; 45 | S / S / S both |
| V7 doubly periodic Kolmogorov box, nu 0.5 (no walls, no open boundary) | 60 (STABLE 45 to 57.5) | S / S / S |
| V9 mode at 60 deg from the SIMPLEC 0.9/0.3 converged state | increments 4.1e-12 -> 3.1e-15 in 400 iterations, rho 0.982; 90 % interior, 7.5 % wall rows, 3 % inlet/outlet columns; no checkerboard | - |
| V9b (amendment 2) the same perturbed by 1e-6 | rho 0.889 over iterations 1..60 (1.2e-6 -> 1.2e-9); 47 % interior, 43 % inlet/outlet columns, 10 % wall rows | - |

Readings (registered rules; every variant used by a reading is STABLE at 0 degrees, so none becomes "not testable"):
* H-X1 (SIMPLEC compact/wide compensation): NOT standing. V4b 70 >= V4a 45 + 10 holds, but SIMPLE 0.7/0.3 (45) is not above SIMPLEC 0.7/0.3 (75).
* H-X2 (omitted non-orthogonal pressure part, alpha_p 1): NOT standing. V4a 45 < V0 50 + 10 (two correctors make it worse); V1 alpha_p 0.5 (70) alone
  would have met its clause.
* H-X3 (inlet/outlet faces): not necessary (V6 diverges without them).
* H-X4 (walls): not necessary (V7 diverges without walls, from 60 degrees).
* H-X5 (convection): not necessary (V5 at cell Peclet 0.2 diverges from 45 degrees).
* H-X6 (start-up only): STANDS (rho 0.982 < 1; V9b 0.889).

Decision (registered): no code fix (neither H-X1 nor H-X2 stands). Documented in `docs/case-schema.md` (SIMPLEC on uniformly sheared meshes).
Observations, no verdict: lowering `alpha_u` at `alpha_p = 1`, or SIMPLE 0.7/0.3, makes the channel worse, not better; lowering `alpha_p` is the
effective remedy; two correctors without the ADR 0050 compensation are unstable from 15 degrees (consistent with the ADR 0050 mode). What amplifies the
start-up transient on these meshes is not identified: the linearised iteration at the converged state contracts, so it is a finite-amplitude or
transient-growth effect (not tested further). A start-up procedure (an alpha_p ramp) was not tested.

### Part N

| Check | Measured | Limit | Result |
|---|---|---|---|
| N1 registered (res_u NaN at the first non-finite iteration) | 375: `max_du` NaN, `continuity` NaN, `res_u` 0.971 (residual of the finite starting state) | all three NaN | set-up defect (amendment 1); ignored `n1_registered_setup_defect` |
| N1' corrected | 375: `max_du`, `continuity` NaN; 376: `max_du`, `continuity`, `res_u` NaN (c89495f reported 0, 0, 0) | as amended | pass |
| N2 `SimpleSolver::run(3000, 1e-6)` | `converged = false` after 3000 iterations (c89495f: `true` at 375 with NaN fields) | false | pass |
| N3 coupled, one NaN cell velocity | `max_du`, `continuity`, `res_u` NaN at iteration 1 | NaN | pass |
| N4 K1 trip on the NaN state | `non_finite`, quantity `velocity`, cell 0, iteration 375; ADR 0077 A1a-A1c unchanged | located | pass |
| N5 `baseline_hashes.rs`, suite | hashes match; suite: see the final gate | unchanged | pass |

### Part G

| Check | Measured | Limit | Result |
|---|---|---|---|
| G0 search for a finite runaway (SM-G, ten set-ups) | none; the coupled solver stays bounded at alpha_u 1 with central, QUICK, superbee and compressive down to nu 1e-6 | reported | K2-K4 untested on the coupled solver |
| G1 non-finite initial field, `runctl::drive` | `diverged`, exit 6, iteration 1, `non_finite`, quantity `velocity`, cell 0 with position, `solver = "steady"` | as registered | pass |
| G2 inviscid coupled cavity (nu = 0, `linear_upwind`) | `diverged`, exit 6, iteration 1, `non_finite` (`velocity`, cell 0) | as registered | pass |

Not diagnosed: why nu = 0 gives NaN at the first coupled iteration (the schema accepts nu = 0; the fluid starts at rest, so the interior momentum rows
have no convective or viscous diagonal: a plausible, unchecked cause). The guard message of G2 carries the ADR 0077 `linear_upwind` note and "lower the
relaxation factors (solver.alpha_u, solver.alpha_p)", neither of which fits this case (the coupled solver has no `alpha_p`); not changed.

### Part P

| Set-up | Ground truth (t_end = 10, check off) | Earlier | Warnings (default `warn`) | P-a / P-b |
|---|---|---|---|---|
| P1 PIMPLE-2 Euler Co_t 1.0 | STABLE (320 steps, max abs(u) 0.903, max abs(p) 1.36, continuity <= 1.2e-8) | STABLE | none | pass / pass |
| P2 PIMPLE-2 Euler Co_t 2.3 | STABLE (max abs(u) 0.900, max abs(p) 1.60) | STABLE | none | pass / pass |
| P3 PIMPLE-4 BDF2 Co_t 6.0 | STABLE (max abs(u) 0.904, max abs(p) 1.16) | STABLE | none | pass / pass |
| P4 PIMPLE-2 Euler Co_t 3.0 | UNSTABLE (first violation step 12, t = 1.125; PIMPLE guard at step 19) | UNSTABLE | one, step 1, rho 1.226 | pass / pass |
| P5 PIMPLE-4 Euler Co_t 5.0 | UNSTABLE (first violation step 13; guard at 18) | UNSTABLE | one, step 1, rho 1.238 | pass / pass |
| P6 PIMPLE-4 BDF2 Co_t 7.5 | UNSTABLE (first violation step 9; guard at 14) | UNSTABLE | one, step 2, rho 1.418 | pass / pass |

C4'' (reported): six one-corrector API runs made an estimate; no warning on the three stable ones (no false positive), one warning on each unstable one,
before its first violation. This makes C4 non-vacuous for these six set-ups only; the suite still has no other one-corrector validation case.

### Deviations from the registered plan

* Amendment 1: N1 asked for a NaN residual of a finite starting state (defective limit); corrected N1'.
* Amendment 2: the part X scan had no unsheared controls; controls at 0, 15, 30 degrees and V9b were registered after the scan and before they ran.
  V9's increments never entered the registered window [1e-9, 1e-2] (they start at round-off), so its rho is over all 400 iterations, as the registered
  text allowed.
* Part X V0 classifies 50 degrees UNSTABLE (overshoot, then convergence) where ADR 0077 S3 called it converged: different criteria, both stated.
* Part G uses the registered fallback (K1 trips); no finite runaway exists in the searched space.
* Not registered, added: the steady-guard note names the limiter as a remedy for an unlimited `linear_upwind` run (message text only).
* The L2s sweep and the smoke runs were scratch-tree code (an environment variable and logging in `sim.rs`), not committed. The L2s run took 2 h 29 min
  of wall time (the whole suite, two test threads); rule (e) of wave 35 (no single run over about 60 minutes) was exceeded by that suite run.

### Unverified references

Barth and Jespersen (1989) for the limiter (the existing rcfd implementation; recalled, UNVERIFIED). The Kovasznay solution (Kovasznay 1948, recalled,
UNVERIFIED; used only through its closed form). The sqrt(2) linear stability bound of Kolmogorov flow (Meshalkin and Sinai 1961, recalled, UNVERIFIED;
only used to choose nu = 0.5 for V7). The statement that Barth-Jespersen clips smooth extrema (recalled, UNVERIFIED; not tested).
