---
type: concept
tags: [foundation, pressure-velocity, linear-solver]
sources: [ref-klaij-vuik-2013-simple-preconditioners, ref-jasak-1996-phd-thesis, ref-mit-ocw-2-29-numerics, ref-venkatakrishnan-1995-icase-95-3]
builds_on: [pressure-velocity-coupling, krylov-methods, preconditioning]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Block-coupled (fully coupled) pressure-velocity solver

## In short

Instead of alternating between a momentum solve and a pressure solve, assemble momentum and continuity for all cells into one large linear system for velocity and pressure together and solve it with a Krylov method. The hard part is preconditioning a saddle-point matrix; the standard trick approximates the pressure Schur complement, which is exactly what SIMPLE does, so SIMPLE becomes the preconditioner instead of the solver ([[ref-klaij-vuik-2013-simple-preconditioners]]).

## Explanation

**The system.** $\begin{bmatrix} Q & G \ D & C \end{bmatrix} \begin{bmatrix} u \ p \end{bmatrix} = \begin{bmatrix} f \ g \end{bmatrix}$, with $Q$ the momentum blocks, $G$ gradient, $D$ divergence, $C$ stabilisation ([[ref-klaij-vuik-2013-simple-preconditioners]] Eqs. 1, 12). Jasak calls such approaches simultaneous algorithms (Caretto et al., Vanka) and notes their great cost in operations and memory compared with segregated PISO and SIMPLE ([[ref-jasak-1996-phd-thesis]] 3.8.2). The OCW notes: after linearisation the coupled linear system is large, and "Direct solution is not recommended: use an iterative scheme" ([[ref-mit-ocw-2-29-numerics]] L18 s.10-15).

**Block factorisation and Schur complement.** The block matrix factorises in LDU form with the Schur complement $S = C - D Q^{-1} G$ on the pressure diagonal; eliminating the velocity gives the pressure equation $S p' = r$ ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.1.1, Eq. 18). $Q^{-1}$ is never formed; replacing it by $\operatorname{diag}(Q)^{-1}$ gives SIMPLE, and other Schur-complement approximations give a family of methods from SIMPLE to purely algebraic ones ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.1.1).

**Krylov plus SIMPLE preconditioning.** The coupled correction $A x' = r$ is solved with FGMRES, right-preconditioned by one SIMPLE-type step; it is matrix-free, storing only $Q$ and the Laplacian $R = D \operatorname{diag}(Q)^{-1} G$, with inner solves to relative tolerance 0.01 (GMRES with Jacobi for $Q$, CG with incomplete Cholesky for $R$) ([[ref-klaij-vuik-2013-simple-preconditioners]] 3.2-3.3, Eqs. 10, 24). SIMPLER preconditioning reduced nonlinear iterations "by a factor 5-20" and CPU time "by a factor 2-5" against SIMPLE as a solver ([[ref-klaij-vuik-2013-simple-preconditioners]] abstract, 4.4).

**Implicit compressible counterpart.** For compressible flow solvers the analogous approach is implicit time stepping that tends to Newton's method as the time step grows, with GMRES and ILU preconditioning found superior over a range of problems ([[ref-venkatakrishnan-1995-icase-95-3]] 6.1.2); see [[newton-raphson-method]].

## Builds on

- [[pressure-velocity-coupling]]: the saddle-point system.
- [[krylov-methods]]: GMRES/FGMRES for the coupled system.
- [[preconditioning]]: the Schur-complement preconditioner.

## Leads to

- rcfd pages: [[amg]], [[doc-case-schema-coupled]]

## How rcfd uses it

- `solver.algorithm = "coupled"` (F-032) is rcfd's steady block-coupled solver: $(u, v[, w], p)$ in one linear system per Picard iteration, right-preconditioned GMRES with ILU(0) on the momentum blocks and "one aggregation-AMG V-cycle on the approximate pressure Schur complement" ([[doc-case-schema-coupled]]; [[amg]]).
- Implicit momentum relaxation `solver.alpha_u` 0.95 to 0.98 works on the cavity; no pressure relaxation; known limits include steady only and a decaying odd-even pressure mode next to a prescribed-flux inlet ([[doc-case-schema-coupled]]).
- The component-diagonal choice does not affect the coupled solver ([[component-diagonal]]).
