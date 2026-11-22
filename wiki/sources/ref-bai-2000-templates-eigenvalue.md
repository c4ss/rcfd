---
type: source
tags: [reference, foundation, linear-solvers, eigenvalues]
sources: []
origin: https://www.netlib.org/utk/people/JackDongarra/etemplates/book.html
licence: none stated on the pages read. Unknown, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Bai, Demmel, Dongarra, Ruhe and van der Vorst (eds., 2000), Templates for the Solution of Algebraic Eigenvalue Problems

**Origin:** Z. Bai, J. Demmel, J. Dongarra, A. Ruhe and H. van der Vorst (editors), *Templates for the Solution of Algebraic Eigenvalue Problems: a Practical Guide*, SIAM, Philadelphia, 2000. HTML edition on Netlib (URL above; pages stamped 2000-11-20).
**Kind:** reference book, HTML. Locators are the Netlib node pages.
**Licence:** unknown; cite only.
**Read:** 2026-11-22 by job p6-linear-rotating: the table of contents and four sections of the Hermitian chapter: Power Method (node95), Inverse Iteration (node96), Subspace Iteration (node98), Lanczos Method by A. Ruhe (node103, apparently sec. 4.9). Algorithm boxes are images and were not read.

## Summary

A practical guide to eigenvalue algorithms, organised by problem type (Hermitian, generalised Hermitian, singular values, non-Hermitian, ...), each with single-vector iterations, Krylov methods (Lanczos, Arnoldi, implicitly restarted Arnoldi) and spectral transformations.

## Key points

- **Power method (node95, Algorithm 4.1):** repeated multiplication by `A` finds the eigenvalue of largest absolute value and its eigenvector; it fails only if the start vector is orthogonal to the dominant eigenvector (unlikely for a random start); the convergence rate is `|lambda_2 / lambda_1|`, slow when that ratio is close to 1.
- **Inverse iteration (node96):** the power method applied to `(A - sigma I)^-1`; it converges to the eigenvalue closest to the shift `sigma`, at rate `|(lambda_j - sigma)/(lambda_k - sigma)|` with `lambda_k` the second closest; the inverse is never formed, each step solves `(A - sigma I) y = v`, which needs a factorisation of `A - sigma I`.
- **Subspace iteration (node98):** the power method on a block of `p > 1` vectors, re-orthonormalised by QR each step, with a Rayleigh-Ritz projection `H = V* A V` whose eigenvalues approximate the `p` dominant eigenvalues; practical versions orthonormalise less often, use more vectors than wanted and lock converged eigenvalues.
- **Lanczos method (node103):** for Hermitian `A`, builds an orthogonal basis `V_j` of the Krylov subspace `K^j(A, v)` with one matrix-vector product a step; in that basis `A` is a real symmetric tridiagonal `T_j`, with `A V_j = V_j T_j + r e_j*`; eigenpairs of `T_j` give Ritz values and Ritz vectors `x_i = V_j s_i` with no extra products; the residual norm of a Ritz pair is `beta_j |s_(j,i)|`, so convergence is monitored without forming the Ritz vector. Subsections treat reorthogonalisation (full, selective, local), spurious Ritz values and a spectral transformation (shift-and-invert).
- **Table of contents:** a section "Spectral Transformations" (node84, Lehoucq and Sorensen) and the Arnoldi (node215) and implicitly restarted Arnoldi (node220) methods for non-Hermitian problems; these were not read.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[eigenvalue-solvers]]
