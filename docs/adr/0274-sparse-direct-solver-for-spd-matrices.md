# ADR 0274: Sparse direct solver for symmetric positive definite matrices (envelope L D L^T after reverse Cuthill-McKee)

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-4 of the structural solver, basic tier. Size M.

## Goal

`rcfd-linalg` has CG, Jacobi, an incomplete Cholesky tied to the finite-volume LDU matrix, and a scalar aggregation AMG without
rigid-body modes: nothing that solves an elasticity matrix robustly. Add a direct factorisation of a symmetric positive definite
`CsrMatrix`, `L D L^T` on the envelope after the existing `rcm_order`, kept for repeated right-hand sides (the modal solver of
ADR 0283 needs that), and reporting the signs of `D`.

## Dependencies

none in this list; existing roadmap items: F-025, F-026.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. rcfd-linalg gains a direct factorisation of a symmetric
positive definite CsrMatrix, reusable for many right-hand sides: on the 7-point Laplacian of 12^3 unknowns, on a randomly
permuted copy (registered seed) and on a matrix with three unknowns per node built from it (Kronecker product with a 3 x 3 SPD
block) the relative residual is <= 1e-12 and the solution agrees with cg (tolerance 1e-13) within 1e-9; the permuted matrix
factorises with an envelope no larger than 1.2 times that of the unpermuted one; a zero or negative pivot, an unsymmetric matrix
and a non-finite entry are refused with the row; the result is bit-identical for 1 and 8 threads; a heavy record factorises 30^3
unknowns and reports time and memory. No existing function of the crate changes.

## Reference to check against

Exact solve: the reference is the residual and the existing `cg`. Method: George and Liu, Computer Solution of Large Sparse
Positive Definite Systems (1981), the envelope method (cited from memory).

## Files

`crates/rcfd-linalg/src/skyline.rs` (new), `crates/rcfd-linalg/src/lib.rs` (one `pub mod` line),
`crates/rcfd-linalg/tests/sb_direct.rs`.

## What could make it miss

Memory: the envelope of a 3-D problem grows like n^(5/3), so the solver is practical to a few times 1e4 unknowns (estimate:
2.8e4 unknowns of a 20^3 hexahedron box need about 0.3 GB). That bounds the mesh sizes of ADRs 0279, 0280 and 0288; ADR 0282 is
the route beyond it.
