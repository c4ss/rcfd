# ADR 0031: Wave 19C, solution transfer and restart (F-115), chained run configurations (F-241), run-control additions (F-121), setup-validation rule engine (F-240)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any code of this wave existed.
Limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and the assertion moves to an
ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned away. A test set-up defect gets a
NEW corrected test and the old one stays as a record. Addenda (file names, mesh sizes not fixed here, measured numbers) may be
appended at the end without touching a registered limit.

Scope. F-115, F-121, F-240 and F-241 are partly delivered or open (F-115 and F-121 are `partial` with a steady-laminar-only
restart, F-240 and F-241 are `todo`). This wave extends them and stops each one honestly at `partial` with named gaps. F-209 and
F-210 are NOT attempted (time). Clean-room: published methods (inverse-distance weighting, weighted least-squares linear
reconstruction, axis-aligned cell-overlap remapping with a linear reconstruction) and first principles only; no vendor code, text,
data or names. Baseline hashes (`crates/rcfd-api/tests/baseline_hashes.rs`) stay bit-identical and are not re-recorded: nothing
in this wave changes a numerical kernel; restart and rules are additive paths that a case without the new keys never enters.
Case-schema additions are additive with explicit unset sentinels (`""`, empty list).

## Decisions

### D1 Shared-file discipline and layout

New modules: `rcfd-post::remap` (field transfer), `rcfd-api::restart` (restart orchestration), `rcfd-api::chain` (chained
configurations), `rcfd-api::rules` (rule engine) with the data file `crates/rcfd-api/rules/setup_rules.json`, and a CLI file
`rcfd-cli/src/cmd_w19c.rs` (subcommands `restart` and `check`). Shared files get small targeted edits only: one `.field(...)` line
each for `restart` and `chain` in `schema.rs`, one `out.extend(...)` each in `case.rs` (static checks of the two new sections), a
few accessors/methods in `sim.rs` (restart save/load for the transient engine and cross-engine restore), one `mod` line in
`lib.rs` of `rcfd-api` and `rcfd-post`, flag handling in `rcfd-cli/src/main.rs`. `rcfd-fv/src/pimple.rs` gets ONE purely additive
block at the end (`PimpleState`, `snapshot`, `restore`, `reinitialise`): the solver history that a bit-exact transient restart needs
is private there. No existing function body of `rcfd-fv` changes. `rcfd-post::interpolate` (F-115 first slice) is not edited; the
modes `gradient` and `conservative` of the new method list call it unchanged.

### D2 Field transfer methods (`rcfd-post::remap`)

`Remapper::new(src: &Mesh, tgt: &Mesh, method)` builds the map once; `scalar(values)`, `vector(values)` apply it. Methods (enum
`RemapMethod`, names used in the case tree and CLI):

* `nearest`: the value of the nearest source cell centre. First order.
* `inverse_distance`: convex combination of the `k = 10` nearest source centres with weights `1/d^2`; a source centre closer than
  `1e-12` times the local spacing returns its value. Bounded by the source extremes. First order.
* `least_squares`: weighted (`1/d^2`) linear least-squares fit over the `k = 10` nearest source centres (constant plus gradient,
  directions in which the neighbours have no spread are dropped, as for a 2D mesh) evaluated at the target centre. Reproduces a
  linear field to rounding; second order; not bounded. A target centre outside the axis-aligned bounding box of the source cell
  centres falls back to `nearest` (the nearest-value fallback).
* `gradient`: the existing `rcfd-post::interpolate` non-conservative mode (nearest source cell plus its least-squares gradient
  times the offset, clipped to the range of the cell and its neighbours). Second order away from extrema.
* `conservative`: the existing mode (`gradient` plus one uniform shift matching the volume average).
* `conservative_overlap`: for meshes whose cells are all axis-aligned boxes (tested per cell: volume equals the bounding-box
  volume to `1e-10` relative and the centre equals the box middle to `1e-10` of the extent), the target value is the overlap
  volume weighted mean of the source reconstruction `s_i + grad_i . (c_ij - x_i)` over the intersections `V_ij` of the target cell
  with the source cells (`c_ij` the centroid of the intersection box, `grad_i` the least-squares gradient of source cell `i`, not
  limited). Because `x_i` is the centroid of the box cell, each source cell contributes exactly `V_i s_i` when fully covered, so the
  volume integral is conserved to rounding for target and source filling the same region; a linear field is reproduced exactly;
  second order. Normalisation is by the covered volume; the uncovered part of a target cell takes the `nearest` value, weighted by
  its volume. Non-box cells give an error (`not all cells are axis-aligned boxes`) and callers fall back to `conservative`.
* Identical meshes (equal `Mesh::content_hash` and equal cell count) short-circuit to a copy for every method: the identity map is
  bit-exact by construction. This is a short-cut, disclosed as such.

The unstructured conservative remap (arbitrary polyhedral overlap) and flux-specific remapping are NOT delivered (named gaps).
"Conservative-aware for fluxes where specified" is covered only as: the face flux of a restarted incompressible run is rebuilt from
the transferred velocity (as in the first slice), and the maximum cell flux imbalance of the rebuilt flux is reported in the
restart report. A divergence-free projection of the transferred flux is a named gap.

### D3 Restart files and restart semantics

A restart file is the existing `rcfd-sim-checkpoint` container (kind and version 1 unchanged, so old files stay readable) with
ADDITIVE content: meta key `engine` (`steady` default when absent, or `transient`), a `mesh` section holding
`rcfd_io::container::mesh_to_bytes` of the run mesh (so a restart onto another mesh needs no source case), and for `transient`:
`u`, `p`, `phi`, the velocity and flux history levels (`old`, `older`, `oldest`) and the step sizes in f64 sections (never as JSON
text), `time`, `step_no`, and the last step record. New methods `Simulation::supports_restart`, `save_restart`, `load_restart`;
the old `supports_checkpoint`/`save_checkpoint`/`load_checkpoint` keep their steady-only contract unchanged (an existing test
asserts that a transient run answers `supports_checkpoint() == false`; that test is kept). The run-control driver uses the new
methods, so transient laminar runs write and continue restart files. Supported transient state: PISO/PIMPLE, laminar, constant
density, no energy, no scalars, no statistics, no frozen flow, no turbulence model; every other combination keeps the "cannot be
restarted" warning (named gap, including the energy-equation history of transient runs).

`Simulation::restart_from(file, method)` (the engine behind `rcfd restart`, `--restart-from` and `restart.from`):

* same mesh hash: exact restore of the writing engine's state; a steady file into a transient run initialises `u`, `p` and the
  flux `phi` from the file with time 0 and a fresh history (the steady-then-transient chain step); a transient file into a steady
  run is an error;
* different mesh: transfer `u`, `p` (and `T` when both runs solve the energy equation) with the chosen method onto the new mesh,
  rebuild the face flux from the velocity, steady iteration counter 0, transient time carried and history fresh;
* the source mesh is the embedded one; an old file without it needs `restart.source_case` / `--source-case`; otherwise an error.

Case-tree section `restart`: `from` (string, `""` unset), `source_case` (string, `""` unset), `method` (enum
`gradient` default, `nearest`, `inverse_distance`, `least_squares`, `conservative`, `conservative_overlap`). CLI:
`rcfd restart <case> --from <restart file> [--mesh <mesh file or polyMesh dir>] [--source-case <case>] [--method m]` plus every
`rcfd run` flag; `--mesh` replaces the mesh of the case by a mesh file (`mesh.source = file`). `rcfd run` also accepts
`--restart-from`, `--source-case`, `--method`. The old `--interpolate-*` flags and `--continue-from` stay (`--continue-from` now also
reads transient files). A case with `restart.from` set starts through `restart_from` in `rcfd run`, in `chain` stages and in
`run.headless`.

### D4 Chained configurations (`chain`)

Section `chain`: `sequence` (list of stage names, run in this order; empty = no chain, the sentinel) and `stages.<name>`:
`set` (list of strings `"dotted.path=<JSON value>"`, applied in order on top of the previous stage's case, so changes
accumulate), `restart` (enum `auto` default = from the previous stage, `previous`, `none`), `method` (transfer method, default
`gradient`), `allow_unconverged` (bool, default false). A stage runs the driver (`runctl::drive`) on a `Simulation` built from its
case; its restart file `<out>/<stage>/restart.rcfd` and results `<out>/<stage>/` are written; the next stage restarts from that
file through `restart_from`, so a mesh change in `set` triggers the transfer. The chain stops at the first stage whose exit code is
not 0 (a steady stage stopped at its cap, exit 5, is accepted when `allow_unconverged`) and returns that code. Status stream:
every event gets a `stage` field; additional events `stage_start` (`stage`, `index`, `of`) and `stage_finish` (`stage`, `exit_code`,
`state`) and a closing `chain_finish`; the format version stays 1 (consumers ignore unknown events; `summarize_stream` is
unchanged and reports the last `finish`). `rcfd run` runs a chain when `chain.sequence` is non-empty (`--no-chain` runs the base
case alone). Static checks (located): unknown stage name in `sequence` (`chain.sequence.<i>`), duplicate names, a stage not
in `sequence` is allowed (info), malformed `set` entry (`chain.stages.<n>.set.<i>`), a `set` path unknown to the schema or a value
of the wrong type (located at that entry). Operating-point sweeps are not part of this (parametric-study item).

### D5 Run-control additions (F-121)

The contract of `docs/run-control.md` is kept. Added: `rcfd run --print-settings` (resolved settings JSON on stdout, exit 0, no
run; exit 3 for an invalid case), `rcfd run --set path=<JSON value>` repeatable (command-line setting overrides applied before
anything else; malformed argument exit 2, unknown path or invalid value exit 3), `rcfd check <case> [--json]` (see D6), the chain
events of D4, and `--restart-from`/`restart`. Not delivered (named gaps, F-121 stays `partial`): MPI/`--hostfile` (not
attempted by design), per-core affinity, command-at-start (arbitrary commands are NOT executed; `--set` is the safe subset),
scheduled commands on iteration/step/time, automatic case-modification strategies, single precision, a control socket.

### D6 Rule engine (F-240)

`rcfd-api::rules` evaluates the data file `setup_rules.json` (`{"format": "rcfd-setup-rules", "version": 1, "rules": [...]}`)
against the resolved settings JSON (`case.tree.to_json()`, so defaults are visible) and, when a mesh is available, a `mesh`
context taken from the serialised `QualityReport`. A rule: `id` (`SVnnn`), `name`, `severity` (`error` | `warning` | `info`),
`category` (`mesh` | `boundaries` | `materials` | `models` | `solver` | `output` | `setup`), optional `each` (`{"over": path}` iterates the
named entries or list items at `path`, binding `{name}` and the relative prefix `@`), `when` (a condition), `path` (the located
path, may contain `{name}`), `message`, optional `fix` (`{"path", "value"}`, a suggestion, never applied). Condition grammar:
`{"all": [..]}`, `{"any": [..]}`, `{"not": c}`, `{"path": P, "op": OP, "value": V}` and `{"count": P, "op": OP, "value": N}`
where `P` may be `@.key` inside `each`, `$mesh.key` for the mesh context (a rule using it is skipped, not failed, when no mesh is
given), and `V` may be `{"path": P2}`. Operators: `eq ne gt ge lt le in not_in vec_zero vec_nonzero is_empty non_empty name_in`
(`name_in`: the value at `P` is a name of the collection at the path in `value`). A path that does not exist makes its condition
false. Output: `RuleDiagnostic {id, severity, category, path, message, fix?}` sorted by (severity, id, path), serialised as JSON
objects with exactly those keys. Evaluation is pure and deterministic.

The rules are DATA, not code. This wave registers these 21 rules (their ids, severities, conditions and thresholds are fixed
here; the mesh thresholds are the library's own `check_mesh` thresholds, 70/85 degrees, 4, 1000, 100):

| id | sev | category | fires when | located path |
| --- | --- | --- | --- | --- |
| SV001 | error | models | `count(solids) >= 1` and `solver.frozen_flow` false | `solver.frozen_flow` |
| SV002 | error | models | `count(solids) >= 1` and `energy.enabled` false | `energy.enabled` |
| SV003 | error | materials | `fluid.density` in {ideal_gas, real_gas}, a report of type `heat_flow` with `conductivity > 0` | `reports.<n>.conductivity` |
| SV004 | error | models | `turbulence.model` in {k_epsilon, k_omega_sst}, `k_initial == 0`, `initial.velocity` zero, every `boundaries.*.velocity_value` zero and every `boundaries.*.bc_kind` equal to `none` (a catalogue kind may derive its speed from a flow rate) | `turbulence.k_initial` |
| SV005 | error | solver | algorithm in {piso, pimple}, `end_time > 0`, `dt > end_time` | `solver.dt` |
| SV006 | error | solver | algorithm in {piso, pimple, compressible}, `bulk_velocity != 0` | `solver.bulk_velocity` |
| SV007 | error | solver | algorithm in {piso, pimple, compressible}, `physics.body_force` not zero | `physics.body_force` |
| SV008 | error | solver | `decomposition.method == none` and `decomposition.parts > 1` | `decomposition.method` |
| SV009 | error | setup | an item of `chain.sequence` that is not a name in `chain.stages` | `chain.sequence.<i>` |
| SV010 | error | output | `statistics.enabled` and algorithm in {simple, simplec, compressible} | `statistics.enabled` |
| SV011 | error | mesh | `$mesh.max_non_orthogonality > 85` | `mesh` |
| SV012 | error | models | a report of type `y_plus` and `turbulence.model == laminar` | `reports.<n>.type` |
| SV013 | error | boundaries | a boundary with `turbulence != auto` and `turbulence.model == laminar` | `boundaries.<n>.turbulence` |
| SV101 | warning | mesh | `70 < $mesh.max_non_orthogonality <= 85` | `mesh` |
| SV102 | warning | mesh | `$mesh.max_skewness > 4` | `mesh` |
| SV103 | warning | mesh | `$mesh.max_aspect_ratio > 1000` | `mesh` |
| SV104 | warning | mesh | `$mesh.max_volume_ratio > 100` | `mesh` |
| SV105 | warning | solver | algorithm in {simple, simplec} and (`tolerance_u > 1e-2` or `tolerance_p > 1e-2`) | `solver.tolerance_u` / `solver.tolerance_p` (the first that applies) |
| SV201 | info | solver | `decomposition.method != none` ("the solver runs in one process and ignores the decomposition") | `decomposition.method` |
| SV202 | info | boundaries | `count(boundaries) == 0` | `boundaries` |
| SV203 | info | materials | a report with `conductivity > 0` whose type is not `heat_flow` | `reports.<n>.conductivity` |

SV001-SV003, SV005-SV008, SV010-SV013 restate checks that the engines/loaders already enforce elsewhere; they are mirrors
(one data-driven place, categorised, with stable ids, evaluable without building the simulation). SV004 restates the
`turb.rs` derivation of the initial k. The genuinely new diagnostics are SV009, SV101-SV105 and SV201-SV203. Wiring: `rcfd
check <case>` (settings, simulation build, mesh quality, then the rules; exit 0 when no error-level diagnostic, 3 with one,
4 when the mesh fails its quality check; `--json` prints `{ok, exit_code, diagnostics, rules, mesh}`), `rcfd validate` prints
rule diagnostics as additional lines (warnings and infos do not change its exit code), `rcfd run` evaluates the rules after the
simulation is built (an error-level diagnostic exits 3 before solving; warnings go to stderr and as `warning` status events
with the rule id), `run.check`/`--check-only` adds a `rules` array to the `check` event (the existing `diagnostics` key keeps its
meaning). Registry case descriptors (`cases/*/case.json`) are not setup files (they describe meshes and references informally
and fail the loader today); "the benchmark registry cases" are therefore evaluated as the TOML setups under `cases/`.

## Registered checks

Notation: relative L2 error is `sqrt(sum V (a-b)^2 / sum V b^2)`. All meshes are the generators named; 2D meshes have the
generator depth in z, the same for source and target of a pair.

### Field transfer (`crates/rcfd-post/tests/remap_w19c.rs`)

* **I1 linear exactness.** `f = 0.7 + 1.3 x - 0.9 y` (2D) and `+ 0.4 z` (3D). 2D: source `cavity_2d_rect(13, 9, 1.3, 0.9)`,
  target `cavity_2d_rect(17, 11, 1.3, 0.9)`, and the reverse direction. 3D: source `channel([5,4,3],[1.0,0.8,0.6])`, target
  `channel([7,5,4],[1.0,0.8,0.6])` (uniform grading, as the generator call allows). Methods `least_squares`, `conservative_overlap`
  (point-value source for `least_squares`, volume-average source for the overlap method: the linear field's cell average equals its
  centre value on a box cell) must give max abs error `<= 1e-12` at ALL target cells; method `gradient` `<= 1e-12` at target cells
  whose centre lies inside the rectangle (box) spanned by the source cell centres.
* **I2 convergence order.** `f = x^2 + 0.5 x y + y^2` on the unit square, sources `cavity_2d_rect(n, n, 1, 1)` with
  `n = 8, 16, 32`, one fixed target `cavity_2d_rect(41, 37, 1, 1)` (non-matching). Point-value methods (`nearest`,
  `inverse_distance`, `least_squares`, `gradient`): source values `f(x_i)`, reference `f(x_t)`. `conservative_overlap`: source values
  are the exact cell averages of `f`, reference the exact target cell averages. Error: relative L2 over the target. Observed order
  `p = log2(e_n / e_2n)` for `8->16` and `16->32` must both lie in `[1.7, 2.5]` for `least_squares`, `gradient` and
  `conservative_overlap`, and in `[0.7, 1.5]` for `nearest` and `inverse_distance`.
* **I3 conservation.** `conservative_overlap`, `f = 1 + sin(3x) cos(2y) + (x > 0.5 ? 0.5 : 0)` (discontinuity on purpose) and the
  vector `(f, -f)` component-wise, `cavity_2d_rect` pairs 10x7 -> 23x17 (refine) and 23x17 -> 10x7 (coarsen) on the unit square:
  `|sum V_t f_t - sum V_s f_s| / sum V_s |f_s| <= 1e-12`. Also the existing `conservative` mode (uniform shift): relative integral
  mismatch `<= 1e-10` for the same fields (re-registered from the first F-115 slice).
* **I4 identity.** Same mesh (two independent builds of `cavity_2d_rect(12, 9, ...)`) and `RemapMethod` of every kind: output equals
  the input bit for bit (`to_bits`), for a scalar and for a vector field.
* **I5 fallback and boundedness.** Target `cavity_2d_rect(20, 20, 1.2, 1.2)` over source `cavity_2d_rect(12, 12, 1, 1)` (the target
  extends beyond the source): every method returns finite values at every target cell; `nearest`, `inverse_distance` and
  `gradient` stay within `[min, max]` of the source field; `least_squares` at target centres outside the source cell-centre
  bounding box equals the `nearest` value exactly; `conservative_overlap` of a constant field `c = 3.25` reproduces `c` to
  `1e-14` relative at every target cell, including the partially covered ones; the overlap method reports the covered fraction
  (`< 1` for the extended cells).
* **I6 refusal.** `conservative_overlap` on a non-box mesh (`cylinder_ogrid`) returns an error whose text contains
  `axis-aligned`; a source field of the wrong length is an error.

### Restart (`crates/rcfd-api/tests/restart_w19c.rs`)

All on the lid-driven cavity of the existing run-control tests (12x12, `nu = 0.01`, `lid` velocity 1, upwind) unless stated.

* **S1 steady same-mesh identity.** `restart_from` of a steady file on the same mesh equals `load_checkpoint`: `ux`, `uy`, `p` and
  the flux after restore are bit-identical; after 25 further iterations bit-identical to the uninterrupted run (max abs
  difference exactly 0).
* **S2 transient restart equals continued run.** PISO + implicit Euler, PISO + BDF2, and PIMPLE (`n_outer = 2`) with BDF2: run
  `N1 = 8` steps, `save_restart`, run `N2 = 12` more; a fresh simulation `load_restart`-ed from the file and stepped 12 times has
  bit-identical `ux`, `uy`, `p`, flux and time (max abs difference exactly 0, time equal). Also a run-control driver run stopped
  with a step limit that writes the transient restart file and a second driver run `--continue-from` it.
* **S3 cross-engine.** A converged steady file loaded with `restart_from` into the PISO simulation of the same mesh gives `u`, `p`
  and the flux bit-identical to the file, time 0.
* **S4 embedded mesh.** A file written on a 12x12 mesh restarts a 24x24 simulation without a source case; a v1-style file
  (embedded mesh removed) needs `source_case` and gives the same result as the embedded one (bit-identical fields); without
  either it is an error; a file of another mesh restarted with the same hash check is refused when the transfer is not asked
  for.
* **S5 coarse-to-fine iterations (reported, pass/fail as registered).** Cold start: uniform 24x24 cavity, `simplec`, tolerances
  `1e-6`, count `n_cold` = iterations until done. Source: 12x12 converged to `1e-7`. For each of `gradient`, `least_squares`,
  `conservative_overlap`: (a) **S5a** `n_interp < n_cold` (iterations to the same tolerance); (b) **S5b** the number of iterations
  until the relative L2 distance of the speed from the converged 24x24 solution (reference run to `1e-10`) first falls below
  `1e-2` is strictly smaller than the same count for the cold start. Both are registered as stated; the first F-115 slice measured
  103 vs 89 iterations for (a) with `gradient`, so a miss of S5a is the expected outcome for that method and is reported as such.

### Chain (`crates/rcfd-api/tests/chain_w19c.rs`, `crates/rcfd-cli/tests/chain_w19c.rs`)

* **C1 chain equals manual procedure, bit for bit.** Base: cavity 16x16, `simplec`, tolerances `1e-8`; stage `steady` (no `set`)
  then stage `transient` with `set = ["solver.algorithm=\"piso\"", "solver.dt=0.05", "solver.end_time=1.0"]`. The chain's final
  `ux`, `uy`, `p`, flux and time equal, bit for bit, those of the manual procedure (run the steady case to its restart file;
  build the transient case; `restart_from`; drive). At the CLI: `rcfd run chain.toml` writes `<out>/transient/solution.vtu`, which
  is byte-identical to the one written by `rcfd run steady.toml` followed by `rcfd restart transient.toml --from <steady
  restart.rcfd>`.
* **C2 F-241 acceptance.** The same chain's final speed field has relative L2 distance `<= 2e-2` from the single steady run (the
  steady solution is a fixed point of the time-stepping up to the steady tolerance and the transient Rhie-Chow difference), and the
  same for the pressure after removing its volume mean.
* **C3 mesh change between stages.** Chain `coarse` (12x12 steady) then `fine` (`set = ["mesh.nx=24","mesh.ny=24"]`, steady): the
  fine stage converges with exit code 0 and 24x24 cells, and its `restart` report says `transferred`.
* **C4 failure propagation.** Stage 1 with `solver.max_iterations=3` and `allow_unconverged` false: chain exit code 5, stage 2
  never started (no `stage_start` for it, no output directory).
* **C5 events.** For C1: events in the order `stage_start(steady)`, ..., `stage_finish(steady)`, `stage_start(transient)`, ...,
  `stage_finish(transient)`, `chain_finish`; every other event carries `stage`; `chain_finish.exit_code == 0`.
* **C6 static checks.** A `sequence` naming a stage that does not exist, a malformed `set` entry (no `=`), an unknown path in `set`
  and a value of the wrong type are each rejected with a diagnostic at the located path (`chain.sequence.1`,
  `chain.stages.<n>.set.<i>`).

### Run control (`crates/rcfd-cli/tests/w19c_run_control.rs`)

* **R1 exit codes through the binary.** Existing tests cover 0, 1, 2, 3, 5, 7, 8. This wave adds 4 and 6 end to end. Candidate for
  4: `wedge_2d` with `ramp_angle = 89`. Candidates for 6: (i) simple with `alpha_u = 1.0`, `alpha_p = 1.0`, lid speed 1000, `nu =
  1e-4`, central convection; (ii) PISO with `dt = 100`, `end_time = 1000`, central convection. If neither candidate produces 6 (or
  the wedge candidate 4) the check is reported as NOT REPRODUCED through the binary and left to the library tests, not
  re-tried with other values.
* **R2 print-settings.** `rcfd run case --print-settings` exits 0 without running (no results directory is created) and its stdout
  JSON equals `Case::tree.to_json()`; an invalid case exits 3.
* **R3 `--set`.** `--set solver.max_iterations=7 --set mesh.nx=10` gives the same `case_hash` in the `start` event as a case file
  edited by hand; a malformed argument (`--set foo`) exits 2; an unknown path exits 3.
* **R4 `rcfd check`.** Exit 0 on a valid case and on one with only warnings, 3 on an error-level rule, 4 on a bad mesh; the
  `--json` output has the top-level keys `ok exit_code diagnostics rules mesh` and every entry of `rules` has exactly the keys
  `id severity category path message` (plus `fix` where the rule has one).
* **R5 thread independence of the new paths.** `rcfd restart ... --threads 1` and `--threads 3` write byte-identical
  `solution.vtu` files (coarse-to-fine restart and a chain).

### Rule engine (`crates/rcfd-api/tests/rules_w19c.rs`)

* **E1 each rule fires.** For each of the 21 registered rules a crafted case (a minimal valid base plus the stated mutation) makes
  the engine return a diagnostic with exactly that rule id, severity, category and the located path of the table (with the
  entry name filled in); and no other error-level rule fires on that case (SV004 and SV011 may co-fire with nothing else; the
  assertion is on the set of error ids). A completeness assertion: the fixtures cover exactly the ids in the data file, and the
  data file has at least 14 rules with unique ids.
* **E2 zero false positives.** The engine (with the mesh context) returns NO diagnostic of severity error or warning (info
  allowed and listed) for: every `cases/**/*.toml` setup file; every fenced `toml` block of `docs/case-schema.md` and
  `docs/run-control.md` that parses as a case; every case text extracted from the sources of `crates/*/tests/*.rs` (raw string
  literals and `"..."` literals that parse as a case and pass `semantic_errors`; format templates with `{}` placeholders are
  skipped and counted). At least 30 distinct case texts must be evaluated in total; if fewer, the check is reported as
  under-powered, not passed. Counts of evaluated and skipped texts are reported.
* **E3 determinism.** The diagnostics of each crafted case serialise to identical bytes in two evaluations and after the case is
  rebuilt from its JSON with the keys re-inserted in reverse order.
* **E4 wiring.** `Case`-level: `run.check`/`check_only` adds the `rules` array; `rcfd check` and `rcfd run` behave as in D6 (exit
  codes 0/3/4; a rule-error case never starts solving; a warning-only case runs and emits a `warning` status event naming the
  rule id). The data file is parsed with strict validation: unknown condition keys, unknown operators, unknown severities or
  categories are load errors tested by a deliberately broken rules text.

### Baseline

* **B1.** `cargo test -p rcfd-api --test baseline_hashes` passes unchanged (no re-recording).

## Anticipated deviations and limits (declared before running)

* `conservative_overlap` is for axis-aligned box cells only; unstructured conservative remapping and flux remapping are open.
* Transient restart covers laminar constant-density PISO/PIMPLE without energy, scalars, statistics or frozen flow.
* A restart across different meshes loses the BDF2 history (fresh history); only same-mesh restarts are exact.
* A solution can only be read back from a restart file (not from VTU/result files).
* The identity check relies on a hash short-cut; the general path is not asserted for identical meshes.
* F-121: no MPI, affinity, scheduled commands or control socket; `--set` is the only command-at-start analogue.
* F-240: SV001-SV003, SV005-SV008, SV010-SV013 duplicate checks already enforced elsewhere; the only new diagnostics are SV009,
  SV101-SV105, SV201-SV203. Registry `case.json` descriptors are not evaluated.
* F-209 and F-210 are not touched.
