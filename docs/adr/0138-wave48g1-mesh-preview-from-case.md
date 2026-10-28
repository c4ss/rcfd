# ADR 0138: Wave 48G1, the mesh preview of a case (first run to first picture, item 1 of 4)

Status: **PROPOSED 2026-10-28** (implemented on branch `worktree-agent-a1e32b5483f27b132`, based on main f3bf005; checks
registered below before any of them ran). Part of 48G "first run to first picture"; items 2 (case starter menu), 3 (store
fields by default) and 4 (one-click start and follow) are separate branches. House rules of ADR 0130 apply: limits are fixed
before the registered runs; a missed limit becomes an ignored `*_known_miss` test with the limit intact; a defective set-up
is corrected by an amendment written before the corrected run; defaults of existing code stay bit-identical;
`crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded. Parent: [`docs/ui-restart-plan.md`](../ui-restart-plan.md)
sections 2 and 5; ADR 0132 (the viewport). Clean-room: our own `rcfd-api`, `rcfd-mesh`, `rcfd-render` and Studio code, the
egui public API.

## Context (read at main f3bf005)

* The maintainer reported: nothing can be seen in the Studio unless a run has stored fields. The viewport reads only
  `runs/<id>/fields/` (ADR 0132 integration amendment 1) and says "no stored fields" otherwise; with no run selected it
  shows "Select a run in the Runs panel" (`viewport_ui::panel`). A case or a mesh cannot be inspected before running.
* The solver builds a case's mesh with `rcfd_api::sim::build_mesh(&Case)` (`crates/rcfd-api/src/sim.rs` line 327): a
  generator recipe (`mesh.generator` and the `mesh.n*` counts) or a mesh file (`mesh.source = "file"`, `mesh.file`
  resolved against `Case::base_dir`). `Simulation::new` calls it first (line 932); the field-store hook stores
  `sim.geom.mesh` (`fieldstore.rs` line 120), which is the mesh the run viewport later draws.
* The runner starts `rcfd run <run>/case.snapshot --case-dir <the case's folder>` (`rcfd-runner/src/runner.rs` lines
  572-588); `rcfd run --case-dir` parses JSON when the text starts with `{`, else TOML, with relative paths against the
  given folder (`rcfd-cli/src/main.rs` `load_case_in`). The case editor's in-process session parses with base folder `.`
  (ADR 0131 deviation), so the editor's settings cannot resolve a case's relative mesh file or data table.
* The run viewport (ADR 0132) holds everything a surface picture needs: `surface_of` (a 2D case is the +z faces of its
  `Empty` patches), `CameraState::start` (2D: orthographic, yaw 0, pitch 0), `ViewGpu` (one upload per surface, the
  texture egui shows), `Viewport::prepare` (render on change), the pick pass and the mouse mapping. `rcfd_render::View`
  has per-part styles (`PartStyle::Solid`, `Hidden`; ADR 0125 V1) that the viewport does not use yet.
* `rcfd-render` draws filled triangles only (no line pass). In a 2D case's face-on view the boundary patches (`lid`,
  `walls`) are edges, not faces, and the drawn front is one patch: solid per-patch colouring alone gives one flat
  rectangle.
* The six `cases/*/case.json` files are benchmark descriptors, not case files; `rcfd run` refuses them
  ([`docs/glossary.md`](../glossary.md)). The ten `cases/*/*.toml` files are case files.

## Decision (the design as implemented)

1. **Source.** The preview shows the case asked for last (`Studio::open_case`, the browser's click) while no run is
   selected (the run selection still wins, as specified). It is built from the case **file on disk**, parsed exactly as
   `rcfd run <file> --case-dir <its folder>` parses it, so the picture is the mesh a run of that file uses; unsaved editor
   edits are not shown (the panel says so). It rebuilds when the editor's base hash of the file changes (a save or a
   reload) and when the watcher reports the case file or its mesh file changed.
2. **Build off the UI thread.** `preview::build` (read, parse, size check, `build_mesh`, `surface_of`, patch list, 2D
   overlay geometry) runs on a thread per request; at most 2 threads are alive, a third request waits. Each request
   gets a generation number; a newer request (another case, a change on disk, "build anyway") makes an older build stop
   at its next phase boundary, and any older result that still arrives is dropped. A running `build_mesh` call cannot be
   interrupted: **cancellation means "discarded", not "stopped"**. Nothing is joined on the UI thread (closing the
   project leaves a running build to end on its own).
3. **Size limits** (`preview::Limits`, the defaults are the registered values): before building, a recipe's cell
   estimate (the product of its `mesh.n*` counts, for the generators where that is the cell count: `cavity_2d`, `box_2d`,
   `channel_2d`, `wedge_2d`, `nozzle_2d`, `channel_2d_interface`, `annulus_3d`, `annulus_3d_sector`,
   `annulus_3d_interface`, `annulus_3d_stage`) at most **2,000,000 cells**, and a mesh file at most **256 MiB**; others
   (`pipe_sweep`, `axial_fan_sector`, `cylinder_ogrid`, `annulus_3d_cylindrical_interface`, OpenFOAM folders) are built
   without a pre-check. After building, the surface at most **250,000 triangles** before it is uploaded, because the
   upload is the one step on the UI thread (ADR 0125 T1: 146-205 ms per 1,000,000 triangles, so about 37-51 ms at the
   limit). Over a limit the panel shows a located message and "Build anyway" or "Show anyway". The 2D cell-edge overlay
   keeps at most **20,000 edges** (above that the edges are counted, not drawn, and the checkbox says why).
4. **Drawing reuses the run viewport.** `Viewport::show_surface` installs the built surface into the same model the run
   view uses (camera from `CameraState::start`, a frame with every face at 0, `ViewGpu` upload and `render_into` through
   `prepare`, the pick pass through `hover_at`); the new `Viewport::part_styles` are copied into `View::parts` (empty for a
   run, so run images are unchanged). 3D: one solid colour per patch (`PartStyle::Solid`, a 10-colour palette by patch
   index) or one colour for all, flat shading, patches can be hidden from the legend; hover names the picked face's patch.
   2D: the front in light grey, unlit, face-on orthographic as run fields; the cell edges (1 point) and every boundary
   patch's front edges (3 points, the patch colour) are drawn by egui over the image from the same camera, cached until
   the camera, size or content changes; hover within 6 points of an outline names its patch. `rcfd-render` is not
   changed.
5. **States.** Idle (no case), building (spinner and elapsed time; a rebuild keeps the last image), ready, too large
   (message and "anyway"), failed (the located diagnostics: `cases/a.toml: line 3: mesh.nx: ...`, at most 4 lines then
   folded). No GPU adapter: the viewport's located "no adapter" message, the counts and the patch legend still show.
6. **Hooks into shared files (small, for the 48G merges).** `studio.rs`: one field, one line each in `open_case`,
   `close_project` and `drain`. `viewport.rs`: the `part_styles` field, `show_surface`, `set_part_styles`,
   `clear_surface`, and `marker` returns `None` without a run. `viewport_ui.rs`: the no-run branch calls
   `preview_ui::panel`; `image` is `pub(crate)`. Everything else is in the new `preview.rs` and `preview_ui.rs`.

**Placement.** No new crate edge: `rcfd-studio` already depends on `rcfd-api`, `rcfd-mesh`, `rcfd-render` and
`rcfd-project` (ADR 0131 Placement). No third-party crate. `crates/rcfd-cli/tests/workspace_graph.rs` and
[`docs/architecture.md`](../architecture.md) are unchanged.

## Registered checks (fixed here, before any of them ran)

All run on this machine in release with `--test-threads 1`, output to files. GPU checks run on every adapter of
`rcfd_render::adapters()` with the SKIP rule of ADR 0125. "Bit for bit" for a surface means equal points (as f64 bits),
face loops, part names and indices, volume-mesh face ids, owner cells and point ids. Frame time is wall time around
`Context::run_ui` plus `Context::tessellate` of a 1280 x 800 headless frame of the whole app (stricter than B7, which
times `run_ui` only). Fixtures: `cases/cavity.toml` (the 16 x 16 cavity of the ADR 0131 tests, 2D), `cases/hex.toml`
(the 4 x 4 x 4 Gmsh hex box of `crates/rcfd-io/tests/data/hex_v2.2_ascii.msh`, copied to the Studio's fixtures, as
`meshes/hex.msh` through `mesh.file = "../meshes/hex.msh"`; patches `inlet`, `outlet`, `walls`), and `box_2d` cases of
`nx` x `ny` cells on the unit square written by the tests.

| Id | Check | Limit | Negative control (must fail) |
|---|---|---|---|
| G1a | Every shipped `cases/*/*.toml` (10): the preview against the solver's mesh of the same file, `Simulation::new(&Case::load(file)).geom.mesh` (when `Simulation::new` refuses the case: `build_mesh(&Case::load(file))`, named in the output) | `content_hash`, cell, face and point counts and the patch list (name, kind, size) equal; the drawn surface equals `surface_of` of the solver's mesh bit for bit; 2D flag and start camera equal | `cases/channel/profile_inlet.toml` parsed with base folder `.` (the case editor's rule) is refused (its table file is not found); `cases/wedge/wedge.toml` with `nx = 61` instead of 60 has another mesh hash |
| G1b | In a project, `cases/cavity.toml` and `cases/hex.toml` opened through the Studio model (no run selected): the preview against `Simulation::new` as in G1a, and against the surface of a store written from the solver's mesh with the hook's call (`Writer::create`, one snapshot), opened with `StoreView::open` (the run viewport's code) | as G1a; equal to the store surface bit for bit, same 2D flag; the viewport shows the preview's surface (the same `Arc`) with the run viewport's start camera of the store surface | the hex case parsed with base folder `.` does not build (its mesh file is not found) |
| G1c | A real run of each of the two fixture cases started as the runner starts it (`rcfd run <tmp>/case.snapshot --case-dir <project>/cases --out <tmp>/output --store-dir <tmp>/fields --set output.store_interval=1 --set solver.max_iterations=2 --quiet`, the binaries of this profile as in ADR 0133) | exit 0; the stored mesh's hash equals the preview's; the store's surface equals the preview's bit for bit | (G1b's controls) |
| G2 | 3D picking: the hex box, view 800 x 600, two views (yaw 0.6, pitch 0.4) and (yaw 0.6 + pi, pitch -0.4), each fitted; for every surface face whose centroid pixel picks that face, the readout; then, through the drawn app, the pointer on a visible face of each patch | each readout starts with `patch <name> (` where `<name>` is the patch whose face range holds the face's volume-mesh face (from the mesh, not the surface's parts); all 3 patches seen; the 3 readouts drawn | the readout through the reversed part list names a wrong patch at least once |
| G3 | UI frames while a 1000 x 1000 `box_2d` (1,000,000 cells) preview builds: from the frame after `open_case` until the preview settles, a frame every 5 ms | set-up: at least 20 frames and a build of at least 0.5 s; median frame at most **8 ms** (B7's limit), no frame above **250 ms** | building on the UI thread (test switch) gives a frame above 250 ms |
| G4 | Failure paths through the Studio model: (a) a TOML syntax error; (b) a missing mesh file, and an `annulus_3d_sector` with `r_out < r_in`; (c) a 30,000 x 30,000 `box_2d`, then the cavity with the cell limit set to 100 and "build anyway"; (d) the hex case with the triangle limit set to 100 and "show anyway"; (e) no adapter (`ViewGpu::headless(Backends::empty())`), 20 frames of the drawn app | (a) failed, the message starts `cases/bad.toml: line `, nothing shown; (b) messages start `cases/nofile.toml: mesh.file: ` (and name `missing.msh`) and `cases/sector.toml: mesh.r_out: `; (c) too large, the message starts `cases/huge.toml: about 900,000,000 cells` and names the limit `2,000,000`, within 5 s and with no mesh built; the cavity refused, then built once (256 cells); (d) too large, `cases/hex.toml: the surface has 192 triangles`, not shown; shown after "show anyway" without a rebuild; (e) the error starts `viewport: ` and contains `no adapter`; every frame draws it, the counts line, the 3 patch names and "no GPU attached", no panic | (each arm's refusal is itself the failure shown) |
| G5 | Cancellation: A (the 1000 x 1000 box) asked, B (the cavity) asked 50 ms after A's build started | set-up: A still building when B is asked, A's thread at least 0.5 s; A's preview never shown after B was asked; B shown within **2 s** of its request; at least 1 result discarded; the viewport shows B's surface | with the generation check off (test switch: no early stop, every result applied) A's result is shown after B's |
| G6 | Through the drawn app (default layout), the hex box (3D) and the cavity (2D): the viewport texture read back | byte-identical to `Renderer::render` of a fresh upload of the same surface (every face 0) under `Viewport::view()`; cavity: at each of the 64 outline midpoints, 3 px inside (away from the image of the box centre) is exactly the front colour (222, 222, 222, 255) and 3 px outside the view's background | the render with other part styles (single colour in 3D, none in 2D) differs; 3 px outside is not the front colour at every midpoint |
| G7 | Every shipped case file: the 10 TOML cases and the 6 benchmark descriptors | the 10 build without error (G1a); each descriptor is refused with a located message (every line starts with its file) | (the descriptors) |
| G8 | The 2D overlay at its edge limit: a 100 x 99 `box_2d` (19,999 cell edges), the camera panned 1 px before each of 100 frames (the overlay is rebuilt every frame) | set-up: the drawn overlay mesh has at least 90 % of 4 x (edges + outline segments) vertices; median frame at most **8 ms** | 1000 x 500 (1,001,500 edges, the overlay limit raised): median of 10 frames above 8 ms |
| G9 | 2D boundary hover on the cavity: `outline_at` at the projected midpoint of each outline segment (800 x 600, tolerance 3 px); then the drawn app with the pointer on a `lid` midpoint | the 64 segments name `lid` where the midpoint has y = 1 and `walls` elsewhere (from the coordinates); `patch lid (wall), boundary` drawn | at the image centre no outline is found and no `..., boundary` readout is drawn |
| G10 | The frame that uploads a preview at the triangle limit: a 500 x 250 `box_2d` (250,000 triangles) after 5 warm-up frames | at most **100 ms** on `DiscreteGpu` adapters, reported on every adapter | 1000 x 500 (1,000,000 triangles, shown anyway): above 100 ms on `DiscreteGpu` adapters |
| G11 | The rest of the `rcfd-studio` suite (ADR 0131, 0132, 0133, 0134 checks), `workspace_graph`, clippy, `cargo fmt -p rcfd-studio -- --check`, `scripts/third_party_licenses.py --check` | pass unchanged (V4 and V10 upload counts included) | (their own) |

Derivations: 8 ms is B7's half of a 60 Hz frame; 250 ms is far below the at least 0.5 s build that the control puts on
the UI thread, and above the scheduling hiccups of a machine where other builds run; 100 ms (G10) is the ADR 0132 V7 scrub
step, and the triangle limit is chosen from ADR 0125 T1 so that the upload stays well under it on discrete GPUs; 2 s
(G5) is the cavity's build (milliseconds) plus slack, far below A's build. The cell and file limits are proposals (memory
and seconds of build), not measured; G3 reports the build time of 1,000,000 cells. The cell estimate's exactness is an
unregistered test (`estimate_equals_the_built_cells`).

"No UI-thread stall" means here: the build never runs on the UI thread (G3), and the one GPU step there, the upload, is
bounded by the triangle limit (G10). Over the limits, "anyway" accepts a longer upload frame (the message says the window
pauses).

**What I already knew (rule d).** The code reading above; `cargo check -p rcfd-studio` and a release build of the tests
(no test binary of this ADR run). No preview check, smoke or measurement had run when this section was written.

## Not in CI

* A person looking at the real window: colours, the overlay on a real swap chain, high-DPI, the feel of orbiting a fan
  mesh, the spinner.
* The real swap chain's upload frame (G10 times the test device, not eframe's).

## View-and-report checklist for the maintainer

1. Build: `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli`, then
   `target\release\rcfd-studio.exe <project folder>`.
2. Make a project with cases: copy shipped case files into its `cases/` (for `cases/channel/profile_inlet.toml` also copy
   `profile_inlet.csv` next to it). Open the project; do not select a run.
3. Click a 2D case (for example `cavity`-like or `channel`): the Viewport shows "Mesh of cases/...", a spinner, then the
   front in light grey with dark cell edges and coloured boundary outlines; the legend lists the patches. Hover an
   outline: `patch <name> (<kind>), boundary`; hover inside: the cell. Wheel zooms, right drag pans, double click fits;
   "cell edges" switches the edges off.
4. Click a 3D case (`axial_fan_sector`, `axial_fan_cgrid`): patch colours with flat shading, orbit with the left drag,
   hide a patch with its legend checkbox, try "single colour"; hover names the patch. Note how long the first picture
   takes.
5. Edit `mesh.nx` in the case editor: the panel says unsaved edits are not shown; save: the preview rebuilds.
6. Write a case with `[mesh] source = "file"` and a missing file, or `nx = 3000`, `ny = 3000` on `box_2d`: the located
   error, or the "too large" message with "Build anyway" (only click it with memory to spare).
7. Select a run: the viewport shows its fields as before. (Getting back to the case mesh needs a project reopen; see
   question 1.)
8. Report anything that looks wrong, slow or confusing, with the case, the adapter and what was done.

## Questions for the maintainer (not decided here)

1. With a run selected the preview cannot be reached again (opening a case does not clear the run selection, and there
   is no "deselect run"). Options: (a) opening a case clears the run selection (plots and log then empty); (b) a
   "case mesh / run" switch in the Viewport panel; (c) the viewport follows the last thing clicked (case or run), the run
   stays selected for plots and log. Recommendation: (c), it keeps every panel's meaning and needs no new control.
2. Should the preview follow unsaved editor edits (a live preview while editing `mesh.*`), or stay on the saved file (as
   built: what a run uses)? Following edits needs the editor's settings resolved against the case's folder (the
   editor's session uses `.`; ADR 0131 question 3).
3. The start dialog's `--set` overrides (for example `mesh.nx=32`) are not reflected in the preview. Show them there?
4. A GPU line pass in `rcfd-render` (wireframe and patch edges with hidden lines in 3D) in a later sub-wave? The 2D
   overlay is drawn by egui because it needs no depth test; 3D has no edges now.
5. Bare mesh files under `meshes/` (not referenced by a case): preview them when clicked in the browser?
6. Are the limits right: 2,000,000 estimated cells, 256 MiB mesh file, 250,000 surface triangles, 20,000 overlay edges?
   The upload could also move off the UI thread (a worker uploading on a clone of eframe's device), which would lift
   the triangle limit's reason; that changes `ViewGpu`'s ownership.
7. A benchmark descriptor copied into a project's `cases/` is refused with the full list of unknown keys; show a short
   "not a case file" message instead?

## Files

New: `crates/rcfd-studio/src/{preview,preview_ui}.rs`, `crates/rcfd-studio/tests/{preview_g1,preview_gpu_g1,
preview_cost_g1}.rs`, `crates/rcfd-studio/tests/pv/mod.rs`, `crates/rcfd-studio/tests/fixtures/hex_v2.2_ascii.msh`
(copy of `crates/rcfd-io/tests/data/hex_v2.2_ascii.msh`), this ADR. Changed: `crates/rcfd-studio/src/{lib,studio,
viewport,viewport_ui}.rs` (hooks listed in decision 6).
