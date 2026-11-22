---
type: concept
tags: [foundation, boundary-layer, validation]
sources: [ref-nasa-tmr, ref-davidson-turbulence-notes]
builds_on: [shear-stress, boundary-layer]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Skin-friction coefficient

## In short

The skin-friction coefficient is the wall shear stress made dimensionless with the dynamic pressure of the outer flow: `Cf = tau_w / ((1/2) rho U^2)`. It is the standard number for wall friction; laminar and turbulent boundary layers each have correlations for it as a function of a Reynolds number ([[ref-nasa-tmr]], [[ref-davidson-turbulence-notes]]).

## Explanation

**Definition** ([[ref-nasa-tmr]] `flatplate_val.html`; [[ref-davidson-turbulence-notes]] eq. E.3):

`c_f = tau_w / ((1/2) rho_inf U_inf^2)`,  `tau_w = mu_w (dU/dy)_w`

`tau_w` is the [[shear-stress]] at the wall, `mu_w` the wall viscosity, `(dU/dy)_w` the wall-normal gradient of the wall-parallel velocity, `rho_inf` and `U_inf` the free-stream density and speed. `Cf` changes sign where the wall shear does; the measured reattachment point of the TMR backward-facing step is the "zero skin-friction location" ([[ref-nasa-tmr]] `backstep_val.html`; [[flow-separation-and-reattachment]]).

**Laminar.** On a flat plate, the [[blasius-solution]] gives `Cf = 0.664/sqrt(Re_x)`, `Re_x = U_inf x/nu` ([[ref-davidson-turbulence-notes]] p. 349).

**Turbulent.** No exact solution; correlations fitted to data. The TMR uses the Karman-Schoenherr relation in terms of the momentum-thickness Reynolds number ([[ref-nasa-tmr]] `flatplate_val.html`):

`c_f = (17.08 (log10 Re_theta)^2 + 25.11 log10 Re_theta + 6.012)^(-1)`

and plots White's correlations in `Re_x` with 5 % error bars, "although it is not clear what the error levels actually are"; "these particular theoretical correlations are not necessarily perfect" (same page). In channel flow the turbulent velocity profile is much flatter in the centre, which makes the near-wall gradient, and so the wall shear stress, much larger than in laminar flow ([[ref-davidson-turbulence-notes]] p. 96).

**Relation to wall units.** `Cf` and the friction velocity are two forms of the same wall stress: `u_tau = U_inf sqrt(Cf/2)` ([[friction-velocity-and-y-plus]]). A friction correlation is therefore how a mesh's first-cell height is chosen for a target `y+` before any solution exists.

**What it adds to its prerequisites.** [[shear-stress]] gives `tau_w`; the [[boundary-layer]] gives the outer velocity and the Reynolds numbers. `Cf` combines them into a dimensionless measure that can be compared across speeds, fluids and codes.

## Builds on

- [[shear-stress]]: the wall shear stress `tau_w`.
- [[boundary-layer]]: the outer velocity and `Re_x`, `Re_theta`.

## Leads to

- [[friction-velocity-and-y-plus]], [[flow-separation-and-reattachment]], [[flat-plate]], [[spalart-allmaras]]

## How rcfd uses it

- Flat-plate correlations in `cases/flat_plate/reference/correlations.csv`: Blasius `0.664/sqrt(Re_x)`, `Cf = 0.0576 Re_x^-0.2` for `5e5 < Re_x < 1e7`, White's `0.455/ln^2(0.06 Re_x)` ([[flat-plate]]).
- SST against TMR CFL3D: `Cf(x = 0.97) = 0.00268299`, rcfd `0.002663` (-0.7 %); F-043 known miss against White (-7.36 %) ([[flat-plate]], [[k-omega-sst]]).
- SA: misses the power law at x = 1.0 by `+5.29%` against a 5 % limit (F-041) ([[spalart-allmaras]]).
- White's formula is used for `y+`-targeted first-cell heights ([[doc-reference-data-sst-wall]]); the fan meshes use `Cf = 0.0576 Re_c^(-1/5)` ([[fan-passage-mesh]]).
