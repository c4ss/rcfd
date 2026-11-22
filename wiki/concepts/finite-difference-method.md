---
type: concept
tags: [foundation, discretisation]
sources: [ref-langtangen-linge-2017-finite-difference-computing, ref-mit-ocw-2-29-numerics, ref-van-leer-1992-icase-92-43]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Finite-difference method

## In short

Put the unknown function on a grid of points in space and time and replace each derivative by a difference quotient of neighbouring point values, such as `(u_{i+1} - u_i)/dx`. The differential equation becomes a set of algebraic equations for the point values. It is the simplest discretisation and the usual model for analysing accuracy and stability ([[ref-langtangen-linge-2017-finite-difference-computing]]).

## Explanation

**Mesh and mesh function.** Take equally spaced points `x_i = i dx` and times `t_n = n dt`; `u_i^n` approximates `u(x_i, t_n)`. Requiring the equation to hold at each mesh point and replacing derivatives by differences gives the discrete equations ([[ref-langtangen-linge-2017-finite-difference-computing]] 3.1.2).

**Example: the diffusion equation.** For `u_t = alpha u_xx`, a forward difference in time and a centred difference in space give `(u_i^{n+1} - u_i^n)/dt = alpha (u_{i+1}^n - 2 u_i^n + u_{i-1}^n)/dx^2`, which can be solved for `u_i^{n+1}` point by point: `u_i^{n+1} = u_i^n + F (u_{i+1}^n - 2u_i^n + u_{i-1}^n)` with the mesh Fourier number `F = alpha dt/dx^2` ([[ref-langtangen-linge-2017-finite-difference-computing]] Eqns. 3.6-3.9). The same scheme appears in the OCW notes as the forward-in-time, centred-in-space heat conduction scheme with `r = kappa dt/dx^2` ([[ref-mit-ocw-2-29-numerics]] L14 s.17-18).

**Example: convection.** For `u_t + c u_x = 0` with `c > 0` the first-order upwind scheme uses the backward space difference `(u_i^n - u_{i-1}^n)/dx` ([[ref-van-leer-1992-icase-92-43]] Eq. 2); see [[upwind-and-central-differencing]].

**Elliptic problems.** The 2D Laplace equation with the five-point stencil `u_{i+1,j} + u_{i-1,j} + u_{i,j+1} + u_{i,j-1} - 4u_{i,j} = 0` gives a sparse linear system, solved directly or iteratively (Jacobi, Gauss-Seidel, SOR) ([[ref-mit-ocw-2-29-numerics]] L14 s.5-7).

**Accuracy.** Each difference quotient has a truncation error found by Taylor expansion; forward and backward differences of `u'` are first order, the centred difference second order ([[ref-langtangen-linge-2017-finite-difference-computing]] B.2). See [[truncation-error-and-order-of-accuracy]].

**Relation to finite volumes.** Finite differences discretise the differential form; finite volumes the integral form over control volumes ([[ref-mit-ocw-2-29-numerics]] L15 s.6). On uniform grids the two often give the same stencils: the 1D finite-volume scheme for linear convection with averaged face fluxes is "as the 2nd order CDS", but in cell averages ([[ref-mit-ocw-2-29-numerics]] L16 s.4).

## Builds on

- Floor: derivatives, Taylor series, solving `A x = b`.

## Leads to

- [[truncation-error-and-order-of-accuracy]], [[stability-and-cfl]], [[upwind-and-central-differencing]], [[implicit-time-integration]], [[explicit-time-integration-and-runge-kutta]]

## How rcfd uses it

- rcfd's flow solvers are finite-volume codes ([[simple]], [[piso]]); finite differences enter rcfd through the time derivative (rcfd's implicit Euler and opt-in BDF2 steps, see [[implicit-time-integration]], where backward differencing in time is a difference quotient over two or three time levels, [[ref-jasak-1996-phd-thesis]] 3.3.2) and as the model problems behind stability limits such as the diffusion number of [[piso]].
