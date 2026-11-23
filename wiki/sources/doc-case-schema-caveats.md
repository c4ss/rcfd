---
type: source
tags: [interface, case, turbulence]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: caveats

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "Caveats", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section (short list)

## Summary

A four-bullet list of limits of the turbulence models and the compressible solver.

## Key points

- "Only the standard wall-function formulation is implemented (no scalable or enhanced wall treatment, no rough walls)."
- "k-epsilon is steady-only and incompressible, without heat transfer."
- k-omega SST skin friction and channel velocity depend on the first-cell y+: flat plate Cf -10.5 percent against White's formula at y+ 2 down to -4.8 percent at y+ 0.1; channel bulk velocity +5.4 percent at y+ 2 to +1.4 percent at 0.25 (roadmap note of F-043).
- "The compressible solver is inviscid and explicit; there is no implicit option."

## Contradictions with the wiki

> [!warning] Contradiction (within the repo doc)
> The first two bullets disagree with other sections of the same page: [[doc-case-schema-schemes-wall-motion-limits]] says the wall options of ADR 0015 (`wall_function`, `roughness_height`, ...) are "now" applied by the build path and "no longer rejected", and [[doc-case-schema-turbulence]] lists `scalable`/`enhanced` wall functions and sand-grain roughness and says k-epsilon runs with `pimple` (ADR 0210). The turbulence section itself also still says the `wall_function` non-default values "are rejected with a located error (ADR 0015)". The Caveats list looks older than those sections; which statement is current is not settled by the doc.

## Pages this source informs

- [[rcfd-turb]]
