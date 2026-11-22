---
type: concept
tags: [foundation, linear-solvers, eigenvalues]
sources: [ref-bai-2000-templates-eigenvalue, ref-barrett-1994-templates-linear-systems]
builds_on: [krylov-methods, direct-solvers-lu-and-cholesky]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Eigenvalue solvers (power, inverse and subspace iteration, Lanczos)

## In short

Large eigenvalue problems (natural frequencies of a structure, stability modes) are solved iteratively, using `A` only through matrix-vector products or solves. Repeated multiplication picks out the largest eigenvalue; multiplying by a shifted inverse picks out the one nearest a chosen value; Krylov methods (Lanczos) extract many eigenvalues at once from a small tridiagonal matrix.

## Explanation

- **Power method.** Multiply a start vector by `A` again and again (with normalisation); the iterates turn toward the eigenvector of the eigenvalue of largest magnitude, at rate `|lambda_2 / lambda_1|`, slow when that ratio is near 1; it fails only if the start is orthogonal to the dominant eigenvector ([[ref-bai-2000-templates-eigenvalue]], node95).
- **Inverse iteration and shifts.** Apply the power method to `(A - sigma I)^-1`: it converges to the eigenvalue closest to the shift `sigma`, at rate `|(lambda_j - sigma)/(lambda_k - sigma)|` with `lambda_k` the second closest, so a good shift gives fast convergence. The inverse is never formed: each step solves `(A - sigma I) y = v`, which needs a factorisation of `A - sigma I` ([[ref-bai-2000-templates-eigenvalue]], node96). This is the basic form of a "shift-invert" spectral transformation (the book's sec. "Spectral Transformations", node84, not read).
- **Subspace iteration.** Iterate a block of `p` vectors, re-orthonormalise with QR, and project: `H = V* A V` is `p x p` and its eigenvalues (Ritz values) approximate the `p` dominant eigenvalues; practical codes use more vectors than wanted and lock converged ones ([[ref-bai-2000-templates-eigenvalue]], node98).
- **Lanczos.** For Hermitian (real symmetric) `A`, build an orthonormal basis `V_j` of the Krylov subspace `span{v, A v, ..., A^(j-1) v}`, one matrix-vector product per step; in that basis `A` becomes a symmetric tridiagonal `T_j` (`A V_j = V_j T_j + r e_j*`). Eigenpairs of `T_j` give Ritz values and Ritz vectors `x_i = V_j s_i`; the residual of each Ritz pair is `beta_j |s_(j,i)|`, so convergence is checked without forming `x_i` ([[ref-bai-2000-templates-eigenvalue]], node103). In floating point the basis loses orthogonality, so practical codes reorthogonalise (full, selective or local) and must detect spurious Ritz values (subsections of node103).
- **Link to CG.** CG and Lanczos build the same orthogonal Krylov basis and the same tridiagonal matrix, so CG coefficients carry eigenvalue information about the (preconditioned) matrix ([[ref-barrett-1994-templates-linear-systems]], secs. 2.3.1, 5.1).

## Builds on

- [[krylov-methods]]: Krylov subspaces, orthogonalisation, Arnoldi/Lanczos recurrences.
- [[direct-solvers-lu-and-cholesky]]: inverse iteration and shift-invert factor `A - sigma I`.
- Floor: eigenvalues and eigenvectors.

## Leads to

- [[modal-analysis]], [[campbell-diagram]]

## How rcfd uses it

- rcfd's structural solver does modal analysis in its basic tier (ADR 0270) and buckling and cyclic-symmetry modes in later tiers ([[structural-solver]], [[cyclic-symmetry]]).
- Campbell tables track modes of a rotating part from speed to speed, with forward and backward whirl from the gyroscopic matrix (F-396, F-499) ([[campbell-diagram]]).
- Which eigenvalue algorithm rcfd's modal solver uses is not stated on these wiki pages.
