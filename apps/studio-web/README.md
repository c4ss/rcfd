# rcfd Studio (Electron)

The Electron desktop front end of rcfd: F-448 and the Studio roadmap of ADR 0800 (F-800 to F-832). This directory is
outside the Cargo workspace (ADR 0350 Q2, guardrail G5). The engine is Rust and stays Rust (G2); this app is a client of
it.

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
  folders come from the main process's dialog. The ipcMain channels stay `app.info` and `app.ready`.
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

- **Run bands**: each band ends in its state's pictogram and word ("converged · it 121", "failed · it 0",
  "running · it 1450"; a transient run's end in simulated time, "converged · 13.2 s"). Segments: sky iterating, a meadow
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
- **Through the engine only**: a fourth port, `window.studioRun` (`src/shared/run.ts`; the one renderer module naming it
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

## The field viewport (F-808)

`src/renderer/field/FieldViewport.tsx` draws one cell field of one snapshot of a store on the boundary surface of the
stored mesh: three.js `WebGPURenderer` (made and initialised asynchronously inside react-three-fiber's `Canvas`), with
three's WebGL 2 back end as the fallback; the back end that runs is named above the canvas ("Drawn by WebGPU", or "Drawn
by WebGL 2 (fallback: <why>)"). Until a project view hosts it (F-806), it is shown on a development page,
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
  orbits with the arrow keys, zooms with + and -, resets with Home, and reads out the value at its centre.
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
  transcript reading, resume and writing the allow-list file (`rcfd-claude` has the code; only the frozen egui Studio
  uses it).

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
| `src/renderer/ui/` | The primitives of `DESIGN.md` Components on React Aria Components: buttons, toggles and segmented toggles, text and number fields, select, switch, tabs (sub and project), table, slider, tooltip, state pictograms and icons; `primitives.css` | as above | F-802 |
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
| `src/renderer/workspace/` | The run workspace (`RunWorkspace.tsx`, `TimePanel.tsx`: run bands, scrubber, residual plot, monitor), `model.ts`, its one door to `window.studioRun` (`door.ts`), `workspace.css` | as above | F-810 |
| `src/main/run/`, `src/shared/run.ts` | The run port: `bridge.ts` (the checks), `series.ts` (a run's residual and monitor series and their units through the project server) | `tsc -p tsconfig.node.json` | F-810 |
| `tests/unit/` | Pure tests of `security.ts` and the built `index.html` (F-801); tokens against `DESIGN.md`, the pictogram grid (F-802); the service client against a fake service (F-803) | run by Playwright (project `unit`) | F-801, F-802, F-803 |
| `tests/e2e/` | The packaged shell driven by Playwright's `_electron.launch`; the primitives (F-802); the service port's allow-list against a fake service, and the client against the real `rcfd serve` (F-803) | run by Playwright (project `electron`) | F-801 onwards |
| `tests/support/` | The fake service, finding and starting `rcfd`, launching the packaged Studio | imported by the tests | F-803 |
| `scripts/` | `package-dir.mjs` (packaging by hand), `measure-cold-start.mjs` (P10), `measure-service.mjs` (P1, P2), `write-tokens-css.mjs`, `open-gallery.mjs` | run by Node | F-801, F-802, F-803 |

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
  the session (asks first with unsaved changes and names them). Each shows its Claude Code and CLI equivalents
  (`CASE_ACTIONS` in `src/shared/case.ts`).
- The page reaches the case operations only through `window.studioCase` (`state`, `sources`, `prepare`, `attach`,
  `status`, `conflict`, `save`, `create`, `duplicate`, `check`, `forget`), from one module, `src/renderer/case/door.ts`;
  a third MessagePort on `studio.case-port`, checked by `src/main/case/bridge.ts` (exact fields, paths `cases/<...>.toml`
  or `.json` without `..`). The ipcMain channels stay `app.info` and `app.ready`.
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
  solver.alpha_u", "Redo: remove boundaries.lid"; "Nothing to undo" when there is none), Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y
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
  which, then confirms). F5 and Ctrl+. are the egui Studio's Run and Stop keys. The table also lists the keys of the
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
- IPC: the page sees only `window.studio` (today `app.info()` and `app.ready()`), one function per channel of
  `src/shared/ipc.ts`. The main process registers exactly those channels and refuses calls from any frame that is not on
  `app://studio/`. New channels are added to the contract, the preload and the main handlers together, and the e2e
  test's allow-list.
- The solver service: the page sees `window.studioService` (`info`, `call`, `state`, `onState`, `offState`,
  `subscribe`, `unsubscribe`), not a port or a socket. The main process hands the preload script a MessagePort after
  each load of an app page; the preload keeps it and forwards. The main process checks every message on it
  (`src/main/service/bridge.ts`): the exact fields of each operation, a method on `SERVICE_METHODS`
  (`src/shared/service.ts`) and parameters of plain JSON values; anything else is refused before the service. The
  shell's ipcMain channels stay `app.info` and `app.ready`. A method is added to the list with a test.
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
