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

# Reference data: Wave 14 mesh quality and editing (F-233, F-231, ADR 0018)

**Origin:** [docs/reference-data.md](../../docs/reference-data.md), section "Wave 14 mesh quality and editing (F-233, F-231, ADR 0018)" (lines 657-668) at origin/main 2e94e5ef. The doc is generated from `docs/reference-data.d/*.part`.
**Kind:** repo doc section (reference-value audit record). Part of the hub [[doc-reference-data]].

## Summary

Wave 14 mesh quality and editing references (ADR 0018): default limits from memory.

## Key points

The section text follows verbatim so that every value, citation and verification label is quoted exactly. Headings are demoted and relative links rebased to the repo.


Every literature statement below was written from memory; none was checked against a source. None of the registered checks depends on them: every expected value is a closed form derived in the ADR from the project definitions.

| Item | Source | Status |
| --- | --- | --- |
| Face warp as the angle between the normals of the two triangles of a split quadrilateral | common finite-element pre-processing practice | unverified (memory); the maximum over both diagonals and the polygon form for 5 or more vertices are project choices |
| Orthogonal quality as the minimum cosine between the face normal and the centre-to-face and centre-to-neighbour vectors | common finite-volume practice | unverified (memory); ortho-skew (intra-cell part only) and size change (neighbour-to-cell volume ratio) are project definitions |
| Default poor-mesh limits (warp 15 deg, face angle 20 and 160 deg, inverse orthogonal quality 0.9, ortho-skew 0.85, face edge-length ratio 100) | rules of thumb | unverified (memory); not guarantees, adjustable through `Thresholds` |
| Closed forms for the right-corner tetrahedron, sheared and warped hexahedra | derived in ADR 0018 A3 | derived; the decimal expansion printed for the tetrahedron in A3 has a typo, the formula `1 - 3/sqrt(11)` is correct |


## Contradictions with the wiki

None found at ingest (2026-11-22). Contradictions within the docs are collected on [[doc-reference-data]].

## Pages this source informs

- [[doc-reference-data]] (hub)
