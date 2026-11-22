---
type: concept
tags: [foundation, boundary-layer, laminar]
sources: [ref-davidson-turbulence-notes]
builds_on: [boundary-layer]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Blasius solution

## In short

Blasius (1907) showed that the laminar boundary layer on a flat plate with no pressure gradient has the same velocity-profile shape at every station once the wall distance is scaled by `sqrt(nu x/U)`. The profile comes from one ordinary differential equation, and it gives exact check values: `delta_99 = 5 sqrt(nu x/U)`, `delta* = 1.721 sqrt(nu x/U)`, `theta = 0.664 sqrt(nu x/U)` and `Cf = 0.664/sqrt(Re_x)` ([[ref-davidson-turbulence-notes]]).

## Explanation

**From PDE to ODE.** Start from the steady 2D laminar [[boundary-layer]] equations with `dp/dx = 0` ([[ref-davidson-turbulence-notes]] eq. 3.42). A stream function `psi` (`U = d psi/dy`, `V = -d psi/dx`) satisfies continuity automatically and turns the momentum equation into a third-order PDE for `psi` (eqs. 3.44-3.46). Introduce the similarity variable and the scaled stream function (eq. 3.48):

`xi = y sqrt(U_inf/(nu x))`,  `psi = sqrt(nu U_inf x) g(xi)`,  so `U/U_inf = g'(xi)`.

Substituting gives the Blasius equation (eq. 3.54):

`g''' + (1/2) g g'' = 0`,  with `g(0) = g'(0) = 0` (no slip) and `g'(inf) = 1`.

It has no closed-form solution; Blasius "derived (and solved numerically!)" it in his 1907 thesis, and the tabulated solution has `g''(0) = 0.3320573362` (Table 3.1, p. 49).

**Check values** ([[ref-davidson-turbulence-notes]] App. E, eqs. E.1-E.3, pp. 348-349):
- boundary-layer thickness `delta_99 = 5 (nu x/U_inf)^(1/2)`;
- displacement thickness `delta* = 1.721 (nu x/U_inf)^(1/2)`;
- momentum thickness `theta = 0.664 (nu x/U_inf)^(1/2)`;
- skin friction `Cf = tau_w/((1/2) rho U_inf^2) = 0.664/sqrt(U_inf x/nu)`.

The last follows from the wall shear `tau_w = mu (dU/dy)_wall = mu U_inf sqrt(U_inf/(nu x)) g''(0)`, so `Cf = 2 g''(0)/sqrt(Re_x)`: the factor 0.664 is twice `g''(0)`.

**What it adds to its prerequisite.** The [[boundary-layer]] page gives the equations and thickness definitions; Blasius gives their exact solution for the simplest case, so a solver's laminar wall friction and thickness growth can be checked against numbers, not just trends. Its `x^(1/2)` growth and `Re_x^(-1/2)` friction law are also the laminar baseline against which turbulent correlations are compared ([[skin-friction-coefficient]]).

## Builds on

- [[boundary-layer]]: the thin-layer equations and thickness definitions.
- floor: ordinary differential equations, similarity variables.

## Leads to

- [[skin-friction-coefficient]], [[flat-plate]]

## How rcfd uses it

- rcfd's flat-plate correlations include Blasius `Cf = 0.664/sqrt(Re_x)`; wave 30B took Blasius to 32 digits from arXiv:2204.02215, `f''(0) = 0.33205733621519629893718`, `delta* = 1.72078765752 sqrt(nu x/U)` ([[flat-plate]]).
