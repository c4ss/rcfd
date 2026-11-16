# ADR 0620: F-697 registration: a fan objective over impedance points and tolerances (fan breakdown H1), first on the reduced-order fan

Status: **REGISTERED 2026-11-16 by Amendment 1 (section "Addenda"), written before any code of F-697 and before any run of its
checks.** Not yet accepted by the maintainer. The text from here to "Addenda" is the drafting agent's stub of 2026-11-15 (no
code, no build, no test), kept as written; where Amendment 1 fixes a limit, a name or a set-up differently, Amendment 1 governs.
The maintainer answered the open questions on 2026-11-15 (section "Maintainer decisions").

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

Results, amendments and the gate line go here; nothing above is changed after a run.

## Amendment 1 (2026-11-16): registration, before any code of F-697 and before any run of its checks

Written by the implementing agent on branch `f697-work` (from c3590958, the F-700 tip on the F-268 tip), worktree
`w49i-work/f697`, scratch `w49i-work/f697-scratch`. Before this text no library or test code of F-697 existed in the branch.

### What was read

`AGENT_RULES.md`; the brief; F-697's roadmap acceptance and note (binding, quoted below); this ADR (stub and maintainer
decisions); ADR 0623 (F-268: the study ledger sink, `FanRomModel`, `fan_rom_bands`) and ADR 0632 (F-700: optional ledger
fields `parents`, `objective`, the `objective_score` record convention of its D4); `docs/fan-project-breakdown.md` H1;
`loop-drafts/decisions.md`. Source: `design/study.rs` (`StudyDef`, `Robust::apply` with the population divisor,
`objective_vector`, `violation_of` worst over conditions, `parallel_map`, `Study::run` for NM, CMA-ES, NSGA-II),
`design/study_ledger.rs` (`LedgerSink`, `entries_of`, `pair_of`, `ConditionModel`), `design/ledger.rs` (`Entry`, `Identity`,
`entry_ref`, `entry_hash`, `Model`, `Outcome`, `entry_of`, `rerun_check`), `design/fan_rom_eval.rs` (`FanRomEvaluator::metrics`,
`RomCondition`), `rcfd_post::fan_rom` (`operating_point`: scan, then 200 bisection steps of `rcfd_fv::lumped::bisect`, so the
root is resolved to the last bits of the bracket; the located errors "no intersection", "lies on a rising fan characteristic:
stall region", "is stalled"; `FanRom::tone`), `rcfd_post::acoustics` (`spl_db`, `a_weighting_db`), `rcfd_mesh::fan_design`
(`ComputerFan::{axial_fan, blade_mass, static_unbalance, ensemble}`, `Tolerances`, `Dist::sample`: one uniform per draw, a
`Fixed` draw takes none), `rcfd_mesh::fan::AxialFan::validate` (stations must start at `r_hub` and end at `r_tip`).

The binding acceptance (clauses numbered as in the roadmap):

> Every limit not stated here is registered in ADR 0620 before any code. An opt-in library objective (no case-file key, no MCP tool, no default changed) builds a study from a list of system curves, metric terms and an optional tolerance plan, and is consumed unchanged by Nelder-Mead, CMA-ES, NSGA-II and the design ledger. (1) On a linear fan curve dp = 100 (1 - Q/0.05) Pa against three quadratic systems that put Q at 1/4, 1/2 and 3/4 of 0.05 m^3/s, the per-point flows are within 1e-9 relative of the closed form and the mean, worst, weighted (0.5, 0.3, 0.2) and mean-plus-one-sigma flows are 0.025, 0.0125, 0.02125 and 0.025 - 0.0125 sqrt(2/3) m^3/s within 1e-12 relative of the same aggregation done by hand on the per-point values. (2) With blade mass errors N(0, sigma) on Z = 7 blades, the mean static unbalance is within 4 standard errors of r_c m sigma sqrt(Z pi / 4) at N = 4096 samples, and a metric linear in one Gaussian tolerance gives mean + k sd within 4 standard errors of a + k |b| sigma. (3) A constraint violated at one impedance point only gives the violation (excess / scale) of that point, a candidate with a stalled point is reported failed with the located reason, and a feasible candidate has violation exactly 0. (4) Objective vectors, constraint violations and ledger lines (fixed clock) are bit-identical for 1, 2, 3 and 8 workers and for two runs with the same seed, and differ for another seed. (5) On the reduced-order fan (rcfd_post::fan_rom) at three impedance points the layer's objective values equal, bit for bit, the same aggregation of FanRomEvaluator::metrics called directly, and flow falls strictly as the system coefficient rises. (6) Nelder-Mead and CMA-ES find the closed-form constrained optimum (speed scale 1.2, pitch 0.3, mean shaft power 3.6 W) within the registered limits, and the NSGA-II front lies within the registered distance of the closed-form front. (7) baseline_hashes, the existing design tests and both tools-list goldens are unchanged.

### Disclosed: smoke runs and arithmetic before this registration

Two smoke runs on the existing code only (a temporary test `crates/rcfd-api/tests/zz_f697_smoke.rs`, never committed, moved
to `f697-scratch/zz_f697_smoke2.rs`; the first version was overwritten by the second; logs `smoke1.log`, `smoke2.log`). They
compute no check of the table; no seed of a check was used.

1. `ComputerFan::default()` (the 120 mm schema fan of ADR 0118) through `RomFan::from_axial_fan(axial_fan(0, 0, 0))` at 3000
   rpm: the strips are stalled (beyond `cl_max`) for every flow below about 0.062 m^3/s and free delivery is about 0.077
   m^3/s; only about the top 20 % of the flow range is usable. The stub's "25, 50 and 75 percent of free delivery" cannot be
   placed on this fan.
2. The ADR 0119 set-up C row (free-vortex design to 45 Pa at Q_D = 0.045 m^3/s, 3000 rpm, as in `fan_rom_design_w46b.rs`) as a
   `ComputerFan` (set-up F below): free delivery about 0.0478 m^3/s; unstalled for q above about 0.0365 m^3/s. Operating
   points against `dp = k Q^2`: k = 5e3: q = 0.044359; 1e4: 0.041716; 1.5e4: 0.039472; 2e4: 0.037533; 2.5e4: 0.035836
   m^3/s (all ok); k = 3e4: "is stalled: 5 of 40 strips"; k = 4e4: "is stalled: 40 of 40 strips"; k >= 7e4: "rising fan
   characteristic: stall region". One evaluation (n_scan 100) costs 0.14 to 0.24 s. Nominal static unbalance
   `[0, 2.7e-20]` kg m, blade mass 1.790e-3 kg, in-plane centroid radius 0.04351 m; one `static_unbalance` costs about 10 ms
   (64 in 0.49 s on the default fan, whose mean |U| for mass_rel TN(0, 0.01) came out 1.153e-6 against the Rayleigh value
   1.121e-6, recorded, not a check).
3. Arithmetic of the closed forms is in the set-ups and the check table below.

### Design (fixed now; names are the implementation's)

New module `crates/rcfd-api/src/design/fan_objective.rs`, one line in `design/mod.rs`. No other existing file is edited; no
existing path calls the module, so every default keeps its bits.

* `ImpedancePoint { label, system: SystemCurve, weight }`. `Over` = `Mean | Worst | MeanPlusKSigma(k) | Weighted` over the
  impedance points; it maps to `study::Robust` (`Weighted` takes the points' weights in point order). There is no default
  (Q2 (a)).
* `TolStat` = `Nominal | Mean | Sd | MeanPlusKSigma(k)` over the tolerance specimens. `MeanPlusKSigma(k)` is taken on the
  minimised quantity: `mean + k sd` for a `Min` term or an `Le` limit, `mean - k sd` for a `Max` term or a `Ge` limit (Beyer and
  Sendhoff 2007, cited from memory: the mean-variance robustness measure). `sd` is the sample standard deviation (divisor
  `n - 1`, n >= 2 required); `Robust::MeanPlusKSigma` over points keeps the population divisor of ADR 0067. A sample whose
  values are bit-identical has mean = that value and sd = 0 exactly (the identity of ADR 0623 D4, applied in this module only).
* `Term { metric, sense, tol: TolStat, over: Over, weight }`, `Limit { metric, cmp, limit, scale, tol: TolStat, over:
  Option<Over> }` (`None`: held at every point, the worst point counts; `Some(o)`: held on the aggregate over the points).
* `TolerancePlan { n, seed, max_failed_fraction }` (common random numbers: the same `seed` for every candidate, Law and Kelton,
  cited from memory; `max_failed_fraction` 0 unless the caller raises it, Q8 (b)).
* `FanObjective { name, params: Vec<(ParamDef, unit)>, points, terms, limits, plan: Option<TolerancePlan>, workers }`.
  `validate(&model)` gives located refusals: no point, duplicate labels, a non-finite system, `Weighted` with weights that are
  not >= 0 summing to 1 within 1e-9, a statistic other than `Nominal` without a plan, `n < 2`, a metric the model does not
  offer, a bad fraction, and everything `StudyDef::validate` refuses.
* Reduced metric names (one per term or limit, used by the study): `<metric>@nominal`, `<metric>@mean`, `<metric>@sd`,
  `<metric>@mean+<k>sd` (Min, Le) or `<metric>@mean-<k>sd` (Max, Ge), `k` printed with Rust's `{}`; an aggregated limit adds
  `@pts.<over>` (`pts.mean`, `pts.worst`, `pts.weighted`, `pts.mean+<k>sigma`). `study_def()` writes one `Condition` per point
  (keys `system_k`, `system_l`, label), one `ObjectiveDef` per term on its reduced name with `Robust` from `Over`, and one
  `ConstraintDef` per limit on its reduced name. An aggregated limit's value is computed by the evaluator over the points (on
  the minimised quantity of its `cmp`, by `Robust::apply`, then signed back) and written identically into every point's result,
  so `violation_of` (worst over identical values) gives exactly its violation. So `Study`, NM, CMA-ES and NSGA-II run it
  unchanged.
* `trait SpecimenModel: Sync { type Specimen; id, version, fidelity, metrics() -> [(name, unit)], describe() -> json,
  nominal(x), perturbed(x, n, seed), evaluate(spec, point, needed) }`. `TolerantEvaluator { objective, model }` implements
  `study::Evaluator`: per candidate it builds the nominal specimen and, with a plan, the `n` perturbed ones, evaluates every
  (specimen, point) pair through `parallel_map` with the objective's workers (result order = index order), then reduces each
  reduced metric per point in specimen-index order. A failed nominal evaluation fails that point with the located reason
  `point '<label>': nominal: <reason>`; failed specimens are dropped and counted, and the point fails (reason naming the count and
  the first failed specimen) when the failed fraction exceeds `max_failed_fraction`. `specimen_table(x)` returns the raw
  per-(specimen, point) results for hand checks.
* Metrics: the model's own; the reduced-order target adds `tone1_db` = `spl_db(tone1_pa / sqrt 2)` and `tone_a3_db` = `10
  log10 sum_{m=1..3} 10^((spl_db(p_m / sqrt 2) + A(f_m)) / 10)`, `p_m = FanRom::tone(.., m, ..)` at the operating point, `f_m =
  m B Omega / (2 pi)`, `A` = `a_weighting_db` (Q6 (b), (c), m = 3; tonal steady-loading proxies only); `unbalance` = `|U|`
  [kg m] with `U = static_unbalance()` of the specimen, `unbalance_force` = `|U| Omega^2` [N] (Q7 (a); blades only, hub balanced).
* `FanRomTarget { geometry: Fn(&[f64]) -> Result<ComputerFan, String>, tolerances: Tolerances, model: RomModel, base:
  RomCondition }` is the first real `SpecimenModel` (id `rcfd.fan_objective.rom`, fidelity `reduced_order`): specimens are
  `ComputerFan::ensemble(tolerances, n, seed)` of the candidate's fan; aero metrics are `FanRomEvaluator { model, base
  }.metrics(RomFan::from_axial_fan(spec.axial_fan(0, 0, 0)), point condition)` (the point condition is `base` with its system
  replaced). Tolerances the one-row model cannot represent (`stagger_blade_deg`, `chord_rel`, `eccentricity`, `roughness_ks`)
  must be `Fixed { 0 }`, else a located refusal naming the field; `stagger_common_deg` and `tip_gap_delta` act on the row;
  `mass_rel` and `thickness_rel` act on the mass model only (ADR 0119 does not model thickness). The ROM is evaluated only when
  an aero metric is needed, the mass model only when `unbalance` or `unbalance_force` is.
* Ledger (Q10 (a), D5 with F-700's fields). `ledger_sink(ledger, fidelity)` gives the ADR 0623 sink with every unit, so the
  study writes one `evaluation` entry per (candidate, point) with the reduced metrics. `score_entries(study, ledger)` gives one
  `objective_score` entry per store record: `objective` = `{id: "rcfd.fan_objective", version: "1", hash: FNV-1a 64 of
  to_json()}`, `parents` = references with hashes to that record's point entries, `settings = {"objective": to_json()}`, metrics
  = every term's aggregate (name `<reduced>@<over>`, natural sign, the metric's unit) and `violation` [1], verdict `ok` or
  `diverged` with the record's message. `ObjectiveModel` is the same as a `ledger::Model`, so `rerun_check` re-runs a score
  entry. `to_json()` is the objective record (sorted keys, the model's id, version and `describe()`).

### Set-ups (fixed now)

* **L, the linear fan** (an analytic `SpecimenModel` in the test file, id `analytic.linear_fan`). Parameters `s` (speed scale,
  [0.5, 2]) and `t` (pitch, [0, 1]); `eta(t) = 0.5 - (t - 0.3)^2`. Specimen: speed error `e` (nominal 0), `S = s (1 + e)`. Fan
  `dp(Q) = 100 S^2 (1 - Q / (0.05 S))` for `Q >= Qs = 0.005 S`, and the rising stall branch `dp = 90 S^2 Q / Qs` below it.
  Operating point by `rcfd_post::fan_rom::operating_point` on `[1e-3 q_hi, q_hi]`, `q_hi = 0.1 S`, n_scan 100. Metrics `q`
  [m3/s], `dp` [Pa], `power = Q dp / eta(t)` [W], `neg_q = -q` [m3/s]. Perturbed specimens: `e` drawn by
  `fan_design::Dist::TruncatedNormal { 0, sigma_s, -10 sigma_s, 10 sigma_s }.sample` from `Rng::new(seed)`, one draw per
  specimen in order. Points I1, I2, I3: `k = 75 / 0.0125^2 = 480000`, `50 / 0.025^2 = 80000`, `25 / 0.0375^2 = 17777.77...`
  Pa s^2/m^6, `l = 0`, weights 0.5, 0.3, 0.2. At `s = 1` the closed form is `Q = [-p0/Q0 + sqrt((p0/Q0)^2 + 4 k p0)] / (2 k)` =
  0.0125, 0.025, 0.0375 and power (eta 0.5 at t = 0.3) 1.875, 2.5, 1.875 W. I4: `k = 1e7`: the only crossing is on the rising
  branch at `Q = 90 / (1e7 x 0.005) = 0.0018` (fan minus system at `q_lo = 1e-4` is `1.8 - 0.1 > 0`), so `operating_point`
  reports the stall region.
* **G, one Gaussian tolerance** (analytic model in the test, id `analytic.gauss`): `m = a + b xi`, `a = 2`, `b = -3`, `xi` drawn
  as `e` above with `sigma = 0.1` (truncation at 10 sigma: the variance changes by a relative 1.5e-21, the mean by 0), `n =
  4096`, seed 697_001, one point (system unused).
* **F, the reduced-order fan.** The set-up C row of ADR 0119 (`fan_rom_design_w46b.rs` `set_up`: 7 blades, r_hub 0.030, r_tip
  0.058, tip gap 1 mm, NACA 4 camber 0.04 at 0.5, chord 0.030, free-vortex stagger to 45 Pa at 0.045 m^3/s at 3000 rpm,
  `RomModel::default()`) as a `ComputerFan`: `ComputerFan::default()` with n_blades 7, r_hub 0.030, r_tip 0.058, tip_gap 0.001
  and stations: the hub (span 0) and tip (span 1) copies of the first and last design sections plus one station per design
  section at `span = (r - r_hub) / (r_tip - r_hub)`, `stagger_deg = stagger.to_degrees()`, thickness 0.08, sweep 0, skew 0.
  Candidate parameter `stagger_offset_deg` in [-3, 3] added to every station's `stagger_deg`. Condition: 3000 rpm, rho 1.2, nu
  1.5e-5, c0 343, observer 1 m at 90 deg, n_scan 100. Points R1 `k = 5e3`, R2 `1.5e4`, R3 `2.5e4` (l = 0; weights 0.5, 0.3,
  0.2), chosen from smoke 2 inside the unstalled range and before any check ran; R4 `k = 4e4` (stalled in smoke 2).
  **Tolerances F-T**: `stagger_common_deg` TN(0, 0.5, -1, 1), `tip_gap_delta` TN(0, 1e-4, -2e-4, 2e-4), `mass_rel` TN(0, 0.01,
  -0.03, 0.03), others Fixed 0.
* **U, unbalance**: `ComputerFan::default()` (only the mass model is evaluated), `mass_rel` TN(0, 0.01, -0.1, 0.1), others
  Fixed 0, n = 4096, seed 697_002, 3000 rpm, one point.

### Checks (limits fixed now)

Test file `crates/rcfd-api/tests/f697_fan_objective.rs` (new). All in release. Every compared float is the value the layer
returns through `Study::objective_vector`, `Study::violation_of` or the store record, unless stated.

| Id | Clause | What | Limit |
|---|---|---|---|
| C1 | (1) | L at `x = (1, 0.3)`, points I1 to I3: per-point `q@nominal` against the closed form | relative <= 1e-9 each |
| C2 | (1) | four terms on `q@nominal` (Max): `Mean`, `Worst`, `Weighted` (0.5, 0.3, 0.2), `MeanPlusKSigma(1)`; the objective vector against `Robust::apply` done by hand in the test on the record's per-point values (signed for Max) | relative <= 1e-12 each |
| C3 | (1) | the same four values against 0.025, 0.0125, 0.02125, 0.025 - 0.0125 sqrt(2/3) | relative <= 1e-9 each (C1) |
| C4 | — | terms on `power@nominal` (Min) `Mean` and `Worst`: 25/12 W and 2.5 W | relative <= 1e-9 |
| C5 | — | a `Max` term on `q` and a `Min` term on `neg_q`, both `Weighted` | objective values bit-equal |
| C6 | — | negative controls: I2 with `k` doubled changes the mean flow by more than 1e-3 relative; the weights (0.2, 0.3, 0.5) change the weighted flow by more than 1e-3 relative | both |
| T1 | (2) | G: `m@mean` (Min, one point) against `a`; `m@mean+2sd` (Min) against `a + 2 |b| sigma`; `m@mean-2sd` (Max) against `a - 2 |b| sigma` | within 4 SE: `|b| sigma / sqrt n` for the mean, `|b| sigma sqrt(1/n + k^2/(2 (n - 1)))` for the sigma forms (normal theory) |
| T2 | (2) | U: `unbalance@mean` against `m r_c sigma sqrt(7 pi / 4)` with `m`, `r_c` (in-plane radius of the centroid) from `blade_mass(0)` of the nominal fan; and the mean of `|U|^2` (from `specimen_table`) against `7 (m r_c sigma)^2` | within 4 SE each, SE = sample sd / sqrt n of the same values |
| T3 | — | `unbalance_force` of every specimen of T2 against `|U| Omega^2` by hand | relative <= 1e-14 |
| T4 | — | nominal fan of U: `|U| / (Z m r_c)` | <= 1e-12 |
| T5 | — | L with a plan (`sigma_s` 0.02, n 64, seed 697_003), `q@mean` with `Over::Mean`: against the mean over all 192 (specimen, point) values of `specimen_table` | relative <= 1e-12 |
| T6 | — | common random numbers on L with the T5 plan: (a) candidates A = (1, 0.3) and B = (1.5, 0.3) evaluated alone, in one batch [A, B] and in [B, A]: reduced metrics bit-equal; (b) `q@mean(B) / 1.5` against `q@mean(A)` | (a) bit-equal; (b) relative <= 1e-13 |
| T7 | — | `TolStat::Nominal` terms with the T5 plan against the run without a plan | bit-equal |
| T8 | — | sample divisor: an analytic model whose three specimens give `m` = 1, 2, 4: `m@sd` against `sqrt(7/3)`; and a bit-constant sample (n 5, value 0.1) gives `sd` exactly 0 and mean equal to the value | relative <= 1e-15; exact |
| K1 | (3) | L at s = 1: limit `q >= 0.02`, scale 0.01, every point: violation `(0.02 - 0.0125) / 0.01 = 0.75` (I1 only) | relative <= 1e-9 |
| K2 | (3) | limit `q >= 0.01`, every point | violation exactly 0.0 |
| K3 | — | limits `q >= 0.02` (0.75) and `power <= 2`, scale 1, every point (I2: 0.5): total against the hand sum of the two worst-point violations computed from the record | relative <= 1e-12 |
| K4 | (3) | L with I4 added: the candidate fails (`fitness_of` = `Fitness::FAILED`), the record's message names `I4` and contains `stall`; a feasible candidate without I4 is better under Deb's rules | exact |
| K5 | — | a limit on `nope` (not offered by the model) | located refusal at `validate` naming `nope` |
| K6 | — | aggregated limit `q@nominal` over `Mean` `>= 0.03`, scale 0.01: violation `(0.03 - 0.025) / 0.01 = 0.5` | relative <= 1e-9 |
| W1 | (4) | L with plan (`sigma_s` 0.02, n 64, seed 697_021), 32 candidates `Doe::Lhs { 32, seed 697_022 }`, terms `q@mean` Max `Mean` and `power@mean+1sd` Min `Worst`, limit `q@nominal >= 0.02` every point; sink and score entries on a file ledger with clock 1_700_000_000_000; workers 1, 2, 3, 8 | objective vectors and violation bits equal; ledger file bytes equal |
| W2 | (4) | W1 at workers 1 twice; then plan seed 697_023 | equal bytes and bits; another seed: the objective vectors differ for every one of the 32 candidates |
| W3 | — | O1 (NM, CMA-ES) and O2 (NSGA-II) studies at workers 1 and 4 | store records and `OptResult` bit-equal |
| R1 | (5) | F at `stagger_offset_deg = 0`, points R1 to R3: terms `q@nominal` Max `Mean`, `power@nominal` Min `Worst`, `tone1_pa@nominal` Min `Weighted`, `dp_s@nominal` Min `MeanPlusKSigma(1)`: against `Robust::apply` on `FanRomEvaluator::metrics` of `RomFan::from_axial_fan(axial_fan(0, 0, 0))` called in the test at each point | bit-equal, all four |
| R2 | (5) | the per-point `q` and `dp_s` of R1 | `q` strictly falls, `dp_s` strictly rises from R1 to R3 |
| R3 | — | F with F-T, n 64, seed 697_031, `max_failed_fraction` 0.5: finishes; the failed-specimen count of each point equals the count of failed direct evaluations in `specimen_table`; wall time | finishes; counts equal; time recorded, no limit |
| R4 | — | F with every tolerance `Fixed 0`, n 13, seed 697_032: `@sd` of `q`, `power`, `tone1_pa`, `unbalance` at every point; `@mean` against `@nominal` | sd exactly 0.0; mean bit-equal to nominal |
| R5 | (4), ledger | F, `Doe::Factorial { levels: [4] }` on `stagger_offset_deg`, sink and score entries, fixed clock: entries read back with 0 skipped lines; 12 point entries and 4 score entries; every score entry: fidelity `reduced_order`, `objective` hash = FNV-1a 64 of `to_json()`, `settings.objective` = `to_json()`, 3 `parents` that `resolve` to the record's three point entries; `rerun_check(score, ObjectiveModel, 0, 0)` of every ok score entry | all exact; every rerun passes with difference 0 |
| R6 | (3) | F with R4 (k 4e4) added | candidate failed, message names `R4` and contains `stalled` |
| N1 | Q6 | `tone1_db@nominal` at R1 to R3 against `spl_db(tone1_pa / sqrt 2)` by hand | bit-equal |
| N2 | Q6 | `tone_a3_db@nominal` at R1 to R3 against the formula by hand from `FanRom::tone` (m = 1, 2, 3) at the operating point | relative <= 1e-12 |
| O1 | (6) | L, minimise `power@nominal` `Mean` subject to `q@nominal` over `Mean` `>= 0.03` (scale 0.01); init (1.5, 0.6). NM `NelderMead { budget 400, restarts 0, seed 697_011 }`; CMA-ES `CmaEs { budget 2000, pop None, sigma0 0.3, restarts 0, seed 697_012 }`. Closed form: `s* = 1.2`, `t* = 0.3`, `P* = (25/24) 1.2^3 / 0.5 = 3.6` W | each: best record feasible (violation 0), `P <= 3.6 (1 + 1e-3)`, `|s - 1.2| <= 1e-3`, `|t - 0.3| <= 3e-2` (the power limit alone implies `s - 1.2 <= 4e-4` and `|t - 0.3| <= 0.0224`) |
| O2 | (6) | L, NSGA-II `Nsga2 { budget 4000, pop 40, seed 697_013 }`, terms `q@nominal` Max `Mean` and `power@nominal` Min `Mean`. Closed-form front `t = 0.3`, `P_f(q) = (25/12) (q / 0.025)^3`, `q` in [0.0125, 0.05] | every Pareto member: `-1e-9 <= P / P_f(q) - 1 <= 0.02`; the members' flows span at least 80 % of [0.0125, 0.05] |
| O3 | (6), robust | L with plan `sigma_s` 0.02, n 64, seed 697_014: minimise `power@mean` `Mean` subject to `q@mean-2sd` over `Mean` `>= 0.03`. Closed form for the sample: with `ebar`, `s_e` the sample mean and sd of the 64 drawn errors and `m3` the sample mean of `(1 + e)^3`, `s* = 1.2 / (1 + ebar - 2 s_e)`, `t* = 0.3`, `P* = (25/24) s*^3 m3 / 0.5`. NM (budget 400, seed 697_015) and CMA-ES (budget 2000, sigma0 0.3, seed 697_016) | each: feasible, `P <= P* (1 + 1e-3)`, `|s - s*| <= 1e-3`, `|t - 0.3| <= 3e-2`; `s*` recorded against the population value `1.2 / (1 - 0.04) = 1.25` |
| B1 | (7) | existing tests: `cargo test -p rcfd-api --release --lib design::`, `--test fan_rom_design_w46b`, `--test fan_rom_bands_f268` (fast tests), `--test f249_default_bits`, `--test f260_default_bits`, `--test f297_default_bits`, `--test baseline_hashes` | pass; no existing test file edited |
| B2 | (7) | rcfd-mcp `default_f1`, `tool_list_f275`, `project_list_f270`; `git diff c3590958 -- crates/rcfd-mcp` | pass; empty diff |
| B3 | — | `cargo fmt -p rcfd-api --check`, `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

Mutations (scratch copy, one at a time, each must fail a registered check):

| Id | Mutation | Expected to fail |
|---|---|---|
| M1 | `Over::Weighted` takes the point weights in reversed order | C2, C6 |
| M2 | sample sd with divisor `n` | T8 |
| M3 | a per-candidate seed (`seed` XOR the bits of `x[0]`) | T6 (b), W2 |
| M4 | per-point reduction over the specimens in completion order (results pushed by the workers into a shared list) | W1 |
| M5 | a point-wise limit taken at the mean over the points (the evaluator writes the mean into every point) | K1 |
| M6 | `unbalance_force` with `Omega` instead of `Omega^2` | T3 |
| M7 | `MeanPlusKSigma` of a `Max` term computed as `mean + k sd` | T1 |
| M8 | the bit-constant rule removed (mean = sum / n, sd from the deviations) | R4 or T8; registered as a doubt (with n 13 and 5 the sum of equal values is not exact in general, but it can be) |
| M9 | an aggregated limit computed with `Worst` whatever its `over` | K6 |
| M10 | score entries without `parents` | R5 |

### Decision rule

* **done:** C1 to C6, T1 to T8, K1 to K6, W1 to W3, R1, R2, R4 to R6, N1, N2, O1 (both optimisers), O2, O3 (both), B1 to B3
  met, R3 finished, and every mutation except possibly M8 fails a check.
* **partial:** C, K, W and B met, and a T, R, N or O limit missed (an ignored `*_known_miss` record with the limit intact), or a
  mutation other than M8 survives.
* **todo** (the lead does not take the code): C2, K2, W1, B1 or B2 missed.

### Limits (not covered)

* Library only (D1): no MCP tool, service method, study-JSON key or case key; F-702 carries the exposure.
* One order of aggregation (statistic over specimens per point, then over points; Q3 (a)); no quantile or chance-constraint
  statistic (Q4 (a)); fixed `n` with common random numbers (Q5 (a)); the bias of one sample set is not estimated.
* The reduced-order target represents only common stagger and tip-gap errors on the aero side, and blade mass and thickness on
  the mass side; per-blade aero, eccentricity and roughness tolerances are refused. Vibration is a rotor force from blade mass
  only. Noise is tonal steady loading only. Every number ranks variants of the model, not real fans (no measured fan data).
* An aggregated limit's value is repeated in every point entry of the ledger.
* The impedance points of set-up F are placed relative to this fan's smoke curve, not taken from a measured system (Q1 (a)).

### Open questions taken on the maintainer's standing instruction of 2026-11-11 (take the recommendation)

* Q11, second part (which formulation the documentation shows first): "minimise the noise metric subject to flow at each
  impedance point and a power cap" (single objective with limits; it needs no weights nobody can justify).
* Q12, second part (the breakdown's H1 row): "partial (F-697)" when this lands, since only the reduced-order tier is covered;
  the row is changed in `docs/fan-project-breakdown.md` with the code.
* Deviation from the stub's R set-up: the points cannot sit at 25, 50 and 75 % of free delivery (smokes 1 and 2: stall); they
  are placed inside the unstalled range of set-up F as listed.

## Addendum 1: results (2026-11-16)

Code: commit 71792e36 (registration 6b695045 before it). New `crates/rcfd-api/src/design/fan_objective.rs` (the design of
amendment 1), `crates/rcfd-api/tests/f697_fan_objective.rs` (22 tests, none ignored), one module line and one doc line in
`design/mod.rs`; docs `docs/design-loop.md` (one section, one code-map row) and `docs/fan-project-breakdown.md` (H1 row:
"partial (F-697, ADR 0620: reduced-order tier only)"). No other existing file, no shared solver file, no golden, no hash table, no
case key, no MCP tool. The registered run was made once: `cargo test -p rcfd-api --release --test f697_fan_objective --
--test-threads=2`, 22 passed in 16.3 s (log `f697-scratch/run1.log`). `cargo fmt -p rcfd-api` then reformatted the two new files
(layout only) before the commit.

| Check | Limit | Measured | Met |
|---|---|---|---|
| C1 | rel <= 1e-9 | 0, 1.4e-16, 0 | yes |
| C2 | rel <= 1e-12 against `Robust::apply` by hand | 0 for all four | yes |
| C3 | rel <= 1e-9 against 0.025, 0.0125, 0.02125, 0.025 - 0.0125 sqrt(2/3) | 1.4e-16, 0, 0, 1.2e-16 | yes |
| C4 | rel <= 1e-9 (25/12 W, 2.5 W) | 0, 0 | yes |
| C5 | bit-equal | equal (-0.02125) | yes |
| C6 | both changes > 1e-3 | 7.3e-2 (k doubled), 0.353 (weights) | yes |
| T1 | abs(z) <= 4 | mean -0.60, mean+2sd -1.46, mean-2sd 0.77 | yes |
| T2 | abs(z) <= 4 | mean abs(U) 1.13684e-6 against 1.12112e-6 kg m (z 1.68); mean abs(U)^2 z 2.02 | yes |
| T3 | rel <= 1e-14 | 0 | yes |
| T4 | <= 1e-12 | 7.3e-17 | yes |
| T5 | rel <= 1e-12 | 1.4e-16 | yes |
| T6 | (a) bit-equal; (b) rel <= 1e-13 | (a) equal; (b) 3.7e-16 | yes |
| T7 | bit-equal | equal | yes |
| T8 | rel <= 1e-15; sd exactly 0 and mean bit-equal | 1.5e-16; 0 and equal | yes |
| K1 | rel <= 1e-9 (0.75) | 0 | yes |
| K2 | exactly 0 | 0 | yes |
| K3 | rel <= 1e-12 | 0 (1.25 = 0.75 + 0.5) | yes |
| K4 | FAILED, reason names I4 and stall, ranked after the feasible one | "I4: point 'I4': nominal: operating point q = 1.800000e-3 m^3/s lies on a rising fan characteristic ...: stall region" | yes |
| K5 | located refusal naming `nope` | "limit 'nope': the model analytic.linear_fan does not offer the metric 'nope'" | yes |
| K6 | rel <= 1e-9 (0.5) | 1.2e-15 | yes |
| W1 | bits and bytes equal at workers 1, 2, 3, 8 | equal; ledger 150590 bytes, 128 lines (96 point entries, 32 score entries) | yes |
| W2 | same seed equal; other seed 32 of 32 differ | equal; 32 of 32 | yes |
| W3 | NM, CMA-ES, NSGA-II records and results bit-equal at workers 1 and 4 | equal, all three | yes |
| R1 | bit-equal, four terms | four of four | yes |
| R2 | q strictly falls, dp_s strictly rises | q 0.044359, 0.039472, 0.035836 m^3/s; dp_s 9.84, 23.37, 32.11 Pa | yes |
| R3 | finishes; counts equal; time recorded | finished; failed specimens 0, 0, 3 (R1, R2, R3) both ways; 7.0 s for 65 specimens x 3 points (4 workers) | yes |
| R4 | sd exactly 0, mean bit-equal to nominal | 12 of 12 (metric, point) pairs | yes |
| R5 | 0 skipped, 12 + 4 entries, fields, parents resolve, reruns pass with difference 0 | as registered; 2 of the 4 candidates (stagger offset -3 and -1 deg) stall at R3 and have `diverged` score entries; the 2 ok ones re-run with difference 0 | yes |
| R6 | failed, names R4 and stalled | "R4: point 'R4': nominal: operating point q = 2.137518e-2 m^3/s is stalled: 40 of 40 strips ..." | yes |
| N1 | bit-equal | equal at R1 to R3 (-86.79, -85.15, -84.40 dB) | yes |
| N2 | rel <= 1e-12 | 0 at R1 to R3 | yes |
| O1 NM | feasible, P <= 3.6 (1 + 1e-3), abs(s - 1.2) <= 1e-3, abs(t - 0.3) <= 3e-2 | s - 1.2 = 1.6e-12, t 0.300135, P / 3.6 - 1 = 3.7e-8, 199 evaluations | yes |
| O1 CMA-ES | same | s - 1.2 = 2.2e-16, t 0.300000, P / 3.6 - 1 = 6.7e-16, 1094 evaluations | yes |
| O2 | -1e-9 <= P / P_f - 1 <= 0.02; span >= 0.8 | [1.0e-11, 1.78e-2] over 1422 Pareto records; span 1.000 | yes |
| O3 NM | feasible, P <= P* (1 + 1e-3), abs(s - s*) <= 1e-3, abs(t - 0.3) <= 3e-2 | s* 1.251698 (population 1.25; sample ebar -3.1e-3, s_e 0.01909); s - s* = 0, P = P* = 4.051873, 306 evaluations | yes |
| O3 CMA-ES | same | s - s* = 0, P = P*, 1181 evaluations | yes |
| B1 | pass | `design::` 76 passed (3 ignored, pre-existing); `fan_rom_design_w46b` 1, `fan_rom_bands_f268` 4 (1 ignored heavy record, pre-existing), `f249_default_bits` 4, `f260_default_bits` 1, `f297_default_bits` 3, `baseline_hashes` 3 (1 ignored, pre-existing); no existing test file edited | yes |
| B2 | pass, empty diff | `default_f1` 2, `tool_list_f275` 7, `project_list_f270` 1 passed; `git diff c3590958 -- crates/rcfd-mcp` empty | yes |
| B3 | clean | `cargo fmt -p rcfd-api --check` clean; `cargo clippy -p rcfd-api --all-targets -- -D warnings` clean | yes |

Mutations (scratch copy `f697-scratch/mcopy` from `git archive` of 71792e36, script `mutate.py`, logs `mut_*.log`; the unmutated
copy passed 22 of 22, `mut_control.log`): M1 caught by C2 and R1, not by C6 as expected (both weight vectors are reversed, so they
still differ); M2 by T8; M3 by T6 and O3, not by W2 as expected (a per-candidate seed is still a function of the candidate, so two
runs agree and another base seed still differs); M4 by W1 and also W3, T6, R1, R3, R4, N1; M5 by K1; M6 by T3; M7 by T1 and O3;
M8 by R4 (T8 did not see it: five copies of 0.1 sum and divide back exactly); M9 by K6, O1 and O3; M10 by R5. Ten of ten caught.

Observations (not checks): the `violation` metric of a score entry with no limit is `-0.0` (the `f64` sum of an empty iterator in
`Study::violation_of`); with a limit that is met it is `+0.0`. NSGA-II's Pareto set is taken over all 1422 store records of the
run, so O2 covers every non-dominated evaluation, not only the last population.

Deviations: none from the registration. The two smoke runs before it are disclosed in amendment 1. Not run: the whole workspace
(the lead's gate), rcfd-mcp tests other than the three listed.

Decision by the registered rule: **done**. Every check met, R3 finished, every mutation fails a check.

### Remaining questions (each with a recommendation; none blocks the decision)

1. **Aggregated limits repeat their value in every point entry of the ledger.** Options: (a) keep; (b) put aggregates only in the
   score entry and give `Study` a constraint over an aggregate (a change to `study.rs`). Recommendation: (a); (b) touches the
   shared study code for no change of result.
2. **`ComputerFan::default()` is not usable in the reduced-order model** (stalled below about 80 % of free delivery, smoke 1).
   Options: (a) leave it, set-up F is the working row; (b) a follow-up that gives the schema default a blade the reduced-order
   model runs over most of its range. Recommendation: (b), a small follow-up, since the schema default is what a user tries
   first; that item must first check whether the default is hashed or in a golden.
3. **The other aggregation order (Q3 (b)) and a chance constraint (Q4 (c)).** Options: (a) wait for a study that needs them; (b)
   add now. Recommendation: (a); the chance constraint first when a tolerance requirement is written down.

## For the lead: roadmap entries

> Proposed status: done

> Fan breakdown part H1 (docs/fan-project-breakdown.md). ADR 0620, 2026-11-16. DONE (reduced-order tier only). Library only, opt-in (no case key, no MCP tool, no default changed): design::fan_objective states a fan objective once (impedance points as system curves with weights, terms with a statistic over tolerance specimens and a combination over the points, limits held at every point or on an aggregate, an optional tolerance plan with one seed for every candidate) and builds an ordinary StudyDef, so Nelder-Mead, CMA-ES, NSGA-II, the study store and the ADR 0623 ledger sink run it unchanged; objective_score entries carry the F-700 objective identity and parents and re-run through ObjectiveModel. FanRomTarget evaluates the ADR 0119 reduced-order fan on a ComputerFan with flow, pressure rise, power, efficiency, tone1_pa, tone1_db, the A-weighted sum of the first three tones and the static-unbalance force of the specimen's blades. Checks: linear fan against three quadratic systems, per-point flows within 1.4e-16 of the closed form and mean, worst, weighted and mean-plus-sigma flows equal to the hand aggregation; mean static unbalance of N(0, sigma) blade mass errors on 7 blades within 1.7 SE of r_c m sigma sqrt(7 pi / 4) at 4096 samples and a linear Gaussian metric's mean + 2 sd within 1.5 SE; the violation of the one violated point is 0.75 exactly, a stalled point fails the candidate with a located reason (linear fan and reduced-order fan), feasible violation exactly 0; objective vectors, violations and ledger bytes identical for 1, 2, 3 and 8 workers and for two runs, all 32 differ for another seed; reduced-order objective values bit-equal to FanRomEvaluator::metrics aggregated by hand, flow strictly falling as the system coefficient rises; Nelder-Mead and CMA-ES reach the constrained optimum (speed scale 1.2, pitch 0.3, 3.6 W) within 4e-8 relative in power and the robust optimum of the sampled plan, NSGA-II front within 1.8 percent of the closed-form front over the full flow range; baseline_hashes, the design tests and the tools-list goldens unchanged. Ten of ten mutations caught. Not covered: per-blade aero, eccentricity and roughness tolerances (refused by the one-row model), the other aggregation order and chance constraints, any CFD evaluator behind the layer; every number ranks variants of the model, not real fans (no measured data). MCP exposure is F-702.

> Follow-up proposed: Schema default fan usable in the reduced-order model. Deps F-697. Acceptance: ComputerFan::default() through RomFan::from_axial_fan runs unstalled at 3000 rpm for every flow above 40 percent of its free delivery in the reduced-order model, every ADR 0118 schema, checker and ensemble test keeps its result, and no golden or hash table changes (or the needed change is listed for the maintainer before any edit).
