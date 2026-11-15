# ADR 0620: F-697 registration: a fan objective over impedance points and tolerances (fan breakdown H1), first on the reduced-order fan

Status: **DRAFT STUB 2026-11-15, not registered and not accepted.** The maintainer answered the open questions on 2026-11-15 (section \"Maintainer decisions\"); the limits are still proposals until the implementing agent registers them. Written by a drafting agent that read the tree and wrote
no code, ran no build and no test. The limits in "Checks" are proposals: the implementing agent fixes them (keeps or changes
each, with the reason) in this ADR before any code of F-697 exists, and does not change them after a run. Nothing below is a
result. The open questions at the end are the maintainer's; none is taken here.

Roadmap item F-697 (proposed deps F-249, F-268, F-113), part H1 of `docs/fan-project-breakdown.md`: "Objective definition: flow
at the system operating point over a range of impedances, noise, power, vibration, robustness to tolerances" (status there:
missing). Number 0620 given by the lead.

House rules that apply: limits fixed before the run (a miss becomes an ignored `*_known_miss` test with the limit intact and
the measured number); each registered run made once; existing tests not edited; defaults bit-identical; no edit of
`baseline_hashes.rs` or of any golden file; no new dependency, no unsafe code; clean-room (published literature and first
principles only, cited; no vendor code or documentation). Every model here is a stated proxy until it is checked against
measured fan data, which does not exist (breakdown section 2, limit 1).

## Context

### What H1 asks and what already exists (read in the source, 2026-11-15)

The generic half of an objective layer is already in `crates/rcfd-api/src/design`:

* `study.rs` (ADR 0067): `StudyDef` with `conditions` (overrides per operating condition), `ObjectiveDef { metric, sense,
  weight, robust }` with `Robust::{Mean, Worst, MeanPlusKSigma(k), Weighted(w)}` applied over the conditions on the minimised
  quantity, `ConstraintDef { metric, cmp, limit, scale }` taken at the worst condition, `Study::objective_vector`,
  `violation_of`, `fitness_of` (weighted scalar plus violation; Deb's feasibility rules in the optimisers), and
  `parallel_map` (result order is the item order for any worker count).
* The optimisers consume that: Nelder-Mead and DE (`optim.rs`, F-249, ADR 0209), NSGA-II (`nsga2.rs`, ADR 0209, ADR 0220),
  CMA-ES (`cmaes.rs`, ADR 0222).
* `fan_rom_eval.rs` (ADR 0119): `FanRomEvaluator`, an `Evaluator` on `rcfd_post::fan_rom`. Per condition it finds the operating
  point against `SystemCurve { k, l }` (`dp = k Q^2 + l Q`, condition keys `rpm`, `rho`, `nu`, `system_k`, `system_l`) and
  returns `q`, `dp_s`, `dp_t`, `power`, `torque`, `eta_s`, `eta_t`, `tone1_pa` (blade-passing tone amplitude from steady
  loading). A stalled or missing operating point is a failed condition with a located reason.
* `ledger.rs`, `uq.rs` (ADR 0121) and `sensitivity.rs` (ADR 0240): the ledger `Entry` with free-JSON `settings` and
  `provenance`, the `Model` trait over named quantities, the tolerance driver and Sobol indices.
* `rcfd_mesh::fan_design` (ADR 0118): `ComputerFan`, `Tolerances` (stagger, chord, thickness, tip gap, eccentricity, blade
  mass, roughness, each a distribution), `ComputerFan::ensemble(tol, n, seed)`, `static_unbalance()` [kg m].
* `rcfd_post::rotordyn` (ADR 0120): `unbalance_load`, `balance_grade` (`G = e Omega`; its grade table is marked UNVERIFIED in
  the source). `rcfd_post::acoustics`: `spl_db`, `a_weighting_db`.

What is missing, and is the scope of F-697:

1. **No fan objective as one stated object.** A caller has to hand-write the conditions for the impedance points, the metric
   names and the robust modes. Nothing records which objective a ledger entry was scored against.
2. **No tolerance axis in the objective.** `Robust` combines over operating conditions only. The tolerance driver (`uq`)
   reports the spread of one design; it is not inside the function the optimiser minimises.
3. **No vibration metric and no level metric for noise** among the reduced-order metrics. The nominal fan has zero static
   unbalance by symmetry, so a vibration metric only exists under a tolerance plan.
4. **No check that the pieces compose**: impedance aggregation, tolerance statistic, constraints and worker-count determinism
   have tests one by one (ADR 0067, 0121, 0209), not as one objective.

F-218 (actuator disk) is named under C7 in the breakdown but nothing here calls it; the first target is `fan_rom` only.

## Decision proposal

### D1. One opt-in module, no change to existing types

A new module `crates/rcfd-api/src/design/fan_objective.rs` (and its test file). It builds on `StudyDef`, `Evaluator`,
`CondResult`, `ledger::Model` and `parallel_map` as they are. `study.rs`, `nsga2.rs`, `cmaes.rs`, `optim.rs`, `ledger.rs`,
`uq.rs` and `fan_rom_eval.rs` are not edited. No path of an existing run calls the module, so every default keeps its bits.
Not offered here: case-file keys, service methods, MCP tools or tool-description edits (open question 9).

### D2. The objective specification (proposed shape, names open to the implementing agent)

```text
ImpedancePoint  { label, system: SystemCurve, weight }            one system curve; weights used only by Robust::Weighted
TolerancePlan   { tolerances: Tolerances, n: usize, seed: u64 }   fixed sample size, one seed for every candidate
TolStat         Nominal | Mean | MeanPlusKSigma(k) | Sd           statistic of a metric over the tolerance samples
Term            { metric, sense, over_tolerance: TolStat, over_impedance: Robust, weight }
Limit           { metric, cmp, limit, scale, over_tolerance: TolStat }      held at every impedance point (worst case)
FanObjectiveSpec { impedances, base: RomCondition, terms, limits, tolerance: Option<TolerancePlan> }
```

* `FanObjectiveSpec::validate()` gives located messages (empty impedance list, duplicate labels, a tolerance statistic without
  a plan, a vibration metric without a plan, weights that do not sum to 1, non-finite numbers).
* `FanObjectiveSpec::study_def(params, workers) -> StudyDef` writes one `Condition` per impedance point (keys `system_k`,
  `system_l`, as `FanRomEvaluator` reads them), one `ObjectiveDef` per term and one `ConstraintDef` per limit. The metric name
  of a term carries its tolerance statistic (for example `q@mean`, `q@mean+2sd`, `unbalance_force@mean`), so
  `Study::objective_vector` and `violation_of` work unchanged and the optimisers see an ordinary study.
* `FanObjectiveSpec::to_json()` is the record of the objective (sorted keys); its text goes into the ledger entry (D5).

### D3. The evaluator and the two-level aggregation

`TolerantEvaluator<G>` implements `Evaluator`. For a candidate `x` and each impedance point it

1. builds the nominal specimen and, with a plan, the `n` perturbed specimens (`ComputerFan::ensemble(tol, n, seed)`: the same
   `seed` for every candidate, that is common random numbers, so the objective is a deterministic function of `x`; the method
   is the variance-reduction device of simulation practice, Law and Kelton, *Simulation Modeling and Analysis*, cited from
   memory);
2. evaluates each specimen's metrics at that impedance point through a per-specimen closure `G` (for the first target:
   `ComputerFan::axial_fan` to `RomFan::from_axial_fan` to `FanRomEvaluator::metrics`; for the closed-form tests: an analytic
   fan curve);
3. reduces each requested metric over the specimens with its `TolStat`, in specimen order with a fixed summation order, and
   returns one `CondResult` holding the reduced metrics.

`Study` then applies `Robust` over the impedance points. The order is therefore: statistic over tolerances at each impedance
point first, aggregation over impedance points second. It follows from reusing `Study` unchanged; the other order needs a
change to `study.rs` and is open question 3. With `Mean` on both levels the two orders agree; with `Worst` or a sigma term
they do not.

Robustness statistic: mean plus `k` standard deviations of the minimised quantity (the mean-variance form of robust design;
Beyer and Sendhoff, "Robust optimization: a comprehensive survey", Comput. Methods Appl. Mech. Engrg. 196, 2007, cited from
memory). The sample standard deviation uses the divisor `n - 1` over tolerance samples; `Robust::MeanPlusKSigma` over
conditions keeps its population divisor `n` (ADR 0067). The difference is stated in the documentation.

### D4. Metrics (all proxies)

| Metric | Unit | Source | Proxy statement |
|---|---|---|---|
| `q`, `dp_s`, `dp_t`, `power`, `torque`, `eta_s`, `eta_t` | SI | `FanRomEvaluator::metrics` (ADR 0119) | strip model with loss correlations, not validated against a measured fan |
| `tone1_pa` | Pa | the same (Gutin-type steady-loading tone at the blade-passing frequency; Gutin 1936, as cited by ADR 0119) | tonal part only, steady loading only; no broadband, no installation effect |
| `tone1_db` (new, if question 6 says so) | dB re 20 uPa | `acoustics::spl_db` of `tone1_pa / sqrt(2)` | as above |
| `unbalance_force` (new) | N | `|U| Omega^2` with `U = ComputerFan::static_unbalance()` of the specimen (first principles: centrifugal force of an offset mass) | blades only, hub taken as balanced; force on the rotor, not housing vibration |
| `balance_grade` (new, if question 7 says so) | mm/s | `rotordyn::balance_grade(|U| / m_rotor, Omega)` | needs a rotor mass the caller supplies |

No broadband noise, bearing response or housing response is added. D3, D4, E2 and E3 of the breakdown are separate parts.

### D5. Ledger

An adaptor gives a `ledger::Model` (fidelity `reduced_order` for the first target) whose `Outcome.settings` holds the
objective's JSON and whose metrics carry units, so `ledger::entry_of`, `uq::run` and `sensitivity::run` accept it as they are.
No ledger format change is proposed (open question 10).

### D6. Determinism

Specimens and impedance points are evaluated through `parallel_map`; reductions run after collection, in index order. No
wall-clock value enters a metric. The ledger clock is injected in tests.

## Registration: checks and proposed limits (to be fixed by the implementing agent before any code)

Test file `crates/rcfd-api/tests/f697_fan_objective.rs` (new). "Linear fan": `dp(Q) = p0 (1 - Q/Q0)`, `p0 = 100` Pa,
`Q0 = 0.05` m^3/s. Against `dp = k Q^2` the operating point is `Q = [-p0/Q0 + sqrt((p0/Q0)^2 + 4 k p0)] / (2 k)` (first
principles: the intersection of the two curves; the fan-and-system construction is in any fan text, for example Eck, *Fans*,
1973, cited from memory). Impedance points I1, I2, I3 have `k = 75 / 0.0125^2`, `50 / 0.025^2`, `25 / 0.0375^2` Pa s^2/m^6,
which put `Q` at `0.0125`, `0.025`, `0.0375`. Shaft power of the linear fan: `Q dp / eta` with `eta = 0.5`, so `1.875`, `2.5`,
`1.875` W.

None of the "estimate" entries was measured; they are arithmetic or the drafting agent's expectation, and the implementing
agent must mark which ones it could back by reading the code.

### Closed-form objective (C)

| Id | What | Proposed limit | Estimate behind the limit |
|---|---|---|---|
| C1 | per-point flow at I1, I2, I3 against the closed form | relative `<= 1e-9` | bisection in `fan_rom::operating_point`; its stopping tolerance was not read by the drafting agent and must be before this limit is fixed |
| C2 | `Robust::Mean`, `Worst` (on `-q`), `Weighted(0.5, 0.3, 0.2)`, `MeanPlusKSigma(1)` of the flow: `0.025`, `0.0125`, `0.02125`, `0.025 - 0.0125 sqrt(2/3)` m^3/s, against the same aggregation by hand on the per-point values the run returned | relative `<= 1e-12` | a few floating-point operations |
| C3 | the same four values against the closed-form numbers | relative `<= 1e-9` | C1 |
| C4 | mean and worst shaft power: `25/12` W and `2.5` W | relative `<= 1e-9` | C1 |
| C5 | sense: a `Max` term on `q` and a `Min` term on `-q` (a derived metric) give the same objective vector | bit-equal | sign handling only |
| C6 | negative control: I2 replaced by `k` twice as large changes the mean flow by more than `1e-3` relative, and a swapped weight vector changes the weighted flow | both change | arithmetic |

### Aggregation over tolerances (T)

| Id | What | Proposed limit | Estimate behind the limit |
|---|---|---|---|
| T1 | analytic specimen metric `m = a + b xi`, `xi ~ N(0, sigma)`, `n = 4096`, seed registered: `Mean` against `a`; `MeanPlusKSigma(2)` (minimised) against `a + 2 |b| sigma` | within 4 standard errors (`|b| sigma / sqrt(n)` for the mean; `|b| sigma sqrt(1/n + 2/(n-1))` for the sigma form, normal theory) | the 4 SE convention of ADR 0240 |
| T2 | unbalance: `Z = 7` blades, blade mass error `N(0, sigma)` relative, nominal blade mass `m` and centroid radius `r_c` from `blade_mass(0)`: mean `|U|` against `m r_c sigma sqrt(Z pi / 4)`; mean `|U|^2` against `Z (m r_c sigma)^2`; `n = 4096` | within 4 standard errors, the standard error taken from the sample | first principles: `U` is a sum of `Z` equal-variance vectors at evenly spaced angles, so its two components are uncorrelated normals of variance `(m r_c sigma)^2 Z / 2` and `|U|` is Rayleigh distributed |
| T3 | `unbalance_force` of one specimen against `|U| Omega^2` by hand | relative `<= 1e-14` | one multiplication |
| T4 | nominal fan: `|U| / (Z m r_c)` | `<= 1e-12` | rounding of the triangulated blades; not measured |
| T5 | order of aggregation: with `Mean` on both levels the result equals the mean over all `(specimen, point)` pairs | relative `<= 1e-12` | arithmetic |
| T6 | common random numbers: two candidates evaluated in either order, alone or in one batch, give bit-equal reduced metrics | bit-equal | D3 |
| T7 | `TolStat::Nominal` with a plan equals the run without a plan | bit-equal | the nominal specimen is evaluated separately |

### Constraints (K)

| Id | What | Proposed limit | Estimate behind the limit |
|---|---|---|---|
| K1 | limit `q >= 0.02` with scale `0.01` on the linear fan: violated at I1 only; violation `(0.02 - 0.0125) / 0.01 = 0.75` | relative `<= 1e-9` | C1 |
| K2 | a candidate that meets every limit | violation exactly `0.0` | `max(0)` |
| K3 | two limits violated: the total is the sum of the two worst-point violations | relative `<= 1e-12` against the hand sum | `Study::violation_of` |
| K4 | a system curve the linear fan cannot meet inside the scan range (no intersection): the candidate is failed, the reason names the impedance label, and Deb's rules rank it after every evaluated candidate | exact | `Fitness::FAILED`; subject to open question 8 |
| K5 | a limit on a metric the evaluator does not return | infinite violation or a located error at `validate`; one of the two, registered | `violation_of` returns infinity today |

### Determinism (W)

| Id | What | Proposed limit |
|---|---|---|
| W1 | objective vectors, violations and ledger lines (fixed clock) of 32 candidates with a 64-specimen plan: workers 1, 2, 3, 8 | bit-equal |
| W2 | two runs with the same seed; a run with another seed | bit-equal; differs |
| W3 | Nelder-Mead, CMA-ES and NSGA-II records on the linear-fan problem: workers 1 and 4 | bit-equal |

### Reduced-order fan (R), the first real target

Blade row from `fan_rom::free_vortex_design` (or the 120 mm `ComputerFan` defaults of ADR 0118), three system curves chosen so
that the nominal fan runs at about 25, 50 and 75 percent of its free-delivery flow (the values are fixed in this ADR after
reading the curve, before the checks run, and disclosed).

| Id | What | Proposed limit |
|---|---|---|
| R1 | every term of the objective equals the same aggregation of `FanRomEvaluator::metrics` called directly | bit-equal |
| R2 | flow strictly falls and `dp_s` strictly rises from the lowest to the highest `k` | strict |
| R3 | tolerance plan with `n = 64`: the run finishes; the count of failed specimens is reported; wall time recorded | finishes; count recorded; time recorded, no limit |
| R4 | zero tolerances (`Tolerances::default()`) with a plan: every `Sd` statistic | exactly `0.0` or `<= 1e-12` relative to the mean, registered after reading `ensemble` |
| R5 | ledger: one entry per candidate, `fidelity = reduced_order`, the objective JSON in `settings`, read back equal; `rerun_check` passes with `rtol = 0` | exact |

### Optimisers through the layer (O)

Linear fan with two design parameters: speed scale `s` in `[0.5, 2]` (`Q0` scales with `s`, `p0` with `s^2`, the fan laws) and
pitch `t` in `[0, 1]` with `eta(t) = 0.5 - (t - 0.3)^2`. Against quadratic systems the flow scales with `s` exactly, so the
mean flow is `0.025 s` and the mean shaft power is `(25/24) s^3 / eta(t)` W.

| Id | What | Proposed limit | Estimate behind the limit |
|---|---|---|---|
| O1 | minimise mean power subject to mean flow `>= 0.03`: optimum `s = 1.2`, `t = 0.3`, power `3.6` W. Nelder-Mead (budget 400) and CMA-ES (budget 2000), seeds registered | best feasible: `|s - 1.2| <= 2e-3`, `|t - 0.3| <= 2e-2`, power within `1e-3` relative, violation `0` | not run; an optimum on a constraint boundary is hard for Nelder-Mead under feasibility rules, so a miss here is plausible |
| O2 | NSGA-II (pop 40, budget 4000), objectives max mean flow and min mean power: closed-form front `t = 0.3`, `P = (25/12) (q / 0.025)^3` | every front member has `P / P_front(q) - 1 <= 0.02`; the front covers at least 80 percent of the flow range `[0.0125, 0.05]` | not run; convention of ADR 0209's analytic-front checks |
| O3 | the same as O1 with a tolerance plan on an analytic specimen model whose robust optimum is known in closed form (to be written down by the implementing agent before code) | registered with the model | none yet |

### Default bits and goldens (B)

| Id | What | Limit |
|---|---|---|
| B1 | `baseline_hashes`, `f249_moo`, `f297_moo`, `f260_default_bits`, `f297_default_bits`, the `design` unit tests | pass, unchanged |
| B2 | `rcfd-mcp` tools-list goldens and tool count | unchanged |
| B3 | fmt and clippy | clean |

### Mutations (scratch copy, one at a time)

| Id | Mutation | Expected to be caught by |
|---|---|---|
| M1 | impedance weights applied in reversed order | C2, C6 |
| M2 | sample standard deviation with divisor `n` | T1 only if the limit resolves `1/(2n)`; at `n = 4096` probably not caught (registered as a doubt; a small-`n` exact check may be added) |
| M3 | a fresh seed per candidate instead of one seed | T6, W2 |
| M4 | reduction in completion order instead of index order | W1 |
| M5 | constraint taken at the mean over impedance points instead of the worst | K1 |
| M6 | `unbalance_force` with `Omega` instead of `Omega^2` | T3 |
| M7 | sense of a `Max` term not negated | C5, O1 |

### Decision rule (proposed)

* **done:** C, T, K, W, R1, R2, R5, B met; O1 and O2 met; M1 and M3 to M7 caught.
* **partial:** C, K, W, B met and any of T, R, O missed (diagnosed, a known miss).
* **todo:** any of C2, K2, W1, B1, B2 missed.

## Known risks

1. **The objective optimises the model, not a fan.** The reduced-order fan is unvalidated; an optimiser will find and exploit
   its errors (for example the edge of a loss correlation's range; `FanPoint::re_outside` counts strips outside it). The
   ledger entry therefore carries the fidelity tier and the proxy statements, and no result of F-697 is a claim about a real
   fan. H7 (fidelity management) is the part that addresses this and is not in scope.
2. **Impedance values are invented until data exist.** No measured heat-sink, filter or grille curve is in the tree. The tests
   use curves placed relative to the fan's own curve, which checks the arithmetic and says nothing about a real system.
3. **Stall under high impedance.** The operating point fails on a rising characteristic. A study with a high-impedance point
   may have a large failed region; under feasibility rules the optimisers get no gradient information there (question 8).
4. **Sampling noise against optimiser tolerance.** With common random numbers the objective is deterministic but biased by the
   one sample set; an optimum can sit in a feature of that set. `n` is the caller's choice and the bias is not estimated here.
5. **The vibration metric is a rotor force.** It ranks balance sensitivity to blade mass error. It is not vibration at the
   housing, and with the present `static_unbalance` (blades only, hub balanced) it ignores hub and magnet unbalance, which may
   dominate in a real fan.
6. **Noise is one tone.** Ranking by `tone1_pa` can prefer a design that is louder in broadband. Stated in the metric's
   documentation and in the ledger record.
7. **Cost.** A candidate costs `(n + 1) x points` reduced-order evaluations, each with an operating-point scan
   (`n_scan` curve evaluations plus bisection). At `n = 64` and three points that is 195 operating points per candidate; the
   time is recorded by R3 and not limited.
8. **Metric names with a suffix** (`q@mean`) enter study stores and ledger entries. They become a format others read; a later
   rename breaks resume of stored studies.
9. **Two divisors for sigma** (population over conditions, sample over tolerances) are easy to confuse; see D3.

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

Every other open question of this ADR takes the recommendation written with it. Here: Q9 is D1; Q10 is D5 (the objective id, version and hash are F-700 fields); deps gain F-700.

## Open questions for the maintainer (each with options and a recommendation)

1. **Which impedance points define "the system" for the first study?** Options: (a) the caller supplies them and the tests use
   three curves placed at 25, 50 and 75 percent of the nominal fan's free-delivery flow; (b) fix a named set in the
   documentation (for example open air, grille, heat sink, filter) with invented coefficients; (c) wait for measured or
   published system curves (G1). Recommendation: (a). A named set without data would look like a claim.
2. **Is there a default aggregation over impedance points?** Options: (a) none, the caller must state `mean`, `worst` or
   `weighted` for every term; (b) `worst` by default; (c) `weighted` with the point weights by default. Recommendation: (a), so
   that no hidden choice enters a ledger entry.
3. **Order of the two aggregation levels.** Options: (a) statistic over tolerances at each impedance point, then aggregation
   over points (reuses `Study` unchanged); (b) aggregation over points for each specimen, then the statistic over specimens
   ("how good is the 2-sigma fan across my systems"; needs a new reduction path beside `Study::objective_vector`); (c) both,
   selectable. Recommendation: (a) now, (b) as a follow-up only if a study needs it.
4. **Robustness statistic.** Options: (a) mean plus `k` sigma only, `k` stated by the caller; (b) a quantile (for example the
   95th percentile) with its order-statistic interval, as `uq` reports; (c) a chance constraint (probability of violating a
   limit at most `p`), using the Clopper-Pearson bound of ADR 0121; (d) all three. Recommendation: (a) first, since it is
   smooth and has closed-form checks; (c) next for limits, because "at most 5 percent of fans miss the flow" is the natural
   statement of a tolerance requirement.
5. **Tolerance sampling inside the objective.** Options: (a) a fixed `n` and one seed for every candidate (common random
   numbers; deterministic objective); (b) the standard-error stopping rule of `uq` per candidate (sample size varies, the
   objective is then noisy between candidates); (c) Latin hypercube instead of the `ensemble` draw order (needs a mapping from
   hypercube columns to the per-blade draws). Recommendation: (a). Is there a preferred `n` for studies (the tests use 64 and
   4096)?
6. **Noise metric.** Options: (a) keep `tone1_pa` only; (b) add `tone1_db` (level of the first tone); (c) add an A-weighted sum
   over the first `m` blade-passing harmonics (`m` to be chosen; A-weighting from `acoustics::a_weighting_db`). All are tonal,
   steady-loading proxies. Recommendation: (b) and (c) with `m = 3` as separate metrics, each labelled a proxy; the objective
   names which one it uses.
7. **Vibration metric.** Options: (a) `unbalance_force` in newtons from the static unbalance of the sampled blades; (b) the
   balance grade `G = e Omega`, which needs a rotor mass from the caller and a check of the grade table marked UNVERIFIED in
   `rotordyn.rs`; (c) a bearing or housing response through `rotordyn::response`, which needs bearing stiffness and damping
   that nobody has. Recommendation: (a) now, (b) when a rotor mass is part of the fan schema; (c) not before data.
8. **A failed impedance point or a failed specimen** (stall, no intersection, geometry rejected). Options: (a) the candidate
   is failed (today's `Study` behaviour) and the counts are reported; (b) failed specimens are dropped and counted, as ADR
   0240 does, and the candidate fails only above a stated fraction; (c) a failure is turned into a constraint violation of a
   stated size so the optimiser can rank failed candidates. Recommendation: (a) for a failed nominal point, (b) with fraction
   0 by default for specimens (which equals (a) until the caller raises it). Option (c) needs a number nobody can justify yet.
9. **Exposure beyond the library.** Options: (a) library only in F-697; (b) keys in the study JSON of `design_create_study`
   (a new `fan_objective` object; changes the tool schema and both tools-list goldens); (c) case-file keys. Recommendation:
   (a) now; a follow-up item for (b) together with F-339 so the goldens change once. No (c): an objective is a study property,
   not a case property.
10. **Ledger record of the objective.** Options: (a) the objective JSON in the existing free-JSON `settings` of each entry (no
    format change); (b) one extra entry of a new summary kind `objective_def` per study, entries refer to it by hash (smaller
    file; a new kind that `ledger` readers filter by); (c) both. Recommendation: (a). If file size becomes a problem, (b).
11. **Scalarisation and the canonical formulation.** The four quantities have different units. Options: (a) no default
    scalar: studies are multi-objective (NSGA-II) or single-objective with the others as limits (for example minimise the
    noise metric subject to flow at each impedance point and a power cap), and a weighted scalar needs caller weights and
    reference scales; (b) ship a default weighting and reference scales. Recommendation: (a). Which formulation should the
    documentation show first: "minimise noise subject to flow and power" or "flow, noise, power as three objectives with a
    vibration limit"?
12. **Deps and status wording.** F-218 is listed under C7 in the breakdown but is not used by this item. Options: (a) deps
    F-249, F-268, F-113 as drafted; (b) add F-218 for traceability to the breakdown. Recommendation: (a). Should the
    breakdown's H1 row change to "partial (F-697)" when this lands, given it covers only the reduced-order tier?

## Not in scope

Broadband noise, psychoacoustic metrics, rotor-dynamic response, dust and ageing, a surrogate model (H3), fidelity management
(H7), any CFD evaluator behind the objective (the layer takes any per-specimen closure, but none is written or tested here),
case-file keys, service methods and MCP tools.

## References (all cited from memory by the drafting agent; the implementing agent re-reads what it relies on)

* K. Deb, A. Pratap, S. Agarwal, T. Meyarivan, "A fast and elitist multiobjective genetic algorithm: NSGA-II", IEEE Trans.
  Evol. Comput. 6 (2002). K. Deb, "An efficient constraint handling method for genetic algorithms", Comput. Methods Appl.
  Mech. Engrg. 186 (2000) (the feasibility rules already used by `optim` and `nsga2`).
* N. Hansen, "The CMA evolution strategy: a tutorial" (ADR 0222's source). J. A. Nelder, R. Mead, Comput. J. 7 (1965).
* H.-G. Beyer, B. Sendhoff, "Robust optimization: a comprehensive survey", Comput. Methods Appl. Mech. Engrg. 196 (2007).
* A. M. Law, W. D. Kelton, *Simulation Modeling and Analysis* (common random numbers).
* L. Gutin, "On the sound field of a rotating propeller" (1936; NACA TM 1195), through ADR 0119.
* B. Eck, *Fans* (1973), for the fan and system curve intersection.
* ISO 1940-1 for `G = e Omega` (the definition is first principles; the grade table is unverified in the source).

## Addenda

None. Results, amendments and the gate line go here after registration; nothing above is changed after a run.
