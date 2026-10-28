# ADR 0085: Wave 37B, why the absolute-velocity rotor passage of ADR 0082 does not converge on M0 and M2, and three remedies tried under registered acceptance

Status: registered before any code of this wave was written and before any solver run of this wave. Limits and decision rules are fixed; a miss of a
check becomes an `#[ignore]`d `*_known_miss` test (or a reported miss where the code it needs is removed) with the limit intact; a defective set-up gets
a NEW corrected test and the old one stays ignored. Baseline hashes are never re-recorded. Clean-room: first principles only; nothing from vendor
documentation or other CFD codes. Parents: ADR 0082 (the spurious torque of first-order upwind on the rigid part of the relative velocity, F-RP and its
removal), ADR 0081 part L (`solver.linear_upwind_limiter = "barth_jespersen"`), ADR 0029 (the coupled solver), ADR 0071 C5 (the periodic-pair mean of
`rAU` in the body-force term of the face flux), ADR 0078 (FAN-01 E0, the X2 empty passage), `docs/milestone-fan.md`. No fan performance claim is made.

## What was known when this was written

* Every number of ADR 0082, in particular: Y(M1) (F-RP, empty passage, upwind, SIMPLEC, SST) converged in 567 iterations with outlet angular-momentum
  flux -1.42e-5 N m (inertial control B(M1) 1.63e-4 N m, relative frame R(M1) 2.2544 N m); Y(M0) did not converge (8000 iterations, res_u 0.78,
  L 0.068 N m, Delta p_tt +981 Pa) while B(M0) converged in 124 and R(M0) in 247 iterations; Y(M2) and B(M2) did not converge within 60 minutes
  (B(M2): 7 063 iterations, two runs sharing the machine); the bladed F1 smoke had continuity about 0.4 for 300 iterations.
* The removed F-RP patch (`w36c_frp_removed.patch`, cut against 6c3ddc9) applies cleanly to feb015f (`git apply --check`). It is re-applied unchanged
  as the starting point of every arm of this wave.
* Code read on feb015f (read-only): `CoupledSolver::with_solids` refuses periodic patches ("the coupled solver does not support periodic patches");
  `Simulation::new` refuses `turbulence.model`, `sources` (and others) with `solver.algorithm = "coupled"`; the coupled engine has no counterpart of
  `conv_flux_offset`; its momentum blocks are one matrix per component (no cross-component block). In `sim.rs` an MRF zone switches
  `component_diag_correction` on, while `wall_pressure_trapezoid` and with it `coupled_body_pair_mean` stay off (they are set only for `rotating_frame` and
  accelerations); the relative-frame arm R has the pair mean on and the component correction off. The `-Omega x c` acceleration of `MrfZone::add_force`
  is evaluated from the previous iterate (cell and face values) and enters SIMPLEC as an explicit source.
* No run of any kind has been made in this wave.
* Arithmetic only (no run). A single-cell model of the lagged rotation source in the relaxed segregated momentum iteration (neighbours frozen,
  diagonal `a/alpha`, explicit rotation at rate Omega on a cell of volume V) has the error amplification `|lambda|^2 = (1 - alpha)^2 + (alpha x)^2`,
  `x = Omega V / a`; with `alpha = 0.9` it exceeds 1 for `x > 1.1`. Estimated on the largest M0 cells (axial 0.228 m, pitchwise 0.054 m, radial 0.07 m,
  `a` about the tangential relative flux `Omega r` times the axial-radial face area): `x` about 0.2 to 0.4. The relative-frame Coriolis term has rate
  `2 Omega`, so by this model R would be closer to the limit than Y, yet R(M0) converges. This is a prior AGAINST 36C's suspect; it is tested anyway
  (H-S below), because the model ignores the neighbour coupling and the face (Rhie-Chow) part of the source.
* In the empty passage the exact solution is an axisymmetric axial flow `c = c_z(r) e_z` (with `p`, `k`, `omega` functions of `r` and `z`), for which
  `-Omega x c = 0` and every tangential transport term vanishes. Therefore the diagnostics below that remove the frame acceleration, or convect `k`
  and `omega` with the absolute flux, or change the seam-pair or component-diagonal treatment, all have the same exact solution as Y; they are valid
  convergence diagnostics (not physical alternatives for a bladed rotor).

## Set-ups (all from ADR 0082; nothing new in the geometry)

Meshes M0, M1, M2 of ADR 0082 amendment 3 (4 320, 13 524, 47 040 cells; M3 is the recorded defective set-up and is not used). The empty-passage arms:

* **Y(h)**: F-RP as in ADR 0082 (`[sources.rotor] type = "mrf"`, `zone = "all"`, `rotation_axis = [0, 0, 1]`, `angular_velocity = Omega` = 1486 rpm,
  `rotor_passage = true`), absolute conditions identical to B (inlet `(0, 0, U)` with the ADR 0078 `k`, `omega`; hub `fixed` 0; shroud `fixed` 0 with
  `turbulence = "wall"`; outlet zero-gradient velocity, fixed pressure), SST, SIMPLEC, `upwind`, tolerance 1e-5, the ADR 0078 / 0082 harness quantities.
* **B(h)**: the inertial control = Y without the source (ADR 0078 X2b).
* An arm's variant `Y_A(h)` and its control `B_A(h)` use the same solver settings (algorithm, scheme, limiter, pseudo-time) as each other; only the
  source differs.

Run rules (all runs): release build, single thread per run, at most two runs at a time, one run per set-up (nothing re-run to pick a result), stop at
8000 iterations or 60 minutes wall clock (a run stopped by the clock is NOT CONVERGED), continuity non-finite or above 1e6 or a recorded steady-guard
trip ends the run as diverged. Converged = `Simulation::is_converged()` (res_u, res_p and the turbulence change below the tolerance) before a stop rule.
The wall-clock cap is the binding one on M2 (ADR 0082: about 0.5 s per iteration with two runs sharing the machine); a timing smoke of B(M2) run
alone (200 iterations) is recorded in amendment 1 before any registered M2 run; it does not change the cap.

## Part D: diagnosis of the non-convergence (registered; run before any remedy arm)

* **D0 reproduction.** Y(M1) and Y(M0) on the re-applied F-RP, with the location record below. Reproduction rule: Y(M1) converged in 567 iterations
  with L -1.42e-5 N m (printed digits of ADR 0082); Y(M0) not converged at 8000 iterations with res_u 0.78, L 0.068 N m, Delta p_tt +981 Pa (printed
  digits). If either does not reproduce, the other runs stop until the difference is explained in an amendment.
* **Location record of Y(M0)** (no limit): every 100 iterations the cell with the largest velocity change `|u^n - u^(n-1)|` and its region (upstream /
  chord band / downstream; wall-, seam-, inlet/outlet-adjacent; radius), continuity, res_u, res_p, and the worst periodic-pair flux sum
  `|phi_f + phi_g| / (Omega r |S|)`.

Hypotheses for the non-convergence of Y(M0) and their falsifiers. Each diagnostic is one run of Y(M0) with exactly one change, made by a scratch switch
in this wave's working copy that is never committed (disclosed here). "Converges" means by the registered criterion within the registered caps.

* **H-S (ADR 0082's suspect): the lagged `-Omega x c` source with SIMPLEC.** DS: the frame acceleration (cell and face parts) is removed; the frame flux
  stays. H-S is FALSIFIED if DS does not converge. If DS converges, H-S stands and the remedies I and C below are aimed at the right term.
* **H-T: the turbulence model on the relative flux with absolute-velocity gradients** (the new `sim.rs` hunk of F-RP). DT: `k`, `omega` convected with the
  absolute flux `phi` (the default `mrf` path). Falsified if DT does not converge.
* **H-P: the seam-pair asymmetry of the body-force term** (pair mean off for MRF). DP: `coupled_body_pair_mean = true`. Falsified if DP does not converge.
  The D0 location record shows the pair-flux sum; H-P is also reported against it.
* **H-K: the component-diagonal correction** (on for MRF, off for R). DK: `component_diag_correction = false`. Falsified if DK does not converge.

If exactly one of DT, DP, DK converges, its change becomes the remedy arm **X** (part of F-RP when `rotor_passage = true`, nothing else changes) and is
screened and accepted like the others. If two or more converge, they are all reported, and arm X is the one whose diagnostic converged in the fewest
iterations. If none of DS, DT, DP, DK converges, no hypothesis stands and the three requested arms are still run as registered.

## Part R: remedy arms (requested by the task; each registered now)

* **L: Barth-Jespersen-limited `linear_upwind`.** Y with `convection = "linear_upwind"`, `linear_upwind_limiter = "barth_jespersen"` (existing keys, no new
  code). Control B_L: the same keys without the source.
* **I: linearised frame source in the segregated solver.** In a segregated solve `-Omega x c` has no diagonal part per component; of the two forms
  available (Gauss-Seidel ordering of the components, which needs a change inside `simple.rs`, the file of the parallel agent 37A; or a diagonal damping
  that vanishes at convergence) this arm uses the second, with the existing keys: `pseudo_mode = "physical"`, `pseudo_dt = 1 / Omega` (6.426e-3 s), i.e.
  `V Omega` on the diagonal and `V Omega c_old` on the right-hand side in every cell (the zone is the whole mesh). In the single-cell model this damps the
  explicit rotation for any `x` below about 2.5 (`|lambda|^2 = ((a (1 - alpha)/alpha + D)^2 + (Omega V)^2) / (a/alpha + D)^2`, `D = Omega V`). No new code.
  Control B_I: the same keys without the source.
* **C: coupled pressure-velocity solver with the frame source implicit.** The coupled solver gets, only for this arm: (1) rotationally periodic patches
  (the pressure is coupled implicitly across a pair in the gradient, continuity and Rhie-Chow rows, the partner velocity rotated into the owner's frame
  exactly in the continuity rows; momentum across the seam through the lagged partner coefficients of `boundary_coeffs`, as SIMPLEC does); (2) a
  convective flux offset (the F-RP frame flux); (3) the MRF acceleration `-Omega x c` as the cross-component blocks `M_xy = -Omega V`, `M_yx = +Omega V`
  (signs for `Omega` along `+z`: `a_x = Omega c_y`, `a_y = -Omega c_x` moved to the left) of the coupled operator (GMRES sees them; the ILU-per-component
  preconditioner does not), the Rhie-Chow body-force term lagged; (4) in `sim.rs`, the turbulence update and the sources for `algorithm = "coupled"`,
  allowed only for `mrf` sources. Nothing changes for a coupled case without periodic patches, offset or sources. Control B_C: coupled, no source.
  Effort bound: if the extension does not pass its construction checks (A1 below on Y_C(M1)) and a 30-iteration smoke with finite continuity on
  B_C(M0), the arm is reported NOT RUNNABLE with the reason and its code removed. Arm C runs last.
* **X**: only if Part D produces it (above).

Smokes (rule (d) of wave 35): every arm's Y and B on M0 get construction plus a 30-iteration smoke before its registered run; values in amendment 1.

## Decision structure

1. Part D runs first (D0, then DS, DT, DP, DK).
2. **Screen on M0**: Y_A(M0) and B_A(M0) for every arm A in {X if any, I, L, C}. An arm passes the screen if Y_A(M0) converges and A2, A3 below hold on M0.
3. **Acceptance** (M1, M2, the M1 iteration check, A0-A4) runs for the arms that passed the screen in the order X, I, L, C (least change from the
   ADR 0082 F-RP and the default scheme first) until one passes all of A0-A4. That arm is kept (opt-in; its keys documented); every other arm's code is
   removed (rule (b) of wave 35). The remaining arms that passed the screen are not run further (their screen results are reported).
4. If no arm passes, F-RP and every arm's code are removed again; nothing of this wave changes the solver.

## Acceptance (all must pass for the kept arm A; a fix that fails any of A0-A4 is removed)

* **A0 bit-identical when off.** (i) The full workspace suite at the final gate passes with the wave-36 totals (1873 passed, 0 failed, 519 ignored)
  plus this wave's tests, and `crates/rcfd-api/tests/baseline_hashes.rs` is unchanged; (ii) ADR 0082 X2a-36 re-run on the final code reproduces outlet
  L 2.25440 N m and Delta p_ts -59.984 Pa to every printed digit; (iii) the ADR 0082 A0 (iii) test (ADR 0008 annulus `mrf`, 20 iterations, the key
  absent and `rotor_passage = false`) bit-identical; (iv) if arm C is kept: the coupled lid cavity (`crates/rcfd-api/tests/w36b_coupled_guard.rs`
  set-up without the NaN start: 12 x 12, nu 0.01, 20 iterations) gives bit-identical u, p and flux on the final code and on feb015f (the feb015f values
  are recorded in amendment 1 before any coupled code is written).
* **A1 construction.** The F-RP closure check passes (1e-10) on M0-M2 and the E0 mesh; at iteration 1 of Y_A(M1) the periodic pairs satisfy
  `|phi_rel,f + phi_rel,g| <= 1e-12 Omega r |S|`.
* **A2 convergence and angular momentum.** Y_A(h) converges within the caps on M0, M1 and M2, and `|L_out(Y_A(h))| <= 10 |L_out(B_A(h))|` on each mesh.
  If B_A(h) does not converge within the caps, that comparison is a set-up defect of the control, not a miss of the fix: the limit on that mesh is then
  `0.01 |L(R(h))|` with ADR 0082's converged relative-frame values (R(M0) 3.4396, R(M1) 2.2544, R(M2) 1.6793 N m, i.e. 0.0344, 0.0225, 0.0168 N m),
  and the report says so. The convergence of Y_A(h) itself has no fallback.
* **A3 energy.** Outlet `rho <c_theta^2/2>_m` of Y_A(h) <= 0.2 Pa (1 % of ADR 0082 X2d's 19.97 Pa) and the mass-averaged duct `Delta p_tt <= 0` (a
  passive duct with walls at rest cannot gain total pressure), on M0-M2.
* **A4 order from the 3-mesh study.** `e_h = |L_out(Y_A(h))|` (the exact value is 0); `e_M2 <= e_M1 <= e_M0` and
  `p = ln(e_M0 / e_M2) / ln(2.25) >= 0.5`. Floor rule (ADR 0082): Y_A(M1) and B_A(M1) are also run at tolerance 1e-7; if `L(Y_A(M1))` changes by more
  than 50 % between 1e-5 and 1e-7, A4 is "not measurable (iteration floor)", neither passed nor missed, and acceptance rests on A0-A3. If L changes by
  more than 10 % for Y_A or B_A, all M0 and M2 runs of that arm use 1e-7 (re-run once at 1e-7; the 1e-5 runs are reported). The order of B_A is reported.

## FAN-01 with the kept arm, once

**F1** = ADR 0078 E0 (13 332 cells, SST, design point Q = 1.4032 m^3/s) with F-RP, the kept arm's settings and the absolute conditions of ADR 0082
(hub and blade `fixed` with `velocity_expression_x/y = -Omega y, Omega x` and the turbulence wall treatment; shroud `fixed` 0, wall), the ADR 0078
stop rules. Run once, only if acceptance passes. Reported as the change of the torque (blade + hub on the fluid, x 9) and of Delta p_ts (ADR 0075
definitions) against E0 (1.32007 N m, -3.813 Pa), with Delta p_tt, outlet swirl energy and cut-U angular momentum alongside, and against the measured
2.142 N m / 126.46 Pa without any agreement claim. If the kept arm uses `linear_upwind`, the change against E0 mixes the scheme and the formulation, and
the report says so. If F1 does not converge by the ADR 0078 stop rules, its numbers are printed as "not converged, not a result" and no torque or
Delta p_ts change is claimed. F1 is not an F-126 attempt; F-126 stays MISSED whatever it gives.

## Code policy

Tests in a new file `crates/rcfd-api/tests/fan01_rotor_passage_w37b.rs` (the harness of `fan01_rotor_frame_w36c.rs` copied, that file not edited; long
runs ignored; construction, bit-identity and refusals not ignored when fast). F-RP as in the ADR 0082 patch (`crates/rcfd-api/src/sources.rs`,
`crates/rcfd-fv/src/sources.rs`, the `sim.rs` and `schema.rs` hunks, `docs/case-schema.md`). Arm C: `crates/rcfd-fv/src/coupled.rs` and its `sim.rs`
wiring. Nothing in `crates/rcfd-fv/src/simple.rs` or `pressure.rs` (the files of agent 37A). The Part D switches live only in the uncommitted working
copy. Shared files (`sim.rs`, `schema.rs`): only this wave's hunks are committed.

## Amendment 1 (2026-10-20): part D results (no hypothesis stands), smokes, A1, the timing smoke, the coupled-cavity hashes, and the definition of B_C; written before any screening run of arm C and before any acceptance run

What I knew when writing this: the runs below; the screening runs Y_I(M0) and B_I(M0) had been started (as registered, unaffected by this
amendment) and their results were not known. Code written so far: the ADR 0082 F-RP patch re-applied unchanged; the uncommitted part D switches
(environment variable `W37B_DIAG` = `no_force`, `turb_abs`, `pair_mean`, `no_compdiag` in the working copy's `sim.rs` / `sources.rs`); arm C (periodic
faces, flux offset and implicit rotation in `coupled.rs`; turbulence and rotor-passage sources for `algorithm = "coupled"` in `sim.rs`, `turb.rs`,
`sources.rs`); the test file.

**D0 reproduction (rule met).** Y(M1): converged in 567 iterations (1.1 min), outlet L -1.418e-5 N m, Delta p_tt -1.541 Pa (ADR 0082: 567, -1.42e-5,
-1.541). Y(M0): not converged at 8000 iterations (5.4 min), res_u 0.785, L 0.0682 N m, Delta p_tt +980.9 Pa (ADR 0082: 0.78, 0.068, +981).

**Location record of Y(M0).** res_u is 0.57 at iteration 10 and 0.73 at iteration 100 and never falls below about 0.5 afterwards (continuity 1e-5 to
1e-3). Of the 80 records (every 100 iterations) the largest velocity change sits 30 times in cell 827 (faces on `outlet` and `seam_lo`, r 0.128 m next to
the hub, z 0.296), 27 times in cells 285 / 555 / 556 (on `seam_lo` at r 0.124-0.125, i.e. against the hub, z 0.17-0.19 just downstream of the chord
band), and otherwise in interior chord-band cells at r 0.18-0.24 (1754 twelve times, one change of 233 m/s in one iteration). Worst periodic-pair flux
sum `|phi_f + phi_g| / (Omega r |S|)`: up to 6.8e-2 with the force, at most 6.2e-14 in DS (without the force the pairs are exact; the asymmetry comes
from the body-force term of the face flux, ADR 0071 C5). At iteration 8000 the outlet has backflow on 92 of its 240 faces (reverse / forward volume flux
0.17, measured on DK, which is bit-identical to D0), which is why the mass-weighted outlet swirl energy prints negative (-2.02 Pa).

**Verdicts (each one run of Y(M0) with one change, 8000 iterations):**

| Diagnostic | Converged | res_u at 8000 | L [N m] | Delta p_tt [Pa] | Hypothesis |
|---|---|---|---|---|---|
| DS, frame acceleration removed | no | 0.79 | 0.198 | +608.7 | H-S FALSIFIED (the lagged `-Omega x c` source is not what stops convergence) |
| DT, k and omega on the absolute flux | no | 0.81 | 0.0464 | +199.7 | H-T FALSIFIED |
| DP, pair mean of `rAU` on | no | 0.66 | 0.412 | +2841 | H-P FALSIFIED |
| DK, component-diagonal correction off | no | 0.785 | 0.0682 | +980.9 | H-K FALSIFIED (bit-identical to D0: no cell has unequal component diagonals) |

No hypothesis stands, so there is no arm X. The three requested arms run as registered; arm I is now known to be aimed at a term that is not the cause
(it still damps every momentum update, which is why it is not dropped). What the record does show: the failure starts within 10 iterations, sits next
to the hub downstream of the chord band and at the outlet / seam corner, and comes with outlet backflow; the inertial control B(M0) converges in 124
iterations on the same mesh. The remaining difference between B and DS is the convecting flux (`phi` against `phi - F_tri`, the large tangential relative
flux crossing the seams through the lagged partner coefficients). Not tested further by a registered diagnostic in this wave.

**A1 and smokes (30 iterations, continuity at iterations 1 / 10 / 20 / 30, all finite):** seam pairs at iteration 1 (limit 1e-12): Y_L(M0) 1.3e-13,
Y_I(M0) 1.3e-13, Y_C(M0) 2.9e-14, Y_L(M1) 2.1e-13, Y_I(M1) 2.1e-13, Y_C(M1) 4.9e-14 (pass). Y_L(M0) 1.8e-6 / 2.9e-6 / 3.5e-6 / 3.7e-6 (res_u 0.30 at
30); B_L(M0) 1.9e-6 / 3.4e-6 / 6.3e-6 / 2.1e-6 (0.048); Y_I(M0) 3.2e-6 / 6.3e-7 / 1.8e-7 / 1.3e-7 (0.029); B_I(M0) 5.7e-6 / 1.8e-6 / 2.9e-7 / 4.7e-8 (0.0080);
Y_C(M0) 1.2e-4 / 2.5e-5 / 2.0e-5 / 1.5e-5 (0.0052); B_C(M0) 3.8e-4 / 4.7e-6 / 1.1e-6 / 1.4e-7 (2.8e-4); Y_L(M1), Y_I(M1), Y_C(M1) res_u at 30: 0.094,
0.028, 0.0069. The coupled solver's continuity is not driven below about 2e-5 per iteration by its relative linear tolerance 1e-3 (the default); its
convergence criterion is res_u and res_p as for SIMPLEC. Cost: Y_C(M1) 1.0 s per iteration (SIMPLEC Y(M1) 0.12 s).

**Timing smoke B(M2) alone:** 200 iterations in 89.6 s, 0.448 s per iteration, so 8000 iterations need about 60 minutes on an otherwise idle core;
res_u was 0.13 at iteration 10 and 0.63 at iteration 200 (the inertial control itself does not converge smoothly on M2). The cap stays as registered.
For arm C the 60-minute cap allows about 1 000 iterations on M2 (estimated from the M1 cost and the cell ratio 3.5).

**A0 (iv) hashes** (coupled lid cavity 12 x 12, nu 0.01, 20 iterations; FNV-1a over the bits of u, p and phi): upwind `0xbccb8f70fa93d84d`,
linear_upwind `0x9c4c56461eaa0365`. Recorded on the working copy with only the F-RP patch applied, before any line of `coupled.rs` was changed (F-RP
touches no code of the coupled path); deterministic in two repeats. After the arm C code: both hashes reproduced (test `a0_coupled_cavity_unchanged`).

**B_C, departure from the registered control (stated before any B_C or Y_C screening run).** The registered B_C ("coupled, no source") cannot be built:
the coupled solver takes a turbulence model only together with rotor-passage sources (the scope registered for arm C), and the parse-time checks of
`turb.rs` and `sources.rs` refuse it otherwise. B_C is therefore Y_C with `angular_velocity = 0` (`rotor_passage = true`): the frame flux, the force and
the cross blocks are exactly zero, so it is the inertial formulation on the coupled solver. Arm C's code scope grows by the two parse-time exceptions
(`sources::rotor_passage_only`). Everything else is unchanged.

## Addendum (2026-10-20): measured values, verdicts, removal of F-RP and of every arm, misses and deviations

All runs: release builds of feb015f plus this wave's working copy, single thread per run, at most two of my runs at a time on a machine shared with
the other wave-37 agents, one run per set-up, tolerance 1e-5 unless stated. Full-annulus values, `rho` = 1.2. Logs and the removed code are kept
outside the repository (`w37b_logs`, `w37b_frp_and_arm_c_removed.patch`, `w37b_fan01_rotor_passage_w37b.rs` in the session scratchpad).

### Outcome in one paragraph

No arm passes. The lagged frame source is not the cause of the M0 failure (H-S falsified: Y(M0) does not converge with the source removed, and three
other candidate mechanisms are falsified the same way). Two of the three requested remedies cure the coarse mesh: the pseudo-time step `1 / Omega`
(arm I) and the coupled solver with the rotation implicit (arm C) both converge on M0 and M1, with outlet angular-momentum flux 3 to 7 times below
their inertial controls and no swirl energy. Neither converges on the fine mesh M2 within the registered 60-minute cap: arm I stalls at res_u 0.41
(its SIMPLEC inertial control also stalls, in a frozen limit cycle), arm C oscillates at res_u 1e-3 to 3e-3 for 1 007 iterations while its coupled
inertial control converges in 326. Barth-Jespersen `linear_upwind` (arm L) fails on M0 already. By the registered rule F-RP and every arm's code are
removed; F1 (FAN-01 with the fix) was NOT RUN, so no torque or Delta p_ts change is reported.

### Checks with limits

| Check | Limit | Measured | Result |
|---|---|---|---|
| D0 Y(M1) reproduces ADR 0082 | 567 it, -1.42e-5 N m | 567 it, -1.418e-5 N m, Delta p_tt -1.541 Pa | pass |
| D0 Y(M0) reproduces ADR 0082 | 8000 it, res_u 0.78, 0.068 N m, +981 Pa | res_u 0.785, 0.0682 N m, +980.9 Pa | pass |
| H-S, H-T, H-P, H-K (amendment 1) | falsified if the diagnostic does not converge | none converges | all four FALSIFIED |
| A1 seam pairs, iteration 1 (Y_L, Y_I, Y_C on M0, M1) | 1e-12 | 2.9e-14 to 2.1e-13 | pass |
| A1 F-RP closure (`Simulation::new`) on M0, M1, M2 | 1e-10 | built on all three | pass |
| A1 F-RP closure on the E0 mesh | 1e-10 | not built (F1 not run); not measured in this wave (ADR 0082: 1.2e-15) | not measured |
| A0 (iii) key absent = `false` (20 iterations) | bit-identical | bit-identical | pass (code then removed) |
| A0 (iv) coupled cavity hashes after the arm C code | feb015f hashes | reproduced | pass (code then removed) |
| Screen I: Y_I(M0) converges; A2, A3 on M0 | converged; 10 x 5.880e-4 N m; 0.2 Pa; 0 Pa | 476 it; 1.201e-4 N m; 0.0000 Pa; -1.648 Pa | pass |
| Screen L: Y_L(M0) converges | converged | NOT CONVERGED (8000 it, res_u 0.36; B_L(M0) also not, res_u 2.5e-3) | MISS |
| Screen C: Y_C(M0) converges; A2, A3 on M0 | converged; 10 x 5.406e-4 N m; 0.2 Pa; 0 Pa | 476 it; 7.467e-5 N m; 0.0000 Pa; -1.648 Pa | pass |
| Arm I, A2/A3 on M1 | converged; 10 x 1.703e-4 N m | 567 it; -2.480e-5 N m; 0.0000 Pa; -1.541 Pa | pass |
| Arm I, M1 iteration check (1e-5 against 1e-7) | 10 % / 50 % | Y 0.006 %, B 0.001 % | tolerance 1e-5 kept, floor rule not triggered |
| Arm I, A2 on M2: Y_I(M2) converges | converged within 60 min | NOT CONVERGED (time cap, 5 715 it, res_u 0.41 stalled from about iteration 300) | MISS |
| Arm I control B_I(M2) | (fallback rule) | NOT CONVERGED (5 976 it, res_u 6.25e-2 frozen from iteration 200) | control defect |
| Arm I, A3 and A4 | | not evaluable (no converged M2) | MISS (follows A2) |
| Arm C, A2/A3 on M1 | converged; 10 x 1.595e-4 N m | 567 it (7.6 min); -5.350e-5 N m; 0.0000 Pa; -1.541 Pa | pass |
| Arm C, M1 iteration check | 10 % / 50 % | Y 0.004 %, B below 0.001 % | tolerance 1e-5 kept, floor rule not triggered |
| Arm C, A2 on M2: Y_C(M2) converges | converged within 60 min | NOT CONVERGED (time cap, 1 007 it; res_u 2.6e-3, 1.7e-3, 2.6e-3, 0.95e-3, 1.5e-3 at iterations 100, 200, 400, 600, 800) | MISS |
| Arm C control B_C(M2) | | converged, 326 it (20.3 min), L 7.880e-5 N m, Delta p_tt -1.526 Pa | control fine |
| Arm C, A3 and A4 | | not evaluable (no converged M2) | MISS (follows A2) |
| A0 (i), (ii) | gate; X2a digits | no solver code of this wave is committed | not applicable |
| F1 | run only if acceptance passes | NOT RUN | not applicable |

Values at the cap, not results (printed for completeness): Y_I(M2) L 1.42e-5 N m, Delta p_tt -1.907 Pa; Y_C(M2) L 2.17e-5 N m, Delta p_tt -1.476 Pa;
B_I(M2) L 8.47e-5 N m.

### What the runs show (observations, not acceptance)

* Where it converges, the absolute-velocity rotor passage removes the spurious angular momentum with either solver: outlet L 1.2e-4 and -2.5e-5 N m
  (arm I, M0 and M1), 7.5e-5 and -5.4e-5 N m (arm C), against 3.44 and 2.25 N m in the relative frame (ADR 0082) and 5.9e-4 to 1.6e-4 N m in the
  inertial controls. The coupled inertial control falls with order 2.4 over M0-M2 (5.41e-4, 1.60e-4, 7.88e-5 N m); Y_C falls by a factor 1.40 from M0
  to M1 (pairwise order 0.83).
* The M0 failure of the SIMPLEC rotor passage is cured by damping the momentum update (`pseudo_dt = 1 / Omega`, arm I) or by the coupled solve (arm C),
  but not by removing the frame source (DS). So the cure is not about the frame source: damping or a coupled solve stabilises something else in the
  convection of absolute momentum by the large relative flux. The D0 record put the largest updates next to the hub downstream of the chord band and at
  the outlet / seam corner, with outlet backflow (92 of 240 faces); the converged arm I and C runs on M0 have no backflow.
* On M2 the largest updates of Y_C sit in hub-adjacent cells (r 0.1241-0.1245) of the chord band and just downstream; those of Y_I in two interior
  chord-band cells (r 0.149 in 42 of 57 records, r 0.170 in 14); B_I(M2) cycles in one interior upstream cell (r 0.222, z 0.076, 58 of 59 records).
  The SIMPLEC inertial control does not converge on M2 with or without the pseudo-time step (ADR 0082: 7 063 iterations; here 5 976): M2 is a hard
  mesh for SIMPLEC by itself (the subject of agent 37A). The coupled inertial control converges there and the coupled rotor passage does not, so the
  remaining M2 obstacle of arm C belongs to the rotor passage and sits against the stationary hub.
* Cost: the coupled solver needs about 0.3 s (M0), 0.8 s (M1) and 3.6 s (M2) per iteration against 0.04, 0.12 and 0.45 s for SIMPLEC, so the 60-minute
  cap allows about 1 000 coupled iterations on M2; Y_C needed 476 (M0) and 567 (M1). Whether Y_C(M2) would converge with more iterations is not known;
  its residual did not decrease over its last 600 iterations.
* The iteration counts of the converged runs are identical across solvers (M0: Y_I and Y_C 476; M1: Y_U, Y_I, Y_C 567, at 1e-7 772; M1 controls
  B_I and B_C 170). At the stopping iteration res_u and res_p were between 3e-6 and 2e-12 (below 1e-7 in every Y run), well below the tolerance, so the binding criterion was the
  turbulence change of the SST update, not the momentum or pressure residual. The counts measure the turbulence model's settling, not solver speed,
  and must not be used to size an iteration budget for the momentum solve.

### Verdict on the fixes

F-RP with arm I misses A2 (and so A3, A4) on M2; F-RP with arm C misses A2 (and so A3, A4) on M2; arm L misses its screen on M0. By the registered
decision structure (item 4) F-RP, the arm C code (periodic faces, flux offset and implicit rotation in `coupled.rs`; the coupled turbulence and
rotor-passage wiring in `sim.rs`, `turb.rs`, `sources.rs`) and the test file that needs them are removed; arms I and L used existing keys only.
Nothing of this wave changes the solver. F1 was not run.

### Deviations from the registered plan (candid)

* B_C was redefined as Y_C at `angular_velocity = 0` (amendment 1, before any C screening run), because the coupled solver takes turbulence only with a
  rotor-passage source.
* The arm C code scope grew by two parse-time exceptions and by a refusal of periodic patches on the coupled solver without a rotor-passage source
  (the extension was verified only on the rotor passage); all removed with arm C.
* The A0 (iv) hashes were recorded on the working copy with only the F-RP patch applied, not on a clean feb015f checkout (F-RP touches no code of the
  coupled path).
* The outlet-backflow print was added to the harness after D0, DS, DT and DP had run (it is read from DK, which is bit-identical to D0).
* The arm runs shared the machine with each other and with other agents; the M2 runs of arm I ran two at a time for the full hour, Y_C(M2) shared the
  machine for its first 20 minutes. The wall-clock cap was applied as registered; the iteration counts at the cap depend on the load.
* Arm I was registered as a remedy for the frame source; after DS falsified H-S it was still run as registered (amendment 1 says so).

### Unverified references

None. Every limit is derived here, measured, or a decision threshold of this protocol (inherited from ADR 0082); no external value is used.

### Final gate

Fresh `git archive` of e52583c (main, which contains the wave-37A, 37C and 37D commits up to 1c17668; this wave committed documentation only):
`cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean, `cargo test --workspace --release --no-fail-fast`
1881 passed, 0 failed, 531 ignored (no LNK1201). `crates/rcfd-api/tests/baseline_hashes.rs` is untouched by this wave. This paragraph is a docs-only
commit and was not re-gated.
