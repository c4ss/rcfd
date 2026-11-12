# ADR 0350: An Electron Studio over the rcfd APIs: a performance spike, and the conditions for retiring rcfd-studio

Status: PROPOSED 2026-11-12. Asked for by the maintainer on 2026-11-12 ("write an ADR to test the performance of
electron<>rcfd APIs and potentially retiring rcfd-studio"), after the lead recommended a web-technology shell over the Rust
engine for designer freedom. This ADR registers a measurement spike with its limits fixed before any code, and a decision rule.
It decides nothing about retiring `rcfd-studio` by itself: retirement is a later, separate decision of the maintainer, taken
only on the evidence and the parity checks named here.

## 1. Context

- The Studio plan (`docs/ui-restart-plan.md`, decisions of 2026-10-26) built `rcfd-studio` as egui on eframe's wgpu back end
  (layer L5), over layers that need no window: L1 project format (`rcfd-project`), L2 run service (`rcfd-runner`), L3
  time-series store (`rcfd-tsstore`), L4 render core (`rcfd-render`), L6 Claude Code bridge (`rcfd-claude`), L7 plugin
  (`rcfd-mcp`).
- The goal stated on 2026-11-12 is an interface that is easy to learn and hard to master, where designers can build a
  technically complex, visually simple interface. egui fixes look, layout and motion in Rust code; it has no style sheets, a
  shallow layout model, hand-made animation, and weaker text, input-method and accessibility support than a browser engine.
  A browser shell (TypeScript, a component library, design tokens) gives designers their own medium.
- The engine already has a client boundary: the JSON-RPC service (`docs/service-protocol.md`, `rcfd-service/1`, protocol
  1.1; newline-delimited JSON-RPC 2.0 over TCP or stdio, one thread per connection, long-poll `events.poll`), used by the MCP
  server and, since ADR 0345, by out-of-process co-simulation participants. An Electron Studio would be one more client.
- What that boundary does not yet carry well: bulk field data. The only field routes today are `field.slice`, `field.grid`,
  `field.stats`, `render.png` / `render.glb` (base64 inside JSON) and `field.export_vtk` (files). The store `store.rts` is
  seek-based with optionally deflated blocks (ADR 0124), read in process by `rcfd-studio`.
- The open performance target of the plan (§5 limit 6): interactive rendering of meshes of millions of cells has never been
  measured, in either shell.

The question this ADR answers with numbers: can an Electron process, talking to the Rust engine only through interfaces that
exist or that this ADR registers, control runs, follow them live, scrub fields through time and draw them in 3D at least as
well as `rcfd-studio` does, on the maintainer's machine?

## 2. What is measured (the spike)

A throw-away benchmark, not a product: `bench/electron/` (outside the Cargo workspace, own `package.json` with exact
versions and a lock file), driving the existing `rcfd serve`. Node v22 is installed on the maintainer's machine (checked
2026-11-12). Electron's Chromium supplies WebGPU on Windows.

Routes for bulk field data, each measured (the spike does not choose one in advance):

- **R0 (baseline, expected to miss):** existing JSON-RPC, a field as a JSON number array or base64 in a reply.
- **R1, binary side channel:** a new opt-in service method family `data.*` (protocol 1.2, no existing method changed) that
  returns a ticket; the bytes come on a second TCP connection (or a Windows named pipe) as length-prefixed frames of raw
  little-endian `f32` / `u32`. Rust side in `rcfd-api` with `std::net` only.
- **R2, file read in Node:** the Electron main process reads `store.rts` itself (a TypeScript reader of the ADR 0124 format;
  deflated blocks through Node's built-in `zlib`), and passes `ArrayBuffer`s to the renderer as transferables.
- **R3, WebAssembly reader:** the `rcfd-tsstore` reader compiled to `wasm32-unknown-unknown` and run in the renderer or a
  worker. Only if it builds without a new crate (it may not: the target may need `wasm-bindgen`, which would be a new
  dependency and needs the maintainer; then R3 is recorded as not run).
- A native Node addon (for example `napi-rs`) is **not** measured: it is a new crate, and house rule 5 forbids it without
  the maintainer's approval (question Q3).

Data sets (made by existing code, fixed now): a box of hexahedra of N = 1e5, 1e6 and 4e6 cells with 20 saved snapshots of
one scalar and one vector field, written by the existing run path to a `store.rts`, deflated and raw; the boundary surface of
the 1e6 case (about 6e4 faces) and an iso-surface-sized triangle soup of 1e6 triangles for the draw tests.

## 3. Checks and limits (fixed before any code)

Times are wall clock on the maintainer's machine (Windows 11, the GPU it has), loopback only, median and 99th percentile of
the stated number of repeats, with the solver idle unless stated. The control column C is the same measurement in
`rcfd-studio` where one exists; it is measured first, by a test in the Studio's own style, release profile.

| Id | What | Limit | Estimate behind the limit |
|---|---|---|---|
| P1 | Round trip of `service.info` and `run.status` from Node, 1000 calls | p50 <= 0.5 ms, p99 <= 2 ms | ADR 0345 T1 measured 0.048 ms per call after its transport fix |
| P2 | Latency from the solver's `iteration` event to the renderer process, `events.poll` long-poll, solver running | p99 <= 20 ms | one long-poll wake plus two IPC hops, each well under 5 ms |
| P3 | `monitors.get` of 1e5 samples in 4 series, parsed in the renderer | <= 100 ms | ~6 MB of JSON; V8 parses JSON at 100 to 300 MB/s |
| P4 | One snapshot of a 1e6-cell scalar (4 MB of `f32`), from request to a filled GPU buffer in the renderer, per route | R1 or R2: p50 <= 30 ms, p99 <= 60 ms; R0 recorded | loopback TCP or file read at >= 1 GB/s (4 ms), inflate at ~300 MB/s (13 ms), transferable hand-off (zero copy), GPU upload ~1 ms |
| P5 | Scrubbing: 20 consecutive snapshots of the 1e6 case, one per frame request | >= 10 snapshots per second sustained; no frame longer than 150 ms | P4's budget per snapshot |
| P6 | The same at 4e6 cells | >= 3 snapshots per second | 4x P4 |
| P7 | First picture of the 1e6-cell surface after opening the run (mesh and one field) | <= 1.5 s | mesh bytes ~50 MB raw; boundary extraction on the Rust side or in the reader |
| P8 | Draw: 1e6 triangles with a per-vertex scalar and a colour map, orbiting | >= 60 frames per second, p99 frame <= 25 ms | a mid-range GPU draws 1e6 triangles far under 16 ms; WebGPU overhead is small at one draw call |
| P9 | Memory of the renderer process holding 20 snapshots of the 1e6 case | <= 2.5 x the raw data (20 x 4 MB) + 300 MB | one copy on the GPU, one in JS, the Chromium base |
| P10 | Cold start of the packaged spike to a project view listing runs | <= 2.5 s | Electron starts in 0.5 to 1.5 s on Windows |
| P11 | A Claude Code transcript of 1e4 entries (existing `rcfd-claude` reader, served by a new opt-in `chat.*` method or read by Node from the transcript files), scrolled in a virtualised list | p99 frame <= 20 ms | virtualised lists render tens of rows |
| P12 | Equality: every field value the renderer receives through R1 or R2 equals the Rust reader's value bit for bit, for all 20 snapshots | 0 differing bits | same bytes, little-endian on both sides |

Negative controls:

- **NC1:** R0 at 1e6 cells is expected to miss P4 (more than 60 ms: JSON of 1e6 numbers is ~10 MB and its parse alone is
  tens of milliseconds). If R0 meets P4, the measurement of P4 is suspect and is re-examined before any decision.
- **NC2:** P12 with one block's bytes deliberately shifted by one in the test harness must report the difference.

Fairness: the C column uses the same data sets, machine and window size. Where `rcfd-studio` has no equivalent (P1 to P3 are
in-process calls there), C is recorded as "in process" and the Electron number is judged against its limit only.

## 4. Decision rule

- **Electron viable:** P1 to P5, P7, P8, P10 and P12 met by at least one bulk route (R1 or R2) that needs no new Rust crate.
  P6, P9 and P11 are recorded; a miss there is listed with its cause and does not block.
- **Viable with conditions:** as above but only through a route that needs a new crate (R3 with `wasm-bindgen`, or a native
  addon), or with P5 met only with a level-of-detail reducer on the Rust side. Then the condition goes to the maintainer.
- **Not viable now:** P4, P5 or P8 missed by every route, with the cause measured. `rcfd-studio` stays the Studio; the
  numbers are kept for a later look.

A viable result does not retire anything. It allows the maintainer to start phase 2 (section 5).

## 5. Retiring rcfd-studio (only after the spike, only on the maintainer's word)

Phase 2, an Electron Studio reaching parity, would:

1. Freeze `rcfd-studio` the way `rcfd-gui` was frozen: kept building and tested, no new features.
2. Build the shell in TypeScript over the service, with the bulk route the spike chose, and pass the parity checks of
   `docs/ui-restart-plan.md` §6 plus the Studio checks added since (project browser, run list and state, background runs that
   survive the app, time scrubbing with linked plots, case editor with schema validation, quick run, stop, live preview,
   Claude Code chat, field view with colour map, legend and hover). Each check becomes an automated test of the new shell
   (Playwright drives Electron) where a check can be automated, and a view-and-report step by the maintainer where it cannot.
3. Retire `rcfd-studio` (and with it the frozen `rcfd-gui`) only when every parity check passes and the maintainer has used
   the new shell and said so. Removal means deleting the crates and their tests and the two layer lines in
   `crates/rcfd-cli/tests/workspace_graph.rs` (an edit of an existing test, so it needs the maintainer, as ADR 0270 M1 did).

What stays in Rust whatever happens: `rcfd-project`, `rcfd-runner`, `rcfd-tsstore`, `rcfd-claude`, `rcfd-mcp`, `rcfd-api`
and `rcfd-render` (its offscreen rendering serves `render.png` / `render.glb` and the MCP tools; its colour maps and
iso-surface and streamline compute are references the WebGPU viewport must match, value for value, by the same "a picture
can be wrong and look right" rule of the plan, §5 limit 2).

## 6. Limits

- Agents cannot judge whether the shell is easier to learn or more pleasant; this spike measures only whether the boundary is
  fast enough and exact. The design benefit is the maintainer's judgement.
- One machine, one GPU, Windows only. Linux (and macOS) are not measured; Electron ships its own Chromium, so WebGPU there
  depends on the GPU driver, not on a system web view. Tauri is not measured (its Linux web view lacks dependable WebGPU).
- The spike measures a box mesh; a fan mesh with many patches and polyhedral cells may change P7.
- The gate (`scripts/gate_test.py`) does not run npm tests; the spike's numbers are run by hand once, like heavy records.
- A second language and toolchain (Node, npm, Electron's release cadence and security updates) is a standing cost that no
  number here captures.

## 7. Questions for the maintainer (each with options and a recommendation)

- **Q1, run the spike?** (a) yes, as registered; (b) yes, without R3; (c) no. Recommendation: (a); R3 is recorded as not run if
  it needs a new crate.
- **Q2, where the JavaScript lives.** (a) `bench/electron/` now, `apps/studio-web/` for phase 2, both outside the Cargo
  workspace with exact versions and lock files; (b) a separate repository. Recommendation: (a), so the service protocol and
  its client change in one commit.
- **Q3, a native Node addon (`napi-rs`).** (a) not now: measure the routes without new crates first; (b) allow it in the spike.
  Recommendation: (a); ask again only if R1 and R2 both miss P4.
- **Q4, the `data.*` methods (R1).** A new opt-in method family in protocol 1.2; existing methods and goldens unchanged, but
  the MCP tool list is not touched only if no MCP tool is added (none is planned). Recommendation: add them on the spike's
  branch, merged only if the spike is viable.
- **Q5, Electron or Tauri.** (a) Electron (one Chromium everywhere, WebGPU dependable); (b) Tauri (smaller, Rust-native, system
  web view). Recommendation: (a) for a viewport-heavy application; revisit if Linux web views gain WebGPU.

## 8. Roadmap

Three items in the category "Studio desktop app" (added by the lead on 2026-11-12): F-447 the spike (this ADR), F-448 an
Electron Studio to parity (depends on F-447 and the maintainer's go-ahead), F-449 retiring `rcfd-studio` and `rcfd-gui`
(depends on F-448; needs the maintainer).

## Decision record (2026-11-12)

- **Q1:** (a), run the spike, asked for by the maintainer in person on 2026-11-12 ("can we do that now?").
- **Q2** (a), **Q3** (a), **Q4** (the `data.*` methods on the spike's branch only) and **Q5** (a, Electron): taken by the lead on
  the maintainer's standing instruction of 2026-11-11 (take the recommendation).

## Amendment 1: protocol (2026-11-12, written before any code of F-447 and before any measurement)

No limit of section 3 changes. This amendment fixes how each number is taken. Read beforehand (disclosed):
`docs/service-protocol.md`, `crates/rcfd-api/src/service.rs`, `fieldstore.rs`, `crates/rcfd-tsstore/src/lib.rs`,
`crates/rcfd-io/src/container.rs` (mesh frame layout), `crates/rcfd-studio/tests/vp/mod.rs`, `viewport_store.rs`,
`frame_cost_b7.rs`, ADR 0345 (how `cosim.*` became protocol 1.1). No timing was taken beforehand. Machine facts read
beforehand: Ryzen 7 9800X3D (8 cores), 93.6 GB RAM, GeForce RTX 5080 on a 1920 x 1080 display at 239 Hz and a Radeon integrated
adapter on a 60 Hz display, Node 22.19.0, npm 11.12.0; the processor showed 100 percent load from other building agents at the
time of reading, so the load is noted with every timing run.

### A1.1 Rust side

- **`data.*`, identifier `rcfd-data/1`, protocol 1.2, opt-in per service instance.** An existing test asserts
  `protocol_version == "1.1"` and 69 methods for a default service (`cosim_service_f360.rs`), and house rule 3 forbids its
  edit. So the family is switched on by the embedding program (`Service::enable_data`); a service that does not call it
  answers `service.info` with exactly the bytes of today and `data.*` with -32601. A service with it on reports
  `protocol_version` `"1.2"`, the member `data` (`"rcfd-data/1"`) and the `data.*` names after the existing ones in `methods`.
  `rcfd serve` does not switch it on (a flag would change the CLI help golden); that step is left for the maintainer.
- Methods: `data.info` (`{protocol, port, unix_ns}`: the data port and the service's clock, for P2), `data.open {path}`
  (snapshots and fields of the store in the sandboxed folder `path`), `data.field {path, snapshot, field}` and
  `data.surface {path}` (the boundary faces of the stored mesh, fan-triangulated: points `f32`, triangle corners `u32`, the
  owner cell of each triangle `u32`). The last two return `{ticket, sections: [{name, dtype, count}]}`. The bytes are prepared
  at the call from `rcfd_tsstore::Reader` (`read_field` cast to `f32`; `mesh`) and wait under the ticket (at most 8 tickets;
  the oldest is dropped). The client holds one second TCP connection to `port` (loopback, `TCP_NODELAY`), writes the ticket
  as 8 bytes little-endian and reads, per section, `u64` little-endian byte length then the raw little-endian values. A
  ticket is used once. No cache of field or surface bytes on the Rust side; a store's `Reader` is opened per call.
- The spike's server is an example of `rcfd-api` (`examples/f447_bench.rs serve`): `Service::new`, `enable_data`,
  `serve_tcp`, the same calls `rcfd serve` makes. For P2 it also runs one thread that long-polls `events.poll` in process
  and writes `(seq, unix_ns)` of every event to a file: the event log carries no time stamp, and adding one would change an
  existing method. That thread wakes on the same condition variable as every poller, so its stamp is later than the emit by
  microseconds, which shortens the measured latency by that much (accepted against a 20 ms limit, stated in the results).
  Deviation from section 2 ("driving the existing `rcfd serve`"): for the two reasons above.
- R3 is checked first by `rustup target list --installed` and by whether the reader could export functions: recorded as not
  run if the target is not installed or an export needs `wasm-bindgen` or `unsafe` (`#[no_mangle]`, raw pointers).

### A1.2 Data sets (generator `examples/f447_bench.rs gen`, output in the scratch folder, not committed)

- Boxes `box_3d` with unit lengths: 50 x 50 x 40 (1e5 cells), 100 x 100 x 100 (1e6), 200 x 200 x 100 (4e6). Lid-driven:
  `ymax` moves with (1, 0, 0), the other five patches are fixed walls, `nu` 0.01, `simplec`, `upwind`, steady,
  `output.store_interval = 1`, `output.store_fields = ["p", "ux", "uy", "uz"]` (one scalar, one vector as its three
  components: the store holds scalars only), `output.store_precision = "f32"` (so a 1e6-cell scalar is 4 MB, as P4 states),
  20 steps through `runctl::drive` (the existing run path), which gives 20 snapshots (steps 1 to 20). If a step yields a
  non-finite field the run is not repaired; it is reported.
- The run path writes raw blocks only (`WriteOptions::default()`). The deflated store of each size is a copy written by the
  generator with `rcfd_tsstore::Writer` and `Codec::ShuffleDeflate` from the values `Reader::read_field` returns, same block
  size (65536), same steps and times.
- Reference for P12: per store size, snapshot and field, the bytes of `Reader::read_field(..)` cast to `f32`, little-endian,
  written by the generator to `ref/<size>/<snapshot>-<field>.f32`, from the raw store; the generator also asserts that the
  deflated store gives the same bits.
- Surface of the 1e6 case: its 6e4 boundary quadrilaterals (1.2e5 triangles). Triangle soup: the sheet
  z = 0.25 sin(3 pi x) cos(3 pi y) over the unit square, 1000 x 500 quadrilaterals each cut in two, 1e6 triangles with three
  own corners each (3e6 vertices), per-vertex scalar z; made by the same formula in the bench (JavaScript) and in the
  Studio control (Rust). Deviation from "made by existing code": the soup is a formula, not an iso-surface of a run.
- A project folder around the stores (`runs/<id>/fields/store.rts`) for P10 and the control.
- P11 transcript: 1e4 lines of JSON in the Claude Code transcript form (alternating user and assistant entries, text of 40 to
  1200 characters from a fixed linear congruential generator), written by the bench, read by Node from the file.

### A1.3 Electron side

- `bench/electron/`: Electron 45.1.0 exactly (published 2026-10-27, 16 days before today; 45.2.x and 45.3.x are younger
  than two weeks), no other package, lock file committed, plain JavaScript (no compile step; deviation from "a TypeScript
  reader": the same code a TypeScript compiler would emit, with one dependency less). No UI framework.
- Arrangement, the one a product would use and the only one judged: the main process does all I/O (the two TCP
  connections, file reads, `zlib.inflateRawSync`); the renderer is sandboxed and context-isolated and talks to the main
  process over one `MessagePort` (handed over by the preload script), so a field crosses one structured-clone hop. Electron
  cannot transfer an `ArrayBuffer` from the main process without a copy; the copy is part of every time below.
- R1 as in A1.1. R2: a reader of the ADR 0124 format in the main process (footer index, snapshot directory, frame-head
  checksums and the payload checksum of every DATA block verified with FNV-1a 64 as the Rust reader does, raw and
  shuffle-deflate blocks, `f32` and `f64` precision), returning `Float32Array` bytes; for P7 it reads the mesh frame's
  metadata, the points and the boundary part of the face arrays by seeks (the mesh frame's payload checksum, 195 MB, is
  not verified in R2: stated as a limit). R0: `field.slice {name: "p", max_points: n_cells}` on a live session of the
  1e6 box after `run.initialize` and one `run.step`, the `values` array turned into a `Float32Array`.
- Window: one visible window, 1280 x 800 content, on the primary display; WebGPU with `powerPreference:
  "high-performance"`; the adapter name is recorded. If no window or adapter can be had, the check is recorded as not run.
- Clock: `performance.now()` in the renderer for intervals. Percentiles: p50 the median, p99 the value at rank
  ceil(0.99 n) of the sorted sample. A check with one limit and no percentile is judged on the median unless stated; the
  maximum is reported too.

### A1.4 How each check is taken (each registered run is made once; warm-up counts are fixed here)

- **P1:** in the main process, one persistent connection with `setNoDelay`; 50 warm-up calls, then 1000 `service.info` and
  1000 `run.status` (a ready session of the 1e5 box), each timed from write to parsed reply. Both must meet both limits.
- **P2:** a session of the 1e5 box, `run.start`; the renderer long-polls `events.poll {after, timeout_ms: 1000}` through
  the port; for each `iteration` event the latency is the renderer's receive time minus the server's stamp of that `seq`.
  The two clocks are joined by 200 `data.info` round trips from the renderer (offset from the round trip with the smallest
  duration, error at most half of it, reported). Sample: the first 300 iteration events or what 120 s give (at least 50,
  else not run).
- **P3:** a session of a 4 x 4 x 4 box stepped 1e5 times (`run.step`, steady; if the run stops early the count is reported
  and the check is judged only if 1e5 samples exist); `monitors.get {names}` of 4 series; the main process passes the reply
  line to the renderer as a string, which parses it; from the renderer's request to the parsed object; 20 repeats, judged
  on the median.
- **P4:** 1e6 box, field `p`, snapshot i mod 20 for repeat i; from the renderer's request to `queue.writeBuffer` into an
  existing vertex buffer followed by `queue.onSubmittedWorkDone()`; 20 warm-up and 200 timed repeats per route and store
  codec (R1 raw, R1 deflated, R2 raw, R2 deflated), R0 20 repeats. A route meets P4 on the raw store (what the run path
  writes); the deflated result is reported with its own met or not met.
- **P5:** 1e6 box, raw store: 10 passes over the 20 snapshots in order; per snapshot: request, write to the GPU buffer
  that colours the 1e6 case's surface, draw, wait for the next animation frame. Rate = 200 / total time; the frame length
  is the time from one snapshot's request to the next one's. Both routes; a pass before as warm-up.
- **P6:** the same at 4e6 cells, 5 passes (100 snapshots).
- **P7:** from the renderer's "open" (`data.open` or the reader's open) through surface and field `p` of snapshot 19 to
  the first animation frame after the draw was submitted and `onSubmittedWorkDone` resolved; a new renderer state each
  time (buffers and pipeline for the surface built inside the timed part; the device exists before); 5 repeats per route,
  judged on the median, the first reported separately.
- **P8:** the soup with a viridis colour map in the fragment shader, depth test, one draw call, the camera orbiting one
  turn per 600 frames; 120 warm-up frames, 1200 timed frame intervals of `requestAnimationFrame`. Taken twice: with vertical
  sync as the window has it (the p99 frame length is judged there, and the display's rate is reported), and with
  `--disable-frame-rate-limit --disable-gpu-vsync` (the frames per second are judged there: on a 60 Hz display the first
  run can only show the display's rate, which may read 59.9). In the second run each frame waits for
  `onSubmittedWorkDone` so that the count is of finished frames.
- **P9:** after loading the 20 snapshots of `p` of the 1e6 box as 20 `Float32Array`s and 20 GPU buffers: the renderer
  process's private bytes from `process.getProcessMemoryInfo()`; the GPU process's working set from `app.getAppMetrics()` is
  reported beside it (not judged: the limit names the renderer process).
- **P10:** packaged by hand (Electron's `dist` copied to the scratch folder, the app in `resources/app`; no packaging
  package). A launcher script takes the time from `spawn` to the renderer's report that the run list (read by the main
  process from the project folder) is in the document and one animation frame has passed. 10 starts, judged on the median;
  the first start and the maximum reported. A true cold start (after a reboot) cannot be taken by an agent: stated.
- **P11:** a hand-written virtualised list (fixed row height, only visible rows in the document); the list scrolled from
  top to bottom in 1200 animation frames, frame intervals as in P8's first run.
- **P12:** for the 1e5 and 1e6 boxes, all 20 snapshots, all four fields, routes R1 and R2, raw and deflated: the renderer
  counts differing bits (XOR and population count over `Uint32Array` views) against the reference files; for the 4e6 box
  field `p` only. **NC2:** the same count after the harness rotates the bytes of block 3 of one received field by one.
- **NC1:** from P4's R0 line.
- **Control C (rcfd-studio, release, measured first):** an added test file `crates/rcfd-studio/tests/f447_control.rs`
  (ignored heavy record, existing code untouched), on the first adapter of `rcfd_render::adapters()` whose name holds
  "RTX" (else the first), offscreen at 1280 x 800: C7 `select_run` to the first prepared image (5 repeats), C4
  `show_snapshot` to the prepared image with the device polled until idle (200 repeats), C5 as P5 (10 passes), C6 as P6,
  C8 `orbit` and `prepare` of the soup shown as a surface (1200 frames). The control has no window, no egui pass and no
  presentation: it is the Studio's data and draw path only, stated wherever C is compared. P1 to P3 and P9 to P12: C is
  "in process" or "none".
- Mutations (scratch copy): M1 `data.field` reads snapshot + 1; M2 the data channel drops the last 4 bytes of a section;
  M3 the JavaScript reader's unshuffle uses the wrong stride. Each must be caught by a Rust test of `data.*` or by P12.

## Addendum 1: results (2026-11-12)

Code: `crates/rcfd-api/src/data_service.rs` (the `data.*` methods), `crates/rcfd-api/examples/f447_bench.rs` (data sets,
server), `crates/rcfd-api/tests/data_service_f447.rs` (D1 to D6), `crates/rcfd-studio/tests/f447_control.rs` (control),
`bench/electron/` (the spike). The registered run was made once (`node run.js`, stages A, B, C, P10), after the control.
Raw results are in the scratch folder (`out/run1/results-A-B-C-P10.json`, `out/control.log`), not committed.

Set-up as found: Electron 45.1.0 (Chromium 156.0.8078.49, Node 24.22.1 inside Electron; the launcher on Node 22.19.0),
WebGPU adapter vendor `nvidia`, architecture `blackwell` (the RTX 5080; Chromium printed that it ignores `powerPreference`
on Windows), window content 1280 x 800, visible. The agent's desktop session showed Electron
one display, 1920 x 1080 at 60 Hz (not the 239 Hz one read beforehand), so every frame-paced number is paced at 60 Hz.
`performance.now()` in the renderer steps by 0.1 ms. Load (processor busy fraction, all cores): 0.45 before the control,
0.27 before stage A and 0.64 during it (the bench's own solver sessions included), 0.61 before B, 0.40 before C, 0.49
before P10; no `cargo`, `rustc` or `link` process was running at the start of any stage.

Data sets: the run path took 20 steps in 4.4 s (1e5), 93.6 s (1e6) and 383.8 s (4e6); 20 snapshots each, no non-finite
value; raw stores 51.6 MB, 514 MB, 2053 MB; deflated copies 43.2 MB, 418 MB, 1701 MB (16 to 19 percent
smaller, mesh frame included); the deflated stores read back bit-equal to the raw ones.

### Checks

Times in ms unless stated. "Met" is against the limit of section 3, judged as amendment 1 fixes.

| Id | Limit | Measured (Electron) | Met | C (rcfd-studio, offscreen) |
|---|---|---|---|---|
| P1 | p50 <= 0.5, p99 <= 2 | `service.info` p50 0.056, p99 0.272, max 0.933; `run.status` p50 0.038, p99 0.132, max 1.111 (1000 each) | yes | in process |
| P2 | p99 <= 20 | 300 `iteration` events in 64 s: p50 0.0, p99 0.80, max 0.92, min -0.85, with a clock join that moved by 1.30 ms between the start and the end of the sample; so p99 <= 2.1 at worst | yes | in process |
| P3 | <= 100 | median 126.6, max 153.1 (20 repeats; 4 series of 1e5 samples; the reply is 12.0 MB) | **no** | in process |
| P4 R1 | p50 <= 30, p99 <= 60 | raw: p50 25.9, p99 57.7, max 59.8; deflated: p50 31.0, p99 66.1, max 75.1 | raw yes; deflated no | C4: p50 12.7, p99 14.4, max 18.4 (to the prepared image) |
| P4 R2 | p50 <= 30, p99 <= 60 | raw: p50 14.2, p99 22.4, max 25.8; deflated: p50 24.0, p99 30.9, max 36.1 | raw yes; deflated yes | as above |
| P4 R0 | recorded | p50 551.7, max 602.8 (20 repeats; a 62.6 MB reply) | recorded | none |
| P5 | >= 10 per s, no frame > 150 | R1: 30.8 per s, longest frame 61.1; R2: 60.1 per s (the display's rate), longest frame 25.0 | yes (both) | C5: 73.0 per s, longest 17.4 |
| P6 | >= 3 per s (recorded) | R1: 10.5 per s, longest 134.0; R2: 17.8 per s, longest 71.0 | yes (both) | C6: 32.0 per s, longest 35.6 |
| P7 | <= 1.5 s | R1: median 1153, first 1580, the five: 1580, 1153, 1099, 1112, 1385; R2: median 55, first 65, max 66 | yes (both, on the median; R1's first opening took 1.58 s) | C7: median 1249, first 1179, max 1635 |
| P8 | >= 60 per s, p99 frame <= 25 | no rate limit: 332 finished frames per s (p50 3.0, p99 4.2, max 9.0); vertical sync: 60.0 per s, p99 frame 16.9, max 17.0 | yes | C8: p50 0.77, p99 1.13, max 9.9 per frame (no presentation, flat colour) |
| P9 | <= 500 MB (recorded) | renderer private bytes 112 MB (109,512 KiB; 24 MB before loading); beside it the GPU process's private bytes went from 182 to 319 MB | yes | none |
| P10 | <= 2.5 s | median 383; first start 1173; the ten: 1173, 385, 6747, 366, 342, 400, 371, 383, 456, 366 | yes on the median; one start took 6.7 s | none |
| P11 | p99 frame <= 20 (recorded) | p99 16.8, max 17.9 (1200 frames, 1e4 entries, 23 rows in the document) | yes | none |
| P12 | 0 differing bits | 0 of 672,000,000 values (720 fields: 1e5 and 1e6 all four fields, 4e6 `p`; R1 and R2; raw and deflated); also the 1e6 surface by R1 and by R2: 0 differing bits | yes | none |
| NC1 | R0 expected > 60 | 551.7 | as expected | |
| NC2 | must report | 1,046,842 differing bits after the shift (0 before) | as expected | |

Rust tests of `data.*` (release): D1 to D6 pass; `cosim_service_f360` (13), `baseline_hashes` (3) and the service's unit
tests (11) pass unedited. `cargo fmt -p rcfd-api --check`, `-p rcfd-studio --check` clean;
`cargo clippy -p rcfd-api -p rcfd-studio --all-targets -- -D warnings` clean.

Mutations: M1 (`data.field` reads snapshot + 1) caught by D3; M2 (the last 4 bytes of a section dropped) caught by D3, D4
and D5; M3 (the JavaScript reader's unshuffle with the wrong stride) caught by the P12 comparison run in Node on the 1e5
deflated store (80 of 80 fields differ; 0 unmutated).

### Diagnosis of the misses

- **P3.** The reply is twice the estimate: each of the 4 series carries its abscissa and its ordinate, 8e5 numbers, 12.0 MB
  (the estimate counted 6 MB). A diagnostic after the run (Node, same server, 20 repeats, not a registered run): the reply
  reaches Node as a string in a median of 116 ms and `JSON.parse` takes 58 ms there; so most of the time is the service
  building and printing the document (`Monitors::to_json` copies every sample into a JSON value before it is printed),
  not the browser. The same diagnostic found that `monitors.get {since}` for the last 100 samples (a 12 kB reply) still
  takes 25 ms, because the whole document is built before `since` is applied.
- **P4 R1 deflated** (p50 31.0, p99 66.1): R1 pays the Rust reader's inflate and its decode to `f64` before the cast to
  `f32`, then the TCP copy. The run path writes raw stores only, so the judged line is R1 raw, which meets with little
  room (p99 57.7 against 60, at a load of 0.64). Not broken down further.
- **P7 R1's first opening** (1.58 s; the median meets): `data.surface` decodes and validates the whole stored mesh
  (`Reader::mesh`), as the Studio does (C7 is the same 1.2 s). R2 reads the points and the boundary part of the face
  arrays only (55 ms).
- **P10's third start** (6.7 s): not diagnosed; it was not repeated (a registered run is made once). The other nine
  starts lie between 342 and 1173 ms.

### Deviations and limits of this run

- P4 writes the field into a storage buffer, not a vertex buffer (the vertex shader looks the value up by owner cell);
  amendment 1 said "vertex buffer".
- P3's session carries one force report so that a fourth series exists (the solver gives three residual series); the
  tolerances are 1e-14 (the schema's lower bound), and the run did not stop before 1e5 steps.
- The frame-paced checks ran on a 60 Hz display; P5's R2 rate is the display's. The window was shown without taking the
  focus, with Chromium's occlusion tracking off so that a covering window cannot stop its frames.
- R2 reads finished stores only (it needs the footer; no scan of a live store) and does not verify the checksum of the
  mesh frame's payload; it does verify every header, frame head, directory and data block it reads.
- The control has no window, no egui pass and no presentation, uses Vulkan, and draws the soup in one flat colour (a
  surface without a run has no field in the Studio); C8 is therefore a lower bound of the Studio's frame time, not its
  frame rate in a window.
- P2's latencies carry the clock join's error (0.1 ms from the round trip, 1.30 ms of drift over the sample); the
  in-process stamp is later than the emit by microseconds.
- Before the registered run the code was tried with small quantities on the 1e5 box (`--smoke`), and a Node script read
  all fields through the R2 reader against the references (it printed 8.1 ms per raw and 23.9 ms per deflated 1e6 field).
  After those trials the following changed: a syntax error, the tolerances (1e-300 was refused), the force report, the
  window's menu (the content was 826 high), the display record. No quantity or limit changed.
- The transcript of P11 was not read by `rcfd-claude`'s reader; the list shows role and text only.

### Not run

- **R3** (WebAssembly reader): `rustup target list --installed` gives `x86_64-pc-windows-msvc` only, and an export from
  `rcfd-tsstore` needs `#[no_mangle]` functions over raw pointers (`unsafe`, which the workspace forbids) or
  `wasm-bindgen` (a new crate); the reader's file calls would also fail at run time on that target.
- R1 over a Windows named pipe (TCP only), a `chat.*` method (Node read the file), a true cold start after a reboot, a
  second machine or operating system, a fan mesh.

### Decision by section 4

- P1, P2, P4, P5, P7, P8, P10 and P12 are met by both bulk routes (R1 on raw stores, R2 on raw and deflated), neither of
  which needs a new Rust crate. P6, P9 and P11 are met.
- **P3 is missed**, and P3 is in the list the rule requires for "Electron viable". So the outcome is not "Electron
  viable". It is not "viable with conditions" either (that outcome names a new crate or a level-of-detail reducer), and
  not "not viable now" (P4, P5 and P8 are met by both routes). The rule has no outcome for this case; the decision goes
  to the maintainer. Proposed status of F-447: partial.
- What the numbers say beside the rule: the bulk boundary is fast enough and exact by either route; the miss is in an
  existing JSON method whose cost is mostly on the service's side and whose remedy does not touch the shell.

### Remaining questions (options and a recommendation each)

- **QA1, P3.** (a) keep the limit and change the service: apply `since` and `names` before building the document, and let
  a shell fetch the history once and increments afterwards; (b) carry monitor series over `data.*` as `f64` sections;
  (c) accept 127 ms for a full history of 1e5 samples. Recommendation: (a), then measure P3 again as registered; (b) only
  if (a) misses. (a) changes the cost of an existing method, not its reply, so no golden changes.
- **QA2, the bulk route of phase 2.** (a) R2 for projects on the same machine (about twice as fast as R1 per snapshot
  here and twenty times at the first picture; no service needed to look at a finished run), with R1 for a service on another machine; (b) R1 only (one reader, in Rust;
  P4 met with little room); (c) R2 only. Recommendation: (a); R2 then needs the scan of an unfinished store and the mesh
  checksum before it is more than a spike.
- **QA3, merging `data.*`.** Q4 said "merged only if the spike is viable". (a) merge it as it is (off by default, no
  existing reply changes, tests D1 to D6); (b) leave it out until P3 is settled. Recommendation: (a): it is the only
  route for a remote service and costs nothing when off.
- **QA4, `rcfd serve`.** Switching `data.*` on from the command line needs a flag, which changes the CLI help golden.
  (a) add `--data` with phase 2; (b) not now. Recommendation: (a), with the maintainer's approval of the golden edit.
- **QA5, the first opening by R1** (1.58 s, and C7's 1.2 s): (a) a surface cache next to the store, written by the run
  path; (b) leave it. Recommendation: (b) until a fan mesh is measured (section 6).

### For the lead: roadmap entries

F-447, proposed status: partial.

> F-447: ADR 0350, 2026-11-12. The Electron spike ran once on the maintainer's machine (Electron 45.1.0, RTX 5080, 60 Hz display, load 0.3 to 0.6). Bulk data is fast enough and exact by both routes: R1 (the opt-in `data.*` methods, `rcfd-data/1`, service protocol 1.2, raw `f32` on a second TCP connection) loads a 1e6-cell scalar to a GPU buffer in p50 25.9 ms, p99 57.7 ms; R2 (a reader of `store.rts` in Node) in p50 14.2 ms, p99 22.4 ms; JSON (R0) takes 552 ms. Scrubbing 30.8 and 60.1 snapshots per s at 1e6 cells, 10.5 and 17.8 at 4e6; first picture of the 1e6 surface 1.15 s (R1, first opening 1.58 s) and 0.06 s (R2); 1e6 triangles at 332 frames per s without a rate limit and p99 16.9 ms per frame with vertical sync; cold start median 0.38 s (one start of ten took 6.7 s); renderer memory 112 MB with 20 snapshots; 0 differing bits in 672e6 values; service round trip p50 0.06 ms; event latency p99 under 2.1 ms. Missed: P3, `monitors.get` of 4 series of 1e5 samples takes 127 ms against 100 ms (a 12 MB reply, most of the time on the service's side). R3 (WebAssembly) not run: it needs `unsafe` exports or `wasm-bindgen`. The rcfd-studio control, offscreen: 12.7 ms per snapshot, 73 per s, first picture 1.25 s. The registered rule has no outcome for a miss of P3 alone, so viability is the maintainer's decision; questions QA1 to QA5 of addendum 1.

F-448, note to add (status unchanged):

> F-448: ADR 0350 addendum 1 (2026-11-12) measured both bulk routes as fast enough; recommended: the Node reader of `store.rts` (R2) for local projects, `data.*` (R1) for a remote service, monitors fetched once and then by increments. Open before the start: the maintainer's go-ahead, QA1 (the cost of `monitors.get`) and QA2 (the route).

F-449: no change.

Follow-up item (number for the lead), category "Studio desktop app", depends on F-447:

> Title: `monitors.get` at 1e5 samples: build only what is asked for.

> Acceptance: `monitors.get {names, since}` applies `names` and `since` before the document is built, with replies equal to today's byte for byte for every existing test; P3 of ADR 0350 measured again as registered (4 series of 1e5 samples, request to parsed object in the renderer) with the 100 ms limit unchanged, and an increment of 100 samples at 1e5 stored in at most 2 ms; a miss is recorded, not tuned.
