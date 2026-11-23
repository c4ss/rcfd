---
type: source
tags: [reference-data, f-item-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-354 immersed solid: DFG 2D-2 force extrema under refinement (ADR 0268)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-354 immersed solid: DFG 2D-2 force extrema under refinement (ADR 0268)" (lines 1489-1497) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-354 immersed solid DFG 2D-2 force extrema under refinement (ADR 0268).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Measurement only; no library code.

| Item | Status | What was read (source) |
|---|---|---|
| DFG 2D-2 level-6 values | read | as the F-219 row above (max `C_D` 3.2200, max `C_L` 0.9859, `St` 0.30188); measured on the D/80 meshes in ADR 0268 addendum 1: T2 `build` 3.22876, 0.98911, 0.30158; T2 bounded 3.22439, 0.98762, 0.30175; M3 `build` 3.22460, 0.98311, 0.30151; M3 bounded 3.22024, 0.98167, 0.30168 |
| Upstream decay of an outlet disturbance in a channel, about `exp(-pi x/H)` | derived | used only to read the outlet measurement P5 of ADR 0268 (no change to six digits when the outlet moves from 2.2 to 4.4) |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
- [[dfg-cylinder-benchmark]]
