# ADR 0577: F-659 registration: the vertex-patch smoother on a mesh sequence of thin plates (robustness of the ADR 0572 smoother in h/t)

Status: **REGISTERED 2026-11-15, written before any code of F-659 in the repository and before any run of its checks.** Not yet
accepted by the maintainer. Before this text no library or test code of F-659 was written in the worktree. A scratch prototype
was written and run (disclosed in full under "Arithmetic and prototype"): it calls the public API of the base e348eb34 and copies
the harness of `rcfd-fem/tests/sa_thin_plate.rs`. Results go in the addenda at the end; nothing above the addenda is changed after
a run.

Roadmap item F-659 (deps F-641), the follow-up proposed by [ADR 0572](0572-f641-registration-smoothed-aggregation-for-thin-plates-and-shells.md)
(its L1, L2 and Q2). Number 0577 given by the lead. Branch `f659-work` from e348eb34, worktree `w49i-work/f659`, scratch
`w49i-work/f659-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-659 asks

> Every limit not stated here is registered in the ADR before the run. The Shell4 and Shell3 simply supported plates on at least three meshes, at thicknesses where the CG's true-residual floor is below the tolerance, solved by SmoothedAggregationStructural with Smoother::VertexPatch and a forced hierarchy in a registered number of iterations each, the growth of the count with h/t bounded by a registered limit; the default settings' bits unchanged.

Clauses: **b1** every limit registered here; **b2** Shell4 and Shell3 simply supported plates on at least three meshes; **b3** at
thicknesses where the CG's true-residual floor is below the tolerance (made operational and checked, D2); **b4** solved by
`SmoothedAggregationStructural` with `Smoother::VertexPatch` and a forced hierarchy in a registered number of iterations each;
**b5** the growth of the count with `h / t` bounded by a registered limit; **b6** the default settings' bits unchanged.

## What was read (no run of a check)

`AGENT_RULES.md`; the brief; ADR 0572 (whole: D1 the shear-to-bending ratio `r = 3.50 (h / t)^2`, D2 the smoother, arithmetic 2
the thickness scan and the residual floor, L1, L2, Q2); ADR 0552 (D3 the plate models, arithmetic 1 the forced hierarchy
`max_coarse = 60`, arithmetic 2 the tolerance `tau = 1e-9` above the rounding floor); ADR 0454 (Shell3, the factor `t^2 / (t^2 +
alpha h_e^2)`); the sources at e348eb34: `rcfd-linalg/src/amg_elastic.rs` (`Smoother`, `build_patches`, `patch_sweep`, the cycle),
`rcfd-fem/src/solve.rs` (`SolveReport`, `preconditioned_cg` and its three true-residual restarts, `smoothed_aggregation_structural`);
the test `rcfd-fem/tests/sa_thin_plate.rs` (F-641), whose harness (the plate builder, `system`, `hierarchy`, `control_cg`) this item
reuses by copy (that file is not edited).

Literature: none beyond ADR 0572's (cited there from memory). Nothing numeric is taken from a paper; every number below is from
the prototype or a closed form.

## D1. The question and what is being measured

ADR 0572 measured the vertex-patch smoother on one mesh per element (Shell4 16 x 16, Shell3 32 x 32). Its D1 argument (the node
smoother's damping of near-Kirchhoff components degrades like `1 / r`, `r = 3.50 (h / t)^2`) predicts for the node smoother a
two-grid rate `1 - c / r` and so a CG count growing like `sqrt(r)`, i.e. like `(h / t)^1`; the patch smoother is meant to remove
that growth. F-659 tests it on a mesh sequence: the same plate (unit square, simply supported, `nu = 0.3`, pressure 1000 Pa, the
in-plane and drilling unknowns fixed; ADR 0552 D3) on `n x n` meshes, `n = 8, 16, 32, 64` (Shell4: `n^2` quadrilaterals; Shell3:
`2 n^2` triangles, each square split by its diagonal, as in ADR 0552), `h = 1 / n`, at thicknesses `t / a` in the grid `T = {0.1,
0.03, 0.01, 0.003, 0.001}` restricted by D2. Four meshes per element (the acceptance asks at least three).

Route of every registered solve: `solve_static_report` with `StaticSolver::PreconditionedCg { preconditioner:
CgPreconditioner::SmoothedAggregationStructural(VP(S1)), tolerance: 1e-9, max_iterations: 5000 }`, `S1 = ElasticAmgSettings {
max_coarse: 60, ..default }` (the forced hierarchy of ADR 0552 and 0572), `VP(S) = S` with `smoother: Smoother::VertexPatch`. The
count is `SolveReport::iterations` (summed over restarts).

**Growth exponent.** On mesh `n`, with `t_0 = 0.1` and `t_n` the thinnest admissible thickness of `T` (D2), `H_n = t_0 / t_n` (the
range of `h / t` covered on that mesh) and

`p_n = ln(count(n, t_n) / count(n, t_0)) / ln(H_n)`,

the exponent of a power law `count ~ (h / t)^p` through the two ends. D1's node-smoother prediction is `p -> 1` for thin plates;
"growth bounded" is registered as `p_n <= 0.6` for every mesh and both elements (b5), and as `p_n` of the patch smoother below
`p_n` of the node smoother on the same mesh (control NC). Taken on the maintainer's standing instruction of 2026-11-11 (options:
(a) this two-end exponent per mesh, taken: one number per mesh, each defined by registered solves; (b) a least-squares fit of `ln
count` on `ln(h / t)` over all thicknesses, rejected because the curve bends, see arithmetic 3, and a fit would hide where; (c) a
bound on the count ratio only, rejected because the ranges `H_n` differ between meshes).

The count is not a function of `h / t` alone: at fixed `t` it also grows with `n` (arithmetic 3). That is recorded by HX (the
thick plate, `t = 0.1`, across the meshes) and stated in the limits; it is not part of b5.

## D2. Admissible thicknesses: the residual floor (b3)

The tolerance `tau = 1e-9` is a true relative residual; the restarted CG cannot reach it when the rounding floor of `|b - A x| /
|b|` is near or above it (ADR 0552 arithmetic 2; ADR 0572 arithmetic 2, where Shell4 32 x 32 at t/a 0.001 stopped at `2.8e-9`).
Operational definition, registered: the **floor probe** of a case is the true relative residual reached by the restarted CG of
`preconditioned_cg` (the test's copy `control_cg`, three restarts, cap 5000) with the library hierarchy of `VP(S1)` and the target
`1e-14`, far below any floor (it always ends by exhausting its restarts). A case is **admissible** when its probe is `<= tau / 4 =
2.5e-10`. The factor 4 is chosen from the prototype (arithmetic 2): every case with probe at most `2.29e-10` solved to `tau` with
no restart, and every case with probe at least `2.59e-10` took a restart or failed; a count taken where restarts begin measures
the floor, not the multigrid. The admissible set (arithmetic 2) is registered as the list below; the test checks it (FL) and checks
that the next thinner thickness of `T` on each mesh is not admissible (BD), so the set is the largest of `T` with this rule.

Closed-form cross-check (disclosed, not a check): the probe of the Shell4 plate scales like `(n / t)^2` (rounding of `A x` against
`|b|` ~ `eps` times the ratio of the shear to the smallest bending eigenvalue): `2.5e-18 (n a / t)^2` fits 6.7e-10 (n 16, t 0.001),
2.7e-10 (n 32, t 0.003), 1.0e-10 (n 64, t 0.01) within 10 %. Shell3's factor caps the shear and its probes are smaller.

## D3. Nothing in the library changes

F-659 is a measurement. No source file under `crates/*/src` changes; the new file is `crates/rcfd-fem/tests/sa_thin_plate_sequence.rs`.
So the default settings' bits (b6) are those of e348eb34 by construction, checked by B1 (the diff is empty) and by re-running the
unedited F-641 checks that pin the defaults (DF, B2 and K0 of `sa_thin_plate.rs`).

## Arithmetic and prototype (disclosed)

The prototype is `f659-scratch/proto` (`src/main.rs`: the F-641 harness copied, a plate builder with `n` as a parameter, and a
loop over `(n, t)`; logs `scan-repro.log`, `scan-quad1.log`, `scan-quad64.log`, `scan-tri1.log`, `scan-node.log`). It calls the
library at e348eb34 unchanged. For each case it prints the direct solve's relative residual, the control count of `VP(S1)` with
restarts and residual, the library-path count, the error against the direct solve, the floor probe, and (with `node`) the control
count of the node smoother `S1`.

1. **Reproduction.** Shell4 16 x 16 at t/a 0.1, 0.01, 0.003, 0.001: 7, 19, 26, 31 (ADR 0572 FX3: 7, 19, 26, 31); library-path and
   control counts equal in every case of the scan.
2. **Floor probes and admissibility** (`VP(S1)`, probe `<= 2.5e-10` admissible; restarts of the `tau = 1e-9` solve in brackets):

   | Element, n | t 0.1 | 0.03 | 0.01 | 0.003 | 0.001 |
   |---|---|---|---|---|---|
   | Shell4 8 | 1.7e-14 | 2.0e-13 | 1.7e-12 | 1.8e-11 | 1.76e-10 |
   | Shell4 16 | 6.7e-14 | 7.7e-13 | 6.3e-12 | 6.3e-11 | 6.68e-10 (1) out |
   | Shell4 32 | 2.5e-13 | 2.9e-12 | 2.6e-11 | 2.71e-10 (1) out | 2.5e-9 (fails) out |
   | Shell4 64 | 9.5e-13 | 1.2e-11 | 1.00e-10 | not run (t 0.006: 2.59e-10) out | not run, out |
   | Shell3 8 | 1.7e-14 | 6.9e-14 | 7.6e-14 | 7.8e-14 | 6.1e-14 |
   | Shell3 16 | 9.8e-14 | 5.2e-13 | 1.0e-12 | 1.2e-12 | 1.2e-12 |
   | Shell3 32 | 3.8e-13 | 3.4e-12 | 1.2e-11 | 1.7e-11 | 1.9e-11 |
   | Shell3 64 | 1.5e-12 | 1.5e-11 | 9.8e-11 | 2.29e-10 | 2.77e-10 (1) out |

   Admissible (registered): Shell4 n 8: all five; n 16: 0.1 to 0.003; n 32 and n 64: 0.1 to 0.01. Shell3 n 8, 16, 32: all five;
   n 64: 0.1 to 0.003. `h / t` covered: Shell4 1.25 to 125, 0.625 to 20.8, 0.3125 to 3.125, 0.156 to 1.56; Shell3 1.25 to 125,
   0.625 to 62.5, 0.3125 to 31.25, 0.156 to 5.21. The Shell4 64 x 64 plate at t 0.003 was not probed; by the `(n / t)^2` scaling of
   D2 its probe is about `1.1e-9`, and BD checks it.
3. **Counts** (`VP(S1)` library path; node smoother `S1` control in brackets; every admissible case 0 restarts; levels 2 at n 8,
   3 at n 16 and 32, 4 at n 64):

   | Element, n | t 0.1 | 0.03 | 0.01 | 0.003 | 0.001 | `p_n` patch | `p_n` node |
   |---|---|---|---|---|---|---|---|
   | Shell4 8 | 6 (11) | 9 (34) | 11 (78) | 14 (141) | 14 (196) | 0.184 | 0.625 |
   | Shell4 16 | 7 (11) | 12 (29) | 19 (85) | 26 (243) | | 0.374 | 0.883 |
   | Shell4 32 | 8 (12) | 14 (21) | 24 (52) | | | 0.477 | 0.637 |
   | Shell4 64 | 9 (13) | 13 (21) | 24 (47) | | | 0.426 | 0.558 |
   | Shell3 8 | 8 (13) | 9 (18) | 9 (20) | 9 (20) | 9 (20) | 0.026 | 0.094 |
   | Shell3 16 | 10 (16) | 14 (26) | 16 (34) | 16 (35) | 16 (33) | 0.102 | 0.157 |
   | Shell3 32 | 11 (19) | 18 (35) | 22 (50) | 25 (58) | 25 (59) | 0.178 | 0.246 |
   | Shell3 64 | 13 (24) | 20 (39) | 32 (75) | 41 (104) | | 0.328 | 0.418 |

   Errors against the direct solve at most `5.4e-11` (relative 2-norm). The patch smoother's exponent is below the node smoother's
   on every mesh, and below 0.48 everywhere; it rises with `n` for Shell3 and is largest on the Shell4 32 x 32 mesh. At fixed `t`
   the count grows with `n`: at t 0.1, Shell4 6, 7, 8, 9 and Shell3 8, 10, 11, 13; at t 0.01, Shell4 11, 19, 24, 24 and Shell3 9,
   16, 22, 32. Shell4 at t 0.01 levels off from n 32 to n 64; Shell3 does not yet.
4. **Limits.** Counts: `ceil(1.5 x estimate)` (the margin of ADR 0552 arithmetic 5 and ADR 0572). Exponent: `0.6`, a fixed
   bound well below D1's node-smoother asymptote 1 and above the largest estimate 0.477 (1.26 times it; tighter than 1.5 times,
   since the runs are deterministic and the library path reproduced the prototype in every case). Node-smoother control for
   Shell4: `p_n >= 0.5` (estimates 0.558 to 0.883). HX: `count(n 64, t 0.1) / count(n 8, t 0.1) <= 2.4` (estimates 1.50, 1.63;
   `1.5 x 1.63`). The estimates and limits were set after the prototype runs; the registered runs are the test below, made once.

## Checks (limits fixed now)

All in `crates/rcfd-fem/tests/sa_thin_plate_sequence.rs`. "Case" = (element, n, t) of the admissible list of arithmetic 2 (32
cases: 15 Shell4, 17 Shell3).

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| FL | b3 | the floor probe (D2) of every admissible case | `<= 2.5e-10` each | largest `2.29e-10` (Shell3 64, t 0.003) |
| BD | b3 | the probe of the next thinner `t` of `T` on each mesh where one exists: Shell4 n 16 t 0.001, n 32 t 0.003, n 64 t 0.003; Shell3 n 64 t 0.001 | `> 2.5e-10` each | 6.68e-10, 2.71e-10, ~1.1e-9, 2.77e-10 |
| LV | b4 | the `VP(S1)` hierarchy of every case has at least two levels (the hierarchy is forced) | `n_levels >= 2` | 2, 3, 3, 4 |
| IT | b4 | library path, every case: solve `Ok`, restarts, iterations | `Ok`; restarts `= 0`; iterations `<=` the table below | arithmetic 3 |
| ER | b4 | every case: `|u - u_d|_2 / |u_d|_2` and `|u - u_d|_inf / |u_d|_inf` against the direct solve | `<= 1e-8` each | `<= 5.4e-11` |
| GX | b5 | `p_n` (D1) of the patch smoother on each of the eight meshes | `<= 0.6` each | 0.026 to 0.477 |
| NC | b5 | `p_n` of the node smoother `S1` (control CG, same cases) on each mesh: patch `p_n` below node `p_n`; Shell4 node `p_n >= 0.5` | as stated | arithmetic 3 |
| HX | (informative) | `count(n 64, t 0.1) / count(n 8, t 0.1)`, each element | `<= 2.4` | 1.50, 1.63 |
| B1 | b6 | `git diff e348eb34 -- crates/*/src` | empty | D3 |
| B2 | b6 | the unedited `sa_thin_plate.rs` (DF, B2 digests, K0) and `sa_amg_structural.rs` pass | pass | D3 |

IT iteration limits (`ceil(1.5 x estimate)`), t = 0.1, 0.03, 0.01, 0.003, 0.001:

| Element, n | limits |
|---|---|
| Shell4 8 | 9, 14, 17, 21, 21 |
| Shell4 16 | 11, 18, 29, 39 |
| Shell4 32 | 12, 21, 36 |
| Shell4 64 | 14, 20, 36 |
| Shell3 8 | 12, 14, 14, 14, 14 |
| Shell3 16 | 15, 21, 24, 24, 24 |
| Shell3 32 | 17, 27, 33, 38, 38 |
| Shell3 64 | 20, 30, 48, 62 |

## Negative controls

- NC: the node smoother on the same cases grows faster in `h / t` (Shell4 node `p_n >= 0.5`), so GX is not met trivially by a
  range `H_n` too small to show growth.
- BD: the admissible set stops where the floor begins; FL alone would be met by a set chosen too thick.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | `patch_sweep` not called: the `VertexPatch` levels sweep with block Gauss-Seidel | IT (e.g. Shell4 16 t 0.003: 243 > 39), GX (Shell4 16: 0.883), NC (patch not below node) |
| MU2 | the patch of block `b` is block `b` alone (`build_patches`) | IT, GX (it is block Gauss-Seidel) |
| MU3 | the post-smoother visits the patches in increasing order (non-symmetric cycle) | IT or ER (the CG loses convergence or takes restarts) |
| MU4 | in the test, the probe target `1e-14` replaced by `tau = 1e-9` (the probe stops at the tolerance and measures the solve, not the floor) | FL (the probes become the residuals of the `tau` solves, e.g. `4.27e-10` for Shell4 16 t 0.1, arithmetic 2's log, above `2.5e-10`) |

No mutation is declared equivalent.

## Decision rule

- **done:** FL, BD, LV, IT, ER, GX, NC, B1, B2 met (HX informative: a miss is recorded and makes the item partial only if GX also
  fails); MU1 to MU4 caught; no amendment.
- **partial:** IT and ER met on at least three meshes per element and some other check missed with a diagnosed cause, or a check
  met only after an amendment, or a mutation not caught.
- **todo:** IT missed on more than one mesh per element, or B1 or B2 missed.

## Limits (what is not covered)

- **L1.** One plate (unit square, simply supported, uniform pressure, flat, structured meshes, Shell3 with one diagonal direction),
  one forced hierarchy (`max_coarse = 60`), one tolerance. No distorted, curved or unstructured meshes; no clamped edges.
- **L2.** Thicknesses and meshes where the floor probe exceeds `tau / 4` (Shell4 below t/a 0.003 at n 16, below 0.01 at n 32 and
  n 64; Shell3 below 0.003 at n 64) are not covered; there the conditioning, not the multigrid, sets the count. The finest meshes
  therefore cover only `h / t <= 1.56` (Shell4) and `<= 5.21` (Shell3); the large `h / t` values come from the coarse meshes.
- **L3.** The count is not a function of `h / t` alone: it grows with `n` at fixed `t` (arithmetic 3); this item bounds the growth
  in `h / t` per mesh and records the growth in `n` (HX), it does not bound the latter in general.
- **L4.** No proof of uniformity; the exponent is a two-point measurement per mesh. Memory and time of the patch inverses not
  checked (as ADR 0572 L3).
