# ADR 0564: F-650 registration: the buckling eigensolver when the subspace holds fewer positive Ritz values than asked (one restart with a subspace sized by Sturm counts)

Status: **REGISTERED 2026-11-15, written before any code of F-650 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-650 was written, compiled or run: the files listed under "What was read" were read, and
the small arithmetic below was done by hand (disclosed). Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-650 (deps F-642), structural solver, advanced tier, the follow-up named by
[ADR 0559](0559-f642-registration-buckling-eigensolver-on-sign-asymmetric-spectra.md) (its question Q1, recommendation (b)).
Number 0564 given by the lead. Branch `f650-work` from cf32422b, worktree `w49i-work/f650`, scratch `w49i-work/f650-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-650 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. With the default BucklingOptions, the lowest three
> lateral-torsional buckling factors of the ADR 0553 monosymmetric I at L = 1.5 under -M_ref are returned and equal those of a
> subspace of 40 within 1e-9; every result that the solver returned before is bit-identical.

Clauses: **a0** every limit registered here; **a1** with `BucklingOptions::new(3)` (default subspace) the `-M_ref` LTB solve of
the ADR 0553 monosymmetric I (16 elements, `L = 1.5`, axis `x`) returns three factors; **a2** each of the three equals the
factor of the same solve with `subspace_size = Some(40)` within `1e-9` relative; **a3** every result that
`lowest_buckling_factors` returned before is bit-identical (values, vectors, residuals, iterations, subspace size, Sturm record,
`exhausted`): the change may act only on inputs that were refused before.

## What was read

- `crates/rcfd-linalg/src/eigen.rs` at cf32422b: `lowest_buckling_factors` (lines 1131 to 1350), its options and result,
  `buckling_count`, `POSITIVE_CUT = 1e-8`, `NEAR_COUNT_STEPS` (ADR 0559 D1), the default pseudo-random start (`START_SEED`,
  `lcg`, columns generated one after another).
- ADR 0390 D5 and D6 (the buckling eigensolver, the Sturm check by Sylvester's law of inertia, the default options), ADR 0553
  amendments 2 and 3, ADR 0559 in full with its addendum and its record of `L = 1.5`.
- `crates/rcfd-fem/tests/sa_buckling_sign_asymmetric_f642.rs` (model helpers, the B2 hash), its run log
  `w49i-work/f642-scratch/run1_fem.log` (the `L = 1.5` record: default refused with "Sturm count: 32 eigenvalues below
  1.114249423400172e10, but 2 were found"; `q = 40`: `610.7109799174692, 2333.791726231034, 5256.577737256932`).
- `crates/rcfd-fem/src/buckling.rs` and `beam_warping.rs` (the mapping to `BucklingEigenOptions`, `WarpingBucklingResult`).
- The tests that call `lowest_buckling_factors` or match a refusal of it (audit below).

## The refusing path, read from the code (no run)

At convergence `positive` is the number of Ritz values `theta > cut`, the wanted set has `mp = min(count, positive)` members
(plus a cluster). When `mp < count` ("exhausted", branch (i) of ADR 0559) the Sturm count is made at the far bound `S = 1/cut`;
if it equals `mp` the result is returned with `exhausted = Some(S)` (no further factor exists below `S`), otherwise
`SturmMismatch { shift: S, counted, expected: mp }` is returned. On branch (i) `mp == positive` always (the cluster loop needs
`mp < positive`). The subspace of `q` columns converges to the `q` Ritz values of largest `|theta|`, i.e. the `q` factors of
smallest `|lambda|` of both signs. Under `-M_ref` at `L = 1.5` the negative factors (the `+M_ref` factors with the sign changed)
crowd the low end so that the default `q = 11` holds only two positive values, and the count at `S` (32) refuses.

## Design

**D1. One restart on the refusing path of branch (i), with `mp >= 1`, and a subspace sized by Sturm counts.** Only when the
solver is about to return `SturmMismatch` from branch (i) (`mp < count`, the count `c` at `S` differs from `mp`) **and** `mp >=
1`:

1. Anchor `l = lambda_mp = 1/theta_mp` (the largest positive factor found). For `k = 1, 2, ..., 64`: `R = l 2^k`; if `R >= S`
   take `R = S` and stop; otherwise count `N+(R) = #{lambda in (0, R)}` (`buckling_count(R)`) and stop when `N+(R) >= count`.
2. Band `B = min(2 R, S)`. Count `N+(B)` and `N-(B) = #{lambda in (-B, 0)}` (`buckling_count(-B)`; derivation below).
3. `q2 = min(n, N+(B) + N-(B))`. If `q2 <= q` or `q2 <= count` the restart is not made.
4. Restart the whole iteration once with `q2` columns from the **default** pseudo-random start (the first `q` of them are the
   default start of `q` columns; a user start of `q` columns is not reused), every other option unchanged. The restart is the
   same iteration with the same Sturm check (step 5, including ADR 0559 D1), not a second kind of solver.
5. If the restart returns `Ok`, that result is returned, with `iterations` the sum of the iterations of both runs (the field is
   "iterations made") and `subspace_size = q2`. If it returns any error, or a count of steps 1 and 2 is refused, or the restart is
   not made (step 3), the solver returns the **same** `SturmMismatch` as before (shift `S`, the count at `S`, `mp`). There is no
   second restart, no option and no new error variant.

Derivation. (Sylvester's law of inertia, as ADR 0390 D5 step 5.) `K + s K_sigma` is congruent to `I - s C`, `C` the diagonal of
the `theta = 1/lambda` (the `K`-orthonormal eigenbasis of `K^{-1} (-K_sigma)`). A negative pivot appears for each `theta` with
`s theta > 1`. For `s > 0` that is `theta > 1/s`, i.e. `lambda in (0, s)`; for `s = -B < 0` it is `theta < -1/B`, i.e. `lambda
in (-B, 0)`. So `N+(B) + N-(B)` is exactly the number of eigen-directions with `|lambda| < B`, i.e. with `|theta| > 1/B`.
Subspace iteration with `q2` columns converges to the `q2` directions of largest `|theta|` (Bathe and Wilson, *Numerical Methods
in Finite Element Analysis*, 1976, ch. 10, cited from memory; the rate for direction `j` is `|theta_{q2+1}| / |theta_j|`). With
`q2 = N+(B) + N-(B)` these are exactly the directions with `|lambda| < B`, which contain the lowest `count` positive factors (all
below `R` by step 1), and the rate of each wanted one is at most `lambda_count / B < R / (2 R) = 1/2`: to reach a residual
`1e-10` about `log(1e-10)/log(1/2) = 33` iterations, well within the default cap of 200. The factor 2 of the band is this margin;
it is fixed, not tuned. `q2 <= #{|theta| > 1/B}` also keeps `q2` at most the number of non-zero `theta`, so the restart does not
exceed the rank of `K_sigma` (the failure of ADR 0553 amendment 3, `q = 40` at 8 elements) unless `n` caps it first. If step 1
reaches `S` (fewer than `count` positive factors exist below `S`), `B = S` and the restart can return an `exhausted` result that
its own Sturm count at `S` confirms.

Correctness does not rest on the sizing: an `Ok` of the restart has passed the same Sturm check as any result of the solver. The
sizing only decides whether the restart succeeds.

Why `mp >= 1` (and the L3 test). With `mp = 0` the subspace holds no positive Ritz value and there is no positive anchor; the
search of step 1 would have to start from a scale the subspace does not supply. The existing test L3
(`si_buckling_eigen.rs::l3_l4_hidden_factor_is_refused_then_found`, `K = I`, `K_sigma = diag(-1, 100, ..., 900)`, `q = 3`, one
factor asked) is exactly that case: `theta = -900, -800, -700`, `positive = 0`, `mp = 0`, refused with `counted 1, expected 0`.
D1 does not act on it, so L3 keeps its refusal, by the design rule `mp >= 1`, not by an opt-in. (Its pencil does have the
positive factor 1 below `S`; a restart anchored elsewhere would find it, mutation M5 shows that L3 would then fail.) Taken on the
standing instruction: the brief's alternative, an opt-in, would leave the acceptance case (default options) refused.

Why not ADR 0559 Q1 (b) as written (`q2 = min(n, 2 q)`): it is not derived; whether 22 columns hold three positive values at
`L = 1.5` depends on how many negative factors lie below `lambda_3`, which is not known before the run, and a fixed doubling can
exceed the rank of `K_sigma`. The counted band is derived and bounded by the rank. Alternative (c) (a shifted iteration) changes
the iteration of every path unless gated; not taken.

Bit-identity (clause a3): nothing before the refusal of branch (i) changes; on every input that returned `Ok` before, the solver
leaves step 5 through the same branch with the same numbers. The iteration is moved, unchanged, into a private function called
once with the original `q` and start; the restart calls it a second time only on the refusing path. Inputs refused for any other
reason are unchanged. `rcfd-fem` and every other caller is untouched.

Cost: at most 66 extra factorisations of `K + s K_sigma` plus one more iteration, only on inputs that were refused.

## Audit of the tests that assert a refusal (before any code)

- `crates/rcfd-linalg/tests/si_buckling_eigen.rs` L3: branch (i) with `mp = 0`; D1 does not act; keeps `counted 1, expected 0`
  and the same shift. L4 returns as before.
- `crates/rcfd-linalg/tests/si_buckling_sign_asymmetric_f642.rs` N1 (ADR 0559): branch (ii) (`mp = 1 = count`); D1 does not
  act (it needs `mp < count`).
- `crates/rcfd-linalg/tests/sb_eigen.rs` E13: the frequency solver, a different function, unchanged.
- `crates/rcfd-fem/tests/si_prestress_mpc.rs` C13 "BM (no roller rows) buckling" expects a `FemError::Solver`: a mechanism, `K`
  not positive definite (`ShiftNotPositiveDefinite` before any iteration), not the Sturm path; unchanged. Other fem refusals
  matched in tests (`RigidBodyMotionFree`, `InvalidValue`, `FieldLength`, `NotYetSupported`, the prescribed-mechanism pivot
  report) are raised before the eigensolver.
- `sa_beam_warping_f627.rs` `a_n_run1_known_miss` and the ADR 0559 `L = 1.5` record: ignored record and print-only record; not
  edited, not checks.
- `grep -rn SturmMismatch crates` finds no other test. So no existing test breaks by design; all are run (B1).

## Checks (limits fixed now)

| id | check | limit | estimate |
|---|---|---|---|
| A1 (a1) | ADR 0553 monosymmetric I, 16 elements, `L = 1.5`, axis `x`, `-M_ref`, `BucklingOptions::new(3)`: `beam_warping::buckling_factors` returns `Ok`, three factors, `exhausted = None` | holds | holds |
| A2 (a2) | each of A1's three factors against the same solve with `subspace_size = Some(40)` (relative) | `<= 1e-9` each | `1e-12` (converged Ritz values; eigenvalue error quadratic in the residual) |
| A3 | A1's Sturm record: `counted == expected >= 3`, `shift > lambda_3` | holds | holds |
| P1 | linalg, a pencil refused before on branch (i) with `mp = 2`: `K = I` (n = 20), `K_sigma = diag(-1, -0.5, -0.1, 10, 11, ..., 18, 0.01 (eight times))`, three factors, default options (`q = 11`: 9 negative and 2 positive Ritz values; three positive factors `1, 2, 10` below `S`). Hand derivation: `R = 4, 8` count 2, `R = 16` count 3, `B = 32`, `N+ = 3`, `N- = 9`, `q2 = 12`. `Ok`, factors against `1, 2, 10`; `subspace_size == 12`; Sturm record `{ S, 3, 3 }` with `S = 1/(1e-8 * 18)`; `exhausted = None` | `<= 1e-12` relative each; holds; `S` within `1e-12` relative | `1e-15` |
| P2 | P1's values, vectors, residuals, Sturm record and `exhausted` bit-equal to the solve of the same pencil with `subspace_size = Some(12)` (default start); P1's `iterations` strictly greater than that solve's | bit-equal; holds | holds (the restart is that solve) |
| N1 | negative control, L3's pencil (`q = 3`, one factor, `mp = 0`): still `SturmMismatch`, `counted 1, expected 0`, shift `1/(1e-8 * 900)` | as stated; shift `<= 1e-12` relative | as stated |
| B1 (a3) | the existing tests pass unchanged: `cargo test -p rcfd-linalg --release` (all, including L3, L4, the ADR 0559 P1 and N1), `cargo test -p rcfd-fem --release` (all), `cargo test -p rcfd-api --release --test baseline_hashes`, `cargo test -p rcfd-cli --release --test docs_site` | pass | pass |
| B2 (a3) | an FNV-1a 64 hash (the ADR 0559 B2 method: bits of every value, vector entry, residual, iteration count, subspace size, Sturm record, `exhausted`) of the results of a fixed list of inputs that returned before, computed by the same new test file on a `git archive` copy of cf32422b (the solver before F-650, with ADR 0559 D1) and on the branch | equal, every case | equal |

B2's list: cases 1 to 9 exactly as ADR 0559 B2; (10) `K = I` (n = 20), `K_sigma = diag(-1, -0.5, 10, 11, ..., 18, 0.01 (nine
times))`, three factors, default `q = 11`: two positive factors only, branch (i) accepted at `S` with `exhausted = Some(S)`, the
path next to D1; (11) the ADR 0553 I, 16 elements, `L = 3`, `-M_ref`, default (returned by ADR 0559 D1); (12) the same at `L = 6`;
(13) the ADR 0559 P1 pencil (`K_sigma = diag(-1, -0.9, -0.8, 10, ..., 17, -0.5, ..., -0.1)`, default, ADR 0559 D1 path).

Records, printed, no limit: A1's iteration count and Sturm record; the `q = 40` factors; the ADR 0553 I at 8 elements, `L = 1.5`,
`-M_ref`, default options (returned and its difference to `subspace_size = Some(24)`, or the refusal).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| M1 | D1 removed (the old refusal) | A1, P1 |
| M2 | `q2 = N+(B)` (the negative factors in the band left out) | P1 (`q2 = 3 <= q`: refused), A1 |
| M3 | `iterations` of the restart alone (not summed) | P2 |
| M4 | the restart also when the count at `S` agreed (exhausted results) | B2 (case 10) |
| M5 | the restart also with `mp = 0`, anchored at `1/max|theta|` | N1 and L3 (L3 returns the factor 1) |

None is declared equivalent.

## Decision rule

**done**: A1 to A3, P1, P2, N1, B1, B2 met in the one run and every mutation caught. **partial**: any registered check missed (a
`*_known_miss` record), any mutation not caught, or any check met only after a post-run amendment. **todo**: A1, A2 or B2 missed.

## Limits (not covered)

- `mp = 0` (no positive Ritz value in the subspace) keeps the refusal (L3).
- One restart only: if the counted band is wrong in a way the restart cannot overcome (a factorisation refused at a count, the
  iteration cap, a cluster exceeding `q2`) the old refusal is returned.
- `q2` can be as large as `n` on a spectrum crowded with small negative factors; the cost is then that of a full-width subspace
  (only on inputs that were refused).
- A user start of `q` columns is not reused by the restart (the restart uses the default start of `q2` columns).
- The frequency solver (`lowest_eigenpairs`) is not touched.

## Open questions

None that changes a default result.
