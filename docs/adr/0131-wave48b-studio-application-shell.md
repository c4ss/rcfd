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

1. W48-9 copy: `schema_model.rs`, `monitors.rs` and `layout.rs` are copied from `crates/rcfd-gui/src/` at main 15b00ef (the files'
   last change is 3db01e0). `layout.rs` gets the Studio's panels (Browser, Case editor, Runs, Viewport, Plots, Log, Chat, Outputs)
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

## Files

`crates/rcfd-studio/` (new), root `Cargo.toml` (member), `Cargo.lock`, `crates/rcfd-cli/tests/workspace_graph.rs`,
`docs/architecture.md`, `.github/workflows/ci.yml` (only with W48-1 (b)), `README.md` (toolchain line), `THIRD_PARTY_LICENSES.md`
(regenerated by `scripts/third_party_licenses.py`), this ADR.
