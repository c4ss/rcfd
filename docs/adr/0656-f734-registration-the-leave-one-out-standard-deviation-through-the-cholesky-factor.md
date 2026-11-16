# ADR 0656: F-734 registration: the leave-one-out standard deviation of R3 through the Cholesky factor

Status: **REGISTERED 2026-11-16, written before any code of F-734 and before any run of its checks.** Written by the
implementing agent on branch `f734-work` (from main 9a6d2f69, which holds F-718 / ADR 0652), worktree `w49i-work/f734`, scratch
`w49i-work/f734-scratch`. Not yet accepted by the maintainer. Every choice below that the maintainer would normally be asked
about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation) and is marked "(standing
instruction)".

Roadmap item F-734 (deps F-718), the follow-up proposed by ADR 0652 (addendum 1, remaining question 3). The binding acceptance:

> the predictive standard deviation of R3 is evaluated without the cancellation of a^T C a, it equals brute force within 1e-8 of the residual scale on the noise-free fits of ADR 0652 (N1a and the driver's final fit), and every ADR 0652 and ADR 0622 check keeps its result.

House rules: clean-room (first principles; the derivation is below); library only, opt-in (R3 stays behind `TrustRule::r3`,
default `None`); defaults bit-identical; no new dependency, no unsafe code. F-716 edits `design/bayes.rs` and
`design/fan_objective.rs` on another branch; this item edits neither, nor `surrogate.rs`. The library change is confined to the
standard-deviation lines of `r3_core` in `crates/rcfd-api/src/design/fidelity.rs` (and the doc comment of `R3Loo::sd`); the checks
go in a new `crates/rcfd-api/tests/f734_r3_sd.rs`. `f718_fidelity_r3.rs` and its ignored records are not edited.

## What was read

`AGENT_RULES.md`; the brief; ADR 0652 (D1 to D5, set-ups, checks, amendments 1 and 2, addendum 1, remaining question 3);
`fidelity.rs` (`lstsq`, `r3_cov`, `chol_inverse`, `r3_core`, `fit_metric`'s coverage, `R3Fit`'s JSON); `surrogate.rs`
(`cholesky`, `solve_lower`, `solve_upper_t`: row-major lower factor); `f718_fidelity_r3.rs` (`brute_e`, `loo_check`, `n_case`,
`nd_corrected2`). No code was run and no number was computed for this registration; the magnitudes below are ADR 0652's
measured numbers and the estimates derived from them.

## The cause, restated

ADR 0652 D3 computes `sd_i = sqrt(a_i^T C a_i)` with `a_i = ((G M)_i,: + ((G P)_ii / (1 - h_ii)) M_i,:) / G_ii`, `G = C^-1`.
On a noise-free fit the correlation matrix has condition about 2e9 (jitter 1e-8), the entries of `G` and so of `a_i` are large,
and `a_i^T C a_i` (smallest about 5e-7 in scaled units, sd 7.0e-4 of rms(r)) is the small difference of terms of order
`|a_i|^2 |C|`. ADR 0652 amendment 1 measured a loss of about 1e-4 of the sd's relative accuracy in both the closed form and the
test's brute force (which also forms `a_i` explicitly and evaluates `a_i^T C a_i` in double). Evaluating `|L^T a_i|^2` (ADR 0652's
option (b)) does not remove the loss: the cancellation moves into the product `L^T a_i`, whose entries are small differences of
large ones. The remedy has to avoid forming the large vector `a_i` at all.

## Design (fixed now)

### D1. A decomposition of the leave-one-out error into two independent parts

Notation as ADR 0652: `X` (n x p, rows `x_j = [yL_j, 1, u_j]`), `z ~ N(0, C)` the process plus noise, `y = X beta + z`; for pair
`i` the others are indexed `-i`, `k = C_(-i),i` (the off-diagonal covariance, no nugget), `w = C_(-i)^-1 k` (the zero-mean kriging
weights), `Q = (X_-^T X_-)^-1 X_-^T` (the refit of R2 on the others), `M_- = I - X_- Q`.

The brute-force leave-one-out residual is `e3_i = y_i - x_i^T Q y_- - w^T M_- y_-`. Since `M_- X_- = 0` and `x_i^T beta - x_i^T
Q X_- beta = 0`, `beta` drops out: `e3_i = z_i - c^T z_-` with `c = Q^T x_i + M_-^T w`. With `M_-^T = I - Q^T X_-^T`,
`c = w - Q^T g`, where

  `g = X_-^T w - x_i` (p-vector: how far the kriging weights are from reproducing the regressors of pair `i`).

Hence `e3_i = eps_i + g^T Q z_-` with `eps_i = z_i - w^T z_-`. And `cov(eps_i, z_-) = k^T - w^T C_- = 0`, so the two parts are
uncorrelated (independent, being jointly Gaussian), and

  `var(e3_i) = (C_ii - k^T C_-^-1 k) + t^T C_- t`, with `t = Q^T g` (an (n-1)-vector).

Both terms are non-negative quantities computed without a difference of large numbers:

* **The conditional variance.** By the block inverse (Schur complement), `C_ii - k^T C_-^-1 k = 1 / G_ii`, and with the
  Cholesky factor `C = L L^T`, `G_ii = |L^-1 e_i|^2`, a sum of squares of one forward substitution (`solve_lower`). This is the
  "solve `L v = k`" form of Rasmussen and Williams (2006, section 2.2, from memory; derived here), applied to the full factor.
* **The refit part.** Pad `t` with `t_i = 0` to an n-vector `q`; then `t^T C_- t = q^T C q = |L^T q|^2`. `q` lies in the column
  space of `X_-` (smooth regressors, entries of order the OLS weights), so `L^T q` has no cancellation of large terms.

`g` from the full inverse: the kriging weights are `w_j = -G_ji / G_ii` (ADR 0652 D3), so `g = -(X^T G e_i) / G_ii`.

`t` from the full design: with `H = (X^T X)^-1`, the deletion formula (Sherman-Morrison, as ADR 0652 D3) gives
`(X_-^T X_-)^-1 = H + H x_i x_i^T H / (1 - h_ii)`, so for `j != i`, `q_j = x_j^T H g + P_ji (x_i^T H g) / (1 - h_ii)` and `q_i = 0`.
`H g` from the existing least squares: the coefficients of regressing `e_k` on `X` are `b_k = H x_k` (already computed, and
discarded, in `r3_core`'s hat-matrix loop), so `B = [b_1 .. b_n] = H X^T` and `B B^T = H X^T X H = H`; hence `H g = sum_k b_k
(b_k . g)`.

Result: `sd_i = s sqrt(1 / |L^-1 e_i|^2 + |L^T q^(i)|^2)` (scaled units, mapped back by `s`, as ADR 0652 D3). Check: with no
process (`G = I / sigma^2`), `w = 0`, `g = -x_i`, `t^T C_- t = sigma^2 x_i^T H_- x_i`, so `var = sigma^2 (1 + x_i^T H_- x_i) =
sigma^2 / (1 - h_ii)`, the variance of the OLS deleted residual. Cost per pair `O(n^2)`, against `O(n^3)` for the old `a_i^T C a_i`.

### D2. What changes and what does not

* `R3Loo::e`, `in_sample`, `r`, `s`, `l`, `sf2`, `beta`, `loglik`, `r2_unit`: the code that computes them is not changed, so they
  are bit-identical to ADR 0652's.
* `R3Loo::sd` changes (more accurate); it enters only R3's coverage (`|e3_i| <= 2 sd_i`, a count). R3's published numbers (the
  `r3` object of a fit: `l`, `sf2`, `jitter`, `beta`, `mse`, `se`, `loglik`, and `coverage`) change only if a pair crosses the
  2-sigma boundary; the run reports the coverage of N1a, N1b and ND final against ADR 0652's 1.000, 0.950, 1.000.
* Nothing that does not ask for R3 reaches `r3_core` (`fit_metric` calls it only when `TrustRule::r3` is `Some`), so every
  default output is bit-identical by construction; the existing tests of ADR 0622 confirm it (check K3).

## Set-ups (fixed now)

ADR 0652's, unchanged: **N1a** (`latin_hypercube(40, 2, 718_011)`, no noise), **N1b** (seed 718_012, noise 0.02, stated 0.02, normals
from 718_013), **N2** (seed 718_021, unstructured, noise 0.05, normals from 718_022) and **ND final** (the driver of `nd_spec(1)`:
300 LHS candidates seed 718_041, 6 rounds of 6, `r3: Some(R3Rule::default())`; the 36 pairs of the final round with stated
uncertainty 0). Hyperparameters: each fit's own `R3Fit::l` and `sf2` (from `Correction::fit` for N1a, N1b, N2; from the driver's
final correction for ND), passed to `r3_loo(.., Some((l, sf2)))`.

## Brute force (the reference), fixed now

The reference must not share the cancellation, so it is ADR 0652's brute force evaluated in **double-double arithmetic** (about
32 significant digits): for pair `i`, R2 refitted by normal equations on the other `n - 1` pairs and the zero-mean process with the
same `(l, s_f^2)` and stated uncertainties solved by Gauss-Jordan with partial pivoting, both in double-double, applied to the `n`
unit vectors to give `a_i` (the registered definition of L3s), then `a_i^T C a_i` in double-double, then `sd_brute_i = sqrt` of its
rounded value. The inputs (`X`, the entries of `C` in output units, built by the test's own squared exponential as in
`f718_fidelity_r3.rs`) are double numbers taken as exact. Double-double from first principles (Dekker 1971; Knuth's two-sum, from
memory, derived here): `two_sum(a, b)`: `s = a + b`, `b' = s - a`, `err = (a - (s - b')) + (b - b')`, exact; `two_prod(a, b)`:
`p = a b`, `err = fma(a, b, -p)` (`f64::mul_add`), exact; addition and multiplication of pairs by these with a final
`quick_two_sum`; division by one Newton correction `q1 = x.hi / y.hi`, `r = x - q1 y`, `q2 = r.hi / y.hi`. Relative rounding about
`1e-32`; the cancellation of `a_i^T C a_i` (about `1e-12` of the terms' size, from the 1e-4 loss at `eps = 2.2e-16`) leaves about
`1e-20` relative in the variance, far below the limit.

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f734_r3_sd.rs`, release profile, every number printed before any assertion, except K3, R, F.

| Id | Clause | What | Limit (estimate) |
|---|---|---|---|
| V1 | accuracy, N1a | `max_i abs(sd_i - sd_brute_i)` with `sd_i` from `r3_loo` | `<= 1e-8 rms(r)` (acceptance; estimate below 1e-9: the conditional variance agreed to 2.0e-10 between two methods in ADR 0652 amendment 1, the refit part has no cancellation) |
| V2 | accuracy, ND final | the same | `<= 1e-8 rms(r)` (acceptance) |
| V3 | — | the same on N1b and N2 | `<= 1e-8 rms(r)` (ADR 0652 measured 1.3e-14, 1.1e-15 there) |
| B0 | reference sanity | the brute force's `a_i . y` (double-double, rounded) against `r3_loo`'s `e_i`, all four fits | `<= 1e-8 rms(r)` (ADR 0652 L3: 2.5e-10 to 4e-14) |
| B1 | reference sanity | `sd_brute` against ADR 0652's double brute force (`a_i^T C a_i` in double, the test's `loo_check` arithmetic) on N1b and N2 | `<= 1e-12 rms(r)` (no cancellation with noise; ADR 0652 measured 1.3e-14 and 1.1e-15 between double forms) |
| E1 | bit-identity of R3's other outputs | `git diff 9a6d2f69 -- crates/rcfd-api/src/design/fidelity.rs` touches only the sd lines of `r3_core`, the hat-matrix loop's binding of the coefficients it already computes, and the doc comment of `R3Loo::sd` | inspection: as stated |
| K-0652 | every ADR 0652 check | `cargo test -p rcfd-api --release --test f718_fidelity_r3` (the 8 non-ignored tests; the 4 ignored records not run) | 8 pass (as in ADR 0652 addendum 1), coverage printed (N1a, N1b, ND) |
| K3 | every ADR 0622 check | `--test f699_fidelity` (16 fast tests) and its heavy record once (`--ignored --exact b_reduced_order_fan_heavy`) | pass, as in ADR 0622 and ADR 0652 |
| R | — | `cargo test -p rcfd-api --release --lib`; `cargo test -p rcfd-cli --release --test docs_site` (docs touched) | pass |
| F | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |
| T | — | the new test's release time | under 120 s (else `#[ignore]`d as heavy with a fast test; estimate: one driver run, about half of `nd_corrected2`, plus `4 x 40` double-double solves of size 39 with 40 right-hand sides, about 1e8 flops) |

## Negative controls and mutations (registered now)

**NC-S (diagnostic control).** The double-precision quadratic form `a_i^T C a_i` of the brute force's `a_i` rounded to double,
against the double-double value, on N1a. Expected to exceed `1e-8 rms(r)` (the ADR 0652 diagnosis: about 1e-4 of an sd of 7e-4 of
rms(r), i.e. about 7e-8). It shows that the reference resolves the cancellation the item removes. If it does not exceed the limit,
that is reported (the ADR 0652 misses are then not explained by the double quadratic form alone) and the decision is unaffected.

Mutations (scratch copy from `git archive` of the code commit, one at a time, the new test file; expected to fail):

| Id | Mutation of `r3_core` | Expected to fail |
|---|---|---|
| MS1 | the ADR 0652 evaluation `sqrt(a_i^T C a_i)` restored | V1 or V2 |
| MS2 | conditional variance only (`var = 1 / G_ii`) | V1, V2, V3 |
| MS3 | deletion correction dropped (`q_j = x_j^T H g` for `j != i`) | at least one of V1 to V3 |
| MS4 | `q_i` not zeroed (`q_i = x_i^T H g / (1 - h_ii)`) | at least one of V1 to V3 |
| MS5 | the wrong triangle (`L q` in place of `L^T q`) | at least one of V1 to V3 |

## Decision rule

* **done:** V1, V2 (the acceptance's accuracy clause, with D1 the evaluation without the cancellation), V3, B0, B1, E1, K-0652,
  K3, R, F, T met; every mutation MS1 to MS5 caught. NC-S is reported, not decided on.
* **partial:** V1, V2, K-0652 and K3 met, and a non-acceptance check (V3, B0, B1, T) missed (an ignored `*_known_miss` record
  with the limit intact) or a mutation survives.
* **todo** (the lead does not take the code): V1 or V2 missed, or an ADR 0652 or ADR 0622 check changes its result.

## Limits (not covered)

* The sd is the variance under the model at fixed hyperparameters (ADR 0652 D3); its calibration is ADR 0652's C1, not rechecked
  here beyond K-0652.
* The reference treats the double entries of `C` and `X` as exact; the sensitivity of the variance to rounding of the inputs
  (about `eps |C| |(-w, 1)|^2`, ADR 0652 amendment 1 measured 2e-10 of rms(r) between two conditional-variance methods) is the
  same for the closed form and is not a cancellation of the evaluation.
* Only the leave-one-out sd is changed; `predict` still returns values, not variances (ADR 0652 limits).
* Synthetic fits only, two parameters, `n` up to 40.

## References

* C. E. Rasmussen and C. K. I. Williams (2006), Gaussian Processes for Machine Learning, MIT Press, section 2.2 (Cholesky form of the
  predictive variance) and section 5.4.2 (leave-one-out identities), from memory; every formula used is derived above.
* T. J. Dekker (1971), A floating-point technique for extending the available precision, Numerische Mathematik 18, 224-242 (from
  memory; the double-double arithmetic, derived above).
* D. E. Knuth, The Art of Computer Programming, vol. 2, section 4.2.2 (from memory; the error-free two-sum).
* D. A. Belsley, E. Kuh and R. E. Welsch (1980), Regression Diagnostics (from memory; the deletion formula, as ADR 0652).

## Addenda

Results, amendments and the roadmap entries go here; nothing above is changed after a run.

## Addendum 1: results (2026-11-16)

Code: cc8a0f43 (the sd of D1 in `r3_core`, the new test `f734_r3_sd.rs`); registration aa00d354 before it. Library change only in
`crates/rcfd-api/src/design/fidelity.rs`: the doc comment of `R3Loo::sd` (line 1101), the hat-matrix loop of `r3_core` keeps the
coefficients it already computed (lines 1174 to 1186), and the sd lines (1192 to 1222) replace the `a_i^T C a_i` evaluation.
Docs: `docs/design-loop.md` (one sentence in the R3 bullet). No shared solver file, golden, hash table, case key or MCP tool;
`bayes.rs`, `fan_objective.rs`, `surrogate.rs` and `f718_fidelity_r3.rs` untouched.

Runs (release profile throughout), each once: the registered run `f734-scratch/run1.log` (0.5 s); K-0652 and K3 fast
`run_k.log`; the heavy record, `--lib` and docs_site `run_r.log`; mutations `mut.log` (script `mutate.py`, runner `mut_run.sh`,
copy `mcopy` from `git archive cc8a0f43`; the unmutated copy passed with the same numbers).

| Check | Limit | Measured | Met |
|---|---|---|---|
| V1 N1a sd vs double-double brute force | `<= 1e-8 rms(r)` | 1.33e-10 (smallest sd 6.97e-4 of rms(r)) | yes |
| V2 ND final (36 pairs) | `<= 1e-8 rms(r)` | 9.22e-10 (smallest sd 2.54e-4 of rms(r)) | yes |
| V3 N1b, N2 | `<= 1e-8 rms(r)` | 1.6e-15, 3.0e-16 | yes |
| B0 reference `a_i . y` vs `e_i` | `<= 1e-8 rms(r)` | N1a 1.4e-10, N1b 1.1e-14, N2 2.8e-14, ND 1.1e-9 | yes |
| B1 double-double vs double `a^T C a` (N1b, N2) | `<= 1e-12 rms(r)` | 2.3e-15, 6.1e-16 | yes |
| E1 diff of `fidelity.rs` | only the sd lines, the loop binding, the doc | three hunks, as stated | yes |
| K-0652 `f718_fidelity_r3` | 8 pass | 8 passed, the 4 ignored records not run; coverage N1a 1.000, N1b 0.950, ND rounds 4 to 6 1.000 (as in ADR 0652) | yes |
| K3 `f699_fidelity` | pass | 17 passed (the file now holds 17 fast tests), 4 ignored; heavy `b_reduced_order_fan_heavy` passed (274.4 s) | yes |
| R `--lib`, docs_site | pass | 141 passed (4 ignored); docs_site 2 passed | yes |
| F fmt, clippy | clean | `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean | yes |
| T time | `< 120 s` | 0.5 s | yes |
| NC-S (diagnostic) double `a^T C a` of the accurate `a_i`, N1a | expected `> 1e-8` | **8.7e-11** (ND 5.3e-11) | not as expected (reported; no effect on the decision) |

R3's published numbers do not change on any registered fit: `e`, `beta`, `mse`, `se` and `loglik` come from unchanged code, and no
pair crossed the 2-sigma boundary (coverage 1.000, 0.950, 0.950, 1.000 as before).

Mutations:

| Id | Expected | Measured (largest V, of rms(r)) | Caught by |
|---|---|---|---|
| MS1 ADR 0652's `a_i^T C a_i` restored | V1 or V2 | N1a 1.31e-10, ND 8.1e-10, N1b 4.6e-15 | **survived** |
| MS2 conditional variance only | V1 to V3 | 1.3e-2 to 3.1e-1 | V (N1a first) |
| MS3 deletion correction dropped | at least one of V | 2.4e-3 to 6.9e-2 | V (N1a first) |
| MS4 `q_i` not zeroed | at least one of V | 3.0e-3 to 8.2e-2 | V (N1a first) |
| MS5 `L q` for `L^T q` | at least one of V | 1.1e-2 (N1a), 2.5e-1 (ND); N2 1.5e-13 | V (N1a first) |

Four of five caught.

### Diagnosis of MS1 and NC-S: the ADR 0652 miss was in its reference, not in the closed form

MS1 survives and NC-S stays under its limit for the same reason. Against the double-double reference, ADR 0652's closed-form
`sqrt(a_i^T C a_i)` is itself accurate to 1.3e-10 (N1a) and 8.1e-10 (ND) of rms(r), and the double quadratic form of an accurate
`a_i` loses only 8.7e-11. The 6.3e-8 and 1.6e-8 of ADR 0652 amendment 1 are the error of its test's double brute force. That test
forms each `a_i` by applying the brute-force predictor to the `n` unit vectors with double Gauss-Jordan solves of the 39 x 39
covariance (condition about 2e9), so the error sits in that `a_i` and not in the summation. Two observations confirm it. First,
K-0652 prints the same L3s numbers with the new sd as ADR 0652 did with the old (6.30e-8 for N1a, 1.62e-8 for ND), so the double
reference differs from both closed forms by the same amount. Second, the double-double reference agrees with both closed forms
within 1e-9. This corrects ADR 0652 amendment 1's diagnosis (that both evaluations lose about 1e-4 to cancellation in `a^T C a`):
at these sizes the closed form did not lose measurable accuracy, but its reference did. ADR 0652 could not see this from the inside,
because its two double references agreed with each other and neither was compared with the exact value; the double-double
reference settles it. The registration's estimate that the cancellation was in the closed form was wrong. The limits were not
changed.

What the new form still gives: no difference of large terms anywhere in the evaluation (both terms are sums of squares, D1), so its
accuracy does not depend on `|a_i|`; `O(n^2)` work per pair instead of `O(n^3)`; and the acceptance's accuracy clause met with a
margin of 11 (ND) to 75 (N1a). The old form's accuracy at larger `n` or smaller jitter was not measured.

### Deviations and what was not run

No set-up or limit changed and there is no amendment. NC-S and MS1 came out against their expectation and are reported above.
`f699_fidelity` now holds 17 fast tests (the registration said 16, the count in ADR 0622's addendum); all passed. Not run: the
whole workspace and whole-crate integration suites (the daily gate, ADR 0653), and `baseline_hashes` (no shared solver file is
touched). docs_site was run before the one-sentence `design-loop.md` edit and once more after it.

### Decision by the registered rule

**partial.** V1 and V2 (the accuracy clause), the evaluation without `a^T C a` (D1), and K-0652 and K3 (every ADR 0652 and ADR 0622
check keeps its result) are met, as is every other check. MS1 survives, and under the rule a surviving mutation makes the item
partial. The survival does not expose a gap in the new code. It shows that the old form was accurate too, so the acceptance's
premise, that the old evaluation lost accuracy, came from a misdiagnosis in ADR 0652, which this item corrects.

### Remaining questions (each with a recommendation)

1. **Status.** Options: (a) partial, as the rule gives; (b) the lead records done, since every acceptance clause is met and the
   surviving mutation only shows that the replaced form was not the faulty one. Recommendation: (a), by the rule, with a note
   saying why. Nothing more is needed to settle the accuracy question.
2. **ADR 0652's L3s records.** Its two `*_known_miss` records keep their limit and numbers (not edited). Options: (a) leave them,
   with the F-718 note naming this correction; (b) a follow-up that swaps the L3s reference in `f718_fidelity_r3.rs` for the
   double-double one. Recommendation: (a), because `f734_r3_sd.rs` already holds the check with the correct reference on the same
   fits.
3. **Keep the new form or the old.** Both are accurate here. Options: (a) keep the new one (no large intermediate terms, `O(n^2)`
   per pair); (b) revert. Recommendation: (a), taken on the maintainer's standing instruction of 2026-11-11 (take the
   recommendation).

## For the lead: roadmap entries

> Proposed status: partial

> Follow-up of F-718 (ADR 0652, 2026-11-16). ADR 0656, 2026-11-16. PARTIAL (every acceptance clause met; one registered mutation survived because the replaced form proved accurate too). Library only, opt-in (behind TrustRule::r3, default None): R3's leave-one-out predictive standard deviation is now evaluated as the conditional variance 1/G_ii = 1/|L^-1 e_i|^2 plus the variance of the R2 refit |L^T q_i|^2 through the Cholesky factor (q_i from the deletion formula of least squares), both sums of squares, instead of a_i^T C a_i; the ADR derives it from first principles. Against a brute force in double-double arithmetic (about 32 digits) it agrees within 1.3e-10 (N1a) and 9.2e-10 (the driver's final fit) of the residual scale on the noise-free fits of ADR 0652 (limit 1e-8), and within 2e-15 with noise. Every ADR 0652 test and every ADR 0622 test (including the heavy fan record) passes, R3's published numbers and coverage are unchanged, and default runs never reach the changed code. Four of five mutations caught. The fifth, which restores ADR 0652's old a^T C a, also agrees with the double-double reference within 8.1e-10: the 6.3e-8 and 1.6e-8 misses of ADR 0652 came from that test's double-precision brute force (unit-vector solves at condition about 2e9), not from cancellation in the closed form. Verified on synthetic fits only (n up to 40, two parameters).

> Note for F-718: the L3s misses recorded by ADR 0652 (6.3e-8 and 1.6e-8 of the residual scale) are errors of its double-precision brute-force reference; against a double-double reference the closed form agreed within 1.3e-10 and 8.1e-10 (ADR 0656, 2026-11-16).

No follow-up is proposed (remaining question 2 recommends keeping ADR 0652's records as they are).
