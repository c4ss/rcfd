---
type: source
tags: [interface, case, physics]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: lumped zone components

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`components` lumped zone components (F-113, ADR 0012)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`components.<name>` adds a lumped component to a constant-density `simple`/`simplec`/`piso`/`pimple` run: `fan`, `porous_jump`, `heat_exchanger` (fixed duty, effectiveness, NTU with optional dual cell), `fan_boundary`, `network_outlet`.

## Key points

- `fan`: piecewise-linear `dp(Q)` curve applied as a pressure jump on the downstream internal faces of the zone, under-relaxed (`relax` 0.5).
- NTU exchanger (ADR 0039): arrangements `counterflow` (default), `parallel`, crossflow variants, `shell_and_tube`; `dual_cell` conserves energy to round-off.
- Refused: coupled solver, ideal-gas runs, fractional step, `dynamic_mesh`. A fan or jump zone needs an interior plane downstream.
- "Pressure-driven loops with little resistance are stiff: use a small `relax` (0.02 in the ADR 0012 network test)."
- Not done: 1D network coupling beyond the lumped outlet relation, shell-and-tube or porous exchanger domains.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]]
