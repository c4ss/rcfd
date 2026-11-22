---
type: source
tags: [reference-data, structural]
sources: []
origin: docs/reference-data.md
origin_commit: 2e94e5ef
created: 2026-11-22
updated: 2026-11-22
status: draft
---

# Reference data: F-549 a drilling membrane for Shell4 matching the ANDES edge (ADR 0470)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "F-549 a drilling membrane for Shell4 matching the ANDES edge (ADR 0470)" (lines 1716-1727) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

F-549 drilling membrane for Shell4 (ADR 0470).

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


No source was read for F-549; every item is cited from memory. The element is the mean of the two diagonal splits of the quadrilateral into ANDES OPT triangles (the Shell3 membrane of ADR 0466, unchanged); that its edges carry the ANDES edge field with weight one, so that it shares edges with the ANDES Shell3 and passes the mixed membrane patch test, is derived in ADR 0470 arithmetic 1 and measured by MP1 (`2.1e-14`). Nothing numerical is taken from the sources below.

| Item | Status | Source |
|---|---|---|
| A quadrilateral with drilling rotations made of ANDES triangles | from memory | K. Alvin, H. M. de la Fuente, B. Haugen and C. A. Felippa, Finite Elem. Anal. Des. 12 (1992) 163-187; B. Haugen, PhD thesis, University of Colorado, 1994: NOT read |
| A quadrilateral as the mean of its two diagonal splits (overlapping triangles) | from memory | the early membrane literature, not attributed: NOT read; ADR 0470 arithmetic 1 (edge field), 4 (pure bending exact) |
| 8 x 2 in-plane cantilever strip on quadrilaterals: `u_y / delta = 0.9957912994` against the Timoshenko beam of the F-539 row above | derived | ADR 0470 arithmetic 5, check CQ1 |

## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
