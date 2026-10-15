# ADR 0025: Wave 17C GUI and assistant: headless verification of viewport, monitors, key storage and local endpoints (F-143, F-144, F-149, F-150, F-073)

Status: accepted. Everything under "Decisions" and "Registered checks" was written before any new test was written,
compiled or run. The limits are not changed, widened, replaced or deleted after a run. A miss is reported as measured and
the assertion moves to an ignored `*_known_miss` test with the registered limit intact; the cause is diagnosed, not tuned
away. A test set-up defect gets a NEW corrected test and the old one stays as a record. Addenda (file names, sizes not
fixed here, results) may be appended at the end without touching registered limits.

Roadmap: F-143, F-144, F-149, F-150, F-073 (F-094 only read, see "Audit"; F-092/F-093 are demoted extras and untouched).
Clean-room: published conventions only (Rec. 709 luma weights, "1-2-5" tick steps, min/max decimation); nothing from
vendor material, vendor installs or manuals; no vendor names in the repository.

## Scope and honesty statement

Nobody looks at a screen in this wave. Every check is a logic-level or data-to-pixel check executed headlessly: state
machines, a CPU raster function whose pixels are compared numerically, egui frames run with `egui::Context::run` and
the emitted shapes inspected, plot-data functions, key-store precedence, and HTTP round trips against a loopback mock
server written with `std::net`. What that cannot establish stays unverified and the items stay `partial`:

* that the viewport or plots LOOK right (layout aesthetics, readability of the legend, overlap, font size, dark theme);
* that plot update latency is acceptable to a person watching a real run;
* a real OS keychain (Windows Credential Manager, macOS Keychain), a real native file dialog, unix 0600 permissions on
  a unix machine (cfg(unix) code, this wave runs on Windows);
* a real hosted provider and a real local model server (Ollama, LM Studio, llama.cpp, vLLM) and the tool-calling quality
  of small local models. The mock server speaks the published OpenAI-compatible wire format as the code under test
  understands it; agreement of the two is not independent evidence about real servers.

## Audit (before this wave)

* F-143: `viewport.rs` has `GridData`/`SliceData` parsing, `colormap`, `legend_ticks`, `ViewState` range logic; `ui.rs`
  paints per-cell rectangles. Tests: `colormap_and_ticks_behave` checks only "endpoints differ" and "no panic". No pixel
  value, hover mapping, tick niceness or legend orientation is checked. The painting loop and the hover mapping live in
  `ui.rs` and cannot be called without egui.
* F-144: `monitors.rs` has incremental merge and log points; tests check idempotence for 2 points and one log value. No
  decimation exists: every sample of every series is handed to `egui_plot` every frame.
* F-149: `Secret` redacts Debug; `LayeredKeyStore`; env var precedence; `tests/keychain.rs` (7) and `providers.rs`.
  Missing: one ordered env > keychain > file test, a planted-canary scan across all outputs including the config files, the
  GUI status line and a server that echoes the key, and a check that the key never reaches `providers.json` or the GUI
  preferences file.
* F-150: presets exist; `providers.rs::local_preset_builds_a_working_request_and_lists_models` uses `FakeTransport`
  (no socket). The real `UreqTransport` path (headers on the wire, chunked streaming, timeouts, refused connections) has
  never run in a test.
* F-073: `rcfd-post/src/render.rs` has 5 unit tests; no pixel check at a field point other than gradient endpoints, no
  timing, and the roadmap criterion (1M cells, under 2 s) is unmeasured.
* F-094: eval harness C1-C8 all pass (its note); nothing to do except that C5-style secret handling is reused here.
  The optional real-model run is not done and cannot be done by an agent.

## Decisions

### D1 Raster module (`rcfd-gui/src/raster.rs`, new, no egui types)

Pure functions that the UI also uses so that what is tested is what is drawn:

* `Rgba { w, h, px: Vec<[u8;4]> }` with `pixel(x,y)` and `to_png()` (the `png` crate, already in the workspace through
  `rcfd-post`; added to `rcfd-gui`, same version, no new crate in `Cargo.lock`).
* `render_grid(&GridData, lo, hi, cell_px) -> Rgba`: row 0 is the top row (as in `field.grid`), each cell is a
  `cell_px x cell_px` block of `colormap(v, lo, hi)`, undefined cells are `BACKGROUND = [255,255,255,255]`.
* `render_cells(&SliceData, extent, w, h, lo, hi) -> Rgba` stamping squares of `cell_square_size` at cell centres
  (y up), background elsewhere.
* `render_legend(lo, hi, w, h) -> Rgba` (top row = `hi`, bottom row = `lo`).
* `hover_cell(&GridData, fx, fy) -> Option<(usize, usize, f64)>` with `fx, fy` in `[0,1]` of the drawn rectangle (top-left
  origin); this replaces the inline hover arithmetic in `ui.rs`.
* `tick_fraction(lo, hi, t)` = `(t - lo)/(hi - lo)` guarded; replaces the inline legend position arithmetic.
* `viewport.rs` gains `luminance(rgb)` (Rec. 709 weights on the 8-bit values). `colormap` and `legend_ticks` are NOT
  changed (existing behaviour is what is measured).

### D2 Monitor data (`rcfd-gui/src/monitors.rs`)

* `decimate_minmax(points: &[[f64;2]], max_points) -> Vec<[f64;2]>`: if `len <= max_points` the input is returned
  unchanged; otherwise the x-range is split into `(max_points - 2) / 2` equal-count buckets (by index), and from each bucket
  the sample with the minimum y and the sample with the maximum y are kept in original order, plus the first and the last
  sample of the whole series. Output is a subsequence of the input, so x stays increasing. NaN y values are skipped for
  the extrema choice (the plot data already excludes them).
* `MonitorStore::plot_points_decimated(name, log, max_points)` = `decimate_minmax(plot_points(..))`; `ui.rs` uses it with
  `MAX_PLOT_POINTS = 4000` per series.
* The log mapping stays `y -> log10(y)` for finite `y > 0`; others are dropped. The registered check measures it.

### D3 Key and endpoint handling

No behaviour change is planned. Tests only, unless a check misses. If a leak is found it is fixed and the finding recorded
(a leak is a defect, not a registered-limit miss). Server-side mock lives in the test file only.

### D4 Mock server

`std::net::TcpListener` on `127.0.0.1:0`, one thread, handles requests sequentially, parses the request line, headers and
Content-Length body, records them, and answers from a scripted queue (fixed-length JSON, chunked SSE, error with echoed
key, delayed/never-answering, dropped connection). No external crate. Loopback only; no other network use.

## Registered checks

Notation: "pass" means every instance passes; counts are exact.

### F-143 viewport (`rcfd-gui/tests/viewport_raster.rs`)

* V1 colour map: `colormap(lo)` is exactly `[68,1,84]` and `colormap(hi)` exactly `[253,231,37]`; values below `lo`
  and above `hi` give the same colours as the endpoints; NaN gives the `lo` colour; a degenerate range (`hi <= lo`) does not
  panic; `luminance` is non-decreasing over 1001 equispaced samples (tolerance 0) and the total rise is at least 150 of 255.
* V2 raster pixels: for a `GridData` of 17 x 9 cells with analytic values `v(c,r) = 0.25 + 3*c/16 - 0.5*r/8` (and 5
  cells set to `None`), `render_grid` with `cell_px = 4` gives, at every pixel of every cell, the channels of
  `colormap(v, lo, hi)` within 1/255 (expected exact) and `BACKGROUND` at every undefined cell. 100 % of pixels.
* V3 hover mapping: for the same grid, the cell found by `hover_cell` at the centre of each cell equals `(c, r, v)` for all
  153 cells; the four corners clamp to the corner cells; `fx` or `fy` outside `[0,1]` or an undefined cell returns `None`.
* V4 legend ticks: for `n = 5` over each of 12 ranges (`(0,1) (0,0.37) (-3.2,7.9) (1e-6,9e-6) (1e5,1.3e6) (-1,1) (0,1e-12)
  (2,2.0001) (-1e3,-10) (0,100) (3.1,3.2) (-0.001,0.002)`) and 300 pseudo-random ranges (fixed seed): every tick lies in
  `[lo - 1e-9 span, hi + 1e-9 span]`; the spacing is constant to 1e-9 relative; the step is `m * 10^k` with `m` in
  `{1, 2, 2.5, 5}` (to 1e-9 relative); the tick count is between 2 and 10 inclusive; and `tick_fraction(lo,hi,lo) = 0`,
  `tick_fraction(lo,hi,hi) = 1` within 1e-12.
* V5 legend image: top-row colour is `colormap(hi)` and bottom-row colour `colormap(lo)` within 1/255; luminance is
  non-increasing from the top row to the bottom row (tolerance 0).
* V6 cell view: `render_cells` for a 12 x 12 lattice of centres with analytic values puts `colormap(v)` within 1/255 at the
  pixel nearest to each centre (144 of 144), with y up (the centre with the largest y is in the first image row).
* V7 range logic: `Fixed(a,b)` with `b > a` is returned unchanged by `effective_range`; `Fixed(a,b)` with `b <= a` falls
  back to the data range; a degenerate data range gives width exactly 1 centred on `lo`; fixed-range clamping: a value
  above the range colours as `colormap(hi)`.
* V8 PNG: `to_png` decoded with the `png` crate has the original width and height and every pixel equal (0 differing
  pixels); the file is also written to `target/tmp` for inspection by a human later (not claimed as inspected).
* V9 headless frames: a cavity run (template case, 60 iterations) with the viewport floated: one `Context::run` frame at
  1280 x 800 emits filled-rectangle shapes whose fill colours include `colormap(min)` and `colormap(max)` of the grid in
  auto range, the number of filled rectangles with a colour that appears in the raster function output is at least the number
  of defined cells, and 48 legend swatches are present. Frames at the sizes `(320,240) (640,480) (800,600) (1280,800)
  (1920,1080) (3840,2160) (100,60)` run without panic and with every emitted rectangle having finite coordinates (7 of 7).
* V10 lid-driven cavity logic check (not a visual claim): after 300 iterations on the template cavity the `u` grid has
  top-row mean greater than 0.1 times the lid speed and a minimum below 0 (return flow) and the `speed` grid maximum lies in
  one of the top two rows. Whether the field "shows a vortex" to a human is not claimed.

### F-144 monitors (`rcfd-gui/tests/monitors_plot.rs`)

* M1 log mapping exactness: for `y = m * 10^k` with `k` in `-30..=30` and 20 mantissas, `plot_points(.., true)` y equals
  `y.log10()` exactly (bitwise) and `10^(that) / y - 1` is within 1e-12 in magnitude; for exact decades `10^k` the mapped
  value differs from `k` by less than 1e-12.
* M2 log filtering: with a series containing `0`, negative values, NaN and infinity among 100 finite positive values the
  log points number exactly 100, linear points number exactly the finite ones, and no point has non-finite coordinates.
* M3 decimation: for `N` in `{5, 1999, 2000, 2001, 10_000, 1_000_000}` and `max_points = 2000` on a noisy series with
  planted spikes: output length is at most `max_points`; for `N <= max_points` output equals input; first and last points
  are retained; the global minimum and maximum y are retained exactly (bitwise); x is strictly increasing; every output point
  is an input point; and for each of the buckets the bucket minimum and maximum of the input appear in the output (all
  instances).
* M4 stream ingest: 3 series of `N = 100_000` points each (values from a deterministic function incl. 100 `null`s) are
  merged from documents of pseudo-random chunk sizes 1 to 500, each document also repeating the last 10 points of the
  previous one (as `since` retries do); the store holds exactly `N` points per series, x equals `1..=N`, y equals the source
  values bitwise (NaN where null), the sum of returned `added` counts is exactly `3N`.
* M5 real run through the controller: a template cavity run for 60 iterations polled with `poll(false)` from the calling
  thread every 5 ms while it runs: each `poll` call returns in under 1 s (the UI thread does not wait for the solver),
  at least one poll before completion sees a state `running` with fewer than 60 iterations (non-vacuous), and after the run the
  `res_u` series has exactly `status.iteration` samples with strictly increasing x and no repeated x.
* M6 frame with large series: the store with 3 x 200 000 points: `plot_points_decimated(.., 4000)` returns at most 4000 per
  series and a monitors-panel frame draws without panic (1 of 1).

### F-149 keys (`rcfd-assistant/tests/key_security.rs`, `rcfd-gui/tests/key_security.rs`)

* K1 precedence, one test, four stages with the same provider: env var set -> the env value; env removed, keychain and file
  both hold different values -> the keychain value; keychain unavailable -> the file value; nothing anywhere -> `None`
  (4 of 4, exact string comparison on `expose()`).
* K2 planted canary: a canary `rcfd-canary-<fixed 24 chars>` is given to the system through (a) the key box and
  `save_key` (success path; failure path with `echo_value_in_errors`), (b) the environment variable, (c) the file store, and a
  mock server answers 401/500 with bodies that echo the canary raw, inside JSON, and inside a longer message. The captured
  strings are: `Display` and `Debug` of every `ProviderError`, `Debug` of `Secret`, `ProviderConfig`, `AssistantConfig`, the GUI
  status line after every step, `key_storage_note`, the list_models/chat/streaming error texts, the bytes of `providers.json`
  and of the GUI preferences file after saving, the request URL and the request body. The canary occurs in none of them
  (0 occurrences over all captured strings) and does occur in the `Authorization` header received by the mock server
  (non-vacuous). A scanner self-test shows that the same scan DOES flag a deliberately unscrubbed string.
* K3 file store: set/get/delete round trip in a temp directory; the file parses as an object with exactly the provider keys
  stored; deleting removes the key from the file; `providers.json` written after `save_key` contains no stored key
  material (covered by K2). Unix mode 0600 is code that is not run on this Windows machine: stated unverified.

### F-150 local endpoints (`rcfd-assistant/tests/local_endpoints.rs`)

All use `UreqTransport` against the loopback mock server with the preset's URL path and the mock's port substituted.

* L1 preset requests: for each of `ollama`, `lm-studio`, `llama-cpp`, `vllm` (4 of 4): the preset base URL ends in `/v1`
  and its default port is the documented one (11434, 1234, 8080, 8000); a chat call produces `POST /v1/chat/completions`
  with `Content-Type: application/json`, no `Authorization` header, a JSON body with the configured model, the messages and the
  tool list, and `stream: false`; the returned text equals the scripted text exactly; a scripted tool call comes back
  with the right name and arguments.
* L2 model probe: `GET /v1/models` (`AssistantSetup::probe`) returns exactly the scripted ids in order; an empty `data`
  list gives an empty list; a body that is not JSON gives a `Parse` error; HTTP 404 and 500 give `Http` errors with the status.
* L3 streaming: a chunked SSE body with the text split into fragments and a tool call whose JSON arguments are split over 5
  events accumulates to the exact text and the exact arguments value, the events arrive in order, and a stream cut before
  `[DONE]` is an error rather than a silent success.
* L4 keyed endpoint: with `key_env` set the mock receives `Authorization: Bearer <value>` exactly once and the value is in no
  other part of the request.
* L5 connectivity failures: a refused connection (closed port) yields `ProviderError::Transport` in under 5 s with the text
  free of the key; a server that accepts and never answers, with `timeout_secs = 1`, yields an error in under 3 s;
  a server that closes mid-body yields an error; none panics (3 of 3).
* L6 quick actions without a model: with the active provider pointing at a closed port, each of the four quick actions
  completes on the existing no-model path and the number of connections the mock server saw during the actions is 0
  (the mock is listening and counts accepts) (4 of 4).

### F-073 headless renderer (`rcfd-post/tests/render_pixels.rs`)

* R1 pixel values: `render_scalar` of the analytic field `f = x` (cell centres) on a 64 x 64 unit cavity mesh at width 640
  without colour bar: at each of 20 fixed probe points (cell interiors) the pixel equals `colormap((f - vmin)/(vmax - vmin))`
  within 1/255 per channel (20 of 20).
* R2 timing, the roadmap criterion: `render_scalar` of a 1000 x 1000 (1 M cell) 2D mesh, width 1024, including the mesh
  generation excluded and the PNG encode excluded, completes in under 2 s on the development machine in a release build;
  the PNG encode time is measured and reported separately. The test is `#[cfg_attr(debug_assertions, ignore)]` (the
  criterion is for a release binary); the number measured on this machine is reported, not generalised.
* R3 PNG: the encoded image decodes to identical pixels (0 differing).

## Not claimed

Visual correctness of anything; human-perceived latency; the real keychain, native dialogs, unix permissions; any real
provider or local server; that R2 holds on other hardware; the cavity vortex "looks" like a vortex.

## Files

New: `crates/rcfd-gui/src/raster.rs`, `crates/rcfd-gui/tests/{viewport_raster,monitors_plot,key_security}.rs`,
`crates/rcfd-assistant/tests/{key_security,local_endpoints}.rs` and a small shared `tests/common` mock server module,
`crates/rcfd-post/tests/render_pixels.rs`. Edited: `crates/rcfd-gui/src/{monitors,viewport,ui,lib}.rs`,
`crates/rcfd-gui/Cargo.toml`, docs and roadmap.
