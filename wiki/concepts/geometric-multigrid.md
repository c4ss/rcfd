---
type: concept
tags: [foundation, linear-solvers, multigrid]
sources: [ref-falgout-2006-amg-intro, ref-barrett-1994-templates-linear-systems, ref-shewchuk-1994-painless-cg]
builds_on: [stationary-iterative-methods, direct-solvers-lu-and-cholesky]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Geometric multigrid

## In short

Simple relaxation sweeps quickly remove error that oscillates from cell to cell but barely touch error that varies smoothly over the domain. Multigrid moves that smooth error to a coarser grid, where it looks oscillatory again and is cheap to remove, and does this recursively. The result solves a system of `N` unknowns in work proportional to `N`.

## Explanation

- **Two complementary processes.** Smoothing applies a relaxation method such as Gauss-Seidel, which reduces high-frequency error; coarse-grid correction transfers the remaining (smooth, low-frequency) error to a coarser grid by restriction, solves there, and transfers the correction back by interpolation (prolongation) ([[ref-falgout-2006-amg-intro]], introduction). The smoothing property comes from the stationary methods: Jacobi damps high-frequency modes fast and low-frequency ones hardly at all ([[ref-barrett-1994-templates-linear-systems]], sec. 2.2.1; [[ref-shewchuk-1994-painless-cg]], sec. 6.2).
- **Two-grid cycle.** With interpolation `P` (coarse to fine) and restriction `P^T`: (a) smooth `nu_1` times on `A u = f`; (b) compute the residual `r = f - A u = A e`; (c) solve the coarse system `A_c e_c = P^T r`; (d) correct `u <- u + P e_c`; (e) smooth `nu_2` times ([[ref-falgout-2006-amg-intro]], eqs. 2a-2e). The coarse operator is usually the Galerkin product `A_c = P^T A P`, which minimises the error after correction in the energy norm (same section). In geometric multigrid the coarse grid doubles the spacing (a quarter of the points in 2-D) and `P` is geometric interpolation ([[ref-barrett-1994-templates-linear-systems]], sec. 5.5).
- **V- and W-cycles.** Applying the cycle recursively to step (c) descends through coarser and coarser grids and ascends back: a V-cycle; visiting the coarse grid twice gives a W-cycle. Recursion stops when the coarsest system is small enough to solve directly ([[ref-barrett-1994-templates-linear-systems]], sec. 5.5).
- **Optimal cost.** Many multigrid methods have work proportional to the number of unknowns ([[ref-barrett-1994-templates-linear-systems]], sec. 5.5); multigrid is "scalable or optimal" with `O(N)` work, and its convergence factors are nearly independent of problem size while the number of levels grows logarithmically ([[ref-falgout-2006-amg-intro]], introduction and table 1).
- **Where geometry fails.** Linear interpolation is wrong for diffusion with large jumps in the coefficient; operator-dependent interpolation, built from the matrix coefficients, fixes it, and AMG takes that idea to the extreme by ignoring geometry altogether ([[ref-falgout-2006-amg-intro]], sidebar). Anisotropic problems need coarsening only in the direction where the error is smooth (semicoarsening) (same article, fig. 2).
- **As a preconditioner.** Multigrid-like methods can precondition Krylov solvers ([[ref-barrett-1994-templates-linear-systems]], sec. 5.5; [[ref-falgout-2006-amg-intro]]).

## Builds on

- [[stationary-iterative-methods]]: the smoothers (Jacobi, Gauss-Seidel) and their smoothing property.
- [[direct-solvers-lu-and-cholesky]]: the coarsest level is solved directly.

## Leads to

- [[amg]] (multigrid built from the matrix alone), [[preconditioning]]

## How rcfd uses it

- rcfd's scalar multigrid is algebraic (smoothed aggregation), not geometric; see [[amg]] ([[adr-thread-amg-coarse-lu]]).
- **Structural multigrid:** CG with one V-cycle on quadratic elements needs 31 iterations against the limit 30 (default 79), first coarse level 1/10.14 of the fine one, operator complexity 1.476 (F-470, ADR 0370); two block Gauss-Seidel sweeps per side (F-477, ADR 0375) ([[adr-thread-structural-linear-solvers]]).
