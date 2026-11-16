# ADR 0801: The dependencies and the directory layout of apps/studio-web

Status: ACCEPTED by the maintainer 2026-11-16 (answers in section 7). Follow-up of [ADR 0800](0800-electron-studio-roadmap.md) for
F-800. ADR 0800 §5 named the prototype's dependency list as "the starting point for F-800's justification, not an approval
of it"; this ADR is that justification (guardrail G5). F-800 stays `partial` until the maintainer accepts it or answers
the questions in §6.

## 1. Context

- The maintainer chose the stack on 2026-11-16 (PRODUCT.md, Stack; ADR 0800 §2): three.js on `WebGPURenderer` with TSL
  for every picture, React 19 + Vite + react-three-fiber, React Aria Components as the accessible widget base, the app in
  `apps/studio-web/`. The go-ahead for F-448 is ADR 0800 §6 Q1.
- G5: each npm dependency pinned to an exact version, the lock file committed, licence AGPL-3.0 compatible and listed,
  each one justified in the ADR that adds it, the count kept small; the JavaScript outside the Cargo workspace. No native
  Node addon without the maintainer.
- G10 (Electron security) and G13 (testable without eyes) are F-801's work; G14 (`rcfd-studio` frozen) means no Rust
  change here.
- The design prototype `apps/studio-prototype/` pins react 19.3.1, three 0.187.0, @react-three/fiber 9.8.2,
  react-aria-components 1.22.1, @fontsource/archivo 5.3.0, vite 8.3.8, @vitejs/plugin-react 6.3.0, typescript 7.0.2 and
  @types/*. The measurement shell `bench/electron/` (F-447) pins electron 45.1.0, the version every number of ADR 0350
  addendum 1 was measured on.

## 2. Decision (proposed): the direct dependencies

Fourteen direct packages: six that ship in the app, eight for building and type checking. Every version is exact in
`package.json` (no `^` or `~`), `.npmrc` sets `save-exact=true` so a later `npm install <pkg>` stays exact, and
`package-lock.json` (lockfile version 3, 119 packages, every one resolved from registry.npmjs.org with an integrity hash)
is committed. All versions are the ones the prototype already builds with, except `electron` (from the spike) and
`@types/node` (new, see below). The versions were the latest releases on 2026-11-16 apart from `electron` (45.3.3 is out,
§6 Q1) and `@types/node` (the latest of major 24 on purpose; 26.6.6 is the newest).

### Shipped in the app (`dependencies`)

| Package | Version | Licence | Why it | Alternatives considered |
|---|---|---|---|---|
| `react` | 19.3.1 | MIT | The maintainer's UI choice (PRODUCT.md, Stack). React 19 is the version react-three-fiber 9 and React Aria Components support. | Preact (smaller, but r3f and React Aria assume React), Solid, Svelte, plain DOM: each rules out r3f and React Aria, which the maintainer chose. |
| `react-dom` | 19.3.1 | MIT | React's DOM renderer; always the same version as `react`. | None: required by React for a DOM app. |
| `three` | 0.187.0 | MIT | Every 2D and 3D picture (maintainer). `three/webgpu` gives `WebGPURenderer` with its built-in WebGL2 fallback, `three/tsl` the TSL node materials; both are entry points of this one package. The spike's WebGPU draw (P7, P8) is the evidence it is fast enough. | Raw WebGPU (no fallback, every picture by hand), Babylon.js (larger, a second scene graph to learn), regl or deck.gl (2D or data-viz only, no general 3D). |
| `@react-three/fiber` | 9.8.2 | MIT | Declares the three.js scene inside React so the viewport, the run bands and the panels share one component tree; per-frame updates (scrubbing, live fields) still mutate three.js objects outside React's render (PRODUCT.md, Stack). Version 9 is the React 19 line. | Imperative three.js inside one React component (possible, and what per-frame paths do anyway; loses declarative scene setup). |
| `react-aria-components` | 1.22.1 | Apache-2.0 | The accessible widget base (maintainer; G8, WCAG 2.2 AA): keyboard, focus and screen-reader behaviour of buttons, lists, tables, dialogs, sliders and menus, unstyled, so DESIGN.md supplies the look. | Radix UI (fewer widgets: no table, date or number field with locale handling), Ariakit, headless hand-written widgets (the accessibility work is the cost the library removes). |
| `@fontsource/archivo` | 5.3.0 | OFL-1.1 | DESIGN.md sets Archivo with tabular figures for all text. Fontsource packages the font files with CSS so they are self-hosted and bundled: no remote content (G10), and the licence is a file in the package (G1). | Font files copied into the repository by hand (same licence, but no version pin and no update path); Google Fonts by URL (remote content, ruled out by G10). |

### Build and type checking only (`devDependencies`)

| Package | Version | Licence | Why it | Alternatives considered |
|---|---|---|---|---|
| `electron` | 45.3.3 | MIT | The shell (ADR 0350 Q5 (a)). The spike measured every limit of G9 on 45.1.0; the maintainer chose the 45.3.3 security patch for F-800 (Q1 (b)), and F-801 measures P10 again on it; it is in the current stable line (45.0.0 was released 2026-10-21; 43, 44 and 45 are the supported majors), which G10 asks for. It bundles Chromium 156 and Node 24.22.1. The npm package is small; it downloads the binary on first use, so `npm ci` stays fast. A dev dependency by Electron's own convention: the packager (F-820) ships the binary, not the npm package. | Tauri (system web view: WebGPU support differs per platform, and the measurements of ADR 0350 do not carry over), NW.js (smaller community, no measured numbers). |
| `vite` | 8.3.8 | MIT | The maintainer's choice; builds the renderer (`vite build`) and serves it in development with hot reload. Brings `rolldown` (bundler), `lightningcss` (MPL-2.0, CSS, build only) and `postcss`. | webpack (slower, more configuration), esbuild alone (no dev server with React refresh), Parcel. |
| `@vitejs/plugin-react` | 6.3.0 | MIT | JSX and React fast refresh in Vite; the version that matches Vite 8. | `@vitejs/plugin-react-swc` (adds a native SWC binary for no measured gain at this size). |
| `typescript` | 7.0.2 | Apache-2.0 | Type checking of all three processes, and the compiler of the main and preload scripts (no bundler for them, §3). 7.x is the native compiler; the prototype already type-checks with it. | TypeScript 6.x, the JavaScript compiler (slower; the fallback if a later tool needs the JavaScript API, §6 Q3). |
| `@types/react` | 19.3.0 | MIT | Types for React 19. | None. |
| `@types/react-dom` | 19.3.0 | MIT | Types for react-dom 19. | None. |
| `@types/three` | 0.187.0 | MIT | Types for three.js; same minor version as `three`. | None (three.js ships no types). |
| `@types/node` | 24.19.2 | MIT | Types of the Node API the main process uses (`node:url` now; files and sockets with F-803 to F-805). Major 24 matches the Node 24 inside Electron 45, and `electron` itself depends on `@types/node ^24.9.0`, so the lock file holds one copy. Pinned directly so the types the main process is checked against do not drift with Electron's range. | Relying on the copy `electron` brings transitively (works, but the version would float inside its range). |

Not added: `@react-three/drei` (PRODUCT.md allows it "where it earns its place"; nothing needs it yet, and the item that
does adds it with its own justification), Playwright (F-801), a packager such as electron-builder or Electron Forge
(F-820), a test runner (F-801 or F-802 decides), ESLint or Prettier (no house decision for TypeScript style yet), and any
native Node addon (G5).

### Transitive packages and their licences

The lock file holds 119 packages, including the platform-specific optional binaries of rolldown, lightningcss and the
TypeScript 7 compiler for every platform. Licences as npm records them: MIT 68, Apache-2.0 32, MPL-2.0 12 (lightningcss and
its platform packages, build only), BSD-3-Clause 2, ISC 2, 0BSD 1, BlueOak-1.0.0 1 (`graceful-fs`, build only), OFL-1.1 1.
All are compatible with AGPL-3.0: the permissive ones directly, MPL-2.0 through its section 3.3, OFL-1.1 because the fonts
are distributed with the software, not sold by themselves. Nothing is GPL-only, unlicensed or without a recorded licence.
The full list, the notices that need care (the Electron binary's `LICENSES.chromium.html`, the OFL text, TypeScript's
`NOTICE.txt`) and the per-package table are in `THIRD_PARTY_LICENSES.md`.

`scripts/third_party_licenses.py` now generates that section from `apps/studio-web/package.json` and
`package-lock.json`, standard library only, so `--check` also reports a stale npm list. The Rust section is unchanged
apart from the header paragraph that names both. The prototype and `bench/electron/` are not listed: neither is a product
and neither ships.

## 3. Decision (proposed): the directory layout

```
apps/studio-web/
  package.json, package-lock.json, .npmrc (save-exact), .gitignore (node_modules/, dist/), README.md
  tsconfig.json        renderer: DOM types, no Node types; what a bare `tsc --noEmit` checks
  tsconfig.node.json   main + preload: Node types, NodeNext modules, emits to dist/
  vite.config.ts       renderer build: root src/renderer, base './', out dist/renderer
  src/main/            Electron main process, an ES module   -> dist/main/index.js   ("main" in package.json)
  src/preload/         preload script, CommonJS (.cts)       -> dist/preload/index.cjs
  src/renderer/        index.html, main.tsx, the empty project view -> dist/renderer/
```

- **Three processes, three directories.** Main, preload and renderer are separate trust zones under G10, so they are
  separate source trees with separate type environments: the renderer is checked without Node types, so a Node call there
  fails to compile. F-801 fills `src/main` (hardened window settings, CSP, IPC allow-list) and `src/preload` (the
  allow-listed bridge); F-803 to F-805 add the service client, bulk data and project folder to `src/main`; F-802 onwards
  fill `src/renderer`.
- **Main is an ES module, preload is CommonJS.** Electron supports ES-module main processes; a sandboxed preload cannot be
  an ES module, so it is a `.cts` file that TypeScript emits as `.cjs`. The renderer is an ES module bundled by Vite.
- **tsc for main and preload, Vite for the renderer.** Main and preload are small and need no bundling now; compiling them
  with `tsc` avoids a second build tool (electron-vite, or a Vite library build per process). F-804's TypeScript reader of
  `store.rts` (R2) lives in `src/main` and is compiled the same way. If a later item needs bundling (for example to
  package fewer files), it changes this with its own justification.
- **One npm package, no workspaces.** Shared types between main and renderer (IPC messages, F-801 and F-803) go in a
  `src/shared/` directory that both tsconfigs include, added when the first one is needed; npm workspaces are not needed
  for one app.
- **Outside the Cargo workspace.** `Cargo.toml` lists its members explicitly under `crates/`; `apps/` is not one, so
  `cargo` never sees this directory and `crates/rcfd-cli/tests/workspace_graph.rs` is unaffected. `node_modules/` and
  `dist/` are ignored and never committed.
- **The skeleton.** The renderer renders an empty project view placeholder ("No project open", a sentence that says
  nothing is connected yet, and the early-stage disclaimer of G6). The main process opens one window with Electron's
  defaults (sandbox on, context isolation on, Node integration off, as the smoke run in §5 read back) and sets only the
  preload path; it asserts nothing and has no Content Security Policy, so Electron prints its development security
  warning. Hardening is F-801. Nothing is copied from the prototype.

## 4. Pin and update policy (proposed)

- **Exact pins, committed lock.** Every direct dependency exact in `package.json`; `npm ci` (never `npm install`) in CI
  and for verification, so the lock file is the build's input.
- **An update is a change like any other.** It changes `package.json` and `package-lock.json` together, regenerates
  `THIRD_PARTY_LICENSES.md`, and passes `npm ci`, the type checks, `vite build` and, once they exist, the Studio tests
  (F-801, F-823). A new direct dependency, or a new major version of one, needs an ADR entry with the columns of §2. A
  patch or minor update of an existing one needs only the regenerated licence list and a passing build, unless its licence
  changes (then the maintainer).
- **Pairs move together.** `react` with `react-dom` and `@types/react*`; `three` with `@types/three`; `vite` with
  `@vitejs/plugin-react`; `@types/node` with the Node major inside the pinned Electron.
- **Electron stays supported (G10).** Electron supports its latest three stable majors and releases a major about every
  eight weeks, so 45 stays supported until about the release of 48. The Studio moves to a new Electron major before its
  pinned one leaves support, and re-runs the G9 benchmark (F-817) when it does.
- **Licence check on every change.** A package whose recorded licence is missing, GPL-only, or not compatible with
  AGPL-3.0 is not added without the maintainer.

## 5. Verification (2026-11-16, Windows 11, Node 22.19.0, npm 11.12.0)

In `apps/studio-web/`:

- `npm ci` (after deleting `node_modules/` and `dist/`): `added 69 packages, and audited 70 packages in 21s`, `found 0
  vulnerabilities` (69 = the 119 lock entries minus the optional binaries of other platforms).
- `npx tsc --noEmit` (renderer): exit 0, no output. `npx tsc --noEmit -p tsconfig.node.json` (main, preload): exit 0.
- `npx vite build`: `vite v8.3.8`, 18 modules transformed, `dist/renderer/index.html` 0.32 kB, one JS chunk 220.06 kB
  (gzip 68.86 kB), one CSS file 2.45 kB, twelve Archivo font files (latin, latin-ext, vietnamese; weights 400 and 700),
  `built in 125ms`. `npm run build` (both type checks, `vite build`, `tsc -p tsconfig.node.json`) passes the same way.
- `npx tsc -p tsconfig.node.json` emits `dist/main/index.js` and `dist/preload/index.cjs`.
- A one-off smoke run (a script outside the repository that imports `dist/main/index.js` under Electron 45.1.0 and reads
  the page back after load) reported `h1` "No project open", `document.fonts.check("12px Archivo")` true, `typeof require`
  in the page "undefined", and web preferences `sandbox: true, contextIsolation: true, nodeIntegration: false`, with
  Electron's insecure-CSP warning as expected before F-801. This is not a committed test; F-801 adds the Playwright one.

In the repository: `python scripts/third_party_licenses.py --check`: `THIRD_PARTY_LICENSES.md is up to date`.
`cargo test -p rcfd-cli --release --test workspace_graph`: `4 passed; 0 failed`. `cargo test -p rcfd-cli --release --test
docs_site`: `2 passed; 0 failed`. No Rust file, test or golden changed.

## 6. Open questions for the maintainer

- **Q1, Electron 45.1.0 or the latest 45 patch.** 45.3.3 was published on 2026-11-16; Electron patches carry Chromium
  security fixes.
  (a) Keep 45.1.0 for F-800 (the measured version) and move to the latest 45.x patch in F-801, where the cold-start limit
  P10 is measured again anyway. (b) Move to 45.3.3 now. (c) Keep 45.1.0 until a new major.
  Recommendation: (a). The skeleton loads no remote content, and F-801 re-measures P10 on whatever version it pins.
- **Q2, the update cadence and Dependabot.** `.github/dependabot.yml` covers `cargo` and `github-actions` only.
  (a) Add an `npm` entry for `/apps/studio-web` (weekly, grouped so the pairs of §4 move together), landed with the CI
  job of F-823 so each update pull request is built and tested. (b) Update by hand when an item needs it, plus a
  monthly security check (`npm audit`). (c) Both.
  Recommendation: (a), with F-823; until then (b).
- **Q3, TypeScript 7 (the native compiler) or 6.x.** The prototype and this skeleton type-check with 7.0.2. Some tools
  that need TypeScript's JavaScript API (for example typescript-eslint) may lag behind 7.
  (a) Keep 7.0.2. (b) Use 6.x until the tooling is settled.
  Recommendation: (a). Nothing in the plan needs the JavaScript API yet; if F-801 or F-823 adds a tool that does, that
  item records the change.
- **Q4, the licence list for npm.** `THIRD_PARTY_LICENSES.md` now lists all 119 lock entries, including build-only and
  other-platform packages, generated by the existing script.
  (a) Keep the full list (what is done here; it mirrors the Rust section, which lists every package on every target).
  (b) List only the packages that ship (runtime dependencies and the Electron binary) and summarise the rest.
  Recommendation: (a): one rule for both ecosystems, and `--check` keeps it true.
- **Q5, accept this ADR.** Accepting it accepts the fourteen dependencies of §2, the layout of §3 and the policy of §4,
  and lets F-800 be marked done.

## 7. Answers of the maintainer (2026-11-16)

- **Q1:** (b), move to Electron 45.3.3 now. Applied in F-800: `npm install --save-exact --save-dev electron@45.3.3`,
  then `npx tsc --noEmit`, `npm run build` and `python scripts/third_party_licenses.py --check` pass. The G9 numbers of
  ADR 0350 were measured on 45.1.0; F-801 measures P10 again on 45.3.3.
- **Q2:** add the weekly, grouped npm Dependabot entry for `/apps/studio-web` together with the CI job of F-823; update
  by hand until then.
- **Q3:** keep TypeScript 7.0.2.
- **Q4:** keep the full npm licence list (every lock-file entry).
- **Q5:** ADR 0801 accepted: the 14 direct dependencies, the layout, the pin and update policy, and the change to
  `scripts/third_party_licenses.py`. F-800 is done.

## Addendum 1: F-801 dependencies (2026-11-16)

F-801 (the hardened shell and its test harness, guardrails G10, G13 and P10 of G9) adds one direct dependency under the
policy of §4 (a new direct dependency needs an ADR entry with the columns of §2; this is that entry). Nothing else
changed in §2: the fourteen packages keep their versions. The maintainer may review this addendum like any ADR text;
F-801 proceeded on it because the dependency is development-only and removing it is one `npm uninstall`.

### The new direct dependency (`devDependencies`)

| Package | Version | Licence | Why it | Alternatives considered |
|---|---|---|---|---|
| `@playwright/test` | 1.65.1 | Apache-2.0 | G13 and F-801's acceptance ask for "a Playwright test that drives the packaged shell". `_electron.launch` starts the packaged executable, gives the renderer as a page (DOM, locators, `page.evaluate`) and evaluates code in the main process (`electronApplication.evaluate`), which the G10 checks need on both sides of the bridge. The same runner runs the pure unit tests (project `unit`), so no second runner is added. It drives the Electron it is given and downloads no browser (`npx playwright install` is never run). 1.65.1 was the latest release on 2026-11-16 (published 2026-11-13; a patch of 1.65.0 of 2026-11-05). | WebdriverIO with `wdio-electron-service` (a larger tree: WebDriver, ChromeDriver for the bundled Chromium); Spectron (deprecated since 2022); Puppeteer over the remote-debugging port (renderer only: no main-process evaluation, and it downloads Chrome unless told not to); a hand-written CDP client under `node --test` (no locators or assertions; the code the library replaces). |

It brings two packages: `playwright` 1.65.1 and `playwright-core` 1.65.1, both Apache-2.0, neither with an install
script or a native binary. The lock file now has 122 entries (Apache-2.0 35, the rest as in §2) and
`THIRD_PARTY_LICENSES.md` is regenerated (`python scripts/third_party_licenses.py`). Pairs (§4): the three move together;
`@playwright/test` pins `playwright` exactly.

**No unit-test runner.** The unit tests (`tests/unit/`, pure checks of `src/main/security.ts` and the built
`index.html`) run under `@playwright/test` too; Node's built-in `node --test` would have been the alternative at zero
dependencies, but one runner and one report are simpler. A component-test runner (for example Vitest for React
components in F-802) would be justified by the item that needs it.

### What F-801 built (so the dependency is read in context)

- **`src/main/security.ts`** (pure, no Electron import): the scheme `app://studio/`, the Content Security Policy
  (`default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self';
  object-src 'none'; base-uri 'none'; form-action 'none'`, plus `frame-ancestors 'none'` in the header only, since a
  `<meta>` cannot carry it), the network rule (cancel http, https, ws, wss, ftp, file and foreign `app://` hosts) and the
  protocol handler's path resolution (no way out of `dist/renderer`).
- **`src/main/index.ts`**: `app.enableSandbox()`; the window with `sandbox`, `contextIsolation`, `webSecurity` on and
  `nodeIntegration`, `nodeIntegrationInWorker`, `nodeIntegrationInSubFrames`, `allowRunningInsecureContent`,
  `webviewTag`, `experimentalFeatures`, `navigateOnDragDrop` off; the renderer served by `protocol.handle('app')` with
  the CSP header on every response, and the header set again by `webRequest.onHeadersReceived`; `onBeforeRequest`
  cancelling the network rule; `will-navigate`, `will-frame-navigate` and `will-redirect` refused for anything but
  `app://studio/`; `setWindowOpenHandler` denying every `window.open`; `will-attach-webview` refused; permission
  requests, checks and device permissions denied. IPC: one `ipcMain.handle` per channel of the allow-list, refusing
  senders whose frame is not on `app://studio/`, each handler checking its own arguments.
- **`src/shared/ipc.ts`**: the typed allow-list (`IpcContract`, `CHANNELS`, `StudioBridge`). Two channels: `app.info`
  (name and versions) and `app.ready` (the renderer reports that the empty project view is in the document and one
  animation frame has passed; used by P10). F-803 adds the service client's channels here.
- **`src/preload/index.cts`**: `window.studio.app.{info, ready}` through `contextBridge`, nothing else (no
  `ipcRenderer`, no generic invoke).
- **`src/renderer/index.html`** carries the policy in a `<meta>`; `vite.config.ts` sets `assetsInlineLimit: 0` (no
  `data:` URLs) and no module-preload polyfill, so `'self'` is enough.
- **The packaged shell**: `scripts/package-dir.mjs` packages by hand as ADR 0350 did for P10 (Electron's `dist` copied
  to `out/packaged/`, the executable renamed `rcfd-studio.exe` so `app.isPackaged` is true, the app's `package.json` and
  `dist/` in `resources/app`). No packaging dependency; installers, signing, fuses and ASAR are F-820's.
- **Tests**: `tests/unit/security.spec.ts` (9 tests) and `tests/e2e/shell.spec.ts` (12 tests on the packaged shell):
  the window's effective web preferences (`getLastWebPreferences`, not public in Electron's types) and the OS sandbox of
  the renderer process (`app.getAppMetrics().sandboxed`, Windows and macOS); no `require`, `process`, `module`,
  `Buffer` or `ipcRenderer` in the page; the bridge is exactly `app.{info, ready}` and frozen; the main process handles
  exactly the allow-listed channels (read from Electron's private `ipcMain._invokeHandlers`, Electron's own all-capitals
  internal handler set aside) and refuses foreign senders and unexpected arguments; the CSP header and meta present and
  equal to `security.ts`; inline script, `eval`, `Function` and string timers blocked; remote fetch, image, stylesheet
  and frame blocked by the policy; navigation to https, file and foreign `app://` URLs (by `location` and by a followed
  link) refused; `window.open` denied for remote, own and `about:blank` URLs; notifications, geolocation, camera and
  microphone denied; and the network layer cancelling remote loads that the main process itself starts
  (`ERR_BLOCKED_BY_CLIENT`).
- **npm scripts**: `build` (three type checks, `vite build`, `tsc` for main, preload and shared), `test` (`build`, then
  `package:dir`, then `playwright test`: everything, no human step), `test:unit`, `package:dir`, `bench:cold-start`.
  `tsconfig.test.json` is a third type environment, for the tests and `playwright.config.ts`.

Mutations (each applied to a scratch copy of `src/`, rebuilt, packaged, and the e2e project run; then restored): window
opening allowed, the CSP meta removed, the CSP header removed in both places, `sandbox: false`, permission checks
granted, the network rule disabled, `ipcRenderer` exposed to the page, an extra channel registered, and both navigation
handlers removed: each makes exactly one e2e test fail, the one that names it. Removing only one CSP header source, or only
`will-navigate`, is not caught: the other layer still holds (`onHeadersReceived`, `will-frame-navigate`), as intended.

### P10 on Electron 45.3.3 (run once by hand, house rule 7)

Method of ADR 0350 §4: packaged by hand (`scripts/package-dir.mjs`), `scripts/measure-cold-start.mjs` takes the time
from `spawn` to the main process's receipt of `app.ready` (the empty project view in the document, one animation frame
later; the main process writes the time to `RCFD_STUDIO_STARTUP_REPORT` and quits), 10 starts one second apart, judged
on the median.

| Id | Limit | Measured (Electron 45.3.3, product shell, empty project view) | Met | ADR 0350 (45.1.0, spike, run list) |
|---|---|---|---|---|
| P10 | <= 2.5 s | median 476 ms; first start 953; max 989; the ten: 953, 427, 387, 460, 484, 429, 589, 848, 989, 476 | yes | median 383; first 1173; max 6747 |

Set-up: 2026-11-16 12:55 UTC, Windows 11 (10.0.26300), AMD Ryzen 7 9800X3D (16 logical), 93.6 GB, launcher on Node
22.19.0. **Deviation:** the machine was busy: processor busy fraction 0.87 before the run and 1.00 after it, with 3
`cargo`/`rustc`/`link` processes of another job running throughout (ADR 0350's P10 ran at 0.49 with none). The numbers
are therefore an upper side of what an idle machine gives; the run was not repeated (a registered run is made once).
A true cold start after a reboot was not taken (an agent cannot reboot), as in ADR 0350. The run appears in no test:
`npm test` never times anything.

### Verification (2026-11-16, Windows 11, Node 22.19.0, npm 11.12.0)

Recorded in the F-801 commit message and the report of the job; summary: `npm ci` clean, the three type checks exit 0,
`npm run build` passes, `npm test` 21 passed (9 unit, 12 e2e), `python scripts/third_party_licenses.py --check` up to
date, `cargo test -p rcfd-cli --release --test docs_site --test workspace_graph` passes. No Rust file, test or golden
changed.

### Notes for later items (no decision needed now)

- **Permissions are denied wholesale**, including clipboard writes from the renderer. An item that needs one (copying a
  CLI command, G17) allows that one permission in `security.ts` with a test.
- **The default application menu stays**, so the developer tools open with Ctrl+Shift+I in the packaged copy too
  (`devtools://` is not on the blocked list). Whether a release build removes them, and which Electron fuses it sets
  (RunAsNode, the inspector arguments that Playwright's launch uses, ASAR integrity), is F-820's.
- **Windows appear during `npm test`** on Windows: Electron has no headless mode there. "Headless" here means no human
  step; on Linux CI the job of F-823 runs under a virtual display.
- **Refused navigations leave Playwright waiting.** After a navigation is prevented, Playwright's locators wait for a
  navigation that never finishes, so the tests read the page with `page.evaluate` from that point on.

## Addendum 2: F-803, the service client (2026-11-16)

F-803 (the service client for control and events; guardrails G2, G4, G7, G9 P1 and P2, G10, G21) adds **no
dependency**: the client uses Node's `net`, `child_process`, `fs` and Electron's `MessageChannelMain`. Section 2 and
addendum 1 are unchanged, and `package-lock.json` did not change. No Rust file, service method, reply shape, golden or
default changed (G4); `rcfd serve` and the spike's server (`crates/rcfd-api/examples/f447_bench.rs`) were only built and
run.

### What F-803 built

- **`src/main/service/rpc.ts`**: one connection of `rcfd-service/1` over TCP or over any pair of streams (the stdio of
  `rcfd serve --stdio`): newline framing kept as bytes until the newline (a UTF-8 character cut between two chunks is
  decoded whole), increasing integer ids, replies matched by id, a time limit per request (a late reply is dropped),
  JSON-RPC errors kept with their code, message and data, pending requests rejected when the connection ends; a reply
  line over 512 MiB, or a line that is not JSON, ends the connection.
- **`src/main/service/client.ts`**: two TCP connections to the service, one for calls (answered in the order sent) and
  one that only long-polls `events.poll {after, timeout_ms: 15000, limit: 500}`, so a waiting poll never holds a call up.
  The first connection starts the cursor at the head of the log, found with instant polls by doubling and halving
  `after` (about 2 log2(head) requests): events from now on. When a connection drops, calls fail at once with
  `disconnected` and the client tries again after 100 ms, doubling to 5 s. After a reconnection it tells a restarted
  service from the same one by the last event it saw (the same instance still has it under its sequence number; a new
  one has a shorter log or another event there; in spawn mode a new process id settles it): a new instance resets the
  cursor to 0 and raises `instance`; events dropped from the bounded log meanwhile are reported as `missed`. Everything
  is asynchronous; nothing waits on the main process's thread.
- **`src/main/service/spawn.ts`**: `rcfd serve --addr 127.0.0.1:0 [--root <dir>]`, detached, its output appended to
  `service.log` (a pipe would break when the Studio exits), the address read from the line it prints; `service.json`
  records address and process id, so the next start attaches to the same service.
- **`src/main/service/config.ts`**: the service from the environment: `RCFD_STUDIO_SERVICE` (attach; loopback addresses
  only), `RCFD_STUDIO_RCFD` (start and own `rcfd serve`), `RCFD_STUDIO_SERVICE_ROOT`, `RCFD_STUDIO_STATE_DIR`; neither
  of the first two: `off`.
- **The renderer's way in** (`src/shared/service.ts`, `src/main/service/bridge.ts`, `src/preload/index.cts`): after each
  load of an app page (`dom-ready`, URL on `app://studio/`) the main process hands the preload script one MessagePort on
  `studio.service-port`; the preload keeps it and exposes `window.studioService` (`info`, `call`, `state`, `onState`,
  `offState`, `subscribe`, `unsubscribe`). Every reply is a value `{ok, result}` or `{ok: false, error: {code, message,
  data?}}`, never a thrown error. The main process checks every message: the exact fields of its operation, a method on
  the allow-list `SERVICE_METHODS`, parameters of plain JSON values (depth at most 64, at most 10^6 values), a time limit
  from 1 ms to 10 min. The allow-list is 14 read methods (`service.info`, `settings.schema`, `session.list`,
  `settings.get`, `settings.validate`, `mesh.check`, `run.status`, `monitors.get`, `reports.get`, `field.stats`,
  `log.tail`, `journal.get`, `panel.snapshot`, `data.info`) and 10 control methods (`session.create`, `session.close`,
  `settings.set`, `settings.remove`, `run.initialize`, `run.start`, `run.pause`, `run.resume`, `run.stop`, `run.step`).
  Left out on purpose: `events.poll` (the main process owns it), the calls that hold the connection until a run or a
  study ends (`run.wait`, `workflow.run`, `run.headless`, `design.*`, `journal.replay`), bulk data (F-804), file output
  and co-simulation; the comment on `SERVICE_METHODS` gives the reason for each.
- **Quitting** (G7): a service the Studio started is stopped only if `session.list` shows no session; otherwise it is
  left running and found again at the next start (`adopted: true`). An attached service is never stopped. A reload of
  the renderer does not touch the connections.
- **Tests** (no human step): `tests/unit/service-rpc.spec.ts` (framing, ids, out-of-order replies, errors, time limits,
  closing, in-memory streams and a stdio child), `service-client.spec.ts` (configuration, back-off, `findHead`, the
  client against `tests/support/fake-service.ts`: head of the log, a long poll not holding calls, immediate failure while
  disconnected, protocol refusal, a dropped connection, two kinds of restart, missed events), `service-bridge.spec.ts`
  (the allow-list, `parseRequest` on accepted, refused and malformed messages, the bridge over Node's MessageChannel);
  `tests/e2e/service-security.spec.ts` (the packaged Studio attached to the fake service, which records every line it
  receives: the bridge's exact shape, two connections only, refused methods and malformed arguments never arriving, the
  page unable to open a connection of its own by fetch, WebSocket, image, EventSource or beacon, the port never visible
  in the page across a reload, the shell's ipcMain channels unchanged, session filtering, every listed method forwarded);
  `tests/e2e/service.spec.ts` (the real `rcfd serve`: every listed method but `data.info` is in its `service.info`;
  `--stdio`; a `service.info` round trip from the renderer; the `created`, `initialized` and three `iteration` events of
  a session in the renderer; kill and restart on the same port, the cursor following the new instance; a started
  service killed and replaced; a running run surviving a reload and the Studio's quit, and found again; an idle started
  service stopped at quit). 62 new tests (43 unit, 19 e2e); F-801's 21 unchanged and passing.
- **Mutations** (applied in place, rebuilt, packaged, both e2e files run, then restored and compared byte for byte): the
  allow-list check removed (3 e2e tests fail, the two refusal tests among them; with the check gone `run.wait` reached
  the service and held the call connection, which made the restart test fail too); the cursor kept on a new instance
  (the restart test fails); a started service always stopped at quit (both G7 tests fail).

### Decisions taken in F-803 (reversible; the maintainer may change them)

- **D1, how the renderer reaches the service.** F-801's `tests/e2e/shell.spec.ts` pins the ipcMain handlers to
  `app.info` and `app.ready` and `window.studio` to `{app}`, and house rule 2 forbids editing an existing test.
  (a) A MessagePort per page, checked in main, behind a second global `window.studioService` (done). (b) `ipcMain.handle`
  channels `service.*` under `window.studio`, with F-801's allow-list constants extended, which needs the maintainer's
  word on the test edit. Recommendation: (a). It leaves ipcMain to the shell, carries event batches as a stream rather
  than one IPC call per event, and is the arrangement ADR 0350 measured P2 on. If (b) is wanted, the bridge's functions
  keep their signatures and only the transport changes.
- **D2, where the service comes from.** (a) The environment variables above, until F-820 (packaging) decides where a
  packaged Studio finds `rcfd` and F-805 whether a project names one (done). (b) Look for `rcfd` on `PATH` by default.
  Recommendation: (a): nothing is started that the user did not name.
- **D3, a started service at quit.** (a) Stop it only when it holds no session, else leave it running and attach to it
  at the next start (done). (b) Always stop it: a run in it would end with the window, against G7. (c) Never stop it:
  idle processes pile up. Recommendation: (a).
- **D4, one ordered call connection.** A slow call (`run.initialize` of a large case, `run.step` with a large count)
  delays the calls behind it. (a) Keep one ordered connection now and revisit with the first surface that makes long
  calls: a second connection for slow methods, or a pool where order does not matter (done). (b) A pool now, which
  loses the order of calls the renderer sends without waiting for each. Recommendation: (a).
- A limit, not a decision: over stdio the client works (tested against `rcfd serve --stdio`), but the Studio uses TCP,
  because a long-poll on the single stdio stream would hold every call behind it (the service answers one stream's
  lines one after the other).

### P1 and P2 through the product shell (run once by hand, house rule 7)

Method of ADR 0350 A1.4, with the Studio's own client in place of the spike's: `scripts/measure-service.mjs` launches
the packaged Studio attached to the spike's server (`f447_bench serve`: the same `Service` and `serve_tcp` as `rcfd
serve`, with `data.*` on and an in-process long-poll that writes `seq kind unix_ns` of every event to a file; the event
log carries no time stamp, which is why ADR 0350 used it too). P1: in the Studio's main process through its
`ServiceClient` (the call connection), a ready session of the 1e5-cell box, 50 warm-up calls, then 1000 timed calls of
each method; the same calls from the renderer through `window.studioService` are recorded beside it. P2: that session
running (`run.start`), the renderer subscribed through the port; the latency of each `iteration` event is the renderer's
receive time minus the server's stamp, the two clocks joined by 200 `data.info` round trips through the Studio before
and after the sample; the first 300 events. One difference from the spike: there the renderer long-polled through the
port; here the main process owns the long-poll and pushes each batch, which is what the product does.

| Id | Limit | Measured (Electron 45.3.3, product shell) | Met | ADR 0350 (45.1.0, spike) |
|---|---|---|---|---|
| P1 | p50 <= 0.5 ms, p99 <= 2 ms (main process) | `service.info` p50 0.114, p99 0.302, max 0.732; `run.status` p50 0.072, p99 0.255, max 0.374 (1000 each) | yes | `service.info` p50 0.056, p99 0.272; `run.status` p50 0.038, p99 0.132 |
| P1, renderer | recorded | `service.info` p50 0.3, p99 0.5, max 1.5; `run.status` p50 0.3, p99 0.4, max 1.0 (1000 each; the renderer's clock steps by 0.1 ms) | recorded | not taken |
| P2 | p99 <= 20 ms | 300 `iteration` events in 75.2 s: p50 0.35, p99 1.17, max 1.62, min -0.52; clock join round trip 0.1 ms before and after, drift 0.48 ms over the sample, so p99 <= 1.7 at worst; no event missed, none without a stamp | yes | p50 0.0, p99 0.80, max 0.92; drift 1.30 |

Set-up: 2026-11-16 13:37 UTC, Windows 11 (10.0.26300), AMD Ryzen 7 9800X3D (16 logical), 93.6 GB (42.5 GB free),
launcher on Node 22.19.0, Electron 45.3.3 (Chromium 156.0.8078.100, Node 24.22.3), `f447_bench` built from this branch
(`cargo build --release -p rcfd-api --example f447_bench`; protocol 1.2, because `data.*` is on). **Deviation:** the
machine was busy: processor busy fraction 0.91 before the run, with 8 `cargo`/`rustc`/`link` processes of other jobs,
and 0.81 after it, with 1; ADR 0350's run had none. The numbers are an upper side of what an idle machine gives; the run
was not repeated (a registered run is made once). The run appears in no test: `npm test` never times anything. The raw
results stay in the scratch folder, not committed.

### Notes for later items

- **Windows handle inheritance.** A child process on Windows inherits every inheritable handle of its parent, and Node
  cannot spawn without that. A service the Studio starts therefore holds the Studio's own standard streams open: when
  the Studio was started with pipes (Playwright does that) and the service is left running, whoever waits for those
  pipes to close waits for the service. The tests wait for the Studio's process instead (`quitLeavingService`). A
  launcher that starts the Studio and reads its output (a future `rcfd studio` command, the Claude Code bridge) would see
  the same; F-820 can start the service through a helper that passes no handles on, or the maintainer can allow a
  native addon (G5).
- `RCFD_STUDIO_BENCH=1` exposes the client to Playwright's main-process evaluate, for the measurement only, as
  `RCFD_STUDIO_STARTUP_REPORT` does in addendum 1.
- The renderer shows nothing of the service yet; F-802 onwards build the surfaces on `window.studioService`. The
  Studio's named actions (G16, G20) will sit on top of the allow-list, not replace the checks in main.

## Addendum 3: F-804, bulk field data by R2 and R1 (2026-11-16)

F-804 (bulk field data by the R2 reader and the R1 route, bit for bit; guardrails G2, G3 with P12, G9 with P4, G10,
G21) adds **no dependency**: the readers use Node's `fs`, `zlib`, `net` and `worker_threads`, and Electron's
`MessageChannelMain`. Section 2 and addenda 1 and 2 are unchanged, and `package-lock.json` did not change. No Rust file,
service method, reply shape, golden or default changed (G4); the spike's example `crates/rcfd-api/examples/f447_bench.rs`
was only built (`cargo build --release -p rcfd-api --example f447_bench`) and run, to make the data sets and to serve
`data.*`. The spike's `bench/electron/io/tsstore.js` and `rpc.js` were ported, not imported; `bench/` is unchanged.

### What F-804 built

- **`src/main/data/tsstore.ts`** (route R2): the reader of `store.rts` (ADR 0124), ported from the spike and brought to
  the checks of the Rust `Reader` (`crates/rcfd-tsstore/src/lib.rs`): the header; every frame head it reads; the payload
  checksum of every SNAP, FOOT and DATA frame; the footer index with its consistency checks or, without a valid footer,
  the same lenient scan of the frame heads (it stops at a short or damaged tail) and `refresh` for a live run; the
  snapshot directory checks (`check_dir`, step and time against the index); the size limits of JSON and data frames;
  raw and shuffle-deflate blocks, `f32` and `f64` precision, returned as `f32` (`as f32`, round to nearest). Two things
  the spike left out (ADR 0350 addendum 1, QA2) are in: the scan of an unfinished store, and the payload checksum of the
  MESH frame before `surface` uses it (streamed, once per open store). `surface` triangulates the boundary faces as
  `data.surface` does. It does not rebuild the mesh's content hash (`Reader::mesh` does; that needs Rust's mesh code, G2).
- **`src/main/data/worker.ts`, `local.ts`**: the reader runs in a `worker_threads` worker of the main process; requests
  go to it one at a time, and results come back with their ArrayBuffers transferred (no copy between the threads).
  Before each request the worker looks at the file again: a store that grew is refreshed, one that was replaced is
  opened anew. `local.ts` only checks the path against the data roots and passes messages; the main thread never reads a
  store.
- **`src/main/data/roots.ts`**: which folders R2 may read. A local store is named by an absolute path and is read only
  when its real path (links and junctions resolved) is inside a root. Roots: `RCFD_STUDIO_DATA_ROOTS` (the platform's
  path delimiter), and `DataRoots.allow(dir)` for F-805 when a project is opened. Relative paths, `..` segments, NUL and
  device paths are refused before the file system is asked.
- **`src/main/data/remote.ts`** (route R1): `data.open`, `data.field` and `data.surface` through F-803's
  `ServiceClient` (the call connection), and one more TCP connection, per service instance, to the data port that
  `data.info` names. The bytes are checked against the sections the call announced (name, dtype, count, byte length); an
  unknown ticket, a wrong length or a time-out closes the data connection, and the next request opens a new one. One
  request at a time: the service keeps at most 8 tickets and drops the oldest. A service without `data.*` (`rcfd serve`
  today) answers `no_data`.
- **The renderer's way in** (`src/shared/data.ts`, `src/main/data/bridge.ts`, `port.ts`, `src/preload/index.cts`):
  after each load of an app page the main process hands the preload script a second MessagePort, on
  `studio.data-port`; the preload keeps it and exposes `window.studioData` (`open`, `field`, `surface`, `cancel`). Each
  answers a value `{ok, result}` or `{ok: false, error: {code, message, data?}}`, never a thrown error; `field` gives a
  `Float32Array`, `surface` a `Float32Array` and two `Uint32Array`s. A store is `{route: 'local' | 'service', path}`. The
  main process checks every message: the exact fields of its operation, a known route, bounded strings, a non-negative
  integer snapshot, a time limit from 1 ms to 10 min, at most 256 waiting requests per window. `cancel(tag)` answers
  every waiting request with that tag `cancelled` at once; a queued one never runs, a running one finishes and its
  result is dropped. `window.studio`, `window.studioService`, `SERVICE_METHODS` (`data.field` stays refused there) and
  the shell's ipcMain channels are unchanged.
- **Tests** (no human step; they make their data afresh on every run):
  - `tests/unit/data-routes.spec.ts` (18 tests) makes the tiny box (120 cells) and the 1e5-cell box of ADR 0350 with
    `f447_bench gen` (the raw store from the run path, the deflated copy from the Rust writer, and the reference bytes:
    `Reader::read_field` cast to `f32`). In Node, R2 through its worker and R1 through a real service (`f447_bench
    serve`) return every value of every field of every snapshot equal to the references bit for bit (XOR and population
    count); the surfaces of R2 and R1 are equal bit for bit; NC2 (one block rotated by a byte is reported); M3 (a wrong
    unshuffle stride makes all 80 deflated fields differ, in the reader and through the worker); a damaged header,
    frame head, data payload and mesh payload are refused by both readers; four kinds of unfinished store (a torn tail,
    a zero-filled tail, no footer, a damaged trailer) are scanned to the same snapshots as the Rust reader scans them; a
    growing store is picked up as `Reader::refresh` does; a replaced store is read anew; errors carry their reason;
    `rcfd serve` answers `no_data`.
  - `tests/unit/data-bridge.spec.ts` (12 tests): the port's checks, the data roots (a junction out of a root is
    refused), the queue's cancellation, the time limits, the pending limit.
  - `tests/e2e/data.spec.ts` (8 tests, the packaged Studio attached to `f447_bench serve`): the bridge's exact shape;
    both routes describe the same store; **P12 in the renderer**: all 80 fields of the 1e5 box by R2 and by R1, raw and
    deflated (320 fields, 3.2e7 values), SHA-256 computed in the page equal to the references', and the differing bits
    counted in the page for every snapshot of `p` (0), with NC2; the surfaces equal; the refusals (shape, outside the
    roots, relative and `..` paths, the service's own sandbox: 1004); cancellation; a reload (a new port, never visible
    to the page).
  - 38 new tests; the 155 of `npm test` pass, the earlier ones unedited.
- **Mutations** (applied in place, rebuilt, the unit file run, then restored): the reader's DATA payload checksum check
  removed (the damaged-checksum test fails); the worker reading snapshot + 1 (the first P12 test fails).
- **`scripts/measure-data.mjs`** (`npm run bench:data`): P4 and, with `--p12`, P12 over the 1e6 and 4e6 boxes, through
  the packaged Studio; by hand only.

### Decisions taken in F-804 (reversible; the maintainer may change them)

- **D5, how field values reach the page.** Electron's `MessagePortMain.postMessage` transfers only ports, so an
  ArrayBuffer from the main process to a renderer is always one structured-clone copy (ADR 0350 A1.3 measured with it).
  (a) A second MessagePort per page, kept in the preload script, values as ArrayBuffers, handed to the page as typed
  arrays through `contextBridge`, which copies once more inside the renderer process (done). (b) The data operations on
  F-803's service port behind `window.studioService`: its exact seven-function shape is pinned by an existing test
  (house rule 2), and 4 MB replies would queue with the event batches. (c) Transfer the port into the page's world, as
  the spike did, which saves the `contextBridge` copy: the page would hold a raw port, against D1 of addendum 2 and its
  test that the page never sees one. (d) A `SharedArrayBuffer`: needs cross-origin isolation and cannot be shared
  between the main and the renderer process anyway. Recommendation: (a); P4 below is met with it, so the second copy
  (about a millisecond for 4 MB) is not worth a weaker boundary. Inside the main process the worker-to-main hop is a
  true transfer.
- **D6, where R2 runs.** (a) A worker thread (done). (b) Asynchronous `fs` and `zlib` on the main thread: the FNV-1a
  checksums and the unshuffle would still run there, 10 to 30 ms per 1e6 field and about 0.4 s for the mesh checksum at
  1e6 (measured in Node on this machine, busy), stalling every window's IPC meanwhile. Recommendation: (a), G21.
- **D7, which local folders R2 reads.** (a) Data roots: `RCFD_STUDIO_DATA_ROOTS` now, the project folder (F-805)
  later, real paths compared (done). (b) Any absolute folder that holds a `store.rts`. (c) Only folders under the
  service's root. Recommendation: (a): a compromised page reads no more than the open project's stores, and R2 works
  without a service.
- **D8, the mesh checksum in R2.** (a) Verify the MESH frame's payload checksum once per open store before its first
  surface (done; about 0.4 s for the 195 MB frame of the 1e6 box, in the worker). (b) Skip it, as the spike did (its P7
  by R2 was 55 ms). Recommendation: (a) (G3: a damaged mesh must not draw). If P7 (a later item) needs the time, a
  checksum cached beside the store by the run path is ADR 0350 QA5's question.
- **D9, R1 one request at a time.** (a) Serial, so the service's limit of 8 tickets never drops one of ours (done).
  (b) Up to 7 calls in flight. Recommendation: (a) until scrubbing over R1 (P5) needs more.
- Progress (G21): a field or a surface is one request; the page shows progress per request and cancels by tag. There
  are no progress events inside a request (the surface of a large fan mesh may need them; a later item).

### P4 through the product shell (run once by hand, house rule 7)

Method of ADR 0350 A1.4 with the Studio's own data path in place of the spike's: `scripts/measure-data.mjs` launches
the packaged Studio attached to the spike's server (`f447_bench serve`, `data.*` on) with `RCFD_STUDIO_DATA_ROOTS` on the
data folder, makes a WebGPU device and one 4 MB storage buffer in the page before any timing, and times in the renderer
`window.studioData.field(store, i mod 20, 'p')` on the 1e6-cell box, then `queue.writeBuffer` and
`queue.onSubmittedWorkDone()`; 20 warm-up and 200 timed repeats per route and codec. The data sets were made afresh by
`f447_bench gen` (the run path took 20 steps in 4.3 s, 119.8 s and 681.0 s for the 1e5, 1e6 and 4e6 boxes; raw stores
51.6 MB, 514 MB and 2053 MB, deflated copies 43.2 MB, 418 MB and 1701 MB, as in ADR 0350; no non-finite value). Beside
each line (G21, recorded, not judged): the longest gap of a 1 ms interval timer in the main process (17.3 ms when idle:
the timer's own resolution on Windows) and the renderer's long tasks (over 50 ms).

| Id | Limit | Measured (Electron 45.3.3, product shell) | Met | ADR 0350 (45.1.0, spike) |
|---|---|---|---|---|
| P4 R2 | p50 <= 30, p99 <= 60 ms | raw: p50 21.7, p99 32.5, max 34.7; deflated: p50 33.1, p99 71.1, max 210.1 | raw yes; deflated no | raw: p50 14.2, p99 22.4; deflated: p50 24.0, p99 30.9 |
| P4 R1 | p50 <= 30, p99 <= 60 ms | raw: p50 47.0, p99 104.2, max 110.2; deflated: p50 62.5, p99 94.7, max 95.1 | **no** (both) | raw: p50 25.9, p99 57.7; deflated: p50 31.0, p99 66.1 |
| G21 (recorded) | | main timer's longest gap: R2 18.1 (raw) and 21.5 (deflated), R1 36.8 and 32.4; renderer long tasks: none, except one of 50.0 ms during R1 raw | | not taken |
| P12 | 0 differing bits | 0 differing fields of 400 (6.4e8 values): the 1e6 box, all four fields of all 20 snapshots, and `p` of the 4e6 box, by R2 and R1, raw and deflated; SHA-256 of the values the page received against SHA-256 of the Rust reader's reference bytes | yes | 0 of 6.72e8 values (bits counted) |

**P4 is met through the product shell by route R2** (the limit is "R1 or R2", judged on the raw store, ADR 0350 §3 and
A1.4), the route ADR 0350 QA2 (a) chose for projects on this machine. **R1 missed it in this run**, raw and deflated.
Diagnosis (after the run, Node against the same server, 50 repeats, not a registered run): of R1's raw time, the
`data.field` call (the Rust reader opening the store, decoding to `f64` and casting) took p50 10.8 ms, p99 13.6, and the
4 MB over the data connection p50 16.9, p99 55.8; that is p50 27.3 and p99 67.0 before any IPC. So most of the miss is the
service's side and the loopback transfer at full processor load; the spike's R1 met with little room (p99 57.7 at a
load of 0.64), and this run had none. Two things the product adds to R1 over the spike, both small next to that: the data
connection copies the 4 MB twice in Node (the chunks joined, then into a buffer of their own; the spike wrote the chunks
into the destination once), and `contextBridge` copies once more in the renderer (D5). Nothing was tuned; the miss is
recorded. R2 deflated missed too (p99 71.1 and one repeat of 210 ms); the spike met it, but the deflated store is not
what the run path writes and is not judged.

Set-up: 2026-11-16 14:27 UTC, Windows 11 (10.0.26300), AMD Ryzen 7 9800X3D (16 logical), 93.6 GB (26.8 GB free),
launcher on Node 22.19.0, Electron 45.3.3 (Chromium 156.0.8078.100, Node 24.22.3), WebGPU adapter vendor `nvidia`,
architecture `blackwell` (the RTX 5080), the window visible with a content area of 1264 x 735 (the Studio's window is
1280 x 800 outside; the spike's content was 1280 x 800), `f447_bench` built from this branch. **Deviation:** the
machine was fully busy: processor busy fraction 1.00 before the run with 9 `cargo`/`rustc`/`link` processes of other
jobs, and 1.00 after it with 6 (ADR 0350's P4 ran at 0.64 with none). The numbers are an upper side of what an idle
machine gives; the run was not repeated (a registered run is made once). The summary line of the script judged "both
routes" when it ran; it was corrected afterwards to the registered rule ("R1 or R2"); the numbers come from its JSON
output. The run appears in no test: `npm test` never times anything. The raw results stay in the scratch folder, not
committed.

### Notes for later items

- **R1 at P4.** If R1 must meet P4 by itself (a remote service as the only source of a picture), the next steps, each to
  be measured: read the data connection into the destination buffer directly (one copy less in Node); a cached `Reader`
  per store in the service (a Rust change: `data.*` opens the store per call, A1.1); `f32` stores read without the `f64`
  round trip (Rust as well).
- **Scrubbing (P5) and the first picture (P7)** are later items' numbers; P7 by R2 now includes the mesh checksum (D8).
- **No progress inside a request**, and no prefetch: a scrubbing surface asks for the next snapshots itself and cancels
  by tag.

### Open questions for the maintainer

- **Q6, `rcfd serve --data` (ADR 0350 QA4).** Route R1 needs a service with `data.*` on, and `rcfd serve` has no
  switch for it (a flag changes the CLI help golden, so it needs the maintainer). Until then R1 works against
  `f447_bench serve` and any program that calls `Service::enable_data`, and answers `no_data` against `rcfd serve`.
  (a) Add `--data` to `rcfd serve` (a Rust change, with the golden edit approved); (b) leave R1 to embedding programs.
  Recommendation: (a), as ADR 0350 recommended. Not done here: F-804 changes no Rust.
- **Q7, R1 for a service on another machine.** F-803's configuration accepts loopback addresses only (G10), so "R1 for
  a remote service" reaches a service on this machine (or one behind an SSH tunnel). A service elsewhere needs a
  decision on transport security: the data connection is plain TCP without authentication, like the call connection.
  (a) Keep loopback only until that is designed; (b) allow other addresses now. Recommendation: (a).

### Verification (2026-11-16, Windows 11, Node 22.19.0, npm 11.12.0)

`npm ci` clean (72 packages added, 0 vulnerabilities; `package-lock.json` unchanged); `npm run typecheck` (the three
type checks) exits 0; `npm test` (build, package, every Playwright test) 155 passed, none skipped (103 unit, 52 e2e;
38 of them new); `python scripts/third_party_licenses.py --check` up to date; `cargo test -p rcfd-cli --release --test
docs_site --test workspace_graph` passes (2 and 4). No Rust file, test, golden or default changed; no existing test was
edited.

## Addendum 4: F-811, the residual and monitor plots (2026-11-16)

F-811 (residual and monitor plots; guardrails G3, G6, G9 P3) adds **no dependency**: the plots use `three` (already
here, section 2) and the React Aria primitives of F-802. `package-lock.json` did not change. No Rust file, service
method, reply shape, golden or default changed (G4); `rcfd serve` was only run.

### What F-811 built

- `src/renderer/plots/feed.ts`: one feed per run (service instance + session). The first `monitors.get {session}` is
  whole; every later one sends `since` = the f64 just below the last abscissa held (ADR 0353), so the samples at that
  abscissa come back, are checked against what the feed holds (same value, `Object.is`), and only the rest is appended.
  A mismatch or a shorter history (a run built again) reads the whole history again ("resync", counted). Series held at
  different abscissas are asked in groups with `names`; a series that appears later is read whole once. The run's
  events (`watchEvents` of the action layer) wake the feed, coalesced to at most one request per 100 ms; nothing polls
  on a timer. A plot only ever reads the snapshot of the run it was opened for; replies of another run are dropped by
  key.
- `range.ts`: the axis rule of G3. The range of every axis holds every value drawn, whatever the magnitude (5e-324 to
  1.7e308, ties, single values); a value that cannot be drawn (not finite, or not above zero on the log scale) breaks
  the line and is counted under the plot, never clamped onto an edge.
- `linePlot.ts`: the lines on three.js `WebGPURenderer` (WebGL2 where WebGPU is missing) with `LineSegments2` and
  `Line2NodeMaterial`. The buffers grow in place, and an increment uploads only its new segments (`addUpdateRange`).
  Coordinates are stored relative to an origin and a span set from the data, so the GPU's 32-bit floats place every
  finite f64 correctly (set again when the data outgrows them a thousandfold). Ticks, names, values and captions are
  HTML in Archivo beside the canvas (the canvas is `aria-hidden`).
- `ResidualPlot.tsx` (log scale, `continuity` and `res_*`), `MonitorPlot.tsx` (the other series, one at a time with a
  Select, the latest value at display size), `parts.tsx`, `words.ts` (empty states: a sentence and the run's state
  pictogram, for example "No residuals: session s2 is defined and has no simulation yet.").
- `src/renderer/dev/PlotsDev.tsx`: the development route `#/dev/plots[?session=..&case=..]`; F-810 places the plots in
  the time panel. It calls only `dispatch`.
- `src/shared/monitors.ts` and one hook in `src/main/service/bridge.ts` (decision D5 below).
- Tests: `tests/unit/plots-feed.spec.ts` (increments exact against `tests/support/fake-monitors.ts`, a stand-in with
  the service's `names`/`since` semantics; replies of two runs crossed; a rebuilt run resyncs; mutation checks: an
  off-by-one in the repeated-sample check and a disabled check both fail it), `plots-range.spec.ts` (4000 seeded random
  series, linear and log), `plots-css.spec.ts` (tokens only); `tests/e2e/plots.spec.ts` against the real `rcfd serve`
  with the quick-start cavity cases.

### The renderer: three.js, not SVG

PRODUCT.md (maintainer, 2026-11-16) names three.js on `WebGPURenderer` for every 2D and 3D visualisation, so three.js is
the default, and SVG needed a measured reason. The measurement (below, same session, same load) gives the opposite
reason. Four polylines of 1e5 points as one SVG took 134.6 ms from insertion to the next frame, and every append of 100
points (an SVG path's `d` is rebuilt whole) 104 to 128 ms, against an empty frame of 45.8 ms under that load. The
three.js plots drew 3e5 residual segments in 67.8 ms (render call 59.1 ms) the first time, and 3.5 ms and 3.8 ms after
an increment, because only new segments are uploaded. Reasons kept: (1) an increment costs its own size, not the
history's (G21); (2) the maintainer's stack, one renderer for plots and fields; (3) the WebGL2 fallback is built in.
react-three-fiber is not used for the plots: an orthographic scene with nothing to declare, and the per-frame path is
imperative anyway (PRODUCT.md, Stack).

### Decisions taken in F-811 (reversible; the maintainer may change them)

- **D5, `monitors.get` crosses the service port as `Float64Array`s.** The main process packs `x` and `y` of each series
  (a `null` of the wire becomes `NaN`) before the reply is posted; a reply of any other shape passes unchanged. Reason:
  in the smoke runs the structured clone of 8e5 JavaScript numbers through main, preload and the context bridge cost
  about 55 ms; typed arrays cost about 3 ms. (a) Pack (done). (b) Send the reply line as a string and parse it in the
  renderer (the spike's way; a second parse of 11 MB). (c) Plain arrays. Recommendation: (a). The action log (F-827)
  records parameters, not replies, so its exports are unchanged.
- **D6, line styles without hue for meaning.** Residuals are told apart by dash and weight in ink and secondary text,
  `res_p` dotted in the converged role; the sky accent is not used for a residual (DESIGN.md, colour has a job). The
  legend draws the same swatch. (a) As done. (b) A categorical palette. Recommendation: (a) until the time panel (F-810)
  shows how many series sit together.
- **D7, coverage.** `service:monitors.get` and the six `monitors:*` keys are mapped to a UI path worded "on the
  development route until F-810". (a) As done. (b) Keep their exemptions until F-810. Recommendation: (a): the path is
  built and tested, and the wording says where it is.

### P3 through the product shell (run once by hand, house rule 7)

Method of ADR 0350 A1.4 and ADR 0353 (check `p3` of bench/electron): `scripts/measure-plots.mjs` starts `rcfd serve`
(the release build in the shared target folder) and the packaged Studio attached to it, makes a 4 x 4 x 4 lid-driven box
with `[reports.lid_drag] type="force" patch="ymax" component="viscous"`, steps it 1e5 times (166.1 s), then from the
renderer sends `monitors.get {session, names}` for the first four series by name (`continuity`, `lid_drag`, `res_p`,
`res_u`, 1e5 samples each) through `window.studioService.call`, the call `dispatch` makes. Request to parsed object (the
reply arrives as `Float64Array`s), 20 repeats.

| Id | Limit | Measured (Electron 45.3.3, product shell) | Met |
|---|---|---|---|
| P3 | median <= 100 ms | median 118.3, max 147.8, min 96.0 (20) | **no** |
| P3, main process alone | recorded | median 95.7, max 112.5, min 90.2 (20) | recorded |
| I1 (ADR 0353), the last 100 samples of each series | median <= 2 ms | median 0.4, p99 1.1, max 6.3 (200 after 20 warm-up); 100 samples per series returned | yes |
| Plots, first whole fetch on `#/dev/plots` | recorded | 112.0 ms (the feed's first `monitors.get`, every series) | recorded |
| Plots, first draw (WebGPU) | recorded | residuals, 3e5 segments: 67.8 ms (render 59.1); monitor, 1e5 segments: 22.3 ms (render 21.2) | recorded |
| Plots, draw after an increment of 100 steps | recorded | 3.5 ms and 3.8 ms; 0 resyncs | recorded |

Set-up: 2026-11-16 15:22 UTC, Windows 11 (10.0.26300), AMD Ryzen 7 9800X3D (16 logical), 35.6 GB free, launcher Node
22.19.0, Electron 45.3.3 (Chromium 156.0.8078.100, Node 24.22.3). **Deviation:** the machine was fully busy: processor
busy fraction 1.00 before, during and after the run, with 4 `cargo`/`rustc`/`link` processes of other jobs; ADR 0350's
run had none. The miss is recorded, not tuned (ADR 0353). Most of the time is in the main process (95.7 ms: the
service's formatting of about 11 MB and the parse in main); the port adds about 20 ms. The run was not repeated; the raw
results stay in the scratch folder, not committed.

### Open questions for the maintainer

- **Q-811-1, P3 missed under load.** (a) Repeat the registered run once on an idle machine and judge P3 on it. (b) Accept
  the miss for F-811 and register a later item for a cheaper whole read (a binary series payload from the service is a
  protocol change, with an ADR of its own). (c) Raise the limit. Recommendation: (a), then (b) if it still misses.
  F-811's P3 clause stays open until then.
- **Q-811-2, abscissa of the compressible solver.** `crates/rcfd-api/src/sim.rs` (near line 3893) pushes `dt`, `courant`
  and `res_rho` against the step index while the document's `x_label` may be "time". The plots read one `x_label` per
  document, so these series would be placed and captioned against the wrong abscissa. A Rust change, not made here
  (G4). (a) The engine pushes them against the same abscissa as the other series. (b) The document gives an abscissa
  label per series. Recommendation: (a).
- **Q-811-3, runs on disk.** The plots read a live session's `monitors.get`; a finished run's `monitors.json` on disk is
  not read yet. Recommendation: the run list (F-806) hands the plots a reader of that file behind the same feed
  interface.

## Addendum 5 (F-808): the field viewport on three.js WebGPU (2026-11-16)

F-808 (mesh and fields on surfaces drawn by `WebGPURenderer` with a TSL material, the WebGL 2 fallback; guardrails G3,
G8, G9 with P7 and P8, G13, G21) adds **no dependency**: it is the first code to use `three` 0.187.0 (`three/webgpu`,
`three/tsl`, and `OrbitControls` from the package's own `examples/jsm`) and `@react-three/fiber` 9.8.2, both in §2 since
F-800. `package-lock.json` did not change. No Rust file, service method, reply shape, golden or default changed; no
existing test was edited. The spike's `bench/electron/` was read for the P7 and P8 methods, not copied; the prototype's
field view and `colormap.ts` were read for the approach, not copied (its stops are not rcfd-render's, see F808-D6).

### What F-808 built

- **`src/renderer/field/`**: `FieldViewport.tsx` (a store, a snapshot and a field: opens the store, reads the surface
  once per store and the field, all through the data door with the view's tag; progress with Cancel, errors with their
  reason and Try again; legend, caption, readout, mesh-grid toggle, Reset view, the back end named on screen),
  `FieldCanvas.tsx` (R3F `Canvas` with an async `gl` factory: `new WebGPURenderer({antialias: false, alpha: false,
  forceWebGL})`, `await init()`, linear output, no tone mapping; the TSL material; OrbitControls; the CPU pick; keyboard
  orbit, zoom and reset; the mesh grid; the P8 loop), `colormap.ts` (the table, the f32 mirror of the shader's
  arithmetic, tick labels, the readout's text), `surface.ts` (corner positions and values, the shown range, the grid's
  edges, the P8 surface), `grid.worker.ts`, `Legend.tsx`, `field.css`.
- **`src/renderer/data/door.ts`**: the one renderer module that names `window.studioData`, like `project/door.ts` for the
  project (F-805).
- **`src/renderer/dev/FieldDev.tsx`** at `#/dev/field?route=&path=&field=&snapshot=[&backend=webgl]` (loaded on demand;
  the project home's chunk does not contain three.js: `index` 582 649 bytes, `FieldDev` 1 569 087 bytes), and
  `#/dev/field?bench=p8[&uncapped=1]`. `main.tsx` gained the route.
- **Scripts**: `npm run field` (the viewport on a store at 1440 x 900, view-and-report), `npm run bench:field`
  (`scripts/measure-field.mjs`, P7 and P8, by hand).
- **Tests** (no human step; data made afresh by `f447_bench gen` on every e2e run; 26 new tests):
  - `tests/unit/field-colormap.spec.ts` (10): the reference is rcfd-render's map taken from the Rust source
    (`tests/support/rcfd-render-map.ts` reads the stops out of `crates/rcfd-post/src/render.rs` and checks, verbatim,
    the lines of `colormap` and of `ColorMap::eval`/`viridis` in `crates/rcfd-render/src/lib.rs` that it repeats). Every
    one of the 256 entries equals rcfd-render's colour at k / 255; entries 0 and 255 equal its colours at 0 and 1; values
    at and beyond both ends and non-finite values take rcfd-render's colours; at the table points the colour of a value
    is exactly rcfd-render's colour of that value; over a sweep of the full range the colour is within 3 levels of
    rcfd-render's colour of the value itself (the bound is reached); tick labels; the readout's text; the stylesheets
    use tokens only.
  - `tests/unit/field-surface.spec.ts` (4): corners carry the owner cell's value bit for bit; the shown range; the grid
    is exactly the boundary faces' edges (no fan diagonal); the P8 surface.
  - `tests/unit/data-door-static.spec.ts` (2).
  - `tests/e2e/field.spec.ts` (10, the packaged Studio, the tiny box and the 1e5-cell box by R2): the back end named
    (WebGPU here); every corner of every surface triangle against rcfd-render's map (worst 3 levels); **pixels read
    back** from screenshots: at grid points inside triangles the pixel equals the table colour of the stored value (the
    Rust reader's reference bytes) of the cell the hover readout names, and the viewport ground is exactly its token
    colour (nothing converts colours on the way), by WebGPU on both boxes and by WebGL 2 on the tiny one; the hover
    readout equals the stored value, bit for bit and as its shortest text; the legend bar's pixels are the table; the
    mesh grid by keyboard (white at 38% over the surface, compounding where lines meet; off restores every pixel); Reset
    view after a drag restores every pixel; keyboard orbit with the readout at the centre; no CSP violation, no console
    error; screenshots at 1440 x 900 in `test-results-visual/f808/`.

### Decisions taken in F-808 (reversible; the maintainer may change them)

- **F808-D1, how a value finds its colour.** The acceptance asks that every vertex colour equal rcfd-render's colour of the
  same value, and DESIGN.md and the item ask for a 256-entry table. Both cannot hold for every value: rcfd-render's map
  rounds a continuous piecewise-linear function and takes 635 distinct colours over [0, 1]. (a) The nearest of 256
  entries, entry k = rcfd-render's colour at k / 255 (done): exact at the 256 table points and at and beyond both ends,
  within 3 levels of 255 per channel elsewhere, a tolerance stated in `colormap.ts` and measured by the tests (G3 allows
  a tolerance stated before the code). (b) 257 entries holding the unrounded values in a float texture, the shader
  interpolating between two entries and rounding: equal to rcfd-render's colour except at f32 rounding ties, but not
  DESIGN.md's 256-entry table. (c) The shader evaluates the 9 stops as rcfd-render's shader does: equal likewise, no
  lookup table. Recommendation: (a), with the tolerance written into the acceptance text; (b) if "equals" must hold for
  every value.
- **F808-D2, where the data door lives.** `src/renderer/data/door.ts` (done), shared by every surface that reads bulk data
  (scrubbing, F-810), rather than inside `field/`.
- **F808-D3, point fields.** `data.surface` gives the used points in order of first use, not their numbers in the mesh, so
  a point field cannot be placed on the surface. (a) Draw cell fields only and say why for a point field (done);
  (b) add the point numbers to `data.surface` (a Rust and protocol addition, outside this item). Recommendation: (a)
  now, (b) as a follow-up when a store holds point fields (the run path writes cell fields).
- **F808-D4, the colour range.** The min and max of the shown values (the cells that own a surface triangle), as
  rcfd-studio's snapshot range (done). Range over the run and a typed range are rcfd-studio's other modes; not in
  F-808's acceptance.
- **F808-D5, picture settings.** Unlit, no MSAA, linear output and no tone mapping, so a pixel is exactly a table colour and
  testable (done). Edges are therefore aliased; MSAA would change only edge pixels, if wanted later.
- **F808-D6, the stops.** The Studio uses rcfd-post's 9 stops (`#440154`, `#482475`, `#414487`, `#355f8d`, `#2a788e`,
  `#22a884`, `#44bf70`, `#7ad151`, `#fde725`). The prototype's `colormap.ts` uses other samples (`#472d7b`, ...), which
  differ from rcfd-render's; DESIGN.md names only the two ends, which agree. Nothing to decide unless DESIGN.md should
  list the stops.
- **F808-D7, capability coverage.** No capability of `capabilities/inventory.json` gains a UI path: the viewport reads the
  store through the data port, which the inventory does not list, not `field.slice`, `field.grid`, `field.stats`,
  `field.export_vtk`, `render.png`, `render.glb` or the MCP tools `render_field`, `render_image`, `field_summary`,
  `get_field_stats`, `export_vtk`, whose exemptions name F-808 with the reason "not built yet".
  `capabilities/coverage.json` is unchanged here (its F-808 lines sit next to F-811's, which a parallel item changes).
  (a) At merge, reword those exemptions ("the viewport draws stored fields through the data port; this live-session
  method has no UI path yet") and keep F-808, or (b) move them to the item that will reach them. Recommendation: (a) at
  merge, then (b) when the roadmap names a home.

### P7 and P8 through the product shell (run once by hand, house rule 7)

`npm run bench:field -- --data=<folder> --bench-server=<f447_bench>` once, after `f447_bench gen <folder> tiny 1e5 1e6`
(made afresh; the run path took 196.7 s for the 1e6 box; its raw store is 491 MB). Method of ADR 0350 with the Studio's
viewport in place of the spike's code:

- P7: from the viewport's open of the store to the first animation frame after the draw finished on the GPU
  (`rcfd:field:first-picture`), through `data.open`, the surface and `p` of snapshot 19, the geometry, the material's
  pipeline and the draw. The renderer and device exist before (the viewport is open on the tiny box, and switches store
  between repeats); 5 repeats per route, judged on the median, the first reported. Also recorded: one cold mount of the
  viewport on the 1e6 box (renderer and device made inside the time).
- P8: `#/dev/field?bench=p8`: the viewport's canvas and material on 1e6 synthetic triangles (1000 x 500 x 2, a value per
  triangle), orbiting one turn per 600 frames; 120 warm-up and 1200 timed `requestAnimationFrame` intervals; once with
  vertical sync, once launched with `--disable-frame-rate-limit --disable-gpu-vsync`, each frame waiting for the GPU.

| Id | Limit | Measured (Electron 45.3.3, product shell, WebGPU) | Met | ADR 0350 (spike) |
|---|---|---|---|---|
| P7 R2 | median <= 1.5 s | median 71 ms; first 752 ms; all 752, 77, 67, 62, 71 | yes | median 55, first 65 |
| P7 R1 | median <= 1.5 s | median 2027 ms; all 2027, 2065, 2026, 1980, 2050 | **no** | median 1153, first 1580 |
| P7 cold mount (recorded) | | 134 ms (R2, the store already opened in the reader's worker) | | |
| P8, vertical sync | p99 frame <= 25 ms | 239.8 frames per s on a 238 Hz display; frame p50 4.20, p99 4.40, max 5.50 ms | yes | 60.0 per s, p99 16.9 |
| P8, no rate limit | >= 60 frames per s | 54.4 per s (frame p50 18.4, p99 19.6 ms); draw to GPU done p50 1.60, p99 5.50 ms | see below | 332 per s |

**P7 is met by route R2** (the limit is "R1 or R2", ADR 0350 §3), the route of projects on this machine. Its first
opening (752 ms) includes the reader's first open of the store with the MESH frame's checksum (D8 of addendum 3) and the first compile of the material. **R1 missed P7**
(about 2.0 s every time): the spike's R1 met with 1.15 s; this run was at full load (below), and `data.surface` decodes
and checks the whole stored mesh on the service's side on every call (ADR 0350, P7 R1 note). Not tuned; recorded.

**P8 is met**: with vertical sync the 1e6 triangles ran at the display's 238 Hz with a p99 frame of 4.40 ms, which is
also more than 60 frames per second. The run without a rate limit did not lift the frame rate in this set-up: launched
with the two switches on the command line (Electron reports both with `app.commandLine.hasSwitch`), even an empty
page's `requestAnimationFrame` ran at about 50 to 58 Hz, below the vertically synced 238 Hz; the spike set the switches
with `app.commandLine.appendSwitch` in its own main process, which the product's main process does not do. The work of
a frame (draw call to the GPU's end of it) took p50 1.60 ms, p99 5.50 ms. Deviation recorded, not tuned; a
measurement-only switch in the main process would let the run be repeated as the spike made it (open question).

Also measured (by hand, recorded): the hover pick (three's Raycaster over every drawn triangle, on the main thread)
takes p50 1.2 ms, max 6.3 ms per pointer move on the 1e5 box (26 000 triangles), p50 5.4 ms, max 10.6 ms on the 1e6 box
(120 000 triangles); a bounding-volume tree would be the next step for larger surfaces.

Set-up: 2026-11-16 15:31 UTC, Windows 11 (10.0.26300), AMD Ryzen 7 9800X3D (16 logical), 93.6 GB (35.4 GB free),
launcher on Node 22.19.0, Electron 45.3.3 (Chromium 156.0.8078.100, Node 24.22.3), WebGPU adapter vendor `nvidia`,
architecture `blackwell`, window content 1280 x 800, visible, a 238 Hz display. **Deviation:** the machine was fully
busy, processor busy fraction 1.00 before and after the run, with 4 `cargo`/`rustc`/`link` processes of other jobs.
`f447_bench` was the one in the shared target folder. The raw results stay in the scratch folder, not committed;
`npm test` times nothing.

### Notes for later items

- The project view (F-806) hosts the viewport: `<FieldViewport store snapshot field />`; scrubbing (F-810) can change
  `snapshot` alone, which loads one field and rewrites one vertex attribute (the geometry and the pipeline stay).
- Range modes, other colour maps, point fields (F808-D3) and cut planes are not in F-808.

## Addendum F-810: run bands and linked time scrubbing, the run workspace (2026-11-16)

Item F-810 of `docs/ROADMAP.md`. Traces G3, G9 (P5, P6), G12; `DESIGN.md` Layout, Run bands, One Live Thing.

### What F-810 built

- **The run workspace**, a third view of an open project ("Workspace", beside Home and Runs; "Open in workspace" in the
  Runs view's selected run brings the run there): the project's runs and the selected run's numbers on the left, its
  field (the viewport of F-808, unchanged but for two optional props) on the right, and the 248 px time panel of
  `DESIGN.md` under both: 200 px row labels | the shared axis | a 380 px monitor column.
- **Run bands**: one 20 px band per run on one shared axis; each end carries its state's pictogram and word on a white
  chip after the end, or inside it when there is no room. Sky iterating, a meadow end cap for converged, a red end cap
  for failed, silver hatching for queued and for the not yet computed part of a running run; orange for one live run
  only. The selected band is underlined in ink and its label bold; the others drop to 40% fill, a failure never. Grow-in
  700 ms ease-out staggered 90 ms (tokens), none under reduced motion.
- **One scrubber**: a deep ink line over bands, residual plot and axis, with its flag ("it 40", or "t = 1.25 s · it 145")
  in its own 18 px lane, flipping to the right near the start; 2 px orange while dragged or focused. It is an ARIA slider
  (arrow keys one stop, Page Up and Down ten, Home and End). It stops at the store's snapshots (a run without fields: at
  its residual samples). Field, residual cursor (`PlotFrame`'s new `cursor` prop, on the shared axis by its new `xAxis`
  prop), monitor readout (Display 30 px, unit from `output/monitors.json` or "no unit given") and snapshot counter move
  together. Transport: first, previous, Play (next stop once the one before is drawn), next; Follow live for a running
  run.
- **The run port**, `window.studioRun` (`src/shared/run.ts`, `src/main/run/bridge.ts`, `series.ts`): `series(run)` and
  `at(run, step)`, answered by the open project's read-only project server: `read_residuals` per series (at most 800
  samples each, so a reply stays under the engine's 36 000-byte text budget and is never written to a file), `read_residuals
  {since_step: step, max_points: 2}` for the exact values at the line, `read_project_file` for the units (at most 1 MB,
  hash checked across pages). `workspace/door.ts` is the one renderer module naming it (unit-tested).
- The open project folder is now a data root of the data door while it is open (`DataRoots.allow` was never called; the
  field of a project's run could not be opened from the project view before).
- `performance` marks `rcfd:scrub:request` and `rcfd:scrub:drawn`; `npm run bench:scrub` (`scripts/measure-scrub.mjs`).

### Decisions taken in F-810 (reversible; the maintainer may change them)

- F810-D1, the shared axis: (a) iterations (steps) for every run: the store's snapshots, `read_residuals` and
  `status.json` share them; (b) simulated time: `read_residuals` gives no time per sample (engine gap below); (c) wall
  time. Recommended and taken: (a); the flag and the band ends show the simulated time where the run has one.
- F810-D2, the source of residuals and monitors: (a) `read_residuals` of the project server, each series under 800
  samples, exact values at the line by `since_step`; (b) `output/monitors.json` through `read_project_file` (written
  only at the end of a run); (c) a new engine query. Taken: (a), with the units from (b).
- F810-D3, a fourth port `window.studioRun` instead of new operations on `window.studioProject`: the project port's key
  set is pinned by `tests/e2e/project.spec.ts`, which F-810 may not change.
- F810-D4, One Live Thing with several running runs: the selected run if it runs, else the newest running one; a paused
  run is not live (ink, not orange).
- F810-D5, converged and failed are drawn as 6 px end caps: the engine records no "converged since step".
- F810-D6, the workspace is a third project tab sharing the selected run with Runs; the Claude Code column of the
  prototype is left to F-814.
- F810-D7, Follow live reads the running run's store again every 250 ms after the last open answered (the run list is
  read once a second, `RUN_POLL_MS`); moving the line by hand stops following, the toggle brings it back.
- F810-D8, a finished run opens on its newest snapshot.

### P5 and P6 through the run workspace (run once by hand, house rule 7)

`npm run bench:scrub -- --data=<folder>` once (CARGO_TARGET_DIR set to the shared target folder), after
`f447_bench gen <folder> tiny 1e5 1e6 4e6` made afresh (the run path took 236.7 s for the 1e6 box, 1333.6 s for the 4e6
box; raw stores of 514 MB and 2.05 GB). The script names each box as a run of a project (`runs/r-20261116T000000-<size>`
with `run.json`, `status.json` and a `residuals.jsonl` of one line per stored step), opens it in the packaged Studio at
1440 x 900 and scrubs in the run workspace. Method of ADR 0350 through the whole product path: a key on the scrubber ->
the workspace's stop -> the viewport's read of field `p` of that snapshot through the data door (route R2) -> the GPU
write -> the draw -> the GPU done -> the next animation frame (`rcfd:scrub:drawn`); the next request as soon as the one
before is drawn. Each pass requests snapshot 1 (Home, from snapshot 20) and the 19 after it (ArrowRight); one warm-up
pass first; rate = requests / total time, frame = request to next request. Every request is checked: the workspace, the
counter and the residual cursor name the drawn snapshot's step (no error in any pass).

| Id | Limit | Measured (Electron 45.3.3, packaged, WebGPU) | Met |
|---|---|---|---|
| P5, 1e6 cells | >= 10 snapshots per s, no frame > 150 ms (10 passes) | 23.6 per s over 200 requests; frame p50 41.5, p99 75.1, max 91.6 ms; passes 755 to 985 ms; warm-up pass 1556 ms | yes |
| P6, 4e6 cells (recorded, not blocking) | >= 3 snapshots per s (5 passes) | 9.1 per s over 100 requests; frame p50 107.8, p99 159.2, max 175.5 ms; passes 2027 to 2465 ms; warm-up pass 2196 ms | yes (rate); 4 of 100 frames over 150 ms |
| Follow live (recorded; the task text's "P6") | none | a running quick-start cavity (a snapshot every 5 iterations, 101 committed in 30 s under load): 65 drawn, a draw every 426 ms (p50; max 1018); commit to drawn p50 105, max 328 ms; the drawn snapshot was the newest in the store at every draw | |

The live figures: commit = the snapshot seen in `store.rts` by the Studio's reader polling every 2 ms; drawn = the
viewport done and the next frame. Before F810-D7 the workspace learnt of new snapshots only from the run list (once a
second) and drew one in 1.3 s; with the 250 ms re-open it draws the newest snapshot every 0.26 to 0.43 s.

Set-up: 2026-11-16 17:07 to 17:10 UTC, Windows 11 (10.0.26300), AMD Ryzen 7 9800X3D (16 logical), 93.6 GB (32.4 GB
free), launcher on Node 22.19.0, Electron 45.3.3 (Chromium 156.0.8078.100, Node 24.22.3), WebGPU adapter vendor
`nvidia`, architecture `blackwell`, window content 1440 x 900. **Deviation:** the machine was fully busy, processor
busy fraction 1.00 before and after, with 6 to 7 `cargo`/`rustc`/`link` processes of other jobs. The raw results
(`full.json`) stay in the scratch folder, not committed; `npm test` times nothing.

### Missing in the engine (no Rust change here)

- `read_residuals` gives no simulated time per sample and no units (the units are in `output/monitors.json`, written
  when a run ends; a running run has none).
- `read_residuals` over its text budget writes `runs/<id>/mcp/residuals-*.json` even on a `--read-only` server; the
  Studio keeps every request under the budget and refuses a reply that names a file.
- No query for every value over a step range without decimation (the cursor asks one step at a time).
- No read-only query of whether the project's runner is alive (as F-806).

### Proposed for `DESIGN.md` (not edited here)

- Run bands: the paused and cancelled treatment (ink iterating segment, the runner pictograms of F-806); converged and
  failed as end caps; the part of a running band not computed yet hatched to the axis end.
- The flag's text for a steady run ("it 40"), and "t = … s · it N" for a transient one.
- The scrubber as a keyboard slider (arrow keys, Page Up and Down, Home and End).
- The monitor readout's unit line ("lid_drag, N · at it 40", or "no unit given").

### Open questions for the maintainer

- `service:panel.snapshot` keeps its exemption naming F-810: the workspace reads runner runs through the project server
  and does not use a service session's panel; which item should own it?
- The roadmap task text named P6 "live update latency"; ADR 0350's P6 is the 4e6-cell scrub. Both are recorded above.
