# ADR 0840: F-990 registration: structural runs in a project, their records, and the structural layout shown from them

Status: **REGISTERED 2026-11-27, written before any code of F-990 and before any run of its checks.** Before this text no
library, Studio or test code of F-990 was written, nothing was built or run, and no file of the repository was changed.
Results go in addenda at the end; nothing above them is changed afterwards. Written by Claude Opus 5.5 (`claude-opus-5-5`),
as a background agent of the lead.

It answers the brief `docs/adr/briefs/f990-structural-project-run.md` ([the file](briefs/f990-structural-project-run.md))
and follows [docs/process/agent-rules.md](../process/agent-rules.md), version 5. Branch `f990-structural-project-run` from
d99e9283, scratch `w49i-work/f990-structural-project-run-scratch`. It closes E8 of
[ADR 0817](0817-f977-registration-layouts-chosen-by-the-case-category.md) (F-977, question Q2, answered (a)).

## What is asked

Roadmap item F-990 (deps F-977), the binding acceptance, read with a script:

> Every limit not stated here is registered in the ADR before the run. start_run of a solid case file of the repository (crates/rcfd-api/tests/solid_cases/) makes a project run with run.json, case.snapshot, its report and its stored displacement and stress fields; show_view with that run and no layout takes the structural layout, which shows those fields and the report, shown by an e2e test in the open Studio.

Clauses: **a1** `start_run` of a solid case file of `solid_cases/` makes a project run with `run.json`, `case.snapshot`,
its report and its stored displacement and stress fields; **a2** `show_view` of that run with no layout takes the
structural layout, which shows those fields and the report; **a3** shown by an e2e test in the open Studio. From the brief:
the solver unchanged, every default unchanged, no fluid run's records or results changed (**a4**); every test that starts
a runner ends it (ADR 0828).

## What was read

`AGENTS.md`, the rules (version 5) and the brief, whole; ADR 0817 whole; `crates/rcfd-workspace/src/runs.rs` and
`crates/rcfd-runner/src/store.rs` whole; of `crates/rcfd-runner/src/runner.rs` lines 30 to 110 and 577 to 1200 (config,
`estimate`, `solver_cmd`, `launch`, `drain`, `finalize`), of `mesh.rs` lines 1 to 50, of `main.rs` the usage; of
`crates/rcfd-api/src/solid_case.rs` the head, the schema's top level (line 272), `SolidCase` (290 to 535), `run` (1278),
`report_json` (1311), `run_solid_case_file` (1454) and `run_solid_case_any` (1535); the top level of `case_schema`
(`schema.rs` line 66) and the unknown-key diagnostic of `case.rs` (line 75); `runctl::make_event` (line 131); of
`crates/rcfd-tsstore/src/lib.rs` `Location`, `Writer::create` and `SnapshotWriter::field`; `Mesh::from_cell_faces`
(`rcfd-mesh/src/mesh.rs` line 294); the faces of `rcfd-fem/src/mesh.rs` (lines 75 to 92, 150 to 200); the Voigt order of
`rcfd-fem/src/recover.rs` (line 13) and `von_mises` (499); `SolidFields` (`rcfd-post/src/solid_output.rs` line 67);
`cantilever_hex8.toml` and its test `a9_cantilever_hex8` (`sb_solid_case.rs` line 327); of
`crates/rcfd-workspace/src/view.rs` the checks of a view (100 to 520) and the categories and layouts (673 to 940); of the
Studio `Canvas.tsx` (panels, `FieldPanel`, `SeriesPanel`), the cell-only rule of `FieldViewport.tsx` (line 357) and
`worker.ts` (185), `RunsTable`, `canvas-f977.spec.ts` whole, of `tests/support/runner.ts` its head; of
`crates/rcfd-mcp/tests/` `canvas_layout_f977.rs` (head) and `common/mod.rs` (the binaries beside `rcfd-mcp`).
Bytes read: about 250 000.

## Findings of the reading (before any code)

1. **Why no solid run can exist today.** `start_run` of a project case applies the store plan
   (`PLAN_FOR_PROJECT_CASES = true`, `plan.rs` line 30; `runs.rs` line 434), which reads the case as a flow case and fails
   for a solid case file with 1003 "not a valid case, so no start plan". Without the plan the runner's pre-launch check
   runs `rcfd run case.snapshot --check-only --status-json` (`runner.rs` `estimate`, `solver_cmd` line 732), and the flow
   reader refuses every top-level key it does not know (`case.rs` line 75).
2. **The runner cannot link the solid solver.** `rcfd-api` depends on `rcfd-runner` (`rcfd-api/Cargo.toml` line 33), so
   the runner cannot call `rcfd_api::solid_case` (rule 5: the graph stays acyclic). The runner already reads a case's text
   as TOML for its mesh (`mesh.rs`, `case_value`). A program that links `rcfd-api` and is not the CLI is needed:
   `rcfd-workspace` links `rcfd-api`, `rcfd-runner`, `rcfd-tsstore` and `rcfd-post` already (its `Cargo.toml`).
3. **The two schemas share no marker key.** The solid schema's top level is `title, mesh, node_sets, materials,
   constraints, loads, analysis, solver, output, superelements, reduction` (`solid_case.rs` line 272); the flow schema's
   (`schema.rs` line 66) has none of `node_sets, materials, constraints, loads, analysis`. Every solid case of
   `solid_cases/` (9 files) has a `[materials.<name>]` table. So a text with a top-level `materials` is refused by
   `rcfd run` today (finding 1): routing such texts elsewhere changes no run that can run now.
4. **The Studio draws cell fields only** (`FieldViewport.tsx` line 357, `worker.ts` line 185), on the mesh of the store
   (`rcfd_mesh::Mesh`). A solid result is nodal (displacement) and per element (`SolidFields::element_stress`, the element
   mean of the stress, Voigt order xx, yy, zz, xy, yz, zx). The canvas's monitor panel gives `data-last-value`, the last
   value of the series `monitors.<name>` of `residuals.jsonl` (`Canvas.tsx`, `SeriesPanel`); the runner forwards an
   `iteration` event's `monitors` there (`runner.rs` `residual_line`) and records the last values in `metrics.json`
   `reports` (`finalize`).
5. **The case's own test.** `a9_cantilever_hex8` runs `cantilever_hex8.toml` (Hex8 80 x 4 x 4, 1280 elements, 2025
   nodes, static, direct) with `run_solid_case_file` and checks the tip deflection against beam theory (ADR 0433). Its
   report (`rcfd-solid-report/1`) holds `displacement.max_magnitude`, `stress.von_mises_max`,
   `stress.element_von_mises_max`, the reactions and the solver's statistics.

## Design

### D1. Which runs are solid, and the solver path (`crates/rcfd-runner/src/mesh.rs`, `runner.rs`)

A new `pub fn solid_case(text, name) -> bool` in `mesh.rs`: the case text read as `case_value` reads it (TOML, or JSON by
the extension) has a top-level key `materials`. In `Runner::solver_cmd` (`runner.rs` line 732) the program is the solid
driver of D2 when `solid_case` holds for the run's `case.snapshot` (named by `run.json` `case`), else `self.solver` as now;
the arguments are the same as now, so the pre-launch check and the launch of a solid run pass the driver
`run <snapshot> --case-dir <dir> [--check-only --status-json | --out ... --status-file ... --store-dir ...]` exactly as they
pass `rcfd`. The driver is `rcfd-solid-run` next to the runner (`sibling_exe`, as `rcfd` is). The launch record's `solver`
facts name the program launched. By finding 3 every run that `rcfd run` can run takes the path it takes now (a4).

### D2. The solid driver (`crates/rcfd-workspace/src/solid_run.rs`, binary `src/bin/rcfd-solid-run.rs`)

Reads the arguments the runner gives (D1). `--set` entries, `--continue-from` and `--max-steps` are refused. The case is
`SolidCase::parse_toml_in(text, case_dir)` and `build()` (relative mesh files resolve against `--case-dir`).
- `--check-only --status-json`: one JSON line on standard output, `{"ok": true, "cells": <elements>}` (the runner's
  memory estimate counts elements as cells), or `{"ok": false, "diagnostics": [{path, line, message}]}`, exit 2.
- A run: `rcfd_api::solid_case::run(setup)` (the library entry `run_solid_case_file` calls; the solver unchanged), then
  the `.vtu` and the report written into `--out` as `run_solid_case_file` writes them (`output.vtu`, `output.report`
  names; `outputs` with the paths written). For a static result, the field store at `--store-dir`: the mesh of D3, one
  snapshot (step 1, time 0) of cell fields, f64: `ux, uy, uz` and `u_magnitude` (m), `von_mises` and
  `sxx, syy, szz, sxy, syz, szx` (Pa). The status stream (`--status-file`, `rcfd-status` events of `runctl::make_event`):
  one `iteration` event (index 1, time 0, `monitors` `displacement_max_magnitude`, `von_mises_max`,
  `element_von_mises_max` from the report), then `finish` `{state: finished, exit_code: 0, ok: true}`; a failure gives
  `finish {state: failed, exit_code: 2 (case) or 1 (solve, files), reason: the first diagnostics}` and that exit code.
  A modal case writes the `.vtu` and report and no store (its fields are mode shapes); `[reduction]` and
  `[superelements]` cases are refused (not covered).

### D3. The cell fields (first principles)

The store's mesh is the FE mesh as a finite-volume mesh: every element one cell, its faces the kind's local faces
(`ElementKind::faces`, corner nodes only), its points the corner nodes, numbered in node order
(`Mesh::from_cell_faces`). Cell `e` is element `e`. Displacement: the mean of the element's corner-node values. For a
Hex8 whose shape functions are `N_a = (1 + xi xi_a)(1 + eta eta_a)(1 + zeta zeta_a)/8`, the element integral of each
`N_a` over a parallelepiped (constant Jacobian `|J|`) is `|J| * 8 / 8 = V/8`, so the corner mean is the exact element mean
of the interpolated displacement for the box generator's elements. `u_magnitude` is the norm of that mean vector.
Stress: the element mean `SolidFields::element_stress` (Voigt order of `recover.rs` line 13), and `von_mises` of it
(`recover::von_mises`), the quantity the report's `element_von_mises_max` maximises. So the largest cell `von_mises` equals
the report's `element_von_mises_max`, bit for bit. For the cantilever, beam theory (ADR 0433 D2) gives a tip deflection
`w(x) ~ x^2 (3L - x)`: the tip element spans x = 1.975 to 2.0 m, where `w(1.975)/w(2.0) = 0.9813`, so the largest cell
`u_magnitude` over the report's `max_magnitude` is about (1 + 0.9813)/2 = 0.991.

### D4. `start_run` of a solid case (`crates/rcfd-workspace/src/runs.rs`)

In `start_run` with `case`, before the plan: the file's text with `rcfd_runner::mesh::solid_case` true is a solid case.
It is parsed by `SolidCase::parse_toml_in` (its folder); a refusal is 1003 with the solid diagnostics. Given `set`, `store`
`plan`, `run_time`, `dry_run`, `continue_from`, `max_steps` or `site`, it is -32602 ("a solid case file
(docs/solid-case-file.md) takes no ..."), nothing written. Else it is submitted with no plan and no entries, and the
reply is `{run, case, state}` (the reply of a flow start without a store plan). Every other start is unchanged.

### D5. The structural layout (`crates/rcfd-workspace/src/view.rs`)

The structural template list (`templates`, now `Case, Table`) becomes `Case, Field(u_magnitude), Field(von_mises),
Reported([displacement_max_magnitude, von_mises_max]), Table`, where `Reported(names)` is one monitor panel per name whose
series `monitors.<name>` is in `residuals.jsonl`. A structural run folder without a store and residuals (ADR 0817 L2 (d))
keeps `case, table`; no other category changes.

## Checks (rule 14)

Release profile. New tests: `crates/rcfd-runner` unit test in `mesh.rs`; `crates/rcfd-workspace/tests/solid_run_f990.rs`
(the driver binary and library); a unit test in `view.rs`; `crates/rcfd-mcp/tests/structural_run_f990.rs` (the MCP
binary, with `rcfd`, `rcfd-runner` and `rcfd-solid-run` beside it); `apps/studio-web/tests/e2e/structural-run-f990.spec.ts`.
The case is `solid_cases/cantilever_hex8.toml`, copied byte for byte as `cases/cantilever-hex8.toml` into a copy of
`example-projects/quick-start` ("the f990 project"). "The test's report" is the report of `run_solid_case_file` of the
same file in the test's process, and "equal" is equality of the JSON values with `outputs` removed from both.

| Id | Clause | Check | Limit | Source | Estimate | Margin | Kind |
|---|---|---|---|---|---|---|---|
| R1 | a1, a4 | runner unit: `solid_case` of the 9 files of `solid_cases/` and of the 13 case files of `example-projects/` | true for the 9, false for the 13; 22 of 22 | finding 3 | 22 of 22 | exact | code verification |
| D1 | a1 | driver binary run as the runner launches it, on the cantilever | exit 0; last event `finish` finished, exit 0; one `iteration` event with the 3 monitors equal to the report's values; `.vtu` and report in `--out`; the report equal to the test's report | D2; the case's own entry (finding 5) | as stated | exact | code verification |
| D2 | a1 | the store of D1 | 1 snapshot (step 1, time 0); fields exactly the 11 of D2, all cell, 1280 values; mesh 1280 cells, 2025 points; max `von_mises` equal to the report's `element_von_mises_max`; max `u_magnitude` / `displacement.max_magnitude` in [0.97, 1.0] | D3; ADR 0433 D2 (beam theory) | 0.991 | 0.98 at the lower end | solution verification |
| D3 | a1 | the store of D1 | every cell's `ux, uy, uz` equal to the mean of its element's corner-node displacements of the test's run, and every cell's `sxx..szx` to `element_stress` of the test's run; max abs difference | 0 (bit for bit) | D3 (the same formula on the same result) | 0 | exact | code verification |
| D4 | a1 | driver `--check-only --status-json` | the first line `{"ok": true, "cells": 1280}`, exit 0 | D2; the runner's estimate | as stated | exact | code verification |
| N1 | a1 | negative controls of the driver: the cantilever with `youngs_modulus = -1.0`; `--set a=1` | check-only: `ok` false, a diagnostic at `materials.steel.youngs_modulus`, exit 2; run: exit not 0, `finish` failed, no store; `--set`: exit not 0, `finish` failed naming `--set` | D2 | as stated | exact | code verification |
| L1 | a2 | `view.rs` unit: a run folder made by the driver (case, snapshot, store, `residuals.jsonl` of its stream) | panels case, field{u_magnitude}, field{von_mises}, monitor{displacement_max_magnitude}, monitor{von_mises_max}, table; category structural; `view_errors` empty | D5 | as stated | exact | code verification |
| S1 | a1 | MCP binary: `start_run {case: cases/cantilever-hex8.toml}`, `wait_run` | finished within 60 s; `run.json` `case` the file; `case.snapshot` byte-identical to the file; `output/cantilever_hex8.json` equal to the test's report; `output/cantilever_hex8.vtu` present; `fields/` holds the 11 fields; `metrics.json` `reports` the 3 report values; `residuals.jsonl` one line | a1; D1, D2, D4 | as stated | exact | code verification |
| S2 | a2 | MCP binary: `show_view {view: {version 1, run}}` of S1's run | category structural, rule `a solid case file (docs/solid-case-file.md)`, 6 panels; `read_view` gives L1's panels | a2; D5 | as stated | exact | code verification |
| S3 | a1, a4 | MCP binary negative controls: `start_run` of the solid case with `set ["a=1"]`, with `dry_run true`; of a copy with `youngs_modulus = -1.0` | -32602, -32602, 1003 (naming `materials.steel.youngs_modulus`); no new run folder after the three | D4 | as stated | exact | code verification |
| E1 | a2, a3 (F-977 E8) | e2e: the packaged Studio has the f990 project open; the test as MCP client runs `start_run` and `wait_run` of the cantilever, then `show_view {version 1, run}` | the run finishes; within 1.6 s of the tool's result the canvas shows the view with the panels of L1 in order (`data-canvas-kind`, `-field`, `-monitor`) and a title starting `Structural: `; then each panel holds its view: the case row, each field `section.fv` with `.fv-caption` and no `.fv-status`, each monitor `.plot-frame` with `data-last-value` equal to `String` of the report's value, the table's row of the run | ADR 0188 C3-St (4 Studio periods of 400 ms), as ADR 0817 E1 | 0.6 s | 2.7 | code verification |
| L2 | a4 | unedited existing tests: `rcfd-runner --lib`, `rcfd-workspace --lib` (ADR 0817 R1, R2, L1, L2 among them), `rcfd-mcp --lib`; `canvas_layout_f977`, `canvas_view_f976`, `project_list_f270`, `new_simulations_f271` (start_run of flow cases); `cargo test -p rcfd-api --release --test baseline_hashes` | pass | rule 3; brief | pass | exact | unchanged default output |
| L3 | a3, a4 | `npm test` in `apps/studio-web` (every unit and e2e spec, `canvas-f977.spec.ts` unedited; main skips 7) against the branch's engine | pass, 7 skipped | rule 7; G13 | pass | exact | unchanged default output |
| L4 | - | `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` of `rcfd-runner`, `rcfd-workspace`, `rcfd-mcp`; `docs_site`; `roadmap_proposals.py check` of the proposal; `process_report.py --check` | clean; pass (only a generated file not current may fail the last) | rule 7; rule 9 | pass | exact | code verification |

The CPU load is recorded beside each timed run. No shared solver file is touched; `baseline_hashes` is run as the brief
asks.

### Negative controls and mutations (rule 16)

Controls: N1, S3, and ADR 0817 L2 (d) in L2 (a structural run folder without store keeps `case, table`). Mutations, on a
scratch copy of the final commit, each restored afterwards:

| Id | Mutation | Must fail |
|---|---|---|
| M1 | `solid_case` always false (every run to `rcfd`) | S1 (the pre-launch check fails) |
| M2 | the structural template without `Field(von_mises)` | L1 |
| M3 | the cell `von_mises` from the first corner node's nodal von Mises | D2 |
| M4 | the cell displacement from the first corner node instead of the mean | D3 |
| M5 | `start_run` without the solid branch of D4 | S1 (1003 from the plan) |

M3 is committed as `crates/rcfd-workspace/tests/solid_run_f990.mutation.patch`, first line
`# expect-fail: cargo test --release --target-dir target/mutation -p rcfd-workspace --test solid_run_f990`.

## Size

Size (agent): done; 1; 2; 1; 250000

(Expected done; one further registration, an amendment, expected; two heavy runs, `npm test` and the mutations' build;
one question, Q1.)

## Decision rule

**done** for F-990 if R1, D1 to D4, N1, L1, S1 to S3, E1 and L2 to L4 hold and M1 to M5 are each caught; **partial**
otherwise, naming the check that missed or did not run. E1 is F-977's E8, reported for the lead to record. A missed limit is
held as a `*_known_miss` record with its number (rule 2).

## Not covered

- Modal cases in the store (mode shapes), `[reduction]` and `[superelements]` cases, Tet10 and Hex20 midside values (their
  cells use the corners), point fields (the Studio draws cell fields only, finding 4).
- Pause and cancel of a solid run (one solve; the files are accepted and not read); a solid run adopted by a later runner
  is probed by the liveness file, not by the name `rcfd`; a solid run on a remote site (refused).
- The installers: the packaged engine carries `rcfd-solid-run` only where the packaging names it (Q1).

## Open questions for the maintainer

- **Q1. The solid driver in the shipped engine (scope).** (a) the lead adds `rcfd-solid-run` to the binaries the Studio's
  packaging and the installers carry, in a follow-up; (b) later move the solid path into `rcfd run` (a CLI change), and
  drop the driver. Recommendation: (a), since it changes no golden and no CLI. Not taken here (scope, rule 6).

## Amendment 1 (2026-11-27): the invalid copy of N1 and S3, and one dependency

Written after the code of D1 to D5 and before any test of F-990 was compiled, committed or run; nothing above is changed.

1. **The invalid copy.** N1 and S3 name "the cantilever with `youngs_modulus = -1.0`" and expect a diagnostic at
   `materials.steel.youngs_modulus`. Read again before any run: a negative modulus passes the parse (the schema's
   `Schema::float(0.0)` has no bound, `solid_case.rs` line 98) and is refused only by the build, at the key
   `materials.steel` (`Material::isotropic_elastic`, line 868); `start_run` parses and does not build (D4), so S3's limit
   (1003, no run folder) could not hold for that text. The set-up is corrected: the invalid copy of N1 and S3 is the
   cantilever **without its line `youngs_modulus = 2.0e11`**, which the parse refuses at
   `materials.steel.youngs_modulus` ("required", `semantic_errors`, line 603). Every limit of N1 and S3 stays as
   registered.
2. **A dependency.** The driver (D2, D3) names the FE mesh and `recover::von_mises` of `rcfd-fem`, which `rcfd-workspace`
   did not list (finding 2): `rcfd-fem` is added to its dependencies. It is a crate of this workspace below `rcfd-api`
   (which depends on it), so no third-party crate is added and the graph stays acyclic (rule 5).

No other row, limit, source or the decision rule changes.

## Addendum 1: results (2026-11-28)

Written after the runs; nothing above is changed. Release profile, `CARGO_TARGET_DIR` in the scratch folder, 8 build
jobs (other agents built and ran on the machine at the same time). Commits: 63b8bdbe (this registration), 13ef5b71
(amendment 1), af54eb1f (engine and Rust tests), 8400d958 (the e2e spec), 4614b5ec (the committed mutation), then this
addendum and the proposal. Each registered run was made once. Deviations: `cargo fmt` (a write) was run before the code
commit; the code and the new tests were compiled with `--no-run` and the spec type-checked (`tsc -p tsconfig.test.json`)
before the commit (none is a check). The CPU load (Windows `LoadPercentage`) was 80 % at the runner's `--lib` run, 74 % at
the workspace's, 55 % at `solid_run_f990`, 79 % at `structural_run_f990`, 46 % at `baseline_hashes`, 80 % at the start
and 99 % at the end of `npm test`.

### Checks

| Id | Limit | Measured | Met |
|---|---|---|---|
| R1 | 22 of 22 | 9 solid, 13 flow, 22 of 22 (`rcfd-runner --lib`: 37 passed, 1 ignored) | yes |
| D1 | exit 0, finish finished, 3 monitors, outputs, the report equal | exit 0, finish finished exit 0, one iteration event with the 3 values, `.vtu` and report written, the report equal to `run_solid_case_file`'s | yes |
| D2 | 11 cell fields of 1280, 1280 cells, 2025 points; max `von_mises` = `element_von_mises_max`; ratio in [0.97, 1.0] | as registered; max `von_mises` 8.725448871789154e6 Pa, bit for bit the report's; ratio 0.990337 | yes |
| D3 | max abs difference 0 | 0 | yes |
| D4 | `{"ok": true, "cells": 1280}`, exit 0 | as registered | yes |
| N1 | refused at `materials.steel.youngs_modulus`, exit 2; run failed, no store; `--set` named | check-only exit 2 at that key (line 20); run exit 2, finish failed, no store; `--set` exit 2, finish failed naming `--set` | yes |
| L1 | the 6 panels, structural, no view error | as registered (`rcfd-workspace --lib`: 20 passed, ADR 0817 R1, R2, L1, L2 among them, L2 (d) still `case, table`) | yes |
| S1 | finished within 60 s with its records | `r-20261127T235310-181f` finished after 0.54 s; `run.json`, the snapshot byte-identical, the report equal, `.vtu`, the 11 fields, `metrics.json` `reports` the 3 values (displacement_max_magnitude 0.0015527103817975585 m, von_mises_max 11577681.623922525 Pa, element_von_mises_max 8725448.871789154 Pa), one residual line | yes |
| S2 | structural, the rule, 6 panels; `read_view` the panels of L1 | as registered, `studios` 1 | yes |
| S3 | -32602, -32602, 1003 at the key; no run folder | as registered | yes |
| E1 (F-977 E8) | the run finishes; the panels within 1.6 s; each panel's view, the monitors the report's values | `r-20261128T000825-1d33` finished 572 ms after `start_run`; the 6 panels 609 ms after the tool's result; every view, the monitors `0.0015527103817975585` and `11577681.623922525`, at 720 ms | yes |
| L2 | pass, unedited | `rcfd-mcp --lib` 18 passed (1 ignored); `canvas_layout_f977` 1, `canvas_view_f976` 4, `project_list_f270` 1 (1 ignored), `new_simulations_f271` 1 passed; `baseline_hashes` 3 passed (1 ignored) | yes |
| L3 | pass, 7 skipped | `npm test` 908 passed, 7 skipped, 0 failed, 14.4 min | yes |
| L4 | fmt, clippy, `docs_site`, the proposal check, the process report | fmt and clippy clean; the other three were run after this addendum, their results in the report | see the report |

### Mutations

| Id | Caught by |
|---|---|
| M1 (`solid_case` always false) | `structural_run_f990`, at S3 (line 110: the call with `set` was not refused as a solid case, since D4 uses the same rule) before S1 is reached; registered: S1 |
| M2 (no `Field(von_mises)`) | L1 (`view.rs`, `l1_f990`) |
| M3 (cell von Mises from a node; committed as `solid_run_f990.mutation.patch`) | D2 (`solid_run_f990.rs` line 157, the maximum) |
| M4 (cell displacement from the first corner) | D2 (line 165, the ratio left [0.97, 1.0]) before D3 is reached; registered: D3 |
| M5 (no solid branch in `start_run`) | `structural_run_f990`, at S3 (line 110) before S1; registered: S1 |

Each mutation was caught by the registered test file, M1, M4 and M5 by an earlier row of it than the registered one.

Not run: nothing registered was left out. Actual size: done; 1 further registration (amendment 1); 2 heavy runs
(`npm test`, the mutations' build); 1 question; about 260 000 bytes read.

Decision by the registered rule: **done**. Every row held and every mutation was caught. F-977's E8 ran as E1 and held.

### For the lead: roadmap entries

> Status: done.

> Note: ADR 0840, 2026-11-27, done: start_run of a solid case file (a text with a top-level materials table) is run by the runner's solid driver rcfd-solid-run with the arguments of rcfd run; the cantilever_hex8 run holds run.json, case.snapshot, the report (equal to run_solid_case_file's), the .vtu and 11 stored cell fields of displacement and stress, and show_view of it takes the structural layout (case, u_magnitude, von_mises, two report monitors, run), shown in the open Studio 609 ms after the tool's result (limit 1.6 s) with the report's values; 5 of 5 mutations caught, one committed.

> For F-977: E8 of ADR 0817 ran as ADR 0840 E1 on 2026-11-28 and held (the structural layout shown from a project run in 609 ms, limit 1.6 s).

The same entries for F-990 are `docs/roadmap/proposals/0840-F-990.json`.

## Amendment 2 (2026-11-28): the cells of row D3

Written after addendum 1, when `python scripts/process_report.py --check` named row D3 ("kind 'exact' is not one of the
four"); nothing above is changed. Row D3 of the table of checks has one cell too many: its limit was written in two cells
(the description, then "0 (bit for bit)"), so every later cell sits one column to the right and the column kind reads
"exact". The row as registered, in its columns: check "the store of D1: every cell's `ux, uy, uz` against the mean of its
element's corner-node displacements of the test's run, every cell's `sxx..szx` against `element_stress` of the test's
run"; limit "max abs difference 0 (bit for bit)"; source "D3 (the same formula on the same result)"; estimate 0; margin
exact; kind **code verification**. The limit, the measured result of addendum 1 (0) and the decision are unchanged. The
table cannot be edited (rule 18), so the process report keeps naming the row; for the lead: the row is listed as
`0840-f990-registration-structural-runs-in-a-project.md: D3` where the lead lists such rows (the registration-kind
exceptions of `scripts/process_report.py`).

## Answers of the maintainer (2026-11-28), written by the lead

1. Q1: option (a), the lead adds rcfd-solid-run to the binaries the Studio's packaging and installers carry, in a follow-up (F-1003); taken by the lead on the maintainer's standing instruction of 2026-11-26 (go with the recommended option always).

Merged and pushed at 5f22065e. The first push stopped at structural_run_f990: push_main built no rcfd-workspace binary before the selected tests; push_main was fixed at a28a0d22 and the merge pushed again.
