# ADR 0645: F-716 registration: a stall pre-check of the reduced-order model before each fan evaluation of the surrogate loop

Status: **REGISTERED 2026-11-16, written before any committed code of F-716 and before any run of its checks.** Not yet
accepted by the maintainer. Results go in the addenda at the end; nothing above the addenda is changed after a run.

Roadmap item F-716 (deps F-698, F-697), proposed by [ADR 0621](0621-f698-surrogate-model-gaussian-process-and-expected-improvement.md)
addendum 1, remaining question 3 ("High failure share on the fan (34 to 39 of 50): recommend an initial design filtered by a
cheap stall pre-check of the reduced-order model, in a follow-up"). Parents: ADR 0621 (F-698: `design::surrogate`,
`design::bayes`, the set-up F3 and the check R1), [ADR 0620](0620-f697-registration-fan-objective-over-impedances-and-tolerances.md)
(F-697: `FanObjective`, `ObjectiveModel`, `FanRomTarget`, `SpecimenModel`), [ADR 0119](0119-wave46b-reduced-order-axial-fan-tier.md)
(the reduced-order fan and its stall test). Number 0645 given by the lead. Branch `f716-work` from d40cd665, worktree
`w49i-work/f716`, scratch `w49i-work/f716-scratch`.

House rules (`w49i-work/AGENT_RULES.md`): limits fixed here and not changed after a run (a miss becomes an ignored
`*_known_miss` test with the limit intact and the measured number); each registered run made once; existing tests not edited;
defaults bit-identical; no edit of `baseline_hashes.rs`, of a hash table or of a golden; no new dependency, no unsafe code;
clean-room; the roadmap files are not edited. Every choice the maintainer would normally be asked about is taken on the
maintainer's standing instruction of 2026-11-11 (take the recommendation), and the maintainer decisions of 2026-11-15 in
ADR 0621 apply (D1: library only, no MCP tool).

Clean-room: the method is first principles (a deterministic filter in front of an evaluation, and the monotone dependence of
blade incidence on flow derived below) on top of the published methods ADR 0621 already cites (Jones, Schonlau and Welch 1998;
Forrester, Sobester and Keane 2006). No source listing and no vendor documentation was opened.

## What F-716 asks

The acceptance, binding:

> On the F-698 R1 set-up the failed share of the 50 evaluations falls below 0.3 on every seed from 1 to 10 while R1 still holds on 9 of 10, with the pre-check evaluations of the reduced-order model counted in the ledger. Every limit not stated here is registered in the ADR before the run.

The brief adds: opt-in, F-698's R1 result and every existing test keep their results (default bits unchanged), library only, no
MCP tool, no golden; negative controls (pre-check off) and mutations registered before the run; heavy runs as `#[ignore]`
records run once.

Reading taken (stated here so that it is fixed before the run): "the 50 evaluations" are the 50 evaluations of the objective
model (`ObjectiveModel` over the F3 objective, the model R1 counts); "failed share" is the number of those 50 ledger entries
whose verdict is not `ok`, divided by 50 (below 0.3 means at most 14). A pre-check is a separate, cheaper evaluation of the
reduced-order model, written to the ledger as an `evaluation` entry of its own evaluator id; it is not one of the 50, and every
one of them is counted and reported (check A3).

## What was read, and what was done before this text (disclosed)

Read: `AGENT_RULES.md` (including the rule 7 update of 2026-11-16: own registered tests, `--lib` of touched crates, named
existing test files, `docs_site` when docs/ changes; no crate-wide integration suite); ADR 0621 in full (stub, amendment 1,
amendment 2, addendum 1, its roadmap entries); `design/bayes.rs`, `design/fan_objective.rs` (`TolerantEvaluator`,
`ObjectiveModel`, `FanRomTarget`), `design/fan_rom_eval.rs`, `rcfd_post::fan_rom` (`FanRom::point`, `solve_strip`,
`operating_point`), `tests/f698_surrogate.rs` (the F3 set-up and `r1_r2_reduced_order_fan_record`), and the F-698 log
`f698-scratch/run_r1r2.log` (per-seed R1 numbers; quoted under NC1).

What the F-698 record says (ADR 0621 addendum 1 and its log): `P_ref = 1.455477 W` (the 21^3 grid, 5129 of 9261 points
feasible); per seed 1 to 10 the failed counts 37, 35, 34, 36, 36, 36, 39, 36, 34, 36 of 50 and best / `P_ref` 0.99328,
0.99325, 0.99304, 0.99351, 0.99317, 0.99345, 0.99328, 0.99328, 0.99306, 0.99311; "most of them in the initial design and next
to the stall boundary, where the optimum sits". With `n0 = 15` and 45 percent of the box failing (97 of a 6^3 grid in ADR 0621
smoke S7), about 7 of the 15 initial points fail, so about 28 of the 35 model picks fail (80 percent). Arithmetic: a pre-check on
the initial design alone (ADR 0621's suggestion) removes about 7 failures and leaves about 27 to 32 of 50, a share above 0.5;
the acceptance needs the pre-check on the model picks too.

**Prototype and an unfinished smoke (disclosed).** Before this text a prototype of the design below, without the ranked
fallback of D3 and without the per-step cap, was written in a scratch copy (`f716-scratch/proto`, a `git archive` of d40cd665,
outside the worktree): `PreCheck`, `PreCheckScope`, `run_prechecked`, the refactor of `run` into a shared loop, and
`StallPreCheck` in `fan_objective.rs`, with one smoke test file there (`zz_f716_smoke.rs`, not committed). The smoke (seeds 101
to 105, 15 runs in parallel: pre-check off, on every pick at R3, on the initial design at R3; `max_rejected` 100 000, so in
effect no cap) printed nothing before it was stopped: it prints only at the end, and after 277 minutes two runs were still
going at a steady two cores and 79 MB. It was stopped by its own PID (60096, the test binary, not cargo) after the diagnosis was
sent to the lead, on the lead's agreement. This is a deviation, recorded again in the results addendum. **No smoke number exists and none is used below.** Diagnosis from the code, not from output: in the prototype a
failed pre-check costs no evaluation, the proposal is refitted after every rejection, and expected improvement times the failure
classifier keeps choosing points at the stall boundary (where the optimum lies). A run whose picks keep landing on the stalled
side never reaches its 50 evaluations, and each rejection adds an observation, so every Gaussian-process fit gets slower
(`O(n^3)` per likelihood value, ten starts). What this changed in the design: the ranked fallback within one fit (D3), a cap per
step and a binding cap per run (D4), both derived below and fixed now, and a progress line per step (D5). Every number in the check table comes from ADR 0621
or from a calculation here.

## Design (fixed now)

### D1. The pre-check model: `fan_objective::StallPreCheck`

`StallPreCheck { objective: &FanObjective, model: &M: SpecimenModel, points: Vec<String> }` implements `ledger::Model` (id
`rcfd.fan_objective.stall_precheck`, version `1`, fidelity that of `model`). For a candidate it builds the nominal specimen
(`model.nominal(x)`), and at each named point, in the given order, calls `model.evaluate(&nominal, point, &objective.needed())`
with the same finiteness test as `TolerantEvaluator::specimen_table`. The first failing point ends the check with verdict
`diverged` and the reason `point '<label>': nominal: <error>`, the reason the full evaluation gives at that point
(`TolerantEvaluator::reduce`). A geometry failure gives `nominal: <error>`. Unknown labels, an empty list or a missing parameter
give `not_evaluated` with a located reason. A pass gives `ok` with the metrics `<metric>@<label>`.

Exactness (first principles): the pre-check runs the same deterministic call on the same specimen as the full evaluation does at
that point, so **a candidate it fails also fails the objective**, with the same reason. A candidate it passes can still fail at
the other points, or (with a tolerance plan) in its specimens. Check A5 verifies the implication on every rejected point.

Which point (set-up F3: R1, R2, R3 at `k` = 5e3, 1.5e4, 2.5e4 Pa s^2/m^6, `l` = 0). The ROM takes the highest-flow crossing
`q*` of the fan's static-pressure curve with `k q^2`. For `k3 > k1`, at `q3*` the fan pressure equals `k3 q3*^2 > k1 q3*^2`,
so the `k1` crossing lies at a higher flow: the operating flow falls as `k` rises. At fixed speed, a strip's axial velocity
`c_x = q / A` falls with `q` while the blade speed `Omega r` stays fixed, so the inflow angle measured from the axial direction
grows, and so does the incidence `alpha_e` of every strip. The attached-flow test `2 pi |alpha_e| <= cl_max` therefore fails
first at the lowest flow. The most loaded point R3 is the one checked: `points = ["R3"]`. It costs one operating point
(a scan of 100 intervals plus bisection, and one tone), against three for the full evaluation. Failures the single-point check
cannot see: stall at negative incidence at high flow, no crossing below `q_hi` at R1, or a rising characteristic at R1 or R2.
These stay failures among the 50, and A1 measures them.

### D2. The driver: `bayes::run_prechecked`

`PreCheck { model: &dyn Model, scope: PreCheckScope (Initial | Every), max_rejected: usize, max_per_step: usize }`;
`run_prechecked(spec, model, pre, ledger) -> Result<PreCheckedResult { run: BayesResult, prechecks: Vec<Entry>, rejected },
String>`. Refused, located, when `pre.model.id() == model.id()`, or when `max_per_step` is 0.

`run(spec, model, ledger)` becomes `run_inner(spec, model, None, ledger)`; with `None` every computed value, entry, provenance
key and ledger line is the one of d40cd665 (check B1 compares bytes). With `Some(pre)`:

* Observations: the objective's rows as in `run`, plus a `Failed` observation for every `evaluation` entry of `spec.study` from
  `pre.model.id()` whose verdict is not `ok`, from the ledger read at the start and from this run (resume, check L1). Passed
  pre-checks are not observations (the evaluation that follows is). `propose` puts all of them in canonical order.
* Scope: `Initial` checks the picks made without a model (`PickKind::Initial` and `Filling`); `Every` checks every pick.
* A pre-check entry: kind `evaluation`, evaluator the pre-check id, variant `<study>/gp_ei/<n>/precheck/<j>` (`n` the
  observation count of the step, `j` the try within the step), seed, the clock of the spec, and provenance
  `{driver "gp_ei", role "precheck", scope, iteration, batch_index, try, acquisition, acquisition_value, predicted_mean,
  predicted_sd, feasibility, evaluated_model, passed}`. It is appended to the ledger before the evaluations of the step.
* An evaluated pick gets the provenance of `run` plus `precheck`: `"passed"` or `"not_run"` (out of scope). `spec.budget`
  counts evaluations of `model` only; `BayesResult::entries`, `picks`, `failed` and `best` concern them only.
* The `surrogate_summary` entry gets two more metrics, `prechecks` and `precheck_failed`.

### D3. The ranked fallback within one fit (batch 1)

When the pick of a step fails its pre-check, the driver does not refit at once. Instead it asks the next point of the same
step's ranked list, from the same fitted models. This bounds the cost to one fit per step, and it stops a refit from choosing
the point next to the rejected one. The list (`propose_ranked`, internal; `propose` is unchanged and is its first element) is
the five refined finalists in score order (the winner first; ties by index), then the raw candidates in score order, keeping
only candidates with a finite score at least **0.05** (unit cube, Euclidean) from every earlier member of the list. The 0.05
is 2.5 times the 0.02 standard deviation of the near perturbations of D6 of ADR 0621, so that a fallback point is not one of
the same cluster. Each member carries its own pick record (`models.acquisition` at the point). A member that is a model-free
pick has no list: the next initial point is the next proposal, as in `run`. With batch > 1, a rejected pick is dropped without
a fallback (not used by any registered check).

### D4. Caps (derived, fixed now)

* `max_per_step = 20` tries in one step. After 20 failed tries the step evaluates nothing. The next step refits with the 20
  new failed observations.
* `max_rejected = 400` per run. One more is a located refusal: the run returns `Err` naming the pre-check id, the cap and the
  evaluations made, and the ledger keeps every entry written. Derivation: with no help from the fallback, the F-698 rate of 80
  percent failed model picks costs 0.8 / 0.2 = 4 rejections per evaluation, about 4 x 35 + 7 = 147 per run. 400 is about 2.7
  times that, and it bounds the observation count at 450, where one fit costs at most minutes (ADR 0621 risk 5). Termination
  follows: each step either evaluates (at most 50) or adds at least one rejection (at most 401 before the refusal).
* A refused run is a miss of A1 for its seed.

### D5. Progress output (registered on the lead's instruction of 2026-11-16)

`PreCheck` has a field `progress: Option<&dyn Fn(&str) + Sync>`, `None` by default. When set, it is called once at the end of
every step with one line: `seed <s> step <n>: evaluations <e>/<budget>, failed <f>, pre-checks <p>, rejected <r>, observations
<o>, <t> s`. The library prints nothing itself. The heavy record passes a closure that prints the line with `println!`, so that a
slow or stuck run shows where it is (the lesson of the unfinished smoke). The callback does not change any value.

### D6. No further smoke

No further smoke run is made: the caps of D4 are derived, and no limit of the check table needs a smoke estimate. The first
runs of the new code are the registered ones.

## Set-ups (fixed now)

* **F3** exactly as `tests/f698_surrogate.rs` (`fan_f`, `target_f3`, `F3_BOX`, `objective_f3`, `spec_f3`; copied into the new
  test file, since an existing test file is not edited). `P_ref = 1.455477 W` from ADR 0621 addendum 1. The grid is not
  recomputed: NC1 reproduces the F-698 runs instead, which checks that the set-up is the same. Target: best `<= P_ref (1.01)`.
  The 6-decimal rounding of `P_ref` moves the target by at most 5e-7 W. This is recorded with each best.
* **PC**: `StallPreCheck { objective_f3, target_f3, points ["R3"] }`, scope `Every`, `max_rejected 400`, `max_per_step 20`.
* **BH** (fast): Branin on `[-5, 10] x [0, 15]` as an `FnModel` (id `test.branin`). The pre-check is an `FnModel` (id
  `test.hole`) that is `diverged` ("point 'hole'") inside the unit-cube disc of radius 0.15 around (pi, 2.275), the F1 disc of
  ADR 0621, and `ok` outside. The objective there fails inside the same disc too.

## Checks (limits fixed now)

New test file `crates/rcfd-api/tests/f716_stall_precheck.rs`. Release profile. Seeds are `BayesOptions::seed`.

| Id | Clause | What | Limit |
|---|---|---|---|
| A1 | acceptance | F3, `run_prechecked` with PC, budget 50, seeds 1 to 10: failed objective entries of 50 | every seed `<= 14` (share `< 0.3`); a refused run is a miss |
| A2 | acceptance | the same runs: seeds with best feasible `<= P_ref (1.01)` | `>= 9 of 10` |
| A3 | acceptance | the same runs, each with its own ledger file: entries of evaluator `rcfd.fan_objective.stall_precheck`, study `f698_rom`, kind `evaluation`, fidelity `reduced_order`; their count and failed count against `prechecks.len()` and `rejected`; the objective entries; the summary metrics `prechecks`, `precheck_failed` | equal; 50; equal; recorded per seed: pre-checks, rejected, ROM operating points (`prechecks + 3 x 50`) |
| A4 | located | the same runs: every failed objective entry names one of R1, R2, R3; every failed pre-check names `point 'R3'` | all |
| A5 | exactness | every rejected pre-check point of A1, evaluated again by the objective model | fails, reason names `point 'R3'`, all |
| A6 | design | the same runs: all points (objective and pre-check) inside the box, unit-cube distance `>= 1e-6` to every earlier point | 0 violations |
| NC1 | control | F3, `run` (pre-check off), budget 50, seeds 1 to 10 | every seed: failed in [34, 39] and best / `P_ref` in [0.9930, 0.9935] (ADR 0621); per seed against the F-698 log, recorded |
| NC2 | control | F3, `run_prechecked` with PC but scope `Initial`, seeds 1 to 10 | recorded (failed counts, best, pre-checks): no limit |
| F1 | fast | F3, PC, budget 18, seed 1, with a ledger: 18 objective entries; ledger counts as A3; A4; A5; A6; every objective entry has provenance `precheck` `"passed"` and an earlier pre-check entry with verdict `ok` at the same parameters (bits) | all hold |
| F2 | fast, control | BH objective, pre-check `FnModel` always `ok` (id `test.pass`), scope `Every`, budget 25, seed 698_901: the objective entries against `run`'s (params, metrics, verdict, reason, variant bits) | equal; 25 pre-checks, 0 rejected |
| F3c | fast | BH objective, pre-check always `diverged` (id `test.never`), `max_rejected 5`: the result; the ledger | `Err` containing `max_rejected = 5`; 6 pre-check entries, 0 objective entries |
| F4 | fast | a pre-check model with the objective model's id; `max_per_step 0` | both refused, located |
| F5 | fast | BH with the hole pre-check, scope `Initial`, budget 25, seed 1: every pre-check entry's `acquisition` | `initial` or `filling`, and the count equals the model-free picks tried |
| L1 | fast | BH with the hole pre-check, scope `Every`, budget 25, seed 1, clock fixed: one run on a ledger, against a run of 15 then a run of 10 on another ledger | evaluation lines (objective and pre-check) byte-equal |
| D1 | fast | L1's single run at workers 1 and 8 | entries and pre-checks bit-equal |
| B1 | defaults | `run` on the F-698 L1 set-up (BH objective, budget 25, seed 698_901, ledger) and on F3 (budget 18, seed 1, no ledger): an FNV-1a 64 digest of the ledger lines and of the entries' JSON | equal to the digest the same test gives on a `git archive` of d40cd665 (scratch) |
| B2 | existing | `cargo test -p rcfd-api --release --lib`; `--test f698_surrogate` (non-ignored); `--test f697_fan_objective`; `cargo test -p rcfd-cli --release --test docs_site` | pass; no existing test file edited |
| B3 | — | `cargo fmt -p rcfd-api --check`; `cargo clippy -p rcfd-api --all-targets -- -D warnings` | clean |

Heavy record (one `#[ignore]` test, run once): A1 to A6, NC1 and NC2 together, the 30 runs in parallel (one worker each).
Estimated at most about an hour from D4 (F-698's 10 runs took about 5 minutes without the grid). The fast tests F1 to F5, L1, D1
and B1 cover the same code paths.

## Mutations (scratch copy, one at a time, fast tests only)

| Id | Mutation | Expected to fail |
|---|---|---|
| M1 | failed pre-check entries not fed back as observations | F1 or L1 (A6: the same point is tried again), or a refusal |
| M2 | a failed pre-check does not stop the evaluation | F1 (an objective entry after a failed pre-check) |
| M3 | pre-check entries not appended to the ledger | F1 (ledger counts) |
| M4 | `max_rejected` not enforced | F3c |
| M5 | scope ignored (`Initial` behaves as `Every`) | F5 |
| M6 | `StallPreCheck` checks the objective's first point instead of the named ones | F1 (A4 and A5 name R3) |
| M7 | pre-check entries read from the ledger ignored | L1 |

## Decision rule

* **done**: every check met (NC2 recorded) and M1 to M7 caught.
* **partial**: B1, B2, B3, F1 to F5, L1, D1 and NC1 met, and A1, A2, A3, A4, A5 or A6 missed (an ignored `*_known_miss` record
  with the limit intact), or a mutation survives.
* **todo** (the lead does not take the code): B1, B2, F2 or NC1 missed (defaults or the control broken).

## Limits (not covered)

* Library only: no MCP tool, no study method, no case key, no golden change.
* In R1 the "expensive" evaluation is itself the reduced-order model, so the pre-check saves no work there: the run makes more
  ROM operating-point solves than F-698 did (A3 records them). It saves work only when the evaluated model is dearer than the
  pre-check (CFD behind a ROM pre-check). That use is not tested, because no CFD fan objective exists for the loop.
* One pre-check point (R3) chosen by the derivation of D1. The single-point check misses stall at negative incidence and missing
  crossings at the other points; A1 measures what is left.
* Batch 1 only for the fallback; no tolerance plan in F3 (a plan's specimen failures are invisible to the nominal check).
* Every number ranks variants of the reduced-order model, not real fans (no measured data).
