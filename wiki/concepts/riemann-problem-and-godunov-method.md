---
type: concept
tags: [foundation, compressible, finite-volume, riemann-solver]
sources: [ref-van-leer-1992-icase-92-43, ref-liou-2001-nasa-tm-210977]
builds_on: [characteristics-and-hyperbolic-systems, shock-waves, finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Riemann problem and Godunov's method

## In short

The Riemann problem is the initial-value problem of a hyperbolic system that starts from two constant states separated by a jump. Godunov's finite-volume scheme treats every cell face as such a jump: it takes the two neighbouring cell averages, solves (or approximates) the Riemann problem between them, and uses the resulting flux through the face to update the cells ([[ref-van-leer-1992-icase-92-43]] sec. 2.2).

## Explanation

**Finite-volume update.** Cell averages $U_i$ change only by the difference of numerical fluxes at the two faces ([[ref-van-leer-1992-icase-92-43]] sec. 2.2):

$$U_i^{n+1} = U_i^n - \frac{\Delta t}{\Delta x}\left(F_{i+1/2} - F_{i-1/2}\right)$$ (van Leer 1992, eq. 13)

Everything then depends on the numerical flux function $F(U_L, U_R)$, "a recipe for computing the interface fluxes from the states $U_L$ and $U_R$" on the two sides of the face.

**The Riemann problem.** In Godunov's first-order scheme "the interface flux is taken from the solution at $t > 0$ of Riemann's initial-value problem" with ([[ref-van-leer-1992-icase-92-43]] sec. 2.2):

$$u(x, 0) = U_L \ (x < 0), \qquad u(x, 0) = U_R \ (x > 0)$$ (van Leer 1992, eqs. 14-15)

For the [[euler-equations]] the solution is made of waves leaving the origin, one for each characteristic family of [[characteristics-and-hyperbolic-systems]] (the wave sum of [[ref-van-leer-1992-icase-92-43]] eqs. 10-12); the discontinuities a scheme must capture are "shock and contact discontinuities" ([[ref-liou-2001-nasa-tm-210977]] introduction; see [[shock-waves]]). The state that sits on the face ($x = 0$) for $t > 0$ gives the flux. Because each face is solved separately, the update is upwind: information enters each cell from the side the waves come from.

**Two readings of upwinding.** The same first-order upwind update can be read as advancing nodal values (which led to Roe's fluctuation splitting) or cell averages (which led to Godunov's "projection/evolution" or "reconstruction/evolution" finite-volume schemes, extended by van Leer) ([[ref-van-leer-1992-icase-92-43]] sec. 2, fig. 1). In the reconstruction/evolution view the cell averages are first reconstructed into a profile in each cell (constant for first order; van Leer's examples also use a second-order "MUSCL-type" scheme, sec. 4; see [[tvd-schemes-and-flux-limiters]]), then evolved by the face Riemann problems.

**Exact or approximate.** "For many applications it is not necessary to use the exact solution to this problem, hence the activity in the research area of 'approximate Riemann solvers'" ([[ref-van-leer-1992-icase-92-43]] sec. 2.2). See [[approximate-riemann-solvers]]. In Liou's survey the exact Godunov solver serves as the reference against which flux-vector splitting, van Leer-Hanel and AUSM fluxes are compared on a hypersonic conical flow ([[ref-liou-2001-nasa-tm-210977]] fig. 1).

**Accuracy and limits.** In one dimension upwind schemes of this kind are "computationally intensive but unsurpassed in their combination of accuracy and robustness" ([[ref-van-leer-1992-icase-92-43]] sec. 1). In more dimensions the usual practice solves 1D Riemann problems normal to each face, which assumes waves travel normal to the faces and "leads to smearing of shock and shear waves when these are not grid-aligned" (abstract).

**What it adds.** The [[finite-volume-method]] gives the update "change = minus net face flux"; [[characteristics-and-hyperbolic-systems]] and [[shock-waves]] give the waves and jumps of the Euler equations. Godunov's method joins them: the face flux is computed from the physical wave pattern between the two cells.

## Builds on

- [[characteristics-and-hyperbolic-systems]]: the wave families and their directions.
- [[shock-waves]]: the jump conditions of the waves in the Riemann fan.
- [[finite-volume-method]]: cell averages updated by face fluxes.

## Leads to

- [[approximate-riemann-solvers]]
- [[doc-case-schema-compressible]], [[adr-thread-acoustic-wave-solver]]

## How rcfd uses it

- The compressible case can be initialised as a Riemann problem (`compressible.initial = "riemann"`, split along `split_axis` at `split_position`) ([[doc-case-schema-compressible]]).
- The linear acoustic wave solver uses "an exact linear Riemann flux" on the polyhedral finite-volume mesh ([[adr-thread-acoustic-wave-solver]]).
