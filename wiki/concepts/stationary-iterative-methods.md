---
type: concept
tags: [foundation, linear-solvers]
sources: [ref-barrett-1994-templates-linear-systems, ref-mit-ocw-2-29-linear-systems, ref-shewchuk-1994-painless-cg, ref-falgout-2006-amg-intro]
builds_on: [sparse-linear-systems]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Stationary iterative methods (Jacobi, Gauss-Seidel, SOR)

## In short

Instead of eliminating, guess $x$ and repeatedly improve it: solve each equation for its own unknown using the current values of the others. The same update rule is used every sweep, hence "stationary". These methods are cheap and simple but slow; their real job in modern solvers is to smooth the error inside multigrid and to act as preconditioners.

## Explanation

- **Form.** A stationary method is $x^{(k)} = B x^{(k-1)} + c$ with $B$ and $c$ independent of $k$ ([[ref-barrett-1994-templates-linear-systems]], sec. 2.2). It converges for every starting vector if and only if the spectral radius of $B$ (its largest eigenvalue magnitude) is below one ([[ref-mit-ocw-2-29-linear-systems]], lecture 7; [[ref-shewchuk-1994-painless-cg]], sec. 5.2).
- **Splitting.** Write $A = D - L - U$ (diagonal, strictly lower, strictly upper parts) ([[ref-barrett-1994-templates-linear-systems]], sec. 2.2.1). Different splittings give different methods ([[ref-shewchuk-1994-painless-cg]], sec. 5.2, eqs. 14-15).
- **Jacobi.** Each equation $i$ is solved for $x_i$ with all other unknowns from the previous iterate: $x^{(k)} = D^{-1} (L + U)\, x^{(k-1)} + D^{-1} b$. The order of equations does not matter, so all updates can be done at once ("simultaneous displacements"), but a second vector is needed ([[ref-barrett-1994-templates-linear-systems]], sec. 2.2.1).
- **Gauss-Seidel.** Use each new value as soon as it is computed: $x^{(k)} = (D - L)^{-1} (U x^{(k-1)} + b)$. The result depends on the ordering, which trades parallelism against convergence; if Jacobi converges, Gauss-Seidel usually converges faster ([[ref-barrett-1994-templates-linear-systems]], secs. 2.1, 2.2.2).
- **SOR.** Extrapolate each Gauss-Seidel update by a factor $\omega$: $x^{(k)} = (D - \omega L)^{-1} (\omega U + (1 - \omega) D)\, x^{(k-1)} + \omega (D - \omega L)^{-1} b$. $\omega = 1$ is Gauss-Seidel; SOR fails for $\omega$ outside $(0, 2)$ (Kahan); for SPD $A$ it converges for every $\omega$ in $(0, 2)$; with the optimal $\omega$ it may be an order of magnitude faster than Gauss-Seidel, but the optimum is rarely computable and $\omega = 2 - O(h)$ is a common heuristic ([[ref-barrett-1994-templates-linear-systems]], secs. 2.1, 2.2.3). $0 < \omega < 1$ is under-relaxation ([[ref-mit-ocw-2-29-linear-systems]], lecture 8).
- **Convergence conditions.** Jacobi and Gauss-Seidel both converge if $A$ is strictly diagonally dominant; Gauss-Seidel also converges if $A$ is SPD ([[ref-mit-ocw-2-29-linear-systems]], lecture 8).
- **Why they are slow, and why that is useful.** For the 1-D model problem the Jacobi iteration damps high-frequency error modes quickly while the damping factor of low-frequency modes is close to 1; the spectral radius is about $1 - 10/N^2$, and for Jacobi and Gauss-Seidel in general $1 - O(h^2)$, so refining the mesh slows them down ([[ref-barrett-1994-templates-linear-systems]], sec. 2.2.1). Shewchuk calls Jacobi a "smoother" because every eigenvector component of the error shrinks every iteration ([[ref-shewchuk-1994-painless-cg]], sec. 6.2). This smoothing property is exactly what multigrid uses: a few sweeps remove the oscillatory error and a coarse grid handles the smooth rest ([[ref-falgout-2006-amg-intro]], introduction; [[ref-barrett-1994-templates-linear-systems]], sec. 5.5).

## Builds on

- [[sparse-linear-systems]]: diagonal dominance, SPD-ness and the spectral radius of the iteration matrix decide convergence.

## Leads to

- [[geometric-multigrid]] (smoothers), [[preconditioning]] (Jacobi, block Jacobi, SSOR preconditioners), [[under-relaxation]]
- [[amg]], [[fixed-point-iteration-and-acceleration]] (the same "repeat a map until it stops changing" idea, for nonlinear coupling)

## How rcfd uses it

- **Jacobi preconditioning** is the default preconditioner of the GPU CG path (`linear_solver.preconditioner = "jacobi"`) ([[doc-case-schema-linear-solver]], [[doc-gpu]]).
- **Block Gauss-Seidel smoothing:** the structural elasticity multigrid uses two block Gauss-Seidel sweeps per side on quadratic elements (F-477, ADR 0375) ([[adr-thread-structural-linear-solvers]]).

## Related

- [[krylov-methods]] — the nonstationary methods these sweeps precondition (Jacobi on the GPU CG path).
