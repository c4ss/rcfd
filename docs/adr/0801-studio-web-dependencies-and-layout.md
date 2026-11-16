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
