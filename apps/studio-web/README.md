# rcfd Studio (Electron)

The Electron desktop front end of rcfd: F-448 and the Studio roadmap of ADR 0800 (F-800 to F-832). This directory is
outside the Cargo workspace (ADR 0350 Q2, guardrail G5). The engine is Rust and stays Rust (G2); this app is a client of
it.

**State: F-800 skeleton.** The window shows an empty project view and nothing else. There is no service client, no project
folder access and no surface yet. The dependencies and this layout are proposed in ADR 0801, which awaits the maintainer.

    npm ci              # exact versions from package-lock.json
    npm run typecheck   # renderer (tsconfig.json) and main + preload (tsconfig.node.json)
    npm run build       # typecheck, then vite build (renderer) and tsc (main, preload) into dist/
    npm start           # electron . (run npm run build first); downloads the Electron binary on first use

## Layout

| Path | Process | Built by | Filled in by |
|---|---|---|---|
| `src/main/` | Electron main process (ES module): windows, and later all service and file I/O | `tsc -p tsconfig.node.json` -> `dist/main/` | F-801 (hardened shell), F-803 (service client), F-804 (bulk data), F-805 (project folder) |
| `src/preload/` | Preload script (CommonJS `.cts`, because a sandboxed preload cannot be an ES module): the IPC allow-list | `tsc -p tsconfig.node.json` -> `dist/preload/` | F-801 |
| `src/renderer/` | Renderer (browser code only, no Node types): React, three.js, React Aria Components | `vite build` -> `dist/renderer/` | F-802 onwards |

`tsconfig.json` (what a bare `tsc --noEmit` checks) covers the renderer; `tsconfig.node.json` covers main and preload.
`vite.config.ts` builds the renderer with `base: './'`, since Electron loads it from the file system.

## Rules

- Guardrails: `docs/studio-guardrails.md` (G1 to G21). Product: `PRODUCT.md`. Visual system: `DESIGN.md`.
- Every dependency is pinned to an exact version (`.npmrc` sets `save-exact`), the lock file is committed, and each
  dependency is listed in `THIRD_PARTY_LICENSES.md` (run `python scripts/third_party_licenses.py` from the repository root
  after any change to `package-lock.json`) and justified in an ADR (ADR 0801 for the first set).
- Not hardened yet: the BrowserWindow uses Electron's defaults (sandbox, context isolation, no Node integration) without
  asserting them, and there is no Content Security Policy, so Electron prints its development-mode security warning.
  F-801 makes these explicit and tests them (G10).
- The design reference is `apps/studio-prototype/` (synthetic data). Nothing is copied from it into this app; the
  surfaces are built from `DESIGN.md` through their own roadmap items.
