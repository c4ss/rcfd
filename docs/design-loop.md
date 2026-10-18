# Design loop: operating maps, DOE, optimisation and the assistant tools

Wave 32C (ADR 0067) builds the infrastructure that lets a person or the assistant iterate on a design (the fan of
`docs/milestone-fan.md`) without hand work: run a case over a range of operating points, sample or optimise design parameters,
keep every evaluation in a resumable store and ask "what next". The real fan is not part of this wave; everything below is
exercised on small laminar cases and on analytic surrogates.

The code is `crates/rcfd-api/src/design/`:

| Module | Content |
|---|---|
| `rng` | SplitMix64-seeded xoshiro256** generator (deterministic, documented) |
| `sampling` | full factorial, Latin hypercube (seeded), Sobol (dimensions 1 to 10), L2-star discrepancy |
| `optim` | Nelder-Mead with restarts, differential evolution (DE/rand/1/bin), Deb's feasibility rules |
| `pareto` | fast non-dominated sorting with constrained domination, crowding distance |
| `store` | JSON-lines evaluation store, exact bit-pattern keys, resume |
| `study` | study definition, robust scalarisation, concurrent evaluation, propose/record, best, Pareto front |
| `opmap` | operating-map driver (generic over a point evaluator), CSV/JSON tables |
| `case_eval` | running rcfd cases: the candidate evaluator, the map point evaluator, warm start |
| `tools` | the `design.*` service methods behind the MCP tools |

## Operating maps

An operating map is a list of points; a point is a set of settings-tree overrides `(dotted path, JSON value)` with a label. Points
come from axes (a grid, first axis slowest, optionally serpentine so that successive points are neighbours) or from an explicit list.

```
rcfd map cavity.toml --axis boundaries.inlet.velocity_value=[0.5,0,0],[0.6,0,0] --out map
rcfd map duct.toml --axis physics.nu=0.02:0.01:5 --axis solver.alpha_u=0.6,0.8 --serpentine --out map
```

(An axis is `path=v1,v2,...` with numbers, strings or JSON values, or `path=start:stop:count` for a linear range with exact end
points. Use `--set path=value` for fixed overrides.) Options: `--cold` never restarts from the previous point, `--stop-on-failure`
ends after the first failed point, `--no-retry` disables the cold retry, `--max-steps n` caps one run, `--json` prints the table.
Exit code 0 when all points converged, 5 otherwise, 2 for usage errors and 3 for an invalid axis path or value.

Each point restarts from the previous converged solution: the velocity and pressure of that run are written into the freshly built
simulation of the next point (`Simulation::seed_fields`, iteration counter 0). This is available for the steady SIMPLE and SIMPLEC
algorithms without energy or turbulence fields; every other set-up runs cold and says so in the `message` column (`cold start:
...`), `restart = cold`. A point that fails when started warm is repeated once cold (`retry_cold`, default on); the row then says
`restart = cold_retry`, counts the iterations of both attempts and keeps the first failure in `message`. After a failed point the
next one starts cold from the base case because the failed state is not a usable guess (`on_failure = continue`, default).

Per point the table holds: `point`, `label`, one column per overridden path, `status` (`converged`, `not_converged`, `diverged`,
`failed`), `restart` (`warm`, `cold`, `cold_retry`), `exit_code`, `state` (run-control state), `iterations`, one column per report
of the case (all reports that evaluate to a finite number, sorted by name), `message`. Files: `map.csv` and `map.json`, rewritten
atomically after every point. They contain no wall-clock times, so two runs of the same map are byte-identical; numbers are printed
with the shortest text that parses back to the same bits.

Measured on a laminar channel (ADR 0067 M3, 40 x 10 cells, SIMPLEC, inlet velocity 0.5 to 1.2 m/s in eight steps, nu 0.05, tolerances
1e-6): the warm chain needed 577 iterations against 746 for eight cold starts (ratio 0.773, limit 0.85); the pressure drop of
each point agrees between the chains to 2e-8 relative (limit 5e-4).

## Studies: DOE and optimisation

A study directory holds `study.json` (the definition) and `evaluations.jsonl` (the store). It is created with `design_create_study`
(or `design.create_study`):

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
  (largest minimised value), `mean_plus_k_sigma` (population sigma) or `weighted` (weights summing to 1), then the objectives are
  combined with their weights into the scalar the optimisers minimise. A constraint must hold at every condition; its violation is the
  excess divided by `scale`. Feasible beats infeasible, two infeasible candidates compare by total violation, two feasible ones by
  the objective (Deb's rules). A failed or unconverged evaluation is the worst possible value and is stored.
* Methods: `factorial` (`levels` per parameter), `lhs` (Latin hypercube, `n`, `seed`), `sobol` (`n`, up to 10 parameters),
  `nelder_mead` (`budget`, `restarts`, `seed`), `differential_evolution` (`budget`, `pop` default `max(10 d, 20)`, `seed`). **CMA-ES is
  not provided.** The Joe-Kuo direction numbers of the Sobol sampler were typed from memory and are verified only by the net
  properties (every power-of-two prefix is stratified, the first two dimensions form a (0, m, 2)-net), not against the published table.
* Everything is deterministic: the same definition and seed give bit-identical histories, for any `workers` value (a worker limit is
  a hard cap on simultaneous evaluations; results are written back by index).

The store has one header line and one record per evaluated candidate (`index` in request order, `key` = exact bit pattern of the
parameters, `params`, `source` (`evaluated` or `recorded`), the result of each condition with status, iterations and metrics,
`message`). A re-run of the same study finds every earlier candidate in the store and continues where it stopped, so a killed run
resumes and ends identical to an uninterrupted one; a truncated last line is dropped and that one evaluation is repeated. A store
belongs to a signature (base case fingerprint, parameters with targets, conditions, chaining, step cap): another definition refuses to
open it.

### Propose and record (ask/tell)

`design_propose` replays the deterministic method against the store with an evaluator that stops at the first missing candidate and
returns the missing points of the current batch. No optimiser state is stored, and the call has no side effects. The assistant (or a
script) evaluates the candidates itself, for example by running the variant case through the ordinary run tools, and hands the metrics
back with `design_record`; or it lets rcfd do the work with `design_evaluate` (one candidate) or `design_run_study` (the method up to
`max_evaluations` new evaluations, bounded by the service limit `max_design_evaluations`, default 1000 per call). `complete: true` means
the method has nothing left to ask.

Measured (ADR 0067): Rosenbrock from 20 seeded starts, up to 3 restarts, 800 evaluations: 20 of 20 reach `f` below 1e-18 (limit 18
of 20 at 1e-6); Rastrigin d = 4 with differential evolution, population 40, 20000 evaluations: 20 of 20 reach `f = 0` (limit 12 of 20); the
Sobol sequence has an L2-star discrepancy of 0.0051 against a seeded random mean of 0.0131 for 256 points in d = 4 (ratio 0.39, limit 0.5).
In the tool loop on Rosenbrock the Nelder-Mead best after 120 recorded candidates is 3.1e-3 (the registered check T3 asked for 1e-3: a
known miss, the run reaches 3.4e-19 after 347 candidates).

End-to-end demonstration (E1): contraction channel (`wedge_2d`, 40 x 12 cells, laminar SIMPLEC, nu 0.05, inlet velocity 1), minimise
the pressure drop over the ramp angle in [0, 12] degrees and the ramp start in [0.5, 3.0] with the constraint "mean outlet velocity at
least 1.3 times the inlet velocity", Nelder-Mead from (6 degrees, 0.5), 80 evaluations plus one restart: 77 evaluations, all converged, the
pressure drop falls from 9.94 to 5.07 (49 %, limit 25 %) at (4.46 degrees, 1.77) with the outlet velocity ratio 1.30005.

### Multi-objective

Several objectives are combined into a scalar for the optimisers, and the Pareto front of all feasible stored candidates in the
objective space (minimised form) is reported by `design_list_study` (`pareto`: record indices). `pareto::non_dominated_sort` gives
all fronts and `crowding_distance` the usual spread measure; a multi-objective optimiser (NSGA-II) is not provided.

## Assistant tools

The service methods `design.*` and the MCP tools `design_*` (see `docs/mcp-tools.md`) expose the loop. Access classes: the six tools
that write files are `write_files` and need approval under the permission model of F-091; the four that only inspect are `read`.
All paths go through the sandbox (`..` and absolute paths outside the root are error 1004 and create nothing); names (study,
condition labels, variant names) use `[A-Za-z0-9_.-]` without a leading dot; errors are located, for example
`parameter 'angle': value 14 is outside [0, 12]`.

| MCP tool | Service method | Access | Purpose |
|---|---|---|---|
| `design_create_study` | `design.create_study` | write_files | define a study (writes `study.json`; the base case is a file or the session settings) |
| `design_create_variant` | `design.create_variant` | write_files | base case (or study with parameter values) plus overrides to a variant case file |
| `design_run_operating_map` | `design.run_map` | write_files | operating map; a repeated identical call returns the stored table |
| `design_run_study` | `design.run_study` | write_files | run the method with the case solver, resumable |
| `design_evaluate` | `design.evaluate` | write_files | evaluate and store one candidate (cached when present) |
| `design_record` | `design.record` | write_files | store externally computed metrics for a candidate |
| `design_get_metrics` | `design.get_metrics` | read | metrics of a stored evaluation (index or exact parameters) or of a map (`map_dir`) |
| `design_compare_variants` | `design.compare` | read | side by side metrics, exact differences to the first, ranking |
| `design_propose` | `design.propose` | read | the next candidates of the method given the store |
| `design_list_study` | `design.list_study` | read | definition, counts, best record, Pareto indices |

Limits: `max_design_evaluations` (service configuration; `rcfd-mcp --max-design-evaluations`) bounds the number of case runs one call
may start (map points, study evaluations); `max_cells` and `max_steps_per_call` apply to every run.

## Divergence guard (PIMPLE and PISO)

An automated loop must not hang on a diverged candidate. The transient solver now stops at the first step whose state is not finite
(after the momentum predictor, on the predicted flux before each pressure solve, after each pressure corrector) or whose Courant number
is non-finite or above `solver.divergence_courant_limit` (default 1e6 when the key is 0 or unset). The run ends with state `diverged`,
exit code 6 and a located message (`PIMPLE divergence guard: <quantity> is <value> at step <n>, outer iteration <k>, <stage>, cell <i>
(x, y, z); limit <L>`). The checks only read the state, so finite runs are bit-identical to before. See ADR 0067 section 0.

## Not delivered

CMA-ES, multi-objective evolutionary search, Halton sequences, gradient (adjoint) optimisation, surrogate models, warm start for
transient, energy or turbulence runs (they run cold), and any use with the real fan (the fan case belongs to the other wave 32
work and the milestone itself).
