# ADR 0572: F-641 registration: smoothed aggregation for thin plates and shells (the cause of the thin Shell4 plate's count, and a vertex-patch smoother)

Status: **REGISTERED 2026-11-15, written before any code of F-641 in the repository and before any run of its checks.** Not yet
accepted by the maintainer. Before this text no library or test code of F-641 was written in the worktree. A scratch prototype
was written and run (disclosed in full under "Arithmetic and prototype"): it uses the public API at the base d1730051 and a
scratch copy of `rcfd-linalg/src/amg_elastic.rs` with exploration switches. Results go in the addenda at the end; nothing above
the addenda is changed after a run.

Roadmap item F-641 (deps F-629), the follow-up named by [ADR 0552](0552-f629-registration-six-unknown-nodes-in-the-smoothed-aggregation-cg.md)
(its L2, Q2 and its "For the lead" entry). Number 0572 given by the lead. Branch `f641-work` from d1730051, worktree
`w49i-work/f641`, scratch `w49i-work/f641-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no new
dependency, no unsafe code; clean-room. The roadmap files are not edited. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-641 asks

> Every limit not stated here is registered in the ADR before the run. The cause (coarse operator, smoother or aggregation) identified by registered controls on the ADR 0552 plate P4b; if a change is made, P4b at max_coarse 60 in at most twice the Shell3 P3b count, the other ADR 0552 counts not increased and the solid-only bits unchanged.

Clauses: **a1** every limit registered here; **a2** the cause, one of coarse operator, smoother, aggregation, identified by
registered controls on P4b (ADR 0552 D3: Shell4 16 x 16 simply supported, t/a = 0.001); **a3** with the change, P4b at
`max_coarse = 60` in at most twice the P3b count; **a4** the other ADR 0552 counts not increased; **a5** the solid-only bits
unchanged.

"Twice the Shell3 P3b count" is read both ways and both are registered: twice ADR 0552's measured P3b count at S1 (59, so 118) and
twice P3b's count with the same change.

## What was read (no run of a check)

`AGENT_RULES.md`; the brief; ADR 0552 (whole: D0 to D3, arithmetic 1 to 5, the checks, addendum 1, Q2); ADR 0305 (smoothed
aggregation: D1 to D5, the V(1,1)-cycle with forward and backward block Gauss-Seidel); ADR 0370 (geometric aggregation) and ADR
0375 (the sweep count); ADR 0432 (Shell4: MITC4 transverse shear, arithmetic 3 and 4, the shear energy `k G t gamma^2`); ADR 0454
(Shell3: MITC3 with the Lyly-Stenberg-Vihinen factor `t^2 / (t^2 + alpha h^2)`, `alpha = shell3::SHEAR_STABILISATION = 0.1`,
`h` the longest edge; N4 its plain-MITC3 control); the sources at d1730051: `rcfd-linalg/src/amg_elastic.rs` (whole),
`rcfd-fem/src/solve.rs` (the preconditioner enum, `smoothed_aggregation`, `structural_rigid_body_modes`,
`smoothed_aggregation_structural`, `preconditioned_cg` with its true-residual restarts), `rcfd-fem/src/shell3.rs` (the
stabilisation constant and `Shell3Options`), `rcfd-api/src/solid_case.rs` (the `amg*` keys build `ElasticAmgSettings` by its
constructors only); the tests `sa_amg_structural.rs` (ADR 0552), `si_amg_smoother.rs`, `si_amg_quadratic.rs`,
`si_amg_elastic.rs` (every literal of `ElasticAmgSettings` in the workspace uses `..`, so a new field breaks no caller);
`sa_shell3.rs` (`assemble_with` with `shell3::element_stiffness_with` for the controls).

Literature, **cited from memory** (titles and pages not verified against the papers; nothing numeric is taken from them; what is
used is derived in D1 and D2):

- P. Vaněk, J. Mandel and M. Brezina, "Algebraic multigrid by smoothed aggregation for second and fourth order elliptic problems",
  Computing 56 (1996) 179-196 (the method of ADR 0305 and 0552).
- M. Brezina and co-authors applied smoothed aggregation to plates and shells with the rotational near-null fields; recalled from
  memory without a title I can vouch for, so it is named as background only.
- J. Schöberl, "Multigrid methods for a parameter dependent problem in primal variables", Numer. Math. 84 (1999) 97-119; D. N.
  Arnold, R. S. Falk and R. Winther, "Preconditioning in H(div) and applications", Math. Comp. 66 (1997) 957-984, and "Multigrid
  in H(div) and H(curl)", Numer. Math. 85 (2000) 197-217; Y.-J. Lee, J. Wu, J. Xu and L. Zikatanov, "Robust subspace correction
  methods for nearly singular systems", Math. Models Methods Appl. Sci. 17 (2007) 1937-1963: for `A = A_0 + lambda A_1` with large
  `lambda`, a point or node smoother is not uniform in `lambda`, and a subspace-correction smoother is uniform when its subspaces
  decompose the kernel of `A_1` stably; vertex patches are the standard such subspaces.
- D. N. Arnold, R. S. Falk and R. Winther, "Preconditioning discrete approximations of the Reissner-Mindlin plate model", M2AN 31
  (1997) 517-557 (the thin Reissner-Mindlin plate as such a penalty problem).

## D1. Why the thin Shell4 plate is hard for this multigrid (first principles)

The MITC4 bending energy of a plate element of size `h` is `t^3 b(v, v) + k G t s(v, v)`, `b` the bending form (scaled `E / (12
(1 - nu^2))`), `s` the assumed transverse shear `|grad w + beta|^2` (ADR 0432 arithmetic 3 and 4). For a nodal field of wavelength
`h` both forms scale with the element area; the ratio of the shear to the bending stiffness is

`r = k G t h^2 / (E t^3 / (12 (1 - nu^2))) = 12 k (1 - nu^2) (G / E) (h / t)^2 = 3.50 (h / t)^2`

(`k = 5/6`, `nu = 0.3`, `G / E = 1 / 2.6`: `12 x 0.8333 x 0.91 x 0.3846 = 3.50`). For P4a and P4b (`h = 1/16`): `(h / t)^2 = 39.1`
and `3906`, `r = 137` and `1.37e4`. So the stiffness is `A = A_b + lambda A_s` with `lambda` growing a hundredfold from P4a to P4b.

A block Gauss-Seidel sweep on node blocks reduces an error component `e` roughly by the ratio of its energy to the largest nodal
energy, `~ e^T A e / (lambda_max(D^{-1} A) |e|_D^2)`. For a component in (or near) the kernel of the shear form (a discrete
Kirchhoff field, `beta ~ -grad w` on every edge) of wavelength a few `h`, `e^T A e` is bending energy only while `lambda_max(D^{-1}
A)` is set by the shear, so the sweep damps it by a factor `~ 1 / r`: such components are oscillatory (the coarse space of
piecewise rigid fields on aggregates cannot represent them) and not smoothed. The two-grid rate then degrades with `r`, that is with
`(h / t)^2`, whatever the aggregation and coarse operator. This is the smoother hypothesis; it predicts (i) the count grows with
`(h / t)^2` under the node smoother, (ii) a smoother whose subspaces contain the local discrete Kirchhoff fields removes the growth,
(iii) changes of the aggregation or of the prolongator do not.

Why Shell3 P3b is not hard: its shear is multiplied by `t^2 / (t^2 + alpha h_e^2)` (ADR 0454). On P3b, `h_e = sqrt(2) / 32 =
0.0442`, `alpha h_e^2 = 1.95e-4`, `t^2 = 1e-6`: factor `5.1e-3`; with `(h / t)^2 = 977` (`h = 1/32`) the ratio is `3.50 x 977 x
5.1e-3 = 17.5`; on P3a (`t^2 = 1e-4`, factor `0.339`, `(h / t)^2 = 9.77`) it is `11.6`. The stabilisation keeps `r` of order 10
at both thicknesses. Prediction (iv): Shell3 with the stabilisation removed (`alpha = 0`, plain MITC3, `r = 3.42e3` on P3b) is as
hard as Shell4 under the node smoother. This is the mechanism control M1.

## D2. The change: a vertex-patch smoother, opt-in

New in `rcfd-linalg/src/amg_elastic.rs`:

- `#[non_exhaustive] pub enum Smoother { BlockGaussSeidel, VertexPatch }` and a field `pub smoother: Smoother` of
  `ElasticAmgSettings`, default `BlockGaussSeidel` (the constructors `quadratic`, `quadratic_smoothed` inherit it through
  `..default()`). With `BlockGaussSeidel` no new line runs: the hierarchy, the cycle and every bit are those of d1730051.
- With `VertexPatch`, on every level above the coarsest, the patch of block `b` is the set of blocks `c` with a stored entry
  `A[r][k]`, `r` in `b`, column `k` in `c` (on the fine level of a shell mesh: the node and the nodes sharing an element with it,
  the vertex patch of the 2 x 2 elements around the node). Its rows are the rows of those blocks in increasing order; its matrix is
  `A` restricted to them (a principal submatrix of an SPD matrix, so SPD), inverted densely by the existing Cholesky
  `spd_inverse`; a non-positive pivot refuses the build with a new `ElasticAmgError::PatchNotPositive { level, block }`.
- The sweep is multiplicative Schwarz with exact local solves: for each patch in order, `res = b_P - (A x)_P` on its rows, `x_P
  += A_PP^{-1} res`. The pre-smoother visits the patches in increasing order (`pre_sweeps` times), the post-smoother in decreasing
  order (`post_sweeps` times). The backward sweep is the adjoint of the forward one in the `A` inner product, so the V-cycle with
  equal counts is symmetric (check SY1) and CG may use it.
- The patches of the fine level contain the local discrete Kirchhoff fields (a nodal `w` with the rotations of its neighbours
  adjusted), which is what D1 asks; that the decomposition is stable in `t` for MITC4 is not proven here, it is tested by FX3 on
  four thicknesses.

The route: `CgPreconditioner::SmoothedAggregationStructural(ElasticAmgSettings { smoother: Smoother::VertexPatch, .. })` (and
equally `SmoothedAggregation`); `rcfd-fem` source is not changed, no case-file key is added. Options considered: (a) this smoother
(taken: the only change the prototype found to remove the growth, arithmetic 3); (b) more block Gauss-Seidel sweeps (reduces the
count, keeps the growth; arithmetic 3); (c) a different aggregation or prolongator (no effect or worse; arithmetic 3); (d) the
patch smoother on the fine level only (prototype 34 against 31 on P4b); all levels above the coarsest is taken because D1 holds on
every level where `h >> t`. Taken on the maintainer's standing instruction of 2026-11-11.

Cost (disclosed, not a check): a fine-level patch of a plate node has 9 nodes x 3 free unknowns = 27 rows (54 for a shell node
with six unknowns), so 729 to 2916 stored inverse entries per node; a Hex8 patch has 27 nodes x 3 = 81 rows (6561 entries per
node). The smoother is meant for thin structures, not for solids.

## Arithmetic and prototype (disclosed)

The prototype is `f641-scratch/proto` (`src/main.rs`, and `src/sa.rs` = `amg_elastic.rs` of d1730051 with two switches: the
number of prolongator smoothing steps, default 1, and the number of levels given patches, default 0; the vertex-patch sweep as D2).
With the switches at their defaults it reproduces the library: P4a, P4b, P3a, P3b, C4, C3, BM at S1 give 85, 531, 50, 59, 50, 23,
21, the numbers of ADR 0552, equal to the library run in the same log (`explore1.log`). The CG is the restarted loop of
`preconditioned_cg` (true relative residual `tau = 1e-9`, at most three restarts, cap 5000), copied as in ADR 0552. S1 =
`ElasticAmgSettings { max_coarse: 60, ..default }`, S0 = the default. Counts below are total iterations with restarts.

1. **The controls on P4b (and P4a, same mesh, t/a = 0.01)** (`explore1.log`, `deep2.log`): P4b / P4a, and `G = P4b / P4a`:

   | Ingredient | Control | P4b | P4a | G |
   |---|---|---|---|---|
   | reference | S1 | 531 | 85 | 6.2 |
   | aggregation | geometric aggregation (ADR 0370 ratio) | 612 | 85 | 7.2 |
   | aggregation | `theta = 0.08` | 576 | 79 | 7.3 |
   | coarse operator | two levels, the level-1 Galerkin operator solved exactly (`max_levels = 2`) | 620 | 77 | 8.1 |
   | coarse operator | two levels, the tentative prolongator | 576 | 119 | 4.8 |
   | coarse operator | two levels, the prolongator smoothed twice | 646 | 72 | 9.0 |
   | coarse operator | three levels, tentative / smoothed twice / three times | 894 / 698 / 750 | 230 / 74 / 74 | 3.9 / 9.4 / 10.1 |
   | smoother | two levels, the vertex-patch smoother on the fine level | 31 | 18 | 1.7 |
   | smoother | three levels, vertex patches on every level above the coarsest (the change) | 31 | 19 | 1.6 |
   | smoother | three levels, vertex patches on the fine level only | 34 | 22 | 1.5 |
   | smoother | block Gauss-Seidel, 2 / 4 sweeps per side | 369 / 242 | 57 / 43 | 6.5 / 5.6 |

   Every change of aggregation or coarse operator leaves P4b above 570 and `G` above 3.9; the vertex-patch smoother alone brings
   P4b to 31 and `G` to 1.7. More node sweeps lower the count but keep `G` above 5.

2. **Thickness scan** (`tscan.log`, S1, Shell4 16 x 16): t/a = 0.1, 0.03, 0.01, 0.003, 0.001: node smoother 11, 29, 85, 243, 531
   (growth about `(a/t)^0.85` per decade below 0.01); vertex patches 7, 12, 19, 26, 31. Shell3 32 x 32: node smoother 19, 35,
   50, 58, 59. At t/a = 3e-4 and 1e-4 the true residual of the Shell4 plate stops above `tau` (rounding floor `7e-9`, `6e-8`) with
   either smoother, so those thicknesses are not registered. On a Shell4 32 x 32 plate at t/a = 0.001 the floor is `2.8e-9`, above
   `tau`, and the counts (704 node, 110 patch) are floor-limited; not registered (L2).
3. **The other models with the change** (`s0.log`): S0 + vertex patches: P4a, P4b, C4, P3a, P3b, C3, BM: 1, 1, 1, 20, 22, 1, 1
   (five one-level solves, where the smoother is not used); S1 + vertex patches: 19, 31, 8, 22, 25, 11, 16 (ADR 0552: 85, 531, 50,
   50, 59, 23, 21). Errors against the direct solve at most `7.1e-11` (P4b). Build times below 0.04 s.
4. **Mechanism control** (`plain.log`, P3b assembled through `assemble_with` with `shell3::element_stiffness_with`, S1): with the
   default stabilisation `alpha = 0.1`: node smoother 59 (as the library), patches 25; with `alpha = 0` (plain MITC3): node smoother
   1147 (three restarts, residual floor `3.4e-9`, not converged to `tau`), patches 303 (floor-limited too). At t/a = 0.01, `alpha = 0`:
   81 and 30. Removing the stabilisation makes Shell3 as hard as Shell4 for the node smoother (prediction iv).
5. **Digests at the base** (`digest-base.log`, FNV-1a 64 of the little-endian bits of `SolidResult::displacement_values`, through
   `solve_static_report` at d1730051): the Hex8 cantilever of ADR 0552 D3 (tolerance `1e-10`, cap 1000):
   `SmoothedAggregation(default)` `29a878b35b6d6b25` (15 iterations, 0 restarts), `SmoothedAggregation(quadratic())`
   `29a878b35b6d6b25` (15, 0), `SmoothedAggregation(quadratic_smoothed())` `949d365bda156212` (12, 0),
   `SmoothedAggregationStructural(default)` `29a878b35b6d6b25` (15, 0); P4b by `SmoothedAggregationStructural(S1)` (`tau = 1e-9`,
   cap 5000) `d8339a8d90774479` (531, 2).
6. **Limits** below are, for counts expected to stay well inside, `ceil(1.5 x estimate)` (the margin of ADR 0552 arithmetic 5) or
   the acceptance's own number where it is tighter; for the cause rule, thresholds placed between the clusters of item 1 (P4b `<=
   118` and `G <= 2.5` against P4b `>= 236` and `G >= 3.5`; nearest estimates 31 / 1.7 and 576 / 4.8). The estimates and the
   thresholds were set after the prototype runs above; the registered runs are the library path and the test harness below, made
   once each.

## Checks (limits fixed now)

Models P4a, P4b, C4, P3a, P3b, C3, BM: exactly those of ADR 0552 D3 (the test file copies `sa_amg_structural.rs`'s builders; that
file is not edited). `VP(S)` = settings `S` with `smoother: Smoother::VertexPatch`. Library-path counts are `SolveReport::iterations`
of `solve_static_report` with `SmoothedAggregationStructural`, `tau = 1e-9`, cap 5000. Control counts use the hierarchy of
`solve::smoothed_aggregation_structural` (the library's) with the restarted CG loop of `preconditioned_cg` copied into the test, which
reports its count also when the residual stops above `tau`; the residual reached is printed. "Test-side two-grid": fine level
matrix, blocks and prolongator taken from the library hierarchy (`ElasticAmg::matrix(0)`, `block_ptr(0)`, `prolongator(0)`,
`tentative(0)`, `omega(0)` of `S1` with `max_levels = 2`), the coarse operator `P^T A P` solved by a dense Cholesky, the fine
smoother written in the test (block Gauss-Seidel as the library's, or the vertex-patch sweep of D2), one pre- and one post-sweep.
`G(X) = count(P4b, X) / count(P4a, X)`.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| K0 | a2 | reference, library path, node smoother, S1: P4b iterations, restarts, digest; P4a iterations | P4b `= 531`, 2 restarts, `d8339a8d90774479`; P4a `= 85` | arithmetic 5 |
| KA1 | a2 | aggregation: S1 with `Aggregation::Geometric { ratio: QUADRATIC_RATIO }`, control CG | P4b `>= 236` and `G >= 3.5` | 612, 7.2 |
| KA2 | a2 | aggregation: S1 with `theta = 0.08`, control CG | P4b `>= 236` and `G >= 3.5` | 576, 7.3 |
| KC1 | a2 | coarse operator: S1 with `max_levels = 2` (exact level-1 solve), control CG | P4b `>= 236` and `G >= 3.5` | 620, 8.1 |
| KC2 | a2 | coarse operator: test-side two-grid, the tentative prolongator, block Gauss-Seidel | P4b `>= 236` and `G >= 3.5` | 576, 4.8 |
| KC3 | a2 | coarse operator: test-side two-grid, `P_2 = P - omega D^{-1} A P` (the smoothed prolongator smoothed once more, same `omega`), block Gauss-Seidel | P4b `>= 236` and `G >= 3.5` | 646, 9.0 |
| KV | a2 | the harness: test-side two-grid with the library's smoothed prolongator and block Gauss-Seidel against KC1, on P4b and P4a | counts within 15 % of KC1's | equal arithmetic up to the coarse solve |
| KS1 | a2 | smoother: test-side two-grid, the library's smoothed prolongator, the vertex-patch sweep | P4b `<= 118` and `G <= 2.5` | 31, 1.7 |
| KS2 | a2 | smoother: library path, `VP(S1)` (the change) | P4b `<= 118` and `G <= 2.5` | 31, 1.6 |
| KS3 | a2 | (informative, not in the cause rule) S1 with 4 block Gauss-Seidel sweeps per side, control CG | `G >= 3.5` | 5.6 |
| M1 | a2 | mechanism: P3b assembled with `Shell3Options::default().stabilisation(0.0)`, S1 node smoother, control CG; and with the default options (same harness) | `alpha = 0`: count `>= 236`; default: `<= 89` (ADR 0552 IT4 limit) | 1147; 59 |
| FX1 | a3 | `VP(S1)`, library path: P4b iterations | `<= 118` and `<= 2 x` P3b's `VP(S1)` count | 31; 2 x 25 |
| FX2 | a4 | `VP(S0)` and `VP(S1)`, library path, every model: solve `Ok`, `|u - u_d|_2 / |u_d|_2` and `|u - u_d|_inf / |u_d|_inf` against the direct solve, iterations | errors `<= 1e-8`; iterations `<=` ADR 0552's measured counts: S0 1, 1, 1, 32, 38, 1, 1; S1 85, 531, 50, 50, 59, 23, 21 | S0 1, 1, 1, 20, 22, 1, 1; S1 19, 31, 8, 22, 25, 11, 16; errors `<= 7.1e-11` |
| FX3 | (beyond) | `VP(S1)`, library path, Shell4 16 x 16 plate at t/a = 0.1, 0.01, 0.003, 0.001 | iterations `<= 11, 29, 39, 47` | 7, 19, 26, 31 |
| SY1 | D2 | the `VP(S1)` hierarchy of P4b as a preconditioner `M`: `|(M x, y) - (x, M y)| / (|M x| |y|)` for two fixed vectors | `<= 1e-11` | rounding |
| RF1 | D2 | a 20-row chain with scalar blocks whose rows 9 and 10 form an indefinite 2 x 2 principal block (diagonal 1, coupling 2) and a constant near-null vector, `max_coarse = 2`: `VertexPatch` refuses with `PatchNotPositive { level: 0, .. }`; `BlockGaussSeidel` builds | as stated | D2 |
| DF | a5 | `ElasticAmgSettings::default().smoother`, `quadratic().smoother`, `quadratic_smoothed().smoother` | `== BlockGaussSeidel` each | D2 |
| B1 | a5 | every existing test of `rcfd-linalg` and `rcfd-fem` passes unedited (among them `sa_amg_structural.rs`, `si_amg*.rs`); `baseline_hashes`; `docs_site` | pass | D2 |
| B2 | a5 | the five digests, iterations and restarts of arithmetic 5, recomputed after the change | equal | D2 |
| B3 | a5 | `git diff d1730051 -- crates/*/src` touches only `rcfd-linalg/src/amg_elastic.rs`, additions only | exact | D2 |

Cause rule (a2), registered: the **smoother** is identified as the cause when KS1 and KS2 meet their limits and KA1, KA2, KC1,
KC2, KC3 each meet theirs, with KV met. Symmetrically, aggregation (or the coarse operator) would be identified if its controls met
the `<= 118, G <= 2.5` side and the other two ingredients' controls the `>= 236, G >= 3.5` side; any other outcome is "cause not
identified". M1 supports the mechanism of D1 and is not needed by the rule.

## Negative controls

- KA1, KA2, KC1 to KC3 are the negative controls of the cause (changes of the other two ingredients that must not remove the
  growth); KS3 shows that more of the node smoother does not remove it.
- M1 is the control of the mechanism (the Shell3 element without the factor that keeps `r` small behaves like Shell4).
- RF1 and SY1 control the new code's refusal and symmetry; DF and B2 control the defaults.

## Mutations (on a scratch copy, own `CARGO_TARGET_DIR`)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| MU1 | the `VertexPatch` sweep calls the block Gauss-Seidel sweep (patches built, not used) | KS2, FX1 (531 > 118) |
| MU2 | the patch of block `b` is block `b` alone | KS2, FX1 (it is block Gauss-Seidel) |
| MU3 | the post-smoother visits the patches in increasing order | SY1 |
| MU4 | the default smoother `VertexPatch` | DF, B2 |
| MU5 | the refusal replaced by building on (a non-positive patch inverted by the unchecked path: `spd_inverse(...).unwrap_or_default()`) | RF1 |

No mutation is declared equivalent.

## Decision rule

- **done:** the smoother (or another ingredient) identified by the cause rule; FX1, FX2, SY1, RF1, DF, B1, B2, B3, K0 met; FX3 and
  M1 met; every mutation caught; no amendment.
- **partial:** the cause identified, or a change made with FX1 met, and some other check missed with a diagnosed cause, or a
  mutation not caught, or a check met only after an amendment.
- **todo:** the cause not identified and FX1 missed, or any of B1, B2, B3, DF missed.

## Limits (what is not covered)

- **L1.** One mesh per element (Shell4 16 x 16, Shell3 32 x 32), one forced hierarchy, the thicknesses of FX3. No proof that the
  vertex patches decompose the MITC4 discrete Kirchhoff kernel stably; FX3 only tests it on one mesh.
- **L2.** Thinner plates or finer meshes where the CG's true residual stops above `tau = 1e-9` (Shell4 at t/a = 3e-4, and a 32 x 32
  Shell4 plate at t/a = 0.001; arithmetic 2) are not covered: there the conditioning, not the multigrid, limits the solve.
- **L3.** Memory and time of the patch inverses (D2) are not checked; the smoother is not meant for solids and no solid count is
  registered with it.
- **L4.** No case-file key, no change to `rcfd-fem`; the existing variants and defaults keep the node smoother.

## Addendum 1: results (2026-11-15)

Run 1, once, at 223b917e with the code later committed as 995a6b87 in the working tree (`f641-scratch/run1.log`: `cargo test -p
rcfd-fem --release --test sa_thin_plate -- --nocapture --test-threads=1`, 6 tests, 2.35 s). `cargo fmt` was applied after the run
(layout only) and the code committed as 995a6b87. No amendment; no check re-run. Then, at 995a6b87: `cargo test -p rcfd-linalg
--release` (129 passed, 0 failed; `linalg-all.log`), `cargo test -p rcfd-fem --release` (1147 passed, 0 failed, 304 ignored; `fem-all.log`), `cargo test -p
rcfd-api --release --test baseline_hashes` (3 passed, 1 ignored; `baseline.log`), `cargo test -p rcfd-cli --release --test docs_site`
(2 passed; `docs.log`), `cargo clippy -p rcfd-linalg -p rcfd-fem --release --all-targets -- -D warnings` (clean), `cargo fmt -p
rcfd-linalg --check` and `cargo fmt -p rcfd-fem --check` (clean).

| Id | Limit | Measured | Met |
|---|---|---|---|
| K0 P4b S1; P4a S1 | `531`, 2 restarts, `d8339a8d90774479`; `85` | 531, 2, `d8339a8d90774479`; 85 | yes |
| KA1 geometric aggregation (P4b, `G`) | `>= 236`, `>= 3.5` | 612, 7.20 (P4a 85) | yes |
| KA2 `theta = 0.08` | `>= 236`, `>= 3.5` | 576, 7.29 (P4a 79) | yes |
| KC1 exact level-1 solve | `>= 236`, `>= 3.5` | 620, 8.05 (P4a 77) | yes |
| KC2 tentative prolongator (two-grid) | `>= 236`, `>= 3.5` | 654, 5.50 (P4a 119) | yes |
| KC3 prolongator smoothed twice (two-grid) | `>= 236`, `>= 3.5` | 744, 10.33 (P4a 72) | yes |
| KV test-side two-grid against KC1 (P4b; P4a) | `<= 15 %` | 629 / 620 = 1.5 %; 77 / 77 = 0 % | yes |
| KS1 vertex patches (two-grid) | `<= 118`, `<= 2.5` | 37, 2.06 (P4a 18) | yes |
| KS2 `VP(S1)` library path | `<= 118`, `<= 2.5` | 31, 1.63 (P4a 19) | yes |
| KS3 four node sweeps, `G` | `>= 3.5` | 5.63 (242 / 43) | yes |
| M1 P3b plain MITC3; default | `>= 236`; `<= 89` | 1147 (3 restarts, residual floor `3.37e-9`); 59 | yes |
| FX1 P4b `VP(S1)` | `<= 118` and `<= 2 x 25 = 50` | 31 (1 restart) | yes |
| FX2 `VP(S0)` iterations P4a, P4b, C4, P3a, P3b, C3, BM | `<= 1, 1, 1, 32, 38, 1, 1` | 1, 1, 1, 20, 22, 1, 1 | yes |
| FX2 `VP(S1)` iterations | `<= 85, 531, 50, 50, 59, 23, 21` | 19, 31, 8, 22, 25, 11, 16 | yes |
| FX2 errors (2-norm, inf-norm), 14 solves | `<= 1e-8` | largest `9.3e-11`, `1.6e-10` (P4b `VP(S0)`, a one-level solve); with `VP(S1)` largest `7.1e-11`, `9.5e-11` (P4b) | yes |
| FX3 P4 at t/a = 0.1, 0.01, 0.003, 0.001 | `<= 11, 29, 39, 47` | 7, 19, 26, 31 | yes |
| SY1 symmetry | `<= 1e-11` | `8.2e-15` | yes |
| RF1 refusal; node smoother builds | as registered | `PatchNotPositive { level: 0, block: 9 }`; builds | yes |
| DF defaults | `BlockGaussSeidel` | `BlockGaussSeidel` (three constructors) | yes |
| B1 existing tests unedited; baseline hashes; docs site | pass | pass (`rcfd-linalg` 129, `rcfd-fem` 1147 passed, 0 failed); 3 passed; 2 passed | yes |
| B2 four Hex8 digests and the P4b digest | equal | equal (15, 15, 12, 15 iterations, 0 restarts; P4b by K0) | yes |
| B3 `git diff d1730051 -- crates/*/src` | `amg_elastic.rs` only, additions only | `rcfd-linalg/src/amg_elastic.rs` +115 -0 (hunks at new lines 32-33, 46, 51-63, 137-143, 168-173, 260-335, 936, 946-949, 961, 1026-1029) | yes |

Against the estimates: KC2 654 and KC3 744 (prototype 576 and 646: the prototype changed the prolongator inside its own build,
the test's two-grid takes the library's tentative and smoothed prolongators and smooths with a test-side Jacobi step; the counts
stay in the same cluster); KS1 37 and `G = 2.06` (prototype 31 and 1.7; the test-side two-grid solves the coarse problem by a dense
Cholesky inverse and P4b reaches the residual floor with three restarts), inside the `2.5` limit with less margin than estimated.
Every library-path count equals the prototype's.

**Cause (a2), by the registered rule: the smoother.** KS1 and KS2 meet the cause side; KA1, KA2, KC1, KC2 and KC3 meet the
exclusion side; KV holds. No change of the aggregation (two rules) or of the coarse operator (exact level-1 solve, tentative or
twice-smoothed prolongator) brings P4b under 576 or `G` under 5.5; the vertex-patch smoother alone, on the same aggregates,
prolongator and coarse operator, brings P4b from 620 to 37 (two levels) and from 531 to 31 (three levels). M1 supports D1: the
Shell3 plate without its shear factor is as hard for the node smoother as Shell4 (1147), so what separates P4b from P3b is the
element's shear-to-bending ratio (`1.37e4` against `17.5`, D1), not the triangle or the aggregation.

Mutations (scratch copy `f641-scratch/mut` from `git archive` of 995a6b87, `target-mut`, `mutate.py`, logs `mut-MU*.log`;
`amg_elastic.rs` restored and checked byte-equal after the run):

| Id | Caught by |
|---|---|
| MU1 patches not used | KS2 (531, `G` 6.25), FX1 (531), FX3 (85, 243, 531) |
| MU2 patch = the block alone | KS2 (506), FX1, FX3, RF1 (no refusal: a single scalar block is positive) |
| MU3 post-smoother in increasing order | SY1 (`1.6e-4`); FX2, FX3 (the CG does not converge with the non-symmetric cycle) |
| MU4 default `VertexPatch` | DF, B2 (all four Hex8 digests, `7280e11dfa92b3d0`, 10 iterations), K0 (31), KA1, KA2, KC1, KV, KS3 |
| MU5 refusal removed | RF1 |

Five of five caught; none declared equivalent.

Not run: meshes other than the ADR 0552 ones and thicknesses below t/a = 0.001 (L1, L2: the CG's residual floor is above `tau`
there, arithmetic 2); memory and time of the patches (L3); solids with the new smoother.

**Decision by the registered rule: done.** The cause identified by the registered rule (the smoother); every other registered
check met in the single run (K0, FX1, FX2, FX3, M1, SY1, RF1, DF, B1, B2, B3); every mutation caught; no amendment.

Remaining questions:

1. **Q1. Should `SmoothedAggregationStructural` take the vertex-patch smoother by default?** (a) Keep the node smoother as the
   variant's default and the patch smoother opt-in (now; the ADR 0552 bits stay). (b) Make the structural variant use
   `VertexPatch` (every ADR 0552 count falls, FX2; its records would change, and ADR 0552's B2b, which compares the variant with
   `SmoothedAggregation` on a solid bit for bit, would need the solid rows kept on the node smoother). Recommendation: (a) until a
   case-file key exists; then (b) for shell models, as a maintainer decision.
2. **Q2. Robustness beyond this mesh.** On a 32 x 32 Shell4 plate at t/a = 0.001 the prototype's patch-smoother count was 110,
   floor-limited: the CG's true residual stops at `2.8e-9` there with either smoother (arithmetic 2). (a) Leave it. (b) A
   follow-up registering the patch smoother on a mesh sequence at thicknesses where the residual floor is below `tau`, with the
   count against `h / t`. Recommendation: (b).
3. **Q3. Memory.** The dense patch inverses cost 27 to 54 rows squared per shell node (D2). (a) Leave it (opt-in, meant for thin
   structures). (b) A sparse local factor (a gain only for the larger patches of coarse levels). Recommendation: (a).
