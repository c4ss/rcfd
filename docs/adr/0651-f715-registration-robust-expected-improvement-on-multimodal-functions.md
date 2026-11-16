# ADR 0651: F-715 registration: a basin restart for expected improvement on multimodal functions, and a check that separates the imputation sign

Status: **REGISTERED 2026-11-16, written before any code of F-715 and before any run of its checks.** Written by the implementing
agent on branch `f715-work` (from local main d40cd665, which has F-698 / ADR 0621 merged), worktree `w49i-work/f715`, scratch
`w49i-work/f715-scratch`. Every choice the maintainer would normally be asked about is taken on the maintainer's standing
instruction of 2026-11-11 (take the recommendation). Not yet accepted by the maintainer.

Parent: [ADR 0621](0621-f698-surrogate-model-gaussian-process-and-expected-improvement.md) (F-698: `design::surrogate`,
`design::bayes`; its addendum 1, open questions 2 and 4; risk 10 and question 14 of its stub).

## What F-715 asks

The acceptance, binding:

> on Hartmann 6 (n0 30, budget 200, seeds 1 to 10) at least 9 of 10 runs within 1 percent of the minimum with hyperparameter averaging or a registered exploration restart, every F-698 check keeps its result, and a check with the failure classifier off separates imputation at m + s from m - s (the surviving mutation M6).

The brief adds: every limit not stated there is registered here before the run; the new behaviour is opt-in and F-698's results
and every existing test are unchanged; library only (no MCP tool, no golden); heavy runs are `#[ignore]` records run once; the
Hartmann 6 minimum stays labelled "from memory" in `docs/reference-data.md`.

## What was read

`AGENT_RULES.md`; the brief; F-715's roadmap entry; ADR 0621 (stub, amendments 1 and 2, addendum 1); ADR 0573 (the style of
the lead's section); `crates/rcfd-api/src/design/bayes.rs`, `surrogate.rs`, `cmaes.rs` (`sym_eigen`, `normal`),
`crates/rcfd-api/tests/f698_surrogate.rs`; `docs/reference-data.md` (the ADR 0621 table); `docs/design-loop.md`; the F-698 logs
`f698-scratch/run_o2o3.log` and `mut_M6.log` (read only). No paper was opened (none is in the repository). Citations below are
from memory and say so; nothing used depends on a number from a paper.

## Disclosed: a scratch prototype and its smoke runs (seeds 101 to 120 only), before this registration

As ADR 0621 amendment 1 did, the candidates were tried in a scratch copy (`f715-scratch/proto`, a `git archive` of d40cd665,
target `f715-scratch/target-proto`) with a scratch test `zz_f715_smoke.rs`; logs `f715-scratch/logs/smoke*.log`. Nothing of it
is in the branch before this commit. No seed of a registered check (1 to 20) was used.

| Smoke | Set-up | Result |
|---|---|---|
| S0 | Hartmann 6, n0 30, budget 200, plain EI (ADR 0621 defaults), seeds 101 to 110 | 8 of 10 (103 and 107 stuck at -3.20316); seeds 111 to 120: 5 of 10 (111, 112, 117, 118, 119 stuck) |
| S0d | diagnosis of 103, 107 | from evaluation about 55 every pick lies in the basin of -3.2032 with predicted sd about 0 and EI 5e-5 falling to 1.1e-6 at 190; the fitted length scales of x3 and x5 sit at 3 to 5 (the model treats those two coordinates as nearly irrelevant); EI never reaches the 1e-6-of-spread rule that ADR 0621 tried (why that rule "changed nothing") |
| S1 | hyperparameter averaging: EI averaged over the ML fit and 8 Laplace draws of the log length scales (Hessian by central differences of the analytic gradient, eigenvalues floored so that no sd exceeds 1 in log units), 101 to 110 | 7 of 10 (103, 106, 107 stuck): the likelihood is sharp at n of 50 to 200, the draws stay close to the ML fit, and the bias (long x3, x5 scales) is not a matter of spread |
| S2 | trigger "best EI below tau sd(y)" with tau 1e-3 or 1e-4, then one exploratory point: the candidate farthest from the data, or of largest predicted sd | 8 of 10 each (103, 107 stuck): the points land at the faces of the cube (values about 0) and EI goes back to the basin |
| S3 | same trigger, then the next point of a fresh Latin hypercube (103, 107 only) | 0 of 2 |
| S4 | same trigger, the incumbent's basin (ellipsoid of radius rho in the length scales) excluded, EI of the same model below the best value outside it (103, 107) | rho 1: 0 of 2; rho 2: 1 of 2; rho 3: 1 of 2 |
| S5 | as S4 but EI of a **fresh** process fitted only to the points outside the basin (the design D1 below), tau 1e-4 | rho 1: 1 of 2 (103, 107); rho 2: **10 of 10** on 101 to 110 (103 at 100, 107 at 172) and **10 of 10** on 111 to 120 (the five stuck seeds at 149 to 188); rho 3: 8 of 10 on 101 to 110 |
| S5p | S5 rho 2 with 200 perturbations around each of the 5 best points outside the basin added to the candidates | 10 of 10 and 10 of 10 (rescues at 140 to 181): no gain, not taken |
| S6 | Branin, n0 10, budget 60, seeds 101 to 110: plain; S5 rho 2 | 10 of 10 each, the same evaluation of first hit on every seed (the restart fires only after the target) |
| S7 | the F1 failure disc of ADR 0621, seeds 101 to 110: plain; S5 rho 2 (without the feasibility factor of D3) | 10 of 10 each; pooled failed share 0.026 and 0.054, worst 4 and 7 of 50 |
| S8 | the F1 disc with the classifier **off**: imputation at m + s; at m - s; imputation off | 9 of 10, pooled failed share 0.104 (one seed 38 of 50); 4 of 10, 0.666; 0 of 10, 0.836 |

Cost: a Hartmann 6 run of 200 is about 100 s on one thread (plain) and up to about 1.5 times that with the restart; ten seeds in
parallel took 380 to 660 s on the shared machine.

## Decision

### D1. The basin restart (opt-in `BayesOptions::restart: Option<Restart>`, default `None`)

`Restart { tau: 1e-4, rho: 2.0 }` (the defaults of the struct; the option itself is off by default). In `propose`, for each point
of the batch, after the plain pick of ADR 0621 (candidates, scores, Nelder-Mead refinement, winner score `a*`) and only when an
incumbent exists:

1. **Trigger**: `a* < tau * sd(f)`, with `sd(f)` the population standard deviation of the evaluated (`Ok`, merged) objective
   values; a NaN `a*` does not trigger. Rationale (first principles): EI is the expected gain of one evaluation; once the best
   expected gain anywhere is below a ten-thousandth of the spread of the values, the model believes nothing is left to gain at
   the resolution of the problem, which in S0d is exactly the state of the stuck runs from evaluation 55 on (EI 5e-5 to 1e-6,
   spread about 0.8). `tau = 1e-4` from S2/S5; 1e-3 gave the same counts in S2.
2. **The exhausted basin**: `u*` is the evaluated feasible point of lowest value (first in canonical order on ties); the basin
   is `delta(u) = sqrt(sum ((u_i - u*_i) / l_i)^2) < rho`, `l` the length scales of the objective process of this step. At
   `delta = 2` the Matern 5/2 correlation is `(1 + r + r^2/3) e^-r` with `r = 2 sqrt 5 = 4.47`: 0.14, so a point outside the
   ellipsoid is mostly informed by data other than the basin. `rho = 2` from S4/S5 (1 and 3 were worse).
3. **A fresh model**: the `Ok` points with `delta >= rho` (at least `d + 2`, else no restart) get their own process, fitted by
   `Gp::fit` with the step's options and the seed `mix(seed, n_total, 5)`; its incumbent `f'` is the lowest feasible value among
   them (none: no restart). Failed points are not imputed into this process (the failure model of item 4 carries them).
4. **The pick**: among the same candidates as the plain pick (the Latin hypercube and the perturbations), those with
   `delta < rho` or within the exclusion radius of an observation or an earlier pick score `-inf`; the score is
   `EI_fresh(u; f') * P_feasible(u)` with `P_feasible` the product of the constraint probabilities and of the failure model of the
   step (1 without them). The highest score wins, the first on ties; no Nelder-Mead refinement. A best score of `-inf`: no
   restart. The pick is recorded with kind `restart` (ledger provenance `"acquisition": "restart"`), its mean, sd and EI from the
   fresh process, the feasibility, `f'` as the incumbent and the objective process's length scales.
5. The batch believer and everything after the pick are unchanged (the believed point is the restart point).

This is a restart in the sense of the stub's question 14 (b): when expected improvement stops, the search starts again in the
rest of the box with a model that does not see the exhausted basin, keeping every evaluation. The idea of searching with the
incumbent region excluded is in the tabu and multi-start literature (for example Glover's tabu search, 1989, and the restarts of
trust-region Bayesian optimisation, Eriksson et al. 2019; both from memory, NOT read); the rule above is derived here and fixed
by the smoke, not taken from a source. Stateless: everything is a function of the canonical observation set, so ledger line
order, resume and worker count keep their properties.

Hyperparameter averaging (the other option the acceptance names) is not taken: S1 shows that a Laplace average does not move the
count. It stays as a remaining question.

### D2. What does not change

With `restart: None` the code path of `propose` is the one of ADR 0621 line for line (the new code is reached only through
`Some`); `Models::acquisition`, the believer, `minimise`, `run` and `surrogate.rs` are not edited. `PickKind` gains a variant
`Restart`. No existing test file is edited; no golden, hash table or `Cargo.lock` changes; no MCP tool or case key.

### D3. The imputation-sign check (M6 of ADR 0621)

With the failure classifier on, the classifier steers away from the failure disc by itself, so the sign of the imputation does
not show (ADR 0621 M6 survived; its mutation log: pooled failed share 0.131 with m - s against 0.030). With the classifier off and
imputation on, imputation is the only thing that keeps EI out of the disc: at `m + s` the failed points look worse than their
surroundings, at `m - s` they look better and EI returns to them (S8: 0.104 against 0.666 pooled).

## Checks (limits fixed now)

New test file `crates/rcfd-api/tests/f715_robust_ei.rs`. Release profile. Test functions, boxes and targets as ADR 0621
amendment 1 (Hartmann 6 on `[0, 1]^6`, minimum -3.32237 from memory, target `<= -3.32237 (1 - 0.01)`; Branin target `<=
0.397887 (1.01)`; the failure disc of F1). "Restart" means `restart: Some(Restart::default())`. E2 (in the box, no point within
1e-6 of an earlier one) applies to every loop below.

| Id | Clause | What | Limit | Estimate |
|---|---|---|---|---|
| A1 | acceptance 1 | Hartmann 6, `n0 = 30`, budget 200, seeds 1 to 10, `bayes::minimise` with the restart: seeds that reach the target; E2 | `>= 9 of 10`; 0 violations | S5: 20 of 20 on 101 to 120, every stuck seed rescued, the latest at 188 of 200 (thin: a rescue can come late) |
| A1r | record | A1: evaluation of first hit, best value, number of restart picks, per seed | recorded | |
| A2a | acceptance 2 | every non-ignored test of `f698_surrogate.rs`, unedited | pass | D2 |
| A2b | acceptance 2 | the F-698 O2 set-up (plain EI, n0 30, budget 200, seeds 1 to 10) run on the F-715 code: evaluation of first hit per seed | exactly `[48, -, 55, 41, 52, -, 54, 50, -, -]` (ADR 0621 addendum 1, `run_o2o3.log`) | D2 |
| A2c | acceptance 2 | `cargo test -p rcfd-api --release --lib design::`; `--test f697_fan_objective` | pass | nothing else calls `bayes` |
| A3 | acceptance 3 | the F1 set-up of ADR 0621 (Branin, disc of radius 0.15 failing, n0 10, budget 60, seeds 1 to 20) with `classify_failures: false`, `impute: true`: pooled failed share among evaluations 11 to 60; seeds that reach the target | `<= 0.30`; `>= 14 of 20` | S8: 0.104 and 9 of 10 at m + s, 0.666 and 4 of 10 at m - s |
| NC3 | control | A3 with `impute: false` too (nothing keeps EI out of the disc) | pooled failed share `> 0.30` | S8 0.836; ADR 0621 F2 0.862 |
| A4 | no harm | Branin, n0 10, budget 60, seeds 1 to 20, with the restart: seeds at the target; median evaluations to it (a miss counts 61) | `>= 18 of 20`; `<= 35` (ADR 0621 O1) | S6: identical first hits to plain |
| A5 | no harm | the F1 set-up with the restart (classifier on): seeds at the target; each seed's failed share | `>= 18 of 20`; `<= 0.25` (ADR 0621 F1) | S7: 10 of 10, worst 0.14 without the feasibility factor |
| A6 | design | every `restart` pick of the A4 and A5 loops (run through `propose` in the test): `delta >= rho` from the incumbent point of that step (computed in the test from the observations and the recorded length scales); its trigger: the restart fired at least once over A4 | 0 violations; `>= 1` restart pick | D1 |
| A7 | determinism | A4 seed 1 with workers 1 and 8; the A4 seed 1 observations after 60 evaluations shuffled (Rng 715_001) and `propose` with the restart | bit-equal; bit-equal | D1 stateless |
| A8 | ledger | `bayes::run` on a Branin `FnModel` with the restart, budget 60, seed 1: the entries whose pick kind is `restart` carry `"acquisition": "restart"`; their count equals the count of restart picks in `BayesResult::picks` | equal | D1 item 4 |
| B1 | rules | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings`; `git diff d40cd665 --stat` touches only `bayes.rs`, the new test, this ADR, `docs/design-loop.md`, `docs/reference-data.md` | clean; as stated | |

A1 and A2b are `#[ignore]`d heavy records (about 10 minutes and 3 minutes with ten threads on the shared machine), each run once.
A4 to A8 are the fast tests on the same code path (the restart fires on Branin after the target is reached, A6).

### Mutations (scratch copy, one at a time, after the registered runs)

| Id | Mutation | Expected to fail |
|---|---|---|
| R1 | the trigger never fires (`a* < tau sd` replaced by `false`) | A1 (S0: 6 of 10 on seeds 1 to 10 in ADR 0621), A6 count |
| R2 | the pick scored by the objective process of the step instead of the fresh one (the S4 variant) | A1 (S4: 1 of 2 at rho 2; doubt: the miss may be on fewer than two seeds) |
| R3 | the basin exclusion of candidates dropped (only the fresh model) | A6 |
| R4 | `restart` defaulting to `Some` | A2b (the plain O2 hit list changes) |
| M6 | imputation at `m - s` (ADR 0621 M6) | A3 |
| R5 | `delta` without the length scales (Euclidean in the unit cube) | A6 (the test measures `delta` in length scales) |

### Decision rule

* **done**: A1 to A8, NC3 and B1 met, and R1, R3, R4, R5 and M6 caught (R2 is a registered doubt).
* **partial**: A2a to A8, NC3 and B1 met and A1 missed (an ignored `a1_..._known_miss` record with the limit intact and the
  measured count), or a mutation other than R2 survives.
* **todo**: A2a, A2b, A2c or A3 missed (an F-698 result changed, or the M6 separation not shown).

## Limits (not covered)

* One test function carries the claim (Hartmann 6, ten seeds); Branin and the failure disc only show no harm. No claim on other
  multimodal functions or on the fan; on the reduced-order fan (ADR 0621 R1) the restart is not run.
* No hyperparameter averaging (S1 did not help); no lower-confidence-bound or generalised EI.
* The restart picks are not refined by Nelder-Mead; failed points are not imputed into the fresh process; the trigger compares
  with the spread of the evaluated values, so a nearly constant objective never triggers.
* `tau` and `rho` are fixed from smoke runs on Hartmann 6 itself (seeds 101 to 120); they are not derived for other functions.
* Library only: no study method, MCP tool, case key or golden.

## Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

1. Averaging or restart: the restart (S5 against S1).
2. Default on or off: off (the acceptance and the brief: opt-in, F-698 unchanged); a change of default would change F-698's
   recorded results.
3. Perturbations around the best points outside the basin: not taken (S5p: no gain).

## Amendment 1 (2026-11-16, after the registered run of the fast tests, before any corrected run): A8 was vacuous

The registered run of the fast tests (commit ed5e4bbf, log `f715-scratch/logs/run_fast.log`) met A3 to A8. A8 is met only
vacuously: `bayes::run` with seed 1 made no restart pick within 60 evaluations (0 picks, 0 entries), so it does not exercise the
ledger path of D1 item 4. The same log shows that seed 1 of A4 has no restart pick either, and seed 2 has 10. The set-up was
defective (a seed without a restart), not the limit.

Correction (registered now): a test `a8b_restart_picks_in_the_ledger_provenance_seed2`, the A8 set-up with seed 2, run once,
with two limits: the count of restart picks in `BayesResult::picks` equals the count of entries with `"acquisition":
"restart"`, and that count is at least 1. A8 stays as it is (met, recorded as vacuous). The decision rule is unchanged with A8b
in the place of A8.

## Amendment 2 (2026-11-16, after the mutation runs, before any corrected run): A2b cannot see the default of `restart`

Mutation R4 (`restart` defaulting to `Some`) survived A2b (log `f715-scratch/logs/mut_R4.log`: the hit list unchanged). Cause, from
the test code: A2b builds its options with `restart: restart.then(Restart::default)`, i.e. an explicit `None`, so it never reads
the default. A supplementary run of the unedited F-698 fast tests under R4 (`mut_R4_f698.log`) also passes: they check limits,
not bits, and the restart does not move a limit on Branin, the disc, the noise or the constraint. The set-up of A2b is defective
for the clause "defaults unchanged"; A2b's own result (the plain O2 hit list) stands.

Correction (registered now): a fast test `a2d_default_options_are_the_plain_ei_of_adr_0621`: (1)
`BayesOptions::default().restart` is `None`; (2) Branin, n0 10, budget 60, seeds 1 and 2 (seed 2 is a seed on which the restart
fires, A4), `bayes::minimise` with `BayesOptions { seed, n0: Some(10), ..Default::default() }` and with the same plus an explicit
`restart: None`: every evaluated point bit-equal, and different from the run with `restart: Some(Restart::default())` on seed 2
(so the check can fail). Run once; then R4 is run once against it. The decision rule is unchanged with A2b and A2d together in
the place of A2b for R4.
