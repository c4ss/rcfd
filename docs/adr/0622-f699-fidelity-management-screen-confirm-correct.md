# ADR 0622: F-699 proposal: fidelity management (H7): screen with the reduced-order fan model, confirm with a higher tier, log the disagreement and correct the screening

Status: **REGISTERED 2026-11-16 in amendment 1 (at the end), written before any code of F-699 and before any run of its
checks.** Not yet accepted by the maintainer. Amendment 1 replaces the section "Registration (to be fixed before any code)"; the
rest of this text is the stub as drafted. Stub status: **DRAFT STUB 2026-11-15. Not registered, not accepted.** The maintainer answered the open questions on 2026-11-15 (section \"Maintainer decisions\"); the limits are still proposals until the implementing agent registers them. No code of F-699 exists and no check has run. Nothing in
this text is a result. Every number marked "proposed" is a starting value for the registration and may be changed until the
registration is written; after that the house rule applies (limits fixed before the run, a miss becomes an ignored
`*_known_miss` test with the limit intact). The open questions at the end are for the maintainer and are not taken here.

Roadmap item F-699 (deps F-268, F-697), part H7 of `docs/fan-project-breakdown.md`: "Fidelity management: use C7 to screen,
CFD to confirm, with the disagreement between them logged and used to correct C7". Written against main 97740ded, read only.
Drafted in parallel with ADR 0620 (F-697, objective definition) and ADR 0621 (F-698, surrogate); neither was available when
this was written, so every reference to them is an assumption to be checked when they exist.

House rules that apply: clean-room (published literature and first principles only); defaults bit-identical (everything here
is opt-in, `crates/rcfd-api/tests/baseline_hashes.rs` and the golden files are not edited); no new dependency, no unsafe code;
no case-file key and no MCP tool-list change without the maintainer's decision (Q1, Q2, Q8).

## Context

What the plan asks. The fan plan has three evaluators of very different cost and trust: the reduced-order tier C7 (a fan
curve in milliseconds), CFD, and a measured fan. H7 joins them: screen many variants with C7, confirm a few with a better
source, record how far the two were apart, and use that record so the next screening is less wrong.

What exists (checked in the source):

* `rcfd_post::fan_rom` (ADR 0119): blade-element strips with loss constants. `RomModel` holds `cl_max`, `cd_normal`,
  `annulus_drag`, `secondary_drag` and `tip_cd`, each marked ASSUMED or UNVERIFIED in the code. These are the constants a
  comparison with a better source would correct.
* `design::fan_rom_eval::FanRomEvaluator`: the study evaluator on that model (metrics `q`, `dp_s`, `dp_t`, `power`, `torque`,
  `eta_s`, `eta_t`, `tone1_pa` at the operating point against a system curve). Not wired into any service tool.
* `design::ledger` (ADR 0121): JSON-lines ledger, `Entry` with `kind` (a free string: `evaluation`, `uq_summary`,
  `sobol_summary`), `fidelity` (`analytic`, `reduced_order`, `cfd`), verdicts, units, `provenance`. `ledger::Model` (id,
  version, tier, `evaluate(&[Quantity]) -> Outcome`) and `FnModel` for closures. Pareto and best-so-far queries refuse a set
  that mixes tiers, so nothing today compares two tiers of one variant.
* `design::uq`, `design::sensitivity` (ADR 0121, 0240): drivers over a `Model` that log to the ledger, bit-identical for any
  worker count. F-699 follows their pattern.
* `design::sampling` (seeded Latin hypercube, Sobol), `design::pareto`, `design::study::parallel_map`.
* F-218 (actuator disk) and F-113 (lumped fan curve) are the other C7 pieces; they need a flow solve and are not used by the
  checks below.

Why the high tier cannot be CFD today. `docs/milestone-fan.md`: the FAN-01 sector stalls in a limit cycle (ADR 0110 to 0181),
its error bands are not shown credible (ADR 0086), and the one comparison with measurement missed (ADR 0075, 0078). A driver
whose acceptance needed a converged fan CFD run could not be accepted. So the driver is written against an abstract
high-fidelity source and verified with synthetic ones. A source that fails at every point is one of the registered cases,
because that is what the CFD tier does now.

Why measured data shapes the interface now. `docs/fan-project-breakdown.md` (limit 1) and `docs/fan-measurement-protocol.md`:
a measured fan is the most valuable input, and the maintainer plans a printed prototype on a rig. That data is proprietary. It
must not enter the repository, and only aggregate calibration residuals may be published. If the privacy boundary is added
after the driver exists, the ledger format will already have leaked the wrong fields, so the boundary is part of this design.

## Decision proposal

### D1. Where and what (opt-in, library only)

New module `crates/rcfd-api/src/design/fidelity.rs` with `fidelity_tests.rs`. One entry point,
`design::fidelity::run(spec, low, high, ledger) -> Result<FidelityResult, String>`. No existing path calls it. No service
method, no MCP tool, no case-file key (Q1).

* `low: &dyn ledger::Model` of tier `reduced_order` or `analytic`.
* `high: &dyn HighSource`, a new trait:
  * `id()`, `version()`, `tier()`;
  * `private() -> bool` (default false);
  * `observe(params: &[Quantity]) -> Observation`, where `Observation` is a `ledger::Outcome` plus an optional standard
    uncertainty per metric (0 for a deterministic model, the rig's stated uncertainty for a measurement);
  * `available(params) -> bool` (default true): a measured source can only answer for variants that were built and tested,
    so the driver must be able to ask before it spends a promotion.
* Every `ledger::Model` is a `HighSource` through an adapter (public, zero uncertainty, always available). This is how the
  synthetic sources of the checks and a future CFD evaluator plug in.
* `spec`: the candidate set (a list, or bounds plus a seeded Latin hypercube of `n_screen` points), the metrics to correct,
  the ranking objective (from F-697, D3), `rounds`, `promote_per_round`, the seed, the correction options (D4, D5) and the
  privacy options (D7).

### D2. The loop

For round `r = 1..rounds`:

1. Screen: evaluate every candidate not yet screened with `low` (`parallel_map`, entries appended in candidate order from the
   calling thread, as `uq::run` does). A failed low evaluation is logged with its verdict and can never be promoted.
2. Predict: `y_c(x) = correct(y_L(x), x)` with the current correction, or `y_c = y_L` when no correction is trusted (D5).
3. Promote `promote_per_round` candidates (D3) and evaluate them with `high`.
4. Log one `fidelity_pair` entry per promoted candidate and refit the correction on all pairs so far (D4).
5. Log one `fidelity_summary` entry (D6).

The driver stops early, without an error, when no candidate is available for promotion.

### D3. Promotion rule (proposed)

Per round, `k_exploit + k_explore = promote_per_round` (proposed split 4 + 2 of 6):

* **Exploit.** The `k_exploit` best candidates by the corrected prediction of the ranking objective that have no high
  result yet. The objective is whatever ADR 0620 defines. This draft assumes a single scalar with a sense (min or max) and
  constraint verdicts; if ADR 0620 defines a Pareto set instead, exploit picks from the non-dominated set of the corrected
  predictions (`pareto::front`), widest gaps first (Q6).
* **Explore.** The `k_explore` candidates that lie farthest, in parameters scaled to the unit box, from every candidate that
  already has a high result (greedy max-min distance). This is what lets the correction be fitted and checked away from the
  current best; without it the correction is only ever calibrated where the uncorrected model already pointed.
* Round 1 has no correction, so its exploit picks use the uncorrected ranking.
* Ties are broken by candidate index, so the choice is deterministic.
* A promoted candidate whose high evaluation fails (`diverged`, `constraint_rejected`, `not_evaluated`) is logged, is not
  used in the fit, and still consumes its promotion (Q11).

Expected-improvement and other acquisition rules that need a predictive variance belong to F-698 and are not proposed here.

### D4. Discrepancy model

Per corrected metric `m`, with pairs `(x_i, y_L,i, y_H,i)`, `i = 1..n`, and `d` design parameters, a ladder of four rungs:

| Rung | Form | Fit | Smallest n (proposed) |
|---|---|---|---|
| R0 | `y_c = y_L` | none | 0 |
| R1 | `y_c = y_L + b` (additive) or `y_c = a y_L` (multiplicative) | mean of `y_H - y_L`, or least squares through the origin | 3 |
| R2 | `y_c = rho y_L + b_0 + b . x` | linear least squares | `d + 4` |
| R3 | `y_c = rho y_L + b_0 + b . x + delta(x)`, `delta` a zero-mean Gaussian process on the R2 residual | R2, then the process | `10 d` |

Notes on the form (first principles and the cited literature):

* R3 is the autoregressive model of Kennedy and O'Hagan (2000), `y_H(x) = rho y_L(x) + delta(x)`. Their co-kriging also
  models the low code with a Gaussian process because the low code is expensive there. Here the low model costs milliseconds
  and is evaluated at every query point, so only the discrepancy needs a statistical model. That removes the joint
  covariance and leaves a regression plus one Gaussian process on its residual.
* The process uses a squared-exponential kernel with one length scale per parameter and a nugget equal to the source's
  stated variance (zero uncertainty gives a small jitter, value to be registered). Hyperparameters are found by maximising
  the likelihood from a fixed, seeded set of starts. If ADR 0621 delivers a Gaussian process, R3 uses it and adds none (Q3).
* R1 additive or multiplicative: both are fitted and the one with the smaller leave-one-out error is kept (Q4).
* Rung choice: among the rungs that `n` allows, the lowest rung whose leave-one-out RMSE is within one standard error of the
  smallest (the one-standard-error rule, Hastie, Tibshirani and Friedman 2009). The simpler model wins unless the richer one
  is clearly better, which is the guard against overfitting a handful of expensive points.
* Leave-one-out errors of R1 and R2 come from the closed form `e_i / (1 - h_ii)` of linear least squares. For R3 they come
  from the closed form of Gaussian-process regression at fixed hyperparameters (Rasmussen and Williams 2006, section 5.4.2);
  the hyperparameters are not refitted per fold, and the ADR will say so in the result.
* Each metric is corrected on its own. Coupled metrics (for example `power = torque x omega`) may become inconsistent after
  correction; the summary reports the largest violation of each identity the caller declares, and nothing is enforced.
* What is not proposed: calibrating the loss constants of `RomModel` themselves (Q13).

### D5. When a correction is trusted

A fitted correction is applied to screening only when all of these hold; otherwise screening uses R0 and the summary says
which condition failed:

1. `n` reaches the smallest n of the rung.
2. It helps on held-out points: leave-one-out RMSE of the corrected prediction is at most `f_trust` times the RMSE of the
   uncorrected one (proposed `f_trust = 0.8`).
3. It is honest about its error (R3 only): the fraction of leave-one-out residuals within two predictive standard
   deviations is at least a registered value (proposed 0.8).
4. The query is inside the calibrated region. The region is the bounding box, in parameters scaled to the unit box, of the
   candidates with a high result, grown by a registered margin (proposed 0.05), and additionally the query must be within a
   registered distance `h_max` of the nearest high result (proposed 0.25). Outside it the driver applies the R1 correction
   only, if R1 is trusted by conditions 1 and 2, and flags the prediction `extrapolated`. A linear or Gaussian-process
   correction is never extrapolated.
5. The evaluator ids and versions match: a correction fitted against low model version A or high source version B is
   refused, with a located message, for any other version (the rule of `ledger::rerun_check`).

Trust is decided per metric and per round, and can be withdrawn in a later round.

### D6. How the disagreement is reported

In the ledger, with `kind` values that existing readers already tolerate (the field is a free string):

* `evaluation` entries for every low and every high evaluation, as today, tier and evaluator on each.
* `fidelity_pair`, one per promoted candidate: `params`; per metric `<m>.low`, `<m>.high`, `<m>.diff` (`high - low`),
  `<m>.rel` (`diff / max(|high|, floor)`, floor registered per metric) and `<m>.pred` (the corrected prediction made before
  the high result was known, so `<m>.high - <m>.pred` is an honest out-of-sample error); provenance `{driver: fidelity,
  round, reason: exploit | explore, low_entry, high_entry}`.
* `fidelity_summary`, one per round: per metric `n_pairs`, `n_failed_high`, the rung and form, `rho` and coefficients, the
  leave-one-out RMSE before and after correction, the coverage of condition 3, the mean and largest absolute `diff` and
  `rel`, `trusted` (and the failed condition), the count of candidates flagged `extrapolated`, and the running out-of-sample
  RMSE of `high - pred` over all rounds. Provenance holds the full spec.

`FidelityResult` returns the same content plus the fitted correction as a value that can be saved and reapplied
(`Correction::to_json` / `from_json`, with the evaluator ids and versions and the calibrated region inside it).

The out-of-sample error `high - pred` is the number H6 (the long-horizon loop) should read: it says whether the tiers are
getting closer, in data the correction had not seen.

### D7. The measured-data hook and its privacy boundary

A measured source is a `HighSource` with `private() = true` and tier `measured` (Q2). The proposed reader,
`MeasuredTable`, takes the path of a table outside the repository, in the CSV layout of the "Data delivery" section of
`docs/fan-measurement-protocol.md`, plus a caller-supplied map from a candidate's parameters to a row (the variant id of the
printed prototype, the speed, the system curve). It reports the stated uncertainty of each column and is `available` only
for variants present in the table. No reader of any vendor or rig file format is proposed.

When the high source is private, the driver splits its output:

| Content | Public ledger (may be committed) | Private ledger (outside the repository) |
|---|---|---|
| Low `evaluation` entries, uncorrected | yes | yes |
| High `evaluation` entries (the measured values) | no | yes |
| `fidelity_pair` entries | no | yes |
| Fitted correction (coefficients, process data) | no | yes |
| Corrected predictions and the ranking by them | no | yes |
| `fidelity_summary` | aggregates only, see below | full |

* Public aggregates per metric: `n_pairs`, the rung name, RMSE before and after correction, mean bias, largest absolute
  relative residual, coverage, `trusted`. They are written only when `n_pairs >= n_pub` (proposed 5), because an aggregate
  over fewer points gives the points away (with one pair the RMSE is the measured difference). Aggregates are rounded to a
  registered number of significant digits (proposed 3).
* Why the correction and the corrected predictions are private too: `y_c - y_L` evaluated at a measured variant is the
  measured difference, and an interpolating Gaussian process returns the measured values exactly. Publishing either
  publishes the data.
* The private path is given by the caller on every call. The driver refuses, with a located error and before any
  evaluation, a private path that lies inside the directory tree of the public ledger's repository (an ancestor directory
  that contains `.git` or the workspace `Cargo.toml`), and a public ledger path that lies inside the private directory (Q8).
* Nothing private goes to logs, panic messages or error strings: errors of a private source name the row and the column,
  never the value.
* The public summary states that a private source was used and names it by an id the caller chooses, not by file path.

The boundary is tested with a synthetic "measured" table generated by the test (D8, P1 to P4). No real measured file is
part of this item, and none may be added to the repository under it.

### D8. Defaults

Opt-in only. No existing function changes behaviour, no ledger entry written by existing code changes, and the ledger
format string stays `rcfd-ledger-1` (new `kind` values and, under Q2, one new tier value that only the new driver writes).

## Registration (to be fixed before any code)

The registration replaces this section. It must fix every item below, after a disclosed smoke with seeds other than the
registered ones, and before the registered runs. No check uses FAN-01, a mesh or a flow solve.

Synthetic sources:

* **S1 exact affine.** Low: a smooth closed form in `d = 3`. High: `a y_L + b_0 + b . x` with registered constants.
* **S2 published pair.** Forrester, Sobester and Keane (2007), one variable on [0, 1]: `f_H = (6x - 2)^2 sin(12x - 4)`,
  `f_L = 0.5 f_H + 10 (x - 0.5) - 5`. Solving for `f_H` gives `f_H = 2 f_L - 20 x + 20`, so R2 must return `rho = 2`,
  slope -20, intercept 20 exactly up to rounding. (Formula from memory; the algebra is checked here, the constants must be
  checked against the paper at registration.)
* **S3 perturbed reduced-order fan.** Low: `FanRomEvaluator` with `RomModel::default()` on a registered `RomFan` family
  with `d = 3` (proposed: stagger offset, chord scale, tip gap) at one registered condition. High: the same evaluator with
  registered perturbed constants (proposed `tip_cd x 1.3`, `annulus_drag x 1.5`, `secondary_drag x 1.5`). Metrics `dp_s`
  and `eta_s`; ranking objective from F-697 or, if it is not available, `eta_s` (max) at fixed system curve. The truth is
  cheap, so the true best of the screened set is computed directly.
* **S4 structure-free noise.** High = low + seeded normal noise of registered sd, declared with that uncertainty.
* **S5 always failing.** High returns `diverged` with a reason at every point (the present state of fan CFD).
* **S6 identical.** High = low.
* **S7 synthetic private table.** S3's high values written by the test to a temporary CSV outside the test's public
  directory, read back through `MeasuredTable`, `private() = true`.

Checks (limits proposed; each gets a test name, a seed and a final limit at registration):

| Id | Check | Proposed limit |
|---|---|---|
| A1 | S1, R2 recovers `a`, `b_0`, `b` | 1e-8 relative |
| A2 | S2, R2 from the 4 points `x = 0, 0.4, 0.6, 1`: `rho`, slope, intercept; corrected RMSE on 101 points | 1e-8; RMSE at most 1e-8 of the range |
| A3 | S2, rung choice: R2 is chosen and R3 is not (when `n` is raised to allow R3) | exact |
| B1 | S3, `n_screen = 2000`, 4 rounds of 6: RMSE of corrected against uncorrected on 500 held-out candidates | ratio at most 0.5, both metrics |
| B2 | S3: high objective of the best promoted candidate against the true best of the 2000 | within 2 % of the range of the high objective over the 2000 |
| B3 | S3: of the true top 5 %, the share inside the corrected top 10 % | at least 0.8, and above the uncorrected share |
| B4 | S3: mean of `abs(high - pred)` in round 4 against round 1 (out-of-sample by construction) | ratio at most 0.5 |
| T1 | S4: correction untrusted in every round; promoted set and low entries bit-identical to a run with correction off | exact |
| T2 | S5: 0 pairs, `n_failed_high = 24`, no error, screening bit-identical to correction off | exact |
| T3 | S6: every `diff` exactly 0, corrected prediction bit-identical to low | exact |
| T4 | S3 calibrated on the half box `x_1 <= 0.5`, queried on the other half: every query flagged; applied correction equals R1 | 100 %; exact |
| T5 | A correction saved for evaluator version A is refused for version B | located error |
| P1 | S7: no private value (shortest round-trip decimal text of each high value, each `diff`, each coefficient) occurs in the public ledger bytes | 0 occurrences |
| P2 | S7 with 4 pairs: no public aggregate; with 5: aggregates present, rounded to 3 digits | exact |
| P3 | Private path inside the repository tree, and public ledger inside the private directory | each refused before any evaluation |
| P4 | S7 with a source that returns an error: the error text contains no value of the table | 0 occurrences |
| R1 | Same seed, 1 and 8 workers, fixed clock: public and private ledgers | byte-identical |
| R2 | Existing default-bits tests and `baseline_hashes.rs` | unchanged, file not edited |

Negative controls (each must fail the named check, or the check proves nothing):

* NC1 for B2 and B3: random promotion with the same budget, and correction off with the same budget. At least one of B2,
  B3 must fail for each. If the uncorrected screening already passes B2, S3's perturbation is too mild and is strengthened
  before registration, not after.
* NC2 for T1: in-sample error used in place of leave-one-out in condition 2; the noise correction is then trusted.
* NC3 for T4: the region test removed; R2 is extrapolated and the error on the far half exceeds a registered bound.
* NC4 for P1: the split of D7 switched off; the values are found.

Planned mutations (scratch copy, not committed): sign of `diff`; correction applied twice; `rho` dropped at R2; leave-one-out
factor `1 - h_ii` dropped; nugget ignored; explore picks removed; `n_pub` test off by one; aggregates not rounded.

Decision rule (proposed): F-699 is done when A, T, P and R pass and B1 to B4 pass with their negative controls failing as
stated. A miss in B with A, T, P, R passing gives `partial`, with the missed limit kept as a known miss.

## Known risks

1. **Overfitting the correction.** A few expensive points and a flexible model. Guards: the rung ladder with smallest n,
   the one-standard-error rule, leave-one-out trust (D5.2), T1 and NC2. Residual risk: leave-one-out is optimistic when the
   promoted points cluster near the current best, which the exploit picks cause; the explore picks reduce it and do not
   remove it.
2. **Extrapolation.** A correction calibrated near one design is wrong elsewhere, and the optimiser will go elsewhere.
   Guard: D5.4 and T4. Residual risk: the box and distance test is crude in more than a few dimensions (most of a
   high-dimensional box is far from every point), so with many parameters most screening falls back to R1.
3. **Correcting the wrong thing.** A discrepancy model absorbs whatever differs, including errors of the high source. With
   CFD as the high tier, a wrong CFD result (the FAN-01 history) would be taught to the reduced-order model. The driver
   cannot detect this. The summary therefore names the high source and its tier on every correction, and a correction from
   an unvalidated tier must not be described as an improvement of accuracy, only as agreement with that tier.
4. **Synthetic verification only.** S3's high source has the same structure as the low model (same strips, other
   constants), so its discrepancy is smoother and lower-dimensional than a real one. Passing B1 to B4 shows the driver
   works, not that the reduced-order model can be corrected to a real fan with 24 points.
5. **Selection bias in the reported disagreement.** Promoted candidates are not a random sample, so the mean `diff` over
   pairs is not the model's mean error over the design space. The summary separates exploit and explore pairs for this
   reason.
6. **Privacy leaks by side channel.** The promotion decisions depend on the private correction; a public record of which
   candidates were promoted in which round carries a little information about it. D7 keeps the ranking private, but the
   public low entries still show which variants were looked at. Aggregates over small `n`, or repeated publication of
   aggregates as `n` grows by one, allow differencing (the change from `n` to `n + 1` isolates one point). Proposed guard:
   public aggregates are written at most once per registered block of new pairs (proposed every 5), not every round.
   Whether this is enough is Q7.
7. **Path check is not access control.** Refusing a private path inside the repository stops the common accident (a
   committed ledger). It does not stop a user from copying the file in, and it does not encrypt anything.
8. **Inconsistent corrected metrics** (D4, last note but one): corrected `eta_s` may disagree with corrected `dp_s`, `q`
   and `power`. Reported, not fixed.
9. **Dependence on parallel drafts.** The objective (ADR 0620) and the Gaussian process (ADR 0621) are assumed. If F-697
   defines something other than a scalar or a Pareto set, D3 changes.
10. **Noise in the high source** (measurement scatter, or CFD iteration error when it stalls). A stated uncertainty that is
    too small makes R3 chase noise; too large and nothing is corrected. The driver takes the stated value on trust.

## Limits of this item

* Two tiers per call (one low, one high). Three tiers (reduced-order, CFD, measured) are run as two calls; a joint model of
  three tiers is not proposed.
* No calibration of the reduced-order model's own constants, no CFD evaluator, no real measured data, no MCP tool.
* Corrections are fitted per condition set given by the caller; scaling a correction between speeds or sizes is Q14.

## Maintainer decisions (2026-11-15)

The maintainer answered the open questions of the design-loop drafts (ADRs 0620 to 0624) on 2026-11-15, after the stub was
written; the questions below are kept as asked. Shared decisions:

- D1, exposure: library only in F-268, F-697, F-698, F-699 and F-701. Their MCP tools and service methods land in one item,
  F-702, together with F-339, so the tools-list goldens change once.
- D2, measured tier: `Fidelity::Measured` is added in the library only (F-700), written only to private ledgers; the MCP
  fidelity enums stay unchanged until F-702.
- D3, privacy: strict split. Measured values, fidelity pairs with a measured source, the fitted correction and corrected
  predictions go to a caller-given private ledger, refused with a located error when it lies inside the repository tree. A
  public ledger gets rounded aggregates only (3 significant digits, at least 5 pairs).
- D4, record: the fidelity pair and discrepancy record of ADR 0622 is the only one; ADR 0136 is amended to point to it.
- D5, ledger fields: new fields are optional top-level keys of `rcfd-ledger-1`, omitted when unset; `provenance` conventions
  until they land (F-700).
- D6, split and order: F-268 keeps the ledger sink, the `FanRomModel` adapter and the bands; the shared ledger fields and
  queries are F-700. Order: F-268 and F-700, then F-697, then F-698 and F-699 in parallel, then F-701.
- B1, measured data format: the CSV of `docs/fan-measurement-protocol.md` with a variant-id column added.
- B2, hypothesis family level: 0.05 with the `6 / (pi^2 i^2)` series, Holm as a batch option.
- B3, estimated limits: a limit this text marks as an estimate or as "to be fixed after a smoke run" is fixed by the
  implementing agent from a disclosed smoke run on separate seeds, or a written calculation, and registered before any
  acceptance run; it is not moved after a run. A negative control that passes by a thin margin is fixed by strengthening the
  test, never by moving the limit.
- B4, ADR 0240 Q4: the NC1 miss on ST1 (|z| 3.87 against 4) is accepted as a known miss; ADR 0240 Q5: fixed N.

Every other open question of this ADR takes the recommendation written with it. Here: Q1 is D1; Q2 is D2; Q3 takes R0 to R2 now with R3 reusing F-698; Q7 and Q8 are D3; Q9 is B1; Q10 and the trust thresholds fall under B3; Q12: deps F-268, F-697 and F-700.

## OPEN QUESTIONS for the maintainer

Each has options and a recommendation. None is taken in this draft.

1. **Q1 Exposure.** (a) Library only. (b) Also a service method and an MCP tool `design_fidelity` for a named evaluator;
   this changes the MCP tools-list goldens and needs a named high source. **Recommendation: (a)**; propose (b) as a
   follow-up next to F-339, which has the same named-evaluator problem.
2. **Q2 Ledger tier for measured data.** (a) Add `Fidelity::Measured` (`"measured"`) in the library only, written only to
   private ledgers; the three MCP schema enums stay `analytic, reduced_order, cfd`, so no golden changes and the public
   tools cannot append a measured entry. (b) Add it to the MCP enums too (golden change). (c) No new tier; mark measured
   sources in `provenance`. **Recommendation: (a).** Note: a reader built before the change skips and reports lines with
   the new tier, which only occur in private ledgers.
3. **Q3 Gaussian-process rung and F-698.** (a) F-699 ships R0 to R2 only; R3 is added when F-698's surrogate exists and is
   reused. (b) F-699 ships its own small Gaussian process. (c) F-699 depends on F-698. **Recommendation: (a)**: one
   Gaussian-process implementation in the tree, and R2 already covers the registered exact cases; the acceptance text then
   needs R3 moved to a follow-up clause.
4. **Q4 Form of the simple correction.** (a) Additive only. (b) Multiplicative only. (c) Fit both, keep the one with the
   smaller leave-one-out error. **Recommendation: (c)**; fan losses scale with dynamic pressure (multiplicative), offsets
   such as an installation effect are additive, and the data can say which.
5. **Q5 Promotion split.** (a) Exploit only. (b) Fixed split, proposed 4 exploit + 2 explore of 6. (c) Caller-set split
   with (b) as the default. **Recommendation: (c).**
6. **Q6 Ranking with several objectives.** Depends on ADR 0620. (a) Use F-697's scalar objective only. (b) Also promote
   from the corrected non-dominated set. **Recommendation: (a) first**, (b) only if ADR 0620 defines no scalar.
7. **Q7 How strict is the privacy split.** (a) As D7: measured values, pairs, the fitted correction and corrected
   predictions all private; public aggregates only for at least 5 pairs, rounded, in blocks of 5. (b) Also publish the R1
   and R2 coefficients when `n_pairs` is large (proposed at least 10 times the coefficient count). (c) Publish everything
   except the raw table. **Recommendation: (a)**; (b) can be allowed later, (c) publishes the data in effect. Please also
   confirm the values 5, 3 digits and blocks of 5, or give others.
8. **Q8 Where private output lives.** (a) A caller-given path on every call, refused if inside the repository tree. (b) An
   environment variable. (c) A private folder of project mode, which touches the project-folder layout and possibly
   project or case-file keys. **Recommendation: (a)**; (c) only as a separate item with its own decision on keys.
9. **Q9 Measured-file format.** (a) The CSV header of `docs/fan-measurement-protocol.md` "Data delivery", plus a variant id
   column. (b) A new JSON format. **Recommendation: (a)**; adding the variant id column is an edit of that protocol page
   and needs your agreement.
10. **Q10 Trust thresholds.** `f_trust = 0.8`, coverage 0.8, margin 0.05, `h_max = 0.25`, smallest n of 3, `d + 4`, `10 d`.
    (a) Accept as starting values, to be confirmed or changed by the disclosed smoke before registration. (b) Give other
    values now. **Recommendation: (a).**
11. **Q11 Failed high evaluations.** (a) A failed promotion consumes its budget. (b) It is refunded and the next candidate
    is promoted, up to a cap. **Recommendation: (a)**: with a failing CFD tier, (b) spends unbounded time; the failure
    count is in the summary either way.
12. **Q12 Dependencies.** (a) deps F-268, F-697; F-698 related only. (b) Also F-698. (c) Also the reduced-order fan tier as
    an explicit dependency (it has no roadmap id of its own that I could find; ADR 0119 is cited under F-218/F-113 in the
    plan). **Recommendation: (a)**, with Q3 (a).
13. **Q13 Calibrating the loss constants instead of the outputs.** Kennedy and O'Hagan's later calibration framework fits
    model parameters and a discrepancy together, and the two are confounded without strong priors (Brynjarsdottir and
    O'Hagan 2014). (a) Output discrepancy only in F-699; fitting `tip_cd`, `annulus_drag` and the other `RomModel`
    constants is a separate follow-up. (b) Both in F-699. **Recommendation: (a).** A fitted loss constant would extrapolate
    better than an output correction, which is the reason to want the follow-up, but it needs its own registration.
14. **Q14 Variables of the fit.** (a) Dimensional metrics at the caller's conditions. (b) Flow and pressure coefficients
    (`q / (A U_tip)`, `dp / (rho U_tip^2)`), so that one measured speed informs others through the fan laws. **Recommendation:
    (a) in the generic driver, with an optional caller-supplied transform** that the fan evaluator can set to (b); the fan
    laws do not hold across the Reynolds-number range of small fans, so (b) must stay a choice and be reported.

## References

All from memory unless stated; equation and section numbers UNVERIFIED and to be checked at registration.

* M. C. Kennedy, A. O'Hagan (2000), Predicting the output from a complex computer code when fast approximations are
  available, Biometrika 87, 1-13 (the autoregressive two-level model).
* M. C. Kennedy, A. O'Hagan (2001), Bayesian calibration of computer models, J. R. Statist. Soc. B 63, 425-464
  (calibration parameters with a discrepancy function).
* J. Brynjarsdottir, A. O'Hagan (2014), Learning about physical parameters: the importance of model discrepancy, Inverse
  Problems 30, 114007 (confounding of parameters and discrepancy).
* A. I. J. Forrester, A. Sobester, A. J. Keane (2007), Multi-fidelity optimization via surrogate modelling, Proc. R. Soc. A
  463, 3251-3269 (the one-variable test pair of S2).
* C. E. Rasmussen, C. K. I. Williams (2006), Gaussian Processes for Machine Learning, MIT Press (regression, leave-one-out
  closed form).
* T. Hastie, R. Tibshirani, J. Friedman (2009), The Elements of Statistical Learning, 2nd ed., Springer (one-standard-error
  rule, leave-one-out for linear smoothers).
* J. L. Loeppky, J. Sacks, W. J. Welch (2009), Choosing the sample size of a computer experiment: a practical guide,
  Technometrics 51, 366-376 (the `10 d` rule of thumb used for R3's smallest n).
* Project documents: ADR 0119 (reduced-order fan), ADR 0121 (ledger, tolerance driver), ADR 0240 (Sobol indices),
  `docs/fan-project-breakdown.md` (H7, C7, limit 1), `docs/fan-measurement-protocol.md`, `docs/milestone-fan.md`.

## For the lead: roadmap entry

See `roadmap-entry.json` next to this file (F-699, phase 9, deps F-268 and F-697, status todo). If Q3 (a) is taken, the
acceptance is unchanged (it names no Gaussian-process clause) and a follow-up item for rung R3 is proposed after F-698.

## Addenda

Results, amendments and the gate line go here; nothing above is changed after a run.

## Amendment 1 (2026-11-16): registration, before any code of F-699 and before any run of its checks

REGISTERED 2026-11-16, written before any code of F-699 and before any run of its checks. Written by the implementing agent on
branch `f699-work` (from 6400c929, the F-697 tip on F-700 and F-268), worktree `w49i-work/f699`, scratch `w49i-work/f699-scratch`.
Before this text no library or test code of F-699 existed in the branch. This amendment replaces the section "Registration (to be
fixed before any code)" above; where the two differ, this text holds. The maintainer answered the open questions (section
"Maintainer decisions"); every further choice below is taken on the maintainer's standing instruction of 2026-11-11 (take the
recommendation) and is marked "(standing instruction)".

### What was read

`AGENT_RULES.md`; the brief; F-699's roadmap acceptance and note (binding, quoted below); this ADR (stub and maintainer
decisions); ADR 0620 (F-697: `FanObjective`; its `ObjectiveModel` is a `ledger::Model`); ADR 0623 (F-268: `FanRomModel`, the
`ledger::Model` adapter of the reduced-order fan with the parameters `stagger_common_deg` [deg], `chord_rel` [1], `tip_gap_delta`
[m]); ADR 0632 (F-700: `Fidelity::Measured`, `Ledger::private` with `repository_ancestor`, the public reader and append refusing
measured content, `fidelity_pair`, `pair_by_params`, `Level`, `entry_ref`, `entry_hash`, the `fitted` and `parents` fields; its
"not covered" gives this driver the public-path-inside-private-directory check and the rounded public aggregates); ADR 0136 and
its 2026-11-15 amendment (the pair record is this ADR's); `docs/fan-project-breakdown.md` H7; `docs/fan-measurement-protocol.md`
("Data delivery": the CSV header `point,rpm,flow_m3_s,dp_static_Pa,dp_side,supply_V,supply_A,p_amb_Pa,T_amb_K,rh,lwa_dB,notes`);
`loop-drafts/decisions.md`. Source: `design/ledger.rs` (all public items), `design/fan_rom_eval.rs`, `design/uq.rs` (the pattern:
`parallel_map`, entries appended in sample order from the calling thread), `design/sampling.rs` (`latin_hypercube`),
`design/study.rs` (`parallel_map`, `Sense`), `rcfd_post::fan_rom::RomModel` (the loss constants and their defaults).

The binding acceptance (clauses numbered as in the roadmap):

> Every limit not stated here is registered in ADR 0622 before the run, and no check uses FAN-01 or any CFD run. An opt-in library driver screens at least 2000 candidates with a ledger::Model of the reduced-order tier, promotes at most 24 of them (4 rounds of 6) to a high-fidelity source, writes one pair entry per promoted candidate (low value, high value, difference, per metric) and one summary per round to the design ledger (ADR 0121), and fits a discrepancy correction that later rounds apply. With synthetic high-fidelity sources: (1) a high source equal to a * low + b + c . x is recovered with a, b, c within 1e-8 relative, and the published one-variable pair of Forrester et al. (2007) gives rho = 2, slope -20, intercept 20 within 1e-8; (2) on the reduced-order fan evaluator against a copy with perturbed loss constants, the corrected screening has a held-out RMSE at most 0.5 times the uncorrected one and the best promoted candidate is within 2 percent of the high-fidelity objective range of the true best of the screened set, while the registered negative control (random promotion, same budget) is not; (3) a high source that is the low model plus unstructured noise, and one that diverges at every point, both leave the correction untrusted and the screening bit-identical to the uncorrected one; (4) every query outside the calibrated region is flagged and gets the constant correction only; (5) with a source marked private, no measured value, no fitted correction and no corrected prediction appears in the public ledger (byte search of the file), aggregates appear only for at least 5 pairs, and a private path inside the repository tree is refused with a located error; (6) the same seed gives a byte-identical ledger for 1 and 8 workers, and no existing default-bits test changes.

Interpretation (standing instruction): "one pair entry per promoted candidate" is one `fidelity_pair` per promoted candidate whose
high evaluation is `ok`; a failed high evaluation cannot be paired (`ledger::fidelity_pair` refuses it), is recorded by its own
`evaluation` entry with its verdict and reason, and is counted in the summary (Q11 (a)).

### Disclosed: smoke runs and arithmetic before this registration

All on seeds that no check below uses. A temporary test `crates/rcfd-api/tests/zz_f699_smoke.rs` (never committed, moved to
`f699-scratch/zz_f699_smoke.rs`) ran the existing `FanRomModel` (set-up C of ADR 0119 as in `fan_rom_bands_f268.rs`: 7 blades,
r_hub 0.030, r_tip 0.058, tip gap 1 mm, free-vortex design to 45 Pa at 0.045 m^3/s, 3000 rpm, system `k` through the design
point, n_scan 100) with `RomModel::default()` (low) and with `tip_cd x 1.3`, `annulus_drag x 1.5`, `secondary_drag x 1.5` (high)
on Latin hypercubes and a grid and wrote CSV files; a numpy prototype of the loop below (`f699-scratch/proto.py`, `b3.py`,
`s4.py`, about 150 lines, written from this text, not rcfd code) read them. Findings:

1. Cost: 4000 evaluations (2000 per tier) in 144 to 149 s with 4 workers, about 0.145 s per evaluation.
2. Box 1 (stagger offset [-3, 3] deg, chord_rel [-0.25, 0.25], tip_gap_delta [-5e-4, 1.5e-3] m, LHS seed 11): every point ok in
   both tiers; the best `eta_s` lies at a corner of the box. A 13 x 7 grid (stagger [-12, 6], chord_rel [-0.6, 0.6], gap delta 0)
   shows an interior optimum of `eta_s` in stagger near -6 to -7.5 deg, a stall edge in chord near -0.4 to -0.6, and points ok
   in the low tier and stalled in the high one. So the registered box is box 2.
3. Box 2 (stagger [-10, 2] deg, chord_rel [-0.45, 0.3], tip_gap_delta [-5e-4, 1e-3] m), LHS seeds 12 and 13, 2000 points each:
   low ok 1912 and 1915, both ok 1869 and 1868 (43 and 47 low-ok points stalled in the high tier). `eta_s` low range 0.160 to
   0.199, high 0.140 to 0.184, `high - low` mean -0.0186, sd 0.0015; `dp_s` mean difference -0.325 Pa, sd 0.12 Pa. A global R2
   fit leaves rms 3.8e-4 (`eta_s`) and 0.031 Pa (`dp_s`). Points within 2 % of the high range of the true best: 2 and 4 of about
   1870, so random promotion of 24 hits with probability 0.025 and 0.050.
4. Prototype of the registered rule (4 rounds of 4 exploit + 2 explore, the trust rule and region below, h_max 0.25): held-out
   RMSE ratio (corrected over uncorrected, final correction) `dp_s` 0.288 and 0.315, `eta_s` 0.107 and 0.091; best promoted
   candidate at gap 0 of the range (seeds 12, 13); random promotion (three seeds each) gaps 0.081 to 0.191; mean |high - pred| of
   round 4 over round 1 (`eta_s`) 0.51 and 0.22; with h_max 0.25, 0.66 to 0.73 of the held-out candidates lie outside the
   calibrated region and get the constant only. The uncorrected screening (correction off) also reaches gap 0: the perturbation
   moves the level of the losses much more than the location of the best, so B2 cannot tell the correction from no correction on
   this fan (risk 4 of the stub); the binding acceptance asks only that random promotion misses it, and the correction-off result
   is recorded, not judged. The stub's B3 (share of the true top 5 % in the corrected top 10 %) was 1.0 with and without the
   correction in both smokes, so it is dropped (it cannot discriminate).
5. Noise source (prototype, 300 synthetic trials, sd 0.05 stated as the uncertainty): the registered rule trusted a correction in
   0 of 300 trials in any round; with only the uncertainty condition off (k_u = 0) 0.11; with every guard off (in-sample errors, no
   one-standard-error rule, k_u = 0) 0.64; with f_trust = 1 as well, the in-sample R2 error is never above R0's (nested least
   squares), so some round is trusted with probability 1. NC2 is therefore registered with every guard off and f_trust 1.
6. Arithmetic, S2. As written in the stub and the acceptance, `f_L = 0.5 f_H + 10 (x - 0.5) - 5`, so `f_H = 2 f_L - 20 x + 20`.
   My recollection of the paper (from memory, UNVERIFIED; the repository does not hold it) is `f_L = A f_H + B (x - 0.5) - C` with
   A = 0.5, B = 10, C = -5, which gives `f_L = 0.5 f_H + 10 x` and `f_H = 2 f_L - 20 x` (intercept 0). Both are exact affine
   relations and both are checked (A2, A2b); A2 carries the acceptance's numbers. Which sign is the paper's is a remaining question.

### Design (fixed now; names are the implementation's)

New module `crates/rcfd-api/src/design/fidelity.rs`, test file `crates/rcfd-api/tests/f699_fidelity.rs`; one module line in
`design/mod.rs` and one line of its doc comment. No other source file is edited. Docs: `docs/design-loop.md` (a section),
`docs/fan-measurement-protocol.md` ("Data delivery": the `variant` column, decision B1), `docs/fan-project-breakdown.md` (H7 row).
No existing path calls the module, so every default keeps its bits.

* `HighSource: Sync` with `id()`, `version()`, `fidelity()`, `level() -> Option<String>` (written as `fidelity_level`),
  `private() -> bool` (default false), `source() -> Option<ledger::Source>` (default None), `available(params) -> bool` (default
  true), `observe(params) -> Observation { outcome: Outcome, uncertainty: Vec<Quantity> }`. `ModelSource { model: &dyn Model, id,
  version, fidelity, level }` adapts any `ledger::Model` (public, uncertainty 0, always available); `ModelSource::of(model)` keeps
  the model's identity. `MeasuredTable` (D7, decision B1) reads a CSV whose header holds a `variant` column and the caller's
  columns: `MeasuredTable::read(path, columns: [(column, metric, unit, standard uncertainty)], variant_of: Fn(&[Quantity]) ->
  Option<String>, id, version)`; `observe` gives the first row of the variant; `available` is true only for variants in the table;
  tier `measured`, `private() = true`, `source() = {id, kind: measured}`. Every error of the table names the file, the 1-based
  row and the column, never a cell's text.
* `FidelitySpec { name, params: Vec<ParamBox { name, unit, lo, hi }>, candidates: Candidates::{List(raw points) | Lhs { n, seed
  }}, metrics: Vec<CorrectedMetric { name, floor }>, objective, sense: study::Sense, rounds, promote_per_round, explore_per_round,
  promotion: Promotion::{Rule | Random { seed }}, correct, trust: TrustRule, workers, privacy: Option<Privacy { path, id, n_pub,
  digits, block }> }`. The objective is a metric of the low model; if it is a corrected metric it is ranked by its corrected
  prediction, else by the low value (an F-697 `ObjectiveModel` can be the low model). Defaults: rounds 4, promote 6, explore 2
  (Q5 (c)), `Rule`, correct true, workers 1, n_pub 5, digits 3, block 5.
* `run(spec, low: &dyn Model, high: &dyn HighSource, public: &Ledger) -> Result<FidelityResult, String>`. Refused before any
  evaluation, with located messages: a box not finite with `lo < hi`, a duplicate parameter, no metric, a metric twice, a floor not
  finite and positive, rounds or promote 0, explore > promote, a `List` point of the wrong length, non-finite or outside the box,
  `Lhs` with n 0, a private high source without `privacy`, `privacy` with `n_pub < 1`, `digits` outside 1..=17 or `block < 1`, the
  low and high levels equal (fidelity and level), `Ledger::private(privacy.path)` refusing (the repository rule of ADR 0632), and a
  public ledger path inside the private ledger's directory (D7; both made absolute with the nearest existing ancestor
  canonicalised, as `ledger::repository_ancestor` does; "inside" is `Path::starts_with`).
* The loop (D2). Round 1 screens every candidate with `low` (`parallel_map` over the candidates with `workers`); entries in
  candidate order (variant `<name>#<i>`, provenance `{driver: fidelity, candidate: i}`) to the public ledger and, with privacy, to
  the private ledger too. Then per round: predictions of every low-ok candidate with the current correction; promotion; the picks
  evaluated with `high` (`parallel_map`, pick order); their `evaluation` entries (provenance `{driver, candidate, round, reason}`)
  to the full ledger (the private one with privacy, else the public one); one `ledger::fidelity_pair` per ok pick (metrics in spec
  order, the floors, `preds` = the predictions made before the high result) with provenance `{driver: fidelity, round, reason:
  exploit | explore | random, candidate}`; refit on all ok pairs so far (when `correct`); one `fidelity_summary` (D6) to the full
  ledger and, with privacy, one public summary. Every entry is stamped with `solver_version()` and the ledger's clock before it is
  appended, so the in-memory mirror used for the pair references hashes as the file does (`entry_hash`); the mirror starts from the
  ledger's existing entries. The driver stops early, without an error, when no candidate is available.
* Promotion (D3). `Rule`: `promote - explore` exploit picks, best corrected objective first (ties: lower index) among low-ok, not
  yet promoted, `available` candidates; then `explore` greedy max-min picks in the unit box against every candidate promoted so far
  including this round's earlier picks and failed ones (ties: lower index; against an empty set: the lowest available index).
  `Random { seed }`: `promote` picks by one `Rng::new(seed)` for the run, `below(len)` over the remaining available candidates in
  index order. A failed high evaluation consumes its promotion (Q11 (a)).
* Discrepancy model (D4, Q3 (a), Q4 (c)), per metric on the ok pairs `(u_i, yL_i, yH_i)`, `u` the parameters scaled to the unit
  box, `d` parameters, `n` pairs, `e` the leave-one-out residuals:
  - R0: `y_c = y_L`; `e_i = yH_i - yL_i`.
  - R1add (n >= 3): `b = mean(yH - yL)`, `e_i = r_i n / (n - 1)`. R1mul (n >= 3, `sum yL^2 > 0`): `a = sum yL yH / sum yL^2`,
    `h_i = yL_i^2 / sum yL^2`, `e_i = r_i / (1 - h_i)`.
  - R2 (n >= d + 4): columns `[yL, 1, u_1..u_d]`, Householder QR least squares; `h_ii` the squared row norms of the thin Q,
    `e_i = r_i / (1 - h_ii)`; skipped (reason in the summary) when some `|R_jj| <= 1e-12 max_k |R_kk|` or some `h_ii >= 1 - 1e-12`.
    Coefficients in raw parameters: `rho`, `b0 = b0_u - sum_j b_j lo_j / (hi_j - lo_j)`, `c_j = b_j / (hi_j - lo_j)`; predictions
    use `rho yL + b0 + c . x`.
  - Levels 0 (R0), 1 (the R1 form with the smaller LOO MSE; ties additive), 2 (R2). `MSE = mean(e^2)`, `SE = sd(e^2) / sqrt(n)`
    (divisor n - 1). One-standard-error rule (Hastie, Tibshirani and Friedman 2009, from memory): the lowest level whose MSE is at
    most `MSE_min + SE_min`.
* Trust (D5). The chosen rung is applied inside the calibrated region only when (t1) it is not R0; (t2) `sqrt(MSE_chosen) <=
  f_trust rmse0`, `rmse0 = sqrt(mean((yH - yL)^2))`; (t3) `rmse0 > k_u u_rms`, `u_rms` the rms over the pairs of the high source's
  stated standard uncertainty of the metric (a discrepancy at or below the stated noise is not corrected; new here, standing
  instruction: the stub's coverage condition belonged to R3). The constant (R1add) is trusted when n >= 3 and t2, t3 hold for
  R1add. Region (D5.4): the query's unit-box point lies in the pairs' bounding box grown by `margin` in every direction and within
  distance `h_max` of the nearest pair point. Inside and trusted: the chosen rung; inside, untrusted: R0; outside: flagged
  `extrapolated`, and the trusted constant is applied (`y_L + b`), else R0. `correct = false`: no fit, R0 everywhere. Values
  (decision B3, from the calculation and the smokes above): `f_trust = 0.8`, `k_u = 2`, `margin = 0.05`, `h_max = 0.25`. Negative
  control switches, documented as such: `in_sample` (in-sample residuals in place of `e`), `one_se = false` (smallest MSE wins).
* `Correction { low (id, version), high (id, version), params, region (points, margin, h_max), fits per metric { metric, rung,
  coef, constant, trusted, why, n, MSE per rung, rmse0, u_rms } }`; `fit(pairs, params, trust, identities)`; `to_json` /
  `from_json` (numbers bit-exact); `predict(x, y_low) -> (values, flags)`; `predict_for(low_id, low_version, high_id, high_version,
  x, y_low)` refuses another identity with a located message naming both versions (D5.5); `loo_residuals(metric, rung)` for checks.
* Summary (D6): `fidelity_summary`, evaluator `rcfd.fidelity` version `1`, verdict ok, params empty, study `<name>`, variant
  `<name>#round<r>`; metrics per corrected metric `m.n_pairs`, `m.rmse_before` (rmse0), `m.rmse_after` (sqrt LOO MSE of the chosen
  rung), `m.bias` (mean diff), `m.max_abs_rel`, `m.trusted` (1 or 0), `m.n_extrapolated` (low-ok candidates flagged by the new
  correction), `m.oos_rmse` (rms of `high - pred` over all pairs so far), and `n_promoted`, `n_failed_high`; settings `{round,
  rungs: {m: {rung, why}}, correction: Correction::to_json()}`; `fitted` = `{id: rcfd.fidelity.correction, version: 1, hash: FNV-1a
  64 of the correction text}` when a correction exists; `parents` = the round's pair entries; provenance `{driver: fidelity, round,
  spec}`. Tier and level: the high source's. With n = 0 a metric's aggregates are written as 0 with `m.n_pairs` 0.
* Privacy (D7, decision D3). With a private source the public ledger gets the low entries and, per round, a public summary (kind
  `fidelity_summary`, tier the low one, settings `{round, private_source: <privacy.id>, public: true}`, metrics `n_promoted`,
  `n_failed_high` and, only when the pair count `n >= n_pub` and `floor(n / block)` has grown since the last publication (risk 6),
  per metric `m.n_pairs`, `m.rmse_before`, `m.rmse_after`, `m.bias`, `m.max_abs_rel`, `m.trusted`, each rounded to `digits`
  significant digits (`format!("{:.*e}", digits - 1, v)` parsed back), with the rung names in settings). Nothing else: no high
  value, pair, coefficient, region point, prediction or pick list. Everything else goes to the private ledger. `FidelityResult`
  holds the private content in memory for the caller. With a public source there is no split.
* `FidelityResult { candidates (raw), low (per candidate: the corrected metrics' low values or None), rounds: [Round { picks:
  [(candidate, reason)], failed_high, predictions (per candidate per metric; NaN when the low failed), extrapolated, correction }],
  pairs: [(candidate, round, reason, low, high, pred)], correction (the last), final_predictions, final_extrapolated }`.

### Set-ups (fixed now)

* **S1, exact affine.** `x1` [m] in [0, 1], `x2` [1] in [-1, 2], `x3` [s] in [10, 20]. Low (`analytic.s1` 1, tier analytic):
  `f = sin(3 x1) + x2^2 + 0.1 x3` [Pa], `g = exp(0.5 x1) - x2 x3 / 20` [W]. High (`synthetic.s1` 1, tier analytic, level `high`):
  `f_H = 1.7 f - 0.3 + 0.5 x1 - 2.0 x2 + 0.04 x3`, `g_H = 0.9 g + 1.25 - 0.1 x1 + 0.3 x2 - 0.02 x3`. Candidates `Lhs { 200, seed
  699_001 }`, objective `f` Min, floors 1e-3, 4 rounds of 6 (2 explore).
* **S2, the one-variable pair.** `x` [1] in [0, 1]; low `f_L = 0.5 f_H + 10 (x - 0.5) - 5`, `f_H = (6 x - 2)^2 sin(12 x - 4)`; high
  `f_H` (level `high`). Candidates `List` of `x = i / 100`, i = 0..=100; objective `f` Min; 2 rounds of 6. **S2b**: low `f_L = 0.5
  f_H + 10 x`; five pairs at `x = 0, 0.25, 0.5, 0.75, 1`, `Correction::fit` directly.
* **S3, the reduced-order fan.** Low `FanRomModel` (set-up C as in the smoke, `RomModel::default()`, n_scan 100); high the same
  with `tip_cd x 1.3`, `annulus_drag x 1.5`, `secondary_drag x 1.5`, through `ModelSource` with id `synthetic.fan_rom_perturbed`
  version `1`, tier reduced_order, level `perturbed-losses`. Box 2. Metrics `dp_s` (floor 1 Pa) and `eta_s` (floor 1e-3); objective
  `eta_s` Max; 4 rounds of 6 (2 explore); workers 4. Heavy: `Lhs { 2000, seed 699_301 }`; negative control `Promotion::Random {
  seed 699_302 }`; and the run with `correct = false`. Truth: the test evaluates both models at every candidate once (4 workers);
  the three driver runs get models that return these stored outcomes by exact parameter bits (same id, version, tier; the models
  are deterministic, so this only saves time; disclosed). Fast: `Lhs { 200, seed 699_311 }` with the real models.
* **S4, noise.** Low S1's model, metric `f` only; high `f + 0.05 z`, `z` standard normal by Box-Muller from `Rng::new(699_004 ^
  bits)`, `bits` the FNV-1a 64 of the parameter bit patterns, stated uncertainty of `f` 0.05 (level `noisy`). Candidates `Lhs {
  300, seed 699_041 }`, 4 rounds of 6.
* **S5, always failing.** High returns `diverged`, reason `synthetic: the high tier diverged`, at every point (level `failing`).
  S1's low, candidates and metrics.
* **S6, identical.** High = S1's low through `ModelSource` with id `synthetic.s6` 1, level `copy`.
* **S1h, the half box.** Low S1; high `f_H = f + 2.0 + 0.2 x1`, `g_H = g + 2.0 + 0.2 x1` (level `high`); candidates `List` of
  `Lhs { 200, seed 699_051 }` with `x1` scaled to [0, 0.5]; queries `Lhs { 500, seed 699_052 }` with `x1` scaled to [0.6, 1] and
  `x2`, `x3` over their boxes.
* **S7, synthetic private table.** S1's low; the test writes `<temp>/rcfd_f699_<tag>_<pid>/table.csv` with the protocol header
  plus `variant` and `eta_s` (`variant,point,rpm,flow_m3_s,dp_static_Pa,dp_side,supply_V,supply_A,p_amb_Pa,T_amb_K,rh,lwa_dB,
  eta_s,notes`), one row per candidate (variant `v<i>`, `dp_static_Pa` = S1's `f_H`, `eta_s` = S1's `g_H`, the other columns fixed
  numbers); columns `dp_static_Pa -> f [Pa]` (uncertainty 0.01), `eta_s -> g [W]` (0.001); `variant_of` finds the candidate by
  its parameter bits. Private ledger `<temp>/.../private/ledger.jsonl`, public `<temp>/.../public/ledger.jsonl`. Candidates `Lhs {
  200, seed 699_071 }`, 4 rounds of 6, `privacy.id` `rig-synthetic`.

### Checks (limits fixed now)

All in `crates/rcfd-api/tests/f699_fidelity.rs`, release, fixed clock `1_700_000_000_000` on every ledger.

| Id | Clause | What | Limit |
|---|---|---|---|
| A1 | (1) | S1 through the driver: the final correction's rung for `f` and `g`, trust, and the coefficients (rho, b0, c1, c2, c3) against (1.7, -0.3, 0.5, -2.0, 0.04) and (0.9, 1.25, -0.1, 0.3, -0.02) | R2 and trusted, both; each coefficient relative error <= 1e-8 |
| A2 | (1) | S2 through the driver: the final rung; rho, slope, intercept; the corrected prediction at the candidates not flagged | R2; rho 2, slope -20, intercept 20 within 1e-8 relative; RMSE over the unflagged candidates <= 1e-8 of the range of `f_H` on the 101 points |
| A2b | (1) | S2b, direct fit | rho 2, slope -20 within 1e-8 relative; intercept within 1e-8 absolute |
| A3 | — | the ledger of A1 read back: 0 skipped lines; 200 low `evaluation` entries, 24 high, 24 `fidelity_pair`, 4 `fidelity_summary`; every pair's `parents` resolve to its low and high entries (same candidate); `m.diff` = `m.high - m.low` bit for bit; `m.pred` equal to the round's prediction bit for bit; every summary's `fitted.hash` = FNV-1a 64 of its `settings.correction` text | all exact |
| L1 | — | S4's final pairs: `loo_residuals` of R1add, R1mul and R2 against brute force (refit without pair i, predict pair i) computed in the test | relative <= 1e-9 each (absolute 1e-12 when below 1e-3 of the rms) |
| E1 | — | A1's run: per round, reasons `exploit` x 4 then `explore` x 2; the explore picks equal the greedy max-min picks computed in the test from the earlier picks | exact |
| B1 | (2) | S3 heavy: RMSE over the held-out candidates (low and high ok, not promoted) of the final corrected prediction against the uncorrected, per metric | ratio <= 0.5 for `dp_s` and for `eta_s` |
| B2 | (2) | S3 heavy: `(best high eta_s of the screened set - best high eta_s of the promoted) / (high range of eta_s over the screened set)`, over candidates ok in both tiers | <= 0.02 |
| NC1 | (2) | S3 heavy with `Promotion::Random { 699_302 }`: the same quantity | > 0.02 (the control must miss) |
| B4 | — | S3 heavy: mean `abs(high - pred)` of `eta_s` over the round-4 pairs against round 1 | ratio <= 0.75 (smokes 0.51, 0.22) |
| B5 | — | S3 heavy, structure: 2000 low entries, 24 high entries, one pair per ok high, 4 summaries; the correction-off run's B2 quantity | exact; correction-off recorded, no limit |
| B6 | — | S3 fast (200, real models): counts as B5 with 200; public ledger bytes for workers 1 and 8 | exact; equal |
| T1 | (3) | S4: every round's fit untrusted (rung and constant); every round's picks and predictions (bits) and the low entries' bytes equal to the run with `correct = false` | exact |
| NC2 | (3) | S4 with `in_sample`, `one_se = false`, `k_u = 0`, `f_trust = 1` | at least one round trusted (the control must fail T1) |
| T2 | (3) | S5: `Ok` result; 0 pairs; `n_failed_high` of the last summary 24; picks, predictions and low entry bytes equal to `correct = false` | exact |
| T3 | (3) | S6: every pair's `f.diff`, `g.diff` exactly 0; every round's predictions bit-equal to the low values; final rung R0, untrusted | exact |
| T4 | (4) | S1h: every one of the 500 queries flagged; the applied value equals `y_L + b` bit for bit for `f` and `g`, where `b` is the mean of `high - low` over the pairs computed in the test in pair order; the constant trusted | 500 of 500; exact |
| T4b | (4) | A1's run: for every low-ok candidate in every round after round 1, the flag equals the region rule computed in the test; a flagged candidate's prediction is `y_L + b` (constant trusted) or `y_L` | exact |
| T5 | — | A1's correction: `to_json` then `from_json` predicts bit-equal at 50 points; `predict_for` with low version `2` and with high version `9` | equal; both refused, each message names both versions |
| P1 | (5) | S7: no private value in the public ledger bytes: every high value, `diff`, `rel` and `pred` of the private pairs, every coefficient and constant of every round's correction, every final prediction; each searched as Rust `{}` text and as serde_json text; only texts of at least 8 characters | 0 occurrences; at least 400 texts searched |
| NC4 | (5) | S7 with the same table as a public source (`ModelSource`-like wrapper reporting tier analytic, privacy off): the same search | at least 1 occurrence (the control must fail P1) |
| P2 | (5) | S7 with 1 round of 4 (explore 0): no `.n_pairs` metric in the public summary; 1 round of 5: present, `f.n_pairs` = 5, every aggregate bit-equal to its 3-digit rounding; 5 rounds of 2 (explore 0): aggregates in rounds 3 and 5 only | exact |
| P3 | (5) | private path `CARGO_MANIFEST_DIR/target/f699_private/ledger.jsonl`; a public ledger inside the private directory | each refused, the message names the path, before any evaluation (low call counter 0, no ledger file created) |
| P4 | (5) | a table with one malformed cell (`12.5x`), one with a missing mapped column, one with a missing `variant` column | refused; row and column (or column) named; no cell text of the table of at least 3 characters in the messages |
| P5 | (5) | S7: the private ledger read with `Ledger::private` has 0 skipped lines, 24 pairs, 4 summaries; the public ledger read with `Ledger::new` has 0 skipped lines, 200 low entries, 4 summaries, no `measured` tier, no `fidelity_pair` | exact |
| R1 | (6) | S7 and S1 runs at workers 1 and 8: public and private ledger bytes | equal |
| R2 | (6) | existing: `cargo test -p rcfd-api --release --lib design::`, `--test f697_fan_objective`, `--test fan_rom_bands_f268`, `--test f249_default_bits`, `--test f260_default_bits`, `--test f297_default_bits`, `--test baseline_hashes`; rcfd-mcp `default_f1`, `tool_list_f275`, `project_list_f270`; `git diff 6400c929 -- crates/rcfd-mcp` | pass; empty diff; no existing test file edited |
| R3 | — | `cargo fmt -p rcfd-api --check`, `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

The S3 heavy test (about 150 s) is `#[ignore]`d as a heavy record and run once; B6 is its fast counterpart on the same code path.

Mutations (scratch copy, one at a time, each must fail a registered check):

| Id | Mutation | Expected to fail |
|---|---|---|
| M1 | the constant `b` computed as `mean(yL - yH)` | T4 |
| M2 | the correction applied twice (`predict` on its own output) | A2 |
| M3 | `rho` dropped at R2 (`yL` with coefficient 1, fit on `yH - yL`) | A1 |
| M4 | leave-one-out factor dropped at R2 (`e_i = r_i`) | L1 |
| M5 | stated uncertainty ignored (`u_rms` = 0) | T1 |
| M6 | explore picks replaced by exploit picks | E1 |
| M7 | `n >= n_pub` replaced by `n > n_pub` | P2 |
| M8 | aggregates not rounded | P2 |
| M9 | region test removed (every query inside) | T4 |
| M10 | public-inside-private check removed | P3 |
| M11 | public summary written with the full content | P1 |
| M12 | low entries appended in completion order (workers push into a shared list) | R1 |

### Decision rule

* **done:** A1 to A3, L1, E1, B1, B2, NC1 (missing as it must), B4 to B6, T1 to T5, T4b, NC2 (trusted as it must), P1 to P5, NC4
  (found as it must), R1 to R3 met, and every mutation fails a check.
* **partial:** A, L, E, T, P and R met and a B limit missed (an ignored `*_known_miss` record with the limit intact), or NC1 does not
  miss, or a mutation survives.
* **todo** (the lead does not take the code): an A, L, E, T, P or R check missed.

### Limits (not covered)

* Library only (D1): no MCP tool, service method or case key (F-702).
* Rungs R0 to R2 only; R3 (a Gaussian process on the R2 residual) reuses F-698 and is a follow-up (Q3 (a)).
* Every check uses synthetic high sources. S3's high tier has the structure of the low one, so B shows the driver works, not that
  the reduced-order model can be corrected to a real fan (risk 4). On S3 the perturbation moves the level of the losses, not the
  location of the best, so B2 does not tell the correction from no correction (smoke 4).
* With three parameters and h_max 0.25 most of the screened set lies outside the calibrated region and gets the constant only
  (smoke 4); the box-and-distance region is crude (risk 2).
* The privacy split is a path rule and a content rule of this driver, not access control (risk 7). The pick pattern is private,
  but the public low entries show which variants were screened (risk 6). Public aggregates are published at most once per block
  of 5 new pairs, so differencing two publications isolates at least 5 pairs, not one.
* Corrected metrics are corrected independently; identities between them are not reported (the stub's D4 note is not built).
* `MeasuredTable` maps a candidate to a row by a caller-given function; several rows per variant (it takes the first), matching by
  tolerance and fan-law scaling between speeds (Q14) are not built.
* No check runs the driver with an F-697 `ObjectiveModel` as the low model (it is a `ledger::Model`, so it plugs in; not run).

## Amendment 2 (2026-11-16, after the first registered run, before the corrected runs): two defective set-ups

The registered run (`cargo test -p rcfd-api --release --test f699_fidelity -- --test-threads=2`, log `f699-scratch/run1.log`)
passed every check except two, both from defects of the test set-up, not of the driver. Diagnosed (a scratch copy of the test
file with one diagnostic test, `f699-scratch/zz_f699_diag.rs`, never committed):

1. **P1 (and with it NC4 and P5, which did not run: they follow P1 in the same test).** Measured: 632 values, 1260 texts searched,
   24 found. Every found text is the `pred` of a round-1 pair, which is bit-equal to that pair's `low` value (12 values in two
   text forms): before any correction exists the prediction is the uncorrected low value, and the low values are public by
   design (D7 table). The registered search set ("every `pred` of the private pairs") wrongly included uncorrected predictions,
   while for the final predictions the test already excluded those equal to the low value. Corrected set-up: a pair's `pred` is
   searched only when it is not bit-equal to the pair's `low` value (the same rule as for the final predictions). Every other
   value, the limits (0 found, at least 400 searched), NC4 and P5 are unchanged.
2. **R1 for S7.** Measured: public 130872 and 130872 bytes, equal; private 201673 and 201673 bytes, not equal. The two runs used
   different directories (`r1s7a`, `r1s7b`), and the private summaries record the spec including the private ledger path
   (D6: provenance holds the full spec; the path is written only to the private ledger), so the private files differ in the path
   text. Corrected set-up: both runs use the same directory; the first run's bytes are read, the directory is removed, and the
   second run writes the same paths. Limit unchanged (bytes equal).

The two original tests stay as ignored records with their names and the measured numbers above; the corrected tests are new
(`p1_nc4_p5_corrected`, `r1_s7_corrected`) and are run once.
