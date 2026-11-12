# ADR 0305: Aggregation multigrid for elasticity: rigid-body modes as near-null space, as a preconditioner of CG

Status: **REGISTERED 2026-11-12, written before any code of F-402 and before any run of its checks**; planned by ADR 0290
(SI-15). Not yet accepted by the maintainer. Before this text nothing of F-402 was written, compiled or run: the code and
documents under "What was read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand; no
script and no library code was run. Results go in the addendum at the end; nothing above the addendum is changed after a run.
The planned stub is kept unchanged at the end ("The planned stub, as written").

Roadmap item F-402 (deps F-026, F-381). Size M. Branch `f402-elasticity-amg` from main bc57e402, worktree `w49i-work/f402`,
scratch `w49i-work/f402-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded, diagnosed and not tuned; a defective set-up gets a corrected
test by an amendment committed before the corrected run; each registered run is made once; existing tests, ignored records,
`baseline_hashes.rs`, the hash tables and the goldens are not edited; defaults bit-identical, the new behaviour opt-in; no new
crate, no unsafe code, no trailers, no push.

**Add, don't alter.** The scalar smoothed-aggregation AMG (`crates/rcfd-linalg/src/amg.rs`), used by the finite-volume pressure
solves, is not edited at all: the elasticity multigrid is a new file `crates/rcfd-linalg/src/amg_elastic.rs` with its own
aggregation, prolongator, smoother and coarse solve (nothing shared with `amg.rs` but the public `CsrMatrix` operations
`matmul`, `transpose`, `matvec`, `residual`). `rcfd-linalg/src/lib.rs` gains one `pub mod amg_elastic;` line and no re-export.
In `crates/rcfd-fem/src/solve.rs`: one new variant of `CgPreconditioner`, one new arm in `solver_name` and in
`preconditioned_cg`, a new `use` line, and one new parameter of the private `preconditioned_cg` (the node coordinates; the
signature line and its single call site change, nothing else of an existing line).

## What F-402 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. On every level the rigid-body modes are in the range of
> the prolongator: |B - P B_c| <= 1e-12 |B|. CG preconditioned by one V-cycle reaches a relative residual of 1e-10 on a clamped
> cube of Hex8 with about 5e3, 4e4 and (heavy record) 3e5 unknowns in a number of iterations that grows by at most 1.5 times
> from the smallest to the largest and stays below 40; on a cantilever of slenderness 20 with about 1e5 unknowns it needs at
> most one fifth of the iterations of the incomplete Cholesky of F-381; the same on a Tet10 mesh within twice the Hex8 count.
> The solution agrees with the direct solver within 1e-7 on the smallest case; operator complexity <= 1.5; iteration counts and
> solution are bit-identical for 1 and 8 threads. The scalar AMG and its tests are unchanged; the default solver's results are
> unchanged bit for bit.

Clauses: (c1) range of the prolongator on every level; (c2) cube, relative residual 1e-10 by CG with one V-cycle; (c3) cube
counts: largest / smallest `<= 1.5`, every count `< 40`; (c4) Hex8 cantilever of slenderness 20, about 1e5 unknowns:
`it_AMG <= it_IC0 / 5`; (c5) the same cantilever on Tet10: `it_AMG,Tet10 <= 2 it_AMG,Hex8`; (c6) smallest cube against the
direct solve `<= 1e-7`; (c7) operator complexity `<= 1.5`; (c8) 1 and 8 threads bit-identical; (c9) scalar AMG and its tests
unchanged; (c10) default results unchanged bit for bit.

**Readings (registered here).**

1. (c1) "The prolongator" whose range must contain the modes is the **tentative** prolongator `P_t` of each level (the one
   built from the modes by the local QR). The smoothed prolongator `P = (I - omega D^{-1} A) P_t` used in the cycle cannot
   satisfy it in rows next to a constraint: there `P B_c = B - omega D^{-1} A B` and `A B != 0`, because `A` is the reduced
   stiffness and a rigid motion of the free nodes strains the elements that touch the clamped ones (first principles: `K B =
   0` for the unconstrained `K`, so `(K_FF B_F)_i = -(K_FP B_P)_i`, nonzero exactly where row `i` couples to a prescribed
   equation). Therefore: B1 checks `P_t` on every level with the limit 1e-12; B2 checks the smoothed `P` on level 0 in the rows
   that couple to no prescribed equation (there `A B = 0` in exact arithmetic) with the limit 1e-10 (roundoff of a sum of
   about 81 products, arithmetic 6); the full smoothed defect is recorded (N2 shows it is not small, so the reading matters).
   Norms are Frobenius over the six columns; `B` on level 0 is built **in the test** from the node coordinates (origin 0,
   not centred) independently of the library code, and on level `l >= 1` it is the library's coarse representation
   `B_l`; the measure is `|B_l - P_t,l (P_t,l^T B_l)| / |B_l|` (`P_t` has orthonormal columns, so `P_t^T B` is the best
   coarse representation) and in addition, for `l >= 1`, `|B_l - P_t,l B_{l+1}| / |B_l|` with the library's `B_{l+1}`.
2. (c2) "CG preconditioned by one V-cycle": `StaticSolver::PreconditionedCg { preconditioner:
   CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::default()), tolerance: 1e-10, max_iterations }`: the CG of ADR
   0282 (true residual `|b - K u| <= 1e-10 |b|`, refinement restarts of D5'), each application of the preconditioner one
   V(1,1)-cycle from zero. The count is `SolverStats::iterations` (summed over restarts). Met when the solve returns `Ok`
   with the relative residual `<= 1e-10`.
3. "Clamped cube of Hex8 with about 5e3, 4e4 and 3e5 unknowns": `hex_box([0; 3], [1; 3], [n; 3])`, `n = 11, 23, 46`, all
   three components fixed on `xmin`, traction `(0, 0, -1e6)` on `xmax`, `E = 2e11`, `nu = 0.3`; unknowns `3 n (n + 1)^2 =
   4752, 39 744, 304 842`. (c3): `it(46) / it(11) <= 1.5` and `max(it) <= 39`; the middle case must also be `<= 39` and is
   recorded against both. The 3e5 case is a heavy record (`#[ignore]`, run once) and carries the growth clause.
4. "Cantilever of slenderness 20 with about 1e5 unknowns": `hex_box([0; 3], [20, 1, 1], [240, 12, 12])` (cubic elements of
   edge 1/12), `xmin` clamped, traction `(0, 0, -1e6)` on `xmax`, the material above; unknowns `240 x 13 x 13 x 3 = 121 680`.
   "The incomplete Cholesky of F-381" is `CgPreconditioner::IncompleteCholesky(IcOptions::default())` (IC(0), ADR 0282),
   measured in the same test, cap 60 000. (c4): `it_AMG <= it_IC0 / 5` (real division). Heavy record.
5. "The same on a Tet10 mesh within twice the Hex8 count": `tet_box([0; 3], [20, 1, 1], [120, 6, 6])` upgraded by
   `upgrade_to_tet10` (the Tet10 nodes of a Kuhn mesh are the half-step lattice, so the nodes and the unknowns are the same
   121 680 as the Hex8 case), same constraints, load and material. (c5): `it_AMG,Tet10 <= 2 it_AMG,Hex8`, the Hex8 count of
   reading 4. IC(0) on the Tet10 case is run and its ratio recorded, without a limit (this reading takes "within twice the Hex8
   count" as the limit of the Tet10 case; the one-fifth ratio on Tet10 is a record). Heavy record.
6. (c6): smallest cube (`n = 11`), `max |u - u_d| / max |u_d| <= 1e-7`, `u_d` from `StaticSolver::Direct`.
7. (c7): `operator_complexity = sum_l nnz(A_l) / nnz(A_0) <= 1.5` on **every** hierarchy built by a registered run (the three
   cubes, the two slenderness-20 cantilevers, the ADR 0282 cantilevers below, the partially constrained cube P1).
8. (c8): the cubes `n = 11` and `n = 23`: a pool of 1 thread with `threads(1)` and of 8 with `threads(8)`; the iteration counts
   equal and every displacement bit-equal.
9. (c9): `git diff bc57e402 -- crates/rcfd-linalg/src/amg.rs crates/rcfd-linalg/tests/amg_contrast.rs` is empty; the
   `rcfd-linalg` lib tests (the `amg` module's among them) and `amg_contrast` pass unedited.
10. (c10): check D1 (hashes of the default and the existing iterative solves, the existing file
    `crates/rcfd-fem/tests/sb_pcg_fill_defaults.rs`, run unedited on this branch and on a `git archive` of bc57e402), D2
    (existing tests, release), D3 (`baseline_hashes`).

## What was read (main bc57e402, read only)

1. `AGENT_RULES.md`; `docs/roadmap.json` F-402; the stub of this ADR; ADR 0290 section on 0305 and its file plan; ADR 0357 in
   full (F-460: counts BJ 156 / IC0 85 / IC1 58 at L/h 5.3, 292 / 206 / 132 at L/h 10.7, 477 / 255 / 171 at 103 968 unknowns);
   ADR 0282 (head, the CG route and its stopping rule as summarised in ADR 0357).
2. Code: `crates/rcfd-linalg/src/amg.rs` (all: scalar SA, greedy aggregation in three passes, `(4/3)/rho` Jacobi smoothing of
   the prolongator with `rho` from 15 power steps times 1.1, Galerkin `R (A P)`, forward / backward Gauss-Seidel, dense LU),
   `matrix.rs` (`CsrMatrix`, `matmul` deterministic per row and parallel over row blocks, `transpose`, `rect`, `matvec`
   parallel over fixed chunks, each row summed sequentially), `precond.rs` (the `Preconditioner: Sync` trait),
   `vecops.rs` (chunked deterministic reductions), `lib.rs`; `crates/rcfd-fem/src/solve.rs` (all), `builders.rs` (`hex_box`,
   `tet_box`, `upgrade_to_tet10`), `mesh.rs` (`nodes()`, element kinds); `crates/rcfd-fem/tests/sb_pcg_fill.rs` and
   `sb_pcg_fill_defaults.rs` (the set-up helpers, copied into the new test file, not imported or edited).
3. **Not read:** the bodies of the papers cited; any multigrid code outside this repository.

## Arithmetic before this registration (disclosed, by hand)

1. **Rank of a local mode block.** The six rigid-body modes of a node set `{x_v}` restricted to the free components: the
   translations `e_c` and the rotations `e_a x (x_v - x_0)`. One node with three free components: rank 3 (a rotation at one
   point is a translation). Two nodes: rank 5 (the rotation about the line through them vanishes on both). Three or more
   collinear nodes: rank 5. Three non-collinear nodes: rank 6. One node with two free components `(0, 1)`: rank 2. On a
   regular box these degeneracies are exact, so a dependent column leaves a residual of roundoff size (about `1e-16` relative)
   and an independent one a residual of at least `h_agg / |x|` relative (`>= 1/12 / 20 ~ 4e-3` for the cases here): a drop
   tolerance of `1e-10` relative to the column's norm before orthogonalisation separates them by more than seven orders.
2. **Aggregates of the cube.** Pass 1 visits the free nodes in their natural order (`x` fastest); with `theta = 0` every stored
   neighbour is strong and a seed takes its 26 neighbours. For `n = 11` the free nodes are `x = 1..11`, `y, z = 0..11`: seeds
   at `x = 1, 4, 7, 10` and `y, z = 0, 3, 6, 9`, the last layer attached in pass 2: about `4^3 = 64` aggregates, 384 coarse
   unknowns (six each), under `max_coarse = 1000`: 2 levels. `n = 23`: `8^3 = 512` aggregates (3072 unknowns), then `3^3 = 27`
   (162): 3 levels. `n = 46`: `16^3 = 4096` (24 576), `6^3 = 216` (1296 > 1000), `2^3 = 8` (48): 4 levels.
3. **Coarse stencil.** In one direction an aggregate covers 3 nodes; the smoothed prolongator's column extends one node further
   on each side (width 5); two columns are coupled by `A` (reach 1) when their supports are within one node: centre distance
   `<= 5`, i.e. only the neighbouring aggregates (distance 3). So the coarse graph is again a 27-point graph, and the same
   aggregation applies on every level.
4. **Operator complexity.** Fine: about 27 neighbour blocks of `3 x 3` per node, `nnz_0 ~ 81 n_0` (less at the boundary).
   Level 1: `n_0 / 81` aggregates of 6 unknowns with about 27 neighbours (fewer at the boundary, `(22/8)^3 ~ 21` for `n = 23`):
   `nnz_1 ~ (n_0 / 81) x 36 x 21`, ratio `nnz_1 / nnz_0 ~ 36 x 21 / 81^2 ~ 0.12 to 0.15`; the next level is about 20 times
   smaller again. Estimate `1.13 to 1.25` for Hex8. Tet10: more neighbours per node and less regular aggregates; estimate
   `1.15 to 1.4`. Limit 1.5 (acceptance).
5. **Iterations (experience, not derivation).** From the literature on smoothed aggregation for 3D elasticity with the six
   rigid-body modes (Vanek, Mandel and Brezina, Computing 56 (1996) 179; Vanek, Brezina and Mandel, Numer. Math. 88 (2001) 559;
   Adams, 2002, from memory): a V(1,1)-cycle with a Gauss-Seidel type smoother as a CG preconditioner reduces the residual by
   about 0.3 to 0.5 per iteration, nearly independent of the mesh, with a slow growth with the number of levels. To `1e-10`:
   `ln(1e-10) / ln(0.3 to 0.5) = 19 to 33`. Estimate: `n = 11` 15 to 28 (two levels, exact coarse solve), `n = 23` 17 to 32,
   `n = 46` 18 to 36; growth 1.05 to 1.35. The limits 39 and 1.5 are the acceptance's; the margin is small at the top of the
   estimate.
6. **Roundoff of B2.** `(D^{-1} A B)_i` over a row with about 81 terms, each `|a_ij| |B_j|`, cancels to 0: roundoff `~ 81 x
   1.1e-16 x |B|_row`, so the Frobenius defect over the interior rows `~ 1e-14 |B|`, times `omega ~ 0.6`. Limit 1e-10 keeps four
   orders of margin and still fails at any real error (a wrong mode gives `O(1)`).
7. **Cantilever of slenderness 20.** IC(0) counts of ADR 0357 / 0282 at the same element size `1/12`: 255 at L/h 5.3 (103 968
   unknowns); doubling the slenderness at fixed element size (85 to 206 on the `1/5.3`-size pair) multiplied the count by
   2.4, i.e. by `s^1.27`. From 5.3 to 20 (`s = 3.75`): `255 x 3.75^1.27 ~ 1370` (estimate 900 to 2000), so the limit for AMG is
   180 to 400. Smoothed aggregation with the rotations represents the bending of each aggregate exactly, so the slenderness
   enters only through the coarsest levels: estimate 20 to 60. Met with a wide margin. Tet10: the quadratic element couples
   more nodes and the point-block smoother is less effective on mid-edge nodes; estimate `1.1 to 1.8 x` the Hex8 count (limit
   2).
8. **Run times (estimates).** `n = 46` cube: `nnz_0 ~ 2.5e7`, setup a few `1e9` flops (the products `A P_t`, `A P`, `P^T
   (A P)`), a V-cycle about 5 matrix-vector products: 30 iterations ~ 10 to 30 s; memory about 1.5 GB with the stiffness.
   Slenderness 20: IC(0) at 1000 to 2000 iterations of a `1e7`-entry matrix: 1 to 3 minutes. Both heavy records.

## Design

### D1. Near-null space (`rcfd_linalg::amg_elastic::rigid_body_modes`)

`rigid_body_modes(points: &[[f64; 3]], components: &[usize]) -> Vec<Vec<f64>>`: row `r` has the point `points[r]` and the
component `components[r] in 0..3`. With `x_0` the mean of the rows' points and `d = x_r - x_0`: columns 0 to 2 the translations
`(e_c)_r = [components[r] == c]`, columns 3 to 5 the rotations about the axes `e_a x d`: `a = 0: (0, -d_z, d_y)`, `a = 1:
(d_z, 0, -d_x)`, `a = 2: (-d_y, d_x, 0)`, component `components[r]`. In `rcfd-fem` the rows are the free equations, the point
of row `r` is the node of `free[r]`, the component its component. (First principles: the rigid displacements `u = t + w x x`.)

### D2. Aggregation on the block graph

Blocks: `block_ptr` (as for `BlockJacobi`; in `rcfd-fem` `node_blocks(free, dofs)`; on a coarse level one block per aggregate).
For blocks `I != J` with any stored entry of `A_IJ`, the weight `w_IJ = |A_IJ|_F`; `J` is a strong neighbour of `I` if `w_IJ
>= theta_l sqrt(|A_II|_F |A_JJ|_F)`, `theta_l = theta 0.5^l` (Vanek, Mandel, Brezina 1996, from memory; the same halving as
`amg.rs`), registered `theta = 0` (every stored neighbour is strong: the meshes here have unit aspect ratio, so the threshold
has nothing to separate). Greedy aggregation (VMB 1996, as in `amg.rs`, on blocks): pass 1, in block order, a block whose
strong neighbours are all unaggregated becomes a seed with all of them; pass 2, each remaining block joins the pass-1
aggregate of its strong neighbour with the largest `w_IJ` (first in column order on ties); pass 3, a block still left (no
strong neighbour aggregated) becomes a singleton. Sequential, deterministic.

### D3. Tentative prolongator by a local QR, with a registered column-drop rule

For each aggregate (in aggregate order) the rows of its blocks (ascending) and the `k` near-null columns (level 0: the six of
D1 in the order `t_0, t_1, t_2, r_0, r_1, r_2`; level `l`: the coarse representation `B_l`) form `B_a` (`m x k`). Modified
Gram-Schmidt with one full reorthogonalisation pass: column `j` is orthogonalised twice against the kept columns; it is
**dropped** if its norm after that is `<= drop_tolerance x` its norm before (registered `drop_tolerance = 1e-10`, arithmetic
1) or if its original norm is 0; else normalised and kept. `Q_a` (`m x k_a`, `k_a <= k`) are the rows of `P_t` in the
aggregate's coarse columns `offset_a .. offset_a + k_a`; the coarse representation is `B_{l+1}[offset_a + i][c] = (Q_a^T
B_a)[i][c]`. Then `P_t^T P_t = I` (disjoint supports, orthonormal columns) and `P_t B_{l+1} = Q Q^T B_l`, which equals `B_l`
up to roundoff whenever every dropped column is in the span of the kept ones. An aggregate with `k_a = 0` gives no coarse
unknowns. The coarse block of an aggregate is its `k_a` unknowns.

### D4. Smoothed prolongator

`P = P_t - omega D^{-1} (A P_t)`, `D^{-1}` the block-diagonal inverse of `A` (the inverse of each `A_II`, by a dense Cholesky;
a non-positive pivot is refused), `omega = (4/3) / rho`, `rho` the spectral radius of `D^{-1} A` from 15 power steps from the
start vector of `amg.rs`, times 1.1 (the safety margin of `amg.rs`). No filtering (with `theta = 0` nothing is weak). The
coarse operator is `A_c = P^T (A P)` (`transpose`, `matmul`).

### D5. Hierarchy and coarsest solve

Settings `ElasticAmgSettings { theta: 0.0, max_coarse: 1000, max_levels: 10, pre_sweeps: 1, post_sweeps: 1, drop_tolerance:
1e-10 }` (default). A level is coarsened while its order is `> max_coarse` and fewer than `max_levels` levels exist; it
becomes the coarsest when the coarse order would exceed `0.85 x` its order (as `amg.rs`). The coarsest level is factored by a
dense LU with partial pivoting (a private copy of the textbook algorithm); a coarsest order above 6000 is refused
(`ElasticAmgError::CoarseTooLarge`). The hierarchy exposes for the tests: `n_levels`, `operator_complexity`, `matrix(l)`,
`tentative(l)`, `prolongator(l)`, `near_null(l)`, `block_ptr(l)`, `aggregates(l)`, `omega(l)`.

### D6. Cycle (the preconditioner)

One V-cycle from zero: on each level `pre_sweeps` forward block Gauss-Seidel sweeps (block `I` in ascending order:
`x_I <- A_II^{-1} (b_I - sum_{J != I} A_IJ x_J)`), the residual, restriction by `P^T`, the coarse correction (one visit, the
coarsest solved by the LU), prolongation by `P`, `post_sweeps` backward block Gauss-Seidel sweeps (blocks in descending
order). The backward sweep is the adjoint of the forward one, the restriction the transpose of the prolongation, so the cycle
is a symmetric operator (up to the roundoff asymmetry of `P^T (A P)`) and positive definite (a convergent smoother for SPD
`A`): a valid CG preconditioner. Everything in setup and cycle is sequential except `matmul` and `matvec`, which are
deterministic per row; CG's reductions are chunked (ADR 0282 D6). So counts and bits do not depend on the thread count.

### D7. Selection in the static solve (`rcfd_fem::solve`, additive)

New variant `CgPreconditioner::SmoothedAggregation(ElasticAmgSettings)`, solver name `"cg-amg-rbm"`. In `preconditioned_cg`
one new arm: the rows' points and components from `dofs.node_component(free[r])` and the mesh nodes, `rigid_body_modes`,
`ElasticAmg::new(a, &node_blocks(free, dofs), &modes, settings)`; a refusal becomes `FemError::Solver { solver: "cg-amg-rbm",
reason }`. The CG loop, the refinement and the stopping rule after the preconditioner are unchanged and shared. To get the
node coordinates the private `preconditioned_cg` gains a parameter `nodes: &[Vec3]` (its signature and its one call change;
the other arms ignore it).

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a) reading 1 (the tentative
prolongator carries c1; the smoothed one is checked where `A B = 0`); (b) `theta = 0`, one pre- and one post-sweep,
`max_coarse = 1000`, `drop_tolerance = 1e-10`; (c) a new file beside `amg.rs`, nothing shared with it; (d) reading 5 for the
Tet10 clause.

## Registered checks

New test files, release profile, one run each:

* `crates/rcfd-linalg/tests/si_amg_elastic.rs` (Q1, Q2): `cargo test -p rcfd-linalg --release --test si_amg_elastic --
  --nocapture`.
* `crates/rcfd-fem/tests/si_amg.rs` (fast: B1, B2, N1, N2, G1, S1, S2, AG1, C-small, A1, TH, OC, P1, X1; heavy `#[ignore]`:
  H1 cube 46, H2 Hex8 slenderness 20, H3 Tet10 slenderness 20, H4 ADR 0282 heavy cantilever): `cargo test -p rcfd-fem
  --release --test si_amg -- --nocapture --test-threads=1`, the heavy ones once each with `--ignored` (one at a time).

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| B1 | c1, reading 1 | every level of the hierarchies of the cubes 11 and 23, the fast cantilever and P1 (fast), and of H1 to H3 (heavy): `|B_l - P_t,l (P_t,l^T B_l)| / |B_l|` (level 0 with the test's own `B`) and, `l >= 1`, `|B_l - P_t,l B_{l+1}| / |B_l|` | `<= 1e-12` | D3, arithmetic 1: roundoff `~1e-15` |
| B2 | reading 1 | level 0 of the cubes 11 and 23: smoothed `P`, rows coupled to no prescribed equation: `|(B - P (P_t^T B))_rows| / |B|` | `<= 1e-10` | arithmetic 6 |
| G1 | D4 | cube 11, every level: `A_{l+1}` against `P_l^T (A_l P_l)` recomputed in the test, max entry difference / max entry | `<= 1e-12` | same operations |
| S1 | D4 | cube 11, level 0 and 1: `P` against `P_t - omega D^{-1} A P_t` recomputed in the test (block inverses solved in the test), max entry difference / max entry of `P` | `<= 1e-10` | dense 3x3 / 6x6 inverses of conditioned blocks |
| S2 | D6 | cube 11 and fast cantilever: `x = sin(1 + i)`, `y = cos(2 + 3i)`: `|x . M y - y . M x| / (|x| |M y|)` and `x . M x > 0`, `y . M y > 0` | `<= 1e-10`, positive | D6 (roundoff asymmetry only) |
| AG1 | D2 | cube 11, level 0: no singleton aggregate; aggregate count recorded (estimate about 64) | none singleton | arithmetic 2 |
| Q1 | D3, arithmetic 1 | `si_amg_elastic.rs`: the local QR of D3 (the same function the hierarchy calls, public as `tentative_block(b_rows, tolerance)`, returning `Q` and the kept count) on the modes of D1 of small node sets, with `|B - Q Q^T B| / |B|`: one node 3 components -> 3; two nodes -> 5; three collinear -> 5; three non-collinear -> 6; one node with components `(0, 1)` -> 2; each defect | counts as stated, defect `<= 1e-12` | arithmetic 1 |
| Q2 | D1 | `rigid_body_modes` on 4 non-coplanar points with all three components: every column stretches no segment (`|(u_i - u_j) . (x_i - x_j)| <= 1e-14 |u| |x_i - x_j|` for all pairs and columns), and the six columns have rank 6 (the smallest pivot of their Gram matrix's Cholesky `> 1e-6` of the largest) | as stated | first principles |
| C-small | c2, c3, reading 3 | cubes 11 and 23: `Ok`, relative residual `<= 1e-10`, count `<= 39` | as stated | arithmetic 5: 15 to 32 |
| A1 | c6 | cube 11: against `Direct`, `max |u - u_d| / max |u_d|` | `<= 1e-7` | CG to 1e-10 on a cond. `~1e5` system |
| TH | c8 | cubes 11 and 23: 1 and 8 threads (pools and `threads(t)`): counts equal, displacement bit-equal | equal | D6 |
| OC | c7 | every hierarchy of a registered run (fast: cubes 11, 23, fast and slender cantilever, P1; heavy: H1 to H4) | `<= 1.5` | arithmetic 4: 1.13 to 1.4 |
| P1 | D3 (partial blocks) | cube 11 with symmetry constraints only (`u_x = 0` on `xmin`, `u_y = 0` on `ymin`, `u_z = 0` on `zmin`; nodes with 1 or 2 free components), traction `(1e6, 0, 0)` on `xmax`: `Ok`, relative residual `<= 1e-10`, against `Direct` `<= 1e-7`, B1 on every level; count recorded | as stated | D3 handles partial blocks |
| X1 | record (brief) | fast `[32, 6, 6]` and slender `[64, 6, 6]` cantilevers of ADR 0282: AMG count, residual, against direct, levels, OC, time; set beside ADR 0357's BJ / IC0 / IC1 (156 / 85 / 58, 292 / 206 / 132) | record (OC and B1 limits apply) | arithmetic 5 |
| H1 | c2, c3, c7 | heavy: cube 46: `Ok`, residual `<= 1e-10`, count `<= 39`, `it(46) / it(11) <= 1.5` (`it(11)` measured again in the same test), OC, B1 | as stated | arithmetic 5: growth 1.05 to 1.35 |
| H2 | c4 | heavy: Hex8 slenderness 20 (121 680 unknowns): `it_AMG <= it_IC0 / 5`, both `Ok` with residual `<= 1e-10`; OC, B1 | as stated | arithmetic 7: IC0 900 to 2000, AMG 20 to 60 |
| H3 | c5 | heavy: Tet10 slenderness 20 (121 680 unknowns): `it_AMG,Tet10 <= 2 it_AMG,Hex8` (H2's count, measured again in the same test), `Ok` with residual `<= 1e-10`; IC0 ratio recorded; OC, B1 | as stated | arithmetic 7: 1.1 to 1.8 |
| H4 | record (brief) | heavy: ADR 0282 heavy cantilever `[96, 18, 18]` (103 968): AMG count beside 477 / 255 / 171; OC | record (OC limit applies) | arithmetic 5 |
| D1 | c10 | `sb_pcg_fill_defaults.rs` (existing, unedited) on this branch and on a `git archive` of bc57e402 (own target): the four hashes equal, and equal to ADR 0357's (`21c39eb0c554d5a4`, `4035e2fb5d4197f5`, `d21ed14c8761bfec`, `0a30d9ef9a8a932a`) | equal | D7: existing arms unedited |
| D2 | c9, c10 | existing tests unedited, release: `rcfd-linalg` lib and test files, `rcfd-fem` lib and test files (ignored records not run), `rcfd-fv` lib | pass | additive only |
| D3 | c10 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no default path touched |
| D4 | c9 | `git diff bc57e402 -- crates/rcfd-linalg/src/amg.rs crates/rcfd-linalg/tests/amg_contrast.rs` | empty | add, don't alter |

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | cube 11: a hierarchy built from the three translations only (`&modes[..3]`): B1's level-0 measure with the six modes; its AMG-CG count (the same CG, through `rcfd_linalg::cg` with the true-residual target) recorded beside the six-mode count | measure `>= 1e-3` |
| N2 | cube 11, level 0: B2's measure over **all** rows | `>= 1e-6` |
| N3 | cube 11: the AMG solution against the IC(0) solution (TH's bit measure) | not bit-equal |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f402-scratch/mut`, own target directory; each applied
alone by exact text replacement, the fast tests of `si_amg` and `si_amg_elastic` run once, the file restored and compared by
SHA-256):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | D7: the rotations dropped from the near-null space (only the three translations passed to `ElasticAmg::new`) | B1 |
| M2 | D7 / D1: the rotation about `z` written `(d_y, d_x, 0)` (sign error, not rigid) | B1, Q2 |
| M3 | D4: no smoothing of the prolongator (`P = P_t`) | S1 |
| M4 | D6: the post-smoother sweeps forward instead of backward | S2 |
| M5 | D4: the coarse operator `P_t^T (A P)` (tentative on the left) | G1, S2 |
| M6 | D3: no column dropped (a dependent column normalised and kept) | Q1 |
| M7 | D2: pass 2 skipped (leftovers become singletons) | AG1 |
| M8 | D6: the coarse correction not added (`x += P x_c` removed) | C-small (count `> 39` or not converged) |

**Decision rule.** F-402 is proposed **done** (pending the maintainer's acceptance) if B1, B2, G1, S1, S2, AG1, Q1, Q2,
C-small, A1, TH, OC, P1 hold, H1, H2 and H3 were run and hold, X1 and H4 were run and recorded, D1 to D4 hold, N1 to N3 hold,
and every mutation fails its registered check in the registered run. **Partial** if any of these checks misses (the miss
becomes a `*_known_miss` record with the limit intact), or a heavy check was not run, or a negative control fails, or (Rule I)
a registered mutation is not caught by its registered check, or is caught only after a post-run amendment. A miss of D1, D2,
D3 or D4 (a default or the scalar AMG changed): **todo** until repaired. No crate that builds: **todo**.

## Limits (what is not covered)

- Three unknowns per node only in the near-null space of D1 (solids); shells (six per node) and the modal solve are not
  covered (the stub's note).
- The strength threshold is 0: anisotropic or stretched meshes would need a threshold and filtering; not tested.
- Setup and smoother are sequential; for the 3e5 case the setup is several seconds.
- One cycle shape: V(1,1) with block Gauss-Seidel; no Chebyshev smoother, no W-cycle, no adaptive near-null space.
- The coarsest solve is dense (order `<= 6000`); larger coarsest levels are refused.
- No case-file key: the selection is through the API only (`StaticSolver::PreconditionedCg`), as for ADR 0357.

## Amendment 1 (2026-11-12, after the first registered run of the fast tests, before the corrected run)

**What happened.** Code d23479b3. Run 1 (logs `f402-scratch/run1-si_amg_elastic.log`, `run1-si_amg.log`), each test once:
`si_amg_elastic` Q1 and Q2 passed; in `si_amg` the tests `cubes_solve`, `hierarchy_cubes` and `n1_translations_only` passed
and `x1_cantilevers_and_p1` **failed at S2 on the fast cantilever: asymmetry 1.422e-4 against the limit 1e-10** (cube 11:
4.4e-17). That test stopped at the assertion, so the slender record of X1 and P1 did not run. The numbers of run 1 are recorded
in addendum 1 and are not replaced.

**Diagnosis** (scratch copy `f402-scratch/diag`, probes `diag.rs`, `diag2.rs`, no file of the branch used): the asymmetry is
the same with no smoothing sweeps (1.9e-4, so it is the coarse correction `P A_c^{-1} P^T`), `A_c` itself is symmetric to
4.7e-16 and its condition number is only 1.4e4, but the dense coarse solve has the residual `|b - A_c x| / |b| = 0.17` on the
fast cantilever (2.7e-15 on cube 11). Cause: `DenseLu::new` swaps whole rows (the stored multipliers move with them, as in
`P A = L U`), while `DenseLu::solve` applies interchange `k` and then eliminates column `k` in turn; that order is correct only
if the multipliers were not moved by later swaps, i.e. only when no row interchange happens. On cube 11 no interchange
happened; on the cantilever's coarsest level (rotational unknowns) some did. Fix: apply all interchanges to `x` first, then
the unit lower and the upper substitution (verified in the scratch copy: residual 1.7e-13, S2 7.2e-17).

**Amendment.** (1) The fix above in `amg_elastic.rs` (`DenseLu::solve` only). (2) The corrected run: every fast test of
`si_amg_elastic` and `si_amg` is run once more on the corrected code (logs `run2-*.log`); the heavy records H1 to H4, the D
checks and the mutations use the corrected code. No limit, no test and no setting is changed. (3) The same interleaved solve is
in the scalar AMG's `DenseLu` (`crates/rcfd-linalg/src/amg.rs`); it is **not** changed here (add, don't alter; it would
change fluid results wherever its coarsest factorisation interchanges rows); it is listed as an open question. (4) By the
registered decision rule a check missed in a registered run (S2 in run 1), so the proposed status is **partial** unless the
maintainer accepts this amendment; addendum 1 reports both runs.

## The planned stub, as written

Status: **PLANNED 2026-11-12** (layout by the intermediate-tier plan, [ADR
0290](0290-structural-solver-intermediate-tier-plan.md)); the registration is written by the implementation agent before any code.
Nothing here is a registered limit: the numbers below are the acceptance draft that the registration turns into checks.

Work package SI-15 of the structural solver, intermediate tier. Size M.

### Goal

The one-level preconditioners of ADR 0282 lose on slender bodies and large models. Add, beside the scalar AMG and without touching
it, a smoothed-aggregation multigrid for matrices with three unknowns per node: aggregation on the node graph, a tentative
prolongator that carries the six rigid-body modes of each aggregate (from the node coordinates, by a local QR), a damped-Jacobi
smoothed prolongator, a block smoother, a dense coarsest solve; used as the preconditioner of CG and selectable in the static
solve. The default solver is unchanged.

### Dependencies

none in this list; roadmap items (SI ids are this tier's packages, ADR 0290 section 7): F-026, F-381.

### Acceptance (draft, in the roadmap's form)

Every limit not stated here is registered in the ADR before the run. On every level the rigid-body modes are in the range of the
prolongator: |B - P B_c| <= 1e-12 |B|. CG preconditioned by one V-cycle reaches a relative residual of 1e-10 on a clamped cube of
Hex8 with about 5e3, 4e4 and (heavy record) 3e5 unknowns in a number of iterations that grows by at most 1.5 times from the
smallest to the largest and stays below 40; on a cantilever of slenderness 20 with about 1e5 unknowns it needs at most one fifth
of the iterations of the incomplete Cholesky of F-381; the same on a Tet10 mesh within twice the Hex8 count. The solution agrees
with the direct solver within 1e-7 on the smallest case; operator complexity <= 1.5; iteration counts and solution are
bit-identical for 1 and 8 threads. The scalar AMG and its tests are unchanged; the default solver's results are unchanged bit for
bit.

### Reference to check against

The reference is the direct solution and the iteration counts themselves. Method, from memory: Vanek, Mandel and Brezina,
Computing 56 (1996) 179 (smoothed aggregation with a near-null space); the rigid-body modes of a node set are translations and x
cross e_i (first principles).

### Files

`crates/rcfd-linalg/src/amg_elastic.rs` (new), `crates/rcfd-linalg/src/lib.rs` (one `pub mod` line),
`crates/rcfd-fem/src/solve.rs` (the solver choice and the modes from the coordinates),
`crates/rcfd-linalg/tests/si_amg_elastic.rs`, `crates/rcfd-fem/tests/si_amg.rs`.

### What could make it miss

Every iteration bound is a draft; none is derived. Constrained unknowns break the block structure (the reduced matrix has nodes
with fewer than three unknowns). Aggregates with too few nodes cannot carry six modes (the local QR is rank deficient and must
drop columns by a registered rule). Six unknowns per node (shells) and the modal solver (which factorises) do not profit: an
iterative eigensolver is left to the advanced tier.
