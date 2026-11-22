---
type: concept
tags: [foundation, pressure-velocity]
sources: [ref-mit-ocw-2-29-numerics, ref-jasak-1996-phd-thesis, ref-klaij-vuik-2013-simple-preconditioners]
builds_on: [incompressible-flow, finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Pressure-velocity coupling

## In short

In incompressible flow there are equations for the velocity (momentum) and a constraint on it (zero divergence), but no equation that says what the pressure is. The pressure is whatever it has to be for the velocity to stay divergence-free. Every incompressible solver therefore needs a way to tie pressure to the continuity constraint: either solve everything together, or alternate between momentum and a pressure equation built from continuity ([[ref-mit-ocw-2-29-numerics]] L18 s.4-5).

## Explanation

**No pressure equation.** The incompressible Navier-Stokes equations have no independent equation for pressure; continuity becomes a kinematic constraint on the velocity, and pressure acts so that the constraint holds ([[ref-mit-ocw-2-29-numerics]] L18 s.4). Taking the divergence of the momentum equation and using continuity gives an elliptic Poisson equation for the pressure; to keep the discrete velocity divergence free its Laplacian must be built from the discrete divergence of continuity and the discrete gradient of momentum, so it is "Best to derive pressure equation from discretized momentum/continuity equations" ([[ref-mit-ocw-2-29-numerics]] L18 s.5).

**Saddle-point structure.** After linearisation (Picard) and finite-volume discretisation the system is `[Q G; D C] [u; p] = [f; g]`: `Q` is the convection-diffusion operator of momentum, `G` the discrete pressure gradient, `D` the discrete divergence, `C` a stabilisation block that is zero without it ([[ref-klaij-vuik-2013-simple-preconditioners]] Eqs. 1, 12). The zero (or small) pressure-pressure block is what makes the system hard for standard iterative solvers.

**Explicit time stepping.** With forward Euler, requiring the velocity at the new step to be discretely divergence free gives a Poisson equation for the current pressure; the momentum update then satisfies continuity ([[ref-mit-ocw-2-29-numerics]] L18 s.7-9).

**Implicit time stepping and steady flow.** With implicit schemes pressure and velocity must be found together. Options: a nonlinear solver per step; linearise and solve the large coupled linear system ("Direct solution is not recommended: use an iterative scheme"); or a local linearisation with a pressure-correction Poisson equation ([[ref-mit-ocw-2-29-numerics]] L18 s.10-15).

**Segregated against coupled.** Jasak distinguishes simultaneous algorithms (Caretto et al., Vanka), which solve the complete system at once at great cost in operations and memory, from segregated algorithms such as PISO and SIMPLE, which solve the equations in sequence ([[ref-jasak-1996-phd-thesis]] 3.8.2). Segregated steady methods use outer iterations to update the nonlinear coefficients and inner iterations for the linear systems ([[ref-mit-ocw-2-29-numerics]] L18 s.16-17). Klaij and Vuik show that SIMPLE-type steps can also be used as preconditioners of a Krylov solver of the coupled system ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.2-3.3).

## Builds on

- [[incompressible-flow]]: the divergence-free constraint and the role of pressure.
- [[finite-volume-method]]: the discrete divergence and gradient operators.

## Leads to

- [[pressure-correction-equation]], [[collocated-grid-and-checkerboarding]], [[rhie-chow-interpolation]], [[projection-method]], [[block-coupled-solver]], [[under-relaxation]]
- rcfd pages: [[simple]], [[piso]]

## How rcfd uses it

- Segregated: `SimpleSolver` (SIMPLE/SIMPLEC, steady) and `PimpleSolver` (PISO/PIMPLE, transient), collocated with Rhie-Chow ([[simple]]; [[piso]]).
- Coupled: `solver.algorithm = "coupled"` solves `(u, v[, w], p)` in one linear system per Picard iteration, steady only ([[doc-case-schema-coupled]]); see [[block-coupled-solver]].
- Transient projection: `time_advance = "fractional_step"` is a recommended remedy for the cylinder wall-pressure spokes ([[doc-investigation-job48t-cylinder-spokes]]); see [[projection-method]].
