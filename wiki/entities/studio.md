---
type: entity
tags: [studio, app]
sources: [doc-glossary, doc-product, doc-readme, doc-architecture, doc-ui-restart-plan, doc-studio-guardrails, doc-studio-coverage, doc-design, doc-design-themes-proposal, doc-impeccable-surfaces, doc-security, doc-process-documentation-review]
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

### From the P1 Studio ingest (2026-11-22)

- Origin: the user's plan of 2026-10-26 to restart the UI (wgpu, egui shell, seven layers L1-L7, a Claude Code chat and plugin); overtaken on 2026-11-16 by the Electron app (ADR 0800) and the deletion of the egui apps before parity ([[doc-ui-restart-plan]]). Its lasting part is the project folder contract (§8), binding under G12 ([[rcfd-project]]).
- Bound by 22 guardrails, G1-G22, approved 2026-11-16 ([[studio-guardrails]], [[doc-studio-guardrails]]).
- Visual system: "The Wayfinding Program" in `DESIGN.md`, with Light, Dark and High contrast themes ([[studio-design-system]], [[doc-design]], [[doc-design-themes-proposal]]); seven surfaces briefed ([[doc-impeccable-surfaces]]).
- Capability coverage (G15) at the snapshot: 1070 of 1363 engine capabilities (78.5 %) have a UI path; all 951 case schema keys are covered through the schema-driven settings surface (F-825) ([[doc-studio-coverage]]).
- Closing the Studio: it owns only a runner it started (`started_by: "studio"`), shuts an idle one down, and asks about active runs (G7 amendment, 2026-11-17) ([[doc-studio-guardrails]]).
- Claude Code: the Studio drives the user's own `claude` CLI; one writer per session, open sessions mirrored read-only, conservative allow-list (G11); it registers `rcfd-project` with `claude mcp add ... --scope local` and gives Claude Code no permissions of its own ([[doc-security]], [[claude-plugin]]).
- Platforms: Windows 11 measured; Linux recorded as "WebGL 2 on software rendering" (ADR 0801 Addendum 19); installers built in CI, signing waits for the maintainer's certificate ([[doc-studio-guardrails]]).
- Human check: [[view-and-report]]; the Studio is one of the two [[work-streams]].

## Contradictions

> [!warning] Contradiction
> [[doc-product]] says stress/strain colouring needs a structural solver 'which does not exist'; [[doc-roadmap-overview]] and [[doc-architecture]] say [[rcfd-fem]] is partly built. Probably stale wording (the Studio may still lack the colouring).

> [!warning] Contradiction (P1 check, 2026-11-22)
> Does a structural solver exist? [[doc-ui-restart-plan]] (2026-10-26) says twice that stress and strain pictures need "a structural solver that does not exist yet"; guardrail G6 ([[doc-studio-guardrails]]) uses the same example. [[doc-studio-coverage]] lists no structural or solid capability, so it settles nothing either way. Neither page settles it. [[doc-process-documentation-review]] (G12, 2026-11-18) lists "No structural solver" statements as stale: "docs/structural-solver.md exists since 2026-11-14; 307 structural items, 133 done", and [[doc-structural-solver]] describes [[rcfd-fem]] ([[structural-solver]]). Reading: the solver exists as a library; what the Studio pages may still mean is that the Studio has no stress/strain colouring. Within the repo docs.

> [!warning] Contradiction
> Path of the Studio's Claude Code bridge: [[doc-architecture]] and the F-814 brief in [[doc-impeccable-surfaces]] say `apps/studio-web/src/main/claude/`; [[doc-security]] says `apps/studio-web/src/main/claudecode/`, the folder present in the snapshot. Within the repo docs.

## Related

- [[run]]
- [[field-store]]
- [[mcp]]
- [[rcfd-workspace]]
- [[studio-guardrails]]
- [[studio-design-system]]
- [[claude-plugin]]
- [[view-and-report]]
- [[work-streams]]
