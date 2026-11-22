---
type: concept
tags: [foundation, linear-solvers, krylov]
sources: [ref-shewchuk-1994-painless-cg, ref-barrett-1994-templates-linear-systems, ref-mit-ocw-2-29-linear-systems]
builds_on: [sparse-linear-systems]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Krylov methods (CG, GMRES, BiCGStab)

## In short

Krylov methods build the solution out of the vectors `r0, A r0, A^2 r0, ...` (the Krylov subspace) and pick, at each step, the best combination by some optimality rule. Conjugate gradients (CG) does this for symmetric positive definite matrices with almost no memory; GMRES and Bi-CGSTAB handle non-symmetric matrices at the price of more memory or less optimality.

## Explanation

- **Nonstationary.** Unlike Jacobi or Gauss-Seidel, the update coefficients change every iteration; they are computed from inner products of residuals and other vectors ([[ref-barrett-1994-templates-linear-systems]], sec. 2.3).
- **CG as minimisation.** For SPD `A`, `A x = b` minimises `f(x) = 1/2 x^T A x - b^T x + c` ([[ref-shewchuk-1994-painless-cg]], sec. 3, eq. 3). Steepest descent walks down the gradient (the residual) but zig-zags in the narrow valleys of an ill-conditioned `f` ([[ref-shewchuk-1994-painless-cg]], secs. 4, 6.2). CG instead uses search directions that are `A`-conjugate, `p_i^T A p_j = 0` for `i != j` (Hestenes and Stiefel, 1952) ([[ref-mit-ocw-2-29-linear-systems]], lecture 8), built from the residuals, so that each step's progress is never undone ([[ref-shewchuk-1994-painless-cg]], secs. 7-8).
- **CG recurrences.** With `r = b - A x`: `alpha_i = r(i-1)^T r(i-1) / p(i)^T A p(i)`, `x(i) = x(i-1) + alpha_i p(i)`, `r(i) = r(i-1) - alpha_i A p(i)`, `beta_i = r(i)^T r(i) / r(i-1)^T r(i-1)`, `p(i+1) = r(i) + beta_i p(i)`; one matrix-vector product, three vector updates and two inner products per iteration ([[ref-barrett-1994-templates-linear-systems]], sec. 2.3.1, eqs. 2.10-2.11, fig. 2.5). The iterate `x(i)` minimises the `A`-norm of the error over `x(0) + span{r(0), ..., A^(i-1) r(0)}`; the minimum is guaranteed only for SPD `A` (same section).
- **CG convergence.** In exact arithmetic CG ends in at most `n` steps, but round-off destroys conjugacy, so it is used as an iterative method ([[ref-shewchuk-1994-painless-cg]], sec. 9). The energy-norm error falls at least by the factor `2 ((sqrt(kappa) - 1)/(sqrt(kappa) + 1))^i` after `i` steps, `kappa` the condition number ([[ref-shewchuk-1994-painless-cg]], eq. 52; [[ref-barrett-1994-templates-linear-systems]], sec. 2.3.1). For elliptic PDEs `kappa = O(h^-2)`, so iterations grow like `h^-1` without preconditioning (same section). CG is often faster than the bound (superlinear convergence once extreme eigenvalues are resolved) ([[ref-barrett-1994-templates-linear-systems]], sec. 2.3.1). For indefinite symmetric matrices CG can break down; MINRES and SYMMLQ are the variants for that case (sec. 2.3.2).
- **Normal equations.** CG on `A^T A x = A^T b` works for any non-singular `A` but squares the condition number and converges slowly ([[ref-shewchuk-1994-painless-cg]], sec. 13; [[ref-barrett-1994-templates-linear-systems]], sec. 2.3.3).
- **GMRES.** For non-symmetric `A` the residuals cannot be kept orthogonal with short recurrences. GMRES builds an orthonormal Krylov basis by modified Gram-Schmidt (the Arnoldi method), stores it, and picks the combination that minimises `||b - A x||_2` through a small least-squares problem with an upper Hessenberg matrix. Work and storage grow with each iteration, so GMRES(m) restarts every `m` steps; there is no rule for `m`, and some problems stagnate for any `m < n` ([[ref-barrett-1994-templates-linear-systems]], sec. 2.3.4).
- **BiCG and Bi-CGSTAB.** BiCG keeps short recurrences by using two mutually orthogonal sequences (with `A` and `A^T`), "at the price of no longer providing a minimization"; convergence may be irregular and it can break down (sec. 2.3.5). Bi-CGSTAB (van der Vorst) combines BiCG with a local steepest-descent (GMRES(1)) step for smoother convergence; it needs no `A^T`, costs two matrix-vector products and four inner products per iteration, and has an extra breakdown when the local step stagnates (sec. 2.3.8).
- **Chebyshev iteration.** Needs no inner products, only bounds on the spectrum; for SPD problems `lambda_min` and `lambda_max` of `M^-1 A`; underestimating `lambda_max` may diverge (sec. 2.3.9). This is why Chebyshev is attractive where inner products are a synchronisation bottleneck.

## Builds on

- [[sparse-linear-systems]]: symmetry and positive definiteness select the method; the condition number sets the speed; the cost is dominated by sparse matrix-vector products.

## Leads to

- [[preconditioning]] (every practical Krylov solve is preconditioned), [[amg]] (AMG-preconditioned CG), [[eigenvalue-solvers]] (Lanczos is the eigenvalue side of CG; [[ref-barrett-1994-templates-linear-systems]], sec. 2.3.1)
- [[simple]] (GPU CG/BiCGStab pressure solves)

## How rcfd uses it

- **Pressure solves:** the VOF and multiphase pressure solve is "CG with smoothed-aggregation AMG above 2000 cells" ([[amg]], from [[doc-case-schema-multiphase]]).
- **GPU:** CG and BiCGStab run entirely on the device for `simple`, `piso`, `pimple`; GMRES is not on the device ([[doc-gpu]]). `linear_solver.device = "cuda"` runs preconditioned CG on an NVIDIA device ([[doc-case-schema-linear-solver]]).
- **Structural solver:** CG with incomplete Cholesky, block Jacobi or AMG preconditioners (F-381, F-460, F-470) ([[adr-thread-structural-linear-solvers]]).
- **An open guard:** an `rz <= 0` guard inside `cg` would change default solves and is the maintainer's decision (ADR 0369 Q4, ADR 0363 Q6) ([[amg]]).
