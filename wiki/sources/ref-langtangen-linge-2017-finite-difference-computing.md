---
type: source
tags: [reference, foundation, discretisation, time-stepping, copy]
sources: []
origin: https://link.springer.com/content/pdf/10.1007/978-3-319-55456-3.pdf
licence: CC BY 4.0 ("Open Access This book is distributed under the terms of the Creative Commons Attribution 4.0 International License", copyright page of the PDF, checked 2026-11-22); third-party images excepted as stated there
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Langtangen and Linge (2017), Finite Difference Computing with PDEs

**Origin:** H. P. Langtangen and S. Linge, *Finite Difference Computing with PDEs: A Modern Software Approach*, Texts in Computational Science and Engineering 16, Springer, 2017, doi:10.1007/978-3-319-55456-3. PDF from the Springer open-access link above.
**Kind:** open textbook.
**Licence:** CC BY 4.0 (the [[foundations-sources]] listing reported it; the PDF's copyright page confirms it). Full copy, with attribution, in [raw/langtangen-linge-2017-finite-difference-computing.pdf](../raw/langtangen-linge-2017-finite-difference-computing.pdf).
**Read:** 2026-11-22 by job p6-pv-discretisation: sections 1.5 (schemes for first-order ODE systems), 2.10.3 (stability of the wave scheme), 3.1.2, 3.2, 3.3.4-3.3.8 (diffusion schemes and their analysis), 4.1.4, 4.2, 4.3 (advection and advection-diffusion), Appendix B.1-B.2 (truncation error). Locators are section and equation numbers.

## Summary

A textbook on finite-difference methods built around worked model problems (vibration ODEs, wave, diffusion and advection equations, nonlinear problems) with Python code. Each scheme is analysed for stability (by inserting a discrete Fourier component and computing its amplification factor) and accuracy (by truncation error).

## Key points (own summary)

- **Forward and backward Euler, Crank-Nicolson for ODE systems (1.5.1-1.5.3, Eqs. 1.26-1.37):** forward Euler `u^{n+1} = u^n + dt v^n` is explicit; backward Euler and Crank-Nicolson give a coupled linear system per step. For oscillating solutions forward and backward Euler are "not suitable", while Crank-Nicolson is much better (1.5.4).
- **Runge-Kutta (1.5.5):** 2nd- and 4th-order Runge-Kutta methods compared on the vibration problem: RK4 "very accurate, under all circumstances in these tests", RK2 suffers amplitude errors unless `dt` is small; Crank-Nicolson outperforms RK2 although "Both schemes have the same order of accuracy O(dt^2)".
- **Courant number and wave stability (2.10.3, Eqs. 2.86-2.89):** for the centred wave scheme, `C = c dt / dx` is "the Courant number"; a real numerical frequency for the shortest mesh wave (length `2 dx`) needs `C <= 1`, otherwise round-off noise of that wavelength grows exponentially; "we must therefore require C <= 1 as a stability criterion".
- **Explicit diffusion (3.1.2, Eqs. 3.6-3.9):** forward in time, centred in space with the mesh Fourier number `F = alpha dt / dx^2`; "the shown algorithm does not produce meaningful results unless F <= 1/2".
- **Amplification factors (3.3.4-3.3.6, Eqs. 3.54-3.62):** forward Euler `A = 1 - 4F sin^2 p` with `p = k dx/2`, stable for `F <= 1/2`, i.e. `dt <= dx^2/(2 alpha)` (3.57-3.58), so halving `dx` needs a quarter of `dt`; backward Euler `A = (1 + 4F sin^2 p)^-1`, with `0 < A < 1`: "all numerical wave components are stable and non-oscillatory for any dt > 0"; Crank-Nicolson `A = (1 - 2F sin^2 p)/(1 + 2F sin^2 p)`, stable for any `dt` but oscillating in time if `F > 1/2`. Truncation errors: `O(dt) + O(dx^2)` for forward and backward Euler, `O(dx^2) + O(dt^2)` for Crank-Nicolson. Leapfrog is always unstable for diffusion (3.3.7).
- **Upwind for advection (4.1.4):** for `u_t + v u_x = 0` the upwind difference against the flow with forward Euler is "stable if C <= 1", exact at `C = 1`, and for `C < 1` damps the solution non-physically; it equals the forward-time centred-space scheme applied to the equation with an artificial diffusion `epsilon = v dx / 2` (interpretation box in 4.1.4).
- **Advection-diffusion (4.2):** the stationary model `v u' = epsilon u''` scaled to one parameter; centred differences give non-physical oscillations for small `epsilon` unless the mesh is fine enough (an oscillation-free criterion is developed in Exercise 4.1); the upwind difference removes the oscillations but gives a thicker boundary layer ("seemingly larger diffusion"); the time-dependent convection-diffusion equation follows in 4.3.
- **Truncation error (B.1-B.2, Eqs. B.1-B.3):** the residual `R = L_dt(u_e)` of the exact solution in the discrete equations is the truncation error; a Taylor expansion gives it as a power series in the mesh parameters; for the backward difference of `u'`, `R^n = -(1/2) u''(t_n) dt + O(dt^2)`, "first order in dt"; the forward difference is also first order; an error model `C h^r` fitted on refined meshes (for example with manufactured solutions) estimates the convergence rate `r` (B.1.2).

## Contradictions with the wiki

None found.

## Pages this source informs

- [[finite-difference-method]], [[truncation-error-and-order-of-accuracy]], [[stability-and-cfl]], [[implicit-time-integration]], [[explicit-time-integration-and-runge-kutta]], [[upwind-and-central-differencing]], [[convection-diffusion-equation]]
