# ADR 0101: Wave 41A, partials close-out: oscillating cylinder and G6 continuity (F-063), two-timescale error control (F-209), primary-vortex bracket (F-143), thick Milne slab (F-052)

Status: registered before the registered tests were run. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test
with the limit unchanged, a defective set-up gets a new corrected test and the old one stays ignored. Test code only: no
library change, so defaults and `crates/rcfd-api/tests/baseline_hashes.rs` are untouched. Acceptance texts are not edited (the
F-209, F-143 and F-052 texts were reworded by the user on 2026-10-21 and are the texts met here). Clean-room: reference values
are closed forms derived below, rcfd's own exact semi-discrete solutions, the Ghia et al. (1982) Re 100 centreline (committed
CSV, verified in ADR 0096) and the Hopf constant (docs/reference-data.md, verified in wave 13).

Gaps re-checked in the code before writing this ADR: `g6_rigidly_rotating_mesh_with_the_fluid` (rcfd-fv tests/ale_flows.rs)
prints the continuity of the last step only and asserts nothing about it; no test moves a cylinder (g1 translates a whole box);
F-209 has no two-timescale test; `v11_..._known_miss` (rcfd-gui tests/viewport_raster.rs) demands exactly one minimum;
rcfd-api tests/radiation.rs already holds `r2_p1_against_the_transport_asymptote` (tau = 20, non-ignored, 0.41 %), which the
F-052 note did not count.

## What was run before this ADR was written (disclosed)

Test code was written first. Only these runs were made (release, HEAD a2e0bfd plus these test files):

* `e1_shear_wave_setup_smoke`: rates 0.999197 and 128.0968 (ratio 128.20); one PISO step of dt = 0.01 reproduces the
  implicit-Euler factors of both modes to 1.66e-15 (relative L2).
* `c1_oscillating_cylinder_setup_smoke`: one period (40 steps) finite, minimum `V_new/V_old` 0.9787. The smoke computes the
  registered quantities but prints only the volume ratio; I did not look at continuity, GCL or total volume.
* `r7_thick_slab_setup_smoke`: tau = 50, 200 iterations, finite wall flow. No flux compared with the reference.
* A Python model of the F-209 controller (scratch, not committed) that copies `ControlledRun::advance` (step doubling, max-norm
  rate relative to max |u|, safety 0.9, growth 1.2, shrink 0.5, landing stretch 1.05, min/max step clamps) applied to the exact
  per-mode implicit-Euler factors on the 64 cell centres. Because the smoke shows the solver equals that linear map to 1e-15,
  the model is a prediction of the Rust run. I ran it for T = 1, 2, 10, 20, 50, 80, 100, 150, 200 and tol 1e-2 to 1e-4, first
  without, then with the S4 clamps. The registered T and tolerances follow the rules stated in E1, chosen after these model
  runs; the model predicts a miss at every T and tolerance it was run for, so the choice cannot favour a pass.
* F-143 P1 has no new set-up: it reads the run of v11 (ADR 0096). Its outcome is known in advance from the ADR 0096 addendum:
  minima (0.6125, 0.7375) speed 0.0048 and (0.3375, 0.8375) speed 0.178.

## C1 and G6 (F-063): oscillating cylinder on a smoothed mesh, rotating annulus continuity

Acceptance: "Oscillating cylinder and rotating toy case conserve mass; geometric conservation satisfied".

C1 set-up (`c1_oscillating_cylinder_on_a_smoothed_mesh_conserves_mass_and_volume`, rcfd-fv tests/ale_flows.rs):
`cylinder_ogrid(48, 16, 0.5, 2.0, Uniform, 0.1)` (one cell in z, `frontAndBack` empty). The cylinder patch moves rigidly along x,
`x_c(t) = 0.1 sin(2 pi t)` (amplitude 0.2 radius, period 1); the outer circle `farfield` is a fixed no-slip wall, so the domain is
closed. Each step the point displacement is Laplacian smoothing (`smooth_displacement`, tight solver) of the cylinder
displacement from the initial mesh, classes cylinder Prescribed, farfield Fixed, frontAndBack Slip (skipped as empty). Fluid at
rest at t = 0, nu = 0.01, PISO, central, implicit Euler, tight solvers (the `settings` of the file), `dt = 1/40`, 80 steps
(2 periods), the cylinder velocity from `set_follow_conditions`, `step_ale_with_walls`. Limits, maximum over all 80 steps:

* (i) mass: `StepInfo::continuity` (max |sum of face fluxes| per cell over the largest face area) <= 1e-9, the limit of
  G5 (i) / g5b in ADR 0033.
* (ii) GCL: `max_c |sum_f sigma_f W_f - (V^{n+1} - V^n)| <= 1e-12 mean(V)`, G1 (i) of ADR 0033, on the smoothed mesh.
* (iii) total volume: a rigid cylinder inside a fixed wall leaves the fluid volume constant, `|V(t) - V0| <= 1e-12 V0`.

G6 (`g6_rigidly_rotating_mesh_with_the_fluid`, unchanged set-up): add (i) the maximum over all 50 steps of `continuity` <= 1e-9
(the 4.1e-14 of ADR 0033 is the last step only, so the margin is about four decades on that value) and (ii) the same GCL limit
1e-12 mean(V) in every step.

Verdict rule: if C1 and the extended G6 pass, F-063's acceptance as written is met by non-ignored tests (G1 already shows the
GCL for rotation and oscillation on box meshes) and F-063 is set `done`. The other open scope in its note (elastic analogy,
motion specifications, BDF2 or adaptive steps, turbulence, energy and scalars on moving meshes, the known misses) is not
acceptance text and stays recorded in the note.

## E1 (F-209): two-timescale linear decay under error control

Acceptance, first clause: "Error-controlled stepping (step doubling, `solver.error_tolerance`) on a two-timescale linear decay
(rates differing by at least 100, both modes of unit initial amplitude) reaches a final error E in fewer steps than the
smallest uniform step count with error <= E".

Set-up (`e1_error_control_beats_the_smallest_uniform_count_on_a_two_timescale_decay`, rcfd-fv tests/adaptive_step.rs).
`box_2d(4, 64, 4h, 2 pi, periodic, periodic)`, `h = 2 pi/64`; `u_x = sin(y) + sin(12 y)`, `u_y = 0`, p = 0; nu = 1; PISO,
central, implicit Euler, solver tolerances 1e-16 abs / 1e-14 rel. Since u_x does not depend on x and u_y = 0, the convective
term and the pressure correction vanish and each mode decays on its own with the eigenvalue of the 3-point periodic Laplacian,
`lambda_k = (4/h^2) sin^2(k h/2)`: rates 0.9992 and 128.10, ratio 128.2 (>= 100; the continuous ratio is 144). Reference: the
exact semi-discrete solution `exp(-nu lambda_k t) sin(k y_c)` (independent of any rcfd run, rule b of wave 34). Error: relative
L2 over cells of the velocity vector against it at `T`.

Rules fixed before the T sweep: `T = 1` (the slow mode decays by e^-1, as the Taylor-Green run with 2 nu T = 1); controller =
S4's (`error_tol`, `min_dt = 1e-5`, `max_dt = 1`, other settings default, first proposal 0.01, no extrapolation);
tolerances 1e-2 and 2.5e-3 (the ends of S4's range). Uniform runs: N steps of exactly T/N, N = 1, 2, ... until the error is <= the
controlled run's E (cap 5000, no monotonicity assumed). The test runs in release and is ignored in debug.

Limit: for both tolerances, `n_controlled < N_min(E)`. Steps are accepted steps (each costs three solves; reported).

Prediction (model, with the S4 clamps): tol 1e-2: 1913 controlled steps (883 of them accepted at `min_dt` above the
tolerance), E = 7.84e-3, uniform N_min = 64; tol 2.5e-3: 3149 steps, E = 2.08e-3, N_min = 240. **E1 is expected to MISS by a
factor 13 to 30.** Without the clamps the model gives 2502 vs 63 and 9989 vs 240. In the T sweep (no clamps) the gap narrows
only slowly (T = 10: 2743 vs 544; T = 50: 3812 vs 2682 at tol 1e-2; 38514 vs 27676 at 1e-3) and from T = 80 the controlled
run's relative error exceeds 1, because the slow mode has decayed by e^-80 and the relative final error loses meaning.

Why, from first principles. For `y' = -lambda y`, one implicit-Euler step of h against two of h/2 differs by
`(lambda h)^2/4` of the amplitude, so the controller (error per unit time, `rate = diff/(max|u| h) <= tol`) takes
`h ~ 4 tol max|u| / (lambda_f^2 a_f(t))` while the fast mode dominates the estimate, that is until
`lambda_f^2 a_f < lambda_s^2 a_s`, `t* ~ 2 ln(lambda_f/lambda_s)/lambda_f`. The number of steps spent there is
`int dt/h ~ lambda_f/(4 tol)`, independent of T. The slow phase then needs about as many steps as uniform stepping at the same
error. Uniform implicit Euler is L-stable: errors made in the fast mode are damped by the next steps and never need resolving,
and the final error is the slow-mode error `lambda_s^2 T h/2` per unit amplitude, so `N_min ~ (lambda_s T)^2/(2E)`. With
`E ~ tol lambda_s T` the controlled count exceeds the uniform one by about `lambda_f/(lambda_s T)` times the slow-phase count:
paying back the transient needs `lambda_s T` of the order of the rate ratio (about 100 here), where the relative final error is
meaningless. A step-doubling controller of the local error can only beat uniform steps on the FINAL error when the early
fast-mode error is not damped away (oscillatory or forced fast modes, or an error measured over the whole run). The clause as
reworded asks it of a decaying linear system, where it should not hold. This is reported to the user; no rewording is applied.

Verdict: F-209 stays `partial` whatever E1 gives (the flat-plate clause is open). A miss becomes
`e1_..._known_miss` with the limit unchanged.

## P1 (F-143): primary-vortex minimum in the Ghia bracket

Acceptance: "Cavity raster at Re 100 (40 x 40, linear upwind) matches the Ghia u centreline within 2 percent of lid speed and its
primary-vortex speed minimum lies inside the Ghia u sign-change bracket (not the near-lid shear-layer minimum); colour map and
range logic unit-tested; viewport renders headless without panic".

Set-up (`v11b_cavity_raster_primary_vortex_minimum_lies_in_the_ghia_bracket`, rcfd-gui tests/viewport_raster.rs): the v11 run
and window unchanged (`v11_cavity_raster()`: F-028 set-up through the GUI controller, 40 x 40 speed raster, strict 3 x 3 minima
with pixel centres at y in [0.5, 0.85], x in [0.15, 0.85]). Selection rule: the primary-vortex minimum is the strict minimum of
lowest speed in that window (the vortex core is where the speed tends to zero; the shear-layer minimum is a local dip in a fast
region). Limit: at least one strict minimum, and the primary one's centre y lies in the Ghia Re 100 u sign-change bracket
0.6172 to 0.7344 (both rows of w31c_ghia1982_u_centerline_read.csv) widened by one pixel (0.025) on each side, [0.5922, 0.7594],
the bracket logic of v11 C2.

Disclosed: the outcome is known (y = 0.7375, ADR 0096). Without the one-pixel widening the pixel centre would lie 0.0031 above
the bracket; the widening is the registered rule of ADR 0096 C2, kept because a pixel spans 0.025 and the upper bracket end sits
next to the centreline zero of u.

Verdict: C1 (`v11_cavity_raster_centreline_matches_ghia_re100`, 0.0028 vs 0.02) passes; colour map and range logic are
unit-tested (ADR 0025; its luma known miss is an 8-bit quantisation property, not a missing unit test); the headless frames run.
If P1 passes, F-143 is set `done`.

## R7 (F-052): thick slab against the Milne asymptote

Acceptance: "P1 grey slab matches the analytic P1 solution (1e-3) and the exact transport limits: optically thin two-plate
exchange and the thick Milne asymptote (1 percent)". R1 (2.6e-9 of 1e-3) and R6 (7.5e-5 / 3.3e-5 of 1e-3) are met; R2 already
checks the Milne asymptote at tau = 20 (0.41 %, limit 1 %).

Closed forms, derived here. Grey medium of absorption coefficient beta between plates at `t = 0` and `t = tau` (optical
coordinate `t = beta x`), radiative equilibrium (conduction negligible, `k = 1e-9`): `div q = 0`, so `q` is constant and
`G = 4 E_b`, `E_b = sigma T^4`. Psi = q / (E_b1 - E_b2).

* Exact transport, thick slab. Far from both walls the intensity is nearly isotropic and the diffusion law
  `q = -(4/3) dE_b/dt` holds, so `E_b` is linear in t. Near the black wall at t = 0, a uniform intensity `E_b1/pi` solves the
  RTE in radiative equilibrium, so `E_b - E_b1` solves the half-space problem with zero incoming intensity and constant flux:
  the Milne problem, whose solution approaches `-(3q/4)(t + q_inf)` away from the wall (q_inf the Hopf constant; the sign
  because the flux points away from this wall). At the cold wall likewise `E_b - E_b2 -> (3q/4)(tau - t + q_inf)`. Matching
  both to the one linear interior profile gives `E_b1 - E_b2 = (3q/4)(tau + 2 q_inf)`, so
  `Psi_Milne = 4 / (3 tau + 6 q_inf)`. The neglected terms decay like the exponential integrals of the distance to the walls,
  about e^-tau relative for the overlap of the two wall layers.
* P1 with Marshak conditions, black walls: `q = -(1/3) dG/dt`, G linear; `q = (4 E_b1 - G(0))/2` and
  `q = (G(tau) - 4 E_b2)/2`; adding, `4(E_b1 - E_b2) = 4q + 3 q tau`, `Psi_P1 = 4/(3 tau + 4)`: the same form with
  `q_inf = 2/3`.
* Gap: `Psi_P1/Psi_Milne - 1 = (6 q_inf - 4)/(3 tau + 4)`. With q_inf = 0.7104 this is 0.2624/(3 tau + 4): 0.41 % at tau = 20,
  0.171 % at tau = 50; it is below 1 % for tau > 7.4 and for every q_inf below 0.923 at tau = 50, so the check does not hinge on
  the last digits of q_inf. R1 shows the discretisation reproduces Psi_P1 to 1e-8, so the outcome is fixed analytically; R7
  checks that nothing in the thick regime (large beta, tiny conduction) breaks it.

Set-up (`r7_p1_thick_slab_matches_the_milne_asymptote`, rcfd-api tests/radiation.rs): R2's slab with tau = 50 (L = 1, beta = 50,
no scattering, black walls, nx = 200, k = 1e-9, T1 1000 K, T2 500 K), `run_converged`. Black walls only: no verified Milne closed
form for grey walls is used. Limit: `|Psi/Psi_Milne - 1| <= 0.01` for the hot and the cold wall flux. Predicted: 0.171 %.

Verdict: if R7 passes, F-052's reworded acceptance is met by R1, R6 and R2/R7 (non-ignored) and it is set `done`; S2S and
discrete ordinates stay in F-130. The note's "still partial until the thick Milne asymptote is shown" was stale: R2 showed it.

## Results

Addendum written after the runs: release build, HEAD 4a94ca6 (this ADR) plus the test files. Each registered test was run once,
except E1, see the deviation below.

| Check | Measured | Limit | Result |
| --- | --- | --- | --- |
| C1 oscillating cylinder, continuity (max over 80 steps) | 2.32e-14 | 1e-9 | met |
| C1 GCL residual (max over steps and cells) | 6.23e-16 mean(V) | 1e-12 mean(V) | met |
| C1 total fluid volume | 7.94e-15 V0 | 1e-12 V0 | met |
| C1 smallest V_new/V_old (reported) | 0.9787 | - | - |
| G6 continuity, max over 50 steps (last step) | 3.25e-13 (4.12e-14) | 1e-9 | met |
| G6 GCL residual | 6.63e-16 mean(V) | 1e-12 mean(V) | met |
| G6 velocity / pressure (ADR 0033 limits, unchanged) | 3.33e-3 W r_o / 4.97e-3 of range | 2e-2 / 5e-2 | met |
| E1 tol 1e-2: controlled steps vs smallest uniform count at equal error | 1913 (10 rejected), E 7.836e-3, against 64 (error 7.750e-3) | controlled < uniform | MISS |
| E1 tol 2.5e-3 | 3149 (10 rejected), E 2.080e-3, against 240 (error 2.076e-3) | controlled < uniform | MISS |
| P1 primary-vortex minimum (lowest of 2 strict minima) | (0.6125, 0.7375), speed 0.0048; other minimum (0.3375, 0.8375), 0.178 | y in [0.5922, 0.7594] | met |
| P1 run (v11 C1 printed again) | 0.00277 at y = 0.8516, 203 iterations, finished | 0.02 | met |
| R7 tau = 50, hot and cold wall | Psi 0.025974 against Milne 0.025930: 0.170 % (P1 analytic 0.025974) | 1 % | met |

E1 matches the model prediction to all printed digits (1913 / 64, E 7.8361e-3; 3149 / 240, E 2.0800e-3), so the miss is the
controller behaviour derived above, not a defect of the set-up. It is kept as
`e1_error_control_beats_the_smallest_uniform_count_on_a_two_timescale_decay_known_miss` (ignored, limit unchanged). R7's 0.170 %
equals the predicted P1 gap 0.2624/154.

Deviation: the first E1 run asserted inside the tolerance loop and stopped at tol 1e-2. I restructured the test to measure both
tolerances before asserting and ran it a second time (ignored, as the known miss) to obtain the tol 2.5e-3 values; the tol 1e-2
values were identical in both runs. No limit or set-up changed. The smoke tests `e1_shear_wave_setup_smoke` and
`c1_oscillating_cylinder_setup_smoke` stay as cheap non-ignored set-up checks; `r7_thick_slab_setup_smoke` is ignored.

Statuses: F-063 `done` (C1 and G6 met; open scope beyond the acceptance stays in its note); F-143 `done` (all clauses met by
non-ignored tests); F-052 `done` (R1, R6, R2 and R7); F-209 `partial` (E1 missed as predicted; the flat-plate clause is open). For
the user: the reworded F-209 clause asks local error control to beat uniform implicit Euler on the final error of a decaying
linear system, which the derivation above shows it cannot do unless the slow mode has decayed by about e^-(rate ratio).
