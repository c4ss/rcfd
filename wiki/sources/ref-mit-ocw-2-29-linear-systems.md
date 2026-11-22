---
type: source
tags: [reference, foundation, linear-solvers, non-commercial]
sources: []
origin: https://ocw.mit.edu/courses/2-29-numerical-fluid-mechanics-spring-2015/pages/lecture-notes-and-references/
licence: CC BY-NC-SA 4.0 (course page footer, verified 2026-11-22). Non-commercial, so cite only (maintainer decision 6, 2026-11-22).
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# MIT OCW 2.29 Numerical Fluid Mechanics (Spring 2015), lectures 5-8: linear systems

**Origin:** P. F. J. Lermusiaux, *2.29 Numerical Fluid Mechanics*, Spring 2015, MIT OpenCourseWare, lecture notes 5, 6, 7 and 8 (slides "PFJL Lecture n, k"). PDFs from the course's resource pages (lecture 5: `.../911ee683a874c35ef3d00b63922e97ce_MIT2_29S15_Lecture5.pdf`; 6: `.../c5feb44463a8c913d025d636ba383850_MIT2_29S15_Lecture6.pdf`; 7: `.../1839ca0d0ab0b2de100c75d5a629331e_MIT2_29S15_Lecture7.pdf`; 8: `.../c6c41d22b990dda0228a1152cb54ee3a_MIT2_29S15_Lecture8.pdf`).
**Kind:** lecture slides. Locators are lecture and slide number.
**Licence:** CC BY-NC-SA 4.0; cite only. (A separate page, [[ref-mit-ocw-2-29-turbulence]], covers the turbulence lectures of the same course.)
**Read:** 2026-11-22 by job p6-linear-rotating, from text extractions of the four PDFs; slide equations were partly lost.

## Summary

The direct and iterative solution of linear systems as used in CFD: Cramer's rule and why it is impractical, Gauss elimination with operation counts and pivoting, LU factorisation, condition number and error analysis, special matrices (tridiagonal, banded, skyline, symmetric positive definite, Cholesky), then Jacobi, Gauss-Seidel and SOR with their convergence conditions, and an introduction to gradient (steepest descent, conjugate gradient) methods.

## Key points

- **Lecture 5, Cramer's rule:** impractical for `n > 3`; operation count `O(n!)`.
- **Lecture 5, Gauss elimination:** forward elimination to an upper triangular system, then back substitution; grand total `O(2/3 n^3)` operations, most of them in the elimination; pivot elements must be non-zero and not close to zero.
- **Lecture 5, pivoting:** partial pivoting by columns (search the largest coefficient below the pivot and swap rows), full pivoting (rows and columns, "rarely done" because it reorders the unknowns); codes usually keep a pointer vector instead of swapping rows; equations should be equilibrated (normalised) for partial pivoting; neither guarantees stability.
- **Lecture 5, multiple right-hand sides:** `O(n^3 + p n^2)`; when a right-hand side depends on the previous solution, use LU factorisation.
- **Lecture 6, LU factorisation:** Gauss elimination directly yields `A = L U` with unit lower triangular `L` (Doolittle); compact storage; the solution is then two triangular substitutions; pivoting is recorded in a pivot vector `p` and applied as `b(p(k))`.
- **Lecture 6, condition number:** the ratio of relative errors; for linear systems `K(A) = ||A|| ||A^-1||` bounds the amplification of relative errors in `b` or `A`; worked ill-conditioned 2 x 2 examples.
- **Lecture 7, slides 4-12, special matrices:** banded and tridiagonal matrices; the Thomas algorithm (`O(n)`); banded storage.
- **Lecture 7, slide ~13, skyline storage:** applicable when no pivoting is needed, "e.g. for banded, symmetric, and positive definite matrices: FEM and FD methods. Skyline solvers are usually based on Cholesky factorization (which preserves the skyline)".
- **Lecture 7, slides 15-16, symmetric positive definite matrices:** Sylvester's criterion; for SPD `A` no pivoting is needed and the elimination is stable; Cholesky factorisation `A = L L^T` (written with the conjugate transpose for complex matrices).
- **Lecture 7, last slides, iterative methods:** for sparse large full-bandwidth systems iterative methods are efficient; an iteration `x(k+1) = B x(k) + c` converges for every start if and only if the spectral radius of `B` is smaller than one.
- **Lecture 8, Jacobi and Gauss-Seidel:** matrix forms of both; both converge if `A` is strictly diagonally dominant; Gauss-Seidel also converges if `A` is symmetric positive definite; a worked tridiagonal example shows where diagonal dominance fails.
- **Lecture 8, SOR:** an extrapolation of each Gauss-Seidel step that aims to reduce the spectral radius of `B`; `omega = 1` is Gauss-Seidel, `1 < omega < 2` over-relaxation, `0 < omega < 1` under-relaxation; converges for `0 < omega < 2` if `A` is symmetric positive definite.
- **Lecture 8, gradient methods:** the earlier schemes are "stationary" (fixed `B`); for SPD matrices the solution minimises a quadratic form; steepest descent reduces it every step but slowly; the conjugate gradient method (Hestenes and Stiefel, 1952) uses search directions that are `A`-conjugate (`v_i^T A v_j = 0` for `i != j`).

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[sparse-linear-systems]], [[direct-solvers-lu-and-cholesky]], [[stationary-iterative-methods]], [[krylov-methods]]
