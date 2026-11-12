# ADR 0305: Aggregation multigrid for elasticity: rigid-body modes as near-null space, as a preconditioner of CG

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-15 of the structural solver, intermediate tier. Size M.

## Goal

The one-level preconditioners of ADR 0282 lose on slender bodies and large models. Add, beside the scalar AMG and without touching
it, a smoothed-aggregation multigrid for matrices with three unknowns per node: aggregation on the node graph, a tentative
prolongator that carries the six rigid-body modes of each aggregate (from the node coordinates, by a local QR), a damped-Jacobi
smoothed prolongator, a block smoother, a dense coarsest solve; used as the preconditioner of CG and selectable in the static
solve. The default solver is unchanged.

## Dependencies

none in this list; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): F-026, F-381.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. On every level the rigid-body modes are in the range of the
prolongator: |B - P B_c| <= 1e-12 |B|. CG preconditioned by one V-cycle reaches a relative residual of 1e-10 on a clamped cube of
Hex8 with about 5e3, 4e4 and (heavy record) 3e5 unknowns in a number of iterations that grows by at most 1.5 times from the
smallest to the largest and stays below 40; on a cantilever of slenderness 20 with about 1e5 unknowns it needs at most one fifth
of the iterations of the incomplete Cholesky of F-381; the same on a Tet10 mesh within twice the Hex8 count. The solution agrees
with the direct solver within 1e-7 on the smallest case; operator complexity <= 1.5; iteration counts and solution are
bit-identical for 1 and 8 threads. The scalar AMG and its tests are unchanged; the default solver's results are unchanged bit for
bit.

## Reference to check against

The reference is the direct solution and the iteration counts themselves. Method, from memory: Vanek, Mandel and Brezina,
Computing 56 (1996) 179 (smoothed aggregation with a near-null space); the rigid-body modes of a node set are translations and x
cross e_i (first principles).

## Files

`crates/rcfd-linalg/src/amg_elastic.rs` (new), `crates/rcfd-linalg/src/lib.rs` (one `pub mod` line),
`crates/rcfd-solid/src/solve.rs` (the solver choice and the modes from the coordinates),
`crates/rcfd-linalg/tests/si_amg_elastic.rs`, `crates/rcfd-solid/tests/si_amg.rs`.

## What could make it miss

Every iteration bound is a draft; none is derived. Constrained unknowns break the block structure (the reduced matrix has nodes
with fewer than three unknowns). Aggregates with too few nodes cannot carry six modes (the local QR is rank deficient and must
drop columns by a registered rule). Six unknowns per node (shells) and the modal solver (which factorises) do not profit: an
iterative eigensolver is left to the advanced tier.
