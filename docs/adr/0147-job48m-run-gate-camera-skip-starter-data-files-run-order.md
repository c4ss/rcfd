# ADR 0147: Job 48M, follow-ups of the live preview: Run waits for a typed field, the camera stays on a rebuild, an identical text is not built again, the starter copies a case's data file, runs ordered by creation, and the runner's log tail shown once

Status: **registered 2026-10-28**, before any registered run (the code and the tests below were written and compiled
first; what had run before is listed under "What I already knew"); amendment 1 (M1's hover set-up) before its corrected
run, amendment 2 (M7 (b) a Windows fixture); results in the addendum: every check met its limit, no known miss; not yet
accepted by the maintainer. Job 48M: the maintainer's answers to the questions of
[ADR 0144](0144-wave48j-mesh-run-toggle-and-live-preview.md) (the Mesh / Run toggle and the live preview), and the extra
scope of [ADR 0146](0146-job48l-runner-output-early-exit-windows-inherited-handles.md) question 5. On main ec6a443 (48J,
ADR 0144, and 48L, ADR 0146, merged). House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are
fixed here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a
defective set-up gets an amendment written before the corrected run; defaults of existing code stay bit-identical outside
the Studio; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; no new crate, no unsafe code, the crate graph
stays acyclic. Clean-room: our own Studio code and the egui public API.

## The maintainer's decisions (2026-10-28, relayed)

* **Q2** (Run while a field is typed): grey Run out, with the hover text "finish editing the field", while a field of the
  case editor is being typed and not committed, so that the picture and what Run would use cannot differ. The existing
  "save it first" rule stays.
* **Q5** (the camera on a rebuild): keep the camera when the case and its bounds are unchanged (typing `nx` and the like
  must not reset it); refit only when the bounds change.
* **Q6** (an edit that changes nothing): skip the mesh build when the (unsaved) text is identical to the one shown (and the
  mesh-relevant content is unchanged).
* **Q1** (the start dialog's `--set` overrides in the preview): kept out of the preview. No change.
* **Q3, Q7, Q9** (the run's view when returning to Run, the 500 ms debounce, the labels `Mesh` / `Run`): ADR 0144's
  recommended defaults: (a) the run's view starts again, 500 ms, the labels kept. No change.
* Three small extras:
  * (a) the case starter also offers `channel/profile_inlet.toml`, copying its table alongside (ADR 0139 question 3, ADR
    0144 question 11). The copy mechanism is this job's to decide and register (decision 4).
  * (b) runs ordered by their `created` time (`run.json`) instead of by id, ties broken by id, so that runs of one second
    sort deterministically (ADR 0144 question 12); check `rcfd-project`'s listing too, and that the existing tests and
    orderings stay valid.
  * (c) the one-line "not a case file" message for a benchmark descriptor also in the status bar of the case editor
    (ADR 0144 question 4).
* **Extra scope** (relayed later the same day, after 48L merged): after ADR 0146's early-exit report the Run button's error
  can show the runner's log tail twice, and two comments in `crates/rcfd-studio/src/quick_run.rs` are stale (ADR 0146
  deviation 6, question 5). Show the tail once and fix the comments; register a check first with the current code as its
  control.

## Context (read at main ec6a443)

* `quick_run::blocked` refuses Run for a case with unsaved committed edits ("save it first") or a conflict; a field still
  being typed (`CaseEditor::draft`) is not an edit to it (ADR 0144 question 2). F5 does what the button does.
* The preview hands every new surface to the viewport through `Viewport::show_surface`, which resets the camera to
  `CameraState::start` of the surface. A rebuild that gives the same mesh keeps its surface (the same `Arc`), so the
  camera stays; any other rebuild resets it (ADR 0138's rule, ADR 0144 question 5).
* A 2D generator mesh is one cell thick, and that thickness is the cell size: `cavity_2d_rect` and `box_2d_graded` make
  `zs = axis_coords(1, lx / nx)` (`crates/rcfd-mesh/src/gen.rs`). The drawn front of a 2D case (`surface_of`, its +z
  `Empty` faces) therefore lies at `z = lx / nx`, so typing `nx` moves the front's z while its x and y extents stay.
* Generator coordinates are `len * i / n` (`geometric`), so the end of an axis can differ from `len` by a rounding step
  for some `n`.
* The debounce, a change on disk (the watcher or the editor's base hash: a save, a reload), "Rebuild", "anyway" and a newly
  asked case all ask for a build the same way (`Preview::want`); every build of the editor's text builds the mesh, also
  when the text equals the text of the mesh on screen.
* The case starter excludes `channel/profile_inlet.toml` and `channel/profile_inlet.csv` (`case_starter::EXCLUDED`).
  The case reads `[tables.inlet_u] file = "profile_inlet.csv"`, relative to the case's folder, which the Studio's editor
  resolves since ADR 0144 amendment 3. `Example::describe` parses an example with `Case::parse_toml`, which reads table
  files from the working folder.
* `Project::cases()` lists every file under `cases/`, so a copied table shows in the Project panel's Cases list.
* Run order, read from the code: `Project::runs` (`crates/rcfd-project/src/project.rs`), `Browser::refresh`, the run
  worker's rows (`poll.rs`), `RunsView::extra` and the queue positions already sort by (`created`, id), the queue by
  priority first, as the runner's `schedule` does. By id only: the launch smoke of `main.rs` (`RCFD_STUDIO_SMOKE`), which
  picks "the newest run" as the largest id; `Project::run_ids` (documented as sorted by name, used by `runs()` before its
  sort); outside the Studio, `rcfd-runner list` (`Client::list`, `FsStore::list_runs`, name order). `created` is
  `rfc3339_ms` for runs made by `rcfd-project` and `rfc3339_ns` for runs made by the runner client.
* The case editor's message for a benchmark descriptor is the service's list of unknown settings (`unknown setting 'id'
  (known: ...)`; ...), shown as the status bar's last message (`Studio::apply`, `Event::Loaded`).
* `quick_run::start_job` appends the last 3 lines of `.rcfd/runner.log` to every `ensure_runner` error. Since ADR 0146 the
  client's early-exit report already ends with the runner's output (`the runner (pid N) exited with exit code C before it
  held the project's lock; its output (<runner.log>): <tail>`, or `it wrote nothing to ...`), so the same lines show twice.
  Run checks the budgets file before the start, so a bad budgets file is refused before the runner starts; the twice-shown
  tail needs another early exit (deviation 6 of ADR 0146).

## Decisions (as implemented)

1. **Run waits for a typed field** (`quick_run::blocked`, `FINISH_EDITING`). While the open case's editor holds a field
   being typed (`CaseEditor::draft`), Run is refused with `<case>: finish editing the field <path> (press Enter or click
   elsewhere); the mesh shows the typed text, but a run uses the file on disk`; the button is greyed out with that hover
   text (`quick_run_ui::toolbar`, unchanged: it shows `blocked`), F5 is refused with `Run: ` and that text in the status
   bar. Checked after a conflict and before "save it first": a committed edit still gets ADR 0141's text. A refused
   commit (`1e`) keeps the field being typed (ADR 0144), so Run stays greyed out until the text is corrected. Test switch
   `QuickRun::switches.run_while_typing` (the route before). The Start run dialog (Options...) is not changed
   (question 4).
2. **The camera on a rebuild** (`Viewport::replace_surface`, `viewport::same_bounds`, `preview::install`). A new surface of
   the case whose surface the viewport holds (`Preview::installed`, the case of the last install; the viewport still
   showing it) is installed with:
   * the camera **kept** when the bounds are the same: target, distance, yaw, pitch and projection unchanged; near and far
     follow the new box (`clip_planes`). "The same bounds" (`same_bounds`): in 2D the x and y extents, in 3D all three, each
     within `SAME_BOUNDS` = 1e-6 of the larger box's size. In 2D the z of the front is left out because it is the cell size
     (Context), so typing `nx` would otherwise always refit; 1e-6 of the box is at most 1/100 of a pixel of the largest
     texture (8192 px) and above the rounding of `len * i / n`.
   * the camera **refit** when the bounds changed: `viewport::fit` of the camera, the double click's Fit: yaw, pitch and
     projection kind kept, framed to the new box. This is my reading of "refit" (question 1).
   * the **start camera** when the 2D flag changed (orthographic face-on and perspective are different pictures), for
     another case, and when the viewport showed nothing before (a project opened, the Run side shown in between: the view
     starts again, as ADR 0144 question 3 (a) has it for the run).
   Counters `Stats::camera_kept`, `camera_refit`. Test switch `preview::Switches::restart_camera` (ADR 0138's rule).
3. **An identical text is not built again** (`preview::build_source_unless`, `Refusal::Unchanged`, `mesh_input_unchanged`).
   A build that the debounce alone asked for (no new case, change on disk, save or reload, "Rebuild" or "anyway" in the
   same request) of the editor's text is compared, on the build thread, with the mesh on screen (`Preview::shown`): when
   the bytes it would parse hash to the shown mesh's `case_hash` and the shown mesh's input is unchanged, nothing is parsed
   or built (`Refusal::Unchanged`); the shown mesh is then the current one (ready, not stale) and `Stats::skipped_identical`
   counts it. "The mesh-relevant content is unchanged" is read as the part outside the text: a recipe mesh has none (the
   text is its whole input); a mesh file (`mesh.source = "file"`) must have the size and modification time read before
   the shown mesh was built (`Built::mesh_stamp`); a mesh folder (OpenFOAM) is never taken as unchanged. The comparison runs
   on the build thread because the editor's bytes need file reads (tables), which the UI thread does not do (ADR 0144
   decision 3). Requests other than the debounce's are never skipped: they are the ways to read files again (question 2).
   A result whose shown mesh changed in between ("show anyway") asks for a build after all. Test switch
   `preview::Switches::no_skip` (ADR 0144's rule).
4. **The starter copies a case's data file** (`case_starter::Companion`, `create_with`, `target_line`; the maintainer's
   extra (a), the mechanism decided here):
   * `channel/profile_inlet.toml` is offered (480 cells, 2D, cheapest-first between `packed_bed` and `wedge`), with
     `channel/profile_inlet.csv` as its **data file** (`Example::companions`). Both are embedded (`include_str!`, as ADR 0139
     D2). `EXCLUDED` keeps the 6 descriptors.
   * The data file is written next to the new case under its own file name, `cases/profile_inlet.csv`, so the case's text
     stays byte-equal to the example (ADR 0139 D3) and its relative `file = "profile_inlet.csv"` finds it.
   * On the project worker, before anything is written: the case file must not exist; a data file that exists with the
     example's bytes is used as it is (a second case from the same example shares it, not rewritten); one with other bytes
     refuses the creation (`cases/profile_inlet.csv exists with other content: <case> would read its table from that file;
     rename or remove cases/profile_inlet.csv, then create the case again`), nothing written. Then the missing data files,
     then the case, each with `write_new` (never replaces); when the case cannot be written, the data files this call wrote
     are removed again.
   * Messages: `created <case> and its data file cases/profile_inlet.csv`, or `created <case>; its data file
     cases/profile_inlet.csv was in the project already, with the example's bytes`. The dialog's line: `creates <case> and
     its data file cases/profile_inlet.csv from the example channel/profile_inlet.toml` (when the data file is listed
     already: `...; cases/profile_inlet.csv is in the project already: used when it holds the example's bytes, else the
     creation is refused`).
   * `Example::parse` (for the dialog's description) gives a data file's text inline (`data` in place of `file`), so no
     file is read.
   * The data file is listed among the Project panel's cases (`Project::cases` lists every file under `cases/`); not
     changed here (question 3).
   * Test switch `view_toggle::Switches::starter_no_companions` (the case written alone, the route before).
5. **Run order** (`project_view::run_order`, `newest_run`). The launch smoke picks the newest run by (`created`, id), not
   the largest id. `Browser::refresh` uses the same comparator as before, now named. Everything else the Studio lists, and
   `rcfd-project`'s listing, already ordered by (`created`, id) and is not changed; M5 checks them with runs of one second
   and a tie. `rcfd-runner list` keeps its id order (outside the Studio; question 5).
6. **The status bar for a benchmark descriptor** (`Studio::apply`, `Event::Loaded`): when the editor cannot open a file
   and the file is a benchmark descriptor (`preview::descriptor_message`), the status bar gets that one-line message
   instead of the editor's list of unknown settings. Test switch `preview::Switches::editor_lists_descriptor_keys`.
7. **The runner's log tail once** (`quick_run::runner_error`, `reports_runner_output`; the extra scope). The last lines of
   `runner.log` are appended to an `ensure_runner` error only when the error does not already report the runner's output,
   that is when it lacks ADR 0146's early-exit marker ` before it held the project's lock; `; the timeout, the exit 3
   race and a failed spawn keep the appended lines. The two stale comments (the module doc and the budgets check in
   `start_job`) now say that the budgets check refuses before a run is queued, while a runner that exits on a bad file is
   reported by `ensure_runner` with its output after the run was queued. Test switch `QuickRun::switches.tail_always` (the
   route before).
8. Not done (the decisions): no overrides in the preview (Q1); the run's view starts again on the Run side (Q3); the
   debounce and the labels unchanged (Q7, Q9).

## Placement

`crates/rcfd-studio` and `docs/`. Hunks in `src/{preview,viewport,quick_run,case_starter,case_starter_ui,watch,studio,
view_toggle,project_view,main}.rs`. No new crate, no new dependency, no new workspace edge; `Cargo.toml`, `Cargo.lock`,
`crates/rcfd-cli/tests/workspace_graph.rs` and `docs/architecture.md` unchanged. Nothing outside the Studio changes:
`rcfd-project`, `rcfd-runner` and the other crates are read, not edited.

## Registered checks (limits fixed here, before any of them ran)

Release builds, `--test-threads 1 --nocapture`, output to files, this host; process checks with `rcfd-runner` and `rcfd`
of the same build next to the test's `rcfd-studio` (`common::binaries`). "Typing" in model checks is
`CaseEditor::type_text`; M1 types through egui events into the drawn field (as ADR 0144 J10). "Settled": no debounce
pending and the preview not building. Fixtures: ADR 0138's project (`pv::project`: the 16 x 16 cavity, the 4 x 4 x 4 hex
box through `mesh.file = "../meshes/hex.msh"`), ADR 0133's `RunnerProject` (runner budgets `max_runs` 3, `cpu_threads` 8,
`poll_ms` 100), and the cases written by the tests. Files: `crates/rcfd-studio/tests/followups_m.rs` (M1, M2, M3, M5, M6,
M7 (a), (c)) and `crates/rcfd-studio/tests/starter_run_m.rs` (M4, M7 (b)).

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| M1 | Run and the case editor's field, drawn app (the cavity, watcher and run worker off, the editor filtered to `mesh.nx`, tooltip delay 0): (a) opened; (b) `20` typed into `mesh.nx` (two Backspaces, `2`, `0`), the field still being edited; hover on Run; F5; a click on Run; (c) the field committed (Enter, unless the click before took the focus); (d) saved; (e) `5` typed and deleted, then Enter; (f) `1e` and Enter (refused), then corrected to `20` and Enter; (g) every state where Run is enabled | (a) `blocked` none, the hover draws the start text; (b) the draft is `("mesh.nx", "20")`; `blocked` starts `cases/cavity.toml: finish editing the field mesh.nx`; the hover draws exactly that text; F5 starts nothing and the last message is `Run: ` and that text; the click starts nothing (whether it committed the field is reported); (c) no draft, the settings' `mesh.nx` 20, `blocked` contains `save it first`; (d) none; (e) `finish editing the field` while typed, none after Enter, no draft, clean; (f) the draft `("mesh.nx", "1e")` kept, `finish editing the field`; none after the correction; (g) in each of the 4 enabled states ((a), (d), (e), (f)) `preview::current_source` is `Disk` | the route before (`run_while_typing`): in (b)'s state `blocked` is none, the hover draws the start text, and the preview's next source is the editor's text |
| M2 | The camera across rebuilds (model, no GPU): (a) the cavity opened, then panned 30, -20 and dollied 2; (b) `nx` typed `20`; (c) `nx` committed, `mesh.lx` typed `2`; (d) a 3D `annulus_3d` ring (12 x 2 x 3, r 0.5-1.0, lz 1) opened, orbited 40, 25; `nz` typed `4`; `nz` committed, `mesh.r_out` typed `1.5`; (e) the cavity opened again, panned, `mesh.source = "file"` and `mesh.file = "../meshes/hex.msh"` committed; (f) orbited, a finished run selected (Run side), then the toggle to Mesh | (a) the start camera of the surface; (b) 320 cells shown; target, distance, yaw, pitch and projection bit for bit the panned camera's; the camera's box the new surface's bounds; the camera equal to `clip_planes(panned, new bounds)`; `camera_kept` + 1, installs + 1 (the front's z before and after reported); (c) the camera equal to `fit(camera before, new bounds)` and its view changed; `camera_refit` + 1, installs + 2 since (a); (d) the ring's start camera; after `nz`: the orbited view bit for bit, `camera_kept` + 1; after `r_out`: `fit(camera before, new bounds)`, yaw and pitch unchanged, perspective; `camera_refit` + 1; (e) the hex box (64 cells, 3D) with its start camera, neither counter moves, installs + 1; (f) the start camera of the shown surface | ADR 0138's rule (`restart_camera`): after (b) the camera is not the panned one, `camera_kept` 0 |
| M3 | Identical text (model): (a) the cavity: `nx` typed `20`; (b) the field committed; then `2`, 100 ms later `20` typed; (c) `24` typed; (d) "Rebuild"; (e) the hex box: `physics.nu` typed `0.02`; the mesh file's modification time moved 10 s ahead (bytes unchanged) and the field committed; then `0.03`, 100 ms later `0.02` typed | (a) built (320 cells, from the editor), `meshes_built` + 1; (b) the commit asks for one build of the editor's text (`editor_requests` + 1) and it is skipped: `skipped_identical` + 1, `meshes_built` unchanged, ready, not stale, the same `Arc` shown and in `built`, no install; the burst: skipped (+1); (c) built (384 cells); (d) built (`meshes_built` + 1, `skipped_identical` unchanged); (e) the typed value built (`mesh_stamp` set); the commit after the time moved: built, no install (the same mesh); the burst: skipped (+1), nothing built | ADR 0144's rule (`no_skip`): (b)'s commit builds (`meshes_built` + 1, `skipped_identical` 0) |
| M4 | The starter's `channel/profile_inlet.toml` end to end (runner served, the drawn app with the cavity open): (e) the dialog asked for with the example selected and the name `profile_inlet`; (a) `create_case_from_example(profile_inlet, "pi")`; (b) Run clicked; (c) a second case `pi2`; (d) `cases/profile_inlet.csv` overwritten by the test with other bytes, then `pi3` | (e) the drawn line is exactly `creates cases/profile_inlet.toml and its data file cases/profile_inlet.csv from the example channel/profile_inlet.toml`; (a) within 30 s (ADR 0139 CS8's wait) the editor holds `cases/pi.toml` and the preview settled on it; `cases/pi.toml` and `cases/profile_inlet.csv` are byte-equal to the repository's files; both listed; the message `created cases/pi.toml and its data file cases/profile_inlet.csv`; the editor clean with the source's hash, `validate()` true with no diagnostic; the preview ready, 480 cells, 2D, the mesh hash of `build_mesh(Case::load(<project>/cases/pi.toml))`, Mesh side, installed; (b) the new run selected within **5.0 s** of the click, `finished` (`status.json`) within **60 s**, its store's mesh hash equal to the preview's; (c) the message `created cases/pi2.toml; its data file cases/profile_inlet.csv was in the project already, with the example's bytes`; `cases/pi2.toml` byte-equal; the data file's bytes and modification time unchanged; (d) a message containing `cases/profile_inlet.csv exists with other content`; no `cases/pi3.toml`; the data file holds the test's bytes | the starter's route before (`starter_no_companions`), a fresh served project: `cases/pa.toml` written and no data file; the editor does not open it (a message names `tables.inlet_u.file`); a run of it (`Client::start`, default settings) fails within 60 s with a reason naming `tables.inlet_u.file` |
| M5 | Run order with runs of one second: A `r-20261028T120000-ffff` created `...12:00:00.100Z`, B `...-0000` and C `...-aaaa` both `...12:00:00.200Z`, D `r-20261028T115959-9999` `...11:59:59.900Z` (made by `Project::create_run`, given these ids and times, moved into `runs/` whole). (a) `Project::runs`; (b) the Studio's browser and the run rows drawn in the Runs panel; (c) `Browser::refresh` after A's `status.json` is named changed; (d) a second project opened empty (watcher off, run worker on), the runs moved in: `RunsView::extra`; (e) `project_view::newest_run`; (f) five more `Project::runs` | (a)-(d) and (f) exactly D, A, B, C; (e) C | the id order, D, B, C, A, differs, and its largest id (A) is not the newest |
| M6 | The status bar for a benchmark descriptor: the 6 shipped `cases/<id>/case.json` copied into a project, each opened through the Studio model | each: the editor does not open it; the last message equals `preview::descriptor_message(<file>, <text>)`, one line (`<file>: not a case file: this is a benchmark descriptor (id "<id>") ...`); the last one drawn in the status bar | a JSON case with a bad value (`cases/bad.json`, `mesh.nx` `"abc"`) keeps the editor's located message (starts `cases/bad.json: `, names `mesh.nx`, no "not a case file"); the route before (`editor_lists_descriptor_keys`) gives a message that is not the short one and contains `unknown setting` |
| M7 | The runner's log tail in a failed start. (a) ADR 0146 E1's set-up (`runner-config.json` `{"max_runs": 0}`), `Client::ensure_runner` of this build, then the Studio's text (`quick_run::runner_error`); (b) Run on a `RunnerProject` (not served) whose `.rcfd/runner.lock` is a folder, so the runner ends at its lock after the Studio's budgets check passed; (c) the text of an error without the runner's output (the 10 s timeout's wording) on a project whose `runner.log` holds 3 lines | (a) the client's error is an early-exit report; the Studio's text equals it and holds `must be positive` exactly once; (b) within **5.0 s** of the click the start's error (`QuickRun::runner_error`, the strip's line) holds ` before it held the project's lock; ` and `rcfd-runner: runner lock: ` exactly once, and is drawn; (c) the text starts with the error and holds `; last lines of ` once and the log's last line once | the route before (`tail_always`): (a)'s text holds `must be positive` twice; (b)'s holds `rcfd-runner: runner lock: ` twice |
| M8 | The rest: the whole `rcfd-studio` suite (ADR 0131-0144 checks, ADR 0139's with amendment 2 below), `workspace_graph`, `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings`, `cargo fmt -p rcfd-studio -- --check`, `scripts/third_party_licenses.py --check` | pass; the four known misses ignored as before (ADR 0131 B9, ADR 0133 R6, ADR 0141 G1, ADR 0144 J3b) | (their own) |

Derivations. M4 (b)'s 5.0 s is ADR 0141 G1 (a)'s; its 60 s is G1 (b)'s 20 s to running plus the run (480 cells, at most
2000 iterations; the case converges well before, ADR 0089 and `docs/component-diagonal-default.md` record 172), generous on
purpose. M7 (b)'s 5.0 s is ADR 0141 G4's (a refused start shown in the strip; ADR 0146 E1 measured the early exit itself
at 0.042 s). The 30 s waits are ADR 0139 CS8's. Everything else is exact.

How the registered runs are made: `cargo test --release -p rcfd-studio --test followups_m -- --test-threads 1 --nocapture`,
the same for `starter_run_m` and for the changed `case_starter_g2`, each output to a file; then the whole `rcfd-studio`
suite once with `--no-fail-fast -- --test-threads 1 --nocapture`, output to a file (M8); clippy, fmt, `workspace_graph` and
the licence check on the final commit.

## Changes to earlier tests (registered here, none run yet)

* ADR 0139's CS1, CS2, CS2 control, CS5 and its controls, CS6, CS7, CS11, CS12, CS13 and its control, and the unregistered
  `panel_button_request_repaints_and_opens_the_dialog_next_frame`: [ADR 0139 amendment 2](0139-wave48g2-case-starter.md),
  written with this registration: `channel/profile_inlet.toml` is offered with its data file. Limits unchanged in kind.
* ADR 0144's J2 (e) reports Run with a field being typed and limits only the committed state ("save it first"); the
  typed state now reports the new refusal. Not changed.

## What I already knew (rule d)

* The code reading above.
* **Baseline before any change of this job**, on main 84ee864 (48J merged, before 48L): a release build in this job's
  target folder (4 min 14 s with `rcfd-runner`, `rcfd`, `fake-claude` and the test binaries), then `cargo test --release -p
  rcfd-studio --no-fail-fast -- --test-threads 1`, output to a file: **175 passed, 0 failed, 4 ignored** (the four known
  misses), 447 s.
* Main moved to ec6a443 (48L, ADR 0146, merged) while I wrote the code; the branch was fast-forwarded to it before any
  check of this job ran (48L changed `rcfd-runner` and tests outside the Studio, no Studio file). ADR 0146's addendum
  records the whole `rcfd-studio` crate on its branch (154 passed and 3 failures that 48J's merge fixed); this job's
  baseline above did not use 48L's runner.
* One throwaway smoke (`crates/rcfd-studio/tests/zz_smoke_m.rs`, deleted, never committed), run once in release after
  the code was written: the editor's bytes of a typed field (`editor_bytes` with the draft in place) and of the same field
  committed (`edit_text`, then no draft) are byte-equal for `mesh.nx = 20` and `physics.nu = 0.02` on the cavity and
  `physics.nu = 0.02` on the hex box (15,922, 15,922 and 17,476 bytes). This is why M3 (b) can register the commit as an
  identical text.
* The code and the tests of this ADR were compiled (`cargo test --release -p rcfd-studio --no-run`), `cargo clippy -p
  rcfd-studio --release --all-targets -- -D warnings` is clean after two `manual_contains` findings in the new tests were
  fixed, `cargo fmt -p rcfd-studio` applied. No registered check of this ADR, and none of the changed earlier tests, has
  run.

## Amendment 1 (2026-10-28, the M1 control's set-up; before the corrected run)

The registered run of `followups_m.rs` on 0bc172f (release, `--test-threads 1 --nocapture`, output to a file) passed M1,
M2, M3, M5, M6, M7 (a), (c) and the controls of M2, M3, M6 and M7, and failed the M1 control: with `run_while_typing`
Run was enabled (`blocked` none) and the preview's next source was the editor's text, as the control requires, but the
start text was not drawn within the hover helper's 5 frames. Cause, read from egui 0.36.2: a headless frame without a
time advances egui's clock by `predicted_dt`, 1/60 s (`input_state/mod.rs` line 374); a first tooltip is shown only while
the pointer is still (`show_tooltips_only_when_still`, `containers/tooltip.rs` lines 354-365), and the pointer counts as
moving while its 0.1 s position history (`History::new(2..1000, 0.1)`, line 1079; every frame adds the position, line 1225)
still holds the position of the click into the field, about 6 frames; within the 0.2 s grace time after another tooltip
that wait is skipped (`tooltip_grace_time`). M1 (b) passed with 4 frames because its hover came 11 frames after (a)'s
tooltip; the control has no earlier tooltip. A set-up defect of the helper, not of the change.

Corrected set-up (the same test names; limits and controls unchanged): after the pointer moves onto Run, frames are drawn
until the hover text is drawn or 30 frames (0.5 s of egui's clock) have passed, and every drawn text of those frames is
kept. What I knew when writing this: the numbers above and egui's code; the corrected helper had not run.

## Amendment 2 (2026-10-28, M7 (b) is a Windows fixture; after its Windows run, before any run elsewhere)

Found in review after M7 (b)'s registered run on Windows, not by a failing run. Its fixture, a folder at
`.rcfd/runner.lock`, ends the runner early only on Windows: opening a folder as the lock file fails there with error 5,
so `serve` exits 1 with `rcfd-runner: runner lock: ...`. On Unix `acquire_lock` creates the lock with `create_new`; the
folder answers "already exists", `unix_takeover` reads no pid from it and leaves a fresh one alone, and after its 10 tries
`serve` exits 3 (another runner holds the lock), which the client awaits for 10 s and does not report as an early exit
(read from `crates/rcfd-runner/src/sys.rs`, `acquire_lock` and `unix_takeover`; not run). M7 (b) would then miss its 5.0 s
limit and its marker on Linux by its set-up, not by the change. The CI (`ci.yml`) runs the Studio's library tests only;
the gate (`gate.yml`) runs on Windows unless another system is asked for. Change: `#[cfg_attr(not(windows), ignore =
"...")]` on M7 (b), naming this amendment; on Windows the test, its limits and its control are unchanged. M7 (a) checks the
rule with the runner's own early exit (a bad budgets file) on every platform.

## Addendum: results against the registered limits (2026-10-28)

Branch `worktree-agent-a56f0d2c2be42e18d`. Commits: registration 0bc172f (on main ec6a443); amendment 1 7d115fc; 1976a47
(M4 (e) reads its line in the first 3 frames, before M4's first run, deviation 2); 96b165f (this addendum's first part);
amendment 2 f208b07; this addendum. Every run: release, `--test-threads 1 --nocapture`, output to files, this host
(Windows 11, 16 threads, 93.6 GB; other agents' work held 50-55 GB in WSL during these runs, so free memory was 1-8 GB);
target folder of this job; `rcfd`, `rcfd-runner` and `fake-claude` built from the same commit as the tests. The table
gives the registered run of each check: `followups_m.rs` on 0bc172f, its corrected run on 7d115fc (M1's control; every
other check of the file passed again with the same values), `starter_run_m.rs` on 1976a47 (3.0 GB free at its start),
`case_starter_g2.rs` on 1976a47 (1.6 GB free), the whole suite (M8) on f208b07 (5.0 GB free at its start, 4.6 GB at its
end).

| Id | Limit | Measured | Result |
|---|---|---|---|
| M1 | (a)-(g) as registered | (a) enabled, the start text drawn on hover; (b) the draft `("mesh.nx", "20")`, `cases/cavity.toml: finish editing the field mesh.nx (press Enter or click elsewhere); the mesh shows the typed text, but a run uses the file on disk` drawn on hover, F5 refused with `Run: ` and that text, the click started nothing (reported: the click took the focus and committed the field); (c) `mesh.nx` 20, no draft, `cases/cavity.toml has unsaved edits: save it first (Case > Save); a run uses the file on disk`; (d) enabled after the save; (e) greyed while typed, enabled after Enter, nothing committed; (f) the draft `1e` kept and greyed, enabled after the correction; (g) 4 enabled states, each with the preview on the file | pass |
| M1 control | enabled, the start text, the editor's text | first run (0bc172f): enabled, preview from the editor, but the start text not drawn within 5 frames (amendment 1); corrected run (7d115fc): `blocked` none, the start text drawn, the preview's next source the editor's text | fails as required (after amendment 1) |
| M2 | (a)-(f) as registered | (a) the start camera, then panned and dollied; (b) 320 cells, the front's z 0.0625 to 0.05 with x and y bounds unchanged: the panned camera kept bit for bit, near and far from `clip_planes`, `camera_kept` + 1; (c) `lx` 1 to 2: `fit` of the camera before, `camera_refit` + 1; (d) the ring's start camera; `nz` 3 to 4 kept the orbited camera; `r_out` 1 to 1.5 refit with yaw, pitch and perspective kept; (e) the hex box's start camera, counters unchanged; (f) the start camera after Run and Mesh | pass |
| M2 control | the camera not kept | not kept, the start camera of the new surface, `camera_kept` 0 | fails as required |
| M3 | (a)-(e) as registered | (a) built; (b) the commit asked for one build and it was skipped (`skipped_identical` + 1, nothing built, ready, not stale, the same `Arc`, no install); the burst `2`, `20`: skipped; (c) built (384 cells); (d) Rebuild built; (e) `physics.nu` built; after the time moved and the commit: built, no install; the burst `0.03`, `0.02`: skipped; skipped 3 in all | pass |
| M3 control | built again | `meshes_built` 2 to 3, `skipped_identical` 0 | fails as required |
| M4 | (e), (a)-(d) as registered | (e) the line drawn; (a) both files written 0.021 s after the call, byte-equal, listed, `created cases/pi.toml and its data file cases/profile_inlet.csv`, the editor clean with the source's hash and valid, the preview ready (480 cells, 2D, the file's mesh hash), Mesh side, installed; (b) the run selected **0.062 s** after the click, finished at step 172 **0.459 s** after it, its stored mesh 82aeeeaf26901ab4 = the preview's; (c) `created cases/pi2.toml; its data file cases/profile_inlet.csv was in the project already, with the example's bytes`, the data file's bytes and modification time unchanged; (d) `cases/profile_inlet.csv exists with other content: cases/pi3.toml would read its table from that file; rename or remove cases/profile_inlet.csv, then create the case again`, no `cases/pi3.toml`, the test's bytes kept | pass |
| M4 control | not opened; the run fails naming the table | the editor did not open `cases/pa.toml` (its message names `tables.inlet_u.file`); the run failed: `pre-launch check failed (exit 3): <run>\case.snapshot: line 28: tables.inlet_u.file: cannot read table file <project>\cases\profile_inlet.csv: The system cannot find the file specified. (os error 2) (see ...)` | fails as required |
| M5 | D, A, B, C everywhere; C newest | `Project::runs`, the browser, the drawn rows of the Runs panel, the browser after a refresh, the run worker's `extra` list and five more listings: D, A, B, C; newest C | pass |
| M5 control | the id order differs; its largest id is not the newest | D, B, C, A; largest id A | fails as required |
| M6 | 6 of 6 one-line messages; drawn | 6 of 6 equal to `descriptor_message` (for example `cases/cavity/case.json: not a case file: this is a benchmark descriptor (id "cavity") (the reference data and tolerances of a validation case, read by rcfd cases), which rcfd run refuses too; Case > New case from example... makes a case file`), nothing opened, the last drawn in the status bar | pass |
| M6 controls | located; the long list | `cases/bad.json: mesh.nx: expected integer, got "abc"`; the route before: `cases/cavity/case.json: boundary_conditions: unknown setting 'boundary_conditions' (known: ...); description: ...` (8 settings listed) | as required |
| M7 (a), (c) | once; equal to the client's report; (c) the tail once | (a) the early exit reported 0.066 s after the call (0.025 s in the corrected run), the Studio's text equal to it, `must be positive` once; (c) `the runner did not take the lock in time (see ...); last lines of <runner.log>: runner 1 started \| line two \| rcfd-runner: something went wrong` | pass |
| M7 (b) | 5.0 s; the marker; once; drawn | **0.084 s** after the click: `the runner did not start: the runner (pid N) exited with exit code 1 before it held the project's lock; its output (<project>\.rcfd\runner.log): rcfd-runner: runner lock: Access is denied. (os error 5). The run stays queued until a runner starts (Runs > Start runner)`, the line once, drawn | pass |
| M7 controls | twice | (a) twice; (b) twice (`...; last lines of <runner.log>: rcfd-runner: runner lock: Access is denied. (os error 5)` appended), 0.082 s | fail as required |
| CS (ADR 0139 amendment 2) | as amended | `case_starter_g2` 25 of 25: CS1 10 of 10 (the profile case opens with its folder, valid, 0.005 s); CS2 10 of 10 and `cases/profile_inlet.csv` 251 bytes = `cases/channel/profile_inlet.csv`, 11 files listed; CS2 control 10 of 10 differ (the profile case's save 17,560 bytes); CS5 24 files, 10 offered, 6 excluded, 7 reference, 0 problems, its 6 controls one problem each; CS6 6 of 6 refused; CS7 10 of 10 (`channel with a profile-file inlet: ... 2D, 480 cells, simplec, at most 2000 iterations.`); CS11 12,823 bytes; CS12 10 rows; CS13 8.611 ms (control 215.108 ms) | pass |
| M8 | the whole suite; `workspace_graph`; clippy; fmt; licences | `cargo test --release -p rcfd-studio --no-fail-fast -- --test-threads 1 --nocapture` on f208b07: 48 test binaries, **187 passed, 0 failed, 4 ignored** (the four known misses: ADR 0131 B9, ADR 0133 R6, ADR 0141 G1, ADR 0144 J3b), 461 s; against the baseline's 175 passed on 84ee864, the 12 new tests of this ADR. Values of the timing checks in that run, under the memory pressure above: B7 median 0.404 ms (control 13.647 ms); J3 0.505 s; J3c 0.504 s, one frame; J4 B shown 0.091 s; J10 median 0.976 ms, max 5.2 ms (control 3,061.3 ms); J2 0.098 / 0.082 s; ADR 0138 G3 median 0.773 ms, max 3.1 ms; G10 NVIDIA Vulkan 46.1 ms, DX12 39.2 ms. `workspace_graph` 3 passed; `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings` and `cargo fmt -p rcfd-studio -- --check` clean on f208b07; `scripts/third_party_licenses.py --check`: up to date | pass |

**Leftover processes** after all runs: none running from this job's target folder (any program, `rcfd.exe`,
`rcfd-runner.exe` and `fake-claude.exe` among them). M4's control and M7 (b) leave runs queued in their temporary projects,
with no runner.

**Deviations and notes.**
1. Amendment 1: the M1 control's hover helper drew 5 frames, fewer than egui needs before a first tooltip once the
   pointer comes from the field (it waits until the pointer is still); corrected before the control's corrected run, which
   failed as required. M1 itself had passed with the old helper only because its hover came within egui's 0.2 s tooltip
   grace time after (a)'s.
2. 1976a47, before M4's first run: M4 (e) looks for the dialog's line in its first 3 frames, because egui 0.36.2 lays a new
   area out unseen in its first frame (`containers/area.rs` lines 444 and 626). The limit is unchanged.
3. Amendment 2: M7 (b)'s fixture is a Windows fixture (ignored elsewhere, limits unchanged); M7 (a) covers the rule
   everywhere. Not run on Linux.
4. The baseline ran on 84ee864, before 48L merged; the registered runs ran on ec6a443 and later (48L's runner). ADR 0146's
   addendum records 48L's own Studio run; no second baseline was made on ec6a443.
5. Memory: other agents' work held 50-55 GB in WSL during this job's runs, so free memory was 0.5-8 GB (each run's start is
   given above). The new process checks' timing limits have wide margins (M4 (b) 0.062 s and M7 (b) 0.084 s against 5.0
   s), and the suite's timing checks met theirs with values like ADR 0144's.
6. ADR 0144 amendment 3's last bullet and its deviation 5 describe `EXCLUDED`'s reason for `channel/profile_inlet.toml`;
   that entry is gone now (the case is offered with its data file).
7. A skipped build still sets the status to building until the comparison's result arrives (milliseconds for the cavity),
   so the stale line and badge can show for a frame or two (checklist item 4).
8. `created` is written with milliseconds by `rcfd-project` and with nanoseconds by the runner client. Within one
   millisecond, a project holding runs of both writers orders the millisecond stamp after the nanosecond one (`Z` sorts
   after a digit): still one fixed order, but not strictly the creation order across the two writers.
9. A bad budgets file cannot reach the strip through Run: the budgets check before the start refuses it first (ADR 0141).
   That is why M7 checks the rule twice: (a) with the runner's own early exit on a bad budgets file through the library,
   and (b) through Run with an early exit after the budgets check passed (the lock folder).
10. `scripts/limit_audit.py` hits in this job's test files, read: `followups_m.rs` `m3_control_no_skip_rule` (class iii), a
    control asserting the pre-change behaviour (built again) as a positive fact, the form the heuristic does not recognise
    (as ADR 0139's and 0144's controls); in `case_starter_g2.rs` the same five class (iii) controls and two class (iv)
    debug skips (CS13) that ADR 0139 read. None in `starter_run_m.rs`.
11. Nothing outside `crates/rcfd-studio` and `docs/adr` changed: `rcfd-project`, `rcfd-runner` and the other crates were
    read, not edited; `Cargo.toml`, `Cargo.lock`, `workspace_graph.rs` and `docs/architecture.md` are unchanged.

**Not run.** A person at the real window (the checklist); Linux (none of the new tests; M7 (b) would be ignored there by
amendment 2); debug builds of the new tests; the workspace gate and workspace-wide clippy (only `rcfd-studio` changed);
`rcfd-project`'s, `rcfd-runner`'s and `rcfd-mcp`'s own tests (not changed; M5 (a) reads `rcfd-project`'s listing and M7
the runner client); `cargo deny` (no dependency changed).

## Not in CI (by hand)

A person at the real window: the greyed Run and its hover text while typing, the camera staying while typing `nx`,
the starter's dialog line and the data file in the Project panel, the strip's error text after a failed runner start.

## View-and-report checklist for the maintainer

1. Build and start: `powershell -ExecutionPolicy Bypass -File scripts\studio.ps1 <project folder>`.
2. Click a 2D case, type into `mesh.nx` (do not press Enter): Run is greyed out; hovering it says "finish editing the field
   mesh.nx ...". F5 says the same in the status bar. Press Enter: Run says "save it first"; save: Run is enabled.
3. Orbit, pan or zoom the mesh, then type a new `nx`: after the pause the mesh changes and the view stays. Type a new `lx`
   (or `ly`): the view is fitted to the new box, in the same direction.
4. With a large case open, type a new value into a field and wait until its mesh is shown; then press Enter (the value is
   committed, the text the same): the mesh is not built again (the "Building the mesh..." line may flash for a frame
   while the text is compared on the build thread).
5. Case > New case from example..., pick "channel with a profile-file inlet": the line says it also creates
   `cases/profile_inlet.csv`; create it, then Run: the run finishes and its fields show. The Project panel lists the table
   too (question 3).
6. Copy a `cases/<id>/case.json` of the repository into the project's `cases/` and click it: the status bar shows the one
   line "not a case file: ...".
7. Report anything slow, confusing or wrong, with the case and what was done.

## Questions for the maintainer (not decided here)

1. **"Refit"** was read as the double click's Fit: the camera keeps its yaw, pitch and projection kind and frames the new
   box. Options: (a) keep; (b) the start camera of the new mesh when the bounds change (what ADR 0138 did on every
   rebuild). Recommendation: (a): a view the user turned stays turned, and a 2D case looks the same either way unless it was
   orbited.
2. **Builds that are not compared.** Only the debounce's builds of the editor's text are compared with the shown mesh. A
   save (its rebuild reads the saved file, whose bytes are the shown text), a reload, a change on disk, "Rebuild", and a
   field typed and put back (the source goes back to the file, whose bytes differ from the editor's canonical text) build
   again. Options: (a) keep; (b) also skip a save's rebuild when the saved bytes are the shown text and the mesh file is
   unchanged (ADR 0138's unregistered rebuild test asserts that rebuild and would change); (c) compare the parsed case
   instead of the bytes. Recommendation: (b), registered as its own check; not (c), which is the key filtering ADR 0144
   question 6 warned about.
3. **The data file in the Project panel.** `cases/profile_inlet.csv` is listed among the cases (`Project::cases` lists every
   file under `cases/`); clicking it gives the editor's and the preview's parse errors. Options: (a) keep; (b) the Studio
   lists only `.toml` and `.json` files as cases and the other files under "Data files"; (c) the same in `rcfd-project`
   (`Project::cases`, also read by `rcfd-mcp`'s `project_info`). Recommendation: (b).
4. **The Start run dialog** (Options...) still plans and starts the file on disk while a field is being typed or edits are
   unsaved. Options: (a) keep; (b) grey its Start out the same way as Run. Recommendation: (b).
5. **`rcfd-runner list`** prints runs in id order (outside the Studio, not changed). Options: (a) keep; (b) order it by
   `created`, then id, like every listing of the Studio and `rcfd-project`. Recommendation: (b), as a runner change.
6. **The early-exit marker.** The Studio recognises an error that carries the runner's output by the client's wording
   (` before it held the project's lock; `); M7 (a) pins that wording against the real client. Options: (a) keep;
   (b) a typed error from `ensure_runner` (an `rcfd-runner` API change). Recommendation: (a) now.
7. **A shared data file.** Two cases made from the same example share `cases/profile_inlet.csv`, so editing the table for
   one changes the other's input. Options: (a) keep (the text names the file; the second creation says that it reuses
   it); (b) give each case its own copy (`<case>_profile_inlet.csv`) and rewrite its `file` key, so that the case is no
   longer byte-equal to the example. Recommendation: (a).

## Files

This ADR; hunks in `crates/rcfd-studio/src/{preview,viewport,quick_run,case_starter,case_starter_ui,watch,studio,
view_toggle,project_view,main}.rs`; new tests `crates/rcfd-studio/tests/{followups_m,starter_run_m}.rs`; changed tests
`crates/rcfd-studio/tests/case_starter_g2.rs`; ADR 0139 amendment 2; notes in ADRs 0144 and 0146.
