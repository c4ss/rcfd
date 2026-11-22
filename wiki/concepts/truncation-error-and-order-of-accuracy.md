---
type: concept
tags: [foundation, discretisation, verification]
sources: [ref-langtangen-linge-2017-finite-difference-computing, ref-mit-ocw-2-29-numerics, ref-jasak-1996-phd-thesis]
builds_on: []
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Truncation error and order of accuracy

## In short

Put the exact solution into the discrete equations: it does not satisfy them exactly, and what is left over is the truncation error. A Taylor expansion shows how it shrinks with the mesh size $h$ (or time step): if the leading term goes like $h^p$, the scheme is of order $p$, and halving $h$ should cut the error by about $2^p$ ([[ref-langtangen-linge-2017-finite-difference-computing]]).

## Explanation

**Definition.** Write the differential problem as $L(u_e) = 0$ and the discrete one as $L_{\Delta t}(u) = 0$. The residual $R = L_{\Delta t}(u_e)$ of the exact solution in the discrete equations is the truncation error of the scheme; it is found as a power series in the discretisation parameters by Taylor expansion, and its leading terms measure the accuracy as the parameters go to zero ([[ref-langtangen-linge-2017-finite-difference-computing]] B.1.1-B.1.2).

**Worked example.** For the backward difference $[D_t^- u]^n = (u^n - u^{n-1})/\Delta t$ of $u'(t_n)$, expanding $u(t_{n-1}) = u(t_n) - u'(t_n)\, \Delta t + \tfrac{1}{2} u''(t_n)\, \Delta t^2 + O(\Delta t^3)$ gives $R^n = -\tfrac{1}{2} u''(t_n)\, \Delta t + O(\Delta t^2)$: the error is proportional to $\Delta t$, so the formula is "first order in $\Delta t$" ([[ref-langtangen-linge-2017-finite-difference-computing]] Eqns. B.1-B.3). The forward difference is also first order; the centred difference is second order (B.2.2-B.2.3).

**Space and time together.** For the forward Euler scheme of the diffusion equation the truncation error is $O(\Delta t) + O(\Delta x^2)$, which suggests an error $E = C_t\, \Delta t + C_x\, \Delta x^2$ with two unknown constants; for Crank-Nicolson it is $O(\Delta x^2) + O(\Delta t^2)$ ([[ref-langtangen-linge-2017-finite-difference-computing]] 3.3.4, 3.3.6).

**Finite volumes.** Assuming a linear variation of $\phi$ in space and time about each cell centre makes the leading truncation term scale with the square of the cell size and of $\Delta t$, so the method is second order; any step that lowers this order (a first-order convection or time scheme, a discarded non-orthogonal correction) creates an error of the size of the other terms that vanishes only on very fine meshes ([[ref-jasak-1996-phd-thesis]] 3.3, Eqns. 3.3-3.7).

**Observed order.** For a convergent scheme the error behaves like $O(\Delta x^p)$. With solutions on three grids refined by a factor 2, $p$ is estimated as $p = \log\left(\dfrac{u_{2\Delta x} - u_{4\Delta x}}{u_{\Delta x} - u_{2\Delta x}}\right) / \log 2$, and the error on the finest grid as $(u_{\Delta x} - u_{2\Delta x})/(2^p - 1)$; checking that the observed $p$ matches the design order is a code check ([[ref-mit-ocw-2-29-numerics]] L12 s.5). Equivalently, an error model $C h^r$ is fitted to solutions on refined meshes, for example with a manufactured solution, to estimate the rate $r$ ([[ref-langtangen-linge-2017-finite-difference-computing]] B.1.2). The same grid sequence allows Richardson extrapolation ([[ref-mit-ocw-2-29-numerics]] L12 s.5-8).

## Builds on

- Floor: Taylor series.

## Leads to

- [[stability-and-cfl]], [[implicit-time-integration]], [[explicit-time-integration-and-runge-kutta]], [[a-posteriori-error-indicators]]
- rcfd pages: [[gci-richardson]]

## How rcfd uses it

- [[gci-richardson]]: rcfd estimates mesh-converged values from systematically refined meshes and their observed order; for the cylinder the extrapolation only counts if the observed order is in [1.5, 2.5] (ADR 0763) ([[gci-richardson]]).
- Order checks: the overset Taylor-Green interpolated-cell order 1.62 against a registered 1.7, background order 1.98 ([[overset-mesh]]); the immersed-solid forcing converges at `1.023` against a registered `1.5`, a known miss ([[immersed-solid]]); the k-epsilon BDF2 time order is a known miss ([[k-epsilon]]).
