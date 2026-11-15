# ADR 0559: F-642 registration: the buckling eigensolver on sign-asymmetric spectra (a Sturm count just above the wanted set when the subspace holds no further positive Ritz value)

Status: **REGISTERED 2026-11-15, written before any code of F-642 and before any run of its checks.** Not yet accepted by the
maintainer. Before this text nothing of F-642 was written, compiled or run: the files listed under "What was read" were read, and
the small arithmetic below was done by hand (disclosed). Results go in the addenda at the end; nothing above the addenda is
changed after a run.

Roadmap item F-642 (deps F-627), structural solver, advanced tier, the follow-up named by
[ADR 0553](0553-f627-registration-monosymmetric-thin-walled-sections-for-the-warping-element.md) (amendment 2, its question
Q3). Number 0559 given by the lead. Branch `f642-work` from 3378a508, worktree `w49i-work/f642`, scratch `w49i-work/f642-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number recorded); each registered run made once; defaults
bit-identical; no edit of `baseline_hashes.rs`, of the case and mesh hash tables or of any golden file; no existing test edited;
no new dependency, no unsafe code, the crate graph unchanged; clean-room. Every choice below that the maintainer would normally
be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## What F-642 asks

The acceptance, binding, verbatim:

> Every limit not stated here is registered in the ADR before the run. With the default BucklingOptions, the lateral-torsional
> buckling factor of the ADR 0553 monosymmetric I under -M_ref is returned (not refused) and equals the factor of the ADR 0553
> amended run within 1e-9; every result that the solver returned before is bit-identical.

Clauses: **a0** every limit registered here; **a1** with `BucklingOptions::new(3)` (default subspace) the `-M_ref` LTB solve of
the ADR 0553 monosymmetric I (16 elements, `L = 3`, axis `x`) returns a result; **a2** its lowest factor equals that of the ADR
0553 amended run (the same model with `subspace_size = Some(40)`, check A1') within `1e-9` relative; **a3** every result that
`lowest_buckling_factors` returned before is bit-identical (values, vectors, residuals, iterations, subspace size, Sturm record,
`exhausted`): the change may act only on inputs that were refused before.

## What was read

- `crates/rcfd-linalg/src/eigen.rs`: `lowest_buckling_factors` (lines 1120 to 1330 at 3378a508), its options, result, the
  Sturm count `buckling_count`, `POSITIVE_CUT = 1e-8`, `CLUSTER_GAP = 1e-6`.
- ADR 0390 D5 (the buckling eigensolver, its Sturm check by Sylvester's law of inertia) and D6 (`BucklingOptions::new`: backward
  residual test, tolerance `1e-10`, subspace `None`).
- ADR 0553 amendments 2 and 3 and addendum 1; `crates/rcfd-fem/tests/sa_beam_warping_f627.rs` (the model helpers, the run-1 and
  run-2 records `a_n_run1_known_miss`, `a_n_amended_run2_known_miss`, and `a_n_amended3_remaining`).
- `crates/rcfd-fem/src/buckling.rs` (`BucklingOptions`, the mapping to `BucklingEigenOptions`).
- The tests that call `lowest_buckling_factors` or match its `SturmMismatch` (audit below).

## The refusal, read from the code (no run)

At convergence (step 4 of D5), `positive` is the number of Ritz values `theta > cut`, the wanted set has `mp` members. Step 5
chooses the Sturm shift:

- if `mp >= count` and `mp < positive` (a further positive Ritz value exists in the subspace): three shifts between `lambda_mp`
  and `lambda_{mp+1}` (fractions 0.5, 0.25, 0.75);
- otherwise the far bound `S = 1/cut`.

The second branch is entered in two situations. (i) `mp < count` (fewer positive Ritz values than asked: "exhausted"). (ii) `mp
>= count` and `mp == positive`: the subspace holds exactly the wanted positive values and nothing positive beyond them. In (ii)
the count at `S` equals `mp` only if the model has no further positive factor below `S`. For the ADR 0553 `-M_ref` case the
default subspace of `q = 11` held three positive and eight negative Ritz values (the negative factors are smaller in magnitude, so
their `|theta|` dominate), the model has 32 positive factors below `S`, and the solver refused: `SturmMismatch { shift:
5.397979e9, counted: 32, expected: 3 }`.

The three found values are converged Ritz pairs. Nothing is wrong with them; the check point is too far away. A count at a shift
just above `lambda_mp` answers the question that the check asks ("is any factor below the last wanted one missing?") without
knowing `lambda_{mp+1}`.

## Design

**D1. A near count on the refusing path only.** In step 5, when the branch is (ii) (`mp >= count`, `mp == positive`) and the
count at `S` differs from `mp` (the path that returns `SturmMismatch` today), the solver counts again at

`s_k = lambda_mp (1 + delta_k)`, `delta = (1e-2, 1e-3, 1e-4)`, in this order,

and accepts the first `s_k` whose count (an `Ok` of `buckling_count`) equals `mp`. The result is then built exactly as on the
accepting path today, from the same converged iterate (values, vectors, residuals, iterations of that iteration), with the Sturm
record `{ shift: s_k, counted: mp, expected: mp }` and `exhausted: None`. If no `s_k` gives `mp` (a count differs or a
factorisation is refused at every `s_k`), the solver returns the **same** `SturmMismatch` as today (the shift `S`, the count at
`S`). No option, no new error variant, no change of any other branch.

Derivation (Sylvester's law of inertia, as ADR 0390 D5 step 5): `K + s K_sigma` is congruent to `I - s C` with `C` the matrix
of the `theta`, so its number of negative pivots is `#{lambda_j in (0, s)}`. The `mp` returned values are distinct converged
Ritz values of the symmetric-definite pencil `(A, K)` with `K`-orthogonal vectors, so each lies within its residual bound of a
distinct eigenvalue below `s_k` (the residuals are at most `1e-10` relative or backward, far below `delta_k >= 1e-4`). A count of
exactly `mp` at `s_k > lambda_mp` then says that no other factor lies in `(0, s_k)`: the `mp` values are the lowest `mp` factors.
This is the same rigor as the midpoint count of branch (i) of the existing code: the check is stated in terms of `s` only.
Why the three `delta`: `1e-2` first, so that the factorisation of `K + s K_sigma` is not near-singular (a pivot near zero makes
an unpivoted `LDL^T` inaccurate); `1e-3` and `1e-4` if a factor lies between `lambda_mp` and `lambda_mp (1 + 1e-2)` (a count of
`mp + 1` there is not a miss but a near factor outside the subspace; a smaller `delta` may still separate it), or if a
factorisation is refused. A factor closer than `1e-4 lambda_mp` keeps the refusal (the subspace should be larger), as today.

Why only branch (ii): in branch (i) (`mp < count`) the result carries `exhausted = Some(S)`, the claim that no factor at all
exists below `S`; a near count cannot support that claim, so (i) is unchanged (and with `mp = 0` there is no `lambda_mp`).

Bit-identity (clause a3): every input on which the solver returned `Ok` before reaches step 5 with the same state (nothing
before step 5 changes) and leaves it through the same branch with the same numbers: on branch (ii) the new code runs only after
the count at `S` differed from `mp`, which was a refusal. Inputs that were refused for any other reason (not converged, rank,
cluster, a refused factorisation, the midpoint branch) are unchanged too. So the change can only turn some refusals of branch
(ii) into results. `rcfd-fem` and every other caller is untouched.

Cost: at most three extra factorisations of `K + s K_sigma`, only on inputs that were refused.

Alternatives considered: (b1) restart with a larger subspace: changes nothing that returned before, but the rank of `K_sigma`
bounds the useful `q` (ADR 0553 amendment 3: `q = 40` lost rank at 8 elements), and the iteration count grows; (b2) a spectral
shift of the iteration (`(K + sigma K_sigma)^{-1}`) to favour the positive side: a larger change of the iteration, the result of
every path would change unless gated. D1 needs neither and is the smallest change that is rigorous; taken on the standing
instruction (recommendation of ADR 0553 Q3 (b), "iterate on or enlarge", is met in its intent: no refusal when the found set is
verifiably complete).

## Audit of the tests that assert the refusal (before any code)

- `crates/rcfd-linalg/tests/si_buckling_eigen.rs` `l3_l4_hidden_factor_is_refused_then_found` (L3): `K = I`, `K_sigma =
  diag(-1, 100, 200, ..., 900)`, `q = 3`, one factor asked. The subspace converges to `theta = -900, -800, -700`, so `positive =
  0`, `mp = 0 < count`: branch (i), unchanged by D1. L3 keeps its refusal (`counted 1, expected 0`). L4 returns as before.
- `crates/rcfd-linalg/tests/sb_eigen.rs` `e13_sturm_detects_a_missed_mode` (E13): the `SturmMismatch` of the frequency solver
  (`eigen.rs` line 996, a different function), unchanged.
- `crates/rcfd-fem/tests/sa_beam_warping_f627.rs` `a_n_run1_known_miss` (ignored record): it expected a result and was refused;
  after D1 it would pass if run. It is an ignored record and is not edited or run as a check.
- No other test matches `SturmMismatch` (`grep -rn SturmMismatch crates`). So no existing test breaks by design; all are run (B1).

## Checks (limits fixed now)

| id | check | limit | estimate |
|---|---|---|---|
| A1 (a1) | ADR 0553 monosymmetric I, 16 elements, `L = 3`, axis `x`, `-M_ref`, `BucklingOptions::new(3)`: `beam_warping::buckling_factors` returns `Ok` with three factors | holds | holds |
| A2 (a2) | A1's lowest factor against the same solve with `subspace_size = Some(40)` (the ADR 0553 amended run, A1'): relative difference | `<= 1e-9` | `1e-13` (both converged Ritz values; eigenvalue error quadratic in the residual) |
| A3 | A1's Sturm record: `counted == expected == 3`, shift in `(lambda_3, 1.01 lambda_3]`; and A1's second and third factors against the amended run | holds; `<= 1e-9` relative | holds; `1e-12` |
| A4 | the same at 8 elements (default options) against the ADR 0553 amendment 3 set-up (`subspace_size = Some(24)`), lowest factor | `<= 1e-9` relative | `1e-13` |
| P1 | linalg, a refused-before diagonal pencil: `K = I` (n = 20), `K_sigma = diag(-1, -0.9, -0.8, 10, 11, ..., 17, -0.5, -0.45, ..., -0.1)`, three factors, default options (`q = 11`: three positive and eight negative Ritz values; twelve positive factors below `S`): `Ok`, factors against `1, 1/0.9, 1/0.8` | `<= 1e-12` relative each; Sturm record `3, 3`, shift in `(1.25, 1.2625]` | `1e-15` |
| N1 | negative control: `K = I` (n = 6), `K_sigma = diag(-2, -1, 300, 400, 1e-7, 1e-7)`, one factor, `q = 3`, start vectors with a zero first entry (the factor `0.5` is invisible): the result is still `SturmMismatch` with `counted 2, expected 1` and the shift `S = 1/(1e-8 * 400)` | as stated; shift `<= 1e-12` relative | as stated |
| B1 (a3) | the existing tests pass unchanged: `cargo test -p rcfd-linalg --release` (all, including L3, L4, the bit records), `cargo test -p rcfd-fem --release` (all, including `sa_sparse_bits`, `sa_f595_bits`, `si_buckling_*`, the ADR 0546, 0548, 0553 records), `cargo test -p rcfd-api --release --test baseline_hashes` | pass | pass |
| B2 (a3) | a hash (FNV-1a 64 over the bits of every value, vector entry, residual, the iteration count, the subspace size, the Sturm record and `exhausted`) of the results of a fixed list of inputs that returned before, computed by the same new test file on a `git archive` copy of 3378a508 (the old solver) and on the branch | equal | equal |

B2's list: (1) the tridiagonal second difference `n = 200` with `K_sigma = -I`, ten factors, default (ADR 0390 A4 set-up);
(2) the mixed-sign pencil of L2 (`q = 30`, five factors); (3) L4 (`q = 10`); (4) `K_sigma = +I`, three factors (exhausted, no
factor); (5) `K = I` (n = 20), `K_sigma = diag(-30, -29, -28, 1, 2, ..., 17)`, three factors, default `q = 11` (three positive
Ritz values and no further positive factor: branch (ii) accepting at `S`, the path D1 sits on); (6) the ADR 0553 monosymmetric
I at 16 elements, `+M_ref`, default options; (7) the same, `-M_ref`, `subspace_size = Some(40)`; (8) 8 elements, both signs,
`subspace_size = Some(24)`; (9) the doubly symmetric I of ADR 0553 D3 (`doubly_symmetric_i(0.1, 0.01, 0.18, 0.006)`, 16 elements, `L = 3`),
`+M_ref`, default options. The fem cases hash the `WarpingBucklingResult` factors, modes, residuals, iterations and Sturm record.

Records, printed, no limit (the outcome is reported whichever it is): the ADR 0553 `L = 1.5` and `L = 6` models under `-M_ref`
with the default options (returned and its difference to the amended `q = 40` solve, or the refusal).

## Planned mutations (on a scratch copy, one at a time, own `CARGO_TARGET_DIR`)

| id | mutation | expected caught by |
|---|---|---|
| M1 | D1 removed (the old refusal) | A1, P1 |
| M2 | D1 accepts the first `Ok` count without comparing it with `mp` | N1 |
| M3 | the near shift below the wanted set (`lambda_mp (1 - delta_k)`) | A1, P1 (the count is `mp - 1`) |
| M4 | D1 applied on branch (ii) before the count at `S` (always the near shift) | B2 (case 5: the Sturm shift changes) |

## Decision rule

**done**: A1 to A4, P1, N1, B1, B2 met in the one run and every mutation caught. **partial**: any registered check missed (a
`*_known_miss` record), any mutation not caught (none is declared equivalent), or any check met only after a post-run amendment.
**todo**: A1, A2 or B2 missed.

## Limits (not covered)

- Branch (i) (fewer positive Ritz values than asked) still refuses when the count at `S` differs: a load case whose negative
  factors dominate so strongly that the subspace holds fewer positive values than asked is not helped (that needs a larger
  subspace or a shifted iteration).
- A factor closer than `1e-4 lambda_mp` above the wanted set, outside the subspace, keeps the refusal.
- The frequency solver (`lowest_eigenpairs`) and its Sturm check are not touched.
- No new option: the near count is always on, but only on inputs that were refused.

## Open questions

None that changes a default result. The follow-up for branch (i) is proposed in the addendum if the records show it matters.
