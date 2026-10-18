# ADR 0067: Wave 32C, design-loop infrastructure (PIMPLE divergence guard, operating-map automation, DOE and optimisation, assistant tools)

Status: accepted. Everything below "Registered" was written before the code or run it governs. Limits are not changed, widened, replaced or deleted after a run;
a miss stays an ignored `*_known_miss` test with the limit intact, a test set-up defect gets a NEW corrected test and the old one stays. Deviations are disclosed in
addenda at the end. Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) and all existing tests stay bit-identical and are never re-recorded; if a proven
change would alter one, the work stops and is reported. Clean-room: published literature only (listed in `docs/reference-data.md`); no source listing of any other
CFD or optimisation code was opened. The real fan is not part of this wave (`docs/milestone-fan.md`, wave 33).

Parents: `docs/milestone-fan.md` (design-loop infrastructure, "Solver robustness under automation"), ADR 0034 (parametric studies F-095), ADR 0050 (robustness),
ADR 0062 item 3 (the 181 x 141 cylinder dt 0.05 blow-up, wave 31A finding: no divergence guard in `PimpleSolver::step`, 12000 pressure iterations per step after a NaN).
Permission model: F-091 (`rcfd-mcp::policy`); tool layer: F-090, F-147, F-148.

## Registered

### 0. PIMPLE/PISO divergence guard (additive)
Defect: after a NaN or a blow-up `PimpleSolver::step` keeps solving (the pressure solve of a non-finite right-hand side runs to its iteration cap, 3000 per solve,
12000 per step in the wave 31A case), so an automated run looks hung. `runctl::drive` only stops after the step, and only for non-finite residuals; a finite blow-up
(fields of order 1e30) is not caught at all.
Design (fixed now):
* `PimpleSettings.divergence_courant_limit: f64`, default `1e6` (`PimpleSettings::piso/pimple`). Tree key `solver.divergence_courant_limit` (float, >= 0), **unset
  sentinel 0** meaning the built-in default 1e6. A positive value replaces it; a value that disables the Courant criterion in practice is `1e300`. A non-finite
  state is never a valid result and is always fatal (not switchable).
* `PimpleSolver.diverged: Option<Divergence>` (step, outer iteration, stage, quantity, cell index, value, limit). Checks are read-only and run (a) after each
  momentum-predictor component solve (finite velocity), (b) on `phiHbyA` before each pressure solve (finite), (c) after each pressure corrector (finite pressure and
  velocity), (d) after the step (finite flux, Courant number <= limit; a non-finite or above-limit Courant number trips). On a trip the step returns at once with a
  `StepInfo` whose `res_u`/`res_p`/`courant` are NaN when non-finite (otherwise the measured values), `p_iters` as spent so far, the solver state is NOT rolled
  back, the history is not advanced, `diverged` is set. While `diverged` is set every further `step()` returns the same stale `StepInfo` immediately (zero
  pressure iterations). `restore()` clears it. The fractional-step path gets checks (c) and (d) only.
* For finite states below the limit nothing is modified: the checks only read, so every result is bit-identical (the baseline hashes and all existing tests).
* `Simulation` turns a trip into the located failure message `PIMPLE divergence guard: <quantity> is <value> at step <n>, outer iteration <k>, <stage>, cell <i>
  (x, y, z); limit <L>; lower solver.dt or the relaxation factors` (the cell location is given when a cell index exists), `failure` is set, `runctl::drive` ends
  with state `diverged`, exit code 6 (as for non-finite residuals before).
Numeric checks (registered):
* G1 Synthetic unstable run (cheap, seconds), tried in this order, the first that blows up with the guard switched off (non-finite state, or Courant above 1e6, within
  500 steps; the test asserts that at least one candidate does, otherwise the guard test is reported as not exercised and an addendum records it): A. 12 x 12 lid
  cavity, nu 1e-5, PISO, central convection, dt 2.0 (Courant about 24), `ddt_correction` on; B. same with PIMPLE `alpha_u` 1.9, `alpha_p` 1.5, 4 outer iterations
  (fv-level settings, outside the schema range); C. same as A with `bdf2` and dt 5.0. With the guard on the run stops at the first step whose state is bad: at most
  **300 steps** from the start, wall time of the guarded run below **20 s** in release, the stopping step spends at most **3000 pressure iterations** (one solver
  cap; before the guard 12000), `diverged` names stage, quantity and step, further `step()` calls take 0 pressure iterations.
* G2 Never triggers: `cargo test --workspace --release` (all existing tests, including baseline hashes and the cylinder/cavity validations that are not ignored)
  is unchanged; in particular no existing test sets the new key.
* G3 Located error through the API: a case of the tree with `solver.algorithm = "piso"` and the candidate A parameters ends via `runctl::drive` with state
  `diverged`, exit code 6 and a reason containing "divergence guard" and "step"; the key rejects a negative value with a located validation error; unset equals 1e6
  (the resolved settings print 0 and the solver receives 1e6).
* G4 Finite states are untouched: for the lid-driven cavity with PISO (a stable run, 200 steps) the fields with the guard on and with `divergence_courant_limit = 1e300`
  are bit-identical (u, p, phi compared by `to_bits`).

### 1. Operating-map automation (F-254 candidate)
Module `crates/rcfd-api/src/design/opmap.rs`. A map is a list of operating points; each point is a set of settings-tree overrides `(path, JSON value)`
(for example `boundaries.inlet.velocity_value`, `boundaries.outlet.pressure_value`, a rotation speed path) with a label, built from axes (`grid` row-major,
first axis slowest, optional serpentine order for the last axis; or an explicit list). Points run in order, each restarting from the previous solution
(velocity and pressure of the previous converged point written into the freshly built simulation of the next point through `set_fields`, iteration counter 0;
steady laminar SIMPLE/SIMPLEC and transient without extras; where the engine cannot be seeded the point runs cold and says so). Collected per point: all
reports of the case (chosen subset optional), iterations, run state, exit code, wall seconds (not in the deterministic table), `restart` = `warm`/`cold`/`cold_retry`,
`status` = `converged`, `not_converged`, `diverged`, `failed`. Failure handling: `on_failure = "continue"` (default; the next point is cold-started from the base
case, because the failed state is not a usable guess), `"stop"`, and `retry_cold` (default true: a warm point that fails is repeated once cold, both attempts
recorded in the message). Outputs: `map.csv`, `map.json` (written after every point, atomically). The numeric core is generic over a `PointEvaluator` trait so that
an analytic surrogate and the real case evaluator share it. Case-tree commands: `rcfd map <case> --axis path=v1,v2,... [--axis ...] [--out dir] [--cold] [--stop-on-failure] [--set ...]`
and the `map` section is NOT added to the case schema (a map is a driver, not a setting); API/MCP: `design.run_map` / `design_run_operating_map`.
Numeric checks (registered):
* M1 Analytic surrogate: pump/fan curve `dP(Q, N) = a N^2 - b N Q - c Q^2` with a = 2.0, b = 0.5, c = 3.0, shaft power `P(Q, N) = p0 N^3 + p1 N Q^2` with p0 = 0.1, p1 = 0.25,
  efficiency `eta = Q dP / P` (P > 0), axes Q in {0.1, 0.2, 0.3, 0.4, 0.5} (inner) and N in {1.0, 1.5, 2.0} (outer): 15 points; every table value equals the closed form
  **to the bit** (the surrogate evaluator uses an independently written expression in the test), the order is the grid order, the status of all points is `converged`,
  each point except the first received the previous point's state (counted by the surrogate), the CSV round-trips every value to the bit (shortest round-trip print).
* M2 Failure handling: a surrogate that fails at flow index 3 of speed 1.5 (and fails again if started warm from the point before it): `continue` gives 15 rows with exactly
  one `failed`, the next point `restart = cold`, a warm point that fails and then passes when cold under `retry_cold` is recorded `cold_retry` with status `converged`;
  `stop` ends after the failing point with exactly the rows up to it.
* M3 Restart benefit on a real small laminar case (steady SIMPLEC, 2-D channel 40 x 10, pressure outlet, inlet velocity sweep 0.5, 0.6, ..., 1.2 m/s, nu 0.05, relative
  tolerances 1e-6): the total iterations of the warm chain are at most **0.85 times** those of eight cold starts (a reduction factor of at least 1.18), every point converged in both
  chains, and the pressure-drop report of each point agrees between the chains within **5e-4 relative**. (Reasoning: the SIMPLEC residual falls geometrically, a warm start saves the
  number of e-folds between the cold initial residual and the warm one, a fixed fraction of the 1e-6 tolerance range, so a modest and not an orders-of-magnitude saving is expected.)
* M4 Determinism: two runs of the same map give byte-identical CSV and JSON tables.

### 2. DOE and optimisation (F-095 partial, F-257 candidate)
Modules `crates/rcfd-api/src/design/{sampling,optim,pareto,store,study}.rs`, pure numerics first, independent of the solver.
* Parameters: named, continuous, bounds `[lo, hi]` (lo < hi, finite), optional initial value, mapped to one or more settings-tree targets `{path, scale, offset}`
  (value written = `scale * x + offset`) and/or a case-generator hook (a Rust closure `(base case, parameter values) -> case`, library use only; the tool layer uses paths).
* DOE samplers on the unit cube, scaled to bounds: `full_factorial(levels)` (levels per parameter, endpoints included, row-major), `latin_hypercube(n, d, seed)` (stratified permutation sampling
  with an own, documented SplitMix64-seeded xoshiro256** RNG, points jittered inside the stratum), `sobol(n, d)` (unscrambled Sobol sequence, Gray-code construction, dimensions 1 to 10,
  direction numbers of Joe and Kuo recalled from memory and verified only by the net properties below; more than 10 dimensions is a located error), `halton` is not provided.
* Optimisers (gradient free, bound-aware, seeded, deterministic): **Nelder-Mead** (Nelder and Mead 1965, standard coefficients 1, 2, 0.5, 0.5, initial simplex of 5 % of the range per axis from the initial point,
  every trial point clamped into the box, up to `restarts` restarts from the best point with a re-seeded simplex of half the previous size) and **differential evolution** (Storn and Price 1997, DE/rand/1/bin,
  F = 0.5, CR = 0.9, population `max(10 d, 20)`, bounds by reflection back into the box, generation-synchronous so each generation is one parallel batch). **CMA-ES-lite is not delivered**; this is stated, not hidden.
  Constraints `g_i(x) <= 0` use Deb's feasibility rules (feasible beats infeasible; two infeasible compare by total violation; two feasible by objective), in both optimisers.
  A failed evaluation is the worst possible fitness (infeasible with infinite violation) and is recorded.
* Multi-objective: each evaluation yields a vector of objective values per operating condition; `Robust` scalarisation per objective over the conditions: `mean`, `worst` (max of the minimised
  quantity), `mean_plus_k_sigma` (population sigma, k >= 0) or `weighted` (weights summing to 1); objectives are then combined by a weight vector (default equal) into the optimiser's scalar.
  `pareto::non_dominated_sort` (Deb et al. 2002 fast non-dominated sorting, minimisation, constraint-dominated: a feasible point dominates any infeasible one) gives fronts, and `pareto::front` the first front of a
  study's feasible records.
* Concurrency: `parallel_map(items, workers, f)` on std scoped threads; results are written back by index, so everything is independent of the worker count; a `workers` limit is a hard cap on simultaneous
  evaluations (measured by a counter).
* Persistent store (JSON lines, `evaluations.jsonl`, one record per evaluation: `index` (request order), `key` (exact bit pattern of parameters and condition label), `params`, `status`, `metrics`
  per condition, `message`); appended after each batch in index order; an incomplete final line is ignored. Resume = rerun the same deterministic study: every evaluation whose key is in the store is
  answered from the store without calling the evaluator, so an interrupted study continues and ends identical to an uninterrupted one. The same replay gives the assistant "propose next": run the optimiser against the store
  with an evaluator that stops at the first missing key and returns the missing points of the current batch (ask/tell without any extra optimiser state).
Numeric checks (registered; budgets are evaluation counts, seeds 1 to 20 unless stated):
* O1 Rosenbrock `100 (y - x^2)^2 + (1 - x)^2`, box [-2, 2]^2, start drawn from the seed in the box, Nelder-Mead with up to 3 restarts, budget **800 evaluations**: at least **18 of 20 seeds** end with `f <= 1e-6` and
  `|x - 1|, |y - 1| <= 1e-3`.
* O2 Rastrigin `10 d + sum(x^2 - 10 cos(2 pi x))`, d = 4, box [-5.12, 5.12]^4 (minimum 0 at the origin), differential evolution, population 40, budget **20000 evaluations**: at least **12 of 20 seeds** end with
  `f <= 1e-6`, and at least **19 of 20** with `f <= 1.0` (the global basin or one neighbouring local minimum).
* O3 Constrained quadratic `(x - 3)^2 + (y - 2)^2` with `x + y <= 4`, box [0, 5]^2; the optimum is the projection of (3, 2) on x + y = 4: (2.5, 1.5), f* = 0.5. Nelder-Mead (budget **600**) ends on all 20 seeds with
  `f - 0.5 <= 1e-3` and violation `<= 1e-6`; differential evolution (budget **3000**) ends on all 20 seeds with `|f - 0.5| <= 1e-4` and violation `<= 1e-6`.
* O4 Determinism: the same seed gives a bit-identical history (all evaluated points and values, compared by `to_bits`) for both optimisers; 1 worker against 4 workers also bit-identical.
* O5 Bounds: every evaluated point of every run above lies inside the box (exact comparison).
* R1 Robust scalarisation: for `f(x; c) = (x - c)^2` over conditions c in {0, 1, 3}: `mean` minimum at x = 4/3 with value 14/9 (the variance of c), `worst` minimum at x = 1.5 with
  value 2.25, `mean_plus_k_sigma` values equal hand-computed ones to 1e-12; the optimiser finds the `worst` optimum x within 1e-3 and the `mean` optimum within 1e-4 (Nelder-Mead, seeds 1 to 5, budget 300).
* D1 Full factorial: point count equals the product of the levels exactly, endpoints are the bounds exactly, row-major order.
* D2 Latin hypercube: for N = 20, d = 5 and seeds 1 to 10 each of the N strata `[k/N, (k+1)/N)` of every dimension holds exactly one point; same seed, same bits; different seeds differ.
* D3 Sobol: the first 8 points of dimension 1 are `0, 0.5, 0.75, 0.25, 0.375, 0.875, 0.625, 0.125` and of dimension 2 `0, 0.5, 0.25, 0.75, 0.375, 0.875, 0.125, 0.625` exactly; for N = 2^m, m = 3..8, and every one of the 10 dimensions, each interval
  `[k/N, (k+1)/N)` holds exactly one point; dimensions 1 and 2 form a (0, m, 2)-net: every dyadic box with `2^a x 2^b`, a + b = m, holds exactly one point (m = 3..8); the L2-star discrepancy (Warnock's formula) of the first 256 points
  in d = 4 is at most **0.5 times** the mean of 20 seeded pseudo-random sets of 256 points.
* P1 Pareto: 200 points exactly on the ZDT1 front (`f2 = 1 - sqrt(f1)`, `f1 = i/199`) mixed with 300 points strictly dominated by them (`f2 + 0.05 + 0.1 u`, `u` from the seeded RNG) in a shuffled order: the first front equals
  exactly the 200 front indices, every other point has rank >= 1, the fronts partition the set; a quarter-circle front and a 3-objective plane `f1 + f2 + f3 = 1` check the same; exact duplicates are both non-dominated; a feasible
  point dominates an infeasible one regardless of objectives.
* S1 Resume: a study (Nelder-Mead on Rosenbrock, then separately differential evolution) is stopped after k evaluations (k = 7 and k = 61, stop request between batches) and resumed from the store: the final
  records (sorted by index) are bit-identical to an uninterrupted run, the evaluator is called exactly `total - k` times in the resume, and a third launch calls it 0 times. A store with a truncated last line resumes
  identically (that evaluation is repeated).
* S2 Concurrency: with `workers = 3` the maximum simultaneous evaluations measured by an atomic counter is at most 3 (and reaches 2 or more on a population batch), and the records equal those of 1 worker.
* E1 End-to-end demo on a real rcfd case (laminar steady SIMPLEC, 2-D channel 40 x 12, `wedge_2d` generator, walls on both sides, no symmetry, velocity inlet, pressure outlet): minimise the pressure drop (area-averaged `p` at the inlet minus
  the outlet value) over the contraction ramp angle in [0, 12] degrees and ramp start in [0.5, 3.0] with the flow constraint "mean outlet streamwise velocity >= 1.3 times the inlet velocity" (the contraction must accelerate the
  flow). Start point (6 degrees, 0.5) which is feasible. Nelder-Mead, budget **80 evaluations**, 1 restart. Acceptance: the best feasible objective is at least **25 % lower** than the start; the best point satisfies the constraint
  (violation <= 1e-3 relative); all records are `converged`; re-running three stored evaluations (indices 0, middle, last) from their stored parameters reproduces their metrics bit-identically; the study resumed from its store at
  the end makes 0 new evaluations. (Reasoning: for a viscous channel the pressure drop is dominated by the integral of `1 / h^3`; keeping the height at 1 for as long as possible and contracting late gives roughly 40 % less than the start.)

### 3. Assistant tools (MCP / API), F-091 model
New service methods `design.*` (`rcfd-api::design::tools`) and MCP tools in `rcfd-mcp` (so the assistant crate, which lists MCP tools, gets them without change):
* `design_create_study` (write_files): define a study (parameters, targets, conditions, objectives, constraints, robust mode, sampler or optimiser, store directory) from a base case file or session settings; writes `study.json`.
* `design_create_variant` (write_files): base case + parameter overrides (or parameter values of a study) -> variant case file, validated through the settings tree, returns path and fingerprint.
* `design_run_operating_map` (write_files, long running; bounded by `max_design_evaluations` of the service configuration, default 1000, and the per-point step cap): map of a case or variant.
* `design_evaluate` (write_files): evaluate and record one candidate of a study (all conditions, chained).
* `design_get_metrics` (read): metrics of a record, a map table or a variant's last evaluation.
* `design_compare_variants` (read): side by side metrics of records with differences and ranking by objective.
* `design_propose` (read): the next candidates of the study's sampler/optimiser given the store (replay, no side effects).
* `design_record` (write_files): append externally computed metrics for a candidate (so the assistant can run the case through the existing tools and record the result), validating names and finiteness.
* `design_list_study` (read): definition, counts, best feasible record, Pareto front.
Access classes follow `policy::Access`; mutating tools need approval under `Approval::ReadOnly`/`Hook` exactly as the other tools; all paths go through `Service::sandbox_path` (`..` and outside-root absolute paths are `LIMIT` errors; study and
store directories must be inside the sandbox root; names are restricted to `[A-Za-z0-9_.-]` without leading dot); errors are located (`parameter 'angle': ...`).
Numeric/behaviour checks (registered):
* T1 In a read-only server the six write_files tools return `PERMISSION_DENIED` (isError result) and none of the three read tools does; with an approval hook the hook sees exactly the mutating calls; with `allow` lists other tools are refused.
* T2 Path sanitisation: for every path-taking tool the arguments `../x`, `a/../../x` and an absolute path outside the root are rejected with code 1004 and create no file; names with separators or a leading dot are rejected.
* T3 The tools run a propose -> record loop on a Rosenbrock study defined with `design_create_study` (the test plays the evaluator): after 120 recorded candidates of Nelder-Mead the best record has `f <= 1e-3`; `design_list_study` counts and best agree with the store;
  a study created twice with the same inputs and seed proposes identical points; `design_compare_variants` reports exact metric differences.
* T4 `design_evaluate` and `design_run_operating_map` on the cavity template (cheap laminar) return converged rows; a second call with the same arguments returns the stored evaluation without running (evaluation counter unchanged).
* T5 `tools/list` shows each new tool with `_meta."rcfd/needsApproval"` equal to not-read and valid `inputSchema`, descriptions longer than 20 characters (the existing test).

### 4. Documentation and registers
`docs/case-schema.md` (the new key and the design-loop files), `docs/design-loop.md` (new), `docs/mcp-tools.md` (new: the whole MCP/API tool set with the access classes), `docs/service-protocol.md` (method list),
`docs/reference-data.md` (sources, what was and was not read), `docs/roadmap.json` (targeted edits for F-095 note, F-254 and F-257 candidates if they are present; none is created if the ids are absent, the note goes into F-095).
`docs/ROADMAP.md` and `docs/parity-matrix.md` are not edited.

### Process
Commits are local, own hunks only (temporary index). `crates/rcfd-fv/src/lib.rs`, `docs/ROADMAP.md`, `docs/adr/0061-*.md` and the w30c Coles CSV are other agents' leftovers and are not committed.
The final gate runs on a `git archive` of the final commit: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --release --no-fail-fast`.

## Addenda
(none yet)
