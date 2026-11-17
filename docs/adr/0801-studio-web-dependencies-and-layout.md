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

## Addendum 6: F-810, run bands and linked time scrubbing, the run workspace (2026-11-16)

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
- **The run port**, `window.studioRunData` (`src/shared/run.ts`, `src/main/run/bridge.ts`, `series.ts`): `series(run)` and
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
- F810-D3, a fourth port `window.studioRunData` instead of new operations on `window.studioProject`: the project port's key
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

## Addendum 7: F-809, derived pictures in the field viewport (2026-11-16)

F-809 (iso-surfaces, streamlines, cut planes and vectors in the field viewport; guardrails G2, G3, G6, G8, G13, G22)
adds **no dependency** and changes no Rust file, service method, reply shape, golden or default. No existing test was
edited.

### What the engine computes today (read before the code)

| Picture | Engine entry point on a stored field | What exists only inside the libraries |
|---|---|---|
| Iso-surface | none | `rcfd_post::surfaces::cut_implicit` cuts by a function of position (marching tetrahedra), not by a cell field; `Surface::clip_by_values` clips a surface by its vertex values. Nothing turns a stored cell field into an iso-surface. |
| Streamlines | none | none (no integrator anywhere in the workspace; `particles.report` tracks particles of a particle run, which is another thing) |
| Cut plane | `render_field` (MCP, project) draws a 2D case only and refuses a 3D one; `render.png` (service, live session) draws the middle slice of the thinnest direction; `field.sample` (service, live session) samples a plane | rcfd-render's cut plane: `View::clip = (n, d)`, "only points with `n . p <= d` are drawn (clipped per fragment in the shader)" (`crates/rcfd-render/src/shader.wgsl`); `rcfd_post::render::render_scalar` with `axis` and `slice` |
| Vectors | `render.png` with `vectors: true` and MCP `render_image` (live session only: an image of arrows on the middle slice) | `rcfd_post::render::overlay_vectors` (in-plane arrows on a lattice of `density` x `density`, 24 in `render.png`) |

The viewport draws a run's **store** (F-804's data door). None of the entry points above takes a store and a 3D
derived picture, and the Studio cannot add one (no Rust change). So:

- **Cut plane**: computed in the Studio by rcfd-render's own rule, on the GPU, for the surface, the mesh grid and the
  vectors, and in the CPU pick.
- **Vectors**: computed in the Studio from the stored components, by the rule of `overlay_vectors` carried to the
  boundary surface.
- **Iso-surface** and **streamlines**: not drawn; each is shown as unavailable with the reason (G6), and recorded below
  as a missing engine entry point.

### Tolerances, registered before the code (G3)

- **T1, which cases are 2D.** The engine's word for a 2D case is `render_field`'s: "one cell layer", the middle plane of
  the thinnest direction cuts every cell. The Studio decides from the surface it already has: the thinnest direction of
  the surface's bounding box is `a`; the case is 2D when every cell of the store owns a surface triangle lying in the
  plane `p[a] = min[a]` and one lying in `p[a] = max[a]` (all three corners equal to the bound, in `f32`). Tolerance:
  none; the verdict must equal `render_field`'s on real cases (the quick-start cavity, 2D; the axial-fan passage, 3D)
  and be 3D for the f447 box.
- **T2, cut plane.** rcfd-render keeps a point when `n . p <= d`. The Studio's planes are normal to an axis, `n = e_a`
  (keep the side below `d`) or `n = -e_a` (keep the side above), so the test is `p[a] <= d` or `p[a] >= d`: exact in
  `f32` and `f64` alike, no tolerance in the rule itself. `d` is held as an `f32`. On screen, a fragment's `p` is the
  rasterised world position, so pixels within 2 CSS pixels of the plane's trace, or of a triangle edge, are not judged;
  every other pixel must be either the viewport ground or exactly the table colour (F-808's 3-level bound to
  rcfd-render's colour) of a cell whose picked point satisfies the rule, and a pixel whose unclipped point broke the
  rule must have changed. The pick (hover readout) takes the nearest hit that satisfies the rule.
- **T3, vectors.** Taken from `overlay_vectors` and `render.png` (read from the Rust source by the tests): a lattice of
  `24` bins per axis over the bounding box, the first candidate in order per bin; the arrow of the largest vector is
  `0.8` of a lattice step and an arrow's length is proportional to `|v| / max |v|`, the maximum over every cell of the
  store; the head is two strokes back `0.3` of the length and out `0.5` of that to each side. Carried to a surface: a
  candidate is a surface triangle (on a 2D case only the triangles of the face the fitted view looks at, and the
  in-plane components, as `overlay_vectors` draws them); the anchor is the triangle's centroid; the lattice step is the
  largest extent of the bounding box over 24; the head's side is the triangle's normal crossed with the arrow; a zero or
  non-finite vector draws nothing (`overlay_vectors`' half-pixel floor is an image rule and is not taken). Tolerance:
  every arrow carries its cell's stored components bit for bit (the Rust reader's reference bytes); the set of arrows
  (triangle, cell) equals an independent `f64` recomputation exactly; every drawn point is within `2^-20` of the
  largest extent of that recomputation (the `f32` rounding of the drawn positions); on screen the only pixels the
  arrows change are deep ink.

### What F-809 built

- **`src/renderer/field/derived.ts`** (pure): which derived picture applies to a store and why not (`availability`),
  the 2D verdict (`meshLayers`, T1), rcfd-render's cut (`clipPlane`, `keeps`, T2), the arrows (`vectorGlyphs`, T3),
  and the four pictograms on the 14-unit grid.
- **`src/renderer/field/DerivedControls.tsx`**: the controls in the viewport's tool row (pictograms; the name in the
  accessible name and tooltip; an unavailable one a dashed, focusable chip whose accessible name and tooltip give the
  reason), the cut plane's panel (axis, position, side kept) and `useArrows` (the stored components of the shown
  snapshot through the data door, with the view's tag, then the arrows).
- **`FieldCanvas.tsx`**: shared clip uniforms and a `maskNode` on the surface, mesh-grid and arrow materials (keep where
  `dot(n, positionWorld) <= d`); the arrows as deep-ink line segments; the pick takes the nearest hit the cut keeps and
  returns the hit point. **`FieldViewport.tsx`**: the controls, the panel, the arrows; the readout carries the hit point
  and the pointer position it was taken at (`data-point`, `data-pointer`) and the frame the number of arrows
  (`data-arrows`), for the tests. `field.css`: the controls and the panel, tokens only.
- **Tests** (no human step, no existing test edited): `tests/unit/field-derived.spec.ts` (9) and
  `tests/e2e/field-derived.spec.ts` (7), described in the README.

### Decisions taken in F-809 (reversible; the maintainer may change them)

- **F809-D1, pictures the engine cannot make.** (a) Show the iso-surface and streamlines as unavailable chips with the
  engine's missing entry point as the reason (done); (b) leave them out until the engine has them. Recommendation: (a):
  G6 asks for the limit to be said, and the chips cost no standing words (G22).
- **F809-D2, what "cut plane" means.** (a) rcfd-render's cut: the surface beyond the plane is not drawn, the cut is not
  filled (done; exactly the engine's rule, testable pixel by pixel); (b) a filled slice coloured by the cells it crosses,
  as `render_scalar` draws a slice: needs the volume mesh through the data door and an engine reference on a store,
  which no entry point gives for a 3D case. Recommendation: (a) now, (b) when the engine has a store-based slice.
- **F809-D3, where the arrows stand.** (a) On the boundary surface, at triangle centroids, by overlay_vectors' lattice
  (done); (b) at cell centres on the cut plane (needs (b) of D2). Recommendation: (a).
- **F809-D4, the 2D verdict.** (a) Computed from the surface the viewport already has and tested against
  `render_field`'s verdict on real runs (done); (b) ask `render_field` each time (only possible for a run of an open
  project, not for a bare store). Recommendation: (a).
- **F809-D5, the arrows on the main thread.** `vectorGlyphs` runs in the renderer's main thread, one pass over the
  surface's triangles (26 000 on the 1e5-cell box; not timed); the mesh grid uses a worker. Recommendation: move it to a worker if a 1e6
  store measures slow (G21); reversible.
- **F809-D6, the first cut.** Normal to x, through the middle of the bounding box, keeping the side below (rcfd-render's
  `n = e_x`). Any axis and side are one key away.
- **F809-D7, capability coverage.** No capability of the inventory gains a UI path: the pictures draw stored fields
  through the data port. The exemptions of `service:field.sample`, `cli:rcfd sample` and its options, which named F-809
  "not built yet", now say what they are (samples of a live session or of a case run again), keep F-809 and are marked
  unplanned. (a) Keep them so; (b) move them to an item that will show samples. Recommendation: (a) until the roadmap
  names such an item.
- **F809-D8, words on screen (G22, The Quiet Rule).** The controls are pictograms; names and reasons are in the accessible
  name and the tooltip. Visible words are only the cut panel's position ("Plane at x", its value: a value the user acts
  on) and the axis letters, and an error if the arrows' components cannot be read.

### Missing in the engine (no Rust change here)

- An iso-surface of a stored cell field (for example a point reconstruction plus `cut_implicit`, returning triangles and
  their cells) as a service `data.*` method or an MCP tool.
- Streamlines of a stored velocity field (an integrator; seeds; polylines with the cell of each point).
- A slice or cut of a stored 3D field with values (`render_field` refuses 3D; `render.png` and `field.sample` work on a
  live session only); `render_field` with `axis`, `slice` and `vectors` for a store would cover slices and arrows.
- G16: a CLI or MCP way to draw the Studio's cut and arrows of a store (rcfd-render's `View::clip` is a library
  feature only), so Claude Code can make the same picture.
- The length unit of the stored mesh coordinates (the cut panel shows the position without a unit).
- (2026-11-17) Collected with the other items' engine needs in "Engine follow-ups of the Studio items" at the end of this
  ADR, as E-809a to E-809e.

### Proposed for `DESIGN.md` (not edited here)

- Field view: the derived-picture controls as pictograms (cut plane, vectors, iso-surface, streamlines) on the 14-unit
  grid, the name in the tooltip; an unavailable one as the dashed chip, its reason in the tooltip.
- Vectors in deep ink, opaque, on the surface (with the iso-lines in ink and streamlines in white of today's list).
- The cut plane's panel in the frame's top left corner, white inside an ink rule (Reset view keeps the top right).
- The cut plane hides the surface beyond the plane and does not fill the cut (rcfd-render's cut).

### Verification (2026-11-16, Windows 11)

- `npm ci`: found 0 vulnerabilities. `npm run typecheck`: no errors.
- `npm test`: 497 passed (13.7 min), the 16 of F-809 among them; nothing skipped.
- `python scripts/third_party_licenses.py --check`: THIRD_PARTY_LICENSES.md is up to date.
- `cargo test -p rcfd-cli --release --test docs_site --test workspace_graph`: 2 passed, 4 passed.

## Addendum 8: F-828, provenance and reproduce from the CLI (2026-11-16)

No new dependency. The popover is React Aria's `Popover` and `Dialog` (already a dependency), drawn in a React root of
its own (`react-dom/client`, already a dependency) so that its keys do not reach the handlers of the row the number sits
in. The clipboard is Electron's `clipboard` in the main process.

### Layout

- `src/shared/provenance.ts`: the provenance port's contract (`studio.provenance-port`, ops `run` and `copy`), the run
  record's facts, a number's source (`NumberSource`), the reproduce command built from a record alone with its gaps
  in words, and the facts shown.
- `src/main/provenance/`: `record.ts` reads `runs/<id>/run.json` whole, the hashes of `runs/<id>/case.snapshot` and of
  the case file now, and the units of `output/monitors.json`, all through the project server's read-only
  `read_project_file`; `bridge.ts` checks each request (a run id, or a text of at most 16000 characters to copy).
- `src/renderer/provenance/`: `Provenance.tsx` (the trigger, a button with the number's words as its accessible name,
  or a passive span inside a grid row or a list option; the popover), `registry.ts` (which number Show provenance
  opens; the palette commands `provenance.show` and `provenance.copy_command`), `sources.ts` (each surface's numbers),
  `door.ts` (the one renderer module naming `window.studioProvenance`), `provenance.css`.
- Wrapped: the time panel's residual values, monitor readout and snapshot counter (the scrubber names the counter);
  the workspace's numbers and run ids; the Runs view's table result, the selected run's id and numbers and its report
  values; the field view's readout (the field view names it). Ctrl+I is Show provenance (global, the shortcut table).

### Decisions (options, the recommendation taken; each reversible without an engine change)

- D1 the reproduce command: `rcfd-runner start <case> --project <folder> --threads --priority [--mem-mb] [--set]...`
  from `run.json` (taken; the CLI twin of `start_run`, making the same run folder), against `rcfd run
  runs/<id>/case.snapshot` with the runner's launch flags (not recorded, so the Studio would write them from the
  runner's source). A changed case, a mismatched snapshot, settings with no `rcfd-runner start` option
  (`continue_from`, `max_steps`), a mesh file, a case outside the project and an unknown or different engine version
  are said in words beside the command.
- D2 a fifth MessagePort rather than new operations on the run port (F-810's test fixes the run port's operations).
- D3 Copy writes the clipboard in the main process (every permission of the page stays denied).
- D4 Ctrl+I for Show provenance.
- D5 the engine versions compared as `<name> <version>`, the form of `rcfd --version` and `solver_version`.

### G17 measured

`tests/e2e/provenance.spec.ts`: a run of `cases/cavity-re100.toml` started from the window, its command copied from its
provenance and run from PowerShell with `--project` changed to an empty project: `fields/store.rts`,
`fields/index.json`, `residuals.jsonl`, `output/monitors.json`, `output/monitors.csv`, `output/report.html`,
`output/restart.rcfd`, `output/solution.vtu`, `case.snapshot` and `check.json` byte for byte equal; `metrics.json`,
`status.json` and `solver-status.jsonl` equal without times, process ids and own paths; `run.json`'s
`case_snapshot_hash`, `solver_version` and `settings` equal; the sequence of `events.jsonl` kinds equal (the README,
F-828, registers the comparison).

### Missing in the engine (no Rust change here)

- `run.json` records neither the solver's launch argv nor a build id (`solver_version` is a release name).
- `rcfd-runner start` has no options for `continue_from` and `max_steps`; no `rcfd-runner rerun <id>` or
  `start_run {from_run}` reruns a run from its own `case.snapshot`.
- `status.json`'s `time` has no unit (the Runs view's "Time, s" caption is the Studio's word).
- No read-only hash of a mesh file a run names (the reproduce check cannot say whether it changed).
- A service session has no reproduce surface yet (`journal.get` then `rcfd run --journal`).

Follow-up (2026-11-17, Addendum 17): the service-session numbers (settings, the dev plots), the field caption's step and
time and the first path's result numbers now open their provenance; a session number says its file and its reproduce
command as not recorded, and a snapshot's time its unit. The engine needs above are E-828a to E-828e of "Engine
follow-ups of the Studio items" at the end of this ADR.

### Proposed for `DESIGN.md` (not edited here)

- The provenance affordance: a number at rest unchanged; hover, focus and open add a 1px dotted ink underline at 3px
  offset; focus keeps the orange outline.
- The provenance popover: the popover lift, 480 px wide; a head (quantity, value, unit in ink-2), a two-column facts
  list (labels ink-2), the command on the viewport fill, its gaps in medium weight, then Copy command and Copy.

## Addendum 9: F-832, responsiveness budget and cancellable work (2026-11-16)

Item F-832 of `docs/ROADMAP.md`. Traces G21, G9 (P5), G22, G18. No new dependency.

### The registered limits (written before the measurement code)

| Name | Value | What it bounds |
|---|---|---|
| `LONG_TASK_LIMIT_MS` | 100 ms | any one `longtask` entry on the renderer's main thread while a run is opened, fields are loaded (both bulk routes) and the scrubber moves |
| `LONG_FRAME_LIMIT_MS` | 150 ms | any one `long-animation-frame` entry in the same window |
| `PROGRESS_AFTER_MS` | 300 ms | an operation still going after this shows progress and Cancel; a shorter one shows nothing |

Chosen as follows. 100 ms per task: while a task runs nothing on the page answers, Cancel included; 0.1 s is the
classic limit for a reaction to feel immediate (Card, Moran and Newell), two thirds of P5's 150 ms frame limit, and
twice Chromium's 50 ms long-task threshold, so ordinary renders of the workspace are not counted. 150 ms per frame: P5's
own "no frame > 150 ms", applied to every frame of the measurement. 300 ms before progress: below it progress would
flash (G22), above it a long operation speaks well before the 1 s at which flow is lost (Nielsen); it equals F-810's
`QUIET_MS` and lies above every scrub frame of addendum 6 (1e6 p99 75 ms, 4e6 p99 159 ms), so scrubbing at those sizes
stays silent while P7's first picture (up to 1.5 s) and opening a project speak.

### What was built

The parts are listed in `apps/studio-web/README.md`, under "Responsiveness and cancellable work":

- the work tracker and its progress row (`src/renderer/work/`);
- the cancel hook for doors (`CancelOptions`, `src/shared/work.ts`), and the `cancel` requests of the project and case
  ports;
- the field worker (`src/renderer/field/prepare.worker.ts`), which also makes F-809's mesh layers and vector arrows
  (`derived.ts` `meshLayers`, `vectorGlyphs`, unchanged) since the merge with F-809;
- the tests' delay (`RCFD_STUDIO_TEST_DELAY`, `src/main/work/delay.ts`);
- the long-task test (`tests/e2e/responsiveness.spec.ts`) and the cancel tests (`tests/e2e/cancel.spec.ts`,
  `tests/unit/work.spec.ts`).

The bridges' key sets are unchanged. `window.studioProject`, `studioCase`, `studioData` and the run port's `RUN_OPS`
are pinned by the existing tests.

### Measurements (AMD Ryzen 7 9800X3D, 16 logical processors, 93 GB; every processor busy with other agents' builds)

All runs happened with every processor busy (`cpuBusy` 0.83 to 1.00 before and after, 26 to 50 build, node and electron
processes). Each figure is the largest entry of its kind, by phase. The limits are 100 ms per task and 150 ms per held
frame.

| Run | Store | Result | Largest task | Largest long frame |
|---|---|---|---|---|
| Before (first harness, no worker or tracker) | 1e5 | passed | 51 ms (run open) | 136 ms (project open), 112 ms (scrub) |
| Before | 1e6 | failed | 69 ms | 298 ms (blocking 0), 183 ms (blocking 11), 152 ms (blocking 19), all while the workspace first opened |
| After, before frames were split by `blockingDuration` | 1e5 | 3 passes failed | 95 ms | 246 / 178 / 163 ms in the Workspace tab's first frame, blocking 0, no script over 5 ms; one 13.5 s frame, blocking 0 |
| After (final) | 1e5 | passed, pass 1 | none over 50 ms | 144 ms (workspace tab), 70 ms (scrub) |
| After (final) | 1e6 | passed, pass 1 | 61 ms (workspace tab) | 136 ms (workspace tab), 67 ms (run open) |

The probe (a deliberate 130 ms task) was recorded every time.

Two changes in the harness cleared the run-open breaches (184 and 258 ms, with blocking 85 and 52 ms) in the first
run after the worker. First, the test now selects a small run before it measures the big one, so the workspace's own
first mount is not counted as "opening a run". Second, the first mount of the Workspace tab gets a phase of its own.
That first mount compiles about 1.6 MB of script (the field viewport and three's WebGPU renderer) and creates the GPU
device, and it showed up as unheld frames, not tasks. Fetching that code on idle once a project was open did not change
those frames, so that experiment was removed.

### Decisions taken on the recommendation (reversible, within the guardrails; the maintainer may overturn any of them)

1. **How a cancel crosses `contextBridge`.** Options: (a) new bridge keys (`cancel` on each bridge); (b) an
   `options.onCancel` callback argument, which the preload script calls with the function that cancels that request;
   (c) a shared request-id scheme exposed to the page. Recommended and taken: **(b)**. The key sets are pinned by
   tests that may not change, functions cross as proxies, and the page never sees a request id.
2. **Cancelling a save after `edit_case` was sent.** Options: (a) refuse the cancel and say so; (b) let the save finish
   and report it as usual, without the progress; (c) undo with a second edit. Recommended and taken: **(b)**. The
   engine writes a file whole or not at all, so there is no partial state. Option (c) would be a second write the user
   did not ask for (G18).
3. **Cancelling a project opening once the project server has started.** Options: (a) let it finish, then close the
   late session; (b) kill the process. Taken: **(a)**. The engine has no abortable start, and killing a child process
   from the renderer's request would be a new kind of power for the page.
4. **Run histories (`run.series`) and mesh checks.** These are dropped on cancel and not stopped. The run port's ops
   are pinned (`RUN_OPS`), and the service has no cancel. They are recorded below as missing entry points.
5. **The tests' delay** (`RCFD_STUDIO_TEST_DELAY`). Options: (a) a delay hook in the main process, read only from the
   environment; (b) large fixtures that are slow by nature; (c) fake engines. Taken: **(a)**. It works on any
   machine, cancels at once, and is inert when unset. The real engine is used in every test.
6. **F-810's `QUIET_MS` is replaced by the tracker's `PROGRESS_AFTER_MS`.** It is the same 300 ms, but now there is one
   mechanism for every surface. The field viewport's loading words ("Reading the stored fields of ...") are replaced
   by the progress row's accessible name (G22: the normal state says nothing).
7. **Which long frames count.** Options: (a) every `long-animation-frame` entry over 150 ms; (b) only those the
   renderer's thread held up (`blockingDuration` > 0); (c) no frame limit, tasks only. Recommended and taken: **(b)**.
   Under full load, frames of 157 to 246 ms, and one of 13.5 s, had `blockingDuration` 0 and no script over 5 ms: the
   thread was free for input all along, and rendering waited on the GPU process or the scheduler. Such frames are
   reported (`notHeld`), not failed. The test also makes up to three passes in fresh windows and passes on the first
   clean one. A task that the Studio's own code makes recurs in every pass.

### Gaps, and missing engine entry points (no Rust was changed)

- `rcfd-mcp` handles one request at a time and ignores notifications. It has no `notifications/cancelled` (MCP's cancel),
  so `check_case_headless` (in-process `run.check`) and `read_residuals` cannot be stopped once sent, only dropped. A
  cancellable tool call in `crates/rcfd-mcp/src/lib.rs` would let the Studio stop them.
- The service has no cancel for `mesh.check` or `run.check`; a `cancel` by request id (or a `$/cancelRequest`) is
  missing.
- A project server's start cannot be aborted half way (`rcfd-mcp --project` start-up).
- Neither the Studio nor the engine's paths to it have a field export, so there is no export to make cancellable. A
  future one must use the tracker.
- Not yet made works: creating or duplicating a case (`new_case`), run-control actions (F-807 shows its own pending
  state), and the Claude panel (F-813 has its own stop). These are short, or already cancellable in their own
  terms, but they do not use the one mechanism yet.
- The hover pick (three's `Raycaster` over the surface) runs on the main thread, O(triangles), on pointer movement.
  It is not engine or data work, and the long-task test does not hover, but at large surfaces it belongs in the worker.
  Proposed for F-819.

### Proposed for DESIGN.md (not edited here)

- **Progress row.** One 26 px row (`--size-control`) per long operation, in the surface it concerns: over the top edge
  of the field frame (overlay), in the flow of the case panel and the empty project home (inline), and in the header
  actions of an open project (band). The row holds an indeterminate bar drawn as the "not yet" hatch in a 1 px ink rule,
  moving one stripe period per `--motion-band-grow` (still under reduced motion), a short word in label type, and a
  Cancel button that holds the row's right edge. There are no percentages, because the engine gives none.
- **Shown only after 300 ms** (`PROGRESS_AFTER_MS`). A shorter operation shows nothing. The words are in the accessible
  names: the bar is "Opening run r-...", and the button is "Cancel opening run r-...".
- **Cancel leaves the screen as it was before the operation**: the previous picture, run or result. It does not show a
  "cancelled" notice (G22).

## Addendum 10: F-815, comparing two runs in the run workspace (2026-11-16)

F-815 (two runs side by side at the same time and the same colour range, the difference field where the meshes match
and a stated refusal where they do not; guardrails G2, G3, G6, G8, G16, G20, G21, G22) adds **no dependency** and
changes no Rust file, service method, reply shape, golden or default. No existing test was edited.

### What the engine computes today (read before the code)

| What | Engine entry point | What exists only inside the libraries |
|---|---|---|
| Metrics of two runs and their differences | MCP `compare_runs` (project): the metrics of `metrics.json` of 2 to 10 runs, `differences` = each run's value minus the first run's, each run's state, step, time and residual line count | none needed |
| Whether two meshes are the same | none as a query; every store records the mesh's content hash in its MESH frame (`rcfd_tsstore`, `Writer` from `Mesh::content_hash`), and `data.open` / the Studio's reader report it as `mesh_hash` | `rcfd_post::remap::Remapper::new`: the transfer is an identity copy when `src.n_cells() == tgt.n_cells() && src.content_hash() == tgt.content_hash()` (`crates/rcfd-post/src/remap.rs`) |
| A difference field of two stored runs | none (no MCP tool, CLI command or service method takes two stores) | `rcfd_post::compare::difference(ref_mesh, ref_values, mesh, values, method)`: `values - remap(ref_values)` in `f64`; on identical meshes the remap is a copy, so it is `values[c] - ref_values[c]`; it refuses a value that is not finite (`difference: values[i] is not finite`) and a length that is not the cell count |

So the metrics come from `compare_runs` through the run port (G16: the same call Claude Code makes), and the
difference field is computed in the Studio by the library's own rule on identical meshes, with the rules below
registered before the code (G3). The missing entry point is recorded at the end.

### Rules and tolerances, registered before the code (G3)

- **C1, the same time.** A run's time is its iteration when it is steady (the runner's `steady`; when not known, a
  store whose every snapshot time is 0, which is what a steady solver writes) and its simulated time otherwise. The
  compared run's snapshot at a stop of the selected run is the one with the same iteration (steady) or exactly the same
  stored `f64` time (transient; when several snapshots carry that time, the last of them). Nothing is interpolated and
  nothing is taken from the nearest snapshot: when the compared run has no snapshot at that time, its view and the
  difference say so in words ("No snapshot at it 45"), and a steady run is never compared with a transient one (no
  common time; stated). The scrubber stops at the selected run's snapshots, as in F-810, and drives both views.
- **C2, the same mesh.** The meshes match when the two stores' cell counts are equal and their recorded content hashes
  are equal: the identity rule of `Remapper::new`. (The point counts, which the hash covers, are checked as well.)
  Otherwise there is no difference field; the refusal says which differs (the cell counts, or with equal counts the
  mesh hashes).
- **C3, the difference.** `d[c] = vB[c] - vA[c]`, A the selected run (the baseline, as `compare_runs` takes the first
  run and `difference` the reference), B the compared run, in IEEE-754 `f64` with round to nearest, where `v` are the
  stored values as `Reader::read_field` returns them (an `f64` store bit for bit, an `f32` store widened exactly).
  Tolerance: none; every value equals an independent `f64` computation from the stored bytes bit for bit. It is drawn
  as `f32` (`Math.fround`, as `as f32`) through F-808's table and range rule, and the readout shows that `f32`. Refused,
  with the reason, as `difference` refuses or because the Studio converts nothing (G2): the field missing from either
  snapshot, a point field, a value count that is not the cell count, different units, a value that is not finite.
  Computed in the main process's reader worker (G21), local route only.
- **C4, one colour range.** Side by side, both views and the one legend use `[min(loA, loB), max(hiA, hiB)]`, where
  `lo` and `hi` are each view's F-808 range (the min and max of its shown values, over the cells that own surface
  triangles): exactly the union, no padding and no rounding. Each view reports the span of the values it shows in the
  same render that shows them, so the range drawn always holds every value drawn and the legend is always the range
  drawn. The difference view has its own F-808 range.

### What F-815 built

- `src/shared/compare.ts`: C1 to C4 as pure functions (`timeKind`, `counterpart`, `counterpartWords`, `meshMatch`,
  `fieldsMatch`, `difference`, `unionSpan`), shared by the reader worker and the workspace.
- The data port gains one request, `difference {store, snapshot, other, otherSnapshot, field}`, answered by the main
  process's reader worker with the `f64` values (an `ArrayBuffer`, as `field`). The renderer reaches it through a
  new frozen bridge, `window.studioCompare {difference}`, over the same data port, so that `window.studioData` keeps
  exactly its four functions (an existing test fixes them). Local route only: a service or mixed
  route is refused (`bad_request`), since the service has no field difference. The reader gains `field64`, the stored
  values as `f64`, decoded as `field` is.
- The run port gains one request, `compare {run, other}` (`RUN_COMPARE_OPS`, kept apart from `RUN_OPS`, whose list an
  existing test fixes), answered by `compare_runs {runs: [run, other]}` through the read-only project server;
  `compare_runs` joins `PROJECT_TOOLS`.
- The run workspace: a pictogram toggle Compare runs, a Select of the compared run and a segmented pictogram toggle
  Side by side / Difference in the field bar (`workspace/Compare.tsx`, `compare-model.ts`, `compare.css`); the two
  views (each a `FieldViewport` with its own camera, legend off, the one range passed in) and one legend; the
  difference view (`Δp`, "B − A", vectors and cuts off); refusals in a view's place (role `status`, pictogram and words);
  the metrics table under the run's numbers. `FieldViewport` gains additive, optional props (`range`, `onShown`,
  `legend`, `load`, `loadKey`, `quantity`, `title`, `derived`); without them it is unchanged.
- The time panel overlays the compared run's residual and monitor lines (secondary ink, 1 px, the same dash per
  equation as the selected run's) and lists its residuals at the line; its band keeps its fill and gets a dashed ink
  underline. The compared run's lines are at their own steps on the one iteration axis.
- The palette: Compare runs (`workspace.compare`, group Project runs, MCP `compare_runs`), asking which run; the
  selected run and a project with fewer than two runs are unavailable with their reason.
- Coverage: `mcp:compare_runs` moves from its F-815 exemption to the run workspace.

### How G22 applies

At rest the comparison adds one pictogram button to the field bar and no words. When it is on, the only visible words
are data (the compared run's case and id in its Select, the captions' run ids and quantity) and the exceptions: a
refusal's words, which are always shown. The toggle and the two modes are pictograms with accessible names and
tooltips (Compare runs, Side by side, Difference) on the 14-unit grid at 0, 45 and 90 degrees.

### Decisions taken here (reversible, within the guardrails; the maintainer may overturn them)

1. **Where the scrubber stops.** Options: (a) at the selected run's snapshots, the compared view saying where it has
   none; (b) only at the times both runs stored; (c) at the union. Taken: (a), which keeps F-810's scrubber unchanged
   and makes the gaps visible instead of hiding them. Recommendation: (a).
2. **Where the difference is computed.** Options: (a) in the Studio's reader worker by `rcfd_post::compare`'s rule on
   identical meshes; (b) wait for an engine entry point. Taken: (a), registered as C3 and tested bit for bit against
   an independent decode. Recommendation: add the engine entry point (below) and switch the Studio to it, keeping the
   test as its check.
3. **Side by side or overlaid.** The acceptance allows either. Taken: fields side by side (two pictures cannot overlay
   on one surface without hiding one), residuals and monitors overlaid on the shared axis. Recommendation: as taken.
4. **Cameras.** Options: (a) each view its own camera; (b) one camera driving both. Taken: (a), since `FieldViewport`
   holds its camera inside and linking them needs a change to F-808's canvas. Recommendation: (b) as a follow-up
   item; it helps reading two pictures at once.
5. **The difference's colour map.** Options: (a) viridis with the difference's own range; (b) a diverging map centred
   on zero. Taken: (a), the only map `DESIGN.md` defines. Recommendation: (b), once `DESIGN.md` adds a diverging map;
   a maintainer decision (design).
6. **The compared run's residuals.** Its lines are drawn at their own steps; the list gives its value at the selected
   run's step (exact, from `read_residuals`). No resampling.

### Missing engine entry points (no Rust changed)

- A field difference of two stores: `rcfd_post::compare::difference` exists only in the library. Proposed: an MCP tool
  (`compare_fields {run_a, run_b, field, step}` returning summary statistics and writing nothing large into the
  reply), a CLI command (`rcfd post difference`) and a service method taking two stores, so that Claude Code and the
  Studio's service route reach the same computation (G16).
- A mesh identity query (whether two runs' meshes are the same, and why not): today read from the stores' MESH frames.
- `compare_runs` compares metrics only; no field or residual comparison. `read_residuals` still gives no simulated
  time per sample (F-810), so transient residuals are compared on the iteration axis.

### Proposed `DESIGN.md` additions (not edited here)

- Compare pictograms: two framed panels (side by side), a triangle (difference); toggle as a pictogram button.
- The compared run's band: full fill like the selected one, with a 1 px dashed ink underline; its plot lines in the
  secondary ink at 1 px with the selected run's dashes.
- The refusal panel: a view's place filled with the viewport colour, a pictogram and the reason in words.
- One legend under two views: one range for both, the caption saying "one range for both runs".
- A small hairline table of metrics (A, B, Δ) in tabular figures.
- A diverging colour map for differences, centred on zero (decision 5).

### Verification (2026-11-16, after merging studio-roadmap d13151f5)

- `npm ci`: 0 vulnerabilities. `npm run typecheck`: clean.
- `npm test`: 556 passed, 2 failed, 4 did not run. Rerun alone: `workspace.spec.ts` 5 passed (the Follow live race
  with a running run); `provenance.spec.ts:220` (focus back on the trigger after Escape) fails the same way on the
  studio-roadmap tip without F-815, so it is not F-815's. `tests/unit/compare.spec.ts` 16 passed,
  `tests/e2e/compare.spec.ts` 7 passed (screenshots in `test-results-visual/f815/`).
- `python scripts/third_party_licenses.py --check`: up to date.
- `cargo test -p rcfd-cli --release --test docs_site --test workspace_graph`: 2 and 4 passed.

## Addendum 11: F-833, the Quiet Rule swept over the built surfaces, and a word budget per surface (2026-11-16)

F-833 (G22, with G6, G8, G13, G18, G19, G20) quiets every built surface of the Studio and locks a budget of visible
chrome words per surface. It adds **no dependency** and changes no Rust file, service method, reply shape, golden or
default. `DESIGN.md` and `docs/studio-guardrails.md` are not edited; proposed `DESIGN.md` changes are below.

### What "a chrome word at rest" is (written before the first count)

Defined in `apps/studio-web/src/shared/word-budget.ts`: a whitespace token with a letter or digit, in a text node or an
empty field's placeholder inside the surface's root, laid out (`checkVisibility`) with a box of at least 2 by 2 pixels
and not clipped away by an ancestor (so `.sr-only` text is not counted); words outside the window count. Not counted:
text whose nearest `data-words` is `data`, `exception`, `disclaimer` or `help`; accessible names, descriptions and
tooltips that are not open. `tests/e2e/word-budget.spec.ts` brings 17 surfaces to rest in one Studio, photographs and
counts them, and fails past a budget.

### Counts (1440 by 900; BEFORE and AFTER photographs in `apps/studio-web/test-results-visual/f833/`)

| Surface | Before | After = proposed budget |
|---|---:|---:|
| project home, no project | 75 | 6 |
| first path, no project | 22 | 22 |
| project home, project open | 167 | 49 |
| first path, empty project | 22 | 22 |
| runs | 64 | 14 |
| runs, a run selected | 64 | 16 |
| run workspace | 111 | 17 |
| compare | 65 | 8 |
| derived-picture controls | 0 | 0 |
| provenance popover | 11 | 11 |
| Claude Code panel | 63 | 13 |
| command palette | 31 | 4 |
| case settings | 144 | 23 |
| case editor | 553 | 51 |
| undo controls | 6 | 0 |
| decision panel | 0 | 0 |
| progress rows | 3 | 3 |

The budgets are proposed and await the maintainer's approval (`BUDGET_STATUS`).

### Where the words went

- Accessible names and tooltips: a normal run state (`StateMark quiet`: the pictogram, the word as name and title), the
  project's Live mark, a band's end ("converged · it 121" as name, "it 121" shown), Saved in the case editor, the undo
  and redo arrows with nothing to take back, icon buttons (Close project, Clear selection, Close session, a case row's
  Run, Play and Pause) whose names did not change.
- Accessible descriptions (`aria-describedby`): what a project is (Open project folder…), when the folder was read (Read
  again), the composer's keys, Save's shortcut, Check the case's what-it-does.
- Readers only (`Quiet`, `.sr-only`): section headings whose place says them, source lines (where the numbers come from:
  also one step away in the provenance, F-828), the normal facts of the folder, of a case file and of a session, the
  palette's key hints, the footer's right-hand line, "listed hh:mm" and "read hh:mm".
- Focus detail: a settings key's description, hint, default, range and value count (`.st-key-detail`, shown while the
  row holds the focus).
- Empty states: `EmptyMark`, DESIGN.md's "not yet" hatch with the sentence as the name of a `note` and its tooltip.

### What stayed visible, and why

Data; the controls' own labels; column headers and field labels; the G6 disclaimer; the exceptions: a failed or
cancelled run's word and note, the WebGL 2 fallback, a decimation of the lines, monitor units the engine does not give,
report values with no unit ("No unit from the engine."), a service that is not connected, changes not saved, why a save
cannot be made, refusals, conflicts and the decision panel; a work in progress (its name and Cancel). The first path
(F-813) was already quiet and is unchanged.

### Decisions taken here (reversible, within the guardrails; the maintainer may overturn them)

1. **How an empty state looks at rest.** Options: (a) nothing at all; (b) a short visible word ("Empty"); (c) the "not
   yet" hatch with the sentence as its accessible name and tooltip. Taken: (c): the place stays marked and the sentence
   stays one step away. Recommendation: (c).
2. **A normal state word.** Options: (a) keep the word; (b) the pictogram alone with the word as name and tooltip.
   Taken: (b), as DESIGN.md already says for normal states. Failed and cancelled keep the word. Recommendation: (b).
3. **A key's description in settings.** Options: (a) visible; (b) tooltip only; (c) focus detail and the field's
   description. Taken: (c) (also the name's title). A set key keeps its Reset button visible. Recommendation: (c).
4. **Shortcut hints beside a control** (Save's Ctrl+S, the composer's Enter). Options: (a) visible; (b) the control's
   description plus `aria-keyshortcuts` and the shortcut list (Ctrl+/). Taken: (b). Recommendation: (b); G20 still
   holds through the shortcut list and the palette.
5. **Where the counter draws the line between data and chrome.** Marked in the markup (`data-words`), not guessed from
   classes. Recommendation: as taken; a new surface marks its data as it is built.
6. **`.sr-only` with `!important`.** Needed so that a quieted element's own padding or display does not bring the
   words back. Recommendation: as taken.

### Proposed `DESIGN.md` changes (not edited here)

- Under The Quiet Rule: "An empty state at rest is the 'not yet' hatch (48 by 8 px), its sentence the accessible name
  and tooltip"; "a normal absence of a value is an en dash named by its words".
- Under Components, buttons: "an action whose icon says it (close, play, pause, a row's run, undo and redo with nothing
  to take back) is an icon button; its name is its tooltip".
- Under Components, settings rows: "a key's description, default and range are focus detail".
- A line on the word budget: "each surface's budget of visible chrome words is in `src/shared/word-budget.ts`; raising
  one needs the maintainer's word (G22)".

### Verification (2026-11-16, on studio-roadmap bfc2f381)

- `npm ci`: exit 0. `npm run typecheck`: clean.
- `npm test`: 586 passed, 1 failed, 3 did not run. The failure is `provenance.spec.ts:220` (focus back on the trigger
  after Escape). Run alone, `provenance.spec.ts` fails at line 156 or 232 (the same focus-after-Escape check) on F-833
  and equally on the unchanged studio-roadmap sources (bfc2f381 built and packaged: 3 runs, failing at 232, 156, 232),
  so it is not F-833's. `tests/e2e/word-budget.spec.ts`: 10 passed.
- `python scripts/third_party_licenses.py --check`: up to date.
- `cargo test -p rcfd-cli --release --test docs_site --test workspace_graph`: 2 and 4 passed.

### The maintainer's answers (2026-11-17)

The maintainer answered the three open points of this addendum on 2026-11-17 and approved all three:

1. **The 17 budgets** of the table above, as proposed (the AFTER counts). `BUDGET_STATUS` in
   `src/shared/word-budget.ts` now reads approved and enforced; `tests/e2e/word-budget.spec.ts` already fails past a
   budget, so nothing else changes in the test. Raising a budget still needs the maintainer's word (G22).
2. **The kept-visible list** ("What stayed visible, and why"), as written.
3. **Moving the G20 shortcut hints out of visible text into the controls' descriptions** (decision 4, option (b)).
   Save's Ctrl+S, the composer's keys, undo and redo, and the palette's key hints were already moved by the sweep. One
   hint was left in a control's accessible name rather than its description: the project home's Commands button was
   named "Commands Ctrl+K" by a readers-only span inside it. It is now named "Commands", with `aria-keyshortcuts`
   `Control+K` and the description "Ctrl+K" (`aria-describedby`, `#ph-commands-keys`). No test read the old name. The
   palette and the shortcut list (Ctrl+/) still list every shortcut, so G20 holds.

The proposed `DESIGN.md` lines above are not applied: `DESIGN.md` is not edited by this item, and they stay for the
maintainer. F-833 is done.

### Verification (2026-11-17, on studio-f833b)

- `npm ci`: exit 0. `npm run build` (typecheck included) and `npm run package:dir`: clean.
- `tests/e2e/word-budget.spec.ts` with `commands.spec.ts`: 20 passed; `project.spec.ts` with `a11y.spec.ts`: 21 passed.
- `npm test`: 649 passed, 2 failed, 3 did not run. The failures were `plots.spec.ts:134` (30 s timeout under load) and
  `service.spec.ts:132` (the restarted service, a known load flake); run alone, `plots.spec.ts` passed 4 of 4 and
  `service.spec.ts` 9 of 9.
- `python scripts/roadmap_progress.py`: done 300, partial 267, todo 82, total 649.

## Addendum 12: F-816, the keyboard and assistive-technology pass (2026-11-16)

F-816 (G8, with G13, G20 and G22) checks every built surface of the Studio from the keyboard and the accessibility tree
and fixes what failed. It adds **no dependency** and changes no Rust file, service method, reply shape, golden or
default. No visible word was added or removed (G22): the word budgets of `src/shared/word-budget.ts` are unchanged and
`tests/e2e/word-budget.spec.ts` passes. `DESIGN.md` and `docs/studio-guardrails.md` are not edited; proposed `DESIGN.md`
lines are below. There is no contrast requirement (the maintainer, 2026-11-16), and no check of colour values was
written.

### What F-816 built

- `tests/support/a11y.ts`: the checks, on the DOM, the keyboard and Chromium's accessibility tree (DevTools protocol:
  `Accessibility.getFullAXTree`, `getPartialAXTree`): the Tab walk both ways with each stop's focus indicator (an
  outline, box shadow or border it has only while focused, on a box on the screen), reach (every interactive element a
  Tab stop or a member of a composite that is one), the arrow walk of each composite, click or pointer handlers no key
  reaches (read from React's props on the element), names of every interactive node, roles of every Tab stop, the CSS
  motion of the document, and animation frames asked for.
- `tests/e2e/a11y.spec.ts`: the packaged Studio against the real engine, every surface reached with keys only (16
  surface states: project home with and without a project, the first path, the Claude Code panel, Runs, a selected run,
  the provenance popover, the run workspace, its derived-picture controls, a select's list, compare mode, the command
  palette, the case editor, its undo controls, case settings and the decision panel), then reduced motion and the
  colour-alone list. It writes `test-results-visual/f816/report.json` and focus screenshots.
- The written manual pass: `apps/studio-web/README.md`, "Keyboard and assistive technology (F-816, G8)".

### Failures found, and what changed

| Found | Fix |
|---|---|
| The run bands' names in the time panel were a `div` with a click handler and no keyboard path (the pointer-only check) | `TimePanel.tsx`: buttons in a vertical toolbar with one Tab stop (the selected run's name), Up, Down, Home and End between them, Enter or Space selects; named "<case>, run <id>", `aria-current` on the selected one. Looks as before; the ring is drawn inside. |
| The decision panel (F-829) is an `alertdialog`, but Tab left it while it stayed open, and its Escape works only while it has the focus | `ui/containFocus.ts`: Tab and Shift+Tab go round the panel's own controls; `aria-modal="true"`. The same for a run's confirmation strip (F-807, `RunControls.tsx`), which the walk cannot reach without a running run (manual pass, step 3). |
| Play stepped as fast as the fields were drawn (or every 150 ms) under reduced motion | `ui/motion.ts` (`usePrefersReducedMotion`), `RunWorkspace.tsx`: under `prefers-reduced-motion: reduce`, at most one stop every 500 ms. Unchanged without the preference. Measured gaps 499 to 502 ms. |
| The live band segment differed from another running run's segment by orange alone | `workspace.css`: a 2px ink edge at the live segment's growing end; `TimePanel.tsx`: "live" as the band end's `aria-description` and in its tooltip. The end's accessible name is unchanged (`workspace.spec.ts` reads it). |
| A Claude Code session that cannot be chosen while a reply runs differed by its ink-2 name alone | `claude.css`: a dashed hairline border, as every unavailable control has. |

The shortcut table's descriptions of the time panel's Up, Down, Home and End name the band names' use of them
(`shortcuts.ts`, descriptions only; the keys were already reserved for the time panel).

Passed with no change: no trap anywhere; Shift+Tab is the reverse of Tab on every surface; every stop shows its ring
(outline on the element for all but the table rows' cells, the scrubber's line and the inputs inside a switch or number
field); no unnamed interactive node and no Tab stop without a role; Escape closes the palette, the provenance popover, a
select's list and the decision panel and gives the focus back; CSS motion is off under reduced motion (`base.css`); the
camera moves at once on a key (no easing, damping off).

### Decisions (options, the recommendation taken; each reversible)

- D1, a checker library. (a) Write the checks on the DOM, the keyboard and Chromium's accessibility tree, no dependency;
  (b) add axe-core (MPL-2.0) under G5, which needs the maintainer; (c) Playwright's ARIA snapshot only (no roles of
  non-focusable nodes, no names from the tree Chromium computes). Taken: (a). Recommendation: (a); (b) can be added later
  on top, by the maintainer's word.
- D2, the band names. (a) A toolbar with one Tab stop and the arrows; (b) one Tab stop per band (a project with 50 runs
  adds 50 stops); (c) drop the click and leave the runs list as the only way; (d) leave a pointer-only target. Taken:
  (a). Recommendation: (a).
- D3, Play under reduced motion. (a) At most one stop per 500 ms (below three changes a second, so the field does not
  flash); (b) unchanged, as a playback the user asked for; (c) Play off. Taken: (a). Recommendation: (a).
- D4, the alert dialogs. (a) Contain Tab, `aria-modal`; (b) keep them non-modal and give them another role. Taken: (a),
  as WAI-ARIA's alertdialog is modal and only the panel answers Escape. Recommendation: (a).
- D5, the live segment. (a) An ink edge at its growing end, "live" in the end's description and tooltip; (b) a new
  "live" pictogram at the band end (a new shape for DESIGN.md); (c) the accessible description only (nothing for a
  sighted colour-blind user). Taken: (a). Recommendation: (a).
- D6, selection by an ink fill (a selected segment of a segmented toggle, the first path's station). Counted as a
  carrier that is not a hue: a filled box against an empty one, with `aria-checked` or `aria-current`. Option: add bold
  weight as well (the label's width changes). Taken: no change. Recommendation: no change.

### Proposed `DESIGN.md` lines (not edited here)

- Run bands: "The live segment ends in a 2px ink edge at its growing end, so live is not told by orange alone."
- Run bands: "A band's name is a button: one Tab stop for the names, Up and Down between them, Enter selects."
- Motion: "Play under reduced motion: one stop at most every 500 ms."
- Do's and Don'ts: "Do give every state colour a second carrier (pictogram, shape, position or word); an unavailable
  control is a dashed hairline."
- Components, decision panel: "Tab stays inside while it asks."

### Open for the maintainer

- D1: whether to add axe-core under G5 on top of the written checks. (Answered 2026-11-17: yes; see the follow-up below.)
- The decision panel and the confirmation strip now hold Tab while open; if the maintainer wants to reach the page
  behind a decision without answering it, D4 (b) is the way back.
- Observed, not changed: while the run workspace is open the page asks for animation frames continuously (743 in the
  second after one orbit key, in an early run of the reduced-motion test, with nothing moving after the first frame;
  the loop was not traced to its owner). Nothing moves, so it is not motion; it is a power and G9 question for F-817.

### Verification (2026-11-17, on studio-roadmap 11b443a8)

- `npm ci`: exit 0. `npm run typecheck`: clean.
- `npm test`: 592 passed, 3 failed, 6 did not run. Two failures were F-816's own and are fixed: the band name's button
  reset used `border-radius` (`tests/unit/workspace.spec.ts` forbids it; now `appearance: none`), and the palette's arrow
  walk read the active option before a loaded machine had drawn it (the walk now waits up to 500 ms for a key's effect);
  the 6 that did not run are the serial `a11y.spec.ts` tests after it. The third is `service.spec.ts:132` (kill and
  restart), known to fail under load. After the fixes, rebuilt and packaged: `tests/unit/workspace.spec.ts` with
  `tests/e2e/a11y.spec.ts` 25 passed; `service.spec.ts` alone 9 passed; `word-budget.spec.ts`, `workspace.spec.ts`
  and `run-control.spec.ts` 23 passed.
- `python scripts/third_party_licenses.py --check`: up to date.
- `cargo test -p rcfd-cli --release --test docs_site --test workspace_graph`: 2 and 3 passed.

### Follow-up (2026-11-17): axe-core, the maintainer's answer to D1

The maintainer, 2026-11-17: "keep axe-core, but don't worry too much about full accessibility features". Also: Tab
stays held inside open dialogs (D4 (a) stands; the decision panel and the confirmation strip keep `aria-modal` and
contain Tab). So D1 is now (a) and (b): the written checks stay, and axe-core runs on top of them. This is not a full
accessibility audit, and no further accessibility work was started.

The new direct dependency (`devDependencies`, test only; nothing of it is in the built app):

| Package | Version | Licence | Why it | Alternatives considered |
|---|---|---|---|---|
| `axe-core` | 4.14.0 (2026-10-05) | MPL-2.0 | The maintainer's word. Its rule set covers what the written checks do not (list structure, ARIA attributes allowed and required, label in name, scrollable regions). It has no dependencies; the test reads `axe.min.js` from the package and puts it into the page through the DevTools protocol, so the app's CSP and code are unchanged. 4.14.1 (2026-11-16) was one day old, under the two-week rule. | `@axe-core/playwright` 4.14.0 (2026-10-15): a thin wrapper that would pull `axe-core` `~4.14.0`, which resolves to 4.14.1 today, unless pinned again; one package more for a call the test makes in four lines. |

What was built:

- `tests/support/axe.ts`: `axeCheck(page)` runs axe on the whole window and sorts the violations into failing (impact
  serious or critical), excluded (serious or critical, on the `EXCLUDED` list with a reason) and other (minor or
  moderate, recorded only). `color-contrast` and `color-contrast-enhanced` are off: there is no WCAG contrast
  requirement (the maintainer, 2026-11-16), and the colours of `DESIGN.md` change only with the maintainer's approval.
- `tests/e2e/a11y.spec.ts`: every surface of the keyboard walk runs axe first, before the walk moves the focus (16
  surface states: home with and without a project, the first path, the Claude Code panel, Runs, a selected run, the
  provenance popover, the run workspace, its derived-picture controls, a select's list, compare mode, the command
  palette, the case editor, its undo controls, case settings and the decision panel). A serious or critical
  violation fails the surface's test; the last test checks that axe ran on every walked surface. The results go to
  `test-results-visual/f816/axe.json`.

Violations found (axe 4.14.0, first run) and fixed:

| Rule (impact) | Where | Fix |
|---|---|---|
| `listitem` (serious) | The Claude Code transcript, on every surface with the panel open: `<ol role="log">` made its `li` items children of a log, not a list | `ClaudePanel.tsx`: the log role and name moved to a wrapping `div.cc-feed-log` with `display: contents` (`claude.css`), so the `ol` stays a list and lays out as before. Also clears the minor `aria-allowed-role` on the `ol`. |
| `label-content-name-mismatch` (serious) | Runs and a selected run: the "Run again" button was named "Run cavity-re100.toml again" (the words shown are not in the name in order) | `RunControls.tsx`: named "Run again: cavity-re100.toml". The icon-only "with fields stored" button keeps its name (nothing shown to match). |
| `label-content-name-mismatch` (serious) | The command palette: each option was named by its title alone while it shows the title, the command id and its keys | `CommandPalette.tsx`: the option is named by its content in the order shown (title, id, group while filtering, keys, the unavailable reason); the visually hidden description that repeated the id and keys is gone. Spaces between the parts and between the keys are text nodes inside flex boxes, so nothing drawn changes. Names still start with the title (`commands.spec.ts`, `f837.spec.ts` match on that). |
| `scrollable-region-focusable` (serious) | The command palette's list scrolls but has no Tab stop: its field moves a virtual focus in it (`aria-activedescendant`) | `CommandPalette.tsx`: the field of the palette (and of a parameter's choice list) is `role="combobox"` with `aria-expanded="true"`, the ARIA pattern for a field over a list it controls; axe then reads the list as the combobox's popup. No Tab stop was added. |

After the fixes, no serious or critical violation is left on any surface. The `EXCLUDED` list is empty: no fix
needed a change to `DESIGN.md` or to a visible design. Left as recorded, not acted on (minor or moderate):
`page-has-heading-one` (moderate) on the run workspace, compare mode, the case editor and its undo controls (no `h1`
there; adding one is a visible change); in compare mode `landmark-unique` (two field views of the same name),
`region` (React Aria's overlay container outside a landmark), both moderate, and `empty-table-header` (minor, the
metric table's first header is named by `aria-label`, not by text).

Decisions taken (each reversible):

- D7, how axe is loaded. (a) `axe-core` alone, injected by the test; (b) `@axe-core/playwright`. Taken: (a), one
  package and an exact pin. Recommendation: (a).
- D8, where axe runs. (a) Inside the keyboard walk, on each of its 16 surface states; (b) a separate spec that reaches
  the same surfaces again. Taken: (a), the surfaces are reached once and stay in step with the walk. Recommendation: (a).

Open for the maintainer: none new. If the maintainer wants the moderate findings above acted on, `page-has-heading-one`
is the one that would change what is shown.

Verification of the follow-up (2026-11-17, on studio-f816b over studio-roadmap 0773c5e2): `npm run typecheck` clean;
after `npm run build && npm run package:dir`, `a11y.spec.ts`, `claude.spec.ts`, `commands.spec.ts`, `f837.spec.ts` and
`word-budget.spec.ts` 53 passed; full `npm test` 654 passed, none failed; `python scripts/third_party_licenses.py
--check` up to date (axe-core added, 123 lock entries).

## Addendum 13: F-837, run logs, runner budgets and field-view extras from rcfd-studio (2026-11-17)

F-837 rebuilds, in the Electron Studio's own design, what the removed egui Studio (`crates/rcfd-studio`, read at
44e63df8 for what it offered only) had beside its run list, field view and project view, where no new engine entry
point is needed. It adds **no dependency**, changes no Rust file, service method, reply shape, golden or default, and
no word budget (G22): every new control is a pictogram named by its tooltip and accessible name, a residual column is
headed by the engine's equation name, designs/ by its folder name, a warning speaks as an exception. `DESIGN.md`,
`docs/studio-guardrails.md` and `PRODUCT.md` are not edited. The full table, the twelve decisions (D1 to D12, options
and the recommendation taken) and the tests are in `apps/studio-web/README.md`, "Run logs, runner budgets and
field-view extras from the egui Studio (F-837)".

### What F-837 built

- Runs: the latest residuals of u and p (`run_status` `last_residual`); the selected run's warnings at rest, its log's
  end and last 50 events on demand (`read_project_file`, `run_status {events}`), Open the report (`shell.openPath`
  after the engine says the file is there), Run again with fields stored (`start_run {store: "plan"}`); the runner's
  budgets (`.rcfd/runner-config.json`: the runner's checks, no write over another writer's change, undo of the last
  write).
- Field view: colour range of the whole run (every stored snapshot, a cancellable work) or fixed (the cells outside
  counted and said); View face on for a 2D case with a scale bar in metres; the domain's extents in the caption; Save
  the view as PNG (the window's picture of the view, legend and caption, to the file a save dialog names).
- Run workspace: the monitor on a log scale (values not above zero counted and said, not drawn).
- Project home: designs/ and its entries; the data files of cases/ apart from the case files; a `*` link on a case
  file whose changes a case-editor session holds unsaved.
- Claude Code panel: a finished turn's model and the summed cost of the session's turns sent from this Studio.
- Start: a project folder named on the command line opens.
- Run port (`studio.run-port`): `log`, `openReport`, `budgets`, `setBudgets`, `undoBudgets`, `savePng`, each checked
  for its exact fields in the main process (`RUN_EXTRA_OPS`; F-810's and F-815's operations unchanged).
- Palette (G20): `runs.log`, `runs.report`, `runs.budgets`, `field.save_png`, `field.range`, `field.face_on`,
  `plots.monitor_log`; a new group, Run workspace. `project.queue_case` takes an optional `store`, default `none` as
  before. Coverage (G15): `capabilities/coverage.json` names the F-837 uses of `read_project_file`, `run_status` and
  `start_run`.

### Needs the engine (not built)

- needs engine: a case-facts query (cell sizes, a velocity scale) or `start_run {dry_run: true}` returning the plan and
  its estimates, for the egui run setup's "simulate N s" helper, adaptive correctors and the Courant fix.
- needs engine: a mesh refine entry point (`refine_mesh {mesh, factor}` or a service method), for Refine by 2.
- needs engine: a service save of a case session to a path outside a project, for opening and saving one case file
  anywhere.
- needs engine: a length unit for the store's coordinates (the Studio says metres from the engine's SI settings, and
  says why).
- Proposed, not needed: `rcfd-runner config set` or an MCP `set_runner_config`, so the budgets are written by their
  owner, atomically.
- (2026-11-17) Collected in "Engine follow-ups of the Studio items" at the end of this ADR, as E-837a to E-837d (the
  length unit is E-809e).

### Not built, no engine needed

- Save or discard unsaved case changes when the window closes (the egui Studio asked): it changes the quit path G7
  holds. Recommendation: a decision panel (F-829) on close only while a case-editor session has pending changes, as a
  follow-up item.

### Found while building

- `run_status`'s `events` has a maximum of 50 (its schema); a first version asked for 200 and every read was refused.
  The e2e test with a solver warning in `events.jsonl` found it.
- The case editor's Edit opens a file in a new session; the unsaved mark therefore links to the session that holds the
  changes, not to Edit.

### Verification (2026-11-17, on studio-f837 over studio-roadmap ca512d5f)

- `npm ci`: exit 0. `npm run typecheck`: clean.
- `npm test`: 629 passed, 1 failed, 1 did not run. The failure is `project.spec.ts:241` (close, read without waiting
  on a loaded machine); `project.spec.ts` alone: 10 passed. `a11y.spec.ts` (11), `word-budget.spec.ts` (every surface
  within its budget) and `f837.spec.ts` (14) passed in the full run.
- `python scripts/third_party_licenses.py --check`: up to date.
- `cargo test -p rcfd-cli --release --test docs_site --test workspace_graph`: 2 and 3 passed.

### The maintainer's answers (2026-11-17)

- Save or discard on close: approved, built below (it was "Not built, no engine needed" above).
- `set_runner_config` (ADR 0733): not built; ADR 0733 stays proposed. The budgets panel keeps writing
  `.rcfd/runner-config.json` itself (D1 (a)).
- D5, `designs/` on the home: the maintainer confirmed (a), the folder's own name as the heading (data, no chrome word).

### Save or discard before a close (F-837, on studio-f837b)

What the egui Studio asked on exit, in the Electron Studio's own design. A close that would lose case-editor changes not
saved asks first with F-829's decision panel (`src/renderer/case/closeGuard.tsx`, mounted once beside every route);
with nothing unsaved nothing asks and the close goes on as before. No dependency, no Rust, no service method and no word
budget changed; `DESIGN.md` is not edited.

- The closes that ask: Close the session (the button and the palette's `case.close`), Close project (the button and the
  palette's `project.close`), and the window: its close button, Alt+F4, the system's Close and the app's quit. The case
  editor is a page with no tabs (its header names the file); closing that "tab" is Close the session, so no other tab
  close exists to guard.
- The window: the main process holds the window's `close` event and sends the page a request
  (`studio.close-request`, main to page); the page answers on a new ipcMain channel, `app.closeAnswer(request,
  answer)`: `asking` at once, then `close` or `cancel`. The preload exposes `app.onCloseRequest(listener)` and
  `app.closeAnswer` (the bridge stays one function per channel, the shell's allow-list tests updated). A page that does
  not say `asking` within 3 s (hung, crashed, not loaded) cannot hold the window; a renderer that crashes or goes away
  while asking lets the window close; a reload ends the question as a cancel. The quit (`before-quit`) asks every
  window before anything is stopped, and a window that cancels cancels the quit with nothing stopped; the shutdown that
  follows is the one before (G7: the service, the project server, the case editor's server, claude, in at most 3 s).
- The words name what is lost (G22: a loss speaks; the panel is an exception surface, its budget 0 chrome words
  unchanged): the title (`Close cases/cavity-re100.toml with 1 change not saved?`, `Close <project> with ...`, `Close
  rcfd Studio with ...`), then "Closing without saving loses them:" and each file with its count and its first four
  setting paths.
- Choices, in the wording of the editor's earlier close question: Save and close (each file through the engine as Save
  does, onto the file as it was opened; a refusal, a removed entry or another writer's change keeps the panel open with
  the engine's words as its title, and nothing closes), Close and lose them, Keep editing (the safe choice: focused when
  the panel opens, and Escape). Tab and Shift+Tab stay in the panel (DecisionPanel, F-816); Keep editing gives the focus
  back where it was, a text field the close blurred included. A value typed but not committed is committed first, as
  Save does, so it is counted and named.
- The case editor's inline close question (Close and lose them, Keep editing) is replaced by the panel, which adds Save
  and close.

Decisions (options, the recommendation taken; each reversible):

- D13, the safe choice. (a) Keep editing, focused, and Escape (taken: F-829's rule, "a loss: keeping is the default",
  and a press of Enter changes nothing); (b) Save and close focused, as many desktop programs do. Recommendation: (a).
- D14, what Close and lose them closes. (a) The session (Close the session) and the project's sessions (Close project)
  with their changes; on the window nothing more than the quit closed before (taken: G7, the Studio's quit stops no
  session or run on the service, and the editor's bindings end with the Studio); (b) close every session with changes
  on the window's close too (a case-editor session could hold a run that G7 keeps going). Recommendation: (a).
- D15, the test-mode quit. (a) Under `RCFD_TEST_HEADLESS=1` the quit does not ask, while a close of the window itself
  still asks (taken: Playwright's `close()` is `app.quit()`, and a test that leaves a change unsaved must not hang its
  teardown; the tests close the window with `BrowserWindow.close()`, what the close button does); (b) ask in test mode
  too and make every test save or discard before it ends. The P10 measurement's quit does not ask either.
  Recommendation: (a).
- D16, the wait for the page. (a) 3 s for `asking`, then no limit while the user decides (taken); (b) one limit for
  the whole answer (a user reading the panel would see the window close under them). Recommendation: (a).

Tests: `tests/e2e/close-guard.spec.ts` (4): Close the session (the panel's words, Tab held, Escape and the focus back,
Save and close by the keyboard writes the file and closes the session); Close project (Keep editing keeps it, Close and
lose them closes the session and the project, the file untouched; nothing unsaved closes at once); the window's own close
(typing not yet committed is counted, Escape gives the focus back to the field, a second close asks again, Save and
close writes the file and the app ends); nothing unsaved closes the window at once, a page with no listener cannot hold
it, and the test-mode quit with a change unsaved does not hang. `case-editor.spec.ts`'s close test now answers the
panel (its last close by the keyboard); `shell.spec.ts`, `project.spec.ts`, `data.spec.ts` and
`service-security.spec.ts` list the new channel and bridge functions, and `shell.spec.ts` checks that a malformed answer
is refused.

### Verification (2026-11-17, on studio-f837b over studio-roadmap ccacac87)

- `npm ci`: exit 0. `npm run typecheck`: clean. `python scripts/third_party_licenses.py --check`: up to date. No
  dependency was added.
- `npx playwright test tests/e2e/close-guard.spec.ts`: 4 passed. The touched specs together (shell, data, project,
  service-security, close-guard, case-editor, a11y, word-budget, commands, first-path, f837, undo-case): 105 passed.
- `npm test` (headless, built and packaged): 658 passed, 0 failed (8.7 min).

## Addendum 14: F-817b, drawing on demand at rest, and a scratch profile per bench case (2026-11-17)

Two follow-ups to F-817 (§9 of ADR 0800). No dependency, no Rust file, no service method, golden or default changed;
`DESIGN.md`, `docs/studio-guardrails.md` and `PRODUCT.md` are not edited.

### 1. The idle frame loop (G9, power)

F-816 saw the run workspace ask for animation frames while nothing moved (743 in the second after one orbit key). The
owner is three.js, not the Studio's code: `WebGPURenderer.render()` (and `compute()`) starts three's own frame loop
(`three/src/renderers/common/Animation.js`), which then asks for a frame on every display refresh for ever, to do its
per-frame bookkeeping (`info.reset()`, `nodeFrame.update()`, a `setAnimationLoop` callback if any). The field canvas
(react-three-fiber with `frameloop="demand"`, which itself stops) and each three.js plot (the residual and monitor
plots, `src/renderer/plots/linePlot.ts`) have a renderer of their own, so the workspace had three such loops. The
counter's stack showed one caller only, three's `Animation.start` loop. On the 60 Hz display of this run that is the
180 per second measured below; at 238 Hz, as in F-816's run, about 714 plus the frames of the key itself.

The fix, `src/renderer/ui/onDemand.ts` (`drawOnDemand(renderer)`, called once after `init()` by `FieldCanvas.tsx` and
`linePlot.ts`): three's loop gets an animation context that lets it ask for two frames after the last `render()` or
`compute()` and then stops it; the next draw starts it again the way three starts it the first time (the bookkeeping of
a frame, then the draw). A draw every frame (an orbit drag, Play, Follow live) keeps the loop running exactly as
before; a user loop set with `setAnimationLoop` keeps it running. What is drawn does not change: every draw sees the
same per-frame state as before (two draws in one frame share three's frame id, draws in different frames do not), which
`tests/unit/on-demand.spec.ts` checks against three's real `Animation` class on a script of draws with rests.

Animation-frame callbacks per second at rest (`tests/e2e/idle-frames.spec.ts`; 1 s to settle, then 2 s counted; packaged
Studio, test mode, 60 Hz display, machine fully loaded, processor busy 1.00):

| Surface at rest | Before | After |
|---|---|---|
| start page (no project) | 0 | 0 |
| project home | 0 | 0 |
| Runs view | 0 | 0 |
| run workspace, field shown, after one orbit key | 180 | 0 |
| run workspace, field shown, after one scrub step | 181.5 | 0 |

The test registers its limit (5 per second) before measuring, counts the callbacks that run (a wrapped
`requestAnimationFrame`, with the callers of the requests in `test-results-visual/f817b/idle-frames.json`), and checks
that the orbit key and the scrub did draw frames (20 and 6 in a run alone, 28 and 6 in the full run), so that a count
of 0 is not a counter that sees nothing.

Picture output: the field, compare, derived, plots and F-837 tests (Save PNG and the face-on view included) pass
unchanged in the full run. P7 and P8 while interacting: `measure-field.mjs --smoke` after the change (scratch profile,
loaded machine) gave P7 R2 median 44 ms, R1 243 ms, P8 60.0 frames per s with vertical sync at 59.9 Hz (p99 frame
16.8 ms) and 54.1 uncapped (recorded, as in F-817's runs); the P8 orbit draws through `renderer.render` every frame,
so the loop keeps running during it.

### 2. Q5: a scratch Chromium profile for every bench case but P10

- `scripts/profile.mjs`: with `--profile-root=<folder>`, `profileArgs()` gives each launch an empty profile
  `<folder>/launch-<n>` as `--user-data-dir` (Electron then keeps userData and every cache there; checked:
  `app.getPath('userData')` is the folder), and `profileRecord()` goes into the script's JSON. Without the flag a
  script launches with the user's own profile, as before.
- `measure-field.mjs` (3 launches), `measure-data.mjs`, `measure-service.mjs`, `measure-scrub.mjs` (one launch in the
  light set, two with Follow live) and `measure-plots.mjs`: `...profileArgs()` in every `electron.launch`, and the
  profile in their JSON. `measure-cold-start.mjs`: unchanged apart from a comment; P10 keeps the user's profile.
- `bench.mjs`: a fresh folder per case in the temporary directory (`rcfd-f817-profile-<P>-*`), passed as
  `--profile-root`, removed when the case has ended (20 tries, 250 ms apart, for helper processes that still hold a
  file); P10 is marked `profile: 'own'`. Each case prints its profile and whether the folder was removed; the summary
  says which cases ran with which; the results keep `cases[].profile`.

Proof, under full load (processor busy 0.99 to 1.00 throughout; the numbers are not a baseline): each script's smoke
case with `--profile-root` made its launch folders (`launch-1` to `launch-3` for the field script), Chromium wrote into
them, and they were removed: field 92 s (numbers above), data 20 s (P4 R2 raw p50 4.5, p99 10.5 ms at 1e5), service
18 s (P2 p99 0.75 ms over 8 events), scrub 21 s (P5 at 1e5: 41.8 per s, longest frame 33.3 ms), plots 28 s (P3 median
0.9 ms). `npm run bench -- --only=P2,P10 --no-gen` (light): P10 with the user's own profile, median 1606 ms, first 1606,
max 2739, met; P2 with a scratch profile (1 launch, folder removed), p99 0.80 ms over 100 events, met; verdict met, 2 of
2 judged lines. The full light set was not run: the machine was fully loaded.

### Decisions (options, the recommendation taken; each reversible)

- D1, where to stop the loop. (a) Give three's loop an animation context that stops it after the last draw, one helper
  for every renderer (taken); (b) patch `Animation.prototype` once for the page (also touches renderers made by code
  that did not ask); (c) call `renderer._animation.stop()` after each draw (drops the bookkeeping tick the next frame's
  draw expects, so two draws in different frames could share a frame id); (d) wait for a three.js release.
  Recommendation: (a). It reads one private field (`_animation`); if a three.js upgrade removes it, `drawOnDemand`
  returns false, the old loop runs again, and the e2e test fails and says so.
- D2, frames after the last draw. (a) Two (taken): a draw every frame keeps the loop running as before, and it stops at
  most three frames after the last draw; (b) one: also correct, but the loop stops and starts every frame while the
  user drags. Recommendation: (a).
- D3, the limit at rest: 5 callbacks per second (the brief's example), registered in the test before measuring; the
  measured value is 0. Recommendation: keep 5, which leaves room for a one-off frame of a late data reply.
- D4, who owns the scratch profile. (a) The bench makes one folder per case and removes it; the scripts take
  `--profile-root` and give each launch a fresh sub-folder (taken); (b) each script makes and removes its own by
  default (changes what a script run alone measures, and a removal while helper processes still run can fail inside
  the script); (c) one folder per case shared by its launches (a case's second launch starts with warm caches).
  Recommendation: (a), as F-817's D1 kept the scripts' defaults and added flags only.
- D5, a script run alone: the user's profile unless `--profile-root` is given (taken), or a scratch profile by default.
  Recommendation: as taken; Q5 names the bench cases.

### Open for the maintainer

- Q-817b-1: should a script run alone also default to a scratch profile (D5, the second option)? Recommendation: no,
  unless a standalone record is to be compared with a bench record.
- The idle baseline of Q4 will be the first bench run with the scratch profiles; the earlier §9 numbers used the shared
  profile.
- The display ran at 60 Hz in this session (238 Hz in F-817's runs); frames per second at rest before the fix scale
  with it.

### Verification (2026-11-17, on studio-f817b after merging studio-roadmap 40365341)

- `npm ci`: exit 0, 0 vulnerabilities. `npm run typecheck`: clean.
- `npm test` (headless, built and packaged): 632 passed, 2 failed, 4 did not run (20.7 min). The failures are the
  known load-flaky `run-control.spec.ts:319` (pause by keyboard: engine answer 20.76 ms against 20; the 4 tests after it
  in the serial file did not run) and `service.spec.ts:132` (kill and restart). Alone: `run-control.spec.ts` 8 passed,
  `service.spec.ts` 9 passed. `idle-frames.spec.ts` (2), `on-demand.spec.ts` (5) and `f837.spec.ts` (14 e2e, 16 unit)
  passed in the full run.
- `python scripts/third_party_licenses.py --check`: up to date.

## Addendum 15: F-819, the Studio on large meshes and on the FAN-01 fan mesh (2026-11-17)

F-819's acceptance: first picture, scrubbing and draw rate measured on a mesh of several million cells and on the FAN-01
mesh with many patches, with the limits registered before the measurement, and any miss stated with its numbers. ADR 0350
§6 left it open: "a fan mesh with many patches and polyhedral cells may change P7".

### What is built

- `scripts/measure-large.mjs` (`npm run bench:large`). Its `LIMITS` are the G9 limits, unchanged: P7 first picture
  <= 1.5 s (median of 5 opens); P5 scrubbing >= 10 snapshots per s with no frame > 150 ms; P8 orbit >= 60 draws per s
  (the better of vertical sync and no rate limit) with a p99 time between draws <= 25 ms (vertical sync); and P6
  >= 3 per s at 4e6, recorded. They are judged on every mesh and in every repeat. They were committed (89090900, "F-819
  limits") before the first measurement and are printed before anything else.
- First picture: the development page's viewport, route R2, field `p` of the last snapshot, 5 opens per mesh, each after
  the tiny box, as `measure-field.mjs`. Scrubbing: the run workspace, Home then ArrowRight through every snapshot of the
  run, about 200 requests per mesh after a warm-up pass, as `measure-scrub.mjs`. Draw rate: a drag on the viewport. The
  drag starts with a trusted press, so OrbitControls takes the mouse pointer, and then the script sends one move of that
  pointer per animation frame, held for 6 s after 1 s of warm-up. A draw is a frame in which the page submitted GPU work
  (a hook on `GPUQueue.submit`, which also times each submit to `onSubmittedWorkDone`). There are two variants: with the
  hover readout (each move picks the cell under the cursor, FieldCanvas `pickAt`), and with the pick skipped (for the
  measurement, the canvas's `getBoundingClientRect` returns an empty rectangle, so `pickAt` returns at once). Each runs
  with vertical sync and with `--disable-frame-rate-limit --disable-gpu-vsync`.
- The processor's busy fraction is taken over 1 s before and after every measurement; the cargo/rustc/link count only at
  the start and the end, because `tasklist` took about 5 minutes once on this machine. Every launch takes
  `--profile-root` (`scripts/profile.mjs`), so each launch gets a fresh profile.
- `scripts/bench.mjs`: under `--heavy` only, one case `F819` runs the script with `--repeats=1` (and `--no-gen` when
  the bench has it). Its lines are printed as recorded, so they do not change the bench's verdict. `--only=F819` runs
  it alone. The README has a section "Large meshes and the FAN-01 mesh (F-819)". No product code changed.

### The meshes

| Name | Cells | Points | Faces (boundary) | Surface triangles | Patches | Cells by faces | Source |
|---|---|---|---|---|---|---|---|
| box-4e6 | 4 000 000 | 4 080 501 | 12 080 000 (160 000) | 320 000 | 6 walls | all 6 | `f447_bench gen 4e6`: the lid-driven box 200 x 200 x 100, 20 steps through the run path, store 1958 MiB (raw f32), made for F-817 |
| fan01-F3 | 4 920 | 5 888 | 15 691 (1 872) | 3 740 | 7: inlet, outlet, seam_lo and seam_hi (cyclic, 40 degrees), hub, shroud, blade | 4910 with 6, 10 with 5 (12 triangular faces) | the committed engine run `example-projects/axial-fan/runs/r-20261030T223519-0489` (1500 iterations, 10 snapshots), copied |
| fan01-F1 | 38 184 | 41 952 | 118 247 (7 410) | 14 816 | the same 7 | 38 164 with 6, 20 with 5 (22 triangular faces) | an engine run made here: `rcfd-runner start cases/fan01-passage-upwind.toml` with F1's counts (12, 24, 12, 15, 6, 21, 10), `solver.max_iterations=20`, `output.store_interval=1`; 20 snapshots, 22 MiB, 78 s |

- No polyhedral FAN-01 mesh exists. FAN-01's mesh is the C-grid passage generator's (`axial_fan_sector`, `c_grid`,
  `camber_seam_h`): hexahedra plus 10 or 20 prism cells, stored in rcfd's face-based format. The only polyhedral path,
  `rcfd_mesh::polydual` (the dual of a tetrahedral mesh), is not reachable from a case. A polyhedral FAN-01 mesh would
  need Rust, which is outside this item.
- 7 patches is what the FAN-01 passage has. The Studio's surface path (`src/main/data/tsstore.ts` `surface()`) does not
  read the patches. It draws every boundary face, the cyclic seams included, as one triangle soup, and fans an n-point
  face into n - 2 triangles. The patch count enters the timed paths only through the number of boundary faces.
- A larger FAN-01 mesh was tried and does not build: F1's counts doubled (24, 48, 24, 30, 12, 42, 20) failed the
  pre-launch check after 75 min ("mesh.fan.front_gap: the line family is folded at layer 14 ... the leading-edge fan
  cannot be built for this blade with the front_gap 0.3 and n_nose 12 given"). The run folder was deleted.

### Results

Machine: AMD Ryzen 7 9800X3D (16 logical), 93.6 GB, Windows 11 (10.0.26300); WebGPU; the display ran at 60 Hz in this
session; Electron 45.3.3 (Chromium 156.0.8078.100); the packaged copy of studio-f819 after merging studio-roadmap
8c31c7e2; the test mode (RCFD_TEST_HEADLESS=1); a scratch profile per launch. **Every number below was taken under
load**: other jobs' test binaries, rustc and cargo ran throughout, 5 to 11 cargo/rustc/link processes, about 29 GB free,
and the processor busy fraction was 0.56 to 1.00 around the measurements (each pair is given). Runs 1 to 3 are one call
of the script (`--repeats=3`, 02:00 to 02:09 UTC). Run 4 is `npm run bench -- --heavy --only=F819 --no-gen` (02:09 to
02:13 UTC). Nothing was retried. An earlier attempt is listed after the tables.

First picture (P7, limit: median <= 1500 ms), the five opens in order, ms:

| Mesh | Run 1 | Run 2 | Run 3 | Run 4 |
|---|---|---|---|---|
| box-4e6 | median 233: 2147, 218, 236, 219, 233 (busy 1.00/1.00) | 179: 1752, 179, 177, 191, 172 (0.85/0.87) | 200: 1834, 201, 200, 183, 197 (0.86/0.82) | 197: 1704, 194, 197, 196, 200 (0.76/0.64) |
| fan01-F3 | 22.6: 29, 16, 19, 23, 27 (0.92/0.93) | 22.9: 20, 23, 26, 31, 18 (1.00/1.00) | 19.5: 19, 20, 25, 16, 20 (0.68/0.74) | 28.5: 29, 32, 22, 23, 29 (1.00/1.00) |
| fan01-F1 | 31.6: 43, 31, 37, 28, 32 (0.81/0.82) | 30.4: 51, 24, 30, 33, 22 (0.97/1.00) | 23.9: 46, 21, 24, 27, 18 (0.65/0.68) | 28.6: 56, 29, 20, 24, 29 (0.99/0.89) |

Scrubbing (P5, limit: >= 10 per s, no frame > 150 ms), the rate, then the longest frame and the p50 and p99 frame:

| Mesh | Run 1 | Run 2 | Run 3 | Run 4 |
|---|---|---|---|---|
| box-4e6 (200 requests) | 11.6 per s; 102, 83.5, 100 ms (0.71/0.88) | 11.1; 117, 85.6, 102 (0.94/0.87) | 11.2; 110, 83.7, 102 (0.94/0.88) | **9.91; 167**, 99.8, 150 (1.00/0.86): **missed** |
| fan01-F3 (20 passes of 10) | 55.2; 32.6, 16.7, 32.5 (0.98/0.88) | 60.3; 32.7, 16.6, 30.5 (0.85/0.82) | 58.4; 33.1, 16.6, 32.6 (0.94/0.82) | 61.6; 17.4, 16.6, 17.1 (0.70/0.56) |
| fan01-F1 (10 passes of 20) | 56.7; 33.5, 16.7, 32.6 (0.90/0.81) | 60.8; 20.1, 16.6, 18.0 (0.81/0.78) | 59.1; 32.2, 16.6, 27.8 (0.84/0.88) | 59.2; 32.9, 16.7, 32.6 (0.78/0.79) |

P6 for box-4e6 (>= 3 per s, recorded) is met in all four runs. The fan scrubs run at the display's 60 Hz.

Draw rate while orbiting (P8, limits: >= 60 draws per s, the better of the two launches; p99 between draws <= 25 ms,
vertical sync). With vertical sync, every drag on every mesh in every run drew 60.0 draws per s (59.8 once, run 4,
fan01-F3 with the pick skipped), with a p99 of 16.8 ms and a maximum of 16.8 to 17.2 ms (33.4 ms once, that same drag).
That meets both limits, but only at the display's rate: 60 is the limit itself, so the rate line is decided by the drags
with no rate limit. Each cell gives the draws per s; the p99 and the maximum time between draws; the GPU time per submit
at p50 and p99 (ms); and the busy fraction before and after:

| Mesh, variant | Run 1 | Run 2 | Run 3 | Run 4 |
|---|---|---|---|---|
| box-4e6, with the readout | 190; 17.4, 32.2; 2.7, 31.6 (1.00/0.97) | 190; 18.3, 32.2; 2.6, 31.7 (1.00/1.00) | 228; 15.0, 16.3; 1.6, 15.0 (0.78/0.64) | 223; 16.0, 17.5; 2.1, 28.6 (0.66/0.69) |
| box-4e6, pick skipped | 909; 2.0, 17.8; 1.4, 3.0 (0.97/0.93) | 913; 2.1, 18.3; 1.3, 3.2 (1.00/0.99) | 757; 2.4, 140; 1.3, 20.3 (0.66/0.81) | 751; 2.6, 139; 2.0, 25.7 (1.00/1.00) |
| fan01-F3, with the readout | 814; 2.5, 19.2; 1.4, 3.4 (0.89/0.87) | 855; 2.3, 18.6; 1.3, 2.9 (0.87/0.92) | 836; 2.3, 21.5; 1.4, 2.9 (0.87/0.86) | 784; 2.6, 19.1; 1.9, 9.8 (1.00/1.00) |
| fan01-F3, pick skipped | 629; 18.6, 248; 1.3, 3.2 (0.92/0.88) | 982; 1.6, 19.2; 1.2, 2.4 (0.80/0.71) | 872; 1.8, 36.3; 1.3, 2.5 (0.86/0.81) | 589; 18.8, 246; 1.8, 66.9 (0.98/1.00) |
| fan01-F1, with the readout | 814; 2.6, 18.7; 1.3, 3.7 (0.91/0.82) | 832; 2.5, 18.6; 1.2, 3.5 (0.71/0.90) | 789; 2.7, 19.0; 1.3, 3.8 (0.83/0.83) | 703; 3.1, 22.5; 2.0, 4.7 (1.00/1.00) |
| fan01-F1, pick skipped | 648; 18.2, 245; 1.2, 3.2 (0.75/0.75) | 611; 18.5, 260; 1.2, 3.1 (0.89/0.82) | 561; 18.8, 261; 1.2, 2.5 (0.78/0.87) | 860; 2.1, 19.6; 1.8, 15.9 (1.00/1.00) |

Verdict: runs 1 to 3 met all 63 judged lines (script exit 0). Run 4 met every line but the 4e6 scrub (P5: 9.91 per s
and a 167 ms frame); the bench prints its lines as recorded.

Earlier attempt (01:56 to 01:58 UTC, the same meshes, run 1 only, then stopped by a fault of the script): P7 box-4e6
median 248 ms (opens 2516, 223, 246, 248, 254; busy 1.00/1.00), fan01-F3 26.3 (31, 33, 23, 26, 16), fan01-F1 35.8 (61,
36, 26, 32, 36). Orbit with vertical sync: 60.0 draws per s, p99 16.8 ms on all six drags. With no rate limit: box-4e6
188 per s with the readout and 927 with the pick skipped, fan01-F3 807 with the readout. The next drag, fan01-F3 with
the pick skipped, drew nothing. The press had landed on what was drawn over the canvas, so OrbitControls never started.
The script now presses at a point where `elementFromPoint` is the canvas, and it stops with the reason when a drag
draws fewer than 2 frames. The script's check runs (`--smoke`, fan01-F3 only) first drove the drag with Playwright's
`mouse.move`. Each call is a round trip of 20 to 30 ms under load, so the drag gave 27.8 to 52.9 draws per s: the
input set that rate, not the Studio. Those runs measured the harness and are not results. They led to the drag of one
move per frame.

### Causes, and fixes proposed (not applied)

- **4e6 scrub, the miss of run 4, and a margin of 11 to 16 % in runs 1 to 3.** Each frame (p50 84 to 100 ms) starts
  with the reader's read of the whole field: 4 000 000 f32 values, 16 MB. Timed outside the Studio, `Store.field` takes
  40 to 43 ms (three fresh Node processes, a warm page cache, busy 0.89). The 16 MB then cross to the renderer, which
  needs only the 320 000 values of the cells that own a surface triangle. Proposed: (a) gather the surface cells' values
  in the data worker and send those (1.3 MB at 4e6), keeping the whole field for the readout's provenance on request;
  (b) cache decoded fields ahead of the scrub direction; (c) leave it as is and record P6 (met) for 4e6. Recommendation:
  (a). It cuts the transfer and the corner-scalar input by 12x at this size and changes no value drawn.
- **The first open of a 4e6 store in a launch (1704 to 2516 ms; judged on the median, so recorded).** Timed outside the
  Studio: `verifyMesh` 1549 to 1644 ms (the FNV-64 of the whole MESH frame, 773 MB, in JavaScript), `surface()` 61 to 66
  ms, the field 40 to 43 ms. Proposed: (a) remember a verified mesh per file identity (path, size, modification time,
  mesh hash) in the Studio's state folder, so the check runs once per store and not once per launch; (b) draw first and
  verify in the background, taking the picture back with the error on a mismatch; (c) a faster checksum loop (32-bit
  halves without BigInt, or WebAssembly); (d) leave it to route R1, where the Rust reader checks. Recommendation: (a)
  with (c). (b) shows a picture before its integrity is known, which DESIGN.md's "what is shown traces to the store"
  argues against.
- **The pick under the cursor at 4e6** (not a miss): with the readout the drag draws 190 to 228 per s, against 751 to
  913 with the pick skipped. That is about 4 ms of pointer handling per move: three's Raycaster tests all 320 000
  triangles, because the geometry has no bounding-volume tree. At 1e6 surface triangles the same cost would be about
  12 ms (extrapolated, not measured), still above 60 per s. Proposed: (a) no pick while a button is held (the readout
  follows the cursor only at rest), with one pick when the drag ends; (b) a bounding-volume tree built in the prepare
  worker, hand-written (no new dependency); (c) one pick per animation frame at most. Recommendation: (a), then (b) if a
  larger surface needs it.
- **Single long frames of about 250 ms** in 5 of the 24 drags with no rate limit (fan01-F3 and F1 with the pick skipped,
  and 139 to 140 ms on box-4e6). This happened once per drag, and the synced drags show nothing like it. Not diagnosed
  (a collection pause or the GPU process are candidates). These drags judge the rate only, not the p99.

### Decisions (options, the recommendation taken; each reversible)

- D1, the FAN-01 mesh. (a) The committed F3 run and an F1 engine run, the finest registered member (taken); (b) F3
  only; (c) a FAN-01 mesh beyond the family (tried: F1 doubled does not build); (d) the `h_slit` topology at high
  counts (a different, lower-fidelity mesh, not FAN-01's registered C-grid). Recommendation: (a).
- D2, the several-million-cell mesh. (a) The generator's largest case, the 4e6 box already stored for F-817 (taken);
  (b) a new engine run of a larger box with the rcfd CLI (over 30 min under this load, with no larger generator size
  without Rust). Recommendation: (a).
- D3, which limits apply to the larger meshes. (a) G9's P7, P5 and P8 unchanged on every mesh, with P6 recorded at 4e6
  (taken: the stricter reading); (b) ADR 0350's P6 instead of P5 at 4e6; (c) limits scaled by surface size.
  Recommendation: (a).
- D4, how the drag is driven. (a) A trusted press, then one pointer move per animation frame from the page (taken);
  (b) Playwright's `mouse.move` (bound by its round trip, tried and dropped); (c) expose `FieldCanvas.bench` on the
  viewport (a product change). Recommendation: (a).
- D5, the hover readout. Both variants are measured and judged (taken), rather than the readout alone or the draw alone.
  The pair separates the cost of the pick from the cost of the draw.
- D6, the bench hook. `--heavy` only, one repeat, lines recorded (taken), rather than judged or kept out of the bench.
  Recommendation: as taken. G9's judged limits belong to the 1e6 sizes.
- D7, the first open. Judged on the median of 5, as P7 has been since ADR 0350, with every open printed (taken), rather
  than judging the first open. Recommendation: as taken, with Q-819-1.

### Open for the maintainer

- Q-819-1: judge the first open of a store in a launch as well (it is what a user sees), and if so with which fix of the
  mesh check (a to d above)? Recommendation: yes, after fix (a) with (c).
- Q-819-2: the 4e6 scrub, at 9.9 to 11.6 per s against 10: apply the surface-cell gather (a)? Recommendation: yes.
- Q-819-3: the pick during a drag (a to c above)? Recommendation: (a).
- Q-819-4: a FAN-01 mesh larger than F1, or a polyhedral one, needs the mesher (other counts, `h_slit`, or a reachable
  polyhedral dual, which needs Rust). Is one wanted for F-448? Recommendation: no. The FAN-01 surfaces are 30 to 100
  times smaller than the 1e6 box's, so the box cases bound them.
- Answers (the maintainer, 2026-11-17): Q-819-1 yes (applied in Addendum 16 with fix (a), and judged since: Addendum 16,
  "The maintainer's answers"); Q-819-2 and Q-819-3 applied in Addendum 16; Q-819-4 no, the polyhedral FAN-01 mesh is
  skipped.
- The display ran at 60 Hz in this session (238 Hz in F-817's runs). The synced rates and the fan scrubs are the
  display's, and the idle baseline of ADR 0800 §9 Q4 should state its display rate.

### Verification (2026-11-17, on studio-f819 after merging studio-roadmap 8c31c7e2)

- `npm ci`: exit 0, 0 vulnerabilities. `npm run typecheck`: clean. `python scripts/third_party_licenses.py --check`: up
  to date. No dependency was added.
- `scripts/measure-large.mjs`: the three-repeat run above (exit 0), the bench's `--heavy --only=F819 --no-gen` (exit 0,
  its lines recorded, the scratch profile folder removed) and `--smoke` runs (fan01-F3).
- `npm test` (headless, built and packaged): 614 passed, 4 failed, 20 did not run (8.7 min). The failures were
  `commands.spec.ts:90` (Ctrl+K, the focus found on `body`), `plots.spec.ts:134` (increments), `project.spec.ts:241`
  (close) and `run-control.spec.ts:192` (quick run by keyboard, the window inactive), and the 20 that did not run follow
  them in serial files. Alone: `commands.spec.ts` 10 passed and `plots.spec.ts` 4 passed. `project.spec.ts` and
  `run-control.spec.ts` failed alone once ("Empty project"; the window inactive) and passed on the second run alone (10
  and 8 passed). No product source changed in this item.

## Addendum 16: F-819b, the three fixes of F-819 on the 4e6 box: surface values, the mesh check remembered, no pick during a drag (2026-11-17)

This follows Addendum 15 (F-819), which measured the misses and proposed the fixes. The maintainer asked for the three
proposals to be applied: Q-819-2 (a), Q-819-1's fixes (a) with (c), and Q-819-3 (a). The G9 limits and the script's
`LIMITS` are unchanged. No Rust and no dependency were added.

### What changed

1. **Scrubbing: surface values, gathered where the whole field is.** The store reader's worker
   (`src/main/data/worker.ts`) keeps the owner cell of every triangle of each open store's surface. It records them the
   first time the surface is asked for, or makes the surface itself on the first `surfaceField`. A new op,
   `surfaceField` (store, snapshot, field; the same checks as `field` in `bridge.ts`), reads the whole field as before
   (`Store.field`, every checksum) and then gathers `out[t] = field[cells[t]]` (`src/shared/surface-values.ts`). The
   gather copies 32-bit words, so each value keeps its stored bits, NaN payloads included. Only the 320 000 values
   (1.22 MiB) cross to the renderer, against 4 000 000 (15.3 MiB) before, together with the store's `meshHash`.
   - The page reaches the op through its own bridge, `window.studioSurfaceValues.field` (preload; `door.ts`
     `loadSurfaceField`). F-804's `window.studioData` keeps exactly its four functions, as it did when F-815 added
     `window.studioCompare`.
   - The field viewport uses surface values for every stored field of a `local` store. The field worker's new
     `surfaceField` op makes the corner scalars and the range from them (`surface.ts` `cornerScalarsOf`, `rangeOf`).
     These take the same values, in the same order and with the same comparisons, as `cornerScalars` and `shownRange`
     take from the whole field.
   - `FieldCanvas` takes `valuesPer: 'triangle'`. The pick reads its value at the triangle's index; before, it read the
     owner cell's. The count of cells outside a fixed range uses `cellsOutsideOf`. The whole run's range (F-837) also
     uses surface values.
   - The viewport refuses surface values whose `meshHash` or length does not match its surface, with the message "the
     store's mesh is not the one its surface was read from".
   - The whole field is still read for route `service` and for a comparison's values (`load`, F-815). The service's
     `data.*` gathers nothing, so `surfaceField` answers `bad_request` there.
2. **The first open: the mesh check remembered for the launch, and a faster loop.**
   - `Store.verifyMesh` (`src/main/data/tsstore.ts`) keeps, in the worker's memory only, each file whose mesh checksum
     matched. The key is the path and the value is an identity: the open file's device, file index, size, and
     modification, change and creation times (nanoseconds, `fstat` with `bigint`), plus the mesh frame's length,
     recorded checksum, hash and counts.
   - A file with the same identity is not hashed again in that launch. Every other file is hashed: a file not verified
     in this launch, another path to the same bytes, a file written in place (even with its modification time set
     back, because the change time moves), a replaced file, and a file that grew. The identity is kept only when it is
     the same before and after the hash.
   - FNV-1a 64 moved to `src/main/data/fnv.ts`. The reference loop is unchanged (`fnv64UpdateJs`). For 4096 bytes or
     more, a WebAssembly loop runs instead, assembled in the source from its instructions: one 64-bit load per eight
     bytes, then the eight byte steps in order, with the bytes copied into the module 1 MiB at a time. FNV-1a must take
     every byte in order whatever the loop, so each byte still costs at least one xor and one 64-bit multiply.
   - Every payload checksum uses the new loop (mesh, DATA blocks, SNAP, FOOT). The mesh check also reuses one 16 MiB
     read buffer instead of making a new array per chunk.
3. **No pick while a button is held.** In `FieldViewport`, `onPointerMove` returns at once when `buttons` is not 0 (an
   orbit or a pan). On the first such move it clears the readout, which would otherwise name a cell no longer under the
   cursor. The later moves of the drag set no state. On `pointerup` on the canvas, it picks where the button was let
   go, or clears the readout when that point is outside the frame (the orbit keeps the pointer captured). Hover without
   a button, the keyboard readout at the centre and the pinned provenance readout work as before.

The F-809 pictures needed no change. The cut plane clips the surface per fragment (rcfd-render's `View::clip`) and uses
no other cell's value. When the arrows are on, they read their velocity components whole through `loadField`
(`useArrows`, unchanged). So the shown field never needs its interior cells, and only those paths read whole fields.

### Bit identity (evidence)

- `tests/unit/data-routes.spec.ts` (new tests) checks the tiny and 1e5 stores, raw and deflated, every snapshot and
  every field:
  - `surfaceField` equals the gather of the Rust reader's reference bytes, with 0 differing bits.
  - On the last snapshot, for every field, the corner scalars equal `cornerScalars` of the whole field byte for byte,
    and `rangeOf` equals `shownRange` of the whole field.
  - `surfaceField` gives the same values when no surface was read first, and its refusals carry their reason.
- `tests/unit/field-surface-values.spec.ts` (new) uses fields with -0, +0, both infinities, two NaNs with payloads, the
  largest and smallest f32, and repeated and skipped owner cells.
  - The gathered words are the cells' words.
  - The corner scalars, the range (`Object.is` on both ends), the cells outside three fixed ranges, and `values[t]`
    against `values[cells[t]]` at every triangle all match the whole field's.
  - The field worker's `surfaceField` answer equals its `field` answer, and the main-thread fallback gives the same.
- These existing tests are unchanged and pass:
  - P12 in Node and in the renderer (`data-routes`, `data.spec`).
  - G3 on the GPU, and the readout bit for bit at every grid point on tiny and 1e5 (`field.spec`).
  - The cut plane and the arrows (`field-derived.spec`).
  - The F-815 difference bit for bit (`compare.spec`), provenance, and the work tests (`work.spec`, the field worker).

### Measurements

The runs were `npm run bench:large -- --no-gen --repeats=3 --profile-root=<scratch>` on box-4e6, fan01-F3 and
fan01-F1. "Before" is studio-f819b at a8933a99, built and packaged unchanged. "After" is the working tree with the three
fixes. The machine is the one of Addendum 15: Ryzen 7 9800X3D, Windows 11, a 60 Hz display, the test mode, and a
scratch profile per launch.

**Both sets were taken on a loaded machine**, with other jobs running throughout:

- Before: busy 0.86 at the start, 6 cargo/rustc/link processes, 28.2 GB free.
- After: busy 0.26 at the start, 1 cargo/rustc/link process, 29.1 GB free, but 0.48 to 1.00 around most measurements
  (each pair is given in the tables).

The after set started on a lighter machine, so load may account for part of each difference. Nothing was retried. Both
sets met 63 of 63 judged lines, exit 0.

First picture, box-4e6 (P7, limit: median <= 1500 ms). Each cell gives the median, then the five opens in order (ms),
then the busy fraction before and after:

| Run | Before | After |
|---|---|---|
| 1 | median 199: **1784**, 199, 199, 206, 188 (1.00/0.90) | median 114: **858**, 106, 114, 105, 114 (0.32/0.28) |
| 2 | 202: **1653**, 175, 172, 202, 207 (0.74/0.80) | 146: **944**, 147, 146, 146, 140 (0.78/0.75) |
| 3 | 161: **1505**, 137, 149, 163, 161 (0.64/0.51) | 132: **933**, 135, 132, 130, 112 (0.72/0.72) |

The first open in a launch is not judged (P7 judges the median). It fell from 1505 to 1784 ms to 858 to 944 ms, so it
is now inside 1.5 s as well. The FAN-01 medians are small and within the load noise in both sets:

- F3: 21.6, 23.9 and 22.0 ms before; 19.1, 26.7 and 24.5 ms after.
- F1: 25.1, 24.9 and 28.6 ms before; 29.7, 35.9 and 31.6 ms after.

Scrubbing, box-4e6 (P5, limit: >= 10 per s, no frame > 150 ms). Each cell gives the rate, then the p50, p99 and longest
frame:

| Run | Before | After |
|---|---|---|
| 1 | 11.9 per s; 83.4, 100, 100 ms (0.76/0.87) | 20.1 per s; 50.0, 51.1, 65.6 ms (0.98/0.65) |
| 2 | 14.8; 66.6, 83.3, 83.4 (0.37/0.31) | 20.1; 50.0, 50.6, 50.8 (0.85/0.93) |
| 3 | 11.3; 83.6, 100, 101 (0.78/0.97) | 20.2; 50.0, 50.5, 51.3 (0.83/0.71) |

- In every after run, the frame takes three display periods (50 ms at 60 Hz). Before, it took four to six.
- The warm-up pass took 1339 to 1728 ms before and 1003 to 1010 ms after.
- P6 (recorded) is met in all six runs.
- The FAN-01 scrubs ran at the display's rate in both sets: 59.0 to 61.1 per s before, 60.4 to 61.5 per s after.

Orbit, box-4e6 (P8, limits: >= 60 draws per s, the better of the two launches; p99 <= 25 ms with vertical sync). With
vertical sync, every drag of both sets drew 60.0 per s with a p99 of 16.8 ms and a maximum of 16.8 to 17.2 ms. With no
rate limit, in draws per s (busy before/after):

| Run | Before, with the readout | Before, pick skipped | After, with the readout | After, pick skipped |
|---|---|---|---|---|
| 1 | 214 (0.76/0.64) | 941 (0.72/1.00) | 2586 (0.75/0.98) | 1365 (0.94/0.99) |
| 2 | 244 (0.48/0.63) | 742 (0.38/0.79) | 2427 (0.86/0.72) | 2702 (0.72/0.81) |
| 3 | 219 (0.95/0.78) | 767 (0.74/0.91) | 2879 (0.49/0.62) | 2794 (0.48/0.70) |

- The drag holds the button, so the variant "with the readout" no longer picks. It now measures what a user gets
  during an orbit, and it equals the pick-skipped control within the noise.
- Both variants rose above the old pick-skipped figure. Before, the pick-skipped variant still set React state on every
  move (it cleared the readout and stored the pointer's position). Now no move of a drag sets state.
- The FAN-01 drags with no rate limit, before:
  - with the readout: F3 777, 1061 and 805; F1 775, 819 and 766;
  - pick skipped: F3 578, 533 and 594; F1 560, 935 and 979.
- The FAN-01 drags with no rate limit, after:
  - with the readout: F3 2184, 2002 and 1894; F1 2245, 2028 and 2344;
  - pick skipped: F3 2294, 917 and 842; F1 2288, 971 and 918.
- The after set also has single long frames of 105 to 261 ms in FAN-01 drags with no rate limit, like the ones
  Addendum 15 left undiagnosed. Those drags judge only the rate.

The reader was also timed outside the Studio: Node 22.19.0, the built `dist/`, the 4e6 store in the page cache, busy
0.89 before and 0.77 after.

| Step | Time |
|---|---|
| `verifyMesh`, WebAssembly loop, 737 MiB mesh frame | 842 ms |
| The reference loop over the same frame (16 MiB chunks, a new array each, as before) | 1163 ms |
| `verifyMesh` again on the same file in the same process (remembered) | 1 ms |
| `surface()` | 56 ms |
| `field(i, 'p')` (40 to 43 ms in Addendum 15; the DATA checksums now use the WebAssembly loop) | 24 to 27 ms |
| The gather | 1 to 3 ms |

On 256 MiB in plain Node, the reference loop ran at 1.1 to 1.2 ns per byte and the WebAssembly loop at 0.85 ns per byte.

### Tests added

- `tests/unit/data-fnv.spec.ts`:
  - the WebAssembly loop runs in Node;
  - the published test vectors;
  - every length from 0 to 80 and from 4096 to 4176 bytes, at view offsets 0 to 7;
  - inputs across the 1 MiB copy;
  - four other start states;
  - input streamed in odd-sized chunks;
  - one changed byte is detected.
- `tests/unit/data-routes.spec.ts` (appended):
  - `surfaceField` bit for bit on the four stores;
  - `surfaceField` before any surface was read, and its refusals (no such field, no such snapshot, `bad_request` on
    the service route);
  - the remembered mesh check, counted with the `meshChecks` counters: a first check hashes and a second is
    remembered; another path is hashed; a byte changed in place, with the size, file index and modification time
    kept, is hashed again and refused, twice; the byte put back is hashed and then remembered; a replacement by a copy
    is hashed; a grown file is hashed and then remembered.
- `tests/unit/data-bridge.spec.ts` (appended): `surfaceField` accepted in its exact shape, and refused wherever `field`
  is refused.
- `tests/unit/field-surface-values.spec.ts`: the tests listed under Bit identity.
- `tests/e2e/field.spec.ts` (one test added): a press and drag on the tiny box. During the drag, the view moves, no
  readout is shown, and the readout's point stays where the press was. At the release, the readout is at the release
  point and shows the stored value of its cell, the same as a plain hover there. The test resets the view afterwards.

### Decisions (options, the recommendation taken; each reversible)

- D1, where the gather runs:
  - (a) in the reader's worker, which holds the store and reads the whole field anyway, through a new op and a bridge
    of its own (taken);
  - (b) an option of `studioData.field` that changes the reply's length (one function would give two shapes, and the
    bridge's key set is pinned);
  - (c) on the main process's thread (against G21).

  Recommendation: (a).
- D2, what the renderer holds:
  - (a) one value per triangle, with `valuesPer` on `FieldCanvas` (taken);
  - (b) the values scattered back into an array of every cell in the field worker (15 MB allocated per step, with
    every other cell's value silently missing);
  - (c) one value per distinct surface cell, plus an index (about half the bytes, but one more array to keep in step).

  Recommendation: (a).
- D3, the colour range: F-808's range covers the cells that own a surface triangle (`shownRange`), not all cells. The
  field worker computes it from the surface values with an identical result, so the reader sends no range.
- D4, the service route:
  - (a) `surfaceField` answers `bad_request` and the viewport reads the whole field (taken);
  - (b) gather in the main process after `data.field` (a loop and a surface cache on the main thread).

  Recommendation: (a), plus a `data.surface_field` method in the service if R1 is ever used for large stores.
- D5, the cut plane and the arrows:
  - (a) always surface values for the shown field, since neither picture needs its interior cells; the arrows read
    their components whole, as before (taken);
  - (b) read the shown field whole while either picture is on (15 MB per step that nothing uses).

  Recommendation: (a).
- D6, the remembered check:
  - (a) in the worker's memory for the launch, keyed by path, with the identity above (taken);
  - (b) kept in the state folder across launches (Addendum 15's (a)); this conflicts with "never skip a file not
    verified in this launch";
  - (c) a sampled checksum (not the store's checksum).

  Recommendation: (a).
- D7, the faster loop:
  - (a) WebAssembly assembled in the source, eight bytes per load (taken: 842 ms against 1163 ms on the 4e6 frame);
  - (b) a JavaScript loop over 32-bit words; the reference loop already works in 32-bit halves, and a variant with a
    floating-point product ran at 3.4 ns per byte, about three times slower;
  - (c) a worker per chunk (FNV-1a cannot be split).

  Recommendation: (a). This is the only WebAssembly in the Studio. It runs in the main process's worker, which the
  page's Content Security Policy does not cover.
- D8, the readout during a drag:
  - (a) cleared at the first held move and taken again at the release (taken);
  - (b) kept as it was during the drag (it would name a cell no longer under the cursor);
  - (c) at most one pick per animation frame (Addendum 15's (c); at this size each pick still costs about 4 ms of a
    frame).

  Recommendation: (a).

### Open for the maintainer

- Q-819b-1 (Q-819-1 again): should the first open of a store in a launch be judged as well? It now takes 858 to 944 ms
  at 4e6 under load, inside 1.5 s. Recommendation: yes, as a fourth P7 line in `measure-large.mjs`. That changes the
  script's registered limits, so it needs the maintainer.
- Q-819b-2: the bench's drag "with the hover readout" no longer measures the pick, because no pick runs during a drag.
  Should a hover sweep with no button held be added, picking on every move as a user's hover does? At 320 000
  triangles a pick still costs about 4 ms per move (Addendum 15). Recommendation: yes, as a recorded line. Build the
  bounding-volume tree of Addendum 15 (b) only if a larger surface makes the hover drop below 60 per s.
- Q-819b-3: a comparison's views (F-815) still carry whole fields. The difference is computed in f64 over every cell
  and sent whole (8 bytes per cell). The reader could apply the same gather to `difference`. Recommendation: yes, when
  compare is measured at 4e6.

### Verification (2026-11-17, on studio-f819b)

- `npm ci`: exit 0, 0 vulnerabilities. `npm run typecheck`: clean. `python scripts/third_party_licenses.py --check`:
  "THIRD_PARTY_LICENSES.md is up to date". No dependency was added.
- `npm test` (headless, built and packaged): 654 passed, 0 failed, 12.6 min. None of the known load-flaky tests failed
  in this run.
- `npm run bench:large`: the two sets of three runs above, both with exit 0 and 63 of 63 judged lines met. The after
  set ran on a package built before one change that only narrows a TypeScript type in `FieldViewport` (a cast replaced
  by an `in` check) and does not change behaviour; `npm test` ran on the final tree.

### The maintainer's answers (2026-11-17), and the first open judged (on studio-f837b)

- Q-819-1 and Q-819b-1: yes, the first open of a store in a launch is judged as well, against the same 1.5 s. Built:
  - `scripts/bench.mjs`: a judged P7 line "first picture of the 1e6 surface, <route>, first open of the launch", the
    first of `measure-field.mjs`'s 5 opens (the first open of the 1e6 store in the Studio's launch, and in the bench
    server's for R1, before the reader's worker remembers its mesh check). As the median line, it is judged on the better
    route, R1 or R2 (ADR 0350 §3); the other route's first open is printed as recorded. The median line is unchanged.
  - `scripts/measure-field.mjs`: says both verdicts on its P7 line and keeps `firstMet` in its JSON.
  - `scripts/measure-large.mjs`: a fourth P7 entry in `LIMITS`, `firstOpen` ("first picture of the surface, first open
    of the store in the launch", <= 1500 ms, judged), one line per mesh and repeat (each repeat is a launch of its own).
    It was added on the maintainer's answer, with the limit unchanged; `measure-large.mjs` has not been run with it (a
    heavy record, house rule 7). Addendum 16's three runs had the 4e6 first open at 858 to 944 ms.
- Q-819-4: no FAN-01 mesh larger than F1 and no polyhedral one; the polyhedral FAN-01 mesh of F-819's acceptance is
  skipped.
- Q-819b-2 and Q-819b-3 had no answer in this round and stay open.

Check of the new line (`npm run bench -- --only=P7,P8 --no-gen`, once, 2026-11-17, processor busy 0.69 to 0.72 from
other jobs): P7 R2 median 41 ms, first open of the launch 289 ms (judged, met); R1 median 1328 ms, first open 1356 ms
(recorded, met); P8 240 frames per s and p99 frame 4.3 ms with vertical sync (met); verdict 4 of 4 judged lines met. The
uncapped P8 run gave 38.3 per s (recorded, not judged).

## Addendum 17: the Studio-only leftovers of F-814, F-813 and F-828 (2026-11-17)

No new dependency, no engine change, no change to a default. On studio-leftovers over studio-roadmap e7b6c43b.

### F-814: a case file named in a reply opens in the case editor

- `src/renderer/claude/links.ts`: `Known` carries the case files of the project (files of `cases/` ending `.toml` or
  `.json`, the engine's rule of ADR 0148, `isCaseFile` of the home); `fileOpensIn(path, known)` says `case` or `home`.
- `src/renderer/claude/ClaudePanel.tsx`: a case file's chip is "Edit <path> in the case editor" and goes to
  `#/case/open/<path>`, as the home's Edit does (F-812 makes the session, binds it and shows the editor). A data file of
  `cases/`, a mesh and a note keep the filtered Home ("Show <path> on the project home").
- Tests: `tests/unit/claude.spec.ts` (where each kind opens), `tests/e2e/claude.spec.ts` (the note filters Home; the
  case chip opens the editor on a session of `rcfd serve`, which the spec now starts; back on Home the transcript and
  its chips are as before; the folder is still unchanged).
- Still waiting for the maintainer, not touched: the claude-head colour role (DESIGN.md), and whether the Studio writes
  `.rcfd/claude-allow.json`. Still the engine's: session discovery (the TypeScript stopgap stays).

### F-813: the result opens the run workspace; the field caption for a learner

- `src/renderer/first/target.ts`: the result's "Open the run" (accessible name "Open the run in the workspace") selects
  the run and shows the run workspace (F-810); a run still going keeps "Follow it in Runs". The project view hands the
  path `openWorkspace` beside `openRun`.
- `FieldViewport` takes `learner: { quantity }`, which the first path passes with its plain word for the field. The
  learner's caption at rest is the run, the plain word with the engine's unit, and "round <step>". The stored name,
  the snapshot and its count, the step and t, the colour range and the cells are said to readers (G22), and the
  readout's cell and triangle are too. The precise terms "round" (step) and "the picture" (the boundary surface's
  triangles coloured by their cells) are added to the result's Explain (G19, PRODUCT Principle 5), and the full control
  names the run workspace. The values and the picture do not change, and the other surfaces keep their caption.
- Unchanged, as asked: the walk's test still writes `rcfd-project.json` itself, because no engine command makes a
  project (ADR 0731, proposed).
- Tests: `tests/e2e/first-path.spec.ts` (the visible caption has none of snapshot, step, t =, triangles, ux; the round
  opens its provenance; the result and the keyboard walk open the workspace with the run drawn),
  `tests/unit/first-path.spec.ts`.

### F-828: the numbers that had no provenance

- A session number is a third subject of the inspector (`{kind: 'session'}`, `SessionNumberSource` and `sessionFacts` in
  `src/shared/provenance.ts`): value, unit as the engine gave it, session, iteration, the service method that answered,
  the session's journal (`journal.get`: its edit count and case hash, read when the popover opens), and the engine of
  `service.info`. There is no run behind it, so there is no run.json, snapshot or reproduce command. What the service
  does not record is said as not recorded: the file a session was made from (neither `session.list` nor `journal.get`
  names it) and a reproduce command (`journal.get` gives the edits, but no journal file exists for `rcfd run --journal`).
- Settings (`KeyRow.tsx`, so the case editor too): a key whose value is a number, or a list of numbers, opens the
  provenance of the engine's value (`settings.get`). Its unit is the schema's `x-rcfd-unit` or `x-rcfd-si-dimension`
  and names which one, or says the schema names none. It opens by Ctrl+I or the palette while the key's field has the
  focus (the row names it with `data-provenance`), or by a click on the key's name, which also puts the focus in the
  field. The name is a passive trigger, as in the runs grid, so Tab and Enter stay the field's. F-816's walk is
  unchanged, and no element is left that no key reaches (the a11y spec showed that a `tabindex="-1"` button would leave
  one).
- Dev plots (`ResidualPlot.tsx`, `MonitorPlot.tsx`, `PlotsDev.tsx`): the latest value of each residual and of the
  monitor (`monitors.get`, its series unit) and the session's iteration (`run.status`) are buttons: click, Enter, Ctrl+I.
  The feed's own counters (fetch counts and times) are the Studio's measurements, not solver numbers, and stay plain.
- The field caption's step and t (the workspace, Compare, the dev field view) and the learner's round open the
  snapshot's provenance (`snapshotFactValue`). The time's unit is said as not recorded, because store.rts gives a
  snapshot's time without one.
- The first path's result numbers (rounds, solver time, the report values) open theirs, with the same sources as Runs.
- The palette's Show provenance now works off a project (a session number needs none; Copy reproduce command still
  needs one). While the palette is open, the number meant is the one that had the focus before the palette opened.
- Tests: `tests/unit/provenance.spec.ts` (the session facts, the not-recorded words, the trigger counts per surface),
  `tests/e2e/settings-rcfd.spec.ts` (click, Ctrl+I, the palette on a real session; the schema's SI dimension named),
  `tests/e2e/plots.spec.ts` (monitor by Enter, residual by click, iteration by Ctrl+I, against `monitors.get` of the
  test's own), `tests/e2e/provenance.spec.ts` (the caption's step and time).
- Numbers still without provenance (Studio work, no engine needed): the Runs table's residual columns u and p (F-837,
  the value is in the tooltip), the compared run's residuals in the time panel (F-815), the legend's ticks, the domain
  size and the scale bar (F-837), the runner budgets (F-837), the home's counts. F-828 stays partial on these.

### Status after this addendum

- F-814 stays partial: what is left is two maintainer decisions and the engine's session discovery (below).
- F-813 stays partial: the engine's project command, and the maintainer's first-time view-and-report.
- F-828 stays partial: the numbers listed above, and the engine's records (below).

### Open for the maintainer

- D-L1 (F-828): should the Studio write a session's journal (`journal.get`) to a file so that a session number gets a
  reproduce command (`rcfd run --journal <file>`)? It would be the Studio writing a file the user did not ask for (G12).
  Recommendation: no. Ask the engine to record the journal file itself (follow-up E-828c).
- The two F-814 decisions are as before: the claude-head colour role, and writing `.rcfd/claude-allow.json`.

### Verification (2026-11-17, on studio-leftovers over studio-roadmap e7b6c43b, RCFD_TEST_HEADLESS=1)

- `npm run typecheck`, `npm run build`, `npm run package:dir`: clean.
- Unit project: 434 passed.
- The touched specs alone (claude, first-path, provenance, settings-rcfd, settings, plots), then word-budget and a11y
  (axe), then case-editor: all passed. Every word budget is unchanged and held. The a11y spec first failed on the
  settings names as `tabindex="-1"` buttons (elements no key reaches), which is why the names are passive triggers.
- The full `npm test` once: 663 passed, none failed or flaky. After it, one reader-only word in the learner caption
  changed ("the last stored picture" to "stored picture", because the viewport does not know that it is the last), and
  first-path.spec.ts passed again on a new build (5 of 5).

## Engine follow-ups of the Studio items (2026-11-17)

Every engine entry point the partial Studio items wait for, in one place, so that one engine ADR can take them
together. The Studio shows each one as unavailable or not recorded, and does not stand in for it (G2, G6, G16). Ids are
local to this list.

| Id | Item | What the engine needs to give | Today in the Studio |
| --- | --- | --- | --- |
| E-809a | F-809 | An iso-surface of a stored cell field (triangles with their cells), as a `data.*` method or an MCP tool | shown unavailable |
| E-809b | F-809 | Streamlines of a stored velocity field (seeds, polylines with the cell of each point) | shown unavailable |
| E-809c | F-809 | A cut or slice of a stored 3D field with values; `render_field` with `axis`, `slice` and `vectors` for a store | the cut and arrows are drawn by the Studio within tolerances T1 to T3 |
| E-809d | F-809, G16 | A CLI or MCP way to draw the Studio's cut and arrows of a store, so Claude Code can make the same picture | none |
| E-809e | F-809, F-837 | The length unit of the stored mesh coordinates | the cut position has no unit; the domain size and the scale bar say m from the SI settings, and say why |
| E-813 | F-813 | A command that creates a project (`rcfd project init`, MCP `create_project` over `Project::create`; ADR 0731, proposed) | the empty folder says it is missing; the walk's test writes `rcfd-project.json` |
| E-814 | F-814 | An engine entry point that lists a project's Claude Code sessions (rcfd-claude) | a TypeScript stopgap in the main process |
| E-828a | F-828 | run.json records the solver's launch argv and a build id (`solver_version` is a release name) | the reproduce command is built from the runner's start options |
| E-828b | F-828 | A unit for `status.json`'s `time`, and for a snapshot's time in store.rts | "Time, s" is the Studio's caption; the provenance says not recorded |
| E-828c | F-828 | A service session's record: the file `session.create` read, and a journal file that `rcfd run --journal` can run | the provenance says not recorded, with no command |
| E-828d | F-828 | `rcfd-runner start` options for `continue_from` and `max_steps`, or `rcfd-runner rerun <id>` / `start_run {from_run}` | said as a gap beside the command |
| E-828e | F-828 | A read-only hash of a mesh file a run names | said as a gap beside the command |
| E-829a | F-829 | Delete or trash for runs, results and cases (`trash_run`, `trash_case`, `list_trash`, `restore_trash`) | the Studio offers no delete |
| E-829b | F-829 | Compare-and-set for `settings.set` (a base value or hash) | the Studio reads the value again before it sends |
| E-837a | F-837 | A case-facts query (cell sizes, a velocity scale) or `start_run {dry_run: true}`, for "simulate N s", adaptive correctors and the Courant fix | not built |
| E-837b | F-837 | A mesh refine entry point (`refine_mesh {mesh, factor}` or a service method), for Refine by 2 | not built |
| E-837c | F-837 | A service save of a case session to a path outside a project, for one case file anywhere | not built |
| E-837d | F-837 | `rcfd-runner config set` or MCP `set_runner_config` (ADR 0733, proposed) | the Studio writes `.rcfd/runner-config.json` conflict-safe, and the runner checks it |
