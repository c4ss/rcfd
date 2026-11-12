# ADR 0334: Sparse L D L^T with a fill-reducing ordering, and its use by the static, buckling and modal solves

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-24 of the structural solver, advanced tier. Size M.

## Goal

The envelope solver of ADR 0274 stops at a few times 1e4 unknowns in three dimensions. Add to `rcfd-linalg` a minimum degree
ordering on the quotient graph, the elimination tree, a symbolic and a numeric L D L^T with the count of negative pivots
(inertia), and solves for many right-hand sides; selectable in the static solve and accepted by the eigensolvers as the
factorisation. The default solver is unchanged.

## Dependencies

ADR 0311; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): SA-1, F-373, F-374, F-382, F-402.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Five-point grid 100 x 100 and seven-point grid 30^3: residual
below 1e-12 relative, and on the 30^3 grid at most half the factor entries of the envelope after reverse Cuthill-McKee (both
counts recorded). On a solid model the solution equals the envelope solver's within 1e-10 and the modal frequencies through either
factorisation agree within 1e-9. Inertia: for the tridiagonal (2, -1) of order 200 and three shifts the number of negative pivots
of K - s I equals the number of eigenvalues 4 sin^2(j pi/(2 (n + 1))) below s. A matrix that is not positive definite is reported
with the equation when positive definiteness was requested. Factor and solution are bit-identical for 1 and 8 threads. Heavy
record: a clamped cube of about 3e5 unknowns is solved, time and memory recorded beside the envelope's estimate. The default
solver's results keep their bits.

## Reference to check against

The residual, the envelope solver and Sylvester's law of inertia are the references. Methods: George and Liu, Computer Solution of
Large Sparse Positive Definite Systems (minimum degree, elimination tree); Liu, ACM Trans. Math. Softw. 11 (1985) 141; Davis,
Direct Methods for Sparse Linear Systems (from memory; the algorithm is checked by the residual and the fill count).

## Files

`crates/rcfd-linalg/src/sparse_chol.rs`, `crates/rcfd-fem/src/solve.rs` (the choice),
`crates/rcfd-linalg/tests/sa_sparse_chol.rs`, `crates/rcfd-fem/tests/sa_sparse_solve.rs`.

## What could make it miss

A plain minimum degree without supernodes is slow beyond about 1e6 unknowns and its fill is worse than nested dissection: the
factor-of-two clause is a draft. If the eigensolver of ADR 0283 is written against the envelope type, one function is appended to
`eigen.rs` (then this package must not run beside another that edits it).
