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

## Addendum 1 (2026-11-15): results

Written after the one run of each registered check. Code and tests: commit e9cb6798 (`eigen.rs`: the constant
`RESTART_DOUBLINGS`, lines 1037 to 1039; `lowest_buckling_factors` now calls the unchanged iteration, moved into
`buckling_iterate`, and makes the D1 restart, lines 1173 to 1284 (with `buckling_default_start`, `buckling_restart_size`,
`BucklingRun`); the branch (i) refusal inside the moved loop hands back `BucklingRun::Short`, lines 1410 to 1442;
`rcfd-linalg/tests/si_buckling_restart_f650.rs`; `rcfd-fem/tests/sa_buckling_restart_f650.rs`). Logs in `w49i-work/f650-scratch`:
`run1_linalg.log`, `run1_fem.log` (the registered run, release), `b2_base.log`, `b2_base.txt`, `b2_branch.txt` (B2 on a `git
archive` copy of cf32422b with the new fem test file copied in, own target directory), `b1_linalg.log`, `b1_fem.log`,
`baseline.log`, `docs_site.log`, `clippy.log`, `mutations.log`, `mut_M*.log`.

### Results against the registered limits

| id | measured | limit | verdict |
|---|---|---|---|
| A1 | `Ok`, three factors `610.7109799174704, 2333.791726231031, 5256.57773727524`, `exhausted None`, 25 iterations (both runs), 112 unknowns | holds | met |
| A2 | against `q = 40` (`610.7109799174692, 2333.791726231034, 5256.577737256932`): `1.8e-15`, `1.2e-15`, `3.5e-12` | `<= 1e-9` each | met |
| A3 | Sturm `{ shift: 7360.265977320068, counted: 3, expected: 3 }`, shift above `lambda_3 = 5256.58` (the midpoint branch of the restart) | holds | met |
| P1 | `Ok`, `1, 2, 10`, each `0` relative; subspace 12; Sturm `{ 5555555.555555556, 3, 3 }`, shift `0` relative to `1/(1e-8 * 18)`; `exhausted None`; 26 iterations | `<= 1e-12`; holds | met |
| P2 | bit-equal to the `subspace_size = Some(12)` solve; 26 iterations against its 11 | bit-equal; greater | met |
| N1 | `SturmMismatch { shift: 1.1111111111111111e5, counted: 1, expected: 0 }`, shift `0` relative | as stated | met |
| B1 | `cargo test -p rcfd-linalg --release`: 123 passed, 0 failed, 6 ignored (20 binaries; L3 refused as before, L4 returned); `cargo test -p rcfd-fem --release`: 1118 passed, 0 failed, 302 ignored (262 result lines); `cargo test -p rcfd-api --release --test baseline_hashes`: 3 passed, 1 ignored; `cargo test -p rcfd-cli --release --test docs_site`: 2 passed | pass | met |
| B2 | base cf32422b and branch: every case hash equal, total `83909e05eefe3b26` on both; cases 1 to 9 also equal the ADR 0559 values (`e5ba1d8f604d723a` ... `4f0cc1d14f7e6bd7`); cases 10 to 13: `2f844c5d0e84a2f9`, `7521122d140d8449`, `7b64f823c6641c39`, `6f4affbbdd28034f` | equal | met |

Record (no limit): 8 elements, `L = 1.5`, `-M_ref`, default options: returned, `612.828208544872, 2367.8108280373926,
5433.573987957625` (60 iterations), against `q = 24`: `2.2e-16`, `2.2e-16`, `6.7e-14`. The restart size `q2` of A1 is not
exposed by `WarpingBucklingResult` and was not printed (the registration did not ask for it).

### Mutations (scratch copy `f650-scratch/mut`, own target directory, one at a time)

| id | mutation | caught | by |
|---|---|---|---|
| M1 | D1 removed | caught | P1 (refused, `counted 3, expected 2`), A1 (refused as in the ADR 0559 record) |
| M2 | `q2 = N+(B)` | caught | P1, A1 (refused: `q2 <= q`) |
| M3 | restart iterations not summed | caught | P2 (11 against 11) |
| M4 | restart also when the count at `S` agreed | caught | B2 (case 4, `K_sigma = +I`, now refused: the mutation passes `mp = 0` exhausted results to the restart path too, so B2 stopped at case 4 before case 10), and the existing A5a of `si_buckling_eigen.rs` |
| M5 | restart also with `mp = 0`, anchored at `1/max|theta|` | caught | N1 and L3 (both return the factor `1.0` with a subspace of 10) |

Deviation in the mutation runs (not in the registered checks): the script ran `si_buckling_eigen` and
`si_buckling_restart_f650` in one cargo call without `--no-fail-fast`, so under M4 and M5 the second binary did not run after the
first failed; it was then run alone for M4 and M5 (`mut_M4_restart.log`, `mut_M5_restart.log`; M5 caught by N1 there, M4 not
caught by the linalg file, as expected). M4 was caught by a different B2 case than the one named (case 4 instead of case 10).

`cargo fmt -p rcfd-linalg --check`, `cargo fmt -p rcfd-fem --check`: clean. `cargo clippy -p rcfd-linalg -p rcfd-fem --release
--all-targets -- -D warnings`: clean.

Not run: the whole workspace suite (the lead's gate). The ignored ADR 0553 records were not run and are not edited.

### Decision by the registered rule

**done**: A1 to A3, P1, P2, N1, B1 and B2 met in the one registered run, no amendment, and the five mutations caught (none
declared equivalent). Clause a1 (three factors returned with the default `BucklingOptions` at `L = 1.5`), a2 (`1.8e-15`, `1.2e-15`,
`3.5e-12` against the subspace of 40, limit `1e-9`) and a3 (B2 hashes over 13 cases equal against cf32422b, B1 suites pass) are
met. Done is the maintainer's call.

### Remaining questions

- **Q1. `mp = 0`** (no positive Ritz value in the subspace while positive factors exist below `S`, e.g. L3). Options: (a) leave it
  refused (the documented remedy is a larger subspace; L3 asserts this refusal); (b) an opt-in restart anchored at
  `1/max|theta|` (M5 shows it finds L3's factor). Recommendation (a); no follow-up proposed unless a model shows the case.
- **Q2. Expose the subspace size on `WarpingBucklingResult` and `BucklingResult`** (the restart changes it). Options: (a) leave;
  (b) add a field (a public struct change, results unchanged). Recommendation (a) for now.

## For the lead: roadmap entries

> F-650 status: done. Note: Structural solver, advanced tier. DONE. ADR 0564, 2026-11-15: rcfd-linalg lowest_buckling_factors restarts once when the subspace holds fewer positive Ritz values than asked (at least one) and the Sturm count at the far bound refuses: the restart size is the number of factors of both signs with |lambda| below a band B = 2R, R the first lambda_mp 2^k holding the asked number of positive factors, both counted by Sylvester's law of inertia (K + s K_sigma at s = B and s = -B); the restart runs the same iteration from the default start and passes the same Sturm check, otherwise the old Sturm mismatch is returned. Only inputs that were refused can change. The ADR 0553 monosymmetric I at L = 1.5 under -M_ref with the default BucklingOptions is returned: 610.71097991747, 2333.79172623103, 5256.57773727524, within 1.8e-15, 1.2e-15, 3.5e-12 of the subspace-40 solve; 8 elements within 6.7e-14 of subspace 24. A diagonal pencil refused before is returned exactly and bit-equal to the solve with the counted subspace; a subspace with no positive Ritz value (the L3 test) is still refused. Results that returned before are bit-identical: a hash over 13 cases (linalg pencils and warping beams, both signs, ADR 0559 paths) equal against cf32422b; rcfd-linalg, rcfd-fem, baseline_hashes and docs_site pass. Five of five mutations caught. Not covered: a subspace with no positive Ritz value at all keeps the refusal; one restart only.
