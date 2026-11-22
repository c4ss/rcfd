---
type: concept
tags: [foundation, discretisation, gradient]
sources: [ref-jasak-1996-phd-thesis, ref-berger-aftosmis-murman-2005-slope-limiters, ref-venkatakrishnan-1995-icase-95-3]
builds_on: [finite-volume-method, linear-regression-least-squares]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Gradient reconstruction (Green-Gauss and least squares)

## In short

A cell-centred finite-volume code stores one value per cell, but many terms need the gradient of the field in that cell: second-order upwind face values, non-orthogonal corrections, limiters, turbulence production. Two standard ways to get it: apply the divergence theorem to the face values (Green-Gauss), or fit a linear function through the neighbouring cell values (least squares) ([[ref-jasak-1996-phd-thesis]]; [[ref-berger-aftosmis-murman-2005-slope-limiters]]).

## Explanation

**Green-Gauss.** Applying the discrete Gauss theorem to the gradient gives $(\nabla \phi)_P = \dfrac{1}{V_P} \sum_f S_f \phi_f$: the sum over the cell's faces of the face area vector times the face value, divided by the cell volume, with $\phi_f$ from linear interpolation of the cell values ([[ref-jasak-1996-phd-thesis]] Eqn. 3.26). Face gradients are then interpolated from the two cell gradients (Eqn. 3.27).

**Least squares.** Choose the cell gradient $g$ that best fits the differences to the neighbours: minimise the sum over neighbours $j$ of $(\phi_j - \phi_i - g \cdot (x_j - x_i))^2$, possibly with weights. It is "a common procedure that extends to multiple dimensions" and "even on distorted meshes, least squares gradients remain exact for linear data"; limiting can destroy that property ([[ref-berger-aftosmis-murman-2005-slope-limiters]] III.B, Eqns. 20-22). In 1D the least-squares slope is a convex combination of the forward and backward differences, and becomes the central difference on a uniform mesh ([[ref-berger-aftosmis-murman-2005-slope-limiters]] Eq. 21). It is only first order accurate as a gradient, which is "sufficient for second-order accuracy overall" ([[ref-berger-aftosmis-murman-2005-slope-limiters]] III.B). Barth's 3D extensions outlined least-squares and data-weighted gradient procedures for unstructured grids ([[ref-venkatakrishnan-1995-icase-95-3]] 3). The fitting step is a small [[linear-regression-least-squares]] problem per cell.

**Use in reconstruction.** A piecewise-linear reconstruction in each cell, $U_i(x) = u_i + \phi(R_i)\, \nabla u_i \cdot (x - x_i)$, starts from the cell average and the cell gradient, with a limiter factor $\phi$ (Berger II.B, Eqn. 4). Higher-order reconstruction asks for conservation of the mean, k-exactness and compact support, with least squares for the coefficients ([[ref-venkatakrishnan-1995-icase-95-3]] 8).

**Boundary faces.** At boundaries the face value comes from the boundary condition (fixed value) or from extrapolation with the prescribed gradient ([[ref-jasak-1996-phd-thesis]] 3.3.3.1); see [[boundary-conditions]].

**Where gradients enter.** Linear-upwind face values ([[linear-upwind-scheme]]), the far-upwind estimate of normalised-variable schemes on unstructured meshes ([[ref-jasak-1996-phd-thesis]] 3.4.2), the explicit non-orthogonal correction ([[non-orthogonal-and-skewness-correction]]), slope limiters ([[slope-limiters-unstructured]]) and gradient-based error indicators ([[a-posteriori-error-indicators]]).

## Builds on

- [[finite-volume-method]]: the discrete Gauss theorem and the face values.
- [[linear-regression-least-squares]]: the least-squares fit of a linear function.

## Leads to

- [[linear-upwind-scheme]], [[slope-limiters-unstructured]], [[non-orthogonal-and-skewness-correction]], [[a-posteriori-error-indicators]]
- rcfd pages: [[adr-thread-gradient-options]], [[doc-case-schema-gradient-options]]

## How rcfd uses it

- F-021 (done): gradient schemes Green-Gauss (cell, node) and least squares, with Barth-Jespersen and Venkatakrishnan limiters ([[roadmap-numerics-and-solvers]]).
- `GradientOptions` (F-208, ADR 0016 D2) changes the explicit non-orthogonal part of $-\nabla \cdot (\gamma \nabla \phi)$: a face correction, a secondary limiter (Barth-Jespersen or Venkatakrishnan) and a boundary rule (`owner`, `corrected`, `dropped`), opt-in through the `solver.gradient_*` keys ([[adr-thread-gradient-options]]; [[doc-case-schema-gradient-options]]).
- "No benefit is claimed": on the manufactured problem the options keep second order but "do not reduce the error", and the face correction raised it 2.1 times on an affine skewed mesh ([[doc-case-schema-gradient-options]]).
