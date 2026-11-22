---
type: concept
tags: [foundation, linear-solvers]
sources: [ref-barrett-1994-templates-linear-systems, ref-mit-ocw-2-29-linear-systems, ref-shewchuk-1994-painless-cg]
builds_on: [finite-volume-method]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Sparse linear systems

## In short

Discretising a PDE turns it into a large system $A x = b$ in which each equation couples a cell only to its few neighbours, so almost every entry of $A$ is zero. How such a matrix is stored, and whether it is symmetric, positive definite, diagonally dominant or badly conditioned, decides which solver works and how fast.

## Explanation

- **Sparsity and storage.** Iterative methods spend their time in matrix-vector products and preconditioner solves, so the storage scheme matters. Compressed Row Storage keeps three arrays: the non-zero values row by row (`val`), their column indices (`col_ind`) and the start of each row (`row_ptr`); it needs $2\, nnz + n + 1$ locations instead of $n^2$ and assumes nothing about the pattern ([[ref-barrett-1994-templates-linear-systems]], sec. 4.3.1, pp. 57-58). Banded and skyline storage suit matrices whose non-zeros sit near the diagonal, as in finite-difference and finite-element methods ([[ref-mit-ocw-2-29-linear-systems]], lecture 7).
- **Cost per product.** A matrix-vector product costs work proportional to the number of non-zeros, which is what makes iterative methods cheap per step on sparse matrices ([[ref-shewchuk-1994-painless-cg]], sec. 10).
- **Symmetric positive definite (SPD).** $A = A^T$ and $x^T A x > 0$ for every $x \ne 0$; Sylvester's criterion: all leading principal minors positive. SPD matrices are "frequent in engineering"; for them Gauss elimination needs no pivoting and is stable ([[ref-mit-ocw-2-29-linear-systems]], lecture 7, slides 15-16). For SPD $A$, solving $A x = b$ is the same as minimising the quadratic form $f(x) = \tfrac{1}{2} x^T A x - b^T x + c$ ([[ref-shewchuk-1994-painless-cg]], sec. 3, eq. 3). This is why conjugate gradients apply to pressure and elasticity systems.
- **Diagonal dominance.** A strictly diagonally dominant matrix (each diagonal entry larger in magnitude than the sum of the others in its row) makes Jacobi and Gauss-Seidel converge ([[ref-mit-ocw-2-29-linear-systems]], lecture 8).
- **Condition number.** $K(A) = \lVert A \rVert\, \lVert A^{-1} \rVert$ measures how much relative errors in $b$ or $A$ are amplified in $x$; small changes in $b$ give large changes in $x$ for an ill-conditioned system ([[ref-mit-ocw-2-29-linear-systems]], lecture 6). For SPD matrices the spectral condition number is $\lambda_\text{max} / \lambda_\text{min}$ ([[ref-barrett-1994-templates-linear-systems]], sec. 2.3.1). Second-order elliptic PDEs give $\kappa(A) = O(h^{-2})$: refining the mesh makes the system harder, so unpreconditioned CG needs iterations proportional to $h^{-1}$ (same section). Geometrically, a large condition number stretches the quadratic form into a long narrow trough ([[ref-shewchuk-1994-painless-cg]], sec. 6.2).
- **Non-symmetric systems.** Convection makes the momentum matrix non-symmetric; CG then no longer applies directly and methods such as GMRES or Bi-CGSTAB are used ([[ref-barrett-1994-templates-linear-systems]], sec. 2.1).

## Builds on

- [[finite-volume-method]]: the cell balances whose assembled coefficients form $A$ (one row per cell, one entry per neighbour).
- Floor: matrices, eigenvalues, norms (first-year linear algebra).

## Leads to

- [[direct-solvers-lu-and-cholesky]], [[stationary-iterative-methods]], [[krylov-methods]] (and through them [[preconditioning]], [[geometric-multigrid]], [[amg]])
- [[simple]], [[structural-solver]]

## How rcfd uses it

- The GPU path implements CSR sparse matrix-vector products, dot/axpy/norm, Jacobi and Chebyshev preconditioning, and CG and BiCGStab on the device in f64; the matrix is assembled on the CPU and uploaded per solve ([[doc-gpu]]).
- The pressure solves of [[simple]], [[piso]] and pimple use CG/BiCGStab on a CUDA device when `linear_solver.device` asks for it ([[doc-gpu]], [[doc-case-schema-linear-solver]]).
- The solid solver's linear systems are covered in [[adr-thread-structural-linear-solvers]].
