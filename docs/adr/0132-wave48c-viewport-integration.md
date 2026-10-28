# ADR 0132: Wave 48C, the viewport: render core in the window, camera interaction, scalar display and time scrubbing

Status: **ACCEPTED 2026-10-28** (the maintainer accepted the ADR and took the recommended option of every open decision W48-1 to W48-16, scope cut (a)) (scoping only; nothing implemented or run). Library part (`rcfd-render` additions) can start at once;
integration needs ADR 0131 (48B) and, for scrubbing runner-made runs, ADR 0130 (48A, store location). House rules as in ADR 0130.
Parent: plan section 3 (L4, L5), section 5 limits 2, 3 and 6. Roadmap: F-266, F-264. Clean-room: WebGPU and wgpu public API
documentation, the egui-wgpu public API, textbook rasterisation; our own `rcfd-render` and `rcfd-post` code.

## Context (read at main 1248a76)

* `rcfd_render::Renderer` owns its device and queue (`crates/rcfd-render/src/gpu.rs` lines 128-136, private fields) and creates them in
  `from_adapter` (line 200). There is no constructor that takes a caller's device, and no way to draw into a caller's texture:
  `render` (line 562) draws into its own `Rgba8Unorm` target and reads the image back; `pick` (line 579) likewise.
* The colour target is linear `Rgba8Unorm` storing exactly `round(c)` (ADR 0125 "Colour maps"); ADR 0125 warns that a wave-48 swap
  chain must keep a linear format or convert, or colours shift.
* A field change is a new upload of the whole 52-byte-per-vertex buffer (ADR 0125 "Vertex layout", "A field change is a new upload
  (v0)"). ADR 0125 T1 measured upload of 1 000 000 triangles at 146-205 ms on the five adapters, against 2.4-68.6 ms for a frame.
* Picking returns the face exactly and, for per-face fields, the value exactly (P1b); the interpolated per-point value is an ignored
  known miss (P2 value, 1.47e-5 of the range against 1e-5).
* `rcfd-tsstore`'s `Reader` gives the mesh once (`mesh()`), snapshot lookup by step or time (`by_step`, `by_time`, `Pick`), a field
  read (`read_field`), linear interpolation in time (`field_at_time` with `Mode::Linear`, flagged `interpolated`) and `refresh()` for a
  live run (`crates/rcfd-tsstore/src/lib.rs`). ADR 0124 C7: one field of a 1M-cell store, nearest snapshot, 9.5 ms; linear 19.5 ms.
* `SurfaceMesh::from_mesh` and `face_values_from_cells` map volume cells to boundary faces by owner (`crates/rcfd-render/src/surface.rs`).
* eframe's wgpu back end exposes its render state (device, queue, target format) to the application at creation, and egui-wgpu can
  show a wgpu texture as an egui image (`register_native_texture`) or run a paint callback inside egui's render pass. These are read
  from the egui-wgpu documentation for the 0.2x/0.3x line; the exact 0.36/0.37 signatures are UNVERIFIED until step 0.

## Decisions needed

### W48-7: the route of the 3D view into the window

* (a) **Shared device, render into a texture.** Additive `Renderer::from_device(device, queue, info)` (the pipelines are built on the
  given device) and `Renderer::render_into(&GpuMesh, &View, &wgpu::TextureView)` that draws the same pipeline into a caller texture of
  format `Rgba8Unorm`; egui shows the texture as an image. No readback.
* (b) **Readback and upload.** Keep `Renderer` on its own device; every frame `render()` returns an `Image`, which is uploaded as an egui
  texture. No `rcfd-render` change, but a full readback and upload per frame (8.3 MB at 1920 x 1080; ADR 0125 T1 frames include it).
* (c) **Paint callback.** Draw inside egui's render pass with egui-wgpu's callback; the pipeline must then target the swap chain's
  format (possibly sRGB, possibly `Bgra8Unorm`) and egui's depth state.

Trade-offs: (b) has no new API but costs tens of milliseconds per frame at full size and two devices on one GPU; (c) avoids a texture
copy but ties the pipeline to egui's pass and format, which is where the colour-shift risk of ADR 0125 sits, and the window-path pixels
can no longer be compared directly with the offscreen references. **Recommendation: (a)**: one extra texture, the same pipeline and
format as the registered offscreen checks, so the window path is tested against G1/G2 by reading the texture back in a test.

### W48-8: field update while scrubbing

* (a) Additive `Renderer::update_scalar(&mut GpuMesh, Field)` that rewrites only the scalar (4 bytes per vertex, through
  `queue.write_buffer` of a separate scalar buffer, or of the scalar slots). Needs a second vertex buffer for the scalar
  (layout change inside `rcfd-render`, the 52-byte layout stays for existing callers).
* (b) Keep v0: re-upload per time step, and register the scrub budget only for meshes small enough to meet it.

Trade-offs: (a) is a layout change in a crate with registered pixel checks (G1-G3, D1 must pass unchanged); (b) makes scrubbing slow on
large meshes (about 150-200 ms per step at 1M triangles, from T1 upload times). **Recommendation: (a)**, with every ADR 0125 check run
unchanged as the guard.

## Design (proposed, assuming (a) and (a))

* **Viewport model** (non-UI, unit-tested): current run, field, time, colour range (auto per snapshot, auto over the run, or fixed),
  camera, part visibility, cut plane. Mouse mapping is a pure function: drag left = orbit (yaw, pitch in radians per pixel), drag right
  = pan, wheel = dolly by a fixed factor per notch, double-click = `Camera::fit`. Sensitivities are constants in the code.
* **Rendering**: on change only (camera, field, time, size), into a texture of the panel's pixel size; egui draws the texture plus a
  legend and the hover readout as egui widgets (text is egui's, not the render core's).
* **Time**: a slider over the run's stored snapshots, play, pause, step forward and back, and a time read-out with "interpolated"
  shown when `Mode::Linear` is used. The slider snaps to stored times by default (`Pick::Nearest`); linear interpolation is an option.
  The same time drives the plots of ADR 0133 (a marker at the current step).
* **Live runs**: the reader's `refresh()` is polled with the run status; new snapshots extend the slider.
* **Hover**: `pick` at the cursor gives face, cell and value; the value shown is the per-face (cell) value from the store, not the
  interpolated pick value (P2 known miss).
* **2D cases**: the cavity-style 2D mesh is drawn as its front patch with an orthographic camera, which replaces the frozen GUI's 2D
  raster view (parity item).
* **No adapter**: the viewport shows "no wgpu adapter" with the located error; the rest of the app works.

## Placement

`rcfd-render` gets additive API only (no new dependency; wgpu stays `=30.0.1`, which eframe 0.36/0.37 accept as `^30.0`; that
`Cargo.lock` resolves eframe's wgpu to the same 30.0.1 is checked at step 0 and is a merge condition: two wgpu versions cannot share a
device). `rcfd-studio -> rcfd-render, rcfd-tsstore`. No change to layers.

## Registered checks (proposal; GPU checks run on every adapter, with the SKIP rule of ADR 0125)

| Id | Check | Limit | Reason | Negative control |
|---|---|---|---|---|
| V1 | Window path equals offscreen: each G2 scene of ADR 0125 drawn with `from_device` + `render_into`, texture read back in the test, against `Renderer::render` on the same adapter | byte-identical | same pipeline, state and format, no MSAA (ADR 0125 D1) | the scene at yaw 37 degrees differs |
| V2 | Composite fidelity: egui-wgpu draws the viewport texture into an offscreen target of the format the Studio's window uses, headless | inside the image rectangle, max channel difference <= 1 against the texture | one rounding in egui's sampling | drawing into an sRGB target with no conversion differs by > 1 in >= 50 % of pixels of a mid-grey ramp |
| V3 | Mouse mapping | orbit, pan, dolly and fit equal the closed forms within 1e-12 (f64) | pure functions | pan with the y sign flipped fails |
| V4 | Field from the store: a store with 3 snapshots on the 8 x 8 cavity; the viewport model's face values at each snapshot | equal `face_values_from_cells(read_field(...))` bit-exactly; 64 per-face picks equal f32 of the stored cell value exactly (as P1b) | the colours must be the solver's numbers (plan limit 2) | values of the next snapshot fail |
| V5 | Time lookup: slider positions over a store with irregular times, both modes | snapshot index equals `Reader::by_time(t, Pick::Nearest)`; linear values equal `field_at_time(..., Mode::Linear)` bit-exactly and are flagged | ADR 0124 C9, C11 | a floor lookup gives a different index at a tie |
| V6 | Scalar-only update: G2 scene (c) rendered after `update_scalar` with the field of scene (a) | byte-identical to a fresh upload of scene (a); every ADR 0125 check passes unchanged | (a) of W48-8 must not change pixels | updating without rebinding the scalar buffer leaves the old image |
| V7 | Scrub cost (release): 100^3 box store of ADR 0124 C7 (6 boundary patches, 60 000 faces, 120 000 triangles) and a 1 000 000-triangle surface (ADR 0125 T1 mesh); step to the next snapshot: store read + face mapping + scalar update + render into the texture | median of 20 steps <= 100 ms on `DiscreteGpu` adapters; reported with the adapter name on every adapter | 10 steps per second, the scrubbing rate ADR 0124 C7 was set for | the same step with a full re-upload on the 1M-triangle surface exceeds 100 ms (T1 upload 160 ms) |
| V8 | Legend and range | tick end labels equal the range ends formatted with the registered precision; auto-range over a run equals min and max over all snapshots read | exact | the per-snapshot range differs on a fixture with growing values |
| V9 | No adapter (`Backends::empty()`) | the viewport model reports a located error containing "no adapter"; headless frames still draw | ADR 0125 S1 | (none) |

V7's 100 ms is a proposal derived from 10 frames per second; the 1M-triangle arm is registered knowing that T1's upload alone is about
160 ms, which is why its negative control is the re-upload path.

## Not in CI

* The real swap chain on DX12, Vulkan and WARP: that the image on screen has the colours of V2's offscreen target (a person compares a
  screenshot's sampled pixels with the saved PNG of the same view); frame pacing while orbiting; high-DPI.
* View-and-report: open a run with a stored field, orbit, scrub, hover.

## Out of scope (left for wave 49)

Iso-surfaces, streamlines, particles and vectors (compute); filled volume cut sections; animation and video export; side-by-side
compare of two runs at one time; deformation (only the point-id hook exists); MSAA and anti-aliased edges; text drawn by the render core.
Wave 50: fan-specific views.

## Risks

* eframe's wgpu may resolve to a different 30.0.x than `rcfd-render`'s `=30.0.1` pin. Then either the pin is relaxed to `~30.0` (a
  change of ADR 0125's dependency decision, for the lead) or eframe is held back.
* Two devices if (b) is ever used as a fallback: twice the GPU memory for meshes.
* Very large meshes are not measured beyond 1M triangles (plan limit 6).

## Files

`crates/rcfd-render/src/{gpu,lib}.rs`, `crates/rcfd-render/tests/` (new file for V1, V2, V6, V7), `crates/rcfd-studio/src/viewport*.rs`
and its tests, this ADR.

## Library part: set-up registered before the first registered run (2026-10-28)

Written on branch `worktree-agent-abf02dbe83f0d73a7` (based on main 3492c75) after the code and tests were written and before V1, V6
or V7 ran. The limits of the table above are unchanged; this section only fixes what the table leaves open.

**Scope of the library part.** V1, V6 and V7 (in `crates/rcfd-render/tests/viewport.rs`) and the ADR 0125 checks run unchanged.
V2 needs egui-wgpu and the Studio window's format; Placement gives `rcfd-render` no new dependency, so V2 is left to the integration
part (see the question in the addendum). V3-V5, V8 and V9 test the viewport model of `rcfd-studio` and are integration too.

**API as implemented (additive).**
* `Renderer::from_device(device, queue, info) -> Result<Renderer>`; `from_adapter` now requests a device and calls it (the pipelines
  are built by the same code). Accessors `Renderer::device()` and `Renderer::queue()`.
* `Renderer::render_into(&GpuMesh, &View, &wgpu::TextureView) -> Result<()>`: one render pass with the colour pipeline, the clear
  colour, bind group and a fresh depth buffer exactly as `render`, then a submit; no readback, no wait. The texture must be
  `Rgba8Unorm`, D2, of the view's size, one layer, one mip level, one sample, with `RENDER_ATTACHMENT` usage; otherwise a located
  `InvalidValue` error at `target.format`, `target.usage`, `target.size` or `target.levels`.
* `Renderer::update_scalar(&mut GpuMesh, Field) -> Result<()>` (W48-8 (a)). The 52-byte buffer and the two ADR 0125 pipelines are
  unchanged. The first update makes a second vertex buffer of 4 bytes per triangle vertex (`VERTEX | COPY_DST`); later updates
  `queue.write_buffer` it. A mesh that has it is drawn (colour, pick, `render_into`) by a second pipeline pair: the same shader
  module and entry points, buffer 0 with stride 52 and locations 0, 1, 2, 4, 5, buffer 1 with location 3. A mesh never updated is
  drawn exactly as before. The field is checked as by `upload` (the same function); on an error the mesh is unchanged. The colour
  range of the mesh follows the new field.
* To keep the ADR's signature (no surface argument), `GpuMesh` keeps the surface's face loops as `u32` CSR (4 bytes per face plus 4 per
  face corner on the host; about 10 MB for the 1M-triangle T1 surface) and expands the field in the fan order of `upload`.
* `#[doc(hidden)] Renderer::render_ignoring_scalar_buffer`: `render` with the scalar buffer left unbound, for V6's negative
  control only.

**V1 set-up.** The four G2 scenes of ADR 0125 (cube 4 x 4 x 4, yaw 35 degrees, 320 x 240, the scene fields of `tests/render.rs`).
Per adapter of `adapters()`: the reference is `Renderer::render` of a `from_adapter` renderer (its own device); the window path is a
renderer from `from_device` on a second device that the test requests from the same adapter, the mesh uploaded on it, drawn by
`render_into` into an `Rgba8Unorm` texture (`RENDER_ATTACHMENT | COPY_SRC | TEXTURE_BINDING`) and read back by code in the test (rows
padded to 256 bytes, padding stripped). Limit: the bytes are equal. Negative control: the window path at yaw 37 differs from the yaw-35
reference.

**V6 set-up.** On the `from_device` renderer: upload the cube with scene (c)'s field (per point), `update_scalar` with scene (a)'s
field (per face), render with scene (a)'s view; compare byte for byte with a fresh upload of scene (a)'s field under the same view.
Also asserted (same limit, byte-identical): `render_into` after the update; the pick pass after the update (ids and values) against the
fresh upload's pick; a second update back to field (c) against the original upload under scene (c)'s view. Negative control:
`render_ignoring_scalar_buffer` after the update differs from the fresh image. Every ADR 0125 test in `tests/render.rs` runs unchanged.

**V7 set-up.** Release only (ignored in debug, as ADR 0124 C7).
* Stores: the C7 layout (20 f64 fields `f00`-`f19`, 3 snapshots at times 0, 0.5, 1, values `sin(i 1e-3 + field) (1 + snapshot) + 1e5`,
  default write options), written under the test target's temporary directory and deleted afterwards. Arm 1: the 100^3 box of C7,
  surface of all 6 patches (60 000 faces, 120 000 triangles). Arm 2: the T1 mesh (500 x 500 x 1 box, size 1 x 1 x 0.01), surface
  `zmin` + `zmax` (1 000 000 triangles), store of the same layout on its 250 000 cells. The surface is built from `Reader::mesh()`.
* View: `Camera::fit(yaw 0.4, pitch 0.6, perspective fovy 0.8)` as T1, flat shading, 1920 x 1080, colour range from the field. One
  `Rgba8Unorm` texture of that size per adapter, reused by every step.
* Step: snapshot index `k -> (k + 1) mod 3`; timed from before `Reader::read_field(k, "f07")` through `face_values_from_cells`,
  `update_scalar` (`Field::Face`) and `render_into`, to the return of `device.poll(wait_indefinitely)`. Mesh first uploaded with
  snapshot 0; one untimed warm-up step (the first update makes the scalar buffer), then 20 timed steps; median of 20 = mean of the
  10th and 11th sorted values.
* Negative control (arm 2 only): the same step with `upload` in place of `update_scalar`, median of 5 steps.
* The limit and the control are asserted on `DiscreteGpu` adapters; every adapter's medians are printed with its name.

**Dependencies.** `rcfd-tsstore` (workspace crate, layer 4) becomes a dev-dependency of `rcfd-render` (layer 5) for V7. No
third-party crate is added; wgpu stays `=30.0.1`.

**What I already knew (rule d).** Before this registration: the existing ADR 0125 tests passed on the base commit (release, five
adapters; T1 medians 2.3-67.7 ms, uploads 142-172 ms), their saved PNGs were copied aside for a byte comparison after the change;
an unregistered smoke test (`render_into_and_update_scalar_reject_bad_input`) passed on all five adapters: wrong size, format (incl.
`Rgba8UnormSrgb`) and usage are located errors, a wrong-length or NaN update fails and leaves the image unchanged, and
`update_scalar(Field::None)` renders the same bytes as `upload(Field::None)`. No V1, V6 or V7 result had been seen.

## Addendum: library part, results against the registered limits (2026-10-28)

Registered run: commit 83d9296, `cargo test --release -p rcfd-render -- --test-threads=1` (`CARGO_BUILD_JOBS=2`), this machine, the
five adapters of ADR 0125 step 0 (NVIDIA RTX 5080 Vulkan and DX12, AMD Radeon integrated Vulkan and DX12, WARP). No test took the
SKIP branch. Result: 5 unit + 11 `render.rs` (2 ignored known misses of ADR 0125, unchanged) + 2 `upload_limit_wfix.rs` + 4
`viewport.rs` passed, 0 failed. No amendment was needed.

| Id | Measured | Limit | Status |
|---|---|---|---|
| V1 | all four G2 scenes on all five adapters: 0 pixels of the `render_into` texture differ from `render` (byte-identical); negative control at yaw 37: 3400 (a), 23 936 (b), 27 619 (c; 27 618 on WARP), 19 161 (d) differing pixels | byte-identical; control differs | pass |
| V6 | all five adapters: after `update_scalar` (field (c) to field (a)) 0 differing pixels against a fresh upload of (a), also through `render_into`; pick ids and values equal; updating back to (c) equals the original upload under (c)'s view; negative control (scalar buffer not bound) 28 109 differing pixels | byte-identical; control differs | pass |
| V6, ADR 0125 checks unchanged | R0, G1, G2 a-d, D1, G3, P1b, P2 depth/visibility, V1, K1, W1, S1, T1, U1 pass; the non-timing values equal those of ADR 0125's addendum (T1 timings are reported below); the 29 PNGs the tests save (G1, G2 per adapter and the CPU references) are byte-identical to those of the base commit 3492c75 (`cmp`) | pass unchanged | pass |
| V7, 100^3 box (120 000 triangles) | median step: NVIDIA Vulkan 11.0 ms, NVIDIA DX12 10.8 ms; AMD Vulkan 12.6 ms, AMD DX12 12.8 ms; WARP 179.3 ms | <= 100 ms on `DiscreteGpu` | pass (others reported) |
| V7, T1 surface (1 000 000 triangles) | median step: NVIDIA Vulkan 16.4 ms, NVIDIA DX12 12.9 ms; AMD Vulkan 19.4 ms, AMD DX12 22.8 ms; WARP 1414.4 ms | <= 100 ms on `DiscreteGpu` | pass (others reported) |
| V7 negative control (full re-upload, T1 surface) | median of 5: NVIDIA Vulkan 164.0 ms, NVIDIA DX12 155.4 ms; AMD Vulkan 208.9 ms, AMD DX12 206.4 ms; WARP 17 581.8 ms | > 100 ms on `DiscreteGpu` | discriminates |

T1 in the same run (reported, limit 250 ms on discrete GPUs): median frame 2.5 / 5.3 ms (NVIDIA Vulkan / DX12), 9.7 / 9.3 ms (AMD),
68.8 ms (WARP); uploads 146-195 ms. V7 set-up times: box store written and surface built in 2.6 s, T1-mesh store in 0.7 s; the whole
`viewport.rs` binary took 163 s, most of it WARP's 1M-triangle arm.

**Run with default test threads** (the command of the task, `cargo test --release -p rcfd-render`, same commit; the registered run
above was single-threaded): all tests pass. V7 medians: box 12.2 / 11.0 ms (NVIDIA Vulkan / DX12), 13.1 / 13.6 ms (AMD), 178.4 ms
(WARP); T1 surface 18.1 / 13.8 ms (NVIDIA), 21.1 / 22.2 ms (AMD), 1381.6 ms (WARP); re-upload control 174.2 / 159.3 ms (NVIDIA),
190.4 / 199.4 ms (AMD), 17 606.9 ms (WARP).

**Not run in the library part:** V2 (egui-wgpu composite), V3, V4, V5, V8, V9 (viewport model), the step-0 check that eframe's wgpu
resolves to the same 30.0.1 in `Cargo.lock`, and the "Not in CI" items. They belong to the integration part on the 48B shell.

**Linux/CI:** the new tests take the SKIP branch before building any fixture when `adapters()` is empty; V7 is ignored in debug
builds. Not run on Linux here (UNVERIFIED on a Linux host); nothing in the new code is platform-specific.

**Notes for the integration part.**
* `render_into` submits but does not wait; egui's frame submitted later on the same queue sees the finished image.
* WARP's scrub step on 1M triangles is about 1.4 s (and its re-upload about 17.6 s), so a software-adapter Studio cannot scrub large
  meshes at the V7 rate; the limit is registered for discrete GPUs only, as T1.
* Question for the maintainer: the Files section puts V2 in `crates/rcfd-render/tests/`, but V2 needs egui-wgpu, which Placement does
  not allow as an `rcfd-render` dependency. Proposal: V2 lives in `rcfd-studio`'s tests (the crate that has egui-wgpu anyway).

## Integration part: amendment 1, set-up registered before the first registered run (2026-10-28)

Written by the 48C integration implementer at main 7486ecb (48A, 48B, 48C library part, 48D, 48E library part, 48F merged)
after reading the code and before any integration code was written or run. The limits of V2-V5, V8 and V9 are unchanged; this
section fixes what the table leaves open, replaces two negative controls that cannot be built or cannot fail as written, and
registers V10, V11 and the B9 parity item. Maintainer answers of 2026-10-28 applied: V2 lives in `rcfd-studio`'s tests; the host
copy of the surface face list in `GpuMesh` (about 10 MB at 1M triangles) is accepted; the hidden test-only
`render_ignoring_scalar_buffer` is accepted.

**Step 0, merge condition (read 2026-10-28, `Cargo.lock` at 7486ecb).** One wgpu 30.x package, **30.0.1**; `egui-wgpu 0.36.2` and
`eframe 0.36.2` both depend on `wgpu 30.0.1` (the other wgpu is the frozen GUI's 0.19.4). eframe's render state therefore hands
the Studio a `wgpu::Device` of the same crate version as `rcfd-render`'s, and `Renderer::from_device` takes it. Read from
eframe 0.36.2 and egui-wgpu 0.36.2 sources: the window device is requested with `wgpu::Limits::default()` (plus
`max_texture_dimension_2d` 8192) on DX12/Vulkan, no features; eframe's egui renderer is made with `RendererOptions { msaa_samples:
0, depth_stencil_format: None, dithering: true, .. }` (the `NativeOptions` defaults); the swap-chain format is the first of
`Rgba8Unorm` or `Bgra8Unorm` that the surface offers (`preferred_framebuffer_format`), so both are possible; a user texture is
registered with `register_native_texture`, which needs `Rgba8Unorm`, and egui's `fs_main_gamma_framebuffer` writes the sampled
bytes unconverted (multiplied by white, plus dithering of at most half a step).

**Design as it will be implemented.**
* Modules: `viewport.rs` (model, camera mapping, store worker, GPU holder; no drawing) and `viewport_ui.rs` (drawing only). Hooks:
  one field `Studio::viewport` plus calls in `close_project` and `drain` (`studio.rs`); the `Panel::Viewport` arm (`ui.rs`); the GPU
  attached from `cc.wgpu_render_state` in `main.rs`. `Studio::new` attaches no GPU (B4, B7 and every model test stay GPU-free).
* Threads: a viewport worker thread per open project (started at the first run selection) owns the `rcfd_tsstore::Reader` of
  `runs/<id>/fields/` (the ADR 0130 D3 contract; no other location is looked at), builds the surface, reads fields and maps them to
  faces (`face_values_from_cells`), and every 250 ms (`poll::PERIOD`) calls `refresh()` while the index is scanned (a live run) or
  tries to open a store that does not exist yet. Requests are latest-wins (only the newest pending sample request is served). The
  UI thread does one `Renderer::upload` per run and surface, then `Renderer::update_scalar` for every change of time or field, and
  `render_into` only when the camera, field, time, range, size or theme changed. Counters `uploads`, `scalar_updates`, `renders` are
  kept for the tests.
* Surface: a mesh with an `Empty` patch is a 2D case: the surface is the faces of its `Empty` patches whose area vector has a
  positive z component (the front of a one-cell-thick mesh), drawn unlit with an orthographic camera at yaw 0, pitch 0
  (`Camera::fit`). Any other mesh: every patch, flat shading, perspective `fovy` 0.8 at yaw 0.6, pitch 0.4 (`Camera::fit`).
  Only cell fields are listed (the run hook writes only cell fields); the default field is `p` when stored, else the first.
* Time axis: when the stored times are strictly increasing the slider is over time `t` in [first, last] and the snapshot is
  `Reader::by_time(t, Pick::Nearest)` (snapped: the slider then shows that snapshot's time); with "linear" on the values are
  `field_at_time(field, t, Mode::Linear)`, shown as "interpolated" when the sample says so. Otherwise (steady runs store time 0 at
  every snapshot, read from `Simulation::time`) the slider is over the step and the snapshot is `Reader::by_step(step,
  Pick::Nearest)`; linear is off. Play advances one stored snapshot per 100 ms (V7's 10 per second) and stops at the last;
  step back and forward move one snapshot.
* Colour range: auto per snapshot (`View::range = None`, the min and max of the shown face values), auto over the run (the min
  and max of the face values of every stored snapshot of the field, read by the worker), or fixed (typed; refused unless finite with
  max > min). A degenerate auto range (max = min) is passed as `None` (the render core then uses a span of 1). Non-finite stored
  values: `update_scalar` refuses them; the located message is shown and the last image is kept.
* Hover: one `PickImage` is cached per rendered image (made lazily when the pointer is over the view after a change); the pixel
  is the pointer position in points times `pixels_per_point`, floored. The readout is face, owner cell and the shown face value
  (the store's cell value, f64, printed with `{}`), not the pick value (P2 known miss); "interpolated" is added in linear mode.
* Legend: an egui colour bar of `ColorMap::viridis()` with 5 ticks at `lo + k (hi - lo) / 4`, `k = 0..4`, each labelled
  `format!("{:.4e}", v)`, where `(lo, hi)` is the range the image was drawn with.
* Texture: `Rgba8Unorm`, the panel's size in pixels (`round(points x pixels_per_point)`, clamped to 1..=8192), usage
  `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC`; registered once with `register_native_texture(.., FilterMode::Nearest)` and
  re-pointed with `update_egui_texture_from_wgpu_texture` (same id) after a resize. Background `[255; 4]` in the light theme and
  `[38, 38, 38, 255]` in the dark one.
* `Studio::plot_marker` follows the viewport: the shown snapshot's step, or `s0 + w (s1 - s0)` in linear mode (`w` the sample
  weight, `s0`, `s1` the steps of `lower`, `upper`); `None` when no snapshot is shown.

**Mouse mapping (V3), constants in `viewport.rs`.** `R`, `U`, `B` are `Camera::basis()` (right, up, back); `H` the texture height
in pixels; drag deltas are converted from points to pixels (times `pixels_per_point`).
* Orbit (primary drag `dx`, `dy` pixels): `yaw' = yaw - 0.01 dx`, `pitch' = clamp(pitch + 0.01 dy, -pi/2, pi/2)`.
* Pan (secondary or middle drag): `s = 2 d tan(fovy / 2) / H` (perspective) or `s = 2 h / H` (orthographic, `h` the half height);
  `target' = target - s dx R + s dy U`.
* Dolly (wheel, `n` notches = scroll points / egui's `line_scroll_speed`, positive away from the user): `f = 0.9^n`;
  perspective `distance' = f distance`, orthographic `half_height' = f half_height` (distance kept).
* Fit (double click): `Camera::fit(lo, hi, yaw, pitch, projection)` of the surface bounds with the current yaw, pitch and
  projection kind (`fovy` 0.8).
* After every one of these, near and far are recomputed around the surface's bounding sphere (centre `c = (lo + hi) / 2`, radius
  `r = max(|hi - lo| / 2, 1e-30)`, as `Camera::fit`): `zc = -(c - eye) . B`, `near = max(zc - 1.01 r, 1e-3 r)`,
  `far = max(zc + 1.01 r, 2 near)`; for a camera from `fit` with the target at `c` this equals `fit`'s own near and far.

**Set-up of the registered checks.** GPU checks run on every adapter of `rcfd_render::adapters()` with ADR 0125's SKIP rule; all in
release with `--test-threads 1`, output to a file. "Rel. 1e-12" means `|a - b| <= 1e-12 max(1, |b|)` per f64 component.
* **V2** (`tests/viewport_v2.rs`). Per adapter: a device from the adapter (`DeviceDescriptor::default()`), a Studio `ViewGpu` built on
  it with `from_device` (the app's code path), its texture drawn by the Studio code with two contents: (i) the cube of ADR 0125 G2
  scene (b) (4 x 4 x 4 box, per-face field, flat, yaw 35 degrees, 320 x 240); (ii) a grey ramp, 256 x 16, column `x` holding
  `(x, x, x, 255)`, written with `queue.write_texture` into a texture of the same descriptor. egui-wgpu's `Renderer::new(device,
  format, RendererOptions::default())` (eframe's options) for `format` in {`Rgba8Unorm`, `Bgra8Unorm`}; the texture registered with
  `register_native_texture(.., Nearest)`; one headless egui frame (`Context::run`, screen 400 x 300 points) painting
  `layer_painter(..).image(id, rect, [0,0]-[1,1], WHITE)` with `rect` at (16, 16) points of the texture's size in points;
  tessellated with egui's `tessellate`, font textures uploaded with `update_texture`, `update_buffers`, one render pass cleared
  to black, read back. For `pixels_per_point` 1 and 2. Limit: for every pixel inside the image rectangle every channel (`Bgra8Unorm`
  swizzled to RGBA) differs from the texture's by <= 1. **Negative control (replaced):** egui-wgpu cannot draw into an sRGB target
  without converting (for an sRGB output format it picks its `fs_main_linear_framebuffer`, which converts, and a pipeline of a
  linear format cannot render into an sRGB view), so the table's control cannot be built from its public API. The equivalent
  colour-shift fault is built on the sampling side: the same ramp texture made with `view_formats: [Rgba8UnormSrgb]` and
  registered through an `Rgba8UnormSrgb` view (the bytes decoded as sRGB, written unconverted to the gamma target). It must differ by
  more than 1 in at least 50 % of the pixels of the mid-grey columns 64..=191.
* **V3** (`tests/viewport_model.rs`, no GPU). Cameras: `Camera::fit` of the box [-1, 3] x [0, 1] x [2, 2.5] in perspective (`fovy`
  0.8) and orthographic, at (yaw, pitch) (0.6, 0.3) and (-2.0, 1.2); texture 800 x 600. Operations: orbit (37, -12) and (-5, 300)
  (the second clamps the pitch); pan (37, -12) and (-5, 80), also after a dolly; dolly 1, -2.5 and 0.25 notches; fit after an orbit
  and a pan. Limit: every camera component (target, distance, yaw, pitch, half height, near, far) equals the closed forms above,
  written out in the test, within rel. 1e-12. Also registered: after a pan, the old target projects to (400 + dx, 300 + dy)
  within 1e-9 pixels (the scene follows the pointer). Negative control: the pan closed form with the `dy` sign flipped differs from
  the model's target by more than rel. 1e-12 (the comparison fails), and its projected target misses by more than 1 pixel.
* **V4** (`tests/viewport_store.rs`). Fixture written in the test with `rcfd_tsstore::Writer`: `cavity_2d(8, 1.0)` (patches `lid`,
  `walls`, `frontAndBack`), 3 snapshots at steps 10, 20, 30 and times 0.1, 0.2, 0.3, cell fields `p` = `sin(0.37 c + k) + 0.1 k`
  and `ux` = `cos(0.11 c) k`, `F64`. Into a project as the store of a run (`Project::create_run`, status finished, store at
  `runs/<id>/fields`). Through the Studio model (`select_run`, `drain`): the viewport surface has 64 faces with distinct owner
  cells 0..63; for each snapshot `k` (slider at its time, nearest), the shown face values equal `read_field(k, "p")` mapped through
  the surface's owner cells (`face_values_from_cells`) bit for bit. GPU part, per adapter: view 400 x 400, the pixel of each face is
  the floor of its projected centroid (mean of its points); `pick` at that pixel returns that face, the pick value equals the stored
  cell value as f32 bit for bit, and the hover readout's value equals the stored f64 bit for bit with the owner cell. Negative
  control: snapshot `k`'s shown values compared with snapshot `k + 1`'s expected values differ.
* **V5** (`tests/viewport_store.rs`, no GPU). The same mesh, 6 snapshots at steps 1..6 and irregular times 0.0, 0.1, 0.35, 0.4, 0.9,
  1.6. Slider positions: every snapshot time, every midpoint of two neighbours (ties), both ends and 200 positions from an LCG (seed
  48005) in [0, 1.6]. Through the worker's sampling code (`StoreView::sample`, which the worker thread calls) and once per position
  class through the Studio model: nearest: the index equals `Reader::by_time(t, Pick::Nearest)` and the values equal that
  snapshot's mapped values bit for bit; linear: the values equal `face_values_from_cells(field_at_time("p", t, Mode::Linear))` bit
  for bit, the flag equals the sample's `interpolated` (false exactly at snapshot times) and is drawn. **Negative control
  (replaced):** "a floor lookup gives a different index at a tie" cannot fail: at an exact midpoint `Pick::Nearest` returns the
  earlier snapshot (`choose` in `rcfd-tsstore`), which is also the floor. The control is a round-half-up lookup (the later
  snapshot at a tie), which must give a different index at at least one midpoint.
* **V8** (`tests/viewport_store.rs`, no GPU). Fixture: the V4 mesh with 3 snapshots whose `p` grows (`0.5 + 0.01 c + k`, k = 0, 1, 2).
  Limit: for each snapshot and each range mode the legend's end labels equal `format!("{:.4e}", lo)` and `format!("{:.4e}", hi)`
  of the range computed in the test (per snapshot: min and max of that snapshot's mapped values; over the run: min and max over all
  three); the over-run range is exact (bit equal); the labels are in a headless frame's drawn text. Negative control: the
  per-snapshot range differs from the over-run range at snapshots 0 and 2.
* **V9** (`tests/viewport_model.rs`). `ViewGpu::headless(Backends::empty())` returns an error starting with `viewport: ` and
  containing `no adapter`; a Studio given that error draws 20 headless frames with the Viewport panel shown, without a panic, and
  the error text is among the drawn shapes. No control.
* **V10** (new, `tests/viewport_store.rs`, GPU): the slider uses the scalar-only update. The V4 project in the drawn app with a
  `ViewGpu` per adapter: after the first image of the run, steps to snapshots 1, 2, 0: `uploads` stays 1, `scalar_updates` grows by
  one per step, and after each step the viewport texture (read back) equals `Renderer::render` of a fresh `upload` of the same
  surface and that snapshot's mapped values under the model's `View`, byte for byte. Negative control: before the step, the texture
  differs from the image of the snapshot about to be shown.
* **V11** (new, `tests/viewport_store.rs`, no GPU): the plot marker. With the V4 project: after each step `Studio::plot_marker`
  equals the shown snapshot's step (10, 20, 30); in linear mode at t = 0.125 it equals `10 + w (20 - 10)` bit for bit, `w` the weight of
  `Reader::field_at_time("p", 0.125, Mode::Linear)` (about 12.5); after closing the project it is `None`; a headless frame with the Plots panel floated draws. Negative control: a
  marker taken from the snapshot index (0, 1, 2) instead of its step differs.
* **B9 parity** (ADR 0131): `parity_2d_field_view_with_colour_map_legend_and_hover` replaces the ignored known miss. Through the drawn
  app with the first adapter (SKIP rule): the V4 project, the run selected, frames until the image is rendered; the legend labels
  and, after a pointer move onto face 0's pixel, the hover readout with that face's stored value are among the drawn text.

**Unregistered (reported only).** A launch smoke of the real binary on this host: `rcfd-studio <project>` with the test hook
`RCFD_STUDIO_SMOKE=<frames>` (selects the newest run, draws that many frames, prints adapter, back end, swap-chain format and the
viewport's counters to stderr, then closes the window). Whether the hook is acceptable in the shipped binary is a question.

**Dependencies.** `rcfd-studio` gains `rcfd-render` (path; layer 5) and `rcfd-mesh` (layer 1, for `PatchKind`), and `rcfd-tsstore`
moves from dev to normal dependency (layer 4); all below layer 9, as ADR 0131 Placement planned. egui-wgpu is used through eframe's
re-export `eframe::egui_wgpu` (already locked as eframe's dependency). No third-party crate is added; wgpu stays `=30.0.1`.
`docs/architecture.md` line of `rcfd-studio` is updated; `workspace_graph` is run.

**What I already knew (rule d).** The code reading above and the `Cargo.lock` reading. No integration code existed and no test
of this part had run.

## Integration part: amendment 2 (2026-10-28, V10 set-up defect, before the corrected run)

The first registered run of `tests/viewport_store.rs` (commit 87b8552) failed V10 on its first adapter before reaching its limit:
the test rendered the expected image with the `View` it read **before** the step. With the colour range "auto per snapshot"
that view carries the previous snapshot's range, so the expected image was drawn with the wrong range (66 766 pixels differed
on NVIDIA Vulkan). A test-code defect: the registered set-up says "under the model's `View`", the view the model draws the new
snapshot with. Corrected: the expected image (a fresh `upload` of the snapshot's mapped values, `Renderer::render`) is made with
`Viewport::view()` read after the step has been drawn; the negative control compares the texture read before the step with that
corrected expected image. Limits unchanged. What I knew: V4, V5, V8 and V11 passed in that run (the addendum reports them); V3
and V9 had passed in their own run.

## Addendum: integration part, results against the registered limits (2026-10-28)

Branch `worktree-agent-a90090d61490b5d71` on main 7486ecb. Registered runs: release, `--test-threads 1`, output to files, this
machine, the five adapters of ADR 0125 step 0 (NVIDIA RTX 5080 Vulkan and DX12, AMD Radeon integrated Vulkan and DX12, WARP); no
test took the SKIP branch. V3, V9 and V4, V5, V8, V11 ran on 87b8552, V10 on 801a378 (after amendment 2), V2 and the whole
`rcfd-studio` suite on 801a378. After two later commits (39cc8e8: a live-store fix and "Save PNG", below; e9d3e8e: the smoke hook
prints once) the viewport tests, parity, B4 and B7 were run again: all pass with the same values. The workspace gate was not run
(the maintainer runs it).

| Id | Measured | Limit | Status |
|---|---|---|---|
| Merge condition | `Cargo.lock`: one wgpu 30.x, 30.0.1; eframe 0.36.2 and egui-wgpu 0.36.2 depend on it; the change adds two dependency lines (`rcfd-mesh`, `rcfd-render` under `rcfd-studio`), no package | one wgpu 30.0.1 | holds |
| V2 | 5 adapters x {`Rgba8Unorm`, `Bgra8Unorm`} x ppp {1, 2}: max channel difference inside the image 1 on the four hardware adapters (cube and ramp; egui's dithering), 0 on WARP | <= 1 | pass |
| V2 control (amendment 1) | ramp through an `Rgba8UnormSrgb` view: 2048 of 2048 mid-grey pixels differ by > 1 (max 74), every adapter, format and ppp | >= 50 % | fails as required |
| V3 | 40 operations (2 projections x 2 starts x 10 inputs): worst rel. difference 0 (bit-equal); pan: the old target lands within 5.7e-14 px of centre + drag; pitch clamps at pi/2 | rel. 1e-12; 1e-9 px | pass |
| V3 control | flipped-y pan: smallest rel. difference 2.07e-2, smallest pixel miss 6.0 px | > 1e-12, > 1 px | fails as required |
| V4 | model: 3 snapshots x 64 faces bit-equal to `face_values_from_cells(read_field)`; 64 faces, owner cells 0..63; picks: 192 of 192 per adapter return the face, the f32 of the stored value bit for bit, and the stored f64 and owner cell in the readout (each adapter: 1 upload, 2 scalar updates) | bit-exact | pass |
| V4 control | snapshot k vs k + 1: 2 of 2 differ | differ | fails as required |
| V5 | 213 positions (6 snapshot times, 5 midpoints, 2 ends, 200 LCG): nearest index = `by_time(Nearest)` and values bit-equal; linear values bit-equal to `field_at_time(Linear)`, flag equal (205 interpolated, none at snapshot times); app: 3 classes x 2 modes bit-equal, "interpolated" drawn iff flagged | bit-exact | pass |
| V5 control (amendment 1) | round-half-up differs from `by_time(Nearest)` at 3 of 5 midpoints | >= 1 | fails as required |
| V8 | per snapshot: ranges (0.5, 1.13), (1.5, 2.13), (2.5, 3.13), end labels `format!("{:.4e}")` equal and drawn; over the run (0.5, 3.13) bit-equal at every snapshot, labels 5.0000e-1 .. 3.1300e0 drawn | exact | pass |
| V8 control | per-snapshot range differs from the run range at snapshots 0 and 2 | differ | fails as required |
| V9 | `viewport: no adapter (wgpu: no adapter for back ends Backends(0x0))`; drawn in 20 of 20 headless frames | located, "no adapter"; frames draw | pass |
| V10 (amendment 2) | every adapter: 3 steps byte-identical to a fresh upload (texture 531 x 379, the default layout's Viewport tab); uploads 1, scalar updates 3, renders 4 | byte-identical; no upload | pass |
| V10 first run | failed on NVIDIA Vulkan, 66 766 pixels (test-code defect, amendment 2) | | superseded |
| V10 control | texture before each step differs from the next image: 3 of 3 per adapter | differ | fails as required |
| V11 | markers 10, 20, 30; linear at t = 0.125: 12.5 (`w` = 0.24999999999999994, bit-equal); `None` after close; Plots frame drawn | exact | pass |
| V11 control | marker != snapshot index | differ | fails as required |
| B9 parity | `parity_2d_field_view_with_colour_map_legend_and_hover` on NVIDIA Vulkan: rendered, legend end labels drawn, hover `value 1.1092974268256817  (face 0, cell 0)` drawn (the stored value) | as registered | pass |

No known miss. ADR 0131 and 0133 checks re-run unchanged in the same suite (801a378): B1-B9 (B7 median 0.329 ms, control 14.7 ms),
R1-R5, R6b, R7-R12 and the live-monitor parity test pass, R6 stays the ignored known miss. `workspace_graph` 3 passed; `cargo clippy
--release -p rcfd-studio --all-targets -- -D warnings` and `cargo fmt -p rcfd-studio -- --check` clean;
`scripts/third_party_licenses.py --check` up to date.

**Unregistered.**
* `viewport_model::steady_runs_slide_over_steps`: a store with time 0 at every snapshot slides over steps (`by_step(Nearest)`), linear
  off. `viewport_live::live_store_extends_the_slider`: a run without a store reports where it looked; a store written while shown
  (header only, then 3 commits) is followed snapshot by snapshot through `refresh()`, marker following. `viewport_live::
  saved_image_equals_the_texture`: "Save PNG" reads back exactly the texture's bytes. All pass.
* Launch smoke of the real binary (e9d3e8e, `RCFD_STUDIO_SMOKE=180`, `APPDATA` pointed to a scratch folder so the maintainer's
  preferences were not touched), on the project of R8's last run (a runner-made 12 x 12 cavity, 100 f32 snapshots): with
  `WGPU_BACKEND=vulkan` and `dx12` the window opened on NVIDIA RTX 5080, swap-chain format **`Bgra8Unorm`**, the run's field `p`
  at snapshot 100 of 100 (step 400) shown, no GPU error, 1 upload, 1 render in 180 frames (render on change only), texture
  531 x 379; the process exited 0 both times. `rcfd-studio --help` prints the usage; two arguments exit 2.

**Deviations and notes.**
* V2 and V5 negative controls replaced before any run (amendment 1, with the reasons); V10 corrected after a test-code defect
  (amendment 2).
* Fixed after the registered runs (found in review, not by a registered test; covered by the unregistered live test): a live run
  whose store had no snapshot yet when opened never requested its first frame, and stepping with no snapshot could panic in a
  `clamp`. Added after the registered runs: "Save PNG..." in the viewport (the shown image through `Renderer::render`, written on a
  background thread after an `rfd` save dialog), so the "Not in CI" colour comparison has a PNG of the exact view.
* The ADR's Files list names `crates/rcfd-render/tests/` for V2; per the maintainer's answer V2 is `crates/rcfd-studio/tests/
  viewport_v2.rs`. Other touched files: `crates/rcfd-studio/{Cargo.toml, src/{lib,main,studio,ui,viewport,viewport_ui}.rs}`, tests
  `viewport_{model,store,v2,live}.rs`, `vp/mod.rs`, `parity.rs`; `Cargo.lock` (two dependency lines); `docs/architecture.md`.
  Edits to `studio.rs` (one field, `close_project`, the end of `drain`) and `ui.rs` (the `Panel::Viewport` arm) are kept small for
  the 48E merge.
* The default layout gives the Viewport tab about 531 x 379 points at 1280 x 800; enlarging it is a layout question, not changed.
* Hover runs one pick pass (a full-size readback) on the first hover after each image change, then reads the cached ids; at large
  windows that first hover costs a frame (not measured).
* A 3D (non-2D) run is drawn with all boundary patches; part visibility and the cut plane of the design list are in the model's
  `View` (`parts`, `clip`) but have no controls yet.
* The store is looked for only at `runs/<id>/fields/` (ADR 0130 D3 (b)); runs made before 48A (store at `output/fields`) show
  "no stored fields" with the path.

**Not run.** The "Not in CI" items below (a person looking at the window); Linux and the CI skip branch of the new GPU tests (no
adapter: they print SKIP before any fixture); a debug build of the new tests; the workspace gate and workspace-wide clippy; `cargo
deny` (not installed).

**View-and-report checklist for the maintainer** (the window is the maintainer's to judge):
1. Build: `cargo build --release -p rcfd-studio -p rcfd-runner -p rcfd-cli` (the Studio starts `rcfd-runner` and `rcfd` from
   its own folder; `cargo run -p rcfd-studio` alone does not build them). Then `target\release\rcfd-studio.exe <project folder>`.
2. A run with stored fields: Runs > "Start run...", pick a case, leave "Store fields for scrubbing" on, Start (a runner starts if
   none runs). A transient case (`piso`, `pimple`) gives a time slider; a steady case gives a step slider.
3. Select the run in the Runs panel: the Viewport shows field `p` coloured (viridis) with the legend at the right; check another
   field in the "field" box.
4. Mouse: left drag orbits, right or middle drag pans (the surface follows the pointer), the wheel zooms, a double click fits.
   Check that nothing clips while zooming and panning, and that a 2D cavity is seen face-on in an orthographic view.
5. Time: drag the slider, use `|<`, `<`, play, `>`, `>|`; the read-out shows snapshot, step and time; with "linear" (transient
   only) it shows "interpolated" between snapshots. Check that the Plots panel's vertical "viewport step" line follows.
6. Hover: the readout under the image shows value, face and cell; the value should match the solver's cell value.
7. Range: switch "snapshot", "whole run", "fixed" and type a min and max.
8. Colours on screen: "Save PNG..." saves the shown image; take a screenshot of the same view and compare a few sampled pixels
   (they should be equal or within 1). Note which adapter and back end the window used (set `WGPU_BACKEND=dx12` or `vulkan` to
   choose).
9. Resize the window and the panels, switch the theme (View menu), try high-DPI scaling if available; watch for stretching,
   stale images or slow orbiting, and while a live run streams, that new snapshots appear on the slider.
10. Report anything that looks wrong, with the run, adapter and what was done.

**Questions for the maintainer.**
1. The launch-smoke hook `RCFD_STUDIO_SMOKE=<frames>` is in the shipped binary (an environment variable, off unless set). Keep it
   as a test hook, or remove it?
2. "Save PNG..." was added beyond the ADR's design (for the "Not in CI" colour check). Keep it?
3. Should runs made before 48A (store at `runs/<id>/output/fields/`) also be found by the viewport (ADR 0130 D3 option (a)'s
   fallback), or stay "no stored fields"?
4. The Viewport tab of the 48B default layout is about 531 x 379 at a 1280 x 800 window. Give the viewport more of the default
   layout?
