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

## Addendum 3: F-811, the residual and monitor plots (2026-11-16)

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
