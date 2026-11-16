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

## Amendment 1 (2026-11-16, after the registered run, before the corrected runs)

The registered run (`cargo test -p rcfd-api --release --test f718_fidelity_r3 -- --test-threads=2 --nocapture`, code commit
70561b19, log `f718-scratch/run1.log`): K1, NC-C, P6 and NC-P passed; three tests failed. Diagnosed with a scratch copy and one
diagnostic test (`f718-scratch/mcopy/crates/rcfd-api/tests/zz_f718_diag.rs`, never committed, log `diag1.log`):

1. **S2 on N3 missed (a design gap).** On the exact affine N3 the R2 residual is rounding only (MSE 2.2e-30 for `f`); R3 is
   available, D2 scales the residual to unit rms, the process fits the rounding pattern and its leave-one-out MSE (1.6e-30) is
   more than one standard error below R2's, so the rule (S1 met: the choice is the one-standard-error rule's) picks R3. The
   registered design had no floor below which a residual is rounding and not a discrepancy. **Design change:** R3 is not
   available, with the reason `the R2 residual is at the rounding level`, when `rms(r) <= 1e-10 rms(y_H)`. Derivation: the
   rounding of a least-squares residual is about `eps cond(X) rms(y_H)`; `eps = 2.2e-16` and `cond(X)` up to about `1e4` for the
   ladder's columns (`y_L`, 1, unit-box parameters) give about `2e-12`, so `1e-10` is 50 times above rounding and far below any
   discrepancy a driver should correct. Fixed now, not searched.
2. **L3s missed on the noise-free fits (N1a: 6.3e-8 of rms(r); ND final: 1.63e-8), limit 1e-8.** The L3 residuals met their limit
   (2.5e-10 and 1.7e-9). Diagnosis: two independent brute-force references (Gauss-Jordan and Cholesky) of the conditional
   variance agree to 2.0e-10 of rms(r), so the reference solves are not the cause; the leave-one-out sd in the noise-free case is
   small (smallest 7.0e-4 of rms(r)) while the quadratic form `a_i^T C a_i` is a sum of terms of order `|a_i|^2 s_f^2` (the
   correlation matrix has condition about 2e9 at jitter 1e-8), so both evaluations lose about 1e-4 of the sd's relative accuracy to
   cancellation. With noise (N1b, diagnostic) the two agree to 1.3e-14. The limit stays; the two misses are recorded.
3. **Test set-up defect.** The N1a/N1b/N2 test asserted L3s for N1a before N1b and N2 were evaluated, and the ND test asserted
   L3s before D2, X1 and T6, so those checks did not run. Corrected set-up: one test per case, every number computed and printed
   before any assertion, the same limits.

The three original tests stay, ignored, with their names suffixed `_known_miss` and the measured numbers in the ignore reason.
New tests, run once each, limits unchanged: `n1a_corrected` (S1, S2, L3, C1, C2; L3s recorded, its miss is the record above),
`n1b_corrected` and `n2_corrected` (S1, S2, L3, L3s, and C1, C2 for N1b), `n3_corrected` (the design change: S1, S2 with R3
unavailable for the rounding reason, R2 coefficients bit-equal), `nd_corrected` (D1 to D4, S1 per round, L3, C1, D2, X1, T6; L3s
recorded). The floor changes no other registered set-up (their residuals are 1e-2 to 1e-1 of `y_H`). K2, K3, R, F and the
mutations run on the code with the floor.

Decision rule, as registered, applied to the corrected runs; the S2 miss of the registered design and the L3s misses are reported
with the decision, and with the L3s misses the decision is at most partial (L3s is a non-acceptance check).

## Amendment 2 (2026-11-16, after the corrected runs of amendment 1, before a second corrected ND run)

The corrected run (code 57df8d5c, log `f718-scratch/run2.log`): `n1a_corrected`, `n1b_corrected`, `n2_corrected`,
`n3_corrected` and the earlier passing tests passed; `nd_corrected` failed on X1's "at least 1 flagged": **0 of 500** queries
were flagged. Set-up defect: with 36 pairs spread by the explore picks over a two-parameter box, `h_max = 0.25` and margin 0.05,
the calibrated region covers the whole box, so a query inside the box is never flagged and X1 could not test the flagged
branch; T6, after X1 in the same test, did not run. Corrected set-up: X1 and T6 use the 500 registered queries plus 100 queries
at `latin_hypercube(100, 2, 718_052)` with `x1` scaled to [1.1, 1.5] (outside the box beyond the margin; `predict` takes any
point) and `x2` over its box. Limits unchanged (flagged values bit for bit, unflagged within 1e-12 relative, at least 1
flagged; round trip bit-equal). `nd_corrected` stays as the ignored record `nd_corrected_known_miss`; the new test is
`nd_corrected2`, run once.

## Addendum 1: results (2026-11-16)

Code: 70561b19 (R3, the registered tests), 57df8d5c (amendment 1: the rounding floor, the corrected tests), 8bd0c6a7 (amendment
2: X1's outside queries); registration 901bd957 and amendments 75884136 and 8bd0c6a7 before their runs. Library change only in
`crates/rcfd-api/src/design/fidelity.rs` (`TrustRule::r3`, `R3Rule`, `Rung::R3`, `MetricFit::r3` and `chosen_mse`, `R3Fit`,
`R3Loo`, `r3_loo`, `r3_log_likelihood`, `R3_JITTER`; `apply` and the summary's `rmse_after` take R3); `surrogate.rs` is used, not
edited. New `crates/rcfd-api/tests/f718_fidelity_r3.rs` (12 tests: 8 run, 4 ignored records). Docs: `docs/design-loop.md` (one
bullet). No shared solver file, golden, hash table, case key or MCP tool.

Runs (release profile throughout): the registered run `run1.log` (3 of 6 passed, amendment 1); the corrected tests once
(`run2.log`; `nd_corrected` failed on X1, amendment 2); `nd_corrected2` once (`run3.log`). The prototype numbers of the
registration came from numpy and other seeds; none is a check.

| Check | Limit | Measured | Met |
|---|---|---|---|
| S1 one-SE (N1a, N1b, N2, N3, ND rounds 1 to 6) | rung = the rule recomputed in the test | equal in every fit (N1a R3, N1b R3, N2 R2, N3 R2 and R2, ND R2, R0, R2, R3, R3, R3) | yes |
| S2 N1a, N1b: R3 chosen | as stated | R3, R3 | yes |
| S2 N2: R3 not chosen | as stated | R2 (R3 MSE equal to R2's, process variance at its lower bound) | yes |
| S2 N3, registered design | R2, coefficients bit-equal to `r3: None` | `f`: **R3** (MSE R2 2.2e-30, R3 1.6e-30, rounding) | **no** (amendment 1; record `n3_exact_affine_keeps_r2_known_miss`) |
| S2 N3, with the floor (`n3_corrected`) | R2, bit-equal, R3 unavailable for rounding | R2 and R2, bit-equal, "the R2 residual is at the rounding level" | yes |
| L3 closed vs brute force | `<= 1e-8 rms(r)` | N1a 2.5e-10, N1b 4.0e-14, N2 1.2e-13, ND final 1.7e-9 | yes |
| L3s sd vs brute force | `<= 1e-8 rms(r)` | N1b 1.3e-14, N2 1.1e-15; **N1a 6.3e-8, ND final 1.6e-8** (noise-free, cancellation in `a^T C a`, amendment 1) | **no** for the two noise-free fits (records `n1_n2_choice_loo_and_coverage_known_miss`, `nd_driver_with_r3_known_miss`) |
| C1 coverage | `>= 0.85` | N1a 1.000, N1b 0.950, ND final 1.000 | yes |
| C2 RMSE ratio R3 / R2 | N1a `<= 0.2`, N1b `<= 0.5` | 0.0057, 0.274 | yes |
| D1 ND final | R3, trusted | R3, trusted (R3 from round 4) | yes |
| D2 held-out ratio | `<= 0.5` | 0.078 (264 candidates inside the region; RMSE 1.02e-2 against 1.31e-1 for the R2 fit) | yes |
| D3 ledger | counts, hashes, `r3` objects | 300, 36, 36, 6; 0 skipped; hashes equal; `r3` in every round, available from round 4 | yes |
| D4 workers 1 and 8 | bytes equal | 244740 bytes, equal | yes |
| X1, registered queries | at least 1 flagged | **0 of 500** (the region covers the box) | set-up defect (amendment 2; record `nd_corrected_known_miss`) |
| X1, corrected queries | flagged bit for bit; unflagged `<= 1e-12`; at least 1 flagged | 100 of 600 flagged, exact; unflagged 3.8e-16 | yes |
| T6 round trip | bit-equal, hash equal | 600 of 600, `fnv1a64:11217d881a24ee8b` | yes |
| NC-C | R3 chosen, untrusted, why names coverage | "leave-one-out 2-sigma coverage 1.000 below 1.01"; predictions = `y_L` | yes (fails trust as it must) |
| K1 gradient | rel `<= 1e-5` | largest 4.6e-8 | yes |
| P6 | 0 found, >= 50 texts | 0 of 128 in 163967 public bytes | yes |
| NC-P | >= 1 found | 128 of 128 | yes (fails P6 as it must) |
| K2 bytes at d40cd665 and at the tip | equal | S1 195021, S4 251074, S7 public 130872, private 201617 bytes: all equal | yes |
| K3 ADR 0622 checks | pass | `f699_fidelity` 16 passed (3 ignored records); heavy `b_reduced_order_fan_heavy` passed (215.9 s) | yes |
| R existing tests and diff | pass; empty | `design::` 76, `f698_surrogate` 19, `f697_fan_objective` 22, `fan_rom_bands_f268` 4, `f249_default_bits` 4, `f260_default_bits` 1, `f297_default_bits` 3, `baseline_hashes` 3; rcfd-mcp `default_f1` 2, `tool_list_f275` 7, `project_list_f270` 1; diff of rcfd-mcp, `surrogate.rs`, `f699_fidelity.rs` against d40cd665 empty | yes |
| F fmt, clippy | clean | `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean | yes |

Mutations (scratch copy `f718-scratch/mcopy` from `git archive 8bd0c6a7`, script `mutate.py`, runner `mut_run.sh`, the 8 fast
tests of the new file plus K2 for the control and MR11; the unmutated copy passed 8 of 8 and K2):

| Id | Expected | Caught by |
|---|---|---|
| MR1 R3 always when available | S1, S2 | S1 on N2 (`n2_corrected`) |
| MR2 no refit term | L3 | L3 on N1a, N1b, N2 and ND |
| MR3 nugget left out | L3 | L3 on N1b and N2 |
| MR4 sd without the regression term | L3s | L3s on N1b and N2 |
| MR5 `mu` not added | X1, D2 | D2 (X1 follows it in the test) |
| MR6 trace term dropped | K1 | K1 |
| MR7 R3 on flagged queries | X1 | X1 (flagged) |
| MR8 correction in the public summary | P6 | P6 |
| MR9 coverage condition removed | NC-C | NC-C |
| MR11 R3 on by default | K2 | K2 (S1, S4 and the S7 private ledger differ) and D2 (its R2-only reference becomes R3) |

Ten of ten caught.

Deviations: amendments 1 and 2 (above). `R3Fit` has no `se_levels` field (D5 listed one); the test recomputes every level's MSE
and SE from the public leave-one-out functions instead, which is the stronger form of S1. `nd_corrected2` keeps the original
order of assertions except L3s (amendment 1 said every number is printed before any assertion; for ND only the L3s assertion
was removed, and every later check ran because the earlier ones passed). The record `n3_exact_affine_keeps_r2_known_miss`
would now pass, since the floor makes R3 unavailable there; it is kept as the record of the registered design. Not run: the
whole workspace (the lead's gate).

Decision by the registered rule: **partial**. The acceptance clauses are met on the corrected runs (S1 in every fit, L3 within
1e-8 of rms(r) in every fit, C1 coverage 0.95 to 1.00 against 0.85, K2 bytes equal, K3 every ADR 0622 check keeps its result),
every non-acceptance check is met except L3s on the two noise-free fits, and every mutation is caught. L3s is missed (a
non-acceptance check), and the registered design itself missed S2 on N3 before the rounding floor of amendment 1, so the item is
not reported as done.

### Remaining questions (each with a recommendation)

1. **Where the zero-mean process lives.** F-698's `Gp` profiles a constant mean and takes a nugget ratio, so R3 builds its own
   zero-mean process with an absolute nugget in `fidelity.rs` from F-698's kernel and factorisation. Options: (a) keep; (b) after
   F-715 merges, add a zero-mean, absolute-noise mode to `surrogate::Gp` and let R3 call it. Recommendation: (b), as a small
   follow-up, so there is one Gaussian-process implementation.
2. **Regression treated by REML or universal kriging.** The process is fitted to the OLS residual with the plain zero-mean
   likelihood. Options: (a) keep (the acceptance's form); (b) the restricted likelihood. Recommendation: (a) until a real
   discrepancy shows biased length scales.
3. **L3s on noise-free fits.** The sd of a near-interpolating process loses about 1e-4 of its relative accuracy to cancellation
   in `a^T C a`. Options: (a) accept and keep the records; (b) a follow-up that evaluates the variance through the factor
   (`|L^T a|^2`) and checks it, registered before its run. Recommendation: (b); coverage is unaffected at these sizes.
4. **R3 on the reduced-order fan.** With three parameters R3 needs 30 pairs, more than the 24 promotions of ADR 0622's fan
   set-up. Options: (a) leave as is; (b) a follow-up run of S3 with 6 rounds. Recommendation: (a) until the CFD tier exists.

## For the lead: roadmap entries

> Proposed status: partial

> Follow-up of F-699 (ADR 0622, 2026-11-15). ADR 0652, 2026-11-16. PARTIAL (every acceptance clause met on the corrected runs; the leave-one-out standard deviation misses 1e-8 on the two noise-free fits). Library only, opt-in (TrustRule::r3, default None; no case key, no MCP tool): rung R3 of the fidelity driver is R2 plus a zero-mean Gaussian process (F-698's squared-exponential kernel and Cholesky factorisation) on the R2 residual, with the nugget from the high source's stated uncertainty, length scales and process variance fitted once per fit by maximum likelihood (analytic gradient, F-698's multi-start pattern) and held in every fold, and leave-one-out residuals from a closed form that includes the refit of the R2 coefficients, e3_i = ((G r)_i + e2_i (G P)_ii) / G_ii; R3 needs 10 d pairs and a residual above the rounding level, is chosen over R2 only by the one-standard-error rule (recomputed by the test in every fit), and is trusted only with 2-sigma leave-one-out coverage at least 0.8. Checks: leave-one-out residuals equal brute force (R2 refitted, the process re-solved) within 2.5e-10 to 4e-14 of the residual scale; coverage 0.95 to 1.00 against the registered 0.85 on a smooth nonlinear synthetic discrepancy with and without noise; R3 chosen on it (RMSE 0.006 and 0.27 of R2's), not chosen on unstructured noise or an exact affine source; through the driver (two parameters, 300 candidates, 36 promotions) R3 is chosen from round 4 and its held-out RMSE is 0.078 of the R2 fit's; flagged queries get the constant only, bit for bit; the correction round-trips bit for bit; no R3 number reaches the public ledger with a private source; with R3 off the S1, S4 and private S7 ledgers are byte-identical to before, and every ADR 0622 check (including the heavy fan record) keeps its result. Ten of ten mutations caught. Misses recorded: the registered design chose R3 on a rounding-level residual (fixed by a floor in amendment 1), and the predictive sd differs from brute force by 1.6e-8 and 6.3e-8 of the residual scale on noise-free fits (cancellation, limit 1e-8). Verified on synthetic sources only.

> Follow-up proposed: Fidelity driver R3: the leave-one-out standard deviation through the Cholesky factor. Deps F-718. Acceptance: the predictive standard deviation of R3 is evaluated without the cancellation of a^T C a, it equals brute force within 1e-8 of the residual scale on the noise-free fits of ADR 0652 (N1a and the driver's final fit), and every ADR 0652 and ADR 0622 check keeps its result.

> Follow-up proposed: A zero-mean, absolute-noise mode of F-698's Gaussian process used by R3. Deps F-718, F-715. Acceptance: surrogate::Gp gains an opt-in zero-mean mode with an absolute per-observation noise variance, R3 calls it in place of its own process, every ADR 0652 check keeps its result (fits bit-identical or within 1e-12), and every ADR 0621 check keeps its result.
