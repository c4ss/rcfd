# Why the steady SA drag of the NACA 0012 was wrong (job 49A, ADR 0164)

The set-up, the limits, amendment 1 and the evaluation are in
[ADR 0164](../../adr/0164-job49a-why-the-sa-drag-of-the-naca0012-is-wrong.md). This folder holds the results only. No run
or mesh enters git. To build the runs again, use `scripts/w49a_sa_drag.py` in an empty work folder outside the repository
(`mesh`, `cases`, `run`, `adopt`, `collect`; see the script's docstring). The reference is the NASA TMR NACA 0012 SA
validation case (CFL3D, 897 x 257 grid, 500 chords, nuTilda = 3 nu): cd 0.00819 at 0 degrees, of which the friction is
0.00683 and the pressure 0.00136; cl 1.0909 and cd 0.01231 at 10 degrees. The two files are in `validation-data/reference/w49a_*`.

## Finding

ADR 0159's SA drag had two independent causes. Neither is the SA model, its constants or its wall law.

1. **At 0 degrees (cd half of the measured value): the force report.**
   - The default `force_coefficient` takes the viscous stress with the molecular viscosity and the wall cell's u_P/dn.
   - On M1w the wall cells are at y+ about 40. There the solver applies the wall-function shear u_tau^2 through the face
     viscosity nu + nu_t,w, and the molecular value is only about u+/y+ of it.
   - On the aerofoil the reported friction is 0.32 of the applied friction; on the y+ 40 plate it is 0.35. At y+ 1 the
     two agree to 0.5 %.
   - With the opt-in component `total_effective`, cd at 0 degrees is **0.00848**, +3.6 % against TMR's 0.00819. Friction
     is 0.00706 against 0.00683, pressure 0.00142 against 0.00136.
   - SST's 0.0036 is the same report effect: arm S00 gives 0.003637 by default and 0.008317 with `total_effective`.
2. **From 4 degrees up (cd 1.4 to 4.6 times too high, rising for thousands of iterations): the far-field nuTilda.**
   - A `velocity = "freestream"` far field gets the turbulence role `zero_gradient`, so nothing holds nuTilda at 3 nu.
   - Around the lifting aerofoil it grows to **6.0e4 nu** in the whole outer flow at 10 degrees (24 nu at 0 degrees). That
     is an outer eddy viscosity of 0.01 U c.
   - It takes thousands of iterations to get there: the cd drift is this growth (A10: cd +19 % between iterations
     1001-2000 and 7001-8000).
   - With the opt-in role `boundaries.farfield.turbulence = "freestream"` (amendment 1: 3 nu on inflow faces, zero
     gradient on outflow faces):
     - the outer nuTilda stays at **3.000 nu**;
     - the run meets the residual targets at iteration 577 with cd constant to 5 digits;
     - at 10 degrees cl goes from 0.982 to **1.055** (TMR 1.091, -3.3 %) and cd from 0.0445 to **0.0155** (TMR 0.0123,
       +26 %).
   - On a 500-chord far field (C10) the same set-up gives cl **1.065** (-2.4 %) and cd **0.0135** (+10 %). Most of the
     remaining excess at 50 chords is the far-field extent; there is no point-vortex correction.

## Results (window means; `cd` = default report, `cd_eff` = `total_effective`)

| Arm | Set-up | Iterations | cl | cd | cd_eff | friction (eff.) | pressure | outer nuTilda/nu |
|---|---|---|---|---|---|---|---|---|
| A00 | ADR 0159, 0 deg | 5157 (targets) | 0.0000 | 0.003678 | 0.008484 | 0.007060 | 0.001424 | 23.8 |
| F00 | far field `freestream`, 0 deg | 261 (targets) | 0.0000 | 0.003645 | **0.008402** | 0.006999 | 0.001402 | 3.000 |
| A10 | ADR 0159, 10 deg | 8000 (cap) | 0.9817 | 0.03956 | 0.04452 | 0.007153 | 0.03737 | 6.02e4 |
| F10 | far field `freestream`, 10 deg | 577 (targets) | **1.0547** | 0.01146 | **0.01551** | 0.006016 | 0.009489 | 3.001 |
| C10 | F10 on 500 chords (report only) | 533 (targets) | 1.0650 | 0.009523 | 0.01355 | 0.005977 | 0.007570 | 3.000 |
| B00 | far field `inlet` (defective) | 7448 | -0.0003 | 0.0726 | 0.0842 | 0.0152 | 0.0689 | 5e71 |
| B10 | far field `inlet` (defective) | 5842 | 1.0525 | 0.172 | 0.182 | 0.0129 | 0.169 | 1e63 |
| TMR | CFL3D SA, 0 / 10 deg | | 0 / 1.0909 | | 0.00819 / 0.01231 | 0.00683 (0 deg) | 0.00136 (0 deg) | 3 |

- A00 and A10 reproduce ADR 0159's SA-a00 and SA-a10 cl and cd bit for bit: 10 314 and 16 000 values, no difference.
- B (`turbulence = "inlet"`, the registered far field) fixes nuTilda on the outflow faces too. The convection operator
  then carries the fixed value out instead of the cell value, so nuTilda cannot leave. B10 sat at F10's solution (cd_eff
  0.015484, cl 1.0550) for iterations 500 to about 1500. Then nut reached the solver's cap of 1e5 nu everywhere, and nuTilda
  grew without bound. See amendment 1.

**Flat plate** (SA, `rcfd run`, Re 5e6 per m; Cf at x = 0.97 against TMR SA 2.70565e-3)

| Arm | y+ at 0.97 | wall-law Cf(0.97) | molecular / wall-law | mean C_F (wall law) vs Kármán-Schoenherr 0.002934 |
|---|---|---|---|---|
| P01 | 0.99 | 0.002790 (+3.1 %) | 1.005 | +0.7 % |
| P05 | 5.3 | 0.003245 (**+19.9 %**) | 0.974 | +17.7 % |
| P40 | 39.6 | 0.002822 (+4.3 %) | **0.352** | +2.1 % |

- The `viscous_effective` report equals the integral of the wall-law Cf to 0.25 % (P01) and 0.02 % (P40).
- At y+ 5, the buffer layer, SA's wall mode `auto` over-predicts the friction by 20 %. This is a separate wall-treatment
  item. M1w's wall cells span y+ 5.5 to 61 (median 42), so some of them are in that range. Where they are was not mapped.

## Files

| File | Contents |
|---|---|
| `results.json` | every run: exit, iterations, window statistics of every report, outer nuTilda, plate wall data, the bit comparison with ADR 0159; the evaluation of every limit and the reported values |
| `results.csv` | one row per run |
| `cd_histories.png` | cd against iteration (default and `total_effective`): A10's drift, F10 and C10 settling |
