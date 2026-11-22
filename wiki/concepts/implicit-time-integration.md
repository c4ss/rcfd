---
type: concept
tags: [foundation, time-stepping]
sources: [ref-jasak-1996-phd-thesis, ref-langtangen-linge-2017-finite-difference-computing, ref-mit-ocw-2-29-numerics]
builds_on: [truncation-error-and-order-of-accuracy]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Implicit time integration (implicit Euler, Crank-Nicolson, BDF2)

## In short

An implicit scheme evaluates the spatial terms (at least partly) at the new time level, so each step requires solving a linear system that couples all cells. In return the time step is no longer limited by the Courant or diffusion number for stability. The three standard choices: implicit (backward) Euler, first order and bounded; Crank-Nicolson, second order but not bounded; and backward differencing over three time levels (BDF2), second order and not bounded ([[ref-jasak-1996-phd-thesis]] 3.3.2).

## Explanation

**Implicit Euler.** Evaluate all face values at the new time level: `(phi_P^n - phi_P^{n-1}) V_P / dt + (convection + diffusion at n) = sources`. It is first order in time, creates a coupled system `a_P phi_P + sum_N a_N phi_N = R`, stays stable when the Courant limit is violated, and "guarantees boundedness" ([[ref-jasak-1996-phd-thesis]] 3.3.2, Eqns. 3.49-3.50). Its truncation error introduces positive numerical diffusion that smears transient profiles ([[ref-jasak-1996-phd-thesis]] 3.6, 3.7.3). For the diffusion equation its amplification factor `A = 1/(1 + 4F sin^2 p)` lies between 0 and 1, so "all numerical wave components are stable and non-oscillatory for any dt > 0" ([[ref-langtangen-linge-2017-finite-difference-computing]] 3.3.5).

**Crank-Nicolson.** Average the spatial terms of the old and new levels. Second order in time and "unconditionally stable", but it "does not guarantee boundedness of the solution" ([[ref-jasak-1996-phd-thesis]] 3.3.2, Eqn. 3.41). For diffusion, `A = (1 - 2F sin^2 p)/(1 + 2F sin^2 p)`: stable for any `dt` but oscillating in time if the mesh Fourier number `F > 1/2`; truncation error `O(dt^2) + O(dx^2)` ([[ref-langtangen-linge-2017-finite-difference-computing]] 3.3.6). The OCW notes give the same orders for the heat equation ([[ref-mit-ocw-2-29-numerics]] L14 s.21-22).

**Backward differencing (BDF2).** Uses three time levels: Taylor expansions of `phi` at `t^{n-1}` and `t^{n-2}` about `t^n` are combined so that the first-order error term cancels, giving a second-order approximation of the time derivative (Eqns. 3.51-3.54; the OCR of the formula is not legible). Own restatement for a constant step: `dphi/dt ~ (3 phi^n - 4 phi^{n-1} + phi^{n-2}) / (2 dt)`, which follows from those two expansions. It is second order, its leading truncation term is "four times larger than for the Crank-Nicholson method", and its extrapolation in time makes it unbounded even with a bounded convection scheme ([[ref-jasak-1996-phd-thesis]] 3.3.2, 3.6, 3.7.3).

**Cost.** Every implicit step needs a linear solve; ADI splits a 2D step into two half steps with tridiagonal systems ([[ref-mit-ocw-2-29-numerics]] L14 s.27-28). In pressure-velocity coupled solvers the implicit step also couples pressure and velocity ([[pressure-velocity-coupling]]).

## Builds on

- Floor: ordinary differential equations `du/dt = f(u)`.
- [[truncation-error-and-order-of-accuracy]]: what "first" and "second order in time" mean.

## Leads to

- [[stability-and-cfl]], [[projection-method]]
- rcfd pages: [[piso]], [[dynamic-mesh]], [[k-epsilon]], [[overset-mesh]]

## How rcfd uses it

- The co-simulation fluid side is the incompressible PISO/PIMPLE step with implicit Euler on a moving mesh ([[piso]]).
- **BDF2 opt-in:** moving meshes use Euler only in the case tree; BDF2 is a library opt-in (F-345, ADR 0379; uniform flow to `7e-15`) ([[dynamic-mesh]]).
- **BDF2 order misses:** with the step-consistent Rhie-Chow face flux BDF2 meets its order at rest but not on the moving overset TG_2 (`2.454` against 4) ([[adr-thread-bdf2-step-consistent-face-flux]]); on overset meshes the time error does not fall with the square of the step (ratio 2.46 against 5; 2.30 on a single mesh) ([[overset-mesh]]); k-epsilon's BDF2 time order is a known miss whose cause is not found ([[k-epsilon]]).

## Open questions

- Variable-step BDF2 (coefficients for unequal steps) is not covered by the sources read for this page.
