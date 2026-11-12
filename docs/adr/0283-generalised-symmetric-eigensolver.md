# ADR 0283: Eigensolver for K x = lambda M x: dense Jacobi and subspace iteration on the direct factorisation

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-13 of the structural solver, basic tier. Size M.

## Goal

`rcfd-linalg` has no eigensolver. Add a dense symmetric one (cyclic Jacobi rotations) and, on top of the factorisation of ADR
0274, subspace iteration for the lowest m eigenpairs of the generalised symmetric problem with a shift (so that a free body,
whose K is singular, can be solved), and a count of the eigenvalues below a value from the signs of `D` so that a missed mode is
detected.

## Dependencies

ADR 0274; existing roadmap items: none.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. rcfd-linalg gains a dense symmetric eigensolver and the
lowest m eigenpairs of K x = lambda M x (K symmetric, M symmetric positive definite) by subspace iteration on the factorisation
of K + sigma M, with a count of the eigenvalues below a value from the signs of D. Checks: the tridiagonal matrix (2, -1) of n =
200 with M = I: the lowest 10 eigenvalues equal 4 sin^2(j pi/(2 (n + 1))) within 1e-10 relative; the 5-point Laplacian on 30 x
30: repeated eigenvalues come with their multiplicity and the vectors are M-orthonormal within 1e-10; the same with a
non-uniform diagonal M against the dense solver within 1e-9; residual |K x - lambda M x| <= 1e-8 |K x| for every returned pair;
a singular K (free chain) with sigma > 0 returns its zero eigenvalue below 1e-9 of the first non-zero one; the count below the
m-th eigenvalue equals m - 1; no convergence within the cap is an error stating the residuals.

## Reference to check against

Closed forms: eigenvalues of the second-difference matrix and of its Kronecker sums (derivable). Methods: Bathe and Wilson, Int.
J. Numer. Meth. Engng 6 (1973) 213 (subspace iteration) and Bathe, Finite Element Procedures, chapter on eigenproblems; Golub
and Van Loan, Matrix Computations (Jacobi); Sylvester's law of inertia for the count. All cited from memory.

## Files

`crates/rcfd-linalg/src/eigen.rs` (new), `crates/rcfd-linalg/src/lib.rs` (one `pub mod` line),
`crates/rcfd-linalg/tests/sb_eigen.rs`.

## What could make it miss

The count needs `L D L^T` of an indefinite shifted matrix without pivoting, which can lose accuracy near an eigenvalue (the
shift must be moved, by a registered rule); slow convergence for clustered modes. It edits `rcfd-linalg/src/lib.rs`, so it
starts after ADR 0274 is merged.
