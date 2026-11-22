---
type: concept
tags: [foundation, discretisation]
sources: [ref-jasak-1996-phd-thesis, ref-klaij-vuik-2013-simple-preconditioners, ref-mit-ocw-2-29-numerics]
builds_on: [finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Deferred correction

## In short

Keep the matrix simple and well behaved, and put the awkward part of the discretisation on the right-hand side, computed from the latest available solution. Iterating then makes the converged answer satisfy the full (higher-order or non-orthogonal) discretisation, while every linear solve only sees the easy matrix ([[ref-jasak-1996-phd-thesis]] 3.5).

## Explanation

**Why.** Iterative linear solvers need a diagonally dominant matrix, and among the convection schemes only upwind differencing gives one under all conditions; higher-order schemes also reach past the immediate neighbours, and the OCW notes say such large molecules lead to "deferred-correction schemes and/or compact (Pade') schemes" ([[ref-jasak-1996-phd-thesis]] 3.5; [[ref-mit-ocw-2-29-numerics]] L16 s.18).

**How (Khosla and Rubin).** Split the face value of the high-order scheme into the upwind value and a remainder, $\phi_f = \phi_f^{UD} + (\phi_f^{HO} - \phi_f^{UD})$. The upwind part goes into the matrix; the bracket is evaluated with the current solution and moved into the source. When the iterations converge the bracket uses the converged field, so the result is the high-order solution (own restatement of Jasak's description: "any differencing scheme is treated as an upgrade of UD", the UD part built into the matrix and "the other part is added into the source term", [[ref-jasak-1996-phd-thesis]] 3.5). This makes the matrix diagonally equal but does not affect boundedness, "as the 'troublesome' part of the discretisation still exists in the source term" ([[ref-jasak-1996-phd-thesis]] 3.5). Klaij and Vuik use the same form: only first-order upwind enters the momentum matrix because "higher-order advection schemes are implemented in defect correction form" ([[ref-klaij-vuik-2013-simple-preconditioners]] 2.2).

**Non-orthogonal correctors.** The non-orthogonal part of the diffusion flux, $k \cdot (\nabla \phi)_f$, is treated the same way: computed from the previous solution and added to the source; the equation is then solved repeatedly within one step or iteration with an updated correction ("non-orthogonal correctors"), used where the non-orthogonal parts must be resolved, as in the pressure equation; this too "only improves the quality of the matrix but does not guarantee boundedness" ([[ref-jasak-1996-phd-thesis]] 3.5). See [[non-orthogonal-and-skewness-correction]].

**Side effect.** The explicit part is a fixed-point iteration on top of the linear solve; when it is large (a strongly non-orthogonal mesh, a high-order scheme on a steep gradient), it can slow or stop convergence. Jasak's Laplace tests on non-orthogonal meshes show this: some decompositions stop converging at 40 to 45 deg ([[ref-jasak-1996-phd-thesis]] 3.7.4).

## Builds on

- [[finite-volume-method]]: the face values and fluxes being split.
- Floor: fixed-point iteration (see [[stationary-iterative-methods]]).

## Leads to

- [[linear-upwind-scheme]], [[non-orthogonal-and-skewness-correction]]
- rcfd pages: [[simple]], [[mesh-adaptation]], [[fan-01]]

## How rcfd uses it

- **Warped mesh W:** rcfd's default steady kernel diverges on F-208's warped mesh W from $n = 32$ because "the deferred-correction pressure step" has an eigenvalue of about $-1.18$; the opt-in `SimpleSolver::implicit_non_orth_pressure` solves the non-orthogonal part implicitly, converges W at $n = 16, 32, 64$ and leaves converged results elsewhere unchanged to $3 \times 10^{-12}$ ([[simple]]).
- **Adapted meshes:** non-orthogonal correction on adapted meshes needs deferred iterations (ADR 0054 addendum 1) ([[mesh-adaptation]]).
- **FAN-01:** the Barth-Jespersen limit cycle on B3 is driven by the convection deferred correction at two sites, the blade-root leading edge and the hub trailing edge (ADRs 0175, 0177, 0178) ([[fan-01]]).
