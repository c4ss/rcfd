---
type: concept
tags: [foundation, discretisation, convection-scheme]
sources: [ref-jasak-1996-phd-thesis, ref-mit-ocw-2-29-numerics, ref-langtangen-linge-2017-finite-difference-computing, ref-van-leer-1992-icase-92-43]
builds_on: [finite-volume-method, convection-diffusion-equation]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Upwind and central differencing (convection schemes)

## In short

A finite-volume code knows $\phi$ at cell centres but needs it at faces to compute the convective flux. Central differencing interpolates linearly between the two cells: second-order accurate, but it produces wiggles when convection dominates. Upwind differencing takes the value from the cell the flow comes from: never wiggles, but first order and smeared, as if extra diffusion had been added ([[ref-jasak-1996-phd-thesis]]; [[ref-mit-ocw-2-29-numerics]]).

## Explanation

**Central differencing (CD).** With $f_x = \lvert fN \rvert / \lvert PN \rvert$ the fraction of the P-N distance on the N side, $\phi_f = f_x \phi_P + (1 - f_x)\, \phi_N$ ([[ref-jasak-1996-phd-thesis]] Eqns. 3.19-3.20). It is second order even on non-uniform meshes but causes "unphysical oscillations in the solution for convection-dominated problems" and so violates boundedness ([[ref-jasak-1996-phd-thesis]] 3.3.1.2). The OCW notes write the same linear interpolation as $\phi_e = \phi_E \lambda_e + \phi_P (1 - \lambda_e)$ and note it "can provide oscillatory solutions" ([[ref-mit-ocw-2-29-numerics]] L16 s.15-16).

**Upwind differencing (UD).** $\phi_f = \phi_P$ if the face flux $F \ge 0$ (flow leaves P) and $\phi_f = \phi_N$ if $F < 0$ ([[ref-jasak-1996-phd-thesis]] Eqn. 3.21). For $u_t + c u_x = 0$ the finite-difference form is the backward difference $(u_i - u_{i-1})/\Delta x$ when $c > 0$, forward when $c < 0$, first-order accurate ([[ref-van-leer-1992-icase-92-43]] 2, Eqns. 1-3). It never gives oscillatory solutions, but its leading truncation error is diffusive ([[ref-mit-ocw-2-29-numerics]] L16 s.13-14).

**Numerical diffusion.** With forward Euler, the upwind scheme for $u_t + v u_x = 0$ is the centred scheme applied to the equation with an added diffusion $\epsilon = v\, \Delta x / 2$; it is stable for $C = v\, \Delta t/\Delta x \le 1$, exact at $C = 1$, and damps the solution for $C < 1$ ([[ref-langtangen-linge-2017-finite-difference-computing]] 4.1.4). UD guarantees boundedness at the cost of accuracy; the numerical diffusion it adds is one of the discretisation errors Jasak lists for convection schemes ([[ref-jasak-1996-phd-thesis]] 3.3.1.2, 3.6).

**When CD wiggles.** For the 1D stationary advection-diffusion model $u' = \epsilon u''$, centred differences are oscillation-free only if $\Delta x/\epsilon \le 2$ (a cell Peclet number of at most two); upwind removes the oscillations everywhere but thickens the boundary layer ([[ref-langtangen-linge-2017-finite-difference-computing]] 4.2.2-4.2.3).

**Blended and higher-order schemes.** Blended differencing mixes UD and CD with a factor $\gamma$ ($\gamma = 0$ is UD), trading boundedness for accuracy ([[ref-jasak-1996-phd-thesis]] Eqns. 3.22-3.23). QUICK fits a parabola through two upstream nodes and one downstream node and is third order at the face (coefficients 6/8, 3/8, -1/8 on a uniform grid); the OCW notes also list linear upwind, skewed upwind and hybrid schemes, and blending to suppress oscillations at higher order ([[ref-mit-ocw-2-29-numerics]] L16 s.17-18). Bounded high-order schemes are the subject of [[tvd-schemes-and-flux-limiters]]; the gradient-based second-order upwind scheme is [[linear-upwind-scheme]].

**Matrix structure.** Only UD gives a convection matrix that is diagonally equal (the coefficients satisfy the diagonal-dominance condition of iterative solvers) under all conditions, which is why higher-order schemes are often written as UD plus a [[deferred-correction]] ([[ref-jasak-1996-phd-thesis]] 3.5).

## Builds on

- [[finite-volume-method]]: the face value $\phi_f$ in the convective flux $F \phi_f$ is what the scheme supplies.
- [[convection-diffusion-equation]]: the model problem on which the schemes are compared.

## Leads to

- [[linear-upwind-scheme]], [[tvd-schemes-and-flux-limiters]], [[deferred-correction]]
- rcfd pages: [[mrf]], [[k-omega-sst]], [[fan-01]]

## How rcfd uses it

- rcfd's convection schemes cover upwind, central, linear-upwind, QUICK/MUSCL, TVD limiters and deferred correction (F-022, done) ([[roadmap-numerics-and-solvers]]).
- **First-order upwind on FAN-01:** the steady relative-frame solve with first-order upwind "manufactures swirl that the walls do not supply" (ADRs 0078, 0082), so the rotor is underloaded ([[mrf]], [[fan-01]]). The Studio example of one FAN-01 passage uses first-order `upwind` and gives a torque 35 % below the measured value, "not a validated fan prediction" ([[fan-01]]).
- For the SST model, `linearUpwind` for $k$/$\omega$ "diverged in tests" ([[k-omega-sst]]); the F-126 attempt 3 on FAN-01 ran SST with `upwind` ([[k-omega-sst]]).
