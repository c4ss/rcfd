---
type: source
tags: [reference, foundation, linear-solvers, multigrid, amg]
sources: []
origin: https://www.osti.gov/biblio/897960
licence: no rights statement on the OSTI record or in the preprint (DOE-sponsored LLNL report with the standard US Government / University of California disclaimer). Unknown, so cite only.
read: 2026-11-22
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Falgout (2006), An Introduction to Algebraic Multigrid

**Origin:** R. D. Falgout, "An Introduction to Algebraic Multigrid", *Computing in Science and Engineering* 8(6) (2006) 24-33; preprint UCRL-JRNL-220851, Lawrence Livermore National Laboratory, OSTI 897960. Full text: https://www.osti.gov/servlets/purl/897960.
**Kind:** review article (preprint, 13 pages of text and figures; locators are the article's headed sections and figure/equation numbers).
**Licence:** unknown; cite only.
**Read:** 2026-11-22 by job p6-linear-rotating, full preprint.

## Summary

An introduction to algebraic multigrid (AMG): the two multigrid ingredients (smoothing and coarse-grid correction), how AMG builds coarse "grids", interpolation and coarse operators from the matrix alone, the classical Ruge-Stueben algorithm, smoothed aggregation, adaptive AMG and recent two-grid theory.

## Key points

- **Introduction:** multigrid solves a system with `N` unknowns in `O(N)` work. Smoothing (a relaxation such as Gauss-Seidel) reduces oscillatory high-frequency error; coarse-grid correction (restriction, a coarse solve, interpolation/prolongation back) removes smooth low-frequency error. Geometric techniques fall short for some problem classes (sidebar on operator-dependent interpolation for diffusion with jumping coefficients).
- **AMG basics, eqs. (1)-(2):** for `A u = f` with `A` symmetric positive definite, interpolation is an `n x n_c` matrix `P`, restriction is `P^T`. The two-grid method (2a)-(2e): smooth, compute `r = f - A u = A e`, solve `A_c e_c = P^T r`, correct `u <- u + P e_c`, smooth again; applied recursively for a hierarchy. The coarse operator is usually the Galerkin product `A_c = P^T A P`, which minimises the error after correction in the energy norm. The "grid" is the set of vertices of the matrix adjacency graph (fig. 1).
- **Algebraic smoothness:** error the smoother leaves is "smooth error"; in AMG it may be geometrically oscillatory ("algebraically smooth"); smooth error corresponds to eigenvectors with small eigenvalues; the smallest modes are the "near null space" (fig. 2, problem (3)). AMG coarsens in directions of geometric smoothness (full coarsening or semicoarsening).
- **Classical AMG, eqs. (4)-(5):** small eigenmodes satisfy `e^T A e ~ 0`, which with zero row sums gives the heuristic "smooth error varies slowly in the direction of relatively large (negative) coefficients". Strength of connection: `u_i` strongly depends on `u_j` if `-a_ij >= theta max_(k != i) {-a_ik}` with `0 < theta <= 1`. Coarse grid: strength matrix, a first pass choosing an independent set (fig. 3), C-points and F-points; interpolation by "collapsing the stencil" (figs. 4-5). Table 1: convergence factors nearly independent of problem size, setup and solve time linear, levels growing logarithmically; grid and operator complexity defined.
- **AMG as preconditioner:** "in practice, it is usually better to use AMG as a preconditioner for a Krylov method such as conjugate gradients"; the AMG cycle must then be symmetric (C-F Jacobi in the example; AMG-CG takes 8 or 9 iterations for all sizes).
- **Smoothed aggregation (SA), section on other AMG methods:** interpolation is seen as columns, basis functions `p_j`; SA partitions the grid into small disjoint aggregates, builds a tentative prolongator whose column `j` holds the near-null-space vector (e.g. the constant) on aggregate `j`, then applies a smoother to the tentative prolongator to get the final interpolation. SA needs the near-null-space components explicitly (refs. [11, 12], Vanek 1992; Vanek, Mandel and Brezina 1996).
- **Theory:** the weak approximation property ties interpolation accuracy to the spectrum (small eigenmodes must be interpolated well); compatible relaxation measures coarse-grid quality.

## Contradictions with the wiki

_None found at ingest._ The wiki's [[amg]] page records that rcfd's smoothed aggregation follows Vanek, Mandel and Brezina 1996 with "structure recalled, not read" ([[adr-thread-amg-coarse-lu]]); this source describes the same structure (aggregates, tentative prolongator, smoothing) at overview level and cites that paper, but is not the paper itself.

## Pages this source informs

- [[geometric-multigrid]], [[amg]], [[preconditioning]]
