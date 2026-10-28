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
