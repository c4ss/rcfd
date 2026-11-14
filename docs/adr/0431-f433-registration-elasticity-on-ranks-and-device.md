# ADR 0431: F-433 registration: elasticity solves on the ranks of rcfd-dist and on the CUDA device

Status: **REGISTERED 2026-11-14, written before any code of F-433 and before any run of its checks.** Not yet accepted by the
maintainer. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Parents: [ADR 0336](0336-elasticity-on-ranks-and-on-the-device.md) (the stub, SA-26), [ADR 0310](0310-structural-solver-advanced-tier-plan.md)
(plan; decision A5 (a): `rcfd-api` gains a dependency on `rcfd-dist` and a library module `solid_parallel`),
[ADR 0282](0282-preconditioned-cg-for-elasticity.md) (F-381: CG with block Jacobi and incomplete Cholesky, the serial
reference), [ADR 0275](0275-assembly-displacement-constraints-and-static-solve.md) (F-374: assembly, elimination, the static
solve), ADR 0058 (F-100/F-016 transport: `Communicator`, `ThreadComm`, `run_ranks_with_timeout`, `DistCsr`, deterministic
reductions), ADR 0038 (F-101: `rcfd-gpu`, `resolve`, `AUTO_MIN_UNKNOWNS`, the device CG with Jacobi or Chebyshev). Roadmap item
F-433 (deps F-016, F-101, F-374, F-381), size M. Branch `f433-elasticity-dist-cuda`, worktree `w49i-work/f433`, scratch
`w49i-work/f433-scratch`. House rules of `w49i-work/AGENT_RULES.md`.

## What F-433 asks

The roadmap acceptance, binding:

> Every limit not stated here is registered in the ADR before the run. Ranks: with one rank the iteration counts and the
> solution have the bits of the serial CG of F-381 with the same preconditioner; with 2, 4 and 8 ranks the solution of a
> clamped cube (about 4e4 unknowns) and of a cantilever of Tet10 equals the serial one within 1e-8 relative at a residual of
> 1e-10, in at most twice the iterations of one rank (counts recorded); repeated runs with the same rank count give the same
> bits; the three unknowns of a node are never split; a rank that fails or times out ends the solve with a located error.
> Scaling is recorded, not asserted. Device: with a usable device the device CG equals the CPU CG with the same
> preconditioner within 1e-9 on a cube of at least 3e5 unknowns, times recorded; `auto` below the threshold runs on the CPU;
> without a device `cuda` is a located error and `auto` falls back with the reason recorded, and the device clauses are
> reported as not run. The default solver's results and the baseline hashes are unchanged.

Clauses: (c1) one rank: iteration count and solution bit-equal to F-381's CG with the same preconditioner; (c2) 2, 4, 8 ranks,
clamped cube of about 4e4 unknowns: within 1e-8 relative of the serial solution at residual 1e-10; (c3) the same on a Tet10
cantilever; (c4) iterations at most twice those of one rank, counts recorded; (c5) repeated runs with the same rank count
bit-identical; (c6) the unknowns of a node never split over ranks; (c7) a rank that fails ends the solve with a located error;
(c8) a rank that times out the same; (c9) scaling recorded; (d1) usable device: device CG within 1e-9 of the CPU CG with the
same preconditioner on a cube of at least 3e5 unknowns, times recorded; (d2) `auto` below the threshold runs on the CPU;
(d3) without a device `cuda` is a located error; (d4) without a device `auto` falls back with the reason recorded; (d5) device
clauses reported as not run if there is no device; (b1) the default solver's results unchanged; (b2) baseline hashes unchanged;
(c0) every limit registered here before the run.

## What was read (no run)

`docs/adr/0336-*`, `0310-*` (A5), `0282-*` (D2 to D6, amendment 2 D5'), the roadmap entries of F-016, F-101, F-374, F-381.
Code: `crates/rcfd-dist/src/{comm,halo,matrix,krylov,solve}.rs`; `crates/rcfd-gpu/src/{lib.rs,cuda/mod.rs,cuda/system.rs}`;
`crates/rcfd-linalg/src/{solvers.rs,vecops.rs,matrix.rs (CsrMatrix::matvec, residual),backend.rs,precond.rs (BlockJacobi,
IcOptions, IncompleteCholesky)}`; `crates/rcfd-fem/src/solve.rs` (`solve_static_report`, `preconditioned_cg`, `node_blocks`,
`residual_norm`), `constraint.rs` (`prescribed_equations`, `check_rigid_body_motions`, `reduce`, `ReducedSystem`,
`reactions`), `builders.rs` (`hex_box`, `tet_box`, `upgrade_to_tet10`), `crates/rcfd-fem/tests/sb_pcg.rs` (set-up of F-381),
`crates/rcfd-fv/src/pressure.rs` (`device_solve`, the fall-back pattern), `crates/rcfd-cli/tests/workspace_graph.rs` (layers:
`rcfd-dist`, `rcfd-fem` at 2, `rcfd-api` at 5). `nvidia-smi` on this machine reports an RTX 5080 (driver 621.14, CUDA 13.5)
and `nvcc` 13.4 is installed, so the device clauses are expected to run (with `--features cuda`); if `probe()` reports the
device unavailable at run time they are reported as not run.

## Arithmetic before this registration (disclosed, by hand)

1. **Sizes.** `hex_box` with `n` divisions per side and the face `xmin` clamped has `(n+1)^2 n` free nodes, 3 unknowns each.
   `n = 23`: `576 * 23 * 3 = 39 744` (about 4e4, clause c2). `n = 46`: `2209 * 46 * 3 = 304 842` (at least 3e5, clause d1).
   `n = 12`: `169 * 12 * 3 = 6 084` (the fast device cube, below `AUTO_MIN_UNKNOWNS = 100 000`). Tet10 cantilever:
   `tet_box([0,0,0], [8, 1.5, 1.5], [16, 3, 3])` upgraded by `upgrade_to_tet10`: 272 corner nodes and about 1.3e3 edges
   (seven edges per Kuhn cell plus the boundary ones), so about 1.6e3 nodes and 4.5e3 free unknowns.
2. **Why the one-rank bits can be equal (c1).** `rcfd_dist::krylov::cg` is `rcfd_linalg::solvers::cg` operation for operation
   with `dot` replaced by `allreduce_sum(dot)`, and `allreduce_sum` returns its argument unchanged for one rank
   (`comm.rs`). `DistCsr::matvec` is a halo exchange (a no-op without peers) followed by `CsrMatrix::matvec` of the local rows,
   whose per-row sum runs over the stored entries in order; if the local rows keep the global entry order (columns only
   renumbered), every row sum has the serial bits for any rank count. The preconditioners of F-381 applied to the owned
   block: block Jacobi on node blocks only reads the entries of each node's diagonal block (`precond.rs` lines 382-420), so
   it is the same operator for every rank count; IC(0) of the owned block equals IC(0) of the whole matrix for one rank. The
   outer loop of F-381 (target `tolerance |b|` on the true residual, up to `MAX_RESTARTS = 3` refinement restarts from a
   zero correction, the non-finite check) is reproduced with every norm a global reduction. So for one rank the arithmetic
   is the serial arithmetic, bit for bit; the limit is exact equality.
3. **What differs on 2 to 8 ranks.** Only the inner products: `dot` sums fixed chunks of 4096 owned entries then the chunk
   partials; with P ranks the partials are per rank and summed in rank order. Each inner product changes by a few ulps;
   CG amplifies such perturbations over hundreds of iterations, so the iterates differ, but each run converges to the true
   residual 1e-10 on its own. With block Jacobi the preconditioner is identical, so the iteration count changes by at most
   a few (estimate: ratio within 1.00 +- 0.05). The solution difference: both runs have relative residual below 1e-10; the
   error of each is `K^-1 r`; F-381 measured 3.3e-13 between CG at 1e-10 and the direct solution on its cantilever, so the
   difference between two CG runs is expected near 1e-12, four orders inside 1e-8.
4. **Rank-local incomplete Cholesky (additive Schwarz without overlap, Saad, Iterative Methods for Sparse Linear Systems,
   chapter 14, cited from memory; only the definition is used: the preconditioner is the direct sum of the IC(0) factors of
   the owned diagonal blocks, couplings to other ranks dropped).** Dropping the coupling across the cut weakens the
   preconditioner, so the count grows with P. Estimate (not a derivation, a judgement from the one-level Schwarz bound
   that grows like `1/(H h)` against IC's `1/h^2` with a better constant): ratio 1.1 to 1.7 at P = 8 on the cube, smaller on
   the slender cantilever, whose cuts are across the length. The limit is the acceptance's 2.
5. **Device (d1).** The device CG (`DeviceSystem::cg`) has the recurrences of `rcfd_linalg::cg` with device reductions in a
   different order; Jacobi on the device multiplies by `1/d`, on the CPU (`Jacobi`) it is the same `1/d` or a division
   (roundings differ by an ulp). Both stop on the true residual 1e-10 through the same outer loop; by the argument of 3
   their difference is expected near 1e-12 relative, inside 1e-9. Iterations of Jacobi-CG on the 46^3 cube: the condition
   number of the Jacobi-scaled stiffness grows like `n^2`; the F-381 cantilever needed 156 block-Jacobi iterations at
   32 x 6 x 6; I expect 500 to 1500 at 46^3. CPU cost about 2.5e7 stored entries per product, tens of milliseconds per
   iteration with 16 threads, so the CPU solve is 10 to 60 s; together with the assembly of 97 336 Hex8 elements the record
   may exceed two minutes, so it is an ignored heavy record (`*_heavy`) run once, with a fast device test (12^3) on the
   same code path.
6. **Time-out (c8).** With a receive time-out of 0.5 s and a rank that stalls 3 s before a send, the other ranks fail after
   0.5 s, end, and the stalled rank's send then fails with "peer has terminated"; the solve returns in about 3 s. Limit:
   an error within 10 s.

## Design

### D1. Where the code lives

`crates/rcfd-api/src/solid_parallel.rs` (new), one `pub mod solid_parallel;` line in `crates/rcfd-api/src/lib.rs`, and in
`crates/rcfd-api/Cargo.toml` the dependencies `rcfd-dist` (decision A5) and `rcfd-fem` (needed for the model; `rcfd-api`
reaches it today only through `rcfd-post`; both are at layer 2, `rcfd-api` at 5, so `workspace_graph` allows it). No file of
`rcfd-fem`, `rcfd-dist`, `rcfd-gpu` or `rcfd-linalg` is changed; no solver default is touched. Tests:
`crates/rcfd-api/tests/sa_solid_parallel.rs` (ranks and no-device clauses, default build) and
`crates/rcfd-api/tests/sa_solid_device.rs` (device clauses, run with `--features cuda`). No unsafe code: the device is
reached only through `rcfd_gpu::resolve` and `LinearBackend::cg`, whose unsafe stays in `rcfd-gpu/src/cuda/launch.rs`
under its existing allowance (ADR 0038 D2).

### D2. The system

`solve_static_ranks(model, prescribed, forces, &RankOptions)` and `solve_static_device(model, prescribed, forces,
&DeviceOptions)` take the steps of `rcfd_fem::solve::solve_static_report` up to the reduced system, by the public functions:
`validate`, `check_supported_with_beams`, refusal of transformation-path models (`mpc::uses_transformation`: a located
`FemError::Solver` naming the reason), `nodal_load_vector`, `prescribed_equations`, `check_rigid_body_motions`,
`stiffness_with(model, threads)`, `reduce`. Assembly stays serial (the stub). After the solve: `expand`, `reactions`,
`SolidResult::new(..).with_reactions(..).with_stats(SolverStats::new(name, nf, iterations, relative_residual))` with the
relative residual by the formula of `solve.rs::residual_norm` (plain sequential sums), so that a one-rank result has the
serial bits in every field but the name.

### D3. The node split over ranks (c6)

`node_partition(free, dofs, nodes, p)`: recursive coordinate bisection of the nodes that have free equations, the rule of
`rcfd_mesh::partition::Rcb` (ADR 0058/F-016) applied to node points: split along the longest extent of the bounding box,
nodes sorted by that coordinate with ties by node id, the left part getting `round(count * floor(p/2) / p)` nodes. It
returns the rank of every free equation, all equations of a node on the node's rank, so a node is never split by
construction. `RankSystems::build(a, free, dofs, part_of_equation, p)` builds one `DistCsr` per rank and checks the split
independently of how the assignment was made: two free equations of one node on different ranks are refused with a located
error naming the node, both components and both ranks (negative control N1 feeds it a split assignment). Owned rows in
ascending reduced order; ghosts are the non-owned columns, grouped by owning rank in ascending rank order and ascending
reduced index within a rank; the halo plan's send list of rank q to rank r is the list of q's local ids of r's ghosts owned
by q, in the same order, so `HaloPlan::exchange` fills them. Each local row keeps the global entry order with columns
renumbered (arithmetic 2). A rank with no equations is refused (more ranks than nodes).

### D4. The rank solve

`RankOptions { ranks, preconditioner: RankPreconditioner::{BlockJacobi, IncompleteCholesky(IcOptions)}, tolerance,
max_iterations, timeout, threads, fault }`. Each rank runs on a `ThreadComm` from `rcfd_dist::run_ranks_with_timeout(p,
timeout, ..)` (also for p = 1), wrapped in a `FaultComm` that forwards to it. Per rank: `DistOperator`, the preconditioner on
the owned block (`DistCsr::owned_block`): `rcfd_linalg::precond::BlockJacobi::new(owned, local node blocks)` or
`IncompleteCholesky::new(owned, options)`; then the outer loop of F-381 (D5' of ADR 0282 amendment 2): target `tolerance *
gnorm2(b)`, `rcfd_dist::krylov::cg` with `tol_abs = target, tol_rel = 0`, restarts from a zero correction of the true residual
up to `rcfd_fem::solve::MAX_RESTARTS`, a non-finite iterate refused (decided by a global maximum so every rank takes the same
branch), the true residual `b - A x` by `DistOperator::try_apply` and `gnorm2`. Every decision is taken from globally reduced
numbers, so all ranks agree; the iteration counts of all ranks are compared and a disagreement is an error. The owned
solutions are gathered into the reduced vector. `RankReport`: ranks, iterations, restarts, relative residual, per rank the
owned equations, owned nodes, ghosts and the IC shift, the rank of every free equation, solve seconds (maximum over ranks,
after the matrices are built), assembly seconds.

### D5. Failures (c7, c8)

Every rank's result is collected; if any rank returned an error, the solve returns `FemError::Solver { solver:
"cg-ranks", reason }` whose reason lists each failing rank in rank order with its `CommError` (which names the rank, the
peer, the tag and the reason) or its preconditioner error; it never returns a partial solution. A panic on a rank is
propagated by `run_ranks` (not caught). The test hook `RankFault::{Fail { rank, after_sends }, Stall { rank, after_sends,
delay }}` (public, documented as a test hook; `None` by default): the `FaultComm` of that rank returns a `CommError` with
reason "injected failure" at its `after_sends`-th send, or sleeps `delay` before it. The time-out is `RankOptions::timeout`
(default `rcfd_dist::comm::DEFAULT_TIMEOUT`, 60 s).

### D6. The device solve

`DeviceOptions { device: LinearDevice, preconditioner: DevicePreconditioner::{Jacobi, Chebyshev { degree }}, tolerance,
max_iterations, threads }`. `rcfd_gpu::resolve(device, nf)` (threshold `rcfd_gpu::AUTO_MIN_UNKNOWNS = 100 000` on the free
unknowns, ADR 0038 D6, not tuned here): `cpu` runs `rcfd_linalg::CpuBackend`; `cuda` without a device is
`FemError::Solver { solver: "cg-device", reason }` with the discovery reason of `resolve` (which names the failing step);
`auto` without a device or below the threshold runs the CPU backend and records `resolve`'s reason. The same outer loop as D4
(serially, `CsrMatrix::residual`, `norm2`) around `LinearBackend::cg(a, rhs_or_residual, x, control, LinearOptions { device,
precond, chebyshev_degree })`. Under `auto`, a device solve that fails falls back to the CPU backend from the start, the
failure noted by `rcfd_gpu::note_fallback` and in the report's reason; under `cuda` it is a located error.
`DeviceReport`: backend (`"cuda"` or `"cpu"`), reason, iterations, restarts, relative residual, solve seconds (wall time of
the outer loop including uploads), assembly seconds.

### D7. Test set-ups

Steel, `E = 2e11`, `nu = 0.3`. **Clamped cube**: `hex_box([0;3], [1;3], [23;3])`, `xmin` clamped in all three components,
traction `[2e6, 1e6, -1.5e6]` on `xmax` (all three components loaded). **Tet10 cantilever**: arithmetic 1 mesh, `xmin`
clamped, traction `[0, 0, -1e6]` on `xmax`. **Device cubes**: the clamped cube with 46 (heavy) and 12 (fast) divisions.
Serial references: `rcfd_fem::solve::solve_static_report` with `StaticSolver::PreconditionedCg { BlockJacobi |
IncompleteCholesky(IcOptions::default()), tolerance 1e-10, max_iterations 20 000 }`. Rank runs: the same tolerance and
cap, time-out 60 s except in F2. Device runs: tolerance 1e-10, cap 50 000, `DevicePreconditioner::Jacobi`; the CPU
comparison is `solve_static_device` with `device = cpu` and the same preconditioner. "Relative difference" of two solutions
is `max |u_a - u_b| / max |u_b|` over all displacement values (the measure of `sb_pcg.rs`).

## Registered checks

| # | Clause | Check | Limit | Estimate |
|---|---|---|---|---|
| R1 | c1 | cube, block Jacobi, 1 rank vs serial F-381 block Jacobi: iterations, and every displacement value, reaction and the relative residual by bits | exact | exact (arith. 2) |
| R2 | c1 | Tet10 cantilever, block Jacobi, 1 rank vs serial | exact | exact |
| R3 | c1 | cube, IC(0) default options, 1 rank vs serial F-381 IC(0) (iterations, displacements by bits, the shift equal) | exact | exact |
| R4 | c2, c4 | cube, block Jacobi, P = 2, 4, 8: relative difference to the serial block-Jacobi solution; relative residual; iterations / one-rank iterations | <= 1e-8; <= 1e-10; <= 2 | ~1e-12; <1e-10; 1.00 +- 0.05 |
| R5 | c3, c4 | Tet10 cantilever, block Jacobi, P = 2, 4, 8: the same three | same | same |
| R6 | c2, c3, c4 | cube and Tet10 cantilever, rank-local IC(0), P = 2, 4, 8: difference to the serial IC(0) solution, residual, iterations / one-rank IC(0) iterations | <= 1e-8; <= 1e-10; <= 2 | ~1e-12; <1e-10; 1.1 to 1.7 |
| R7 | c5 | each run of R4 and R5 and the P = 8 runs of R6 repeated once: iterations and displacements by bits | exact | exact (fixed reduction order) |
| R8 | c6 | for every run of R4 to R6, from the report: every node's free equations on one rank, the owned equation counts sum to the free count, every rank owns at least one node | exact | exact (D3) |
| R9 | c7 | Tet10 cantilever, P = 4, block Jacobi, `RankFault::Fail { rank: 1, after_sends: 200 }`: `Err(FemError::Solver)` whose text contains "rank 1" and "injected failure"; elapsed | Err; text; <= 10 s | about 0 s |
| R10 | c8 | same, `RankFault::Stall { rank: 2, after_sends: 200, delay: 3 s }`, time-out 0.5 s: `Err`, text contains "no message within" and names a rank; elapsed | Err; text; <= 10 s | about 3 s |
| R11 | c9 | solve seconds of R4 (cube, block Jacobi) for P = 1, 2, 4, 8 printed | recorded | - |
| D1 | d1 | heavy, device available: cube 46^3 (304 842 unknowns), `auto`: backend `cuda`; relative difference to `device = cpu`; both relative residuals; times | `cuda`; <= 1e-9; <= 1e-10; recorded | -; ~1e-12; -; - |
| D1f | d1 (fast path) | device available: cube 12^3, `cuda` Jacobi vs `cpu` Jacobi; and `cuda` Chebyshev (degree 0 = default) vs `cpu` Chebyshev | <= 1e-9 each | ~1e-12 |
| D2 | d2 | device available: cube 12^3 (6 084 < 1e5), `auto`: backend `cpu`, reason contains "below the threshold"; displacements bit-equal to `device = cpu` | exact | exact |
| D3 | d3 | device unavailable (default build without the feature `cuda`, which `probe()` reports unavailable): cube 6^3, `cuda` is `Err(FemError::Solver)` whose text contains "cuda" and the reason of `probe()` | text | - |
| D4 | d4 | same build: `auto`: Ok, backend `cpu`, reason contains "falling back" and the reason of `probe()`; displacements bit-equal to `device = cpu` | exact | exact |
| D5 | d5 | if `probe()` reports unavailable in the `--features cuda` run, D1, D1f, D2 print "NOT RUN" with the reason and the clauses are reported not run | reported | - |
| B1 | b1 | `git diff` of this item touches no file under `crates/rcfd-fem`, `rcfd-linalg`, `rcfd-dist`, `rcfd-gpu`, `rcfd-fv`; the fast tests of `crates/rcfd-fem/tests/sb_pcg.rs` and the `rcfd-dist` tests pass | exact | - |
| B2 | b2 | `cargo test -p rcfd-api --release --test baseline_hashes` passes | pass | pass |
| B3 | (graph) | `cargo test -p rcfd-cli --test workspace_graph` passes (acyclic, layers) | pass | pass |

D3 and D4 run genuinely without a device: the default build of `rcfd-api` has no device backend (`rcfd-gpu` without the
feature), which is the "without a device" situation of the acceptance as the program sees it. They are also run once in the
`--features cuda` build with `RCFD_GPU_DISABLE=1` (recorded, same limits).

## Negative controls and planned mutations

Negative controls: **N1** `RankSystems::build` with an assignment that puts component 2 of one node on another rank is
refused, the text naming that node (exact); **N2** cube, P = 2, block Jacobi at tolerance 1e-4: its relative difference to
the serial 1e-10 solution is above 1e-8 (shows R4 discriminates; estimate 1e-5 to 1e-3); **N3** fast device cube: `cpu`
Jacobi at tolerance 1e-4 against `cuda` (or, without a device, `cpu`) at 1e-10 differs by more than 1e-9 (shows D1
discriminates). **N4** a rank fault on a rank number not below P (`rank: 9`, P = 4) has no effect: the solve succeeds with
the bits of the fault-free run (the hook is inert unless aimed).

Mutations, run once each on a scratch `git archive` copy of the test commit under `w49i-work/f433-scratch`, the two new
test files run in release after each (the device file with `--features cuda`); "caught" means at least the named checks
fail:

| Mutation | Change | Must be caught by |
|---|---|---|
| M1 | the global reduction dropped from the outer loop's target (`gnorm2` replaced by the local `norm2`) | R4 or R5 or R6 (multi-rank residuals) |
| M2 | ghosts not exchanged: the halo plans built with empty send and receive lists | R4, R5 |
| M3 | `node_partition` assigns equations in contiguous chunks of the free equations, ignoring nodes | R8 |
| M4 | the node-split check of `RankSystems::build` removed | N1 |
| M5 | `RankOptions::timeout` ignored (`run_ranks` with the 60 s default) | R10 (elapsed) |
| M6 | rank errors ignored when rank 0 succeeded (the gather keeps only rank 0's result status) | R9 or R10 |
| M7 | the rank preconditioner block Jacobi replaced by point Jacobi on the owned diagonal | R1, R2 |
| M8 | `auto` ignores the threshold (`resolve` called with `usize::MAX` unknowns) | D2 |
| M9 | `cuda` without a device silently runs on the CPU | D3 |
| M10 | the restart branch of the rank outer loop adds the correction twice | R1 or R2 or R3 if a restart happens; if no run restarts, recorded as not caught with that cause |

## Decision rule

**done** if every check of the table is met, N1 to N4 behave as stated, and every mutation is caught by the checks named
(M10 excepted only if no registered run restarts, recorded as such); **partial** if a registered check missed, a mutation
was not caught, a check was met only after an amendment written after its run, or a clause was not run (in particular the
device clauses D1, D1f, D2 if no device is usable); a miss is kept as an ignored `*_known_miss` test with the limit intact
and the measured number recorded.

## Limits (what is not covered)

In-process ranks only (threads with channels, ADR 0058); MPI is F-100's later transport and not structural work. Assembly
is serial and every rank's rows are cut from the global matrix (no distributed assembly). Only block Jacobi and rank-local
IC(0) on ranks; no AMG, no fill levels, no overlap. Models on the transformation path (linear constraints, rigid sets,
elastic supports) are refused by both new entry points. The device solves the reduced system uploaded per CG call (no
resident matrix across restarts); Chebyshev is checked only on the fast cube. Scaling is recorded, not asserted, on one
machine shared with other building agents. No case-file keys, CLI, schema or MCP surface (A5 (a): library only), so no
golden changes.

## Amendment 1 (2026-11-14, written and committed before any mutation run)

The registered checks R1 to R11, D1 to D4 and N1 to N4 have been run once (results in addendum 1). Before running the
mutations I checked mutation M3 by arithmetic: the free-equation counts are 39 744 (cube) and 4 704 (Tet10 cantilever,
measured in the R2 run), and 39 744 / P and 4 704 / P are multiples of 3 for P = 2, 4, 8 (19 872, 9 936, 4 968; 2 352, 1 176,
588). Contiguous equal chunks of the free equations therefore end on node boundaries (each node's three free equations are
consecutive), so M3 as registered splits no node on these models: it is an equivalent mutant for clause c6, a defect of the
mutation set-up, not of the check. It is replaced, before any mutation run, by **M3'**: equations assigned by
`g * p / (n + 1)` (chunk boundaries at `k (n + 1) / p`, which for these `n` fall inside a node, for example 19 873 = 3 * 6 624
+ 1 for P = 2 on the cube), ignoring nodes. Must be caught by R8 or by the build's own split refusal (which then fails R4 to
R6 with "is split"). M3 as registered is not run; nothing else changes.

## Addendum 1: results (2026-11-14)

Runs, each made once, in release, on the code of commit 074edbfc: `cargo test -p rcfd-api --release --test
sa_solid_parallel -- --test-threads=1` (default build, no device backend); `cargo test -p rcfd-api --release --features cuda
--test sa_solid_device -- --test-threads=1` and the same with `--ignored` for D1 (RTX 5080 found by `probe()`); the D3/D4
record in the `--features cuda` build with `RCFD_GPU_DISABLE=1`. Logs in `w49i-work/f433-scratch`.

| Check | Limit | Measured | Met |
|---|---|---|---|
| R1 cube (39 744 unknowns), block Jacobi, 1 rank vs F-381 | bits | 276 = 276 iterations, 0 restarts both; displacements, reactions, residual (8.560e-11) bit-equal | yes |
| R2 Tet10 cantilever (4 704 unknowns), block Jacobi, 1 rank | bits | 581 = 581 iterations, 1 restart both; all bit-equal | yes |
| R3 cube, IC(0), 1 rank | bits | 94 = 94 iterations, shift 0 both; all bit-equal | yes |
| R4 cube, block Jacobi, P = 2 / 4 / 8: difference; residual; iteration ratio | 1e-8; 1e-10; 2 | 7.9e-15 / 5.6e-15 / 1.9e-14; 8.57e-11 / 8.56e-11 / 8.54e-11; 276 of 276 = 1.000 at every P | yes |
| R5 Tet10, block Jacobi, P = 2 / 4 / 8 | same | 1.4e-13 / 6.3e-14 / 2.1e-13; 6.6e-11 / 9.5e-11 / 6.8e-11; 583, 581, 582 of 581 (1.0034, 1.0000, 1.0017) | yes |
| R6 cube, rank-local IC(0), P = 2 / 4 / 8 | same | 1.2e-11 / 1.1e-11 / 1.4e-11; 9.5e-11 / 9.0e-11 / 7.8e-11; 122, 132, 143 of 94 (1.298, 1.404, 1.521) | yes |
| R6 Tet10, rank-local IC(0), P = 2 / 4 / 8 | same | 2.7e-13 / 4.5e-13 / 4.4e-13; 8.4e-11 / 6.9e-11 / 8.8e-11; 190, 202, 219 of 180 (1.056, 1.122, 1.217) | yes |
| R7 repeats (R4, R5 at 2, 4, 8; R6 at 8) | bits | all bit-identical | yes |
| R8 node split, every run of R4 to R6 | exact | no node split; counts sum; owned equations equal per rank (19 872 / 9 936 / 4 968; 2 352 / 1 176 / 588) | yes |
| R9 failing rank 1 of 4 | Err naming rank 1, "injected failure"; <= 10 s | 0.015 s; "4 of 4 rank(s) failed: ... rank 1: communication error on rank 1 with peer 2 (tag 1): injected failure at send 200" | yes |
| R10 stalling rank 2 of 4, time-out 0.5 s | Err "no message within"; <= 10 s | 3.015 s; ranks 0, 1, 3 "no message within 500ms" from peer 2, rank 2 "peer has terminated" | yes |
| R11 scaling, cube block Jacobi, solve seconds P = 1 / 2 / 4 / 8 | recorded | 0.167 / 0.142 / 0.127 / 0.116 s (serial assembly 0.107 s) | recorded |
| D1 cube 46^3 (304 842 unknowns), `auto` | `cuda`; 1e-9; 1e-10 | backend cuda ("auto: cuda, 304842 unknowns"); difference 2.1e-13; residuals 9.44e-11 (device) / 9.53e-11 (cpu); 559 iterations both; device 0.596 s, cpu 4.504 s (16 threads), assembly 0.78 s | yes |
| D1f cube 12^3, `cuda` vs `cpu` | 1e-9 | Jacobi 4.5e-14 (146 iterations both, 0.022 s / 0.030 s); Chebyshev 4.5e-15 (54 both) | yes |
| D2 `auto` below the threshold | cpu, reason, bits | backend cpu, "auto: cpu, 6084 unknowns is below the threshold 100000", bit-equal to cpu | yes |
| D3 `cuda` without a device | located Err | "solver cg-device: linear_solver.device = "cuda" but no device is usable: built without feature cuda (rebuild with --features cuda)"; with `RCFD_GPU_DISABLE=1`: "... disabled by RCFD_GPU_DISABLE=1" | yes |
| D4 `auto` without a device | cpu, reason, bits | backend cpu, "auto: falling back to cpu, built without feature cuda ..." (and "... disabled by RCFD_GPU_DISABLE=1"), bit-equal to cpu | yes |
| D5 device clauses not run | reported | the device was usable; every device clause ran | n/a |
| B1 no file of rcfd-fem, rcfd-linalg, rcfd-dist, rcfd-gpu, rcfd-fv changed; `sb_pcg` fast tests; `rcfd-dist` tests | pass | the diff touches only `crates/rcfd-api` (Cargo.toml two lines, lib.rs one line, the new module, two new tests), `Cargo.lock` and this ADR; sb_pcg 13 passed (4 ignored as before); rcfd-dist 12 passed | yes |
| B2 `baseline_hashes` | pass | 3 passed, 1 ignored (as before) | yes |
| B3 `workspace_graph` (rcfd-cli) | pass | 4 passed | yes |
| N1 split assignment | refused naming the node | "node 1 is split: component 0 on rank 0, component 2 on rank 1" | yes |
| N2 P = 2 at 1e-4 | > 1e-8 | 6.1e-6 | yes |
| N3 cpu at 1e-4 vs cuda at 1e-10 | > 1e-9 | 5.2e-6 | yes |
| N4 fault aimed at rank 9 of 4 | bits of the clean run | bit-equal | yes |

Mutations (scratch `git archive` copy of commit 93bee490, its own target directory, the test files run once per mutant;
M8 and the second half of M9 with `--features cuda`):

| Mutation | Failed checks | Expected | Caught |
|---|---|---|---|
| M1 local norm for the target | R4, R5, both R6, N4 (rank solves end in errors at P >= 4) | R4 or R5 or R6 | yes |
| M2 no ghost exchange | R4, R5, both R6 (difference 0.9, residual up to 8) | R4, R5 | yes |
| M3' chunks at `g p / (n + 1)` (amendment 1) | R4, R5, R6, R9, R10, N1, N2, N4 (all refused "node ... is split") | R8 or the split refusal | yes |
| M4 split check removed | N1 | N1 | yes |
| M5 time-out ignored | R10 (Ok after 3.07 s: the stalled rank resumed within the 60 s default) | R10 | yes |
| M6 only rank 0's status kept | R9 (text lacks "injected failure") | R9 or R10 | yes |
| M7 point Jacobi for block Jacobi | R1 (280 against 276 iterations), R2 (624 against 581) | R1, R2 | yes |
| M8 threshold ignored | D2 (backend cuda) | D2 | yes |
| M9 `cuda` silently downgraded | D3 (default build); D1f (`--features cuda`: backend cpu) | D3 | yes |
| M10 correction added twice | R2 (the Tet10 runs restart once), R5 | R1 or R2 or R3 | yes |

Deviations. (1) Mutation M3 was replaced by M3' in amendment 1 before any mutation run, because M3 is an equivalent mutant
on these sizes (arithmetic in the amendment); M3 itself was not run. (2) D1 was registered as an ignored heavy record from
the estimate of arithmetic 5 (500 to 1500 iterations, possibly over two minutes); it took 559 iterations and 7.0 s in all.
It stays ignored as registered; un-ignoring it is remaining question 1. (3) The Tet10 cantilever has 4 704 free unknowns
(estimate 4.5e3). (4) Arithmetic 4 estimated a rank-local IC(0) ratio of 1.1 to 1.7 at P = 8 on the cube: measured 1.52.
No limit, test or parameter was changed after a run.

`cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets --release -- -D warnings` clean, also with
`--features cuda`. Not run: the workspace suite (the lead's gate) and the other rcfd-api tests (no existing line of
rcfd-api changed except one `pub mod` line and two dependency lines).

**Decision by the registered rule: done.** Every registered check met on its single run, N1 to N4 as stated, every
mutation caught; the only amendment (M3 to M3') was written before any mutation run and changes no check or limit; the
device clauses ran on a usable device.

### Remaining questions

1. D1 runs in 7 s, not minutes. Options: (a) leave it ignored as registered; (b) a follow-up un-ignores it (it still needs
   `--features cuda` and skips without a device). Recommendation: (b), at the lead's convenience.
2. Case-file keys for the rank and device solves of a solid (A5 (c) of ADR 0310). Options: (a) not now; (b) a follow-up
   item with keys for the solid solver's ranks and device, which changes the schema golden. Recommendation: (a) until a fan
   case needs it; the numbers above (8 ranks 1.4x faster than one at 4e4 unknowns; the device 7.6x faster than 16 CPU
   threads at 3e5) favour the device for large models.

## For the lead: roadmap entries

F-433: proposed status **done**. Note:

> Structural solver, advanced tier (plan ADR 0310). ADR 0431, 2026-11-14: rcfd_api::solid_parallel (rcfd-api gains rcfd-dist and rcfd-fem as dependencies, decision A5 (a); library only). solve_static_ranks: the reduced stiffness split by nodes (recursive coordinate bisection of node points, all unknowns of a node on one rank, a split assignment refused naming the node) into DistCsr rows keeping the global entry order, solved on in-process ranks by the distributed CG of rcfd-dist in the outer loop of F-381 (true residual, refinement restarts), with the node block Jacobi of F-381 or a rank-local IC(0) (additive Schwarz without overlap). One rank has the bits of F-381 (cube of 39 744 unknowns: 276 block-Jacobi and 94 IC(0) iterations; Tet10 cantilever of 4 704 unknowns: 581 with one restart), displacements, reactions and residual bit-equal. On 2, 4, 8 ranks: within 1.9e-14 (block Jacobi) and 1.4e-11 (IC(0)) of the serial solution at residual below 1e-10, iterations 1.00 (block Jacobi) and up to 1.52 (rank-local IC(0), cube, 8 ranks) times those of one rank; repeated runs bit-identical; a failing rank ends the solve in 0.015 s and a stalled rank in 3.0 s (time-out 0.5 s) with errors naming rank, peer and tag; scaling recorded (0.167, 0.142, 0.127, 0.116 s for 1, 2, 4, 8 ranks). solve_static_device: rcfd_gpu::resolve with its threshold of 1e5 unknowns; on an RTX 5080 the device Jacobi CG equals the CPU one within 2.1e-13 on a cube of 304 842 unknowns (559 iterations both, 0.60 s against 4.50 s on 16 threads); Chebyshev within 4.5e-15 on 6 084; auto below the threshold runs on the CPU bit-equal; without a device cuda is a located error and auto falls back with the reason recorded. Ten mutations caught. Default solver and baseline hashes unchanged.
