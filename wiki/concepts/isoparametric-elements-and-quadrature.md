---
type: concept
tags: [foundation, structural, fem]
sources: [ref-bathe-mit-2-094-fea-notes, ref-felippa-ifem, ref-roylance-mit-3-11-mechanics-of-materials, adr-thread-structural-basic-tier, adr-thread-structural-verification-elements]
builds_on: [finite-element-method-basics]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Isoparametric elements and Gauss quadrature

## In short

Real elements are distorted quadrilaterals, bricks or tetrahedra. Isoparametric elements describe each one as a map from a fixed reference shape (a square $[-1,1]^2$ or cube) using the same shape functions that interpolate the displacement. The stiffness integrals are then computed on the reference shape by Gauss quadrature, a weighted sum of the integrand at a few optimally placed points ([[ref-bathe-mit-2-094-fea-notes]], lecture 7; [[ref-felippa-ifem]] ch. 17).

## Explanation

**The map.** Natural coordinates $(r, s)$ (Felippa: $(\xi, \eta)$) correspond one-to-one with $(x, y)$: $x = \sum h_i x_i$, $y = \sum h_i y_i$, with the same $h_i$ as the displacements ("iso-parametric") ([[ref-bathe-mit-2-094-fea-notes]], lecture 7). Roylance notes the economy of using the interpolation functions for the mapping too ([[ref-roylance-mit-3-11-mechanics-of-materials]], "Finite Element Analysis").

**Jacobian and derivatives.** $\mathbf J$ collects $\partial(x, y)/\partial(\xi, \eta)$, computed from the nodal coordinates and the shape-function derivatives (Felippa, eqs. 17.7-17.8), and inverted numerically at each point to get the $x$- and $y$-derivatives needed in $\mathbf B$ by the chain rule (17.5, 17.9) ([[ref-felippa-ifem]]). $\mathbf J$ must be non-singular for the correspondence to be unique ([[ref-bathe-mit-2-094-fea-notes]], lecture 7). Then $$\mathbf K = \int_{-1}^{1}\!\int_{-1}^{1}\mathbf B^T\mathbf C\,\mathbf B\;t\,\det\mathbf J\;dr\,ds$$ (Bathe, eq. 7.11).

**Gauss quadrature.** On $[-1, 1]$ a $p$-point Gauss rule integrates polynomials up to degree $2p - 1$ exactly (Felippa, sec. 17.3.1, Table 17.1); in 2D the product rule is $$\int_{-1}^{1}\!\int_{-1}^{1}F(\xi,\eta)\,d\xi\,d\eta \approx \sum_{i=1}^{p_1}\sum_{j=1}^{p_2} w_i w_j\,F(\xi_i, \eta_j)$$ (17.15) ([[ref-felippa-ifem]]), and $\mathbf K \approx t\sum_{i,j}\mathbf B_{ij}^T\mathbf C\mathbf B_{ij}\det\mathbf J_{ij}\,w_{ij}$ (Bathe, eq. 7.13). The 4-node element uses 2x2 points, the 9-node element 3x3; for a rectangle $\mathbf J$ is constant and 3x3 gives the exact stiffness, a distorted element is still accurate enough if the rule is high enough ([[ref-bathe-mit-2-094-fea-notes]], lecture 7). Useful rules are fully symmetric, have positive weights and interior points (Felippa, sec. 17.3.3).

**Rank.** Too few points make $\mathbf K$ rank deficient: a 1x1 rule on the 4-node quadrilateral leaves it deficient by two, so at least 2x2 is needed for rank sufficiency (Felippa, sec. 17.5.1). The deficient modes are zero-energy deformation patterns; this is the trade-off exploited by reduced and selective integration against locking ([[element-locking]]) ([[ref-felippa-ifem]]).

**Not covered by the sources read.** Specific rules for tetrahedra (Tet4, Tet10) and the 20-node brick were not in the chapters read.

## Builds on

- [[finite-element-method-basics]]: shape functions, $\mathbf B$, element stiffness as an integral.

## Leads to

- [[element-locking]], [[stress-recovery]] (values at Gauss points), [[incompatible-modes-hex8]], [[structural-solver]]

## How rcfd uses it

- The element library of [[rcfd-fem]] has Tet4, Hex8 and Tet10 (ADR 0273), and Hex20 (ADR 0292) ([[adr-thread-structural-basic-tier]]; [[structural-solver]]).
- Verification on distorted meshes: the observed orders of Tet4, Hex8 and Tet10 with moved interior nodes (F-459, ADR 0355), the Hex8 patch test on a moved mesh (F-461), curved Hex20 on a quarter annulus (F-508) ([[adr-thread-structural-verification-elements]]).
- Stresses are stored per integration point (`IntegrationPointField`, ADR 0277) ([[adr-thread-structural-basic-tier]]).
