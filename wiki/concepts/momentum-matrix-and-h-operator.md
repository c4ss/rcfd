---
type: concept
tags: [foundation, pressure-velocity]
sources: [ref-jasak-1996-phd-thesis, ref-klaij-vuik-2013-simple-preconditioners]
builds_on: [finite-volume-method, sparse-linear-systems]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Momentum matrix and the H operator

## In short

Discretising the momentum equation on a cell gives one linear equation per cell and velocity component: a diagonal coefficient $a_P$ times the cell velocity, plus the neighbours' coefficients times their velocities, equals sources minus the pressure gradient. Moving everything except the diagonal term and the pressure gradient to one side defines $H(U)$, so that $a_P U_P = H(U) - \nabla p$. Dividing by $a_P$ expresses the cell velocity through the pressure gradient, which is the step every pressure-correction method starts from ([[ref-jasak-1996-phd-thesis]] 3.8.1).

## Explanation

**Algebraic form.** Each discretised transport equation reads $a_P \phi_P + \sum_N a_N \phi_N = R_P$, one row of a sparse matrix whose diagonal is $a_P$ and whose off-diagonal entries $a_N$ couple the cell to its face neighbours ([[ref-jasak-1996-phd-thesis]] Eqns. 3.42-3.43). For momentum, $\phi$ is a velocity component and the pressure gradient is left undiscretised for now.

**The H operator.** "In the spirit of the Rhie and Chow procedure", the semi-discrete momentum equation is $a_P U_P = H(U) - \nabla p$ (Eqn. 3.136), where $H(U)$ contains the neighbour part, minus the sum of $a_N U_N$, and the source part, which holds the source part of the transient term and all other sources except the pressure gradient (Eqn. 3.137) ([[ref-jasak-1996-phd-thesis]] 3.8.1). Hence $U_P = H(U)/a_P - (\nabla p)/a_P$ (Eqn. 3.139). The quantity $H/a_P$ (often written `HbyA`) is the velocity the momentum equation would give without the pressure gradient.

**Matrix view.** In the block system of [[pressure-velocity-coupling]] this is the momentum block $Q$; SIMPLE replaces $Q^{-1}$ by $\operatorname{diag}(Q)^{-1}$, i.e. by $1/a_P$ cell by cell ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.1.1, Eqs. 13-19). Only first-order upwind enters $Q$ when higher-order convection is applied as a defect correction ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.2). With Picard linearisation and the transpose viscous term treated explicitly the velocity-component blocks are equal ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.2), so one $a_P$ serves all components; when boundary conditions act on single components the diagonals differ (rcfd below).

**What depends on $a_P$.** Implicit time stepping and implicit under-relaxation both add to the diagonal, so $a_P$ and anything built from it (the pressure equation, the Rhie-Chow face flux) depend on the time step and relaxation factor ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.5); see [[rhie-chow-interpolation]] and [[under-relaxation]].

## Builds on

- [[finite-volume-method]]: the discretised momentum equation per cell.
- [[sparse-linear-systems]]: the matrix with diagonal $a_P$ and neighbour coefficients $a_N$.

## Leads to

- [[pressure-correction-equation]], [[rhie-chow-interpolation]]
- rcfd pages: [[simple]], [[component-diagonal]]

## How rcfd uses it

- `solver.component_diagonal` chooses the diagonal used for `HbyA` $= H/a_P$ and the pressure equation: `averaged` (mean of the velocity-component diagonals) or `corrected`, which adds $(a_P - a_k)\, u_k$ to $H_k$ (ADR 0010) ([[component-diagonal]]).
- `SimpleSettings.h_from_solution` (default true since wave 6) builds $H$ from the solved predictor; the old lagged $H$ from the previous iterate made the iteration "only marginally stable" ([[simple]]).
