---
type: concept
tags: [foundation, time-stepping, stability]
sources: [ref-mit-ocw-2-29-numerics, ref-langtangen-linge-2017-finite-difference-computing, ref-jasak-1996-phd-thesis]
builds_on: [implicit-time-integration, explicit-time-integration-and-runge-kutta, finite-difference-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Stability, the Courant number and the diffusion number

## In short

A time-stepping scheme is stable if small errors (round-off, initial errors) do not grow without bound from step to step. For explicit schemes stability limits the time step: information must not travel more than about one cell per step (Courant number $C = u\, \Delta t/\Delta x \le 1$), and for diffusion the mesh Fourier (diffusion) number $\alpha\, \Delta t/\Delta x^2$ must stay below about 1/2. Implicit schemes remove these limits for stability, but not for accuracy ([[ref-mit-ocw-2-29-numerics]] L12, L14; [[ref-langtangen-linge-2017-finite-difference-computing]]).

## Explanation

**Ways to check stability.** Introduce an isolated error and watch it (heuristic); show that a norm of the solution stays bounded (energy method, also for nonlinear problems and non-periodic boundaries); or von Neumann analysis: insert one Fourier mode and require its amplification factor to have magnitude at most one, which assumes a linear problem with periodic boundaries ([[ref-mit-ocw-2-29-numerics]] L12 s.16-19).

**Amplification factor, worked.** For the diffusion equation with forward Euler and centred space, inserting $u_q^n = A^n e^{i k q \Delta x}$ gives $A = 1 - 4F \sin^2 p$ with $p = k\, \Delta x/2$ and $F = \alpha\, \Delta t/\Delta x^2$; $\lvert A \rvert \le 1$ for all $p$ needs $F \le 1/2$, i.e. $\Delta t \le \Delta x^2/(2 \alpha)$, so halving $\Delta x$ needs a quarter of the time step ([[ref-langtangen-linge-2017-finite-difference-computing]] 3.3.4, Eqns. 3.54-3.58). Backward Euler has $0 < A < 1$ for every $\Delta t$; Crank-Nicolson is stable for every $\Delta t$ but oscillates in time for $F > 1/2$ ([[ref-langtangen-linge-2017-finite-difference-computing]] 3.3.5-3.3.6). In 2D the explicit limit is $r \le 1/4$ ([[ref-mit-ocw-2-29-numerics]] L14 s.26).

**CFL condition.** "Numerical domain of dependence must include Mathematical domain of dependence" ([[ref-mit-ocw-2-29-numerics]] L14 s.1). For the wave equation with the centred scheme the shortest mesh wave grows exponentially unless $C = c\, \Delta t/\Delta x \le 1$, "the Courant number" ([[ref-langtangen-linge-2017-finite-difference-computing]] 2.10.3, Eqns. 2.86-2.89). First-order upwind with forward Euler for advection is stable for $C \le 1$ ([[ref-langtangen-linge-2017-finite-difference-computing]] 4.1.4). In finite-volume form the Courant number is built from the face velocity, the time step and the cell-centre distance, and the explicit system is unstable above one ([[ref-jasak-1996-phd-thesis]] 3.3.2, Eqns. 3.47-3.48).

**Numerical diffusion and stability.** Explicit time stepping adds negative numerical diffusion; explicit central differencing of pure convection is unconditionally unstable because "Any system with negative diffusion is unstable", while explicit upwind adds enough positive diffusion to be stable within the Courant limit ([[ref-jasak-1996-phd-thesis]] 3.6).

**Stiffness.** When a system has both fast and slow components, an explicit scheme is limited by the fastest one even if it is irrelevant to the answer; the diffusion limit $\Delta t \sim \Delta x^2$ on fine cells is the typical case in CFD (own summary of the scaling above). Implicit schemes are used for this reason ([[implicit-time-integration]]).

## Builds on

- [[implicit-time-integration]], [[explicit-time-integration-and-runge-kutta]]: the schemes being analysed.
- [[finite-difference-method]]: the model discretisations of the analysis.

## Leads to

- rcfd pages: [[piso]], [[divergence-guard]], [[adr-thread-setup-warnings-and-time-step]]

## How rcfd uses it

- **Diffusion-number limit of PISO:** the default PISO step "grows unstable above a diffusion number $\nu\, \Delta t/\Delta x^2$ of about 3 to 4 across the finest cells", a sign-alternating mode, also in a plane channel; remedy consistent correctors, plus outer iterations at large diffusion numbers ([[piso]]). On the cylinder O-grid the onset is between 6.5 and 11.4 in $\nu\, \Delta t / \Delta r_1^2$ (job 48T) ([[piso]]).
- **Courant limit as a guard:** the transient divergence guard stops a run at a Courant number over `solver.divergence_courant_limit` (0 means a built-in 1e6) ([[divergence-guard]]).
- **Courant target:** the Studio sets an automatic step under the solver's Courant controller with target 0.5 and warns above Courant 2; "The Courant target 0.5 is not a claim of stability on every mesh": on the graded 256 x 256 O-grid PISO with two correctors is unstable well below Courant 1 (ADR 0153) ([[adr-thread-setup-warnings-and-time-step]]).
- These rcfd limits concern the implicit PISO pressure-velocity iteration, not the explicit-scheme limits above; the sources read here do not analyse them.
