---
type: source
tags: [reference, foundation, discretisation, limiter, cite-only]
sources: []
origin: https://ntrs.nasa.gov/citations/20050182793
licence: conflicting (2026-11-22): the NTRS record says "Public Use Permitted."; the paper itself says "Copyright (c) 2005 by the American Institute of Aeronautics and Astronautics, Inc. The U.S. Government has a royalty-free license ... for Governmental purposes. All other rights are reserved by the copyright owner."; treated as cite only
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Berger, Aftosmis and Murman (2005), Analysis of slope limiters on irregular grids

**Origin:** M. Berger, M. J. Aftosmis, S. M. Murman, "Analysis of Slope Limiters on Irregular Grids", AIAA Paper 2005-0490, 43rd AIAA Aerospace Sciences Meeting, Reno, January 2005. NTRS 20050182793 (PDF from `https://ntrs.nasa.gov/api/citations/20050182793/downloads/20050182793.pdf`). The NTRS record lists Berger and Aftosmis; the paper's title page also lists Murman.
**Kind:** conference paper (NTRS copy).
**Licence:** conflicting statements (frontmatter); cite only, not copied into `raw/`.
**Read:** 2026-11-22 by job p6-pv-discretisation: sections I to IV and the first results of section V. Locators are section and equation numbers.

## Summary

Studies flux and slope limiters on non-uniform and unstructured meshes. Rewrites common limiters in a symmetric form, shows that most of them limit even linear data on stretched meshes, derives the TVD bound on non-uniform 1D meshes, and casts multidimensional limiting of a cell gradient as a linear-programming problem. Tests on a Cartesian mesh with embedded boundaries (ONERA M6 wing) compare accuracy and convergence of the limiters.

## Key points (own summary)

- **Why limiters (I):** limiters "suppress oscillations and maintain monotonicity" but "reduce accuracy and can hamper convergence, particularly for multidimensional unstructured grids"; "Non-smooth limiter formulations lead to limiter chatter, even in near constant portions of the flow field, making it difficult to achieve steady state either with time marching or Newton's method"; over-compressive limiters can staircase oblique shocks.
- **Flux limiters (II.A, Eqs. 1-3):** Sweby's framework writes a second-order scheme as a first-order scheme plus a limited anti-diffusive flux; with the ratio $R$ of consecutive differences, choosing $\psi(R)$ properly gives a TVD scheme, with values bounded by 2 (Sweby's result; the shaded second-order TVD region of the Sweby diagram, Fig. 1; the inequality itself is not legible in the text layer); second order away from extrema needs $\psi(1) = 1$ ("the method should not do any limiting when the solution is a linear function"), and $\psi$ must be symmetric. Listed limiters in order of increasing dissipation for $R \ge 0$: superbee, Barth-Jespersen, van Leer, van Albada, min ($\psi_\text{min}(R) = \min(1, R)$); all set $\psi = 0$ for $R \le 0$ (an extremum). Superbee and Barth-Jespersen "are the most compressive, and are known to turn smooth waves into square waves".
- **Slope limiters (II.B, Eqs. 4-8):** flux limiting is equivalent to slope limiting (Spekreijse), the natural form for finite volumes: reconstruct $U_i(x) = u_i + \phi(R_i)\, \nabla u_i \cdot (x - x_i)$ from the cell average, evolve, project. The slope forms of Barth-Jespersen, van Leer, van Albada and min are given (Eq. 8).
- **Symmetric form (II.C, Eqs. 9-12):** plotting limiters against $f = (u_i - u_{i-1})/(u_{i+1} - u_{i-1})$ shows that the neighbour with the larger jump causes the limiting; a new smooth "sin" limiter falls between van Leer and van Albada.
- **TVD on non-uniform meshes (III.A, Eqs. 13-19):** the geometric condition (the reconstructed face value must not pass the neighbour's cell value; gradient zero at an extremum) still gives TVD, but the bounds are no longer twice the forward and backward differences; on uniform grids this condition is the Barth-Jespersen limiter, first proposed by van Leer and known elsewhere as monotonized central differences.
- **Least-squares gradient (III.B, Eqs. 20-22):** "a common procedure that extends to multiple dimensions is to approximate the gradient at cell i using a least squares fit to the solution in neighboring cells"; least squares gives "first-order slope estimates which are sufficient for second-order accuracy overall" and "even on distorted meshes, least squares gradients remain exact for linear data. Limiting, however, can destroy this property"; in 1D the least-squares slope is a convex combination of the forward and backward differences, becoming the central difference on a uniform mesh.
- **Linear data on stretched meshes (III.C, Eqs. 23-26):** if the limiter ignores stretching "all limiters except Barth-Jespersen will activate and fail to preserve the linear data"; at stretching ratio 1.2 the min limiter gives 0.90, van Leer 0.99, van Albada 0.98; at a 2:1 refinement jump min gives 6/7, van Albada 0.96, van Leer 0.98. Rewriting the ratio with scaled differences makes them linearity preserving but not entirely TVD on general irregular meshes.
- **Scalar versus directional limiting (IV.A):** a scalar limiter (one factor per cell) is "far more severe than their vector counterparts, since limiting triggered by any face of a polyhedral cell degrades the gradient in all directions"; directional (per-component) limiting is straightforward on cells with orthogonal faces but on general cells limiting one face can violate the bound of another.
- **Limiting as linear programming (IV.B-D):** each face gives a linear constraint on the gradient components and sign constraints keep the gradient direction; the best limited gradient is a corner of the feasible polygon; scalar limiting is the special case with the direction fixed.
- **Convergence on ONERA M6 (V, Table 1, Fig. 12):** at Mach 0.5, five-level multigrid; drag coefficient $\times 10^3$: no limiter 3.24, Barth-Jespersen 4.56, van Leer 5.41, sin 5.59, van Albada 6.15, min 8.15 (increasing with dissipation); all converge "with the exception of the calculation using the Barth-Jespersen limiter, which stalls after converging roughly three orders of magnitude".

## Contradictions with the wiki

None found.

## Pages this source informs

- [[gradient-reconstruction]], [[linear-upwind-scheme]], [[tvd-schemes-and-flux-limiters]], [[slope-limiters-unstructured]]
