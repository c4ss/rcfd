# ADR 0357: F-460 registration: incomplete Cholesky with one level of fill and after a node-wise reverse Cuthill-McKee ordering

Status: **REGISTERED 2026-11-12, written before any code of F-460 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-460 was written, compiled or run: the code and documents under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand and with a short Python script of graph
counts (`w49i-work/f460-scratch/arith.py`, output `arith.log`) that runs no library code and no numerics (it counts the
entries of node-graph patterns of the box). Results go in the addendum at the end; nothing above the addendum is changed after a
run.

Parent: [ADR 0282](0282-preconditioned-cg-for-elasticity.md) (F-381, partial: IC(0) in the natural order needed 85 iterations
against block Jacobi's 156, ratio 0.545 against the limit 0.5; remaining question 1, option (a), is this item). Roadmap item
F-460 (deps F-381). Branch `f460-ic-fill-ordering` from main 0a226bda, worktree `w49i-work/f460`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded, diagnosed and not tuned; each registered run is made once;
existing tests (`sb_pcg.rs` among them), ignored records, `baseline_hashes.rs`, the hash tables and the goldens are not edited;
defaults bit-identical, the new behaviour opt-in; no new crate, no unsafe code, no trailers, no push. Another agent (F-385)
works in `rcfd-fem` on a static co-simulation participant: the change to `crates/rcfd-fem/src/solve.rs` is kept small and
additive (one new enum, one new variant, one new match arm in each of two existing matches, no existing line edited);
`rcfd-linalg/src/precond.rs` and `rcfd-linalg/src/reorder.rs` only gain appended items.

## What F-460 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. Beside IC(0) in the natural order, an incomplete
> Cholesky with one level of fill and IC(0) after a node-wise reverse Cuthill-McKee ordering, both with the shift strategy of
> ADR 0282, selectable in StaticSolver::PreconditionedCg: on the fast Hex8 cantilever of ADR 0282 (32 x 6 x 6, 4704 unknowns)
> at least one of them reaches the relative residual 1e-10 in at most half the iterations of block Jacobi (156), agrees with
> the direct solution within 1e-7 and is bit-identical for 1 and 8 threads; the iteration counts at L/h = 5.3 and 10.7 and on
> the 103 968-unknown case are recorded; IC(0) in the natural order and every default are unchanged bit for bit.

Clauses: (c1) IC with one level of fill, with ADR 0282's shift strategy, selectable in `PreconditionedCg`; (c2) IC(0) after a
node-wise RCM ordering, the same strategy, selectable; (c3) fast cantilever: at least one of (c1), (c2) reaches relative
residual `<= 1e-10` in `<= 0.5 x` block Jacobi's iterations **and** agrees with the direct solution within 1e-7 **and** is
bit-identical for 1 and 8 threads; (c4) iteration counts recorded at L/h = 5.3 (the fast case), 10.7 (the slender case of ADR
0282, `[64, 6, 6]`, 16 m) and on the 103 968-unknown case (ADR 0282 H1c, `[0, 8] x [0, 1.5]^2`, `[96, 18, 18]`); (c5) IC(0)
in the natural order unchanged bit for bit; (c6) every default unchanged bit for bit.

**Readings (registered here).**

1. (c3) is met by a variant only if that **same** variant meets all three conditions on the same run. "Half the iterations of
   block Jacobi (156)": the limit is `iterations <= 0.5 x it_BJ` with `it_BJ` block Jacobi's count measured in the same test on
   this tree, and that count must equal 156 (it did on main, ADR 0282 addendum 1; check A0). So the limit is 78 iterations.
2. "One level of fill" is the level-of-fill rule of ILU(p)/IC(p) (Watts 1981 and Saad's textbook, "Iterative Methods for
   Sparse Linear Systems", section 10.3.3, from memory), on the scalar (equation) graph of the reduced stiffness: the entries of
   the pattern of `A` have level 0; eliminating `k` creates at `(i, j)` (both `> k`) the level `lev(i, k) + lev(k, j) + 1`,
   the minimum over all `k` is kept, and the factor keeps every position of level `<= p`. For `p = 1` this is (derived in
   arithmetic 1) exactly: `(i, j)`, `j < i`, is kept if `a_ij` is stored or some `k < j` has both `a_ik` and `a_jk` stored.
3. "Node-wise reverse Cuthill-McKee": the RCM ordering (`rcfd_linalg::reorder::rcm_order`, F-236: pseudo-peripheral start,
   neighbours by ascending degree, reversed) of the **node graph** of the reduced system (one vertex per block of
   `solve::node_blocks`, an edge where any stored entry couples two blocks); each node's equations stay consecutive and in
   their original order. The stiffness is permuted symmetrically and factored by IC(0); CG itself runs in the original order
   (only the preconditioner is permuted, so `K`, `b`, the matvec and every reduction are the ones of ADR 0282).
4. "Both with the shift strategy of ADR 0282": the same `IcOptions` (`A + alpha diag(A)`, `alpha <- max(2 alpha, 1e-3)` from
   0, pivot test `d_i > 1e-10 (1 + alpha) a_ii`, 64 attempts), the breakdowns reported in `SolveReport` with node and
   component, and the solver name extended by ", shift ... after k breakdown(s)".
5. (c5) and (c6): the existing variants `CgPreconditioner::{BlockJacobi, IncompleteCholesky(_)}`, `StaticSolver::Direct` and
   `ConjugateGradient` produce the same bits as on main 0a226bda (check D1, by hash of the result compiled on both trees),
   the existing tests pass unedited (D2), `baseline_hashes` passes (D3).

## What was read (main 0a226bda, read only)

1. `AGENT_RULES.md`; ADR 0282 in full (design D1 to D7, checks, amendments 1 to 4, addendum 1, remaining question 1 and the
   follow-up text that is F-460's acceptance); ADR 0354 (style); `docs/roadmap.json` F-460.
2. Code: `crates/rcfd-linalg/src/precond.rs` (all: `IncompleteCholesky`, `ic_attempt`, `IcOptions`, `PrecondError`,
   `BlockJacobi`), `reorder.rs` (all: `rcm_order`, `inverse_permutation`, `CsrMatrix::permute_symmetric`), `matrix.rs`
   (`CsrMatrix`, `from_triplets` keeps stored zeros and sums duplicates, `matvec`, `residual`), `lib.rs` (exports);
   `crates/rcfd-fem/src/solve.rs` (all), `error.rs` (`FemError::Solver { solver: &'static str, reason }`), `lib.rs` (public
   modules `assemble`, `constraint`, `solve`), `constraint.rs` (`reduce`, `ReducedSystem::{matrix, rhs, free}`), `Cargo.toml`
   (rayon is already a dev-dependency); `crates/rcfd-fem/tests/sb_pcg.rs` (all: the set-up helpers, which are copied into the
   new test file, not imported or edited).
3. **Not read:** the bodies of the papers cited; any preconditioner or ordering code outside this repository.

Findings that shape the design:

1. `ic_attempt` (ADR 0282) already factors on an **arbitrary** sorted strict-lower pattern: `l_ik` for each stored `k < i`
   with the sum over the common columns of rows `i` and `k` found by a marker of row `i`. Given a larger pattern with `a_ij =
   0` at the fill positions it computes IC with that pattern. So IC(1) needs only the symbolic pattern; the numerics and the
   shift loop are ADR 0282's.
2. `IncompleteCholesky::new` is not changed: a new constructor `with_fill` builds the pattern and runs a copy of the shift
   loop. With `level = 0` the pattern is the stored lower pattern of `A`, in the same order with the same values, so
   `with_fill(a, 0, o)` performs the same floating-point operations as `new(a, o)` (check L2).
3. `rcm_order` takes an edge list and is deterministic; `permute_symmetric` builds `B[i][j] = A[order[i]][order[j]]` through
   `from_triplets` (no duplicates, so no summation: the values are copied). With `(P x)_i = x[order[i]]`, `B = P A P^T` and the
   preconditioner of `A` is `P^T M_B^{-1} P`: gather `r_p[i] = r[order[i]]`, apply, scatter `z[order[i]] = z_p[i]`.

## Arithmetic before this registration (disclosed)

1. **Level 1 in closed form.** Levels are `>= 0` and a new level is `lev(i, k) + lev(k, j) + 1 >= 1`; it is `<= 1` only if
   both terms are 0, i.e. `a_ik` and `a_kj` stored, with `k < min(i, j)`. Entries of level 1 cannot create further entries
   of level `<= 1` (they would give `>= 2`). So the level-1 pattern is `pattern(A) U {(i, j): exists k < min(i, j), a_ik !=
   0 stored, a_jk stored}`. For `K4` of ADR 0282 (the 4-cycle 0-1-2-3-0): eliminating 0 couples 1 and 3 (fill `(3, 1)`);
   `(2, 0)` has no common neighbour below 0. So IC(1) of `K4` has the pattern of the exact `L` (whose only fill is `l_42`, 1-based,
   = `(3, 1)` 0-based), and is the exact `L D L^T`: `d = (3, 5/3, 3/5, 1/3)` with no breakdown (ADR 0282 arithmetic 2).
2. **Pattern sizes** (`arith.py`, node graph of the box, every 3 x 3 block stored; strict lower entries at equation level =
   `9 x` node pairs `+ 3` per node):

   | Case | Unknowns | Strict lower IC(0) | Strict lower IC(1) | ratio | Work proxy `sum (row length)^2`, IC(0) / IC(1) |
   | --- | --- | --- | --- | --- | --- |
   | fast `[32, 6, 6]` | 4704 | 150 351 | 317 751 | 2.11 | 5.7e6 / 2.5e7 |
   | slender `[64, 6, 6]` | 9408 | 303 951 | 647 607 | 2.13 | 1.2e7 / 5.3e7 |
   | heavy `[96, 18, 18]` | 103 968 | 3 841 191 | 8 705 727 | 2.27 | 1.6e8 / 7.9e8 |

   An IC(1) apply costs about `2 x` an IC(0) apply (two sweeps over 2.1 times the entries), a CG iteration with IC(1) about
   1.5 times one with IC(0) (the matvec over `2 x 150 351 + 4704` entries is unchanged). The heavy factorisation is `~ 1e9`
   multiply-adds, a few seconds; memory `~ 8.7e6 x 16` bytes, 140 MB.
3. **Node bandwidth.** Natural order of the reduced nodes (free nodes `i = 1..32`, `j, k = 0..6`, index `(i - 1) + 32 (j + 7
   k)`): the largest index distance of two adjacent nodes is `1 + 32 + 224 = 257`. RCM on the 27-neighbour graph: breadth-first
   levels from a corner are the shells `max(i', j, k) = l` (widths `(l + 1)^3 - l^3` up to `l = 6`: 1, 7, 19, 37, 61, 91, 127),
   then slices of 49 nodes; an edge joins the same or consecutive levels, so the bandwidth is at most `w_l + w_{l+1} - 1 <= 91 +
   127 - 1 = 217` nodes. Registered: RCM's node bandwidth `<` the natural 257 (check R1; estimate `<= 217`).
4. **Iterations (experience, not derivation).** ADR 0282 measured IC(0) 85, block Jacobi 156 (fast), 206 / 292 (slender),
   255 / 477 (heavy); the count is set by the low bending modes, which a one-level local preconditioner does not remove, so no
   variant here changes the order of growth. From general experience with ICCG on model problems (Meijerink and van der Vorst
   1977, Saad's textbook tables on ILU(p), from memory): one level of fill lowers the count by roughly 15 to 30 percent at
   about twice the memory. IC(1): `85 x (0.70 to 0.85) = 60 to 72` iterations, ratio 0.38 to 0.46 against 0.5: met, with a
   small margin (limit 78). For IC(0) an RCM ordering gives about the same count as the natural order (Duff and Meurant, BIT 29
   (1989) 635, from memory: RCM close to natural for IC(0), better orderings for fill-in variants), so IC(0)-RCM: 78 to 95,
   ratio 0.50 to 0.61, probably not met alone. The acceptance needs one of the two.
5. **Residual and agreement.** The stopping rule, the refinement steps and the floor are ADR 0282's (D5', the componentwise
   floor 1.3e-11 on the fast case, 1.4e-10 at L/h = 10.7): A1 and A2 limits as there, margins as there (measured 9.5e-11 and
   3.3e-13 for IC(0)).

## Design

### D1. Symbolic level-of-fill pattern (`rcfd_linalg::precond::ic_fill_pattern`, appended)

`ic_fill_pattern(a: &CsrMatrix, level: usize) -> (Vec<usize>, Vec<usize>)`: the row pointers and the sorted columns of the
strictly lower pattern of IC(`level`). Row by row (ascending `i`): the levels of row `i` start at 0 on the stored `a_ik`, `k
< i`; the columns are visited in ascending order in an ordered set (`BTreeSet`) that may grow during the visit; for a visited
`k` with level `l_k` and every entry `(j, k)` of the already finished rows `k < j < i` with level `l_jk` (kept per column `k`
in ascending `j`), the candidate `l_k + l_jk + 1` is kept at `(i, j)` if `<= level` (the minimum over `k`). The finished row
appends `(i, lev(i, k))` to the list of each of its columns. Sequential, deterministic; `level = 0` returns the stored lower
pattern of `A`.

### D2. Factor on the pattern (`IncompleteCholesky::with_fill(a, level, options)`, appended)

The refusals of `IncompleteCholesky::new` in the same order (`NotSquare`, `NonFinite`, `MissingDiagonal`,
`NonPositiveDiagonal`); the pattern of D1; `low` = the stored `a_ik` at the positions of `A`, `0.0` at fill positions; then
the shift loop of ADR 0282 D3 (a private copy, the same statements) with `ic_attempt` unchanged. The result is an
`IncompleteCholesky` (same apply). `fill_entries()` returns the strict-lower entry count. `new` is not edited.

### D3. Permuted preconditioner (`rcfd_linalg::precond::Permuted<P>`, appended)

`Permuted::new(order, inner)`; apply: `r_p[i] = r[order[i]]`, `inner.apply(r_p, z_p)`, `z[order[i]] = z_p[i]` (two buffers
allocated per apply, sequential).

### D4. Node-wise RCM (`rcfd_linalg::reorder::block_rcm_order(a, block_ptr)`, appended)

The block of each row from `block_ptr` (as for `BlockJacobi::new`); an edge `(b, c)` for every stored `a_ij` with `block(i) =
b < c = block(j)`; `rcm_order(n_blocks, lower, upper)` (it sorts and deduplicates the adjacency); the equation order is the
rows of each block in the block order, each block's rows ascending.

### D5. Selection (`rcfd_fem::solve`, additive)

* New `#[non_exhaustive] pub enum IcOrdering { Natural, NodeReverseCuthillMcKee }` and a new variant
  `CgPreconditioner::IncompleteCholeskyFill { options: IcOptions, level: usize, ordering: IcOrdering }`. The two items of the
  acceptance are `{ level: 1, ordering: Natural }` and `{ level: 0, ordering: NodeReverseCuthillMcKee }`; `level: 1` with RCM
  is allowed and recorded (X1).
* Solver names: `"cg-ic1"`, `"cg-ic0-rcm"`, `"cg-ic1-rcm"`, `"cg-ic0"` for `{0, Natural}` (the same factor as the existing
  variant), `"cg-ic<p>"` / `"cg-ic<p>-rcm"` in the report for higher levels (the static name in errors is then `"cg-icp"` /
  `"cg-icp-rcm"`); after breakdowns `"<name>, shift <alpha:e> after <k> breakdown(s)"` as in ADR 0282.
* In `preconditioned_cg` one new match arm: for RCM, `order = block_rcm_order(a, &node_blocks(free, dofs))`, the factor of
  `a.permute_symmetric(&order)`, the preconditioner `Permuted::new(order, ic)`; rows of a breakdown or a refusal are mapped
  through `order` to the free equation and then to node and component (by a permuted copy of `free` passed to the existing
  `precond_reason` and breakdown mapping). Everything after the preconditioner (the CG loop, the refinement steps, the
  stopping test) is unchanged and shared.

Taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation): (a) one variant with a level and an
ordering instead of two fixed variants (it keeps the existing `IncompleteCholesky(IcOptions)` untouched and covers both
acceptance items and their combination); (b) the level on the equation graph, not on the node graph (they coincide here, every
3 x 3 block being stored, check P1; the equation graph is the generic definition); (c) RCM of the node graph by the existing
`rcm_order`, not a new ordering code.

### D6. Determinism

Every new step is sequential (pattern, ordering, permutation, factorisation, apply); CG, the matvec and the reductions are
ADR 0282's, independent of the thread count (its D6). So iterations and bits are the same for any pool.

## Registered checks

New test files, release profile, one run each: `crates/rcfd-fem/tests/sb_pcg_fill.rs` (fast checks; `cargo test -p rcfd-fem
--release --test sb_pcg_fill -- --nocapture --test-threads=1`; the heavy record `h1_heavy_fill_record` `#[ignore]`, run once
with `--ignored`), `crates/rcfd-fem/tests/sb_pcg_fill_defaults.rs` (D1, uses only the API of main, so the same file compiles
on a `git archive` of main 0a226bda in `w49i-work/f460-scratch`), `crates/rcfd-linalg/tests/ic_fill.rs` (P2 to P4, L2). Set-up
helpers copied from `sb_pcg.rs` (fast `[32, 6, 6]` box `[0, 8] x [0, 1.5]^2`, slender `[64, 6, 6]` 16 m, heavy `[96, 18, 18]`
on `[0, 8] x [0, 1.5]^2`, `E = 2e11`, `nu = 0.3`, `xmin` clamped, traction `(0, 0, -1e6)` on `xmax`, tolerance 1e-10, caps
20 000 / 40 000 / 60 000). Variants: BJ, IC0 (existing), IC1 = `{1, Natural}`, RCM0 = `{0, NodeRcm}`, RCM1 = `{1, NodeRcm}`,
all with `IcOptions::default()`.

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| A0 | reading 1, c5 | fast: iterations of BJ and IC0 | `== 156` and `== 85` | ADR 0282 addendum 1, same code |
| A1 | c1, c2, c3 | fast, IC1 and RCM0: relative residual (`SolverStats`) | `<= 1e-10` | arithmetic 5 |
| A2 | c3 | fast, IC1 and RCM0: `max |u - u_d| / max |u_d|` against the default direct solve | `<= 1e-7` | arithmetic 5 |
| A3 | c3 | fast, IC1 and RCM0: iterations / iterations of BJ (the same run) | `<= 0.5` (78) | arithmetic 4: IC1 0.38 to 0.46, RCM0 0.50 to 0.61 |
| A4 | c3 | fast, IC1 and RCM0: pool of 1 thread with `threads(1)` and of 8 with `threads(8)`: iteration counts equal and every displacement bit-equal | equal | D6 |
| C3 | c3 | at least one of IC1, RCM0 meets A1, A2, A3 and A4 together | one | the rows above |
| F1 | c1, c2, reading 4 | fast, IC1 and RCM0 with `IcOptions { pivot_tolerance: 0.9, .. }`: solver name contains "shift", report shift `> 0`, breakdowns non-empty, every displacement finite, relative residual `<= 1e-10`, against direct `<= 1e-7` | as stated | ADR 0282 A6 (shift 2.048 there) |
| F2 | reading 4, D5 | in F1, for every reported breakdown `(node, component, bd)`: `bd.diagonal == (1 + bd.shift) x K_qq` bit for bit, `q = dofs.equation(node, component)`, `K` from `assemble::stiffness_with` | equal | D5 (the mapping through `order`) |
| S1 | c5, finding 2 | fast: `{0, Natural}` through the new variant against the existing `IncompleteCholesky(default)`: iterations equal, every displacement bit-equal | equal | finding 2 |
| D1 | c5, c6 | FNV-1a hash of displacement, reactions, relative residual, iterations, unknowns of the fast cantilever for `Direct`, `ConjugateGradient { 1e-10, 20 000 }`, `PreconditionedCg` BJ and IC0 (tolerance 1e-10, 20 000), on this branch and on a `git archive` of main 0a226bda (the same file) | equal, all four | D5: existing arms unedited |
| D2 | c5, c6 | existing tests unedited, release: `rcfd-fem` (lib and every test file, `sb_pcg` included, its ignored records not run), `rcfd-linalg` (lib and tests), `rcfd-fv` lib | pass | appended items only |
| D3 | c6 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no default path touched |
| P1 | D1, reading 2 | fast reduced stiffness (`stiffness_with`, `prescribed_equations`, `reduce`): `ic_fill_pattern(K_FF, 1)` equals, entry for entry, the set of arithmetic 1 computed in the test (for each `k`, all pairs of stored `(i, k)`, `(j, k)` with `i > j > k`, union the stored lower pattern); `ic_fill_pattern(K_FF, 0)` equals the stored lower pattern; record the counts | equal | arithmetic 1; counts expected 317 751 and 150 351 (arithmetic 2) |
| R1 | D4, reading 3 | `block_rcm_order` on the fast `K_FF` with `node_blocks`: a permutation of `0..4704`; every node's 3 equations consecutive and ascending; node bandwidth after the ordering `<` the natural node bandwidth, which is `257` | as stated | arithmetic 3: RCM `<= 217` |
| P2 | D2, arithmetic 1 | `IncompleteCholesky::with_fill(K4, 1, default)`: no breakdown, shift 0, pivots against `(3, 5/3, 3/5, 1/3)` relative `<= 1e-14`, `max |M^{-1} K4 x - x| / max |x| <= 1e-14` for `x = (1, 2, 3, 4)` | as stated | arithmetic 1: exact factor |
| P3 | D3 | the 6 x 6 full-pattern matrix of ADR 0282 L1 permuted by `order = (3, 0, 5, 1, 4, 2)`: `Permuted::new(order, IncompleteCholesky::new(P A P^T))` applied to `A x`, `x_i = 1 + i`: `max |z - x| / max |x|` | `<= 1e-12` | full pattern: exact |
| P4 | D2 | `with_fill(K4, 3, default)` and `with_fill(A6, 5, default)` (levels above what the graph needs): equal pattern to level 1 / the full pattern, refusals of `new` reproduced by `with_fill` on the four matrices of ADR 0282 L4 (same variant) | as stated | D2 |
| L2 | finding 2 | `with_fill(A, 0, o)` and `new(A, o)` on `K4` (9 breakdowns) and on the 12 x 10 five-point matrix: pivots, shift, breakdown list and the apply to `r_i = sin(i)` bit-equal | equal | finding 2 |
| X1 | c4 | record: iterations, refinement steps, relative residual, against direct, factor entries and time of BJ, IC0, IC1, RCM0, RCM1 on the fast and slender cases; ratios to BJ | record | arithmetic 4 |
| H1 | c4 | heavy record (`#[ignore]`, once): the same for the five variants (no direct solve) | record; the relative residual is printed, its limit 1e-10 is not part of F-460 | ADR 0282 H1c: 477 / 255 |

**Negative controls** (each must hold, else the check is not sensitive):

| Id | Control | Expected |
| --- | --- | --- |
| N1 | IC1's and RCM0's fast solutions each against IC0's (A4's measure) | not bit-equal, both |
| N2 | P1's comparison between the level-0 and the level-1 pattern | not equal |
| N3 | R1's bandwidth measure on the natural order | `== 257` (the measure is the one compared) |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f460-scratch/mut`, own target directory; each applied
alone by exact text replacement, the fast tests of `sb_pcg_fill` and `ic_fill` run once, the file restored and compared by
SHA-256):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | D1: the candidate level `l_k + l_jk + 1` becomes `l_k + l_jk` (level-2 entries admitted) | P1 |
| M2 | D1: no fill (the pattern of `A` returned for every level) | P1, P2 |
| M3 | D2: fill positions initialised with the diagonal value instead of 0 | P2 |
| M4 | D3: the scatter `z[order[i]] = z_p[i]` becomes `z[i] = z_p[i]` | P3, A1 or A2 (RCM0) |
| M5 | D4: `block_rcm_order` returns the identity | R1 |
| M6 | D5: the RCM arm factors the unpermuted matrix (`Permuted` kept) | A1 or A2 (RCM0), N1 |
| M7 | D5: breakdown rows mapped without `order` (with `free` instead of the permuted copy) | F2 (RCM0), else recorded with the reason |
| M8 | D5: `level` ignored (0 used) | N1 (IC1), P-checks do not see it |
| M9 | D2: the shift loop of `with_fill` does not apply the shift (always 0) | F1 |

**Decision rule.** F-460 is proposed **done** (pending the maintainer's acceptance) if C3 holds (at least one of IC1 and RCM0
meets A1 to A4 together), F1 and F2 hold for both, S1, D1, D2, D3 hold, A0 holds, P1 to P4, L2, R1 hold, X1 and H1 were run
and recorded, N1 to N3 hold, and every mutation fails a test (M7 may survive if recorded with the reason). **Partial** if C3
fails (both variants miss at least one of A1 to A4; the misses become `*_known_miss` records), or H1 or X1 was not run, or a
negative control fails, or a mutation other than M7 survives, or (Rule I) a mutation is caught only after a post-run amendment.
A miss of A1 to A4 by the variant that does not carry C3 is recorded and does not by itself make the item partial. A miss of
S1, D1, D2 or D3 (a default changed): **todo** until repaired. No crate that builds: **todo**.

## Limits (what is not covered)

- Still one-level preconditioners: the count grows with slenderness and size; no variant here changes that (AMG with
  rigid-body modes is ADR 0305).
- Levels above 1 are allowed but not checked beyond P4; the memory of IC(p) grows quickly with p.
- RCM is the only ordering tried (no nested dissection, no multicolour, no minimum degree); the ordering is applied to the
  preconditioner only.
- The pattern and the factorisation are sequential; for the heavy case the IC(1) factorisation is a few seconds.
- Only the elastic Hex8 cantilever of ADR 0282 is measured; no Tet10 and no thermal stiffness.
