---
type: source
tags: [reference, foundation, linear-solvers, krylov]
sources: []
origin: https://www.cs.cmu.edu/~quake-papers/painless-conjugate-gradient.pdf
licence: "(c) 1994 by Jonathan Richard Shewchuk. This article may be freely duplicated and distributed so long as no consideration is received in return, and this copyright notice remains intact." (notice on p. 1, read 2026-11-22). Non-commercial condition, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Shewchuk (1994), An Introduction to the Conjugate Gradient Method Without the Agonizing Pain

**Origin:** J. R. Shewchuk, "An Introduction to the Conjugate Gradient Method Without the Agonizing Pain", Edition 1 1/4, School of Computer Science, Carnegie Mellon University, August 4, 1994 (technical report CMU-CS-94-125). URL above.
**Kind:** tutorial report, 58 pages, 66 figures.
**Licence:** the copyright notice above permits free duplication only when "no consideration is received in return"; that is a non-commercial condition, so no copy goes into `raw/`.
**Read:** 2026-11-22 by job p6-linear-rotating, from a text extraction of the PDF. The prose and section structure were readable; most mathematical symbols were lost in the extraction, so equations below are cited by number and described in words, not transcribed.

## Summary

A geometric introduction to the conjugate gradient (CG) method: the linear system is recast as the minimisation of a quadratic form, steepest descent and the Jacobi method are analysed with eigenvectors, conjugate directions are built by Gram-Schmidt conjugation, and CG follows as conjugate directions built from the residuals. Convergence is bounded with Chebyshev polynomials in terms of the condition number. Preconditioning, CG on the normal equations and nonlinear CG close the report.

## Key points

- **Sec. 3, eq. (3), p. 2:** the quadratic form $f(x) = \tfrac{1}{2} x^T A x - b^T x + c$; if $A$ is symmetric and positive-definite, $f$ is minimised by the solution of $A x = b$.
- **Sec. 4, pp. 6-9:** steepest descent steps along the residual, which is the negative gradient of $f$.
- **Sec. 5.2, eqs. (14)-(15), pp. 11-12:** the Jacobi method splits $A$ into its diagonal $D$ and off-diagonal part $E$ and iterates with $D^{-1}$; another splitting gives Gauss-Seidel or SOR. Convergence of the iteration depends on the spectral radius of the iteration matrix, which should be small.
- **Sec. 6.2, p. 17 (text near fig. 17-20):** a large condition number gives a long narrow trough in $f$, along which steepest descent zig-zags; convergence per iteration worsens as the condition number grows (fig. 20). The same section notes that the Jacobi method is a "smoother" (every eigenvector component of the error is reduced every iteration), while steepest descent and CG are "roughers".
- **Sec. 7.1-7.3, pp. 21-30:** search directions that are $A$-orthogonal ("conjugate"); Gram-Schmidt conjugation; the error term is optimal in the energy norm over the space searched.
- **Sec. 8, p. 30:** CG is the method of conjugate directions with search directions built from the residuals; the residual is orthogonal to the previous search directions, so each new direction is new unless the residual is zero. The space searched is a Krylov subspace (sec. 9.1).
- **Sec. 9, pp. 32-36:** in exact arithmetic CG finishes in at most $n$ steps, but round-off makes the search vectors lose $A$-orthogonality; CG was dropped in the 1960s and came back as an iterative method in the 1970s. Eq. (52) bounds the energy-norm error after $i$ steps by a factor $2\left(\dfrac{\sqrt{\kappa} - 1}{\sqrt{\kappa} + 1}\right)^i$; in practice CG usually beats the bound because of good eigenvalue distributions or starting points.
- **Sec. 10, p. 37:** the dominant work per iteration is the matrix-vector product, whose cost is proportional to the number of non-zero entries, so sparse matrices make each iteration cheap; the section compares the maximum iteration counts of steepest descent and CG for reducing the error by a given factor (fig. 35: iterations of steepest descent needed to match one of CG). The exact complexity expressions were lost in the text extraction.
- **Sec. 12, pp. 39-41:** preconditioning solves $M^{-1} A x = M^{-1} b$ with $M$ symmetric positive-definite, approximating $A$ and easy to invert; its effectiveness is set by the condition number (and eigenvalue clustering) of $M^{-1} A$. Diagonal (Jacobi) preconditioning scales the quadratic form along the coordinate axes ("often only a mediocre preconditioner"); incomplete Cholesky drops fill and "is not always stable"; "for large-scale applications, CG should nearly always be used with a preconditioner".
- **Sec. 13, p. 41:** CG on the normal equations $A^T A x = A^T b$ handles non-symmetric or non-square $A$, but the condition number is squared, so convergence is "significantly slower".
- **Appendix B2-B3, pp. 50-51:** pseudocode of CG and preconditioned CG.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[sparse-linear-systems]], [[stationary-iterative-methods]], [[krylov-methods]], [[preconditioning]]
