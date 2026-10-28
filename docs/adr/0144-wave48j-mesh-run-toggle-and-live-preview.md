# ADR 0144: Wave 48J, the Mesh / Run toggle and a mesh preview that follows the case editor

Status: **registered 2026-10-28**, before any registered run (the code and the tests below were written and compiled
first; what had run before is listed under "What I already knew"). Job 48J, the maintainer's answers to the questions
of [ADR 0138](0138-wave48g1-mesh-preview-from-case.md) (the mesh preview of a case), on main e11c656 (the four 48G jobs,
ADRs 0138-0141, merged). House rules of [ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits are fixed
here before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a
defective set-up gets an amendment written before the corrected run; defaults of existing code stay bit-identical
outside the Studio; `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded; the crate graph stays acyclic; no new
crate. Clean-room: our own Studio code and the egui public API.

## The maintainer's decisions (2026-10-28, answers to ADR 0138's questions)

* **Q1** An explicit Mesh / Run toggle in the Viewport's toolbar. Mesh shows the preview of the open case, Run the
  selected run's fields. Selecting a run or starting one (ADR 0141's auto-select) switches to Run; clicking or opening
  a case switches to Mesh. A side with nothing to show (no case open, no run selected) is disabled with a tooltip. The
  user can always switch back.
* **Q2** The preview follows unsaved editor edits as they are typed: debounced (400-800 ms after the last keystroke;
  the value registered here), the generation counter drops stale builds, the UI thread never blocks, at most two
  builds at once, the size limits (2,000,000 cells, 256 MiB, 250,000 triangles) hold for unsaved text, and half-typed
  or invalid text shows the located error while the last good mesh stays on screen, marked stale. The preview is built
  from the editor's text parsed as the runner parses a snapshot, with the same case folder for relative paths. Run still
  refuses unsaved edits (greyed out, "save it first"), unchanged.
* **Q3, Q4, Q5, Q7, Q8** ADR 0138's recommended defaults: no GPU wireframe now (Q4); no preview of bare files under
  `meshes/` (Q5); a short "not a case file" message for the benchmark descriptors `cases/*/case.json` (Q7); G3 and G5
  shrunk to a 500 x 500 box (Q8); Q3 (the start dialog's `--set` overrides in the preview) as ADR 0138 recommends.
  ADR 0138 states no recommendation for Q3 (it is only asked); it is read here as its default, the overrides are not
  shown in the preview (the same reading as Q4 and Q5), and asked again below (question 1).
* **Q6** The limits stay as they are.
* Also: check that a case made by the case starter (ADR 0139) is opened through `Studio::open_case`, so the preview
  shows it; fix it if not.

## Context (read at e11c656)

* The viewport follows `Studio::selected_run`; with none it shows the preview (`viewport_ui::panel`). Nothing clears a
  selection, so once a run was selected the mesh could not be reached again (ADR 0138 question 1).
* The case editor is a form over the settings schema, not a text buffer. A field's text is committed on focus loss
  (`ui.rs` `leaf_ui`: `lost_focus` then `CaseEditor::edit_text`, parsed with `schema_model::parse_input`, then
  `settings.set` through the in-process service); while typing, the text lives only in the UI's buffers. The editor's
  settings are a JSON tree (`CaseEditor::settings`); a save writes `Case::from_json(settings)` as TOML (JSON for a
  `.json` case), `CaseEditor::to_bytes`, which resolves data files against `.` (ADR 0131 question 3) and reads lookup
  tables (file I/O).
* The preview (ADR 0138) builds the file on disk only and, on a failed or over-limit build, takes the shown mesh away.
* The case starter (ADR 0139 D5) does not go through `Studio::open_case`: its worker command writes the file, sends
  `Event::Created` and then `Event::Loaded`, which opens it in the editor; `Preview::select` is never called, so the
  preview stays on the previous case (read from `case_starter.rs` `create_case_from_example` and `on_created`).
* ADR 0141's one-click Run selects the queued run with `Studio::select_run` (`quick_run.rs`, `Msg::Queued`). The
  viewport followed a new selection only at the next `Studio::drain`, so in the frame where the run was first selected
  the viewport could still hold the preview's zero frame.
* A run that ends before its first snapshot gets ADR 0141's explanation once (`quick_run::explain_end`, the run
  remembered in `QuickRun::probed`); reopening the run's store (which the toggle does) brings back ADR 0140's text.
* `mesh.nx` and `mesh.ny` are limited to [1, 100000] by the settings schema, so a typed `nx = 10000000` is refused by
  `parse_input` before any size check; `nx = 100000` with `ny = 1000` passes it and is over the cell limit.

## Decisions (as implemented)

1. **The toggle** (`src/view_toggle.rs`, new). `Studio::view: Toggle` holds the side (`Side::Mesh`, the default, or
   `Side::Run`). The viewport shows the selected run only on the Run side (`Toggle::run_shown`); the selection is
   never cleared by the toggle, so the run worker, the plots, the log and the run strip keep following the selected run
   on the Mesh side. Switches: `Studio::open_case` and a case the starter opens go to Mesh
   (`Studio::show_case_mesh`); `Studio::select_run` (the Runs panel, also the run already selected, and ADR 0141's
   auto-select) goes to Run; a selection written to `selected_run` directly (as earlier tests do) is seen at the next
   drain and goes to Run, unless a case was opened after it; `Studio::set_view` (the toggle) goes to the side asked for,
   refused with the reason when that side has nothing to show (`NO_CASE` "No case is open: click a case in the Project
   panel to see its mesh.", `NO_RUN` "No run is selected: select a run in the Runs panel, or press Run (F5) to start
   one."); closing the project goes back to Mesh. Every switch synchronises the viewport at once (no frame shows one
   side's picture under the other's selection). The toggle is the first row of the Viewport panel, on both sides:
   two selectable buttons `Mesh` and `Run`, disabled with the reason as hover text, and on the Run side the caption
   `stored fields of run <id>`. Switching back to Run opens the run's store again: the newest snapshot, the start
   camera and the `p` field, as when the run was first shown (question 3).
2. **ADR 0141's explanation after a round trip.** When the selected run was explained once (it ended before its first
   snapshot) and its store is opened again, the explanation is applied again (`quick_run::follow_selected`; test switch
   `QuickRun::switches.no_reexplain` restores the earlier route).
3. **The preview follows the case editor** (`src/preview.rs`, `src/case_edit.rs`, `src/ui.rs`).
   * Keystrokes reach the model: every change of a field's text calls `CaseEditor::type_text(path, text)` (the field's
     whole text, kept as `CaseEditor::draft`); a commit of that field clears the draft, a refused commit keeps it (the
     field keeps showing the refused text); a focus loss with the stored value's text ends it (`end_draft`). The
     settings are not changed by typing. `CaseEditor::revision()` changes with every change of the settings or the
     draft.
   * Source: the case file on disk while the editor holds no unsaved edit and no draft of the previewed case (ADR 0138's
     path, unchanged); otherwise the editor's text: the settings with the draft's text put in place (parsed with
     `parse_input`, the editor's own parse), checked and resolved by `Case::from_json_in(settings, <the case's folder>)`,
     written as `CaseEditor::to_bytes` writes it (TOML, JSON for a `.json` case), then parsed with
     `preview::parse_case(text, <the case's folder>)` (the runner's `rcfd run <snapshot> --case-dir <folder>` rule) and
     built as a file is. All of it runs on the build thread (`preview::build_source`); the UI thread only clones the
     settings tree when a build is asked for. Not `to_bytes()` itself: it resolves against `.` and reads tables.
   * Debounce: a change of the editor's revision asks for a build `DEBOUNCE` = **500 ms** after the last change
     (`Preview::debounce`); opening a case, a change on disk, Rebuild and "anyway" ask at once (the pending edit is
     built by that request too). `StudioApp::ui` asks egui for a frame when the debounce ends
     (`ctx.request_repaint_after(Preview::due_in())`), so the build starts without further input.
   * Stale builds and the cap: ADR 0138's generation counter and `MAX_BUILDS` = 2 unchanged; a request that waits for
     a thread keeps its source (the text as it was asked for), not the file.
   * Limits: `Limits::default()` unchanged and applied to the editor's text as to a file (the estimate is made from the
     parsed text).
   * The last good mesh: `Preview::shown` is what the viewport shows, the last build of the case within the limits (or
     shown anyway); `Preview::built` keeps ADR 0138's meaning (the latest successful build, also over the triangle limit,
     until "show anyway"). A build that fails or is over a limit leaves `shown` on screen; a rebuild that gives the same
     mesh keeps its surface (no new upload, the camera stays). `Preview::stale()` is true while `shown` is not the
     current text's mesh (a debounce pending, a build running, the latest build failed or over a limit). The panel then
     shows the line `Stale: the picture is the last mesh that built: <reason>.` and paints a `stale` badge on the
     image. Test switches: `Switches::disk_only` (ADR 0138's file-only source), `Switches::drop_on_failure` (ADR 0138's
     display rule).
   * Panel texts: the source under the title, `(built from the case file on disk; the solver is not run)` or `(built
     from the unsaved edits in the case editor; the solver is not run)`; with unsaved edits or a field being typed:
     `The mesh follows the unsaved edits in the case editor; a run uses the file on disk, so save the case before Run.`
     (replaces ADR 0138's "Unsaved edits are not shown ...").
   * Run is not changed: `quick_run::blocked` still refuses a case with unsaved (committed) edits; a field still being
     typed is not an unsaved edit for it (question 2).
4. **Benchmark descriptors** (`preview::descriptor_message`). When the case text does not parse and it is a JSON
   object with a `reference` table and an `id` text or a `roadmap` list at the top level (keys no case file has), the
   preview's message is one line: `<file>: not a case file: this is a benchmark descriptor (id "<id>") (the reference
   data and tolerances of a validation case, read by rcfd cases), which rcfd run refuses too; Case > New case from
   example... makes a case file`. The case editor's own message for such a file is not changed (question 4).
5. **The case starter.** Found as read above (the preview never saw a case the starter opened). Fixed at
   `Event::Created { opened: true }`, when the file exists: the starter calls `Studio::show_case_mesh`, the preview and
   view part of `open_case` (calling `open_case` itself would load the file a second time). With unsaved edits the
   starter neither opens nor previews the new case, as before. Test switch `view_toggle::Switches::starter_skips_preview`
   restores the earlier route.
6. **ADR 0138's G3 and G5 shrunk** (maintainer's Q8): ADR 0138 amendment 3, written in ADR 0138 with this registration
   and before their reruns: the box of both is 500 x 500 (250,000 cells) instead of 1000 x 1000; limits and set-up
   preconditions unchanged.
7. Not done (the decisions): no GPU line pass (Q4); no preview of files under `meshes/` (Q5); the limits unchanged (Q6);
   the start dialog's overrides not shown in the preview (Q3, question 1).

## Placement

`crates/rcfd-studio` and `docs/`. New module `src/view_toggle.rs`; hunks in `src/{lib,studio,preview,preview_ui,
viewport_ui,case_edit,ui,case_starter,quick_run}.rs`. Amendment 3 adds an opt-in parameter to `rcfd-api`'s
`session.create` (`src/service.rs`, a new test file) and changes ADR 0142's tests in `rcfd-mcp` and `rcfd-studio`. No new
crate, no new dependency, no new workspace edge; `Cargo.toml`, `Cargo.lock`, `crates/rcfd-cli/tests/workspace_graph.rs`
and `docs/architecture.md` unchanged. `rcfd run` and the runner do not use `session.create`, so they write what they
wrote before.

## Registered checks (limits fixed here, before any of them ran)

Release builds, `--test-threads 1`, output to files, this host; GPU checks on every adapter of
`rcfd_render::adapters()` with ADR 0125's SKIP rule; process checks with `rcfd-runner` and `rcfd` of the same build next
to the test's `rcfd-studio` (`common::binaries`). "Typing" in model checks is `CaseEditor::type_text` (the call the
field makes on every change); J10 types through egui events into the drawn field. "Settled": no debounce pending and
the preview not building. Fixtures: ADR 0138's project (`pv::project`: the 16 x 16 cavity, the 4 x 4 x 4 hex box through
`mesh.file = "../meshes/hex.msh"`), ADR 0131's B1 project (cases `a` (16 x 16), `b` (12 x 16), `sub/c`, eight runs: two
each queued, running, finished, failed with reason `test failure`, no stores), `box_2d` cases written by the tests.

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| J1 | The toggle's state machine, B1 project, run worker on, watcher off: (a) nothing open; (b) `a` opened; (c) the finished run selected; (d) toggle Mesh; (e) toggle Run; (f) `b` opened; (g) the same run selected again; (h) Mesh, then a running run written to `selected_run`, one drain; (i) a queued run written to `selected_run`, then `a` opened before any drain; (j) a failed run selected until ADR 0141's explanation shows, then Mesh and Run; (k) project closed | (a) Mesh, both sides refused with `NO_CASE`, `NO_RUN`, side unchanged; (b) Mesh, no run in the viewport, the preview's surface installed (same `Arc`), Run still `NO_RUN`; (c) at once Run with the run in the viewport, the run worker shows it; (d) at once no run in the viewport, then the preview's surface; selection, run worker and strip still on the run; (e) Run, the run; (f) Mesh, `b`'s 192 cells shown, the run still selected; (g) Run; (h) Run, the running run; (i) Mesh, the preview of `a`; (j) the viewport's `Failed` text starting `Run <id> failed before writing its first stored snapshot: test failure` before, and the same text within 5 s after the round trip, held 0.5 s; (k) Mesh, both refused | ADR 0138's rule (`run_always_wins`): after (c), (d) the viewport still shows the run; the route before decision 2 (`no_reexplain`): (j)'s text is not back after the round trip |
| J2 | Run start, process: the 12 x 12 cavity, 300 steps, runner served directly; (a) opened; (b) Run clicked; (c) the case opened again; (d) Run clicked again; (e) `mesh.nx` typed then committed | (a), (c) Mesh with the preview shown, (c) the first run still selected; (b), (d) the new run selected, Run side, the run in the viewport, 5.0 s after the click; (e) with the edit committed `quick_run::blocked` contains `save it first` (the typed-only state is reported) | ADR 0141's auto-select off (`no_select`): a third Run leaves the Mesh side for 5.0 s after the run is queued |
| J3 | Debounce: the cavity, `nx` typed `2` then `20` 100 ms later, drained every 5 ms | one build asked for (of the editor's text), 0.50 to 0.75 s after the last keystroke; then ready with 320 cells from the editor; no further build in 1 s of idle drains | debounce 0: the same burst asks for at least 2 builds |
| J3b | The drawn app after 30 idle frames: `nx` typed `24`, one frame, then one frame after the repaint delay it asked for | the frame after the keystroke asks for the next at most `DEBOUNCE` later; the frame then asks for the build (one) | an idle frame asks for none within 1 s |
| J4 | Stale drop: the 500 x 500 box; `nx = 501` (A, 250,500 cells) typed; 50 ms after A's thread started, `nx = 20` (B, 10,000 cells) | A never in `built` or `shown` after B was typed; B shown 2.0 s after its request; at least 1 result discarded; never more than 2 build threads. Set-up: A still running when B is typed, A's thread at least 0.5 s | generation check off (`ignore_generation`): A applied after B was typed |
| J5 | Invalid edits on the cavity (256 cells shown): (a) `nx` typed `1e`; (b) `0`; (c) `16`, then `mesh.source = "file"` and `mesh.file = "missing.msh"` committed; (d) `mesh.source = "generator"` committed and `12` typed | (a) failed with exactly `cases/cavity.toml: mesh.nx: '1e' is not a whole number`; (b) exactly `cases/cavity.toml: mesh.nx: 0 is below the minimum 1`; (c) starts `cases/cavity.toml: mesh.file: `, names `missing.msh`, `build_mesh` reached (one more mesh built); (a)-(c) the 256-cell mesh still shown (same `Arc` in `shown` and the viewport, no new upload) and stale; (a), (b) no mesh built, the settings' `mesh.nx` 16 and the file unchanged; drawn in (a): the message, `Stale: the picture is the last mesh that built: the current text does not build (above).` and the badge `stale`; (d) ready, 192 cells from the editor, not stale, neither line nor badge drawn | ADR 0138's rule (`drop_on_failure`): after (a) nothing shown |
| J6 | Limits on unsaved text: (a) a 20 x 1000 `box_2d`, `nx` typed `100000`, then `1000000`; (b) the hex box with the file limit at 10,000 bytes, `mesh.file` typed `../meshes/big.msh` (20,000 bytes); (c) a 4 x 4 cavity with the triangle limit at 100, `nx` typed `20`, then "show anyway" | (a) too large, exactly `cases/tall.toml: about 100,000,000 cells (from the mesh.n* counts of generator "box_2d"), more than the preview limit of 2,000,000`, 5.0 s after its request at most, no mesh built, the 20,000 cells kept and stale; `1000000`: failed, exactly `cases/tall.toml: mesh.nx: 1000000 is above the maximum 100000`, no mesh built, kept; (b) too large, starts `cases/hex.toml: mesh.file `, ends `big.msh is 20,000 bytes, more than the preview limit of 10,000`, no mesh built, kept; (c) too large, exactly `cases/c4.toml: the surface has 160 triangles, more than the preview limit of 100 (uploading it pauses the window)`, the 4 x 4 kept, stale; after "show anyway" ready, 80 cells shown, no request and no mesh build | ADR 0138's rule (`disk_only`): after (a)'s keystroke and 1.5 s, ready with the file's 20,000 cells |
| J7 | The hex box, `mesh.scale = 2` committed and `physics.nu` typed `0.02`; then the field committed, the save's bytes taken, saved, and run as the runner starts it (`rcfd run <tmp>/case.snapshot --case-dir <project>/cases --out .. --store-dir .. --set output.store_interval=1 --set solver.max_iterations=2 --quiet`) | built from the editor; `hash_bytes` of `CaseEditor::to_bytes` equals the hash the preview built; the mesh equals `Simulation::new` of those bytes parsed with `parse_case(.., <project>/cases)` (hash, counts, patches, surface bit for bit) and differs from the mesh before the edit; the saved file holds those bytes; the run exits 0 or 5 and stores that mesh (hash) and surface (bit for bit) | the same bytes parsed with base `.` (the editor's service) do not build |
| J8 | The 6 shipped descriptors through `preview::build`; `cases/cavity/case.json` copied into a project and opened | each refused with one line starting `cases/<id>/case.json: not a case file: this is a benchmark descriptor (id "<id>")`; through the model the same one line, nothing shown | `{"title": "x", "mesh": {"nx": "abc"}}` keeps its located diagnostics (first line `cases/bad.json: `, no "not a case file"); the descriptor with `reference` renamed gets at least 2 lines; ADR 0138's message of the cavity descriptor has at least 7 lines |
| J9 | The case starter, watcher off: the cavity open and a finished run selected (Run side); `create_case_from_example(sod, "first")` | 30 s at most (ADR 0139 CS8's wait) until the preview's case is `cases/first.toml`, settled and in the editor; then ready, 100 cells, 2D, the mesh hash of `build_mesh(Case::load(file))`, installed, Mesh side, no run in the viewport | the route before decision 5 (`starter_skips_preview`): after `second` opens in the editor the preview stays on `cases/first.toml`; with unsaved edits `third` is created, not opened, the preview stays and the side stays Run |
| J10 | Typing cost, drawn app: the 500 x 500 box open (its build not measured), the `mesh.nx` field clicked, 4 bursts typed through egui events (3 Backspaces, then `50k`, one key per 50 ms, a frame every 5 ms), each followed by 700 ms of frames, then frames until the builds settle | median frame at most **8 ms**, no frame above **250 ms**. Set-up: at least 20 frames; after each burst the editor's draft is `("mesh.nx", "50k")`; 4 builds of the editor's text; the last build at least 0.5 s; never more than 2 build threads | building on the UI thread (`build_on_ui_thread`): a frame above 250 ms |
| J11 | The toggle drawn (B1 project, no run worker; tooltip delay 0): (a) nothing open, each button hovered; (b) `a` opened, then a click on Run; (c) a finished run selected; (d) a click on Mesh; (e) a click on Run | (a) both disabled; hovering draws `NO_CASE` and `NO_RUN`; (b) Mesh enabled, Run disabled, the click on Run changes nothing, `Mesh of cases/a.toml` drawn; (c) both enabled, Run, `stored fields of run <id>` drawn; (d) Mesh, no run in the viewport, `Mesh of cases/a.toml` drawn; (e) Run, the run, its caption drawn | (b)'s click on the disabled Run is the refusal shown |
| J12 | GPU: a project with the cavity case and a finished run storing the 8 x 8 cavity; the drawn app with the case opened, the run selected, toggle Mesh, toggle Run | after each of the 4 steps the viewport texture is byte-identical to `Renderer::render` of a fresh upload of what the model shows (ADR 0138 G6's rule), every adapter | the textures of the two sides differ |
| J13 | ADR 0138's G3 and G5 at 500 x 500 (ADR 0138 amendment 3) | their registered limits, unchanged | their own controls |
| J14 | The rest: the whole `rcfd-studio` suite (ADR 0131-0141 checks), `workspace_graph`, `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings`, `cargo fmt -p rcfd-studio -- --check`, `scripts/third_party_licenses.py --check` | pass, with ADR 0141's G1 control corrected by its amendment 2 (item 1) and G1b unchanged; the three known misses ignored as before. Excepted: ADR 0141's G4 control, which fails on e11c656 before this change for a reason outside this job (ADR 0141 amendment 2 item 3, question 8): reported, not counted | (their own) |

Derivations. 500 ms is in the maintainer's range; the cavity builds in milliseconds, so its picture follows about
0.5 s after the last keystroke. J3's window: never earlier than the debounce (the property); at most one viewport poll
period (250 ms, ADR 0141 G1 (c)) later, for the 5 ms drain loop and scheduling under load. J4's 2.0 s is ADR 0138 G5's
(the small build plus slack); J6's 5 s ADR 0138 G4 (c)'s; J10's 8 ms and 250 ms ADR 0138 G3's (B7's half frame; far
below a 0.5 s build on the UI thread); J2's 5.0 s ADR 0141 G1 (a)'s; J9's 30 s ADR 0139 CS8's; J1 (j)'s 5 s ADR 0141's
R3-based 5.0 s.

How the registered runs are made: `cargo test --release -p rcfd-studio --test <file> -- --test-threads 1 --nocapture`
for each new test file (`live_preview_j`, `live_preview_cost_j`, `live_preview_gpu_j`, `live_preview_run_j`) and for the
changed ones (`preview_g1`, `preview_cost_g1`, `oneclick_g4`, `oneclick_e2e_g4`), output to files; then the whole
`rcfd-studio` suite once with `--no-fail-fast -- --test-threads 1 --nocapture`, output to a file (J14); clippy, fmt,
`workspace_graph` and the licence check on the final commit. A check whose limit is missed becomes an ignored
`*_known_miss` test with its limit intact; a set-up defect gets an amendment here before the corrected run.

## Changes to earlier tests (all registered here, none run yet)

* ADR 0138's unregistered `rebuilds_after_a_save_and_an_external_change` asserted that an unsaved edit is not shown,
  the opposite of decision Q2. Changed to the decided behaviour: the unsaved edit is shown from the editor's text after
  the debounce (one build); the save rebuilds it from the file with the same mesh, kept without a new upload; the
  external change still rebuilds (384 cells).
* ADR 0138's G3 and G5: the 500 x 500 box (ADR 0138 amendment 3).
* ADR 0141's G1 control: the frame of a run (ADR 0141 amendment 2 item 1).

## What I already knew (rule d)

* The code reading above, and ADR 0138's addendum (G3 and G5 at 1,000,000 cells: 27-32 s and about 2.7 GB each under
  `gate_test.py --jobs 5`).
* **Baseline on main e11c656, before any change of this job**: release build in a new target folder of this job,
  `cargo test --release -p rcfd-studio --no-fail-fast -- --test-threads 1`, output to a file (7 min 2 s): 150 passed,
  3 failed, 3 ignored (the known misses of ADR 0131 B9, ADR 0133 R6 and ADR 0141 G1). The failures, all ADR 0141 tests
  after the merges of ADRs 0138 and 0140: G1b (`renders 10 uploads 2`, shown indices `[0, 17, 36, ...]`), the G1 control
  ("frame within 5 s of the first snapshot true (0.000 s)") and the G4 control (`assertion failed: !vp.contains(&reason)`,
  the viewport holding ADR 0140's `Run <id> failed before it stored its first snapshot: <reason>`). That run captured
  the output of failing tests only, so it gives no measured values of the passing ones.
* The first baseline build failed while linking (`LNK1181: cannot open input file 'dbghelp.lib'`): a Visual Studio
  update (`setup.exe update`, 15:13-15:26) was replacing the Windows SDK; the build was repeated after it ended. Not a
  defect of the code.
* The code and the tests of this ADR were written and compiled after the baseline run (`cargo test --release -p
  rcfd-studio --no-run`); `cargo clippy -p rcfd-studio --release --all-targets -- -D warnings` was clean after two
  `manual_contains` findings in `tests/live_preview_j.rs` were fixed; `cargo fmt -p rcfd-studio` applied. No test of
  this ADR, and none of the changed earlier tests, had run.

## Amendment 1 (2026-10-28, J3b set-up defect, before the corrected run)

The registered run of `tests/live_preview_j.rs` on 0922e73 (release, `--test-threads 1 --nocapture`, output to a
file) failed J3b; its other twelve tests passed (addendum). J3b measured: an idle frame asked for no repaint
(`Duration::MAX`); the frame after the keystroke asked for the next one after 483.33 ms; the one frame drawn then
asked for no build. Cause, read from egui 0.36.2 (`context.rs`, `request_repaint_after`, lines 149-152): egui shortens
every requested delay by its predicted frame time (`input.predicted_dt`, 1/60 s in these headless frames) "to make it
less likely we over-shoot the target", so 500 ms became 483.33 ms and the frame drawn then came about 16.7 ms before
the debounce ended. That frame asks again for the time left (less 1/60 s, so at once), and the next frame asks for the
build. The registered limit presumed that a frame drawn at the reported delay is past the deadline: a set-up defect of
the check (like ADR 0138 amendment 1), not a miss of its property, a build that starts without further input.

Corrected set-up and limits (same test name): after the keystroke frame, frames are drawn as the window draws them,
each after the repaint delay the previous frame asked for (at most 2 s), no input, until a frame asks for the build or
2 s pass. Limits: the frame after the keystroke asks for the next at most `DEBOUNCE` later (unchanged); a frame of that
chain asks for the build; at most 3 frames after the keystroke frame; 0.50 to 0.75 s after the keystroke (J3's window);
every frame before it asked for a repaint. Control unchanged: an idle frame asks for no repaint within 1 s. What I knew
when writing this: the numbers above and egui's code; the corrected test had not run.

## Amendment 2 (2026-10-28, J3b missed its corrected limit; a change and J3c, before J3c ran)

J3b with amendment 1's procedure ran on 7b1d532 (output to a file) and **missed** its limit of at most 3 frames: after
the frame drawn at 483.33 ms, 44 frames followed, each asking for a repaint at once (delay 0) and drawn back to back,
until the debounce had ended; the build was asked 0.5009 s after the keystroke, every frame asked for a repaint. A
window with vsync draws such frames one per 1/60 s, but without it the app redraws without pause for up to one frame
time before every debounced build: a defect of the code, not of the set-up. Amendment 1's reading of the first failure
holds for egui's design, but the app can do better. The test becomes the ignored
`j3b_the_frame_after_a_keystroke_asks_for_the_debounced_frame_known_miss` with amendment 1's limits intact; it measures
the code that missed through the test switch `preview::Switches::uncompensated_repaint`.

Change: `StudioApp::ui` asks for the debounce's remaining time plus egui's predicted frame time (`input.predicted_dt`),
which egui takes off again, so the frame asked for starts when the debounce ends.

**J3c** (`j3c_the_frame_asked_for_comes_when_the_debounce_ends`, registered here, before it ran): J3b's fixture and
procedure (frames drawn as the window draws them). Limits: the frame after the keystroke asks for the next at most
`DEBOUNCE` later; the first frame drawn at the delay it asked for (no input) asks for the build (one frame); 0.50 to
0.75 s after the keystroke. Controls: an idle frame asks for no frame within 1 s; with the switch (the code that
missed) the first frame drawn at the asked delay does not ask for the build.

## Amendment 3 (2026-10-28, the maintainer's decision on the case editor's base folder; before any of its runs)

**Decision** (the maintainer, relayed 2026-10-28, answering ADR 0131 question 3 and ADR 0142 questions 2 and 11): the
case editor resolves relative paths (`mesh.file`, lookup tables) against the case's own folder, not the working folder.
Evidence: ADR 0142's E1 known miss and H1's aerofoil branch (the wind-tunnel aerofoil cases name
`../meshes/naca0012-ogrid`). Main (with 48H, ADR 0142, merged as 0a69af4) was merged into this branch first (e38ef9c;
48H added tests and data only, no conflict).

**Change** (decision 8 of this ADR):
* `rcfd-api` `session.create` takes an optional `base_dir` with `toml` or `settings`: the folder relative data paths
  resolve against (sandbox-checked when the service has a sandbox root). Without it nothing changes (the sandbox root,
  else the working folder), and the journal's create entry gets a `base_dir` key only when it was given;
  `journal.replay` passes it on. `docs/service-protocol.md` says so. This is the change ADR 0131 question 3 named; the
  other route of ADR 0142 question 2, `session.create {"path"}`, was not taken: the service would read the file again
  on the UI thread and the session could hold other bytes than the editor's `base_hash`.
* The Studio's editor is opened with the case's folder (`CaseEditor::from_bytes_in`, `CaseEditor::dir`, from
  `Studio`'s `Loaded` event): its session, `validate()`, reloads and the bytes a save writes (`to_bytes`:
  `Case::from_json_in(settings, dir)`) resolve against it. `CaseEditor::from_bytes` (no folder) keeps the working folder
  for callers that give none (the ADR 0139 tests call it).
* ADR 0142's tests: `e1_mesh_file_cases_validate_through_the_editor_calls_known_miss` is un-ignored (renamed without
  `_known_miss`, its limit unchanged: the 2 aerofoil cases open and validate through the editor's calls) and its helper
  gives the case's folder as the editor now does; E1's generator half likewise; E2 keeps the text call without a base
  folder as its contrast (the service's default, unchanged); H1's aerofoil branch becomes the positive check (the two
  aerofoil cases validate in the Studio's editor, like the eight others). ADR 0142 gets a note.
* `case_starter::EXCLUDED`'s reason for `channel/profile_inlet.toml` names the true cause now (the table is not copied).

**Registered checks** (limits fixed here, before any run; release, `--test-threads 1 --nocapture`, output to files):

| Id | Check | Limit | Negative control |
|---|---|---|---|
| J15 | The Studio's editor, through the Studio model (watcher and run worker off): (a) `cases/hex.toml` of ADR 0138's project; (b) `cases/channel/profile_inlet.toml` with `profile_inlet.csv` copied into the project's `cases/`, then `solver.max_iterations = 7` committed and saved | (a) the editor's folder is `<project>/cases`; `validate()` true, no diagnostic; (b) opens, `validate()` true, no diagnostic; the save ends clean; `Case::load` of the saved file holds `max_iterations = 7` | the route before (`CaseEditor::from_bytes`, base `.`): the hex case fails `validate()` at `mesh.file`; the profile case does not open and the message names `tables.inlet_u.file` |
| J16 | `rcfd-api` `session.create`'s `base_dir` (`crates/rcfd-api/tests/service_base_dir_w48j.rs`): (i) the hex case as text and as settings with `base_dir = <root>/cases`; (ii) `journal.get` and `journal.replay` of (i); (iii) a service with a sandbox root: `base_dir = <root>/meshes` with `mesh.file = "hex.msh"`, and a `base_dir` outside the root | (i) both validate; (ii) the journal's create entry holds `base_dir`, the replayed session validates and `matches`; (iii) inside: validates; outside: refused with code `LIMIT` (1004) | the defaults: without `base_dir` the text opens and fails at `mesh.file`, and its journal's create entry has no `base_dir` key |
| J17 | ADR 0142's E1 (`crates/rcfd-mcp/tests/example_projects_w48h.rs`, the whole file run) | E1's limit unchanged: the 2 aerofoil cases (and the 8 generator cases) open and validate through the editor's calls; E2, S1-S9 and the rest of the file as registered in ADR 0142 (G3's crossing count stays its ignored known miss) | E2's text call without a base folder fails at `mesh.file` |
| J18 | ADR 0142's H1 (`crates/rcfd-studio/tests/example_projects_h.rs`) | H1's limits unchanged, and the 2 aerofoil cases validate in the Studio's editor (no diagnostic), like the 8 others | (the route before: J15's controls) |

J14 (the rest) now also covers, in `rcfd-api`: the library's unit tests (the service module's among them) and the
integration tests `service_tcp`, `tables`, `workflow`, `physics_service`, `mesh_quality` (they create sessions, read
tables or replay journals) and `baseline_hashes` (unchanged, not re-recorded): pass as before. The rest of `rcfd-api`
and of `rcfd-mcp` is left to the maintainer's gate.

## Amendment 4 (2026-10-28, the maintainer's decision on ADR 0141's tests; before the corrected runs)

Main (with 48I, ADR 0143, merged as 9052841) was merged into this branch first (0d547c2; no conflict). The maintainer
chose to amend ADR 0141's three tests that fail on main from the 48G merges (amendment 2 of ADR 0141), limits as strict
as registered, none a known miss: [ADR 0141 amendment 3](0141-wave48g4-one-click-run-and-follow.md). G1b counts (g)'s
upload and renders from the frame where the run is first listed and reads frames of the run only; the G4 control reads
the viewport without ADR 0140's empty-state text; the G1 control as before (a frame of a run). J14's exception for the
G4 control is withdrawn: J14 is the whole `rcfd-studio` suite passing (the known misses ignored as before: ADR 0131 B9,
ADR 0133 R6, ADR 0141 G1, and this ADR's J3b), with ADR 0143's new `native_start_w48i` among them.

## Not in CI (by hand)

A person at the real window: the toggle's look and its disabled tooltips, the stale line and badge, typing into the
editor's fields with the preview following (the feel of the 500 ms pause), DX12 and Vulkan, high DPI.

## View-and-report checklist for the maintainer

1. Build and start: `powershell -ExecutionPolicy Bypass -File scripts\studio.ps1 <project folder>` (or `cargo build
   --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`, then `target\release\rcfd-studio.exe <project folder>`).
2. With nothing selected, the Viewport's first row shows `Mesh` and `Run` greyed out; hovering each says why.
3. Click a case in the Project panel: `Mesh` is selected, the case's mesh shows. Press Run (F5): the Viewport switches
   to `Run` and follows the run. Click `Mesh`: the case's mesh again, while the strip, the plots and the log stay on the
   run. Click `Run`: the run again (from its newest snapshot). Click the case in the Project panel: `Mesh`.
4. In the case editor type into `mesh.nx` of a 2D case (do not press Enter): about half a second after you stop
   typing, the mesh follows; the title line says "built from the unsaved edits". Type `1e` or `0`: the located error,
   and the last mesh stays with an orange `stale` badge and the "Stale: ..." line; fix the text: the badge goes.
5. Type `100000` into `mesh.nx` of a case with `ny` of 1000 or more: "about ... cells, more than the preview limit"
   and the last mesh stays (only click "Build anyway" with memory to spare).
6. With the edit committed (Enter), Run is greyed out with "save it first" (unchanged); save: the mesh stays (now "built
   from the case file on disk").
7. Case > New case from example..., create one: the editor and the Viewport show the new case's mesh.
8. Copy a `cases/<id>/case.json` of the repository into the project's `cases/` and click it: one line "not a case
   file: this is a benchmark descriptor ...".
9. Report anything slow, confusing or wrong, with the case, the adapter and what was done.

## Questions for the maintainer (not decided here)

1. Q3, the start dialog's `--set` overrides in the preview: ADR 0138 gave no recommendation; implemented as before
   (not shown). Options: (a) keep; (b) a "mesh with these overrides" preview in the start dialog. Recommendation: (a);
   the preview is of the case (file or editor), and the dialog shows its cell count already (ADR 0140).
2. Run with a field still being typed (not committed): Run is not greyed out (ADR 0141's rule sees committed edits
   only) while the preview shows the typed text; the click takes the focus, which commits the field, but the run's plan
   is made from the file. Options: (a) keep; (b) grey Run out while a field is being typed; (c) commit and save before
   Run. Recommendation: (b), it states the rule the preview implies.
3. Back on the Run side the run's view starts again (newest snapshot, start camera, `p`). Options: (a) keep; (b)
   remember the camera and the field per run. Recommendation: (a) until users toggle often.
4. The case editor's status-bar message for a benchmark descriptor still lists the unknown keys. Use the short "not a
   case file" message there too? Recommendation: yes.
5. A rebuild of the same case that changes the mesh (typing `nx`) resets the camera (ADR 0138's rule). Options: (a)
   keep; (b) keep the camera when the case and the bounds are unchanged. Recommendation: (b).
6. Any edit of the case rebuilds the mesh after the pause, also of settings the mesh does not use (it can depend on
   `physics.nu` through wall layers, so filtering by key is not safe); a large mesh then builds again for each pause.
   Options: (a) keep; (b) skip the build when the text equals the text of the shown mesh (exact). Recommendation: (b).
7. The debounce is 500 ms (the range given: 400-800 ms). Accept?
8. ADR 0141's G4 control has failed on main since ADR 0140 was merged: its premise, that without ADR 0141's explanation
   the viewport lacks the run's reason, no longer holds (ADR 0140's empty state names it). Options: (a) amend the
   control to assert that ADR 0141's own text ("failed before writing its first stored snapshot") is absent, limits
   unchanged; (b) keep it failing as a known miss. Recommendation: (a), by an amendment of ADR 0141.
9. The toggle's `Run` sits below the toolbar's green Run button. Keep the labels `Mesh` / `Run` (the maintainer's
   wording) or name the toggle's side `Run fields`? Recommendation: keep.
10. `channel/profile_inlet.toml` (a relative table) cannot be opened in the case editor (ADR 0131 question 3), so its
    edits cannot be previewed; the preview resolves the editor's text against the case's folder, the editor's save
    path against `.`. Recommendation: settle ADR 0131 question 3 (the editor's service with the case's folder as base).

## Files

This ADR; `crates/rcfd-studio/src/view_toggle.rs` (new); hunks in `crates/rcfd-studio/src/{lib,studio,preview,
preview_ui,viewport_ui,case_edit,ui,case_starter,quick_run}.rs`; tests `crates/rcfd-studio/tests/{live_preview_j,
live_preview_cost_j,live_preview_gpu_j,live_preview_run_j}.rs` and `tests/lj/mod.rs` (new); changed tests
`crates/rcfd-studio/tests/{preview_g1,preview_cost_g1,oneclick_g4}.rs`; ADR 0138 amendment 3, ADR 0141 amendment 2.