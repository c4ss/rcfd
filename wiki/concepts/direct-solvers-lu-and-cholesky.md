---
type: concept
tags: [foundation, linear-solvers]
sources: [ref-mit-ocw-2-29-linear-systems, ref-barrett-1994-templates-linear-systems, ref-shewchuk-1994-painless-cg]
builds_on: [sparse-linear-systems]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Direct solvers: LU and Cholesky

## In short

A direct solver eliminates unknowns one by one (Gauss elimination) and so factors the matrix into triangular pieces, `A = L U`; after that each right-hand side costs only two triangular substitutions. Row interchanges (pivoting) keep the elimination safe; for symmetric positive definite matrices the symmetric form `A = L L^T` (Cholesky) needs none.

## Explanation

- **Gauss elimination.** Forward elimination subtracts multiples of the pivot row to zero the entries below the pivot, reducing the system to upper triangular form; back substitution then solves from the last unknown upward. The work is `O(2/3 n^3)`, most of it in the elimination ([[ref-mit-ocw-2-29-linear-systems]], lecture 5). Cramer's rule, by contrast, costs `O(n!)` and is impractical beyond `n = 3` (same lecture).
- **Pivoting.** A pivot must be non-zero and should not be close to zero. Partial pivoting searches the largest coefficient in the pivot column below the pivot and interchanges rows; full pivoting searches rows and columns but reorders the unknowns and is rarely done; codes usually record the interchanges in a pivot vector rather than move rows; equations should be equilibrated (scaled) first. Neither guarantees stability ([[ref-mit-ocw-2-29-linear-systems]], lecture 5).
- **LU factorisation.** The multipliers of the elimination form a unit lower triangular `L`, the reduced matrix is `U`, and `A = L U` (Doolittle form, stored compactly in place of `A`). Solving `A x = b` is then `L y = b` (forward) and `U x = y` (backward). With pivoting the factorisation is of the row-permuted matrix, and the permutation recorded in `p` is applied to the right-hand side as `b(p(k))` ([[ref-mit-ocw-2-29-linear-systems]], lecture 6). The order matters: the interchanges must be applied to `b` consistently with how they were applied during the factorisation.
- **Multiple right-hand sides.** Factor once, `O(n^3)`; each further right-hand side costs `O(n^2)`. This is what makes LU worth keeping when right-hand sides arrive one after another ([[ref-mit-ocw-2-29-linear-systems]], lecture 5).
- **Cholesky.** For a symmetric positive definite matrix no pivoting is needed and the elimination is stable; the factorisation is `A = L L^T` ([[ref-mit-ocw-2-29-linear-systems]], lecture 7, slides 15-16). Skyline solvers are usually based on Cholesky, which preserves the skyline (profile) of a banded SPD matrix (same lecture, skyline slide).
- **Fill-in.** Factorising a sparse matrix creates non-zeros where `A` had zeros ("fill"). Incomplete factorisations used as preconditioners deliberately drop that fill ([[ref-barrett-1994-templates-linear-systems]], sec. 3.4; [[ref-shewchuk-1994-painless-cg]], sec. 12); a complete factorisation keeps it, which is why direct solvers become expensive in memory on large 3-D meshes. Fill-reducing orderings and supernodal techniques (in the map's scope for this node) were not covered by the sources read.

## Builds on

- [[sparse-linear-systems]]: SPD-ness decides between LU with pivoting and Cholesky; sparsity decides how much fill a factorisation creates.

## Leads to

- [[preconditioning]] (incomplete LU and Cholesky), [[amg]] (dense LU on the coarsest level), [[eigenvalue-solvers]] (inverse iteration factors `A - sigma I`)
- [[gaussian-process-surrogate]] (Cholesky of a covariance matrix), [[adr-thread-structural-linear-solvers]]

## How rcfd uses it

- **AMG coarse level:** the coarsest level of rcfd's AMG is solved by a dense LU; it applied its row interchanges in the wrong order whenever a pivot interchange happened at a step `k >= 1`, giving a coarse residual of `0.213` instead of `5.3e-15` and an indefinite V-cycle; the corrected order (row interchanges applied before the forward substitution) is the default since ADR 0743 ([[adr-thread-amg-coarse-lu]], [[amg]]).
- **Structural solver:** direct and PCG solves in the basic tier ([[structural-solver]]); the sparse direct route is part of [[adr-thread-structural-linear-solvers]].
