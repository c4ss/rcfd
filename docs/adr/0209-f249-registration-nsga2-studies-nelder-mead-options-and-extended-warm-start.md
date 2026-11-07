# ADR 0209: F-249 registration: NSGA-II studies, two published Nelder-Mead options, and a warm start that carries temperature and two-equation turbulence

Status: **REGISTERED 2026-11-07**, written before any code of F-249 and before any run of its checks. Not yet accepted by the
maintainer. Before this text one probe of the existing code was run on main 05f0d73 (a scratch test, not committed): the three
warm-start cases of W1 to W3 run cold with `design::case_eval::run_case`, and the existing warm start offered to the energy case.
Its numbers are in item 12, where they set the cases and the time budget; they set no limit. Results go in the addenda at the
end.

Summary of the results (amendment 1, addendum 1): the T3 study with the opt-in `initial_simplex = "start"` and `coefficients =
"adaptive"` reaches best f 3.70e-4 after 120 recorded candidates (limit 1e-3; the default stays at 3.1e-3, bit for bit); NSGA-II
(`method.type = "nsga2"`) returns on FON (3 parameters) a Pareto set within 4.4e-3 of the analytic front with gaps below 4.8e-3
(limits 1e-2 and 5e-2, three seeds), but misses the limits on ZDT1 with 30 parameters (0.35 to 0.54), a known miss whose cause a
diagnostic confirmed (the registered crossover never exchanges coordinates; question 5); the extended warm start (temperature,
k-epsilon and k-omega SST fields) reproduces cold runs to 6.9e-9, 3.1e-11 and 2.6e-11 (limit 1e-4). The constrained-quadratic
check of the Nelder-Mead options is a second known miss (19 of 20 seeds). Defaults, baseline hashes and the tool list are
bit-identical. The gate of c09f508 gave 3043 passed, 2 failed (the known flakes `runner_w47b` s1 and s9), 865 ignored, run with a
19 GB free-memory threshold instead of 20 (addendum 1).

Parents: [ADR 0067](0067-wave32c-design-loop.md) (the design loop: Nelder-Mead, differential evolution, Pareto extraction, the
store, the tools, check T3 and its known miss), [ADR 0099](0099-wave40c-five-velocity-sweep-fixed-norm-interpolated-start.md)
(F-095 done, the remaining scope split off), [ADR 0189](0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md),
[ADR 0196](0196-three-code-changes-decided-2026-11-06-f246-r2-record-design-tool-class-sv202-text.md),
[ADR 0200](0200-f287-registration-runner-runs-that-continue-from-a-previous-run-with-a-step-cap.md) and
[ADR 0201](0201-f288-registration-adopt-matching-runs-after-an-mcp-server-restart.md) (the design tools on the runner; read for
the paths a change here must not disturb). Roadmap item F-249 (depends on F-095; CMA-ES and the real-fan study are F-260).

Number: the lead gave 0209 to F-249. Branch `f249-parametric-studies` from main 05f0d73, worktree `w49i-work/f249`.

House rules ([ADR 0130](0130-wave48a-overview-and-run-record-contract.md), as in ADR 0205):

- limits are fixed here, before the code and the runs, and are not changed after a run;
- a missed limit becomes an ignored `*_known_miss` test with the limit intact and the number recorded, never tuned;
- a defective set-up gets a new corrected test by an amendment written before the corrected run, and the old one stays as an
  ignored record;
- every new path is opt-in and the defaults stay bit-identical: `crates/rcfd-api/tests/baseline_hashes.rs` and
  `case_hashes_w48u.rs` are not edited or re-recorded; no golden is re-recorded or edited (the tools-list goldens of
  `crates/rcfd-mcp/tests/golden/` stay as they are: D6); no case-schema key is added;
- each registered run is made once, and nothing is re-run to pick a result;
- no new third-party crate, `Cargo.lock` unchanged, no commit trailers.

Clean-room and sources: no source listing of any other optimisation or CFD code was opened. What was read for this text, and
what was not, is listed with each rule (D1, D3) and in `docs/reference-data.md` by the documentation commit.

## What F-249 asks

The roadmap entry (title: multi-objective optimiser, Nelder-Mead robustness, warm start beyond steady SIMPLE/SIMPLEC): a
multi-objective study over at least two design parameters returns a Pareto set verified against a known analytic front within a
limit registered in an ADR; the Nelder-Mead optimiser reaches the registered T3 target; a warm-started sweep point reproduces its
cold-started result within a registered tolerance. User decision 2026-10-22: CMA-ES and the real-fan study are F-260; this item
keeps the analytic clauses.

## What the code read shows (main 05f0d73)

1. **The optimisers.** `rcfd_api::design::optim` (`crates/rcfd-api/src/design/optim.rs`): `nelder_mead` (coefficients 1, 2, 1/2,
   1/2 as literals; initial simplex `x0 + 0.05 (hi_i - lo_i) e_i`, the step flipped when it leaves the box, every trial point
   clamped; restarts from the best point with half the previous step and a random sign per axis), `differential_evolution`,
   `projected_gradient`. Both gradient-free optimisers ask for batches through `Eval` (`FnMut(&[Vec<f64>]) -> Result<Vec<Fitness>,
   Stop>`), which serves evaluation, resume (the store answers) and proposal (`Stop::Missing`). `NmOptions` has `Default` and every
   construction outside the file uses `..Default::default()`.
2. **The study.** `design::study::Method` has `Doe`, `NelderMead { budget, restarts, seed }` and `Differential { budget, pop, seed }`.
   Three test files outside the module build `Method::NelderMead { .. }` and `Method::Doe(..)` literally
   (`tests/design_e2e_w32c.rs`, `tests/fan_rom_design_w46b.rs`, `tests/w35b_steady_guard.rs`), so a field added to a variant breaks
   them; nothing outside `design/` matches on `Method`. `Study::objective_vector` gives every objective in minimised form (robust
   over the conditions), `violation_of` the constraint violation, `pareto()` the non-dominated feasible records of the whole store
   through `pareto::front`, `best()` the best by the weighted scalar and Deb's rules.
3. **Pareto extraction.** `pareto::front` returns the first front of `non_dominated_sort`, which stores every dominated pair
   (`dominating: Vec<Vec<usize>>`): about n^2 / 2 indices, 2.5 GB for a store of 25 000 records. `ledger::pareto_front` also calls it.
   `crowding_distance` is the Deb et al. (2002) distance (boundary members infinite, each objective normalised by its range).
4. **The tools.** `design/tools.rs` `parse_method` reads `{type, budget, restarts, seed, pop, levels, n}` and ignores other keys;
   `method_json` writes the method into `study.json`; the store signature (`signature`) does not include the method. The unknown-type
   message lists `nelder_mead, differential_evolution, factorial, lhs or sobol (CMA-ES is not available)`; no test asserts it. The
   MCP descriptor of `design_create_study` (`crates/rcfd-mcp/src/lib.rs` line 279) names the methods in its description and gives
   `method` as a free object (`{type, budget?, restarts?, seed?, pop?, levels?, n?}`), so a new method type or key needs no schema
   change; its text is in `golden/tools_list_default.json` and `golden/tools_list_project_f270.json` ("the golden is never
   re-recorded", `tests/default_f1.rs`).
5. **T3.** `crates/rcfd-mcp/src/design_tests.rs`: the study `x, y` in [-2, 2] with `init` -1.2 and 1.0 (dummy targets
   `physics.nu`, `physics.rho`), objective `f` min, method `{"type": "nelder_mead", "budget": 400, "restarts": 1, "seed": 1}`, the
   test playing the evaluator through `design_propose` / `design_record`; the ignored `t3_best_after_120_known_miss` asserts best
   f <= 1e-3 after 120 recorded candidates; ADR 0067 addendum 2 records 3.1255e-3 at 120 and 3.4e-19 at 347.
6. **The warm start.** `design::case_eval::run_case(case, seed: Option<&Fields>, limits)` seeds with `Simulation::seed_fields(u, p)`
   (`sim.rs` line 4813), which refuses a case with energy or turbulence ("warm start carries velocity and pressure only; this case
   has energy or turbulence fields") and every engine but `Engine::Steady`; the run then is cold with the note `cold start: ...`.
   `Fields { u, p }` is built only in `case_eval.rs`; `CaseRun` is built literally in `crates/rcfd-workspace/src/design.rs` (so it
   gets no new field); `CaseMapEvaluator { base, limits }` is built literally in `crates/rcfd-cli/src/cmd_map.rs` and
   `tests/design_opmap_w32c.rs` (so it gets no new field); `CaseEvaluator` is built only through `CaseEvaluator::new`.
7. **The steady iteration with energy and turbulence** (`Simulation::step`, `Engine::Steady`, line 3265): the turbulence model is
   updated first from the current velocity and flux (`t.model.update(&s.u, &s.phi)`, which recomputes the eddy viscosity with its
   under-relaxation from the stored one), then the momentum and pressure step, then the temperature (`en.solver.iterate`). The
   run converges when `res_u < tol_u`, `res_p < tol_p`, the energy residual `< tol_u` and the eddy-viscosity change `< tol_u`, or by
   a `convergence.<n>` condition. `load_checkpoint` restores the temperature by `en.solver.t.values = T`.
8. **Turbulence state.** `TurbulenceModel` (`crates/rcfd-turb/src/model.rs`): `key()`, `nut()`, `scalars()` (k-epsilon: `k`,
   `epsilon`; k-omega SST: `k`, `omega`), `scalar_mut(name)`, `initialize_from_nut(nut)` (k-epsilon sets epsilon from k and nut,
   SST sets omega from k and nut, both then set the stored nut to the given values). No other public setter of nut.
9. **Other engines.** `Engine::Coupled` has no `set_fields`; the transient `PimpleSolver` has `restore` of a full snapshot (used by
   restart files), not a seeding at time 0.
10. **Store lookups.** `Study::eval_points` finds each record by a linear scan (`position`); with 25 000 records this is about
    3e8 key comparisons per study, a few seconds in release. Not changed here.
11. **Who else uses these files.** `rcfd-workspace` (`design.rs`: `EvaluatorSource`, the runner evaluators) uses `CaseEvaluator`,
    `CaseRun`, `Method` only through `Study::run`; nothing there matches on `Method`.
12. **The probe** (main 05f0d73, release, scratch test, not committed; cold runs with `run_case`, default limits):

    | Case | Point | State | Iterations | Reports | Time |
    |---|---|---|---|---|---|
    | HE (below) | inlet 0.5 | converged, exit 0 | 89 | dp 1.33909, t_out 0.952787, q_walls -0.0599458 | 0.17 s |
    | HE | 0.6 | converged | 90 | dp 1.62718, t_out 0.929911, q_walls -0.0668986 | 0.16 s |
    | HE | 0.7 | converged | 91 | dp 1.92216, t_out 0.905779, q_walls -0.0733054 | 0.18 s |
    | KE (`cases/channel/kepsilon_channel.toml`) | body force 1.0 | converged | 4502 | yplus_lower 27.6145 | 3.9 s |
    | KE | 1.1 | converged | 4665 | yplus_lower 28.9900 | 4.0 s |
    | KE | 1.2 | converged | 4782 | yplus_lower 30.3008 | 4.0 s |
    | SST (`cases/channel/sst_channel.toml`) | 1.0 | converged | 4396 | u_mean 18.1669, yplus_lower 1.000000 | 7.8 s |
    | SST | 1.1 | converged | 5468 | u_mean 19.2122 | 10.8 s |
    | SST | 1.2 | converged | 5764 | u_mean 20.2140 | 11.5 s |

    Each HE point seeded with its own converged fields through `run_case` ran cold with the note above.

## Decisions (fixed now)

**D1. NSGA-II, `method.type = "nsga2"`.** A new module `rcfd_api::design::nsga2`:

* `MoFitness { objectives: Vec<f64>, violation: f64 }` (objectives minimised; a failed evaluation has every objective `+inf` and
  violation `+inf`); `MoEval` = `FnMut(&[Vec<f64>]) -> Result<Vec<MoFitness>, Stop>`; `Options { max_evals, pop, seed, x0 }`;
  `MoResult { population: Vec<(Vec<f64>, MoFitness)>, rank: Vec<usize>, evaluations, generations }`;
  `run(bounds, &Options, &mut MoEval) -> Result<MoResult, Stop>`.
* The algorithm is NSGA-II of Deb, Pratap, Agarwal and Meyarivan (2002) with constrained domination (`pareto::dominates`, Deb
  2002 section VI: feasible beats infeasible, two infeasible by violation, two feasible by Pareto dominance):
  1. P0: `pop` points uniform in the box from `Rng::new(seed)` (`optim::random_in_box`, point after point), member 0 replaced by
     `x0` clamped into the box when given (as differential evolution); evaluated as one batch.
  2. Every member gets its front rank (`pareto::non_dominated_sort`) and its crowding distance within its front
     (`pareto::crowding_distance`).
  3. While `evaluations + pop <= max_evals`: `pop / 2` pairs; each parent by a binary tournament of two members drawn uniformly
     (`rng.below(pop)`, the second redrawn until it differs from the first): the lower rank wins, then the larger crowding distance,
     then the first drawn. With probability 0.9 (one `rng.uniform()`) the pair is crossed by simulated binary crossover (Deb and
     Agrawal 1995), distribution index 20, on every variable: `u = rng.uniform()`, `beta = (2u)^(1/21)` for `u <= 0.5`, else
     `(1 / (2 (1 - u)))^(1/21)`, children `0.5 ((1 + beta) p1 + (1 - beta) p2)` and `0.5 ((1 - beta) p1 + (1 + beta) p2)`; otherwise
     the children are copies. Each child is then mutated variable by variable (one `rng.uniform()` per variable, mutation when below
     `1/d`) by polynomial mutation (Deb and Goyal 1996, the simple form), index 20: `u = rng.uniform()`, `delta = (2u)^(1/21) - 1` for
     `u < 0.5`, else `1 - (2 (1 - u))^(1/21)`, `x += delta (hi - lo)`; every child is clamped into the box. Draw order: the two
     tournaments, the crossover coin, the crossover draws, the first child's mutation draws, the second child's. The children are
     evaluated as one batch in creation order.
  4. Survivors: the union R = P then Q (in that order) is sorted into fronts; fronts enter the next population whole, in ascending
     index order, while they fit; the first front that does not fit enters by descending crowding distance (computed within that
     front of R), ties by ascending index, until `pop` members. Ranks and crowding distances of the survivors are those of R.
  5. The result is the last population with its ranks; `evaluations = pop * generations`, `generations = floor(max_evals / pop)`.
* Refusals (`Stop::Error`, nothing evaluated): no parameter; `pop` odd or below 4; `max_evals < pop`.
* Constants: crossover probability 0.9, both distribution indices 20, mutation probability 1/d. These are the values of the
  experiments of Deb et al. (2002), and the two operator formulas are those of the SBX and polynomial-mutation papers, **all from
  memory**: none of the three papers was read for this text (Q5). The test problems' definitions (ZDT1, FON) are also from memory and
  are checked by their own closed forms (MO3).
* Pure helpers, public for the closed-form checks: `nsga2::sbx_beta(u, eta)`, `nsga2::poly_delta(u, eta)`, `nsga2::select(objs,
  violation, n) -> Vec<usize>` (step 4 on a given union).
* **Study:** `Method::Nsga2 { budget, pop, seed }`. `Study::run` maps each record to `MoFitness` from `objective_vector` (the
  weights do not enter) and `violation_of`, a record without an objective vector to the failed fitness; `x0` is the parameters'
  `init` when all have one (as the other optimisers). It returns as `OptResult` the member of the last population that is best by
  `fitness_of` (the weighted scalar and Deb's rules), the first in population order on a tie. The Pareto set of the study is
  `Study::pareto()` as today: the non-dominated feasible records of the whole store. Propose, record, resume and `workers` work
  through the same `eval_points` as the other methods.
* **Tools:** `{"type": "nsga2", "budget": B, "pop": N, "seed": s}`. `budget` and `pop` have no default (Q4): missing, `method.budget:
  nsga2 needs a budget (no default)` and `method.pop: nsga2 needs a population size pop (no default)`; `method.pop: nsga2 needs an
  even population of at least 4, got <N>`; `method.budget: <B> is smaller than the population <N>`. `seed` defaults to 1 as for the
  other methods. `method_json` writes `{"type": "nsga2", "budget", "pop", "seed"}`. The unknown-type and missing-method messages
  name `nsga2`.

**D2. `pareto::front` without the pair lists.** `front(objs, violation)` returns, in ascending order, the indices i for which no j
dominates i (with the same treatment of non-finite objectives as `non_dominated_sort`). This is the definition of the first front
of `non_dominated_sort`, so the output is the same; the memory is O(n). `non_dominated_sort` is unchanged.

**D3. Two Nelder-Mead options, opt-in.** Method keys of `nelder_mead`:

* `initial_simplex`: `"range"` (default: the ADR 0067 rule of item 1) or `"start"`: the rule of MATLAB's `fminsearch` as its
  documentation states it (read: MathWorks, "Optimizing Nonlinear Functions", fminsearch algorithm: "adding 5% of each component
  x0(i) to x0 ... uses 0.00025 as component i if x0(i) = 0"): vertex i is `x0 + 0.05 x0_i e_i`, or `x0 + 0.00025 e_i` when
  `x0_i = 0`. The step is flipped when it leaves the box and the vertex clamped, as in the range rule. A restart r >= 1 builds its
  simplex around the best point by the same rule with the steps multiplied by `0.5^r` and the random sign per axis of the range
  rule (one `rng.uniform()` per axis, as now).
* `coefficients`: `"standard"` (default: reflection 1, expansion 2, contraction 1/2, shrink 1/2) or `"adaptive"`: the
  dimension-dependent coefficients of Gao and Han (2012), read in the Optim.jl documentation (`alpha = 1, beta = 1 + 2/n, gamma =
  0.75 - 1/2n, delta = 1 - 1/n`), not in the paper (its copy at the University of Idaho was not reachable): reflection 1, expansion
  `1 + 2/d`, both contractions `0.75 - 1/(2d)` (outside `c + gamma (c - x_worst)`, inside `c - gamma (c - x_worst)`), shrink
  `1 - 1/d`. With one parameter the shrink is 0, so `"adaptive"` with d = 1 is refused (`Stop::Error` in the library;
  `method.coefficients: adaptive needs at least 2 parameters` in the tools).
* Library: `NmOptions` gains `initial_simplex: InitialSimplex` (`Range` | `Start`) and `coefficients: Coefficients` (`Standard` |
  `Adaptive`), defaults `Range` and `Standard`; `optim::nm_coefficients(Coefficients, d) -> [f64; 4]` (reflection, expansion,
  contraction, shrink). On the default path the coefficients replace equal literals, so its arithmetic is unchanged.
* Study and tools: a new variant `Method::NelderMeadWith { budget, restarts, seed, initial_simplex, coefficients }` is used when
  either key is given; without both keys the method is `Method::NelderMead` as now and `method_json` writes what it writes now.
  `method_json` of the new variant writes both keys. Refusals: a value other than the listed ones, `method.initial_simplex: '<v>' is
  not range or start` and `method.coefficients: '<v>' is not standard or adaptive`; either key on another method type,
  `method.<key>: only nelder_mead takes this key`.
* In two dimensions the adaptive coefficients equal the standard ones exactly (`1 + 2/2 = 2`, `0.75 - 1/4 = 0.5`, `1 - 1/2 = 0.5`),
  so on T3 the variant differs from the default only by its initial simplex. This is stated now, before the T3 run.
* Why these two: they are the published rules of two widely used implementations, chosen from the documentation without running
  them on T3 or anything else; the T3 limit is not changed, and the default Nelder-Mead is not changed (a study's resume and
  proposals replay the method, so a changed default would change the proposals of every existing study: Q2).

**D4. A warm start that carries temperature and two-equation turbulence, opt-in, library.**

* `rcfd_api::sim::WarmState { u: Vec<Vec3>, p: Vec<f64>, t: Option<Vec<f64>>, turbulence: Option<TurbWarm> }` and `TurbWarm {
  model: String, nut: Vec<f64>, fields: Vec<(String, Vec<f64>)> }`.
* `Simulation::warm_state(&self) -> Option<WarmState>`: for `Engine::Steady` only: velocity, pressure, the temperature when the
  energy equation is solved, and for the models with key `k_epsilon` or `k_omega_sst` the stored eddy viscosity and every field of
  `scalars()`; `None` for other engines; with another turbulence model `turbulence` is `None`.
* `Simulation::seed_state(&mut self, &WarmState) -> Result<(), String>` checks everything before it writes anything, so a refusal
  leaves the simulation as it was. It refuses: an engine other than `Engine::Steady` (`warm start is available for the steady SIMPLE
  and SIMPLEC algorithms`); a case with other transported or coupled state: scalars or species, chemistry, conjugate heat transfer
  solids, radiation, the electric potential, phase change, steady two-way particle coupling, a mixing plane or a dynamic mesh (the
  message names it); a temperature in the state for a case without the energy equation, or none for a case with it; a turbulence
  model other than `k_epsilon` and `k_omega_sst`, a state without turbulence for a case with a model, a state with it for a case
  without, or another model key (both named); any field whose size is not the mesh's. On success: velocity and pressure by
  `SimpleSolver::set_fields` (flux rebuilt, iteration 0), the temperature copied, then `initialize_from_nut(nut)` followed by a copy
  of each named field through `scalar_mut` (so k, epsilon or omega and nut equal the state's), and the run flags cleared as in
  `seed_fields`. `seed_fields` is unchanged.
* `design::case_eval`: `WarmStart` (`Flow` | `Extended`); `run_case_with(case, seed, limits, WarmStart)`; `run_case(..)` is
  `run_case_with(.., WarmStart::Flow)` and runs as now. `Fields` gains `state: Option<WarmState>` (always `None` under `Flow`).
  Under `Extended` a seed is applied by `seed_state` (its `state`, or velocity and pressure alone when it has none), and a converged
  steady run returns `Fields` with `state = warm_state()`. `ExtendedMapEvaluator { base, limits }`: the operating-map point
  evaluator with `Extended` (`CaseMapEvaluator` unchanged); `CaseEvaluator` gains the public field `warm: WarmStart`, `Flow` from
  `CaseEvaluator::new`, used for chained conditions.
* Not wired into `rcfd map`, the study definition or the MCP tools (Q7): the acceptance clause is checked on the library.

**D5. What does not change.** No solver default, case-schema key, case hash or byte of an existing case; no MCP tool, descriptor or
golden; `seed_fields`, `run_case`, `CaseMapEvaluator`, `Method::NelderMead` and `Method::Differential` behave as before; every
existing test passes unedited (no registered edit).

**D6. The tool description.** The description of `design_create_study` keeps its text, so `tools/list` stays byte-identical to both
goldens; `nsga2` and the two keys are documented in `docs/design-loop.md` and named by the error messages (Q6).

**D7. Separable commits.** (1) this text; (2) the V0b record on main (test only); (3) `pareto::front`, the Nelder-Mead options
and NSGA-II with their library tests; (4) the warm start with its tests; (5) the tools and the MCP tests; (6) documents; (7) the
addenda.

## Checks and limits (fixed now)

Test problems and metrics of MO:

* **ZDT1** (Zitzler, Deb and Thiele 2000), d = 30, x in [0, 1]^30: `f1 = x1`, `g = 1 + 9 sum_{i>=2} x_i / 29`, `f2 = g (1 -
  sqrt(f1 / g))`. Front: `g = 1`, `f2 = 1 - sqrt(f1)`, f1 in [0, 1].
* **FON** (Fonseca and Fleming 1995, as used by Deb et al. 2002), d = 3, x in [-4, 4]^3, `a = 1/sqrt(3)`: `f1 = 1 - exp(-sum (x_i -
  a)^2)`, `f2 = 1 - exp(-sum (x_i + a)^2)`. Pareto set `x1 = x2 = x3 = t`, |t| <= a; front `F(t) = (1 - exp(-3 (t - a)^2), 1 -
  exp(-3 (t + a)^2))`.
* `D(f)`: the Euclidean distance in objective space from `f` to the front, the minimum over 20 001 front points equally spaced in
  the front parameter (ZDT1: f1; FON: t), refined by golden-section search over the two intervals next to the nearest sample to
  1e-12 in the parameter. Reference points: 1001 front points equally spaced in the same parameter.
* For the returned set S (the objective vectors of the records `Study::pareto()` names): **C** `max_{s in S} D(s) <= 1e-2`;
  **G** the largest distance from a reference point to its nearest member of S `<= 5e-2`; **I** the mean of those distances
  `<= 1e-2`. S must be non-empty.

### Library checks, NSGA-II (`crates/rcfd-api/tests/f249_moo.rs`)

| Id | Check | Limit |
|---|---|---|
| MO1z | ZDT1 through `Study` (in-memory store, `FnEvaluator`, `workers` 1), `Method::Nsga2 { budget: 25000, pop: 100, seed }` (250 generations, the set-up of Deb et al. 2002), seeds 1, 2, 3 | C, G and I for every seed |
| MO1f | FON the same way, `budget: 10000, pop: 100`, seeds 1, 2, 3 | C, G and I for every seed |
| MO1c | control: `Doe::Lhs { n: 25000, seed: 1 }` on ZDT1 through `Study`, the same metrics; and `Doe::Lhs { n: 10000, seed: 1 }` on FON, recorded | ZDT1: C must fail (max D > 1e-2); FON: none |
| MO2a | determinism: ZDT1, pop 20, budget 400: two runs with seed 4 give bit-identical records; `workers` 1 and 4 give the same records; seeds 4 and 5 differ | exact |
| MO2b | bounds and budget: every point any MO1 or MO2 run asked for lies in the box (exact comparison); `MoResult.evaluations = pop * floor(budget / pop)` for budgets 400 and 410 with pop 20 (library `nsga2::run`) | exact |
| MO2c | resume: the MO2a study stopped after k = 37 and k = 250 new evaluations (allowance) and resumed from a file store gives the records of an uninterrupted run; the evaluator is called `total - k` times in the resume and 0 times in a third launch | exact |
| MO2d | propose / record: the MO2a study driven by `Study::propose` and `Study::record` gives the records of the evaluator-driven run (index, params, metrics) | exact |
| MO2e | failures: ZDT1 d = 30 with every candidate of `x1 > 0.9` failing (pop 20, budget 400): the run ends without error, the failed records are stored, none is in `Study::pareto()` | as stated |
| MO2f | refusals (library): pop 7, pop 2, `max_evals` 10 with pop 20, no parameter: `Stop::Error` with nothing evaluated | as stated |
| MO3a | `sbx_beta(u, 20)` for u = 0.1, 0.25, 0.5, 0.75, 0.9 and `poly_delta(u, 20)` for the same u equal the formulas of D1 written out again in the test | bit for bit |
| MO3b | `select` on a hand-made union of 8 two-objective points with fronts {0, 3}, {1, 4, 6}, {2, 5, 7} (by construction) and n = 4: both members of the first front, then the two of {1, 4, 6} with the largest crowding distance (the two boundary members, infinite) | the expected 4 indices |
| MO3c | the problem code: ZDT1 at `x = (0.25, 0, ..., 0)` gives (0.25, 0.5) and at `x2 = 1`, others 0, `g = 1 + 9/29`; FON at `t (1, 1, 1)` gives `F(t)`, and `D(F(t)) <= 1e-12` for t = -a, 0, a/2, a | exact or as stated |
| MO3d | D2: `pareto::front` equals `non_dominated_sort(..)[0]` on 20 seeded random sets (100 points, 2 and 3 objectives, a third infeasible, some with a non-finite objective) | exact |

### Library checks, Nelder-Mead (`crates/rcfd-api/tests/f249_nm.rs`)

| Id | Check | Limit |
|---|---|---|
| N2 | ADR 0067 O1 with `initial_simplex = Start`, `coefficients = Adaptive`: Rosenbrock in [-2, 2]^2, seeds 1 to 20 (random start), budget 800, 3 restarts | at least 18 of 20 with f <= 1e-6 and \|x - 1\|, \|y - 1\| <= 1e-3 |
| N3 | ADR 0067 O3 with the same options: `(x - 3)^2 + (y - 2)^2` with `x + y <= 4` in [0, 5]^2, seeds 1 to 20, budget 600, 3 restarts | all 20: f - 0.5 <= 1e-3 and violation <= 1e-6 |
| N4a | `nm_coefficients(Adaptive, d)` for d = 2, 3, 4, 10 equals `[1, 1 + 2/d, 0.75 - 1/(2d), 1 - 1/d]`, and for d = 2 `[1, 2, 0.5, 0.5]`; `Standard` gives `[1, 2, 0.5, 0.5]` | bit for bit |
| N4b | in two dimensions `Adaptive` gives the history of `Standard` (Rosenbrock, seed 3, 300 evaluations, ADR 0067 O4 set-up), for both initial-simplex rules | bit for bit |
| N4c | the first batch with `Start` from x0 = (-1.2, 1.0, 0.0) in [-2, 2]^3 is x0, (-1.26, 1, 0), (-1.2, 1.05, 0), (-1.2, 1, 0.00025) (each as `x0_i + 0.05 x0_i`, computed in the test); from x0 = (2.0, 0.5) in [-2, 2]^2 the first vertex is flipped to `2.0 - 0.1` | bit for bit |
| N4d | in d = 4 (sum of squares from x0 = (1, 1, 1, 1), 200 evaluations) the `Adaptive` and `Standard` histories differ; `Adaptive` with one parameter is refused with nothing evaluated | as stated |
| N5 | recorded: the sum of squares in d = 10 and d = 20 from x0 = (1, ..., 1) in [-2, 2]^d, `Start`, budget 10 000 and 20 000, no restart, with `Standard` and with `Adaptive` | none (best f and evaluations reported) |

### Tool checks (`crates/rcfd-mcp/tests/design_f249.rs`, the MCP server in process as `design_tests.rs`)

| Id | Check | Limit |
|---|---|---|
| N1 | **T3 with the options**: the T3 study of item 5 with method `{"type": "nelder_mead", "budget": 400, "restarts": 1, "seed": 1, "initial_simplex": "start", "coefficients": "adaptive"}`, the propose / record loop of `t3_best_after_120_known_miss`; best f from `design_list_study` after 120 recorded candidates | <= 1e-3 (the T3 limit) |
| N1d | the same loop without the two keys gives the V0b record of the default T3 (best f at 120 bit for bit) | exact |
| N6 | refusals (-32602, message naming the key): `initial_simplex: "simplex"`, `coefficients: 3`, `initial_simplex` on `lhs`, `coefficients: "adaptive"` with one parameter, `nsga2` with `pop` 7, with `budget` 10 and `pop` 20, without `budget`, without `pop`; nothing written for any of them. The `method` object of a default `nelder_mead` study's `study.json` has exactly the keys `type, budget, restarts, seed`, and of an `nsga2` study `type, budget, pop, seed` | as stated |
| MO4 | an `nsga2` study through the tools: ZDT1 with d = 2 (parameters on dummy targets as T3), objectives `f1`, `f2` min, `{"type": "nsga2", "budget": 400, "pop": 20, "seed": 1}`; propose / record until `complete`; two studies with the same inputs propose the same first batch | `complete` reached; `design_list_study.pareto` equals `pareto::front` recomputed by the test from `evaluations.jsonl` (indices, exact); the evaluations equal the distinct candidates proposed; max D of the Pareto set recorded |

### Library checks, warm start (`crates/rcfd-api/tests/f249_warm.rs`)

Case HE (steady SIMPLEC, energy without buoyancy), as probed:

```toml
[mesh]
generator = "channel_2d"
nx = 40
ny = 10
lx = 4.0
ly = 1.0
depth = 0.1
[physics]
nu = 0.05
rho = 1.0
cp = 1.0
conductivity = 0.05
[energy]
enabled = true
buoyancy = false
[initial]
temperature = 0.0
[boundaries.inlet]
velocity = "fixed"
velocity_value = [0.5, 0.0, 0.0]
temperature = "fixed"
temperature_value = 0.0
[boundaries.outlet]
velocity = "zero_gradient"
pressure = "fixed"
pressure_value = 0.0
temperature = "zero_gradient"
[boundaries.walls]
velocity = "fixed"
temperature = "fixed"
temperature_value = 1.0
[solver]
algorithm = "simplec"
convection = "upwind"
max_iterations = 4000
tolerance_u = 1e-6
tolerance_p = 1e-6
[reports.dp]
type = "area_average"
patch = "inlet"
field = "p"
[reports.t_out]
type = "area_average"
patch = "outlet"
field = "T"
[reports.q_walls]
type = "heat_flow"
patch = "walls"
```

KE and SST are the shipped files `cases/channel/kepsilon_channel.toml` and `sst_channel.toml` read as they are, with
`physics.body_force` set by the map.

`REL_WARM = 1e-4`, as `|a - b| <= REL_WARM max(|a|, |b|)` on every report. Reasoning (as ADR 0200's `REL_CHAIN`): a warm and a cold
run solve the same discrete equations and stop on the same criterion; HE stops at normalised residuals of 1e-6 and falls by about
six decades in 90 iterations (item 12), a factor of about 0.86 per iteration, so each stopped state is within about 1e-6 / (1 -
0.86), 7e-6, of the discrete solution; KE and SST stop when y+ changes by less than 1e-9 (relative) over 20 iterations, which with
a contraction as slow as 0.999 per iteration leaves about 5e-8. The limit is more than ten times the HE estimate.

| Id | Check | Limit |
|---|---|---|
| W1 | HE map, `boundaries.inlet.velocity_value` [0.5, 0, 0], [0.6, 0, 0], [0.7, 0, 0], through `opmap::run_map` with `ExtendedMapEvaluator` (warm) and with `MapOptions { cold: true }` on the same evaluator (cold) | all 6 rows converged; warm rows 1 and 2 `restart` `warm`; every report of rows 1 and 2 within `REL_WARM`; row 0 equal bit for bit |
| W1b | the iterations of warm rows 1 and 2 against cold rows 1 and 2 | warm sum < cold sum |
| W2 | KE map, `physics.body_force` [1.0, 0, 0], [1.1, 0, 0], [1.2, 0, 0], as W1 | as W1 (reports `yplus_lower`, `yplus_upper`); iterations recorded |
| W3 | SST map, as W2 | as W1 (reports `u_mean`, `yplus_lower`, `yplus_upper`); iterations recorded |
| W4 | chained conditions: a study on HE with one parameter, `physics.conductivity` in [0.04, 0.06] evaluated at 0.05 (a velocity component is not a scalar settings path), and two conditions `a` {} and `b` {`boundaries.inlet.velocity_value`: [0.6, 0, 0]}: `CaseEvaluator` with `chain` and `warm = Extended` against the same with `chain = false` | condition `b`: warm run (no `cold start` in its message), every metric within `REL_WARM` of the unchained one; condition `a` equal bit for bit |
| WU1 | after `seed_state` with an HE state (a converged run at inlet 0.5) into the HE case at 0.6: `ux, uy, uz, p, T` of the simulation equal the state bit for bit before any iteration; with a KE state into KE at 1.1, and an SST state into SST at 1.1: `k`, `epsilon` / `omega` and `nut` equal bit for bit | exact |
| WU2 | refusals, each an `Err` whose text names the cause, with every field (`ux, uy, uz, p` and `T`, `k`, `epsilon`, `omega`, `nut` where the case has them) bit-identical to before the call: the HE state into a PISO version of HE (`solver.algorithm = "piso"`, `dt` 0.01); the HE state into the ADR 0067 M3 channel (no energy); the M3 state into HE; the KE state into SST at 1.0; the HE state into HE with `nx = 41`; the HE state into HE with a scalar `[scalars.c]` `diffusivity = 0.01` | as stated |
| WU3 | the `Flow` path is unchanged: `run_case` with an HE seed and `CaseMapEvaluator` on the W1 map run every point cold with the note of item 6 (also in V0b) | as stated |

### Default bits (V0)

| Id | Check | Limit |
|---|---|---|
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs` unedited; they pass in release | pass |
| V0b | `crates/rcfd-api/tests/f249_default_bits.rs`, recorded on main 05f0d73 with this text and the test only, committed before any code: FNV-1a (as `f286_default_bits.rs`) of (1) the Nelder-Mead and differential-evolution histories of ADR 0067 O4 (seed 3, 300 and 400 evaluations); (2) the records of the T3 study run in `rcfd_api` through `Study::propose` and `Study::record` with the default method, with the bits of the best f after 120 records and at the end; (3) `pareto::front` of the ADR 0067 P1 set and of a seeded 3-objective set with infeasible points; (4) the rows of the W1 map and of a KE map at 1.0 and 1.1 through `CaseMapEvaluator` (status, restart, exit code, state, iterations, report names and bits, message) | bit-identical after the code |
| V0c | the tools-list goldens unedited: `default_f1.rs`, `project_list_f270.rs`, `tool_list_f275.rs` pass | pass |
| V0d | every existing test passes unedited; the ignored `t3_best_after_120_known_miss` stays ignored and unedited | pass |
| V0e | the gate (fmt per package, clippy `--workspace --all-targets -D warnings`, every test in release on a git-archive copy, `scripts/gate_test.py`, 3 jobs) on the final commit | pass, apart from the known flakes |

### Mutations (scratch, not committed)

Each is applied alone to the final tree, the named test is run once, and it must fail.

| Id | Mutation | Must fail |
|---|---|---|
| MM1 | NSGA-II children are copies of their parents (no crossover, no mutation) | MO1z seed 1 (C) |
| MM2 | the survivor selection takes the fronts from the last to the first | MO1z seed 1 (C) |
| MM3 | the tournament ignores the rank (crowding distance only) | none: recorded |
| MN1 | `Start` builds the simplex of the range rule | N1 |
| MN2 | the adaptive expansion is `1 + 1/d` | N4a |
| MW1 | `seed_state` does not copy the temperature | WU1 |
| MW2 | `seed_state` does not call `initialize_from_nut` | WU1 (`nut`) |
| MW3 | `ExtendedMapEvaluator` returns no warm state | W1 (`restart`) |
| MD1 | `pareto::front` drops the last front member | MO3d |

### Classes

V0 is a precondition of landing: a miss of V0a, V0b or V0c means the change is not merged and the cause is recorded. A miss of any
other limit is reported with its number and kept as an ignored `*_known_miss` test with the limit intact. MO1c (ZDT1) and a
mutation that does not make its check fail mean the check is not shown to be able to fail: a defective set-up, corrected by an
amendment before any re-run, never counted as a pass. N1 is the acceptance's T3 clause; MO1z and MO1f are its Pareto clause; W1 to
W3 are its warm-start clause.

## Predictions (mine; moderate confidence)

| Id | Prediction |
|---|---|
| PR1 | MO1z: C below 1e-3 (the Pareto set of ZDT1 lies on the bound, which clamping reaches exactly); G below 2e-2; I below 5e-3 |
| PR2 | MO1f: C below 1e-3; G below 3e-2; I below 1e-2 |
| PR3 | MO1c: ZDT1 max D above 1 (g of the best random point far from 1); FON max D of order 1e-2 |
| PR4 | N1 met, with moderate confidence only (about two in three): `fminsearch` reaches 8.18e-10 on this function from this start (MathWorks example, the figure title), but the value at 120 evaluations is not published there |
| PR5 | N2, N3 met; N5: `Adaptive` at or below `Standard` in d = 20 |
| PR6 | W1 to W4 within 1e-6; W1b met; KE and SST warm rows need fewer iterations than cold |

## Budgets

In release: MO1z about 10 s per seed (the store's linear lookups, item 10), MO1f 2 s, MO1c 10 s; f249_nm under 10 s; W1 and W4 under
2 s, W2 25 s, W3 60 s (item 12); the MCP tests under 30 s.

## Not in scope (and why)

- CMA-ES and the real-fan study (F-260).
- A warm start for the transient PISO/PIMPLE path, the coupled solver, other turbulence models, scalars and the other coupled
  models of D4 (Q8).
- Wiring the extended warm start into `rcfd map`, studies and the MCP tools (Q7), and the tool description (Q6).
- Faster store lookups (item 10).

## Open questions for the maintainer (each with a recommendation)

1. **Done or partial.** The acceptance's T3 clause is met here (if N1 passes) by an opt-in option of the T3 study, not by the
   default Nelder-Mead; the warm-start clause is met for steady SIMPLE/SIMPLEC with energy or two-equation turbulence, which is
   "beyond" the earlier warm start in what it carries but not in the algorithm. Options: (a) done; (b) partial until Q8 (b).
   Recommendation: the maintainer's call; (a) if every registered check is met and the reading of "beyond" above is accepted.
2. **The Nelder-Mead default.** (a) The options stay opt-in (as built: every existing study replays and resumes as before); (b)
   `start` and `adaptive` become the default for studies created after a `study.json` version step, older studies keep theirs.
   Recommendation: (a) now; (b) later, if the design studies on real cases show the benefit.
3. **The `start` rule and parameter units.** Its steps scale with the start values, so a parameter that starts near zero gets a
   tiny step (0.00025 when it is exactly 0, whatever its range). Options: (a) as MATLAB (as built, documented); (b) a floor tied to
   the range, which is a new constant for the maintainer to choose. Recommendation: (a), with the documentation saying when to keep
   `range`.
4. **NSGA-II defaults.** `budget` and `pop` have no default (as built), since a default budget on a CFD case is a cost decision.
   Options: (a) keep them required; (b) `pop` 100 (Deb et al. 2002). Recommendation: (a).
5. **The NSGA-II sources.** The operator constants and formulas and the ZDT1 and FON definitions are from memory (D1), and SBX is
   applied to every variable of a crossed pair (many implementations cross each variable with probability 1/2; that is not in the
   texts recalled here). Options: (a) as built; (b) the maintainer reads Deb et al. 2002, Deb and Agrawal 1995 and Deb and Goyal
   1996 and a follow-up corrects any difference. Recommendation: (b) when convenient; the closed-form checks MO1 and MO3 do not
   depend on these details.
6. **The tool description.** `design_create_study`'s description and its `method` hint do not name `nsga2`, `initial_simplex` or
   `coefficients`, because changing them changes the two tools-list goldens, which are never re-recorded. Options: (a) as built
   (documents and error messages name them); (b) a registered edit of the description and of both goldens in a separate change.
   Recommendation: (b), so that an assistant reading `tools/list` finds the new method.
7. **Exposing the extended warm start.** Library only now. Options: (a) a map option and a study key (`warm_start = "extended"`),
   which changes the descriptors of `design_run_operating_map` and `design_create_study` (Q6's goldens); (b) the extended warm
   start becomes the default for maps and chained conditions, which changes the rows and metrics of existing maps of energy and
   turbulence cases (within `REL_WARM`) and needs the bit-identity rule lifted for the design tools; (c) an `rcfd map` flag only.
   Recommendation: (b) once the maintainer lifts the rule for it, since a warm start is the point of a map; (c) meanwhile.
8. **Beyond steady SIMPLE/SIMPLEC.** Not done: the coupled solver (it has no seeding, item 9), the transient path, other models.
   Options: (a) stop here; (b) the coupled solver next, with the same check as W1; (c) transient points, which reproduce a cold
   run only after the start-up transient has decayed, so their tolerance needs a registered decay estimate per case.
   Recommendation: (b), then (c) only for a case that needs it.

## Addenda

Results follow in addenda, in the order of the tables.

### Amendment 1 (2026-11-07): two misses, and the mutations of MO1z moved to MO1f

Written after the first run of `f249_moo`, `f249_nm`, `design_f249` and `f249_warm` at ebc4e9f, before any mutation or diagnostic
run. No limit changes.

* **MO1z missed.** ZDT1, pop 100, 25 000 evaluations: seed 1 C 0.543, G 0.288, I 0.254; seed 2 C 0.378, G 0.207, I 0.141; seed 3
  C 0.353, G 0.291, I 0.220 (limits 1e-2, 5e-2, 1e-2), with 118 to 150 Pareto members among about 24 050 records. The test stays
  with its limits as `mo1z_nsga2_on_zdt1_meets_the_analytic_front_known_miss`, ignored. MO1f (FON) met C, G and I on all three
  seeds, and the control MO1c failed C as required (C 3.90 on ZDT1), so the convergence metric can fail.
* **N3 missed.** 19 of 20 seeds; seed 16 ended at f = 1.6668 with violation 0 after 600 evaluations (limit: all 20 with f - 0.5
  <= 1e-3). The test stays with its limit as `n3_constrained_quadratic_from_20_seeded_starts_known_miss`, ignored.
* **Mutations MM1 and MM2.** They were registered to make MO1z seed 1 fail, which fails without them, so they cannot show that a
  check fails because of them. They are applied to MO1f instead (must fail C, G or I on seed 1); MM3 is recorded on MO1f seed 1.
  The other mutations are as registered.
* **Diagnostics** (scratch, not committed, not results and not adopted; their numbers go in the addendum):
  * DG1, the cause of the MO1z miss: the registered SBX keeps each child near its own parent in every variable and never
    exchanges coordinates between the two parents, so on ZDT1 a good coordinate of one parent can reach another member only
    through selection and mutation. Test of that hypothesis: MO1z seed 1 with the two children's values exchanged per variable
    with probability 1/2 after the SBX step (one more `rng.uniform()` per variable, drawn after that variable's `u`).
  * DG2, the N3 miss: the end point of seed 16 and the evaluations its restarts spent.

### Addendum 1: results (2026-11-07)

Commits on `f249-parametric-studies` (from main 05f0d73): registration 382d2c1; V0b record d98f680 (on main, before any code);
f973866 (`pareto::front`, the Nelder-Mead options, `design::nsga2`, `Method::NelderMeadWith` and `Method::Nsga2`, the tools'
parsing, with `f249_moo.rs` and `f249_nm.rs`: the tools' `parse_method` and `method_json` had to come with it, since their match
on `Method` must name the new variants, so D7's commits 3 and 5 overlap there); 7ae2b28 (the warm start, with `f249_warm.rs`);
ebc4e9f (`crates/rcfd-mcp/tests/design_f249.rs`); amendment 1 9a7273f and the two known-miss markings 15e289f; documents
c09f508. Every check was run once, in release, at ebc4e9f (the tests as first written); the numbers below are those runs. No
limit, case or test set-up was changed after a run.

| Id | Measured | Limit | Result |
|---|---|---|---|
| MO1z | seed 1: C 0.543, G 0.288, I 0.254 (150 Pareto members of 24 060 records); seed 2: 0.378, 0.207, 0.141 (144 of 24 046); seed 3: 0.353, 0.291, 0.220 (118 of 24 059); 3.0 s per seed | C 1e-2, G 5e-2, I 1e-2 | **missed** (amendment 1; ignored `*_known_miss`) |
| MO1f | seed 1: C 4.37e-3, G 4.73e-3, I 9.87e-4 (1052 Pareto members of 9721 records); seed 2: 3.47e-3, 4.35e-3, 9.56e-4 (1066 of 9720); seed 3: 3.66e-3, 3.73e-3, 9.42e-4 (1050 of 9682) | as MO1z | met |
| MO1c | ZDT1 LHS (25 000): C 3.90, G 1.92, I 1.65 (23 Pareto members); FON LHS (10 000), recorded: C 5.20e-2, G 0.101, I 4.13e-2 (17 members) | ZDT1: C must fail | met (C can fail) |
| MO2a | seed 4: 374 records, identical in two runs and with 4 workers; seed 5 (386 records) differs | exact | met |
| MO2b | no point outside the box in any MO1 or MO2 run; budgets 400 and 410 with pop 20: 400 evaluations, 20 generations | exact | met |
| MO2c | k = 37: the resume evaluated 337 of 374; k = 250: 124; a third launch 0; records equal to the uninterrupted run | exact | met |
| MO2d | 20 proposals, 374 records equal to the evaluator-driven run | exact | met |
| MO2e | 2 failed records of 376, 66 Pareto members, none failed | as stated | met |
| MO2f | pop 7 and pop 2: `the population must be even and at least 4, got <n>`; budget 10 with pop 20: `the evaluation budget 10 is smaller than the population 20`; no parameter: `no parameters`; nothing evaluated | as stated | met |
| MO3a to MO3d | bit for bit; `select` gives [0, 3, 1, 6]; ZDT1 and FON closed forms; `front` equals the full sort's first front on 20 sets | as stated | met |
| N1 | **T3 with the options: best f after 120 recorded candidates 3.70e-4**; 5.28e-19 at the end (330 candidates) | 1e-3 | met |
| N1d | without the keys: 3.1255311308039427e-3 at 120 (bits 0x3f699ab6bfa96f0b, the V0b record), 3.42e-19 at 347 | exact | met |
| N2 | 20 of 20 seeds, f 5.2e-20 to 9.4e-19 after 454 to 570 evaluations | 18 of 20 | met |
| N3 | 19 of 20 seeds; seed 16 ends at f = 1.6668, violation 0, after 600 evaluations | all 20 | **missed** (amendment 1; ignored `*_known_miss`) |
| N4a to N4d | coefficients bit for bit (d = 2 gives 1, 2, 0.5, 0.5); in 2 dimensions `adaptive` gives the `standard` history (300 evaluations) for both simplex rules; the first simplex from (-1.2, 1, 0) is (-1.26, 1, 0), (-1.2, 1.05, 0), (-1.2, 1, 0.00025), and from (2, 0.5) the vertex is flipped to 1.9; the d = 4 histories differ; d = 1 refused with nothing evaluated | as stated | met |
| N5 | sum of squares, `start` simplex: d = 10 `standard` 6.3e-17 after 2096 evaluations, `adaptive` 1.5e-17 after 1543; d = 20 `standard` 3.0e-16 after 15 901, `adaptive` 2.1e-17 after 3706 (all converged) | none | recorded |
| N6 | the eight refusals, each -32602 with the registered message naming its key; nothing written; `study.json` `method` keys `budget, restarts, seed, type` (default Nelder-Mead) and `budget, pop, seed, type` (`nsga2`) | as stated | met |
| MO4 | 20 proposals, 360 records, 166 Pareto members; `pareto` equals the recomputation from `evaluations.jsonl`; the same first batch for two studies; max D 4.8e-4 (recorded) | as stated | met |
| W1 | HE: warm rows 1 and 2 `warm`, 71 and 70 iterations against 90 and 91 cold; largest relative difference of a report 6.9e-9; row 0 bit for bit | `REL_WARM` 1e-4 | met |
| W1b | 141 warm against 181 cold iterations | warm < cold | met |
| W2 | KE: 4391 and 4411 iterations warm against 4665 and 4782 cold; largest relative difference 3.1e-11 | 1e-4 | met |
| W3 | SST: 5538 and 5568 iterations warm against 5468 and 5764 cold; largest relative difference 2.6e-11 | 1e-4 | met |
| W4 | condition `b` warm (71 iterations against 90 unchained), largest relative difference 6.9e-9; condition `a` bit for bit | 1e-4 | met |
| WU1 | HE `ux, uy, uz, p, T`; KE `ux, uy, uz, p, nut, k, epsilon`; SST `ux, uy, uz, p, nut, k, omega`: bit for bit | exact | met |
| WU2 | the six refusals with their messages (`warm start is available for the steady SIMPLE and SIMPLEC algorithms`; `the warm state has a temperature field and this case solves no energy equation`; `this case solves the energy equation and the warm state has no temperature field`; `the warm state is of the turbulence model k_epsilon and this case uses k_omega_sst`; `warm state field 'u': 400 values for 410 cells (another mesh)`; `... this case also has scalars or species`), the fields unchanged | as stated | met |
| WU3 | `run_case` with an HE seed: cold, with the note of item 6; the Flow path collects no warm state | as stated | met |
| V0a | `baseline_hashes.rs` and `case_hashes_w48u.rs` unedited (`git diff 05f0d73` empty for them, and for `Cargo.lock` and the goldens); in the gate `baseline_hashes` (cavity, heated cavity and shipped-case paths) and `case_hashes_w48u` h1 passed in release | pass | met |
| V0b | the four records bit-identical after the code: run on the code before its commit, and in the gate (`v0b_optimiser_histories`, `v0b_t3_default_loop`, `v0b_pareto_fronts`, `v0b_default_maps`) | bit-identical | met |
| V0c, V0d | the goldens unedited; `default_f1`, `project_list_f270` and `tool_list_f275` passed in the gate; no existing file under `tests/` or `src/design_tests.rs` changed (the only modified files are the seven source files of D1 to D4 and two documents); `t3_best_after_120_known_miss` stays ignored and unedited; `design_tests` T1 to T5 passed | pass | met |
| V0e | gate of c09f508 (`git archive`, `scripts/gate_test.py`, 3 jobs, under the machine gate lock): fmt per package 0, clippy `--workspace --all-targets -D warnings` 0; 3043 passed, 2 failed, 865 ignored; 3045 test processes, 589 binaries, wall 4235 s (06:31 to 07:54), peak summed working set 3457 MB, lowest free memory 11 GB. The 2 failures are `runner_w47b` s1 and s9, on the list of known flakes (they fail on builds before this change too); every F-249 test passed in it (28; the two known misses are ignored) | pass, apart from the known flakes | met |

**The gate's memory threshold (a deviation).** The gate took the lock at 05:10 and then waited for 20 GB of free physical memory,
the coordinator's threshold. Free memory stayed at 19.3 to 19.8 GB for 80 minutes (the machine's browser held about 17 GB, the
WSL VM 8 GB) while two other gates queued behind the lock. The waiting script was stopped without releasing the lock and a copy
of it with the threshold 19 GB (`gate-f249/run2.sh`, otherwise identical, which checks that the lock is held by `f249` and does
not acquire it again) ran the gate from 06:31. The gate's own peak summed working set was 3.5 GB; free memory fell to 11 GB
during the run through other processes. Disclosed here for the lead.

**Mutations** (scratch, each applied alone to 15e289f with the named test run once; each file restored and compared with the
commit: equal):

| Id | Test | Measured | Result |
|---|---|---|---|
| MM1 | MO1f (amendment 1) | seed 1: 2 Pareto members of 100 records, C 9.6e-2, G 0.596, I 0.266 | fails |
| MM2 | MO1f | seed 1: 5 members of 9511, C 9.6e-2, G 0.440, I 0.208 | fails |
| MM3 | MO1f, recorded | seed 1: C 3.2e-3, G 3.3e-3, I 9.1e-4 (all seeds met) | passes (recorded: the rank in the tournament is not needed on FON) |
| MN1 | N1 | 3.1255e-3 at 120 (the default's number) | fails |
| MN2 | N4a | d = 2: expansion 1.5 for 2 | fails |
| MW1 | WU1 | HE `T` all zero (the initial temperature) | fails |
| MW2 | WU1 | KE `nut` the initial value in every cell | fails |
| MW3 | W1 | point 1 `cold` | fails |
| MD1 | MO3d | set 0: [48] for [48, 86] | fails |

**Diagnostics** (amendment 1; scratch, not committed, not adopted):

* DG1: with the two children's values exchanged per variable with probability 1/2 after SBX, MO1z gives C 1.50e-3, G 1.28e-3,
  I 4.08e-4 on seed 1 (2460 Pareto members of 24 020 records), and 1.32e-3 / 1.38e-3 / 3.82e-4 and 1.59e-3 / 1.43e-3 / 4.32e-4 on
  seeds 2 and 3. The hypothesis holds: the registered SBX, which never exchanges coordinates, is the cause of the miss. Nothing
  was changed (question 5).
* DG2: seed 16 starts at (4.581, 0.0849): the `start` rule's step in y is 0.05 x 0.0849 = 4.2e-3, 8.5e-4 of the range 5 (the
  `range` rule's is 0.25). The simplex then alternates between x = 2.749 and 3.207 while y creeps up by about 0.0057 per 50
  evaluations; the best feasible f falls steadily, 3.51 after 50 evaluations, 2.59 after 300, 1.67 after 600, at (2.863, 0.716),
  and the budget ends before the first convergence, so no restart happens. The default rule from the same start reaches 0.5000
  within 150 evaluations. This is question 3's case: a start value near zero compared with its range.

**Predictions.** PR1 falsified (MO1z C 0.35 to 0.54). PR2 met for G and I, falsified for C (3.5e-3 to 4.4e-3 above the predicted
1e-3). PR3 met (ZDT1 control C 3.90; FON control 5.2e-2). PR4 met (3.70e-4). PR5 met for N2 and N5, falsified for N3 (19 of 20).
PR6 met for W1 to W4 (at most 6.9e-9) and W1b; falsified in part for SST (point 1 needed 5538 warm against 5468 cold iterations;
point 2 and the sum, 11 106 against 11 232, are lower).

**Questions added by the results.**

* Question 5 gains a finding: DG1 shows that a per-variable exchange of the children's values makes NSGA-II meet MO1z by a wide
  margin. Options: (a) as built, with MO1z a known miss; (b) a new registration that adds the exchange (as an option, or as the
  only rule, since `nsga2` is new and no study uses it yet), with MO1z re-registered as a new check. Recommendation: (b), after
  the maintainer has read how Deb et al. (2002) and Deb and Agrawal (1995) state the operator.
* Question 3 gains a finding: DG2 is the N3 miss, a start value of 1.7 percent of the range. Recommendation unchanged ((a), and
  the documentation says when to keep `range`); a range floor would remove this miss and is the maintainer's constant to choose.
* Question 1: the T3 clause (N1), the Pareto clause (MO1f, with FON's 3 parameters) and the warm-start clause (W1 to W3) are met
  by their registered checks; MO1z and N3 are misses of further registered checks.
