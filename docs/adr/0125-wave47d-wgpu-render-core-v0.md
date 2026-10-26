# ADR 0125: Wave 47D, the wgpu render core v0 (`rcfd-render`, layer L4 of the Studio plan)

Status: **ACCEPTED 2026-10-26** (limits and decision rules registered before any registered check ran). Written at main f307f59 on branch
`w47d` (worktree `<scratch dir>`, target `<target dir>`). House rules of ADRs 0110-0121 apply:
* a missed limit becomes an ignored `*_known_miss` test with the limit intact;
* a defective set-up gets a new corrected test, registered here as an amendment before it runs;
* defaults of existing code stay bit-identical, and `crates/rcfd-api/tests/baseline_hashes.rs` is not re-recorded.

Parent plan: [ui-restart-plan.md](../ui-restart-plan.md), layer L4 and the wave-47 item "wgpu offscreen render core v0". Clean-room:
the WebGPU and wgpu public API documentation, textbook rasterisation (edge functions, the top-left fill rule, perspective-correct
interpolation) and our own code (`rcfd-post` `render.rs` and `gltf.rs`). No windowing, no egui.

## Step 0: does wgpu work on this machine? (spike, run before any design)

Machine: Windows 11 Pro 10.0.26300, rustc 1.99.0, wgpu **30.0.1**. A throw-away example enumerated every DX12 and Vulkan adapter. On
each one it drew the triangle (-1,-1), (1,-1), (-1,1) in clip space into a 64 x 64 `Rgba8Unorm` texture and read it back. Result, verbatim:

| Adapter | Back end | Type | Driver | max_texture_2d | max_buffer_size | max_storage_binding | red / black pixels |
|---|---|---|---|---|---|---|---|
| NVIDIA GeForce RTX 5080 | Vulkan | DiscreteGpu | NVIDIA 617.42 | 32768 | 4294967295 | 2147483644 | 2016 / 2080 |
| AMD Radeon(TM) Graphics | Vulkan | IntegratedGpu | AMD proprietary 24.30.18 | 16384 | 2147483648 | 2147483644 | 2016 / 2080 |
| NVIDIA GeForce RTX 5080 | Dx12 | DiscreteGpu | 32.0.16.1742 | 16384 | 2147483647 | 2147483644 | 2016 / 2080 |
| AMD Radeon(TM) Graphics | Dx12 | IntegratedGpu | 32.0.13018.6 | 16384 | 2147483647 | 2147483644 | 2016 / 2080 |
| Microsoft Basic Render Driver (WARP) | Dx12 | Cpu | 10.0.26100.9711 | 16384 | 2147483647 | 2147483644 | 2016 / 2080 |

All five adapters work, including the WARP software fallback. 2016 = (4096 - 64) / 2: the 64 pixel centres lie exactly on the hypotenuse.
The top-left fill rule excludes them because the hypotenuse is a right edge. All five adapters agree, so the CPU reference below uses the
top-left rule. The spike is kept as `examples/adapters.rs`, which prints the same table and writes a cavity PNG per adapter.

## Dependency decision

* **wgpu =30.0.1** (published 2026-08-22, 65 days before this ADR; MIT OR Apache-2.0; declared rust-version 1.87). Features:
  `default-features = false`, `std`, `wgsl`, `dx12`, `vulkan`. GLES, Metal and WebGPU are off; Metal is an additive feature for macOS
  later. Reason: the user's renderer decision (plan section 1). Nothing in the workspace already renders on a GPU.
* **No async runtime.** wgpu's futures are driven by a 15-line `block_on` written on `std::task::Wake` (no `pollster`).
* **Pairing with eframe for wave 48** (crates.io dependency data, read 2026-10-26). eframe's `wgpu` feature is
  `["wgpu_no_default_features", "egui-wgpu/default"]`, and `wgpu_no_default_features` enables `dep:wgpu`, `dep:egui-wgpu` and
  `dep:pollster`.

| eframe / egui-wgpu | wgpu requirement | Published | rust-version |
|---|---|---|---|
| 0.27.2 | ^0.19.1 | 2024-04-02 | 1.72 |
| 0.28.1 | ^0.20.0 | 2024-07-05 | 1.76 |
| 0.29.1 / 0.30.0 / 0.31.1 / 0.32.3 | ^22.1 / ^23 / ^24 / ^25 | 2024-10-01 to 2025-09-12 | 1.76 to 1.85 |
| 0.33.3 | ^27.0.1 | 2025-12-11 | 1.88 |
| 0.34.3 / 0.35.0 | ^29.0 | 2026-05-27 / 2026-06-25 | 1.92 |
| **0.36.2** | **^30.0** | 2026-09-08 | 1.95 |
| **0.37.0** | **^30.0** | 2026-10-15 | 1.95 |

  eframe 0.27 and 0.28 pin wgpu 0.19 and 0.20. They cannot share a device or types with wgpu 30, so they are not usable with this
  crate. The pairing is **wgpu 30.0.x with eframe 0.36.2**, which passes the two-week rule today, or with 0.37.0, which passes it on
  2026-10-29. Both resolve to the same wgpu 30. eframe is not added here.
* **MSRV.** The crate declares `rust-version = "1.87"` (wgpu's). The workspace stays at 1.75. Precedent: ADR 0004 gave `rcfd-gui` and
  `rcfd-assistant` 1.88. Nothing enforces 1.75: CI uses stable, and `docs/audit/partials-group-2.md` notes that 1.75 is not tested. eframe
  0.36 needs 1.95, so the Studio track will need 1.95 in wave 48. **That is for the lead or the user to decide**, not this ADR.
* **Licences.** wgpu 30.0.1's normal and build dependency closure for `x86_64-pc-windows-msvc` has 69 crates (`cargo metadata`). All are
  MIT, Apache-2.0, BSD or Zlib, alone or as one choice of a dual licence, except `libloading` 0.8.9 (ISC, Vulkan loader).
  * ISC is permissive, but it is not on the preamble's list; the lead should note it.
  * `libloading` 0.8.9 is already in `Cargo.lock` on main through another dependency, so this ADR does not introduce it.
* **An older wgpu is already locked.** The frozen `rcfd-gui` uses eframe 0.27.2 (glow), whose lock entries include wgpu 0.19.4, which is
  not built. It cannot share types with wgpu 30. This does not matter while `rcfd-gui` is frozen, but the Studio must use eframe 0.36
  or later.
* **Lockfile.** 37 new packages, about 20 of them Windows-only (`windows*`, `gpu-allocator`, `ash` for Vulkan). No version of an existing
  locked package moved (checked with `git diff Cargo.lock`: no removed `version` line). The rebase re-checks this.
* **Build and disk cost** (measured, not registered). One clean release build of `rcfd-render` with `-j 3 --timings` in an empty target
  directory, while other agents were also building:
  * 2 min 03 s wall clock for 145 units, 368 s of summed unit time;
  * the 62 crates reached only through wgpu account for 202 s of that (wgpu-core 50 s, naga 48 s, wgpu-hal 24 s, windows 14 s, ash 13 s,
    wgpu 13 s);
  * `rcfd-render` itself takes 2.8 s;
  * the release target directory is 554 MB, of which about 309 MB are the wgpu, naga, windows, gpu-allocator and ash artefacts.
  * The debug build after the workspace was already built took 63 s.

## Design (v0)

* **`SurfaceMesh`**: polygon faces in CSR form, named parts, and for each face the volume-mesh face id and owner cell.
  * `from_mesh(mesh, names)` takes patches, or face zones when no patch has that name; an empty list takes every patch. Boundary faces
    come from `Mesh::patches()` ranges, the rule of `rcfd-post` `gltf.rs`.
  * `face_values_from_cells` gives each face its owner cell's value, also the glTF rule.
  * Faces are fan-triangulated from their first point. A fan is only correct for convex faces; this limit is marked in the code.
* **`Field`**: `Face(&[f64])` (flat colour per face) or `Vertex(&[f64])` (one value per point, interpolated perspective-correctly over
  each triangle, then colour mapped; colours are not interpolated). Values must be finite and match the count.
  * The range is the view's (finite, max > min) or the field's min and max. A zero span maps to 1, as in `gltf.rs`.
* **Colour maps**: `ColorMap` holds 2 to 16 evenly spaced stops. `ColorMap::viridis()` reads the 9 integer stops of
  `rcfd_post::render::colormap` by sampling it at k/8, which is exact. The shader runs the same piecewise-linear formula and outputs
  `round(c) / 255` into `Rgba8Unorm`, which stores exactly `round(c)`.
  * Not sRGB. A wave-48 swap chain must keep a linear format or convert, otherwise colours shift.
* **Camera** (`camera.rs`, pure f64 and unit-tested): an orbit camera with yaw and pitch about a target, +y up, perspective or
  orthographic, wgpu clip space (depth 0..1), and a closed-form basis with no singularity at pitch +-90 degrees.
  * `project` maps a point to pixel coordinates (origin top-left, y down) plus depth.
  * `unproject` is the closed-form inverse of each matrix. It is deliberately not a numeric inverse of `view_proj`, so the round trip
    checks two independent derivations.
  * `fit` frames a bounding sphere.
* **Shading**: `Unlit` (exact map colours), `Flat` (face normals) or `Smooth` (point normals: the normalised sum of the surrounding face
  area vectors).
  * The lit modes use a two-sided headlight: `c (0.25 + 0.75 |n . l|)`, with `l` the camera back vector.
* **Depth**: `Depth32Float`, compare `Less`, cleared to 1. No back-face culling (cut-away views show inner faces). No MSAA, so every
  pixel is a deterministic function of the scene on a given adapter.
* **Parts**: per part `Field`, `Solid(rgb)` or `Hidden`, held in a storage buffer and read in the fragment shader. A hidden part is
  discarded, so it is neither drawn nor pickable.
* **Cut plane: clipped in the fragment shader** (`discard` where `n . p > d`), not CPU slicing. It is exact per pixel centre and portable
  (wgpu's `CLIP_DISTANCES` is an optional feature).
  * It cuts the boundary surface away; it does not cap the volume. A filled section of the volume field is the CPU slice that already
    exists (`rcfd_post::surfaces::cut_plane`), fed in as a `SurfaceMesh`; that is wave-49 work.
* **Picking**: a colour-ID pass with the same pipeline state and two targets:
  * `R32Uint` holding the surface face + 1 (0 = background);
  * `R32Float` holding the interpolated field value.

  `PickImage::at(x, y)` returns `(face, value)`. `SurfaceMesh::mesh_face` and `SurfaceMesh::cell` map the face to the volume-mesh face
  and cell.
* **Vertex layout** (one buffer, 52 bytes per expanded triangle vertex): position f32x3 @0, flat normal f32x3 @12, smooth normal f32x3 @24,
  scalar f32 @36, face u32 @40, part u32 @44, surface point u32 @48.
  * **The deformation hook is the point id at @48.** Displacement will be a per-point storage buffer read as `disp[point]` in the
    vertex shader, so a new time step uploads 12 bytes per point and never rebuilds this buffer. Smooth normals will then need
    recomputing, which is not designed here.
  * A field change is a new upload (v0).
* **Offscreen output**: `Renderer::render` returns an `rcfd_post::render::Image` (RGBA). The PNG comes from its existing `to_png`.
  Readback pads rows to 256 bytes and strips the padding. Sizes outside 1..=`max_texture_dimension_2d` are a located error.
* **Adapter choice**: `Renderer::new()` takes a discrete GPU, else an integrated one, else any adapter, over DX12 and Vulkan (WARP
  included). `Renderer::from_adapter` uses a named adapter. No adapter is a located error that contains "no adapter".

## What I already knew when registering (rule c, rule d; disclosed)

* The crate, its tests and the CPU reference were written before this registration, so that rule d could be applied. Everything is
  committed after this ADR, unchanged except as stated in amendments.
* Seen before registration:
  * the step-0 spike above, whose triangle is R0's scene, so R0's result was known;
  * a smoke test that is not committed:
    * construction only: the cube surface has parts `xmin, xmax, ymin, ymax, zmin, zmax`, 96 faces and 98 points; the 8 x 8 cavity has
      `lid, walls, frontAndBack` and 160 faces; the T1 surface has exactly 1 000 000 triangles and took 10.1 s to build in debug;
      `Backends::empty()` gives 0 adapters;
    * one *different* scene (a tilted quad with a per-point field, smooth shading and a cut plane, at 90 x 70) rendered and picked on all
      five adapters without error, with 808 pixels drawn and 808 picked on each;
  * `examples/adapters.rs`, whose 640 x 480 perspective cavity image I looked at; it is not a registered scene.
* G3's negative control was computed on the CPU before registration (oracle against oracle, no GPU output). At the 256 pixel-centre
  values of t, the 6-stop map of `rcfd-gui` differs from the 9-stop map of `rcfd-post` by more than 1 in 97.3 % of the columns.
* No registered check (C1-C3, G1-G3, P1-P2, V1, K1, W1, S1, T1, D1, U1) had run when this ADR was committed.

## Registered checks

All GPU checks are in `crates/rcfd-render/tests/render.rs`. Each runs on **every** DX12 and Vulkan adapter that `adapters()` returns,
WARP included. With no adapter, each test prints `SKIP <file>:<line>: no wgpu adapter ...` and passes. The camera checks are unit tests in
`src/camera.rs`. Pixel differences are per channel, in 8-bit steps.

| Id | Check | Limit | Reason for the limit | Negative control (must fail) |
|---|---|---|---|---|
| R0 | Spike triangle through the library: orthographic camera over [-1,1]^2, 64 x 64, solid red | red = 2016 and black = 2080 exactly | closed form with the top-left rule | the inclusive-diagonal count 2080 is a different number |
| C1 | Projection of known points (perspective and orthographic, 800 x 600) against closed forms; target at centre; near / far depth; behind-eye point | pixel error <= 1e-9 px; depth error <= 1e-12; behind-eye point gives `None` | f64 rounding of a few operations on values of order 1e3 is about 1e-13 | (closed forms) |
| C2 | Orbit invariants over 200 orbit steps, a full yaw turn, and pitch = 90 degrees | distance relative 1e-12; target at centre 1e-9 px; basis orthonormal 1e-12; right vector horizontal 1e-15; full turn 1e-12; eye straight above 1e-12 | f64 rounding | (invariants) |
| C3 | `unproject(project(p))` for 1000 points in a cube of side 3 around the target; 4 orientations (one at pitch 1.5707963 rad) x 2 projections | relative error <= 1e-6 (the task's figure) | f64; the two directions are derived independently | unprojecting with width and height swapped must exceed 1e-3 |
| G1 | 16 x 16 cavity, all patches, per-face field from random cell values in [0,1], range [0,1], orthographic front view, 272 x 272, against `rcfd_post::render::render_scalar` (`colorbar: false`, width 272) | max channel difference <= 1 over all pixels | margin 8 and 16 px per cell put every cell edge on a pixel boundary, so coverage is identical; colours differ by at most one rounding (f32 against f64) | rcfd-post's image of the x-mirrored field: >= 50 % of the 65 536 drawn pixels differ by > 1 |
| G2 | 4 x 4 x 4 cube boundary (96 faces), perspective (fovy 0.8, yaw 35 degrees, pitch 25 degrees), 320 x 240, against the CPU reference rasteriser; scenes: (a) face field unlit, (b) face field flat, (c) point field smooth, (d) point field unlit with `xmax` solid red and `ymax` hidden | on interior pixels (the 3 x 3 neighbourhood in the reference id buffer is uniform): max <= 1 unlit, <= 2 lit; edge pixels above that limit <= 1 % of edge pixels; mean absolute difference over all pixels <= 0.25 | f32 vertex positions move edges by about 1e-4 px, so only centres that close to an edge can change coverage (edge pixels); lighting adds one f32 product before rounding | the reference at yaw 37 degrees must exceed the interior limit |
| D1 | Determinism: each G2 scene rendered twice on the same adapter | byte-identical | no MSAA, no blending, a fixed scene | (none) |
| G3 | Scalar ramp: unit square, point field = x, range [0,1], orthographic 256 x 256, unlit; rows 64, 128 and 192, all 256 columns, against `rcfd_post::render::colormap((x + 0.5) / 256)`. Strip: 8 faces with values k/7, centre pixels against `colormap(k/7)` | max channel difference <= 1 | one rounding, f32 against f64 | the `rcfd-gui` 6-stop map as the expectation: >= 384 of the 768 ramp pixels differ by > 1 |
| P1 | Picking on the 8 x 8 cavity (front view, 256 x 256), one probe at each cell centre | 64 / 64 correct (face centroid at the cell centre on the front plane z = 1/8, `SurfaceMesh::cell` = mesh owner = expected cell, value = f32(cell value) exactly); background gives `None` | ids are integers, and the value is the f32 of the face value | the id read as a triangle index: <= 32 / 64 correct |
| P2 | Depth and visibility by picking on the cube; per-point values | front view, centre pixel: part `zmax`; with `zmax` hidden: part `zmin`. Smooth G2 scene: interior pick ids equal the reference ids (0 mismatches); interior values within 1e-5 of the field range | depth test `Less`; f32 interpolation is about 1e-7 relative | (the `zmax` / `zmin` pair is its own control) |
| V1 | Part visibility and solid colour, 8 x 8 cavity front view, 256 x 256 | `frontAndBack` hidden: 0 drawn pixels; solid red: 65 536 pixels exactly (255, 0, 0, 255) | the walls and lid are edge-on, and the square fills the image | visible must draw 65 536 pixels |
| K1 | Cut plane x <= 100/256 on the unit square, orthographic 256 x 256, colour and pick pass | 0 mismatches: columns 0-99 drawn and picked, 100-255 neither, in every row | the plane is half a pixel from the nearest centres; f32 position error is about 1e-7 | the boundary expected at column 101 must give mismatches |
| W1 | PNG round trip (100 x 37, a width whose rows need padding) and input validation | the decoded PNG equals the pixels; sizes 0 x 10, 10 x 0 and 100000 x 10, range (1, 1), a NaN value and a wrong field length are all errors | API boundary | (the error cases) |
| S1 | No adapter (`Backends::empty()`) | located error containing "no adapter" | skip path | (none) |
| T1 | Frame time: 500 x 500 x 1 box, `zmin` + `zmax` = 1 000 000 triangles, per-face field, flat shading, 1920 x 1080; median of 5 renders after one warm-up, each including uniforms, draw and full image readback; upload time reported | **reported on every adapter** with its name; asserted only on `DiscreteGpu` adapters: median <= 250 ms | generous: drawing 1 M triangles takes a few ms on a current discrete GPU; each call also allocates textures and buffers and reads back 8.3 MB | (report) |
| U1 | `ColorMap::viridis().eval(t)` equals `rcfd_post::render::colormap(t)` at 1001 points | exact | same formula, exact stops | (none) |

Cross-backend differences (for example the same scene on NVIDIA DX12 against WARP) are reported in the addendum. They are not limited
beyond each adapter passing every check against the shared references.

### Status decision rule (registered now)

A check passes only if every adapter passes it. If a check misses on some adapters only, the test is split: the passing adapters keep a
passing test, and the missing adapters get an ignored `*_known_miss` test with the same limit. A negative control that does not fail
makes its check **non-discriminating**; the result is reported and the limit is not changed. On another machine the T1 limit applies to
discrete GPUs only, and the frame time is always printed with the adapter name.

## Out of scope

Windowing, swap chains and egui integration (wave 48); iso-surfaces, streamlines, vectors, particles, animation and video (wave 49);
deformation (only its buffer hook, above); MSAA and anti-aliased lines; legends and text (`rcfd-post` `draw_text` can be used on the
image); volume-capped cut planes; Metal, GLES and WebGPU back ends.

## Files

`crates/rcfd-render/` (`src/lib.rs`, `src/camera.rs`, `src/surface.rs`, `src/gpu.rs`, `src/shader.wgsl`, `tests/render.rs`,
`tests/cpu_ref/mod.rs`, `examples/adapters.rs`); workspace member in the root `Cargo.toml`; `Cargo.lock`.
