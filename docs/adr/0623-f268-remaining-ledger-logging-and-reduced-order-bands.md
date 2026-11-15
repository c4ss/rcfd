# ADR 0623: F-268 remaining scope: automatic ledger logging and uncertainty bands on the reduced-order fan (stub)

Status: **DRAFT STUB 2026-11-15. Not registered, not accepted.** The maintainer answered the open questions on 2026-11-15 (section \"Maintainer decisions\"); the limits are still proposals until the implementing agent registers them. No code of this item exists and no check has run. The
numbers in "Registration" are proposals; they become limits only when the maintainer has answered the open questions and
this text is registered, before any code. Nothing here is a maintainer decision.

Parents: [ADR 0121](0121-wave46d-design-ledger-and-tolerance-study-driver.md) (wave 46D: the ledger and the tolerance driver),
[ADR 0240](0240-f268-registration-sobol-sensitivity-indices.md) (Sobol indices; its questions Q2, Q4, Q5 are open),
[ADR 0118](0118-wave46a-computer-fan-schema-checker-perturbations.md) (fan tolerances),
[ADR 0119](0119-wave46b-reduced-order-axial-fan-tier.md) (reduced-order fan),
[ADR 0136](0136-fidelity-tiers-xhigh-to-xlow.md) (fidelity tiers, proposed, not implemented). Roadmap item F-268 (deps F-249);
follow-up F-339 (MCP tools for uq and Sobol). Number 0623 given by the lead. Drafted in parallel with ADR 0620 (F-697,
objective definition), ADR 0621 (F-698, surrogate model) and ADR 0622 (F-699, multi-fidelity management with a measured-data
hook); I have not read those drafts.

House rules: clean-room (published literature and first principles only); limits fixed before the code and not changed after a
run; a missed limit becomes an ignored `*_known_miss` test with the limit intact; defaults bit-identical, every new thing
opt-in; no edit of `baseline_hashes.rs`, hash tables or goldens (MCP tools-list, CLI help); no existing test edited; no new
dependency; no case-file key and no MCP tool or argument without the maintainer.

## What F-268 asks

Acceptance (quoted, binding today): "Every design variant is recorded in a ledger with its inputs, solver version and results,
and a tolerance study driver samples manufacturing spread and reports output spread against registered limits (ADR 0121)."

The note lists as not done: uncertainty bands on the reduced-order model, automatic ledger logging from runs, an MCP tool for
uq or Sobol studies. The third is roadmap item F-339 and is not repeated here.

## What exists (read in the source on 2026-11-15, not taken from the note)

- **Ledger.** `crates/rcfd-api/src/design/ledger.rs`: format `rcfd-ledger-1`, one JSON object per line. `Entry` has `kind`,
  `study`, `variant`, `params` and `metrics` as `{name, value, unit}`, `evaluator`, `evaluator_version`, `solver_version`,
  `geometry_hash`, `settings` (free JSON), `verdict` (`ok`, `diverged`, `constraint_rejected`, `not_evaluated`), `reason`,
  `fidelity` (`analytic`, `reduced_order`, `cfd`), `recorded_at_ms`, `wall_s`, `seed`, `provenance` (free JSON).
  `Filter` has `kind`, `verdict`, `study`, `evaluator`, `fidelity` and nothing else. Queries: `pareto_front`, `best_so_far`,
  `history`, `to_csv`. Re-run: `Model` trait, `FnModel`, `entry_of`, `rerun_check`, `compare_metrics`.
- **Reader behaviour that matters for new fields.** `Entry::from_json` reads keys by name and ignores keys it does not know,
  so an extra optional key does not make an old reader skip the line. An unknown `fidelity` or `verdict` text does: the line
  is skipped and reported. `Entry::to_json` always writes every key, so a new key must be left out when unset or every new
  ledger differs in bytes from an old one.
- **Who writes to a ledger.** Only `uq::run` (`design/uq.rs`), `sensitivity::run` (`design/sensitivity.rs`) and the service
  method `design.ledger_append` (`design/tools.rs`, `ledger_append`). `Study::run` and `Study::record` (`design/study.rs`),
  `design.evaluate` and `design.run_study` (`design/tools.rs`), the operating maps (`design/opmap.rs`) and the runner path of
  ADR 0189 write the per-study store (`design/store.rs`) or `run.json` only. A search for `ledger` in `study.rs`, `store.rs`,
  `case_eval.rs`, `opmap.rs`, `optim.rs`, `cmaes.rs` and `nsga2.rs` finds nothing. So "every design variant is recorded" holds
  only for entries a caller appends by hand.
- **Study data lacks what an entry needs.** `ParamDef` (`study.rs`) has a name, bounds and targets but no unit. `CondResult`
  (`store.rs`) has metric values by name without units, a status text and a message. The store key is `store::key_of`, the
  bit patterns of the parameter vector.
- **Reduced-order fan.** `rcfd_post::fan_rom` (`crates/rcfd-post/src/fan_rom.rs`): `RomFan` is one blade row (all blades
  alike), `RomModel` holds the loss constants, four of them marked ASSUMED or UNVERIFIED in the source (`cl_max` 1.1,
  `cd_normal` 2.0, `annulus_drag` 0.020, `secondary_drag` 0.018, `tip_cd` pi / (pi + 2)).
  `design/fan_rom_eval.rs`: `FanRomEvaluator` implements `study::Evaluator` and gives `q`, `dp_s`, `dp_t`, `power`, `torque`,
  `eta_s`, `eta_t`, `tone1_pa`. It does **not** implement `ledger::Model`; the only `Model` in `crates/` is `FnModel`. So
  `uq::run` and `sensitivity::run` cannot take the fan without an adapter, and no test runs either of them on it.
- **Fan tolerances.** `crates/rcfd-mesh/src/fan_design.rs`: `Tolerances` (common and per-blade stagger, chord, thickness,
  tip-gap error, eccentricity, blade mass, roughness) and `ComputerFan::ensemble`. `RomFan::from_axial_fan` reads the older
  `rcfd_mesh::fan::AxialFan`; there is no conversion from `ComputerFan` to `RomFan`. The reduced-order model has no input for
  per-blade differences, eccentricity, mass or roughness.
- **MCP.** `crates/rcfd-mcp/src/lib.rs`: four ledger tools. `design_ledger_append` and `design_ledger_query` fix `fidelity` to
  the enum of three values in the schema, which is in the tools-list goldens. `provenance` and `settings` are free objects.
  No `design_uq` or `design_sobol` tool (F-339, todo).
- **ADR 0136** proposes a `tier` field, a `cross_tier_check` entry kind and project tools `record_design`, `verify_design`,
  `query_designs`. None of these names occurs in `crates/`.

## What is left, exactly

1. **Automatic ledger logging** of design-study evaluations (first clause of the acceptance).
2. **Bands on the reduced-order fan**: an adapter so the tolerance driver runs on it, a stated mapping from tolerances to the
   model's inputs, and the spread reported as quantile bands. Which uncertainty the word "bands" covers is Q2.
3. ADR 0240 Q4 and Q5 (decisions, no code unless Q5 b). ADR 0240 Q2 is carried by F-339.
4. Not part of F-268's acceptance, but asked of this draft: the ledger fields and queries the three new consumers need (P1 to
   P8 below).

## Decision proposal

### D1. Ledger sink for studies (opt-in, library)

A new optional sink on the study driver, off by default: a `Ledger`, the evaluator id and version, the fidelity, and unit
maps for parameters and metrics. When set:

- each evaluated (candidate, condition) pair becomes one `evaluation` entry (Q5): `study` from the definition, `variant` the
  store index, `params` from the candidate with the unit map, `metrics` from the `CondResult`, the condition's overrides in
  `settings`, the store key and the condition label in `provenance`;
- a failed condition becomes an entry with `diverged` or `not_evaluated` and the located message as `reason`;
- the store is written first and the ledger second; on resume, store records with no ledger entry of the same store key and
  condition are appended, and existing ones are not written again;
- a candidate served from the store without a new evaluation writes nothing.

With the sink unset, no file is touched and no byte of the store or of a result changes. Reaching the sink through the
service or MCP needs a new argument and is Q4.

### D2. Reduced-order fan as a `ledger::Model` (opt-in, library)

A `FanRomModel` in `design/fan_rom_eval.rs` over named parameters with units, built from a nominal `RomFan`, a `RomModel` and
a `RomCondition`. Parameters are the tolerances the model can represent (Q3): a common stagger error, relative chord and
thickness errors of the row, a tip-gap error, and, if Q2 includes them, the loss constants of `RomModel`. A stalled or missing
operating point is a `not_evaluated` outcome with the located reason. Its metrics equal `FanRomEvaluator::metrics` for the
same fan and condition.

### D3. Bands

No new statistics: `uq::run` already gives quantiles with distribution-free order-statistic intervals (ADR 0121 D6, checked
by U2 and U3). The band of a metric is its 5 and 95 percent quantiles with the median, each with its interval, plus the failed
fraction; the bands describe the model's response to the stated input spread and nothing about a real fan. If Q2 includes the
loss constants, `sensitivity::run` splits the variance into the tolerance part and the model-constant part.

## Registration (proposed; to be fixed before code)

Set-ups: an analytic study evaluator with closed-form metrics, and the reduced-order fan of
`crates/rcfd-api/tests/fan_rom_design_w46b.rs` (`set_up`). Seeds, tolerance values and the reference sample are fixed at
registration.

| Check | What | Proposed limit |
|---|---|---|
| LG1 | 32 candidates (Latin hypercube) at 2 conditions, sink on | exactly 64 `evaluation` entries, 0 skipped lines |
| LG2 | same study with and without the sink | store file bytes equal; result bits equal |
| LG3 | `rerun_check` of every `ok` entry of LG1 (deterministic evaluator) | 64 of 64 pass, largest difference 0 |
| LG4 | 8 candidates of 32 made to fail (4 diverged, 4 not evaluated) | 16 non-ok entries with a non-empty reason; counts equal the store's |
| LG5 | 16-evaluation study killed at each of 33 points (before and after each store write, and at the end), then resumed | after resume 0 missing and 0 duplicate (store key, condition) pairs at 33 of 33 points |
| LG6 | the finished LG1 study run again on its store | 0 new entries |
| LG7 | 1 and 4 workers, fixed clock | ledger bytes equal |
| LG8 | a parameter or metric without a unit in the map (if Q6 a) | located refusal before the first evaluation, 0 entries |
| BD1 | `FanRomModel` against `FanRomEvaluator::metrics` at 64 Latin-hypercube points | every metric bit for bit |
| BD2 | bands of `q`, `dp_s`, `power`, `tone1_pa` at 5, 50, 95 percent, n = 2000, confidence 0.95, against an independent Monte Carlo reference of 100 000 samples (another seed) | at least 10 of 12 intervals contain the reference quantile |
| BD3 | small tolerances: sd of each metric against first-order propagation with central-difference gradients | relative difference at most 0.05 plus 4 SE of the sd |
| BD4 | every tolerance of zero width | sd exactly 0; every quantile equals the nominal value bit for bit |
| BD5 | every tolerance sd doubled (small-tolerance set-up of BD3) | band width ratio in [1.6, 2.4] for each metric |
| BD6 | a tolerance set that stalls some samples | failed count in the summary equals the non-ok entries in the ledger; failed fraction reported; `ok` fraction within its Clopper-Pearson 0.999 interval of the reference |
| BD7 | 1 and 4 workers, fixed clock | ledger bytes equal |
| BD8 | only if Q2 b: Sobol indices with the loss constants as inputs, N to be fixed | sum of first-order indices at most 1 + 4 SE; a constant the operating point does not use gets exactly 0 |
| DF1 | sink and adapter unused | no existing test edited; `uq_tests`, `ledger_tests`, `sensitivity_tests`, the design tests of `rcfd-mcp`, and both tools-list goldens unchanged |

Negative controls: NC1, a sink that writes before the store fails LG5 at one cut point at least; NC2, a mapping that drops
the tip-gap error fails BD3 on `q`; NC3, a reference drawn with every sd scaled by 1.1 puts fewer than 10 of 12 intervals of
BD2 on the reference (to be confirmed by a disclosed estimate before registration; ADR 0121 found a control of this kind
defective once). Mutations on a scratch copy: one entry per candidate instead of per pair; condition overrides left out of
`settings`; resume that re-appends; the unit map ignored; the adapter using the default `RomModel` instead of the given one.

BD2's "10 of 12" is my estimate from independent 0.95 intervals (probability about 0.98 of 10 or more); the quantiles of one
sample are correlated, so the count must be re-derived or simulated and disclosed before registration. BD3's 0.05 depends on
how small "small" is and is fixed with the tolerance values.

Decision rule (proposed): done when LG1 to LG8, BD1 to BD7 and DF1 are met, the controls hold and each mutation fails a check;
partial when DF1, LG2 and BD1 hold but another limit misses; not merged when DF1 or LG2 fails.

## Ledger fields and queries for F-697, F-698, F-699 (proposals, none decided)

| | Need | Who | Today | Proposal |
|---|---|---|---|---|
| P1 | a stable reference to an entry | all three | the index is the line position in one file; stable because nothing is rewritten, lost on export or merge | reference = `{index}` within one ledger; a content hash of the line as an optional second form (Q9) |
| P2 | links to parent entries (a band to its nominal variant, a discrepancy to its two entries, a prediction to its training set) | all three | `provenance` is free JSON, no convention | `parents`: list of references |
| P3 | source id of a value: computed, measured or from literature; data set id, citation, licence, file hash | F-699 | none | `source`: `{id, kind, citation, file_sha256}` |
| P4 | fidelity level finer than three values, and a level for measured data | F-699, F-698 | `fidelity` in {`analytic`, `reduced_order`, `cfd`}; an unknown value makes old readers skip the line; the MCP enum is in the goldens | Q8, Q10 |
| P5 | uncertainty of a metric (measurement uncertainty, iteration or sampling error) | F-698 (noise term), F-699 | `Quantity` has no uncertainty | optional standard uncertainty per metric, same unit (Q9 for where) |
| P6 | discrepancy record between two levels at the same inputs: both references, per-metric difference and relative difference, both uncertainties, a verdict | F-699 | none; ADR 0136 proposes `cross_tier_check` | one summary kind; name and verdict words per Q8 |
| P7 | objective identity on every scored variant: objective id, version and a hash of its definition; raw metrics per condition kept, so a changed objective can be re-scored from the ledger | F-697 | the study store keeps the scalar; the ledger does not know objectives | `objective`: `{id, version, hash}` on a summary entry of kind `objective_score` with `parents` the raw entries |
| P8 | queries: filter by `variant` and by condition; group the entries of one variant; pairs of entries with bit-identical parameters across two levels; a training set (parameter matrix and metric columns) that refuses mixed units, evaluators or versions and returns failed points separately; the record of a fitted surrogate or correction (kind, training references, a hash) | F-697, F-698, F-699 | `Filter` has five fields; `to_csv` exists; no pairing | new library functions in `ledger.rs`; through MCP they need new arguments (goldens) |

## Known risks

- **Two files, two truths.** The store and the ledger hold the same evaluations. LG5 covers a kill; it does not cover a
  ledger edited by hand or shared by two studies under one name.
- **Units.** Studies carry no units. A wrong unit map is recorded faithfully and nothing can detect it.
- **Single writer.** ADR 0121 assumes one writer process. A study with a sink and an assistant calling
  `design_ledger_append` on the same file are two writers; the index returned by the MCP tool can then be wrong.
- **Bands read as truth.** The bands are the spread of a model with unverified loss constants. Without measured data the
  dominant uncertainty may be the model form, which no sampling of inputs shows (Kennedy and O'Hagan 2001).
- **Tolerance mapping.** Per-blade errors, eccentricity, mass and roughness have no place in a one-row model; any averaging
  rule is an assumption and under-states the spread.
- **Stalled samples.** Dropping them conditions the bands on "not stalled"; the failed fraction must travel with every band.
- **Ledger growth.** Tolerance and Sobol studies write every sample; queries read the whole file.
- **Format drift.** Conventions inside `provenance` are not validated by the reader; top-level fields are, but old builds
  drop them on export.
- **Overlap with ADR 0136.** Building P4 and P6 here under other names would leave two schemes for the same thing.

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

Every other open question of this ADR takes the recommendation written with it. Here: Q1 is (b), the split to F-700 (D6); Q4 is (a), D1; Q8 is (b), D4; Q9 is (b), D5; Q10 is D2 (library only, not the MCP enum); Q11 is (c) with (b) per entry, D3; Q12 is B4; Q13 is (a); BD2 and BD3 fall under B3.

## Open questions for the maintainer

- **Q1. Amend F-268 or split.** (a) F-268 keeps D1 to D3; P1 to P8 are decided inside F-697 to F-699 as each needs them;
  (b) F-268 keeps D1 to D3 and P1 to P8 become a new item F-700 that the three depend on; (c) everything left moves to F-700
  and F-268 closes as far as built. Recommendation: (a) if the three drafts ask for few fields, (b) if two or more of them ask
  for the same field; not (c), because F-268's own acceptance needs D1.
- **Q2. What "uncertainty bands on the reduced-order model" means.** (a) the spread from manufacturing tolerances only;
  (b) also the spread from the unverified loss constants of `RomModel`, with ranges to be stated by the maintainer or taken
  from the cited sources; (c) also a model-form band from the discrepancy against CFD or measured data, which belongs to
  F-699. Recommendation: (a) in F-268, (b) only with ranges from a published source, (c) left to F-699.
- **Q3. Tolerances the one-row model cannot represent.** (a) use only the common stagger, row-mean chord and thickness and the
  tip-gap error, and refuse a tolerance set with any other non-zero entry; (b) average per-blade draws over the blades and
  ignore eccentricity, mass and roughness, with a stated warning; (c) wait for a model with per-blade inputs.
  Recommendation: (a).
- **Q4. Which runs "automatic ledger logging from runs" covers.** (a) design-study evaluations through the library sink only;
  (b) also `design_evaluate` and `design_run_study` through a new optional `ledger` argument (service protocol and MCP
  tools-list goldens change); (c) also runs of the project runner (ADR 0189 tags `run.json` with study, variant and
  condition) by a library importer, no schema change; (d) every solver run. Recommendation: (a) now, (b) in the same
  registered golden edit as F-339, (c) later if wanted, not (d).
- **Q5. Entry granularity.** (a) one entry per (candidate, condition); (b) one entry per candidate with metrics prefixed by
  the condition label. Recommendation: (a); it keeps one operating point per entry and makes `rerun_check` direct.
- **Q6. Units.** (a) the sink needs a unit for every parameter and metric and refuses otherwise; (b) a missing unit is
  recorded as `1`; (c) add an optional unit to `ParamDef`, which reaches the study definition accepted by the MCP tools.
  Recommendation: (a).
- **Q7. A ledger write that fails during a study.** (a) the study stops with a located error; (b) the study continues and
  reports the entries not written. Recommendation: (a), because the acceptance says "every".
- **Q8. ADR 0136.** It is proposed, not accepted, and proposes `tier`, `cross_tier_check` and three project tools. (a) P4 and
  P6 follow ADR 0136's names and wait for its acceptance; (b) F-699 defines its own record and ADR 0136 is amended to match;
  (c) both exist. Recommendation: (a) or (b), decided once, not (c).
- **Q9. Where new fields live.** (a) as conventions inside `provenance`: no format change, reachable through today's MCP
  tool, not validated; (b) as new optional top-level keys of `rcfd-ledger-1`, omitted when unset: validated, old readers keep
  the line but drop the keys on export, not reachable through MCP without a golden edit; (c) a new format `rcfd-ledger-2`.
  Recommendation: (b) for P2, P3, P5, (a) until then; not (c).
- **Q10. Measured data in the ledger.** (a) a new `fidelity` value `measured`: old readers skip those lines (which also keeps
  them out of old Pareto queries), the MCP enum and goldens change; (b) keep three values and mark measured entries by
  `source.kind`; (c) measured data stays outside the ledger and only discrepancy records refer to it. Recommendation: (a),
  with the golden edit listed for approval.
- **Q11. Proprietary measured data.** (a) values in the ledger file, which then must stay out of the repository; (b) the
  ledger holds a reference and a file hash only, values stay in the user's file; (c) a separate ledger file for measured
  data. Recommendation: (c) with (b) as an option per entry; needs the maintainer, since it concerns the user's data.
- **Q12. ADR 0240 Q4, still open.** The NC1 control on ST1 missed its margin (|z| 3.87 against 4). (a) the Sobol part stays
  partial; (b) the maintainer accepts it as met. ADR 0240 recommends (b), in person. Nothing in this ADR depends on it.
- **Q13. ADR 0240 Q5, still open.** Stopping rule on the index SE: (a) fixed N; (b) doubling until every SE is below a
  target. ADR 0240 recommends (a) until a costly evaluator needs it. If Q2 b is chosen and BD8 runs on the reduced-order fan
  (milliseconds per evaluation), (a) still suffices; F-699 with CFD evaluations would be the first reason for (b).
- **ADR 0240 Q2** (MCP exposure) is not open here: it is roadmap item F-339, which still needs the maintainer before merge
  because it edits the tools-list goldens.

## References

- ADR 0121 and ADR 0240 and their references (order-statistic intervals, Clopper and Pearson 1934, Saltelli et al. 2010,
  Jansen 1999).
- M. C. Kennedy, A. O'Hagan (2001), Bayesian calibration of computer models, J. R. Statist. Soc. B 63, 425-464 (model
  discrepancy). Pages UNVERIFIED.
- A. I. J. Forrester, A. Sobester, A. J. Keane (2007), Multi-fidelity optimization via surrogate modelling, Proc. R. Soc. A
  463, 3251-3269 (pairing of cheap and costly evaluations). Pages UNVERIFIED.
- JCGM 100:2008, Evaluation of measurement data: Guide to the expression of uncertainty in measurement (standard
  uncertainty, first-order propagation used in BD3).

## For the lead: roadmap entries

See `roadmap-entry.json` next to this file: the amended F-268 object (Q1 a), and F-700 as the alternative of Q1 b.
