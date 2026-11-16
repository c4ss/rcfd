# rcfd Studio (Electron)

The Electron desktop front end of rcfd: F-448 and the Studio roadmap of ADR 0800 (F-800 to F-832). This directory is
outside the Cargo workspace (ADR 0350 Q2, guardrail G5). The engine is Rust and stays Rust (G2); this app is a client of
it.

**State: F-801 hardened shell, F-802 design tokens and primitives, F-803 service client.** The window shows an empty
project view inside the hardened shell of guardrail G10, with Playwright tests that drive the packaged app. The design
tokens of `DESIGN.md` and the primitives (on React Aria Components) are in place, shown on a primitives gallery. The main
process is a client of the solver service (`rcfd-service/1`): it attaches to a running `rcfd serve` or starts its own,
long-polls its events and reconnects when it restarts; the renderer reaches it only through `window.studioService` and
an allow-list of methods. There is no project folder access and no surface yet. Dependencies and layout: ADR 0801
(accepted), addendum 1 (F-801) and addendum 2 (F-803); F-802 added no dependency.

    npm ci                    # exact versions from package-lock.json
    npm run typecheck         # renderer (tsconfig.json), main + preload + shared (tsconfig.node.json), tests (tsconfig.test.json)
    npm run build             # typecheck, then vite build (renderer) and tsc (main, preload, shared) into dist/
    npm test                  # build, package into out/packaged/, then every Playwright test (unit and e2e); no human step
    npm run test:unit         # the unit tests only (needs dist/ for the built index.html check)
    npm run package:dir       # package dist/ by hand into out/packaged/ (rcfd-studio.exe; resources/app)
    npm run tokens:css        # regenerate src/renderer/design/tokens.css from tokens.ts (a unit test fails when stale)
    npm run gallery           # open the packaged app on the primitives gallery at 1440x900 (view-and-report)
    npm run bench:cold-start  # P10, 10 cold starts of the packaged copy; a heavy record, run by hand only
    npm run bench:service -- --bench-server=<f447_bench>   # P1 and P2 through the shell; heavy, by hand only
    npm start                 # electron . (run npm run build first); downloads the Electron binary on first use

`npm test` opens the Studio window for a few seconds on Windows (Electron has no headless mode there); it needs no input.
On Linux it needs a display (CI uses a virtual one, F-823). The service tests of `tests/e2e/service.spec.ts` need the
`rcfd` binary (`cargo build --release -p rcfd-cli`; found through `RCFD_BIN`, `$CARGO_TARGET_DIR/release` or a
`target/release` above this folder); without it they are skipped and say why.

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

## Layout

| Path | Process | Built by | Filled in by |
|---|---|---|---|
| `src/main/` | Electron main process (ES module): the window, the `app://` protocol, the session's security settings, the IPC handlers; later all service and file I/O | `tsc -p tsconfig.node.json` -> `dist/main/` | F-801 (hardened shell), F-803 (service client), F-804 (bulk data), F-805 (project folder) |
| `src/main/service/` | The service client (no Electron import, unit-tested in Node): `rpc.ts` (framing, ids, time limits), `client.ts` (connections, events loop, reconnection), `spawn.ts` (starting and finding `rcfd serve`), `config.ts`, `bridge.ts` (the checks on the service port) | as above | F-803 |
| `src/main/security.ts` | The security policy as values and pure functions (CSP, own-URL and network rules, protocol path resolution), unit-tested without Electron | as above | F-801 |
| `src/preload/` | Preload script (CommonJS `.cts`, because a sandboxed preload cannot be an ES module): `window.studio`, the IPC allow-list | `tsc -p tsconfig.node.json` -> `dist/preload/` | F-801, F-803 |
| `src/shared/ipc.ts` | The typed IPC contract shared by all three processes: channel names, arguments, replies | `tsc -p tsconfig.node.json` -> `dist/shared/` | F-801 |
| `src/shared/service.ts` | The service port's contract: the allow-list of service methods, the port messages, `window.studioService` | as above | F-803 |
| `src/renderer/` | Renderer (browser code only, no Node types): React, three.js, React Aria Components | `vite build` -> `dist/renderer/` | F-802 onwards |
| `src/renderer/design/` | `tokens.ts`, the one source of the `DESIGN.md` tokens (colour roles, type ramp, spacing, sizes, lines, focus, motion, pictogram grid); `tokens.css`, generated from it | as above | F-802 |
| `src/renderer/ui/` | The primitives of `DESIGN.md` Components on React Aria Components: buttons, toggles and segmented toggles, text and number fields, select, switch, tabs (sub and project), table, slider, tooltip, state pictograms and icons; `primitives.css` | as above | F-802 |
| `src/renderer/dev/` | The primitives gallery at `#/dev/primitives` (loaded on demand): every primitive and the tokens, with sample labels only | as above | F-802 |
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
