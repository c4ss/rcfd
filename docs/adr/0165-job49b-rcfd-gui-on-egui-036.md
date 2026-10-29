# ADR 0165: Job 49B, the legacy desktop app `rcfd-gui` moves to eframe/egui 0.36.2, egui_dock 0.21.1 and egui_plot 0.37.0 (Dependabot 7, 8, 10), one egui version set in the workspace

Status: **registered** 2026-10-29, before any code of this job was written and before any build or run of this job (set-up,
limits and the not-run list are fixed here); results in the addendum. House rules of
[ADR 0130](0130-wave48a-overview-and-run-record-contract.md): limits fixed before the runs; a missed limit is reported, not
hidden, and becomes an ignored `*_known_miss` test where it is a test; defaults stay bit-identical (only the GUI crate, its
manifest, CI configuration, the lock file and documentation are touched; no solver or library code);
`crates/rcfd-api/tests/baseline_hashes.rs` is not edited or re-recorded; no unsafe code; no new crate in the lock file
(expected, limit L5). Parents: [ADR 0004](0004-gui-assistant-msrv.md) and [ADR 0095](0095-wave39d-field-registers-monitor-frames-fan-vent-curves.md) (rcfd-gui), [ADR 0131](0131-wave48b-studio-application-shell.md)
(W48-1 (b): the Studio states `rust-version = "1.95"` and is excluded from the `min-toolchain` job; W48-2 (a) with amendment 1:
eframe 0.36.2, the two-week rule of ADR 0125), [docs/ui-restart-plan.md](../ui-restart-plan.md) (rcfd-gui is frozen: kept
building and tested until the Studio reaches parity, then removed in a separate, explicitly requested step). Number: ADR 0164 is
taken by job 49A (commit 590fe32 on another branch); no file or commit for 0165 exists in the main checkout or any worktree on
disk (checked 2026-10-29 just before writing). Base: local main 35cc6a1 (the worktree branch was reset to it before any work).

## The problem

Dependabot pull requests 7 (eframe 0.27.2 -> 0.36.2), 8 (egui_dock 0.12.0 -> 0.21.1) and 10 (egui_plot 0.27.2 -> 0.37.0) bump
`crates/rcfd-gui/Cargo.toml` one crate at a time. Each is red: the three crates are coupled to one egui version, so none of them
compiles alone, and egui 0.27 -> 0.36 changed the API rcfd-gui uses (panels and the eframe `App` are driven from a root `Ui`
instead of a `Context`; `Context::run` is gone in favour of `run_ui`; menus, `id_source` and plot constructors changed). The
workspace meanwhile carries two complete egui stacks in `Cargo.lock` (0.27.2 for rcfd-gui, 0.36.2 for the Studio).

## Decision (set-up)

S1. **Versions.** rcfd-gui takes exactly the versions of the three pull requests and of the Studio: `eframe = "0.36.2"`,
`egui = "0.36.2"`, `egui_dock = { version = "0.21.1", features = ["serde"] }`, `egui_plot = "0.37.0"`; `rfd = "=0.15.4"`
unchanged. All of them are already in `Cargo.lock` for the Studio and pass the two-week rule (ADR 0131 step 0 table: 2026-09-08,
2026-08-06, 2026-08-05). The lock file is re-resolved without `cargo update`: only rcfd-gui's own edges change and the 0.27
stack falls out. Dependabot 11 and 9 (wat 1.262.0, miniz_oxide 0.9.1, already on main) are not undone.

S2. **Back end: glow (OpenGL), as before, chosen explicitly.** eframe 0.36 still has the `glow` feature (egui_glow 0.36.2,
glow 0.17, glutin 0.32); all of those packages are already in `Cargo.lock` (eframe 0.36.2's entry lists them), so glow adds no
package. Reasons: it keeps the legacy app's renderer unchanged (OpenGL, the 2D raster viewport needs nothing from wgpu), and the
frozen app keeps working on machines where only OpenGL is available, while the Studio needs DX12 or Vulkan (its `wgpu` entry).
Features: `default-features = false, features = ["glow", "default_fonts", "x11", "wayland"]`, the old set. Trap handled:
cargo unifies eframe's features across the packages of one build, so `cargo build --workspace` compiles one eframe with both
`glow` (rcfd-gui) and `wgpu_no_default_features` (Studio), and eframe 0.36's `Renderer::default()` then picks wgpu. rcfd-gui
therefore sets `renderer: eframe::Renderer::Glow` in its `NativeOptions`, so it renders with OpenGL whatever else is in the
build; the Studio's default stays wgpu in such a build (that `Default` returns wgpu when both are present), so the Studio is not
affected. Rejected: following the Studio onto wgpu (it would need rcfd-gui to pin `wgpu =30.0.1` with the Studio's back ends
to get any back end when built alone, and would change the legacy app's renderer).

S3. **Port, behaviour unchanged.** The UI code is rewritten only where the API forces it, following the Studio's use of the
same versions (`crates/rcfd-studio/src/ui.rs`):
* `RcfdApp::ui` takes the root `&mut egui::Ui` (as `StudioApp::ui`); `eframe::App::ui` calls it. Same panels in the same order:
  menu bar (top panel), status bar (bottom panel), the typed-path window, then the dock area in a central panel with the frame
  egui_dock 0.12's `DockArea::show` used (central-panel frame, zero inner margin, transparent fill).
* Menus: `egui::menu::bar` -> `egui::MenuBar::new().ui`, `ui.close_menu()` -> `ui.close()`; `id_source` -> `id_salt`;
  `ComboBox::from_id_source` -> `from_id_salt`; `egui_plot::Line::new(PlotPoints)` + `.name(n)` -> `Line::new(n, points)`;
  any other rename the compiler reports is applied one to one and listed in the addendum.
* Same strings, ids, colours, sizes, repaint intervals, preferences file format (the layout is still egui_dock's own serde JSON
  inside `gui-prefs.json`).
* `main.rs`: `run_native`'s creator returns `Ok(Box::new(..))` if the 0.36 signature asks for it; window size 1280 x 800 and
  title as before; `renderer: Glow` (S2).

S4. **Toolchain.** rcfd-gui states `rust-version = "1.95"` (eframe 0.36 needs it). The CI job `min-toolchain` keeps its name
(`type-checks with Rust 1.90 (README minimum; the Studio needs 1.95)`, so a required-check name does not change) and adds
`--exclude rcfd-gui` next to `--exclude rcfd-studio`, with the comment saying why; README, `docs/releasing.md` and
`docs/architecture.md` say that both desktop apps need 1.95. `gate.yml` has no Studio special case (it runs on 1.99) and is not
changed.

S5. **Licences.** `THIRD_PARTY_LICENSES.md` is regenerated with `scripts/third_party_licenses.py`. The script's notice about
`epaint` (0.27 bundled the fonts, licence `... AND OFL-1.1 AND LicenseRef-UFL-1.0`, rcfd-gui only) is replaced: egui 0.36's
`epaint` is `MIT OR Apache-2.0` and the fonts are in `epaint_default_fonts`, now in both desktop apps. `deny.toml` drops the
`epaint` exception only if no `epaint` in the lock still declares those licences (checked from the regenerated file).

S6. **Tests: API-only edits.** rcfd-gui's tests keep every assertion. Allowed edits: the headless frame helpers
(`ctx.run(input, |ctx| app.ui(ctx))` -> `ctx.run_ui(input, |ui| app.ui(ui))`) and any renamed type, field or method the
compiler reports in test code (for example in `epaint::Shape` matching); each is listed in the addendum.

## Limits (fixed before the runs)

| id | what | limit |
| --- | --- | --- |
| L1 | `cargo build --release --locked -p rcfd-gui -p rcfd-studio` (both egui consumers link) | succeeds |
| L2 | `cargo clippy --workspace --all-targets --locked -- -D warnings` (the CI clippy job; type-checks every crate and test of the workspace) | clean |
| L3 | `cargo test -p rcfd-gui --release --locked --no-fail-fast` (unit tests and all six integration test files) | 0 failures; the ignored set equals the one before the port (`v11_cavity_raster_has_one_vortex_speed_minimum_known_miss` only); test edits only of the S6 kind |
| L4 | `Cargo.lock`: one version each of `eframe`, `egui`, `egui_dock`, `egui_plot`, `epaint`, `emath`, `ecolor`, `egui-winit`, `egui-wgpu`, `egui_glow`; `cargo tree --workspace --locked -d` lists none of them | holds |
| L5 | `git diff main -- Cargo.lock`: packages only removed (no package added, no version of a kept package changed), apart from rcfd-gui's own dependency list | holds |
| L6 | `python scripts/third_party_licenses.py --check` after regeneration; every licence expression in the regenerated file is allowed by `deny.toml` (allow list or a crate exception) | up to date; all allowed |
| L7 | `cargo fmt -p rcfd-gui --check` | clean |
| L8 | the min-toolchain command with `--exclude rcfd-studio --exclude rcfd-gui` selects no package whose `rust-version` is above 1.90 (`cargo metadata`, a script reading each selected workspace member's `rust-version`) | holds |
| L9 | behaviour: the renderer rcfd-gui asks for is glow in a build with both eframe back ends (a unit test in rcfd-gui asserting `native_options().renderer == Renderer::Glow`, run inside L3 and in a `--workspace`-style build by L2's type-check plus one `cargo test -p rcfd-gui -p rcfd-studio --release --locked --lib`) | passes |

How behaviour is checked (no window): the existing headless tests run the real UI code frame by frame: `smoke.rs` (frames,
light/dark theme switch, every panel hidden/shown/floated while drawing, a solver run drawn in monitors and viewport, an
assistant turn with the mock provider approving and denying tool cards), `viewport_raster.rs` (v9: the painted raster colours
and the 48-step legend match the colormap of the run's grid; frames at 7 window sizes give finite geometry; v10/v11 cavity
checks on the raster), `monitors_plot.rs` (M7: a long run driven by headless frames with the Monitors panel and its plots),
`key_security.rs` (frames with the key field), `logic.rs` and `files.rs` (non-UI logic, layout JSON round trip).

## Not run (by design)

* A real window (no display session for an agent): how the app looks, OpenGL context creation on this machine, the native file
  dialog, keyboard/mouse input on a real window. Not inspected visually (as before the port: ADR 0004/0095 are headless only).
* A preferences file written by the 0.27 build (egui_dock 0.12 JSON) read by the port: not tested. By the code,
  `Prefs::dock` falls back to the default layout when the layout does not parse and the theme is still restored.
* `cargo deny check` itself (cargo-deny is not installed here); replaced by L6's licence comparison. The CI deny job runs it.
* The Rust 1.90 type-check itself (no 1.90 toolchain installed here); replaced by L8.
* The Studio's integration tests (its dependency versions are unchanged by L5; only eframe's feature set in a joint build gains
  `glow`, covered by L2 and L9's joint `--lib` run).
* The full gate and Linux/macOS builds.

## Addendum (results, 2026-10-29)

Branch `job49b-egui-036-rcfd-gui`, base local main 35cc6a1. Commits: c54042f (this registration, before any code or build),
ad4fbad (the port, manifests, lock file, CI, docs, licences), then this addendum. `CARGO_TARGET_DIR=E:\projects\cfd\rcfd\target-49b`,
Rust 1.99.0, Windows 11, one heavy job at a time (free RAM 12 to 30 GB before each).

| id | limit | result | verdict |
| --- | --- | --- | --- |
| L1 | `cargo build --release --locked -p rcfd-gui -p rcfd-studio` succeeds | finished in 1 m 31 s, no warning (a joint build: eframe with `glow` and `wgpu_no_default_features`, shown by `cargo tree -e features -i eframe`) | met |
| L2 | workspace clippy `--all-targets --locked -D warnings` clean | exit 0, no warning; all 22 workspace crates checked | met |
| L3 | `cargo test -p rcfd-gui --release --locked --no-fail-fast`: 0 failures, same ignored set as before, S6 edits only | 49 passed, 0 failed, 4 ignored (lib 1, files 7, key_security 3, logic 10, monitors_plot 7, smoke 6, viewport_raster 15 + 4 ignored); the 4 ignored tests are exactly main's 4 `#[ignore]` (see D-1); test counts per file equal main's | met |
| L4 | one version of each egui-family package; `cargo tree -d` lists none | `Cargo.lock`: eframe, egui, egui-winit, egui-wgpu, egui_glow, epaint, emath, ecolor, epaint_default_fonts 0.36.2, egui_dock 0.21.1, egui_plot 0.37.0, one each; `cargo tree --workspace -d` lists no egui-family package | met |
| L5 | lock: packages only removed | 577 -> 469 packages: 0 added, 108 removed (the 0.27 stack: eframe/egui/egui_dock/egui_plot 0.27, winit 0.29, glutin 0.31, glow 0.13, wgpu 0.19 and its naga/metal/gpu-alloc family, image 0.24, png 0.17, windows 0.52, windows-sys 0.45/0.48 and their targets, ...); no kept package changed version. Dependency lists changed only by dropping the version suffix where a name is now unique, plus one new edge `eframe 0.36.2 -> glow` (glow 0.17.0 was already locked through egui_glow; eframe uses it directly once its `glow` feature is on). wat 1.262.0 and miniz_oxide 0.9.1 kept | met |
| L6 | licence file up to date; every licence allowed by `deny.toml` | regenerated (13 + / 126 - lines), `--check` "up to date"; every expression in the summary is satisfied by the allow list, the only non-allow-list licences (`OFL-1.1`, `Ubuntu-font-1.0`) are on `epaint_default_fonts`, which has the exception; `LicenseRef-UFL-1.0` no longer occurs (it was `epaint` 0.27 only), so that exception was removed | met |
| L7 | `cargo fmt -p rcfd-gui --check` clean | exit 0 (`cargo fmt -p rcfd-gui` was applied before ad4fbad) | met |
| L8 | no package checked by the min-toolchain job has `rust-version` above 1.90 | the 20 checked members: highest 1.90 (rcfd-render); rcfd-gui and rcfd-studio (1.95) excluded. Extra, not registered: the feature-blind dependency closure of the 20 members on x86_64 Linux (145 packages) has no `rust-version` above 1.90 and no egui-family package | met |
| L9 | glow asked for in a build with both back ends | `ui::tests::native_options_ask_for_glow_at_1280_by_800 ... ok` in L3 (glow only) and in the joint `cargo test -p rcfd-gui -p rcfd-studio --release --locked --lib` (both back ends; the Studio's 6 lib tests also pass there) | met |

What changed in the app's code (all API-forced, behaviour kept):
* `RcfdApp::ui(&mut self, ui: &mut egui::Ui)` (was `&egui::Context`); `eframe::App::ui` replaces `update`.
* Menu bar and status bar: `egui::Panel::top/bottom(..).show(ui, ..)` (were `TopBottomPanel::..show(ctx, ..)`);
  `egui::MenuBar::new().config(..).ui` (was `egui::menu::bar`); `ui.close()` (was `ui.close_menu()`). The menu config sets
  `PopupCloseBehavior::CloseOnClickOutside`: egui 0.36's default closes a menu on any click inside it, while 0.27 kept it open
  until a click outside or `close_menu` (so the View menu's panel checkboxes still leave the menu open).
* Dock: `DockArea::show_inside` inside `egui::CentralPanel` with the frame egui_dock 0.12's `show(ctx)` used
  (`Frame::central_panel(..)`, inner margin 0, transparent fill). `TabViewer::id` is now required; it returns 0.12's default
  (`Id::new(title text)`).
* `id_source` -> `id_salt`, `ComboBox::from_id_source` -> `from_id_salt`, `egui_plot::Line::new(name, points)` (was
  `Line::new(points).name(name)`).
* Theme: `Theme::apply` calls `ctx.set_theme(..)` and then `ctx.set_visuals(..)` as before. egui 0.36 keeps a dark and a light
  style and by default follows the OS theme; `set_visuals` alone writes only the slot of the current theme, so without
  `set_theme` an OS theme change could switch the app's theme. New `Theme::egui()`.
* `main.rs`: the options come from the new `ui::native_options()` (1280 x 800, `Renderer::Glow`, title unset so eframe names
  the window "rcfd" as before); the app creator returns `Ok(Box::new(..))`.
* Test edits (S6, API only, 7 lines): `ctx.run(input, |ctx| app.ui(ctx))` -> `ctx.run_ui(input, |ui| app.ui(ui))` in
  `key_security.rs`, `monitors_plot.rs` (2), `smoke.rs`, `viewport_raster.rs`; `ctx.style()` -> `ctx.global_style()` in
  `smoke.rs` (2; `Context::style` no longer exists). No assertion changed. One new unit test (L9).

Other files: `crates/rcfd-gui/Cargo.toml` (versions as the three PRs, `rust-version = "1.95"`), `Cargo.lock`, `.github/workflows/ci.yml`
(`--exclude rcfd-gui`, comments; job name kept), `README.md`, `docs/releasing.md`, `docs/architecture.md`,
`scripts/third_party_licenses.py` (font notice), `THIRD_PARTY_LICENSES.md`, `deny.toml` (the `epaint` exception removed).
`gate.yml` unchanged (it has no Studio special case). No solver or library code, no `baseline_hashes.rs`, no unsafe.

What differs from the 0.27 build although the code keeps its behaviour (UNVERIFIED visually):
* egui's own look between 0.27 and 0.36 (default visuals, spacing, fonts now from `epaint_default_fonts`, menu and popup
  widgets) is egui's, not the app's; the headless tests do not check appearance.
* A `gui-prefs.json` written by the 0.27 build: its layout is egui_dock 0.12 JSON. If 0.21 cannot read it, `Prefs::dock`
  returns `None` and the default layout is used once (the theme is still restored, it is a separate field); not tested.
* In a joint build (`cargo build --workspace`) the Studio binary also links eframe's glow code (unused: eframe picks wgpu when
  both are present and has no automatic fallback from wgpu to glow, read in eframe 0.36.2 `init_native`/`run_native_ext`).
  `scripts/studio.*` build the Studio without rcfd-gui, so that binary is unchanged.

### Deviations

D-1. Registration text error: L3 says the ignored set before the port is "`v11_cavity_raster_has_one_vortex_speed_minimum_known_miss`
only". Main has four ignored rcfd-gui tests (`v1_luminance_non_decreasing_1001_samples_known_miss`, `v3_hover_mapping`,
`v4_legend_ticks_random_ranges_unguarded`, `v11_cavity_raster_has_one_vortex_speed_minimum_known_miss`, all in
`viewport_raster.rs`). The limit is "equals the one before the port"; it was checked against main's `#[ignore]` lines, not
against the wrong parenthesis.

D-2. S3 named "any other rename the compiler reports"; the compiler reported only the missing `TabViewer::id`; the menu close
behaviour, the theme and the dock frame were found by reading the egui/egui_dock 0.36/0.21 sources, not by the compiler.

### Not run

As registered: a real window (look, OpenGL context creation, native file dialog, real input); reading a 0.27-era
`gui-prefs.json`; `cargo deny check` (not installed; L6 instead, the CI deny job runs it); the Rust 1.90 type-check (no 1.90
toolchain; L8 instead); the Studio's integration tests; the full gate; Linux/macOS builds.

### Questions

Q1. **Retire rcfd-gui?** The port shows that the legacy app is a second, frozen copy of what the Studio does (settings editor,
monitors and plots, log, run control, docked panels; the Studio has its own versions of all of these and a 3D viewport), and
every egui bump now has to be ported twice. Options: (a) keep it until the parity checks of `docs/ui-restart-plan.md` section 6
pass, then remove it in a separate, requested step (the plan's rule); (b) remove it now; (c) keep it indefinitely as the OpenGL
fallback app. **Recommendation: (a)**; the cost of keeping it until then is this kind of port (about 60 changed lines here).
The assistant sidebar with API providers (rcfd-assistant) exists only in rcfd-gui; whether the Studio's Claude Code sidebar
replaces it is part of that parity decision.

Q2. **CI job name.** The `min-toolchain` job is still called "type-checks with Rust 1.90 (README minimum; the Studio needs
1.95)", although rcfd-gui is now excluded too. Options: (a) keep the name (a required status check of that name keeps
matching); (b) rename it to "... (README minimum; the desktop apps need 1.95)" and update the branch protection rule in the
same step. **Recommendation: (b)** when the maintainer next edits branch protection, otherwise (a).

Q3. **Dependabot 7, 8, 10.** This branch contains the three bumps together (they cannot land separately). Once it is on main,
Dependabot should close the three pull requests itself as their versions are already met; if not, close them by hand.
**Recommendation:** merge this branch instead of the three PRs; do not rebase them.
