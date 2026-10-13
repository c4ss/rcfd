# Backward-facing step: why second-order SIMPLE did not converge (wave 6, F-106 / F-042)

Scope: rcfd-fv `SimpleSolver` (SIMPLE/SIMPLEC, collocated, Rhie-Chow), backward-facing step (BFS),
Re_h >= 200 with second-order convection (linearUpwind, vanLeer), cavity Re = 1000.

## Root cause

`SimpleSettings.h_from_solution` defaulted to `false`: the H operator (neighbour part of the
momentum matrix, used for `HbyA` and the Rhie-Chow face flux) was evaluated from the *previous
iterate* instead of from the solved momentum predictor. That makes the pressure-velocity step a
Jacobi-like lagged iteration that is only marginally stable. With first-order upwind the extra
numerical diffusion hides it; with deferred-correction second-order schemes on a mesh with cell
Peclet number ~50 (Re_h = 200, dx = 0.25 h) it is unstable.

Evidence (uniform inlet, expansion ratio 2, l_in 4, l_out 20, 16+80 x 8+8 cells, Re_h = 200,
linearUpwind, SIMPLE 0.7/0.3, residual = max(res_u, res_p)):

| variant | outcome |
|---|---|
| H from previous iterate (old default) | no convergence in 3000 its; residual 0.27-0.30, max update 0.3-0.5, max v of 0.8-1.2 (U = 1) |
| same, but l_out = 10 (outlet 14 h from inlet) | converges (1500 its, 4e-7) |
| same, l_out = 14, 17, 20 | wave of order-one amplitude, wavelength ~10 cells, in the last 1-9 h of the channel; never converges |
| same, l_out = 14, dx = 0.5 or dx = 0.125 | converges (1170 / 397 its) -> marginal, mesh-dependent iteration instability, not a physical unsteadiness |
| same, l_out = 14, alpha_u/alpha_p = 0.5/0.2 or 0.3/0.1 | converges (lower relaxation hides it) |
| central differencing, same mesh | converges in 219 its |
| pseudo-transient local cfl 1 | converges (slow) |
| **H from solved predictor** | converges: 646 its (l_out 20), 628 (l_out 14); Re_h = 400 vanLeer 1004 its |

The velocity field is therefore not physically unsteady: the failure is a numerical iteration
instability seeded near the outlet and amplified downstream.

Things checked and found *not* to be the cause: outlet treatment (zero-gradient velocity, fixed
pressure; outlet at a distance changes nothing once H is consistent), inlet profile (uniform and
parabolic both fine), pressure reference (fixed-value outlet pressure, no reference cell used),
non-orthogonality (orthogonal block mesh), residual normalisation (the same stalls are visible
in max velocity update and in max |v|), and the deferred-correction limiter for the main failure.

## Fix

* `h_from_solution` now defaults to **true** (classic SIMPLE / simpleFoam behaviour). The old
  behaviour is still available with `h_from_solution = false`.
* While a `MeanVelocityDriver` is active the lagged H is still used: the driver's `1/rAU` gain
  assumes the lagged response and goes unstable (NaN) with the solved-predictor H. This keeps
  the periodic bulk-velocity test (F-107) unchanged.

Side effects measured (release, tests/pseudo.rs): lid-driven cavity Re = 1000, 32x32, linearUpwind,
plain SIMPLEC alpha_u = 0.95 and no pseudo-transient now converges in 442 iterations (before:
did not converge in 4154); SIMPLE 0.7/0.3 converges in 660 (before 1900). BFS Re_h = 100: SIMPLE
240 its (before 646), pseudo cfl 5 135 (before 571). So the wave-5 premise "plain SIMPLE needs
lower relaxation than pseudo-transient on the cavity at Re 1000" came from this defect; the test
now records the legacy failure explicitly (`h_from_solution = false` still fails) next to the new
behaviour.

## Remaining robustness limits (not fixed, recorded)

Laminar step, ER 2, parabolic inlet (mean 1), Re_h = 400 (Armaly/Gartling Re = 800), 120 x 16
cells, l_out 30:

| scheme / coupling | result |
|---|---|
| vanLeer, SIMPLE 0.7/0.3 | limit cycle, residual 0.02-0.2 (limiter chatter), no convergence in 4000 its |
| vanLeer, SIMPLE 0.5/0.2, 0.3/0.1 | no convergence (0.15 / 0.24) |
| vanLeer, SIMPLEC 0.9 | converges, 696 its |
| linearUpwind, SIMPLE 0.7/0.3 | converges, 2795 its |
| linearUpwind, SIMPLEC 0.95 | no convergence in 3000 its (residual 0.4) |
| vanLeer, SIMPLEC alpha_u = 1 + local pseudo cfl 5 | no convergence in 4000 its (residual 4.9e-2) |
| first-order upwind, SIMPLE 0.7/0.3 | slow (4e-3 after 3000 its) |

Re_h = 400 with a TVD limiter is therefore sensitive to the coupling/relaxation choice
(non-smooth limiter chatter; SIMPLEC with a smooth scheme is marginal). Re_h <= 200 is robust
(SIMPLE and SIMPLEC, linearUpwind and vanLeer, all converge). No automatic setting was added
beyond the H default; the working recipe is vanLeer + SIMPLEC alpha_u 0.9.

Also found in rcfd-turb, now fixed (F-042): in the Driver and Seegmiller k-epsilon runs the
nu_t of some cells kept oscillating by O(1) (relative) between iterations. A top-N listing of the
changing cells showed all of them wall-adjacent (step face x = 5.04 and floor), i.e. the hard
viscous/log switch of the wall function at y* = 11.225 (epsilon jumps about 4x, production 0 to
log-law). `WallTreatment::Smooth` (default; compound epsilon and blended production) and
`WallTreatment::Hysteresis` (+-10 percent dead band) remove it: with the unchanged criterion
(max velocity update < 1e-7 and nu_t change < 1e-6) all three variants now converge in 800 to 920
iterations on the medium mesh (`Switch`: du 2e-4 to 2e-3, nu_t change 0.4 to 1.1, never).
Reattachment x_r/H is unchanged by the treatment (< 0.003): Standard 5.359, RNG 6.373,
Realizable 6.402 (medium mesh, smooth); fine mesh 5.460, 6.380, 6.506. `Switch` and `Hysteresis`
give 5.358/6.373/6.404 and 5.358/6.372/6.404.

## Laminar reattachment length (tests/step_laminar.rs, release)

Expansion ratio 2 (inlet channel height = step height h = 1), l_in 2, parabolic inlet with mean
velocity 1, Re_h = U_mean h / nu, so Armaly's Re = U_mean 2 h_in / nu = 2 Re_h. Reattachment = last
negative-to-positive sign change of u in the first cell row above the floor, linear interpolation,
in step heights. vanLeer convection.

| Re_h (Re_Armaly) | mesh (cells downstream x per channel height) | x_r/h |
|---|---|---|
| 100 (200) | 120x16 / 240x32 | 4.92 / 4.95 (first-order upwind, coarse: 4.23) |
| 200 (400) | 120x16 / 240x32 | 8.03 / 8.19 (upwind coarse: 6.93) |
| 389 (778) | 320x32 | 11.54 |
| 400 (800) | 120x16 / 320x32 / 320x32 graded (x ratio 4, y ratio 3) / 480x48 | 11.24 / 11.67 / 11.70 / 11.75 |

Re_h = 400 is not yet mesh independent (each refinement adds 0.05-0.4 and the sequence is
flattening toward about 11.8-11.9).

Literature (NOT verified in this wave, flagged as from memory; the web search did not return the
tables): Gartling (1990, Int. J. Numer. Methods Fluids 11, 953, the benchmark with the parabolic inlet,
expansion ratio 2, Re = 800) primary reattachment about 12.2 step heights; Armaly, Durst, Pereira
and Schonung (1983, J. Fluid Mech. 127, 473) measured about 12 at Re = 800 with three-dimensional
effects and ER 1.94. For Re_Armaly = 200 and 400 I recall roughly 5 and 8.5-9 respectively (2D
computations), but I cannot cite a table, so no pass/fail is claimed there. At Re_Armaly = 800 the
finest mesh is 3.7 percent below the remembered 12.2 and still rising. The Re_Armaly = 800
outlet was 40-60 h downstream; the 30 h outlet used on the coarse mesh shortens x_r slightly.
