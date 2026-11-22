---
type: source
tags: [studio, product]
sources: []
origin: PRODUCT.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: stable
---

# PRODUCT.md

**Origin:** [PRODUCT.md](../../PRODUCT.md) at `origin/main` 2e94e5ef
**Kind:** product brief for the Studio (impeccable product schema)

## Summary

Product definition of the Studio: platform, stack, users, purpose, positioning, constraints, principles and accessibility.

## Key points

- Stack: Electron Studio in `apps/studio-web/` (ADR 0350 Q2), TypeScript, outside the Cargo workspace; JSON-RPC service `rcfd-service/1` for control; Node reader of `store.rts` (R2) for local bulk field data, opt-in `data.*` methods (R1) for remote.
- Maintainer decisions 2026-11-16: three.js on `WebGPURenderer` (WebGL2 fallback) for every visualisation; React 19 + Vite + react-three-fiber; React Aria Components as accessible base; every dependency pinned and justified (guardrail G5).
- Users: first non-CFD learners and undergraduates, then simulation engineers; the maintainer's 120 mm axial computer fan is the first real engineering use.
- A project is a folder of cases, meshes and many runs; runs outlive the window; a detached runner owns runs ([[rcfd-runner]]).
- Guardrails cited: G2 (engine stays Rust), G5, G8 (keyboard/focus/names), G15 (every engine capability reachable from the Studio), G16 (no UI-only capability), G17 (reproducible from CLI, every number shows provenance and unit).
- Six product principles: traceable numbers, honest limits, clear run state, linked time/fields/residuals/monitors, progressive disclosure, Studio parity with the engine.
- No WCAG contrast requirement (removed by the maintainer 2026-11-16); colour-blind-safe colour maps and keyboard access required.
- Only measured platform: Windows 11 (RTX 5080, 60 Hz; ADR 0350 addendum 1).

## Contradictions with the wiki

- PRODUCT.md says stress and strain colouring 'needs a structural solver, which does not exist'; [[doc-architecture]] and [[doc-roadmap-overview]] say a structural solver ([[rcfd-fem]]) is partly built. Likely stale wording: the Studio may still lack the colouring. Flagged on [[studio]].

## Pages this source informs

- [[studio]]
- [[overview]]
