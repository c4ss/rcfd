---
type: concept
tags: [foundation, mesh, overset, chimera]
sources: [ref-meakin-1996-nasa-cr-200828, ref-yao-2026-ale-chalmers]
builds_on: [finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Overset (chimera) interpolation

## In short

Instead of one mesh that fits everything, overlapping meshes are used: a body-fitted grid around each body laid over a background grid. Where they overlap, each grid takes the values it is missing at its edges by interpolating from the other; cells of the background that lie inside a body are switched off ("holes"). Bodies can then move simply by moving their own grid.

## Explanation

- **Decomposition.** The geometry is split into "geometrically simple overlapping component grids"; grids attached to moving bodies "move with the bodies without stretching or distorting the grid system" ([[ref-meakin-1996-nasa-cr-200828]], sec. 1).
- **Domain connectivity.** Connectivity "is achieved through interpolation of necessary intergrid boundary information from solutions in the overlap region of neighboring grid systems". In the airfoil example (fig. 1) the near-body grid's outer boundary takes values interpolated from the background; a hole boundary is cut in the background where the airfoil overlaps it, and the hole-boundary (fringe) values are interpolated from the near-body grid; points inside the hole are excluded ([[ref-meakin-1996-nasa-cr-200828]], sec. 2.1).
- **The ingredients.** Hole cutting, donor search (finding the cell of the other grid that contains each receiving point, with quality optimisation) and interpolation weights; for moving bodies connectivity is recomputed every time step ([[ref-meakin-1996-nasa-cr-200828]], sec. 2.2.4).
- **Solver changes are small.** An array `ib` is 0 at hole points and 1 at field points; hole points are not updated and receive interpolated values, so existing single-grid solvers carry over without special branching ([[ref-meakin-1996-nasa-cr-200828]], sec. 2.3).
- **Accuracy.** With component boundaries fixed during refinement, a second-order solver stays second order with a second-order interpolation such as tri-linear ([[ref-meakin-1996-nasa-cr-200828]], sec. 2.3.1).
- **Conservation.** Simple interpolation "implies that conservation is not strictly enforced", but with a conservative solver conservation holds except at a few intergrid boundary points; the primary issue is resolution of the flow in both donor and recipient grids. For incompressible flow, non-conservative interpolation can be made conservative "by local redistribution of fluxes such that global conservation is ensured" ([[ref-meakin-1996-nasa-cr-200828]], sec. 2.3.1). Interpolation across overlaps needs its own accuracy and conservation checks ([[ref-yao-2026-ale-chalmers]], sec. 8.2).

## Builds on

- [[finite-volume-method]]: the solver on each component grid, with fringe cells as an extra kind of boundary.
- Floor: linear and multilinear interpolation.

## Leads to

- [[overset-mesh]]

## How rcfd uses it

- **Connectivity:** each step `Overset::connectivity` sorts cells into calculated, fringe and hole (user hole function); fringe rows of momentum and pressure are the interpolations, one system for both meshes; weights reproduce linear fields ([[overset-mesh]]).
- **Donor search** on warped and non-convex cells: every one of 100000 random points in exactly one cell (F-352, F-356) ([[overset-mesh]]).
- **Mass:** `MassMode::Conservative` offsets the background fringe pressures so the net flux into the hole region is zero; `MassMode::Interpolated` loses `2.7e-3` of `U D L` on the cylinder case ([[overset-mesh]]).
- **Order:** the interpolated-cell order 1.62 (registered 1.7) is the truncation of the linear donor interpolation at one fringe cell; `DonorInterpolation::Quadratic` restores about 2 (F-336) ([[overset-mesh]], [[adr-thread-overset]]).
