---
type: source
tags: [reference-data, wave-section]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: Wave 20C study references (ADR 0034)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 20C study references (ADR 0034)" (lines 821-827) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 20C study references (ADR 0034).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


| Quantity | Source | Status |
| --- | --- | --- |
| Fully developed laminar plane-channel pressure drop `dp = 12 mu U L / h^2` (mean velocity U, gap h) | textbook result, derived from the parabolic profile `u = 6 U (y/h)(1 - y/h)` | derived here from the momentum balance; no external source was consulted (UNVERIFIED against a printed source). Check S7 misses its 1 percent limit at 10 cells across the gap (1.9 percent) and meets it at 20 (0.29 percent) |
| Steady 1-D conduction `q = k A dT / L` | first principles | exact for the linear profile; the depth of the `box_2d` mesh (`lx / nx`) was measured from the mesh volume, not looked up |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
