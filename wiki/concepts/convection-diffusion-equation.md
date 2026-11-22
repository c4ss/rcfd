---
type: concept
tags: [foundation, discretisation]
sources: [ref-jasak-1996-phd-thesis, ref-langtangen-linge-2017-finite-difference-computing, ref-mit-ocw-2-29-numerics]
builds_on: [conservation-law]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Convection-diffusion equation (scalar transport)

## In short

The model equation for one transported quantity `phi` (a temperature, a concentration, one velocity component): it is carried along by the flow (convection), spreads out (diffusion), and may be produced or destroyed (source). Almost every discretisation question in CFD is first asked on this equation ([[ref-jasak-1996-phd-thesis]]).

## Explanation

**The equation.** For density `rho`, velocity `U`, diffusivity `Gamma_phi` and source `S_phi(phi)`: `d(rho phi)/dt + div(rho U phi) - div(rho Gamma_phi grad phi) = S_phi(phi)`, with the four terms temporal, convection, diffusion and source ([[ref-jasak-1996-phd-thesis]] 3.3, Eqn. 3.2). Its integral form over a control volume is the starting point of the [[finite-volume-method]] ([[ref-jasak-1996-phd-thesis]] Eqn. 3.8).

**Character of the terms.** Convection is first order in space and moves information in the flow direction; diffusion is second order and spreads information in all directions. Convection is the term that "causes problems" for the accuracy and boundedness of a scheme ([[ref-jasak-1996-phd-thesis]] 3.4).

**One parameter.** The 1D stationary model `v u' = alpha u''` on `[0, L]` with fixed end values, scaled by `L` and the boundary values, becomes `u' = epsilon u''` with `u(0) = 0`, `u(1) = 1`, where `epsilon = alpha/(vL)` measures diffusion against advection; the exact solution `(e^{x/epsilon} - 1)/(e^{1/epsilon} - 1)` has a boundary layer at the outflow end when `epsilon` is small and is close to linear when it is large ([[ref-langtangen-linge-2017-finite-difference-computing]] 4.2.1, Eqn. 4.13). Centred differences give non-physical oscillations for small `epsilon` unless the mesh is fine enough; the solution is oscillation-free if `dx/epsilon <= 2`, a mesh-level ratio of advection to diffusion; the upwind difference removes the oscillations but thickens the layer ([[ref-langtangen-linge-2017-finite-difference-computing]] 4.2.2-4.2.3). See [[upwind-and-central-differencing]] and, for the Peclet number, [[dimensionless-numbers]].

**Boundedness.** If the exact solution has no sources and fixed boundary values, it stays between its boundary and initial extremes; a scheme that keeps the discrete solution within these bounds is bounded. Diagonally dominant matrices with positive coefficients guarantee it; for convection only upwind differencing gives such a matrix unconditionally ([[ref-jasak-1996-phd-thesis]] 3.4, 3.5). See [[tvd-schemes-and-flux-limiters]].

**Sources.** A source that depends on `phi` is linearised as `S_phi = S_u + S_p phi` and treated "as implicitly as possible": `S_p` goes into the matrix diagonal when it is negative, which improves diagonal dominance ([[ref-jasak-1996-phd-thesis]] 3.3.1.4, Eqns. 3.35-3.36).

**Where it shows up.** Each momentum component of the Navier-Stokes equations is a convection-diffusion equation with the pressure gradient as an extra source ([[ref-mit-ocw-2-29-numerics]] L17 s.8; [[ref-jasak-1996-phd-thesis]] 3.8).

## Builds on

- [[conservation-law]]: the equation is the differential form of a balance of one quantity.

## Leads to

- [[upwind-and-central-differencing]], [[tvd-schemes-and-flux-limiters]], [[finite-volume-method]], [[momentum-matrix-and-h-operator]]
- rcfd pages: [[vof]] (alpha transport)

## How rcfd uses it

- VOF transports the volume fraction alpha as a convected scalar with interface compression ([[vof]]).
- The k and omega (or epsilon, nu-tilde) equations of the RANS models are convection-diffusion equations with source terms; their convection scheme choice matters on FAN-01, where `linearUpwind` diverged for k/omega ([[k-omega-sst]]).
