# ADR 0336: Elasticity solves on the ranks of rcfd-dist and on the CUDA device

Status: **PLANNED 2026-11-12** (layout by the advanced-tier plan, [ADR 0310](0310-structural-solver-advanced-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are the
acceptance draft that the registration turns into checks.

Work package SA-26 of the structural solver, advanced tier. Size M.

## Goal

Use the two parallel layers the repository has for the static solve of a solid: the stiffness matrix split by nodes over the ranks
of `rcfd-dist` (ADR 0058: in-process ranks only) and solved by its distributed CG with the block-Jacobi preconditioner; and the CG
of `rcfd-gpu` (ADR 0038) with Jacobi or Chebyshev on the device, chosen by the existing device setting. Assembly stays serial. The
glue lives in `rcfd-api`, which gains the dependency on `rcfd-dist`; the default solver is untouched.

## Dependencies

none in this list; roadmap items (SA ids are this tier's packages, ADR 0310 section 7): F-016, F-101, F-374, F-381.

## Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. Ranks: with one rank the iteration counts and the solution
have the bits of the serial CG of F-381 with the same preconditioner; with 2, 4 and 8 ranks the solution of a clamped cube (about
4e4 unknowns) and of a cantilever of Tet10 equals the serial one within 1e-8 relative at a residual of 1e-10, in at most twice the
iterations of one rank (counts recorded); repeated runs with the same rank count give the same bits; the three unknowns of a node
are never split; a rank that fails or times out ends the solve with a located error. Scaling is recorded, not asserted. Device:
with a usable device the device CG equals the CPU CG with the same preconditioner within 1e-9 on a cube of at least 3e5 unknowns,
times recorded; `auto` below the threshold runs on the CPU; without a device `cuda` is a located error and `auto` falls back with
the reason recorded, and the device clauses are reported as not run. The default solver's results and the baseline hashes are
unchanged.

## Reference to check against

The serial solution is the reference (as ADR 0058 and ADR 0038 did for the flow matrices). Preconditioned CG and block-Jacobi as
additive Schwarz without overlap (Saad, Iterative Methods for Sparse Linear Systems).

## Files

`crates/rcfd-api/Cargo.toml` (dependency on `rcfd-dist`), `crates/rcfd-api/src/solid_parallel.rs`, `crates/rcfd-api/src/lib.rs`
(one `pub mod` line), `crates/rcfd-api/tests/sa_solid_parallel.rs`.

## What could make it miss

`rcfd_mesh::partition::Decomposition` is built from a finite-volume mesh (cells); a solid needs a split of nodes, so the package
may have to build the rank rows itself (`DistCsr::from_rows`). Jacobi on elasticity converges slowly: the device is unlikely to
pay below about 1e6 unknowns (recorded, not asserted). No device on the machine: the device clauses are not run and the item is
partial. A real transport (MPI) is F-100 and is not structural work.
