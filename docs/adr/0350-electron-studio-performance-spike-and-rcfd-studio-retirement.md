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
