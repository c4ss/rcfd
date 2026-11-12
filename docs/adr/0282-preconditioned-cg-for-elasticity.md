# ADR 0282: Iterative route for larger models: CG with block Jacobi and incomplete Cholesky on the stiffness matrix

Status: **REGISTERED 2026-11-12, written before any code of F-381 and before any run of its checks; planned by ADR 0270**.
Not yet accepted by the maintainer. Before this text nothing of F-381 was written, compiled or run: the code and documents
under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no script
and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Work package SB-12 of the structural solver, basic tier ([ADR 0270](0270-structural-solver-basic-tier-plan.md), section 3, the
entry for ADR 0282). Roadmap item F-381 (depends on F-374, done; F-026). Size M. Number 0282, fixed by the plan; the file
name of the planned stub is kept and the stub's text is kept unchanged at the end ("The planned stub, as written"). Branch
`f381-iterative-elasticity` from main 861e4bd5, worktree `w49i-work/f381`.

House rules (`w49i-work/AGENT_RULES.md`):

- limits are fixed here, before the code and the runs, and are not changed after a run; a missed limit becomes an ignored
  `*_known_miss` test with the limit intact and the number recorded, diagnosed, not tuned away; a defective set-up gets a
  corrected test by an amendment written and committed before the corrected run, the old test stays as an ignored record;
- each registered run is made once; nothing is re-run to pick a result;
- defaults bit-identical: the default `StaticSolver::Direct` and the existing `StaticSolver::ConjugateGradient` (Jacobi) give
  the bytes they gave before (check A7); `rcfd-linalg/src/precond.rs` only gains appended items (no existing line of it is
  changed, no re-export in `lib.rs`), so no finite-volume path can change; baseline hashes and the `rcfd-linalg` tests are run
  because a shared solver file is touched;
- no new dependency: `rayon` (already a workspace dependency of `rcfd-linalg`) is added to `rcfd-fem` as a **dev-dependency
  only**, for the thread pools of the determinism test; no unsafe code; the graph stays acyclic;
- clean-room: the algorithms are derived below from first principles (Gaussian elimination with dropping, diagonal dominance,
  Cholesky of a small block); papers are cited from memory for orientation only;
- F-380 (ADR 0281) is built at the same time in `loads.rs` and `recover.rs`: this package does not edit those files, nor
  `lib.rs` or `error.rs` of `rcfd-fem` (no new error variant is needed: `FemError::Solver` carries the reason).

## What F-381 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. CG on the stiffness matrix with (a) 3 x 3 block Jacobi
> and (b) incomplete Cholesky with zero fill on CSR and a diagonal shift on breakdown, selectable beside the direct solver: on
> a cantilever of about 5e3 unknowns (fast) and of at least 1e5 unknowns (heavy record) the relative residual reaches 1e-10;
> on the fast case the displacement agrees with the direct solution within 1e-7 relative, incomplete Cholesky needs at most
> half the iterations of block Jacobi, and the iteration count and the solution are bit-identical for 1 and 8 threads; a
> breakdown is reported with the shift used, never as a non-finite result. The default solver and its results are unchanged
> bit for bit.

Clauses: (c1) CG with 3 x 3 block Jacobi, selectable; (c2) CG with IC(0) on CSR with a diagonal shift on breakdown,
selectable; (c3) fast cantilever (~5e3 unknowns): relative residual <= 1e-10, both; (c4) heavy cantilever (>= 1e5 unknowns):
relative residual <= 1e-10, both; (c5) fast: displacement within 1e-7 relative of the direct solution, both; (c6) fast:
iterations of IC <= 0.5 x iterations of block Jacobi; (c7) fast: iteration count and solution bit-identical for 1 and 8
threads, both; (c8) a breakdown is reported with the shift used; (c9) never a non-finite result; (c10) the default solver and
its results unchanged bit for bit.

The brief adds: decide whether IC(0) goes into `rcfd-linalg` or `rcfd-fem` (D1), the shift strategy (D3, with Manteuffel or a
derivation), how thread determinism is guaranteed (D6); ADR 0277's note for F-381 (record the residual of the direct and the
iterative routes against the slenderness: record X1); ADR 0277's finding (direct residual 6.7e-11 on a slender Tet10
cantilever) is F-455's, not done here.

## What was read (main 861e4bd5, read only)

1. `AGENT_RULES.md`; ADR 0270 sections 1, 2.1 to 2.4, the entry for ADR 0282 in section 3, the wave table of section 4 and
   section 6 ("half the iterations" is among the numbers the planner trusts least); this ADR's stub; ADR 0275 in full (D2
   determinism, D6 the static solve, its note for F-381); ADR 0277's remaining question 1 and note for F-381; ADR 0284's
   amendment 1 (the residual floor of the eigensolver `c eps lambda_max / lambda_1` on slender cantilevers) by search.
2. Code: `crates/rcfd-fem/src/solve.rs` in full; `model.rs` (`SolverStats`, its solver name is a `String`); `lib.rs`;
   `error.rs` (variant list); `Cargo.toml`; `builders.rs` (node numbering `i + (nx + 1)(j + (ny + 1) k)`, x fastest);
   the test helpers of `tests/sb_modal.rs` (`box_of`, `model_of`, `cantilever`) and the traction loads in `sb_recover.rs`.
   `crates/rcfd-linalg/src/{lib.rs, precond.rs (in full: Identity, Jacobi, Ic0 on LDU, Ilu0 on CSR, Chebyshev),
   solvers.rs (cg), vecops.rs (dot, norm2 over fixed chunks of 4096), matrix.rs (CsrMatrix, matvec over fixed row chunks,
   residual, diag)}`, `Cargo.toml`.
3. **Not read:** the bodies of the papers cited; any finite-element or preconditioner code outside this repository.

Findings that shape the design:

1. `rcfd_linalg::Ic0` is the diagonal incomplete Cholesky of the finite-volume `LduMatrix` (no off-diagonal update, no
   breakdown handling); `Ilu0` is ILU(0) on CSR with no pivot test (a zero or negative pivot gives an infinite or indefinite
   factor silently). Neither fits clause c2. The name `Ic0` is taken: the new type is `IncompleteCholesky`.
2. `rcfd_linalg::cg` stops on the recursive residual (`r_k` updated by `r -= alpha q`), never on the true residual `b - A x`.
   On a slender model the two drift apart by rounding (ADR 0284 amendment 1 met the analogous floor in the eigensolver);
   clause c3 is about the true residual, so the static solve must check it (D5).
3. `cg`'s reductions (`dot`, `norm2`) and `CsrMatrix::matvec` are independent of the thread count by construction (fixed
   chunks, partial sums combined in chunk order; rows summed sequentially). `rcfd-fem`'s assembly is too (ADR 0275 D2). The
   preconditioners added here are applied sequentially. The only thread dependence would come from a new parallel reduction;
   none is added (D6).
4. The reduced system of `constraint::reduce` keeps the free equations in ascending order and `DofMap` numbers node by node,
   so the free components of one node are consecutive rows of `K_FF`: the 3 x 3 blocks are runs of consecutive rows with the
   same node (1, 2 or 3 rows where components are prescribed).

## Arithmetic before this registration (disclosed, by hand)

1. **IC(0) as elimination with dropping.** Write `A = L D L^T` with `L` unit lower. Row by row, for `k < i` in the stored
   lower pattern of row i, in ascending k: `l_ik = (a_ik - sum_{j < k} l_ij d_j l_kj) / d_k`, the sum over the j stored in
   both row i and row k of `L`; then `d_i = a_ii - sum_{k < i} l_ik^2 d_k`. Zero fill: `l_ik` exists only where `a_ik` is
   stored. Where the pattern of `A` is closed under elimination (a full matrix, a tridiagonal one) nothing is dropped and the
   factor is the exact `L D L^T`. In exact arithmetic the factor of a symmetric matrix equals the ILU(0) factor of the same
   matrix with `U = D L^T` (the same recurrences read by rows), which gives an independent check against the existing `Ilu0`.
2. **Breakdown: the Kershaw-type matrix.** `K4 = [[3, -2, 0, 2], [-2, 3, -2, 0], [0, -2, 3, -2], [2, 0, -2, 3]]` (pattern of
   a 4-cycle). Exact `L D L^T`: `d = (3, 5/3, 3/5, 1/3)` (with the fill `l_42 = 4/5`, `l_43 = -2/3`): positive definite. IC(0)
   drops `l_42`: `d_1 = 3`, `l_21 = -2/3`, `l_41 = 2/3`, `d_2 = 3 - 4/3 = 5/3`, `l_32 = -6/5`, `d_3 = 3 - (36/25)(5/3) = 3/5`,
   `l_43 = -2/(3/5) = -10/3`, `d_4 = 3 - (4/9) 3 - (100/9)(3/5) = 3 - 4/3 - 20/3 = -5`: a negative pivot at row 3
   (0-based), although `K4` is positive definite. With the shifted diagonal `s = 3 (1 + alpha)` (off-diagonals unchanged):
   `d_1 = s`, `d_2 = s - 4/s`, `d_3 = s - 4/d_2`, `d_4 = s - 4/s - 4/d_3`. `s = 3.4`: `d_4 = 3.4 - 1.176 - 2.498 = -0.27`;
   `s = 3.5`: `+0.14`; the root is near `s = 3.47`, `alpha = 0.157`. With the registered sequence (D3) `alpha = 0, 1e-3,
   2e-3, ..., 0.128` (`s = 3.384`, fails), `0.256` (`s = 3.768`: `d_2 = 2.7064`, `d_3 = 2.2900`, `d_4 = 3.768 - 1.0616 -
   1.7467 = 0.960 > 0`): success at `alpha = 1e-3 * 2^8 = 0.256` after **9 breakdowns** (alpha = 0 and the eight shifts
   1e-3 to 0.128), the first at row 3 with pivot -5.
3. **Diagonal dominance guarantees termination.** Let `A` be symmetric with positive diagonal and `S_i = sum_{j != i}
   |a_ij|`. Eliminate pivot k with dropping. For a later row i, the new diagonal is `a_ii - a_ik^2 / a_kk` and the new
   off-diagonal sum over the still active columns is at most `S_i - |a_ik| + |a_ik| (S_k - |a_ik|) / a_kk` (each update adds at
   most `|a_ik| |a_kj| / a_kk`; dropping only removes terms). So the margin `m_i = a_ii - S_i` becomes at least `m_i + |a_ik| (1
   - S_k / a_kk) >= m_i` if row k is strictly dominant. By induction every row stays strictly dominant with a margin no smaller
   than its initial one, and the pivot is `d_k >= a_kk - S_k`. For the shifted matrix (diagonal `(1 + alpha) a_ii`) the pivot
   test `d_k > tau (1 + alpha) a_kk` therefore holds for every k once `(1 - tau)(1 + alpha) a_kk > S_k`, i.e. for `alpha >=
   alpha* = max_k S_k / ((1 - tau) a_kk) - 1`. The doubling sequence passes `alpha*` after `log2(alpha*/1e-3) + 2` attempts:
   for `K4`, `alpha* = 4/3 - 1 = 1/3` (tau negligible), reached at 0.512; for a Hex8 stiffness `S_k / a_kk` is of order 3 to
   6 (26 neighbours, three components, entries a fraction of the diagonal), so at most about 14 attempts; 64 attempts are
   allowed (`alpha` up to `1e-3 2^62`), and the case of no success is an error, not a result. This is the argument behind
   Manteuffel's shifted factorisation (Math. Comp. 34 (1980) 473, from memory: the incomplete factorisation of an H-matrix
   exists, and `A + alpha diag(A)` becomes one for alpha large enough); it is used here in the elementary form derived above.
4. **Pivot tolerance.** ADR 0275 arithmetic 4: in the exact factorisation of a positive definite matrix `d_i / a_ii >= 1 /
   (a_ii (A^{-1})_ii)`, above 1e-10 for a solid cantilever up to `L/h ~ 1000`. A pivot of IC(0) at or below `tau = 1e-10` of
   its (shifted) diagonal is therefore far outside what the exact factor gives and is treated as a breakdown, as is a
   negative or non-finite one. The test is `!(d_i > tau a~_ii)`, so a NaN pivot is a breakdown.
5. **The fast cantilever.** Hex8 box `[0, 8] x [0, 1.5] x [0, 1.5]` m, divisions `[32, 6, 6]` (cubes of 0.25 m), `L/h =
   5.33`; `33 x 7 x 7 = 1617` nodes, 4851 equations, the 49 nodes of `xmin` clamped: **4704 unknowns** ("about 5e3"). Tip
   traction `(0, 0, -1e6)` Pa on the surface set `xmax`, `F = 2.25e6` N; `E = 2e11` Pa, `nu = 0.3`. Euler-Bernoulli tip
   deflection `4 F L^3 / (E h^4) = 4 x 2.25e6 x 512 / (2e11 x 5.0625) = 4.6e-3` m (order of magnitude only; no limit on it).
6. **The heavy cantilever.** The same box with divisions `[96, 18, 18]` (cubes of 1/12 m): `97 x 19 x 19 = 35017` nodes,
   105 051 equations, 361 clamped nodes: **103 968 unknowns** (>= 1e5).
7. **Residual floor.** The true residual `b - A x` cannot be computed better than about `c eps ||A|| ||x||`, and CG and the
   direct solve both stop near `c eps ||A|| ||x|| / ||b||` relative. For a tip load: `||A|| ~ 2 E a` (a the cell size),
   `||x||_2 ~ delta sqrt(N/3)` (N nodes, the deflection varying along the length), `||b||_2 = F / sqrt(m)` (m tip nodes), so
   `||A|| ||x|| / ||b|| ~ 8 (a/h) (L/h)^3 sqrt(N/3) sqrt(m)`. Calibration on ADR 0277's measurement (Tet10 10 x 2 x 2, L/h =
   10, node spacing h/4, N = 525, m = 25, `||A||` ten times larger for Tet10): `~ 6.6e5`, `eps x 6.6e5 = 1.5e-10` against the
   measured 6.7e-11, so `c ~ 0.5`. Fast case: `8 x (1/6) x 152 x 23.2 x 7 = 3.3e4`, floor `~ 0.5 x 2.2e-16 x 3.3e4 = 3.6e-12`:
   a factor 30 below 1e-10. Heavy case: `8 x (1/18) x 152 x 108 x 19 = 1.4e5`, floor `~ 1.5e-11`: a factor 7. A slender record
   case `[64, 6, 6]` (L/h = 10.7): `8 x (1/6) x 1213 x 32.8 x 7 = 3.7e5`, floor `~ 4e-11` (record only, X1).
8. **Iterations.** With Jacobi scaling, `lambda_max ~ 2` and `lambda_min ~` the Rayleigh quotient of the first bending shape,
   `kappa ~ 1.6 (a/h) (L/h)^3 N`: fast `0.27 x 152 x 1617 = 6.5e4`, heavy `0.089 x 152 x 35017 = 4.7e5`. The CG bound
   `0.5 sqrt(kappa) ln(2/1e-10)` gives about 3000 (fast) and 8100 (heavy) iterations for (block) Jacobi; the bound is
   pessimistic for a clustered spectrum (ADR 0275 E6: 142 iterations on a small Hex8 bar). IC(0) on such matrices reduces
   `kappa` by a constant factor (not its order in `a`); from general experience (not a derivation) IC(0) needs a third to a
   half of the iterations of a diagonal preconditioner on 3-D elasticity. The factor 0.5 of clause c6 is the plan's draft and
   is kept as the acceptance's limit; the margin may be small. Caps: `max_iterations = 20 000` (fast), `60 000` (heavy).
9. **Agreement with the direct solution.** With `||b - A x|| <= 1e-10 ||b||` the error `e = A^{-1} r` has `||e||_2 <= 1e-10
   ||b|| / lambda_min`, and for a tip load `||x||_2 ~ c_1 ||b|| / lambda_min` (c_1 the share of the first mode in b, order
   0.5): `||e|| / ||x|| <~ 2e-10`, in the maximum norm at most `sqrt(N)` worse, `<~ 1e-8`; the direct solution's own error is
   of the same order (its residual is at the floor of arithmetic 7). Limit 1e-7: margin 10 or more.
10. **Cost.** Heavy case: 27 adjacent nodes per interior node, 81 entries per row, `8.4e6` stored entries;
    a CG iteration (matvec, IC apply with two triangular sweeps over the `4.2e6` lower entries) `~ 3e7` flops, `~ 30` ms;
    5000 to 8000 iterations of block Jacobi and 2000 to 4000 of IC: 2 to 5 minutes in all. The direct solve of the heavy case
    is not part of the acceptance and is not run.

## Design

### D1. Where the code lives

Both preconditioners are generic numerics and go into `rcfd-linalg/src/precond.rs`, appended after the existing items (ADR
0270 section 2.1: "generic numerics go into `rcfd-linalg`"; the plan's file list), reachable as
`rcfd_linalg::precond::{BlockJacobi, IncompleteCholesky, IcOptions, IcBreakdown, PrecondError}` without a re-export in
`lib.rs` (the plan keeps `lib.rs` for other waves). The finite-volume side can use them later. Taken on the maintainer's
standing instruction of 2026-11-11 (take the recommendation).

### D2. Block Jacobi (`BlockJacobi`)

`BlockJacobi::new(a: &CsrMatrix, block_ptr: &[usize]) -> Result<BlockJacobi, PrecondError>`: `block_ptr` partitions the rows
into consecutive blocks (`block_ptr[0] = 0`, strictly increasing, last `= n`; else `InvalidBlocks`). Each diagonal block is
copied dense from the CSR (absent entries 0) and factored by Cholesky `B = C C^T`; a pivot that is not positive and finite is
refused (`BlockNotPositive { block, row, pivot }`). Apply: per block, forward and backward substitution with `C`, block by
block in order. `BlockJacobi::uniform(a, size)`: blocks of `size` rows (the last shorter). In the static solve the blocks are
the free components of each node (finding 4): `solve::node_blocks(free, dofs) -> Vec<usize>`, public so that check B1 can
test it. With every node unconstrained or clamped this is the "3 x 3 block Jacobi" of clause c1.

### D3. Incomplete Cholesky with zero fill and a shift (`IncompleteCholesky`)

`IcOptions { initial_shift: 0.0, shift_step: 1e-3, pivot_tolerance: 1e-10, max_attempts: 64 }` (`Default`).
`IncompleteCholesky::new(a, &options) -> Result<IncompleteCholesky, PrecondError>`:

* Refusals before factoring: not square (`NotSquare`); a row without a stored diagonal (`MissingDiagonal { row }`); a
  diagonal that is not positive (`NonPositiveDiagonal { row, value }`); any stored value that is not finite (`NonFinite {
  row, col }`). Only the lower triangle (columns `<= i`) is read; the matrix is assumed symmetric (the stiffness is, to
  `~1e-16`, ADR 0275 S1).
* Attempt with shift `alpha` (starting at `initial_shift`): the recurrences of arithmetic 1 on the stored lower pattern with
  the diagonal `(1 + alpha) a_ii`, row by row, the sum for `l_ik` over the common columns of rows i and k in ascending order
  (a dense marker of row i's positions), `d_i` by subtracting `l_ik^2 d_k` in ascending k. The first pivot with `!(d_i >
  pivot_tolerance (1 + alpha) a_ii)` ends the attempt: an `IcBreakdown { shift: alpha, row: i, pivot: d_i, diagonal: (1 +
  alpha) a_ii }` is recorded and the next shift is `max(2 alpha, shift_step)` (arithmetic 3). After `max_attempts` failed
  attempts: `NoShift { attempts, last_shift, breakdowns }`.
* The factor keeps `L` (unit lower, without the diagonal) in CSR, `1/d`, the shift used and the list of breakdowns
  (`shift()`, `breakdowns()`, `attempts()`).
* Apply `z = (L D L^T)^{-1} r`: forward sweep `y_i = r_i - sum l_ik y_k` (ascending rows, each sum in ascending k), `y_i /=
  d_i`, backward sweep by columns (descending i: `x_i` final, then `x_k -= l_ik x_i` for the entries of row i): sequential,
  fixed order.

Strategy (clause c8): Manteuffel's shift `A + alpha diag(A)` (from memory) with the doubling sequence from `1e-3`; I recall the
same rule (`alpha <- max(2 alpha, alpha_S)`, `alpha_S = 1e-3`) from Lin and More's ICFS (SIAM J. Sci. Comput. 21 (1999) 24,
from memory); the termination argument is derived in arithmetic 3. Taken on the maintainer's standing instruction of
2026-11-11 (take the recommendation): (a) this restart-with-a-larger-shift; the alternatives (b) replacing a failed pivot by
its diagonal locally, (c) a fixed shift chosen in advance, were not taken: (b) has no termination or quality argument, (c)
costs iterations on every model that does not break down.

### D4. Selection in the static solve (`solve.rs`)

* New variant (the enum is `#[non_exhaustive]`): `StaticSolver::PreconditionedCg { preconditioner: CgPreconditioner,
  tolerance, max_iterations }` with `CgPreconditioner::{BlockJacobi, IncompleteCholesky(IcOptions)}`. `Direct` stays the
  default; `ConjugateGradient` (Jacobi) is unchanged in behaviour.
* Solver names in `SolverStats`: `"cg-block-jacobi"`, `"cg-ic0"`, and when the factorisation broke down `"cg-ic0, shift
  <alpha:e> after <k> breakdown(s)"` (clause c8 in every result).
* `solve_static_report(model, prescribed, forces, options) -> Result<(SolidResult, SolveReport)>` does the work;
  `solve_static_with` returns its first part (the default path computes exactly what it computed before: the same calls in the
  same order). `SolveReport { solver, iterations, restarts, relative_residual, shift: Option<f64>, breakdowns: Vec<(node,
  component, IcBreakdown)> }` (rows of a breakdown mapped back to node and component).
* Errors: a refusal of a preconditioner is `FemError::Solver { solver, reason }` with the reason text (row mapped to node and
  component where there is one); not converged likewise, stating iterations, true residual and tolerance.

### D5. The iteration and its stopping test

`pcg(a, b, m, tolerance, max_iterations)`: `target = tolerance ||b||_2` (`vecops::norm2`), `x = 0`; call `rcfd_linalg::cg`
with `tol_abs = target`, `tol_rel = 0`, the remaining iteration budget; then the true residual `||b - A x||_2` (`residual`,
`norm2`). If it is `<= target`: done. If not and `cg` had converged on its recursive residual: restart `cg` from the current
`x` (its initial residual is then the true one), at most **3 restarts**; otherwise not converged (error). Iterations are
summed over restarts. After every call: if the residual or any entry of `x` is not finite, the solve is refused
(`FemError::Solver`, "non-finite iterate") (clause c9). The relative residual stored in `SolverStats` is the existing
`residual_norm` of `solve.rs` (sequential sum), the number clause c3 is checked on.

### D6. Determinism (clause c7)

Every operation of the iterative route is either sequential (both factorisations, both applies, the block partition) or one of
`rcfd-linalg`'s kernels whose result does not depend on the thread count: `dot`/`norm2` (fixed chunks of 4096, partial sums
combined in chunk order), `axpy`/`xpby` (element-wise), `CsrMatrix::matvec`/`residual` (each row summed sequentially), and
the assembly of ADR 0275 D2 (sequential scatter in element order). So the iteration count and every bit of the solution are
the same for any rayon pool and any `StaticOptions::threads`. No new parallel reduction is introduced; a parallel apply of
block Jacobi would also be deterministic but is not needed (its cost is below the matvec's).

### D7. Test set-up (`crates/rcfd-fem/tests/sb_pcg.rs`)

Material `E = 2e11` Pa, `nu = 0.3`, density 0 (no mass needed). Cantilevers of arithmetic 5, 6, 7 built with `hex_box`,
`Constraint::displacement("xmin", c, 0.0)` for `c = 0, 1, 2`, `Load::traction("xmax", [0, 0, -1e6])`. Tolerance `1e-10`,
`max_iterations` 20 000 (fast) and 60 000 (heavy). Thread pools by `rayon::ThreadPoolBuilder` (dev-dependency) of 1 and 8
threads, with `StaticOptions::threads` 1 and 8 respectively. No random numbers are used.

## Registered checks

All in `crates/rcfd-fem/tests/sb_pcg.rs`, release profile, one run (`cargo test -p rcfd-fem --release --test sb_pcg --
--nocapture --test-threads=1`); the heavy record `h1_heavy_cantilever` is `#[ignore]` and run once with `--ignored`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| A1 | c1, c2, c3 | fast cantilever, `PreconditionedCg` with `BlockJacobi` and with `IncompleteCholesky(default)`: `SolverStats::relative_residual` | `<= 1e-10` each | arithmetic 7: floor `~ 4e-12` |
| A2 | c5 | fast: `max |u_cg - u_d| / max |u_d|` over all components, `u_d` the default direct solve | `<= 1e-7` each | arithmetic 9: `<~ 1e-8` |
| A3 | c6 | fast: `iterations(IC) / iterations(BJ)` | `<= 0.5` | arithmetic 8: 0.3 to 0.5 (experience, not derivation) |
| A4 | c7 | fast, both: pool of 1 thread with `threads(1)` and of 8 threads with `threads(8)`: iteration counts equal and every displacement bit-equal (`to_bits`) | equal | D6 |
| A5 | c2, c8 | `K4` of arithmetic 2 through `IncompleteCholesky::new(default)`: (a) first breakdown at row 3 with pivot `-5` (relative error `<= 1e-14`) and shift 0; (b) 9 breakdowns, final shift `== 1e-3 * 2^8` (computed by the same doublings, bit-equal); (c) the pivots `d` of the factor against the closed form of arithmetic 2 at `s = 3 (1 + 0.256)`, relative `<= 1e-14`; (d) every pivot positive and finite | as stated | arithmetic 2 |
| A6 | c8, c9 | fast cantilever with `IncompleteCholesky(IcOptions { pivot_tolerance: 0.9, .. })` (forces breakdowns): the result's solver name contains "shift" and the report's shift `> 0` and its breakdown list non-empty, every displacement finite, relative residual `<= 1e-10`, agreement with the direct solution `<= 1e-7` | as stated | arithmetic 3: succeeds by `alpha ~ 10 S/a`, about 15 attempts |
| A7 | c10 | (a) the existing `rcfd-fem` tests pass unedited (release); (b) the FNV-1a hash of the bits of displacement, reactions, relative residual and iterations of the default (`Direct`) solve and of the `ConjugateGradient { 1e-10, 20 000 }` solve of the fast cantilever (or of its error text) is the same on this branch and on a `git archive` of main 861e4bd5 (the same test function, compiled there as a scratch test file); (c) `baseline_hashes` passes | equal / pass | D4: same calls, same order; D1: appended items only |
| H1 | c3, c4 | heavy cantilever (`#[ignore]`, run once), both preconditioners: relative residual | `<= 1e-10` each | arithmetic 7: floor `~ 1.5e-11` |
| B1 | D2 | `node_blocks` on the fast cantilever's reduced system: one block per node with a free component (1568), all of size 3, each block's rows one node's | as stated | finding 4 |
| L1 | D3 | `IncompleteCholesky` of a full-pattern 6 x 6 positive definite matrix (`A = G G^T + 6 I`, `G_ij = sin(i + 2 j + 1)`): `max |M^{-1} A x - x| / max |x|`, `x_i = 1 + i` (IC(0) is the exact factor) | `<= 1e-12` | arithmetic 1 |
| L2 | D3 | on the 2-D five-point matrix 12 x 10 (diagonal 4, off-diagonals -1, CSR from triplets): `IncompleteCholesky` and the existing `Ilu0` applied to `r_i = sin(i)`: `max |z_IC - z_ILU| / max |z_ILU|`; no breakdown, shift 0 | `<= 1e-13` | arithmetic 1 (an M-matrix: no breakdown) |
| L3 | D2 | `BlockJacobi::uniform(a, 3)` on a 9 x 9 matrix with three SPD 3 x 3 diagonal blocks and off-block couplings: apply against each block's inverse by the cofactor formula, relative | `<= 1e-14` | Cholesky of a 3 x 3 block, `~ 10 eps` |
| L4 | D2, D3 | refusals: a zero diagonal (`NonPositiveDiagonal` naming the row), a NaN entry (`NonFinite` naming row and column), a missing diagonal, a block that is not positive definite (`BlockNotPositive` naming the block), an invalid `block_ptr`; `max_attempts = 3` on `K4` gives `NoShift` with 3 breakdowns | as stated | D2, D3 |
| L5 | c9 | the fast cantilever with `max_iterations = 5`: an error stating "not converged" and the iterations, no result | as stated | D5 |
| X1 | 0277 note | record: relative residuals of the direct solve and of both preconditioners and the iteration counts, on the fast case and on the slender `[64, 6, 6]` case (L/h = 10.7, 9312 unknowns); and the shift and breakdowns of the default IC on the fast and heavy cases; times | record | arithmetic 7: slender floor `~ 4e-11` |

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | A2's measure for block Jacobi CG stopped at tolerance `1e-4` | `> 1e-7` |
| N2 | A3's ratio with `IncompleteCholesky(IcOptions { initial_shift: 1.0, .. })` (factor of `A + diag(A)`) | `> 0.5` |
| N3 | A4's comparison between the block Jacobi and the IC solutions (the measure sees a difference) | not bit-equal |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f381-scratch`, own target directory; each applied alone
by exact text replacement, `sb_pcg` run once (fast tests), the file restored and compared byte for byte):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | IC: `l_ik = a_ik / d_k` (the sum over common columns left out) | L1 |
| M2 | IC: the shift not applied to the diagonal | A5 (no success: `NoShift`) |
| M3 | IC: the pivot test removed (every pivot accepted) | A5 |
| M4 | IC apply: the backward sweep left out | L1, L2 |
| M5 | block Jacobi: the off-diagonal entries of each block dropped (scalar Jacobi) | L3 |
| M6 | `node_blocks`: blocks of three consecutive reduced rows regardless of the node | B1 |
| M7 | D5: no restart (the recursive residual accepted) | A1 or recorded as surviving if no restart happened |
| M8 | `vecops::dot`: a plain rayon `par_iter().map().sum()` (thread-dependent order) | A4 |
| M9 | D4: `IncompleteCholesky` dispatched to block Jacobi | A3 |
| M10 | D5: the non-finite test removed and the IC pivot test made `d_i != 0` | A6 or L4, or recorded |

**Decision rule.** F-381 is proposed **done** (pending the maintainer's acceptance) if A1 to A7 and H1 are met (every
acceptance clause), B1, L1 to L5 hold, N1 to N3 hold, and every mutation fails its test (M7 and M10 may survive if recorded
with the reason: no restart happened, or no non-finite value reachable). A miss of any of A1 to A7 or H1, an unrun H1, a
failed negative control, or an uncaught mutation other than M7, M10: **partial**, with the miss named. X1 is a record without
a limit. No crate that builds: **todo**.

## Limits (what is not covered)

- One-level preconditioners only: the iteration count grows like `sqrt(kappa)`, `kappa ~ (L/h)^3 N (a/h)`, so slender or
  large models take thousands of iterations; aggregation AMG with rigid-body modes is ADR 0305.
- The natural (node) order is used for IC(0); no reordering is tried.
- Only CG: the stiffness must be symmetric positive definite after the constraints (the rigid-body check of ADR 0275 D5 runs
  first; a mechanism shows as a breakdown or as non-convergence, not as `SingularStiffness`).
- IC(0) reads the lower triangle only and assumes symmetry.
- The true-residual restarts (D5) cannot go below the floor of arithmetic 7; a model whose floor exceeds the tolerance is
  reported as not converged.
- Nothing in the finite-volume solvers uses the new preconditioners.

## Amendment 1 (2026-11-12, written and committed before the corrected run)

The registered run (code e8e8b82a, log `w49i-work/f381-scratch/run1.log`, made once) ended with `a1_a2_a3_a6_fast_cantilever`
failing on A3. That test prints every measure before its assertions, so all of A1, A2, A3, A6, N1, N2 were measured: A3 is
missed (IC 85 iterations, block Jacobi 156, ratio 0.5449 against 0.5) and the negative control N1 failed (2.2e-8, expected
`> 1e-7`). Every other check of the run was met; the numbers go in the addendum.

1. **A3 is a miss, not a defect of the set-up.** The limit stays 0.5. The test is kept as the ignored record
   `a1_a2_a3_a6_fast_cantilever_known_miss` (renamed, `#[ignore]` with the measured numbers; its body is unchanged). The
   cause is diagnosed in the addendum from numbers already measured; nothing is re-run with other parameters to meet it.
2. **N1 is a defect of the set-up.** A2's measure must be shown able to exceed 1e-7; stopping at a relative residual of 1e-4
   did not show it, because the residual left at that point lies in the stiff (high) modes, whose displacement is small: the
   measured error per unit of relative residual was `2.2e-8 / 1e-4 = 2.2e-4`. Corrected control **N1b** (new test
   `n1b_agreement_measure_is_sensitive`): block Jacobi CG on the fast cantilever stopped at tolerance **1e-1**: A2's measure
   `> 1e-7`. Estimate: at least `2.2e-4 x 1e-1 = 2e-5` by the ratio just measured, larger in fact because after a handful of
   iterations the bending mode is not yet resolved; margin above 200. The old N1 stays in the ignored record of item 1.
3. **Regression guard of the met checks.** Because the record of item 1 is ignored, the met checks A1, A2, A6 and N2 are
   repeated by a new fast test `a1_a2_a6_fast_cantilever` with the same set-up and the same assertions without A3 and N1.
   It is deterministic (A4): its printed numbers must equal those of run 1 digit for digit; it is a guard for the mutations
   and the gate, not a new measurement.
4. The mutation runs (registered) use `--include-ignored` except `h1_heavy_cantilever`, so that the record of item 1 also
   counts (M9 must still fail A3 there, with the ratio moving to about 1).

The decision rule is unchanged: with A3 missed it gives **partial**.

## Amendment 2 (2026-11-12, written and committed before the corrected run)

**What happened.** The registered heavy run H1 (code e8e8b82a, log `w49i-work/f381-scratch/run-h1.log`, made once, 61 s)
did not converge with either preconditioner: block Jacobi stopped after 6530 iterations and 3 restarts with a true residual of
1.42e-3 against the target 1.215e-5 (relative 1.17e-8 against 1e-10), IC(0) after 2540 iterations and 3 restarts at 9.16e-4
(relative 7.5e-9). Both had converged on their recursive residual each time. H1 is **missed**; the test is kept as the
ignored record `h1_heavy_cantilever_known_miss` (renamed, body unchanged, the numbers in its ignore text).

**Diagnosis (a defect of D5, not of the set-up).** Arithmetic 7 estimated the floor of the *best attainable* residual, one
rounding of `x`: `c eps ||A|| ||x|| / ||b|| ~ 1.5e-11`. CG's update `x <- x + alpha p` rounds `x` at every step by `~ eps |x|`,
and these roundings are not seen by the recursive residual: after k steps the gap between the true and the recursive residual
is of order `eps ||A|| ||x|| sqrt(k)` to `k` times that (the accumulated rounding of the iterate; Greenbaum's analysis of
the attainable accuracy of CG, from memory). With `k ~ 5000` this is a factor 70 to 5000 above the one-rounding floor:
1e-9 to 1e-7, which brackets the measured 1.2e-8 and 7.5e-9. The registered restart (D5) restarts `cg` on the same iterate
`x`, so each restarted run accumulates the same kind of rounding again and cannot get below that gap.

**Corrected design D5' (registered now).** A restart becomes a step of iterative refinement with CG as the inner solver:
with `r_j = b - A x_j` (true residual), solve `A delta = r_j` by `cg` from `delta = 0` with the same preconditioner,
`tol_abs = target`, `tol_rel = 0` and the remaining budget, then `x_{j+1} = x_j + delta` (one element-wise addition). The
inner iterate has the size of the correction, `||delta|| ~ ||A^{-1} r_j||`, so its per-step roundings are `eps |delta|`, a
factor `||delta|| / ||x|| ~ 1e-8` smaller, and `x` is rounded once per refinement step: the residual after the step is the
inner residual (`<= target` on its recursive estimate, true within `eps ||A|| ||delta|| k`, negligible) plus `c eps ||A||
||x||`, the floor of arithmetic 7, `~ 1.5e-11` relative for the heavy case. The first run is unchanged (`cg` on `x` from 0):
where no restart was needed (the fast case, all checks of the registered run that did not restart) the results are the same
bits; the limit of 3 restarts is kept. This is classical iterative refinement (Wilkinson), and the same idea as the group
update of van der Vorst and Ye's residual replacement (SIAM J. Sci. Comput. 22 (2000) 835, from memory); it is derived here
from the rounding argument above.

**Corrected test.** `h1b_heavy_cantilever` (`#[ignore]`, heavy, run once): the body of H1 unchanged (same model, both
preconditioners, tolerance 1e-10, 60 000 iterations), limit relative residual `<= 1e-10` each and `>= 1e5` unknowns.
Estimate: the floor `~ 1.5e-11` (margin 7); the iteration count rises by the inner solves (an inner reduction of about 1e-3
costs a few thousand more iterations for block Jacobi), within the cap.

**What else it touches.** In the registered fast run no restart occurred in A1, A2, A3, A4, A6, B1, L5 (the counts printed
0 restarts), so their numbers stand and the guard of amendment 1 must reproduce them. The record X1 of the slender case had
one restart for each preconditioner; its run-1 numbers stay recorded as made with D5 and the run with D5' is printed by the
same test and recorded beside them (X1 has no limit). Mutation M7 (no restart) is now registered against H1b as well
("must fail H1b, else recorded"); it is run on the heavy test once only if time allows, else recorded as not run.

The decision rule is unchanged: H1b met and A3 missed gives **partial** (A3); H1b missed as well gives partial with both.

## Amendment 3 (2026-11-12, written and committed before the mutation runs and before the run of B2)

**Defect of the set-up.** Preparing the mutation runs showed that B1 cannot catch M6: on the fast cantilever every node
outside `xmin` has all three components free, so "blocks of three consecutive reduced rows" and "one block per node" are the
same partition. B1 is kept unedited (it passed).

**Added check** (new test `b2_node_blocks_with_partial_constraints`; no library change; one run):

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| B2 | c1, D2 | `node_blocks` with the `DofMap` of the fast cantilever and the free list of all equations except (node 0, component 0), (node 5, components 1 and 2), (node 9, component 1): block sizes `2, 3, 3, 3, 3, 1, 3, 3, 3, 2, 3, ...` (1617 blocks), every block one node's | as stated | finding 4 |

M6 must fail B2. The decision rule is unchanged, with B2 beside B1.

## Amendment 4 (2026-11-12, written and committed before the corrected run)

**What happened.** The heavy test H1b of amendment 2 (code 6406bee9, log `w49i-work/f381-scratch/run-h1b.log`, made once,
172 s) did not converge either: true residual 2.35e-4 (block Jacobi, 6782 iterations) and 2.36e-4 (IC, 2358 iterations)
after 3 refinement steps, relative 1.9e-9 against 1e-10, the same for both preconditioners.

**Diagnosis: a defect of the set-up of H1 and H1b.** A scratch diagnosis (not a registered check; scratch file
`w49i-work/f381-scratch/mut/crates/rcfd-fem/tests/diag_floor.rs`, log `diag.log`) solved the reduced systems by IC-CG to a
recursive residual of 1e-12 and evaluated the residual in working precision, with compensated sums and products, and the
componentwise bound `eps || |A| |x| ||_2 / ||b||_2` of its evaluation. It showed the heavy model's tip deflection to be 0.121
m against 4.5e-3 m for the fast one: the test helper `cantilever(div)` sets the length to `8 nx / 32` m (meant for the
slender record X1, `[64, 6, 6]`, L = 16 m), so H1 and H1b ran a **24 m** beam (L/h = 16, cells 0.25 x 0.083 x 0.083 m)
instead of the registered `[0, 8] x [0, 1.5]^2` box with cubes of 1/12 m (arithmetic 6). On that model the bound is 5.0e-9 and
the compensated residual of the CG iterate 9.9e-9: its floor is above 1e-10 whatever the iteration, which H1 and H1b measured.
The same diagnosis gives for the fast case bound 1.28e-11 (residual of the iterate 8.5e-12) and for the slender record
1.41e-10 (1.50e-10): the attainable residual is about the componentwise bound, a better estimate than arithmetic 7's
`||A|| ||x||`, which ignored that `|A| |x|` sums about four diagonals' worth of absolute row entries.

**Estimate for the registered heavy model.** From the fast case's bound 1.28e-11: `|A| |x|` per row scales with the cell
size (x 1/3), `sqrt(N)` with `sqrt(103968/4704) = 4.7`, `||b||_2 = F / sqrt(m)` with `1 / (19/7)`: bound `~ 1.28e-11 x 4.7
x 2.71 / 3 = 5.4e-11`, a margin of about 2 to the limit 1e-10. With D5' the residual reached is about the bound (fast and
slender cases above).

**Corrected test.** `h1c_heavy_cantilever` (`#[ignore]`, heavy, run once): the box `[0, 8] x [0, 1.5] x [0, 1.5]` m with
divisions `[96, 18, 18]` (103 968 unknowns), otherwise H1's body (both preconditioners, tolerance 1e-10, 60 000
iterations, D5'); limit relative residual `<= 1e-10` each and `>= 1e5` unknowns. H1 and H1b stay as ignored records (H1b
renamed `h1b_heavy_cantilever_wrong_geometry_record`, body unchanged). The fast, slender and other checks use the
geometry registered for them and are not affected.

The decision rule is unchanged (A3 missed: **partial**).

## Addendum 1: results (2026-11-12)

Code: registration 0ae93640; library and tests e8e8b82a (`crates/rcfd-linalg/src/precond.rs` lines 250 to 686 inserted
before the test module, no existing line changed, no re-export; `crates/rcfd-fem/src/solve.rs`; `crates/rcfd-fem/Cargo.toml`
rayon dev-dependency and its `Cargo.lock` line; `crates/rcfd-fem/tests/sb_pcg.rs`); amendments 796b80fb (1), f8b8b03d (2),
cde1ac9a (3), 329c3cad (4), each committed before its run, with their code and tests at 6406bee9 (D5', N1b, the guard,
H1b), e0d9dd45 (B2), ee25672a (H1c). Release profile throughout. Runs, each made once: run 1 (`run1.log`, fast tests), H1
(`run-h1.log`), run 2 after amendments 1 and 2 (`run2.log`: N1b, the guard, the unchanged fast tests reproduced), H1b
(`run-h1b.log`), B2 (`run-b2.log`), H1c (`run-h1c.log`); logs in `w49i-work/f381-scratch`.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| A1 fast (4704 unknowns), relative residual | `<= 1e-10` | block Jacobi 9.64e-11 (156 iterations, 0 restarts); IC 9.54e-11 (85, 0 restarts, shift 0, no breakdown) | yes |
| A2 fast, against the direct solution | `<= 1e-7` | 3.16e-13; 3.31e-13 | yes |
| A3 fast, iterations IC / block Jacobi | `<= 0.5` | 85 / 156 = **0.5449** | **no** (known miss, amendment 1) |
| A4 1 and 8 threads | bit-equal | both: iterations 156 / 156 and 85 / 85, every displacement bit-equal | yes |
| A5 `K4` | as stated | (a) row 3, pivot -4.999999999999999 (1.8e-16), shift 0; (b) 9 breakdowns, shift 0.256 bit-equal to `1e-3 2^8`; (c) pivots within 5.9e-17 of the closed form; (d) all positive | yes |
| A6 forced breakdowns (tolerance 0.9) | as stated | "cg-ic0, shift 2.048e0 after 12 breakdown(s)"; first at node 1 component 2 (pivot 2.077e10, 0.884 of 2.350e10); 240 iterations, residual 7.88e-11, 5.46e-13 from direct, all finite | yes |
| A7 defaults | equal / pass | (a) `rcfd-fem` sb_elements 8, sb_loads 7, sb_modal 8, sb_model 11, sb_recover 9, sb_static 9 pass unedited; (b) direct `21c39eb0c554d5a4`, CG-Jacobi (162 iterations) `4035e2fb5d4197f5` on main 861e4bd5 and on this branch; (c) baseline_hashes 3 passed (1 ignored, as on main) | yes |
| H1 heavy (registered run) | `<= 1e-10` | not converged: 1.17e-8 (block Jacobi, 6530 iterations), 7.5e-9 (IC, 2540); on a 24 m beam by a test-helper defect (amendment 4) | record |
| H1b (amendment 2, D5') | `<= 1e-10` | not converged, 1.9e-9 both (6782 and 2358 iterations); same wrong geometry | record |
| H1c (amendment 4), 103 968 unknowns | `<= 1e-10` | block Jacobi **7.59e-11** (477 iterations, 1 refinement step, 1.9 s); IC **7.21e-11** (255, 1 step, 4.4 s), no breakdown; tip deflection 4.608e-3 m | yes |
| B1 node blocks | 1568 of 3 | 1568 blocks of 3, one node each | yes |
| B2 (amendment 3) | as stated | 1617 blocks, sizes 2, 3, 3, 3, 3, 1, 3, 3, 3, 2, 3, then 3 | yes |
| L1 full pattern exact | `<= 1e-12` | 7.4e-17 | yes |
| L2 IC(0) against ILU(0) | `<= 1e-13` | 4.0e-16, shift 0 | yes |
| L3 block Jacobi against cofactor inverses | `<= 1e-14` | 1.3e-16 | yes |
| L4 refusals | as stated | missing diagonal row 1; "row 1: diagonal 0e0 is not positive"; NaN at (0, 1); indefinite block 0 at row 1 (pivot -3); invalid partition; `NoShift` after 3 attempts | yes |
| L5 five iterations | error | "not converged in 5 iterations and 0 restart(s) (true residual ..., target 3.4375e-5 = 1e-10 relative)", both | yes |
| N1 tolerance 1e-4 | `> 1e-7` | 2.2e-8 | **failed** (set-up defect, amendment 1) |
| N1b tolerance 1e-1 | `> 1e-7` | 2.3e-5 (103 iterations) | yes |
| N2 IC of `A + diag(A)` | `> 0.5` | 192 / 156 = 1.23 | yes |
| N3 block Jacobi against IC solution | not bit-equal | not equal | yes |
| guard (amendment 1) | run 1's numbers | 9.639e-11 / 9.538e-11, 3.160e-13 / 3.309e-13, 156 / 85, A6 and N2 identical | yes |

**Record X1** (no limit). Direct solve: relative residual 6.5e-12 (fast, 0.05 s), 7.1e-11 (slender `[64, 6, 6]`, L/h =
10.7, 9408 unknowns). Slender: block Jacobi 292 iterations, 1 restart, 9.46e-11, 3.9e-12 from direct; IC 206 iterations, 1
restart, 7.97e-11, 1.0e-11 from direct; ratio 0.705. Heavy (H1c): ratio 0.535. The default IC had no breakdown on any
cantilever. Scratch diagnosis of amendment 4 (`diag.log`): the componentwise floor `eps || |A| |x| || / ||b||` is 1.3e-11
(fast), 1.4e-10 (slender), 5.0e-9 (the 24 m beam); the direct solve's residual sits at it, so ADR 0277's 6.7e-11 on a slender
Tet10 cantilever is likely the evaluation floor of the residual rather than an inaccuracy that refinement could remove
(relevant to F-455; Tet10 not measured here).

**Diagnosis of A3.** Block Jacobi hardly improves on scalar Jacobi here (156 against 162 iterations of the existing
`ConjugateGradient`), and IC(0) reduces the count by a factor 1.84 on the fast case and 1.87 on the heavy one (if the count
goes as `sqrt(kappa)`, `kappa` by about 3.4; a factor 2 needs 4). The count is set by the low bending modes of the beam,
which a one-level, local preconditioner leaves in place: with slenderness the ratio moves towards 1 (0.705 at L/h = 10.7).
The draft factor 0.5 was the plan's least trusted number (ADR 0270 section 6); arithmetic 8 took it from experience, not
from a derivation. Nothing was tried to reach it (no ordering, no fill level), by the rules.

**Mutations** (scratch `git archive` of ee25672a, own target; `mutate.ps1`, `mutations.log`; each alone, `sb_pcg` with
`--include-ignored` except the two wrong-geometry heavy records, files restored and compared by SHA-256; the unmutated run
fails only the known-miss record):

| Id | Caught by |
| --- | --- |
| M1 `l_ik` without the sum | L1 |
| M2 shift not applied | A5 (`NoShift`), the guard (A6) |
| M3 pivot test removed | A5, L4, the guard (A6) |
| M4 backward sweep left out | L1, L2, A4, the guard, H1c |
| M5 block off-diagonals dropped | L3, L4 |
| M6 blocks of three rows regardless of node | B2 only (B1 cannot see it, amendment 3) |
| M7 no restart | H1c (the fast tests do not restart) |
| M8 thread-dependent `dot` | A4 |
| M9 IC dispatched to block Jacobi | the guard (A6: no shift), A4 (N3: the solutions became equal); the first text (`_ if true` arm) did not compile, so the match was redirected to `&CgPreconditioner::BlockJacobi` instead |
| M10 non-finite test removed, pivot test `d != 0` | A5, L4, the guard (A6) |

`cargo fmt -p rcfd-fem --check` and `-p rcfd-linalg --check`: clean. `cargo clippy -p rcfd-linalg -p rcfd-fem --all-targets
-- -D warnings` (debug and release): clean. Shared code: `rcfd-linalg` tests (lib 19, amg_contrast 2, reorder 2, sb_direct
16, sb_eigen 15), `rcfd-fv` lib (104), `amr_solve` (2), `vof_dambreak_amg` (1) and `rcfd-api` `baseline_hashes` (3) pass
unedited in release.

**Deviations.**

1. Amendment 1: A3 missed; N1 was not sensitive (corrected N1b). Amendment 2: H1 missed, diagnosed then as the CG
   rounding gap; the restart became iterative refinement (D5'). Amendment 4 found the real cause of both heavy misses: the
   helper built a 24 m beam, whose evaluation floor 5e-9 is above the limit. D5' stays (it is the sounder restart and it is
   what H1c ran); whether D5 alone would have met H1c was not run. Amendment 3: B2 added so that M6 is seen.
2. Arithmetic 7 used `||A|| ||x||`; the componentwise `|| |A| |x| ||` is the right floor (about four times larger on the fast
   case); arithmetic 8's iteration estimates (thousands) were pessimistic by a factor 10 (477 and 255 on the heavy case).
3. H1c takes 6 s in release; it stays `#[ignore]` as registered.
4. A scratch diagnosis (not a check) was run once between H1b and amendment 4 to find the cause; its file and log are in the
   scratch folder.
5. Not run: the whole workspace (the lead runs the gate); M7 against H1 or H1b (wrong geometry).

**Decision by the registered rule: partial.** A3 (clause c6) is missed: IC(0) needs 0.545 of block Jacobi's iterations, not
at most half. Every other acceptance clause is met (A1, A2, A4 to A7, H1c), B1, B2, L1 to L5 hold, N1b, N2, N3 hold, every
mutation is caught (M6 by B2 of amendment 3).

**Remaining questions.**

1. Clause c6 (half the iterations). Options: (a) keep F-381 partial and add a follow-up that tries an ordering (reverse
   Cuthill-McKee by node) and one level of fill for IC, registered with the same limit; (b) the maintainer changes the
   clause to what a one-level method gives ("fewer iterations than block Jacobi", or "at most 0.6") and accepts the measured
   0.545; (c) drop the clause and let ADR 0305's AMG carry the iteration target. Recommendation: (a), with (c) as the
   fallback if the follow-up also misses; (b) would fit the limit to a result.
2. The relative residual of 1e-10 is near the evaluation floor `eps || |A| |x| || / ||b||` on slender or fine models (1.4e-10
   at L/h = 10.7 with 9408 unknowns). Options: (a) state the floor in the solver's error when the target is below it; (b) a
   residual relative to `|| |A| |x| || + ||b||` (a backward error) as a second statistic. Recommendation: (b), within F-455.
3. `StaticOptions::threads` drives the assembly only; the CG runs in the caller's rayon pool. Options: (a) as is; (b) add
   rayon to `rcfd-fem` and install a pool of that size for the iterative solve. Recommendation: (a); the results do not depend
   on it.

## For the lead: roadmap entries

F-381: proposed status **partial**. Note:

> Structural solver, basic tier (plan ADR 0270). ADR 0282, 2026-11-12: rcfd_linalg::precond (appended, no re-export) BlockJacobi (Cholesky of each diagonal block of a given partition) and IncompleteCholesky (L D L^T with zero fill on the lower CSR triangle; on a pivot not above 1e-10 of its diagonal the factorisation restarts with A + alpha diag(A), alpha <- max(2 alpha, 1e-3), which terminates by diagonal dominance; every breakdown recorded with shift, row and pivot; IcOptions, PrecondError). rcfd_fem::solve: StaticSolver::PreconditionedCg { preconditioner: CgPreconditioner::{BlockJacobi, IncompleteCholesky(IcOptions)}, tolerance, max_iterations } on the true relative residual with up to three iterative-refinement steps, node_blocks, solve_static_report with SolveReport (iterations, restarts, shift, breakdowns by node and component), a shifted factor named in SolverStats ("cg-ic0, shift ... after k breakdown(s)"), non-finite iterates refused. Hex8 cantilever 32 x 6 x 6 (4704 unknowns): relative residual 9.6e-11 (block Jacobi, 156 iterations) and 9.5e-11 (IC, 85), within 3.3e-13 of the direct solution (limit 1e-7), iterations and solutions bit-identical for 1 and 8 threads; IC needs 0.545 of block Jacobi's iterations against the limit 0.5: missed (0.535 on the heavy case, 0.705 at L/h = 10.7; the low bending modes set the count). Heavy 96 x 18 x 18 (103 968 unknowns): 7.6e-11 and 7.2e-11 (limit 1e-10), 477 and 255 iterations (amendment 4; two earlier heavy runs were on a 24 m beam by a test-helper defect). A Kershaw-type matrix breaks down at row 3 with pivot -5 and is factored with shift 0.256 after 9 breakdowns, pivots within 5.9e-17 of the closed form; forced breakdowns on the cantilever are reported with shift 2.048 and solved to 7.9e-11. Default and CG-Jacobi results bit-identical to main, baseline hashes pass; ten mutations caught (M6 after amendment 3).

F-455 (direct-solve residual against slenderness), sentence to append to its note:

> ADR 0282 (2026-11-12): on Hex8 cantilevers the direct solve's relative residual sits at the evaluation floor eps |||A| |x||| / ||b|| (6.5e-12 against a floor of 1.3e-11 at L/h = 5.3, 7.1e-11 against 1.4e-10 at L/h = 10.7), so iterative refinement cannot lower the computed residual there; a backward-error statistic relative to |||A| |x||| + ||b|| would separate the two.

F-386 (ADR 0287, solid case file), sentence to append to its note:

> ADR 0282 (2026-11-12): the static solve's iterative route is StaticSolver::PreconditionedCg with CgPreconditioner::BlockJacobi or CgPreconditioner::IncompleteCholesky(IcOptions::default()); a case-file key for it maps onto these.

New follow-up item. Title: "Incomplete Cholesky at half the block-Jacobi iterations on the elastic cantilever: ordering and one level of fill". Deps: F-381. Acceptance:

> Every limit not stated here is registered in the ADR before the run. Beside IC(0) in the natural order, an incomplete Cholesky with one level of fill and IC(0) after a node-wise reverse Cuthill-McKee ordering, both with the shift strategy of ADR 0282, selectable in StaticSolver::PreconditionedCg: on the fast Hex8 cantilever of ADR 0282 (32 x 6 x 6, 4704 unknowns) at least one of them reaches the relative residual 1e-10 in at most half the iterations of block Jacobi (156), agrees with the direct solution within 1e-7 and is bit-identical for 1 and 8 threads; the iteration counts at L/h = 5.3 and 10.7 and on the 103 968-unknown case are recorded; IC(0) in the natural order and every default are unchanged bit for bit.

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the basic-tier plan, [ADR 0270](0270-structural-solver-basic-tier-plan.md)); the
registration is written by the implementation agent before any code. Nothing here is a registered limit: the numbers below are
the acceptance draft that the registration turns into checks.

Work package SB-12 of the structural solver, basic tier. Size M.

### Goal

The direct solver stops at a few times 1e4 unknowns. Add two preconditioners for the CSR stiffness matrix: a 3 x 3 block Jacobi
and an incomplete Cholesky with zero fill and a diagonal shift when a pivot fails, and make the solver selectable in the static
solve. The default stays the direct solver, bit for bit.

### Dependencies

ADR 0275; existing roadmap items: F-026.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. CG on the stiffness matrix with (a) 3 x 3 block Jacobi and
(b) incomplete Cholesky with zero fill on CSR and a diagonal shift on breakdown, selectable beside the direct solver: on a
cantilever of about 5e3 unknowns (fast) and of at least 1e5 unknowns (heavy record) the relative residual reaches 1e-10; on the
fast case the displacement agrees with the direct solution within 1e-7 relative, incomplete Cholesky needs at most half the
iterations of block Jacobi, and the iteration count and the solution are bit-identical for 1 and 8 threads; a breakdown is
reported with the shift used, never as a non-finite result. The default solver and its results are unchanged bit for bit.

### Reference to check against

The reference is the direct solution. Methods: Meijerink and van der Vorst, Math. Comp. 31 (1977) 148 (incomplete Cholesky);
Manteuffel, Math. Comp. 34 (1980) 473 (the shifted factorisation); both cited from memory.

### Files

`crates/rcfd-linalg/src/precond.rs` (appended types, no re-export in `lib.rs`), `crates/rcfd-fem/src/solve.rs`,
`crates/rcfd-fem/tests/sb_pcg.rs`.

### What could make it miss

The factor of two over block Jacobi is a draft, not a derivation; on slender bodies CG converges slowly whatever the one-level
preconditioner. The real answer (aggregation AMG with the rigid-body modes as near-null space) is left to the intermediate tier.
