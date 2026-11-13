# ADR 0375: F-477 registration: two block Gauss-Seidel sweeps per side for the elasticity multigrid on quadratic elements, and a graded Tet10 box on which pass 2's restriction is observable

Status: **REGISTERED 2026-11-13, written before any code of F-477 and before any run of its checks**. Not yet accepted by
the maintainer. Before this text nothing of F-477 was written, compiled or run; the arithmetic below was done by hand from
numbers already in ADR 0370. Results go in the addendum at the end; nothing above the addendum is changed after a run.

Roadmap item F-477 (deps F-470), from [ADR 0370](0370-f470-registration-tet10-aggregation-like-hex8.md) remaining questions
1 and 2, both option (a). Branch `f477-tet10-smoother` from main ee776d4d, worktree `w49i-work/f477`, scratch
`w49i-work/f477-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded; a defective set-up gets a corrected test by an amendment
committed before the corrected run; each registered run is made once; existing tests, ignored records, `baseline_hashes.rs`,
the hash tables and the goldens are not edited; defaults bit-identical, the new behaviour opt-in; no new crate, no unsafe
code, no trailers, no push. The scalar AMG `crates/rcfd-linalg/src/amg.rs` is not touched; nothing in `rcfd-fv` or the field
store is touched (other items work there).

## What F-477 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. With the geometric aggregation of ADR 0370
> (ElasticAmgSettings::quadratic()), a registered opt-in smoother setting for quadratic elements (more block Gauss-Seidel
> sweeps or a Chebyshev smoother, derived before the run) such that on the Tet10 cantilever of slenderness 20 of ADR 0305 CG
> with one V-cycle reaches 1e-8 in at most 30 iterations with operator complexity <= 1.5 and the setup and solve time
> recorded beside ADR 0370's (31 iterations, 3.1 s); a test on a graded or anisotropic Tet10 box where pass 2's restriction to
> the candidates changes the aggregates (ADR 0370 mutation M5); every count and bit of ADR 0305 and ADR 0370 with their
> settings unchanged.

Clauses: (c1) an opt-in smoother setting for quadratic elements, on top of `quadratic()`, with its parameter derived here;
(c2) on the slenderness-20 Tet10 cantilever (tet_box 120 x 6 x 6 on `[0, 20] x [0, 1]^2` upgraded, 121 680 unknowns) CG with
one V-cycle to the relative residual `1e-8` in `<= 30` iterations; (c3) operator complexity `<= 1.5`; (c4) setup and solve
time recorded beside ADR 0370's; (c5) a test on a graded or anisotropic Tet10 box on which restricting pass 2 to the
candidates changes the aggregates, so that ADR 0370's mutation M5 is caught; (c6) every count and bit of ADR 0305
(`default()`) and ADR 0370 (`quadratic()`) unchanged.

**Readings (registered here).**

1. (c2) The count is `SolverStats::iterations` of `StaticSolver::PreconditionedCg { preconditioner:
   CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic_smoothed()), tolerance: 1e-8, max_iterations: 60 000
   }` (as reading 2 of ADR 0370), met when the solve returns `Ok`, relative residual `<= 1e-8` and count `<= 30`.
2. (c4) "Recorded beside ADR 0370's": the same heavy test also solves the case with `quadratic()` and prints both setups
   (the hierarchy build alone) and both solve times (`solve_static_report`, wall clock, assembly included, as ADR 0370's
   3.1 s), so the comparison is on one machine in one run. A recording clause: met when printed.
3. (c5) "Changes the aggregates": an independent implementation of ADR 0370 D1 in the test, run once with pass 2 over the
   candidates (D1 as registered) and once with pass 2 over every stored neighbour (mutation M5), gives level-0 aggregates that
   differ in at least one block on at least one of the two registered graded boxes G1, G2 ("a graded **or** anisotropic box";
   G1 is both); and the library's level-0 aggregates equal the first on both. Under M5 the library equals the second, so the
   check fails on the box where they differ.
4. (c6) Checked by D1 to D5 below.

## What was read (main ee776d4d, read only)

1. `AGENT_RULES.md`; `docs/roadmap.json` F-477 and F-470; ADR 0370 in full (design D1 and D2, its checks, addendum 1: H1b 31
   iterations, 0 restarts, residual 9.17e-9, solve 3.1 s, setup 0.95 s, OC 1.4762, levels 121 680 / 12 000 / 522; diagnosis:
   20 iterations to 1e-4, 23 to 1e-5, 26 to 1e-6, 28 to 1e-7, 31 to 1e-8, factor about 0.46 per iteration against 0.28 for
   Hex8; E3 fast Tet10 27 iterations, default 40; M5 not caught); ADR 0305 sections D5, D6 (the V(1,1) cycle, forward sweep
   before and backward sweep after, so the cycle is symmetric) and its limits ("no Chebyshev smoother").
2. Code: `crates/rcfd-linalg/src/amg_elastic.rs` (all: `ElasticAmgSettings` already carries `pre_sweeps` and `post_sweeps`,
   used by `cycle` on every level), `crates/rcfd-fem/src/solve.rs` (`smoothed_aggregation`), `crates/rcfd-fem/src/builders.rs`
   (`tet_box`, `upgrade_to_tet10`: mid-edge nodes at the midpoints of the corner coordinates), `crates/rcfd-fem/src/mesh.rs`
   (`SolidMesh::new`, `add_block`, the set accessors), `crates/rcfd-fem/tests/si_amg_quadratic.rs` and `f470_defaults.rs`
   (set-ups copied, not imported or edited), `w49i-work/f470-scratch/mutate.py` (its M5 text is reused).
3. **Not read:** any multigrid code outside this repository; the bodies of the papers cited.

## Arithmetic before this registration (disclosed)

1. **Why sweeps rather than Chebyshev.** The setting space already exists (`pre_sweeps`, `post_sweeps`), so the opt-in is a
   new preset with no new code path in the cycle: the setup (aggregation, prolongator, coarse operators) is untouched, hence
   the operator complexity is exactly that of `quadratic()` (1.4762 on the heavy case, clause c3 met by construction), and no
   default or `quadratic()` byte can move. A Chebyshev smoother would need new cycle code and an eigenvalue interval of
   `D^{-1} A` (the 15-step power estimate of `spectral_radius` is the only bound available) and a registered lower end; it is
   left out (limits).
2. **Symmetry.** With `nu` forward sweeps before and `nu` backward sweeps after, the error propagation of a level is
   `E = S_B^nu (I - P B_c^{-1} P^T A) S_F^nu` with `S_F = I - (D + L)^{-1} A`, `S_B = I - (D + U)^{-1} A`, `U = L^T`. The
   A-adjoint of `S_F` is `S_B`, so `E` is A-self-adjoint when the coarse correction is (recursively), and the V-cycle stays a
   symmetric preconditioner for CG for any equal `pre_sweeps = post_sweeps` (first principles; ADR 0305 D6 argued it for
   `nu = 1`). Unequal counts break it (mutation M2).
3. **The sweep count `nu` (model, from ADR 0370's measured rate).** Model (a): for a symmetric V-cycle used as a stationary
   iteration, `|E|_A <= C / (C + nu)` with a constant `C` independent of `nu` (the shape of the Braess-Hackbusch V-cycle bound,
   SIAM J. Numer. Anal. 20 (1983) 967, cited from memory; used here only as a model for the estimate, never as a limit).
   Model (b), first principles: with `0 <= E <= rho` in the A-inner product the preconditioned operator has its spectrum in
   `[1 - rho, 1]`, `kappa = 1 / (1 - rho)`, and CG contracts by `q = (sqrt kappa - 1) / (sqrt kappa + 1)` per iteration.
   From the measured `q_1 = 0.46` (ADR 0370 diagnosis, about 3 iterations per decade; `1 / log10(1 / 0.46) = 2.97`):
   `sqrt kappa_1 = 1.46 / 0.54 = 2.704`, `kappa_1 = 7.31`, `rho_1 = 0.863`, `C = rho_1 / (1 - rho_1) = 6.31`.
   - `nu = 2`: `rho_2 = 6.31 / 8.31 = 0.759`, `kappa_2 = 4.16`, `sqrt = 2.04`, `q_2 = 0.342` (2.15 iterations per decade).
     Count scaled from 31 by `ln q_1 / ln q_2 = 0.777 / 1.073`: **22.4**.
   - `nu = 3`: `rho_3 = 0.678`, `q_3 = 0.276`, count 18.7.
   The smallest `nu` whose estimate meets the limit 30 with a margin is registered: **`nu = 2`** (`QUADRATIC_SWEEPS = 2`,
   pre and post). Pessimistic end: the part of the error the smoother cannot reach (the prolongator's, OC-limited reach-2
   smoothing) does not shrink with `nu`; if half the gap to Hex8 is that part, the count is about 27. Estimate **19 to 28,
   central 22**; the limit 30 is the acceptance's.
4. **Fast Tet10 (32 x 6 x 6, 32 448 unknowns).** ADR 0370 E3: 27 iterations with `quadratic()`. Scaled by the same ratio:
   19.5; estimate 16 to 24. Registered limit (F1): strictly fewer than the `quadratic()` count measured in the same test, and
   that count `== 27` (ADR 0370 E3, a default-of-0370 check).
5. **Cost (estimate, not a limit).** In fine-level matrix-vector units per CG iteration: V(1,1) = 2 sweeps + residual + `P`,
   `R` (about 1) + CG matvec + coarse levels (`0.48 x 3`) = about 6.4; V(2,2) = 4 + 1 + 1 + 1 + `0.48 x 5` = about 9.4, 1.47
   times. With 22 against 31 iterations the iterative phase takes about 1.04 times as long; with assembly and setup (about
   1.5 s of ADR 0370's 3.1 s) the solve time is estimated at 2.8 to 3.6 s on an idle machine. Recorded only (c4).
6. **Why ADR 0370's meshes cannot see M5, and the graded boxes.** On the uniform Kuhn lattice every block has the same
   neighbourhood shape up to the boundary, and the stiffness weight `|A_IJ|_F` falls with distance, so a pass-2 block's
   strongest aggregated neighbour is a candidate (ADR 0370 addendum). Grading breaks both: across a corner where the cell
   width changes by the factor `g`, the distances to the two sides differ by `g`, the nearest distance `d_I` is set by the
   finer side, and the far side's diagonal neighbours fall outside `rho d_I` while their weights (larger elements, stiffness
   per element scales with its size) can exceed those of the candidates on the fine side. The candidate sets then lose their
   symmetry from block to block, so pass 2 has leftovers whose strongest aggregated neighbour is not a candidate. This is
   an argument, not a proof that a given box shows a difference; two boxes are registered and the clause needs one (reading
   3). Risk disclosed: if neither shows a difference the clause is missed and M5 is again not caught.
   - **G1** (graded in x, so also anisotropic): `tet_box([0; 3], [4, 1.5, 1.5], [16, 6, 6])` with the corner x-coordinates
     mapped to `x_i = 4 (g^i - 1) / (g^16 - 1)`, `g = 1.25` (cell widths 0.029 to 0.818 against 0.25 across: aspect ratios
     from 8.6 flat to 3.3 long), then upgraded; 33 x 13 x 13 nodes, clamped `xmin`, traction on `xmax`.
   - **G2** (graded in all three directions): `tet_box([0; 3], [3, 1, 1], [12, 8, 8])` with each axis mapped geometrically,
     growth `(1.2, 1.3, 1.4)` per cell in `(x, y, z)`, then upgraded; 25 x 17 x 17 nodes.
   The map is monotone per axis, so each hexahedral cell stays a box and its six Kuhn tetrahedra stay positive.

## Design

### D1. The setting (`rcfd_linalg::amg_elastic`, lines inserted only)

`pub const QUADRATIC_SWEEPS: usize = 2` (arithmetic 3) and `ElasticAmgSettings::quadratic_smoothed()` = `quadratic()` with
`pre_sweeps = post_sweeps = QUADRATIC_SWEEPS`. Nothing else in the library changes: the cycle already loops `pre_sweeps`
forward and `post_sweeps` backward block Gauss-Seidel sweeps on every level. `rcfd_fem::solve::smoothed_aggregation` already
passes the node points for any non-`Greedy` aggregation, so `CgPreconditioner::SmoothedAggregation(quadratic_smoothed())`
selects it with no change in `rcfd-fem`'s source.

Derivation: arithmetic 2 and 3; block Gauss-Seidel smoothing and its symmetric V-cycle as in ADR 0305 D6.

### D2. The graded-box test (`crates/rcfd-fem/tests`, new file)

A copy of ADR 0370's E2 reference (`reference_aggregates`) with one flag, `pass2_all`: when set, pass 2 scans every stored
neighbour (with its weight) instead of the candidates. G1, G2 built by the test (arithmetic 6; node, element and surface
sets copied to the mapped mesh before the upgrade).

## Registered checks

New test files, release profile, each run once: `crates/rcfd-fem/tests/si_amg_smoother.rs` (fast: DF2, F1, F2, G, N1;
heavy `#[ignore]`: H1), `cargo test -p rcfd-fem --release --test si_amg_smoother -- --nocapture --test-threads=1`, the heavy
one once with `--ignored`; `crates/rcfd-fem/tests/f477_defaults.rs` (D1).

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| H1a | c2 | heavy: slenderness-20 Tet10, `quadratic_smoothed()`, CG with one V-cycle to 1e-8: `Ok`, relative residual `<= 1e-8`, count | `<= 30` | arithmetic 3: 19 to 28, central 22 |
| H1b | c3 | heavy: operator complexity of the `quadratic_smoothed()` hierarchy; and its level orders and nnz equal those of `quadratic()` | `<= 1.5`; equal | arithmetic 1: 1.4762, setup identical |
| H1c | c4 | heavy: setup and solve time of `quadratic_smoothed()` and of `quadratic()` printed side by side | printed | arithmetic 5: 2.8 to 3.6 s against about 3.1 s |
| D5 | c6 | heavy (in H1): the same case with `quadratic()`: count and first coarse level | `== 31`, `== 12 000` | ADR 0370 H1, code path unchanged |
| DF2 | c1, c6 | `default()` equals the ADR 0305 values with `Greedy`; `quadratic()` equals `default()` but for `Geometric { ratio: QUADRATIC_RATIO }`; `quadratic_smoothed()` equals `quadratic()` but for `pre_sweeps = post_sweeps = QUADRATIC_SWEEPS = 2` | as stated | D1 |
| F1 | c1, c2 (fast), c6 | fast Tet10: `quadratic_smoothed()` hierarchy level orders and nnz equal `quadratic()`'s; S2 of ADR 0305 (asymmetry `<= 1e-10`, both forms positive); CG to 1e-8 `Ok`, residual `<= 1e-8`, count `<` the `quadratic()` count of the same test, and that count `== 27` | as stated | arithmetic 2, 4: 16 to 24 against 27 |
| F2 | ADR 0305 c8 | fast Tet10, `quadratic_smoothed()`: 1 and 8 threads (pools and `threads(t)`): counts equal, displacements bit-equal | equal | sequential cycle |
| G | c5 | G1 and G2: (i) library level-0 aggregates of `quadratic()` equal the reference with pass 2 over the candidates, on both; (ii) that reference and the one with pass 2 over every stored neighbour differ in `>= 1` block on at least one of G1, G2 (numbers of differing blocks printed for both) | (i) equal on both; (ii) `>= 1` on one | arithmetic 6 |
| D1 | c6 | `f477_defaults.rs`: `default()` and `quadratic()` on cube 11, the fast Hex8 cantilever `[32, 6, 6]` on `[8, 1.5, 1.5]`, P1 (symmetry cube 11) and fast Tet10: printed count, level orders and an FNV-1a hash of the displacement bits; run on this branch and on a `git archive` of ee776d4d (own target, the same file copied in) | identical output lines | no existing line changed |
| D2 | c6 | existing tests unedited, release: `cargo test -p rcfd-linalg --release`, `cargo test -p rcfd-fem --release` (ignored records not run); the counts printed by `si_amg` equal ADR 0305's (cube 16 / 17, X1 15 / 22, P1 16, AG1 64, N1 29 / 16) and by `si_amg_quadratic` equal ADR 0370's (E1 16 / 16, E3 27, default 40) | pass, equal | default path unchanged |
| D3 | c6 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | no default path touched |
| D4 | scope | `git diff ee776d4d -- crates/rcfd-linalg/src/amg.rs crates/rcfd-fem/src crates/rcfd-fv` empty; `git diff --numstat ee776d4d -- crates/rcfd-linalg/src/amg_elastic.rs` with 0 deleted lines | as stated | D1 |

Records (no limit): in H1, the levels and restarts of both solves, the floor estimate of ADR 0305 amendment 2, and the Hex8
slenderness-20 case with `quadratic_smoothed()` (count, time); in G, the CG counts to 1e-8 on G1 with `default()`,
`quadratic()` and `quadratic_smoothed()`, and the number of pass-2 blocks.

**Negative controls.**

| Id | Control | Expected |
| --- | --- | --- |
| N1 | fast Tet10 (the uniform box of ADR 0370): the two references (pass 2 over candidates / over all neighbours) | equal (ADR 0370's finding that M5 is equivalent on uniform boxes, reproduced) |
| N2 | (in F1) the setup of `quadratic_smoothed()` against `quadratic()` | identical level orders and nnz (the smoother is not part of the setup) |

**Mutations** (on a `git archive` copy of the final code in `f477-scratch/mut`, own target directory; each applied alone by
exact text replacement, the fast tests of `si_amg_smoother` run once, the file restored and compared by SHA-256):

| Id | Mutation | Must fail |
| --- | --- | --- |
| M1 | `QUADRATIC_SWEEPS = 1` (the preset becomes `quadratic()`) | DF2, F1 (count not fewer) |
| M2 | `quadratic_smoothed()` with `post_sweeps: 1` (unequal sweeps) | DF2, F1 (S2 asymmetry) |
| M3 | `quadratic_smoothed()` built on `default()` instead of `quadratic()` (greedy aggregation with two sweeps) | DF2, F1 (level orders) |
| M4 | `cycle`: the post-smoothing sweeps run forward instead of backward (arithmetic 2: the cycle loses its symmetry) | F1 (S2 asymmetry) |
| M5 | ADR 0370 M5: pass 2 over all stored neighbours instead of the candidates (the text of `f470-scratch/mutate.py`) | G (i) |
| M6 | pass 2 of `aggregate_geometric` removed (leftovers become singletons) | G (i) |

**Decision rule.** F-477 is proposed **done** (pending the maintainer's acceptance) if H1a, H1b, H1c, D5, DF2, F1, F2, G and
D1 to D4 hold in their registered runs, N1 and N2 hold, and every mutation M1 to M6 fails a registered check of its own in
the registered run. **Partial** if any of these misses (the miss becomes a `*_known_miss` record with the limit intact), the heavy
test was not run, a negative control fails, or (Rule I) a registered mutation is not caught by its registered check or is
caught only after a post-run amendment. A miss of D1 to D5 (a count or bit of ADR 0305 or 0370 changed): **todo** until
repaired. No crate that builds: **todo**.

## Limits (what is not covered)

- No Chebyshev or other polynomial smoother (arithmetic 1); no W-cycle; the sweep count is the same on every level.
- The cost per iteration rises (arithmetic 5); whether V(2,2) is the cheapest way under 30 iterations is not studied.
- G1 and G2 are tensor-graded boxes; unstructured Tet10 meshes and other quadratic kinds (Hex20, Hex27, Wedge15) are not
  measured; on G1 and G2 only the aggregation is checked, convergence there is recorded.
- No case-file key (F-386).

## Addendum 1: results (2026-11-13)

Code: registration 58d4069e; library and tests e4cd0d48 (`crates/rcfd-linalg/src/amg_elastic.rs`: 14 lines inserted, 0
deleted, at new 75-84 (`quadratic_smoothed()`) and 87-90 (`QUADRATIC_SWEEPS`); no other source file changed; tests
`crates/rcfd-fem/tests/si_amg_smoother.rs`, `crates/rcfd-fem/tests/f477_defaults.rs`). Release profile throughout. Logs in
`w49i-work/f477-scratch`: `run1-si_amg_smoother.log` (fast, once), `run1-h1.log` (heavy, once), `run-d1-branch.log`,
`run-d1-main.log` (the same file on a `git archive` of ee776d4d, own target), `run-d2-linalg.log`, `run-d2-fem.log`,
`run-d3-baseline.log`, `mutations.log` (`mutate.py`), `clippy.log`.

| Id | Limit | Measured | Met |
| --- | --- | --- | --- |
| H1a CG with one V-cycle to 1e-8, slenderness-20 Tet10, `quadratic_smoothed()` | `<= 30`, residual `<= 1e-8` | **23** iterations, 0 restarts, residual 7.22e-9 (floor estimate 8.64e-9) | yes |
| H1b operator complexity; setup equal to `quadratic()`'s | `<= 1.5`; equal | 1.4762 (levels 121 680 / 12 000 / 522, nnz 9.52e6 / 4.47e6 / 6.5e4), identical | yes |
| H1c times printed | printed | setup 0.86 s (`quadratic()` 0.83 s); solve with assembly 3.56 s (`quadratic()` 2.84 s in the same run; ADR 0370: 3.1 s) | yes |
| D5 `quadratic()` on the same case | `== 31`, `== 12 000` | 31 (residual 9.166e-9, as ADR 0370's 9.17e-9), 12 000 | yes |
| DF2 settings | as stated | default as ADR 0305, `quadratic()` as ADR 0370, `quadratic_smoothed()` = `quadratic()` with 2 / 2 sweeps | yes |
| F1 fast Tet10 | levels equal; S2 `<= 1e-10`, positive; `Ok`, `<= 1e-8`, fewer than `quadratic()`, which `== 27` | levels equal; S2 1.56e-16, positive; 20 iterations, 8.67e-9, against 27 (== 27); 0.56 s against 0.54 s | yes |
| F2 1 and 8 threads | equal | 20 / 20, bit-equal | yes |
| G (i) library equal to the reference on G1, G2 | equal on both | equal on both | yes |
| G (ii) pass 2's restriction changes the aggregates | `>= 1` block on one box | **G1 16** blocks (5408 blocks, 1121 aggregates, 870 left after pass 1); **G2 46** blocks (6936, 1750, 1034) | yes |
| N1 uniform fast Tet10: the two references | equal | equal (0 differing of 10 816 blocks; 2704 left after pass 1) | yes |
| N2 setup of the smoothed preset | identical | identical level orders and nnz (fast and heavy) | yes |
| D1 hashes branch / ee776d4d | identical | 8 identical lines (default and `quadratic()`: cube 11 `63183f7b70a37472` 16 / 16, fast Hex8 `eac2e37cdbf7447a` 15 / 15, P1 `6a5b29ee81bc9dc7` 16 / 16, fast Tet10 default `3000b383efab3700` 40, `quadratic()` `d4e126c81ff1b533` 27) | yes |
| D2 existing tests | pass, counts equal | `rcfd-linalg` release: every test file passes; `rcfd-fem` release: 117 passed, 0 failed, 25 ignored (not run); `si_amg` cube 16 / 17, X1 15 / 22, P1 16, AG1 64, N1 29 / 16; `si_amg_quadratic` E1 16 / 16, E3 27, default 40 | yes |
| D3 baseline_hashes | pass | 3 passed, 1 ignored | yes |
| D4 scope | as stated | `amg.rs`, `rcfd-fem/src`, `rcfd-fv` unchanged; `amg_elastic.rs` 14 inserted, 0 deleted | yes |

Records: G1 to 1e-8: `default()` 60 iterations, `quadratic()` 26, `quadratic_smoothed()` 19 (no restarts). Hex8 slenderness
20 with `quadratic_smoothed()`: 13 iterations (15 with the default), 3.09 s.

**Deviations.**

1. The heavy test took 12 s, not 1 to 3 minutes; it stays `#[ignore]` as registered.
2. The solve time of the smoothed preset is 1.25 times that of `quadratic()` in the same run (3.56 s against 2.84 s), above the
   estimate of arithmetic 5 (about 1.04 for the iterative phase): the iteration count fell less than the smoothing work rose
   (23 against the central estimate 22, but two sweeps cost more than the matvec units assumed). Recorded; no limit.
3. Not run: the whole workspace (the lead runs the gate); the heavy records of ADR 0305 and ADR 0370 (D5 measured ADR 0370's
   count again).

**Mutations** (archive of e4cd0d48 in `f477-scratch/mut`, own target, each alone, the file restored and compared by SHA-256,
all restored):

| Id | Registered | Caught by |
| --- | --- | --- |
| M1 `QUADRATIC_SWEEPS = 1` | DF2, F1 | DF2, F1 (27 iterations, not fewer than 27) |
| M2 `post_sweeps: 1` | DF2, F1 (S2) | DF2, F1 (S2) |
| M3 preset on `default()` | DF2, F1 (levels) | DF2, F1 (levels 32 448 / 594 against 32 448 / 3300 / 144) |
| M4 post-smoothing forward | F1 (S2) | F1: S2 asymmetry 1.27e-4 (limit 1e-10); the test function stopped at the `quadratic()` reference solve of F1, which under the mutation did not converge in 60 000 iterations (the mutation changes every preset), before the S2 assertion line; the run took about 70 minutes for that reason |
| M5 ADR 0370 M5 (pass 2 over all neighbours) | G (i) | G (i): the library differs from the reference on G1 and G2 (16 and 46 blocks); F1, N1 held |
| M6 pass 2 removed | G (i) | G (i), and F1 (the fast Tet10 hierarchy changed: `quadratic()` 19 iterations, not 27) |

`cargo fmt -p rcfd-linalg --check`, `cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-linalg -p rcfd-fem
--all-targets --release -- -D warnings`: clean.

**Decision by the registered rule: done** (pending the maintainer's acceptance). Every registered check held in its
registered run (H1a 23 iterations against 30; H1b OC 1.4762; H1c printed; D5 31 and 12 000; DF2, F1, F2, G, N1, N2, D1 to D4),
and every mutation M1 to M6 failed a registered check of its own in the registered run, with no amendment. ADR 0370's
uncaught mutation M5 is now caught on the graded boxes G1 and G2 (16 and 46 blocks change aggregate), and N1 confirms it is
equivalent on the uniform box.

**Remaining questions.**

1. Cost. The smoothed preset needs 23 instead of 31 iterations but its solve took 3.56 s against 2.84 s for `quadratic()`
   in the same run. Options: (a) keep both presets, `quadratic()` the cheaper and `quadratic_smoothed()` the one under the
   iteration limit; (b) a follow-up for a cheaper smoother (Chebyshev on block Jacobi, or two sweeps on level 0 only) against
   the time of `quadratic()`; (c) make `quadratic()` itself use two sweeps (changes ADR 0370's counts and bits: not taken).
   Recommendation: (a), with (b) only if a solid case needs the time.
2. Selection from a case file (F-386): the key that selects `quadratic()` for quadratic meshes could offer the smoothed
   preset as well. Recommendation: note it on F-386.

## For the lead: roadmap entries

F-477: proposed status **done** (pending the maintainer's acceptance). Note:

> From ADR 0370 questions 1 and 2 (with a graded or anisotropic Tet10 mesh on which mutation M5 changes the aggregates), recommendation taken on the maintainer's standing instruction of 2026-11-11. ADR 0375, 2026-11-13: rcfd_linalg::amg_elastic::ElasticAmgSettings::quadratic_smoothed() (opt-in; quadratic() with QUADRATIC_SWEEPS = 2 forward block Gauss-Seidel sweeps before and 2 backward after the coarse correction, so the cycle stays symmetric; the count derived from ADR 0370's measured rate of 0.46 per iteration with a C/(C + nu) V-cycle model, estimate 22). On the Tet10 cantilever of slenderness 20 (121 680 unknowns) CG with one V-cycle reaches 1e-8 in 23 iterations (limit 30; quadratic() 31 in the same run), operator complexity 1.4762 (the setup is that of quadratic()), setup 0.86 s against 0.83 s, solve 3.56 s against 2.84 s (ADR 0370: 3.1 s): fewer iterations, more time. On two graded Tet10 boxes (x-graded 1.25 per cell; graded 1.2, 1.3, 1.4 in x, y, z) restricting pass 2 to the candidates changes 16 and 46 aggregates, so ADR 0370's mutation M5 is now caught; on the uniform box it changes none. Library change: 14 lines inserted in amg_elastic.rs, nothing else; counts and bits of ADR 0305 and ADR 0370 unchanged (hashes against main, ADR 0370 counts 27 and 31 measured again, baseline hashes pass). Six mutations, all caught. Done by the registered rule; acceptance is the maintainer's call.

F-470, sentence to append to its note:

> ADR 0375 (2026-11-13, F-477): with ElasticAmgSettings::quadratic_smoothed() (two block Gauss-Seidel sweeps per side) the slenderness-20 Tet10 cantilever reaches 1e-8 in 23 iterations, and the mutation M5 left uncaught here is caught on graded Tet10 boxes.

F-386 (solid case file), sentence to append to its note:

> ADR 0375 (2026-11-13): a quadratic-mesh key could also offer CgPreconditioner::SmoothedAggregation(ElasticAmgSettings::quadratic_smoothed()) (Tet10 slenderness 20: 23 iterations, 3.56 s, against 31 iterations, 2.84 s with quadratic()).
