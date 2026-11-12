# ADR 0354: F-457 registration: scale-aware convergence test of the subspace eigensolver

Status: **REGISTERED 2026-11-12, written before any code of F-457 and before any run of its checks**. Not yet accepted by the
maintainer. Before this text nothing of F-457 was written, compiled or run: the code and documents under "What was read" were
read, and the arithmetic under "Arithmetic before this registration" was done by hand and with a short Python script of closed
forms (`w49i-work/f457-scratch/arith.py`, output `arith.log`) that runs no library code. Results go in the addendum at the end;
nothing above the addendum is changed after a run.

Parents: [ADR 0283](0283-generalised-symmetric-eigensolver.md) (the eigensolver, D6 the residual test and its floor) and
[ADR 0284](0284-mass-matrix-and-modal-analysis.md) (amendment 1: the slender cantilever stops at the residual floor of the
default test; remaining question 2, option (c), is this item). Roadmap item F-457 (deps F-382, F-383). Branch
`f457-scale-aware-eigen` from main 910fbe22, worktree `w49i-work/f457`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run; a miss becomes an ignored
`*_known_miss` test with the limit intact and the number recorded, diagnosed and not tuned; each registered run is made once;
existing tests, ignored records, `baseline_hashes.rs`, the hash tables and the goldens are not edited; defaults bit-identical,
the new test opt-in; no new crate, no unsafe code, no trailers, no push. `crates/rcfd-fem/src/solve.rs` is being edited by
another agent (F-381) and is not touched.

## What F-457 asks

The acceptance of `docs/roadmap.json`, binding:

> Every limit not stated here is registered in the ADR before the run. rcfd_linalg::eigen::lowest_eigenpairs gains a residual
> test relative to an estimate of ||A|| ||x|| (for example from the factorisation or a few power steps), selectable without
> changing the existing default; the 40 x 2 x 2 Tet10 cantilever of ADR 0284 then converges with the scale-aware tolerance
> 1e-12 in at most 20 iterations, its first two bending frequencies agree with the runs of ADR 0284 amendment 1 within 1e-10
> relative, and every check of ADR 0283 still passes with the default unchanged.

Clauses: (a1) a residual test relative to an estimate of `||A|| ||x||`; (a2) selectable, the default unchanged; (a3) the Tet10
40 x 2 x 2 cantilever converges at the scale-aware tolerance 1e-12 in at most 20 iterations; (a4) its first two bending
frequencies (both members of each pair, four numbers) agree with the amendment-1 run within 1e-10 relative; (a5) every check of
ADR 0283 passes with the default unchanged.

**Readings (registered here).**

1. "The runs of ADR 0284 amendment 1" for the Tet10 40 x 2 x 2 cantilever is the run of `t2_amended_tet10_cantilever_covering`
   (default residual test, tolerance 1e-7, 7 iterations), log `w49i-work/f383-scratch/run2_amended_fast.log`, which prints the
   six frequencies in full (`{:?}`, shortest round-trip): **40.83463169470747, 40.843159320566095, 253.00307073453942,
   253.07827467142752**, 696.06213246288, 696.3847805894118 Hz (the first four are the two bending pairs; modes 0 to 5 are all
   bending there, `e_y + e_z >= 0.02`). They are copied here so that the reference lives in the repository. The test also
   re-runs that configuration on this tree and requires it to give these bits (check D0): this is the "default unchanged" seen
   on the run that the acceptance compares with.
2. "Every check of ADR 0283" is the test file `crates/rcfd-linalg/tests/sb_eigen.rs` (E1 to E17), run unedited in release
   (E4-style: the existing tests, not new copies).
3. "Converges ... in at most 20 iterations": `lowest_eigenpairs` returns `Ok` (including the Sturm check) with `iterations <=
   20`, through `modal_analysis` with `ModalOptions::new(6)` and the new test selected at tolerance 1e-12, everything else as
   the amendment-1 run.

## What was read (main 910fbe22, read only)

1. `AGENT_RULES.md`; ADR 0283 in full (D3 to D8, checks, mutations, addendum headings); ADR 0284 in full (D3, D4, D6,
   amendment 1, addendum 1, remaining question 2, the follow-up text); ADR 0353 (style); `docs/roadmap.json` F-457.
2. Code: `crates/rcfd-linalg/src/eigen.rs` (all), `crates/rcfd-fem/src/modal.rs` (all), `crates/rcfd-fem/tests/sb_modal.rs`
   (lines 1 to 212 helpers, 600 to 810 the cantilever tests), `crates/rcfd-linalg/tests/sb_eigen.rs` (the uses of
   `SubspaceOptions`: every construction is `SubspaceOptions::default()` or `{ .., ..SubspaceOptions::default() }`, so a new
   field compiles without editing it). `git grep "eigen::"`: the only library user of `lowest_eigenpairs` is `modal.rs`.
3. Logs of F-383: `f383-scratch/run2_amended_fast.log` (the frequencies above; Hex8 40 x 2 x 2: 43.53945747018705,
   43.539457471918574, 270.17409714143236, 270.1740971417086 Hz, 7 iterations, split of the first pair 3.977e-11).

## Arithmetic before this registration (disclosed)

From `f457-scratch/arith.py` (`arith.log`) and by hand.

1. **T** (ADR 0283 set-up: (2, -1), n = 200, `M = I`, `sigma = 0`): `lambda_1 = 2.4429e-4`, `lambda_10 = 2.4379e-2`,
   `lambda_21 = 0.10677`, `||A||_inf = 4`. The default-equivalent of the backward tolerance 1e-12 (`rho = eta (4 + mu) / mu`,
   D1 below): 1.6e-8 for pair 1, 1.65e-10 for pair 10.
2. **T2 = T^2** (new set-up Q): pentadiagonal integers, eigenvalues `(4 sin^2(j pi / 402))^2`: `lambda_1 = 5.968e-8`,
   `lambda_10 = 5.944e-4`, `lambda_19 = 7.664e-3`, `kappa = ||A|| / lambda_1 = 2.68e8`, `eps kappa = 5.95e-8` (so the default
   floor `c eps kappa` is above 1e-10 unless `c < 2e-3`). Backward 1e-12 is, in the default measure, 2.7e-4 at pair 1 and
   2.7e-8 at pair 10; rate of pair 10 with `q = 18`: `mu_10 / mu_19 = 0.0776`, about 7 iterations from 1 to 2.7e-8.
3. **N** (ADR 0283: 30 x 30 five-point, `M = diag(1 + 0.5 i/29 + 0.25 (j/29)^2)`): `||A||_inf = 8`, `||M||_inf <= 1.75`;
   `lambda_1 ~ 0.0205` (the `M = I` value), so the `|mu| ||M||` term is about 0.45 percent of `||A||` at pair 1, more above.
4. **Cantilever** (Tet10 40 x 2 x 2, amendment 1): `mu_1..6 = (2 pi f)^2 = 6.583e4, 6.586e4, 2.527e6, 2.529e6, 1.913e7,
   1.915e7`. Spectrum above the six (by hand, `omega` in `c / L`, from ADR 0284 arithmetic 2 lowered by Timoshenko): third
   bending pair 0.862, torsion 0.895, axial 1.571, fourth bending pair ~1.66, fifth ~2.6, second torsion ~2.68, sixth bending
   pair ~3.8 (the 14th and 15th): `q = 14`, rates `mu_i / mu_15 = 1.8e-4` (pair 1), `7.0e-3` (pair 2), `5.1e-2` (pair 3). With
   these rates the default test at 1e-7 needs about `ln(1e-7) / ln(0.051) = 5.4` iterations past the start transient for
   pair 3: 7 measured, consistent.
5. **The ratio `lambda_max / lambda_1`** of the cantilever: amendment 1 gives `~6e7` (Hex8, bound) and about ten times that
   for Tet10 (its residual floor is ten times higher), so `lambda_max ~ 4e12` to `4e13`. The default-equivalent of backward
   1e-12 at pair 3 (`rho_6 ~ 1e-12 F / mu_6`, `F = ||A||_inf ||x|| / ||M x||`, of the order of `lambda_max` up to a factor of
   a few): 2e-7 to 2e-6, reached after about 4.4 to 5.2 iterations of the rate 0.051: **5 to 6 iterations** expected.
6. **Rounding noise of a converged frequency** (for clause a4). The Hex8 40 x 2 x 2 first pair of amendment 1, equal in exact
   arithmetic, is split by 3.98e-11 in frequency: that is the rounding noise of a converged Ritz value there. Scaled to Tet10 by
   the residual-floor ratio 10: 4.0e-10; by the floor ratio over `sqrt(N_Tet10 / N_Hex8) = sqrt(6000 / 1080)`: 1.7e-10. The
   second pair is lower by `mu_1 / mu_3 = 1/38`: about 1e-11. See D4 for what part of that noise enters the comparison.

## Design

**D1. The scale-aware residual: the normwise backward error of a pair.** For an approximate pair `(mu, x)` of `A x = mu M x`
(`A = K + sigma M`, ADR 0283 D3) with residual `r = A x - mu M x`, set

`eta = ||r||_2 / ((||A|| + |mu| ||M||) ||x||_2)`.

*Derivation* (first principles; it is the normwise backward error of Higham and Higham, SIAM J. Matrix Anal. Appl. 20 (1998)
493, cited from memory for orientation only). Take `dA = -a r x^T / ||x||^2`, `dM = b r x^T / ||x||^2` with `a, b >= 0` and `a +
|mu| b = 1` (sign of `b` with `mu`). Then `(A + dA) x - mu (M + dM) x = r - a r - mu b r = 0` (for `mu >= 0`): `(mu, x)` is an
exact eigenpair of the perturbed pencil, with `||dA|| = a ||r|| / ||x||`, `||dM|| = b ||r|| / ||x||`. Choosing `a = eta ||A||
||x|| / ||r||`, `b = eta ||M|| ||x|| / ||r||` satisfies `a + |mu| b = 1` and makes `||dA|| / ||A|| = ||dM|| / ||M|| = eta`.
So `eta <= tau` says: the pair is exact for a pencil within relative distance `tau` of `(A, M)` in each matrix. (A symmetric
pair of perturbations, `dA = -a (r x^T + x r^T - (x^T r) x x^T / ||x||^2) / ||x||^2` and alike for `dM`, also satisfies the
equation, at most twice the norm.)

*Why its floor does not depend on the spectrum.* The computed residual carries the rounding of the products: per entry `|fl(r)
- r| <= gamma_w (|A| |x| + |mu| |M| |x|)` (`w` the largest row length, `gamma_w = w eps / (1 - w eps)`, the standard bound of a
dot product). In 2-norms `|| |A| |x| || <= || |A| ||_2 ||x|| <= ||A||_inf ||x||` (`|A|` symmetric, `||.||_2 <= ||.||_inf` for a
symmetric matrix). So with the infinity norms below the computed `eta` has an error of at most `gamma_w` (`w <~ 200` for
Tet10 rows: `<= 4.5e-14`, typically `sqrt(w) eps ~ 3e-15`), and the solve's backward error (ADR 0274 D4, `c eps ||A||` per
column) contributes `c eps`: the floor is `c eps`, against `c eps ||A|| / mu` for the default test of ADR 0283 D6. A tolerance
1e-12 is about 4500 `eps`, above both.

**D2. The estimate of `||A||` and `||M||`.** `||A||_inf = max_i sum_j |a_ij|` and `||M||_inf` alike, computed once from the
assembled `A = K + sigma M` (the matrix that was factorised) and `M`, sequentially in row order. For a symmetric matrix
`||A||_2 <= sqrt(||A||_1 ||A||_inf) = ||A||_inf`, and `||A||_inf <= sqrt(n) ||A||_2`; for finite-element matrices with bounded
row length the overestimate is a small factor. The estimate is an upper bound, so `eta` computed with it is at most the 2-norm
backward error: the test is never looser than the same test with `||A||_2` by more than that factor. It is exact arithmetic on
the stored entries (no iteration, no start vector, no dependence on the thread count). Chosen over power steps (a lower bound,
needing a start vector and a number of steps to register) and over the factor's pivots (which bound `||A||` only loosely).

**D3. Selection and the default.** `rcfd_linalg::eigen::ResidualTest { Relative, Backward }`, `#[default] Relative`;
`SubspaceOptions::residual_test` (default `Relative`). `Relative` is the test of ADR 0283 D6, `rho = ||r|| / ||A x||`, computed
by exactly the same operations as before (the norms of D2 are not computed): every result of the default is the same bits.
`Backward` replaces `rho_i` by `eta_i` of D1 in the convergence test of each wanted pair (pairs `1..m'`, the cluster rule
unchanged) and in what is reported (`GeneralizedEigen::residuals`, `EigenError::NotConverged { tolerance, residuals }`: the
residuals are those of the selected test; their text and the variant are unchanged). Everything else (shift, start, iteration,
Rayleigh-Ritz, Sturm check) is shared. `rcfd_fem::modal::ModalOptions` gains `residual_test` (default `Relative`) and the
builder `residual_test(t)`, passed through to `SubspaceOptions`; `ModalOptions::new` keeps its defaults.

*The Sturm check under `Backward`* (ADR 0283 D8). Its conclusion does not depend on the residual test: Ritz values are upper
bounds, so `nu(s) >= m'`; if `nu(s) = m'` the eigenvalues below `s` are exactly `lambda_1..lambda_m'`. What a looser test
changes is step (i), the Kahan interval of each Ritz value: if an interval reaches past `s`, the count is below `m'` and the
result is the error `SturmMismatch`, not a wrong answer.

**D4. What the cantilever should give.** By arithmetic 5 the backward test at 1e-12 stops after about 5 to 6 iterations
(driven by pair 3, the slowest wanted pair); limit 20 (the acceptance's). At that point pair 1 has been at its floor since about
iteration 3 (rate 1.8e-4) and pair 2 has a residual `<~ 7e-3^5 ~ 2e-11` in the default measure, whose second-order eigenvalue
error is far below 1e-20. So the four compared frequencies are converged in both runs, and their difference is the rounding
noise of a converged Ritz value between two iteration counts of the same factorisation. The factorisation's backward error is
common to both runs (one factor `L D L^T` of the same matrix, the same bits); what differs is the rounding of the triangular
solves, the products and the Rayleigh-Ritz step at different iterations. Estimate (arithmetic 6): first pair **5e-11 to
4e-10** relative (the lower end if the common factor error dominates the Hex8 split, the upper if the solves do); second pair
about 1e-11. **The first-pair part of clause a4 may miss**: the limit 1e-10 is the acceptance's and is kept; a miss is
recorded and diagnosed by G1 (the spread of the same frequency over runs stopping at different iterations), not tuned. If the
backward run stops at iteration 7, the iterates are those of the amendment-1 run and the difference is exactly 0.

**D5. API** (`crates/rcfd-linalg/src/eigen.rs`; `crates/rcfd-fem/src/modal.rs`; no re-export, `lib.rs` untouched):
`pub enum ResidualTest { Relative, Backward }` (`Debug, Clone, Copy, PartialEq, Eq, Default`); field `SubspaceOptions::
residual_test`; field `ModalOptions::residual_test` and builder. Doc comments of `tolerance`, `residuals` and `NotConverged`
name the selected test.

## Set-ups

All runs in release, `--nocapture --test-threads=1`.

- **T**, **N** as ADR 0283 (default `Shift::Auto`, which gives `sigma = 0` for both, ADR 0283 E5, E9); m = 10 and 11.
- **Q** (new): `T^2`, n = 200, entries from the integer stencil (rows 0 and n-1: 5, -4, 1; rows 1 and n-2: -4, 6, -4, 1;
  interior 1, -4, 6, -4, 1), `M = I`, m = 10, `Shift::Value(0.0)` (SPD; registered rather than `Auto` so that the shift is not a
  variable). Closed form `(4 sin^2(j pi / 402))^2`.
- **CANT** as ADR 0284 (`L = 1`, `b = 0.05`, steel, `nu = 0.3`, all components 0 on `xmin`, `modes = 6`), Tet10 40 x 2 x 2
  and Hex8 40 x 2 x 2, through `modal_analysis`; bending modes identified as in ADR 0284 D8 (`e_y + e_z >= 0.02`, ascending,
  pairs (1, 2), (3, 4)). New test file `crates/rcfd-fem/tests/sb_modal_scale.rs` (the helpers it needs copied from
  `sb_modal.rs`, which is not edited); new tests in a new file `crates/rcfd-linalg/tests/sb_eigen_scale.rs`.

## Registered checks

| Id | Clause | Check | Limit | Estimate behind it |
| --- | --- | --- | --- | --- |
| C1 | a3 | CANT Tet10 40 x 2 x 2, `Backward`, tolerance 1e-12: `Ok`; iterations; M-orthonormality with the full `M`; Sturm `counted = expected` | `Ok`; `<= 20`; `<= 1e-9`; equal | D4: 5 to 6 iterations; orthonormality as ADR 0284 (4.2e-15) |
| C2 | a4 | the four bending frequencies of C1 against the amendment-1 values (reading 1): `max |f / f_amend - 1|`, each member | `<= 1e-10` each | D4: first pair 5e-11 to 4e-10 (may miss), second ~1e-11 |
| D0 | a2 | CANT Tet10 40 x 2 x 2 with `ModalOptions::new(6).tolerance(1e-7)` (default test) on this tree: the six frequencies equal the logged amendment-1 values bit for bit, 7 iterations | `==` | the default path is the same operations; same compiler and machine |
| E0 | a5 | `cargo test -p rcfd-linalg --release`: every existing test (ADR 0283's `sb_eigen` E1 to E17 and the rest) passes unedited | pass | D3 |
| E1 | a2 | `cargo test -p rcfd-fem --release`: every existing test (with `sb_modal`, whose amended tests are the amendment-1 runs) passes unedited | pass | D3 |
| E2 | house rule 3 | `cargo test -p rcfd-api --release --test baseline_hashes` | pass | `eigen.rs` is reached only by `modal.rs` |
| L1 | a1 | N, m = 11, `Backward`, tolerance 1e-12: the reported residuals against `||K x - lambda M x|| / ((||A||_inf + |mu| ||M||_inf) ||x||)` recomputed in the test from the returned pairs (`mu = lambda + sigma`) with its own norms; also T (m = 10) | `<= 1e-12` relative each | the same residual vector (same `matvec`), different summation of the norms: `~1e-15` |
| L2 | a1, a2 | T, m = 10: `Backward` 1e-12 against the default run: eigenvalues against the closed form; M-orthonormality; `iterations(Backward) <= iterations(default)`; the default run equals ADR 0283 E5's limits | `<= 1e-10`; `<= 1e-10`; as stated; as E5 | arithmetic 1: `eta_i < rho_i` for `M = I`, `sigma = 0` (`||A x|| <= 4 ||x||`), so the default stop implies the backward one at the same iterate |
| L3 | a1 (extension) | Q, m = 10, `Backward` 1e-12: `Ok`; iterations; eigenvalues against the closed form; M-orthonormality; record the default run (`Relative` 1e-10, cap 200): its outcome, iterations or last residuals | `Ok`; `<= 40`; `<= 1e-6` relative; `<= 1e-10`; recorded | arithmetic 2: ~7 to 12 iterations; eigenvalue rounding `<= c eps kappa = 6e-8 c` worst case; the default likely `NotConverged` (floor `6e-8 c`) |
| H1 | a3, a4 (extension) | CANT Hex8 40 x 2 x 2, `Backward` 1e-12: `Ok`, iterations `<= 20`; four bending frequencies against the logged amendment-1 Hex8 values; the S1 pair splits | `<= 20`; `<= 1e-10`; `<= 1e-6` | Hex8 noise ~4e-11 (its split) |
| G1 | diagnostic (record) | CANT Tet10 40 x 2 x 2, default test at tolerances 1e-3, 1e-4, 1e-5, 1e-6, and `Backward` at 1e-13, 1e-14: iterations and the four frequencies against the amendment-1 values (each run once; a run that does not converge is recorded as such) | recorded | the spread of converged runs measures the noise of D4 |
| X1 | D3 (extension) | `SubspaceOptions::default().residual_test == Relative`; `ModalOptions::new(6).residual_test == Relative`; T with `Backward` and `max_iterations = 3`: `NotConverged { iterations: 3, tolerance: 1e-12, residuals }` with 10 residuals | as stated | D3 |

**Mutations** (on a `git archive` copy of the final tree in `w49i-work/f457-scratch`, own target directory; each applied alone
by exact text replacement, the named tests run once, the file restored and compared by SHA-256):

| Id | Mutation | Must fail |
| --- | --- | --- |
| MS1 | `Backward` computes the default `||r|| / ||A x||` | C1 (`NotConverged`), L1 |
| MS2 | the `|mu| ||M||` term dropped | L1 (N: 0.45 percent and more) |
| MS3 | `||A||_inf` replaced by `max_i |a_ii|` | L1 (T: 2 against 4) |
| MS4 | `||x||` dropped from the denominator | L1 (N: `||x|| != 1` for `x^T M x = 1`, `M != I`) |
| MS5 | the `Backward` test applied to the first wanted pair only | L2 (pair 10 not converged, eigenvalue error) |
| MS6 | the default of `SubspaceOptions` is `Backward` | D0 |
| MS7 | `modal_analysis` does not pass `residual_test` to the eigensolver | C1 (`NotConverged` at 1e-12) |

**Decision rule.** F-457 is proposed **done** (pending the maintainer's acceptance) if C1, C2, D0, E0, E1, E2, L1 and L2 are
met and every mutation MS1 to MS7 fails a registered test. L3, H1 and X1 are extensions: a miss is a `*_known_miss` record and
a follow-up and does not alone block done. G1 is a record. A miss or an unrun check of the done set, or a mutation caught only
after a post-run amendment (Rule I): **partial**, with the miss named. No converged backward run of the cantilever: **todo**.

## Limits (what is not covered)

- `eta <= tau` certifies a backward error: the pair is exact for a pencil within `tau` of `(A, M)`. The forward error of a low
  eigenvalue is governed by the default-equivalent `rho_i = eta_i F / mu_i` (`F ~ lambda_max`): second order `rho_i^2 /
  relgap` for a Ritz value, first order for the vector. With few wanted modes on a slender model (say m = 2 on the cantilever)
  the loosest wanted pair decides the stop and its eigenvalue error can be of order `(1e-12 F / mu_m)^2` (`~1e-7` to `1e-8`
  there): the backward test is the right test for "as accurate as the arithmetic allows", not a forward-accuracy guarantee.
- The infinity norms overestimate `||.||_2` by at most `sqrt(n)`; no sharper estimate (power steps, Lanczos) is made.
- The default stays the relative test of ADR 0283 (and of `ModalOptions`): a slender model still needs either the backward test
  or a looser tolerance. Changing a default is left to the maintainer (remaining questions).
- No change to the shift rule, the start vectors, the cluster rule or the Sturm check.

## Open points taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. The estimate of `||A||` is the infinity norm of the stored matrix (D2), not power steps (the acceptance's "for example").
2. The backward error includes the `|mu| ||M||` term (D1, the standard pencil form), not `||A|| ||x||` alone; for the low
   modes the two differ by `mu ||M|| / ||A|| <~ 1e-6` on the cantilever and 0.5 percent on N.
3. Reading 1 (the logged amendment-1 frequencies are the reference, copied here) and reading 3 (convergence through
   `modal_analysis`).
4. `ModalOptions` gains the selection (D3), so that a modal user can ask for it; its default is unchanged.
