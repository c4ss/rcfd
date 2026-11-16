# ADR 0623: F-268 remaining scope registration: automatic ledger logging from design studies and uncertainty bands on the reduced-order fan

Status: **REGISTERED 2026-11-15, written before any code of F-268's remaining scope and before any run of its checks.** Not yet
accepted by the maintainer. This text replaces the draft stub of 2026-11-15 (commit 9cf95fc3); the stub's survey, the
maintainer's decisions and the open questions as asked are kept below (sections "What exists", "Maintainer decisions", "Open
questions as asked"). Before this text no library or test code of this scope was written in the branch. Disclosed: two smoke
runs on the existing code (a temporary test calling `FanRomEvaluator::metrics` through `uq::run` with an `FnModel`, kept in
`w49i-work/f268r-scratch/zz_f268r_smoke.rs`, logs `smoke.log`, `smoke2.log`; it was never committed and computes no check of
the table, and its seeds 7, 11, 21 are not used by any check). Its numbers are quoted in "Arithmetic done beforehand". Results go
in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-268 (deps F-249). Parents: [ADR 0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md) (ledger and
tolerance driver), [ADR 0240](0240-f268-registration-sobol-sensitivity-indices.md) (Sobol indices),
[ADR 0118](0118-wave46a-computer-fan-schema-checker-perturbations.md) (fan tolerances, set-up T),
[ADR 0119](0119-wave46b-reduced-order-axial-fan-tier.md) (reduced-order fan). Number 0623 given by the lead. Branch
`f268r-work` from local main 9cf95fc3, worktree `w49i-work/f268r`, scratch `w49i-work/f268r-scratch`. F-700 (new optional
ledger fields, pairing and training-set queries, `Fidelity::Measured`) is a separate item and is not touched here.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run is made once; existing tests not
edited; defaults bit-identical (with the one disclosed exception of D4 below, argued from the binding acceptance); no edit of
`baseline_hashes.rs`, of the case and mesh hash tables or of any golden; no new dependency, no unsafe code; clean-room. Library
only: no MCP tool, no service argument, no case-file key (decision D1 of 2026-11-15). Every choice the maintainer would
normally be asked about is taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation), unless the
maintainer's decisions of 2026-11-15 below already settle it, or it is listed as open.

## What F-268 asks (remaining scope)

The acceptance, binding:

> Every limit not stated here is registered in ADR 0623 before the run. (a) Logging: a design study run with the opt-in ledger
> sink writes exactly one evaluation entry per evaluated (candidate, condition) pair, failed ones included with a located reason:
> 64 of 64 on the registered 32-candidate, 2-condition study; a resumed study adds 0 duplicate and leaves 0 missing entries at
> every one of the registered kill points; rerun_check of every ok entry of a deterministic evaluator passes with difference 0;
> the study store file and the study result are byte-identical with and without the sink. (b) Bands: uq::run on the
> reduced-order fan through a ledger::Model adapter reports the 5, 50 and 95 percent quantiles with 0.95 order-statistic
> intervals for q, dp_s, power and tone1_pa; the adapter equals FanRomEvaluator::metrics bit for bit at 64 points; at least 10
> of the 12 intervals contain the value of an independent 100000-sample reference; zero-width tolerances give a standard
> deviation of exactly 0; the ledger bytes are identical for 1 and 4 workers. (c) Defaults: with the sink and the adapter
> unused, no existing test, hash table or golden changes.

Clauses: **a1** 64 of 64 entries; **a2** 0 duplicate / 0 missing at every kill point; **a3** rerun_check of every ok entry,
difference 0; **a4** store bytes and result bits equal with and without the sink; **b1** quantiles 5/50/95 with 0.95 intervals
for the four metrics; **b2** adapter = `FanRomEvaluator::metrics` bit for bit at 64 points; **b3** at least 10 of 12 intervals
contain the 100000-sample reference; **b4** zero-width tolerances give sd exactly 0; **b5** ledger bytes equal for 1 and 4
workers; **c1** defaults.

## What was read

`AGENT_RULES.md`; the brief; `loop-drafts/decisions.md` (D1 to D6, B1 to B4); the stub of this ADR; ADR 0121 (D6: quantile
`X_(ceil(n p))`, order-statistic interval with exact binomial coverage for independent samples); ADR 0118 (set-up T of the
tolerance checks, `crates/rcfd-mesh/tests/fan_design_w46a.rs` `tol_t`); ADR 0119 and `crates/rcfd-post/src/fan_rom.rs`
(`RomFan`, `RomModel`, `operating_point`; thickness is not modelled); `crates/rcfd-api/src/design/` `study.rs` (`Study`,
`eval_points`, `record`), `store.rs` (`Store::append` writes one batch with one `write_all`; `Store::open` truncates a torn
last line), `ledger.rs` (`Entry`, `Ledger::append`, `Model`, `rerun_check`, `compare_metrics`), `uq.rs` (`run`, `mean_sd`,
`quantile_interval`, `Dist`), `fan_rom_eval.rs`; `crates/rcfd-api/tests/fan_rom_design_w46b.rs` (`set_up`, `evaluator`);
`crates/rcfd-mesh/src/fan_design.rs` (`Tolerances`, `Dist::{Fixed, Uniform, TruncatedNormal}`).

## What exists (read in the source on 2026-11-15)

- Only `uq::run`, `sensitivity::run` and the service method `design.ledger_append` write ledger entries. `Study::run`,
  `Study::record`, `design.evaluate`, `design.run_study` and the operating maps write the per-study store only. So the
  acceptance's "every design variant is recorded" holds only for entries a caller appends by hand.
- `ParamDef` has no unit; `CondResult` has metric values by name without units, a status text and a message.
- `FanRomEvaluator` implements `study::Evaluator` with metrics `q`, `dp_s`, `dp_t`, `power`, `torque`, `eta_s`, `eta_t`,
  `tone1_pa`; no `ledger::Model` exists for the fan, so `uq::run` cannot run on it.
- `uq::Dist` refuses zero widths (`lo < hi`, `sd > 0`), and `mean_sd` of `n` bit-identical values returns `fl(sum)/n`, which is
  not the value in general (smoke: 13 of 10000 random values in [0.01, 1000] at n = 2000 come back exact), so the sd of a
  constant sample is rounding noise, not 0.
- A pre-existing defect found while planning the kill points: a kill during the very first `Store::append` (the header line torn)
  leaves a file that `Store::open` truncates to 0 bytes; the next `append` sees an existing file and writes no header, and the
  store can no longer be opened ("not a design store"). Not fixed here (it is store behaviour, not ledger); that one kill point
  is left out of LG5 and listed as a follow-up.

## Arithmetic done beforehand (disclosed)

1. **Cost of the reduced-order fan** (smoke, set-up below, release): 127 ms per evaluation at `n_scan = 100`, 84 ms at 20
   (single thread; the first smoke took 264.8 s for 2000 evaluations). The 100000-sample reference therefore costs about
   `1e5 x 0.13 s / 4` = 55 min with 4 workers; it is an ignored heavy record (BD2/BD3), run once, with BD7 the fast test of the
   same code path. `n_scan = 100` of the ADR 0119 set-up is kept (no tuning of the set-up for speed).
2. **Linearity of the fan at set-up T** (smoke): for each input and metric, the ratio `|f(+s) - 2 f(0) + f(-s)| / |(f(+s) -
   f(-s))/2|` at one sd `s` is at most 0.031 for `q`, `dp_s`, `power` and `tone1_pa` (0.092 for `eta_s`, not a band metric).
   For `f = g x + c x^2 / 2` with `x ~ N(0, s^2)`, `Var f = g^2 s^2 + c^2 s^4 / 2`, so the relative sd excess of the linear
   propagation is about `r^2 / 4` with `r` that ratio: at most `0.031^2 / 4 = 2.4e-4`. The central difference with step `s`
   has a truncation error of the same order. This fixes BD3's model allowance at 0.01 (40 times the estimate).
3. **Dominance of the common stagger** (smoke, `g s` per input): the stagger carries 99.4 % of the linear variance of `q`,
   98.7 % of `power`, 99.1 % of `tone1_pa`, and `dp_s = k q^2` on the system curve. So the four band metrics are nearly
   comonotone functions of one input, and the 12 coverage events of b3 are close to 3 events (one per quantile level, the 5 and
   95 % ones nearly independent: order-statistic correlation `sqrt(0.05^2/0.95^2) = 0.05`, the median and a tail `0.23`) each
   repeated 4 times. With per-interval coverage `>= 0.95` (exact for Monte Carlo) and a reference whose own quantile error is
   `sqrt(2000/100000) = 0.14` of the interval's sigma (effective coverage about `P(|Z| < 1.96 / 1.01) = 0.948`), the
   probability that at least 10 of 12 contain the reference is about `0.948^3 = 0.85` (if a cluster misses, 4 intervals miss
   together, and 8 of 12 follows). The limit 10 of 12 is binding and kept; this calculation is the disclosed estimate that B3
   asks for: a correct implementation fails b3 with a probability of about 0.15. A miss is read against this: a miss of exactly
   one quantile level across all four metrics is the expected failure mode; a miss in a pattern that is not clustered points to
   a defect.
4. **BD3 sampling allowance.** The relative standard error of a sample sd from `n` values is `sqrt((kappa - 1)/(4 n))` with
   `kappa` the kurtosis; the truncated normals and the near-linear map give `kappa <= 3`, so at most `1/sqrt(2 n) = 0.0158` at
   n = 2000; 4 SE = 0.063.
5. **BD6 failed fraction.** With the tip-gap error uniform on `[-1.5, 0]` mm and the nominal gap 1 mm, a sample fails
   (negative gap, `RomFan::validate`) exactly when the error is below `-1` mm: probability `0.5 / 1.5 = 1/3`.
6. **LG set-up strata.** `sampling::latin_hypercube` puts exactly one point in every stratum `[k/32, (k+1)/32)` of each
   dimension, so a failure rule on `floor(32 x)` makes exactly the registered number of candidates fail.

## Decision

### D1. Ledger sink for design studies (opt-in, library; Q4 (a), Q5 (a), Q6 (a), Q7 (a))

New module `crates/rcfd-api/src/design/study_ledger.rs`:

- `LedgerSink<'l> { ledger: &'l Ledger, evaluator, evaluator_version, fidelity, param_units, metric_units }` (unit maps
  name -> unit text).
- `Study::set_ledger_sink(&mut self, sink) -> Result<usize, String>` (`study.rs`). Refused, located, before anything is
  written, when a parameter of the definition or a metric of an objective or constraint has no unit in the maps. Then it
  **reconciles**: it reads the ledger and appends, in store order, the entry of every (store record, condition) pair of the
  study that has no entry yet (an entry is the study's when `study` is the definition's name and `provenance.driver` is
  `study`; it is that pair's when `provenance.store_key` and `provenance.condition` match). It returns the number appended.
- In `Study::eval_points`, after `store.append` of a batch has returned `Ok`, the entries of the new records (record order,
  then condition order) are built and appended in **one** `Ledger::append`. In `Study::record` the same after its store append.
  A build or append error stops the study with `Stop::Error("ledger: ...")` (Q7 (a)); the batch is then in the store and
  reconciled on the next `set_ledger_sink`. Records served from the store write nothing. Nothing in `Study` changes when no
  sink is set (the field is `None`, every new line is behind it).
- Entry of record `r` at condition `c`: `kind = evaluation`, `study` = definition name, `variant = "<name>#<index>@<label>"`,
  `params` in definition order with units, evaluator id/version/fidelity from the sink, `settings = {"condition": label,
  "set": {path: value, ...}}`, `metrics` from the `CondResult` (name order) with units, `verdict` = `ok` when the result is ok,
  `constraint_rejected` / `not_evaluated` when its status says so, else `diverged`; `reason = "condition '<label>': <status>:
  <message>"` when not ok; `seed` and `wall_s` unset; `provenance = {"driver": "study", "store_index", "store_key",
  "condition", "source", "status", "iterations"}`. No new top-level key (F-700, D5).
- `ConditionModel<'m>` implements `ledger::Model` for one condition of a study (the definition, the `Evaluator`, the sink's
  id/version/fidelity/metric units): parameters by name in definition order, the evaluator run on a one-condition copy of the
  definition, the same non-finite rule as `eval_points`. `rerun_check(entry, &ConditionModel)` re-runs a study entry.

### D2. Reduced-order fan as a `ledger::Model` (opt-in, library; Q3 (a))

In `fan_rom_eval.rs`, `FanRomModel { fan: RomFan, model: RomModel, cond: RomCondition }`, id `rcfd.fan_rom`, fidelity
`reduced_order`. Parameters (any subset, each at most once, a missing one is 0): `stagger_common_deg` [deg] added to every
section's stagger (radians), `chord_rel` [1] multiplying every chord by `1 + x`, `tip_gap_delta` [m] added to the tip gap. An
unknown name or another unit is `not_evaluated` with a located reason. Metrics: `FanRomEvaluator { model, base: cond
}.metrics(perturbed fan, &cond)` (the same function, so b2 tests the mapping), units `q` m3/s, `dp_s`, `dp_t`, `tone1_pa` Pa,
`power` W, `torque` N m, `eta_s`, `eta_t` 1. A failed fan or operating point is `not_evaluated` with reason `fan_rom: <error>`
(as the stub proposed). `geometry_hash` = FNV-1a 64 (Fowler, Noll, Vo; public-domain definition: offset basis
0xcbf29ce484222325, prime 0x100000001b3) over the bit patterns of the perturbed fan, the model and the condition, so
`rerun_check` notices another nominal set-up under the same id and version. `settings` = the three deviations and the condition.

**Tolerance mapping (Q3 (a)).** `fan_rom_tolerances(&Tolerances)` maps `stagger_common_deg`, `chord_rel` and `tip_gap_delta`;
`stagger_blade_deg`, `thickness_rel`, `eccentricity`, `mass_rel` and `roughness_ks` must be `Fixed { value: 0 }`, else a
located refusal naming the field (the one-row model has no per-blade, eccentricity, mass or roughness input, and thickness is not
modelled by ADR 0119, so Q3 (a)'s "thickness" cannot be used; recorded as a reading). Reading of "row-mean chord": the per-blade
`chord_rel` distribution is applied as one error common to all blades, an upper bound on the spread of the row mean (which is
about `1/sqrt(B)` of it for independent blades); listed as open question R1.

**Standardised inputs (for b4).** `uq::Dist` cannot have zero width, and changing its validation would change a refusal that
existing callers see. So a generic wrapper `uq::Standardized<'m>` (a `Model`) maps inputs `z.<name>` [1] to the inner model's
`<name> = offset + width z` (`uq::Affine { name, unit, offset, width }`), id `<inner id>+standardized`, version `<inner
version>; <name>=<offset bits>,<width bits>; ...`. A fan_design `Dist` becomes: `Fixed{v}` -> offset v, width 0, `z ~
Uniform(0, 1)`; `Uniform{lo, hi}` -> offset 0, width 1, `z ~ Uniform(lo, hi)`; `TruncatedNormal{mean, sd, lo, hi}` -> offset
mean, width sd, `z ~ Normal(0, 1)` truncated to `[(lo - mean)/sd, (hi - mean)/sd]` (both renormalised truncations, ADR 0118
and ADR 0121). Zero-width tolerances (every field `Fixed{0}`) then evaluate the nominal fan in every sample (`0 * z` is
`+-0`, and `x + (+-0) = x`, `x (1 + (+-0)) = x`).

### D3. Bands

`fan_rom_band_spec(name, &Tolerances, n, seed, workers)` gives the `UqSpec` (Monte Carlo, one batch of `n`, quantiles
`[0.05, 0.5, 0.95]`, confidence 0.95, no stopping on the SE) and the affine maps; `fan_rom_bands(&FanRomModel, name,
&Tolerances, n, seed, workers, ledger)` runs `uq::run` on `Standardized(FanRomModel)`. The band of a metric is its 5 and 95 %
quantiles with the median, each with its order-statistic interval, together with the failed fraction `1 - n_ok / n_samples`
(statistics are over the ok samples). No new statistics. The bands are the reduced-order model's response to the stated input
spread (Q2 (a): manufacturing tolerances only), nothing about a real fan.

### D4. `uq::mean_sd` of a bit-constant sample (the one default change, disclosed)

When every value of the sample has the same bit pattern, `mean_sd` returns that value and sd 0. This is required by b4 and is a
first-principles identity (the mean of identical values is the value, the deviations are 0). It changes the output of
`uq::run` only for a metric whose usable samples are all bit-identical, where the old output was rounding noise (sd of order
`1e-16 |y|`, mean off by a few ulps); also `metric_ses` then gives SE 0 for such a metric. Every other sample takes the old
code path unchanged. DF1 checks that no existing test changes. Listed as open question R2 (keep, or revert and record b4 as a
known miss).

## Set-ups (fixed now)

- **LG study.** Parameters `x` in [0, 1] unit `m`, `y` in [0, 1] unit `1`; conditions `c_lo` (`set c = 0.25`) and `c_hi`
  (`set c = 0.75`); objective `f` min mean; metrics `f = (x - c)^2 + 0.01 y` [m2], `g = x c` [m]. Failure rule on `s =
  floor(32 x)`: `s` in 0..=3 both conditions `status diverged`, message `analytic: x < 1/8 diverges`; `s` in 4..=7 both
  `status not_evaluated`, message `analytic: x in [1/8, 1/4) not evaluated`. Method `Doe::Lhs { n: 32, seed: 623_001 }`. So 32
  candidates, 64 pairs, 16 failed pairs (8 candidates). Ledger clock fixed at 1_700_000_000_000; evaluator `analytic.lg`, version
  `1`, fidelity analytic.
- **LG5 study.** The same definition and evaluator, `Method::NelderMead { budget: 16, restarts: 0, seed: 623_005 }`, workers 1,
  file store and file ledger. A write event is one `Store::append` or one `Ledger::append` inside `eval_points`; `E` = the
  number of events of the uninterrupted run (determined by the method, read from that run, not chosen). Kill points: for every
  event `e` in 1..=E, (i) killed before `e` (the write not made) and (ii) killed during `e` (the write not made and a torn
  fragment `{"format":"rcfd-` without newline appended to the file of that event), except (ii) at the first store write (the
  pre-existing store defect above). The kill is a test-only hook (`#[cfg(test)]` counter in `Study`) that returns
  `Stop::Error` at the event; the `Study` is then dropped and a new one is built over the reopened files, `set_ledger_sink`
  called, and the method run to the end. Count: `2 E - 1` kill points.
- **Fan set-up.** `set_up()` of `fan_rom_design_w46b.rs` (7 blades, free-vortex design to 45 Pa at Q_D = 0.045 m3/s, 3000
  rpm, system `k` through the design point, `l = 0`), condition `c0 = 343`, observer 1 m at 90 deg, `n_scan = 100`,
  `RomModel::default()`. **Tolerances T3**: set-up T of ADR 0118 for the three representable fields
  (`stagger_common_deg` TN(0, 0.5, -1, 1), `chord_rel` TN(0, 0.01, -0.015, 0.02), `tip_gap_delta` TN(0, 1e-4, -2e-4, 2e-4)),
  the other five `Fixed{0}`.

## Checks (limits fixed now)

| Check | Clause | What | Limit |
|---|---|---|---|
| LG1 | a1 | LG study with the sink, workers 1 | exactly 64 `evaluation` entries of the study, 0 skipped lines; one per (store key, condition); 16 non-ok, each with a reason naming its condition and the evaluator message; every entry's params (values and units), `settings.condition`, `settings.set`, metrics (values and units) equal the store record and the unit maps |
| LG2 | a4 | LG study and LG5 study each with and without the sink (separate files) | store file bytes equal; `OptResult` x bits, fitness bits and evaluations equal |
| LG3 | a3 | `rerun_check(e, ConditionModel, 0, 0)` for every ok entry of LG1 (48) | 48 of 48 pass, largest difference exactly 0; the 16 non-ok entries re-run to the same verdict |
| LG4 | a1 | LG1's non-ok entries | 8 `diverged` and 8 `not_evaluated`; equal to the store's failed (record, condition) counts |
| LG5 | a2 | LG5 study killed at each of the `2E - 1` kill points, then resumed | at every point: 0 duplicate and 0 missing (store key, condition) pairs against the resumed store; the set of valid entry lines equals the uninterrupted run's; at most 1 skipped line (the fragment) |
| LG6 | a2 | the finished LG1 study: new `Study` over its store, `set_ledger_sink`, run again | 0 entries appended (reconcile 0, run 0) |
| LG7 | a1 | LG1 with workers 1 and 4 | ledger bytes equal |
| LG8 | Q6 (a) | sink without a unit for `y`; then a sink without a unit for metric `g` | located refusal naming `y` before any evaluation and no ledger file; the run with `g` missing stops with an error naming `g` and the ledger holds 0 entries |
| BD1 | b2 | `FanRomModel` (with `RomModel { annulus_drag: 0.024, secondary_drag: 0.025, ..default }`) against `FanRomEvaluator::metrics` of the fan perturbed in the test, 64 LHS points (seed 623_101) over the T3 truncation boxes | 64 of 64 points, every one of the 8 metrics bit for bit, same unit set |
| BD2 | b1, b3 | `fan_rom_bands` on T3, n = 2000, seed 623_102, workers 4, ledger; reference: the same with n = 100000, seed 623_200, no ledger | the result has the quantiles 0.05, 0.5, 0.95 with finite intervals of coverage >= 0.95 for q, dp_s, power, tone1_pa; at least 10 of 12 intervals contain the reference's quantile; 2001 ledger lines (2000 evaluations, 1 summary) |
| BD3 | — | sd of BD2's four metrics (n = 2000) against first-order propagation: central differences of `FanRomModel` with step one parent sd per input, times the truncated-normal sd of each input (closed form) | relative difference at most 0.01 + 4 / sqrt(2 x 2000) = 0.0732 for each of the four |
| BD4 | b4 | every tolerance `Fixed{0}`, n = 64, seed 623_104, workers 4 | sd of each of the 8 metrics exactly 0.0; every quantile value and interval end bit-equal to the nominal metric (`FanRomEvaluator::metrics` of the nominal fan) |
| BD6 | — | T3 with `tip_gap_delta` Uniform(-1.5e-3, 0), n = 120, seed 623_106, workers 4, ledger | `n_samples - n_ok` = number of non-ok evaluation entries, each reason containing `tip_gap`; at least one ok and one failed; 1/3 inside the 0.999 Clopper-Pearson interval of the failed fraction |
| BD7 | b5 | `fan_rom_bands` on T3, n = 64, seed 623_107, workers 1 and 4, fixed clock | ledger bytes equal (65 lines) |
| DF1 | c1 | sink and adapter unused | no existing test file edited; existing tests pass: the `design::` unit tests of rcfd-api (ledger, uq, sensitivity, study, store, tools), `fan_rom_design_w46b`, and the rcfd-mcp tests (tools-list goldens), in release; no golden or hash table edited |

BD2 and BD3 share one ignored heavy test (about 1 h), run once; BD7 is the fast test of the same path. All others are fast.

Negative control **NC1**: the sink writing the ledger before the store fails LG5 at one kill point at least (a kill between the
two writes leaves an entry whose record the resume evaluates again). Run as a mutation on a scratch copy.

Mutations (scratch copy; each must fail a check): M1 one entry per candidate (first condition only) -> LG1; M2 `settings`
without the overrides -> LG1; M3 reconcile appending every pair whether present or not -> LG6, LG5; M4 unit maps ignored (unit
`1`) -> LG1; M5 the adapter using `RomModel::default()` instead of its model -> BD1; M6 the chord mapping dropped from the
adapter -> BD1; M7 `Standardized` ignoring the width (width 1) -> BD4; M8 the constant-sample rule of D4 removed -> BD4.

## Decision rule

**done** when LG1 to LG8, BD1 to BD4, BD6, BD7 and DF1 are met, NC1 holds and every mutation fails a check; **partial** when
DF1, LG2 and BD1 hold but another limit is missed (a missed limit is an ignored `*_known_miss` record); **todo** (the lead does
not take the code) when DF1 or LG2 fails.

## Limits (not covered)

- Only studies run through `Study` in the library get the sink; `design_evaluate`, `design_run_study`, the operating maps and
  the project runner do not (Q4 (a); the service and MCP arguments are F-702).
- Two files, two truths: a ledger edited by hand, two studies with one name on one ledger, or two writer processes are not
  covered; units come from the caller's maps and a wrong unit is recorded faithfully.
- Bands describe the reduced-order model with unverified loss constants under T3; per-blade, eccentricity, mass, roughness and
  thickness tolerances are not representable; the chord reading over-states the row-mean spread.
- The kill is simulated by a hook at the write calls and a torn fragment, not by a killed process; a kill inside the OS write is
  represented by the fragment only.
- Not done here: BD5 of the stub (doubled widths; implied by BD3's linearity), BD8 (Q2 (b) not taken), ADR 0240 Q2 (F-339).

## Maintainer decisions (2026-11-15)

The maintainer answered the open questions of the design-loop drafts (ADRs 0620 to 0624) on 2026-11-15, after the stub was
written. Shared decisions:

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

Every other open question of this ADR takes the recommendation written with it: Q1 (b), the split to F-700 (D6); Q2 (a); Q3
(a); Q4 (a), D1; Q5 (a); Q6 (a); Q7 (a); Q8 (b), D4; Q9 (b), D5; Q10 D2 (library only); Q11 (c) with (b) per entry, D3; Q12
B4; Q13 (a). BD2 and BD3 fall under B3 (arithmetic 2 to 4 above). Stub negative control NC2 (a mapping that drops the tip gap
fails BD3 on `q`) is replaced: by arithmetic 3 the tip gap carries 0.25 % of the variance of `q`, so BD3 cannot see it; the
mapping is checked bit for bit by BD1 instead (M6). Stub NC3 (reference with sd times 1.1) is not run: by arithmetic 3 its
outcome is dominated by the clustering, so it would not separate a defect from chance.

## Open questions as asked in the stub (kept for the record; answered above)

Q1 amend or split (b); Q2 meaning of bands (a); Q3 tolerances the one-row model cannot represent (a); Q4 which runs (a); Q5
granularity (a); Q6 units (a); Q7 failed ledger write (a); Q8 ADR 0136 (b); Q9 where new fields live (b); Q10 measured data
(library only); Q11 proprietary data ((c) with (b)); Q12, Q13 ADR 0240 Q4, Q5. The stub's P1 to P8 (ledger fields and queries
for F-697 to F-699) are F-700.

## References

- ADR 0121 and ADR 0240 and their references (order-statistic intervals; Clopper and Pearson 1934).
- JCGM 100:2008, Guide to the expression of uncertainty in measurement (first-order propagation, used in BD3).
- G. Fowler, L. C. Noll, K.-P. Vo, the FNV hash (public-domain definition; constants quoted from memory and stated above,
  used only as a change detector).
- M. C. Kennedy, A. O'Hagan (2001), Bayesian calibration of computer models, J. R. Statist. Soc. B 63 (model discrepancy, why
  the bands are not a statement about a real fan). Pages UNVERIFIED.

## Amendment 1 (2026-11-15, before any run of BD4): BD4's sample size

Found while writing the BD4 test, before it ran: with n = 64 the order-statistic interval of the 0.05 quantile at confidence
0.95 has no lower end (`P(B <= 0) = 0.95^64 = 0.0375 > 0.025`, so `lo = -inf`), and that of the 0.95 quantile no upper end;
BD4's "every interval end bit-equal to the nominal metric" could not hold for a defect-free implementation. The smallest n with
finite ends is 72 (`0.95^n <= 0.025`). BD4 is corrected to **n = 128** (seed 623_104, workers 4 unchanged; `0.95^128 =
1.4e-3`); the limit is unchanged. LG1 to LG8 had run (once) before this amendment; no BD check had run.

## Addendum 1: results (2026-11-16)

Code: commit 6643b85b. New `crates/rcfd-api/src/design/study_ledger.rs` (D1, with `study_ledger_tests.rs`); `study.rs` (the
`sink` field, `set_ledger_sink`, `log_records`, the `#[cfg(test)]` kill hook; lines 15, 408-417, 437-443, 447-479, 553-561,
855: additions only, every one behind `sink: None` or `cfg(test)`); `uq.rs` (D4 at lines 354-358, `Affine` and
`Standardized` at 781-887); `fan_rom_eval.rs` (`FanRomModel`, `fan_rom_tolerances`, `fan_rom_band_spec`, `fan_rom_bands`,
lines 112-404, plus imports); `mod.rs` (module line and doc); new `crates/rcfd-api/tests/fan_rom_bands_f268.rs` (BD1 to BD7).
No shared solver file, no golden, no hash table, no case-file key, no MCP tool. All runs in release.

| Check | Limit | Measured | Met |
|---|---|---|---|
| LG1 | 64 entries, 0 skipped, one per pair, 16 non-ok with located reason, content equal to the store and the unit maps | 64 entries, 64 unique pairs of 64, 0 skipped, 8 failing candidates (set-up), 0 content problems | yes |
| LG2 | store bytes and result bits equal with and without the sink | DOE: 14346 = 14346 bytes, result equal; NM: 6955 = 6955 bytes, result equal; no ledger file without the sink | yes |
| LG3 | 48 of 48 ok entries pass `rerun_check` with difference 0; non-ok re-run to the same verdict | 48 of 48, largest difference 0; 16 of 16 same verdict | yes |
| LG4 | 8 diverged, 8 not_evaluated, equal to the store's failed pairs | 8, 8, store 16 | yes |
| LG5 | 0 duplicate, 0 missing, same entry set, at most 1 skipped line at all `2E - 1` kill points | E = 28 write events, 55 kill points, 0 failing points (32 reference entries) | yes |
| LG6 | 0 new entries on rerunning the finished study | reconcile 0, entries after the rerun 64 | yes |
| LG7 | ledger bytes equal for 1 and 4 workers | 46841 = 46841 bytes | yes |
| LG8 | located refusal naming `y`, no file; run without a unit for `g` stops naming `g`, 0 entries | "parameter 'y': no unit in the ledger sink", no file; "ledger: metric 'g': no unit ...", 0 entries | yes |
| BD1 | 64 of 64 points bit for bit (8 metrics, units) | 64 of 64 | yes |
| BD2 | quantiles 0.05/0.5/0.95, finite intervals, coverage >= 0.95; at least 10 of 12 contain the reference; 2001 ledger lines | coverage 0.9600 (tails), 0.9534 (median); **12 of 12**; 2001 entries, 0 skipped; 2000 of 2000 and 100000 of 100000 ok; 66 s band, 3763 s in all | yes |
| BD3 | relative sd difference at most 0.0732 | q 0.0265, dp_s 0.0263, power 0.0266, tone1_pa 0.0266 | yes |
| BD4 (amendment 1, n = 128) | sd exactly 0, every quantile and interval end bit-equal to the nominal | sd 0.0 for all 8 metrics, 128 of 128 ok, 0 failing | yes |
| BD6 | failed = non-ok entries, every reason naming `tip_gap`, at least 1 ok and 1 failed, 1/3 in the 0.999 Clopper-Pearson interval | 36 failed of 120 = 36 non-ok entries, 36 naming tip_gap, fraction 0.300, interval [0.174, 0.451] | yes |
| BD7 | band ledger bytes equal for 1 and 4 workers | 104556 = 104556 bytes, 65 lines | yes |
| DF1 | existing tests pass, nothing edited | rcfd-api `design::` unit tests 65 passed (3 ignored, pre-existing), `fan_rom_design_w46b` 1 passed; rcfd-mcp lib 18, `default_f1` 2, `design_f249` 4, `design_f260` 2, `design_f297` 2, `tool_list_f275` 7 (tools-list goldens) passed; no existing test, golden or hash table touched | yes |

BD3: all four relative differences are 0.026 to 0.027 and of one sign (sample sd below the first-order value), consistent with
one sampling deviation of the dominant stagger input shared by the four near-comonotone metrics (arithmetic 3), not four
independent errors; within the limit, not a finding.

Negative control and mutations (scratch copy `f268r-scratch/mut`, script `mutate.py`, logs `mut_*.log`): **NC1** (ledger
before store) fails LG5; **M1** fails LG1 and LG5; **M2** LG1; **M3** LG1 (its LG6 part) and LG5; **M4** LG1; **M5** BD1 (0 of
64); **M6** BD1 (0 of 64); **M7** BD4 (126 of 128 samples fail, sds up to 0.57 Pa); **M8** BD4 (sds of rounding size, `dp_s`
2.6e-14 Pa, not 0). Nine of nine caught.

fmt: `cargo fmt -p rcfd-api --check` clean. clippy: `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean.

Deviations and run record:
- Each registered check ran once, except that the LG tests ran a second time inside DF1's `design::` run (same results;
  deterministic, fixed clock). The heavy BD2/BD3 record ran once, to completion (3766 s); it was not killed.
- **Re-runs because a process was killed or blocked** (mutations, not registered checks): the first attempt of M5 to M8 could
  not link its test binary while the heavy BD2/BD3 binary was running (Windows lock on the executable of the same name,
  LNK1104); the second attempt was killed when the session ended (M5 had completed and failed BD1; M6 was killed mid-run and
  had left its mutation in the scratch copy, restored from the worktree before continuing); M6 to M8 were then run a third time,
  to completion. No result was chosen among runs: the earlier attempts produced no test result.
- The smoke runs before registration are disclosed in the status line; nothing else ran before the registration commit.
- Not run: `baseline_hashes` (no solver file changed), the workspace suite (the lead's gate), rcfd-mcp tests other than the six
  listed.

Decision by the registered rule: **done**. Every check met (LG1 to LG8, BD1 to BD4, BD6, BD7, DF1), NC1 holds and every
mutation fails a check.

Remaining questions (each with a recommendation; none blocks the decision):
- **R1, chord reading.** The per-blade `chord_rel` of `Tolerances` is applied as one row-common error, an upper bound on the
  row-mean spread. (a) keep; (b) map it to the row mean of `B` independent draws (sd over `sqrt(B)` for a normal, not exact
  under truncation); (c) refuse a non-zero `chord_rel` until a per-blade model exists. Recommendation: (a), with (b) only after a
  derivation of the truncated row-mean distribution.
- **R2, D4.** `uq::mean_sd` returns the value and sd 0 for a bit-constant sample; before, it returned rounding noise. No
  existing test changes, but `uq::run` on a constant metric now differs in its last bits. (a) keep (required by b4, a
  first-principles identity); (b) revert and record BD4 as a known miss. Recommendation: (a). It is the one default change and
  needs the maintainer's yes.
- **R3, store header defect.** A kill during the first `Store::append` leaves a store that cannot be reopened ("What exists").
  Proposed as a follow-up below. Recommendation: write the header when the file is empty, not only when it is absent, with a
  kill test.
- **R4, uniform tolerance input.** A `Uniform` tolerance becomes `z ~ Uniform(lo, hi)` with width 1, so its ledger parameter
  `z.<name>` carries the physical value under unit `1` (the value with its unit is in `settings`). Recommendation: keep; F-700's
  field conventions can carry the unit.

## For the lead: roadmap entries

> Proposed status: done

> Design loop. ADR 0121, ADR 0240, ADR 0623, 2026-11-16. DONE. Every design variant can be recorded in the ledger with its inputs, solver version and results, and the tolerance study driver reports output spread on the reduced-order fan. ADR 0623 (library only, opt-in): Study::set_ledger_sink writes one evaluation entry per evaluated (candidate, condition) pair after its store write, failed pairs with a located reason, and reconciles store and ledger on resume (64 of 64 on the 32-candidate, 2-condition study; 0 duplicate and 0 missing at all 55 kill points of a Nelder-Mead study; rerun_check of the 48 ok entries through ConditionModel with difference 0; store bytes and result bits equal with and without the sink; ledger bytes equal for 1 and 4 workers; missing units refused). FanRomModel is the reduced-order fan as a ledger::Model over common stagger, chord and tip-gap errors, bit for bit equal to FanRomEvaluator::metrics at 64 points; fan_rom_bands runs uq::run on it through the standardised wrapper uq::Standardized: 5, 50 and 95 percent quantiles with 0.95 order-statistic intervals for q, dp_s, power and tone1_pa under the ADR 0118 set-up T tolerances, 12 of 12 intervals contain a 100000-sample reference, the sd within 2.7 percent of first-order propagation, zero-width tolerances give sd exactly 0 (uq::mean_sd returns the value of a bit-constant sample, the one default change, listed for the maintainer), failed samples counted and logged. Per-blade, eccentricity, mass, roughness and thickness tolerances are refused (one-row model); the bands describe the model, not a real fan. No MCP tool or service argument (F-702), no new ledger field (F-700). Nine of nine mutations caught.

> Follow-up proposed: Design store header after a torn first write. Deps F-268. Acceptance: a study store whose first append is killed while writing the header line reopens and resumes with the header present at every kill point inside the first append, and every existing store test keeps its bytes.
