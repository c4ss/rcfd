---
type: concept
tags: [foundation, compressible, shock]
sources: [ref-naca-1953-report-1135, ref-liou-2001-nasa-tm-210977, ref-van-leer-1992-icase-92-43]
builds_on: [euler-equations]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Shock waves

## In short

A shock wave is a very thin region in a supersonic flow across which pressure, density and temperature jump up and the velocity drops. Across a normal shock the flow goes from supersonic to subsonic. The isentropic relations do not hold across it; instead mass, momentum and energy fluxes are equal on both sides, and the entropy must rise ([[ref-naca-1953-report-1135]] pp. 618-619).

## Explanation

**Jump conditions.** For a steady normal shock (upstream state 1, downstream state 2, velocities normal to the shock), conservation gives ([[ref-naca-1953-report-1135]] p. 618, fig. 1, Eqs. 84-86):

- mass `rho_1 u_1 = rho_2 u_2` (Eq. 84)
- momentum `p_1 + rho_1 u_1^2 = p_2 + rho_2 u_2^2` (Eq. 85)
- energy `u_1^2/2 + h_1 = u_2^2/2 + h_2` (Eq. 86a)

with the requirement that the entropy does not decrease, `Delta s = s_2 - s_1 >= 0` (Eq. 87). These are the integral forms of the [[euler-equations]] across a discontinuity. Total enthalpy, total temperature and the critical speed of sound are the same on both sides (Eq. 88), because the shock is adiabatic.

**Which way.** Combining the three gives Prandtl's relation `u_1 u_2 = a_*^2` (Eq. 89), "which implies that the flow is supersonic ahead of the shock wave and subsonic behind (the reverse possibility is ruled out by the requirement of nondecreasing entropy)" ([[ref-naca-1953-report-1135]] p. 619). A jump from subsonic to supersonic (an expansion shock) satisfies the conservation laws but not the entropy condition.

**Rankine-Hugoniot relations.** Eliminating the velocities gives the pressure-density relation across the shock: `p_2/p_1 = [(gamma+1) rho_2 - (gamma-1) rho_1]/[(gamma+1) rho_1 - (gamma-1) rho_2]` (Eq. 90), and its inverse (Eq. 91) ([[ref-naca-1953-report-1135]] p. 619). This differs from the isentropic law `p/rho^gamma = const`, which holds on either side but not across ([[ref-naca-1953-report-1135]] p. 618).

**In terms of the upstream Mach number** `M_1` ([[ref-naca-1953-report-1135]] p. 619):

- `p_2/p_1 = (2 gamma M_1^2 - (gamma-1))/(gamma+1)` (Eq. 93)
- `rho_2/rho_1 = u_1/u_2 = (gamma+1) M_1^2/((gamma-1) M_1^2 + 2)` (Eq. 94)
- `M_2^2 = ((gamma-1) M_1^2 + 2)/(2 gamma M_1^2 - (gamma-1))` (Eq. 96)
- total pressure falls, `p_t2/p_t1 = e^(-Delta s/R)` (Eq. 99): the loss of total pressure is the entropy rise.

As `M_1 -> 1` all ratios tend to one: a weak shock is nearly isentropic, its entropy rise being third order in `(M_1^2 - 1)` (Eq. 104).

**Oblique shocks and expansions.** NACA 1135 also gives oblique-shock and Prandtl-Meyer expansion relations; expansions round a convex corner are isentropic and continuous, unlike compressions that steepen into shocks ([[ref-naca-1953-report-1135]] p. 626).

**Shocks in a numerical scheme.** A finite-volume scheme in conservation form captures shocks as steep but resolved jumps. Upwind schemes do this well in 1D ("crisply", [[ref-van-leer-1992-icase-92-43]] sec. 3), while waves not aligned with the grid are smeared in multi-D (abstract). The choice of interface flux decides shock resolution and robustness, for example freedom from shock instabilities ([[ref-liou-2001-nasa-tm-210977]] "How It Began"); see [[approximate-riemann-solvers]].

**What it adds.** The [[euler-equations]] admit discontinuous solutions; this page gives the jump conditions they imply and the entropy condition that selects the physical one.

## Builds on

- [[euler-equations]]: conservation form, integrated across the jump.
- [[ideal-gas-and-thermodynamics-basics]]: enthalpy and entropy (via the Euler equations).

## Leads to

- [[riemann-problem-and-godunov-method]] (the shock is one of the waves of the Riemann problem)
- [[doc-case-schema-compressible]] (shock tube initial condition)

## How rcfd uses it

- The compressible solver can start from a `riemann` initial state split along `split_axis` at `split_position`, the shock-tube set-up, and offers TVD-limited reconstruction to keep shocks free of oscillations ([[doc-case-schema-compressible]]).
- The reference shock positions for the NACA 0012 Euler case at `M = 0.8` are "recollection only", "UNVERIFIED, not used"; the F-062 airfoil criterion is "not attempted" ([[doc-reference-data-compressible-bcs]]).
