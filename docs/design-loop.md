# Design loop: operating maps, DOE, optimisation and the assistant tools

This page explains how rcfd runs a case over many operating points or design candidates without hand work, what each part
guarantees (determinism, resume, failure handling) and what was measured. It also describes the study and map file formats. It
is for users and assistant integrators who plan a parameter study or an optimisation. For a step-by-step walk-through with the
assistant tools read the tutorial [tutorials/mcp-design-loop.md](tutorials/mcp-design-loop.md); for the tool list and the order
of calls see [mcp-tools.md](mcp-tools.md).

Wave 32C (ADR 0067) builds the infrastructure that lets a person or the assistant iterate on a design (the fan of
`docs/milestone-fan.md`) without hand work: run a case over a range of operating points, sample or optimise design parameters,
keep every evaluation in a resumable store and ask "what next". The real fan is not part of this wave; everything below is
exercised on small laminar cases and on analytic surrogates. The [glossary](glossary.md) defines wave and ADR.

## Not delivered

* the real-fan study of F-260 (a multi-objective study on the FAN-01 passage; ADR 0222 question 1)
* Halton sequences
* adjoint gradients for turbulent, transient or energy runs
* adjoint gradients through studies, the service or MCP tools
* RBF morphing
* surrogate models
* warm start for transient runs, the coupled solver and turbulence models other than k-epsilon and k-omega SST (they run
  cold); the warm start of energy and k-epsilon / k-omega SST runs exists in the library only (below)
* any use with the real fan (the fan case belongs to the other wave 32 work and the milestone itself)

## Operating maps

An operating map is a list of points. A point is a set of settings-tree overrides `(dotted path, JSON value)` with a label.
Points come from axes (a grid, first axis slowest, optionally serpentine so that successive points are neighbours) or from an
explicit list.

```text
rcfd map cavity.toml --axis boundaries.inlet.velocity_value=[0.5,0,0],[0.6,0,0] --out map
rcfd map duct.toml --axis physics.nu=0.02:0.01:5 --axis solver.alpha_u=0.6,0.8 --serpentine --out map
```

An axis is `path=v1,v2,...` with numbers, strings or JSON values, or `path=start:stop:count` for a linear range with exact end
points. Use `--set path=value` for fixed overrides.

Options:

* `--cold` never restarts from the previous point.
* `--stop-on-failure` ends after the first failed point.
* `--no-retry` disables the cold retry.
* `--max-steps n` caps one run.
* `--warm-start extended` restarts each point from the previous one with the extended warm start (ADR 0209 D4: velocity,
  pressure, the temperature and the k-epsilon or k-omega SST fields, steady SIMPLE and SIMPLEC only; ADR 0235 B). The default,
  `--warm-start flow`, carries velocity and pressure only and runs a case with energy or turbulence cold.
* `--json` prints the table.

The exit code is 0 when all points converged, 5 otherwise, 2 for usage errors and 3 for an invalid axis path or value.

Each point restarts from the previous converged solution: rcfd writes the velocity and pressure of that run into the freshly
built simulation of the next point (`Simulation::seed_fields`, iteration counter 0). This is available for the steady SIMPLE and
SIMPLEC algorithms without energy or turbulence fields; every other set-up runs cold and says so in the `message` column (`cold start:
...`), `restart = cold`. A point that fails when started warm is repeated once cold (`retry_cold`, default on); the row then says
`restart = cold_retry`, counts the iterations of both attempts and keeps the first failure in `message`. After a failed point the
next one starts cold from the base case because the failed state is not a usable guess (`on_failure = continue`, default).

**Extended warm start (library only, ADR 0209).** `design::case_eval::ExtendedMapEvaluator` (an operating map) and
`CaseEvaluator::warm = WarmStart::Extended` (chained conditions of a study) also carry the temperature of the energy equation
and the fields of the k-epsilon and k-omega SST models (k, epsilon or omega, and the eddy viscosity), through
`Simulation::warm_state` and `Simulation::seed_state`. It is for steady SIMPLE and SIMPLEC runs; a case with scalars,
chemistry, conjugate heat transfer, radiation, an electric potential, phase change, two-way particle coupling, a mixing plane or a
dynamic mesh, another turbulence model, or a state that does not fit (another mesh, another model, a temperature for a case
without energy) runs cold with the reason in the message. `rcfd map`, the study definition and the MCP tools still use the
velocity-and-pressure warm start; wiring the extended one in is an open question of ADR 0209 (question 7). Measured (ADR 0209 W1
to W3): warm points agree with cold runs of the same points to 6.9e-9 relative on a heated channel (energy), 3.1e-11 on the
k-epsilon channel and 2.6e-11 on the k-omega SST channel (limit 1e-4); the heated channel needed 141 iterations warm against
181 cold for two points.

Per point the table holds:

* `point`
* `label`
* one column per overridden path
* `status` (`converged`, `not_converged`, `diverged`, `failed`)
* `restart` (`warm`, `cold`, `cold_retry`)
* `exit_code`
* `state` (run-control state)
* `iterations`
* one column per report of the case (all reports that evaluate to a finite number, sorted by name)
* `message`

The table goes to the files `map.csv` and `map.json`, rewritten atomically after every point. They contain no wall-clock times,
so two runs of the same map are byte-identical. Numbers are printed with the shortest text that parses back to the same bits.

Measured on a laminar channel (ADR 0067 M3, 40 x 10 cells, SIMPLEC, inlet velocity 0.5 to 1.2 m/s in eight steps, nu 0.05,
tolerances 1e-6): the warm chain needed 577 iterations against 746 for eight cold starts (ratio 0.773, limit 0.85). The pressure
drop of each point agrees between the chains to 2e-8 relative (limit 5e-4).

## Studies: DOE and optimisation

A study directory holds `study.json` (the definition) and `evaluations.jsonl` (the store). The tool `design_create_study` (or the
service method `design.create_study`) creates it:

```json
{
  "name": "contraction",
  "base": "cases/contraction.toml",
  "parameters": [
    {"name": "angle", "lo": 0, "hi": 12, "init": 6, "path": "mesh.ramp_angle"},
    {"name": "start", "lo": 0.5, "hi": 3.0, "init": 0.5, "targets": [{"path": "mesh.ramp_start", "scale": 1, "offset": 0}]}
  ],
  "conditions": [{"label": "design", "set": {"boundaries.xmin.velocity_value": [1, 0, 0]}}],
  "objectives": [{"metric": "dp", "sense": "min", "weight": 1, "robust": "mean"}],
  "constraints": [{"metric": "ux_out", "cmp": ">=", "limit": 1.3, "scale": 1.3}],
  "method": {"type": "nelder_mead", "budget": 80, "restarts": 1, "seed": 1},
  "workers": 1
}
```

* Parameters are named, continuous, with bounds and an optional initial value; the value written to each target is
  `scale * x + offset`. A library user can also pass a generator hook (`CaseEvaluator::hook`, a closure from the base case and
  the parameter vector to a case) for geometry that is not one settings path; the tool layer uses paths only.
* Operating conditions are override sets applied to every candidate; the conditions of one candidate run in order and, with
  `chain` (default true), each starts from the previous converged one.
* Metrics are the reports of the case. An objective combines its metric over the conditions with a robust mode: `mean`, `worst`
  (largest minimised value), `mean_plus_k_sigma` (population sigma) or `weighted` (weights summing to 1). The objectives are then
  combined with their weights into the scalar the optimisers minimise.
* A constraint must hold at every condition; its violation is the excess divided by `scale`. Feasible beats infeasible, two
  infeasible candidates compare by total violation, two feasible ones by the objective (Deb's rules).
* A failed or unconverged evaluation is the worst possible value and is stored.
* Methods: `factorial` (`levels` per parameter), `lhs` (Latin hypercube, `n`, `seed`), `sobol` (`n`, up to 10 parameters),
  `nelder_mead` (`budget`, `restarts`, `seed`, and the optional `initial_simplex` and `coefficients` below),
  `differential_evolution` (`budget`, `pop` default `max(10 d, 20)`, `seed`), `nsga2` (`budget` and `pop` required, `pop` even
  and at least 4, `seed`, and the optional `crossover`, `"sbx"` or `"exchange"`; see Multi-objective), `cma_es` (`budget`
  required, `pop` default `4 + floor(3 ln d)`, `sigma0` default 0.3, `restarts` default 0, `seed`; see CMA-ES below).
* Nelder-Mead options (ADR 0209, both opt-in; without them the method is unchanged): `initial_simplex = "start"` builds the first
  simplex as MATLAB's `fminsearch` does, `x0 + 0.05 x0_i e_i` (`0.00025 e_i` when `x0_i = 0`), instead of 5 % of each range
  (`"range"`, the default); `coefficients = "adaptive"` uses the dimension-dependent coefficients of Gao and Han (2012): expansion
  `1 + 2/d`, contraction `0.75 - 1/(2d)`, shrink `1 - 1/d` (`"standard"`: 2, 1/2, 1/2). In two dimensions the adaptive
  coefficients equal the standard ones. `adaptive` needs at least two parameters. The `start` rule scales with the start values,
  so keep `range` when a parameter starts at or near zero compared with its range. The Joe-Kuo direction numbers of the Sobol sampler were typed from memory and are verified only by the net
  properties (every power-of-two prefix is stratified, the first two dimensions form a (0, m, 2)-net), not against the published table.
* Everything is deterministic: the same definition and seed give bit-identical histories, for any `workers` value (a worker limit is
  a hard cap on simultaneous evaluations; results are written back by index).

The store has one header line and one record per evaluated candidate (`index` in request order, `key` = exact bit pattern of the
parameters, `params`, `source` (`evaluated` or `recorded`), the result of each condition with status, iterations and metrics,
`message`). A re-run of the same study finds every earlier candidate in the store and continues where it stopped, so a killed run
resumes and ends identical to an uninterrupted one. A truncated last line is dropped and that one evaluation is repeated. A store
belongs to a signature (base case fingerprint, parameters with targets, conditions, chaining, step cap): another definition refuses to
open it.

### Propose and record (ask/tell)

The tool `design_propose` replays the deterministic method against the store with an evaluator that stops at the first missing
candidate, and returns the missing points of the current batch. No optimiser state is stored, and the call has no side effects.

The assistant (or a script) then has two routes. It evaluates the candidates itself, for example by running the variant case
through the ordinary run tools, and hands the metrics back with `design_record`. Or it lets rcfd do the work with
`design_evaluate` (one candidate) or `design_run_study` (the method up to `max_evaluations` new evaluations, bounded by the
service limit `max_design_evaluations`, default 1000 per call). The value `complete: true` means the method has nothing left to ask.

Measured (ADR 0067):

* Rosenbrock from 20 seeded starts, up to 3 restarts, 800 evaluations: 20 of 20 reach `f` below 1e-18 (limit 18 of 20 at 1e-6).
* Rastrigin d = 4 with differential evolution, population 40, 20000 evaluations: 20 of 20 reach `f = 0` (limit 12 of 20).
* The Sobol sequence has an L2-star discrepancy of 0.0051 against a seeded random mean of 0.0131 for 256 points in d = 4 (ratio
  0.39, limit 0.5).
* In the tool loop on Rosenbrock the Nelder-Mead best after 120 recorded candidates is 3.1e-3 (the registered check T3 asked for
  1e-3: a known miss, the run reaches 3.4e-19 after 347 candidates). With `initial_simplex = "start"` and `coefficients =
  "adaptive"` the same loop reaches 3.7e-4 after 120 candidates (ADR 0209 N1) and 5.3e-19 after 330.
* With both options, Rosenbrock from 20 seeded starts (800 evaluations) reaches f < 1e-18 on 20 of 20 seeds; the constrained
  quadratic of ADR 0067 O3 is solved on 19 of 20 seeds (seed 16 stops at f = 1.67: a known miss, ADR 0209 N3). On the sum of
  squares from (1, ..., 1) with the `start` simplex, `adaptive` needs 1543 evaluations in 10 dimensions and 3706 in 20, against
  2096 and 15 901 with `standard` (ADR 0209 N5, recorded).

End-to-end demonstration (E1): contraction channel (`wedge_2d`, 40 x 12 cells, laminar SIMPLEC, nu 0.05, inlet velocity 1), minimise
the pressure drop over the ramp angle in [0, 12] degrees and the ramp start in [0.5, 3.0] with the constraint "mean outlet velocity
at least 1.3 times the inlet velocity", Nelder-Mead from (6 degrees, 0.5), 80 evaluations plus one restart. The result: 77
evaluations, all converged, and the pressure drop falls from 9.94 to 5.07 (49 %, limit 25 %) at (4.46 degrees, 1.77) with the
outlet velocity ratio 1.30005.

### CMA-ES

The method `cma_es` (ADR 0222, `design::cmaes`) is the covariance matrix adaptation evolution strategy as N. Hansen's tutorial
states it (arXiv:1604.00772, 2023: the algorithm of its Fig. 6 with the default parameters of its Table 1, active negative
weights included). It samples a population around a mean, moves the mean towards the better samples and adapts the step size and
a full covariance matrix, so it follows curved and badly scaled valleys and, with a large population, the global structure of a
function with many local minima. It runs in the unit cube of the parameter box (each parameter scaled by its range), starting from
the parameters' `init` (all of them, else a random point) with the step size `sigma0` (default 0.3 of each range, the tutorial's
rule). Each generation is one batch of evaluations, so propose, record, resume and `workers` work as for the other methods.

* **The box.** A sample outside the box is not evaluated: it ranks below every evaluated sample, by its distance to the centre of
  the box (the tutorial's rule (62) for a box). An optimum on a bound is therefore approached from inside only.
* **Stopping.** A run stops by the tutorial's termination criteria (no effect of an axis or a coordinate, covariance condition
  above `1e14`, equal or stagnating best values, `TolFun` `1e-12`, `TolX` `1e-12 sigma0`, `TolXUp`) or by the budget.
* **Restarts.** With `restarts = R`, a run that stopped by a criterion is followed by another one with the population doubled
  (Auger and Hansen 2005), up to `R` times, all from the same start point (from a new random point when there is none). Use
  restarts for functions with many local minima; the budget counts every evaluation of every run.
* `budget` has no default: a budget on a CFD case is a cost decision.

Measured (ADR 0222): on the shifted Rastrigin function in 4 dimensions, from a start three local minima away from the optimum
in every coordinate, CMA-ES with 9 restarts reaches `f <= 1e-8` within 40 000 evaluations on 20 of 20 seeds (first after 731 to
28 727 evaluations, median about 6 000), while Nelder-Mead from the same start stops in the local minimum next to it (`f = 35.8`)
on all 40 runs, with the default and with the ADR 0209 options; without restarts CMA-ES reaches it on 3 of 20 seeds. In 10
dimensions (100 000 evaluations) 14 of 20 seeds reach it (recorded). The sphere and the ellipsoid of condition `1e6` in 10
dimensions reach `f <= 1e-10` on every seed within 2 700 and 5 400 evaluations; an optimum on a corner of the box is reached to
`3e-11` in every parameter.

### Multi-objective

Several objectives are combined into a scalar for the single-objective optimisers. The tool `design_list_study` reports the
Pareto front of all feasible stored candidates in the objective space (minimised form), as record indices in `pareto`. The
function `pareto::non_dominated_sort` gives all fronts and `crowding_distance` the usual spread measure.

The method `nsga2` (ADR 0209, `design::nsga2`) searches for that front directly: NSGA-II of Deb et al. (2002), generation by
generation (`pop` evaluations per generation, one parallel batch), with binary tournaments on rank and crowding distance,
simulated binary crossover (probability 0.9, index 20), polynomial mutation (probability 1/d, index 20), and survivors chosen
front by front from parents and children. Constraints use constrained domination (a feasible candidate beats an infeasible
one). The objective weights are not used; the study's `best` is still the best by the weighted scalar. The method's result is the
`pareto` list of `design_list_study`, over every stored candidate. Propose, record, resume and `workers` work as for the other
methods. Measured (ADR 0209): on FON (3 parameters, 10 000 evaluations, population 100) every member of the returned Pareto set
lies within 4.4e-3 of the analytic front and the front is covered with gaps below 4.8e-3 (limits 1e-2 and 5e-2, three seeds); on
ZDT1 with 30 parameters (25 000 evaluations) the set stays 0.35 to 0.54 from the front (a known miss, ADR 0209 MO1z, limit
1e-2).

The crossover (ADR 0220, opt-in). With the default `crossover = "sbx"` (or no key) each child stays next to its own parent in
every variable, so a good coordinate of one parent reaches the other's line only through selection and mutation; that is the
cause of the ZDT1 miss. `crossover = "exchange"` also exchanges the two children's values of each variable of a crossed pair with
probability 1/2. Measured (ADR 0220): on ZDT1 with 30 parameters every member of the Pareto set lies within 1.6e-3 of the front,
gaps below 1.5e-3 (three seeds, the limits above); on FON within 5.9e-3, gaps below 6.4e-3. Without the key a study writes and
replays exactly what it did before; with it, `study.json` records the key. The constants (crossover probability 0.9, indices
20, mutation probability 1/d) and the test problems were checked against Deb et al. (2002), and the SBX and mutation formulas
against Deb and Agrawal (1995, 1999), for ADR 0220. Two differences from those papers remain: they cross each variable with
probability 0.5 (rcfd crosses every variable), and they bound the distributions where rcfd clamps into the box
(`docs/reference-data.md`, ADR 0220 questions 2 and 4).

## Assistant tools

The service methods `design.*` and the MCP tools `design_*` (see `docs/mcp-tools.md`) expose the loop. Access classes: the eight
tools that write files are `write_files` and need approval under the permission model of F-091; the six that only inspect are
`read`. With `rcfd-mcp --project`, `design_run_study` and `design_evaluate` are `execute` instead, since they start runs of the
project's runner there (ADR 0196 B); they still need approval. All paths go through the sandbox (`..` and absolute paths outside
the root are error 1004 and create nothing). Names (study, condition labels, variant names) use `[A-Za-z0-9_.-]` without a leading
dot. Errors are located, for example `parameter 'angle': value 14 is outside [0, 12]`.

| MCP tool | Service method | Access | Purpose |
|---|---|---|---|
| `design_create_study` | `design.create_study` | write_files | define a study (writes `study.json`; the base case is a file or the session settings) |
| `design_create_variant` | `design.create_variant` | write_files | base case (or study with parameter values) plus overrides to a variant case file |
| `design_run_operating_map` | `design.run_map` | write_files | operating map; a repeated identical call returns the stored table |
| `design_run_study` | `design.run_study` | write_files (execute with `--project`) | run the method with the case solver, resumable |
| `design_evaluate` | `design.evaluate` | write_files (execute with `--project`) | evaluate and store one candidate (cached when present) |
| `design_record` | `design.record` | write_files | store externally computed metrics for a candidate |
| `design_get_metrics` | `design.get_metrics` | read | metrics of a stored evaluation (index or exact parameters) or of a map (`map_dir`) |
| `design_compare_variants` | `design.compare` | read | side by side metrics, exact differences to the first, ranking |
| `design_propose` | `design.propose` | read | the next candidates of the method given the store |
| `design_list_study` | `design.list_study` | read | definition, counts, best record, Pareto indices |
| `design_ledger_append` | `design.ledger_append` | write_files | append one evaluated variant to a design ledger |
| `design_ledger_query` | `design.ledger_query` | read | filter; Pareto set, best so far, parameter history |
| `design_ledger_export` | `design.ledger_export` | write_files | ledger entries to CSV or JSON |
| `design_ledger_rerun_check` | `design.ledger_rerun_check` | read | compare a re-run with a ledger entry |

Limits: `max_design_evaluations` (service configuration; `rcfd-mcp --max-design-evaluations`) bounds the number of case runs one call
may start (map points, study evaluations); `max_cells` and `max_steps_per_call` apply to every run in the server process.

### In a project: evaluations on the runner (ADR 0189)

With `rcfd-mcp --project`, `design_run_study` and `design_evaluate` evaluate each candidate at each operating condition as one run of
the project's runner ([ADR 0189](adr/0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md)), not in
the server process. Their arguments, the study folder and the store are the same; so are the other design tools.

* A run is the base case file with one `rcfd run --set` entry per parameter target and per setting of the condition. Before the
  run is queued, the base case with those entries is checked to give the settings tree the in-process evaluator builds.
* Its `run.json` names the study folder (`study`, for example `designs/nu3`), the candidate (`variant`, for example `nu=0.015`)
  and the condition's label (`condition`). These fields appear only in the runs of a study.
* Its result is read from the run's `metrics.json`: the status by the in-process rule on the solver's `finish` event (a run the
  runner calls `finished` may still be `not_converged`), the metrics from its report values.
* `workers` candidates are in flight at once, each with all its conditions queued; the runner's `max_runs` bounds how many run.
  The runs count against the runner's budgets; as every run of the runner, they go on when the server stops and show in the
  Studio's run list (ADR 0123, ADR 0133; not checked again by F-274's tests).
* The result adds `evaluator` (`runner`), `runs` (at most 50 objects {`run`, `variant`, `condition`, `state`} of the runs this
  call made or adopted, with `continue_from` for a continued run and `adopted: true` for an adopted one), `runs_total` (the runs
  it submitted) and `runs_adopted`.

Chained conditions and step caps ([ADR 0200](adr/0200-f287-registration-runner-runs-that-continue-from-a-previous-run-with-a-step-cap.md)):

* With `chain` (true by default) and two or more conditions, the conditions of a candidate run one after another. A condition
  continues from the run of the condition before (`run.json` `settings.continue_from`, which the runner passes as `rcfd run
  --continue-from runs/<id>/output/restart.rcfd`) when that condition was usable and its restart file is of the steady engine,
  without a temperature field, written for the same mesh; otherwise it runs cold and its message says why (`cold start: ...`).
  A continued run keeps the face flux and the iteration counter of the run before, where the in-process chain rebuilds the flux
  and counts from 0, so the two agree to the solver tolerance, not bit for bit: measured on the 16 x 16 cavity, 5.6e-10
  relative on `lid_drag` (ADR 0200 C-CH, limit 1e-4). Its `iterations` count the steps of that run; because `--continue-from`
  keeps the iteration cap, it has `max_iterations` minus the restart's iteration left (said in the message when it does not
  converge).
* A study's own `max_steps` caps each run (`settings.max_steps`, passed as `rcfd run --max-steps`), at the value the in-process
  evaluator uses (at most `max_steps_per_call`).
* `design_run_operating_map` runs each point as one run (untagged), each continued from the run of the converged point before by
  the same rule; the table, its files and the cache are those of the server process.

Adopting runs ([ADR 0201](adr/0201-f288-registration-adopt-matching-runs-after-an-mcp-server-restart.md)): a server stopped during
a study leaves its runs in the runner and no record of them in the store. Before a study's run is submitted, the call looks for a
run of the project with the same `study`, `variant`, `condition`, `case`, case bytes (`case_snapshot_hash`), `settings.set`,
`settings.continue_from`, `settings.max_steps` and `solver_version`; one that is not `failed` or `cancelled` is adopted (the newest
`finished` one, else the newest of the others), waited for or read from its `metrics.json`. A resumed study so makes no second run
for a candidate that has one, and its records equal those of a study that was not interrupted. Operating maps adopt nothing (an
identical map call returns its stored table).

A study or map whose base case is outside the project is evaluated in the server process, as before, with `evaluator: process`
and the reason in `evaluator_note`. The server's `max_cells` and `max_steps_per_call` do not bound runs on the runner, as they do
not bound `start_run`, except that a study's own step cap is bounded by `max_steps_per_call` as in process.

Measured (ADR 0189 C4, the 32 x 32 lid-driven cavity at three viscosities): the runner's records equal the in-process ones bit for
bit (largest relative difference of a metric 0, the same iteration counts). One evaluation took 0.64 s on the runner and 0.23 s in
the server process, so about 0.4 s more per run; the time was not broken down.

## Divergence guard (PIMPLE and PISO)

An automated loop must not hang on a diverged candidate. The transient solver now stops at the first step whose state is not finite
(after the momentum predictor, on the predicted flux before each pressure solve, after each pressure corrector) or whose Courant number
is non-finite or above `solver.divergence_courant_limit` (default 1e6 when the key is 0 or unset). The run ends with state `diverged`,
exit code 6 and a located message (`PIMPLE divergence guard: <quantity> is <value> at step <n>, outer iteration <k>, <stage>, cell <i>
(x, y, z); limit <L>`). The checks only read the state, so finite runs are bit-identical to before. See ADR 0067 section 0.

## Divergence guard (steady SIMPLE, SIMPLEC, coupled)

Since ADR 0077 a steady run is also stopped when it diverges: a non-finite residual or field, a continuity that grows by 1e6 over 50 iterations,
or a speed or pressure range that runs away against its value after the first iteration (`solver.steady_divergence_guard`, `docs/case-schema.md`).
The design loop's runs go through the run-control driver, so a diverged candidate or operating point ends after the tripping
iteration with status `diverged` (a failed evaluation, `Fitness::FAILED`) and the located reason, including the kind and the last
good iteration, in the record's message. Before, a finite blow-up such as SIMPLE with alpha 1/1 ran to its iteration cap as
`not_converged`.

PISO/PIMPLE runs with one pressure corrector also make one pressure-lag estimate and report a `pressure_lag` warning when the
lagged pressure error will be amplified (`solver.pressure_lag_check`).

## Adjoint shape sensitivities and the gradient method (library only, ADR 0091)

The module `rcfd_api::design::adjoint` computes the derivative of objectives of a converged steady laminar SIMPLE/SIMPLEC solution
with respect to shape parameters. The function `rcfd_api::design::optim::projected_gradient` uses such gradients (bound-constrained
steepest descent, Armijo backtracking; trial points ask for the value only). Library API only: no case-file, study, service or MCP
entry.

* Morph: `Ffd`, a Bernstein free-form-deformation lattice over a box in the `(x, y)` plane; each parameter moves one control
  point in one direction, points outside the box stay; the mesh is rebuilt with `rcfd_mesh::motion::moved_mesh` (same topology).
* `adjoint_gradient(build, a, converged, objectives, opts)`: `build(a)` returns a solver on the morphed mesh with the case's
  conditions and settings; `converged` is the converged `SimpleState` at `a`. `shape_gradient` solves the primal from rest first.
* Formulation: fixed-point discrete adjoint with a finite-difference-assembled dense Jacobian of one SIMPLE/SIMPLEC iteration
  (`(I - G_X)^T lambda = J_X^T`, `dJ/da = J_a + lambda^T G_a`). At a converged fixed point this is the sensitivity of the discrete
  residual root, independent of the relaxation. Cost: `2 N` single iterations plus a dense LU of size `N` (state unknowns), so
  it is for small meshes only (`max_state` 4000 by default); it is independent of the number of parameters.
* Accurate for: the discrete objective of the converged discrete solution on the given mesh and settings, interior-node motion
  included; not the mesh-converged quantity. Needs inner linear solves converged to round-off (as `adjoint::Duct::settings`) and
  no face flux on the upwind or limiter switch. Refused: turbulence (`nu_face`; no frozen-turbulence approximation is made),
  body forces, the bulk-velocity driver, the pseudo-time controller and face jumps.
* Verified (ADR 0091): 2D duct, Re 20, 40 x 10 cells, three FFD parameters: the adjoint derivatives of the pressure drop and the
  wall drag match central finite differences of converged solutions to 1.1e-6 relative (registered limit 5 percent). One gradient
  (N = 2050) takes about 6 s against 0.33 s per primal solve, so it pays off only beyond roughly 9 parameters (central
  differences) on this mesh.

## Design ledger (ADR 0121)

The study store above belongs to one optimiser run. The design ledger (`design::ledger`) keeps every evaluated variant of every study,
evaluator and fidelity tier in one JSON-lines file, so the design history survives sessions:

* An entry holds the parameters and metrics as `{name, value, unit}` (unit `1` when dimensionless), the evaluator id and version, the
  solver version (`rcfd <version>`, plus the git revision when the build sets `RCFD_GIT_REV`), a geometry or mesh hash, the settings
  that re-run it, a verdict (`ok`, `diverged`, `constraint_rejected`, `not_evaluated`) with a located reason, the tier (`analytic`,
  `reduced_order`, `cfd`), a time stamp, a seed and free provenance. Its index is the position of its line.
* Crash safety: each batch is one append-mode write followed by a sync; a writer that finds a torn last line (from a killed process)
  starts with a newline, and the reader reports such lines and skips them. Nothing is truncated or rewritten. Threads share one
  `Ledger`; several writer processes are not locked against each other (single-writer assumption).
* Queries: filters, Pareto set, best so far with its running trace, history of one parameter, CSV and JSON export. Pareto and best use
  `ok` evaluation entries of one evaluator and tier only, and refuse a metric recorded in two units.
* Re-run: an evaluator implements `ledger::Model` (id, version, tier, evaluate over named parameters); `ledger::rerun_check` refuses
  another id or version, re-evaluates from the stored parameters and compares metrics, units, verdict and geometry hash.

## Tolerance and uncertainty studies (ADR 0121, library only)

The function `design::uq::run(spec, model, ledger)` samples named parameters with units from uniform, truncated normal or beta
distributions (Monte Carlo, or replicated Latin hypercubes: one independent LHS per batch) and evaluates any `Model` with a worker
limit. It stops when the standard error of the mean of every controlled metric is below `max(se_abs, se_rel |mean|)` or at
`max_samples` (`converged = false`).

For each metric it reports:

* the mean, standard deviation and standard error
* quantiles with distribution-free order-statistic intervals at a stated confidence (exact coverage for Monte Carlo, approximate
  for LHS)
* the probability of violating each requirement with a Clopper-Pearson interval (a failed sample counts as a violation)
* a sensitivity ranking by standardised regression coefficients with R^2

SRC sees linear effects only: an R^2 well below 1 means the ranking misses interactions or non-monotone effects. Every sample and
a summary are written to the ledger. The same seed gives a byte-identical ledger for any worker count (with a fixed clock). It is
verified on analytic functions (closed-form means, variances, quantiles and SRC); no fan evaluator is attached yet and there is no
MCP tool for it.

## Sobol sensitivity indices (ADR 0240, library only)

The function `design::sensitivity::run(spec, model, ledger)` estimates the first-order index `S_i` and the total index `ST_i` of
every independent input (the distributions of `uq`) for every metric. It evaluates two Monte Carlo matrices `A` and `B` of `N` rows
and, for each input `i`, `A` with column `i` from `B`: `N (d + 2)` evaluations. The first-order estimator is the centred form of
Saltelli et al. (2010), the total estimator Jansen's (1999); each comes with a delta-method standard error. Unlike SRC it sees
interactions (`ST_i - S_i`) and non-monotone effects; an input the model does not read gets exactly zero. A row with a failed
evaluation is dropped as a whole. Every evaluation and a `sobol_summary` entry can be written to the ledger; the result does not
depend on the worker count. It is opt-in, verified on the Ishigami function and a linear function, and has no MCP tool.

## Code map

The code is `crates/rcfd-api/src/design/`:

| Module | Content |
|---|---|
| `rng` | SplitMix64-seeded xoshiro256** generator (deterministic, documented) |
| `sampling` | full factorial, Latin hypercube (seeded), Sobol (dimensions 1 to 10), L2-star discrepancy |
| `optim` | Nelder-Mead with restarts and the ADR 0209 options, differential evolution (DE/rand/1/bin), Deb's feasibility rules |
| `cmaes` | CMA-ES with restarts of doubled population (ADR 0222) |
| `nsga2` | NSGA-II, the multi-objective optimiser (ADR 0209) |
| `pareto` | fast non-dominated sorting with constrained domination, crowding distance |
| `store` | JSON-lines evaluation store, exact bit-pattern keys, resume |
| `study` | study definition, robust scalarisation, concurrent evaluation, propose/record, best, Pareto front |
| `opmap` | operating-map driver (generic over a point evaluator), CSV/JSON tables |
| `case_eval` | running rcfd cases: the candidate evaluator, the map point evaluators, warm start (flow, or extended) |
| `tools` | the `design.*` service methods behind the MCP tools |
| `ledger` | design ledger across studies (ADR 0121): entries with units, verdicts, tiers, queries, export, re-run check |
| `uq` | tolerance / uncertainty study driver (ADR 0121): distributions, MC and replicated LHS, stopping rule, statistics, SRC |
| `sensitivity` | first-order and total Sobol indices with standard errors (ADR 0240) |

## Where to go next

* [tutorials/mcp-design-loop.md](tutorials/mcp-design-loop.md): the design loop driven through MCP, executed step by step.
* [milestone-fan.md](milestone-fan.md): the fan design this loop is built for.
* [studies.md](studies.md): the older parametric-study section of the case file.
