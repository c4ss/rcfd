---
type: source
tags: [reference, foundation, discretisation, unstructured, copy]
sources: []
origin: https://ntrs.nasa.gov/citations/19950016514
raw: raw/venkatakrishnan-1995-icase-95-3.pdf
licence: "Work of the US Gov. Public Use Permitted." (NTRS record, checked 2026-11-22)
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Venkatakrishnan (1995), A perspective on unstructured grid flow solvers (ICASE 95-3)

**Origin:** V. Venkatakrishnan, "A perspective on unstructured grid flow solvers", ICASE Report No. 95-3, NASA CR-195025, February 1995, NTRS 19950016514.
**Kind:** survey report (NASA contractor report).
**Licence:** US Government work, public use permitted; full copy in [raw/venkatakrishnan-1995-icase-95-3.pdf](../raw/venkatakrishnan-1995-icase-95-3.pdf).
**Read:** 2026-11-22 by job p6-pv-discretisation: abstract, sections 1, 3 (finite-volume spatial discretisation, history of reconstruction and limiting), 6 (time discretisation and solution strategies) and 7-8 (adaptive grids, higher-order methods). Locators are section numbers.

## Summary

A survey of compressible Euler and Navier-Stokes solvers on unstructured grids (triangles and tetrahedra) as of 1995: grid types, cell-centred and cell-vertex finite-volume schemes, MUSCL reconstruction and multidimensional limiters, finite-element schemes, turbulence models, explicit, multigrid and implicit time integration, grid adaptation, higher-order reconstruction, parallel computing.

## Key points (own summary)

- **Grid types (1):** structured, body-fitted grids in generalised coordinates were the norm by the mid-1980s; complex configurations led to multi-block structured grids (logically rectangular blocks) and overlapping (chimera) grids; "unstructured grids" here means simplices; they give flexibility around complex geometry and for adapting to shocks and boundary layers, and their generation is assumed to be "far more automatable" than multi-block generation.
- **Reconstruction and limiters (3):** van Leer's MUSCL schemes rely on piecewise-polynomial reconstruction with nonlinear limiters and a Riemann solver at the faces. Barth and Jespersen stated a multidimensional monotonicity principle, "that the reconstructed distribution in the control volume be bounded everywhere by the values of the neighbors", and built "a truly multi-dimensional limiter". That limiter "may be thought of as a generalization of the min-mod limiter and as such, leads to convergence difficulties"; Venkatakrishnan "has analyzed this problem and has proposed modifications that ameliorate the situation at the expense of monotonicity", and Aftosmis et al. found that these modifications "significantly improve the convergence as well as solution accuracy". Barth's three-dimensional extensions outlined "least-squares and data-weighted procedures for the construction of gradients".
- **Explicit time stepping (6.1, 6.2):** after spatial discretisation, $dW/dt + R(W) = 0$ (Eq. 7); explicit multi-stage Runge-Kutta schemes have coefficients optimised on a model problem "to yield a large CFL number and good damping properties", with local time stepping and residual averaging; convergence to steady state is still "usually unacceptably slow", hence multigrid or implicit schemes. For unsteady problems low-storage Runge-Kutta and predictor-corrector schemes need only simple updates but are restricted by the CFL condition.
- **Implicit schemes (6.1.2, Eqs. 8-10):** evaluating $R$ at the new level and linearising gives a large nonsymmetric linear system per step; as $\Delta t$ grows it tends to Newton's method; direct solvers are impractical in 3D; iterative solvers (Gauss-Seidel, GMRES, QMR) with preconditioning (block-diagonal, SSOR, ILU) are used; GMRES with ILU was found superior to other implicit schemes over a range of problems. For unsteady flows the unsteady residual must also be driven down by inner iterations, or a backward-difference unsteady residual can be driven to zero with a relaxation or multigrid method, which "allows the time step to be determined solely based on flow physics".
- **Multigrid on unstructured grids (6.1.1):** nested refinement, non-nested independent coarse grids, or agglomeration of fine control volumes into polyhedral coarse ones.
- **Adaptive grids (7):** h-refinement is the most popular adaptation for compressible flows; regions are identified "through a combination of heuristic criteria such as density gradients (undivided) or through estimation of the truncation error"; with a Delaunay initial grid, Watson's incremental algorithm gives a new Delaunay triangulation, edge swapping can restore it, and other triangulations are subdivided and smoothed with "a few passes of a Laplacian-type smoother"; advancing-front regeneration after error estimation is an alternative; refining to flow features can over-resolve some regions "resulting in seemingly sharp but incorrect solutions", and Warren et al. proposed a modification of the indicators against this.
- **Viscous grids (3):** the Advancing-Layers Method "produces thin-layer tetrahedral grids suitable for computing viscous flows".
- **Higher order (8):** Barth and Fredrickson's conditions for higher-order reconstruction: conservation of the mean, k-exactness (exact reconstruction of polynomials of degree at most $k$) and compact support, with least squares for the coefficients.

## Contradictions with the wiki

None found.

## Pages this source informs

- [[mesh-generation-basics]], [[gradient-reconstruction]], [[linear-upwind-scheme]], [[tvd-schemes-and-flux-limiters]], [[slope-limiters-unstructured]], [[a-posteriori-error-indicators]], [[explicit-time-integration-and-runge-kutta]], [[implicit-time-integration]], [[block-coupled-solver]], [[finite-volume-method]]
