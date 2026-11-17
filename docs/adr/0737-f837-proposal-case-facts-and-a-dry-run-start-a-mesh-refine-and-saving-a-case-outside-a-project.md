# ADR 0737: F-837 proposal: case facts and a dry-run start, a mesh refine, and saving a case outside a project

Status: **ACCEPTED 2026-11-17** (the maintainer took the recommended option of every open question; see "Maintainer
answers" below). **Built 2026-11-17** on branch `adr0737`; see "Built" at the end.

*Proposed 2026-11-17; the proposal below is kept as written.*

Roadmap item F-837 (Electron Studio: run setup, logs and field-view extras from rcfd-studio), ADR 0801 Addendum 13
"Needs the engine" and "Engine follow-ups of the Studio items", rows E-837a, E-837b and E-837c. E-837d is
[ADR 0733](0733-f837-proposal-set-runner-config-the-runner-writes-its-own-budgets.md) (accepted, built); the length unit
F-837 also asks for is
[ADR 0738](0738-f809-proposal-derived-pictures-of-a-stored-field-iso-surfaces-streamlines-slices-and-the-length-unit.md)
P5. The three rows here are the egui Studio's run set-up helpers and its case file handling, which lived in the removed
`crates/rcfd-studio` and must now come from the engine (G2). Parents:
[ADR 0153](0153-job48s-time-step-set-when-a-run-starts.md) (the time step set when a run starts),
[ADR 0156](0156-job48v-adaptive-pressure-correctors.md) and
[ADR 0158](0158-job48x-studio-run-plan-adaptive-pressure-correctors.md) (adaptive correctors),
[ADR 0148](0148-job48n-selected-case-middle-drag-units-scale-refine.md) (Refine x2),
[ADR 0185](0185-adr-0173-amendment-1-f270-registration-the-rcfd-workspace-crate.md) (the save rule), guardrails G2, G4,
G16 and G18. Number 0737 follows ADR 0736.

## Context

### Run set-up facts (E-837a)

- The egui Studio computed, from a case text, what the time step depends on: `time_step::facts`
  (`crates/rcfd-studio/src/time_step.rs:415` at 44e63df8): the kind of run, `solver.dt` and `solver.end_time` and
  whether the text sets them, the case's own step control (`OWN_CONTROL`, 13 keys), the thinnest cell `h_min` (cell
  volume over its largest face area, `empty` patches left out), the fastest speed at the start `u0` (initial field,
  boundary faces at t = 0, or free fall `sqrt(|g| L)`), and the corrector facts (ADR 0158). `time_step::plan`
  (`time_step.rs:485`) turned them and "Simulate for", a Courant target 0.5, a largest step and a mode into `rcfd run
  --set` entries; `warning` (`time_step.rs:573`) warned above an estimated Courant number of 2. All of it went with the
  crate (F-449).
- What the engine has now: `Simulation::new` builds the mesh and the initial state (`crates/rcfd-api/src/sim.rs:1396`);
  `runctl::check_only` returns `{ok, cells, diagnostics, rules, mesh}` without running
  (`crates/rcfd-api/src/runctl.rs:149-205`), which the runner uses for its memory estimate
  (`crates/rcfd-runner/src/runner.rs:543-583`); `rcfd_mesh::adapt::cell_lengths` (`crates/rcfd-mesh/src/adapt.rs:682`);
  the start plan of the field store, its entries, snapshot count and size estimate
  (`crates/rcfd-workspace/src/plan.rs:1-12`, `:47-192`), returned by `start_run` as `store`
  (`crates/rcfd-workspace/src/runs.rs:292-296`).
- `start_run` queues at once (`runs.rs:270-279`). There is no way to ask what a start would do.

### Refine by 2 (E-837b)

- The egui Studio's Refine x2 (ADR 0148; `crates/rcfd-studio/src/preview.rs:989-1119` at 44e63df8) doubled every cell
  count of a generator (`count_keys`, `preview.rs:212-223`: `nx`, `ny`, `nz` and the `_right` counts of the interface
  generators) as unsaved edits, refused a mesh file and generators counted by other keys (`mesh.fan.n_*`), refused a
  count over its schema maximum, and showed cells before and after and the runner's memory estimate.
- The engine has no entry point for it. `rcfd_mesh::adapt::AmrMesh` refines box meshes only (`adapt.rs:1-10`); nothing
  refines an arbitrary mesh file.

### One case file anywhere (E-837c)

- A service session can be made from any sandboxed path (`session.create {path}`,
  `crates/rcfd-api/src/service.rs:1463-1465`; `sandbox_path`, `service.rs:355-380`; no sandbox when `rcfd serve` has no
  `--root`).
- A session can be saved only into a project: the case editor saves with the project tool `edit_case` (base hash, edit
  lock, journal; `crates/rcfd-workspace/src/write.rs:26-160`; the Studio, `apps/studio-web/src/main/case/editor.ts:422`)
  or `save_case` (`crates/rcfd-workspace/src/save.rs:84-100`). Both need a project root (`replace_if_hash_with(root,
  rel, ...)`, `crates/rcfd-project/src/atomic.rs:424`). The service has no method that writes a case; `Case::to_toml`
  and `to_json_string` exist (`crates/rcfd-api/src/case.rs:330-337`).

## Proposal

### P1. Case facts (E-837a)

- Library: `rcfd_api::facts::case_facts(&Case) -> CaseFacts`, the fields of the egui `Facts` above (with `hash`,
  `kind`, `algorithm`, `dt` and `dt_in_text`, `end_time` and `end_in_text`, `own_control`, `write_times_last`, `cells`,
  `two_d`, `bounds`, `h_min`, `h_max`, `u0` and `u0_from`, `correctors`), computed from `Simulation::new`'s mesh and
  initial state; plus `memory_mb` by the runner's rule with the project's budgets, when there is a project. Units in the
  reply (`h_min_unit: "m"`, `u0_unit: "m/s"`).
- MCP project tool `case_facts {case}`, class `read`. Service method `case.facts {session}` (the editor's unsaved
  state), opt-in like ADR 0730's `rcfd serve --ui` (Q1). CLI `rcfd check <case> --facts` prints the same JSON (G16).
- Errors: 1003 with located diagnostics when the case does not build; 1004 over the service's `max_cells`.

### P2. A run-time plan and a dry run (E-837a)

- `start_run` gains `run_time: {simulate_s, courant, max_dt, mode: "adaptive" | "fixed" | "case", adaptive_correctors}`
  (every member optional; the egui defaults 0.5, adaptive, adaptive correctors on). The engine makes the `--set` entries
  by the rule of ADR 0153 and 0158, ported from `time_step::plan` with its constants, and puts them before the call's
  own `set` (as the store plan's are, `runs.rs:263`).
- `start_run {dry_run: true}` returns what the call would do and writes nothing: `{case, set: [..], store: {..},
  run_time: {entries, dt0, steps_estimate, courant_estimate, warning}, check: <check_only's reply>, memory_mb,
  runner: {would_start: bool}}`. Same errors as a real start.
- Without `run_time` and `dry_run`, `start_run` is as now.

### P3. Refine by 2 (E-837b)

- Library: `rcfd_api::refine::count_refinement(&Case, factor) -> Refinement {edits: [{path, before, after}], cells,
  cells_after, memory_mb}` or a refusal (a mesh file; a generator not in the count list; a count over its schema
  maximum; `factor` not 2, Q4). The count list moves from the egui `count_keys` into `rcfd-api`, beside the generators
  it names.
- Service method `mesh.refine {session, factor, apply}` (opt-in as P1): the refinement; with `apply: true` it sets the
  edits in the session (each with `base_value`, ADR 0736 P2) so they are unsaved edits, as in the egui Studio.
- MCP project tool `refine_case_mesh {case, factor, base_hash, reason?}`, class `write_files`: the same edits written
  through `edit_case`'s rule. CLI `rcfd check <case> --refine 2` prints the refinement without writing.
- A mesh file is not refined (Q3).

### P4. Save a session outside a project (E-837c)

- Service method `case.save {session, path, base_hash, form: "whole" | "lines"}` (opt-in as P1): writes the session's
  case to `path` (sandboxed), `.toml` or `.json` by extension, only if the file still has `base_hash` (`fnv64:`; null
  means "no file there yet"), by write-then-rename. `lines` keeps the file's comments and layout where it can
  (`rcfd_workspace::case_text::apply_sets`, the rule of `SaveForm::Lines`, `save.rs:20-30`).
- Reply `{path, old_hash, new_hash, form}`. Errors: 1007 conflict with `data.current_hash` (ADR 0736's code); 1004
  outside the sandbox; 1003 the session's case does not serialise; -32000 the write failed (nothing replaced).
- A path inside a project is refused (1004, "use edit_case") so that project writes keep their journal and edit lock
  (Q5).
- The lock outside a project: an exclusive `<file>.rcfd-lock` created beside the file for the compare and the rename
  (Q6).

### How defaults and `baseline_hashes.rs` stay unchanged

- `case_facts` and `count_refinement` only read; they build the mesh and initial state as `Simulation::new` does and run
  no step. No solver path, default or scheme changes; `crates/rcfd-api/tests/baseline_hashes.rs` is not edited or
  re-recorded.
- `start_run` without `run_time` or `dry_run` writes the same `run.json` as now. A run started with `run_time` is a run
  with `--set` entries, which `rcfd run` already reads.
- The service methods are opt-in, so `service.info` and every method answer as now without the flag. The two project
  tools are a registered restatement of the project tool list goldens, none edited.

## Alternatives

1. **Compute facts and the plan in the Studio.** A copy of the solver's rules in TypeScript (G2). Recommendation: no.
2. **Only `dry_run`, no facts query.** The Studio's set-up line needs `h_min` and `u0` before a start is asked for.
   Recommendation: both; `dry_run` reuses the facts.
3. **Refine a mesh file by splitting cells.** A general polyhedral splitter is a new mesher feature, not a Studio
   follow-up. Recommendation: not now (Q3).
4. **Save outside a project by making a one-file project.** Adds `rcfd-project.json` next to a file the user picked.
   Recommendation: no.

## Open questions for the maintainer

1. **Opt-in of the new service methods.** Recommendation: (a). (a) the same `rcfd serve --ui` family as ADR 0730 and
   0732; (b) their own flag; (c) always on (changes `service.info`, G4).
2. **Who owns the run-time rule.** Recommendation: (a). (a) the engine (`start_run {run_time}`, ported from ADR 0153
   with its constants); (b) the engine gives facts only and the Studio keeps the rule (G2 says no).
3. **Mesh files and Refine by 2.** Recommendation: (a). (a) refused with the reason, as in the egui Studio; (b) a mesh
   splitter as a separate roadmap item.
4. **Factors.** Recommendation: (a). (a) 2 only, as Refine x2; (b) any integer from 2 to 4; (c) also 1/2 (coarsen).
5. **A path inside a project for `case.save`.** Recommendation: (a). (a) refused, pointing to `edit_case`; (b) allowed,
   going through the project's write rule with its journal line.
6. **The lock outside a project.** Recommendation: (a). (a) a `<file>.rcfd-lock` beside the file during the write;
   (b) no lock, compare then rename (a small window); (c) a lock in the user's config folder.
7. **`memory_mb` without a project.** Recommendation: (a). (a) the runner's default budgets, said as such;
   (b) omitted.

## Tests that would prove it

- **C1** `case_facts` of the shipped cases equals the egui `time_step::facts` at 44e63df8 on the same texts (its own
  test fixtures, run against the old commit in a separate worktree once and recorded with the commit named).
- **C2** `start_run {run_time}` gives the entries of the egui `plan` for the same facts and settings; `dry_run` writes
  nothing (the project's file list and `edits.jsonl` unchanged) and its `set` equals the `run.json` `settings.set` of
  the same call without `dry_run`.
- **C3** `count_refinement` of each generator in the list doubles exactly its counts and gives 4x (2D) or 8x (3D) cells,
  equal to `Simulation::new` of the refined case; a mesh file, `axial_fan_sector` and a count over its maximum are
  refused with the reason and nothing set.
- **C4** `case.save` with the right base writes text that `Case::load` reads to the session's `case_hash`; with a stale
  base 1007 and the file unchanged; inside a project 1004; `lines` keeps a comment line.
- **C5** without the opt-in flag, `service.info` equals the base commit's and the new methods are -32601;
  `baseline_hashes.rs` passes unedited; the tool lists equal the restated goldens.

## Unblocks

F-837 rows E-837a ("simulate N s", adaptive correctors and the Courant fix in the run set-up), E-837b (Refine by 2) and
E-837c (open and save one case file anywhere).

## Maintainer answers (2026-11-17)

Every recommended option:

1. (a) The new service methods are behind `rcfd serve --ui`, listed in `UI_METHODS`.
2. (a) The engine owns the run-time rule: `start_run {run_time}`, ported from ADR 0153 with its constants. The Studio
   computes none of it.
3. (a) Refine x2 refuses a mesh-file case and says why.
4. (a) Factor 2 only.
5. (a) `case.save` refuses a path inside a project and points to `edit_case`.
6. (a) Outside a project, a `<file>.rcfd-lock` sits beside the file while it is written.
7. (a) Without a project, the memory estimate uses the runner's default budgets and says so.

## Built

Engine (Rust):

- `rcfd_api::facts`: `case_facts`, `RunTime`, `plan`, `warning`, `CorrectorPlan`, `memory_mb` and `memory_json`, ported
  from the egui `time_step.rs` at 44e63df8 with its constants (Courant target 0.5, warning above 2, at least 100 steps,
  4.7e-6 s per cell step, the 13 own-control keys). Its unit tests are the egui tests with the same numbers.
- `rcfd_api::refine`: `count_keys` (ported from `preview.rs`, plus `box_3d` and the cylindrical-interface annulus) and
  `count_refinement`.
- `rcfd_api::case_save`: the hash compare, the `create_new` lock, and write-then-rename.
- `case_text` moved from `rcfd-workspace` to `rcfd-api` (`git mv`; the workspace re-exports it), with `lines_text`,
  so the service's `case.save {form: "lines"}` and `edit_case` share one rule. `rcfd-api` now depends on
  `rcfd-project` (the manifest and the hash) and `rcfd-runner` (`Config`, the memory rule); the crate layering allows
  both.
- Service, behind `--ui`: `case.facts {session, run_time?}`, `mesh.refine {session, factor, apply?}` and
  `case.save {session, path, base_hash, form?}`. `UI_METHODS` is now `help.glossary, help.errors, case.facts,
  mesh.refine, case.save` after main's `mesh.preview` (ADR 0732): `help.glossary, help.errors, mesh.preview,
  case.facts, mesh.refine, case.save`. Error 1008 `FILE_CONFLICT` is in the catalogue (see below).
- MCP project tools: `case_facts {case, run_time?}` (read) and `refine_case_mesh {case, factor, base_hash, reason?}`
  (write_files, through `edit_case`). `start_run` takes `run_time` and `dry_run`. CLI: `rcfd check <case> --facts` and
  `--refine 2`.

Deviations from the proposal, decided while building:

- The facts field is `case_hash`, not `hash`, as in the session journal.
- The corrector entry is the last of the plan's `entries`, not a separate list. A refused plan answers
  `{refused, correctors}`.
- `refine_case_mesh` writes through `edit_case`, so its journal line names `who: edit_case` and the call's reason.
- `mesh.refine {apply: true}` sets each count with `settings.set` and its `base_value` (ADR 0736 P2, merged from main),
  so a writer that changed a count in between gets 1007. The counts already set by the same call are put back first,
  so the refine is applied whole or not at all. The Studio's undo puts each `before` value back.
- `case.save`'s conflict is **1008 `FILE_CONFLICT`**, not 1007. On main, 1007 is ADR 0736's `SETTINGS_CONFLICT`: a
  session's setting changed since its base, with `data.current_value`. `case.save`'s conflict is a file on disk changed
  since its hash was read, with `data.current_hash`, and its recovery is to read the file again. The meaning differs,
  so it has its own code and catalogue entry. The Studio's outside-project save reads 1008.
- `start_run {from_run}` (ADR 0734) also refuses `run_time` and `dry_run` with -32602, beside its own list (`case`,
  `new_case`, `set`, `store`, `continue_from`, `max_steps`). The run-time plan adds settings, which a rerun takes from
  its record. A dry run reads a case file, and a rerun has none.
- `case_facts` is not on the Claude Code first-use allow-list; that was not asked (ADR 0188 Q6 (a)).
- C1 is covered by the ported egui unit tests and hand-computed facts, not by a recording of the old commit. That
  follows ADR 0730: no frozen recordings.

Studio (apps/studio-web):

- The case editor's estimate line comes from `case.facts`: cells, the thinnest cell, the time step, the steps, the
  Courant estimate and the memory. Its tooltip names the runner's default budgets when there is no project, and the
  engine's warning is shown as an exception. It is asked again after every change.
- Refine x2 is an icon button that dispatches `mesh.refine {apply: true}`. The counts become unsaved changes and can be
  undone. A refusal shows the engine's reason.
- Open a case file outside a project: `#/case/file`, the palette action `case.open_file`, an icon in the editor's top bar
  and a button on `#/case`. The main process shows the dialog and binds only files it gave. The hash is the one the
  service's journal recorded.
- A save of such a file is the service's `case.save {form: "lines"}`. A file changed outside is a conflict (1008) with
  both sides shown. "Write my changes into the file as it is now" sets this session's changes in a fresh session of the
  file and saves that, so the other writer's other changes stay (G18).
- `case.facts` and `mesh.refine` are on the allow-list, each with a registry action. `case.save` is not: only the main
  process calls it.
- The run-time rule: the Studio had no copy of it, so there was nothing to remove. A unit test checks that none of its
  constants appear in the Studio's source.
- **Decision: the Studio starts runs with the store plan.** The Studio's Run, Run again and Queue now call
  `start_run {store: "plan"}` (ADR 0140: about 100 f32 snapshots, unless the case or a `set` entry gives the store). The
  reason is the maintainer's report of 2026-11-17: `cylinder_re100_shedding.toml` (dt 0.02, end_time 200, no
  `[output]`) ran from the Studio and stored no field, and the workspace could only say so, a dead end.
  - A case that sets `output.store_interval` or `output.store_time_interval` keeps its own store.
  - The case file is not changed.
  - `start_run`'s own default (`none`, ADR 0186 Q2) and the CLI's stay.
  - The start line says what the plan applied: about N snapshots and the size, from the reply's `store`.
  - Queue's store `none` stays an explicit choice.

Tests:

- `crates/rcfd-api/tests/case_facts_adr0737.rs`: 8 tests covering C1 to C5 for the service.
- `crates/rcfd-mcp/tests/case_facts_adr0737.rs`: 3 tests. They cover the project tools and C2, including a real run
  whose `run.json` `settings.set` equals the dry run's `set`.
- The `facts` unit tests in `rcfd-api`.
- Restated, with no golden edited: `ui_byte_identity_adr0730.rs`, `project_list_f270.rs`, `tool_list_f275.rs`,
  `project_w48f.rs` and `project_runs_w48f.rs`.
- Studio unit tests: `tests/unit/case-file-adr0737.spec.ts` (9 tests).
- Studio restated tests: `actions-registry`, `run-control` and `f837` (the store plan) unit specs, and the `case-editor`
  e2e spec (the case port's keys).
- Studio e2e: `tests/e2e/case-file-adr0737.spec.ts`, 4 tests against `rcfd serve --ui`, with screenshots in
  `test-results-visual/adr0737/`. Restated: `run-control.spec.ts` (store plan).
- `word-budget.spec.ts` passes with the budgets unchanged.

## Change of 2026-11-17: a mesh-file refusal has its own code, 1009 MESH_FILE

The maintainer approved this after Refine x2 of a mesh-file case in the Studio was refused with 1004. The catalogue's
line for 1004 (ADR 0730) is about paths outside the service's root and the limits of `service.info`, which does not fit
this refusal.

- Refine x2 of a case whose `mesh.source` is `file` is now **1009 `MESH_FILE`**, the next free code of the service's
  1000 range (1001 to 1008 are taken; the project tools' codes start at 1200 and the MCP server's at 1100). It is
  `rcfd_api::service::code::MESH_FILE` and `rcfd_api::refine::Refusal::MeshFile`. The message is unchanged.
- The catalogue (`help.errors`, `rcfd help errors`) has its row: "the case's mesh is a file, which Refine x2 does not
  refine". The recovery line is "Refine the mesh file in the program that made it, or switch the case to a generator mesh
  (mesh.source = generator)."
- The same code is given on every path that refines: the service's `mesh.refine` and the MCP project tool
  `refine_case_mesh` (the project tools reuse the service's code, as they reuse 1003 and 1004). `rcfd check <case>
  --refine 2` still exits 3, as its other refusals do. Under the reason it now prints the catalogue's line of 1009,
  because exit 3's own line is about invalid keys.
- The other refusals of a refine stay 1004: a generator counted by other settings (`axial_fan_sector`), a count over
  its schema maximum, and a mesh over the service's `max_cells`. Every other use of 1004 is as before.
- Studio: the "Refine x2 refused" alert and the estimate line's "No estimate" show the catalogue's line for their code
  under the engine's message. For a mesh-file case the alert's line is 1009's. The estimate of a mesh-file case fails
  only when the mesh file does not build (1003, with its line), because `case.facts` reads a mesh file as it is.
- Docs: the error table and the `mesh.refine` entry of `docs/service-protocol.md`, and the `refine_case_mesh` row of
  `docs/mcp-tools.md` and the tool's description.
- Defaults and `baseline_hashes.rs` are unchanged: only an error code of a refusal changes, and no solver path.

Tests, restated with no golden re-recorded:

- `crates/rcfd-api/tests/case_facts_adr0737.rs` `c3_refusals_and_apply`: the mesh file is 1009 (it was 1004); the
  other two refusals stay 1004.
- `crates/rcfd-mcp/tests/case_facts_adr0737.rs` `refine_case_mesh_writes_through_edit_case`: 1009 for the mesh file.
- `crates/rcfd-api/tests/errors_catalogue_adr0730.rs`: 14 constants in `service::code` (it was 13).
- New: `crates/rcfd-cli/tests/check_refine_adr0737.rs`, which checks the exit 3 with the reason and 1009's line, and a
  generator case's edits.
- Studio e2e: `tests/e2e/error-walk.spec.ts` provokes 1009 (Refine x2 of a mesh-file case in the quick-start copy) and
  finds its line. `tests/e2e/case-file-adr0737.spec.ts` finds 1009's line under the refusal, and 1003's line under
  "No estimate".
