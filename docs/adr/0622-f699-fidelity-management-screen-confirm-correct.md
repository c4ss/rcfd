# ADR 0622: F-699 proposal: fidelity management (H7): screen with the reduced-order fan model, confirm with a higher tier, log the disagreement and correct the screening

Status: **DRAFT STUB 2026-11-15. Not registered, not accepted.** The maintainer answered the open questions on 2026-11-15 (section \"Maintainer decisions\"); the limits are still proposals until the implementing agent registers them. No code of F-699 exists and no check has run. Nothing in
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
