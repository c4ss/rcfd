# ADR 0142: Wave 48H, finished example projects: a wind tunnel and a quick start that open straight to a picture

Status: **PROPOSED 2026-10-28** (registered before any counted run; results in the addendum). Written at main f558285
(waves 47 and 48, the 48G case starter and store-fields default, the Linux gate fixes). Job 5 of the "first run to first
picture" work; jobs 1 (mesh preview from a case), 3 (store fields by default, merged as ADR 0140) and 4 (one-click start
and follow) are done on other branches and edit the Studio, which this job does not. House rules of ADRs 0110-0140 apply:
limits are fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit
intact; a defective set-up is corrected by an amendment registered before the corrected run; defaults of existing code
stay bit-identical; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; the crate graph stays acyclic.
Parents: [`docs/ui-restart-plan.md`](../ui-restart-plan.md) section 8 (project folder contract), ADR 0130 (run-record
contract), ADR 0132 (viewport), ADR 0133 (run control, `output.store_precision`), ADR 0139 (shipped case files), ADR 0092
(registry set-ups of the benchmark descriptors), ADR 0121 (design ledger).

## Problem (reported by the maintainer)

The Studio showed nothing on first open: a new project is empty, so there is no picture, no mesh and no run to scrub. The
task: finished example projects in `example-projects/` that open straight to a picture and show that rcfd holds more
than one problem per project; a regeneration script; a staleness test; this ADR.

## Context (read at main f558285)

* Runnable case files in `cases/`: the nine TOML files of ADR 0139 plus `channel/profile_inlet.toml`. None is a cavity,
  heated cavity, cylinder, aerofoil or step; for those `cases/` holds benchmark descriptors (`case.json`) only, which
  `rcfd run` refuses. The task names "the lid-driven cavity and the heated cavity (runnable TOMLs from the 48G list)":
  that list has no such files (deviation 1).
* Mesh generators of the case schema (`crates/rcfd-api/src/schema.rs`): `cylinder_ogrid` and the box family, no aerofoil
  and no step generator. ADR 0092's registry set-ups (`crates/rcfd-api/tests/registry_setups_w39a.rs`) build the
  descriptors' meshes in Rust (`gen::backward_step`; a `cylinder_ogrid` warped onto NACA 0012), write them as OpenFOAM
  polyMesh folders and read them with `mesh.source = "file"`; they were only put through `rcfd check`, never solved.
  `rcfd convert <case.toml> <dir>` writes a case's generator mesh as `<dir>/polyMesh`.
* The run service (`crates/rcfd-runner/src/runner.rs` `launch`) runs `rcfd run runs/<id>/case.snapshot --case-dir <the
  case's folder> --out runs/<id>/output --store-dir runs/<id>/fields --status-monitors ...`; `run.json` records `case`
  project-relative and `case_source`, `case_dir` as absolute paths; `serve --project <relative path>` fails every run
  (ADR 0130 addendum, found, not fixed), so absolute paths are used. Requests are files; a client started from a
  capturing shell can hand the runner inheritable handles (ADR 0123 S5), so the runner is started directly with its
  output in a file.
* The field store is written only when the case (or an override) sets `output.store_interval` or
  `output.store_time_interval`; `output.store_precision = "f32"` (ADR 0133) halves it; `output.store_fields` picks the
  fields (`vorticity_z` and `speed` are available, `crates/rcfd-api/src/sim.rs` `field`).
* `Project::validate` (`crates/rcfd-project/src/project.rs`) reports a missing directory for each of `cases`, `meshes`,
  `runs`, `designs`, `notes`, `.rcfd`; git keeps no empty folder. `.gitignore` does not name `.rcfd/` and ignores `*.vtu`.
  `.gitattributes` has `* text=auto eol=lf`, which normalises line ends of text files on commit (a CRLF case file would
  then no longer match the hash recorded for its snapshot); the ADR 0130 A4 fixture folder carries its own `* -text`.
* The Studio's case editor validates with a default `Service`: `session.create {"toml": text}` (base folder `.`, the
  process's working folder) and `settings.validate`, which builds the mesh (`crates/rcfd-studio/src/case_edit.rs`;
  ADR 0131 addendum question 3, open). A relative `mesh.file` therefore resolves against the Studio's working folder.
* `crates/rcfd-cli/tests/workspace_graph.rs` reads every dependency kind from `cargo metadata`, dev-dependencies
  included: a test of `rcfd-project` or `rcfd-runner` that uses `rcfd-api` would break the layering. `rcfd-mcp` (layer 6)
  already depends on `rcfd-api`, `rcfd-io`, `rcfd-project`, `rcfd-runner` and `rcfd-tsstore`.

## What I already knew before registration (rule d smoke)

Release build of f558285 in this worktree (`rcfd`, `rcfd-runner`). Unregistered runs of draft cases with `rcfd run`
(single thread), and of `scripts/make_example_projects.py` on a scratch copy; nothing of it is kept:

| Draft | Result |
|---|---|
| cylinder 64 x 48, Re 100, PISO, dt 0.05, kick 0.1 at (2, 0.3), t 150 | 37 s; shedding saturates near t 70; St 0.153, mean C_D 1.325 over 8 periods |
| the same, kick 0.5 at (1.5, 0), t 100 | saturates near t 40; **the last step was 3.5e-12 s** (2000 x 0.05 falls short of 100 by 4e-12, the solver lands with a remainder step) and its C_D, C_L were -4e5, 9e5: the pressure of that step is spoiled (found, not fixed; question 6) |
| the same with dt 0.0625 (a power of two, exact) | 1600 steps, 22 s, no remainder step; St 0.152, C_D 1.324, saturated from t 40; max Courant 2.06 |
| Re 200 (nu 0.005), dt 0.0625, t 80 | 18 s; St 0.175, C_D 1.291, C_L amplitude 0.63 |
| aerofoil mesh by the recipe of D3 | `rcfd check-mesh`: 6144 cells, max non-orthogonality 80.3 deg (12 faces above 70), skewness 2.25, aspect 164; "mesh ok"; `rcfd check` warns SV101 |
| NACA 0012 SST Re 6e6, alpha 4, SIMPLEC, linear upwind | 3000 its 99 s, exit 5: res_u stalls at 1.4e-6, res_nut 4e-4; C_L 0.3453 (it 200) to 0.3489 (it 3000), C_D 0.0253 |
| the same at alpha 8 | not usable: C_L swings between -0.08 and 0.92 over 2000 its (exit 5); with `pseudo_mode = local` diverged at it 145; with SIMPLE 0.7/0.3 diverged at it 116 (at the trailing-edge cell); with `linear_upwind_limiter = barth_jespersen` C_L drifts to 0.06 |
| the same at alpha 2 | 2000 its 68 s, exit 5, res_u 1.6e-6; C_L 0.2022 (it 200) to 0.1988, C_D 0.0189 |
| step 200 x 40, Re 800 (Gartling form, D1) | converged in 740 its, 28 s; floor reattachment 5.89 H, upper bubble 4.65 H to 10.42 H (read from the VTU) |
| the same at Re 400 | converged in 429 its, 20 s; floor reattachment 4.25 H, upper bubble 3.86 H to 5.27 H |
| cavity 32 x 32 Re 100 and 400; heated cavity 40 x 40 Ra 1e3 and 1e4 | each under 2 s; converged in 121, 158, 339, 278 its; hot-wall Nusselt 1.119 and 2.257 (with the corrected divisor of D1) |
| the script on a scratch copy, quick-start only | 4 runs through the run service in 5 s, all finished; ledger and run table written; `--check` passed for quick-start |
| `--rerun-check` of that copy | case.snapshot, residuals.jsonl, fields/store.rts, fields/index.json and output/monitors.json byte-equal for all 4 runs |

The first Nusselt divisor (0.1, the schema's `mesh.depth` default) was wrong by 4: the 2D generators make the cell one
cell width deep (`lx / nx`, `crates/rcfd-mesh/src/gen.rs`), so the divisor is 0.025. No registered check had run.

## Decisions (none is a maintainer decision; the open points are questions at the end)

1. **D1 Projects, problems and set-ups.** Two project folders under `example-projects/`, each a valid project of the
   section-8 contract:

   | Project | Case (variant) | Set-up | Mesh | Stored |
   |---|---|---|---|---|
   | wind-tunnel | `cylinder-re100.toml` (`cylinder-re200.toml`) | laminar PISO, linear upwind, dt 0.0625, t 0..80, D = U = 1, nu 0.01 (0.005); uniform start plus a kick `0.5 exp(-((x-1.5)^2 + y^2))` in `uy`; free stream on the far field; reports `cd`, `cl` | `cylinder_ogrid` 64 x 48, r 0.5..15, grading 100 (3072 cells) | `ux uy p vorticity_z` every 1.0 (81) |
   | wind-tunnel | `naca0012-aoa4.toml` (`naca0012-aoa2.toml`) | k-omega SST, Re 6e6 (ADR 0092's condition), alpha 4 (2), SIMPLEC, linear upwind, 2000 iterations; reports `cl`, `cd` | `meshes/naca0012-ogrid` (D3) | `ux uy p nut` every 200 (11) |
   | wind-tunnel | `step-re800.toml` (`step-re400.toml`) | laminar step, expansion ratio 2, Gartling's form: the domain starts at the step, the left boundary is the step face (zero velocity) below y = 0.5 and a parabolic inlet `24 (y - 0.5)(1 - y)` (mean 1) above; Re = U_mean H / nu = 800 (400); SIMPLEC, linear upwind, to 1e-6; report `p_inlet` | `box_2d` 200 x 40 on 15 x 1 (8000 cells) | `ux uy p` every 40 |
   | quick-start | `cavity-re100.toml` (`cavity-re400.toml`) | the cavity tutorial's case (`docs/tutorials/cavity.md`), nu 0.01 (0.0025); report `lid_drag` | `cavity_2d` 32 x 32 | `ux uy p` every 5 |
   | quick-start | `heated-cavity-ra1e3.toml` (`heated-cavity-ra1e4.toml`) | ADR 0092's heated-cavity set-up, g 710 (7100); reports `q_hot` and `nu_hot = -q_hot / 0.025 W` | `box_2d` 40 x 40 | `ux uy p T` every 10 |

   Every variant also gets its completed run (each is cheap, smoke above). Names carry the changed value. The aerofoil
   variant is at 2 degrees because 8 degrees did not converge (smoke); the cylinder's dt is a power of two because of the
   remainder step (smoke). None of this is a validation; each problem's note says how it relates to its benchmark.
2. **D2 Generation through the real run service.** `scripts/make_example_projects.py` per project: keeps the manifest
   (written once: schema 1, name, created, `rcfd --version`), rebuilds the meshes from their recipes (D3), empties `runs/`
   and `.rcfd/`, writes `.rcfd/runner-config.json` (`max_runs` 1, `cpu_threads` = threads, `poll_ms` 100), starts
   `rcfd-runner serve --project <absolute>` itself (stdout and stderr to a file in `.rcfd/`, `RCFD_BIN` set), queues every
   case with `rcfd-runner start <absolute case> --threads 1 --no-runner` in the order of D1 (no `--set`: the case file
   alone describes each run; no `--mem-mb`: the runner's own pre-launch check runs), waits with `rcfd-runner wait`, shuts
   the runner down, prunes (D5), writes the ledger (D8) and the run tables (D9), then `example-projects/manifest.json`
   (D6) and runs its own `--check`. One solver at a time, one thread (results do not depend on the thread count,
   `docs/run-control.md`).
3. **D3 Aerofoil mesh recipe.** `meshes/naca0012-ogrid.recipe.toml` is a case holding only the `cylinder_ogrid` mesh of
   ADR 0092 (128 x 48, r 1..2, grading 200, depth 0.1). The script writes it with `rcfd convert` and moves every point as
   ADR 0092's `naca0012_mesh` does (formula in the recipe's comment; ported to Python, points written with `repr`), and
   renames the patch `cylinder` to `airfoil`: `meshes/naca0012-ogrid/{points,faces,owner,neighbour,boundary}`. The
   aerofoil cases name it `../meshes/naca0012-ogrid` (relative to the case's folder, as the run service resolves it).
   Every other case uses a generator in the case, so it validates from any working folder.
4. **D4 Field stores.** Set by each case: f32, the fields of D1, at most 100 snapshots; steady runs store fewer.
5. **D5 What a rebuild keeps.** Kept per run: the contract files (`run.json`, `case.snapshot`, `status.json`,
   `events.jsonl`, `log.txt`, `residuals.jsonl`, `fields/`, `metrics.json`), the runner's `check.json` and the solver's
   `output/monitors.json`, `output/monitors.csv`, `output/report.html`. Removed: `solver-status.jsonl` (the raw stream that
   `residuals.jsonl` and `events.jsonl` carry), `output/*.vtu` (ignored by git anyway) and `output/*.rcfd` (the
   checkpoint); request files. `.rcfd/` keeps only a `.gitignore` (`*`, `!.gitignore`), so the folder exists in every
   checkout (validation needs it) and the Studio's and runner's state there is never versioned. `quick-start/meshes/`
   holds a `README.md` so the folder exists (the Studio lists it as a mesh entry; accepted). `example-projects/.gitattributes`
   has `* -text`, so git keeps every byte (snapshot hashes, binary stores). `log.txt` and the solver events name the
   removed checkpoint; accepted and noted.
6. **D6 Committed runs and user runs.** `example-projects/manifest.json` lists each project's committed runs (id and
   case), the solver version and the caps. The checks read only those runs; a run a user starts inside the checkout is
   reported as "not one of the committed example runs" and left out of every check and of the size count. The notes ask
   users to copy a project out of the repository before starting runs (question 3).
7. **D7 Size caps** (bytes of every file of a project folder with only its committed runs, `.rcfd/` included): wind-tunnel
   35 000 000, quick-start 6 000 000, all of `example-projects/` 40 000 000. Estimate from ADR 0140 D4 plus the smoke's
   output sizes: wind-tunnel about 29 MB (cylinder runs about 5.8 MB each, aerofoil 4.1, step 4.3 and 3.5, the polyMesh
   1.3), quick-start about 4.4 MB.
8. **D8 Design ledger.** `designs/variants.ledger.jsonl` per project, one `rcfd-ledger-1` entry (ADR 0121 D1, keys
   sorted) per run: `study` per problem (`cylinder-reynolds`, `naca0012-angle-of-attack`, `step-reynolds`,
   `cavity-reynolds`, `heated-cavity-rayleigh`), `variant` the case stem, `params` the changed value (`Re`, `alpha` in
   deg, `Ra`), `metrics` every report of `metrics.json` plus derived values, `evaluator` "rcfd-runner + rcfd run",
   `evaluator_version` and `solver_version` from `run.json`, `geometry_hash` the store's mesh hash, `settings` (case,
   snapshot hash, run id, run settings), `verdict` ok, `fidelity` cfd, `recorded_at_ms` the run's final state event,
   `wall_s` launched to final state, `provenance` (driver, project, run, state, solver state, reason). Derived: cylinder
   `strouhal`, `cd_mean`, `cl_amplitude` over the last 6 whole periods of `cl` (upward crossings through its mean over the
   second half of the run; D = U = 1; trapezoidal mean); step `x_reattach` (last sign change of `ux` along the first cell
   row above the floor of the last snapshot, linear interpolation, as `crates/rcfd-fv/tests/step_laminar.rs`), and
   `x_top_separation`, `x_top_reattach` (the first two sign changes along the row below the upper wall), in m = H. An
   aerofoil run that ends at its iteration cap is `ok` with the solver state "not_converged" in `provenance` (its metrics
   are finite and are what the run gives); question 4.
9. **D9 Notes.** `notes/README.md` per project (contents, how to open, what to click in order, limits) with a run table
   between two markers that the script rewrites (run id, state, steps, wall time, snapshots, final values), and one note
   per wind-tunnel problem (`cylinder.md`, `naca0012.md`, `step.md`). Paths in backticks are checked (S7); repository
   files are named as `../../<path>`.
10. **D10 Reproducibility.** Byte-reproducible on the same machine and binaries (checked by `--rerun-check`, R1):
    `case.snapshot`, `residuals.jsonl`, `fields/store.rts`, `fields/index.json`, `output/monitors.json`, the polyMesh.
    Not reproducible: run ids, time stamps and pids (`run.json`, `status.json`, `events.jsonl`, `metrics.json`
    `solver_finish`), absolute paths (`run.json`, `log.txt`, solver events), `report.html`, the ledger's
    `recorded_at_ms`, `wall_s` and run ids, the run tables.

## Placement

No change to any crate's source, no new dependency, no new workspace edge. New: `example-projects/` (two projects,
`README.md`, `manifest.json`, `.gitattributes`), `scripts/make_example_projects.py` and its self-test
`scripts/test_make_example_projects.py` (added to the `scripts` job of `.github/workflows/ci.yml`, next to the other script
self-tests), `crates/rcfd-mcp/tests/example_projects_w48h.rs`, and a headless Studio test
`crates/rcfd-studio/tests/example_projects_h.rs` (a test file only; the Studio's source is not edited).
`workspace_graph.rs` and `docs/architecture.md` are unchanged.

## Registered checks (limits fixed here, before any counted run)

Counted runs: one rebuild of both projects by the script from the committed inputs (G), the tests below on the result,
release builds, `--test-threads 1`, output to files.

| Id | Check | Limit | Negative control (must fail or discriminate) |
|---|---|---|---|
| G1 | Every run of the rebuild | 10 of 10 `finished`; solver state `finished` with step 1280 and time 80 (cylinders), `converged` (steps, cavities, heated cavities), `not_converged` at iteration 2000 with exit code 5 (aerofoils, D1) | (S1 reads the states) |
| G2 | Wall time per run (launched to final state, from `events.jsonl`) | each <= 180 s; the whole rebuild <= 20 min | (reported) |
| G3 | The examples show their flow (sanity, not validation) | cylinders: at least 7 upward crossings of `cl` in the second half, `cl_amplitude` > 0.1, the Re 200 `strouhal` above the Re 100 one; aerofoils: 0 < `cl`(2) < `cl`(4); steps: 0 < `x_reattach` < 15 at both Re, Re 800's longer than Re 400's; heated cavities: 1 < `nu_hot`(1e3) < `nu_hot`(1e4); cavities: `lid_drag` < 0 | the same comparisons with the two variants swapped fail (aerofoil, step, heated cavity) |
| S1 | `rcfd-project` reader (`example_projects_w48h.rs`) | both projects open, no migration, `validate()` 0 issues, `runs()` 0 skipped, every committed run listed `finished` with its case; each case file byte-equal to its run's `case.snapshot`; every store opens from its footer, verifies, and its snapshot count and mesh size equal `index.json` | on copies: (a) a changed byte of a `case.snapshot`, (b) a store cut after its second-last snapshot, (c) `schema_version` 0 and 2, (d) a case edited after its run: each reported |
| S2 | Pruned and runtime files | `.rcfd/` holds only `.gitignore`; no `solver-status.jsonl`, `*.vtu`, `*.rcfd` in a committed run | (direct listing) |
| S3 | Cases through rcfd-api | 10 of 10 case files: `Case::load` (base: the case's folder), `runctl::check_only` exit 0, 0 error-level rules | (ADR 0092 V1's controls) |
| E1 | Cases through the Studio editor's calls (default `Service`, `session.create {"toml"}`, `settings.validate`, test working folder `crates/rcfd-mcp`) | 10 of 10 open and validate | **predicted miss** for the 2 aerofoil cases (Context: base `.`): registered as `e1_mesh_file_cases_..._known_miss` (ignored, limit intact); the 8 generator cases are a separate passing test |
| E2 | Contrast for E1 | the 2 aerofoil cases validate through `session.create {"path"}` (base: their folder), and through the text call they open and fail with a diagnostic naming `mesh.file` | (the text call is the control) |
| S4 | Aerofoil angle | the angle of `initial.velocity` equals the far-field `velocity_value`'s; `reports.cl.direction` = (-sin, cos, 0), `reports.cd.direction` = (cos, sin, 0) within 1e-15; equals the ledger's `alpha` within 1e-12 deg | (a mismatch is the failure) |
| S5 | Runs against their case | store mesh hash = `build_mesh(case.snapshot).content_hash()` and its cell count; 1 to 100 snapshots, each exactly the case's `output.store_fields`, all f32; `residuals.jsonl` without torn tail, steps 1..N, N = `status.json` step; transient end time = `solver.end_time`; `metrics.json` `reports` bit-equal to the last residual line's monitors | (S1 (d) covers a changed case) |
| S6 | Ledger | reads with `ledger::parse_text`, 0 skipped lines, every entry `validate()`s; one entry per committed run with its case, snapshot hash and mesh hash; report metrics bit-equal to `metrics.json`; derived metrics recomputed in Rust from the run's own files within 1e-9 relative; no unchecked derived metric | (a missing or extra entry fails the count) |
| S7 | Notes | every backticked path of `notes/*.md` exists; each project's `README.md` names every committed run | a copy whose README names `cases/no-such-case.toml` is reported |
| S8 | Sizes | each project and the total under D7's caps | (reported against the cap) |
| S9 | Aerofoil mesh | `meshes/naca0012-ogrid` reads, 6144 cells, patches `airfoil`, `farfield`, `frontAndBack`; `airfoil` points on |y| = y_t(x) and `farfield` points on the circle r = 20 about (0.5, 0), within 1e-12 | (a point off the curve is the failure) |
| P1 | `scripts/make_example_projects.py --check` and `scripts/test_make_example_projects.py` | `--check` 0 problems on the committed examples; the self-test passes | the self-test's tampered copy (a changed case, a missing snapshot count) is reported |
| R1 | `--rerun-check` (both projects rebuilt in a temporary copy) | the D10 byte-reproducible files of all 10 runs and the 5 polyMesh files byte-equal | (a different case would differ; shown by S1 (d)) |
| H1 | Headless Studio (`example_projects_h.rs`, model only, no GPU) | each project opens in `Studio::in_memory`: the browser lists 6 (4) cases, 6 (4) runs, 0 skipped; selecting each committed run gives viewport status `Ready` with the store's snapshot count, a shown frame of field `p` at the last snapshot; each generator case opens in the Case editor and validates | the two aerofoil cases open and fail validation in the editor (E1's predicted miss, through the Studio itself) |
| H2 | Real binary (unregistered, ADR 0132's launch smoke `RCFD_STUDIO_SMOKE`) | reported: adapter, run, status, frame, uploads, renders for each project | |

## Not in CI (for the maintainer, a person at the window)

View-and-report checklist:
1. `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`; copy `example-projects\wind-tunnel` to a scratch
   folder; start `target\release\rcfd-studio.exe <that folder>`.
2. The Project panel lists 6 cases, `meshes/naca0012-ogrid...` and `designs/variants.ledger.jsonl`; the Runs panel lists 6
   finished runs.
3. Select the run of `cases/cylinder-re100.toml`: the Viewport shows `p` at once. Pick `vorticity_z`, range "whole run",
   press first, then play: the vortex street forms. Check that the Plots panel shows `cl` swinging and the marker moving.
4. Select the aerofoil run (alpha 4): zoom in on the aerofoil (it is small in a 40-chord far field); the suction peak shows
   in `p`. Check whether the default view is useful or needs a zoom (question 7).
5. Select the step run (Re 800), field `ux`: the reversed flow behind the step and the upper bubble.
6. Select a case in the Project panel and use Case > Validate: the generator cases validate; the aerofoil cases report the
   mesh as missing (E1). Report the exact text.
7. Runs > Start run..., `cases/step-re400.toml`, Start: a new run streams; compare it with the committed one.
8. Open `example-projects\quick-start` the same way; the four runs show at once; start a run of `cases/cavity-re400.toml`.
9. Report what looked wrong, slow or confusing, with the run and the step.

## Out of scope

Side-by-side comparison of runs in the Studio (wave 49); a NACA or step generator in the case schema; a finer or C-grid
aerofoil mesh; validation of any of these flows; the mesh preview, store default and start-and-follow jobs of 48G; the
transonic NACA 0012 condition of the descriptor.

## Amendment 1 (2026-10-28, after the counted rebuild's solver runs, before its post-processing ran again)

What I knew when writing this: the counted rebuild (registration acc912b) ran the six wind-tunnel runs through the run
service and they ended as G1 registers (cylinders `finished` at step 1280, aerofoils `not_converged` at 2000, steps
`converged` at 740 and 429; from the script's printout); the script then stopped in its post-processing with "6
crossings, fewer than 6 periods" for the run of `cases/cylinder-re100.toml`. So the ledger, the run tables, the manifest
and the quick-start project were not written. Nothing else of the runs was looked at.

1. **D8 was defective**: "over the last 6 whole periods of `cl`" needs 7 upward crossings in the second half of the run,
   but at St about 0.15 the second half of an 80 D/U run holds about 6 periods. Corrected rule (for both cylinder runs,
   in the script and in S6's recomputation alike): the shedding statistics are taken over **all whole periods between
   the first and the last upward crossing of `cl` through its mean in the second half of the run**, at least one;
   `strouhal` = (number of crossings - 1) / (last - first crossing time), `cd_mean` the trapezoidal mean of `cd` over that
   window, `cl_amplitude` half the range of `cl` in it. The ledger gains the metric `periods` (unit 1), the number used.
   The solver runs are not repeated: the post-processing runs again on the run folders of the counted rebuild (a new
   script option `--summarise-only`, which writes the ledger and run tables of existing runs), and the quick-start
   project is then built by the script as registered.
2. **G3's crossing count is a predicted miss** for the Re 100 run (6 upward crossings in the second half, limit 7). The
   limit stays; that part of G3 becomes the ignored test `g3_cylinders_cross_at_least_7_times_known_miss`; the other G3
   comparisons are a separate test with their limits unchanged.
3. The cylinder note's sentence on the method follows the corrected rule.

## Questions for the maintainer (open at registration; the addendum may add to them)

1. The stores are binary (about 30 MB in all); every regeneration adds a new copy to the git history. Keep them in the
   repository, move them to a release asset or Git LFS, or regenerate on demand only?
2. E1: the Studio's Case editor validates text with the working folder as base, so a case naming a mesh file by a
   relative path does not validate there (ADR 0131 question 3). Change the editor to create its session with the file's
   path or with the case's folder as base (one call in `crates/rcfd-studio/src/case_edit.rs`, not this job's to edit)?
3. Runs a user starts inside the checkout land in the repository folder. Keep "copy the project out first" (D6, the
   checks ignore extra runs), or should the example folders ignore new runs in git?
4. The aerofoil runs end at their iteration cap (exit 5). The ledger records them as `ok` with the solver state in
   `provenance`. Or `not_evaluated` with a reason (then their metrics may not be used by Pareto or best-so-far queries)?
5. Add NACA 4-digit O-grid and backward-step generators to the case schema, so that these cases need no mesh file and
   validate anywhere (a schema change: every case's settings hash changes, ADR 0133 question 5)?
6. Found, not fixed: a transient run whose `end_time` is not reached exactly by `n x dt` takes a remainder step (here
   3.5e-12 s) whose pressure is spoiled (C_D of -4e5), and that state is the run's final result and last stored snapshot.
   Look at the landing rule (for example, absorb a remainder below a fraction of dt into the last step)?
7. The aerofoil is small in the default fitted view (far field at 20 chords). Should the viewport fit to the wall patches
   of a 2D case, or offer a "fit to patch" control (a Studio change)?
8. Add the two cavity cases to `cases/` (ADR 0139 question 2) so that the case starter offers them too?
9. Should the examples be rebuilt in a scheduled job (the solver changes them slowly), or only by hand?

## Files

This ADR; `example-projects/**`; `scripts/make_example_projects.py`, `scripts/test_make_example_projects.py`;
`.github/workflows/ci.yml` (one line); `crates/rcfd-mcp/tests/example_projects_w48h.rs`;
`crates/rcfd-studio/tests/example_projects_h.rs`.
