---
type: source
tags: [reference, foundation, linear-solvers]
sources: []
origin: https://www.netlib.org/templates/templates.pdf
licence: none stated in the PDF; it says it "is the electronic version of the 2nd edition of the Templates book, which is available for purchase from the Society for Industrial and Applied Mathematics". Unknown licence, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Barrett et al. (1994), Templates for the Solution of Linear Systems: Building Blocks for Iterative Methods

**Origin:** R. Barrett, M. Berry, T. F. Chan, J. Demmel, J. M. Donato, J. Dongarra, V. Eijkhout, R. Pozo, C. Romine and H. van der Vorst, *Templates for the Solution of Linear Systems: Building Blocks for Iterative Methods*, 2nd ed., SIAM, Philadelphia, 1994. Electronic version hosted by Netlib (URL above).
**Kind:** reference book (method descriptions with pseudocode). Page numbers below are the book's own, from its table of contents.
**Licence:** unknown (no rights statement in the electronic copy); cite only.
**Read:** 2026-11-22 by job p6-linear-rotating, from a text extraction: ch. 2 (secs. 2.1-2.3.8), ch. 3 (secs. 3.1-3.4.2), sec. 4.3.1, secs. 5.4 and 5.5. Equations lost in the extraction are described in words.

## Summary

The standard practical reference for iterative solvers of sparse linear systems: stationary methods (Jacobi, Gauss-Seidel, SOR, SSOR), Krylov ("nonstationary") methods (CG, MINRES/SYMMLQ, CGNE/CGNR, GMRES, BiCG, QMR, CGS, Bi-CGSTAB, Chebyshev), preconditioners (Jacobi, block Jacobi, SSOR, incomplete factorisations), sparse storage formats, domain decomposition and multigrid.

## Key points

### Ch. 2, iterative methods
- **Sec. 2.1, pp. 5-7, overview:** Jacobi solves for every variable locally with the others fixed, "easy to understand and implement, but convergence is slow"; Gauss-Seidel uses updated values as soon as available and, if Jacobi converges, usually converges faster; SOR adds an extrapolation parameter $\omega$ and with the optimal choice "may converge faster than Gauss-Seidel by an order of magnitude"; SSOR is useful mainly as a preconditioner. CG is "extremely effective" for symmetric positive definite matrices; GMRES stores the whole orthogonal sequence, so restarted versions are used; BiCG uses two mutually ("bi-") orthogonal sequences with $A$ and $A^T$; Bi-CGSTAB is a variant of BiCG with smoother convergence than CGS.
- **Sec. 2.2, p. 7:** a method of the form $x^{(k)} = B x^{(k-1)} + c$ with $B$ and $c$ independent of $k$ is "stationary".
- **Sec. 2.2.1, pp. 7-8, Jacobi:** each equation solved for its own unknown with the others from the previous iterate, eq. (2.3); in matrix form with $A = D - L - U$ (diagonal, strictly lower, strictly upper parts). For the 1-D model problem $-u_{xx} = f$ the Jacobi iteration damps high-frequency eigenmodes quickly while low-frequency modes have damping factor near 1; the spectral radius is about $1 - 10/N^2$. For Jacobi and Gauss-Seidel the spectral radius is $1 - O(h^2)$.
- **Sec. 2.2.2, pp. 8-9, Gauss-Seidel:** eq. (2.5); the result depends on the order of the equations; reordering trades parallelism against convergence rate.
- **Sec. 2.2.3, pp. 9-11, SOR:** $x^{(k)} = (D - \omega L)^{-1} (\omega U + (1 - \omega) D)\, x^{(k-1)} + \omega (D - \omega L)^{-1} b$; $\omega = 1$ is Gauss-Seidel; Kahan's theorem: SOR fails to converge for $\omega$ outside $(0, 2)$; for symmetric positive definite $A$ it converges for any $\omega$ in $(0, 2)$; the optimal $\omega$ is usually too expensive to compute, a heuristic is $\omega = 2 - O(h)$.
- **Sec. 2.3.1, pp. 12-15, CG:** iterates, residuals and search directions updated by two coupled two-term recurrences, eqs. (2.10)-(2.11), with $\alpha_i = r^T r / p^T A p$ and $\beta_i = r_i^T r_i / r_{i-1}^T r_{i-1}$; one matrix-vector product, three vector updates and two inner products per iteration. Unpreconditioned CG builds $x^{(i)}$ in $x^{(0)} + \operatorname{span}\{r^{(0)}, \ldots, A^{i-1} r^{(0)}\}$ minimising the $A$-norm of the error; the minimum exists in general only for symmetric positive definite $A$, and the preconditioner $M$ must also be symmetric positive definite. The error bound uses $\kappa_2(M^{-1} A) = \lambda_\text{max} / \lambda_\text{min}$ with factor $\alpha = (\sqrt{\kappa} - 1)/(\sqrt{\kappa} + 1)$. Second-order elliptic PDEs give $\kappa_2(A) = O(h^{-2})$, so unpreconditioned CG needs a number of iterations proportional to $h^{-1}$. CG is closely related to the Lanczos method (sec. 5.1).
- **Sec. 2.3.3, p. 16:** CG on the normal equations converges at a rate set by the square of the condition number.
- **Sec. 2.3.4, pp. 17-19, GMRES:** builds an orthonormal Krylov basis by modified Gram-Schmidt (the Arnoldi method), stores the coefficients in an upper Hessenberg matrix and chooses the combination that minimises $\lVert b - A x \rVert$; work and storage rise linearly with the iteration count, so GMRES(m) restarts every $m$ steps; there are no definite rules for $m$, and some problems stagnate for any $m < n$.
- **Sec. 2.3.5, pp. 19-20, BiCG:** CG's residuals cannot be made orthogonal by short recurrences for non-symmetric matrices; BiCG replaces them by two mutually orthogonal sequences (with $A$ and $A^T$) "at the price of no longer providing a minimization"; convergence may be irregular and the method can break down.
- **Sec. 2.3.8, pp. 24-25, Bi-CGSTAB** (van der Vorst): computes $Q_i(A) P_i(A)\, r^{(0)}$ with $Q_i$ a steepest-descent polynomial; can be read as BiCG combined with repeated GMRES(1); smoother convergence; an extra breakdown if the local GMRES(1) step stagnates; two matrix-vector products and four inner products per iteration.
- **Sec. 2.3.9, pp. 25-26, Chebyshev iteration:** needs no inner products but needs bounds on the spectrum; for symmetric positive definite problems the bounds are $\lambda_\text{min}$ and $\lambda_\text{max}$ of $M^{-1} A$; underestimating $\lambda_\text{max}$ may diverge.

### Ch. 3, preconditioners
- **Sec. 3.1, p. 35:** a preconditioner $M$ transforms the system so that $M^{-1} A$ has more favourable spectral properties; there is a trade-off between construction and application cost and the gain in iterations; most preconditioners improve the iteration count only by a constant, multigrid-based and modified incomplete factorisations do better.
- **Sec. 3.1.2, p. 36:** left and right preconditioning; for CG, $M^{-1} A$ is not symmetric, so the method uses the $M^{-1}$ inner product instead.
- **Sec. 3.2, p. 37:** point Jacobi preconditioner $M = \operatorname{diag}(A)$; block Jacobi from a partition of the unknowns (per node, per line or plane, per processor); little storage, no parallel difficulties, "more sophisticated preconditioners usually yield a larger improvement".
- **Sec. 3.3, pp. 37-38:** SSOR preconditioner $M = (D + L) D^{-1} (D + L)^T$ in the symmetric case.
- **Sec. 3.4, pp. 38-44:** incomplete factorisation $M = L U$ that ignores fill; may break down (zero pivot) or be indefinite even when the full factorisation exists; guaranteed to exist for M-matrices (Meijerink and van der Vorst); construction costs at least one solve. ILU(0) keeps the sparsity of $A$; D-ILU alters only the diagonal (p. 39-40); on central-difference Cartesian matrices ILU(0) and D-ILU coincide.

### Ch. 4-5
- **Sec. 4.3.1, pp. 57-58, storage:** Compressed Row Storage keeps `val`, `col_ind`, `row_ptr`, $2\, nnz + n + 1$ storage locations instead of $n^2$; it assumes nothing about the sparsity pattern.
- **Sec. 5.4.1, pp. 76-77, overlapping (additive) Schwarz:** the preconditioner sums subdomain solves (restriction, local solve, extension) plus a coarse-grid solve; with sufficient overlap the condition number is bounded independently of $H$ and $h$ (Dryja and Widlund); without the coarse term it grows as $O(1/H^2)$.
- **Sec. 5.5, pp. 80-81, multigrid:** simple iterative methods damp high-frequency error fastest; smooth, restrict to a coarse grid and solve, interpolate back and smooth again; recursive application gives a V-cycle, visiting the coarse grid twice a W-cycle; the work of many multigrid methods is proportional to the number of unknowns; multigrid-like methods also serve as preconditioners.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[sparse-linear-systems]], [[stationary-iterative-methods]], [[krylov-methods]], [[preconditioning]], [[geometric-multigrid]]
