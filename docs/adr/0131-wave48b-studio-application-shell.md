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

## Files

`crates/rcfd-studio/` (new), root `Cargo.toml` (member), `Cargo.lock`, `crates/rcfd-cli/tests/workspace_graph.rs`,
`docs/architecture.md`, `.github/workflows/ci.yml` (only with W48-1 (b)), `README.md` (toolchain line), `THIRD_PARTY_LICENSES.md`
(regenerated by `scripts/third_party_licenses.py`), this ADR.
