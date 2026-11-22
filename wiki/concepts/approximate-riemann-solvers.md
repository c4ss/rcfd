---
type: concept
tags: [foundation, compressible, riemann-solver, flux-splitting]
sources: [ref-van-leer-1992-icase-92-43, ref-liou-2001-nasa-tm-210977]
builds_on: [riemann-problem-and-godunov-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Approximate Riemann solvers and flux splitting

## In short

Solving the exact Riemann problem at every face is expensive and rarely needed. Approximate Riemann solvers and flux-splitting schemes give the face flux from the two neighbouring states more cheaply. Most can be written as a central average of the two fluxes minus a dissipation term proportional to the jump; they differ in how that dissipation is built, which decides shock resolution, contact and boundary-layer accuracy, and robustness ([[ref-van-leer-1992-icase-92-43]] sec. 2.2; [[ref-liou-2001-nasa-tm-210977]]).

## Explanation

**Roe's solver.** Roe replaces the nonlinear problem by the exact solution of a locally linearised one, $U_t + \tilde A\, U_x = 0$ (eq. 16), where the mean matrix $\tilde A$ satisfies $\Delta F = \tilde A\,\Delta U$ exactly for any two states (eq. 6); for a calorically perfect gas it is obtained by averaging the parameter vector $w = \sqrt{\rho}\,(1, u, H)^T$ ([[ref-van-leer-1992-icase-92-43]] sec. 2.1-2.2). The face flux has three equivalent forms (eqs. 17-19); the symmetric one is

$$F(U_L, U_R) = \tfrac12 (F_L + F_R) - \tfrac12 |\tilde A|\,\Delta U, \qquad |\tilde A| = \tilde A^+ - \tilde A^-$$ (van Leer 1992, eqs. 19-20)

expanded over the waves as

$$F(U_L, U_R) = \tfrac12 (F_L + F_R) - \tfrac12 \sum_k |\lambda_k|\, \alpha_k R_k$$ (van Leer 1992, eq. 21)

Each wave is damped in proportion to its own speed $|\lambda_k|$: a matrix dissipation. In the finite-volume form $|\tilde A|$ "may be derived from any average" without losing conservation; this flux form, due to van Leer, preceded Roe's fluctuation approach "by a decade" ([[ref-van-leer-1992-icase-92-43]] sec. 2.2).

**Two families.** The 1980s produced "two classes of upwind schemes, namely the flux-vector and flux-difference schemes" ([[ref-liou-2001-nasa-tm-210977]] "How It Began"). Flux-difference splitting (Roe) splits the flux difference by wave; flux-vector splitting splits the flux itself into a part carried right and a part carried left. Van Leer's flux-vector splitting is "perhaps the most robust one among all for the Euler equations, aside from a slight disadvantage in the shock resolution", passes the positivity test and is free of shock instabilities; but flux-vector schemes are diffusive for Navier-Stokes calculations (wrong wall temperature, thicker boundary layer), which "spelt the downfall" of those schemes for viscous work ([[ref-liou-2001-nasa-tm-210977]] "How It Began", fig. 1).

**AUSM.** The Advection Upstream Splitting Method writes the inviscid flux as a convective part plus a pressure part ([[ref-liou-2001-nasa-tm-210977]] eqs. 3-4):

$$\mathbf{F} = \dot m\, \psi + \mathbf{P}, \qquad \dot m = \rho u_n, \quad \psi = (1, u, v, w, H)^T, \quad \mathbf{P} = (0, p n_x, p n_y, p n_z, 0)^T$$ (Liou 2001, eqs. 3-4)

At a face, one mass flux $\dot m_{1/2}$ carries $\psi_L$ if it is positive and $\psi_R$ if negative, and the pressure flux $p_{1/2}$ is split separately (eqs. 9-12). Both come from polynomial split functions of the left and right Mach numbers, $M_{1/2} = \mathcal{M}^+(M_L) + \mathcal{M}^-(M_R)$, with a common interface speed of sound $a_{1/2}$ (eqs. 13-23); a suitable $a_{1/2}$ captures a stationary normal shock exactly. Its dissipation is scalar (one factor for all equations), the same category as central schemes with artificial damping, van Leer's flux-vector scheme and HLLE; flux-difference splittings use matrix dissipation ([[ref-liou-2001-nasa-tm-210977]] "How it is shaping up").

**Low Mach number.** Compressible upwind schemes run at low speed show "(1) extremely slow or stalled convergence" and "(2) ... globally incorrect" solutions; the first comes from the wide spread of wave speeds, condition number $(|u| + a)/|u| \to \infty$ as $|u| \to 0$ (eq. 31), and is treated by preconditioning; the second comes from dissipation scaled by the speed of sound, treated by a scaled numerical speed of sound (eqs. 32-35) ([[ref-liou-2001-nasa-tm-210977]]).

**What it adds.** [[riemann-problem-and-godunov-method]] says the face flux should come from the Riemann problem; this page gives the cheaper fluxes actually used and the trade-offs between them.

## Builds on

- [[riemann-problem-and-godunov-method]]: the exact problem these fluxes approximate, and the finite-volume update they feed.

## Leads to

- [[doc-case-schema-compressible]] (flux choice of the density-based solver)

## How rcfd uses it

- The density-based Euler solver offers `flux` = `rusanov`, `roe`, `hllc`, `ausm_plus` ([[doc-case-schema-compressible]]).

## Open questions

- The sources read here describe Roe, flux-vector splitting and AUSM, and name HLLE only as a scalar-dissipation scheme; Rusanov and HLLC, two of rcfd's four fluxes, still need an open reference (Toro 2009 is cited by the repo but no open copy was found, [[foundations-sources]]).
