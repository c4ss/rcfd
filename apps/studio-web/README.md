# rcfd Studio (Electron)

The Electron desktop front end of rcfd: F-448 and the Studio roadmap of ADR 0800 (F-800 to F-832). This directory is
outside the Cargo workspace (ADR 0350 Q2, guardrail G5). The engine is Rust and stays Rust (G2); this app is a client of
it.

For users: how to start the Studio from a checkout and use it, step by step, is the tutorial
`docs/tutorials/desktop-assistant.md` (F-822).

**State: F-801 hardened shell, F-802 design tokens and primitives, F-803 service client.** The window shows an empty
project view inside the hardened shell of guardrail G10, with Playwright tests that drive the packaged app. The design
tokens of `DESIGN.md` and the primitives (on React Aria Components) are in place, shown on a primitives gallery. The main
process is a client of the solver service (`rcfd-service/1`): it attaches to a running `rcfd serve` or starts its own,
long-polls its events and reconnects when it restarts; the renderer reaches it only through `window.studioService` and
an allow-list of methods. F-805 adds the project home: a project folder opened through the main process's folder
dialog, read through the engine's project server, and kept live (below). Dependencies and layout: ADR 0801
(accepted), addendum 1 (F-801) and addendum 2 (F-803); F-802 added no dependency.

    npm ci                    # exact versions from package-lock.json
    npm run typecheck         # renderer (tsconfig.json), main + preload + shared (tsconfig.node.json), tests (tsconfig.test.json)
    npm run build             # typecheck, then vite build (renderer) and tsc (main, preload, shared) into dist/
    npm test                  # build, package into out/packaged/, then every Playwright test (unit and e2e); no human step
    npm run test:unit         # the unit tests only (needs dist/ for the built index.html check)
    npm run package:dir       # package dist/ by hand into out/packaged/ (rcfd-studio.exe; resources/app)
    npm run tokens:css        # regenerate src/renderer/design/tokens.css from tokens.ts (a unit test fails when stale)
    npm run capabilities      # F-824: inventory.json from the rcfd binaries, then docs/studio-coverage.md (-- --report-only: report only)
    npm run gallery           # open the packaged app on the primitives gallery at 1440x900 (view-and-report)
    npm run bench:cold-start  # P10, 10 cold starts of the packaged copy; a heavy record, run by hand only
    npm run bench:service -- --bench-server=<f447_bench>   # P1 and P2 through the shell; heavy, by hand only
    npm run bench:data -- --bench-server=<f447_bench> --data=<folder> [--p12]   # P4 (and P12 at 1e6, 4e6); by hand only

    npm run bench:plots -- --rcfd=<rcfd>   # P3 through the shell, with the plots' first fetch and draw; heavy, by hand only

    npm run field -- --data=<folder> [--store=<fields folder>]   # F-808: the field viewport on a store at 1440x900 (view-and-report)
    npm run bench:field -- --data=<folder> [--bench-server=<f447_bench>]   # P7 and P8 through the shell; heavy, by hand only
    npm run bench [-- --heavy]  # F-817: P2, P4, P5, P7, P8, P10 against their limits in one run (below); not in npm test
    npm start                 # electron . (run npm run build first); downloads the Electron binary on first use

`npm test` opens the Studio window for a few seconds on Windows (Electron has no headless mode there); it needs no input.
On Linux it needs a display (CI uses a virtual one, F-823). The service tests of `tests/e2e/service.spec.ts` need the
`rcfd` binary (`cargo build --release -p rcfd-cli`; found through `RCFD_BIN`, `$CARGO_TARGET_DIR/release` or a
`target/release` above this folder); without it they are skipped and say why. The bulk data tests
(`tests/unit/data-routes.spec.ts`, `tests/e2e/data.spec.ts`) need the Rust example `f447_bench`
(`cargo build --release -p rcfd-api --example f447_bench`; found through `RCFD_F447_BENCH`, `$CARGO_TARGET_DIR` or a
`target/release/examples` above this folder), which makes their data sets afresh on every run; without it they are
skipped and say why.

## The solver service (F-803)

Which service the Studio uses is set by its environment (`src/main/service/config.ts`); where a packaged Studio finds its
`rcfd` is F-820's:

| Variable | Effect |
|---|---|
| `RCFD_STUDIO_SERVICE=127.0.0.1:7878` | attach to a service someone else started; loopback addresses only (G10). The Studio never stops it. |
| `RCFD_STUDIO_RCFD=<path of rcfd>` | start `rcfd serve --addr 127.0.0.1:0` (detached, output to `service.log`) and own it |
| `RCFD_STUDIO_SERVICE_ROOT=<dir>` | with `RCFD_STUDIO_RCFD`: the service's sandbox (`--root`) |
| `RCFD_STUDIO_STATE_DIR=<dir>` | where `service.json` (address and process id of a started service) and `service.log` go; default `<user data>/service` |
| neither of the first two | no service: state `off`, every call answers `not_configured` |

- Two TCP connections: one for calls, answered in the order sent, and one that only long-polls `events.poll`, so a
  waiting poll never holds up a call. Every request has an id and a time limit; nothing waits on the main thread.
- When a connection drops, calls fail at once with `disconnected` and the client tries again after 100 ms, doubling to at
  most 5 s. A service the Studio started is started again if its process is gone. After a reconnection the client tells
  a restarted service from the same one by the last event it saw, and a restart raises `instance` in the state.
- Runs outlive the window (G7): a reload does not touch the connections. When the Studio quits, a service it started is
  stopped only if it holds no session; otherwise it keeps running, and the next start attaches to it again
  (`adopted: true`).
- `RCFD_STUDIO_BENCH=1` (measurement only, `scripts/measure-service.mjs`): the client is reachable from Playwright's
  main-process evaluate. Unset, nothing is exposed.

## The project home (F-805)

The window opens on the project home. With no project open it says so (the first `h1` is "No project open"), offers the
folder dialog and the recent list. An open project is one ruled index of its `cases/`, `meshes/`, `runs/` and `notes/`,
filtered by kind and by name, and a lane of every change the engine's feed reports, newest first. Surface brief:
`.impeccable/surfaces/web-src-renderer-project-projecthome-tsx-41df8cd1.md`.

| Variable | Meaning |
|---|---|
| `RCFD_STUDIO_MCP=<path of rcfd-mcp>` | the engine's project server; for each open project the main process starts `rcfd-mcp --project <folder> --read-only` |
| `RCFD_STUDIO_RCFD=<path of rcfd>` | (F-803) when `RCFD_STUDIO_MCP` is not set: the `rcfd-mcp` beside that `rcfd` |
| `RCFD_STUDIO_RECENT=<file>` | where the recent list is kept; default `<user data>/recent-projects.json` |

Neither set: the home says what to set, and "Open project folder" is unavailable with the reason.

- Through the engine: the main process asks the project server `project_info`, `list_runs`, `list_meshes` and
  `read_feed` (the workspace of ADR 0173, the tools Claude Code uses, so both see one folder the same way). The server
  runs `--read-only`; the Studio writes nothing to the folder (G12; the e2e test compares a snapshot of every file).
- Live: the main process reads the feed every 400 ms (`FEED_POLL_MS`); when it holds anything it reads the lists again
  and pushes the new state. Edits from Claude Code, the CLI, the runner or an editor show within 2 s without a reload
  (measured by `tests/e2e/project.spec.ts`: new case, new note, deleted note, new run folder). When the feed cannot be
  read or the server stops, the header says "Not live", a notice says why and what is shown, and Reconnect starts a new
  server.
- The renderer reaches the home only through `window.studioProject` (`state`, `onState`, `offState`, `pick`, `open`,
  `forget`, `close`, `refresh`, `dismiss`), from one module, `src/renderer/project/door.ts`. A second MessagePort on
  `studio.project-port`, like the service port; `src/main/project/bridge.ts` checks every message (exact fields, a path
  of at most 4096 characters). The page cannot name a folder: `open` takes only a folder of the recent list, and new
  folders come from the main process's dialog. The ipcMain channels are `app.info`, `app.ready` and (F-837)
  `app.closeAnswer`.
- The recent list holds each folder's path and when it was last opened, nothing of the project (G12). It is never opened
  on its own at start.

Decisions taken in building F-805 (options, the recommendation followed; each can be changed later without touching the
engine):

- D1, the route to the folder: (a) the engine's project server `rcfd-mcp --project <folder> --read-only`, one per open
  project, or (b) new project methods in `rcfd serve`'s protocol (a Rust change, outside this item). Taken: (a).
- D2, case and note names: the engine has no query that lists them (`project_info` counts them; `list_meshes` names only
  the cases that use a mesh or that it cannot read). (a) the main process lists the file names under `cases/` and
  `notes/` the way the engine counts them (`src/main/project/listing.ts`; names only, never content), until the engine
  has the query; (b) counts only; (c) add `list_cases` and `list_notes` workspace queries (Rust, a follow-up). Taken:
  (a) now, (c) proposed. The page shows when a name count and the engine's count disagree.
- D3, where the project server comes from: `RCFD_STUDIO_MCP`, else beside `RCFD_STUDIO_RCFD`; never from `PATH`. Where a
  packaged Studio finds it is F-820's.
- D4, the recent list: paths and times only, at most 10, in the user data; never opened on its own at start (the empty
  window stays "No project open").
- D5, which folders the page may open: only the dialog's choice or a folder of the recent list.
- D6, paused and cancelled runs: `DESIGN.md` has no pictogram for them; shown as words only until F-806, which added paused and cancelled pictograms (proposed for DESIGN.md).
- D7, where these decisions live: here, rather than an ADR 0801 addendum (F-805 adds no dependency); they can move into
  an addendum if preferred.
- The folder dialog, the recent list and the project server's queries are main-process operations behind the project
  port, not service methods, so they have no entry in the action registry (F-827) yet (G16).

## Bulk field data (F-804)

The page asks `window.studioData` (`src/shared/data.ts`) for a store's sizes and snapshots (`open`), one field of one
snapshot as a `Float32Array` (`field`) or the boundary surface (`surface`), naming the store as `{route, path}`:

| Route | What reads the store | `path` |
|---|---|---|
| `local` (R2 of ADR 0350) | the main process, in a worker thread: `src/main/data/tsstore.ts`, a port of the Rust reader of `crates/rcfd-tsstore` (footer or scan, every checksum it reads) | an absolute folder holding `store.rts`, inside a data root |
| `service` (R1) | the service's opt-in `data.*` methods (`rcfd-data/1`, protocol 1.2): the call on the client's call connection, the bytes on one more TCP connection to its data port | the folder as the service takes it (its own sandbox applies) |

- Every value the page receives equals the Rust reader's value cast to `f32`, bit for bit (G3, P12); the tests compare
  against reference bytes written by the Rust reader.
- `RCFD_STUDIO_DATA_ROOTS=<dir>[;<dir>...]`: the folders route `local` may read (real paths, links resolved). Without it
  nothing local is read; F-805's project folder adds its own root.
- `rcfd serve` does not switch `data.*` on (ADR 0350 QA4), so route `service` answers `no_data` there; the spike's server
  `f447_bench serve` has it on.
- Nothing blocks (G21): local reads run in a worker thread, the service route is asynchronous, every request has a time
  limit and `cancel(tag)` answers waiting requests at once. Values cross from main to the renderer as one structured
  clone of an ArrayBuffer over a second MessagePort, then once more through `contextBridge` (ADR 0801 addendum 3).
- F-819b: `window.studioSurfaceValues.field(store, snapshot, field)` (route `local`) gives a cell field on the boundary
  surface only, one value per triangle of `surface` (its owner cell's stored value, bit for bit), gathered in the
  reader's worker where the whole field was read (`src/shared/surface-values.ts`); the field viewport draws, ranges and
  reads out from it. The reader remembers, for the launch, each store file whose mesh checksum it verified (path,
  device, file index, size, modification, change and creation times, the mesh frame's length, checksum and hash) and
  verifies any other file, or the same file changed. Its FNV-1a 64 runs a WebAssembly loop assembled in
  `src/main/data/fnv.ts` for many bytes (the reference loop for few).

## The run list and run state (F-806)

An open project has two views, Home (above) and Runs: the runs table of `DESIGN.md` (case and run, state pictogram and
word, result; sorted by any column, filtered by text and by state) and, beside it, the selected run: its numbers, why it
is in its state, and the state changes the runner recorded. Selecting a run fades the other rows by colour, never a
failed one; Escape clears the selection. Surface brief: `.impeccable/surfaces/web-src-renderer-runs-runsview-tsx.md`.

- Through the engine: `list_runs` and, for each run whose state or step changed, `run_status` (the last 50 events,
  `RUN_EVENTS`), asked by the main process (`src/main/project/runs.ts`) of the same read-only project server. The runner
  owns the runs (G7): closing or killing the Studio leaves them going, and the next start shows their true state
  (`tests/e2e/runs.spec.ts` closes and kills the Studio during a run started from `rcfd-runner`, and checks the step
  against the runner's own `status.json`).
- Live: state changes come with the feed (400 ms); while a run is queued, running or paused the main process also reads
  the run list every second (`RUN_POLL_MS`), since the feed reports state changes and not progress.
- Time: shown only where it is a simulated time. A steady solver writes time 0, and the engine writes NaN as `null`; the
  table says "none, steady", "none yet", "none" or "not known" instead (G3). The home's run list says the same.
- Tests: `tests/unit/runs.spec.ts` (answers, table model, live reads, pictograms, stylesheet), `tests/e2e/runs.spec.ts`
  (needs `rcfd-runner` beside `rcfd-mcp`, or `RCFD_RUNNER_BIN`; screenshots in `test-results-visual/f806/`).

Decisions taken in building F-806 (options, the recommendation followed; each reversible without touching the engine):

- D1, where the runs live: (a) a Runs tab beside Home, table left and the selected run right; (b) a larger runs group on
  the home; (c) the prototype's run workspace with its columns still empty. Taken: (a).
- D2, keeping progress live: (a) read the run list every second while a run is active, `run_status` only for changed
  runs; (b) `wait_run` with a timeout (blocks the server for that time); (c) feed only (no progress). Taken: (a).
- D3, Converged: a finished run is shown as Converged when the solver's finish event says converged, else Finished (the
  reason says not converged or end time reached).
- D4, paused and cancelled: drawn on the pictogram grid (`RUNNER_STATES` in `src/renderer/ui/pictogramPaths.ts`), ink
  coloured, proposed for `DESIGN.md` and `design.json`; kept apart from the five recorded states until recorded.
- D5, report values: shown in the number grid with the note that the engine gives no unit for them.
- Coverage: `run_status`, `rcfd-runner list` and `rcfd-runner status` have UI paths; `wait_run`, `rcfd-runner wait`,
  `events.poll` and `log.tail` now point at run control (F-807) as their nearest home (the table needs no waiting call
  and shows the runner's runs, not service sessions).
- Missing in the engine (no Rust change here): units of report values; a read-only query of whether the project's runner
  is alive (`RunService::runner_running` has no MCP tool), so a queued run under no runner cannot be told apart.

## The run workspace (F-810)

The third view of an open project, Workspace, is where a run is read through time: the runs of the project and the
selected run's numbers on the left, its field (the viewport of F-808) on the right, and under both the 248 px time panel
of `DESIGN.md`: one band per run on one shared axis, the selected run's residual plot on the same axis, a monitor
readout and chart, and one scrubber line with its own flag lane. "Open in workspace" in the Runs view brings the
selected run here. Code: `src/renderer/workspace/` (`RunWorkspace.tsx`, `TimePanel.tsx`, `model.ts`, `door.ts`,
`workspace.css`), `src/main/run/` and `src/shared/run.ts`.

- **Run bands**: each band ends in its state's pictogram and where it ended ("it 121"; a transient run's end in
  simulated time, "13.2 s"); the full label ("converged · it 121") is the end's accessible name and tooltip. A failed or
  cancelled band keeps the word on screen ("failed · it 0"), the Quiet Rule's exceptions (F-833). Segments: sky iterating, a meadow
  cap for converged, a red cap for failed, silver hatching for queued and for the part of the axis a running run has not
  computed yet; orange only for the one live run (One Live Thing: the selected run if it runs, else the newest running
  one). The other bands fade to 40% fill, a failure never. The bands grow in (700 ms, staggered 90 ms) except under
  reduced motion.
- **One line moves everything**: the scrubber stops at the store's snapshots (a run without fields: at its residual
  samples). Moving it (pointer, arrow keys, Page Up and Down, Home and End, the transport's first, previous, Play, next)
  moves the field, the residual cursor, the monitor readout and the snapshot counter together; they read the same stop.
  Play steps to the next snapshot once the one before is drawn. The flag reads "it 40", or "t = 1.25 s · it 145" for a
  transient run.
- **Follow live**: on for a running run; the line stays on its newest snapshot as the runner writes them; moving the line
  stops following, Follow live brings it back.
- **The axis is iterations** (steps) for every run: the store's snapshots, `read_residuals` and `status.json` share it,
  and `read_residuals` gives no simulated time per sample.
- **Through the engine only**: a fourth port, `window.studioRunData` (`src/shared/run.ts`; the one renderer module naming it
  is `workspace/door.ts`), asks the open project's read-only project server: `read_residuals` (every series on its own,
  at most `SERIES_POINTS` = 800 samples, so the reply stays under the engine's text budget and is never written to a
  file), `read_residuals {since_step}` for the exact values at the line when a series was decimated or the line is past
  what was read, and `read_project_file` for the units in `runs/<id>/output/monitors.json` (at most 1 MB). Fields come
  through the data door (F-804) from `runs/<id>/fields`; the project folder is a data root while it is open.
- `performance` marks `rcfd:scrub:request` and `rcfd:scrub:drawn` (P5 and P6, `npm run bench:scrub`).
- Tests: `tests/unit/workspace.spec.ts` (stops, axis, bands, the port's checks, reading series and units against a fake
  engine, the door, the stylesheet) and `tests/e2e/workspace.spec.ts` (a copy of quick-start and the real runner: one
  snapshot's field values, residual cursor, monitor readout and counter agree with `store.rts`, `residuals.jsonl` and
  `field_summary`; the bands; the keyboard; Follow live on a running run; screenshots at 1440 x 900 in
  `test-results-visual/f810/`).

## Comparing two runs (F-815)

In the run workspace, Compare runs (the pictogram toggle in the field bar, or the palette's Compare runs, which asks
which run) adds a second run to the selected one. The selected run is the baseline A, the compared run B; the rules are
ADR 0801, addendum F-815 (C1 to C4), coded in `src/shared/compare.ts`. Code: `src/renderer/workspace/Compare.tsx`,
`compare-model.ts`, `compare.css`; the difference in the main process's reader (`src/main/data/worker.ts`).

- **The same time** (C1): steady runs are compared at the same iteration, transient runs at exactly the same stored
  simulated time. The scrubber stops at A's snapshots; where B has no snapshot at that stop its view says
  "No snapshot at it 45" (or "No snapshot at t = 1.25 s"). Nothing is interpolated, nothing taken from the nearest
  snapshot. A steady run is not compared with a transient one ("Steady against transient: no common time").
- **Side by side**: two views edge to edge, one scrubber moving both, one legend under both: the colour range is exactly
  the union of the two views' own ranges (C4). Each view keeps its own camera.
- **Difference**: B − A of the stored values in f64, computed in the main process's reader worker from the two stores
  (C3), drawn as f32 with its own range; the readout shows the f32 of a cell's difference. It is refused, in words, when
  the meshes differ (C2: the cell counts or the recorded mesh content hashes, rcfd-post's identity rule), when one run
  lacks the field, when the units differ, for a point field and for a value that is not finite. No remap is done.
- **Residuals overlaid**: the time panel's residual plot and monitor chart draw B's lines with A's, thinner and in the
  secondary ink; the list under the plot gives B's residuals at the line. B's band keeps its fill and gets a dashed ink
  underline.
- **Metrics**: `compare_runs` of the two runs (the engine's metrics and its difference) in a table under the run's numbers.
- Through the engine and the stores only: the difference is a new data-port request (`difference`, on its own bridge
  `window.studioCompare` so that `window.studioData` keeps its four functions; local route only:
  the service route has no field difference, see the addendum's missing entry points), and `compare` a new run-port
  request (`compare_runs` through the read-only project server).
- Tests: `tests/unit/compare.spec.ts` (the rules on the example projects' stores; the mesh rule read from rcfd-post's
  source; the difference through the reader worker against an independent f64 decode of the stored bytes,
  `tests/support/rts-decode.ts`; the ports; compare_runs of the real project server; the palette; pictograms and
  stylesheet) and `tests/e2e/compare.spec.ts` (a copy of quick-start plus two short runs that store f64 fields: both
  views at the same snapshot time, the one range, the refusals, the difference bit for bit, the residual overlay, the
  metrics, the keyboard, the palette; screenshots at 1440 x 900 in `test-results-visual/f815/`).

## The field viewport (F-808)

`src/renderer/field/FieldViewport.tsx` draws one cell field of one snapshot of a store on the boundary surface of the
stored mesh: three.js `WebGPURenderer` (made and initialised asynchronously inside react-three-fiber's `Canvas`), with
three's WebGL 2 back end as the fallback; the back end that runs is named for readers ("Drawn by WebGPU"), and the
fallback, a loss, is named above the canvas ("Drawn by WebGL 2 (fallback: <why>)", F-833). Until a project view hosts it (F-806), it is shown on a development page,
`#/dev/field?route=local&path=<fields folder>&field=p&snapshot=19[&backend=webgl]` (`npm run field`).

- **Colour from the raw value on the GPU** (G3, the Data Owns the Field Rule): each triangle corner carries its owner
  cell's stored `f32`; a TSL material computes the table entry `k = floor(clamp((v - lo) * inv, 0, 1) * 255 + 0.5)` in the
  vertex stage (passed flat) and reads it from a 256 x 1 texture with nearest sampling. Unlit, no tone mapping, no
  colour space conversion, no MSAA: a pixel is exactly a table colour.
- **The table is rcfd-render's map** (`field/colormap.ts`): entry k is rcfd-render's `ColorMap::viridis().eval(k / 255)`
  (the 9 stops of `rcfd_post::render::colormap`). Stated tolerance: exact at the 256 table points and at and beyond both
  ends; between them a value's colour is within 3 levels of 255 per channel of rcfd-render's colour of the same value
  (a 256-entry table cannot hold the 635 colours of the piecewise-linear map). ADR 0801, addendum F-808, F808-D1.
- **Legend** from the same table (its 256 entries put into a canvas), ticks in rcfd-studio's `{:.4e}`, the unit in the
  title; the range is the min and max of the shown values (rcfd-studio's snapshot range).
- **Hover readout**: the stored value of the cell under the cursor (a CPU pick with three's Raycaster on the drawn
  triangles), its shortest text that reads back as the same `f32`, with cell and triangle. From the keyboard the view
  orbits with the arrow keys, zooms with + and -, resets with Home, and reads out the value at its centre. F-819b: no
  pick while a button is held (an orbit or pan); the readout goes at the drag's first move and is taken again where the
  button is let go.
- **Mesh grid** toggle (the faces' edges, 38% white, computed once per surface in a worker) and **Reset view** (top right
  once the view has moved).
- **Data only through the door**: `src/renderer/data/door.ts` is the one renderer module that names `window.studioData`
  (`tests/unit/data-door-static.spec.ts`). Every load carries the view's tag and a superseded or cancelled one is
  cancelled (G21); progress and errors are shown in the frame with Cancel or Try again.
- Point fields are refused with the reason: `data.surface` does not say which mesh points the surface uses (ADR 0801, addendum F-808, F808-D3).
- Tests: `tests/unit/field-colormap.spec.ts` (every entry and both ends against rcfd-render's map read from the Rust
  source; the full range against the tolerance), `field-surface.spec.ts`, `data-door-static.spec.ts`, and
  `tests/e2e/field.spec.ts` (pixels read back from screenshots of the packaged Studio equal the table colour of the
  stored value under them, by WebGPU and by WebGL 2; the hover readout equals the stored value; legend; grid; reset;
  keyboard; screenshots at 1440 x 900 in `test-results-visual/f808/`).
- `performance` marks: `rcfd:field:open` and the measure `rcfd:field:first-picture` (P7, `scripts/measure-field.mjs`).

## Derived pictures (F-809)

The field viewport's tool row has one pictogram per derived picture, named in its accessible name and tooltip (G22):
cut plane, vectors, iso-surface, streamlines (`src/renderer/field/derived.ts`, `DerivedControls.tsx`). One that does
not apply is a dashed chip that says why in its accessible name and in the tooltip its focus shows (G6), never as
standing text. What the engine computes was read first (ADR 0801, addendum F-809), and the tolerances were registered
there before the code (G3):

- **Cut plane**: rcfd-render's own cut (`View::clip`, only points with `n . p <= d` are drawn), on the GPU for the
  surface, the mesh grid and the arrows (`maskNode` of each TSL material), and in the pick, so the readout names what is
  seen. A plane normal to x, y or z: axis, position (a slider in mesh coordinates; the store gives no length unit) and
  the side kept, in a small panel in the frame's top left corner. Unavailable on a 2D case (one cell layer, the
  engine's own test of `render_field`).
- **Vectors**: the arrows of `render.png` (`rcfd_post::render::overlay_vectors`: a 24-per-axis lattice, the largest
  arrow 0.8 of a step, the head 0.3 back and half that out) carried to the boundary surface: one arrow per lattice bin,
  at the centroid of the first surface triangle in it, carrying its owner cell's stored components unchanged (on a 2D
  case the in-plane components, on the face the view looks at). Deep ink. The components come through the data door
  for the shown snapshot, with the view's tag.
- **Iso-surface, streamlines**: unavailable everywhere: the engine has no entry point for either on a stored field, and
  the Studio does not draw a picture it cannot check against the engine.
- Tests: `tests/unit/field-derived.spec.ts` (the 2D verdict against `render_field` on the quick-start and axial-fan
  runs; availability and reasons; the cut rule and the arrows' constants read from the Rust source; every arrow against
  an independent recomputation, components bit for bit the Rust reader's; the pictograms on the grid) and
  `tests/e2e/field-derived.spec.ts` (the packaged Studio: reasons by role and accessible name, tooltips on keyboard focus;
  the cut plane pixel by pixel on the tiny and 1e5-cell boxes; the arrows change only pixels to deep ink and their number
  is the recomputation's; keyboard; screenshots at 1440 x 900 in `test-results-visual/f809/`).
- Missing in the engine (no Rust change here): an iso-surface of a stored cell field, streamlines, and a store-based
  slice, sample or vector picture of a 3D case (`render_field` refuses 3D; `render.png`, `field.sample` and
  `render_image` work on live sessions only).

## Run control (F-807)

Runs are started, queued, paused, resumed, stopped and cancelled from the window, and only through the project's runner
(G7): the Studio asks the engine's project server (`start_run`, `pause_run`, `resume_run`, `cancel_run`), the server
writes the request into the project, and `rcfd-runner` acts on it. Closing or killing the Studio leaves every run going.

- Where: the run bar above the runs table (Start a run: the case, Run, Queue…, Start the runner); Run on each case of the
  project home (switches to Runs with the new run selected); and Run control in the selected run's panel (Pause or
  Resume, Stop… or Cancel…, Run again). Every control is reached and operated from the keyboard.
- Actions: `src/shared/actions/runs.ts` names each one (`project.run_case`, `project.queue_case`, `project.pause_run`,
  `project.resume_run`, `project.stop_run`, `project.cancel_run`, `project.start_runner`) with its parameters, when it
  applies (with the reason when it does not), its confirmation, and its MCP and `rcfd-runner` equivalents (G16), shown
  under each request. They are a registry of their own: the action registry of F-827 maps the service methods one to one.
- Availability: a control that cannot apply is an unavailable chip whose tooltip says why and what to do (Stop on an
  ended run: "there is nothing to stop. Run its case again for a new run."). The main process checks the same rules
  again before anything reaches the engine (`src/main/project/control.ts`).
- Confirmation (G18): Stop… and Cancel… open a confirmation strip in the panel that says what is lost (Stop: the steps
  not computed yet; the run cannot be resumed; results so far stay in `runs/<id>/`; a solver that does not stop within
  the runner's grace time is killed. Cancel of a queued run: no result is lost, only its place in the queue). The safe
  button has the focus; Escape keeps the run and returns the focus to the control.
- Feedback: the request shows in the run list at once (a note in the run's result cell: what was asked, when, and what it
  waits for), the new run of a start shows from `start_run`'s answer before the list holds it, and the runner's own state
  follows with the feed and the fast reads (every 100 ms for 5 s after an action, `RUN_BOOST_MS`).
- Refusals (G19): each says the problem (with the engine's own words when the engine refused) and the recovery: a case
  deleted outside the Studio, a run that ended before the request reached the runner, a run that is not there, a project
  server that does not start, a runner that does not start.
- No runner alive: the engine has no query for it, so a request the runner has not acted on after 3 s (`RUN_SLOW_MS`)
  says the runner may not be running and offers Start the runner (`rcfd-runner ensure --project <folder>`, the runner
  beside `rcfd-mcp`, never one from `PATH`). `start_run` itself starts a runner when none is alive.
- P2, measured (`tests/e2e/run-control.spec.ts`, written to `test-results-visual/f807/p2.json`): from the main process's
  engine answer to the renderer's state listener, under 1 ms (0.75 to 0.90 ms); to the run list's note in the DOM, 13 to
  19 ms; for run, queue, cancel, pause, resume and stop, on the maintainer's machine. The test holds the arrival to P2
  (20 ms) and the DOM commit to 50 ms.
- Tests: `tests/unit/run-control.spec.ts` (each action against the engine's sources and inventory, availability,
  confirmations, refusals, the session with a fake engine, the port, the run list's notes, the stylesheet),
  `tests/e2e/run-control.spec.ts` (against the real `rcfd-mcp` and `rcfd-runner` on a copy of the quick-start project
  holding one run at a time: each action by keyboard, a refused action in the window, past the page and by the engine,
  no runner alive, and G7; screenshots in `test-results-visual/f807/`).

Decisions taken in building F-807 (options, the recommendation followed; each reversible without touching the engine):

- D1, the route to the runner: (a) a second project server for the project, not read-only, started on the first action
  and used only for the four run tools; (b) run `rcfd-runner start|pause|resume|cancel` as child processes; (c) open the
  one project server without `--read-only`. Taken: (a). The reading server stays read-only, the requests are journalled
  by the engine (`rcfd-runner`'s own commands write no journal line), and the engine's error codes come back as they are.
- D2, quick run and queue: (a) Run is `start_run` with the case only, and the new run is selected and followed; Queue…
  is `start_run` with per-run priority, threads, memory and `--set` lines from a form under the run bar; both select the
  new run. (b) one action with an optional form. Taken: (a). Neither makes a plan (`store: none`), as the CLI does not.
- D3, stop and cancel: both are `cancel_run`; Stop… is offered for a launched run (it has a solver to stop) and Cancel…
  for one that has not started (queued, or held before launch), each with its own words.
- D4, a runner that is not alive: (a) mark a request after 3 s and offer Start the runner (`rcfd-runner ensure`);
  (b) start a runner before every action; (c) add an engine query (needs Rust; not here). Taken: (a).
- D5, confirmation: an inline strip in the run's panel (no dialog of `DESIGN.md` covers it yet), proposed for
  `DESIGN.md` with the pending-request note and the run bar.
- D6, the window's door: `window.studioRuns` (`dispatch(action, params)`) over the project port's new `action`
  operation; only `src/renderer/project/door.ts` names it, and `window.studioProject` keeps its methods.
- Coverage: `start_run`, `pause_run`, `resume_run`, `cancel_run`, `rcfd-runner start|pause|resume|cancel|ensure`,
  `--project` and the start options `--priority`, `--threads`, `--mem-mb` and `--set` have UI paths.
- Missing in the engine (no Rust change here): an MCP query of whether the project's runner is alive
  (`RunService::runner_running`), and an MCP tool that starts the runner alone (Start the runner runs `rcfd-runner
  ensure`, the only process the Studio starts for run control besides `rcfd-mcp`).

## The Claude Code panel (F-814)

An open project has a Claude Code panel on the right of its views (DESIGN.md Layout): the project's Claude Code
sessions, the transcript of the selected one, and a message field. A session another process has open (a terminal, the
VS Code extension, another Studio) is mirrored read-only as its transcript grows; a closed one is resumed with
`claude -p --resume <id>`; "New chat" starts one with `--session-id <uuid>`. Run ids and project files named in a reply
are chips that open the run in Runs or the file on Home. The panel hides to a slim strip and shows again. Surface brief:
`.impeccable/surfaces/web-src-renderer-claude-claudepanel-tsx.md`.

- Which claude: only the program named by `RCFD_STUDIO_CLAUDE=<path>` (a `.mjs`, `.cjs` or `.js` path runs on the
  Studio's own runtime). PATH is never searched. Unset, the sessions are still listed from their transcripts, read-only.
  Claude Code's home is `CLAUDE_CONFIG_DIR`, else `~/.claude`; the Studio's own records (folder confirmations, the MCP
  config it hands Claude Code) are in `RCFD_STUDIO_CLAUDE_STATE`, else `<userData>/claude`, never in the project.
- Sessions: the transcripts under `<home>/projects/<folder>/` and `claude agents --json` (every 2 s, `DISCOVER_MS`;
  the transcript every 500 ms, `MIRROR_MS`; the last 50 entries, `TAIL`). Open: listed with a live process. Closed: not
  listed. Unknown (the listing failed): read-only. Studio: answered by this Studio now.
- One writer per session (G11): the main process holds one lock for the whole turn, from before its checks to the
  process's exit; a second message is refused (`busy`). Right before every resume it lists again, so a session opened
  elsewhere since the last listing is refused (`read_only`). A running `claude -p` is listed by `claude agents --json`,
  so another Studio or terminal sees the session open.
- Spawn: from the main process, no shell, the project folder as the working folder, the message as one stream-json line
  on stdin (never in the argument list): `-p --output-format stream-json --verbose --include-partial-messages
  --input-format stream-json --strict-mcp-config --mcp-config <app-owned file> --permission-mode dontAsk
  --permission-prompts none --allowedTools <rule>... --resume|--session-id <id>`. `checkArgs` checks the whole list
  before every spawn and refuses any bypass or approve-everything flag, another permission mode, `--settings`,
  `--add-dir`, `--permission-prompt-tool` and malformed rules. A turn whose `system/init` does not report `dontAsk` is
  killed before any action. Stop kills the process tree.
- Tool approvals: the allow-list file `.rcfd/claude-allow.json` (`{"version":1,"rules":[...]}`) when valid (one bad
  rule refuses the whole file and no chat starts), else the conservative first list: the 15 read-class tools of the
  engine's project server (`mcp__rcfd_project__project_info`, `list_runs`, `run_status`, `read_metrics`,
  `read_residuals`, `field_summary`, `render_field`, `compare_runs`, `list_changes`, `read_project_file`, `wait_run`,
  `read_feed`, `check_case_mesh`, `preview_mesh`, `list_meshes`). Everything else is denied without asking; a denied
  call shows the narrowest rule that would allow it.
- The project server Claude Code is given: `rcfd-mcp --project <folder> --root <folder>` (`RCFD_STUDIO_MCP`), in an MCP
  config the Studio writes in its own state folder; with `--strict-mcp-config` no other server is loaded. Without
  `RCFD_STUDIO_MCP` the config lists no server.
- The folder's Claude Code control files (`.mcp.json`, `CLAUDE.md`, `.claude/`, `.claude-plugin/`, the allow-list file)
  can run code or widen permissions, so the first chat in a folder that has them asks to confirm them by name and hash;
  a change asks again.
- The page sees `window.studioClaude` (`state`, `onState`, `offState`, `select`, `send`, `cancel`, `confirm`,
  `refresh`) over a MessagePort; the main process checks every message (`src/main/claude/bridge.ts`): exact fields, a
  session that is null or a UUID, a message of at most 20,000 characters. The page cannot name a program, a flag or a
  path.
- Tests: `tests/unit/claude.spec.ts` (configuration, allow-list and argument check, readers, discovery, port, links, the
  panel driven against the fake claude, stylesheet) and `tests/e2e/claude.spec.ts` (packaged app, quick-start copy,
  screenshots in `test-results-visual/f814/`). Both use `tests/support/fake-claude.mjs`, which records its argument
  list and replays a canned stream; no test starts a real claude.

Decisions taken in building F-814 (options, the recommendation followed; each reversible without touching the engine):

- D1, session discovery: (a) port the discovery, transcript and allow-list logic of `rcfd-claude` to the main process
  as a stopgap; (b) wait for an engine entry point. Taken: (a); the engine has none (see below).
- D2, which claude: (a) `RCFD_STUDIO_CLAUDE` only, as F-805 takes `RCFD_STUDIO_MCP`; (b) search PATH. Taken: (a), which
  also keeps automated tests off a real CLI; packaging the CLI path is F-820's.
- D3, placement: (a) the right column of the workspace, hideable (DESIGN.md Layout); (b) a third tab. Taken: (a).
- D4, folder confirmation: ported from `rcfd-claude` (names and sha256, recorded in the Studio's state, not the folder).
- D5, approvals: the allow-list file if valid, else the first list; a denied call shows its rule, but the Studio does
  not write the allow-list file (it writes nothing into the project, G12; that needs an engine entry point).
- D6, a file link: opens Home filtered to the file, until the case editor (F-812) can open it.
- D7, a new chat: allowed, with `--session-id` and a UUID chosen by the main process.
- D8, MCP config: always `--strict-mcp-config` with an app-owned config (no server when there is no `rcfd-mcp`).
- Missing in the engine (no Rust change here): a CLI, MCP or service entry point for Claude Code session discovery,
  transcript reading, resume and writing the allow-list file (`rcfd-claude` has the code; its only user, the egui Studio
  `rcfd-studio`, was removed by F-449).

## Layout

| Path | Process | Built by | Filled in by |
|---|---|---|---|
| `src/main/` | Electron main process (ES module): the window, the `app://` protocol, the session's security settings, the IPC handlers; later all service and file I/O | `tsc -p tsconfig.node.json` -> `dist/main/` | F-801 (hardened shell), F-803 (service client), F-804 (bulk data), F-805 (project folder) |
| `src/main/service/` | The service client (no Electron import, unit-tested in Node): `rpc.ts` (framing, ids, time limits), `client.ts` (connections, events loop, reconnection), `spawn.ts` (starting and finding `rcfd serve`), `config.ts`, `bridge.ts` (the checks on the service port) | as above | F-803 |
| `src/main/security.ts` | The security policy as values and pure functions (CSP, own-URL and network rules, protocol path resolution), unit-tested without Electron | as above | F-801 |
| `src/preload/` | Preload script (CommonJS `.cts`, because a sandboxed preload cannot be an ES module): `window.studio`, the IPC allow-list | `tsc -p tsconfig.node.json` -> `dist/preload/` | F-801, F-803 |
| `src/shared/ipc.ts` | The typed IPC contract shared by all three processes: channel names, arguments, replies | `tsc -p tsconfig.node.json` -> `dist/shared/` | F-801 |
| `src/main/project/` | The project home (no Electron import, unit-tested in Node): `server.ts` (the engine's project server over MCP), `session.ts` (one open folder: lists, feed, liveness), `home.ts` (recent list, opening, failures), `listing.ts` (case and note names, D2), `recent.ts`, `config.ts`, `bridge.ts` (the checks on the project port) | `tsc -p tsconfig.node.json` -> `dist/main/` | F-805 |
| `src/shared/project.ts` | The project port's contract: what the home shows, the port messages, `window.studioProject` | as above | F-805 |
| `src/main/claude/` | The Claude Code panel (unit-tested in Node; only `port.ts` imports Electron): `config.ts` (which claude), `allow.ts` (rules, the allow-list file, `checkArgs`), `discover.ts`, `transcript.ts`, `stream.ts`, `trust.ts` (control-file confirmation), `driver.ts` (spawning `claude`), `panel.ts` (one project's sessions, the writer lock), `bridge.ts` and `port.ts` (the Claude port) | `tsc -p tsconfig.node.json` -> `dist/main/` | F-814 |
| `src/shared/claude.ts` | The Claude port's contract: what the panel shows, the port messages, `window.studioClaude` | as above | F-814 |
| `src/shared/service.ts` | The service port's contract: the allow-list of service methods, the port messages, `window.studioService` | as above | F-803 |
| `src/renderer/` | Renderer (browser code only, no Node types): React, three.js, React Aria Components | `vite build` -> `dist/renderer/` | F-802 onwards |
| `src/renderer/design/` | `tokens.ts`, the one source of the `DESIGN.md` tokens (colour roles, type ramp, spacing, sizes, lines, focus, motion, pictogram grid); `tokens.css`, generated from it | as above | F-802 |
| `src/renderer/ui/` | The primitives of `DESIGN.md` Components on React Aria Components: buttons, toggles and segmented toggles, text and number fields, select, switch, tabs (sub and project), table, slider, tooltip, state pictograms and icons; `primitives.css`; `onDemand.ts`, three.js drawn on demand (F-817b) | as above | F-802 |
| `src/renderer/project/` | The project home (`ProjectHome.tsx`, `project.css`) and its one door to `window.studioProject` and `window.studioRuns` (`door.ts`) | as above | F-805, F-807 |
| `src/renderer/runs/` | The Runs view (`RunsView.tsx`, `runs.css`, F-806) and run control (`RunControls.tsx`, `control-model.ts`, `control.css`, F-807) | as above | F-806, F-807 |

| `src/renderer/project/` | The project home (`ProjectHome.tsx`, `project.css`) and its one door to `window.studioProject` (`door.ts`) | as above | F-805 |
| `src/renderer/commands/` | The command palette (`CommandPalette.tsx`, `palette.css`, loaded on first use), its key listener (`PaletteHost.tsx`), what each command does (`run.ts`) and the surfaces' handlers (`host.ts`); `src/shared/commands/` holds the palette's model and the shortcut table | as above | F-831 |
| `src/renderer/claude/` | The Claude Code panel (`ClaudePanel.tsx`, `claude.css`), reference links in a reply (`links.ts`) and its one door to `window.studioClaude` (`door.ts`) | as above | F-814 |
| `src/renderer/dev/` | The primitives gallery at `#/dev/primitives` (loaded on demand): every primitive and the tokens, with sample labels only | as above | F-802 |
| `src/renderer/settings/` | The case settings surface at `#/settings/<session>` (loaded on demand): `schema.ts` (the model of `settings.schema`, value encoding), `disclosure.ts` (learner-first tiers, search), the page, the surface, key rows and groups, `settings.css` | as above | F-825 |

| `src/renderer/plots/` | The residual plot (log scale) and the monitor chart (F-811): `feed.ts` (one run's `monitors.get`, whole once, then increments), `series.ts`, `range.ts` (axes that hold every value drawn), `linePlot.ts` (the lines on three.js `WebGPURenderer`), `PlotFrame.tsx` and the two plots; `src/renderer/dev/PlotsDev.tsx` shows them at `#/dev/plots` | as above | F-811 |
| `src/shared/monitors.ts` | The `monitors.get` reply packed into `Float64Array`s in main before it crosses the port (F-811) | `tsc -p tsconfig.node.json` | F-811 |

| `src/renderer/field/` | The field viewport: `FieldViewport.tsx` (loading through the data door, legend, caption, readout), `FieldCanvas.tsx` (WebGPURenderer, the TSL material, pick, grid, keyboard, P8 bench), `colormap.ts`, `surface.ts`, `grid.worker.ts`, `Legend.tsx`, `field.css`; its development page is `src/renderer/dev/FieldDev.tsx` at `#/dev/field` | as above | F-808 |
| `src/renderer/data/` | `door.ts`: the renderer's one door to `window.studioData` (F-804's bulk data) | as above | F-808 |
| `src/renderer/workspace/` | The run workspace (`RunWorkspace.tsx`, `TimePanel.tsx`: run bands, scrubber, residual plot, monitor), `model.ts`, its one door to `window.studioRunData` (`door.ts`), `workspace.css` | as above | F-810 |
| `src/main/run/`, `src/shared/run.ts` | The run port: `bridge.ts` (the checks), `series.ts` (a run's residual and monitor series and their units through the project server) | `tsc -p tsconfig.node.json` | F-810 |
| `src/shared/compare.ts`, `src/renderer/workspace/Compare.tsx` | Comparing two runs: the rules (same time, same mesh, difference, one range), the side-by-side and difference views, the metrics table (`compare-model.ts`, `compare.css`) | as above | F-815 |

| `src/renderer/first/` | A learner's first path: `FirstPath.tsx` (the intro, the not-a-project section, the four steps), `model.ts` (the step from the engine's lists), `words.ts` (disclaimer, limits, help, precise terms), `target.ts` (the result link), `first.css`; its run action is `src/shared/actions/first.ts` | as above | F-813 |

| `src/renderer/provenance/`, `src/main/provenance/`, `src/shared/provenance.ts` | The provenance inspector: the trigger and popover (`Provenance.tsx`), which number Show provenance opens (`registry.ts`), each number's facts (`sources.ts`), its one door to `window.studioProvenance` (`door.ts`), `provenance.css`; in main the port's checks (`bridge.ts`) and the run record read through the project server (`record.ts`); the reproduce command (`shared/provenance.ts`) | `vite build`, `tsc -p tsconfig.node.json` | F-828 |
| `tests/unit/` | Pure tests of `security.ts` and the built `index.html` (F-801); tokens against `DESIGN.md`, the pictogram grid (F-802); the service client against a fake service (F-803) | run by Playwright (project `unit`) | F-801, F-802, F-803 |
| `tests/e2e/` | The packaged shell driven by Playwright's `_electron.launch`; the primitives (F-802); the service port's allow-list against a fake service, and the client against the real `rcfd serve` (F-803) | run by Playwright (project `electron`) | F-801 onwards |
| `tests/support/` | The fake service, finding and starting `rcfd`, launching the packaged Studio | imported by the tests | F-803 |
| `scripts/` | `package-dir.mjs` (packaging by hand), `measure-cold-start.mjs` (P10), `measure-service.mjs` (P1, P2), `write-tokens-css.mjs`, `open-gallery.mjs`; `bench.mjs` (F-817: the G9 bench over the `measure-*.mjs` scripts) | run by Node | F-801, F-802, F-803, F-817 |

`out/`, `dist/`, `test-results/`, `test-results-visual/` and `node_modules/` are build output and never committed.

## Design tokens and primitives (F-802)

- The values come from `DESIGN.md` and `.impeccable/design.json`; `tests/unit/tokens.spec.ts` checks them against both,
  checks that `tokens.css` is exactly `tokens.ts`'s output, and that the stylesheets use only defined tokens and no
  colour literals. Components use roles (`--role-text`, `--role-focus-ring`), never palette names.
- The product tokens equal `DESIGN.md`; where `DESIGN.md` is silent (text selection, scrollbar) they take the
  prototype's values (`apps/studio-prototype/src/styles.css`). Focus is DESIGN.md's 2px orange outline on every
  focusable element, inset on tabs, table rows and cells, list options and panels.
- `tests/e2e/primitives.spec.ts` drives the gallery from the keyboard (focus visible, operable, role and name) and
  checks that the page raised no CSP violation or console error, then writes screenshots at 1440x900 to
  `test-results-visual/f802/` for the maintainer's view-and-report.
- React Aria's `usePress` would inject a one-rule `<style>` element, which the CSP refuses; `main.tsx` puts a marker
  element with its id in the head and the rule lives in `base.css`.

## Capability coverage (F-824, G15)

- `capabilities/inventory.json` lists every capability the engine reports, with a stable id and its kind: the methods
  of `service.info` and the keys of `settings.schema` (from `rcfd serve --stdio`), the MCP tools of `rcfd-mcp` with and
  without `--project`, and the commands and options of the `--help` of `rcfd`, `rcfd-runner` and `rcfd-mcp`. It is
  generated by `npm run capabilities` (`scripts/write-capabilities.mjs`, `tests/support/capabilities.ts`), never edited.
- `capabilities/coverage.json` is kept by hand: each capability, by id or by a prefix for one case section or one
  command's options, gets a UI path or a dated exemption naming the roadmap item expected to cover it.
- `tests/unit/capabilities.spec.ts` fails on an unmapped capability, a malformed or unused rule, a stale
  `docs/studio-coverage.md` (the generated report) and an inventory that differs from the engine; that last test needs
  `rcfd` with `rcfd-runner` and `rcfd-mcp` next to it and is skipped, with the reason, when no `rcfd` is found.

## The action layer (F-827)

Every UI action is a named entry of one registry, `src/shared/actions/registry.ts` (G16): a stable id, a title and a
description (the command palette of F-831 and help on the spot, G19), a parameter schema (`params.ts`: the TypeScript
type, a JSON Schema and the shape check), the engine call (a method of `SERVICE_METHODS`, sent the action's parameters
unchanged), its CLI equivalent (`rcfd call --addr <service> <method> '<json>'`) and MCP tool where the MCP server has one,
an availability predicate with the reason in words, and an undo hook for F-829. Today it holds one action per method of
the allow-list (24).

- The dispatcher (`src/shared/actions/dispatcher.ts`) checks the parameters and the availability, makes the call, and
  logs it: every change kept (up to 10 000), the latest 500 reads in a ring of their own. The log exports as a JSON
  script (`rcfd-studio-actions` 1) that `replay` runs again, mapping the sessions it made, or as a POSIX shell script of
  `rcfd call` commands.
- In the renderer, `src/renderer/actions/index.ts` is the one module that touches `window.studioService`; surfaces
  call `dispatch('run.start', { session })` and `availability(...)` from there. `tests/unit/actions-static.spec.ts`
  fails when any other renderer file names the bridge or calls a service method by name.
- Tests: `tests/unit/actions-registry.spec.ts` (every action has an engine equivalent, read from the engine's sources;
  every allow-listed method has an action; schemas, types, availability, equivalents), `actions-dispatch.spec.ts`
  (dispatch, log, export and replay against the fake service through the port bridge), and
  `tests/e2e/actions-rcfd.spec.ts` (the real `rcfd serve`: a recorded session replayed from JSON and from the shell gives
  the same journal hash and run status; the undo hooks restore what was there).

## Case settings (F-825)

The first surface: `#/settings` (from the empty project view, "Case settings of a service session"), then
`#/settings/<session>`. Every key of the engine's `settings.schema` gets a control made from its schema node at run time
(`src/renderer/settings/schema.ts`): a switch for a boolean, a select for an enumeration, a text field for numbers,
strings and lists, a JSON field for any shape a later schema adds; its unit (`x-rcfd-unit`, or the SI dimension
`x-rcfd-si-dimension`), range, default and description are the schema's. There is no list of keys in the Studio
(`tests/unit/settings-static.spec.ts` fails on a string that names a case section or key path).

- Learner-first disclosure (`disclosure.ts`, PRODUCT Principle 5) comes from the schema's structure until F-826 adds an
  advanced flag: sections in the engine's declaration order; in each, the first 8 own keys (`COMMON_KEYS`), then "Show N
  more keys"; nested objects, optional objects and named collections one step away; every key that differs from its
  default always shown, and listed under "Set in this case"; search and "Show every key" reach everything.
- Edits are actions (`dispatch('settings.set' | 'settings.remove' | 'settings.validate' ...)`). The Studio refuses no
  value: a number is sent as a number, other text as text (the engine reads `36 km/h` or refuses), and the message of a
  refused change or of the check is the engine's, shown as it came. The last change shows its `rcfd call` command.
- Events: a `setting_changed` of the session, from the Studio or another writer, reloads the tree; a running session makes
  the controls read-only with the action layer's reason.
- Tests: `tests/unit/settings-schema.spec.ts` (the model from a test schema, a key and a section added with no code
  change, encoding, disclosure; every key of the real schema against the F-824 inventory), `tests/e2e/settings.spec.ts`
  (the packaged Studio against `tests/support/fake-case-service.ts`: controls, an added key, the service's words,
  the check, live changes, read-only, keyboard and focus, screenshots) and `tests/e2e/settings-rcfd.spec.ts` (the real
  `rcfd serve`: every one of the schema's keys has exactly one control in the window). Screenshots at 1440 by 900:
  `test-results-visual/f825/`.

## Residual and monitor plots (F-811)

- One feed per run (`src/renderer/plots/feed.ts`, keyed by service instance and session): the first `monitors.get` is
  whole, every later one asks `since` the last abscissa held (ADR 0353), checks the samples it already holds that come
  back, and appends the rest; a mismatch (a run rebuilt) reads the whole history again. The run's events wake it; nothing
  polls on a timer. A plot only ever draws the snapshot of the run it was opened for.
- The range rule (G3): every axis range holds every value drawn (`range.ts`); a value that cannot be drawn (not finite,
  or not above zero on the log scale) breaks the line and is counted under the plot, never clamped onto an edge.
- A run without series says so in words with its state pictogram (`parts.tsx`, `words.ts`).
- The lines are three.js (`WebGPURenderer`, WebGL2 where WebGPU is missing); ticks, names and values are HTML. The choice,
  the packing of the reply and P3 through the shell are in ADR 0801 addendum 3.
- Development route: `#/dev/plots?session=<id>` (and `&case=cases/<file>.toml` for its "New session" button) against a
  service named by `RCFD_STUDIO_SERVICE` or started from `RCFD_STUDIO_RCFD` with `RCFD_STUDIO_SERVICE_ROOT`. F-810 places
  the plots in the time panel.
- Tests: `tests/unit/plots-feed.spec.ts` (increments exact against `tests/support/fake-monitors.ts`, two runs never
  mixed), `plots-range.spec.ts`, `plots-css.spec.ts`; `tests/e2e/plots.spec.ts` against the real `rcfd serve` with the
  quick-start cavity, screenshots at 1440x900 to `test-results-visual/f811/`.

## The case editor (F-812)

From the project home, "Edit" on a case row (or "New case" in the header band) opens `#/case/...`: the F-825 surface on
a service session made from the file, with the file named as a white tab in the header band and a case file panel at
the head of the right column. Surface brief: `.impeccable/surfaces/apps-studio-web-src-renderer-case.md`.

- Open: the main process reads the file's hash through the project server (`read_project_file`); the renderer dispatches
  `session.create {path}` (an action, logged and replayable, so the engine parses and validates the file with its own
  schema); the main process then binds the session to the file only when its journal names that file as its source and
  the hash is unchanged. A file the engine refuses opens nothing and shows the engine's diagnostics (key, problem) with
  the fix.
- Edit: the F-825 surface unchanged (learner first, every key one step away, search, "Show every key").
- Save (Ctrl+S, or the Save button): the settings set since the last save, read from the session's journal, with the
  session's current values, written by the engine's `edit_case` against the hash read (`base_hash`), in place: other
  lines and comments of the file stay. The panel names the keys written and the form. A file changed by another writer
  meanwhile is not overwritten: the panel shows each key with the value here and the value in the file now, and three
  named choices (write my changes into the file as it is now; load the file as it is now, dropping mine; keep
  editing), none preselected. The editor reads the file's hash again when the engine's change feed names it, so
  "Changed outside" shows before a save.
- Case actions: New case (from the engine's templates, the shipped examples one step away, both lists from the
  project server's `new_case` schema), Duplicate as (byte-for-byte copy, `new_case {copy}`), Check the mesh (the
  `mesh.check` action), Check the saved file (`check_case_headless`, the check-only launch with its exit code), Close
  the session (with unsaved changes it asks first, names them, and offers Save and close: F-837's close question). Each shows its Claude Code and CLI equivalents
  (`CASE_ACTIONS` in `src/shared/case.ts`).
- The page reaches the case operations only through `window.studioCase` (`state`, `sources`, `prepare`, `attach`,
  `status`, `conflict`, `save`, `create`, `duplicate`, `check`, `forget`), from one module, `src/renderer/case/door.ts`;
  a third MessagePort on `studio.case-port`, checked by `src/main/case/bridge.ts` (exact fields, paths `cases/<...>.toml`
  or `.json` without `..`). The ipcMain channels are `app.info`, `app.ready` and (F-837) `app.closeAnswer`.
- Tests: `tests/unit/case-editor.spec.ts` (fake service and project server: what a save writes, conflicts, removals,
  refusals, bindings, the allow-list, the port's checks; no case key in the editor's code; tokens-only CSS) and
  `tests/e2e/case-editor.spec.ts` (the packaged Studio against `rcfd serve` and `rcfd-mcp`: open from the home, a key
  saved and read back with `rcfd run <case> --print-settings` and `rcfd validate`, a conflict with nothing overwritten
  and each choice, an invalid file, new case, duplicate, checks, close, the keyboard path). Screenshots at 1440 by 900:
  `test-results-visual/f812/`.

Decisions taken in building F-812 (options, the recommendation followed; each can be reversed without an engine change):

- D1, the write path: (a) a second project server per open project, `rcfd-mcp --project <folder>` without
  `--read-only`, started on the first case operation, called only with the tools of `CASE_TOOLS` (`read_project_file`,
  `edit_case`, `new_case`, `check_case_headless`; `src/main/case/server.ts` refuses the rest before the server); (b)
  drop `--read-only` from the project home's server; (c) a new save method in `rcfd serve` around the workspace's
  `save_case` (a Rust change). Taken: (a). The project home's server stays read-only. Every write is the engine's, hash
  guarded, with a line in `.rcfd/edits.jsonl` (reason `rcfd Studio case editor: save`, `...: new case`, `...: duplicate`).
- D2, the action registry: the registry (F-827) holds one service method per action and its test requires exactly the
  service's methods, so the case-file actions (which combine project-server tools and service methods in the main
  process) are a separate table, `CASE_ACTIONS`, as the project home's operations are (F-805 D7). Proposed: let the
  registry hold actions whose engine side is a project-server tool.
- D3, opening: the session is made by the renderer through the dispatcher (`session.create {path}`), and the main process
  binds it after checking the journal's source and the hash; a file changed between the two is opened again (up to
  three times), then reported.
- D4, what a save writes: the leaf settings set since the last save, with the session's values; an entry removed in the
  session that the file holds cannot be written (`edit_case` sets keys; it does not remove), so the save is refused with
  the entry named and the way out (remove it in a text editor or ask Claude Code, then open the file again). An entry
  added and removed again since the last save writes nothing.
- D5, conflicts: never resolved for the user; no choice preselected; "write my changes" writes only the keys listed onto
  the file as it is now and then reopens the session from the result.
- D6, the sessions made in the main process to read the file as it is now (for the conflict table and the removal
  check) are short-lived, closed at once, and bypass the action log, which records the user's actions only.

## Undo, safe deletes and edit conflicts (F-829, G18)

- Undo and redo (`src/shared/actions/history.ts`): every edit dispatched through `src/renderer/actions/` that succeeds
  and has an undo hook in the registry becomes a step of its session's history: `settings.set`, `settings.remove`, a
  named entry made from its first key (the settings surface gives the inverse, removing the entry it made, because the
  engine has no old value), and a new session made from the case settings page. Undo dispatches the registry's inverse
  (origin `undo`), redo the step's own call (origin `redo`), so both are in the action log and replayable. Opening a
  case file is not a step (`dispatch(..., { undo: false })`); Close the session is the way back.
- Scope: one history per session. The case settings page (`#/settings/<id>`) and the case editor (`#/case/<id>`) show
  the history of the session they edit; a closed session takes its history with it; a restarted service clears every
  history. Redo of a new session makes a new session, and the history (and the page) follow its new id. A case save is
  the file's own step: undo changes the session, never the file, and a save after an undo writes the restored value
  (hash guarded as before). At most 200 steps per session.
- Controls (`src/renderer/history/`): two ghost buttons in the header band that name the step ("Undo: set
  solver.alpha_u", "Redo: remove boundaries.lid"; when there is none, an arrow alone named "Nothing to undo", F-833), Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y
  (Cmd on macOS) from anywhere on the page. Inside a text field that holds typing not yet sent, Ctrl+Z is the field's own
  undo of that typing; once the value is sent (Enter, or focus leaves) or dropped (Escape) it undoes the last edit.
- Conflicts: before a step is undone or redone, the settings it left are read again; a value another writer (Claude
  Code, the CLI, another window) changed since is shown with both values and three choices (undo anyway, replacing the
  value now; keep the value now and drop this step; cancel), none preselected, and nothing is sent until one is chosen.
  A typed value is checked the same way before it is sent: when the engine's value changed after the typing began,
  the field says so and offers "Set mine" or "Keep" the other value. The F-812 case save already refuses a file changed
  since it was read (`edit_case` with `base_hash`, conflict 1200) and was checked, not changed; `new_case` and duplicate
  never replace a file. The recent list (user data) is read again before each write and the one change applied to it,
  so another window's entries stay.
- Losses (`src/shared/actions/loss.ts`): every change action has an inverse or words for what it loses
  (`NO_INVERSE`; a unit test fails on one with neither). Close session (case settings page) and undoing a new session
  ask first, in words that name the session, its settings that exist only in the service (with the edits made here),
  its built mesh and fields and its results, with keeping as the focused default. An undo or redo of a settings change
  on a session that holds results (an ended run, or steps taken) asks first too: the engine discards them. The words
  for `run.start`, `run.step` and `run.initialize` on a session with results are there for run control (F-807).
- Deletes: the engine has no delete or trash of a run, a result or a case (no service method, MCP tool or CLI command;
  the F-824 inventory holds only `settings.remove` and `workflow.delete`, which touch no file). The Studio offers none
  and never deletes a project file itself (G2, G12): `tests/unit/history.spec.ts` fails when the engine gains one (so
  the Studio can offer it) and when Studio code removes a file other than its own service record.
- Tests: `tests/unit/history.spec.ts` (fake service: round trips, conflicts on undo and redo, losses, the loss table,
  rename on redo, no delete path, the recent list, history.css), `tests/e2e/undo-rcfd.spec.ts` (the same history on the
  real `rcfd serve`, another writer on a second connection, results from `run.step`), `tests/e2e/undo.spec.ts`
  (packaged Studio, fake service: buttons, keyboard, conflicts, typed-value conflict, confirmations, focus) and
  `tests/e2e/undo-case.spec.ts` (packaged Studio, `rcfd serve` and `rcfd-mcp`: the case editor's history). Screenshots
  at 1440 by 900: `test-results-visual/f829/`.

Decisions taken in building F-829 (options, the recommendation followed; each can be reversed without an engine change):

- D1, scope: (a) one history per session, (b) one per page, (c) one per surface. Taken: (a); a new session belongs to
  the history of the session it made; opening a case file is not a step.
- D2, conflict checks: the service has no compare-and-set, so (a) read again and compare before undo, redo and a typed
  value is sent (a gap of a few milliseconds between read and write), (b) an engine `settings.set` that takes the
  expected value (Rust, proposed), (c) none. Taken: (a), (b) proposed.
- D3, deletes: (a) none in the Studio until the engine has one, the gap recorded and tested; (b) a Studio-side trash
  that moves files (refused: G2, G12). Taken: (a); proposed engine entry points: project tools to move a run or a case
  to a trash under `.rcfd/` with a journal line, list it, restore and empty it.
- D4, Ctrl+Z in a text field: the field's own while it holds typing not yet sent, the Studio's otherwise.
- D5, a named entry made from its first key: the surface passes the inverse (remove the entry) when the name was new;
  the registry's inverse stays as F-827 tested it.
- D6, the recent list: merged (read again, one change applied), not a conflict prompt: an entry is a path and a time.
- D7, the decision panel's focus: shown always (the panel moves focus itself, also after Ctrl+Z, which does not count as
  keyboard use for focus-visible).
- Open: whether an ordinary settings change on a session that holds results should also ask first (F-825 sends it at
  once; undo and redo ask). Options: (a) ask once before the first change that discards results, (b) keep as is.
  Recommended (a); not done here, it changes F-825's surface.

## The command palette and keyboard shortcuts (F-831)

Ctrl+K (or Ctrl+Shift+P, or "Commands" in the header band) opens the command palette on every page: every action of the
registries and every Studio command, found by its name or its id, with its shortcut and its CLI and Claude Code
equivalents (G20). Ctrl+/ (or "Show the keyboard shortcuts" in the palette) lists every shortcut by where it works.

- The model (`src/shared/commands/palette.ts`) is generated from the registries, so nothing is left out: `ACTIONS`
  (service methods, F-827), `RUN_ACTIONS` (run control, F-807), `CASE_ACTIONS` (case files, F-812) and `STUDIO_COMMANDS`
  (`studio.ts`: going to Home, Runs, Case settings, the case editor and the Claude Code panel; the project folder of
  F-805; the Claude Code panel's operations of F-814). The Studio commands make no result of their own (G16): a view
  change has no engine call, and each says what it uses.
- Search ranks exact title or id, then prefix, then the start of each word, then contained text, then a fuzzy
  subsequence. Nothing typed lists every command by group, the frequent ones first.
- An unavailable command stays in the list with its reason and the way out (G19); Enter on it shows the reason with the
  command's facts. It is not disabled, so the arrow keys reach every command.
- A value a command needs is chosen from what the engine or the main process lists: a session from `session.list`, a
  case or a run from the project's lists (the run selected in Runs first), a folder from the recent list, a Claude Code
  session from the panel's list. Each choice shows why the command cannot run on it, when it cannot. The other
  parameters are a form made from the registry's schema; left empty, the engine's default applies. Nothing is made up.
- An action whose form exists opens it: New case opens the case editor's form, Duplicate the case puts the focus in its
  "Duplicate as" field, Go to Claude Code in the message field. Stop… and Cancel… of a run ask in the run actions' own
  words, the safe choice focused (G18). A service action shows the engine's reply and the same request as
  `rcfd call` and, where it has one, the MCP tool.
- Commands that need a surface's state (Go to Runs, Save the case) are run by that surface: while mounted it registers a
  handler (`src/renderer/commands/host.ts`, `useCommandHandlers`); with none, the command is unavailable and says what
  to open first.
- One shortcut table, `src/shared/commands/shortcuts.ts`, feeds the palette, the listing, the key listener and the
  tests. Global: Ctrl+K and Ctrl+Shift+P palette, Ctrl+/ shortcuts, Ctrl+1 Home, Ctrl+2 Runs, Ctrl+L Claude Code message
  field, Ctrl+O open a project folder, Ctrl+N new case, F5 run a case (asks which), Ctrl+. stop a project run (asks
  which, then confirms). F5 and Ctrl+. were the removed egui Studio's Run and Stop keys. The table also lists the keys of the
  surfaces (Ctrl+S in the case editor, Enter in the Claude Code field, the field view's keys) and the reserved keys:
  the Electron default menu (reload, developer tools, zoom, clipboard, close), Chromium's text and focus keys, Windows'
  menu keys, Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y for undo and redo (F-829) and the time panel's arrows, Home and End (F-810).
- Escape closes the palette from any step and the focus goes back where it was (React Aria's modal). The palette is
  loaded on first use, so the project home's cold start (P10) does not carry it.
- Tests: `tests/unit/commands-palette.spec.ts` (every registered action is in the model once, G20; each is found first by
  its title and by its id; equivalents; choices; the stylesheet keeps to the tokens), `tests/unit/commands-shortcuts.spec.ts`
  (no conflict within a scope or across scopes live together; no F-831 key takes a reserved one; F-829 and F-810 keys
  reserved; the keys the surfaces bind are the table's; chords on any layout) and `tests/e2e/commands.spec.ts` (the
  packaged Studio from the keyboard alone: open, find and run, Escape gives the focus back, every command reached with
  the arrows, arguments asked, navigation, the listing, the running application menu's keys all reserved, F5 does not
  reload). Screenshots at 1440 by 900: `test-results-visual/f831/`.

Decisions taken in building F-831 (options, the recommendation followed; each reversible without touching the engine):

- D1, the source of the palette: (a) generate it from the four registries as they are, with the Studio's own commands
  in a fifth table; (b) fold every action into `ACTIONS` (its test requires exactly one action per service method, an
  existing test). Taken: (a).
- D2, the palette keys: Ctrl+K, with Ctrl+Shift+P as an alias: neither is bound by Electron's default menu (read from
  the running app by the e2e test) and Electron has no browser keys. Ctrl+K stays free in text fields on Windows.
- D3, F5 and Ctrl+.: (a) ask which case or run, the run selected in Runs offered first; (b) act on the case open in the
  editor or the selected run directly (a run of the saved file while edits are unsaved, or a stop without a choice).
  Taken: (a).
- D4, unavailable commands: listed, focusable and explained, not disabled (a disabled option is skipped by the arrows).
- D5, look: a floating overlay with the popover lift and an ink edge, no scrim; key caps as square silver hairline chips;
  proposed for `DESIGN.md` with the listing.
- D6, coverage: the service-session run methods and `log.tail`, whose exemptions said no item plans a surface, now
  have a UI path through the palette (`capabilities/coverage.json`); the methods a planned item will show
  (`field.stats` F-808, `reports.get` F-811, `service.info` and `journal.get` F-828, `panel.snapshot` F-810) keep their
  exemptions.
- D7, a pointer path: "Commands Ctrl+K" in the project home's header band; the case editor's and settings' headers do
  not have it yet.

## A learner's first path (F-813)

From no project, or an empty folder, to a first honest result with the quick-start example (the engine's built-in
template `lid_driven_cavity`, the case of `docs/tutorials/quick-start-cli.md`). The path lives on the project home
(`src/renderer/first/`): with no project open, a station strip of four steps (Make the example, Run it, Watch it work,
See the result); a folder the engine refuses as no project gets a short "not a project yet" section beside the failure;
an open project that holds nothing but the example's case and runs shows the path above its index until the learner
makes a case of their own (or hides it).

- Make the example: the engine's `new_case {name: "first-cavity", template: "lid_driven_cavity"}` through the case door
  (F-812), offered only when `case.sources` lists the template.
- Run it: `first.run_example`, the engine's `start_run {case, store: "plan"}` through run control (F-807; CLI
  equivalent `rcfd-runner start <case> --project <folder>` plus the plan's `--set` entries, which the engine answers
  and the help shows). The plan stores the fields about 100 times in single precision, so the result has a picture.
- Watch it work: the run's pictogram and round count; "Start the runner" when the runner has not picked the run up.
- See the result: the engine's numbers (rounds, solver time, the case's `lid_drag` report with its full value on
  hover), the stored field of the last snapshot drawn by the field viewport (F-808) with a plain-word field toggle,
  the limits and the README's disclaimer, "Open the run in Runs" and "Edit the case".

The Quiet Rule (G22): at rest the path shows the stations, one action and the numbers. What a step is and does, where
its full control lives (G15), the precise terms with their sources (G19), the engine's words and the same run from the
command line (G17) open under "Explain" (help on demand), and run states are pictograms whose plain word, precise word
and outcome are their accessible name and tooltip. Visible sentences are kept for the exceptions: the folder that is not
a project, failures, a runner that has not picked the run up, a run with no stored pictures, and the result's limits and
disclaimer (G6). Tests read them by role and accessible name. Focus moves to each new step's action when the focus was
on the path, so the walk works by keyboard alone.

Nothing on the path writes to a project folder or keeps project state (G2, G12): its step is read from the engine's
lists every time (`model.ts`), and the result link is one function, `openFirstResult` in `target.ts` (it opens the run in
Runs today; repoint it there when F-810's run workspace lands).

Missing engine entry point: no CLI, MCP or service call makes a project in an empty folder (only the library's
`rcfd_project::Project::create`, used by the egui `rcfd-studio` removed by F-449). Until one exists the Studio shows the
folder as not a project yet, with "Make a project here" unavailable and the reason in its tooltip, watches the folder
(every second, no writes) and opens it by itself when `rcfd-project.json` appears. Proposed:
`rcfd project init <dir> [--name]` and an MCP `create_project` wrapping `Project::create`.

Decisions taken in building F-813 (options, the recommendation followed; each reversible without touching the engine):

- D1, an empty folder: (a) say what is missing, watch the folder and open it when the project file appears; (b) write
  the manifest from the Studio (refused: G2, G12); (c) an engine entry point (proposed above, Rust). Taken: (a).
- D2, the missing command: show "Make a project here" as unavailable with its reason, rather than leave it out, so the
  learner sees it is a known gap (G6). Taken.
- D3, how the example runs: (a) `start_run` with the engine's start plan (a picture to show); (b) F-807's
  `project.run_case` (stores no fields, so no picture); (c) one `start_run {new_case}` call (no separate make step to
  explain). Taken: (a), as a separate action list (`src/shared/actions/first.ts`), because `RUN_ACTIONS` is pinned by
  the F-807 tests.
- D4, after Run: the path stays on Home and moves to "Watch it work", with "Follow it in Runs", rather than switching to
  Runs as the case table's Run does. Taken.
- D5, the result: on Home, with the field viewport embedded; the link to the run is the single function
  `openFirstResult`.
- D6, the project folder as a data root: opening a project now allows its folder for the field viewport's local route
  (`src/main/index.ts`), so a run's stored fields can be drawn.
- D7, G22 (maintainer, 2026-11-16): the path's explanations moved behind Explain and into accessible names; visible
  copy is kept for the exceptions listed above.
- Open: name the command palette (F-831) in each step's "Full control" once it is merged; the field viewport's own
  caption keeps the precise terms (`ux, m/s`, snapshot, step), one line under the picture.

## The performance bench (F-817, G9)

`npm run bench` repeats P2, P4, P5, P7, P8 and P10 of `docs/studio-guardrails.md` G9 against the packaged Studio
(`npm run build && npm run package:dir` first) and prints each against its limit, one line each:

    P<n> <what> <value> <unit> limit <limit> met|MISSED

    npm run bench                              # the light set: a few minutes idle, 20 under full load
    npm run bench -- --heavy                   # the full samples and the large cases; once, by hand (house rule 7)
    npm run bench -- --only=P2,P10             # a subset; also --data=<folder>, --bench-server=<f447_bench>, --out=<file.json>, --no-gen

- One entry point over the scripts of the items that measured each limit first: `measure-cold-start.mjs` (P10, F-801),
  `measure-service.mjs` (P2 with P1, F-803), `measure-data.mjs` (P4, F-804), `measure-scrub.mjs` (P5, F-810) and
  `measure-field.mjs` (P7 and P8, F-808). Each runs as its own process; its output is echoed under `|` and its JSON kept.
  F-807's P2 per run action (`test-results-visual/f807/p2.json`, written by `npm test`) is printed beside P2 when present.
- Light: P2 on 100 iteration events of the 1e5 box, P4 at 1e6 with 200 repeats per route and codec, P5 at 1e6 with 10
  passes, P7 with 5 repeats per route, P8 with 1200 frames twice, P10 with 10 starts. `--heavy` takes P2's full 300 events
  and adds, recorded and not judged, P6 at 4e6, Follow live, and P12 over the 1e6 and 4e6 boxes.
- Judging follows the records of ADR 0801: P4 and P7 by route R1 or R2 (ADR 0350 §3), P4 on the raw store; P7 twice,
  on the median of the 5 opens and on the first open of the store in the launch (F-819, the maintainer's answer of
  2026-11-17), each against 1.5 s; P8's frame
  rate is the better of the vertically synced and the uncapped run, its p99 frame the synced run's. Lines marked
  `(recorded)` are not judged. Exit code 0 when every judged line is met, 1 on a miss, 2 when a case could not run.
- Load: the processor busy fraction and the processes that use the processor (summed by name over 2 s, in cores) are
  taken before and after the run, and each script takes its own before and after. A case whose busy fraction was 0.5 or
  more says `LOADED MACHINE` beside its lines. Nothing is retried.
- Data: the boxes of `f447_bench gen` (tiny and 1e6; 4e6 for `--heavy`) in `--data`, `$RCFD_BENCH_DATA` or
  `<temp>/rcfd-studio-bench-data`, generated once when missing (about 3 min for 1e6 and 25 min for 4e6, not counted).
  `measure-scrub.mjs` makes the folder a project the Studio opens (the boxes move into run folders, linked back).
  Binaries: `f447_bench` and `rcfd-mcp`, found as the tests find them (`RCFD_F447_BENCH`, `RCFD_MCP_BIN`,
  `$CARGO_TARGET_DIR`, `target/release` here or above).
- Window: the Studio runs in the tests' mode, `RCFD_TEST_HEADLESS=1` (transparent, click-through, unfocused, off the
  taskbar, background throttling off), so a run opens no visible window; `RCFD_TEST_HEADLESS=0 npm run bench` measures
  with the normal window. The mode is printed and kept in the results.
- Profile (F-817b; ADR 0800 §9, Q5): every case but P10 runs with a scratch Chromium profile. The bench makes a fresh
  folder per case in the temporary directory and passes it as `--profile-root=<folder>` (`scripts/profile.mjs`); each
  launch of the Studio gets an empty `--user-data-dir=<folder>/launch-<n>`, and the folder is removed when the case
  ends. P10 keeps the user's own profile (`%APPDATA%\rcfd Studio`). A `measure-*.mjs` script run alone uses the user's
  profile unless given `--profile-root`.
- Drawing on demand (F-817b): three.js's WebGPURenderer keeps a frame loop of its own once it has drawn;
  `src/renderer/ui/onDemand.ts` stops it two or three frames after the last draw, on the field canvas and the plots, so
  the Studio asks for no animation frames at rest (`tests/e2e/idle-frames.spec.ts`: at most 5 per second, measured over
  2 s on the start page, the project home, the Runs view and the workspace after an orbit key and after a scrub).
- Results: `out/bench/bench-<time>.json` (every line, the machine, the load, each script's own JSON). The bench is not
  part of `npm test` or CI; results are recorded by hand in ADR 0800 §9.

### Large meshes and the FAN-01 mesh (F-819)

`npm run bench:large` (`scripts/measure-large.mjs`) applies the G9 limits of P7, P5 and P8, unchanged, to meshes larger
and less regular than the 1e6 box they were set on. The limits are registered in the script (`LIMITS`) and printed
before anything is measured; every case runs `--repeats` times (default 3) and every run is printed with the
processor's busy fraction before and after it. Nothing is retried.

    npm run bench:large -- --profile-root=<scratch folder>          # 4e6 box, FAN-01 F3 and F1, 3 repeats
    npm run bench:large -- --smoke                                  # FAN-01 F3, one repeat, to check the script
    npm run bench -- --heavy                                        # runs it once (--repeats=1), its lines recorded

- First picture (P7): the development page's viewport, 5 opens per mesh after the tiny box, judged on the median as
  `measure-field.mjs` does, and (the maintainer's answer to Q-819b-1, 2026-11-17) on the first of them, the first open
  of that store in the launch, against the same 1.5 s.
- Scrubbing (P5): the run workspace, Home then ArrowRight through every snapshot of the run, about 200 requests per
  mesh, as `measure-scrub.mjs`; P6 (at least 3 per second) is printed beside it for the 4e6 box, recorded.
- Draw rate (P8): a drag on the viewport, a trusted press and then one move of that pointer per animation frame, held
  6 s after a 1 s warm-up; a draw is a frame in which the page submitted GPU work. Taken with the hover readout (each
  move picks the cell under the cursor) and with the pick skipped, each with vertical sync and with no rate limit.
- Meshes: the 4e6 box of `f447_bench gen` (`--sizes`), and the FAN-01 blade passage of `example-projects/axial-fan`
  (`--fans`): its committed F3 run (4 920 cells) and an F1 run (38 184 cells, 20 iterations, a snapshot of every
  iteration) that the script makes once with `rcfd-runner` in `<data>/fan01` when missing (`--no-gen` refuses).
- Exit code 0 when every judged line is met, 1 on a miss, 2 when a case could not run. Results with `--out=<file.json>`.
- F-819b: since the viewport takes no pick while a button is held, the drag "with the hover readout" now measures
  what the user has during an orbit (no pick); the pick-skipped drag is kept as the control. ADR 0801, Addendum F-819b,
  has the numbers before and after the three fixes.

## Provenance and reproduce from the CLI (F-828)

Any number of the runs view, the run workspace, the time panel and the field readout opens where it came from, and every
run shows the command that runs it again (G17). Code: `src/renderer/provenance/` (`Provenance.tsx`, `registry.ts`,
`sources.ts`, `door.ts`, `provenance.css`), `src/main/provenance/` and `src/shared/provenance.ts`.

- **Quiet (G22, The Quiet Rule)**: at rest a number looks as it did; the trigger takes the font and colour around it and
  adds no word, mark, border or fill. Its words are its accessible name (`Provenance of lid_drag: -0.008166 N`,
  `Provenance of run r-…`); hover and focus add only a dotted ink underline (focus keeps the orange outline). The facts
  appear on demand: a click, Enter or Space on the number; Show provenance (Ctrl+I, or the palette) on the focused
  number, the number inside the focused row (the runs table, whose numbers are not tab stops of their own), the number
  the focused or pointed element names (the scrubber names the snapshot counter, the field view its readout), the
  pointed number, else the selected run. Escape closes the popover and the focus goes back.
- **What it shows**: the value (as shown and the engine's full value), the unit exactly as the engine wrote it (the
  field's unit of `store.rts`, the series' unit of `runs/<id>/output/monitors.json`; an empty unit is said to be empty,
  a missing one says why; nothing is converted or guessed, G2), the run, the snapshot (index of the store, step, time),
  the file the number was read from, the reader (`read_residuals`, `list_runs`/`run_status`, the Studio's reader of
  `store.rts` that equals rcfd-tsstore's bit for bit), the engine version that made the run (`solver_version` of
  `run.json`) and, for the run, its record (`runs/<id>/run.json`, its hash, the settings verbatim, the case snapshot's
  hash). Copy puts the provenance as text on the clipboard; Copy command the command alone. The clipboard is written by
  the main process (`window.studioProvenance.copy`; every permission stays denied in the page, G10).
- **The reproduce command is the record's** (`reproduceCommand`): `rcfd-runner start <case> --project <folder> --threads
  <n> --priority <n> [--mem-mb <n>] [--set path=value]...`, with the case of `run.json` in the open folder and every
  setting of `run.json` in its order. Nothing comes from the Studio's own settings. It names the case whole (rcfd-runner
  reads a relative case against its working folder). The gaps are the exception that speaks, in words, in the popover
  and beside a copied command: a settings key `rcfd-runner start` has no option for (`continue_from`, `max_steps` of ADR
  0200, or any later key), a case file changed since the run (the engine's `read_project_file` hash of the case now
  against `case_snapshot_hash`), a `case.snapshot` that does not match its record, a case outside the project (not
  checkable), a mesh file the record names (not checkable), a run whose engine version is not recorded or differs from
  the engine here (the project server's `serverInfo`, and the attached service's `service.info` when one answers). A
  design study's labels (study, variant, condition) do not change the result and are left out.
- **Through the engine only**: a fifth port, `window.studioProvenance` (`src/shared/provenance.ts`; the one renderer
  module naming it is `provenance/door.ts`), asks the open project's read-only project server `read_project_file` for
  `runs/<id>/run.json`, `runs/<id>/case.snapshot`, the case file and `runs/<id>/output/monitors.json`. The Studio reads
  no file itself and writes none.
- **Commands** (G20): Show provenance (`provenance.show`, Ctrl+I) and Copy reproduce command (`provenance.copy_command`,
  asks which run, the selected one first) in the palette.

The reproduce test (G17, `tests/e2e/provenance.spec.ts`), registered here before its code. A run of
`cases/cavity-re100.toml` is started from the window (Runs, Run, by keyboard) in a copy of the quick-start project,
with the real `rcfd-mcp`, `rcfd-runner` and `rcfd`; its command is copied from its provenance, run from a shell
(PowerShell on Windows, `sh` elsewhere) with only `--project` changed to another, empty project folder, and the two run
folders are compared:

| Files | Compared | Why |
|---|---|---|
| `fields/store.rts`, `fields/index.json`, `residuals.jsonl`, `output/monitors.json`, `output/monitors.csv`, `output/report.html`, `output/restart.rcfd`, `output/solution.vtu`, `case.snapshot`, `check.json` | byte for byte | the results and their inputs; they hold no time, path or process id (checked by hand on 2026-11-16: two runs of the same case in two folders gave the same bytes) |
| `metrics.json` | equal after removing `elapsed_s` and `wall_s` (wherever they are) | wall-clock times of the solver |
| `status.json` | equal after removing `updated` and `pid` | when the runner wrote it, and the solver's process id |
| `run.json` | `case_snapshot_hash`, `solver_version` and `settings` equal; `id`, `created`, `case`, `case_source` and `case_dir` excluded | the new run's own id and creation time; the case's path as seen from the other folder (it is outside that project, so the runner records it absolute) |
| `solver-status.jsonl` | equal line by line after removing `wall_s` and the run folder's paths (`info.case`, the checkpoint's `path`) | wall-clock times; paths of the run's own folder |
| `events.jsonl` | the sequence of event kinds equal; `t`, `pid`, `launch_seq`, paths and the solver lines (compared through `solver-status.jsonl`) excluded | times and process ids of the runner; `launch_seq` counts the runner's launches in its project |
| `log.txt` | not compared | the solver's human-readable log names the run folder's absolute paths |

Decisions taken in building F-828 (options, the recommendation followed; each reversible without touching the engine):

- D1, the reproduce command: (a) `rcfd-runner start` with the record's case and settings, the CLI twin of `start_run`
  that makes the same run folder (residuals, fields, status); (b) `rcfd run runs/<id>/case.snapshot --case-dir ...` with
  the runner's launch flags (exact case bytes, but the launch argv is not recorded, so the Studio would write the
  runner's flags from its source, and no `residuals.jsonl` comes out). Taken: (a), with the case's hash checked against
  the record so that a changed case is said; (b) needs the engine to record its launch argv (proposed below).
- D2, the case path: the record's project-relative case in the open folder (the record's absolute `case_source` goes
  stale when a project moves, as in the shipped example projects).
- D3, engine version: the run's `solver_version` is shown; another engine version here (project server or service) is a
  gap in words, compared as `<name> <version>`, which is how `rcfd --version` prints it.
- D4, the port: a fifth MessagePort (`studio.provenance-port`), since the run port's operations are fixed by F-810's test.
- D5, the clipboard: written by the main process for Copy, so the page keeps no clipboard permission.
- D6, Ctrl+I for Show provenance (free in the Electron default menu, Chromium and the table); Copy reproduce command has
  no key.
- Not covered: numbers of a service session (the settings page, the `#/dev/plots` route): their reproduce command would
  be `rcfd call journal.get` then `rcfd run --journal`; `journal.get` keeps its exemption until a surface shows a
  session's results. The axis ticks of the plots and the legend's ticks are scales, not values, and open nothing.
- Missing in the engine (no Rust change here): `run.json` records neither the solver's launch argv nor a build id
  (`solver_version` is a release name, so two builds of one release cannot be told apart); `rcfd-runner start` has no
  options for `continue_from` and `max_steps` (`start_run` and the runner take them); no tool reruns a run from its own
  snapshot (`rcfd-runner rerun <id>` or `start_run {from_run}`, proposed); `status.json` gives `time` with no unit, so
  the Runs view's "Time, s" caption is the Studio's word, not the engine's (shown as such in the provenance).

## Responsiveness and cancellable work (F-832, G21)

### The registered limits

Written here before the measurement code (and in ADR 0801, addendum 9); `src/shared/work.ts` holds the same numbers
and the tests import them from there. Changing one needs the maintainer, like a G9 limit.

| Name | Value | What it bounds |
|---|---|---|
| `LONG_TASK_LIMIT_MS` | 100 ms | any one task on the renderer's main thread (a `longtask` entry) while a run is opened, fields are loaded and the scrubber moves |
| `LONG_FRAME_LIMIT_MS` | 150 ms | any one animation frame in the same window (a `long-animation-frame` entry: its tasks, style, layout and paint) |
| `PROGRESS_AFTER_MS` | 300 ms | an operation still going after this shows its progress and a Cancel; a shorter one shows nothing |

How they were chosen:

- 100 ms per task: while a task runs, nothing else on the page answers, not even the Cancel button or the scrubber's next
  key. 100 ms is the classic limit for a reaction to feel immediate (Card, Moran and Newell's 0.1 s; the same figure as
  the "respond within 100 ms" input budget of web performance practice), so a key pressed during the longest allowed
  task is still answered in time. It is also two thirds of G9's P5 frame limit (150 ms), leaving the rest of a frame for
  the GPU write and the draw, and twice the 50 ms at which Chromium starts reporting a task as long, so ordinary React
  renders of the 1440 x 900 workspace (10 to 40 ms) are not counted against it.
- 150 ms per frame: G9's P5 already says "no frame > 150 ms" while scrubbing; the frame limit is that number, applied to
  every frame of the measurement, not just the scrub requests.
- 300 ms before progress shows: below it, progress would appear and vanish (noise; G22: the normal state carries no
  words); above it, anything long says so well before the 1 s at which a person loses the sense of an uninterrupted
  flow (Nielsen). It is the `QUIET_MS` F-810 chose for scrubbing, so the field viewport keeps its behaviour, and it lies
  above every scrub frame measured in ADR 0801 addendum 6 (1e6 cells p99 75 ms, max 92 ms; 4e6 cells p99 159 ms), so
  scrubbing at those sizes never shows progress, while a first picture of 1e6 cells (P7, up to 1.5 s) or the opening
  of a project does.

### What is built

- **One mechanism for long operations.** `src/renderer/work/work.ts` is the work tracker: every operation that may take
  long begins a work there and ends it when it is done, failed or cancelled. A work still going after
  `PROGRESS_AFTER_MS` is shown by `WorkProgress` (`src/renderer/work/WorkProgress.tsx`) in its own surface as one row:
  an indeterminate bar drawn as DESIGN.md's "not yet" hatch moving (still under reduced motion), a few words, and
  Cancel. Nothing is rendered for a shorter work (G22). Cancel aborts the work's `AbortSignal` and runs the work's own
  undo. The operations: opening a project (the project home), opening a run (the run workspace), opening a store and
  loading a field (the field viewport, both bulk routes), checking or saving a case and checking a mesh (the case
  editor).
- **How a cancel crosses a door.** `contextBridge` copies objects, so an `AbortSignal` would arrive dead. The caller
  passes `options.onCancel` instead (`CancelOptions`, `src/shared/work.ts`); the preload script calls it with the
  function that cancels that request, and functions do cross as proxies. The bridges' key sets stay as they were (the
  e2e tests pin them). The data door already cancelled by tag (F-806); the project port gained `cancel`, and the case
  port gained `cancel {target}`.
- **What a cancel undoes.**
  - Opening a project: the project open before stays open; the recent list is not written; no failure is shown. A
    cancel before the engine's project server is started starts nothing. After it, the server's start finishes (the
    engine cannot stop it half way) and the late session is closed.
  - Opening a run: the run selected before is selected again, with its picture and stop. With none before, nothing is
    selected and the workspace stops choosing one for you until you pick one. The store's request is cancelled through
    the data door. The histories (`run.series`) are dropped, because the run port has no cancel.
  - A field load: the picture shown before stays, the same pixels. The line goes back to the stop of that picture, and
    playing and Follow live stop. The request is cancelled through the data door: a queued one is never sent, and a
    started one's result is dropped.
  - Saving a case: a cancel that comes before anything has been sent to the engine's `edit_case` writes nothing (the
    file's bytes are unchanged) and the changes stay pending. Once `edit_case` has been sent, the save completes and
    reports itself as usual. The engine writes a file whole or not at all, so the outcome is never half a file.
  - Checking the saved file: no result is shown, and a result shown before stays. The engine's `check_case_headless`
    finishes anyway, and it writes nothing.
  - Checking a mesh: the reply is dropped. The service has no way to stop `mesh.check`.
- **The main thread does no data work.** The arrays the GPU draws (a corner position per triangle, the bounding box and
  sphere, a scalar per corner, the shown range) are made in a worker (`src/renderer/field/prepare.worker.ts`, client
  `prepare.ts`) from `surface.ts`'s unchanged arithmetic. So are F-809's mesh layers and vector arrows (`derived.ts`,
  unchanged). Values go there and come back transferred. The decoding was already off the main thread: the data
  bridge reads and decodes in the main process.
- **The proof.** `tests/e2e/responsiveness.spec.ts` (below) and `tests/e2e/cancel.spec.ts` (each operation cancelled
  while it runs, and checked to leave nothing partial; screenshots of the progress at 1440 x 900 in
  `test-results-visual/f832/`), plus `tests/unit/work.spec.ts`.

### The long-task test

`tests/e2e/responsiveness.spec.ts` runs the packaged Studio at 1440 x 900, with a `PerformanceObserver` on `longtask`
and `long-animation-frame` installed before anything is measured. It opens a project and then a run of 1e5 cells
(`RCFD_F832_SIZE=1e6` gives a run of 1e6 cells, by hand). It scrubs through the run's 20 snapshots, one stop at a time
and then with a held key, changes the field, and loads fields by both bulk routes (R1 through `f447_bench serve`, R2
on the field viewport's page). First it runs a probe: a deliberate 130 ms task, which must be recorded. The test fails
on any task over `LONG_TASK_LIMIT_MS`, and on any frame over `LONG_FRAME_LIMIT_MS` that the renderer's thread held up
(`blockingDuration` over 0). A frame whose tasks were all short and whose rendering came late is reported, not failed.
It makes up to three passes, each in a fresh window, and passes on the first clean one. A task that the Studio's own
code makes recurs in every pass. A stall caused by the machine's other load does not. The entries, their phases and
the machine's load go to `test-results-visual/f832/long-tasks.json`.

### The tests' delay

`RCFD_STUDIO_TEST_DELAY` is a JSON object such as `{"data": 4000, "projectOpen": 4000, "caseCheck": 4000, "caseSave": 4000}`.
It adds a delay, in milliseconds, before the engine is asked (`src/main/work/delay.ts`), so the e2e tests can cancel an
operation while it is running on any machine. A cancel during the delay sends nothing to the engine. The variable is unset outside the
tests, and then nothing waits.

## The Quiet Rule and the word budget (F-833, G22)

A normal state carries no visible words (DESIGN.md's Quiet Rule, G22). The words a normal state needs are kept, moved off
the screen: to accessible names and descriptions, to tooltips, to focus detail, to Explain (help on demand) and to the
command palette. What stays on the screen: data (names, paths, ids, numbers, times), the controls' own labels, column
headers and field labels, the G6 disclaimer, and the exceptions, which speak: a failure, a cancelled run, a loss (the
WebGL 2 fallback, a decimation), a known miss (a report value with no unit from the engine), a conflict, something
unavailable and why, a value to act on (changes not saved).

- The pieces (`src/renderer/ui/Quiet.tsx`): `Quiet` (words for readers only, the `.sr-only` class), `EmptyMark` (an empty
  state at rest: DESIGN.md's "not yet" hatch, its sentence as the accessible name of a `note` and its tooltip) and
  `Dash` (a normal absence of a value, an en dash named by its words). `StateMark quiet` shows a normal run state's
  pictogram alone (role `img`, the word as name and tooltip); `failed` and `cancelled` keep their word.
- A key of case settings and the case editor shows its name and control; its description, hint, default, range and value
  count are its focus detail (`.st-key-detail`, shown while the row holds the focus, always its field's description).
  A set key keeps its Reset button.
- The counter: `src/shared/word-budget.ts` defines a chrome word at rest (written before the first count) and holds
  the budget of every surface; `tests/support/word-count.ts` counts from the DOM; `data-words` marks the visible words
  that are not chrome (`data`, `exception`, `disclaimer`, `help`). `tests/e2e/word-budget.spec.ts` brings each surface
  to rest in one Studio, photographs it, counts it, and fails when a surface passes its budget. `RCFD_WORD_PHASE=before`
  writes the photographs and counts to `test-results-visual/f833/before/` instead of `after/`.
- The budgets are the AFTER counts of the sweep, approved by the maintainer on 2026-11-17 and enforced
  (`BUDGET_STATUS`); raising one needs the maintainer's word.

| Surface | Before | After = budget | What stayed |
|---|---:|---:|---|
| project home, no project | 75 | 6 | brand, Commands, Open project folder… |
| first path, no project | 22 | 22 | the four stations and Explain (F-813 was quiet already) |
| project home, project open | 167 | 49 | tabs, filter toggles, group titles, column headers, row actions |
| first path, empty project | 22 | 22 | as above |
| runs | 64 | 14 | run control, filters, column headers |
| runs, a run selected | 64 | 16 | actions and number captions; the no-unit miss and the runner's reason (data) |
| run workspace | 111 | 17 | number captions, Mesh grid, caption keys, axis titles |
| compare | 65 | 8 | Mesh grid and caption keys of each view |
| derived-picture controls | 0 | 0 | |
| provenance popover | 11 | 11 | on demand: its field captions and Copy |
| Claude Code panel | 63 | 13 | title, List again, Sessions, New chat, Tool approvals, the placeholder |
| command palette | 31 | 4 | title and placeholder |
| case settings | 144 | 23 | section list, Show every key, Check the case |
| case editor | 553 | 51 | as settings, plus Reset of each set key and the case actions |
| undo controls | 6 | 0 | arrows named "Nothing to undo" and "Nothing to redo" |
| decision panel | 0 | 0 | the whole panel is an exception |
| progress rows | 3 | 3 | the work's name and Cancel (a work in progress is not at rest) |

## Keyboard and assistive technology (F-816, G8)

Every built surface works from the keyboard alone, shows where the focus is, names every control, keeps still under
reduced motion and never says a state by colour alone. No WCAG contrast rule applies (the maintainer removed it on
2026-11-16); colour values follow `DESIGN.md`. The checks are written in `tests/support/a11y.ts` on the DOM, the
keyboard and Chromium's own accessibility tree (read through the DevTools protocol). On top of them, axe-core 4.14.0
(a dev dependency, MPL-2.0, ADR 0801 Addendum 12, by the maintainer's word of 2026-11-17) runs on each surface of the
walk (`tests/support/axe.ts`) and fails only on violations of impact serious or critical; `color-contrast` is off,
and a rule whose fix would change `DESIGN.md` or a visible design goes in the documented `EXCLUDED` list there with
its reason (empty today). It writes `test-results-visual/f816/axe.json`. This is not a full accessibility audit.

`tests/e2e/a11y.spec.ts` opens the packaged Studio against the real engine (as `word-budget.spec.ts` does) and reaches
every surface with keys only: Tab, Shift+Tab, the arrows, Enter, Space, Escape and the shortcuts (Ctrl+1, Ctrl+2, Ctrl+K,
Ctrl+O). The folder dialog is the one thing stubbed. Surfaces walked: project home (no project, a project, an empty
project with the first path), the Claude Code panel, Runs and a selected run, the provenance popover, the run workspace
with the field view, the derived-picture controls, a select's list, compare mode, the command palette, the case
editor and its undo controls, case settings and the decision panel. On each surface:

| Check | How |
|---|---|
| No trap | Tab goes round the whole window and comes back to its first stop; Shift+Tab goes round the same stops in reverse order. A dialog (palette, provenance, a select's list, the decision panel) keeps Tab inside, closes on Escape and gives the focus back to what opened it. |
| Visible focus | Each stop draws an outline, box shadow or border that it does not have once the focus has moved on (on itself, or on the part that draws its ring: a switch's track, a row's cells, the scrubber's line), and that box is on the screen. Not a colour or contrast check. |
| Reach | Every button, link, field, slider, tab, option and other interactive element of the surface is a Tab stop or a member of a composite that is one (a list box, grid, tab list, radio group, roving toolbar); the arrow keys reach every member and come back. A number field's steppers are its Up and Down keys; React Aria's hidden Dismiss buttons are Escape. |
| No pointer-only target | No element with a React click or pointer handler lacks a keyboard path (React keeps an element's props on it; the check reads them). |
| Names and roles | Every node of Chromium's accessibility tree with an interactive role (button, link, textbox, slider, tab, option, checkbox, radio, switch, combobox, list box, grid, dialog, img, progressbar and the like) has a non-empty name; every Tab stop has a role. |
| Reduced motion | With `prefers-reduced-motion: reduce` emulated, no element declares a CSS animation or transition over 50 ms and none runs; without it, the run bands' grow-in is seen (so the check sees motion when there is some). Play steps no faster than one stop per 500 ms; one orbit key moves the camera at once (the picture 150 ms after the key is the picture 900 ms after). |
| Colour alone | Each state that has a colour also has a pictogram, a shape, a position or a word (the list below), read from the page and from the stylesheets. |

It writes `test-results-visual/f816/report.json` (every surface's stops, how each draws its focus, the composites,
the motion measured, the colour list) and screenshots of focus states (`<surface>-NN-<role>.png`, one per role, up to
six per surface).

What F-816 changed in the product:

- The run bands' names select a run from the keyboard: they are buttons with one Tab stop between them (the selected
  run's); Up and Down, Home and End move between them; Enter or Space selects (ARIA's toolbar pattern). They were a
  click-only `div`, and the runs list was the only keyboard path.
- The decision panel (F-829) and a run's confirmation strip (F-807) keep Tab inside while they ask (`ui/containFocus.ts`,
  `aria-modal`), so Escape, which only they answer, stays in reach. Before, Tab left them open behind the focus.
- Play under reduced motion steps at most one stop per 500 ms (`ui/motion.ts`), below three changes a second; without
  the preference it is unchanged.
- The live band segment ends in a 2px ink edge, and its band end says "live" in its description and tooltip. Before,
  orange alone told it from another running run's band.
- A Claude Code session that cannot be chosen while a reply runs has a dashed hairline, as every unavailable control
  does. Before, only its ink-2 name said so.

State and its non-colour carrier:

| State (colour) | Also carried by |
|---|---|
| Run state: queued, running, converged, finished, failed, paused, cancelled (state colours) | its pictogram, one shape per state, and the word (shown for failed and cancelled, the name and tooltip otherwise) |
| Failed row, notice or problem (fail wash, red text) | the failed pictogram and the words ("Failed", "Change failed.") |
| Band segments: iterating (sky), converged (meadow), failed (red) | position: the phase at the band's end is the end's pictogram and state word |
| Live band segment (orange) | the ink edge at its growing end; "live" in the end's description and tooltip (new) |
| Queued or not computed yet (silver hatch) | the hatch pattern |
| Band end | its pictogram and its name ("converged · it 121"), and where the band ends |
| Selected band | bold name, ink underline, `aria-current` |
| Project folder live or not live | an ink square, or the failed pictogram and "Not live" |
| Compared run's lines | thinner lines, the same dashes per equation, names in the readout |
| Difference field and metrics (colour map) | the legend's numbers, the readout, signed numbers in the metrics table |
| Conflict | the words of the decision panel or notice, and the table of yours and theirs |
| Validation (red field border) | the failed pictogram and "Change failed." with the engine's words |
| Switch on (meadow) | the knob's position |
| Selected segment, tab, option, row, section, session | ink fill, bold weight, a check mark or an ink box; `aria-selected`, `aria-pressed`, `aria-current` |
| Unavailable control or session | a dashed hairline border; the reason in the name or tooltip |
| Focus (orange) | the ring is a shape: a 2px line around the control |

### Manual pass (view-and-report)

What the automated checks cannot judge, for the maintainer to repeat with the packaged Studio
(`npm run build && npm run package:dir`, then `out/packaged/rcfd-studio.exe`) on a project with a running run (start one
from a case row's Run in the project home, or with `rcfd-runner start`):

1. Keyboard only, no mouse, from start to finish: open a project (Ctrl+O), open Runs (Ctrl+2), select a run (Tab into
   the table, arrows, Space), open its provenance (Tab to a number, Enter; Escape), go to Workspace (arrows on the
   project tabs), pick a run by its band name (Tab to the names, Up and Down, Enter), scrub (arrows, Page Up and Down,
   Home, End), Play and Pause, orbit and zoom the field (arrows, + and -, Home or 0 to reset), Compare runs (Space, then the compared run's list),
   open the palette (Ctrl+K) and run a command, edit a case (Ctrl+1, Edit, change a key, Ctrl+S), undo (Ctrl+Z), close a
   settings session and keep it. Report any step you could not do, or where you lost the focus.
2. At every step: is the orange ring always visible, and on the thing Enter would act on? Report any place where you had
   to look for it (inside scroll boxes, at the window's edges, on the field and the plots).
3. A running run: press Stop… or Cancel… on it. The strip asks and has the focus; Tab and Shift+Tab stay on its two
   buttons; Escape keeps the run and the focus goes back to Stop… (the automated walk has no running run).
4. Windows Narrator (Ctrl+Win+Enter): Tab through the project home, Runs and the workspace. Every control is read with
   a name and a role, and a run's state with its word. Report a control read as "button" with no name, or a name that
   does not match what the control does.
5. Windows Settings > Accessibility > Visual effects > Animation effects off, then restart the Studio: the run bands
   appear at once, switches and disclosures do not slide, Play steps about twice a second.
6. A colour filter (Windows Settings > Accessibility > Colour filters, red-green): every run state, band phase, the live
   band, a failure and a conflict can still be told apart.
7. Windows text scaling at 150 %: no control is cut off so that its name or its focus ring cannot be seen.

Open for the maintainer: whether to add a checker library under G5 (axe-core, MPL-2.0) on top of these checks. The
written checks cover the rules of F-816 and need no dependency (ADR 0801, Addendum F-816, D1).

## Run logs, runner budgets and field-view extras from the egui Studio (F-837)

What the removed egui Studio (`crates/rcfd-studio`, read at 44e63df8 for what it offered, not how it looked) had beside
its run list, field view and project view, rebuilt in this Studio's design with no new engine entry point. Every
control is a pictogram with its name as tooltip and accessible name (G22); at rest no surface gains a chrome word (the
word budgets of `src/shared/word-budget.ts` are unchanged). The tests are `tests/unit/f837.spec.ts` and
`tests/e2e/f837.spec.ts`; the screenshots go to `test-results-visual/f837/`.

| Built | Where | From the engine |
|---|---|---|
| The latest residuals of u and p, one column each, headed by the engine's names | Runs list | `run_status` `last_residual` (the last line of `residuals.jsonl`) |
| The selected run's warnings at rest; its log's end (200 lines) and last events on demand, warnings named as such | Runs, a run selected | `read_project_file` of `runs/<id>/log.txt` from its end; `run_status {events: 50}` (the tool's maximum) |
| Open the report | Runs, a run selected | `read_project_file` says `runs/<id>/output/report.html` is there; the system's program for HTML opens it (`shell.openPath`) |
| Run again with fields stored | Runs, a run selected | `start_run {case, store: "plan"}` (ADR 0140) |
| The runner's budgets: read, set, undo | Runs, beside the run bar | `.rcfd/runner-config.json` and `.rcfd/runner.json` (rcfd-runner's own files; no engine command sets them) |
| Colour range of the whole run, or fixed | Field view | `data.field` of every stored snapshot (a cancellable work, F-832) |
| View face on (2D), with a scale bar | Field view | none (the camera) |
| The domain's extents in the caption | Field view | the stored mesh's points |
| Save the view as PNG | Field view | none (the window's picture, the save dialog) |
| The monitor on a log scale | Run workspace | none (values not above zero are counted and said, not drawn) |
| designs/ and its entries | Project home | the folder (the engine's count from `project_info` shown when it differs) |
| Data files of cases/ apart (no Edit or Run) | Project home | the engine's rule: a case is `.toml` or `.json` (ADR 0148) |
| A case file with unsaved changes in the case editor marked `*`, a link to the session that holds them | Project home | the case port's pending changes |
| The model and the session's summed cost of a finished Claude Code turn | Claude Code panel | Claude Code's own `result` and `system` lines |
| A project folder named on the command line opens at start | start | none |

Seven palette commands (G20): `runs.log`, `runs.report`, `runs.budgets` (group Project runs), `field.save_png`,
`field.range`, `field.face_on`, `plots.monitor_log` (a new group, Run workspace). `project.queue_case` takes an optional
`store` (`none`, the default as before, or `plan`).

### Decisions (options, the recommendation taken; each reversible)

- D1, the budgets' writer. (a) The main process writes `.rcfd/runner-config.json` with the runner's own checks (keys,
  whole numbers, positive counts), only the keys set, never over a change since it was read (a hash), and keeps the
  last write to undo; (b) hand it to Claude Code; (c) wait for an engine command. Taken: (a). The write is not atomic
  (a rename is outside the house rule of `tests/unit/history.spec.ts`); the runner reads the file once, when it starts.
  Proposed engine command: `rcfd-runner config set` or an MCP `set_runner_config`.
- D2, the budgets' undo. (a) The panel's own Undo of its last write, refused over another writer's change unless asked
  ("Undo anyway"); (b) a step in F-829's history, which is per case-editor session. Taken: (a). Undo of a write to a
  file that was not there leaves `{}` (every key at the runner's default): the Studio removes no file.
- D3, the report. (a) `shell.openPath` (the system's program for HTML); (b) show the file in its folder; (c) copy the
  path. Taken: (a).
- D4, the residual columns' heads. (a) The engine's names `u` and `p`, as data; (b) "Residual u" (a chrome word each).
  Taken: (a); the accessible name is "Last residual of u".
- D5, designs/ on the home. (a) Headed by the folder's own name `designs/` as data; (b) a group title "Designs" (+1
  chrome word on home-open, a maintainer decision). Taken: (a). A folder entry ends in `/`. The maintainer confirmed
  (a) on 2026-11-17: `designs/` stays the heading.
- D6, the unsaved mark. (a) `*`, a link to the case-editor session that holds the changes (Edit opens the file in a new
  session, without them); (b) a word ("unsaved"). Taken: (a).
- D7, the length unit. (a) Metres, from the engine's SI settings, with the reason as the tooltip; (b) no unit. Taken:
  (a). The store names no length unit (a gap for the engine).
- D8, face on. (a) A button that looks along the thin axis and keeps the view until Reset view; (b) make the fit face on
  for every 2D case (changes F-808's default view). Taken: (a). The scale bar shows only while the view is within 0.1
  degrees of an axis, so it is true across the picture.
- D9, the whole run's range. (a) Read every stored snapshot in the renderer, as a cancellable work, and colour by the
  min and max; (b) wait for an engine range query. Taken: (a); a cancel goes back to the snapshot's range.
- D10, Run again with fields stored. (a) `start_run` with `store: "plan"` (the engine's store plan); (b) a `--set
  output.store_interval=...` the Studio chooses. Taken: (a).
- D11, the project argument. (a) The first argument that is not a switch, opened once the project state is read; (b)
  only a `--project <folder>` switch. Taken: (a).
- D12, the session's cost. (a) The sum of the `total_cost_usd` of the turns sent from this Studio window, shown after a
  sum sign; (b) the session's whole history, turns from the terminal included (the Studio reads no cost from a
  transcript). Taken: (a).

### Needs the engine (not built)

- The egui run setup's "simulate N s" helper (time step and step count from a duration), adaptive correctors and the
  Courant fix: needs a case-facts query (cell sizes, a velocity scale) or `start_run {dry_run: true}` returning the
  plan and its estimates.
- Refine the mesh by 2: needs a mesh refine entry point (`refine_mesh {mesh, factor}` or a service method).
- Open and save one case file anywhere, outside a project: the case editor's save is project-relative; needs a service
  save to a path outside a project.
- The store's length unit: needs a unit for the coordinates in the store's info.

### Save or discard before a close (F-837, approved by the maintainer 2026-11-17)

A close that would lose case-editor changes not saved asks first, with F-829's decision panel
(`src/renderer/case/closeGuard.tsx`, mounted once in `main.tsx`); with nothing unsaved nothing asks.

- What asks: Close the session (the button and the palette's `case.close`), Close project (the button and
  `project.close`), and the window: its close button, Alt+F4 and the app's quit. The main process holds the window's
  `close` event and sends the page a request (`studio.close-request`); the page answers `asking` at once on
  `app.closeAnswer`, then `close` or `cancel`. A page that does not say `asking` within 3 s (hung, crashed, not loaded)
  cannot hold the window; a page that crashes while asking lets it close; a reload ends the question as a cancel. The
  quit asks every window before anything is stopped, and a cancel cancels the quit with nothing stopped.
- The words name what is lost: the title (`Close cases/cavity-re100.toml with 1 change not saved?`, `Close <project>
  with ...`, `Close rcfd Studio with ...`) and each file with its count and its first four setting paths. The panel is
  an exception surface (G22: a loss speaks), so no word budget changes.
- Choices: Save and close (each file through the engine, as Save; a refusal or another writer's change keeps the panel
  open with the engine's words, and nothing closes), Close and lose them, Keep editing (the safe choice: focused, and
  Escape). Tab stays in the panel; Keep editing gives the focus back where it was, a text field included. A value typed
  but not committed is committed first, as Save does, so it is counted.
- Close and lose them closes the session (Close the session) or the project's sessions with their changes (Close
  project). On the window it closes nothing more than the quit did before: G7 holds (the Studio's quit stops no session
  or run on the service).
- Test mode (`RCFD_TEST_HEADLESS=1`) quits without asking, so Playwright's `close()` (which is `app.quit()`) never waits
  for an answer; the window's own close still asks there. The P10 measurement's quit does not ask either.
- Tests: `tests/e2e/close-guard.spec.ts` (each close, by the keyboard, Tab held, focus back, the window's close with
  typing not yet committed, a page that does not answer, the test-mode quit with a change unsaved).
- Decisions D13 to D16 (the safe choice, what Close and lose them closes, the test-mode quit, the wait for the page):
  ADR 0801, Addendum 13.

## Security (G10)

- The renderer is sandboxed (`app.enableSandbox()` and the window's settings), context-isolated, with Node integration
  off and web security on. The tests read the effective settings back from the running app.
- The page is served from `app://studio/` by a protocol handler that only reads `dist/renderer/`. Every response carries
  the Content Security Policy header, and `src/renderer/index.html` repeats it in a `<meta>`:
  `default-src 'none'`, scripts, styles, images, fonts and fetches from `'self'` only, no inline script or style element,
  no `eval`, no frames, forms, plugins or `<base>`. A later item that needs more widens one directive in `security.ts`,
  with a test.
- No remote content: requests to http, https, ws, wss, ftp and file are cancelled in the session; navigation away from
  `app://studio/` and every `window.open` are refused; `<webview>` cannot be attached; permission requests are denied.
- IPC: the page sees only `window.studio` (`app.info()`, `app.ready()`, and F-837's `app.onCloseRequest(listener)` and
  `app.closeAnswer(request, answer)`, the window's close round trip), one function per channel of
  `src/shared/ipc.ts`. The main process registers exactly those channels and refuses calls from any frame that is not on
  `app://studio/`. New channels are added to the contract, the preload and the main handlers together, and the e2e
  test's allow-list.
- The solver service: the page sees `window.studioService` (`info`, `call`, `state`, `onState`, `offState`,
  `subscribe`, `unsubscribe`), not a port or a socket. The main process hands the preload script a MessagePort after
  each load of an app page; the preload keeps it and forwards. The main process checks every message on it
  (`src/main/service/bridge.ts`): the exact fields of each operation, a method on `SERVICE_METHODS`
  (`src/shared/service.ts`) and parameters of plain JSON values; anything else is refused before the service. The
  shell's ipcMain channels are `app.info`, `app.ready` and `app.closeAnswer`. A method is added to the list with a test.
- The case editor (F-812): the page sees `window.studioCase`, a third MessagePort checked by `src/main/case/bridge.ts`;
  the project server that writes is called only with the four tools of `CASE_TOOLS`.
- `RCFD_STUDIO_STARTUP_REPORT=<file>` (measurement only): the main process writes the time the project view was ready
  to that file and quits. Unset, it does nothing.

## Rules

- Guardrails: `docs/studio-guardrails.md` (G1 to G21). Product: `PRODUCT.md`. Visual system: `DESIGN.md`.
- Every dependency is pinned to an exact version (`.npmrc` sets `save-exact`), the lock file is committed, and each
  dependency is listed in `THIRD_PARTY_LICENSES.md` (run `python scripts/third_party_licenses.py` from the repository root
  after any change to `package-lock.json`) and justified in an ADR (ADR 0801 and its addenda).
- The design reference is `apps/studio-prototype/` (synthetic data). Nothing is copied from it into this app; the
  surfaces are built from `DESIGN.md` through their own roadmap items.
