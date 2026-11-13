# ADR 0369: F-474 registration: the loss of definiteness of the AMG pressure preconditioner on T, and the order of the coarsest dense LU solve (F-471)

Status: **REGISTERED 2026-11-12, written before any code of F-474 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-474 or F-471 was written, compiled or run: the code and documents under "What was
read" were read, and the arithmetic under "Arithmetic before this registration" was done by hand. Results go in an addendum;
a defective set-up, or a remedy chosen after the diagnosis, is registered by an amendment committed before its code and its
run; nothing above an amendment or the addendum is changed after a run.

Parents: [ADR 0363](0363-f466-registration-bounded-forcing-stop-on-t.md) (F-466: the stop of DFG 2D-2 on T at step 2951 located
in the AMG-preconditioned pressure CG, `r^T z < 0` from iteration 1; its question Q4 recommendation (a) with Q6 folded in,
from which F-474 was made; its opt-in capture `PimpleSolver::pressure_capture`, `pressure::pressure_system_src`, the read-only
AMG trace, and the twelve states written by its H1 in `w49i-work/f466-scratch/runs`), [ADR 0305](0305-aggregation-amg-with-rigid-body-modes.md)
(F-402: amendment 1 found and fixed the order of the dense coarsest solve in `amg_elastic.rs`; its question 1 is F-471).
Roadmap items F-474 (deps F-466) and the diagnosis part of F-471 (folded in by the lead's brief).

Number 0369 (given by the lead). Branch `f474-amg-definiteness` from main 7a533a53, worktree `w49i-work/f474`, scratch
`w49i-work/f474-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a missed limit becomes an ignored
`*_known_miss` test with the limit intact; each registered run is made once; defaults stay bit-identical; no hash table,
golden or existing test file is edited; no new crate, no unsafe code; unsigned commits without trailers; no push. **The brief
adds: no default path is changed.** A corrected coarse solve is built as an opt-in; which registered hashes and fluid results
a default switch would move is measured on a scratch copy with the switch flipped and reported. Moving the default is the
maintainer's decision.

Clean-room. Nothing outside this repository is used. LU factorisation with partial pivoting and the order of the
permutation in the triangular solves are textbook numerical linear algebra (Golub and Van Loan, Matrix Computations, chapter
3, cited from memory; derived below on a 3 x 3 example, which is all that is used). Smoothed aggregation is rcfd's own
(`amg.rs`, Vanek, Mandel and Brezina 1996 as cited by its header). The double-double products of the measurements use the
error-free transformations TwoSum (Knuth) and TwoProd by a fused multiply-add (`f64::mul_add`), cited from memory and
derived in one line each below.

## What is asked

F-474, binding:

> On the pressure system of ADR 0363 (T, bounded immersed forcing, step 2951 corrector 1, rebuilt from the state of step 2950
> and the opt-in capture) the cause of r^T M^-1 r < 0 of the smoothed-aggregation AMG is located by a registered measurement
> (per level, the Rayleigh quotient of the cycle, the smoothed prolongator and the coarse operators against a higher-precision
> product), and either the AMG is repaired so that AMG-CG converges on that system with every registered hash unchanged, or
> the breakdown is detected inside cg (rz <= 0) and reported with a located message.

Its note: "Check first whether the coarsest dense LU ordering defect of F-471 (ADR 0305 question 1) is the cause."

F-471, binding (only its diagnosis is in this brief):

> A test shows whether the coarsest dense LU of rcfd_linalg::amg (DenseLu) interchanges rows on each fluid case of the
> baseline set and records where; DenseLu::solve applies every interchange before the forward substitution (as
> rcfd_linalg::amg_elastic after ADR 0305 amendment 1) so that a coarsest matrix that needs interchanges is solved to a
> relative residual of 1e-12; the rcfd-linalg, rcfd-fv and rcfd-api tests and the baseline hashes are run, and any changed
> hash is reported to the maintainer, not re-recorded.

Readings (questions Q1 to Q3):

- **"repaired ... with every registered hash unchanged"**: the repair is an opt-in (`AmgSettings`), so every registered hash is
  unchanged by construction and by the `baseline_hashes` run; what a default switch would move is measured and reported (the
  brief). Whether the default is switched is the maintainer's (Q1).
- **F-471's "DenseLu::solve applies every interchange first"** is a change of the default solve. It is not made here; the
  corrected order is the opt-in, and the hashes it would move are reported. F-471 can therefore be at most **partial** from
  this ADR (decided now, Q2).
- **F-471's "a test ... on each fluid case of the baseline set ... records where"**: a library hook that reports from inside a
  case run would be new state in the default path. It is read as: on a scratch copy, an `eprintln` in `DenseLu::new` prints the
  size of the coarsest matrix and the steps `k` with `piv[k] != k` for every factorisation during `baseline_hashes` (one test
  thread, so each line belongs to the case printed before it); the committed fast tests show the same property on fixed
  matrices through the new read-only `Amg::coarse_interchanges` (Q3).

## What was read and done before this text (disclosed)

Read: the F-474 and F-471 entries of `docs/roadmap.json`; ADR 0363 in full; ADR 0305 amendment 1 and the head of its addendum;
`crates/rcfd-linalg/src/amg.rs` in full; `amg_elastic.rs` `DenseLu` (lines 181 to 248); `crates/rcfd-fv/tests/immersed_bounded_stop_f466.rs`
in full (helpers, checkpoint reader, traced CG, H2's analysis); `rcfd-fv/src/fvmatrix.rs` `solve`, `solve_amg`; the AMG call
sites (`fvmatrix.rs:76`, `coupled.rs:735` Schur complement, `overset.rs:1710, 1855`, `pressure.rs:471, 591`); `rcfd-api/src/sim.rs`
line 2028 (`amg_pressure = n_cells > 2000` for the transient engines); `crates/rcfd-api/tests/baseline_hashes.rs` (the cases);
the list of test files that use the AMG (grep). No `AmgSettings { .. }` literal exists outside `amg.rs` (grep), so a new field
breaks no caller.

Run: nothing. Free memory read: 12.8 GB free.

What the reading established (no run):

1. **The order defect is present in `amg.rs`.** `DenseLu::new` swaps whole rows (`for j in 0..n`), so multipliers already
   stored in columns `< k` move with the rows (the stored factors satisfy `P A = L U`, `P = P_{n-1} ... P_0`). `DenseLu::solve`
   applies interchange `k` and then eliminates column `k`, interleaved. That order is correct only for the other storage, in
   which later swaps do not move earlier multipliers. The two agree exactly when no row is interchanged (then every swap is
   `swap(k, k)`, a no-op, and the arithmetic of both orders is the same sequence of operations), which is why the existing test
   `hierarchy_shape_and_dense_lu` (a 2D Laplacian, no interchange) passes. `amg_elastic.rs` is the same code after ADR 0305
   amendment 1.
2. **An M-matrix never interchanges in exact arithmetic.** For a matrix that is weakly diagonally dominant by columns, every
   Schur complement keeps that property, so `|a_ik| <= u_kk` below every pivot and the strict comparison `>` never selects
   another row (ties keep the row). The fine pressure matrix is a symmetric M-matrix (ADR 0363 M6). The Galerkin operators
   `R A P` of a smoothed prolongator are not M-matrices (positive off-diagonals appear), so interchanges can happen on the
   coarsest level. With a 5500 : 1 face-coefficient contrast (ADR 0363 M6), a coarse diagonal much smaller than a neighbour's
   makes `|a_ik| > a_kk` possible for an SPD matrix (`a_ik^2 < a_ii a_kk` allows `|a_ik| > a_kk` when `a_ii > a_kk`). The
   coarsest pivots of the stopping system span 9.4e-10 to 2.3e-5 (ADR 0363 M6), consistent with that.
3. **A wrong coarse solve makes the cycle non-symmetric and possibly indefinite.** With exact `A_c^{-1}` and forward then
   backward Gauss-Seidel, the cycle is a symmetric operator; with the coarse solve replaced by a matrix `C` that is not
   `A_c^{-1}` and not symmetric, the coarse correction `P C R` is neither, and `r^T B r` can be negative. That is the
   signature of ADR 0363 M5 (`r^T z < 0` from iteration 1, alternating signs).
4. The coarsest level of the stopping system has 24 unknowns (ADR 0363 M6: 53 136, 10 745, 1653, 206, 24), so it is factorised
   (`<= 2000`).
5. The transient engines of `rcfd-api` use the AMG pressure only above 2000 cells; the coupled engine uses an AMG on its Schur
   complement when its `amg` setting is on. Whether any case of `baseline_hashes` reaches a coarsest factorisation with an
   interchange is not known from the reading; it is measured (X1, X2 below).

## Arithmetic before this registration (disclosed)

**The 3 x 3 example** `A3 = [[1, 1, 0], [1, 2, 2], [0, 2, 5]]` (SPD: leading minors 1, 1, 1). Factorisation: step 0, column 0
holds 1, 1, 0, no strict maximum below, no swap; multipliers 1 and 0; rows become `[1 | 1, 2]`, `[0 | 2, 5]`. Step 1: `|2| > |1|`,
rows 1 and 2 are swapped whole: row 1 `[0 | 2, 5]`, row 2 `[1 | 1, 2]`; multiplier 0.5; row 2 becomes `[1 | 0.5, -0.5]`.
Stored `lu = [[1, 1, 0], [0, 2, 5], [1, 0.5, -0.5]]`, `piv = [0, 2, 2]`. For `b = A3 (1, 1, 1) = (2, 5, 7)`:

- interleaved (the current solve): `k = 0`: `x1 = 5 - 0 * 2 = 5`, `x2 = 7 - 1 * 2 = 5`; `k = 1`: swap `x1, x2` (no change in
  value: 5, 5), `x2 = 5 - 0.5 * 5 = 2.5`; back substitution `x2 = 2.5 / -0.5 = -5`, `x1 = (5 - 5 * (-5)) / 2 = 15`,
  `x0 = 2 - 15 = -13`. **`x = (-13, 15, -5)`**;
- interchanges first: `P b = (2, 7, 5)`; forward `y = (2, 7, 5 - 2 - 3.5 = -0.5)`; back `x2 = 1`, `x1 = (7 - 5) / 2 = 1`,
  `x0 = 1`. **`x = (1, 1, 1)`**.

Every operation is exact in binary floating point (halves and small integers), so both results are bit-exact.

**The 20 x 20 example** (check L3): tridiagonal, `d_i = 1e-4` for even `i` and `1` for odd `i`, off-diagonals
`-0.4 sqrt(d_i d_{i+1})` (= `-4e-3`). The symmetrically scaled matrix is `I - 0.4 T` with `T` the path adjacency, eigenvalues
in `[1 - 0.8, 1 + 0.8]`, so it is SPD with scaled condition number at most 9. Step 0: `|a_10| = 4e-3 > a_00 = 1e-4`: swap.
Step 1: the updated `a_11` is `-4e-3 - 0.025 * 1`, about `-0.029` in magnitude against `4e-3` in row 2: no swap. Step 2: the
updated `a_22` is about `1e-4 - 1.9e-5` against `4e-3` in row 3: swap, and row 2 carries the nonzero multiplier of column 1,
so the interleaved solve is wrong. Interchanges are expected at the even steps. With `x = (1 + i / 20)` and `b = A x`, a
backward-stable solve has a residual of a few `n eps |A| |x|`, about `1e-15 |b|` here (`|A| |x|` and `|b|` within a factor 2).

**Double-double.** `TwoSum(a, b)`: `s = a + b`, `z = s - a`, `e = (a - (s - z)) + (b - z)`, `a + b = s + e` exactly.
`TwoProd(a, b)`: `p = a b`, `e = mul_add(a, b, -p)`, `a b = p + e` exactly. A dot product accumulated as `(hi, lo)` with both
is accurate to about `n eps^2` relative to `sum |a_i b_i|` plus one rounding. Check L6 uses `(1e16, 1, -1e16) . (1, 1, 1) = 1`
(plain f64 gives 0: `1e16 + 1` rounds to `1e16`).

**Cost.** The heavy record H restores the state of step 2940 (ADR 0363 H1), runs 11 steps of T (about 1 s each) with the
capture on from step 2950, then a few AMG-CG solves of 53 136 unknowns (up to 3000 iterations for the default one, about 1 to 2
min) and the per-level measurements (each a few W-cycles): about 3 to 6 minutes in total. `#[ignore]`d, run once.

## Design (D1 to D3)

**D1. The opt-in corrected coarse solve (`rcfd-linalg`, `amg.rs`).** A new field `AmgSettings::coarse_lu_interchanges_first:
bool`, `false` in `Default`. `DenseLu` stores it. In `DenseLu::solve` the `false` branch is the current loop, unchanged
operation for operation; the `true` branch applies every interchange to `x` first (`for k: x.swap(k, piv[k])`), then the unit
lower substitution, then the upper (the `amg_elastic.rs` solve). `DenseLu::new` is not changed.

**D2. Read-only accessors (`amg.rs`)**, none of which writes: `coarse_interchanges() -> Vec<usize>` (the steps `k` with
`piv[k] != k`, empty when the coarsest level is above 2000 unknowns); `level_operator(l) -> &CsrMatrix`;
`level_prolongator(l) -> Option<&CsrMatrix>`; `cycle_at(l, b, x)` (the existing private `cycle` from level `l` and the given
`x`); `coarse_solve(b, x)` (the dense solve of the coarsest level, as the cycle calls it).

**D3. Measurements, test side.** New files: `crates/rcfd-linalg/tests/amg_coarse_lu_f474.rs` (fast checks L1 to L6, the
double-double dot) and `crates/rcfd-fv/tests/amg_definiteness_f474.rs` (the heavy record H, the DFG helpers and the checkpoint
reader copied from `immersed_bounded_stop_f466.rs`, which is not edited, and a fast check F1 on the M2 captures). Neither
`cg`, `cycle`, `FvMatrix::solve_amg` nor any `rcfd-fv` source is changed.

No guard on `rz <= 0` inside `cg` is added (it would change any solve that passes through a non-positive `rz` and still
converges, ADR 0363 Q6): the acceptance's second branch is used only if the first fails, and then by an amendment (Q4).

## Registered checks

### Fast (the gate runs them)

| Id | Check | Limit | Estimate |
| --- | --- | --- | --- |
| L1 | the defect, default: single-level `Amg` on `A3` (n = 3 <= `max_coarse`), `apply(b = (2, 5, 7))` | `z = (-13, 15, -5)` bit for bit; `coarse_interchanges() == [1]` | the arithmetic above |
| L2 | the opt-in on `A3` | `z = (1, 1, 1)` bit for bit | the arithmetic above |
| L3 | the 20 x 20 example, `b = A x`, `x_i = 1 + i / 20` | interchanges at some step `k >= 1` (recorded); opt-in `|b - A z| / |b| <= 1e-12` (F-471's limit); the default's residual printed | 1e-15 |
| L4 | the opt-in is bit-identical without interchanges: 2D 5-point Laplacian 40 x 40 (multi-level), and `b` as in the existing tests | `coarse_interchanges()` empty; `apply` with and without the flag bit-equal (0 differing values) | reading 1 |
| L5 | the accessors: `cycle_at(0, b, 0)` against `apply(b)`; `level_operator(l).n` against `level_sizes()`; `level_prolongator` `Some` on every level but the last; `coarse_solve` against `cycle_at(last)` | bit-equal; equal; as stated; bit-equal | D2 |
| L6 | the double-double dot | `(1e16, 1, -1e16) . (1, 1, 1) == 1.0` exactly | TwoSum |
| F1 | the M2 captures (ADR 0363 S2 set-up: M2, bounded, 3 steps): per capture the coarsest interchanges recorded; where there is none, AMG-CG with and without the flag bit-equal with equal iteration counts | 0 differing values where no interchange; the counts printed | reading 1 |

### The heavy record H (one run, `#[ignore]`, release, `RAYON_NUM_THREADS` 4, `F466_DIR` = `w49i-work/f466-scratch/runs`)

A fresh T solver with the bounded forcing restored from `state_2940.bin` (ADR 0363 H1), the capture on from step 2950, stepped
to the stop. The systems: `S` = step 2951 corrector 1 (the stopping one), `S0` = step 2951 corrector 0 (406 iterations in ADR
0363), `G` = step 2950 corrector 1 (healthy).

| Id | Measurement | Limit or decision number | Estimate |
| --- | --- | --- | --- |
| H0 | reproduction: the run stops at step 2951; `S`'s captured solve has non-finite `p` and iteration count 3000; the traced default AMG-CG on `S` from its `p0` has `rz` after iteration 1 printed as `-2.006e-9` (`{:.3e}`) | as stated | ADR 0363 M5, the same code |
| H1 | coarsest interchanges of the default hierarchies of `S`, `S0`, `G` | counted, positions printed; **> 0 on `S` is the defect active on the stopping system** | reading 2 |
| H2 | coarse solve relative residual `|b - A_c x| / |b|`, default and opt-in, on (a) the vector the coarse solve receives in the first visit of the first application on `S`'s initial residual, (b) 10 deterministic vectors | opt-in `<= 1e-12` on all eleven (F-471's limit); default printed | backward stable; coarsest condition near the pivot ratio 2.4e4 or more |
| H3 | Rayleigh quotient of the cycle per level on `S`: for each level `l`, `B_l v = cycle_at(l, v, 0)`, the quotient `v^T B_l v / v^T v` for the vector `v` that level receives in the first application on `S`'s initial residual and for 10 deterministic vectors; the asymmetry `|u^T B_l v - v^T B_l u| / (|u| |B_l v| + |v| |B_l u|)` over 5 pairs of them; default and opt-in | opt-in: every quotient `> 0` at every level, asymmetry `<= 1e-8`; default: the finest level with a negative quotient or an asymmetry `> 1e-8` is printed | the coarse solve is the only non-symmetric part (reading 3) |
| H4 | the coarse operators and the smoothed prolongators of `S` against double-double: per level, `max |A_{l+1} v - P^T (A_l (P v))_dd| / max |(..)_dd|` for 3 deterministic `v`; the symmetry `max |a_ij - a_ji| / max |a|` of every `A_l`; the double-double quotient `v^T A_l v` (positive or not) for 10 vectors; the smallest column 2-norm of every `P`; the coarsest operator's Cholesky pivots (all positive or not) | Galerkin defect `<= 1e-12`, symmetry `<= 1e-12`; quotients, column norms and pivots printed (a non-positive value names the level) | a product of a few sums of rounded terms: `1e-15` |
| H5 | **the repair**: AMG-CG with the opt-in on `S` from its `p0`, the captured controls | converged, `rz > 0` at every iteration, iterations `<= 100` | 10 (every healthy solve of steps 2949, 2950) to 20 |
| H6 | the same on `S0` | converged, `rz > 0` at every iteration, iterations `<= 100` | as H5 (406 with the default) |
| H7 | H5's solution against incomplete-Cholesky CG on `S` (the same start and controls) | `max |p_amg - p_ic| <= 1e-4 max |p_ic|` | both converge to `2.5e-14`; error amplification below `1e4` |

H1 to H4 are measurements whose numbers decide the branch; H0, H2's opt-in limit, H3's opt-in limits, H4's two limits and
H5 to H7 are pass/fail.

### What follows from H (decided now)

- **H1 > 0 on `S`, default fails H3 (negative quotient or asymmetry) and the opt-in holds H2, H3, H5 to H7**: the cause is the
  coarsest-solve order defect of F-471. The located statement is the level from H3 and the interchange positions from H1.
  F-474's first branch is reached as an opt-in.
- **H1 = 0 on `S`, or the opt-in does not hold H5**: the defect is not (or not alone) the cause; H3 and H4 locate the level
  and the object (a coarse operator that is not positive definite or not symmetric, a smoother, a prolongator with a zero
  column). A remedy or the `rz <= 0` guard of the acceptance's second branch is registered by an amendment before its code.

### The default switch, measured on a scratch copy (X1 to X3)

Not committed. A `git archive` copy of the branch tip in `f474-scratch`, its own target directory, release.

| Id | What | Recorded |
| --- | --- | --- |
| X1 | the default unchanged, an `eprintln` in `DenseLu::new` (n and the interchange steps) and in `baseline_hashes.rs`'s `run` (the first 40 characters of the case text), `cargo test -p rcfd-api --release --test baseline_hashes -- --nocapture --test-threads=1` | per case: factorisations, those with interchanges, where |
| X2 | `Default` with `coarse_lu_interchanges_first: true`, no other change: `baseline_hashes`, `case_hashes_w48u`, `cargo test -p rcfd-linalg --release`, `cargo test -p rcfd-fv --release --lib`, `immersed_bounded_stop_f466` (fast tests), `consistent_defaults_f301`, `consistent_periodic_defaults_f367` | every failing test and changed hash (got against registered) |
| X3 | the same copy: `immersed_bounded_stop_f466`'s H2 (restart from step 2900 to the stop, minutes) | whether T still stops at step 2951 |

The remaining test files of `rcfd-fv` and `rcfd-api` that use the AMG (about 24, several taking 15 to 40 minutes) are not run
under the switch; they are listed as what the maintainer's gate would run if the default is switched.

### Mutations (scratch copy, each alone, the named fast test once in release, the file restored and compared byte for byte)

| Id | Mutation | Must fail |
| --- | --- | --- |
| MU1 | the opt-in branch of `DenseLu::solve` runs the interleaved loop (the flag ignored) | L2 |
| MU2 | `Default` has `coarse_lu_interchanges_first: true` | L1 |
| MU3 | `coarse_interchanges` returns an empty vector | L3 |
| MU4 | `cycle_at` starts at level `l + 1` when `l + 1` exists | L5 |
| MU5 | the double-double sum drops its error term | L6 |
| MU6 | the opt-in branch applies the interchanges after the forward substitution | L2 |

H is not run under mutations.

## Decision rule

**F-474**:

- **done**: L1 to L6 and F1 held; H0 held; H located the cause (either branch of "What follows"); on the first branch H2, H3
  (opt-in limits), H4 (its two limits), H5, H6 and H7 held; `baseline_hashes` passes with the final library; X1 to X3 recorded
  and reported; every mutation caught by its test without a post-run amendment.
- **partial**: a registered check missed (kept as `*_known_miss`); or a mutation not caught, or caught only after a post-run
  amendment; or the cause located but neither branch of the acceptance reached.
- **todo**: the cause not located.

**F-471** (diagnosis only here): at most **partial** by the reading above (the default solve is not changed, and the full
`rcfd-fv` and `rcfd-api` suites are not run under the switch); partial if L1 to L3, H1, H2 and X1, X2 were made and held;
otherwise todo.

## Limits (not covered)

One stopping system of one run (T, bounded forcing, `dt = 1e-3`); the measurements do not establish that no other AMG
breakdown exists. The opt-in is not reachable from a case file, nor from `PimpleSettings` (`FvMatrix::solve_amg` builds its AMG
with `AmgSettings::default()`); a fluid run with the corrected solve exists only on the scratch copy of X2 and X3. No change of
`cg`.

## Open questions

- **Q1. The default.** Options: (a) keep the corrected order opt-in; (b) switch `Default` (F-471's acceptance). (b) moves
  every result whose coarsest factorisation interchanges rows; it is the maintainer's, with X1 to X3 as the evidence. Not taken.
- **Q2. F-471's status from this ADR.** Taken on the maintainer's standing instruction of 2026-11-11 (take the
  recommendation): at most partial, as above.
- **Q3. "Records where" on the baseline set.** Options: (a) the scratch instrumentation X1 plus the committed fixed-matrix
  checks; (b) a library hook. Taken on the standing instruction: (a); (b) is state in the default path.
- **Q4. The `rz <= 0` guard in `cg`.** Options: (a) add it only by an amendment if H does not reach the first branch; (b) add it
  anyway. Taken on the standing instruction: (a); (b) changes default solves (ADR 0363 Q6) and is the maintainer's.

## Amendment 1 (2026-11-12, after the registered run of L1 to L6, before any code or run of what it registers)

Written after the fast checks L1 to L6 ran once (code f08d7df9, log `f474-scratch/run-linalg-fast.log`), before F1, H and X1
to X3 ran. Nothing above this amendment is changed.

**What happened.** L1, L2, L3, L5, L6 held (L1: `(-13, 15, -5)`; L2: `(1, 1, 1)`; L3: interchanges at steps 0, 2, ..., 18,
relative residual default 4.07e-3, opt-in 8.9e-19). **L4 missed its first clause**: the coarsest factorisation of the 40 x 40
Laplacian hierarchy interchanges rows at step 0 (`coarse_interchanges() == [0]`), against the registered "empty". The
assertion stopped the test, so its bit-equality clause did not run. The set-up assumed the reading 2 property (an M-matrix
never interchanges) for the coarsest Galerkin operator, which is not an M-matrix.

**The corrected reading.** An interchange at step 0 is harmless to the interleaved order: no multiplier is stored before it,
so the interleaved and the interchanges-first solves perform the same operations in the same order. The two differ only
when some step `k >= 1` interchanges (reading 1 said "no interchange"; the exact condition is "no interchange at a step
`k >= 1`"). The same correction applies to the checks not yet run:

- **L4** is kept as the ignored record `l4_opt_in_is_bit_identical_without_interchanges_known_miss` with its limit intact and
  the measured `[0]` recorded.
- **L4b** (new, fast): on the same 40 x 40 Laplacian and right-hand side, every interchange is at step 0 (limit: the steps
  are a subset of `{0}`), and `apply` with and without the flag is bit-equal (0 differing values).
- **F1** (not yet run): its neutrality condition becomes "no interchange at a step `k >= 1`" (in place of "no interchange");
  the limit is unchanged otherwise.
- **H1** (not yet run): "the defect active on the stopping system" means an interchange at a step `k >= 1`; the steps are
  printed as registered.
- **X1**: the record prints, per factorisation, the steps; the summary counts those with an interchange at `k >= 1`.

**Mutation for L4b.** MU7: the opt-in branch skips the interchange of step 0 (`for k in 1..n` in the first loop); must fail
L4b. (MU1 to MU6 unchanged.)

**Rule.** Unchanged. L4 is a registered check missed in its registered run, so by the rule F-474 is at most **partial**
unless the maintainer accepts this amendment.

## Addendum 1: results (2026-11-12)

Measured against the limits above and in amendment 1. No limit was changed. Each registered run was made once, in release.
L1 to L6 ran at f08d7df9 (`f474-scratch/run-linalg-fast.log`). L4b, F1 and H ran at fb6e43cf, the code of amendment 1
(`run-l4b.log`, `run-f1.log`, `run-h.log`; H with `RAYON_NUM_THREADS` 4 and `F466_DIR` = `f466-scratch/runs`, 19 s). X1 to X3
ran on a `git archive` copy of fb6e43cf (`f474-scratch/xsrc`, target `target-x`; logs `x1.log`, `x2-*.log`, `x3.log`). The
mutations ran on a second copy (`mutsrc`, target `target-mut`, `mutate.py`, `mutations.log`).

Commits: registration b50cd5cd; library and tests f08d7df9; amendment 1 002fd9f1; its code fb6e43cf (L4 renamed to an ignored
known miss, L4b added, the F1 condition and the H1 print changed to "a step `k >= 1`", all before their runs); this addendum.

### Results

| Id | Check | Limit | Measured | Met |
| --- | --- | --- | --- | --- |
| L1 | default solve on `A3` | `(-13, 15, -5)` bit for bit; interchanges `[1]` | `(-13, 15, -5)`; `[1]` | yes |
| L2 | opt-in on `A3` | `(1, 1, 1)` bit for bit | `(1, 1, 1)` | yes |
| L3 | 20 x 20, opt-in residual | an interchange at `k >= 1`; `<= 1e-12` | steps 0, 2, ..., 18; opt-in 8.9e-19 (default 4.07e-3) | yes |
| L4 | Laplacian 40 x 40: no interchange, bit-equal | interchanges empty | **`[0]`**; bit-equality not reached | **no** (`l4_..._known_miss`) |
| L4b | the same, interchanges only at step 0, bit-equal (amendment 1) | subset of `{0}`; 0 differing | `[0]`; 0 differing | yes |
| L5 | accessors | bit-equal, equal, as stated | yes | yes |
| L6 | double-double dot | `== 1.0` | 1.0 | yes |
| F1 | M2 captures, neutral without an interchange at `k >= 1` | 0 differing, same iterations | 6 of 6 systems have no interchange at all (coarsest 89 to 93 unknowns); all 6 bit-equal, same iterations | yes |
| H0 | stop at 2951; captured `S` 3000 iterations, non-finite; default `rz` after iteration 1 `-2.006e-9` | as stated | 2951; 3000, 53 136 non-finite; `-2.006e-9` (start `+5.52e-9`; `rz <= 0` at 2616 of 3001 iterations) | yes |
| H1 | coarsest interchanges | counted | **`S`: steps 13, 16, 17; `S0`: the same; `G` (step 2950): 14, 16, 17, 20, 21**; coarsest 24 unknowns, pivots 9.4e-10 to 2.3e-5, none guarded | located: the defect is active |
| H2 | coarse solve relative residual | opt-in `<= 1e-12` | opt-in at most **5.3e-15** on all eleven; default **0.213** on the received vector, 0.083 to 0.86 on the ten others | yes |
| H3 | cycle quotients and asymmetry per level | opt-in `> 0`, asymmetry `<= 1e-8` | opt-in: every quotient positive (8.8e6, 1.6e6, 2.1e5, 4.9e4, 4.0e4 on levels 0 to 4), asymmetry at most 1.2e-15; **default: negative minimum quotient on level 4 (-7.0e8, the coarse solve alone) and level 0 (-3.4e7), asymmetry 4.2e-3, 7.5e-3, 1.2e-2, 5.1e-2, 0.13 on levels 0 to 4** | yes |
| H4 | operators and prolongators against double-double | Galerkin `<= 1e-12`, symmetry `<= 1e-12` | Galerkin at most 5.8e-16, symmetry at most 5.4e-16; every `v^T A_l v` positive (min 1.3e-5 to 3.1e-5 per unit `v^T v`); min column norms of `P` 1.09, 1.15, 1.22, 0.13; coarsest Cholesky pivots 2.6e-10 to 2.3e-5, all positive | yes |
| H5 | opt-in AMG-CG on `S` | converged, `rz > 0`, `<= 100` | **10 iterations**, converged, every `rz > 0` | yes |
| H6 | opt-in AMG-CG on `S0` | the same | **10** (default 406), every `rz > 0` | yes |
| H7 | against IC-CG | `<= 1e-4` | 4.1e-9 (IC-CG 448 iterations) | yes |

**The cause, located.** The hierarchy itself is sound: every operator is symmetric to rounding, positive definite (dd
quotients, Cholesky of the coarsest), the Galerkin products agree with a double-double product to 6e-16, and no prolongator
column is zero (H4). The only defective part is the dense coarsest solve. On the coarsest operator of the stopping system
(24 unknowns) partial pivoting interchanges rows at steps 13, 16 and 17 (H1). The default interleaved solve then misses by a
relative residual of 0.21 (H2). That makes the coarse correction non-symmetric (asymmetry 0.13 at level 4, falling to
4.2e-3 at level 0) and indefinite (negative quotients at level 4 and, through the W-cycle, at level 0; H3), so
`r^T M^-1 r < 0` (H0). With the interchanges applied first (the opt-in), the cycle is symmetric to 1e-15 and positive on every
level, and AMG-CG converges in 10 iterations on the stopping system and on corrector 0 (406 with the default). This is the
F-471 defect (ADR 0305 question 1), and nothing else was found.

Also seen: the healthy system `G` of step 2950 has interchanges at `k >= 1` too, so its default coarse solve is also wrong,
yet CG converged in 10 iterations. The defect is present in solves that look healthy. Breakdown happens only when the wrong
correction happens to make `r^T z` negative.

### The default switch (X1 to X3, scratch copy only)

| Id | Measured |
| --- | --- |
| X1 (default, instrumented `baseline_hashes`) | Only two cases build an AMG: `kepsilon_channel` (60 factorisations, coarsest 56 unknowns) and `cavity-coupled` (59, the Schur complement, coarsest 71). **No interchange in any of them.** No other baseline case reaches an AMG. |
| X2 (`Default` with the flag `true`) | `baseline_hashes` **3 passed, 1 ignored: no registered hash moves**; `case_hashes_w48u` passed; `rcfd-linalg` all test files pass except this ADR's **L1**, which pins the old default (expected); `rcfd-fv --lib` 104 passed; `immersed_bounded_stop_f466` fast S1 to S7, `consistent_defaults_f301` (6), `consistent_periodic_defaults_f367` (1), `amg_definiteness_f474` F1 passed |
| X3 (ADR 0363 H2 under the switch) | **T no longer stops**: no non-finite pressure up to step 2960, every solve of steps 2949 to 2960 10 iterations; the Courant numbers differ from ADR 0363 H1's from step 2917 on (42 of 60 steps), so the default T bounded run of ADR 0268 R1 / ADR 0363 H1 moves; the record H2 fails ("M2: H2 did not stop") |

So a default switch moves **no registered hash** (none of the baseline cases interchanges at all). It moves the fluid results of
any AMG pressure solve whose coarsest factorisation interchanges at a step `k >= 1`. Here that is the T bounded run (the stop
at 2951 goes away, and the results differ from step 2917 on). It would also change what ADR 0363's records H1, H2 and H3a
assert (the stop and the single IC repeat). Not run under the switch: the other AMG-using test files of `rcfd-fv` and `rcfd-api`
(about 24, listed by grep in the registration; several take 15 to 40 minutes). The lead's gate would run them.

### Mutations

| Id | Mutation | Must fail | Result |
| --- | --- | --- | --- |
| MU1 | opt-in branch interleaved | L2 | failed: `z = [-13, 15, -5]` |
| MU2 | `Default` true | L1 | failed: `z = [1, 1, 1]` |
| MU3 | `coarse_interchanges` empty | L3 | failed: "no interchange at a step k >= 1" |
| MU4 | `cycle_at` from `l + 1` | L5 | failed: "cycle_at(0) differs from apply" |
| MU5 | dd sum without its error term | L6 | failed |
| MU6 | interchanges after the forward substitution | L2 | failed: `z = [-24.25, 26.25, -10]` |
| MU7 | opt-in skips the step-0 interchange (amendment 1) | L4b | failed: "differing values" |

Each file was restored and compared byte for byte (equal every time).

### Tools and the defaults

- `cargo fmt -p rcfd-linalg --check`, `cargo fmt -p rcfd-fv --check`: clean. `cargo clippy -p rcfd-linalg -p rcfd-fv
  --all-targets -- -D warnings`: clean.
- On the branch, release: `baseline_hashes` 3 passed, 1 ignored; `cargo test -p rcfd-linalg` all passed (L4 ignored as the
  known miss); `rcfd-fv --lib` 104 passed; `immersed_bounded_stop_f466` 7 passed (4 ignored); `amg_definiteness_f474` F1
  passed.
- Shared solver file changed: `crates/rcfd-linalg/src/amg.rs` only. New lines 30-33 (the field), 45 (its default), 70
  (`DenseLu::interchanges_first`), 74 (`new`'s parameter), 111-116 (the struct literal), 122-139 (the opt-in branch of
  `solve`), 354-357 (the call in `Amg::new`), 449-479 (the accessors). The default branch of `DenseLu::solve` is the old loop,
  unchanged. No `rcfd-fv` or `rcfd-api` source changed.

### Deviations and disclosures

- L4 missed its registered limit (the set-up assumed that the coarsest Galerkin operator never interchanges). It is kept as
  an ignored `*_known_miss`, and L4b was registered by amendment 1 before its run.
- `cargo test -p rcfd-linalg` in X2 stopped after the first failing test binary (L1). It was run again with `--no-fail-fast`
  to reach the remaining files. That completes the registered run and does not repeat a result.
- F1 turned out to be a check without interchanges (the M2 coarsest levels interchange nothing), so on M2 it shows the
  opt-in neutral where it must be. It does not test a case with interchanges only at step 0; L4b does.
- The `rz <= 0` guard in `cg` was not added (Q4 (a)): the first branch was reached.

### Decision by the registered rule

- **F-474**: the cause was located by the registered measurements (H1 to H4: the coarsest dense LU order defect, interchanges
  at steps 13, 16 and 17, coarse residual 0.21, a non-symmetric, indefinite cycle, everything else sound). The opt-in repair
  holds H2, H3, H5, H6 and H7. AMG-CG converges on that system in 10 iterations, every `rz > 0`, every registered hash
  unchanged (with the opt-in off, and also with the default switched, X2). All mutations were caught. But **L4 missed its
  registered limit in its registered run**, so by the rule the status is **partial** unless the maintainer accepts
  amendment 1.
- **F-471**: **partial**, as registered. The defect is shown (L1, H1, H2), the corrected order is built and solves to 5e-15 and
  9e-19, and the baseline set is recorded (X1: no interchange) with the hashes of a switch (X2: none moves). The default solve
  is not changed, and the full `rcfd-fv`/`rcfd-api` suites were not run under the switch.

### Remaining questions

- **Q1. Switch the default** (`coarse_lu_interchanges_first: true`, or drop the flag and fix `DenseLu::solve`). Options: (a)
  keep the opt-in; (b) switch. Recommendation **(b)**: the default solve is wrong whenever a coarsest factorisation
  interchanges at a step `k >= 1`, which happens silently even in solves that converge (`G`). No registered hash moves (X2).
  This is the maintainer's decision. Before switching, the gate should run the AMG-using test files. ADR 0363's records H1, H2
  and H3a assert the stop at 2951, and this ADR's L1 asserts the old default, so they would need new ignored records or an
  amendment. They are not edited here.
- **Q2. ADR 0363's opt-in `pressure_ic_fallback`.** With the switch it is not needed on T. Recommendation: keep it as a safety
  net (it costs nothing on finite solves).
- **Q3. The `rz <= 0` guard in `cg`** (ADR 0363 Q6). Options: (a) a separate item; (b) drop it. Recommendation (a), low
  priority: it would have stopped this solve 800 iterations earlier with a located message, but it changes solves that pass
  through `rz <= 0` and still converge, so it needs the hash check.

### For the lead: roadmap entries

**F-474: proposed status `partial`** (Rule I: L4 missed in its registered run, corrected by amendment 1; `done` if the
maintainer accepts the amendment). Proposed note:

> PARTIAL (ADR 0369, 2026-11-12). From ADR 0363 Q4/Q6. Cause located on the stopping system of ADR 0363 (T, bounded forcing, step 2951 corrector 1, rebuilt from the state of step 2940): it is the coarsest dense LU order defect of F-471. The coarsest operator (24 unknowns) interchanges rows at steps 13, 16 and 17; the default interleaved DenseLu::solve misses by a relative residual of 0.21; the cycle becomes non-symmetric (0.13 at the coarsest level, 4.2e-3 at the finest) and indefinite (negative Rayleigh quotients at levels 4 and 0), hence r^T M^-1 r < 0. Everything else is sound against double-double: Galerkin products to 6e-16, every operator symmetric to 5e-16 and positive definite, no zero prolongator column. Delivered (opt-in, defaults bit-identical, baseline_hashes pass): AmgSettings::coarse_lu_interchanges_first applies every interchange before the forward substitution; with it the coarse solve is exact to 5e-15, the cycle symmetric to 1e-15 and positive on every level, and AMG-CG converges on the stopping system in 10 iterations (corrector 0: 10 against 406), agreeing with IC-CG to 4e-9; read-only accessors Amg::coarse_interchanges, level_operator, level_prolongator, cycle_at, coarse_solve. With the default switched on a scratch copy no registered hash moves (kepsilon_channel and cavity-coupled, the only baseline cases with an AMG, never interchange), T no longer stops at 2951, and its run differs from ADR 0363 H1 from step 2917 on. Seven mutations caught. Partial by Rule I: the check L4 missed (the Laplacian's coarsest LU interchanges at step 0, harmless to the order) and was corrected by amendment 1. Open: switching the default (maintainer; recommended).

**F-471: proposed status `partial`.** Proposed note:

> PARTIAL (ADR 0369, 2026-11-12, with F-474). The defect is shown in rcfd_linalg::amg DenseLu: whole-row swaps in the factorisation with interleaved interchanges in the solve give (-13, 15, -5) for (1, 1, 1) on a 3 x 3 SPD matrix, a relative residual of 4e-3 on a 20 x 20 SPD tridiagonal, and 0.21 on the coarsest operator of the T pressure system of ADR 0363, where it is the cause of the AMG-CG breakdown (F-474). Interchanges at step 0 alone are harmless. On the baseline set only kepsilon_channel (coarsest 56) and cavity-coupled (Schur, coarsest 71) reach an AMG, and neither interchanges (scratch instrumentation). The corrected order is built as the opt-in AmgSettings::coarse_lu_interchanges_first (5e-15 and 9e-19 relative residual). With it as the default on a scratch copy, baseline_hashes, case_hashes_w48u, rcfd-linalg (except ADR 0369 L1, which pins the old default), rcfd-fv lib and the defaults tests f301/f367 pass, so no registered hash moves; the T bounded run of ADR 0268/0363 changes (no stop, different from step 2917). Remaining (NEEDS THE USER): switch the default and run the AMG-using rcfd-fv/rcfd-api test files in the gate; ADR 0363's records H1, H2 and H3a and ADR 0369 L1 assert the old behaviour.

**F-466 / F-354: no change of status;** proposed addition to F-466's note:

> ADR 0369 (2026-11-12, F-474): the AMG breakdown at step 2951 is the coarsest dense LU order defect (F-471); with AmgSettings::coarse_lu_interchanges_first the stopping system converges in 10 iterations and T does not stop.
