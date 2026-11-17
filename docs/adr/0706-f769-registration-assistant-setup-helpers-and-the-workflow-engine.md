# ADR 0706: F-769 registration: the assistant setup helpers, the workflow engine and the checks of their test files

Status: **REGISTERED 2026-11-17, written before any run of the checks of [ADR 0705](0705-f769-registration-adrs-for-the-five-feature-groups-that-no-adr-describes.md)
(F-769, group 1 of 5).** Not yet accepted by the maintainer. Results are in the addendum of ADR 0705.

**These tests predate this ADR.** The four files below were committed on 2026-10-13 (`f71f6e2c`, the first import of rcfd, for
`physics_workflow.rs` and the assistant `workflow.rs`; `643caf54` for `workflow_engine.rs` and the api `workflow.rs`) and no ADR
registered their limits; ADR 0067 and ADR 0123 only name them in the "Test files" notes that ADR 0685 appended, and say there that the
setup helpers and the workflow engine are not described. This ADR records what the files check and which limits they carry, read from
the code and from the roadmap notes of F-120, F-121, F-147 and F-148. It is not a decision taken before the tests, and it does not claim a
derivation that the files do not contain. No Rust file is edited and no test is run for it (ADR 0705, Limits).

## The feature

* **Setup helpers of the assistant** (`rcfd-assistant::workflow`, F-148): `explain_residuals`, `assess_mesh`,
  `suggest_boundary_conditions`, `validate_case`, all read-only tools that go through the same argument validation as the other tools.
  The F-148 roadmap note says their thresholds are rule-of-thumb values, not validated against real cases. A later, additive layer
  (`suggest_boundary_conditions_for`, and the settings argument of `validate_case` and `explain_residuals`) makes the helpers aware of
  the turbulence model and the compressible solver; it is what `physics_workflow.rs` checks. The laminar incompressible output must
  stay as it was.
* **Workflow engine** (`rcfd-api::workflow` and the service methods `workflow.*`, `run.check`, `run.headless`, `status.read`; F-120, F-121;
  contract in `docs/run-control.md`): a task graph over the settings tree (templates `mesh_to_results`, `channel_flow`, `empty`), plans
  with prerequisites, stale tracking, insert and delete with validation, JSON documents in a sandbox, and the headless run with check-only,
  checkpoint, stop file and status stream. The assistant offers it as the tools `list_workflow_tasks`, `create_workflow`, `edit_workflow`,
  `run_workflow_task`, `check_case_headless`, `run_headless`, `read_status_stream` behind the approval gate of F-147.

## Files

| file | tests | what it is |
|---|---|---|
| `crates/rcfd-assistant/tests/physics_workflow.rs` | 5 | the settings-aware helpers for turbulence and compressible cases (additive) |
| `crates/rcfd-assistant/tests/workflow_engine.rs` | 1 | a scripted loop with the mock provider through the approval gate over the engine and the headless contract |
| `crates/rcfd-assistant/tests/workflow.rs` | 9 | F-148, the four helpers on synthetic data and the tools on a real service (named here as ADR 0704 recommends; the note in ADR 0067 stays) |
| `crates/rcfd-api/tests/workflow.rs` | 5 | F-120 and F-121, the engine and the headless methods of the service (named here as ADR 0704 recommends; the note in ADR 0123 stays) |

## Limits carried by `physics_workflow.rs`

The file uses a three-patch input (inlet of kind `Inlet`, outlet `Outlet`, walls `Wall`).

| id | test | kind | limit | source |
|---|---|---|---|---|
| H1 | `laminar_settings_leave_the_suggestions_unchanged` | exact | with `turbulence.model = laminar` and `solver.algorithm = simplec`, `suggest_boundary_conditions_for` returns exactly what `suggest_boundary_conditions` returns | the assert (equality of two JSON values) |
| H2 | `k_epsilon_inlets_get_turbulence_helpers` | structural | with `k_epsilon`, the inlet suggestions contain the paths `boundaries.inlet.turbulence_spec`, `.turbulence_intensity`, `.turbulence_length`, and still `boundaries.inlet.velocity`; with `k_omega_sst` the inlet has `.turbulence_length`; the walls have no path that contains `turbulence` | the asserts |
| H3 | `compressible_patches_get_compressible_bc_entries` | exact | with `solver.algorithm = compressible` the inlet has `compressible_bc` and `pressure_value` and does not have `pressure`; the outlet paths are exactly `[boundaries.outlet.compressible_bc]`; the walls exactly `[boundaries.walls.compressible_bc]` | the asserts |
| H4 | `validate_case_adds_physics_notes_without_the_laminar_reynolds_warning` | numeric and structural | the laminar tree still warns "laminar flow only"; `k_epsilon` gives a warning containing `y+` and no longer "laminar flow only"; `k_omega_sst` on a `ly = 0.1`, `ny = 20` mesh gives a warning at `turbulence.model` with a number `y_plus_estimate` **above 500.0 and below 1500.0**, and a message containing `SST`; the same model with `wall_layers = 10` and `first_layer_height = 1e-5` gives `y_plus_estimate` **below 3.0**; a compressible tree with `compressible.cfl = 0.95` warns with `compressible.cfl` and not "laminar flow only" | the asserts; the comment of the test derives the first window (U = 10 m/s, nu = 1e-6, x = 1: Re_x = 1e7, u_tau about 0.36, first cell 0.005, y+ about 900) |
| H5 | `explain_residuals_handles_turbulence_and_compressible_series` | exact and structural | on a laminar tree an extra series `res_nut` does not change the verdict; a rising `res_u` and `res_nut` series (50 samples, from 1e-3, factor 10 per 5 samples) on a `k_epsilon` tree with `relax = 0.7` gives advice that names `res_nut` and a suggested change at `turbulence.relax`; a transient compressible tree gives the verdict "transient compressible run"; a steady compressible tree with `steady_tolerance = 1e-6` and a falling `res_rho` series (50 samples, 10^(-i/5), last about 1e-9.8) gives the first finding series `res_rho` and the verdict "converged" | the asserts |

What it does not check: that the suggested numbers are good (the y+ windows are wide on purpose), any case that is run, the wording of
the messages beyond the quoted words.

## Limits carried by `workflow_engine.rs`

One test, `assistant_drives_the_workflow_engine_and_the_headless_contract`, with the mock provider (no network), a temporary root, mode
`ApproveWrites`, five scripted turns and a sixth that reads the last tool result.

| id | kind | limit | source |
|---|---|---|---|
| H6 | exact | the tools that asked for approval are, in order, `create_case`, `set_setting`, `set_setting`, `create_workflow`, `run_workflow_task`, `edit_workflow`, `run_workflow_task`, `set_setting`, `run_headless`: nine prompts; the read-only tools (`list_workflow_tasks`, `check_case_headless`, `read_status_stream`) never prompt | the assert on `asked` |
| H7 | exact | the first listing has `next = mesh_check` and **5** tasks; `check_case_headless` returns `exit_code` **0**; `run_workflow_task` on `solve` runs exactly `mesh_check`, `setup`, `initialize`, `solve` and the solve outcome state is `converged` | the asserts |
| H8 | exact | after an inserted `export` task (kind `export_vtk`, dir `results`) the next `run_workflow_task` leaves `workflow.complete = true` and `results/solution.vtu` exists; after `set_setting physics.nu = 0.02` every task is `stale` | the asserts |
| H9 | exact | `run_headless` with `max_steps = 15` gives the outcome state `step_limit` and the file `restart.rcfd`; `read_status_stream` gives state `step_limit`, iterations **15**, complete true; the final text is "Stream says step_limit after 15 iterations."; the mock provider received **6** requests; the first request offers the seven new tools | the asserts |
| H10 | structural | a denied `create_workflow` leaves no workflow: the following `list_workflow_tasks` is an error whose text contains `workflow.create` | the asserts of the second half of the test |

What it does not check: the convergence history of the cavity (only the outcome state), the contents of the VTU file, a real model
provider, the MCP server.

## Limits carried by `rcfd-assistant/tests/workflow.rs` (F-148, 9 tests)

| id | test | kind | limit |
|---|---|---|---|
| H11 | `growth_is_diverging_with_robustness_changes` | exact and numeric | a 60-sample series `0.1 * 1.15^i` for `res_u` and `res_p` gives the verdict and the finding pattern `diverging`; the suggested changes include `solver.alpha_u`, `solver.alpha_p`, `solver.convection`; the first suggested value lies in **[0.05, 0.9)** (the case has `alpha_u = 0.9`) |
| H12 | `nan_is_reported_with_its_location` | structural | NaN at sample **20** of a 30-sample decay gives the pattern `non_finite`, a verdict containing `NaN`, and evidence containing "sample 20" |
| H13 | `oscillation_is_detected` | exact | 80 samples alternating 1e-2 and 5e-2 give `oscillating` as pattern and verdict |
| H14 | `stagnation_above_tolerance_is_detected` | exact | 30 decaying samples followed by 70 at 3e-4 (tolerance 1e-6) give `stagnating` and a suggested change at `solver.non_orthogonal_correctors` |
| H15 | `healthy_and_converged_runs_get_no_changes` | exact | a 60-sample decay at rate 0.1 gives `converging` and no suggested changes; 200 samples at rate 0.2 give `converged`; two samples give "not enough data" |
| H16 | `mesh_assessment_thresholds` | exact | a mesh with non-orthogonality 5.0, skewness 0.2, aspect ratio 3.0, volume ratio 1.5 is `good`; non-orthogonality 55.0 is `usable with care` and the advice mentions `non_orthogonal_correctors`; non-orthogonality 80.0, skewness 5.0, aspect ratio 2000.0 and a negative-volume error is `not usable` with **4** findings. The test brackets the thresholds; it does not pin them |
| H17 | `bc_suggestions_by_name_and_kind_only_propose` | exact | six patches: `inlet` role `inlet` with a rationale containing `PLACEHOLDER`; `outlet` second setting `boundaries.outlet.pressure = fixed`; `top_wall` role `wall`; `frontAndBack` role `empty` with no settings; `mystery` role `unknown`; `walls` already configured: status `configured` and no settings |
| H18 | `validation_finds_mismatches_and_high_reynolds` | structural | a boundary entry `nowhere` gives `ok = false` and an error naming it; the patch `walls` without a boundary gives a warning with `'walls'` and "no boundary condition"; a warning contains `Re =`; none mentions `frontAndBack` |
| H19 | `workflow_tools_work_against_a_real_service_and_change_nothing` | exact | on a created cavity: `assess_mesh` verdict `good`; `validate_case` ok; `suggest_boundary_conditions` has **3** suggestions; `explain_residuals` before any run is "not enough data"; the case hash before and after is equal; without a session `assess_mesh` is an error |

## Limits carried by `rcfd-api/tests/workflow.rs` (F-120, F-121, 5 tests)

The case is a 10 x 10 cavity (`nu = 0.01`, `simplec`, upwind, `max_iterations = 400`, tolerances 1e-6) with a `lid_drag` force report.

| id | test | kind | limit |
|---|---|---|---|
| H20 | `template_runs_end_to_end_and_setting_changes_make_tasks_stale` | exact | `workflow.list` before a create is an error naming `workflow.create`; an unknown template names `mesh_to_results`; after create `next = mesh_check` and all tasks `not_run`; running `solve` runs `mesh_check`, `setup`, `initialize`, `solve` (the state of `report` stays `not_run`) and the solve state is `converged`; `with_prerequisites = false` on `solve` is refused ("not done"); running the rest runs **1** task, the workflow is complete and the report names `lid_drag`; `settings.set physics.nu = 0.02` makes every task `stale`; running `report` again runs **5** tasks |
| H21 | `set_task_changes_the_case_and_marks_its_dependants_stale` | exact | a `set` task `finer` (`mesh.nx = 14`) inserted after `mesh_check`; deleting `mesh_check` is refused ("depend"); running it sets `mesh.nx` to **14**, leaves itself `done` and makes `mesh_check` `stale`; after running `report` and then `finer` again without prerequisites, `report` and `solve` are `stale` and `finer` is `done` |
| H22 | `failing_tasks_stop_the_run_and_say_why` | exact | `max_steps = 5` makes the `solve` task fail with a message containing `step_limit`, `report` stays `not_run`; an invalid boundary name fails the validate task with a message containing `nonexistent` |
| H23 | `workflow_files_are_sandboxed_and_round_trip` | exact | `workflow.save` of the `channel_flow` template writes `rcfd-workflow` with **6** tasks, `workflow.load` reads **6**; `../outside.json` and `/etc/passwd` are refused with "sandbox"; a hand-edited cycle is refused with "cycle"; the export task writes `results/solution.vtu` |
| H24 | `headless_run_check_stop_file_checkpoint_and_status_file` | exact and numeric | `run.check` gives ok and `exit_code` **0**; `run.headless` with `max_steps = 25`, a checkpoint and a status file (every 10) ends `step_limit`, status `ready`, the checkpoint exists, `status.read` gives `step_limit`, complete, **1** checkpoint; a stop file gives `exit_code` **7** and status `stopped`; continuing from the checkpoint ends `converged` with more than **25** iterations and writes `res/solution.vtu`; each of the five path arguments (`continue_from`, `stop_file`, `checkpoint`, `status_path`, `out_dir`) with `../x` is refused with "sandbox"; `service.info` advertises `workflow.run`, `run.headless`, `run.check`, `status.read` |

## What this ADR does not cover

* Exit codes 4 and 6 of the headless contract (the F-121 note says they are covered by library tests only), the `--max-elapsed` code 8.
* Real model providers, the MCP server tools list (a golden, not edited), concurrency of two runs in one session (a comment of H24 mentions
  it; there is no assert).
* Thresholds of `assess_mesh` and `explain_residuals`: rule of thumb (F-148 note); the tests only bracket them.
