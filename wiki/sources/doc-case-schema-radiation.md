---
type: source
tags: [interface, case, heat]
sources: [doc-case-schema]
origin: docs/case-schema.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Case schema: grey radiation

**Origin:** [docs/case-schema.md](../../docs/case-schema.md), section "`radiation` grey radiation (F-130, F-052, ADR 0012)", at `origin/main` 2e94e5ef. Hub: [[doc-case-schema]]
**Kind:** reference section

## Summary

`radiation.model` `none` (default), `rosseland` or `p1`; both need `energy.enabled` and a steady SIMPLE/SIMPLEC run.

## Key points

- `rosseland` adds the radiative conductivity `16 sigma n^2 T^3 / (3 beta)` (optically thick).
- `p1` solves the incident radiation `G` with Marshak conditions and wall `emissivity` in (0, 1] (default 1).
- Not done: non-grey band models, WSGG, discrete ordinates, surface-to-surface radiation, transient and ideal-gas runs.

## Contradictions with the wiki

_None found at ingest._

## Pages this source informs

- [[case-file]]
