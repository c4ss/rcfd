# rcfd Studio prototype (run workspace)

A code-led design prototype of the Electron Studio's run workspace, made with impeccable new-work (seed `3c0b0ece`,
Aicher direction). It is the visual reference that `DESIGN.md` is recorded from. It is **not** the product: all data is
synthetic and labelled so, and nothing here talks to the rcfd service. The product shell is F-448 in `apps/studio-web/`.

    npm ci
    npm run dev       # http://localhost:5173
    npm run build && npm run preview

Stack (maintainer, 2026-11-16): React 19, Vite, react-three-fiber, three.js WebGPURenderer with TSL, React Aria
Components, Archivo (OFL, self-hosted through @fontsource). Guardrails: `docs/studio-guardrails.md`.
