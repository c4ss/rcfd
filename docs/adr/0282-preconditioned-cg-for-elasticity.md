# ADR 0282: Iterative route for larger models: CG with block Jacobi and incomplete Cholesky on the stiffness matrix

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-12 of the structural solver, basic tier. Size M.

## Goal

The direct solver stops at a few times 1e4 unknowns. Add two preconditioners for the CSR stiffness matrix: a 3 x 3 block Jacobi
and an incomplete Cholesky with zero fill and a diagonal shift when a pivot fails, and make the solver selectable in the static
solve. The default stays the direct solver, bit for bit.

## Dependencies

ADR 0275; existing roadmap items: F-026.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. CG on the stiffness matrix with (a) 3 x 3 block Jacobi and
(b) incomplete Cholesky with zero fill on CSR and a diagonal shift on breakdown, selectable beside the direct solver: on a
cantilever of about 5e3 unknowns (fast) and of at least 1e5 unknowns (heavy record) the relative residual reaches 1e-10; on the
fast case the displacement agrees with the direct solution within 1e-7 relative, incomplete Cholesky needs at most half the
iterations of block Jacobi, and the iteration count and the solution are bit-identical for 1 and 8 threads; a breakdown is
reported with the shift used, never as a non-finite result. The default solver and its results are unchanged bit for bit.

## Reference to check against

The reference is the direct solution. Methods: Meijerink and van der Vorst, Math. Comp. 31 (1977) 148 (incomplete Cholesky);
Manteuffel, Math. Comp. 34 (1980) 473 (the shifted factorisation); both cited from memory.

## Files

`crates/rcfd-linalg/src/precond.rs` (appended types, no re-export in `lib.rs`), `crates/rcfd-solid/src/solve.rs`,
`crates/rcfd-solid/tests/sb_pcg.rs`.

## What could make it miss

The factor of two over block Jacobi is a draft, not a derivation; on slender bodies CG converges slowly whatever the one-level
preconditioner. The real answer (aggregation AMG with the rigid-body modes as near-null space) is left to the intermediate tier.
