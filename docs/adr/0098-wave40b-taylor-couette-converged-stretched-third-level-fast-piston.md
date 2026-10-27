# ADR 0098: Wave 40B, partials close-out for F-204 and F-208, and a Mach 0.2 piston check for F-230

Status: registered before the registered tests were run. Limits are fixed; a miss becomes an `#[ignore]`d `*_known_miss` test
with the limit unchanged. No library code changes: every addition is test code, so defaults and the baseline hashes are
untouched. Clean-room: the only reference values are closed forms derived in ADR 0016 (Taylor-Couette torque, manufactured
solution) and ADR 0090 (isentrope); no literature value enters a gate.

The gaps are those stated in docs/audit/partials-group-4.md (F-204) and docs/audit/partials-group-8.md (F-208). I re-checked
them against the code before writing this ADR: `d5c_taylor_couette_torque` (crates/rcfd-api/tests/wall_motion.rs) runs the case
with `max_iterations = 6000` and asserts only the torque, not `is_converged()`; `m1_m4_order_and_option_effects`
(crates/rcfd-fv/tests/mms_poor_meshes.rs) uses grids n = 8, 16, 32 only, so the stretched-mesh order 1.744 comes from the
finest pair 16 -> 32 with the coarse pair at 1.594.

## F-204: Taylor-Couette torque on a converged finest grid

Set-up: the ADR 0016 D5 case unchanged (`cylinder_ogrid`, 128 x n_r, r 1 to 2, depth 1, nu 0.1, inner Omega 1, outer -0.5,
SIMPLEC, linear upwind, one non-orthogonal corrector, `tolerance_u = tolerance_p = 1e-11`, viscous moment about z on
`cylinder`), n_r = 128 only. Registered set-up change (not a solver change): `solver.max_iterations` 6000 -> 20000; the residual
tolerances stay at 1e-11. Exact torque magnitude `4 pi mu B = 2.513274` per unit depth (ADR 0016 derivation).

New test D5d `d5d_taylor_couette_torque_converged`. Limits: `s.is_converged() == true` (asserted) and
`|M / M_exact - 1| <= 0.01`.

Smoke (ignored scratch test on 85705c7, not kept): the D5 case at n_r = 128 with the 20000 cap, stepped in chunks of 1000
iterations: torque -2.4704335 / -2.5004316 / -2.5023231 / -2.5024422 / -2.5024496 / -2.5024501 / -2.5024501 after 1000 ... 7000;
res_u 1.79e-5, 1.12e-6, 7.07e-8, 4.45e-9, 2.80e-10, 1.762e-11, 9.965e-12; res_p 1.29e-5 ... 1.365e-11, 9.971e-12; the
converged flag is set by 7000 iterations (580 s). The residual fell by about 16 per 1000 iterations down to 6000 and by only
1.8 from 6000 to 7000, so it is close to a floor just under 1e-11; the converged flag is reached with little margin (res_u
0.9965 of the tolerance). The run repeats: at 6000 iterations the smoke matches the ADR 0016 diagnostic to all its printed
digits (res_u 1.76e-11, res_p 1.37e-11, torque -2.502450). The tolerance is kept at 1e-11; only the cap changes.

What I knew when registering: the ADR 0016 record (torque -2.502450 at 6000 iterations, relative error -4.307e-3, residuals
1.76e-11 / 1.37e-11) and the smoke above. The smoke was the registered set-up run to convergence, so the D5d outcome was known when
this was written; the limits (converged flag, 1 percent) come from the task and the roadmap acceptance, not from that run. The
registered run must repeat the smoke digits; a difference is reported.

Decision: D5d passes -> F-204 is set to done (the plane Couette and thermocapillary clauses already pass in non-ignored tests
D1, D4); D5d fails -> it becomes `d5d_known_miss_...` with the limits intact and F-204 stays partial.

## F-208: third refinement level on the stretched mesh

Set-up: the ADR 0016 M1 problem unchanged (convection-diffusion of the manufactured solution
`1 + sin(1.3 x + 0.4) cos(1.1 y) cos(0.9 z + 0.2)`, u = (1, 0.7, 0.4), Gamma 0.1, central convection, Dirichlet exact values on
all patches, L2 error against the exact cell-centre values) on the stretched mesh S (`box_mesh([n; 3])`, unit cube,
`Grading::Expansion(50)` in y), option sets C0 (none), C1 (face correction, corrected boundary gradient), C2 (C1 plus a
Venkatakrishnan(5) secondary limiter). Grids n = 16, 32, 64 (64 is new).

Registered solver tolerance: the unchanged `solve` rule of the file, i.e. outer deferred-correction sweeps until the largest
cell change is `<= 1e-13` of the solution range, at most 600 sweeps; inner solve `tol_rel = 1e-13`, `max_iter = 4000`.

New test M5 `m5_stretched_mesh_third_level_order`. Limits: every run converged under that rule, and the L2 order
`log2(e_32 / e_64) >= 1.7` for each of C0, C1 and C2 (the user-set threshold; stricter than the ADR 0016 limit of 1.5 for C2,
which on this orthogonal mesh has been inert: identical orders for all three sets). The 16 -> 32 order is printed only.

Smoke (scratch test on 85705c7 with the sweep cap as a parameter, not kept): C0 at n = 32 converges in 19 sweeps
(change/range 5.7e-14, 0.9 s), L2 error 2.091082e-4; C0 at n = 64 for 8 sweeps only takes 7.4 s, the change/range falls by a
factor of about 7 per sweep (3.6e-8 at sweep 8), L2 error 5.571089e-5 at that point. What I knew when registering: that
unconverged n = 64 error (it would give an order of about 1.91) and the ADR 0016 orders (S: 1.594 / 1.744 for all sets).

Convergence-failure branch (registered now): if a run fails only the convergence rule while its order is in band, that is
a set-up defect; M5 stays as an ignored record and a corrected test is registered by amendment before any re-run.

Decision: M5 passes -> F-208 is set to done (the skewed and warped orders and the good-mesh no-degradation checks already
pass in the non-ignored `m1_m4_order_and_option_effects`); M5 fails -> `m5_known_miss_...` with the limits intact, F-208 stays
partial.

## F-230 (optional): Mach 0.2 piston

F-230 is done already (ADR 0090, peak piston Mach 0.05). This check does not change its status. Set-up: the ADR 0090 piston
case (40 layers, 2 x 2 cross-section 0.1 x 0.1, rest state rho 1, p 1, gamma 1.4, HLLC / van Leer / SSP-RK3 CFL 0.4, layer
removal below 0.4 h0 and split above 1.6 h0, `x_p = 1 - 0.3 (1 - cos(pi t / P))`) with the stroke period
`P = 0.3 pi / (0.2 c0) = 3.9827` (c0 = sqrt(1.4)), so the peak piston speed is Mach 0.2 at c0. The test body is the ADR 0090
one, refactored into `piston_run(period, strokes)`; the existing test calls it with `P = 16`, 2 strokes (same operations).

New test `piston_mach02_layering`. Limits: the ADR 0090 ones, A1 and A2 `|p_bar V^gamma / (p0 V0^gamma) - 1| <= 0.02`
at t = P and t = 2P, B1 `<= 1e-13`, B2 `<= 1e-12`, B3 `<= 1e-13`, B4 each `>= 5`, B5 = 0.

Why the 2 percent limit is not a physics bound here: at Mach 0.2 the gas is no longer in uniform quasi-static compression. At
t = P the piston is at rest but pressure waves remain; their kinetic energy is of order `gamma (gamma - 1) / 2 * M^2 / 3`
of the internal energy if the whole column moved with the linear piston profile, about 3.7e-3, and the volume-mean pressure
differs from the isentrope at the same order. That estimate is below the limit by about 5, so a miss would point at the
numerics (entropy production of the scheme or the remap), not at the physics. The time step still uses `|u| + c` rather than
`|u - w| + c` (ADR 0090 shortcut); with mesh speed `|w| <= 0.24` the true signal speed `|u - w| + c` exceeds `|u| + c` by at
most `0.24 / 1.18`, so the effective CFL stays below 0.49.

Smoke (ignored `piston_mach02_smoke`, kept): `piston_run(P, 0.25)`, a quarter stroke (285 steps, 0.4 s): V/V0
0.912, 3 removals, p_bar V^gamma - 1 = 2.28e-4, B1 3.3e-16, B2 1.3e-14, B3 6.7e-16, p/rho^gamma spread [0.999994, 1.000063]. What I
knew when registering: those numbers and the ADR 0090 results at Mach 0.05.

## Addendum: measured (registered runs on the work tree of d5bdf33, release)

| Item | Check | Measured | Limit | Verdict |
|---|---|---|---|---|
| F-204 | D5d converged flag, n_r = 128, cap 20000 | `true` after 6206 iterations (res_u 9.965e-12, res_p 9.971e-12) | `true`, tolerance 1e-11 | met |
| F-204 | D5d torque | -2.5024501, relative error -4.307e-3 | `|.| <= 0.01` | met |
| F-208 | M5 converged under the `solve` rule, S, n = 16, 32, 64, C0 / C1 / C2 | all nine runs converged | all | met |
| F-208 | M5 L2 order 32 -> 64 on S | 1.908 for C0, C1 and C2 (errors 2.0911e-4 -> 5.5711e-5) | `>= 1.7` | met |
| F-230 | Mach 0.2 piston A1 / A2 | 2.935e-4 / 4.345e-4 (max over all steps 4.372e-4) | `|.| <= 2e-2` | met |
| F-230 | B1 / B2 / B3 | 4.44e-16 / 1.35e-13 / 8.88e-16 | 1e-13 / 1e-12 / 1e-13 | met |
| F-230 | B4 / B5 | 24 removals, 24 additions / 0 | each >= 5 / 0 | met |

Repeat check: the D5d torque and final residuals equal the smoke digits (the smoke printed every 1000 iterations and stopped at
the first chunk boundary after convergence, so it printed "7000"; the run converged at iteration 6206). On S the three option
sets give the same errors to 1e-14 relative, as expected on an orthogonal mesh where the secondary-gradient terms vanish; the
16 -> 32 order is 1.744 as in ADR 0016, and the new pair shows the asymptotic range is reached (1.908). The M5 n = 64 error
equals the smoke value after 8 sweeps to 2e-6 relative.

F-230: the refactor into `piston_run` changed nothing in the registered Mach 0.05 run (1.528e-5 / 3.415e-5, B2 5.37e-13, 24 / 24
events, 9192 steps, as in ADR 0090). At Mach 0.2 the isentrope deviation grows by a factor 19 (A1) and 13 (A2), and the
p/rho^gamma spread at t = P grows to [0.999990, 1.000922] from [0.999986, 1.000137]; both stay far inside the limit. The run
takes 2394 steps, 1.8 s. F-230 status unchanged (done).

Roadmap: F-204 and F-208 set to done under the registered decision rules. Gaps that stay outside the acceptance (to be split
into new items by the lead, not created here to avoid id clashes): F-204 turbulence with the new wall kinds, MRF
Taylor-Couette, the D5b rigid-rotation velocity known miss, D2 specified shear still reporting `converged false`; F-208
pressure-interpolation variants, a warped-face correction separate from the face correction, case-tree wiring of
`GradientOptions`. No deviations from the registered plan.
