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
