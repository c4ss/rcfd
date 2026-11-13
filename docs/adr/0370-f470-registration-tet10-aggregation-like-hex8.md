# ADR 0370: F-470 registration: an opt-in geometric aggregation that coarsens Tet10 like Hex8 in the elasticity multigrid

Status: **REGISTERED 2026-11-13, written before any code of F-470 and before any run of its checks**. Not yet accepted by
the maintainer. Before this text nothing of F-470 was written or run except the disclosed probe of the fine matrix under
"Arithmetic before this registration" (facts of the assembled stiffness only: no aggregation, no multigrid, no solve).
Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-470 (deps F-402), from [ADR 0305](0305-aggregation-amg-with-rigid-body-modes.md) remaining question 2,
option (a). Branch `f470-tet10-aggregation` from main 7a533a53, worktree `w49i-work/f470`, scratch `w49i-work/f470-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; a defective set-up gets a corrected test by an amendment
committed before the corrected run; each registered run is made once; existing tests, ignored records, `baseline_hashes.rs`,
the hash tables and the goldens are not edited; defaults bit-identical, the new behaviour opt-in; no new crate, no unsafe
code, no trailers, no push. The scalar AMG `crates/rcfd-linalg/src/amg.rs` is not touched (another item, F-474, works there).

## What F-470 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. In rcfd_linalg::amg_elastic, an opt-in aggregation
> rule for meshes of quadratic elements (a registered strength threshold or an aggregate size bound, derived before the run)
> such that on the Tet10 cantilever of slenderness 20 of ADR 0305 (tet_box 120 x 6 x 6 upgraded, 121 680 unknowns) the first
> coarse level has between 1/15 and 1/6 of the fine unknowns, CG with one V-cycle reaches 1e-8 in at most twice the Hex8 count
> of ADR 0305 H2b (15), operator complexity <= 1.5, the rigid-body modes in the range of the tentative prolongator to 1e-12 on
> every level; every count and bit of ADR 0305 with the default settings unchanged.

Clauses: (c1) an opt-in rule in `rcfd_linalg::amg_elastic` with a parameter derived here; (c2) first coarse level of the
slenderness-20 Tet10 case `n_1 / n_0` in `[1/15, 1/6]`, i.e. `n_1` in `[8112, 20280]` for `n_0 = 121 680`; (c3) CG with one
V-cycle to the relative residual `1e-8` in `<= 30` iterations (2 x 15, the number of ADR 0305 H2b, fixed); (c4) operator
complexity `<= 1.5`; (c5) `|B_l - P_t,l (P_t,l^T B_l)| / |B_l| <= 1e-12` on every level (reading 1 of ADR 0305); (c6) every
count and bit of ADR 0305 with the default settings unchanged.

**Readings (registered here).**

1. (c1) "A registered strength threshold": the rule below is a strength threshold on a **geometric** strength measure
   (the distance between the points of two coupled blocks, relative to the nearest coupled block), with one registered
   number, the ratio `rho`. The probe (arithmetic 2) shows that the algebraic measure `|A_IJ|_F / sqrt(|A_II| |A_JJ|)` does not
   separate the near couplings of Tet10 from the far ones (their ranges overlap), so no algebraic threshold can be derived
   that reproduces the Hex8 neighbourhood; the geometric one can, from the lattice alone. Taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation); listed again as an open question in the addendum.
2. (c3) The count is `SolverStats::iterations` of `StaticSolver::PreconditionedCg { preconditioner:
   CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic()), tolerance: 1e-8, max_iterations: 60 000 }` (the CG
   of ADR 0282 with its refinement restarts, summed), met when the solve returns `Ok`, relative residual `<= 1e-8` and count
   `<= 30`. The Hex8 count 15 is the fixed number of ADR 0305 H2b; it is measured again in the same heavy test as check D5.
3. (c2) "First coarse level": `matrix(1).n` of the hierarchy built by the static solve's route
   (`rcfd_fem::solve::smoothed_aggregation` with `ElasticAmgSettings::quadratic()`) on the reduced stiffness of that case.
4. (c4) `operator_complexity()` of that hierarchy.
5. (c6) Checked by D1 to D6 below: a bit-hash test of the default route on fast cases run on this branch and on a `git
   archive` of 7a533a53; the existing tests unedited; the fast counts of `si_amg` printed equal to ADR 0305's; the Hex8 H2b
   count 15 and the default Tet10 first coarse level 2220 measured again; `baseline_hashes`.

## What was read (main 7a533a53, read only)

1. `AGENT_RULES.md`; `docs/roadmap.json` F-470; ADR 0305 in full (design D1 to D7, amendments 1 and 2, addendum 1: H3b Tet10
   79 iterations, 1 restart, residual 9.31e-9, floor estimate 8.64e-9, first coarse level 2220 of 121 680, OC 1.033; Hex8 H2b
   15, first coarse level 12 000, OC 1.169; diagnosis 3(a): the corner nodes, numbered first, become seeds, aggregates of
   about 110 nodes against about 20 for Hex8).
2. Code: `crates/rcfd-linalg/src/amg_elastic.rs` (all), `crates/rcfd-fem/src/solve.rs` (the `SmoothedAggregation` arm,
   `smoothed_aggregation`, `node_blocks`), `crates/rcfd-fem/src/builders.rs` (`hex_box` numbering `i + (nx+1)(j + (ny+1)k)`,
   x fastest; `tet_box` Kuhn split; `upgrade_to_tet10`: mid-edge nodes numbered after all corners, in element order),
   `crates/rcfd-fem/tests/si_amg.rs` (all; set-ups copied, not imported or edited).
3. **Not read:** any multigrid code outside this repository; the bodies of the papers cited.

## Arithmetic before this registration (disclosed)

1. **Why the default over-coarsens Tet10 (by hand).** In half-step lattice units (the Tet10 nodes of the Kuhn mesh 120 x 6 x 6
   are the lattice of spacing `h' = 1/12`, the same 241 x 13 x 13 points as the Hex8 mesh 240 x 12 x 12), a Tet10 node couples
   to nodes up to Chebyshev distance 2; Hex8 only to Chebyshev distance 1. With `theta = 0` a seed takes every coupled node:
   Hex8 seeds fall every 3 lattice steps (aggregates 3^3 = 27 interior, 20.3 on average with the boundary: 2000 aggregates of
   40 560 nodes, `n_1 = 12 000`); Tet10 seeds (corners first) every 5 to 6 steps, about 110 nodes per aggregate. Even with a
   neighbourhood restricted to Chebyshev distance 1, visiting the corner nodes first (even lattice sites) puts seeds every 4
   steps (an even site at distance 4 is the first whose neighbourhood is free): about 4^3 = 64 nodes per aggregate, `n_1 /
   n_0 ~ 2/64 = 1/32`, outside the window. So both the neighbourhood **and** the visiting order must follow the geometry.
2. **Probe of the fine matrix** (scratch `f470-scratch/probe`, run once, before this text; it assembles the two reduced
   stiffnesses of the slenderness-20 cases and counts; output kept in this section):
   - Hex8 240 x 12 x 12: `n = 121 680`, nnz 8 846 478, 23.23 neighbour blocks per block; every Chebyshev-1 lattice neighbour
     is coupled (942 382 of 942 382); normalised block strength `|A_IJ|_F / sqrt(|A_II|_F |A_JJ|_F)` 0.043 to 0.237.
   - Tet10 120 x 6 x 6 upgraded: `n = 121 680`, nnz 9 515 196, 25.07 neighbour blocks per block; of the 942 382 Chebyshev-1
     lattice neighbour pairs 264 000 (28 percent) are **not** coupled; Chebyshev-1 couplings 678 382, strength min 0.0295, q10
     0.0456, median 0.127, q90 0.266, max 0.562; Chebyshev-2 couplings 338 302, strength min 0.0099, q10 0.0139, median
     0.0247, q90 0.0607, max 0.126; 38 distinct Chebyshev-2 offsets, none with two components `+-2` of opposite sign (the
     Kuhn split couples only along `e_S` directions).
   - Consequence: the algebraic strengths of the two classes overlap (Chebyshev-2 max 0.126 above the Chebyshev-1 q10 0.046),
     so a threshold `theta` would cut both classes; reading 1.
3. **The ratio `rho`.** On a uniform lattice of spacing `h'` the Chebyshev-1 neighbours are at distances `h', sqrt 2 h',
   sqrt 3 h'` and the nearest Chebyshev-2 one at `2 h'`. With `d_I` the distance to the nearest coupled block, the candidate
   set `{J coupled : |p_J - p_I| <= rho d_I}` is exactly the coupled Chebyshev-1 neighbourhood for any `rho` in `[sqrt 3, 2)`;
   registered the middle of the gap, **`rho = (sqrt 3 + 2) / 2 = 1.8660254037844386`** (margin 7.7 percent to either side,
   far above floating-point noise). On a coarse level the points are the aggregates' centroids; on the regular part of the
   coarse grid (spacing `3 h'`) the same gap separates the neighbouring aggregates (`<= 3 sqrt 3 h'`) from the second ring
   (`>= 6 h'`).
4. **Hex8 identity.** On `hex_box` the lexicographic order (z, then y, then x) of the node points is the node numbering, and
   every coupled block is within `sqrt 3 h <= rho h` of a block whose nearest neighbour is at `h`: so on level 0 the rule
   gives exactly the default aggregates of a Hex8 box (check E1). On the coarse levels it may differ from the default (the
   boundary aggregates' centroids are spaced `2.5` and `3` lattice steps, which can drop a diagonal neighbour).
5. **First coarse level of the Tet10 case (estimate).** Candidate sets: 678 382 / 40 560 = 16.7 coupled Chebyshev-1
   neighbours per block on average (about 26 x 0.72 = 18.7 in the interior). Pass-1 aggregates are a seed with its
   candidates, about 15 to 19 blocks; pass 2 adds the leftovers. Estimate 15 to 25 blocks per aggregate, six columns each
   (fewer only next to the clamp): `n_1` about 2 x 121 680 / 3 / (15 to 25) x 3 = 9700 to 16 200, i.e. `1/12.5` to `1/7.5`.
   Window `[8112, 20280]`: met by the estimate with a margin of about 1.2 on the lower and 1.25 on the upper side.
6. **Operator complexity (estimate, the tightest clause).** The smoothed prolongator `(I - omega D^{-1} A) P_t` spreads each
   column by the Tet10 reach 2, so two aggregates of width about 3 couple when their centres are up to about 8 lattice steps
   apart: up to 5 aggregates per direction (the second ring is coupled, unlike Hex8's 3). Interior coarse stencil: the 27 of
   the first ring and about 60 of the 98 of the second (opposite-sign double offsets excluded, arithmetic 2); the cross-section
   of 13 nodes holds about 4.8 aggregates, which cuts a 5-wide stencil to about 0.76 per transverse direction: about 50
   coupled aggregates per coarse block row. `nnz_1 ~ n_agg x 36 x 50`; for 20 blocks per aggregate (2030 aggregates) 3.65e6,
   `nnz_1 / nnz_0 = 3.65e6 / 9.52e6 = 0.38`; for 15 blocks 0.51, for 25 blocks 0.31; the next levels add about 0.01 to 0.03.
   Estimate **OC 1.30 to 1.55**, central 1.40. The limit 1.5 (acceptance) may be missed if the aggregates come out small;
   this is disclosed, not compensated.
7. **Iterations (experience, not derivation).** With Hex8-like aggregates the remaining differences from H2b are the Tet10
   smoother (point-block Gauss-Seidel on mid-edge nodes is weaker) and the irregular aggregates (holes of arithmetic 2).
   Estimate 1.2 to 2.2 times the Hex8 count: 18 to 33, central 25; limit 30 (acceptance). The rounding floor of the Tet10
   case (ADR 0305 H3b: estimate 8.6e-9, true floor about 0.4 of it, as measured for Hex8: 1.42e-9 against 3.6e-9) is about
   3.5e-9, below 1e-8, so a refinement restart may add a few iterations; included in the estimate.
8. **Run times (estimates).** Tet10 slender case: assembly a few seconds, setup 3 to 10 s (the denser coarse product), 25
   V-cycles 10 to 30 s; with the Hex8 reference solve and the default Tet10 setup (D5, D6) 1 to 3 minutes: a heavy record.

## Design

### D1. The rule (`rcfd_linalg::amg_elastic`, additive)

New `#[non_exhaustive] pub enum Aggregation { Greedy, Geometric { ratio: f64 } }` and a new field `aggregation:
Aggregation` of `ElasticAmgSettings`, default `Greedy` (the ADR 0305 rule, code path unchanged). `pub const QUADRATIC_RATIO:
f64 = 1.8660254037844386` (arithmetic 3) and `ElasticAmgSettings::quadratic()` = the default with `Geometric { ratio:
QUADRATIC_RATIO }`. New `ElasticAmg::with_points(a, block_ptr, near_null, points, settings)` with one point per block;
`ElasticAmg::new` builds as before and refuses `Geometric` with the new `ElasticAmgError::MissingPoints`; a points slice of
the wrong length is refused (`PointsLength`). With `Greedy`, `with_points` ignores the points.

`Geometric { ratio }` on level `l` with block points `p` (level 0: given; level `l + 1`: the centroid, unweighted mean, of the
member blocks' points of each aggregate that has coarse unknowns, in coarse block order):

1. Stored neighbours of block `I`: blocks `J != I` with a stored entry in `A_IJ`, weight `w_IJ = |A_IJ|_F` (as D2 of ADR
   0305). `d_I` = the smallest `|p_J - p_I|` (Euclidean) over them. Candidates of `I`: the stored neighbours with
   `|p_J - p_I| <= ratio d_I`. `theta` is not used.
2. Visiting order: the blocks sorted by `(p_z, p_y, p_x)` with `f64::total_cmp`, ties by block index.
3. Pass 1, in that order: a block not yet aggregated whose candidates are all unaggregated becomes a seed with all its
   candidates (new aggregate number, in creation order). Pass 2, in that order: each remaining block joins the pass-1
   aggregate of its candidate with the largest `w_IJ` (candidates in ascending block index, strict `>`, so the first wins on
   ties). Pass 3, in that order: a block still left becomes a singleton.
4. Everything after the aggregation (D3 tentative prolongator, D4 smoothing with the full `A`, D5 hierarchy limits and the
   0.85 rule, D6 cycle) is unchanged.

Derivation: arithmetic 1, 3, 4 (first principles on the lattice); the greedy passes are those of Vanek, Mandel and Brezina,
Computing 56 (1996) 179 (cited from memory, as in ADR 0305), with the strength measured by distance and the seeds visited
in geometric order.

### D2. Selection in the static solve (`rcfd_fem::solve`, inserted lines only)

In `smoothed_aggregation`, before its last line: if `settings.aggregation` is not `Greedy`, the block points (the node of
each block's first free equation) are formed and `ElasticAmg::with_points` is returned. One new `use` line. The variant
`CgPreconditioner::SmoothedAggregation(settings)` carries the choice; no new variant, no existing line of `solve.rs` changes.

## Registered checks

New test files, release profile, each run once: `crates/rcfd-fem/tests/si_amg_quadratic.rs` (fast: DF, E1, E2, E3, E4, E5,
N1; heavy `#[ignore]`: H1), `cargo test -p rcfd-fem --release --test si_amg_quadratic -- --nocapture --test-threads=1`, the
heavy one once with `--ignored`; `crates/rcfd-fem/tests/f470_defaults.rs` (D1).

Fast Tet10 case ("fast Tet10"): `tet_box([0; 3], [8, 1.5, 1.5], [32, 6, 6])` upgraded, `xmin` clamped, traction `(0, 0,
-1e6)` on `xmax`, `E = 2e11`, `nu = 0.3` (65 x 13 x 13 nodes, 32 448 unknowns, expected three levels so that the coarse
centroid rule is exercised).

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| H1a | c2 | heavy: slenderness-20 Tet10, `quadratic()`: `matrix(1).n` | in `[8112, 20280]` | arithmetic 5: 9700 to 16 200 |
| H1b | c3 | heavy: the same, CG with one V-cycle to 1e-8: `Ok`, relative residual `<= 1e-8`, count | `<= 30` | arithmetic 7: 18 to 33 |
| H1c | c4 | heavy: operator complexity of that hierarchy | `<= 1.5` | arithmetic 6: 1.30 to 1.55 |
| H1d | c5 | heavy: B1 of ADR 0305 on every level (level 0 with the test's own modes; `l >= 1` also `|B_l - P_t B_{l+1}| / |B_l|`) | `<= 1e-12` | D3 unchanged: roundoff |
| DF | c1, c6 | `ElasticAmgSettings::default()` equals the ADR 0305 values with `aggregation == Greedy`; `quadratic()` differs only by `Geometric { ratio }` with `|ratio - (sqrt 3 + 2) / 2| <= 1e-15` | as stated | D1 |
| E1 | D1, arithmetic 4 | Hex8 cube 11 (ADR 0305 reading 3): level-0 aggregates of `quadratic()` equal the default's, and the AMG-CG solutions to 1e-10 are bit-equal with equal counts | equal | arithmetic 4 (two levels, identical level 0) |
| E2 | D1 | fast Tet10: the library's level-0 and level-1 aggregates equal those of an independent implementation of D1 written in the test (its own ratio constant, its own points from the mesh, level-1 centroids from the library's level-0 aggregates, the library's level-1 matrix) | equal | same rule |
| E3 | c3 to c5 (fast) | fast Tet10, `quadratic()`: B1 every level `<= 1e-12`; S2 of ADR 0305 (symmetry `<= 1e-10`, both forms positive); CG to 1e-8 `Ok` with residual `<= 1e-8`; recorded: levels, `n_1 / n_0`, OC, count, the default count | as stated; the rest recorded | D3, D6 unchanged |
| E4 | ADR 0305 c8 | fast Tet10, `quadratic()`: 1 and 8 threads (pools and `threads(t)`): counts equal, displacements bit-equal | equal | D1 sequential |
| E5 | D1 | `ElasticAmg::new` with `quadratic()` returns `Err(MissingPoints)`; `with_points` with one point too few returns `Err(PointsLength)` | as stated | D1 |
| D1 | c6 | `f470_defaults.rs`: default `SmoothedAggregation` on cube 11, cube 23, the fast Hex8 cantilever `[32, 6, 6]` on `[8, 1.5, 1.5]`, P1 and fast Tet10: printed count, level orders and an FNV-1a hash of the displacement bits; run on this branch and on a `git archive` of 7a533a53 (own target, the same file copied in) | identical output lines | default path unchanged |
| D2 | c6 | existing tests unedited, release: `cargo test -p rcfd-linalg --release`, `cargo test -p rcfd-fem --release` (ignored records not run); the counts printed by `si_amg` equal ADR 0305's (cube 16 / 17, X1 15 / 22, P1 16, AG1 64 aggregates, N1 29 / 16) | pass, equal | default path unchanged |
| D3 | c6 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no default path touched |
| D4 | scope | `git diff 7a533a53 -- crates/rcfd-linalg/src/amg.rs` | empty | not touched |
| D5 | c6 | heavy (in H1): Hex8 slenderness 20, default settings, CG to 1e-8 (ADR 0305 H2b) | count `== 15` | default path unchanged |
| D6 | c6 | heavy (in H1): Tet10 slenderness 20, default settings: `matrix(1).n` | `== 2220` | ADR 0305 addendum |

Records in H1: levels (order, nnz), setup and solve time, restarts, floor estimate `eps |(|K_FF| |u_F|)| / |b_F|`, the Hex8
case with `quadratic()` (count, levels).

**Negative controls.**

| Id | Control | Expected |
| --- | --- | --- |
| N1 | fast Tet10: `matrix(1).n` with `quadratic()` over the default's | `>= 2` (the default over-coarsens; arithmetic 1) |
| N2 | heavy: D6's default first coarse level against the window of H1a | outside `[8112, 20280]` |

**Mutations** (on a `git archive` copy of the final code in `f470-scratch/mut`, own target directory; each applied alone
by exact text replacement, the fast tests of `si_amg_quadratic` run once, the file restored and compared by SHA-256):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | D1.2: pass 1 visits the blocks in index order (no geometric sort) | E2 |
| M2 | `QUADRATIC_RATIO` set to `sqrt 3` (1.7320508075688772) | DF, E2 |
| M3 | D1.1: `d_I` the largest instead of the smallest neighbour distance | E2 |
| M4 | D1: coarse points the first member's point instead of the centroid | E2 (level 1) |
| M5 | D1.3: pass 2 over all stored neighbours instead of the candidates | E2 |
| M6 | `ElasticAmgSettings::default()` with `aggregation: Geometric { ratio: QUADRATIC_RATIO }` | DF |
| M7 | D2: the new branch of `smoothed_aggregation` removed (always `ElasticAmg::new`) | E3 (the solve is refused with `MissingPoints`) |

**Decision rule.** F-470 is proposed **done** (pending the maintainer's acceptance) if H1a to H1d, DF, E1 to E5 and D1 to D6
hold in their registered runs, N1 and N2 hold, and every mutation fails its registered check in the registered run.
**Partial** if any of these misses (the miss becomes a `*_known_miss` record with the limit intact), the heavy test was not
run, a negative control fails, or (Rule I) a registered mutation is not caught by its registered check or is caught only
after a post-run amendment. A miss of D1 to D6 (a default changed): **todo** until repaired. No crate that builds: **todo**.

## Limits (what is not covered)

- The rule needs a point per block; for coarse levels it uses centroids. On graded or stretched meshes the relative distance
  `rho d_I` follows the local spacing, but anisotropic meshes are not tested.
- Only the Kuhn Tet10 box of ADR 0305 and a Hex8 cube are measured; other quadratic kinds (Hex20, Hex27, Wedge15) are not.
- The prolongator is smoothed with the full Tet10 operator (reach 2); a filtered smoothing that would lower the operator
  complexity is not part of this item.
- No case-file key (F-386).

## The probe, as run

`f470-scratch/probe/src/main.rs`: assembles the two reduced stiffnesses with the library's `stiffness_with`, `reduce` and
`node_blocks` and prints the numbers of arithmetic 2. It calls no multigrid code.

## Addendum 1: results (2026-11-13)

Code: registration 340810da; library and tests b0c025c0 (`crates/rcfd-linalg/src/amg_elastic.rs`: lines inserted at new
30-31 (field), 43-72 (`Aggregation`, `QUADRATIC_RATIO`, `quadratic()`), 105-113 and 130-133 (two error variants), 465-574
(`aggregate_geometric`, `coarse_points`), 729-765 (`new` refuses `Geometric` and delegates to the private `build`;
`with_points`), and one changed line, old 596 / new 788-792 (the `aggregate_blocks` call became a `match` whose default arm is
that same call), new 794-798 (coarse points); `crates/rcfd-fem/src/solve.rs`: lines inserted only, new 23 (`use`) and 275-280
(the branch of `smoothed_aggregation`); tests `crates/rcfd-fem/tests/si_amg_quadratic.rs`, `crates/rcfd-fem/tests/f470_defaults.rs`);
165efa90 the known-miss rename after the heavy run. Release profile throughout. Logs in `w49i-work/f470-scratch`:
`run1-si_amg_quadratic.log` (fast, once), `run1-h1.log` (heavy, once), `diag-h1b.log` (diagnosis), `run-d1-branch.log`,
`run-d1-main.log` (the same file on a `git archive` of 7a533a53, own target), `run-d2-linalg.log`, `run-d2-fem.log`,
`run-d3-baseline.log`, `mutations.log` (`mutate.py`).

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| H1a first coarse level, slenderness-20 Tet10 | in `[8112, 20280]` | **12 000** of 121 680 (1/10.14; exactly the Hex8 number) | yes |
| H1b CG with one V-cycle to 1e-8 | `<= 30`, residual `<= 1e-8` | **31** iterations, 0 restarts, residual 9.17e-9 (floor estimate 8.64e-9), solve 3.1 s | **no** (`h1_slender20_tet10_quadratic_known_miss`) |
| H1c operator complexity | `<= 1.5` | 1.4762 (levels 121 680 / 12 000 / 522; nnz 9.52e6 / 4.47e6 / 6.5e4), setup 0.95 s | yes |
| H1d B1 every level | `<= 1e-12` | 2.8e-16 | yes |
| DF settings | as stated | default = ADR 0305 with `Greedy`; ratio 1.8660254037844386 | yes |
| E1 Hex8 cube 11 identity | equal | level-0 aggregates equal, counts 16 / 16, bit-equal | yes |
| E2 reference aggregation, fast Tet10 | equal | level 0 (550 aggregates) and level 1 equal | yes |
| E3 fast Tet10 | B1 `<= 1e-12`, S2 `<= 1e-10` positive, `Ok` `<= 1e-8` | B1 2.9e-16, S2 1.7e-16, positive; 27 iterations, 8.2e-9 (default 40); records: levels 32 448 / 3300 / 144, `n_1/n_0` 1/9.83, OC 1.4787 (default 1.0342) | yes |
| E4 1 and 8 threads | equal | 27 / 27, bit-equal | yes |
| E5 refusals | as stated | `MissingPoints`; `PointsLength { expected: 48, got: 47 }` | yes |
| N1 quadratic / default first coarse level | `>= 2` | 3300 / 594 = 5.56 | yes |
| N2 default outside the window | outside | 2220 | yes |
| D1 default-route hashes branch / 7a533a53 | identical | identical five lines (cube 11 `63183f7b70a37472` 16 its, cube 23 `f4e0b9eacf326034` 17, fast Hex8 `eac2e37cdbf7447a` 15, P1 `6a5b29ee81bc9dc7` 16, fast Tet10 `3000b383efab3700` 40) | yes |
| D2 existing tests | pass, counts equal | `rcfd-linalg` and `rcfd-fem` release, every test file passes (ignored not run); `si_amg` prints 16 / 17, 15 / 22, P1 16, AG1 64, N1 29 / 16 as ADR 0305 | yes |
| D3 baseline_hashes | pass | 3 passed, 1 ignored | yes |
| D4 scalar AMG | empty diff | empty | yes |
| D5 Hex8 slenderness 20 default at 1e-8 | `== 15` | 15 | yes |
| D6 Tet10 default first coarse level | `== 2220` | 2220 | yes |

Record: the Hex8 slenderness-20 case with `quadratic()`: 15 iterations (as the default).

**Diagnosis of the H1b miss** (`diag-h1b.log`, a diagnostic solve of the same case at looser tolerances, not a registered
check, nothing tuned): 20 iterations to 1e-4, 23 to 1e-5, 26 to 1e-6, 28 to 1e-7, 31 to 1e-8, no restart at any tolerance.
The convergence is steady to the end (about 3 iterations per decade, factor about 0.46 per iteration, against about 0.28 for
Hex8 and 0.79 for the default Tet10 hierarchy), so the residual floor is not the cause. The coarsening clause is met exactly
(12 000 coarse unknowns, as Hex8), so what remains is the cycle on the Tet10 operator: the point-block Gauss-Seidel smoother
on mid-edge nodes (one pre- and one post-sweep) and the reach-2 prolongator smoothing (OC 1.48, as estimated in arithmetic 6).
The count fell from 79 to 31 (2.5 times fewer); the miss is one iteration, within the registered estimate 18 to 33.

**Mutations** (archive of 165efa90 in `f470-scratch/mut`, own target, each alone, files restored and compared by SHA-256, all
restored):

| Id | Registered | Caught by |
| --- | --- | --- |
| M1 no geometric order | E2 | E2 (the aggregate count differs from the reference) |
| M2 ratio `sqrt 3` | DF, E2 | DF, E2, and E1 |
| M3 `d_I` the largest distance | E2 | E2 (aggregate count) |
| M4 coarse point the first member | E2 (level 1) | E2 (level 1) |
| M5 pass 2 over all stored neighbours | E2 | **not caught**: every check held. On these meshes the strongest coupled neighbour of every pass-2 block is already one of its candidates, so the mutation changes no aggregate (equivalent here); no check registered can see it |
| M6 default `Geometric` | DF | DF (and N1) |
| M7 branch of `smoothed_aggregation` removed | E3 | E3 (`MissingPoints`), and E1, E2 |

`cargo fmt -p rcfd-linalg --check`, `cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-linalg -p rcfd-fem
--all-targets --release -- -D warnings`: clean.

**Deviations.**

1. Arithmetic 4 predicted that the coarse levels might differ from the default on Hex8; the record shows the same count (15)
   on the slenderness-20 Hex8 case; level orders of that run were not printed.
2. E2's assertion that every aggregate has coarse unknowns (a precondition of the level-1 comparison) is what failed first
   under M1 and M3; it is part of E2.
3. Not run: the whole workspace (the lead runs the gate); the heavy records of ADR 0305 (H1 to H4, H2b) except H2b's count,
   which D5 measured again.

**Decision by the registered rule: partial.** H1b missed in its registered run (31 against 30), and (Rule I) the registered
mutation M5 was not caught by its registered check E2. Every other clause holds: the first coarse level is 1/10.14 of the fine
one (window 1/15 to 1/6), operator complexity 1.476, the rigid-body modes are in the range of the tentative prolongator to
2.8e-16 on every level, and the defaults are unchanged bit for bit (D1 to D6).

**Remaining questions.**

1. The iteration limit (H1b). Options: (a) a follow-up item for the cycle on quadratic elements: a registered smoother change
   (two pre- and post-sweeps, or a Chebyshev smoother) with the coarsening of this ADR, against the same limit 30; (b) accept
   31 against 30 (2.5 times fewer than the default, coarsening like Hex8). Recommendation: (a), since the diagnosis places the
   remaining gap in the smoother, not in the aggregation.
2. M5. Options: (a) a test on a mesh where a leftover block's strongest coupling lies outside its candidates (a graded or
   anisotropic Tet10 box), so that pass 2's restriction is observable; (b) drop the restriction from D1 as immaterial.
   Recommendation: (a), as part of the follow-up of question 1.
3. Reading 1 (a geometric strength counts as "a registered strength threshold"), taken on the maintainer's standing
   instruction of 2026-11-11 (take the recommendation); the probe shows an algebraic threshold cannot separate the Tet10
   couplings. Recommendation: accept the reading.
4. Selection from a case file (F-386): an iterative key could map a mesh of quadratic elements onto
   `ElasticAmgSettings::quadratic()`. Recommendation: note it on F-386.

## For the lead: roadmap entries

F-470: proposed status **partial**. Note:

> From ADR 0305 question 2, recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0370, 2026-11-13: rcfd_linalg::amg_elastic::Aggregation::Geometric (opt-in through ElasticAmgSettings::quadratic() and ElasticAmg::with_points; rcfd_fem::solve::smoothed_aggregation passes the node points): a coupled block is a candidate when it lies within (sqrt 3 + 2)/2 of the distance to the nearest coupled block (the gap between the Chebyshev-1 and Chebyshev-2 lattice neighbours), seeds visited in coordinate order, coarse points the aggregate centroids. On the Tet10 cantilever of slenderness 20 (121 680 unknowns) the first coarse level is 12 000 (1/10.14, the Hex8 number; default 2220), operator complexity 1.476, rigid-body modes in the range of the tentative prolongator to 2.8e-16; CG with one V-cycle reaches 1e-8 in 31 iterations (default 79) against the limit 30, steady at about 3 iterations per decade, so the remaining gap is the smoother, not the floor or the coarsening. On Hex8 the rule reproduces the default aggregates bit for bit. Defaults unchanged bit for bit (hashes against main, ADR 0305 counts 15 and 2220 measured again, baseline hashes pass). Partial by Rule I: the iteration limit missed by one, and one of seven mutations (pass 2 over all neighbours) equivalent on these meshes and not caught. Follow-up: the smoother on quadratic elements.

Follow-up item (new), title "Elasticity multigrid on quadratic tetrahedra: a smoother that brings Tet10 within twice the Hex8 count", deps F-470:

> Every limit not stated here is registered in the ADR before the run. With the geometric aggregation of ADR 0370 (ElasticAmgSettings::quadratic()), a registered opt-in smoother setting for quadratic elements (more block Gauss-Seidel sweeps or a Chebyshev smoother, derived before the run) such that on the Tet10 cantilever of slenderness 20 of ADR 0305 CG with one V-cycle reaches 1e-8 in at most 30 iterations with operator complexity <= 1.5 and the setup and solve time recorded beside ADR 0370's (31 iterations, 3.1 s); a test on a graded or anisotropic Tet10 box where pass 2's restriction to the candidates changes the aggregates (ADR 0370 mutation M5); every count and bit of ADR 0305 and ADR 0370 with their settings unchanged.

F-386 (solid case file), sentence to append to its note:

> ADR 0370 (2026-11-13): for meshes of quadratic elements an iterative key should be able to select CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic()) (Tet10 slenderness 20: 31 iterations against 79 with the default settings).
