# ADR 0652: F-718 registration: the Gaussian-process discrepancy rung R3 of the fidelity driver

Status: **REGISTERED 2026-11-16, written before any code of F-718 and before any run of its checks.** Written by the
implementing agent on branch `f718-work` (from local main d40cd665, which holds F-698 / ADR 0621 and F-699 / ADR 0622),
worktree `w49i-work/f718`, scratch `w49i-work/f718-scratch`. Not yet accepted by the maintainer. Every choice below that the
maintainer would normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation) and is marked "(standing instruction)".

Roadmap item F-718 (deps F-699, F-698), follow-up of ADR 0622 (its addendum 1, "Follow-up proposed"). The binding acceptance:

> with F-698's Gaussian process on the R2 residual (zero mean, nugget from the stated uncertainty, hyperparameters fixed per fit and leave-one-out from the closed form), R3 is chosen over R2 only by the one-standard-error rule, its leave-one-out residuals equal brute force within 1e-8, its 2-sigma leave-one-out coverage is at least the registered value on a smooth nonlinear synthetic discrepancy, and every ADR 0622 check keeps its result.

House rules: clean-room (published literature and first principles; every formula used is derived below); library only,
opt-in (no case key, no MCP tool, no service method); defaults bit-identical (`baseline_hashes.rs`, the hash tables and the
golden files are not edited); no new dependency, no unsafe code. F-715 edits `surrogate.rs` and `bayes.rs` on another branch,
so this item edits neither: the library change is confined to `crates/rcfd-api/src/design/fidelity.rs`, the checks to a new
`crates/rcfd-api/tests/f718_fidelity_r3.rs`. ADR 0642 (F-717, the uncertainty guard check and the P4 fix) is on another branch
and is not used.

## What was read

`AGENT_RULES.md`; the brief; ADR 0622 (stub D4, D5; amendment 1, its design and checks; amendment 2; addendum 1 and its
follow-up text); ADR 0621 only through its code: `design/surrogate.rs` (public API: `Kernel::{Matern52, SquaredExponential}`,
`correlation`, `cholesky`, `solve_lower`, `solve_upper_t`, `JITTER`, `JITTER_STEPS`, `Gp::{with_hyper, fit, predict, loo}`);
`design/fidelity.rs` in full; `tests/f699_fidelity.rs` (its helpers and how it builds `TrustRule`: always with
`..TrustRule::default()`; its `match` on `Rung` has a wildcard arm); `design/optim.rs` (`projected_gradient`, `PgOptions`,
`GradEval`, `Stop`); `design/sampling.rs` (`latin_hypercube`); `design/rng.rs` (`Rng::new`, `uniform`).

## Disclosed: a prototype and arithmetic before this registration

A numpy prototype (`f718-scratch/proto.py`, `proto2.py`, about 90 lines, written from the design below, not rcfd code; numpy's
own Latin hypercube and generator, seeds 5, 6, 7, none used by a check; hyperparameters by 3000 random draws and a random walk,
not the gradient search below) on the set-up N1 below (`n = 40`, `d = 2`) gave:

1. N1a (no noise): length scales about (0.37, 0.70), R2 leave-one-out MSE 1.1e-2 to 1.4e-2, R3 1.1e-7 to 8.7e-7, RMSE ratio
   R3 / R2 0.003 to 0.008, 2-sigma coverage 1.000, R3 chosen by the one-standard-error rule in 3 of 3. With `n = 24`: ratio 0.04
   to 0.07, coverage 1.000.
2. N1b (noise sd 0.02, stated 0.02): ratio 0.24 to 0.31 (the floor from the noise alone is 0.02 / 0.11 = 0.18), coverage 0.950
   to 1.000, R3 chosen in 3 of 3.
3. N2 (affine plus white noise 0.05, stated 0.05, no structure): R3 never chosen (MSE equal to R2's when the process variance
   falls to its lower bound; 0.92 of R2's but within one standard error at seed 6), coverage 0.925 to 1.000.
4. Closed-form against brute-force leave-one-out residuals (proto2): noise-free, the correlation matrix has condition number
   about 2.2e9 (jitter 1e-8) and the largest difference is 1.1e-10 of the rms R2 residual, which is 2.4e-8 of the rms of the R3
   residuals themselves; with noise 0.02 the condition is about 2e3 and the difference 3e-14. So a tolerance relative to the R3
   residuals' own size would fail on rounding alone in the noise-free case; the tolerance below is relative to the rms of the R2
   residual (the scale of the quantity the process models), and is 100 times the prototype's rounding.

## Design (fixed now; names are the implementation's)

### D1. Model

R3 is the autoregressive form of Kennedy and O'Hagan (2000), `y_H(x) = rho y_L(x) + b0 + c . x + delta(x)`, with the low
model evaluated exactly at every query (ADR 0622 D4 note: only the discrepancy needs a statistical model). Two stages, as the
acceptance says:

1. R2 exactly as ADR 0622 amendment 1: the columns `X = [yL, 1, u_1..u_d]` (`u` the unit-box parameters), Householder least
   squares, coefficients `b`, in-sample residuals `r = M y_H` with `M = I - P`, `P = X (X^T X)^-1 X^T` the hat matrix,
   leverages `h_ii = P_ii`, R2 leave-one-out residuals `e2_i = r_i / (1 - h_ii)`. The coefficients stored for R3 are R2's, bit
   for bit.
2. `delta` a zero-mean Gaussian process on `r` (Rasmussen and Williams 2006, chapter 2, from memory; the formulas used are
   derived here): covariance `C = s_f^2 (R + J I) + diag(sigma_i^2)`, `R_ij = exp(-sum_k ((u_ik - u_jk) / l_k)^2 / 2)` (F-698's
   `correlation(Kernel::SquaredExponential, ..)`; ADR 0622 D4 names the squared exponential), `J = surrogate::JITTER = 1e-8`,
   `sigma_i` the high source's stated standard uncertainty of the metric at pair `i` (the nugget from the stated uncertainty;
   zero uncertainty leaves the jitter only). Posterior mean at a unit-box point `u`: `mu(u) = s_f^2 sum_j R(u, u_j) beta_j`,
   `beta = C^-1 r`. Prediction of R3: `rho y_L + b0 + c . x + mu(u)`.

Interpretation (standing instruction): F-698's `Gp` is ordinary kriging with a profiled constant mean and a nugget ratio
relative to the profiled process variance; neither the zero mean nor an absolute nugget from a stated uncertainty can be had
from it without editing `surrogate.rs`, which F-715 is changing. So R3 uses F-698's kernel (`correlation`), its dense
factorisation (`cholesky`, `solve_lower`, `solve_upper_t`) and its jitter constant, and builds the zero-mean process with the
absolute nugget in `fidelity.rs` (about 250 lines). Remaining question 1 offers moving it into `surrogate.rs` after F-715.

### D2. Hyperparameters, fixed per fit

`theta = (ln l_1, .., ln l_d, ln tau)`, computed in scaled units: `s = sqrt(mean(r^2))`, `r' = r / s`, `sigma'_i = sigma_i / s`,
`tau = s_f^2 / s^2`. Box: `l_k` in `[0.05, 10]` (F-698's default length-scale box), `tau` in `[1e-6, 1e2]`. Objective: the
zero-mean log marginal likelihood without its constant, `L = -r'^T C'^-1 r' / 2 - sum_i ln L_ii` (`C' = L L^T`, the Cholesky
factor). Gradient (derived: `dL = (alpha^T dC alpha - tr(C^-1 dC)) / 2`, `alpha = C'^-1 r'`, from `d(C^-1) = -C^-1 dC C^-1` and
`d ln det C = tr(C^-1 dC)`): `dC/d ln tau = tau (R + J I)`; `dC_ij / d ln l_k = tau R_ij ((u_ik - u_jk) / l_k)^2` (zero on the
diagonal). Search as F-698's (ADR 0621 D4): the centre of the log box and `starts - 1` points of `latin_hypercube(starts - 1,
d + 1, seed)` evaluated in order, the best `refine` by likelihood (ties by start order) refined by `projected_gradient`
(`step 0.1`, `armijo 1e-4`, `max_halvings 30`, `iters` iterations), the best refined value kept (ties: the earlier). Defaults
`starts 10`, `refine 3`, `iters 40`, `seed 1`, sequential (no thread count: the fit is cheap at the driver's sizes). A point where
the factorisation fails has the value `-inf`. R3 is not available, with the reason recorded, when `n < 10 d` (ADR 0622 D4's
smallest n), R2 is not available, `s` is 0 or not finite, or no start gives a factorisation. The hyperparameters are found
once per fit on all `n` pairs and held in every leave-one-out fold (the acceptance: "fixed per fit").

### D3. Leave-one-out from the closed form

Brute force for pair `i`: refit R2 by least squares on the other `n - 1` pairs, `b_(-i)`; their residuals `r^(i)_j = y_j - x_j^T
b_(-i)`; the zero-mean process with the same `(l, s_f^2)` and the same `sigma_j` on them; the R3 leave-one-out residual
`e3_i = y_i - x_i^T b_(-i) - mu_(-i)(u_i)`. Closed form (derived now, first principles):

* From the deletion formula of least squares (the Sherman-Morrison update of `(X^T X)^-1`), `b - b_(-i) = (X^T X)^-1 x_i e2_i`,
  so `y_i - x_i^T b_(-i) = e2_i` and `r^(i)_j = r_j + P_ji e2_i`.
* With `G = C^-1` and the block inverse, the zero-mean kriging weights of observation `i` from the others are
  `C_(-i)^-1 C_(-i),i = -G_(-i),i / G_ii` (the off-diagonal covariance of an observation and a latent value are the same).
* Hence `mu_(-i)(u_i) = -sum_{j != i} G_ij (r_j + P_ji e2_i) / G_ii`, and with `e2_i (1 - h_ii) = r_i`:

  `e3_i = ((G r)_i + e2_i (G P)_ii) / G_ii`.

  Check: with no process (`G = I / sigma^2`), `e3_i = r_i + h_ii e2_i = e2_i`.
* The leave-one-out predictive standard deviation used for coverage: `e3 = A y` is linear in `y_H` with rows
  `a_i = ((G M)_i,: + ((G P)_ii / (1 - h_ii)) M_i,:) / G_ii`, and `A X = 0` (since `M X = 0`), so under the model (regression plus
  process plus noise of covariance `C`) `var(e3_i) = a_i^T C a_i` exactly, at the fixed hyperparameters: the variance includes the
  refit of the R2 coefficients and the noise of the left-out observation. `sd_i = sqrt(a_i^T C a_i)`.
* `P` is built column by column from the existing least squares: `P e_k = e_k - (residual of regressing e_k on X)`, so the
  existing `lstsq` is reused unchanged.
* Coverage: the fraction of pairs with `|e3_i| <= 2 sd_i`.
* Computed in scaled units and mapped back (`e3 = s e3'`, `sd = s sd'`, `s_f^2 = tau s^2`, `beta = C'^-1 r' / s`).

### D4. Rung choice and trust

The ladder gains level 3 = R3 above R2 (levels 0 R0, 1 the better R1 form, 2 R2, 3 R3); `MSE_3 = mean(e3^2)`, `SE_3 =
sd(e3^2) / sqrt(n)` (divisor `n - 1`), as for the other levels. The rung is chosen by the existing one-standard-error rule over
every available level, unchanged: the lowest level whose MSE is at most `MSE_min + SE_min`. Nothing else enters the choice of
R3 over R2 (the acceptance's "only by the one-standard-error rule"). The `in_sample` negative-control switch uses R3's in-sample
residuals `r - s_f^2 R beta`. Trust of R3 (ADR 0622 D5): t2 (`sqrt(MSE_3) <= f_trust rmse0`), t3 (`rmse0 > k_u u_rms`) as for
the other rungs, and the coverage condition of ADR 0622 D5.3 (stub) for R3 only: coverage `>= coverage_min`, default 0.8 (the
stub's proposed value; standing instruction). As for every rung, a chosen but untrusted rung leaves R0 inside the region.
Outside the calibrated region R3, like R2, is never applied: the query is flagged and gets the trusted constant or nothing.

### D5. Opt-in, defaults bit-identical

* `TrustRule` gets `pub r3: Option<R3Rule>`, default `None`. `R3Rule { n_per_d: 10, coverage_min: 0.8, starts: 10, refine: 3,
  iters: 40, l_bounds: (0.05, 10.0), tau_bounds: (1e-6, 1e2), seed: 1 }` (`R3Rule::default()`). Validation refuses non-finite or
  inverted bounds, `starts` or `refine` 0, `coverage_min` outside `[0, 1.01]` (1.01 allowed only as a negative-control value).
* `Rung::R3` is added (`as_str` "R3"); `Rung::ALL` stays the four rungs of ADR 0622 so the existing `mse` map and its JSON
  keys keep their bytes. `MetricFit` gets `pub r3: Option<R3Fit>` (`None` when the rule is off). `R3Fit { available, why, l,
  sf2, jitter, beta, mse, se, coverage, loglik, se_levels }`.
* JSON: the spec's `trust` object gets an `r3` key, and each fit an `r3` object, only when the rule is on. With the rule off
  every byte the driver writes is the bytes it wrote before (check K2).
* `Correction::predict` with an R3 fit inside the region and trusted: the R2 form plus `mu(u)` with the region's points as the
  process inputs (they are the pairs' unit-box points in pair order, the same set). The summary's `rmse_after` takes the
  chosen rung's MSE (R3's when R3 is chosen).
* Public functions for the checks: `r3_loo(pairs, params, m, rule, hyper: Option<(&[f64], f64)>) -> Result<R3Loo, String>`
  (residuals, standard deviations, hyperparameters, in-sample R2 residuals, scale; `hyper` given: no search) and
  `r3_log_likelihood(u, r, sigma, theta) -> Option<(f64, Vec<f64>)>` (scaled units as in D2).
* Privacy (ADR 0622 D7) is unchanged: the R3 numbers travel inside the correction, which goes to the full (private) ledger
  only; the public summary is untouched.

## Set-ups (fixed now)

* **N1, smooth nonlinear discrepancy.** `x1` [m] in [0, 1], `x2` [1] in [-1, 1]. Low (`analytic.n1` 1, tier analytic):
  `f = sin(3 x1) + x2^2` [Pa]. High: `f_H = 1.3 f - 0.2 + 0.5 x1 - 0.3 x2 + 0.3 sin(2 pi x1) cos(1.5 x2)` (+ noise in N1b).
  **N1a**: 40 pairs at `latin_hypercube(40, 2, 718_011)` scaled to the box, no noise, stated uncertainty 0, `Correction::fit`
  directly. **N1b**: 40 pairs at `latin_hypercube(40, 2, 718_012)`, noise `0.02 z_i`, `z_i` standard normal by Box-Muller from
  one `Rng::new(718_013)` in pair order (`z = sqrt(-2 ln(1 - u1)) cos(2 pi u2)`, the f699 test's form), stated 0.02.
* **N2, no structure.** N1's parameters and low; high `f_H = 1.3 f - 0.2 + 0.5 x1 - 0.3 x2 + 0.05 z_i`, stated 0.05, 40 pairs at
  `latin_hypercube(40, 2, 718_021)`, `z` from `Rng::new(718_022)`.
* **N3, exact affine, three parameters.** ADR 0622's S1 (low `f, g`; high `f_H = 1.7 f - 0.3 + 0.5 x1 - 2.0 x2 + 0.04 x3`,
  `g_H = 0.9 g + 1.25 - 0.1 x1 + 0.3 x2 - 0.02 x3`), 40 pairs at `latin_hypercube(40, 3, 718_031)` scaled to S1's box.
* **ND, N1 through the driver.** N1's low; high N1a's `f_H` through `ModelSource` (`synthetic.n1` 1, level `high`);
  candidates `Lhs { 300, seed 718_041 }`; metric `f` (floor 1e-3); objective `f` Min; 6 rounds of 6 (2 explore), so R3 is
  available from round 4 (`n = 24 >= 10 d = 20`); `trust.r3 = Some(R3Rule::default())`; workers 1 (and 8 for D4).
* **NP, privacy.** ND with the high source wrapped as private (`private() = true`, tier measured, source `{id: rig-n1, kind:
  measured}`, stated uncertainty of `f` 0.001), `Privacy::new(<temp>/rcfd_f718_np_<pid>/private/ledger.jsonl, "rig-n1")`, public
  ledger `<temp>/.../public/ledger.jsonl`; and its control, the same source unwrapped (public), no privacy.

Every ledger has the fixed clock `1_700_000_000_000`.

## Checks (limits fixed now)

All in `crates/rcfd-api/tests/f718_fidelity_r3.rs`, release profile, except K2 (scratch) and R.

| Id | Clause | What | Limit |
|---|---|---|---|
| S1 | one-SE | N1a, N1b, N2, N3 and every round's correction of ND: the chosen rung of every metric equals the one-standard-error rule recomputed in the test from leave-one-out residuals the test obtains through `loo_residuals` (R0 to R2) and `r3_loo` (R3, with the fit's hyperparameters), MSE and SE computed in the test | exact, every fit |
| S2 | one-SE | N1a and N1b: R3 chosen; N2: R3 not chosen; N3: R2 chosen for `f` and `g` and its coefficients bit-equal to the fit with `r3: None` | as stated |
| L3 | LOO | N1a, N1b, N2 and ND's final pairs: `r3_loo` residuals against brute force in the test (R2 refit by the test's own normal-equation solve on `n - 1` pairs, the zero-mean process with the same hyperparameters and stated uncertainties by the test's own Gauss-Jordan solve) | `max_i abs(closed_i - brute_i) <= 1e-8 rms(r)`, `r` the in-sample R2 residual |
| L3s | — | the same fits: `sd_i` against brute force (`a_i` by the brute-force predictor applied to the `n` unit vectors, `sqrt(a_i^T C a_i)`) | `max_i abs(sd_i - sd_brute_i) <= 1e-8 rms(r)` |
| C1 | coverage | 2-sigma leave-one-out coverage of R3 on N1a, N1b and ND's final fit | `>= 0.85` each (nominal 0.954; three binomial sd at n = 40 is 0.10) |
| C2 | — | N1a: `sqrt(MSE_3 / MSE_2)`; N1b the same | `<= 0.2` (prototype 0.003 to 0.008); `<= 0.5` (prototype 0.24 to 0.31) |
| D1 | — | ND final correction: rung R3, trusted | as stated |
| D2 | — | ND: over the candidates not promoted, low ok, inside the final region, RMSE of `f_H - pred` of the final correction against the fit on the same pairs with `r3: None` | ratio `<= 0.5` |
| D3 | — | ND: ledger read back, 0 skipped lines; 300 low entries, 36 high, 36 pairs, 6 summaries; every summary's `fitted.hash` is FNV-1a 64 of its `settings.correction` text; rounds 4 to 6 record an `r3` object per fit | exact |
| D4 | — | ND at workers 1 and 8: ledger bytes | equal |
| X1 | — | ND final correction at `latin_hypercube(500, 2, 718_051)` scaled to the box: a flagged query's value is `y_L + b` (constant trusted) or `y_L` bit for bit; an unflagged one's is the R2 form plus `mu(u)` computed in the test from the fit's `l`, `sf2`, `beta` and the region points | exact (bit for bit for flagged; `<= 1e-12` relative for unflagged, the test's sum order may differ); at least 1 flagged |
| T6 | — | ND final correction: `to_json` then `from_json` predicts bit-equal at the 500 queries of X1; `hash` equal | exact |
| NC-C | — | N1a with `coverage_min = 1.01`: R3 chosen, untrusted, `why` names the coverage; `predict` inside the region returns `y_L` | as stated (the control must fail D1's trust) |
| K1 | — | `r3_log_likelihood` gradient against central differences (step 1e-5 in `theta`) at the N1b fit's `theta` and at two fixed points `(ln 0.3, ln 0.3, 0)`, `(ln 2, ln 0.1, ln 0.01)` | relative `<= 1e-5` per component (absolute 1e-7 when the component is below 1e-3 of the largest) |
| P6 | — | NP: every R3 number of every round's correction (`l`, `sf2`, `beta`, `mse`, `se`, `coverage`, `loglik`) and every coefficient, searched in the public ledger bytes as Rust `{}` text and as serde_json text, texts of at least 8 characters | 0 found; at least 50 texts searched |
| NC-P | — | NP's control (public source): the same search in its (single) ledger | at least 1 found (the control must fail P6) |
| K2 | ADR 0622 | scratch: a test that runs ADR 0622's S1 (A1 spec), S4 (noise) and S7-like private run (S1 with a private wrapper) and writes their ledgers, built once on `git archive d40cd665` and once on the F-718 tip with the same test file | every ledger byte-equal |
| K3 | ADR 0622 | `cargo test -p rcfd-api --release --test f699_fidelity` (the 16 fast tests) and the heavy record once (`--ignored --exact b_reduced_order_fan_heavy`); the file is not edited | every test passes, as in ADR 0622 addendum 1; the ignored set-up-defect records are not run |
| R | ADR 0622 R2 | `--lib design::`, `--test f698_surrogate`, `--test f697_fan_objective`, `--test fan_rom_bands_f268`, `--test f249_default_bits`, `--test f260_default_bits`, `--test f297_default_bits`, `--test baseline_hashes`; rcfd-mcp `default_f1`, `tool_list_f275`, `project_list_f270`; `git diff d40cd665 -- crates/rcfd-mcp crates/rcfd-api/src/design/surrogate.rs crates/rcfd-api/tests/f699_fidelity.rs` | pass; empty diff |
| F | — | `cargo fmt -p rcfd-api --check`, `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

## Negative controls and mutations (registered now)

Negative controls: NC-C (coverage guard), NC-P (privacy search), and the check S2 on N2 (no structure: the one-standard-error
rule must keep R3 out).

Mutations (scratch copy from `git archive` of the code commit, one at a time, the fast tests of the new file plus K2 for MR11):

| Id | Mutation | Expected to fail |
|---|---|---|
| MR1 | R3 chosen whenever it is available (one-standard-error rule bypassed for R3) | S1, S2 (N2, N3) |
| MR2 | closed form without the R2 refit term (`e3_i = (G r)_i / G_ii`) | L3 |
| MR3 | stated uncertainty left out of `C` (nugget = jitter only) | L3 (N1b, N2) |
| MR4 | `sd_i = 1 / sqrt(G_ii)` (no regression term) | L3s |
| MR5 | `mu(u)` not added in `predict` (R3 predicts as R2) | X1, D2 |
| MR6 | trace term of the likelihood gradient dropped | K1 |
| MR7 | R3 (the R2 form plus `mu`) applied to flagged queries | X1 |
| MR8 | the correction (with its `r3` objects) copied into the public summary's settings | P6 |
| MR9 | coverage condition removed from R3's trust | NC-C |
| MR11 | `TrustRule::default()` with `r3: Some(R3Rule::default())` | K2 |

## Decision rule

* **done:** S1, S2, L3, L3s, C1, C2, D1 to D4, X1, T6, K1, P6, K2, K3, R, F met; NC-C and NC-P fail as they must; every
  registered mutation (MR1 to MR9, MR11) caught by a check.
* **partial:** S1, S2, L3, C1 (the acceptance clauses) and K2, K3 met, and a non-acceptance check (C2, D1, D2, X1, T6, K1, P6,
  D3, D4) missed (an ignored `*_known_miss` record with the limit intact), or a mutation survives.
* **todo** (the lead does not take the code): an acceptance clause missed (S1, S2, L3, C1, K2 or K3).

## Limits (not covered)

* Library only; no MCP tool, case key or service method (F-702). No measured or CFD source: every check is synthetic.
* The process is fitted to the in-sample R2 residual by the zero-mean marginal likelihood, which ignores that `r = M y` lies in
  an `(n - p)`-dimensional subspace; the restricted likelihood (REML) or a joint generalised-least-squares fit of the regression
  (universal kriging) would treat the regression exactly. Not built (remaining question 2).
* The hyperparameters are not refitted per fold (the acceptance asks for it), so the leave-one-out error slightly flatters R3
  against R2, whose coefficients are refitted per fold; the one-standard-error rule is the guard.
* Squared exponential kernel only; one length scale per parameter; no per-metric coupling (each metric's process is its own).
* With `d` parameters R3 needs `10 d` pairs, so on the three-parameter fan of ADR 0622 (24 promotions) it is never available;
  ND uses two parameters and 36 promotions for that reason.
* The predictive standard deviation of R3 is computed for the leave-one-out coverage only; `predict` returns values, not
  variances (an acquisition rule using it belongs to F-698's `bayes`).

## References

* M. C. Kennedy and A. O'Hagan (2000), Predicting the output from a complex computer code when fast approximations are
  available, Biometrika 87, 1-13 (from memory; the autoregressive form only).
* C. E. Rasmussen and C. K. I. Williams (2006), Gaussian Processes for Machine Learning, MIT Press, chapters 2 and 5 (from
  memory; every formula used is derived above).
* O. Dubrule (1983), Cross validation of kriging in a unique neighborhood, Mathematical Geology 15, 687-699 (from memory; the
  block-inverse leave-one-out, derived above).
* T. Hastie, R. Tibshirani and J. Friedman (2009), The Elements of Statistical Learning, 2nd ed., section 7.10 (from memory; the
  one-standard-error rule, as already used by ADR 0622).
* D. A. Belsley, E. Kuh and R. E. Welsch (1980), Regression Diagnostics (from memory; the deletion formula `b - b_(-i)`,
  derived above from Sherman-Morrison).

## Addenda

Results, amendments and the roadmap entries go here; nothing above is changed after a run.
