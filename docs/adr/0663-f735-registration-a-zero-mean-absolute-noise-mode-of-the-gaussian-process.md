# ADR 0663: F-735 registration: a zero-mean, absolute-noise mode of F-698's Gaussian process, used by R3

Status: **REGISTERED 2026-11-16, written before any code of F-735 and before any run of its checks.** Written by the
implementing agent on branch `f735-work` (from main 9f06e866, which holds F-698 / ADR 0621, F-715 / ADR 0651, F-718 / ADR 0652 and
F-734 / ADR 0656), worktree `w49i-work/f735`, scratch `w49i-work/f735-scratch`. Not yet accepted by the maintainer. Every choice
below that the maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation) and is marked "(standing instruction)".

Roadmap item F-735 (deps F-718, F-715), the follow-up proposed by ADR 0652 (its D1 interpretation and remaining question 1: move
R3's process into `surrogate.rs` once F-715 had landed). The binding acceptance:

> surrogate::Gp gains an opt-in zero-mean mode with an absolute per-observation noise variance, R3 calls it in place of its own process, every ADR 0652 check keeps its result (fits bit-identical or within 1e-12), and every ADR 0621 check keeps its result.

House rules: clean-room (first principles; the formulas are ADR 0652 D2's, restated below); library only, opt-in; defaults
bit-identical; no new dependency, no unsafe code; `baseline_hashes.rs`, the hash tables and the golden files are not edited.
`design/bayes.rs` (F-716's pending merge) and the hypothesis code (F-727) are not touched. The library change is confined to
`crates/rcfd-api/src/design/surrogate.rs` (the new mode) and `crates/rcfd-api/src/design/fidelity.rs` (R3's process replaced by
calls to it); the checks go in a new `crates/rcfd-api/tests/f735_gp_zero_mean.rs`. No existing test file is edited.

## What was read

`AGENT_RULES.md`; the brief; ADR 0652 (D1, D2, D3, addendum 1, remaining question 1) and ADR 0656 (D1, D2, addendum 1);
`design/surrogate.rs` in full; in `design/fidelity.rs` the R3 section (`R3Rule`, `R3Fit`, `R3_JITTER`, `r3_cov`, `chol_inverse`,
`r3_lik`, `r3_log_likelihood`, `r3_search`, `R3Loo`, `r3_core`, `r3_loo`, `r3_mean`) and `Correction::hash`;
`design/study.rs` (`parallel_map`: with one worker a plain in-order map, with more the results in index order);
`tests/f718_fidelity_r3.rs` (its set-ups N1a, N1b, N2, N3 and `k1_likelihood_gradient`); the list of tests in
`tests/f734_r3_sd.rs`; the R3 and surrogate sections of `docs/design-loop.md`.

## Disclosed: a probe on main before this registration

To have a reference for "fits bit-identical" that does not depend on the new code, a scratch test (`f735-scratch/main-copy`, a
`git archive` of 9f06e866, never committed; its two files are in the scratch folder) was built and run once on unchanged main
(release). It prints, for four R3 fits, the `Correction::hash` of `Correction::fit` with `r3: Some(R3Rule::default())` and an
FNV-1a 64 digest of the bits of `r3_loo(.., None)`'s `l, sf2, loglik, beta, e, sd, in_sample` (in that order, each `f64` as its
little-endian bits); and for four fits of F-698's existing modes a digest of the bits of `length_scales, nugget, jitter,
log_likelihood, mean, process_variance`, `predict` at 7 points, `loo`, and `log_likelihood_and_gradient`. The set-ups and the
numbers are below (set-ups S, checks Z2 and Z5). No other number was computed before this registration.

| Fit (main 9f06e866) | `Correction::hash` | `r3_loo` digest | chosen rung |
|---|---|---|---|
| N1a | `fnv1a64:45da7c9c7db509b3` | `2d53dcb4b5b1dfc1` | R3 |
| N1b | `fnv1a64:9d93b31564bf8555` | `9b3ca685c94031f2` | R3 |
| N2 | `fnv1a64:83d3c555da6b8adc` | `be3435a70f99e7fb` | R2 |
| H1 | `fnv1a64:528df5f492569d7d` | `f64c44a1f528b3da` | R3 |

| Existing-mode fit (main 9f06e866) | digest |
|---|---|
| A (`Gp::fit`, defaults) | `2a8fd6de3d74ce5a` |
| B (SE, `Noise::Fitted`, gradient with `g`) | `e614e23323bfb49d` |
| C (`Noise::Nugget(1e-3)`, 4 workers) | `8617aa91cf0047b4` |
| D (`with_hyper`, SE, gradient with `g`) | `9f131f2c8570d07d` |

The probe also printed R3's fitted `l` bits and `s_f^2` (N1a `l = (0x3fd7928aa155265e, 0x3fe5c7bd210b7525)`, `s_f^2 = 0.2161`;
N2 at the lower `tau` bound, `s_f^2 = 2.1e-9`); these are not limits. The probe files are
`f735-scratch/main-copy/crates/rcfd-api/tests/f735_probe.rs` and `f735_probe2.rs`, logs `f735-scratch/probe.log`, `probe2.log`.

## Design (fixed now)

### D1. The mode

A `Gp` built in the zero-mean mode models the outputs `y` (no centring, no scaling) as `y ~ N(0, C)` with

  `C = s_f^2 (R + J I) + diag(v)`, `R_ij = correlation(kernel, u_i, u_j, l)`, `J = JITTER = 1e-8`,

`v_i >= 0` the absolute noise variance of observation `i` in output units squared (not a ratio to the process variance, which is
what `Noise::Nugget` and `Noise::Fitted` mean), `s_f^2 > 0` the process variance, a hyperparameter (not profiled). This is ADR 0652
D1/D2's process with the kernel left to the caller. The jitter is fixed: no escalation (the existing modes multiply it by 100 up
to three times on a failed factorisation; R3 never did, and an escalation would turn a start that fails in R3 into one that does
not, which would change R3's fits). Objective, the zero-mean log marginal likelihood without its constant (Rasmussen and Williams
2006, eq. 2.30, from memory; derived in ADR 0652 D2): `L = -y^T alpha / 2 - sum_i ln L_ii`, `alpha = C^-1 y`, `C = L L^T`.
Gradient in `theta = (ln l_1, .., ln l_d, ln s_f^2)`: `dL = (alpha^T dC alpha - tr(C^-1 dC)) / 2` with `dC/d ln s_f^2 = s_f^2 (R +
J I)` and `dC_ij / d ln l_k = s_f^2 (dk_ij / d ln l_k)`, off the diagonal: for the squared exponential `R_ij ((u_ik - u_jk) /
l_k)^2`, for Matern 5/2 the existing `dcorr_common` factor times `((u_ik - u_jk) / l_k)^2` (ADR 0621's derivative, unchanged).

Prediction (latent function, output units): `m(x) = k^T alpha` with `k_i = s_f^2 R(u_i, x)`; `var(x) = s_f^2 - |L^-1 k|^2`,
floored at 0 (Rasmussen and Williams eqs. 2.25, 2.26, from memory; the Cholesky form as ADR 0656 D1). Leave-one-out (the
observation, as the existing `loo`): with `G = C^-1`, residual `y_i - m_-i = alpha_i / G_ii` and variance `1 / G_ii` (block
inverse; Rasmussen and Williams eqs. 5.10 to 5.12, from memory; ADR 0652 D3 uses the same identity with the R2 refit added). So
the mean is `y_i - alpha_i / G_ii` and the sd `sqrt(1 / G_ii)`. In the zero-mean mode `nugget()` is 0, `mean()` is 0,
`process_variance()` is `s_f^2`, `jitter()` is `J`.

### D2. API (names fixed now)

* `pub struct ZeroMeanOptions { kernel, starts, refine, iters, l_bounds, sf2_bounds, seed, workers }`, `Default`: `kernel:
  Kernel::Matern52` (F-698's default kernel), `starts 10`, `refine 3`, `iters 40`, `l_bounds (0.05, 10)`, `sf2_bounds (1e-6,
  1e2)`, `seed 1`, `workers 1` (R3Rule's numbers; the `sf2` box is in output units squared, so a caller scales its outputs, as R3
  does). `GpOptions` is not changed (tests build it with struct literals).
* `Gp::zero_mean_with_hyper(u, y, noise_var, kernel, l, sf2) -> Result<Gp, String>`: no search.
* `Gp::fit_zero_mean(u, y, noise_var, &ZeroMeanOptions) -> Result<Gp, String>`: the search of ADR 0621 D4 / ADR 0652 D2 over the
  log box of `(l, s_f^2)`: the centre and `starts - 1` points of `latin_hypercube(starts - 1, d + 1, seed)`, evaluated by
  `parallel_map` (results in index order), the best `refine` (ties by start order) refined by `projected_gradient` (step 0.1,
  Armijo 1e-4, 30 halvings, `iters`), the best refined value kept (ties: the earlier); a failed factorisation or a non-finite
  likelihood is `-inf`. Then the process at the best point. Refuses: the inputs `check_inputs` refuses (including more than
  `N_MAX = 2000` observations), `noise_var` of the wrong length or with an entry not finite or negative, `starts` or `refine` 0,
  bounds not `0 < lo < hi` finite, and no start that factorises.
* `Gp::is_zero_mean()`, `Gp::noise_variances() -> &[f64]`, `Gp::cholesky_factor() -> &[f64]` (row-major lower factor of `C`),
  `Gp::weights() -> &[f64]` (`alpha`). `predict`, `loo`, `log_likelihood`, `log_likelihood_and_gradient` (its `with_g` ignored in
  this mode, the gradient's last entry is in `ln s_f^2`) dispatch on the mode.
* The existing modes: the struct gains two private fields (`zero_mean: false`, `noise: []`) and `predict`, `loo` and
  `lik_gradient` gain one branch on the mode at their top; no line of their arithmetic changes.

### D3. R3 on the mode

`r3_cov`, `r3_lik` and `r3_search` are removed from `fidelity.rs`. `r3_core` scales as before (`r' = r / s`, `v_i = (sigma_i /
s)^2`, `tau = s_f^2 / s^2` for given hyperparameters), then calls `Gp::fit_zero_mean(u, r', v, opts)` (no hyperparameters given;
`opts` from `R3Rule`: `kernel SquaredExponential`, `sf2_bounds = tau_bounds`, `workers 1`, the rest copied) or
`Gp::zero_mean_with_hyper(u, r', v, SquaredExponential, l, tau)`, and takes `l`, `tau`, the factor, `alpha` and the
log likelihood from the `Gp`; everything after (the inverse, the leave-one-out residuals and sds of ADR 0652 D3 and ADR 0656 D1, the
in-sample residuals, the R3Loo fields) is unchanged code. `r3_log_likelihood` becomes `zero_mean_with_hyper` then
`log_likelihood_and_gradient`, `None` when it refuses or the value is not finite (as before). An error's text is the `Gp`'s with
its `gp: ` prefix removed; the two texts R3 could produce before ("no start of the likelihood search factorised", "covariance not
positive definite (row i, pivot p)") are the `Gp`'s texts, so R3's `why` strings are unchanged.

Why bits are kept: for the squared exponential, the new code performs R3's floating-point operations in R3's order (the diagonal
`s_f^2 (1 + J) + v_i`, the off-diagonal `s_f^2 R_ij`, the same factorisation and substitutions, the quadratic as `sum y_i
alpha_i`, the gradient loop with the squared-exponential derivative taken as `s_f^2 R_ij` itself, the same starts, the same
search). With one worker `parallel_map` is R3's sequential map, and the refined starts are compared in the same order. So the fits
are expected bit-identical, inside the acceptance's 1e-12.

### D4. Opt-in, defaults bit-identical

Nothing reaches the zero-mean mode unless it calls the two new constructors (R3 behind `TrustRule::r3`, default `None`). The
existing modes' arithmetic is not changed (D2). Checked by Z5 against main's bits and by the existing ADR 0621 tests.

## Set-ups (fixed now)

* **R3 fits** (two parameters, `n_pairs` of `f718_fidelity_r3.rs`, 40 LHS pairs): **N1a** (seed 718_011, structured, noise-free),
  **N1b** (718_012, noise 0.02, normals 718_013), **N2** (718_021, unstructured, noise 0.05, normals 718_022), and a new **H1**
  (735_001, structured, heteroscedastic stated noise `0.03 (0.5 + 1.5 x1)` per pair, normals 735_002), each with
  `r3: Some(R3Rule::default())`.
* **Existing modes** (F-698): 15 LHS points `latin_hypercube(15, 2, 735_101)`, `y = sin(3 u_1) + u_2^2 + 1e-3 ((7 i mod 5) - 2)`,
  queries `latin_hypercube(7, 2, 735_102)`; **A** `Gp::fit` default options; **B** squared exponential, `Noise::Fitted`; **C**
  `Noise::Nugget(1e-3)`, 4 workers; **D** `Gp::with_hyper(SE, l = (0.3, 0.5), g = 1e-4)`.
* **Mode checks** (new): **M** 20 LHS points `latin_hypercube(20, 2, 735_201)`, `y = sin(4 u_1) cos(2 u_2)`, absolute noise
  `v_i = 1e-4 (1 + i mod 3)`, both kernels, hyperparameters `l = (0.4, 0.6)`, `s_f^2 = 0.5`, queries `latin_hypercube(5, 2,
  735_202)`.

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f735_gp_zero_mean.rs` (release), numbers printed before any assertion, except K and F.

| Id | Clause | What | Limit (estimate) |
|---|---|---|---|
| Z1 | R3 calls the mode; fits bit-identical | A frozen copy, in the test, of main's `r3_cov`, `r3_lik` and `r3_search` (verbatim from 9f06e866, the reference) against the mode on R3's scaled residuals and noise of N1a, N1b, N2, H1 (`r3_loo`'s `r / s`, `(u_high / s)^2`): the searched `theta` (as `ln` of the `Gp`'s `l`, `s_f^2`), the log likelihood, and the likelihood and gradient at the fitted point and at `(ln 0.3, ln 0.3, 0)`, `(ln 2, ln 0.1, ln 0.01)` | bit-equal, or relative `<= 1e-12` per number (acceptance); estimate bit-equal (D3) |
| Z2 | fits bit-identical | `Correction::hash` and the `r3_loo` digest of N1a, N1b, N2, H1 against the probe on main (above) | equal (the acceptance's 1e-12 cannot be shown by a hash: if a digest differs, the test prints every component's largest relative difference against the frozen copy of Z1 chained through the unchanged `r3_core` arithmetic, and the clause is judged on that; estimate equal) |
| Z3 | R3 calls the mode | `fidelity.rs` holds no `r3_cov`, `r3_lik`, `r3_search` and no call of `surrogate::cholesky`; `r3_core` and `r3_log_likelihood` call `Gp::fit_zero_mean` / `Gp::zero_mean_with_hyper` | inspection of `git diff 9f06e866 -- crates/rcfd-api/src/design/fidelity.rs` and a grep |
| Z4a | the mode | M, both kernels: `log_likelihood_and_gradient` against central differences of `log_likelihood` (step 1e-5 in `theta`) at `theta` of M and at `(ln 0.2, ln 1.5, ln 2)` | relative `<= 1e-5` per component, absolute `1e-7` when the component is below 1e-3 of the largest (K1's limit) |
| Z4b | the mode | M, both kernels: `log_likelihood` against the test's own `-y^T C^-1 y / 2 - ln det C / 2` (Gauss-Jordan with partial pivoting, determinant from the pivots, `C` built by the test) | relative `<= 1e-10` (condition of C about `s_f^2 / min v` = 5e3; estimate 1e-13) |
| Z4c | the mode | M, both kernels: `predict` mean and sd at the 5 queries against the test's solve of `C w = k` (mean `w . y`, variance `s_f^2 - k . w`) | `<= 1e-10 sqrt(s_f^2)` (estimate 1e-13) |
| Z4d | the mode | M, both kernels: `loo` against brute force (observation `i` removed, the test's solve on `n - 1`, observation variance `s_f^2 (1 + J) + v_i - k . w`) | `<= 1e-10 sqrt(s_f^2)` |
| Z4e | absolute per-observation noise | M, SE: observation 0 given `v_0 = 1e8`; `predict` at the 5 queries against the fit with observation 0 removed | `<= 1e-6 sqrt(s_f^2)` (the effect of `v_0` finite is about `s_f^2 / v_0` = 5e-9 relative) |
| Z4f | the mode's search | M, SE, `fit_zero_mean` default options with the SE kernel, at 1 and 4 workers | `l`, `s_f^2`, log likelihood bit-equal; log likelihood not below that at `l = (0.4, 0.6), s_f^2 = 0.5` |
| Z4g | refusals | `noise_var` of length n - 1, an entry `-1` and `NaN`; `sf2` 0; `sf2_bounds (1, 1)`; `starts 0` | each `Err` |
| Z5 | every ADR 0621 check keeps its result (defaults) | digests of A, B, C, D against the probe on main (above) | equal |
| K-0652 | every ADR 0652 check | `--test f718_fidelity_r3` (8 tests; 4 ignored records not run), `--test f734_r3_sd` (1 test), `--test f699_fidelity` (fast tests; the heavy record not run, see Limits) | all pass |
| K-0621 | every ADR 0621 check | `--test f698_surrogate`, `--test f715_robust_ei`, `--test f697_fan_objective`; `--lib` | all pass (the ignored records not run) |
| R | — | `cargo test -p rcfd-cli --release --test docs_site` (if `docs/` is touched) | pass |
| F | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |
| T | — | the new test's release time | under 120 s (estimate: four R3 searches, about 2 s) |

## Negative controls and mutations (registered now)

**NC-K (control of Z1's power).** F-698's existing ordinary-kriging mode, `Gp::fit` with `kernel SquaredExponential`, `Noise::
Nugget(mean v / mean r'^2)`, on N1b's scaled residual: its length scales differ from R3's frozen search by more than 1e-12
relative. Expected to differ (a profiled constant mean and a relative nugget are a different model); it shows that Z1 can tell the
modes apart.

Mutations (scratch copy from `git archive` of the code commit, one at a time, the new test file plus K-0652's `f718_fidelity_r3`
where named; expected to fail):

| Id | Mutation | Expected to fail |
|---|---|---|
| MZ1 | the jitter dropped from the zero-mean diagonal (`s_f^2 + v_i`) | Z1, Z2 |
| MZ2 | the noise taken as a ratio (`s_f^2 (1 + J + v_i)`) | Z1, Z2, Z4b, Z4e |
| MZ3 | the trace term dropped from the zero-mean gradient | Z1, Z4a |
| MZ4 | the zero-mean `predict` variance without the subtraction (`var = s_f^2`) | Z4c |
| MZ5 | the zero-mean `loo` variance `G_ii` in place of `1 / G_ii` | Z4d |
| MZ6 | the search's LHS seed `seed + 1` in `fit_zero_mean` | Z1, Z2 |
| MZ7 | the existing `predict`'s mean-uncertainty term `b^2 / (1^T K^-1 1)` dropped | Z5 (and f698_surrogate) |
| MZ8 | the jitter escalation applied in the zero-mean mode (factor 100 on a failed factorisation) | none expected on the registered fits if no start fails; reported, not decided on |

## Decision rule

* **done:** Z1, Z2 (or the 1e-12 fallback of Z2), Z3, Z5, K-0652 and K-0621 met (the acceptance), and Z4a to Z4g, R, F, T met;
  MZ1 to MZ7 caught. NC-K and MZ8 are reported, not decided on.
* **partial:** the acceptance's checks (Z1, Z2, Z3, Z5, K-0652, K-0621) met, and a non-acceptance check missed (an ignored
  `*_known_miss` record with the limit intact) or one of MZ1 to MZ7 survives.
* **todo** (the lead does not take the code): Z1, Z2, Z3 or Z5 missed, or an ADR 0652 or ADR 0621 test changes its result.

## Limits (not covered)

* The mode adds no fitted noise: `v` is given. A fitted noise level on top of the stated one is not covered.
* No output scaling in the mode: the `sf2` box is absolute; R3 scales by the RMS of its residual before calling.
* R3 inherits `N_MAX = 2000` (it had no cap; an R3 fit of more than 2000 pairs now reports "observations exceed the cap" as
  its `why`). Hyperparameters given to `r3_loo` whose `tau = s_f^2 / s^2` underflows to 0 or overflows now refuse in the `Gp`
  (before they reached the factorisation). Neither occurs in the registered set-ups.
* `R3Rule` has no thread count: R3 runs the search on one worker, as before.
* R3's `predict` from a stored `R3Fit` (`r3_mean`, from the JSON's `beta`, `l`, `sf2`) is not moved: it has no data to build a
  `Gp` from, and its arithmetic is left as it is.
* ADR 0622's heavy record (`b_reduced_order_fan_heavy`, 215 to 274 s) is not rerun: R3 is off there (it never reaches the changed
  code), the fast `f699_fidelity` tests cover the same driver path.
* Synthetic set-ups only, two parameters, `n` up to 40.

## References

* C. E. Rasmussen and C. K. I. Williams (2006), Gaussian Processes for Machine Learning, MIT Press, sections 2.2, 2.3 and 5.4
  (from memory; every formula used is derived in ADR 0652 D2, D3 and ADR 0656 D1 or above).
* ADR 0621 (F-698), ADR 0652 (F-718), ADR 0656 (F-734).

## Addenda

Results, amendments and the roadmap entries go here; nothing above is changed after a run.
