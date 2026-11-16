# rcfd Studio (Electron)

The Electron desktop front end of rcfd: F-448 and the Studio roadmap of ADR 0800 (F-800 to F-832). This directory is
outside the Cargo workspace (ADR 0350 Q2, guardrail G5). The engine is Rust and stays Rust (G2); this app is a client of
it.

**State: F-801 hardened shell.** The window shows an empty project view and nothing else, inside the hardened shell of
guardrail G10, with Playwright tests that drive the packaged app. There is no service client, no project folder access
and no surface yet. Dependencies and layout: ADR 0801 (accepted) and its addendum 1 (F-801).

    npm ci                    # exact versions from package-lock.json
    npm run typecheck         # renderer (tsconfig.json), main + preload + shared (tsconfig.node.json), tests (tsconfig.test.json)
    npm run build             # typecheck, then vite build (renderer) and tsc (main, preload, shared) into dist/
    npm test                  # build, package into out/packaged/, then every Playwright test (unit and e2e); no human step
    npm run test:unit         # the unit tests only (needs dist/ for the built index.html check)
    npm run package:dir       # package dist/ by hand into out/packaged/ (rcfd-studio.exe; resources/app)
    npm run bench:cold-start  # P10, 10 cold starts of the packaged copy; a heavy record, run by hand only
    npm start                 # electron . (run npm run build first); downloads the Electron binary on first use

`npm test` opens the Studio window for a few seconds on Windows (Electron has no headless mode there); it needs no input.
On Linux it needs a display (CI uses a virtual one, F-823).

## Layout

| Path | Process | Built by | Filled in by |
|---|---|---|---|
| `src/main/` | Electron main process (ES module): the window, the `app://` protocol, the session's security settings, the IPC handlers; later all service and file I/O | `tsc -p tsconfig.node.json` -> `dist/main/` | F-801 (hardened shell), F-803 (service client), F-804 (bulk data), F-805 (project folder) |
| `src/main/security.ts` | The security policy as values and pure functions (CSP, own-URL and network rules, protocol path resolution), unit-tested without Electron | as above | F-801 |
| `src/preload/` | Preload script (CommonJS `.cts`, because a sandboxed preload cannot be an ES module): `window.studio`, the IPC allow-list | `tsc -p tsconfig.node.json` -> `dist/preload/` | F-801, F-803 |
| `src/shared/ipc.ts` | The typed IPC contract shared by all three processes: channel names, arguments, replies | `tsc -p tsconfig.node.json` -> `dist/shared/` | F-801, F-803 |
| `src/renderer/` | Renderer (browser code only, no Node types): React, three.js, React Aria Components | `vite build` -> `dist/renderer/` | F-802 onwards |
| `tests/unit/` | Pure tests of `security.ts` and the built `index.html` | run by Playwright (project `unit`) | F-801 |
| `tests/e2e/` | The packaged shell driven by Playwright's `_electron.launch` | run by Playwright (project `electron`) | F-801 onwards |
| `scripts/` | `package-dir.mjs` (packaging by hand), `measure-cold-start.mjs` (P10) | run by Node | F-801 |

`out/`, `dist/`, `test-results/` and `node_modules/` are build output and never committed.

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
- `RCFD_STUDIO_STARTUP_REPORT=<file>` (measurement only): the main process writes the time the project view was ready
  to that file and quits. Unset, it does nothing.

## Rules

- Guardrails: `docs/studio-guardrails.md` (G1 to G21). Product: `PRODUCT.md`. Visual system: `DESIGN.md`.
- Every dependency is pinned to an exact version (`.npmrc` sets `save-exact`), the lock file is committed, and each
  dependency is listed in `THIRD_PARTY_LICENSES.md` (run `python scripts/third_party_licenses.py` from the repository root
  after any change to `package-lock.json`) and justified in an ADR (ADR 0801 and its addenda).
- The design reference is `apps/studio-prototype/` (synthetic data). Nothing is copied from it into this app; the
  surfaces are built from `DESIGN.md` through their own roadmap items.
