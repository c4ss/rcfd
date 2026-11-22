---
type: entity
tags: [studio, app]
sources: [doc-glossary, doc-product, doc-readme, doc-architecture]
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Studio (desktop app)

**What it is:** the desktop application, an Electron app in `apps/studio-web/` (ADR 0800), in development and run from a checkout; no installer yet ([[doc-glossary]], [[doc-readme]]).
**Where:** `apps/studio-web/`; plan `docs/ui-restart-plan.md`, guardrails `docs/studio-guardrails.md` (P1).

## Details

- Opens a project folder, edits cases, starts and follows runs through [[rcfd-runner]], shows stored fields through time, compares runs, and registers its project server with Claude Code in its own terminal (ADR 0801 Addendum 23) ([[doc-glossary]]).
- Stack: TypeScript, React 19 + Vite + react-three-fiber, three.js on WebGPU (WebGL2 fallback), React Aria Components; JSON-RPC `rcfd-service/1` ([[doc-product]]).
- Depends on no crate; runs the `rcfd`, `rcfd-runner`, `rcfd-mcp` binaries ([[doc-architecture]]).
- Replaced the egui apps `rcfd-studio` and `rcfd-gui`, removed 2026-11-16 (F-449) ([[doc-product]]).
- Users: learners first, then simulation engineers; Windows 11 is the only measured platform ([[doc-product]]).
- Principles: every value traces to a solver number; limits stay visible; runs outlive the window; nothing UI-only (G15-G17) ([[doc-product]]).

## Contradictions

> [!warning] Contradiction
> [[doc-product]] says stress/strain colouring needs a structural solver 'which does not exist'; [[doc-roadmap-overview]] and [[doc-architecture]] say [[rcfd-fem]] is partly built. Probably stale wording (the Studio may still lack the colouring).

## Related

- [[run]]
- [[field-store]]
- [[mcp]]
- [[rcfd-workspace]]
