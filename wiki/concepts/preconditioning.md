---
type: concept
tags: [foundation, linear-solvers, preconditioner]
sources: [ref-barrett-1994-templates-linear-systems, ref-shewchuk-1994-painless-cg, ref-falgout-2006-amg-intro]
builds_on: [krylov-methods, stationary-iterative-methods]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Preconditioning

## In short

A preconditioner $M$ is a cheap approximation of $A$ (or of $A^{-1}$). Solving $M^{-1} A x = M^{-1} b$ instead of $A x = b$ gives the same answer, but if $M^{-1} A$ is better conditioned a Krylov method needs far fewer iterations. Choosing $M$ is a trade-off between how well it approximates $A$ and how much it costs to build and apply.

## Explanation

- **Idea.** The convergence of Krylov methods depends on the spectrum of the matrix; a preconditioner transforms the system into one with the same solution and more favourable spectral properties ([[ref-barrett-1994-templates-linear-systems]], sec. 3.1). In the CG picture it stretches the quadratic form so it looks more spherical; the perfect preconditioner $M = A$ gives condition number one and one iteration, but applying it is the original problem ([[ref-shewchuk-1994-painless-cg]], sec. 12). "For large-scale applications, CG should nearly always be used with a preconditioner" (same section).
- **Cost trade-off.** Construction and application cost against the gain in iterations; most preconditioners only improve the iteration count by a constant factor, while modified incomplete factorisations and multigrid-based preconditioners can do better ([[ref-barrett-1994-templates-linear-systems]], sec. 3.1.1).
- **Preserving symmetry.** $M^{-1} A$ is not symmetric even when $A$ and $M$ are, so preconditioned CG uses the $M^{-1}$ inner product instead; $M$ must be SPD ([[ref-barrett-1994-templates-linear-systems]], secs. 2.3.1, 3.1.2). Left and right preconditioning ($M_1^{-1} A M_2^{-1}$) is the general form.
- **Jacobi (diagonal).** $M = \operatorname{diag}(A)$; trivial to invert, needs almost no storage, parallelises without difficulty, "often only a mediocre preconditioner" ([[ref-barrett-1994-templates-linear-systems]], sec. 3.2; [[ref-shewchuk-1994-painless-cg]], sec. 12). Block Jacobi inverts diagonal blocks chosen per node (several unknowns per node), per line or plane, or per processor (sec. 3.2.1).
- **SSOR.** $M = (D + L) D^{-1} (D + L)^T$ in the symmetric case, available at no construction cost (sec. 3.3). (SOR and Gauss-Seidel themselves are not used as preconditioners, footnote in sec. 3.3.)
- **Incomplete factorisations (ILU, IC).** Gauss elimination that ignores some or all fill gives $M = L U$ (incomplete Cholesky in the SPD case); ILU(0) keeps exactly the sparsity of $A$; D-ILU alters only the diagonal and coincides with ILU(0) on central-difference Cartesian matrices. They may break down (zero pivot) or become indefinite even when the full factorisation exists; they are guaranteed to exist for M-matrices; building them costs about one solve and parallelises poorly (sec. 3.4). Incomplete Cholesky "is not always stable" ([[ref-shewchuk-1994-painless-cg]], sec. 12).
- **Polynomial and Chebyshev.** A preconditioner can approximate $A^{-1}$ by a polynomial in $A$, applied with matrix-vector products only (sec. 3.5); the Chebyshev iteration needs spectral bounds but no inner products (sec. 2.3.9).
- **Additive Schwarz.** Split the unknowns into overlapping subdomains, solve each locally and add the corrections, plus a coarse-grid solve; with enough overlap the condition number is bounded independently of the mesh sizes; without the coarse term it grows as $O(1/H^2)$ (sec. 5.4.1).
- **Multigrid as preconditioner.** An AMG cycle used inside CG must be symmetric; then AMG-CG converged in 8 or 9 iterations for all sizes of Falgout's example ([[ref-falgout-2006-amg-intro]], classical AMG section).

## Builds on

- [[krylov-methods]]: the iteration being accelerated and its dependence on the condition number.
- [[stationary-iterative-methods]]: Jacobi, block Jacobi and SSOR are the splittings reused as $M$.

## Leads to

- [[amg]], [[geometric-multigrid]]
- [[doc-case-schema-linear-solver]] (rcfd's preconditioner keys)

## How rcfd uses it

- **GPU:** `linear_solver.preconditioner` is `jacobi` (default) or `chebyshev` (`chebyshev_degree` 4, range 0..32) ([[doc-case-schema-linear-solver]]). On the GPU "Jacobi or Chebyshev replace DIC/AMG" ([[doc-gpu]], [[amg]]).
- **CPU pressure solves:** smoothed-aggregation AMG preconditions CG ([[amg]]).
- **Structural solver:** incomplete Cholesky with zero fill needs 0.545 of block Jacobi's iterations against the limit 0.5; IC with one level of fill after reverse Cuthill-McKee ordering meets it at 0.372 (F-381, F-460, ADR 0357); `amg`, `amg_quadratic`, `amg_quadratic_smoothed` CG preconditioners ([[adr-thread-structural-linear-solvers]], [[amg]]).
