# ADR 0131: Wave 48B, the Studio application shell (`rcfd-studio`, layer L5)

Status: **ACCEPTED 2026-10-28** (the maintainer accepted the ADR and took the recommended option of every open decision W48-1 to W48-16, scope cut (a)) (scoping only; nothing implemented or run). Depends on ADR 0130 (48A). House rules as in ADR 0130.
Parent: [`docs/ui-restart-plan.md`](../ui-restart-plan.md) sections 1, 2, 3 (L5), 5, 6 and 8. Roadmap: F-266. Clean-room: egui,
eframe and wgpu public API documentation and crates.io dependency data only; no product is imitated.

## Context (read at main 1248a76)

* There is no Studio crate. The glossary says "It has no window yet" ([`docs/glossary.md`](../glossary.md)).
* The frozen `rcfd-gui` (`crates/rcfd-gui/`) is eframe 0.27.2 on glow with `egui_dock` 0.12, `egui_plot` 0.27.2 and `rfd` 0.15.4
  (`crates/rcfd-gui/Cargo.toml`). It drives the solver in process through `rcfd_api::service::Service` sessions
  (`src/controller.rs` line 142), not through project folders or the runner. Its non-UI logic is unit-tested: schema-driven editor
  model (`src/schema_model.rs`), monitor store with log mapping and decimation (`src/monitors.rs`, tests M1-M7 in
  `tests/monitors_plot.rs`), dock layout (`src/layout.rs`), file dialogs behind a `FileChooser` trait (`src/files.rs`). Headless frames
  are drawn with `egui::Context::run` in `tests/smoke.rs`.
* eframe 0.27 pins wgpu 0.19 and cannot share types with wgpu 30 (ADR 0125, "Pairing with eframe"). The pairing is wgpu 30.0.x with
  eframe 0.36.2 (published 2026-09-08) or 0.37.0 (2026-10-15, passes the two-week rule of ADR 0125 on 2026-10-29). Both need rustc
  1.95.
* Toolchain: `README.md` lines 50-52 promise Rust 1.90 for the whole workspace; `.github/workflows/ci.yml` job `min-toolchain` runs
  `cargo check --workspace --all-targets --locked` on 1.90.0; the gate uses 1.99.0. `rcfd-render` declares 1.90. A workspace member
  that needs 1.95 breaks the `min-toolchain` job.
* `rcfd-project` gives everything the browser needs: `Project::create`, `open`, `cases`, `meshes`, `designs`, `runs` (with
  `skipped`), `validate`, `resolve` (path safety) and a poll `Watcher` with `poll`/`changes_since` (`crates/rcfd-project/src/`).
* Licences: `deny.toml` allows MIT, Apache-2.0, BSD, ISC, Zlib, BSL-1.0, 0BSD, Unlicense, CC0, Unicode-3.0, CDLA-Permissive-2.0, and
  has an exception for `epaint`'s bundled fonts (OFL-1.1, UFL-1.0). The repository is AGPL-3.0-only with a commercial option
  (`CONTRIBUTING.md`); every new dependency must be permissive so that both licences remain possible.

## Decisions needed (none decided here)

### W48-1: toolchain minimum

* (a) Raise the README minimum and the CI `RUST_MIN_TOOLCHAIN` to 1.95 for the whole workspace.
* (b) `rcfd-studio` declares `rust-version = "1.95"`; the `min-toolchain` job adds `--exclude rcfd-studio`; README states that the
  solver and libraries need 1.90 and the Studio 1.95.
* (c) The Studio lives in its own Cargo workspace (for example `apps/studio/`) with its own lock file.

Trade-offs: (a) is simplest but drops users of 1.90-1.94 for the solver, which does not need the newer compiler; (c) isolates the
dependency tree but splits the lock file, the gate and `deny` into two, and the layer test no longer sees the Studio. **Recommendation:
(b)**: one workspace, one gate, the solver's promise kept; the precedent is per-crate `rust-version` (`rcfd-render` 1.90, `rcfd-gui`
1.89).

### W48-2: window and UI toolkit

* (a) eframe 0.36.2 or 0.37.0 with its wgpu back end (`wgpu` feature), egui panels, `egui_dock` for docking, `egui_plot` for plots,
  `rfd` for file dialogs. This is the plan's L5 text (section 3), set by the lead.
* (b) winit plus wgpu directly, with a UI drawn by the Studio itself. Full control, no egui, but every widget (text input, lists,
  docking) is new code.
* (c) Another retained-mode toolkit on wgpu (for example iced). New API to learn, a second widget model next to the frozen egui GUI;
  licences and wgpu 30 pairing UNVERIFIED.

Trade-offs: (a) reuses the team's egui knowledge and the frozen GUI's tested logic, and egui-wgpu exposes the device so the render core
can share it (ADR 0132); its cost is the 1.95 toolchain and egui's immediate-mode redraw (cheap for panels, the 3D view is a texture).
**Recommendation: (a)**, eframe **0.37.0** if the gate starts on or after 2026-10-29 (two-week rule met), else 0.36.2; both resolve to
wgpu 30. Step 0 of the implementation reads the crates.io dependency data (as ADR 0125 did) for the `egui_dock`, `egui_plot` and `rfd`
releases that pair with the chosen egui; those versions are not known here (UNVERIFIED) and are recorded with their licences before
the build.

### W48-3: new crate or extend `rcfd-gui`

* (a) New crate `rcfd-studio` (plan section 1 decision 2, "default chosen by the lead; the user can change it"); `rcfd-gui` frozen.
* (b) Port `rcfd-gui` to eframe 0.36 and grow it into the Studio.

Trade-offs: (b) changes a frozen crate, moves its 0.27 lock entries and its registered tests at once, and keeps the in-process session
model that the plan replaces with project folders; (a) duplicates some logic until `rcfd-gui` is removed. **Recommendation: (a)**.

### W48-9: reuse of the frozen GUI's logic modules

* (a) Copy `schema_model.rs`, `monitors.rs` (store, log mapping, decimation) and `layout.rs` into `rcfd-studio` with a note naming the
  source file and commit; port their tests (M1-M7, the logic tests) with limits unchanged.
* (b) Move them into a small shared crate used by both apps (edits the frozen crate's imports; its tests must still pass).
* (c) Write new modules.

Trade-offs: (a) is duplication until `rcfd-gui` is removed (then it disappears); (b) touches a frozen crate for no user-visible gain;
(c) throws away tested code. **Recommendation: (a)**.

## Design (proposed, assuming the recommendations)

* **Crate** `crates/rcfd-studio` (library plus `rcfd-studio` binary). State and decisions in non-UI modules (`project_view`,
  `case_edit`, `watch`, `layout`, `prefs`); `ui` only draws. The binary takes `rcfd-studio [PROJECT_DIR]`.
* **Project open/new.** "New" calls `Project::create` (name, solver version from `rcfd_api`); "Open" calls `Project::open` and shows a
  migration (`migrated_from`) or a refused newer schema as a located message. A recent-projects list is kept in the user's config
  directory, never inside a project.
* **Browser.** Cases, meshes, designs and runs from `rcfd-project`; runs with state, step, time and numeric metrics (`RunSummary`);
  `skipped` runs listed with their reason, not hidden.
* **Case editor.** The schema-driven editor (copied model) works on a case file of `cases/`, validated through
  `rcfd_api` (`settings.schema`, `settings.set` on a service session made from the file, as `Controller::open_path` and `save_to` do).
  Saving goes through `rcfd_project::atomic::atomic_write` and the conflict rule of ADR 0135 (base hash).
* **Watching.** One `Watcher` per open project, polled every 500 ms on a background thread; changes are posted to the UI by channel.
  An external change to the file open in a clean editor reloads it; to a dirty editor it raises a conflict (ADR 0135).
* **Panels.** Browser, case editor, run list, viewport (ADR 0132), plots and log (ADR 0133), chat (ADR 0134), outputs (metrics
  table, `report.html` link). Layout and theme persist in the user config directory.
* **Threads.** The UI thread never does file I/O beyond reading cached state; watcher, run polling, store reads and chat streams run
  on worker threads (std only, as `rcfd-claude` and `rcfd-runner` do).

## Placement (acyclic)

`rcfd-studio -> rcfd-project, rcfd-runner, rcfd-tsstore, rcfd-render, rcfd-claude, rcfd-api, rcfd-mesh, rcfd-core`, plus eframe and
friends. Layer: a new top layer 9 above `rcfd-gui` (8) in `crates/rcfd-cli/tests/workspace_graph.rs` and
[`docs/architecture.md`](../architecture.md) (`rcfd-studio -> project, runner, tsstore, render, claude, api`). No crate depends on
`rcfd-studio`. `rcfd-runner` (layer 2) and `rcfd-claude` (layer 0, or 2 after ADR 0134) are in the table after ADR 0130 D6. `rcfd-studio` does not depend on
`rcfd-gui`, `rcfd-assistant` or `rcfd-mcp`.

New third-party dependencies, licences to be confirmed from `cargo metadata` at step 0: eframe, egui, egui-wgpu, egui-winit, winit
(MIT OR Apache-2.0 for the egui family at 0.27.2 per `THIRD_PARTY_LICENSES.md`; for 0.36/0.37 UNVERIFIED), `egui_dock` (MIT at
0.12.0; newer UNVERIFIED), `egui_plot`, `rfd`, `epaint` fonts (OFL-1.1, UFL-1.0, already an exception in `deny.toml`; its notice duty in
`THIRD_PARTY_LICENSES.md` applies to the Studio binary too). `cargo deny check licenses bans sources` must pass; `git diff Cargo.lock`
must remove no existing version line (as ADR 0125).

## Registered checks (proposal; fixed here before any run)

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| B1 | Open a fixture project (3 cases, 2 meshes, 8 runs of 4 states, one half-created run folder) | the browser model's lists equal `cases()`, `meshes()`, `runs()` exactly, including the skipped run with its reason | the contract | a model built from `run_ids()` without summaries lists 9 runs, not 8 |
| B2 | New project | folder passes `Project::validate` with 0 issues; manifest name and solver version as entered | the contract | creating into an existing project folder is a located error |
| B3 | Newer schema and a v0 project | newer: located message, nothing written; v0: migrated in memory, "unsaved migration" shown | ADR 0122 M1 | (built in) |
| B4 | Headless frames: 200 frames through every panel shown, hidden and floated, both themes, a project open | no panic; one instance per panel (as the frozen GUI's `layout_keeps_exactly_one_instance_per_panel`) | parity | a layout with a duplicated panel fails the instance check |
| B5 | Case editor: every schema leaf reachable and its default parses (port of `every_schema_leaf_is_reachable_and_parses_its_default`); out-of-range and garbage input refused with a message naming the path; save round trip | exact | parity (plan section 6) | an edit that the service refuses leaves the file hash unchanged |
| B6 | External edit: a case file replaced atomically by another process while the project is open | the model reports it within 2.0 s; a clean editor reloads; a dirty editor enters the conflict state | 4 poll periods of 500 ms | the watcher off: no report after 5 s |
| B7 | UI frame build with 500 runs listed (release) | median of 100 frames <= 8 ms of CPU for building the egui frame (no GPU present) | half a 60 Hz frame, leaving the rest to the viewport and painting | the same frame with 1 ms added per listed row exceeds 8 ms |
| B8 | Layout and preferences round trip | exact | | a corrupt prefs file gives the default layout and a message, no panic |
| B9 | Parity map: one named test per plan section 6 item (open and save a case; settings editor with schema validation; run with live residual and report monitors (ADR 0133); 2D field view with colour map, legend and hover (ADR 0132); assistant access (ADR 0134); tutorials naming the GUI) | each exists and passes, or is an ignored `*_known_miss` with its reason | plan section 6 | (each test's own) |

B7's limit is a proposal; its reason follows ADR 0122 L1's style (a fraction of a perceptible delay). B9's tutorial item: the
tutorials that name the GUI (`docs/tutorials/desktop-assistant.md` and others) are rewritten for the Studio or kept for the frozen GUI;
which, is the user's call at the parity step, not wave 48.

## Not in CI (by hand, recorded in the addendum)

* Opening a real window on DX12, Vulkan and WARP; resizing; high-DPI scaling; keyboard focus; native file dialogs (`rfd`).
* The view-and-report checklist: open the user's project folder, browse runs, edit a case, see an external edit appear.

## Out of scope

Viewport (ADR 0132), run control (ADR 0133), chat (ADR 0134), conflict resolution UI details (ADR 0135); multiple windows; macOS and
Linux packaging; installers; removal of `rcfd-gui`.

## Risks

* egui companion crates (`egui_dock`, `egui_plot`) may lag eframe 0.37; if no release pairs with egui 0.36/0.37, docking is built on
  egui's own panels first (no `egui_dock`) and that is recorded.
* Build time and target size grow (ADR 0125: wgpu alone 202 s of unit time); the gate machine's memory is the constraint.
* Immediate-mode redraw while runs stream: the UI requests repaints only on new data (no busy loop), checked by B7 and by hand.

## Amendment 1 (2026-10-28, 48B, before any registered run): step 0 and set-up

Written by the 48B implementer at main 15b00ef (48A, 48C library part and 48E library part merged) after reading the code and the
crates.io dependency data, before any 48B test ran. What I knew: the code reading; `cargo tree` and `cargo metadata` of the changed
manifest (no compile). Limits are unchanged.

**Step 0 (W48-2), crates.io data read 2026-10-28.**

| Crate | Version | Published | Licence | rust-version | Why this version |
|---|---|---|---|---|---|
| eframe, egui, egui-wgpu, egui-winit, epaint, emath, ecolor | 0.36.2 | 2026-09-08 | MIT OR Apache-2.0 | 1.95 | 0.37.0 (2026-10-15) passes the two-week rule only on 2026-10-29 |
| `epaint_default_fonts` | 0.36.2 | 2026-09-08 | (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 | 1.95 | the fonts moved out of `epaint` into this crate |
| egui_dock | 0.21.1 | 2026-08-06 | MIT | 1.95 | newest release on egui ^0.36 |
| egui_plot | 0.37.0 | 2026-08-05 | MIT OR Apache-2.0 | 1.95 | egui ^0.36.0; 0.38.0 (2026-10-19) needs egui ^0.37 |
| rfd | =0.15.4 | 2025-07-12 | MIT | none | already locked and reviewed for `rcfd-gui`; no egui coupling; 0.16.1 (2026-10-22) fails the two-week rule |
| winit | 0.30.13 | (eframe's requirement) | Apache-2.0 | 1.70 | |

* eframe features: `wgpu_no_default_features`, `default_fonts`, `x11`, `wayland`, default features off. eframe's `wgpu` feature
  turns on `egui-wgpu/default`, which enables wgpu's `metal`, `webgl` and default back ends; through feature unification
  `cargo test --workspace` would then build `rcfd-render` with more back ends, which can change what `rcfd_render::adapters()`
  returns. `rcfd-studio` therefore also lists `wgpu = "=30.0.1"` (not a new crate: ADR 0125's version and features `std`, `wgsl`,
  `dx12`, `vulkan`). `cargo tree -e features -i wgpu@30.0.1 --workspace` lists `dx12`, `std`, `vulkan`, `wgsl`, as for
  `rcfd-render` alone before the change. `accesskit`, `links` and `persistence` are off: they add crates the ADR does not name (the
  layout and preferences are persisted by the Studio itself, B8).
* `Cargo.lock`: one wgpu 30.x, **30.0.1** (and the frozen GUI's 0.19.4). 85 new packages, no package removed (compared by name
  and version; the two `-version = "0.3.2"` lines of `git diff` are hunk boundaries of `objc2-metal` and `objc2-quartz-core` 0.3.2,
  which stay). Every new licence is on `deny.toml`'s list alone or as one choice, except `epaint_default_fonts` (below). One new
  package was published less than two weeks ago: `font-types` 0.12.7 (2026-10-15); it is held at 0.12.6 (2026-10-03) with
  `cargo update -p font-types --precise 0.12.6`.
* **Deviation (deny.toml, not in Files).** `deny.toml`'s exception is keyed on `epaint`; in 0.36 the fonts are in
  `epaint_default_fonts`, which declares the SPDX id `Ubuntu-font-1.0` instead of `LicenseRef-UFL-1.0`. The same exception is added
  for that crate (`OFL-1.1`, `Ubuntu-font-1.0`): the same fonts and licences the ADR already accepts. The notice text of
  `scripts/third_party_licenses.py` names both crates.
* `cargo-deny` is not installed on this host; the licence check is from `cargo metadata` (the CI `deny` job is the reference).

**Set-up of the checks.**

1. W48-9 copy: `schema_model.rs`, `monitors.rs` and `layout.rs` are copied from `crates/rcfd-gui/src/` at main 15b00ef (last changed in
   f71f6e2, c2345a4 and f71f6e2). `layout.rs` gets the Studio's panels (Browser, Case editor, Runs, Viewport, Plots, Log, Chat, Outputs)
   and egui_dock 0.21's `TabPath`; the other two are unchanged apart from the source note. Ported tests, limits unchanged:
   `every_schema_leaf_is_reachable_and_parses_its_default` and `parse_input_rejects_out_of_range_and_garbage` (B5),
   `layout_keeps_exactly_one_instance_per_panel` (B4), `monitors_merge_is_idempotent_and_incremental`, M1-M4 and M6 (M6 draws the
   Plots panel from a monitor store set by the test). **Not ported: M5 and M7.** They start a run of the in-process solver
   (`Controller::run_start`); ADR 0133 D1 says the Studio never runs the solver in process, so their Studio form is ADR 0133 R1, R3
   and R4 (48D).
2. Save and the conflict rule: the editor records `rcfd_project::records::hash_bytes` of the bytes it loaded; save re-reads the
   file, compares, and writes with `atomic::atomic_write` only on a match; a mismatch is the conflict state. This is done inside
   `rcfd-studio`. The cooperative lock and `atomic::replace_if_hash` belong to the parallel 48F branch (ADR 0135) and replace this
   compare-then-write when it merges. A save writes the case as `Case::to_toml` of the edited settings (as the frozen GUI's
   `save_to`), so comments and layout of the file are not kept.
3. Threads: one worker thread per open project owns the `Project` and the `Watcher` (poll every 500 ms), opens and creates the
   project, builds the browser lists, loads and saves case files, and sends results by channel; the UI thread only drains the
   channel. Editing calls the in-process `rcfd_api` service (`session.create`, `settings.set`), which does no file I/O.
4. B1: the test builds the fixture at run time in a temporary folder: `Project::create`; `cases/a.toml`, `cases/b.toml`,
   `cases/sub/c.toml`; `meshes/m1.msh`, `meshes/m2.msh`; 8 runs by `Project::create_run`, two each left `queued` and set to
   `running`, `finished` and `failed` by `write_status`; one half-created run folder with a valid run id holding only
   `case.snapshot`. The model is the browser snapshot the worker sends after open. Compared with `cases()`, `meshes()`,
   `runs().runs` and `runs().skipped` (path and reason). Negative control: `run_ids()` has 9 entries.
5. B2: name and solver version entered as `rcfd_api::design::ledger::solver_version()`; `Project::validate` on the created folder.
   Negative control: a second create into the same folder is an error whose message names `rcfd-project.json` in that folder.
6. B3: the newer manifest has `schema_version` 2; "nothing written" is checked as equal bytes of every file of the folder before
   and after the open. The v0 manifest is ADR 0122's constructed format (`title`, `created_unix`); the model shows "unsaved
   migration (schema 0)" and the manifest bytes are unchanged.
7. B4: 200 frames of `StudioApp::ui` through `egui::Context::run_ui` (1280 x 800), the B1 project open and `cases/a.toml` in the
   editor. Frame i acts on panel `ALL[i % 8]`: hide, show or float in turn ((i / 8) % 3), and the theme alternates every frame.
   After every frame each panel has at most one instance, and the acted-on panel exactly one after show or float. Negative control:
   a layout with the Browser twice fails the instance check (`count` is 2) and `layout::from_json` refuses it.
8. B5: messages naming the path: an editor refusal is returned and shown as `<path>: <reason>`. Negative control: a value that
   parses locally but the service refuses (a string for `mesh.nx` through `set_value`) is refused, the editor stays clean, and a
   save leaves the file's hash unchanged.
9. B6: the other process is this test binary run again with an environment variable (the pattern of `rcfd-project`'s
   `tests/process.rs`); it replaces `cases/a.toml` with `atomic_write` and exits. The time is measured from the child's exit to the
   first drained model event that names `cases/a.toml`. Clean and dirty editors are two runs. The negative control starts the
   worker with polling off and waits 5 s.
10. B7: release build. 500 runs are put into the browser snapshot in memory (the UI never reads files, so the frame cost does not
    depend on where they came from); Runs and Browser panels shown. 5 warm-up frames, then 100 frames timed with wall-clock time
    around `ctx.run_ui` (wall time bounds the thread's CPU time from above, so a pass is conservative; thread CPU time needs an
    unsafe OS call, which the workspace forbids). The run list is drawn with `ScrollArea::show_rows`, so only visible rows are
    built; the negative control adds 1 ms of busy CPU per built row and the number of built rows per frame is reported.
11. B8: the preferences (theme, layout, recent projects) are a JSON file in a directory the caller passes (the user's config
    directory in the app: `%APPDATA%\rcfd\studio` on Windows, else `$XDG_CONFIG_HOME/rcfd/studio` or `~/.config/rcfd/studio`;
    tests pass a temporary folder). Round trip: saved and loaded preferences are equal (layout compared as JSON). Negative
    controls: a file of garbage and a file whose layout lists a panel twice each give the default layout and a message.
12. B9: `tests/parity.rs` holds one test per plan section 6 item. Open and save a case and the settings editor with schema
    validation are real tests. Run with live monitors (48D, ADR 0133), the 2D field view (48C, ADR 0132) and assistant access
    (48E, ADR 0134) are ignored `*_known_miss` tests naming their sub-wave. The tutorials item is ignored with the reason that
    rewriting or keeping them is the user's call at the parity step (this ADR).

## Addendum: 48B results (2026-10-28, branch `worktree-agent-a3d28c7c2cfa896cd`, release build on the gate host)

Decisions applied as accepted: W48-1 (b), W48-2 (a) with eframe 0.36.2 (amendment 1), W48-3 (a), W48-9 (a). Commits 5cbebb0
(step 0, amendment 1) to 945e143 on main 15b00ef. Two set-up details were changed in code after amendment 1 and before any test ran,
both in 61690d0: `layout::to_json` writes positions that are not laid out (NaN, written as `null`) as 0, as `from_json` reads them, so
a saved layout reads back to the same text (B8 compares the text); `Studio::save_case` refuses a clean editor ("nothing to save"),
which is how B5's control leaves the file untouched (a save of an unedited case would rewrite it as canonical TOML, see below). All
tests ran once, in release, `--test-threads 1`; B6 was run three times in all. The workspace gate was not run (the maintainer runs it).

| Id | Limit | Measured | Result |
|---|---|---|---|
| B1 | lists equal `cases()`, `meshes()`, `runs()` (8) and `skipped` (1, with its reason) | equal; 3 cases, 2 meshes, 8 runs (2 each queued, running, finished, failed), skipped 1: the half-created run, "read .../runs/r-20261028T120000-beef/run.json: The system cannot find the file specified" | pass |
| B1 control | `run_ids()` lists 9 | 9 | fails as required |
| B2 | `validate()` 0 issues; name and version as entered | 0 issues, "Fan study", "rcfd 0.1.0" | pass |
| B2 control | located error | ".../rcfd-project.json: already exists"; the first manifest kept | as required |
| B3 newer | located message, nothing written | ".../rcfd-project.json: schema_version 2 is newer than supported 1"; every file byte-equal before and after; the model closed the project | pass |
| B3 v0 | migrated in memory, "unsaved migration" shown | `migrated_from` 0, name from `title`; the notice is in the status message and drawn in the Project panel; manifest and every file byte-equal | pass |
| B4 | 200 frames, no panic, one instance per panel | 200 frames (100 dark, 100 light), project and case open, every panel hidden, shown and floated in turn; at most one instance after every frame, exactly one after show and float; at least 17 shapes per frame | pass |
| B4 control | duplicated panel fails the instance check | `count` 2; `from_json`: "layout lists a panel twice" | fails as required |
| B5 | every leaf reachable and its default parses; refusals name the path; save round trip | 568 leaves, 476 defaults parsed (the rest text, lists or no default); "mesh.nx: 'abc' is not a whole number", "mesh.nx: -4 is below the minimum 1", unknown setting and empty entry name refused with their path, value unchanged; saved through the worker (base hash checked), the file parses, its content hash equals the settings', a fresh editor reads the edited values | pass |
| B5 control | refused edit leaves the file hash unchanged | service refusal "mesh.nx: invalid value at 'mesh.nx': expected integer, got \"abc\""; editor clean; save refused ("nothing to save"); hash fnv64:a1968111d28ff100 before and after | as required |
| B6 | report within 2.0 s; clean reloads; dirty enters conflict | clean: 0.334, 0.314, 0.294 s, reloaded (nx 16 -> 24), clean; dirty: 0.310, 0.320, 0.308 s, conflict with the disk bytes, the edit (nx 10) kept; then "keep my edits" and a save write nx 10 | pass |
| B6 control | watcher off: no report after 5 s | no report after 5.0 s (3 of 3), editor unchanged | fails as required |
| B7 | median of 100 frames <= 8 ms | median 0.327 ms (min 0.311, p90 0.480, max 0.768), 500 runs listed, 17 rows built per frame | pass |
| B7 control | 1 ms per built row exceeds 8 ms | median 17.665 ms, 17 rows | fails as required |
| B8 | round trip exact | theme, recent list (10, most recent first, no duplicate) and layout text equal; saving the loaded preferences writes the same 3,088 bytes | pass |
| B8 controls | corrupt file: default layout and a message, no panic | garbage: ".../studio-prefs.json: invalid type: integer `12`, expected a string at line 1 column 30; default preferences used"; duplicated panel: "...: layout: layout lists a panel twice; default layout used", theme and recent list kept | as required |
| B9 | one named test per item, passing or an ignored `*_known_miss` | `parity_open_and_save_a_case` and `parity_settings_editor_with_schema_validation` pass; live monitors (48D), 2D field view (48C), assistant access (48E) and the tutorials (the user's call) are ignored `*_known_miss` with their reason | pass |

Also run: the ported tests M1 (worst inverse relative error 4.219e-15), M2, M3, M4, M6, `monitors_merge_is_idempotent_and_incremental`,
`layout_keeps_exactly_one_instance_per_panel` (Plots and Chat stand in for the frozen GUI's Monitors and Assistant), and B6's extra
case "a save over a changed file is a conflict and writes nothing": pass. `workspace_graph` 3 passed with `rcfd-studio` at layer 9; a
throwaway layer 5 for it fails `layering_matches_architecture_doc` ("rcfd-studio must not depend on rcfd-api"), restored.
`cargo clippy --workspace --all-targets --locked -- -D warnings` (Rust 1.99) is clean, `rcfd-gui` included; `cargo fmt --check` of
`rcfd-studio` and `rcfd-cli` is clean; `scripts/third_party_licenses.py --check` up to date.

No known miss. Reported, not limited (ADR 0130 asks for the first Studio build with `--timings`): release build of the Studio and its
tests in an empty target folder, `-j 6` on 16 threads, nothing else building: 2 min 00 s wall clock, 256 units, 658 s summed unit
time; longest units wgpu-core 40.2 s, naga 39.9 s, egui 36.7 s, rcfd-mesh 31.1 s, vello_cpu 29.8 s, rcfd-fv 25.3 s, rcfd-api 22.6 s,
read-fonts 21.1 s, image 20.4 s, wgpu-hal 19.4 s; release target folder 1.2 GB.

Deviations and notes:
* `deny.toml` gets an exception for `epaint_default_fonts` (amendment 1); `font-types` is held at 0.12.6 (two-week rule);
  `rcfd-studio` lists `wgpu =30.0.1` itself only to pin the back ends (amendment 1). `egui_plot` is in the build although only the
  Plots panel uses it (drawing a monitor store, as M6 needs); 48D fills the store (ADR 0133 Placement expects it here).
* A save writes `Case::to_toml` of the whole settings tree, as the frozen GUI did: the 23-line cavity case (419 bytes) is saved as 15,898 bytes
  with every default written out, and comments and order are lost.
* The editor's in-process service has no sandbox root, and the case text is parsed with base folder `.` (the service's rule for
  text); a case that names a relative data file (a lookup table) resolves it against the working folder, not the case's folder.
  Opening by path would fix this but makes the service read the file on the UI thread.
* Docs touched beyond the Files list: `docs/glossary.md` (the Studio entry said "It has no window yet") and `docs/releasing.md` (what
  the 1.90 job proves).
* M5 and M7 are not ported (amendment 1 item 1).
* Fixed after the registered runs (found in review, not by a test): closing the window saved the preferences on a background thread,
  which the process exit can cut off, and tab moves are only captured at close. The close path now saves on the UI thread
  (`Studio::save_prefs_now`, one small write at exit; the only file I/O of the UI thread). Unregistered test
  `closing_the_window_saves_the_layout_synchronously` (a frame carrying the close event) passes; B8 re-run: 4 of 4 pass.
* `scripts/gate_test.py --jobs 5 -- -p rcfd-studio`: 34 test processes, all pass, wall 9.9 s, peak summed working set 54 MB, max
  time/limit 0.002. `scripts/limit_audit.py` hits in the new tests, read: B7's two `return`s are the debug-build skip with a
  `SKIP` line (the limit is for release; the gate is release); `child_entry`'s `return` is the child-process pattern of
  `rcfd-project`; B6's `dt <= LIMIT` is flagged because the test sleeps, but `dt` is measured from the child's exit, after the
  sleep; the four `*_control*` tests (B1, B4, B8 twice) assert the failure they show as positive facts (`count` 2, 9 ids, a
  message and the default layout), which the heuristic does not recognise.
* Known limits: closing or switching a project joins the worker thread on the UI thread, so a watcher scan of a large project in
  progress delays it by that scan; every change seen by the watcher re-reads all run summaries (`Browser::load`), a worker-side
  cost for 48D to revisit when runs stream.

Not run: a real window (DX12, Vulkan, WARP; resizing, high-DPI, keyboard focus), the native `rfd` dialogs and the by-hand
view-and-report checklist; `cargo deny` (not installed; licences checked from `cargo metadata`); the CI `min-toolchain` job
(`cargo check --workspace --all-targets --locked --exclude rcfd-studio` on 1.90.0; 1.90 is not installed here); a Linux build
(the `x11` and `wayland` features of eframe and `rfd` 0.15.4 on Linux, as `rcfd-gui` already uses them, not checked); the full gate;
the `rcfd-render` GPU tests (unchanged code, and `cargo tree` shows the same wgpu features).

Questions for the maintainer:
1. Is the `deny.toml` exception for `epaint_default_fonts` (OFL-1.1, Ubuntu-font-1.0) accepted?
2. Should a save keep the user's file text (comments, only the changed keys), which needs a TOML editing crate the ADR does not
   name, or is the canonical full TOML of the frozen GUI acceptable?
3. Should relative data paths in an edited case resolve against the case's folder (a change in `rcfd_api`'s `session.create` for
   text with a base folder), or is the working-folder rule acceptable until 48D starts runs through the runner?
4. Move to eframe 0.37.0 once it passes the two-week rule (2026-10-29), or stay on 0.36.2 for wave 48?

## Files

`crates/rcfd-studio/` (new), root `Cargo.toml` (member), `Cargo.lock`, `crates/rcfd-cli/tests/workspace_graph.rs`,
`docs/architecture.md`, `.github/workflows/ci.yml` (only with W48-1 (b)), `README.md` (toolchain line), `THIRD_PARTY_LICENSES.md`
(regenerated by `scripts/third_party_licenses.py`), this ADR. Also touched (amendment 1, addendum): `deny.toml`, `scripts/third_party_licenses.py`,
`docs/glossary.md`, `docs/releasing.md`.
