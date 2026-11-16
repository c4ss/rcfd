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
- D6, paused and cancelled runs: `DESIGN.md` has no pictogram for them; shown as words only until F-806.
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
| `src/shared/service.ts` | The service port's contract: the allow-list of service methods, the port messages, `window.studioService` | as above | F-803 |
| `src/renderer/` | Renderer (browser code only, no Node types): React, three.js, React Aria Components | `vite build` -> `dist/renderer/` | F-802 onwards |
| `src/renderer/design/` | `tokens.ts`, the one source of the `DESIGN.md` tokens (colour roles, type ramp, spacing, sizes, lines, focus, motion, pictogram grid); `tokens.css`, generated from it | as above | F-802 |
| `src/renderer/ui/` | The primitives of `DESIGN.md` Components on React Aria Components: buttons, toggles and segmented toggles, text and number fields, select, switch, tabs (sub and project), table, slider, tooltip, state pictograms and icons; `primitives.css` | as above | F-802 |
| `src/renderer/project/` | The project home (`ProjectHome.tsx`, `project.css`) and its one door to `window.studioProject` (`door.ts`) | as above | F-805 |
| `src/renderer/dev/` | The primitives gallery at `#/dev/primitives` (loaded on demand): every primitive and the tokens, with sample labels only | as above | F-802 |
| `src/renderer/field/` | The field viewport: `FieldViewport.tsx` (loading through the data door, legend, caption, readout), `FieldCanvas.tsx` (WebGPURenderer, the TSL material, pick, grid, keyboard, P8 bench), `colormap.ts`, `surface.ts`, `grid.worker.ts`, `Legend.tsx`, `field.css`; its development page is `src/renderer/dev/FieldDev.tsx` at `#/dev/field` | as above | F-808 |
| `src/renderer/data/` | `door.ts`: the renderer's one door to `window.studioData` (F-804's bulk data) | as above | F-808 |
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
- `RCFD_STUDIO_STARTUP_REPORT=<file>` (measurement only): the main process writes the time the project view was ready
  to that file and quits. Unset, it does nothing.

## Rules

- Guardrails: `docs/studio-guardrails.md` (G1 to G21). Product: `PRODUCT.md`. Visual system: `DESIGN.md`.
- Every dependency is pinned to an exact version (`.npmrc` sets `save-exact`), the lock file is committed, and each
  dependency is listed in `THIRD_PARTY_LICENSES.md` (run `python scripts/third_party_licenses.py` from the repository root
  after any change to `package-lock.json`) and justified in an ADR (ADR 0801 and its addenda).
- The design reference is `apps/studio-prototype/` (synthetic data). Nothing is copied from it into this app; the
  surfaces are built from `DESIGN.md` through their own roadmap items.
