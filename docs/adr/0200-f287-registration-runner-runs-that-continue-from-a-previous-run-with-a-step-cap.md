# ADR 0200: F-287 registration: runner runs that continue from a previous run, with a step cap

Status: **REGISTERED 2026-11-06**, written before any code of F-287. Not yet accepted by the maintainer. Amendment 1 corrects
the start iteration of a continued run (its `start` event says 0) before any further code. Addendum 1 reports S1 to S12 built as
registered on branch `f287-f288-runner`, not merged: a run continued after 25 steps ends where a run of 50 steps does, bit for
bit; on the 16 x 16 cavity a chained study and an operating map on the runner agree with the in-process chain to 6.7e-10 relative
(limit 1e-4), the cold and step-capped runs bit for bit. The gate at `56b194b` gave 2934 passed, 3 failed (the flakes
`runner_w47b` s1 and s9, and the race control `edit_lock_w48f`, which passed alone), 823 ignored. The two new `settings` keys
wait for the lead's approval (Q1).

Parents: [ADR 0189](0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md) (F-274, its decision
record of 2026-11-06: Q2 (d) and Q3 (d)), [ADR 0196](0196-three-code-changes-decided-2026-11-06-f246-r2-record-design-tool-class-sv202-text.md)
(B, the class `execute` of the design tools under `--project`), [ADR 0123](0123-wave47b-run-service.md) (the run service),
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md) (the run record), [ADR 0067](0067-wave32c-design-loop.md) (the design
loop, check M3), [ADR 0031](0031-wave19c-restart-chain.md) (restart files). Roadmap item F-287. F-288 (adopting runs after a
server restart) is registered separately in [ADR 0201](0201-f288-registration-adopt-matching-runs-after-an-mcp-server-restart.md)
and built on the same branch, `f287-f288-runner`.

Read at main 9edfbda. Number: 0197 is the highest on main; the branch `f285-warped-kernel` holds 0198 and `f289-f290` holds 0202;
0200 and 0201 were given to this item and F-288.

House rules (ADR 0130, ADR 0196): defaults stay bit-identical (no solver result, case hash or byte of an existing case changes);
`crates/rcfd-api/tests/baseline_hashes.rs` and `case_hashes_w48u.rs` are not edited and nothing is re-recorded; no case-schema
key is added; a golden is never re-recorded and an edit of an existing test is a registered edit named here; a missed limit
becomes an ignored `*_known_miss` test with its numbers, never tuned; no new third-party crate, `Cargo.lock` unchanged; no
commit trailers. A new run-record field is additive, optional and written only when set, and needs the lead's approval under
ADR 0130: the two of S1 are listed for it (Q1).

## What F-287 asks

The roadmap entry: a run setting that the runner passes to `rcfd run --continue-from` (a change to `RunSettings` and the launch;
ADR 0189 Q2 (d)) and a run setting for the step cap (ADR 0189 Q3 (d)), so that in project mode a chained design study with several
conditions, a study with its own `max_steps` and an operating map (`design_run_operating_map`) are evaluated on the runner instead
of in the server process; their records match the in-process evaluator for the same cases within the tolerance fixed here, and a
run without the new settings keeps the bytes of its `run.json` and its results.

## What the code read shows (main 9edfbda, nothing run)

1. **The launch.** The runner starts `rcfd run <snapshot> --case-dir <dir>` with one `--set` per entry (`solver_cmd`,
   `crates/rcfd-runner/src/runner.rs` line 597) and adds in `launch` (line 650) `--out`, `--status-file`, `--status-every 1`,
   `--quiet`, `--threads`, `--stop-file`, `--pause-file`, `--store-dir` and `--status-monitors`. The pre-launch check
   (`estimate`, line 491) runs `solver_cmd` with `--check-only`. Nothing passes `--continue-from` or a step cap.
2. **`rcfd run --continue-from f`** (`crates/rcfd-cli/src/main.rs` line 526) calls `Simulation::load_restart` on the freshly built
   simulation. For a steady file this is `load_checkpoint` (`crates/rcfd-api/src/sim.rs` line 4742): the file's mesh hash must be
   the case mesh's, its energy flag the case's, no turbulence model, the steady SIMPLE/SIMPLEC engine; velocity, pressure, the face
   flux and the iteration counter are restored. `docs/run-control.md`: "the iteration counter and the iteration cap continue from
   the checkpoint". A load failure is `error: --continue-from <f>: ...` on stderr and exit 1, before the `start` event. The `start`
   event carries `start_iteration` and `info.continue_from` (`runctl.rs` line 281, `main.rs` line 582). `restart.from` together
   with `--continue-from` is exit 2.
3. **The restart file a run leaves.** `drive` writes the checkpoint (`<out>/restart.rcfd` by default, so `runs/<id>/output/
   restart.rcfd` under the runner) at the end of a `converged`, `finished`, `not_converged`, `step_limit`, `interrupted` or
   `time_limit` run when `supports_restart` holds: steady SIMPLE/SIMPLEC without turbulence (with the temperature when energy is
   on), or transient laminar PISO/PIMPLE without other transported state (`sim/restart.rs` lines 99 and 106). Its `meta` section
   names `engine` (`steady` when absent, or `transient`), `iteration`, `mesh_hash` and `energy`.
4. **No step cap in `rcfd run`.** `--max-iterations n` sets `solver.max_iterations`, a case setting counted on the iteration
   counter (so for a continued run it caps the total). The drive's own cap, `RunOptions::max_steps` (steps of this launch; state
   `step_limit`, exit 0, the checkpoint written), is what the in-process evaluator uses; `rcfd run` passes `None` (`main.rs` line
   570).
5. **The in-process chain.** `CaseEvaluator::evaluate` (`design/case_eval.rs` line 256) starts condition k from condition k-1's
   fields when k-1 was usable: `run_case` with a seed, which is `Simulation::seed_fields` (`sim.rs` line 4797): velocity and
   pressure are set, the face flux is rebuilt from the velocity, the iteration counter is 0. It is refused (the run is cold, with
   the note `cold start: <why>`) for energy or turbulence, an engine other than steady SIMPLE/SIMPLEC, or fields of another size; a
   transient run carries no fields (`fields` only when `exit 0` and `is_steady`). So a condition continued on the runner starts from
   the previous converged state with that run's face flux and iteration counter, a condition seeded in process from the same
   velocity and pressure with a rebuilt flux and a counter at 0: the two do not start from the same state, and their records
   cannot be expected to agree bit for bit. Both stop on the same tolerances of the same discrete equations.
6. **Prior evidence on differently started runs.** ADR 0067 M3 (`docs/design-loop.md`): on a laminar channel at tolerances 1e-6, a
   chain of warm starts and eight cold starts gave pressure drops that agree to 2e-8 relative (limit 5e-4).
7. **The in-process step cap.** `Loaded::limits` (`design/tools.rs` line 517): `max_steps = min(study.max_steps or
   max_steps_per_call, max_steps_per_call)` (`max_steps_per_call` 10 000 000 by default); `run_map` the same with the call's
   `max_steps` (line 922).
8. **The in-process map.** `run_map` (`design/tools.rs` line 879) builds its base case (a study's candidate at a condition, or the
   file `base`), applies `overrides` with `apply_value`, then runs `opmap::run_and_write` with `CaseMapEvaluator` behind `Counting`:
   each point warm from the previous converged point (`seed_fields`), a failed warm point repeated cold (`retry_cold`), the table
   written after every point. A repeated identical call returns the stored table (`cached`) without running.
9. **F-274 as merged.** `RunnerSource::runner_folder` (`crates/rcfd-workspace/src/design.rs` line 334) sends a chained study with
   two or more conditions and a study with `max_steps` to the server process with an `evaluator_note`; `design.run_map` goes to
   `svc.call` (`crates/rcfd-mcp/src/lib.rs` line 948 routes only `design::ON_RUNNER`). `crates/rcfd-mcp/tests/design_runner_f274.rs`
   C4-C (ii) and (iii) assert `evaluator` `process` for those two studies.
10. **`RunSettings`** (`crates/rcfd-runner/src/records.rs` line 19): `priority`, `threads`, `mem_mb`, `set`. It is built with every
    field named in `crates/rcfd-studio/src/runs.rs` line 458 and `crates/rcfd-runner/src/main.rs` line 103, so a new field breaks
    the Studio's build unless that Studio file changes. The Studio's files wait for the maintainer's worktree `studio-case-lineage`,
    whose uncommitted work also changes `RunSettings` (`threads` optional, a `device` field), the runner's launch and
    `crates/rcfd-studio/src/runs.rs`. serde_json is built without `preserve_order`, so the keys of `settings` are written sorted.
11. **The access class.** `design_run_operating_map` is `write_files` with and without `--project`; ADR 0196 B made
    `design_run_study` and `design_evaluate` `execute` under `--project` because there they start runs (`PROJECT_EXECUTE`,
    `crates/rcfd-mcp/src/lib.rs` line 62), checked by `tool_list_f275.rs` (`EXECUTE`, B1, C5-L) and `project_list_f270.rs` (A2).

## Choices of this registration

**S1. Two run settings (Q1 (a), Q3 (a)).** `run.json` `settings` gains two optional keys, each written only when set:
`continue_from`, the id of a run of the same project whose result the run starts from, and `max_steps`, a positive integer, the
most steps of the run's launch. They are the fields of a new type `rcfd_runner::records::LaunchSettings` (`continue_from:
Option<String>`, `max_steps: Option<u64>`), read beside `RunSettings` from the same object (`LaunchSettings::of(&RunRecord)`), so
`RunSettings` and every struct literal of it stay as they are (item 10); the JSON is the same as if they were fields of
`RunSettings`. A `run.json` without them reads and writes as before, byte for byte.

**S2. The submission.** `rcfd_runner::client::Client::submit_with(case, &RunSettings, &LaunchSettings, &RunTags)` writes the run
with the launch settings merged into `settings`; `submit_tagged` is `submit_with` with no launch settings, so it and `submit` write
what they write today. `submit_with` refuses, with no run folder made: a `continue_from` that is not a run id
(`rcfd_project::runid::is_run_id`) or names no run folder of the project, and `max_steps` 0. `FsStore::create_run_with` holds the
body of `create_run_tagged` (which becomes a call of it). `rcfd_workspace::runs::RunService::submit_with` is a pass-through; like
`submit_tagged` it writes no journal line (ADR 0189 S10, Q6 not asked).

**S3. The launch.** With `max_steps` the runner adds `--max-steps <n>`; with `continue_from` it adds `--continue-from
<project>/runs/<id>/output/restart.rcfd`, both only to the launch command (not to the pre-launch check). Before it launches a run
with `continue_from`, the named run must have ended (finished, failed or cancelled) and its `output/restart.rcfd` must exist;
otherwise the run ends `failed` before launch with a reason naming the run and the file (Q2 (a)). A run without the two keys is
launched with the command of today.

**S4. `rcfd run --max-steps <n>`.** A new option of `rcfd run`: the drive's `RunOptions::max_steps = n`, the steps of this launch
(a continued run counts from its restart), state `step_limit` and exit 0 when reached, the checkpoint written as for any
`step_limit`. `n` must be at least 1 (exit 2), and it is refused with `chain.sequence` (exit 2: a chain runs its own stages; use
`--no-chain`). The `start` event's `info` gains `max_steps` only when the option is given. Without it every output of `rcfd run`
is unchanged.

**S5. Which studies the runner evaluates.** `RunnerSource` chooses the runner when the base case file and the study folder lie in
the project, as before; a chained study with several conditions and a study with its own `max_steps` now go to the runner too
(item 9 is replaced). With `max_steps` set, each run of the study gets `max_steps` = the in-process cap of item 7,
`min(max_steps, max_steps_per_call)`; without it, none (as in F-274: the server's `max_steps_per_call` does not bound runner runs).

**S6. Chained conditions.** With `chain` and two or more conditions, the conditions of a candidate run one after another: the run
of condition k is queued after the run of condition k-1 has ended. It continues from that run (`continue_from` its id) when
(a) condition k-1 is usable (`ok`), (b) that run's `output/restart.rcfd` exists and its `meta` names the engine `steady` (or none),
`energy` false, and (c) its `mesh_hash` is the hash of the mesh of condition k's case (`rcfd_api::sim::build_mesh`, in the
server). Otherwise the run is cold; the condition's message gets `cold start: <why>` except when condition k-1 was not usable, or
left no restart file of the steady engine because its run was transient (in process neither has a note). When a continued run
ends without a solver result and its reason names `--continue-from` (`rcfd run` could not load the file: an engine or turbulence
model of condition k's case that the steady file does not fit), the condition is run once more, cold, with `cold start: <reason>`.
Without `chain`, or with one condition, the conditions are queued together as in F-274.

**S7. The iterations of a continued run.** The record's `iterations` is `solver_finish.iterations` minus the `start_iteration` of
the run's `start` event (in `runs/<id>/solver-status.jsonl`; 0 when there is none), so it counts the steps of that run as in
process. Because `--continue-from` continues the iteration cap (item 2), a continued run has `max_iterations - start_iteration`
iterations left where the run in process has `max_iterations` (Q6 (a)); a continued condition that ends `not_converged` gets the
note `the run continued at iteration <n> (--continue-from keeps the iteration cap)`.

**S8. Operating maps.** `rcfd_api::design::tools::call_with` also takes `design.run_map`: `run_map_with(svc, p, source)` asks the
source for a point evaluator (`EvaluatorSource::map_evaluator(&MapRun) -> Rpc<Option<Box<dyn PointEvaluator<State = String>>>>`,
a default method returning `None`, so other sources need no change) after the map's cached table was looked for and before it
runs; `None` is the in-process `CaseMapEvaluator`, as today. `MapRun` gives the base case file, the settings applied to it before
each point's own (a study candidate's parameter targets and condition, then `overrides`, in that order), the base case as built in
process, the call's own `max_steps` and the output folder. The workspace's map evaluator answers when the base case file and
`out_dir` lie in the project: each point is one run (untagged, Q4 (a)) of the base case file with the entries of S4 of ADR 0189
(the prefix, then the point's settings), checked to give the settings tree of the in-process point case before it is queued;
a point continues from the previous converged point's run by the rule of S6 (`warm_applied` then true), else runs cold; its
result is read from `metrics.json` by the in-process status rule (usable when `converged`, metrics only then), `iterations` by
S7. The step cap is the call's `max_steps` (min `max_steps_per_call`) when given, else none. `opmap`, the table files and the
cache are unchanged. The workspace's `ON_RUNNER` gains `design.run_map`, so `rcfd-mcp` routes `design_run_operating_map` to
`Workspace::design_call` under `--project`.

**S9. The class of `design_run_operating_map` (Q5 (a)).** Under `--project` it now starts runs, so it joins `PROJECT_EXECUTE` and
has the class `execute`, the reason of ADR 0196 B; without `--project` it stays `write_files` and the tool list is unchanged.

**S10. The result.** `design_run_study`, `design_evaluate` and `design_run_operating_map` keep every key. With the runner they add
`evaluator`, `runs` and `runs_total` as in ADR 0189 S7 (for a map the `variant` and `condition` of a run object are absent and
`point` is its index); a run object of a continued run adds `continue_from` (the run it continued from). With the server process
they add `evaluator` `process` and `evaluator_note`. A cached map is returned as before, without these keys.

**S11. Where the code goes.** `rcfd-runner`: `records.rs` (`LaunchSettings`), `store.rs` (`create_run_with`), `client.rs`
(`submit_with`), `runner.rs` (the launch and its check). `rcfd-cli`: `main.rs` (`--max-steps`, usage text). `rcfd-api`:
`design/tools.rs` (`MapRun`, `map_evaluator`, `run_map_with`, `call_with`). `rcfd-workspace`: `design.rs` (the source, the
evaluators, the start rule), `runs.rs` (`submit_with`). `rcfd-mcp`: `lib.rs` (`PROJECT_EXECUTE`). No Studio file, no MCP
descriptor text, no `initialize` text and no case-schema key change; `Cargo.lock` does not change.

**S12. Separable commits.** (1) this text and ADR 0201; (2) the runner, the CLI and their tests; (3) the `rcfd-api` seam; (4) the
workspace, the MCP route and class, the tests and the registered edits; (5) F-288; (6) documents; (7) the addenda.

## Open questions

**Q1. Where the two settings live (item 10).** (a) A type beside `RunSettings` read from the same `settings` object (as built: no
Studio file changes, no conflict with the maintainer's changes to `RunSettings`); (b) two fields of `RunSettings` (the Studio's
struct literal must name them, so after the maintainer's worktree lands). The JSON is the same either way. The two keys are new
run-record fields and need the lead's approval (ADR 0130). Recommendation (a) now, folded into `RunSettings` once the worktree has
landed if wanted.

**Q2. A run whose source run has not ended.** (a) It ends `failed` before launch, with the reason (as built; the design evaluators
queue a continued run only after its source ended); (b) it waits in the queue until the source ends (the strict queue order could
then hold other runs behind it). Recommendation (a).

**Q3. What `continue_from` names.** (a) A run id of the project (as built: the record says which run it continued, the runner
finds the file); (b) a restart file path. Recommendation (a).

**Q4. Tags of a map's runs.** (a) None (as built): `study`, `variant` and `condition` describe a study's runs, approved as such;
(b) the map's folder as `study` and the point label as `condition` (a second meaning of approved fields). Recommendation (a).

**Q5. The class of `design_run_operating_map` under `--project` (item 11).** (a) `execute` (as built, the reason of ADR 0196 B,
with the registered edits E1 and E2); (b) `write_files` (a policy that allows it would let it start runs). Recommendation (a).

**Q6. The iteration cap of a continued run (S7).** (a) As `--continue-from` defines it, with the note (as built; the run's
settings stay the candidate's exactly); (b) a continued run also gets the entry `solver.max_iterations = start_iteration +
max_iterations` (the run's case then differs from the candidate's in that setting, and S4 of ADR 0189 needs an exception).
Recommendation (a); (b) if real studies hit the cap.

**Q7. A chained condition whose mesh differs but has the same cell count.** In process it is seeded with the other mesh's fields
(item 5: `set_fields` checks the size only); on the runner it runs cold (S6 (c)). (a) As built, documented; (b) send such studies
to the server process. Recommendation (a).

## Acceptance checks and limits (fixed here, before any code)

The study cases are those of ADR 0189: the built-in `lid_driven_cavity` template (steady SIMPLEC, upwind, tolerances 1e-6, the
`lid_drag` report) at 16 x 16 cells with `max_iterations = 3000` (`cases/small.toml`) and at 32 x 32 (`cases/cavity.toml`). Two
limits on a metric, as `|a - b| <= tol * max(|a|, |b|)`:

* `REL_EXACT = 1e-12`, for a run that starts as the in-process one does (cold): ADR 0189 C4-M's limit, met there with 0.
* `REL_CHAIN = 1e-4`, for a run continued on the runner against the same condition seeded in process (item 5): both converge to
  the same discrete solution and stop at normalised residuals of 1e-6. An estimate of the distance of each from that solution is the
  residual over one minus the contraction per iteration; the cold cavity runs of ADR 0189 fall from 1 to 1e-6 in about 100
  iterations, a factor of about 0.87 per iteration, so about 8e-6 each and 2e-5 between them. The limit is five times that, and
  5000 times the agreement ADR 0067 M3 measured between differently started chains (item 6). The iterations of a continued
  condition are recorded with no limit (the two start from different face fluxes); S7's arithmetic is checked instead.

* **R-1 (the runner; `rcfd-runner` test, release).** A project with the 12 x 12 cavity of `runner_w47b.rs` (tolerances 1e-14, so
  only a cap ends it). Run A `max_steps` 25; run B `continue_from` A, `max_steps` 25; run C `max_steps` 50. Pass: A and C end
  `finished` with `solver_finish.state` `step_limit`, A after steps 1 to 25, C after 1 to 50; B's `start` event has
  `start_iteration` 25 and `info.continue_from` the path of A's `output/restart.rcfd`, its residual steps are 26 to 50, and its line
  of step 50 in `residuals.jsonl` equals C's line of step 50 (every key, the numbers bit for bit). A run D with neither key: its
  `run.json` has the keys `submit` writes today and `settings` exactly `mem_mb`, `priority`, `set`, `threads`; its `start` event
  has `info.continue_from` null and no `info.max_steps`. Refusals: `submit_with` with `continue_from` `../x`, with a well-formed id
  that names no run, and with `max_steps` 0 each return an error and add no run folder; a run whose `continue_from` names a run
  without `output/restart.rcfd` ends `failed` before launch (no `launched` event) with a reason naming that run and the file.
* **R-2 (`rcfd run`; `rcfd-cli` test).** On the same 12 x 12 case: `--max-steps 0` is exit 2; `--max-steps 5 --status-json` is
  exit 0 with a `finish` event of state `step_limit` and 5 iterations, and the `start` event's `info.max_steps` 5; with a case
  that has `chain.sequence`, `--max-steps 5` is exit 2.
* **C-CH (a chained study on the runner; the `rcfd-mcp` binary).** A new project with `cases/small.toml`, `.rcfd/runner-config.json`
  `{"max_runs": 2, "cpu_threads": 4, "mem_budget_mb": 65536}`. Through `rcfd-mcp --project`: the study `designs/two-chain` of ADR
  0189 C4-C (ii) (`nu` in [0.01, 0.02] at 2 levels, the conditions `base` {} and `slow` {`boundaries.lid.velocity_value`: [0.5,
  0.0, 0.0]}, `chain` left to its default, `workers` 2), then `design_run_study`. Pass: `evaluator` `runner`, `evaluations_run` 2,
  `runs_total` 4; the `base` runs have no `continue_from`; each `slow` run has `settings.continue_from` the `base` run of the same
  `variant`, its `start` event `start_iteration` equal to that `base` run's `solver_finish.iterations` and `info.continue_from` the
  path of that run's `output/restart.rcfd`, and its record's `iterations` plus that `start_iteration` equals its
  `solver_finish.iterations`. The same definition on a server without a project (its own folder): every in-process record usable
  with at least one metric (control); the records agree in `params`, `ok`, `status` and metric names; the `base` conditions'
  iterations are equal and their metrics within `REL_EXACT`; the `slow` conditions' metrics within `REL_CHAIN`. The largest
  relative difference of each kind and the iterations of both evaluators are recorded, with the `slow` iterations of a cold
  (`chain: false`) study of the same definition on the runner, for comparison only.
* **C-MS (a study's step cap).** In the same project: (i) the definition of C-CH with `chain: false` and `max_steps: 3000`
  (`designs/two-steps`, ADR 0189 C4-C (iii)): `evaluator` `runner`, four runs, each `settings.max_steps` 3000; the records equal the
  in-process ones in every field above, iterations included, metrics within `REL_EXACT`. (ii) The definition of C-CH (`chain`
  default) with `max_steps: 20` (`designs/two-cap`; the conditions converge in about 86 iterations, ADR 0189 addendum 1): every
  condition of both candidates not usable, `status` `not_converged`, `iterations` 20, no metrics, and each run's
  `solver_finish.state` `step_limit`; no `slow` run has `continue_from` (its `base` was not usable); the records equal the
  in-process ones (`params`, `ok`, `status`, `iterations`, no metrics).
* **C-MAP (an operating map on the runner).** In the same project: `design_run_operating_map {base: "cases/small.toml", out_dir:
  "designs/map", axes: [{path: "boundaries.lid.velocity_value", values: [[1.0, 0.0, 0.0], [0.75, 0.0, 0.0], [0.5, 0.0, 0.0]]}]}`,
  and the same call on the server without a project. Pass: `evaluator` `runner`, `runs_total` 3; point 0's run has no
  `continue_from`, points 1 and 2 continue from the run of the point before; the rows agree in `status` (all `converged`),
  `restart` (`cold`, `warm`, `warm`), `exit_code` and `state`; point 0's iterations are equal and its metrics within `REL_EXACT`,
  points 1 and 2's metrics within `REL_CHAIN` (iterations recorded). A second identical call returns `cached` true with no new
  run folder.
* **C-U (unit, workspace and runner).** `LaunchSettings::default()` serialises to `{}`; a `settings` object with both keys reads
  both and round-trips; `RunSettings::of` reads a `settings` object with the two keys (they are ignored there). The start rule of
  S6 on restart files written by `Simulation::save_restart` of the 16 x 16 case: the same case continues; the case at 20 x 20
  cells (another mesh) does not, with a note naming the mesh; a file of another engine (a transient laminar PISO run of the same
  mesh) does not, with no note. S7: `iterations` from a `solver_finish` of 130 iterations and a stream whose `start` event has
  `start_iteration` 86 is 44; without a stream it is 130.
* **C-1 (parity).** Every existing test passes, with only these registered edits:
  * **E1** `crates/rcfd-mcp/tests/tool_list_f275.rs`: `EXECUTE` gains `design_run_operating_map` (3 names), and B1's `args` gives
    it `{"out_dir": "designs/m", "base": "cases/cavity.toml", "axes": [{"path": "physics.nu", "values": [0.02]}]}`.
  * **E2** `crates/rcfd-mcp/tests/project_list_f270.rs`: the loop of ADR 0196 B (registered edit E2 there) runs over the three
    names.
  * **E3** `crates/rcfd-mcp/tests/design_runner_f274.rs`: C4-C (ii) and (iii) asserted `evaluator` `process` and no run folder;
    they now assert `evaluator` `runner` and four new run folders each, which C-CH and C-MS check in full; F-274's mutation (3)
    (the chain rule removed) has nothing left to remove.
* **Can fail.** Each mutation is made, the named check run and seen to fail, the file restored and compared byte for byte with
  the kept one: (1) the runner does not pass `--continue-from`: R-1 fails (B's `start_iteration`); (2) the runner does not pass
  `--max-steps`: R-1 fails (A's steps); (3) a `slow` run continues from the other candidate's `base` run: C-CH fails at
  `continue_from`; (4) S7's subtraction left out: C-CH fails at the iteration sum; (5) the map evaluator never continues: C-MAP
  fails at `restart`. `scripts/limit_audit.py` is run on the new tests.
* **Defaults.** No solver code changes; `baseline_hashes.rs`, `case_hashes_w48u.rs` and the case schema are unchanged; `Cargo.lock`
  is unchanged; `tools/list` without `--project` is byte-identical (`default_f1.rs` unedited); with `--project` it changes only by
  S9 (E1, E2). The full gate (fmt per package, clippy `-D warnings`, every test in release on a `git archive` copy, one process per
  test, three jobs) reports no failure; a failure is reported with its name, a known flake rerun alone.

## Not in F-287

Adopting runs after a server restart (F-288, ADR 0201); `start_run` arguments for the two settings (the MCP descriptor and its
golden would change; the project equivalent of `run_headless`'s `continue_from` that ADR 0195 Q2 names is the run setting, not yet
a tool argument); the `rcfd-runner submit` command's options; journal lines for design runs (ADR 0189 Q6, not asked); tags for map
runs (Q4); the Studio showing a run's source run.

## Where to go next

* [ADR 0189](0189-adr-0173-amendment-5-f274-registration-design-study-evaluations-on-the-runner.md) for the runner evaluator this
  extends.
* [Run control](../run-control.md) for `rcfd run` and its restart files; [the design loop](../design-loop.md) and
  [MCP tools](../mcp-tools.md) for the tools.

## Amendment 1 (2026-11-06): the start iteration of a continued run

Written after the first run of R-1, before any further code or test, and before C-CH, C-MS, C-MAP or C-U was written.

**What happened.** R-1 as registered failed at its first assertion on run B: B's `start` event has `start_iteration` 0, not 25
(its `info.continue_from` named A's restart file, as registered). Item 2 misread the code: the event's `start_iteration` is
`Simulation::iteration()` (`sim.rs` line 2566), the index of the last step taken, and `load_restart` clears that step
(`reset_run_state`, `load_checkpoint`), so a continued run's `start` event always says 0. The restored counter shows in the
run's first `iteration` event (index 26 after a restart at 25) and in `solver_finish.iterations` (50). Checked by hand with
`rcfd run --max-steps 25` and `--continue-from` on the same 12 x 12 case: `start_iteration` 0, first index 26, finish
iterations 50. No solver code is changed for this (the field's meaning stays as it is).

**The corrections.** No limit changes.

* **S7.** The start iteration of a continued run is the `iteration` of the restart file's `meta` that it continues from (the
  counter `load_checkpoint` restores), read by the evaluator when S6 lets the run continue, before it is queued; the record's
  `iterations` is `solver_finish.iterations` minus it. The `start` event is not read.
* **R-1.** The registered test stays, ignored, as the record of this defect (its reason names this amendment and the corrected
  test). The corrected test asserts, in place of B's `start_iteration` 25: B's `start` event `start_iteration` 0 and
  `info.continue_from` the path of A's restart file; B's first residual step 26; the `iteration` of A's restart file 25. Every
  other assertion of R-1 is unchanged.
* **C-CH.** In place of "its `start` event `start_iteration` equal to that `base` run's `solver_finish.iterations`": the `iteration`
  of that `base` run's restart file equals its `solver_finish.iterations`, and the `slow` run's first residual step is one more;
  the `slow` run's `info.continue_from` is the path of that file, and its record's `iterations` plus that `iteration` equals its
  `solver_finish.iterations`.
* **C-U.** In place of the stream: `iterations` from a `solver_finish` of 130 iterations with a start iteration of 86 is 44, with
  none 130; the start iteration read from the restart file of a run of 25 steps is 25.
* **Can fail (4)** is unchanged: S7's subtraction left out fails C-CH at the iteration sum.

## Addendum 1: what was built, and the results (2026-11-06)

The work is on branch `f287-f288-runner` (worktree `w49i-work/f287`), from main 9edfbda, with F-288 (ADR 0201). After this text
(3d63cc2) and amendment 1 (6562280):

* `a18ad80`: `rcfd-runner` and `rcfd-cli`, S1 to S4, with R-1 (registered, ignored by amendment 1), R-1c and R-2;
* `37bc4f1`: `rcfd-api`, the map seam of S8;
* `a1c3a31`: `rcfd-workspace` and `rcfd-mcp`, S5 to S10, C-CH, C-MS, C-MAP, C-U and the registered edits E1 to E3;
* `702183d`: F-288 (ADR 0201);
* `56b194b`: the documents of both items. The gate ran on `56b194b`; the branch is not merged.

**Built as registered**, with amendment 1's S7. Details:

* `rcfd-runner`: `records::LaunchSettings` (`of`, `is_empty`, `merged`), `FsStore::create_run_with` (the body of
  `create_run_tagged`, now a call of it), `Client::submit_with` with its refusals and `Client::solver_version` (also used by F-288),
  and in `runner.rs` `continue_file` and the two options added to the launch command only.
* `rcfd-cli`: `--max-steps` in the usage text, `RUN_VALUE_OPTS` and `cmd_run`; `info.max_steps` only when given.
* `rcfd-api`: `MapRun`, the default method `EvaluatorSource::map_evaluator`, `run_map_with` and `design.run_map` in `call_with`;
  `run_map` without a source runs exactly as before (it now also collects the prefix of S8, which nothing in process reads).
* `rcfd-workspace` `design.rs`: `case_run`, `cond_result_from` (`cond_result` is it with 0), `restart_iteration`, `StartFrom` and
  `start_from`, the shared `Queue`, `RunnerEvaluator` with the chained path, `RunnerMapEvaluator`, the map rule of the source,
  `ON_RUNNER` with `design.run_map`, and `continue_from` in the run objects; `runs.rs` gains `submit_with`.
* `rcfd-mcp`: `PROJECT_EXECUTE` with `design_run_operating_map`, and a comment in `dispatch`.

No Studio file, MCP descriptor text, `initialize` text or case-schema key changed; `Cargo.lock` is unchanged.

New tests: `rcfd-runner` `tests/continue_f287.rs` (R-1 ignored, R-1c), `rcfd-cli` `tests/max_steps_f287.rs` (R-2),
`rcfd-workspace` `tests/design_f287.rs` (C-U, 2), `rcfd-mcp` `tests/design_runner_f287.rs` (C-CH, C-MS and C-MAP in one project).

**Differences found while building.**

1. Amendment 1 (the `start` event of a continued run), registered before the corrected code and tests.
2. A map's `out_dir` does not exist before its first table is written, so the source's project test takes a path that does not
   exist yet from its nearest existing folder (`RunnerSource::in_project`).
3. The ignored R-1 was run once, before amendment 1: it failed at B's `start_iteration` (0 for 25), as amendment 1 records.

**Results.** The numbers are those the tests printed at `a1c3a31` (release build, this machine).

| Check | Result |
|---|---|
| R-1c | refusals: `../x` ("not a run id"), `r-20000101T000000-abcd` ("no run ... in this project") and `max_steps` 0, each with no folder made. A and C: `step_limit`, exit 0, steps 1 to 25 and 1 to 50, `settings.max_steps` and `info.max_steps` 25 and 50. B: `start_iteration` 0, `info.continue_from` A's restart file, whose `meta` says `iteration` 25; steps 26 to 50; its line of step 50 equals C's (`u` 5.239876409525057e-5, `p` 5.859085143335133e-5, continuity 4.6755819764314434e-7); `solver_finish.iterations` 50. D: the 8 keys of today, `settings` `mem_mb`, `priority`, `set`, `threads`, `info.continue_from` null, no `info.max_steps`. E: failed before launch, "continue_from: run <F> left no restart file .../runs/<F>/output/restart.rcfd", no `launched` event |
| R-2 | `--max-steps 0` exit 2; `--max-steps 5`: exit 0, `finish` `step_limit` with 5 iterations, `info.max_steps` 5, `restart.rcfd` written; with `chain.sequence`: exit 2 ("--max-steps caps a single run; ...") |
| C-CH | `evaluator` `runner`, 2 evaluations, 4 runs; the `slow` runs continue from their variant's `base` run: `nu=0.01` base ended at 88 (restart file `iteration` 88), slow steps 89 to 179, record 91; `nu=0.02` base 86, slow 87 to 175, record 89; `info.continue_from` the base run's restart file. Against in process: `base` iterations equal (88, 86) and `lid_drag` bit for bit; `slow` `lid_drag` -0.003935181616902512 against -0.003935181614683011 and -0.007530798984346871 against -0.007530798985682589, largest relative difference 5.6e-10 (limit 1e-4); `slow` iterations 91 and 89 on the runner, 91 and 88 in process; the same `slow` conditions cold on the runner took 86 and 87 |
| C-MS | (i) `max_steps` 3000, chain false: 4 runs with `settings.max_steps` 3000, records equal to in process bit for bit (iterations 88, 86, 86, 87). (ii) `max_steps` 20, chain default: 4 runs, each `step_limit` with 20 iterations, none continued, every condition `not_converged` without metrics, equal to in process |
| C-MAP | `evaluator` `runner`, 3 untagged runs, points 1 and 2 continued from the run before; `restart` cold, warm, warm and `status` converged on both; point 0 88 iterations both and `lid_drag` bit for bit; points 1 and 2: 82 and 83 iterations both, `lid_drag` within 4.2e-10 and 6.7e-10 (limit 1e-4). The second call: `cached`, no new run |
| C-U | `LaunchSettings::default()` is `{}`; both keys round-trip; `RunSettings::of` reads the object. The restart file of 25 steps of the 16 x 16 case has `iteration` 25; the case at another lid speed continues from it (`Continue { iteration: 25 }`); at 20 x 20 cells: cold, "cold start: the restart file was written for another mesh"; a transient PISO file: cold, no note; no file: cold with a note. 130 iterations from 86: 44; from 0: 130 |
| C-1 | E1, E2 and E3 as registered; `default_f1.rs`, `project_list_f270.rs` (with E2), `tool_list_f275.rs` (with E1), `design_runner_f274.rs` (with E3: both studies now `runner`, four runs each) and the F-274 tests of `rcfd-workspace` pass. Every other test: see the gate |
| Can fail | (1) no `--continue-from`: R-1c failed at B's first step (1 for 26). (2) no `--max-steps`: R-1c failed at A (`not_converged` after 400 steps, not `step_limit`). (3) `slow` continued from the first run of the call: C-CH failed at `continue_from` (the other variant's `base` run). (4) S7's subtraction left out: C-CH failed at the iteration sum (261 for 175). (5) the map never continued: C-MAP failed at point 1's `continue_from` (asserted before `restart`). Each file restored and compared byte for byte with the kept copy: equal |
| Defaults | against 9edfbda: `Cargo.lock`, `baseline_hashes.rs`, `case_hashes_w48u.rs` and the case schema unchanged (`git diff` empty); no solver file changed (`rcfd-api` changes only `design/tools.rs`). `scripts/limit_audit.py` reports 154 hits on the branch as on main 9edfbda, none in the new tests |
| Gate | `56b194b` (the code and documents of both items, before these addenda), `git archive` copy, three jobs (`scripts/gate_test.py`, one process per test), under the machine's gate lock, after 15 quiet minutes and 20 GB of free memory (the coordinator's threshold of 2026-11-06, in place of 30): fmt 0, clippy `-D warnings` 0, 2934 passed, 3 failed, 823 ignored, 2937 test processes, 572 binaries, the doc-tests passed; 84 min from start to end (71 min of tests), peak summed working set 3163 MB, lowest free memory 26 GB. Every new test passed there (R-1c, R-2, C-U, C-CH, C-MS, C-MAP, A-U, A-R, A-N), as did the tests with the registered edits. The three failures: `runner_w47b` `s1` and `s9`, known flakes, which failed the same way in the gates of `f289-f290` and `f286-transient-sst` the same evening (branches without this work); rerun alone on the gate's build, `s1` failed 3 of 3 (its OS sampler saw no solver process, "max by OS 0") and `s9` 2 of 3 ("two 60 MB runs ran together under a 100 MB budget": 0 for 1). And `rcfd-project` `edit_lock_w48f` `two_writers_without_lock_and_hash_lose_updates_control`, a race control in a crate this branch does not change, whose two writers did not interleave in the gate ("no update lost"); rerun alone it passed 3 of 3 (200, 200 and 195 updates lost). Reported as failures, with their names |

**Questions for the maintainer.** Q1 to Q7 stand as registered, each built on its option (a). In order of weight: Q1 (the two
`settings` keys, the lead's approval under ADR 0130, and where they live); Q5 (the class of `design_run_operating_map`); Q6 (the
iteration cap of a continued run); Q2, Q3, Q4 and Q7. Also for the maintainer: `rcfd run --max-steps` is a new command-line option.
