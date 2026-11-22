---
type: source
tags: [reference, foundation, pressure-velocity, cite-only]
sources: []
origin: https://diamhomes.ewi.tudelft.nl/~kvuik/papers/Kla12V.pdf
licence: "Copyright (c) 2012 John Wiley & Sons, Ltd." (printed on every page of the author-hosted copy, checked 2026-11-22); cite only
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Klaij and Vuik (2013), SIMPLE-type preconditioners for cell-centered, colocated finite volume discretization of incompressible RANS equations

**Origin:** C. M. Klaij and C. Vuik, "SIMPLE-type preconditioners for cell-centered, colocated finite volume discretization of incompressible Reynolds-averaged Navier-Stokes equations", *Int. J. Numer. Meth. Fluids* 71(7):830-849, 2013 (online 4 May 2012), doi:10.1002/fld.3686. Read from the copy on C. Vuik's TU Delft home page (URL above).
**Kind:** journal paper (author-hosted copy).
**Licence:** Wiley copyright; read and cited, never copied.
**Read:** 2026-11-22 by job p6-pv-discretisation: sections 1 to 4.4 (discretisation, iterative method, academic test cases and their relaxation tables). Locators are section and equation numbers.

## Summary

A finite-volume RANS code with collocated variables on unstructured grids (MARIN's REFRESCO) is moved from the classical segregated SIMPLE solver to SIMPLE-type methods used as preconditioners for a Krylov solver of the coupled mass-momentum system. The paper defines the discretisation as a block saddle-point system, derives SIMPLE and SIMPLER from it via the Schur complement, and shows how the pressure-weighted interpolation (a Rhie-Chow-type stabilisation) is handled without forming its wide-stencil matrix.

## Key points (own summary)

- **Block system (2, Eq. 1; 3, Eq. 12):** Picard linearisation and finite-volume discretisation give $\begin{bmatrix} Q & G \\ D & C \end{bmatrix} \begin{bmatrix} u \\ p \end{bmatrix} = \begin{bmatrix} f \\ g \end{bmatrix}$ with $Q$ the momentum (convection-diffusion) blocks, $G$ the pressure gradient, $D$ the divergence and $C$ the stabilisation matrix. With Picard linearisation and the explicit treatment of the transpose viscous term the velocity blocks are equal, $Q_1 = Q_2$ (2.2).
- **Checkerboard and PWIM (2.1, Eqs. 2-3):** "The pressure-weighted interpolation method (PWIM) is used to avoid checkerboard oscillations on grids with colocated variables"; it adds to the velocity a term $h^2$ times a second derivative of the pressure gradient, i.e. a fourth-order pressure derivative in momentum and mass, which vanishes as $h \to 0$ and is scaled not to spoil second-order accuracy.
- **Face interpolation (2.2-2.3, Eqs. 5-9):** only first-order upwind enters $Q$ because "higher-order advection schemes are implemented in defect correction form"; face pressure and velocity are linear interpolations with a factor 1/2 on uniform grids; the PWIM coefficients are, on each side, the interpolation weight times the cell volume over that cell's $\operatorname{diag}(Q)$ (Eq. 9); the matrix $C$ "involves not only the neighbors of the cell but also the neighbors of the neighbors".
- **Origin of the PWIM (2.5):** under the SIMPLE approximation $Q^{-1} \approx \operatorname{diag}(Q)^{-1}$, a cell velocity can be "interchanged" with its pressure gradient; doing this for the part of the face velocity that is interpolated from the cells recovers exactly the PWIM term. Defect correction keeps $\operatorname{diag}(Q)$ and so $C$ independent of the advection scheme.
- **Dependence on time step and relaxation (2.5):** implicit time discretisation or implicit relaxation adds a diagonal term to $Q$; through the PWIM this makes "the converged steady-state solution depends on the user's choice of time-step and/or implicit relaxation factor (although the effect vanishes with grid refinement)"; the authors therefore omit these contributions from $\operatorname{diag}(Q)$ in the PWIM only.
- **SIMPLE derived (3.1.1, Eqs. 13-19):** predict $u^*$ from $Q u^* = f - G p^k$; corrections $u' = -Q^{-1} G p'$; continuity gives $(C - D Q^{-1} G)\, p' = g - D u^* - C p^k$, where $C - D Q^{-1} G$ "is the Schur complement"; SIMPLE replaces $Q^{-1}$ by $\operatorname{diag}(Q)^{-1}$. Steps: solve for $u^*$, solve for $p'$, correct $u$, update $u$ and $p$. $p'$ is zero where $p$ has a Dirichlet condition and has zero normal derivative where $p$ has a Neumann condition. An equivalent derivation factorises the block matrix in LDU form, with the Schur complement on the diagonal; different Schur-complement approximations give methods from SIMPLE to purely algebraic ones.
- **Relaxation forms (3.1.2):** velocity prediction with relaxation $u^{k+1} = (1 - w_u)\, u^k + w_u u^*$, or the equivalent correction form; the pressure is relaxed with $w_p$.
- **SIMPLER (3.1.3, Eqs. 20-23):** an extra pressure-prediction equation $(C - D \operatorname{diag}(Q)^{-1} G)\, p^* = g - D u^k - D \operatorname{diag}(Q)^{-1} (f - Q u^k)$.
- **As preconditioner (3.2-3.3, Eq. 24):** a stationary nonlinear iteration whose correction solves $A x' = r$ with FGMRES right-preconditioned by one SIMPLE-type step; matrix-free, storing only $Q$ and the Laplacian $R = D \operatorname{diag}(Q)^{-1} G$ (Eq. 10); the inner systems are solved to relative tolerance 0.01 (GMRES with Jacobi for $Q$, CG with incomplete Cholesky for $R$); $C$ is dropped from the pressure operator in the preconditioner.
- **Implicit relaxation (3.4, Eq. 25):** $\left(Q + \dfrac{1 - w_i}{w_i} \operatorname{diag}(Q)\right) u^{k+1} = f + \dfrac{1 - w_i}{w_i} \operatorname{diag}(Q)\, u^k$, which leaves the converged equation unchanged; it is supplementary to the explicit relaxation by $w_u$ and $w_p$. A simulation "must start with a constant pressure field" because the PWIM needs $\operatorname{diag}(Q)$ of the previous iteration.
- **SIMPLEC (3.4):** at high Reynolds numbers $\operatorname{diag}(Q)$ became very small in some cells and caused sudden divergence, so $Q^{-1} \approx \operatorname{diag}(Q)^{-1}$ was replaced by the SIMPLEC approximation built from the row sums of absolute values of $Q$.
- **Relaxation sensitivity (4.4, Table I):** as a solver, SIMPLE "is highly sensitive to the relaxation parameters, notably $w_p$"; on the backward-facing step (96 x 48) "The only choice that gives convergence is $w_p = 0.2$"; SIMPLER "does not diverge for any of the parameter choices". As preconditioners (Table II) all variants need far fewer nonlinear iterations; abstract: SIMPLER preconditioning cuts nonlinear iterations "by a factor 5-20" and CPU time "by a factor 2-5".

## Contradictions with the wiki

None found.

## Pages this source informs

- [[pressure-velocity-coupling]], [[pressure-correction-equation]], [[momentum-matrix-and-h-operator]], [[collocated-grid-and-checkerboarding]], [[rhie-chow-interpolation]], [[under-relaxation]], [[block-coupled-solver]], [[deferred-correction]], [[boundary-conditions]]
