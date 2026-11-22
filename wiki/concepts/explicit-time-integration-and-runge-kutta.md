---
type: concept
tags: [foundation, time-stepping]
sources: [ref-langtangen-linge-2017-finite-difference-computing, ref-jasak-1996-phd-thesis, ref-venkatakrishnan-1995-icase-95-3, ref-mit-ocw-2-29-numerics]
builds_on: [truncation-error-and-order-of-accuracy]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Explicit time integration and Runge-Kutta methods

## In short

An explicit scheme computes the new value directly from known (old) values: no linear system per step, so each step is cheap. The price is a stability limit on the time step (a Courant or diffusion number limit). Forward Euler is the simplest explicit scheme and is first order; Runge-Kutta methods evaluate the right-hand side several times per step to reach higher order ([[ref-langtangen-linge-2017-finite-difference-computing]]; [[ref-jasak-1996-phd-thesis]] 3.3.2).

## Explanation

**Forward Euler.** For `du/dt = f(u)`: `u^{n+1} = u^n + dt f(u^n)`; for a system such as the vibration problem written as `u' = v`, `v' = -omega^2 u`, the update `u^{n+1} = u^n + dt v^n` and its partner need no system solve ([[ref-langtangen-linge-2017-finite-difference-computing]] 1.5.1). It is first order (the forward difference has an `O(dt)` truncation error, [[ref-langtangen-linge-2017-finite-difference-computing]] B.2). In a finite-volume code all face values are taken at the old level, so "it is no longer necessary to solve the system of linear equations"; the drawback is the Courant number limit (Eqn. 3.48), and "If the Courant number is larger than unity, the explicit system becomes unstable" ([[ref-jasak-1996-phd-thesis]] 3.3.2). Its numerical diffusion is negative, which steepens profiles, and explicit central differencing is unconditionally unstable for pure convection because of it ([[ref-jasak-1996-phd-thesis]] 3.6, 3.7.3).

**Runge-Kutta.** Second- and fourth-order Runge-Kutta compared on the undamped vibration problem: RK4 is "very accurate, under all circumstances in these tests"; RK2 shows amplitude errors unless `dt` is small; Crank-Nicolson is better than RK2 although "Both schemes have the same order of accuracy O(dt^2)" ([[ref-langtangen-linge-2017-finite-difference-computing]] 1.5.5). The stage formulas themselves are not reproduced here (see Open questions).

**Multi-stage schemes for steady flow.** Compressible unstructured-grid solvers use explicit multi-stage Runge-Kutta schemes whose coefficients are optimised on a model problem "to yield a large CFL number and good damping properties", with local time stepping and residual averaging; even so convergence to steady state is "usually unacceptably slow", which motivates multigrid and implicit schemes ([[ref-venkatakrishnan-1995-icase-95-3]] 6.1). For unsteady problems low-storage Runge-Kutta and predictor-corrector schemes need only simple updates but are bound by the CFL condition ([[ref-venkatakrishnan-1995-icase-95-3]] 6.2).

**Explicit diffusion limit.** For the heat equation, forward in time and centred in space is stable only for `r = kappa dt/dx^2 <= 0.5` in 1D and `<= 1/4` in 2D ([[ref-mit-ocw-2-29-numerics]] L14 s.18, s.26). See [[stability-and-cfl]].

## Builds on

- Floor: ordinary differential equations `du/dt = f(u)`.
- [[truncation-error-and-order-of-accuracy]]: order of the schemes.

## Leads to

- [[stability-and-cfl]]
- rcfd pages: [[lagrangian-particles]]

## How rcfd uses it

- Lagrangian particle tracking offers the integrators `analytic` (exact for Stokes drag in a uniform field), `euler`, `rk2` and `rk4` ([[lagrangian-particles]]).

## Open questions

- The Runge-Kutta stage formulas (RK2, RK3, RK4) and the TVD/SSP Runge-Kutta family were not covered by a readable source in this ingest (an attempt on Gottlieb and Shu, ICASE 96-50, failed); the map's link to an RK3 in [[adr-thread-compressible-acoustic-boundaries]] was not confirmed on that page.
